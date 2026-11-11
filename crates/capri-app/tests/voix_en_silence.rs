//! La vraie voix de Capri, avec et sans effets, exercée sans un bruit.
//!
//! Le système ne rend le son traité que sur le thread principal, par sa boucle : un test de
//! `cargo test` n'y tourne jamais. Celui-ci lance donc `capri voice rehearse` (version de
//! débogage seulement), qui fait tout sur le thread principal de son propre processus, avec des
//! voix d'essai muettes ou rendues en mémoire, sans lire aucun réglage de l'utilisateur.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
#[ignore = "crée de vraies voix et un vrai moteur audio (à volume nul ou hors ligne), macOS seulement"]
fn the_real_voice_with_and_without_effects_without_a_sound() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let directory = std::env::temp_dir().join(format!("capri-test-voix-en-silence-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let mut child = Command::new(env!("CARGO_BIN_EXE_capri"))
        .args(["voice", "rehearse"])
        // rien ne doit les lire, mais au cas où : des lieux d'essai
        .env("CAPRI_CONFIG_DIR", &directory)
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-voix-en-silence")
        .env("CAPRI_SOCKET", directory.join("capri.sock"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("capri se lance");
    let deadline = Instant::now() + Duration::from_secs(400);
    while child.try_wait().expect("état du processus").is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("la répétition de la voix ne finit pas");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let output = child.wait_with_output().expect("sortie");
    let (stdout, stderr) = (String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    println!("{stdout}{stderr}");
    assert!(output.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("tout est conforme"), "{stdout}");
    assert!(!directory.exists(), "aucun réglage n'a été écrit");
}
