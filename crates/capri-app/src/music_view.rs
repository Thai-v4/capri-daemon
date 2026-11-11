//! Elle se balance sur ta musique, assise : quand écouter si un son sort d'une autre app, et ce
//! qu'elle en conclut. L'écoute est dans `capri_platform::music` (un oui ou un non, sans
//! permission, sans titre ni nom d'app), la décision dans `capri_core::music::Groove`.
//!
//! **L'écoute ne tourne que quand elle sert** : elle est assise (avec sa pose assise), visible, et
//! c'est permis (« Se balancer sur la musique » dans son menu). Une fois par seconde
//! ([`LISTEN_EVERY`]), sur un thread à elle (« capri-musique ») : la première écoute d'un processus
//! prend environ 70 ms (mesuré), elle ne doit pas retenir une image. Dès qu'elle ne sert plus,
//! plus rien n'est demandé, et ce qui avait été entendu est oublié. Le thread attend, endormi,
//! qu'on lui demande d'écouter de nouveau.
//!
//! **Sa propre voix ne compte pas** : le système l'attribue à son propre processus, que l'écoute
//! écarte (mesuré par `capri music rehearse`, sans effet comme avec, sans un bruit).
//!
//! Le journal n'en dit que des oui et des non. Windows et ailleurs : elle ne sait pas, et ne se
//! balance pas.

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Instant;

use capri_core::music::{Groove, LISTEN_EVERY};
use capri_platform::music::{self, MusicError, SoundWatch};

/// Ce qui écoute vraiment : une écoute, rendue comme un oui, un non, ou ce que le système refuse.
type Listener = Box<dyn FnMut() -> Result<bool, MusicError> + Send>;

pub struct MusicView {
    enabled: bool,
    groove: Groove,
    /// Le thread d'écoute, lancé à la première écoute utile.
    worker: Option<Worker>,
    /// Une écoute demandée n'a pas encore répondu.
    pending: bool,
    /// Pas d'écoute avant cet instant.
    next: Option<Instant>,
    /// Ce qui écoutera, tant que le thread n'est pas lancé.
    listener: Option<Listener>,
    /// Elle ne sait pas écouter ici (ou le thread n'a pas pu être lancé) : elle n'essaie plus.
    unavailable: bool,
}

struct Worker {
    requests: Sender<Instant>,
    answers: Receiver<(Instant, Result<bool, MusicError>)>,
}

impl MusicView {
    /// `enabled` : le réglage de son menu.
    pub fn new(enabled: bool) -> Self {
        let mut watch = SoundWatch::new();
        let listener: Listener = Box::new(move || watch.listen().map(|heard| heard.playing));
        Self::with_listener(enabled, music::AVAILABLE, listener)
    }

    fn with_listener(enabled: bool, available: bool, listener: Listener) -> Self {
        Self {
            enabled,
            groove: Groove::new(),
            worker: None,
            pending: false,
            next: None,
            listener: Some(listener),
            unavailable: !available,
        }
    }

    /// Le réglage se change dans son menu, qui n'existe que sur macOS et Windows.
    #[cfg(any(target_os = "macos", windows))]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Le lui permettre, ou le lui interdire : elle cesse aussitôt d'écouter.
    #[cfg(any(target_os = "macos", windows))]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// À chaque pas, à l'instant `now` : `wanted`, elle est assise et visible. Rend : de la musique
    /// joue-t-elle (de quoi se balancer) ?
    pub fn tick(&mut self, now: Instant, wanted: bool) -> bool {
        let listening = wanted && self.enabled && !self.unavailable;
        self.collect(listening);
        if !listening {
            if self.groove.on() {
                log::debug!("musique : elle ne se balance plus (elle n'écoute plus)");
            }
            self.groove.reset();
            self.next = None;
            return false;
        }
        if !self.pending && self.next.is_none_or(|next| now >= next) {
            self.ask(now);
        }
        self.groove.on()
    }

    /// Relève les écoutes revenues ; elles ne comptent que si elle écoute encore.
    fn collect(&mut self, listening: bool) {
        let Some(worker) = &self.worker else { return };
        let mut answers = Vec::new();
        let stopped = loop {
            match worker.answers.try_recv() {
                Ok(answer) => answers.push(answer),
                Err(TryRecvError::Empty) => break false,
                Err(TryRecvError::Disconnected) => break true,
            }
        };
        if stopped {
            log::warn!("musique : l'écoute s'est arrêtée, elle ne se balancera pas");
            self.unavailable = true;
        }
        if !answers.is_empty() || stopped {
            self.pending = false;
        }
        if listening {
            for (at, heard) in answers {
                self.hear(at, heard);
            }
        }
    }

    fn hear(&mut self, at: Instant, heard: Result<bool, MusicError>) {
        let was = self.groove.on();
        let now = match heard {
            Ok(playing) => self.groove.hear(at, Some(playing)),
            Err(MusicError::Unavailable) => {
                log::info!("musique : elle ne sait pas, sur ce système, si un son joue");
                self.unavailable = true;
                self.groove.hear(at, None)
            }
            Err(err) => {
                log::debug!("musique : {err}");
                self.groove.hear(at, None)
            }
        };
        if now != was {
            log::debug!("musique : elle {}", if now { "se balance" } else { "ne se balance plus" });
        }
    }

    /// Demande une écoute, à faire tout de suite, sur le thread d'écoute (lancé s'il ne l'est pas).
    fn ask(&mut self, now: Instant) {
        if self.worker.is_none() {
            self.worker = self.listener.take().and_then(spawn);
            if self.worker.is_none() {
                self.unavailable = true;
                return;
            }
        }
        let Some(worker) = &self.worker else { return };
        if worker.requests.send(now).is_ok() {
            self.pending = true;
            self.next = Some(now + LISTEN_EVERY);
        }
    }

    /// Pour les essais : attend la réponse à l'écoute demandée, et la prend en compte.
    #[cfg(test)]
    fn wait(&mut self) {
        let Some(worker) = self.worker.as_ref().filter(|_| self.pending) else { return };
        if let Ok((at, heard)) = worker.answers.recv_timeout(std::time::Duration::from_secs(5)) {
            self.pending = false;
            self.hear(at, heard);
        }
    }
}

/// Lance le thread d'écoute : il écoute à chaque demande, et rend l'instant de la demande avec ce
/// qu'il a entendu. Il s'arrête quand la vue disparaît.
fn spawn(mut listen: Listener) -> Option<Worker> {
    let (requests, asked) = channel::<Instant>();
    let (answer, answers) = channel();
    let spawned = std::thread::Builder::new().name("capri-musique".to_owned()).spawn(move || {
        for at in asked {
            if answer.send((at, listen())).is_err() {
                return;
            }
        }
    });
    match spawned {
        Ok(_) => Some(Worker { requests, answers }),
        Err(err) => {
            log::warn!("musique : pas d'écoute ({err}), elle ne se balancera pas");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    /// Une écoute qui répond toujours `playing`, et compte combien de fois on l'a appelée.
    fn counting(playing: bool) -> (Listener, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&count);
        (
            Box::new(move || {
                seen.fetch_add(1, Ordering::SeqCst);
                Ok(playing)
            }),
            count,
        )
    }

    /// Un pas par seconde pendant `seconds` secondes, l'écoute attendue à chaque fois ; rend ce
    /// qu'elle en conclut à la fin.
    fn listen_for(view: &mut MusicView, t0: Instant, from: u64, seconds: u64, wanted: bool) -> bool {
        for s in from..from + seconds {
            view.tick(t0 + Duration::from_secs(s), wanted);
            view.wait();
        }
        view.groove.on()
    }

    #[test]
    fn she_listens_once_a_second_only_when_seated_visible_and_allowed() {
        let (listener, count) = counting(true);
        let mut view = MusicView::with_listener(true, true, listener);
        let t0 = Instant::now();
        // debout ou cachée : aucune écoute, pas même de thread
        assert!(!view.tick(t0, false));
        assert!(view.worker.is_none());
        // assise : une écoute par seconde, pas une de plus entre deux
        view.tick(t0, true);
        view.wait();
        view.tick(t0 + Duration::from_millis(400), true);
        view.tick(t0 + Duration::from_millis(900), true);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        // trois secondes de son : de la musique
        assert!(listen_for(&mut view, t0, 1, 3, true));
        assert_eq!(count.load(Ordering::SeqCst), 4);
        // elle se lève : plus d'écoute, et ce qu'elle avait entendu est oublié
        assert!(!view.tick(t0 + Duration::from_secs(5), false));
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(count.load(Ordering::SeqCst), 4);
        assert!(!view.tick(t0 + Duration::from_secs(6), true), "elle repart de zéro");
    }

    #[test]
    fn forbidden_or_unavailable_she_never_listens_nor_sways() {
        let (listener, count) = counting(true);
        let mut view = MusicView::with_listener(false, true, listener);
        assert!(!listen_for(&mut view, Instant::now(), 0, 6, true));
        assert_eq!(count.load(Ordering::SeqCst), 0, "interdit : aucune écoute");

        let (listener, count) = counting(true);
        let mut view = MusicView::with_listener(true, false, listener);
        assert!(!listen_for(&mut view, Instant::now(), 0, 6, true));
        assert_eq!(count.load(Ordering::SeqCst), 0, "ce système ne sait pas : aucune écoute");

        // le système dit qu'il ne sait pas : elle n'essaie plus
        let tries = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&tries);
        let listener: Listener = Box::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            Err(MusicError::Unavailable)
        });
        let mut view = MusicView::with_listener(true, true, listener);
        assert!(!listen_for(&mut view, Instant::now(), 0, 6, true));
        assert_eq!(tries.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_short_sound_does_not_make_her_sway() {
        let (listener, _) = counting(false);
        let mut view = MusicView::with_listener(true, true, listener);
        assert!(!listen_for(&mut view, Instant::now(), 0, 10, true));
    }
}
