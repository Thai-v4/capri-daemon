//! La cadence de la boucle : quand l'app se réveille pour lire le curseur, avancer Capri et, si
//! quelque chose a changé, la redessiner.
//!
//! **Deux temps, à ne pas confondre.** Ses dessins (flottement, « boil », écrasement…) sont des
//! poses clés tenues un nombre entier d'images à 24 i/s ([`crate::anim`]) : ils ne changent pas.
//! Sa **position**, elle, suit quelque chose qui bouge à la cadence de l'écran : le curseur quand
//! on la tient, la fenêtre sur laquelle elle est posée quand on la déplace. Là, rien de « cel » :
//! elle doit coller à ce qu'elle suit, à chaque rafraîchissement de l'écran (60 Hz sur la plupart,
//! 120 Hz sur un écran ProMotion), sinon elle saccade.
//!
//! D'où deux allures :
//! - **au repos** (elle flotte, ou posée sur une fenêtre immobile) : un pas tous les [`REST`],
//!   60 fois par seconde, comme avant ; on ne redessine que quand sa pose change ;
//! - **pressée** (on la tient, elle vole, ou sa fenêtre bouge) : un pas à chaque rafraîchissement
//!   de l'écran. Sur macOS, c'est l'écran qui réveille la boucle (un `CADisplayLink`) ; ailleurs,
//!   un minuteur réglé sur sa fréquence ([`Clock`]).

use std::time::{Duration, Instant};

/// Un pas au repos : 60 par seconde (la lecture du curseur, comme avant ce réglage).
pub const REST: Duration = Duration::from_micros(16_667);
/// La fréquence supposée quand le système ne dit pas celle de l'écran.
pub const DEFAULT_HZ: f64 = 60.0;
/// Les fréquences d'écran crues : en dehors, le système se trompe, et on suppose [`DEFAULT_HZ`].
pub const MIN_HZ: f64 = 24.0;
pub const MAX_HZ: f64 = 240.0;

/// La durée d'un rafraîchissement de l'écran, d'après la fréquence que dit le système (en Hz).
pub fn refresh_period(hz: Option<f64>) -> Duration {
    let hz = hz.filter(|hz| (MIN_HZ..=MAX_HZ).contains(hz)).unwrap_or(DEFAULT_HZ);
    Duration::from_secs_f64(1.0 / hz)
}

/// Le réveil suivant quand c'est un minuteur qui fait les pas pressés : un rafraîchissement après
/// le précédent réveil prévu, sans dériver (le temps passé dans un pas ne s'ajoute pas à la
/// période), et sans jamais repousser un réveil qu'un autre événement a devancé.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clock {
    deadline: Option<Instant>,
}

impl Clock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Quand se réveiller, à l'instant `now`, pour un pas tous les `period`.
    pub fn next(&mut self, now: Instant, period: Duration) -> Instant {
        let next = match self.deadline {
            // réveillée plus tôt par autre chose : le réveil prévu tient
            Some(deadline) if deadline > now => deadline,
            // à l'heure : la période suivante, comptée depuis le réveil prévu
            Some(deadline) if deadline + period > now => deadline + period,
            // en retard de plus d'une période (ou premier pas) : on repart d'ici
            _ => now + period,
        };
        self.deadline = Some(next);
        next
    }

    /// De nouveau au repos : le prochain pas pressé repartira de son propre instant.
    pub fn reset(&mut self) {
        self.deadline = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_refresh_rate_comes_from_the_system_within_sane_bounds() {
        assert_eq!(refresh_period(Some(120.0)), Duration::from_secs_f64(1.0 / 120.0));
        assert_eq!(refresh_period(Some(60.0)), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(refresh_period(Some(144.0)), Duration::from_secs_f64(1.0 / 144.0));
        // le système ne sait pas, ou se trompe : 60 Hz
        let sixty = Duration::from_secs_f64(1.0 / 60.0);
        for odd in [None, Some(0.0), Some(1.0), Some(f64::NAN), Some(f64::INFINITY), Some(1000.0)] {
            assert_eq!(refresh_period(odd), sixty, "{odd:?}");
        }
    }

    #[test]
    fn the_clock_does_not_drift_and_never_pushes_back_an_earlier_wake() {
        let start = Instant::now();
        let period = Duration::from_micros(8_333);
        let mut clock = Clock::new();
        let first = clock.next(start, period);
        assert_eq!(first, start + period);
        // réveillée à l'heure mais 1 ms de travail plus tard : la période ne s'allonge pas
        let second = clock.next(first + Duration::from_millis(1), period);
        assert_eq!(second, start + period * 2);
        // réveillée avant l'heure par un autre événement : le réveil prévu tient
        assert_eq!(clock.next(second - Duration::from_millis(3), period), second);
        // très en retard (la machine sortait de veille) : on repart de maintenant
        let late = second + Duration::from_millis(100);
        assert_eq!(clock.next(late, period), late + period);
        // de retour au repos puis pressée de nouveau : depuis maintenant
        clock.reset();
        let again = late + Duration::from_secs(1);
        assert_eq!(clock.next(again, period), again + period);
    }
}
