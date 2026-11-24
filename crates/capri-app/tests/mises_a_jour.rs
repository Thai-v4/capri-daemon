//! Savoir qu'une version existe, sans jamais sortir de la machine.
//!
//! - `capri update check`, lancée pour de vrai avec des lieux d'essai : avec `CAPRI_CONFIG_DIR`,
//!   elle ne regarde nulle part (ni GitHub, ni rien) ; en version de débogage, `CAPRI_UPDATE_URL`
//!   la fait lire un faux serveur de cette machine, qui redirige deux fois comme GitHub, l'une vers
//!   une adresse « signée » : la sortie dit le verdict, jamais cette adresse ; une adresse d'essai
//!   qui n'est pas cette machine est refusée, et GitHub ne la remplace pas. Dans la version
//!   livrée, `CAPRI_UPDATE_URL` n'est pas lue.
//! - la vraie app (essai ignoré, version de débogage, avec ses images : `CAPRI_ASSETS=…` dans un
//!   worktree qui n'en a pas), avec ses quatre lieux d'essai et le même faux serveur : elle le lit
//!   après son délai de grâce d'essai, le dit dans son journal et au-dessus de sa tête, et ni sa
//!   sortie d'erreur ni son fichier de journal (au niveau `trace`) ne contiennent l'adresse signée.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Un dossier d'essai, retiré quand l'essai le lâche, même en cas d'échec.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("le dossier d'essai");
        Self(directory)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// La signature de l'adresse « signée » du faux serveur : elle ne doit apparaître nulle part.
const SIGNATURE: &str = "ZqSiGnAtUrEdEsSaI";

/// Un faux serveur de cette machine, comme GitHub : la dernière version renvoie vers la version,
/// qui renvoie vers une adresse signée, qui sert le manifeste. Il rend l'en-tête de chaque requête.
fn github_like(version: &str) -> (SocketAddr, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let address = listener.local_addr().expect("son adresse");
    let manifest = format!(
        r#"{{"format": 1, "version": "{version}", "bundle": 1, "targets": {{
            "macos-arm64": {{"files": [{{"name": "capri", "asset": "capri-macos-arm64", "size": 1, "sha256": "{0}"}}]}},
            "windows-x86_64": {{"files": [{{"name": "capri.exe", "asset": "capri-windows-x86_64.exe", "size": 1, "sha256": "{0}"}}]}}
        }}}}"#,
        "b".repeat(64)
    );
    let redirect = |location: String| {
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    };
    let replies = vec![
        redirect(format!("/Thai-v4/capri-daemon/releases/download/v{version}/latest.json")),
        redirect(format!("http://{address}/signe/latest.json?se=2026&sig={SIGNATURE}")),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{manifest}",
            manifest.len()
        ),
    ];
    let (sender, requests) = channel();
    std::thread::spawn(move || {
        for (stream, reply) in listener.incoming().zip(replies) {
            let Ok(mut stream) = stream else { return };
            let (mut bytes, mut buffer) = (Vec::new(), [0u8; 4096]);
            while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(received) if received > 0 => bytes.extend_from_slice(&buffer[..received]),
                    _ => return,
                }
            }
            let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    (address, requests)
}

/// `capri update check`, avec tous ses lieux d'essai, et `extra` en plus.
fn check(scratch: &Path, extra: &[(&str, &str)]) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_capri"));
    command
        .args(["update", "check"])
        .env("CAPRI_CONFIG_DIR", scratch.join("capri"))
        .env("CLAUDE_CONFIG_DIR", scratch.join("claude"))
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-mises-a-jour")
        .env("CAPRI_SOCKET", scratch.join("capri.sock"))
        .env_remove("CAPRI_UPDATE_URL");
    for (name, value) in extra {
        command.env(name, value);
    }
    let output = command.output().expect("capri se lance");
    let said =
        format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    (output.status.success(), said)
}

/// Tout son journal en fichier, dans le dossier `logs` : le précédent (`capri.old.log`, s'il a
/// tourné, ce qu'il fait vite au niveau `trace`), puis le courant ; ce qui est attendu peut être
/// dans l'un ou l'autre, et ce qui ne doit jamais y être, ni dans l'un ni dans l'autre. Rend aussi
/// s'il a tourné.
fn whole_journal(logs: &Path) -> (String, bool) {
    let old = std::fs::read_to_string(logs.join("capri.old.log")).ok();
    let current = std::fs::read_to_string(logs.join("capri.log")).unwrap_or_default();
    let rotated = old.is_some();
    (old.unwrap_or_default() + &current, rotated)
}

#[test]
fn from_a_test_folder_the_command_looks_nowhere() {
    let scratch = Scratch::new("capri-test-mises-a-jour-commande");
    let (succeeded, said) = check(&scratch.0, &[]);
    assert!(!succeeded, "{said}");
    assert!(said.contains("Version installée : "), "{said}");
    assert!(said.contains("rien ne part vers GitHub"), "{said}");
    assert!(!said.contains("Manifeste lu à"), "elle n'a rien lu : {said}");
    assert!(!scratch.0.join("capri").exists(), "rien n'est écrit : ni réglage ni journal");
}

#[test]
fn a_local_server_like_github_is_read_and_its_signed_address_never_said() {
    let scratch = Scratch::new("capri-test-mises-a-jour-serveur");
    let (address, requests) = github_like("9.0.0");
    let url = format!("http://{address}/Thai-v4/capri-daemon/releases/latest/download/latest.json");
    let (succeeded, said) = check(&scratch.0, &[("CAPRI_UPDATE_URL", &url)]);
    if !cfg!(debug_assertions) {
        // la version livrée ne lit pas CAPRI_UPDATE_URL : le dossier d'essai l'emporte
        assert!(!succeeded && said.contains("rien ne part vers GitHub"), "{said}");
        assert_eq!(requests.try_iter().count(), 0, "personne n'a été contacté");
        return;
    }
    assert!(succeeded, "{said}");
    assert!(said.contains(&format!("Manifeste lu à    : {url}")), "{said}");
    assert!(said.contains("Dernière publiée  : 9.0.0"), "{said}");
    assert!(said.contains("Une version plus récente existe"), "{said}");
    assert!(!said.contains(SIGNATURE) && !said.contains("sig="), "l'adresse signée est sortie : {said}");
    let received: Vec<String> = requests.try_iter().collect();
    assert_eq!(received.len(), 3, "{received:?}");
    assert!(received[2].starts_with(&format!("GET /signe/latest.json?se=2026&sig={SIGNATURE} HTTP/1.1")));
    for request in &received {
        let lower = request.to_ascii_lowercase();
        assert!(!lower.contains("user-agent") && !lower.contains("authorization"), "{request}");
    }
    assert!(!scratch.0.join("capri").exists(), "rien n'est écrit : ni réglage ni journal");
}

#[test]
fn a_test_address_elsewhere_than_this_machine_is_refused_and_never_replaced_by_github() {
    if !cfg!(debug_assertions) {
        return; // la version livrée ne lit pas CAPRI_UPDATE_URL
    }
    let scratch = Scratch::new("capri-test-mises-a-jour-ailleurs");
    for url in [
        "https://github.com/Thai-v4/capri-daemon/releases/latest/download/latest.json",
        "http://localhost:1/latest.json",
        "http://192.168.1.2:8080/latest.json",
    ] {
        let (succeeded, said) = check(&scratch.0, &[("CAPRI_UPDATE_URL", url)]);
        assert!(!succeeded, "{url} : {said}");
        assert!(said.contains("ne désigne pas un serveur de cette machine"), "{url} : {said}");
        assert!(!said.contains("Manifeste lu à"), "{url} : {said}");
    }
}

#[test]
#[ignore = "lance la vraie app (version de débogage, macOS, avec ses images) ; à lancer à la main"]
fn the_real_app_hears_of_a_newer_version_from_a_local_server_and_never_logs_the_signed_address() {
    if !cfg!(all(debug_assertions, target_os = "macos")) {
        return;
    }
    // retirés à la fin, même en cas d'échec ; le canal à part, pour garder son chemin court
    let directory = Scratch::new("capri-test-maj-app");
    let sockets = Scratch::new("capri-test-maj-app-socket");
    let claude = Scratch::new("capri-test-maj-app-claude");
    let (address, requests) = github_like("9.0.0");
    let url = format!("http://{address}/Thai-v4/capri-daemon/releases/latest/download/latest.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_capri"))
        // ses quatre lieux d'essai : réglages, journal et ouverture de session ; trousseau ; canal ;
        // Claude Code
        .env("CAPRI_CONFIG_DIR", &directory.0)
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-mises-a-jour")
        .env("CAPRI_SOCKET", sockets.0.join("capri.sock"))
        .env("CLAUDE_CONFIG_DIR", &claude.0)
        .env("CAPRI_UPDATE_URL", &url)
        .env("RUST_LOG", "trace")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("capri se lance");
    let mut stderr = child.stderr.take().expect("sortie d'erreur");
    let (lines, heard) = channel();
    std::thread::spawn(move || {
        let mut text = Vec::new();
        let mut buffer = [0u8; 4096];
        while let Ok(received) = stderr.read(&mut buffer) {
            if received == 0 {
                break;
            }
            text.extend_from_slice(&buffer[..received]);
            let _ = lines.send(String::from_utf8_lossy(&text).into_owned());
        }
    });
    // jusqu'à ce qu'elle ait dit la version au-dessus de sa tête, une minute au plus
    let started = Instant::now();
    let mut log = String::new();
    while started.elapsed() < Duration::from_secs(60) {
        if let Ok(text) = heard.recv_timeout(Duration::from_millis(200)) {
            log = text;
        }
        if log.contains("message de l'app affiché à son tour") {
            break;
        }
    }
    // ce n'est pas la Capri de l'utilisateur : c'est celle que cet essai vient de lancer
    let _ = child.kill();
    let _ = child.wait();
    while let Ok(text) = heard.recv_timeout(Duration::from_millis(500)) {
        log = text;
    }
    let shown: Vec<&str> =
        log.lines().filter(|line| line.contains("mises à jour") || line.contains("à son tour")).collect();
    println!("{}", shown.join("\n"));
    assert!(log.contains("mises à jour : le serveur d'essai regardé dans 3 s"), "{}", shown.join("\n"));
    assert!(log.contains("mises à jour : version 9.0.0 disponible"), "{}", shown.join("\n"));
    assert!(log.contains("message de l'app affiché à son tour"), "{}", shown.join("\n"));
    // ses deux fichiers : au niveau `trace`, il tourne vite
    let (journal, rotated) = whole_journal(&directory.0.join("logs"));
    println!("journal : {} octets, tourné : {rotated}", journal.len());
    assert!(journal.contains("mises à jour : version 9.0.0 disponible"));
    for text in [&log, &journal] {
        assert!(!text.contains(SIGNATURE), "l'adresse signée est dans le journal");
        assert!(!text.contains("github.com"), "GitHub n'a rien à faire ici");
    }
    assert_eq!(requests.try_iter().count(), 3);
    // ses lieux d'essai seulement : son dossier d'ouverture de session est celui d'essai, vide
    assert!(log.contains("ouverture de session : dossier d'essai"), "{}", shown.join("\n"));
}
