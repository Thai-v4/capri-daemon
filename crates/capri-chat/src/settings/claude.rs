//! La rubrique « Claude Code » : le branchement du hook, le style de sortie « Capri », et le
//! miroir.
//!
//! Les mêmes règles et les mêmes phrases que `capri hook` (`capri_claude::settings`, `style`,
//! `mirror`) :
//! - brancher et débrancher passent par les fonctions de `capri hook install | uninstall`
//!   (sauvegarde datée, écriture d'un seul coup, réécriture à l'octet près ou refus) ;
//! - le style n'est écrit que sur l'appui du bouton, jamais par-dessus un fichier qui n'est pas le
//!   nôtre ; Capri ne l'active pas : la rubrique dit quoi taper dans Claude Code ;
//! - le miroir est désactivé tant que la case n'est pas cochée, et la rubrique dit ce qu'il fait
//!   avant qu'on la coche, et ce qui lui manque encore pour qu'une phrase arrive.
//!
//! Rien ici ne renvoie à une commande de terminal : `/output-style Capri` se tape dans Claude
//! Code.

use capri_claude::mirror;
use capri_claude::settings::ALL_HOOKS_DISABLED;
use capri_claude::style::{self, StyleState};
use capri_core::form::{Button, Control, FormEvent, Row, Tone};

use super::{Effect, HookChange, HookState, Outcome, Rubric, StyleFile};

pub const ID: &str = "claude";

// Les lignes et les boutons de la rubrique.
pub const HOOK: &str = "claude.hook";
pub const HOOK_ACTIONS: &str = "claude.hook.actions";
pub const PLUG: &str = "claude.hook.plug";
pub const UNPLUG: &str = "claude.hook.unplug";
pub const HOOK_NOTE: &str = "claude.hook.note";
pub const STYLE_ABOUT: &str = "claude.style.about";
pub const STYLE: &str = "claude.style";
pub const STYLE_ACTIONS: &str = "claude.style.actions";
pub const INSTALL_STYLE: &str = "claude.style.install";
pub const REMOVE_STYLE: &str = "claude.style.remove";
pub const STYLE_HOW: &str = "claude.style.how";
pub const STYLE_NOTE: &str = "claude.style.note";
pub const MIRROR: &str = "claude.mirror";
pub const MIRROR_ABOUT: &str = "claude.mirror.about";
pub const MIRROR_NOTE: &str = "claude.mirror.note";

/// Ce qu'on affiche en attendant une relecture.
const READING: &str = "Lecture…";
const BUSY: &str = "Un instant…";

#[derive(Default)]
pub struct ClaudeRubric {
    /// `None` : pas encore lu.
    hook: Option<Result<HookState, String>>,
    mirror: Option<Result<bool, String>>,
    style: Option<Result<StyleFile, String>>,
    /// Ce qu'a donné le dernier appui, sous chaque partie.
    hook_note: Option<(Tone, String)>,
    style_note: Option<(Tone, String)>,
    mirror_note: Option<(Tone, String)>,
}

impl ClaudeRubric {
    pub fn new() -> Self {
        Self::default()
    }

    fn hook_state(&self) -> Option<&HookState> {
        self.hook.as_ref().and_then(|hook| hook.as_ref().ok())
    }

    fn style_state(&self) -> Option<StyleState> {
        self.style.as_ref().and_then(|style| style.as_ref().ok()).map(|style| style.state)
    }

    fn mirror_on(&self) -> Option<bool> {
        self.mirror.as_ref().and_then(|mirror| mirror.as_ref().ok()).copied()
    }

    /// Ce que dit la ligne du branchement.
    fn hook_line(&self) -> (Tone, String) {
        let state = match &self.hook {
            None => return (Tone::Busy, READING.to_owned()),
            Some(Err(problem)) => return (Tone::Bad, problem.clone()),
            Some(Ok(state)) => state,
        };
        let status = &state.status;
        let mut text = status.describe();
        let mut tone = if status.is_installed() { Tone::Good } else { Tone::Quiet };
        if !status.is_installed() {
            text.push_str(
                " Le bandeau ne suit pas tes sessions tant qu'il ne l'est pas : appuie sur « Brancher ».",
            );
        } else if state.missing_binary {
            tone = Tone::Warning;
            text.push_str(
                " Mais le programme capri-hook qu'il appelle est introuvable : « Brancher » le remplace par \
                 celui de cette Capri.",
            );
        } else if !status.is_complete() {
            tone = Tone::Warning;
            text.push_str(" « Brancher » ajoute les autres.");
        }
        if status.all_hooks_disabled {
            tone = Tone::Warning;
            text.push(' ');
            text.push_str(ALL_HOOKS_DISABLED);
        }
        text.push_str(&format!(" Réglages de Claude Code : {}.", state.file));
        (tone, text)
    }

    /// Ce que dit la ligne du style.
    fn style_line(&self) -> (Tone, String) {
        match &self.style {
            None => (Tone::Busy, READING.to_owned()),
            Some(Err(problem)) => (Tone::Bad, problem.clone()),
            Some(Ok(style)) => {
                let tone = match style.state {
                    StyleState::Absent => Tone::Quiet,
                    StyleState::Ours { current: true } => Tone::Good,
                    StyleState::Ours { current: false } | StyleState::Foreign => Tone::Warning,
                };
                (tone, style::describe(style.state, &style.file))
            }
        }
    }

    /// Ce qui manque encore pour qu'une phrase arrive jusqu'à Capri, miroir activé.
    fn missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.hook_state().is_some_and(|state| !state.status.is_installed()) {
            missing.push(mirror::NOT_PLUGGED);
        }
        if self.style_state().is_some_and(|state| !matches!(state, StyleState::Ours { .. })) {
            missing.push(mirror::NO_STYLE);
        }
        missing
    }

    /// Ce que dit la note du miroir : le dernier changement, puis ce qui manque encore.
    fn mirror_line(&self) -> Option<(Tone, String)> {
        if let Some(Err(problem)) = &self.mirror {
            return Some((Tone::Bad, problem.clone()));
        }
        let missing = if self.mirror_on() == Some(true) { self.missing() } else { Vec::new() };
        let mut parts: Vec<String> = Vec::new();
        let mut tone = Tone::Warning;
        if let Some((saved, text)) = &self.mirror_note {
            parts.push(text.clone());
            if missing.is_empty() || matches!(saved, Tone::Bad | Tone::Busy) {
                tone = *saved;
            }
        }
        parts.extend(missing.iter().map(|text| (*text).to_owned()));
        (!parts.is_empty()).then(|| (tone, parts.join(" ")))
    }
}

/// Une note, si elle a quelque chose à dire.
fn note(id: &str, text: Option<&(Tone, String)>) -> Option<Row> {
    text.map(|(tone, text)| Row::new(id, "", Control::Note { text: text.clone(), tone: *tone }))
}

impl Rubric for ClaudeRubric {
    fn id(&self) -> &'static str {
        ID
    }

    fn title(&self) -> &'static str {
        "Claude Code"
    }

    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();

        // le branchement
        let (tone, text) = self.hook_line();
        rows.push(Row::new(HOOK, "Branchement", Control::Note { text, tone }));
        let state = self.hook_state();
        let installed = state.is_some_and(|state| state.status.is_installed());
        let to_redo = state.is_some_and(|state| {
            !state.status.is_installed() || !state.status.is_complete() || state.missing_binary
        });
        let idle = !matches!(self.hook_note, Some((Tone::Busy, _)));
        rows.push(Row::new(
            HOOK_ACTIONS,
            "",
            Control::Buttons(vec![
                Button::new(PLUG, "Brancher", idle && to_redo),
                Button::new(UNPLUG, "Débrancher", idle && installed),
            ]),
        ));
        rows.extend(note(HOOK_NOTE, self.hook_note.as_ref()));

        // le style
        rows.push(Row::new(STYLE_ABOUT, "Style « Capri »", Control::Text { text: style::ABOUT.to_owned() }));
        let (tone, text) = self.style_line();
        rows.push(Row::new(STYLE, "", Control::Note { text, tone }));
        let style = self.style_state();
        let idle = !matches!(self.style_note, Some((Tone::Busy, _)));
        rows.push(Row::new(
            STYLE_ACTIONS,
            "",
            Control::Buttons(vec![
                Button::new(
                    INSTALL_STYLE,
                    "Installer le style Capri",
                    idle && matches!(style, Some(StyleState::Absent | StyleState::Ours { current: false })),
                ),
                Button::new(REMOVE_STYLE, "Retirer", idle && matches!(style, Some(StyleState::Ours { .. }))),
            ]),
        ));
        if matches!(style, Some(StyleState::Ours { .. })) {
            rows.push(Row::new(STYLE_HOW, "", Control::Text { text: style::HOW_TO_CHOOSE.to_owned() }));
        }
        rows.extend(note(STYLE_NOTE, self.style_note.as_ref()));

        // le miroir
        let idle = !matches!(self.mirror_note, Some((Tone::Busy, _)));
        rows.push(Row::new(
            MIRROR,
            "Miroir",
            Control::Check {
                title: "Capri dit la ligne que Claude écrit pour elle".to_owned(),
                checked: self.mirror_on() == Some(true),
                enabled: idle && self.mirror_on().is_some(),
            },
        ));
        rows.push(Row::new(MIRROR_ABOUT, "", Control::Text { text: mirror::ABOUT.to_owned() }));
        if let Some(line) = self.mirror_line() {
            rows.extend(note(MIRROR_NOTE, Some(&line)));
        }
        rows
    }

    fn open(&mut self) -> Vec<Effect> {
        vec![Effect::LoadClaude]
    }

    fn event(&mut self, event: FormEvent) -> Vec<Effect> {
        match event {
            FormEvent::Pressed { id, .. } if id == PLUG || id == UNPLUG => {
                let busy = matches!(self.hook_note, Some((Tone::Busy, _)));
                if busy || self.hook_state().is_none() {
                    return Vec::new();
                }
                self.hook_note = Some((Tone::Busy, BUSY.to_owned()));
                vec![Effect::ChangeHook { plug: id == PLUG }]
            }
            FormEvent::Pressed { id, .. } if id == INSTALL_STYLE || id == REMOVE_STYLE => {
                let busy = matches!(self.style_note, Some((Tone::Busy, _)));
                // jamais sur un fichier qui n'est pas le nôtre, même si le bouton était resté allumé
                if busy || matches!(self.style_state(), None | Some(StyleState::Foreign)) {
                    return Vec::new();
                }
                self.style_note = Some((Tone::Busy, BUSY.to_owned()));
                vec![if id == INSTALL_STYLE { Effect::InstallStyle } else { Effect::RemoveStyle }]
            }
            FormEvent::Toggled { id, checked } if id == MIRROR => {
                let busy = matches!(self.mirror_note, Some((Tone::Busy, _)));
                if busy || self.mirror_on().is_none_or(|on| on == checked) {
                    return Vec::new();
                }
                self.mirror_note = Some((Tone::Busy, BUSY.to_owned()));
                vec![Effect::SaveMirror(checked)]
            }
            _ => Vec::new(),
        }
    }

    fn done(&mut self, outcome: Outcome) -> Vec<Effect> {
        match outcome {
            Outcome::ClaudeLoaded { hook, mirror, style } => {
                self.hook = Some(hook);
                self.mirror = Some(mirror);
                self.style = Some(style);
            }
            Outcome::HookChanged { plug, result } => match result {
                Ok(HookChange { state, changed, backup }) => {
                    let mut text = match (plug, changed) {
                        (true, true) => {
                            "Claude Code est branché. Les sessions ouvertes le prennent en compte toutes \
                                         seules ; /hooks, dans Claude Code, liste les entrées capri-hook."
                                .to_owned()
                        }
                        (true, false) => "Rien à faire : Claude Code est déjà branché.".to_owned(),
                        (false, true) => "Claude Code est débranché.".to_owned(),
                        (false, false) => "Rien à faire : Claude Code n'était pas branché.".to_owned(),
                    };
                    if let Some(backup) = backup {
                        text.push_str(&format!(" L'original est sauvegardé à côté : {backup}."));
                    }
                    self.hook = Some(Ok(state));
                    self.hook_note = Some((Tone::Good, text));
                }
                Err(problem) => self.hook_note = Some((Tone::Bad, problem)),
            },
            Outcome::MirrorSaved(Ok(enabled)) => {
                self.mirror = Some(Ok(enabled));
                let text = if enabled { mirror::TURNED_ON } else { mirror::TURNED_OFF };
                self.mirror_note = Some((Tone::Good, text.to_owned()));
            }
            Outcome::MirrorSaved(Err(problem)) => self.mirror_note = Some((Tone::Bad, problem)),
            Outcome::StyleInstalled(Ok((found, done))) => {
                self.style_note = Some((Tone::Good, style::installed(done, &found.file)));
                self.style = Some(Ok(found));
            }
            Outcome::StyleRemoved(Ok((found, done))) => {
                self.style_note = Some((Tone::Good, style::removed(done, &found.file)));
                self.style = Some(Ok(found));
            }
            Outcome::StyleInstalled(Err(problem)) | Outcome::StyleRemoved(Err(problem)) => {
                self.style_note = Some((Tone::Bad, problem));
                // ce qu'il y a à la place a pu changer : on le relit
                return vec![Effect::LoadClaude];
            }
            _ => {}
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use capri_claude::settings::{EVENTS, Status};
    use capri_claude::style::{Installed, Removed};

    use super::*;

    const SETTINGS: &str = "/Users/moi/.claude/settings.json";
    const STYLE_FILE: &str = "/Users/moi/.claude/output-styles/capri.md";

    fn hook(installed: bool) -> HookState {
        let events = if installed { EVENTS.iter().map(|e| (*e).to_owned()).collect() } else { Vec::new() };
        let commands = if installed { vec!["/Applications/Capri/capri-hook".to_owned()] } else { Vec::new() };
        HookState {
            file: SETTINGS.to_owned(),
            status: Status { events, commands, all_hooks_disabled: false },
            missing_binary: false,
        }
    }

    fn style_file(state: StyleState) -> StyleFile {
        StyleFile { file: STYLE_FILE.to_owned(), state }
    }

    fn opened(hook_state: HookState, mirror: bool, style: StyleState) -> ClaudeRubric {
        let mut rubric = ClaudeRubric::new();
        assert!(matches!(rubric.open().as_slice(), [Effect::LoadClaude]));
        rubric.done(Outcome::ClaudeLoaded {
            hook: Ok(hook_state),
            mirror: Ok(mirror),
            style: Ok(style_file(style)),
        });
        rubric
    }

    fn control(rubric: &ClaudeRubric, id: &str) -> Option<Control> {
        rubric.rows().into_iter().find(|row| row.id == id).map(|row| row.control)
    }

    fn text(rubric: &ClaudeRubric, id: &str) -> String {
        match control(rubric, id) {
            Some(Control::Note { text, .. } | Control::Text { text }) => text,
            other => panic!("{id} : {other:?}"),
        }
    }

    fn buttons(rubric: &ClaudeRubric, id: &str) -> Vec<(String, bool)> {
        match control(rubric, id) {
            Some(Control::Buttons(buttons)) => buttons.into_iter().map(|b| (b.title, b.enabled)).collect(),
            other => panic!("{id} : {other:?}"),
        }
    }

    fn press(rubric: &mut ClaudeRubric, id: &str) -> Vec<Effect> {
        rubric.event(FormEvent::Pressed { id: id.to_owned(), secret: None })
    }

    #[test]
    fn before_reading_nothing_can_be_done() {
        let mut rubric = ClaudeRubric::new();
        assert_eq!(text(&rubric, HOOK), READING);
        assert_eq!(
            buttons(&rubric, HOOK_ACTIONS),
            [("Brancher".into(), false), ("Débrancher".into(), false)]
        );
        assert!(matches!(
            control(&rubric, MIRROR),
            Some(Control::Check { checked: false, enabled: false, .. })
        ));
        assert!(press(&mut rubric, PLUG).is_empty() && press(&mut rubric, INSTALL_STYLE).is_empty());
        assert!(rubric.event(FormEvent::Toggled { id: MIRROR.to_owned(), checked: true }).is_empty());
    }

    #[test]
    fn plugging_in_goes_through_the_same_effect_as_capri_hook_install() {
        let mut rubric = opened(hook(false), false, StyleState::Absent);
        assert!(
            text(&rubric, HOOK)
                .starts_with("Claude Code n'est pas branché sur Capri. Le bandeau ne suit pas")
        );
        assert!(text(&rubric, HOOK).ends_with(&format!("Réglages de Claude Code : {SETTINGS}.")));
        assert_eq!(buttons(&rubric, HOOK_ACTIONS), [("Brancher".into(), true), ("Débrancher".into(), false)]);

        assert!(matches!(press(&mut rubric, PLUG).as_slice(), [Effect::ChangeHook { plug: true }]));
        assert!(press(&mut rubric, PLUG).is_empty(), "pas deux fois pendant que ça tourne");
        assert_eq!(
            buttons(&rubric, HOOK_ACTIONS),
            [("Brancher".into(), false), ("Débrancher".into(), false)]
        );
        rubric.done(Outcome::HookChanged {
            plug: true,
            result: Ok(HookChange {
                state: hook(true),
                changed: true,
                backup: Some("/Users/moi/.claude/settings.json.capri-backup-20261110-150000".into()),
            }),
        });
        assert!(text(&rubric, HOOK).starts_with("Claude Code est branché sur Capri (12 événements, tous"));
        assert!(text(&rubric, HOOK_NOTE).starts_with("Claude Code est branché. Les sessions ouvertes"));
        assert!(
            text(&rubric, HOOK_NOTE).ends_with(
                "sauvegardé à côté : /Users/moi/.claude/settings.json.capri-backup-20261110-150000."
            )
        );
        assert_eq!(buttons(&rubric, HOOK_ACTIONS), [("Brancher".into(), false), ("Débrancher".into(), true)]);

        assert!(matches!(press(&mut rubric, UNPLUG).as_slice(), [Effect::ChangeHook { plug: false }]));
        rubric.done(Outcome::HookChanged {
            plug: false,
            result: Err("le fichier n'est pas dans la forme que Claude Code écrit".into()),
        });
        assert!(matches!(control(&rubric, HOOK_NOTE), Some(Control::Note { tone: Tone::Bad, .. })));
        assert_eq!(buttons(&rubric, HOOK_ACTIONS)[1], ("Débrancher".into(), true), "on peut réessayer");
    }

    #[test]
    fn a_hook_pointing_at_a_missing_binary_or_disabled_hooks_are_said() {
        let mut missing = hook(true);
        missing.missing_binary = true;
        let rubric = opened(missing, false, StyleState::Absent);
        assert!(matches!(control(&rubric, HOOK), Some(Control::Note { tone: Tone::Warning, .. })));
        assert!(text(&rubric, HOOK).contains("capri-hook qu'il appelle est introuvable"));
        assert_eq!(buttons(&rubric, HOOK_ACTIONS), [("Brancher".into(), true), ("Débrancher".into(), true)]);

        let mut disabled = hook(true);
        disabled.status.all_hooks_disabled = true;
        let rubric = opened(disabled, false, StyleState::Absent);
        assert!(text(&rubric, HOOK).contains(ALL_HOOKS_DISABLED));

        let mut rubric = ClaudeRubric::new();
        rubric.done(Outcome::ClaudeLoaded {
            hook: Err("settings.json n'est pas du JSON valide".into()),
            mirror: Ok(false),
            style: Ok(style_file(StyleState::Absent)),
        });
        assert!(matches!(control(&rubric, HOOK), Some(Control::Note { tone: Tone::Bad, .. })));
        assert!(press(&mut rubric, PLUG).is_empty(), "rien à brancher dans un fichier illisible");
    }

    #[test]
    fn the_style_is_installed_on_demand_and_never_over_someone_else_s_file() {
        let mut rubric = opened(hook(true), false, StyleState::Absent);
        assert_eq!(text(&rubric, STYLE_ABOUT), style::ABOUT);
        assert_eq!(text(&rubric, STYLE), "Le style Capri n'est pas installé dans Claude Code.");
        assert_eq!(control(&rubric, STYLE_HOW), None, "rien à choisir tant qu'il n'est pas là");
        assert_eq!(
            buttons(&rubric, STYLE_ACTIONS),
            [("Installer le style Capri".into(), true), ("Retirer".into(), false)]
        );

        assert!(matches!(press(&mut rubric, INSTALL_STYLE).as_slice(), [Effect::InstallStyle]));
        rubric.done(Outcome::StyleInstalled(Ok((
            style_file(StyleState::Ours { current: true }),
            Installed::Written,
        ))));
        assert_eq!(text(&rubric, STYLE_NOTE), format!("Le style Capri est installé ({STYLE_FILE})."));
        assert_eq!(text(&rubric, STYLE_HOW), style::HOW_TO_CHOOSE, "et ce qu'il faut taper dans Claude Code");
        assert!(text(&rubric, STYLE_HOW).contains("/output-style Capri"));
        assert_eq!(
            buttons(&rubric, STYLE_ACTIONS),
            [("Installer le style Capri".into(), false), ("Retirer".into(), true)]
        );

        assert!(matches!(press(&mut rubric, REMOVE_STYLE).as_slice(), [Effect::RemoveStyle]));
        rubric.done(Outcome::StyleRemoved(Ok((style_file(StyleState::Absent), Removed::Removed))));
        assert!(text(&rubric, STYLE_NOTE).starts_with("Le style Capri est retiré"));

        // le fichier de quelqu'un d'autre : les deux boutons sont éteints, et rien ne part
        let mut rubric = opened(hook(true), false, StyleState::Foreign);
        assert!(text(&rubric, STYLE).contains("n'a pas été écrit par Capri : elle n'y touche pas"));
        assert_eq!(
            buttons(&rubric, STYLE_ACTIONS),
            [("Installer le style Capri".into(), false), ("Retirer".into(), false)]
        );
        assert!(press(&mut rubric, INSTALL_STYLE).is_empty() && press(&mut rubric, REMOVE_STYLE).is_empty());

        // un refus ou une panne se dit, et fait relire ce qu'il y a
        let mut rubric = opened(hook(true), false, StyleState::Absent);
        press(&mut rubric, INSTALL_STYLE);
        let effects = rubric.done(Outcome::StyleInstalled(Err("refusé".into())));
        assert!(matches!(effects.as_slice(), [Effect::LoadClaude]));
        assert!(matches!(control(&rubric, STYLE_NOTE), Some(Control::Note { tone: Tone::Bad, .. })));

        // une autre version du nôtre se met à jour
        let rubric = opened(hook(true), false, StyleState::Ours { current: false });
        assert_eq!(buttons(&rubric, STYLE_ACTIONS)[0], ("Installer le style Capri".into(), true));
    }

    #[test]
    fn the_mirror_is_off_until_checked_and_says_what_it_does_and_what_it_still_lacks() {
        let mut rubric = opened(hook(false), false, StyleState::Absent);
        assert!(matches!(
            control(&rubric, MIRROR),
            Some(Control::Check { checked: false, enabled: true, title }) if title == "Capri dit la ligne que Claude écrit pour elle"
        ));
        assert_eq!(text(&rubric, MIRROR_ABOUT), mirror::ABOUT, "ce qu'il fait, avant qu'on le coche");
        assert_eq!(control(&rubric, MIRROR_NOTE), None);
        assert!(rubric.event(FormEvent::Toggled { id: MIRROR.to_owned(), checked: false }).is_empty());

        let effects = rubric.event(FormEvent::Toggled { id: MIRROR.to_owned(), checked: true });
        assert!(matches!(effects.as_slice(), [Effect::SaveMirror(true)]));
        rubric.done(Outcome::MirrorSaved(Ok(true)));
        assert!(matches!(control(&rubric, MIRROR), Some(Control::Check { checked: true, .. })));
        let said = text(&rubric, MIRROR_NOTE);
        assert!(said.starts_with(mirror::TURNED_ON), "{said}");
        assert!(said.contains(mirror::NOT_PLUGGED) && said.contains(mirror::NO_STYLE), "{said}");
        assert!(matches!(control(&rubric, MIRROR_NOTE), Some(Control::Note { tone: Tone::Warning, .. })));

        // tout est en place : la note ne dit plus que ce qui vient de se passer
        let mut rubric = opened(hook(true), true, StyleState::Ours { current: true });
        assert_eq!(control(&rubric, MIRROR_NOTE), None);
        let effects = rubric.event(FormEvent::Toggled { id: MIRROR.to_owned(), checked: false });
        assert!(matches!(effects.as_slice(), [Effect::SaveMirror(false)]));
        rubric.done(Outcome::MirrorSaved(Ok(false)));
        assert_eq!(text(&rubric, MIRROR_NOTE), mirror::TURNED_OFF);
        assert!(matches!(control(&rubric, MIRROR_NOTE), Some(Control::Note { tone: Tone::Good, .. })));

        // un réglage illisible n'est pas écrasé
        let mut rubric = ClaudeRubric::new();
        rubric.done(Outcome::ClaudeLoaded {
            hook: Ok(hook(true)),
            mirror: Err("claude-code.json : il compte pour désactivé".into()),
            style: Ok(style_file(StyleState::Absent)),
        });
        assert!(matches!(
            control(&rubric, MIRROR),
            Some(Control::Check { checked: false, enabled: false, .. })
        ));
        assert!(matches!(control(&rubric, MIRROR_NOTE), Some(Control::Note { tone: Tone::Bad, .. })));
        assert!(rubric.event(FormEvent::Toggled { id: MIRROR.to_owned(), checked: true }).is_empty());
    }

    #[test]
    fn nothing_shown_here_sends_to_a_terminal() {
        let mut rubrics = vec![
            opened(hook(false), true, StyleState::Absent),
            opened(hook(true), true, StyleState::Ours { current: true }),
            opened(hook(true), false, StyleState::Foreign),
        ];
        let mut last = opened(hook(true), true, StyleState::Ours { current: false });
        press(&mut last, PLUG);
        last.done(Outcome::HookChanged {
            plug: true,
            result: Ok(HookChange { state: hook(true), changed: false, backup: None }),
        });
        rubrics.push(last);
        for rubric in &rubrics {
            for row in rubric.rows() {
                if let Control::Note { text, .. } | Control::Text { text } = &row.control {
                    assert!(!text.contains("capri ") && !text.contains("--"), "{} : {text}", row.id);
                }
            }
        }
    }
}
