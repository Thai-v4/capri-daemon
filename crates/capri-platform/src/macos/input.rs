//! Le champ de saisie : un petit panneau qui, lui, prend le clavier.
//!
//! Il est non activant comme celui de Capri — l'app en cours reste au premier plan — mais il
//! accepte de devenir fenêtre clé : le temps de taper, les touches lui arrivent. Dès qu'il se
//! ferme (Entrée, Échap, ou un clic ailleurs), le clavier revient à la fenêtre d'avant.
//!
//! Le champ est un vrai `NSTextField` : accents, émojis, méthodes de saisie, copier-coller et
//! dictée sont ceux du système. Les raccourcis d'édition (⌘V…), qu'une app sans menu Édition ne
//! transmet pas d'elle-même, passent par la vue qui le porte (`edit_keys`).

use std::cell::{Cell, OnceCell};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBackingStoreType, NSColor, NSFocusRingType, NSFont, NSPanel,
    NSStatusWindowLevel, NSTextField, NSTextFieldBezelStyle, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use super::Shared;
use super::edit_keys::EditKeysView;
use crate::OverlayEvent;

/// Taille du champ, en points.
pub const WIDTH: f64 = 340.0;
pub const HEIGHT: f64 = 30.0;
const FONT_SIZE: f64 = 15.0;

pub struct Ivars {
    shared: Rc<Shared>,
    field: OnceCell<Retained<NSTextField>>,
    /// Vrai entre l'ouverture et la fermeture : ce qui ferme le panneau ne le fait qu'une fois.
    open: Cell<bool>,
}

define_class!(
    // SAFETY:
    // - `NSPanel` accepte d'être sous-classé ; les méthodes surchargées ne touchent qu'à nos
    //   propres variables d'instance, et `resignKeyWindow` appelle l'implémentation d'origine.
    // - `InputPanel` n'implémente pas `Drop`.
    #[unsafe(super(NSPanel))]
    #[name = "CapriInputPanel"]
    #[ivars = Ivars]
    pub struct InputPanel;

    impl InputPanel {
        /// Contrairement au panneau de Capri, celui-ci existe pour recevoir le clavier.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }

        /// Entrée : c'est l'action du champ de texte.
        #[unsafe(method(capriSubmit:))]
        fn submit(&self, _sender: Option<&AnyObject>) {
            self.finish(true);
        }

        /// Échap.
        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            self.finish(false);
        }

        /// Le clavier part ailleurs : un clic dans une autre fenêtre, un changement d'app.
        #[unsafe(method(resignKeyWindow))]
        fn resign_key_window(&self) {
            // SAFETY: appelle l'implémentation de `NSPanel` de cette même méthode, qui ne prend
            // aucun argument et ne rend rien.
            let _: () = unsafe { msg_send![super(self), resignKeyWindow] };
            self.finish(false);
        }
    }
);

impl InputPanel {
    pub fn create(mtm: MainThreadMarker, shared: Rc<Shared>) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT));
        // Sans bordure, et non activant : il prend le clavier sans faire passer l'app au premier
        // plan.
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let this =
            mtm.alloc::<Self>().set_ivars(Ivars { shared, field: OnceCell::new(), open: Cell::new(false) });
        // SAFETY: initialiseur désigné de `NSPanel`, avec les types d'arguments de sa signature
        // (NSRect, NSWindowStyleMask, NSBackingStoreType, BOOL), appelé sur le thread principal.
        let panel: Retained<Self> = unsafe {
            msg_send![
                super(this),
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
        panel.setHasShadow(true);
        panel.setHidesOnDeactivate(false);
        // juste au-dessus de Capri
        panel.setLevel(NSStatusWindowLevel + 1);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );

        let field = NSTextField::initWithFrame(mtm.alloc(), frame);
        field.setBezeled(true);
        field.setBezelStyle(NSTextFieldBezelStyle::RoundedBezel);
        field.setFont(Some(&NSFont::systemFontOfSize(FONT_SIZE)));
        field.setFocusRingType(NSFocusRingType::None);
        field.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        // l'action ne part que sur Entrée, pas quand le champ perd le clavier
        if let Some(cell) = field.cell() {
            cell.setSendsActionOnEndEditing(false);
        }
        // SAFETY: la cible est le panneau, qui possède le champ et lui survit (le champ ne garde
        // pas sa cible en vie) ; le sélecteur désigne `capriSubmit:`, définie plus haut, qui
        // prend l'expéditeur en argument comme toute action.
        unsafe {
            field.setTarget(Some(&panel));
            field.setAction(Some(sel!(capriSubmit:)));
        }
        // la vue qui le porte fait marcher ⌘X, ⌘C, ⌘V, ⌘A et ⌘Z dans le champ, et ⌘W, ⌘Q, ⌘H le ferment
        let content = EditKeysView::create(mtm, frame);
        content.addSubview(&field);
        panel.setContentView(Some(&content));
        let _ = panel.ivars().field.set(field);
        panel
    }

    /// Ouvre le champ, vide, à l'endroit donné (repère de l'écran, en points) et lui donne le
    /// clavier.
    pub fn open(&self, origin: NSPoint, placeholder: &str) {
        let Some(field) = self.ivars().field.get() else { return };
        self.setFrame_display(NSRect::new(origin, NSSize::new(WIDTH, HEIGHT)), true);
        field.setStringValue(&NSString::from_str(""));
        field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
        self.ivars().open.set(true);
        self.makeKeyAndOrderFront(None);
        self.makeFirstResponder(Some(field));
    }

    pub fn is_open(&self) -> bool {
        self.ivars().open.get()
    }

    /// Ferme le champ et dit à l'application ce qu'il contenait (`submitted`) ou que la saisie
    /// est abandonnée. Sans effet s'il est déjà fermé.
    pub fn finish(&self, submitted: bool) {
        let ivars = self.ivars();
        if !ivars.open.replace(false) {
            return;
        }
        let text = ivars.field.get().map(|field| field.stringValue().to_string()).unwrap_or_default();
        // rend le clavier à la fenêtre d'avant ; rappelle `resignKeyWindow`, qui ne fera rien
        self.orderOut(None);
        ivars.shared.push(if submitted {
            OverlayEvent::InputSubmitted(text)
        } else {
            OverlayEvent::InputCancelled
        });
    }
}
