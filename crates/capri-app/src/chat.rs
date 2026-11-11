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
//! fait Claude Code, que cette partie de l'app ne connaît même pas. S'il faut une clé, celle
//! rangée pour l'adresse du cerveau est lue dans le trousseau (`keys`) à chaque message, et confiée
//! à `capri-net` pour l'en-tête de la requête : elle ne passe par rien d'autre.
//!
//! Si sa voix est activée (`voice_setup`), chaque phrase terminée est aussi dite à voix haute par
//! le système, pendant que le modèle écrit la suite.
//!
//! Elle dit aussi, comme une réponse, la ligne que Claude Code a écrite pour elle quand le miroir
//! est activé ([`Chat::relay`]) : dans sa bulle, à voix haute si sa voix est activée, et « La
//! faire taire » la coupe. Cette ligne n'entre pas dans la conversation : son cerveau ne la voit
//! jamais.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use capri_chat::plain::{PlainStream, plain_text};
use capri_chat::voice::{Sentences, speakable};
use capri_chat::{Conversation, Place, ReplyEvent, Role, Turn, echo, reply};
use capri_core::Speech;
use capri_net::{Ask, Client};
use capri_platform::{Delivery, Voice};

use crate::keys::{self, KeyStore};
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
    /// Là où sont rangées les clés des fournisseurs.
    keys: Arc<dyn KeyStore>,
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
    origin: Origin,
}

/// D'où vient ce qu'elle dit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    /// Sa réponse à l'utilisateur : elle entre dans la conversation.
    Conversation,
    /// La ligne que Claude Code a écrite pour elle : elle la dit, et l'oublie.
    ClaudeCode,
}

/// Ce que le thread du cerveau envoie.
enum Thought {
    /// Cette réponse se dit aussi à voix haute, ainsi (voix, hauteur, débit, effets). Envoyé
    /// avant le premier mot, et seulement si sa voix est activée.
    Aloud(Delivery),
    Reply(ReplyEvent),
}

/// La réponse en cours, côté voix.
struct Aloud {
    delivery: Delivery,
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
    /// `settings` : le dossier des réglages de Capri, où un cerveau a pu être choisi. Les clés
    /// sont lues dans le trousseau du système.
    pub fn new(settings: Option<PathBuf>) -> Self {
        Self::with_keys(settings, keys::SystemKeychain::shared())
    }

    /// Comme [`Chat::new`], avec les clés rangées dans `keys`.
    pub fn with_keys(settings: Option<PathBuf>, keys: Arc<dyn KeyStore>) -> Self {
        Self {
            conversation: Conversation::new(MAX_TURNS),
            reply: None,
            speech: None,
            clock: SpeechClock { origin: Instant::now(), held_since: None, held_total: Duration::ZERO },
            settings,
            keys,
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
            let keys = Arc::clone(&self.keys);
            let (turns, cancelled) = (self.conversation.turns().to_vec(), Arc::clone(&cancelled));
            move || think(settings.as_deref(), &*keys, &client, &turns, &cancelled, &sender)
        };
        self.clock.resume();
        self.speech = Some(Speech::new(self.clock.now_ms()));
        match std::thread::Builder::new().name("capri-cerveau".to_owned()).spawn(brain) {
            Ok(_) => self.reply = Some(Reply { events, cancelled, origin: Origin::Conversation }),
            Err(err) => {
                log::warn!("impossible de lancer la réponse : {err}");
                self.end_reply(Some("Je n'arrive pas à répondre."));
            }
        }
    }

    /// Elle dit la ligne que Claude Code a écrite pour elle (le miroir) : dans sa bulle, et à
    /// voix haute si sa voix est activée, comme une réponse. Ce qu'elle disait s'arrête. La ligne
    /// n'entre pas dans la conversation, et n'est jamais écrite dans le journal.
    pub fn relay(&mut self, line: &str) {
        // nettoyée comme ses réponses : du Markdown n'a de sens ni dans une bulle ni à l'oral
        let text = plain_text(line, true);
        if text.trim().is_empty() {
            return;
        }
        self.hush();
        let (sender, events) = channel();
        let settings = self.settings.clone();
        let job = move || {
            announce_voice(settings.as_deref(), &sender);
            let _ = sender.send(Thought::Reply(ReplyEvent::Delta(text)));
            let _ = sender.send(Thought::Reply(ReplyEvent::Done));
        };
        self.clock.resume();
        self.speech = Some(Speech::new(self.clock.now_ms()));
        match std::thread::Builder::new().name("capri-claude-dit".to_owned()).spawn(job) {
            Ok(_) => {
                let cancelled = Arc::new(AtomicBool::new(false));
                self.reply = Some(Reply { events, cancelled, origin: Origin::ClaudeCode });
            }
            Err(err) => {
                log::warn!("impossible de dire la phrase de Claude Code : {err}");
                self.speech = None;
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

    /// Parle-t-elle, de quelque façon que ce soit : une réponse attendue, sa bulle, ou sa voix
    /// qui n'a pas fini ? Ce qui doit attendre son tour attend que ce soit fini.
    pub fn is_busy(&self) -> bool {
        self.reply.is_some() || self.speech.is_some() || self.voice.as_ref().is_some_and(Voice::is_speaking)
    }

    /// Prend en compte ce que le cerveau a envoyé depuis le dernier appel.
    pub fn pump(&mut self) {
        loop {
            let Some(reply) = &self.reply else { return };
            match reply.events.try_recv() {
                Ok(Thought::Aloud(delivery)) => {
                    if self.voice.is_none() {
                        self.voice = Voice::new();
                    }
                    self.aloud = Some(Aloud { delivery, sentences: Sentences::new() });
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
                voice.say(&spoken, &aloud.delivery);
            }
        }
    }

    /// La réponse est complète, ou s'arrête sur un problème à dire à l'utilisateur. Seule une
    /// réponse à l'utilisateur entre dans la conversation.
    fn end_reply(&mut self, problem: Option<&str>) {
        let origin = self.reply.take().map_or(Origin::Conversation, |reply| reply.origin);
        let now_ms = self.clock.now_ms();
        let Some(speech) = &mut self.speech else { return };
        let in_conversation = origin == Origin::Conversation;
        if let Some(problem) = problem {
            if !speech.text().trim().is_empty() {
                speech.push(" — ");
            }
            speech.push(problem);
            // la question n'a pas eu sa réponse : reposée, elle ne doit pas arriver en double
            if in_conversation {
                self.conversation.retract_unanswered();
            }
        } else if in_conversation {
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
    keys: &dyn KeyStore,
    client: &Client,
    turns: &[Turn],
    cancelled: &AtomicBool,
    sender: &Sender<Thought>,
) {
    // si personne n'écoute plus, `cancelled` est déjà levé : la réponse va s'arrêter d'elle-même
    let mut emit = |event: ReplyEvent| {
        let _ = sender.send(Thought::Reply(event));
    };
    announce_voice(settings, sender);
    let setup = match settings.map(brain_setup::load).transpose() {
        Ok(setup) => setup.flatten(),
        Err(err) => {
            log::error!("cerveau : {err:#}");
            let problem = format!("Mes réglages sont illisibles : {}", Place::Bubble.where_to_see_why());
            return emit(ReplyEvent::Failed(problem));
        }
    };
    let Some(setup) = setup else { return repeat(turns, cancelled, &mut emit) };
    let provider = setup.brain.provider();
    let key = match keys::key_for(keys, provider, setup.brain.endpoint()) {
        Ok(key) => key,
        Err(err) => {
            log::error!("clé de {} : {err:#}", provider.name());
            let problem = format!(
                "Je n'ai pas pu lire ma clé dans {} : {}",
                keys.name(),
                Place::Bubble.where_to_see_why()
            );
            return emit(ReplyEvent::Failed(problem));
        }
    };
    log::debug!(
        "réponse demandée à {} ({}, {})",
        setup.brain.model(),
        provider.name(),
        if key.is_some() { "avec sa clé" } else { "sans clé" }
    );
    // un modèle écrit volontiers du Markdown, qui n'a aucun sens dans une bulle ni à voix haute
    let mut plain = PlainStream::new();
    let ask =
        Ask { place: Place::Bubble, brain: &setup.brain, key: key.as_ref(), persona: &setup.persona, turns };
    client.answer(&ask, cancelled, &mut |event| plain.filter(event, &mut emit));
}

/// Si sa voix est activée, l'annonce, avec la voix choisie et sa façon de parler : elles doivent
/// être connues avant le premier mot. Lit les réglages et les voix installées : hors du thread
/// principal.
fn announce_voice(settings: Option<&Path>, sender: &Sender<Thought>) {
    match settings.map(voice_setup::load) {
        Some(Ok(voice)) if voice.enabled => {
            let _ = sender.send(Thought::Aloud(voice_setup::delivery(&voice)));
        }
        Some(Err(err)) => log::warn!("pas de voix : {err:#}"),
        _ => {}
    }
}

/// Le cerveau de secours : il répète le dernier message, mot à mot.
fn repeat(turns: &[Turn], cancelled: &AtomicBool, emit: &mut dyn FnMut(ReplyEvent)) {
    let message = turns.last().map_or("", |turn| turn.text.as_str());
    let answer = echo::reply_to(Place::Bubble, message);
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
        settings_with(name, capri_chat::Provider::Ollama, address)
    }

    /// Pareil, avec ce fournisseur.
    fn settings_with(name: &str, provider: capri_chat::Provider, address: std::net::SocketAddr) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("capri-test-chat-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let brain = capri_chat::Brain::new(provider, Some(&format!("http://{address}/v1")), "modele-de-test")
            .expect("adresse locale");
        brain_setup::save(&directory, &brain).expect("réglages écrits");
        directory
    }

    use crate::fake_server::{self, header};

    /// Un faux fournisseur : il répond « Réponse 1 », « Réponse 2 »… à chaque requête.
    fn fake_provider(replies: usize) -> (std::net::SocketAddr, Receiver<fake_server::Request>) {
        fake_server::serve(
            (1..=replies).map(|number| fake_server::reply(&format!("Réponse {number}"))).collect(),
        )
    }

    /// Le corps d'une requête reçue, en JSON.
    fn body_of(request: fake_server::Request) -> serde_json::Value {
        serde_json::from_str(&request.1).expect("une requête en JSON")
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

        let first = body_of(requests.recv_timeout(Duration::from_secs(5)).expect("première requête"));
        assert_eq!(first["model"], "modele-de-test");
        assert_eq!(first["messages"][0]["role"], "system");
        assert_eq!(first["messages"][0]["content"], capri_chat::persona::DEFAULT);
        assert_eq!(first["messages"][1], serde_json::json!({ "role": "user", "content": "Bonjour" }));
        assert_eq!(first["messages"].as_array().map(Vec::len), Some(2));
        let second = body_of(requests.recv_timeout(Duration::from_secs(5)).expect("seconde requête"));
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

    /// Une clé qui n'existe nulle part ailleurs que dans ces tests.
    const KEY: &str = "sk-test-capri-chat-0123456789";

    #[test]
    fn with_a_key_she_sends_it_and_a_refused_key_says_how_to_put_it_back() {
        let refusal = format!(r#"{{"error":{{"message":"bad key {KEY}"}}}}"#);
        // une réponse, puis un refus de la clé
        let (address, requests) = fake_server::serve(vec![
            fake_server::reply("Oui."),
            fake_server::response("401 Unauthorized", "application/json", &refusal),
        ]);
        let directory = settings_with("cle-envoyee", capri_chat::Provider::Custom, address);
        let url = format!("http://{address}/v1");
        let keys = Arc::new(keys::memory::MemoryKeys::with(capri_chat::Provider::Custom, &url, KEY));
        let mut chat = Chat::with_keys(Some(directory.clone()), keys);

        chat.say("Tu m'entends ?");
        assert_eq!(listen(&mut chat), "Oui.");
        let (head, body) = requests.recv_timeout(Duration::from_secs(5)).expect("une requête");
        assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));
        assert!(!body.contains(KEY), "la clé ne part que dans son en-tête");

        chat.say("Et maintenant ?");
        let answer = listen(&mut chat);
        assert_eq!(
            answer,
            format!(
                "{address} refuse ma clé (401) : bad key [clé masquée]. Pour la remettre : ouvre Réglages… dans mon menu."
            )
        );
        assert!(!answer.contains(KEY));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn without_its_key_she_says_so_and_contacts_nobody() {
        let (address, requests) = fake_provider(1);
        // aucune clé pour OpenRouter
        let directory = settings_with("cle-absente", capri_chat::Provider::OpenRouter, address);
        let mut chat =
            Chat::with_keys(Some(directory.clone()), Arc::new(keys::memory::MemoryKeys::default()));
        chat.say("Coucou");
        assert_eq!(
            listen(&mut chat),
            format!(
                "OpenRouter demande une clé, et je n'en ai pas pour {address}. Pour me la donner : ouvre Réglages… dans mon menu."
            )
        );

        // un trousseau qui refuse l'accès
        let denied = keys::memory::MemoryKeys::denied();
        let mut chat = Chat::with_keys(Some(directory.clone()), Arc::new(denied));
        chat.say("Coucou");
        assert_eq!(
            listen(&mut chat),
            "Je n'ai pas pu lire ma clé dans le trousseau de test : Réglages… dans mon menu te dira pourquoi."
        );
        assert!(requests.recv_timeout(Duration::from_millis(100)).is_err(), "personne n'a été contacté");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_key_given_for_another_address_or_without_one_never_leaves() {
        use capri_chat::Provider;
        // la clé est donnée pour un autre port de cette machine, et une ancienne clé, sans
        // adresse, traîne encore dans le trousseau
        let (given_for, given_requests) = fake_provider(1);
        let (address, requests) = fake_provider(2);
        let keys = keys::memory::MemoryKeys::with(Provider::Custom, &format!("http://{given_for}/v1"), KEY);
        keys.add_legacy(Provider::Custom, KEY);
        keys.add(Provider::OpenRouter, "https://openrouter.ai/api/v1", KEY);
        keys.add_legacy(Provider::OpenRouter, KEY);
        let keys = Arc::new(keys);

        let directory = settings_with("cle-autre-adresse", Provider::Custom, address);
        let mut chat = Chat::with_keys(Some(directory.clone()), keys.clone());
        chat.say("Tu m'entends ?");
        assert_eq!(listen(&mut chat), "Réponse 1");
        let (head, body) = requests.recv_timeout(Duration::from_secs(5)).expect("une requête");
        assert_eq!(header(&head, "authorization"), None, "{head}");
        assert!(!head.contains(KEY) && !body.contains(KEY));

        // OpenRouter joint ailleurs que chez lui n'emporte pas la clé de son adresse habituelle
        let directory_openrouter = settings_with("cle-adresse-habituelle", Provider::OpenRouter, address);
        let mut chat = Chat::with_keys(Some(directory_openrouter.clone()), keys);
        chat.say("Et là ?");
        assert_eq!(
            listen(&mut chat),
            format!(
                "OpenRouter demande une clé, et je n'en ai pas pour {address}. Pour me la donner : ouvre Réglages… dans mon menu."
            )
        );
        assert!(requests.recv_timeout(Duration::from_millis(100)).is_err(), "personne n'a été contacté");
        assert!(
            given_requests.recv_timeout(Duration::from_millis(100)).is_err(),
            "ni là où la clé a été donnée"
        );
        let _ = std::fs::remove_dir_all(directory);
        let _ = std::fs::remove_dir_all(directory_openrouter);
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
    fn she_says_the_line_from_claude_code_without_keeping_it_in_the_conversation() {
        let mut chat = Chat::new(None);
        chat.relay("**C'est réparé**, et `tout` passe.");
        assert!(chat.is_speaking() && chat.is_busy());
        assert_eq!(listen(&mut chat), "C'est réparé, et tout passe.", "sans Markdown, comme ses réponses");
        assert_eq!(chat.turns(), 0, "ce n'est pas une prise de parole de la conversation");
        let (page, _) = chat.page().expect("la bulle reste le temps d'être lue");
        assert!(page.starts_with("C'est réparé"));

        // « La faire taire » la coupe
        chat.relay("Encore une.");
        chat.hush();
        assert!(!chat.is_speaking() && !chat.is_busy() && chat.page().is_none());
        // une ligne vide ne dit rien
        chat.relay("  ** ` ");
        assert!(!chat.is_busy());
    }

    #[test]
    fn what_she_said_for_claude_code_never_reaches_her_brain() {
        let (address, requests) = fake_provider(1);
        let directory = settings_with_a_brain_at("ligne-de-claude", address);
        let mut chat = Chat::new(Some(directory.clone()));
        chat.relay("Tous les tests passent.");
        assert_eq!(listen(&mut chat), "Tous les tests passent.");
        assert!(requests.recv_timeout(Duration::from_millis(100)).is_err(), "son cerveau n'est pas contacté");

        // l'utilisateur lui parle : ce qu'elle disait s'arrête, et sa phrase ne part pas avec
        chat.relay("Une autre phrase de Claude.");
        chat.say("Bonjour");
        assert_eq!(listen(&mut chat), "Réponse 1");
        let request = body_of(requests.recv_timeout(Duration::from_secs(5)).expect("une requête"));
        let messages = request["messages"].as_array().expect("des messages");
        assert_eq!(messages.len(), 2, "{messages:?}");
        assert_eq!(messages[1], serde_json::json!({ "role": "user", "content": "Bonjour" }));
        assert!(!request.to_string().contains("tests passent") && !request.to_string().contains("Claude"));
        assert_eq!(chat.turns(), 2);
        let _ = std::fs::remove_dir_all(directory);
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
