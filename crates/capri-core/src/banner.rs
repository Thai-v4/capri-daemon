//! Le bandeau au-dessus de Capri : quand il apparaît, comment, et où il se pose.
//!
//! Comme elle, il est animé « en deux » : chaque dessin est tenu deux images à 24 i/s, sans
//! interpolation. Il surgit en trois dessins, tressaute quand son texte change et s'éclipse en
//! deux. Entre-temps il ne bouge pas : rien à redessiner.

use crate::geom::{Rect, Vec2};

/// Nombre d'images pendant lesquelles un dessin est tenu.
const HOLD: u64 = 2;

/// Ce que le rendu doit savoir du bandeau à une image donnée.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BannerPose {
    /// Change à chaque nouveau contenu : le rendu sait qu'il doit le retracer.
    pub generation: u64,
    /// Agrandissement autour de son centre (1.0 : taille réelle).
    pub scale: f32,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Hidden,
    /// Affiché depuis l'image `since`. `replaced` : il remplace un contenu déjà à l'écran.
    Shown {
        since: u64,
        replaced: bool,
    },
    /// En train de disparaître depuis l'image `since`.
    Leaving {
        since: u64,
    },
}

/// Le bandeau et son contenu (un texte et son ton, par exemple).
#[derive(Clone, Debug)]
pub struct Banner<T> {
    content: Option<T>,
    generation: u64,
    stage: Stage,
}

impl<T> Default for Banner<T> {
    fn default() -> Self {
        Self { content: None, generation: 0, stage: Stage::Hidden }
    }
}

impl<T: PartialEq> Banner<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ce que le bandeau doit montrer à partir de l'image `frame` ; `None` pour le retirer.
    /// Redonner le même contenu ne relance pas l'animation.
    pub fn set(&mut self, wanted: Option<T>, frame: u64) {
        let showing = matches!(self.stage, Stage::Shown { .. });
        match wanted {
            Some(content) => {
                if showing && self.content.as_ref() == Some(&content) {
                    return;
                }
                self.content = Some(content);
                self.generation += 1;
                self.stage = Stage::Shown { since: frame, replaced: showing };
            }
            None => {
                if showing {
                    self.stage = Stage::Leaving { since: frame };
                }
            }
        }
    }

    /// Le contenu à l'écran, y compris pendant qu'il s'éclipse.
    pub fn content(&self, frame: u64) -> Option<&T> {
        self.pose(frame).and(self.content.as_ref())
    }

    /// La pose à dessiner à l'image `frame`, ou `None` s'il n'y a rien à montrer.
    pub fn pose(&self, frame: u64) -> Option<BannerPose> {
        let drawing = |since: u64| frame.saturating_sub(since) / HOLD;
        let (scale, opacity) = match self.stage {
            Stage::Hidden => return None,
            Stage::Shown { since, replaced: false } => match drawing(since) {
                0 => (0.86, 0.7),
                1 => (1.05, 1.0),
                _ => (1.0, 1.0),
            },
            Stage::Shown { since, replaced: true } => match drawing(since) {
                0 => (1.04, 1.0),
                _ => (1.0, 1.0),
            },
            Stage::Leaving { since } => match drawing(since) {
                0 => (0.97, 0.6),
                1 => (0.92, 0.3),
                _ => return None,
            },
        };
        Some(BannerPose { generation: self.generation, scale, opacity })
    }
}

/// Où poser le bandeau (son coin haut gauche, en pixels entiers) : centré au-dessus d'elle, à
/// `gap` de sa tête ; en dessous de ses pieds s'il n'y a pas la place ; toujours dans `bounds`.
pub fn place_banner(capri: Rect, banner: Vec2, bounds: Rect, gap: f32) -> Vec2 {
    let lowest = |min: f32, max: f32| if max < min { min } else { max };
    let left = bounds.min.x + gap;
    let right = lowest(left, bounds.max().x - gap - banner.x);
    let x = (capri.min.x + (capri.size.x - banner.x) / 2.0).clamp(left, right);

    let top = bounds.min.y + gap;
    let bottom = lowest(top, bounds.max().y - gap - banner.y);
    let above = capri.min.y - gap - banner.y;
    let below = capri.max().y + gap;
    let y = if above >= top {
        above
    } else if below <= bottom {
        below
    } else {
        above.clamp(top, bottom)
    };
    Vec2::new(x.round(), y.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(banner: &Banner<&str>, frame: u64) -> Option<(f32, f32)> {
        banner.pose(frame).map(|p| (p.scale, p.opacity))
    }

    #[test]
    fn pops_in_over_three_held_drawings_then_stays_still() {
        let mut banner = Banner::new();
        assert_eq!(banner.pose(0), None);
        banner.set(Some("Claude réfléchit…"), 10);
        assert_eq!(pose(&banner, 10), Some((0.86, 0.7)));
        assert_eq!(pose(&banner, 11), pose(&banner, 10), "dessin tenu deux images");
        assert_eq!(pose(&banner, 12), Some((1.05, 1.0)));
        assert_eq!(pose(&banner, 14), Some((1.0, 1.0)));
        assert_eq!(banner.pose(14), banner.pose(10_000), "au repos, rien à redessiner");
        assert_eq!(banner.content(14), Some(&"Claude réfléchit…"));
    }

    #[test]
    fn the_same_content_again_does_not_restart_the_animation() {
        let mut banner = Banner::new();
        banner.set(Some("a"), 0);
        let settled = banner.pose(100);
        banner.set(Some("a"), 100);
        assert_eq!(banner.pose(100), settled);
    }

    #[test]
    fn a_new_text_gives_a_small_jolt_and_a_new_generation() {
        let mut banner = Banner::new();
        banner.set(Some("a"), 0);
        let first = banner.pose(100).unwrap();
        banner.set(Some("b"), 100);
        let jolt = banner.pose(100).unwrap();
        assert_ne!(jolt.generation, first.generation, "le rendu doit retracer le texte");
        assert_eq!((jolt.scale, jolt.opacity), (1.04, 1.0), "pas de nouvelle entrée : il est déjà là");
        assert_eq!(pose(&banner, 102), Some((1.0, 1.0)));
        assert_eq!(banner.content(102), Some(&"b"));
    }

    #[test]
    fn leaves_in_two_drawings_and_can_come_back() {
        let mut banner = Banner::new();
        banner.set(Some("a"), 0);
        banner.set(None, 50);
        assert_eq!(pose(&banner, 50), Some((0.97, 0.6)));
        assert_eq!(banner.content(51), Some(&"a"), "encore visible pendant qu'il s'éclipse");
        assert_eq!(pose(&banner, 52), Some((0.92, 0.3)));
        assert_eq!(banner.pose(54), None);
        assert_eq!(banner.content(54), None);
        banner.set(None, 60);
        assert_eq!(banner.pose(60), None, "le retirer deux fois ne le fait pas réapparaître");

        // revenir avec le même texte est une vraie entrée
        banner.set(Some("a"), 70);
        assert_eq!(pose(&banner, 70), Some((0.86, 0.7)));
        // et un texte qui arrive pendant la sortie entre lui aussi par une vraie entrée
        banner.set(None, 80);
        banner.set(Some("b"), 81);
        assert_eq!(pose(&banner, 81), Some((0.86, 0.7)));
    }

    #[test]
    fn sits_centred_above_her_head() {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        let capri = Rect::new(Vec2::new(1000.0, 800.0), Vec2::new(314.0, 600.0));
        let at = place_banner(capri, Vec2::new(400.0, 60.0), screen, 16.0);
        assert_eq!(at, Vec2::new(957.0, 724.0));
    }

    #[test]
    fn goes_below_her_feet_when_there_is_no_room_above() {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        let capri = Rect::new(Vec2::new(1000.0, 20.0), Vec2::new(314.0, 600.0));
        let at = place_banner(capri, Vec2::new(400.0, 60.0), screen, 16.0);
        assert_eq!(at.y, 636.0);
    }

    #[test]
    fn never_leaves_the_screen() {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        let banner = Vec2::new(800.0, 60.0);
        // collée à gauche, puis à droite
        let left =
            place_banner(Rect::new(Vec2::new(0.0, 800.0), Vec2::new(314.0, 600.0)), banner, screen, 16.0);
        assert_eq!(left.x, 16.0);
        let right =
            place_banner(Rect::new(Vec2::new(2710.0, 800.0), Vec2::new(314.0, 600.0)), banner, screen, 16.0);
        assert_eq!(right.x, 3024.0 - 16.0 - 800.0);
        // elle occupe toute la hauteur : ni dessus ni dessous, le bandeau reste en haut de l'écran
        let tall =
            place_banner(Rect::new(Vec2::new(500.0, 0.0), Vec2::new(314.0, 1964.0)), banner, screen, 16.0);
        assert_eq!(tall.y, 16.0);
        // un bandeau plus large que l'écran reste calé à gauche plutôt que de sortir
        let tiny = Rect::new(Vec2::ZERO, Vec2::new(300.0, 200.0));
        let wide =
            place_banner(Rect::new(Vec2::new(100.0, 100.0), Vec2::new(50.0, 80.0)), banner, tiny, 16.0);
        assert_eq!(wide.x, 16.0);
    }
}
