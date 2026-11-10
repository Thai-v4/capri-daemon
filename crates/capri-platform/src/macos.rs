//! macOS : AppKit via objc2.
//!
//! La fenêtre de Capri est un `NSPanel` non activant que nous créons nous-mêmes : cliquer
//! dessus ne fait jamais passer l'app au premier plan et ne prend pas le clavier à la fenêtre
//! en cours. winit ne sait créer que des `NSWindow` qui deviennent fenêtre clé au clic ; ici il
//! ne fournit donc que la boucle d'événements.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ptr::NonNull;
use std::rc::Rc;

use dispatch2::MainThreadBound;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSBackingStoreType, NSColor, NSEvent,
    NSEventModifierFlags, NSPanel, NSScreen, NSStatusWindowLevel, NSView, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSPoint, NSRect};
use raw_window_handle::{
    AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    WindowHandle,
};
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopBuilder};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

use crate::{OverlayError, OverlayEvent};

pub fn configure_event_loop<T: 'static>(builder: &mut EventLoopBuilder<T>) {
    // Une app « accessoire » : pas d'icône dans le Dock, pas de menu d'app, comme un utilitaire
    // de barre des menus.
    builder.with_activation_policy(ActivationPolicy::Accessory);
    // Par défaut, winit fait passer l'app au premier plan à son lancement : Capri prendrait le
    // clavier à l'app en cours rien qu'en démarrant.
    builder.with_activate_ignoring_other_apps(false);
}

/// Ce que la vue et l'[`Overlay`] partagent. Tout se passe sur le thread principal.
struct Shared {
    events: RefCell<VecDeque<OverlayEvent>>,
    /// Taille (pixels physiques) et densité telles que l'application les connaît.
    size: Cell<(u32, u32)>,
    scale: Cell<f64>,
    redraw: Cell<bool>,
}

impl Shared {
    fn push(&self, event: OverlayEvent) {
        self.events.borrow_mut().push_back(event);
    }
}

define_class!(
    // SAFETY:
    // - `NSPanel` accepte d'être sous-classé ; on ne surcharge que deux accesseurs sans état.
    // - `OverlayPanel` n'implémente pas `Drop`.
    #[unsafe(super(NSPanel))]
    #[name = "CapriOverlayPanel"]
    struct OverlayPanel;

    impl OverlayPanel {
        /// Jamais fenêtre clé : le clavier reste à la fenêtre en cours.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

define_class!(
    // SAFETY:
    // - `NSView` accepte d'être sous-classé ; les méthodes surchargées ne touchent qu'à nos
    //   propres variables d'instance.
    // - `OverlayView` n'implémente pas `Drop`.
    #[unsafe(super(NSView))]
    #[name = "CapriOverlayView"]
    #[ivars = Rc<Shared>]
    struct OverlayView;

    impl OverlayView {
        /// L'app n'est jamais active : sans ceci, AppKit avalerait chaque clic sur elle.
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.pressed(MouseButton::Left, event);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            self.ivars().push(OverlayEvent::Released { button: MouseButton::Left });
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.pressed(MouseButton::Right, event);
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, _event: &NSEvent) {
            self.ivars().push(OverlayEvent::Released { button: MouseButton::Right });
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            self.pressed(other_button(event), event);
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            self.ivars().push(OverlayEvent::Released { button: other_button(event) });
        }

        /// Résolution, densité ou écran principal modifiés (réglages d'affichage, écran
        /// branché ou débranché).
        #[unsafe(method(screenParametersChanged:))]
        fn screen_parameters_changed(&self, _notification: &NSNotification) {
            self.sync_geometry();
        }
    }
);

impl OverlayView {
    fn pressed(&self, button: MouseButton, event: &NSEvent) {
        let ctrl = event.modifierFlags().contains(NSEventModifierFlags::Control);
        self.ivars().push(OverlayEvent::Pressed { button, ctrl });
    }

    /// Recale le panneau sur l'écran principal et prévient l'application si sa taille ou sa
    /// densité ont changé.
    fn sync_geometry(&self) {
        let Some(screen) = primary_screen(MainThreadMarker::from(self)) else { return };
        let frame = screen.frame();
        if let Some(panel) = self.window()
            && panel.frame() != frame
        {
            panel.setFrame_display(frame, true);
        }
        let shared = self.ivars();
        let scale = screen.backingScaleFactor();
        if shared.scale.replace(scale) != scale {
            shared.push(OverlayEvent::ScaleFactorChanged(scale));
        }
        let size = physical_size(frame, scale);
        if shared.size.replace(size) != size {
            shared.push(OverlayEvent::Resized { width: size.0, height: size.1 });
        }
    }
}

pub struct Overlay {
    panel: Retained<OverlayPanel>,
    view: Retained<OverlayView>,
    shared: Rc<Shared>,
}

impl Overlay {
    pub fn new(_event_loop: &ActiveEventLoop) -> Result<Self, OverlayError> {
        let mtm = MainThreadMarker::new().ok_or(OverlayError::NotMainThread)?;
        let screen = primary_screen(mtm).ok_or(OverlayError::NoScreen)?;
        let frame = screen.frame();
        let scale = screen.backingScaleFactor();
        let shared = Rc::new(Shared {
            events: RefCell::new(VecDeque::new()),
            size: Cell::new(physical_size(frame, scale)),
            scale: Cell::new(scale),
            redraw: Cell::new(false),
        });

        // Sans bordure (valeur 0) et non activant : un clic dessus n'active pas l'app.
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel = mtm.alloc::<OverlayPanel>().set_ivars(());
        // SAFETY: initialiseur désigné de `NSPanel`, avec les types d'arguments de sa signature
        // (NSRect, NSWindowStyleMask, NSBackingStoreType, BOOL), appelé sur le thread principal.
        let panel: Retained<OverlayPanel> = unsafe {
            msg_send![
                super(panel),
                initWithContentRect: frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false,
            ]
        };
        // SAFETY: le panneau est tenu par un `Retained` ; sans ce réglage, AppKit le libérerait
        // une seconde fois à sa fermeture.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        // Un panneau se cache par défaut quand son app n'est pas active : la nôtre ne l'est jamais.
        panel.setHidesOnDeactivate(false);
        panel.setIgnoresMouseEvents(true);
        // Au-dessus des fenêtres, du Dock et de la barre des menus.
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces // sur tous les bureaux
                | NSWindowCollectionBehavior::FullScreenAuxiliary // y compris au-dessus du plein écran
                | NSWindowCollectionBehavior::Stationary // pas emportée par Exposé
                | NSWindowCollectionBehavior::IgnoresCycle, // absente de Cmd+`
        );

        let view = mtm.alloc::<OverlayView>().set_ivars(Rc::clone(&shared));
        // SAFETY: initialiseur désigné de `NSView`, qui prend un NSRect, appelé sur le thread
        // principal.
        let view: Retained<OverlayView> =
            unsafe { msg_send![super(view), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), frame.size)] };
        panel.setContentView(Some(&view));

        // SAFETY: `view` est un objet valide dont la méthode `screenParametersChanged:`, définie
        // plus haut, prend bien une NSNotification. Le centre de notifications ne garde qu'une
        // référence faible sur l'observateur et l'oublie tout seul à sa destruction.
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &view,
                sel!(screenParametersChanged:),
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
            );
        }

        Ok(Self { panel, view, shared })
    }

    pub fn surface_handle(&self) -> SurfaceHandle {
        SurfaceHandle(MainThreadBound::new(self.view.clone(), MainThreadMarker::from(&*self.view)))
    }

    pub fn size(&self) -> (u32, u32) {
        self.shared.size.get()
    }

    pub fn scale_factor(&self) -> f64 {
        self.shared.scale.get()
    }

    pub fn cursor(&self) -> Option<(f64, f64)> {
        // Coordonnées globales en points, origine en bas à gauche de l'écran principal ;
        // le cadre du panneau est dans le même repère.
        let p = NSEvent::mouseLocation();
        let frame = self.panel.frame();
        let scale = self.shared.scale.get();
        Some(((p.x - frame.origin.x) * scale, (frame.origin.y + frame.size.height - p.y) * scale))
    }

    pub fn set_click_through(&self, through: bool) {
        self.panel.setIgnoresMouseEvents(through);
    }

    pub fn set_shown(&self, shown: bool) {
        if shown {
            // au premier plan sans activer l'app (contrairement à makeKeyAndOrderFront)
            self.panel.orderFrontRegardless();
        } else {
            self.panel.orderOut(None);
        }
    }

    pub fn request_redraw(&self) {
        self.shared.redraw.set(true);
    }

    pub fn pre_present_notify(&self) {}

    pub fn on_window_event(&self, _event: &WindowEvent) {}

    pub fn drain_events(&self) -> Vec<OverlayEvent> {
        // On vide la file avant de rendre la main : traiter un événement peut ouvrir un menu,
        // dont la boucle imbriquée fait arriver d'autres clics dans cette même file.
        let mut events: Vec<_> = self.shared.events.borrow_mut().drain(..).collect();
        if self.shared.redraw.replace(false) {
            events.push(OverlayEvent::RedrawRequested);
        }
        events
    }

    pub fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(view_handle(&self.view))
    }
}

/// La vue du panneau, sous une forme que wgpu peut garder : `Send + Sync`, mais lisible sur le
/// seul thread principal.
pub struct SurfaceHandle(MainThreadBound<Retained<OverlayView>>);

impl HasWindowHandle for SurfaceHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let mtm = MainThreadMarker::new().ok_or(HandleError::Unavailable)?;
        Ok(view_handle(self.0.get(mtm)))
    }
}

impl HasDisplayHandle for SurfaceHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::appkit())
    }
}

fn view_handle(view: &OverlayView) -> WindowHandle<'_> {
    let raw = RawWindowHandle::AppKit(AppKitWindowHandle::new(NonNull::from(view).cast()));
    // SAFETY: le pointeur désigne une NSView vivante, et la poignée ne peut pas survivre à
    // l'emprunt de `view` dont elle porte la durée de vie.
    unsafe { WindowHandle::borrow_raw(raw) }
}

/// L'écran qui porte la barre des menus, à l'origine du repère global.
fn primary_screen(mtm: MainThreadMarker) -> Option<Retained<NSScreen>> {
    NSScreen::screens(mtm).firstObject()
}

fn physical_size(frame: NSRect, scale: f64) -> (u32, u32) {
    ((frame.size.width * scale).round() as u32, (frame.size.height * scale).round() as u32)
}

/// Boutons au-delà de gauche et droite : 2 est la molette.
fn other_button(event: &NSEvent) -> MouseButton {
    match event.buttonNumber() {
        2 => MouseButton::Middle,
        n => MouseButton::Other(u16::try_from(n).unwrap_or(u16::MAX)),
    }
}
