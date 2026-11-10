//! Le binaire `capri-hook`, lancé pour de vrai comme le ferait Claude Code.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use capri_claude::protocol::{Event, MAX_LINE_BYTES, Message, Said};
use capri_ipc::{Endpoint, Listener};

const PRE_TOOL_USE: &str = r#"{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/x/transcript.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": { "command": "npm test", "description": "Run test suite", "timeout": 120000 },
  "tool_use_id": "toolu_01ABC123"
}"#;

/// Un canal de test dans un dossier neuf, au chemin court.
fn endpoint(name: &str) -> Endpoint {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique =
        format!("capri-hook-{}-{}-{name}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
    let mut base = std::env::temp_dir();
    if cfg!(unix) && base.as_os_str().len() > 55 {
        base = PathBuf::from("/tmp");
    }
    let dir = base.join(unique);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Endpoint::at(dir.join(if cfg!(windows) { "hook-pipe.txt" } else { "hook.sock" }))
}

/// Le hook, avec le canal de ce test et son dossier de Capri à lui (celui du canal) : jamais
/// celui de l'utilisateur. Sans rien d'autre, le miroir y est désactivé.
fn hook(endpoint: &Endpoint) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_capri-hook"));
    command
        .arg("--capri-managed")
        .env("CAPRI_SOCKET", endpoint.path())
        .env("CAPRI_CONFIG_DIR", capri_dir(endpoint))
        .env_remove("CAPRI_HOOK_DEBUG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Le dossier de Capri du test : celui de son canal.
fn capri_dir(endpoint: &Endpoint) -> &Path {
    endpoint.path().parent().expect("le canal est dans un dossier")
}

/// Écrit le réglage du miroir dans le dossier de Capri du test.
fn set_mirror(endpoint: &Endpoint, contents: &[u8]) {
    let file = capri_dir(endpoint).join("claude-code.json");
    let _ = std::fs::remove_dir_all(&file);
    std::fs::write(file, contents).unwrap();
}

/// Un `Stop` comme celui de la documentation, avec cette réponse de Claude.
fn stop(answer: &str, extra: &str) -> String {
    format!(
        r#"{{"session_id":"abc123","cwd":"/home/user/my-project","permission_mode":"default",
            "hook_event_name":"Stop","stop_hook_active":false,"last_assistant_message":{},
            "background_tasks":[],"session_crons":[]{extra}}}"#,
        json_string(answer)
    )
}

/// `text` en chaîne JSON.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const ANSWER: &str =
    "J'ai relu secret.rs : la clé sk-123 traînait.\n\n[Capri] C'est réglé, on peut souffler.";

/// La phrase que porte la fin de tour reçue, s'il y en a une.
fn said_in(line: &str) -> Option<String> {
    match Message::decode(line).expect("une ligne du protocole").event {
        Event::TurnEnd { said } => said.map(Said::into_text),
        other => panic!("pas une fin de tour : {other:?}"),
    }
}

/// Lance le hook, lui écrit `input` puis ferme son entrée, comme Claude Code.
fn run(mut command: Command, input: &[u8]) -> Output {
    let mut child = command.spawn().expect("lancement de capri-hook");
    let mut stdin = child.stdin.take().unwrap();
    // le hook peut partir avant d'avoir tout lu : ce n'est pas une erreur du test
    let _ = stdin.write_all(input);
    drop(stdin);
    child.wait_with_output().unwrap()
}

/// Ce que Claude Code doit toujours voir : succès, et pas un octet sur la sortie standard.
fn assert_invisible(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "code de sortie");
    assert!(output.stdout.is_empty(), "sortie standard : {:?}", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn forwards_the_event_and_stays_invisible() {
    let endpoint = endpoint("envoi");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    let output = run(hook(&endpoint), PRE_TOOL_USE.as_bytes());
    assert_invisible(&output);
    assert!(output.stderr.is_empty(), "sortie d'erreur : {:?}", String::from_utf8_lossy(&output.stderr));

    let message = Message::decode(&listener.next_line().unwrap()).unwrap();
    assert_eq!(message.session, "abc123");
    assert_eq!(message.cwd.as_deref(), Some("/home/user/my-project"));
    assert!(message.at_ms > 1_700_000_000_000, "heure du hook : {}", message.at_ms);
    assert_eq!(
        message.event,
        Event::ToolStart {
            tool: "Bash".into(),
            id: Some("toolu_01ABC123".into()),
            summary: Some("Run test suite".into())
        }
    );
}

#[test]
fn without_capri_it_leaves_quietly_and_fast() {
    let endpoint = endpoint("absente");
    // Un premier lancement, non chronométré : macOS inspecte un binaire tout juste compilé la
    // première fois qu'il s'exécute, ce qui peut prendre plus d'une seconde et ne dit rien du hook.
    run(hook(&endpoint), PRE_TOOL_USE.as_bytes());
    let started = Instant::now();
    let output = run(hook(&endpoint), PRE_TOOL_USE.as_bytes());
    assert_invisible(&output);
    assert!(output.stderr.is_empty());
    assert!(started.elapsed() < Duration::from_millis(1500), "{:?}", started.elapsed());
}

#[test]
fn garbage_unknown_events_and_empty_input_are_not_errors() {
    let endpoint = endpoint("rebut");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    for input in [
        &b"pas du json"[..],
        b"",
        b"{}",
        br#"{"session_id":"s","hook_event_name":"PreCompact"}"#,
        &[0xff, 0xfe],
    ] {
        assert_invisible(&run(hook(&endpoint), input));
    }
    // rien de tout cela n'a été transmis : la prochaine ligne reçue est celle-ci
    assert_invisible(&run(hook(&endpoint), br#"{"session_id":"s","hook_event_name":"Stop"}"#));
    assert_eq!(Message::decode(&listener.next_line().unwrap()).unwrap().event, Event::TurnEnd { said: None });
}

#[test]
fn a_huge_payload_becomes_a_small_line() {
    let endpoint = endpoint("gros");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    let input = format!(
        r#"{{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Write","tool_input":{{"file_path":"/a/b/gros.txt","content":"{}"}}}}"#,
        "x".repeat(3_000_000)
    );
    assert_invisible(&run(hook(&endpoint), input.as_bytes()));
    let line = listener.next_line().unwrap();
    assert!(line.len() < 300, "{} octets", line.len());
    assert!(
        matches!(Message::decode(&line).unwrap().event, Event::ToolStart { summary: Some(s), .. } if s == "gros.txt")
    );
}

#[test]
fn it_never_outlives_its_welcome_even_if_stdin_stays_open() {
    let endpoint = endpoint("pendu");
    let mut child = hook(&endpoint).spawn().unwrap();
    let stdin = child.stdin.take().unwrap(); // gardée ouverte, sans rien écrire
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(started.elapsed() < Duration::from_secs(6), "capri-hook ne s'arrête pas");
        std::thread::sleep(Duration::from_millis(50));
    };
    drop(stdin);
    assert_eq!(status.code(), Some(0));
}

#[test]
fn debug_mode_explains_itself_on_stderr_only() {
    let endpoint = endpoint("debug");
    let mut debug = hook(&endpoint);
    debug.env("CAPRI_HOOK_DEBUG", "1");
    let output = run(debug, PRE_TOOL_USE.as_bytes());
    assert_invisible(&output);
    assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "capri-hook : Capri n'écoute pas");

    let mut debug = hook(&endpoint);
    debug.env("CAPRI_HOOK_DEBUG", "1");
    let output = run(debug, b"pas du json");
    assert_invisible(&output);
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("capri-hook : échec : entrée illisible"));
}

#[test]
fn the_mirror_is_off_by_default_and_then_nothing_more_is_sent() {
    let endpoint = endpoint("miroir-eteint");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    for setting in [None, Some(&b"{\n  \"mirror\": false\n}\n"[..])] {
        if let Some(setting) = setting {
            set_mirror(&endpoint, setting);
        }
        let output = run(hook(&endpoint), stop(ANSWER, "").as_bytes());
        assert_invisible(&output);
        assert!(output.stderr.is_empty());
        let line = listener.next_line().unwrap();
        assert_eq!(said_in(&line), None);
        for absent in ["souffler", "secret", "sk-123", "said", "Capri"] {
            assert!(!line.contains(absent), "{absent} dans {line}");
        }
        // la même ligne qu'avant le miroir, à l'heure près
        let message = Message::decode(&line).unwrap();
        assert_eq!(
            line,
            format!(
                r#"{{"v":1,"session":"abc123","cwd":"/home/user/my-project","at_ms":{},"kind":"turn_end"}}"#,
                message.at_ms
            )
        );
    }
}

#[test]
fn with_the_mirror_on_only_the_marked_line_of_the_main_session_is_sent() {
    let endpoint = endpoint("miroir");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    set_mirror(&endpoint, b"{\n  \"mirror\": true\n}\n");

    let output = run(hook(&endpoint), stop(ANSWER, "").as_bytes());
    assert_invisible(&output);
    assert!(output.stderr.is_empty());
    let line = listener.next_line().unwrap();
    assert_eq!(said_in(&line).as_deref(), Some("C'est réglé, on peut souffler."));
    assert!(!line.contains("secret") && !line.contains("sk-123"), "rien d'autre de la réponse : {line}");

    // pas de ligne marquée, un Stop dans un sous-agent, un SubagentStop : rien de plus qu'avant
    assert_invisible(&run(hook(&endpoint), stop("Fini, sans rien pour elle.", "").as_bytes()));
    assert_eq!(said_in(&listener.next_line().unwrap()), None);
    assert_invisible(&run(hook(&endpoint), stop(ANSWER, r#","agent_id":"a1""#).as_bytes()));
    assert_eq!(said_in(&listener.next_line().unwrap()), None);
    let subagent = br#"{"session_id":"s","hook_event_name":"SubagentStop","agent_id":"a1","agent_type":"Explore","last_assistant_message":"[Capri] Pas moi."}"#;
    assert_invisible(&run(hook(&endpoint), subagent));
    let line = listener.next_line().unwrap();
    assert!(matches!(Message::decode(&line).unwrap().event, Event::AgentEnd { .. }));
    assert!(!line.contains("Pas moi"));

    // le reste ne change pas
    assert_invisible(&run(hook(&endpoint), PRE_TOOL_USE.as_bytes()));
    assert!(matches!(
        Message::decode(&listener.next_line().unwrap()).unwrap().event,
        Event::ToolStart { .. }
    ));
}

#[test]
fn an_unreadable_mirror_setting_counts_as_off() {
    let endpoint = endpoint("miroir-illisible");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    let too_big = format!("{{\"mirror\":true,\"bourrage\":\"{}\"}}", "x".repeat(5_000));
    for setting in [
        &b"{ cass\xc3\xa9"[..],
        b"{\"mirror\":\"oui\"}",
        b"[true]",
        b"\xff\xfe{\"mirror\":true}",
        b"",
        too_big.as_bytes(),
    ] {
        set_mirror(&endpoint, setting);
        let output = run(hook(&endpoint), stop(ANSWER, "").as_bytes());
        assert_invisible(&output);
        assert!(output.stderr.is_empty());
        assert_eq!(said_in(&listener.next_line().unwrap()), None, "{}", String::from_utf8_lossy(setting));
    }

    // un dossier à la place du fichier
    let file = capri_dir(&endpoint).join("claude-code.json");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert_invisible(&run(hook(&endpoint), stop(ANSWER, "").as_bytes()));
    assert_eq!(said_in(&listener.next_line().unwrap()), None);
    std::fs::remove_dir(&file).unwrap();

    // un tube nommé à la place du fichier : surtout ne pas l'ouvrir, la lecture bloquerait
    #[cfg(unix)]
    {
        let made = Command::new("mkfifo").arg(&file).status().expect("mkfifo");
        assert!(made.success());
        let started = Instant::now();
        assert_invisible(&run(hook(&endpoint), stop(ANSWER, "").as_bytes()));
        assert_eq!(said_in(&listener.next_line().unwrap()), None);
        assert!(started.elapsed() < Duration::from_millis(1500), "{:?}", started.elapsed());
        std::fs::remove_file(&file).unwrap();
    }

    // un dossier de Capri qui n'existe pas
    let mut elsewhere = hook(&endpoint);
    elsewhere.env("CAPRI_CONFIG_DIR", capri_dir(&endpoint).join("nulle-part"));
    assert_invisible(&run(elsewhere, stop(ANSWER, "").as_bytes()));
    assert_eq!(said_in(&listener.next_line().unwrap()), None);
}

#[test]
fn a_huge_answer_with_the_mirror_on_stays_small_quick_and_invisible() {
    let endpoint = endpoint("miroir-gros");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    set_mirror(&endpoint, b"{\"mirror\":true}");
    let answer = format!(
        "{}\n```\n[Capri] Dans un bloc de code.\n```\n[Capri] {}",
        "Une réponse très longue. ".repeat(120_000),
        "😀 \"guillemets\" et \\ ".repeat(400)
    );
    assert!(answer.len() > 3_000_000);
    let input = stop(&answer, "");
    // un premier lancement, non chronométré (voir `without_capri_it_leaves_quietly_and_fast`)
    run(hook(&endpoint), PRE_TOOL_USE.as_bytes());
    let _ = listener.next_line().unwrap();

    let started = Instant::now();
    let output = run(hook(&endpoint), input.as_bytes());
    let took = started.elapsed();
    assert_invisible(&output);
    assert!(output.stderr.is_empty());
    assert!(took < Duration::from_millis(1500), "{took:?}");
    let line = listener.next_line().unwrap();
    assert!(line.len() < 2_500, "{} octets", line.len());
    let said = said_in(&line).expect("la ligne marquée, hors du bloc de code");
    assert!(said.starts_with("😀 \"guillemets\" et \\ 😀"), "{said}");
    assert_eq!(said.chars().count(), capri_claude::mirror::MAX_SAID_CHARS);
    assert!(said.len() <= capri_claude::mirror::MAX_SAID_BYTES);
}

#[test]
fn with_the_mirror_on_it_still_never_outlives_its_welcome() {
    let endpoint = endpoint("miroir-pendu");
    set_mirror(&endpoint, b"{\"mirror\":true}");
    let mut child = hook(&endpoint).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    // le début d'un Stop, puis plus rien, entrée gardée ouverte
    let _ =
        stdin.write_all(br#"{"hook_event_name":"Stop","last_assistant_message":"[Capri] Je n'ai pas fini"#);
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(started.elapsed() < Duration::from_secs(6), "capri-hook ne s'arrête pas");
        std::thread::sleep(Duration::from_millis(50));
    };
    drop(stdin);
    assert_eq!(status.code(), Some(0));
    let mut stdout = Vec::new();
    std::io::Read::read_to_end(&mut child.stdout.take().unwrap(), &mut stdout).unwrap();
    assert!(stdout.is_empty());
}

#[test]
fn debug_mode_never_quotes_the_line_for_capri() {
    let endpoint = endpoint("miroir-debug");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    set_mirror(&endpoint, b"{\"mirror\":true}");
    let mut debug = hook(&endpoint);
    debug.env("CAPRI_HOOK_DEBUG", "1");
    let output = run(debug, stop(ANSWER, "").as_bytes());
    assert_invisible(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "capri-hook : fin de tour transmise, avec une phrase pour Capri"
    );
    assert!(said_in(&listener.next_line().unwrap()).is_some());
}
