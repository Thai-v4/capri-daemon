//! Brancher et débrancher le hook de Capri dans le `settings.json` de Claude Code, de texte à
//! texte : ce module ne lit ni n'écrit aucun fichier.
//!
//! Ce fichier appartient à l'utilisateur et à d'autres outils. Trois garde-fous :
//! 1. on ne touche qu'à des fichiers que l'on sait réécrire à l'identique, à l'octet près (la
//!    forme que Claude Code écrit lui-même : JSON strict, indentation de deux espaces) ;
//! 2. on n'ajoute que nos propres groupes, à la fin de chaque événement, sans jamais modifier un
//!    groupe existant ;
//! 3. avant de rendre le résultat, on vérifie qu'une fois nos entrées retirées il est égal à
//!    l'original.
//!
//! Format des hooks : <https://code.claude.com/docs/en/hooks#configuration>.

use serde_json::{Map, Value, json};

use crate::text::file_name;

/// Argument qui signe nos entrées. Avec le nom du binaire, c'est ce qui permet de les retrouver.
pub const SENTINEL_ARG: &str = "--capri-managed";
/// Nom du binaire appelé par les hooks (sans extension).
pub const HOOK_BINARY: &str = "capri-hook";
/// Les événements de Claude Code que Capri suit.
pub const EVENTS: [&str; 12] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("le fichier n'est pas du JSON valide ({0})")]
    InvalidJson(String),
    #[error(
        "le fichier n'est pas dans la forme que Claude Code écrit (JSON indenté de deux espaces) : \
         le réécrire modifierait autre chose que les hooks de Capri"
    )]
    NotCanonical,
    #[error("« {0} » n'a pas la forme attendue")]
    UnexpectedShape(String),
    #[error("le résultat ne redonne pas l'original une fois les hooks de Capri retirés : rien n'est écrit")]
    Verification,
}

/// Ce que le fichier dit du branchement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// Événements qui portent un hook de Capri, dans l'ordre du fichier.
    pub events: Vec<String>,
    /// Commandes appelées par ces hooks, sans doublon.
    pub commands: Vec<String>,
    /// `disableAllHooks` est à `true` : aucun hook de ce fichier ne s'exécute.
    pub all_hooks_disabled: bool,
}

impl Status {
    /// Tous les événements suivis sont-ils branchés ?
    pub fn is_complete(&self) -> bool {
        EVENTS.iter().all(|event| self.events.iter().any(|e| e == event))
    }

    pub fn is_installed(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Le texte du fichier une fois le hook branché sur `hook_command` (chemin absolu du binaire).
/// Un texte vide vaut un fichier absent. Rebrancher remplace l'ancien branchement.
pub fn install(original: &str, hook_command: &str) -> Result<String, SettingsError> {
    let (mut root, layout) = read(original)?;
    let reference = normalized(&root)?;
    strip(&mut root)?;

    let hooks =
        object_mut(&mut root, "settings.json")?.entry("hooks").or_insert_with(|| Value::Object(Map::new()));
    let hooks = object_mut(hooks, "hooks")?;
    for event in EVENTS {
        let groups = hooks.entry(event).or_insert_with(|| Value::Array(Vec::new()));
        let groups =
            groups.as_array_mut().ok_or_else(|| SettingsError::UnexpectedShape(format!("hooks.{event}")))?;
        groups.push(our_group(hook_command));
    }

    if normalized(&root)? != reference {
        return Err(SettingsError::Verification);
    }
    Ok(layout.render(&root))
}

/// Le texte du fichier une fois le hook débranché. Sans hook de Capri, le texte est rendu tel quel.
pub fn uninstall(original: &str) -> Result<String, SettingsError> {
    let (mut root, layout) = read(original)?;
    let reference = normalized(&root)?;
    if strip(&mut root)? == 0 {
        return Ok(original.to_owned());
    }
    if normalized(&root)? != reference {
        return Err(SettingsError::Verification);
    }
    Ok(layout.render(&root))
}

/// Où en est le branchement. Contrairement à [`install`], n'exige pas un fichier canonique.
pub fn status(original: &str) -> Result<Status, SettingsError> {
    let root = parse(original)?;
    let mut status = Status {
        all_hooks_disabled: root.get("disableAllHooks").and_then(Value::as_bool).unwrap_or(false),
        ..Status::default()
    };
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else { return Ok(status) };
    for (event, groups) in hooks {
        let ours: Vec<&str> = groups
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|group| group.get("hooks")?.as_array())
            .flatten()
            .filter(|handler| is_ours(handler))
            .filter_map(|handler| handler.get("command")?.as_str())
            .collect();
        if !ours.is_empty() {
            status.events.push(event.clone());
        }
        for command in ours {
            if !status.commands.iter().any(|c| c == command) {
                status.commands.push(command.to_owned());
            }
        }
    }
    Ok(status)
}

/// Le bloc à ajouter à la main quand le fichier ne peut pas être réécrit automatiquement.
pub fn manual_snippet(hook_command: &str) -> String {
    let events: Map<String, Value> = EVENTS
        .iter()
        .map(|event| ((*event).to_owned(), Value::Array(vec![our_group(hook_command)])))
        .collect();
    let mut hooks = Map::new();
    hooks.insert("hooks".to_owned(), Value::Object(events));
    Layout { trailing_newline: true, crlf: false }.render(&Value::Object(hooks))
}

/// Nom de la copie de sauvegarde faite à l'instant `epoch_secs` (secondes depuis 1970, UTC).
pub fn backup_name(epoch_secs: u64) -> String {
    let (days, secs) = (epoch_secs / 86_400, epoch_secs % 86_400);
    // Date civile à partir d'un nombre de jours (algorithme de Howard Hinnant).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "settings.json.capri-backup-{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        secs / 3_600,
        secs % 3_600 / 60,
        secs % 60
    )
}

/// Préfixe commun à toutes nos sauvegardes, pour les retrouver.
pub const BACKUP_PREFIX: &str = "settings.json.capri-backup-";

/// Ce qu'il faut retenir de la mise en forme pour réécrire le fichier à l'identique.
#[derive(Clone, Copy)]
struct Layout {
    trailing_newline: bool,
    crlf: bool,
}

impl Layout {
    fn render(self, root: &Value) -> String {
        let mut text = serde_json::to_string_pretty(root).expect("une valeur JSON se sérialise toujours");
        if self.trailing_newline {
            text.push('\n');
        }
        if self.crlf { text.replace('\n', "\r\n") } else { text }
    }
}

fn parse(original: &str) -> Result<Value, SettingsError> {
    if original.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let root: Value =
        serde_json::from_str(original).map_err(|err| SettingsError::InvalidJson(err.to_string()))?;
    if !root.is_object() {
        return Err(SettingsError::UnexpectedShape("settings.json".to_owned()));
    }
    Ok(root)
}

/// Lit le fichier et refuse ceux que l'on ne saurait pas réécrire à l'identique.
fn read(original: &str) -> Result<(Value, Layout), SettingsError> {
    let root = parse(original)?;
    if original.trim().is_empty() {
        return Ok((root, Layout { trailing_newline: true, crlf: false }));
    }
    let layout = Layout { trailing_newline: original.ends_with('\n'), crlf: original.contains("\r\n") };
    if layout.render(&root) != original {
        return Err(SettingsError::NotCanonical);
    }
    Ok((root, layout))
}

fn object_mut<'a>(value: &'a mut Value, name: &str) -> Result<&'a mut Map<String, Value>, SettingsError> {
    value.as_object_mut().ok_or_else(|| SettingsError::UnexpectedShape(name.to_owned()))
}

fn our_group(hook_command: &str) -> Value {
    json!({
        "hooks": [{
            "type": "command",
            "command": hook_command,
            "args": [SENTINEL_ARG],
            "async": true
        }]
    })
}

/// Cette entrée est-elle la nôtre ? Il faut les deux signes : le binaire et l'argument.
fn is_ours(handler: &Value) -> bool {
    let binary = handler.get("command").and_then(Value::as_str).map(file_name).unwrap_or("");
    let binary = match binary.len().checked_sub(4) {
        Some(cut) if binary.is_char_boundary(cut) && binary[cut..].eq_ignore_ascii_case(".exe") => {
            &binary[..cut]
        }
        _ => binary,
    };
    handler.get("type").and_then(Value::as_str) == Some("command")
        && binary == HOOK_BINARY
        && handler
            .get("args")
            .and_then(Value::as_array)
            .is_some_and(|args| args.iter().any(|arg| arg.as_str() == Some(SENTINEL_ARG)))
}

/// Retire nos entrées, et les conteneurs que ce retrait a vidés (jamais ceux qui étaient déjà
/// vides). Rend le nombre d'entrées retirées.
fn strip(root: &mut Value) -> Result<usize, SettingsError> {
    let root = object_mut(root, "settings.json")?;
    let Some(hooks) = root.get_mut("hooks") else { return Ok(0) };
    let hooks = object_mut(hooks, "hooks")?;

    let mut removed = 0;
    let mut emptied_events = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        // une valeur qui n'est pas une liste n'est pas à nous : on n'y touche pas
        let Some(groups) = groups.as_array_mut() else { continue };
        let mut removed_here = 0;
        groups.retain_mut(|group| {
            let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else { return true };
            let before = handlers.len();
            handlers.retain(|handler| !is_ours(handler));
            removed_here += before - handlers.len();
            // un groupe ne disparaît que si c'est notre retrait qui l'a vidé
            !(handlers.is_empty() && before > 0)
        });
        if removed_here > 0 && groups.is_empty() {
            emptied_events.push(event.clone());
        }
        removed += removed_here;
    }
    for event in emptied_events {
        hooks.shift_remove(&event);
    }
    if removed > 0 && hooks.is_empty() {
        root.shift_remove("hooks");
    }
    Ok(removed)
}

/// Le fichier sans nos entrées, et sans les conteneurs de hooks vides : c'est la partie qui ne
/// nous appartient pas, et qui doit rester la même avant et après.
fn normalized(root: &Value) -> Result<Value, SettingsError> {
    let mut copy = root.clone();
    strip(&mut copy)?;
    if let Some(hooks) = copy.get_mut("hooks").and_then(Value::as_object_mut) {
        hooks.retain(|_, groups| groups.as_array().is_none_or(|groups| !groups.is_empty()));
        if hooks.is_empty()
            && let Some(root) = copy.as_object_mut()
        {
            root.shift_remove("hooks");
        }
    }
    Ok(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOOK: &str = "/Users/moi/capri/target/release/capri-hook";

    /// Un fichier comme on en trouve en vrai : d'autres réglages, et des hooks d'autres outils.
    const OTHER_TOOLS: &str = r#"{
  "model": "opus",
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "if [ -x \"$HOME/.outil/hook.sh\" ]; then \"$HOME/.outil/hook.sh\" start; fi",
            "timeout": 5
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "[ -n \"$OUTIL\" ] && outil pre || true",
            "timeout": 5,
            "statusMessage": "Vérification…"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "/usr/local/bin/formatter --check"
          }
        ]
      }
    ],
    "PreCompact": []
  },
  "statusLine": {
    "type": "command",
    "command": "~/.claude/statusline.sh"
  },
  "theme": "dark"
}
"#;

    /// Les lignes de `text`, sans la virgule finale que JSON impose quand on ajoute un voisin.
    fn lines(text: &str) -> Vec<&str> {
        text.lines().map(|line| line.trim_end_matches(',')).collect()
    }

    fn is_subsequence(needle: &[&str], haystack: &[&str]) -> bool {
        let mut rest = haystack.iter();
        needle.iter().all(|line| rest.any(|candidate| candidate == line))
    }

    #[test]
    fn installing_only_adds_lines_and_uninstalling_restores_the_file_byte_for_byte() {
        let installed = install(OTHER_TOOLS, HOOK).unwrap();
        assert_ne!(installed, OTHER_TOOLS);
        assert!(
            is_subsequence(&lines(OTHER_TOOLS), &lines(&installed)),
            "toutes les lignes d'origine sont encore là, dans l'ordre"
        );
        assert_eq!(uninstall(&installed).unwrap(), OTHER_TOOLS);
    }

    #[test]
    fn every_event_gets_its_own_group_after_the_existing_ones() {
        let installed: Value = serde_json::from_str(&install(OTHER_TOOLS, HOOK).unwrap()).unwrap();
        for event in EVENTS {
            let groups = installed["hooks"][event].as_array().unwrap_or_else(|| panic!("{event} absent"));
            let ours = groups.last().unwrap();
            assert_eq!(
                *ours,
                json!({"hooks": [{"type": "command", "command": HOOK, "args": ["--capri-managed"], "async": true}]}),
                "{event}"
            );
        }
        // les groupes des autres outils sont toujours devant, intacts
        let original: Value = serde_json::from_str(OTHER_TOOLS).unwrap();
        assert_eq!(installed["hooks"]["PreToolUse"][0], original["hooks"]["PreToolUse"][0]);
        assert_eq!(installed["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(installed["hooks"]["PreCompact"], json!([]), "un événement que Capri ne suit pas");
        // et l'ordre des clés du fichier n'a pas bougé
        let keys: Vec<&String> = installed.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "statusLine", "theme"]);
    }

    #[test]
    fn installing_twice_is_the_same_as_once_and_a_new_path_replaces_the_old() {
        let once = install(OTHER_TOOLS, HOOK).unwrap();
        assert_eq!(install(&once, HOOK).unwrap(), once);

        let moved = install(&once, "/Applications/Capri.app/Contents/MacOS/capri-hook").unwrap();
        assert!(!moved.contains(HOOK));
        assert_eq!(status(&moved).unwrap().commands, ["/Applications/Capri.app/Contents/MacOS/capri-hook"]);
        assert_eq!(uninstall(&moved).unwrap(), OTHER_TOOLS);
    }

    #[test]
    fn a_missing_or_empty_file_gets_just_our_hooks() {
        for empty in ["", "\n", "{}\n"] {
            let installed = install(empty, HOOK).unwrap();
            let value: Value = serde_json::from_str(&installed).unwrap();
            assert_eq!(value.as_object().unwrap().len(), 1, "seulement « hooks »");
            assert_eq!(value["hooks"].as_object().unwrap().len(), EVENTS.len());
            assert!(installed.ends_with("}\n"));
            assert_eq!(uninstall(&installed).unwrap(), "{}\n");
        }
        assert_eq!(uninstall("").unwrap(), "", "rien à débrancher : texte rendu tel quel");
        assert_eq!(uninstall(OTHER_TOOLS).unwrap(), OTHER_TOOLS);
    }

    #[test]
    fn keys_added_by_the_user_afterwards_keep_their_place() {
        let installed = install("{\n  \"theme\": \"dark\"\n}\n", HOOK).unwrap();
        // l'utilisateur ajoute un réglage après « hooks », puis débranche
        let edited = installed.replacen("\n}\n", ",\n  \"model\": \"opus\"\n}\n", 1);
        assert!(serde_json::from_str::<Value>(&edited).is_ok());
        assert_eq!(uninstall(&edited).unwrap(), "{\n  \"theme\": \"dark\",\n  \"model\": \"opus\"\n}\n");
    }

    #[test]
    fn only_our_own_entries_are_ever_removed() {
        // quelqu'un a fusionné notre entrée dans un groupe à lui, et un homonyme traîne
        let mixed = r#"{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/opt/autre/notify.sh"
          },
          {
            "type": "command",
            "command": "C:\\Capri\\capri-hook.exe",
            "args": [
              "--capri-managed"
            ],
            "async": true
          },
          {
            "type": "command",
            "command": "/usr/local/bin/capri-hook",
            "args": [
              "--autre-chose"
            ]
          },
          {
            "type": "command",
            "command": "/usr/local/bin/pas-capri-hook --capri-managed"
          }
        ]
      }
    ]
  }
}
"#;
        assert_eq!(status(mixed).unwrap().commands, ["C:\\Capri\\capri-hook.exe"]);
        let cleaned: Value = serde_json::from_str(&uninstall(mixed).unwrap()).unwrap();
        let left: Vec<&str> = cleaned["hooks"]["Stop"][0]["hooks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert_eq!(
            left,
            [
                "/opt/autre/notify.sh",
                "/usr/local/bin/capri-hook",
                "/usr/local/bin/pas-capri-hook --capri-managed"
            ]
        );
    }

    #[test]
    fn containers_that_were_already_empty_are_left_alone() {
        let with_empties = "{\n  \"hooks\": {\n    \"Stop\": [],\n    \"PreCompact\": []\n  }\n}\n";
        let installed = install(with_empties, HOOK).unwrap();
        let cleaned: Value = serde_json::from_str(&uninstall(&installed).unwrap()).unwrap();
        // « Stop » a reçu puis perdu notre groupe : il disparaît ; « PreCompact » n'a jamais été touché
        assert_eq!(cleaned, json!({"hooks": {"PreCompact": []}}));
    }

    #[test]
    fn files_we_could_not_rewrite_identically_are_refused() {
        let four_spaces = "{\n    \"theme\": \"dark\"\n}\n";
        let compact = "{\"theme\":\"dark\"}\n";
        let exotic_number = "{\n  \"x\": 1e5\n}\n";
        for text in [four_spaces, compact, exotic_number] {
            assert_eq!(install(text, HOOK), Err(SettingsError::NotCanonical), "{text}");
            assert_eq!(uninstall(text), Err(SettingsError::NotCanonical), "{text}");
            assert!(status(text).is_ok(), "lire l'état ne demande pas un fichier canonique");
        }
        for text in ["{\n  \"theme\": \"dark\",\n}\n", "{\n  // commentaire\n  \"a\": 1\n}\n", "{"] {
            assert!(matches!(install(text, HOOK), Err(SettingsError::InvalidJson(_))), "{text}");
            assert!(matches!(status(text), Err(SettingsError::InvalidJson(_))), "{text}");
        }
        assert_eq!(install("[]\n", HOOK), Err(SettingsError::UnexpectedShape("settings.json".into())));
        assert_eq!(
            install("{\n  \"hooks\": []\n}\n", HOOK),
            Err(SettingsError::UnexpectedShape("hooks".into()))
        );
        assert_eq!(
            install("{\n  \"hooks\": {\n    \"Stop\": {}\n  }\n}\n", HOOK),
            Err(SettingsError::UnexpectedShape("hooks.Stop".into()))
        );
    }

    #[test]
    fn windows_line_endings_and_a_missing_final_newline_are_preserved() {
        let crlf = "{\r\n  \"theme\": \"dark\"\r\n}\r\n";
        let installed = install(crlf, HOOK).unwrap();
        assert!(installed.contains("\r\n") && !installed.replace("\r\n", "").contains('\n'));
        assert_eq!(uninstall(&installed).unwrap(), crlf);

        let no_newline = "{\n  \"theme\": \"dark\"\n}";
        let installed = install(no_newline, HOOK).unwrap();
        assert!(installed.ends_with('}'));
        assert_eq!(uninstall(&installed).unwrap(), no_newline);
    }

    #[test]
    fn status_tells_what_is_plugged_in() {
        let nothing = status(OTHER_TOOLS).unwrap();
        assert!(!nothing.is_installed() && !nothing.is_complete() && !nothing.all_hooks_disabled);

        let installed = install(OTHER_TOOLS, HOOK).unwrap();
        let full = status(&installed).unwrap();
        assert!(full.is_installed() && full.is_complete());
        assert_eq!(full.commands, [HOOK]);
        assert_eq!(full.events.len(), EVENTS.len());

        let disabled = status("{\n  \"disableAllHooks\": true\n}\n").unwrap();
        assert!(disabled.all_hooks_disabled);
        let kept = install("{\n  \"disableAllHooks\": true\n}\n", HOOK).unwrap();
        assert!(kept.contains("\"disableAllHooks\": true"), "ce réglage ne nous appartient pas");
        assert_eq!(status("").unwrap(), Status::default());
    }

    #[test]
    fn the_manual_snippet_is_what_install_would_add() {
        let snippet: Value = serde_json::from_str(&manual_snippet(HOOK)).unwrap();
        let installed: Value = serde_json::from_str(&install("", HOOK).unwrap()).unwrap();
        assert_eq!(snippet, installed);
    }

    /// À lancer à la main sur son propre fichier, qui n'est que lu :
    /// `CAPRI_SETTINGS_FILE=~/.claude/settings.json cargo test -p capri-claude real_settings -- --ignored --nocapture`
    #[test]
    #[ignore = "lit le fichier désigné par CAPRI_SETTINGS_FILE"]
    fn real_settings_file_survives_install_then_uninstall() {
        let path =
            std::env::var("CAPRI_SETTINGS_FILE").expect("CAPRI_SETTINGS_FILE doit désigner un settings.json");
        let original = std::fs::read_to_string(&path).expect("fichier lisible");
        let installed = install(&original, HOOK).expect("branchement accepté");
        assert!(
            is_subsequence(&lines(&original), &lines(&installed)),
            "aucune ligne d'origine perdue ou déplacée"
        );
        assert_eq!(uninstall(&installed).expect("débranchement accepté"), original, "retour à l'octet près");
        let before = status(&original).unwrap();
        println!(
            "{path} : {} octets, {} après branchement (+{} lignes) ; hooks de Capri déjà présents : {}",
            original.len(),
            installed.len(),
            installed.lines().count() - original.lines().count(),
            before.is_installed()
        );
    }

    #[test]
    fn backups_are_named_after_the_utc_date() {
        assert_eq!(backup_name(0), "settings.json.capri-backup-19700101-000000");
        assert_eq!(backup_name(1_731_250_000), "settings.json.capri-backup-20241110-144640");
        assert_eq!(backup_name(1_709_251_199), "settings.json.capri-backup-20240229-235959"); // bissextile
        assert_eq!(backup_name(1_794_322_800), "settings.json.capri-backup-20261110-150000");
        assert!(backup_name(1).starts_with(BACKUP_PREFIX));
    }
}
