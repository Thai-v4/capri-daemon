//! Le réglage « Intro au démarrage », côté fichiers.
//!
//! `intro.json`, dans le dossier de Capri, ne dit qu'une chose : `{ "enabled": false }` quand on
//! la lui a retirée depuis son menu. Sans ce fichier, l'intro se joue au lancement (et elle dit
//! bonjour après). Il ne contient rien d'autre.
//!
//! **Jamais pendant un essai** ([`testing`]) : dès que `CAPRI_CONFIG_DIR` est posée, ou qu'une
//! répétition de la vraie app est demandée (`update_setup::REHEARSALS` : sa variable là, vide ou
//! non, comme les répétitions elles-mêmes le comprennent), pas d'intro ni de
//! bonjour, quoi que dise la case : elle se matérialise tout de suite, comme sans intro. Les essais
//! qui lancent `capri` posent tous `CAPRI_CONFIG_DIR` (un essai de `update_setup` le vérifie) : ce
//! qu'ils attendent (un message au-dessus de sa tête, sa voix, ses annonces) n'est ni retardé ni
//! précédé d'une phrase de plus.

use std::ffi::OsString;
use std::path::Path;

use anyhow::{Context, Result, bail};
use capri_claude::places::CAPRI_DIR_ENV;

use crate::brain_setup::read_text;
use crate::update_setup;

const INTRO_FILE: &str = "intro.json";

/// L'intro se joue-t-elle au lancement ? Sans fichier : oui.
pub fn load(directory: &Path) -> Result<bool> {
    let file = directory.join(INTRO_FILE);
    let Some(text) = read_text(&file)? else { return Ok(true) };
    parse(&text).with_context(|| format!("réglage illisible ({})", file.display()))
}

/// Retient le choix fait dans son menu, qui n'existe que sur macOS et Windows.
#[cfg(any(target_os = "macos", windows, test))]
pub fn save(directory: &Path, enabled: bool) -> Result<std::path::PathBuf> {
    crate::brain_setup::write_file(directory, INTRO_FILE, &format!("{{\n  \"enabled\": {enabled}\n}}\n"))
}

/// Est-ce un essai (`env` : `std::env::var_os` en vrai, une table dans les essais) ?
/// `CAPRI_CONFIG_DIR` (vide, elle compte pour absente), ou une répétition de la vraie app, dès que
/// sa variable est là, vide ou non (`update_setup::rehearsing`, la règle des répétitions).
pub fn testing(env: impl Fn(&str) -> Option<OsString>) -> bool {
    let set_dir = env(CAPRI_DIR_ENV).is_some_and(|value| !value.is_empty());
    set_dir || update_setup::rehearsing(&env)
}

/// Un objet JSON ; `enabled` absent vaut le défaut (l'intro se joue), un champ inconnu est ignoré.
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
        let directory = std::env::temp_dir().join(format!("capri-test-intro-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn the_intro_plays_by_default_and_the_choice_is_remembered() {
        let directory = temporary("retenu");
        assert!(load(&directory).expect("sans fichier"), "par défaut, l'intro se joue");
        assert!(!directory.exists(), "lire n'écrit rien");
        let file = save(&directory, false).expect("écrit");
        assert_eq!(file, directory.join("intro.json"));
        assert_eq!(std::fs::read_to_string(&file).expect("lu"), "{\n  \"enabled\": false\n}\n");
        assert!(!load(&directory).expect("relu"), "décochée, d'un lancement à l'autre");
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

    /// Un environnement d'essai.
    fn table(pairs: Vec<(&'static str, &'static str)>) -> impl Fn(&str) -> Option<OsString> {
        move |name| pairs.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value))
    }

    #[test]
    fn a_test_folder_or_a_rehearsal_means_no_intro() {
        assert!(!testing(table(vec![])), "la vraie app, sans rien de posé");
        assert!(!testing(table(vec![(CAPRI_DIR_ENV, "")])), "vide : comme absente");
        assert!(testing(table(vec![(CAPRI_DIR_ENV, "/tmp/essai")])));
        for name in update_setup::REHEARSALS {
            assert!(testing(table(vec![(name, "1")])), "{name}");
            // la répétition démarre sur la seule présence de sa variable : vide, c'en est une
            assert!(testing(table(vec![(name, "")])), "{name} vide");
        }
    }
}
