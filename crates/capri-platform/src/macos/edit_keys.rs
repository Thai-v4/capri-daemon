//! ⌘X, ⌘C, ⌘V, ⌘A, ⌘Z et ⇧⌘Z dans les champs de Capri.
//!
//! Dans une app ordinaire, ces raccourcis sont ceux du menu Édition : AppKit cherche un
//! équivalent clavier dans la fenêtre clé, puis dans les menus de la barre des menus, dont
//! « Coller » envoie `paste:` au premier répondeur (le champ). Capri est une app « accessoire »,
//! sans barre des menus, et le menu principal que crée winit n'a pas de menu Édition : sans rien
//! faire, ⌘V ne colle rien.
//!
//! D'après le guide des événements d'AppKit (« Handling Key Equivalents »), `NSApp` passe un
//! équivalent clavier à la fenêtre clé, qui le fait descendre dans sa hiérarchie de vues par
//! `performKeyEquivalent:`, avant les menus. Le guide déconseille de surcharger celui de
//! `NSWindow`, mais demande aux vues qui en reconnaissent un de le traiter : la vue de contenu de
//! nos panneaux, [`EditKeysView`], reconnaît ces raccourcis (par `charactersIgnoringModifiers`,
//! comme le veut le guide) et envoie l'action correspondante au premier répondeur, exactement
//! comme le ferait le menu Édition. Le champ (son éditeur de texte) la traite ; sans répondeur
//! qui la connaisse, l'équivalent continue son chemin.
//!
//! Un champ secret refuse lui-même de couper ou de copier son contenu.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{DefinedClass, MainThreadMarker, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSView};
use objc2_foundation::NSRect;

/// L'action d'édition que demande ce raccourci, s'il en est un.
pub fn edit_action(event: &NSEvent) -> Option<Sel> {
    let flags = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    let command = NSEventModifierFlags::Command;
    let shifted = command | NSEventModifierFlags::Shift;
    // la touche telle que la disposition du clavier la nomme (AZERTY compris), Maj comprise
    let key = event.charactersIgnoringModifiers()?.to_string();
    if flags == command {
        match key.as_str() {
            "x" => Some(sel!(cut:)),
            "c" => Some(sel!(copy:)),
            "v" => Some(sel!(paste:)),
            "a" => Some(sel!(selectAll:)),
            "z" => Some(sel!(undo:)),
            _ => None,
        }
    } else if flags == shifted && key.eq_ignore_ascii_case("z") {
        Some(sel!(redo:))
    } else {
        None
    }
}

/// ⌘W : fermer la fenêtre.
fn is_close(event: &NSEvent) -> bool {
    let flags = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    flags == NSEventModifierFlags::Command
        && event.charactersIgnoringModifiers().is_some_and(|key| key.to_string() == "w")
}

pub struct Ivars {
    /// ⌘W ferme-t-il la fenêtre ?
    closes: Cell<bool>,
}

define_class!(
    // SAFETY:
    // - `NSView` accepte d'être sous-classé ; la méthode surchargée appelle l'implémentation
    //   d'origine pour tout ce qu'elle ne traite pas.
    // - `EditKeysView` n'implémente pas `Drop`.
    #[unsafe(super(NSView))]
    #[name = "CapriEditKeysView"]
    #[ivars = Ivars]
    pub struct EditKeysView;

    impl EditKeysView {
        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            self.handle(event)
        }
    }
);

impl EditKeysView {
    /// Traite les raccourcis d'édition, et ⌘W s'il le faut ; le reste suit son chemin.
    fn handle(&self, event: &NSEvent) -> bool {
        if let Some(action) = edit_action(event) {
            let app = NSApplication::sharedApplication(MainThreadMarker::from(self));
            // SAFETY: les sélecteurs sont ceux des actions standard d'édition, qui prennent
            // l'expéditeur en argument ; sans cible (`None`), AppKit les envoie au premier
            // répondeur de la fenêtre clé, et rend `false` si personne ne les connaît.
            if unsafe { app.sendAction_to_from(action, None, Some(self)) } {
                return true;
            }
        } else if self.ivars().closes.get()
            && is_close(event)
            && let Some(window) = self.window()
        {
            window.performClose(None);
            return true;
        }
        // SAFETY: appelle l'implémentation de `NSView`, avec le même événement.
        unsafe { msg_send![super(self), performKeyEquivalent: event] }
    }

    /// `closes` : ⌘W ferme la fenêtre qui la porte.
    pub fn create(mtm: MainThreadMarker, frame: NSRect, closes: bool) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(Ivars { closes: Cell::new(closes) });
        // SAFETY: initialiseur désigné de `NSView`, qui prend un NSRect, appelé sur le thread
        // principal.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}
