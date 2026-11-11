//! Windows : les fenêtres de premier niveau, par Win32 et DWM. **Écrit et compilé depuis un Mac,
//! jamais exécuté** : à vérifier sur un vrai PC (voir le README).
//!
//! - `EnumWindows` passe les fenêtres de premier niveau du premier plan vers l'arrière (l'ordre
//!   d'empilement : c'est ce que fait Windows, sans que la documentation le promette en toutes
//!   lettres) ;
//! - ne comptent que les fenêtres visibles (`IsWindowVisible`), pas réduites (`IsIconic`), pas
//!   « masquées » par DWM (`DWMWA_CLOAKED` : autre bureau virtuel, app suspendue), et pas le
//!   bureau lui-même (`GetShellWindow`) ;
//! - leur rectangle est le cadre réel que dessine DWM (`DWMWA_EXTENDED_FRAME_BOUNDS`, sans les
//!   bordures invisibles de redimensionnement), en pixels physiques de l'écran virtuel, comme
//!   l'overlay (winit déclare l'app « per-monitor DPI aware ») ; à défaut, `GetWindowRect` ;
//! - une fenêtre est « ordinaire » si elle n'est ni un outil (`WS_EX_TOOLWINDOW`), ni toujours
//!   au-dessus (`WS_EX_TOPMOST`), ni traversable par les clics (`WS_EX_TRANSPARENT`), ni jamais
//!   active (`WS_EX_NOACTIVATE`) ;
//! - son opacité est celle de `SetLayeredWindowAttributes` pour une fenêtre « layered » ; une
//!   fenêtre layered dont on ne peut pas la lire (transparence au pixel) compte pour invisible ;
//! - « à nous » : du même processus (`GetWindowThreadProcessId`).
//!
//! Le titre d'une fenêtre (`GetWindowText`) et son app ne sont jamais lus.

use std::ffi::c_void;

use capri_core::geom::{Rect, Vec2};
use capri_core::perch::DeskWindow;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetLayeredWindowAttributes, GetShellWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, LWA_ALPHA, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};
use windows_sys::core::BOOL;

use super::DeskError;

/// Ce qui fait d'une fenêtre autre chose qu'une fenêtre d'app ordinaire.
const UNUSUAL: u32 = WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE;

pub fn windows() -> Result<Vec<DeskWindow>, DeskError> {
    let mut found: Vec<DeskWindow> = Vec::new();
    // SAFETY: `visit` est un rappel `extern "system"` de la signature attendue ; `lparam` porte un
    // pointeur vers `found`, qui vit jusqu'au retour d'`EnumWindows` (le rappel n'est appelé que
    // pendant cet appel, sur ce thread) et qu'aucun autre code ne touche pendant ce temps.
    let ok = unsafe { EnumWindows(Some(visit), &raw mut found as LPARAM) };
    if ok == 0 {
        return Err(DeskError::Refused);
    }
    Ok(found)
}

/// Les boutons physiques gauche et droit (Windows ne tient pas compte ici de leur éventuelle
/// inversion) : l'un des deux est le bouton principal.
pub fn mouse_down() -> bool {
    // SAFETY: lecture de l'état d'une touche virtuelle, sans pointeur ; le bit de poids fort dit
    // qu'elle est enfoncée.
    unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON)) < 0 || GetAsyncKeyState(i32::from(VK_RBUTTON)) < 0 }
}

/// Appelé par `EnumWindows` pour chaque fenêtre de premier niveau ; rend 1 pour continuer.
unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` est le pointeur vers le `Vec` de `windows`, vivant et à nous seuls pendant
    // tout l'appel d'`EnumWindows` (voir là-bas).
    let found = unsafe { &mut *(lparam as *mut Vec<DeskWindow>) };
    if let Some(window) = describe(hwnd) {
        found.push(window);
    }
    1
}

/// Une fenêtre à l'écran, ou `None` si elle n'y est pas (ou plus).
fn describe(hwnd: HWND) -> Option<DeskWindow> {
    // SAFETY: `hwnd` vient d'`EnumWindows` ; ces fonctions acceptent une fenêtre détruite
    // entre-temps (elles échouent ou rendent 0), et n'écrivent que dans nos variables locales,
    // dont elles reçoivent l'adresse et la taille exactes.
    unsafe {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 || hwnd == GetShellWindow() {
            return None;
        }
        let mut cloaked: u32 = 0;
        let read = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            (&raw mut cloaked).cast::<c_void>(),
            size_of::<u32>() as u32,
        );
        if read >= 0 && cloaked != 0 {
            return None;
        }
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let opacity = if style & WS_EX_LAYERED == 0 { 1.0 } else { layered_opacity(hwnd) };
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        Some(DeskWindow {
            id: hwnd as usize as u64,
            rect: frame(hwnd)?,
            ordinary: style & UNUSUAL == 0,
            opacity,
            ours: pid != 0 && pid == GetCurrentProcessId(),
        })
    }
}

/// Le cadre réel de la fenêtre, en pixels physiques de l'écran virtuel.
///
/// # Safety
/// `hwnd` est une fenêtre (éventuellement détruite depuis : l'appel échoue alors).
unsafe fn frame(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: `r` est un RECT à nous, dont on passe l'adresse et la taille exactes.
    let read = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            (&raw mut r).cast::<c_void>(),
            size_of::<RECT>() as u32,
        )
    };
    // SAFETY: idem, pour `GetWindowRect`.
    if read < 0 && unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
        return None;
    }
    Some(Rect::new(
        Vec2::new(r.left as f32, r.top as f32),
        Vec2::new((r.right - r.left) as f32, (r.bottom - r.top) as f32),
    ))
}

/// L'opacité d'une fenêtre « layered » : 0 si on ne peut pas la lire.
///
/// # Safety
/// `hwnd` est une fenêtre (éventuellement détruite depuis : l'appel échoue alors).
unsafe fn layered_opacity(hwnd: HWND) -> f32 {
    let (mut alpha, mut flags) = (0u8, 0u32);
    // SAFETY: `alpha` et `flags` sont à nous ; la couleur clé n'est pas demandée (pointeur nul,
    // permis par la documentation).
    let read = unsafe { GetLayeredWindowAttributes(hwnd, std::ptr::null_mut(), &mut alpha, &mut flags) };
    match (read != 0, flags & LWA_ALPHA != 0) {
        (false, _) => 0.0,
        (true, true) => f32::from(alpha) / 255.0,
        (true, false) => 1.0,
    }
}
