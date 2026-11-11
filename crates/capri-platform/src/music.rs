//! Un son sort-il en ce moment d'une autre app que Capri ? **Un oui ou un non, rien d'autre** :
//! jamais le titre, l'artiste, l'album ni le nom de l'app, qui ne sont même pas lus. Aucune
//! permission demandée, aucune API privée, rien sur le réseau, rien de gardé ni d'écrit dans le
//! journal.
//!
//! - macOS : CoreAudio dit, sans permission, quels de ses clients ont un flux de sortie en marche
//!   (`kAudioProcessPropertyIsRunningOutput`, macOS 14.2 et après) : voir `macos/music.rs` ;
//! - ailleurs : [`MusicError::Unavailable`], elle ne sait pas.
//!
//! **Ce que « joue » veut dire.** Une app autre que Capri a un flux de sortie audio en marche, et
//! pas de flux d'entrée (le micro) en même temps. Comptent donc : un lecteur de musique, une vidéo
//! dans le navigateur, un jeu. Ne comptent pas : Capri elle-même, une app d'appel qui a le micro
//! ouvert. Ce que ça rate ou compte à tort : une app qui garde son flux ouvert sans rien jouer (en
//! pause, ou en silence) compte comme si elle jouait ; le volume coupé aussi ; un son du système
//! compte le temps qu'il dure. La décision de `capri_core::music` laisse passer les sons courts et
//! les pauses entre deux morceaux.

#[cfg(target_os = "macos")]
#[path = "macos/music.rs"]
mod imp;

#[cfg(not(target_os = "macos"))]
mod imp {
    //! Ici, elle ne sait pas si un son joue.

    use super::{Heard, MusicError};

    #[derive(Default)]
    pub struct Watch;

    impl Watch {
        pub fn listen(&mut self, _thoroughly: bool) -> Result<Heard, MusicError> {
            Err(MusicError::Unavailable)
        }

        #[cfg(debug_assertions)]
        pub fn count(&mut self) -> Result<super::Playing, MusicError> {
            Err(MusicError::Unavailable)
        }
    }
}

/// Pour les essais : combien d'autres apps jouent, et si Capri elle-même sort du son. Deux
/// nombres, rien d'autre.
#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Playing {
    pub others: usize,
    pub ours: bool,
}

/// Ce système sait-il dire si un son joue ?
pub const AVAILABLE: bool = cfg!(target_os = "macos");

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MusicError {
    #[error("elle ne sait pas, sur ce système, si un son joue")]
    Unavailable,
    #[error("le système n'a pas dit si un son joue")]
    Refused,
}

/// Ce qu'une écoute a trouvé : un oui ou un non, et des nombres pour la répétition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Heard {
    /// Une autre app que Capri sort du son, sans micro ouvert.
    pub playing: bool,
    /// Combien d'appareils audio sont en marche quelque part (sortie ou entrée).
    pub devices_running: usize,
    /// Combien de clients du son le système connaît.
    pub clients: usize,
    /// Combien de clients ont été interrogés un par un pour cette écoute.
    pub asked: usize,
}

/// Écoute si un son joue. Se garde d'une écoute à l'autre le numéro (opaque) du dernier client
/// trouvé en train de jouer, pour le réinterroger d'abord : rien d'autre.
#[derive(Default)]
pub struct SoundWatch {
    inner: imp::Watch,
}

impl SoundWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un son joue-t-il en ce moment ? Sur macOS, environ 0,05 ms quand aucun appareil audio
    /// n'est en marche, quelques millisecondes quand il faut interroger tous les clients du son
    /// (mesuré par `capri music rehearse`). La première écoute d'un processus est plus lente (le
    /// système prépare sa connexion au son) : ne pas la faire au milieu d'une animation.
    pub fn listen(&mut self) -> Result<Heard, MusicError> {
        self.inner.listen(false)
    }

    /// En version de débogage, pour mesurer le pire cas : interroge tous les clients, même quand
    /// aucun appareil n'est en marche.
    #[cfg(debug_assertions)]
    pub fn listen_thoroughly(&mut self) -> Result<Heard, MusicError> {
        self.inner.listen(true)
    }

    /// En version de débogage, pour savoir si sa propre voix compterait : combien d'autres
    /// clients jouent, et si ce processus sort du son.
    #[cfg(debug_assertions)]
    pub fn count(&mut self) -> Result<Playing, MusicError> {
        self.inner.count()
    }
}
