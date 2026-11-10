//! Ce que Claude Code est en train de faire, d'après les événements que `capri-hook` envoie.
//!
//! Un thread attend les lignes sur le canal local et les passe à la boucle principale, qui tient
//! à jour le suivi des sessions. Aucun modèle n'est appelé : tout vient des hooks.

use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use capri_claude::protocol::MAX_LINE_BYTES;
use capri_claude::{Headline, Message, Tracker};
use capri_ipc::{Endpoint, IpcError, Listener};

pub struct Feed {
    messages: Receiver<Message>,
    tracker: Tracker,
    /// Origine de l'horloge du suivi.
    origin: Instant,
}

impl Feed {
    /// Ouvre le canal et commence à écouter. `None` si c'est impossible : Capri vit très bien
    /// sans bandeau.
    pub fn start() -> Option<Self> {
        let endpoint = Endpoint::for_current_user()
            .inspect_err(|err| log::warn!("pas de canal pour Claude Code : {err}"))
            .ok()?;
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
        Some(Self { messages, tracker: Tracker::new(), origin: Instant::now() })
    }

    fn now_ms(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    /// Prend en compte les messages arrivés depuis le dernier appel. `true` s'il y en a eu.
    pub fn pump(&mut self) -> bool {
        let now_ms = self.now_ms();
        let mut any = false;
        while let Ok(message) = self.messages.try_recv() {
            log::debug!("Claude Code : {:?} (session {})", message.event, message.session);
            self.tracker.apply(&message, now_ms);
            any = true;
        }
        any
    }

    /// Ce que le bandeau doit dire maintenant.
    pub fn headline(&mut self) -> Option<Headline> {
        let now_ms = self.now_ms();
        self.tracker.headline(now_ms)
    }
}
