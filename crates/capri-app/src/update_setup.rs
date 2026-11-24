//! Chercher les mises à jour, côté fichiers et terminal : la case de son menu, retenue dans
//! `update.json`, ce qui tourne ici ([`installed`]), où regarder ([`source`]), et
//! `capri update check`.
//!
//! `update.json`, dans le dossier de Capri, ne dit qu'une chose : `{ "enabled": false }` quand on
//! le lui a interdit depuis son menu. Sans ce fichier, elle regarde (décision de l'utilisateur, le
//! 23 novembre 2026, comme Droppy). Un fichier illisible vaut « ne regarde pas » : dans le doute,
//! rien ne part. Il ne contient rien d'autre : ni date, ni version vue.
//!
//! **Jamais GitHub depuis un essai** : dès que `CAPRI_CONFIG_DIR` est posée, ou qu'une répétition
//! de la vraie app est demandée ([`REHEARSALS`]), elle ne regarde nulle part, ni d'elle-même ni par
//! `capri update check`. En version de débogage seulement, [`TEST_URL_ENV`] peut désigner un faux
//! serveur **de cette machine** (`http://127.0.0.1:<port>/…`) pour la répéter : rien d'autre n'est
//! permis, et une adresse refusée n'est jamais remplacée par GitHub.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use capri_chat::update::{self, Installed, Offline, Source, Version, say};
use capri_claude::places::CAPRI_DIR_ENV;

use crate::brain_setup::read_text;

const UPDATE_FILE: &str = "update.json";

/// Les répétitions de la vraie app (version de débogage) : pendant l'une d'elles, elle ne regarde
/// pas les versions, même sans `CAPRI_CONFIG_DIR`. Un essai vérifie que la liste suit le code.
pub const REHEARSALS: [&str; 3] =
    ["CAPRI_SETTINGS_REHEARSAL", "CAPRI_PERCH_REHEARSAL", "CAPRI_MUSIC_REHEARSAL"];

/// En version de débogage seulement : l'adresse d'un faux serveur de cette machine, à la place de
/// GitHub. N'est pas lue dans la version livrée.
pub const TEST_URL_ENV: &str = "CAPRI_UPDATE_URL";

/// Avec un faux serveur, la première vérification n'attend pas une minute.
pub const REHEARSAL_GRACE: Duration = Duration::from_secs(3);

/// Le format de ce que `Capri.app` contient en plus de ses programmes : celui de cette version.
const BUNDLE_FORMAT: &str = include_str!("../../../tools/macos/bundle-format");

/// Ce qui tourne ici : sa version, le format du contenu de son app, sa cible.
pub fn installed() -> Installed {
    Installed {
        version: Version::parse(env!("CARGO_PKG_VERSION")).expect("la version du Cargo.toml est lisible"),
        bundle: update::bundle_format(BUNDLE_FORMAT).expect("tools/macos/bundle-format est lisible"),
        target: update::TARGET,
    }
}

/// Regarde-t-elle s'il existe une version plus récente ? Sans fichier : oui.
pub fn load(directory: &Path) -> Result<bool> {
    let file = directory.join(UPDATE_FILE);
    let Some(text) = read_text(&file)? else { return Ok(true) };
    parse(&text).with_context(|| format!("réglage illisible ({})", file.display()))
}

/// Retient le choix fait dans son menu, qui n'existe que sur macOS et Windows.
#[cfg(any(target_os = "macos", windows, test))]
pub fn save(directory: &Path, enabled: bool) -> Result<std::path::PathBuf> {
    crate::brain_setup::write_file(directory, UPDATE_FILE, &format!("{{\n  \"enabled\": {enabled}\n}}\n"))
}

/// Un objet JSON ; `enabled` absent vaut le défaut (elle regarde), un champ inconnu est ignoré.
fn parse(text: &str) -> Result<bool> {
    let value: serde_json::Value = serde_json::from_str(text).context("ce n'est pas du JSON")?;
    let Some(object) = value.as_object() else { bail!("ce n'est pas un objet JSON") };
    match object.get("enabled") {
        None => Ok(true),
        Some(serde_json::Value::Bool(enabled)) => Ok(*enabled),
        Some(_) => bail!("« enabled » n'est ni true ni false"),
    }
}

/// La case est-elle cochée ? Un réglage illisible vaut « non », et le journal le dit.
pub fn enabled() -> bool {
    crate::brain_setup::config_dir()
        .and_then(|directory| load(&directory))
        .inspect_err(|err| log::warn!("mises à jour : réglage illisible, elle ne regarde pas : {err:#}"))
        .unwrap_or(false)
}

/// Une répétition de la vraie app est-elle demandée (`env` : `std::env::var_os` en vrai, une
/// table dans les essais) ? La règle des répétitions elles-mêmes, qui démarrent sur la seule
/// présence de leur variable : là, vide ou non. Écrite ici une fois, pour tout ce qui doit s'en
/// garder : les versions ([`source`]), ses départs d'elle-même (`roam_setup`), l'intro
/// (`intro_setup`).
pub fn rehearsing(env: impl Fn(&str) -> Option<OsString>) -> bool {
    REHEARSALS.iter().any(|name| env(name).is_some())
}

/// Où regarder, d'après l'environnement (`env` : `std::env::var_os` en vrai, une table dans les
/// essais). `CAPRI_CONFIG_DIR` vide compte pour absente ; une répétition compte dès que sa variable
/// est là, vide ou non ([`rehearsing`]).
pub fn source(env: impl Fn(&str) -> Option<OsString>) -> Result<Source, Offline> {
    let set = |name: &str| env(name).is_some_and(|value| !value.is_empty());
    let rehearsal = set(CAPRI_DIR_ENV) || rehearsing(&env);
    let test_url =
        if cfg!(debug_assertions) { env(TEST_URL_ENV).filter(|value| !value.is_empty()) } else { None };
    let test_url = test_url.map(|url| url.to_string_lossy().into_owned());
    update::source(rehearsal, test_url.as_deref())
}

/// Terminal : pourquoi elle ne regarde nulle part.
fn offline(why: &Offline) -> String {
    match why {
        Offline::Rehearsal => format!(
            "{CAPRI_DIR_ENV} (ou une répétition de la vraie app) est posée : depuis un essai, rien ne part vers GitHub."
        ),
        Offline::NotThisMachine(err) => format!(
            "{TEST_URL_ENV} ne désigne pas un serveur de cette machine (http://127.0.0.1:<port>/…) : {err}. \
             Rien n'est vérifié, et GitHub ne la remplace pas."
        ),
    }
}

/// La commande `capri update <action>`. Rend le code de sortie du processus. `finish` est réservée
/// au programme de fin d'une mise à jour, que Capri lance elle-même (`update_finish`) : l'usage ne
/// la cite pas.
pub fn run_cli(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("finish") => return crate::update_finish::run_finish(&args[1..]),
        #[cfg(debug_assertions)]
        Some("rehearse") if args.len() == 1 => return crate::update_finish::run_rehearse(),
        _ => {}
    }
    if args.len() != 1 || args[0] != "check" {
        eprintln!(
            "usage : capri update check    la version installée, la dernière publiée, et si elle peut \
             s'installer (ne télécharge rien)"
        );
        return 2;
    }
    let installed = installed();
    println!("{}", say::installed(&installed));
    let source = match source(|name| std::env::var_os(name)) {
        Ok(source) => source,
        Err(why) => {
            println!("{}", offline(&why));
            return 1;
        }
    };
    let route = source.route();
    if route.is_secure() && !capri_net::TLS {
        println!("{}", update::Failure::NoTls.explain(&source));
        return 1;
    }
    // l'adresse de départ, jamais signée : elle peut s'écrire
    println!("Manifeste lu à    : {}", source.url());
    let fetched = capri_net::update::fetch(&route, &source.url(), capri_net::update::Limits::default());
    match fetched.and_then(|body| update::conclude(&body, &installed)) {
        Ok(verdict) => {
            println!("{}", say::terminal(&verdict));
            0
        }
        Err(failure) => {
            println!("Vérification impossible : {}", failure.explain(&source));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use capri_chat::update::Route;

    use super::*;

    fn temporary(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("capri-test-mises-a-jour-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        let table: HashMap<&str, &str> = vars.iter().copied().collect();
        move |name| table.get(name).map(OsString::from)
    }

    #[test]
    fn checked_by_default_and_remembered_from_one_launch_to_the_next() {
        let directory = temporary("retenu");
        assert!(load(&directory).expect("sans fichier"), "cochée par défaut");
        let file = save(&directory, false).expect("écrit");
        assert_eq!(file, directory.join("update.json"));
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

    #[test]
    fn what_runs_here_is_this_version_with_this_bundle_format() {
        let here = installed();
        assert_eq!(here.version.to_string(), env!("CARGO_PKG_VERSION"));
        assert_eq!(here.bundle, 2);
        assert_eq!(here.target, update::TARGET);
    }

    /// Le format de ce que `Capri.app` contient en plus de ses programmes suit la liste `IMAGES`
    /// de `tools/bundle-macos.sh` : une mise à jour faite dans l'app ne remplacera que ses
    /// programmes, et refusera une version d'un autre format. Si les images changent sans que le
    /// format change, l'app mise à jour garderait les anciennes.
    #[test]
    fn the_bundle_format_follows_the_images_the_app_carries() {
        /// Les images de `Capri.app` au format 1.
        const FORMAT_1: [&str; 10] = [
            "capri/capri-cutout.png",
            "capri/perch/perch.png",
            "capri/perch/perch_blink.png",
            "capri/perch/perch_talk.png",
            "capri/perch/perch_music.png",
            "capri/perch/perch_arrive.png",
            "capri/perch/perch_leave.png",
            "capri/stand/idle.png",
            "capri/stand/idle_blink.png",
            "capri/stand/idle_mouth.png",
        ];
        /// Au format 2, ses planches de vie en plus (elle apparaît, elle s'efface ; elle marche,
        /// elle pend, elle se relève, elle sourit ; elle dort debout, elle se réveille), ses
        /// planches facultatives assises et debout (un éclat, ses clignements glitchés), sa pose
        /// allongée, et le film de ses intros (« de tout près » ; le coucou, essayé un temps, en a été
        /// retiré).
        /// Cette liste a pu changer sans
        /// que le numéro change : aucune version n'a été publiée au format 2 (la 0.2.0, la dernière
        /// publiée, le 23 novembre 2026, est au format 1), et aucune app installée ne l'attend
        /// encore. Une fois une version publiée à ce format, elle ne change plus : toute nouvelle
        /// image demande le format 3.
        const FORMAT_2: [&str; 49] = [
            "capri/capri-cutout.png",
            "capri/perch/perch.png",
            "capri/perch/perch_blink.png",
            "capri/perch/perch_talk.png",
            "capri/perch/perch_music.png",
            "capri/perch/perch_arrive.png",
            "capri/perch/perch_leave.png",
            "capri/perch/perch_glitch.png",
            "capri/perch/perch_blink_glitch.png",
            "capri/stand/idle.png",
            "capri/stand/idle_blink.png",
            "capri/stand/idle_mouth.png",
            "capri/stand/idle_blink_glitch.png",
            "capri/life/arrive.png",
            "capri/life/leave.png",
            "capri/life/walk.png",
            "capri/life/dangle.png",
            "capri/life/glitch.png",
            "capri/life/grin.png",
            "capri/life/sleep.png",
            "capri/life/wake.png",
            "capri/laydown/laydown.png",
            "capri/laydown/laydown_blink.png",
            "capri/laydown/laydown_talk.png",
            "capri/laydown/laydown_sleep.png",
            "capri/laydown/laydown_glitch.png",
            "capri/laydown/laydown_leave.png",
            "capri/intro/01.png",
            "capri/intro/02.png",
            "capri/intro/03.png",
            "capri/intro/04.png",
            "capri/intro/05.png",
            "capri/intro/06.png",
            "capri/intro/07.png",
            "capri/intro/08.png",
            "capri/intro/09.png",
            "capri/intro/10.png",
            "capri/intro/11.png",
            "capri/intro/12.png",
            "capri/intro/13.png",
            "capri/intro/14.png",
            "capri/intro/15.png",
            "capri/intro/16.png",
            "capri/intro/17.png",
            "capri/intro/18.png",
            "capri/intro/19.png",
            "capri/intro/20.png",
            "capri/intro/21.png",
            "capri/intro/22.png",
        ];
        let script = include_str!("../../../tools/bundle-macos.sh");
        let mut listed: Vec<&str> = script
            .lines()
            .map(str::trim)
            .skip_while(|line| *line != "IMAGES=(")
            .skip(1)
            .take_while(|line| *line != ")")
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        listed.sort_unstable();
        let format = update::bundle_format(BUNDLE_FORMAT).expect("un entier");
        let mut expected = match format {
            1 => FORMAT_1.to_vec(),
            2 => FORMAT_2.to_vec(),
            other => panic!("format {other} : écris sa liste d'images dans cet essai"),
        };
        expected.sort_unstable();
        assert_eq!(
            listed, expected,
            "la liste IMAGES de tools/bundle-macos.sh a changé : augmente tools/macos/bundle-format \
             (et écris la liste du nouveau format dans cet essai)"
        );
    }

    #[test]
    fn no_rehearsal_and_no_test_folder_ever_looks_at_github() {
        assert_eq!(source(env(&[])), Ok(Source::GitHub), "la vraie app, sans rien de posé");
        assert_eq!(source(env(&[(CAPRI_DIR_ENV, "")])), Ok(Source::GitHub), "vide : comme absente");
        assert_eq!(source(env(&[(CAPRI_DIR_ENV, "/tmp/essai")])), Err(Offline::Rehearsal));
        for rehearsal in REHEARSALS {
            assert_eq!(source(env(&[(rehearsal, "1")])), Err(Offline::Rehearsal), "{rehearsal}");
            // la répétition démarre sur la seule présence de sa variable : vide, c'en est une
            assert_eq!(source(env(&[(rehearsal, "")])), Err(Offline::Rehearsal), "{rehearsal} vide");
            assert_eq!(
                source(env(&[(rehearsal, "1"), (CAPRI_DIR_ENV, "/tmp/essai")])),
                Err(Offline::Rehearsal)
            );
        }
        // un faux serveur de cette machine, en version de débogage seulement ; jamais ailleurs
        let local = "http://127.0.0.1:8123/releases/latest/download/latest.json";
        let found = source(env(&[(TEST_URL_ENV, local), (CAPRI_DIR_ENV, "/tmp/essai")]));
        if cfg!(debug_assertions) {
            assert_eq!(
                found,
                Ok(Source::ThisMachine {
                    url: local.to_owned(),
                    route: Route::this_machine(8123).expect("règle")
                })
            );
            let elsewhere = source(env(&[(TEST_URL_ENV, &update::manifest_url())]));
            assert!(matches!(elsewhere, Err(Offline::NotThisMachine(_))), "{elsewhere:?}");
        } else {
            assert_eq!(found, Err(Offline::Rehearsal), "la version livrée ignore {TEST_URL_ENV}");
            assert_eq!(source(env(&[(TEST_URL_ENV, local)])), Ok(Source::GitHub));
        }
    }

    /// Les répétitions que lit l'app sont toutes dans [`REHEARSALS`] : une répétition ajoutée plus
    /// tard ne pourra pas joindre GitHub sans qu'on le remarque.
    #[test]
    fn every_rehearsal_the_app_reads_is_known_here() {
        let sources = [
            include_str!("main.rs"),
            include_str!("perch_rehearsal.rs"),
            include_str!("perch_rehearsal/follow.rs"),
            include_str!("music_rehearsal.rs"),
            include_str!("settings_view.rs"),
        ];
        let mut found = Vec::new();
        for source in sources {
            let mut rest = source;
            while let Some(at) = rest.find("\"CAPRI_") {
                let name: String =
                    rest[at + 1..].chars().take_while(|c| c.is_ascii_uppercase() || *c == '_').collect();
                if name.ends_with("_REHEARSAL") && !found.contains(&name) {
                    found.push(name);
                }
                rest = &rest[at + 1..];
            }
        }
        assert!(!found.is_empty(), "aucune répétition trouvée : l'essai ne lit plus les bons fichiers ?");
        for name in &found {
            assert!(REHEARSALS.contains(&name.as_str()), "{name} n'est pas dans REHEARSALS");
        }
    }

    /// Chaque essai qui lance un binaire de Capri pose `CAPRI_CONFIG_DIR` : aucun ne peut joindre
    /// GitHub, ni par l'app ni par `capri update check`.
    #[test]
    fn every_test_that_launches_capri_sets_a_test_folder() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let mut launching = 0;
        for entry in std::fs::read_dir(&directory).expect("le dossier des essais") {
            let path = entry.expect("une entrée").path();
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("lisible");
            if text.contains("CARGO_BIN_EXE_capri") {
                launching += 1;
                assert!(
                    text.contains("\"CAPRI_CONFIG_DIR\""),
                    "{} lance Capri sans dossier d'essai",
                    path.display()
                );
            }
        }
        assert!(launching >= 4, "{launching} essais lancent Capri");
    }
}
