//! Les intros qu'elle joue elle-même, avec ses propres dessins (ses planches de vie et de gestes,
//! voir [`crate::sheets::life`]) : ce que font son corps et son dessin à chaque image, et quand
//! c'est fini. `look` les joue ([`crate::look::Look::perform`]), `body` la déplace.
//!
//! Comme tous ses dessins, ils sont tenus un nombre entier d'images à 24 i/s ; sa position, elle,
//! est fluide : elle suit l'écran, comme quand on la tient ou qu'elle marche.
//!
//! - **Elle tombe du ciel** ([`Act::Fall`]) : elle entre par le haut de l'écran en pendant (sa
//!   planche en l'air, `dangle`), tombe jusqu'à sa place de plus en plus vite ([`fall_at`], en
//!   [`FALL_SECONDS`]), puis se relève (`glitch`, le geste « se relever ») et reprend sa boucle.
//!   Il lui faut ses planches `dangle` et `glitch`.
//! - **Elle arrive en marchant** ([`Act::Walk`]) : elle entre par le bord droit de l'écran en
//!   marchant vers la gauche (`walk`, qui regarde vers la gauche) jusqu'à sa place, à sa vitesse de
//!   marche, puis sa boucle. Si sa place est trop loin du bord droit pour un trajet de
//!   [`WALK_LONGEST`] secondes, elle part de moins loin, en se matérialisant d'abord là, toujours
//!   entière dans l'écran ([`walk_from`]) ; s'il ne lui restait pas [`WALK_SHORTEST`] de marche,
//!   cette intro n'est pas possible depuis sa place. Il lui faut sa planche `walk`.
//! - **Elle se rassemble** ([`Act::Gather`]) : les premières images de son arrivée (`arrive`),
//!   découpées en [`BANDS`] bandes horizontales étirées sur toute la largeur de l'écran, qui se
//!   resserrent en elle en [`GATHER`] poses clés tenues deux images ([`bands`]) ; puis son arrivée
//!   reprend là où elle en est ([`ARRIVE_FROM`]), jusqu'à sa boucle. Rien de plus que ce que le
//!   rendu sait déjà faire : des parts d'une image, étirées. Il lui faut ses planches de vie.

use crate::geom::{Rect, Vec2};
use crate::sheets::{CELL_HEIGHT, life};

/// Une intro jouée par elle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    /// Elle tombe du ciel.
    Fall,
    /// Elle arrive en marchant.
    Walk,
    /// Elle se rassemble.
    Gather,
}

impl Act {
    pub const ALL: [Act; 3] = [Act::Fall, Act::Walk, Act::Gather];
}

/// Sa chute dure ça, en secondes (30 images).
pub const FALL_SECONDS: f32 = 1.25;

/// D'où tombent ses pieds, pour une place `place` (ses pieds une fois arrivée), une image debout
/// de `size` pixels aux pieds en `feet` (coordonnées normalisées), sur l'écran `screen` : à leur
/// place en x, et toute son image (et sa toile de vie, de la même hauteur) au-dessus du haut de
/// l'écran, d'un pixel.
pub fn fall_from(place: Vec2, size: Vec2, feet: Vec2, screen: Rect) -> Vec2 {
    Vec2::new(place.x, screen.min.y - size.y * (1.0 - feet.y) - 1.0)
}

/// Où sont ses pieds (en y) `t` secondes après le début de sa chute de `from` à `to` : de plus en
/// plus vite, comme une chute (le carré du temps écoulé), arrivés au bout de [`FALL_SECONDS`].
pub fn fall_at(from: f32, to: f32, t: f32) -> f32 {
    let u = (t / FALL_SECONDS).clamp(0.0, 1.0);
    if u >= 1.0 { to } else { from + (to - from) * u * u }
}

/// Elle ne marche pas plus longtemps que ça (en secondes) pour arriver.
pub const WALK_LONGEST: f32 = 4.0;
/// Ni moins longtemps que ça (en secondes) : si sa place est si près du bord droit qu'il ne lui
/// resterait presque rien à marcher, cette intro n'est pas possible depuis là.
pub const WALK_SHORTEST: f32 = 1.0;

/// Où ses pieds sont hors de l'écran `screen`, à droite : sa toile de vie entière au-delà du bord
/// (elle commence `left` pixels à gauche de ses pieds), d'un pixel.
pub fn offscreen_right(screen: Rect, left: f32) -> f32 {
    screen.max().x + left + 1.0
}

/// Où ses pieds sont le plus à droite avec sa toile de vie entière dans l'écran `screen` (elle finit
/// `right` pixels à droite de ses pieds).
pub fn inside_right(screen: Rect, right: f32) -> f32 {
    screen.max().x - right
}

/// D'où elle part (le x de ses pieds) pour arriver en `place` (x) en marchant à `speed` pixels par
/// seconde : de `offscreen` (hors de l'écran, à droite) si le trajet dure au plus
/// [`WALK_LONGEST`] ; sinon de plus près, et elle s'y matérialise d'abord (`true`), toujours
/// entièrement dans l'écran (ses pieds en `inside` au plus).
/// Moins de [`WALK_SHORTEST`] de marche : `None`, cette intro n'est pas possible depuis là.
pub fn walk_from(place: f32, offscreen: f32, inside: f32, speed: f32) -> Option<(f32, bool)> {
    if speed.is_nan() || speed <= 0.0 {
        return None;
    }
    let farthest = place + WALK_LONGEST * speed;
    let (x, appears) = if offscreen <= farthest { (offscreen, false) } else { (farthest.min(inside), true) };
    (x - place >= WALK_SHORTEST * speed).then_some((x, appears))
}

/// Ses poses clés quand elle se rassemble : (la case de son arrivée, la part de l'écran où ses
/// bandes s'étirent : 1, toute sa largeur ; 0, elle seule). Les trois premières sont le trait de
/// son arrivée, étiré d'un bord à l'autre ; puis ses éclats se resserrent en elle.
pub const GATHER: [(u32, f32); 9] =
    [(0, 1.0), (1, 1.0), (2, 1.0), (3, 0.85), (4, 0.7), (5, 0.5), (6, 0.32), (7, 0.16), (8, 0.05)];
/// Chaque pose clé est tenue deux images.
pub const GATHER_HOLD: u64 = 2;
/// Combien d'images elle met à se rassembler (0,75 s).
pub const GATHER_FRAMES: u64 = GATHER.len() as u64 * GATHER_HOLD;
/// Puis son arrivée reprend à cette image de sa feuille (`life::arriving`) : sa case 9, celle qui
/// suit la dernière pose clé ; il lui reste 10 images (0,42 s) jusqu'à sa boucle.
pub const ARRIVE_FROM: u64 = GATHER.len() as u64 * 2;
/// En combien de bandes elle est découpée : 8 bandes de 48 px de sa toile.
pub const BANDS: u32 = 8;
/// Le décalage des bandes de côté, au plus, en part de la largeur de l'écran, quand elles sont
/// étirées de tout leur long (il diminue avec elles) : de quoi faire un éclat, pas un mur.
pub const JITTER: f32 = 0.06;

/// La pose clé montrée `elapsed` images après qu'elle a commencé à se rassembler (la dernière
/// au-delà).
pub fn gather_key(elapsed: u64) -> u8 {
    (elapsed / GATHER_HOLD).min(GATHER.len() as u64 - 1) as u8
}

/// Une bande : ses lignes dans une case de 240 × 384 px (de la première, combien), et où la
/// dessiner (pixels entiers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub rows: (u32, u32),
    pub rect: Rect,
}

/// Les bandes de la pose clé `key`, pour une case dessinée en `cell` (sa toile de vie, là où son
/// arrivée la dessinerait) sur l'écran `screen` : chacune à sa hauteur, sans trou entre elles,
/// étirée vers les bords de l'écran selon la part de sa pose clé, et décalée de côté d'autant
/// (d'une bande à l'autre et d'une pose à l'autre, toujours de même : rien n'est tiré au hasard).
/// À la part 0, elles refont la case telle quelle.
pub fn bands(cell: Rect, key: u8, screen: Rect) -> [Band; BANDS as usize] {
    stretched(cell, GATHER[usize::from(key).min(GATHER.len() - 1)].1, key, screen)
}

/// Les bandes étirées de la part `spread` (de 0 à 1), décalées comme à la pose clé `key`.
fn stretched(cell: Rect, spread: f32, key: u8, screen: Rect) -> [Band; BANDS as usize] {
    let spread = spread.clamp(0.0, 1.0);
    let rows = CELL_HEIGHT / BANDS;
    let k = cell.size.y / CELL_HEIGHT as f32;
    let lerp = |from: f32, to: f32| from + (to - from) * spread;
    std::array::from_fn(|index| {
        let index = index as u32;
        // une valeur de -1 à 1, fixe pour chaque bande et chaque pose clé
        let wobble = ((index * 7 + u32::from(key) * 5) % 11) as f32 / 5.0 - 1.0;
        let shift = (wobble * JITTER * screen.size.x * spread).round();
        let left = (lerp(cell.min.x, screen.min.x) + shift).round();
        let right = (lerp(cell.max().x, screen.max().x) + shift).round();
        let top = (cell.min.y + (index * rows) as f32 * k).round();
        let bottom = (cell.min.y + ((index + 1) * rows) as f32 * k).round();
        Band {
            rows: (index * rows, rows),
            rect: Rect::new(
                Vec2::new(left, top),
                Vec2::new((right - left).max(1.0), (bottom - top).max(0.0)),
            ),
        }
    })
}

// les bandes se partagent ses lignes sans reste, dans des cases de sa toile de vie
const _: () = assert!(life::CELL_WIDTH == 240 && CELL_HEIGHT.is_multiple_of(BANDS));

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));

    #[test]
    fn she_falls_faster_and_faster_from_above_the_screen_to_her_place() {
        let (size, feet) = (Vec2::new(181.0, 600.0), Vec2::new(0.457, 0.947));
        let place = Vec2::new(2700.0, 1600.0);
        let from = fall_from(place, size, feet, SCREEN);
        // toute son image au-dessus du haut de l'écran
        assert_eq!(from.x, place.x);
        assert!(from.y + size.y * (1.0 - feet.y) < 0.0, "{from:?}");
        let mut last = from.y;
        let mut step = 0.0;
        for frame in 1..=30 {
            let y = fall_at(from.y, place.y, frame as f32 / 24.0);
            assert!(y > last && y - last > step, "de plus en plus vite : image {frame}");
            step = y - last;
            last = y;
        }
        assert_eq!(fall_at(from.y, place.y, FALL_SECONDS), place.y);
        assert_eq!(fall_at(from.y, place.y, 99.0), place.y);
        assert_eq!(fall_at(from.y, place.y, -1.0), from.y);
        assert!((FALL_SECONDS * 24.0 - 30.0).abs() < 1e-3, "30 images");
    }

    #[test]
    fn she_walks_in_from_beyond_the_right_edge_or_from_closer_after_appearing() {
        // sa toile commence 197 px à gauche de ses pieds
        let offscreen = offscreen_right(SCREEN, 197.0);
        assert_eq!(offscreen, 3024.0 + 198.0);
        // à 144 px par seconde (600 px debout), sa place à 300 px du bord : elle entre
        // sa toile finit 178 px à droite de ses pieds : entière dans l'écran, ses pieds en 2846 au plus
        let inside = inside_right(SCREEN, 178.0);
        assert_eq!(inside, 2846.0);
        let (x, appears) = walk_from(2724.0, offscreen, inside, 144.0).expect("possible");
        assert_eq!((x, appears), (offscreen, false));
        assert!((x - 2724.0) / 144.0 <= WALK_LONGEST);
        // sa place loin du bord : 4 s de marche au plus, et elle se matérialise là
        let (x, appears) = walk_from(500.0, offscreen, inside, 144.0).expect("possible");
        assert!(appears && (x - 500.0 - 4.0 * 144.0).abs() < 1e-3, "{x}");
        assert!(x < offscreen);
        // sa place à 400 px du bord : 4 s de marche la mettraient en partie dehors ; elle se
        // matérialise entière dans l'écran, et marche moins longtemps
        let (x, appears) = walk_from(2624.0, offscreen, inside, 144.0).expect("possible");
        assert!(appears && x == inside, "{x}");
        assert!((x - 2624.0) / 144.0 >= WALK_SHORTEST);
        // un écran où il ne resterait pas une seconde de marche : pas possible depuis là
        assert_eq!(walk_from(2800.0, 3500.0, 2846.0, 144.0), None);
        assert_eq!(walk_from(500.0, offscreen, inside, 0.0), None, "sans vitesse");
    }

    #[test]
    fn her_bands_stretch_across_the_screen_then_close_in_on_her() {
        let cell = Rect::new(Vec2::new(2500.0, 1000.0), Vec2::new(375.0, 600.0));
        for key in 0..GATHER.len() as u8 {
            let bands = bands(cell, key, SCREEN);
            // toutes ses lignes, dans l'ordre, et à sa hauteur sans trou
            assert_eq!(bands[0].rows.0, 0);
            assert_eq!(bands[BANDS as usize - 1].rows.0 + bands[BANDS as usize - 1].rows.1, CELL_HEIGHT);
            assert_eq!(bands[0].rect.min.y, cell.min.y);
            assert_eq!(bands[BANDS as usize - 1].rect.max().y, cell.max().y);
            for pair in bands.windows(2) {
                assert_eq!(pair[0].rows.0 + pair[0].rows.1, pair[1].rows.0);
                assert_eq!(pair[0].rect.max().y, pair[1].rect.min.y);
            }
            for band in bands {
                assert_eq!(band.rect.min, Vec2::new(band.rect.min.x.round(), band.rect.min.y.round()));
                assert!(band.rect.size.x >= cell.size.x - 1.0, "jamais plus étroite qu'elle");
            }
        }
        // d'abord d'un bord à l'autre (au décalage près), puis resserrées sur elle
        for band in bands(cell, 0, SCREEN) {
            assert_eq!(band.rect.size.x, SCREEN.size.x);
        }
        let widths: Vec<f32> =
            (0..GATHER.len() as u8).map(|key| bands(cell, key, SCREEN)[3].rect.size.x).collect();
        assert!(widths.windows(2).all(|pair| pair[1] <= pair[0]), "{widths:?}");
        // à la dernière pose clé, presque elle ; à la part 0, la case telle quelle
        let last = bands(cell, GATHER.len() as u8 - 1, SCREEN);
        assert!(last.iter().all(|band| (band.rect.min.x - cell.min.x).abs() < 0.07 * SCREEN.size.x));
        for band in stretched(cell, 0.0, 4, SCREEN) {
            assert_eq!((band.rect.min.x, band.rect.size.x), (cell.min.x, cell.size.x));
        }
        assert_eq!(gather_key(0), 0);
        assert_eq!(gather_key(GATHER_FRAMES - 1), GATHER.len() as u8 - 1);
        assert_eq!(gather_key(1000), GATHER.len() as u8 - 1);
        // puis son arrivée reprend après la dernière case montrée
        assert_eq!(ARRIVE_FROM / 2, u64::from(GATHER[GATHER.len() - 1].0) + 1);
        assert_eq!(GATHER_FRAMES, 18);
    }
}
