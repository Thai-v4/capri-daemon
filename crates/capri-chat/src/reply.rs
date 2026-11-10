//! Une réponse arrive par morceaux : c'est ce qui permet à la bulle de se remplir pendant que le
//! modèle écrit encore.

/// Ce qu'un cerveau envoie pendant qu'il répond.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplyEvent {
    /// Un morceau de texte de plus, à ajouter au bout de ce qui est déjà arrivé.
    Delta(String),
    /// La réponse est complète.
    Done,
    /// La réponse s'arrête là, avec la raison à montrer à l'utilisateur.
    Failed(String),
}

/// Découpe un texte en morceaux qui, mis bout à bout, le redonnent exactement : un mot et les
/// espaces qui le suivent. C'est à peu près le grain auquel un modèle envoie sa réponse.
pub fn chunks(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut in_spaces = false;
    for (at, c) in text.char_indices() {
        if c.is_whitespace() {
            in_spaces = true;
        } else if in_spaces {
            pieces.push(&text[start..at]);
            start = at;
            in_spaces = false;
        }
    }
    if start < text.len() {
        pieces.push(&text[start..]);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_put_back_together_give_the_original_text() {
        for text in [
            "",
            "mot",
            "deux mots",
            "  des   espaces\tpartout \n",
            "Ça c'est « noté », déjà !",
            "日本語 の テキスト",
        ] {
            assert_eq!(chunks(text).concat(), text, "{text:?}");
        }
    }

    #[test]
    fn each_chunk_is_a_word_with_its_trailing_spaces() {
        assert_eq!(chunks("Tu as dit : « bonjour »."), ["Tu ", "as ", "dit ", ": ", "« ", "bonjour ", "»."]);
        assert_eq!(chunks("seul"), ["seul"]);
        assert!(chunks("").is_empty());
    }
}
