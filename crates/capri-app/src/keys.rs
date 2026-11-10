//! Les clés d'API côté application : où elles sont rangées, et comment on les donne à Capri.
//!
//! Une clé vit dans le trousseau du système (`capri_platform::keychain`), sous le service `Capri`,
//! avec un compte par fournisseur (`openrouter`, `deepseek`…). Elle n'est écrite nulle part
//! ailleurs : ni dans `chat.json`, ni dans un autre fichier, ni dans le journal, ni dans un
//! message. Elle est relue dans le trousseau à chaque message, comme le reste des réglages.
//!
//! `CAPRI_KEYCHAIN_SERVICE` remplace le nom du service : pour des essais qui ne touchent pas aux
//! vraies clés.

use std::io::{IsTerminal, Read, Write};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use capri_chat::{ApiKey, KeyNeed, Provider};
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
    fn save(&self, provider: Provider, key: &ApiKey) -> Result<()>;
    fn load(&self, provider: Provider) -> Result<Option<ApiKey>>;
    /// Une clé est-elle rangée ? Sans la lire : sur macOS, le système ne demande rien.
    fn contains(&self, provider: Provider) -> Result<bool>;
    /// Retire la clé. `false` : il n'y en avait pas.
    fn forget(&self, provider: Provider) -> Result<bool>;
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

    fn save(&self, provider: Provider, key: &ApiKey) -> Result<()> {
        keychain::save(&self.service, provider.id(), key.reveal_for_keychain())
            .with_context(|| format!("rangement de la clé dans {}", self.name()))
    }

    fn load(&self, provider: Provider) -> Result<Option<ApiKey>> {
        let Some(text) = keychain::load(&self.service, provider.id())
            .with_context(|| format!("lecture de la clé dans {}", self.name()))?
        else {
            return Ok(None);
        };
        ApiKey::parse(&text).map(Some).map_err(|err| {
            anyhow!(
                "la clé rangée n'est pas valable ({err}) : pour la remettre, capri chat key {}",
                provider.id()
            )
        })
    }

    fn contains(&self, provider: Provider) -> Result<bool> {
        keychain::contains(&self.service, provider.id())
            .with_context(|| format!("consultation de {}", self.name()))
    }

    fn forget(&self, provider: Provider) -> Result<bool> {
        keychain::delete(&self.service, provider.id())
            .with_context(|| format!("retrait de la clé de {}", self.name()))
    }
}

/// La clé à envoyer à `provider` : aucune s'il n'en prend pas (le trousseau n'est alors même pas
/// consulté) ou si le système n'a pas de trousseau, sinon celle du trousseau s'il y en a une.
pub fn key_for(keys: &dyn KeyStore, provider: Provider) -> Result<Option<ApiKey>> {
    match provider.key_need() {
        KeyNeed::None => Ok(None),
        _ if !keys.available() => Ok(None),
        KeyNeed::Optional | KeyNeed::Required => keys.load(provider),
    }
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
        keys: Mutex<HashMap<&'static str, ApiKey>>,
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

        pub fn with(provider: Provider, key: &str) -> Self {
            let keys = Self::default();
            keys.save(provider, &ApiKey::parse(key).expect("une clé")).expect("rangée");
            keys
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

        fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<&'static str, ApiKey>> {
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

        fn save(&self, provider: Provider, key: &ApiKey) -> Result<()> {
            self.check()?;
            self.lock().insert(provider.id(), key.clone());
            Ok(())
        }

        fn load(&self, provider: Provider) -> Result<Option<ApiKey>> {
            self.check()?;
            Ok(self.lock().get(provider.id()).cloned())
        }

        fn contains(&self, provider: Provider) -> Result<bool> {
            self.check()?;
            Ok(self.lock().contains_key(provider.id()))
        }

        fn forget(&self, provider: Provider) -> Result<bool> {
            self.check()?;
            Ok(self.lock().remove(provider.id()).is_some())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::memory::MemoryKeys;
    use super::*;

    #[test]
    fn a_provider_that_takes_no_key_never_opens_the_keychain() {
        // un trousseau qui refuserait tout accès : Ollama n'y touche même pas
        let keys = MemoryKeys::denied();
        assert!(key_for(&keys, Provider::Ollama).expect("rien à lire").is_none());
        assert!(key_for(&keys, Provider::OpenRouter).is_err(), "les autres le consultent");

        // sans trousseau, pas de clé, et pas d'erreur : un serveur à clé facultative reste utilisable
        assert!(key_for(&MemoryKeys::unavailable(), Provider::LmStudio).expect("rien à lire").is_none());

        let keys = MemoryKeys::with(Provider::DeepSeek, "sk-test-0123456789");
        assert!(key_for(&keys, Provider::DeepSeek).expect("lisible").is_some());
        assert!(key_for(&keys, Provider::OpenRouter).expect("lisible").is_none(), "une clé par fournisseur");
    }
}
