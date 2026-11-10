//! Sa voix : la synthèse vocale du système.
//!
//! macOS : `AVSpeechSynthesizer`, avec les voix installées sur la machine. Aucun son n'est
//! enregistré ni envoyé où que ce soit : le texte est dit par le système, sur place. Ailleurs,
//! Capri n'a pas encore de voix : [`Voice::new`] le dit en rendant `None`.

use std::time::Duration;

#[cfg(target_os = "macos")]
#[path = "macos/voice.rs"]
mod imp;

#[cfg(not(target_os = "macos"))]
mod imp {
    //! Pas encore de voix sur ce système : rien ne peut être créé, donc rien n'est jamais dit.

    use std::time::Duration;

    use super::SystemVoice;

    pub struct Voice(());

    impl Voice {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn installed() -> Vec<SystemVoice> {
            Vec::new()
        }

        pub fn say(&self, _text: &str, _voice_id: Option<&str>) {}

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
}

/// Une voix installée sur le système.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemVoice {
    /// L'identifiant à donner à [`Voice::say`].
    pub id: String,
    pub name: String,
    /// La langue, sous la forme `fr-FR`.
    pub language: String,
    /// De 1 (voix compacte, livrée avec le système) à 3 (la meilleure, à télécharger).
    pub quality: u8,
    pub female: bool,
}

/// De quoi parler à voix haute. À créer et à utiliser sur un seul thread, celui de la boucle
/// d'événements : c'est elle qui fait avancer la parole.
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

    /// Dit `text`, à la suite de ce qui est déjà en train d'être dit. Ne bloque pas.
    /// `voice_id` : l'identifiant d'une voix installée ; sans lui, ou si elle a disparu, la voix
    /// française par défaut du système.
    pub fn say(&self, text: &str, voice_id: Option<&str>) {
        self.0.say(text, voice_id);
    }

    /// Se tait tout de suite, et oublie ce qui restait à dire.
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

    /// Hors de la boucle d'événements (en ligne de commande) : laisse la voix finir, pendant
    /// `limit` au plus. Rend `false` si elle parlait encore à l'échéance.
    pub fn wait(&self, limit: Duration) -> bool {
        self.0.wait(limit)
    }
}
