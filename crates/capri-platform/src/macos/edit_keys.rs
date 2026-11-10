//! ⌘X, ⌘C, ⌘V, ⌘A, ⌘Z et ⇧⌘Z dans les champs de Capri, et ⌘W, ⌘Q, ⌘H qui ferment le panneau.
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
//! ⌘W, ⌘Q et ⌘H ferment le panneau et lui seul (voir [`shortcut`]).
//!
//! Un champ secret refuse lui-même de couper ou de copier son contenu.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSView};
use objc2_foundation::NSRect;

/// Ce que demande un raccourci reconnu par nos panneaux.
#[derive(Debug, PartialEq, Eq)]
enum Shortcut {
    /// Une action d'édition, à envoyer au premier répondeur.
    Edit(Sel),
    /// ⌘W, ⌘Q ou ⌘H : fermer le panneau, et lui seul.
    Dismiss,
}

/// Le raccourci que forment ces modificateurs et cette touche, s'il en est un. `key` est la
/// touche telle que la disposition du clavier la nomme (AZERTY compris), Maj non comprise.
///
/// ⌘Q et ⌘H sont ceux du menu par défaut de winit (Quitter, Masquer) : tant qu'un de nos
/// panneaux est la fenêtre clé, Capri est l'app active et ce menu répondrait. Les recevoir ici,
/// avant les menus, les empêche d'arriver jusqu'à lui : Capri ne se quitte que par son propre
/// menu, et ne se masque que par son propre raccourci.
fn shortcut(flags: NSEventModifierFlags, key: &str) -> Option<Shortcut> {
    let command = NSEventModifierFlags::Command;
    let shifted = command | NSEventModifierFlags::Shift;
    if flags == command {
        match key {
            "x" => Some(Shortcut::Edit(sel!(cut:))),
            "c" => Some(Shortcut::Edit(sel!(copy:))),
            "v" => Some(Shortcut::Edit(sel!(paste:))),
            "a" => Some(Shortcut::Edit(sel!(selectAll:))),
            "z" => Some(Shortcut::Edit(sel!(undo:))),
            "w" | "q" | "h" => Some(Shortcut::Dismiss),
            _ => None,
        }
    } else if flags == shifted && key.eq_ignore_ascii_case("z") {
        Some(Shortcut::Edit(sel!(redo:)))
    } else {
        None
    }
}

/// Le raccourci que porte cet événement.
fn event_shortcut(event: &NSEvent) -> Option<Shortcut> {
    let flags = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    let key = event.charactersIgnoringModifiers()?.to_string();
    shortcut(flags, &key)
}

define_class!(
    // SAFETY:
    // - `NSView` accepte d'être sous-classé ; la méthode surchargée appelle l'implémentation
    //   d'origine pour tout ce qu'elle ne traite pas.
    // - `EditKeysView` n'implémente pas `Drop`.
    #[unsafe(super(NSView))]
    #[name = "CapriEditKeysView"]
    pub struct EditKeysView;

    impl EditKeysView {
        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            self.handle(event)
        }
    }
);

impl EditKeysView {
    /// Traite les raccourcis d'édition et ceux qui ferment le panneau ; le reste suit son chemin.
    fn handle(&self, event: &NSEvent) -> bool {
        match event_shortcut(event) {
            Some(Shortcut::Edit(action)) => {
                let app = NSApplication::sharedApplication(MainThreadMarker::from(self));
                // SAFETY: les sélecteurs sont ceux des actions standard d'édition, qui prennent
                // l'expéditeur en argument ; sans cible (`None`), AppKit les envoie au premier
                // répondeur de la fenêtre clé, et rend `false` si personne ne les connaît.
                if unsafe { app.sendAction_to_from(action, None, Some(self)) } {
                    return true;
                }
            }
            Some(Shortcut::Dismiss) => {
                if let Some(window) = self.window() {
                    // Même chemin qu'Échap : chaque panneau sait se fermer par `cancelOperation:`
                    // (celui des réglages se ferme, le champ de saisie abandonne la saisie et
                    // rend le clavier). On rend `true` : l'équivalent est consommé et n'arrive
                    // jamais au menu de winit.
                    // SAFETY: `cancelOperation:` est une méthode de `NSResponder`, donc de toute
                    // fenêtre ; elle prend l'expéditeur (ici aucun) et ne rend rien.
                    let _: () = unsafe { msg_send![&*window, cancelOperation: None::<&AnyObject>] };
                }
                return true;
            }
            None => {}
        }
        // SAFETY: appelle l'implémentation de `NSView`, avec le même événement.
        unsafe { msg_send![super(self), performKeyEquivalent: event] }
    }

    pub fn create(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        // SAFETY: initialiseur désigné de `NSView`, qui prend un NSRect, appelé sur le thread
        // principal.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND: NSEventModifierFlags = NSEventModifierFlags::Command;

    #[test]
    fn quit_hide_and_close_only_dismiss_the_panel() {
        for key in ["q", "h", "w"] {
            assert_eq!(shortcut(COMMAND, key), Some(Shortcut::Dismiss), "⌘{key}");
        }
    }

    #[test]
    fn other_modifiers_do_not_dismiss() {
        // ⌥⌘H (Masquer les autres) et ⇧⌘Q (se déconnecter) ne sont pas des nôtres
        for key in ["q", "h", "w"] {
            assert_eq!(shortcut(COMMAND | NSEventModifierFlags::Option, key), None);
            assert_eq!(shortcut(COMMAND | NSEventModifierFlags::Shift, key), None);
            assert_eq!(shortcut(NSEventModifierFlags::empty(), key), None);
        }
    }

    #[test]
    fn edit_shortcuts_are_kept() {
        assert_eq!(shortcut(COMMAND, "v"), Some(Shortcut::Edit(sel!(paste:))));
        assert_eq!(shortcut(COMMAND, "z"), Some(Shortcut::Edit(sel!(undo:))));
        assert_eq!(shortcut(COMMAND | NSEventModifierFlags::Shift, "Z"), Some(Shortcut::Edit(sel!(redo:))));
        assert_eq!(shortcut(COMMAND, "n"), None);
    }
}
