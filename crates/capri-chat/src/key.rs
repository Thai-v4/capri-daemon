//! La clé d'API d'un fournisseur : ce qu'on en accepte, où elle a le droit d'aller, et comment
//! elle ne se montre jamais.
//!
//! La clé vit dans le trousseau du système (c'est le travail de `capri-platform`) ; ici, on ne
//! touche ni au trousseau ni au réseau. Une [`ApiKey`] ne s'affiche pas : elle n'a pas de
//! `Display`, son `Debug` la masque, et le seul moyen d'en lire le texte est d'en faire un en-tête
//! `Authorization` pour une adresse qui a le droit de la recevoir ([`authorization`]).

use std::fmt;

use crate::endpoint::Endpoint;

/// Longueur maximale d'une clé. Celles des fournisseurs font quelques dizaines de caractères : au-delà,
/// ce n'est pas une clé (un fichier collé par erreur, par exemple).
pub const MAX_CHARS: usize = 1024;

/// Ce qui remplace un morceau de clé dans un texte venu d'ailleurs.
pub const MASK: &str = "[clé masquée]";

/// Le plus court morceau de clé masqué dans un texte : assez long pour ne pas masquer un mot
/// ordinaire, assez court pour qu'un morceau de clé ne passe pas.
const MIN_FRAGMENT_CHARS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("la clé est vide")]
    Empty,
    #[error("la clé dépasse {MAX_CHARS} caractères")]
    TooLong,
    #[error("une clé ne contient ni espace, ni accent, ni retour à la ligne")]
    NotPlain,
    #[error("une clé ne part qu'en https, ou vers cette machine : jamais en clair sur le réseau")]
    InClear,
}

/// Une clé d'API, vérifiée. Jamais affichée.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// Lit une clé, telle que l'utilisateur l'a tapée ou collée : les espaces et les retours à la
    /// ligne autour sont retirés. Une clé est faite de caractères ASCII visibles, sans espace.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let key = text.trim();
        if key.is_empty() {
            return Err(KeyError::Empty);
        }
        if key.chars().count() > MAX_CHARS {
            return Err(KeyError::TooLong);
        }
        if !key.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(KeyError::NotPlain);
        }
        Ok(Self(key.to_owned()))
    }

    /// Le texte de la clé, pour le ranger dans le trousseau. Rien d'autre.
    pub fn reveal_for_keychain(&self) -> &str {
        &self.0
    }

    /// `text`, où chaque morceau de la clé (la clé entière, ou un bout d'au moins quelques
    /// caractères) est remplacé par [`MASK`]. Pour citer un message venu d'un fournisseur, qui
    /// pourrait la répéter.
    pub fn redact(&self, text: &str) -> String {
        let key: Vec<char> = self.0.chars().collect();
        let shortest = MIN_FRAGMENT_CHARS.min(key.len());
        let text: Vec<char> = text.chars().collect();
        let mut redacted = String::with_capacity(text.len());
        let mut at = 0;
        while at < text.len() {
            // le plus long morceau de la clé qui commence ici
            let longest = (0..key.len())
                .map(|start| key[start..].iter().zip(&text[at..]).take_while(|(a, b)| a == b).count())
                .max()
                .unwrap_or(0);
            if longest >= shortest {
                if !redacted.ends_with(MASK) {
                    redacted.push_str(MASK);
                }
                at += longest;
            } else {
                redacted.push(text[at]);
                at += 1;
            }
        }
        redacted
    }
}

/// Jamais la clé elle-même, même dans un journal de débogage.
impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(…)")
    }
}

/// La valeur de l'en-tête `Authorization` qui porte `key` vers `endpoint`, si elle a le droit d'y
/// aller : en `https://`, ou en `http://` vers cette machine. Jamais en clair sur le réseau.
pub fn authorization(key: &ApiKey, endpoint: &Endpoint) -> Result<String, KeyError> {
    if !endpoint.may_carry_secret() {
        return Err(KeyError::InClear);
    }
    Ok(format!("Bearer {}", key.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> ApiKey {
        ApiKey::parse(text).expect("une clé valable")
    }

    fn endpoint(url: &str) -> Endpoint {
        Endpoint::parse(url).expect("adresse valable")
    }

    #[test]
    fn a_pasted_key_loses_the_blanks_around_it() {
        assert_eq!(key("  sk-or-v1-abc123\n").reveal_for_keychain(), "sk-or-v1-abc123");
        assert_eq!(key("\tcle_de~test.42/+=\r\n").reveal_for_keychain(), "cle_de~test.42/+=");
        assert_eq!(key(&"x".repeat(MAX_CHARS)).reveal_for_keychain().len(), MAX_CHARS);
    }

    #[test]
    fn what_is_not_a_key_is_refused() {
        assert_eq!(ApiKey::parse(""), Err(KeyError::Empty));
        assert_eq!(ApiKey::parse(" \n\t "), Err(KeyError::Empty));
        assert_eq!(ApiKey::parse(&"x".repeat(MAX_CHARS + 1)), Err(KeyError::TooLong));
        for text in ["deux mots", "ligne\nsuivante", "avec\ttabulation", "clé", "sk-🙂", "nul\0", "\u{7f}"]
        {
            assert_eq!(ApiKey::parse(text), Err(KeyError::NotPlain), "{text:?}");
        }
    }

    #[test]
    fn a_key_never_shows_itself() {
        let secret = key("sk-tres-secret-123456");
        assert_eq!(format!("{secret:?}"), "ApiKey(…)");
        assert_eq!(format!("{:?}", Some(&secret)), "Some(ApiKey(…))");
    }

    #[test]
    fn a_key_only_leaves_over_https_or_to_this_machine() {
        let secret = key("sk-test");
        for url in [
            "https://openrouter.ai/api/v1",
            "https://api.deepseek.com",
            "https://192.168.1.20/v1",
            "http://localhost:1234/v1",
            "http://127.0.0.1:8080",
            "http://[::1]:1234",
        ] {
            assert_eq!(authorization(&secret, &endpoint(url)).as_deref(), Ok("Bearer sk-test"), "{url}");
        }
        for url in [
            "http://192.168.1.20:11434/v1",
            "http://10.0.0.5",
            "http://100.64.0.1:1234",
            "http://[fe80::1]",
            "http://[fd00::1]:8080",
            "http://[::ffff:10.0.0.1]",
        ] {
            assert_eq!(authorization(&secret, &endpoint(url)), Err(KeyError::InClear), "{url}");
        }
    }

    #[test]
    fn a_message_that_repeats_the_key_is_masked() {
        let secret = key("sk-or-v1-0123456789abcdef");
        assert_eq!(
            secret.redact("Invalid key: sk-or-v1-0123456789abcdef. Try again."),
            "Invalid key: [clé masquée]. Try again."
        );
        // des morceaux aussi, même coupés ou répétés
        assert_eq!(
            secret.redact("début sk-or-v1-01… et fin 6789abcdef"),
            "début [clé masquée]… et fin [clé masquée]"
        );
        assert_eq!(secret.redact("sk-or-v1-0123456789abcdefsk-or-v1-0123456789abcdef"), "[clé masquée]");
        // un mot ordinaire, ou un bout trop court pour dire quoi que ce soit, reste
        assert_eq!(secret.redact("User not found. sk-or ****cdef"), "User not found. sk-or ****cdef");
        assert_eq!(secret.redact(""), "");
        assert_eq!(secret.redact("Ça marche, déjà 🙂"), "Ça marche, déjà 🙂");
        // une clé très courte est masquée en entier
        assert_eq!(key("abc").redact("clé abc refusée"), "clé [clé masquée] refusée");
    }
}
