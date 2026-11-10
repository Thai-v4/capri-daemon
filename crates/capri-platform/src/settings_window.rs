//! La fenêtre de réglages : elle affiche un formulaire décrit par `capri_core::form` et rapporte
//! ce que l'utilisateur y fait, sans rien savoir de ce qu'elle règle.
//!
//! macOS : un `NSPanel` à onglets, une rubrique par onglet, avec les contrôles et les couleurs du
//! système (clair comme sombre). C'est, avec le champ de saisie, la seule fenêtre de Capri qui
//! prend le clavier : quand on l'ouvre, et seulement parce qu'on l'a demandée. Elle ne fait pas
//! passer Capri au premier plan (panneau non activant) : en se fermant, elle rend le clavier à la
//! fenêtre d'avant. Aucune boucle modale : Capri continue de vivre pendant qu'elle est ouverte.
//!
//! Ailleurs, pas encore de fenêtre de réglages : [`SettingsWindow::open`] le dit
//! ([`SettingsWindowError::Unavailable`]).

use capri_core::form::{Form, FormEvent};

#[cfg(target_os = "macos")]
#[path = "macos/settings_window.rs"]
mod imp;

#[cfg(not(target_os = "macos"))]
mod imp {
    //! Pas encore de fenêtre de réglages sur ce système : rien ne peut être ouvert.

    use capri_core::form::{Form, FormEvent};

    use super::SettingsWindowError;

    pub struct SettingsWindow(());

    impl SettingsWindow {
        pub fn open(_form: &Form) -> Result<Self, SettingsWindowError> {
            Err(SettingsWindowError::Unavailable)
        }

        pub fn show(&self) {}

        pub fn update(&self, _form: &Form) {}

        pub fn is_open(&self) -> bool {
            false
        }

        pub fn close(&self) {}

        pub fn drain(&self) -> Vec<FormEvent> {
            Vec::new()
        }

        pub fn has_keyboard(&self) -> bool {
            false
        }

        #[cfg(debug_assertions)]
        pub fn rehearse_typing(&self, _id: &str, _text: &str) {}

        #[cfg(debug_assertions)]
        pub fn rehearse_press(&self, _id: &str) {}
    }
}

/// Ce système a-t-il une fenêtre de réglages ?
pub const AVAILABLE: bool = cfg!(target_os = "macos");

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SettingsWindowError {
    #[error("pas encore de fenêtre de réglages sur ce système")]
    Unavailable,
    #[error("la fenêtre de réglages doit être créée sur le thread principal")]
    NotMainThread,
}

/// La fenêtre de réglages. Une seule à la fois : la rouvrir, c'est appeler [`SettingsWindow::show`]
/// sur celle qui existe. À créer et à utiliser sur le thread principal.
pub struct SettingsWindow(imp::SettingsWindow);

impl SettingsWindow {
    /// Crée la fenêtre avec ce formulaire, la montre au premier plan et lui donne le clavier.
    pub fn open(form: &Form) -> Result<Self, SettingsWindowError> {
        imp::SettingsWindow::open(form).map(Self)
    }

    /// La ramène au premier plan, avec le clavier.
    pub fn show(&self) {
        self.0.show();
    }

    /// Affiche ce formulaire à la place du précédent. Les contrôles dont l'identifiant et la
    /// sorte n'ont pas changé sont gardés, avec ce qu'on est en train d'y taper.
    pub fn update(&self, form: &Form) {
        self.0.update(form);
    }

    /// Est-elle encore ouverte ? Elle se ferme d'elle-même (bouton de fermeture, ⌘W, Échap).
    pub fn is_open(&self) -> bool {
        self.0.is_open()
    }

    pub fn close(&self) {
        self.0.close();
    }

    /// Ce que l'utilisateur a fait depuis le dernier appel, dans l'ordre. Avant chaque appui sur
    /// un bouton ou choix dans une liste, un champ en cours de saisie est validé
    /// ([`FormEvent::Committed`]) : l'action porte sur ce qui est affiché.
    pub fn drain(&self) -> Vec<FormEvent> {
        self.0.drain()
    }

    /// A-t-elle le clavier (est-elle la fenêtre clé) ?
    pub fn has_keyboard(&self) -> bool {
        self.0.has_keyboard()
    }

    /// Pour les essais en version de débogage : remplit ce champ (secret ou non) comme si
    /// l'utilisateur y avait collé `text`.
    #[cfg(debug_assertions)]
    pub fn rehearse_typing(&self, id: &str, text: &str) {
        self.0.rehearse_typing(id, text);
    }

    /// Pour les essais en version de débogage : presse ce bouton, comme d'un clic.
    #[cfg(debug_assertions)]
    pub fn rehearse_press(&self, id: &str) {
        self.0.rehearse_press(id);
    }
}
