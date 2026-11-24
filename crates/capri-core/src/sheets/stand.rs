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
//! (60 dessins), le clignement (8) et la bouche (5), toutes trois nécessaires ([`Strip::SET`]) ; et,
//! facultative ([`Strip::EXTRA`]), une variante glitchée du clignement (8), aux mêmes dessins de la
//! boucle (sans elle, ses clignements d'avant). Le clignement et la bouche se posent sur la
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
//! **Elle guette** (de derrière le côté d'une fenêtre, voir `crate::perch` et `crate::look`) avec
//! ces mêmes dessins, jamais retournés ni modifiés : penchée de [`LEAN_DEGREES`] autour de sa
//! hanche ([`HIP`]) vers l'extérieur de la fenêtre ([`lean`]), et coupée net à la verticale du bord,
//! [`EYE_MARGIN`] pixels en deçà de ses yeux penchés, du côté de la fenêtre ([`cut`]). L'angle et
//! la coupe sont la variante n° 2 de la maquette (la n° 4 de l'autre côté), choisie telle quelle par
//! l'utilisateur le 24 novembre 2026 ; ces deux constantes la font toute (14° et 23 px redonnent, à
//! un pixel près, la variante n° 1, où elle se montre davantage).
//!
//! **Ce qui se voit d'elle**, mesuré sur ses vrais dessins (les centres de leurs pixels, vérifié par
//! l'essai des vraies planches de l'app) : sur le côté **droit** d'une fenêtre, sa tête et ses
//! cheveux, et, le long de la coupe, un mince bord de son corps jusqu'à sa taille (30 px au-dessus
//! de sa hanche) ; sur le côté **gauche**, sa tête, ses cheveux, et aussi son épaule, son haut, son
//! bras et un bout de sa jupe, le long de la coupe jusqu'à 6 px au-dessus de sa hanche. Le reste est
//! caché derrière la fenêtre.
//!
//! La hanche, sa silhouette bande par bande ([`OUTLINE`]) et ses yeux ([`EYES`]) sont mesurés sur
//! les planches. Ce qui se voit d'elle ([`lurk_bounds`]) se calcule de sa silhouette, un peu plus
//! large que le vrai : de quoi savoir la place qu'il lui faut, et de combien glisser pour être
//! cachée tout entière derrière la fenêtre. Ses yeux (le vert autour, entier) restent du côté qui se
//! voit, à [`EYE_MARGIN`] de la coupe au moins, des deux côtés, dans chaque dessin de sa boucle et
//! de son clignement ordinaire ; sa bouche aussi (vérifié plus bas). **Son clignement glitché, non** :
//! il décale de côté des bandes de ses yeux, dont la plus proche de la fenêtre passe jusqu'à 3,2 px
//! derrière la coupe à gauche, et en reste à 0,3 px à droite ([`GLITCHY_EYE_MARGIN`], mesuré sur la
//! vraie planche) ; ce clignement-là se joue quand même sur un côté.
//!
//! Le texte est en pixels des planches (y vers le bas, origine en haut à gauche de la case).

use crate::anim::Sheet;
use crate::geom::{Rect, Vec2};
use crate::perch::Side;

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
/// yeux fermés, 0 et 7 ouverts. De même chaque case de sa variante glitchée
/// ([`Strip::StandBlinkGlitch`]).
pub const BLINK_FROM: u32 = 24;
/// Où est la bouche dans une case : de (58, 117) à (74, 131) inclus, dans les cinq dessins ;
/// rien n'est dessiné ailleurs dans sa planche.
pub const MOUTH_BOX: ((u32, u32), (u32, u32)) = ((58, 117), (74, 131));
/// Ses yeux : le masque vert autour d'eux, de (54, 109) à (77, 120) inclus, dans chaque dessin de
/// sa boucle et de son clignement (mesuré : le vert, rouge sous 110, vert au-dessus de 150, bleu
/// sous 160, opaque, au-dessus de son cou). Sa variante glitchée décale de côté des bandes de ces
/// lignes-là, ses yeux compris (de la colonne 43 à la 93) : voir [`GLITCHY_EYE_MARGIN`].
pub const EYES: ((u32, u32), (u32, u32)) = ((54, 109), (77, 120));
/// Guettant, dans son clignement glitché ([`Strip::StandBlinkGlitch`]), la marge la plus faible de
/// ses yeux à la coupe, sur le côté gauche d'une fenêtre puis sur le droit ([`Side::index`]) :
/// le vert des lignes de [`EYES`], coin de pixel le plus près de la fenêtre, penché. Négative, une
/// bande passe derrière la fenêtre : de 3,2 px à gauche (son dessin 6) ; à droite, elle s'approche
/// à 0,3 px de la coupe (son dessin 2). Mesuré sur la vraie planche le 24 novembre 2026 ; l'essai
/// des vraies planches de l'app le refait, pour qu'une planche qui change se voie.
pub const GLITCHY_EYE_MARGIN: [f32; 2] = [-3.21, 0.30];
/// Sa hanche, autour de laquelle elle se penche quand elle guette : le milieu de sa silhouette (alpha
/// au-dessus de 96, comme le masque de clic) à la ligne 240, juste sous sa jupe (de la colonne 55 ou
/// 56 à la 79 : son milieu en 67 ou 67,5 selon le dessin), arrondi au pixel comme dans la maquette.
pub const HIP: (f32, f32) = (68.0, 240.0);
/// Sa silhouette bande par bande : pour chaque bande de [`BAND`] lignes à partir de [`TOP`] (la
/// dernière s'arrête à [`BOTTOM`]), la première et la dernière colonne où quelque chose est dessiné
/// (alpha non nul), dans tous ses dessins debout, clignements, variante glitchée et bouche compris.
pub const OUTLINE: [(u32, u32); 15] = [
    (20, 87),
    (7, 103),
    (4, 111),
    (4, 113),
    (10, 107),
    (10, 82),
    (28, 75),
    (41, 79),
    (43, 81),
    (54, 82),
    (53, 77),
    (48, 73),
    (43, 67),
    (43, 61),
    (48, 57),
];
/// La hauteur d'une bande de [`OUTLINE`], en lignes.
pub const BAND: u32 = 24;
/// Guettant, elle est penchée de tant de degrés vers l'extérieur de la fenêtre, autour de sa hanche
/// (la maquette, variante n° 2).
pub const LEAN_DEGREES: f32 = 22.0;
/// Guettant, la coupe passe à tant de pixels de ses yeux penchés ([`EYES`], le vert autour, entier),
/// du côté de la fenêtre (la maquette, variante n° 2).
pub const EYE_MARGIN: f32 = 9.0;
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
    /// Une variante de son clignement, l'œil brouillé : 8 dessins, à la place des dessins 24 à 31.
    /// Facultative.
    StandBlinkGlitch,
}

impl Strip {
    pub const ALL: [Strip; 4] = [Strip::Stand, Strip::StandBlink, Strip::StandMouth, Strip::StandBlinkGlitch];
    /// Sa pose debout : toutes ensemble, ou son dessin d'avant.
    pub const SET: [Strip; 3] = [Strip::Stand, Strip::StandBlink, Strip::StandMouth];
    /// Facultative (et seulement avec [`Strip::SET`]).
    pub const EXTRA: [Strip; 1] = [Strip::StandBlinkGlitch];

    /// Le nom du fichier, dans [`DIR`] (le même que dans le dépôt d'origine).
    pub fn file(self) -> &'static str {
        match self {
            Strip::Stand => "idle.png",
            Strip::StandBlink => "idle_blink.png",
            Strip::StandMouth => "idle_mouth.png",
            Strip::StandBlinkGlitch => "idle_blink_glitch.png",
        }
    }

    /// Le nombre de dessins.
    pub fn count(self) -> u32 {
        match self {
            Strip::Stand => 60,
            Strip::StandBlink | Strip::StandBlinkGlitch => 8,
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
            Strip::StandBlink | Strip::StandBlinkGlitch => 8,
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

    /// Un dessin sur combien est montré : un sur [`STEP`] pour la boucle et les clignements (qui
    /// remplacent les dessins 24 à 31, un sur deux aussi), toutes les formes de la bouche.
    pub fn step(self) -> u32 {
        match self {
            Strip::Stand | Strip::StandBlink | Strip::StandBlinkGlitch => STEP,
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
        Strip::StandBlink | Strip::StandBlinkGlitch => 4,
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
    looped(blink.then_some(Strip::StandBlink))
}

/// Debout, en boucle, et elle cligne avec la variante glitchée de son clignement
/// ([`Strip::StandBlinkGlitch`]), aux mêmes dessins.
pub fn standing_glitchy() -> Sheet<Cell> {
    looped(Some(Strip::StandBlinkGlitch))
}

/// Sa boucle debout, les dessins 24 à 31 pris dans `blink` s'il y en a un.
fn looped(blink: Option<Strip>) -> Sheet<Cell> {
    let keys: Vec<(Cell, u32)> = (0..Strip::Stand.count())
        .step_by(STEP as usize)
        .map(|index| {
            let cell = match blink {
                Some(strip) if (BLINK_FROM..BLINK_FROM + strip.count()).contains(&index) => {
                    Cell { strip, index: index - BLINK_FROM }
                }
                _ => Cell { strip: Strip::Stand, index },
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

/// Guettant sur le côté `side` d'une fenêtre : de combien elle est penchée, en radians, à l'écran
/// (y vers le bas : positif dans le sens des aiguilles d'une montre). Toujours vers l'extérieur de
/// la fenêtre : sa tête vers la gauche sur le côté gauche, vers la droite sur le côté droit.
pub fn lean(side: Side) -> f32 {
    // la tête vers la droite, c'est le sens des aiguilles d'une montre
    side.outward() * LEAN_DEGREES.to_radians()
}

/// Guettant sur le côté `side` d'une fenêtre : où passe la coupe dans sa case penchée, en x (pixels
/// de ses planches) : à [`EYE_MARGIN`] de ses yeux penchés ([`EYES`]), du côté de la fenêtre.
pub fn cut(side: Side) -> f32 {
    let (hip, angle, o) = (Vec2::new(HIP.0, HIP.1), lean(side), side.outward());
    let ((x0, y0), (x1, y1)) = EYES;
    let (x0, y0, x1, y1) = (x0 as f32, y0 as f32, (x1 + 1) as f32, (y1 + 1) as f32);
    // le bord de ses yeux penchés le plus près de la fenêtre (vers l'intérieur : -o)
    let nearest = [Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
        .map(|corner| turn(corner, hip, angle).x * o)
        .into_iter()
        .fold(f32::MAX, f32::min);
    (nearest - EYE_MARGIN) * o
}

/// Guettant sur le côté `side` d'une fenêtre : de combien de pixels de ses planches sa hanche est
/// dehors, de l'autre côté de la coupe que la fenêtre ; négatif, elle est derrière la fenêtre (avec
/// [`LEAN_DEGREES`] et [`EYE_MARGIN`] d'aujourd'hui, sa hanche est derrière, des deux côtés).
pub fn hip_out(side: Side) -> f32 {
    (HIP.0 - cut(side)) * side.outward()
}

/// Le point `p` tourné de `angle` radians autour de `pivot` (à l'écran, y vers le bas : positif dans
/// le sens des aiguilles d'une montre).
pub fn turn(p: Vec2, pivot: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    let d = p - pivot;
    pivot + Vec2::new(d.x * cos - d.y * sin, d.x * sin + d.y * cos)
}

/// Guettant sur le côté `side` d'une fenêtre, ce qui se voit d'elle, en pixels de ses planches,
/// autour de son point d'accroche (sur la coupe, à la hauteur de sa hanche) : sa silhouette bande
/// par bande ([`OUTLINE`]) penchée ([`lean`]), et ce qui en reste du côté dehors de la coupe. Un peu
/// plus que ce qui se voit vraiment (chaque bande compte pour un rectangle), jamais moins. Rien
/// au-delà de la coupe : `max.x` vaut 0 à gauche, `min.x` à droite.
pub fn lurk_bounds(side: Side) -> Rect {
    let hip = Vec2::new(HIP.0, HIP.1);
    let angle = lean(side);
    let o = side.outward();
    let cut = cut(side);
    let shows = |p: Vec2| (p.x - cut) * o >= 0.0;
    let (mut lo, mut hi) = (Vec2::new(f32::MAX, f32::MAX), Vec2::new(f32::MIN, f32::MIN));
    for (band, &(x0, x1)) in OUTLINE.iter().enumerate() {
        let y0 = (TOP + band as u32 * BAND) as f32;
        let y1 = ((TOP + (band as u32 + 1) * BAND).min(BOTTOM)) as f32;
        // les bords des pixels : de la colonne `x0` au bout de la colonne `x1`
        let (x0, x1) = (x0 as f32, (x1 + 1) as f32);
        let corners = [Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
            .map(|corner| turn(corner, hip, angle));
        // la bande penchée, privée de ce qui est derrière la fenêtre (un côté du polygone à la fois)
        for (index, &a) in corners.iter().enumerate() {
            let b = corners[(index + 1) % corners.len()];
            let mut keep = |p: Vec2| {
                lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
            };
            if shows(a) {
                keep(a);
            }
            if shows(a) != shows(b) {
                let t = (cut - a.x) / (b.x - a.x);
                keep(Vec2::new(cut, a.y + t * (b.y - a.y)));
            }
        }
    }
    let anchor = Vec2::new(cut, hip.y);
    Rect::new(lo - anchor, hi - lo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::FRAME_RATE;

    #[test]
    fn strips_have_the_measured_sizes() {
        let sizes: Vec<(u32, u32)> = Strip::ALL.iter().map(|s| s.strip_size()).collect();
        assert_eq!(sizes, [(6960, 384), (928, 384), (580, 384), (928, 384)]);
        assert!(Strip::SET.iter().all(|strip| !Strip::EXTRA.contains(strip)));
        assert_eq!(Strip::SET.len() + Strip::EXTRA.len(), Strip::ALL.len());
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
        // sa variante glitchée : les mêmes dessins, de l'autre planche
        let glitchy = standing_glitchy();
        for frame in 0..60 {
            let (b, g) = (blink.at(frame), glitchy.at(frame));
            assert_eq!(b.index, g.index, "image {frame}");
            let expected = if b.strip == Strip::StandBlink { Strip::StandBlinkGlitch } else { b.strip };
            assert_eq!(g.strip, expected, "image {frame}");
        }
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
        for sheet in [standing(true), standing(false), standing_glitchy(), talking()] {
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
        for sheet in [standing(true), standing(false), standing_glitchy(), talking()] {
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

    /// Les coins d'une boîte de pixels (inclusive), aux bords des pixels.
    fn corners(((x0, y0), (x1, y1)): ((u32, u32), (u32, u32))) -> [Vec2; 4] {
        let (x0, y0, x1, y1) = (x0 as f32, y0 as f32, (x1 + 1) as f32, (y1 + 1) as f32);
        [Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    #[test]
    fn her_outline_covers_her_whole_silhouette_band_by_band() {
        // les bandes vont de ses cheveux à la pointe de son pied, sans trou ni de trop
        let bands = OUTLINE.len() as u32;
        assert!(TOP + (bands - 1) * BAND < BOTTOM && TOP + bands * BAND >= BOTTOM);
        assert!(OUTLINE.iter().all(|&(x0, x1)| x0 <= x1 && x1 < CELL_WIDTH));
        // sa hanche, ses yeux et sa bouche y sont
        let band = |y: u32| OUTLINE[((y - TOP) / BAND) as usize];
        let (x0, x1) = band(HIP.1 as u32);
        assert!(x0 as f32 <= HIP.0 && HIP.0 <= x1 as f32);
        for ((x0, y0), (x1, y1)) in [EYES, MOUTH_BOX] {
            for y in y0..=y1 {
                let (lo, hi) = band(y);
                assert!(lo <= x0 && x1 <= hi, "ligne {y}");
            }
        }
    }

    #[test]
    fn lurking_she_leans_out_of_the_window_and_only_her_head_shows_eyes_and_mouth_whole() {
        let hip = Vec2::new(HIP.0, HIP.1);
        let above = Vec2::new(HIP.0, TOP as f32);
        for side in Side::ALL {
            // sa tête vers l'extérieur de la fenêtre, de 22 degrés
            let head = turn(above, hip, lean(side));
            assert!((head.x - hip.x) * side.outward() > 0.0, "{side:?} : {head:?}");
            let tilt = ((head.x - hip.x).abs() / (hip.y - head.y)).atan().to_degrees();
            assert!((tilt - LEAN_DEGREES).abs() < 1e-3, "{side:?} : {tilt}");
            // ses yeux entiers, penchés, à `EYE_MARGIN` de la coupe au plus près, du côté qui se voit
            let margin = |corner: Vec2| (turn(corner, hip, lean(side)).x - cut(side)) * side.outward();
            let nearest = corners(EYES).into_iter().map(margin).fold(f32::MAX, f32::min);
            assert!((nearest - EYE_MARGIN).abs() < 1e-3, "{side:?} : {nearest}");
            // sa bouche aussi (7,7 px au plus près à gauche, 8,6 à droite)
            for corner in corners(MOUTH_BOX) {
                assert!(margin(corner) > 7.0, "{side:?} : sa bouche à {} px de la coupe", margin(corner));
            }
            // sa hanche est derrière la fenêtre
            assert!(hip_out(side) < -20.0, "{side:?} : {}", hip_out(side));
        }
        // tourner de 0 ne change rien ; d'un angle puis de son opposé, on revient au même point
        let p = Vec2::new(20.0, 50.0);
        assert_eq!(turn(p, hip, 0.0), p);
        let back = turn(turn(p, hip, 0.3), hip, -0.3);
        assert!((back - p).length() < 1e-3);
    }

    #[test]
    fn what_shows_of_her_lurking_is_measured_from_her_outline_and_stops_at_the_cut() {
        for side in Side::ALL {
            let shown = lurk_bounds(side);
            let (lo, hi) = (shown.min, shown.max());
            // rien derrière la fenêtre
            match side {
                Side::Left => assert!(hi.x.abs() < 1e-3 && lo.x < -80.0, "{shown:?}"),
                Side::Right => assert!(lo.x.abs() < 1e-3 && hi.x > 80.0, "{shown:?}"),
            }
            // ses cheveux, bien au-dessus de sa hanche ; en bas, ce qui se voit d'elle va : à gauche,
            // le long de la coupe, jusqu'à sa hanche (son épaule, son haut, son bras, un bout de
            // jupe) ; à droite, pas plus bas que sa taille (sa tête, ses cheveux, un bord de son
            // corps le long de la coupe)
            assert!(lo.y < -200.0, "{side:?} : {shown:?}");
            match side {
                Side::Left => assert!(hi.y > -10.0 && hi.y < 10.0, "jusqu'à sa hanche : {shown:?}"),
                Side::Right => assert!(hi.y < -15.0, "pas plus bas que sa taille : {shown:?}"),
            }
            // ses yeux dedans
            let hip = Vec2::new(HIP.0, HIP.1);
            let anchor = Vec2::new(cut(side), hip.y);
            for corner in corners(EYES) {
                let p = turn(corner, hip, lean(side)) - anchor;
                assert!(shown.to_unit(p).is_some(), "{side:?} : {p:?} hors de {shown:?}");
            }
        }
        // calculé de sa silhouette le 24 novembre 2026 : à gauche, la coupe en x = 41,7, 103,7 px
        // dehors, 211,5 au-dessus de sa hanche et 1,6 en dessous ; à droite, en x = 90,6, 84,2 px
        // dehors, de 222,0 à 18,6 au-dessus de sa hanche
        assert!((cut(Side::Left) - 41.7).abs() < 0.05 && (cut(Side::Right) - 90.6).abs() < 0.05);
        let left = lurk_bounds(Side::Left);
        assert!((left.min.x + 103.7).abs() < 0.1 && (left.min.y + 211.5).abs() < 0.1, "{left:?}");
        assert!((left.max().y - 1.6).abs() < 0.1, "{left:?}");
        let right = lurk_bounds(Side::Right);
        assert!((right.max().x - 84.2).abs() < 0.1 && (right.min.y + 222.0).abs() < 0.1, "{right:?}");
        assert!((right.max().y + 18.6).abs() < 0.1, "{right:?}");
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
