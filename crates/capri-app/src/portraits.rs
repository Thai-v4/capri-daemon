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
//!
//! Le film de ses intros, « de tout près », avec et sans son glitch, et toute la suite (le film puis
//! chaque entrée, et leur raccord), sur un faux écran de MacBook Pro (`intro_portraits`) ;
//! `intro_played_for_real` le joue à 24 i/s par le code de l'app et dit ce qu'il a montré et ce
//! qu'il a pesé. Ses entrées, et « se matérialise » (`act_intro_portraits`), sur un faux écran de
//! MacBook Pro, image par image. Ses gestes (`gesture_portraits`) : tenue en l'air, sa
//! mèche et le curseur marqués ; lancée ; elle se relève ; elle sourit ; elle marche ; son éclat
//! assise ; ses clignements glitchés. Son repos (`rest_portraits`) : elle s'allonge, allongée
//! (clignement, parole), endormie, elle se rassoit, son départ allongée, la superposition de sa
//! pose assise et de sa pose allongée sur le même bord ; debout, elle s'endort et se réveille.
//! `rest_weighed_for_real` charge ses planches de repos comme l'app (son thread, la carte
//! graphique) et dit ce qu'elles pèsent, avant, pendant, après. Elle guette (`lurk_portraits`) :
//! derrière le côté gauche et le côté droit d'une fenêtre, sa bouche ouverte, ses clignements
//! (glitché compris), sa glissade image par image, et, en croix, ce que voit son bandeau (le haut
//! du milieu de ce qu'elle occupe) et ce qui l'attrape.

use std::path::{Path, PathBuf};
use std::time::Instant;

use capri_core::look::{Drawing, Picture};
use capri_core::perch::{Perch, Spot};
use capri_core::sheets::{self, stand};
use capri_core::{Capri, Rect, Vec2};
use capri_render::{Crop, ImageId, Keep, Offscreen, SpriteDraw, SpriteImage, Tint, Turn};

use crate::assets;
use crate::figure::Figure;

/// Sa hauteur debout à la taille d'origine sur un écran Retina : 300 points, 600 pixels.
const HEIGHT: f32 = 600.0;
/// Le faux bureau, une fenêtre, et sa barre de titre.
const DESK: [u8; 4] = [38, 44, 62, 255];
const WINDOW: [u8; 4] = [236, 238, 242, 255];
const TITLE: [u8; 4] = [214, 216, 222, 255];
/// Les repères tracés sur les superpositions : la ligne de ses pieds, le haut de ses cheveux ; et,
/// tenue en l'air, le curseur (en jaune) et sa mèche (le point bleu).
const FEET_MARK: [u8; 4] = [255, 64, 64, 255];
const TOP_MARK: [u8; 4] = [64, 200, 255, 255];
const CURSOR_MARK: [u8; 4] = [255, 220, 0, 255];

/// Un pinceau : de quoi peindre des rectangles pleins avec le même rendu qu'elle.
struct Scene {
    offscreen: Offscreen,
    figure: Figure,
    desk: ImageId,
    window: ImageId,
    title: ImageId,
    feet_mark: ImageId,
    top_mark: ImageId,
    cursor_mark: ImageId,
    size: (u32, u32),
}

impl Scene {
    fn new(size: (u32, u32), height: f32) -> Scene {
        let started = Instant::now();
        let sprite = assets::load_sprite().expect("son dessin d'avant");
        let stand = assets::stand::load();
        let perch = assets::perch::load();
        let life = assets::life::load();
        println!(
            "planches lues en {} ms : debout {}, assise {}, vie {}",
            started.elapsed().as_millis(),
            stand.is_some(),
            perch.is_some(),
            life.is_some()
        );
        let mut offscreen = Offscreen::new(size.0, size.1).expect("une carte graphique");
        let figure = Figure::new(sprite, stand, perch, life, height, |image| offscreen.add_image(image))
            .expect("confiées");
        println!("mémoire graphique : {:.1} Mo", offscreen.image_bytes() as f64 / 1_000_000.0);
        let mut plain = |color: [u8; 4]| {
            offscreen.add_image(SpriteImage { width: 1, height: 1, rgba: &color }).expect("une couleur")
        };
        let (desk, window, title) = (plain(DESK), plain(WINDOW), plain(TITLE));
        let (feet_mark, top_mark, cursor_mark) = (plain(FEET_MARK), plain(TOP_MARK), plain(CURSOR_MARK));
        Scene { offscreen, figure, desk, window, title, feet_mark, top_mark, cursor_mark, size }
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
                tint: Tint::NONE,
                turn: Turn::NONE,
                keep: Keep::ALL,
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
        let screen = Rect::new(Vec2::ZERO, Vec2::new(w as f32, h as f32));
        for picture in pictures {
            layers.extend(self.figure.layers(picture, screen));
        }
        self.offscreen.render(&layers, None).expect("rendue")
    }
}

/// Elle va jusqu'à l'image `frame`, et ce qu'elle dessine alors.
fn step(scene: &mut Scene, c: &mut Capri, frame: u64) -> Picture {
    c.step(0.0, frame);
    scene.figure.look.update(c, frame, false, false);
    scene.figure.look.picture(c)
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
        scene.figure.look.update(&mut c, frame, talking, false);
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
        scene.figure.look.update(&mut c, frame, talking, false);
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
        zoomed.figure.look.update(&mut c, frame, talking, false);
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
    scene.figure.look.update(&mut c, 50, false, false);
    let held = scene.render(None, &[scene.figure.look.picture(&c)]);
    c.drag_to(grip + Vec2::new(-200.0, -10.0), 0.04);
    c.release(0.05);
    c.step(1.0 / 120.0, 51);
    scene.figure.look.update(&mut c, 51, false, false);
    let thrown = scene.render(None, &[scene.figure.look.picture(&c)]);
    let (sheet, sheet_size) = contact(&[held, thrown], size, 2);
    save(&dir, "tenue-et-lancee.png", &sheet, sheet_size);
}

/// Pour les essais de la pose assise : posée sur le bord de `window`, son point posé en `seat`.
fn perched(scene: &mut Scene, window: Rect, seat: Vec2, from: u64) -> Capri {
    let mut c = capri(&scene.figure, scene.size, seat - Vec2::new(0.0, 4.0));
    c.step(0.0, from);
    // comme dans l'app : elle n'était pas posée juste avant (lâchée près du bord)
    scene.figure.look.update(&mut c, from, false, false);
    let spot = Spot { window: 1, along: seat.x - window.min.x, left: window.min.x, width: window.size.x };
    c.land(Perch { spot, feet: seat });
    scene.figure.look.update(&mut c, from, false, false);
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
        scene.figure.look.update(&mut c, frame, talking, music);
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
        scene.figure.look.update(&mut c, frame, false, false);
        frames.push(scene.render(Some(window), &[scene.figure.look.picture(&c)]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 7);
    save(&dir, "arrivee.png", &sheet, sheet_size);
    let left = 2000;
    c.step(0.0, left);
    scene.figure.look.update(&mut c, left, false, false);
    c.take_off();
    scene.figure.look.update(&mut c, left, false, false);
    let mut frames = Vec::new();
    for frame in (left..left + sheets::leaving().len() + 4).step_by(2) {
        c.step(0.0, frame);
        scene.figure.look.update(&mut c, frame, false, false);
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
        scene.figure.look.update(&mut c, 24, false, false);
        scene.figure.look.picture(&c)
    };
    let mut c = perched(&mut scene, window, Vec2::new(1000.0, window.min.y), 0);
    c.step(0.0, since + 24);
    scene.figure.look.update(&mut c, since + 24, false, false);
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
    // pour comparer : assise à 1,11 (le rapport du projet d'origine) au lieu de STAND_OVER_PERCH (sa
    // ligne d'assise au même endroit)
    let other = stand::STAND_OVER_PERCH / 1.11;
    let seat = Vec2::new(1000.0, window.min.y);
    let larger = Picture {
        rect: Rect::new(
            seat + (sitting.rect.min - seat) * other,
            Vec2::new((sitting.rect.size.x * other).round(), (sitting.rect.size.y * other).round()),
        ),
        ..sitting
    };
    let both = scene.render(Some(window), &[standing, larger]);
    save(&dir, "cote-a-cote-a-1-11.png", &both, wide);
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
    let (stand, perch, life) = (assets::stand::load(), assets::perch::load(), assets::life::load());
    let mut figure = Figure::new(sprite, stand, perch, life, HEIGHT, |image| {
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
    assert!(!figure.look.has_stand() && !figure.look.has_perch() && !figure.look.has_life());
    let mut c = capri(&figure, (900, 900), Vec2::new(450.0, 800.0));
    c.step(0.0, 10);
    figure.look.update(&mut c, 10, true, true);
    assert_eq!(figure.look.picture(&c).drawing, Drawing::Cutout);
    // posée : debout sur le bord, toujours son dessin d'avant
    let spot = Spot { window: 1, along: 100.0, left: 350.0, width: 400.0 };
    c.land(Perch { spot, feet: Vec2::new(450.0, 800.0) });
    figure.look.update(&mut c, 11, false, true);
    assert_eq!(figure.look.picture(&c).drawing, Drawing::Cutout);
    assert!(!figure.look.wants_music(&c));
    // sans planches du tout : pareil
    let sprite = assets::load_sprite().expect("son dessin d'avant");
    let mut offscreen = Offscreen::new(64, 64).expect("une carte graphique");
    let figure =
        Figure::new(sprite, None, None, None, HEIGHT, |image| offscreen.add_image(image)).expect("confiée");
    let screen = Rect::new(Vec2::ZERO, Vec2::new(900.0, 900.0));
    let layers =
        figure.layers(&figure.look.picture(&capri(&figure, (900, 900), Vec2::new(450.0, 800.0))), screen);
    assert_eq!(layers.len(), 1);
    assert!(layers[0].1.ghost_strength > 0.0, "son dessin d'avant garde sa copie violette");
    assert_eq!(offscreen.image_bytes(), u64::from(cutout.0) * u64::from(cutout.1) * 4);
}

/// Ses planches de vie : son arrivée et son départ, image par image, leur début, leur milieu et
/// leur fin ; et la coupe entre la dernière image de son arrivée et la première de sa boucle (puis
/// entre sa boucle et la première image de son départ), côte à côte et superposées, avec la
/// ligne de ses pieds (en rouge) et le haut de ses cheveux dans sa boucle (en bleu).
#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies planches et la vraie carte graphique"]
fn life_portraits() {
    use capri_core::sheets::life;

    let dir = directory();
    let size = (900, 900);
    let mut scene = Scene::new(size, HEIGHT);
    assert!(scene.figure.look.has_life(), "il faut les planches de vie et la pose debout");
    let feet = Vec2::new(450.0, 820.0);
    let mut c = capri(&scene.figure, size, feet);
    c.step(0.0, 0);
    scene.figure.look.enter(&mut c, 0);

    // son arrivée, chaque dessin (un toutes les deux images)
    let arrive = life::arriving().len();
    let mut drawings = Vec::new();
    for frame in (0..arrive).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        drawings.push((picture, scene.render(None, &[picture])));
    }
    let images: Vec<Vec<u8>> = drawings.iter().map(|(_, image)| image.clone()).collect();
    let (sheet, sheet_size) = contact(&images, size, 7);
    save(&dir, "vie-arrivee.png", &sheet, sheet_size);
    let (sheet, sheet_size) = contact(&[images[0].clone(), images[7].clone(), images[13].clone()], size, 3);
    save(&dir, "vie-arrivee-debut-milieu-fin.png", &sheet, sheet_size);
    let last_arrive = drawings[13].0;

    // la première image de sa boucle, juste après
    let first_loop = step(&mut scene, &mut c, arrive);
    assert!(matches!(first_loop.drawing, Drawing::Stand(_)), "{first_loop:?}");
    let cut = superposition(&mut scene, last_arrive, first_loop);
    save(&dir, "vie-superposition-arrivee.png", &cut.0, cut.1);

    // son départ, bien plus tard, au même endroit
    let before = step(&mut scene, &mut c, 300);
    scene.figure.look.vanish(&mut c, 300);
    let leave = life::leaving().len();
    let mut drawings = Vec::new();
    for frame in (300..300 + leave).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        drawings.push((picture, scene.render(None, &[picture])));
    }
    let images: Vec<Vec<u8>> = drawings.iter().map(|(_, image)| image.clone()).collect();
    let (sheet, sheet_size) = contact(&images, size, 6);
    save(&dir, "vie-depart.png", &sheet, sheet_size);
    // la fin : le dernier dessin qui montre quelque chose (le dernier de tous est vide)
    let (sheet, sheet_size) = contact(&[images[0].clone(), images[5].clone(), images[10].clone()], size, 3);
    save(&dir, "vie-depart-debut-milieu-fin.png", &sheet, sheet_size);
    let cut = superposition(&mut scene, drawings[0].0, before);
    save(&dir, "vie-superposition-depart.png", &cut.0, cut.1);
    step(&mut scene, &mut c, 300 + leave);
    assert!(scene.figure.look.is_gone());
    let k = stand::scale(HEIGHT);
    println!(
        "vie : {:.3} px d'écran par px de planche ; ses pieds sur la ligne {:.0}, la toile décalée de {:.1} px",
        k,
        before.rect.min.y + (life::STAND_LOWEST.1 + 1) as f32 * k,
        life::STAND_LEFT as f32 * k
    );
}

/// `life` (un dessin de ses planches de vie) et `standing` (un dessin de sa boucle debout), au
/// même endroit : trois cases côte à côte, `life` seul, `standing` seul, puis `standing` à moitié
/// transparent par-dessus `life` ; sur chacune, la ligne de ses pieds et le haut de ses cheveux
/// dans sa boucle. Recadrée autour d'elle.
fn superposition(scene: &mut Scene, life: Picture, standing: Picture) -> (Vec<u8>, (u32, u32)) {
    use capri_core::sheets::life as sheet;

    let rect = standing.rect;
    let k = rect.size.y / sheets::CELL_HEIGHT as f32;
    let feet_y = (rect.min.y + (sheet::STAND_LOWEST.1 + 1) as f32 * k).round();
    let feet_x = (rect.min.x + sheet::STAND_LOWEST.0 * rect.size.x / stand::CELL_WIDTH as f32).round();
    let top_y = (rect.min.y + stand::TOP as f32 * k).round();
    let width = scene.size.0 as f32;
    let marks = |scene: &Scene| {
        vec![
            Scene::rect(scene.feet_mark, Rect::new(Vec2::new(0.0, feet_y), Vec2::new(width, 2.0))),
            Scene::rect(scene.feet_mark, Rect::new(Vec2::new(feet_x, feet_y - 40.0), Vec2::new(2.0, 80.0))),
            Scene::rect(scene.top_mark, Rect::new(Vec2::new(0.0, top_y), Vec2::new(width, 2.0))),
        ]
    };
    let mut panels = Vec::new();
    for pictures in [vec![(life, 1.0)], vec![(standing, 1.0)], vec![(life, 1.0), (standing, 0.5)]] {
        let (w, h) = scene.size;
        let screen = Rect::new(Vec2::ZERO, Vec2::new(w as f32, h as f32));
        let mut layers = vec![Scene::rect(scene.desk, screen)];
        for (picture, opacity) in pictures {
            layers.extend(
                scene
                    .figure
                    .layers(&picture, screen)
                    .into_iter()
                    .map(|(id, draw)| (id, SpriteDraw { opacity, ..draw })),
            );
        }
        layers.extend(marks(scene));
        panels.push(scene.offscreen.render(&layers, None).expect("rendue"));
    }
    // recadrée : sa case de vie entière, et un peu de marge
    let frame = sheet::over_stand(rect);
    let at = ((frame.min.x - 20.0).max(0.0) as u32, (frame.min.y - 20.0).max(0.0) as u32);
    let crop_size = (
        ((frame.size.x + 40.0) as u32).min(scene.size.0 - at.0),
        ((frame.size.y + 40.0) as u32).min(scene.size.1 - at.1),
    );
    let cropped: Vec<Vec<u8>> = panels.iter().map(|panel| crop(panel, scene.size.0, at, crop_size)).collect();
    contact(&cropped, crop_size, 3)
}

/// Elle guette (le lot 9) : derrière le côté gauche, puis le côté droit d'une fenêtre dont le haut
/// touche presque le haut de la scène, comme dans l'app (`capri_core::perch::flanks`, son point
/// d'accroche au milieu du côté). Sortie : sa boucle, sa bouche ouverte, son clignement, son
/// clignement glitché ; le haut du milieu de ce qu'elle occupe (là où se pose son bandeau) en croix
/// bleue. Sa glissade, de rien de visible à penchée dehors. Et une grille de ce qui l'attrape (des
/// points jaunes là où le curseur la prendrait), pour voir que ce qui est caché laisse passer.
#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies planches et la vraie carte graphique"]
fn lurk_portraits() {
    use capri_core::perch::{self, Seen, Side};

    let dir = directory();
    let size = (900, 900);
    let mut scene = Scene::new(size, HEIGHT);
    let look = &scene.figure.look;
    let (Some(lurking), Some(fit)) = (look.lurk_stature(), look.lurk_fit()) else {
        panic!("il faut ses planches debout")
    };
    let screen = Rect::new(Vec2::ZERO, Vec2::new(size.0 as f32, size.1 as f32));
    for side in Side::ALL {
        // la fenêtre de l'autre côté du milieu de la scène
        let window = match side {
            Side::Left => Rect::new(Vec2::new(450.0, 24.0), Vec2::new(430.0, 860.0)),
            Side::Right => Rect::new(Vec2::new(20.0, 24.0), Vec2::new(430.0, 860.0)),
        };
        let seen = [Seen { id: 1, rect: window }];
        let flank = perch::flanks(&seen, screen, lurking)
            .into_iter()
            .find(|flank| flank.side == side)
            .expect("un côté où elle tient");
        let lurk = flank.at((flank.lo + flank.hi) / 2.0);
        let name = match side {
            Side::Left => "gauche",
            Side::Right => "droite",
        };
        let mut c = capri(&scene.figure, size, Vec2::new(450.0, 820.0));
        c.lurk_on(lurk, fit);
        c.emerge();
        // sa glissade, au pas de l'écran (60 Hz), une image sur trois
        let mut glide = Vec::new();
        let mut frame = 0;
        while c.is_gliding() || glide.is_empty() {
            let picture = step(&mut scene, &mut c, frame / 2);
            if frame % 3 == 0 {
                glide.push(scene.render(Some(window), &[picture]));
            }
            c.step(1.0 / 60.0, frame / 2);
            frame += 1;
        }
        let (sheet, sheet_size) = contact(&glide, size, 6);
        save(&dir, &format!("guette-{name}-glissade.png"), &sheet, sheet_size);
        // sortie : sa boucle (dessin 0), sa bouche, son clignement, son clignement glitché
        let mut shots = Vec::new();
        // son tour suit l'image d'animation : 960, le début d'un tour où elle cligne (le 16e)
        for (tour_frame, talking) in [(0u64, false), (6, true), (28, false), (5 * 60 + 28, false)] {
            let frame = 960 + tour_frame;
            c.step(0.0, frame);
            scene.figure.look.update(&mut c, frame, talking, false);
            let picture = scene.figure.look.picture(&c);
            let shown = scene.figure.look.shown(&c);
            let mut layers = vec![Scene::rect(scene.desk, screen)];
            layers.push(Scene::rect(scene.window, window));
            layers.push(Scene::rect(scene.title, Rect::new(window.min, Vec2::new(window.size.x, 56.0))));
            layers.extend(scene.figure.layers(&picture, screen));
            layers.extend(cross(
                scene.top_mark,
                Vec2::new(shown.min.x + shown.size.x / 2.0, shown.min.y),
                8.0,
            ));
            shots.push(scene.offscreen.render(&layers, None).expect("rendue"));
            if tour_frame == 0 {
                let lean = picture.lean.expect("penchée");
                println!(
                    "guette à {name} : le bord en x = {:.0}, sa hanche en ({:.0}, {:.0}), penchée de {:.1}°, \
                     ce qu'elle occupe de ({:.0}, {:.0}) à ({:.0}, {:.0})",
                    lean.wall,
                    lean.pivot.x,
                    lean.pivot.y,
                    lean.angle.to_degrees(),
                    shown.min.x,
                    shown.min.y,
                    shown.max().x,
                    shown.max().y
                );
            }
        }
        save(&dir, &format!("guette-{name}.png"), &shots[0], size);
        let (sheet, sheet_size) = contact(&shots, size, 4);
        save(&dir, &format!("guette-{name}-planche.png"), &sheet, sheet_size);
        // ce qui l'attrape : un point jaune tous les 6 pixels là où le curseur la prendrait
        let frame = 960;
        c.step(0.0, frame);
        scene.figure.look.update(&mut c, frame, false, false);
        let picture = scene.figure.look.picture(&c);
        let mut layers = vec![Scene::rect(scene.desk, screen), Scene::rect(scene.window, window)];
        layers.extend(scene.figure.layers(&picture, screen));
        let mut caught = 0;
        for y in (0..size.1).step_by(6) {
            for x in (0..size.0).step_by(6) {
                let at = Vec2::new(x as f32, y as f32);
                if scene.figure.look.is_over(&c, at) {
                    caught += 1;
                    layers.push(Scene::rect(scene.cursor_mark, Rect::new(at, Vec2::new(2.0, 2.0))));
                    assert!(
                        picture.lean.is_some_and(|lean| lean.shows(at)),
                        "attrapée derrière la fenêtre en {at:?}"
                    );
                }
            }
        }
        assert!(caught > 50, "{caught}");
        let pixels = scene.offscreen.render(&layers, None).expect("rendue");
        save(&dir, &format!("guette-{name}-attrape.png"), &pixels, size);
    }
}

/// Une croix de `color` (une image de `Scene`) centrée en `at`, de `arm` pixels de chaque côté.
fn cross(id: ImageId, at: Vec2, arm: f32) -> [(ImageId, SpriteDraw); 2] {
    [
        Scene::rect(id, Rect::new(Vec2::new(at.x - arm, at.y - 1.0), Vec2::new(2.0 * arm, 2.0))),
        Scene::rect(id, Rect::new(Vec2::new(at.x - 1.0, at.y - arm), Vec2::new(2.0, 2.0 * arm))),
    ]
}

/// Ses gestes, image par image (un dessin sur deux images, trois pour son sourire) : tenue en
/// l'air (le curseur en croix jaune, sa mèche en croix bleue, qui se rejoignent à mesure que le
/// curseur bouge, et se recouvrent à la fin), lancée,
/// elle se relève (puis sa boucle : la coupe, superposée), elle sourit, elle marche (sur place
/// dans la planche, et sa position qui avance), son éclat assise, ses clignements glitchés.
#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies planches et la vraie carte graphique"]
fn gesture_portraits() {
    use capri_core::look::Gesture;
    use capri_core::sheets::life;

    let dir = directory();
    let size = (900, 900);
    let mut scene = Scene::new(size, HEIGHT);
    let look = &scene.figure.look;
    assert!(
        life::Strip::MOVES.iter().all(|&strip| look.has_move(strip)) && look.can(Gesture::PerchGlitch),
        "il faut les planches de ses gestes"
    );

    // tenue en l'air, prise par la taille : son corps ne bouge pas à la prise ; puis le curseur
    // monte, puis va à droite, et elle glisse jusqu'à avoir sa mèche sous lui (une image sur quatre)
    let feet = Vec2::new(450.0, 820.0);
    let mut c = capri(&scene.figure, size, feet);
    c.step(0.0, 0);
    scene.figure.look.update(&mut c, 0, false, false);
    let mut cursor = c.position() + Vec2::new(c.size().x * 0.5, c.size().y * 0.5);
    let grab = capri_core::input::Grab { at: cursor, since: 0.0, trail: Vec::new() };
    assert!(scene.figure.look.hold(&mut c, &grab));
    let mut held = Vec::new();
    for frame in 1..97u64 {
        cursor += if frame <= 48 { Vec2::new(0.0, -6.0) } else { Vec2::new(6.0, 0.0) };
        c.drag_to(cursor, frame as f64 / 24.0);
        let picture = step(&mut scene, &mut c, frame);
        let grip = scene.figure.look.grip(&c).expect("elle pend");
        if frame % 4 == 1 || frame == 96 {
            let screen = Rect::new(Vec2::ZERO, Vec2::new(900.0, 900.0));
            let mut layers = vec![Scene::rect(scene.desk, screen)];
            layers.extend(scene.figure.layers(&picture, screen));
            layers.extend(cross(scene.cursor_mark, cursor, 14.0));
            layers.extend(cross(scene.top_mark, grip, 6.0));
            held.push(scene.offscreen.render(&layers, None).expect("rendue"));
        }
        if frame == 1 || frame == 96 {
            println!(
                "tenue, image {frame} : sa mèche en ({:.1}, {:.1}), le curseur en ({:.1}, {:.1})",
                grip.x, grip.y, cursor.x, cursor.y
            );
        }
    }
    save(&dir, "geste-tenue.png", held.last().expect("rendue"), size);
    let (sheet, sheet_size) = contact(&held, size, 6);
    save(&dir, "geste-tenue-planche.png", &sheet, sheet_size);
    // lancée vers la gauche : étirée dans le sens de sa course, toujours en l'air
    c.drag_to(cursor + Vec2::new(-200.0, 0.0), 96.0 / 24.0 + 0.04);
    c.release(96.0 / 24.0 + 0.05);
    let thrown = step(&mut scene, &mut c, 97);
    let flying = scene.render(None, &[thrown]);
    let (sheet, sheet_size) = contact(&[held.last().expect("rendue").clone(), flying], size, 2);
    save(&dir, "geste-tenue-et-lancee.png", &sheet, sheet_size);

    // lâchée doucement : elle se relève, puis sa boucle
    let mut c = capri(&scene.figure, size, feet);
    c.step(0.0, 100);
    scene.figure.look.update(&mut c, 100, false, false);
    let grip_at = c.position() + Vec2::new(100.0, 100.0);
    let grab = capri_core::input::Grab { at: grip_at, since: 0.0, trail: Vec::new() };
    assert!(scene.figure.look.hold(&mut c, &grab));
    step(&mut scene, &mut c, 101);
    c.release(100.0);
    let mut rise = Vec::new();
    let mut last = None;
    for frame in (102..102 + Gesture::Rise.frames()).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        last = Some(picture);
        rise.push(scene.render(None, &[picture]));
    }
    let (sheet, sheet_size) = contact(&rise, size, 5);
    save(&dir, "geste-se-releve.png", &sheet, sheet_size);
    let after = step(&mut scene, &mut c, 102 + Gesture::Rise.frames());
    let cut = superposition(&mut scene, last.expect("ses dessins"), after);
    save(&dir, "geste-se-releve-superposition.png", &cut.0, cut.1);

    // un clic : elle sourit, deux tours, un dessin sur trois images
    let mut c = capri(&scene.figure, size, feet);
    c.step(0.0, 300);
    scene.figure.look.update(&mut c, 300, false, false);
    assert_eq!(scene.figure.look.poke(&mut c, 300), Some(Gesture::Grin));
    let mut grin = Vec::new();
    for frame in (300..348).step_by(3) {
        let picture = step(&mut scene, &mut c, frame);
        grin.push(scene.render(None, &[picture]));
    }
    let (sheet, sheet_size) = contact(&grin, size, 8);
    save(&dir, "geste-sourit.png", &sheet, sheet_size);
    let last = step(&mut scene, &mut c, 300 + Gesture::Grin.frames() - 1);
    let after = step(&mut scene, &mut c, 300 + Gesture::Grin.frames());
    let cut = superposition(&mut scene, last, after);
    save(&dir, "geste-sourit-superposition.png", &cut.0, cut.1);

    // elle marche vers la gauche : ses douze dessins, puis là où elle est toutes les 6 images
    let wide = (1800, 900);
    let mut street = Scene::new(wide, HEIGHT);
    let mut c = capri(&street.figure, wide, Vec2::new(1500.0, 820.0));
    c.step(0.0, 500);
    street.figure.look.update(&mut c, 500, false, false);
    let to = c.feet().x - 1000.0;
    assert!(street.figure.look.walk(&mut c, 500, to));
    let speed = street.figure.look.walk_speed().expect("sa planche de marche");
    let mut drawings = Vec::new();
    let mut steps = Vec::new();
    let mut frame = 500;
    while c.is_walking() && frame < 524 {
        c.step(1.0 / 24.0, frame);
        street.figure.look.update(&mut c, frame, false, false);
        let picture = street.figure.look.picture(&c);
        if (frame - 500) % 2 == 0 {
            drawings.push(street.render(None, &[picture]));
        }
        if (frame - 500) % 6 == 0 {
            steps.push(picture);
        }
        frame += 1;
    }
    let small = drawings.iter().map(|image| crop(image, wide.0, (1100, 100), (500, 800))).collect::<Vec<_>>();
    let (sheet, sheet_size) = contact(&small, (500, 800), 6);
    save(&dir, "geste-marche.png", &sheet, sheet_size);
    let trail = street.render(None, &steps);
    save(&dir, "geste-marche-trajet.png", &trail, wide);
    println!("marche : {speed:.1} px par seconde, {:.1} px par pas", life::WALK_STEP * stand::scale(HEIGHT));

    // assise : son éclat, puis sa boucle
    let window = Rect::new(Vec2::new(140.0, 480.0), Vec2::new(640.0, 420.0));
    let seat = Vec2::new(430.0, window.min.y);
    let mut c = perched(&mut scene, window, seat, 2000);
    let since = 2000 + sheets::arriving().len();
    step(&mut scene, &mut c, since + 10);
    assert_eq!(scene.figure.look.poke(&mut c, since + 10), Some(Gesture::PerchGlitch));
    let mut glitch = Vec::new();
    for frame in (since + 10..since + 10 + Gesture::PerchGlitch.frames() + 2).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        glitch.push(scene.render(Some(window), &[picture]));
    }
    let (sheet, sheet_size) = contact(&glitch, size, 6);
    save(&dir, "geste-eclat-assise.png", &sheet, sheet_size);

    // ses clignements glitchés : debout puis assise, au tour 5 (les dessins 24 à 31 debout, 28 à 35
    // assise), et le clignement ordinaire du tour 0 à côté ; une scène neuve, son dessin repartant
    // de zéro
    let mut scene = Scene::new(size, HEIGHT);
    let mut c = capri(&scene.figure, size, feet);
    let mut faces = Vec::new();
    for tour in [0u64, 5] {
        for drawing in [24u64, 26, 28, 30] {
            let picture = step(&mut scene, &mut c, tour * 60 + drawing);
            let s = picture.rect.size.y / sheets::CELL_HEIGHT as f32;
            let center = picture.rect.min + Vec2::new(66.0 * s, 124.0 * s);
            let image = scene.render(None, &[picture]);
            faces.push(crop(
                &image,
                size.0,
                ((center.x - 150.0) as u32, (center.y - 150.0) as u32),
                (300, 300),
            ));
        }
    }
    let mut c = perched(&mut scene, window, seat, 3000);
    let since = 3000 + sheets::arriving().len();
    for tour in [0u64, 5] {
        for drawing in [28u64, 30, 32, 34] {
            let picture = step(&mut scene, &mut c, since + tour * 60 + drawing);
            let s = picture.rect.size.y / sheets::CELL_HEIGHT as f32;
            let center = picture.rect.min + Vec2::new(134.0 * s, 160.0 * s);
            let image = scene.render(Some(window), &[picture]);
            faces.push(crop(
                &image,
                size.0,
                ((center.x - 150.0) as u32, (center.y - 150.0) as u32),
                (300, 300),
            ));
        }
    }
    let (sheet, sheet_size) = contact(&faces, (300, 300), 8);
    save(&dir, "geste-clignements.png", &sheet, sheet_size);
}

impl crate::intro_view::Canvas for Offscreen {
    type Id = ImageId;

    fn create(&mut self, (width, height): (u32, u32)) -> Result<ImageId, capri_render::RenderError> {
        self.add_canvas(width, height)
    }

    fn paint(&mut self, id: ImageId, premultiplied: &[u8]) -> Result<(), capri_render::RenderError> {
        self.paint_canvas(id, premultiplied)
    }

    fn remove(&mut self, id: ImageId) {
        self.remove_image(id);
    }

    fn settle(&mut self) {
        Offscreen::settle(self);
    }
}

/// La graine des portraits du film : son glitch, tel qu'un lancement de cette graine le tirerait.
const FILM_SEED: u64 = 0x5eed_0008;

/// Les dessins du film que l'utilisateur veut voir en entier, par leur numéro (depuis 1).
const FILM_DRAWINGS: [u32; 9] = [1, 2, 4, 8, 11, 14, 18, 21, 22];

/// Le film « de tout près » et toute la suite, sur un faux écran de MacBook Pro 14" (3024 × 1964),
/// peints sur une seule toile et dessinés par le même code que l'app (`intro_view::film_layers`) :
/// - ses dessins 1, 2, 4, 8, 11, 14, 18, 21 et 22 entiers, avec leur glitch
///   (`film-glitch-<dessin>.png`) et sans (`film-<dessin>.png`) ; les deux extrêmes aussi sur un
///   écran 1080p (`film-glitch-1920x1080-<dessin>.png`), où l'image ne dépasse pas sur les côtés ;
/// - ses 22 dessins avec leur glitch, au huitième (`film-planche.png`) ;
/// - pour chaque entrée, toute la suite au huitième : le film puis l'entrée, une image sur deux,
///   jusqu'à ce qu'elle soit là (`suite-<entrée>-planche.png`) ; et le raccord, au quart : les
///   deux derniers dessins du film, puis les premières images de l'entrée
///   (`raccord-<entrée>.png`).
///
/// Dit aussi ce que fait le glitch à chaque dessin, et le temps de décoder chacun.
#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies images et la vraie carte graphique"]
fn intro_portraits() {
    use capri_core::intro::glitch::{self, Glitch};
    use capri_core::intro::{self, Play};

    use crate::intro_view::{film_layers, layer};

    let dir = directory();
    let film = intro::FILM;
    let sequence = assets::intro::sequence(film).expect("un dossier d'images");
    sequence.check().expect("toutes les images du film, à la bonne taille");
    let decoded = |position: u32| {
        let started = Instant::now();
        let mut pixels = sequence.decode(position).expect("lisible");
        capri_render::premultiply_in_place(&mut pixels);
        (pixels, started.elapsed())
    };
    let size = (3024u32, 1964u32);
    let mut scene = Scene::new(size, HEIGHT);
    let canvas = scene.offscreen.add_canvas(film.size.0, film.size.1).expect("la toile");
    let whole = Rect::new(Vec2::ZERO, Vec2::new(size.0 as f32, size.1 as f32));
    let rect = intro::place(whole.size, film.size);
    println!("{} sur un écran {} × {} : en {rect:?}", film.name, size.0, size.1);
    println!(
        "graine {FILM_SEED:#x} : rafales aux dessins {:?} (depuis 0)",
        glitch::UP_CLOSE.bursts(FILM_SEED)
    );
    let mut wide = Offscreen::new(1920, 1080).expect("une carte graphique");
    let wide_desk = wide.add_image(SpriteImage { width: 1, height: 1, rgba: &DESK }).expect("le bureau");
    let wide_canvas = wide.add_canvas(film.size.0, film.size.1).expect("la toile");
    let wide_whole = Rect::new(Vec2::ZERO, Vec2::new(1920.0, 1080.0));
    let wide_rect = intro::place(wide_whole.size, film.size);

    let mut film_thumbs = Vec::new();
    let mut ending = Vec::new();
    let mut times = Vec::new();
    let mut most = 0;
    for position in 0..film.len() {
        let number = position + 1;
        let (pixels, took) = decoded(position);
        times.push(took.as_secs_f64() * 1000.0);
        scene.offscreen.paint_canvas(canvas, &pixels).expect("peinte");
        let glitch = Glitch::of(film, position, FILM_SEED, glitch::STRENGTH);
        let drawn = film_layers(canvas, rect, film.size, &glitch);
        most = most.max(drawn.len());
        println!(
            "dessin {number:2} : force {:.2}, {} bandes dont {} décalées et {} manquantes, doubles {:?} / {:?}, \
             tremblement {:?} : {} dessins",
            glitch.force,
            glitch.bands.len(),
            glitch.bands.iter().filter(|band| band.shift != 0 && !band.missing).count(),
            glitch.bands.iter().filter(|band| band.missing).count(),
            (glitch.doubles[0].offset, (glitch.doubles[0].opacity * 100.0).round()),
            (glitch.doubles[1].offset, (glitch.doubles[1].opacity * 100.0).round()),
            glitch.shake,
            drawn.len()
        );
        let mut layers = vec![Scene::rect(scene.desk, whole)];
        layers.extend(drawn);
        let image = scene.offscreen.render(&layers, None).expect("rendue");
        if FILM_DRAWINGS.contains(&number) {
            save(&dir, &format!("film-glitch-{number:02}.png"), &image, size);
            let plain = [Scene::rect(scene.desk, whole), layer(canvas, rect)];
            let image = scene.offscreen.render(&plain, None).expect("rendue");
            save(&dir, &format!("film-{number:02}.png"), &image, size);
        }
        if number == 1 || number == film.len() {
            wide.paint_canvas(wide_canvas, &pixels).expect("peinte");
            let mut layers = vec![Scene::rect(wide_desk, wide_whole)];
            layers.extend(film_layers(wide_canvas, wide_rect, film.size, &glitch));
            let image = wide.render(&layers, None).expect("rendue");
            save(&dir, &format!("film-glitch-1920x1080-{number:02}.png"), &image, (1920, 1080));
        }
        drop(pixels);
        film_thumbs.push(eighth(&image, size));
        if number + 2 > film.len() {
            ending.push(quarter(&image, size));
        }
    }
    let (sheet, sheet_size) = contact(&film_thumbs, (size.0 / 8, size.1 / 8), 6);
    save(&dir, "film-planche.png", &sheet, sheet_size);
    let slowest = times.iter().cloned().fold(0.0, f64::max);
    println!(
        "décoder et prémultiplier une image : {:.1} ms en moyenne, {slowest:.1} ms au plus (chacune est tenue {:.0} \
         ms) ; au plus {most} dessins pour une image du film",
        times.iter().sum::<f64>() / times.len() as f64,
        1000.0 * 2.0 / capri_core::FRAME_RATE
    );
    // la toile lâchée, comme l'app à la fin du film
    scene.offscreen.remove_image(canvas);
    scene.offscreen.settle();

    // pour chaque entrée : toute la suite, et le raccord
    let look = &scene.figure.look;
    let body = look.standing_size();
    let feet_uv = look.standing_feet();
    let start = Vec2::new(size.0 as f32 - body.x * 1.8, size.1 as f32 - body.y * 1.3);
    let home = start + Vec2::new(body.x * feet_uv.x, body.y * feet_uv.y);
    for entry in intro::LIST {
        let mut scene = Scene::new(size, HEIGHT);
        let mut c = capri(&scene.figure, size, home);
        c.step(0.0, 0);
        match entry.play {
            Play::Act(act) => assert!(scene.figure.look.perform(&mut c, act, 0), "{}", entry.key),
            Play::Plain => scene.figure.look.enter(&mut c, 0),
        }
        let mut thumbs = film_thumbs.clone();
        let mut joined = ending.clone();
        let mut frame = 0u64;
        let mut over = None;
        loop {
            let picture = scene.figure.look.picture(&c);
            if frame.is_multiple_of(2) || frame < 2 {
                let image = scene.render(None, &[picture]);
                if [0, 1, 2, 4, 6, 8].contains(&frame) {
                    joined.push(quarter(&image, size));
                }
                if frame.is_multiple_of(2) {
                    thumbs.push(eighth(&image, size));
                }
            }
            let done = !scene.figure.look.is_performing() && scene.figure.look.is_here();
            if done && over.is_none() {
                over = Some(frame);
            }
            if over.is_some_and(|at| frame >= at + 12) || frame > 400 {
                break;
            }
            frame += 1;
            c.step(1.0 / 24.0, frame);
            scene.figure.look.update(&mut c, frame, false, false);
            let _ = scene.figure.look.take_performed();
        }
        let (sheet, sheet_size) = contact(&thumbs, (size.0 / 8, size.1 / 8), 8);
        save(&dir, &format!("suite-{}-planche.png", entry.key), &sheet, sheet_size);
        let (strip, strip_size) = contact(&joined, (size.0 / 4, size.1 / 4), 4);
        save(&dir, &format!("raccord-{}.png", entry.key), &strip, strip_size);
        println!("{} : là à l'image {over:?} après le film", entry.name);
    }
}

/// Le film joué pour de vrai, deux fois de suite, à 24 images par seconde, par `intro_view` (son
/// thread, sa toile) et dessiné avec son glitch (`IntroView::layers`), sur la vraie carte graphique
/// : combien de dessins montrés, à l'heure ou non, combien de dessins par image au plus, et
/// l'empreinte mémoire du processus (`phys_footprint`, lue par `/usr/bin/footprint`, mémoire
/// graphique comprise sur un Mac à puce Apple) avant, au plus haut pendant (aussi souvent que
/// l'outil répond), et après ; puis ce qui a changé, par catégorie. Ensuite, la carte graphique ne
/// garde rien du film : ni sa toile, ni les réglages de ses bandes (`SLOTS_KEPT` au plus).
#[test]
#[ignore = "joue les intros sur la vraie carte graphique, avec les vraies images"]
fn intro_played_for_real() {
    use capri_core::intro::{self, End};

    use crate::intro_view::IntroView;

    // le rapport de `footprint` : son total, et ses lignes (« sale  propre  récupérable  régions
    // catégorie »)
    let report = || -> String {
        let output = std::process::Command::new("/usr/bin/footprint")
            .args(["-p", &std::process::id().to_string(), "-f", "bytes"])
            .output()
            .expect("footprint");
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    let total = |report: &str| -> u64 {
        report
            .split("Footprint: ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    };
    let dirty = |report: &str| -> std::collections::BTreeMap<String, i64> {
        report
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split_whitespace().collect();
                let bytes = fields.first()?.parse::<i64>().ok()?;
                (fields.get(1) == Some(&"B") && fields.len() > 7).then(|| (fields[7..].join(" "), bytes))
            })
            .collect()
    };
    let rss = move || total(&report());
    let mut offscreen = Offscreen::new(64, 64).expect("une carte graphique");
    // la carte graphique prête à dessiner (son pipeline, la relecture) avant de mesurer quoi que ce soit
    let _ = offscreen.render(&[], None).expect("rendue");
    // l'allocateur de macOS garde en réserve le dernier grand bloc lâché (« Malloc Large ») : sans
    // un premier bloc lâché avant de mesurer, cette réserve compterait dans « après ». L'app en a
    // déjà lâché de bien plus grands avant l'intro (ses planches, une fois confiées)
    let block = std::hint::black_box(vec![7u8; 1920 * 1080 * 4]);
    drop(block);
    std::thread::sleep(std::time::Duration::from_millis(300));
    // deux fois : la seconde dit si quelque chose reste d'une fois à l'autre, ou si ce qui reste
    // après la première est la réserve de l'allocateur
    let film = intro::FILM;
    for round in 0..2u64 {
        let sequence = assets::intro::sequence(film).expect("un dossier d'images");
        // la mémoire se lit sur un autre thread pendant qu'elle se joue
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let first = report();
        let before = total(&first);
        let sampler = std::thread::spawn(move || {
            let (mut highest, mut samples) = (0, 0);
            while stopped.try_recv().is_err() {
                highest = highest.max(rss());
                samples += 1;
            }
            (highest, samples)
        });
        let started = Instant::now();
        let mut view: IntroView = IntroView::start(sequence, 0, FILM_SEED + round).expect("lancé");
        let mut painted = 0;
        let mut most = 0;
        let mut last = None;
        let end = loop {
            let frame = capri_core::frame_at(started.elapsed());
            if let Some(end) = view.step(frame, &mut offscreen) {
                break end;
            }
            if let Some((_, shown, _)) = view.picture(Vec2::new(64.0, 64.0))
                && last != Some(shown)
            {
                last = Some(shown);
                painted += 1;
                let layers = view.layers(Vec2::new(64.0, 64.0));
                most = most.max(layers.len());
                let _ = offscreen.render(&layers, None).expect("rendue");
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        };
        let elapsed = started.elapsed();
        let first_after = view.close(&mut offscreen);
        // la toile lâchée : la carte graphique ne garde rien ; comme l'app à la fin du film
        // (`Running::film_over`), elle la rend tout de suite, avec les réglages de ses bandes, puis
        // un dessin de plus
        assert_eq!(offscreen.image_bytes(), 0);
        offscreen.settle();
        assert!(offscreen.slots() <= capri_render::SLOTS_KEPT, "{}", offscreen.slots());
        let _ = offscreen.render(&[], None).expect("rendue");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = stop.send(());
        let (highest, samples) = sampler.join().expect("les mesures");
        let last = report();
        let after = total(&last);
        println!("{} : {end:?}, la première image prête en {first_after:?}", film.name);
        println!(
            "joué en {:.2} s, {painted} dessins à l'écran, au plus {most} dessins par image",
            elapsed.as_secs_f64()
        );
        println!(
            "empreinte mémoire : {:.1} Mo avant, {:.1} Mo au plus haut (+{:.1}, {samples} mesures), {:.1} Mo après ({:+.1})",
            before as f64 / 1e6,
            highest as f64 / 1e6,
            highest.saturating_sub(before) as f64 / 1e6,
            after as f64 / 1e6,
            (after as f64 - before as f64) / 1e6
        );
        let (was, is) = (dirty(&first), dirty(&last));
        for (category, bytes) in &is {
            let change = bytes - was.get(category).copied().unwrap_or(0);
            if change.abs() >= 256 * 1024 {
                println!("  {category} : {:+.1} Mo", change as f64 / 1e6);
            }
        }
        assert!(matches!(end, End::Played { .. }), "{end:?}");
        assert!(elapsed.as_secs_f64() < 1.0 + film.frames() as f64 / 24.0);
        assert!(most <= 3 * capri_core::intro::glitch::MAX_BANDS, "{most}");
    }
}

/// Une image de `rgba` (`size`) réduite au quart de chaque côté, pour une planche.
fn quarter(rgba: &[u8], size: (u32, u32)) -> Vec<u8> {
    let image = image::RgbaImage::from_raw(size.0, size.1, rgba.to_vec()).expect("une image");
    image::imageops::thumbnail(&image, size.0 / 4, size.1 / 4).into_raw()
}

/// De même, au huitième.
fn eighth(rgba: &[u8], size: (u32, u32)) -> Vec<u8> {
    let image = image::RgbaImage::from_raw(size.0, size.1, rgba.to_vec()).expect("une image");
    image::imageops::thumbnail(&image, size.0 / 8, size.1 / 8).into_raw()
}

/// Ses entrées, qu'elle joue elle-même, et « se matérialise », sur un faux écran de MacBook Pro 14" (3024 ×
/// 1964), à sa place du lancement (en bas à droite), à sa taille d'origine, son corps avancé d'un
/// vingt-quatrième de seconde par image comme dans l'app : une planche de chacune, un dessin sur
/// deux, au quart (`intro-<nom>-planche.png`), et quelques images entières
/// (`intro-<nom>-<image>.png`). « Arrive en marchant » deux fois : de hors de l'écran (sa place
/// près du bord), et de plus près après s'être matérialisée (`intro-marche-loin-…`).
#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies planches et la vraie carte graphique"]
fn act_intro_portraits() {
    use capri_core::intro::{self, Play};

    let dir = directory();
    let size = (3024u32, 1964u32);
    let mut scene = Scene::new(size, HEIGHT);
    let look = &scene.figure.look;
    // là où l'app la met au lancement : en bas à droite, au-dessus du Dock
    let body = look.standing_size();
    let feet_uv = look.standing_feet();
    let start = Vec2::new(size.0 as f32 - body.x * 1.8, size.1 as f32 - body.y * 1.3);
    let home = start + Vec2::new(body.x * feet_uv.x, body.y * feet_uv.y);
    let at_home = capri(&scene.figure, size, home);
    assert!(
        intro::Act::ALL.iter().all(|&act| look.can_perform(act, &at_home)),
        "il faut ses planches de vie et de gestes"
    );
    let far = Vec2::new(700.0, home.y);
    let mut cases: Vec<(String, Option<intro::Act>, Vec2)> = Vec::new();
    for entry in intro::LIST {
        match entry.play {
            Play::Act(act) => cases.push((entry.key.to_owned(), Some(act), home)),
            Play::Plain => cases.push((entry.key.to_owned(), None, home)),
        }
    }
    cases.push(("marche-loin".to_owned(), Some(intro::Act::Walk), far));
    for (name, act, feet) in cases {
        let mut c = capri(&scene.figure, size, feet);
        c.step(0.0, 0);
        match act {
            Some(act) => assert!(scene.figure.look.perform(&mut c, act, 0)),
            None => scene.figure.look.enter(&mut c, 0),
        }
        let mut thumbs = Vec::new();
        let mut kept = 0;
        let mut frame = 0u64;
        let mut over = None;
        loop {
            let picture = scene.figure.look.picture(&c);
            if frame.is_multiple_of(2) {
                let image = scene.render(None, &[picture]);
                if frame.is_multiple_of(12) || over == Some(frame) {
                    save(&dir, &format!("intro-{name}-{frame:03}.png"), &image, size);
                    kept += 1;
                }
                thumbs.push(quarter(&image, size));
            }
            let done = !scene.figure.look.is_performing() && scene.figure.look.is_here();
            if done && over.is_none() {
                over = Some(frame);
            }
            // un peu de sa boucle (ou la fin de son geste) après
            if over.is_some_and(|at| frame >= at + 24) || frame > 400 {
                break;
            }
            frame += 1;
            c.step(1.0 / 24.0, frame);
            scene.figure.look.update(&mut c, frame, false, false);
            if let Some(end) = scene.figure.look.take_performed() {
                println!("{name} : {end:?}, ses pieds en ({:.0}, {:.0})", c.feet().x, c.feet().y);
            }
        }
        let (sheet, sheet_size) = contact(&thumbs, (size.0 / 4, size.1 / 4), 6);
        save(&dir, &format!("intro-{name}-planche.png"), &sheet, sheet_size);
        println!(
            "{name} : là à l'image {over:?}, {kept} images entières, sa place en ({:.0}, {:.0})",
            feet.x, feet.y
        );
        // une scène neuve pour la suivante : son dessin et son corps repartent de zéro
        scene = Scene::new(size, HEIGHT);
    }
}

/// Charge pour de vrai (le thread de `on_demand`, la carte graphique de `scene`) les planches de
/// repos que son dessin veut maintenant, au plus 60 s ; rend le temps mis.
fn load_rests(scene: &mut Scene) -> std::time::Duration {
    let started = Instant::now();
    loop {
        let wants = scene.figure.look.wants();
        let dressed = scene.figure.look.dressed();
        if (!wants.lying || dressed.lying) && (!wants.sleeping || dressed.sleeping) {
            return started.elapsed();
        }
        scene.figure.sync_rests(&mut scene.offscreen, Instant::now());
        assert!(started.elapsed().as_secs() < 60, "ses planches de repos n'arrivent pas");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Son repos : sur un bord, elle s'allonge (son éclat, image par image), allongée (sa boucle, un
/// clignement, sa parole), endormie, elle se rassoit, son départ allongée ; la superposition de
/// sa pose assise et de sa pose allongée sur le même bord (la ligne du bord en rouge, sa hanche
/// assise en bleu) ; debout, elle s'endort, puis se réveille (et la coupe vers sa boucle,
/// superposée).
#[test]
#[ignore = "écrit des images PNG dans CAPRI_PORTRAITS_DIR, avec les vraies planches et la vraie carte graphique"]
fn rest_portraits() {
    use capri_core::look::{Gesture, Rest};
    use capri_core::sheets::{lie, life};

    let dir = directory();
    let size = (900, 900);
    let mut scene = Scene::new(size, HEIGHT);
    scene.figure.offer_rests(crate::figure::Rests {
        lying: assets::rest::lying(),
        sleeping: assets::rest::sleeping(),
    });
    let offered = scene.figure.look.offered();
    assert!(offered.lying && offered.sleeping, "il faut ses planches de repos");
    scene.figure.look.set_room_to_lie(true);
    // une fenêtre dont le haut est à 480 pixels ; assise vers la droite (elle s'allonge vers la gauche)
    let window = Rect::new(Vec2::new(60.0, 480.0), Vec2::new(800.0, 420.0));
    let seat = Vec2::new(640.0, window.min.y);
    let mut c = perched(&mut scene, window, seat, 0);
    let since = sheets::arriving().len() + 30;
    let seated = step(&mut scene, &mut c, since);
    // elle s'allonge : d'abord l'envie, ses planches chargées, puis son éclat
    assert!(!scene.figure.look.lie_down(&mut c, since), "pas encore chargées");
    let took = load_rests(&mut scene);
    println!(
        "pose allongée chargée en {} ms ; mémoire graphique : {:.1} Mo",
        took.as_millis(),
        scene.offscreen.image_bytes() as f64 / 1e6
    );
    assert!(scene.figure.look.lie_down(&mut c, since));
    let mut frames = Vec::new();
    for frame in (since..since + Gesture::LieGlitch.frames() + 4).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        frames.push(scene.render(Some(window), &[picture]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 6);
    save(&dir, "repos-s-allonge.png", &sheet, sheet_size);
    let lain = since + Gesture::LieGlitch.frames();
    let lying = step(&mut scene, &mut c, lain + 1);
    save(&dir, "repos-allongee.png", &scene.render(Some(window), &[lying]), size);
    // allongée : sa boucle, un clignement (tour 0, dessins 28 à 35), sa parole
    let mut poses = Vec::new();
    for (frame, talking) in
        [(lain + 1, false), (lain + 30, false), (lain + 32, false), (lain + 64, true), (lain + 70, true)]
    {
        c.step(0.0, frame);
        scene.figure.look.update(&mut c, frame, talking, false);
        poses.push(scene.render(Some(window), &[scene.figure.look.picture(&c)]));
    }
    let (sheet, sheet_size) = contact(&poses, size, 5);
    save(&dir, "repos-allongee-planche.png", &sheet, sheet_size);
    // la superposition : assise et allongée, sur le même bord
    let cut = lying_over_seated(&mut scene, window, seated, lying);
    save(&dir, "repos-superposition-assise-allongee.png", &cut.0, cut.1);
    // endormie
    let asleep_at = lain + 120;
    step(&mut scene, &mut c, asleep_at);
    assert!(scene.figure.look.doze(&mut c, asleep_at));
    let mut sleeping = Vec::new();
    for frame in (asleep_at..asleep_at + 60).step_by(6) {
        let picture = step(&mut scene, &mut c, frame);
        sleeping.push(scene.render(Some(window), &[picture]));
    }
    let (sheet, sheet_size) = contact(&sleeping, size, 5);
    save(&dir, "repos-endormie.png", &sheet, sheet_size);
    // elle se rassoit : réveillée, l'éclat de sa pose assise, puis sa boucle
    let up = asleep_at + 100;
    step(&mut scene, &mut c, up);
    assert!(scene.figure.look.rouse(&mut c, up) && scene.figure.look.sit_up(&mut c, up));
    let mut frames = Vec::new();
    for frame in (up..up + Gesture::PerchGlitch.frames() + 4).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        frames.push(scene.render(Some(window), &[picture]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 6);
    save(&dir, "repos-se-rassoit.png", &sheet, sheet_size);
    assert_eq!(scene.figure.look.rest(), Rest::Awake);
    // allongée de nouveau, sa fenêtre se ferme : elle s'efface allongée
    let again = up + 200;
    step(&mut scene, &mut c, again);
    assert!(scene.figure.look.lie_down(&mut c, again));
    let gone = again + Gesture::LieGlitch.frames() + 10;
    step(&mut scene, &mut c, gone);
    c.take_off();
    let mut frames = Vec::new();
    for frame in (gone..gone + lie::leaving().len() + 2).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        frames.push(scene.render(None, &[picture]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 7);
    save(&dir, "repos-depart-allongee.png", &sheet, sheet_size);

    // debout : elle s'endort, puis se réveille
    let mut scene = Scene::new(size, HEIGHT);
    scene.figure.offer_rests(crate::figure::Rests { lying: None, sleeping: assets::rest::sleeping() });
    let feet = Vec2::new(450.0, 820.0);
    let mut c = capri(&scene.figure, size, feet);
    let awake = step(&mut scene, &mut c, 10);
    assert!(!scene.figure.look.doze(&mut c, 10), "pas encore chargées");
    let took = load_rests(&mut scene);
    println!("sommeil debout chargé en {} ms", took.as_millis());
    assert!(scene.figure.look.doze(&mut c, 10));
    let mut frames = Vec::new();
    for frame in (10..10 + life::sleeping().len()).step_by(4) {
        let picture = step(&mut scene, &mut c, frame);
        frames.push(scene.render(None, &[picture]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 8);
    save(&dir, "repos-endormie-debout.png", &sheet, sheet_size);
    let dozing = step(&mut scene, &mut c, 80);
    let cut = superposition(&mut scene, dozing, awake);
    save(&dir, "repos-endormie-debout-superposition.png", &cut.0, cut.1);
    assert!(scene.figure.look.rouse(&mut c, 100));
    let mut frames = Vec::new();
    let mut last = None;
    for frame in (100..100 + life::waking().len()).step_by(2) {
        let picture = step(&mut scene, &mut c, frame);
        last = Some(picture);
        frames.push(scene.render(None, &[picture]));
    }
    let (sheet, sheet_size) = contact(&frames, size, 7);
    save(&dir, "repos-reveil-debout.png", &sheet, sheet_size);
    let after = step(&mut scene, &mut c, 100 + life::waking().len());
    assert!(matches!(after.drawing, Drawing::Stand(_)), "{after:?}");
    let cut = superposition(&mut scene, last.expect("ses dessins"), after);
    save(&dir, "repos-reveil-debout-superposition.png", &cut.0, cut.1);
}

/// `lying` (allongée) et `seated` (assise) sur le même bord de `window` : trois cases côte à côte,
/// assise seule, allongée seule, puis assise à moitié transparente par-dessus allongée ; sur
/// chacune, la ligne du bord (en rouge) et sa hanche assise (en bleu, `lie::PERCH_HIP`). Recadrée
/// autour d'elles.
fn lying_over_seated(
    scene: &mut Scene,
    window: Rect,
    seated: Picture,
    lying: Picture,
) -> (Vec<u8>, (u32, u32)) {
    use capri_core::sheets::lie;

    let k = seated.rect.size.y / sheets::CELL_HEIGHT as f32;
    let hip_x = (seated.rect.min.x + lie::PERCH_HIP * k).round();
    let width = scene.size.0 as f32;
    let mut panels = Vec::new();
    for pictures in [vec![(seated, 1.0)], vec![(lying, 1.0)], vec![(lying, 1.0), (seated, 0.5)]] {
        let (w, h) = scene.size;
        let screen = Rect::new(Vec2::ZERO, Vec2::new(w as f32, h as f32));
        let mut layers = vec![
            Scene::rect(scene.desk, screen),
            Scene::rect(scene.window, window),
            Scene::rect(scene.title, Rect::new(window.min, Vec2::new(window.size.x, 56.0))),
        ];
        for (picture, opacity) in pictures {
            layers.extend(
                scene
                    .figure
                    .layers(&picture, screen)
                    .into_iter()
                    .map(|(id, draw)| (id, SpriteDraw { opacity, ..draw })),
            );
        }
        layers.push(Scene::rect(
            scene.feet_mark,
            Rect::new(Vec2::new(0.0, window.min.y - 1.0), Vec2::new(width, 2.0)),
        ));
        layers.push(Scene::rect(
            scene.top_mark,
            Rect::new(Vec2::new(hip_x - 1.0, window.min.y - 80.0), Vec2::new(2.0, 160.0)),
        ));
        panels.push(scene.offscreen.render(&layers, None).expect("rendue"));
    }
    // recadrée : ses deux cases entières, et un peu de marge
    let (a, b) = (lying.rect, seated.rect);
    let min = Vec2::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y));
    let max = Vec2::new(a.max().x.max(b.max().x), a.max().y.max(b.max().y));
    let at = ((min.x - 20.0).max(0.0) as u32, (min.y - 20.0).max(0.0) as u32);
    let crop_size = (
        ((max.x - min.x + 40.0) as u32).min(scene.size.0 - at.0),
        ((max.y - min.y + 40.0) as u32).min(scene.size.1 - at.1),
    );
    let cropped: Vec<Vec<u8>> = panels.iter().map(|panel| crop(panel, scene.size.0, at, crop_size)).collect();
    contact(&cropped, crop_size, 3)
}

/// Ses planches de repos chargées pour de vrai, comme dans l'app (le thread de `on_demand`, la vraie
/// carte graphique), puis lâchées une fois `on_demand::KEEP` passé : la mémoire graphique comptée
/// (`image_bytes`) et l'empreinte mémoire du processus (`phys_footprint`, lue par
/// `/usr/bin/footprint`, mémoire graphique comprise sur un Mac à puce Apple), au repos (avant), au
/// plus haut pendant le chargement, une fois chargées, et après les avoir lâchées.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "charge ses planches de repos sur la vraie carte graphique, et lit son empreinte mémoire"]
fn rest_weighed_for_real() {
    let total = || -> u64 {
        let output = std::process::Command::new("/usr/bin/footprint")
            .args(["-p", &std::process::id().to_string(), "-f", "bytes"])
            .output()
            .expect("footprint");
        String::from_utf8_lossy(&output.stdout)
            .split("Footprint: ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    };
    for (name, lying) in [("pose allongée", true), ("sommeil debout", false)] {
        // une scène neuve pour chacune : son dessin repart de zéro
        let mut scene = Scene::new((64, 64), HEIGHT);
        scene.figure.offer_rests(crate::figure::Rests {
            lying: assets::rest::lying(),
            sleeping: assets::rest::sleeping(),
        });
        let _ = scene.offscreen.render(&[], None).expect("rendue");
        // l'allocateur garde en réserve le dernier grand bloc lâché : un premier, avant de mesurer
        // (l'app en a lâché de bien plus grands au lancement, ses planches une fois confiées)
        drop(std::hint::black_box(vec![7u8; 22320 * 250 * 4]));
        scene.offscreen.settle();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let at_rest = scene.offscreen.image_bytes();
        println!("au repos : {:.1} Mo de mémoire graphique comptée", at_rest as f64 / 1e6);
        // elle en a envie
        let mut c = capri(&scene.figure, scene.size, Vec2::new(32.0, 60.0));
        if lying {
            let window = Rect::new(Vec2::new(0.0, 40.0), Vec2::new(64.0, 24.0));
            c = perched(&mut scene, window, Vec2::new(40.0, 40.0), 0);
            scene.figure.look.set_room_to_lie(true);
            step(&mut scene, &mut c, 100);
            scene.figure.look.lie_down(&mut c, 100);
        } else {
            step(&mut scene, &mut c, 10);
            scene.figure.look.doze(&mut c, 10);
        }
        let before = total();
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let sampler = std::thread::spawn(move || {
            let mut highest = 0;
            while stopped.try_recv().is_err() {
                highest = highest.max(total());
            }
            highest
        });
        let took = load_rests(&mut scene);
        // chargées : elle s'allonge, ou s'endort, pour de bon
        let rested =
            if lying { scene.figure.look.lie_down(&mut c, 100) } else { scene.figure.look.doze(&mut c, 10) };
        assert!(rested, "{name}");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let loaded = total();
        let bytes = scene.offscreen.image_bytes();
        // relevée (ou réveillée) : plus voulues, lâchées une fois KEEP passé, et rendues
        if lying {
            scene.figure.look.sit_up(&mut c, 101);
            step(&mut scene, &mut c, 101 + capri_core::look::LONGEST_GESTURE);
        } else {
            scene.figure.look.rouse(&mut c, 20);
            step(&mut scene, &mut c, 200);
        }
        assert_eq!(scene.figure.look.wants(), capri_core::look::Wants::default());
        let now = Instant::now();
        scene.figure.sync_rests(&mut scene.offscreen, now);
        scene.figure.sync_rests(
            &mut scene.offscreen,
            now + crate::on_demand::KEEP + std::time::Duration::from_secs(1),
        );
        assert_eq!(scene.offscreen.image_bytes(), at_rest, "{name} : tout est lâché");
        let _ = scene.offscreen.render(&[], None).expect("rendue");
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = stop.send(());
        let highest = sampler.join().expect("les mesures");
        let after = total();
        println!(
            "{name} : chargée en {} ms ; mémoire graphique comptée {:.1} Mo (+{:.1}) ; empreinte {:.1} Mo avant, \
             {:.1} au plus haut (+{:.1}), {:.1} chargée (+{:.1}), {:.1} après l'avoir lâchée ({:+.1})",
            took.as_millis(),
            bytes as f64 / 1e6,
            (bytes - at_rest) as f64 / 1e6,
            before as f64 / 1e6,
            highest as f64 / 1e6,
            highest.saturating_sub(before) as f64 / 1e6,
            loaded as f64 / 1e6,
            loaded as f64 / 1e6 - before as f64 / 1e6,
            after as f64 / 1e6,
            (after as f64 - before as f64) / 1e6
        );
    }
}
