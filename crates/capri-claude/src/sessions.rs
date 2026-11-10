//! À partir des messages reçus, quoi afficher dans le bandeau.
//!
//! Chaque session Claude Code a une phase (elle réfléchit, lance un outil, attend une permission,
//! a fini…). Le bandeau montre la phase la plus importante du moment, et se tait dès qu'il n'y a
//! plus rien à dire : le silence est un choix valide.
//!
//! Les hooks sont asynchrones : deux événements proches peuvent arriver dans le désordre, et une
//! session peut disparaître sans prévenir. Rien ici ne suppose un ordre parfait.
//!
//! Deux angles morts viennent de Claude Code lui-même, qui n'envoie alors aucun événement :
//! - une interruption par l'utilisateur (Échap) : ni `Stop`, ni fin d'outil ;
//! - une permission accordée : rien avant la fin de l'outil.
//!
//! Dans le doute, mieux vaut se taire qu'affirmer quelque chose de périmé : une phase sans
//! nouvelles finit par expirer.

use crate::protocol::{Event, Message};
use crate::text::{file_name, truncate};

/// Longueur maximale du texte du bandeau.
pub const MAX_HEADLINE_CHARS: usize = 72;

/// Durées d'affichage, en millisecondes.
const STARTED_FOR_MS: u64 = 4_000;
const DONE_FOR_MS: u64 = 6_000;
const FAILED_FOR_MS: u64 = 10_000;
const IDLE_PROMPT_FOR_MS: u64 = 12_000;
/// Sans nouvelles depuis ce délai, une session qui exécute un outil ou attend une réponse est
/// considérée comme partie (une commande peut tourner dix minutes).
const STALE_AFTER_MS: u64 = 10 * 60 * 1000;
/// Sans outil en cours, « Claude réfléchit » expire plus vite : c'est l'état dans lequel une
/// interruption laisse la session.
const THINKING_STALE_AFTER_MS: u64 = 2 * 60 * 1000;
/// Une demande de permission et le `PreToolUse` du même outil se suivent de très près, parfois
/// dans le désordre : en deçà de ce délai, le second ne lève pas l'attente.
const PERMISSION_RACE_MS: u64 = 1_500;

/// Le ton du bandeau, pour le rendu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Claude travaille.
    Working,
    /// Claude attend quelque chose de l'utilisateur.
    Waiting,
    /// Claude a fini.
    Done,
    /// Le tour a échoué.
    Failed,
}

/// Ce que le bandeau doit dire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Headline {
    pub text: String,
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// Rien à dire.
    Quiet,
    /// La session vient de démarrer.
    Started,
    /// Un tour est en cours, sans outil pour l'instant.
    Thinking,
    /// Un outil qui demande une réponse de l'utilisateur est ouvert (question, plan à valider).
    Asking {
        tool: String,
    },
    /// Claude Code attend une permission pour cet outil.
    NeedsPermission {
        tool: String,
        summary: Option<String>,
    },
    /// Claude a fini depuis un moment et attend la suite.
    IdlePrompt,
    Done,
    Failed {
        error: Option<String>,
    },
}

#[derive(Clone, Debug)]
struct RunningTool {
    id: Option<String>,
    tool: String,
    summary: Option<String>,
}

#[derive(Clone, Debug)]
struct Session {
    id: String,
    /// Nom du dossier de travail.
    project: Option<String>,
    phase: Phase,
    /// Depuis quand la phase dure, et dernier signe de vie (horloge de Capri).
    since_ms: u64,
    seen_ms: u64,
    /// Outils en cours, du plus ancien au plus récent.
    tools: Vec<RunningTool>,
}

/// Suit toutes les sessions Claude Code qui envoient des événements.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    sessions: Vec<Session>,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prend en compte un message reçu à l'instant `now_ms` (horloge de Capri).
    pub fn apply(&mut self, message: &Message, now_ms: u64) {
        if matches!(message.event, Event::SessionEnd { .. }) {
            self.sessions.retain(|s| s.id != message.session);
            return;
        }
        let index = match self.sessions.iter().position(|s| s.id == message.session) {
            Some(index) => index,
            None => {
                self.sessions.push(Session {
                    id: message.session.clone(),
                    project: None,
                    phase: Phase::Quiet,
                    since_ms: now_ms,
                    seen_ms: now_ms,
                    tools: Vec::new(),
                });
                self.sessions.len() - 1
            }
        };
        let session = &mut self.sessions[index];
        if let Some(cwd) = &message.cwd {
            session.project = Some(file_name(cwd).to_owned()).filter(|p| !p.is_empty());
        }
        session.seen_ms = now_ms;
        session.react(&message.event, now_ms);
    }

    /// Ce que le bandeau doit dire à l'instant `now_ms`, s'il y a quelque chose à dire.
    pub fn headline(&mut self, now_ms: u64) -> Option<Headline> {
        self.sessions.retain(|s| now_ms.saturating_sub(s.seen_ms) < 3 * STALE_AFTER_MS);
        let live = self.sessions.iter().filter(|s| now_ms.saturating_sub(s.seen_ms) < STALE_AFTER_MS).count();
        let (session, line, tone) = self
            .sessions
            .iter()
            .filter_map(|s| s.line(now_ms).map(|(line, tone)| (s, line, tone)))
            .min_by_key(|(s, _, tone)| (priority(*tone), std::cmp::Reverse(s.since_ms)))?;
        let text = match (&session.project, live > 1) {
            (Some(project), true) => format!("{project} · {line}"),
            _ => line,
        };
        Some(Headline { text: truncate(&text, MAX_HEADLINE_CHARS), tone })
    }

    /// Nombre de sessions suivies.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

/// Du plus au moins urgent.
fn priority(tone: Tone) -> u8 {
    match tone {
        Tone::Waiting => 0,
        Tone::Failed => 1,
        Tone::Working => 2,
        Tone::Done => 3,
    }
}

impl Session {
    fn set_phase(&mut self, phase: Phase, now_ms: u64) {
        if self.phase != phase {
            self.phase = phase;
            self.since_ms = now_ms;
        }
    }

    fn react(&mut self, event: &Event, now_ms: u64) {
        match event {
            Event::SessionStart { .. } => {
                self.tools.clear();
                self.set_phase(Phase::Started, now_ms);
            }
            Event::TurnStart => {
                self.tools.clear();
                self.set_phase(Phase::Thinking, now_ms);
            }
            Event::ToolStart { tool, id, summary } => {
                if !id.is_some() || !self.tools.iter().any(|t| t.id == *id) {
                    self.tools.push(RunningTool {
                        id: id.clone(),
                        tool: tool.clone(),
                        summary: summary.clone(),
                    });
                }
                let racing_permission = matches!(&self.phase, Phase::NeedsPermission { tool: waiting, .. }
                    if waiting == tool && now_ms.saturating_sub(self.since_ms) < PERMISSION_RACE_MS);
                if asks_the_user(tool) {
                    self.set_phase(Phase::Asking { tool: tool.clone() }, now_ms);
                } else if !racing_permission {
                    self.forget_denied_tool(Some(tool));
                    self.set_phase(Phase::Thinking, now_ms);
                }
            }
            Event::ToolEnd { tool, id, .. } | Event::ToolFailed { tool, id, .. } => {
                self.finish_tool(tool, id.as_deref());
                // La fin d'un outil rend la main à Claude, sauf si elle arrive en retard, après
                // la fin du tour.
                if matches!(
                    self.phase,
                    Phase::Thinking | Phase::Asking { .. } | Phase::NeedsPermission { .. }
                ) {
                    self.set_phase(Phase::Thinking, now_ms);
                }
            }
            Event::NeedsPermission { tool, summary } => {
                self.set_phase(waiting_for_the_user(tool, summary.as_deref()), now_ms);
            }
            Event::Notification { notification_type, .. } => match notification_type.as_str() {
                // la même demande que `NeedsPermission`, quelques secondes plus tard
                "permission_prompt" => {
                    if !matches!(self.phase, Phase::NeedsPermission { .. } | Phase::Asking { .. }) {
                        let waiting = self.tools.last().map(|t| (t.tool.clone(), t.summary.clone()));
                        let (tool, summary) = waiting.unwrap_or_default();
                        self.set_phase(waiting_for_the_user(&tool, summary.as_deref()), now_ms);
                    }
                }
                "idle_prompt" => self.set_phase(Phase::IdlePrompt, now_ms),
                "elicitation_dialog" | "elicitation_url_dialog" | "agent_needs_input" => {
                    self.set_phase(Phase::Asking { tool: String::new() }, now_ms);
                }
                _ => {}
            },
            // Les sous-agents se voient par les outils qu'ils lancent.
            Event::AgentStart { .. } | Event::AgentEnd { .. } => {}
            Event::TurnEnd => {
                self.tools.clear();
                self.set_phase(Phase::Done, now_ms);
            }
            Event::TurnFailed { error } => {
                self.tools.clear();
                self.set_phase(Phase::Failed { error: error.clone() }, now_ms);
            }
            Event::SessionEnd { .. } => {}
        }
    }

    fn finish_tool(&mut self, tool: &str, id: Option<&str>) {
        let position = match id {
            Some(id) => self.tools.iter().position(|t| t.id.as_deref() == Some(id)),
            None => None,
        }
        .or_else(|| self.tools.iter().rposition(|t| t.tool == tool));
        if let Some(position) = position {
            self.tools.remove(position);
        }
    }

    /// Une permission en attente que suit un autre événement a été refusée : l'outil ne
    /// s'exécutera pas, et aucun `PostToolUse` ne viendra le retirer. `starting` : l'outil qui
    /// démarre à l'instant, à ne pas confondre avec lui.
    fn forget_denied_tool(&mut self, starting: Option<&str>) {
        let Phase::NeedsPermission { tool: denied, .. } = &self.phase else { return };
        let newest = self.tools.len().saturating_sub(1);
        let position = self
            .tools
            .iter()
            .enumerate()
            .rposition(|(i, t)| t.tool == *denied && !(i == newest && starting == Some(t.tool.as_str())));
        if let Some(position) = position {
            self.tools.remove(position);
        }
    }

    /// Le texte de cette session et son ton, si elle a quelque chose à dire.
    fn line(&self, now_ms: u64) -> Option<(String, Tone)> {
        let age = now_ms.saturating_sub(self.since_ms);
        let silent_for = now_ms.saturating_sub(self.seen_ms);
        let stale = silent_for >= STALE_AFTER_MS;
        match &self.phase {
            Phase::Quiet => None,
            Phase::Started => (age < STARTED_FOR_MS).then(|| {
                let line = match &self.project {
                    Some(project) => format!("Claude se réveille dans {project}"),
                    None => "Claude se réveille".to_owned(),
                };
                (line, Tone::Working)
            }),
            Phase::Thinking => match self.tools.last() {
                Some(tool) => {
                    (!stale).then(|| (working_line(&tool.tool, tool.summary.as_deref()), Tone::Working))
                }
                None => (silent_for < THINKING_STALE_AFTER_MS)
                    .then(|| ("Claude réfléchit…".to_owned(), Tone::Working)),
            },
            Phase::Asking { tool } => (!stale).then(|| {
                let line = match tool.as_str() {
                    "ExitPlanMode" => "Claude te soumet son plan",
                    _ => "Claude te pose une question",
                };
                (line.to_owned(), Tone::Waiting)
            }),
            Phase::NeedsPermission { tool, summary } => (!stale).then(|| {
                let what = permission_subject(tool, summary.as_deref());
                (format!("Claude attend ton accord{what}"), Tone::Waiting)
            }),
            Phase::IdlePrompt => {
                (age < IDLE_PROMPT_FOR_MS).then(|| ("Claude t'attend".to_owned(), Tone::Waiting))
            }
            Phase::Done => (age < DONE_FOR_MS).then(|| ("Claude a fini".to_owned(), Tone::Done)),
            Phase::Failed { error } => (age < FAILED_FOR_MS).then(|| {
                let line = match error.as_deref().map(error_label) {
                    Some(label) => format!("Claude a calé : {label}"),
                    None => "Claude a calé".to_owned(),
                };
                (line, Tone::Failed)
            }),
        }
    }
}

/// Les outils qui ouvrent une question à l'utilisateur plutôt qu'un travail.
fn asks_the_user(tool: &str) -> bool {
    matches!(tool, "AskUserQuestion" | "ExitPlanMode")
}

/// La phase d'attente qui convient à cet outil. Claude Code signale une question posée à
/// l'utilisateur comme une demande de permission : ce n'en est pas une.
fn waiting_for_the_user(tool: &str, summary: Option<&str>) -> Phase {
    if asks_the_user(tool) {
        Phase::Asking { tool: tool.to_owned() }
    } else {
        Phase::NeedsPermission { tool: tool.to_owned(), summary: summary.map(str::to_owned) }
    }
}

fn working_line(tool: &str, summary: Option<&str>) -> String {
    let with = |verb: &str, fallback: &str| match summary {
        Some(summary) => format!("{verb} {summary}"),
        None => fallback.to_owned(),
    };
    match tool {
        "Bash" | "PowerShell" | "Monitor" => match summary {
            Some(summary) => format!("Claude lance : {summary}"),
            None => "Claude lance une commande".to_owned(),
        },
        "Read" => with("Claude lit", "Claude lit un fichier"),
        "Write" => with("Claude écrit", "Claude écrit un fichier"),
        "Edit" | "NotebookEdit" => with("Claude modifie", "Claude modifie un fichier"),
        "Glob" | "Grep" => with("Claude cherche", "Claude cherche dans le projet"),
        "WebFetch" | "WebSearch" => "Claude cherche sur le web".to_owned(),
        "Agent" => match summary {
            Some(summary) => format!("Claude envoie un sous-agent : {summary}"),
            None => "Claude envoie un sous-agent".to_owned(),
        },
        _ => format!("Claude utilise {}", tool_label(tool)),
    }
}

/// Un nom lisible pour un outil : `mcp__github__search` devient `github`.
fn tool_label(tool: &str) -> &str {
    match tool.strip_prefix("mcp__") {
        Some(rest) => rest.split("__").next().filter(|server| !server.is_empty()).unwrap_or(tool),
        None => tool,
    }
}

fn permission_subject(tool: &str, summary: Option<&str>) -> String {
    match (tool, summary) {
        (_, Some(summary)) => format!(" : {summary}"),
        ("", None) => String::new(),
        ("Bash" | "PowerShell" | "Monitor", None) => " pour une commande".to_owned(),
        (tool, None) => format!(" pour {}", tool_label(tool)),
    }
}

/// Les catégories d'erreur de `StopFailure`, en clair.
fn error_label(error: &str) -> &str {
    match error {
        "rate_limit" => "limite d'usage atteinte",
        "overloaded" => "serveurs surchargés",
        "authentication_failed" | "oauth_org_not_allowed" => "connexion refusée",
        "account_on_hold" | "billing_error" => "problème de compte",
        "invalid_request" => "requête refusée",
        "model_not_found" => "modèle introuvable",
        "server_error" => "erreur du serveur",
        "max_output_tokens" => "réponse trop longue",
        "cloud_credential_error" => "identifiants cloud refusés",
        "unknown" => "erreur inconnue",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::VERSION;

    fn message(session: &str, cwd: Option<&str>, event: Event) -> Message {
        Message {
            v: VERSION,
            session: session.into(),
            cwd: cwd.map(Into::into),
            at_ms: 0,
            agent: None,
            event,
        }
    }

    fn start(tool: &str, id: &str, summary: Option<&str>) -> Event {
        Event::ToolStart { tool: tool.into(), id: Some(id.into()), summary: summary.map(Into::into) }
    }

    fn end(tool: &str, id: &str) -> Event {
        Event::ToolEnd { tool: tool.into(), id: Some(id.into()), duration_ms: None }
    }

    fn permission(tool: &str, summary: Option<&str>) -> Event {
        Event::NeedsPermission { tool: tool.into(), summary: summary.map(Into::into) }
    }

    /// Un suivi à une seule session « s », pour dérouler un scénario.
    struct Scenario(Tracker);

    impl Scenario {
        fn new() -> Self {
            Self(Tracker::new())
        }

        fn at(&mut self, now_ms: u64, event: Event) -> &mut Self {
            self.0.apply(&message("s", Some("/Users/moi/capri"), event), now_ms);
            self
        }

        fn text(&mut self, now_ms: u64) -> Option<String> {
            self.0.headline(now_ms).map(|h| h.text)
        }

        fn tone(&mut self, now_ms: u64) -> Option<Tone> {
            self.0.headline(now_ms).map(|h| h.tone)
        }
    }

    #[test]
    fn says_nothing_until_something_happens() {
        let mut tracker = Tracker::new();
        assert_eq!(tracker.headline(0), None);
        assert!(tracker.is_empty());
    }

    #[test]
    fn follows_a_whole_turn() {
        let mut s = Scenario::new();
        s.at(0, Event::SessionStart { source: Some("startup".into()), model: None });
        assert_eq!(s.text(100).as_deref(), Some("Claude se réveille dans capri"));
        assert_eq!(s.text(STARTED_FOR_MS), None, "l'accueil ne dure pas");

        s.at(5_000, Event::TurnStart);
        assert_eq!(s.text(5_100).as_deref(), Some("Claude réfléchit…"));

        s.at(6_000, start("Read", "t1", Some("main.rs")));
        assert_eq!(s.text(6_100).as_deref(), Some("Claude lit main.rs"));
        s.at(6_200, end("Read", "t1"));
        assert_eq!(s.text(6_300).as_deref(), Some("Claude réfléchit…"));

        s.at(7_000, start("Bash", "t2", Some("Run test suite")));
        assert_eq!(s.text(7_100).as_deref(), Some("Claude lance : Run test suite"));
        assert_eq!(s.tone(7_100), Some(Tone::Working));
        s.at(
            9_000,
            Event::ToolFailed {
                tool: "Bash".into(),
                id: Some("t2".into()),
                error: Some("Exit code 1".into()),
                interrupted: false,
                duration_ms: None,
            },
        );
        assert_eq!(
            s.text(9_100).as_deref(),
            Some("Claude réfléchit…"),
            "un outil qui échoue, c'est la routine"
        );

        s.at(10_000, Event::TurnEnd);
        assert_eq!(s.text(10_100).as_deref(), Some("Claude a fini"));
        assert_eq!(s.tone(10_100), Some(Tone::Done));
        assert_eq!(s.text(10_000 + DONE_FOR_MS), None, "puis le bandeau se tait");
    }

    #[test]
    fn names_what_each_tool_does() {
        let line = |tool: &str, summary: Option<&str>| {
            let mut s = Scenario::new();
            s.at(0, start(tool, "t", summary));
            s.text(1).unwrap()
        };
        assert_eq!(line("Bash", None), "Claude lance une commande");
        assert_eq!(line("Write", Some("lib.rs")), "Claude écrit lib.rs");
        assert_eq!(line("Edit", Some("lib.rs")), "Claude modifie lib.rs");
        assert_eq!(line("Grep", Some("TODO")), "Claude cherche TODO");
        assert_eq!(line("Glob", None), "Claude cherche dans le projet");
        assert_eq!(line("WebSearch", Some("rust wgpu")), "Claude cherche sur le web");
        assert_eq!(line("Agent", Some("Explore")), "Claude envoie un sous-agent : Explore");
        assert_eq!(line("mcp__github__search_repositories", None), "Claude utilise github");
        assert_eq!(line("OutilInconnu", None), "Claude utilise OutilInconnu");
    }

    #[test]
    fn a_permission_request_takes_over_until_it_is_answered() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(1_000, start("Bash", "t1", Some("Remove node_modules directory")));
        s.at(1_010, permission("Bash", Some("Remove node_modules directory")));
        assert_eq!(
            s.text(1_100).as_deref(),
            Some("Claude attend ton accord : Remove node_modules directory")
        );
        assert_eq!(s.tone(1_100), Some(Tone::Waiting));

        // le rappel de Claude Code, six secondes plus tard, ne change rien
        s.at(7_000, Event::Notification { notification_type: "permission_prompt".into(), message: None });
        assert_eq!(s.tone(60_000), Some(Tone::Waiting), "tant qu'il n'y a pas de réponse");

        // accordée : l'outil s'exécute puis se termine
        s.at(70_000, end("Bash", "t1"));
        assert_eq!(s.text(70_100).as_deref(), Some("Claude réfléchit…"));
    }

    #[test]
    fn a_permission_request_may_arrive_before_its_tool_start() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(1_000, permission("Bash", None));
        s.at(1_020, start("Bash", "t1", Some("cargo publish")));
        assert_eq!(s.tone(1_100), Some(Tone::Waiting), "le PreToolUse en retard ne lève pas l'attente");
        assert_eq!(s.text(1_100).as_deref(), Some("Claude attend ton accord pour une commande"));
        s.at(9_000, end("Bash", "t1"));
        assert_eq!(s.text(9_100).as_deref(), Some("Claude réfléchit…"));
    }

    #[test]
    fn a_denied_tool_is_forgotten_when_claude_moves_on() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(1_000, start("Bash", "t1", Some("rm -rf build")));
        s.at(1_010, permission("Bash", Some("rm -rf build")));
        // refusée : aucun PostToolUse pour t1, Claude passe à autre chose
        s.at(20_000, start("Read", "t2", Some("Makefile")));
        assert_eq!(s.text(20_100).as_deref(), Some("Claude lit Makefile"));
        s.at(20_200, end("Read", "t2"));
        assert_eq!(s.text(20_300).as_deref(), Some("Claude réfléchit…"), "la commande refusée ne traîne pas");
    }

    #[test]
    fn questions_and_plans_wait_for_the_user() {
        let mut s = Scenario::new();
        s.at(0, start("AskUserQuestion", "t1", None));
        assert_eq!(s.text(100).as_deref(), Some("Claude te pose une question"));
        assert_eq!(s.tone(100), Some(Tone::Waiting));
        s.at(5_000, end("AskUserQuestion", "t1"));
        assert_eq!(s.tone(5_100), Some(Tone::Working));

        s.at(6_000, start("ExitPlanMode", "t2", None));
        assert_eq!(s.text(6_100).as_deref(), Some("Claude te soumet son plan"));

        let mut s = Scenario::new();
        s.at(0, Event::TurnEnd);
        s.at(60_000, Event::Notification { notification_type: "idle_prompt".into(), message: None });
        assert_eq!(s.text(60_100).as_deref(), Some("Claude t'attend"));
        assert_eq!(s.text(60_000 + IDLE_PROMPT_FOR_MS), None);
        s.at(80_000, Event::Notification { notification_type: "auth_success".into(), message: None });
        assert_eq!(s.text(80_100), None, "les autres notifications ne regardent pas le bandeau");
    }

    #[test]
    fn a_question_reported_as_a_permission_request_stays_a_question() {
        // la suite d'événements que Claude Code envoie vraiment pour AskUserQuestion
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(1_000, start("AskUserQuestion", "t1", None));
        s.at(1_010, permission("AskUserQuestion", None));
        assert_eq!(s.text(1_100).as_deref(), Some("Claude te pose une question"));
        s.at(7_000, Event::Notification { notification_type: "permission_prompt".into(), message: None });
        assert_eq!(s.text(7_100).as_deref(), Some("Claude te pose une question"));
        assert_eq!(s.tone(7_100), Some(Tone::Waiting));
        s.at(20_000, end("AskUserQuestion", "t1"));
        assert_eq!(s.text(20_100).as_deref(), Some("Claude réfléchit…"));

        // même chose pour un plan à valider, et si le rappel arrive seul
        let mut s = Scenario::new();
        s.at(0, permission("ExitPlanMode", None));
        assert_eq!(s.text(100).as_deref(), Some("Claude te soumet son plan"));
        let mut s = Scenario::new();
        s.at(0, start("AskUserQuestion", "t1", None));
        s.at(10, end("Read", "ailleurs"));
        s.at(6_000, Event::Notification { notification_type: "permission_prompt".into(), message: None });
        assert_eq!(s.text(6_100).as_deref(), Some("Claude te pose une question"));
    }

    #[test]
    fn failures_are_explained_in_plain_words() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnFailed { error: Some("rate_limit".into()) });
        assert_eq!(s.text(100).as_deref(), Some("Claude a calé : limite d'usage atteinte"));
        assert_eq!(s.tone(100), Some(Tone::Failed));
        assert_eq!(s.text(FAILED_FOR_MS), None);

        let mut s = Scenario::new();
        s.at(0, Event::TurnFailed { error: None });
        assert_eq!(s.text(100).as_deref(), Some("Claude a calé"));
    }

    #[test]
    fn parallel_tools_and_late_events_do_not_confuse_it() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(100, start("Read", "a", Some("a.rs")));
        s.at(110, start("Read", "b", Some("b.rs")));
        assert_eq!(s.text(200).as_deref(), Some("Claude lit b.rs"), "le plus récent");
        s.at(300, end("Read", "b"));
        assert_eq!(s.text(400).as_deref(), Some("Claude lit a.rs"));
        // la fin d'un outil jamais vu, puis un doublon : sans effet
        s.at(500, end("Grep", "inconnu"));
        s.at(510, start("Read", "a", Some("a.rs")));
        s.at(600, end("Read", "a"));
        assert_eq!(s.text(700).as_deref(), Some("Claude réfléchit…"));
    }

    #[test]
    fn a_session_that_goes_silent_is_dropped() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(1_000, start("Bash", "t1", None));
        assert!(s.text(STALE_AFTER_MS).is_some(), "une commande peut tourner longtemps");
        assert_eq!(s.text(1_000 + STALE_AFTER_MS), None, "Claude Code a sans doute été fermé sans prévenir");
        assert_eq!(s.text(1_000 + 3 * STALE_AFTER_MS), None);
        assert!(s.0.is_empty(), "et la session finit par être oubliée");
    }

    #[test]
    fn thinking_without_news_expires_quickly() {
        // après une interruption (Échap), Claude Code n'envoie plus rien
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        assert!(s.text(THINKING_STALE_AFTER_MS - 1).is_some());
        assert_eq!(s.text(THINKING_STALE_AFTER_MS), None, "mieux vaut se taire");
        // le moindre signe de vie la ranime
        s.at(THINKING_STALE_AFTER_MS + 5_000, start("Read", "t1", Some("a.rs")));
        assert_eq!(s.text(THINKING_STALE_AFTER_MS + 5_100).as_deref(), Some("Claude lit a.rs"));
    }

    #[test]
    fn a_tool_end_arriving_after_the_end_of_the_turn_changes_nothing() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(100, start("Read", "t1", Some("a.rs")));
        s.at(200, Event::TurnEnd);
        s.at(210, end("Read", "t1"));
        assert_eq!(s.text(300).as_deref(), Some("Claude a fini"));
        assert_eq!(s.text(210 + DONE_FOR_MS), None);
    }

    #[test]
    fn the_end_of_a_session_clears_it_at_once() {
        let mut s = Scenario::new();
        s.at(0, Event::TurnStart);
        s.at(100, Event::SessionEnd { reason: Some("other".into()) });
        assert_eq!(s.text(200), None);
        assert_eq!(s.0.len(), 0);
        // la fin d'une session inconnue n'en crée pas
        s.at(300, Event::SessionEnd { reason: None });
        assert_eq!(s.0.len(), 0);
    }

    #[test]
    fn with_several_sessions_the_most_urgent_speaks_and_names_its_project() {
        let mut tracker = Tracker::new();
        tracker.apply(&message("a", Some("/Users/moi/capri"), Event::TurnStart), 0);
        assert_eq!(
            tracker.headline(10).unwrap().text,
            "Claude réfléchit…",
            "une seule session : pas de préfixe"
        );

        tracker.apply(&message("b", Some("/Users/moi/portfolio"), start("Bash", "t", Some("npm test"))), 100);
        assert_eq!(
            tracker.headline(200).unwrap().text,
            "portfolio · Claude lance : npm test",
            "la plus récente"
        );

        tracker.apply(&message("a", None, permission("Edit", Some("main.rs"))), 300);
        let headline = tracker.headline(400).unwrap();
        assert_eq!(headline.text, "capri · Claude attend ton accord : main.rs");
        assert_eq!(headline.tone, Tone::Waiting, "une attente passe avant un travail en cours");

        tracker.apply(&message("a", None, Event::TurnEnd), 500);
        assert_eq!(tracker.headline(600).unwrap().text, "portfolio · Claude lance : npm test");
        tracker.apply(&message("b", None, Event::SessionEnd { reason: None }), 700);
        assert_eq!(tracker.headline(800).unwrap().text, "Claude a fini");
    }

    #[test]
    fn headlines_never_exceed_the_banner_width() {
        let mut s = Scenario::new();
        s.at(0, start("Bash", "t", Some(&"très long ".repeat(40))));
        let text = s.text(1).unwrap();
        assert_eq!(text.chars().count(), MAX_HEADLINE_CHARS);
        assert!(text.ends_with('…'));
    }
}
