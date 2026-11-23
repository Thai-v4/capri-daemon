//! L'intro du lancement (« Up close ») : avant qu'elle n'apparaisse à sa place, elle surgit dans
//! l'écran, regarde autour d'elle, colle son visage à la vitre avec un grand sourire, puis repart.
//! Ensuite elle se matérialise à sa place ([`crate::look::Look::enter`]), puis dit bonjour.
//!
//! **Ses images.** [`COUNT`] images de [`SIZE`] px à fond transparent, du même dépôt que ses
//! planches (voir `CREDITS.md`), téléchargées par `tools/fetch-assets.sh` dans
//! `assets/capri/intro/` ([`DIR`], [`file`]). Ce module n'en contient aucune, ni aucune ligne de
//! leur code : il décide seulement laquelle montrer à chaque image d'animation, où, quand c'est
//! fini, et ce qui suit. Les décoder, les confier à la carte graphique et les lâcher est l'affaire
//! de `capri-app`.
//!
//! **Le temps.** Comme tous ses dessins (voir [`crate::anim`]) : chaque image est tenue
//! [`HOLD`] images d'animation à 24 i/s, 12 par seconde, soit 44 images (1,83 s), la cadence du
//! projet d'origine. L'horloge démarre quand la première image arrive à l'écran, pas avant : les
//! images sont décodées une à une pendant qu'elles défilent, et la première peut se faire
//! attendre ; si elle n'est pas là après [`START_WAIT`], il n'y a pas d'intro. Ensuite, une image
//! qui n'est pas prête à son heure laisse le dessin précédent tenu, et l'horloge ne s'arrête pas :
//! l'intro finit à l'heure, et l'image en retard est montrée dès qu'elle arrive (plus récente que
//! celle à l'écran, elle avance toujours), puis on demande celle qui est déjà l'heure
//! ([`Intro::shown`]).
//!
//! **Où.** L'image couvre tout l'écran ([`place`]) : mise à l'échelle la plus grande des deux
//! (celle de la largeur ou celle de la hauteur), centrée horizontalement et calée sur le bord bas ;
//! ce qui dépasse, sur les côtés ou en haut, est coupé. Aucune bande vide, quelle que soit la forme
//! de l'écran (sur un écran plus haut que 16:9, comme celui d'un MacBook Pro, ce sont les côtés).
//! Ni sa taille choisie ni la densité de l'écran n'y comptent : c'est tout l'écran qu'elle prend.
//! Rien ne s'y attrape : elle n'est pas encore là.
//!
//! **Ce qui la coupe** ([`Cut`]) et ce qui suit ([`End`]) : la cacher la coupe net, et elle reste
//! cachée ; lui parler la coupe, et elle se matérialise tout de suite ; jouée jusqu'au bout (ou
//! arrêtée là par une image illisible au milieu), elle se matérialise puis dit bonjour
//! ([`End::greets`]). Sans ses images (absentes, illisibles, d'une autre taille, refusées par la
//! carte graphique, ou la première trop lente), pas d'intro, et rien d'autre ne change : elle se
//! matérialise comme sans elle, sans bonjour. Quitter reste immédiat : rien à décider ici.

use crate::geom::{Rect, Vec2};

/// Le dossier de ses images, dans celui des images (`assets/`).
pub const DIR: &str = "capri/intro";
/// Combien d'images.
pub const COUNT: u32 = 22;
/// La taille de chacune, en pixels.
pub const SIZE: (u32, u32) = (1920, 1080);
/// Chaque image est tenue deux images d'animation : 12 dessins par seconde.
pub const HOLD: u64 = 2;
/// La première image a jusque-là pour arriver à l'écran (2 s), sinon pas d'intro : elle ne
/// retarde jamais longtemps son apparition.
pub const START_WAIT: u64 = 48;

/// Le nom du fichier de l'image n° `index` (depuis 0) : `01.png` à `22.png`.
pub fn file(index: u32) -> String {
    format!("{:02}.png", index + 1)
}

/// Où dessiner une image de `image` pixels sur un écran de `screen` pixels : à l'échelle qui lui
/// fait couvrir tout l'écran, centrée horizontalement, calée sur le bord bas, en pixels entiers.
/// Ce qui dépasse (sur les côtés, ou en haut) est hors de l'écran : coupé.
pub fn place(screen: Vec2, image: (u32, u32)) -> Rect {
    let (width, height) = (image.0.max(1) as f32, image.1.max(1) as f32);
    // jusqu'au dernier pixel touché : une taille fractionnaire est couverte jusqu'au bout
    let (screen_w, screen_h) = (screen.x.max(0.0).ceil(), screen.y.max(0.0).ceil());
    let k = (screen_w / width).max(screen_h / height);
    // jamais plus petite que l'écran, même d'un arrondi
    let size = Vec2::new((width * k).round().max(screen_w), (height * k).round().max(screen_h));
    Rect::new(Vec2::new(((screen_w - size.x) / 2.0).round(), screen_h - size.y), size)
}

/// Ce qui la coupe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// On la cache (⌃⌥C, ou son menu) : l'intro s'arrête net, et elle est cachée.
    Hidden,
    /// On lui parle (⌃⌥P, ⌃⌥D, ou son menu) : l'intro s'arrête, elle se matérialise tout de suite.
    Talked,
}

/// Pourquoi il n'y a pas d'intro du tout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// `found` images sur [`COUNT`] seulement (0 : pas même son dossier).
    Absent { found: u32 },
    /// L'image n° `index` (depuis 0) est illisible.
    Unreadable { index: u32 },
    /// L'image n° `index` n'a pas la taille attendue ([`SIZE`]).
    WrongSize { index: u32, found: (u32, u32) },
    /// La carte graphique refuse la toile où elles se peignent.
    Refused,
    /// La première image n'est pas arrivée à temps ([`START_WAIT`]).
    Late,
    /// Rien pour les décoder (le thread n'a pas pu être lancé, ou s'est arrêté avant la première).
    NoDecoder,
}

/// Comment elle a fini.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Jouée jusqu'au bout ; `shown` images différentes ont été à l'écran.
    Played { shown: u32 },
    /// Coupée, l'image `at` à l'écran (`None` : avant la première).
    Cut { why: Cut, at: Option<u32> },
    /// Une image illisible au milieu (la n° `at`) : elle s'arrête là, proprement.
    Broken { at: u32 },
    /// Pas d'intro du tout.
    Missing(Missing),
}

impl End {
    /// Dit-elle bonjour une fois là ? Seulement après l'intro : jouée jusqu'au bout, ou arrêtée
    /// par une image illisible au milieu. Ni sans intro, ni coupée (cachée, ou on lui a parlé
    /// d'abord).
    pub fn greets(self) -> bool {
        matches!(self, End::Played { .. } | End::Broken { .. })
    }

    /// Reste-t-elle cachée ? Seulement coupée en la cachant ; sinon elle se matérialise.
    pub fn hides(self) -> bool {
        matches!(self, End::Cut { why: Cut::Hidden, .. })
    }
}

/// Où en est l'intro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Elle attend sa première image depuis l'image d'animation `since`.
    Waiting { since: u64 },
    /// L'image 0 aurait été à l'écran à l'image `since` ; l'image `shown` y est.
    Playing { since: u64, shown: u32 },
    /// Finie : plus rien à montrer.
    Over,
}

/// Le déroulé de l'intro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intro {
    count: u32,
    phase: Phase,
    /// Combien d'images différentes ont été à l'écran.
    distinct: u32,
}

impl Intro {
    /// Lancée à l'image d'animation `frame`, avec `count` images : elle attend la première.
    pub fn new(count: u32, frame: u64) -> Self {
        Self { count: count.max(1), phase: Phase::Waiting { since: frame }, distinct: 0 }
    }

    /// L'image qui devrait être à l'écran à l'image `frame` : la première tant qu'elle l'attend,
    /// ensuite d'après l'horloge ; `None` une fois finie (à l'heure, même si des images ont manqué).
    pub fn due(&self, frame: u64) -> Option<u32> {
        match self.phase {
            Phase::Waiting { .. } => Some(0),
            Phase::Playing { since, .. } => {
                let drawing = frame.saturating_sub(since) / HOLD;
                (drawing < u64::from(self.count)).then_some(drawing as u32)
            }
            Phase::Over => None,
        }
    }

    /// L'image à l'écran, s'il y en a une.
    pub fn showing(&self) -> Option<u32> {
        match self.phase {
            Phase::Playing { shown, .. } => Some(shown),
            Phase::Waiting { .. } | Phase::Over => None,
        }
    }

    /// Faut-il prendre maintenant l'image que le décodeur a rendue ? Seulement quand l'heure d'une
    /// image plus récente que celle à l'écran est venue : avant, elle attend où elle est.
    pub fn wants(&self, frame: u64) -> bool {
        match (self.due(frame), self.showing()) {
            (Some(due), Some(shown)) => due > shown,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    /// L'image `index` vient d'arriver à l'écran, à l'image `frame` : la première démarre
    /// l'horloge. Rend celle à décoder ensuite : la suivante, ou, en retard, celle dont c'est déjà
    /// l'heure (les autres sont sautées) ; `None` s'il n'y en a plus, ou si `index` n'est pas plus
    /// récente que celle à l'écran (rien ne change alors).
    pub fn shown(&mut self, index: u32, frame: u64) -> Option<u32> {
        match self.phase {
            Phase::Waiting { .. } => {
                // la première : son heure est maintenant (une autre que la 0 garde son retard)
                let since = frame.saturating_sub(u64::from(index) * HOLD);
                self.phase = Phase::Playing { since, shown: index };
            }
            Phase::Playing { since, shown } if index > shown => {
                self.phase = Phase::Playing { since, shown: index };
            }
            Phase::Playing { .. } | Phase::Over => return None,
        }
        self.distinct += 1;
        let next = (index + 1).max(self.due(frame).unwrap_or(0));
        (next < self.count).then_some(next)
    }

    /// Un pas, à l'image `frame` : `Some` quand elle vient de finir, jouée jusqu'au bout, ou parce
    /// que sa première image n'est pas arrivée à temps.
    pub fn step(&mut self, frame: u64) -> Option<End> {
        let end = match self.phase {
            Phase::Waiting { since } if frame >= since + START_WAIT => End::Missing(Missing::Late),
            Phase::Playing { since, .. } if frame >= since + u64::from(self.count) * HOLD => {
                End::Played { shown: self.distinct }
            }
            _ => return None,
        };
        self.phase = Phase::Over;
        Some(end)
    }

    /// Coupée (voir [`Cut`]) : finie tout de suite.
    pub fn cut(&mut self, why: Cut) -> End {
        let at = self.showing();
        self.phase = Phase::Over;
        End::Cut { why, at }
    }

    /// L'image n° `index` est illisible : avant la première, pas d'intro ; au milieu, elle
    /// s'arrête là.
    pub fn broken(&mut self, index: u32) -> End {
        let end = match self.phase {
            Phase::Waiting { .. } => End::Missing(Missing::Unreadable { index }),
            Phase::Playing { .. } | Phase::Over => End::Broken { at: index },
        };
        self.phase = Phase::Over;
        end
    }

    /// Pas d'intro (voir [`Missing`]) : finie tout de suite.
    pub fn missing(&mut self, why: Missing) -> End {
        self.phase = Phase::Over;
        End::Missing(why)
    }

    /// Combien d'images différentes ont été à l'écran.
    pub fn distinct(&self) -> u32 {
        self.distinct
    }

    /// Combien d'images en tout.
    pub fn count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_files_are_numbered_from_one() {
        assert_eq!(file(0), "01.png");
        assert_eq!(file(9), "10.png");
        assert_eq!(file(COUNT - 1), "22.png");
    }

    #[test]
    fn it_plays_in_twos_from_its_first_image_then_ends_on_time() {
        let mut intro = Intro::new(COUNT, 100);
        // elle attend la première : rien n'est à l'écran, et c'est elle qu'on veut
        assert_eq!((intro.due(100), intro.showing(), intro.wants(100)), (Some(0), None, true));
        assert_eq!(intro.step(120), None);
        // elle arrive à l'image 130 : l'horloge part de là
        assert_eq!(intro.shown(0, 130), Some(1));
        let mut seen = vec![0];
        for frame in 130..130 + 44 {
            assert_eq!(intro.step(frame), None, "image {frame}");
            assert_eq!(intro.due(frame), Some(((frame - 130) / 2) as u32), "en deux");
            if intro.wants(frame) {
                let index = intro.due(frame).expect("pas finie");
                assert_eq!(intro.shown(index, frame), (index + 1 < COUNT).then_some(index + 1));
                seen.push(index);
            }
        }
        assert_eq!(seen, (0..COUNT).collect::<Vec<_>>(), "chaque image, une fois, dans l'ordre");
        // 44 images (1,83 s) après la première, elle est finie
        assert_eq!(intro.step(130 + 44), Some(End::Played { shown: COUNT }));
        assert_eq!((intro.due(175), intro.showing(), intro.wants(175)), (None, None, false));
        assert_eq!(intro.step(200), None, "finie une fois");
        assert_eq!(intro.shown(3, 200), None, "rien ne revient");
    }

    #[test]
    fn an_early_image_waits_and_a_late_one_holds_the_previous_then_skips_ahead() {
        let mut intro = Intro::new(COUNT, 0);
        assert_eq!(intro.shown(0, 0), Some(1));
        // la n° 1 est décodée tôt : elle attend son heure (image 2)
        assert!(!intro.wants(1));
        assert!(intro.wants(2));
        assert_eq!(intro.shown(1, 2), Some(2));
        // la n° 2 se fait attendre : la n° 1 reste à l'écran
        for frame in 4..10 {
            assert!(intro.wants(frame));
            assert_eq!(intro.showing(), Some(1), "image {frame}");
        }
        // elle arrive à l'image 10, où c'est l'heure de la n° 5 : montrée quand même (elle avance),
        // et c'est la n° 5 qu'on demande ensuite
        assert_eq!(intro.shown(2, 10), Some(5));
        assert_eq!(intro.showing(), Some(2));
        // une image plus ancienne que celle à l'écran ne change rien
        assert_eq!(intro.shown(1, 11), None);
        assert_eq!(intro.showing(), Some(2));
        assert_eq!(intro.shown(5, 12), Some(6));
        // l'horloge ne s'est pas arrêtée : elle finit à l'heure, avec moins d'images montrées
        assert_eq!(intro.step(43), None);
        assert_eq!(intro.step(44), Some(End::Played { shown: 4 }));
    }

    #[test]
    fn the_last_image_asks_for_nothing_more() {
        let mut intro = Intro::new(3, 0);
        assert_eq!(intro.shown(0, 0), Some(1));
        assert_eq!(intro.shown(2, 5), None, "la dernière");
        assert_eq!(intro.due(5), Some(2));
        assert_eq!(intro.step(6), Some(End::Played { shown: 2 }));
    }

    #[test]
    fn without_its_first_image_in_time_there_is_no_intro() {
        let mut intro = Intro::new(COUNT, 10);
        assert_eq!(intro.step(10 + START_WAIT - 1), None);
        assert_eq!(intro.step(10 + START_WAIT), Some(End::Missing(Missing::Late)));
        assert_eq!(intro.due(10 + START_WAIT), None);
    }

    #[test]
    fn hiding_or_talking_cuts_it_and_says_what_follows() {
        let mut intro = Intro::new(COUNT, 0);
        assert_eq!(intro.cut(Cut::Hidden), End::Cut { why: Cut::Hidden, at: None });
        assert_eq!(intro.due(0), None);
        let mut intro = Intro::new(COUNT, 0);
        intro.shown(0, 0);
        intro.shown(1, 2);
        assert_eq!(intro.cut(Cut::Talked), End::Cut { why: Cut::Talked, at: Some(1) });
        assert_eq!(intro.step(1000), None, "coupée, elle ne finit pas une seconde fois");

        // ce qui suit : cachée, elle le reste ; sinon elle se matérialise, et ne dit bonjour
        // qu'après l'intro
        let hidden = End::Cut { why: Cut::Hidden, at: Some(4) };
        let talked = End::Cut { why: Cut::Talked, at: Some(4) };
        assert!(hidden.hides() && !hidden.greets());
        assert!(!talked.hides() && !talked.greets());
        for end in [End::Played { shown: 22 }, End::Broken { at: 9 }] {
            assert!(end.greets() && !end.hides(), "{end:?}");
        }
        for why in [
            Missing::Absent { found: 3 },
            Missing::Unreadable { index: 0 },
            Missing::WrongSize { index: 7, found: (1280, 720) },
            Missing::Refused,
            Missing::Late,
            Missing::NoDecoder,
        ] {
            let end = End::Missing(why);
            assert!(!end.greets() && !end.hides(), "sans intro, comme sans elle : {end:?}");
        }
    }

    #[test]
    fn an_unreadable_image_means_no_intro_at_first_and_stops_it_halfway() {
        let mut intro = Intro::new(COUNT, 0);
        assert_eq!(intro.broken(0), End::Missing(Missing::Unreadable { index: 0 }));
        let mut intro = Intro::new(COUNT, 0);
        intro.shown(0, 0);
        assert_eq!(intro.broken(9), End::Broken { at: 9 });
        assert_eq!(intro.due(20), None);
        let mut intro = Intro::new(COUNT, 0);
        assert_eq!(intro.missing(Missing::Refused), End::Missing(Missing::Refused));
        assert_eq!(intro.step(1), None);
    }

    #[test]
    fn it_covers_the_screen_centred_and_resting_on_its_bottom_edge() {
        let at = |w: f32, h: f32| place(Vec2::new(w, h), SIZE);
        // un écran 16:9 : tout l'écran, rien de coupé
        assert_eq!(at(1920.0, 1080.0), Rect::new(Vec2::ZERO, Vec2::new(1920.0, 1080.0)));
        assert_eq!(at(3840.0, 2160.0), Rect::new(Vec2::ZERO, Vec2::new(3840.0, 2160.0)));
        // plus haut que 16:9, un MacBook Pro 14" (3024 × 1964) : toute la hauteur, les côtés coupés
        assert_eq!(at(3024.0, 1964.0), Rect::new(Vec2::new(-234.0, 0.0), Vec2::new(3492.0, 1964.0)));
        // 16:10 : de même
        assert_eq!(at(1440.0, 900.0), Rect::new(Vec2::new(-80.0, 0.0), Vec2::new(1600.0, 900.0)));
        // un écran en hauteur : toute la hauteur, centrée, les côtés coupés
        assert_eq!(at(1080.0, 1920.0), Rect::new(Vec2::new(-1167.0, 0.0), Vec2::new(3413.0, 1920.0)));
        // plus large que 16:9 : toute la largeur, calée en bas, le haut coupé
        assert_eq!(at(3440.0, 1440.0), Rect::new(Vec2::new(0.0, -495.0), Vec2::new(3440.0, 1935.0)));
        // un écran vide ne fait pas de nombres absurdes
        assert_eq!(at(0.0, 0.0), Rect::new(Vec2::ZERO, Vec2::ZERO));
        let flat = at(0.0, 100.0);
        assert!(flat.size.y == 100.0 && flat.min.y == 0.0 && flat.min.x <= 0.0 && flat.max().x >= 0.0);
        // une taille fractionnaire est couverte jusqu'au bout
        let odd = at(1000.4, 700.6);
        assert!(odd.min.x <= 0.0 && odd.min.y <= 0.0, "{odd:?}");
        assert!(odd.max().x >= 1000.4 && odd.max().y >= 700.6, "{odd:?}");
    }

    #[test]
    fn it_always_covers_the_whole_screen_in_whole_pixels() {
        let mut sizes: Vec<f32> = (0..4000).step_by(37).map(|v| v as f32).collect();
        sizes.extend([0.5, 1.0, 1.5, 799.25, 1919.9, 1080.1]);
        for &w in &sizes {
            for &h in &sizes {
                let screen = Vec2::new(w, h);
                let rect = place(screen, SIZE);
                // aucune bande vide : l'image couvre tout l'écran
                assert!(rect.min.x <= 0.0 && rect.min.y <= 0.0, "{w}×{h} : {rect:?}");
                assert!(rect.max().x >= w && rect.max().y >= h, "{w}×{h} : {rect:?}");
                // calée en bas, sur le dernier pixel de l'écran
                assert_eq!(rect.max().y, h.ceil(), "{w}×{h} : {rect:?}");
                // l'une de ses dimensions est celle de l'écran, et elle reste en 16:9 (au pixel près)
                assert!(rect.size.x == w.ceil() || rect.size.y == h.ceil(), "{w}×{h} : {rect:?}");
                assert!((rect.size.x * 1080.0 / 1920.0 - rect.size.y).abs() <= 1.0, "{w}×{h} : {rect:?}");
                // centrée, au pixel près : autant de coupé à gauche qu'à droite
                assert!((-rect.min.x - (rect.max().x - w.ceil())).abs() <= 1.0, "{w}×{h} : {rect:?}");
                for v in [rect.min.x, rect.min.y, rect.size.x, rect.size.y] {
                    assert_eq!(v, v.round(), "{w}×{h} : {rect:?}");
                }
            }
        }
    }
}
