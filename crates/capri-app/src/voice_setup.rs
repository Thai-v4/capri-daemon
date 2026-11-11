//! Sa voix côté fichiers, et la commande `capri voice …`.
//!
//! `voice.json`, dans le dossier de Capri, dit si elle parle à voix haute, avec quelle voix, quel
//! effet, à quelle hauteur et quel débit ; son contenu est décidé par `capri_chat::voice`, comme
//! pour la fenêtre de réglages. Sans ce fichier, elle ne répond que par écrit. Les voix sont celles
//! du système et les effets sont appliqués sur la machine : rien n'est téléchargé, rien n'est
//! envoyé.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use capri_chat::voice::{
    BETTER_VOICES, Intensity, LANGUAGE, Pitch, Rate, TEST_SENTENCE, VOICE_OFF, VOICE_ON, VoiceEffect,
    VoiceInfo, VoiceSettings, describe, describe_style, french, pick, speakable,
};
use capri_platform::{Delivery, Voice};

use crate::brain_setup::{config_dir, read_text, write_file};

const VOICE_FILE: &str = "voice.json";

const USAGE: &str = "\
usage : capri voice status            si elle parle à voix haute, et avec quelle voix
        capri voice on | off          lui donner ou lui retirer la parole
        capri voice list              les voix françaises installées
        capri voice use <nom> | auto  choisir sa voix (auto : la meilleure installée)
        capri voice effect <nom>      son effet : natural (aucun), construct, radio, hologram
        capri voice intensity <1…3>   l'intensité de l'effet : 1 légère, 2 moyenne, 3 forte
        capri voice pitch <-2…2>      sa hauteur : -2 plus grave, 0 normale, 2 plus aiguë
        capri voice rate <-2…2>       son débit : -2 lent, 0 normal, 2 rapide
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

/// Comment dire ses phrases d'après ces réglages : la voix choisie, sa hauteur, son débit et son
/// effet. Lit les voix installées.
pub fn delivery(settings: &VoiceSettings) -> Delivery {
    capri_chat::voice::delivery(settings, chosen_voice(settings))
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
        [what @ ("effect" | "intensity" | "pitch" | "rate"), value] => style(what, value),
        ["test", words @ ..] => test(&words.join(" ")),
        // en version de débogage : la vraie voix, avec et sans effets, exercée sans un bruit
        #[cfg(debug_assertions)]
        ["rehearse"] => return crate::voice_rehearsal::run(),
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
    let effect = settings.effect;
    match effect {
        VoiceEffect::Natural => println!("effet    : {} ({})", effect.name(), effect.summary()),
        _ => println!(
            "effet    : {} ({}), intensité {}",
            effect.name(),
            effect.summary(),
            settings.intensity.name()
        ),
    }
    println!("hauteur  : {}", settings.pitch.name());
    println!("débit    : {}", settings.rate.name());
    Ok(())
}

/// `capri voice effect | intensity | pitch | rate <valeur>` : le change dans `voice.json`.
fn style(what: &str, value: &str) -> Result<()> {
    let directory = config_dir()?;
    let (settings, said) = restyle(load(&directory)?, what, value)?;
    save(&directory, &settings)?;
    println!("{said} Dès sa prochaine phrase. Pour l'écouter : capri voice test");
    Ok(())
}

/// Ces réglages, avec `what` changé en `value`, et ce qu'il faut en dire. Les règles et les noms
/// sont ceux de la fenêtre de réglages (`capri_chat::voice`).
fn restyle(settings: VoiceSettings, what: &str, value: &str) -> Result<(VoiceSettings, String)> {
    match what {
        "effect" => {
            let effect = VoiceEffect::parse(value).with_context(|| {
                let known: Vec<String> = VoiceEffect::ALL
                    .iter()
                    .map(|effect| format!("{} ({})", effect.id(), effect.name()))
                    .collect();
                format!("effet inconnu « {} » : {}", value.trim(), known.join(", "))
            })?;
            Ok((
                VoiceSettings { effect, ..settings },
                format!("Son effet : {}, {}.", effect.name(), effect.summary()),
            ))
        }
        "intensity" => {
            let intensity = Intensity::parse(value).context("l'intensité va de 1 (légère) à 3 (forte)")?;
            let mut said = format!("Intensité de l'effet : {}.", intensity.name());
            if settings.effect == VoiceEffect::Natural {
                said.push_str(" Elle ne s'entend qu'avec un effet (capri voice effect <nom>).");
            }
            Ok((VoiceSettings { intensity, ..settings }, said))
        }
        "pitch" => {
            let pitch = Pitch::parse(value).context("la hauteur va de -2 (plus grave) à 2 (plus aiguë)")?;
            Ok((VoiceSettings { pitch, ..settings }, format!("Hauteur : {}.", pitch.name())))
        }
        "rate" => {
            let rate = Rate::parse(value).context("le débit va de -2 (lent) à 2 (rapide)")?;
            Ok((VoiceSettings { rate, ..settings }, format!("Débit : {}.", rate.name())))
        }
        _ => bail!("réglage inconnu « {what} »"),
    }
}

fn switch(enabled: bool) -> Result<()> {
    let directory = config_dir()?;
    if enabled && Voice::new().is_none() {
        bail!("Capri n'a pas encore de voix sur ce système");
    }
    let settings = VoiceSettings { enabled, ..load(&directory)? };
    save(&directory, &settings)?;
    if enabled {
        println!("{VOICE_ON} Pour l'écouter : capri voice test");
    } else {
        println!("{VOICE_OFF}");
    }
    Ok(())
}

fn list() -> Result<()> {
    let settings = load(&config_dir()?)?;
    let voices = installed();
    let chosen = pick(&voices, LANGUAGE, settings.voice.as_deref()).map(|voice| voice.id.clone());
    let french = french(&voices);
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
    println!("façon de parler : {}", describe_style(&settings));
    let delivery = capri_chat::voice::delivery(&settings, chosen.map(|voice| voice.id.clone()));
    voice.say(&spoken, &delivery);
    if !voice.wait(Duration::from_secs(60)) {
        voice.stop();
        bail!("la voix parlait encore au bout d'une minute : arrêtée");
    }
    if !settings.enabled {
        println!("(sa voix est désactivée dans Capri : capri voice on)");
    }
    Ok(())
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

        let settings =
            VoiceSettings { enabled: true, voice: Some("une.voix".to_owned()), ..VoiceSettings::default() };
        let file = save(&directory, &settings).expect("écrit");
        assert_eq!(file, directory.join("voice.json"));
        assert_eq!(load(&directory).expect("lisible"), settings);

        std::fs::write(&file, "{ cassé").expect("écrit");
        let error = format!("{:#}", load(&directory).expect_err("illisible"));
        assert!(error.contains("réglages illisibles") && error.contains("voice.json"), "{error}");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn the_effect_its_intensity_the_pitch_and_the_rate_are_changed_one_at_a_time() {
        let before =
            VoiceSettings { enabled: true, voice: Some("une.voix".to_owned()), ..VoiceSettings::default() };
        let (settings, said) = restyle(before.clone(), "effect", "Hologramme").expect("connu");
        assert_eq!(settings, VoiceSettings { effect: VoiceEffect::Hologram, ..before.clone() });
        assert!(said.starts_with("Son effet : Hologramme, "), "{said}");
        let (settings, said) = restyle(settings, "intensity", "3").expect("connu");
        assert_eq!((settings.intensity, said.as_str()), (Intensity::Strong, "Intensité de l'effet : forte."));
        let (settings, _) = restyle(settings, "pitch", "-1").expect("connu");
        let (settings, said) = restyle(settings, "rate", "vif").expect("connu");
        assert_eq!(said, "Débit : vif.");
        assert_eq!(
            settings,
            VoiceSettings {
                effect: VoiceEffect::Hologram,
                intensity: Intensity::Strong,
                pitch: Pitch::Low,
                rate: Rate::Fast,
                ..before.clone()
            },
            "le reste ne bouge pas"
        );
        // sans effet, l'intensité est notée, et l'on dit qu'elle ne s'entend pas
        let (_, said) = restyle(before.clone(), "intensity", "légère").expect("connu");
        assert!(said.contains("ne s'entend qu'avec un effet"), "{said}");
    }

    #[test]
    fn an_unknown_effect_or_a_level_out_of_range_is_refused_and_says_what_exists() {
        let before = VoiceSettings::default();
        let error = format!("{:#}", restyle(before.clone(), "effect", "chorale").expect_err("inconnu"));
        assert_eq!(
            error,
            "effet inconnu « chorale » : natural (Naturelle), construct (Construct), radio (Liaison radio), hologram \
             (Hologramme)"
        );
        for (what, value) in [("intensity", "0"), ("pitch", "3"), ("rate", "-3"), ("pitch", "beaucoup")] {
            assert!(restyle(before.clone(), what, value).is_err(), "{what} {value}");
        }
    }

    #[test]
    fn what_the_command_line_writes_is_what_the_window_reads() {
        let directory = std::env::temp_dir().join(format!("capri-test-voix-style-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let (settings, _) = restyle(load(&directory).expect("lisible"), "effect", "radio").expect("connu");
        save(&directory, &settings).expect("écrit");
        let read = load(&directory).expect("lisible");
        assert_eq!(read.effect, VoiceEffect::Radio);
        let text = std::fs::read_to_string(directory.join("voice.json")).expect("écrit");
        assert!(text.contains("\"effect\": \"radio\""), "{text}");
        let _ = std::fs::remove_dir_all(directory);
    }
}
