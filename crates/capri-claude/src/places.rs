//! Où vivent les réglages (ceux de Capri, et ceux de Claude Code) et le journal de l'app.
//!
//! `capri-hook` et l'app doivent trouver **le même** dossier de Capri (le hook y lit si le miroir
//! est activé, l'app l'y écrit) : le calcul est fait ici, une seule fois. Il est pur : l'appelant
//! lui passe de quoi lire l'environnement (`std::env::var_os` en vrai, une table dans les tests) et
//! le système visé. Une variable vide compte pour absente.
//!
//! Le fichier du journal ([`log_file`]) n'est que celui de l'app à fenêtre : le hook n'écrit rien,
//! nulle part.

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
