//! macOS : la chaîne de traitements de la voix, un `AVAudioEngine` en mémoire :
//!
//! lecteur (`AVAudioPlayerNode`, au format de la voix) → mélangeur (`AVAudioMixerNode`, qui le
//! convertit en 48 kHz stéréo, un format que toutes les unités d'effet du système acceptent) →
//! unités d'effet, dans l'ordre de la description → mélangeur principal → sortie du Mac (ou,
//! pour les essais, rendu hors ligne en mémoire, sans périphérique).
//!
//! AVFAudio lève une exception Objective-C sur un format ou un branchement invalide, et Rust ne
//! sait pas l'attraper : le processus s'arrêterait (la fonction `exception` d'objc2 le permettrait,
//! mais elle compile du C et n'est pas parmi nos dépendances). Tout ce qui pourrait en lever est
//! donc vérifié avant l'appel : formats créés par nous (flottants séparés, fréquence et canaux
//! vérifiés), nœuds attachés avant d'être branchés, paquets créés avec le format même du
//! branchement du lecteur, `play` seulement moteur démarré, bandes d'égaliseur parcourues sans
//! index. Ce qui peut échouer autrement rend une erreur, et la voix se replie sur la voix directe.

use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use block2::RcBlock;
use capri_core::sound::{self, DistortionKind, Effect, Filter, Room, Sample};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_avf_audio::{
    AVAudioBuffer, AVAudioCommonFormat, AVAudioEngine, AVAudioFormat, AVAudioMixerNode, AVAudioNode,
    AVAudioPCMBuffer, AVAudioPlayerNode, AVAudioPlayerNodeCompletionCallbackType, AVAudioUnitDelay,
    AVAudioUnitDistortion, AVAudioUnitDistortionPreset, AVAudioUnitEQ, AVAudioUnitEQFilterType,
    AVAudioUnitReverb, AVAudioUnitReverbPreset,
};
use objc2_foundation::NSError;

/// La fréquence d'échantillonnage et le nombre de canaux des traitements.
const FX_RATE: f64 = 48_000.0;
const FX_CHANNELS: u32 = 2;
/// Les fréquences d'une voix qu'on accepte de traiter (celles de ce Mac : 16 et 22,05 kHz).
const VOICE_RATES: std::ops::RangeInclusive<f64> = 8_000.0..=192_000.0;
/// Le plus grand nombre de canaux d'un paquet qu'on sait lire (au-delà de deux : ramené à un).
const MAX_VOICE_CHANNELS: usize = 8;
/// La taille d'un rendu hors ligne, en trames.
#[cfg(debug_assertions)]
const RENDER_FRAMES: u32 = 4096;

/// Le format du son d'une voix, une fois en canaux séparés de flottants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceFormat {
    pub rate: f64,
    pub channels: u32,
}

/// Où va le son.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Les haut-parleurs du Mac. `muted` : à volume nul, pour les essais.
    Speakers { muted: bool },
    /// Nulle part : rendu à la demande, en mémoire, pour les essais.
    #[cfg(debug_assertions)]
    Offline,
}

/// De quoi noter qu'une phrase a fini d'être jouée, depuis le thread où le système le dit.
pub struct Marker {
    pub seq: u64,
    /// La génération à laquelle elle a été confiée ; une autre, et c'est une phrase abandonnée.
    pub generation: u64,
    pub current: Arc<AtomicU64>,
    /// La plus haute phrase jouée jusqu'au bout.
    pub played: Arc<AtomicU64>,
}

pub struct Stage {
    engine: Retained<AVAudioEngine>,
    player: Retained<AVAudioPlayerNode>,
    /// Le format du branchement du lecteur : tout paquet confié au lecteur est créé avec lui.
    voice_format: Retained<AVAudioFormat>,
    #[cfg(debug_assertions)]
    fx_format: Retained<AVAudioFormat>,
    format: VoiceFormat,
    chain: Vec<Effect>,
    output: Output,
}

impl Stage {
    /// Monte la chaîne pour une voix de ce format et ces traitements (déjà bornés). Rien ne joue
    /// encore : [`Stage::start`] démarre le moteur.
    pub fn build(format: VoiceFormat, chain: &[Effect], output: Output) -> Result<Self, String> {
        if !VOICE_RATES.contains(&format.rate) || !(1..=2).contains(&format.channels) {
            return Err(format!("format de voix inattendu ({} Hz, {} canaux)", format.rate, format.channels));
        }
        let voice_format = standard(format.rate, format.channels).ok_or("format de voix refusé")?;
        let fx_format = standard(FX_RATE, FX_CHANNELS).ok_or("format des traitements refusé")?;
        // SAFETY: création d'un moteur, sans argument ; il n'est utilisé que sur ce thread.
        let engine = unsafe { AVAudioEngine::new() };
        #[cfg(debug_assertions)]
        if output == Output::Offline {
            // SAFETY: le moteur est neuf, donc arrêté, et ni son mélangeur ni sa sortie n'ont été
            // touchés (ce que demande Apple) ; le format est standard, l'erreur est rendue.
            unsafe {
                engine.enableManualRenderingMode_format_maximumFrameCount_error(
                    objc2_avf_audio::AVAudioEngineManualRenderingMode::Offline,
                    &fx_format,
                    RENDER_FRAMES,
                )
            }
            .map_err(|error| format!("pas de rendu hors ligne : {}", describe(&error)))?;
        }
        // SAFETY: créations de nœuds sans argument.
        let (player, resampler) = unsafe { (AVAudioPlayerNode::new(), AVAudioMixerNode::new()) };
        let effects: Vec<Retained<AVAudioNode>> = chain.iter().map(effect_node).collect();
        // SAFETY: chaque nœud est neuf et attaché une seule fois à ce moteur avant d'être branché ;
        // les formats des branchements sont des formats standard que nous venons de créer (le
        // mélangeur accepte n'importe quel format PCM, les unités d'effet le 48 kHz stéréo). Le
        // mélangeur principal reçoit sur sa prochaine entrée libre.
        unsafe {
            engine.attachNode(&player);
            engine.attachNode(&resampler);
            for node in &effects {
                engine.attachNode(node);
            }
            engine.connect_to_format(&player, &resampler, Some(&voice_format));
            let mut previous: &AVAudioNode = &resampler;
            for node in &effects {
                engine.connect_to_format(previous, node, Some(&fx_format));
                previous = node;
            }
            let mixer = engine.mainMixerNode();
            engine.connect_to_format(previous, &mixer, Some(&fx_format));
            if output == (Output::Speakers { muted: true }) {
                mixer.setOutputVolume(0.0);
            }
            engine.prepare();
        }
        Ok(Self {
            engine,
            player,
            voice_format,
            #[cfg(debug_assertions)]
            fx_format,
            format,
            chain: chain.to_vec(),
            output,
        })
    }

    /// Est-elle montée pour ce format et ces traitements ?
    pub fn fits(&self, format: VoiceFormat, chain: &[Effect]) -> bool {
        self.format == format && self.chain == chain
    }

    /// Confie au lecteur ce paquet, à la suite des précédents ; `marker` : c'est le dernier d'une
    /// phrase, à noter quand il aura été joué. Rend sa durée, en secondes.
    pub fn schedule(&self, samples: &[Vec<f32>], marker: Option<Marker>) -> Result<f64, String> {
        let frames = samples.first().map_or(0, Vec::len);
        let fits = samples.len() == self.format.channels as usize
            && samples.iter().all(|channel| channel.len() == frames)
            && frames > 0
            && u32::try_from(frames).is_ok();
        if !fits {
            return Err("paquet de son incohérent".to_owned());
        }
        // SAFETY: le format est celui du branchement du lecteur, standard donc PCM (sinon
        // l'initialisation lèverait) ; elle rend nil si la taille ne tient pas.
        let buffer = unsafe {
            AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
                AVAudioPCMBuffer::alloc(),
                &self.voice_format,
                frames as u32,
            )
        }
        .ok_or("paquet de son impossible à créer")?;
        // SAFETY: la longueur est la capacité demandée juste au-dessus.
        unsafe { buffer.setFrameLength(frames as u32) };
        // SAFETY: un paquet en flottants séparés : un pointeur par canal du format, chacun vers
        // `frames` échantillons à nous, qu'on remplit sans déborder.
        unsafe {
            let data = buffer.floatChannelData();
            if data.is_null() {
                return Err("paquet de son sans données".to_owned());
            }
            for (index, channel) in samples.iter().enumerate() {
                let target: NonNull<f32> = *data.add(index);
                std::ptr::copy_nonoverlapping(channel.as_ptr(), target.as_ptr(), frames);
            }
        }
        let handler = marker.map(|marker| {
            RcBlock::new(move |_kind: AVAudioPlayerNodeCompletionCallbackType| {
                // une phrase abandonnée depuis (arrêt, repli) ne compte pas comme jouée
                if marker.current.load(Ordering::SeqCst) == marker.generation {
                    marker.played.fetch_max(marker.seq, Ordering::SeqCst);
                }
            })
        });
        let kind = match self.output {
            Output::Speakers { .. } => AVAudioPlayerNodeCompletionCallbackType::DataPlayedBack,
            #[cfg(debug_assertions)]
            Output::Offline => AVAudioPlayerNodeCompletionCallbackType::DataRendered,
        };
        // SAFETY: le paquet a exactement le format du branchement du lecteur (le même objet) :
        // ses canaux correspondent, et la planification ne lève pas. Le bloc, que le système
        // copie et appelle sur un de ses threads, ne touche qu'à des atomiques partagés ; sans
        // marqueur, aucun bloc (nil est permis).
        unsafe {
            self.player.scheduleBuffer_completionCallbackType_completionHandler(
                &buffer,
                kind,
                handler.as_ref().map_or(std::ptr::null_mut(), RcBlock::as_ptr),
            );
        }
        Ok(frames as f64 / self.format.rate)
    }

    /// Démarre le moteur s'il ne tourne pas, et le lecteur s'il ne joue pas.
    pub fn start(&self) -> Result<(), String> {
        // SAFETY: simple lecture d'une propriété.
        let running = unsafe { self.engine.isRunning() };
        if !running {
            // SAFETY: démarrage d'un moteur préparé ; l'erreur est rendue, sans exception.
            unsafe { self.engine.startAndReturnError() }
                .map_err(|error| format!("le moteur audio ne démarre pas : {}", describe(&error)))?;
        }
        // SAFETY: `play` lève une exception si le moteur ne tourne pas : vérifié juste avant.
        unsafe {
            if !self.engine.isRunning() {
                return Err("le moteur audio ne tourne pas".to_owned());
            }
            if !self.player.isPlaying() {
                self.player.play();
            }
        }
        Ok(())
    }

    /// Suspend tout, la queue de réverbération comprise, sans rien oublier.
    pub fn pause(&self) {
        // SAFETY: suspendre un moteur, démarré ou non, est toujours permis.
        unsafe { self.engine.pause() };
    }

    /// Arrête tout : ce qui restait à jouer est oublié. Jamais depuis un bloc de fin de paquet
    /// (Apple prévient que cela peut bloquer) : seulement depuis le thread de la voix.
    pub fn halt(&self) {
        // SAFETY: arrêter un lecteur et un moteur, en marche ou non, est toujours permis.
        unsafe {
            self.player.stop();
            self.engine.stop();
        }
    }

    pub fn is_running(&self) -> bool {
        // SAFETY: simple lecture d'une propriété.
        unsafe { self.engine.isRunning() }
    }

    pub fn is_offline(&self) -> bool {
        !matches!(self.output, Output::Speakers { .. })
    }

    /// Pour les essais : le temps joué par le lecteur depuis son départ, en secondes.
    #[cfg(debug_assertions)]
    pub fn played_seconds(&self) -> Option<f64> {
        // SAFETY: simples lectures de temps ; nil quand le lecteur ne joue pas.
        unsafe {
            let now = self.player.lastRenderTime()?;
            let time = self.player.playerTimeForNodeTime(&now)?;
            time.isSampleTimeValid().then(|| time.sampleTime() as f64 / time.sampleRate())
        }
    }

    /// Pour les essais, hors ligne : rend `frames` trames de la sortie, en stéréo entrelacée.
    #[cfg(debug_assertions)]
    pub fn render(&self, frames: usize) -> Result<Vec<f32>, String> {
        use objc2_avf_audio::AVAudioEngineManualRenderingStatus;
        if self.output != Output::Offline {
            return Err("rendu hors ligne demandé à une chaîne qui joue".to_owned());
        }
        // SAFETY: le format de rendu du moteur, standard ; nil si la taille ne tient pas.
        let out = unsafe {
            AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
                AVAudioPCMBuffer::alloc(),
                &self.fx_format,
                RENDER_FRAMES,
            )
        }
        .ok_or("paquet de rendu impossible à créer")?;
        let mut rendered = Vec::with_capacity(frames * 2);
        let mut done = 0;
        while done < frames {
            let wanted = RENDER_FRAMES.min((frames - done) as u32);
            let mut error: *mut NSError = std::ptr::null_mut();
            // SAFETY: `renderOffline:toBuffer:error:` (non lié par objc2-avf-audio) : le moteur
            // est en rendu hors ligne et démarré, le paquet a le format de rendu et assez de
            // place, l'erreur est un pointeur de sortie que nous possédons.
            let status: AVAudioEngineManualRenderingStatus = unsafe {
                objc2::msg_send![&*self.engine, renderOffline: wanted, toBuffer: &*out, error: &mut error]
            };
            if status != AVAudioEngineManualRenderingStatus::Success {
                return Err(format!("rendu hors ligne : statut {}", status.0));
            }
            // SAFETY: le paquet de rendu, en flottants séparés sur deux canaux, contient
            // `frameLength` trames valides.
            unsafe {
                let count = out.frameLength() as usize;
                let data = out.floatChannelData();
                if data.is_null() || count == 0 {
                    return Err("rendu hors ligne vide".to_owned());
                }
                let left = std::slice::from_raw_parts((*data).as_ptr(), count);
                let right = std::slice::from_raw_parts((*data.add(1)).as_ptr(), count);
                for (l, r) in left.iter().zip(right) {
                    rendered.push(*l);
                    rendered.push(*r);
                }
                done += count;
            }
        }
        Ok(rendered)
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        self.halt();
    }
}

/// Les échantillons d'un paquet rendu par le synthétiseur, en canaux séparés de flottants (au
/// plus deux), et leur format. Une erreur pour ce qu'on ne sait pas lire.
pub fn read(buffer: &AVAudioBuffer) -> Result<(VoiceFormat, Vec<Vec<f32>>), String> {
    let pcm = buffer.downcast_ref::<AVAudioPCMBuffer>().ok_or("un paquet qui n'est pas du son PCM")?;
    // SAFETY: simples lectures de propriétés d'un paquet vivant le temps de l'appel.
    let (format, frames, stride) = unsafe { (pcm.format(), pcm.frameLength() as usize, pcm.stride()) };
    // SAFETY: de même pour son format.
    let (common, rate, channels, interleaved) = unsafe {
        (format.commonFormat(), format.sampleRate(), format.channelCount() as usize, format.isInterleaved())
    };
    if !VOICE_RATES.contains(&rate) || !(1..=MAX_VOICE_CHANNELS).contains(&channels) {
        return Err(format!("format de son inattendu ({rate} Hz, {channels} canaux)"));
    }
    if stride != if interleaved { channels } else { 1 } {
        return Err("disposition du son inattendue".to_owned());
    }
    // SAFETY: chaque accesseur est celui qui correspond au format commun du paquet (les autres
    // rendent nil) ; voir `gather`.
    let samples = unsafe {
        match common {
            AVAudioCommonFormat::PCMFormatFloat32 => {
                gather(pcm.floatChannelData(), channels, frames, interleaved)
            }
            AVAudioCommonFormat::PCMFormatInt16 => {
                gather(pcm.int16ChannelData(), channels, frames, interleaved)
            }
            AVAudioCommonFormat::PCMFormatInt32 => {
                gather(pcm.int32ChannelData(), channels, frames, interleaved)
            }
            _ => None,
        }
    }
    .ok_or_else(|| format!("échantillons illisibles (format {})", common.0))?;
    let channels = samples.len() as u32;
    Ok((VoiceFormat { rate, channels }, samples))
}

/// Les échantillons d'un paquet, par les pointeurs de ses canaux.
///
/// # Safety
///
/// `data` est nul, ou pointe vers `channels` pointeurs non nuls (la documentation d'Apple) : en
/// séparé, chacun vers `frames` échantillons ; en entrelacé, le premier vers `frames × channels`
/// échantillons. Le paquet vit le temps de l'appel.
unsafe fn gather<T: Sample>(
    data: *mut NonNull<T>,
    channels: usize,
    frames: usize,
    interleaved: bool,
) -> Option<Vec<Vec<f32>>> {
    if data.is_null() {
        return None;
    }
    let (pointers, length) =
        if interleaved { (1, frames.checked_mul(channels)?) } else { (channels, frames) };
    let slices: Vec<&[T]> = (0..pointers)
        // SAFETY: voir le contrat de la fonction.
        .map(|index| unsafe { std::slice::from_raw_parts((*data.add(index)).as_ptr(), length) })
        .collect();
    sound::planar(&slices, channels, frames, interleaved)
}

/// Un format standard (flottants séparés), ou `None` si le système le refuse (plus de deux
/// canaux, notamment).
fn standard(rate: f64, channels: u32) -> Option<Retained<AVAudioFormat>> {
    // SAFETY: initialise un objet tout juste alloué ; rend nil plutôt que de lever.
    unsafe {
        AVAudioFormat::initStandardFormatWithSampleRate_channels(AVAudioFormat::alloc(), rate, channels)
    }
}

/// L'unité d'effet d'un traitement, réglée. Les valeurs sont déjà dans leurs plages
/// (`Delivery::bounded`) ; les fréquences restent sous la moitié de celle des traitements.
fn effect_node(effect: &Effect) -> Retained<AVAudioNode> {
    let nyquist = (FX_RATE / 2.0) as f32 * 0.95;
    // SAFETY: créations d'unités sans argument, puis réglages de leurs propriétés, aux valeurs
    // bornées par la documentation d'Apple ; un préréglage chargé avant le mélange, qu'il
    // remettrait sinon à sa valeur.
    unsafe {
        match effect {
            Effect::Reverb { room, mix } => {
                let unit = AVAudioUnitReverb::new();
                unit.loadFactoryPreset(reverb_preset(*room));
                unit.setWetDryMix(*mix);
                Retained::into_super(Retained::into_super(Retained::into_super(unit)))
            }
            Effect::Delay { seconds, feedback, cutoff, mix } => {
                let unit = AVAudioUnitDelay::new();
                unit.setDelayTime(f64::from(*seconds));
                unit.setFeedback(*feedback);
                unit.setLowPassCutoff(cutoff.min(nyquist));
                unit.setWetDryMix(*mix);
                Retained::into_super(Retained::into_super(Retained::into_super(unit)))
            }
            Effect::Distortion { kind, pre_gain, mix } => {
                let unit = AVAudioUnitDistortion::new();
                unit.loadFactoryPreset(distortion_preset(*kind));
                unit.setPreGain(*pre_gain);
                unit.setWetDryMix(*mix);
                Retained::into_super(Retained::into_super(Retained::into_super(unit)))
            }
            Effect::Equalizer { bands, gain } => {
                // au moins une bande ; celles qu'on ne règle pas restent court-circuitées (leur
                // défaut, d'après Apple)
                let unit = AVAudioUnitEQ::initWithNumberOfBands(AVAudioUnitEQ::alloc(), bands.len().max(1));
                for (parameters, band) in unit.bands().to_vec().iter().zip(bands) {
                    parameters.setFilterType(filter_type(band.filter));
                    parameters.setFrequency(band.frequency.min(nyquist));
                    parameters.setBandwidth(band.bandwidth);
                    parameters.setGain(band.gain);
                    parameters.setBypass(false);
                }
                unit.setGlobalGain(*gain);
                Retained::into_super(Retained::into_super(Retained::into_super(unit)))
            }
        }
    }
}

fn reverb_preset(room: Room) -> AVAudioUnitReverbPreset {
    match room {
        Room::SmallRoom => AVAudioUnitReverbPreset::SmallRoom,
        Room::MediumRoom => AVAudioUnitReverbPreset::MediumRoom,
        Room::LargeRoom => AVAudioUnitReverbPreset::LargeRoom,
        Room::MediumHall => AVAudioUnitReverbPreset::MediumHall,
        Room::LargeHall => AVAudioUnitReverbPreset::LargeHall,
        Room::Plate => AVAudioUnitReverbPreset::Plate,
        Room::MediumChamber => AVAudioUnitReverbPreset::MediumChamber,
        Room::LargeChamber => AVAudioUnitReverbPreset::LargeChamber,
        Room::Cathedral => AVAudioUnitReverbPreset::Cathedral,
        Room::LargeRoom2 => AVAudioUnitReverbPreset::LargeRoom2,
        Room::MediumHall2 => AVAudioUnitReverbPreset::MediumHall2,
        Room::MediumHall3 => AVAudioUnitReverbPreset::MediumHall3,
        Room::LargeHall2 => AVAudioUnitReverbPreset::LargeHall2,
    }
}

fn distortion_preset(kind: DistortionKind) -> AVAudioUnitDistortionPreset {
    use AVAudioUnitDistortionPreset as Preset;
    match kind {
        DistortionKind::DrumsBitBrush => Preset::DrumsBitBrush,
        DistortionKind::DrumsBufferBeats => Preset::DrumsBufferBeats,
        DistortionKind::DrumsLoFi => Preset::DrumsLoFi,
        DistortionKind::MultiBrokenSpeaker => Preset::MultiBrokenSpeaker,
        DistortionKind::MultiCellphoneConcert => Preset::MultiCellphoneConcert,
        DistortionKind::MultiDecimated1 => Preset::MultiDecimated1,
        DistortionKind::MultiDecimated2 => Preset::MultiDecimated2,
        DistortionKind::MultiDecimated3 => Preset::MultiDecimated3,
        DistortionKind::MultiDecimated4 => Preset::MultiDecimated4,
        DistortionKind::MultiDistortedFunk => Preset::MultiDistortedFunk,
        DistortionKind::MultiDistortedCubed => Preset::MultiDistortedCubed,
        DistortionKind::MultiDistortedSquared => Preset::MultiDistortedSquared,
        DistortionKind::MultiEcho1 => Preset::MultiEcho1,
        DistortionKind::MultiEcho2 => Preset::MultiEcho2,
        DistortionKind::MultiEchoTight1 => Preset::MultiEchoTight1,
        DistortionKind::MultiEchoTight2 => Preset::MultiEchoTight2,
        DistortionKind::MultiEverythingIsBroken => Preset::MultiEverythingIsBroken,
        DistortionKind::SpeechAlienChatter => Preset::SpeechAlienChatter,
        DistortionKind::SpeechCosmicInterference => Preset::SpeechCosmicInterference,
        DistortionKind::SpeechGoldenPi => Preset::SpeechGoldenPi,
        DistortionKind::SpeechRadioTower => Preset::SpeechRadioTower,
        DistortionKind::SpeechWaves => Preset::SpeechWaves,
    }
}

fn filter_type(filter: Filter) -> AVAudioUnitEQFilterType {
    match filter {
        Filter::Parametric => AVAudioUnitEQFilterType::Parametric,
        Filter::LowPass => AVAudioUnitEQFilterType::LowPass,
        Filter::HighPass => AVAudioUnitEQFilterType::HighPass,
        Filter::ResonantLowPass => AVAudioUnitEQFilterType::ResonantLowPass,
        Filter::ResonantHighPass => AVAudioUnitEQFilterType::ResonantHighPass,
        Filter::BandPass => AVAudioUnitEQFilterType::BandPass,
        Filter::BandStop => AVAudioUnitEQFilterType::BandStop,
        Filter::LowShelf => AVAudioUnitEQFilterType::LowShelf,
        Filter::HighShelf => AVAudioUnitEQFilterType::HighShelf,
        Filter::ResonantLowShelf => AVAudioUnitEQFilterType::ResonantLowShelf,
        Filter::ResonantHighShelf => AVAudioUnitEQFilterType::ResonantHighShelf,
    }
}

/// Une erreur du système, pour le journal.
fn describe(error: &NSError) -> String {
    format!("{} ({})", error.localizedDescription(), error.code())
}
