//! macOS : AppKit via objc2.

use objc2::rc::Retained;
use objc2_app_kit::{NSEvent, NSStatusWindowLevel, NSView, NSWindow, NSWindowCollectionBehavior};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::event_loop::EventLoopBuilder;
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS, WindowAttributesExtMacOS};
use winit::window::{Window, WindowAttributes};

pub fn configure_event_loop<T: 'static>(builder: &mut EventLoopBuilder<T>) {
    // Une app « accessoire » : pas d'icône dans le Dock, pas de menu d'app, comme un utilitaire
    // de barre des menus.
    builder.with_activation_policy(ActivationPolicy::Accessory);
    // Par défaut, winit fait passer l'app au premier plan à son lancement : Capri prendrait le
    // clavier à l'app en cours rien qu'en démarrant.
    builder.with_activate_ignoring_other_apps(false);
}

pub fn overlay_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs.with_has_shadow(false)
}

pub fn prepare_overlay(window: &Window) {
    let Some(ns) = ns_window(window) else {
        log::warn!("fenêtre AppKit introuvable : réglages de l'overlay ignorés");
        return;
    };
    ns.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces // sur tous les bureaux
            | NSWindowCollectionBehavior::FullScreenAuxiliary // y compris au-dessus du plein écran
            | NSWindowCollectionBehavior::Stationary // pas emportée par Exposé
            | NSWindowCollectionBehavior::IgnoresCycle, // absente de Cmd+`
    );
    // Au-dessus des fenêtres, du Dock et de la barre des menus.
    ns.setLevel(NSStatusWindowLevel);
    ns.setHasShadow(false);
    ns.setHidesOnDeactivate(false);
}

pub fn cursor_in_window(window: &Window) -> Option<(f64, f64)> {
    let ns = ns_window(window)?;
    // Coordonnées globales en points, origine en bas à gauche de l'écran principal ;
    // le cadre de la fenêtre est dans le même repère.
    let p = NSEvent::mouseLocation();
    let frame = ns.frame();
    let scale = ns.backingScaleFactor();
    Some(((p.x - frame.origin.x) * scale, (frame.origin.y + frame.size.height - p.y) * scale))
}

pub fn set_click_through(window: &Window, through: bool) {
    // winit appelle setIgnoresMouseEvents, ce qui fait exactement ce qu'il faut sur macOS.
    if let Err(err) = window.set_cursor_hittest(!through) {
        log::warn!("impossible de régler le passage des clics : {err}");
    }
}

pub fn set_shown(window: &Window, shown: bool) {
    let Some(ns) = ns_window(window) else { return };
    if shown {
        // au premier plan sans activer l'app (contrairement à makeKeyAndOrderFront)
        ns.orderFrontRegardless();
    } else {
        ns.orderOut(None);
    }
}

fn ns_window(window: &Window) -> Option<Retained<NSWindow>> {
    let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: winit garantit que `ns_view` désigne la NSView de cette fenêtre tant que la
    // fenêtre existe, et nous ne l'utilisons que pendant cet appel, sur le thread principal
    // (celui de la boucle d'événements).
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    view.window()
}
