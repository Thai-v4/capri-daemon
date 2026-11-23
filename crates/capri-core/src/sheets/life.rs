//! Les planches de sa vie : ce qu'elle fait debout, en dehors de sa boucle. Elle se matérialise
//! ([`Strip::Arrive`]) et elle s'efface ([`Strip::Leave`]), dans un éclat de couleurs décalées, à
//! partir d'un trait horizontal ou jusqu'à lui ; elle marche ([`Strip::Walk`]), elle pend quand on
//! la tient ([`Strip::Dangle`]), elle se relève quand on la lâche ([`Strip::Glitch`]), elle sourit
//! ([`Strip::Grin`]).
//!
//! **D'où elles viennent.** Du même dépôt que ses poses assise et debout (voir `CREDITS.md` et le
//! module parent), téléchargées par `tools/fetch-assets.sh` dans `assets/capri/life/`. Le dépôt
//! d'origine ne leur nomme aucun auteur : sa licence les donne alors à son artiste. Ce module ne
//! contient aucune image ni aucune ligne de leur code : seulement la description de leurs planches.
//! Quand l'arrivée ou le départ manquent, elle apparaît et disparaît d'un coup, comme avant (voir
//! `look`) ; les quatre autres sont facultatives une à une ([`Strip::MOVES`]) : sans l'une, ce
//! qu'elle porte ne se joue pas.
//!
//! **Une famille.** Toutes ses planches de ce dépôt hors de la boucle debout (celles-ci, et aussi,
//! pas encore branchées : le sommeil, le réveil) sont des bandes horizontales de cases de
//! [`CELL_WIDTH`] × 384 px, sur une même toile où, d'après le code du dépôt d'origine, une case de
//! sa boucle debout (116 px de large) se pose à [`STAND_LEFT`] px du bord gauche, à la même
//! hauteur et à la même échelle (vérifié ici sur les six planches branchées : le masque vert de
//! ses yeux est au même endroit, à un pixel près, dans la dernière case de l'arrivée, du glitch et
//! du sourire, et dans la première de la marche ; ses pieds sur la ligne 360 à 363 dans la marche,
//! 363 dans le sourire). Pour en brancher une : une variante de [`Strip`] (son fichier, son nombre
//! de cases, ses colonnes une fois remise en grille, sa tenue), sa feuille d'exposition, et la
//! vérification de cet alignement sur la vraie image (les essais
//! `the_real_life_sheets_match_their_measures_when_present` et
//! `the_real_moves_match_their_measures_when_present` de `capri-app`). Le nombre de cases des
//! autres, d'après la largeur de leur fichier : sommeil 16, réveil 13 (pas vérifié case par case).
//!
//! **D'où viennent les nombres.** Mesurés sur les images (canal alpha, couleurs), sauf la cadence
//! d'origine de chaque planche et le décalage de la boucle debout sur leur toile, lus dans le code
//! du dépôt d'origine sans rien en reprendre. Ici, chaque dessin est tenu un nombre entier d'images
//! à 24 i/s, le plus proche de sa cadence d'origine ([`Strip::hold`]) : « en deux » (12 dessins par
//! seconde) pour l'arrivée et le départ (15 à l'origine), la marche (12, exactement), tenue en l'air
//! (10 : 83 ms par dessin au lieu de 100, plus près que 125 en trois) et le glitch (15) ; « en
//! trois » (8 par seconde, exactement) pour le sourire.
//!
//! **Tenue en l'air** ([`Strip::Dangle`]) : on la tient par une mèche du haut de ses cheveux,
//! [`GRIP`], la seule partie d'elle au même endroit dans ses 24 dessins (mesuré : les pixels pleins
//! communs à toutes ses cases, en haut, forment cette mèche, lignes 28 à 40). Ce point se met sous
//! le curseur ([`grip`]).
//!
//! **La marche** ([`Strip::Walk`]) : 12 dessins, un tour par seconde. **Elle marche sur place** :
//! mesuré, le pied posé ne bouge pas dans la case pendant que l'autre se lève (à un pixel près :
//! colonnes 164 à 166 aux dessins 1 à 5, 127,5 à 129,5 aux dessins 7 à 11), le pied levé retombe
//! là d'où il est parti, et ses deux pieds sont aux mêmes places aux dessins 0 et 6 (qui ne
//! diffèrent que par le dessin de ses jambes). Aucune vitesse n'évite donc que ses pieds
//! glissent : celle choisie ([`walk_speed`]) avance d'un pas par pied posé, un pas étant l'écart de
//! ses deux pieds quand tous deux touchent le sol ([`WALK_STEP`], aux dessins 0 et 6). **Elle
//! regarde vers la gauche** ([`FACING`]) : son visage est de trois quarts, tourné et penché vers la
//! gauche de l'écran. Mesuré dans le masque vert de son visage (colonnes 125 à 143, dessin 0) : son
//! œil gauche, le plus loin, est petit (2 pixels bleus, colonnes 126 et 127) et collé au bord du
//! masque (1 px), le droit plus grand (11 pixels, colonnes 137 à 139) et à 4 px de l'autre bord ;
//! regardé : son menton en bas à gauche, l'arrière de sa tête et ses cheveux vers la droite. Le
//! projet d'origine, lui, joue cette planche telle quelle pour aller vers la droite et la retourne
//! en miroir pour la gauche (lu dans son code). Ici, elle ne marche que vers la gauche : l'image
//! n'est jamais retournée en miroir (un miroir montrerait à l'envers ce qui n'est pas symétrique
//! dans son dessin : son masque, ses mèches, la pose de ses bras).
//!
//! **Deux dessins différents.** Ces planches montrent son ancien dessin debout (de face, le bras
//! gauche tendu, sans jupe, les cheveux en pointes), et sa boucle debout son nouveau (de trois
//! quarts, une main sur la hanche, une jupe). La dernière image de l'arrivée et la première de la
//! boucle ne peuvent donc pas se superposer : on passe de l'une à l'autre par une coupe franche,
//! comme dans le projet d'origine, sans fondu ni interpolation ; de même de la boucle à la
//! première image du départ. Ce qui est aligné, c'est ce qui peut l'être : **ses pieds et sa
//! hauteur**, pour qu'au raccord elle ne change ni de place ni de taille.
//!
//! **L'alignement**, mesuré sur la dernière case de l'arrivée (la même que l'avant-dernière, et
//! que la première du départ aux éclats près) contre les cases de la boucle :
//! - **ses pieds** : le point le plus bas de sa silhouette (dernière ligne pleine, alpha > 96) est
//!   sur la ligne 363 dans les deux ; dans l'arrivée, c'est la pointe de son pied gauche, colonne
//!   126 ([`LOWEST`]) ; dans la boucle, la pointe de son pied, colonnes 52 et 53 (dessins 0 et 58 ;
//!   53 seule aux dessins 24 et 30 : [`STAND_LOWEST`]). Décalée de [`STAND_LEFT`] = 73 px, la
//!   case de la boucle met donc ses pieds sur ceux de l'arrivée, au demi-pixel près ;
//! - **sa hauteur** : la dernière ligne dessinée (alpha > 0) est la 366 dans les deux ; le haut
//!   de ses cheveux est à la ligne 18 dans l'arrivée, 20 dans la boucle (30 quand ses cheveux sont
//!   au plus bas). 349 lignes contre 347 : la même échelle, les 2 px d'écart sont ses pointes de
//!   cheveux, pas sa taille. Rien à mettre à l'échelle, contrairement à sa pose assise
//!   (`stand::perch_scale`) ;
//! - **sa largeur**, elle, diffère : de la colonne 33 à la 208 dans l'arrivée (176 px, son bras
//!   tendu), de la 5 à la 113 dans la boucle (109 px) ;
//! - **ce qui le confirme** : le décalage qu'emploie le dépôt d'origine (lu dans son code : le
//!   milieu d'une case de la boucle 11 px à droite du milieu de celles de ces planches, à la même
//!   échelle ; 120 + 11 − 58 = 73), et le masque vert de ses yeux ([`EYES`], [`STAND_EYES`]),
//!   73,1 px plus à droite dans l'arrivée (et 2,9 px plus bas : sa tête est penchée).

use crate::anim::Sheet;
use crate::geom::{Rect, Vec2};

use super::{CELL_HEIGHT, HOLD, Layout, Shown, WrongSize, stand};

/// Le dossier des planches, dans celui des images (`assets/`).
pub const DIR: &str = "capri/life";
/// Largeur d'une case, pour toute la famille.
pub const CELL_WIDTH: u32 = 240;
/// Où commence une case de sa boucle debout ([`stand::CELL_WIDTH`] de large) dans une case de
/// cette famille, à la même hauteur et à la même échelle : ses pieds sur ses pieds.
pub const STAND_LEFT: u32 = 73;
/// Le haut de ses cheveux à la fin de l'arrivée (alpha non nul).
pub const TOP: u32 = 18;
/// Sous sa silhouette, à la fin de l'arrivée : la dernière ligne dessinée est la 366, comme dans
/// sa boucle debout ([`stand::BOTTOM`]).
pub const BOTTOM: u32 = 367;
/// Le point le plus bas de sa silhouette à la fin de l'arrivée (la dernière ligne où un pixel
/// dépasse alpha 96, et le milieu de ses pixels pleins) : la pointe de son pied gauche, colonne
/// 126 de la ligne 363.
pub const LOWEST: (f32, u32) = (126.5, 363);
/// Le même, dans sa boucle debout : colonnes 52 et 53 de la ligne 363 (dessins 0 et 58).
pub const STAND_LOWEST: (f32, u32) = (53.0, 363);
/// Le centre du masque vert de ses yeux à la fin de l'arrivée, et dans la case 0 de la boucle
/// debout (pixels nettement verts des lignes 90 à 139 : alpha > 200, vert > 150, rouge < 140,
/// bleu < 190) : une confirmation de [`STAND_LEFT`].
pub const EYES: (f32, f32) = (134.1, 123.2);
pub const STAND_EYES: (f32, f32) = (61.0, 120.3);

/// Le point par où on la tient en l'air, dans une case de [`Strip::Dangle`] : le centre des 119
/// pixels pleins (alpha > 96) communs à ses 24 dessins dans les lignes 28 à 40, une mèche du haut de
/// ses cheveux (colonnes 137 à 153 en haut, 143 à 146 en bas).
pub const GRIP: (f32, f32) = (144.9, 33.3);
/// Un pas de sa marche, en px de toile : l'écart de ses deux pieds quand tous deux touchent le sol
/// (dessins 0 et 6 de la marche : la pointe du pied gauche colonne 126 de la ligne 363, celle du
/// pied droit colonnes 167 et 168 de la ligne 362 ; 41,5 px entre leurs milieux, 126,5 et 168).
pub const WALK_STEP: f32 = 41.5;
/// Deux pas par tour de sa marche : un pied se lève aux dessins 1 à 5, l'autre aux dessins 7 à 11.
pub const WALK_STEPS: u32 = 2;
/// De quel côté elle regarde dans sa marche (voir le module).
pub const FACING: Facing = Facing::Left;
/// Le sourire est tenu en trois (8 dessins par seconde, sa cadence d'origine) et joué deux tours
/// (comme dans le projet d'origine), soit 96 images : 4 s.
pub const GRIN_HOLD: u32 = 3;
pub const GRIN_TOURS: u64 = 2;

/// Un côté de l'écran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    /// Un déplacement de `dx` (en x, vers la droite si positif) va-t-il dans ce sens ?
    pub fn allows(self, dx: f32) -> bool {
        match self {
            Facing::Left => dx < 0.0,
            Facing::Right => dx > 0.0,
        }
    }
}

/// Une planche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Strip {
    /// Elle se matérialise : un trait, des éclats, puis elle, le bras tendu. 14 dessins, les deux
    /// derniers identiques.
    Arrive,
    /// Elle s'efface : la même, en éclats, jusqu'à un trait, puis plus rien. 12 dessins, le dernier
    /// vide.
    Leave,
    /// Elle marche (sur place, voir le module) : 12 dessins, en boucle.
    Walk,
    /// Tenue en l'air par une mèche, elle pend, bras écartés, et agite les jambes : 24 dessins, en
    /// boucle (sept sont les mêmes que d'autres : 0 et 12, 4 et 16, 5 et 17, 6 et 18, 8 et 20, 9
    /// et 21, 11 et 23).
    Dangle,
    /// Lâchée, elle se relève dans un glitch : 10 dessins, une fois ; le dernier est la fin de son
    /// arrivée, à quelques pixels près.
    Glitch,
    /// Elle sourit de toutes ses dents : 16 dessins, sur la fin de son arrivée.
    Grin,
}

impl Strip {
    pub const ALL: [Strip; 6] =
        [Strip::Arrive, Strip::Leave, Strip::Walk, Strip::Dangle, Strip::Glitch, Strip::Grin];
    /// Apparaître et s'effacer : les deux ensemble, ou ni l'une ni l'autre.
    pub const APPEAR: [Strip; 2] = [Strip::Arrive, Strip::Leave];
    /// Ses gestes : chacune à part, facultative.
    pub const MOVES: [Strip; 4] = [Strip::Walk, Strip::Dangle, Strip::Glitch, Strip::Grin];

    /// Le nom du fichier, dans [`DIR`] (le même que dans le dépôt d'origine).
    pub fn file(self) -> &'static str {
        match self {
            Strip::Arrive => "arrive.png",
            Strip::Leave => "leave.png",
            Strip::Walk => "walk.png",
            Strip::Dangle => "dangle.png",
            Strip::Glitch => "glitch.png",
            Strip::Grin => "grin.png",
        }
    }

    /// Le nombre de dessins.
    pub fn count(self) -> u32 {
        match self {
            Strip::Arrive => 14,
            Strip::Leave | Strip::Walk => 12,
            Strip::Dangle => 24,
            Strip::Glitch => 10,
            Strip::Grin => 16,
        }
    }

    /// Combien d'images chaque dessin est tenu, à 24 i/s : le plus près de sa cadence d'origine
    /// (voir le module).
    pub fn hold(self) -> u32 {
        match self {
            Strip::Grin => GRIN_HOLD,
            _ => HOLD,
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

    /// Combien de cases par ligne une fois remise en grille ([`repack`]) : des lignes pleines,
    /// aucun côté au-delà de 2 304 px.
    pub fn columns(self) -> u32 {
        match self {
            Strip::Arrive => 7,
            Strip::Leave | Strip::Walk => 6,
            Strip::Dangle | Strip::Grin => 8,
            Strip::Glitch => 5,
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

    /// Un dessin sur combien est montré : tous (jouées une fois, chaque dessin compte).
    pub fn step(self) -> u32 {
        1
    }
}

/// Les seuls dessins montrés d'une planche, serrés dans une grille (voir [`super::shown`]) :
/// ici tous, la grille même de [`repack`].
pub fn shown(strip: Strip) -> Shown {
    Shown::new(strip.layout(), strip.step(), strip.columns())
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

/// Tous les dessins d'une planche, chacun tenu [`Strip::hold`] images. Elle boucle comme toute
/// [`Sheet`] : pour une suite jouée une fois, à l'appelant de s'arrêter après `len()` images.
fn once(strip: Strip) -> Sheet<Cell> {
    let keys: Vec<(Cell, u32)> =
        (0..strip.count()).map(|index| (Cell { strip, index }, strip.hold())).collect();
    Sheet::new(&keys)
}

/// Elle se matérialise (une fois, 28 images : 1,17 s).
pub fn arriving() -> Sheet<Cell> {
    once(Strip::Arrive)
}

/// Elle s'efface (une fois, 24 images : 1 s ; la dernière case est vide).
pub fn leaving() -> Sheet<Cell> {
    once(Strip::Leave)
}

/// Elle marche, en boucle (24 images : un tour par seconde, deux pas).
pub fn walking() -> Sheet<Cell> {
    once(Strip::Walk)
}

/// Tenue en l'air, elle pend, en boucle (48 images : 2 s).
pub fn dangling() -> Sheet<Cell> {
    once(Strip::Dangle)
}

/// Lâchée, elle se relève (une fois, 20 images : 0,83 s).
pub fn rising() -> Sheet<Cell> {
    once(Strip::Glitch)
}

/// Elle sourit : un tour de 48 images (2 s) ; à jouer [`GRIN_TOURS`] fois ([`grin_frames`]).
pub fn grinning() -> Sheet<Cell> {
    once(Strip::Grin)
}

/// Combien d'images dure son sourire entier : [`GRIN_TOURS`] tours.
pub fn grin_frames() -> u64 {
    u64::from(Strip::Grin.count() * GRIN_HOLD) * GRIN_TOURS
}

/// Sa vitesse de marche, en pixels d'écran par seconde, pour une hauteur debout de `height`
/// pixels : [`WALK_STEPS`] pas de [`WALK_STEP`] px de toile par tour de sa marche, à la même
/// échelle que sa pose debout. 83 px de toile par seconde, soit 0,24 de sa hauteur par seconde
/// (le projet d'origine, lu dans son code : 0,42, ses pieds glissant d'autant).
pub fn walk_speed(height: f32) -> f32 {
    let tour = (Strip::Walk.count() * Strip::Walk.hold()) as f32 / crate::anim::FRAME_RATE as f32;
    WALK_STEP * WALK_STEPS as f32 * stand::scale(height) / tour
}

/// Où est le point par où on la tient ([`GRIP`]), depuis le coin haut gauche de son image debout
/// de taille `standing` (une case de sa boucle, sans écrasement ni étirement), par le même
/// alignement que [`over_stand`] : là où mettre le curseur.
pub fn grip(standing: Vec2) -> Vec2 {
    let kx = standing.x / stand::CELL_WIDTH as f32;
    let ky = standing.y / CELL_HEIGHT as f32;
    Vec2::new((GRIP.0 - STAND_LEFT as f32) * kx, GRIP.1 * ky)
}

/// Où dessiner une case de cette famille à la place de son image debout dessinée en `standing`
/// (une case de sa boucle, telle que `body.rs` la place, flottement, écrasement et étirement
/// compris) : ses pieds sur ses pieds, à la même échelle, la même hauteur. En pixels entiers.
pub fn over_stand(standing: Rect) -> Rect {
    let kx = standing.size.x / stand::CELL_WIDTH as f32;
    let ky = standing.size.y / CELL_HEIGHT as f32;
    Rect::new(
        Vec2::new((standing.min.x - STAND_LEFT as f32 * kx).round(), standing.min.y.round()),
        Vec2::new((CELL_WIDTH as f32 * kx).round(), (CELL_HEIGHT as f32 * ky).round()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::FRAME_RATE;

    #[test]
    fn strips_have_the_measured_sizes() {
        let sizes: Vec<(u32, u32)> = Strip::ALL.iter().map(|s| s.strip_size()).collect();
        assert_eq!(sizes, [(3360, 384), (2880, 384), (2880, 384), (5760, 384), (2400, 384), (3840, 384)]);
        assert!(Strip::APPEAR.iter().all(|strip| !Strip::MOVES.contains(strip)));
        assert_eq!(Strip::APPEAR.len() + Strip::MOVES.len(), Strip::ALL.len());
    }

    #[test]
    fn grids_stay_small_and_are_full() {
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
            assert_eq!(gw / cw * (gh / ch), strip.count(), "{strip:?} : aucune case vide");
            // tous les dessins montrés : la grille confiée à la carte graphique est la même
            assert_eq!(shown(strip).grid_size(), (gw, gh));
            assert_eq!(shown(strip).count(), strip.count());
        }
    }

    #[test]
    fn repack_and_cell_pixels_keep_each_cell_whole() {
        let strip = Strip::Arrive;
        let (w, h) = strip.strip_size();
        let rgba: Vec<u8> =
            (0..w * h).flat_map(|i| [((i % w) / CELL_WIDTH) as u8, (i / w % 256) as u8, 0, 255]).collect();
        let grid = repack(strip, w, h, &rgba).unwrap();
        for index in [0, 6, 7, 13] {
            let cell = cell_pixels(strip, &grid, index).unwrap();
            for y in [0, 255, CELL_HEIGHT - 1] {
                let at = ((y * CELL_WIDTH + CELL_WIDTH - 1) * 4) as usize;
                assert_eq!(cell[at..at + 2], [index as u8, (y % 256) as u8], "case {index}, ligne {y}");
            }
        }
        let packed = shown(strip).pack(&grid).expect("la bonne taille");
        assert_eq!(packed, grid, "tous montrés : la grille telle quelle");
        assert_eq!(
            repack(Strip::Leave, 220, 420, &vec![0; 220 * 420 * 4]),
            Err(WrongSize { expected: (2880, 384), found: (220, 420) })
        );
        assert!(cell_pixels(strip, &grid, 14).is_none(), "une case de trop");
    }

    #[test]
    fn each_plays_once_in_twos() {
        let (arrive, leave) = (arriving(), leaving());
        assert_eq!((arrive.len(), leave.len()), (28, 24));
        assert_eq!(arrive.len() as f64 / FRAME_RATE, 28.0 / 24.0);
        for sheet in [&arrive, &leave] {
            for frame in (0..sheet.len()).step_by(2) {
                assert_eq!(sheet.at(frame), sheet.at(frame + 1), "tenu deux images");
                assert_eq!(sheet.at(frame).index as u64, frame / 2);
            }
        }
        assert_eq!(arrive.at(27), Cell { strip: Strip::Arrive, index: 13 });
        assert_eq!(leave.at(23), Cell { strip: Strip::Leave, index: 11 });
    }

    #[test]
    fn each_move_is_held_closest_to_its_own_pace() {
        // la marche : 12 dessins par seconde, comme à l'origine ; un tour par seconde
        let walk = walking();
        assert_eq!(walk.len(), 24);
        // tenue en l'air : 10 par seconde à l'origine ; en deux (83 ms) plutôt qu'en trois (125 ms)
        assert_eq!(dangling().len(), 48);
        // le glitch : 15 par seconde à l'origine, en deux comme son arrivée
        let rise = rising();
        assert_eq!(rise.len(), 20);
        assert_eq!(rise.at(19), Cell { strip: Strip::Glitch, index: 9 });
        // le sourire : 8 par seconde, exactement, en trois ; deux tours, 4 s
        let grin = grinning();
        assert_eq!(grin.len(), 48);
        assert_eq!(grin_frames(), 96);
        assert_eq!(grin_frames() as f64 / FRAME_RATE, 4.0);
        for frame in (0..48).step_by(3) {
            assert_eq!(grin.at(frame), grin.at(frame + 2), "tenu trois images");
            assert_ne!(grin.at(frame), grin.at(frame + 3));
        }
        for sheet in [&walk, &rise] {
            for frame in (0..sheet.len()).step_by(2) {
                assert_eq!(sheet.at(frame), sheet.at(frame + 1), "tenu deux images");
            }
        }
        // chaque dessin de chaque suite existe, et est confié à la carte graphique
        for sheet in [walking(), dangling(), rising(), grinning()] {
            for frame in 0..sheet.len() {
                let cell = sheet.at(frame);
                assert!(cell.index < cell.strip.count() && shown(cell.strip).origin(cell.index).is_some());
            }
        }
    }

    #[test]
    fn she_walks_one_step_per_foot_set_down_at_the_scale_of_her_standing_pose() {
        // 300 points sur un écran Retina : 600 px debout
        let speed = walk_speed(600.0);
        let per_tour = WALK_STEP * 2.0 * stand::scale(600.0);
        assert!((speed - per_tour).abs() < 1e-3, "un tour par seconde : {speed}");
        // 0,24 de sa hauteur par seconde, quelle que soit sa taille
        for height in [240.0, 600.0, 840.0] {
            assert!((walk_speed(height) / height - 83.0 / 347.0).abs() < 1e-4);
        }
        assert!(FACING.allows(-1.0) && !FACING.allows(1.0) && !FACING.allows(0.0));
        assert!(Facing::Right.allows(1.0) && !Facing::Right.allows(-1.0));
    }

    #[test]
    fn she_is_held_by_her_grip_over_her_standing_image() {
        // une case debout de 600 px de haut, à (500, 300)
        let k = 600.0 / 384.0;
        let standing = Rect::new(Vec2::new(500.0, 300.0), Vec2::new((116.0 * k as f32).round(), 600.0));
        let canvas = over_stand(standing);
        let from_canvas =
            canvas.min + Vec2::new(GRIP.0 * canvas.size.x / 240.0, GRIP.1 * canvas.size.y / 384.0);
        let from_standing = standing.min + grip(standing.size);
        assert!((from_canvas - from_standing).length() <= 1.0, "{from_canvas:?} contre {from_standing:?}");
        // la mèche est dans la case de sa boucle debout, en haut
        const { assert!(GRIP.0 as u32 > STAND_LEFT && (GRIP.0 as u32) < STAND_LEFT + stand::CELL_WIDTH) };
        assert!(GRIP.1 < stand::TOP as f32 + 20.0);
    }

    #[test]
    fn her_standing_cell_sits_in_the_middle_of_the_canvas_at_the_same_scale() {
        // le milieu d'une case de la boucle, 11 px à droite du milieu de la toile
        const {
            assert!(STAND_LEFT + stand::CELL_WIDTH / 2 == CELL_WIDTH / 2 + 11);
            assert!(STAND_LEFT + stand::CELL_WIDTH <= CELL_WIDTH);
        };
        // la même ligne de pied, la même hauteur à 2 px près
        const { assert!(BOTTOM == stand::BOTTOM && stand::TOP - TOP <= 2) };
        // ses pieds sur ses pieds : le point le plus bas, sur la même ligne, au demi-pixel près
        assert_eq!(LOWEST.1, STAND_LOWEST.1);
        assert!((LOWEST.0 - STAND_LOWEST.0 - STAND_LEFT as f32).abs() <= 0.5);
        // ses yeux le confirment, au pixel près ; un peu plus bas (sa tête penchée)
        let (dx, dy) = (EYES.0 - STAND_EYES.0, EYES.1 - STAND_EYES.1);
        assert!((dx - STAND_LEFT as f32).abs() < 1.0 && (0.0..4.0).contains(&dy), "{dx} {dy}");
    }

    #[test]
    fn over_her_standing_image_the_canvas_lines_up_pixel_for_pixel() {
        // une case debout de 600 px de haut, à (500, 300) : 1,5625 px d'écran par px de planche
        let k = 600.0 / 384.0;
        let standing = Rect::new(Vec2::new(500.0, 300.0), Vec2::new((116.0 * k as f32).round(), 600.0));
        let life = over_stand(standing);
        assert_eq!(life.min.y, standing.min.y);
        assert_eq!(life.size.y, standing.size.y, "la même échelle");
        // le pixel de toile (73 + x, y) tombe sur le pixel (x, y) de la case debout, à un pixel près
        for (x, y) in [(0.0, 0.0), (52.5, 364.0), (115.0, 383.0)] {
            let on_life =
                life.min + Vec2::new((STAND_LEFT as f32 + x) * life.size.x / 240.0, y * life.size.y / 384.0);
            let on_stand = standing.min + Vec2::new(x * standing.size.x / 116.0, y * standing.size.y / 384.0);
            assert!((on_life - on_stand).length() <= 1.0, "({x}, {y}) : {on_life:?} contre {on_stand:?}");
        }
        // écrasée ou étirée comme son image debout
        let squashed = Rect::new(standing.min, Vec2::new(standing.size.x * 1.14, standing.size.y * 0.84));
        let life = over_stand(squashed);
        assert!((life.size.x / life.size.y - 240.0 * 1.14 / (384.0 * 0.84)).abs() < 0.01);
    }
}
