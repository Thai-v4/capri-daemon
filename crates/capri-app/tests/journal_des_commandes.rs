//! Ses commandes de terminal n'écrivent aucun journal dans un fichier : seule l'app à fenêtre le
//! fait. Lancées pour de vrai, avec des lieux d'essai (rien n'est lu ni écrit ailleurs) : le
//! dossier du journal (`<CAPRI_CONFIG_DIR>/logs`) n'apparaît jamais.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Un dossier d'essai, retiré quand l'essai le lâche, même en cas d'échec.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn capri(directory: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_capri"))
        .args(args)
        .env("CAPRI_CONFIG_DIR", directory.join("capri"))
        .env("CLAUDE_CONFIG_DIR", directory.join("claude"))
        .env("CAPRI_KEYCHAIN_SERVICE", "capri-test-journal-des-commandes")
        .env("CAPRI_SOCKET", directory.join("capri.sock"))
        .env("RUST_LOG", "trace")
        .output()
        .expect("capri se lance")
}

#[test]
fn terminal_commands_never_write_a_journal_file() {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("capri-test-journal-commandes-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&scratch.0);
    std::fs::create_dir_all(scratch.0.join("capri")).unwrap();
    std::fs::create_dir_all(scratch.0.join("claude")).unwrap();
    for args in [&["hook", "status"][..], &["hook"], &["update", "check"], &["inconnue"]] {
        let output = capri(&scratch.0, args);
        let said =
            format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(!said.is_empty(), "{args:?} : elle dit quelque chose");
        assert!(
            !scratch.0.join("capri").join("logs").exists(),
            "{args:?} : aucun journal en fichier\n{said}"
        );
    }
}
