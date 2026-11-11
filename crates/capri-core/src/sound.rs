//! Comment dire une phrase, décrit sans rien savoir du système qui la dira : avec quelle voix, à
//! quelle hauteur, à quel débit, et à travers quels traitements du son, appliqués après la
//! synthèse.
//!
//! `capri_chat::voice` compose cette description à partir des réglages (ses préréglages et leur
//! intensité) ; `capri_platform::Voice` l'applique avec ce que le système sait faire (sur macOS,
//! les unités d'effet d'AVFAudio), sans rien savoir de Capri ni de ce que veut dire « construct ».
//! Elle est ici, comme `form`, parce que les deux doivent la voir et que `capri-chat` ne peut pas
//! dépendre de `capri-platform`.
//!
//! Les bornes de chaque réglage sont celles que documente Apple dans les en-têtes d'AVFAudio du
//! SDK de macOS (`AVAudioUnitReverb.h`, `AVAudioUnitDelay.h`, `AVAudioUnitDistortion.h`,
//! `AVAudioUnitEQ.h`, `AVSpeechSynthesis.h`) ; [`Delivery::bounded`] y ramène toute valeur, et
//! remplace ce qui n'est pas un nombre par une valeur neutre. Les fréquences sont bornées à
//! 20 kHz : le système traite le son à une fréquence d'échantillonnage d'au moins 44,1 kHz.
//!
//! On y trouve aussi de quoi remettre des échantillons rendus par le système en canaux séparés de
//! flottants ([`planar`]), ce que les unités d'effet attendent.

use std::ops::RangeInclusive;

/// Combien de traitements au plus dans une chaîne : au-delà, ils sont ignorés.
pub const MAX_EFFECTS: usize = 6;
/// Combien de bandes au plus dans un égaliseur.
pub const MAX_BANDS: usize = 6;

/// La plage du multiplicateur de hauteur (`AVSpeechUtterance.pitchMultiplier` : 0,5 à 2).
pub const PITCH: RangeInclusive<f32> = 0.5..=2.0;
/// La plage du débit, relative à celle du système : -1 le plus lent qu'il permet, 0 son débit
/// habituel, 1 le plus rapide.
pub const RATE: RangeInclusive<f32> = -1.0..=1.0;
/// Le mélange du son traité et du son d'origine, en pour cent (0 : rien que l'origine).
pub const MIX: RangeInclusive<f32> = 0.0..=100.0;
/// La durée d'un délai, en secondes.
pub const DELAY_SECONDS: RangeInclusive<f32> = 0.0..=2.0;
/// La part de la sortie d'un délai réinjectée à son entrée, en pour cent.
pub const FEEDBACK: RangeInclusive<f32> = -100.0..=100.0;
/// La coupure des aigus d'un délai, en hertz.
pub const CUTOFF: RangeInclusive<f32> = 10.0..=20_000.0;
/// Le gain avant une distorsion, en décibels.
pub const PRE_GAIN: RangeInclusive<f32> = -80.0..=20.0;
/// La fréquence d'une bande d'égaliseur, en hertz.
pub const FREQUENCY: RangeInclusive<f32> = 20.0..=20_000.0;
/// La largeur d'une bande d'égaliseur, en octaves.
pub const BANDWIDTH: RangeInclusive<f32> = 0.05..=5.0;
/// Le gain d'une bande d'égaliseur, ou de l'égaliseur entier, en décibels.
pub const GAIN: RangeInclusive<f32> = -96.0..=24.0;

/// Comment dire une phrase.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    /// L'identifiant d'une voix installée ; `None`, ou une voix disparue : celle du système.
    pub voice: Option<String>,
    /// Multiplicateur de hauteur ([`PITCH`]) : 1, la hauteur de la voix.
    pub pitch: f32,
    /// Débit, relatif à la plage du système ([`RATE`]) : 0, son débit habituel.
    pub rate: f32,
    /// Les traitements du son, dans l'ordre où il les traverse. Vide : la voix telle que le
    /// système la dit, sans aucun détour.
    pub effects: Vec<Effect>,
}

impl Delivery {
    /// Cette voix, à sa hauteur et à son débit, sans traitement.
    pub fn plain(voice: Option<String>) -> Self {
        Self { voice, pitch: 1.0, rate: 0.0, effects: Vec::new() }
    }

    /// La hauteur et le débit sont-ils ceux de la voix ? Alors rien ne doit en être demandé au
    /// système : même une valeur égale à la sienne peut changer le rendu de certaines voix.
    pub fn natural_prosody(&self) -> bool {
        self.pitch == 1.0 && self.rate == 0.0
    }

    /// La même description, chaque valeur ramenée dans sa plage : ce que `capri-platform`
    /// applique. Ce qui n'est pas un nombre devient neutre (hauteur 1, débit 0, mélange nul), les
    /// traitements et les bandes en trop sont ignorés.
    pub fn bounded(&self) -> Self {
        Self {
            voice: self.voice.clone(),
            pitch: bound(self.pitch, PITCH, 1.0),
            rate: bound(self.rate, RATE, 0.0),
            effects: self.effects.iter().take(MAX_EFFECTS).map(Effect::bounded).collect(),
        }
    }
}

/// Un traitement du son.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Une réverbération : le son d'une pièce. `mix` en pour cent ([`MIX`]).
    Reverb { room: Room, mix: f32 },
    /// Un délai : le son répété après `seconds` ([`DELAY_SECONDS`]), réinjecté à `feedback`
    /// pour cent ([`FEEDBACK`]), ses aigus coupés au-dessus de `cutoff` hertz ([`CUTOFF`]),
    /// mélangé à `mix` pour cent.
    Delay { seconds: f32, feedback: f32, cutoff: f32, mix: f32 },
    /// Une distorsion d'une des sortes livrées avec le système, après un gain de `pre_gain`
    /// décibels ([`PRE_GAIN`]), mélangée à `mix` pour cent.
    Distortion { kind: DistortionKind, pre_gain: f32, mix: f32 },
    /// Un égaliseur : des bandes de filtres, puis un gain d'ensemble en décibels ([`GAIN`]).
    Equalizer { bands: Vec<Band>, gain: f32 },
}

impl Effect {
    fn bounded(&self) -> Self {
        match self {
            Self::Reverb { room, mix } => Self::Reverb { room: *room, mix: bound(*mix, MIX, 0.0) },
            Self::Delay { seconds, feedback, cutoff, mix } => Self::Delay {
                seconds: bound(*seconds, DELAY_SECONDS, 0.0),
                feedback: bound(*feedback, FEEDBACK, 0.0),
                cutoff: bound(*cutoff, CUTOFF, *CUTOFF.end()),
                mix: bound(*mix, MIX, 0.0),
            },
            Self::Distortion { kind, pre_gain, mix } => Self::Distortion {
                kind: *kind,
                pre_gain: bound(*pre_gain, PRE_GAIN, -6.0),
                mix: bound(*mix, MIX, 0.0),
            },
            Self::Equalizer { bands, gain } => Self::Equalizer {
                bands: bands.iter().take(MAX_BANDS).map(Band::bounded).collect(),
                gain: bound(*gain, GAIN, 0.0),
            },
        }
    }
}

/// Les pièces que simule la réverbération du système (`AVAudioUnitReverbPreset`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    SmallRoom,
    MediumRoom,
    LargeRoom,
    MediumHall,
    LargeHall,
    Plate,
    MediumChamber,
    LargeChamber,
    Cathedral,
    LargeRoom2,
    MediumHall2,
    MediumHall3,
    LargeHall2,
}

/// Les distorsions livrées avec le système (`AVAudioUnitDistortionPreset`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistortionKind {
    DrumsBitBrush,
    DrumsBufferBeats,
    DrumsLoFi,
    MultiBrokenSpeaker,
    MultiCellphoneConcert,
    MultiDecimated1,
    MultiDecimated2,
    MultiDecimated3,
    MultiDecimated4,
    MultiDistortedFunk,
    MultiDistortedCubed,
    MultiDistortedSquared,
    MultiEcho1,
    MultiEcho2,
    MultiEchoTight1,
    MultiEchoTight2,
    MultiEverythingIsBroken,
    SpeechAlienChatter,
    SpeechCosmicInterference,
    SpeechGoldenPi,
    SpeechRadioTower,
    SpeechWaves,
}

/// Une bande d'égaliseur : un filtre à `frequency` hertz ([`FREQUENCY`]), large de `bandwidth`
/// octaves ([`BANDWIDTH`]), de gain `gain` décibels ([`GAIN`]) pour ceux qui en ont un.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub filter: Filter,
    pub frequency: f32,
    pub bandwidth: f32,
    pub gain: f32,
}

impl Band {
    fn bounded(&self) -> Self {
        Self {
            filter: self.filter,
            frequency: bound(self.frequency, FREQUENCY, 1_000.0),
            bandwidth: bound(self.bandwidth, BANDWIDTH, 1.0),
            gain: bound(self.gain, GAIN, 0.0),
        }
    }
}

/// Les filtres d'une bande d'égaliseur (`AVAudioUnitEQFilterType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Parametric,
    LowPass,
    HighPass,
    ResonantLowPass,
    ResonantHighPass,
    BandPass,
    BandStop,
    LowShelf,
    HighShelf,
    ResonantLowShelf,
    ResonantHighShelf,
}

/// `value` ramenée dans `range`, ou `neutral` si ce n'est pas un nombre.
fn bound(value: f32, range: RangeInclusive<f32>, neutral: f32) -> f32 {
    if value.is_finite() { value.clamp(*range.start(), *range.end()) } else { neutral }
}

/// Un échantillon tel que le rend un système, ramené à un flottant entre -1 et 1.
pub trait Sample: Copy {
    fn to_f32(self) -> f32;
}

impl Sample for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl Sample for i16 {
    fn to_f32(self) -> f32 {
        f32::from(self) / 32_768.0
    }
}

impl Sample for i32 {
    fn to_f32(self) -> f32 {
        (f64::from(self) / 2_147_483_648.0) as f32
    }
}

/// Des échantillons remis en canaux séparés de flottants, d'au plus deux canaux.
///
/// `slices` : une tranche par canal (`interleaved` faux), ou une seule tranche où les `channels`
/// canaux se suivent trame après trame (`interleaved` vrai). Au-delà de deux canaux, le son est
/// ramené à un seul (leur moyenne) : les unités d'effet du système n'en prennent pas plus. Ce qui
/// n'est pas un nombre devient un silence, qui ne peut pas empoisonner une réverbération.
///
/// `None` si les tranches ne contiennent pas `frames` trames de `channels` canaux.
pub fn planar<T: Sample>(
    slices: &[&[T]],
    channels: usize,
    frames: usize,
    interleaved: bool,
) -> Option<Vec<Vec<f32>>> {
    if channels == 0 {
        return None;
    }
    let read = |channel: usize, frame: usize| -> f32 {
        let sample = if interleaved { slices[0][frame * channels + channel] } else { slices[channel][frame] };
        let value = sample.to_f32();
        if value.is_finite() { value } else { 0.0 }
    };
    let enough = if interleaved {
        slices.len() == 1 && frames.checked_mul(channels).is_some_and(|needed| slices[0].len() >= needed)
    } else {
        slices.len() == channels && slices.iter().all(|slice| slice.len() >= frames)
    };
    if !enough {
        return None;
    }
    if channels <= 2 {
        return Some(
            (0..channels).map(|channel| (0..frames).map(|frame| read(channel, frame)).collect()).collect(),
        );
    }
    let mono = (0..frames)
        .map(|frame| (0..channels).map(|channel| read(channel, frame)).sum::<f32>() / channels as f32)
        .collect();
    Some(vec![mono])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_effect() -> Vec<Effect> {
        vec![
            Effect::Reverb { room: Room::SmallRoom, mix: 12.0 },
            Effect::Delay { seconds: 0.02, feedback: 40.0, cutoff: 6_000.0, mix: 30.0 },
            Effect::Distortion { kind: DistortionKind::MultiDecimated1, pre_gain: -6.0, mix: 10.0 },
            Effect::Equalizer {
                bands: vec![Band { filter: Filter::HighPass, frequency: 300.0, bandwidth: 1.0, gain: 0.0 }],
                gain: -2.0,
            },
        ]
    }

    #[test]
    fn a_plain_delivery_asks_nothing_of_the_system() {
        let plain = Delivery::plain(Some("une.voix".to_owned()));
        assert!(plain.natural_prosody() && plain.effects.is_empty());
        assert_eq!(plain.bounded(), plain);
        assert!(!Delivery { pitch: 1.05, ..plain.clone() }.natural_prosody());
        assert!(!Delivery { rate: -0.1, ..plain }.natural_prosody());
    }

    #[test]
    fn values_within_their_range_are_kept_as_they_are() {
        let delivery = Delivery { voice: None, pitch: 0.8, rate: 0.09, effects: every_effect() };
        assert_eq!(delivery.bounded(), delivery);
    }

    #[test]
    fn values_out_of_range_are_brought_back_and_non_numbers_made_neutral() {
        let wild = Delivery {
            voice: None,
            pitch: 7.0,
            rate: f32::NAN,
            effects: vec![
                Effect::Reverb { room: Room::Cathedral, mix: 250.0 },
                Effect::Delay { seconds: -1.0, feedback: f32::INFINITY, cutoff: 1.0e9, mix: -5.0 },
                Effect::Distortion { kind: DistortionKind::SpeechWaves, pre_gain: f32::NAN, mix: 101.0 },
                Effect::Equalizer {
                    bands: vec![Band {
                        filter: Filter::BandPass,
                        frequency: 5.0,
                        bandwidth: 9.0,
                        gain: -200.0,
                    }],
                    gain: 40.0,
                },
            ],
        };
        assert_eq!(
            wild.bounded(),
            Delivery {
                voice: None,
                pitch: 2.0,
                rate: 0.0,
                effects: vec![
                    Effect::Reverb { room: Room::Cathedral, mix: 100.0 },
                    Effect::Delay { seconds: 0.0, feedback: 0.0, cutoff: 20_000.0, mix: 0.0 },
                    Effect::Distortion { kind: DistortionKind::SpeechWaves, pre_gain: -6.0, mix: 100.0 },
                    Effect::Equalizer {
                        bands: vec![Band {
                            filter: Filter::BandPass,
                            frequency: 20.0,
                            bandwidth: 5.0,
                            gain: -96.0
                        }],
                        gain: 24.0,
                    },
                ],
            }
        );
        let low = Delivery { voice: None, pitch: 0.1, rate: -3.0, effects: Vec::new() }.bounded();
        assert_eq!((low.pitch, low.rate), (0.5, -1.0));
    }

    #[test]
    fn too_many_effects_or_bands_are_left_out() {
        let band = Band { filter: Filter::Parametric, frequency: 1_000.0, bandwidth: 1.0, gain: 1.0 };
        let crowded = Delivery {
            voice: None,
            pitch: 1.0,
            rate: 0.0,
            effects: vec![Effect::Equalizer { bands: vec![band; 20], gain: 0.0 }; 20],
        };
        let bounded = crowded.bounded();
        assert_eq!(bounded.effects.len(), MAX_EFFECTS);
        assert!(
            bounded
                .effects
                .iter()
                .all(|effect| matches!(effect, Effect::Equalizer { bands, .. } if bands.len() == MAX_BANDS))
        );
    }

    #[test]
    fn separate_channels_are_read_as_they_are() {
        let left = [0.5f32, -0.5, 0.25];
        let right = [0.1f32, 0.2, 0.3];
        assert_eq!(planar(&[&left, &right], 2, 3, false), Some(vec![left.to_vec(), right.to_vec()]));
        // une tranche plus longue que le nombre de trames : le reste est ignoré
        assert_eq!(planar(&[&left[..]], 1, 2, false), Some(vec![vec![0.5, -0.5]]));
        assert_eq!(planar::<f32>(&[&[]], 1, 0, false), Some(vec![Vec::new()]));
    }

    #[test]
    fn interleaved_and_integer_samples_become_separate_floats() {
        let interleaved = [i16::MIN, 0, 16_384, -16_384, i16::MAX, 8_192];
        let planar = planar(&[&interleaved[..]], 2, 3, true).expect("cohérent");
        assert_eq!(planar[0], [-1.0, 0.5, i16::MAX as f32 / 32_768.0]);
        assert_eq!(planar[1], [0.0, -0.5, 0.25]);
        let wide = [i32::MIN, 0];
        assert_eq!(super::planar(&[&wide[..]], 1, 2, true), Some(vec![vec![-1.0, 0.0]]));
    }

    #[test]
    fn more_than_two_channels_are_brought_down_to_one_and_non_numbers_to_silence() {
        let samples = [0.3f32, 0.6, 0.9, f32::NAN, 0.0, 0.3];
        let mono = planar(&[&samples[..]], 3, 2, true).expect("cohérent");
        assert_eq!(mono.len(), 1);
        assert!((mono[0][0] - 0.6).abs() < 1e-6 && (mono[0][1] - 0.1).abs() < 1e-6, "{mono:?}");
        assert_eq!(planar(&[&[f32::INFINITY][..]], 1, 1, false), Some(vec![vec![0.0]]));
    }

    #[test]
    fn inconsistent_samples_are_refused() {
        let short = [0.0f32; 3];
        assert_eq!(planar(&[&short[..]], 2, 2, true), None, "il en faut quatre");
        assert_eq!(planar(&[&short[..]], 2, 2, false), None, "une tranche par canal");
        assert_eq!(planar(&[&short[..], &short[..2]], 2, 3, false), None, "un canal trop court");
        assert_eq!(planar(&[&short[..]], 0, 1, false), None, "aucun canal");
        assert_eq!(planar(&[&short[..], &short[..]], 1, 1, true), None, "entrelacé : une seule tranche");
        assert_eq!(planar(&[&short[..]], 2, usize::MAX, true), None, "taille impossible");
    }
}
