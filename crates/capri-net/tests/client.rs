//! Le client face à un faux serveur, sur cette machine : ce qui part, ce qui est lu, et ce qui
//! se passe quand le serveur se tait, refuse, raccroche ou n'en finit pas.
//!
//! Aucun test ne sort de la machine.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use capri_chat::{ApiKey, BoundKey, Brain, KeyAccount, Place, Provider, ReplyEvent, Role, Turn};
use capri_net::{Ask, Client, Limits, Problem};
use serde_json::{Value, json};

const STREAM_HEAD: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n";
const LAST_CHUNK: &[u8] = b"0\r\n\r\n";
const DONE: &[u8] = b"data: [DONE]\n\n";

/// Ce que le faux serveur fait, une fois la requête lue. Au bout du script, il raccroche.
enum Step {
    Send(Vec<u8>),
    Wait(Duration),
    /// Envoie ces octets à ce rythme jusqu'à ce que le client raccroche (dix secondes au plus).
    Drip(Vec<u8>, Duration),
}

struct Server {
    address: SocketAddr,
    /// La requête reçue : ses en-têtes, puis son corps.
    request: Receiver<(String, String)>,
    /// Le client a-t-il raccroché pendant un [`Step::Drip`] ?
    hung_up: Arc<AtomicBool>,
}

impl Server {
    fn brain(&self) -> Brain {
        brain_at(self.address)
    }

    fn request(&self) -> (String, String) {
        self.request.recv_timeout(Duration::from_secs(5)).expect("le serveur a reçu une requête")
    }

    fn wait_for_hang_up(&self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !self.hung_up.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.hung_up.load(Ordering::Relaxed)
    }
}

fn brain_at(address: SocketAddr) -> Brain {
    Brain::new(Provider::Ollama, Some(&format!("http://{address}/v1")), "modele-de-test")
        .expect("adresse locale")
}

/// Lance un serveur qui sert une seule connexion en suivant `steps`.
fn serve(steps: Vec<Step>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let address = listener.local_addr().expect("l'adresse du serveur");
    let (sender, request) = channel();
    let hung_up = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&hung_up);
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let Some(received) = read_request(&mut stream) else { return };
        let _ = sender.send(received);
        for step in steps {
            match step {
                Step::Send(bytes) => {
                    if stream.write_all(&bytes).and_then(|()| stream.flush()).is_err() {
                        return;
                    }
                }
                Step::Wait(pause) => std::thread::sleep(pause),
                Step::Drip(bytes, pace) => {
                    let deadline = Instant::now() + Duration::from_secs(10);
                    while Instant::now() < deadline {
                        if stream.write_all(&bytes).and_then(|()| stream.flush()).is_err() {
                            flag.store(true, Ordering::Relaxed);
                            return;
                        }
                        std::thread::sleep(pace);
                    }
                    return;
                }
            }
        }
    });
    Server { address, request, hung_up }
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut read_more = |bytes: &mut Vec<u8>| -> Option<()> {
        let received = stream.read(&mut buffer).ok().filter(|&received| received > 0)?;
        bytes.extend_from_slice(&buffer[..received]);
        Some(())
    };
    let head_end = loop {
        if let Some(at) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        read_more(&mut bytes)?;
    };
    let head = String::from_utf8_lossy(&bytes[..head_end]).into_owned();
    let length = header(&head, "content-length").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
    while bytes.len() < head_end + length {
        read_more(&mut bytes)?;
    }
    Some((head, String::from_utf8_lossy(&bytes[head_end..head_end + length]).into_owned()))
}

/// La valeur d'un en-tête de la requête, quelle que soit sa casse.
fn header(head: &str, name: &str) -> Option<String> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
    })
}

/// Un morceau du codage « chunked » de HTTP.
fn chunk(payload: &[u8]) -> Vec<u8> {
    let mut bytes = format!("{:x}\r\n", payload.len()).into_bytes();
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(b"\r\n");
    bytes
}

/// Un événement du flux qui apporte ce texte.
fn delta(text: &str) -> Vec<u8> {
    let event = json!({ "choices": [{ "index": 0, "delta": { "content": text }, "finish_reason": null }] });
    format!("data: {event}\n\n").into_bytes()
}

/// Une réponse complète, de longueur annoncée.
fn plain(status: &str, content_type: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// Une demande dont la panne s'affichera dans la bulle.
fn bubble<'a>(brain: &'a Brain, key: Option<&'a BoundKey>, persona: &'a str, turns: &'a [Turn]) -> Ask<'a> {
    Ask { place: Place::Bubble, brain, key, persona, turns }
}

fn conversation() -> Vec<Turn> {
    vec![
        Turn { role: Role::User, text: "Salut".to_owned() },
        Turn { role: Role::Capri, text: "Salut !".to_owned() },
        Turn { role: Role::User, text: "Ça va ?".to_owned() },
    ]
}

/// Pose la question et rend ce qui a été émis, avec le temps que cela a pris.
fn ask(client: &Client, brain: &Brain) -> (Vec<ReplyEvent>, Duration) {
    let started = Instant::now();
    let mut events = Vec::new();
    client.answer(
        &bubble(brain, None, "Tu es Capri.", &conversation()),
        &AtomicBool::new(false),
        &mut |event| events.push(event),
    );
    (events, started.elapsed())
}

fn said(text: &str) -> ReplyEvent {
    ReplyEvent::Delta(text.to_owned())
}

fn failed(reason: &str) -> ReplyEvent {
    ReplyEvent::Failed(reason.to_owned())
}

#[test]
fn a_reply_arrives_piece_by_piece_and_the_request_is_what_the_provider_expects() {
    // un événement coupé entre deux envois, au milieu d'un caractère accentué
    let second = delta(" toi, ça va ?");
    let cut = second.iter().position(|&byte| byte == 0xC3).expect("un « ç »") + 1;
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Salut"))),
        Step::Wait(Duration::from_millis(20)),
        Step::Send(chunk(&second[..cut])),
        Step::Wait(Duration::from_millis(20)),
        Step::Send(chunk(&[&second[cut..], &delta(" 🙂")[..]].concat())),
        Step::Send(chunk(DONE)),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [said("Salut"), said(" toi, ça va ?"), said(" 🙂"), ReplyEvent::Done]);

    let (head, body) = server.request();
    assert!(head.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"), "{head}");
    assert_eq!(header(&head, "host"), Some(server.address.to_string()));
    assert_eq!(header(&head, "content-type").as_deref(), Some("application/json"));
    assert_eq!(header(&head, "accept").as_deref(), Some("text/event-stream"));
    for absent in ["authorization", "cookie", "x-api-key", "proxy-authorization"] {
        assert_eq!(header(&head, absent), None, "{head}");
    }
    let body: Value = serde_json::from_str(&body).expect("un corps en JSON");
    assert_eq!(
        body,
        json!({
            "model": "modele-de-test",
            "stream": true,
            "messages": [
                { "role": "system", "content": "Tu es Capri." },
                { "role": "user", "content": "Salut" },
                { "role": "assistant", "content": "Salut !" },
                { "role": "user", "content": "Ça va ?" },
            ],
        }),
        "rien d'autre ne part que la personnalité et la conversation"
    );
}

#[test]
fn she_can_be_cut_off_and_the_server_is_told() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Drip(chunk(&delta("mot ")), Duration::from_millis(10)),
    ]);
    let cancelled = AtomicBool::new(false);
    let mut events = Vec::new();
    let started = Instant::now();
    Client::new().answer(&bubble(&server.brain(), None, "", &conversation()), &cancelled, &mut |event| {
        events.push(event);
        if events.len() == 3 {
            cancelled.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(
        events,
        [said("mot "), said("mot "), said("mot ")],
        "plus rien après l'arrêt, pas même la fin"
    );
    assert!(started.elapsed() < Duration::from_secs(2), "l'arrêt est immédiat : {:?}", started.elapsed());
    assert!(server.wait_for_hang_up(), "la connexion est fermée : le serveur cesse d'écrire");
}

#[test]
fn a_cancel_before_the_first_word_stops_at_the_server_s_first_sign() {
    // le serveur charge son modèle : il ne répond qu'au bout d'un moment
    let server = serve(vec![
        Step::Wait(Duration::from_millis(300)),
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Drip(chunk(&delta("mot ")), Duration::from_millis(10)),
    ]);
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        flag.store(true, Ordering::Relaxed);
    });
    let mut events = Vec::new();
    let started = Instant::now();
    Client::new().answer(&bubble(&server.brain(), None, "", &conversation()), &cancelled, &mut |event| {
        events.push(event)
    });
    assert_eq!(events, [], "rien n'est émis");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    assert!(server.wait_for_hang_up());
}

#[test]
fn a_refusal_is_explained_with_the_provider_s_own_words() {
    let error = r#"{"error":{"message":"model \"modele-de-test\" not found, try pulling it first","type":"not_found_error","param":null,"code":null}}"#;
    let server = serve(vec![Step::Send(plain("404 Not Found", "application/json", error))]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(
        events,
        [failed("Ollama refuse (404) : model \"modele-de-test\" not found, try pulling it first")]
    );

    let server = serve(vec![Step::Send(plain("502 Bad Gateway", "text/html", "<html>502</html>"))]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [failed("Ollama refuse (502).")]);
}

#[test]
fn nobody_listening_is_said_plainly() {
    let address =
        TcpListener::bind("127.0.0.1:0").and_then(|listener| listener.local_addr()).expect("un port");
    // le port vient d'être libéré : personne n'y écoute
    let (events, elapsed) = ask(&Client::new(), &brain_at(address));
    assert_eq!(events, [failed(&format!("Je n'arrive pas à joindre Ollama ({address}). Il est lancé ?"))]);
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    assert_eq!(
        Client::new().models(Provider::Ollama, brain_at(address).endpoint(), None),
        Err(Problem::Unreachable)
    );
}

#[test]
fn a_reply_cut_short_keeps_what_arrived_and_says_so() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Je commence"))),
        Step::Wait(Duration::from_millis(30)),
        // fin du script : le serveur raccroche sans terminer sa réponse
    ]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [said("Je commence"), failed("La réponse a été coupée.")]);
}

#[test]
fn a_provider_that_gives_up_midway_is_quoted() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Je"))),
        Step::Send(chunk(
            b"data: {\"error\":{\"code\":\"server_error\",\"message\":\"Provider disconnected\"}}\n\n",
        )),
        Step::Send(chunk(&delta(" continue quand même"))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [said("Je"), failed("Ollama a abandonné : Provider disconnected")]);
}

#[test]
fn a_stream_that_ends_without_saying_done_is_a_complete_reply() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Fini."))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [said("Fini."), ReplyEvent::Done]);
}

#[test]
fn the_reply_ends_with_its_finish_reason_even_if_the_server_lingers() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Voilà."))),
        Step::Send(chunk(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n")),
        Step::Wait(Duration::from_secs(5)),
    ]);
    let (events, elapsed) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [said("Voilà."), ReplyEvent::Done]);
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn a_redirect_is_a_refusal_and_is_never_followed() {
    let elsewhere = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    elsewhere.set_nonblocking(true).expect("écoute sans attente");
    let target = elsewhere.local_addr().expect("adresse");
    let redirect = format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target}/v1/chat/completions\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let server = serve(vec![Step::Send(redirect.into_bytes())]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [failed("Ollama refuse (307).")]);
    std::thread::sleep(Duration::from_millis(100));
    let knocked = elsewhere.accept();
    assert!(
        matches!(&knocked, Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
        "personne n'a contacté l'autre adresse : {knocked:?}"
    );
}

/// Sans TLS sur ce système, une adresse en https est refusée avant toute connexion (et même avant
/// de chercher le nom : `.invalid` ne se résout jamais, mais on ne le demande même pas).
#[cfg(not(any(target_os = "macos", windows)))]
#[test]
fn https_is_refused_before_any_connection() {
    let brain =
        Brain::new(Provider::Ollama, Some("https://example.invalid/v1"), "m").expect("adresse valide");
    let (events, elapsed) = ask(&Client::new(), &brain);
    assert_eq!(events, [failed("Je ne sais pas me connecter en https sur ce système.")]);
    assert!(elapsed < Duration::from_millis(500), "aucune tentative : {elapsed:?}");
    assert_eq!(Client::new().models(Provider::Ollama, brain.endpoint(), None), Err(Problem::NoTls));
}

/// Un serveur qui répond en clair à une adresse en https : la connexion sûre échoue, et rien de
/// la conversation n'est parti. Le client n'a envoyé que le début d'une négociation TLS.
#[cfg(any(target_os = "macos", windows))]
#[test]
fn https_never_falls_back_to_plain_text() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let address = listener.local_addr().expect("adresse");
    let (sender, received) = channel();
    std::thread::spawn(move || {
        for _ in 0..2 {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let mut bytes = vec![0u8; 16 * 1024];
            let length = stream.read(&mut bytes).unwrap_or(0);
            bytes.truncate(length);
            let _ = stream.write_all(&plain("200 OK", "text/event-stream", "data: [DONE]\n\n"));
            let _ = sender.send(bytes);
        }
    });
    let brain = Brain::new(Provider::Ollama, Some(&format!("https://{address}/v1")), "modele-de-test")
        .expect("adresse valide");
    let (events, elapsed) = ask(&Client::new(), &brain);
    assert_eq!(
        events,
        [failed(&format!(
            "Je n'ai pas pu établir de connexion sûre avec Ollama ({address}) : je ne lui ai rien envoyé."
        ))]
    );
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
    assert_eq!(Client::new().models(Provider::Ollama, brain.endpoint(), None), Err(Problem::Insecure));

    for _ in 0..2 {
        let bytes = received.recv_timeout(Duration::from_secs(5)).expect("le serveur a été contacté");
        assert_eq!(bytes.first(), Some(&0x16), "un message de négociation TLS, rien d'autre");
        let seen = String::from_utf8_lossy(&bytes);
        for absent in ["POST", "GET", "Salut", "Tu es Capri", "modele-de-test"] {
            assert!(!seen.contains(absent), "« {absent} » est parti en clair");
        }
    }
}

#[test]
fn a_server_gone_silent_is_given_up_on() {
    let limits = Limits { silence: Duration::from_millis(300), ..Limits::default() };
    // silencieux après un premier mot
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Un mot"))),
        Step::Wait(Duration::from_secs(4)),
    ]);
    let (events, elapsed) = ask(&Client::with_limits(limits), &server.brain());
    assert_eq!(events, [said("Un mot"), failed("Ollama met trop de temps à répondre.")]);
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");

    // silencieux d'entrée
    let server = serve(vec![Step::Wait(Duration::from_secs(4))]);
    let (events, elapsed) = ask(&Client::with_limits(limits), &server.brain());
    assert_eq!(events, [failed("Ollama met trop de temps à répondre.")]);
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn a_pause_shorter_than_the_limit_does_not_end_the_reply() {
    let limits = Limits { silence: Duration::from_millis(600), ..Limits::default() };
    let server = serve(vec![
        Step::Wait(Duration::from_millis(250)),
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Un"))),
        Step::Wait(Duration::from_millis(250)),
        Step::Send(chunk(&delta(" deux"))),
        Step::Wait(Duration::from_millis(250)),
        Step::Send(chunk(&delta(" trois"))),
        Step::Send(chunk(DONE)),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let (events, elapsed) = ask(&Client::with_limits(limits), &server.brain());
    assert_eq!(events, [said("Un"), said(" deux"), said(" trois"), ReplyEvent::Done]);
    assert!(elapsed >= Duration::from_millis(700), "la limite vaut pour chaque silence, pas pour le tout");
}

#[test]
fn an_endless_reply_is_stopped() {
    let limits = Limits { reply_chars: 2_000, ..Limits::default() };
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Drip(chunk(&delta(&"x".repeat(500))), Duration::from_millis(1)),
    ]);
    let (events, elapsed) = ask(&Client::with_limits(limits), &server.brain());
    assert_eq!(events.len(), 5, "quatre morceaux de 500 caractères, puis la fin");
    assert_eq!(events.last(), Some(&ReplyEvent::Done));
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    assert!(server.wait_for_hang_up());
}

#[test]
fn an_endless_event_is_refused() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(b"data: ")),
        Step::Drip(chunk(&[b'x'; 16 * 1024]), Duration::from_millis(1)),
    ]);
    let (events, elapsed) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [failed("Ollama a répondu quelque chose que je n'ai pas compris.")]);
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    assert!(server.wait_for_hang_up());
}

#[test]
fn a_page_that_is_not_a_stream_is_not_a_reply() {
    let server = serve(vec![Step::Send(plain("200 OK", "text/html", "<html><body>Bonjour</body></html>"))]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [failed("Ollama a répondu quelque chose que je n'ai pas compris.")]);

    // un flux bien formé, mais où le modèle ne dit rien
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(b"data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n")),
        Step::Send(chunk(DONE)),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let (events, _) = ask(&Client::new(), &server.brain());
    assert_eq!(events, [failed("Ollama n'a rien répondu.")]);
}

#[test]
fn lists_the_models_the_server_offers() {
    let listing = r#"{"object":"list","data":[{"id":"un-modele:8b","object":"model"},{"id":"un-autre:4b","object":"model"}]}"#;
    let server = serve(vec![Step::Send(plain("200 OK", "application/json", listing))]);
    let models = Client::new().models(Provider::Ollama, server.brain().endpoint(), None);
    assert_eq!(models, Ok(vec!["un-modele:8b".to_owned(), "un-autre:4b".to_owned()]));
    let (head, body) = server.request();
    assert!(head.starts_with("GET /v1/models HTTP/1.1\r\n"), "{head}");
    assert_eq!(body, "");

    let server = serve(vec![Step::Send(plain("200 OK", "text/html", "<html>autre chose</html>"))]);
    assert_eq!(
        Client::new().models(Provider::Ollama, server.brain().endpoint(), None),
        Err(Problem::Unreadable)
    );
    let server = serve(vec![Step::Send(plain(
        "500 Internal Server Error",
        "application/json",
        r#"{"error":"en panne"}"#,
    ))]);
    assert_eq!(
        Client::new().models(Provider::Ollama, server.brain().endpoint(), None),
        Err(Problem::Refused { status: 500, reason: Some("en panne".to_owned()) })
    );
}

// --- La clé d'API ---

/// Une clé qui n'existe nulle part ailleurs que dans ces tests.
const KEY: &str = "sk-test-capri-0123456789abcdef";

/// La clé de test, donnée pour l'adresse de ce cerveau.
fn key_for(brain: &Brain) -> BoundKey {
    BoundKey::new(KeyAccount::new(brain.provider(), brain.endpoint()), ApiKey::parse(KEY).expect("une clé"))
}

/// Un cerveau du fournisseur `provider`, joint sur le faux serveur.
fn provider_at(provider: Provider, address: SocketAddr) -> Brain {
    Brain::new(provider, Some(&format!("http://{address}/v1")), "modele-de-test").expect("adresse locale")
}

/// Pose la question avec cette clé, et rend ce qui a été émis.
fn ask_with(brain: &Brain, key: Option<&BoundKey>) -> Vec<ReplyEvent> {
    let mut events = Vec::new();
    Client::new().answer(
        &bubble(brain, key, "Tu es Capri.", &conversation()),
        &AtomicBool::new(false),
        &mut |event| events.push(event),
    );
    events
}

fn a_short_reply() -> Vec<Step> {
    vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Salut."))),
        Step::Send(chunk(DONE)),
        Step::Send(LAST_CHUNK.to_vec()),
    ]
}

#[test]
fn the_key_travels_as_a_bearer_token_in_its_header_and_nowhere_else() {
    let server = serve(a_short_reply());
    let brain = provider_at(Provider::OpenRouter, server.address);
    assert_eq!(ask_with(&brain, Some(&key_for(&brain))), [said("Salut."), ReplyEvent::Done]);

    let (head, body) = server.request();
    assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));
    assert_eq!(head.matches(KEY).count(), 1, "une seule fois, dans son en-tête : {head}");
    assert!(!head.to_ascii_lowercase().contains("x-api-key"), "{head}");
    assert!(!body.contains(KEY), "jamais dans le corps : {body}");
    // aucun en-tête qui présente l'app au fournisseur
    for absent in ["http-referer", "referer", "x-title", "x-openrouter-title"] {
        assert_eq!(header(&head, absent), None, "{head}");
    }
    // le seul qui nomme un logiciel : celui de ureq, la bibliothèque HTTP, que ureq ajoute de
    // lui-même (`ureq/<version>`) ; il ne dit rien de Capri
    let agent = header(&head, "user-agent").unwrap_or_default();
    assert!(agent.starts_with("ureq/") && !agent.to_lowercase().contains("capri"), "{head}");
}

#[test]
fn without_a_key_no_authorization_header_is_sent() {
    // un serveur qui peut se passer de clé, et n'en a pas
    let server = serve(a_short_reply());
    assert_eq!(
        ask_with(&provider_at(Provider::LmStudio, server.address), None),
        [said("Salut."), ReplyEvent::Done]
    );
    let (head, _) = server.request();
    assert_eq!(header(&head, "authorization"), None, "{head}");

    // Ollama ne prend pas de clé : il n'en reçoit jamais, même si on lui en confie une
    let server = serve(a_short_reply());
    let ollama = provider_at(Provider::Ollama, server.address);
    assert_eq!(ask_with(&ollama, Some(&key_for(&ollama))), [said("Salut."), ReplyEvent::Done]);
    let (head, body) = server.request();
    assert_eq!(header(&head, "authorization"), None, "{head}");
    assert!(!head.contains(KEY) && !body.contains(KEY));
}

#[test]
fn a_provider_that_needs_a_key_is_not_contacted_without_one() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    listener.set_nonblocking(true).expect("écoute sans attente");
    let address = listener.local_addr().expect("adresse");
    for provider in [Provider::OpenRouter, Provider::DeepSeek, Provider::ProxyApi, Provider::Anthropic] {
        let brain = provider_at(provider, address);
        assert_eq!(
            ask_with(&brain, None),
            [failed(&format!(
                "{} demande une clé, et je n'en ai pas pour {address}. Pour me la donner : ouvre Réglages… dans mon menu.",
                provider.name()
            ))]
        );
        assert_eq!(Client::new().models(provider, brain.endpoint(), None), Err(Problem::NoKey));
    }
    std::thread::sleep(Duration::from_millis(50));
    let knocked = listener.accept();
    assert!(
        matches!(&knocked, Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
        "personne n'a été contacté : {knocked:?}"
    );
}

#[test]
fn a_refused_key_says_how_to_put_it_back_and_never_quotes_it() {
    // le fournisseur répète la clé dans son refus
    let error = format!(r#"{{"error":{{"message":"Incorrect API key provided: {KEY}","code":401}}}}"#);
    let server = serve(vec![Step::Send(plain("401 Unauthorized", "application/json", &error))]);
    let brain = provider_at(Provider::OpenRouter, server.address);
    let events = ask_with(&brain, Some(&key_for(&brain)));
    assert_eq!(
        events,
        [failed(
            "OpenRouter refuse ma clé (401) : Incorrect API key provided: [clé masquée]. Pour la remettre : \
             ouvre Réglages… dans mon menu."
        )]
    );

    // un 403 est cité, puisqu'il peut dire autre chose qu'une mauvaise clé, mais masqué
    let error = format!(r#"{{"error":{{"message":"Key {KEY} may not use this model.","code":403}}}}"#);
    let server = serve(vec![Step::Send(plain("403 Forbidden", "application/json", &error))]);
    let brain = provider_at(Provider::ProxyApi, server.address);
    let events = ask_with(&brain, Some(&key_for(&brain)));
    assert_eq!(
        events,
        [failed(
            "ProxyAPI refuse (403) : Key [clé masquée] may not use this model. Si c'est ma clé, pour la remettre : ouvre Réglages… dans mon menu."
        )]
    );

    // même un morceau de la clé ne passe pas, ni dans un refus ordinaire
    let error = format!(r#"{{"detail":"trop de requêtes pour {}…","code":429}}"#, &KEY[..20]);
    let server = serve(vec![Step::Send(plain("429 Too Many Requests", "application/json", &error))]);
    let brain = provider_at(Provider::ProxyApi, server.address);
    let events = ask_with(&brain, Some(&key_for(&brain)));
    assert_eq!(events, [failed("ProxyAPI refuse (429) : trop de requêtes pour [clé masquée]…")]);
}

#[test]
fn a_server_that_wants_a_key_says_so() {
    let server = serve(vec![Step::Send(plain(
        "401 Unauthorized",
        "application/json",
        r#"{"error":"token required"}"#,
    ))]);
    let events = ask_with(&provider_at(Provider::LmStudio, server.address), None);
    assert_eq!(
        events,
        [failed(
            "LM Studio demande une clé (401) : token required. Pour me la donner : ouvre Réglages… dans mon menu."
        )]
    );
}

#[test]
fn a_key_repeated_in_the_middle_of_the_stream_is_masked() {
    let error = format!("data: {{\"error\":{{\"message\":\"session {KEY} closed\"}}}}\n\n");
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&delta("Je"))),
        Step::Send(chunk(error.as_bytes())),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let brain = provider_at(Provider::DeepSeek, server.address);
    let events = ask_with(&brain, Some(&key_for(&brain)));
    assert_eq!(events, [said("Je"), failed("DeepSeek a abandonné : session [clé masquée] closed")]);
}

#[test]
fn the_models_are_listed_with_the_key_and_a_refusal_is_explained() {
    let listing = r#"{"object":"list","data":[{"id":"fournisseur/un-modele","object":"model"}]}"#;
    let server = serve(vec![Step::Send(plain("200 OK", "application/json", listing))]);
    let brain = provider_at(Provider::DeepSeek, server.address);
    assert_eq!(
        Client::new().models(Provider::DeepSeek, brain.endpoint(), Some(&key_for(&brain))),
        Ok(vec!["fournisseur/un-modele".to_owned()])
    );
    let (head, _) = server.request();
    assert!(head.starts_with("GET /v1/models HTTP/1.1\r\n"), "{head}");
    assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));

    // la réponse de DeepSeek à une clé refusée : du texte brut, rien à citer
    let server =
        serve(vec![Step::Send(plain("401 Unauthorized", "text/plain", "Authentication Fails (governor)"))]);
    let brain = provider_at(Provider::DeepSeek, server.address);
    assert_eq!(
        Client::new().models(Provider::DeepSeek, brain.endpoint(), Some(&key_for(&brain))),
        Err(Problem::KeyRefused { status: 401, reason: None })
    );
}

/// Une clé donnée pour une adresse (ici, un port de cette machine) ne part jamais vers une autre :
/// elle y compte pour absente, que ce soit pour parler ou pour lister les modèles.
#[test]
fn a_key_given_for_another_address_is_never_sent() {
    let given_for = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    given_for.set_nonblocking(true).expect("écoute sans attente");
    let elsewhere = given_for.local_addr().expect("adresse");

    // un fournisseur à clé facultative : la requête part, sans clé
    let server = serve(a_short_reply());
    let brain = provider_at(Provider::LmStudio, server.address);
    let other_port = key_for(&provider_at(Provider::LmStudio, elsewhere));
    assert_eq!(ask_with(&brain, Some(&other_port)), [said("Salut."), ReplyEvent::Done]);
    let (head, body) = server.request();
    assert_eq!(header(&head, "authorization"), None, "{head}");
    assert!(!head.contains(KEY) && !body.contains(KEY));

    let server = serve(vec![Step::Send(plain("200 OK", "application/json", r#"{"data":[{"id":"m"}]}"#))]);
    let brain = provider_at(Provider::Custom, server.address);
    let other_port = key_for(&provider_at(Provider::Custom, elsewhere));
    assert_eq!(
        Client::new().models(Provider::Custom, brain.endpoint(), Some(&other_port)),
        Ok(vec!["m".to_owned()])
    );
    let (head, _) = server.request();
    assert_eq!(header(&head, "authorization"), None, "{head}");
    assert!(!head.contains(KEY));

    // la même adresse, mais un autre fournisseur : pas sa clé non plus
    let server = serve(a_short_reply());
    let brain = provider_at(Provider::Custom, server.address);
    let lmstudio_here = key_for(&provider_at(Provider::LmStudio, server.address));
    assert_eq!(ask_with(&brain, Some(&lmstudio_here)), [said("Salut."), ReplyEvent::Done]);
    let (head, _) = server.request();
    assert_eq!(header(&head, "authorization"), None, "{head}");

    // un fournisseur qui l'exige : personne n'est contacté, ni ici ni là où la clé a été donnée
    let unused = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    unused.set_nonblocking(true).expect("écoute sans attente");
    let address = unused.local_addr().expect("adresse");
    let brain = provider_at(Provider::OpenRouter, address);
    let other_port = key_for(&provider_at(Provider::OpenRouter, elsewhere));
    assert_eq!(
        ask_with(&brain, Some(&other_port)),
        [failed(&format!(
            "OpenRouter demande une clé, et je n'en ai pas pour {address}. Pour me la donner : ouvre Réglages… dans mon menu."
        ))]
    );
    assert_eq!(
        Client::new().models(Provider::OpenRouter, brain.endpoint(), Some(&other_port)),
        Err(Problem::NoKey)
    );
    std::thread::sleep(Duration::from_millis(50));
    for listener in [&unused, &given_for] {
        let knocked = listener.accept();
        assert!(
            matches!(&knocked, Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
            "personne n'a été contacté : {knocked:?}"
        );
    }
}

#[test]
fn a_redirect_never_takes_the_key_elsewhere() {
    let elsewhere = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    elsewhere.set_nonblocking(true).expect("écoute sans attente");
    let target = elsewhere.local_addr().expect("adresse");
    let redirect = format!(
        "HTTP/1.1 308 Permanent Redirect\r\nLocation: http://{target}/v1/chat/completions\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let server = serve(vec![Step::Send(redirect.into_bytes())]);
    let brain = provider_at(Provider::OpenRouter, server.address);
    let events = ask_with(&brain, Some(&key_for(&brain)));
    assert_eq!(events, [failed("OpenRouter refuse (308).")]);
    std::thread::sleep(Duration::from_millis(100));
    let knocked = elsewhere.accept();
    assert!(
        matches!(&knocked, Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
        "personne n'a contacté l'autre adresse : {knocked:?}"
    );
}

// --- L'API d'Anthropic ---

/// Un événement du flux d'Anthropic : son nom, et ses données.
fn event(name: &str, data: Value) -> Vec<u8> {
    format!("event: {name}\ndata: {data}\n\n").into_bytes()
}

fn text_delta(index: u32, text: &str) -> Vec<u8> {
    event(
        "content_block_delta",
        json!({ "type": "content_block_delta", "index": index, "delta": { "type": "text_delta", "text": text } }),
    )
}

fn message_start() -> Vec<u8> {
    event(
        "message_start",
        json!({ "type": "message_start", "message": { "id": "msg_test", "type": "message", "role": "assistant",
            "content": [], "model": "modele-de-test", "stop_reason": null, "stop_sequence": null,
            "usage": { "input_tokens": 25, "output_tokens": 1 } } }),
    )
}

/// Un bloc de réflexion tel que l'envoient les modèles récents par défaut : vide, signé.
fn thinking_block(index: u32) -> Vec<u8> {
    [
        event(
            "content_block_start",
            json!({ "type": "content_block_start", "index": index,
                "content_block": { "type": "thinking", "thinking": "", "signature": "" } }),
        ),
        event(
            "content_block_delta",
            json!({ "type": "content_block_delta", "index": index,
                "delta": { "type": "thinking_delta", "thinking": "" } }),
        ),
        event(
            "content_block_delta",
            json!({ "type": "content_block_delta", "index": index,
                "delta": { "type": "signature_delta", "signature": "EqQBCgIYAhIM1gbcDa9GJwZA" } }),
        ),
        event("content_block_stop", json!({ "type": "content_block_stop", "index": index })),
    ]
    .concat()
}

fn stop(reason: &str) -> Vec<u8> {
    [
        event(
            "message_delta",
            json!({ "type": "message_delta", "delta": { "stop_reason": reason, "stop_sequence": null },
                "usage": { "output_tokens": 15 } }),
        ),
        event("message_stop", json!({ "type": "message_stop" })),
    ]
    .concat()
}

fn anthropic_at(address: SocketAddr) -> Brain {
    provider_at(Provider::Anthropic, address)
}

fn declined() -> ReplyEvent {
    failed("Le modèle d'Anthropic a décliné cette demande. Essaie de la dire autrement.")
}

#[test]
fn anthropic_gets_its_own_request_and_only_the_text_of_its_answer_is_said() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&message_start())),
        Step::Send(chunk(&event("ping", json!({ "type": "ping" })))),
        Step::Send(chunk(&thinking_block(0))),
        Step::Send(chunk(&event(
            "content_block_start",
            json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "text", "text": "" } }),
        ))),
        Step::Send(chunk(&text_delta(1, "Salut"))),
        Step::Send(chunk(&event("un_evenement_futur", json!({ "type": "un_evenement_futur" })))),
        Step::Send(chunk(&text_delta(1, ", ça va ?"))),
        Step::Send(chunk(&event("content_block_stop", json!({ "type": "content_block_stop", "index": 1 })))),
        Step::Send(chunk(&stop("end_turn"))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let brain = anthropic_at(server.address);
    assert_eq!(
        ask_with(&brain, Some(&key_for(&brain))),
        [said("Salut"), said(", ça va ?"), ReplyEvent::Done]
    );

    let (head, body) = server.request();
    assert!(head.starts_with("POST /v1/messages HTTP/1.1\r\n"), "{head}");
    assert_eq!(header(&head, "x-api-key").as_deref(), Some(KEY), "la clé seule, dans son en-tête");
    assert_eq!(head.matches(KEY).count(), 1, "une seule fois : {head}");
    assert_eq!(header(&head, "authorization"), None, "{head}");
    assert_eq!(header(&head, "anthropic-version").as_deref(), Some("2026-10-21"));
    assert_eq!(header(&head, "content-type").as_deref(), Some("application/json"));
    for absent in ["anthropic-beta", "cookie", "http-referer", "referer", "x-title", "proxy-authorization"] {
        assert_eq!(header(&head, absent), None, "{head}");
    }
    let agent = header(&head, "user-agent").unwrap_or_default();
    assert!(agent.starts_with("ureq/") && !agent.to_lowercase().contains("capri"), "{head}");
    assert!(!body.contains(KEY), "jamais dans le corps");
    let body: Value = serde_json::from_str(&body).expect("un corps en JSON");
    assert_eq!(
        body,
        json!({
            "model": "modele-de-test",
            "max_tokens": capri_chat::anthropic::MAX_TOKENS,
            "stream": true,
            "system": "Tu es Capri.",
            "messages": [
                { "role": "user", "content": "Salut" },
                { "role": "assistant", "content": "Salut !" },
                { "role": "user", "content": "Ça va ?" },
            ],
        }),
        "rien d'autre ne part que la personnalité et la conversation"
    );
}

#[test]
fn a_refusal_from_anthropic_is_said_in_our_own_words_not_kept_silent() {
    // avant le moindre mot
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&message_start())),
        Step::Send(chunk(&event(
            "message_delta",
            json!({ "type": "message_delta", "delta": { "stop_reason": "refusal", "stop_sequence": null,
                "stop_details": { "type": "refusal", "category": "cyber", "explanation": "…" } },
                "usage": { "output_tokens": 0 } }),
        ))),
        Step::Send(chunk(&event("message_stop", json!({ "type": "message_stop" })))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let brain = anthropic_at(server.address);
    assert_eq!(ask_with(&brain, Some(&key_for(&brain))), [declined()]);

    // en plein milieu : ce qui a été écrit avant ne compte pas comme une réponse complète
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&message_start())),
        Step::Send(chunk(&text_delta(0, "Je commence"))),
        Step::Send(chunk(&stop("refusal"))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let brain = anthropic_at(server.address);
    assert_eq!(ask_with(&brain, Some(&key_for(&brain))), [said("Je commence"), declined()]);
}

#[test]
fn an_answer_cut_by_its_token_ceiling_is_kept_like_any_other_cut_answer() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&message_start())),
        Step::Send(chunk(&thinking_block(0))),
        Step::Send(chunk(&text_delta(1, "Une réponse qui s'arr"))),
        Step::Send(chunk(&stop("max_tokens"))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let brain = anthropic_at(server.address);
    assert_eq!(ask_with(&brain, Some(&key_for(&brain))), [said("Une réponse qui s'arr"), ReplyEvent::Done]);

    // toute la place prise par la réflexion : rien n'a été dit, et elle le dit
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&message_start())),
        Step::Send(chunk(&thinking_block(0))),
        Step::Send(chunk(&stop("max_tokens"))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let brain = anthropic_at(server.address);
    assert_eq!(ask_with(&brain, Some(&key_for(&brain))), [failed("Anthropic n'a rien répondu.")]);
}

#[test]
fn an_error_in_the_middle_of_anthropic_s_stream_is_explained_and_masked() {
    let server = serve(vec![
        Step::Send(STREAM_HEAD.to_vec()),
        Step::Send(chunk(&message_start())),
        Step::Send(chunk(&text_delta(0, "Je"))),
        Step::Send(chunk(&event(
            "error",
            json!({ "type": "error", "error": { "type": "overloaded_error", "message": format!("Overloaded ({KEY})") } }),
        ))),
        Step::Send(chunk(&text_delta(0, " continue quand même"))),
        Step::Send(LAST_CHUNK.to_vec()),
    ]);
    let brain = anthropic_at(server.address);
    assert_eq!(
        ask_with(&brain, Some(&key_for(&brain))),
        [said("Je"), failed("Anthropic a abandonné : Overloaded ([clé masquée])")]
    );
}

#[test]
fn a_refusal_from_anthropic_is_classified_like_the_others_and_never_quotes_the_key() {
    let error = |kind: &str, message: &str| {
        json!({ "type": "error", "error": { "type": kind, "message": message }, "request_id": "req_test" })
            .to_string()
    };
    // une clé refusée, dont le message rappelle le début et la fin : rien n'en reste
    let echoed =
        error("authentication_error", &format!("invalid key {}…{}", &KEY[..7], &KEY[KEY.len() - 4..]));
    let server = serve(vec![Step::Send(plain("401 Unauthorized", "application/json", &echoed))]);
    let brain = anthropic_at(server.address);
    let events = ask_with(&brain, Some(&key_for(&brain)));
    assert_eq!(
        events,
        [failed(
            "Anthropic refuse ma clé (401) : invalid key [clé masquée]…[clé masquée]. Pour la remettre : ouvre \
             Réglages… dans mon menu."
        )]
    );

    let cases = [
        (
            "403 Forbidden",
            error("permission_error", "Your API key does not have permission"),
            "Anthropic refuse (403) : Your API key does not have permission. Si c'est ma clé, pour la remettre : \
             ouvre Réglages… dans mon menu.",
        ),
        (
            "429 Too Many Requests",
            error("rate_limit_error", "Number of requests has exceeded your rate limit"),
            "Anthropic refuse (429) : Number of requests has exceeded your rate limit",
        ),
        (
            "529 Site Overloaded",
            error("overloaded_error", "Overloaded"),
            "Anthropic refuse (529) : Overloaded",
        ),
        (
            "404 Not Found",
            error("not_found_error", "model: modele-de-test"),
            "Anthropic refuse (404) : model: modele-de-test",
        ),
        (
            "400 Bad Request",
            error("invalid_request_error", &format!("bad header {KEY}")),
            "Anthropic refuse (400) : bad header [clé masquée]",
        ),
    ];
    for (status, body, expected) in cases {
        let server = serve(vec![Step::Send(plain(status, "application/json", &body))]);
        let brain = anthropic_at(server.address);
        assert_eq!(ask_with(&brain, Some(&key_for(&brain))), [failed(expected)], "{status}");
    }
}

/// Un serveur qui sert une connexion par réponse donnée, dans l'ordre, et rend chaque requête.
fn serve_each(responses: Vec<Vec<u8>>) -> (SocketAddr, Receiver<(String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let address = listener.local_addr().expect("l'adresse du serveur");
    let (sender, requests) = channel();
    std::thread::spawn(move || {
        for response in responses {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let Some(received) = read_request(&mut stream) else { return };
            let _ = sender.send(received);
            let _ = stream.write_all(&response);
        }
    });
    (address, requests)
}

#[test]
fn anthropic_s_models_are_listed_page_by_page_with_its_key() {
    let first = json!({ "data": [{ "type": "model", "id": "modele-a", "display_name": "A", "max_tokens": 128000 },
        { "type": "model", "id": "modele-b" }], "first_id": "modele-a", "has_more": true, "last_id": "modele-b" });
    let last = json!({ "data": [{ "type": "model", "id": "modele-c" }], "first_id": "modele-c",
        "has_more": false, "last_id": "modele-c" });
    let (address, requests) = serve_each(vec![
        plain("200 OK", "application/json", &first.to_string()),
        plain("200 OK", "application/json", &last.to_string()),
    ]);
    let brain = anthropic_at(address);
    assert_eq!(
        Client::new().models(Provider::Anthropic, brain.endpoint(), Some(&key_for(&brain))),
        Ok(vec!["modele-a".to_owned(), "modele-b".to_owned(), "modele-c".to_owned()])
    );
    let next = || requests.recv_timeout(Duration::from_secs(5)).expect("une requête");
    let (head, body) = next();
    assert!(head.starts_with("GET /v1/models?limit=1000 HTTP/1.1\r\n"), "{head}");
    assert_eq!(body, "");
    let (second, _) = next();
    assert!(second.starts_with("GET /v1/models?limit=1000&after_id=modele-b HTTP/1.1\r\n"), "{second}");
    for head in [head, second] {
        assert_eq!(header(&head, "x-api-key").as_deref(), Some(KEY));
        assert_eq!(header(&head, "anthropic-version").as_deref(), Some("2026-10-21"));
        assert_eq!(header(&head, "authorization"), None);
    }

    // une clé refusée en demandant la liste
    let refused = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
    let (address, _requests) = serve_each(vec![plain("401 Unauthorized", "application/json", refused)]);
    let brain = anthropic_at(address);
    assert!(matches!(
        Client::new().models(Provider::Anthropic, brain.endpoint(), Some(&key_for(&brain))),
        Err(Problem::KeyRefused { status: 401, reason: Some(_) })
    ));
}

/// La clé d'Anthropic donnée pour son adresse ne part jamais vers une autre : elle y compte pour
/// absente, et personne n'est contacté.
#[test]
fn anthropic_s_key_given_for_another_address_is_never_sent() {
    let given_for = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    given_for.set_nonblocking(true).expect("écoute sans attente");
    let unused = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    unused.set_nonblocking(true).expect("écoute sans attente");
    let address = unused.local_addr().expect("adresse");
    let brain = anthropic_at(address);
    let other_port = key_for(&anthropic_at(given_for.local_addr().expect("adresse")));
    assert_eq!(
        ask_with(&brain, Some(&other_port)),
        [failed(&format!(
            "Anthropic demande une clé, et je n'en ai pas pour {address}. Pour me la donner : ouvre Réglages… dans mon menu."
        ))]
    );
    assert_eq!(
        Client::new().models(Provider::Anthropic, brain.endpoint(), Some(&other_port)),
        Err(Problem::NoKey)
    );
    // ni la clé d'un autre fournisseur donnée pour la même adresse
    let openrouter_here = key_for(&provider_at(Provider::OpenRouter, address));
    assert_eq!(ask_with(&brain, Some(&openrouter_here)).len(), 1);
    std::thread::sleep(Duration::from_millis(50));
    for listener in [&unused, &given_for] {
        let knocked = listener.accept();
        assert!(
            matches!(&knocked, Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
            "personne n'a été contacté : {knocked:?}"
        );
    }
}
