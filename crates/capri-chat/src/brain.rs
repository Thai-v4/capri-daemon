//! Quel cerveau Capri utilise : un fournisseur, une adresse, un modèle.
//!
//! Ce module décide de ce que contient le fichier de réglages et vérifie ce qu'il y lit. Il ne
//! touche ni au disque ni au réseau : `capri-app` lit et écrit le fichier, `capri-net` se connecte.
//! Aucun nom de modèle n'est écrit ici : c'est l'utilisateur, ou la liste que donne le serveur,
//! qui le fournit.

use serde_json::{Value, json};

use crate::endpoint::{Endpoint, EndpointError};

/// Longueur maximale d'un nom de modèle.
const MAX_MODEL_CHARS: usize = 200;

/// Les fournisseurs que Capri connaît.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// Ollama, sur cette machine ou sur le réseau privé : aucun compte, aucune clé.
    Ollama,
}

impl Provider {
    pub const ALL: [Self; 1] = [Self::Ollama];

    /// Le nom écrit dans le fichier de réglages et tapé en ligne de commande.
    pub fn id(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
        }
    }

    /// Le nom à afficher.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
        }
    }

    /// L'adresse de base quand l'utilisateur n'en donne pas.
    pub fn default_url(self) -> &'static str {
        match self {
            Self::Ollama => "http://localhost:11434/v1",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|provider| provider.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BrainError {
    #[error("ce n'est pas du JSON valide")]
    InvalidJson,
    #[error("il manque « {0} »")]
    Missing(&'static str),
    #[error("fournisseur inconnu : « {0} »")]
    UnknownProvider(String),
    #[error("adresse refusée : {0}")]
    Endpoint(#[from] EndpointError),
    #[error("le nom du modèle est vide, trop long, ou contient des espaces")]
    Model,
}

/// Un cerveau prêt à servir : tout ce qu'il contient a été vérifié.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brain {
    provider: Provider,
    endpoint: Endpoint,
    model: String,
}

impl Brain {
    /// `base_url` : `None` pour l'adresse habituelle du fournisseur.
    pub fn new(provider: Provider, base_url: Option<&str>, model: &str) -> Result<Self, BrainError> {
        let endpoint = Endpoint::parse(base_url.unwrap_or(provider.default_url()).trim())?;
        let model = model.trim();
        let model_is_plain = !model.is_empty()
            && model.chars().count() <= MAX_MODEL_CHARS
            && !model.chars().any(|c| c.is_whitespace() || c.is_control());
        if !model_is_plain {
            return Err(BrainError::Model);
        }
        Ok(Self { provider, endpoint, model: model.to_owned() })
    }

    pub fn provider(&self) -> Provider {
        self.provider
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Le même cerveau avec un autre modèle.
    pub fn with_model(&self, model: &str) -> Result<Self, BrainError> {
        Self::new(self.provider, Some(&self.endpoint.url("")), model)
    }

    /// Lit le fichier de réglages. Les clés inconnues sont ignorées : un fichier écrit par une
    /// version plus récente reste lisible tant que l'essentiel y est.
    pub fn from_json(text: &str) -> Result<Self, BrainError> {
        let settings: Value = serde_json::from_str(text).map_err(|_| BrainError::InvalidJson)?;
        let field =
            |name: &'static str| settings.get(name).and_then(Value::as_str).ok_or(BrainError::Missing(name));
        let provider = field("provider")?;
        let provider =
            Provider::from_id(provider).ok_or_else(|| BrainError::UnknownProvider(provider.to_owned()))?;
        Self::new(provider, Some(field("base_url")?), field("model")?)
    }

    /// Le contenu du fichier de réglages.
    pub fn to_json(&self) -> String {
        let settings = json!({
            "provider": self.provider.id(),
            "base_url": self.endpoint.url(""),
            "model": self.model,
        });
        let mut text =
            serde_json::to_string_pretty(&settings).expect("un objet JSON simple s'écrit toujours");
        text.push('\n');
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_brain_survives_being_written_and_read_back() {
        let brain = Brain::new(Provider::Ollama, None, "  un-modele:8b ").expect("valide");
        assert_eq!(brain.model(), "un-modele:8b");
        assert_eq!(brain.endpoint().url("/chat/completions"), "http://localhost:11434/v1/chat/completions");
        let text = brain.to_json();
        assert!(text.ends_with("}\n"));
        assert_eq!(Brain::from_json(&text), Ok(brain.clone()));

        let elsewhere =
            Brain::new(Provider::Ollama, Some("http://192.168.1.20:11434/v1/"), "m").expect("valide");
        assert_eq!(Brain::from_json(&elsewhere.to_json()), Ok(elsewhere.clone()));
        assert_eq!(elsewhere.endpoint().display_host(), "192.168.1.20:11434");
        let other = elsewhere.with_model("autre:4b").expect("valide");
        assert_eq!((other.model(), other.endpoint()), ("autre:4b", elsewhere.endpoint()));
    }

    #[test]
    fn a_hand_written_file_is_checked() {
        let read = |text: &str| Brain::from_json(text);
        assert!(
            read(r#"{"provider":"ollama","base_url":"http://localhost:11434/v1","model":"m","futur":true}"#)
                .is_ok(),
            "une clé inconnue ne gêne pas"
        );
        assert_eq!(read("pas du json"), Err(BrainError::InvalidJson));
        assert_eq!(read("[]"), Err(BrainError::Missing("provider")));
        assert_eq!(read(r#"{"provider":"ollama","model":"m"}"#), Err(BrainError::Missing("base_url")));
        assert_eq!(
            read(r#"{"provider":"ollama","base_url":"http://localhost:11434/v1","model":7}"#),
            Err(BrainError::Missing("model"))
        );
        assert_eq!(
            read(r#"{"provider":"skynet","base_url":"http://localhost/v1","model":"m"}"#),
            Err(BrainError::UnknownProvider("skynet".to_owned()))
        );
        // l'adresse d'un fichier modifié à la main passe par les mêmes règles
        assert!(matches!(
            read(r#"{"provider":"ollama","base_url":"http://example.com/v1","model":"m"}"#),
            Err(BrainError::Endpoint(EndpointError::PlainHttpToPublicHost(_)))
        ));
    }

    #[test]
    fn a_model_name_is_one_plain_word() {
        for model in ["", "   ", "deux mots", "avec\ttabulation", "ligne\nsuivante", &"x".repeat(201)] {
            assert_eq!(Brain::new(Provider::Ollama, None, model), Err(BrainError::Model), "{model:?}");
        }
        for model in
            ["ministral-3:8b", "hf.co/quelquun/depot:Q4_K_M", "fournisseur/modèle:free", &"x".repeat(200)]
        {
            assert!(Brain::new(Provider::Ollama, None, model).is_ok(), "{model:?}");
        }
    }

    #[test]
    fn every_provider_has_a_usable_default_address() {
        for provider in Provider::ALL {
            assert_eq!(Provider::from_id(provider.id()), Some(provider));
            assert!(Endpoint::parse(provider.default_url()).is_ok(), "{}", provider.name());
        }
        assert_eq!(Provider::from_id("Ollama"), None, "l'identifiant s'écrit en minuscules");
    }
}
