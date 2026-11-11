//! Autres systèmes (Linux…) : pas encore pris en charge. Capri s'affiche, les clics passent
//! toujours au travers, mais on ne peut pas l'attraper.

use capri_core::geom::Vec2;
use capri_core::perch::DeskFrame;
use winit::event_loop::EventLoopBuilder;
use winit::window::{Window, WindowAttributes};

pub fn configure_event_loop<T: 'static>(_builder: &mut EventLoopBuilder<T>) {}

pub fn overlay_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
}

pub fn prepare_overlay(_window: &Window) {}

pub fn cursor_in_window(_window: &Window) -> Option<(f64, f64)> {
    None
}

pub fn set_click_through(window: &Window, through: bool) {
    let _ = window.set_cursor_hittest(!through);
}

pub fn set_shown(window: &Window, shown: bool) {
    window.set_visible(shown);
}

/// Pas de liste des fenêtres ici (voir `desk`) : un repère sans décalage, qui ne sert pas.
pub fn desk_frame(_window: &Window) -> DeskFrame {
    DeskFrame { origin: Vec2::ZERO, pixels_per_unit: 1.0 }
}
