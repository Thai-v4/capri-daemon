//! macOS : la synthèse vocale d'AVFoundation.
//!
//! `AVSpeechSynthesizer` tient lui-même la file de ce qu'il a à dire : chaque phrase lui est
//! confiée dès qu'elle est prête, et il les enchaîne.

use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2_avf_audio::{
    AVSpeechBoundary, AVSpeechSynthesisVoice, AVSpeechSynthesisVoiceGender, AVSpeechSynthesizer,
    AVSpeechUtterance,
};
use objc2_foundation::{NSDate, NSRunLoop, NSString};

use super::SystemVoice;

/// La langue de la voix quand aucune n'est désignée : Capri parle français.
const DEFAULT_LANGUAGE: &str = "fr-FR";

pub struct Voice {
    /// Ni `Send` ni `Sync` : la voix reste sur le thread qui l'a créée.
    synthesizer: Retained<AVSpeechSynthesizer>,
}

impl Voice {
    pub fn new() -> Option<Self> {
        // SAFETY: crée un objet sans argument ; il n'est utilisé ensuite que depuis ce thread,
        // puisque `Retained<AVSpeechSynthesizer>` ne peut pas en changer.
        let synthesizer = unsafe { AVSpeechSynthesizer::new() };
        Some(Self { synthesizer })
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

    pub fn say(&self, text: &str, voice_id: Option<&str>) {
        // SAFETY: les chaînes passées vivent le temps de chaque appel ; un identifiant ou une
        // langue inconnus rendent simplement `nil`.
        let voice = unsafe {
            voice_id
                .and_then(|id| AVSpeechSynthesisVoice::voiceWithIdentifier(&NSString::from_str(id)))
                .or_else(|| {
                    AVSpeechSynthesisVoice::voiceWithLanguage(Some(&NSString::from_str(DEFAULT_LANGUAGE)))
                })
        };
        // SAFETY: l'énoncé est créé à partir d'une chaîne valide, reçoit une voix vivante (ou
        // aucune : le système prend alors la sienne), et le synthétiseur le retient jusqu'à
        // l'avoir dit.
        unsafe {
            let utterance = AVSpeechUtterance::speechUtteranceWithString(&NSString::from_str(text));
            utterance.setVoice(voice.as_deref());
            self.synthesizer.speakUtterance(&utterance);
        }
    }

    pub fn stop(&self) {
        // SAFETY: appel sans autre condition que d'être sur le thread du synthétiseur ; rend
        // `false` s'il n'y avait rien à arrêter.
        unsafe { self.synthesizer.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
    }

    pub fn pause(&self) {
        // SAFETY: comme `stop` ; sans effet si rien n'est en train d'être dit.
        unsafe { self.synthesizer.pauseSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
    }

    pub fn resume(&self) {
        // SAFETY: comme `stop` ; sans effet si la parole n'était pas suspendue.
        unsafe { self.synthesizer.continueSpeaking() };
    }

    pub fn is_speaking(&self) -> bool {
        // SAFETY: simple lecture d'une propriété du synthétiseur, sur son thread.
        unsafe { self.synthesizer.isSpeaking() }
    }

    pub fn wait(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let run_loop = NSRunLoop::currentRunLoop();
        loop {
            // la parole avance avec la boucle du thread : on la fait tourner par petites tranches
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
            if !self.is_speaking() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
        }
    }
}
