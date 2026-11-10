//! Le style de sortie « Capri » côté fichiers : l'installer dans le dossier de Claude Code, le
//! retirer, dire où il en est.
//!
//! Le texte, et la façon de reconnaître le nôtre, sont décidés par `capri_claude::style`. Ici, on
//! trouve le dossier (`output-styles/` du dossier de Claude Code, qui suit `CLAUDE_CONFIG_DIR`),
//! on regarde ce qui s'y trouve, et on écrit d'un seul coup (`hook_setup::replace_atomically`).
//! Trois règles :
//! - on n'écrit qu'à la demande de l'utilisateur (`capri hook style install`, ou le bouton de la
//!   fenêtre de réglages) ;
//! - on n'écrase ni ne retire jamais un fichier qui n'est pas le nôtre, ni un lien, ni ce qui
//!   n'est pas un fichier ordinaire ;
//! - on n'active pas le style : c'est l'utilisateur qui le choisit dans Claude Code, et
//!   `settings.json` n'est pas touché.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use capri_claude::style::{self, Installed, Removed, StyleState};

use crate::hook_setup;

/// Le nôtre fait quelques kilo-octets : un fichier plus gros n'est pas lu, et pas le nôtre.
const MAX_READ_BYTES: u64 = 64 * 1024;

/// Le dossier des styles de sortie de l'utilisateur.
pub fn styles_dir() -> Result<PathBuf> {
    Ok(hook_setup::claude_dir()?.join(style::DIRECTORY))
}

/// Le fichier du style, et ce qu'on y trouve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub file: PathBuf,
    pub state: StyleState,
}

/// Où en est le style dans ce dossier. Ne change rien.
pub fn status(directory: &Path) -> Result<Found> {
    let file = directory.join(style::FILE_NAME);
    let (state, _) = look(&file)?;
    Ok(Found { file, state })
}

/// Installe le style, ou le met à jour si c'est une autre version du nôtre. Refuse s'il y a là un
/// fichier qui n'est pas le nôtre.
pub fn install(directory: &Path) -> Result<(PathBuf, Installed)> {
    let file = directory.join(style::FILE_NAME);
    let (state, existing) = look(&file)?;
    let done = match state {
        StyleState::Foreign => bail!("{}", style::foreign(&file.display().to_string())),
        StyleState::Ours { current: true } => return Ok((file, Installed::AlreadyThere)),
        StyleState::Ours { current: false } => Installed::Updated,
        StyleState::Absent => Installed::Written,
    };
    std::fs::create_dir_all(directory).with_context(|| format!("création de {}", directory.display()))?;
    hook_setup::replace_atomically(&file, existing.as_deref(), style::TEXT.as_bytes())?;
    Ok((file, done))
}

/// Retire le style, s'il est le nôtre. Refuse s'il y a là un fichier qui n'est pas le nôtre.
pub fn uninstall(directory: &Path) -> Result<(PathBuf, Removed)> {
    let file = directory.join(style::FILE_NAME);
    let (state, existing) = look(&file)?;
    match state {
        StyleState::Absent => Ok((file, Removed::NothingThere)),
        StyleState::Foreign => bail!("{}", style::foreign(&file.display().to_string())),
        StyleState::Ours { .. } => {
            // quelqu'un a pu le remplacer entre-temps : on ne retire que ce qu'on a lu
            if std::fs::read(&file).ok() != existing {
                bail!("{} a changé pendant l'opération : rien n'a été retiré, réessaie", file.display());
            }
            std::fs::remove_file(&file).with_context(|| format!("suppression de {}", file.display()))?;
            Ok((file, Removed::Removed))
        }
    }
}

/// Ce qu'il y a à la place du fichier, et son contenu s'il a été lu. Un lien, un dossier, un
/// fichier spécial ou trop gros ne sont ni lus ni à nous.
fn look(file: &Path) -> Result<(StyleState, Option<Vec<u8>>)> {
    match std::fs::symlink_metadata(file) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok((StyleState::Absent, None)),
        Err(err) => Err(err).with_context(|| format!("lecture de {}", file.display())),
        Ok(meta) if !meta.file_type().is_file() || meta.len() > MAX_READ_BYTES => {
            Ok((StyleState::Foreign, None))
        }
        Ok(_) => {
            let bytes = std::fs::read(file).with_context(|| format!("lecture de {}", file.display()))?;
            Ok((StyleState::of(Some(&bytes)), Some(bytes)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// Un dossier `output-styles` qui n'existe pas encore, dans un dossier neuf propre à ce test.
    fn scratch_styles(name: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = format!(
            "capri-test-style-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let base = std::env::temp_dir().join(unique);
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base.join(style::DIRECTORY)
    }

    fn entries(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(directory)
            .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn installed_once_read_back_and_removed_without_leftovers() {
        let directory = scratch_styles("aller-retour");
        assert_eq!(status(&directory).unwrap().state, StyleState::Absent, "le dossier n'existe même pas");

        let (file, done) = install(&directory).unwrap();
        assert_eq!(done, Installed::Written);
        assert_eq!(file, directory.join("capri.md"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), style::TEXT);
        assert_eq!(
            status(&directory).unwrap(),
            Found { file: file.clone(), state: StyleState::Ours { current: true } }
        );
        assert_eq!(install(&directory).unwrap().1, Installed::AlreadyThere);
        assert_eq!(entries(&directory), ["capri.md"], "aucun fichier temporaire ne traîne");

        assert_eq!(uninstall(&directory).unwrap(), (file.clone(), Removed::Removed));
        assert!(!file.exists());
        assert_eq!(uninstall(&directory).unwrap().1, Removed::NothingThere);
        assert!(entries(&directory).is_empty(), "le dossier reste, vide : il est à Claude Code");
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    #[test]
    fn an_older_version_of_ours_is_updated() {
        let directory = scratch_styles("mise-a-jour");
        std::fs::create_dir_all(&directory).unwrap();
        let older = style::TEXT.replace("200 caractères", "150 caractères");
        std::fs::write(directory.join("capri.md"), &older).unwrap();
        assert_eq!(status(&directory).unwrap().state, StyleState::Ours { current: false });
        assert_eq!(install(&directory).unwrap().1, Installed::Updated);
        assert_eq!(std::fs::read_to_string(directory.join("capri.md")).unwrap(), style::TEXT);
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    #[test]
    fn a_file_that_is_not_ours_is_never_overwritten_nor_removed() {
        let directory = scratch_styles("pas-a-nous");
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("capri.md");
        let theirs = "---\nname: Capri\n---\n\nParle comme un pirate.\n";
        std::fs::write(&file, theirs).unwrap();
        std::fs::write(directory.join("autre.md"), "à quelqu'un d'autre").unwrap();
        assert_eq!(status(&directory).unwrap().state, StyleState::Foreign);
        for refused in [install(&directory).map(|_| ()), uninstall(&directory).map(|_| ())] {
            let error = format!("{:#}", refused.unwrap_err());
            assert!(error.contains("n'a pas été écrit par Capri"), "{error}");
        }
        assert_eq!(std::fs::read_to_string(&file).unwrap(), theirs);
        assert_eq!(entries(&directory), ["autre.md", "capri.md"]);

        // ni un dossier qui porte ce nom
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert_eq!(status(&directory).unwrap().state, StyleState::Foreign);
        assert!(install(&directory).is_err() && uninstall(&directory).is_err());
        assert!(file.is_dir());
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_neither_followed_nor_replaced() {
        let directory = scratch_styles("lien");
        std::fs::create_dir_all(&directory).unwrap();
        // même vers une copie de notre texte : on ne sait pas à qui est ce qu'il désigne
        let elsewhere = directory.parent().unwrap().join("ailleurs.md");
        std::fs::write(&elsewhere, style::TEXT).unwrap();
        let link = directory.join("capri.md");
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert_eq!(status(&directory).unwrap().state, StyleState::Foreign);
        assert!(install(&directory).is_err() && uninstall(&directory).is_err());
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), style::TEXT);
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }
}
