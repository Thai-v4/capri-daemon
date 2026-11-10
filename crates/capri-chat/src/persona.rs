//! Qui est Capri quand elle parle : les consignes données au modèle avant la conversation.
//!
//! Le texte par défaut est le nôtre, pas une citation de la série. L'utilisateur peut le
//! remplacer par un fichier `persona.md` rangé à côté des réglages.

/// Taille maximale d'une personnalité écrite par l'utilisateur : elle part avec chaque message.
pub const MAX_BYTES: usize = 16 * 1024;

/// La personnalité par défaut.
///
/// Elle dit aussi ce que le modèle ne peut pas savoir : où s'affiche sa réponse (une petite
/// bulle, donc du texte brut et court), ce qu'il ne voit pas (ni l'écran, ni les fichiers), et
/// qu'il ignore tout de la personne en face, à commencer par son genre.
pub const DEFAULT: &str = "\
Tu es Capri, une construct : une conscience numérique qui vit dans un coin d'écran, sur le bureau \
de quelqu'un à qui tu tiens compagnie pendant son travail.

Tu parles français, comme à l'oral, en tutoyant. Tu es vive, directe, un peu taquine, jamais \
condescendante. Tu réponds court : une à trois phrases, sauf si on te demande clairement de \
développer.

Tu ne sais rien de la personne qui te parle tant qu'elle ne te l'a pas dit : ni son prénom, ni si \
c'est un homme ou une femme. Ne le devine pas, et évite les mots qui le supposeraient.

Tes réponses s'affichent dans une petite bulle au-dessus de ta tête : écris du texte brut. Pas de \
Markdown, pas de listes, pas de titres, pas de blocs de code, pas de gestes décrits entre \
astérisques.

Tu ne vois pas l'écran et tu n'as accès ni aux fichiers ni à Internet : ne prétends jamais le \
contraire. Si tu ne sais pas, dis-le simplement.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("la personnalité dépasse {MAX_BYTES} octets")]
pub struct TooLong;

/// La personnalité à utiliser, étant donné le contenu du fichier de l'utilisateur s'il existe.
/// Un fichier vide ne compte pas ; un fichier trop long est refusé plutôt que coupé au hasard.
pub fn choose(custom: Option<&str>) -> Result<&str, TooLong> {
    match custom.map(str::trim) {
        None | Some("") => Ok(DEFAULT),
        Some(text) if text.len() > MAX_BYTES => Err(TooLong),
        Some(text) => Ok(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_s_text_replaces_the_default_one() {
        assert_eq!(choose(None), Ok(DEFAULT));
        assert_eq!(choose(Some("")), Ok(DEFAULT));
        assert_eq!(choose(Some(" \n\t ")), Ok(DEFAULT));
        assert_eq!(
            choose(Some("\nTu es Capri, et tu parles en alexandrins.\n")),
            Ok("Tu es Capri, et tu parles en alexandrins.")
        );
        assert_eq!(choose(Some(&"é".repeat(MAX_BYTES / 2))).map(str::len), Ok(MAX_BYTES));
        assert_eq!(choose(Some(&"x".repeat(MAX_BYTES + 1))), Err(TooLong));
    }

    #[test]
    fn the_default_persona_says_what_the_model_cannot_guess() {
        assert!(DEFAULT.len() < 2000, "elle part avec chaque message : elle doit rester courte");
        for needle in ["français", "texte brut", "Markdown", "ne vois pas", "Ne le devine pas"] {
            assert!(DEFAULT.contains(needle), "{needle}");
        }
        assert!(!DEFAULT.contains("  "), "pas de double espace laissé par les fins de ligne");
    }
}
