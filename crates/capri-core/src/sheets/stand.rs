//! Les planches de sa pose debout : elle se tient sur la pointe des pieds, une main sur la
//! hanche, ses cheveux montent et retombent ; elle cligne des yeux, et sa bouche bouge quand elle
//! parle.
//!
//! **D'où elles viennent.** Du même artiste que sa pose assise, dans le même dépôt et à la même
//! révision (voir `CREDITS.md` et le module parent), téléchargées par `tools/fetch-assets.sh` dans
//! `assets/capri/stand/`. Ce module ne contient aucune image ni aucune ligne de leur code.
//! Quand elles manquent, elle garde son dessin debout d'avant (`capri/capri-cutout.png`).
//!
//! **D'où viennent les nombres.** Tous ont été mesurés sur les images elles-mêmes (canal alpha,
//! ligne par ligne, et comparaison des cases entre elles), sauf deux, qu'une image ne dit pas, lus
//! dans le code du dépôt d'origine sans rien en reprendre : la cadence d'origine (24 images par
//! seconde pour la boucle et le clignement, 12 pour la bouche) et le rapport d'échelle entre ses
//! poses debout et assise ([`STAND_OVER_PERCH`] : le rapport du projet d'origine est 1,11 ; c'est
//! 1,20 qui a été choisi, à l'œil). Comme pour sa pose assise, les dessins sont tenus « en deux » à
//! 24 i/s (voir [`crate::anim`]).
//!
//! **Les planches** sont des bandes horizontales de cases de [`CELL_WIDTH`] × 384 px : la boucle
//! (60 dessins), le clignement (8) et la bouche (5). Le clignement et la bouche se posent sur la
//! boucle à la même place : la bouche est un petit morceau de visage, sur fond transparent, à
//! dessiner par-dessus le dessin de la boucle (son visage ne bouge pas d'un pixel d'un dessin à
//! l'autre).
//!
//! **Pas de flottement dans la boucle.** D'un dessin à l'autre, son visage, son torse et ses
//! jambes restent exactement à la même place dans la case (mesuré : aucun décalage, à un pixel
//! près, entre le dessin 0 et chacun des autres) ; la pointe de son pied ne quitte pas la ligne
//! 365-366. Seul le haut de ses cheveux descend de [`HAIR_DROP`] px (lignes 20 à 30) puis
//! remonte, sur le tour. Le flottement que le programme ajoute (`body.rs`) ne s'additionne donc à
//! rien : il reste le seul.
//!
//! Le texte est en pixels des planches (y vers le bas, origine en haut à gauche de la case).

use crate::anim::Sheet;
use crate::geom::Vec2;

use super::{CELL_HEIGHT, HOLD, Layout, STEP, Shown, WrongSize};

/// Le dossier des planches, dans celui des images (`assets/`).
pub const DIR: &str = "capri/stand";
/// Largeur d'une case, pour les trois planches.
pub const CELL_WIDTH: u32 = 116;
/// Le haut de sa silhouette (alpha non nul) au plus haut de la boucle : la ligne 20, celle du
/// bout de ses cheveux aux dessins 0 à 8 et 54 à 59.
pub const TOP: u32 = 20;
/// Sous sa silhouette : la dernière ligne non vide est la 366 (la pointe de son pied droit), dans
/// tous les dessins, à une ligne près.
pub const BOTTOM: u32 = 367;
/// Sa hauteur debout dans les planches, des cheveux à la pointe du pied : 347 px sur 384. C'est
/// elle qui doit valoir « sa hauteur debout » à l'écran (voir [`scale`]).
pub const STATURE: u32 = BOTTOM - TOP;
/// De combien le haut de ses cheveux descend au fil de la boucle (ligne 20 aux dessins 0 à 8,
/// ligne 30 aux dessins 24 à 30, puis il remonte).
pub const HAIR_DROP: u32 = 10;
/// Ses pieds, comme les mesure `AlphaMask::feet` (seuil 96, celui du masque de clic ; bande de 2 %
/// de la hauteur) : le milieu du bas de sa silhouette est à x = 52,5 et la dernière ligne pleine
/// est la 363, dans les 60 dessins.
pub const FEET: (f32, f32) = (52.5, 364.0);
/// Le clignement remplace, dans la boucle, les dessins 24 à 31 (comptés depuis 0). Mesuré : chaque
/// case du clignement est la plus proche de ces dessins-là, dans l'ordre ; ses cases 1 à 6 ont les
/// yeux fermés, 0 et 7 ouverts.
pub const BLINK_FROM: u32 = 24;
/// Où est la bouche dans une case : de (58, 117) à (74, 131) inclus, dans les cinq dessins ;
/// rien n'est dessiné ailleurs dans sa planche.
pub const MOUTH_BOX: ((u32, u32), (u32, u32)) = ((58, 117), (74, 131));
/// Combien de fois un pixel de ses planches debout est plus grand à l'écran qu'un pixel de ses
/// planches assises, pour qu'elle ait la même taille dans les deux poses.
///
/// Une image ne le dit pas directement : les deux poses ne montrent aucune partie du corps sous le
/// même angle. 1,11 est le rapport qu'emploie le dépôt d'origine (lu dans son code). Nos mesures,
/// grossières, le confirment : du cou à l'entrejambe, 98 px debout contre 106 assise (1,08) ; du
/// haut des cheveux à l'assise, 237 px assise contre 203 debout jusqu'à l'entrejambe, alors
/// qu'elle est penchée en avant (au moins 1,17 pour le seul torse) ; et à 1,0, assise, les jambes
/// pendantes, elle serait aussi haute que debout.
///
/// Regardé sur des images rendues côte à côte (le 11 novembre 2026, `portraits.rs` de l'app) : les
/// deux dessins n'ont pas les mêmes proportions (assise, ses cheveux plus hérissés, sa tête plus
/// grande, environ 1,4 du haut des cheveux au menton, et ses jambes dessinées bien plus longues).
/// À 1,11, elle paraît un peu plus grande assise, la tête surtout ; à 1,20, les têtes se
/// rapprochent.
///
/// Choisi : **1,20**, par l'utilisateur le 12 novembre 2026, après l'avoir vue en mouvement à 1,11
/// et avoir regardé les deux comparaisons rendues. Assise, elle est dessinée un peu plus petite
/// qu'avec 1,11, le rapport du projet d'origine. `portraits.rs` de l'app rend encore la
/// comparaison avec 1,11 (`cote-a-cote-a-1-11.png`).
pub const STAND_OVER_PERCH: f32 = 1.20;

/// Une planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Strip {
    /// Debout, en boucle : 60 dessins.
    Stand,
    /// Les yeux qui se ferment et se rouvrent : 8 dessins, à la place des dessins 24 à 31.
    StandBlink,
    /// Sa bouche quand elle parle : 5 formes, à poser sur n'importe quel dessin des deux autres.
    StandMouth,
}

impl Strip {
    pub const ALL: [Strip; 3] = [Strip::Stand, Strip::StandBlink, Strip::StandMouth];

    /// Le nom du fichier, dans [`DIR`] (le même que dans le dépôt d'origine).
    pub fn file(self) -> &'static str {
        match self {
            Strip::Stand => "idle.png",
            Strip::StandBlink => "idle_blink.png",
            Strip::StandMouth => "idle_mouth.png",
        }
    }

    /// Le nombre de dessins.
    pub fn count(self) -> u32 {
        match self {
            Strip::Stand => 60,
            Strip::StandBlink => 8,
            Strip::StandMouth => 5,
        }
    }

    /// Taille d'une case.
    pub fn cell(self) -> (u32, u32) {
        (CELL_WIDTH, CELL_HEIGHT)
    }

    /// Taille de la planche telle qu'elle est téléchargée : une bande horizontale.
    pub fn strip_size(self) -> (u32, u32) {
        self.layout().strip_size()
    }

    /// Combien de cases par ligne une fois remise en grille ([`repack`]) : la boucle, une bande
    /// de 6 960 px, devient une grille de 15 × 4 cases (1 740 × 1 536 px).
    pub fn columns(self) -> u32 {
        match self {
            Strip::Stand => 15,
            Strip::StandBlink => 8,
            Strip::StandMouth => 5,
        }
    }

    /// Taille de la grille ([`repack`]).
    pub fn grid_size(self) -> (u32, u32) {
        self.layout().grid_size()
    }

    /// Le coin haut gauche de la case `index` dans la grille, en px.
    pub fn cell_origin(self, index: u32) -> (u32, u32) {
        self.layout().cell_origin(index)
    }

    fn layout(self) -> Layout {
        Layout { cell: self.cell(), count: self.count(), columns: self.columns() }
    }

    /// Un dessin sur combien est montré : un sur [`STEP`] pour la boucle et le clignement (qui
    /// remplace les dessins 24 à 31, un sur deux aussi), toutes les formes de la bouche.
    pub fn step(self) -> u32 {
        match self {
            Strip::Stand | Strip::StandBlink => STEP,
            Strip::StandMouth => 1,
        }
    }
}

/// Les seuls dessins montrés d'une planche, serrés dans une grille plus petite : ce qui est
/// confié à la carte graphique (voir [`super::shown`]). La boucle : 1 740 × 768 px au lieu de
/// 1 740 × 1 536.
pub fn shown(strip: Strip) -> Shown {
    let columns = match strip {
        Strip::Stand => 15,
        Strip::StandBlink => 4,
        Strip::StandMouth => 5,
    };
    Shown::new(strip.layout(), strip.step(), columns)
}

/// Un dessin : quelle planche, quelle case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub strip: Strip,
    pub index: u32,
}

/// Remet une bande horizontale (RGBA 8 bits, `strip.strip_size()`) en grille de
/// `strip.columns()` cases par ligne.
pub fn repack(strip: Strip, width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, WrongSize> {
    strip.layout().repack(width, height, rgba)
}

/// Les pixels de la case `index` d'une planche remise en grille ([`repack`]), de taille
/// `strip.cell()`, par exemple pour le masque de clic. `None` si la grille n'a pas la bonne
/// taille ou si la case n'existe pas.
pub fn cell_pixels(strip: Strip, grid: &[u8], index: u32) -> Option<Vec<u8>> {
    strip.layout().cell_pixels(grid, index)
}

/// Debout, en boucle : un dessin sur deux, chacun tenu [`HOLD`] images (30 dessins, 60 images :
/// 2,5 s par tour, la durée d'origine). Avec `blink`, elle cligne des yeux au milieu du tour (les
/// dessins 24 à 31 viennent du clignement : yeux fermés pendant 6 images, 0,25 s) ; sans, les yeux
/// restent ouverts. À l'appelant de choisir les tours où elle cligne.
pub fn standing(blink: bool) -> Sheet<Cell> {
    let blinking = BLINK_FROM..BLINK_FROM + Strip::StandBlink.count();
    let keys: Vec<(Cell, u32)> = (0..Strip::Stand.count())
        .step_by(STEP as usize)
        .map(|index| {
            let cell = if blink && blinking.contains(&index) {
                Cell { strip: Strip::StandBlink, index: index - BLINK_FROM }
            } else {
                Cell { strip: Strip::Stand, index }
            };
            (cell, HOLD)
        })
        .collect();
    Sheet::new(&keys)
}

/// Les cinq formes de sa bouche, dans l'ordre de la planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mouth {
    /// Fermée, en accent circonflexe.
    Shut,
    /// À peine ouverte.
    Small,
    /// Les dents serrées.
    Teeth,
    /// Ouverte, les dents du haut.
    Open,
    /// Grande ouverte.
    Wide,
}

impl Mouth {
    pub const ALL: [Mouth; 5] = [Mouth::Shut, Mouth::Small, Mouth::Teeth, Mouth::Open, Mouth::Wide];

    /// Sa case dans la planche de la bouche.
    pub fn cell(self) -> Cell {
        let index = Mouth::ALL.iter().position(|&m| m == self).unwrap_or(0) as u32;
        Cell { strip: Strip::StandMouth, index }
    }
}

/// Elle parle : sa bouche change de forme tous les deux images (12 fois par seconde, la cadence
/// d'origine de cette planche), dans un ordre à nous, sans rapport avec les mots. À dessiner
/// par-dessus [`standing`], qui continue dessous ; quand elle se tait, on ne pose rien (la boucle
/// a déjà sa bouche fermée).
pub fn talking() -> Sheet<Cell> {
    use Mouth::*;
    let keys: Vec<(Cell, u32)> = [Open, Small, Wide, Teeth, Open, Shut, Small, Wide, Teeth, Shut]
        .iter()
        .map(|mouth| (mouth.cell(), HOLD))
        .collect();
    Sheet::new(&keys)
}

/// [`FEET`] en coordonnées normalisées de la case (0..1), comme `AlphaMask::feet`.
pub fn feet_uv() -> Vec2 {
    Vec2::new(FEET.0 / CELL_WIDTH as f32, FEET.1 / CELL_HEIGHT as f32)
}

/// Combien de pixels d'écran pour un pixel de ses planches debout, pour qu'elle mesure `height`
/// pixels d'écran debout, des cheveux à la pointe du pied ([`STATURE`]). `height` est « sa
/// hauteur debout » : 300 points aujourd'hui, multipliés par le facteur d'échelle de l'écran.
pub fn scale(height: f32) -> f32 {
    height / STATURE as f32
}

/// Pareil pour ses planches assises (le module parent), à la même échelle qu'elle debout.
pub fn perch_scale(height: f32) -> f32 {
    scale(height) / STAND_OVER_PERCH
}

/// La taille à l'écran d'une case debout, pour qu'elle mesure `height` debout. Plus haute que
/// `height` : la case a de la marge au-dessus de ses cheveux et sous son pied.
pub fn cell_size(height: f32) -> Vec2 {
    let s = scale(height);
    Vec2::new(CELL_WIDTH as f32 * s, CELL_HEIGHT as f32 * s)
}

/// La taille à l'écran d'une case de sa pose assise, à la même échelle que [`cell_size`].
pub fn perch_cell_size(strip: super::Strip, height: f32) -> Vec2 {
    let ((w, h), s) = (strip.cell(), perch_scale(height));
    Vec2::new(w as f32 * s, h as f32 * s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::FRAME_RATE;

    #[test]
    fn strips_have_the_measured_sizes() {
        let sizes: Vec<(u32, u32)> = Strip::ALL.iter().map(|s| s.strip_size()).collect();
        assert_eq!(sizes, [(6960, 384), (928, 384), (580, 384)]);
    }

    #[test]
    fn grids_stay_small_and_hold_every_cell() {
        assert_eq!(Strip::Stand.grid_size(), (1740, 1536));
        for strip in Strip::ALL {
            let (gw, gh) = strip.grid_size();
            assert!(gw <= 2304 && gh <= 2304, "{strip:?} : {gw} × {gh}");
            let (cw, ch) = strip.cell();
            let mut seen = std::collections::HashSet::new();
            for index in 0..strip.count() {
                let (x, y) = strip.cell_origin(index);
                assert!(x + cw <= gw && y + ch <= gh, "{strip:?} {index}");
                assert!(seen.insert((x, y)), "{strip:?} {index} : case déjà prise");
            }
            // aucune case vide : la grille est pleine
            assert_eq!(gw / cw * (gh / ch), strip.count(), "{strip:?}");
        }
    }

    #[test]
    fn repack_and_cell_pixels_keep_each_cell_whole() {
        let strip = Strip::Stand;
        let (w, h) = strip.strip_size();
        let rgba: Vec<u8> =
            (0..w * h).flat_map(|i| [((i % w) / CELL_WIDTH) as u8, (i / w % 256) as u8, 0, 255]).collect();
        let grid = repack(strip, w, h, &rgba).unwrap();
        for index in [0, 14, 15, 59] {
            let cell = cell_pixels(strip, &grid, index).unwrap();
            for y in [0, 255, CELL_HEIGHT - 1] {
                let at = ((y * CELL_WIDTH + CELL_WIDTH - 1) * 4) as usize;
                assert_eq!(cell[at..at + 2], [index as u8, (y % 256) as u8], "case {index}, ligne {y}");
            }
        }
        assert_eq!(
            repack(strip, 220, 420, &vec![0; 220 * 420 * 4]),
            Err(WrongSize { expected: (6960, 384), found: (220, 420) })
        );
    }

    #[test]
    fn a_loop_lasts_two_and_a_half_seconds_in_twos() {
        for sheet in [standing(false), standing(true)] {
            assert_eq!(sheet.len(), 60);
            assert_eq!(sheet.len() as f64 / FRAME_RATE, 2.5);
            for frame in (0..60).step_by(2) {
                assert_eq!(sheet.at(frame), sheet.at(frame + 1));
                assert_ne!(sheet.at(frame), sheet.at(frame + 2));
            }
        }
    }

    #[test]
    fn she_blinks_only_where_her_artist_drew_it() {
        let (open, blink) = (standing(false), standing(true));
        let mut shut = 0;
        for frame in 0..60 {
            let drawing = frame as u32 / 2 * 2;
            assert_eq!(open.at(frame), Cell { strip: Strip::Stand, index: drawing });
            if (24..32).contains(&drawing) {
                let cell = blink.at(frame);
                assert_eq!(cell, Cell { strip: Strip::StandBlink, index: drawing - 24 });
                // cases 1 à 6 : yeux fermés
                shut += u32::from((1..7).contains(&cell.index));
            } else {
                assert_eq!(blink.at(frame), open.at(frame));
            }
        }
        assert_eq!(shut, 6, "les yeux fermés 6 images, 0,25 s");
    }

    #[test]
    fn her_mouth_moves_twelve_times_a_second_with_every_shape() {
        let sheet = talking();
        assert_eq!(sheet.len() % 2, 0);
        let mut shapes = std::collections::HashSet::new();
        for frame in (0..sheet.len()).step_by(2) {
            let cell = sheet.at(frame);
            assert_eq!(cell.strip, Strip::StandMouth);
            assert!(cell.index < Strip::StandMouth.count());
            assert_eq!(cell, sheet.at(frame + 1), "tenue deux images");
            assert_ne!(cell, sheet.at(frame + 2), "puis une autre forme");
            shapes.insert(cell.index);
        }
        assert_eq!(shapes.len(), 5);
        assert_eq!(Mouth::Shut.cell().index, 0);
        assert_eq!(Mouth::Wide.cell().index, 4);
    }

    #[test]
    fn only_the_drawings_shown_are_packed_and_every_one_shown_is_there() {
        for sheet in [standing(true), standing(false), talking()] {
            for frame in 0..sheet.len() {
                let cell = sheet.at(frame);
                assert!(shown(cell.strip).origin(cell.index).is_some(), "{cell:?}");
            }
        }
        assert_eq!(shown(Strip::Stand).grid_size(), (1740, 768));
        assert_eq!(shown(Strip::StandBlink).grid_size(), (464, 384));
        assert_eq!(shown(Strip::StandMouth).grid_size(), Strip::StandMouth.grid_size());
        // la boucle serrée : le dessin 58 à sa place, intact
        let strip = Strip::Stand;
        let (w, h) = strip.strip_size();
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [((i % w) / CELL_WIDTH) as u8, 1, 2, 255]).collect();
        let grid = repack(strip, w, h, &rgba).unwrap();
        let packed = shown(strip).pack(&grid).expect("la bonne taille");
        let (x, y) = shown(strip).origin(58).unwrap();
        let at = (((y + 383) * 1740 + x + CELL_WIDTH - 1) * 4) as usize;
        assert_eq!(packed[at..at + 4], [58, 1, 2, 255]);
        assert_eq!(shown(strip).origin(59), None, "jamais montré");
    }

    #[test]
    fn every_cell_exists() {
        for sheet in [standing(true), standing(false), talking()] {
            for frame in 0..sheet.len() {
                let cell = sheet.at(frame);
                assert!(cell.index < cell.strip.count(), "{cell:?}");
            }
        }
    }

    #[test]
    fn her_silhouette_fits_in_the_cell() {
        const { assert!(TOP + HAIR_DROP < 64 && BOTTOM <= CELL_HEIGHT && STATURE == 347) };
        let feet = feet_uv();
        assert!(
            (feet.x * CELL_WIDTH as f32 - 52.5).abs() < 1e-3
                && (feet.y * CELL_HEIGHT as f32 - 364.0).abs() < 1e-3
        );
        assert!(FEET.1 <= BOTTOM as f32);
        let ((x0, y0), (x1, y1)) = MOUTH_BOX;
        assert!(x0 < x1 && x1 < CELL_WIDTH && y0 < y1 && y1 < CELL_HEIGHT);
    }

    #[test]
    fn her_standing_height_is_the_one_asked_and_sitting_is_to_scale() {
        // 300 points sur un écran Retina
        let height = 600.0;
        let s = scale(height);
        assert!((STATURE as f32 * s - height).abs() < 1e-3, "des cheveux au pied : sa hauteur debout");
        let cell = cell_size(height);
        assert!(
            (cell.y - 384.0 * 600.0 / 347.0).abs() < 1e-2 && (cell.x - 116.0 * 600.0 / 347.0).abs() < 1e-2
        );
        // assise : chaque pixel de ses planches un peu moins grand, de STAND_OVER_PERCH
        assert!((perch_scale(height) * STAND_OVER_PERCH - s).abs() < 1e-6);
        let perch = perch_cell_size(crate::sheets::Strip::Perch, height);
        assert!((perch.y * STAND_OVER_PERCH - cell.y).abs() < 1e-2);
        assert!((perch.x / perch.y - 196.0 / 384.0).abs() < 1e-6);
        // assise, des cheveux (ligne 29) au bout des pieds (ligne 380), elle est moins haute que debout
        let sitting = (crate::sheets::HEADROOM + crate::sheets::HANG) as f32 * perch_scale(height);
        assert!(sitting < height, "{sitting} ≥ {height}");
    }
}
