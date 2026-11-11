//! macOS, en version de débogage seulement : une petite fenêtre d'essai, pour mesurer comment
//! Capri suit une fenêtre qu'on déplace (`capri perch window`). N'existe pas dans la version
//! livrée.
//!
//! Une fenêtre ordinaire (couche 0), opaque, sans bordure ni titre, ouverte par un processus à
//! part (les fenêtres de Capri elle-même ne comptent pas pour se poser). Elle ne prend jamais le
//! clavier : l'app est « accessoire » (ni icône dans le Dock, ni menu) et la fenêtre est seulement
//! mise devant (`orderFrontRegardless`), sans activer l'app. Elle se ferme quand on la lâche.

use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSScreen, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize};

pub struct TestWindow {
    window: Retained<NSWindow>,
    /// La hauteur de l'écran principal, en points : AppKit compte depuis son bas, CoreGraphics (et
    /// cette fenêtre d'essai) depuis son haut.
    primary_height: f64,
}

impl TestWindow {
    /// Ouvre la fenêtre, son coin haut gauche en (`x`, `y`) points depuis le coin haut gauche de
    /// l'écran principal (le repère de la liste des fenêtres). `None` hors du thread principal, ou
    /// sans écran.
    pub fn open(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let app = NSApplication::sharedApplication(mtm);
        // ni icône dans le Dock, ni menu ; l'app n'est jamais activée
        if !app.setActivationPolicy(NSApplicationActivationPolicy::Accessory) {
            return None;
        }
        let primary_height = NSScreen::screens(mtm).firstObject()?.frame().size.height;
        let frame = NSRect::new(NSPoint::new(x, primary_height - y - height), NSSize::new(width, height));
        // SAFETY: initialiseur désigné de `NSWindow`, avec les types de sa signature (NSRect,
        // NSWindowStyleMask, NSBackingStoreType, BOOL), sur le thread principal.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc::<NSWindow>(),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: la fenêtre est tenue par un `Retained` ; sans ce réglage, AppKit la libérerait
        // une seconde fois à sa fermeture.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setBackgroundColor(Some(&NSColor::darkGrayColor()));
        window.orderFrontRegardless();
        Some(Self { window, primary_height })
    }

    /// Son numéro, celui de la liste des fenêtres.
    pub fn number(&self) -> u64 {
        u64::try_from(self.window.windowNumber()).unwrap_or_default()
    }

    /// La déplace, son coin haut gauche en (`x`, `y`) points (même repère qu'à l'ouverture).
    pub fn move_to(&self, x: f64, y: f64) {
        self.window.setFrameTopLeftPoint(NSPoint::new(x, self.primary_height - y));
    }

    /// Laisse tourner la boucle du thread principal pendant `duration` : c'est là qu'AppKit envoie
    /// ses changements au serveur de fenêtres.
    pub fn pump(&self, duration: Duration) {
        let until = NSDate::dateWithTimeIntervalSinceNow(duration.as_secs_f64());
        NSRunLoop::currentRunLoop().runUntilDate(&until);
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        self.window.close();
    }
}
