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

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
