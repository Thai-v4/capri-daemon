//! Windows : un tube nommé, dont le nom tiré au hasard à chaque lancement est publié dans un
//! fichier du profil de l'utilisateur.
//!
//! - Le tube est en lecture seule côté serveur (`PIPE_ACCESS_INBOUND`) : pour s'y connecter, un
//!   client doit avoir le droit d'écriture, que la sécurité par défaut d'un tube ne donne qu'à son
//!   créateur, aux administrateurs et au système.
//! - `PIPE_REJECT_REMOTE_CLIENTS` : aucun accès par le réseau.
//! - Nom aléatoire et `FILE_FLAG_FIRST_PIPE_INSTANCE` : personne ne peut créer le tube avant nous
//!   pour se faire passer pour Capri. Le fichier qui publie le nom est dans `%LOCALAPPDATA%`, que
//!   seul l'utilisateur peut lire.
//! - Jamais un instant sans une instance à nous tant que le nom est publié : la suivante est créée
//!   avant que la courante ne soit fermée, et le fichier qui publie le nom est retiré avant la
//!   dernière. Les autres ne peuvent pas en créer une à notre place : la sécurité par défaut ne
//!   leur donne que la lecture, pas `FILE_CREATE_PIPE_INSTANCE`.
//!
//! **Pourquoi un thread.** Sous Unix, la file d'attente de la socket accepte les clients même
//! quand Capri n'est pas dans `next_line`. Un tube n'a rien de tel : une instance tout juste créée
//! accepte un client, mais une fois ce client raccroché (`DisconnectNamedPipe`), elle n'en accepte
//! plus avant le prochain `ConnectNamedPipe`, et les clients trouvent le tube occupé
//! (`ERROR_PIPE_BUSY`). Servir le tube seulement dans `next_line` perdait donc tout client arrivé
//! entre deux appels, puis attendait pour toujours (c'est ce qui bloquait la CI). Un thread à lui
//! (« capri-tube ») sert donc le tube sans arrêt, en entrées-sorties synchrones, comme le serveur
//! multithread de la documentation : il attend un client, crée aussitôt l'instance suivante (au
//! plus [`MAX_INSTANCES`]), lit la ligne, ferme l'instance servie, et range la ligne dans une file
//! bornée ([`BACKLOG`]) que `next_line` vide, dans l'ordre d'arrivée.
//!
//! Ce qui sert, dans la documentation de Microsoft :
//! - <https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-operations> : « As soon as
//!   the pipe server creates a pipe instance, a pipe client can connect to it » ; après
//!   `DisconnectNamedPipe`, le serveur ferme l'instance, ou bien « can use ConnectNamedPipe to
//!   enable a new client to connect to this instance » ;
//! - <https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-waitnamedpipew> :
//!   une instance est disponible quand « the pipe's server process has a pending
//!   ConnectNamedPipe operation on the pipe » ;
//! - <https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-client> : « If the pipe
//!   exists but all of its instances are busy, CreateFile returns INVALID_HANDLE_VALUE and the
//!   GetLastError function returns ERROR_PIPE_BUSY » ;
//! - <https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-connectnamedpipe> :
//!   un client arrivé avant l'appel donne `ERROR_PIPE_CONNECTED` (« there is a good connection »),
//!   et `ERROR_NO_DATA` s'il a déjà fermé son handle ;
//! - <https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew> :
//!   toutes les instances d'un tube ont le même accès, le même type, le même nombre maximal
//!   d'instances et le même délai, sinon `ERROR_ACCESS_DENIED` ; `FILE_FLAG_FIRST_PIPE_INSTANCE`
//!   ne vaut que pour la première ; la sécurité par défaut donne « full control to the
//!   LocalSystem account, administrators, and the creator owner » et la lecture seule aux autres ;
//!   une écriture qui tient dans le quota du tube « completes immediately », sinon elle attend que
//!   le serveur lise ; une instance disparaît quand son dernier handle est fermé ;
//! - <https://learn.microsoft.com/en-us/windows/win32/ipc/multithreaded-pipe-server> : le modèle
//!   d'une boucle qui ne fait qu'attendre les clients.
//!
//! La version précédente a tourné dans la CI de GitHub (et s'y bloquait) ; celle-ci compile mais
//! n'a encore jamais tourné sur un vrai Windows.

use std::fs::{File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};

use crate::IpcError;

/// Tous nos tubes commencent ainsi ; la suite est faite de 32 chiffres hexadécimaux.
const PIPE_PREFIX: &str = r"\\.\pipe\capri-hook-";
/// Celle qu'on lit, et la suivante, déjà à l'écoute pendant qu'on lit.
const MAX_INSTANCES: u32 = 2;
/// Les lignes reçues qui attendent `next_line`. File pleine, le thread attend qu'on la vide et
/// les clients trouvent le tube occupé jusqu'à leur échéance : la file d'attente d'une socket.
const BACKLOG: usize = 128;
/// Entre deux essais d'ouverture d'un tube occupé.
const RETRY: Duration = Duration::from_millis(5);
/// Ce que la fermeture du canal laisse au thread pour s'arrêter, avant de le laisser finir seul.
const STOP_GRACE: Duration = Duration::from_millis(500);

pub fn default_path() -> Result<PathBuf, IpcError> {
    let base =
        std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()).ok_or(IpcError::NoHome("LOCALAPPDATA"))?;
    Ok(PathBuf::from(base).join("Capri").join("hook-pipe.txt"))
}

pub struct Listener {
    /// Les lignes rangées par le thread ; `None` seulement pendant la fermeture.
    lines: Option<Receiver<Result<String, IpcError>>>,
    /// Demande au thread de s'arrêter.
    stop: Arc<AtomicBool>,
    server: Option<JoinHandle<()>>,
    /// Le nom du tube.
    name: String,
    /// Le fichier qui publie le nom du tube.
    pointer: PathBuf,
}

impl Listener {
    pub fn bind(pointer: &Path, max_line_bytes: usize) -> Result<Self, IpcError> {
        if let Some(name) = published_name(pointer) {
            match OpenOptions::new().write(true).open(&name) {
                // Ce client d'essai est fermé aussitôt : l'autre Capri lit une ligne vide et
                // passe au suivant.
                Ok(_) => return Err(IpcError::AlreadyRunning),
                Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                    return Err(IpcError::AlreadyRunning);
                }
                // le tube n'existe plus : l'instance précédente est partie sans retirer son fichier
                Err(_) => {}
            }
        }

        let name = format!("{PIPE_PREFIX}{:016x}{:016x}", random_u64(), random_u64());
        let first = Instance::create(&name, true, max_line_bytes)?;
        let (sender, lines) = sync_channel(BACKLOG);
        let stop = Arc::new(AtomicBool::new(false));
        // Le thread d'abord, le fichier ensuite : si le thread ne part pas, l'instance est fermée
        // avant que son nom n'ait été publié.
        let server = {
            let (name, stop) = (name.clone(), Arc::clone(&stop));
            std::thread::Builder::new()
                .name("capri-tube".to_owned())
                .spawn(move || serve(first, &name, max_line_bytes, sender, &stop))?
        };
        let listener =
            Self { lines: Some(lines), stop, server: Some(server), name, pointer: pointer.to_owned() };
        // En cas d'échec, `listener` arrête le thread en disparaissant.
        if let Some(directory) = pointer.parent() {
            std::fs::create_dir_all(directory)?;
        }
        std::fs::write(pointer, &listener.name)?;
        Ok(listener)
    }

    pub fn next_line(&mut self) -> Result<String, IpcError> {
        match self.lines.as_ref().map(Receiver::recv) {
            Some(Ok(line)) => line,
            // le thread est parti, après avoir rendu son erreur
            _ => {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "le tube de Capri n'est plus servi")
                    .into())
            }
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        // Le nom n'est plus publié avant que la dernière instance ne soit fermée.
        if published_name(&self.pointer).as_deref() == Some(self.name.as_str()) {
            let _ = std::fs::remove_file(&self.pointer);
        }
        self.stop.store(true, Ordering::SeqCst);
        // un thread qui attend de pouvoir ranger une ligne repart aussitôt
        drop(self.lines.take());
        let Some(server) = self.server.take() else { return };
        server.thread().unpark();
        // Un thread qui attend un client dans `ConnectNamedPipe` n'en sort que par un client :
        // nous en sommes un (le créateur du tube a le droit d'écriture), aussitôt raccroché.
        let deadline = Instant::now() + STOP_GRACE;
        while !server.is_finished() && Instant::now() < deadline {
            match OpenOptions::new().write(true).open(&self.name) {
                Ok(_wake) => {}
                // plus aucune instance : le thread a fini
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                // occupé : il lit un client, et verra la demande d'arrêt juste après
                Err(_) => {}
            }
            std::thread::sleep(RETRY);
        }
        if server.is_finished() {
            let _ = server.join();
        }
        // Sinon, il lit un client qui n'écrit ni ne raccroche (de cet utilisateur : personne
        // d'autre ne peut se connecter) ; il s'arrêtera seul quand ce client partira. Le nom
        // n'est déjà plus publié.
    }
}

/// Le travail du thread « capri-tube » : servir le tube jusqu'à la fermeture du canal.
fn serve(
    mut current: Instance,
    name: &str,
    max_line_bytes: usize,
    lines: SyncSender<Result<String, IpcError>>,
    stop: &AtomicBool,
) {
    let failure = loop {
        if let Err(err) = current.wait_for_client() {
            break err;
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }
        // La suivante avant de lire celle-ci : le client d'après trouve le tube à l'écoute. Si
        // elle ne peut pas être créée, on raccrochera celle-ci pour la réutiliser.
        let next = Instance::create(name, false, max_line_bytes).ok();
        let line = current.read_line(max_line_bytes);
        current = match next {
            // l'ancienne est fermée ici, une fois la suivante en place
            Some(next) => next,
            None => {
                current.hang_up();
                current
            }
        };
        if let Some(line) = line
            && lines.send(Ok(line)).is_err()
        {
            return; // le canal est fermé
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }
    };
    // Une erreur que rien n'explique : `next_line` la rend, puis rend qu'il n'y a plus rien.
    let _ = lines.send(Err(failure));
    drop(lines);
    // L'instance reste ouverte jusqu'à la fermeture du canal, qui retire d'abord le fichier qui
    // publie son nom : le nom n'est jamais publié sans une instance à nous.
    while !stop.load(Ordering::SeqCst) {
        std::thread::park();
    }
    drop(current);
}

/// Une instance du tube, côté serveur.
struct Instance(File);

impl Instance {
    /// Crée une instance du tube `name` ; `first` pour la toute première.
    fn create(name: &str, first: bool, max_line_bytes: usize) -> Result<Self, IpcError> {
        let wide: Vec<u16> = std::ffi::OsStr::new(name).encode_wide().chain(std::iter::once(0)).collect();
        let open_mode =
            if first { PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE } else { PIPE_ACCESS_INBOUND };
        // SAFETY: `wide` est une chaîne UTF-16 terminée par un zéro, vivante pendant tout l'appel ;
        // le pointeur nul des attributs de sécurité demande la sécurité par défaut. Les autres
        // arguments sont des valeurs ; l'accès, le type, le nombre d'instances et le délai sont
        // les mêmes pour toutes les instances, comme la documentation l'exige.
        let handle = unsafe {
            CreateNamedPipeW(
                wide.as_ptr(),
                open_mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                MAX_INSTANCES,
                0,
                u32::try_from(max_line_bytes).unwrap_or(u32::MAX),
                0,
                std::ptr::null(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: `handle` est un handle valide, tout juste créé, dont personne d'autre n'est
        // propriétaire ; le `File` le fermera.
        let pipe = unsafe { OwnedHandle::from_raw_handle(handle) };
        // Le tube vu comme un fichier, pour lire avec les outils de la bibliothèque standard.
        Ok(Self(File::from(pipe)))
    }

    /// Attend un client, ou constate qu'il est déjà venu.
    fn wait_for_client(&self) -> Result<(), IpcError> {
        // SAFETY: le handle est celui d'une instance que nous possédons, ouverte tant que `self`
        // vit ; sans structure OVERLAPPED (le tube n'est pas en mode overlapped), l'appel attend
        // simplement le prochain client.
        let accepted = unsafe { ConnectNamedPipe(self.0.as_raw_handle(), std::ptr::null_mut()) } != 0;
        if accepted {
            return Ok(());
        }
        // SAFETY: lit le code d'erreur du thread courant, sans autre effet.
        let error = unsafe { GetLastError() };
        // Un client arrivé avant notre appel est déjà connecté, voire déjà reparti en laissant
        // sa ligne : dans les deux cas il y a quelque chose à lire.
        if error == ERROR_PIPE_CONNECTED || error == ERROR_NO_DATA {
            return Ok(());
        }
        Err(std::io::Error::from_raw_os_error(error as i32).into())
    }

    /// La ligne du client connecté (voir `crate::read_line`).
    fn read_line(&self, max_line_bytes: usize) -> Option<String> {
        crate::read_line(&self.0, max_line_bytes)
    }

    /// Raccroche le client (ce qui restait à lire est jeté), pour en accepter un autre.
    fn hang_up(&self) {
        // SAFETY: le handle est celui d'une instance que nous possédons, ouverte tant que `self`
        // vit ; l'appel ne touche à aucune mémoire de notre côté. Son résultat est sans
        // importance : sans client, il n'y a rien à raccrocher.
        unsafe { DisconnectNamedPipe(self.0.as_raw_handle()) };
    }
}

impl Drop for Instance {
    /// Raccrocher, puis fermer (le `File`) : la séquence de « Named Pipe Operations ».
    fn drop(&mut self) {
        self.hang_up();
    }
}

pub fn retire(pointer: &Path) {
    // seulement un fichier qui a bien la forme des nôtres
    if published_name(pointer).is_some() {
        let _ = std::fs::remove_file(pointer);
    }
}

/// Le temps donné ne borne que l'attente d'une instance libre (`ERROR_PIPE_BUSY`). L'écriture
/// elle-même ne peut pas avoir d'échéance (le tube est synchrone) ; une ligne qui tient dans le
/// quota du tube, que le serveur dimensionne pour une ligne entière, est écrite sans attendre,
/// et le serveur lit dès qu'un client arrive. `capri-hook` borne de toute façon sa propre vie.
pub fn send_line(pointer: &Path, line: &str, timeout: Duration) -> Result<(), IpcError> {
    let name = published_name(pointer).ok_or(IpcError::NotRunning)?;
    let deadline = Instant::now() + timeout;
    let mut pipe = loop {
        match OpenOptions::new().write(true).open(&name) {
            Ok(pipe) => break pipe,
            // Capri sert un autre client : réessayer jusqu'à l'échéance
            Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) && Instant::now() < deadline => {
                std::thread::sleep(RETRY);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Err(IpcError::NotRunning),
            Err(err) => return Err(err.into()),
        }
    };
    // une seule écriture, saut de ligne compris
    let mut bytes = Vec::with_capacity(line.len() + 1);
    bytes.extend_from_slice(line.as_bytes());
    if !line.ends_with('\n') {
        bytes.push(b'\n');
    }
    pipe.write_all(&bytes)?;
    Ok(())
}

/// Le nom publié dans le fichier, s'il a bien la forme de nos tubes : on n'ouvre rien d'autre.
fn published_name(pointer: &Path) -> Option<String> {
    let text = std::fs::read_to_string(pointer).ok()?;
    let name = text.trim();
    let suffix = name.strip_prefix(PIPE_PREFIX)?;
    (suffix.len() == 32 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| name.to_owned())
}

/// 64 bits imprévisibles : chaque `RandomState` tire ses clés du générateur du système.
fn random_u64() -> u64 {
    RandomState::new().hash_one(std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::scratch_dir;

    const MAX: usize = 8 * 1024;
    const SOON: Duration = Duration::from_millis(150);

    #[test]
    fn a_stale_pointer_is_replaced_and_a_foreign_one_is_never_followed() {
        let pointer = scratch_dir("perime").join("hook-pipe.txt");
        // le nom d'une Capri partie sans retirer son fichier
        let stale = format!("{PIPE_PREFIX}{}", "0".repeat(32));
        std::fs::write(&pointer, &stale).unwrap();
        assert!(matches!(send_line(&pointer, "x", SOON), Err(IpcError::NotRunning)));
        let mut listener = Listener::bind(&pointer, MAX).expect("le fichier périmé est remplacé");
        let name = published_name(&pointer).expect("un nom publié");
        assert_ne!(name, stale);
        send_line(&pointer, "après", SOON).unwrap();
        assert_eq!(listener.next_line().unwrap(), "après");
        drop(listener);
        assert!(!pointer.exists(), "retiré à la fermeture");

        // pas un de nos noms : le hook ne l'ouvre pas, et Capri le remplace
        std::fs::write(&pointer, r"\\.\pipe\autre-chose").unwrap();
        assert!(matches!(send_line(&pointer, "x", SOON), Err(IpcError::NotRunning)));
        drop(Listener::bind(&pointer, MAX).expect("remplacé"));
    }

    #[test]
    fn closing_leaves_a_pointer_that_names_another_pipe_alone() {
        let pointer = scratch_dir("autre").join("hook-pipe.txt");
        let listener = Listener::bind(&pointer, MAX).unwrap();
        // une autre Capri a publié son propre tube entre-temps
        let other = format!("{PIPE_PREFIX}{}", "f".repeat(32));
        std::fs::write(&pointer, &other).unwrap();
        drop(listener);
        assert_eq!(std::fs::read_to_string(&pointer).unwrap(), other);
    }
}
