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
//!
//! Ce code compile mais n'a encore jamais tourné sur un vrai Windows.

use std::fs::{File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
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

pub fn default_path() -> Result<PathBuf, IpcError> {
    let base =
        std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()).ok_or(IpcError::NoHome("LOCALAPPDATA"))?;
    Ok(PathBuf::from(base).join("Capri").join("hook-pipe.txt"))
}

pub struct Listener {
    pipe: OwnedHandle,
    /// Le même tube, vu comme un fichier, pour lire avec les outils de la bibliothèque standard.
    reader: File,
    /// Le fichier qui publie le nom du tube.
    pointer: PathBuf,
    max_line_bytes: usize,
}

impl Listener {
    pub fn bind(pointer: &Path, max_line_bytes: usize) -> Result<Self, IpcError> {
        if let Some(name) = published_name(pointer) {
            match OpenOptions::new().write(true).open(&name) {
                Ok(_) => return Err(IpcError::AlreadyRunning),
                Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                    return Err(IpcError::AlreadyRunning);
                }
                // le tube n'existe plus : l'instance précédente est partie sans retirer son fichier
                Err(_) => {}
            }
        }

        let name = format!("{PIPE_PREFIX}{:016x}{:016x}", random_u64(), random_u64());
        let wide: Vec<u16> = std::ffi::OsStr::new(&name).encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` est une chaîne UTF-16 terminée par un zéro, vivante pendant tout l'appel ;
        // le pointeur nul des attributs de sécurité demande la sécurité par défaut.
        let handle = unsafe {
            CreateNamedPipeW(
                wide.as_ptr(),
                PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
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
        // propriétaire ; `OwnedHandle` le fermera.
        let pipe = unsafe { OwnedHandle::from_raw_handle(handle) };
        let reader = File::from(pipe.try_clone()?);

        if let Some(directory) = pointer.parent() {
            std::fs::create_dir_all(directory)?;
        }
        std::fs::write(pointer, &name)?;
        Ok(Self { pipe, reader, pointer: pointer.to_owned(), max_line_bytes })
    }

    pub fn next_line(&mut self) -> Result<String, IpcError> {
        loop {
            // SAFETY: `pipe` est un handle de tube valide que nous possédons ; sans structure
            // OVERLAPPED, l'appel attend simplement le prochain client.
            let accepted = unsafe { ConnectNamedPipe(self.pipe.as_raw_handle(), std::ptr::null_mut()) } != 0;
            if !accepted {
                // SAFETY: lit le code d'erreur du thread courant, sans autre effet.
                let error = unsafe { GetLastError() };
                // Un client arrivé avant notre appel est déjà connecté, voire déjà reparti en
                // laissant sa ligne : dans les deux cas il y a quelque chose à lire.
                if error != ERROR_PIPE_CONNECTED && error != ERROR_NO_DATA {
                    return Err(std::io::Error::from_raw_os_error(error as i32).into());
                }
            }
            let line = crate::read_line(&self.reader, self.max_line_bytes);
            // SAFETY: `pipe` est un handle de tube valide ; raccroche ce client pour pouvoir
            // accepter le suivant.
            unsafe { DisconnectNamedPipe(self.pipe.as_raw_handle()) };
            if let Some(line) = line {
                return Ok(line);
            }
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.pointer);
    }
}

pub fn retire(pointer: &Path) {
    // seulement un fichier qui a bien la forme des nôtres
    if published_name(pointer).is_some() {
        let _ = std::fs::remove_file(pointer);
    }
}

pub fn send_line(pointer: &Path, line: &str, timeout: Duration) -> Result<(), IpcError> {
    let name = published_name(pointer).ok_or(IpcError::NotRunning)?;
    let deadline = Instant::now() + timeout;
    let mut pipe = loop {
        match OpenOptions::new().write(true).open(&name) {
            Ok(pipe) => break pipe,
            // Capri sert un autre client : réessayer jusqu'à l'échéance
            Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Err(IpcError::NotRunning),
            Err(err) => return Err(err.into()),
        }
    };
    pipe.write_all(line.as_bytes())?;
    if !line.ends_with('\n') {
        pipe.write_all(b"\n")?;
    }
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
