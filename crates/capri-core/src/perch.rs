//! Elle s'assoit sur tes fenêtres : quels bords elle voit, où elle se pose, comment elle suit la
//! fenêtre, et quand elle repart. Tout est décidé ici, sans rien savoir du système.
//!
//! **Ce qu'on garde d'une fenêtre** : son numéro (un identifiant opaque attribué par le système,
//! pour la retrouver d'une lecture à l'autre) et son rectangle, dans l'ordre d'empilement. Jamais
//! son titre, jamais le nom de son app : la couche système ne les lit pas ([`DeskWindow`] n'a pas
//! où les mettre), et le journal ne dit que des nombres.
//!
//! **Les repères.** Le système donne ses rectangles dans le sien ([`DeskFrame`]) : sur macOS, en
//! points, origine en haut à gauche de l'écran principal, y vers le bas (CoreGraphics) ; sous
//! Windows, en pixels de l'écran virtuel. Capri vit en pixels physiques de son overlay, origine en
//! haut à gauche de l'overlay, comme partout dans ce crate. [`DeskFrame::to_overlay`] passe de l'un
//! à l'autre ; [`DeskFrame::from_appkit`] dit où est l'overlay quand AppKit le place depuis le bas.
//!
//! **« Assise »** veut dire ici : posée sur le bord supérieur d'une fenêtre, en un point du bord.
//! Avec les planches de sa pose assise, c'est sa ligne d'assise qui passe sur le bord, ses jambes
//! pendent devant la fenêtre ; sans elles, elle s'y tient debout, les pieds dessus. Ce module ne
//! connaît que ce qu'il lui faut de place autour de ce point ([`Stature`]) : ce qui se voit est
//! dans `look.rs` et `body.rs`.
//!
//! **Seules comptent** les fenêtres ordinaires, à l'écran (le système ne donne que celles-là),
//! opaques, assez grandes, sur son écran, et pas les siennes ([`skip`]) ; et seules les parties
//! visibles de leurs bords supérieurs ([`ledges`]) : une fenêtre devant en cache une derrière.
//!
//! **« Elle guette »** veut dire ici : derrière le côté gauche ou droit d'une fenêtre ([`Side`]),
//! penchée dehors, coupée net au bord. Ce module ne connaît que son point d'accroche, sur le côté,
//! à la hauteur de sa hanche ([`Lurk`]), et ce qui se voit d'elle autour ([`LurkStature`]) : les
//! morceaux visibles des côtés ([`jambs`], le pendant de [`ledges`]), ceux où elle tiendrait
//! ([`flanks`], le pendant de [`benches`]), le suivi et le départ ([`follow_flank`], le pendant de
//! [`follow`]). Ce qui se voit d'elle doit tenir dans l'écran, devant le côté de sa fenêtre, loin de
//! ses coins ([`CORNER`]), et aucune fenêtre devant ne doit le recouvrir, ni le côté sous elle :
//! jamais coupée dans le vide. Ce qui se voit et comment est dans `look.rs` et `body.rs`.
//!
//! Les seuils sont en fractions de sa hauteur, comme dans `body.rs`.

use crate::geom::{Rect, Vec2};

/// Distance d'accroche : lâchée avec les pieds à moins de cette distance d'un bord (au-dessus ou
/// en dessous), elle s'y pose. 0,15 de sa hauteur, soit 45 points pour 300 : un peu plus qu'une
/// barre de titre, sans happer une Capri qu'on voulait laisser flotter plus haut.
pub const SNAP: f32 = 0.15;
/// La moitié de la longueur de bord visible qu'il faut sous ses pieds, de part et d'autre de leur
/// milieu ; c'est aussi la distance qu'elle garde avec les coins (arrondis) d'une fenêtre.
pub const FOOT: f32 = 0.06;
/// Une fenêtre plus étroite que ça ne compte pas…
pub const MIN_WIDTH: f32 = 0.3;
/// … ni une fenêtre plus basse que ça.
pub const MIN_HEIGHT: f32 = 0.15;
/// En dessous de cette opacité, une fenêtre ne compte pas : elle apparaît, disparaît, ou ne se
/// voit pas.
pub const MIN_OPACITY: f32 = 0.9;
/// Guettant, ce qui se voit d'elle reste à cette distance des coins d'une fenêtre, en haut et en
/// bas de son côté : 0,1 de sa hauteur (30 points à sa taille d'origine, 12 à la plus petite),
/// pour que sa coupe ne tombe pas là où la fenêtre s'arrondit. Choisi, pas mesuré : le rayon des
/// coins dépend du système et de l'app.
pub const CORNER: f32 = 0.1;

/// Posée, elle relit la liste des fenêtres toutes les `SLOW` secondes (4 fois par seconde) tant
/// que sa fenêtre ne bouge pas…
pub const SLOW: f64 = 0.25;
/// … à chaque rafraîchissement de l'écran pendant que sa fenêtre bouge, ou qu'un bouton de la
/// souris est enfoncé (on est peut-être en train de la déplacer), et encore ce nombre de secondes
/// après le dernier mouvement vu.
pub const KEEP_FAST: f64 = 0.5;

/// Une fenêtre telle que le système la décrit, dans son propre repère. Rien d'autre : ni titre,
/// ni app. Le système ne donne que les fenêtres à l'écran, de la plus en avant à la plus en
/// arrière.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeskWindow {
    /// Son numéro, attribué par le système : de quoi la retrouver à la lecture suivante.
    pub id: u64,
    /// Son rectangle, dans le repère du système.
    pub rect: Rect,
    /// Une fenêtre d'app ordinaire : sur macOS, de la couche 0 (ni menu, ni Dock, ni panneau
    /// flottant) ; sous Windows, ni « outil », ni toujours au-dessus, ni traversable.
    pub ordinary: bool,
    /// 1 : opaque ; 0 : invisible.
    pub opacity: f32,
    /// Une fenêtre de Capri elle-même (son calque, son champ de saisie, ses réglages, ses menus).
    pub ours: bool,
}

/// Où est l'overlay dans le repère du système, et à quelle échelle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeskFrame {
    /// Le coin haut gauche de l'overlay, dans le repère du système.
    pub origin: Vec2,
    /// Pixels de l'overlay par unité du système : la densité de l'écran sur macOS (les fenêtres
    /// sont en points), 1 sous Windows (elles sont déjà en pixels).
    pub pixels_per_unit: f32,
}

impl DeskFrame {
    /// macOS : AppKit place l'overlay en points depuis le coin **bas** gauche de l'écran principal,
    /// y vers le haut ; CoreGraphics donne les fenêtres en points depuis son coin **haut** gauche,
    /// y vers le bas. `panel` : le cadre de l'overlay pour AppKit (`min` est son coin bas gauche) ;
    /// `primary_height` : la hauteur de l'écran principal, en points ; `scale` : la densité.
    pub fn from_appkit(panel: Rect, primary_height: f32, scale: f32) -> Self {
        let top = panel.min.y + panel.size.y;
        Self { origin: Vec2::new(panel.min.x, primary_height - top), pixels_per_unit: scale }
    }

    /// Un rectangle du système, en pixels de l'overlay.
    pub fn to_overlay(&self, rect: Rect) -> Rect {
        let k = self.pixels_per_unit;
        Rect::new((rect.min - self.origin) * k, rect.size * k)
    }
}

/// Une fenêtre qui compte : son numéro et son rectangle, en pixels de l'overlay. C'est tout ce que
/// Capri garde.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub id: u64,
    pub rect: Rect,
}

/// Pourquoi une fenêtre ne compte pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// Une des siennes.
    Ours,
    /// Pas une fenêtre d'app ordinaire (menu, Dock, panneau flottant…).
    Unusual,
    /// Pas assez opaque.
    Faint,
    /// Trop petite.
    Small,
    /// Pas sur son écran.
    Elsewhere,
}

/// Une fenêtre compte-t-elle ? `None` : oui. `screen` : l'overlay, en ses pixels ; `height` : la
/// hauteur de Capri, dans les mêmes pixels.
pub fn skip(window: &DeskWindow, frame: DeskFrame, screen: Rect, height: f32) -> Option<Skip> {
    if window.ours {
        return Some(Skip::Ours);
    }
    if !window.ordinary {
        return Some(Skip::Unusual);
    }
    // faux aussi pour une valeur qui n'est pas un nombre : elle ne compte pas
    let opaque = window.opacity >= MIN_OPACITY;
    if !opaque {
        return Some(Skip::Faint);
    }
    let rect = frame.to_overlay(window.rect);
    if !(rect.size.x >= MIN_WIDTH * height && rect.size.y >= MIN_HEIGHT * height) {
        return Some(Skip::Small);
    }
    if !overlaps(rect, screen) {
        return Some(Skip::Elsewhere);
    }
    None
}

/// Les fenêtres qui comptent, en pixels de l'overlay, dans l'ordre du système (de la plus en avant
/// à la plus en arrière).
pub fn sift(windows: &[DeskWindow], frame: DeskFrame, screen: Rect, height: f32) -> Vec<Seen> {
    windows
        .iter()
        .filter(|window| skip(window, frame, screen, height).is_none())
        .map(|window| Seen { id: window.id, rect: frame.to_overlay(window.rect) })
        .collect()
}

/// Un morceau visible du bord supérieur d'une fenêtre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ledge {
    pub window: u64,
    /// La hauteur du bord.
    pub y: f32,
    /// D'où à où il est visible.
    pub from: f32,
    pub to: f32,
}

/// Tous les morceaux visibles des bords supérieurs, sur l'écran, de la fenêtre la plus en avant
/// à la plus en arrière.
pub fn ledges(seen: &[Seen], screen: Rect) -> Vec<Ledge> {
    (0..seen.len())
        .flat_map(|index| {
            let window = seen[index];
            visible_edge(seen, index, screen).into_iter().map(move |(from, to)| Ledge {
                window: window.id,
                y: window.rect.min.y,
                from,
                to,
            })
        })
        .collect()
}

/// Ce qu'il lui faut sur un bord, en pixels de l'overlay. De quoi savoir où elle tient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stature {
    /// Sa hauteur debout : la mesure de [`SNAP`], [`MIN_WIDTH`] et [`MIN_HEIGHT`].
    pub height: f32,
    /// La place qu'elle occupe autour du point posé sur le bord : tout doit tenir dans l'écran.
    pub reach: Reach,
    /// Ce qui repose sur le bord, à gauche et à droite du point posé : ce morceau du bord doit
    /// être visible, et sur la fenêtre (assez loin de ses coins arrondis).
    pub rest: (f32, f32),
}

/// La place qu'elle occupe autour d'un point, vers la gauche, le haut, la droite et le bas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub left: f32,
    pub up: f32,
    pub right: f32,
    pub down: f32,
}

impl Stature {
    /// Debout, les pieds sur le bord : son image de `size` pixels, ses pieds en `feet` dans
    /// l'image (coordonnées normalisées : (0,5 ; 1) pour des pieds au milieu du bas), sa hauteur
    /// debout `height`. Ce qui repose sur le bord : [`FOOT`] de sa hauteur de part et d'autre du
    /// milieu de ses pieds.
    pub fn standing(size: Vec2, feet: Vec2, height: f32) -> Self {
        let foot = FOOT * height;
        Self {
            height,
            reach: Reach {
                left: size.x * feet.x,
                up: size.y * feet.y,
                right: size.x * (1.0 - feet.x),
                down: size.y * (1.0 - feet.y),
            },
            rest: (foot, foot),
        }
    }
}

/// Où elle est posée : sur quelle fenêtre, à quelle distance du bord gauche de cette fenêtre se
/// trouve le milieu de ses pieds, et où étaient alors les côtés de la fenêtre (de quoi savoir, à la
/// lecture suivante, si elle a été déplacée ou tirée par un côté). Pixels de l'overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub window: u64,
    pub along: f32,
    pub left: f32,
    pub width: f32,
}

impl Spot {
    fn on(window: &Seen, x: f32) -> Self {
        Self {
            window: window.id,
            along: x - window.rect.min.x,
            left: window.rect.min.x,
            width: window.rect.size.x,
        }
    }

    /// Le milieu de ses pieds sur le bord de `window` (nouvelle lecture de sa fenêtre), avant
    /// d'être borné : déplacée, ou tirée par la droite, elle garde sa distance au côté gauche ; tirée
    /// par la gauche, sa distance au côté droit (le morceau de bord sous elle n'a pas bougé) ;
    /// changée des deux côtés à la fois, sa proportion le long du bord.
    fn x_on(&self, window: Rect) -> f32 {
        let same = |a: f32, b: f32| (a - b).abs() < 0.5;
        let (left, right) = (window.min.x, window.max().x);
        let old_right = self.left + self.width;
        if same(window.size.x, self.width) || same(left, self.left) {
            left + self.along
        } else if same(right, old_right) {
            right - (self.width - self.along)
        } else {
            left + self.along * window.size.x / self.width.max(1.0)
        }
    }
}

/// Une place sur un bord : la fenêtre, et où mettre ses pieds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perch {
    pub spot: Spot,
    pub feet: Vec2,
}

/// Pourquoi elle quitte le bord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    /// Sa fenêtre n'est plus à l'écran : fermée, réduite dans le Dock, app masquée, autre bureau.
    Gone,
    /// Le morceau de bord sous ses pieds est recouvert par une autre fenêtre.
    Covered,
    /// Le bord sort de l'écran, ou ne lui laisse plus la place (plein écran, fenêtre trop haute).
    NoRoom,
}

impl Leave {
    /// Pour le journal, posée sur un bord.
    pub fn describe(self) -> &'static str {
        match self {
            Leave::Gone => "sa fenêtre n'est plus à l'écran",
            Leave::Covered => "le bord sous ses pieds est recouvert",
            Leave::NoRoom => "le bord sort de l'écran ou ne lui laisse plus la place",
        }
    }

    /// Pour le journal, guettant derrière un côté ([`follow_flank`]).
    pub fn describe_side(self) -> &'static str {
        match self {
            Leave::Gone => "sa fenêtre n'est plus à l'écran",
            Leave::Covered => "une fenêtre passe devant le côté où elle guette, ou là où elle se voit",
            Leave::NoRoom => "le côté sort de l'écran ou ne lui laisse plus la place",
        }
    }
}

/// On vient de la lâcher, ou elle vient de s'arrêter, les pieds en `feet` : se pose-t-elle ? Sur
/// le bord visible le plus proche de ses pieds, à moins de [`SNAP`], qui lui laisse la place
/// (`stature` : ce qu'il lui faudra une fois posée) ; le plus en avant à égalité. Le point posé
/// est sous ses pieds, glissé juste ce qu'il faut pour que ce qui repose sur le bord tienne sur le
/// morceau visible, loin de ses bouts, et qu'elle reste entière dans l'écran.
pub fn landing(seen: &[Seen], screen: Rect, stature: Stature, feet: Vec2) -> Option<Perch> {
    let (left, right) = stature.rest;
    let mut best: Option<(f32, Perch)> = None;
    for (index, window) in seen.iter().enumerate() {
        let y = window.rect.min.y;
        let gap = (y - feet.y).abs();
        let close = gap <= SNAP * stature.height;
        if !close {
            continue;
        }
        let Some((lo, hi)) = room(window.rect, screen, stature) else { continue };
        for (from, to) in visible_edge(seen, index, screen) {
            if !(from <= feet.x && feet.x <= to) {
                continue;
            }
            // `clamp` exige lo <= hi : un morceau trop court pour ce qui repose dessus ne la porte pas
            let (lo, hi) = (lo.max(from + left), hi.min(to - right));
            let fits = lo <= hi;
            if !fits {
                continue;
            }
            let x = feet.x.clamp(lo, hi);
            let perch = Perch { spot: Spot::on(window, x), feet: Vec2::new(x, y) };
            if best.is_none_or(|(closest, _)| gap < closest) {
                best = Some((gap, perch));
            }
        }
    }
    best.map(|(_, perch)| perch)
}

/// Un morceau visible du bord supérieur d'une fenêtre où elle tiendrait, et d'où à où peut aller
/// le point posé dessus : ce qui repose sur le bord sur le morceau, loin de ses bouts, et elle
/// entière dans l'écran, comme [`landing`] la poserait. De quoi choisir où aller (`crate::roam`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bench {
    /// Sa fenêtre : son numéro et son rectangle, en pixels de l'overlay.
    pub window: Seen,
    /// D'où à où peut aller le point posé (`lo` ≤ `hi`).
    pub lo: f32,
    pub hi: f32,
}

impl Bench {
    /// La hauteur du bord.
    pub fn y(&self) -> f32 {
        self.window.rect.min.y
    }

    /// La place sur ce morceau de bord, le point posé en `x` (borné au morceau).
    pub fn at(&self, x: f32) -> Perch {
        let x = if x.is_nan() { self.lo } else { x.clamp(self.lo, self.hi) };
        Perch { spot: Spot::on(&self.window, x), feet: Vec2::new(x, self.y()) }
    }
}

/// Tous les morceaux visibles des bords où elle tiendrait (`stature` : ce qu'il lui faut une fois
/// posée), de la fenêtre la plus en avant à la plus en arrière. Posée sur l'un d'eux
/// ([`Bench::at`]), [`follow`] la garde là tant que rien ne bouge.
pub fn benches(seen: &[Seen], screen: Rect, stature: Stature) -> Vec<Bench> {
    let (left, right) = stature.rest;
    let mut benches = Vec::new();
    for (index, window) in seen.iter().enumerate() {
        let Some((lo, hi)) = room(window.rect, screen, stature) else { continue };
        for (from, to) in visible_edge(seen, index, screen) {
            // `clamp` exige lo <= hi : un morceau trop court pour ce qui repose dessus ne la porte pas
            let (lo, hi) = (lo.max(from + left), hi.min(to - right));
            if lo <= hi {
                benches.push(Bench { window: *window, lo, hi });
            }
        }
    }
    benches
}

/// Posée en `spot`, d'après une nouvelle lecture des fenêtres : où mettre ses pieds pour rester à
/// la même place le long du bord (bornée au bord, loin de ses coins, et elle entière dans
/// l'écran), ou pourquoi elle repart.
pub fn follow(seen: &[Seen], screen: Rect, stature: Stature, spot: Spot) -> Result<Perch, Leave> {
    let index = seen.iter().position(|window| window.id == spot.window).ok_or(Leave::Gone)?;
    let window = &seen[index];
    let (lo, hi) = room(window.rect, screen, stature).ok_or(Leave::NoRoom)?;
    // `room` garantit lo <= hi, deux nombres ; une place qui ne serait pas un nombre donne un x qui
    // n'en est pas un, et aucun morceau visible ne le contient
    let x = spot.x_on(window.rect).clamp(lo, hi);
    let (left, right) = stature.rest;
    let free =
        visible_edge(seen, index, screen).into_iter().any(|(from, to)| from <= x - left && x + right <= to);
    if !free {
        return Err(Leave::Covered);
    }
    Ok(Perch { spot: Spot::on(window, x), feet: Vec2::new(x, window.rect.min.y) })
}

/// Posée en `spot`, d'après une lecture des fenêtres qui vient d'être faite : tiendrait-elle là
/// avec `stature` (une autre pose, plus large : allongée), **sans bouger** ? Ce qui repose sur le
/// bord sur un morceau visible, loin de ses bouts, et elle entière dans l'écran, son point posé là
/// où il est. De quoi savoir si elle peut s'y allonger, sans lire la liste une fois de plus.
pub fn fits(seen: &[Seen], screen: Rect, stature: Stature, spot: Spot) -> bool {
    let Some(index) = seen.iter().position(|window| window.id == spot.window) else { return false };
    let window = &seen[index];
    let Some((lo, hi)) = room(window.rect, screen, stature) else { return false };
    let x = spot.x_on(window.rect);
    if !(lo <= x && x <= hi) {
        return false;
    }
    let (left, right) = stature.rest;
    visible_edge(seen, index, screen).into_iter().any(|(from, to)| from <= x - left && x + right <= to)
}

/// Où peut aller le point posé le long du bord supérieur de `window` : ce qui repose dessus sur
/// le bord, loin de ses bouts, et elle entière dans l'écran. `None` : le bord est hors de l'écran,
/// ou ne lui laisse pas la place.
fn room(window: Rect, screen: Rect, stature: Stature) -> Option<(f32, f32)> {
    let Stature { reach, rest, .. } = stature;
    let y = window.min.y;
    if !(y - reach.up >= screen.min.y && y + reach.down <= screen.max().y) {
        return None;
    }
    let lo = (window.min.x + rest.0).max(screen.min.x + reach.left);
    let hi = (window.max().x - rest.1).min(screen.max().x - reach.right);
    (lo <= hi).then_some((lo, hi))
}

/// Les morceaux visibles du bord supérieur de la fenêtre n° `index` : sur l'écran, moins ce que
/// cachent les fenêtres devant elle.
fn visible_edge(seen: &[Seen], index: usize, screen: Rect) -> Vec<(f32, f32)> {
    let Some(window) = seen.get(index) else { return Vec::new() };
    let y = window.rect.min.y;
    if !(screen.min.y <= y && y < screen.max().y) {
        return Vec::new();
    }
    let (from, to) = (window.rect.min.x.max(screen.min.x), window.rect.max().x.min(screen.max().x));
    let mut pieces = if from < to { vec![(from, to)] } else { Vec::new() };
    for front in &seen[..index] {
        let r = front.rect;
        if !(r.min.y <= y && y < r.max().y) {
            continue;
        }
        pieces = pieces.into_iter().flat_map(|piece| cut(piece, (r.min.x, r.max().x))).collect();
    }
    pieces
}

/// Un côté d'une fenêtre, où elle guette : le gauche (elle est à gauche de la fenêtre, penchée
/// vers la gauche) ou le droit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Left, Side::Right];

    /// Vers l'extérieur de la fenêtre, en x : −1 à gauche, +1 à droite.
    pub fn outward(self) -> f32 {
        match self {
            Side::Left => -1.0,
            Side::Right => 1.0,
        }
    }

    /// Où est ce côté de `window`, en x.
    pub fn x(self, window: Rect) -> f32 {
        match self {
            Side::Left => window.min.x,
            Side::Right => window.max().x,
        }
    }

    /// Son rang dans [`Side::ALL`] : de quoi ranger une mesure par côté.
    pub fn index(self) -> usize {
        match self {
            Side::Left => 0,
            Side::Right => 1,
        }
    }

    /// Pour le journal.
    pub fn describe(self) -> &'static str {
        match self {
            Side::Left => "gauche",
            Side::Right => "droit",
        }
    }
}

/// Un morceau visible du côté d'une fenêtre (le pendant de [`Ledge`]) : la fenêtre, le côté, où il
/// est en x, et d'où à où il se voit, en y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jamb {
    pub window: u64,
    pub side: Side,
    pub x: f32,
    pub from: f32,
    pub to: f32,
}

/// Tous les morceaux visibles des côtés des fenêtres, sur l'écran, de la fenêtre la plus en avant à
/// la plus en arrière : le pendant de [`ledges`].
pub fn jambs(seen: &[Seen], screen: Rect) -> Vec<Jamb> {
    let mut jambs = Vec::new();
    for (index, window) in seen.iter().enumerate() {
        for side in Side::ALL {
            let x = side.x(window.rect);
            for (from, to) in visible_side(seen, index, side, screen, 0.0) {
                jambs.push(Jamb { window: window.id, side, x, from, to });
            }
        }
    }
    jambs
}

/// Ce qu'il lui faut sur le côté d'une fenêtre pour y guetter, en pixels de l'overlay : ce qui se
/// voit d'elle autour de son point d'accroche (sur le côté, à la hauteur de sa hanche), pour chaque
/// côté, et sa hauteur debout. Rien ne se voit d'elle derrière la fenêtre : `left.right` et
/// `right.left` valent 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LurkStature {
    /// Sa hauteur debout : la mesure de [`CORNER`].
    pub height: f32,
    /// Sur le côté gauche d'une fenêtre, et sur le côté droit.
    pub left: Reach,
    pub right: Reach,
}

impl LurkStature {
    /// Ce qui se voit d'elle sur ce côté.
    pub fn on(&self, side: Side) -> Reach {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }
}

/// Où elle guette : sur quelle fenêtre, de quel côté, à quelle distance du haut de cette fenêtre
/// est son point d'accroche, et où étaient alors le haut et le bas de la fenêtre (de quoi savoir,
/// à la lecture suivante, si elle a été déplacée ou tirée par le haut ou le bas). Pixels de
/// l'overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlankSpot {
    pub window: u64,
    pub side: Side,
    pub along: f32,
    pub top: f32,
    pub height: f32,
}

impl FlankSpot {
    fn on(window: &Seen, side: Side, y: f32) -> Self {
        Self {
            window: window.id,
            side,
            along: y - window.rect.min.y,
            top: window.rect.min.y,
            height: window.rect.size.y,
        }
    }

    /// Son point d'accroche le long du côté de `window` (nouvelle lecture de sa fenêtre), avant
    /// d'être borné, comme [`Spot`] le long d'un bord : déplacée, ou tirée par le bas, elle garde sa
    /// distance au haut ; tirée par le haut, sa distance au bas ; changée des deux à la fois, sa
    /// proportion le long du côté.
    fn y_on(&self, window: Rect) -> f32 {
        let same = |a: f32, b: f32| (a - b).abs() < 0.5;
        let (top, bottom) = (window.min.y, window.max().y);
        let old_bottom = self.top + self.height;
        if same(window.size.y, self.height) || same(top, self.top) {
            top + self.along
        } else if same(bottom, old_bottom) {
            bottom - (self.height - self.along)
        } else {
            top + self.along * window.size.y / self.height.max(1.0)
        }
    }
}

/// Une place sur le côté d'une fenêtre : la fenêtre et son côté, et son point d'accroche, sur le
/// côté (en x), à la hauteur de sa hanche (en y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lurk {
    pub spot: FlankSpot,
    pub at: Vec2,
}

/// Un morceau visible du côté d'une fenêtre où elle tiendrait à guetter (le pendant de [`Bench`]) :
/// ce qui se voit d'elle sur le morceau, loin des coins de la fenêtre, et dans l'écran, sans
/// fenêtre devant. De quoi choisir où aller (`crate::roam`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flank {
    /// Sa fenêtre : son numéro et son rectangle, en pixels de l'overlay.
    pub window: Seen,
    pub side: Side,
    /// D'où à où peut aller son point d'accroche, en y (`lo` ≤ `hi`).
    pub lo: f32,
    pub hi: f32,
}

impl Flank {
    /// Où est le côté, en x.
    pub fn x(&self) -> f32 {
        self.side.x(self.window.rect)
    }

    /// La place sur ce morceau de côté, son point d'accroche en `y` (borné au morceau).
    pub fn at(&self, y: f32) -> Lurk {
        let y = if y.is_nan() { self.lo } else { y.clamp(self.lo, self.hi) };
        Lurk { spot: FlankSpot::on(&self.window, self.side, y), at: Vec2::new(self.x(), y) }
    }
}

/// Tous les morceaux visibles des côtés où elle tiendrait à guetter (`stature` : ce qui se voit
/// d'elle), de la fenêtre la plus en avant à la plus en arrière. Guettant sur l'un d'eux
/// ([`Flank::at`]), [`follow_flank`] la garde là tant que rien ne bouge.
pub fn flanks(seen: &[Seen], screen: Rect, stature: LurkStature) -> Vec<Flank> {
    let mut flanks = Vec::new();
    for (index, window) in seen.iter().enumerate() {
        for side in Side::ALL {
            let Some((lo, hi)) = flank_room(window.rect, side, screen, stature) else { continue };
            let reach = stature.on(side);
            for (from, to) in visible_side(seen, index, side, screen, out_of(reach)) {
                // `clamp` exige lo <= hi : un morceau trop court pour ce qui se voit d'elle ne la
                // porte pas
                let (lo, hi) = (lo.max(from + reach.up), hi.min(to - reach.down));
                if lo <= hi {
                    flanks.push(Flank { window: *window, side, lo, hi });
                }
            }
        }
    }
    flanks
}

/// Guettant en `spot`, d'après une nouvelle lecture des fenêtres : où mettre son point d'accroche
/// pour rester à la même place le long du côté (borné au côté, loin de ses coins, ce qui se voit
/// d'elle dans l'écran), ou pourquoi elle ne guette plus là : sa fenêtre partie, le côté recouvert
/// (sous elle ou là où elle se voit), sorti de l'écran, ou plus la place.
pub fn follow_flank(
    seen: &[Seen],
    screen: Rect,
    stature: LurkStature,
    spot: FlankSpot,
) -> Result<Lurk, Leave> {
    let index = seen.iter().position(|window| window.id == spot.window).ok_or(Leave::Gone)?;
    let window = &seen[index];
    let (lo, hi) = flank_room(window.rect, spot.side, screen, stature).ok_or(Leave::NoRoom)?;
    // `flank_room` garantit lo <= hi ; une place qui n'est pas un nombre donne un y qui n'en est
    // pas un, et aucun morceau visible ne le contient
    let y = spot.y_on(window.rect).clamp(lo, hi);
    let reach = stature.on(spot.side);
    let free = visible_side(seen, index, spot.side, screen, out_of(reach))
        .into_iter()
        .any(|(from, to)| from <= y - reach.up && y + reach.down <= to);
    if !free {
        return Err(Leave::Covered);
    }
    Ok(Lurk { spot: FlankSpot::on(window, spot.side, y), at: Vec2::new(spot.side.x(window.rect), y) })
}

/// Combien ce qui se voit d'elle va vers l'extérieur de sa fenêtre.
fn out_of(reach: Reach) -> f32 {
    reach.left.max(reach.right).max(0.0)
}

/// Où peut aller son point d'accroche le long du côté `side` de `window` : ce qui se voit d'elle
/// dans l'écran et devant le côté, loin de ses coins ([`CORNER`]). `None` : le côté est hors de
/// l'écran, ou ne lui laisse pas la place.
fn flank_room(window: Rect, side: Side, screen: Rect, stature: LurkStature) -> Option<(f32, f32)> {
    let reach = stature.on(side);
    let x = side.x(window);
    if !(x - reach.left >= screen.min.x && x + reach.right <= screen.max().x) {
        return None;
    }
    let corner = CORNER * stature.height;
    let lo = (window.min.y + corner + reach.up).max(screen.min.y + reach.up);
    let hi = (window.max().y - corner - reach.down).min(screen.max().y - reach.down);
    (lo <= hi).then_some((lo, hi))
}

/// Les morceaux visibles du côté `side` de la fenêtre n° `index`, en y : sur l'écran, moins ce que
/// cachent les fenêtres devant elle, sur le côté même ou sur la bande de `out` pixels dehors où
/// elle se voit (une fenêtre devant qui la recouvrirait). Sans bande (`out` = 0), seul le côté
/// compte.
fn visible_side(seen: &[Seen], index: usize, side: Side, screen: Rect, out: f32) -> Vec<(f32, f32)> {
    let Some(window) = seen.get(index) else { return Vec::new() };
    let x = side.x(window.rect);
    if !(screen.min.x <= x && x <= screen.max().x) {
        return Vec::new();
    }
    let (from, to) = (window.rect.min.y.max(screen.min.y), window.rect.max().y.min(screen.max().y));
    let mut pieces = if from < to { vec![(from, to)] } else { Vec::new() };
    // la bande où elle se voit, dehors, le côté compris
    let (a, b) = match side {
        Side::Left => (x - out, x),
        Side::Right => (x, x + out),
    };
    for front in &seen[..index] {
        let r = front.rect;
        // sur la bande (une fenêtre qui la touche seulement ne la recouvre pas) ; sans bande, il
        // faut qu'elle passe sur le côté même
        let over = if out > 0.0 { r.min.x < b && a < r.max().x } else { r.min.x < x && x < r.max().x };
        if !over {
            continue;
        }
        pieces = pieces.into_iter().flat_map(|piece| cut(piece, (r.min.y, r.max().y))).collect();
    }
    pieces
}

/// `[a, b]` privé de `[c, d]` : zéro, un ou deux morceaux.
fn cut((a, b): (f32, f32), (c, d): (f32, f32)) -> impl Iterator<Item = (f32, f32)> {
    [(a, b.min(c)), (a.max(d), b)].into_iter().filter(|(from, to)| from < to)
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.min.x < b.max().x && b.min.x < a.max().x && a.min.y < b.max().y && b.min.y < a.max().y
}

/// Quand relire la liste des fenêtres pendant qu'elle est posée. Le temps est compté en secondes,
/// pas en images d'animation : suivre sa fenêtre, c'est comme être tenue par la souris, à la
/// cadence de l'écran (voir [`crate::pace`]) ; ses dessins, eux, restent à 24 i/s.
#[derive(Clone, Debug, Default)]
pub struct Watch {
    last_read: Option<f64>,
    last_rect: Option<Rect>,
    moved_at: Option<f64>,
}

impl Watch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Relire à la première occasion : elle revient d'être cachée.
    pub fn wake(&mut self) {
        *self = Self::default();
    }

    /// Est-elle pressée à l'instant `now` (secondes) : un bouton de la souris est enfoncé
    /// (`mouse_down`), ou sa fenêtre a bougé il y a moins de [`KEEP_FAST`] ? Alors elle relit à
    /// chaque rafraîchissement de l'écran, et la boucle doit se réveiller aussi souvent.
    pub fn hurried(&self, now: f64, mouse_down: bool) -> bool {
        mouse_down || self.moved_at.is_some_and(|at| now - at < KEEP_FAST)
    }

    /// Faut-il relire à l'instant `now` ? `period` : la durée d'un rafraîchissement de l'écran, en
    /// secondes. Pressée, une fois par rafraîchissement : un quart de période suffit entre deux
    /// lectures, pour qu'un réveil d'écran arrivé en retard (une lecture prend jusqu'à 3,5 ms
    /// pendant qu'une fenêtre bouge, mesuré) ne fasse pas sauter le suivant, tout en ne relisant
    /// pas pour un réveil de plus dans la même image. Sinon toutes les [`SLOW`] secondes.
    pub fn due(&self, now: f64, mouse_down: bool, period: f64) -> bool {
        let Some(last) = self.last_read else { return true };
        let wait = if self.hurried(now, mouse_down) { period * 0.25 } else { SLOW };
        // un instant qui n'est pas un nombre : relire plutôt que se figer
        let elapsed = now - last;
        elapsed >= wait || elapsed.is_nan()
    }

    /// Une lecture vient d'être faite à l'instant `now` ; `rect` : sa fenêtre, telle que lue.
    pub fn saw(&mut self, now: f64, rect: Option<Rect>) {
        if self.last_read.is_some() && rect != self.last_rect {
            self.moved_at = Some(now);
        }
        self.last_read = Some(now);
        self.last_rect = rect;
    }

    /// Guettant en `spot`, à l'instant `now` : la veille de son côté, comme posée sur un bord. S'il
    /// est l'heure de relire ([`Watch::due`] : `mouse_down`, `period`), `read` lit les fenêtres qui
    /// comptent (`None` : la liste illisible), la lecture est notée ([`Watch::saw`]), et elle suit
    /// son côté, ou ne guette plus là ([`follow_flank`], `screen`, `stature`). Sinon, rien n'est lu.
    #[allow(clippy::too_many_arguments)]
    pub fn flank(
        &mut self,
        spot: FlankSpot,
        now: f64,
        mouse_down: bool,
        period: f64,
        screen: Rect,
        stature: LurkStature,
        read: impl FnOnce() -> Option<Vec<Seen>>,
    ) -> Kept {
        if !self.due(now, mouse_down, period) {
            return Kept::Waiting;
        }
        let Some(seen) = read() else { return Kept::Lost(None) };
        let rect = seen.iter().find(|window| window.id == spot.window).map(|window| window.rect);
        self.saw(now, rect);
        match follow_flank(&seen, screen, stature, spot) {
            Ok(lurk) => Kept::Follows(lurk),
            Err(why) => Kept::Lost(Some(why)),
        }
    }
}

/// Ce que dit la veille d'une guetteuse ([`Watch::flank`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kept {
    /// Pas l'heure de relire : rien n'a été lu.
    Waiting,
    /// Relue : elle suit son côté, là.
    Follows(Lurk),
    /// Relue : elle ne guette plus là, pour cette raison ; `None` : la liste illisible.
    Lost(Option<Leave>),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un écran Retina de 1512 × 982 points, en pixels.
    fn screen() -> Rect {
        Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0))
    }

    /// Capri à 300 points de haut sur cet écran, les pieds au milieu du bas de son image.
    fn capri() -> Stature {
        Stature::standing(Vec2::new(314.0, 600.0), Vec2::new(0.5, 1.0), 600.0)
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(Vec2::new(x, y), Vec2::new(w, h))
    }

    fn seen(id: u64, r: Rect) -> Seen {
        Seen { id, rect: r }
    }

    fn window(id: u64, r: Rect) -> DeskWindow {
        DeskWindow { id, rect: r, ordinary: true, opacity: 1.0, ours: false }
    }

    const RETINA: DeskFrame = DeskFrame { origin: Vec2::ZERO, pixels_per_unit: 2.0 };

    #[test]
    fn appkit_and_coregraphics_meet_at_the_top_left_of_the_main_screen() {
        // l'overlay couvre l'écran principal : son coin haut gauche est l'origine de CoreGraphics
        let main = DeskFrame::from_appkit(rect(0.0, 0.0, 1512.0, 982.0), 982.0, 2.0);
        assert_eq!(main, DeskFrame { origin: Vec2::ZERO, pixels_per_unit: 2.0 });
        // une fenêtre à (100, 50) points depuis le haut est à (200, 100) pixels dans l'overlay
        assert_eq!(main.to_overlay(rect(100.0, 50.0, 800.0, 600.0)), rect(200.0, 100.0, 1600.0, 1200.0));

        // un écran posé au-dessus de l'écran principal : y négatif pour CoreGraphics
        let above = DeskFrame::from_appkit(rect(-200.0, 982.0, 1920.0, 1080.0), 982.0, 1.0);
        assert_eq!(above.origin, Vec2::new(-200.0, -1080.0));
        // son coin haut gauche, vu par CoreGraphics, est bien le (0, 0) de cet overlay
        assert_eq!(above.to_overlay(rect(-200.0, -1080.0, 10.0, 10.0)).min, Vec2::ZERO);
        // et une fenêtre de l'écran principal est sous lui
        assert_eq!(above.to_overlay(rect(0.0, 0.0, 10.0, 10.0)).min, Vec2::new(200.0, 1080.0));
    }

    #[test]
    fn windows_pixels_are_only_shifted() {
        // Windows : un overlay sur un second écran à gauche de l'écran principal
        let frame = DeskFrame { origin: Vec2::new(-1920.0, 0.0), pixels_per_unit: 1.0 };
        assert_eq!(frame.to_overlay(rect(-1900.0, 40.0, 800.0, 600.0)), rect(20.0, 40.0, 800.0, 600.0));
    }

    #[test]
    fn only_ordinary_opaque_large_windows_on_her_screen_and_not_hers_count() {
        let h = capri().height;
        let ok = window(1, rect(100.0, 300.0, 800.0, 500.0));
        assert_eq!(skip(&ok, RETINA, screen(), h), None);
        assert_eq!(skip(&DeskWindow { ours: true, ..ok }, RETINA, screen(), h), Some(Skip::Ours));
        assert_eq!(skip(&DeskWindow { ordinary: false, ..ok }, RETINA, screen(), h), Some(Skip::Unusual));
        assert_eq!(skip(&DeskWindow { opacity: 0.5, ..ok }, RETINA, screen(), h), Some(Skip::Faint));
        assert_eq!(skip(&DeskWindow { opacity: f32::NAN, ..ok }, RETINA, screen(), h), Some(Skip::Faint));
        // 80 × 500 points : 160 pixels de large, moins de 0,3 × 600
        let narrow = DeskWindow { rect: rect(100.0, 300.0, 80.0, 500.0), ..ok };
        assert_eq!(skip(&narrow, RETINA, screen(), h), Some(Skip::Small));
        let flat = DeskWindow { rect: rect(100.0, 300.0, 800.0, 40.0), ..ok };
        assert_eq!(skip(&flat, RETINA, screen(), h), Some(Skip::Small));
        let nan = DeskWindow { rect: rect(f32::NAN, 300.0, f32::NAN, 500.0), ..ok };
        assert!(skip(&nan, RETINA, screen(), h).is_some());
        // sur un second écran, à droite : hors de l'overlay
        let right = DeskWindow { rect: rect(1600.0, 300.0, 800.0, 500.0), ..ok };
        assert_eq!(skip(&right, RETINA, screen(), h), Some(Skip::Elsewhere));
        // à cheval sur les deux : elle compte
        let across = DeskWindow { rect: rect(1400.0, 300.0, 800.0, 500.0), ..ok };
        assert_eq!(skip(&across, RETINA, screen(), h), None);

        let kept = sift(
            &[DeskWindow { ours: true, ..ok }, ok, narrow, DeskWindow { id: 2, ..across }],
            RETINA,
            screen(),
            h,
        );
        assert_eq!(
            kept,
            [seen(1, rect(200.0, 600.0, 1600.0, 1000.0)), seen(2, rect(2800.0, 600.0, 1600.0, 1000.0))],
            "dans l'ordre, en pixels de l'overlay"
        );
    }

    #[test]
    fn a_window_in_front_hides_the_edge_behind_it() {
        let back = seen(1, rect(200.0, 1000.0, 1600.0, 800.0));
        // devant, à cheval sur le milieu du bord de `back`
        let front = seen(2, rect(800.0, 900.0, 400.0, 400.0));
        let found = ledges(&[front, back], screen());
        assert_eq!(
            found,
            [
                Ledge { window: 2, y: 900.0, from: 800.0, to: 1200.0 },
                Ledge { window: 1, y: 1000.0, from: 200.0, to: 800.0 },
                Ledge { window: 1, y: 1000.0, from: 1200.0, to: 1800.0 },
            ]
        );
        // derrière, elle ne cache rien
        let found = ledges(&[back, front], screen());
        assert!(found.contains(&Ledge { window: 1, y: 1000.0, from: 200.0, to: 1800.0 }));

        // une fenêtre au-dessus du bord sans le toucher ne cache rien ; une fenêtre dont le bas
        // tombe pile sur le bord non plus
        let above = seen(3, rect(0.0, 500.0, 3000.0, 500.0));
        assert_eq!(
            ledges(&[above, back], screen())[1],
            Ledge { window: 1, y: 1000.0, from: 200.0, to: 1800.0 }
        );
        // une fenêtre dont le haut est pile sur le bord le cache
        let level = seen(4, rect(0.0, 1000.0, 1000.0, 50.0));
        assert_eq!(
            ledges(&[level, back], screen())[1],
            Ledge { window: 1, y: 1000.0, from: 1000.0, to: 1800.0 }
        );
    }

    #[test]
    fn edges_are_clipped_to_her_screen() {
        // une fenêtre qui dépasse à gauche et à droite, et une dont le bord est au-dessus de l'écran
        let wide = seen(1, rect(-500.0, 1000.0, 4000.0, 500.0));
        let high = seen(2, rect(100.0, -50.0, 800.0, 500.0));
        assert_eq!(ledges(&[wide, high], screen()), [Ledge { window: 1, y: 1000.0, from: 0.0, to: 3024.0 }]);
    }

    #[test]
    fn she_lands_on_the_visible_edge_closest_to_her_feet() {
        let back = seen(1, rect(200.0, 1000.0, 1600.0, 800.0));
        // devant, son bord 60 pixels plus bas : il ne cache pas celui de `back`
        let front = seen(2, rect(1200.0, 1060.0, 1200.0, 600.0));
        let windows = [front, back];
        // les pieds 40 pixels au-dessus du bord de `back` : elle descend s'y poser
        let perch = landing(&windows, screen(), capri(), Vec2::new(700.0, 960.0)).expect("posée");
        let spot = Spot { window: 1, along: 500.0, left: 200.0, width: 1600.0 };
        assert_eq!(perch, Perch { spot, feet: Vec2::new(700.0, 1000.0) });
        // 60 pixels dans la barre de titre : elle remonte
        let perch = landing(&windows, screen(), capri(), Vec2::new(700.0, 1060.0)).expect("posée");
        assert_eq!(perch.feet, Vec2::new(700.0, 1000.0));
        // trop loin (plus de 0,15 × 600 = 90 pixels) : elle flotte
        assert_eq!(landing(&windows, screen(), capri(), Vec2::new(700.0, 895.0)), None);
        // entre les deux bords : le plus proche de ses pieds
        let perch = landing(&windows, screen(), capri(), Vec2::new(1500.0, 1040.0)).expect("posée");
        assert_eq!(perch.spot.window, 2);
        let perch = landing(&windows, screen(), capri(), Vec2::new(1500.0, 1020.0)).expect("posée");
        assert_eq!(perch.spot.window, 1);

        // au-dessus d'un morceau du bord de `back` caché par une fenêtre devant : rien
        let cover = seen(3, rect(1200.0, 900.0, 1200.0, 600.0));
        assert_eq!(landing(&[cover, back], screen(), capri(), Vec2::new(1500.0, 1000.0)), None);
    }

    #[test]
    fn her_feet_slide_onto_the_edge_and_away_from_its_corners() {
        let w = seen(1, rect(200.0, 1000.0, 1600.0, 800.0));
        let foot = FOOT * capri().height; // 36 pixels
        // juste au bord gauche de la fenêtre : ses pieds rentrent d'une demi-largeur
        let perch = landing(&[w], screen(), capri(), Vec2::new(205.0, 1000.0)).expect("posée");
        assert_eq!(perch.feet.x, 200.0 + foot);
        // à côté de la fenêtre : rien
        assert_eq!(landing(&[w], screen(), capri(), Vec2::new(195.0, 1000.0)), None);
        // un morceau visible trop court pour ses pieds : rien
        let cover = seen(2, rect(260.0, 900.0, 2000.0, 400.0));
        assert_eq!(landing(&[cover, w], screen(), capri(), Vec2::new(230.0, 1000.0)), None);
    }

    #[test]
    fn she_needs_room_above_the_edge_and_stays_whole_on_screen() {
        // le bord à 500 pixels du haut : elle en fait 600, sa tête sortirait
        let high = seen(1, rect(200.0, 500.0, 1600.0, 800.0));
        assert_eq!(landing(&[high], screen(), capri(), Vec2::new(700.0, 500.0)), None);
        // plein écran : le bord est tout en haut
        let full = seen(2, rect(0.0, 0.0, 3024.0, 1964.0));
        assert_eq!(landing(&[full], screen(), capri(), Vec2::new(700.0, 0.0)), None);
        // contre le bord droit de l'écran : ses pieds rentrent assez pour qu'elle y tienne entière
        let right = seen(3, rect(2000.0, 1000.0, 1024.0, 800.0));
        let perch = landing(&[right], screen(), capri(), Vec2::new(3000.0, 1000.0)).expect("posée");
        assert_eq!(perch.feet.x, 3024.0 - 157.0);
        // ses pieds ne sont pas au milieu de son image : c'est la place de ses pieds qui compte
        let offset = Stature::standing(Vec2::new(314.0, 600.0), Vec2::new(0.6, 1.0), 600.0);
        let perch = landing(&[right], screen(), offset, Vec2::new(3000.0, 1000.0)).expect("posée");
        assert!((perch.feet.x - (3024.0 - 314.0 * 0.4)).abs() < 0.01, "{perch:?}");
    }

    #[test]
    fn sitting_she_needs_less_room_above_but_a_longer_piece_of_edge_and_room_below() {
        // assise, à la taille d'origine sur cet écran : 341 pixels au-dessus du bord, 164 en
        // dessous (ses jambes pendent), et 95 de part et d'autre sur le bord (sa main, son genou)
        let sitting = Stature {
            height: 600.0,
            reach: Reach { left: 99.0, up: 341.0, right: 169.0, down: 164.0 },
            rest: (95.0, 95.0),
        };
        // un bord à 400 pixels du haut : debout, sa tête sortirait ; assise, elle s'y pose
        let high = seen(1, rect(200.0, 400.0, 1600.0, 800.0));
        assert_eq!(landing(&[high], screen(), capri(), Vec2::new(700.0, 400.0)), None);
        let perch = landing(&[high], screen(), sitting, Vec2::new(700.0, 400.0)).expect("posée");
        assert_eq!(perch.feet, Vec2::new(700.0, 400.0));
        // un morceau visible de 150 pixels porte ses pieds, pas ce qui repose assise sur le bord
        let cover = seen(2, rect(350.0, 300.0, 2000.0, 400.0));
        let piece = [cover, high];
        let standing_low = Stature::standing(Vec2::new(157.0, 300.0), Vec2::new(0.5, 1.0), 300.0);
        assert!(landing(&piece, screen(), standing_low, Vec2::new(275.0, 400.0)).is_some());
        assert_eq!(landing(&piece, screen(), sitting, Vec2::new(275.0, 400.0)), None);
        // près du bout du bord, le point posé rentre de ce qui repose dessus
        let perch = landing(&[high], screen(), sitting, Vec2::new(210.0, 400.0)).expect("posée");
        assert_eq!(perch.feet.x, 295.0);
        // un bord trop bas : ses jambes sortiraient de l'écran
        let low = seen(3, rect(200.0, 1900.0, 1600.0, 64.0));
        assert_eq!(landing(&[low], screen(), sitting, Vec2::new(700.0, 1900.0)), None);
        // posée, si une fenêtre recouvre ce qui repose sur le bord, elle repart
        let spot = perch.spot;
        let over = seen(4, rect(100.0, 350.0, 220.0, 300.0));
        assert_eq!(follow(&[over, high], screen(), sitting, spot), Err(Leave::Covered));
    }

    #[test]
    fn benches_are_the_visible_pieces_where_she_would_land_and_stay() {
        let sitting = Stature {
            height: 600.0,
            reach: Reach { left: 99.0, up: 341.0, right: 169.0, down: 164.0 },
            rest: (95.0, 95.0),
        };
        let back = seen(1, rect(200.0, 1000.0, 1600.0, 800.0));
        // devant, à cheval sur le milieu du bord de `back`
        let front = seen(2, rect(800.0, 900.0, 400.0, 400.0));
        // trop haut pour elle (sa tête sortirait), et une fenêtre trop étroite pour ce qui repose
        let high = seen(3, rect(2000.0, 100.0, 800.0, 800.0));
        let slim = seen(4, rect(2400.0, 1200.0, 150.0, 400.0));
        let found = benches(&[front, back, high, slim], screen(), sitting);
        assert_eq!(
            found,
            [
                Bench { window: front, lo: 895.0, hi: 1105.0 },
                Bench { window: back, lo: 295.0, hi: 705.0 },
                Bench { window: back, lo: 1295.0, hi: 1705.0 },
            ]
        );
        assert_eq!(found[1].y(), 1000.0);
        // posée n'importe où sur l'un d'eux, elle y reste ; `landing` l'y poserait aussi
        let windows = [front, back, high, slim];
        for bench in &found {
            for x in [bench.lo, (bench.lo + bench.hi) / 2.0, bench.hi] {
                let perch = bench.at(x);
                assert_eq!(perch.feet, Vec2::new(x, bench.y()));
                assert_eq!(follow(&windows, screen(), sitting, perch.spot), Ok(perch), "{bench:?} en {x}");
                assert_eq!(landing(&windows, screen(), sitting, perch.feet), Some(perch));
            }
        }
        // bornée au morceau, et jamais un point qui n'est pas un nombre
        assert_eq!(found[0].at(0.0).feet.x, 895.0);
        assert_eq!(found[0].at(9999.0).feet.x, 1105.0);
        assert_eq!(found[0].at(f32::NAN).feet.x, 895.0);
    }

    #[test]
    fn she_follows_her_window_when_it_moves_or_is_resized() {
        // posée à 500 pixels du côté gauche d'une fenêtre qui va de 200 à 1800
        let spot = Spot { window: 1, along: 500.0, left: 200.0, width: 1600.0 };
        // immobile : rien ne change
        let still = [seen(1, rect(200.0, 1000.0, 1600.0, 800.0))];
        assert_eq!(
            follow(&still, screen(), capri(), spot),
            Ok(Perch { spot, feet: Vec2::new(700.0, 1000.0) })
        );
        // la fenêtre a glissé de 300 pixels vers la droite et de 100 vers le bas : elle la suit
        let moved = [seen(1, rect(500.0, 1100.0, 1600.0, 800.0))];
        let perch = follow(&moved, screen(), capri(), spot).expect("elle suit");
        assert_eq!(perch.feet, Vec2::new(1000.0, 1100.0));
        assert_eq!(perch.spot, Spot { left: 500.0, ..spot });
        // tirée par la gauche : le bord sous elle n'a pas bougé, elle non plus
        let wider = [seen(1, rect(0.0, 1000.0, 1800.0, 800.0))];
        let perch = follow(&wider, screen(), capri(), spot).expect("elle suit");
        assert_eq!(perch.feet.x, 700.0);
        assert_eq!(perch.spot, Spot { window: 1, along: 700.0, left: 0.0, width: 1800.0 });
        // tirée par la droite, plus étroite qu'elle n'était loin : bornée au bout du bord
        let narrow = [seen(1, rect(200.0, 1000.0, 400.0, 800.0))];
        let perch = follow(&narrow, screen(), capri(), spot).expect("elle suit");
        assert_eq!(perch.feet.x, 600.0 - 36.0);
        assert_eq!(perch.spot.along, 400.0 - 36.0, "sa nouvelle place est retenue");
        // changée des deux côtés (rangée par un gestionnaire de fenêtres) : même proportion
        let tiled = [seen(1, rect(1000.0, 1000.0, 1000.0, 800.0))];
        let perch = follow(&tiled, screen(), capri(), spot).expect("elle suit");
        assert_eq!(perch.feet.x, 1000.0 + 500.0 * 1000.0 / 1600.0);
        // à moitié sortie de l'écran par la gauche : bornée à ce qui reste dans l'écran
        let out = [seen(1, rect(-1000.0, 1000.0, 1600.0, 800.0))];
        let perch = follow(&out, screen(), capri(), spot).expect("elle suit");
        assert_eq!(perch.feet.x, 157.0);
    }

    #[test]
    fn she_leaves_when_her_window_goes_or_her_edge_is_covered_or_has_no_room() {
        let spot = Spot { window: 1, along: 500.0, left: 200.0, width: 1600.0 };
        let w = seen(1, rect(200.0, 1000.0, 1600.0, 800.0));
        assert_eq!(follow(&[seen(2, w.rect)], screen(), capri(), spot), Err(Leave::Gone));
        // une autre fenêtre passe devant, sur ses pieds
        let over = seen(2, rect(600.0, 950.0, 400.0, 400.0));
        assert_eq!(follow(&[over, w], screen(), capri(), spot), Err(Leave::Covered));
        // devant mais à côté de ses pieds : elle reste
        let beside = seen(2, rect(1000.0, 950.0, 400.0, 400.0));
        assert!(follow(&[beside, w], screen(), capri(), spot).is_ok());
        // à moitié sur ses pieds : elle repart aussi
        let half = seen(2, rect(710.0, 950.0, 400.0, 400.0));
        assert_eq!(follow(&[half, w], screen(), capri(), spot), Err(Leave::Covered));
        // la fenêtre passe en plein écran, ou remonte trop haut
        let full = seen(1, rect(0.0, 0.0, 3024.0, 1964.0));
        assert_eq!(follow(&[full], screen(), capri(), spot), Err(Leave::NoRoom));
        let high = seen(1, rect(200.0, 300.0, 1600.0, 800.0));
        assert_eq!(follow(&[high], screen(), capri(), spot), Err(Leave::NoRoom));
        // sortie de l'écran par le bas, ou toute entière sur un autre écran
        let below = seen(1, rect(200.0, 2000.0, 1600.0, 800.0));
        assert_eq!(follow(&[below], screen(), capri(), spot), Err(Leave::NoRoom));
        let away = seen(1, rect(3100.0, 1000.0, 1600.0, 800.0));
        assert_eq!(follow(&[away], screen(), capri(), spot), Err(Leave::NoRoom));
        // une place qui n'est pas un nombre ne la fait pas tomber en panne
        let odd = Spot { along: f32::NAN, ..spot };
        assert_eq!(follow(&[w], screen(), capri(), odd), Err(Leave::Covered));
    }

    #[test]
    fn a_new_size_keeps_her_on_her_edge_when_there_is_room_and_lets_her_go_otherwise() {
        // posée au milieu du bord d'une fenêtre dont le haut est à 800 pixels du haut de l'écran
        let w = seen(1, rect(200.0, 800.0, 1600.0, 800.0));
        let spot = Spot { window: 1, along: 800.0, left: 200.0, width: 1600.0 };
        let at = |height: f32| {
            Stature::standing(Vec2::new(height * 220.0 / 420.0, height), Vec2::new(0.5, 1.0), height)
        };
        // plus petite (40 % : 240 pixels sur cet écran) : elle reste, au même endroit
        let small = follow(&[w], screen(), at(240.0), spot).expect("elle reste");
        assert_eq!(small.feet, Vec2::new(1000.0, 800.0));
        // un peu plus grande (130 % : 780 pixels) : il y a encore la place
        assert!(follow(&[w], screen(), at(780.0), spot).is_ok());
        // la plus grande (140 % : 840 pixels) ne tient plus au-dessus de ce bord : elle repart
        assert_eq!(follow(&[w], screen(), at(840.0), spot), Err(Leave::NoRoom));
        // plus petite, elle se pose là où elle ne tenait pas : un bord à 300 pixels du haut
        let high = seen(2, rect(200.0, 300.0, 1600.0, 800.0));
        assert_eq!(landing(&[high], screen(), capri(), Vec2::new(700.0, 300.0)), None);
        assert!(landing(&[high], screen(), at(240.0), Vec2::new(700.0, 300.0)).is_some());
        // grandie près du bout du bord, ses pieds glissent juste assez pour tenir dans l'écran
        let edge = seen(3, rect(2400.0, 1000.0, 624.0, 800.0));
        let near_end = Spot { window: 3, along: 580.0, left: 2400.0, width: 624.0 };
        let grown = follow(&[edge], screen(), at(780.0), near_end).expect("elle reste");
        assert!(grown.feet.x <= 3024.0 - at(780.0).reach.right + 0.01, "{grown:?}");
    }

    #[test]
    fn a_wider_pose_fits_only_where_she_is_without_moving() {
        // une pose qui s'étend loin à gauche du point posé (allongée), et repose sur 300 px à gauche
        let wide = Stature {
            height: 600.0,
            reach: Reach { left: 400.0, up: 170.0, right: 40.0, down: 130.0 },
            rest: (300.0, 36.0),
        };
        let w = seen(1, rect(200.0, 800.0, 1600.0, 800.0));
        let spot = |x: f32| Spot { window: 1, along: x - 200.0, left: 200.0, width: 1600.0 };
        assert!(fits(&[w], screen(), wide, spot(1000.0)));
        // trop près du bout gauche du bord : ce qui repose n'y tient pas (`follow` la glisserait)
        assert!(!fits(&[w], screen(), wide, spot(450.0)));
        assert!(follow(&[w], screen(), wide, spot(450.0)).is_ok_and(|perch| perch.feet.x > 450.0));
        // ni trop près du bout droit
        assert!(!fits(&[w], screen(), wide, spot(1790.0)));
        // un bord trop près du haut de l'écran : pas la place au-dessus
        let high = seen(2, rect(200.0, 100.0, 1600.0, 800.0));
        let up = Spot { window: 2, along: 800.0, left: 200.0, width: 1600.0 };
        assert!(!fits(&[high], screen(), wide, up));
        // une fenêtre devant, sur ce qui repose à gauche : recouvert
        let front = seen(3, rect(600.0, 600.0, 300.0, 400.0));
        assert!(!fits(&[front, w], screen(), wide, spot(1000.0)));
        assert!(fits(&[front, w], screen(), wide, spot(1300.0)));
        // sa fenêtre partie
        assert!(!fits(&[], screen(), wide, spot(1000.0)));
    }

    /// Guettant, à sa taille d'origine sur cet écran : ce qui se voit d'elle autour de son point
    /// d'accroche, 200 pixels dehors, 390 au-dessus, 240 en dessous.
    fn lurking() -> LurkStature {
        LurkStature {
            height: 600.0,
            left: Reach { left: 200.0, up: 390.0, right: 0.0, down: 240.0 },
            right: Reach { left: 0.0, up: 390.0, right: 200.0, down: 240.0 },
        }
    }

    #[test]
    fn jambs_are_the_visible_pieces_of_window_sides() {
        let back = seen(1, rect(400.0, 300.0, 1600.0, 1200.0));
        // devant, sur le haut du côté gauche de `back`
        let front = seen(2, rect(200.0, 100.0, 600.0, 500.0));
        let found = jambs(&[front, back], screen());
        assert_eq!(
            found,
            [
                Jamb { window: 2, side: Side::Left, x: 200.0, from: 100.0, to: 600.0 },
                Jamb { window: 2, side: Side::Right, x: 800.0, from: 100.0, to: 600.0 },
                Jamb { window: 1, side: Side::Left, x: 400.0, from: 600.0, to: 1500.0 },
                Jamb { window: 1, side: Side::Right, x: 2000.0, from: 300.0, to: 1500.0 },
            ]
        );
        // une fenêtre qui touche le côté sans passer dessus ne le cache pas ; hors de l'écran, rien
        let beside = seen(3, rect(0.0, 0.0, 400.0, 1964.0));
        assert!(jambs(&[beside, back], screen()).contains(&Jamb {
            window: 1,
            side: Side::Left,
            x: 400.0,
            from: 300.0,
            to: 1500.0
        }));
        let wide = seen(4, rect(-100.0, 300.0, 4000.0, 800.0));
        assert!(jambs(&[wide], screen()).is_empty(), "ses deux côtés hors de l'écran");
    }

    #[test]
    fn flanks_need_room_outside_and_along_the_side_away_from_its_corners() {
        // une fenêtre dont le haut touche presque le haut de l'écran : aucun bord ne la porte, assise
        // ou debout, mais ses côtés oui
        let sitting = Stature {
            height: 600.0,
            reach: Reach { left: 99.0, up: 341.0, right: 169.0, down: 164.0 },
            rest: (95.0, 95.0),
        };
        let tall = seen(1, rect(600.0, 60.0, 1600.0, 1800.0));
        assert!(benches(&[tall], screen(), sitting).is_empty());
        let found = flanks(&[tall], screen(), lurking());
        let corner = CORNER * 600.0;
        assert_eq!(
            found,
            [
                Flank {
                    window: tall,
                    side: Side::Left,
                    lo: 60.0 + corner + 390.0,
                    hi: 1860.0 - corner - 240.0
                },
                Flank {
                    window: tall,
                    side: Side::Right,
                    lo: 60.0 + corner + 390.0,
                    hi: 1860.0 - corner - 240.0
                },
            ]
        );
        // guettant n'importe où sur l'un d'eux, elle y reste
        for flank in &found {
            for y in [flank.lo, (flank.lo + flank.hi) / 2.0, flank.hi] {
                let lurk = flank.at(y);
                assert_eq!(lurk.at, Vec2::new(flank.x(), y));
                assert_eq!(
                    follow_flank(&[tall], screen(), lurking(), lurk.spot),
                    Ok(lurk),
                    "{flank:?} en {y}"
                );
            }
        }
        // bornée au morceau, et jamais un point qui n'est pas un nombre
        assert_eq!(found[0].at(0.0).at.y, found[0].lo);
        assert_eq!(found[0].at(f32::NAN).at.y, found[0].lo);
        // trop près du bord gauche de l'écran : ce qui se voit d'elle en sortirait ; à droite, oui
        let near = seen(2, rect(150.0, 60.0, 1600.0, 1800.0));
        assert_eq!(
            flanks(&[near], screen(), lurking()).iter().map(|f| f.side).collect::<Vec<_>>(),
            [Side::Right]
        );
        // trop basse pour elle, loin des coins
        let short = seen(3, rect(600.0, 300.0, 1600.0, 600.0));
        assert!(flanks(&[short], screen(), lurking()).is_empty());
        // une fenêtre devant, sur la bande où elle se voit (sans toucher le côté) : coupé
        let front = seen(4, rect(450.0, 1000.0, 100.0, 100.0));
        let cut = flanks(&[front, tall], screen(), lurking());
        let left: Vec<(f32, f32)> =
            cut.iter().filter(|f| f.window.id == 1 && f.side == Side::Left).map(|f| (f.lo, f.hi)).collect();
        assert_eq!(
            left,
            [(60.0 + corner + 390.0, 1000.0 - 240.0), (1100.0 + 390.0, 1860.0 - corner - 240.0)]
        );
        // une fenêtre devant qui la touche seulement, à côté de la bande : rien ne change
        let away = seen(5, rect(200.0, 900.0, 200.0, 300.0));
        assert_eq!(flanks(&[away, tall], screen(), lurking()), found);
    }

    #[test]
    fn she_follows_her_side_when_it_moves_and_leaves_when_it_goes_is_covered_or_has_no_room() {
        let tall = seen(1, rect(600.0, 60.0, 1600.0, 1800.0));
        let lurk = flanks(&[tall], screen(), lurking())[0].at(1000.0);
        let spot = lurk.spot;
        assert_eq!(spot, FlankSpot { window: 1, side: Side::Left, along: 940.0, top: 60.0, height: 1800.0 });
        // déplacée : elle la suit
        let moved = seen(1, rect(700.0, 100.0, 1600.0, 1800.0));
        assert_eq!(
            follow_flank(&[moved], screen(), lurking(), spot).map(|l| l.at),
            Ok(Vec2::new(700.0, 1040.0))
        );
        // tirée par le haut : la distance au bas est gardée ; par le bas : celle au haut
        let higher = seen(1, rect(600.0, 160.0, 1600.0, 1700.0));
        assert_eq!(follow_flank(&[higher], screen(), lurking(), spot).map(|l| l.at.y), Ok(1000.0));
        let shorter = seen(1, rect(600.0, 60.0, 1600.0, 1500.0));
        assert_eq!(follow_flank(&[shorter], screen(), lurking(), spot).map(|l| l.at.y), Ok(1000.0));
        // raccourcie sous elle : bornée, loin du coin
        let small = seen(1, rect(600.0, 60.0, 1600.0, 1200.0));
        let kept = follow_flank(&[small], screen(), lurking(), spot).expect("elle reste");
        assert_eq!(kept.at.y, 1260.0 - CORNER * 600.0 - 240.0);
        // partie, recouverte (sur le côté, ou là où elle se voit), sortie de l'écran, plus la place
        assert_eq!(follow_flank(&[seen(2, tall.rect)], screen(), lurking(), spot), Err(Leave::Gone));
        let over = seen(2, rect(500.0, 900.0, 300.0, 300.0));
        assert_eq!(follow_flank(&[over, tall], screen(), lurking(), spot), Err(Leave::Covered));
        let outside = seen(2, rect(420.0, 900.0, 100.0, 300.0));
        assert_eq!(follow_flank(&[outside, tall], screen(), lurking(), spot), Err(Leave::Covered));
        let off = seen(1, rect(100.0, 60.0, 1600.0, 1800.0));
        assert_eq!(follow_flank(&[off], screen(), lurking(), spot), Err(Leave::NoRoom));
        let flat = seen(1, rect(600.0, 60.0, 1600.0, 500.0));
        assert_eq!(follow_flank(&[flat], screen(), lurking(), spot), Err(Leave::NoRoom));
        // une place qui n'est pas un nombre ne la fait pas tomber en panne
        let odd = FlankSpot { along: f32::NAN, ..spot };
        assert_eq!(follow_flank(&[tall], screen(), lurking(), odd), Err(Leave::Covered));
    }

    #[test]
    fn the_right_side_follows_the_same_rules_as_the_left() {
        let corner = CORNER * 600.0;
        // le côté droit en x = 2400 : 200 pixels dehors, à droite, jusqu'à 2600
        let tall = seen(1, rect(800.0, 60.0, 1600.0, 1800.0));
        let right = |windows: &[Seen]| -> Vec<(f32, f32)> {
            flanks(windows, screen(), lurking())
                .iter()
                .filter(|f| f.window.id == 1 && f.side == Side::Right)
                .map(|f| (f.lo, f.hi))
                .collect()
        };
        let whole = (60.0 + corner + 390.0, 1860.0 - corner - 240.0);
        assert_eq!(right(&[tall]), [whole]);
        // trop près du bord droit de l'écran : ce qui se voit d'elle en sortirait ; à gauche, oui
        let near = seen(1, rect(1300.0, 60.0, 1600.0, 1800.0));
        assert_eq!(
            flanks(&[near], screen(), lurking()).iter().map(|f| f.side).collect::<Vec<_>>(),
            [Side::Left]
        );
        // une fenêtre devant, sur la bande dehors à droite (sans toucher le côté) : coupé
        let strip = seen(4, rect(2450.0, 1000.0, 100.0, 100.0));
        assert_eq!(right(&[strip, tall]), [(whole.0, 1000.0 - 240.0), (1100.0 + 390.0, whole.1)]);
        // une fenêtre devant sur le côté même : coupé de même
        let line = seen(5, rect(2350.0, 1000.0, 100.0, 100.0));
        assert_eq!(right(&[line, tall]), [(whole.0, 1000.0 - 240.0), (1100.0 + 390.0, whole.1)]);
        // une fenêtre devant qui finit sur le côté, dedans, ou qui commence au bout de la bande :
        // rien ne change
        let inside = seen(6, rect(2300.0, 1000.0, 100.0, 100.0));
        let beyond = seen(7, rect(2600.0, 1000.0, 100.0, 100.0));
        assert_eq!(right(&[inside, tall]), [whole]);
        assert_eq!(right(&[beyond, tall]), [whole]);
        // le suivi : recouverte sur la bande ou sur le côté, sortie de l'écran à droite
        let spot = flanks(&[tall], screen(), lurking())
            .into_iter()
            .find(|f| f.side == Side::Right)
            .unwrap()
            .at(1050.0)
            .spot;
        assert!(follow_flank(&[tall], screen(), lurking(), spot).is_ok());
        assert_eq!(follow_flank(&[strip, tall], screen(), lurking(), spot), Err(Leave::Covered));
        assert_eq!(follow_flank(&[line, tall], screen(), lurking(), spot), Err(Leave::Covered));
        assert!(follow_flank(&[beyond, tall], screen(), lurking(), spot).is_ok());
        assert_eq!(follow_flank(&[near], screen(), lurking(), FlankSpot { ..spot }), Err(Leave::NoRoom));
        // et le haut de l'écran compte de même des deux côtés
        let high = seen(1, rect(800.0, -200.0, 1600.0, 700.0));
        assert!(flanks(&[high], screen(), lurking()).is_empty());
    }

    #[test]
    fn lurking_her_window_is_read_slowly_at_rest_and_at_every_refresh_while_it_moves() {
        // un écran à 120 Hz, un pas de la boucle par rafraîchissement
        let period = 1.0 / 120.0;
        let mut window = seen(1, rect(600.0, 60.0, 1600.0, 1800.0));
        let spot = flanks(&[window], screen(), lurking())[0].at(1000.0).spot;
        let mut watch = Watch::new();
        let (mut reads, mut step) = (0, 0);
        let run = |watch: &mut Watch, window: Seen, seconds: f64, reads: &mut usize, step: &mut u32| {
            let mut last = None;
            let until = *step + (seconds * 120.0) as u32;
            while *step < until {
                let now = f64::from(*step) * period;
                let kept = watch.flank(spot, now, false, period, screen(), lurking(), || {
                    *reads += 1;
                    Some(vec![window])
                });
                if kept != Kept::Waiting {
                    last = Some(kept);
                }
                *step += 1;
            }
            last
        };
        // immobile, 2 s : une lecture tout de suite, puis 4 par seconde
        let last = run(&mut watch, window, 2.0, &mut reads, &mut step);
        assert!((8..=10).contains(&reads), "{reads} lectures");
        assert_eq!(last, Some(Kept::Follows(follow_flank(&[window], screen(), lurking(), spot).unwrap())));
        // elle bouge : à chaque rafraîchissement, et elle la suit
        reads = 0;
        for _ in 0..60 {
            window.rect.min.x += 2.0;
            run(&mut watch, window, 1.0 / 120.0, &mut reads, &mut step);
        }
        assert!(reads >= 55, "{reads} lectures en 60 rafraîchissements");
        let last = run(&mut watch, window, 1.0 / 120.0, &mut reads, &mut step);
        let x = |kept: Option<Kept>| match kept {
            Some(Kept::Follows(lurk)) => lurk.at.x,
            other => panic!("{other:?}"),
        };
        assert_eq!(x(last), window.rect.min.x);
        // immobile de nouveau : après `KEEP_FAST`, lentement
        run(&mut watch, window, KEEP_FAST + 0.1, &mut reads, &mut step);
        reads = 0;
        run(&mut watch, window, 1.0, &mut reads, &mut step);
        assert!((3..=5).contains(&reads), "{reads} lectures en 1 s");
        // sa fenêtre partie, ou la liste illisible : elle ne guette plus
        watch.wake();
        assert_eq!(
            watch.flank(spot, 100.0, false, period, screen(), lurking(), || Some(Vec::new())),
            Kept::Lost(Some(Leave::Gone))
        );
        watch.wake();
        assert_eq!(watch.flank(spot, 101.0, false, period, screen(), lurking(), || None), Kept::Lost(None));
    }

    #[test]
    fn the_journal_says_side_for_a_side_and_edge_for_an_edge() {
        for why in [Leave::Gone, Leave::Covered, Leave::NoRoom] {
            assert!(!why.describe_side().contains("bord"), "{why:?} : {}", why.describe_side());
            assert!(!why.describe().contains("côté"), "{why:?} : {}", why.describe());
        }
        assert!(
            Leave::Covered.describe_side().contains("côté") && Leave::NoRoom.describe_side().contains("côté")
        );
    }

    #[test]
    fn the_watch_reads_slowly_at_rest_and_at_every_refresh_while_her_window_moves() {
        // un écran à 120 Hz
        let period = 1.0 / 120.0;
        let r = rect(200.0, 1000.0, 1600.0, 800.0);
        let mut watch = Watch::new();
        assert!(watch.due(0.0, false, period), "première lecture tout de suite");
        watch.saw(0.0, Some(r));
        assert!(!watch.hurried(0.0, false));
        assert!(!watch.due(0.2, false, period));
        assert!(watch.due(SLOW, false, period), "au repos : 4 fois par seconde");

        // un bouton de la souris enfoncé : pressée, à chaque rafraîchissement
        assert!(watch.hurried(0.01, true));
        assert!(watch.due(0.01, true, period));
        // mais pas deux fois dans la même image (un réveil de plus ne relit pas)…
        watch.saw(0.01, Some(r));
        assert!(!watch.due(0.01 + period * 0.2, true, period));
        // … et un réveil d'écran qui suit de près un réveil en retard ne fait pas sauter l'image
        assert!(watch.due(0.01 + period * 0.5, true, period));

        // la fenêtre bouge : pressée, et encore un moment après le dernier mouvement vu
        let moved = rect(210.0, 1000.0, 1600.0, 800.0);
        watch.saw(1.0, Some(moved));
        assert!(watch.hurried(1.0, false));
        assert!(watch.due(1.0 + period, false, period));
        watch.saw(1.0 + period, Some(moved));
        assert!(watch.hurried(1.0 + KEEP_FAST - 0.01, false), "elle vient de bouger");
        assert!(watch.due(1.0 + period * 2.0, false, period));
        let later = 1.0 + KEEP_FAST;
        watch.saw(later, Some(moved));
        assert!(!watch.hurried(later, false), "immobile depuis un moment : de nouveau au repos");
        assert!(!watch.due(later + period, false, period));
        assert!(watch.due(later + SLOW, false, period));

        // revenue d'être cachée : tout de suite
        watch.wake();
        assert!(watch.due(later + period, false, period));
        // un instant qui n'est pas un nombre ne la fige pas
        watch.saw(2.0, Some(r));
        assert!(watch.due(f64::NAN, false, period));
    }
}
