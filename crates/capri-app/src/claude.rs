//! Ce que Claude Code est en train de faire, d'après les événements que `capri-hook` envoie.
//!
//! Un thread attend les lignes sur le canal local et les passe à la boucle principale, qui tient
//! à jour le suivi des sessions. Aucun modèle n'est appelé : tout vient des hooks.
//!
//! Avec le miroir, une fin de tour peut porter la ligne que Claude a écrite pour Capri : elle
//! attend ici (`capri_claude::mirror::Waiting` : la plus récente seulement, pas trop longtemps)
//! que Capri soit libre de la dire. Elle n'est jamais écrite dans le journal, sa longueur
//! seulement.

use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use capri_claude::mirror::Waiting;
use capri_claude::protocol::{Event, MAX_LINE_BYTES, Said};
use capri_claude::{Headline, Message, Tracker};
use capri_ipc::{Endpoint, IpcError, Listener};

pub struct Feed {
    messages: Receiver<Message>,
    tracker: Tracker,
    /// La ligne de Claude pour Capri qui attend d'être dite.
    waiting: Waiting,
    /// Origine de l'horloge du suivi.
    origin: Instant,
    endpoint: Endpoint,
}

impl Drop for Feed {
    /// Le thread d'écoute ne s'arrête qu'avec le processus : c'est ici qu'on range le canal.
    fn drop(&mut self) {
        capri_ipc::retire(&self.endpoint);
    }
}

impl Feed {
    /// Ouvre le canal et commence à écouter. `None` si c'est impossible : Capri vit très bien
    /// sans bandeau.
    pub fn start() -> Option<Self> {
        let endpoint = Endpoint::for_current_user()
            .inspect_err(|err| log::warn!("pas de canal pour Claude Code : {err}"))
            .ok()?;
        Self::start_at(endpoint)
    }

    /// Comme [`Feed::start`], sur ce canal.
    fn start_at(endpoint: Endpoint) -> Option<Self> {
        let mut listener = match Listener::bind(&endpoint, MAX_LINE_BYTES) {
            Ok(listener) => listener,
            Err(IpcError::AlreadyRunning) => {
                log::warn!("une autre Capri écoute déjà Claude Code : pas de bandeau dans celle-ci");
                return None;
            }
            Err(err) => {
                log::warn!("impossible d'écouter Claude Code sur {} : {err}", endpoint.path().display());
                return None;
            }
        };

        let (sender, messages) = channel();
        std::thread::Builder::new()
            .name("capri-claude".to_owned())
            .spawn(move || {
                loop {
                    match listener.next_line() {
                        Ok(line) => match Message::decode(&line) {
                            // la boucle principale est partie : plus personne à prévenir
                            Ok(message) => {
                                if sender.send(message).is_err() {
                                    break;
                                }
                            }
                            Err(err) => log::debug!("message de hook ignoré : {err}"),
                        },
                        Err(err) => {
                            log::warn!("écoute de Claude Code interrompue : {err}");
                            break;
                        }
                    }
                }
            })
            .inspect_err(|err| log::warn!("impossible de lancer l'écoute de Claude Code : {err}"))
            .ok()?;

        log::info!("à l'écoute de Claude Code sur {}", endpoint.path().display());
        Some(Self {
            messages,
            tracker: Tracker::new(),
            waiting: Waiting::new(),
            origin: Instant::now(),
            endpoint,
        })
    }

    fn now_ms(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    /// Prend en compte les messages arrivés depuis le dernier appel. `true` s'il y en a eu.
    pub fn pump(&mut self) -> bool {
        let now_ms = self.now_ms();
        let mut any = false;
        while let Ok(mut message) = self.messages.try_recv() {
            // le `Debug` d'une phrase pour Capri ne montre que sa longueur
            log::debug!("Claude Code : {:?} (session {})", message.event, message.session);
            match &mut message.event {
                Event::TurnEnd { said } => {
                    if let Some(said) = said.take() {
                        self.waiting.offer(said, &message.session, now_ms);
                    }
                }
                // un nouveau prompt, ou la fin de la session : sa phrase d'avant serait périmée
                Event::TurnStart | Event::SessionEnd { .. } => self.waiting.moved_on(&message.session),
                _ => {}
            }
            self.tracker.apply(&message, now_ms);
            any = true;
        }
        any
    }

    /// La ligne de Claude pour Capri à dire maintenant, s'il y en a une qui n'est pas périmée.
    /// Rendue une seule fois.
    pub fn take_said(&mut self) -> Option<Said> {
        let now_ms = self.now_ms();
        self.waiting.take(now_ms)
    }

    /// Oublie la ligne qui attendait : on a fait taire Capri (depuis son menu).
    #[cfg(any(target_os = "macos", windows, test))]
    pub fn drop_said(&mut self) {
        self.waiting.clear();
    }

    /// Ce que le bandeau doit dire maintenant.
    pub fn headline(&mut self) -> Option<Headline> {
        let now_ms = self.now_ms();
        self.tracker.headline(now_ms)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;

    /// Un canal neuf, au chemin court, propre à ce test.
    fn endpoint(name: &str) -> Endpoint {
        let mut base = std::env::temp_dir();
        if cfg!(unix) && base.as_os_str().len() > 55 {
            base = PathBuf::from("/tmp");
        }
        let dir = base.join(format!("capri-test-feed-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Endpoint::at(dir.join(if cfg!(windows) { "hook-pipe.txt" } else { "hook.sock" }))
    }

    /// Envoie cet événement comme le ferait `capri-hook`, et attend que le suivi l'ait reçu.
    fn deliver(feed: &mut Feed, endpoint: &Endpoint, event: Event) {
        let message = Message {
            v: capri_claude::protocol::VERSION,
            session: "s".into(),
            cwd: None,
            at_ms: 1,
            agent: None,
            event,
        };
        capri_ipc::send_line(endpoint, &message.encode().unwrap(), Duration::from_secs(2)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !feed.pump() {
            assert!(Instant::now() < deadline, "le message n'arrive pas");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn said(text: &str) -> Option<Said> {
        Some(Said::new(text.to_owned()))
    }

    #[test]
    fn the_latest_line_for_capri_waits_for_her_and_the_banner_still_says_claude_is_done() {
        let endpoint = endpoint("phrase");
        let mut feed = Feed::start_at(endpoint.clone()).expect("le canal s'ouvre");
        assert_eq!(feed.take_said(), None);
        deliver(&mut feed, &endpoint, Event::TurnEnd { said: said("Première.") });
        deliver(&mut feed, &endpoint, Event::TurnEnd { said: said("Seconde.") });
        assert_eq!(feed.take_said(), said("Seconde."), "la plus récente seulement");
        assert_eq!(feed.take_said(), None, "une seule fois");
        assert_eq!(
            feed.headline().map(|h| h.text).as_deref(),
            Some("Claude a fini"),
            "le bandeau, comme avant"
        );

        // une fin de tour sans phrase n'efface pas celle qui attend ; la faire taire, si
        deliver(&mut feed, &endpoint, Event::TurnEnd { said: said("Encore.") });
        deliver(&mut feed, &endpoint, Event::TurnEnd { said: None });
        assert_eq!(feed.take_said(), said("Encore."));
        deliver(&mut feed, &endpoint, Event::TurnEnd { said: said("Tue.") });
        feed.drop_said();
        assert_eq!(feed.take_said(), None);
        // un nouveau prompt dans la même session : la phrase du tour d'avant n'est plus dite
        deliver(&mut feed, &endpoint, Event::TurnEnd { said: said("Périmée.") });
        deliver(&mut feed, &endpoint, Event::TurnStart);
        assert_eq!(feed.take_said(), None);
        drop(feed);
        assert!(!endpoint.path().exists(), "le canal est rangé");
        let _ = std::fs::remove_dir_all(endpoint.path().parent().unwrap());
    }
}
