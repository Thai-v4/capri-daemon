//! La vraie liste des fenêtres, lue et triée comme Capri le fait, sans rien afficher.
//!
//! L'écran principal ne se lit que sur le thread principal, qu'un test de `cargo test` n'a pas :
//! celui-ci lance donc `capri perch rehearse` (version de débogage seulement), avec des lieux
//! d'essai, et vérifie qu'il ne dit que des nombres : chaque mot de sa sortie doit figurer dans son
//! propre code source (ou dans celui des erreurs de la liste). Un titre de fenêtre ou un nom d'app
//! qui s'y glisserait ferait échouer le test.

use std::collections::HashSet;
use std::process::Command;

#[test]
#[ignore = "lit la vraie liste des fenêtres de cette session (macOS) ; à lancer à la main"]
fn the_real_window_list_says_only_numbers() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let directory = std::env::temp_dir().join(format!("capri-test-fenetres-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let output = Command::new(env!("CARGO_BIN_EXE_capri"))
        .args(["perch", "rehearse"])
        // rien ne doit les lire, mais au cas où : des lieux d'essai
        .env("CAPRI_CONFIG_DIR", &directory)
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-fenetres")
        .env("CAPRI_SOCKET", directory.join("capri.sock"))
        .output()
        .expect("capri se lance");
    let (stdout, stderr) = (String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    println!("{stdout}{stderr}");
    assert!(output.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("tout est conforme"), "{stdout}");
    assert!(!directory.exists(), "aucun réglage n'a été écrit");

    let words = |text: &str| -> Vec<String> {
        text.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
    };
    let sources = [
        include_str!("../src/perch_rehearsal.rs"),
        include_str!("../src/assets.rs"),
        include_str!("../../capri-platform/src/desk.rs"),
    ];
    let known: HashSet<String> = sources.iter().flat_map(|source| words(source)).collect();
    let strangers: Vec<String> = words(&stdout).into_iter().filter(|word| !known.contains(word)).collect();
    assert!(strangers.is_empty(), "des mots qui ne viennent pas de Capri : {strangers:?}");
    assert!(stderr.trim().is_empty(), "rien sur la sortie d'erreur : {stderr}");
}
