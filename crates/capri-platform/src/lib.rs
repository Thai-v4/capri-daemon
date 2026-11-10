//! La seule couche qui parle au système d'exploitation.
//!
//! Capri vit dans une fenêtre transparente qui couvre l'écran. Pour qu'elle ne gêne jamais :
//! - la fenêtre laisse passer les clics, sauf quand le curseur est sur elle ;
//! - elle ne vole pas le focus et n'apparaît ni dans le Dock ni dans la barre des tâches ;
//! - sur macOS, elle suit tous les bureaux (Spaces) et passe au-dessus des apps en plein écran.
//!
//! Comme la fenêtre ignore la souris la plupart du temps, elle ne reçoit pas les mouvements :
//! on lit donc la position du curseur directement auprès du système.

use winit::event_loop::EventLoopBuilder;
use winit::window::{Window, WindowAttributes};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

#[cfg(not(any(target_os = "macos", windows)))]
mod other;
#[cfg(not(any(target_os = "macos", windows)))]
use other as imp;

/// Ce système sait-il lire le curseur et laisser passer les clics au pixel près ?
/// Sinon, Capri s'affiche mais ne peut pas être attrapée.
pub const INTERACTIVE: bool = cfg!(any(target_os = "macos", windows));

/// Sur ce système, Ctrl + clic principal vaut-il un clic secondaire ? Sur macOS oui, et le
/// système l'envoie à l'app comme un clic principal avec Ctrl enfoncé.
pub const CTRL_CLICK_IS_SECONDARY: bool = cfg!(target_os = "macos");

/// Réglages de la boucle d'événements (macOS : pas d'icône dans le Dock).
pub fn configure_event_loop<T: 'static>(builder: &mut EventLoopBuilder<T>) {
    imp::configure_event_loop(builder);
}

/// Attributs propres à la plateforme, à ajouter avant de créer la fenêtre.
pub fn overlay_attributes(attrs: WindowAttributes) -> WindowAttributes {
    imp::overlay_attributes(attrs)
}

/// Une fois la fenêtre créée : niveau, bureaux, focus, ombre.
pub fn prepare_overlay(window: &Window) {
    imp::prepare_overlay(window);
}

/// Position du curseur dans la fenêtre, en pixels physiques, même quand elle ignore la souris.
pub fn cursor_in_window(window: &Window) -> Option<(f64, f64)> {
    imp::cursor_in_window(window)
}

/// `true` : les clics traversent la fenêtre ; `false` : elle les reçoit.
pub fn set_click_through(window: &Window, through: bool) {
    imp::set_click_through(window, through);
}

/// Afficher ou cacher sans voler le focus de l'app au premier plan.
pub fn set_shown(window: &Window, shown: bool) {
    imp::set_shown(window, shown);
}
