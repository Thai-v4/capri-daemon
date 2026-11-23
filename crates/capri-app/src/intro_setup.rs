//! Le réglage « Intro au démarrage », et la dernière intro jouée au lancement, côté fichiers.
//!
//! `intro.json`, dans le dossier de Capri, dit deux choses : `"enabled": false` quand on lui a
//! retiré son intro depuis son menu (sans ce fichier, l'intro se joue au lancement, et elle dit
//! bonjour après) ; et `"last"`, le nom court de la dernière intro tirée au lancement
//! (`capri_core::intro::Entry::key`), pour ne pas jouer la même deux lancements de suite. Il ne
//! contient rien d'autre. Un fichier illisible : l'intro se joue, tirée sans contrainte ; un nom
//! inconnu, ou qui n'est pas un texte, ne contraint rien.
//!
//! **Jamais pendant un essai** ([`testing`]) : dès que `CAPRI_CONFIG_DIR` est posée, ou qu'une
//! répétition de la vraie app est demandée (`update_setup::REHEARSALS` : sa variable là, vide ou
//! non, comme les répétitions elles-mêmes le comprennent), pas d'intro ni de bonjour, quoi que dise
//! la case : elle se matérialise tout de suite, comme sans intro, et `intro.json` n'est pas écrit.
//! Les essais qui lancent `capri` posent tous `CAPRI_CONFIG_DIR` (un essai de `update_setup` le
//! vérifie) : ce qu'ils attendent (un message au-dessus de sa tête, sa voix, ses annonces) n'est ni
//! retardé ni précédé d'une phrase de plus.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use capri_claude::places::CAPRI_DIR_ENV;

use capri_core::intro::Entry;

use crate::brain_setup::read_text;
use crate::update_setup;

const INTRO_FILE: &str = "intro.json";

/// Ce que retient `intro.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroSettings {
    /// L'intro se joue-t-elle au lancement ?
    pub enabled: bool,
    /// Le nom court de la dernière intro tirée au lancement, s'il y en a un.
    pub last: Option<String>,
}

impl Default for IntroSettings {
    /// Sans fichier : l'intro se joue, tirée sans contrainte.
    fn default() -> Self {
        Self { enabled: true, last: None }
    }
}

/// Ce que dit `intro.json` ; sans fichier, le défaut (l'intro se joue).
pub fn load(directory: &Path) -> Result<IntroSettings> {
    let file = directory.join(INTRO_FILE);
    let Some(text) = read_text(&file)? else { return Ok(IntroSettings::default()) };
    parse(&text).with_context(|| format!("réglage illisible ({})", file.display()))
}

/// Retient la case de son menu et la dernière intro tirée au lancement.
pub fn save(directory: &Path, settings: &IntroSettings) -> Result<PathBuf> {
    let last = match &settings.last {
        Some(key) => format!(",\n  \"last\": {}", serde_json::to_string(key).context("son nom")?),
        None => String::new(),
    };
    let text = format!("{{\n  \"enabled\": {}{last}\n}}\n", settings.enabled);
    crate::brain_setup::write_file(directory, INTRO_FILE, &text)
}

/// L'intro du lancement, d'après `intro.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opening {
    /// La case « Intro au démarrage », telle que retenue (cochée si le fichier est illisible).
    pub enabled: bool,
    /// L'intro tirée, s'il y en a une (case cochée, et pas pendant un essai).
    pub entry: Option<&'static Entry>,
}

/// L'intro du lancement : ce que dit `intro.json` dans `directory` (`None` : pas de dossier),
/// tirée par `draw` d'après la dernière jouée, puis retenue pour la prochaine fois. Pendant un essai
/// (`testing`), ni intro ni écriture. Un fichier illisible : elle joue l'intro, tirée sans
/// contrainte, et **le fichier n'est pas réécrit** (la dernière jouée n'est pas retenue cette
/// fois-là ; une ligne d'avertissement le dit) : seule la case de son menu le réécrit.
pub fn opening(
    directory: Option<&Path>,
    testing: bool,
    draw: impl FnOnce(Option<&str>) -> &'static Entry,
) -> Opening {
    let loaded = directory.map(load);
    let settings = match &loaded {
        Some(Ok(settings)) => settings.clone(),
        Some(Err(err)) => {
            log::warn!("réglage « Intro au démarrage » illisible, elle la joue : {err:#}");
            IntroSettings::default()
        }
        None => IntroSettings::default(),
    };
    if !settings.enabled || testing {
        return Opening { enabled: settings.enabled, entry: None };
    }
    let entry = draw(settings.last.as_deref());
    match (directory, &loaded) {
        (Some(directory), Some(Ok(_))) => {
            let remembered = IntroSettings { last: Some(entry.key.to_owned()), ..settings.clone() };
            if let Err(err) = save(directory, &remembered) {
                log::warn!("intro : la dernière jouée pas retenue : {err:#}");
            }
        }
        // illisible : jamais réécrit au lancement
        (Some(_), Some(Err(_))) => {
            log::warn!("intro : la dernière jouée pas retenue, son réglage est illisible (laissé tel quel)");
        }
        _ => {}
    }
    Opening { enabled: settings.enabled, entry: Some(entry) }
}

/// Est-ce un essai (`env` : `std::env::var_os` en vrai, une table dans les essais) ?
/// `CAPRI_CONFIG_DIR` (vide, elle compte pour absente), ou une répétition de la vraie app, dès que
/// sa variable est là, vide ou non (`update_setup::rehearsing`, la règle des répétitions).
pub fn testing(env: impl Fn(&str) -> Option<OsString>) -> bool {
    let set_dir = env(CAPRI_DIR_ENV).is_some_and(|value| !value.is_empty());
    set_dir || update_setup::rehearsing(&env)
}

/// Un objet JSON ; `enabled` absent vaut le défaut (l'intro se joue) ; `last` absent, ou qui n'est
/// pas un texte, ne contraint rien ; un champ inconnu est ignoré.
fn parse(text: &str) -> Result<IntroSettings> {
    let value: serde_json::Value = serde_json::from_str(text).context("ce n'est pas du JSON")?;
    let Some(object) = value.as_object() else { bail!("ce n'est pas un objet JSON") };
    let enabled = match object.get("enabled") {
        None => true,
        Some(serde_json::Value::Bool(enabled)) => *enabled,
        Some(_) => bail!("« enabled » n'est ni true ni false"),
    };
    let last = object.get("last").and_then(serde_json::Value::as_str).map(str::to_owned);
    Ok(IntroSettings { enabled, last })
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
    fn the_intro_plays_by_default_and_the_choice_and_the_last_one_are_remembered() {
        let directory = temporary("retenu");
        assert_eq!(load(&directory).expect("sans fichier"), IntroSettings::default());
        assert!(IntroSettings::default().enabled, "par défaut, l'intro se joue");
        assert!(!directory.exists(), "lire n'écrit rien");
        let off = IntroSettings { enabled: false, last: None };
        let file = save(&directory, &off).expect("écrit");
        assert_eq!(file, directory.join("intro.json"));
        assert_eq!(std::fs::read_to_string(&file).expect("lu"), "{\n  \"enabled\": false\n}\n");
        assert_eq!(load(&directory).expect("relu"), off, "décochée, d'un lancement à l'autre");
        let drawn = IntroSettings { enabled: true, last: Some("tombe".to_owned()) };
        save(&directory, &drawn).expect("écrit");
        assert_eq!(
            std::fs::read_to_string(&file).expect("lu"),
            "{\n  \"enabled\": true,\n  \"last\": \"tombe\"\n}\n"
        );
        assert_eq!(load(&directory).expect("relu"), drawn);
        std::fs::remove_dir_all(&directory).expect("nettoyé");
    }

    #[test]
    fn a_strange_file_is_said_not_guessed() {
        assert_eq!(parse("{}").expect("vide"), IntroSettings::default(), "le défaut");
        assert!(!parse(r#"{"enabled": false, "autre": 3}"#).expect("champ inconnu ignoré").enabled);
        for text in ["", "true", "[]", r#"{"enabled": "non"}"#, r#"{"enabled": 0}"#] {
            assert!(parse(text).is_err(), "{text}");
        }
        // un nom qui n'est pas un texte ne contraint rien, et ne rend pas le fichier illisible
        let odd = parse(r#"{"enabled": false, "last": 4}"#).expect("lisible");
        assert_eq!(odd, IntroSettings { enabled: false, last: None });
        // un nom inconnu est gardé tel quel : le tirage l'ignore (`capri_core::intro::draw`)
        let unknown = parse(r#"{"last": "disparue"}"#).expect("lisible");
        assert_eq!(unknown.last.as_deref(), Some("disparue"));
        assert!(capri_core::intro::by_key("disparue").is_none());
    }

    #[test]
    fn at_launch_the_drawn_intro_is_remembered_but_an_unreadable_file_is_never_rewritten() {
        use capri_core::intro::{LIST, by_key};
        let directory = temporary("lancement");
        // sans fichier : tirée sans contrainte, puis retenue
        let mut asked = Vec::new();
        let opening = opening(Some(&directory), false, |last| {
            asked.push(last.map(str::to_owned));
            &LIST[2]
        });
        assert_eq!(asked, [None]);
        assert_eq!(opening, Opening { enabled: true, entry: Some(&LIST[2]) });
        assert_eq!(load(&directory).expect("relu").last.as_deref(), Some(LIST[2].key));
        // le lancement suivant tire d'après elle
        let next = super::opening(Some(&directory), false, |last| {
            assert_eq!(last, Some(LIST[2].key));
            &LIST[3]
        });
        assert_eq!(next.entry, Some(&LIST[3]));
        // décochée : ni tirage ni écriture
        save(&directory, &IntroSettings { enabled: false, last: Some("tombe".to_owned()) }).expect("écrit");
        let before = std::fs::read_to_string(directory.join("intro.json")).expect("lu");
        let off = super::opening(Some(&directory), false, |_| panic!("pas de tirage"));
        assert_eq!(off, Opening { enabled: false, entry: None });
        assert_eq!(std::fs::read_to_string(directory.join("intro.json")).expect("lu"), before);
        // pendant un essai : ni tirage ni écriture
        std::fs::remove_file(directory.join("intro.json")).expect("retiré");
        let testing = super::opening(Some(&directory), true, |_| panic!("pas de tirage"));
        assert_eq!(testing, Opening { enabled: true, entry: None });
        assert!(!directory.join("intro.json").exists());
        // un fichier illisible (édité à la main) : elle joue l'intro, et il reste tel quel
        let hand_written = "{ \"enabled\": tru";
        std::fs::write(directory.join("intro.json"), hand_written).expect("écrit");
        let unreadable = super::opening(Some(&directory), false, |last| {
            assert_eq!(last, None, "tirée sans contrainte");
            by_key("rien").expect("rien")
        });
        assert!(unreadable.enabled && unreadable.entry.is_some());
        assert_eq!(std::fs::read_to_string(directory.join("intro.json")).expect("lu"), hand_written);
        std::fs::remove_dir_all(&directory).expect("nettoyé");
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
