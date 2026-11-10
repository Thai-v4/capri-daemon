//! Son oreille : écouter le micro et transcrire ce qui est dit, sur l'appareil.
//!
//! macOS : le micro par `AVAudioEngine`, la transcription par la reconnaissance vocale du système
//! (`SFSpeechRecognizer`), **exigée sur l'appareil** : le son ne quitte jamais la machine, et rien
//! n'est enregistré. Le micro n'est ouvert qu'entre [`Ear::start`] et [`Ear::stop`]. Ailleurs,
//! Capri n'a pas encore d'oreille : [`Ear::new`] le dit en rendant `None`.
//!
//! Le système n'accorde le micro et la reconnaissance vocale qu'à une app qui dit pourquoi elle
//! les veut (dans son `Info.plist`). Lancée hors de son `.app`, Capri ne les demande donc pas :
//! [`EarError::NeedsApp`].

#[cfg(target_os = "macos")]
#[path = "macos/ear.rs"]
mod imp;

#[cfg(not(target_os = "macos"))]
mod imp {
    //! Pas encore d'oreille sur ce système : rien ne peut être créé, donc rien n'est écouté.

    use super::{EarError, EarEvent, EarStatus};

    pub struct Ear(());

    impl Ear {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn status() -> Option<EarStatus> {
            None
        }

        pub fn start(&mut self) -> Result<(), EarError> {
            Err(EarError::Unavailable)
        }

        pub fn stop(&mut self) {}

        pub fn cancel(&mut self) {}

        pub fn drain(&mut self) -> Vec<EarEvent> {
            Vec::new()
        }
    }
}

/// Ce que l'écoute rapporte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EarEvent {
    /// Tout ce qui a été compris depuis le début de la prise de parole. `last` : c'est le texte
    /// définitif, l'écoute est terminée.
    Heard { text: String, last: bool },
    /// L'écoute s'est terminée sans texte définitif : rien d'intelligible, ou le micro s'est tu.
    Failed,
    /// L'utilisateur vient de répondre à une demande de permission du système.
    PermissionAnswered,
}

/// Une permission que le système accorde ou non.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Microphone,
    SpeechRecognition,
}

/// Où en est une permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    NotAsked,
    Denied,
    Granted,
}

/// Ce qu'il faut savoir avant d'écouter, sans rien demander au système.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EarStatus {
    /// Capri tourne-t-elle dans son `.app`, qui dit au système pourquoi elle veut le micro ?
    pub in_app: bool,
    pub microphone: Access,
    pub speech_recognition: Access,
    /// Le système sait-il transcrire le français sans rien envoyer hors de la machine ?
    pub on_device: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EarError {
    #[error("Capri n'est pas lancée depuis son app : le système ne lui accorderait pas le micro")]
    NeedsApp,
    #[error("le système demande la permission à l'utilisateur")]
    Asking,
    #[error("permission refusée : {0:?}")]
    Denied(Permission),
    #[error("la reconnaissance vocale en français n'est pas disponible sur cet appareil")]
    Unavailable,
    #[error("aucun micro utilisable")]
    NoMicrophone,
}

/// De quoi écouter. À créer et à utiliser sur le thread de la boucle d'événements : c'est elle
/// qui fait arriver les transcriptions.
pub struct Ear(imp::Ear);

impl Ear {
    /// `None` sur les systèmes où Capri n'a pas encore d'oreille.
    pub fn new() -> Option<Self> {
        imp::Ear::new().map(Self)
    }

    /// Où en sont les permissions et la reconnaissance sur l'appareil. Ne demande rien à
    /// l'utilisateur. `None` sur les systèmes sans oreille.
    pub fn status() -> Option<EarStatus> {
        imp::Ear::status()
    }

    /// Ouvre le micro et commence à transcrire. Les textes arrivent par [`Ear::drain`].
    ///
    /// La première fois, le système demande les permissions : l'appel rend
    /// [`EarError::Asking`], et un [`EarEvent::PermissionAnswered`] suit la réponse de
    /// l'utilisateur. Une écoute déjà en cours est abandonnée.
    pub fn start(&mut self) -> Result<(), EarError> {
        self.0.start()
    }

    /// Ferme le micro : la prise de parole est finie. Le texte définitif arrive peu après par
    /// [`Ear::drain`] (ou un [`EarEvent::Failed`] si rien n'a été compris).
    pub fn stop(&mut self) {
        self.0.stop();
    }

    /// Ferme le micro et abandonne la transcription : plus rien n'arrivera de cette écoute.
    pub fn cancel(&mut self) {
        self.0.cancel();
    }

    /// Ce qui est arrivé depuis le dernier appel, dans l'ordre.
    pub fn drain(&mut self) -> Vec<EarEvent> {
        self.0.drain()
    }
}
