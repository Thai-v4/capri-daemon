//! Quand regarder s'il existe une version plus récente : une première fois un peu après son
//! lancement, puis une fois par jour ; après un échec, plus tard, de plus en plus tard, sans jamais
//! boucler vite. L'horloge est passée en paramètre (le temps depuis le lancement de la veille).
//!
//! Le délai de grâce ne retarde rien : elle est déjà à l'écran quand il commence, et c'est un
//! thread à part qui attend. Il laisse son lancement (ses images, son cerveau, Ollama) se faire
//! avant qu'elle aille sur le réseau.

use std::time::Duration;

/// Avant la première vérification, après son lancement.
pub const GRACE: Duration = Duration::from_secs(60);
/// Entre deux vérifications réussies : une par jour.
pub const EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// Après un premier échec ; chaque échec suivant double l'attente, jusqu'à [`EVERY`].
pub const RETRY_FIRST: Duration = Duration::from_secs(15 * 60);

/// L'emploi du temps des vérifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    /// Le moment de la prochaine, depuis le lancement de la veille.
    next: Duration,
    /// Les échecs d'affilée.
    failures: u32,
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    /// La première vérification après [`GRACE`].
    pub fn new() -> Self {
        Self::with_grace(GRACE)
    }

    /// La première vérification après `grace` (une répétition, en version de débogage, n'attend
    /// pas une minute) ; la suite ne change pas.
    pub fn with_grace(grace: Duration) -> Self {
        Self { next: grace, failures: 0 }
    }

    /// Combien attendre encore, `now` après le lancement de la veille : rien si c'est l'heure.
    pub fn wait(&self, now: Duration) -> Duration {
        self.next.saturating_sub(now)
    }

    /// Une vérification vient de finir, `now` après le lancement de la veille, réussie ou non.
    /// Rend l'attente jusqu'à la suivante : [`EVERY`] après une réussite ; après un échec,
    /// [`RETRY_FIRST`], puis le double à chaque échec d'affilée, jamais plus de [`EVERY`].
    pub fn checked(&mut self, now: Duration, succeeded: bool) -> Duration {
        let delay = if succeeded {
            self.failures = 0;
            EVERY
        } else {
            self.failures = self.failures.saturating_add(1);
            let doublings = (self.failures - 1).min(16);
            RETRY_FIRST.saturating_mul(1 << doublings).min(EVERY)
        };
        self.next = now + delay;
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);
    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn the_first_check_waits_for_her_launch_then_one_a_day() {
        let mut schedule = Schedule::new();
        assert_eq!(schedule.wait(Duration::ZERO), GRACE);
        assert_eq!(schedule.wait(GRACE - Duration::from_secs(1)), Duration::from_secs(1));
        assert_eq!(schedule.wait(GRACE), Duration::ZERO);
        assert_eq!(schedule.wait(GRACE * 3), Duration::ZERO, "en retard : tout de suite, une fois");
        assert_eq!(schedule.checked(GRACE, true), EVERY);
        assert_eq!(schedule.wait(GRACE), EVERY);
        assert_eq!(schedule.wait(GRACE + EVERY), Duration::ZERO);
        assert_eq!(Schedule::with_grace(Duration::from_secs(3)).wait(Duration::ZERO), Duration::from_secs(3));
    }

    #[test]
    fn after_a_failure_she_tries_later_and_later_never_quickly() {
        let mut schedule = Schedule::new();
        let delays: Vec<Duration> = (0..10).map(|_| schedule.checked(GRACE, false)).collect();
        assert_eq!(
            delays,
            [15 * MINUTE, 30 * MINUTE, HOUR, 2 * HOUR, 4 * HOUR, 8 * HOUR, 16 * HOUR, EVERY, EVERY, EVERY]
        );
        // une réussite remet le compte à zéro
        assert_eq!(schedule.checked(GRACE, true), EVERY);
        assert_eq!(schedule.checked(GRACE, false), RETRY_FIRST);
        // des milliers d'échecs : jamais moins de quinze minutes, jamais plus d'un jour
        let mut schedule = Schedule::new();
        for _ in 0..5_000 {
            let delay = schedule.checked(GRACE, false);
            assert!((RETRY_FIRST..=EVERY).contains(&delay), "{delay:?}");
        }
    }

    #[test]
    fn a_week_without_network_costs_a_handful_of_attempts() {
        let (mut schedule, mut now, mut attempts) = (Schedule::new(), Duration::ZERO, 0);
        while now < 7 * EVERY {
            now += schedule.wait(now);
            attempts += 1;
            schedule.checked(now, false);
        }
        assert!(attempts <= 15, "{attempts} tentatives en une semaine");
        // et en marche normale, une par jour
        let (mut schedule, mut now, mut checks) = (Schedule::new(), Duration::ZERO, 0);
        while now < 7 * EVERY {
            now += schedule.wait(now);
            checks += 1;
            schedule.checked(now, true);
        }
        assert_eq!(checks, 8, "la première, puis une par jour");
    }
}
