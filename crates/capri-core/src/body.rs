//! Capri elle-même : où elle est, ce qu'elle fait, et la pose qu'on dessine.
//!
//! Les vitesses et seuils sont exprimés en fractions de sa hauteur, pour qu'elle se comporte
//! pareil sur un écran Retina et sur un écran standard, quelle que soit sa taille.

use std::collections::VecDeque;

use crate::anim::Sheet;
use crate::geom::{Rect, Vec2};
use crate::perch::{Lurk, Perch, Spot};

/// Ce qu'elle est en train de faire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stance {
    /// Elle flotte sur place.
    Floating,
    /// Tenue par la souris ; `grab` est l'écart entre le curseur et son coin haut gauche.
    Held { grab: Vec2 },
    /// Lancée : elle file, ralentit et rebondit sur les bords de l'écran.
    Thrown { vel: Vec2 },
    /// Posée sur le bord supérieur d'une fenêtre (`spot` dit laquelle et où, voir
    /// [`crate::perch`]), le point posé sur le bord en `seat` : elle ne flotte plus, et suit le bord.
    Perched { spot: Spot, seat: Vec2 },
    /// Elle marche, ses pieds vers `to` (en x), à `speed` pixels par seconde, à la cadence de
    /// l'écran ; arrivée, elle flotte de nouveau. Son flottement reste à la hauteur où il était
    /// (`look` arrête son tour le temps de la marche).
    Walking { to: f32, speed: f32 },
    /// Elle tombe du ciel (une intro, voir `crate::intro::act`) : ses pieds descendent de `from` à
    /// `to` (en y), `t` secondes après le début de sa chute, de plus en plus vite
    /// (`act::fall_at`), à la cadence de l'écran ; le haut de l'écran ne la retient pas. Arrivée,
    /// elle flotte de nouveau.
    Falling { from: f32, to: f32, t: f32 },
    /// Elle guette de derrière le côté d'une fenêtre (`lurk` dit laquelle, quel côté, et son point
    /// d'accroche, sur le côté à la hauteur de sa hanche, voir [`crate::perch`]) : elle ne flotte
    /// pas, suit le côté, et sa hanche est placée d'après [`LurkFit`] ; `look` la dessine penchée et
    /// coupée net au bord. `out` dit combien elle est sortie de derrière la fenêtre (0 : rien ne se
    /// voit d'elle ; 1 : penchée dehors), et elle glisse vers `toward` (0 ou 1) à la cadence de
    /// l'écran, en [`GLIDE`] secondes d'un bout à l'autre.
    Lurking { lurk: Lurk, out: f32, toward: f32 },
}

/// Guettant, elle sort de derrière la fenêtre, ou s'y retire, en tant de secondes d'un bout à
/// l'autre (sa position, à la cadence de l'écran, pas ses dessins).
pub const GLIDE: f32 = 0.6;

/// Comment son image se place quand elle guette, d'après ses planches debout (`look` le calcule) :
/// sa hanche dans son image (coordonnées normalisées), et, pour chaque côté
/// ([`crate::perch::Side::index`]),
/// de combien sa hanche est dehors, de l'autre côté du bord que la fenêtre (négatif : derrière la
/// fenêtre), et de combien elle glisse pour être cachée tout entière, en fractions de la largeur de
/// son image (elle grandit et rapetisse avec elle).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LurkFit {
    pub hip: Vec2,
    pub hip_out: [f32; 2],
    pub slide: [f32; 2],
}

/// Sa glissade, adoucie au départ et à l'arrivée (0 et 1 aux bouts, et au milieu à mi-chemin) : de
/// quoi revenir sur ses pas à tout moment sans saut, sa place ne dépendant que de `out`.
fn eased(out: f32) -> f32 {
    let t = out.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ce que le rendu doit dessiner à une image donnée.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Où et à quelle taille dessiner le sprite (pixels entiers : pas de scintillement).
    pub rect: Rect,
    /// Graine du « boil », la légère vibration d'un trait redessiné à la main.
    pub boil_seed: u32,
    /// Décalage de sa copie violette, mal repérée, comme dans l'anime.
    pub ghost: Vec2,
}

/// Flottement au repos : dix positions tenues six images chacune, en fraction de sa hauteur
/// (négatif : plus haut). Pas une sinusoïde qui glisse : des poses clés, comme en anime.
///
/// Un tour dure 60 images (2,5 s), comme la boucle de sa pose debout (`sheets::stand`), dont les
/// cheveux montent et retombent sans qu'elle bouge. Les deux tournent sur le même tour
/// ([`Capri::tour`]) : elle est au plus bas (images 24 à 29 du tour) quand ses cheveux sont le plus
/// bas (dessins 24 à 30, poussés vers le bas quand elle remonte), et au plus haut (images 54 à 59)
/// quand ils sont relevés (dessins 54 à 8). Le tour s'arrête le temps qu'elle apparaisse ou
/// s'efface : elle ne flotte pas pendant ce temps-là.
const FLOAT: [(f32, u32); 10] = [
    (-0.0172, 6),
    (-0.0124, 6),
    (-0.0066, 6),
    (-0.0018, 6),
    (0.0, 6),
    (-0.0018, 6),
    (-0.0066, 6),
    (-0.0124, 6),
    (-0.0172, 6),
    (-0.019, 6),
];

/// Combien de dessins de « boil » tournent (un nouveau toutes les deux images).
const BOIL_DRAWINGS: u64 = 3;
/// En dessous de cette vitesse au lâcher (hauteurs par seconde), elle reste où on la pose.
const THROW_MIN: f32 = 1.5;
/// En dessous de cette vitesse, un lancer s'arrête.
const STOP_BELOW: f32 = 0.25;
/// Freinage de l'air (par seconde) : elle flotte, elle ne tombe pas.
const AIR_DRAG: f32 = 2.2;
/// Ce qu'elle garde de sa vitesse en rebondissant.
const BOUNCE: f32 = 0.6;
/// Durée de l'écrasement à l'impact, en images.
const SQUASH_FRAMES: u64 = 4;
/// Fenêtre de temps (s) pour mesurer la vitesse du lancer.
const VELOCITY_WINDOW: f64 = 0.08;
/// Tenue, elle glisse vers l'écart voulu ([`Capri::hold_toward`]) de cette part du chemin du
/// curseur à chaque pas, au plus : elle le suit toujours d'au moins l'autre moitié.
const TOWARD: f32 = 0.5;
/// Lâchée en partie hors de l'écran (une intro l'y avait mise), elle y revient à cette vitesse, en
/// hauteurs par seconde, sans saut.
pub const RETURN: f32 = 2.5;
/// Elle se pose sur un bord : écrasée sur ses pieds (3 images), un rebond étiré (2 images), puis
/// immobile. Échelles (largeur, hauteur), tenues un nombre entier d'images, ancrées à ses pieds.
const LANDING: [((f32, f32), u32); 2] = [((1.14, 0.84), 3), ((0.95, 1.06), 2)];
/// Elle quitte un bord : étirée vers le haut (2 images), puis elle flotte de nouveau.
const LIFTING: [((f32, f32), u32); 1] = [((0.93, 1.08), 2)];

/// Son tour : celui de son flottement, que partage la boucle de sa pose debout (`look`). Il peut
/// s'arrêter un moment, puis reprendre là où il s'était arrêté : à l'image `frame`, il en est à
/// `at + (frame - from)`, et reste à `at` avant `from`. Sans arrêt, il suit l'image d'animation.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tour {
    at: u64,
    from: u64,
}

impl Tour {
    fn at(self, frame: u64) -> u64 {
        self.at + frame.saturating_sub(self.from)
    }
}

/// Un geste joué une fois, à partir d'une image donnée.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    Landing,
    Lifting,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug)]
pub struct Capri {
    pos: Vec2,
    size: Vec2,
    bounds: Rect,
    stance: Stance,
    float: Sheet<f32>,
    /// Image d'animation courante, mise à jour par [`Capri::step`].
    frame: u64,
    squash: Option<(u64, Axis)>,
    /// Le geste en cours (se poser, repartir) et l'image où il a commencé.
    gesture: Option<(u64, Gesture)>,
    /// Où sont ses pieds dans son image (coordonnées normalisées).
    feet: Vec2,
    samples: VecDeque<(f64, Vec2)>,
    /// Combien d'images dure son départ d'un bord avant qu'elle flotte de nouveau, debout (le
    /// temps que s'efface sa pose assise, s'il y en a une ; 0 sinon).
    leave_frames: u64,
    /// Son tour, qui s'arrête le temps qu'elle apparaisse ou s'efface ([`Capri::still`]).
    tour: Tour,
    /// Tenue : l'écart entre le curseur et son coin haut gauche vers lequel elle glisse à mesure
    /// que le curseur bouge ([`Capri::hold_toward`]) ; aucun, elle garde l'écart de la prise.
    toward: Option<Vec2>,
    /// Guettant : comment son image se place ([`Capri::lurk_on`]).
    fit: LurkFit,
}

impl Capri {
    /// `size` : taille affichée ; `bounds` : zone où elle peut aller ; `pos` : coin haut gauche.
    pub fn new(size: Vec2, bounds: Rect, pos: Vec2) -> Self {
        let mut capri = Self {
            pos,
            size,
            bounds,
            stance: Stance::Floating,
            float: Sheet::new(&FLOAT),
            frame: 0,
            squash: None,
            gesture: None,
            feet: Vec2::new(0.5, 1.0),
            samples: VecDeque::new(),
            leave_frames: 0,
            tour: Tour { at: 0, from: 0 },
            toward: None,
            fit: LurkFit::default(),
        };
        capri.clamp();
        capri
    }

    pub fn stance(&self) -> Stance {
        self.stance
    }

    pub fn is_held(&self) -> bool {
        matches!(self.stance, Stance::Held { .. })
    }

    pub fn position(&self) -> Vec2 {
        self.pos
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// La zone où elle peut aller (l'écran).
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Où sont ses pieds dans son image (coordonnées normalisées, voir `AlphaMask::feet`). Par
    /// défaut : au milieu du bas de l'image.
    pub fn set_feet(&mut self, feet: Vec2) {
        self.feet = feet;
    }

    /// Le milieu de ses pieds, là où elle touche un bord ; posée, le point posé sur le bord.
    pub fn feet(&self) -> Vec2 {
        match self.stance {
            Stance::Perched { seat, .. } => seat,
            _ => self.standing_feet(),
        }
    }

    /// Le milieu des pieds de son image debout, où qu'elle soit.
    fn standing_feet(&self) -> Vec2 {
        self.pos + Vec2::new(self.size.x * self.feet.x, self.size.y * self.feet.y)
    }

    /// Où elle est posée, si elle l'est.
    pub fn perched(&self) -> Option<Spot> {
        match self.stance {
            Stance::Perched { spot, .. } => Some(spot),
            _ => None,
        }
    }

    /// Posée, le point posé sur le bord (ses pieds debout, ou sa ligne d'assise).
    pub fn seat(&self) -> Option<Vec2> {
        match self.stance {
            Stance::Perched { seat, .. } => Some(seat),
            _ => None,
        }
    }

    /// Où elle guette, si elle guette.
    pub fn lurking(&self) -> Option<Lurk> {
        match self.stance {
            Stance::Lurking { lurk, .. } => Some(lurk),
            _ => None,
        }
    }

    /// Guettant, combien elle est sortie de derrière la fenêtre (0 : rien ne se voit d'elle ; 1 :
    /// penchée dehors).
    pub fn lurk_out(&self) -> Option<f32> {
        match self.stance {
            Stance::Lurking { out, .. } => Some(out),
            _ => None,
        }
    }

    /// Guettant, glisse-t-elle (sort-elle de derrière la fenêtre, ou s'y retire-t-elle) ? Sa position
    /// change alors à chaque rafraîchissement de l'écran.
    pub fn is_gliding(&self) -> bool {
        matches!(self.stance, Stance::Lurking { out, toward, .. } if out != toward)
    }

    /// Son corps suit-il quelque chose à la cadence de l'écran (voir `crate::pace`) : tenue,
    /// lancée, en marche, en chute, ou en train de glisser de derrière une fenêtre ? Guettant
    /// immobile, posée, flottant : non (sa fenêtre qui bouge, c'est la veille de `perch` qui le dit).
    pub fn follows_screen(&self) -> bool {
        match self.stance {
            Stance::Held { .. } | Stance::Thrown { .. } | Stance::Walking { .. } | Stance::Falling { .. } => {
                true
            }
            Stance::Lurking { .. } => self.is_gliding(),
            Stance::Floating | Stance::Perched { .. } => false,
        }
    }

    /// Comment son image se place quand elle guette (la dernière donnée par [`Capri::lurk_on`]).
    pub fn lurk_fit(&self) -> LurkFit {
        self.fit
    }

    /// Combien d'images dure son départ d'un bord, avant qu'elle flotte de nouveau, debout.
    pub fn set_leave_frames(&mut self, frames: u64) {
        self.leave_frames = frames;
    }

    /// Où en est son tour à l'image courante (60 images par tour) : son flottement, et la boucle
    /// de sa pose debout que `look` dessine, qui partagent ce tour.
    pub fn tour(&self) -> u64 {
        self.tour.at(self.frame)
    }

    /// Elle cesse de flotter, à la hauteur où elle est, jusqu'à l'image `until` (`None` : jusqu'à
    /// ce qu'on la reprenne) ; puis son tour reprend là où il s'était arrêté, sans saut. Appelée
    /// alors qu'elle ne flotte déjà plus, elle garde cette hauteur-là et change seulement la fin.
    /// Un geste qui attendait son tour (l'étirement d'après un départ de son bord) est oublié : elle
    /// apparaît ou s'efface à sa place.
    pub fn still(&mut self, until: Option<u64>) {
        let at = self.tour();
        self.tour = Tour { at, from: until.unwrap_or(u64::MAX).max(self.frame) };
        if self.gesture.is_some_and(|(start, _)| start > self.frame) {
            self.gesture = None;
        }
    }

    /// Son tour reprend tout de suite, là où il s'était arrêté.
    pub fn unstill(&mut self) {
        self.still(Some(self.frame));
    }

    /// Elle se pose sur un bord, en `perch.feet`, avec un écrasement à l'atterrissage.
    pub fn land(&mut self, perch: Perch) {
        self.place_feet(perch.feet);
        self.stance = Stance::Perched { spot: perch.spot, seat: perch.feet };
        self.squash = None;
        self.gesture = Some((self.frame, Gesture::Landing));
    }

    /// Posée, elle suit son bord : en `perch.feet`, sans nouvel atterrissage. Sans effet si elle
    /// n'est pas posée.
    pub fn ride(&mut self, perch: Perch) {
        if let Stance::Perched { .. } = self.stance {
            self.place_feet(perch.feet);
            self.stance = Stance::Perched { spot: perch.spot, seat: perch.feet };
        }
    }

    /// Elle se retrouve posée sur un bord, en `perch.feet`, d'un coup et sans geste : elle y
    /// change de place en s'y matérialisant (`crate::roam`), elle n'y atterrit pas.
    pub fn set_on(&mut self, perch: Perch) {
        self.set_at(perch.feet);
        self.stance = Stance::Perched { spot: perch.spot, seat: perch.feet };
    }

    /// Elle se retrouve derrière le côté d'une fenêtre, en `lurk`, d'un coup et sans geste, cachée
    /// tout entière derrière (rien ne se voit d'elle) ; `fit` : comment son image s'y place (`look`).
    /// Pour qu'elle en sorte : [`Capri::emerge`].
    pub fn lurk_on(&mut self, lurk: Lurk, fit: LurkFit) {
        self.fit = fit;
        self.stance = Stance::Lurking { lurk, out: 0.0, toward: 0.0 };
        self.squash = None;
        self.gesture = None;
        self.samples.clear();
        self.place_lurking();
    }

    /// Guettant, elle suit le côté de sa fenêtre : en `lurk`, sans rien changer à sa glissade. Sans
    /// effet si elle ne guette pas.
    pub fn ride_flank(&mut self, lurk: Lurk) {
        if let Stance::Lurking { out, toward, .. } = self.stance {
            self.stance = Stance::Lurking { lurk, out, toward };
            self.place_lurking();
        }
    }

    /// Guettant, elle sort de derrière la fenêtre (vers sa pose penchée), d'où elle en est. Sans
    /// effet si elle ne guette pas.
    pub fn emerge(&mut self) {
        if let Stance::Lurking { lurk, out, .. } = self.stance {
            self.stance = Stance::Lurking { lurk, out, toward: 1.0 };
        }
    }

    /// Guettant, elle se retire derrière la fenêtre (jusqu'à ce que rien ne se voie d'elle), d'où
    /// elle en est. Sans effet si elle ne guette pas.
    pub fn withdraw(&mut self) {
        if let Stance::Lurking { lurk, out, .. } = self.stance {
            self.stance = Stance::Lurking { lurk, out, toward: 0.0 };
        }
    }

    /// Guettant, sa place : sa hanche à la hauteur de son point d'accroche, dehors de ce que dit
    /// [`LurkFit`] une fois sortie, et d'autant plus vers l'intérieur qu'elle est cachée. Son image
    /// n'est pas ramenée dans l'écran : ce qui se voit d'elle y est (le côté choisi le garantit),
    /// ce qui est derrière la fenêtre n'est pas dessiné.
    fn place_lurking(&mut self) {
        let Stance::Lurking { lurk, out, .. } = self.stance else { return };
        let (fit, side) = (self.fit, lurk.spot.side);
        let i = side.index();
        let dx = (fit.hip_out[i] - (1.0 - eased(out)) * fit.slide[i]) * self.size.x;
        let hip = Vec2::new(lurk.at.x + side.outward() * dx, lurk.at.y);
        self.pos = hip - Vec2::new(self.size.x * fit.hip.x, self.size.y * fit.hip.y);
    }

    /// Elle se retrouve debout, ses pieds en `feet`, d'un coup et sans geste ; elle y flotte.
    pub fn set_at(&mut self, feet: Vec2) {
        // debout d'abord : guettant, son image n'est pas ramenée dans l'écran
        self.stance = Stance::Floating;
        self.place_feet(feet);
        self.squash = None;
        self.gesture = None;
        self.samples.clear();
    }

    /// Elle marche jusqu'à ce que ses pieds soient en x = `to`, à `speed` pixels par seconde, à la
    /// hauteur où elle est. Seulement si elle flotte : rend `false` sinon, et rien ne change.
    pub fn walk_to(&mut self, to: f32, speed: f32) -> bool {
        if self.stance != Stance::Floating || speed.is_nan() || speed <= 0.0 || !to.is_finite() {
            return false;
        }
        self.stance = Stance::Walking { to, speed };
        self.squash = None;
        self.gesture = None;
        self.samples.clear();
        true
    }

    /// Marche-t-elle ?
    pub fn is_walking(&self) -> bool {
        matches!(self.stance, Stance::Walking { .. })
    }

    /// Elle se retrouve debout, ses pieds en `feet`, d'un coup et sans geste, **même hors de
    /// l'écran** (une intro l'y fait entrer, voir `crate::intro::act`) ; elle y flotte. Prise en
    /// partie dehors, elle ne saute pas : elle suit le curseur, l'écran l'empêchant seulement d'en
    /// sortir davantage ; lâchée encore en partie dehors, elle y revient d'elle-même ([`RETURN`]).
    /// Une nouvelle taille, un autre écran l'y ramènent d'un coup.
    pub fn set_beyond(&mut self, feet: Vec2) {
        self.pos = feet - Vec2::new(self.size.x * self.feet.x, self.size.y * self.feet.y);
        self.stance = Stance::Floating;
        self.squash = None;
        self.gesture = None;
        self.samples.clear();
    }

    /// Elle tombe du ciel : ses pieds partent de `from` (hors de l'écran, au-dessus) et tombent
    /// jusqu'en y = `to` (`crate::intro::act::fall_at`). Seulement si elle flotte : rend `false`
    /// sinon, et rien ne change.
    pub fn fall(&mut self, from: Vec2, to: f32) -> bool {
        if self.stance != Stance::Floating || !from.x.is_finite() || !from.y.is_finite() || !to.is_finite() {
            return false;
        }
        self.set_beyond(from);
        self.stance = Stance::Falling { from: from.y, to, t: 0.0 };
        true
    }

    /// Tombe-t-elle ?
    pub fn is_falling(&self) -> bool {
        matches!(self.stance, Stance::Falling { .. })
    }

    /// Elle s'arrête là où elle est, et flotte. Sans effet si elle ne marche pas.
    pub fn halt(&mut self) {
        if self.is_walking() {
            self.stance = Stance::Floating;
        }
    }

    /// Elle quitte son bord et se remet à flotter sur place, avec un petit étirement une fois son
    /// départ joué ([`Capri::set_leave_frames`]). Guettant, elle ne guette plus : debout, entière et
    /// dans l'écran, là où est son image, sans geste. Sans effet si elle n'est ni posée ni ne guette.
    pub fn take_off(&mut self) {
        match self.stance {
            Stance::Perched { seat, .. } => {
                self.place_feet(seat);
                self.stance = Stance::Floating;
                self.gesture = Some((self.frame + self.leave_frames, Gesture::Lifting));
            }
            Stance::Lurking { .. } => {
                self.stance = Stance::Floating;
                self.clamp();
            }
            _ => {}
        }
    }

    /// Nouvelle zone (écran redimensionné, changement d'écran) : elle reste dedans.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.clamp();
    }

    /// Nouvelle taille, en gardant ses pieds au même endroit. En marche, elle s'arrête là : sa
    /// vitesse était celle de son ancienne taille ; en train de tomber, de même, ramenée dans
    /// l'écran. La même taille ne change rien.
    pub fn set_size(&mut self, size: Vec2) {
        // la même taille n'est pas une nouvelle taille : rien ne s'arrête
        if size == self.size {
            return;
        }
        // guettant, sa hanche reste sur son point d'accroche
        if self.lurking().is_some() {
            self.size = size;
            return self.place_lurking();
        }
        let feet = self.feet();
        self.size = size;
        self.place_feet(feet);
        self.halt();
        if self.is_falling() {
            self.stance = Stance::Floating;
        }
    }

    /// On l'attrape avec le curseur en `cursor` à l'instant `t` (secondes), debout, son image
    /// debout replacée d'abord en `origin` (coin haut gauche) : on l'attrape assise, elle se lève
    /// sous le curseur, ramenée dans l'écran ; guettant, elle se redresse sous lui, **exactement**
    /// là, même en partie hors de l'écran (ce que tient le curseur reste sous lui) : comme une prise
    /// ordinaire en partie dehors, l'écran l'empêche ensuite seulement d'en sortir davantage, et,
    /// lâchée, elle y revient d'elle-même ([`RETURN`]).
    pub fn grab_at(&mut self, cursor: Vec2, origin: Vec2, t: f64) {
        let lurking = self.lurking().is_some();
        self.pos = origin;
        self.stance = Stance::Floating;
        if !lurking {
            self.clamp();
        }
        self.grab(cursor, t);
    }

    /// On l'attrape avec le curseur en `cursor` à l'instant `t` (secondes), là où elle est : son
    /// corps ne bouge pas. Si son tour était arrêté (elle apparaissait), il reprend ; si elle
    /// rebondissait, elle n'est plus écrasée.
    pub fn grab(&mut self, cursor: Vec2, t: f64) {
        self.unstill();
        self.stance = Stance::Held { grab: cursor - self.pos };
        self.squash = None;
        self.gesture = None;
        self.toward = None;
        self.samples.clear();
        self.samples.push_back((t, cursor));
    }

    /// Tenue, elle glisse vers l'écart `offset` entre le curseur et son coin haut gauche (sa mèche
    /// sous le curseur), à mesure que le curseur bouge : à chaque pas, l'écart se réduit au plus de
    /// la moitié du chemin du curseur ([`TOWARD`]), et elle le suit toujours de l'autre moitié au
    /// moins ; curseur immobile, elle ne bouge pas. Et le bord de l'écran ne la retient jamais
    /// au-delà : dès que le curseur s'en éloigne, elle le suit. Sans effet si on ne la tient pas.
    pub fn hold_toward(&mut self, offset: Vec2) {
        if self.is_held() {
            self.toward = Some(offset);
        }
    }

    /// Le curseur bouge pendant qu'on la tient. Prise en partie hors de l'écran (une intro),
    /// l'écran l'empêche seulement d'en sortir davantage ; entièrement dedans, il l'y retient.
    pub fn drag_to(&mut self, cursor: Vec2, t: f64) {
        let Stance::Held { mut grab } = self.stance else { return };
        if let (Some(toward), Some(&(_, last))) = (self.toward, self.samples.back()) {
            let gap = toward - grab;
            let (room, length) = ((cursor - last).length() * TOWARD, gap.length());
            grab += if length <= room { gap } else { gap * (room / length) };
        }
        let before = self.pos;
        self.pos = cursor - grab;
        // prise en partie hors de l'écran (une intro) : l'écran l'empêche seulement d'en sortir
        // davantage ; entièrement dedans, il l'y retient comme toujours
        self.contain(before);
        if self.toward.is_some() {
            // le bord l'arrête : l'écart suit, pour qu'elle reparte dès que le curseur s'en éloigne
            grab = cursor - self.pos;
        }
        self.stance = Stance::Held { grab };
        self.samples.push_back((t, cursor));
        while self.samples.len() > 2 && self.samples[1].0 < t - VELOCITY_WINDOW {
            self.samples.pop_front();
        }
    }

    /// On la lâche à l'instant `t` : assez vite, c'est un lancer ; sinon elle reste là.
    pub fn release(&mut self, t: f64) {
        if !self.is_held() {
            return;
        }
        let vel = match (self.samples.front(), self.samples.back()) {
            (Some(&(t0, p0)), Some(&(t1, p1))) if t1 > t0 && t - t1 < VELOCITY_WINDOW => {
                (p1 - p0) * (1.0 / (t1 - t0) as f32)
            }
            _ => Vec2::ZERO,
        };
        self.samples.clear();
        self.stance =
            if vel.length() > THROW_MIN * self.size.y { Stance::Thrown { vel } } else { Stance::Floating };
    }

    /// Avance la simulation de `dt` secondes, à l'image d'animation `frame`.
    pub fn step(&mut self, dt: f32, frame: u64) {
        self.frame = frame;
        if let Some((until, _)) = self.squash
            && frame >= until
        {
            self.squash = None;
        }
        // un geste fini est oublié ; un geste à venir (après un départ) attend son tour
        if self.gesture.is_some_and(|(start, _)| frame >= start) && self.gesture_scale().is_none() {
            self.gesture = None;
        }
        if let Stance::Walking { to, speed } = self.stance {
            return self.walk(to, speed * dt.max(0.0));
        }
        if let Stance::Lurking { lurk, out, toward } = self.stance {
            // un pas qui n'est pas un nombre ne la fait pas glisser
            let room = if dt > 0.0 { dt / GLIDE } else { 0.0 };
            let out = if out < toward { (out + room).min(toward) } else { (out - room).max(toward) };
            self.stance = Stance::Lurking { lurk, out, toward };
            return self.place_lurking();
        }
        if let Stance::Falling { from, to, t } = self.stance {
            return self.drop_by(from, to, t + dt.max(0.0));
        }
        if self.stance == Stance::Floating {
            return self.come_in(dt.max(0.0));
        }
        let Stance::Thrown { mut vel } = self.stance else { return };

        let before = self.pos;
        self.pos += vel * dt;
        vel = vel * (-AIR_DRAG * dt).exp();

        // elle rebondit sur un bord de l'écran ; lancée d'en partie dehors (une intro), seulement
        // si elle allait en sortir davantage
        let (lo, hi) = self.reach(before);
        if self.pos.x < lo.x || self.pos.x > hi.x {
            self.pos.x = self.pos.x.clamp(lo.x, hi.x);
            vel.x = -vel.x * BOUNCE;
            self.squash = Some((frame + SQUASH_FRAMES, Axis::Horizontal));
        }
        if self.pos.y < lo.y || self.pos.y > hi.y {
            self.pos.y = self.pos.y.clamp(lo.y, hi.y);
            vel.y = -vel.y * BOUNCE;
            self.squash = Some((frame + SQUASH_FRAMES, Axis::Vertical));
        }

        self.stance =
            if vel.length() < STOP_BELOW * self.size.y { Stance::Floating } else { Stance::Thrown { vel } };
    }

    /// Un pas de sa marche : ses pieds avancent de `step` pixels vers `to`, sans le dépasser ; elle
    /// flotte de nouveau une fois arrivée, ou si le bord de l'écran vers lequel elle va l'arrête.
    /// Partie de hors de l'écran (une intro), elle y entre : seul ce bord-là la retient.
    fn walk(&mut self, to: f32, step: f32) {
        let gap = to - self.standing_feet().x;
        let before = self.pos.x;
        if gap.abs() <= step {
            self.pos.x += gap;
        } else {
            self.pos.x += step * gap.signum();
        }
        let (min, max) = (self.bounds.min.x, (self.bounds.max().x - self.size.x).max(self.bounds.min.x));
        if gap < 0.0 {
            self.pos.x = self.pos.x.max(min);
        } else {
            self.pos.x = self.pos.x.min(max);
        }
        let moved = (self.pos.x - before).abs();
        if gap.abs() <= step || moved + 1e-3 < step.min(gap.abs()) {
            self.stance = Stance::Floating;
        }
    }

    /// Un pas de sa chute : ses pieds là où `crate::intro::act::fall_at` les met `t` secondes
    /// après son début ; arrivés, elle flotte de nouveau, dans l'écran.
    fn drop_by(&mut self, from: f32, to: f32, t: f32) {
        let y = crate::intro::act::fall_at(from, to, t);
        self.pos.y = y - self.size.y * self.feet.y;
        if y >= to {
            self.stance = Stance::Floating;
            self.clamp();
        } else {
            self.stance = Stance::Falling { from, to, t };
        }
    }

    /// La pose à dessiner pour l'image courante. Tant qu'elle flotte, la pose ne change
    /// qu'avec les dessins : inutile de redessiner entre deux.
    pub fn pose(&self) -> Pose {
        let frame = self.frame;
        let mut offset = Vec2::ZERO;
        let mut scale = Vec2::new(1.0, 1.0);
        let unit = self.size.y / 420.0; // le sprite d'origine fait 420 px de haut
        let mut ghost = Vec2::new(4.0, 3.0) * unit;

        match self.stance {
            // en marche, son tour est arrêté : elle garde la hauteur de son flottement
            Stance::Floating | Stance::Walking { .. } => offset.y = self.float.at(self.tour()) * self.size.y,
            // posée, guettant, ou en train de tomber : sans flotter, seul son trait continue de bouillir
            Stance::Held { .. }
            | Stance::Perched { .. }
            | Stance::Falling { .. }
            | Stance::Lurking { .. } => {}
            Stance::Thrown { vel } => {
                // étirée dans le sens de la course, sa copie violette traîne derrière
                let speed = vel.length() / self.size.y;
                let stretch = (speed / 20.0).min(0.15);
                if vel.x.abs() > vel.y.abs() {
                    scale = Vec2::new(1.0 + stretch, 1.0 - stretch * 0.6);
                } else {
                    scale = Vec2::new(1.0 - stretch * 0.6, 1.0 + stretch);
                }
                let dir = vel * (1.0 / vel.length().max(1.0));
                ghost += dir * (-18.0 * unit * stretch / 0.15);
            }
        }
        if let Some((_, axis)) = self.squash {
            scale = match axis {
                Axis::Horizontal => Vec2::new(0.84, 1.1),
                Axis::Vertical => Vec2::new(1.12, 0.86),
            };
        }
        // se poser ou repartir : déformée autour de ses pieds, qui ne bougent pas
        let gesture = self.gesture_scale();
        if let Some(gesture) = gesture {
            scale = gesture;
        }

        let size = Vec2::new(self.size.x * scale.x, self.size.y * scale.y);
        let min = if gesture.is_some() {
            self.standing_feet() - Vec2::new(size.x * self.feet.x, size.y * self.feet.y) + offset
        } else {
            self.pos + self.size * 0.5 + offset - size * 0.5
        };
        Pose {
            rect: Rect::new(
                Vec2::new(min.x.round(), min.y.round()),
                Vec2::new(size.x.round(), size.y.round()),
            ),
            boil_seed: ((frame / 2) % BOIL_DRAWINGS) as u32,
            ghost: Vec2::new(ghost.x.round(), ghost.y.round()),
        }
    }

    /// L'échelle du geste en cours à l'image courante, s'il n'est pas fini.
    fn gesture_scale(&self) -> Option<Vec2> {
        let (start, gesture) = self.gesture?;
        let keys: &[((f32, f32), u32)] = match gesture {
            Gesture::Landing => &LANDING,
            Gesture::Lifting => &LIFTING,
        };
        let mut t = self.frame.checked_sub(start)?;
        for &((x, y), hold) in keys {
            if t < u64::from(hold) {
                return Some(Vec2::new(x, y));
            }
            t -= u64::from(hold);
        }
        None
    }

    fn place_feet(&mut self, feet: Vec2) {
        self.pos = feet - Vec2::new(self.size.x * self.feet.x, self.size.y * self.feet.y);
        self.clamp();
    }

    /// Où son coin haut gauche peut aller, venant de `before` : dans l'écran, ou, si elle en était
    /// en partie dehors, pas plus dehors qu'elle ne l'était (une intro l'y avait mise).
    fn reach(&self, before: Vec2) -> (Vec2, Vec2) {
        let max = self.bounds.max() - self.size;
        let max = Vec2::new(max.x.max(self.bounds.min.x), max.y.max(self.bounds.min.y));
        let lo = Vec2::new(self.bounds.min.x.min(before.x), self.bounds.min.y.min(before.y));
        let hi = Vec2::new(max.x.max(before.x), max.y.max(before.y));
        (lo, hi)
    }

    /// Ramenée là où elle peut aller, venant de `before` (voir [`Capri::reach`]).
    fn contain(&mut self, before: Vec2) {
        let (lo, hi) = self.reach(before);
        self.pos = Vec2::new(self.pos.x.clamp(lo.x, hi.x), self.pos.y.clamp(lo.y, hi.y));
    }

    /// Lâchée en partie hors de l'écran (une intro l'y avait mise), elle y revient d'elle-même, à
    /// [`RETURN`] hauteurs par seconde, sans saut ; dans l'écran, rien ne change.
    fn come_in(&mut self, dt: f32) {
        let max = self.bounds.max() - self.size;
        let inside = Vec2::new(
            self.pos.x.clamp(self.bounds.min.x, max.x.max(self.bounds.min.x)),
            self.pos.y.clamp(self.bounds.min.y, max.y.max(self.bounds.min.y)),
        );
        let gap = inside - self.pos;
        let room = RETURN * self.size.y * dt;
        let length = gap.length();
        self.pos += if length <= room { gap } else { gap * (room / length) };
    }

    fn clamp(&mut self) {
        // guettant, sa place est celle de son point d'accroche (voir `place_lurking`)
        if self.lurking().is_some() {
            return;
        }
        let max = self.bounds.max() - self.size;
        self.pos.x = self.pos.x.clamp(self.bounds.min.x, max.x.max(self.bounds.min.x));
        self.pos.y = self.pos.y.clamp(self.bounds.min.y, max.y.max(self.bounds.min.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capri() -> Capri {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(1920.0, 1080.0));
        Capri::new(Vec2::new(160.0, 300.0), screen, Vec2::new(800.0, 400.0))
    }

    /// Guettant : sa hanche au milieu de son image, 20 % de sa largeur derrière la fenêtre une fois
    /// sortie, et une glissade de 70 % de sa largeur, des deux côtés.
    const FIT: LurkFit = LurkFit { hip: Vec2::new(0.5, 0.6), hip_out: [-0.2, -0.2], slide: [0.7, 0.7] };

    /// Sur le côté `side` d'une fenêtre dont le bord est en x = 1000, son point d'accroche en y = 500.
    fn lurk(side: crate::perch::Side) -> Lurk {
        let spot = crate::perch::FlankSpot { window: 3, side, along: 300.0, top: 200.0, height: 800.0 };
        Lurk { spot, at: Vec2::new(1000.0, 500.0) }
    }

    /// Sa hanche, d'après son image.
    fn hip(c: &Capri) -> Vec2 {
        c.position() + Vec2::new(c.size().x * FIT.hip.x, c.size().y * FIT.hip.y)
    }

    #[test]
    fn lurking_she_glides_out_from_behind_the_window_and_back_at_the_pace_of_the_screen() {
        use crate::perch::Side;
        for side in Side::ALL {
            let mut c = capri();
            c.lurk_on(lurk(side), FIT);
            assert_eq!(c.lurk_out(), Some(0.0));
            assert!(!c.is_gliding(), "elle attend qu'on la fasse sortir");
            // cachée : sa hanche à 90 % de sa largeur derrière le bord
            let o = side.outward();
            assert!((hip(&c) - Vec2::new(1000.0 - o * 0.9 * 160.0, 500.0)).length() < 1e-3, "{side:?}");
            assert_eq!(c.pose().rect.min, Vec2::new(c.position().x.round(), c.position().y.round()));
            // elle sort, à pas de 120 Hz : sans à-coup, toujours vers l'extérieur, en GLIDE secondes
            c.emerge();
            let mut last = hip(&c).x;
            let mut steps = 0;
            while c.is_gliding() {
                steps += 1;
                c.step(1.0 / 120.0, steps / 5);
                let x = hip(&c).x;
                assert!((x - last) * o >= 0.0, "{side:?} : elle ne recule pas");
                assert!((x - last).abs() <= 1.5 * 0.7 * 160.0 / GLIDE / 120.0 + 1e-3, "{side:?} : un saut");
                last = x;
                assert!(steps < 200);
            }
            assert!((steps as f32 / 120.0 - GLIDE).abs() < 0.02, "{side:?} : {steps}");
            assert_eq!(c.lurk_out(), Some(1.0));
            assert!((hip(&c) - Vec2::new(1000.0 - o * 0.2 * 160.0, 500.0)).length() < 1e-3);
            // elle ne flotte pas, et suit son côté
            c.step(1.0, 600);
            assert!((hip(&c) - Vec2::new(1000.0 - o * 0.2 * 160.0, 500.0)).length() < 1e-3);
            let moved = Lurk { at: Vec2::new(1100.0, 450.0), ..lurk(side) };
            c.ride_flank(moved);
            assert!((hip(&c) - Vec2::new(1100.0 - o * 0.2 * 160.0, 450.0)).length() < 1e-3);
            // à mi-chemin de son retrait, elle revient sur ses pas sans saut
            c.withdraw();
            c.step(GLIDE / 2.0, 610);
            assert!((c.lurk_out().unwrap() - 0.5).abs() < 1e-4);
            let before = hip(&c);
            c.emerge();
            c.step(0.0, 611);
            assert_eq!(hip(&c), before);
            // une nouvelle taille : sa hanche reste sur son point d'accroche, à sa place dans la glissade
            c.set_size(Vec2::new(80.0, 150.0));
            assert!((hip(&c) - Vec2::new(1100.0 - o * (0.2 + 0.5 * 0.7) * 80.0, 450.0)).length() < 1e-3);
            // un pas qui n'est pas un nombre ne la fait pas glisser
            c.step(f32::NAN, 612);
            assert!((c.lurk_out().unwrap() - 0.5).abs() < 1e-4);
        }
    }

    #[test]
    fn only_a_moving_body_follows_the_screen_and_lurking_only_while_she_glides() {
        use crate::perch::Side;
        let mut c = capri();
        assert!(!c.follows_screen(), "elle flotte");
        c.lurk_on(lurk(Side::Left), FIT);
        assert!(!c.follows_screen(), "cachée, immobile");
        c.emerge();
        assert!(c.follows_screen(), "elle glisse");
        c.step(GLIDE / 2.0, 1);
        assert!(c.follows_screen());
        c.step(GLIDE, 2);
        assert!(!c.follows_screen(), "sortie, immobile : la boucle au repos");
        c.withdraw();
        assert!(c.follows_screen(), "elle se retire");
        c.grab(Vec2::new(900.0, 500.0), 0.0);
        assert!(c.follows_screen(), "tenue");
        c.release(0.0);
        assert!(!c.follows_screen());
        assert!(c.walk_to(100.0, 50.0) && c.follows_screen(), "en marche");
    }

    #[test]
    fn she_stops_lurking_whole_and_on_screen_where_she_is_or_when_grabbed() {
        use crate::perch::Side;
        // près du haut de l'écran : son image dépasse un peu (sa case a de la marge au-dessus)
        let mut c = capri();
        c.lurk_on(Lurk { at: Vec2::new(1000.0, 120.0), ..lurk(Side::Right) }, FIT);
        assert!(c.position().y < 0.0, "{:?}", c.position());
        // une nouvelle zone ne la déplace pas
        let before = c.position();
        c.set_bounds(Rect::new(Vec2::ZERO, Vec2::new(1920.0, 1080.0)));
        assert_eq!(c.position(), before);
        // son côté parti : debout, entière, dans l'écran, là où elle est, sans geste
        c.take_off();
        assert_eq!(c.stance(), Stance::Floating);
        assert_eq!(c.position(), Vec2::new(before.x, 0.0));
        assert_eq!(c.lurking(), None);
        // attrapée guettant, son image replacée : tenue, exactement là, même en partie hors de
        // l'écran (ce que tient le curseur reste sous lui) ; puis comme une prise ordinaire en
        // partie dehors : l'écran l'empêche seulement d'en sortir davantage, et lâchée, elle y revient
        for (cursor, origin) in [
            (Vec2::new(1100.0, 400.0), Vec2::new(1050.0, -20.0)),
            (Vec2::new(30.0, 400.0), Vec2::new(-47.0, 250.0)),
            (Vec2::new(1900.0, 400.0), Vec2::new(1790.0, 250.0)),
        ] {
            let mut c = capri();
            c.lurk_on(lurk(Side::Left), FIT);
            c.emerge();
            c.step(1.0, 30);
            c.grab_at(cursor, origin, 0.0);
            assert!(c.is_held() && c.lurking().is_none());
            assert_eq!(c.position(), origin, "pas de saut à la prise");
            let out =
                |c: &Capri| (0.0 - c.position().x).max(c.position().x + 160.0 - 1920.0).max(-c.position().y);
            let before = out(&c);
            c.drag_to(cursor + Vec2::new(-30.0, -30.0), 0.02);
            assert!(out(&c) <= before + 1e-3, "pas plus dehors");
            c.release(1.0);
            for frame in 31..80 {
                c.step(1.0 / 24.0, frame);
            }
            assert!(out(&c) <= 1e-3, "revenue dans l'écran : {:?}", c.position());
        }
        // seulement en guettant
        let mut c = capri();
        c.emerge();
        c.ride_flank(lurk(Side::Left));
        assert_eq!(c.stance(), Stance::Floating);
    }

    #[test]
    fn she_falls_from_above_the_screen_to_her_place_then_floats() {
        let mut c = capri();
        let place = c.feet();
        let from = Vec2::new(place.x, -40.0);
        assert!(c.fall(from, place.y));
        assert!(c.is_falling() && c.position().y < 0.0, "au-dessus de l'écran : {:?}", c.position());
        assert_eq!(c.feet(), from);
        let mut last = c.feet().y;
        let mut frame = 0;
        while c.is_falling() {
            frame += 1;
            c.step(1.0 / 60.0, frame / 2);
            assert!(c.feet().y > last || !c.is_falling(), "elle descend");
            assert!(c.feet().y <= place.y + 1e-3, "jamais plus bas que sa place");
            last = c.feet().y;
            assert!(frame < 200, "elle finit de tomber");
        }
        assert_eq!(c.stance(), Stance::Floating);
        assert!((c.feet() - place).length() < 1e-3, "à sa place : {:?}", c.feet());
        assert!((frame as f32 / 60.0 - crate::intro::act::FALL_SECONDS).abs() < 0.05, "{frame}");
        // tenue en tombant : tenue, comme d'habitude ; une nouvelle taille l'arrête dans l'écran
        let mut c = capri();
        assert!(c.fall(Vec2::new(500.0, -40.0), 700.0));
        c.grab(Vec2::new(500.0, 10.0), 0.0);
        assert!(c.is_held());
        let mut c = capri();
        assert!(c.fall(Vec2::new(500.0, -40.0), 700.0));
        c.step(0.3, 1);
        c.set_size(Vec2::new(80.0, 150.0));
        assert_eq!(c.stance(), Stance::Floating);
        assert!(c.position().y >= 0.0);
        // seulement en flottant
        c.grab(Vec2::new(500.0, 10.0), 0.0);
        assert!(!c.fall(Vec2::new(500.0, -40.0), 700.0));
    }

    #[test]
    fn landing_where_the_screen_no_longer_reaches_she_is_brought_into_it() {
        let mut c = capri();
        assert!(c.fall(Vec2::new(880.0, -40.0), 1000.0));
        c.step(0.5, 1);
        // l'écran rapetisse pendant sa chute : sa place est maintenant hors de l'écran
        c.set_bounds(Rect::new(Vec2::ZERO, Vec2::new(1920.0, 800.0)));
        c.step(1.0, 2);
        assert_eq!(c.stance(), Stance::Floating);
        assert!(c.position().y + c.size().y <= 800.0 + 1e-3, "dans l'écran à l'arrivée : {:?}", c.position());
    }

    #[test]
    fn grabbed_partly_off_the_screen_she_does_not_jump_and_never_goes_further_out() {
        // au-dessus de l'écran, 200 px de son image dehors (sa chute)
        let mut c = capri();
        assert!(c.fall(Vec2::new(880.0, 100.0), 700.0));
        assert_eq!(c.position(), Vec2::new(800.0, -200.0));
        c.grab(Vec2::new(880.0, 50.0), 0.0);
        assert_eq!(c.position(), Vec2::new(800.0, -200.0), "pas de saut à la prise");
        // le curseur descend un peu : elle le suit, toujours en partie dehors
        c.drag_to(Vec2::new(880.0, 60.0), 0.02);
        assert_eq!(c.position(), Vec2::new(800.0, -190.0), "elle suit le curseur");
        // il remonte : l'écran l'empêche de sortir davantage ; elle garde l'écart de la prise,
        // comme contre un bord, et repart quand le curseur le retrouve
        c.drag_to(Vec2::new(880.0, 20.0), 0.04);
        assert_eq!(c.position().y, -190.0, "pas plus dehors");
        c.drag_to(Vec2::new(870.0, 40.0), 0.06);
        assert_eq!(c.position(), Vec2::new(790.0, -190.0));
        c.drag_to(Vec2::new(870.0, 80.0), 0.07);
        assert_eq!(c.position(), Vec2::new(790.0, -170.0));
        // entièrement dedans : la règle ordinaire reprend
        c.drag_to(Vec2::new(870.0, 300.0), 0.08);
        assert_eq!(c.position().y, 50.0);
        c.drag_to(Vec2::new(870.0, 100.0), 0.10);
        assert_eq!(c.position().y, 0.0, "le bord haut l'arrête de nouveau");
        // au-delà du bord droit (sa marche d'entrée), de même
        let mut c = capri();
        c.set_beyond(Vec2::new(1960.0, 700.0));
        assert_eq!(c.position().x, 1880.0, "sa moitié gauche dans l'écran");
        c.grab(Vec2::new(1900.0, 600.0), 0.0);
        assert_eq!(c.position().x, 1880.0, "pas de saut à la prise");
        c.drag_to(Vec2::new(1910.0, 600.0), 0.02);
        assert_eq!(c.position().x, 1880.0, "ni plus à droite");
        c.drag_to(Vec2::new(1880.0, 600.0), 0.04);
        assert_eq!(c.position().x, 1860.0, "elle suit vers l'intérieur");
        // tenue par sa mèche : arrêtée, elle repart dès que le curseur revient vers l'écran
        let mut c = capri();
        assert!(c.fall(Vec2::new(880.0, 100.0), 700.0));
        c.grab(Vec2::new(880.0, 50.0), 0.0);
        c.hold_toward(Vec2::new(80.0, 250.0));
        c.drag_to(Vec2::new(880.0, 20.0), 0.02);
        assert_eq!(c.position().y, -200.0, "pas plus dehors");
        c.drag_to(Vec2::new(880.0, 30.0), 0.04);
        assert!(c.position().y > -200.0, "sans temps mort : {:?}", c.position());
    }

    #[test]
    fn let_go_partly_off_the_screen_she_glides_back_into_it_without_a_jump() {
        for thrown in [false, true] {
            let mut c = capri();
            assert!(c.fall(Vec2::new(880.0, 100.0), 700.0));
            c.grab(Vec2::new(880.0, 50.0), 0.0);
            if thrown {
                // lancée vers le bas, d'au-dessus de l'écran
                c.drag_to(Vec2::new(880.0, 90.0), 0.02);
                c.release(0.03);
                assert!(matches!(c.stance(), Stance::Thrown { .. }), "{:?}", c.stance());
            } else {
                c.release(1.0);
                assert_eq!(c.stance(), Stance::Floating);
            }
            let mut last = c.position();
            let mut frame = 0;
            while c.position().y < 0.0 {
                frame += 1;
                c.step(1.0 / 60.0, frame);
                let moved = c.position() - last;
                assert!(moved.y >= 0.0, "jamais vers le dehors : {moved:?}");
                assert!(moved.length() <= RETURN * 300.0 / 60.0 + 1e-3 || thrown, "sans saut : {moved:?}");
                last = c.position();
                assert!(frame < 120, "elle revient dans l'écran");
            }
            // dedans, elle y reste, et finit par flotter
            for more in 0..600 {
                c.step(1.0 / 60.0, frame + more);
                assert!(c.position().y >= 0.0 && c.position().y <= 780.0, "{:?}", c.position());
            }
            assert_eq!(c.stance(), Stance::Floating);
        }
    }

    #[test]
    fn she_walks_in_from_beyond_the_right_edge() {
        let mut c = capri();
        let start = Vec2::new(2100.0, 700.0);
        c.set_beyond(start);
        assert_eq!(c.feet(), start, "hors de l'écran, sans être ramenée dedans");
        assert!(c.walk_to(1500.0, 300.0));
        let mut frame = 0;
        let mut last = c.feet().x;
        while c.is_walking() {
            frame += 1;
            c.step(1.0 / 60.0, frame);
            // à sa vitesse, sans saut, dès le premier pas (pas ramenée contre le bord)
            let moved = last - c.feet().x;
            assert!(moved > 0.0, "elle avance : pas {frame}");
            assert!((moved - 5.0).abs() < 1e-2 || !c.is_walking(), "pas {frame} : {moved} px au lieu de 5");
            last = c.feet().x;
            assert!(frame < 1000);
        }
        assert_eq!(frame, 120, "600 px à 300 px par seconde : 2 s");
        assert!((c.feet().x - 1500.0).abs() < 1e-3);
        // dans l'écran, le bord vers lequel elle marche l'arrête toujours
        assert!(c.walk_to(-500.0, 3000.0));
        for frame in 0..100 {
            c.step(1.0 / 60.0, frame);
        }
        assert_eq!((c.position().x, c.stance()), (0.0, Stance::Floating));
    }

    #[test]
    fn holds_each_float_drawing_for_several_frames() {
        let mut c = capri();
        c.step(0.0, 0);
        let a = c.pose();
        c.step(0.0, 1);
        assert_eq!(a, c.pose(), "même dessin tenu : rien à redessiner");
        c.step(0.0, 6);
        assert_ne!(a.rect, c.pose().rect, "dessin suivant du flottement");
    }

    #[test]
    fn follows_the_cursor_where_it_was_grabbed() {
        let mut c = capri();
        c.grab(Vec2::new(850.0, 500.0), 0.0);
        c.drag_to(Vec2::new(950.0, 520.0), 0.1);
        assert_eq!(c.position(), Vec2::new(900.0, 420.0));
    }

    #[test]
    fn slow_release_stays_put_fast_release_throws() {
        let mut c = capri();
        c.grab(Vec2::new(850.0, 500.0), 0.0);
        c.drag_to(Vec2::new(852.0, 500.0), 0.05);
        c.release(0.06);
        assert_eq!(c.stance(), Stance::Floating);

        c.grab(Vec2::new(850.0, 500.0), 1.0);
        c.drag_to(Vec2::new(900.0, 500.0), 1.04);
        c.drag_to(Vec2::new(950.0, 500.0), 1.06);
        c.release(1.07);
        assert!(matches!(c.stance(), Stance::Thrown { vel } if vel.x > 0.0));
    }

    #[test]
    fn a_throw_bounces_inside_the_screen_then_settles() {
        let mut c = capri();
        c.grab(Vec2::new(850.0, 500.0), 0.0);
        c.drag_to(Vec2::new(1050.0, 400.0), 0.05);
        c.release(0.05);
        let mut bounced = false;
        for f in 0..2400u64 {
            c.step(1.0 / 120.0, f / 5);
            let p = c.position();
            assert!(p.x >= 0.0 && p.x <= 1920.0 - 160.0 && p.y >= 0.0 && p.y <= 1080.0 - 300.0);
            bounced |= c.squash.is_some();
        }
        assert!(bounced, "elle a tapé un bord");
        assert_eq!(c.stance(), Stance::Floating);
    }

    const SPOT: Spot = Spot { window: 7, along: 120.0, left: 780.0, width: 600.0 };

    fn perch_at(x: f32, y: f32) -> Perch {
        Perch { spot: SPOT, feet: Vec2::new(x, y) }
    }

    #[test]
    fn landing_puts_her_feet_on_the_edge_with_a_squash_held_whole_frames() {
        let mut c = capri();
        c.set_feet(Vec2::new(0.6, 1.0));
        c.step(0.0, 10);
        c.land(perch_at(900.0, 700.0));
        assert_eq!(c.perched(), Some(SPOT));
        assert_eq!(c.feet(), Vec2::new(900.0, 700.0));
        assert_eq!(c.position(), Vec2::new(900.0 - 96.0, 400.0));

        // l'écrasement, ses pieds au même endroit : 3 images, puis le rebond 2 images
        let bottom = |p: Pose| p.rect.min.y + p.rect.size.y;
        let squashed = c.pose();
        assert!(squashed.rect.size.y < 300.0 && squashed.rect.size.x > 160.0, "{squashed:?}");
        assert_eq!(bottom(squashed), 700.0);
        for frame in 11..13 {
            c.step(0.0, frame);
            assert_eq!(c.pose().rect, squashed.rect, "tenu, image {frame}");
        }
        c.step(0.0, 13);
        let rebound = c.pose();
        assert!(rebound.rect.size.y > 300.0, "{rebound:?}");
        assert_eq!(bottom(rebound), 700.0);
        c.step(0.0, 14);
        assert_eq!(c.pose().rect, rebound.rect);
        // puis immobile : ni flottement ni écrasement, image après image
        c.step(0.0, 15);
        let rest = c.pose().rect;
        assert_eq!(rest, Rect::new(Vec2::new(804.0, 400.0), Vec2::new(160.0, 300.0)));
        for frame in 16..80 {
            c.step(1.0 / 24.0, frame);
            assert_eq!(c.pose().rect, rest, "posée, elle ne flotte pas (image {frame})");
        }
    }

    #[test]
    fn she_rides_her_edge_and_takes_off_floating_in_place() {
        let mut c = capri();
        c.step(0.0, 0);
        c.land(perch_at(900.0, 700.0));
        c.ride(Perch { spot: Spot { along: 150.0, ..SPOT }, feet: Vec2::new(930.0, 720.0) });
        assert_eq!(c.feet(), Vec2::new(930.0, 720.0));
        assert_eq!(c.perched().map(|spot| spot.along), Some(150.0));

        c.take_off();
        assert_eq!(c.stance(), Stance::Floating);
        assert_eq!(c.feet(), Vec2::new(930.0, 720.0), "elle flotte sur place");
        // un dernier geste, étirée sur ses pieds, puis le flottement
        assert!(c.pose().rect.size.y > 300.0);
        c.step(0.0, 2);
        assert_eq!(c.pose().rect.size.y, 300.0);
        // suivre un bord sans y être posée ne fait rien
        c.ride(perch_at(100.0, 100.0));
        assert_eq!(c.feet(), Vec2::new(930.0, 720.0));
        assert_eq!(c.perched(), None);
    }

    #[test]
    fn after_a_leave_of_some_frames_she_stretches_then_floats() {
        let mut c = capri();
        c.set_leave_frames(24);
        c.step(0.0, 100);
        c.land(perch_at(900.0, 700.0));
        assert_eq!(c.seat(), Some(Vec2::new(900.0, 700.0)));
        c.take_off();
        assert_eq!(c.seat(), None);
        // le temps qu'elle s'efface : ni étirement, ni geste oublié
        for frame in 100..124 {
            c.step(0.0, frame);
            assert_eq!(c.pose().rect.size, Vec2::new(160.0, 300.0), "image {frame}");
        }
        c.step(0.0, 124);
        assert!(c.pose().rect.size.y > 300.0, "puis étirée");
        c.step(0.0, 126);
        assert_eq!(c.pose().rect.size.y, 300.0);
    }

    #[test]
    fn grabbed_where_she_was_drawn_seated_she_stands_under_the_cursor() {
        let mut c = capri();
        c.land(perch_at(900.0, 700.0));
        c.grab_at(Vec2::new(950.0, 720.0), Vec2::new(880.0, 600.0), 0.0);
        assert!(c.is_held());
        assert_eq!(c.position(), Vec2::new(880.0, 600.0));
        c.drag_to(Vec2::new(960.0, 720.0), 0.1);
        assert_eq!(c.position(), Vec2::new(890.0, 600.0), "elle suit le curseur d'où on l'a prise");
        // jamais hors de l'écran, et elle suit le curseur tout de suite (comme avant sa planche
        // en l'air)
        c.grab_at(Vec2::new(10.0, 10.0), Vec2::new(-500.0, -500.0), 0.2);
        assert_eq!(c.position(), Vec2::ZERO);
        c.drag_to(Vec2::new(11.0, 12.0), 0.3);
        assert_eq!(c.position(), Vec2::new(1.0, 2.0));
    }

    #[test]
    fn held_toward_her_grip_she_glides_with_the_cursor_and_never_waits_at_an_edge() {
        // debout au bas de l'écran, prise aux pieds ; sa mèche est à (124, 58) de son coin
        let screen = Rect::new(Vec2::ZERO, Vec2::new(1920.0, 2000.0));
        let size = Vec2::new(200.0, 664.0);
        let mut c = Capri::new(size, screen, Vec2::new(800.0, 2000.0 - 664.0));
        let start = c.position();
        let cursor = start + Vec2::new(100.0, 630.0);
        let grip = Vec2::new(124.0, 58.0);
        c.grab(cursor, 0.0);
        c.hold_toward(grip);
        assert_eq!(c.position(), start, "à la prise, son corps ne bouge pas");
        // curseur immobile : rien ne bouge
        c.drag_to(cursor, 0.05);
        assert_eq!(c.position(), start);
        // le curseur monte : elle monte aussitôt, d'au moins la moitié du chemin, et sa mèche
        // s'approche du curseur
        let mut cursor = cursor;
        let mut gap = (cursor - (c.position() + grip)).length();
        for step in 1..=400 {
            let before = c.position();
            cursor += Vec2::new(0.0, -3.0);
            c.drag_to(cursor, 0.05 + f64::from(step) * 0.01);
            let moved = before.y - c.position().y;
            assert!(moved >= 1.5 - 1e-3, "pas {step} : elle n'a monté que de {moved}");
            let now = (cursor - (c.position() + grip)).length();
            assert!(now <= gap + 1e-3, "pas {step} : sa mèche s'éloigne ({gap} → {now})");
            gap = now;
        }
        assert!(gap < 1e-3, "sa mèche sous le curseur : {gap}");
        // poussée contre le haut de l'écran, puis le curseur redescend : elle suit aussitôt
        for step in 0..100 {
            cursor += Vec2::new(0.0, -10.0);
            c.drag_to(cursor, 5.0 + f64::from(step) * 0.01);
        }
        assert_eq!(c.position().y, 0.0);
        let top = c.position();
        c.drag_to(cursor + Vec2::new(0.0, 4.0), 6.5);
        assert!(c.position().y > top.y, "elle redescend dès le premier pas");
        // une nouvelle prise oublie l'écart voulu : son corps ne bouge plus vers la mèche
        let here = c.position();
        let at = here + Vec2::new(50.0, 500.0);
        c.grab(at, 7.0);
        c.drag_to(at + Vec2::new(0.0, 40.0), 7.1);
        assert_eq!(c.position(), here + Vec2::new(0.0, 40.0));
    }

    #[test]
    fn a_new_size_stops_her_walk_where_she_is() {
        let mut c = capri();
        c.step(0.0, 0);
        let feet = c.feet();
        assert!(c.walk_to(feet.x - 300.0, 100.0));
        c.step(0.5, 1);
        let walked = c.feet();
        c.set_size(Vec2::new(240.0, 450.0));
        assert_eq!(c.stance(), Stance::Floating);
        assert_eq!(c.feet(), walked, "ses pieds là où ils étaient");
    }

    #[test]
    fn grabbing_her_takes_her_off_her_edge() {
        let mut c = capri();
        c.land(perch_at(900.0, 700.0));
        c.grab(Vec2::new(880.0, 600.0), 0.0);
        assert!(c.is_held());
        assert_eq!(c.perched(), None);
        assert_eq!(c.pose().rect.size, Vec2::new(160.0, 300.0), "aucun geste pendant qu'on la tient");
    }

    #[test]
    fn a_new_size_keeps_her_feet_where_they_were() {
        let mut c = capri();
        c.set_feet(Vec2::new(0.6, 1.0));
        c.land(perch_at(900.0, 700.0));
        c.set_size(Vec2::new(320.0, 600.0));
        assert_eq!(c.feet(), Vec2::new(900.0, 700.0));
    }

    #[test]
    fn a_new_size_keeps_her_whole_on_screen() {
        // en haut de l'écran, elle grandit : ses pieds descendent juste assez pour que sa tête reste
        // dedans ; elle ne déborde ni en haut ni sur le côté
        let screen = Rect::new(Vec2::ZERO, Vec2::new(1920.0, 1080.0));
        let mut c = Capri::new(Vec2::new(160.0, 300.0), screen, Vec2::new(1700.0, 0.0));
        c.set_size(Vec2::new(320.0, 600.0));
        assert_eq!(c.position(), Vec2::new(1600.0, 0.0));
        assert_eq!(c.feet(), Vec2::new(1760.0, 600.0));
        // elle rapetisse : ses pieds restent où ils sont
        c.set_size(Vec2::new(64.0, 120.0));
        assert_eq!(c.feet(), Vec2::new(1760.0, 600.0));
        assert_eq!(c.size(), Vec2::new(64.0, 120.0));
    }

    #[test]
    fn her_tour_stands_still_then_resumes_where_it_stopped_without_a_jump() {
        let mut c = capri();
        let at = |c: &mut Capri, frame: u64| {
            c.step(0.0, frame);
            (c.tour(), c.pose().rect)
        };
        // sans arrêt, son tour suit l'image d'animation
        assert_eq!(at(&mut c, 10).0, 10);
        // arrêtée à l'image 10 jusqu'à l'image 40 : ni flottement, ni tour qui avance
        let (tour, still) = at(&mut c, 10);
        c.still(Some(40));
        for frame in 10..40 {
            assert_eq!(at(&mut c, frame), (tour, still), "image {frame}");
        }
        // puis il reprend là où il s'était arrêté : à l'image 40, le dessin de l'image 10
        assert_eq!(at(&mut c, 40), (10, still));
        assert_eq!(at(&mut c, 41).0, 11);
        // le flottement suit le tour, pas l'image : 6 images plus tard, le dessin suivant
        assert_ne!(at(&mut c, 46).1, still);

        // arrêtée sans fin, puis une fin fixée plus tard : la même hauteur jusque-là
        let (tour, still) = at(&mut c, 100);
        c.still(None);
        assert_eq!(at(&mut c, 500), (tour, still));
        c.still(Some(530));
        assert_eq!(at(&mut c, 529), (tour, still));
        assert_eq!(at(&mut c, 531).0, tour + 1);

        // l'étirement qui attendait la fin d'un départ de son bord est oublié ; pas celui en cours
        c.set_leave_frames(24);
        c.land(perch_at(900.0, 700.0));
        c.take_off();
        c.still(Some(560));
        for frame in 532..570 {
            assert_eq!(at(&mut c, frame).1.size, Vec2::new(160.0, 300.0), "image {frame}");
        }
        c.step(0.0, 600);
        c.land(perch_at(900.0, 700.0));
        c.take_off();
        c.step(0.0, 624);
        c.still(None);
        assert!(c.pose().rect.size.y > 300.0, "l'étirement déjà commencé continue");

        // on l'attrape alors qu'elle est arrêtée : son tour reprend tout de suite
        let tour = c.tour();
        c.grab(Vec2::new(850.0, 500.0), 0.0);
        c.release(0.01);
        assert_eq!(at(&mut c, 700).0, tour + 76);
    }

    #[test]
    fn set_somewhere_else_she_is_there_at_once_without_a_gesture() {
        let mut c = capri();
        c.set_leave_frames(24);
        c.step(0.0, 10);
        // en plein lancer, puis posée ailleurs d'un coup : ni écrasement, ni rebond
        c.grab(Vec2::new(850.0, 500.0), 0.0);
        c.drag_to(Vec2::new(1050.0, 500.0), 0.05);
        c.release(0.05);
        assert!(matches!(c.stance(), Stance::Thrown { .. }));
        c.set_on(perch_at(500.0, 700.0));
        assert_eq!((c.perched(), c.seat()), (Some(SPOT), Some(Vec2::new(500.0, 700.0))));
        assert_eq!(c.pose().rect.size, Vec2::new(160.0, 300.0), "pas d'écrasement à l'arrivée");
        // repartie de son bord, puis debout ailleurs : l'étirement qui attendait est oublié
        c.take_off();
        c.set_at(Vec2::new(300.0, 900.0));
        assert_eq!((c.stance(), c.feet()), (Stance::Floating, Vec2::new(300.0, 900.0)));
        for frame in 10..60 {
            c.step(0.0, frame);
            assert_eq!(c.pose().rect.size, Vec2::new(160.0, 300.0), "image {frame}");
        }
        // jamais hors de l'écran
        c.set_at(Vec2::new(5000.0, 5000.0));
        assert_eq!(c.position(), Vec2::new(1920.0 - 160.0, 1080.0 - 300.0));
    }

    #[test]
    fn she_walks_to_where_she_goes_at_her_speed_then_floats() {
        let mut c = capri();
        c.set_feet(Vec2::new(0.5, 1.0));
        c.step(0.0, 0);
        let feet = c.feet();
        assert!(c.walk_to(feet.x - 100.0, 60.0));
        assert!(c.is_walking() && !c.is_held());
        // une seconde à 60 pas : 60 px, à la hauteur où elle était
        for frame in 1..=60 {
            c.step(1.0 / 60.0, frame);
        }
        assert!((c.feet().x - (feet.x - 60.0)).abs() < 0.01, "{:?}", c.feet());
        assert_eq!(c.feet().y, feet.y);
        assert!(c.is_walking());
        // elle ne dépasse pas là où elle va
        c.step(1.0, 61);
        assert_eq!(c.feet().x, feet.x - 100.0);
        assert_eq!(c.stance(), Stance::Floating);
        // déjà en marche, ou pas en flottant : refusé
        assert!(c.walk_to(feet.x, 60.0));
        assert!(!c.walk_to(feet.x - 10.0, 60.0), "déjà en marche");
        c.halt();
        assert_eq!(c.stance(), Stance::Floating, "elle s'arrête là où elle est");
        assert!(!c.walk_to(feet.x, 0.0) && !c.walk_to(f32::NAN, 60.0));
        c.grab(Vec2::new(850.0, 500.0), 0.0);
        assert!(!c.walk_to(feet.x, 60.0), "tenue");
        // attrapée en marche : elle ne marche plus
        c.release(0.01);
        assert!(c.walk_to(feet.x - 50.0, 60.0));
        c.grab(Vec2::new(850.0, 500.0), 1.0);
        assert!(c.is_held() && !c.is_walking());
    }

    #[test]
    fn walking_she_keeps_the_height_of_her_float_and_stops_at_the_edge_of_the_screen() {
        let mut c = capri();
        c.step(0.0, 30);
        c.still(None);
        let floating = c.pose().rect;
        assert!(c.walk_to(-500.0, 600.0));
        c.step(0.05, 31);
        let walking = c.pose().rect;
        assert_eq!(walking.min.y, floating.min.y, "son flottement arrêté, à la même hauteur");
        assert_eq!(walking.size, floating.size, "ni écrasée ni étirée");
        // le bord de l'écran l'arrête : elle flotte de nouveau, là
        for frame in 32..200 {
            c.step(0.05, frame);
        }
        assert_eq!(c.position().x, 0.0);
        assert_eq!(c.stance(), Stance::Floating);
    }

    #[test]
    fn stays_on_screen_when_bounds_shrink() {
        let mut c = capri();
        c.set_bounds(Rect::new(Vec2::ZERO, Vec2::new(600.0, 400.0)));
        let p = c.position();
        assert!(p.x <= 440.0 && p.y <= 100.0);
    }
}
