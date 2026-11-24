//! L'effet de glitch de « de tout près » : calculé à l'affichage, sur les vrais dessins de son
//! artiste, sans aucune image de plus. Ce module le **décrit** pour chaque dessin du film ;
//! `capri-app` le dessine (la toile du film en bandes, chacune décalée, avec ses doubles teintés).
//!
//! **Ce qu'il fait** ([`Glitch`]), à chaque dessin :
//! - des **déchirures** : des bandes horizontales de l'image ([`Band`], leurs lignes dans
//!   l'image), décalées de côté, au plus de [`SHIFT`] de sa largeur ; au plus [`MAX_TORN`] par
//!   dessin ;
//! - des **bandes manquantes**, par instants : une déchirure sur trois environ à pleine force
//!   ([`MISSING`]), où rien n'est dessiné ; jamais pendant que son visage est collé à la vitre
//!   ([`Envelope::face`]) : là, seulement de rares déchirures fines ([`FACE_ROWS`]) ;
//! - deux **doubles colorés** ([`Double`]), décalés à gauche et à droite, derrière elle, dans le
//!   violet ([`VIOLET`]) et le vert ([`GREEN`]) des doubles de ses propres dessins (mesurés sur sa
//!   planche `arrive` : le violet au bord gauche de sa silhouette, le vert au bord droit) ;
//! - un **tremblement** bref de toute l'image ([`SHAKE`]), sur certains dessins seulement.
//!
//! **Sa force** suit une enveloppe nommée ([`Envelope`], [`UP_CLOSE`]) : forte sur les premiers
//! dessins (elle surgit) et les derniers (elle repart), moins au milieu, légère mais visible quand
//! elle colle son visage à la vitre (surtout ses doubles colorés, qu'il reste lisible), avec **une
//! ou deux rafales** au milieu, tirées au sort ([`Envelope::bursts`]), jamais sur son visage.
//! **Une seule constante la règle** : [`STRENGTH`] (0 : aucun glitch, l'image exactement telle
//! qu'elle est dessinée).
//!
//! **Déterministe** : la même graine (celle de sa vie, donc une autre à chaque lancement) et le même
//! dessin donnent le même glitch ([`Glitch::of`]) ; il ne change donc que quand le dessin change
//! (tenu deux images, comme tous ses dessins). Rien n'est tiré à l'affichage.
//!
//! **Borné** : tout est en pixels entiers de l'image ; rien n'est décalé de plus que [`reach`]
//! (une déchirure, un tremblement et un double ensemble), et ses bandes couvrent toujours toute la
//! hauteur de l'image, sans trou ni chevauchement ; deux bandes voisines qui ne bougent ni ne
//! manquent n'en font qu'une (pas de bord droit dessiné pour rien).

use super::Film;
use crate::roam::Rng;

/// **La force de l'effet** : la seule constante qui la règle. 1 : l'effet tel qu'écrit ici ; 0 :
/// aucun, et chaque dessin est exactement l'image ; plus de 1 : plus fort, chaque dessin plafonnant
/// à la force de ses premiers et derniers dessins (les bornes ne changent pas).
pub const STRENGTH: f32 = 1.0;

/// Le violet de ses doubles, mesuré sur sa planche `arrive` (le bord gauche de sa silhouette, de la
/// case 4 à la case 11 : de 341 à 485 pixels par case, la couleur la plus fréquente).
pub const VIOLET: [u8; 3] = [122, 57, 252];
/// Le vert de ses doubles, mesuré de même au bord droit (cases 4 et 5 : 263 et 143 pixels). Un peu
/// plus sombre que le vert de son masque (1, 249, 110).
pub const GREEN: [u8; 3] = [1, 236, 109];

/// Le plus grand décalage d'une déchirure, en part de la largeur de l'image (à pleine force).
pub const SHIFT: f32 = 0.05;
/// Au plus tant de déchirures par dessin (à pleine force, de la moitié à toutes).
pub const MAX_TORN: usize = 6;
/// Au plus tant de bandes par dessin : ses déchirures, et ce qui reste entre elles.
pub const MAX_BANDS: usize = 2 * MAX_TORN + 1;
/// La hauteur d'une déchirure, en part de la hauteur de l'image : de la plus fine à la plus haute.
pub const ROWS: (f32, f32) = (0.01, 0.14);
/// La chance qu'une déchirure manque, à pleine force.
pub const MISSING: f32 = 0.35;
/// Son visage à la vitre : au plus une déchirure, d'au plus tant de la hauteur de l'image, et
/// jamais manquante.
pub const FACE_ROWS: f32 = 0.02;
/// Son visage à la vitre : la chance de cette déchirure, fois la force.
pub const FACE_TEAR: f32 = 2.0;
/// Le décalage de côté d'un double, en part de la largeur de l'image (à pleine force ; il suit la
/// racine de la force, pour que ses doubles se voient encore quand le reste se tait).
pub const DOUBLE: (f32, f32) = (0.004, 0.016);
/// Son décalage vers le haut ou le bas, au plus, en part de la hauteur de l'image.
pub const DOUBLE_RISE: f32 = 0.004;
/// L'opacité d'un double, à force 1/3 et au-delà (moins en dessous : la racine de trois fois la
/// force).
pub const DOUBLE_OPACITY: f32 = 0.85;
/// Le tremblement de toute l'image, au plus : en part de sa largeur, de sa hauteur.
pub const SHAKE: (f32, f32) = (0.012, 0.008);
/// La chance d'un tremblement sur un dessin, à pleine force.
pub const SHAKE_CHANCE: f32 = 0.6;

/// Une rafale dure tant de dessins.
pub const BURST_LENGTH: u32 = 2;
/// Sa force.
pub const BURST: f32 = 0.9;

/// La graine de ses rafales, et celle de chaque dessin : celle de sa vie, mêlée à ces nombres.
const BURST_SEED: u64 = 0x7261_6661_6c65;
const DRAWING_SEED: u64 = 0x676c_6974_6368;

/// L'enveloppe du glitch d'un film : sa force à chaque dessin de sa feuille, où ses rafales peuvent
/// tomber, et où son visage doit rester lisible.
#[derive(Debug, PartialEq)]
pub struct Envelope {
    /// La force à chaque dessin de sa feuille, de 0 à 1 (au-delà de la liste : 0).
    pub force: &'static [f32],
    /// Les dessins où une rafale peut tomber : le premier et le dernier, compris.
    pub bursts: (u32, u32),
    /// Les dessins où son visage est collé à la vitre (le premier et le dernier, compris) : ni
    /// rafale, ni bande manquante, au plus une déchirure fine.
    pub face: (u32, u32),
}

/// L'enveloppe de « de tout près », dessin par dessin (regardés un à un) : elle surgit (l'écran
/// rouge de ses cheveux, puis elle regarde autour d'elle), elle colle son visage à la vitre avec un
/// grand sourire (dessins 13 à 19 : léger, surtout ses doubles), puis elle repart (ses cheveux).
/// Ses rafales tombent entre les dessins 6 et 11, avant son visage.
pub const UP_CLOSE: Envelope = Envelope { force: &UP_CLOSE_FORCE, bursts: (5, 10), face: (12, 18) };

#[rustfmt::skip]
const UP_CLOSE_FORCE: [f32; 22] = [
    // elle surgit (dessins 1 à 6)
    1.0, 0.95, 0.85, 0.75, 0.65, 0.55,
    // elle regarde autour d'elle (7 à 12)
    0.45, 0.42, 0.4, 0.36, 0.33, 0.3,
    // son visage contre la vitre (13 à 19)
    0.15, 0.13, 0.12, 0.12, 0.12, 0.14, 0.18,
    // elle repart (20 à 22)
    0.7, 0.92, 1.0,
];

impl Envelope {
    /// Où commencent ses rafales pour la graine `seed` : une, ou deux (une fois sur deux, quand la
    /// place le permet), de [`BURST_LENGTH`] dessins chacune, dans sa plage, sans se toucher.
    pub fn bursts(&self, seed: u64) -> [Option<u32>; 2] {
        let (first, last) = self.bursts;
        if last + 1 < first + BURST_LENGTH {
            return [None, None];
        }
        // les débuts possibles : de `first` à `last + 1 - BURST_LENGTH`
        let starts = last + 2 - first - BURST_LENGTH;
        let mut rng = Rng::new(seed ^ BURST_SEED);
        let pick = |rng: &mut Rng, count: u32| ((rng.unit() * f64::from(count)) as u32).min(count - 1);
        let one = first + pick(&mut rng, starts);
        // la seconde, une fois sur deux : séparée de la première d'au moins un dessin, s'il y a la place
        let apart: Vec<u32> =
            (first..first + starts).filter(|&start| start.abs_diff(one) > BURST_LENGTH).collect();
        let two = (rng.unit() < 0.5 && !apart.is_empty())
            .then(|| apart[pick(&mut rng, apart.len() as u32) as usize]);
        [Some(one), two]
    }

    /// Sa force au dessin `position` pour la graine `seed`, ses rafales comprises (jamais sur son
    /// visage).
    pub fn at(&self, position: u32, seed: u64) -> f32 {
        let base = self.force.get(position as usize).copied().unwrap_or(0.0);
        let bursting = !self.is_face(position)
            && self
                .bursts(seed)
                .into_iter()
                .flatten()
                .any(|start| (start..start + BURST_LENGTH).contains(&position));
        if bursting { base.max(BURST) } else { base }
    }

    /// Son visage est-il collé à la vitre au dessin `position` ?
    pub fn is_face(&self, position: u32) -> bool {
        (self.face.0..=self.face.1).contains(&position)
    }
}

/// Une bande de l'image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    /// Ses lignes dans l'image : la première, et combien.
    pub rows: (u32, u32),
    /// Son décalage de côté, en pixels de l'image (vers la droite s'il est positif).
    pub shift: i32,
    /// Manquante : rien n'en est dessiné, ni elle ni ses doubles.
    pub missing: bool,
}

impl Band {
    /// Ni décalée ni manquante.
    pub fn is_plain(&self) -> bool {
        self.shift == 0 && !self.missing
    }
}

/// Un double coloré : sa silhouette, d'un aplat de sa couleur, dessinée derrière elle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Double {
    pub color: [u8; 3],
    /// Son décalage, en pixels de l'image.
    pub offset: (i32, i32),
    /// Son opacité : 0, pas de double.
    pub opacity: f32,
}

impl Double {
    const NONE: [Double; 2] = [
        Double { color: VIOLET, offset: (0, 0), opacity: 0.0 },
        Double { color: GREEN, offset: (0, 0), opacity: 0.0 },
    ];
}

/// Le glitch d'un dessin du film.
#[derive(Clone, Debug, PartialEq)]
pub struct Glitch {
    /// Sa force, de 0 à 1 : 0, aucun glitch.
    pub force: f32,
    /// Le tremblement de toute l'image, en pixels de l'image.
    pub shake: (i32, i32),
    /// Ses bandes, de haut en bas : toute la hauteur de l'image, sans trou ni chevauchement ; au
    /// plus [`MAX_BANDS`].
    pub bands: Vec<Band>,
    /// Ses doubles : le violet (à gauche), puis le vert (à droite).
    pub doubles: [Double; 2],
}

/// Tant de la dimension `size`, en pixels entiers.
fn limit(fraction: f32, size: u32) -> i32 {
    (fraction * size as f32).round() as i32
}

/// Le plus loin que va quoi que ce soit du glitch, sur une image de `size` pixels : de côté (une
/// déchirure, un tremblement et un double ensemble), et en hauteur (un tremblement et un double).
pub fn reach(size: (u32, u32)) -> (i32, i32) {
    let (w, h) = size;
    (limit(SHIFT, w) + limit(SHAKE.0, w) + limit(DOUBLE.1, w), limit(SHAKE.1, h) + limit(DOUBLE_RISE, h))
}

impl Glitch {
    /// Aucun glitch, sur une image de `size` pixels : une seule bande, toute l'image.
    pub fn none(size: (u32, u32)) -> Self {
        Self {
            force: 0.0,
            shake: (0, 0),
            bands: vec![Band { rows: (0, size.1.max(1)), shift: 0, missing: false }],
            doubles: Double::NONE,
        }
    }

    /// Rien ne bouge, rien ne manque, aucun double : l'image telle quelle.
    pub fn is_still(&self) -> bool {
        self.shake == (0, 0)
            && self.bands.iter().all(Band::is_plain)
            && self.doubles.iter().all(|double| double.opacity <= 0.0)
    }

    /// Le glitch du dessin n° `position` de `film`, pour la graine `seed` (celle de sa vie), à la
    /// force `strength` ([`STRENGTH`] dans l'app) : sa force est celle de l'enveloppe du film à ce
    /// dessin, fois `strength`, plafonnée à 1. Sans enveloppe, ou à force nulle : aucun.
    pub fn of(film: &Film, position: u32, seed: u64, strength: f32) -> Self {
        let force = film.glitch.map_or(0.0, |envelope| envelope.at(position, seed)) * strength;
        let face = film.glitch.is_some_and(|envelope| envelope.is_face(position));
        let drawing = seed ^ DRAWING_SEED ^ u64::from(position).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        Self::with_force(force, film.size, drawing, face)
    }

    /// Un glitch de force `force` (ramenée de 0 à 1) sur une image de `size` pixels, tiré avec la
    /// graine `seed` ; `face` : son visage est collé à la vitre (aucune bande manquante, au plus une
    /// déchirure fine).
    pub fn with_force(force: f32, size: (u32, u32), seed: u64, face: bool) -> Self {
        let force = if force.is_nan() { 0.0 } else { force.clamp(0.0, 1.0) };
        if force <= 0.0 {
            return Self::none(size);
        }
        let (w, h) = (size.0.max(1), size.1.max(1));
        let mut rng = Rng::new(seed);
        let mut unit = move || rng.unit() as f32;
        let sign = |u: f32| if u < 0.5 { -1 } else { 1 };

        // ses déchirures : où, combien de lignes, de combien, et si elles manquent ; sur son
        // visage, au plus une, fine, jamais manquante
        let torn = if face {
            usize::from(unit() < FACE_TEAR * force)
        } else {
            ((MAX_TORN as f32 * force * (0.5 + 0.5 * unit())).round() as usize).min(MAX_TORN)
        };
        let tallest = if face { FACE_ROWS } else { ROWS.1 };
        let mut cuts: Vec<Band> = (0..torn)
            .map(|_| {
                let start = ((unit() * h as f32) as u32).min(h - 1);
                let rows = ((ROWS.0 + (tallest - ROWS.0) * unit()) * h as f32).round().max(1.0) as u32;
                let side = sign(unit());
                let shift = side * (limit(SHIFT, w) as f32 * force * (0.35 + 0.65 * unit())).round() as i32;
                let missing = unit() < MISSING * force && !face;
                Band { rows: (start, rows), shift, missing }
            })
            .collect();
        cuts.sort_by_key(|band| band.rows.0);
        // toute la hauteur, de haut en bas : ce qui reste entre elles ne bouge pas
        let mut bands: Vec<Band> = Vec::with_capacity(MAX_BANDS);
        let mut push = |band: Band| match bands.last_mut() {
            // deux bandes qui ne bougent pas n'en font qu'une
            Some(last) if last.is_plain() && band.is_plain() => last.rows.1 += band.rows.1,
            _ => bands.push(band),
        };
        let mut y = 0;
        for cut in cuts {
            let start = cut.rows.0.max(y);
            let end = (cut.rows.0 + cut.rows.1).min(h);
            if end <= start {
                continue;
            }
            if start > y {
                push(Band { rows: (y, start - y), shift: 0, missing: false });
            }
            push(Band { rows: (start, end - start), ..cut });
            y = end;
        }
        if y < h {
            push(Band { rows: (y, h - y), shift: 0, missing: false });
        }

        // ses doubles : le violet à gauche, le vert à droite ; ils suivent la racine de la force,
        // pour se voir encore quand le reste se tait
        let presence = force.sqrt();
        let opacity = DOUBLE_OPACITY * (force * 3.0).min(1.0).sqrt();
        let near = DOUBLE.0 / DOUBLE.1;
        let mut double = |color: [u8; 3], side: i32| {
            let x =
                side * (limit(DOUBLE.1, w) as f32 * presence * (near + (1.0 - near) * unit())).round() as i32;
            let y = (limit(DOUBLE_RISE, h) as f32 * presence * (2.0 * unit() - 1.0)).round() as i32;
            let opacity = if (x, y) == (0, 0) { 0.0 } else { opacity };
            Double { color, offset: (x, y), opacity }
        };
        let doubles = [double(VIOLET, -1), double(GREEN, 1)];

        // un tremblement de toute l'image, sur certains dessins
        let shake = if unit() < SHAKE_CHANCE * force {
            let x = sign(unit()) * (limit(SHAKE.0, w) as f32 * force * (0.5 + 0.5 * unit())).round() as i32;
            let y = (limit(SHAKE.1, h) as f32 * force * (2.0 * unit() - 1.0)).round() as i32;
            (x, y)
        } else {
            (0, 0)
        };
        Self { force, shake, bands, doubles }
    }

    /// Ce que ses déchirures et ses bandes manquantes dérangent de l'image, de 0 à 1 environ : la
    /// part de ses lignes touchées, pondérée par leur décalage ; pour comparer deux dessins.
    pub fn disorder(&self, size: (u32, u32)) -> f32 {
        let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let bands: f32 = self
            .bands
            .iter()
            .map(|band| {
                let weight = if band.missing { 1.0 } else { band.shift.unsigned_abs() as f32 / (SHIFT * w) };
                weight * band.rows.1 as f32 / h
            })
            .sum();
        let doubles: f32 = self
            .doubles
            .iter()
            .map(|double| double.opacity * double.offset.0.unsigned_abs() as f32 / (DOUBLE.1 * w))
            .sum();
        bands + 0.1 * doubles + 0.1 * self.shake.0.unsigned_abs() as f32 / (SHAKE.0 * w)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{FILM, UP_CLOSE as FILM_UP_CLOSE};
    use super::*;

    const SIZE: (u32, u32) = (1920, 1080);

    fn glitches(seed: u64, strength: f32) -> Vec<Glitch> {
        (0..FILM.len()).map(|position| Glitch::of(FILM, position, seed, strength)).collect()
    }

    #[test]
    fn the_same_seed_and_drawing_give_the_same_glitch_and_another_launch_another() {
        for seed in [0u64, 7, 1_763_900_000_000_000_123] {
            assert_eq!(glitches(seed, STRENGTH), glitches(seed, STRENGTH), "graine {seed}");
        }
        // deux lancements : pas le même glitch sur ses dessins les plus forts
        let (one, two) = (glitches(1, STRENGTH), glitches(2, STRENGTH));
        for position in [0usize, 1, 20, 21] {
            assert_ne!(one[position], two[position], "dessin {}", position + 1);
        }
        // d'un dessin au suivant, il change
        let first = glitches(5, STRENGTH);
        assert!(first.windows(2).filter(|pair| pair[0] != pair[1]).count() >= 18);
    }

    #[test]
    fn it_changes_only_from_one_drawing_to_the_next() {
        // à chaque image d'animation, le dessin du film ; le glitch suit le dessin, pas l'image
        let seed = 42;
        let mut previous: Option<(u32, Glitch)> = None;
        for frame in 0..FILM.frames() {
            let position = FILM.at(frame).expect("dans le film");
            let glitch = Glitch::of(FILM, position, seed, STRENGTH);
            if let Some((before, ref was)) = previous
                && before == position
            {
                assert_eq!(&glitch, was, "image {frame} : le même dessin, le même glitch");
            }
            previous = Some((position, glitch));
        }
        // chaque dessin est tenu deux images : le glitch aussi
        assert!(FILM.sheet.iter().all(|&(_, hold)| hold == 2));
    }

    #[test]
    fn it_is_bounded_and_its_bands_cover_the_image_without_gap_or_overlap() {
        let (reach_x, reach_y) = reach(SIZE);
        assert_eq!((reach_x, reach_y), (96 + 23 + 31, 9 + 4));
        let mut torn_seen = 0;
        let mut missing_seen = 0;
        let mut shaken = 0;
        for seed in 0..300u64 {
            for strength in [STRENGTH, 3.0] {
                for (position, glitch) in glitches(seed, strength).iter().enumerate() {
                    let at = format!("graine {seed}, force {strength}, dessin {}", position + 1);
                    assert!((0.0..=1.0).contains(&glitch.force), "{at}");
                    assert!(!glitch.bands.is_empty() && glitch.bands.len() <= MAX_BANDS, "{at}");
                    let mut y = 0;
                    for pair in glitch.bands.windows(2) {
                        assert!(!(pair[0].is_plain() && pair[1].is_plain()), "{at} : deux bandes immobiles");
                    }
                    for band in &glitch.bands {
                        assert_eq!(band.rows.0, y, "{at} : sans trou ni chevauchement");
                        assert!(band.rows.1 > 0, "{at}");
                        y += band.rows.1;
                        assert!(band.shift.abs() <= limit(SHIFT, SIZE.0), "{at} : {band:?}");
                        torn_seen += usize::from(band.shift != 0);
                        missing_seen += usize::from(band.missing);
                    }
                    assert_eq!(y, SIZE.1, "{at} : toute la hauteur");
                    let torn = glitch.bands.iter().filter(|band| !band.is_plain()).count();
                    assert!(torn <= MAX_TORN, "{at}");
                    assert_eq!(glitch.doubles[0].color, VIOLET);
                    assert_eq!(glitch.doubles[1].color, GREEN);
                    for (index, double) in glitch.doubles.iter().enumerate() {
                        assert!((0.0..=1.0).contains(&double.opacity), "{at}");
                        assert!(double.offset.0.abs() <= limit(DOUBLE.1, SIZE.0), "{at}");
                        assert!(double.offset.1.abs() <= limit(DOUBLE_RISE, SIZE.1), "{at}");
                        // le violet à gauche, le vert à droite
                        let side = if index == 0 { -1 } else { 1 };
                        assert!(double.offset.0 * side >= 0, "{at} : {double:?}");
                    }
                    assert!(glitch.shake.0.abs() <= limit(SHAKE.0, SIZE.0), "{at}");
                    assert!(glitch.shake.1.abs() <= limit(SHAKE.1, SIZE.1), "{at}");
                    shaken += usize::from(glitch.shake != (0, 0));
                    // tout ensemble, jamais plus loin que `reach`
                    let widest = glitch.bands.iter().map(|band| band.shift.abs()).max().unwrap_or(0)
                        + glitch.shake.0.abs()
                        + glitch.doubles.iter().map(|double| double.offset.0.abs()).max().unwrap_or(0);
                    assert!(widest <= reach_x, "{at}");
                }
            }
        }
        // chaque chose se voit
        assert!(
            torn_seen > 1000 && missing_seen > 100 && shaken > 500,
            "{torn_seen} {missing_seen} {shaken}"
        );
    }

    #[test]
    fn at_zero_strength_there_is_none_and_the_image_is_as_drawn() {
        for seed in 0..100u64 {
            for glitch in glitches(seed, 0.0) {
                assert_eq!(glitch, Glitch::none(SIZE));
                assert!(glitch.is_still());
            }
        }
        let none = Glitch::none(SIZE);
        assert_eq!(none.bands, [Band { rows: (0, 1080), shift: 0, missing: false }]);
        assert_eq!(Glitch::with_force(f32::NAN, SIZE, 3, false), none);
        assert_eq!(Glitch::with_force(-1.0, SIZE, 3, true), none);
        // un film sans enveloppe : aucun glitch, quelle que soit la force
        let plain = Film { glitch: None, ..FILM_UP_CLOSE };
        assert!(Glitch::of(&plain, 0, 9, 5.0).is_still());
        // à pleine force, jamais immobile
        for seed in 0..100u64 {
            assert!(!Glitch::of(FILM, 0, seed, STRENGTH).is_still(), "graine {seed}");
        }
    }

    #[test]
    fn it_is_strong_at_the_start_and_the_end_lighter_in_the_middle_and_lightest_on_her_face() {
        // l'enveloppe, dessin par dessin
        let force = UP_CLOSE.force;
        assert_eq!(force.len(), FILM.len() as usize);
        assert!(force[..6].iter().all(|&f| f >= 0.5), "elle surgit : forte longtemps");
        assert!(force[6..12].iter().all(|&f| (0.3..=0.45).contains(&f)), "elle regarde autour d'elle");
        assert!(force[12..19].iter().all(|&f| (0.12..=0.18).contains(&f)), "son visage à la vitre");
        assert!(force[19..].iter().all(|&f| (0.7..=1.0).contains(&f)), "elle repart");
        assert_eq!((UP_CLOSE.face, BURST), ((12, 18), 0.9));
        // ses premiers et ses derniers dessins plus forts que le milieu, rafales comprises
        for edge in [force[0], force[1], force[20], force[21]] {
            assert!(force[6..19].iter().all(|&m| m < edge) && BURST <= edge, "{edge}");
        }
        // mesuré sur les glitchs eux-mêmes, sur beaucoup de lancements
        let seeds = 400u64;
        let mut disorder = vec![0.0f32; FILM.len() as usize];
        for seed in 0..seeds {
            for (position, glitch) in glitches(seed, STRENGTH).iter().enumerate() {
                disorder[position] += glitch.disorder(SIZE) / seeds as f32;
            }
        }
        let mean =
            |range: std::ops::Range<usize>| disorder[range.clone()].iter().sum::<f32>() / range.len() as f32;
        let (start, middle, face, end) = (mean(0..3), mean(6..12), mean(12..19), mean(19..22));
        assert!(start > 1.5 * middle && end > 1.5 * middle, "{disorder:?}");
        assert!(middle > 3.0 * face && face > 0.0, "{disorder:?}");
    }

    #[test]
    fn on_her_face_mostly_her_doubles_rare_thin_tears_and_never_a_missing_band() {
        let (mut drawings, mut tears) = (0, 0);
        let mut doubles = (0.0f32, 0.0f32);
        for seed in 0..1000u64 {
            // quelle que soit la force : son visage n'est jamais rayé d'une bande manquante
            for strength in [STRENGTH, 5.0] {
                for position in UP_CLOSE.face.0..=UP_CLOSE.face.1 {
                    let glitch = Glitch::of(FILM, position, seed, strength);
                    let at = format!("graine {seed}, force {strength}, dessin {}", position + 1);
                    assert!(glitch.bands.iter().all(|band| !band.missing), "{at}");
                    let torn: Vec<&Band> = glitch.bands.iter().filter(|band| !band.is_plain()).collect();
                    assert!(torn.len() <= 1, "{at}");
                    let thinnest = (FACE_ROWS * SIZE.1 as f32).round() as u32;
                    assert!(torn.iter().all(|band| band.rows.1 <= thinnest), "{at} : {torn:?}");
                    if strength == STRENGTH {
                        drawings += 1;
                        tears += torn.len();
                        doubles.0 += glitch.doubles.iter().map(|double| double.opacity).sum::<f32>() / 2.0;
                        doubles.1 +=
                            glitch.doubles.iter().map(|double| double.offset.0.abs() as f32).sum::<f32>()
                                / 2.0;
                    }
                }
            }
        }
        let (opacity, offset) = (doubles.0 / drawings as f32, doubles.1 / drawings as f32);
        println!(
            "son visage : {tears} déchirures sur {drawings} dessins ; doubles à {opacity:.2}, à {offset:.1} px"
        );
        // de rares déchirures (de l'ordre d'un dessin sur quatre), des doubles qui se voient
        assert!(tears * 8 > drawings && tears * 2 < drawings, "{tears} sur {drawings}");
        assert!(opacity > 0.45 && offset > 4.0, "{opacity} {offset}");
    }

    #[test]
    fn one_or_two_bursts_fall_in_the_middle_never_on_her_face() {
        let (mut one, mut two) = (0, 0);
        for seed in 0..1000u64 {
            let bursts = UP_CLOSE.bursts(seed);
            let first = bursts[0].expect("au moins une");
            for start in bursts.into_iter().flatten() {
                assert!(start >= UP_CLOSE.bursts.0 && start + BURST_LENGTH - 1 <= UP_CLOSE.bursts.1);
                // jamais sur son visage collé à la vitre (dessins 13 à 19)
                assert!(start + BURST_LENGTH - 1 < UP_CLOSE.face.0, "graine {seed}");
                for position in start..start + BURST_LENGTH {
                    assert!(UP_CLOSE.at(position, seed) >= BURST);
                }
            }
            match bursts[1] {
                Some(second) => {
                    two += 1;
                    assert!(second.abs_diff(first) > BURST_LENGTH, "graine {seed} : elles se touchent");
                }
                None => one += 1,
            }
            // hors de ses rafales, l'enveloppe telle quelle
            let bursting = |position: u32| {
                bursts.into_iter().flatten().any(|start| (start..start + BURST_LENGTH).contains(&position))
            };
            for position in 0..FILM.len() {
                if !bursting(position) {
                    assert_eq!(UP_CLOSE.at(position, seed), UP_CLOSE.force[position as usize]);
                }
            }
        }
        assert!(one > 300 && two > 200, "{one} {two}");
        // au-delà de sa feuille, rien
        assert_eq!(UP_CLOSE.at(99, 0), 0.0);
    }
}
