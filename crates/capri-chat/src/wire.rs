//! Les deux formats que parlent les fournisseurs : celui d'OpenAI (« chat completions »), que
//! parlent presque tous, et celui de l'API d'Anthropic, qui n'y ressemble pas (autre en-tête de
//! clé, autre corps, autre flux).
//!
//! Ce module dit, pour chacun, où envoyer la conversation, quels en-têtes ajouter (jamais la clé :
//! c'est [`crate::key::key_header`]), quel corps envoyer et comment lire ce qui revient. Il ne se
//! connecte à rien : `capri-net` pose ce qu'on lui donne, et rend ce qu'il reçoit.

use crate::brain::Provider;
use crate::conversation::Turn;
use crate::{anthropic, openai};

/// Le format d'un fournisseur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    /// `POST …/chat/completions`, clé dans `Authorization: Bearer`.
    OpenAi,
    /// `POST …/messages`, clé dans `x-api-key`, version de l'API dans `anthropic-version`.
    Anthropic,
}

/// Ce qu'apporte un événement du flux.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Piece {
    /// Le texte à ajouter à la réponse ; vide si l'événement n'en porte pas.
    pub text: String,
    pub end: Option<End>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// La réponse est complète.
    Complete,
    /// Le fournisseur a abandonné en cours de route, et dit pourquoi.
    Failed(String),
    /// Le modèle a décliné la demande : ce qu'il avait commencé à écrire ne compte pas.
    Declined,
}

/// Une page de la liste des modèles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelPage {
    pub ids: Vec<String>,
    /// De quoi demander la page suivante, s'il y en a une ([`Wire::models_path`]).
    pub next: Option<String>,
}

impl Provider {
    /// Le format que parle ce fournisseur.
    pub fn wire(self) -> Wire {
        match self {
            Self::Anthropic => Wire::Anthropic,
            Self::Ollama
            | Self::LmStudio
            | Self::OpenRouter
            | Self::DeepSeek
            | Self::ProxyApi
            | Self::OllamaCloud
            | Self::Custom => Wire::OpenAi,
        }
    }
}

impl Wire {
    /// Où envoyer la conversation, sous l'adresse de base du cerveau.
    pub fn reply_path(self) -> &'static str {
        match self {
            Self::OpenAi => "/chat/completions",
            Self::Anthropic => anthropic::MESSAGES_PATH,
        }
    }

    /// Où demander la liste des modèles, sous l'adresse de base. `after` : la page suivante, telle
    /// que l'a annoncée [`Wire::model_page`]. `None` : il n'y a pas de page suivante à demander.
    pub fn models_path(self, after: Option<&str>) -> Option<String> {
        match (self, after) {
            (Self::OpenAi, None) => Some("/models".to_owned()),
            (Self::OpenAi, Some(_)) => None,
            (Self::Anthropic, after) => anthropic::models_path(after),
        }
    }

    /// Les en-têtes propres à ce format, en plus du type du contenu : jamais la clé, jamais rien
    /// qui présente l'app.
    pub fn headers(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::OpenAi => &[],
            Self::Anthropic => &[(anthropic::VERSION_HEADER, anthropic::API_VERSION)],
        }
    }

    /// Le corps de la requête : la personnalité de Capri, puis la conversation.
    pub fn request_body(self, model: &str, persona: &str, turns: &[Turn]) -> String {
        match self {
            Self::OpenAi => openai::request_body(model, persona, turns),
            Self::Anthropic => anthropic::request_body(model, persona, turns),
        }
    }

    /// Lit les données d'un événement du flux.
    pub fn interpret(self, data: &str) -> Piece {
        match self {
            Self::OpenAi => openai::interpret(data),
            Self::Anthropic => anthropic::interpret(data),
        }
    }

    /// Le message d'erreur que porte un corps de réponse, s'il en porte un.
    pub fn error_message(self, body: &str) -> Option<String> {
        match self {
            Self::OpenAi => openai::error_message(body),
            Self::Anthropic => anthropic::error_message(body),
        }
    }

    /// Les modèles que liste une page de la réponse, et la suite s'il y en a une.
    pub fn model_page(self, body: &str) -> Option<ModelPage> {
        match self {
            Self::OpenAi => openai::model_ids(body).map(|ids| ModelPage { ids, next: None }),
            Self::Anthropic => anthropic::model_page(body),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_anthropic_speaks_its_own_format() {
        for provider in Provider::ALL {
            let expected = if provider == Provider::Anthropic { Wire::Anthropic } else { Wire::OpenAi };
            assert_eq!(provider.wire(), expected, "{}", provider.name());
        }
        assert_eq!(Wire::OpenAi.reply_path(), "/chat/completions");
        assert_eq!(Wire::Anthropic.reply_path(), "/messages");
        assert!(Wire::OpenAi.headers().is_empty(), "rien de plus qu'avant pour les autres");
        assert_eq!(Wire::Anthropic.headers(), [("anthropic-version", "2026-10-21")]);
        assert_eq!(Wire::OpenAi.models_path(None).as_deref(), Some("/models"));
        assert_eq!(Wire::OpenAi.models_path(Some("x")), None, "pas de pages chez eux");
    }
}
