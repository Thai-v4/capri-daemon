//! Elle se balance sur ta musique, assise : l'abonnement aux annonces des lecteurs connus, et ce
//! qu'elle en conclut. Le guetteur est dans `capri_platform::music` (Spotify et Musique, dont
//! seul l'état est lu, sans permission), la décision dans `capri_core::music`.
//!
//! **L'abonnement existe tant que c'est permis** (« Se balancer sur la musique » dans son menu),
//! dès le lancement, qu'elle soit assise ou non : une annonce ne coûte rien, et l'état doit être
//! connu quand elle s'assoit. Décoché, elle se désabonne et oublie ce qu'elle savait. Elle ne se
//! balance qu'assise, visible, avec ses planches.
//!
//! **Ce qu'elle ne sait pas** : une musique déjà en cours quand elle s'abonne (au lancement, ou
//! quand on recoche la case). Elle ne se balance pas avant la prochaine annonce du lecteur.
//!
//! **Le lecteur disparu** : tant qu'un lecteur est cru en lecture, elle demande toutes les
//! quelques secondes (`capri_core::music::ALIVE_EVERY`) s'il tourne encore, et l'oublie sinon.
//!
//! Le journal n'en dit que « se balance » et « ne se balance plus » : jamais le lecteur, ni rien de
//! l'annonce. Windows et ailleurs : elle ne sait pas, et ne se balance pas.

use std::time::Instant;

use capri_core::music::{self, Alive};
use capri_platform::music::{Announcements, MusicError};

/// Ce qui guette les annonces : de la musique joue-t-elle d'après les états connus, et l'oubli
/// des lecteurs disparus. Le vrai guetteur, ou un faux pour les essais.
pub trait Source {
    fn playing(&self) -> bool;
    /// Oublie les lecteurs crus en lecture qui ne tournent plus ; rend combien.
    fn forget_gone(&self) -> usize;
}

impl Source for Announcements {
    fn playing(&self) -> bool {
        music::playing(self.states().map(|(_, state)| state))
    }

    fn forget_gone(&self) -> usize {
        Announcements::forget_gone(self)
    }
}

/// S'abonner : rend le guetteur, ou ce que le système refuse.
type Subscribe = Box<dyn FnMut() -> Result<Box<dyn Source>, MusicError>>;

pub struct MusicView {
    enabled: bool,
    subscribe: Subscribe,
    /// Le guetteur, tant que c'est permis.
    source: Option<Box<dyn Source>>,
    /// Elle ne sait pas guetter ici (ou le système a refusé) : elle n'essaie plus.
    unavailable: bool,
    alive: Alive,
    /// De la musique joue, au dernier pas (qu'elle soit assise ou non).
    playing: bool,
    /// Elle se balance, au dernier pas.
    swaying: bool,
}

impl MusicView {
    /// `enabled` : le réglage de son menu. Sur le thread principal (le guetteur y vit).
    pub fn new(enabled: bool) -> Self {
        Self::with(enabled, Box::new(|| Ok(Box::new(Announcements::new()?))))
    }

    /// En version de débogage, pour la répétition dans la vraie app : le seul lecteur d'essai (un
    /// vrai lecteur qui jouerait pendant ce temps ne la trouble pas).
    #[cfg(debug_assertions)]
    pub fn rehearsal(enabled: bool, announcement: String, bundle: &'static str) -> Self {
        Self::with(
            enabled,
            Box::new(move || Ok(Box::new(Announcements::rehearsal(&announcement, bundle, false)?))),
        )
    }

    fn with(enabled: bool, subscribe: Subscribe) -> Self {
        let mut view = Self {
            enabled,
            subscribe,
            source: None,
            unavailable: false,
            alive: Alive::new(),
            playing: false,
            swaying: false,
        };
        view.sync();
        view
    }

    /// Le réglage se change dans son menu, qui n'existe que sur macOS et Windows.
    #[cfg(any(target_os = "macos", windows))]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Le lui permettre (elle s'abonne), ou le lui interdire (elle se désabonne et oublie).
    #[cfg(any(target_os = "macos", windows, test))]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.sync();
    }

    /// Abonnée si et seulement si c'est permis.
    fn sync(&mut self) {
        if !self.enabled {
            if self.source.take().is_some() {
                log::debug!("musique : elle ne guette plus les annonces");
            }
            self.alive = Alive::new();
            self.playing = false;
            return;
        }
        if self.source.is_some() || self.unavailable {
            return;
        }
        match (self.subscribe)() {
            Ok(source) => {
                log::debug!("musique : elle guette les annonces des lecteurs");
                self.source = Some(source);
            }
            Err(MusicError::Unavailable) => {
                log::info!("musique : elle ne sait pas, sur ce système, si de la musique joue");
                self.unavailable = true;
            }
            Err(err) => {
                log::warn!("musique : {err}, elle ne se balancera pas");
                self.unavailable = true;
            }
        }
    }

    /// À chaque pas, à l'instant `now` : `wanted`, elle est assise (avec ses planches) et visible.
    /// Rend : se balance-t-elle ?
    pub fn tick(&mut self, now: Instant, wanted: bool) -> bool {
        self.playing = self.source.as_ref().is_some_and(|source| {
            let playing = source.playing();
            if self.alive.due(now, playing) && source.forget_gone() > 0 {
                return source.playing();
            }
            playing
        });
        let swaying = wanted && self.playing;
        if swaying != self.swaying {
            log::debug!("musique : elle {}", if swaying { "se balance" } else { "ne se balance plus" });
            self.swaying = swaying;
        }
        swaying
    }

    /// En version de débogage, pour la répétition : de la musique joue-t-elle, au dernier pas,
    /// qu'elle soit assise ou non ?
    #[cfg(debug_assertions)]
    pub fn playing(&self) -> bool {
        self.playing
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    use capri_core::music::ALIVE_EVERY;

    use super::*;

    /// Ce que le faux guetteur sait, partagé avec l'essai.
    #[derive(Default)]
    struct Fake {
        playing: Cell<bool>,
        /// Le lecteur tourne-t-il encore ?
        gone: Cell<bool>,
        subscribed: Cell<usize>,
        alive: Cell<usize>,
        checks: Cell<usize>,
    }

    struct FakeSource(Rc<Fake>);

    impl Source for FakeSource {
        fn playing(&self) -> bool {
            self.0.playing.get()
        }

        fn forget_gone(&self) -> usize {
            self.0.checks.set(self.0.checks.get() + 1);
            if self.0.gone.get() && self.0.playing.get() {
                self.0.playing.set(false);
                return 1;
            }
            0
        }
    }

    impl Drop for FakeSource {
        fn drop(&mut self) {
            self.0.alive.set(self.0.alive.get() - 1);
        }
    }

    fn view(enabled: bool) -> (MusicView, Rc<Fake>) {
        let fake = Rc::new(Fake::default());
        let shared = Rc::clone(&fake);
        let subscribe: Subscribe = Box::new(move || {
            shared.subscribed.set(shared.subscribed.get() + 1);
            shared.alive.set(shared.alive.get() + 1);
            Ok(Box::new(FakeSource(Rc::clone(&shared))))
        });
        (MusicView::with(enabled, subscribe), fake)
    }

    #[test]
    fn subscribed_from_the_start_seated_or_not_and_sways_only_when_wanted() {
        let (mut view, fake) = view(true);
        assert_eq!(fake.subscribed.get(), 1, "abonnée dès le lancement");
        let t0 = Instant::now();
        assert!(!view.tick(t0, true), "rien d'annoncé : elle ne se balance pas");
        // une annonce de lecture alors qu'elle est debout : l'état est connu, elle ne se balance pas
        fake.playing.set(true);
        assert!(!view.tick(t0, false));
        assert!(view.playing());
        // elle s'assoit : tout de suite
        assert!(view.tick(t0 + Duration::from_millis(16), true));
        // la pause : tout de suite
        fake.playing.set(false);
        assert!(!view.tick(t0 + Duration::from_millis(32), true));
        assert_eq!(fake.subscribed.get(), 1, "un seul abonnement");
    }

    #[test]
    fn unchecked_she_unsubscribes_and_forgets_rechecked_she_knows_nothing() {
        let (mut view, fake) = view(true);
        fake.playing.set(true);
        assert!(view.tick(Instant::now(), true));
        view.set_enabled(false);
        assert_eq!(fake.alive.get(), 0, "désabonnée");
        assert!(!view.tick(Instant::now(), true));
        assert!(!view.playing());
        // recochée : un nouvel abonnement, qui ne sait rien de la musique en cours
        fake.playing.set(false);
        view.set_enabled(true);
        assert_eq!((fake.subscribed.get(), fake.alive.get()), (2, 1));
        assert!(!view.tick(Instant::now(), true));
    }

    #[test]
    fn forbidden_she_never_subscribes() {
        let (mut view, fake) = view(false);
        fake.playing.set(true);
        assert!(!view.tick(Instant::now(), true));
        assert_eq!(fake.subscribed.get(), 0);
    }

    #[test]
    fn unavailable_she_tries_once_and_never_sways() {
        let tries = Rc::new(Cell::new(0));
        let seen = Rc::clone(&tries);
        let subscribe: Subscribe = Box::new(move || {
            seen.set(seen.get() + 1);
            Err(MusicError::Unavailable)
        });
        let mut view = MusicView::with(true, subscribe);
        view.set_enabled(false);
        view.set_enabled(true);
        assert!(!view.tick(Instant::now(), true));
        assert_eq!(tries.get(), 1);
    }

    #[test]
    fn a_vanished_player_is_forgotten_within_a_check() {
        let (mut view, fake) = view(true);
        let t0 = Instant::now();
        fake.playing.set(true);
        assert!(view.tick(t0, true));
        // il tourne encore : vérifié une fois par intervalle, rien n'est oublié
        let step = Duration::from_millis(16);
        for t in [t0 + step, t0 + ALIVE_EVERY, t0 + ALIVE_EVERY + step, t0 + ALIVE_EVERY * 2] {
            assert!(view.tick(t, true));
        }
        assert_eq!(fake.checks.get(), 2, "une vérification par intervalle");
        // il a planté sans rien annoncer : oublié à la vérification suivante, pas avant
        fake.gone.set(true);
        assert!(view.tick(t0 + ALIVE_EVERY * 2 + step, true));
        assert!(!view.tick(t0 + ALIVE_EVERY * 3, true));
        assert!(!view.playing());
        assert_eq!(fake.checks.get(), 3);
        // sans lecture, plus aucune vérification
        assert!(!view.tick(t0 + ALIVE_EVERY * 10, true));
        assert_eq!(fake.checks.get(), 3);
    }
}
