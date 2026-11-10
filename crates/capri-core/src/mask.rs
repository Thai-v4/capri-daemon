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
}

impl AlphaMask {
    /// `rgba` : pixels RGBA 8 bits, ligne par ligne. Un pixel compte si son alpha dépasse
    /// `threshold`. `grow` dilate la silhouette de ce nombre de pixels (du sprite).
    ///
    /// # Panics
    /// Si `rgba` ne fait pas `width * height * 4` octets.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8], threshold: u8, grow: u32) -> Self {
        let (w, h) = (width as usize, height as usize);
        assert_eq!(rgba.len(), w * h * 4, "taille RGBA incohérente");
        let base: Vec<bool> = rgba.chunks_exact(4).map(|p| p[3] > threshold).collect();
        let solid = if grow == 0 { base } else { dilate(&base, w, h, grow as usize) };
        Self { width, height, solid }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
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
    fn grow_makes_thin_parts_easier_to_grab() {
        let m = AlphaMask::from_rgba(4, 4, &dot(), 128, 1);
        assert!(m.hit(Vec2::new(0.05, 0.05))); // pixel (0, 0), voisin
        assert!(m.hit(Vec2::new(0.6, 0.6))); // pixel (2, 2), voisin
        assert!(!m.hit(Vec2::new(0.9, 0.9))); // pixel (3, 3), trop loin
    }
}
