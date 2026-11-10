//! `capri-hook` : appelé par les hooks de Claude Code, il transmet l'événement à Capri.
//!
//! Il lit le JSON que Claude Code écrit sur son entrée standard, en tire une ligne (voir
//! `capri_claude::protocol`) et l'envoie sur le canal local (voir `capri_ipc`).
//!
//! Règle d'or : ne jamais gêner la session qu'il observe.
//! - Rien sur la sortie standard : sur `SessionStart` et `UserPromptSubmit`, Claude Code
//!   l'ajouterait au contexte de Claude.
//! - Toujours le code de sortie 0 : tout autre code afficherait une erreur dans la session.
//! - Une durée de vie bornée, même si l'entrée ne se ferme jamais ou si Capri est figée.
//!
//! Pour comprendre ce qu'il fait : `CAPRI_HOOK_DEBUG=1` lui fait écrire une ligne sur la sortie
//! d'erreur, que Claude Code ne montre que dans son journal de débogage.

use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use capri_ipc::{Endpoint, IpcError};

/// Temps laissé à l'envoi quand Capri écoute.
const SEND_TIMEOUT: Duration = Duration::from_millis(150);
/// Passé ce délai, le processus s'arrête quoi qu'il soit en train d'attendre.
const LIFETIME: Duration = Duration::from_secs(2);
/// Définie dans les sessions Claude Code que Capri lance elle-même : leurs événements ne doivent
/// pas revenir dans son bandeau.
const OWN_SESSION_ENV: &str = "CAPRI_CHAT";
const DEBUG_ENV: &str = "CAPRI_HOOK_DEBUG";

fn main() {
    // Une panique écrirait sur la sortie d'erreur et sortirait avec le code 101.
    std::panic::set_hook(Box::new(|_| {}));
    std::thread::spawn(|| {
        std::thread::sleep(LIFETIME);
        std::process::exit(0);
    });

    let outcome = std::panic::catch_unwind(run);
    if std::env::var_os(DEBUG_ENV).is_some() {
        let report = match outcome {
            Ok(Ok(done)) => done.to_owned(),
            Ok(Err(problem)) => format!("échec : {problem}"),
            Err(_) => "panique".to_owned(),
        };
        let _ = writeln!(std::io::stderr(), "capri-hook : {report}");
    }
    std::process::exit(0);
}

fn run() -> Result<&'static str, String> {
    if std::env::var_os(OWN_SESSION_ENV).is_some() {
        return Ok("session lancée par Capri, ignorée");
    }
    let at_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
    let message = capri_claude::hook_input::read(std::io::stdin().lock(), at_ms)
        .map_err(|err| format!("entrée illisible ({err})"))?;
    let Some(message) = message else { return Ok("événement non suivi") };
    let line = message.encode().map_err(|err| err.to_string())?;
    let endpoint = Endpoint::for_current_user().map_err(|err| err.to_string())?;
    match capri_ipc::send_line(&endpoint, &line, SEND_TIMEOUT) {
        Ok(()) => Ok("événement transmis"),
        Err(IpcError::NotRunning) => Ok("Capri n'écoute pas"),
        Err(err) => Err(err.to_string()),
    }
}
