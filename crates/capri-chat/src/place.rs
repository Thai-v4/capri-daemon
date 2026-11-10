//! Où s'affiche un message, et donc quel conseil il donne.
//!
//! Le but de la fenêtre de réglages est que personne n'ait à ouvrir un terminal : ce que Capri
//! dit dans sa bulle ou dans la fenêtre ne renvoie jamais à une commande. Seul un terminal
//! (`capri chat …`) garde les siennes. Un message est écrit une seule fois ; c'est [`Place`] qui
//! choisit le conseil qui le termine, ici et nulle part ailleurs.

/// L'endroit où s'affichera le message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Une commande `capri chat …` : les commandes sont le bon conseil.
    Terminal,
    /// La bulle de Capri : elle renvoie aux réglages, dans son menu.
    Bubble,
    /// La fenêtre de réglages : elle dit quoi faire sur place.
    Window,
}

/// Ce que le conseil demande de faire d'une clé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyJob {
    /// La donner (ou la ranger).
    Give,
    /// La remettre, parce qu'elle est refusée.
    PutBack,
    /// La retirer.
    Forget,
}

impl Place {
    /// Comment faire ce que demande `job`, pour finir une phrase (« Pour me la donner : … »).
    ///
    /// `command` : la commande de terminal qui range la clé (`key::set_key_command`), ajoutée
    /// telle quelle pour un terminal. Pour `Give` et `PutBack`, la phrase finit là : le point est
    /// dedans, sauf après une commande. `Forget` se glisse entre parenthèses, sans point.
    pub fn how_to_key(self, job: KeyJob, command: &str) -> String {
        match (self, job) {
            (Self::Terminal, KeyJob::Forget) => format!("{command} --forget"),
            (Self::Terminal, KeyJob::Give | KeyJob::PutBack) => command.to_owned(),
            (Self::Bubble, KeyJob::Give | KeyJob::PutBack) => "ouvre Réglages… dans mon menu.".to_owned(),
            (Self::Bubble, KeyJob::Forget) => "ouvre Réglages… dans mon menu".to_owned(),
            (Self::Window, KeyJob::Give) => "saisis-la ci-dessus.".to_owned(),
            (Self::Window, KeyJob::PutBack) => "remets-la ci-dessus.".to_owned(),
            (Self::Window, KeyJob::Forget) => "bouton « Retirer la clé »".to_owned(),
        }
    }

    /// Comment lui donner un cerveau, pour finir « Pour m'en donner un : … ».
    pub fn how_to_give_brain(self) -> &'static str {
        match self {
            Self::Terminal => "capri chat use ollama",
            Self::Bubble => "ouvre Réglages… dans mon menu.",
            Self::Window => "choisis un fournisseur ci-dessus.",
        }
    }

    /// Où trouver la raison d'un réglage illisible : « … : {…} ». Là où Capri ne sait pas encore
    /// l'afficher, ce n'est pas promis : la fenêtre montre la raison dans son onglet, jamais
    /// ailleurs.
    pub fn where_to_see_why(self) -> &'static str {
        match self {
            Self::Terminal => "capri chat status te dira pourquoi.",
            Self::Bubble => "Réglages… dans mon menu te dira pourquoi.",
            Self::Window => "la raison est affichée dans cet onglet.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND: &str = "capri chat key openrouter";

    #[test]
    fn a_terminal_keeps_its_commands() {
        assert_eq!(Place::Terminal.how_to_key(KeyJob::Give, COMMAND), COMMAND);
        assert_eq!(Place::Terminal.how_to_key(KeyJob::PutBack, COMMAND), COMMAND);
        assert_eq!(Place::Terminal.how_to_key(KeyJob::Forget, COMMAND), "capri chat key openrouter --forget");
        assert_eq!(Place::Terminal.how_to_give_brain(), "capri chat use ollama");
        assert_eq!(Place::Terminal.where_to_see_why(), "capri chat status te dira pourquoi.");
    }

    #[test]
    fn the_bubble_points_at_the_settings_and_the_window_at_itself() {
        assert_eq!(Place::Bubble.how_to_key(KeyJob::Give, COMMAND), "ouvre Réglages… dans mon menu.");
        assert_eq!(Place::Bubble.how_to_key(KeyJob::PutBack, COMMAND), "ouvre Réglages… dans mon menu.");
        assert_eq!(Place::Window.how_to_key(KeyJob::PutBack, COMMAND), "remets-la ci-dessus.");
        assert_eq!(Place::Window.how_to_key(KeyJob::Give, COMMAND), "saisis-la ci-dessus.");
    }

    #[test]
    fn only_a_terminal_is_ever_told_to_type_a_command() {
        for place in [Place::Bubble, Place::Window] {
            let mut said = vec![place.how_to_give_brain().to_owned(), place.where_to_see_why().to_owned()];
            for job in [KeyJob::Give, KeyJob::PutBack, KeyJob::Forget] {
                said.push(place.how_to_key(job, COMMAND));
            }
            for text in said {
                assert!(!text.contains("capri ") && !text.contains("--"), "{place:?} : {text}");
            }
        }
    }
}
