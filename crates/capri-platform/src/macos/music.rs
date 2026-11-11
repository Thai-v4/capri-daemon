//! macOS : un son sort-il d'une autre app, d'après CoreAudio, sans aucune permission.
//!
//! Deux fonctions C de CoreAudio, déclarées à la main d'après `AudioHardware.h` (SDK de macOS) :
//! aucun crate de plus, rien qui compile du C. `AudioObjectGetPropertyDataSize` dit combien
//! d'octets occupe une propriété ; `AudioObjectGetPropertyData` la copie dans notre tampon, dont
//! `ioDataSize` donne la taille à l'entrée et la part utilisée à la sortie. Le qualificatif est
//! vide (taille 0, pointeur nul) : « not all properties require qualification, in which case
//! this value will be NULL ».
//!
//! Les propriétés lues, et rien d'autre (toutes `UInt32` ou tableaux d'`AudioObjectID`, sauf le
//! PID, un `pid_t`) :
//! - de l'objet système (`kAudioObjectSystemObject`, 1) : `kAudioHardwarePropertyDevices`
//!   (`'dev#'`, « all the devices currently available ») et
//!   `kAudioHardwarePropertyProcessObjectList` (`'prs#'`, « all client processes currently
//!   connected ») ;
//! - de chaque appareil : `kAudioDevicePropertyDeviceIsRunningSomewhere` (`'gone'`, « 1 means
//!   that the AudioDevice is running in at least one process ») ;
//! - de chaque client : `kAudioProcessPropertyIsRunningOutput` (`'piro'`) et
//!   `kAudioProcessPropertyIsRunningInput` (`'piri'`), « 1 [...] the process is running IO and
//!   there is at least one active output [input] stream » ; et `kAudioProcessPropertyPID`
//!   (`'ppid'`), comparé au nôtre puis oublié.
//!
//! Jamais `kAudioProcessPropertyBundleID` (`'pbid'`), qui nommerait l'app. La seule permission
//! dont parlent ces en-têtes concerne la capture du son (`AudioHardwareTapping.h`, les « taps ») :
//! rien n'est capturé ici, aucun échantillon n'est lu. Sur un système plus ancien que
//! macOS 14.2, qui n'a pas la liste des clients, la lecture échoue et elle ne sait pas.

use std::ffi::c_void;

use super::{Heard, MusicError};

/// `AudioObjectPropertyAddress` (`AudioHardwareBase.h`) : trois `UInt32`.
#[repr(C)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    /// `OSStatus AudioObjectGetPropertyDataSize(AudioObjectID, const AudioObjectPropertyAddress*,
    /// UInt32, const void* __nullable, UInt32*)`.
    fn AudioObjectGetPropertyDataSize(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;

    /// `OSStatus AudioObjectGetPropertyData(AudioObjectID, const AudioObjectPropertyAddress*,
    /// UInt32, const void* __nullable, UInt32*, void*)`.
    fn AudioObjectGetPropertyData(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

const fn code(name: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*name)
}

/// `kAudioObjectSystemObject`.
const SYSTEM: u32 = 1;
/// `kAudioObjectPropertyScopeGlobal`.
const GLOBAL: u32 = code(b"glob");
/// `kAudioObjectPropertyElementMain`.
const MAIN: u32 = 0;
const DEVICES: u32 = code(b"dev#");
const CLIENTS: u32 = code(b"prs#");
const RUNNING_SOMEWHERE: u32 = code(b"gone");
const RUNNING_OUTPUT: u32 = code(b"piro");
const RUNNING_INPUT: u32 = code(b"piri");
const PID: u32 = code(b"ppid");

/// Au-delà, une liste n'est pas lue (un garde-fou : il y en a quelques dizaines).
const MAX_OBJECTS: u32 = 4096;

fn address(selector: u32) -> Address {
    Address { selector, scope: GLOBAL, element: MAIN }
}

/// Une propriété de 4 octets (`UInt32` ou `pid_t`).
fn word(object: u32, selector: u32) -> Option<u32> {
    let address = address(selector);
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `address` est un `AudioObjectPropertyAddress` valide le temps de l'appel ; pas de
    // qualificatif (taille 0, pointeur nul, permis par l'en-tête) ; `value` est un tampon de
    // `size` = 4 octets, qui ne peut pas être dépassé puisque l'en-tête dit que `size` en donne
    // la taille à l'entrée.
    let status = unsafe {
        AudioObjectGetPropertyData(object, &address, 0, std::ptr::null(), &mut size, (&raw mut value).cast())
    };
    (status == 0 && size == size_of::<u32>() as u32).then_some(value)
}

/// Une propriété tableau d'`AudioObjectID`.
fn objects(object: u32, selector: u32) -> Option<Vec<u32>> {
    let address = address(selector);
    let mut size = 0u32;
    // SAFETY: mêmes arguments valides que ci-dessus ; `size` reçoit le nombre d'octets.
    let status = unsafe { AudioObjectGetPropertyDataSize(object, &address, 0, std::ptr::null(), &mut size) };
    if status != 0 || size > MAX_OBJECTS * 4 {
        return None;
    }
    let mut list = vec![0u32; size as usize / 4];
    let mut size = (list.len() * 4) as u32;
    if size == 0 {
        return Some(list);
    }
    // SAFETY: `list` a `size` octets, alignés pour des `u32` ; l'en-tête dit que l'appel n'en
    // écrit pas plus que `size` à l'entrée, et dit combien il en a écrit à la sortie.
    let status = unsafe {
        AudioObjectGetPropertyData(object, &address, 0, std::ptr::null(), &mut size, list.as_mut_ptr().cast())
    };
    if status != 0 {
        return None;
    }
    list.truncate(size as usize / 4);
    Some(list)
}

#[derive(Default)]
pub struct Watch {
    /// Le dernier client trouvé en train de jouer : un numéro opaque de CoreAudio.
    last: Option<u32>,
}

impl Watch {
    pub fn listen(&mut self, thoroughly: bool) -> Result<Heard, MusicError> {
        let devices = objects(SYSTEM, DEVICES).ok_or(MusicError::Refused)?;
        let devices_running =
            devices.iter().filter(|&&device| word(device, RUNNING_SOMEWHERE) == Some(1)).count();
        let clients = objects(SYSTEM, CLIENTS).ok_or(MusicError::Refused)?;
        let mut heard = Heard { playing: false, devices_running, clients: clients.len(), asked: 0 };
        // aucun appareil en marche : personne ne sort de son, inutile d'interroger les clients
        if devices_running == 0 && !thoroughly {
            self.last = None;
            return Ok(heard);
        }
        let me = std::process::id();
        let mut plays = |client: u32| {
            heard.asked += 1;
            word(client, RUNNING_OUTPUT) == Some(1)
                && word(client, RUNNING_INPUT) != Some(1)
                && word(client, PID).is_some_and(|pid| pid != me)
        };
        let found = match self.last.filter(|&client| clients.contains(&client)) {
            Some(client) if plays(client) => Some(client),
            last => {
                clients.iter().copied().filter(|&client| Some(client) != last).find(|&client| plays(client))
            }
        };
        self.last = found;
        heard.playing = found.is_some();
        Ok(heard)
    }
}
