//! Le cerveau de secours : il répète ce qu'on lui dit.
//!
//! Aucun modèle, aucun réseau : c'est lui qui répond tant qu'aucun cerveau n'a été choisi. Il
//! sert aussi à vérifier la saisie (accents, émojis, longs textes) et la bulle (remplissage
//! progressif, pages) sans dépendre d'un fournisseur.

use crate::place::Place;

/// Ce que Capri répond à `message` tant qu'elle n'a pas de cerveau ; `place` : où ça s'affiche,
/// pour le conseil qui termine sa réponse.
pub fn reply_to(place: Place, message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        return "Tu n'as rien dit.".to_owned();
    }
    format!(
        "Tu as dit : « {message} ». Je n'ai pas encore de cerveau, alors je répète. Pour m'en donner un : \
         {}",
        place.how_to_give_brain()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_what_it_was_told_exactly() {
        let reply = reply_to(Place::Bubble, "  Ça marche, l'accent circonflexe ? 🙂  ");
        assert!(reply.contains("« Ça marche, l'accent circonflexe ? 🙂 »"), "{reply}");
        assert!(reply.starts_with("Tu as dit"));
    }

    #[test]
    fn says_so_when_there_was_nothing_to_repeat() {
        assert_eq!(reply_to(Place::Bubble, "   "), "Tu n'as rien dit.");
    }

    #[test]
    fn the_advice_depends_on_where_it_is_shown() {
        assert!(
            reply_to(Place::Bubble, "Salut")
                .ends_with("Pour m'en donner un : ouvre Réglages… dans mon menu.")
        );
        assert!(reply_to(Place::Terminal, "Salut").ends_with("Pour m'en donner un : capri chat use ollama"));
    }
}
