//! Sa voix : la synthèse vocale du système, et des traitements du son appliqués après elle.
//!
//! macOS : `AVSpeechSynthesizer`, avec les voix installées sur la machine. Sans traitement, la
//! phrase est dite directement par le système, comme avant les effets. Avec, elle est rendue en
//! mémoire puis jouée à travers une chaîne d'unités d'effet d'AVFAudio (un `AVAudioEngine`), et
//! si la chaîne pose le moindre problème, elle est dite directement, sans effet. Aucun son n'est
//! enregistré ni envoyé où que ce soit : tout se passe sur place, en mémoire. Ailleurs, Capri n'a
//! pas encore de voix : [`Voice::new`] le dit en rendant `None`.
//!
//! Comment dire une phrase est décrit par [`Delivery`] (voix, hauteur, débit, traitements),
//! composée par `capri_chat::voice` : ce module ne sait rien de Capri ni de ses préréglages.

use std::time::Duration;

pub use capri_core::sound::Delivery;

#[cfg(target_os = "macos")]
#[path = "macos/voice.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "macos/voice_stage.rs"]
mod stage;

#[cfg(not(target_os = "macos"))]
mod imp {
    //! Pas encore de voix sur ce système : rien ne peut être créé, donc rien n'est jamais dit.

    use std::time::Duration;

    use super::{Delivery, SystemVoice};

    pub struct Voice(());

    impl Voice {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn installed() -> Vec<SystemVoice> {
            Vec::new()
        }

        pub fn say(&self, _text: &str, _delivery: &Delivery) {}

        pub fn stop(&self) {}

        pub fn pause(&self) {}

        pub fn resume(&self) {}

        pub fn is_speaking(&self) -> bool {
            false
        }

        pub fn wait(&self, _limit: Duration) -> bool {
            true
        }
    }

    #[cfg(debug_assertions)]
    impl Voice {
        pub fn rehearsal(_rehearsal: super::Rehearsal) -> Option<Self> {
            None
        }

        pub fn rehearsal_log(&self) -> Vec<super::Rehearsed> {
            Vec::new()
        }

        pub fn break_effects(&self, _breakage: Option<super::Breakage>) {}

        pub fn played_seconds(&self) -> Option<f64> {
            None
        }

        pub fn render(&self, _limit: Duration) -> Result<Vec<f32>, String> {
            Err("pas de voix sur ce système".to_owned())
        }

        pub fn run_for(&self, _length: Duration) {}
    }
}

/// Une voix installée sur le système.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemVoice {
    /// L'identifiant à donner dans [`Delivery::voice`].
    pub id: String,
    pub name: String,
    /// La langue, sous la forme `fr-FR`.
    pub language: String,
    /// De 1 (voix compacte, livrée avec le système) à 3 (la meilleure, à télécharger).
    pub quality: u8,
    pub female: bool,
}

/// De quoi parler à voix haute. À créer et à utiliser sur un seul thread, celui de la boucle
/// d'événements (le thread principal) : c'est elle qui fait avancer la parole, et le système
/// n'y rend le son traité que là. Créée ailleurs, elle parle sans effets.
pub struct Voice(imp::Voice);

impl Voice {
    /// `None` sur les systèmes où Capri n'a pas encore de voix.
    pub fn new() -> Option<Self> {
        imp::Voice::new().map(Self)
    }

    /// Les voix installées, toutes langues confondues.
    pub fn installed() -> Vec<SystemVoice> {
        imp::Voice::installed()
    }

    /// Dit `text` comme le décrit `delivery`, à la suite de ce qui est déjà en train d'être dit,
    /// dans l'ordre. Ne bloque pas. Sa voix : l'identifiant d'une voix installée ; sans lui, ou si
    /// elle a disparu, la voix française par défaut du système. Ses valeurs sont ramenées dans
    /// leurs plages ([`Delivery::bounded`]).
    pub fn say(&self, text: &str, delivery: &Delivery) {
        self.0.say(text, delivery);
    }

    /// Se tait tout de suite, et oublie ce qui restait à dire. Les effets qui avaient échoué
    /// sont retentés à la phrase suivante.
    pub fn stop(&self) {
        self.0.stop();
    }

    /// Suspend la parole, sans rien oublier.
    pub fn pause(&self) {
        self.0.pause();
    }

    /// Reprend là où [`Voice::pause`] s'était arrêtée.
    pub fn resume(&self) {
        self.0.resume();
    }

    /// Reste-t-il quelque chose en train d'être dit, ou à dire ?
    pub fn is_speaking(&self) -> bool {
        self.0.is_speaking()
    }

    /// Hors de la boucle d'événements (en ligne de commande) : laisse la voix finir, la queue
    /// d'un effet comprise, pendant `limit` au plus. Rend `false` si elle parlait encore à
    /// l'échéance.
    pub fn wait(&self, limit: Duration) -> bool {
        self.0.wait(limit)
    }
}

/// Pour les essais en version de débogage : où va le son d'une voix d'essai. Ni l'une ni l'autre
/// ne fait de bruit.
#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rehearsal {
    /// Les vrais chemins jusqu'à la sortie du Mac, mais à volume nul : le direct par un énoncé de
    /// volume 0, le traité par un mélangeur principal à 0.
    Silent,
    /// Le chemin traité rendu en mémoire, sans périphérique ([`Voice::render`]) ; le direct, s'il
    /// sert, à volume nul.
    Offline,
}

/// Pour les essais : ce qu'a fait une voix d'essai, dans l'ordre.
#[cfg(debug_assertions)]
#[derive(Clone, Debug, PartialEq)]
pub enum Rehearsed {
    /// Dite par le chemin direct ; `prosody` : avec une hauteur ou un débit demandés.
    Direct {
        text: String,
        prosody: bool,
    },
    /// Rendue en mémoire, entière, en tant de trames à tant de hertz.
    Synthesized {
        text: String,
        frames: usize,
        rate: f64,
    },
    /// Jouée jusqu'au bout par la chaîne de traitements.
    PlayedBack {
        text: String,
    },
    /// Le chemin traité est abandonné, pour cette raison.
    Fallback {
        reason: String,
    },
    EngineStarted,
    EngineStopped,
    Paused,
    Resumed,
    Stopped,
}

/// Pour les essais : une panne à simuler sur le chemin traité.
#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breakage {
    /// La chaîne ne peut pas être montée.
    EngineStart,
    /// Le système ne rend rien de la phrase.
    Silence,
    /// Les paquets sont dans un format qu'on ne sait pas lire.
    ForeignFormat,
    /// Le moteur s'arrête de lui-même juste après avoir démarré, comme à un changement de sortie.
    EngineStops,
}

#[cfg(debug_assertions)]
impl Voice {
    /// Pour les essais : une voix qui ne fait aucun bruit. `None` là où il n'y a pas de voix.
    pub fn rehearsal(rehearsal: Rehearsal) -> Option<Self> {
        imp::Voice::rehearsal(rehearsal).map(Self)
    }

    /// Pour les essais : ce qu'elle a fait depuis sa création.
    pub fn rehearsal_log(&self) -> Vec<Rehearsed> {
        self.0.rehearsal_log()
    }

    /// Pour les essais : fait échouer le chemin traité de cette façon, à partir de maintenant.
    pub fn break_effects(&self, breakage: Option<Breakage>) {
        self.0.break_effects(breakage);
    }

    /// Pour les essais : le temps de son joué par la chaîne de traitements, en secondes.
    pub fn played_seconds(&self) -> Option<f64> {
        self.0.played_seconds()
    }

    /// Pour les essais, voix [`Rehearsal::Offline`] : laisse rendre en mémoire ce qu'on lui a
    /// fait dire, puis le fait traverser la chaîne, et rend la sortie (48 kHz, stéréo entrelacée).
    pub fn render(&self, limit: Duration) -> Result<Vec<f32>, String> {
        self.0.render(limit)
    }

    /// Pour les essais, hors de la boucle d'événements : fait tourner la boucle de ce thread
    /// pendant `length`, qu'elle parle ou non.
    pub fn run_for(&self, length: Duration) {
        self.0.run_for(length);
    }
}
