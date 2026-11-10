//! Le temps de l'animation, à la manière d'une feuille d'exposition.
//!
//! Rien n'est interpolé : chaque pose est un dessin tenu un nombre entier d'images à 24 i/s.
//! C'est ce qui donne le rendu « cel » d'un anime plutôt que la fluidité d'un widget.

use std::time::Duration;

/// Images d'animation par seconde (la base, pas le rafraîchissement de l'écran).
pub const FRAME_RATE: f64 = 24.0;

/// Numéro de l'image d'animation à un instant donné.
pub fn frame_at(elapsed: Duration) -> u64 {
    (elapsed.as_secs_f64() * FRAME_RATE) as u64
}

/// Une suite de dessins, chacun tenu `hold` images, qui boucle.
#[derive(Clone, Debug)]
pub struct Sheet<T> {
    keys: Vec<(T, u32)>,
    total: u64,
}

impl<T: Copy> Sheet<T> {
    /// `keys` : (dessin, nombre d'images tenues). Un `hold` de 0 compte pour 1.
    ///
    /// # Panics
    /// Si `keys` est vide.
    pub fn new(keys: &[(T, u32)]) -> Self {
        assert!(!keys.is_empty(), "une feuille d'exposition a au moins un dessin");
        let keys: Vec<_> = keys.iter().map(|&(t, h)| (t, h.max(1))).collect();
        let total = keys.iter().map(|&(_, h)| u64::from(h)).sum();
        Self { keys, total }
    }

    /// Durée d'un tour complet, en images.
    pub fn len(&self) -> u64 {
        self.total
    }

    /// Une feuille n'est jamais vide (voir [`Sheet::new`]).
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Index du dessin montré à l'image `frame`.
    pub fn index_at(&self, frame: u64) -> usize {
        let mut t = frame % self.total;
        for (i, &(_, hold)) in self.keys.iter().enumerate() {
            let hold = u64::from(hold);
            if t < hold {
                return i;
            }
            t -= hold;
        }
        self.keys.len() - 1
    }

    /// Dessin montré à l'image `frame`.
    pub fn at(&self, frame: u64) -> T {
        self.keys[self.index_at(frame)].0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_each_drawing_then_loops() {
        let s = Sheet::new(&[('a', 2), ('b', 3)]);
        let seq: String = (0..10).map(|f| s.at(f)).collect();
        assert_eq!(seq, "aabbbaabbb");
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn zero_hold_counts_as_one() {
        let s = Sheet::new(&[(1, 0), (2, 1)]);
        assert_eq!((s.at(0), s.at(1), s.at(2)), (1, 2, 1));
    }

    #[test]
    fn frames_follow_24_per_second() {
        assert_eq!(frame_at(Duration::from_millis(0)), 0);
        assert_eq!(frame_at(Duration::from_secs(1)), 24);
        assert_eq!(frame_at(Duration::from_millis(1500)), 36);
    }
}
