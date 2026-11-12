//! macOS : sa voix, par la synthèse vocale d'AVFoundation.
//!
//! Deux chemins, une seule file :
//!
//! - **direct**, sans effet : `AVSpeechSynthesizer::speakUtterance`, qui tient lui-même la file de
//!   ce qu'il a à dire. C'est le chemin d'avant les effets, inchangé : à la hauteur et au débit
//!   normaux, l'énoncé ne reçoit que sa voix. Une valeur égale à la normale n'est pas demandée non
//!   plus : sur ce Mac, un débit de 0,5 écrit (la valeur par défaut) change déjà le rendu des voix
//!   « eloquence ».
//! - **traité**, avec effets : un second synthétiseur rend chaque phrase en paquets de son, en
//!   mémoire (`writeUtterance:toBufferCallback:`), que la chaîne de `voice_stage` joue à travers
//!   ses unités d'effet. Ce qui a été vérifié sur ce Mac, et dont tout dépend :
//!   - les paquets arrivent sur le thread principal, par sa boucle (sans elle, rien n'arrive), bien
//!     plus vite que le temps réel ;
//!   - deux énoncés confiés à la suite sont rendus en même temps et peuvent finir dans le
//!     désordre : on n'en confie donc qu'un à la fois ;
//!   - un paquet vide n'est pas la fin : un long énoncé de plusieurs phrases en contient un à
//!     chaque morceau. La fin, c'est `speechSynthesizer:didFinishSpeechUtterance:` du délégué ;
//!   - le format dépend de la voix (16 ou 22,05 kHz ici, mono, flottants) : il est lu à chaque
//!     paquet, et la chaîne remontée quand il change ;
//!   - pendant un tel rendu, `isSpeaking` reste faux et `stopSpeakingAtBoundary:` ne fait rien :
//!     c'est à nous de savoir ce qui reste à dire, et d'ignorer ce qui arrive d'une phrase
//!     abandonnée.
//!
//! La file garde l'ordre entre les deux : une phrase directe attend que la précédente, traitée,
//! ait été jouée, et une phrase traitée attend la fin de la précédente, directe. Au moindre
//! problème du chemin traité (format refusé, moteur qui ne démarre pas ou s'arrête, son qui ne
//! sort pas, rien de rendu), ce qui n'a pas été joué jusqu'au bout est redit par le chemin direct,
//! sans effet, avec une ligne dans le journal ; les effets sont retentés après le prochain
//! [`Voice::stop`] (la réponse suivante).
//!
//! Tout se passe sur le thread de la boucle d'événements : les rappels du système (paquets, fin
//! d'un énoncé, minuteur) ne portent qu'un numéro et retrouvent la voix dans un registre propre à
//! ce thread ; arrivés ailleurs, ils sont ignorés, et la garde du minuteur s'en aperçoit. Seuls les
//! blocs de fin de paquet, appelés sur un thread du système, ne touchent qu'à des atomiques.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use block2::RcBlock;
use capri_core::sound::{Delivery, Effect};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, define_class, msg_send};
use objc2_avf_audio::{
    AVAudioBuffer, AVAudioPCMBuffer, AVSpeechBoundary, AVSpeechSynthesisVoice, AVSpeechSynthesisVoiceGender,
    AVSpeechSynthesizer, AVSpeechSynthesizerDelegate, AVSpeechUtterance, AVSpeechUtteranceDefaultSpeechRate,
    AVSpeechUtteranceMaximumSpeechRate, AVSpeechUtteranceMinimumSpeechRate,
};
use objc2_foundation::{NSDate, NSRunLoop, NSRunLoopCommonModes, NSString, NSThread, NSTimer};

use super::SystemVoice;
use super::stage::{self, Marker, Output, Stage, VoiceFormat};
#[cfg(debug_assertions)]
use super::{Breakage, Rehearsal, Rehearsed};

/// La langue de la voix quand aucune n'est désignée : Capri parle français.
const DEFAULT_LANGUAGE: &str = "fr-FR";
/// Le pas du minuteur qui fait avancer le chemin traité, en secondes.
const TICK: f64 = 0.05;
/// Sans nouvelles d'une phrase en cours de rendu pendant ce temps : le système ne la rendra pas.
const SILENT_SYNTHESIS: Duration = Duration::from_secs(5);
/// Le son confié doit avoir fini d'être joué dans ce délai après sa durée : sinon, il ne sort pas.
const PLAYBACK_MARGIN: Duration = Duration::from_secs(3);
/// Un pas du minuteur plus long que cela : la boucle a été ralentie par le système.
const THROTTLED: Duration = Duration::from_secs(1);
/// Le temps laissé à la queue d'une réverbération ou d'un délai avant d'arrêter le moteur.
const TAIL: Duration = Duration::from_millis(1500);
/// Juste après `speakUtterance`, `isSpeaking` est encore faux (mesuré : une dizaine de ms) ; une
/// phrase directe compte comme dite pendant ce temps, pour qu'aucune autre ne passe devant.
const DIRECT_START: Duration = Duration::from_millis(500);

thread_local! {
    /// Les voix de ce thread, par numéro : ce que les rappels du système retrouvent.
    static VOICES: RefCell<HashMap<u64, Weak<Shared>>> = RefCell::new(HashMap::new());
}
static KEYS: AtomicU64 = AtomicU64::new(1);

pub struct Voice {
    key: u64,
    /// Ni `Send` ni `Sync` : la voix reste sur le thread qui l'a créée.
    shared: Rc<Shared>,
}

/// Ce que partagent la voix et les rappels du système. Un rappel arrivé pendant que l'état est
/// occupé (un appel du système qui rappellerait aussitôt) attend dans `inbox`, que l'occupant vide.
struct Shared {
    inbox: RefCell<VecDeque<News>>,
    state: RefCell<State>,
}

/// Ce que disent les rappels du système.
enum News {
    /// Un paquet de son de la phrase `seq`, déjà lu, ou ce qu'il avait d'illisible.
    Buffer { seq: u64, samples: Result<(VoiceFormat, Vec<Vec<f32>>), String> },
    /// Le synthétiseur a fini de rendre cet énoncé (son adresse).
    Finished { utterance: usize },
    /// Le pas du minuteur.
    Tick,
}

/// Une phrase à dire, sa description déjà bornée.
struct Sentence {
    seq: u64,
    text: String,
    delivery: Delivery,
}

/// La phrase en cours de rendu : il n'y en a qu'une à la fois.
struct InFlight {
    seq: u64,
    /// Retenu jusqu'à la fin, pour reconnaître son `didFinish`.
    utterance: Retained<AVSpeechUtterance>,
    chain: Vec<Effect>,
    /// Le dernier paquet reçu, gardé jusqu'au suivant : c'est ainsi qu'on sait lequel finit la
    /// phrase, et qu'on peut noter quand elle a été jouée.
    held: Option<(VoiceFormat, Vec<Vec<f32>>)>,
    news_at: Instant,
    #[cfg(debug_assertions)]
    frames: usize,
    #[cfg(debug_assertions)]
    rate: f64,
}

/// Un paquet prêt à jouer.
struct Packet {
    seq: u64,
    format: VoiceFormat,
    samples: Vec<Vec<f32>>,
    last: bool,
    chain: Vec<Effect>,
}

/// Le synthétiseur du chemin traité, et son délégué (une propriété faible : on le retient ici).
struct Treated {
    synthesizer: Retained<AVSpeechSynthesizer>,
    _watcher: Retained<Watcher>,
}

impl Treated {
    fn new(key: u64) -> Self {
        // SAFETY: crée un objet sans argument, utilisé seulement depuis ce thread.
        let synthesizer = unsafe { AVSpeechSynthesizer::new() };
        let watcher = Watcher::new(key);
        // SAFETY: le délégué est une propriété faible ; le nôtre vit aussi longtemps que le
        // synthétiseur, dans cette même structure.
        unsafe { synthesizer.setDelegate(Some(ProtocolObject::from_ref(&*watcher))) };
        Self { synthesizer, _watcher: watcher }
    }
}

struct State {
    key: u64,
    direct: Retained<AVSpeechSynthesizer>,
    output: Output,
    /// Le système rappelle sur le thread principal : hors de lui, pas d'effets.
    can_treat: bool,
    queue: VecDeque<Sentence>,
    next_seq: u64,
    /// Quand la dernière phrase directe a été confiée.
    direct_since: Option<Instant>,
    treated: Option<Treated>,
    in_flight: Option<InFlight>,
    /// Confiées au chemin traité, pas encore jouées jusqu'au bout : à redire si tout casse.
    unplayed: VecDeque<Sentence>,
    stage: Option<Stage>,
    /// Des paquets qui attendent que la chaîne soit remontée (une autre voix, d'autres effets),
    /// une fois joué ce qui la traverse encore.
    stash: VecDeque<Packet>,
    /// Change à chaque arrêt ou repli : les fins de paquets d'avant ne comptent plus.
    generation: Arc<AtomicU64>,
    /// La plus haute phrase jouée jusqu'au bout (écrite par les blocs de fin de paquet).
    played: Arc<AtomicU64>,
    /// La plus haute phrase dont le dernier paquet a été confié au lecteur.
    scheduled: u64,
    /// Une partie de la phrase en cours est déjà confiée au lecteur.
    partial: bool,
    /// Quand le son confié devrait avoir fini d'être joué.
    playback_end: Option<Instant>,
    paused: bool,
    /// En pause : ce qui restait à jouer.
    paused_left: Option<Duration>,
    /// Le chemin traité a échoué : tout passe par le chemin direct jusqu'au prochain arrêt.
    broken: bool,
    idle_since: Option<Instant>,
    timer: Option<Retained<NSTimer>>,
    /// Le dernier pas du minuteur : un trou, et la boucle a été ralentie (Capri cachée, App Nap).
    last_tick: Instant,
    #[cfg(debug_assertions)]
    log: Vec<Rehearsed>,
    #[cfg(debug_assertions)]
    breakage: Option<Breakage>,
    #[cfg(debug_assertions)]
    scheduled_seconds: f64,
}

impl Voice {
    pub fn new() -> Option<Self> {
        Some(Self::with_output(Output::Speakers { muted: false }))
    }

    fn with_output(output: Output) -> Self {
        // SAFETY: crée un objet sans argument ; il n'est utilisé ensuite que depuis ce thread,
        // puisque `Retained<AVSpeechSynthesizer>` ne peut pas en changer.
        let direct = unsafe { AVSpeechSynthesizer::new() };
        let key = KEYS.fetch_add(1, Ordering::Relaxed);
        let can_treat = MainThreadMarker::new().is_some();
        if !can_treat {
            log::debug!("voix créée hors du thread principal : elle parlera sans effets");
        }
        let state = State {
            key,
            direct,
            output,
            can_treat,
            queue: VecDeque::new(),
            next_seq: 0,
            direct_since: None,
            treated: None,
            in_flight: None,
            unplayed: VecDeque::new(),
            stage: None,
            stash: VecDeque::new(),
            generation: Arc::new(AtomicU64::new(0)),
            played: Arc::new(AtomicU64::new(0)),
            scheduled: 0,
            partial: false,
            playback_end: None,
            paused: false,
            paused_left: None,
            broken: false,
            idle_since: None,
            timer: None,
            last_tick: Instant::now(),
            #[cfg(debug_assertions)]
            log: Vec::new(),
            #[cfg(debug_assertions)]
            breakage: None,
            #[cfg(debug_assertions)]
            scheduled_seconds: 0.0,
        };
        let shared = Rc::new(Shared { inbox: RefCell::new(VecDeque::new()), state: RefCell::new(state) });
        VOICES.with(|voices| voices.borrow_mut().insert(key, Rc::downgrade(&shared)));
        Self { key, shared }
    }

    pub fn installed() -> Vec<SystemVoice> {
        // SAFETY: méthode de classe sans argument, qui rend un tableau que nous possédons.
        let voices = unsafe { AVSpeechSynthesisVoice::speechVoices() };
        voices
            .to_vec()
            .iter()
            .map(|voice| {
                // SAFETY: simples lectures de propriétés d'une voix vivante, que `voice` retient.
                unsafe {
                    SystemVoice {
                        id: voice.identifier().to_string(),
                        name: voice.name().to_string(),
                        language: voice.language().to_string(),
                        quality: voice.quality().0.clamp(1, 3) as u8,
                        female: voice.gender() == AVSpeechSynthesisVoiceGender::Female,
                    }
                }
            })
            .collect()
    }

    pub fn say(&self, text: &str, delivery: &Delivery) {
        self.shared.state.borrow_mut().enqueue(text, delivery);
        self.shared.pump();
    }

    pub fn stop(&self) {
        self.shared.state.borrow_mut().stop();
        self.shared.pump();
    }

    pub fn pause(&self) {
        self.shared.state.borrow_mut().pause();
        self.shared.pump();
    }

    pub fn resume(&self) {
        self.shared.state.borrow_mut().resume();
        self.shared.pump();
    }

    pub fn is_speaking(&self) -> bool {
        // occupée ailleurs (un rappel en cours) : dans le doute, elle parle
        self.shared.state.try_borrow().map_or(true, |state| state.is_speaking())
    }

    pub fn wait(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let run_loop = NSRunLoop::currentRunLoop();
        loop {
            // la parole avance avec la boucle du thread : on la fait tourner par petites tranches
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
            // avec des effets, on laisse aussi finir la queue de réverbération
            let ringing = self.shared.state.try_borrow().map_or(true, |state| state.is_ringing());
            if !self.is_speaking() && !ringing {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
        }
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.try_borrow_mut() {
            state.stop();
            if let Some(timer) = state.timer.take() {
                timer.invalidate();
            }
            state.stage = None;
        }
        let _ = VOICES.try_with(|voices| voices.borrow_mut().remove(&self.key));
    }
}

impl Shared {
    /// Traite ce qui est arrivé, puis fait avancer la file. Sans effet si l'état est occupé :
    /// l'occupant le fera en rendant la main.
    fn pump(&self) {
        let Ok(mut state) = self.state.try_borrow_mut() else { return };
        loop {
            loop {
                let news = self.inbox.borrow_mut().pop_front();
                match news {
                    Some(news) => state.handle(news),
                    None => break,
                }
            }
            state.advance();
            if self.inbox.borrow().is_empty() {
                return;
            }
        }
    }
}

/// Remet une nouvelle à la voix `key`, sur ce thread. Rend `false` si elle n'existe plus.
fn deliver(key: u64, news: News) -> bool {
    let shared = VOICES.try_with(|voices| voices.borrow().get(&key).and_then(Weak::upgrade)).ok().flatten();
    let Some(shared) = shared else { return false };
    shared.inbox.borrow_mut().push_back(news);
    shared.pump();
    true
}

/// Le bloc qui reçoit les paquets de la phrase `seq` : il les lit tout de suite (le paquet ne vit
/// que le temps de l'appel) et les remet à la voix.
fn on_buffer(key: u64, seq: u64, buffer: NonNull<AVAudioBuffer>) {
    // le registre des voix est celui du thread principal, où le système rappelle (vérifié)
    if !NSThread::isMainThread_class() {
        return;
    }
    // SAFETY: le système passe un paquet vivant le temps de l'appel.
    let buffer = unsafe { buffer.as_ref() };
    // SAFETY: simple lecture d'une propriété du paquet.
    let empty =
        buffer.downcast_ref::<AVAudioPCMBuffer>().is_some_and(|pcm| unsafe { pcm.frameLength() } == 0);
    // un paquet vide n'est pas la fin de la phrase (un long énoncé en a un par morceau)
    if empty {
        return;
    }
    deliver(key, News::Buffer { seq, samples: stage::read(buffer) });
}

/// Le minuteur qui fait avancer le chemin traité : il ne vit que tant qu'il y a quelque chose à
/// faire, sur la boucle de ce thread (dans tous ses modes, pour avancer menu ouvert).
fn start_timer(key: u64) -> Retained<NSTimer> {
    let block = RcBlock::new(move |timer: NonNull<NSTimer>| {
        if !deliver(key, News::Tick) {
            // la voix n'existe plus
            // SAFETY: le système passe le minuteur qui se déclenche, vivant le temps de l'appel.
            unsafe { timer.as_ref() }.invalidate();
        }
    });
    // SAFETY: le minuteur est ajouté à la boucle de ce thread, qui seule l'appelle ; son bloc ne
    // capture qu'un nombre. La constante des modes est celle du système.
    unsafe {
        let timer = NSTimer::timerWithTimeInterval_repeats_block(TICK, true, &block);
        // le système peut regrouper ses réveils : moins de réveils, moins de batterie
        timer.setTolerance(TICK / 2.0);
        NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
        timer
    }
}

/// Le délégué du synthétiseur traité : il ne dit qu'une chose, qu'un énoncé est fini.
struct WatcherIvars {
    key: u64,
}

define_class!(
    // SAFETY: NSObject n'impose rien à ses sous-classes, et `Watcher` n'implémente pas `Drop`.
    #[unsafe(super(NSObject))]
    #[ivars = WatcherIvars]
    struct Watcher;

    // SAFETY: NSObjectProtocol n'a rien à implémenter de plus pour un NSObject.
    unsafe impl NSObjectProtocol for Watcher {}

    // SAFETY: la signature est celle du protocole (synthétiseur et énoncé vivants le temps de
    // l'appel) ; la méthode ne fait que remettre une adresse à la voix, sur le thread principal.
    unsafe impl AVSpeechSynthesizerDelegate for Watcher {
        #[unsafe(method(speechSynthesizer:didFinishSpeechUtterance:))]
        fn did_finish(&self, _synthesizer: &AVSpeechSynthesizer, utterance: &AVSpeechUtterance) {
            if NSThread::isMainThread_class() {
                let utterance = std::ptr::from_ref(utterance) as usize;
                deliver(self.ivars().key, News::Finished { utterance });
            }
        }
    }
);

impl Watcher {
    fn new(key: u64) -> Retained<Self> {
        let this = Self::alloc().set_ivars(WatcherIvars { key });
        // SAFETY: l'initialiseur de NSObject, sur un objet tout juste alloué.
        unsafe { msg_send![super(this), init] }
    }
}

/// L'énoncé d'une phrase : sa voix et, s'ils ne sont pas normaux, sa hauteur et son débit.
fn utterance(text: &str, delivery: &Delivery) -> Retained<AVSpeechUtterance> {
    // SAFETY: les chaînes passées vivent le temps de chaque appel ; un identifiant ou une
    // langue inconnus rendent simplement `nil`.
    let voice = unsafe {
        delivery
            .voice
            .as_deref()
            .and_then(|id| AVSpeechSynthesisVoice::voiceWithIdentifier(&NSString::from_str(id)))
            .or_else(|| {
                AVSpeechSynthesisVoice::voiceWithLanguage(Some(&NSString::from_str(DEFAULT_LANGUAGE)))
            })
    };
    // SAFETY: l'énoncé est créé à partir d'une chaîne valide et reçoit une voix vivante (ou
    // aucune : le système prend alors la sienne) ; hauteur et débit sont dans les plages que
    // documente Apple (`Delivery::bounded`), et ne sont demandés que s'ils ne sont pas normaux.
    unsafe {
        let utterance = AVSpeechUtterance::speechUtteranceWithString(&NSString::from_str(text));
        utterance.setVoice(voice.as_deref());
        if delivery.pitch != 1.0 {
            utterance.setPitchMultiplier(delivery.pitch);
        }
        if delivery.rate != 0.0 {
            utterance.setRate(system_rate(delivery.rate));
        }
        utterance
    }
}

/// Un débit relatif (-1 le plus lent, 0 l'habituel, 1 le plus rapide) dans les unités du système.
fn system_rate(relative: f32) -> f32 {
    // SAFETY: lectures de constantes du système.
    let (slowest, usual, fastest) = unsafe {
        (
            AVSpeechUtteranceMinimumSpeechRate,
            AVSpeechUtteranceDefaultSpeechRate,
            AVSpeechUtteranceMaximumSpeechRate,
        )
    };
    if relative < 0.0 { usual + relative * (usual - slowest) } else { usual + relative * (fastest - usual) }
}

impl State {
    fn enqueue(&mut self, text: &str, delivery: &Delivery) {
        self.next_seq += 1;
        self.queue.push_back(Sentence {
            seq: self.next_seq,
            text: text.to_owned(),
            delivery: delivery.bounded(),
        });
    }

    fn handle(&mut self, news: News) {
        match news {
            News::Buffer { seq, samples } => self.received(seq, samples),
            News::Finished { utterance } => self.finished(utterance),
            News::Tick => self.tick(),
        }
    }

    /// Confie à son chemin ce qui peut l'être, dans l'ordre.
    fn advance(&mut self) {
        while let Some(front) = self.queue.front() {
            let treat = !front.delivery.effects.is_empty() && self.can_treat && !self.broken;
            if !treat {
                // le chemin traité parle encore : la phrase directe attend son tour
                if self.treating() {
                    break;
                }
                let Some(sentence) = self.queue.pop_front() else { break };
                self.speak_direct(&sentence);
                continue;
            }
            if self.in_flight.is_some() || self.direct_speaking() {
                break;
            }
            let Some(sentence) = self.queue.pop_front() else { break };
            self.synthesize(sentence);
        }
        self.keep_time();
    }

    fn speak_direct(&mut self, sentence: &Sentence) {
        let utterance = utterance(&sentence.text, &sentence.delivery);
        if self.output != (Output::Speakers { muted: false }) {
            // une voix d'essai ne fait aucun bruit
            // SAFETY: volume dans la plage documentée (0 à 1), avant de confier l'énoncé.
            unsafe { utterance.setVolume(0.0) };
        }
        // SAFETY: le synthétiseur retient l'énoncé jusqu'à l'avoir dit.
        unsafe { self.direct.speakUtterance(&utterance) };
        self.direct_since = Some(Instant::now());
        #[cfg(debug_assertions)]
        self.note(Rehearsed::Direct {
            text: sentence.text.clone(),
            prosody: !sentence.delivery.natural_prosody(),
        });
    }

    fn synthesize(&mut self, sentence: Sentence) {
        let (key, seq) = (self.key, sentence.seq);
        let treated = self.treated.get_or_insert_with(|| Treated::new(key));
        let utterance = utterance(&sentence.text, &sentence.delivery);
        let block = RcBlock::new(move |buffer: NonNull<AVAudioBuffer>| on_buffer(key, seq, buffer));
        #[cfg(debug_assertions)]
        let mute = self.breakage == Some(Breakage::Silence);
        #[cfg(not(debug_assertions))]
        let mute = false;
        if !mute {
            // SAFETY: l'énoncé reste vivant (retenu jusqu'à sa fin) ; le système copie le bloc et
            // l'appelle sur le thread principal ; il ne capture que deux nombres.
            unsafe {
                treated.synthesizer.writeUtterance_toBufferCallback(&utterance, RcBlock::as_ptr(&block))
            };
        }
        self.in_flight = Some(InFlight {
            seq,
            utterance,
            chain: sentence.delivery.effects.clone(),
            held: None,
            news_at: Instant::now(),
            #[cfg(debug_assertions)]
            frames: 0,
            #[cfg(debug_assertions)]
            rate: 0.0,
        });
        self.unplayed.push_back(sentence);
    }

    fn received(&mut self, seq: u64, samples: Result<(VoiceFormat, Vec<Vec<f32>>), String>) {
        // une phrase abandonnée depuis (arrêt, repli) : ses paquets sont ignorés
        let Some(flight) = self.in_flight.as_mut().filter(|flight| flight.seq == seq) else { return };
        flight.news_at = Instant::now();
        #[cfg(debug_assertions)]
        let samples = match self.breakage {
            Some(Breakage::ForeignFormat) => Err("format inconnu (essai)".to_owned()),
            _ => samples,
        };
        match samples {
            Err(reason) => self.fail(&reason),
            Ok((format, samples)) => {
                #[cfg(debug_assertions)]
                {
                    flight.frames += samples.first().map_or(0, Vec::len);
                    flight.rate = format.rate;
                }
                let chain = flight.chain.clone();
                if let Some((format, samples)) = flight.held.replace((format, samples)) {
                    self.play(Packet { seq, format, samples, last: false, chain });
                }
            }
        }
    }

    fn finished(&mut self, utterance: usize) {
        let current = self
            .in_flight
            .as_ref()
            .is_some_and(|flight| Retained::as_ptr(&flight.utterance) as usize == utterance);
        if !current {
            return;
        }
        let Some(flight) = self.in_flight.take() else { return };
        #[cfg(debug_assertions)]
        if let Some(sentence) = self.unplayed.iter().find(|sentence| sentence.seq == flight.seq) {
            self.note(Rehearsed::Synthesized {
                text: sentence.text.clone(),
                frames: flight.frames,
                rate: flight.rate,
            });
        }
        // une phrase sans aucun son n'a rien à jouer
        if let Some((format, samples)) = flight.held {
            self.play(Packet { seq: flight.seq, format, samples, last: true, chain: flight.chain });
        }
    }

    /// Joue un paquet, ou le met de côté s'il faut d'abord laisser finir ce qui traverse une
    /// chaîne qui ne lui convient pas.
    fn play(&mut self, packet: Packet) {
        if !self.stash.is_empty() {
            self.stash.push_back(packet);
            return;
        }
        if !self.stage.as_ref().is_some_and(|stage| stage.fits(packet.format, &packet.chain)) {
            if self.player_busy() {
                self.stash.push_back(packet);
                return;
            }
            if let Err(reason) = self.rebuild(packet.format, &packet.chain) {
                return self.fail(&reason);
            }
        }
        self.schedule(packet);
    }

    fn rebuild(&mut self, format: VoiceFormat, chain: &[Effect]) -> Result<(), String> {
        self.stage = None;
        #[cfg(debug_assertions)]
        if self.breakage == Some(Breakage::EngineStart) {
            return Err("le moteur audio ne démarre pas (essai)".to_owned());
        }
        self.stage = Some(Stage::build(format, chain, self.output)?);
        Ok(())
    }

    fn schedule(&mut self, packet: Packet) {
        let marker = packet.last.then(|| Marker {
            seq: packet.seq,
            generation: self.generation.load(Ordering::SeqCst),
            current: Arc::clone(&self.generation),
            played: Arc::clone(&self.played),
        });
        let scheduled = match &self.stage {
            Some(stage) => stage.schedule(&packet.samples, marker),
            None => Err("pas de chaîne de traitements".to_owned()),
        };
        let seconds = match scheduled {
            Ok(seconds) => seconds,
            Err(reason) => return self.fail(&reason),
        };
        if packet.last {
            self.scheduled = self.scheduled.max(packet.seq);
        }
        self.partial = !packet.last;
        self.extend_playback(Duration::from_secs_f64(seconds));
        self.idle_since = None;
        let Some(stage) = &self.stage else { return };
        if !self.paused {
            #[cfg(debug_assertions)]
            let was_running = stage.is_running();
            // pas de `return` ici : en version optimisée, où le bloc de répétition qui suit
            // n'existe pas, ce serait la dernière instruction et clippy le refuserait
            let started = stage.start();
            #[cfg(debug_assertions)]
            if started.is_ok() && !was_running {
                if self.breakage == Some(Breakage::EngineStops) {
                    // comme quand le système change de sortie audio : le moteur s'arrête seul
                    stage.halt();
                }
                self.note(Rehearsed::EngineStarted);
            }
            if let Err(reason) = started {
                self.fail(&reason);
            }
        }
    }

    fn extend_playback(&mut self, length: Duration) {
        #[cfg(debug_assertions)]
        {
            self.scheduled_seconds += length.as_secs_f64();
        }
        if self.paused {
            *self.paused_left.get_or_insert(Duration::ZERO) += length;
        } else {
            let now = Instant::now();
            let from = self.playback_end.filter(|end| *end > now).unwrap_or(now);
            self.playback_end = Some(from + length);
        }
    }

    /// Remet à jouer ce qui attendait une autre chaîne, dès que l'ancienne s'est tue.
    fn flush_stash(&mut self) {
        while let Some(front) = self.stash.front() {
            let fits = self.stage.as_ref().is_some_and(|stage| stage.fits(front.format, &front.chain));
            if !fits {
                if self.player_busy() {
                    return;
                }
                let (format, chain) = (front.format, front.chain.clone());
                if let Err(reason) = self.rebuild(format, &chain) {
                    return self.fail(&reason);
                }
            }
            let Some(packet) = self.stash.pop_front() else { return };
            self.schedule(packet);
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_tick) > THROTTLED
            && let Some(flight) = &mut self.in_flight
        {
            // la boucle a été ralentie : les paquets n'ont pas pu arriver, on ne compte pas ce temps
            flight.news_at = now;
        }
        self.last_tick = now;
        self.flush_stash();
        if self.in_flight.as_ref().is_some_and(|flight| now.duration_since(flight.news_at) > SILENT_SYNTHESIS)
        {
            return self.fail("le système n'a pas rendu la phrase");
        }
        let speakers = self.stage.as_ref().is_some_and(|stage| !stage.is_offline());
        if speakers && !self.paused && self.player_busy() {
            if !self.stage.as_ref().is_some_and(Stage::is_running) {
                return self.fail("le moteur audio s'est arrêté");
            }
            if self.playback_end.is_some_and(|end| now > end + PLAYBACK_MARGIN) {
                return self.fail("le son ne sort pas");
            }
        }
        self.prune();
        // plus rien à jouer : le moteur s'arrête, après la queue de réverbération
        if speakers && !self.paused && !self.treating() && self.stage.as_ref().is_some_and(Stage::is_running)
        {
            let since = *self.idle_since.get_or_insert(now);
            if now.duration_since(since) >= TAIL {
                if let Some(stage) = &self.stage {
                    stage.halt();
                }
                self.idle_since = None;
                #[cfg(debug_assertions)]
                self.note(Rehearsed::EngineStopped);
            }
        }
    }

    /// Oublie ce qui a été joué jusqu'au bout.
    fn prune(&mut self) {
        let played = self.played.load(Ordering::SeqCst);
        while self.unplayed.front().is_some_and(|sentence| sentence.seq <= played) {
            let _sentence = self.unplayed.pop_front();
            #[cfg(debug_assertions)]
            if let Some(sentence) = _sentence {
                self.note(Rehearsed::PlayedBack { text: sentence.text });
            }
        }
    }

    /// Le chemin traité a échoué : ce qui n'a pas été joué jusqu'au bout sera redit, dans
    /// l'ordre, par le chemin direct, et plus rien ne passe par les effets jusqu'au prochain arrêt.
    fn fail(&mut self, reason: &str) {
        log::warn!("voix : effets abandonnés ({reason}) ; repli sur la voix directe, sans effet");
        #[cfg(debug_assertions)]
        self.note(Rehearsed::Fallback { reason: reason.to_owned() });
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.stage = None;
        self.broken = true;
        self.in_flight = None;
        self.stash.clear();
        self.partial = false;
        let played = self.played.load(Ordering::SeqCst);
        self.scheduled = played;
        self.playback_end = None;
        self.paused_left = None;
        while let Some(sentence) = self.unplayed.pop_back() {
            if sentence.seq > played {
                self.queue.push_front(sentence);
            }
        }
    }

    fn stop(&mut self) {
        // SAFETY: appel sans autre condition que d'être sur le thread du synthétiseur ; rend
        // `false` s'il n'y avait rien à arrêter.
        unsafe { self.direct.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
        self.direct_since = None;
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.queue.clear();
        self.unplayed.clear();
        self.in_flight = None;
        self.stash.clear();
        self.partial = false;
        self.scheduled = self.played.load(Ordering::SeqCst);
        self.playback_end = None;
        self.paused_left = None;
        self.paused = false;
        self.broken = false;
        self.idle_since = None;
        if let Some(stage) = &self.stage {
            // coupe net, la queue de réverbération comprise
            stage.halt();
        }
        #[cfg(debug_assertions)]
        {
            self.scheduled_seconds = 0.0;
            self.note(Rehearsed::Stopped);
        }
    }

    fn pause(&mut self) {
        // SAFETY: comme `stop` ; sans effet si rien n'est en train d'être dit.
        unsafe { self.direct.pauseSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
        if self.paused {
            return;
        }
        self.paused = true;
        self.paused_left = self.playback_end.take().map(|end| end.saturating_duration_since(Instant::now()));
        if let Some(stage) = &self.stage {
            stage.pause();
        }
        #[cfg(debug_assertions)]
        self.note(Rehearsed::Paused);
    }

    fn resume(&mut self) {
        // SAFETY: comme `stop` ; sans effet si la parole n'était pas suspendue.
        unsafe { self.direct.continueSpeaking() };
        if !self.paused {
            return;
        }
        self.paused = false;
        self.playback_end = self.paused_left.take().map(|left| Instant::now() + left);
        #[cfg(debug_assertions)]
        self.note(Rehearsed::Resumed);
        if self.player_busy()
            && let Some(stage) = &self.stage
            && let Err(reason) = stage.start()
        {
            self.fail(&reason);
        }
    }

    /// Le lecteur a-t-il encore du son à jouer ?
    fn player_busy(&self) -> bool {
        self.partial || self.scheduled > self.played.load(Ordering::SeqCst)
    }

    /// Le chemin traité a-t-il encore quelque chose à rendre ou à jouer ?
    fn treating(&self) -> bool {
        self.in_flight.is_some() || !self.stash.is_empty() || self.player_busy()
    }

    fn direct_speaking(&self) -> bool {
        // SAFETY: simple lecture d'une propriété du synthétiseur, sur son thread.
        let speaking = unsafe { self.direct.isSpeaking() };
        speaking || self.direct_since.is_some_and(|since| since.elapsed() < DIRECT_START)
    }

    fn is_speaking(&self) -> bool {
        self.direct_speaking() || !self.queue.is_empty() || self.treating()
    }

    /// Le moteur tourne-t-il encore (la queue d'une réverbération) ?
    fn is_ringing(&self) -> bool {
        self.stage.as_ref().is_some_and(|stage| !stage.is_offline() && stage.is_running())
    }

    /// Pour les essais : note ce qu'elle fait, seulement si c'est une voix d'essai (une Capri de
    /// débogage qui parle toute la journée ne garde rien).
    #[cfg(debug_assertions)]
    fn note(&mut self, step: Rehearsed) {
        if self.output != (Output::Speakers { muted: false }) {
            self.log.push(step);
        }
    }

    /// Le minuteur ne tourne que tant que le chemin traité a quelque chose à faire. En pause, rien
    /// de ce qu'il fait avancer ne bouge (Capri cachée au milieu d'une phrase peut le rester des
    /// heures) : il s'arrête aussi, et la reprise le relance. Les paquets d'une phrase en cours
    /// de rendu arrivent sans lui.
    fn keep_time(&mut self) {
        let needed = !self.paused && (!self.queue.is_empty() || self.treating() || self.is_ringing());
        if needed && self.timer.is_none() {
            self.timer = Some(start_timer(self.key));
        } else if !needed && let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
    }
}

#[cfg(debug_assertions)]
impl Voice {
    pub fn rehearsal(rehearsal: Rehearsal) -> Option<Self> {
        let output = match rehearsal {
            Rehearsal::Silent => Output::Speakers { muted: true },
            Rehearsal::Offline => Output::Offline,
        };
        Some(Self::with_output(output))
    }

    pub fn rehearsal_log(&self) -> Vec<Rehearsed> {
        self.shared.state.borrow().log.clone()
    }

    pub fn break_effects(&self, breakage: Option<Breakage>) {
        self.shared.state.borrow_mut().breakage = breakage;
    }

    pub fn played_seconds(&self) -> Option<f64> {
        self.shared.state.borrow().stage.as_ref().and_then(Stage::played_seconds)
    }

    pub fn run_for(&self, length: Duration) {
        let deadline = Instant::now() + length;
        let run_loop = NSRunLoop::currentRunLoop();
        while Instant::now() < deadline {
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.02));
        }
    }

    pub fn render(&self, limit: Duration) -> Result<Vec<f32>, String> {
        let deadline = Instant::now() + limit;
        let run_loop = NSRunLoop::currentRunLoop();
        loop {
            {
                let state = self.shared.state.borrow();
                if state.queue.is_empty() && state.in_flight.is_none() && state.stash.is_empty() {
                    break;
                }
            }
            if Instant::now() >= deadline {
                return Err("le rendu en mémoire n'a pas fini à temps".to_owned());
            }
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.02));
        }
        let rendered = {
            let state = self.shared.state.borrow();
            let stage = state.stage.as_ref().ok_or("rien n'a traversé la chaîne")?;
            let frames = ((state.scheduled_seconds + TAIL.as_secs_f64()) * 48_000.0) as usize;
            stage.render(frames)
        };
        self.shared.pump();
        let _ = deliver(self.key, News::Tick);
        rendered
    }
}
