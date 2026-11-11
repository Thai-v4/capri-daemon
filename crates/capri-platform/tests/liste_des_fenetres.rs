//! Ce que coûte une lecture de la vraie liste des fenêtres, sur cette machine.
//!
//! Lancer en version optimisée, comme Capri tourne :
//! `cargo test --release -p capri-platform --test liste_des_fenetres -- --ignored --nocapture`.
//! N'imprime que des nombres : combien de fenêtres, et en combien de temps. Aucun titre, aucun nom
//! d'app n'est lu (la liste n'en contient pas), aucune permission n'est demandée.

use std::time::{Duration, Instant};

use capri_platform::desk;

#[test]
#[ignore = "lit la vraie liste des fenêtres (macOS ou Windows) ; à lancer à la main"]
fn reading_the_window_list_is_cheap() {
    if !desk::AVAILABLE {
        assert_eq!(desk::windows(), Err(desk::DeskError::Unavailable));
        return;
    }
    const READS: usize = 500;
    let first = desk::windows().expect("la liste des fenêtres");
    let mut times = Vec::with_capacity(READS);
    for _ in 0..READS {
        let start = Instant::now();
        let windows = desk::windows().expect("la liste des fenêtres");
        times.push(start.elapsed());
        assert!(windows.iter().all(|w| w.rect.size.x.is_finite() && w.rect.size.y.is_finite()));
    }
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let total: Duration = times.iter().sum();
    let ordinary = first.iter().filter(|w| w.ordinary).count();
    let ours = first.iter().filter(|w| w.ours).count();
    println!(
        "{} fenêtres à l'écran ({ordinary} ordinaires, {ours} à nous) ; {READS} lectures : \
         min {:.3} ms, médiane {:.3} ms, 95 % sous {:.3} ms, max {:.3} ms, moyenne {:.3} ms",
        first.len(),
        ms(times[0]),
        ms(times[READS / 2]),
        ms(times[READS * 95 / 100]),
        ms(times[READS - 1]),
        ms(total) / READS as f64,
    );
    assert!(times[READS / 2] < Duration::from_millis(20), "une lecture ne devrait pas coûter autant");

    // l'indice « un bouton de la souris est enfoncé », demandé à chaque image pendant qu'elle est
    // posée
    let mut times = Vec::with_capacity(READS);
    for _ in 0..READS {
        let start = Instant::now();
        std::hint::black_box(desk::mouse_down());
        times.push(start.elapsed());
    }
    times.sort();
    let total: Duration = times.iter().sum();
    println!(
        "bouton de la souris : {READS} lectures : médiane {:.4} ms, 95 % sous {:.4} ms, max {:.4} ms, \
         moyenne {:.4} ms",
        ms(times[READS / 2]),
        ms(times[READS * 95 / 100]),
        ms(times[READS - 1]),
        ms(total) / READS as f64,
    );
}
