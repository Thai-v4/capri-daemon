//! Le coucou : elle attrape l'écran à deux mains, lâche une main d'un geste vif (un filé), fait
//! coucou, la main qui va et vient deux fois, parle, sourit les yeux fermés, puis s'en va d'un
//! geste vif. Neuf images de 1 672 × 941 px, en WebP, à fond transparent (un canal alpha : mesuré
//! sur les vraies images par un essai de `capri-app`), téléchargées par `tools/fetch-assets.sh`
//! dans `assets/capri/` depuis le dépôt `mochiiwuff/capri-desktop` (voir `CREDITS.md`) : des
//! captures de la série et du fan art, un autre style de dessin que ses planches.
//!
//! **Notre propre enchaînement** : la feuille d'exposition ([`FILM`]) est écrite ici, d'après ce
//! que montrent les images (regardées une à une) ; rien n'est repris du code de ce dépôt. L'ordre
//! du coucou suit l'inclinaison de sa main : droite dans `intro-wave3`, un peu penchée dans
//! `intro-wave1`, très penchée dans `intro-wave2`.
//!
//! **Le retirer** : sa ligne de [`super::LIST`] et `pub mod coucou;` dans `intro.rs`, ce fichier,
//! les neuf lignes `capri/intro-*.webp` de `IMAGES` dans `tools/bundle-macos.sh` et les mêmes
//! dans `FORMAT_2` de l'essai de `update_setup.rs` (le format 2 n'a jamais été publié), et, s'il ne
//! sert plus, `"webp"` dans les fonctions du crate `image` de `crates/capri-app/Cargo.toml`. Rien
//! d'autre ne le nomme, hors des commentaires.

use super::Film;

/// Ses images, dans le dossier des images (`assets/capri/`).
const FILES: [&str; 9] = [
    "intro-grab.webp",
    "intro-smear.webp",
    "intro-wave1.webp",
    "intro-wave2.webp",
    "intro-wave3.webp",
    "intro-mouth1.webp",
    "intro-mouth2.webp",
    "intro-mouth3.webp",
    "intro-happy.webp",
];

/// Les numéros de ses images dans [`FILES`].
const GRAB: u8 = 0;
const SMEAR: u8 = 1;
const WAVE1: u8 = 2;
const WAVE2: u8 = 3;
const WAVE3: u8 = 4;
const MOUTH1: u8 = 5;
const MOUTH2: u8 = 6;
const MOUTH3: u8 = 7;
const HAPPY: u8 = 8;

/// Sa feuille d'exposition, à 24 i/s : 71 images, 2,96 s.
const SHEET: [(u8, u8); 20] = [
    // elle attrape l'écran (un tiers de seconde), puis lâche sa main droite d'un geste vif
    (GRAB, 8),
    (SMEAR, 2),
    // coucou : la main va et vient deux fois, en trois (8 dessins par seconde)
    (WAVE3, 3),
    (WAVE1, 3),
    (WAVE2, 3),
    (WAVE1, 3),
    (WAVE3, 3),
    (WAVE1, 3),
    (WAVE2, 3),
    (WAVE1, 3),
    (WAVE3, 3),
    // sa main revient sur l'écran
    (SMEAR, 2),
    // elle parle : sa bouche s'ouvre et se ferme, en trois
    (MOUTH1, 3),
    (MOUTH3, 3),
    (MOUTH2, 3),
    (MOUTH3, 3),
    (MOUTH1, 3),
    // elle sourit, les yeux fermés (une demi-seconde)
    (HAPPY, 12),
    // puis elle s'en va : elle reprend l'écran, et le lâche d'un geste vif
    (GRAB, 3),
    (SMEAR, 2),
];

/// Le coucou, cadré comme « Up close » : il couvre l'écran, centré, calé en bas.
pub const FILM: Film = Film { dir: "capri", files: &FILES, size: (1672, 941), sheet: &SHEET };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_lasts_about_three_seconds_and_waves_back_and_forth_twice() {
        assert_eq!(FILM.frames(), 71);
        assert!((FILM.frames() as f64 / crate::anim::FRAME_RATE - 2.96).abs() < 0.01);
        // le coucou : la main droite, puis penchée, puis très penchée, et retour, deux fois
        let waves: Vec<u8> =
            SHEET.iter().map(|&(image, _)| image).filter(|&image| (WAVE1..=WAVE3).contains(&image)).collect();
        assert_eq!(waves, [WAVE3, WAVE1, WAVE2, WAVE1, WAVE3, WAVE1, WAVE2, WAVE1, WAVE3]);
        // il commence en attrapant l'écran et finit par le filé : elle s'en va
        assert_eq!((SHEET[0].0, SHEET[SHEET.len() - 1].0), (GRAB, SMEAR));
        assert!(SHEET.iter().any(|&(image, _)| image == HAPPY));
        assert!(FILES.iter().all(|file| file.starts_with("intro-") && file.ends_with(".webp")));
    }

    #[test]
    fn it_covers_the_screen_like_up_close() {
        use crate::geom::{Rect, Vec2};
        // sur un MacBook Pro 14" (3024 × 1964) : toute la hauteur, les côtés coupés
        let rect = super::super::place(Vec2::new(3024.0, 1964.0), FILM.size);
        assert_eq!(rect, Rect::new(Vec2::new(-233.0, 0.0), Vec2::new(3490.0, 1964.0)));
    }
}
