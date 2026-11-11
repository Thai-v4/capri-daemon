//! De la musique joue-t-elle ? La décision, d'après une suite d'écoutes (« un son sort-il d'une
//! autre app, oui ou non », que `capri_platform::music` donne), et une horloge passée en
//! paramètre : rien ne lit l'heure ici.
//!
//! Elle ne réagit pas à un son d'une seconde (une alerte, un clic) : il faut [`START`] d'écoutes
//! qui disent « oui » sans interruption. Elle n'arrête pas à la première pause entre deux
//! morceaux : il faut [`STOP`] d'écoutes qui disent « non » sans interruption. Entre deux écoutes
//! trop espacées ([`GAP`] : elle était cachée, la machine dormait), on ne sait pas ce qui s'est
//! passé : le compte repart de zéro. Quand le système ne sait pas, elle n'entend rien.

use std::time::{Duration, Instant};

/// Combien de temps attendre entre deux écoutes.
pub const LISTEN_EVERY: Duration = Duration::from_secs(1);
/// Un son qui dure moins que ça n'est pas de la musique.
pub const START: Duration = Duration::from_secs(3);
/// Un silence plus court que ça est une pause entre deux morceaux.
pub const STOP: Duration = Duration::from_secs(8);
/// Deux écoutes plus espacées que ça ne se suivent pas.
pub const GAP: Duration = Duration::from_secs(5);

/// La décision, d'une écoute à l'autre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Groove {
    on: bool,
    /// Depuis quand les écoutes disent le contraire de `on`, sans interruption.
    contrary_since: Option<Instant>,
    last: Option<Instant>,
}

impl Groove {
    pub fn new() -> Self {
        Self::default()
    }

    /// De la musique joue-t-elle, d'après les écoutes passées ?
    pub fn on(&self) -> bool {
        self.on
    }

    /// Une écoute, faite à l'instant `now` : `Some(oui ou non)`, ou `None` si le système n'a rien
    /// pu dire. Rend la décision.
    pub fn hear(&mut self, now: Instant, heard: Option<bool>) -> bool {
        let follows = self.last.is_some_and(|last| now.saturating_duration_since(last) <= GAP);
        self.last = Some(now);
        if !follows {
            self.contrary_since = None;
        }
        let Some(playing) = heard else {
            self.on = false;
            self.contrary_since = None;
            return false;
        };
        if playing == self.on {
            self.contrary_since = None;
            return self.on;
        }
        let since = *self.contrary_since.get_or_insert(now);
        let needed = if self.on { STOP } else { START };
        if now.saturating_duration_since(since) >= needed {
            self.on = playing;
            self.contrary_since = None;
        }
        self.on
    }

    /// Oublie tout (elle est cachée, l'écoute est arrêtée) : la prochaine écoute repart de zéro,
    /// sans musique.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une écoute par seconde, de `from` (en secondes) à `to` exclu, toutes avec la même réponse ;
    /// rend la dernière décision.
    fn listen(groove: &mut Groove, t0: Instant, from: u64, to: u64, playing: bool) -> bool {
        (from..to)
            .map(|s| groove.hear(t0 + Duration::from_secs(s), Some(playing)))
            .last()
            .unwrap_or(groove.on())
    }

    #[test]
    fn a_short_sound_is_not_music() {
        let (mut groove, t0) = (Groove::new(), Instant::now());
        assert!(!listen(&mut groove, t0, 0, 2, true), "deux secondes de son");
        assert!(!listen(&mut groove, t0, 2, 3, false));
        assert!(!listen(&mut groove, t0, 3, 5, true), "le compte est reparti de zéro");
        assert!(!groove.on());
    }

    #[test]
    fn three_seconds_of_sound_are_music() {
        let (mut groove, t0) = (Groove::new(), Instant::now());
        assert!(!listen(&mut groove, t0, 0, 3, true), "0, 1, 2 s : pas encore");
        assert!(groove.hear(t0 + Duration::from_secs(3), Some(true)), "3 s après la première écoute");
    }

    #[test]
    fn a_pause_between_two_songs_does_not_stop_her() {
        let (mut groove, t0) = (Groove::new(), Instant::now());
        assert!(listen(&mut groove, t0, 0, 10, true));
        assert!(listen(&mut groove, t0, 10, 17, false), "sept secondes de silence");
        assert!(listen(&mut groove, t0, 17, 18, true), "le morceau suivant");
        assert!(listen(&mut groove, t0, 18, 25, false), "le compte du silence est reparti de zéro");
    }

    #[test]
    fn a_long_silence_stops_her() {
        let (mut groove, t0) = (Groove::new(), Instant::now());
        assert!(listen(&mut groove, t0, 0, 10, true));
        assert!(listen(&mut groove, t0, 10, 18, false), "10 à 17 s : sept secondes depuis la première");
        assert!(!groove.hear(t0 + Duration::from_secs(18), Some(false)), "huit secondes de silence");
    }

    #[test]
    fn a_gap_between_listens_starts_the_count_again() {
        let (mut groove, t0) = (Groove::new(), Instant::now());
        assert!(!groove.hear(t0, Some(true)));
        // elle était cachée : rien entre 0 et 60 s
        assert!(!groove.hear(t0 + Duration::from_secs(60), Some(true)), "ne compte pas les 60 s");
        assert!(!listen(&mut groove, t0, 61, 63, true));
        assert!(groove.hear(t0 + Duration::from_secs(63), Some(true)));
        // et dans l'autre sens : un long trou ne vaut pas un silence
        assert!(groove.hear(t0 + Duration::from_secs(64), Some(false)));
        assert!(groove.hear(t0 + Duration::from_secs(200), Some(false)));
    }

    #[test]
    fn when_the_system_cannot_say_she_hears_nothing() {
        let (mut groove, t0) = (Groove::new(), Instant::now());
        assert!(listen(&mut groove, t0, 0, 5, true));
        assert!(!groove.hear(t0 + Duration::from_secs(5), None));
        assert!(!groove.hear(t0 + Duration::from_secs(6), Some(true)), "repart de zéro");
    }

    #[test]
    fn reset_forgets_everything() {
        let (mut groove, t0) = (Groove::new(), Instant::now());
        assert!(listen(&mut groove, t0, 0, 5, true));
        groove.reset();
        assert_eq!(groove, Groove::new());
    }

    #[test]
    fn time_going_backwards_does_not_panic() {
        let (mut groove, t0) = (Groove::new(), Instant::now() + Duration::from_secs(100));
        groove.hear(t0, Some(true));
        assert!(!groove.hear(t0 - Duration::from_secs(50), Some(true)));
    }

    #[test]
    fn listening_every_second_fits_the_thresholds() {
        const { assert!(LISTEN_EVERY.as_millis() * 2 < START.as_millis()) };
        const { assert!(LISTEN_EVERY.as_millis() < GAP.as_millis() && GAP.as_millis() < STOP.as_millis()) };
    }
}
