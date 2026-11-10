//! La conversation avec Capri, côté application.
//!
//! Un message part, un cerveau répond par morceaux sur un autre thread, et la réponse devient une
//! prise de parole que la bulle montre page par page. Pour l'instant le cerveau est celui de
//! `capri_chat::echo` : aucun modèle, aucun réseau.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use capri_chat::{Conversation, ReplyEvent, Role, echo, reply};
use capri_core::Speech;

/// Nombre de prises de parole gardées en mémoire.
const MAX_TURNS: usize = 24;
/// Le cerveau provisoire écrit un mot à ce rythme, comme le ferait un modèle.
const ECHO_PACE: Duration = Duration::from_millis(45);

pub struct Chat {
    conversation: Conversation,
    reply: Option<Reply>,
    speech: Option<Speech>,
    /// L'horloge de la parole : elle s'arrête quand Capri est cachée, pour que la lecture
    /// reprenne là où elle en était.
    clock: SpeechClock,
}

/// Une réponse en train d'arriver.
struct Reply {
    events: Receiver<ReplyEvent>,
    cancelled: Arc<AtomicBool>,
}

struct SpeechClock {
    origin: Instant,
    held_since: Option<Instant>,
    held_total: Duration,
}

impl SpeechClock {
    fn now_ms(&self) -> u64 {
        let until = self.held_since.unwrap_or_else(Instant::now);
        (until.duration_since(self.origin).saturating_sub(self.held_total)).as_millis() as u64
    }

    fn hold(&mut self) {
        self.held_since.get_or_insert_with(Instant::now);
    }

    fn resume(&mut self) {
        if let Some(since) = self.held_since.take() {
            self.held_total += since.elapsed();
        }
    }
}

impl Chat {
    pub fn new() -> Self {
        Self {
            conversation: Conversation::new(MAX_TURNS),
            reply: None,
            speech: None,
            clock: SpeechClock { origin: Instant::now(), held_since: None, held_total: Duration::ZERO },
        }
    }

    /// L'utilisateur dit quelque chose : ce qu'elle était en train de dire s'arrête, et elle
    /// répond. Un message vide ne lance rien.
    pub fn say(&mut self, message: &str) {
        let message = message.trim();
        if message.is_empty() {
            return;
        }
        self.hush();
        self.conversation.push(Role::User, message);

        let (sender, events) = channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let brain = {
            let (message, cancelled) = (message.to_owned(), Arc::clone(&cancelled));
            move || {
                let answer = echo::reply_to(&message);
                for piece in reply::chunks(&answer) {
                    if cancelled.load(Ordering::Relaxed)
                        || sender.send(ReplyEvent::Delta(piece.to_owned())).is_err()
                    {
                        return;
                    }
                    std::thread::sleep(ECHO_PACE);
                }
                let _ = sender.send(ReplyEvent::Done);
            }
        };
        self.clock.resume();
        self.speech = Some(Speech::new(self.clock.now_ms()));
        match std::thread::Builder::new().name("capri-cerveau".to_owned()).spawn(brain) {
            Ok(_) => self.reply = Some(Reply { events, cancelled }),
            Err(err) => {
                log::warn!("impossible de lancer la réponse : {err}");
                self.end_reply(Some("Je n'arrive pas à répondre."));
            }
        }
    }

    /// Fait taire Capri : la réponse en cours est abandonnée et sa bulle s'efface.
    pub fn hush(&mut self) {
        if let Some(reply) = self.reply.take() {
            reply.cancelled.store(true, Ordering::Relaxed);
        }
        self.speech = None;
    }

    /// Y a-t-il une prise de parole à montrer ou à venir ?
    pub fn is_speaking(&self) -> bool {
        self.speech.is_some()
    }

    /// Prend en compte ce que le cerveau a envoyé depuis le dernier appel.
    pub fn pump(&mut self) {
        let Some(reply) = &self.reply else { return };
        loop {
            match reply.events.try_recv() {
                Ok(ReplyEvent::Delta(piece)) => {
                    if let Some(speech) = &mut self.speech {
                        speech.push(&piece);
                    }
                }
                Ok(ReplyEvent::Done) => return self.end_reply(None),
                Ok(ReplyEvent::Failed(reason)) => return self.end_reply(Some(&reason)),
                Err(TryRecvError::Empty) => return,
                // le cerveau s'est arrêté sans prévenir : on garde ce qui est arrivé
                Err(TryRecvError::Disconnected) => return self.end_reply(None),
            }
        }
    }

    /// La réponse est complète, ou s'arrête sur un problème à dire à l'utilisateur.
    fn end_reply(&mut self, problem: Option<&str>) {
        self.reply = None;
        let now_ms = self.clock.now_ms();
        let Some(speech) = &mut self.speech else { return };
        if let Some(problem) = problem {
            if !speech.text().trim().is_empty() {
                speech.push(" — ");
            }
            speech.push(problem);
        } else {
            self.conversation.push(Role::Capri, speech.text());
        }
        speech.finish(now_ms);
    }

    /// La page de sa parole à montrer maintenant, et s'il y en a d'autres après. La lecture
    /// avance à chaque appel.
    pub fn page(&mut self) -> Option<(String, bool)> {
        self.clock.resume();
        let now_ms = self.clock.now_ms();
        let page = self.speech.as_mut()?.view(now_ms).map(|view| (view.text.to_owned(), view.more));
        if page.is_none() {
            self.speech = None;
        }
        page
    }

    /// Personne ne peut lire la bulle (Capri est cachée) : la lecture attend, sans perdre de page.
    pub fn hold(&mut self) {
        if self.speech.is_some() {
            self.clock.hold();
        }
    }

    /// Nombre de prises de parole en mémoire.
    #[cfg(test)]
    fn turns(&self) -> usize {
        self.conversation.turns().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Laisse le cerveau finir, en relevant ce qu'il envoie.
    fn listen(chat: &mut Chat) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        while chat.reply.is_some() {
            assert!(Instant::now() < deadline, "le cerveau ne répond pas");
            chat.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        chat.speech.as_ref().map(|speech| speech.text().to_owned()).unwrap_or_default()
    }

    #[test]
    fn she_answers_what_she_is_told_and_remembers_the_exchange() {
        let mut chat = Chat::new();
        assert!(!chat.is_speaking() && chat.page().is_none());
        chat.say("  Ça marche ?  ");
        assert!(chat.is_speaking());
        assert_eq!(chat.page(), Some(("…".to_owned(), false)), "elle réfléchit avant le premier mot");
        let answer = listen(&mut chat);
        assert!(answer.contains("« Ça marche ? »"), "{answer}");
        assert_eq!(chat.turns(), 2, "la question et la réponse");
        let (page, _) = chat.page().expect("la réponse reste le temps d'être lue");
        assert!(answer.starts_with(&page));
    }

    #[test]
    fn an_empty_message_starts_nothing() {
        let mut chat = Chat::new();
        chat.say("   ");
        assert!(!chat.is_speaking());
        assert_eq!(chat.turns(), 0);
    }

    #[test]
    fn a_new_message_or_a_hush_cuts_her_off() {
        let mut chat = Chat::new();
        chat.say(&"un très long message ".repeat(40));
        chat.pump();
        chat.say("autre chose");
        let answer = listen(&mut chat);
        assert!(answer.contains("« autre chose »") && !answer.contains("très long"), "{answer}");
        // la première réponse, coupée, n'a pas été retenue
        assert_eq!(chat.turns(), 3);

        chat.say("encore");
        chat.hush();
        assert!(!chat.is_speaking() && chat.page().is_none());
    }

    #[test]
    fn holding_the_bubble_pauses_the_reading() {
        let mut clock = SpeechClock { origin: Instant::now(), held_since: None, held_total: Duration::ZERO };
        clock.hold();
        let frozen = clock.now_ms();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(clock.now_ms(), frozen, "le temps ne passe pas pendant l'attente");
        clock.hold(); // une seconde fois : sans effet
        clock.resume();
        let resumed = clock.now_ms();
        assert!(resumed < frozen + 25, "l'attente n'est pas comptée : {frozen} puis {resumed}");
        std::thread::sleep(Duration::from_millis(30));
        assert!(clock.now_ms() >= resumed + 25, "puis il reprend");
    }
}
