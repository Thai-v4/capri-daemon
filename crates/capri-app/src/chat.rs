//! La conversation avec Capri, côté application.
//!
//! Un message part, un cerveau répond par morceaux sur un autre thread, et la réponse devient une
//! prise de parole que la bulle montre page par page.
//!
//! Le cerveau est celui que désignent les réglages (`brain_setup`), relus à chaque message : le
//! modèle choisi, joint par `capri-net`. Sans réglages, c'est le cerveau de secours de
//! `capri_chat::echo`, qui répète ce qu'on lui dit et n'envoie rien nulle part.
//!
//! Seules la personnalité de Capri et la conversation partent vers le modèle : jamais ce que
//! fait Claude Code, que cette partie de l'app ne connaît même pas.
//!
//! Si sa voix est activée (`voice_setup`), chaque phrase terminée est aussi dite à voix haute par
//! le système, pendant que le modèle écrit la suite.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use capri_chat::plain::PlainStream;
use capri_chat::voice::{Sentences, speakable};
use capri_chat::{Conversation, ReplyEvent, Role, Turn, echo, reply};
use capri_core::Speech;
use capri_net::Client;
use capri_platform::Voice;

use crate::{brain_setup, voice_setup};

/// Nombre de prises de parole gardées en mémoire, et envoyées au modèle avec chaque message.
const MAX_TURNS: usize = 24;
/// Le cerveau de secours écrit un mot à ce rythme, comme le ferait un modèle.
const ECHO_PACE: Duration = Duration::from_millis(45);

pub struct Chat {
    conversation: Conversation,
    reply: Option<Reply>,
    speech: Option<Speech>,
    /// L'horloge de la parole : elle s'arrête quand Capri est cachée, pour que la lecture
    /// reprenne là où elle en était.
    clock: SpeechClock,
    /// Le dossier où lire quel cerveau utiliser. `None` : toujours le cerveau de secours.
    settings: Option<PathBuf>,
    client: Arc<Client>,
    /// Sa voix, créée la première fois qu'elle doit parler à voix haute.
    voice: Option<Voice>,
    /// Ce qu'il faut pour dire la réponse en cours à voix haute, si sa voix est activée.
    aloud: Option<Aloud>,
    /// Sa voix est suspendue parce que Capri est cachée.
    voice_held: bool,
}

/// Une réponse en train d'arriver.
struct Reply {
    events: Receiver<Thought>,
    cancelled: Arc<AtomicBool>,
}

/// Ce que le thread du cerveau envoie.
enum Thought {
    /// Cette réponse se dit aussi à voix haute, avec cette voix (`None` : celle du système).
    /// Envoyé avant le premier mot, et seulement si sa voix est activée.
    Aloud(Option<String>),
    Reply(ReplyEvent),
}

/// La réponse en cours, côté voix.
struct Aloud {
    voice_id: Option<String>,
    /// Le texte reçu, découpé en phrases : une phrase n'est dite qu'une fois entière.
    sentences: Sentences,
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
    /// `settings` : le dossier des réglages de Capri, où un cerveau a pu être choisi.
    pub fn new(settings: Option<PathBuf>) -> Self {
        Self {
            conversation: Conversation::new(MAX_TURNS),
            reply: None,
            speech: None,
            clock: SpeechClock { origin: Instant::now(), held_since: None, held_total: Duration::ZERO },
            settings,
            client: Arc::new(Client::new()),
            voice: None,
            aloud: None,
            voice_held: false,
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
            let (settings, client) = (self.settings.clone(), Arc::clone(&self.client));
            let (turns, cancelled) = (self.conversation.turns().to_vec(), Arc::clone(&cancelled));
            move || think(settings.as_deref(), &client, &turns, &cancelled, &sender)
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

    /// Fait taire Capri : la réponse en cours est abandonnée (le modèle cesse d'écrire), sa
    /// bulle s'efface et sa voix s'arrête.
    pub fn hush(&mut self) {
        if let Some(reply) = self.reply.take() {
            reply.cancelled.store(true, Ordering::Relaxed);
        }
        self.speech = None;
        self.silence();
    }

    /// Arrête sa voix tout de suite, sans toucher à sa bulle.
    pub fn silence(&mut self) {
        if let Some(voice) = &self.voice {
            voice.stop();
        }
        self.aloud = None;
        self.voice_held = false;
    }

    /// Repart de zéro : elle se tait et oublie ce qui s'est dit.
    #[cfg(any(target_os = "macos", windows))]
    pub fn forget(&mut self) {
        self.hush();
        self.conversation.clear();
    }

    /// Y a-t-il une prise de parole à montrer ou à venir ?
    pub fn is_speaking(&self) -> bool {
        self.speech.is_some()
    }

    /// Prend en compte ce que le cerveau a envoyé depuis le dernier appel.
    pub fn pump(&mut self) {
        loop {
            let Some(reply) = &self.reply else { return };
            match reply.events.try_recv() {
                Ok(Thought::Aloud(voice_id)) => {
                    if self.voice.is_none() {
                        self.voice = Voice::new();
                    }
                    self.aloud = Some(Aloud { voice_id, sentences: Sentences::new() });
                }
                Ok(Thought::Reply(ReplyEvent::Delta(piece))) => {
                    if let Some(speech) = &mut self.speech {
                        speech.push(&piece);
                    }
                    self.say_aloud(&piece, false);
                }
                Ok(Thought::Reply(ReplyEvent::Done)) => {
                    self.say_aloud("", true);
                    return self.end_reply(None);
                }
                Ok(Thought::Reply(ReplyEvent::Failed(reason))) => {
                    // ce qu'elle avait commencé à dire est dit ; la panne, elle, reste écrite
                    self.say_aloud("", true);
                    return self.end_reply(Some(&reason));
                }
                Err(TryRecvError::Empty) => return,
                // le cerveau s'est arrêté sans prévenir : on garde ce qui est arrivé
                Err(TryRecvError::Disconnected) => {
                    self.say_aloud("", true);
                    return self.end_reply(None);
                }
            }
        }
    }

    /// Confie à sa voix les phrases que `text` vient de terminer. `last` : la réponse est finie,
    /// ce qui reste se dit aussi, même sans point final.
    fn say_aloud(&mut self, text: &str, last: bool) {
        let (Some(voice), Some(aloud)) = (&self.voice, &mut self.aloud) else { return };
        let mut ready = aloud.sentences.push(text);
        if last {
            ready.extend(aloud.sentences.finish());
        }
        for sentence in ready {
            let spoken = speakable(&sentence);
            if !spoken.is_empty() {
                voice.say(&spoken, aloud.voice_id.as_deref());
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
            // la question n'a pas eu sa réponse : reposée, elle ne doit pas arriver en double
            self.conversation.retract_unanswered();
        } else {
            self.conversation.push(Role::Capri, speech.text());
        }
        speech.finish(now_ms);
    }

    /// La page de sa parole à montrer maintenant, et s'il y en a d'autres après. La lecture
    /// avance à chaque appel.
    pub fn page(&mut self) -> Option<(String, bool)> {
        self.clock.resume();
        if std::mem::take(&mut self.voice_held)
            && let Some(voice) = &self.voice
        {
            voice.resume();
        }
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
        if !self.voice_held
            && let Some(voice) = self.voice.as_ref().filter(|voice| voice.is_speaking())
        {
            voice.pause();
            self.voice_held = true;
        }
    }

    /// Nombre de prises de parole en mémoire.
    #[cfg(test)]
    fn turns(&self) -> usize {
        self.conversation.turns().len()
    }
}

/// Le travail du thread « capri-cerveau » : trouver quel cerveau répond, et le laisser répondre
/// à la conversation `turns` (qui finit par le message de l'utilisateur).
fn think(
    settings: Option<&Path>,
    client: &Client,
    turns: &[Turn],
    cancelled: &AtomicBool,
    sender: &Sender<Thought>,
) {
    // si personne n'écoute plus, `cancelled` est déjà levé : la réponse va s'arrêter d'elle-même
    let mut emit = |event: ReplyEvent| {
        let _ = sender.send(Thought::Reply(event));
    };
    // sa voix d'abord : elle doit être connue avant le premier mot
    match settings.map(voice_setup::load) {
        Some(Ok(voice)) if voice.enabled => {
            let _ = sender.send(Thought::Aloud(voice_setup::chosen_voice(&voice)));
        }
        Some(Err(err)) => log::warn!("pas de voix : {err:#}"),
        _ => {}
    }
    let setup = match settings.map(brain_setup::load).transpose() {
        Ok(setup) => setup.flatten(),
        Err(err) => {
            log::error!("cerveau : {err:#}");
            let problem = "Mes réglages sont illisibles : capri chat status te dira pourquoi.";
            return emit(ReplyEvent::Failed(problem.to_owned()));
        }
    };
    let Some(setup) = setup else { return repeat(turns, cancelled, &mut emit) };
    log::debug!("réponse demandée à {} ({})", setup.brain.model(), setup.brain.provider().name());
    // un modèle écrit volontiers du Markdown, qui n'a aucun sens dans une bulle ni à voix haute
    let mut plain = PlainStream::new();
    client.answer(&setup.brain, None, &setup.persona, turns, cancelled, &mut |event| {
        plain.filter(event, &mut emit)
    });
}

/// Le cerveau de secours : il répète le dernier message, mot à mot.
fn repeat(turns: &[Turn], cancelled: &AtomicBool, emit: &mut dyn FnMut(ReplyEvent)) {
    let message = turns.last().map_or("", |turn| turn.text.as_str());
    let answer = echo::reply_to(message);
    for piece in reply::chunks(&answer) {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        emit(ReplyEvent::Delta(piece.to_owned()));
        std::thread::sleep(ECHO_PACE);
    }
    emit(ReplyEvent::Done);
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

    /// Un dossier de réglages propre à ce test (le nom ne doit servir à aucun autre, ici ou dans
    /// `brain_setup`), où un cerveau est choisi à cette adresse.
    fn settings_with_a_brain_at(name: &str, address: std::net::SocketAddr) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("capri-test-chat-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let brain = capri_chat::Brain::new(
            capri_chat::Provider::Ollama,
            Some(&format!("http://{address}/v1")),
            "modele-de-test",
        )
        .expect("adresse locale");
        brain_setup::save(&directory, &brain).expect("réglages écrits");
        directory
    }

    /// Un faux fournisseur, sur cette machine : il répond « Réponse 1 », « Réponse 2 »… à chaque
    /// requête, et rend le corps de ce qu'il a reçu.
    fn fake_provider(replies: usize) -> (std::net::SocketAddr, Receiver<serde_json::Value>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("un port libre");
        let address = listener.local_addr().expect("adresse");
        let (sender, requests) = channel();
        std::thread::spawn(move || {
            for (number, stream) in listener.incoming().take(replies).enumerate() {
                let Ok(mut stream) = stream else { return };
                let (mut bytes, mut buffer) = (Vec::new(), [0u8; 4096]);
                // la requête entière : ses en-têtes, puis le corps qu'ils annoncent
                let body = loop {
                    if let Some(head_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        let length = String::from_utf8_lossy(&bytes[..head_end])
                            .lines()
                            .filter_map(|line| line.split_once(':'))
                            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if bytes.len() >= head_end + 4 + length {
                            break String::from_utf8_lossy(&bytes[head_end + 4..]).into_owned();
                        }
                    }
                    match stream.read(&mut buffer) {
                        Ok(received) if received > 0 => bytes.extend_from_slice(&buffer[..received]),
                        _ => return,
                    }
                };
                let _ = sender.send(serde_json::from_str(&body).expect("une requête en JSON"));
                let event = serde_json::json!({ "choices": [{ "delta": { "content": format!("Réponse {}", number + 1) } }] });
                let stream_body = format!("data: {event}\n\ndata: [DONE]\n\n");
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{stream_body}",
                    stream_body.len()
                );
            }
        });
        (address, requests)
    }

    #[test]
    fn with_a_brain_she_answers_through_it_and_sends_the_conversation_so_far() {
        let (address, requests) = fake_provider(2);
        let directory = settings_with_a_brain_at("cerveau", address);
        let mut chat = Chat::new(Some(directory.clone()));

        chat.say("Bonjour");
        assert_eq!(listen(&mut chat), "Réponse 1");
        chat.say("Et ensuite ?");
        assert_eq!(listen(&mut chat), "Réponse 2");
        assert_eq!(chat.turns(), 4);

        let first = requests.recv_timeout(Duration::from_secs(5)).expect("première requête");
        assert_eq!(first["model"], "modele-de-test");
        assert_eq!(first["messages"][0]["role"], "system");
        assert_eq!(first["messages"][0]["content"], capri_chat::persona::DEFAULT);
        assert_eq!(first["messages"][1], serde_json::json!({ "role": "user", "content": "Bonjour" }));
        assert_eq!(first["messages"].as_array().map(Vec::len), Some(2));
        let second = requests.recv_timeout(Duration::from_secs(5)).expect("seconde requête");
        assert_eq!(
            second["messages"].as_array().expect("des messages")[1..],
            [
                serde_json::json!({ "role": "user", "content": "Bonjour" }),
                serde_json::json!({ "role": "assistant", "content": "Réponse 1" }),
                serde_json::json!({ "role": "user", "content": "Et ensuite ?" }),
            ]
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_brain_that_cannot_be_reached_says_so_and_the_question_is_taken_back() {
        // un port qui vient d'être libéré : personne n'y écoute
        let address = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("un port");
        let directory = settings_with_a_brain_at("injoignable", address);
        let mut chat = Chat::new(Some(directory.clone()));
        chat.say("Tu es là ?");
        let answer = listen(&mut chat);
        assert!(answer.starts_with("Je n'arrive pas à joindre Ollama"), "{answer}");
        assert_eq!(chat.turns(), 0, "la question sans réponse n'est pas gardée");
        assert!(chat.page().is_some(), "et elle le dit dans sa bulle");

        // des réglages illisibles ne la rendent pas muette, et ne la font pas répéter non plus
        std::fs::write(directory.join("chat.json"), "{ cassé").expect("écrit");
        chat.say("Et maintenant ?");
        let answer = listen(&mut chat);
        assert!(answer.contains("réglages sont illisibles"), "{answer}");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn she_answers_what_she_is_told_and_remembers_the_exchange() {
        let mut chat = Chat::new(None);
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
        let mut chat = Chat::new(None);
        chat.say("   ");
        assert!(!chat.is_speaking());
        assert_eq!(chat.turns(), 0);
    }

    #[test]
    fn a_new_message_or_a_hush_cuts_her_off() {
        let mut chat = Chat::new(None);
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
