//! Un son joue-t-il sur cette machine ? Écouté comme Capri le fait, sans rien jouer.
//!
//! Lance `capri music rehearse` (version de débogage seulement), avec des lieux d'essai, et vérifie
//! qu'il ne dit que « oui », « non » et des nombres : chaque mot de sa sortie doit figurer dans son
//! propre code source (ou dans celui de l'écoute). Un titre de morceau, un nom d'artiste ou d'app
//! qui s'y glisserait ferait échouer le test. Ne joue aucun son, ne demande aucune permission.

use std::collections::HashSet;
use std::process::Command;

#[test]
#[ignore = "écoute la vraie sortie audio de cette session pendant quelques secondes ; à lancer à la main"]
fn listening_for_music_says_only_yes_no_and_numbers() {
    let directory = std::env::temp_dir().join(format!("capri-test-musique-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let output = Command::new(env!("CARGO_BIN_EXE_capri"))
        .args(["music", "rehearse"])
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
        assert!(stdout.contains("joue : "), "{stdout}");
    } else {
        assert!(stdout.contains("elle ne sait pas"), "{stdout}");
    }
    assert!(!directory.exists(), "aucun réglage n'a été écrit");

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
