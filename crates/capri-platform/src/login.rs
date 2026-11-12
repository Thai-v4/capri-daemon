//! Se lancer à l'ouverture de session : écrire, lire et retirer notre agent de lancement.
//!
//! Le texte de l'agent, et la façon de reconnaître le nôtre, sont décidés par
//! `capri_core::login`. Ici, on regarde ce qu'il y a dans le dossier, on écrit d'un seul coup (un
//! fichier temporaire, puis un renommage) et on retire. Le dossier est **passé en paramètre** :
//! seul [`user_agents_dir`] désigne le vrai (`~/Library/LaunchAgents`), et les essais en
//! utilisent un autre. Trois règles :
//! - on n'écrit qu'à la demande de l'utilisateur (la case de son menu), ou pour réparer notre
//!   propre agent quand l'app a changé de place ;
//! - on n'écrase ni ne retire jamais un fichier qui n'est pas le nôtre, ni un lien, ni ce qui
//!   n'est pas un fichier ordinaire ;
//! - on ne demande rien à `launchd` : le fichier suffit, il le charge à la prochaine ouverture de
//!   session. Retiré, il n'est plus chargé à la suivante ; rien d'autre n'est touché.
//!
//! Pourquoi un agent de lancement plutôt que `SMAppService` (macOS 13 et suivants) : d'après
//! `SMAppService.h`, « Apps that use SMAppService APIs must be code signed », et une app mal signée
//! reçoit `kSMErrorInvalidSignature` ; l'inscription est rattachée à l'app signée, et « if an app
//! updates either the plist or the executable […] the SMAppService must be re-registered or it may
//! not launch ». Capri est signée « ad hoc » et reconstruite souvent : sa signature change à chaque
//! fois. Un agent de lancement est un fichier, lié à un chemin, qui ne dépend pas de la signature,
//! survit aux reconstructions et se vérifie sans écran. `SMAppService.h` le désigne comme la voie
//! « legacy » (`statusForLegacyURL:`), toujours chargée par `launchd`.
//!
//! Ailleurs que sur macOS : [`LoginError::Unavailable`].

use std::io::Write;
use std::path::{Path, PathBuf};

pub use capri_core::login::LoginState;
use capri_core::login::{self, AgentError, FILE_NAME};

/// Ce système sait-il l'inscrire à l'ouverture de session ?
pub const AVAILABLE: bool = cfg!(target_os = "macos");

/// Le nôtre fait moins d'un kilo-octet : un fichier plus gros n'est pas lu, et pas le nôtre.
const MAX_READ_BYTES: u64 = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("elle ne sait pas encore se lancer à l'ouverture de session sur ce système")]
    Unavailable,
    #[error("le dossier de l'utilisateur est inconnu (HOME n'est pas défini)")]
    NoHome,
    #[error("{} existe déjà et n'a pas été écrit par Capri : elle n'y touche pas", .0.display())]
    Foreign(PathBuf),
    #[error("{} a changé pendant l'opération : rien n'a été fait, réessaie", .0.display())]
    Changed(PathBuf),
    #[error("inscription impossible : {0}")]
    App(AgentError),
    #[error("{what} de {} : {source}", path.display())]
    Io { what: &'static str, path: PathBuf, source: std::io::Error },
}

fn io<'a>(what: &'static str, path: &'a Path) -> impl FnOnce(std::io::Error) -> LoginError + 'a {
    move |source| LoginError::Io { what, path: path.to_owned(), source }
}

/// Le dossier des agents de lancement de l'utilisateur : `~/Library/LaunchAgents`. Ailleurs que
/// sur macOS, [`LoginError::Unavailable`].
pub fn user_agents_dir() -> Result<PathBuf, LoginError> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").filter(|home| !home.is_empty()).ok_or(LoginError::NoHome)?;
        Ok(PathBuf::from(home).join("Library").join("LaunchAgents"))
    }
    #[cfg(not(target_os = "macos"))]
    Err(LoginError::Unavailable)
}

/// Le fichier de notre agent dans ce dossier.
pub fn agent_file(directory: &Path) -> PathBuf {
    directory.join(FILE_NAME)
}

/// Où en est l'inscription dans ce dossier. Ne change rien.
pub fn status(directory: &Path) -> Result<LoginState, LoginError> {
    look(&agent_file(directory)).map(|(state, _)| state)
}

/// Ce qu'une inscription a fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registered {
    /// Il n'y avait rien : le fichier est écrit.
    Written,
    /// Le nôtre est remplacé (une autre app, ou une autre version du texte).
    Updated,
    /// Le nôtre, à jour pour cette app, était déjà là : rien n'est écrit.
    AlreadyThere,
}

/// Inscrit l'app `app` (son chemin absolu, `/…/Capri.app`) dans ce dossier, ou met notre agent à
/// jour. Refuse s'il y a là un fichier qui n'est pas le nôtre.
pub fn register(directory: &Path, app: &str) -> Result<Registered, LoginError> {
    let text = login::agent_text(app).map_err(LoginError::App)?;
    let file = agent_file(directory);
    let (state, existing) = look(&file)?;
    let done = match state {
        LoginState::Foreign => return Err(LoginError::Foreign(file)),
        LoginState::Ours { .. } if existing.as_deref() == Some(text.as_bytes()) => {
            return Ok(Registered::AlreadyThere);
        }
        LoginState::Ours { .. } => Registered::Updated,
        LoginState::Absent => Registered::Written,
    };
    std::fs::create_dir_all(directory).map_err(io("création", directory))?;
    replace_atomically(&file, existing.as_deref(), text.as_bytes())?;
    Ok(done)
}

/// Ce qu'un retrait a fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unregistered {
    Removed,
    /// Il n'y avait pas d'inscription de Capri.
    NothingThere,
}

/// Retire notre agent de ce dossier, et rien d'autre. Refuse s'il y a là un fichier qui n'est pas
/// le nôtre.
pub fn unregister(directory: &Path) -> Result<Unregistered, LoginError> {
    let file = agent_file(directory);
    let (state, existing) = look(&file)?;
    match state {
        LoginState::Absent => Ok(Unregistered::NothingThere),
        LoginState::Foreign => Err(LoginError::Foreign(file)),
        LoginState::Ours { .. } => {
            // quelqu'un a pu le remplacer entre-temps : on ne retire que ce qu'on a lu
            if std::fs::read(&file).ok() != existing {
                return Err(LoginError::Changed(file));
            }
            std::fs::remove_file(&file).map_err(io("suppression", &file))?;
            Ok(Unregistered::Removed)
        }
    }
}

/// Ce qu'il y a à la place du fichier, et son contenu s'il a été lu. Un lien, un dossier, un
/// fichier spécial ou trop gros ne sont ni lus ni à nous.
fn look(file: &Path) -> Result<(LoginState, Option<Vec<u8>>), LoginError> {
    match std::fs::symlink_metadata(file) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok((LoginState::Absent, None)),
        Err(err) => Err(io("lecture", file)(err)),
        Ok(meta) if !meta.file_type().is_file() || meta.len() > MAX_READ_BYTES => {
            Ok((LoginState::Foreign, None))
        }
        Ok(_) => {
            let bytes = std::fs::read(file).map_err(io("lecture", file))?;
            Ok((LoginState::of(Some(&bytes)), Some(bytes)))
        }
    }
}

/// Remplace `target` par `updated` d'un seul coup : un fichier temporaire (dont le nom ne finit
/// pas par `.plist`, pour que `launchd` ne le prenne jamais pour un agent), écrit, synchronisé,
/// lisible par tous et modifiable par l'utilisateur seul (`launchd` refuse un agent que d'autres
/// pourraient modifier), puis renommé. `original` : le contenu lu au départ ; s'il a changé
/// entre-temps, rien n'est écrit.
fn replace_atomically(target: &Path, original: Option<&[u8]>, updated: &[u8]) -> Result<(), LoginError> {
    let directory = target.parent().unwrap_or(Path::new("."));
    let temporary = directory.join(format!(".{FILE_NAME}.capri-tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    let written = (|| -> Result<(), LoginError> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o644);
        let mut file = options.open(&temporary).map_err(io("création", &temporary))?;
        file.write_all(updated).map_err(io("écriture", &temporary))?;
        file.sync_all().map_err(io("écriture", &temporary))?;
        drop(file);
        let current = match std::fs::read(target) {
            Ok(bytes) => Some(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(io("relecture", target)(err)),
        };
        if current.as_deref() != original {
            return Err(LoginError::Changed(target.to_owned()));
        }
        std::fs::rename(&temporary, target).map_err(io("remplacement", target))
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    const APP: &str = "/Users/moi/Github/capri-daemon/target/Capri.app";

    /// Un dossier d'agents qui n'existe pas encore, dans un dossier neuf propre à ce test : jamais
    /// le vrai `~/Library/LaunchAgents`.
    fn scratch(name: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = format!(
            "capri-test-ouverture-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let base = std::env::temp_dir().join(unique);
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base.join("LaunchAgents")
    }

    fn entries(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(directory)
            .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    fn clean(directory: &Path) {
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    #[test]
    fn registered_read_back_and_removed_without_leftovers() {
        let directory = scratch("aller-retour");
        assert_eq!(status(&directory).unwrap(), LoginState::Absent, "le dossier n'existe même pas");
        assert_eq!(register(&directory, APP).unwrap(), Registered::Written);
        let file = agent_file(&directory);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), login::agent_text(APP).unwrap());
        assert_eq!(
            status(&directory).unwrap(),
            LoginState::Ours { app: Some(APP.to_owned()), current: true }
        );
        assert_eq!(register(&directory, APP).unwrap(), Registered::AlreadyThere);
        assert_eq!(entries(&directory), [FILE_NAME], "aucun fichier temporaire ne traîne");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode & 0o022, 0, "ni le groupe ni les autres ne peuvent le modifier : {mode:o}");
            assert_eq!(mode & 0o444, 0o444, "launchd doit pouvoir le lire : {mode:o}");
        }

        // une autre app : notre agent est mis à jour
        assert_eq!(register(&directory, "/Applications/Capri.app").unwrap(), Registered::Updated);
        assert_eq!(
            status(&directory).unwrap(),
            LoginState::Ours { app: Some("/Applications/Capri.app".to_owned()), current: true }
        );

        assert_eq!(unregister(&directory).unwrap(), Unregistered::Removed);
        assert!(!file.exists());
        assert_eq!(unregister(&directory).unwrap(), Unregistered::NothingThere);
        assert!(entries(&directory).is_empty(), "le dossier reste, vide : il est à l'utilisateur");
        clean(&directory);
    }

    #[test]
    fn only_our_file_is_touched_and_the_others_are_left_alone() {
        let directory = scratch("voisins");
        std::fs::create_dir_all(&directory).unwrap();
        // l'agent d'une autre app, à côté du nôtre
        let neighbour = directory.join("com.exemple.autre.plist");
        std::fs::write(&neighbour, "<plist>à quelqu'un d'autre</plist>").unwrap();
        register(&directory, APP).unwrap();
        unregister(&directory).unwrap();
        assert_eq!(entries(&directory), ["com.exemple.autre.plist"]);
        assert_eq!(std::fs::read_to_string(&neighbour).unwrap(), "<plist>à quelqu'un d'autre</plist>");
        clean(&directory);
    }

    #[test]
    fn a_file_that_is_not_ours_is_never_overwritten_nor_removed() {
        let directory = scratch("pas-a-nous");
        std::fs::create_dir_all(&directory).unwrap();
        let file = agent_file(&directory);
        let theirs = login::agent_text(APP)
            .unwrap()
            .lines()
            .filter(|line| !line.starts_with(login::MARK))
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        std::fs::write(&file, &theirs).unwrap();
        assert_eq!(status(&directory).unwrap(), LoginState::Foreign);
        assert!(matches!(register(&directory, APP), Err(LoginError::Foreign(_))));
        assert!(matches!(unregister(&directory), Err(LoginError::Foreign(_))));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), theirs);

        // ni un dossier qui porte ce nom
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert_eq!(status(&directory).unwrap(), LoginState::Foreign);
        assert!(register(&directory, APP).is_err() && unregister(&directory).is_err());
        assert!(file.is_dir());
        // ni un fichier trop gros pour être le nôtre
        std::fs::remove_dir(&file).unwrap();
        std::fs::write(&file, vec![b' '; 70 * 1024]).unwrap();
        assert_eq!(status(&directory).unwrap(), LoginState::Foreign);
        assert!(unregister(&directory).is_err() && file.exists());
        clean(&directory);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_neither_followed_nor_replaced() {
        let directory = scratch("lien");
        std::fs::create_dir_all(&directory).unwrap();
        // même vers une copie de notre texte : on ne sait pas à qui est ce qu'il désigne
        let elsewhere = directory.parent().unwrap().join("ailleurs.plist");
        std::fs::write(&elsewhere, login::agent_text(APP).unwrap()).unwrap();
        let link = agent_file(&directory);
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert_eq!(status(&directory).unwrap(), LoginState::Foreign);
        assert!(register(&directory, APP).is_err() && unregister(&directory).is_err());
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), login::agent_text(APP).unwrap());
        clean(&directory);
    }

    #[test]
    fn a_bare_program_is_never_registered() {
        let directory = scratch("binaire");
        let refused = register(&directory, "/Users/moi/Github/capri-daemon/target/release/capri");
        assert!(matches!(refused, Err(LoginError::App(AgentError::NotAnApp))), "{refused:?}");
        assert!(!directory.exists(), "rien n'est créé");
        clean(&directory);
    }

    #[test]
    fn the_real_folder_is_the_user_s_launch_agents_on_macos_only() {
        // rien n'est écrit : on regarde seulement quel dossier serait le vrai
        match (user_agents_dir(), AVAILABLE) {
            (Ok(directory), true) => {
                assert!(directory.ends_with("Library/LaunchAgents"), "{}", directory.display());
            }
            (Err(LoginError::Unavailable), false) => {}
            (other, available) => panic!("{other:?} (disponible : {available})"),
        }
    }
}
