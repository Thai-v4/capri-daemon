//! Le réglage « Se balancer sur la musique », côté fichiers.
//!
//! `music.json`, dans le dossier de Capri, ne dit qu'une chose : `{ "enabled": false }` quand on
//! le lui a interdit depuis son menu. Sans ce fichier, c'est permis. Il ne contient rien d'autre :
//! ni morceau, ni app, ni moment.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::brain_setup::read_text;

const MUSIC_FILE: &str = "music.json";

/// Se balance-t-elle sur la musique ? Sans fichier : oui.
pub fn load(directory: &Path) -> Result<bool> {
    let file = directory.join(MUSIC_FILE);
    let Some(text) = read_text(&file)? else { return Ok(true) };
    parse(&text).with_context(|| format!("réglage illisible ({})", file.display()))
}

/// Retient le choix fait dans son menu, qui n'existe que sur macOS et Windows.
#[cfg(any(target_os = "macos", windows, test))]
pub fn save(directory: &Path, enabled: bool) -> Result<std::path::PathBuf> {
    crate::brain_setup::write_file(directory, MUSIC_FILE, &format!("{{\n  \"enabled\": {enabled}\n}}\n"))
}

/// Un objet JSON ; `enabled` absent vaut le défaut (permis), un champ inconnu est ignoré.
fn parse(text: &str) -> Result<bool> {
    let value: serde_json::Value = serde_json::from_str(text).context("ce n'est pas du JSON")?;
    let Some(object) = value.as_object() else { bail!("ce n'est pas un objet JSON") };
    match object.get("enabled") {
        None => Ok(true),
        Some(serde_json::Value::Bool(enabled)) => Ok(*enabled),
        Some(_) => bail!("« enabled » n'est ni true ni false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("capri-test-musique-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn allowed_by_default_and_remembered_from_one_launch_to_the_next() {
        let directory = temporary("retenu");
        assert!(load(&directory).expect("sans fichier"), "permis par défaut");
        let file = save(&directory, false).expect("écrit");
        assert_eq!(file, directory.join("music.json"));
        assert_eq!(std::fs::read_to_string(&file).expect("lu"), "{\n  \"enabled\": false\n}\n");
        assert!(!load(&directory).expect("relu"), "interdit, d'un lancement à l'autre");
        save(&directory, true).expect("écrit");
        assert!(load(&directory).expect("relu"));
        std::fs::remove_dir_all(&directory).expect("nettoyé");
    }

    #[test]
    fn a_strange_file_is_said_not_guessed() {
        assert!(parse("{}").expect("vide"), "le défaut");
        assert!(!parse(r#"{"enabled": false, "autre": 3}"#).expect("champ inconnu ignoré"));
        for text in ["", "true", "[]", r#"{"enabled": "non"}"#, r#"{"enabled": 0}"#] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}
