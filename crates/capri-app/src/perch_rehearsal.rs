//! `capri perch rehearse`, en version de débogage seulement : la vraie liste des fenêtres, lue et
//! triée comme Capri le fait, sans rien afficher à l'écran.
//!
//! N'imprime que des nombres : combien de fenêtres, pourquoi certaines ne comptent pas, le rang
//! (de la plus en avant à la plus en arrière) et le rectangle de celles qui comptent, les morceaux
//! visibles de leurs bords, en pixels de l'overlay, un essai de pose sur l'un d'eux, et ce que
//! coûte une lecture. Jamais un titre ni un nom d'app : la liste n'en contient pas. Ne lit et
//! n'écrit aucun réglage, ne demande aucune permission.
//!
//! Rend 0 si tout est conforme, 1 sinon.
//!
//! Et, pour mesurer comment elle suit une fenêtre qu'on déplace, `CAPRI_PERCH_REHEARSAL=1` au
//! lancement de la vraie app (voir [`follow`]).

use std::time::{Duration, Instant};

use capri_core::perch::{self, Skip, Stature};
use capri_core::{Rect, Vec2};
use capri_platform::desk;

/// Combien de lectures chronométrer.
const READS: usize = 200;

pub fn run_cli(args: &[String]) -> i32 {
    match args {
        [action] if action == "rehearse" => run(),
        // lancée par la vraie app pendant `CAPRI_PERCH_REHEARSAL`, pas à la main
        [action, hz] if action == "window" => follow::moving_window(hz.parse().unwrap_or(60.0)),
        _ => {
            eprintln!("usage : capri perch rehearse   la vraie liste des fenêtres, sans rien afficher");
            2
        }
    }
}

pub use follow::{Follow, Trace};

fn run() -> i32 {
    let mut failed = 0;
    let mut check = |ok: bool, what: &str| {
        println!("{} : {what}", if ok { "ok" } else { "ÉCHEC" });
        failed += usize::from(!ok);
    };
    let Some((frame, (width, height))) = desk::main_screen() else {
        println!("pas d'écran principal ici (sur macOS : il faut le thread principal)");
        return i32::from(cfg!(target_os = "macos"));
    };
    let screen = Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32));
    println!(
        "écran principal : {width} × {height} px, {} px par point, overlay à ({}, {}) points",
        frame.pixels_per_unit, frame.origin.x, frame.origin.y
    );
    if let Some(allowed) = desk::screen_recording_allowed() {
        println!(
            "enregistrement de l'écran : {} (rien n'a été demandé)",
            if allowed { "accordé" } else { "non accordé" }
        );
    }
    let stature = stature(frame.pixels_per_unit);
    println!(
        "Capri : {:.0} × {:.0} px, pieds à ({:.2}, {:.2})",
        stature.size.x, stature.size.y, stature.feet.x, stature.feet.y
    );

    let windows = match desk::windows() {
        Ok(windows) => windows,
        Err(err) => {
            println!("ÉCHEC : la liste des fenêtres ({err})");
            return 1;
        }
    };
    let reasons: Vec<Option<Skip>> =
        windows.iter().map(|window| perch::skip(window, frame, screen, stature.size.y)).collect();
    let count = |reason: Skip| reasons.iter().filter(|&&r| r == Some(reason)).count();
    let seen = perch::sift(&windows, frame, screen, stature.size.y);
    println!(
        "fenêtres à l'écran : {} ; comptées : {} (à elle : {}, inhabituelles : {}, peu opaques : {}, \
         petites : {}, ailleurs : {})",
        windows.len(),
        seen.len(),
        count(Skip::Ours),
        count(Skip::Unusual),
        count(Skip::Faint),
        count(Skip::Small),
        count(Skip::Elsewhere)
    );
    for (rank, window) in seen.iter().enumerate() {
        let r = window.rect;
        println!("  n° {} : ({:.0}, {:.0}) {:.0} × {:.0} px", rank + 1, r.min.x, r.min.y, r.size.x, r.size.y);
    }
    let rank = |id: u64| seen.iter().position(|window| window.id == id).map_or(0, |rank| rank + 1);
    let ledges = perch::ledges(&seen, screen);
    println!("bords visibles : {}", ledges.len());
    for ledge in &ledges {
        println!(
            "  fenêtre n° {} : y = {:.0}, de {:.0} à {:.0} px",
            rank(ledge.window),
            ledge.y,
            ledge.from,
            ledge.to
        );
    }

    check(
        reasons.iter().filter(|r| r.is_none()).count() == seen.len(),
        "chaque fenêtre comptée a passé le tri, et elles seules",
    );
    check(
        windows.iter().filter(|window| window.ours).all(|w| !seen.iter().any(|s| s.id == w.id)),
        "aucune des siennes",
    );
    check(
        seen.iter().all(|window| window.rect.size.x.is_finite() && window.rect.size.y.is_finite()),
        "des rectangles qui sont des nombres",
    );
    check(
        ledges.iter().all(|l| {
            l.from < l.to && l.from >= 0.0 && l.to <= screen.size.x && l.y >= 0.0 && l.y < screen.size.y
        }),
        "des bords visibles dans l'écran",
    );

    // un essai : lâchée au milieu du premier bord qui lui laisse la place, elle s'y pose, et la
    // même lecture la garde au même endroit
    let tried = ledges.iter().find_map(|ledge| {
        let feet = Vec2::new((ledge.from + ledge.to) / 2.0, ledge.y - 10.0);
        perch::landing(&seen, screen, stature, feet).map(|perch| (ledge, perch))
    });
    match tried {
        Some((ledge, landed)) => {
            println!(
                "essai : lâchée au-dessus du bord de la fenêtre n° {}, posée sur celui de la fenêtre n° {}, \
                 pieds en ({:.0}, {:.0}) px",
                rank(ledge.window),
                rank(landed.spot.window),
                landed.feet.x,
                landed.feet.y
            );
            check(
                seen.iter().any(|w| w.id == landed.spot.window && w.rect.min.y == landed.feet.y),
                "ses pieds sur le bord de sa fenêtre",
            );
            check(
                perch::follow(&seen, screen, stature, landed.spot) == Ok(landed),
                "à la lecture suivante, elle reste à la même place",
            );
        }
        None => println!("essai : aucun bord ne lui laisse la place en ce moment"),
    }

    let mut times = Vec::with_capacity(READS);
    for _ in 0..READS {
        let start = Instant::now();
        let read = desk::windows().map(|windows| perch::sift(&windows, frame, screen, stature.size.y));
        times.push(start.elapsed());
        if read.is_err() {
            check(false, "chaque lecture réussit");
            break;
        }
    }
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{} lectures triées (version de débogage) : médiane {:.3} ms, 95 % sous {:.3} ms, max {:.3} ms",
        times.len(),
        ms(times[times.len() / 2]),
        ms(times[times.len() * 95 / 100]),
        ms(times[times.len() - 1])
    );
    println!("bouton de la souris enfoncé : {}", if desk::mouse_down() { "oui" } else { "non" });

    if failed == 0 {
        println!("tout est conforme");
        0
    } else {
        println!("{failed} vérification(s) en échec");
        1
    }
}

/// Sa taille à l'écran et la place de ses pieds, d'après le vrai sprite s'il est là ; sinon ses
/// proportions d'origine (220 × 420), les pieds au milieu du bas.
fn stature(scale: f32) -> Stature {
    match crate::assets::load_sprite() {
        Ok(sprite) => Stature {
            size: crate::display_size(sprite.mask.size(), f64::from(scale), capri_core::Scale::ORIGINAL),
            feet: sprite.mask.feet(),
        },
        Err(_) => {
            println!("images absentes : proportions d'origine");
            let size = crate::display_size((220, 420), f64::from(scale), capri_core::Scale::ORIGINAL);
            Stature { size, feet: Vec2::new(0.5, 1.0) }
        }
    }
}

pub mod follow;
