//! Les planches de sa pose assise : elle s'assoit sur le bord d'une fenêtre, jambes pendantes,
//! cligne des yeux, parle, se balance sur ta musique, arrive et repart. Celles de sa pose debout,
//! du même artiste, sont décrites dans [`stand`].
//!
//! **D'où elles viennent.** Dessinées par l'artiste du projet
//! [lachydotmcg/capri-desktop](https://github.com/lachydotmcg/capri-desktop) (voir `CREDITS.md`),
//! téléchargées par `tools/fetch-assets.sh` dans `assets/capri/perch/`. Ce module ne contient
//! aucune image ni aucune ligne de leur code : seulement la description de leurs planches.
//!
//! **D'où viennent les nombres.** Tous ont été mesurés sur les images elles-mêmes (canal alpha,
//! colonne par colonne et ligne par ligne), sauf la cadence d'origine, qu'une image ne dit pas :
//! 30 images par seconde pour les boucles, 15 pour l'arrivée et le départ, d'après le code du
//! dépôt d'origine, lu sans rien en reprendre. Ici, on ne garde pas leur cadence : comme partout
//! chez Capri, les dessins sont tenus « en deux » à 24 i/s (voir [`crate::anim`]).
//!
//! **Les planches** sont des bandes horizontales de 384 px de haut, une image à côté de l'autre.
//! Les boucles (assise, clignement, parole, musique) ont des cases de 196 px de large ;
//! l'arrivée et le départ, plus larges à cause de leurs éclats, des cases de 288 px, où le même
//! dessin est décalé de [`WIDE_SHIFT`] px vers la droite (mesuré : la dernière image de l'arrivée
//! et la première du départ recouvrent l'image 0 de l'assise, à ce décalage près).
//!
//! **Le bord de la fenêtre** passe à [`SEAT_Y`] px du haut de chaque case : c'est la ligne d'où
//! l'arrivée naît et où le départ s'efface (tous deux commencent ou finissent en un trait
//! horizontal sur les lignes 264 à 268), et le milieu du dessous de sa cuisse, qui descend de la
//! ligne 256 (fesse) à la ligne 270 (genou). Sa main gauche et ses jambes pendent devant la
//! fenêtre, jusqu'à [`HANG`] px sous le bord ; ses cheveux montent jusqu'à [`HEADROOM`] px
//! au-dessus.
//!
//! Le texte est en pixels des planches (y vers le bas, origine en haut à gauche de la case) ;
//! [`Strip::seat_uv`] le donne en coordonnées normalisées, comme `AlphaMask::feet`.

use crate::anim::Sheet;
use crate::geom::Vec2;

pub mod stand;

/// Le dossier des planches, dans celui des images (`assets/`).
pub const DIR: &str = "capri/perch";
/// Hauteur d'une case, pour toutes les planches.
pub const CELL_HEIGHT: u32 = 384;
/// Largeur d'une case des boucles (assise, clignement, parole, musique).
pub const NARROW: u32 = 196;
/// Largeur d'une case de l'arrivée et du départ.
pub const WIDE: u32 = 288;
/// Dans une case large, le dessin est décalé d'autant vers la droite.
pub const WIDE_SHIFT: u32 = 46;
/// Le bord de la fenêtre, en px depuis le haut d'une case.
pub const SEAT_Y: u32 = 266;
/// Ce qui repose sur le bord, dans une case étroite : de sa main (à gauche) à son genou (à
/// droite), mesuré sur la ligne 266 des 60 images de l'assise (en musique, sa main est sur sa
/// tête : de 49 à 142).
pub const REST: (u32, u32) = (10, 142);
/// Le milieu de [`REST`] : le point de la case qui se pose sur le bord.
pub const SEAT_X: u32 = (REST.0 + REST.1) / 2;
/// Ce qui pend sous le bord : ses pieds descendent jusqu'à la ligne 379 (image 33 de l'assise,
/// la plus basse de toutes les planches), soit 114 px sous [`SEAT_Y`].
pub const HANG: u32 = 380 - SEAT_Y;
/// La place qu'elle prend au-dessus du bord : ses cheveux montent jusqu'à la ligne 29 (musique),
/// soit 237 px au-dessus de [`SEAT_Y`].
pub const HEADROOM: u32 = SEAT_Y - 29;
/// Ce qu'elle occupe en largeur dans une case étroite, de la colonne 7 à la colonne 192 incluse :
/// mesuré sur l'union des quatre boucles (assise, clignement, parole, musique), alpha non nul.
/// L'arrivée et le départ, plus larges, débordent un instant de leurs éclats.
pub const SPAN: (u32, u32) = (7, 193);
/// Les dessins sont tenus en deux : 12 dessins par seconde à 24 i/s.
pub const HOLD: u32 = 2;
/// Dans une boucle de 60 dessins, on en montre un sur deux (30 dessins, 60 images : 2,5 s par
/// tour, au lieu de 2 s à la cadence d'origine).
pub const STEP: u32 = 2;
/// Le clignement remplace, dans la boucle de l'assise, les dessins 28 à 35 (comptés depuis 0).
/// Mesuré : chaque case du clignement est la plus proche de ces dessins-là, dans l'ordre.
pub const BLINK_FROM: u32 = 28;

/// Une planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Strip {
    /// Assise, immobile (elle respire) : 60 dessins.
    Perch,
    /// Les yeux qui se ferment et se rouvrent : 8 dessins, à la place des dessins 28 à 35.
    PerchBlink,
    /// Elle parle : la même boucle que [`Strip::Perch`], dessin pour dessin, la bouche en plus.
    PerchTalk,
    /// Elle se balance sur la musique, les yeux fermés : 60 dessins.
    PerchMusic,
    /// Elle apparaît sur le bord, à partir d'un trait : 14 dessins, une seule fois.
    PerchArrive,
    /// Elle s'efface jusqu'à un trait, puis plus rien : 12 dessins, une seule fois.
    PerchLeave,
}

impl Strip {
    pub const ALL: [Strip; 6] = [
        Strip::Perch,
        Strip::PerchBlink,
        Strip::PerchTalk,
        Strip::PerchMusic,
        Strip::PerchArrive,
        Strip::PerchLeave,
    ];

    /// Le nom du fichier, dans [`DIR`] (le même que dans le dépôt d'origine).
    pub fn file(self) -> &'static str {
        match self {
            Strip::Perch => "perch.png",
            Strip::PerchBlink => "perch_blink.png",
            Strip::PerchTalk => "perch_talk.png",
            Strip::PerchMusic => "perch_music.png",
            Strip::PerchArrive => "perch_arrive.png",
            Strip::PerchLeave => "perch_leave.png",
        }
    }

    /// Le nombre de dessins.
    pub fn count(self) -> u32 {
        match self {
            Strip::Perch | Strip::PerchTalk | Strip::PerchMusic => 60,
            Strip::PerchBlink => 8,
            Strip::PerchArrive => 14,
            Strip::PerchLeave => 12,
        }
    }

    /// Taille d'une case.
    pub fn cell(self) -> (u32, u32) {
        match self {
            Strip::PerchArrive | Strip::PerchLeave => (WIDE, CELL_HEIGHT),
            _ => (NARROW, CELL_HEIGHT),
        }
    }

    /// Taille de la planche telle qu'elle est téléchargée : une bande horizontale.
    pub fn strip_size(self) -> (u32, u32) {
        (self.cell().0 * self.count(), CELL_HEIGHT)
    }

    /// Combien de cases par ligne une fois remise en grille ([`repack`]) : assez peu pour
    /// qu'aucun côté ne dépasse 2 304 px (une bande de 11 760 px dépasse la taille de texture
    /// que bien des cartes graphiques acceptent).
    pub fn columns(self) -> u32 {
        match self {
            Strip::Perch | Strip::PerchTalk | Strip::PerchMusic => 10,
            Strip::PerchBlink => 8,
            Strip::PerchArrive => 7,
            Strip::PerchLeave => 6,
        }
    }

    /// Taille de la grille ([`repack`]).
    pub fn grid_size(self) -> (u32, u32) {
        let (w, h) = self.cell();
        let columns = self.columns();
        (w * columns, h * self.count().div_ceil(columns))
    }

    /// Le coin haut gauche de la case `index` dans la grille, en px.
    pub fn cell_origin(self, index: u32) -> (u32, u32) {
        let (w, h) = self.cell();
        let index = index.min(self.count() - 1);
        (index % self.columns() * w, index / self.columns() * h)
    }

    /// Le point de la case qui se pose sur le bord, en px de la case.
    pub fn seat(self) -> (u32, u32) {
        let shift = if self.cell().0 == WIDE { WIDE_SHIFT } else { 0 };
        (SEAT_X + shift, SEAT_Y)
    }

    /// [`Strip::seat`] en coordonnées normalisées de la case (0..1).
    pub fn seat_uv(self) -> Vec2 {
        let ((x, y), (w, h)) = (self.seat(), self.cell());
        Vec2::new(x as f32 / w as f32, y as f32 / h as f32)
    }
}

/// Un dessin : quelle planche, quelle case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub strip: Strip,
    pub index: u32,
}

/// Une planche lue n'a pas la taille attendue : ce n'est pas le bon fichier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrongSize {
    pub expected: (u32, u32),
    pub found: (u32, u32),
}

/// Remet une bande horizontale (RGBA 8 bits, `strip.strip_size()`) en grille de
/// `strip.columns()` cases par ligne ; les cases manquantes de la dernière ligne restent
/// transparentes.
pub fn repack(strip: Strip, width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, WrongSize> {
    Layout::of(strip).repack(width, height, rgba)
}

/// Les pixels de la case `index` d'une planche remise en grille ([`repack`]), de taille
/// `strip.cell()`, par exemple pour faire le masque de clic d'un dessin. `None` si la grille n'a
/// pas la bonne taille ou si la case n'existe pas.
pub fn cell_pixels(strip: Strip, grid: &[u8], index: u32) -> Option<Vec<u8>> {
    Layout::of(strip).cell_pixels(grid, index)
}

/// Les seuls dessins montrés d'une planche ([`Strip::step`]), serrés dans une grille plus petite :
/// c'est ce qui est confié à la carte graphique. Les boucles et le clignement ne montrent qu'un
/// dessin sur deux (tenus « en deux ») : leur grille fait moitié moins.
pub fn shown(strip: Strip) -> Shown {
    let columns = match strip {
        Strip::Perch | Strip::PerchTalk | Strip::PerchMusic => 10,
        Strip::PerchBlink => 4,
        Strip::PerchArrive => 7,
        Strip::PerchLeave => 6,
    };
    Shown { full: Layout::of(strip), step: strip.step(), columns }
}

impl Strip {
    /// Un dessin sur combien est montré : un sur [`STEP`] pour les boucles et le clignement (qui
    /// remplace les dessins 28 à 35 de la boucle, un sur deux aussi), tous pour l'arrivée et le
    /// départ.
    pub fn step(self) -> u32 {
        match self {
            Strip::PerchArrive | Strip::PerchLeave => 1,
            _ => STEP,
        }
    }
}

/// Une grille de dessins montrés ([`shown`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    full: Layout,
    step: u32,
    columns: u32,
}

impl Shown {
    pub(crate) fn new(full: Layout, step: u32, columns: u32) -> Self {
        Self { full, step, columns }
    }

    /// Combien de dessins elle garde.
    pub fn count(self) -> u32 {
        self.full.count.div_ceil(self.step)
    }

    /// Sa taille, en pixels.
    pub fn grid_size(self) -> (u32, u32) {
        let (cw, ch) = self.full.cell;
        (cw * self.columns, ch * self.count().div_ceil(self.columns))
    }

    /// Le coin haut gauche du dessin n° `index` (de la planche) dans cette grille ; `None` s'il
    /// n'est jamais montré.
    pub fn origin(self, index: u32) -> Option<(u32, u32)> {
        let (cw, ch) = self.full.cell;
        let shown = index < self.full.count && index.is_multiple_of(self.step);
        let slot = index / self.step;
        shown.then_some((slot % self.columns * cw, slot / self.columns * ch))
    }

    /// Cette grille, faite à partir de la grille complète de la planche ([`repack`]) ; `None` si
    /// celle-ci n'a pas la bonne taille.
    pub fn pack(self, grid: &[u8]) -> Option<Vec<u8>> {
        let (fw, fh) = self.full.grid_size();
        if grid.len() != (fw * fh * 4) as usize {
            return None;
        }
        let (cw, ch) = self.full.cell;
        let (gw, gh) = self.grid_size();
        let mut packed = vec![0u8; (gw * gh * 4) as usize];
        let row = (cw * 4) as usize;
        for index in (0..self.full.count).step_by(self.step as usize) {
            let ((fx, fy), (px, py)) = (self.full.cell_origin(index), self.origin(index)?);
            for y in 0..ch {
                let from = (((fy + y) * fw + fx) * 4) as usize;
                let to = (((py + y) * gw + px) * 4) as usize;
                packed[to..to + row].copy_from_slice(&grid[from..from + row]);
            }
        }
        Some(packed)
    }
}

/// Le découpage d'une planche, commun aux poses assise et debout : la taille d'une case, le
/// nombre de dessins, et combien de cases par ligne une fois remise en grille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub cell: (u32, u32),
    pub count: u32,
    pub columns: u32,
}

impl Layout {
    fn of(strip: Strip) -> Self {
        Self { cell: strip.cell(), count: strip.count(), columns: strip.columns() }
    }

    pub fn strip_size(self) -> (u32, u32) {
        (self.cell.0 * self.count, self.cell.1)
    }

    pub fn grid_size(self) -> (u32, u32) {
        (self.cell.0 * self.columns, self.cell.1 * self.count.div_ceil(self.columns))
    }

    pub fn cell_origin(self, index: u32) -> (u32, u32) {
        let index = index.min(self.count - 1);
        (index % self.columns * self.cell.0, index / self.columns * self.cell.1)
    }

    fn repack(self, width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, WrongSize> {
        let expected = self.strip_size();
        if (width, height) != expected || rgba.len() != (width * height * 4) as usize {
            return Err(WrongSize { expected, found: (width, height) });
        }
        let (cw, ch) = self.cell;
        let (gw, gh) = self.grid_size();
        let mut grid = vec![0u8; (gw * gh * 4) as usize];
        let row = (cw * 4) as usize;
        for index in 0..self.count {
            let (gx, gy) = self.cell_origin(index);
            for y in 0..ch {
                let from = ((y * width + index * cw) * 4) as usize;
                let to = (((gy + y) * gw + gx) * 4) as usize;
                grid[to..to + row].copy_from_slice(&rgba[from..from + row]);
            }
        }
        Ok(grid)
    }

    /// Les pixels d'une seule case d'une grille (RGBA 8 bits, ligne par ligne), par exemple pour en
    /// faire un `AlphaMask`. `None` si la grille n'a pas la taille de `grid_size`.
    fn cell_pixels(self, grid: &[u8], index: u32) -> Option<Vec<u8>> {
        let (gw, gh) = self.grid_size();
        if grid.len() != (gw * gh * 4) as usize || index >= self.count {
            return None;
        }
        let (cw, ch) = self.cell;
        let (gx, gy) = self.cell_origin(index);
        let row = (cw * 4) as usize;
        let mut cell = Vec::with_capacity(row * ch as usize);
        for y in 0..ch {
            let from = (((gy + y) * gw + gx) * 4) as usize;
            cell.extend_from_slice(&grid[from..from + row]);
        }
        Some(cell)
    }
}

/// Une boucle de 60 dessins, un sur deux, chacun tenu [`HOLD`] images.
fn looped(strip: Strip, cell: impl Fn(u32) -> Cell) -> Sheet<Cell> {
    let keys: Vec<(Cell, u32)> = (0..strip.count()).step_by(STEP as usize).map(|i| (cell(i), HOLD)).collect();
    Sheet::new(&keys)
}

/// Une suite jouée une fois : chaque dessin tenu [`HOLD`] images. Elle boucle comme toute
/// [`Sheet`] : à l'appelant de s'arrêter après `len()` images.
fn once(strip: Strip) -> Sheet<Cell> {
    let keys: Vec<(Cell, u32)> = (0..strip.count()).map(|index| (Cell { strip, index }, HOLD)).collect();
    Sheet::new(&keys)
}

/// Assise, en boucle. Avec `blink`, elle cligne des yeux au milieu du tour (les dessins 28 à 35
/// viennent du clignement) ; sans, les yeux restent ouverts. À l'appelant de choisir les tours où
/// elle cligne, pour qu'elle ne le fasse pas toutes les 2,5 s.
pub fn sitting(blink: bool) -> Sheet<Cell> {
    looped(Strip::Perch, |index| {
        if blink && (BLINK_FROM..BLINK_FROM + Strip::PerchBlink.count()).contains(&index) {
            Cell { strip: Strip::PerchBlink, index: index - BLINK_FROM }
        } else {
            Cell { strip: Strip::Perch, index }
        }
    })
}

/// Elle parle, assise : la même boucle que [`sitting`], image pour image ; on peut passer de
/// l'une à l'autre à n'importe quelle image sans saut.
pub fn talking() -> Sheet<Cell> {
    looped(Strip::PerchTalk, |index| Cell { strip: Strip::PerchTalk, index })
}

/// Elle se balance sur la musique, en boucle.
pub fn music() -> Sheet<Cell> {
    looped(Strip::PerchMusic, |index| Cell { strip: Strip::PerchMusic, index })
}

/// Elle apparaît sur le bord (une fois, 28 images : 1,17 s).
pub fn arriving() -> Sheet<Cell> {
    once(Strip::PerchArrive)
}

/// Elle s'efface du bord (une fois, 24 images : 1 s ; les deux dernières cases sont vides).
pub fn leaving() -> Sheet<Cell> {
    once(Strip::PerchLeave)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::FRAME_RATE;

    #[test]
    fn strips_have_the_measured_sizes() {
        let sizes: Vec<(u32, u32)> = Strip::ALL.iter().map(|s| s.strip_size()).collect();
        assert_eq!(sizes, [(11760, 384), (1568, 384), (11760, 384), (11760, 384), (4032, 384), (3456, 384)]);
    }

    #[test]
    fn grids_stay_small_and_hold_every_cell() {
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
        }
    }

    #[test]
    fn repack_moves_each_cell_into_its_place() {
        let strip = Strip::PerchArrive;
        let (w, h) = strip.strip_size();
        let (cw, _) = strip.cell();
        // chaque pixel porte le numéro de sa case
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [((i % w) / cw) as u8, 0, 0, 255]).collect();
        let grid = repack(strip, w, h, &rgba).unwrap();
        let (gw, gh) = strip.grid_size();
        assert_eq!(grid.len(), (gw * gh * 4) as usize);
        for index in 0..strip.count() {
            let (x, y) = strip.cell_origin(index);
            for (dx, dy) in [(0, 0), (cw - 1, CELL_HEIGHT - 1), (cw / 2, 100)] {
                let at = (((y + dy) * gw + x + dx) * 4) as usize;
                assert_eq!(grid[at..at + 4], [index as u8, 0, 0, 255], "case {index}");
            }
        }
    }

    #[test]
    fn cell_pixels_gives_back_the_cell_as_it_was_in_the_strip() {
        let strip = Strip::PerchBlink;
        let (w, h) = strip.strip_size();
        let (cw, ch) = strip.cell();
        // chaque pixel porte sa case et sa ligne
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [((i % w) / cw) as u8, (i / w) as u8, 0, 255]).collect();
        let grid = repack(strip, w, h, &rgba).unwrap();
        for index in 0..strip.count() {
            let cell = cell_pixels(strip, &grid, index).unwrap();
            assert_eq!(cell.len(), (cw * ch * 4) as usize);
            for y in [0, 100, ch - 1] {
                let at = ((y * cw + cw / 2) * 4) as usize;
                assert_eq!(cell[at..at + 2], [index as u8, y as u8], "case {index}, ligne {y}");
            }
        }
        assert!(cell_pixels(strip, &grid, strip.count()).is_none(), "une case de trop");
        assert!(cell_pixels(strip, &grid[4..], 0).is_none(), "une grille de la mauvaise taille");
    }

    #[test]
    fn repack_refuses_a_wrong_image() {
        let rgba = vec![0u8; 220 * 420 * 4];
        assert_eq!(
            repack(Strip::Perch, 220, 420, &rgba),
            Err(WrongSize { expected: (11760, 384), found: (220, 420) })
        );
        assert!(repack(Strip::PerchBlink, 1568, 384, &[0; 16]).is_err(), "trop peu d'octets");
    }

    #[test]
    fn the_seat_is_the_same_point_of_her_in_every_strip() {
        assert_eq!(Strip::Perch.seat(), (76, 266));
        assert_eq!(Strip::PerchArrive.seat(), (76 + 46, 266));
        for strip in Strip::ALL {
            let (x, y) = strip.seat();
            let (w, h) = strip.cell();
            assert!(x < w && y < h);
            let uv = strip.seat_uv();
            assert!((uv.x * w as f32 - x as f32).abs() < 1e-3 && (uv.y * h as f32 - y as f32).abs() < 1e-3);
        }
        // ce qui pend et ce qui monte tient dans la case
        assert_eq!((HANG, HEADROOM), (114, 237));
        const { assert!(SEAT_Y + HANG <= CELL_HEIGHT && HEADROOM <= SEAT_Y) };
    }

    #[test]
    fn a_loop_lasts_two_and_a_half_seconds_in_twos() {
        for sheet in [sitting(false), sitting(true), talking(), music()] {
            assert_eq!(sheet.len(), 60);
            assert_eq!(sheet.len() as f64 / FRAME_RATE, 2.5);
            // chaque dessin est tenu deux images
            for frame in (0..60).step_by(2) {
                assert_eq!(sheet.at(frame), sheet.at(frame + 1));
                assert_ne!(sheet.at(frame), sheet.at(frame + 2));
            }
        }
    }

    #[test]
    fn she_blinks_only_where_her_artist_drew_it() {
        let open = sitting(false);
        let blink = sitting(true);
        for frame in 0..60 {
            let (o, b) = (open.at(frame), blink.at(frame));
            // en deux : à l'image `frame`, le dessin pair qui la précède
            let drawing = frame as u32 / 2 * 2;
            assert_eq!(o, Cell { strip: Strip::Perch, index: drawing });
            if (28..36).contains(&drawing) {
                assert_eq!(b, Cell { strip: Strip::PerchBlink, index: drawing - 28 });
            } else {
                assert_eq!(b, o);
            }
        }
    }

    #[test]
    fn talking_keeps_the_same_drawing_number() {
        let (sit, talk) = (sitting(false), talking());
        for frame in 0..120 {
            assert_eq!(sit.at(frame).index, talk.at(frame).index);
            assert_eq!(talk.at(frame).strip, Strip::PerchTalk);
        }
    }

    #[test]
    fn arriving_and_leaving_play_every_drawing_once() {
        let (arrive, leave) = (arriving(), leaving());
        assert_eq!((arrive.len(), leave.len()), (28, 24));
        assert_eq!(arrive.at(0), Cell { strip: Strip::PerchArrive, index: 0 });
        assert_eq!(arrive.at(27), Cell { strip: Strip::PerchArrive, index: 13 });
        assert_eq!(leave.at(23), Cell { strip: Strip::PerchLeave, index: 11 });
    }

    #[test]
    fn only_the_drawings_shown_are_packed_and_every_one_shown_is_there() {
        // chaque dessin que ses feuilles d'exposition montrent est dans la grille serrée
        for sheet in [sitting(true), sitting(false), talking(), music(), arriving(), leaving()] {
            for frame in 0..sheet.len() {
                let cell = sheet.at(frame);
                assert!(shown(cell.strip).origin(cell.index).is_some(), "{cell:?}");
            }
        }
        let mut full = 0;
        let mut packed = 0;
        for strip in Strip::ALL {
            let grid = shown(strip);
            let (gw, gh) = grid.grid_size();
            assert!(gw <= 2304 && gh <= 2304, "{strip:?} : {gw} × {gh}");
            let (cw, ch) = strip.cell();
            let mut seen = std::collections::HashSet::new();
            for index in (0..strip.count()).filter_map(|index| grid.origin(index)) {
                assert!(index.0 + cw <= gw && index.1 + ch <= gh && seen.insert(index), "{strip:?}");
            }
            assert_eq!(seen.len() as u32, grid.count());
            assert_eq!(grid.origin(1).is_some(), strip.step() == 1, "{strip:?}");
            full += u64::from(strip.grid_size().0 * strip.grid_size().1);
            packed += u64::from(gw * gh);
        }
        // un peu plus de la moitié de la place : l'arrivée et le départ gardent tout
        assert!(packed * 100 / full < 60, "{packed} sur {full}");
        assert_eq!(shown(Strip::Perch).grid_size(), (1960, 1152));

        // chaque dessin gardé est à sa place, ses pixels intacts
        let strip = Strip::PerchBlink;
        let (w, h) = strip.strip_size();
        let (cw, _) = strip.cell();
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [((i % w) / cw) as u8, 7, 0, 255]).collect();
        let grid = repack(strip, w, h, &rgba).unwrap();
        let shown = shown(strip);
        let packed = shown.pack(&grid).expect("la bonne taille");
        let (gw, _) = shown.grid_size();
        for index in [0, 2, 4, 6] {
            let (x, y) = shown.origin(index).unwrap();
            let at = (((y + 100) * gw + x + 5) * 4) as usize;
            assert_eq!(packed[at..at + 4], [index as u8, 7, 0, 255], "dessin {index}");
        }
        assert!(shown.pack(&grid[4..]).is_none(), "une grille de la mauvaise taille");
    }

    #[test]
    fn every_cell_exists() {
        for sheet in [sitting(true), talking(), music(), arriving(), leaving()] {
            for frame in 0..sheet.len() {
                let cell = sheet.at(frame);
                assert!(cell.index < cell.strip.count(), "{cell:?}");
            }
        }
    }
}
