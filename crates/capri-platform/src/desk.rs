//! Les fenêtres des autres apps, telles que Capri a le droit de les voir, sans demander aucune
//! permission : pour chacune, un numéro, un rectangle, si c'est une fenêtre d'app ordinaire, son
//! opacité, et si elle est à Capri. Rien d'autre n'est lu : ni le titre d'une fenêtre, ni le nom
//! de son app, ni ce qu'elle affiche. Ce module ne garde rien et n'écrit rien dans le journal.
//!
//! - macOS : la liste de CoreGraphics (`CGWindowListCopyWindowInfo`), qui ne demande ni
//!   l'Accessibilité, ni l'Enregistrement de l'écran (voir `macos/desk.rs`) ;
//! - Windows : les fenêtres de premier niveau (`EnumWindows`), avec leur cadre réel et leur état
//!   « masquée » selon DWM (voir `windows/desk.rs`) ;
//! - ailleurs : pas de liste, et [`windows`] le dit ([`DeskError::Unavailable`]) : Capri flotte
//!   comme avant.
//!
//! Le repère des rectangles est celui du système ; `Overlay::desk_frame` dit où est l'overlay
//! dedans, et `capri_core::perch` fait la conversion.

use capri_core::perch::DeskWindow;

#[cfg(target_os = "macos")]
#[path = "macos/desk.rs"]
mod imp;

#[cfg(windows)]
#[path = "windows/desk.rs"]
mod imp;

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    //! Pas de liste des fenêtres sur ce système.

    use capri_core::perch::DeskWindow;

    use super::DeskError;

    pub fn windows() -> Result<Vec<DeskWindow>, DeskError> {
        Err(DeskError::Unavailable)
    }

    pub fn mouse_down() -> bool {
        false
    }
}

/// Ce système sait-il donner la liste des fenêtres ?
pub const AVAILABLE: bool = cfg!(any(target_os = "macos", windows));

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeskError {
    #[error("pas de liste des fenêtres sur ce système")]
    Unavailable,
    #[error("le système n'a pas donné la liste des fenêtres")]
    Refused,
}

/// Les fenêtres à l'écran, de la plus en avant à la plus en arrière. Seulement celles qui sont à
/// l'écran : ni réduites, ni d'un autre bureau, ni d'une app masquée.
pub fn windows() -> Result<Vec<DeskWindow>, DeskError> {
    imp::windows()
}

/// Un bouton de la souris est-il enfoncé, où que soit le curseur et quelle que soit l'app ? Un
/// indice qu'une fenêtre est peut-être en train d'être déplacée. Sans permission. Toujours `false`
/// là où le système ne sait pas le dire.
pub fn mouse_down() -> bool {
    imp::mouse_down()
}

/// En version de débogage, pour `capri perch rehearse` : Capri a-t-elle la permission
/// d'enregistrer l'écran ? Ne la demande jamais (`CGPreflightScreenCaptureAccess`, qui, dit
/// `CGWindow.h`, répond « sans rien demander à l'utilisateur ») : c'est pour montrer que la liste
/// des fenêtres n'en a pas besoin. `None` ailleurs que sur macOS.
#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn screen_recording_allowed() -> Option<bool> {
    Some(objc2_core_graphics::CGPreflightScreenCaptureAccess())
}

#[cfg(all(debug_assertions, not(target_os = "macos")))]
pub fn screen_recording_allowed() -> Option<bool> {
    None
}

/// En version de débogage, pour `capri perch rehearse` : où serait l'overlay, sans le créer (il
/// couvre l'écran principal), et sa taille en pixels. À appeler sur le thread principal ; `None`
/// ailleurs que sur macOS.
#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn main_screen() -> Option<(capri_core::perch::DeskFrame, (u32, u32))> {
    crate::macos::main_screen_desk()
}

#[cfg(all(debug_assertions, not(target_os = "macos")))]
pub fn main_screen() -> Option<(capri_core::perch::DeskFrame, (u32, u32))> {
    None
}

/// En version de débogage, pour `capri perch window` : une petite fenêtre d'essai, ordinaire et
/// opaque, qu'on déplace par programme (macOS seulement).
#[cfg(all(debug_assertions, target_os = "macos"))]
#[path = "macos/test_window.rs"]
mod test_window;
#[cfg(all(debug_assertions, target_os = "macos"))]
pub use test_window::TestWindow;

/// En version de débogage, pour mesurer ce que coûte Capri : le temps de processeur consommé par
/// tout le processus depuis son lancement. `None` ailleurs que sur macOS.
#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn cpu_time() -> Option<std::time::Duration> {
    /// `CLOCK_PROCESS_CPUTIME_ID`, de `<_time.h>`.
    const CLOCK_PROCESS_CPUTIME_ID: std::ffi::c_int = 12;
    clock_nanos(CLOCK_PROCESS_CPUTIME_ID)
}

/// Comme [`cpu_time`], pour le seul fil qui appelle.
#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn thread_cpu_time() -> Option<std::time::Duration> {
    /// `CLOCK_THREAD_CPUTIME_ID`, de `<_time.h>`.
    const CLOCK_THREAD_CPUTIME_ID: std::ffi::c_int = 16;
    clock_nanos(CLOCK_THREAD_CPUTIME_ID)
}

#[cfg(all(debug_assertions, target_os = "macos"))]
fn clock_nanos(clock: std::ffi::c_int) -> Option<std::time::Duration> {
    // `<_time.h>` du SDK de macOS : `__uint64_t clock_gettime_nsec_np(clockid_t __clock_id);`,
    // où `clockid_t` est une énumération C (un `int`). Elle rend 0 en cas d'erreur.
    unsafe extern "C" {
        fn clock_gettime_nsec_np(clock_id: std::ffi::c_int) -> u64;
    }
    // SAFETY: fonction de la bibliothèque C du système, déclarée comme dans son en-tête, qui ne
    // prend qu'un entier et ne touche à aucune mémoire de l'appelant.
    let nanos = unsafe { clock_gettime_nsec_np(clock) };
    (nanos > 0).then(|| std::time::Duration::from_nanos(nanos))
}

#[cfg(all(debug_assertions, not(target_os = "macos")))]
pub fn cpu_time() -> Option<std::time::Duration> {
    None
}

#[cfg(all(debug_assertions, not(target_os = "macos")))]
pub fn thread_cpu_time() -> Option<std::time::Duration> {
    None
}
