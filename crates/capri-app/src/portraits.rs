//! Pour regarder ce qu'elle dessine, sans écran ni fenêtre : de vraies images de chaque pose,
//! rendues par la vraie carte graphique avec les vraies planches, par le même code que l'app
//! (`figure::Figure`, `capri_core::look`, `capri_render::Offscreen`), sur un faux bureau où une
//! fenêtre est un rectangle clair.
//!
//! À lancer à la main, en version optimisée (lire les planches est lent sinon) :
//! `CAPRI_PORTRAITS_DIR=/tmp/portraits cargo test --release -p capri-app portraits -- --ignored`
//!
//! Ce que ces images ne montrent pas : le mouvement (sauf image par image), le suivi d'une vraie
//! fenêtre, la musique réelle.

use std::path::{Path, PathBuf};
use std::time::Instant;

use capri_core::look::{Drawing, Picture};
use capri_core::perch::{Perch, Spot};
use capri_core::sheets::{self, stand};
use capri_core::{Capri, Rect, Vec2};
use capri_render::{Crop, ImageId, Offscreen, SpriteDraw, SpriteImage};

use crate::assets;
use crate::figure::Figure;

/// Sa hauteur debout à la taille d'origine sur un écran Retina : 300 points, 600 pixels.
const HEIGHT: f32 = 600.0;
/// Le faux bureau, une fenêtre, et sa barre de titre.
const DESK: [u8; 4] = [38, 44, 62, 255];
const WINDOW: [u8; 4] = [236, 238, 242, 255];
const TITLE: [u8; 4] = [214, 216, 222, 255];

/// Un pinceau : de quoi peindre des rectangles pleins avec le même rendu qu'elle.
struct Scene {
    offscreen: Offscreen,
    figure: Figure,
    desk: ImageId,
    window: ImageId,
    title: ImageId,
    size: (u32, u32),
}

impl Scene {
    fn new(size: (u32, u32), height: f32) -> Scene {
        let started = Instant::now();
        let sprite = assets::load_sprite().expect("son dessin d'avant");
        let stand = assets::stand::load();
        let perch = assets::perch::load();
        println!(
            "planches lues en {} ms : debout {}, assise {}",
            started.elapsed().as_millis(),
            stand.is_some(),
            perch.is_some()
        );
        let mut offscreen = Offscreen::new(size.0, size.1).expect("une carte graphique");
        let figure =
            Figure::new(sprite, stand, perch, height, |image| offscreen.add_image(image)).expect("confiées");
        println!("mémoire graphique : {:.1} Mo", offscreen.image_bytes() as f64 / 1_000_000.0);
        let mut plain = |color: [u8; 4]| {
            offscreen.add_image(SpriteImage { width: 1, height: 1, rgba: &color }).expect("une couleur")
        };
        let (desk, window, title) = (plain(DESK), plain(WINDOW), plain(TITLE));
        Scene { offscreen, figure, desk, window, title, size }
    }

    fn rect(id: ImageId, rect: Rect) -> (ImageId, SpriteDraw) {
        (
            id,
            SpriteDraw {
                min: [rect.min.x, rect.min.y],
                size: [rect.size.x, rect.size.y],
                ghost: [0.0, 0.0],
                ghost_strength: 0.0,
                boil_seed: 0,
                boil: 0.0,
                opacity: 1.0,
                crop: Crop::WHOLE,
            },
        )
    }

    /// Rend le bureau, la fenêtre `window` (si elle est là) et ce qu'elle montre ; rend des pixels
    /// opaques.
    fn render(&mut self, window: Option<Rect>, pictures: &[Picture]) -> Vec<u8> {
        let (w, h) = self.size;
        let mut layers = vec![Self::rect(self.desk, Rect::new(Vec2::ZERO, Vec2::new(w as f32, h as f32)))];
        if let Some(window) = window {
            layers.push(Self::rect(self.window, window));
            layers.push(Self::rect(self.title, Rect::new(window.min, Vec2::new(window.size.x, 56.0))));
        }
        for picture in pictures {
            layers.extend(self.figure.layers(picture));
        }
        self.offscreen.render(&layers, None).expect("rendue")
    }
}

/// Capri telle que l'app la crée, à `feet` (ses pieds debout).
fn capri(figure: &Figure, screen: (u32, u32), feet: Vec2) -> Capri {
    let look = &figure.look;
    let bounds = Rect::new(Vec2::ZERO, Vec2::new(screen.0 as f32, screen.1 as f32));
    let size = look.standing_size();
    let feet_uv = look.standing_feet();
    let mut capri = Capri::new(size, bounds, feet - Vec2::new(size.x * feet_uv.x, size.y * feet_uv.y));
    capri.set_feet(feet_uv);
    capri.set_leave_frames(look.leave_frames());
    capri
}

fn save(dir: &Path, name: &str, rgba: &[u8], size: (u32, u32)) {
    let path = dir.join(name);
    image::save_buffer(&path, rgba, size.0, size.1, image::ExtendedColorType::Rgba8).expect("écrite");
    println!("{}", path.display());
}

/// Un morceau d'une image : le rectangle de `size` pixels au coin haut gauche `at`.
fn crop(rgba: &[u8], width: u32, at: (u32, u32), size: (u32, u32)) -> Vec<u8> {
    let mut out = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for y in at.1..at.1 + size.1 {
        let from = ((y * width + at.0) * 4) as usize;
        out.extend_from_slice(&rgba[from..from + (size.0 * 4) as usize]);
    }
    out
}

/// Des images de même taille, côte à côte, en lignes de `columns`.
fn contact(images: &[Vec<u8>], size: (u32, u32), columns: usize) -> (Vec<u8>, (u32, u32)) {
    let rows = images.len().div_ceil(columns);
    let (w, h) = (size.0 as usize * columns, size.1 as usize * rows);
    let mut out = vec![0u8; w * h * 4];
    for (index, image) in images.iter().enumerate() {
        let (cx, cy) = (index % columns * size.0 as usize, index / columns * size.1 as usize);
        for y in 0..size.1 as usize {
            let from = y * size.0 as usize * 4;
            let to = ((cy + y) * w + cx) * 4;
            out[to..to + size.0 as usize * 4].copy_from_slice(&image[from..from + size.0 as usize * 4]);
        }
    }
    (out, (w as u32, h as u32))
}

fn directory() -> PathBuf {
    let dir = PathBuf::from(std::env::var("CAPRI_PORTRAITS_DIR").expect("CAPRI_PORTRAITS_DIR"));
    std::fs::create_dir_all(&dir).expect("dossier");
    dir
}

#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies planches et la vraie carte graphique"]
fn portraits() {
    let dir = directory();
    // une scène de 900 × 900 pixels : un morceau d'écran Retina
    let size = (900, 900);
    let mut scene = Scene::new(size, HEIGHT);
    assert!(scene.figure.look.has_stand() && scene.figure.look.has_perch(), "il faut les planches");

    // debout : la boucle, un clignement, sa bouche
    let feet = Vec2::new(450.0, 820.0);
    let mut c = capri(&scene.figure, size, feet);
    let mut stood = Vec::new();
    for (frame, talking) in [(0, false), (28, false), (6, true), (10, true), (12, true), (40, false)] {
        c.step(0.0, frame);
        scene.figure.look.update(&c, frame, talking, false);
        stood.push(scene.render(None, &[scene.figure.look.picture(&c)]));
    }
    save(&dir, "debout.png", &stood[0], size);
    save(&dir, "debout-clignant.png", &stood[1], size);
    save(&dir, "debout-parlant.png", &stood[2], size);
    let (sheet, sheet_size) = contact(&stood, size, 3);
    save(&dir, "debout-planche.png", &sheet, sheet_size);

    // son visage de près : les yeux ouverts, fermés, puis ses cinq bouches (à 1 pixel d'écran
    // pour 1 pixel d'image : c'est la netteté réelle sur un écran Retina)
    let face = (200, 200);
    let mut faces = Vec::new();
    for (frame, talking) in [(0, false), (28, false), (0, true), (2, true), (4, true), (6, true), (10, true)]
    {
        c.step(0.0, frame);
        scene.figure.look.update(&c, frame, talking, false);
        let picture = scene.figure.look.picture(&c);
        let s = picture.rect.size.y / sheets::CELL_HEIGHT as f32;
        let center = picture.rect.min + Vec2::new(66.0 * s, 124.0 * s);
        let at = ((center.x - 100.0) as u32, (center.y - 100.0) as u32);
        let image = scene.render(None, &[picture]);
        faces.push(crop(&image, size.0, at, face));
    }
    let (sheet, sheet_size) = contact(&faces, face, 7);
    save(&dir, "visage.png", &sheet, sheet_size);

    // le même, quatre fois plus grande : la bouche posée couvre-t-elle bien celle de la boucle ?
    let big = (420, 300);
    let mut zoomed = Scene::new((1200, 2800), HEIGHT * 4.0);
    let mut c = capri(&zoomed.figure, zoomed.size, Vec2::new(600.0, 2700.0));
    let mut mouths = Vec::new();
    for (frame, talking) in [(0, false), (0, true), (2, true), (4, true), (6, true), (10, true)] {
        c.step(0.0, frame);
        zoomed.figure.look.update(&c, frame, talking, false);
        let picture = zoomed.figure.look.picture(&c);
        let s = picture.rect.size.y / sheets::CELL_HEIGHT as f32;
        let center = picture.rect.min + Vec2::new(66.0 * s, 124.0 * s);
        let at = ((center.x - big.0 as f32 / 2.0) as u32, (center.y - big.1 as f32 / 2.0) as u32);
        let image = zoomed.render(None, &[picture]);
        mouths.push(crop(&image, zoomed.size.0, at, big));
    }
    let (sheet, sheet_size) = contact(&mouths, big, 3);
    save(&dir, "bouche-agrandie.png", &sheet, sheet_size);

    // tenue, puis lancée : la même boucle, étirée dans le sens de sa course
    let mut c = capri(&scene.figure, size, feet);
    c.step(0.0, 50);
    let grip = c.position() + Vec2::new(100.0, 300.0);
    c.grab(grip, 0.0);
    scene.figure.look.update(&c, 50, false, false);
    let held = scene.render(None, &[scene.figure.look.picture(&c)]);
    c.drag_to(grip + Vec2::new(-200.0, -10.0), 0.04);
    c.release(0.05);
    c.step(1.0 / 120.0, 51);
    scene.figure.look.update(&c, 51, false, false);
    let thrown = scene.render(None, &[scene.figure.look.picture(&c)]);
    let (sheet, sheet_size) = contact(&[held, thrown], size, 2);
    save(&dir, "tenue-et-lancee.png", &sheet, sheet_size);
}

/// Pour les essais de la pose assise : posée sur le bord de `window`, son point posé en `seat`.
fn perched(scene: &mut Scene, window: Rect, seat: Vec2, from: u64) -> Capri {
    let mut c = capri(&scene.figure, scene.size, seat - Vec2::new(0.0, 4.0));
    c.step(0.0, from);
    // comme dans l'app : elle n'était pas posée juste avant (lâchée près du bord)
    scene.figure.look.update(&c, from, false, false);
    let spot = Spot { window: 1, along: seat.x - window.min.x, left: window.min.x, width: window.size.x };
    c.land(Perch { spot, feet: seat });
    scene.figure.look.update(&c, from, false, false);
    c
}

#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies planches et la vraie carte graphique"]
fn seated_portraits() {
    let dir = directory();
    let size = (900, 900);
    let mut scene = Scene::new(size, HEIGHT);
    assert!(scene.figure.look.has_perch(), "il faut les planches assises");
    // une fenêtre dont le haut est à 480 pixels du haut de la scène
    let window = Rect::new(Vec2::new(140.0, 480.0), Vec2::new(640.0, 420.0));
    let seat = Vec2::new(430.0, window.min.y);

    // assise : sa boucle (une fois l'arrivée finie), un clignement, sa parole, en musique
    let mut c = perched(&mut scene, window, seat, 0);
    let since = sheets::arriving().len();
    let mut seated = Vec::new();
    for (frame, talking, music) in [
        (since, false, false),
        (since + 30, false, false),
        (since + 10, true, false),
        (since + 60, false, true),
        (since + 80, false, true),
        (since + 100, false, true),
    ] {
        c.step(0.0, frame);
        scene.figure.look.update(&c, frame, talking, music);
        seated.push(scene.render(Some(window), &[scene.figure.look.picture(&c)]));
    }
    save(&dir, "assise.png", &seated[0], size);
    save(&dir, "assise-musique.png", &seated[3], size);
    let (sheet, sheet_size) = contact(&seated, size, 3);
    save(&dir, "assise-planche.png", &sheet, sheet_size);

    // l'arrivée et le départ, image par image (un dessin sur deux images)
    let mut c = perched(&mut scene, window, seat, 1000);
    let mut frames = Vec::new();
    for frame in (1000..1000 + since).step_by(2) {
        c.step(0.0, frame);
        scene.figure.look.update(&c, frame, false, false);
        frames.push(scene.render(Some(window), &[scene.figure.look.picture(&c)]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 7);
    save(&dir, "arrivee.png", &sheet, sheet_size);
    let left = 2000;
    c.step(0.0, left);
    scene.figure.look.update(&c, left, false, false);
    c.take_off();
    scene.figure.look.update(&c, left, false, false);
    let mut frames = Vec::new();
    for frame in (left..left + sheets::leaving().len() + 4).step_by(2) {
        c.step(0.0, frame);
        scene.figure.look.update(&c, frame, false, false);
        frames.push(scene.render(Some(window), &[scene.figure.look.picture(&c)]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 7);
    save(&dir, "depart.png", &sheet, sheet_size);

    // les deux poses côte à côte, à la même échelle : debout (à gauche, les pieds sur le même bord)
    // et assise (à droite)
    let wide = (1400, 900);
    let mut scene = Scene::new(wide, HEIGHT);
    let window = Rect::new(Vec2::new(60.0, 760.0), Vec2::new(1280.0, 140.0));
    let standing = {
        let mut c = capri(&scene.figure, wide, Vec2::new(330.0, window.min.y));
        c.step(0.0, 24); // au plus bas de son flottement : ses pieds sur le bord
        scene.figure.look.update(&c, 24, false, false);
        scene.figure.look.picture(&c)
    };
    let mut c = perched(&mut scene, window, Vec2::new(1000.0, window.min.y), 0);
    c.step(0.0, since + 24);
    scene.figure.look.update(&c, since + 24, false, false);
    let sitting = scene.figure.look.picture(&c);
    let both = scene.render(Some(window), &[standing, sitting]);
    save(&dir, "cote-a-cote.png", &both, wide);
    // leurs visages, à la même échelle, de près : le masque vert de ses yeux, son visage noir
    let head = (240, 200);
    let face_of = |picture: &Picture, (x, y): (f32, f32)| {
        let s = picture.rect.size.y / sheets::CELL_HEIGHT as f32;
        let center = picture.rect.min + Vec2::new(x * s, y * s);
        ((center.x - head.0 as f32 / 2.0) as u32, (center.y - head.1 as f32 / 2.0) as u32)
    };
    // les yeux : vers (66, 112) debout, vers (134, 160) assise (regardés sur les planches)
    let heads = [crop(&both, wide.0, face_of(&standing, (66.0, 112.0)), head), {
        crop(&both, wide.0, face_of(&sitting, (134.0, 160.0)), head)
    }];
    let (sheet, sheet_size) = contact(&heads, head, 2);
    save(&dir, "visages-cote-a-cote.png", &sheet, sheet_size);
    // pour comparer : assise à 1,20 au lieu de STAND_OVER_PERCH (sa ligne d'assise au même endroit)
    let other = stand::STAND_OVER_PERCH / 1.20;
    let seat = Vec2::new(1000.0, window.min.y);
    let smaller = Picture {
        rect: Rect::new(
            seat + (sitting.rect.min - seat) * other,
            Vec2::new((sitting.rect.size.x * other).round(), (sitting.rect.size.y * other).round()),
        ),
        ..sitting
    };
    let both = scene.render(Some(window), &[standing, smaller]);
    save(&dir, "cote-a-cote-a-1-20.png", &both, wide);
    let k = stand::perch_scale(HEIGHT);
    println!(
        "échelle : {:.3} px d'écran par px de planche debout, {:.3} assise ; debout {:.0} px de haut, \
         assise {:.0} px au-dessus du bord et {:.0} en dessous",
        stand::scale(HEIGHT),
        k,
        HEIGHT,
        sheets::HEADROOM as f32 * k,
        sheets::HANG as f32 * k
    );
}

/// Sur la vraie carte graphique : sans planches, ou si la carte graphique les refuse, c'est son
/// dessin d'avant qui est dessiné, debout comme posée, sans planter.
#[test]
#[ignore = "confie des images à la vraie carte graphique de cette machine"]
fn without_her_sheets_or_refused_she_falls_back_to_her_old_drawing() {
    let mut offscreen = Offscreen::new(64, 64).expect("une carte graphique");
    let sprite = assets::load_sprite().expect("son dessin d'avant");
    let cutout = (sprite.width, sprite.height);
    // planches présentes, mais la carte graphique refuse tout ce qui dépasse son dessin d'avant
    let mut figure = Figure::new(sprite, assets::stand::load(), assets::perch::load(), HEIGHT, |image| {
        if image.width > cutout.0 || image.height > cutout.1 {
            Err(capri_render::RenderError::TooLarge {
                width: image.width,
                height: image.height,
                max: cutout.1,
            })
        } else {
            offscreen.add_image(image)
        }
    })
    .expect("son dessin d'avant suffit");
    assert!(!figure.look.has_stand() && !figure.look.has_perch());
    let mut c = capri(&figure, (900, 900), Vec2::new(450.0, 800.0));
    c.step(0.0, 10);
    figure.look.update(&c, 10, true, true);
    assert_eq!(figure.look.picture(&c).drawing, Drawing::Cutout);
    // posée : debout sur le bord, toujours son dessin d'avant
    let spot = Spot { window: 1, along: 100.0, left: 350.0, width: 400.0 };
    c.land(Perch { spot, feet: Vec2::new(450.0, 800.0) });
    figure.look.update(&c, 11, false, true);
    assert_eq!(figure.look.picture(&c).drawing, Drawing::Cutout);
    assert!(!figure.look.wants_music(&c));
    // sans planches du tout : pareil
    let sprite = assets::load_sprite().expect("son dessin d'avant");
    let mut offscreen = Offscreen::new(64, 64).expect("une carte graphique");
    let figure =
        Figure::new(sprite, None, None, HEIGHT, |image| offscreen.add_image(image)).expect("confiée");
    let layers = figure.layers(&figure.look.picture(&capri(&figure, (900, 900), Vec2::new(450.0, 800.0))));
    assert_eq!(layers.len(), 1);
    assert!(layers[0].1.ghost_strength > 0.0, "son dessin d'avant garde sa copie violette");
    assert_eq!(offscreen.image_bytes(), u64::from(cutout.0) * u64::from(cutout.1) * 4);
}
