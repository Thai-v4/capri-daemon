//! Ses intros du lancement : avant qu'elle n'apparaisse à sa place, **toujours le même film**,
//! « de tout près » ([`FILM`], ses 22 images en plein écran, avec un effet de glitch calculé sur
//! ses dessins : [`glitch`]), **puis une entrée** ([`LIST`]), tirée au sort à chaque lancement
//! parmi celles que ses planches permettent ([`draw`]). Ensuite elle est là, et elle dit bonjour.
//!
//! **La liste des entrées** ([`LIST`]) est écrite ici, et seulement ici : chaque entrée a son nom
//! court (retenu d'un lancement à l'autre), son nom pour le journal, son poids au tirage, et sa
//! façon de se jouer ([`Play`]) : **jouée par elle**, avec ses propres dessins ([`act`] : elle tombe
//! du ciel, elle arrive en marchant, elle se rassemble), ou **rien de plus** : elle se matérialise
//! simplement. En retirer une tient en une ligne.
//!
//! **Le tirage** ([`draw`]) : au hasard d'après les poids (3 pour chacune des trois qu'elle joue, 2
//! pour « se matérialise »), parmi les entrées possibles avec ses planches (« se matérialise »
//! l'est toujours), et **jamais deux lancements de suite la même** quand une autre est possible :
//! la dernière est retenue par `capri-app` (son nom court dans `intro.json` ; un nom inconnu, comme
//! ceux des intros d'avant, ne contraint rien). Le hasard vient de la graine de sa vie, mêlée à
//! [`DRAW_SEED`]. « Voir une intro », dans son menu, rejoue le film puis montre les entrées l'une
//! après l'autre, dans l'ordre de la liste ([`next`]).
//!
//! **La suite** ([`Suite`]) : le film d'abord, si ses images sont là (sans elles, l'entrée seule),
//! puis l'entrée. Ce qui la coupe pendant le film : la cacher (elle reste cachée, sans entrée), lui
//! parler (elle est à sa place tout de suite, sans entrée). Pendant l'entrée, ce qui coupe une
//! intro qu'elle joue (voir [`Cut`]). Le bonjour vient après toute la suite, au lancement
//! seulement, et seulement si rien ne l'a coupée ([`Suite::greets`]).
//!
//! **Le film.** Ce module n'en contient aucune image, ni aucune ligne de leur code : il décide
//! seulement laquelle montrer à chaque image d'animation (sa feuille d'exposition,
//! [`Film::sheet`]), où, quand c'est fini, et son glitch. Les décoder, les confier à la carte
//! graphique et les lâcher est l'affaire de `capri-app`.
//!
//! **Le temps.** Comme tous ses dessins (voir [`crate::anim`]) : chaque image est tenue un nombre
//! entier d'images d'animation à 24 i/s (« de tout près » : [`HOLD`] chacune, 12 par seconde, soit
//! 44 images, 1,83 s, la cadence du projet d'origine). L'horloge démarre quand la première image
//! arrive à l'écran, pas avant : les images sont décodées une à une pendant qu'elles défilent, et la
//! première peut se faire attendre ; si elle n'est pas là après [`START_WAIT`], il n'y a pas de
//! film. Ensuite, une image qui n'est pas prête à son heure laisse le dessin précédent tenu, et
//! l'horloge ne s'arrête pas : le film finit à l'heure, et l'image en retard est montrée dès
//! qu'elle arrive (plus récente que celle à l'écran, elle avance toujours), puis on demande celle
//! qui est déjà l'heure ([`Intro::shown`]). Son glitch ne change qu'avec le dessin.
//!
//! **Où.** L'image couvre tout l'écran ([`place`]) : mise à l'échelle la plus grande des deux
//! (celle de la largeur ou celle de la hauteur), centrée horizontalement et calée sur le bord bas ;
//! ce qui dépasse, sur les côtés ou en haut, est coupé. Aucune bande vide, quelle que soit la forme
//! de l'écran (sur un écran plus haut que 16:9, comme celui d'un MacBook Pro, ce sont les côtés).
//! Ni sa taille choisie ni la densité de l'écran n'y comptent : c'est tout l'écran qu'elle prend.
//! Rien ne s'y attrape : elle n'est pas encore là.
//!
//! **Comment il finit** ([`End`]) : joué jusqu'au bout (ou arrêté là par une image illisible au
//! milieu), l'entrée suit. Sans ses images (absentes, illisibles, d'une autre taille, refusées par
//! la carte graphique, ou la première trop lente), pas de film, et l'entrée se joue seule. Quitter
//! reste immédiat : rien à décider ici.

pub mod act;
pub mod glitch;

pub use act::Act;

use crate::body::Capri;
use crate::geom::{Rect, Vec2};
use crate::look::Look;
use crate::roam::Rng;

/// Comment une entrée se joue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Play {
    /// Jouée par elle, avec ses propres dessins (voir [`act`]).
    Act(Act),
    /// Rien de plus : elle se matérialise simplement (`Look::enter`).
    Plain,
}

/// Une entrée de la liste.
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

/// Ses entrées, après le film, dans l'ordre où « Voir une intro » les montre. **Écrite ici
/// seulement** : en ajouter une, ou en retirer une, ne se fait qu'ici (et dans ce qu'elle joue).
pub const LIST: &[Entry] = &[
    Entry { key: "tombe", name: "tombe du ciel", weight: 3, play: Play::Act(Act::Fall) },
    Entry { key: "marche", name: "arrive en marchant", weight: 3, play: Play::Act(Act::Walk) },
    Entry { key: "rassemble", name: "se rassemble", weight: 3, play: Play::Act(Act::Gather) },
    Entry { key: "rien", name: "se matérialise", weight: 2, play: Play::Plain },
];

/// Le film du lancement, joué d'abord à chaque fois, s'il est là : « de tout près ».
pub const FILM: &Film = &UP_CLOSE;

/// La graine du tirage : celle de sa vie, mêlée à ce nombre (« intro »), pour ne rien changer au
/// hasard de ses trajets ni de son bonjour.
pub const DRAW_SEED: u64 = 0x696e_7472_6f00;

/// L'entrée de nom court `key`, si elle est dans la liste.
pub fn by_key(key: &str) -> Option<&'static Entry> {
    LIST.iter().find(|entry| entry.key == key)
}

/// « Se matérialise » : toujours possible.
pub fn plain() -> &'static Entry {
    LIST.iter().find(|entry| entry.play == Play::Plain).unwrap_or(&LIST[LIST.len() - 1])
}

/// L'entrée du lancement, tirée avec la graine `seed` (celle de sa vie) : au hasard d'après leurs
/// poids, parmi celles que `possible` permet (« se matérialise » l'est toujours), jamais `last` (la
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

/// « Voir une intro » : l'entrée qui suit `after` (par son nom court ; aucune ou inconnue, la
/// première) dans l'ordre de la liste, en sautant celles que `possible` ne permet pas, et en
/// repartant du début après la dernière. « Se matérialise » en fait partie, pour qu'on puisse
/// toutes les voir.
pub fn next(after: Option<&str>, possible: impl Fn(&Entry) -> bool) -> &'static Entry {
    let from = after.and_then(|key| LIST.iter().position(|entry| entry.key == key)).map_or(0, |at| at + 1);
    (0..LIST.len())
        .map(|offset| &LIST[(from + offset) % LIST.len()])
        .find(|entry| entry.play == Play::Plain || possible(entry))
        .unwrap_or_else(plain)
}

/// Où en est la suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// Le film se joue.
    Film,
    /// L'entrée se joue.
    Entry,
    /// Finie.
    Over,
}

/// Ce qui suit la fin du film ([`Suite::film_over`], [`Suite::record_film`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    /// L'entrée, maintenant : le film s'est joué, ou n'a pas pu l'être.
    Entry,
    /// Rien : elle est à sa place tout de suite (on lui a parlé pendant le film).
    Here,
    /// Rien : elle reste cachée (on l'a cachée pendant le film).
    Hidden,
}

/// La suite d'une intro : le film (s'il est là), puis l'entrée ; au lancement, ou sur « Voir une
/// intro ». Elle retient comment chacun a fini, pour son bonjour et pour le journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suite {
    entry: &'static Entry,
    at_launch: bool,
    with_film: bool,
    part: Part,
    film: Option<End>,
    entered: Option<End>,
}

impl Suite {
    /// Une suite qui commence : par le film si `film` (ses images sont là), sinon par l'entrée
    /// `entry` ; `at_launch` : celle du lancement (son bonjour l'attend), sinon « Voir une intro ».
    pub fn new(entry: &'static Entry, film: bool, at_launch: bool) -> Self {
        let part = if film { Part::Film } else { Part::Entry };
        Self { entry, at_launch, with_film: film, part, film: None, entered: None }
    }

    /// Son entrée.
    pub fn entry(&self) -> &'static Entry {
        self.entry
    }

    /// Celle du lancement ?
    pub fn at_launch(&self) -> bool {
        self.at_launch
    }

    /// A-t-elle commencé par le film ?
    pub fn with_film(&self) -> bool {
        self.with_film
    }

    /// Où elle en est.
    pub fn part(&self) -> Part {
        self.part
    }

    /// Comment le film a fini (`None` : pas encore, ou pas de film).
    pub fn film_end(&self) -> Option<End> {
        self.film
    }

    /// Comment l'entrée a fini (`None` : pas encore, ou pas jouée).
    pub fn entry_end(&self) -> Option<End> {
        self.entered
    }

    /// Le film a fini par `end`, à l'image `frame` : ce qui suit, fait à son dessin `look` et à son
    /// corps `capri` (écrit ici une fois, pour l'app et pour l'essai des gestes au hasard). Joué
    /// jusqu'au bout, arrêté par une image illisible, ou sans ses images : l'entrée commence
    /// ([`Suite::begin_entry`]) ; coupé en la cachant : elle est cachée sans s'être montrée ; coupé
    /// autrement (on lui parle) : elle se matérialise à sa place tout de suite, sans entrée. Rend ce
    /// qui suit ; `None`, et rien ne change, si le film ne se jouait pas.
    pub fn film_over(&mut self, end: End, look: &mut Look, capri: &mut Capri, frame: u64) -> Option<After> {
        let after = self.record_film(end)?;
        match after {
            After::Entry => self.begin_entry(look, capri, frame),
            After::Here => look.enter(capri, frame),
            After::Hidden => look.hide_unseen(capri, frame),
        }
        Some(after)
    }

    /// L'entrée commence, à l'image `frame`, si c'est son tour : jouée par elle
    /// ([`Look::perform`]) ; « se matérialise », ou une entrée refusée (ses planches, ou la place
    /// d'où elle est, ou autre chose : [`Look::cannot_perform`] dit quoi), elle se matérialise à sa
    /// place, et la suite est finie.
    pub fn begin_entry(&mut self, look: &mut Look, capri: &mut Capri, frame: u64) {
        if self.part != Part::Entry {
            return;
        }
        match self.entry.play {
            Play::Act(act) => {
                if !look.perform(capri, act, frame) {
                    let why = look.cannot_perform(act, capri).unwrap_or(Missing::NotReady);
                    look.enter(capri, frame);
                    self.record_entry(End::Missing(why));
                }
            }
            Play::Plain => {
                look.enter(capri, frame);
                self.record_entry(End::Played { shown: 0 });
            }
        }
    }

    /// L'entrée qu'elle jouait a fini par `end` (ce que dit [`Look::take_performed`]), à l'image
    /// `frame` : la suite est finie ; coupée en la cachant, elle est cachée.
    pub fn entry_over(&mut self, end: End, look: &mut Look, capri: &mut Capri, frame: u64) {
        if self.part != Part::Entry {
            return;
        }
        self.record_entry(end);
        if end.hides() {
            look.hide_unseen(capri, frame);
        }
    }

    /// Retient seulement que le film a fini par `end`, et rend ce qui suit, sans rien faire d'elle
    /// (pour le journal et les essais ; l'app passe par [`Suite::film_over`]). `None`, et rien ne
    /// change, si le film ne se jouait pas.
    pub fn record_film(&mut self, end: End) -> Option<After> {
        if self.part != Part::Film {
            return None;
        }
        self.film = Some(end);
        let after = match end {
            End::Played { .. } | End::Broken { .. } | End::Missing(_) => After::Entry,
            End::Cut { why: Cut::Hidden, .. } => After::Hidden,
            End::Cut { .. } => After::Here,
        };
        self.part = if after == After::Entry { Part::Entry } else { Part::Over };
        Some(after)
    }

    /// Retient seulement que l'entrée a fini par `end` : la suite est finie (voir
    /// [`Suite::record_film`]). Sans effet si l'entrée ne se jouait pas.
    pub fn record_entry(&mut self, end: End) {
        if self.part == Part::Entry {
            self.entered = Some(end);
            self.part = Part::Over;
        }
    }

    /// Finie ?
    pub fn is_over(&self) -> bool {
        self.part == Part::Over
    }

    /// Reste-t-elle cachée à la fin ? Coupée en la cachant, pendant le film ou l'entrée.
    pub fn hides(&self) -> bool {
        [self.film, self.entered].into_iter().flatten().any(End::hides)
    }

    /// Coupée, pendant le film ou l'entrée ?
    pub fn was_cut(&self) -> bool {
        [self.film, self.entered].into_iter().flatten().any(|end| matches!(end, End::Cut { .. }))
    }

    /// Dit-elle bonjour ? Au lancement seulement, une fois toute la suite finie, si rien ne l'a
    /// coupée et que quelque chose s'est joué (le film, ou l'entrée).
    pub fn greets(&self) -> bool {
        let played = [self.film, self.entered].into_iter().flatten().any(End::greets);
        self.at_launch && self.is_over() && !self.was_cut() && played
    }
}

/// Un film en images plein écran : son nom, ses fichiers, leur taille, sa feuille d'exposition et
/// son glitch.
#[derive(Debug, PartialEq)]
pub struct Film {
    /// Son nom pour le journal, à nous.
    pub name: &'static str,
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
    /// L'enveloppe de son glitch, s'il en a un (voir [`glitch`]).
    pub glitch: Option<&'static glitch::Envelope>,
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

/// Le dossier des images de « de tout près », dans celui des images (`assets/`).
pub const DIR: &str = "capri/intro";
/// Combien d'images dans « de tout près ».
pub const COUNT: u32 = 22;
/// La taille de chacune, en pixels.
pub const SIZE: (u32, u32) = (1920, 1080);
/// Chaque image de « de tout près » est tenue deux images d'animation : 12 dessins par seconde.
pub const HOLD: u64 = 2;
/// La première image a jusque-là pour arriver à l'écran (2 s), sinon pas de film : il ne retarde
/// jamais longtemps son apparition.
pub const START_WAIT: u64 = 48;

/// Le nom du fichier de l'image n° `index` (depuis 0) de « de tout près » : `01.png` à `22.png`.
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

/// « De tout près » (« Up close » dans son dépôt d'origine) : elle surgit dans l'écran, regarde
/// autour d'elle, colle son visage à la vitre avec un grand sourire, puis repart.
pub const UP_CLOSE: Film = Film {
    name: "de tout près",
    dir: DIR,
    files: &UP_CLOSE_FILES,
    size: SIZE,
    sheet: &UP_CLOSE_SHEET,
    glitch: Some(&glitch::UP_CLOSE),
};

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

/// Ce qui coupe le film ou l'entrée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// On la cache (⌃⌥C, ou son menu) : tout s'arrête net, et elle est cachée.
    Hidden,
    /// On lui parle (⌃⌥P, ⌃⌥D, ou son menu) : tout s'arrête, elle se matérialise tout de suite.
    Talked,
    /// On l'attrape, dans une entrée qu'elle joue : elle est tenue, comme d'habitude.
    Grabbed,
    /// Sa taille change (son menu, la molette, la densité de l'écran), dans une entrée qu'elle
    /// joue : comme quand on lui parle, elle est à sa place tout de suite.
    Resized,
}

/// Pourquoi il n'y a pas de film (ou, pour une entrée, pas ses planches).
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
    /// Une entrée : pas la place de la jouer d'où elle est (arriver en marchant, sa place trop près
    /// du bord droit, ou trop grande pour l'écran).
    NoRoom,
    /// Une entrée : pas prête à la jouer (ni à son entrée en scène, ni effacée pour une intro ;
    /// tenue ; en marche). Ce qui ne doit pas arriver après le film.
    NotReady,
}

/// Comment le film, ou l'entrée, a fini. Pour une entrée, les nombres sont des images
/// d'animation : combien elle a duré, jouée ou coupée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Jouée jusqu'au bout ; `shown` dessins différents ont été à l'écran (images d'animation,
    /// pour une intro jouée par elle).
    Played { shown: u32 },
    /// Coupée, le dessin `at` à l'écran (`None` : avant le premier).
    Cut { why: Cut, at: Option<u32> },
    /// Une image illisible au milieu (le dessin n° `at`) : elle s'arrête là, proprement.
    Broken { at: u32 },
    /// Rien ne s'est joué (pas d'images, pas de planches).
    Missing(Missing),
}

impl End {
    /// S'est-il joué : jusqu'au bout, ou arrêté par une image illisible au milieu ? Ni manquant, ni
    /// coupé. (Son bonjour, lui, se décide sur toute la suite : [`Suite::greets`].)
    pub fn greets(self) -> bool {
        matches!(self, End::Played { .. } | End::Broken { .. })
    }

    /// Reste-t-elle cachée ? Seulement coupée en la cachant ; sinon elle est là.
    pub fn hides(self) -> bool {
        matches!(self, End::Cut { why: Cut::Hidden, .. })
    }
}

/// Où en est un film.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Elle attend son premier dessin depuis l'image d'animation `since`.
    Waiting { since: u64 },
    /// Le dessin 0 aurait été à l'écran à l'image `since` ; le dessin `shown` y est.
    Playing { since: u64, shown: u32 },
    /// Finie : plus rien à montrer.
    Over,
}

/// Le déroulé d'un film : quel dessin de sa feuille montrer, et quand c'est fini.
#[derive(Clone, Debug, PartialEq)]
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

    /// Le dessin n° `index` est illisible : avant le premier, pas de film ; au milieu, il
    /// s'arrête là.
    pub fn broken(&mut self, index: u32) -> End {
        let end = match self.phase {
            Phase::Waiting { .. } => End::Missing(Missing::Unreadable { index }),
            Phase::Playing { .. } | Phase::Over => End::Broken { at: index },
        };
        self.phase = Phase::Over;
        end
    }

    /// Pas de film (voir [`Missing`]) : fini tout de suite.
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
    fn the_film_is_well_formed() {
        let film = FILM;
        assert_eq!(film, &UP_CLOSE, "« de tout près », toujours");
        assert!(!film.is_empty() && !film.files.is_empty());
        assert!(film.size.0 > 0 && film.size.1 > 0);
        for pair in film.sheet.windows(2) {
            assert_ne!(pair[0].0, pair[1].0, "deux fois la même image de suite");
        }
        for &(image, hold) in film.sheet {
            assert!((image as usize) < film.files.len() && hold >= 1);
        }
        // chaque image sert
        for image in 0..film.files.len() {
            assert!(film.sheet.iter().any(|&(shown, _)| shown as usize == image), "{image}");
        }
        // une intro, pas un film de cinéma : 4 s au plus
        assert!(film.frames() as f64 / FRAME_RATE <= 4.0);
        // son glitch a une force pour chacun de ses dessins
        assert_eq!(film.glitch.map(|envelope| envelope.force.len()), Some(film.len() as usize));
        assert!(!film.name.is_empty() && !film.name.contains('/'));
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
        // « se matérialise » est la plus rare
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
        // chacune sort, à peu près selon son poids ; « se matérialise » moins souvent
        let total: u32 = LIST.iter().map(|entry| entry.weight).sum();
        for (entry, &count) in LIST.iter().zip(&counts) {
            let expected = 4000.0 * f64::from(entry.weight) / f64::from(total);
            assert!((count as f64 - expected).abs() < expected * 0.25 + 20.0, "{}: {count}", entry.key);
        }
        // la même graine donne la même
        assert_eq!(draw(7, |_| true, None), draw(7, |_| true, None));
    }

    #[test]
    fn only_possible_entries_are_drawn_and_she_materializes_without_her_sheets() {
        let falls_only = |entry: &Entry| entry.play == Play::Act(Act::Fall);
        for seed in 0..500u64 {
            let drawn = draw(seed, |_| false, None);
            assert_eq!(drawn.play, Play::Plain, "sans ses planches, elle se matérialise");
            // la dernière était « se matérialise » : rien d'autre n'est possible, encore elle
            assert_eq!(draw(seed, |_| false, Some("rien")).play, Play::Plain);
            let drawn = draw(seed, falls_only, None);
            assert!(falls_only(drawn) || drawn.play == Play::Plain, "{drawn:?}");
            // une seule possible en plus de « se matérialise », et c'était la dernière
            assert_eq!(draw(seed, falls_only, Some("tombe")).play, Play::Plain);
        }
    }

    #[test]
    fn the_names_of_the_intros_of_before_constrain_nothing() {
        // « de tout près » et le coucou étaient des intros tirées au sort : leur nom, retenu par un
        // lancement d'avant, ne dit rien de la dernière entrée
        for old in ["upclose", "coucou", "inconnue"] {
            assert_eq!(by_key(old), None);
            let mut seen = std::collections::HashSet::new();
            for seed in 0..400u64 {
                seen.insert(draw(seed, |_| true, Some(old)).key);
                assert_eq!(draw(seed, |_| true, Some(old)), draw(seed, |_| true, None), "graine {seed}");
            }
            assert_eq!(seen.len(), LIST.len(), "chacune peut sortir après {old}");
        }
        // la liste n'a plus que des entrées : aucune n'est un film
        assert!(LIST.iter().all(|entry| matches!(entry.play, Play::Act(_) | Play::Plain)));
        let acts: Vec<Act> = LIST
            .iter()
            .filter_map(|entry| match entry.play {
                Play::Act(act) => Some(act),
                Play::Plain => None,
            })
            .collect();
        assert_eq!(acts, Act::ALL, "chacune de ses entrées jouées, une fois");
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
        // sans ses planches : « se matérialise », toujours
        assert_eq!(next(None, |_| false).play, Play::Plain);
        assert_eq!(next(Some("rien"), |_| false).play, Play::Plain);
        // une inconnue : la première
        assert_eq!(next(Some("inconnue"), all), &LIST[0]);
        // celles qui ne sont pas possibles sont sautées
        let acts = |entry: &Entry| matches!(entry.play, Play::Act(_));
        assert!(matches!(next(None, acts).play, Play::Act(_)));
    }

    /// Un petit film d'essai : trois images, la n° 1 tenue trois images.
    static UNEVEN: Film = Film {
        name: "essai",
        dir: "essai",
        files: &["a.png", "b.png", "c.png"],
        size: (8, 6),
        sheet: &[(0, 2), (1, 3), (2, 2), (1, 1)],
        glitch: None,
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

        // ce qui suit : cachée, elle le reste ; sinon elle est là ; seul ce qui s'est joué compte
        // pour son bonjour
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
            assert!(!end.greets() && !end.hides(), "rien de joué : {end:?}");
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
        for image in [FILM.size] {
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

    #[test]
    fn the_suite_plays_the_film_then_the_entry_then_she_says_hello() {
        let entry = by_key("tombe").expect("tombe du ciel");
        let mut suite = Suite::new(entry, true, true);
        assert_eq!((suite.part(), suite.entry(), suite.with_film()), (Part::Film, entry, true));
        assert!(!suite.greets() && !suite.is_over(), "pas avant la fin");
        // l'entrée ne finit pas pendant le film
        suite.record_entry(End::Played { shown: 30 });
        assert_eq!((suite.part(), suite.entry_end()), (Part::Film, None));
        assert_eq!(suite.record_film(End::Played { shown: 22 }), Some(After::Entry));
        assert_eq!(suite.part(), Part::Entry);
        assert!(!suite.greets(), "pas avant la fin de l'entrée");
        assert_eq!(suite.record_film(End::Played { shown: 22 }), None, "le film ne finit qu'une fois");
        suite.record_entry(End::Played { shown: 30 });
        assert!(suite.is_over() && suite.greets() && !suite.hides() && !suite.was_cut());
        assert_eq!(suite.film_end(), Some(End::Played { shown: 22 }));
        assert_eq!(suite.entry_end(), Some(End::Played { shown: 30 }));
        // « Voir une intro » : la même suite, sans bonjour
        let mut encore = Suite::new(entry, true, false);
        encore.record_film(End::Played { shown: 22 });
        encore.record_entry(End::Played { shown: 30 });
        assert!(encore.is_over() && !encore.greets());
        // « se matérialise » après le film : un bonjour aussi
        let mut plain_one = Suite::new(plain(), true, true);
        plain_one.record_film(End::Played { shown: 22 });
        plain_one.record_entry(End::Played { shown: 0 });
        assert!(plain_one.greets());
        // un film arrêté par une image illisible au milieu : l'entrée suit, et le bonjour
        let mut broken = Suite::new(entry, true, true);
        assert_eq!(broken.record_film(End::Broken { at: 9 }), Some(After::Entry));
        broken.record_entry(End::Played { shown: 30 });
        assert!(broken.greets());
    }

    #[test]
    fn without_the_film_the_entry_plays_alone() {
        let entry = by_key("rassemble").expect("se rassemble");
        // ses images absentes dès le départ : l'entrée tout de suite
        let mut alone = Suite::new(entry, false, true);
        assert_eq!((alone.part(), alone.with_film()), (Part::Entry, false));
        assert_eq!(alone.record_film(End::Played { shown: 22 }), None, "pas de film à finir");
        alone.record_entry(End::Played { shown: 28 });
        assert!(alone.greets() && alone.film_end().is_none());
        // ses images manquent en route (illisibles, refusées, la première trop lente) : de même
        for why in
            [Missing::Absent { found: 3 }, Missing::Unreadable { index: 0 }, Missing::Refused, Missing::Late]
        {
            let mut late = Suite::new(entry, true, true);
            assert_eq!(late.record_film(End::Missing(why)), Some(After::Entry), "{why:?}");
            late.record_entry(End::Played { shown: 28 });
            assert!(late.greets(), "{why:?}");
        }
        // rien de joué du tout (ni film, ni planches) : pas de bonjour
        let mut nothing = Suite::new(entry, true, true);
        nothing.record_film(End::Missing(Missing::Absent { found: 0 }));
        nothing.record_entry(End::Missing(Missing::Absent { found: 0 }));
        assert!(nothing.is_over() && !nothing.greets());
    }

    #[test]
    fn hidden_or_talked_to_during_the_film_there_is_no_entry_and_no_hello() {
        let entry = by_key("marche").expect("arrive en marchant");
        let mut hidden = Suite::new(entry, true, true);
        assert_eq!(hidden.record_film(End::Cut { why: Cut::Hidden, at: Some(4) }), Some(After::Hidden));
        assert!(hidden.is_over() && hidden.hides() && hidden.was_cut() && !hidden.greets());
        assert_eq!(hidden.entry_end(), None, "pas d'entrée");
        hidden.record_entry(End::Played { shown: 30 });
        assert_eq!(hidden.entry_end(), None, "finie : plus rien ne change");
        let mut talked = Suite::new(entry, true, true);
        assert_eq!(talked.record_film(End::Cut { why: Cut::Talked, at: None }), Some(After::Here));
        assert!(talked.is_over() && !talked.hides() && !talked.greets());
        // pendant l'entrée : cachée, on lui parle, attrapée, sa taille ; jamais de bonjour
        for why in [Cut::Hidden, Cut::Talked, Cut::Grabbed, Cut::Resized] {
            let mut suite = Suite::new(entry, true, true);
            suite.record_film(End::Played { shown: 22 });
            suite.record_entry(End::Cut { why, at: Some(12) });
            assert!(suite.is_over() && suite.was_cut() && !suite.greets(), "{why:?}");
            assert_eq!(suite.hides(), why == Cut::Hidden, "{why:?}");
        }
    }
}
