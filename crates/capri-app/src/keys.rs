//! Les clés d'API côté application : où elles sont rangées, et comment on les donne à Capri.
//!
//! Une clé vit dans le trousseau du système (`capri_platform::keychain`), sous le service `Capri`,
//! avec un compte par fournisseur et par adresse (`openrouter@openrouter.ai:443` : voir
//! `capri_chat::KeyAccount`). Elle est liée à l'adresse pour laquelle elle a été donnée : elle n'est
//! relue que pour cette adresse, et `capri-net` refuse de l'envoyer ailleurs. Elle n'est écrite
//! nulle part ailleurs : ni dans `chat.json`, ni dans un autre fichier, ni dans le journal, ni dans
//! un message. Elle est relue dans le trousseau à chaque message, comme le reste des réglages.
//!
//! Une version précédente rangeait la clé sous l'identifiant du fournisseur seul
//! (`capri_chat::key::legacy_account`). Une telle clé n'est jamais lue pour être envoyée : on sait
//! seulement si elle est là, pour le dire, et on peut la retirer.
//!
//! `CAPRI_KEYCHAIN_SERVICE` remplace le nom du service : pour des essais qui ne touchent pas aux
//! vraies clés.

use std::io::{IsTerminal, Read, Write};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use capri_chat::{ApiKey, BoundKey, Endpoint, KeyAccount, KeyNeed, Provider};
use capri_platform::keychain;

/// Le service sous lequel les clés sont rangées dans le trousseau.
pub const SERVICE: &str = "Capri";
/// Ce qu'on lit au plus sur l'entrée standard : une clé tient en bien moins.
const MAX_INPUT_BYTES: u64 = 16 * 1024;

/// Là où les clés sont rangées.
pub trait KeyStore: Send + Sync {
    /// Le système a-t-il un trousseau ?
    fn available(&self) -> bool;
    /// Son nom, pour les messages : « le trousseau de session ».
    fn name(&self) -> &str;
    /// Range `key` sous ce compte, à la place de ce qui y était.
    fn save(&self, account: &KeyAccount, key: &ApiKey) -> Result<()>;
    /// La clé rangée sous ce compte, liée à son adresse. Seul un compte qui nomme une adresse se
    /// lit : une clé rangée sans adresse ne peut pas l'être.
    fn load(&self, account: &KeyAccount) -> Result<Option<BoundKey>>;
    /// Un secret est-il rangé sous ce nom de compte ? Sans le lire : sur macOS, le système ne
    /// demande rien.
    fn contains(&self, account: &str) -> Result<bool>;
    /// Retire le secret rangé sous ce nom de compte. `false` : il n'y en avait pas.
    fn forget(&self, account: &str) -> Result<bool>;
}

/// Le trousseau du système.
pub struct SystemKeychain {
    service: String,
}

impl SystemKeychain {
    /// Le service `Capri`, ou celui que nomme `CAPRI_KEYCHAIN_SERVICE`.
    pub fn from_env() -> Self {
        let service = std::env::var("CAPRI_KEYCHAIN_SERVICE")
            .ok()
            .filter(|service| !service.trim().is_empty())
            .unwrap_or_else(|| SERVICE.to_owned());
        Self { service }
    }

    pub fn shared() -> Arc<dyn KeyStore> {
        Arc::new(Self::from_env())
    }
}

impl KeyStore for SystemKeychain {
    fn available(&self) -> bool {
        keychain::AVAILABLE
    }

    fn name(&self) -> &str {
        keychain::NAME
    }

    fn save(&self, account: &KeyAccount, key: &ApiKey) -> Result<()> {
        keychain::save(&self.service, &account.name(), key.reveal_for_keychain())
            .with_context(|| format!("rangement de la clé dans {}", self.name()))
    }

    fn load(&self, account: &KeyAccount) -> Result<Option<BoundKey>> {
        let Some(text) = keychain::load(&self.service, &account.name())
            .with_context(|| format!("lecture de la clé dans {}", self.name()))?
        else {
            return Ok(None);
        };
        let key = ApiKey::parse(&text).map_err(|err| {
            anyhow!(
                "la clé rangée pour {} n'est pas valable ({err}) : il faut la redonner",
                account.address()
            )
        })?;
        Ok(Some(BoundKey::new(account.clone(), key)))
    }

    fn contains(&self, account: &str) -> Result<bool> {
        keychain::contains(&self.service, account).with_context(|| format!("consultation de {}", self.name()))
    }

    fn forget(&self, account: &str) -> Result<bool> {
        keychain::delete(&self.service, account)
            .with_context(|| format!("retrait de la clé de {}", self.name()))
    }
}

/// La clé à envoyer à `provider` joint à `endpoint` : aucune s'il n'en prend pas (le trousseau
/// n'est alors même pas consulté) ou si le système n'a pas de trousseau, sinon celle rangée pour
/// cette adresse, s'il y en a une. Jamais celle d'une autre adresse.
pub fn key_for(keys: &dyn KeyStore, provider: Provider, endpoint: &Endpoint) -> Result<Option<BoundKey>> {
    match provider.key_need() {
        KeyNeed::None => Ok(None),
        _ if !keys.available() => Ok(None),
        KeyNeed::Optional | KeyNeed::Required => keys.load(&KeyAccount::new(provider, endpoint)),
    }
}

/// Une clé rangée sans adresse par une version précédente est-elle encore là ? Elle n'est jamais
/// lue : seulement signalée.
pub fn has_legacy_key(keys: &dyn KeyStore, provider: Provider) -> bool {
    provider.key_need() != KeyNeed::None
        && keys.available()
        && keys.contains(capri_chat::key::legacy_account(provider)).unwrap_or(false)
}

/// Lit la clé que donne l'utilisateur : tapée sans s'afficher si l'entrée est un terminal, sinon
/// lue sur l'entrée standard (`pbpaste | capri chat key openrouter`). `prompt` s'affiche sur la
/// sortie d'erreur, seulement au terminal.
pub fn read_from_user(prompt: &str) -> Result<String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprint!("{prompt}");
        let _ = std::io::stderr().flush();
        return capri_platform::terminal::read_hidden_line().context("lecture de la clé au clavier");
    }
    let mut bytes = Vec::new();
    stdin.lock().take(MAX_INPUT_BYTES + 1).read_to_end(&mut bytes).context("lecture de l'entrée standard")?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        bail!("l'entrée standard est bien trop longue pour une clé : rien n'est changé");
    }
    String::from_utf8(bytes).map_err(|_| anyhow!("l'entrée standard n'est pas du texte : rien n'est changé"))
}

#[cfg(test)]
pub mod memory {
    //! Un trousseau en mémoire, pour les tests : aucun ne touche au vrai.

    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    pub struct MemoryKeys {
        /// Les secrets, par nom de compte.
        keys: Mutex<HashMap<String, ApiKey>>,
        /// Pour simuler un système sans trousseau.
        unavailable: bool,
        /// Pour simuler un accès refusé par le système.
        denied: bool,
    }

    impl MemoryKeys {
        /// Un système sans trousseau.
        pub fn unavailable() -> Self {
            Self { unavailable: true, ..Self::default() }
        }

        /// Un trousseau dont le système refuse l'accès.
        pub fn denied() -> Self {
            Self { denied: true, ..Self::default() }
        }

        /// Un trousseau où `key` est rangée pour l'adresse `url` de ce fournisseur.
        pub fn with(provider: Provider, url: &str, key: &str) -> Self {
            let keys = Self::default();
            keys.add(provider, url, key);
            keys
        }

        /// Range aussi `key` pour l'adresse `url` de ce fournisseur.
        pub fn add(&self, provider: Provider, url: &str, key: &str) {
            let account = KeyAccount::new(provider, &Endpoint::parse(url).expect("adresse valable"));
            self.save(&account, &ApiKey::parse(key).expect("une clé")).expect("rangée");
        }

        /// Range aussi `key` comme le faisait une version précédente : sous l'identifiant seul.
        pub fn add_legacy(&self, provider: Provider, key: &str) {
            let account = capri_chat::key::legacy_account(provider).to_owned();
            self.lock().insert(account, ApiKey::parse(key).expect("une clé"));
        }

        /// Les noms des comptes rangés, dans l'ordre alphabétique.
        pub fn accounts(&self) -> Vec<String> {
            let mut accounts: Vec<String> = self.lock().keys().cloned().collect();
            accounts.sort();
            accounts
        }

        /// Le texte de la clé rangée sous ce nom de compte.
        pub fn secret(&self, account: &str) -> Option<String> {
            self.lock().get(account).map(|key| key.reveal_for_keychain().to_owned())
        }

        fn check(&self) -> Result<()> {
            if self.unavailable {
                bail!("pas de trousseau sur ce système");
            }
            if self.denied {
                bail!("le système refuse l'accès au trousseau");
            }
            Ok(())
        }

        fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, ApiKey>> {
            self.keys.lock().expect("verrou")
        }
    }

    impl KeyStore for MemoryKeys {
        fn available(&self) -> bool {
            !self.unavailable
        }

        fn name(&self) -> &str {
            "le trousseau de test"
        }

        fn save(&self, account: &KeyAccount, key: &ApiKey) -> Result<()> {
            self.check()?;
            self.lock().insert(account.name(), key.clone());
            Ok(())
        }

        fn load(&self, account: &KeyAccount) -> Result<Option<BoundKey>> {
            self.check()?;
            Ok(self.lock().get(&account.name()).map(|key| BoundKey::new(account.clone(), key.clone())))
        }

        fn contains(&self, account: &str) -> Result<bool> {
            self.check()?;
            Ok(self.lock().contains_key(account))
        }

        fn forget(&self, account: &str) -> Result<bool> {
            self.check()?;
            Ok(self.lock().remove(account).is_some())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::memory::MemoryKeys;
    use super::*;

    fn at(url: &str) -> Endpoint {
        Endpoint::parse(url).expect("adresse valable")
    }

    #[test]
    fn a_provider_that_takes_no_key_never_opens_the_keychain() {
        // un trousseau qui refuserait tout accès : Ollama n'y touche même pas
        let keys = MemoryKeys::denied();
        assert!(
            key_for(&keys, Provider::Ollama, &at("http://localhost:11434/v1"))
                .expect("rien à lire")
                .is_none()
        );
        assert!(!has_legacy_key(&keys, Provider::Ollama));
        let openrouter = at("https://openrouter.ai/api/v1");
        assert!(key_for(&keys, Provider::OpenRouter, &openrouter).is_err(), "les autres le consultent");

        // sans trousseau, pas de clé, et pas d'erreur : un serveur à clé facultative reste utilisable
        let lmstudio = at("http://localhost:1234/v1");
        assert!(
            key_for(&MemoryKeys::unavailable(), Provider::LmStudio, &lmstudio)
                .expect("rien à lire")
                .is_none()
        );

        let deepseek = at("https://api.deepseek.com");
        let keys = MemoryKeys::with(Provider::DeepSeek, "https://api.deepseek.com", "sk-test-0123456789");
        let key = key_for(&keys, Provider::DeepSeek, &deepseek).expect("lisible").expect("rangée");
        assert_eq!(key.account().name(), "deepseek@api.deepseek.com:443", "liée à son adresse");
        assert!(
            key_for(&keys, Provider::OpenRouter, &openrouter).expect("lisible").is_none(),
            "une clé par fournisseur"
        );
    }

    #[test]
    fn a_key_is_read_only_for_the_address_it_was_given_for() {
        let keys = MemoryKeys::with(Provider::Custom, "http://127.0.0.1:1234/v1", "sk-test-0123456789");
        assert!(
            key_for(&keys, Provider::Custom, &at("http://127.0.0.1:1234/autre")).expect("lisible").is_some()
        );
        for url in ["http://127.0.0.1:1235/v1", "http://localhost:1234/v1", "https://llm.example/v1"] {
            assert!(key_for(&keys, Provider::Custom, &at(url)).expect("lisible").is_none(), "{url}");
        }
        assert!(
            key_for(&keys, Provider::LmStudio, &at("http://127.0.0.1:1234/v1")).expect("lisible").is_none()
        );
    }

    #[test]
    fn a_key_kept_without_an_address_is_noticed_but_never_read() {
        let keys = MemoryKeys::default();
        keys.add_legacy(Provider::OpenRouter, "sk-test-0123456789");
        assert!(has_legacy_key(&keys, Provider::OpenRouter));
        assert!(!has_legacy_key(&keys, Provider::DeepSeek));
        assert!(
            key_for(&keys, Provider::OpenRouter, &at("https://openrouter.ai/api/v1"))
                .expect("lisible")
                .is_none()
        );
        // sans trousseau, rien à signaler
        assert!(!has_legacy_key(&MemoryKeys::unavailable(), Provider::OpenRouter));
    }
}
