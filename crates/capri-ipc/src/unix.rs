//! Unix : une socket dans un dossier réservé à l'utilisateur.
//!
//! Trois protections se recouvrent : le dossier (créé en `0700`, refusé s'il appartient à un autre
//! ou si d'autres peuvent y écrire), la socket (`0600`), et l'identité du pair, vérifiée à chaque
//! connexion.

use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::IpcError;

/// Longueur maximale d'un chemin de socket : `sun_path` fait 104 octets sur macOS (108 sur
/// Linux), zéro final compris.
const MAX_SOCKET_PATH: usize = 103;
/// Un client qui se connecte sans rien écrire ne bloque pas les suivants plus longtemps que ça.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

pub fn default_path() -> Result<PathBuf, IpcError> {
    let home = || {
        std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from).ok_or(IpcError::NoHome("HOME"))
    };
    if cfg!(target_os = "macos") {
        return Ok(home()?.join("Library/Application Support/Capri/hook.sock"));
    }
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(runtime) => Ok(PathBuf::from(runtime).join("capri/hook.sock")),
        None => Ok(home()?.join(".capri/hook.sock")),
    }
}

pub struct Listener {
    listener: UnixListener,
    path: PathBuf,
    max_line_bytes: usize,
    uid: u32,
}

impl Listener {
    pub fn bind(path: &Path, max_line_bytes: usize) -> Result<Self, IpcError> {
        if path.as_os_str().len() > MAX_SOCKET_PATH {
            return Err(IpcError::PathTooLong(path.to_owned()));
        }
        let uid = current_uid();
        let directory = path.parent().filter(|dir| !dir.as_os_str().is_empty()).unwrap_or(Path::new("."));
        secure_directory(directory, uid)?;

        match std::fs::symlink_metadata(path) {
            Ok(existing) if !existing.file_type().is_socket() => {
                return Err(IpcError::NotASocket(path.to_owned()));
            }
            Ok(_) => {
                if UnixStream::connect(path).is_ok() {
                    return Err(IpcError::AlreadyRunning);
                }
                // personne n'écoute : la socket d'une instance qui n'a pas pu la retirer
                std::fs::remove_file(path)?;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }

        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self { listener, path: path.to_owned(), max_line_bytes, uid })
    }

    pub fn next_line(&mut self) -> Result<String, IpcError> {
        loop {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(err.into()),
            };
            if peer_uid(&stream).ok() != Some(self.uid) {
                continue;
            }
            // Sur macOS, ce réglage échoue (EINVAL) quand le client a déjà raccroché, ce qui est
            // le cas normal : le hook écrit sa ligne et s'en va. La lecture ne peut alors plus
            // attendre, donc l'échec est sans conséquence.
            let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
            if let Some(line) = crate::read_line(&stream, self.max_line_bytes) {
                return Ok(line);
            }
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn send_line(path: &Path, line: &str, timeout: Duration) -> Result<(), IpcError> {
    // Jamais vers autre chose que notre propre socket : ni lien symbolique, ni socket d'un autre.
    let ours = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type().is_socket() && meta.uid() == current_uid(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err.into()),
    };
    if !ours {
        return Err(IpcError::NotRunning);
    }
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(err)
            if matches!(err.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound) =>
        {
            return Err(IpcError::NotRunning);
        }
        Err(err) => return Err(err.into()),
    };
    stream.set_write_timeout(Some(timeout.max(Duration::from_millis(1))))?;
    stream.write_all(line.as_bytes())?;
    if !line.ends_with('\n') {
        stream.write_all(b"\n")?;
    }
    Ok(())
}

/// Crée le dossier s'il manque (en `0700`), et refuse un dossier qui appartient à un autre ou où
/// d'autres peuvent écrire : ils pourraient y remplacer notre socket par la leur.
fn secure_directory(directory: &Path, uid: u32) -> Result<(), IpcError> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(directory)?;
    let meta = std::fs::metadata(directory)?;
    if meta.uid() != uid || meta.mode() & 0o022 != 0 {
        return Err(IpcError::ForeignDirectory(directory.to_owned()));
    }
    Ok(())
}

fn current_uid() -> u32 {
    // SAFETY: `getuid` ne prend aucun argument, ne modifie rien et ne peut pas échouer.
    unsafe { libc::getuid() }
}

/// L'UID du processus à l'autre bout de la socket.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: le descripteur est celui d'une socket ouverte, que `stream` garde en vie pendant
    // l'appel, et les deux pointeurs désignent des entiers locaux du bon type.
    let status = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if status == 0 { Ok(uid) } else { Err(std::io::Error::last_os_error()) }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    let mut credentials = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: le descripteur est celui d'une socket ouverte, que `stream` garde en vie pendant
    // l'appel ; `credentials` est une `ucred` locale dont `length` donne la taille exacte, ce que
    // SO_PEERCRED attend.
    let status = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut credentials).cast(),
            &mut length,
        )
    };
    if status == 0 { Ok(credentials.uid) } else { Err(std::io::Error::last_os_error()) }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "linux",
    target_os = "android"
)))]
fn peer_uid(_stream: &UnixStream) -> std::io::Result<u32> {
    // Sans moyen de vérifier le pair, on refuse toutes les connexions plutôt que de les accepter.
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::scratch_dir;

    const MAX: usize = 8 * 1024;
    const SOON: Duration = Duration::from_millis(150);

    #[test]
    fn the_directory_and_the_socket_are_for_the_user_only() {
        let dir = scratch_dir("perms");
        let path = dir.join("nouveau/hook.sock");
        let _listener = Listener::bind(&path, MAX).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().mode() & 0o777;
        assert_eq!(mode(&dir.join("nouveau")), 0o700);
        assert_eq!(mode(&path), 0o600);
        assert!(std::fs::metadata(&path).unwrap().file_type().is_socket());
    }

    #[test]
    fn a_directory_others_can_write_to_is_refused() {
        let dir = scratch_dir("partage");
        let shared = dir.join("partage");
        std::fs::create_dir(&shared).unwrap();
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();
        let path = shared.join("hook.sock");
        assert!(matches!(Listener::bind(&path, MAX), Err(IpcError::ForeignDirectory(_))));
        assert!(!path.exists());
    }

    #[test]
    fn a_stale_socket_is_replaced_but_a_live_one_is_respected() {
        let dir = scratch_dir("vivante");
        let path = dir.join("hook.sock");
        // une instance qui a planté laisse sa socket derrière elle
        drop(UnixListener::bind(&path).unwrap());
        assert!(path.exists());
        let first = Listener::bind(&path, MAX).expect("la socket abandonnée est remplacée");
        assert!(matches!(Listener::bind(&path, MAX), Err(IpcError::AlreadyRunning)));
        drop(first);
        assert!(!path.exists(), "la socket est retirée à la fermeture");
    }

    #[test]
    fn something_that_is_not_a_socket_is_never_deleted() {
        let dir = scratch_dir("fichier");
        let path = dir.join("hook.sock");
        std::fs::write(&path, "précieux").unwrap();
        assert!(matches!(Listener::bind(&path, MAX), Err(IpcError::NotASocket(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "précieux");
        // et le hook n'écrit pas dedans non plus
        assert!(matches!(send_line(&path, "x", SOON), Err(IpcError::NotRunning)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "précieux");
    }

    #[test]
    fn a_path_too_long_for_a_socket_is_refused_up_front() {
        let path = scratch_dir("long").join("x".repeat(120)).join("hook.sock");
        assert!(matches!(Listener::bind(&path, MAX), Err(IpcError::PathTooLong(_))));
        assert!(!path.parent().unwrap().exists(), "aucun dossier créé pour rien");
    }

    #[test]
    fn a_silent_client_does_not_block_the_next_one() {
        let dir = scratch_dir("muet");
        let path = dir.join("hook.sock");
        let mut listener = Listener::bind(&path, MAX).unwrap();
        let mute = UnixStream::connect(&path).unwrap(); // se connecte et n'écrit jamais
        send_line(&path, "après le muet", SOON).unwrap();
        let started = std::time::Instant::now();
        assert_eq!(listener.next_line().unwrap(), "après le muet");
        assert!(started.elapsed() < READ_TIMEOUT * 3, "{:?}", started.elapsed());
        drop(mute);
    }

    #[test]
    fn a_client_that_already_hung_up_is_still_heard() {
        // le cas normal : le hook écrit sa ligne et s'en va avant que Capri n'accepte
        let dir = scratch_dir("parti");
        let path = dir.join("hook.sock");
        let mut listener = Listener::bind(&path, MAX).unwrap();
        send_line(&path, "déjà parti", SOON).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(listener.next_line().unwrap(), "déjà parti");
    }

    #[test]
    fn the_peer_of_our_own_connection_is_ourselves() {
        let (a, _b) = UnixStream::pair().unwrap();
        assert_eq!(peer_uid(&a).unwrap(), current_uid());
    }
}
