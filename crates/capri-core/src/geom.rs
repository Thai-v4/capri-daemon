//! Petite géométrie 2D, juste ce qu'il faut.

use std::ops::{Add, AddAssign, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Self) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, k: f32) -> Self {
        Self::new(self.x * k, self.y * k)
    }
}

/// Rectangle aligné sur les axes : coin haut gauche + taille.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub size: Vec2,
}

impl Rect {
    pub const fn new(min: Vec2, size: Vec2) -> Self {
        Self { min, size }
    }

    pub fn max(&self) -> Vec2 {
        self.min + self.size
    }

    /// Coordonnées normalisées (0..1) d'un point dans le rectangle, `None` s'il est dehors.
    pub fn to_unit(&self, p: Vec2) -> Option<Vec2> {
        if self.size.x <= 0.0 || self.size.y <= 0.0 {
            return None;
        }
        let u = (p.x - self.min.x) / self.size.x;
        let v = (p.y - self.min.y) / self.size.y;
        ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)).then_some(Vec2::new(u, v))
    }
}
