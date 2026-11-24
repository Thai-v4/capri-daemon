//! Les planches de sa pose allongée : elle s'allonge sur le bord d'une fenêtre, la tête et ses
//! cheveux à gauche (ses cheveux et un bras pendent devant la fenêtre), ses jambes à droite (l'une
//! pend, l'autre se replie) ; elle cligne des yeux, parle, dort, et s'efface ; et un éclat
//! ([`Strip::LieGlitch`]), avec lequel elle s'allonge, et qu'elle a quand on clique sur elle
//! allongée.
//!
//! **D'où elles viennent.** Du nouveau jeu de l'artiste du projet
//! [lachydotmcg/capri-desktop](https://github.com/lachydotmcg/capri-desktop) (voir `CREDITS.md`),
//! à la même révision que ses poses assise et debout, téléchargées par `tools/fetch-assets.sh`
//! dans `assets/capri/laydown/`. Ce module ne contient aucune image ni aucune ligne de leur code :
//! seulement la description de leurs planches. Pas leurs variantes retournées (`_flipped`), ni son
//! clignement glitché.
//!
//! **Chargées à la demande** : elles pèsent lourd (trois boucles de 60 dessins de 372 × 250 px), et
//! elle n'est allongée qu'une petite partie du temps. Au lancement, seuls leurs en-têtes sont lus ;
//! les six planches sont lues, décodées et confiées à la carte graphique quand elle décide de
//! s'allonger, et lâchées quand elle a quitté la pose depuis un moment (voir `capri-app`). Toutes
//! les six ensemble ([`Strip::ALL`]), ou elle ne s'allonge jamais.
//!
//! **D'où viennent les nombres.** Mesurés sur les images (canal alpha, colonne par colonne et
//! ligne par ligne, et comparaison des cases entre elles), sauf trois, qu'une image ne dit pas,
//! lus dans le code du dépôt d'origine sans rien en reprendre : la cadence d'origine (30 dessins
//! par seconde pour les boucles et le clignement, 15 pour l'éclat et le départ), l'échelle face à
//! sa pose assise ([`LIE_OVER_PERCH`] : un pixel de ces planches vaut 0,85 pixel de ses planches
//! assises), et sa hanche dans les deux poses ([`HIP`], [`PERCH_HIP`]), qui reste à la même place
//! quand elle s'allonge et se rassoit. Ici, comme partout chez Capri, les dessins sont tenus un
//! nombre entier d'images à 24 i/s (voir [`crate::anim`]).
//!
//! **Les planches** sont des bandes horizontales de 250 px de haut. La boucle (60 dessins), sa
//! parole (la même boucle, dessin pour dessin, la bouche en plus : mesuré, leurs silhouettes sont
//! identiques), son sommeil (60) et son clignement (8, à la place des dessins 28 à 35, comme assise)
//! ont des cases de [`NARROW`] px ; l'éclat (10) et le départ (12), plus larges à cause de leurs
//! éclats, de [`WIDE`] px, où le même dessin est centré, décalé de [`WIDE_SHIFT`] px (mesuré : la
//! dernière case de l'éclat et la première du départ sont les plus proches du dessin 0 de la boucle
//! à ce décalage).
//!
//! **Le bord de la fenêtre** passe à [`SEAT_Y`] px du haut de chaque case : la ligne où son départ
//! s'efface (ses dernières cases, un trait sur les lignes 141 à 144) et le dessous de ses hanches
//! (lignes 139 à 143, de la colonne 284 jusqu'à la 294 à 308 selon le dessin, où ses jambes
//! commencent à pendre). Son torse repose sur le bord de son épaule ([`SHOULDER`]) à ses hanches
//! (il remonte un peu vers ses épaules) ; ses cheveux et son bras pendent à gauche, ses jambes à
//! droite.
//!
//! **Où elle s'allonge** : là où elle était assise, sa hanche à la même place ([`SEAT_X`] : le point
//! de la case qui va sur le point posé de sa pose assise, `super::SEAT_X`). Elle s'étend vers la
//! gauche : il lui faut de la place de ce côté (voir `look::lying_stature`).
//!
//! **Son sommeil** est montré un dessin sur trois, chacun tenu trois images (8 dessins par seconde,
//! un tour de 2,5 s comme ses autres boucles) : endormie, elle est redessinée 8 fois par seconde
//! au lieu de 12 éveillée. Mesuré : deux dessins montrés voisins de son sommeil diffèrent au plus de
//! 3,7 % de ses pixels pleins, moins que deux dessins voisins de sa boucle éveillée, un sur deux
//! (3,9 %) ; un sur quatre (6 par seconde) irait jusqu'à 4,9 %, une respiration plus saccadée.
//!
//! Le texte est en pixels des planches (y vers le bas, origine en haut à gauche de la case).

use crate::anim::Sheet;
use crate::geom::Vec2;

use super::{HOLD, Layout, STEP, Shown, stand};

/// Le dossier des planches, dans celui des images (`assets/`).
pub const DIR: &str = "capri/laydown";
/// Hauteur d'une case, pour toutes ces planches.
pub const CELL_HEIGHT: u32 = 250;
/// Largeur d'une case des boucles (allongée, parole, sommeil) et du clignement.
pub const NARROW: u32 = 372;
/// Largeur d'une case de l'éclat et du départ.
pub const WIDE: u32 = 440;
/// Dans une case large, le dessin est décalé d'autant vers la droite (centré : (440 − 372) / 2).
pub const WIDE_SHIFT: u32 = 34;
/// Le bord de la fenêtre, en px depuis le haut d'une case.
pub const SEAT_Y: u32 = 142;
/// Un pixel de ces planches vaut tant de pixels de ses planches assises (lu dans le code d'origine).
pub const LIE_OVER_PERCH: f32 = 0.85;
/// Sa hanche, dans une case étroite (lue dans le code d'origine) : au bout de son torse, là où ses
/// jambes commencent à pendre (mesuré : de la colonne 295 à la 309 selon le dessin, le dessous de
/// son corps sur la ligne du bord).
pub const HIP: f32 = 303.5;
/// Sa hanche dans une case de sa pose assise (lue dans le code d'origine : à l'arrière de ce qui
/// repose sur le bord ; mesuré, son corps y touche la ligne du bord à partir des colonnes 49 à 63
/// selon le dessin).
pub const PERCH_HIP: f32 = 47.2;
/// Le point de la case qui va sur le point posé sur le bord (celui de sa pose assise,
/// `super::SEAT_X`) : sa hanche à la même place que sa hanche assise.
pub const SEAT_X: f32 = HIP + (super::SEAT_X as f32 - PERCH_HIP) / LIE_OVER_PERCH;
/// Ce qu'elle occupe en largeur dans une case étroite, de la colonne 4 à la 367 incluse : l'union
/// de ses boucles et de son clignement, alpha non nul (l'éclat et le départ débordent un instant).
pub const SPAN: (u32, u32) = (4, 368);
/// Le haut de ses cheveux (ligne 2) et sous ses pieds (la dernière ligne dessinée est la 247, son
/// sommeil), dans ses boucles.
pub const TOP: u32 = 2;
pub const BOTTOM: u32 = 248;
/// Son épaule : la première colonne où le dessous de son torse, et non ses cheveux qui pendent,
/// est le plus bas de sa silhouette (colonne 137 ou 138 selon le dessin, dans ses trois boucles).
/// De là à sa hanche, son torse repose sur le bord.
pub const SHOULDER: u32 = 137;
/// Le clignement remplace, dans la boucle, les dessins 28 à 35 (comptés depuis 0). Mesuré : chaque
/// case du clignement est la plus proche de ces dessins-là, dans l'ordre.
pub const BLINK_FROM: u32 = 28;
/// Son sommeil : un dessin sur tant, chacun tenu autant d'images (voir le module).
pub const SLEEP_STEP: u32 = 3;
pub const SLEEP_HOLD: u32 = 3;

/// Une planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Strip {
    /// Allongée, éveillée (elle respire) : 60 dessins.
    Lie,
    /// Les yeux qui se ferment et se rouvrent : 8 dessins, à la place des dessins 28 à 35.
    LieBlink,
    /// Elle parle : la même boucle que [`Strip::Lie`], dessin pour dessin, la bouche en plus.
    LieTalk,
    /// Elle dort : 60 dessins.
    LieSleep,
    /// Un éclat : elle se brouille, puis se reforme sur le dessin 0 de sa boucle. 10 dessins, une
    /// fois ; c'est aussi avec lui qu'elle passe de sa pose assise à celle-ci.
    LieGlitch,
    /// Elle s'efface jusqu'à un trait sur le bord, puis plus rien : 12 dessins, une fois.
    LieLeave,
}

impl Strip {
    /// Toutes ensemble, ou elle ne s'allonge pas.
    pub const ALL: [Strip; 6] =
        [Strip::Lie, Strip::LieBlink, Strip::LieTalk, Strip::LieSleep, Strip::LieGlitch, Strip::LieLeave];

    /// Le nom du fichier, dans [`DIR`] (le même que dans le dépôt d'origine).
    pub fn file(self) -> &'static str {
        match self {
            Strip::Lie => "laydown.png",
            Strip::LieBlink => "laydown_blink.png",
            Strip::LieTalk => "laydown_talk.png",
            Strip::LieSleep => "laydown_sleep.png",
            Strip::LieGlitch => "laydown_glitch.png",
            Strip::LieLeave => "laydown_leave.png",
        }
    }

    /// Le nombre de dessins.
    pub fn count(self) -> u32 {
        match self {
            Strip::Lie | Strip::LieTalk | Strip::LieSleep => 60,
            Strip::LieBlink => 8,
            Strip::LieGlitch => 10,
            Strip::LieLeave => 12,
        }
    }

    /// Taille d'une case.
    pub fn cell(self) -> (u32, u32) {
        match self {
            Strip::LieGlitch | Strip::LieLeave => (WIDE, CELL_HEIGHT),
            _ => (NARROW, CELL_HEIGHT),
        }
    }

    /// Taille de la planche telle qu'elle est téléchargée : une bande horizontale.
    pub fn strip_size(self) -> (u32, u32) {
        (self.cell().0 * self.count(), CELL_HEIGHT)
    }

    /// Un dessin sur combien est montré : un sur [`STEP`] pour les boucles et le clignement, un sur
    /// [`SLEEP_STEP`] pour son sommeil, tous pour l'éclat et le départ.
    pub fn step(self) -> u32 {
        match self {
            Strip::Lie | Strip::LieTalk | Strip::LieBlink => STEP,
            Strip::LieSleep => SLEEP_STEP,
            Strip::LieGlitch | Strip::LieLeave => 1,
        }
    }

    /// Combien de dessins montrés par ligne, dans la grille confiée à la carte graphique : aucun
    /// côté au-delà de 2 304 px, des lignes pleines.
    fn columns(self) -> u32 {
        match self {
            Strip::Lie | Strip::LieTalk => 6,
            Strip::LieBlink | Strip::LieLeave => 4,
            Strip::LieSleep | Strip::LieGlitch => 5,
        }
    }

    /// Le point de la case qui se pose sur le point posé du bord ([`SEAT_X`], [`SEAT_Y`]).
    pub fn seat(self) -> (f32, u32) {
        let shift = if self.cell().0 == WIDE { WIDE_SHIFT } else { 0 };
        (SEAT_X + shift as f32, SEAT_Y)
    }

    /// [`Strip::seat`] en coordonnées normalisées de la case (0..1).
    pub fn seat_uv(self) -> Vec2 {
        let ((x, y), (w, h)) = (self.seat(), self.cell());
        Vec2::new(x / w as f32, y as f32 / h as f32)
    }
}

/// Les seuls dessins montrés d'une planche, serrés dans une grille : ce qui est confié à la carte
/// graphique, fait directement à partir de la bande (`Shown::pack_strip`). La boucle : 2 232 ×
/// 1 250 px (30 dessins), son sommeil 1 860 × 1 000 (20).
pub fn shown(strip: Strip) -> Shown {
    // la grille complète n'est jamais faite : seule la taille d'une case et le nombre de dessins
    // comptent ici
    let full = Layout { cell: strip.cell(), count: strip.count(), columns: strip.count() };
    Shown::new(full, strip.step(), strip.columns())
}

/// Un dessin : quelle planche, quelle case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub strip: Strip,
    pub index: u32,
}

/// Une boucle : un dessin sur `step`, chacun tenu `hold` images ; les dessins de `blink` à la
/// place de ceux de [`BLINK_FROM`], s'il y en a un.
fn looped(strip: Strip, step: u32, hold: u32, blink: Option<Strip>) -> Sheet<Cell> {
    let keys: Vec<(Cell, u32)> = (0..strip.count())
        .step_by(step as usize)
        .map(|index| {
            let cell = match blink {
                Some(blink) if (BLINK_FROM..BLINK_FROM + blink.count()).contains(&index) => {
                    Cell { strip: blink, index: index - BLINK_FROM }
                }
                _ => Cell { strip, index },
            };
            (cell, hold)
        })
        .collect();
    Sheet::new(&keys)
}

/// Une suite jouée une fois : chaque dessin tenu [`HOLD`] images. Elle boucle comme toute
/// [`Sheet`] : à l'appelant de s'arrêter après `len()` images.
fn once(strip: Strip) -> Sheet<Cell> {
    let keys: Vec<(Cell, u32)> = (0..strip.count()).map(|index| (Cell { strip, index }, HOLD)).collect();
    Sheet::new(&keys)
}

/// Allongée, en boucle : un dessin sur deux, tenu en deux (60 images, 2,5 s par tour). Avec
/// `blink`, elle cligne au milieu du tour.
pub fn lying(blink: bool) -> Sheet<Cell> {
    looped(Strip::Lie, STEP, HOLD, blink.then_some(Strip::LieBlink))
}

/// Elle parle, allongée : la même boucle que [`lying`], image pour image.
pub fn talking() -> Sheet<Cell> {
    looped(Strip::LieTalk, STEP, HOLD, None)
}

/// Elle dort, allongée : un dessin sur trois, tenu trois images (60 images, 2,5 s par tour).
pub fn sleeping() -> Sheet<Cell> {
    looped(Strip::LieSleep, SLEEP_STEP, SLEEP_HOLD, None)
}

/// Son éclat, allongée (une fois, 20 images : 0,83 s). Il finit sur le dessin 0 de sa boucle.
pub fn glitching() -> Sheet<Cell> {
    once(Strip::LieGlitch)
}

/// Elle s'efface de son bord, allongée (une fois, 24 images : 1 s ; les dernières cases ne sont
/// qu'un trait, puis rien).
pub fn leaving() -> Sheet<Cell> {
    once(Strip::LieLeave)
}

/// Combien de pixels d'écran pour un pixel de ces planches, pour une hauteur debout de `height`
/// pixels : [`LIE_OVER_PERCH`] de ses planches assises.
pub fn scale(height: f32) -> f32 {
    stand::perch_scale(height) * LIE_OVER_PERCH
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::FRAME_RATE;

    #[test]
    fn strips_have_the_measured_sizes() {
        let sizes: Vec<(u32, u32)> = Strip::ALL.iter().map(|s| s.strip_size()).collect();
        assert_eq!(sizes, [(22320, 250), (2976, 250), (22320, 250), (22320, 250), (4400, 250), (5280, 250)]);
    }

    #[test]
    fn the_grids_given_to_the_graphics_card_stay_small_and_full() {
        let mut bytes = 0u64;
        for strip in Strip::ALL {
            let grid = shown(strip);
            let (gw, gh) = grid.grid_size();
            assert!(gw <= 2304 && gh <= 2304, "{strip:?} : {gw} × {gh}");
            let (cw, ch) = strip.cell();
            assert_eq!(gw / cw * (gh / ch), grid.count(), "{strip:?} : des lignes pleines");
            let mut seen = std::collections::HashSet::new();
            for index in (0..strip.count()).filter_map(|index| grid.origin(index)) {
                assert!(index.0 + cw <= gw && index.1 + ch <= gh && seen.insert(index), "{strip:?}");
            }
            assert_eq!(seen.len() as u32, grid.count());
            bytes += u64::from(gw * gh * 4);
        }
        assert_eq!(shown(Strip::Lie).grid_size(), (2232, 1250));
        assert_eq!(shown(Strip::LieSleep).grid_size(), (1860, 1000));
        // ce qu'elles pèsent sur la carte graphique, toutes ensemble : 40,9 Mo
        assert_eq!(bytes, 40_928_000);
    }

    #[test]
    fn her_loops_last_two_and_a_half_seconds() {
        for sheet in [lying(false), lying(true), talking(), sleeping()] {
            assert_eq!(sheet.len(), 60);
            assert_eq!(sheet.len() as f64 / FRAME_RATE, 2.5);
        }
        // éveillée, un dessin toutes les deux images ; endormie, toutes les trois
        let (awake, asleep) = (lying(false), sleeping());
        for frame in (0..60).step_by(2) {
            assert_eq!(awake.at(frame), awake.at(frame + 1));
            assert_ne!(awake.at(frame), awake.at(frame + 2));
        }
        for frame in (0..60).step_by(3) {
            assert!((frame..frame + 3).all(|f| asleep.at(f) == asleep.at(frame)));
            assert_ne!(asleep.at(frame), asleep.at(frame + 3));
            assert_eq!(asleep.at(frame).index as u64, frame);
        }
        // l'éclat et le départ : chaque dessin, en deux
        assert_eq!((glitching().len(), leaving().len()), (20, 24));
        assert_eq!(glitching().at(19), Cell { strip: Strip::LieGlitch, index: 9 });
        assert_eq!(leaving().at(23), Cell { strip: Strip::LieLeave, index: 11 });
    }

    #[test]
    fn she_blinks_and_talks_on_the_drawings_of_her_loop() {
        let (open, blink, talk) = (lying(false), lying(true), talking());
        for frame in 0..60 {
            let drawing = frame as u32 / 2 * 2;
            assert_eq!(open.at(frame), Cell { strip: Strip::Lie, index: drawing });
            assert_eq!(talk.at(frame), Cell { strip: Strip::LieTalk, index: drawing });
            if (28..36).contains(&drawing) {
                assert_eq!(blink.at(frame), Cell { strip: Strip::LieBlink, index: drawing - 28 });
            } else {
                assert_eq!(blink.at(frame), open.at(frame));
            }
        }
    }

    #[test]
    fn every_drawing_shown_is_in_its_grid() {
        for sheet in [lying(true), lying(false), talking(), sleeping(), glitching(), leaving()] {
            for frame in 0..sheet.len() {
                let cell = sheet.at(frame);
                assert!(cell.index < cell.strip.count(), "{cell:?}");
                assert!(shown(cell.strip).origin(cell.index).is_some(), "{cell:?}");
            }
        }
    }

    #[test]
    fn her_hip_stays_where_it_was_on_the_same_edge_at_the_scale_of_her_seated_pose() {
        // sa hanche assise, à droite de la hanche allongée d'autant (en pixels assis)
        let k = scale(600.0);
        assert!((k / stand::perch_scale(600.0) - 0.85).abs() < 1e-6);
        let seat = Vec2::new(1000.0, 800.0);
        let perch_hip = seat.x - (super::super::SEAT_X as f32 - PERCH_HIP) * stand::perch_scale(600.0);
        let lie_hip = seat.x - (SEAT_X - HIP) * k;
        assert!((perch_hip - lie_hip).abs() < 1e-3, "{perch_hip} {lie_hip}");
        assert!((SEAT_X - 337.38).abs() < 0.01, "{SEAT_X}");
        // le point posé, dans la case, à la même place dans les cases larges
        for strip in Strip::ALL {
            let ((x, y), (w, h)) = (strip.seat(), strip.cell());
            assert!(x < w as f32 && y < h);
            let uv = strip.seat_uv();
            assert!((uv.x * w as f32 - x).abs() < 1e-3 && (uv.y * h as f32 - y as f32).abs() < 1e-3);
        }
        assert_eq!(Strip::LieGlitch.seat().0 - Strip::Lie.seat().0, WIDE_SHIFT as f32);
        // tout tient dans la case ; son torse, de l'épaule à la hanche, à gauche du point posé
        const { assert!(TOP < SEAT_Y && SEAT_Y < BOTTOM && BOTTOM <= CELL_HEIGHT && SPAN.1 <= NARROW) };
        const { assert!((SHOULDER as f32) < HIP && HIP < SEAT_X && SEAT_X < SPAN.1 as f32) };
    }
}
