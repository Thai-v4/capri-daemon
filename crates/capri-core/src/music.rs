//! De la musique joue-t-elle ? La décision, d'après l'état que les lecteurs connus ont annoncé
//! (`capri_platform::music` les guette), et une horloge passée en paramètre : rien ne lit l'heure
//! ici, rien ne sait de quel lecteur il s'agit.
//!
//! **Ce qu'une annonce dit.** Un lecteur annonce son état à chaque changement (lecture, pause,
//! arrêt, morceau suivant) ; Capri n'en garde que la valeur de [`STATE_KEY`], lue par
//! [`PlayerState::announced`]. Toute valeur inconnue, ou absente, vaut « ne joue pas ».
//!
//! **La décision** ([`playing`]) : de la musique joue dès qu'un lecteur a annoncé `Playing`, et ne
//! joue plus dès qu'il a annoncé autre chose. Aucun délai : une annonce n'est pas un son d'une
//! seconde qu'il faudrait laisser passer. Un changement de morceau annonce `Playing` de nouveau,
//! sans pause entre les deux. Un aller-retour bref (pause puis lecture) ne la fait pas clignoter :
//! `look` ne prend et ne quitte la musique qu'au début d'un tour de sa boucle (2,5 s), il ne voit
//! que l'état de ce moment-là.
//!
//! **Le lecteur disparu.** Un lecteur qui plante ou qu'on quitte peut ne jamais annoncer qu'il
//! s'arrête : tant qu'un lecteur est cru en lecture, [`Alive`] dit quand vérifier qu'il tourne
//! encore (toutes les [`ALIVE_EVERY`]).

use std::time::{Duration, Instant};

/// La seule clé lue dans une annonce : l'état du lecteur.
pub const STATE_KEY: &str = "Player State";

/// Tant qu'un lecteur est cru en lecture, vérifier qu'il tourne encore à cet intervalle. Assez
/// court pour qu'un lecteur quitté sans annonce ne la fasse pas se balancer longtemps, assez long
/// pour que la demande au système (quelques millisecondes au plus) ne pèse rien.
pub const ALIVE_EVERY: Duration = Duration::from_secs(5);

/// Ce qu'un lecteur a annoncé de lui-même, et rien d'autre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerState {
    Playing,
    Paused,
    Stopped,
    /// Pas d'annonce depuis qu'on le guette, une valeur inconnue, ou un lecteur disparu.
    #[default]
    Unknown,
}

impl PlayerState {
    /// L'état d'après la valeur de [`STATE_KEY`] dans l'annonce (`None` : clé absente, ou pas une
    /// chaîne). Seules les trois valeurs exactes comptent.
    pub fn announced(value: Option<&str>) -> Self {
        match value {
            Some("Playing") => Self::Playing,
            Some("Paused") => Self::Paused,
            Some("Stopped") => Self::Stopped,
            _ => Self::Unknown,
        }
    }

    /// Ce lecteur joue-t-il ?
    pub fn plays(self) -> bool {
        self == Self::Playing
    }
}

/// De la musique joue-t-elle, d'après l'état connu de chaque lecteur ?
pub fn playing(states: impl IntoIterator<Item = PlayerState>) -> bool {
    states.into_iter().any(PlayerState::plays)
}

/// Quand vérifier que les lecteurs crus en lecture tournent encore.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Alive {
    next: Option<Instant>,
}

impl Alive {
    pub fn new() -> Self {
        Self::default()
    }

    /// À l'instant `now`, `playing` : un lecteur est-il cru en lecture ? Rend `true` quand il faut
    /// vérifier maintenant. La première vérification vient [`ALIVE_EVERY`] après qu'un lecteur a
    /// été cru en lecture (il vient de l'annoncer : il tournait) ; sans lecture, rien.
    pub fn due(&mut self, now: Instant, playing: bool) -> bool {
        if !playing {
            self.next = None;
            return false;
        }
        match self.next {
            Some(next) if now >= next => {
                self.next = Some(now + ALIVE_EVERY);
                true
            }
            Some(_) => false,
            None => {
                self.next = Some(now + ALIVE_EVERY);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_three_exact_values_count() {
        assert_eq!(PlayerState::announced(Some("Playing")), PlayerState::Playing);
        assert_eq!(PlayerState::announced(Some("Paused")), PlayerState::Paused);
        assert_eq!(PlayerState::announced(Some("Stopped")), PlayerState::Stopped);
        for strange in ["playing", "PLAYING", " Playing", "Playing ", "Rewinding", "", "Lecture"] {
            assert_eq!(PlayerState::announced(Some(strange)), PlayerState::Unknown, "{strange:?}");
        }
        assert_eq!(PlayerState::announced(None), PlayerState::Unknown, "clé absente");
        assert_eq!(PlayerState::default(), PlayerState::Unknown, "pas d'annonce : elle ne sait pas");
    }

    #[test]
    fn music_plays_as_soon_as_one_player_plays() {
        use PlayerState::*;
        assert!(!playing([]));
        assert!(!playing([Unknown, Unknown]));
        assert!(playing([Playing, Unknown]));
        assert!(playing([Paused, Playing]));
        assert!(!playing([Paused, Stopped]));
        // un changement de morceau : `Playing` de nouveau, sans pause entre les deux
        let mut state = Playing;
        assert!(playing([state]));
        state = PlayerState::announced(Some("Playing"));
        assert!(playing([state]));
        // la pause l'arrête tout de suite, une valeur inconnue aussi
        assert!(!playing([PlayerState::announced(Some("Paused"))]));
        assert!(!playing([PlayerState::announced(Some("Buffering"))]));
    }

    #[test]
    fn alive_checks_only_while_playing_every_few_seconds() {
        let (mut alive, t0) = (Alive::new(), Instant::now());
        // sans lecture : jamais
        assert!(!alive.due(t0, false));
        assert!(!alive.due(t0 + ALIVE_EVERY * 3, false));
        // lecture annoncée : pas tout de suite, il vient de l'annoncer
        let t1 = t0 + ALIVE_EVERY * 4;
        assert!(!alive.due(t1, true));
        assert!(!alive.due(t1 + ALIVE_EVERY - Duration::from_millis(1), true));
        assert!(alive.due(t1 + ALIVE_EVERY, true));
        assert!(!alive.due(t1 + ALIVE_EVERY + Duration::from_millis(16), true), "une fois seulement");
        assert!(alive.due(t1 + ALIVE_EVERY * 2, true));
        // la pause : on oublie, et une nouvelle lecture repart d'un intervalle entier
        assert!(!alive.due(t1 + ALIVE_EVERY * 2 + Duration::from_secs(1), false));
        let t2 = t1 + ALIVE_EVERY * 10;
        assert!(!alive.due(t2, true));
        assert!(alive.due(t2 + ALIVE_EVERY, true));
    }

    #[test]
    fn alive_with_time_going_backwards_does_not_panic() {
        let (mut alive, t0) = (Alive::new(), Instant::now() + Duration::from_secs(100));
        assert!(!alive.due(t0, true));
        assert!(!alive.due(t0 - Duration::from_secs(50), true));
        assert!(alive.due(t0 + ALIVE_EVERY, true));
    }
}
