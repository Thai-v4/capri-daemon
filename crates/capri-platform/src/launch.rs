//! Lancer un autre programme, sans l'attendre et sans jamais l'arrêter.
//!
//! Capri ne s'en sert que pour **Ollama**, et seulement quand c'est son cerveau sur cette machine
//! et que la case « Se lancer à l'ouverture de session » est cochée : c'est `capri_chat::startup`
//! qui décide, et qui donne les emplacements, fixes, où le chercher. Ici, on lance ce qu'on nous
//! donne, par son chemin absolu (jamais par le `PATH`), et rien d'autre. Et pour **montrer son
//! journal** dans le Finder, sur un geste de l'utilisateur (« Montrer le journal » de son menu).
//!
//! - un programme ([`start_program`]) : détaché de Capri (son propre groupe de processus, entrée
//!   et sorties fermées), il continue de tourner quand on la quitte, et un Ctrl+C dans le terminal
//!   qui l'a lancée ne l'atteint pas. Un thread à part attend sa fin, pour que le système n'en
//!   garde pas la trace (`waitpid`) ;
//! - une app ([`open_app`]) : par `/usr/bin/open -g -a`, comme un double-clic, sans la mettre au
//!   premier plan (`open(1)`) ; si elle tourne déjà, `open` ne fait que la retrouver ;
//! - un fichier montré dans le Finder ([`reveal`]) : par `/usr/bin/open -R` (« Reveals the file(s)
//!   in the Finder instead of opening them », `open(1)`), sans attendre `open` : un thread à part
//!   attend sa fin ;
//! - le **relais d'une mise à jour** ([`hand_over`]) : le `capri` de la nouvelle app, sur le geste
//!   de l'utilisateur qui redémarre Capri, détaché comme un programme, son entrée branchée sur un
//!   tube dont Capri garde l'autre bout jusqu'à sa sortie (le protocole est décrit dans
//!   `capri_core::bundle`). Il saura qu'elle est sortie quand le tube se fermera : aucun processus
//!   n'est jamais signalé, ni par elle ni par lui.
//!
//! Ailleurs que sur macOS : [`LaunchError::Unavailable`].

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

/// Ce système sait-il lancer Ollama, et montrer un fichier ?
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

/// Montre le fichier `file` dans le Finder (`/usr/bin/open -R`), sans l'ouvrir. N'attend pas
/// qu'`open` ait rendu la main : on l'appelle depuis la boucle d'affichage. Un échec d'`open`
/// lui-même n'est connu qu'après : il est écrit dans le journal.
pub fn reveal(file: &Path) -> Result<(), LaunchError> {
    if !AVAILABLE {
        return Err(LaunchError::Unavailable);
    }
    if !file.is_absolute() {
        return Err(LaunchError::NotAbsolute(file.to_owned()));
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        let mut child = Command::new("/usr/bin/open")
            .arg("-R")
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| LaunchError::Io { path: file.to_owned(), source })?;
        let waiting =
            std::thread::Builder::new().name("capri-montre".to_owned()).spawn(move || match child.wait() {
                Ok(status) if status.success() => {}
                Ok(status) => log::warn!("open n'a pas pu montrer le fichier ({status})"),
                Err(err) => log::warn!("open : fin inconnue ({err})"),
            });
        if let Err(err) = waiting {
            log::warn!("open lancé, mais sa fin ne sera pas attendue : {err}");
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Err(LaunchError::Unavailable)
}

/// Le relais d'une mise à jour, du côté de l'ancienne Capri : le programme de fin est lancé, et
/// elle garde le bout du tube qui le tient en attente.
pub struct Relay {
    /// Le bout du tube : écrire `capri_core::bundle::HANDOVER_GO` dit « vas-y » ; le fermer sans
    /// l'avoir écrit lui dit de renoncer.
    writer: Option<std::io::PipeWriter>,
    /// A-t-il dit qu'il prenait le relais ? (sa première ligne)
    answer: Receiver<bool>,
    heard: std::cell::Cell<Option<bool>>,
}

/// Le bout du tube après « vas-y » : il doit rester ouvert jusqu'à la sortie de Capri, que le
/// programme de fin attend ([`Gone::keep_until_exit`]). Lâché avant, le programme de fin croirait
/// qu'elle est sortie : seuls les essais le lâchent.
pub struct Gone {
    _writer: std::io::PipeWriter,
}

impl Gone {
    /// Le tube reste ouvert jusqu'à la sortie du processus, qui le fermera.
    pub fn keep_until_exit(self) {
        std::mem::forget(self);
    }
}

impl Relay {
    /// A-t-il pris le relais ? `None` : il ne l'a pas encore dit. `Some(false)` : il a dit autre
    /// chose, ou il est sorti sans rien dire.
    pub fn ready(&self) -> Option<bool> {
        if self.heard.get().is_none() {
            match self.answer.try_recv() {
                Ok(ready) => self.heard.set(Some(ready)),
                Err(TryRecvError::Disconnected) => self.heard.set(Some(false)),
                Err(TryRecvError::Empty) => {}
            }
        }
        self.heard.get()
    }

    /// Lui dit d'y aller : il commencera quand ce processus sera sorti. Le bout du tube est rendu,
    /// à garder ouvert jusque-là.
    pub fn go(mut self) -> Result<Gone, LaunchError> {
        use std::io::Write;
        let mut writer = self.writer.take().ok_or(LaunchError::Unavailable)?;
        writer
            .write_all(capri_core::bundle::HANDOVER_GO.as_bytes())
            .and_then(|()| writer.flush())
            .map_err(|source| LaunchError::Io { path: PathBuf::from("relais"), source })?;
        Ok(Gone { _writer: writer })
    }
}

/// Lance `program` (le `capri` de la nouvelle app) avec `args`, détaché (son propre groupe de
/// processus, sa sortie d'erreur jetée), son entrée sur un tube dont on garde l'autre bout, sa
/// sortie lue par un thread « capri-relais » qui attend sa première ligne. Rien d'autre n'est lu
/// de lui.
pub fn hand_over(program: &Path, args: &[&OsStr]) -> Result<Relay, LaunchError> {
    if !AVAILABLE {
        return Err(LaunchError::Unavailable);
    }
    if !program.is_absolute() {
        return Err(LaunchError::NotAbsolute(program.to_owned()));
    }
    #[cfg(target_os = "macos")]
    {
        use std::io::Read;
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        let io = |source| LaunchError::Io { path: program.to_owned(), source };
        // les deux bouts sont fermés à l'exécution d'un autre programme (`std::io::pipe`) : seul
        // celui que reçoit son entrée passe au programme de fin ; le nôtre ne passe à personne
        let (reader, writer) = std::io::pipe().map_err(io)?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::from(reader))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            // son propre groupe : un Ctrl+C du terminal qui a lancé Capri ne l'atteint pas
            .process_group(0)
            .spawn()
            .map_err(io)?;
        let mut stdout = child.stdout.take().ok_or_else(|| io(std::io::Error::other("pas de sortie")))?;
        let (sender, answer) = std::sync::mpsc::channel();
        let reading = std::thread::Builder::new().name("capri-relais".to_owned()).spawn(move || {
            let expected = capri_core::bundle::HANDOVER_READY.as_bytes();
            let mut line = Vec::new();
            let mut byte = [0u8; 1];
            while line.len() < expected.len() && !line.ends_with(b"\n") {
                match stdout.read(&mut byte) {
                    Ok(1) => line.push(byte[0]),
                    Ok(_) => break,
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = sender.send(line == expected);
            // sa fin est attendue ici, pour que le système n'en garde pas la trace, si elle
            // arrive avant la nôtre
            drop(stdout);
            let _ = child.wait();
        });
        if let Err(err) = reading {
            // sans personne pour l'écouter, il renoncera quand le tube se fermera
            log::warn!("relais : pas de thread pour l'écouter : {err}");
        }
        Ok(Relay { writer: Some(writer), answer, heard: std::cell::Cell::new(None) })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err(LaunchError::Unavailable)
    }
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
        // refusé avant de lancer quoi que ce soit : aucun Finder ne s'ouvre pendant les essais
        assert!(matches!(reveal(Path::new("logs/capri.log")), Err(LaunchError::NotAbsolute(_))));
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
        assert!(matches!(reveal(Path::new("/tmp/capri.log")), Err(LaunchError::Unavailable)));
        assert!(matches!(hand_over(Path::new("/bin/true"), &[]), Err(LaunchError::Unavailable)));
    }

    /// Attend `condition`, cinq secondes au plus.
    #[cfg(target_os = "macos")]
    fn within_five_seconds(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Un faux programme de fin (le shell du système) : il dit qu'il prend le relais, puis
    /// recopie ce qu'il reçoit dans un fichier jusqu'à la fermeture du tube, et laisse une trace de
    /// sa fin. Le tube ne se ferme qu'une fois notre bout lâché : il n'est passé à personne d'autre.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_relay_waits_for_the_pipe_to_close_and_nothing_else() {
        use std::ffi::OsString;
        let directory = scratch("relais");
        let (received, ended) = (directory.join("recu"), directory.join("fini"));
        let script = format!(
            "printf '{}'; cat > '{}'; : > '{}'",
            capri_core::bundle::HANDOVER_READY.trim_end().to_owned() + "\\n",
            received.display(),
            ended.display()
        );
        let args: Vec<OsString> = ["-c", &script].map(OsString::from).to_vec();
        let args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        let relay = hand_over(Path::new("/bin/sh"), &args).expect("lancé");
        assert!(within_five_seconds(|| relay.ready().is_some()), "aucune réponse");
        assert_eq!(relay.ready(), Some(true));
        let gone = relay.go().expect("vas-y");
        assert!(within_five_seconds(|| std::fs::read_to_string(&received).is_ok_and(|text| text == "go\n")));
        std::thread::sleep(Duration::from_millis(300));
        assert!(!ended.exists(), "le tube est encore ouvert : il attend");
        drop(gone); // ce que fait la sortie de Capri
        assert!(within_five_seconds(|| ended.exists()), "le tube s'est fermé : il a fini");
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// Un programme qui ne dit pas la bonne ligne n'a pas pris le relais ; lâcher le relais sans
    /// « vas-y » ferme le tube sans rien y écrire.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_wrong_answer_is_no_handover_and_dropping_the_relay_says_nothing() {
        use std::ffi::OsString;
        let directory = scratch("relais-refuse");
        let received = directory.join("recu");
        let script = format!("echo autre chose; cat > '{}'", received.display());
        let args: Vec<OsString> = ["-c", &script].map(OsString::from).to_vec();
        let args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        let relay = hand_over(Path::new("/bin/sh"), &args).expect("lancé");
        assert!(within_five_seconds(|| relay.ready().is_some()));
        assert_eq!(relay.ready(), Some(false));
        drop(relay);
        assert!(within_five_seconds(|| std::fs::metadata(&received).is_ok_and(|meta| meta.len() == 0)));
        // muet et sorti : pas de relais non plus
        let relay = hand_over(Path::new("/usr/bin/true"), &[]).expect("lancé");
        assert!(within_five_seconds(|| relay.ready().is_some()));
        assert_eq!(relay.ready(), Some(false));
        assert!(matches!(hand_over(Path::new("sh"), &[]), Err(LaunchError::NotAbsolute(_))));
        let _ = std::fs::remove_dir_all(&directory);
    }
}
