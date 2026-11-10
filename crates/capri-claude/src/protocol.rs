//! Les messages que `capri-hook` envoie à Capri : une ligne JSON par événement.
//!
//! ```json
//! {"v":1,"session":"abc123","cwd":"/Users/moi/projet","at_ms":1731250000000,"kind":"tool_start","tool":"Bash","id":"toolu_01","summary":"Run test suite"}
//! ```
//!
//! Le message ne transporte que ce que le bandeau affiche : jamais le texte d'un prompt, le
//! contenu d'un fichier ou la sortie d'une commande.

use serde::{Deserialize, Serialize};

/// Version du format. Capri ignore les messages d'une autre version.
pub const VERSION: u32 = 1;
/// Taille maximale d'une ligne, saut de ligne compris.
pub const MAX_LINE_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub v: u32,
    /// Identifiant de la session Claude Code.
    pub session: String,
    /// Dossier de travail de la session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Heure de l'événement vue par le hook, en millisecondes depuis 1970.
    pub at_ms: u64,
    /// Type du sous-agent dans lequel l'événement s'est produit, s'il y en a un.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(flatten)]
    pub event: Event,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// La session démarre ou reprend (`SessionStart`).
    SessionStart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
    /// Un prompt vient d'être envoyé (`UserPromptSubmit`). Son texte n'est pas transmis.
    TurnStart,
    /// Un outil va s'exécuter (`PreToolUse`).
    ToolStart {
        tool: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// Un outil a réussi (`PostToolUse`).
    ToolEnd {
        tool: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// Un outil a échoué (`PostToolUseFailure`).
    ToolFailed {
        tool: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Première ligne de l'erreur.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        interrupted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// Claude Code va demander une permission (`PermissionRequest`).
    NeedsPermission {
        tool: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// Claude Code envoie une notification (`Notification`).
    Notification {
        notification_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// Un sous-agent démarre (`SubagentStart`).
    AgentStart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
    },
    /// Un sous-agent a fini (`SubagentStop`).
    AgentEnd {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
    },
    /// Claude a fini de répondre (`Stop`).
    TurnEnd,
    /// Le tour s'est arrêté sur une erreur d'API (`StopFailure`).
    TurnFailed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// La session se termine (`SessionEnd`).
    SessionEnd {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("ligne de {0} octets, au-delà des {MAX_LINE_BYTES} autorisés")]
    TooLong(usize),
    #[error("version {0} du protocole, seule la {VERSION} est comprise")]
    Version(u32),
    #[error("message illisible : {0}")]
    Json(#[from] serde_json::Error),
}

impl Message {
    /// La ligne à envoyer, saut de ligne final compris.
    pub fn encode(&self) -> Result<String, ProtocolError> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        if line.len() > MAX_LINE_BYTES {
            return Err(ProtocolError::TooLong(line.len()));
        }
        Ok(line)
    }

    /// Relit une ligne reçue, avec ou sans son saut de ligne.
    pub fn decode(line: &str) -> Result<Self, ProtocolError> {
        if line.len() > MAX_LINE_BYTES {
            return Err(ProtocolError::TooLong(line.len()));
        }
        let message: Self = serde_json::from_str(line.trim_end_matches(['\r', '\n']))?;
        if message.v != VERSION {
            return Err(ProtocolError::Version(message.v));
        }
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(event: Event) -> Message {
        Message {
            v: VERSION,
            session: "abc123".into(),
            cwd: Some("/Users/moi/projet".into()),
            at_ms: 1_731_250_000_000,
            agent: None,
            event,
        }
    }

    #[test]
    fn a_line_looks_like_the_documented_example() {
        let line = message(Event::ToolStart {
            tool: "Bash".into(),
            id: Some("toolu_01".into()),
            summary: Some("Run test suite".into()),
        })
        .encode()
        .unwrap();
        assert_eq!(
            line,
            "{\"v\":1,\"session\":\"abc123\",\"cwd\":\"/Users/moi/projet\",\"at_ms\":1731250000000,\
             \"kind\":\"tool_start\",\"tool\":\"Bash\",\"id\":\"toolu_01\",\"summary\":\"Run test suite\"}\n"
        );
    }

    #[test]
    fn every_event_survives_a_round_trip() {
        let events = [
            Event::SessionStart { source: Some("startup".into()), model: None },
            Event::TurnStart,
            Event::ToolStart { tool: "Read".into(), id: None, summary: Some("main.rs".into()) },
            Event::ToolEnd { tool: "Read".into(), id: Some("t1".into()), duration_ms: Some(12) },
            Event::ToolFailed {
                tool: "Bash".into(),
                id: Some("t2".into()),
                error: Some("Exit code 1".into()),
                interrupted: true,
                duration_ms: Some(4187),
            },
            Event::NeedsPermission { tool: "Bash".into(), summary: None },
            Event::Notification { notification_type: "idle_prompt".into(), message: None },
            Event::AgentStart { agent_type: Some("Explore".into()), agent_id: Some("a1".into()) },
            Event::AgentEnd { agent_type: None, agent_id: None },
            Event::TurnEnd,
            Event::TurnFailed { error: Some("rate_limit".into()) },
            Event::SessionEnd { reason: Some("other".into()) },
        ];
        for event in events {
            let sent = message(event);
            let line = sent.encode().unwrap();
            assert!(line.ends_with('\n') && !line.trim_end().contains('\n'), "une seule ligne : {line}");
            assert_eq!(Message::decode(&line).unwrap(), sent);
        }
    }

    #[test]
    fn unknown_fields_are_ignored_but_unknown_kinds_and_versions_are_refused() {
        let extra = r#"{"v":1,"session":"s","at_ms":1,"kind":"turn_end","futur":{"x":[1,2]}}"#;
        assert_eq!(Message::decode(extra).unwrap().event, Event::TurnEnd);

        let unknown_kind = r#"{"v":1,"session":"s","at_ms":1,"kind":"teleport"}"#;
        assert!(matches!(Message::decode(unknown_kind), Err(ProtocolError::Json(_))));

        let future = r#"{"v":2,"session":"s","at_ms":1,"kind":"turn_end"}"#;
        assert!(matches!(Message::decode(future), Err(ProtocolError::Version(2))));

        assert!(matches!(Message::decode("pas du json"), Err(ProtocolError::Json(_))));
    }

    #[test]
    fn oversized_lines_are_refused_both_ways() {
        let huge = message(Event::ToolStart { tool: "x".repeat(MAX_LINE_BYTES), id: None, summary: None });
        assert!(matches!(huge.encode(), Err(ProtocolError::TooLong(_))));
        let line = format!("{{\"v\":1,\"pad\":\"{}\"}}", "x".repeat(MAX_LINE_BYTES));
        assert!(matches!(Message::decode(&line), Err(ProtocolError::TooLong(_))));
    }
}
