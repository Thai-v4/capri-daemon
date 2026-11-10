//! Sa voix côté fichiers, et la commande `capri voice …`.
//!
//! `voice.json`, dans le dossier de Capri, dit si elle parle à voix haute et avec quelle voix ;
//! son contenu est décidé par `capri_chat::voice`. Sans ce fichier, elle ne répond que par écrit.
//! Les voix sont celles du système : rien n'est téléchargé, rien n'est envoyé.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use capri_chat::voice::{VoiceInfo, VoiceSettings, pick, speakable};
use capri_platform::Voice;

use crate::brain_setup::{config_dir, read_text, write_file};

const VOICE_FILE: &str = "voice.json";
/// La langue dans laquelle Capri parle.
const LANGUAGE: &str = "fr";
const TEST_SENTENCE: &str = "Salut, c'est Capri. Tu m'entends bien ?";
/// Où télécharger de meilleures voix (libellés de macOS 27).
const BETTER_VOICES: &str = "Pour une voix plus naturelle : Réglages Système > Accessibilité > Lire et énoncer > \
                             Voix système, et télécharge une voix française de meilleure qualité.";

const USAGE: &str = "\
usage : capri voice status            si elle parle à voix haute, et avec quelle voix
        capri voice on | off          lui donner ou lui retirer la parole
        capri voice list              les voix françaises installées
        capri voice use <nom> | auto  choisir sa voix (auto : la meilleure installée)
        capri voice test [<phrase>]   l'écouter depuis le terminal";

/// Lit les réglages de la voix. Sans fichier : pas de voix.
pub fn load(directory: &Path) -> Result<VoiceSettings> {
    let file = directory.join(VOICE_FILE);
    let Some(text) = read_text(&file)? else { return Ok(VoiceSettings::default()) };
    VoiceSettings::from_json(&text).with_context(|| format!("réglages illisibles ({})", file.display()))
}

pub fn save(directory: &Path, settings: &VoiceSettings) -> Result<PathBuf> {
    write_file(directory, VOICE_FILE, &settings.to_json())
}

/// Les voix installées sur le système, dans le vocabulaire de `capri-chat`.
pub fn installed() -> Vec<VoiceInfo> {
    Voice::installed()
        .into_iter()
        .map(|voice| VoiceInfo {
            id: voice.id,
            name: voice.name,
            language: voice.language,
            quality: voice.quality,
            female: voice.female,
        })
        .collect()
}

/// L'identifiant de la voix à utiliser d'après ces réglages, si une voix convient.
pub fn chosen_voice(settings: &VoiceSettings) -> Option<String> {
    pick(&installed(), LANGUAGE, settings.voice.as_deref()).map(|voice| voice.id.clone())
}

/// La commande `capri voice <action>`. Rend le code de sortie du processus.
pub fn run_cli(args: &[String]) -> i32 {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = match args.as_slice() {
        ["status"] => status(),
        ["on"] => switch(true),
        ["off"] => switch(false),
        ["list"] => list(),
        ["use", name] => choose(name),
        ["test", words @ ..] => test(&words.join(" ")),
        _ => {
            eprintln!("{USAGE}");
            return 2;
        }
    };
    match outcome {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("capri : {err:#}");
            1
        }
    }
}

fn status() -> Result<()> {
    let directory = config_dir()?;
    let settings = load(&directory)?;
    println!("réglages : {}", directory.join(VOICE_FILE).display());
    if Voice::new().is_none() {
        println!("Capri n'a pas encore de voix sur ce système.");
        return Ok(());
    }
    println!("voix     : {}", if settings.enabled { "activée" } else { "désactivée (capri voice on)" });
    let voices = installed();
    match pick(&voices, LANGUAGE, settings.voice.as_deref()) {
        Some(voice) => {
            let how = if settings.voice.is_some() { "choisie" } else { "choix automatique" };
            println!("avec     : {} ({how})", describe(voice));
            if voice.quality <= 1 {
                println!("{BETTER_VOICES}");
            }
        }
        None => println!("avec     : aucune voix française n'est installée. {BETTER_VOICES}"),
    }
    Ok(())
}

fn switch(enabled: bool) -> Result<()> {
    let directory = config_dir()?;
    if enabled && Voice::new().is_none() {
        bail!("Capri n'a pas encore de voix sur ce système");
    }
    let settings = VoiceSettings { enabled, ..load(&directory)? };
    save(&directory, &settings)?;
    if enabled {
        println!("Capri dira ses réponses à voix haute, dès la prochaine. Pour l'écouter : capri voice test");
    } else {
        println!("Capri ne répondra plus que par écrit.");
    }
    Ok(())
}

fn list() -> Result<()> {
    let settings = load(&config_dir()?)?;
    let voices = installed();
    let chosen = pick(&voices, LANGUAGE, settings.voice.as_deref()).map(|voice| voice.id.clone());
    let mut french: Vec<&VoiceInfo> =
        voices.iter().filter(|voice| pick(std::slice::from_ref(voice), LANGUAGE, None).is_some()).collect();
    french.sort_by(|a, b| (b.quality, &a.name, &a.language).cmp(&(a.quality, &b.name, &b.language)));
    if french.is_empty() {
        println!("Aucune voix française n'est installée. {BETTER_VOICES}");
        return Ok(());
    }
    for voice in french {
        let mark = if Some(&voice.id) == chosen.as_ref() { "→" } else { " " };
        println!("{mark} {}", describe(voice));
    }
    println!(
        "Pour en choisir une : capri voice use <nom>   (ou son identifiant, si deux voix portent le même nom)"
    );
    Ok(())
}

fn choose(name: &str) -> Result<()> {
    let directory = config_dir()?;
    let mut settings = load(&directory)?;
    if name == "auto" {
        settings.voice = None;
    } else {
        let voices = installed();
        let wanted = name.trim();
        let voice = voices
            .iter()
            .find(|voice| voice.id == wanted)
            .or_else(|| {
                pick(&voices, LANGUAGE, Some(wanted)).filter(|voice| voice.name.eq_ignore_ascii_case(wanted))
            })
            .with_context(|| {
                format!("aucune voix installée ne s'appelle « {wanted} » (voir capri voice list)")
            })?;
        settings.voice = Some(voice.id.clone());
    }
    save(&directory, &settings)?;
    match pick(&installed(), LANGUAGE, settings.voice.as_deref()) {
        Some(voice) => println!("Sa voix : {}. Pour l'écouter : capri voice test", describe(voice)),
        None => println!("Aucune voix française n'est installée. {BETTER_VOICES}"),
    }
    Ok(())
}

/// Dit une phrase avec la voix choisie, et attend qu'elle ait fini.
fn test(sentence: &str) -> Result<()> {
    let settings = load(&config_dir()?)?;
    let voice = Voice::new().context("Capri n'a pas encore de voix sur ce système")?;
    let voices = installed();
    let chosen = pick(&voices, LANGUAGE, settings.voice.as_deref());
    let sentence = if sentence.trim().is_empty() { TEST_SENTENCE } else { sentence.trim() };
    let spoken = speakable(sentence);
    if spoken.is_empty() {
        bail!("il n'y a rien à prononcer dans « {sentence} »");
    }
    match chosen {
        Some(chosen) => println!("{} dit : {spoken}", describe(chosen)),
        None => println!("la voix par défaut du système dit : {spoken}"),
    }
    voice.say(&spoken, chosen.map(|voice| voice.id.as_str()));
    if !voice.wait(Duration::from_secs(60)) {
        voice.stop();
        bail!("la voix parlait encore au bout d'une minute : arrêtée");
    }
    if !settings.enabled {
        println!("(sa voix est désactivée dans Capri : capri voice on)");
    }
    Ok(())
}

fn describe(voice: &VoiceInfo) -> String {
    let quality = match voice.quality {
        3.. => "qualité supérieure",
        2 => "qualité améliorée",
        _ => "qualité de base",
    };
    let gender = if voice.female { "voix de femme" } else { "autre voix" };
    format!("{} [{}, {gender}, {quality}] {}", voice.name, voice.language, voice.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_settings_are_saved_and_read_back() {
        let directory = std::env::temp_dir().join(format!("capri-test-voix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(
            load(&directory).expect("lisible"),
            VoiceSettings::default(),
            "sans fichier : pas de voix"
        );

        let settings = VoiceSettings { enabled: true, voice: Some("une.voix".to_owned()) };
        let file = save(&directory, &settings).expect("écrit");
        assert_eq!(file, directory.join("voice.json"));
        assert_eq!(load(&directory).expect("lisible"), settings);

        std::fs::write(&file, "{ cassé").expect("écrit");
        let error = format!("{:#}", load(&directory).expect_err("illisible"));
        assert!(error.contains("réglages illisibles") && error.contains("voice.json"), "{error}");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_voice_is_described_in_one_line() {
        let voice = VoiceInfo {
            id: "com.exemple.voix".to_owned(),
            name: "Amélie".to_owned(),
            language: "fr-CA".to_owned(),
            quality: 1,
            female: true,
        };
        assert_eq!(describe(&voice), "Amélie [fr-CA, voix de femme, qualité de base] com.exemple.voix");
    }
}
