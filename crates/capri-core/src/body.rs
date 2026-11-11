//! Capri elle-même : où elle est, ce qu'elle fait, et la pose qu'on dessine.
//!
//! Les vitesses et seuils sont exprimés en fractions de sa hauteur, pour qu'elle se comporte
//! pareil sur un écran Retina et sur un écran standard, quelle que soit sa taille.

use std::collections::VecDeque;

use crate::anim::Sheet;
use crate::geom::{Rect, Vec2};
use crate::perch::{Perch, Spot, Stature};

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
    /// [`crate::perch`]) : elle ne flotte plus, et suit le bord.
    Perched { spot: Spot },
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

/// Flottement au repos : huit dessins tenus six images chacun (2 s par cycle), en fraction
/// de sa hauteur. Pas une sinusoïde : des poses clés, comme en anime.
const FLOAT: [(f32, u32); 8] =
    [(0.0, 6), (-0.004, 6), (-0.010, 6), (-0.016, 6), (-0.019, 6), (-0.016, 6), (-0.010, 6), (-0.004, 6)];

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
/// Elle se pose sur un bord : écrasée sur ses pieds (3 images), un rebond étiré (2 images), puis
/// immobile. Échelles (largeur, hauteur), tenues un nombre entier d'images, ancrées à ses pieds.
const LANDING: [((f32, f32), u32); 2] = [((1.14, 0.84), 3), ((0.95, 1.06), 2)];
/// Elle quitte un bord : étirée vers le haut (2 images), puis elle flotte de nouveau.
const LIFTING: [((f32, f32), u32); 1] = [((0.93, 1.08), 2)];

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

    /// Où sont ses pieds dans son image (coordonnées normalisées, voir `AlphaMask::feet`). Par
    /// défaut : au milieu du bas de l'image.
    pub fn set_feet(&mut self, feet: Vec2) {
        self.feet = feet;
    }

    /// Le milieu de ses pieds, là où elle touche un bord.
    pub fn feet(&self) -> Vec2 {
        self.pos + Vec2::new(self.size.x * self.feet.x, self.size.y * self.feet.y)
    }

    /// Sa taille et la place de ses pieds, pour [`crate::perch`].
    pub fn stature(&self) -> Stature {
        Stature { size: self.size, feet: self.feet }
    }

    /// Où elle est posée, si elle l'est.
    pub fn perched(&self) -> Option<Spot> {
        match self.stance {
            Stance::Perched { spot } => Some(spot),
            _ => None,
        }
    }

    /// Elle se pose sur un bord : ses pieds en `perch.feet`, avec un écrasement à l'atterrissage.
    pub fn land(&mut self, perch: Perch) {
        self.place_feet(perch.feet);
        self.stance = Stance::Perched { spot: perch.spot };
        self.squash = None;
        self.gesture = Some((self.frame, Gesture::Landing));
    }

    /// Posée, elle suit son bord : ses pieds en `perch.feet`, sans nouvel atterrissage. Sans
    /// effet si elle n'est pas posée.
    pub fn ride(&mut self, perch: Perch) {
        if let Stance::Perched { .. } = self.stance {
            self.place_feet(perch.feet);
            self.stance = Stance::Perched { spot: perch.spot };
        }
    }

    /// Elle quitte son bord et se remet à flotter sur place. Sans effet si elle n'est pas posée.
    pub fn take_off(&mut self) {
        if let Stance::Perched { .. } = self.stance {
            self.stance = Stance::Floating;
            self.gesture = Some((self.frame, Gesture::Lifting));
        }
    }

    /// Nouvelle zone (écran redimensionné, changement d'écran) : elle reste dedans.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.clamp();
    }

    /// Nouvelle taille, en gardant ses pieds au même endroit.
    pub fn set_size(&mut self, size: Vec2) {
        let feet = self.feet();
        self.size = size;
        self.place_feet(feet);
    }

    /// On l'attrape avec le curseur en `cursor` à l'instant `t` (secondes).
    pub fn grab(&mut self, cursor: Vec2, t: f64) {
        self.stance = Stance::Held { grab: cursor - self.pos };
        self.gesture = None;
        self.samples.clear();
        self.samples.push_back((t, cursor));
    }

    /// Le curseur bouge pendant qu'on la tient.
    pub fn drag_to(&mut self, cursor: Vec2, t: f64) {
        let Stance::Held { grab } = self.stance else { return };
        self.pos = cursor - grab;
        self.clamp();
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
        if self.gesture.is_some() && self.gesture_scale().is_none() {
            self.gesture = None;
        }
        let Stance::Thrown { mut vel } = self.stance else { return };

        self.pos += vel * dt;
        vel = vel * (-AIR_DRAG * dt).exp();

        let (min, max) = (self.bounds.min, self.bounds.max() - self.size);
        if self.pos.x < min.x || self.pos.x > max.x {
            self.pos.x = self.pos.x.clamp(min.x, max.x.max(min.x));
            vel.x = -vel.x * BOUNCE;
            self.squash = Some((frame + SQUASH_FRAMES, Axis::Horizontal));
        }
        if self.pos.y < min.y || self.pos.y > max.y {
            self.pos.y = self.pos.y.clamp(min.y, max.y.max(min.y));
            vel.y = -vel.y * BOUNCE;
            self.squash = Some((frame + SQUASH_FRAMES, Axis::Vertical));
        }

        self.stance =
            if vel.length() < STOP_BELOW * self.size.y { Stance::Floating } else { Stance::Thrown { vel } };
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
            Stance::Floating => offset.y = self.float.at(frame) * self.size.y,
            // posée : immobile, seul son trait continue de bouillir
            Stance::Held { .. } | Stance::Perched { .. } => {}
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
            self.feet() - Vec2::new(size.x * self.feet.x, size.y * self.feet.y) + offset
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

    fn clamp(&mut self) {
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
    fn stays_on_screen_when_bounds_shrink() {
        let mut c = capri();
        c.set_bounds(Rect::new(Vec2::ZERO, Vec2::new(600.0, 400.0)));
        let p = c.position();
        assert!(p.x <= 440.0 && p.y <= 100.0);
    }
}
