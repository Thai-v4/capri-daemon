//! Lancer un autre programme, sans l'attendre et sans jamais l'arrêter.
//!
//! Capri ne s'en sert que pour **Ollama**, et seulement quand c'est son cerveau sur cette machine
//! et que la case « Se lancer à l'ouverture de session » est cochée : c'est `capri_chat::startup`
//! qui décide, et qui donne les emplacements, fixes, où le chercher. Ici, on lance ce qu'on nous
//! donne, par son chemin absolu (jamais par le `PATH`), et rien d'autre.
//!
//! - un programme ([`start_program`]) : détaché de Capri (son propre groupe de processus, entrée
//!   et sorties fermées), il continue de tourner quand on la quitte, et un Ctrl+C dans le terminal
//!   qui l'a lancée ne l'atteint pas. Un thread à part attend sa fin, pour que le système n'en
//!   garde pas la trace (`waitpid`) ;
//! - une app ([`open_app`]) : par `/usr/bin/open -g -a`, comme un double-clic, sans la mettre au
//!   premier plan (`open(1)`) ; si elle tourne déjà, `open` ne fait que la retrouver.
//!
//! Ailleurs que sur macOS : [`LaunchError::Unavailable`].

use std::path::{Path, PathBuf};

/// Ce système sait-il lancer Ollama ?
pub const AVAILABLE: bool = cfg!(target_os = "macos");

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("elle ne sait pas encore lancer un programme sur ce système")]
    Unavailable,
    #[error("{} n'est pas un chemin absolu", .0.display())]
    NotAbsolute(PathBuf),
    #[error("lancement de {} : {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },
    #[error("open n'a pas pu ouvrir {} ({status})", path.display())]
    Open { path: PathBuf, status: std::process::ExitStatus },
}

/// Un programme qu'on peut lancer : un fichier ordinaire (ou un lien vers un tel fichier, comme
/// ceux de Homebrew), exécutable.
pub fn is_program(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false };
    #[cfg(unix)]
    let executable = std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111 != 0;
    #[cfg(not(unix))]
    let executable = true;
    meta.is_file() && executable
}

/// Une app : un dossier `….app` avec son `Contents/Info.plist`.
pub fn is_app(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "app") && path.join("Contents/Info.plist").is_file()
}

/// Lance `program` avec `args` et, en plus de l'environnement de Capri, `env`, sans l'attendre.
/// Rend son numéro de processus.
pub fn start_program(program: &Path, args: &[&str], env: &[(&str, String)]) -> Result<u32, LaunchError> {
    if !AVAILABLE {
        return Err(LaunchError::Unavailable);
    }
    if !program.is_absolute() {
        return Err(LaunchError::NotAbsolute(program.to_owned()));
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(env.iter().map(|(name, value)| (*name, value.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // son propre groupe : ni un Ctrl+C du terminal, ni la fin de Capri ne l'arrêtent
            .process_group(0);
        let mut child =
            command.spawn().map_err(|source| LaunchError::Io { path: program.to_owned(), source })?;
        let pid = child.id();
        let name = program.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let waiting =
            std::thread::Builder::new().name("capri-lance".to_owned()).spawn(move || match child.wait() {
                Ok(status) => log::info!("{name} (processus {pid}) s'est arrêté : {status}"),
                Err(err) => log::warn!("{name} (processus {pid}) : fin inconnue ({err})"),
            });
        if let Err(err) = waiting {
            log::warn!("processus {pid} lancé, mais sa fin ne sera pas attendue : {err}");
        }
        Ok(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (args, env);
        Err(LaunchError::Unavailable)
    }
}

/// Ouvre l'app `app` comme un double-clic, sans la mettre au premier plan. Attend seulement
/// qu'`open` ait rendu la main (l'app, elle, continue).
pub fn open_app(app: &Path) -> Result<(), LaunchError> {
    if !AVAILABLE {
        return Err(LaunchError::Unavailable);
    }
    if !app.is_absolute() {
        return Err(LaunchError::NotAbsolute(app.to_owned()));
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        let status = Command::new("/usr/bin/open")
            .args(["-g", "-a"])
            .arg(app)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|source| LaunchError::Io { path: app.to_owned(), source })?;
        if status.success() { Ok(()) } else { Err(LaunchError::Open { path: app.to_owned(), status }) }
    }
    #[cfg(not(target_os = "macos"))]
    Err(LaunchError::Unavailable)
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use std::time::{Duration, Instant};

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("capri-test-lance-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_an_absolute_path_is_launched() {
        assert!(matches!(
            start_program(Path::new("ollama"), &["serve"], &[]),
            Err(LaunchError::NotAbsolute(_))
        ));
        assert!(matches!(open_app(Path::new("Ollama.app")), Err(LaunchError::NotAbsolute(_))));
    }

    #[test]
    fn a_program_and_an_app_are_told_apart() {
        let directory = scratch("reconnus");
        let program = directory.join("ollama");
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert!(!is_program(&program), "pas exécutable");
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(is_program(&program));
        assert!(!is_program(&directory.join("absent")) && !is_program(&directory));
        let app = directory.join("Ollama.app");
        std::fs::create_dir_all(app.join("Contents")).unwrap();
        assert!(!is_app(&app), "sans Info.plist");
        std::fs::write(app.join("Contents/Info.plist"), "<plist/>").unwrap();
        assert!(is_app(&app) && !is_app(&program) && !is_program(&app));
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// Un programme inoffensif (le shell du système), pour voir qu'il reçoit ses arguments et son
    /// environnement, et qu'il tourne sans qu'on l'attende. Jamais un vrai Ollama.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_program_runs_detached_with_its_arguments_and_environment() {
        let directory = scratch("detache");
        let out = directory.join("sortie");
        let script = format!("printf '%s %s' \"$1\" \"$CAPRI_ESSAI_HOTE\" > '{}'", out.display());
        let started = Instant::now();
        let pid = start_program(
            Path::new("/bin/sh"),
            &["-c", &script, "sh", "serve"],
            &[("CAPRI_ESSAI_HOTE", "127.0.0.1:11500".to_owned())],
        )
        .expect("lancé");
        assert!(pid > 0);
        assert!(started.elapsed() < Duration::from_secs(1), "pas attendu : {:?}", started.elapsed());
        let deadline = Instant::now() + Duration::from_secs(5);
        while std::fs::read_to_string(&out).map_or(true, |text| text.is_empty()) {
            assert!(Instant::now() < deadline, "le programme n'a rien écrit");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "serve 127.0.0.1:11500");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn elsewhere_nothing_is_launched() {
        assert!(matches!(start_program(Path::new("/bin/true"), &[], &[]), Err(LaunchError::Unavailable)));
        assert!(matches!(open_app(Path::new("/Applications/Ollama.app")), Err(LaunchError::Unavailable)));
    }
}
