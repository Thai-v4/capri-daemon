//! Hit-test au pixel près, à partir du canal alpha du sprite.
//!
//! Hors de son corps, la souris doit traverser la fenêtre comme si elle n'existait pas.
//! Le masque est légèrement dilaté pour qu'on puisse attraper ses membres les plus fins.

use crate::geom::Vec2;

#[derive(Clone, Debug)]
pub struct AlphaMask {
    width: u32,
    height: u32,
    solid: Vec<bool>,
    feet: Vec2,
}

/// Le bas de sa silhouette sur cette part de la hauteur de l'image compte comme ses pieds.
const FEET_BAND: f32 = 0.02;

impl AlphaMask {
    /// `rgba` : pixels RGBA 8 bits, ligne par ligne. Un pixel compte si son alpha dépasse
    /// `threshold`. `grow` dilate la silhouette de ce nombre de pixels (du sprite).
    ///
    /// # Panics
    /// Si `rgba` ne fait pas `width * height * 4` octets.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8], threshold: u8, grow: u32) -> Self {
        let (w, h) = (width as usize, height as usize);
        assert_eq!(rgba.len(), w * h * 4, "taille RGBA incohérente");
        let base: Vec<bool> = rgba.as_chunks::<4>().0.iter().map(|p| p[3] > threshold).collect();
        let feet = feet(&base, w, h);
        let solid = if grow == 0 { base } else { dilate(&base, w, h, grow as usize) };
        Self { width, height, solid, feet }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Où sont ses pieds dans l'image, en coordonnées normalisées : le milieu, en largeur, du bas
    /// de sa silhouette, et la dernière ligne pleine. (0,5 ; 1) pour une image vide.
    pub fn feet(&self) -> Vec2 {
        self.feet
    }

    /// Le point de coordonnées normalisées `uv` (0..1) est-il sur elle ?
    pub fn hit(&self, uv: Vec2) -> bool {
        if !(0.0..=1.0).contains(&uv.x) || !(0.0..=1.0).contains(&uv.y) {
            return false;
        }
        let x = ((uv.x * self.width as f32) as u32).min(self.width - 1);
        let y = ((uv.y * self.height as f32) as u32).min(self.height - 1);
        self.solid[(y * self.width + x) as usize]
    }

    /// Part des pixels pleins, utile pour vérifier qu'une image a bien un canal alpha.
    pub fn coverage(&self) -> f32 {
        self.solid.iter().filter(|&&s| s).count() as f32 / self.solid.len().max(1) as f32
    }
}

/// Ses pieds, d'après la silhouette non dilatée : voir [`AlphaMask::feet`].
fn feet(solid: &[bool], w: usize, h: usize) -> Vec2 {
    let row = |y: usize| &solid[y * w..(y + 1) * w];
    let Some(bottom) = (0..h).rev().find(|&y| row(y).contains(&true)) else { return Vec2::new(0.5, 1.0) };
    let band = ((h as f32 * FEET_BAND) as usize).max(1);
    let (mut left, mut right) = (w, 0);
    for y in bottom.saturating_sub(band - 1)..=bottom {
        for (x, _) in row(y).iter().enumerate().filter(|(_, s)| **s) {
            (left, right) = (left.min(x), right.max(x + 1));
        }
    }
    Vec2::new((left + right) as f32 / 2.0 / w as f32, (bottom + 1) as f32 / h as f32)
}

/// Dilatation carrée en deux passes séparables (horizontale puis verticale).
fn dilate(src: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let mut tmp = vec![false; src.len()];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            tmp[y * w + x] = (a..=b).any(|i| src[y * w + i]);
        }
    }
    let mut out = vec![false; src.len()];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
            out[y * w + x] = (a..=b).any(|j| tmp[j * w + x]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4×4 transparent avec un seul pixel plein en (1, 1).
    fn dot() -> Vec<u8> {
        let mut px = vec![0u8; 4 * 4 * 4];
        px[(4 + 1) * 4 + 3] = 255;
        px
    }

    #[test]
    fn hits_only_solid_pixels() {
        let m = AlphaMask::from_rgba(4, 4, &dot(), 128, 0);
        assert!(m.hit(Vec2::new(0.3, 0.3))); // pixel (1, 1)
        assert!(!m.hit(Vec2::new(0.9, 0.9)));
        assert!(!m.hit(Vec2::new(-0.1, 0.3)));
        assert!((m.coverage() - 1.0 / 16.0).abs() < 1e-6);
    }

    #[test]
    fn her_feet_are_the_middle_of_the_bottom_of_her_silhouette() {
        // 4×4 : le pixel (1, 1) et, plus bas, les pixels (2, 2) et (3, 2) ; rien en dernière ligne
        let mut px = dot();
        px[(2 * 4 + 2) * 4 + 3] = 255;
        px[(2 * 4 + 3) * 4 + 3] = 255;
        let m = AlphaMask::from_rgba(4, 4, &px, 128, 1);
        assert_eq!(m.feet(), Vec2::new(0.75, 0.75), "la dilatation n'y change rien");
        let empty = AlphaMask::from_rgba(4, 4, &[0; 64], 128, 0);
        assert_eq!(empty.feet(), Vec2::new(0.5, 1.0));
    }

    #[test]
    fn grow_makes_thin_parts_easier_to_grab() {
        let m = AlphaMask::from_rgba(4, 4, &dot(), 128, 1);
        assert!(m.hit(Vec2::new(0.05, 0.05))); // pixel (0, 0), voisin
        assert!(m.hit(Vec2::new(0.6, 0.6))); // pixel (2, 2), voisin
        assert!(!m.hit(Vec2::new(0.9, 0.9))); // pixel (3, 3), trop loin
    }
}
