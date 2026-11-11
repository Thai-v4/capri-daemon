//! La clé d'API d'un fournisseur : ce qu'on en accepte, où elle a le droit d'aller, et comment
//! elle ne se montre jamais.
//!
//! La clé vit dans le trousseau du système (c'est le travail de `capri-platform`) ; ici, on ne
//! touche ni au trousseau ni au réseau. Une [`ApiKey`] ne s'affiche pas : elle n'a pas de
//! `Display`, son `Debug` la masque, et le seul moyen d'en lire le texte, hors du trousseau, est
//! d'en faire l'en-tête qui la porte ([`key_header`]) : `Authorization: Bearer …` pour les
//! fournisseurs au format d'OpenAI, `x-api-key: …` pour Anthropic. Le nom et la valeur de cet
//! en-tête sont décidés ici ; `capri-net` ne fait que le poser.
//!
//! Une clé est liée à l'adresse pour laquelle elle a été donnée. Elle est rangée sous un compte
//! qui nomme le fournisseur et l'adresse ([`KeyAccount`] : `openrouter@openrouter.ai:443`), relue
//! avec ce compte ([`BoundKey`]), et [`key_header`] refuse de la mettre dans une requête vers
//! une autre adresse. Choisir une autre adresse (`--url`) n'emporte donc jamais la clé : il faut en
//! donner une pour cette adresse-là.

use std::fmt;

use crate::anthropic;
use crate::brain::{Brain, Provider};
use crate::endpoint::Endpoint;
use crate::wire::Wire;

/// Longueur maximale d'une clé. Celles des fournisseurs font quelques dizaines de caractères : au-delà,
/// ce n'est pas une clé (un fichier collé par erreur, par exemple).
pub const MAX_CHARS: usize = 1024;

/// Ce qui remplace un morceau de clé dans un texte venu d'ailleurs.
pub const MASK: &str = "[clé masquée]";

/// Le plus court morceau de clé masqué dans un texte : assez long pour ne pas masquer un mot
/// ordinaire, assez court pour qu'un morceau de clé ne passe pas.
const MIN_FRAGMENT_CHARS: usize = 8;

/// Le plus court morceau masqué dans le message d'un refus de la clé elle-même.
const MIN_TRACE_CHARS: usize = 4;

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
    #[error("cette clé a été donnée pour une autre adresse : elle n'est envoyée qu'à celle-là")]
    OtherAddress,
}

/// Le compte du trousseau sous lequel est rangée la clé d'un fournisseur pour une adresse :
/// `openrouter@openrouter.ai:443`.
///
/// L'adresse, c'est l'hôte et le port : l'hôte tel que [`Endpoint`] l'a lu (en minuscules, une
/// adresse IPv6 sous sa forme courte, entre crochets ici), et le port, celui du schéma s'il n'est
/// pas écrit. Deux ports du même hôte font deux comptes. Le chemin ne compte pas (`/api/v1` ou
/// `/v1`, c'est le même serveur), ni le schéma : `http://` et `https://` vers le même hôte et le
/// même port, c'est le même serveur, et une clé ne part de toute façon en clair que vers cette
/// machine ([`Endpoint::may_carry_secret`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyAccount {
    provider: Provider,
    /// `openrouter.ai:443`, `[::1]:1234`.
    address: String,
}

impl KeyAccount {
    pub fn new(provider: Provider, endpoint: &Endpoint) -> Self {
        // écrit ici, et pas emprunté à un texte fait pour l'affichage : changer ce nom ferait
        // perdre les clés déjà rangées
        let host =
            if endpoint.host.contains(':') { format!("[{}]", endpoint.host) } else { endpoint.host.clone() };
        Self { provider, address: format!("{host}:{}", endpoint.port) }
    }

    /// Le nom du compte dans le trousseau.
    pub fn name(&self) -> String {
        format!("{}@{}", self.provider.id(), self.address)
    }

    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// L'adresse pour laquelle est la clé, pour les messages : `openrouter.ai:443`.
    pub fn address(&self) -> &str {
        &self.address
    }
}

/// Le nom de compte d'une version précédente de Capri : l'identifiant du fournisseur seul, sans
/// adresse. Une clé rangée sous ce nom n'est jamais lue pour être envoyée : aucun [`KeyAccount`]
/// ne porte ce nom (le sien contient toujours `@`). On peut seulement savoir si elle est là, et la
/// retirer.
pub fn legacy_account(provider: Provider) -> &'static str {
    provider.id()
}

/// L'adresse pour laquelle `capri chat key <fournisseur>` range ou retire une clé : celle qu'on lui
/// donne (`--url`), sinon celle du cerveau choisi s'il est de ce fournisseur, sinon l'adresse
/// habituelle du fournisseur. `None` : il n'y en a aucune (un serveur personnalisé, sans `--url`,
/// quand Capri ne pense pas avec lui).
pub fn key_endpoint(
    provider: Provider,
    given: Option<&Endpoint>,
    chosen: Option<&Brain>,
) -> Option<Endpoint> {
    if let Some(given) = given {
        return Some(given.clone());
    }
    if let Some(brain) = chosen.filter(|brain| brain.provider() == provider) {
        return Some(brain.endpoint().clone());
    }
    provider.default_url().and_then(|url| Endpoint::parse(url).ok())
}

/// La commande qui range la clé de `provider` pour `endpoint`, `chosen` étant le cerveau choisi.
/// `--url` n'y figure que si, sans lui, elle viserait une autre adresse ([`key_endpoint`]).
pub fn set_key_command(provider: Provider, endpoint: &Endpoint, chosen: Option<&Brain>) -> String {
    let implied = key_endpoint(provider, None, chosen).map(|implied| KeyAccount::new(provider, &implied));
    if implied == Some(KeyAccount::new(provider, endpoint)) {
        format!("capri chat key {}", provider.id())
    } else {
        format!("capri chat key {} --url {}", provider.id(), endpoint.url(""))
    }
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
        self.redact_from(text, MIN_FRAGMENT_CHARS)
    }

    /// Comme [`ApiKey::redact`], mais jusqu'aux plus petits morceaux : pour le message d'un
    /// fournisseur qui vient de refuser la clé, et qui en cite volontiers le début ou la fin
    /// (« sk-…abcd »). Quitte à masquer un mot ordinaire qui lui ressemblerait.
    pub fn redact_any_trace(&self, text: &str) -> String {
        self.redact_from(text, MIN_TRACE_CHARS)
    }

    fn redact_from(&self, text: &str, shortest: usize) -> String {
        let key: Vec<char> = self.0.chars().collect();
        let shortest = shortest.min(key.len());
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

/// Une clé, avec le compte sous lequel elle était rangée : l'adresse pour laquelle elle a été
/// donnée. C'est sous cette forme seulement qu'une clé peut partir ([`key_header`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundKey {
    account: KeyAccount,
    key: ApiKey,
}

impl BoundKey {
    /// `key`, lue sous `account`.
    pub fn new(account: KeyAccount, key: ApiKey) -> Self {
        Self { account, key }
    }

    pub fn account(&self) -> &KeyAccount {
        &self.account
    }

    /// Voir [`ApiKey::redact`].
    pub fn redact(&self, text: &str) -> String {
        self.key.redact(text)
    }

    /// `text`, sans le moindre morceau de la clé : voir [`ApiKey::redact_any_trace`].
    pub fn redact_any_trace(&self, text: &str) -> String {
        self.key.redact_any_trace(text)
    }
}

/// L'en-tête qui porte une clé : son nom, et sa valeur, qui contient la clé. Il ne s'affiche
/// pas : son `Debug` ne montre que le nom.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyHeader {
    name: &'static str,
    value: String,
}

impl KeyHeader {
    /// `Authorization`, ou `x-api-key`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// La valeur, à poser dans la requête et nulle part ailleurs.
    pub fn reveal_for_request(&self) -> &str {
        &self.value
    }
}

/// Jamais la clé, même dans un journal de débogage.
impl fmt::Debug for KeyHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyHeader({}: …)", self.name)
    }
}

/// L'en-tête qui porte `key` vers `endpoint` pour `provider`, si elle a le droit d'y aller :
/// seulement vers l'adresse pour laquelle elle a été donnée, à ce fournisseur, et seulement en
/// `https://`, ou en `http://` vers cette machine. Jamais en clair sur le réseau.
///
/// Son nom dépend du format du fournisseur : `Authorization: Bearer <clé>` pour celui d'OpenAI,
/// `x-api-key: <clé>` pour Anthropic
/// (<https://platform.claude.com/docs/en/build-with-claude/streaming>, relue le 11 novembre 2026).
pub fn key_header(key: &BoundKey, provider: Provider, endpoint: &Endpoint) -> Result<KeyHeader, KeyError> {
    if key.account != KeyAccount::new(provider, endpoint) {
        return Err(KeyError::OtherAddress);
    }
    if !endpoint.may_carry_secret() {
        return Err(KeyError::InClear);
    }
    Ok(match provider.wire() {
        Wire::OpenAi => KeyHeader { name: "Authorization", value: format!("Bearer {}", key.key.0) },
        Wire::Anthropic => KeyHeader { name: anthropic::KEY_HEADER, value: key.key.0.clone() },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_that_quotes_the_ends_of_the_key_shows_none_of_it() {
        let key = ApiKey::parse("sk-test-0123456789abcdef").expect("une clé");
        // la façon dont des fournisseurs rappellent la clé qu'ils refusent : son début et sa fin
        assert_eq!(
            key.redact_any_trace("Incorrect API key provided: sk-t****cdef. Check it."),
            "Incorrect API key provided: [clé masquée]****[clé masquée]. Check it."
        );
        assert_eq!(
            key.redact_any_trace("User not found."),
            "User not found.",
            "un message ordinaire reste entier"
        );
        assert_eq!(key.redact_any_trace("bad key sk-test-0123456789abcdef"), "bad key [clé masquée]");
        // le masquage ordinaire laisse passer un morceau aussi court : c'est pour cela que l'autre existe
        assert_eq!(key.redact("fin de clé : cdef"), "fin de clé : cdef");
        assert_eq!(key.redact_any_trace("fin de clé : cdef"), "fin de clé : [clé masquée]");
        // trois caractères ne disent rien de la clé, et masquer si court abîmerait tout message
        assert_eq!(key.redact_any_trace("code def"), "code def");
    }

    fn key(text: &str) -> ApiKey {
        ApiKey::parse(text).expect("une clé valable")
    }

    /// L'en-tête qui porterait la clé, nom et valeur, ou pourquoi elle ne part pas.
    fn authorization(key: &BoundKey, provider: Provider, endpoint: &Endpoint) -> Result<String, KeyError> {
        key_header(key, provider, endpoint).map(|header| {
            assert_eq!(header.name(), "Authorization");
            header.reveal_for_request().to_owned()
        })
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

    /// `text`, donnée pour l'adresse `url` de ce fournisseur.
    fn bound(provider: Provider, url: &str, text: &str) -> BoundKey {
        BoundKey::new(KeyAccount::new(provider, &endpoint(url)), key(text))
    }

    fn account(provider: Provider, url: &str) -> String {
        KeyAccount::new(provider, &endpoint(url)).name()
    }

    #[test]
    fn a_key_only_leaves_over_https_or_to_this_machine() {
        for url in [
            "https://openrouter.ai/api/v1",
            "https://api.deepseek.com",
            "https://192.168.1.20/v1",
            "http://localhost:1234/v1",
            "http://127.0.0.1:8080",
            "http://[::1]:1234",
        ] {
            let secret = bound(Provider::Custom, url, "sk-test");
            assert_eq!(
                authorization(&secret, Provider::Custom, &endpoint(url)).as_deref(),
                Ok("Bearer sk-test"),
                "{url}"
            );
        }
        for url in [
            "http://192.168.1.20:11434/v1",
            "http://10.0.0.5",
            "http://100.64.0.1:1234",
            "http://[fe80::1]",
            "http://[fd00::1]:8080",
            "http://[::ffff:10.0.0.1]",
        ] {
            let secret = bound(Provider::Custom, url, "sk-test");
            assert_eq!(
                authorization(&secret, Provider::Custom, &endpoint(url)),
                Err(KeyError::InClear),
                "{url}"
            );
        }
        // donnée en https pour un serveur du réseau privé : pas en clair vers le même hôte et port
        let secret = bound(Provider::Custom, "https://192.168.1.20:8443/v1", "sk-test");
        assert_eq!(
            authorization(&secret, Provider::Custom, &endpoint("http://192.168.1.20:8443/v1")),
            Err(KeyError::InClear)
        );
    }

    #[test]
    fn a_key_account_names_the_provider_and_the_address() {
        assert_eq!(
            account(Provider::OpenRouter, "https://openrouter.ai/api/v1"),
            "openrouter@openrouter.ai:443"
        );
        assert_eq!(account(Provider::DeepSeek, "https://api.deepseek.com"), "deepseek@api.deepseek.com:443");
        assert_eq!(account(Provider::LmStudio, "http://localhost:1234/v1"), "lmstudio@localhost:1234");
        // la casse de l'hôte ne compte pas
        assert_eq!(
            account(Provider::OpenRouter, "HTTPS://OpenRouter.AI/api/v1"),
            "openrouter@openrouter.ai:443"
        );
        // le port par défaut du schéma, écrit ou non
        assert_eq!(account(Provider::Custom, "https://llm.example.com"), "custom@llm.example.com:443");
        assert_eq!(account(Provider::Custom, "https://llm.example.com:443/v1"), "custom@llm.example.com:443");
        assert_eq!(account(Provider::Custom, "http://localhost/v1"), "custom@localhost:80");
        // IPv6 : entre crochets, sous sa forme courte et en minuscules, quelle que soit l'écriture
        assert_eq!(account(Provider::LmStudio, "http://[::1]:1234/v1"), "lmstudio@[::1]:1234");
        assert_eq!(account(Provider::Custom, "https://[FD00:0:0:0:0:0:0:1]:8443"), "custom@[fd00::1]:8443");
        assert_eq!(account(Provider::Custom, "https://[fd00::1]:8443/v1"), "custom@[fd00::1]:8443");
        // le chemin ne compte pas, ni le schéma : c'est le même serveur
        assert_eq!(
            account(Provider::Custom, "https://h.example/v1"),
            account(Provider::Custom, "https://h.example/api")
        );
        assert_eq!(
            account(Provider::Custom, "http://localhost:8443"),
            account(Provider::Custom, "https://localhost:8443")
        );

        let account = KeyAccount::new(Provider::Custom, &endpoint("http://[::1]:1234"));
        assert_eq!((account.provider(), account.address()), (Provider::Custom, "[::1]:1234"));
    }

    #[test]
    fn another_port_another_host_or_another_provider_is_another_account() {
        let different = [
            (
                account(Provider::Custom, "http://localhost:1234"),
                account(Provider::Custom, "http://localhost:1235"),
            ),
            (
                account(Provider::Custom, "https://h.example"),
                account(Provider::Custom, "https://h.example:8443"),
            ),
            (
                account(Provider::Custom, "http://localhost:1234"),
                account(Provider::Custom, "http://127.0.0.1:1234"),
            ),
            (
                account(Provider::Custom, "http://127.0.0.1:1234"),
                account(Provider::Custom, "http://[::1]:1234"),
            ),
            (account(Provider::Custom, "https://a.example"), account(Provider::Custom, "https://b.example")),
            (
                account(Provider::Custom, "https://api.example"),
                account(Provider::Custom, "https://api.example.com"),
            ),
            (
                account(Provider::Custom, "http://localhost:1234"),
                account(Provider::LmStudio, "http://localhost:1234"),
            ),
        ];
        for (one, other) in different {
            assert_ne!(one, other);
        }
        // une clé d'une version précédente, sans adresse, ne porte jamais le nom d'un compte
        for provider in Provider::ALL {
            assert_eq!(legacy_account(provider), provider.id());
            assert!(!legacy_account(provider).contains('@'));
            assert!(account(provider, "https://h.example").contains('@'));
        }
    }

    #[test]
    fn a_key_never_goes_to_another_address_than_its_own() {
        let secret = bound(Provider::Custom, "http://127.0.0.1:1234/v1", "sk-test");
        assert_eq!(
            authorization(&secret, Provider::Custom, &endpoint("http://127.0.0.1:1234/autre")).as_deref(),
            Ok("Bearer sk-test"),
            "même hôte, même port"
        );
        for url in [
            "http://127.0.0.1:1235/v1",
            "http://localhost:1234/v1",
            "http://[::1]:1234/v1",
            "https://h.example",
        ] {
            assert_eq!(
                authorization(&secret, Provider::Custom, &endpoint(url)),
                Err(KeyError::OtherAddress),
                "{url}"
            );
        }
        // ni à un autre fournisseur, même à la même adresse
        assert_eq!(
            authorization(&secret, Provider::LmStudio, &endpoint("http://127.0.0.1:1234/v1")),
            Err(KeyError::OtherAddress)
        );
        // la clé de l'adresse habituelle d'OpenRouter ne suit pas un --url vers un autre serveur
        let openrouter = bound(Provider::OpenRouter, "https://openrouter.ai/api/v1", "sk-test");
        assert_eq!(
            authorization(&openrouter, Provider::OpenRouter, &endpoint("https://proxy.example/api/v1")),
            Err(KeyError::OtherAddress)
        );
        assert_eq!(format!("{openrouter:?}").matches("sk-test").count(), 0, "{openrouter:?}");
    }

    #[test]
    fn the_key_command_aims_at_the_given_then_the_chosen_then_the_usual_address() {
        let chosen = |provider, url: &str| Brain::new(provider, Some(url), "m").expect("valide");
        let at = |provider, given: Option<&str>, chosen: Option<&Brain>| {
            key_endpoint(provider, given.map(endpoint).as_ref(), chosen).map(|found| found.url(""))
        };
        let proxy = chosen(Provider::OpenRouter, "https://proxy.example/api/v1");
        let custom = chosen(Provider::Custom, "https://llm.example/v1");

        assert_eq!(at(Provider::OpenRouter, None, None).as_deref(), Some("https://openrouter.ai/api/v1"));
        assert_eq!(
            at(Provider::OpenRouter, None, Some(&proxy)).as_deref(),
            Some("https://proxy.example/api/v1")
        );
        assert_eq!(
            at(Provider::OpenRouter, None, Some(&custom)).as_deref(),
            Some("https://openrouter.ai/api/v1"),
            "le cerveau choisi est d'un autre fournisseur"
        );
        assert_eq!(
            at(Provider::OpenRouter, Some("https://autre.example/v1"), Some(&proxy)).as_deref(),
            Some("https://autre.example/v1")
        );
        assert_eq!(at(Provider::Custom, None, Some(&custom)).as_deref(), Some("https://llm.example/v1"));
        assert_eq!(at(Provider::Custom, None, Some(&proxy)), None, "un serveur personnalisé sans adresse");
        assert_eq!(at(Provider::Custom, None, None), None);

        // la commande à proposer n'a besoin de --url que s'il change l'adresse visée
        let command = |provider, url: &str, chosen| set_key_command(provider, &endpoint(url), chosen);
        assert_eq!(
            command(Provider::OpenRouter, "https://openrouter.ai/api/v1", None),
            "capri chat key openrouter"
        );
        assert_eq!(
            command(Provider::OpenRouter, "https://proxy.example/api/v1", Some(&proxy)),
            "capri chat key openrouter"
        );
        assert_eq!(
            command(Provider::OpenRouter, "https://openrouter.ai/api/v1", Some(&proxy)),
            "capri chat key openrouter --url https://openrouter.ai/api/v1",
            "sans --url, elle viserait le cerveau choisi"
        );
        assert_eq!(
            command(Provider::OpenRouter, "https://proxy.example/api/v1", None),
            "capri chat key openrouter --url https://proxy.example/api/v1"
        );
        assert_eq!(
            command(Provider::Custom, "https://llm.example/v1", Some(&custom)),
            "capri chat key custom"
        );
        assert_eq!(
            command(Provider::Custom, "http://localhost:8080/v1", None),
            "capri chat key custom --url http://localhost:8080/v1"
        );
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

    #[test]
    fn the_anthropic_key_travels_in_x_api_key_and_only_to_its_own_address() {
        const ANTHROPIC: &str = "https://api.anthropic.com/v1";
        let text = "sk-ant-api03-test-0123456789abcdef";
        let secret = bound(Provider::Anthropic, ANTHROPIC, text);
        assert_eq!(secret.account().name(), "anthropic@api.anthropic.com:443");
        let header = key_header(&secret, Provider::Anthropic, &endpoint(ANTHROPIC)).expect("son adresse");
        assert_eq!(
            (header.name(), header.reveal_for_request()),
            ("x-api-key", text),
            "la clé seule, sans Bearer"
        );
        // les autres fournisseurs gardent leur en-tête d'avant
        let openrouter = bound(Provider::OpenRouter, "https://openrouter.ai/api/v1", text);
        let header = key_header(&openrouter, Provider::OpenRouter, &endpoint("https://openrouter.ai/api/v1"))
            .expect("son adresse");
        assert_eq!(header.name(), "Authorization");
        assert_eq!(header.reveal_for_request(), format!("Bearer {text}"));

        // ni vers une autre adresse, ni à un autre fournisseur, ni en clair
        for url in
            ["https://proxy.example/v1", "https://api.anthropic.com:8443/v1", "https://anthropic.com/v1"]
        {
            assert_eq!(
                key_header(&secret, Provider::Anthropic, &endpoint(url)),
                Err(KeyError::OtherAddress),
                "{url}"
            );
        }
        for provider in Provider::ALL.into_iter().filter(|&provider| provider != Provider::Anthropic) {
            assert_eq!(
                key_header(&secret, provider, &endpoint(ANTHROPIC)),
                Err(KeyError::OtherAddress),
                "{}",
                provider.name()
            );
        }
        let lan = bound(Provider::Anthropic, "http://192.168.1.20:8080/v1", text);
        assert_eq!(
            key_header(&lan, Provider::Anthropic, &endpoint("http://192.168.1.20:8080/v1")),
            Err(KeyError::InClear)
        );

        // nulle part en clair : ni dans un Debug, ni dans un message d'erreur
        let header = key_header(&secret, Provider::Anthropic, &endpoint(ANTHROPIC)).expect("son adresse");
        for shown in [
            format!("{header:?}"),
            format!("{secret:?}"),
            format!("{:?}", key_header(&secret, Provider::Anthropic, &endpoint(ANTHROPIC))),
            KeyError::OtherAddress.to_string(),
            KeyError::InClear.to_string(),
        ] {
            assert!(!shown.contains(text) && !shown.contains("0123456789"), "{shown}");
        }
        assert_eq!(format!("{header:?}"), "KeyHeader(x-api-key: …)");
        // un refus qui citerait un bout de la clé : rien n'en reste
        assert_eq!(
            secret.redact_any_trace("invalid key: sk-ant-…cdef"),
            "invalid key: [clé masquée]…[clé masquée]"
        );
        // le prix de ce masquage : un mot qui ressemble à un morceau de la clé (« -api », comme dans
        // le début public des clés d'Anthropic) est masqué lui aussi
        assert_eq!(secret.redact_any_trace("invalid x-api-key"), "invalid x[clé masquée]-key");
    }
}
