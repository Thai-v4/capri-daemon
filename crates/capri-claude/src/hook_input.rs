//! Lire ce que Claude Code écrit sur l'entrée standard d'un hook.
//!
//! On ne garde que les champs utiles au bandeau, et on tolère tout le reste : un champ absent,
//! d'un type inattendu ou inconnu ne fait jamais échouer la lecture. Les gros champs qu'on ne
//! lit pas (le contenu d'un fichier écrit, la sortie d'un outil) sont sautés sans être copiés.

use std::io::Read;

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

use crate::protocol::{Event, Message, VERSION};
use crate::text::{file_name, one_line};

/// Longueur maximale des textes libres transmis (résumé, erreur, message).
pub const MAX_TEXT_CHARS: usize = 200;
/// Longueur maximale des identifiants et des noms.
const MAX_NAME_CHARS: usize = 128;
/// Longueur maximale du dossier de travail.
const MAX_PATH_CHARS: usize = 1024;

/// Lit l'entrée d'un hook depuis un flux. `at_ms` : l'heure à noter dans le message.
///
/// `Ok(None)` : un événement que Capri ne suit pas. Ce n'est pas une erreur.
pub fn read(reader: impl Read, at_ms: u64) -> Result<Option<Message>, serde_json::Error> {
    serde_json::from_reader::<_, HookInput>(reader).map(|input| input.into_message(at_ms))
}

/// Comme [`read`], à partir d'un texte.
pub fn parse(json: &str, at_ms: u64) -> Result<Option<Message>, serde_json::Error> {
    serde_json::from_str::<HookInput>(json).map(|input| input.into_message(at_ms))
}

/// Les champs de l'entrée des hooks que Capri lit, tous événements confondus.
#[derive(Default, Deserialize)]
struct HookInput {
    #[serde(default, deserialize_with = "lenient")]
    hook_event_name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    session_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    cwd: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    agent_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    agent_type: Option<String>,
    // SessionStart
    #[serde(default, deserialize_with = "lenient")]
    source: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    model: Option<String>,
    // outils
    #[serde(default, deserialize_with = "lenient")]
    tool_name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    tool_use_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    tool_input: Option<ToolInput>,
    #[serde(default, deserialize_with = "lenient")]
    duration_ms: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    is_interrupt: Option<bool>,
    // PostToolUseFailure, StopFailure
    #[serde(default, deserialize_with = "lenient")]
    error: Option<String>,
    // Notification
    #[serde(default, deserialize_with = "lenient")]
    notification_type: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    message: Option<String>,
    // SessionEnd
    #[serde(default, deserialize_with = "lenient")]
    reason: Option<String>,
}

/// Les arguments d'outil dont on peut tirer un résumé court.
#[derive(Default, Deserialize)]
struct ToolInput {
    #[serde(default, deserialize_with = "lenient")]
    description: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    command: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    file_path: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pattern: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    url: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    query: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    subagent_type: Option<String>,
}

/// Lit une valeur du type attendu, ou `None` si le JSON contient autre chose à cet endroit.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Lenient<T> {
        Value(T),
        Other(IgnoredAny),
    }
    Ok(match Lenient::<T>::deserialize(deserializer)? {
        Lenient::Value(value) => Some(value),
        Lenient::Other(_) => None,
    })
}

impl HookInput {
    fn into_message(self, at_ms: u64) -> Option<Message> {
        let name =
            |text: Option<String>| text.map(|t| one_line(&t, MAX_NAME_CHARS)).filter(|t| !t.is_empty());
        let free =
            |text: Option<String>| text.map(|t| one_line(&t, MAX_TEXT_CHARS)).filter(|t| !t.is_empty());
        let duration_ms = self.duration_ms.filter(|d| d.is_finite() && *d >= 0.0).map(|d| d.round() as u64);
        let summary = self.tool_input.as_ref().and_then(ToolInput::summary);
        let tool = name(self.tool_name);
        let id = name(self.tool_use_id);
        let agent_type = name(self.agent_type);
        let mut inside_agent = agent_type.clone();

        let event = match self.hook_event_name.as_deref()? {
            "SessionStart" => Event::SessionStart { source: name(self.source), model: name(self.model) },
            "UserPromptSubmit" => Event::TurnStart,
            "PreToolUse" => Event::ToolStart { tool: tool?, id, summary },
            "PostToolUse" => Event::ToolEnd { tool: tool?, id, duration_ms },
            "PostToolUseFailure" => Event::ToolFailed {
                tool: tool?,
                id,
                error: free(self.error),
                interrupted: self.is_interrupt.unwrap_or(false),
                duration_ms,
            },
            "PermissionRequest" => Event::NeedsPermission { tool: tool?, summary },
            "Notification" => Event::Notification {
                notification_type: name(self.notification_type)?,
                message: free(self.message),
            },
            "SubagentStart" => {
                inside_agent = None; // le type est déjà dans l'événement
                Event::AgentStart { agent_type, agent_id: name(self.agent_id) }
            }
            "SubagentStop" => {
                inside_agent = None;
                Event::AgentEnd { agent_type, agent_id: name(self.agent_id) }
            }
            "Stop" => Event::TurnEnd,
            "StopFailure" => Event::TurnFailed { error: free(self.error) },
            "SessionEnd" => Event::SessionEnd { reason: name(self.reason) },
            _ => return None,
        };

        Some(Message {
            v: VERSION,
            session: name(self.session_id).unwrap_or_default(),
            cwd: self.cwd.map(|c| one_line(&c, MAX_PATH_CHARS)).filter(|c| !c.is_empty()),
            at_ms,
            agent: inside_agent,
            event,
        })
    }
}

impl ToolInput {
    /// Un résumé court de ce que l'outil va faire. La description rédigée par Claude passe avant
    /// la commande elle-même, qui peut contenir un secret ; d'un chemin on ne garde que le nom du
    /// fichier.
    fn summary(&self) -> Option<String> {
        let path = self.file_path.as_deref().map(file_name);
        [
            self.description.as_deref(),
            path,
            self.command.as_deref(),
            self.pattern.as_deref(),
            self.query.as_deref(),
            self.url.as_deref(),
            self.subagent_type.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(|text| one_line(text, MAX_TEXT_CHARS))
        .find(|text| !text.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: u64 = 1_731_250_000_000;

    fn event(json: &str) -> Event {
        parse(json, AT).expect("JSON valide").expect("événement suivi").event
    }

    // Les exemples ci-dessous reprennent ceux de la référence des hooks de Claude Code.

    #[test]
    fn pre_tool_use_for_bash() {
        let message = parse(
            r#"{
              "session_id": "abc123",
              "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
              "transcript_path": "/home/user/.claude/projects/.../transcript.jsonl",
              "cwd": "/home/user/my-project",
              "scratchpad_dir": "/tmp/claude-1000/-home-user-my-project/abc123/scratchpad",
              "permission_mode": "default",
              "hook_event_name": "PreToolUse",
              "tool_name": "Bash",
              "tool_input": {
                "command": "npm test",
                "description": "Run test suite",
                "timeout": 120000,
                "run_in_background": false
              },
              "tool_use_id": "toolu_01ABC123..."
            }"#,
            AT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            message,
            Message {
                v: VERSION,
                session: "abc123".into(),
                cwd: Some("/home/user/my-project".into()),
                at_ms: AT,
                agent: None,
                event: Event::ToolStart {
                    tool: "Bash".into(),
                    id: Some("toolu_01ABC123...".into()),
                    summary: Some("Run test suite".into()),
                },
            }
        );
    }

    #[test]
    fn session_start_after_a_resume() {
        assert_eq!(
            event(
                r#"{"session_id":"abc123","transcript_path":"/Users/.../x.jsonl","cwd":"/Users/...",
                    "hook_event_name":"SessionStart","source":"resume","model":"claude-opus-5",
                    "seconds_since_last_response":5400,"context_tokens":182340,
                    "prompt_cache_likely_expired":true,"estimated_cache_write_usd":1.1396}"#
            ),
            Event::SessionStart { source: Some("resume".into()), model: Some("claude-opus-5".into()) }
        );
    }

    #[test]
    fn the_prompt_text_is_never_forwarded() {
        let message = parse(
            r#"{"session_id":"abc123","cwd":"/Users/...","permission_mode":"default",
                "hook_event_name":"UserPromptSubmit",
                "prompt":"Write a function to calculate the factorial of a number"}"#,
            AT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(message.event, Event::TurnStart);
        assert!(!message.encode().unwrap().contains("factorial"));
    }

    #[test]
    fn post_tool_use_and_its_failure() {
        assert_eq!(
            event(
                r#"{"session_id":"abc123","hook_event_name":"PostToolUse","tool_name":"Write",
                    "tool_input":{"file_path":"/path/to/file.txt","content":"file content"},
                    "tool_response":{"filePath":"/path/to/file.txt","type":"create"},
                    "tool_use_id":"toolu_01ABC123...","duration_ms":12}"#
            ),
            Event::ToolEnd {
                tool: "Write".into(),
                id: Some("toolu_01ABC123...".into()),
                duration_ms: Some(12)
            }
        );
        assert_eq!(
            event(
                r#"{"session_id":"abc123","hook_event_name":"PostToolUseFailure","tool_name":"Bash",
                    "tool_input":{"command":"npm test","description":"Run test suite"},
                    "tool_use_id":"toolu_01ABC123...",
                    "error":"Exit code 1\nError: Cannot find module 'express'",
                    "is_interrupt":false,"duration_ms":4187}"#
            ),
            Event::ToolFailed {
                tool: "Bash".into(),
                id: Some("toolu_01ABC123...".into()),
                error: Some("Exit code 1".into()),
                interrupted: false,
                duration_ms: Some(4187),
            }
        );
    }

    #[test]
    fn permission_request_and_notification() {
        assert_eq!(
            event(
                r#"{"session_id":"abc123","permission_mode":"default","hook_event_name":"PermissionRequest",
                    "tool_name":"Bash",
                    "tool_input":{"command":"rm -rf node_modules","description":"Remove node_modules directory"},
                    "permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"rm -rf node_modules"}],"behavior":"allow","destination":"localSettings"}]}"#
            ),
            Event::NeedsPermission {
                tool: "Bash".into(),
                summary: Some("Remove node_modules directory".into())
            }
        );
        assert_eq!(
            event(
                r#"{"session_id":"abc123","hook_event_name":"Notification",
                    "message":"Claude needs your permission","title":"Permission needed",
                    "notification_type":"permission_prompt"}"#
            ),
            Event::Notification {
                notification_type: "permission_prompt".into(),
                message: Some("Claude needs your permission".into()),
            }
        );
    }

    #[test]
    fn subagents() {
        let start = parse(
            r#"{"session_id":"abc123","hook_event_name":"SubagentStart","agent_id":"agent-abc123","agent_type":"Explore"}"#,
            AT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(start.agent, None, "le type du sous-agent n'est pas répété hors de l'événement");
        assert_eq!(
            start.event,
            Event::AgentStart { agent_type: Some("Explore".into()), agent_id: Some("agent-abc123".into()) }
        );
        assert_eq!(
            event(
                r#"{"session_id":"abc123","hook_event_name":"SubagentStop","stop_hook_active":false,
                    "agent_id":"def456","agent_type":"Explore",
                    "last_assistant_message":"Analysis complete. Found 3 potential issues...",
                    "background_tasks":[],"session_crons":[]}"#
            ),
            Event::AgentEnd { agent_type: Some("Explore".into()), agent_id: Some("def456".into()) }
        );

        // un outil lancé par un sous-agent porte le type de ce sous-agent
        let inside = parse(
            r#"{"session_id":"abc123","hook_event_name":"PreToolUse","tool_name":"Read",
                "tool_input":{"file_path":"/a/b/main.rs"},"agent_id":"def456","agent_type":"Explore"}"#,
            AT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(inside.agent.as_deref(), Some("Explore"));
    }

    #[test]
    fn end_of_turn_and_of_session() {
        let stop = parse(
            r#"{"session_id":"abc123","permission_mode":"default","hook_event_name":"Stop",
                "stop_hook_active":true,
                "last_assistant_message":"I've completed the refactoring. Here's a summary...",
                "background_tasks":[{"id":"task-001","type":"shell","status":"running"}],"session_crons":[]}"#,
            AT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(stop.event, Event::TurnEnd);
        assert!(!stop.encode().unwrap().contains("refactoring"), "la réponse de Claude n'est pas transmise");

        assert_eq!(
            event(
                r#"{"session_id":"abc123","hook_event_name":"StopFailure","error":"rate_limit",
                    "error_details":"429 Too Many Requests","last_assistant_message":"API Error: Rate limit reached"}"#
            ),
            Event::TurnFailed { error: Some("rate_limit".into()) }
        );
        assert_eq!(
            event(r#"{"session_id":"abc123","hook_event_name":"SessionEnd","reason":"other"}"#),
            Event::SessionEnd { reason: Some("other".into()) }
        );
    }

    #[test]
    fn summaries_prefer_the_description_and_hide_directories() {
        let summary = |tool_input: &str| {
            let json = format!(
                r#"{{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"X","tool_input":{tool_input}}}"#
            );
            match event(&json) {
                Event::ToolStart { summary, .. } => summary,
                other => panic!("événement inattendu : {other:?}"),
            }
        };
        assert_eq!(
            summary(r#"{"command":"curl -H 'Authorization: secret' x","description":"Fetch the page"}"#)
                .as_deref(),
            Some("Fetch the page")
        );
        assert_eq!(summary(r#"{"command":"cargo test\n--workspace"}"#).as_deref(), Some("cargo test"));
        assert_eq!(
            summary(r#"{"file_path":"/Users/moi/secret-projet/src/main.rs","content":"fn main() {}"}"#)
                .as_deref(),
            Some("main.rs")
        );
        assert_eq!(summary(r#"{"file_path":"C:\\projet\\src\\index.ts"}"#).as_deref(), Some("index.ts"));
        assert_eq!(summary(r#"{"pattern":"TODO.*fix","path":"/Users/moi"}"#).as_deref(), Some("TODO.*fix"));
        assert_eq!(summary(r#"{"query":"react hooks"}"#).as_deref(), Some("react hooks"));
        assert_eq!(
            summary(r#"{"prompt":"Find all API endpoints","subagent_type":"Explore"}"#).as_deref(),
            Some("Explore")
        );
        assert_eq!(summary(r#"{"questions":[{"question":"Which framework?"}]}"#), None);
        assert_eq!(summary("{}"), None);
    }

    #[test]
    fn long_texts_are_cut_and_big_fields_are_not_kept() {
        let big = "x".repeat(2_000_000);
        let json = format!(
            r#"{{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Write",
                "tool_input":{{"file_path":"/a/gros.txt","content":"{big}","description":"{}"}}}}"#,
            "d".repeat(5_000)
        );
        let message = parse(&json, AT).unwrap().unwrap();
        let line = message.encode().expect("la ligne reste sous la limite du protocole");
        assert!(line.len() < 1_000, "{} octets", line.len());
        match message.event {
            Event::ToolStart { summary: Some(summary), .. } => {
                assert_eq!(summary.chars().count(), MAX_TEXT_CHARS);
                assert!(summary.ends_with('…'));
            }
            other => panic!("événement inattendu : {other:?}"),
        }
    }

    #[test]
    fn unknown_or_incomplete_events_are_skipped_without_error() {
        assert_eq!(parse(r#"{"session_id":"s","hook_event_name":"PreCompact"}"#, AT).unwrap(), None);
        assert_eq!(
            parse(r#"{"session_id":"s","hook_event_name":"EventFromTheFuture","x":1}"#, AT).unwrap(),
            None
        );
        assert_eq!(parse(r#"{"session_id":"s"}"#, AT).unwrap(), None);
        assert_eq!(parse("{}", AT).unwrap(), None);
        // un événement d'outil sans nom d'outil ne dit rien d'utile
        assert_eq!(parse(r#"{"session_id":"s","hook_event_name":"PreToolUse"}"#, AT).unwrap(), None);
    }

    #[test]
    fn unexpected_types_do_not_break_the_reading() {
        let message = parse(
            r#"{"session_id":42,"cwd":["pas","un","texte"],"hook_event_name":"PostToolUse",
                "tool_name":"Bash","tool_input":"pas un objet","tool_use_id":null,
                "duration_ms":"vite","is_interrupt":"non"}"#,
            AT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(message.session, "");
        assert_eq!(message.cwd, None);
        assert_eq!(message.event, Event::ToolEnd { tool: "Bash".into(), id: None, duration_ms: None });

        // une durée décimale est arrondie
        assert_eq!(
            event(r#"{"hook_event_name":"PostToolUse","tool_name":"Bash","duration_ms":12.6}"#),
            Event::ToolEnd { tool: "Bash".into(), id: None, duration_ms: Some(13) }
        );
    }

    #[test]
    fn malformed_json_is_an_error_and_a_stream_works_like_a_string() {
        assert!(parse("{ pas du json", AT).is_err());
        assert!(parse("", AT).is_err());
        let json = r#"{"session_id":"s","hook_event_name":"Stop"}"#;
        assert_eq!(read(json.as_bytes(), AT).unwrap(), parse(json, AT).unwrap());
    }
}
