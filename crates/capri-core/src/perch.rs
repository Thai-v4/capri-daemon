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
//! **« Assise »** veut dire ici : posée debout sur le bord supérieur d'une fenêtre (il n'y a qu'un
//! dessin d'elle, et pas de pose assise). Ce qui se voit est dans `body.rs`.
//!
//! **Seules comptent** les fenêtres ordinaires, à l'écran (le système ne donne que celles-là),
//! opaques, assez grandes, sur son écran, et pas les siennes ([`skip`]) ; et seules les parties
//! visibles de leurs bords supérieurs ([`ledges`]) : une fenêtre devant en cache une derrière.
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

/// Sa taille, et où sont ses pieds dans son image (coordonnées normalisées : (0,5 ; 1) pour des
/// pieds au milieu du bas de l'image). De quoi savoir où elle tient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stature {
    pub size: Vec2,
    pub feet: Vec2,
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
    /// Pour le journal.
    pub fn describe(self) -> &'static str {
        match self {
            Leave::Gone => "sa fenêtre n'est plus à l'écran",
            Leave::Covered => "le bord sous ses pieds est recouvert",
            Leave::NoRoom => "le bord sort de l'écran ou ne lui laisse plus la place",
        }
    }
}

/// On vient de la lâcher, ou elle vient de s'arrêter, les pieds en `feet` : se pose-t-elle ? Sur
/// le bord visible le plus proche de ses pieds, à moins de [`SNAP`], qui lui laisse la place ; le
/// plus en avant à égalité. Ses pieds glissent juste ce qu'il faut pour tenir sur le morceau
/// visible et loin de ses bouts, et elle reste entière dans l'écran.
pub fn landing(seen: &[Seen], screen: Rect, stature: Stature, feet: Vec2) -> Option<Perch> {
    let foot = FOOT * stature.size.y;
    let mut best: Option<(f32, Perch)> = None;
    for (index, window) in seen.iter().enumerate() {
        let y = window.rect.min.y;
        let gap = (y - feet.y).abs();
        let close = gap <= SNAP * stature.size.y;
        if !close {
            continue;
        }
        let Some((lo, hi)) = room(window.rect, screen, stature) else { continue };
        for (from, to) in visible_edge(seen, index, screen) {
            if !(from <= feet.x && feet.x <= to) {
                continue;
            }
            // `clamp` exige lo <= hi : un morceau trop court pour ses pieds ne la porte pas
            let (lo, hi) = (lo.max(from + foot), hi.min(to - foot));
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
    let foot = FOOT * stature.size.y;
    let free =
        visible_edge(seen, index, screen).into_iter().any(|(from, to)| from <= x - foot && x + foot <= to);
    if !free {
        return Err(Leave::Covered);
    }
    Ok(Perch { spot: Spot::on(window, x), feet: Vec2::new(x, window.rect.min.y) })
}

/// Où peut aller le milieu de ses pieds le long du bord supérieur de `window` : sur le bord, à
/// [`FOOT`] de ses bouts, et elle entière dans l'écran. `None` : le bord est hors de l'écran, ou ne
/// lui laisse pas la place.
fn room(window: Rect, screen: Rect, stature: Stature) -> Option<(f32, f32)> {
    let Stature { size, feet } = stature;
    let y = window.min.y;
    let (top, bottom) = (y - size.y * feet.y, y + size.y * (1.0 - feet.y));
    if !(top >= screen.min.y && bottom <= screen.max().y) {
        return None;
    }
    let foot = FOOT * size.y;
    let lo = (window.min.x + foot).max(screen.min.x + size.x * feet.x);
    let hi = (window.max().x - foot).min(screen.max().x - size.x * (1.0 - feet.x));
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
        Stature { size: Vec2::new(314.0, 600.0), feet: Vec2::new(0.5, 1.0) }
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
        let h = capri().size.y;
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
        let foot = FOOT * capri().size.y; // 36 pixels
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
        let offset = Stature { feet: Vec2::new(0.6, 1.0), ..capri() };
        let perch = landing(&[right], screen(), offset, Vec2::new(3000.0, 1000.0)).expect("posée");
        assert!((perch.feet.x - (3024.0 - 314.0 * 0.4)).abs() < 0.01, "{perch:?}");
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
