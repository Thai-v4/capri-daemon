//! Les messages que `capri-hook` envoie à Capri : une ligne JSON par événement.
//!
//! ```json
//! {"v":1,"session":"abc123","cwd":"/Users/moi/projet","at_ms":1731250000000,"kind":"tool_start","tool":"Bash","id":"toolu_01","summary":"Run test suite"}
//! ```
//!
//! Le message ne transporte que ce que le bandeau affiche : jamais le texte d'un prompt, le
//! contenu d'un fichier ou la sortie d'une commande. Une seule exception, si l'utilisateur a
//! activé le miroir (`crate::mirror`) : la fin de tour peut porter la ligne que Claude a écrite
//! pour Capri, et rien d'autre de sa réponse.
//!
//! ```json
//! {"v":1,"session":"abc123","at_ms":1731250000000,"kind":"turn_end","said":"C'est réparé."}
//! ```
//!
//! Le format n'évolue que par des champs facultatifs, que l'autre côté ignore s'il ne les connaît
//! pas : un hook récent parle à une Capri ancienne, et l'inverse, sans changer de version.
//!
//! Le journal de l'app n'en écrit jamais que [`Message::for_log`] : la sorte d'événement, des noms
//! et des catégories, des durées, des longueurs et des étiquettes, jamais un texte libre.

use std::fmt;

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};

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
    /// Claude a fini de répondre (`Stop`). `said` : la ligne qu'il a écrite pour Capri, seulement
    /// si le miroir est activé et qu'il y en a une (absente des messages d'avant le miroir).
    TurnEnd {
        #[serde(default, deserialize_with = "lenient_said", skip_serializing_if = "Option::is_none")]
        said: Option<Said>,
    },
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

/// La ligne que Claude a écrite pour Capri (le miroir), déjà débarrassée de son marqueur.
///
/// Comme un secret : son `Debug` ne montre que sa longueur, pour qu'elle ne finisse jamais dans un
/// journal ; son texte ne sort que par [`Said::text`], pour être dit.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Said(String);

impl Said {
    pub fn new(text: String) -> Self {
        Self(text)
    }

    pub fn text(&self) -> &str {
        &self.0
    }

    pub fn into_text(self) -> String {
        self.0
    }
}

impl fmt::Debug for Said {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Said({} caractères)", self.0.chars().count())
    }
}

/// Une phrase qui n'est pas du texte (une version future qui en changerait la forme) ne compte
/// pas, et n'empêche pas de lire le reste du message.
fn lenient_said<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Said>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Maybe {
        Text(String),
        Other(IgnoredAny),
    }
    Ok(match Maybe::deserialize(deserializer)? {
        Maybe::Text(text) => Some(Said(text)),
        Maybe::Other(_) => None,
    })
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

    /// Comme [`Message::encode`], mais une ligne trop longue part sans la phrase pour Capri plutôt
    /// que pas du tout : la fin du tour compte plus que ce qu'elle aurait dit.
    pub fn encode_or_shed(&mut self) -> Result<String, ProtocolError> {
        match self.encode() {
            Err(ProtocolError::TooLong(_)) if matches!(self.event, Event::TurnEnd { said: Some(_) }) => {
                self.event = Event::TurnEnd { said: None };
                self.encode()
            }
            done => done,
        }
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

    /// Ce que le journal en garde, à tout niveau : de quoi comprendre ce qui s'est passé, et
    /// jamais un texte libre. Champ par champ :
    ///
    /// - la sorte d'événement, toujours ;
    /// - le nom de l'outil et le type de sous-agent (celui de l'événement, ou celui où il s'est
    ///   produit), tels quels s'ils ont la forme d'un nom (lettres et chiffres ASCII, `_`, `-`,
    ///   `.`, `:`) : Claude Code les tire de sa configuration, pas de ce qu'écrit Claude ;
    ///   autrement, leur longueur ;
    /// - les catégories que donne Claude Code (la source d'un début de session, la raison d'une
    ///   fin, la sorte de notification, celle d'un tour arrêté sur une erreur d'API : `rate_limit`),
    ///   telles quelles si elles en ont la forme (minuscules, chiffres et `_`) ; autrement, leur
    ///   longueur ;
    /// - les durées, et si l'outil a été interrompu ;
    /// - le résumé d'un outil, le message d'une notification, la première ligne d'une erreur
    ///   d'outil, la ligne du miroir : leur longueur seulement. C'est là qu'entrent ce qu'écrit
    ///   Claude et ce qui vient d'une commande, d'un fichier ou d'un prompt ;
    /// - l'identifiant de session, celui d'un appel d'outil, celui d'un sous-agent : une étiquette
    ///   (`#` et huit chiffres hexadécimaux), la même pour le même identifiant, qui distingue deux
    ///   sessions et relie le début d'un outil à sa fin sans désigner la conversation ;
    /// - ni le dossier de travail (un chemin de l'utilisateur, qui nomme ses projets), ni le
    ///   modèle, ni l'heure vue par le hook (la ligne du journal a la sienne), ni la version du
    ///   format (une seule est acceptée) : rien.
    pub fn for_log(&self) -> String {
        let session = match self.session.as_str() {
            "" => "session inconnue".to_owned(),
            session => format!("session {:?}", Shown::tag(session)),
        };
        match self.agent.as_deref() {
            Some(agent) => {
                format!("{:?} ({session}, sous-agent {:?})", Logged(&self.event), Shown::name(agent))
            }
            None => format!("{:?} ({session})", Logged(&self.event)),
        }
    }
}

/// Longueur maximale d'un nom montré tel quel par [`Message::for_log`] : celle des noms que lit le
/// hook.
const MAX_LOGGED_NAME_CHARS: usize = 128;
/// Longueur maximale d'une catégorie montrée telle quelle par [`Message::for_log`].
const MAX_LOGGED_CODE_CHARS: usize = 64;

/// Un événement tel que le journal le montre (voir [`Message::for_log`]).
struct Logged<'a>(&'a Event);

impl fmt::Debug for Logged<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ni `_` ni `..` : un nouvel événement, ou un nouveau champ, oblige à décider ce que le
        // journal en garde
        match self.0 {
            Event::SessionStart { source, model: _ } => {
                f.debug_struct("SessionStart").field("source", &Shown::code_of(source)).finish()
            }
            Event::TurnStart => f.write_str("TurnStart"),
            Event::ToolStart { tool, id, summary } => f
                .debug_struct("ToolStart")
                .field("tool", &Shown::name(tool))
                .field("id", &Shown::tag_of(id))
                .field("summary", &Shown::length_of(summary))
                .finish(),
            Event::ToolEnd { tool, id, duration_ms } => f
                .debug_struct("ToolEnd")
                .field("tool", &Shown::name(tool))
                .field("id", &Shown::tag_of(id))
                .field("duration_ms", &Shown::millis(*duration_ms))
                .finish(),
            Event::ToolFailed { tool, id, error, interrupted, duration_ms } => f
                .debug_struct("ToolFailed")
                .field("tool", &Shown::name(tool))
                .field("id", &Shown::tag_of(id))
                .field("error", &Shown::length_of(error))
                .field("interrupted", interrupted)
                .field("duration_ms", &Shown::millis(*duration_ms))
                .finish(),
            Event::NeedsPermission { tool, summary } => f
                .debug_struct("NeedsPermission")
                .field("tool", &Shown::name(tool))
                .field("summary", &Shown::length_of(summary))
                .finish(),
            Event::Notification { notification_type, message } => f
                .debug_struct("Notification")
                .field("notification_type", &Shown::code(notification_type))
                .field("message", &Shown::length_of(message))
                .finish(),
            Event::AgentStart { agent_type, agent_id } => f
                .debug_struct("AgentStart")
                .field("agent_type", &Shown::name_of(agent_type))
                .field("agent_id", &Shown::tag_of(agent_id))
                .finish(),
            Event::AgentEnd { agent_type, agent_id } => f
                .debug_struct("AgentEnd")
                .field("agent_type", &Shown::name_of(agent_type))
                .field("agent_id", &Shown::tag_of(agent_id))
                .finish(),
            Event::TurnEnd { said } => f
                .debug_struct("TurnEnd")
                .field("said", &said.as_ref().map_or(Shown::Absent, |said| Shown::length(said.text())))
                .finish(),
            Event::TurnFailed { error } => {
                f.debug_struct("TurnFailed").field("error", &Shown::code_of(error)).finish()
            }
            Event::SessionEnd { reason } => {
                f.debug_struct("SessionEnd").field("reason", &Shown::code_of(reason)).finish()
            }
        }
    }
}

/// Un champ tel que le journal le montre.
enum Shown<'a> {
    /// Un nom ou une catégorie, tel quel.
    Name(&'a str),
    /// Un texte qu'on ne montre pas : sa longueur, en caractères.
    Length(usize),
    /// Un identifiant qu'on ne montre pas : son étiquette.
    Tag(u32),
    /// Une durée, en millisecondes.
    Millis(u64),
    /// Pas de valeur.
    Absent,
}

impl<'a> Shown<'a> {
    /// Le texte s'il a la forme d'un nom, sinon sa longueur.
    fn name(text: &'a str) -> Self {
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':');
        if !text.is_empty() && text.chars().count() <= MAX_LOGGED_NAME_CHARS && text.chars().all(allowed) {
            Self::Name(text)
        } else {
            Self::length(text)
        }
    }

    fn name_of(text: &'a Option<String>) -> Self {
        text.as_deref().map_or(Self::Absent, Self::name)
    }

    /// Le texte s'il a la forme d'une catégorie de Claude Code (`rate_limit`), sinon sa longueur.
    fn code(text: &'a str) -> Self {
        let allowed = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_';
        if !text.is_empty() && text.len() <= MAX_LOGGED_CODE_CHARS && text.bytes().all(allowed) {
            Self::Name(text)
        } else {
            Self::length(text)
        }
    }

    fn code_of(text: &'a Option<String>) -> Self {
        text.as_deref().map_or(Self::Absent, Self::code)
    }

    fn length(text: &str) -> Self {
        Self::Length(text.chars().count())
    }

    fn length_of(text: &Option<String>) -> Self {
        text.as_deref().map_or(Self::Absent, Self::length)
    }

    /// Une étiquette pour cet identifiant : FNV-1a sur 32 bits, la même d'un lancement à l'autre.
    /// Elle distingue des identifiants, et ne les redonne pas.
    fn tag(text: &str) -> Self {
        let hash = text
            .bytes()
            .fold(0x811c_9dc5_u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193));
        Self::Tag(hash)
    }

    fn tag_of(text: &Option<String>) -> Self {
        text.as_deref().map_or(Self::Absent, Self::tag)
    }

    fn millis(duration: Option<u64>) -> Self {
        duration.map_or(Self::Absent, Self::Millis)
    }
}

impl fmt::Debug for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => write!(f, "{name:?}"),
            Self::Length(count) => {
                write!(f, "{count} {}", if *count > 1 { "caractères" } else { "caractère" })
            }
            Self::Tag(tag) => write!(f, "#{tag:08x}"),
            Self::Millis(ms) => write!(f, "{ms} ms"),
            Self::Absent => f.write_str("—"),
        }
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
            Event::TurnEnd { said: None },
            Event::TurnEnd { said: Some(Said::new("C'est réparé, « enfin ».".into())) },
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
        assert_eq!(Message::decode(extra).unwrap().event, Event::TurnEnd { said: None });

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

    #[test]
    fn a_turn_end_without_a_line_for_capri_is_written_as_before() {
        let line = message(Event::TurnEnd { said: None }).encode().unwrap();
        assert_eq!(
            line,
            "{\"v\":1,\"session\":\"abc123\",\"cwd\":\"/Users/moi/projet\",\"at_ms\":1731250000000,\"kind\":\"turn_end\"}\n"
        );
        let with = message(Event::TurnEnd { said: Some(Said::new("Bien joué.".into())) }).encode().unwrap();
        assert!(with.ends_with(",\"kind\":\"turn_end\",\"said\":\"Bien joué.\"}\n"), "{with}");
    }

    #[test]
    fn the_line_for_capri_never_shows_in_a_debug_print() {
        let said = Said::new("le secret de la réponse".into());
        let printed = format!("{:?}", message(Event::TurnEnd { said: Some(said.clone()) }));
        assert!(!printed.contains("secret"), "{printed}");
        assert!(printed.contains("Said(23 caractères)"), "{printed}");
        assert_eq!(said.text(), "le secret de la réponse");
    }

    /// Un faux secret par champ, reconnaissable : une clé, un chemin, une commande.
    fn secret(field: &str) -> String {
        format!("SECRET-{field} sk-ant-api03-Zq9 cat /Users/moi/.ssh/id_rsa")
    }

    /// Chaque événement, chaque champ de texte rempli d'un faux secret.
    fn full_of_secrets() -> Vec<Message> {
        let s = secret;
        let events = [
            Event::SessionStart { source: Some(s("source")), model: Some("SECRET-modele-claude".into()) },
            Event::TurnStart,
            Event::ToolStart { tool: s("outil"), id: Some(s("id")), summary: Some(s("resume")) },
            Event::ToolEnd { tool: s("outil"), id: Some(s("id")), duration_ms: Some(12) },
            Event::ToolFailed {
                tool: s("outil"),
                id: Some(s("id")),
                error: Some(s("erreur")),
                interrupted: true,
                duration_ms: Some(4187),
            },
            Event::NeedsPermission { tool: s("outil"), summary: Some(s("resume")) },
            Event::Notification { notification_type: s("sorte"), message: Some(s("message")) },
            Event::AgentStart { agent_type: Some(s("type")), agent_id: Some(s("agent")) },
            Event::AgentEnd { agent_type: Some(s("type")), agent_id: Some(s("agent")) },
            Event::TurnEnd { said: Some(Said::new(s("phrase"))) },
            Event::TurnFailed { error: Some(s("erreur-api")) },
            Event::SessionEnd { reason: Some(s("raison")) },
        ];
        events
            .into_iter()
            .map(|event| Message {
                v: VERSION,
                // un identifiant qui a la forme d'un nom : il n'est jamais montré pour autant
                session: "SECRET-session-0b7c".into(),
                cwd: Some(s("dossier")),
                at_ms: 1_731_250_000_000,
                agent: Some(s("sous-agent")),
                event,
            })
            .collect()
    }

    #[test]
    fn the_log_form_never_shows_a_secret_from_any_field() {
        let messages = full_of_secrets();
        assert_eq!(messages.len(), 12, "chaque sorte d'événement");
        for message in &messages {
            let logged = message.for_log();
            for leak in ["SECRET", "sk-ant", "Users", "ssh", "id_rsa", "cat ", "1731250000000"] {
                assert!(!logged.contains(leak), "« {leak} » est sorti : {logged}");
            }
            // aucun de ces textes n'a la forme d'un nom : aucun n'est montré tel quel
            assert!(!logged.contains('"'), "{logged}");
            let session =
                format!("(session {:?}, sous-agent 61 caractères)", Shown::tag("SECRET-session-0b7c"));
            assert!(logged.ends_with(&session), "{logged}");
        }
        // les textes libres ne sont plus que des longueurs
        let tool = messages[4].for_log();
        assert!(tool.starts_with("ToolFailed { tool: 56 caractères, id: #"), "{tool}");
        assert!(tool.contains("error: 57 caractères, interrupted: true, duration_ms: 4187 ms }"), "{tool}");
        assert_eq!(messages[1].for_log(), messages[1].for_log(), "la même chose à chaque fois");
    }

    #[test]
    fn the_log_form_keeps_names_categories_durations_and_lengths() {
        let line = |event: Event| message(event).for_log();
        // l'identifiant de session « abc123 » : son étiquette, jamais lui
        assert_eq!(
            line(Event::ToolStart {
                tool: "Bash".into(),
                id: Some("toolu_01".into()),
                summary: Some("cargo test --workspace".into()),
            }),
            "ToolStart { tool: \"Bash\", id: #f56858b0, summary: 22 caractères } (session #38b29a05)"
        );
        assert_eq!(
            line(Event::ToolEnd { tool: "mcp__github__search".into(), id: None, duration_ms: Some(12) }),
            "ToolEnd { tool: \"mcp__github__search\", id: —, duration_ms: 12 ms } (session #38b29a05)"
        );
        assert_eq!(
            line(Event::Notification {
                notification_type: "permission_prompt".into(),
                message: Some("Claude needs your permission".into()),
            }),
            "Notification { notification_type: \"permission_prompt\", message: 28 caractères } (session #38b29a05)"
        );
        // le modèle n'est pas gardé ; une catégorie l'est
        let start =
            line(Event::SessionStart { source: Some("resume".into()), model: Some("claude-opus-5".into()) });
        assert_eq!(start, "SessionStart { source: \"resume\" } (session #38b29a05)");
        assert_eq!(
            line(Event::TurnFailed { error: Some("rate_limit".into()) }),
            "TurnFailed { error: \"rate_limit\" } (session #38b29a05)"
        );
        // une phrase à la place d'une catégorie : sa longueur
        assert_eq!(
            line(Event::TurnFailed { error: Some("API Error: Rate limit".into()) }),
            "TurnFailed { error: 21 caractères } (session #38b29a05)"
        );
        assert_eq!(
            line(Event::TurnEnd { said: Some(Said::new("C'est réparé.".into())) }),
            "TurnEnd { said: 13 caractères } (session #38b29a05)"
        );
        assert_eq!(line(Event::TurnEnd { said: None }), "TurnEnd { said: — } (session #38b29a05)");
        assert_eq!(line(Event::TurnStart), "TurnStart (session #38b29a05)");
        assert_eq!(
            line(Event::AgentStart {
                agent_type: Some("Explore".into()),
                agent_id: Some("agent-abc123".into())
            }),
            "AgentStart { agent_type: \"Explore\", agent_id: #ba0cb911 } (session #38b29a05)"
        );
        // un texte d'un seul caractère, au singulier
        assert_eq!(
            line(Event::NeedsPermission { tool: "Bash".into(), summary: Some("x".into()) }),
            "NeedsPermission { tool: \"Bash\", summary: 1 caractère } (session #38b29a05)"
        );

        // dans un sous-agent, son type ; une session sans identifiant est dite inconnue
        let inside = Message { agent: Some("vercel:ai-architect".into()), ..message(Event::TurnStart) };
        assert_eq!(inside.for_log(), "TurnStart (session #38b29a05, sous-agent \"vercel:ai-architect\")");
        let nameless = Message { session: String::new(), ..message(Event::TurnStart) };
        assert_eq!(nameless.for_log(), "TurnStart (session inconnue)");
    }

    #[test]
    fn a_tag_tells_identifiers_apart_and_is_the_same_from_one_launch_to_the_next() {
        // les valeurs publiées de FNV-1a sur 32 bits
        assert_eq!(format!("{:?}", Shown::tag("")), "#811c9dc5");
        assert_eq!(format!("{:?}", Shown::tag("a")), "#e40c292c");
        assert_eq!(format!("{:?}", Shown::tag("foobar")), "#bf9cf968");
        let of = |session: &str| Message { session: session.into(), ..message(Event::TurnStart) }.for_log();
        assert_eq!(of("session-une"), of("session-une"));
        assert_ne!(of("session-une"), of("session-deux"));
    }

    #[test]
    fn a_line_too_long_sheds_the_line_for_capri_rather_than_the_turn_end() {
        let mut sent = message(Event::TurnEnd { said: Some(Said::new("x".repeat(MAX_LINE_BYTES))) });
        let line = sent.encode_or_shed().expect("la fin du tour part quand même");
        assert_eq!(Message::decode(&line).unwrap().event, Event::TurnEnd { said: None });
        // ailleurs, rien n'est retiré : trop long reste trop long
        let mut tool =
            message(Event::ToolStart { tool: "x".repeat(MAX_LINE_BYTES), id: None, summary: None });
        assert!(matches!(tool.encode_or_shed(), Err(ProtocolError::TooLong(_))));
        let mut fine = message(Event::TurnEnd { said: Some(Said::new("court".into())) });
        assert!(fine.encode_or_shed().unwrap().contains("court"));
    }

    /// Le protocole tel qu'il était avant le miroir (commit 7115389), recopié tel quel : ce que
    /// comprend une Capri qui n'a pas été mise à jour.
    mod before_the_mirror {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Message {
            pub v: u32,
            pub session: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub cwd: Option<String>,
            pub at_ms: u64,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub agent: Option<String>,
            #[serde(flatten)]
            pub event: Event,
        }

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum Event {
            ToolStart {
                tool: String,
                #[serde(default, skip_serializing_if = "Option::is_none")]
                id: Option<String>,
                #[serde(default, skip_serializing_if = "Option::is_none")]
                summary: Option<String>,
            },
            TurnEnd,
            TurnFailed {
                #[serde(default, skip_serializing_if = "Option::is_none")]
                error: Option<String>,
            },
        }
    }

    #[test]
    fn an_older_capri_reads_the_new_turn_end_and_the_new_capri_reads_the_old_one() {
        // le hook d'aujourd'hui, une Capri d'avant : la phrase est ignorée, la fin du tour comprise
        let new_line =
            message(Event::TurnEnd { said: Some(Said::new("Tout passe.".into())) }).encode().unwrap();
        let old: before_the_mirror::Message =
            serde_json::from_str(new_line.trim_end()).expect("une Capri d'avant ne plante pas");
        assert_eq!(old.event, before_the_mirror::Event::TurnEnd);
        assert_eq!(old.session, "abc123");
        // une phrase qui n'est pas du texte ne la fait pas planter non plus
        let odd = r#"{"v":1,"session":"s","at_ms":1,"kind":"turn_end","said":{"x":[1,2]}}"#;
        assert_eq!(
            serde_json::from_str::<before_the_mirror::Message>(odd).unwrap().event,
            before_the_mirror::Event::TurnEnd
        );

        // un hook d'avant, la Capri d'aujourd'hui
        let old_line = serde_json::to_string(&before_the_mirror::Message {
            v: VERSION,
            session: "s".into(),
            cwd: None,
            at_ms: 1,
            agent: None,
            event: before_the_mirror::Event::TurnEnd,
        })
        .unwrap();
        assert_eq!(Message::decode(&old_line).unwrap().event, Event::TurnEnd { said: None });
        // et le reste de ce qu'il envoyait n'a pas changé d'un octet
        let tool = before_the_mirror::Event::ToolStart {
            tool: "Read".into(),
            id: None,
            summary: Some("a.rs".into()),
        };
        let old_tool = serde_json::to_string(&before_the_mirror::Message {
            v: VERSION,
            session: "s".into(),
            cwd: None,
            at_ms: 1,
            agent: None,
            event: tool,
        })
        .unwrap();
        let new_tool = Message {
            v: VERSION,
            session: "s".into(),
            cwd: None,
            at_ms: 1,
            agent: None,
            event: Event::ToolStart { tool: "Read".into(), id: None, summary: Some("a.rs".into()) },
        };
        assert_eq!(new_tool.encode().unwrap().trim_end(), old_tool);
        assert_eq!(Message::decode(&old_tool).unwrap(), new_tool);

        // une phrase qui n'est pas du texte ne compte pas, et la fin du tour reste comprise
        assert_eq!(Message::decode(odd).unwrap().event, Event::TurnEnd { said: None });
    }
}
