//! Ses intros du lancement : avant qu'elle n'apparaisse à sa place, une intro, tirée au sort à
//! chaque lancement parmi celles que ses images permettent ([`LIST`], [`draw`]). Ensuite elle est
//! là ([`crate::look::Look::enter`]), et elle dit bonjour.
//!
//! **La liste** ([`LIST`]) est écrite ici, et seulement ici : chaque intro a son nom court (retenu
//! d'un lancement à l'autre), son nom pour le journal, son poids au tirage, et sa façon de se jouer
//! ([`Play`]) :
//! - **en images plein écran** ([`Film`]) : « Up close » ([`UP_CLOSE`], 22 images à fond
//!   transparent, du même dépôt que ses planches, voir `CREDITS.md`), et le coucou de
//!   [`coucou`] (9 images d'un autre dépôt, à retirer en quelques lignes : voir ce module) ;
//! - **jouée par elle**, avec ses propres dessins ([`act`]) : elle tombe du ciel, elle arrive en
//!   marchant, elle se rassemble ;
//! - **rien** : elle se matérialise simplement, comme sans intro, mais elle dit bonjour.
//!
//! **Le tirage** ([`draw`]) : au hasard d'après les poids (« rien » est rare), parmi les intros
//! possibles avec les images présentes (« rien » l'est toujours), et **jamais deux lancements de
//! suite la même** quand une autre est possible : la dernière est retenue par `capri-app` (son nom
//! court dans `intro.json`). Le hasard vient de la graine de sa vie, mêlée à [`DRAW_SEED`]. « Voir
//! une intro », dans son menu, les montre l'une après l'autre, dans l'ordre de la liste ([`next`]).
//!
//! **Une intro en images.** Ce module n'en contient aucune, ni aucune ligne de leur code : il
//! décide seulement laquelle montrer à chaque image d'animation (sa feuille d'exposition,
//! [`Film::sheet`]), où, quand c'est fini, et ce qui suit. Les décoder, les confier à la carte
//! graphique et les lâcher est l'affaire de `capri-app`.
//!
//! **Le temps.** Comme tous ses dessins (voir [`crate::anim`]) : chaque image est tenue un nombre
//! entier d'images d'animation à 24 i/s (« Up close » : [`HOLD`] chacune, 12 par seconde, soit 44
//! images, 1,83 s, la cadence du projet d'origine). L'horloge démarre quand la première image
//! arrive à l'écran, pas avant : les images sont décodées une à une pendant qu'elles défilent, et la
//! première peut se faire attendre ; si elle n'est pas là après [`START_WAIT`], il n'y a pas
//! d'intro. Ensuite, une image qui n'est pas prête à son heure laisse le dessin précédent tenu, et
//! l'horloge ne s'arrête pas : l'intro finit à l'heure, et l'image en retard est montrée dès
//! qu'elle arrive (plus récente que celle à l'écran, elle avance toujours), puis on demande celle
//! qui est déjà l'heure ([`Intro::shown`]).
//!
//! **Où.** L'image couvre tout l'écran ([`place`]) : mise à l'échelle la plus grande des deux
//! (celle de la largeur ou celle de la hauteur), centrée horizontalement et calée sur le bord bas ;
//! ce qui dépasse, sur les côtés ou en haut, est coupé. Aucune bande vide, quelle que soit la forme
//! de l'écran (sur un écran plus haut que 16:9, comme celui d'un MacBook Pro, ce sont les côtés).
//! Ni sa taille choisie ni la densité de l'écran n'y comptent : c'est tout l'écran qu'elle prend.
//! Rien ne s'y attrape : elle n'est pas encore là.
//!
//! **Ce qui la coupe** ([`Cut`]) et ce qui suit ([`End`]) : la cacher la coupe net, et elle reste
//! cachée ; lui parler la coupe, et elle se matérialise tout de suite ; dans une intro jouée par
//! elle, l'attraper la coupe aussi, et elle est tenue (sans saut, même en partie hors de l'écran),
//! et une autre taille la coupe comme quand on lui parle. Jouée jusqu'au bout (ou arrêtée là par une
//! image illisible au milieu), elle est là, puis dit bonjour ([`End::greets`]). Sans ses images
//! (absentes, illisibles, d'une autre taille, refusées par la carte graphique, ou la première trop
//! lente), pas d'intro, et rien d'autre ne change : elle se matérialise comme sans elle, sans
//! bonjour. Quitter reste immédiat : rien à décider ici.

pub mod act;
pub mod coucou;

pub use act::Act;

use crate::geom::{Rect, Vec2};
use crate::roam::Rng;

/// Comment une intro se joue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Play {
    /// Des images en plein écran, décodées une à une (voir [`Film`]).
    Film(&'static Film),
    /// Jouée par elle, avec ses propres dessins (voir [`act`]).
    Act(Act),
    /// Rien : elle se matérialise simplement (`Look::enter`), comme sans intro, et dit bonjour.
    Plain,
}

/// Une intro de la liste.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    /// Son nom court, retenu d'un lancement à l'autre (`intro.json`) : des lettres minuscules.
    pub key: &'static str,
    /// Son nom pour le journal, à nous.
    pub name: &'static str,
    /// Son poids au tirage.
    pub weight: u32,
    pub play: Play,
}

/// Ses intros, dans l'ordre où « Voir une intro » les montre. **Écrite ici seulement** : en
/// ajouter une, ou en retirer une, ne se fait qu'ici (et dans ce qu'elle joue).
pub const LIST: &[Entry] = &[
    Entry { key: "upclose", name: "Up close", weight: 4, play: Play::Film(&UP_CLOSE) },
    Entry { key: "coucou", name: "coucou", weight: 3, play: Play::Film(&coucou::FILM) },
    Entry { key: "tombe", name: "tombe du ciel", weight: 3, play: Play::Act(Act::Fall) },
    Entry { key: "marche", name: "arrive en marchant", weight: 3, play: Play::Act(Act::Walk) },
    Entry { key: "rassemble", name: "se rassemble", weight: 3, play: Play::Act(Act::Gather) },
    Entry { key: "rien", name: "rien", weight: 1, play: Play::Plain },
];

/// La graine du tirage : celle de sa vie, mêlée à ce nombre (« intro »), pour ne rien changer au
/// hasard de ses trajets ni de son bonjour.
pub const DRAW_SEED: u64 = 0x696e_7472_6f00;

/// L'intro de nom court `key`, si elle est dans la liste.
pub fn by_key(key: &str) -> Option<&'static Entry> {
    LIST.iter().find(|entry| entry.key == key)
}

/// « Rien » : toujours possible.
pub fn plain() -> &'static Entry {
    LIST.iter().find(|entry| entry.play == Play::Plain).unwrap_or(&LIST[LIST.len() - 1])
}

/// L'intro du lancement, tirée avec la graine `seed` (celle de sa vie) : au hasard d'après leurs
/// poids, parmi celles que `possible` permet (« rien » l'est toujours), jamais `last` (la
/// précédente, par son nom court) quand une autre est possible. Un nom inconnu ne contraint rien.
pub fn draw(seed: u64, possible: impl Fn(&Entry) -> bool, last: Option<&str>) -> &'static Entry {
    let allowed = |entry: &Entry| entry.play == Play::Plain || possible(entry);
    let others = LIST.iter().filter(|entry| allowed(entry) && Some(entry.key) != last).count();
    let weights: Vec<f32> = LIST
        .iter()
        .map(|entry| {
            let open = allowed(entry) && (others == 0 || Some(entry.key) != last);
            if open { entry.weight as f32 } else { 0.0 }
        })
        .collect();
    let mut rng = Rng::new(seed ^ DRAW_SEED);
    rng.pick(&weights).map_or_else(plain, |index| &LIST[index])
}

/// « Voir une intro » : celle qui suit `after` (par son nom court ; aucune ou inconnue, la
/// première) dans l'ordre de la liste, en sautant celles que `possible` ne permet pas, et en
/// repartant du début après la dernière. « Rien » en fait partie, pour qu'on puisse toutes les voir.
pub fn next(after: Option<&str>, possible: impl Fn(&Entry) -> bool) -> &'static Entry {
    let from = after.and_then(|key| LIST.iter().position(|entry| entry.key == key)).map_or(0, |at| at + 1);
    (0..LIST.len())
        .map(|offset| &LIST[(from + offset) % LIST.len()])
        .find(|entry| entry.play == Play::Plain || possible(entry))
        .unwrap_or_else(plain)
}

/// Une intro en images plein écran : ses fichiers, leur taille, et sa feuille d'exposition.
#[derive(Debug, PartialEq, Eq)]
pub struct Film {
    /// Le dossier de ses images, dans celui des images (`assets/`).
    pub dir: &'static str,
    /// Ses images, toutes de la même taille.
    pub files: &'static [&'static str],
    /// Leur taille, en pixels.
    pub size: (u32, u32),
    /// Sa feuille d'exposition : quelle image (son numéro dans [`Film::files`]), tenue combien
    /// d'images d'animation à 24 i/s. Jamais deux fois la même image de suite (elle serait
    /// décodée deux fois pour rien) ; jamais une tenue de 0.
    pub sheet: &'static [(u8, u8)],
}

impl Film {
    /// Combien de dessins dans sa feuille.
    pub fn len(&self) -> u32 {
        self.sheet.len() as u32
    }

    /// Une feuille n'est jamais vide (un essai le vérifie pour chacune).
    pub fn is_empty(&self) -> bool {
        self.sheet.is_empty()
    }

    /// Combien d'images d'animation elle dure.
    pub fn frames(&self) -> u64 {
        self.sheet.iter().map(|&(_, hold)| u64::from(hold.max(1))).sum()
    }

    /// L'image d'animation (depuis son début) où commence le dessin n° `position`.
    pub fn start(&self, position: u32) -> u64 {
        self.sheet.iter().take(position as usize).map(|&(_, hold)| u64::from(hold.max(1))).sum()
    }

    /// Le dessin montré `elapsed` images après son début ; `None` une fois finie.
    pub fn at(&self, elapsed: u64) -> Option<u32> {
        let mut left = elapsed;
        for (position, &(_, hold)) in self.sheet.iter().enumerate() {
            let hold = u64::from(hold.max(1));
            if left < hold {
                return Some(position as u32);
            }
            left -= hold;
        }
        None
    }

    /// Le numéro (dans [`Film::files`]) de l'image du dessin n° `position`.
    pub fn image(&self, position: u32) -> Option<u8> {
        self.sheet.get(position as usize).map(|&(image, _)| image)
    }
}

/// Le dossier des images de « Up close », dans celui des images (`assets/`).
pub const DIR: &str = "capri/intro";
/// Combien d'images dans « Up close ».
pub const COUNT: u32 = 22;
/// La taille de chacune, en pixels.
pub const SIZE: (u32, u32) = (1920, 1080);
/// Chaque image de « Up close » est tenue deux images d'animation : 12 dessins par seconde.
pub const HOLD: u64 = 2;
/// La première image a jusque-là pour arriver à l'écran (2 s), sinon pas d'intro : elle ne
/// retarde jamais longtemps son apparition.
pub const START_WAIT: u64 = 48;

/// Le nom du fichier de l'image n° `index` (depuis 0) de « Up close » : `01.png` à `22.png`.
pub fn file(index: u32) -> String {
    format!("{:02}.png", index + 1)
}

const UP_CLOSE_FILES: [&str; COUNT as usize] = [
    "01.png", "02.png", "03.png", "04.png", "05.png", "06.png", "07.png", "08.png", "09.png", "10.png",
    "11.png", "12.png", "13.png", "14.png", "15.png", "16.png", "17.png", "18.png", "19.png", "20.png",
    "21.png", "22.png",
];

/// Chaque image une fois, dans l'ordre, tenue [`HOLD`] images.
const fn in_twos<const N: usize>() -> [(u8, u8); N] {
    let mut sheet = [(0u8, HOLD as u8); N];
    let mut index = 0;
    while index < N {
        sheet[index].0 = index as u8;
        index += 1;
    }
    sheet
}

const UP_CLOSE_SHEET: [(u8, u8); COUNT as usize] = in_twos();

/// « Up close » : elle surgit dans l'écran, regarde autour d'elle, colle son visage à la vitre avec
/// un grand sourire, puis repart.
pub const UP_CLOSE: Film = Film { dir: DIR, files: &UP_CLOSE_FILES, size: SIZE, sheet: &UP_CLOSE_SHEET };

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
    /// On l'attrape, dans une intro jouée par elle : elle est tenue, comme d'habitude.
    Grabbed,
    /// Sa taille change (son menu, la molette, la densité de l'écran), dans une intro jouée par
    /// elle : comme quand on lui parle, elle est à sa place tout de suite.
    Resized,
}

/// Pourquoi il n'y a pas d'intro du tout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    /// `found` images sur toutes celles de l'intro seulement (0 : pas même son dossier).
    Absent { found: u32 },
    /// L'image n° `index` (depuis 0, dans [`Film::files`]) est illisible.
    Unreadable { index: u32 },
    /// L'image n° `index` n'a pas la taille attendue ([`Film::size`]).
    WrongSize { index: u32, found: (u32, u32) },
    /// La carte graphique refuse la toile où elles se peignent.
    Refused,
    /// La première image n'est pas arrivée à temps ([`START_WAIT`]).
    Late,
    /// Rien pour les décoder (le thread n'a pas pu être lancé, ou s'est arrêté avant la première).
    NoDecoder,
}

/// Comment elle a fini. Pour une intro jouée par elle, les nombres sont des images d'animation :
/// combien elle a duré, jouée ou coupée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Jouée jusqu'au bout ; `shown` dessins différents ont été à l'écran (images d'animation,
    /// pour une intro jouée par elle).
    Played { shown: u32 },
    /// Coupée, le dessin `at` à l'écran (`None` : avant le premier).
    Cut { why: Cut, at: Option<u32> },
    /// Une image illisible au milieu (le dessin n° `at`) : elle s'arrête là, proprement.
    Broken { at: u32 },
    /// Pas d'intro du tout.
    Missing(Missing),
}

impl End {
    /// Dit-elle bonjour une fois là ? Seulement après l'intro : jouée jusqu'au bout, ou arrêtée
    /// par une image illisible au milieu. Ni sans intro, ni coupée (cachée, on lui a parlé, ou on
    /// l'a attrapée d'abord).
    pub fn greets(self) -> bool {
        matches!(self, End::Played { .. } | End::Broken { .. })
    }

    /// Reste-t-elle cachée ? Seulement coupée en la cachant ; sinon elle est là.
    pub fn hides(self) -> bool {
        matches!(self, End::Cut { why: Cut::Hidden, .. })
    }
}

/// Où en est une intro en images.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Elle attend son premier dessin depuis l'image d'animation `since`.
    Waiting { since: u64 },
    /// Le dessin 0 aurait été à l'écran à l'image `since` ; le dessin `shown` y est.
    Playing { since: u64, shown: u32 },
    /// Finie : plus rien à montrer.
    Over,
}

/// Le déroulé d'une intro en images : quel dessin de sa feuille montrer, et quand c'est fini.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intro {
    film: &'static Film,
    phase: Phase,
    /// Combien de dessins différents ont été à l'écran.
    distinct: u32,
}

impl Intro {
    /// Lancée à l'image d'animation `frame` : elle attend son premier dessin.
    pub fn new(film: &'static Film, frame: u64) -> Self {
        Self { film, phase: Phase::Waiting { since: frame }, distinct: 0 }
    }

    /// Le dessin qui devrait être à l'écran à l'image `frame` : le premier tant qu'elle l'attend,
    /// ensuite d'après l'horloge ; `None` une fois finie (à l'heure, même si des dessins ont manqué).
    pub fn due(&self, frame: u64) -> Option<u32> {
        match self.phase {
            Phase::Waiting { .. } => Some(0),
            Phase::Playing { since, .. } => self.film.at(frame.saturating_sub(since)),
            Phase::Over => None,
        }
    }

    /// Le dessin à l'écran, s'il y en a un.
    pub fn showing(&self) -> Option<u32> {
        match self.phase {
            Phase::Playing { shown, .. } => Some(shown),
            Phase::Waiting { .. } | Phase::Over => None,
        }
    }

    /// Faut-il prendre maintenant le dessin que le décodeur a rendu ? Seulement quand l'heure d'un
    /// dessin plus récent que celui à l'écran est venue : avant, elle attend où elle est.
    pub fn wants(&self, frame: u64) -> bool {
        match (self.due(frame), self.showing()) {
            (Some(due), Some(shown)) => due > shown,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    /// Le dessin `index` vient d'arriver à l'écran, à l'image `frame` : le premier démarre
    /// l'horloge. Rend celui à décoder ensuite : le suivant, ou, en retard, celui dont c'est déjà
    /// l'heure (les autres sont sautés) ; `None` s'il n'y en a plus, ou si `index` n'est pas plus
    /// récent que celui à l'écran (rien ne change alors).
    pub fn shown(&mut self, index: u32, frame: u64) -> Option<u32> {
        match self.phase {
            Phase::Waiting { .. } => {
                // le premier : son heure est maintenant (un autre que le 0 garde son retard)
                let since = frame.saturating_sub(self.film.start(index));
                self.phase = Phase::Playing { since, shown: index };
            }
            Phase::Playing { since, shown } if index > shown => {
                self.phase = Phase::Playing { since, shown: index };
            }
            Phase::Playing { .. } | Phase::Over => return None,
        }
        self.distinct += 1;
        let next = (index + 1).max(self.due(frame).unwrap_or(0));
        (next < self.film.len()).then_some(next)
    }

    /// Un pas, à l'image `frame` : `Some` quand elle vient de finir, jouée jusqu'au bout, ou parce
    /// que son premier dessin n'est pas arrivé à temps.
    pub fn step(&mut self, frame: u64) -> Option<End> {
        let end = match self.phase {
            Phase::Waiting { since } if frame >= since + START_WAIT => End::Missing(Missing::Late),
            Phase::Playing { since, .. } if frame >= since + self.film.frames() => {
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

    /// Le dessin n° `index` est illisible : avant le premier, pas d'intro ; au milieu, elle
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

    /// Combien de dessins différents ont été à l'écran.
    pub fn distinct(&self) -> u32 {
        self.distinct
    }

    /// Combien de dessins dans sa feuille.
    pub fn count(&self) -> u32 {
        self.film.len()
    }

    /// Ce qu'elle joue.
    pub fn film(&self) -> &'static Film {
        self.film
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::FRAME_RATE;

    #[test]
    fn its_files_are_numbered_from_one() {
        assert_eq!(file(0), "01.png");
        assert_eq!(file(9), "10.png");
        assert_eq!(file(COUNT - 1), "22.png");
        for index in 0..COUNT {
            assert_eq!(UP_CLOSE.files[index as usize], file(index));
        }
    }

    #[test]
    fn up_close_shows_each_image_once_in_twos() {
        assert_eq!((UP_CLOSE.len(), UP_CLOSE.frames()), (COUNT, 44));
        for position in 0..COUNT {
            assert_eq!(UP_CLOSE.image(position), Some(position as u8));
            assert_eq!(UP_CLOSE.start(position), u64::from(position) * HOLD);
        }
        assert_eq!(UP_CLOSE.at(0), Some(0));
        assert_eq!(UP_CLOSE.at(43), Some(21));
        assert_eq!(UP_CLOSE.at(44), None);
    }

    #[test]
    fn every_film_is_well_formed() {
        for entry in LIST {
            let Play::Film(film) = entry.play else { continue };
            assert!(!film.is_empty() && !film.files.is_empty(), "{}", entry.key);
            assert!(film.size.0 > 0 && film.size.1 > 0, "{}", entry.key);
            for pair in film.sheet.windows(2) {
                assert_ne!(pair[0].0, pair[1].0, "{} : deux fois la même image de suite", entry.key);
            }
            for &(image, hold) in film.sheet {
                assert!((image as usize) < film.files.len() && hold >= 1, "{}", entry.key);
            }
            // chaque image sert
            for image in 0..film.files.len() {
                assert!(
                    film.sheet.iter().any(|&(shown, _)| shown as usize == image),
                    "{} {image}",
                    entry.key
                );
            }
            // une intro, pas un film : 4 s au plus
            assert!(film.frames() as f64 / FRAME_RATE <= 4.0, "{}", entry.key);
        }
    }

    #[test]
    fn the_list_names_each_intro_once_with_short_keys() {
        assert!(LIST.len() >= 2);
        for (index, entry) in LIST.iter().enumerate() {
            assert!(!entry.key.is_empty() && entry.key.chars().all(|c| c.is_ascii_lowercase()), "{entry:?}");
            assert!(!entry.name.is_empty() && entry.weight > 0, "{entry:?}");
            assert!(LIST[index + 1..].iter().all(|other| other.key != entry.key && other.name != entry.name));
            assert_eq!(by_key(entry.key), Some(entry));
        }
        assert_eq!(by_key("inconnue"), None);
        assert_eq!(plain().play, Play::Plain);
        // « rien » est la plus rare
        assert!(LIST.iter().all(|entry| entry.play == Play::Plain || entry.weight > plain().weight));
        assert_eq!(LIST.iter().filter(|entry| entry.play == Play::Plain).count(), 1);
    }

    #[test]
    fn the_draw_never_gives_the_same_twice_in_a_row_when_another_is_possible() {
        let mut counts = vec![0usize; LIST.len()];
        for seed in 0..4000u64 {
            for last in LIST.iter().map(|entry| Some(entry.key)).chain([None, Some("inconnue")]) {
                let drawn = draw(seed, |_| true, last);
                assert_ne!(Some(drawn.key), last, "graine {seed}");
                if last.is_none() {
                    counts[LIST.iter().position(|entry| entry == drawn).unwrap()] += 1;
                }
            }
        }
        // chacune sort, à peu près selon son poids ; « rien » rarement
        let total: u32 = LIST.iter().map(|entry| entry.weight).sum();
        for (entry, &count) in LIST.iter().zip(&counts) {
            let expected = 4000.0 * f64::from(entry.weight) / f64::from(total);
            assert!((count as f64 - expected).abs() < expected * 0.25 + 20.0, "{}: {count}", entry.key);
        }
        // la même graine donne la même
        assert_eq!(draw(7, |_| true, None), draw(7, |_| true, None));
    }

    #[test]
    fn only_possible_intros_are_drawn_and_nothing_remains_without_images() {
        let films_only = |entry: &Entry| matches!(entry.play, Play::Film(_));
        for seed in 0..500u64 {
            let drawn = draw(seed, |_| false, None);
            assert_eq!(drawn.play, Play::Plain, "sans aucune image, il reste « rien »");
            // la dernière était « rien » : rien d'autre n'est possible, « rien » encore
            assert_eq!(draw(seed, |_| false, Some("rien")).play, Play::Plain);
            let drawn = draw(seed, films_only, None);
            assert!(films_only(drawn) || drawn.play == Play::Plain, "{drawn:?}");
            // une seule possible en plus de « rien », et c'était la dernière : « rien »
            let up_close = |entry: &Entry| entry.key == "upclose";
            assert_eq!(draw(seed, up_close, Some("upclose")).play, Play::Plain);
        }
    }

    #[test]
    fn see_an_intro_goes_through_the_list_in_order() {
        let all = |_: &Entry| true;
        let mut key = None;
        let mut seen = Vec::new();
        for _ in 0..LIST.len() * 2 {
            let entry = next(key, all);
            seen.push(entry.key);
            key = Some(entry.key);
        }
        let keys: Vec<&str> = LIST.iter().map(|entry| entry.key).collect();
        assert_eq!(seen[..LIST.len()], keys[..]);
        assert_eq!(seen[LIST.len()..], keys[..], "puis de nouveau depuis le début");
        // sans images : « rien », toujours
        assert_eq!(next(None, |_| false).play, Play::Plain);
        assert_eq!(next(Some("rien"), |_| false).play, Play::Plain);
        // une inconnue : la première
        assert_eq!(next(Some("inconnue"), all), &LIST[0]);
        // celles qui ne sont pas possibles sont sautées
        let acts = |entry: &Entry| matches!(entry.play, Play::Act(_));
        assert!(matches!(next(None, acts).play, Play::Act(_)));
    }

    /// Une petite intro d'essai : trois images, la n° 1 tenue trois images.
    static UNEVEN: Film = Film {
        dir: "essai",
        files: &["a.png", "b.png", "c.png"],
        size: (8, 6),
        sheet: &[(0, 2), (1, 3), (2, 2), (1, 1)],
    };

    #[test]
    fn it_plays_in_twos_from_its_first_image_then_ends_on_time() {
        let mut intro = Intro::new(&UP_CLOSE, 100);
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
    fn uneven_holds_follow_the_sheet() {
        let mut intro = Intro::new(&UNEVEN, 0);
        assert_eq!(intro.shown(0, 10), Some(1));
        let due: Vec<Option<u32>> = (10..19).map(|frame| intro.due(frame)).collect();
        assert_eq!(
            due,
            [Some(0), Some(0), Some(1), Some(1), Some(1), Some(2), Some(2), Some(3), None],
            "tenues de 2, 3, 2 et 1"
        );
        assert_eq!(intro.count(), 4);
        assert_eq!(intro.step(17), None);
        assert_eq!(intro.step(18), Some(End::Played { shown: 1 }));
        // un premier dessin arrivé en retard garde son retard
        let mut late = Intro::new(&UNEVEN, 0);
        assert_eq!(late.shown(2, 20), Some(3));
        assert_eq!(late.due(20), Some(2));
        // son heure était l'image 15 (2 + 3 images plus tôt) : finie 8 images après, à la 23
        assert_eq!(late.step(22), None);
        assert_eq!(late.step(23), Some(End::Played { shown: 1 }));
    }

    #[test]
    fn an_early_image_waits_and_a_late_one_holds_the_previous_then_skips_ahead() {
        let mut intro = Intro::new(&UP_CLOSE, 0);
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
        let mut intro = Intro::new(&UNEVEN, 0);
        assert_eq!(intro.shown(0, 0), Some(1));
        assert_eq!(intro.shown(3, 7), None, "la dernière");
        assert_eq!(intro.due(7), Some(3));
        assert_eq!(intro.step(8), Some(End::Played { shown: 2 }));
    }

    #[test]
    fn without_its_first_image_in_time_there_is_no_intro() {
        let mut intro = Intro::new(&UP_CLOSE, 10);
        assert_eq!(intro.step(10 + START_WAIT - 1), None);
        assert_eq!(intro.step(10 + START_WAIT), Some(End::Missing(Missing::Late)));
        assert_eq!(intro.due(10 + START_WAIT), None);
    }

    #[test]
    fn hiding_talking_or_grabbing_cuts_it_and_says_what_follows() {
        let mut intro = Intro::new(&UP_CLOSE, 0);
        assert_eq!(intro.cut(Cut::Hidden), End::Cut { why: Cut::Hidden, at: None });
        assert_eq!(intro.due(0), None);
        let mut intro = Intro::new(&UP_CLOSE, 0);
        intro.shown(0, 0);
        intro.shown(1, 2);
        assert_eq!(intro.cut(Cut::Talked), End::Cut { why: Cut::Talked, at: Some(1) });
        assert_eq!(intro.step(1000), None, "coupée, elle ne finit pas une seconde fois");

        // ce qui suit : cachée, elle le reste ; sinon elle est là, et ne dit bonjour qu'après
        // l'intro
        let hidden = End::Cut { why: Cut::Hidden, at: Some(4) };
        let talked = End::Cut { why: Cut::Talked, at: Some(4) };
        let grabbed = End::Cut { why: Cut::Grabbed, at: Some(4) };
        let resized = End::Cut { why: Cut::Resized, at: Some(4) };
        assert!(hidden.hides() && !hidden.greets());
        assert!(!talked.hides() && !talked.greets());
        assert!(!grabbed.hides() && !grabbed.greets());
        assert!(!resized.hides() && !resized.greets());
        for end in [End::Played { shown: 22 }, End::Played { shown: 0 }, End::Broken { at: 9 }] {
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
        let mut intro = Intro::new(&UP_CLOSE, 0);
        assert_eq!(intro.broken(0), End::Missing(Missing::Unreadable { index: 0 }));
        let mut intro = Intro::new(&UP_CLOSE, 0);
        intro.shown(0, 0);
        assert_eq!(intro.broken(9), End::Broken { at: 9 });
        assert_eq!(intro.due(20), None);
        let mut intro = Intro::new(&UP_CLOSE, 0);
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
        for image in LIST.iter().filter_map(|entry| match entry.play {
            Play::Film(film) => Some(film.size),
            _ => None,
        }) {
            let ratio = image.1 as f32 / image.0 as f32;
            for &w in &sizes {
                for &h in &sizes {
                    let screen = Vec2::new(w, h);
                    let rect = place(screen, image);
                    // aucune bande vide : l'image couvre tout l'écran
                    assert!(rect.min.x <= 0.0 && rect.min.y <= 0.0, "{w}×{h} : {rect:?}");
                    assert!(rect.max().x >= w && rect.max().y >= h, "{w}×{h} : {rect:?}");
                    // calée en bas, sur le dernier pixel de l'écran
                    assert_eq!(rect.max().y, h.ceil(), "{w}×{h} : {rect:?}");
                    // l'une de ses dimensions est celle de l'écran, et elle garde sa forme (au pixel près)
                    assert!(rect.size.x == w.ceil() || rect.size.y == h.ceil(), "{w}×{h} : {rect:?}");
                    assert!((rect.size.x * ratio - rect.size.y).abs() <= 1.0, "{w}×{h} : {rect:?}");
                    // centrée, au pixel près : autant de coupé à gauche qu'à droite
                    assert!((-rect.min.x - (rect.max().x - w.ceil())).abs() <= 1.0, "{w}×{h} : {rect:?}");
                    for v in [rect.min.x, rect.min.y, rect.size.x, rect.size.y] {
                        assert_eq!(v, v.round(), "{w}×{h} : {rect:?}");
                    }
                }
            }
        }
    }
}
