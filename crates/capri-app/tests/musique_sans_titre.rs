//! Les annonces des lecteurs, guettées comme Capri le fait : reçues, leur état lu, et jamais un mot
//! de leur titre, de leur artiste ni de leur album.
//!
//! Le test fabrique de faux titre, artiste et album, propres à ce lancement (on ne peut pas les
//! trouver dans le code de Capri), et les fait porter par une annonce d'essai, postée par un autre
//! processus sous un nom d'essai (jamais celui d'un vrai lecteur). Puis il vérifie :
//! - `capri music rehearse` (version de débogage) : l'annonce est reçue, son état est lu, et ni sa
//!   sortie ni sa sortie d'erreur ne contiennent les faux champs ; chaque mot de sa sortie figure
//!   dans le code source de la répétition ou du guetteur ;
//! - la vraie app (`CAPRI_MUSIC_REHEARSAL`, journal au niveau `trace`) : l'annonce est reçue alors
//!   que l'app n'est pas active, et le journal ne contient pas les faux champs. Elle a besoin de
//!   ses images : `CAPRI_ASSETS=/chemin/vers/assets` dans un worktree qui n'en a pas. Capri
//!   apparaît à l'écran une quinzaine de secondes, puis quitte.
//!
//! Ne joue aucun son, ne demande aucune permission, ne pilote aucun lecteur.

use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// De faux champs propres à ce lancement : des lettres tirées du numéro du processus et de
/// l'heure, collées à un préfixe qu'aucun code de Capri ne contient.
fn fakes() -> [String; 3] {
    let seed = u64::from(std::process::id())
        ^ std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
    let letters = |salt: u64| -> String {
        let mut n = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(salt);
        (0..10)
            .map(|_| {
                n = n.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                char::from(b'a' + ((n >> 33) % 26) as u8)
            })
            .collect()
    };
    [format!("Zqtitre{}", letters(1)), format!("Zqartiste{}", letters(2)), format!("Zqalbum{}", letters(3))]
}

fn assert_no_fakes(text: &str, fakes: &[String; 3]) {
    let lower = text.to_lowercase();
    for fake in fakes {
        assert!(!lower.contains(&fake.to_lowercase()), "un faux champ de l'annonce est sorti : {fake}");
    }
    // et pas même le préfixe, au cas où un morceau en serait cité
    assert!(!lower.contains("zqtitre") && !lower.contains("zqartiste") && !lower.contains("zqalbum"));
}

#[test]
#[ignore = "guette une annonce d'essai sur cette session (macOS) ; à lancer à la main"]
fn the_rehearsal_reads_the_state_and_never_the_title() {
    let directory = std::env::temp_dir().join(format!("capri-test-musique-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let fakes = fakes();
    let output = Command::new(env!("CARGO_BIN_EXE_capri"))
        .args(["music", "rehearse", &fakes[0], &fakes[1], &fakes[2]])
        // rien ne doit les lire, mais au cas où : des lieux d'essai
        .env("CAPRI_CONFIG_DIR", &directory)
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-musique")
        .env("CAPRI_SOCKET", directory.join("capri.sock"))
        .output()
        .expect("capri se lance");
    let (stdout, stderr) = (String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    println!("{stdout}{stderr}");
    assert!(output.status.success(), "{stdout}{stderr}");
    if cfg!(target_os = "macos") {
        assert!(stdout.contains("tout est conforme"), "{stdout}");
        assert!(stdout.contains("« lecture » : reçue oui"), "{stdout}");
        assert!(stdout.contains("état lu : en lecture"), "{stdout}");
        assert!(stdout.contains("état lu : en pause"), "{stdout}");
    } else {
        assert!(stdout.contains("elle ne sait pas"), "{stdout}");
    }
    assert!(!directory.exists(), "aucun réglage n'a été écrit");
    assert_no_fakes(&stdout, &fakes);
    assert_no_fakes(&stderr, &fakes);

    let words = |text: &str| -> Vec<String> {
        text.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
    };
    let sources = [
        include_str!("../src/music_rehearsal.rs"),
        include_str!("../../capri-platform/src/music.rs"),
        include_str!("../../capri-core/src/music.rs"),
    ];
    let known: HashSet<String> = sources.iter().flat_map(|source| words(source)).collect();
    let strangers: Vec<String> = words(&stdout).into_iter().filter(|word| !known.contains(word)).collect();
    assert!(strangers.is_empty(), "des mots qui ne viennent pas de Capri : {strangers:?}");
    assert!(stderr.trim().is_empty(), "rien sur la sortie d'erreur : {stderr}");
}

#[test]
#[ignore = "lance la vraie app une quinzaine de secondes (macOS, avec ses images) ; à lancer à la main"]
fn the_real_app_hears_an_announcement_while_inactive_and_never_logs_the_title() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let directory = std::env::temp_dir().join(format!("capri-test-musique-app-{}", std::process::id()));
    let sockets = std::env::temp_dir().join(format!("capri-test-musique-app-socket-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let fakes = fakes();
    let mut child = Command::new(env!("CARGO_BIN_EXE_capri"))
        .env("CAPRI_CONFIG_DIR", &directory)
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-musique")
        .env("CAPRI_SOCKET", sockets.join("capri.sock"))
        .env("CAPRI_MUSIC_REHEARSAL", fakes.join(";"))
        .env("RUST_LOG", "trace")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("capri se lance");
    // ses sorties lues à part, pour qu'elle ne bloque pas sur un tube plein
    let read = |pipe: Box<dyn std::io::Read + Send>| {
        std::thread::spawn(move || {
            let mut text = Vec::new();
            let mut pipe = pipe;
            let _ = std::io::Read::read_to_end(&mut pipe, &mut text);
            String::from_utf8_lossy(&text).into_owned()
        })
    };
    let stdout = read(Box::new(child.stdout.take().expect("sortie")));
    let stderr = read(Box::new(child.stderr.take().expect("sortie d'erreur")));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("attente") {
            break Some(status);
        }
        if started.elapsed() > Duration::from_secs(90) {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let log = format!("{}{}", stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
    let _ = std::fs::remove_dir_all(&sockets);
    let lines: Vec<&str> = log.lines().filter(|line| line.contains("musique")).collect();
    println!("{}", lines.join("\n"));
    assert!(status.is_some_and(|s| s.success()), "elle a quitté d'elle-même, sans erreur");
    assert!(log.contains("répétition de la musique : tout est conforme"), "{}", lines.join("\n"));
    assert!(log.contains("alors que l'app n'est pas active"), "{}", lines.join("\n"));
    assert!(
        log.contains("annonce d'essai reçue par Capri (en lecture)"),
        "livraison suspendue, reçue quand même"
    );
    assert!(log.contains("oublié au bout de"), "le lecteur d'essai introuvable oublié");
    assert_no_fakes(&log, &fakes);
    assert!(!directory.exists(), "aucun réglage n'a été écrit");
}
