//! Ce que veut dire un appui de bouton sur elle, indépendamment du système.
//!
//! Sur macOS, « Ctrl + clic » est un clic secondaire à part entière, mais le système l'envoie à
//! l'app comme un clic principal avec la touche Ctrl enfoncée : c'est à l'app de le reconnaître.

/// Bouton de souris, tel que le système le rapporte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// Bouton principal (gauche).
    Primary,
    /// Bouton secondaire (droit, ou clic à deux doigts).
    Secondary,
    /// Molette, boutons latéraux…
    Other,
}

/// Ce qu'un appui sur elle déclenche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// On l'attrape.
    Grab,
    /// On ouvre son menu.
    Menu,
    /// Rien.
    Nothing,
}

/// Interprète un appui. `ctrl_click_is_secondary` : sur ce système, Ctrl + clic principal vaut
/// clic secondaire (vrai sur macOS).
pub fn interpret_press(button: Button, ctrl: bool, ctrl_click_is_secondary: bool) -> Press {
    match button {
        Button::Secondary => Press::Menu,
        Button::Primary if ctrl && ctrl_click_is_secondary => Press::Menu,
        Button::Primary => Press::Grab,
        Button::Other => Press::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_grabs_and_secondary_opens_the_menu() {
        for convention in [false, true] {
            assert_eq!(interpret_press(Button::Primary, false, convention), Press::Grab);
            assert_eq!(interpret_press(Button::Secondary, false, convention), Press::Menu);
            assert_eq!(interpret_press(Button::Other, false, convention), Press::Nothing);
        }
    }

    #[test]
    fn ctrl_click_opens_the_menu_only_where_it_is_the_convention() {
        // macOS : Ctrl + clic arrive comme un clic principal avec Ctrl
        assert_eq!(interpret_press(Button::Primary, true, true), Press::Menu);
        // ailleurs, Ctrl + clic reste un clic ordinaire
        assert_eq!(interpret_press(Button::Primary, true, false), Press::Grab);
        // Ctrl ne change rien aux autres boutons
        assert_eq!(interpret_press(Button::Secondary, true, true), Press::Menu);
        assert_eq!(interpret_press(Button::Other, true, true), Press::Nothing);
    }
}
