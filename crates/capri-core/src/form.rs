//! Un formulaire de réglages, décrit sans rien savoir de l'écran : des rubriques faites de
//! lignes, et ce que l'utilisateur y fait.
//!
//! La fenêtre qui l'affiche (`capri_platform::SettingsWindow`) ne connaît que ces types : ni
//! fournisseurs, ni clés, ni voix. Le modèle des réglages (`capri_chat::settings`) produit un
//! [`Form`] à partir de son état et reçoit des [`FormEvent`] ; l'app fait le lien.
//!
//! Chaque ligne porte un identifiant stable : c'est lui qui désigne la ligne dans les
//! événements, et qui permet à la fenêtre de garder le même champ (et ce qu'on est en train d'y
//! taper) d'un affichage à l'autre. Les identifiants commencent par celui de leur rubrique :
//! `brain.provider`, `voice.enabled`.
//!
//! Un champ secret ([`Control::Secret`]) n'a jamais de valeur dans la description : on ne peut
//! pas y remettre un secret. Ce qui y a été tapé n'en sort qu'une fois, dans un [`Secret`], quand
//! on appuie sur le bouton qui le demande, et le champ est vidé aussitôt.

use std::fmt;

/// Tout le formulaire : son titre et ses rubriques, dans l'ordre.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Form {
    pub title: String,
    pub sections: Vec<Section>,
}

/// Une rubrique : un onglet de la fenêtre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub rows: Vec<Row>,
}

/// Une ligne : un libellé (vide s'il n'y en a pas) et ce qu'on y voit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub label: String,
    pub control: Control,
}

impl Row {
    pub fn new(id: impl Into<String>, label: impl Into<String>, control: Control) -> Self {
        Self { id: id.into(), label: label.into(), control }
    }
}

/// Ce qu'une ligne montre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Control {
    /// Un paragraphe qui passe à la ligne, sélectionnable (pour le copier).
    Text { text: String },
    /// Une note d'état : où on en est, ce qui vient de se passer.
    Note { text: String, tone: Tone },
    /// Un champ d'une ligne. `search` : un champ de recherche (avec sa loupe et sa croix).
    Field { value: String, placeholder: String, enabled: bool, search: bool },
    /// Un champ dont la frappe ne s'affiche pas. Jamais prérempli. `submit` : le bouton
    /// qu'Entrée presse.
    Secret { placeholder: String, enabled: bool, submit: Option<String> },
    /// Une liste déroulante ; `selected` désigne une des `options`.
    Choice { options: Vec<String>, selected: usize, enabled: bool },
    /// Une case à cocher.
    Check { title: String, checked: bool, enabled: bool },
    /// Des boutons côte à côte.
    Buttons(Vec<Button>),
}

impl Control {
    /// La sorte de contrôle : deux lignes de même identifiant et de même sorte sont le même
    /// contrôle, mis à jour plutôt que recréé.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Text { .. } => Kind::Text,
            Self::Note { .. } => Kind::Note,
            Self::Field { search: false, .. } => Kind::Field,
            Self::Field { search: true, .. } => Kind::Search,
            Self::Secret { .. } => Kind::Secret,
            Self::Choice { .. } => Kind::Choice,
            Self::Check { .. } => Kind::Check,
            Self::Buttons(buttons) => Kind::Buttons(buttons.len()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Text,
    Note,
    Field,
    Search,
    Secret,
    Choice,
    Check,
    /// Avec leur nombre.
    Buttons(usize),
}

/// Un bouton.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    /// Le champ secret dont ce bouton emporte le contenu, s'il y en a un : la fenêtre le lit au
    /// moment de l'appui, le vide, et le joint à [`FormEvent::Pressed`].
    pub secret: Option<String>,
}

impl Button {
    pub fn new(id: impl Into<String>, title: impl Into<String>, enabled: bool) -> Self {
        Self { id: id.into(), title: title.into(), enabled, secret: None }
    }
}

/// La couleur d'une note : celles du système, en clair comme en sombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Discrète : une explication.
    Quiet,
    /// En cours.
    Busy,
    /// Tout va bien.
    Good,
    /// À lire avant de continuer.
    Warning,
    /// Ça n'a pas marché.
    Bad,
}

/// Ce que l'utilisateur vient de faire dans le formulaire.
#[derive(Debug, PartialEq, Eq)]
pub enum FormEvent {
    /// Le texte d'un champ a changé (à chaque frappe).
    Edited {
        id: String,
        value: String,
    },
    /// La saisie d'un champ est terminée : Entrée, ou le clavier est passé ailleurs.
    Committed {
        id: String,
        value: String,
    },
    /// Une autre option a été choisie dans une liste.
    Chose {
        id: String,
        index: usize,
    },
    Toggled {
        id: String,
        checked: bool,
    },
    /// Un bouton a été pressé ; `secret` : ce qu'il emporte d'un champ secret, s'il en
    /// emporte un.
    Pressed {
        id: String,
        secret: Option<Secret>,
    },
    /// La fenêtre a été fermée.
    Closed,
}

impl FormEvent {
    /// L'identifiant de la ligne ou du bouton concerné ; `None` pour la fermeture.
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Edited { id, .. }
            | Self::Committed { id, .. }
            | Self::Chose { id, .. }
            | Self::Toggled { id, .. }
            | Self::Pressed { id, .. } => Some(id),
            Self::Closed => None,
        }
    }
}

/// Ce qui a été tapé dans un champ secret. Ne s'affiche jamais : pas de `Display`, un `Debug`
/// masqué, pas de `Clone` ; le seul moyen d'en lire le texte est [`Secret::reveal`].
#[derive(PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(text: String) -> Self {
        Self(text)
    }

    /// Le texte, pour le vérifier et le ranger. Rien d'autre.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

impl Form {
    /// La ligne qui porte cet identifiant, dans n'importe quelle rubrique.
    pub fn row(&self, id: &str) -> Option<&Row> {
        self.sections.iter().flat_map(|section| &section.rows).find(|row| row.id == id)
    }

    /// Le bouton qui porte cet identifiant.
    pub fn button(&self, id: &str) -> Option<&Button> {
        self.sections.iter().flat_map(|section| &section.rows).find_map(|row| match &row.control {
            Control::Buttons(buttons) => buttons.iter().find(|button| button.id == id),
            _ => None,
        })
    }

    /// Les identifiants qui reviennent deux fois : il ne doit y en avoir aucun.
    pub fn duplicate_ids(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut twice = Vec::new();
        let ids = self.sections.iter().map(|section| section.id.as_str()).chain(
            self.sections.iter().flat_map(|section| &section.rows).flat_map(|row| {
                let buttons: Vec<&str> = match &row.control {
                    Control::Buttons(buttons) => buttons.iter().map(|button| button.id.as_str()).collect(),
                    _ => Vec::new(),
                };
                std::iter::once(row.id.as_str()).chain(buttons)
            }),
        );
        for id in ids {
            if !seen.insert(id) {
                twice.push(id.to_owned());
            }
        }
        twice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_shows_itself() {
        let secret = Secret::new("sk-tres-secret-0123456789".to_owned());
        assert_eq!(format!("{secret:?}"), "Secret(…)");
        let event = FormEvent::Pressed { id: "brain.key.save".to_owned(), secret: Some(secret) };
        let shown = format!("{event:?}");
        assert!(!shown.contains("sk-tres"), "{shown}");
        assert!(shown.contains("brain.key.save") && shown.contains("Secret(…)"), "{shown}");
        if let FormEvent::Pressed { secret: Some(secret), .. } = &event {
            assert_eq!(secret.reveal(), "sk-tres-secret-0123456789");
        }
    }

    fn sample() -> Form {
        let mut save = Button::new("a.save", "Enregistrer", true);
        save.secret = Some("a.key".to_owned());
        Form {
            title: "Réglages".to_owned(),
            sections: vec![
                Section {
                    id: "a".to_owned(),
                    title: "A".to_owned(),
                    rows: vec![
                        Row::new(
                            "a.key",
                            "Clé",
                            Control::Secret {
                                placeholder: String::new(),
                                enabled: true,
                                submit: Some("a.save".to_owned()),
                            },
                        ),
                        Row::new(
                            "a.actions",
                            "",
                            Control::Buttons(vec![save, Button::new("a.forget", "Retirer", false)]),
                        ),
                    ],
                },
                Section {
                    id: "b".to_owned(),
                    title: "B".to_owned(),
                    rows: vec![Row::new(
                        "b.on",
                        "",
                        Control::Check { title: "Oui".to_owned(), checked: true, enabled: true },
                    )],
                },
            ],
        }
    }

    #[test]
    fn rows_and_buttons_are_found_by_their_id() {
        let form = sample();
        assert_eq!(form.row("b.on").map(|row| row.control.kind()), Some(Kind::Check));
        assert_eq!(form.row("a.actions").map(|row| row.control.kind()), Some(Kind::Buttons(2)));
        assert_eq!(form.button("a.save").and_then(|button| button.secret.as_deref()), Some("a.key"));
        assert!(form.button("a.forget").is_some_and(|button| !button.enabled));
        assert!(form.row("a.save").is_none() && form.button("a.key").is_none());
        assert!(form.duplicate_ids().is_empty());

        let mut twice = form.clone();
        twice.sections[1].rows.push(Row::new("a.key", "", Control::Text { text: String::new() }));
        assert_eq!(twice.duplicate_ids(), ["a.key"]);
    }

    #[test]
    fn an_event_names_what_it_is_about() {
        assert_eq!(FormEvent::Chose { id: "a.b".to_owned(), index: 2 }.id(), Some("a.b"));
        assert_eq!(FormEvent::Closed.id(), None);
        let search =
            Control::Field { value: String::new(), placeholder: String::new(), enabled: true, search: true };
        assert_eq!(search.kind(), Kind::Search, "un champ de recherche n'est pas un champ ordinaire");
    }
}
