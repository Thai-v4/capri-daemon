//! macOS : AppKit via objc2.
//!
//! La fenêtre de Capri est un `NSPanel` non activant que nous créons nous-mêmes : cliquer
//! dessus ne fait jamais passer l'app au premier plan et ne prend pas le clavier à la fenêtre
//! en cours. winit ne sait créer que des `NSWindow` qui deviennent fenêtre clé au clic ; ici il
//! ne fournit donc que la boucle d'événements.

pub(crate) mod edit_keys;
mod input;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ptr::NonNull;
use std::rc::Rc;

use dispatch2::MainThreadBound;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSBackingStoreType, NSColor, NSEvent,
    NSEventModifierFlags, NSEventPhase, NSPanel, NSScreen, NSStatusWindowLevel, NSView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes,
};
use objc2_quartz_core::{CADisplayLink, CAFrameRateRange};
use raw_window_handle::{
    AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    WindowHandle,
};
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopBuilder};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

use capri_core::geom::{Rect, Vec2};
use capri_core::perch::DeskFrame;

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
    /// L'écran a réveillé la boucle (son `CADisplayLink`) depuis la dernière fois qu'on l'a demandé.
    refreshed: Cell<bool>,
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

        /// La molette sur elle (la fenêtre ne reçoit la souris que là). Seulement avec ⌥ : sans la
        /// touche, rien ne se passe, comme avant.
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            if let Some(steps) = zoom_steps(event) {
                self.ivars().push(OverlayEvent::Zoom { steps });
            }
        }

        /// Résolution, densité ou écran principal modifiés (réglages d'affichage, écran
        /// branché ou débranché).
        #[unsafe(method(screenParametersChanged:))]
        fn screen_parameters_changed(&self, _notification: &NSNotification) {
            self.sync_geometry();
        }
    }
);

define_class!(
    // SAFETY: NSObject n'impose rien à ses sous-classes, et `FrameTarget` n'implémente pas `Drop`.
    // Elle ne vit que sur le thread principal, où le lien l'appelle (il est ajouté à sa boucle).
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CapriFrameTarget"]
    #[ivars = Rc<Shared>]
    struct FrameTarget;

    // SAFETY: NSObjectProtocol n'a rien à implémenter de plus pour un NSObject.
    unsafe impl NSObjectProtocol for FrameTarget {}

    impl FrameTarget {
        /// Appelée par le `CADisplayLink` à chaque rafraîchissement de l'écran, sur la boucle
        /// principale. Être appelée réveille la boucle (winit rappelle `about_to_wait` avant
        /// qu'elle se rendorme : c'est là que Capri fait son pas) ; on note seulement que c'est
        /// l'écran qui l'a réveillée, pour que l'app ne refasse pas son pas à chaque autre réveil.
        #[unsafe(method(step:))]
        fn step(&self, _link: &CADisplayLink) {
            self.ivars().refreshed.set(true);
        }
    }
);

impl FrameTarget {
    fn new(mtm: MainThreadMarker, shared: Rc<Shared>) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(shared);
        // SAFETY: l'initialiseur de NSObject, sur un objet tout juste alloué.
        unsafe { msg_send![super(this), init] }
    }
}

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
    /// Le champ de saisie, fermé tant qu'on ne le demande pas.
    input: Retained<input::InputPanel>,
    /// Le réveil au rythme de l'écran (macOS 14 et suivants), en pause tant qu'on ne le demande
    /// pas ; `None` là où le système ne le propose pas.
    frame_link: Option<Retained<CADisplayLink>>,
}

impl Drop for Overlay {
    fn drop(&mut self) {
        // la boucle principale le retient (et sa cible) tant qu'il n'est pas invalidé
        if let Some(link) = &self.frame_link {
            link.invalidate();
        }
    }
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
            refreshed: Cell::new(false),
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

        // Créé dès maintenant, encore fermé : s'il y a un problème avec ce panneau, autant le
        // découvrir au lancement qu'à la première question.
        let input = input::InputPanel::create(mtm, Rc::clone(&shared));
        let frame_link = frame_link(mtm, &view, &screen, Rc::clone(&shared));
        Ok(Self { panel, view, shared, input, frame_link })
    }

    /// La fréquence de rafraîchissement de son écran, en Hz (120 en ProMotion), d'après
    /// `NSScreen.maximumFramesPerSecond` (macOS 12 et suivants). `None` si le système ne la dit pas.
    pub fn refresh_rate(&self) -> Option<f64> {
        let mtm = MainThreadMarker::from(&*self.view);
        let screen = primary_screen(mtm)?;
        let fps =
            screen.respondsToSelector(sel!(maximumFramesPerSecond)).then(|| screen.maximumFramesPerSecond());
        fps.filter(|&fps| fps > 0).map(|fps| fps as f64)
    }

    /// Se réveiller à chaque rafraîchissement de l'écran (`true`), ou plus (`false`). Rend `true`
    /// si c'est l'écran qui réveille désormais la boucle ; `false` s'il ne le peut pas (avant
    /// macOS 14), et c'est alors à l'app de se réveiller elle-même assez souvent.
    pub fn follow_refresh(&self, on: bool) -> bool {
        let Some(link) = &self.frame_link else { return false };
        if link.isPaused() == on {
            link.setPaused(!on);
            self.shared.refreshed.set(false);
        }
        on
    }

    /// L'écran a-t-il réveillé la boucle depuis le dernier appel ?
    pub fn refreshed(&self) -> bool {
        self.shared.refreshed.replace(false)
    }

    pub fn show_input(&self, placeholder: &str, place: impl FnOnce(f64, f64) -> (f64, f64)) -> bool {
        // l'application raisonne en pixels physiques depuis le coin haut gauche de l'overlay ;
        // AppKit en points depuis le coin bas gauche de l'écran
        let scale = self.shared.scale.get();
        let (x, y) = place(input::WIDTH * scale, input::HEIGHT * scale);
        let frame = self.panel.frame();
        let origin = NSPoint::new(
            frame.origin.x + x / scale,
            frame.origin.y + frame.size.height - y / scale - input::HEIGHT,
        );
        self.input.open(origin, placeholder);
        true
    }

    pub fn hide_input(&self) {
        self.input.finish(false);
    }

    pub fn input_is_open(&self) -> bool {
        self.input.is_open()
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

    pub fn desk_frame(&self) -> DeskFrame {
        let mtm = MainThreadMarker::from(&*self.view);
        let panel = self.panel.frame();
        // l'overlay est sur l'écran principal : à défaut de le trouver, sa hauteur est la sienne
        let primary_height =
            primary_screen(mtm).map_or(panel.size.height, |screen| screen.frame().size.height);
        DeskFrame::from_appkit(appkit_rect(panel), primary_height as f32, self.shared.scale.get() as f32)
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

/// Un `CADisplayLink` de la vue, en pause, ajouté à la boucle principale : il la réveillera à
/// chaque rafraîchissement de l'écran où est la vue (`NSView.h` : « en phase avec l'écran de la
/// vue » ; il ne se déclenche pas quand la vue est cachée). `None` avant macOS 14, où
/// `displayLinkWithTarget:selector:` n'existe pas (l'appeler lèverait une exception).
fn frame_link(
    mtm: MainThreadMarker,
    view: &OverlayView,
    screen: &NSScreen,
    shared: Rc<Shared>,
) -> Option<Retained<CADisplayLink>> {
    if !view.respondsToSelector(sel!(displayLinkWithTarget:selector:)) {
        log::info!("pas de réveil au rythme de l'écran avant macOS 14 : un minuteur le remplace");
        return None;
    }
    let target = FrameTarget::new(mtm, shared);
    // SAFETY: `target` est un objet vivant (le lien le retient jusqu'à `invalidate`), dont la
    // méthode `step:` a la signature qu'exige `CADisplayLink.h` : `(void)selector:(CADisplayLink
    // *)sender`.
    let link = unsafe { view.displayLinkWithTarget_selector(&target, sel!(step:)) };
    link.setPaused(true);
    // ProMotion : demander toute la fréquence de l'écran, pas la cadence que le système choisirait
    if screen.respondsToSelector(sel!(maximumFramesPerSecond)) {
        let hz = screen.maximumFramesPerSecond() as f32;
        if hz > 0.0 {
            link.setPreferredFrameRateRange(CAFrameRateRange { minimum: hz, maximum: hz, preferred: hz });
        }
    }
    // SAFETY: la boucle principale (on est sur le thread principal, la vue en témoigne) et le mode
    // commun, constante de Foundation valable toute la vie du processus ; `CADisplayLink.h` : un lien
    // n'est ajouté qu'à une seule boucle, ce qu'on fait une fois.
    unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
    Some(link)
}

/// L'écran qui porte la barre des menus, à l'origine du repère global.
fn primary_screen(mtm: MainThreadMarker) -> Option<Retained<NSScreen>> {
    NSScreen::screens(mtm).firstObject()
}

/// Un cadre d'AppKit tel quel (origine en bas à gauche, y vers le haut), en `Rect`.
fn appkit_rect(frame: NSRect) -> Rect {
    Rect::new(
        Vec2::new(frame.origin.x as f32, frame.origin.y as f32),
        Vec2::new(frame.size.width as f32, frame.size.height as f32),
    )
}

/// Où serait l'overlay (il couvre l'écran principal), et sa taille en pixels, sans le créer : pour
/// `capri perch rehearse`. `None` hors du thread principal.
#[cfg(debug_assertions)]
pub(crate) fn main_screen_desk() -> Option<(DeskFrame, (u32, u32))> {
    let screen = primary_screen(MainThreadMarker::new()?)?;
    let (frame, scale) = (screen.frame(), screen.backingScaleFactor());
    let desk = DeskFrame::from_appkit(appkit_rect(frame), frame.size.height as f32, scale as f32);
    Some((desk, physical_size(frame, scale)))
}

fn physical_size(frame: NSRect, scale: f64) -> (u32, u32) {
    ((frame.size.width * scale).round() as u32, (frame.size.height * scale).round() as u32)
}

/// Sur le pavé tactile, combien de points de défilement font un cran de molette.
const POINTS_PER_STEP: f64 = 10.0;

/// Les crans d'un défilement avec ⌥ tenue, positifs vers le haut ; `None` sans ⌥, pendant l'élan
/// qui suit un geste, ou sans mouvement. D'après `NSEvent.h` : `scrollingDeltaY` est en points
/// quand `hasPreciseScrollingDeltas` (pavé tactile, Magic Mouse), en lignes sinon (molette), et il
/// est inversé selon les préférences de l'utilisateur, ce que dit `isDirectionInvertedFromDevice` :
/// on revient au sens du geste, doigts ou molette vers le haut pour la grandir.
fn zoom_steps(event: &NSEvent) -> Option<f64> {
    if !event.modifierFlags().contains(NSEventModifierFlags::Option)
        || event.momentumPhase() != NSEventPhase::None
    {
        return None;
    }
    let (dx, dy) = (event.scrollingDeltaX(), event.scrollingDeltaY());
    let delta = if dy.abs() >= dx.abs() { dy } else { dx };
    let delta = if event.isDirectionInvertedFromDevice() { -delta } else { delta };
    let steps = if event.hasPreciseScrollingDeltas() { delta / POINTS_PER_STEP } else { delta };
    (steps.is_finite() && steps != 0.0).then_some(steps)
}

/// Boutons au-delà de gauche et droite : 2 est la molette.
fn other_button(event: &NSEvent) -> MouseButton {
    match event.buttonNumber() {
        2 => MouseButton::Middle,
        n => MouseButton::Other(u16::try_from(n).unwrap_or(u16::MAX)),
    }
}
