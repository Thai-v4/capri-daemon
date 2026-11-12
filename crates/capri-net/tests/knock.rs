//! Frapper à une adresse de cette machine pour savoir si Ollama tourne : contre de faux serveurs
//! locaux, sans jamais rien envoyer ni sortir de la machine.

use std::io::Read;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use capri_chat::Endpoint;
use capri_net::{KnockRefused, Presence, knock};

fn at(url: &str) -> Endpoint {
    Endpoint::parse(url).expect("valide")
}

#[test]
fn a_listening_server_answers_and_receives_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let port = listener.local_addr().expect("son adresse").port();
    assert_eq!(knock(&at(&format!("http://127.0.0.1:{port}/v1"))), Ok(Presence::Answers));
    let (mut stream, _) = listener.accept().expect("la connexion");
    stream.set_read_timeout(Some(Duration::from_secs(2))).expect("délai");
    let mut received = Vec::new();
    stream.read_to_end(&mut received).expect("fermée sans erreur");
    assert!(received.is_empty(), "rien n'est envoyé : {received:?}");
}

#[test]
fn localhost_is_this_machine_without_asking_the_name_system() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let port = listener.local_addr().expect("son adresse").port();
    assert_eq!(knock(&at(&format!("http://localhost:{port}/v1"))), Ok(Presence::Answers));
}

#[test]
fn nobody_listening_is_nobody() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
        listener.local_addr().expect("son adresse").port()
    };
    // le port vient d'être libéré : personne n'y écoute
    let started = Instant::now();
    assert_eq!(knock(&at(&format!("http://127.0.0.1:{port}"))), Ok(Presence::Nobody));
    // une seconde au plus par adresse (Windows insiste avant de croire un port fermé)
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

#[test]
fn another_machine_is_never_knocked_at() {
    for url in [
        "http://192.168.1.20:11434/v1",
        "http://10.0.0.5:11434",
        "http://100.64.0.1:11434",
        "http://[fe80::1]:11434",
        "https://ollama.com/v1",
        "https://example.com",
    ] {
        let started = Instant::now();
        assert_eq!(knock(&at(url)), Err(KnockRefused::NotThisMachine), "{url}");
        assert!(started.elapsed() < Duration::from_millis(50), "refusé avant toute connexion : {url}");
    }
    assert_eq!(knock(&at("https://localhost:11434/v1")), Err(KnockRefused::Encrypted));
}
