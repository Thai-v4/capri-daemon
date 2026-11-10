//! Windows : Win32 via windows-sys.
//!
//! winit sait rendre une fenêtre traversable par les clics, mais il la passe en « layered »
//! sans jamais appeler `SetLayeredWindowAttributes`, ce qui peut la rendre invisible avec une
//! surface DirectComposition. On gère donc les styles nous-mêmes : la fenêtre est layered (et
//! opaque au sens layered) dès le départ, et seul `WS_EX_TRANSPARENT` bascule ensuite.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetCursorPos, GetWindowLongPtrW, HWND_TOPMOST, LWA_ALPHA, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetLayeredWindowAttributes, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};
use winit::event_loop::EventLoopBuilder;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowAttributes};

/// Styles qu'on veut toujours : layered (pour pouvoir traverser), outil (hors Alt+Tab),
/// non activable (ne vole pas le focus).
const OVERLAY_STYLES: u32 = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;

pub fn configure_event_loop<T: 'static>(_builder: &mut EventLoopBuilder<T>) {}

pub fn overlay_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
        .with_skip_taskbar(true)
        // indispensable pour la transparence via DirectComposition (voir capri-render)
        .with_no_redirection_bitmap(true)
        .with_undecorated_shadow(false)
}

pub fn prepare_overlay(window: &Window) {
    let Some(hwnd) = hwnd(window) else {
        log::warn!("fenêtre Win32 introuvable : réglages de l'overlay ignorés");
        return;
    };
    update_ex_style(hwnd, OVERLAY_STYLES, 0);
    // SAFETY: `hwnd` est la fenêtre vivante de winit ; alpha 255 = la couche est pleinement
    // visible, la transparence au pixel vient de DirectComposition.
    unsafe { SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) };
}

pub fn cursor_in_window(window: &Window) -> Option<(f64, f64)> {
    let hwnd = hwnd(window)?;
    let mut pt = POINT { x: 0, y: 0 };
    // SAFETY: `pt` est un POINT valide qu'on possède ; `hwnd` est une fenêtre vivante.
    // winit déclare l'app « per-monitor DPI aware » : les coordonnées sont en pixels physiques.
    let ok = unsafe { GetCursorPos(&mut pt) != 0 && ScreenToClient(hwnd, &mut pt) != 0 };
    ok.then(|| (f64::from(pt.x), f64::from(pt.y)))
}

pub fn set_click_through(window: &Window, through: bool) {
    let Some(hwnd) = hwnd(window) else { return };
    if through {
        update_ex_style(hwnd, OVERLAY_STYLES | WS_EX_TRANSPARENT, 0);
    } else {
        update_ex_style(hwnd, OVERLAY_STYLES, WS_EX_TRANSPARENT);
    }
}

pub fn set_shown(window: &Window, shown: bool) {
    let Some(hwnd) = hwnd(window) else { return };
    // SAFETY: `hwnd` est une fenêtre vivante.
    unsafe { ShowWindow(hwnd, if shown { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
    // au cas où winit aurait recalculé les styles entre-temps
    update_ex_style(hwnd, OVERLAY_STYLES, 0);
}

fn update_ex_style(hwnd: HWND, set: u32, clear: u32) {
    // SAFETY: lecture/écriture des styles étendus d'une fenêtre vivante qu'on possède.
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let wanted = (current | set) & !clear;
        if wanted != current {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted as isize);
            // fait prendre en compte les nouveaux styles, en restant au premier plan
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

fn hwnd(window: &Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}
