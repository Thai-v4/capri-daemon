//! `capri voice rehearse`, en version de débogage seulement : la vraie voix, avec et sans effets,
//! exercée sans un bruit.
//!
//! Chaque voix d'essai est muette (`Rehearsal::Silent` : les vrais chemins jusqu'à la sortie du
//! Mac, à volume nul) ou rendue en mémoire (`Rehearsal::Offline` : sans périphérique). Aucun
//! réglage de l'utilisateur n'est lu : seulement les voix installées sur le système, et les
//! préréglages de `capri_chat::voice`. Ce qu'on prouve ainsi : rien ne plante, les phrases sont
//! rendues et jouées dans l'ordre, la fin est vue, l'arrêt et la pause agissent, le chemin sans
//! effet reste direct et sans réglage, le repli se déclenche, et les effets changent bien le son.
//! Ce qu'on ne prouve pas : que c'est agréable à entendre.
//!
//! Rend 0 si tout est conforme, 1 sinon ; chaque vérification écrit une ligne.

use std::time::{Duration, Instant};

use capri_chat::voice::{
    Intensity, LANGUAGE, Pitch, Rate, TEST_SENTENCE, VoiceEffect, VoiceSettings, delivery, pick, speakable,
};
use capri_core::sound::{Effect, Room};
use capri_platform::{Breakage, Delivery, Rehearsal, Rehearsed, Voice};

use crate::voice_setup::installed;

/// Le temps laissé à chaque scène pour finir.
const LIMIT: Duration = Duration::from_secs(60);

/// Les vérifications, et combien ont échoué.
struct Checks {
    failed: usize,
}

impl Checks {
    fn check(&mut self, ok: bool, what: &str) {
        if ok {
            println!("ok     : {what}");
        } else {
            println!("ÉCHEC  : {what}");
            self.failed += 1;
        }
    }
}

pub fn run() -> i32 {
    if Voice::rehearsal(Rehearsal::Silent).is_none() {
        println!("Capri n'a pas encore de voix sur ce système : rien à répéter.");
        return 0;
    }
    let voices = installed();
    let Some(main) = pick(&voices, LANGUAGE, None).map(|voice| voice.id.clone()) else {
        println!("Aucune voix française installée : rien à répéter.");
        return 1;
    };
    println!("voix d'essai : {main}");
    let mut checks = Checks { failed: 0 };
    direct(&mut checks, &main);
    treated_in_order(&mut checks, &main);
    pause_and_resume(&mut checks, &main);
    stop(&mut checks, &main);
    other_voice_and_other_chain(&mut checks, &main, &voices);
    mixed_paths(&mut checks, &main);
    fallbacks(&mut checks, &main);
    offline(&mut checks, &main);
    if checks.failed == 0 {
        println!("répétition de la voix : tout est conforme");
        0
    } else {
        println!("répétition de la voix : {} vérification(s) en échec", checks.failed);
        1
    }
}

fn with(effect: VoiceEffect, intensity: Intensity, voice: &str) -> Delivery {
    let settings = VoiceSettings { effect, intensity, ..VoiceSettings::default() };
    delivery(&settings, Some(voice.to_owned()))
}

fn silent() -> Voice {
    Voice::rehearsal(Rehearsal::Silent).expect("une voix existe sur ce système")
}

/// Les textes, dans l'ordre, des entrées du journal qui répondent à `pick`.
fn texts(log: &[Rehearsed], pick: impl Fn(&Rehearsed) -> Option<&String>) -> Vec<String> {
    log.iter().filter_map(pick).cloned().collect()
}

fn direct_texts(log: &[Rehearsed]) -> Vec<String> {
    texts(log, |step| match step {
        Rehearsed::Direct { text, .. } => Some(text),
        _ => None,
    })
}

fn synthesized(log: &[Rehearsed]) -> Vec<String> {
    texts(log, |step| match step {
        Rehearsed::Synthesized { text, .. } => Some(text),
        _ => None,
    })
}

fn played(log: &[Rehearsed]) -> Vec<String> {
    texts(log, |step| match step {
        Rehearsed::PlayedBack { text } => Some(text),
        _ => None,
    })
}

fn fallbacks_of(log: &[Rehearsed]) -> Vec<String> {
    texts(log, |step| match step {
        Rehearsed::Fallback { reason } => Some(reason),
        _ => None,
    })
}

fn strings(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

/// Naturelle : le chemin direct, sans réglage de hauteur ni de débit ; avec, il les demande.
fn direct(checks: &mut Checks, main: &str) {
    let voice = silent();
    let plain = delivery(&VoiceSettings::default(), Some(main.to_owned()));
    voice.say("Première phrase, dite sans effet.", &plain);
    voice.say("Deuxième phrase.", &plain);
    checks.check(voice.is_speaking(), "naturelle : elle parle dès la phrase confiée");
    checks.check(voice.wait(LIMIT), "naturelle : elle finit");
    let log = voice.rehearsal_log();
    checks.check(
        log == [
            Rehearsed::Direct { text: "Première phrase, dite sans effet.".to_owned(), prosody: false },
            Rehearsed::Direct { text: "Deuxième phrase.".to_owned(), prosody: false },
        ],
        "naturelle : chemin direct, dans l'ordre, sans hauteur ni débit demandés, rien d'autre",
    );
    checks.check(!voice.is_speaking(), "naturelle : elle s'est tue");

    let voice = silent();
    let settings = VoiceSettings { pitch: Pitch::Low, rate: Rate::Fast, ..VoiceSettings::default() };
    voice.say("Une phrase un peu plus grave et plus vive.", &delivery(&settings, Some(main.to_owned())));
    voice.wait(LIMIT);
    checks.check(
        voice.rehearsal_log()
            == [Rehearsed::Direct {
                text: "Une phrase un peu plus grave et plus vive.".to_owned(),
                prosody: true,
            }],
        "naturelle, hauteur et débit changés : chemin direct, qui les demande",
    );
}

/// Avec effets : trois phrases confiées d'un coup sont rendues, puis jouées, dans l'ordre.
fn treated_in_order(checks: &mut Checks, main: &str) {
    for effect in [VoiceEffect::Construct, VoiceEffect::Radio, VoiceEffect::Hologram] {
        let voice = silent();
        let how = with(effect, Intensity::Medium, main);
        let sentences =
            ["Une première phrase.", "Une deuxième, un peu plus longue que la première.", "Et la fin."];
        let started = Instant::now();
        for sentence in sentences {
            voice.say(sentence, &how);
        }
        checks.check(voice.is_speaking(), &format!("{} : elle parle dès la phrase confiée", effect.name()));
        let done = voice.wait(LIMIT);
        let elapsed = started.elapsed().as_secs_f64();
        let log = voice.rehearsal_log();
        let length: f64 = log
            .iter()
            .map(|step| match step {
                Rehearsed::Synthesized { frames, rate, .. } if *rate > 0.0 => *frames as f64 / rate,
                _ => 0.0,
            })
            .sum();
        let name = effect.name();
        checks.check(done && !voice.is_speaking(), &format!("{name} : elle finit, et s'est tue"));
        checks.check(
            fallbacks_of(&log).is_empty(),
            &format!("{name} : aucun repli ({:?})", fallbacks_of(&log)),
        );
        checks.check(synthesized(&log) == strings(&sentences), &format!("{name} : rendues dans l'ordre"));
        checks.check(
            played(&log) == strings(&sentences),
            &format!("{name} : jouées dans l'ordre, jusqu'au bout"),
        );
        checks.check(direct_texts(&log).is_empty(), &format!("{name} : rien par le chemin direct"));
        checks.check(
            elapsed >= length * 0.9,
            &format!("{name} : jouées en temps réel ({length:.2} s de son, {elapsed:.2} s écoulées)"),
        );
        checks.check(
            log.contains(&Rehearsed::EngineStarted) && log.last() == Some(&Rehearsed::EngineStopped),
            &format!("{name} : le moteur démarre, puis s'arrête une fois la queue jouée"),
        );
    }
}

/// La pause fige le son, la reprise le relance, et rien n'est perdu.
fn pause_and_resume(checks: &mut Checks, main: &str) {
    let voice = silent();
    let long = "Une phrase assez longue pour avoir le temps de la mettre en pause, puis de la reprendre sans rien perdre.";
    voice.say(long, &with(VoiceEffect::Construct, Intensity::Medium, main));
    voice.run_for(Duration::from_millis(900));
    let before = voice.played_seconds();
    voice.pause();
    voice.run_for(Duration::from_millis(1200));
    let during = voice.played_seconds();
    checks.check(voice.is_speaking(), "pause : elle parle encore (suspendue)");
    checks.check(
        before.is_some_and(|before| before > 0.1),
        &format!("pause : du son était joué avant ({before:?} s)"),
    );
    checks.check(
        during.is_none_or(|during| before.is_some_and(|before| (during - before).abs() < 0.1)),
        &format!("pause : le son n'avance plus ({before:?} puis {during:?} s)"),
    );
    checks.check(played(&voice.rehearsal_log()).is_empty(), "pause : la phrase n'est pas finie");
    voice.resume();
    voice.run_for(Duration::from_millis(500));
    let after = voice.played_seconds();
    checks.check(
        after.is_some_and(|after| before.is_some_and(|before| after > before + 0.2)),
        &format!("reprise : le son avance de nouveau ({after:?} s)"),
    );
    checks.check(voice.wait(LIMIT), "reprise : elle finit");
    let log = voice.rehearsal_log();
    checks.check(played(&log) == [long.to_owned()], "reprise : la phrase est jouée jusqu'au bout");
    checks.check(fallbacks_of(&log).is_empty(), "pause et reprise : aucun repli");
}

/// L'arrêt coupe net : plus rien n'est rendu ni joué de ce qui était confié.
fn stop(checks: &mut Checks, main: &str) {
    let voice = silent();
    let how = with(VoiceEffect::Hologram, Intensity::Strong, main);
    voice.say("Une première phrase assez longue, qui ne doit pas aller au bout.", &how);
    voice.say("Une seconde, qui ne doit pas être dite.", &how);
    voice.run_for(Duration::from_millis(700));
    checks.check(voice.played_seconds().is_some(), "arrêt : elle jouait");
    voice.stop();
    checks.check(!voice.is_speaking(), "arrêt : elle se tait aussitôt");
    voice.run_for(Duration::from_millis(1500));
    let log = voice.rehearsal_log();
    let after: Vec<&Rehearsed> = log.iter().skip_while(|step| **step != Rehearsed::Stopped).skip(1).collect();
    checks.check(after.is_empty(), &format!("arrêt : rien après ({after:?})"));
    checks.check(played(&log).is_empty(), "arrêt : aucune phrase jouée jusqu'au bout");
    checks.check(voice.played_seconds().is_none(), "arrêt : le moteur ne tourne plus");
    checks.check(!voice.is_speaking(), "arrêt : elle reste muette");
}

/// Une autre voix (un autre format) puis d'autres effets : la chaîne est remontée entre deux
/// phrases, et l'ordre tient.
fn other_voice_and_other_chain(checks: &mut Checks, main: &str, voices: &[capri_chat::voice::VoiceInfo]) {
    let other = voices
        .iter()
        .find(|voice| voice.id.contains(".eloquence.") && voice.language.starts_with("fr"))
        .map(|voice| voice.id.clone());
    let Some(other) = other else {
        println!("sauté  : pas de seconde voix française d'un autre format");
        return;
    };
    let voice = silent();
    voice.say("Avec la première voix.", &with(VoiceEffect::Construct, Intensity::Medium, main));
    voice.say("Avec une autre voix.", &with(VoiceEffect::Construct, Intensity::Medium, &other));
    voice.say("Et d'autres effets.", &with(VoiceEffect::Radio, Intensity::Light, main));
    checks.check(voice.wait(LIMIT), "autre voix, autres effets : elle finit");
    let log = voice.rehearsal_log();
    let rates: Vec<f64> = log
        .iter()
        .filter_map(|step| match step {
            Rehearsed::Synthesized { rate, .. } => Some(*rate),
            _ => None,
        })
        .collect();
    let all = strings(&["Avec la première voix.", "Avec une autre voix.", "Et d'autres effets."]);
    checks.check(played(&log) == all, "autre voix, autres effets : jouées dans l'ordre");
    checks.check(
        rates.len() == 3 && rates[0] != rates[1],
        &format!("autre voix : un autre format ({rates:?} Hz)"),
    );
    checks.check(fallbacks_of(&log).is_empty(), "autre voix, autres effets : aucun repli");
}

/// Une phrase sans effet au milieu de phrases avec : chacune attend la précédente.
fn mixed_paths(checks: &mut Checks, main: &str) {
    let voice = silent();
    let treated = with(VoiceEffect::Construct, Intensity::Light, main);
    let plain = delivery(&VoiceSettings::default(), Some(main.to_owned()));
    voice.say("D'abord avec effet.", &treated);
    voice.say("Puis sans.", &plain);
    voice.say("Puis de nouveau avec.", &treated);
    checks.check(voice.wait(LIMIT), "chemins mêlés : elle finit");
    let log = voice.rehearsal_log();
    let at = |wanted: &Rehearsed| log.iter().position(|step| step == wanted);
    let first_played = at(&Rehearsed::PlayedBack { text: "D'abord avec effet.".to_owned() });
    let direct = at(&Rehearsed::Direct { text: "Puis sans.".to_owned(), prosody: false });
    let last = log.iter().position(
        |step| matches!(step, Rehearsed::Synthesized { text, .. } if text == "Puis de nouveau avec."),
    );
    checks.check(
        matches!((first_played, direct, last), (Some(a), Some(b), Some(c)) if a < b && b < c),
        &format!("chemins mêlés : chacune après la précédente ({first_played:?}, {direct:?}, {last:?})"),
    );
    checks.check(played(&log).len() == 2, "chemins mêlés : les deux phrases traitées sont jouées");
}

/// Chaque panne simulée fait redire, sans effet, ce qui n'a pas été joué ; et après un arrêt, les
/// effets reviennent.
fn fallbacks(checks: &mut Checks, main: &str) {
    for breakage in [Breakage::EngineStart, Breakage::ForeignFormat, Breakage::EngineStops, Breakage::Silence]
    {
        let voice = silent();
        voice.break_effects(Some(breakage));
        let how = with(VoiceEffect::Construct, Intensity::Medium, main);
        voice.say("Une phrase malgré la panne.", &how);
        voice.say("Et une autre.", &how);
        let done = voice.wait(LIMIT);
        let log = voice.rehearsal_log();
        let label = format!("{breakage:?}");
        checks.check(done && !voice.is_speaking(), &format!("panne {label} : elle finit quand même"));
        checks.check(
            fallbacks_of(&log).len() == 1,
            &format!("panne {label} : un repli ({:?})", fallbacks_of(&log)),
        );
        checks.check(
            direct_texts(&log) == strings(&["Une phrase malgré la panne.", "Et une autre."]),
            &format!("panne {label} : tout est redit sans effet, dans l'ordre"),
        );
        checks.check(played(&log).is_empty(), &format!("panne {label} : rien n'a été joué par la chaîne"));

        // après un arrêt (la réponse suivante), les effets sont retentés
        voice.stop();
        voice.break_effects(None);
        voice.say("Les effets reviennent.", &how);
        voice.wait(LIMIT);
        checks.check(
            played(&voice.rehearsal_log()).last().map(String::as_str) == Some("Les effets reviennent."),
            &format!("panne {label} : après un arrêt, les effets reviennent"),
        );
    }
}

/// Hors ligne : chaque préréglage, à chaque intensité, change bien le son, sans saturer.
fn offline(checks: &mut Checks, main: &str) {
    let sentence = speakable(TEST_SENTENCE);
    // la référence : la même chaîne, mais sans rien y mélanger
    let dry = Delivery {
        effects: vec![Effect::Reverb { room: Room::SmallRoom, mix: 0.0 }],
        ..Delivery::plain(Some(main.to_owned()))
    };
    let Some(reference) = render(checks, &sentence, &dry, "référence sèche") else { return };
    let reference_rms = rms(&reference);
    println!(
        "mesure : référence sèche, {:.2} s, niveau {:.4}, crête {:.3}",
        reference.len() as f64 / 96_000.0,
        reference_rms,
        peak(&reference)
    );
    for effect in [VoiceEffect::Construct, VoiceEffect::Radio, VoiceEffect::Hologram] {
        for intensity in Intensity::ALL {
            let label = format!("{} ({})", effect.name(), intensity.name());
            let Some(out) = render(checks, &sentence, &with(effect, intensity, main), &label) else {
                continue;
            };
            let length = out.len().min(reference.len());
            let difference: Vec<f32> =
                out[..length].iter().zip(&reference[..length]).map(|(a, b)| a - b).collect();
            let change = rms(&difference) / reference_rms;
            let level = 20.0 * (rms(&out) / reference_rms).log10();
            let top = peak(&out);
            println!(
                "mesure : {label} : écart {change:.2} du son sec, niveau {level:+.1} dB, crête {top:.3}"
            );
            checks.check(change > 0.05, &format!("hors ligne, {label} : le son est changé"));
            checks.check(top < 1.0, &format!("hors ligne, {label} : il ne sature pas"));
            // la liaison radio est voulue un peu plus discrète (environ -3 à -4 dB)
            checks.check(level.abs() <= 6.0, &format!("hors ligne, {label} : niveau proche du son sec"));
        }
    }
}

fn render(checks: &mut Checks, sentence: &str, how: &Delivery, label: &str) -> Option<Vec<f32>> {
    let voice = Voice::rehearsal(Rehearsal::Offline).expect("une voix existe sur ce système");
    voice.say(sentence, how);
    let rendered = voice.render(LIMIT);
    let log = voice.rehearsal_log();
    checks.check(
        rendered.as_ref().is_ok_and(|samples| !samples.is_empty()) && fallbacks_of(&log).is_empty(),
        &format!("hors ligne, {label} : rendu sans repli ({:?})", rendered.as_ref().err()),
    );
    rendered.ok()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |top, sample| top.max(sample.abs()))
}
