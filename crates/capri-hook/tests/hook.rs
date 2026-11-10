//! Le binaire `capri-hook`, lancé pour de vrai comme le ferait Claude Code.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use capri_claude::protocol::{Event, MAX_LINE_BYTES, Message};
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

fn hook(endpoint: &Endpoint) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_capri-hook"));
    command
        .arg("--capri-managed")
        .env("CAPRI_SOCKET", endpoint.path())
        .env_remove("CAPRI_CHAT")
        .env_remove("CAPRI_HOOK_DEBUG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
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
    assert_eq!(Message::decode(&listener.next_line().unwrap()).unwrap().event, Event::TurnEnd);
}

#[test]
fn sessions_started_by_capri_herself_are_ignored() {
    let endpoint = endpoint("chat");
    let mut listener = Listener::bind(&endpoint, MAX_LINE_BYTES).unwrap();
    let mut own = hook(&endpoint);
    own.env("CAPRI_CHAT", "1");
    assert_invisible(&run(own, PRE_TOOL_USE.as_bytes()));
    assert_invisible(&run(hook(&endpoint), br#"{"session_id":"autre","hook_event_name":"Stop"}"#));
    assert_eq!(Message::decode(&listener.next_line().unwrap()).unwrap().session, "autre");
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
