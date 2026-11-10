//! Le cerveau provisoire : il répète ce qu'on lui dit.
//!
//! Aucun modèle, aucun réseau. Il sert à vérifier la saisie (accents, émojis, longs textes) et la
//! bulle (remplissage progressif, pages) avant de brancher un vrai fournisseur.

/// Ce que Capri répond à `message` tant qu'elle n'a pas de cerveau.
pub fn reply_to(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        return "Tu n'as rien dit.".to_owned();
    }
    format!("Tu as dit : « {message} ». Je n'ai pas encore de cerveau, alors je répète ce qu'on me dit.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_what_it_was_told_exactly() {
        let reply = reply_to("  Ça marche, l'accent circonflexe ? 🙂  ");
        assert!(reply.contains("« Ça marche, l'accent circonflexe ? 🙂 »"), "{reply}");
        assert!(reply.starts_with("Tu as dit"));
    }

    #[test]
    fn says_so_when_there_was_nothing_to_repeat() {
        assert_eq!(reply_to("   "), "Tu n'as rien dit.");
    }
}
