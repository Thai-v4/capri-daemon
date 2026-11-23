//! Installer une version de bout en bout, sans jamais sortir de la machine (essais ignorés,
//! version de débogage, macOS) :
//!
//! - une petite `Capri.app` d'essai dans un dossier temporaire : le vrai `capri` de débogage, un
//!   faux second programme, notre `Info.plist`, de fausses ressources, signée « ad hoc » par le
//!   vrai `codesign` (le trousseau n'est jamais lu) ;
//! - un faux serveur de cette machine, rangé comme GitHub : le manifeste et chaque fichier
//!   derrière deux redirections, la dernière vers une adresse « signée » ;
//! - `capri update rehearse`, lancé depuis cette app avec les quatre lieux d'essai : le plan, le
//!   téléchargement vérifié, l'app refaite à côté et signée, le relais ; puis elle sort, et le
//!   programme de fin (le `capri` de la nouvelle app) la met en place ;
//! - le résultat : la nouvelle version à sa place, signature valide, ressources de l'ancienne,
//!   rien d'autre dans le dossier, et ni l'adresse signée ni rien de la signature dans ce qui
//!   s'écrit. **La relance n'est pas essayée** : avec `CAPRI_CONFIG_DIR`, le programme de fin la
//!   saute (`open` ne lui passerait pas ses lieux d'essai, et pourrait retrouver la Capri de
//!   l'utilisateur, du même identifiant).
//!
//! Et un fichier abîmé (une autre empreinte) : rien n'est installé, et rien ne reste.
//!
//! `cargo test -p capri-app --test mise_a_jour_installee -- --ignored --test-threads=1`

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// La signature de l'adresse « signée » du faux serveur : elle ne doit apparaître nulle part.
const SIGNATURE: &str = "ZqSiGnAtUrEdEsSaI";
const VERSION: &str = "9.0.0";

/// Un dossier d'essai, retiré à la fin, même en cas d'échec.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("le dossier d'essai");
        Self(directory.canonicalize().expect("son vrai chemin"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn info(version: &str) -> String {
    include_str!("../../../tools/macos/Info.plist").replace("@VERSION@", version)
}

/// Le vrai `codesign`, par son chemin absolu.
fn codesign(args: &[&str], path: &Path) -> std::process::Output {
    Command::new("/usr/bin/codesign").args(args).arg(path).output().expect("codesign")
}

/// L'empreinte SHA-256 de `bytes`, par l'outil du système (pas par celle de Capri).
fn shasum(bytes: &[u8], scratch: &Path) -> String {
    let file = scratch.join("a-mesurer");
    std::fs::write(&file, bytes).unwrap();
    let output = Command::new("/usr/bin/shasum").args(["-a", "256"]).arg(&file).output().expect("shasum");
    let _ = std::fs::remove_file(&file);
    String::from_utf8_lossy(&output.stdout).split(' ').next().unwrap_or_default().to_owned()
}

/// La petite app installée d'essai, signée « ad hoc ».
fn installed_app(folder: &Path) -> PathBuf {
    let app = folder.join("Capri.app");
    std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    std::fs::create_dir_all(app.join("Contents/Resources/assets/capri/stand")).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_capri"), app.join("Contents/MacOS/capri")).unwrap();
    std::fs::copy("/usr/bin/true", app.join("Contents/MacOS/capri-hook")).unwrap();
    std::fs::write(app.join("Contents/Info.plist"), info("0.1.0")).unwrap();
    std::fs::write(app.join("Contents/Resources/CREDITS.md"), "crédits d'essai\n").unwrap();
    std::fs::write(
        app.join("Contents/Resources/assets/capri/stand/idle.png"),
        [137u8, 80, 78, 71, 13, 10, 26, 10],
    )
    .unwrap();
    assert!(
        codesign(&["--force", "--timestamp=none", "--sign", "-"], &app.join("Contents/MacOS/capri-hook"))
            .status
            .success()
    );
    assert!(codesign(&["--force", "--timestamp=none", "--sign", "-"], &app).status.success());
    assert!(codesign(&["--verify", "--strict"], &app).status.success());
    app
}

/// Les fichiers de la version, par leur nom parmi ceux de la version, et le manifeste.
/// `spoil` : le fichier dont l'empreinte annoncée est fausse.
fn release(scratch: &Path, spoil: Option<&str>) -> (HashMap<String, Vec<u8>>, String) {
    let mut files = HashMap::new();
    files.insert("capri-macos-arm64".to_owned(), std::fs::read(env!("CARGO_BIN_EXE_capri")).unwrap());
    files.insert("capri-hook-macos-arm64".to_owned(), std::fs::read("/usr/bin/false").unwrap());
    files.insert("Info.plist".to_owned(), info(VERSION).into_bytes());
    let entry = |name: &str, asset: &str| {
        let bytes = &files[asset];
        let sha = if spoil == Some(asset) { "0".repeat(64) } else { shasum(bytes, scratch) };
        format!(r#"{{"name": "{name}", "asset": "{asset}", "size": {}, "sha256": "{sha}"}}"#, bytes.len())
    };
    let bundle = include_str!("../../../tools/macos/bundle-format").trim();
    let manifest = format!(
        r#"{{"format": 1, "version": "{VERSION}", "bundle": {bundle}, "targets": {{"macos-arm64": {{"files": [{}, {}, {}]}}}}}}"#,
        entry("capri", "capri-macos-arm64"),
        entry("capri-hook", "capri-hook-macos-arm64"),
        entry("Info.plist", "Info.plist")
    );
    (files, manifest)
}

/// Un faux serveur de cette machine, rangé comme GitHub : chaque adresse de la version renvoie,
/// par deux redirections, vers une adresse « signée » qui sert le fichier. Il rend l'en-tête de
/// chaque requête.
fn github_like(files: HashMap<String, Vec<u8>>, manifest: String) -> (SocketAddr, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let address = listener.local_addr().expect("son adresse");
    let (sender, requests) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let (mut bytes, mut buffer) = (Vec::new(), [0u8; 4096]);
            while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(received) if received > 0 => bytes.extend_from_slice(&buffer[..received]),
                    _ => break,
                }
            }
            let head = String::from_utf8_lossy(&bytes).into_owned();
            let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
            let _ = sender.send(head);
            let redirect = |location: String| {
                format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .into_bytes()
            };
            let found = |body: &[u8]| {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                [head.as_bytes(), body].concat()
            };
            let reply = if path == "/Thai-v4/capri-daemon/releases/latest/download/latest.json" {
                redirect(format!("/Thai-v4/capri-daemon/releases/download/v{VERSION}/latest.json"))
            } else if let Some(name) =
                path.strip_prefix(&format!("/Thai-v4/capri-daemon/releases/download/v{VERSION}/"))
            {
                redirect(format!("http://{address}/signe/{name}?se=2026&sig={SIGNATURE}"))
            } else if let Some(rest) = path.strip_prefix("/signe/") {
                let name = rest.split('?').next().unwrap_or_default();
                match (name, files.get(name)) {
                    ("latest.json", _) => found(manifest.as_bytes()),
                    (_, Some(bytes)) => found(bytes),
                    _ => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                }
            } else {
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
            };
            let _ = stream.write_all(&reply);
        }
    });
    (address, requests)
}

/// `capri update rehearse`, lancé depuis l'app `app`, avec les quatre lieux d'essai.
fn rehearse(app: &Path, scratch: &Path, sockets: &Path, address: SocketAddr) -> (bool, String) {
    let output = Command::new(app.join("Contents/MacOS/capri"))
        .args(["update", "rehearse"])
        // ses quatre lieux d'essai : réglages, journal et ouverture de session ; trousseau ; canal ;
        // Claude Code
        .env("CAPRI_CONFIG_DIR", scratch.join("capri"))
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-installation")
        .env("CAPRI_SOCKET", sockets.join("capri.sock"))
        .env("CLAUDE_CONFIG_DIR", scratch.join("claude"))
        .env(
            "CAPRI_UPDATE_URL",
            format!("http://{address}/Thai-v4/capri-daemon/releases/latest/download/latest.json"),
        )
        .env("RUST_LOG", "trace")
        .output()
        .expect("capri se lance");
    let said =
        format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    (output.status.success(), said)
}

/// Ce qu'il y a dans le dossier des apps.
fn listed(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn applications(scratch: &Path) -> PathBuf {
    let folder = scratch.join("Applications");
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
#[ignore = "signe pour de vrai (« ad hoc ») et lance le programme de fin ; version de débogage, macOS ; à lancer à la main"]
fn a_version_is_downloaded_checked_rebuilt_signed_and_put_in_place() {
    if !cfg!(all(debug_assertions, target_os = "macos")) {
        return;
    }
    let scratch = Scratch::new("capri-test-installation");
    let sockets = Scratch::new("capri-test-installation-canal");
    let folder = applications(&scratch.0);
    let app = installed_app(&folder);
    let old_seal = std::fs::read(app.join("Contents/_CodeSignature/CodeResources")).unwrap();
    let (files, manifest) = release(&scratch.0, None);
    let (address, requests) = github_like(files, manifest);
    let (succeeded, said) = rehearse(&app, &scratch.0, &sockets.0, address);
    println!("{said}");
    assert!(succeeded, "{said}");
    assert!(said.contains("Bulle : Je télécharge la 9.0.0…"), "{said}");
    assert!(said.contains("Bulle : La 9.0.0 est prête ; macOS te redemandera le micro"), "{said}");
    assert!(said.contains("Relais pris"), "{said}");
    // le programme de fin finit seul, une fois la répétition sortie
    let journal = scratch.0.join("capri/logs/capri.log");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut log = String::new();
    while Instant::now() < deadline {
        log = std::fs::read_to_string(&journal).unwrap_or_default();
        if log.contains("relance sautée") || log.contains("pas mise en place") {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let shown: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("mise à jour") || line.contains("programme de fin"))
        .collect();
    println!("{}", shown.join("\n"));
    assert!(log.contains("programme de fin d'une mise à jour"), "{log}");
    assert!(log.contains("mise à jour : l'ancienne Capri est sortie, mise en place"), "{log}");
    assert!(log.contains("mise à jour : nouvelle app à sa place, signature vérifiée"), "{log}");
    assert!(log.contains("mise à jour : la nouvelle version est en place"), "{log}");
    assert!(log.contains("mise à jour : relance sautée (dossier d'essai)"), "{log}");
    // le résultat : la nouvelle version à sa place, et rien d'autre dans le dossier
    assert_eq!(listed(&folder), ["Capri.app"]);
    assert_eq!(std::fs::read_to_string(app.join("Contents/Info.plist")).unwrap(), info(VERSION));
    assert!(codesign(&["--verify", "--strict"], &app).status.success(), "signature valide à sa place");
    let described = codesign(&["-d", "--verbose=2"], &app);
    assert!(
        String::from_utf8_lossy(&described.stderr).contains("Signature=adhoc"),
        "signée comme l'ancienne"
    );
    assert_ne!(
        std::fs::read(app.join("Contents/_CodeSignature/CodeResources")).unwrap(),
        old_seal,
        "une signature neuve"
    );
    // le second programme est celui de la version (le faux, /usr/bin/false), signé de nouveau
    assert!(!Command::new(app.join("Contents/MacOS/capri-hook")).status().unwrap().success());
    // les ressources de l'ancienne, telles quelles
    assert_eq!(
        std::fs::read_to_string(app.join("Contents/Resources/CREDITS.md")).unwrap(),
        "crédits d'essai\n"
    );
    assert_eq!(
        std::fs::read(app.join("Contents/Resources/assets/capri/stand/idle.png")).unwrap(),
        [137u8, 80, 78, 71, 13, 10, 26, 10]
    );
    // rien sur l'utilisateur ni sur la machine dans les requêtes ; jamais l'adresse signée écrite
    let received: Vec<String> = requests.try_iter().collect();
    assert_eq!(received.len(), 9, "le manifeste (trois requêtes), puis trois fichiers (deux chacun)");
    for request in &received {
        let headers: Vec<String> = request
            .lines()
            .skip(1)
            .filter(|line| !line.is_empty())
            .map(|line| line.split(':').next().unwrap_or_default().to_ascii_lowercase())
            .collect();
        assert_eq!(headers, ["host"], "{request}");
    }
    for text in [&said, &log] {
        assert!(!text.contains(SIGNATURE), "l'adresse signée est écrite");
        assert!(!text.contains("github.com"), "GitHub n'a rien à faire ici");
        assert!(!text.contains("Authority="), "rien de la signature");
    }
}

#[test]
#[ignore = "signe pour de vrai (« ad hoc ») ; version de débogage, macOS ; à lancer à la main"]
fn a_damaged_file_installs_nothing_and_leaves_nothing() {
    if !cfg!(all(debug_assertions, target_os = "macos")) {
        return;
    }
    let scratch = Scratch::new("capri-test-installation-abimee");
    let sockets = Scratch::new("capri-test-installation-abimee-canal");
    let folder = applications(&scratch.0);
    let app = installed_app(&folder);
    let (files, manifest) = release(&scratch.0, Some("capri-hook-macos-arm64"));
    let (address, _) = github_like(files, manifest);
    let (succeeded, said) = rehearse(&app, &scratch.0, &sockets.0, address);
    println!("{said}");
    assert!(!succeeded, "{said}");
    assert!(said.contains("Refus : Un fichier reçu est abîmé : je n'installe rien"), "{said}");
    assert!(said.contains("empreinte différente de celle du manifeste"), "{said}");
    assert_eq!(listed(&folder), ["Capri.app"], "rien ne reste à côté de l'app");
    assert_eq!(std::fs::read_to_string(app.join("Contents/Info.plist")).unwrap(), info("0.1.0"));
    assert!(codesign(&["--verify", "--strict"], &app).status.success(), "l'app installée n'a pas bougé");
    assert!(!said.contains(SIGNATURE));
}
