//! Où vivent les réglages (ceux de Capri, et ceux de Claude Code), le journal de l'app et son
//! inscription à l'ouverture de session.
//!
//! `capri-hook` et l'app doivent trouver **le même** dossier de Capri (le hook y lit si le miroir
//! est activé, l'app l'y écrit) : le calcul est fait ici, une seule fois. Il est pur : l'appelant
//! lui passe de quoi lire l'environnement (`std::env::var_os` en vrai, une table dans les tests) et
//! le système visé. Une variable vide compte pour absente.
//!
//! Le fichier du journal ([`log_file`]) n'est que celui de l'app à fenêtre : le hook n'écrit rien,
//! nulle part. De même le dossier où elle s'inscrit à l'ouverture de session ([`login_agents`]),
//! qui suit la même règle de `CAPRI_CONFIG_DIR`.

use std::ffi::OsString;
use std::path::PathBuf;

/// Variable qui remplace le dossier des réglages de Capri (essais, seconde instance). Le hook et
/// l'app doivent la voir tous les deux, ou aucun des deux.
pub const CAPRI_DIR_ENV: &str = "CAPRI_CONFIG_DIR";
/// Variable de Claude Code qui remplace `~/.claude` : tout ce qui y vit (`settings.json`,
/// `output-styles/`…) est alors dans ce dossier
/// (<https://code.claude.com/docs/en/claude-directory>).
pub const CLAUDE_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

/// Le système pour lequel on calcule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    MacOs,
    Windows,
    /// Linux et les autres : les conventions XDG.
    Other,
}

impl System {
    /// Celui pour lequel ce programme est compilé.
    pub const CURRENT: Self = if cfg!(target_os = "macos") {
        Self::MacOs
    } else if cfg!(windows) {
        Self::Windows
    } else {
        Self::Other
    };
}

/// Une variable d'environnement nécessaire manque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0} n'est pas définie")]
pub struct MissingVar(pub &'static str);

/// Le dossier des réglages de Capri : `CAPRI_CONFIG_DIR` si elle est définie, sinon le dossier
/// habituel du système (sur macOS, celui où vit aussi sa socket).
pub fn capri_dir(env: impl Fn(&str) -> Option<OsString>, system: System) -> Result<PathBuf, MissingVar> {
    let var = |name: &str| env(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    if let Some(directory) = var(CAPRI_DIR_ENV) {
        return Ok(directory);
    }
    let home = || var("HOME").ok_or(MissingVar("HOME"));
    Ok(match system {
        System::MacOs => home()?.join("Library/Application Support/Capri"),
        System::Windows => var("LOCALAPPDATA").ok_or(MissingVar("LOCALAPPDATA"))?.join("Capri"),
        System::Other => match var("XDG_CONFIG_HOME") {
            Some(config) => config.join("capri"),
            None => home()?.join(".config").join("capri"),
        },
    })
}

/// Le nom du fichier du journal de l'app.
pub const LOG_FILE_NAME: &str = "capri.log";
/// Le sous-dossier du journal dans le dossier de Capri, ailleurs que dans `~/Library/Logs`.
pub const LOG_SUBDIR: &str = "logs";

/// Le fichier du journal de l'app à fenêtre :
///
/// - macOS : `~/Library/Logs/Capri/capri.log`, dans le dossier des journaux de l'utilisateur
///   (c'est là que le système range les rapports de plantage de ses apps :
///   `~/Library/Logs/DiagnosticReports/`, d'après `man 8 ReportCrash`) ;
/// - ailleurs : `<dossier de Capri>/logs/capri.log` ([`capri_dir`] : `%LOCALAPPDATA%\Capri` sous
///   Windows, `$XDG_CONFIG_HOME/capri` ou `~/.config/capri` sous Linux). Ce que proposent pour les
///   journaux des bibliothèques comme `platformdirs` (un dossier `Logs` dans les données locales
///   sous Windows, `$XDG_STATE_HOME/<app>/log` sous Linux) n'a été vérifié ni dans la
///   documentation de Microsoft ni dans la spécification de freedesktop.org : à défaut, le
///   dossier de Capri lui-même ;
/// - partout, `CAPRI_CONFIG_DIR` définie : `<CAPRI_CONFIG_DIR>/logs/capri.log`, même sur macOS.
///   Les essais et les répétitions la posent : ils n'écrivent jamais dans le vrai dossier de
///   l'utilisateur.
pub fn log_file(env: impl Fn(&str) -> Option<OsString>, system: System) -> Result<PathBuf, MissingVar> {
    let var = |name: &str| env(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    if system == System::MacOs && var(CAPRI_DIR_ENV).is_none() {
        let home = var("HOME").ok_or(MissingVar("HOME"))?;
        return Ok(home.join("Library").join("Logs").join("Capri").join(LOG_FILE_NAME));
    }
    Ok(capri_dir(&env, system)?.join(LOG_SUBDIR).join(LOG_FILE_NAME))
}

/// Le sous-dossier du dossier de Capri qui tient lieu de `~/Library/LaunchAgents` quand
/// `CAPRI_CONFIG_DIR` est définie.
pub const LOGIN_AGENTS_SUBDIR: &str = "LaunchAgents";

/// Le dossier où elle s'inscrit à l'ouverture de session (voir [`login_agents`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginAgents {
    /// Le dossier des agents de lancement de l'utilisateur, `~/Library/LaunchAgents`, que
    /// `launchd` lit à l'ouverture de session : seul `capri_platform::login::user_agents_dir` le
    /// désigne.
    User,
    /// `<CAPRI_CONFIG_DIR>/LaunchAgents` : un dossier d'essai, que `launchd` ne lit jamais.
    Rehearsal(PathBuf),
}

/// Le dossier où elle s'inscrit à l'ouverture de session, sur macOS seulement (`None` ailleurs :
/// elle ne sait pas encore s'y inscrire) :
///
/// - `CAPRI_CONFIG_DIR` définie : `<CAPRI_CONFIG_DIR>/LaunchAgents`, et **rien** n'est lu ni écrit
///   dans le vrai. Comme son journal ([`log_file`]) : les essais et les répétitions de la vraie
///   app posent cette variable. Une Capri qui tourne avec d'autres réglages que ceux de
///   l'utilisateur n'a rien à faire dans son vrai dossier : son agent (un seul nom de fichier,
///   `capri_core::login::FILE_NAME`) lance l'app sans aucune variable, donc avec les vrais
///   réglages, et prendrait la place du sien ;
/// - sinon, le vrai ([`LoginAgents::User`]), comme avant.
pub fn login_agents(env: impl Fn(&str) -> Option<OsString>, system: System) -> Option<LoginAgents> {
    if system != System::MacOs {
        return None;
    }
    let var = |name: &str| env(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    Some(match var(CAPRI_DIR_ENV) {
        Some(directory) => LoginAgents::Rehearsal(directory.join(LOGIN_AGENTS_SUBDIR)),
        None => LoginAgents::User,
    })
}

/// Le dossier de Claude Code : `CLAUDE_CONFIG_DIR` si elle est définie, sinon `~/.claude`
/// (`%USERPROFILE%\.claude` sous Windows).
pub fn claude_dir(env: impl Fn(&str) -> Option<OsString>, system: System) -> Result<PathBuf, MissingVar> {
    let var = |name: &str| env(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    if let Some(directory) = var(CLAUDE_DIR_ENV) {
        return Ok(directory);
    }
    let home = if system == System::Windows { "USERPROFILE" } else { "HOME" };
    Ok(var(home).ok_or(MissingVar(home))?.join(".claude"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use super::*;

    /// Un environnement de test : seulement ces variables.
    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let vars: HashMap<String, OsString> =
            vars.iter().map(|(name, value)| ((*name).to_owned(), OsString::from(value))).collect();
        move |name| vars.get(name).cloned()
    }

    #[test]
    fn capri_lives_where_each_system_keeps_its_settings() {
        let home = env(&[("HOME", "/Users/moi"), ("LOCALAPPDATA", "C:\\Users\\moi\\AppData\\Local")]);
        assert_eq!(
            capri_dir(&home, System::MacOs).unwrap(),
            Path::new("/Users/moi/Library/Application Support/Capri")
        );
        assert_eq!(
            capri_dir(&home, System::Windows).unwrap(),
            Path::new("C:\\Users\\moi\\AppData\\Local").join("Capri")
        );
        assert_eq!(capri_dir(&home, System::Other).unwrap(), Path::new("/Users/moi/.config/capri"));
        let xdg = env(&[("HOME", "/home/moi"), ("XDG_CONFIG_HOME", "/home/moi/conf")]);
        assert_eq!(capri_dir(&xdg, System::Other).unwrap(), Path::new("/home/moi/conf/capri"));
    }

    #[test]
    fn capri_config_dir_wins_everywhere_unless_it_is_empty() {
        for system in [System::MacOs, System::Windows, System::Other] {
            let chosen = env(&[("HOME", "/Users/moi"), (CAPRI_DIR_ENV, "/tmp/essai-capri")]);
            assert_eq!(capri_dir(&chosen, system).unwrap(), Path::new("/tmp/essai-capri"));
            let empty = env(&[("HOME", "/Users/moi"), ("LOCALAPPDATA", "C:\\L"), (CAPRI_DIR_ENV, "")]);
            assert_ne!(capri_dir(&empty, system).unwrap(), Path::new(""));
        }
    }

    #[test]
    fn a_missing_home_is_said_by_name() {
        assert_eq!(capri_dir(env(&[]), System::MacOs), Err(MissingVar("HOME")));
        assert_eq!(capri_dir(env(&[("HOME", "")]), System::Other), Err(MissingVar("HOME")));
        assert_eq!(capri_dir(env(&[("HOME", "/h")]), System::Windows), Err(MissingVar("LOCALAPPDATA")));
        assert_eq!(MissingVar("HOME").to_string(), "HOME n'est pas définie");
        assert_eq!(claude_dir(env(&[("HOME", "/h")]), System::Windows), Err(MissingVar("USERPROFILE")));
    }

    #[test]
    fn claude_code_lives_in_dot_claude_or_in_claude_config_dir() {
        let home = env(&[("HOME", "/Users/moi"), ("USERPROFILE", "C:\\Users\\moi")]);
        assert_eq!(claude_dir(&home, System::MacOs).unwrap(), Path::new("/Users/moi/.claude"));
        assert_eq!(claude_dir(&home, System::Other).unwrap(), Path::new("/Users/moi/.claude"));
        assert_eq!(claude_dir(&home, System::Windows).unwrap(), Path::new("C:\\Users\\moi").join(".claude"));
        let chosen = env(&[("HOME", "/Users/moi"), (CLAUDE_DIR_ENV, "/tmp/essai-claude")]);
        assert_eq!(claude_dir(&chosen, System::MacOs).unwrap(), Path::new("/tmp/essai-claude"));
        let empty = env(&[("HOME", "/Users/moi"), (CLAUDE_DIR_ENV, "")]);
        assert_eq!(claude_dir(&empty, System::MacOs).unwrap(), Path::new("/Users/moi/.claude"));
    }

    #[test]
    fn the_journal_lives_in_library_logs_on_macos_and_in_capri_s_folder_elsewhere() {
        let home = env(&[
            ("HOME", "/Users/moi"),
            ("LOCALAPPDATA", "C:\\Users\\moi\\AppData\\Local"),
            ("XDG_STATE_HOME", "/Users/moi/etat"),
        ]);
        assert_eq!(
            log_file(&home, System::MacOs).unwrap(),
            Path::new("/Users/moi/Library/Logs/Capri/capri.log")
        );
        assert_eq!(
            log_file(&home, System::Windows).unwrap(),
            Path::new("C:\\Users\\moi\\AppData\\Local").join("Capri").join("logs").join("capri.log")
        );
        // ailleurs, le dossier de Capri, pas celui d'« état » de XDG (pas vérifié dans sa spécification)
        assert_eq!(
            log_file(&home, System::Other).unwrap(),
            Path::new("/Users/moi/.config/capri/logs/capri.log")
        );
        let xdg = env(&[("HOME", "/home/moi"), ("XDG_CONFIG_HOME", "/home/moi/conf")]);
        assert_eq!(log_file(&xdg, System::Other).unwrap(), Path::new("/home/moi/conf/capri/logs/capri.log"));
        for system in [System::Windows, System::Other] {
            let file = log_file(&home, system).unwrap();
            assert_eq!(file.parent().unwrap().parent().unwrap(), capri_dir(&home, system).unwrap());
        }
    }

    #[test]
    fn with_capri_config_dir_the_journal_never_goes_to_the_real_folder() {
        for system in [System::MacOs, System::Windows, System::Other] {
            let chosen = env(&[
                ("HOME", "/Users/moi"),
                ("LOCALAPPDATA", "C:\\L"),
                ("XDG_CONFIG_HOME", "/c"),
                (CAPRI_DIR_ENV, "/tmp/essai-capri"),
            ]);
            let file = log_file(&chosen, system).unwrap();
            assert_eq!(file, Path::new("/tmp/essai-capri").join(LOG_SUBDIR).join(LOG_FILE_NAME));
            assert!(file.starts_with(capri_dir(&chosen, system).unwrap()), "{}", file.display());
            // une variable vide compte pour absente
            let empty = env(&[("HOME", "/Users/moi"), ("LOCALAPPDATA", "C:\\L"), (CAPRI_DIR_ENV, "")]);
            assert!(!log_file(&empty, system).unwrap().starts_with("/tmp/essai-capri"));
            assert!(log_file(env(&[(CAPRI_DIR_ENV, "/e")]), system).is_ok(), "sans dossier personnel");
        }
        // sans dossier personnel ni variable du système : rien, et la variable manquante est dite
        assert_eq!(log_file(env(&[]), System::MacOs), Err(MissingVar("HOME")));
        assert_eq!(log_file(env(&[("HOME", "")]), System::MacOs), Err(MissingVar("HOME")));
        assert_eq!(log_file(env(&[]), System::Other), Err(MissingVar("HOME")));
        assert_eq!(log_file(env(&[("HOME", "/h")]), System::Windows), Err(MissingVar("LOCALAPPDATA")));
    }

    #[test]
    fn the_login_folder_is_the_real_one_unless_capri_config_dir_is_set() {
        // sans la variable (ou vide) : le vrai, comme avant
        let home = env(&[("HOME", "/Users/moi")]);
        assert_eq!(login_agents(&home, System::MacOs), Some(LoginAgents::User));
        let empty = env(&[("HOME", "/Users/moi"), (CAPRI_DIR_ENV, "")]);
        assert_eq!(login_agents(&empty, System::MacOs), Some(LoginAgents::User));
        // avec : le dossier d'essai, dans celui de Capri, jamais sous le dossier personnel
        let chosen = env(&[("HOME", "/Users/moi"), (CAPRI_DIR_ENV, "/tmp/essai-capri")]);
        let Some(LoginAgents::Rehearsal(directory)) = login_agents(&chosen, System::MacOs) else {
            panic!("pas le dossier d'essai");
        };
        assert_eq!(directory, Path::new("/tmp/essai-capri").join(LOGIN_AGENTS_SUBDIR));
        assert!(directory.starts_with(capri_dir(&chosen, System::MacOs).unwrap()));
        assert!(!directory.starts_with("/Users/moi"));
        // à côté de son journal, qui suit la même règle
        assert_eq!(directory.parent(), log_file(&chosen, System::MacOs).unwrap().parent().unwrap().parent());
        // sans dossier personnel, le dossier d'essai suffit
        let alone = env(&[(CAPRI_DIR_ENV, "/e")]);
        assert_eq!(
            login_agents(alone, System::MacOs),
            Some(LoginAgents::Rehearsal(Path::new("/e").join("LaunchAgents")))
        );
        // ailleurs que sur macOS : rien, avec ou sans la variable
        for system in [System::Windows, System::Other] {
            assert_eq!(login_agents(&home, system), None);
            assert_eq!(login_agents(&chosen, system), None);
        }
    }

    #[test]
    fn this_program_knows_its_own_system() {
        let expected = if cfg!(target_os = "macos") {
            System::MacOs
        } else if cfg!(windows) {
            System::Windows
        } else {
            System::Other
        };
        assert_eq!(System::CURRENT, expected);
    }
}
