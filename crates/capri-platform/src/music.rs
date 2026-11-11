//! De la musique joue-t-elle ? Elle le sait par **les annonces des lecteurs connus, et seulement
//! par elles** : Spotify et Musique (Apple Music) annoncent leur état à tout le système, par une
//! notification distribuée, à chaque lecture, pause, arrêt ou changement de morceau. Capri s'y
//! abonne et n'en lit **qu'une clé**, l'état du lecteur (`capri_core::music::STATE_KEY`), rendu
//! sous la forme d'un [`PlayerState`]. Tout autre son (navigateur, vidéo, jeu) ne compte pas.
//!
//! **Ce que l'annonce transporte et que Capri ne lit pas.** Le titre, l'artiste, l'album et
//! d'autres champs arrivent avec elle, dans le même dictionnaire : ils ne sont jamais lus (seule
//! la clé de l'état est demandée, par `objectForKey:` ; jamais d'itération du dictionnaire,
//! jamais de `Debug` ni de `description` de l'annonce), ni gardés, ni écrits dans le journal, ni
//! affichés, ni envoyés à son cerveau. Aucune permission, aucune API privée, rien sur le réseau.
//!
//! Les lecteurs connus sont nommés ici, en constantes ([`KNOWN`]) : le nom de leur annonce et
//! leur identifiant de bundle, vérifiés sur ce Mac (dans leur binaire et leur `Info.plist`).
//!
//! **Le lecteur disparu.** Un lecteur qui plante ou qu'on quitte peut ne jamais annoncer qu'il
//! s'arrête : [`Announcements::forget_gone`] demande au système si un lecteur cru en lecture
//! tourne encore, par son identifiant de bundle (le nôtre, en constante), et oublie son état
//! sinon. Jamais une liste des apps, jamais leur nom. Quand le demander :
//! `capri_core::music::Alive`.
//!
//! **Ce qu'elle ne sait pas** : l'état d'un lecteur avant sa première annonce. Une musique déjà en
//! cours quand Capri s'abonne ne compte qu'à la prochaine annonce (pause, lecture, morceau
//! suivant).
//!
//! - macOS : `NSDistributedNotificationCenter`, voir `macos/music.rs` ;
//! - ailleurs : [`MusicError::Unavailable`], elle ne sait pas.

pub use capri_core::music::PlayerState;

#[cfg(target_os = "macos")]
#[path = "macos/music.rs"]
mod imp;

#[cfg(not(target_os = "macos"))]
mod imp {
    //! Ici, elle ne sait pas si de la musique joue : rien ne se construit.

    use super::{Player, PlayerState};

    pub enum Watch {}

    impl Watch {
        pub fn states(&self) -> std::iter::Empty<(Player, PlayerState)> {
            match *self {}
        }

        pub fn forget_gone(&self) -> usize {
            match *self {}
        }

        #[cfg(debug_assertions)]
        pub fn received(&self) -> std::iter::Empty<(Player, u32)> {
            match *self {}
        }
    }
}

/// Ce système sait-il guetter les annonces des lecteurs ?
pub const AVAILABLE: bool = cfg!(target_os = "macos");

/// Un lecteur guetté.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    Spotify,
    /// Musique, l'app d'Apple.
    Music,
    /// Le lecteur d'essai d'une répétition : il n'existe pas dans la version livrée.
    #[cfg(debug_assertions)]
    Rehearsal,
}

/// Un lecteur à guetter : qui, le nom de l'annonce qu'il poste, et l'identifiant de bundle par
/// lequel on demande au système s'il tourne encore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watched<'a> {
    pub player: Player,
    pub announcement: &'a str,
    pub bundle: &'a str,
}

/// Spotify : `CFBundleIdentifier` de `/Applications/Spotify.app/Contents/Info.plist`, et le nom
/// de l'annonce, présent dans son binaire.
pub const SPOTIFY: Watched<'static> = Watched {
    player: Player::Spotify,
    announcement: "com.spotify.client.PlaybackStateChanged",
    bundle: "com.spotify.client",
};

/// Musique : `CFBundleIdentifier` de `/System/Applications/Music.app/Contents/Info.plist`, et le
/// nom de l'annonce, présent dans son binaire.
pub const MUSIC: Watched<'static> =
    Watched { player: Player::Music, announcement: "com.apple.Music.playerInfo", bundle: "com.apple.Music" };

/// Les lecteurs connus, les seuls guettés dans la version livrée.
pub const KNOWN: [Watched<'static>; 2] = [SPOTIFY, MUSIC];

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MusicError {
    #[error("elle ne sait pas, sur ce système, si de la musique joue")]
    Unavailable,
    #[error("les annonces des lecteurs ne se guettent que depuis le thread principal")]
    NotMainThread,
}

/// Le guetteur des annonces : abonné aux lecteurs guettés tant qu'il vit, désabonné quand on le
/// lâche. Il ne garde, pour chaque lecteur, que son dernier état annoncé. Les annonces arrivent
/// sur le thread principal, par sa boucle ; il ne vit que là.
pub struct Announcements {
    inner: imp::Watch,
}

impl Announcements {
    /// S'abonne aux annonces des lecteurs connus. Sur le thread principal seulement.
    pub fn new() -> Result<Self, MusicError> {
        Self::listen(&KNOWN, true)
    }

    /// En version de débogage : un lecteur d'essai dont l'annonce et l'identifiant de bundle sont
    /// donnés (`bundle` : un identifiant qu'aucune app n'a, pour exercer l'oubli d'un lecteur
    /// disparu), et les lecteurs connus aussi si `with_known`.
    #[cfg(debug_assertions)]
    pub fn rehearsal(announcement: &str, bundle: &str, with_known: bool) -> Result<Self, MusicError> {
        let rehearsed = Watched { player: Player::Rehearsal, announcement, bundle };
        let [spotify, music] = KNOWN;
        if with_known {
            Self::listen(&[spotify, music, rehearsed], true)
        } else {
            Self::listen(&[rehearsed], true)
        }
    }

    /// En version de débogage, pour comparer : le seul lecteur d'essai, abonné comme le ferait une
    /// app ordinaire, sans demander la livraison immédiate. Dans une app qui n'est pas active,
    /// AppKit en retient les annonces : c'est le piège que [`Announcements::new`] évite.
    #[cfg(debug_assertions)]
    pub fn ordinary(announcement: &str, bundle: &str) -> Result<Self, MusicError> {
        Self::listen(&[Watched { player: Player::Rehearsal, announcement, bundle }], false)
    }

    /// `immediately` : livrées même quand AppKit suspend la livraison (l'app n'est pas active).
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
    fn listen(watched: &[Watched<'_>], immediately: bool) -> Result<Self, MusicError> {
        #[cfg(target_os = "macos")]
        return Ok(Self { inner: imp::Watch::new(watched, immediately)? });
        #[cfg(not(target_os = "macos"))]
        Err(MusicError::Unavailable)
    }

    /// Le dernier état annoncé par chaque lecteur guetté (`Unknown` sans annonce).
    pub fn states(&self) -> impl Iterator<Item = (Player, PlayerState)> + '_ {
        self.inner.states()
    }

    /// Oublie l'état des lecteurs crus en lecture qui ne tournent plus. Rend combien ont été
    /// oubliés. Ne demande au système que les lecteurs crus en lecture, par leur identifiant.
    pub fn forget_gone(&self) -> usize {
        self.inner.forget_gone()
    }

    /// En version de débogage : combien d'annonces de chaque lecteur sont arrivées (un nombre,
    /// rien de leur contenu).
    #[cfg(debug_assertions)]
    pub fn received(&self) -> impl Iterator<Item = (Player, u32)> + '_ {
        self.inner.received()
    }
}

/// En version de débogage : une app de cet identifiant de bundle tourne-t-elle, d'après la même
/// question que [`Announcements::forget_gone`] ? `None` ailleurs que sur macOS.
#[cfg(debug_assertions)]
pub fn is_running(bundle: &str) -> Option<bool> {
    #[cfg(target_os = "macos")]
    return Some(imp::is_running(bundle));
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        None
    }
}

/// En version de débogage : poste une annonce, comme le ferait un lecteur, sous le nom
/// `announcement`, avec `state` pour l'état (`None` : sans cette clé) et d'autres champs
/// (`others` : de faux titre, artiste, album). Sans livraison immédiate, comme un lecteur
/// ordinaire. Pour les répétitions, depuis un autre processus que celui qui guette.
#[cfg(debug_assertions)]
pub fn announce(announcement: &str, state: Option<&str>, others: &[(&str, &str)]) -> Result<(), MusicError> {
    #[cfg(target_os = "macos")]
    {
        imp::announce(announcement, state, others);
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (announcement, state, others);
        Err(MusicError::Unavailable)
    }
}

/// En version de débogage, hors de l'app : fait tourner la boucle du thread courant pendant
/// `duration`, le temps que les annonces arrivent (dans l'app, c'est sa boucle qui le fait).
#[cfg(debug_assertions)]
pub fn run_for(duration: std::time::Duration) {
    #[cfg(target_os = "macos")]
    imp::run_for(duration);
    #[cfg(not(target_os = "macos"))]
    std::thread::sleep(duration);
}

/// En version de débogage, dans l'app : est-elle l'app active, et la livraison des notifications
/// distribuées est-elle suspendue (AppKit la suspend quand l'app n'est pas active) ?
#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub app_active: bool,
    pub suspended: bool,
}

/// En version de débogage, pour la répétition : suspend (ou reprend) la livraison des
/// notifications distribuées de ce processus, comme AppKit le fait quand une app cesse d'être
/// active. Sans effet ailleurs que sur macOS.
#[cfg(debug_assertions)]
pub fn set_suspended(suspended: bool) {
    #[cfg(target_os = "macos")]
    imp::set_suspended(suspended);
    #[cfg(not(target_os = "macos"))]
    let _ = suspended;
}

/// Voir [`Delivery`]. `None` hors du thread principal, ou ailleurs que sur macOS.
#[cfg(debug_assertions)]
pub fn delivery() -> Option<Delivery> {
    #[cfg(target_os = "macos")]
    return imp::delivery();
    #[cfg(not(target_os = "macos"))]
    None
}
