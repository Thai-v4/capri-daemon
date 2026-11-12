//! Le transport local entre `capri-hook` et Capri : des lignes de texte, dans un seul sens.
//!
//! Avec `capri-platform`, c'est la seconde (et dernière) couche qui parle au système. Elle est à
//! part parce que `capri-hook` la charge à chaque événement de Claude Code : elle ne doit entraîner
//! ni winit ni AppKit.
//!
//! Le canal est réservé à l'utilisateur, sans port réseau :
//! - Unix : une socket dans un dossier `0700`, elle-même en `0600`, et chaque connexion est
//!   vérifiée (le pair doit avoir le même UID) ;
//! - Windows : un tube nommé, en lecture seule côté serveur, refusant les clients distants, dont
//!   le nom aléatoire est publié dans un fichier du profil de l'utilisateur.
//!
//! Une connexion porte une ligne : le client se connecte, écrit, raccroche. Des clients arrivés
//! alors que Capri n'attend pas dans [`Listener::next_line`] sont servis quand même, dans l'ordre :
//! sous Unix par la file d'attente de la socket, sous Windows par un thread qui sert le tube sans
//! arrêt (voir `windows.rs`).

use std::path::PathBuf;
use std::time::Duration;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

/// Variable d'environnement qui remplace l'emplacement par défaut du canal (tests, seconde
/// instance). Unix : chemin de la socket. Windows : chemin du fichier qui porte le nom du tube.
pub const ENDPOINT_ENV: &str = "CAPRI_SOCKET";

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("impossible de situer le dossier de l'utilisateur ({0} n'est pas définie)")]
    NoHome(&'static str),
    #[error("le chemin {0} est trop long pour une socket")]
    PathTooLong(PathBuf),
    #[error("le dossier {0} n'appartient pas à l'utilisateur courant")]
    ForeignDirectory(PathBuf),
    #[error("{0} existe déjà et n'est pas une socket : rien n'est supprimé")]
    NotASocket(PathBuf),
    #[error("une autre instance de Capri écoute déjà")]
    AlreadyRunning,
    #[error("Capri n'écoute pas")]
    NotRunning,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Où Capri écoute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint(PathBuf);

impl Endpoint {
    /// L'emplacement de l'utilisateur courant, ou celui que désigne [`ENDPOINT_ENV`].
    pub fn for_current_user() -> Result<Self, IpcError> {
        match std::env::var_os(ENDPOINT_ENV).filter(|path| !path.is_empty()) {
            Some(path) => Ok(Self(PathBuf::from(path))),
            None => imp::default_path().map(Self),
        }
    }

    /// Un emplacement choisi (tests).
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

/// Le côté de Capri : reçoit les lignes, une connexion après l'autre.
pub struct Listener(imp::Listener);

impl Listener {
    /// Ouvre le canal. Les lignes de plus de `max_line_bytes` octets seront ignorées.
    pub fn bind(endpoint: &Endpoint, max_line_bytes: usize) -> Result<Self, IpcError> {
        imp::Listener::bind(endpoint.path(), max_line_bytes).map(Self)
    }

    /// Attend la prochaine ligne valide (sans son saut de ligne). Les connexions refusées, vides,
    /// trop longues ou illisibles sont passées en silence.
    pub fn next_line(&mut self) -> Result<String, IpcError> {
        self.0.next_line()
    }
}

/// À appeler quand Capri s'arrête : retire du disque ce qui annonce le canal (la socket, ou sous
/// Windows le fichier qui porte le nom du tube), pour que le hook sache tout de suite qu'elle
/// n'écoute plus. À n'appeler que par l'instance qui a ouvert le canal.
pub fn retire(endpoint: &Endpoint) {
    imp::retire(endpoint.path());
}

/// Le côté du hook : envoie une ligne à Capri, en `timeout` au plus. Si Capri n'écoute pas, rend
/// [`IpcError::NotRunning`] sans attendre.
pub fn send_line(endpoint: &Endpoint, line: &str, timeout: Duration) -> Result<(), IpcError> {
    imp::send_line(endpoint.path(), line, timeout)
}

/// Lit une ligne d'au plus `max` octets : `None` si elle est vide, trop longue ou n'est pas de
/// l'UTF-8.
fn read_line(mut reader: impl std::io::Read, max: usize) -> Option<String> {
    let mut line = Vec::with_capacity(512);
    let mut chunk = [0u8; 1024];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            // délai dépassé, tube rompu : on garde ce qui est arrivé si la ligne était complète
            Err(_) => break,
        };
        match chunk[..read].iter().position(|&byte| byte == b'\n') {
            Some(end) => {
                line.extend_from_slice(&chunk[..end]);
                break;
            }
            None => line.extend_from_slice(&chunk[..read]),
        }
        if line.len() > max {
            return None;
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    if line.is_empty() || line.len() > max {
        return None;
    }
    String::from_utf8(line).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_one_line_and_stops_at_the_newline() {
        assert_eq!(read_line("bonjour\nle reste".as_bytes(), 100).as_deref(), Some("bonjour"));
        assert_eq!(read_line("sans fin de ligne".as_bytes(), 100).as_deref(), Some("sans fin de ligne"));
        assert_eq!(read_line("windows\r\n".as_bytes(), 100).as_deref(), Some("windows"));
        assert_eq!(read_line("é".repeat(600).as_bytes(), 2000).map(|l| l.chars().count()), Some(600));
    }

    #[test]
    fn refuses_empty_oversized_or_non_utf8_lines() {
        assert_eq!(read_line("".as_bytes(), 100), None);
        assert_eq!(read_line("\n".as_bytes(), 100), None);
        assert_eq!(read_line("x".repeat(101).as_bytes(), 100), None);
        assert_eq!(read_line("x".repeat(100).as_bytes(), 100).map(|l| l.len()), Some(100));
        assert_eq!(read_line("x".repeat(5000).as_bytes(), 100), None, "sans attendre la fin du flux");
        assert_eq!(read_line(&[0xff, 0xfe, b'\n'][..], 100), None);
    }
}

#[cfg(test)]
mod tests_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Un dossier neuf, au chemin court (une socket Unix ne supporte pas les chemins longs).
    pub fn scratch_dir(name: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique =
            format!("capri-ipc-{}-{}-{name}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
        let mut base = std::env::temp_dir();
        if cfg!(unix) && base.as_os_str().len() > 55 {
            base = PathBuf::from("/tmp");
        }
        let dir = base.join(unique);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dossier de test");
        dir
    }

    /// Le chemin du canal dans un dossier de test : la socket, ou sous Windows le fichier qui
    /// porte le nom du tube.
    pub fn endpoint_in(dir: &std::path::Path) -> crate::Endpoint {
        crate::Endpoint::at(dir.join(if cfg!(windows) { "hook-pipe.txt" } else { "hook.sock" }))
    }
}

#[cfg(test)]
mod channel_tests {
    use std::time::{Duration, Instant};

    use super::tests_support::{endpoint_in, scratch_dir};
    use super::*;

    const MAX: usize = 8 * 1024;
    const SOON: Duration = Duration::from_millis(150);

    #[test]
    fn a_line_sent_is_a_line_received() {
        let endpoint = endpoint_in(&scratch_dir("aller"));
        let mut listener = Listener::bind(&endpoint, MAX).unwrap();
        send_line(&endpoint, "{\"v\":1,\"kind\":\"turn_end\"}\n", SOON).unwrap();
        send_line(&endpoint, "sans saut de ligne", SOON).unwrap();
        assert_eq!(listener.next_line().unwrap(), "{\"v\":1,\"kind\":\"turn_end\"}");
        assert_eq!(listener.next_line().unwrap(), "sans saut de ligne");
    }

    #[test]
    fn hooks_that_come_while_capri_is_busy_elsewhere_are_still_heard() {
        // Comme les tests de `capri-hook` : un envoi, une lecture, un envoi, une lecture… Sous
        // Windows, servir le tube seulement pendant `next_line` perdait le second envoi, puis
        // attendait pour toujours.
        let endpoint = endpoint_in(&scratch_dir("entre"));
        let mut listener = Listener::bind(&endpoint, MAX).unwrap();
        for round in 0..5 {
            send_line(&endpoint, &format!("tour {round}"), SOON).unwrap();
            std::thread::sleep(Duration::from_millis(20)); // Capri fait autre chose
            assert_eq!(listener.next_line().unwrap(), format!("tour {round}"));
        }
        // Et plusieurs d'affilée avant de lire, bien plus longues qu'une vraie ligne du hook
        // (quelques centaines d'octets, 2,5 Kio au plus avec le miroir). Pas `MAX` : sous macOS,
        // une ligne de 8 Kio écrite avant que Capri n'accepte dépasse le tampon de la socket et
        // l'envoi échoue à son échéance (mesuré).
        let big: Vec<String> = (0..3).map(|i| format!("{i}{}", "x".repeat(MAX / 2 - 1))).collect();
        for line in &big {
            send_line(&endpoint, line, SOON).unwrap();
        }
        for line in &big {
            assert_eq!(&listener.next_line().unwrap(), line);
        }
    }

    #[test]
    fn closing_capri_is_quick_and_frees_the_place() {
        let endpoint = endpoint_in(&scratch_dir("fermeture"));
        for _ in 0..3 {
            let listener = Listener::bind(&endpoint, MAX).unwrap();
            let started = Instant::now();
            drop(listener);
            assert!(started.elapsed() < Duration::from_millis(400), "{:?}", started.elapsed());
            assert!(matches!(send_line(&endpoint, "x", SOON), Err(IpcError::NotRunning)));
        }
    }

    #[test]
    fn without_capri_the_hook_gives_up_at_once() {
        let endpoint = endpoint_in(&scratch_dir("absent"));
        let started = Instant::now();
        assert!(matches!(send_line(&endpoint, "x", SOON), Err(IpcError::NotRunning)));
        assert!(started.elapsed() < Duration::from_millis(100), "{:?}", started.elapsed());

        // après la fermeture de Capri non plus
        drop(Listener::bind(&endpoint, MAX).unwrap());
        assert!(matches!(send_line(&endpoint, "x", SOON), Err(IpcError::NotRunning)));
    }

    #[test]
    fn retiring_the_channel_tells_the_hook_at_once() {
        let endpoint = endpoint_in(&scratch_dir("retraite"));
        let listener = Listener::bind(&endpoint, MAX).unwrap();
        assert!(endpoint.path().exists());
        // Capri s'arrête alors que son thread d'écoute tient encore le canal ouvert
        retire(&endpoint);
        assert!(!endpoint.path().exists());
        assert!(matches!(send_line(&endpoint, "x", SOON), Err(IpcError::NotRunning)));
        drop(listener);
        retire(&endpoint); // une seconde fois, ou sans rien à retirer : sans effet
    }

    #[test]
    fn a_second_capri_does_not_steal_the_channel() {
        let endpoint = endpoint_in(&scratch_dir("double"));
        let mut first = Listener::bind(&endpoint, MAX).unwrap();
        assert!(matches!(Listener::bind(&endpoint, MAX), Err(IpcError::AlreadyRunning)));
        send_line(&endpoint, "toujours pour la première", SOON).unwrap();
        assert_eq!(first.next_line().unwrap(), "toujours pour la première");
    }

    #[test]
    fn oversized_and_unreadable_lines_are_skipped() {
        let endpoint = endpoint_in(&scratch_dir("trop"));
        let mut listener = Listener::bind(&endpoint, 64).unwrap();
        let sender = {
            let endpoint = endpoint.clone();
            std::thread::spawn(move || {
                // l'envoi d'une ligne refusée peut échouer quand Capri raccroche : peu importe
                let _ = send_line(&endpoint, &"x".repeat(100_000), SOON);
                let _ = send_line(&endpoint, "\n", SOON);
                send_line(&endpoint, "bonne taille", SOON).unwrap();
            })
        };
        assert_eq!(listener.next_line().unwrap(), "bonne taille");
        sender.join().unwrap();
    }

    #[test]
    fn many_hooks_at_once_all_get_through() {
        let endpoint = endpoint_in(&scratch_dir("foule"));
        let mut listener = Listener::bind(&endpoint, MAX).unwrap();
        let senders: Vec<_> = (0..24)
            .map(|i| {
                let endpoint = endpoint.clone();
                std::thread::spawn(move || {
                    send_line(&endpoint, &format!("ligne {i}"), Duration::from_secs(2)).unwrap()
                })
            })
            .collect();
        let mut received: Vec<String> = (0..24).map(|_| listener.next_line().unwrap()).collect();
        for sender in senders {
            sender.join().unwrap();
        }
        received.sort();
        let mut expected: Vec<String> = (0..24).map(|i| format!("ligne {i}")).collect();
        expected.sort();
        assert_eq!(received, expected);
    }

    #[test]
    fn the_default_endpoint_lives_in_the_users_own_folders() {
        let path = imp::default_path().unwrap();
        assert!(path.is_absolute(), "{}", path.display());
        let text = path.to_string_lossy().to_lowercase();
        assert!(text.contains("capri"), "{text}");
    }
}
