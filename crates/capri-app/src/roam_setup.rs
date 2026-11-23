//! Le réglage « Vivre sa vie », côté fichiers.
//!
//! `roam.json`, dans le dossier de Capri, ne dit qu'une chose : `{ "enabled": false }` quand on
//! le lui a interdit depuis son menu. Sans ce fichier, elle vit sa vie. Il ne contient rien
//! d'autre : ni fenêtre, ni place, ni heure.
//!
//! Et si une répétition de la vraie app est en cours ([`rehearsing`]) : elle ne change alors pas
//! de place d'elle-même.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::brain_setup::read_text;

const ROAM_FILE: &str = "roam.json";

/// Vit-elle sa vie ? Sans fichier : oui.
pub fn load(directory: &Path) -> Result<bool> {
    let file = directory.join(ROAM_FILE);
    let Some(text) = read_text(&file)? else { return Ok(true) };
    parse(&text).with_context(|| format!("réglage illisible ({})", file.display()))
}

/// Retient le choix fait dans son menu, qui n'existe que sur macOS et Windows.
#[cfg(any(target_os = "macos", windows, test))]
pub fn save(directory: &Path, enabled: bool) -> Result<std::path::PathBuf> {
    crate::brain_setup::write_file(directory, ROAM_FILE, &format!("{{\n  \"enabled\": {enabled}\n}}\n"))
}

/// Une répétition de la vraie app est-elle demandée (la règle des répétitions, écrite une fois
/// dans `update_setup` : leur variable est là, vide ou non) ? Ses départs d'elle-même fausseraient
/// ce que la répétition mesure : le suivi d'une fenêtre (elle la quitterait), la fenêtre de
/// réglages, les annonces de musique. `CAPRI_CONFIG_DIR` seule n'en est pas une : sa vie peut se
/// répéter dans un dossier d'essai.
pub use crate::update_setup::rehearsing;

/// Un objet JSON ; `enabled` absent vaut le défaut (elle vit sa vie), un champ inconnu est ignoré.
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
    use std::ffi::OsString;

    use super::*;
    use crate::update_setup::REHEARSALS;

    fn temporary(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!("capri-test-vie-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn she_lives_her_life_by_default_and_the_choice_is_remembered() {
        let directory = temporary("retenu");
        assert!(load(&directory).expect("sans fichier"), "par défaut, elle vit sa vie");
        assert!(!directory.exists(), "lire n'écrit rien");
        let file = save(&directory, false).expect("écrit");
        assert_eq!(file, directory.join("roam.json"));
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
    fn only_a_rehearsal_of_the_real_app_keeps_her_from_leaving_on_her_own() {
        assert!(!rehearsing(table(vec![])));
        let alone = table(vec![(capri_claude::places::CAPRI_DIR_ENV, "/tmp/essai")]);
        assert!(!rehearsing(alone), "un dossier d'essai seul");
        for name in REHEARSALS {
            assert!(rehearsing(table(vec![(name, "1")])), "{name}");
            // la répétition démarre sur la seule présence de sa variable : vide, c'en est une
            assert!(rehearsing(table(vec![(name, "")])), "{name} vide");
        }
    }
}
