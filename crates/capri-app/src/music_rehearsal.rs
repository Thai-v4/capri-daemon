//! `capri music rehearse`, en version de débogage seulement : pendant quelques secondes, écoute si
//! un son sort d'une autre app, comme Capri le ferait, et dit ce qu'elle en déciderait.
//!
//! N'imprime que « joue : oui » ou « joue : non » et des nombres (appareils en marche, clients du
//! son, clients interrogés, durée d'une écoute). Jamais un titre ni un nom d'app : rien de tel
//! n'est lu. Ne joue aucun son, ne lit et n'écrit aucun réglage, ne demande aucune permission.
//!
//! Rend 0 si tout est conforme (ou si ce système ne sait pas écouter, ce qu'elle dit), 1 sinon.

use std::time::{Duration, Instant};

use capri_core::music::Groove;
use capri_platform::music::{self, MusicError, SoundWatch};

/// Combien de temps écouter.
const LENGTH: Duration = Duration::from_secs(6);
/// Entre deux écoutes, pendant la répétition (Capri écoutera moins souvent :
/// `capri_core::music::LISTEN_EVERY`).
const EVERY: Duration = Duration::from_millis(500);
/// Combien d'écoutes chronométrer, dans chaque cas.
const READS: usize = 200;

pub fn run_cli(args: &[String]) -> i32 {
    match args {
        [action] if action == "rehearse" => run(),
        _ => {
            eprintln!("usage : capri music rehearse   écouter si un son joue, sans rien jouer");
            2
        }
    }
}

fn yes(playing: bool) -> &'static str {
    if playing { "oui" } else { "non" }
}

fn run() -> i32 {
    let mut watch = SoundWatch::new();
    let start = Instant::now();
    let first = watch.listen();
    let first_time = start.elapsed();
    match first {
        Err(MusicError::Unavailable) => {
            println!("elle ne sait pas, sur ce système, si un son joue");
            return i32::from(music::AVAILABLE);
        }
        Err(err) => {
            println!("ÉCHEC : {err}");
            return 1;
        }
        Ok(_) => println!("première écoute : {:.3} ms", ms(first_time)),
    }

    let mut groove = Groove::new();
    let mut failed = 0;
    let begin = Instant::now();
    while begin.elapsed() < LENGTH {
        let now = Instant::now();
        let heard = watch.listen();
        let took = now.elapsed();
        let decided = groove.hear(now, heard.ok().map(|heard| heard.playing));
        match heard {
            Ok(heard) => println!(
                "{:.1} s : joue : {} ({} appareils en marche, {} clients du son, {} interrogés, {:.3} ms) ; \
                 elle se balancerait : {}",
                now.duration_since(begin).as_secs_f64(),
                yes(heard.playing),
                heard.devices_running,
                heard.clients,
                heard.asked,
                ms(took),
                yes(decided)
            ),
            Err(err) => {
                println!("ÉCHEC : {err}");
                failed += 1;
            }
        }
        std::thread::sleep(EVERY.saturating_sub(now.elapsed()));
    }

    own_voice(&mut watch);

    // le coût : une écoute ordinaire, puis le pire cas (tous les clients interrogés)
    for (what, thoroughly) in [("écoutes ordinaires", false), ("écoutes de tous les clients", true)] {
        let mut times = Vec::with_capacity(READS);
        let mut asked = 0;
        for _ in 0..READS {
            let start = Instant::now();
            let heard = if thoroughly { watch.listen_thoroughly() } else { watch.listen() };
            times.push(start.elapsed());
            match heard {
                Ok(heard) => asked = asked.max(heard.asked),
                Err(_) => {
                    failed += 1;
                    break;
                }
            }
        }
        times.sort();
        println!(
            "{} {what} (version de débogage, jusqu'à {asked} clients interrogés) : médiane {:.3} ms, \
             95 % sous {:.3} ms, max {:.3} ms",
            times.len(),
            ms(times[times.len() / 2]),
            ms(times[times.len() * 95 / 100]),
            ms(times[times.len() - 1])
        );
    }

    if failed == 0 {
        println!("tout est conforme");
        0
    } else {
        println!("{failed} écoute(s) en échec");
        1
    }
}

/// Sa propre voix compte-t-elle comme « un son d'une autre app » ? Dans ce processus, une voix
/// d'essai muette (les vrais chemins jusqu'à la sortie du Mac, à volume nul) dit une phrase, sans
/// effet puis avec, pendant qu'elle compte les autres apps qui jouent et regarde si ce processus
/// sort du son ; puis elle compte encore après. Si une autre app joue déjà, c'est le nombre qui
/// dit si sa voix s'y ajoute. N'imprime que des oui, des non et des nombres.
fn own_voice(watch: &mut SoundWatch) {
    use capri_chat::voice::{VoiceEffect, VoiceSettings, delivery};
    use capri_platform::{Rehearsal, Voice};

    const SENTENCE: &str =
        "Une phrase assez longue pour avoir le temps d'écouter plusieurs fois pendant qu'elle la dit.";
    let count = |watch: &mut SoundWatch| watch.count().unwrap_or_default();
    let before = count(watch);
    println!(
        "sa voix : avant qu'elle parle, {} autres clients jouent ; ce processus sort du son : {}",
        before.others,
        yes(before.ours)
    );
    for (path, effect) in [("directe", VoiceEffect::Natural), ("traitée", VoiceEffect::Construct)] {
        let Some(voice) = Voice::rehearsal(Rehearsal::Silent) else {
            println!("sa voix : pas de voix sur ce système");
            return;
        };
        voice.say(SENTENCE, &delivery(&VoiceSettings { effect, ..VoiceSettings::default() }, None));
        let (mut during, mut more, mut ours) = (0, 0, 0);
        let started = Instant::now();
        while voice.is_speaking() && started.elapsed() < Duration::from_secs(20) {
            voice.run_for(Duration::from_millis(300));
            let now = count(watch);
            during += 1;
            more += usize::from(now.others > before.others);
            ours += usize::from(now.ours);
        }
        // après sa dernière syllabe : combien de temps une autre app, ou ce processus, joue encore
        let (mut others_after, mut ours_after) = (0, 0);
        for step in 1..=12 {
            voice.run_for(Duration::from_millis(250));
            let now = count(watch);
            if now.others > before.others {
                others_after = step * 250;
            }
            if now.ours {
                ours_after = step * 250;
            }
        }
        println!(
            "sa voix ({path}) : {during} écoutes pendant qu'elle parle ; une autre app de plus joue : {} ({more}) ; \
             ce processus sort du son : {} ({ours}) ; après : une autre app de plus encore {others_after} ms, \
             ce processus encore {ours_after} ms",
            yes(more > 0),
            yes(ours > 0)
        );
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
