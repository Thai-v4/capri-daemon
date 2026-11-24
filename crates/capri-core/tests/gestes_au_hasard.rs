//! Des gestes au hasard, à graines fixes, dans l'ordre où la boucle de l'app les traite : entre
//! deux pas, les menus, les raccourcis et la souris (cacher, montrer, attraper, cliquer, lâcher,
//! « Changer de place », sa taille, ses cases), Claude Code qui finit son tour ; puis le pas
//! lui-même, comme `Running::tick` (son pas, le suivi de sa fenêtre comme `perch_view`, son dessin,
//! sa vie) ; et, autour, un bureau qui bouge (des fenêtres qui glissent, se ferment, s'ouvrent,
//! passent devant) et le temps qui passe, par à-coups. Ses gestes (sourire, éclat, se relever), sa
//! marche et sa planche en l'air s'y mêlent : attrapée, lâchée, cachée, on lui parle pendant
//! chacun.
//!
//! Ce qui doit toujours tenir, quoi qu'il arrive :
//! - « cachée » et son état sont d'accord (`Look::is_hidden`), et jamais elle n'est cachée, ni
//!   entre deux places, sans qu'on l'ait cachée : la fenêtre ne disparaît pas sous elle ;
//! - jamais coincée : montrée, elle est de nouveau là peu après ; cachée, elle finit de s'effacer ;
//! - là et debout, son tour avance (son flottement et sa boucle ne sont jamais figés) ;
//! - posée, avec ses planches assises, elle est dessinée assise, sa ligne d'assise sur son bord ;
//! - en chemin, elle va sur un bord où elle tient, ou debout en bas de l'écran, jamais sous le
//!   curseur ; la liste des fenêtres n'est lue qu'une fois par décision (et relue à l'arrivée sur
//!   un bord) ;
//! - jamais deux gestes à la fois : ce qui est dessiné est le geste en cours, ou aucun ; ni en
//!   marchant, ni tenue ; un geste finit toujours (au plus tard à sa dernière image) ;
//! - la main (`capri_core::input::Hand`, comme l'app) : un appui bref est un clic, un appui long
//!   ou déplacé une prise, un glissé plus court qu'un pas une prise lâchée à la fin du pas suivant ;
//!   un clic sur elle restée tenue (le relâchement perdu) la lâche, sans geste ;
//! - tenue, avec sa planche en l'air : jamais immobile pendant que le curseur s'éloigne d'un bord ;
//!   loin des bords, sa mèche se rapproche du curseur d'au moins la moitié de son chemin à chaque
//!   pas, jusqu'à être sous lui ;
//! - elle ne marche jamais à reculons, toujours vers la gauche (là où elle regarde), et sa marche
//!   finit ; une vie sur trois la laisse aller au bout (rien ne la dérange en marchant), et elle
//!   arrive là où elle allait ou au bord de l'écran ; elle ne s'arrête en chemin que si on lui
//!   parle ;
//! - une vie commence par la suite de l'intro, comme dans l'app : le film « de tout près » (quand
//!   ses images sont là : deux vies sur trois), puis n'importe quelle entrée
//!   (`capri_core::intro::LIST`) ; pendant le film, rien d'elle ne se joue ni ne s'attrape, la
//!   cacher le coupe net (cachée, sans entrée), lui parler aussi (à sa place tout de suite, sans
//!   entrée) ; fini à son heure, l'entrée suit. « Voir une intro » fait partie des gestes : refusée
//!   seulement quand quelque chose la retient, sans rien changer ; acceptée, elle s'efface, le film
//!   se joue, puis l'entrée suivante ; une entrée qu'elle joue finit toujours, à temps, coupée net si
//!   on la cache, matérialisée à sa place si on lui parle, tenue si on l'attrape ; jouée jusqu'au
//!   bout, elle est à sa place ; hors d'une intro, elle est toujours dans l'écran ; ce qui est
//!   dessiné pendant une entrée est l'entrée en cours ;
//! - elle guette (une vie sur trois commence sur un bureau dont les fenêtres touchent le haut de
//!   l'écran, et des fenêtres s'ouvrent parfois tout en haut) : seulement sans aucun bord, sa vie et
//!   « S'asseoir » cochées ; là, elle est sortie tout entière, ce qui se voit d'elle est dans
//!   l'écran, non vide, et son côté est sous elle ; jamais coincée derrière la fenêtre ; sa glissade
//!   finit toujours, sans saut ; ni geste ni repos ; dessinée penchée, sa boucle debout seulement ;
//!   son côté parti, recouvert, sa taille, ses cases : elle ne guette plus, debout, entière ;
//! - son repos : de temps en temps, personne ne s'occupe d'elle pendant longtemps (le temps passe
//!   alors à grandes enjambées) ; elle s'allonge, s'endort, se réveille par chaque moyen (on lui
//!   parle, un clic, une prise, Claude Code qui attend, « Changer de place », cachée), son bord s'en
//!   va sous elle, ses planches de repos arrivent tard, jamais, ou échouent, et sont reprises un
//!   moment après qu'elle n'en veut plus (comme `on_demand` dans l'app). Elle ne dort jamais
//!   pendant qu'on lui parle, n'est jamais allongée hors d'un bord (ni sans la place, ni sans ses
//!   planches), n'est jamais dessinée dans une planche qui n'est pas là, et finit toujours par se
//!   relever (allongée, jamais plus de `LIE_LONGEST`) et par se réveiller debout ;
//! - la même graine donne la même vie.
//!
//! Rapide (quelques secondes en version de débogage), sans écran ni bruit.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use capri_core::body::GLIDE;
use capri_core::input::{Hand, Let};
use capri_core::intro::{self, Act, After, End, Entry, Part, Play, Suite};
use capri_core::look::{
    Drawing, Gesture, LONGEST_GESTURE, LieMasks, Look, Moves, NapMasks, PerchMasks, Rest, Unsettled, Wants,
    Wardrobe,
};
use capri_core::perch::{self, LurkStature, Seen, Stature};
use capri_core::roam::{
    AVOID, BY_HAND, Busy, Choice, Event, FLOOR, Hold, LIE_LONGEST, LURK_STAY, Rng, Roamer, SEATED, UP_ASLEEP,
    WALK_LONGEST, World,
};
use capri_core::sheets::{self, lie, life, stand};
use capri_core::{AlphaMask, Capri, Rect, Stance, Vec2};

const SCREEN: Rect = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
/// Ses tailles possibles sur un écran Retina, de 40 % à 140 % (600 : sa taille d'origine).
const HEIGHTS: [f32; 5] = [240.0, 360.0, 600.0, 780.0, 840.0];
/// Combien de pas par vie, et combien de vies par garde-robe.
const STEPS: usize = 4000;
const SEEDS: u64 = 100;
/// Montrée, elle est de nouveau là en moins de ça (en images) ; cachée, partie en moins de ça.
const SETTLED_WITHIN: u64 = 150;
/// Une intro qu'elle joue dure au plus ça, en pas de la boucle (chacun avance son corps d'un
/// vingt-quatrième de seconde) : se matérialiser (28 images), puis marcher 4 s, et un peu de marge.
const PERFORM_LONGEST: u64 = 28 + 4 * 24 + 4;

fn full(width: u32, height: u32) -> AlphaMask {
    AlphaMask::from_rgba(width, height, &vec![255; (width * height * 4) as usize], 96, 0)
}

/// Ses planches de gestes et de variantes, chacune là ou non : la marche, en l'air, se relever,
/// sourire, l'éclat assise, ses clignements glitchés (un bit chacune, dans cet ordre).
const ALL_MOVES: u8 = 0b11_1111;
/// Ses planches de repos, offertes ou non : sa pose allongée, son sommeil debout (les deux bits
/// suivants).
const RESTS: u8 = 0b1100_0000;
/// Ses planches de repos reprises tant de secondes après qu'elle n'en veut plus (`on_demand::KEEP`,
/// en temps comme dans l'app : quand personne ne s'occupe d'elle, le temps passe vite, et elles sont
/// reprises en quelques pas).
const KEEP_SECONDS: f64 = 60.0;

/// Quand ses planches de repos arrivent, une fois qu'elle en veut : après tant de pas, jamais, ou
/// le chargement échoue.
#[derive(Clone, Copy, Debug)]
enum Arrival {
    After(u64),
    Never,
    Fails,
}

/// Ce qu'un pas du chargement change.
enum Load {
    Nothing,
    Dress,
    Undress,
    Fail,
}

/// Le chargement à la demande d'un jeu de planches, comme `on_demand` dans l'app : commencé quand
/// elle en veut, fini à son heure (en pas) même si elle n'en veut plus, repris `KEEP_SECONDS` après
/// qu'elle n'en veut plus.
struct Loader {
    arrival: Arrival,
    started: Option<u64>,
    ready: bool,
    unwanted: Option<f64>,
    failed: bool,
}

impl Loader {
    fn new(arrival: Arrival) -> Self {
        Self { arrival, started: None, ready: false, unwanted: None, failed: false }
    }

    fn step(&mut self, wanted: bool, step: u64, now: f64) -> Load {
        if self.failed {
            return Load::Nothing;
        }
        if !self.ready {
            if wanted && self.started.is_none() {
                self.started = Some(step);
            }
            let Some(started) = self.started else { return Load::Nothing };
            return match self.arrival {
                Arrival::After(delay) if step >= started + delay => {
                    self.ready = true;
                    self.unwanted = None;
                    Load::Dress
                }
                Arrival::Fails if step >= started + 3 => {
                    self.failed = true;
                    Load::Fail
                }
                Arrival::After(_) | Arrival::Fails | Arrival::Never => Load::Nothing,
            };
        }
        if wanted {
            self.unwanted = None;
            return Load::Nothing;
        }
        let since = *self.unwanted.get_or_insert(now);
        if now >= since + KEEP_SECONDS {
            self.ready = false;
            self.started = None;
            return Load::Undress;
        }
        Load::Nothing
    }
}

fn lie_masks() -> LieMasks {
    LieMasks { narrow: full(lie::NARROW, lie::CELL_HEIGHT), wide: full(lie::WIDE, lie::CELL_HEIGHT) }
}

fn nap_masks() -> NapMasks {
    NapMasks {
        sleep: full(life::CELL_WIDTH, sheets::CELL_HEIGHT),
        wake: full(life::CELL_WIDTH, sheets::CELL_HEIGHT),
    }
}

/// Posée, y a-t-il la place pour elle allongée là où elle est (ce que l'app sait de la lecture de
/// sa veille posée) ?
fn room_to_lie(look: &Look, capri: &Capri, desk: &Desk) -> bool {
    desk.perch
        && look
            .lying_stature()
            .zip(capri.perched())
            .is_some_and(|(stature, spot)| perch::fits(&desk.windows, SCREEN, stature, spot))
}

fn wardrobe((stand_, perch_, life_, moves): (bool, bool, bool, u8)) -> Wardrobe {
    let on = |bit: u8| moves & (1 << bit) != 0;
    let body = || full(life::CELL_WIDTH, sheets::CELL_HEIGHT);
    Wardrobe {
        cutout: full(220, 420),
        stand: stand_.then(|| full(stand::CELL_WIDTH, sheets::CELL_HEIGHT)),
        perch: perch_.then(|| PerchMasks {
            narrow: full(sheets::NARROW, sheets::CELL_HEIGHT),
            wide: full(sheets::WIDE, sheets::CELL_HEIGHT),
        }),
        life: life_.then(|| full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
        moves: Moves {
            walk: on(0).then(body),
            dangle: on(1).then(body),
            glitch: on(2).then(body),
            grin: on(3).then(body),
            perch_glitch: on(4),
            perch_blink_glitch: on(5),
            stand_blink_glitch: on(5),
        },
    }
}

/// Les bords de l'écran que touche son image debout : leur normale vers l'extérieur.
fn edges(capri: &Capri) -> Vec<Vec2> {
    let (min, max) = (capri.position(), capri.position() + capri.size());
    let mut touching = Vec::new();
    if min.x <= 0.5 {
        touching.push(Vec2::new(-1.0, 0.0));
    }
    if min.y <= 0.5 {
        touching.push(Vec2::new(0.0, -1.0));
    }
    if max.x >= SCREEN.max().x - 0.5 {
        touching.push(Vec2::new(1.0, 0.0));
    }
    if max.y >= SCREEN.max().y - 0.5 {
        touching.push(Vec2::new(0.0, 1.0));
    }
    touching
}

/// Combien son image debout dépasse de l'écran, au plus d'un côté (0 : entièrement dedans).
fn outside(capri: &Capri) -> f32 {
    beyond_screen(capri.position(), capri.position() + capri.size())
}

/// Combien ce qu'on voit d'elle dépasse de l'écran : guettant, tout ce qui se voit d'elle
/// (`Look::visible` : sa part derrière la fenêtre ne se dessine pas ; pas la place de son bandeau,
/// plus étroite) ; sinon son image debout.
fn outside_seen(capri: &Capri, look: &Look) -> f32 {
    if capri.lurking().is_some() {
        let seen = look.visible(capri);
        return beyond_screen(seen.min, seen.max());
    }
    outside(capri)
}

/// Combien le rectangle de `min` à `max` dépasse de l'écran, au plus d'un côté.
fn beyond_screen(min: Vec2, max: Vec2) -> f32 {
    [SCREEN.min.x - min.x, SCREEN.min.y - min.y, max.x - SCREEN.max().x, max.y - SCREEN.max().y]
        .into_iter()
        .fold(0.0, f32::max)
}

/// Les bords de l'écran au-delà desquels dépasse son image debout, de coin haut gauche `at` et de
/// taille `size` : leur normale vers l'extérieur.
fn beyond(at: Vec2, size: Vec2) -> Vec<Vec2> {
    let (min, max) = (at, at + size);
    let mut out = Vec::new();
    if min.x < SCREEN.min.x - 0.5 {
        out.push(Vec2::new(-1.0, 0.0));
    }
    if min.y < SCREEN.min.y - 0.5 {
        out.push(Vec2::new(0.0, -1.0));
    }
    if max.x > SCREEN.max().x + 0.5 {
        out.push(Vec2::new(1.0, 0.0));
    }
    if max.y > SCREEN.max().y + 0.5 {
        out.push(Vec2::new(0.0, 1.0));
    }
    out
}

/// Un vrai curseur est dans l'écran.
fn on_screen(cursor: Vec2) -> Vec2 {
    Vec2::new(
        cursor.x.clamp(SCREEN.min.x + 1.0, SCREEN.max().x - 1.0),
        cursor.y.clamp(SCREEN.min.y + 1.0, SCREEN.max().y - 1.0),
    )
}

/// Le chemin du curseur depuis l'appui jusqu'à la prise (ce que la prise rejoue).
fn trail_length(grab: &capri_core::input::Grab) -> f32 {
    let mut last = grab.at;
    grab.trail.iter().fold(0.0, |length, &(_, cursor)| {
        let step = (cursor - last).length();
        last = cursor;
        length + step
    })
}

/// Le seuil d'une prise, en pixels (`CLICK_SLOP` sur un écran Retina).
const SLOP: f32 = 8.0;

/// Le bureau : ses fenêtres (de la plus en avant à la plus en arrière), le curseur, la case
/// « S'asseoir sur les fenêtres », et combien de fois la liste a été lue.
struct Desk {
    windows: Vec<Seen>,
    cursor: Option<Vec2>,
    perch: bool,
    reads: usize,
}

impl World for Desk {
    fn screen(&self) -> Rect {
        SCREEN
    }
    fn cursor(&self) -> Option<Vec2> {
        self.cursor
    }
    fn perch_allowed(&self) -> bool {
        self.perch
    }
    fn seen(&mut self, _stature: Stature) -> Option<Vec<Seen>> {
        self.reads += 1;
        Some(self.windows.clone())
    }
    fn recheck(&mut self, capri: &mut Capri, stature: Stature, lurking: Option<LurkStature>) {
        ride(capri, self, stature, lurking);
    }
}

/// Posée, elle suit sa fenêtre ou la quitte, comme `perch_view` (à chaque pas : sa cadence ne
/// change rien à ce qui est vérifié ici) ; guettant, de même.
fn ride(capri: &mut Capri, desk: &Desk, stature: Stature, lurking: Option<LurkStature>) {
    if let Some(lurk) = capri.lurking() {
        match lurking.filter(|_| desk.perch).map(|l| perch::follow_flank(&desk.windows, SCREEN, l, lurk.spot))
        {
            Some(Ok(lurk)) => capri.ride_flank(lurk),
            _ => capri.take_off(),
        }
        return;
    }
    let Some(spot) = capri.perched() else { return };
    if !desk.perch {
        return capri.take_off();
    }
    match perch::follow(&desk.windows, SCREEN, stature, spot) {
        Ok(perch) => capri.ride(perch),
        Err(_) => capri.take_off(),
    }
}

fn carried(capri: &Capri) -> bool {
    matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. })
}

/// Le petit hasard des gestes, à part de celui de sa vie.
struct Dice(Rng);

impl Dice {
    fn chance(&mut self, one_in: u64) -> bool {
        self.0.next_u64().is_multiple_of(one_in)
    }
    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.0.unit() as f32
    }
    fn below(&mut self, n: usize) -> usize {
        (self.0.next_u64() % n as u64) as usize
    }
}

/// Ce que les vies ont exercé, pour s'assurer que l'essai passe bien par chaque chemin.
#[derive(Debug, Default)]
struct Tally {
    to_ledge: usize,
    to_floor: usize,
    stays: usize,
    arrived: usize,
    arrived_seated: usize,
    arrived_lost: usize,
    refused: usize,
    turned_back: usize,
    hidden_on_the_way: usize,
    grabbed_appearing: usize,
    /// Des pas où elle se balançait, où l'on se parlait, où le curseur était sur elle.
    grooving: usize,
    talking: usize,
    hovered: usize,
    /// Des gestes qui lui ont fait quitter son bord.
    touched_off: usize,
    /// Des clics (et ceux qui ont joué un geste), des fins de tour de Claude Code (et ses
    /// sourires), des fois où elle s'est relevée, des éclats d'elle-même.
    clicks: usize,
    clicked: usize,
    turn_ends: usize,
    cheers: usize,
    rises: usize,
    fidgets: usize,
    /// Des pas où elle pendait, où elle marchait ; ses marches, arrêtées en chemin ou non.
    dangling: usize,
    walking: usize,
    walks: usize,
    halted: usize,
    /// Des prises pendant un geste, pendant sa marche ; des fois où on lui parlait pendant un geste.
    grabbed_in_gesture: usize,
    grabbed_walking: usize,
    talked_in_gesture: usize,
    /// Ses marches allées au bout ; des pas tenue, loin des bords, où sa mèche s'est rapprochée ;
    /// des pas où elle s'est éloignée d'un bord avec le curseur.
    walks_arrived: usize,
    gliding: usize,
    off_an_edge: usize,
    /// Des prises nées d'un appui (par le pas, par le relâchement), des relâchements perdus, et des
    /// clics qui l'ont lâchée.
    grabs_by_step: usize,
    grabs_by_release: usize,
    stuck: usize,
    unstuck: usize,
    /// Ses intros jouées par elle : commencées (par intro), jouées jusqu'au bout, coupées (on la
    /// cache, on lui parle, on l'attrape) ; « Voir une intro » demandé, servi, refusé, et revenue
    /// sur ses pas avant l'intro.
    performed: [usize; 3],
    /// Le film : joué jusqu'au bout (l'entrée suit), coupé en la cachant, en lui parlant ; et
    /// joué pour « Voir une intro ».
    films: usize,
    film_cut_hidden: usize,
    film_cut_talked: usize,
    encore_films: usize,
    played: usize,
    cut_hidden: usize,
    cut_talked: usize,
    cut_grabbed: usize,
    cut_resized: usize,
    encores: usize,
    encores_started: usize,
    encores_refused: usize,
    encores_turned_back: usize,
    /// Des prises sans saut alors qu'elle dépassait de l'écran, des pas tenue en partie dehors où sa
    /// mèche s'est rapprochée, des pas hors d'une intro en partie dehors, des pas de marche entamés
    /// au-delà du bord droit.
    grabbed_outside: usize,
    gliding_outside: usize,
    outside: usize,
    walked_in: usize,
    /// Son repos : des moments où personne ne s'occupe d'elle ; allongée, endormie (allongée,
    /// debout), relevée d'elle-même, réveillée d'elle-même debout, réveillée par quelqu'un (dont
    /// Claude Code) ; des pas allongée, endormie ; son bord parti sous elle allongée, cachée
    /// allongée, attrapée allongée ou endormie, ses planches attendues, jamais venues, en échec.
    calm: usize,
    lay_downs: usize,
    dozes_lying: usize,
    dozes_standing: usize,
    sit_ups: usize,
    woke_alone: usize,
    woken: usize,
    woken_by_claude: usize,
    lying_steps: usize,
    asleep_steps: usize,
    ledge_gone_lying: usize,
    hidden_lying: usize,
    grabbed_resting: usize,
    waited_for_sheets: usize,
    sheets_failed: usize,
    /// Son repos allongée arrêté sans qu'on s'occupe d'elle (plus la place, plus posée).
    unsettled: usize,
    /// Elle guette : ses départs vers un côté, ses arrivées, des pas sortie, des pas à glisser ; son
    /// côté parti sous elle, cachée, attrapée, parlée en guettant, décochée ou trop grande.
    to_flank: usize,
    lurks: usize,
    lurk_steps: usize,
    glide_steps: usize,
    flank_gone: usize,
    hidden_lurking: usize,
    grabbed_lurking: usize,
    talked_lurking: usize,
    touched_lurking: usize,
}

/// Une entrée est-elle possible avec ses planches ?
fn possible(entry: &Entry, look: &Look, capri: &Capri) -> bool {
    match entry.play {
        Play::Plain => true,
        Play::Act(act) => look.can_perform(act, capri),
    }
}

/// Ce que la suite a fait en commençant son entrée (`Suite::begin_entry`, par `Suite::film_over`
/// ou directement, comme l'app) : jouée par elle, on la suit (sa place, `place`, prise juste
/// avant) ; « se matérialise », la suite est finie. Une entrée possible n'est jamais refusée.
fn entry_begun(
    suite: &Suite,
    look: &Look,
    place: Vec2,
    frame: u64,
    performing: &mut Option<(Act, u64, Vec2, bool)>,
    tally: &mut Tally,
) -> Result<(), String> {
    match suite.entry().play {
        Play::Act(act) if suite.part() == Part::Entry && look.is_performing() => {
            tally.performed[act_index(act)] += 1;
            *performing = Some((act, 0, place, false));
        }
        Play::Plain if suite.is_over() && suite.entry_end() == Some(End::Played { shown: 0 }) => {}
        _ => {
            return Err(format!(
                "image {frame} : l'entrée {:?} : {:?}",
                suite.entry().key,
                suite.entry_end()
            ));
        }
    }
    Ok(())
}

/// L'entrée qu'elle jouait a fini par `end` : la suite aussi (`Suite::entry_over`, comme l'app).
fn suite_over(
    suite: &mut Option<Suite>,
    end: End,
    look: &mut Look,
    capri: &mut Capri,
    frame: u64,
) -> Result<(), String> {
    let Some(playing) = suite.take() else {
        return Err(format!("image {frame} : une entrée finie hors d'une suite"));
    };
    let mut playing = playing;
    playing.entry_over(end, look, capri, frame);
    if !playing.is_over() || playing.entry_end() != Some(end) {
        return Err(format!("image {frame} : l'entrée finie ({end:?}), et la suite {playing:?}"));
    }
    if playing.hides() && !look.is_gone() {
        return Err(format!("image {frame} : l'entrée coupée en la cachant, et pas cachée"));
    }
    Ok(())
}

/// L'indice d'une intro jouée par elle, pour le décompte.
fn act_index(act: Act) -> usize {
    Act::ALL.iter().position(|&each| each == act).unwrap_or(0)
}

/// Une vie : `seed` pour ses gestes et sa vie. Rend une empreinte de ce qui s'est passé, ou ce
/// qui n'a pas tenu.
fn live(seed: u64, dressed: (bool, bool, bool, u8), tally: &mut Tally) -> Result<u64, String> {
    let mut dice = Dice(Rng::new(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ 0x5eed));
    let mut look = Look::new(wardrobe(dressed), HEIGHTS[2]);
    let rests = dressed.3;
    look.offer(Wants { lying: rests & (1 << 6) != 0, sleeping: rests & (1 << 7) != 0 });
    // ses planches de repos : le plus souvent elles arrivent, plus ou moins vite ; parfois jamais,
    // parfois le chargement échoue
    let arrival = |n: u64| match n % 9 {
        0 => Arrival::Never,
        1 => Arrival::Fails,
        n => Arrival::After(n * 3),
    };
    let mut lying_sheets = Loader::new(arrival(seed));
    let mut sleeping_sheets = Loader::new(arrival(seed / 9 + 4));
    let mut steps = 0u64;
    // personne ne s'occupe d'elle pendant tant de pas encore (le temps passe alors vite)
    let mut alone = 0u64;
    // depuis quand elle est allongée, endormie debout (en secondes)
    let mut lain_at: Option<f64> = None;
    let mut dozing_at: Option<f64> = None;
    let mut capri = Capri::new(look.standing_size(), SCREEN, Vec2::new(2400.0, 1200.0));
    capri.set_feet(look.standing_feet());
    capri.set_leave_frames(look.leave_frames());
    // une vie sur trois, des fenêtres qui touchent presque le haut de l'écran : aucun bord, des côtés
    let windows = if seed % 3 == 1 {
        vec![
            Seen { id: 1, rect: Rect::new(Vec2::new(400.0, 30.0), Vec2::new(1000.0, 1800.0)) },
            Seen { id: 2, rect: Rect::new(Vec2::new(1700.0, 60.0), Vec2::new(900.0, 1700.0)) },
        ]
    } else {
        vec![
            Seen { id: 1, rect: Rect::new(Vec2::new(300.0, 900.0), Vec2::new(1400.0, 900.0)) },
            Seen { id: 2, rect: Rect::new(Vec2::new(1500.0, 700.0), Vec2::new(1200.0, 1000.0)) },
        ]
    };
    let mut desk = Desk { windows, cursor: Some(Vec2::new(100.0, 100.0)), perch: true, reads: 0 };
    let mut next_id = 3;
    let mut roamer = Roamer::new(seed, 0.0, true);
    let mut moving = false;
    let (mut hidden, mut talking, mut music) = (false, false, false);
    let mut frame = 0u64;
    capri.step(0.0, frame);
    // une vie commence par la suite de l'intro : le film, quand ses images sont là (deux vies sur
    // trois), puis une entrée, chacune à son tour selon la graine (« se matérialise » si ses
    // planches ne la permettent pas) ; une sur cinq est coupée en la cachant tout de suite : partie
    // d'un coup, sans s'être jamais montrée (`Running::end_intro`, `Look::hide_unseen`,
    // `Look::vanish`). Le film se joue sans elle : ensuite, l'entrée
    let with_film = seed % 3 != 2;
    let opening = &intro::LIST[(seed / 5) as usize % intro::LIST.len()];
    let opening = if possible(opening, &look, &capri) { opening } else { intro::plain() };
    let mut shown_last = Some(opening.key);
    // l'entrée qu'elle joue : laquelle, depuis combien de pas, sa place, et si sa taille a changé
    // entre-temps
    let mut performing: Option<(Act, u64, Vec2, bool)> = None;
    // la suite qui se joue (`Running::intro`, par les mêmes méthodes que l'app) ; et le film :
    // l'image d'animation où il finit
    let mut suite = Some(Suite::new(opening, with_film, true));
    let mut film: Option<u64> = None;
    // combien elle dépassait de l'écran au pas précédent (hors d'une intro qu'elle joue)
    let mut last_out: Option<f32> = None;
    // depuis combien de pas elle glisse de derrière une fenêtre, ou s'y retire, vers où, et de quel
    // côté
    let mut gliding_for = 0u64;
    let mut glide_to: Option<(f32, perch::FlankSpot)> = None;
    if let Some(launch) = suite.as_mut() {
        if with_film {
            if seed % 5 == 4 {
                // cachée pendant le film : ni entrée, ni rien
                let cut = End::Cut { why: intro::Cut::Hidden, at: None };
                if launch.film_over(cut, &mut look, &mut capri, frame) != Some(After::Hidden)
                    || !look.is_gone()
                {
                    return Err("le film coupé en la cachant au lancement, et pas cachée".to_owned());
                }
                tally.film_cut_hidden += 1;
            } else {
                film = Some(frame + intro::FILM.frames());
            }
        } else if seed % 5 == 4 && opening.play == Play::Plain {
            // cachée tout de suite : partie d'un coup, sans s'être montrée
            launch.record_entry(End::Cut { why: intro::Cut::Hidden, at: None });
            look.hide_unseen(&mut capri, frame);
        } else {
            let place = capri_place(&capri);
            launch.begin_entry(&mut look, &mut capri, frame);
            entry_begun(launch, &look, place, frame, &mut performing, tally)?;
            if seed % 5 == 4 {
                look.vanish(&mut capri, frame);
            }
        }
    }
    if suite.as_ref().is_some_and(Suite::is_over) {
        suite = None;
    }
    if seed % 5 == 4 {
        hidden = true;
    }
    // « Voir une intro » demandé : celle qui se jouera une fois effacée
    let mut encore: Option<&'static Entry> = None;
    // depuis quand elle n'est pas là, montrée ; depuis quand on l'a cachée
    let mut away_since: Option<u64> = None;
    let mut toggled_at = 0u64;
    let mut decisions = 0usize;
    // « Changer de place » demandé et pas encore servi ; sa dernière arrivée debout en bas de l'écran
    let mut asked = false;
    let mut floor_arrival: Option<f64> = None;
    // sa dernière arrivée sur un côté, tant qu'elle y guette (son côté perdu, oubliée)
    let mut lurk_arrival: Option<f64> = None;
    // sa marche en cours : combien de pas de la boucle elle dure (chacun avance son corps d'un
    // vingt-quatrième de seconde, même quand le temps passe par à-coups), et d'où elle est partie
    let mut walk: Option<(u64, f32, f32)> = None;
    // une vie sur trois laisse ses marches aller au bout : rien ne la dérange pendant qu'elle marche
    let calm_life = seed.is_multiple_of(3);
    // la main ; l'appui en cours : quand le relâcher, et de combien le curseur bouge à chaque pas ;
    // le relâchement perdu (son menu ouvert pendant qu'on la tenait) : elle reste tenue
    let mut hand = Hand::new();
    let mut pressing: Option<(f64, Vec2)> = None;
    let mut stuck = false;
    // le curseur au pas précédent
    let mut last_cursor: Option<Vec2> = None;
    let mut print = DefaultHasher::new();
    let mut story: Vec<String> = Vec::new();
    let fail = |what: String, story: &[String]| {
        Err(format!("{what} ; derniers gestes : {:?}", &story[story.len().saturating_sub(10)..]))
    };

    for _ in 0..STEPS {
        steps += 1;
        // le temps passe, par à-coups
        frame += 1 + dice.below(3) as u64;
        if dice.chance(400) {
            frame += 24 * (30 + dice.below(400) as u64);
            story.push(format!("{frame} : le temps passe"));
            // ce qui se jouait est fini : ce qui suit est nouveau
            away_since = None;
        }
        // de temps en temps, personne ne s'occupe d'elle pendant longtemps : on ne lui parle plus,
        // la musique s'arrête, et le temps passe à grandes enjambées (5 à 55 s par pas)
        if alone == 0 && !capri.is_held() && pressing.is_none() && !stuck && dice.chance(250) {
            alone = 20 + dice.below(140) as u64;
            talking = false;
            music = false;
            tally.calm += 1;
            story.push(format!("{frame} : personne pendant {alone} pas"));
        }
        let quiet_stretch = alone > 0;
        if quiet_stretch {
            alone -= 1;
            frame += 24 * (5 + dice.below(50) as u64);
            away_since = None;
        }
        let now = frame as f64 / 24.0;

        // --- le film de l'intro (`Running::intro_tick`) : elle n'est pas encore là, rien ne
        // s'attrape, sa vie ne bouge pas ; la cacher le coupe net, lui parler aussi ; fini à son
        // heure, l'entrée suit ; ce qui suit, la suite le fait (`Suite::film_over`, comme l'app) ---
        if let (Some(until), Some(playing)) = (film, suite.as_mut()) {
            if look.is_performing() || performing.is_some() || playing.part() != Part::Film {
                return fail(format!("image {frame} : pendant le film, elle joue déjà une entrée"), &story);
            }
            let place = capri_place(&capri);
            if !quiet_stretch && dice.chance(200) {
                film = None;
                hidden = true;
                toggled_at = frame;
                away_since = None;
                let cut = End::Cut { why: intro::Cut::Hidden, at: Some(3) };
                let after = playing.film_over(cut, &mut look, &mut capri, frame);
                if after != Some(After::Hidden) || !look.is_gone() || !playing.is_over() {
                    return fail(format!("image {frame} : cachée pendant le film, et {after:?}"), &story);
                }
                tally.film_cut_hidden += 1;
                story.push(format!("{frame} : cachée pendant le film"));
            } else if !quiet_stretch && dice.chance(200) {
                film = None;
                talking = true;
                away_since = None;
                let cut = End::Cut { why: intro::Cut::Talked, at: Some(3) };
                let after = playing.film_over(cut, &mut look, &mut capri, frame);
                if after != Some(After::Here) || !playing.is_over() || playing.entry_end().is_some() {
                    return fail(
                        format!("image {frame} : on lui parle pendant le film, et {after:?}"),
                        &story,
                    );
                }
                roamer.came_back(now, &capri);
                asked = false;
                tally.film_cut_talked += 1;
                story.push(format!("{frame} : on lui parle pendant le film"));
            } else if frame >= until {
                film = None;
                away_since = None;
                tally.films += 1;
                story.push(format!("{frame} : le film fini, l'entrée {}", playing.entry().key));
                if playing.film_over(End::Played { shown: 22 }, &mut look, &mut capri, frame)
                    != Some(After::Entry)
                {
                    return fail(format!("image {frame} : le film fini, et pas d'entrée"), &story);
                }
                if let Err(what) = entry_begun(playing, &look, place, frame, &mut performing, tally) {
                    return fail(what, &story);
                }
                if playing.is_over() {
                    roamer.came_back(now, &capri);
                    asked = false;
                }
            }
            if playing.is_over() {
                suite = None;
            }
            // le film fait partie de l'intro : ce qu'elle dépassait de l'écran avant ne compte plus
            last_out = None;
            continue;
        }

        // --- entre deux pas : le bureau, les menus, les raccourcis, la souris ---
        if dice.chance(20) {
            desk.cursor = Some(Vec2::new(dice.between(0.0, 3024.0), dice.between(0.0, 1964.0)));
        }
        if !desk.windows.is_empty() && dice.chance(40) {
            let index = dice.below(desk.windows.len());
            let (dx, dy) = (dice.between(-200.0, 200.0), dice.between(-150.0, 150.0));
            desk.windows[index].rect.min += Vec2::new(dx, dy);
        }
        if !desk.windows.is_empty() && dice.chance(250) {
            let index = dice.below(desk.windows.len());
            desk.windows.remove(index);
            story.push(format!("{frame} : une fenêtre se ferme"));
        }
        if desk.windows.len() < 6 && dice.chance(150) {
            // parfois tout en haut de l'écran, et haute : sans bord, avec des côtés
            let top = dice.chance(3);
            let min = Vec2::new(
                dice.between(-200.0, 2800.0),
                if top { dice.between(0.0, 200.0) } else { dice.between(0.0, 1800.0) },
            );
            let size = Vec2::new(
                dice.between(200.0, 2000.0),
                if top { dice.between(900.0, 1900.0) } else { dice.between(100.0, 1200.0) },
            );
            desk.windows.insert(0, Seen { id: next_id, rect: Rect::new(min, size) });
            next_id += 1;
        }
        if desk.windows.len() > 1 && dice.chance(200) {
            let index = dice.below(desk.windows.len());
            let raised = desk.windows.remove(index);
            desk.windows.insert(0, raised);
        }
        // une vie calme : rien ne la dérange pendant qu'elle marche ; ni quand personne n'est là
        let calm = (calm_life && look.is_walking(&capri)) || quiet_stretch;
        if !calm && dice.chance(120) {
            talking = !talking;
            tally.talked_lurking += usize::from(talking && capri.lurking().is_some() && look.is_here());
            // on lui parle (`App::talk`, `Running::cut_intro`) : l'intro qu'elle joue s'arrête et
            // elle se matérialise ; en train de s'effacer pour une intro, elle revient sur ses pas
            if talking && look.is_performing() {
                look.cut_performance(&mut capri, frame);
                story.push(format!("{frame} : on lui parle pendant son intro"));
                // relevé tout de suite, comme l'app (`Running::cut_intro`), avant qu'une prise au
                // même passage ne la déplace
                if let Some(end) = look.take_performed() {
                    if let Err(what) = performed(end, &mut performing, &look, &capri, hidden, tally) {
                        return fail(format!("image {frame} : {what}"), &story);
                    }
                    if let Err(what) = suite_over(&mut suite, end, &mut look, &mut capri, frame) {
                        return fail(what, &story);
                    }
                    roamer.came_back(now, &capri);
                    asked = false;
                    away_since = None;
                }
            }
            if talking && encore.is_some() && look.is_bowing_out() {
                look.turn_back(&mut capri, frame);
                encore = None;
                tally.encores_turned_back += 1;
                story.push(format!("{frame} : on lui parle avant son intro"));
            }
        }
        if !calm && dice.chance(150) {
            music = !music;
        }
        // « S'asseoir sur les fenêtres » (`App::toggle_perch` : `perch_view::set_enabled`, puis
        // `roam_view::touched`)
        if !calm && dice.chance(500) {
            desk.perch = !desk.perch;
            let lurked = capri.lurking().is_some();
            let was = capri.perched().is_some() || lurked;
            if !desk.perch && was {
                capri.take_off();
            }
            roamer.touched(now, &capri);
            if was && capri.perched().is_none() && capri.lurking().is_none() {
                tally.touched_off += 1;
                tally.touched_lurking += usize::from(lurked);
                if (roamer.next() - (now + BY_HAND.min)).abs() > 1e-6 {
                    return fail(
                        format!("image {frame} : décochée, elle repartira à {}", roamer.next()),
                        &story,
                    );
                }
            }
            story.push(format!("{frame} : s'asseoir sur les fenêtres {}", desk.perch));
        }
        if !calm && dice.chance(600) {
            roamer.set_enabled(!roamer.is_enabled(), now);
        }
        // cacher, montrer (`App::toggle_hidden`, `Running::vanish`, `Running::come_back`)
        if !calm && dice.chance(200) {
            hidden = !hidden;
            toggled_at = frame;
            away_since = None;
            if hidden {
                hand.cancel();
                pressing = None;
                if look.is_traveling() {
                    tally.hidden_on_the_way += 1;
                }
                if matches!(look.rest(), Rest::Lying | Rest::Asleep { lying: true }) {
                    tally.hidden_lying += 1;
                }
                tally.hidden_lurking += usize::from(capri.lurking().is_some());
                look.vanish(&mut capri, frame);
            } else {
                if look.is_gone() {
                    capri.step(0.0, frame);
                    let stature = look.stature();
                    ride(&mut capri, &desk, stature, look.lurk_stature());
                }
                look.appear(&mut capri, frame);
                let waiting = roamer.next();
                roamer.came_back(now, &capri);
                asked = false;
                if roamer.next() < waiting {
                    return fail(format!("image {frame} : revenue, son attente raccourcie"), &story);
                }
            }
            story.push(format!("{frame} : cachée {hidden}"));
        }
        // sa taille (`Running::resize` : `perch_view::resized`, puis `roam_view::touched`) ; pas
        // pendant qu'on la tient (la molette l'ignore alors, et son menu ne s'ouvre pas sous un
        // bouton enfoncé)
        if !calm && !capri.is_held() && dice.chance(300) {
            look.set_height(HEIGHTS[dice.below(HEIGHTS.len())]);
            capri.set_size(look.standing_size());

            let stature = look.stature();
            let lurked = capri.lurking().is_some();
            let was = capri.perched().is_some() || lurked;
            ride(&mut capri, &desk, stature, look.lurk_stature());
            look.set_room_to_lie(room_to_lie(&look, &capri, &desk));
            roamer.touched(now, &capri);
            if was && capri.perched().is_none() && capri.lurking().is_none() {
                tally.touched_off += 1;
                tally.touched_lurking += usize::from(lurked);
                if (roamer.next() - (now + BY_HAND.min)).abs() > 1e-6 {
                    return fail(
                        format!("image {frame} : trop grande, elle repartira à {}", roamer.next()),
                        &story,
                    );
                }
            }
            story.push(format!("{frame} : taille"));
        }
        // « Changer de place » (cachée, ou pendant une intro, l'app n'en fait rien)
        if !hidden && !calm && !look.is_performing() && dice.chance(80) {
            roamer.ask();
            asked = true;
            story.push(format!("{frame} : changer de place"));
        }
        // on se parle (`Running::tick` : `Look::set_conversing`, avant son pas)
        look.set_conversing(talking);
        // « Voir une intro » (`App::see_intro` ; cachée, ou une intro en cours, l'app n'en fait rien)
        if !hidden && !calm && encore.is_none() && !look.is_performing() && dice.chance(250) {
            let entry = intro::next(shown_last, |entry| possible(entry, &look, &capri));
            let (here, walking, held) = (look.is_here(), look.is_walking(&capri), carried(&capri));
            let before = (look.is_here(), look.is_traveling(), look.gesture().is_some(), capri.position());
            tally.encores += 1;
            match look.bow_out(&mut capri, frame) {
                Ok(()) => {
                    if !here || talking || walking || held {
                        return fail(format!("image {frame} : « Voir une intro » servi, retenue"), &story);
                    }
                    shown_last = Some(entry.key);
                    encore = Some(entry);
                    away_since = None;
                    story.push(format!("{frame} : voir l'intro {}", entry.key));
                }
                Err(hold) => {
                    tally.encores_refused += 1;
                    let fair = match hold {
                        Hold::Carried => held,
                        Hold::Away => !here,
                        Hold::Talking => talking,
                        Hold::Walking => walking,
                        Hold::Music => false,
                    };
                    let after =
                        (look.is_here(), look.is_traveling(), look.gesture().is_some(), capri.position());
                    if !fair || after != before {
                        return fail(format!("image {frame} : « Voir une intro » refusé : {hold:?}"), &story);
                    }
                }
            }
        }
        // Claude Code attend une réponse (`Running::tick`, `BannerView::take_awaiting`) : endormie,
        // elle se réveille ; plus souvent quand personne d'autre ne s'occupe d'elle
        if dice.chance(if quiet_stretch { 40 } else { 300 }) {
            let asleep = matches!(look.rest(), Rest::Asleep { .. });
            if look.stir(&mut capri, frame) {
                tally.woken_by_claude += 1;
                if !asleep {
                    return fail(format!("image {frame} : réveillée sans dormir"), &story);
                }
                story.push(format!("{frame} : Claude attend, elle se réveille"));
            }
        }
        // Claude Code finit son tour (`Running::tick`, `BannerView::take_turn_end`)
        if dice.chance(150) {
            tally.turn_ends += 1;
            if look.cheer(&mut capri, frame) {
                tally.cheers += 1;
                story.push(format!("{frame} : Claude a fini, elle sourit"));
            }
        }
        // un appui sur elle, là où elle est dessinée, si quelque chose l'attrape (`handle_overlay`) :
        // relâché plus tard, le curseur immobile ou non ; ou appui, glissé et relâchement dans le
        // même passage. Restée tenue sans bouton (son relâchement perdu), un clic la lâche
        if !hidden && !calm && pressing.is_none() && (!capri.is_held() || stuck) && dice.chance(40) {
            let picture = look.picture(&capri);
            // sur la part d'elle qui se voit (une intro peut la mettre en partie hors de l'écran ;
            // guettant, ce qui se voit d'elle est autour de sa tête, dehors)
            let shown = if picture.lean.is_some() { look.shown(&capri) } else { picture.rect };
            let cursor = on_screen(shown.min + shown.size * 0.5);
            if look.is_over(&capri, cursor) {
                hand.press(cursor, now);
                desk.cursor = Some(cursor);
                let drift = if dice.chance(3) { 12.0 } else { 1.0 };
                let drift = Vec2::new(dice.between(-drift, drift), dice.between(-drift, drift));
                let quick = dice.chance(6);
                pressing = Some((if quick { now } else { now + f64::from(dice.between(0.0, 0.5)) }, drift));
                if quick {
                    desk.cursor = Some(on_screen(cursor + Vec2::new(30.0, -10.0)));
                }
                story.push(format!("{frame} : un appui"));
            }
        }
        // l'appui en cours : le curseur bouge un peu, ou pas ; relâché à son heure
        if let Some((release_at, drift)) = pressing {
            if now < release_at {
                desk.cursor = desk.cursor.map(|c| on_screen(c + drift));
            } else {
                pressing = None;
                let held = capri.is_held();
                let gesture = look.gesture();
                match hand.release(desk.cursor, now, SLOP, held) {
                    Let::Click => {
                        tally.clicks += 1;
                        if let Some(started) = look.poke(&mut capri, frame) {
                            tally.clicked += 1;
                            if gesture.is_some() {
                                return fail(
                                    format!("image {frame} : {started:?} par-dessus {gesture:?}"),
                                    &story,
                                );
                            }
                            story.push(format!("{frame} : un clic, {}", started.describe()));
                        }
                    }
                    Let::Go => {
                        // un clic sur elle restée tenue : elle est lâchée, sans geste ; ou elle
                        // était déjà prise (l'appui devenu prise au pas), et c'est le lâcher
                        capri.release(now);
                        if stuck {
                            tally.unstuck += 1;
                            stuck = false;
                            if look.gesture() != gesture {
                                return fail(
                                    format!("image {frame} : un clic qui la lâche et la fait sourire"),
                                    &story,
                                );
                            }
                        }
                        story.push(format!("{frame} : relâchée"));
                    }
                    Let::Grab(grab) => {
                        // un glissé plus court qu'un pas : prise maintenant, lâchée à la fin du pas
                        tally.grabs_by_release += 1;
                        stuck = false;
                        let (seated, before) = (look.is_seated(&capri), capri.position());
                        let held = held_point(&look, &capri, &grab);
                        if look.hold(&mut capri, &grab) {
                            story.push(format!("{frame} : prise et lâchée au même passage"));
                            let jumped = match held {
                                Some(held) => no_jump_lurking(&capri, &grab, held),
                                None => no_jump(&capri, &grab, seated, before, tally),
                            };
                            // redressée, son image peut dépasser de l'écran : c'est là que la mesure repart
                            if held.is_some() {
                                last_out = None;
                            }
                            if let Err(what) = jumped {
                                return fail(format!("image {frame} : {what}"), &story);
                            }
                        }
                    }
                }
                if capri.is_held() && !hand.is_letting_go() {
                    return fail(format!("image {frame} : relâchée, et toujours tenue"), &story);
                }
            }
        }
        // tenue : le curseur l'emmène, parfois près du haut d'une fenêtre ; puis lâchée (parfois le
        // relâchement se perd : elle reste tenue, jusqu'à un clic sur elle)
        if capri.is_held()
            && pressing.is_none()
            && !stuck
            && !hand.is_letting_go()
            && let Some(cursor) = desk.cursor
        {
            let grab = cursor - capri.position();
            let target = match desk.windows.first() {
                Some(window) if dice.chance(3) => {
                    Vec2::new(dice.between(window.rect.min.x, window.rect.max().x), window.rect.min.y - 20.0)
                }
                _ => capri.feet() + Vec2::new(dice.between(-80.0, 80.0), dice.between(-80.0, 80.0)),
            };
            let feet = capri.feet() - capri.position();
            desk.cursor = Some(on_screen(target - feet + grab));
            if dice.chance(8) {
                let fast = dice.chance(2);
                if fast {
                    let away = desk.cursor.map(|c| on_screen(c + Vec2::new(900.0, -300.0)));
                    if let Some(c) = away {
                        let out = outside(&capri);
                        capri.drag_to(c, now + 0.03);
                        desk.cursor = Some(c);
                        if outside(&capri) > out.max(0.0) + 0.01 {
                            return fail(format!("image {frame} : tenue, plus hors de l'écran"), &story);
                        }
                    }
                }
                if dice.chance(3) {
                    stuck = true;
                    tally.stuck += 1;
                    story.push(format!("{frame} : relâchement perdu"));
                } else {
                    match hand.release(desk.cursor, now + 0.03, SLOP, true) {
                        Let::Go => capri.release(now + 0.03),
                        other => return fail(format!("image {frame} : lâchée, et {other:?}"), &story),
                    }
                    story.push(format!("{frame} : lâchée{}", if fast { ", lancée" } else { "" }));
                }
            }
        }
        if hidden != look.is_hidden() {
            return fail(
                format!("image {frame} : cachée {hidden}, mais is_hidden {}", look.is_hidden()),
                &story,
            );
        }
        // cachée pendant qu'elle s'effaçait pour une intro : pas d'intro
        if look.is_hidden() {
            encore = None;
        }
        if let Some(end) = look.take_performed() {
            if let Err(what) = performed(end, &mut performing, &look, &capri, hidden, tally) {
                return fail(format!("image {frame} : {what}"), &story);
            }
            if let Err(what) = suite_over(&mut suite, end, &mut look, &mut capri, frame) {
                return fail(what, &story);
            }
            if !end.hides() {
                roamer.came_back(now, &capri);
                asked = false;
            }
            away_since = None;
        }

        // ses planches de repos, chargées quand elle en veut, reprises quand elle n'en veut plus
        // (`Figure::sync_rests`, aussi quand elle est partie)
        let wants = look.wants();
        if (wants.lying && !look.dressed().lying) || (wants.sleeping && !look.dressed().sleeping) {
            tally.waited_for_sheets += 1;
        }
        // reprises, jamais pendant qu'elle s'en sert : vu de ce qu'elle fait et de ce qui est
        // dessiné, pas de ce qu'elle en veut (`Look::wants`, que cet essai doit aussi surveiller)
        let shown = (!look.is_gone()).then(|| look.picture(&capri).drawing);
        let uses_lying = matches!(look.rest(), Rest::Lying | Rest::Asleep { lying: true })
            || matches!(shown, Some(Drawing::Lie(_)));
        let uses_sleeping = matches!(look.rest(), Rest::Asleep { lying: false } | Rest::Waking)
            || matches!(shown, Some(Drawing::Life(cell)) if life::Strip::NAP.contains(&cell.strip));
        match lying_sheets.step(wants.lying, steps, now) {
            Load::Nothing => {}
            Load::Dress => look.dress_lying(Some(lie_masks())),
            Load::Undress if uses_lying => {
                return fail(
                    format!("image {frame} : ses planches allongées reprises alors qu'elle s'en sert"),
                    &story,
                );
            }
            Load::Undress => look.dress_lying(None),
            Load::Fail => {
                tally.sheets_failed += 1;
                look.offer(Wants { lying: false, ..look.offered() });
            }
        }
        match sleeping_sheets.step(wants.sleeping, steps, now) {
            Load::Nothing => {}
            Load::Dress => look.dress_sleeping(Some(nap_masks())),
            Load::Undress if uses_sleeping => {
                return fail(
                    format!("image {frame} : son sommeil debout repris alors qu'elle s'en sert"),
                    &story,
                );
            }
            Load::Undress => look.dress_sleeping(None),
            Load::Fail => {
                tally.sheets_failed += 1;
                look.offer(Wants { sleeping: false, ..look.offered() });
            }
        }

        // --- le pas (`Running::tick`) : la boucle n'en fait pas quand elle est partie ---
        if look.is_gone() {
            continue;
        }
        // un appui qui dure ou se déplace devient une prise, faite là et quand il a commencé
        if let Some(grab) = hand.step(desk.cursor, now, SLOP) {
            tally.grabs_by_step += 1;
            if look.is_traveling() || !look.is_here() {
                tally.grabbed_appearing += 1;
            }
            tally.grabbed_in_gesture += usize::from(look.gesture().is_some());
            tally.grabbed_walking += usize::from(look.is_walking(&capri));
            tally.grabbed_resting += usize::from(look.is_resting());
            stuck = false;
            let (seated, before) = (look.is_seated(&capri), capri.position());
            let held = held_point(&look, &capri, &grab);
            tally.grabbed_lurking += usize::from(capri.lurking().is_some() && !look.is_away());
            if look.hold(&mut capri, &grab) {
                story.push(format!("{frame} : attrapée"));
                let jumped = match held {
                    Some(held) => no_jump_lurking(&capri, &grab, held),
                    None => no_jump(&capri, &grab, seated, before, tally),
                };
                // redressée, son image peut dépasser de l'écran : c'est là que la mesure repart
                if held.is_some() {
                    last_out = None;
                }
                if let Err(what) = jumped {
                    return fail(format!("image {frame} : {what}"), &story);
                }
                // le curseur que son corps connaît : le dernier relevé depuis l'appui
                last_cursor = grab.trail.last().map(|&(_, cursor)| cursor).or(Some(grab.at));
            }
        }
        // tenue, le curseur l'emmène ; avec sa planche en l'air, elle glisse vers sa mèche sous lui
        let dangling = capri.is_held() && look.is_dangling(&capri);
        let cursor_before = last_cursor;
        // l'écart de sa mèche au curseur, là où il était au pas précédent
        let (from, gap, touching) = (
            capri.position(),
            look.grip(&capri).zip(cursor_before).map(|(grip, cursor)| (grip - cursor).length()),
            edges(&capri),
        );
        if capri.is_held()
            && !hand.is_letting_go()
            && let Some(cursor) = desk.cursor
        {
            let out = outside(&capri);
            capri.drag_to(cursor, now);
            // l'écran ne la laisse jamais sortir davantage (prise en partie dehors : pas plus loin)
            if outside(&capri) > out.max(0.0) + 0.01 {
                return fail(
                    format!("image {frame} : tenue, plus hors de l'écran ({out} → {})", outside(&capri)),
                    &story,
                );
            }
            if dangling
                && let Some(before) = cursor_before
                && (cursor - before).length() > 0.5
            {
                let way = cursor - before;
                let away = touching.iter().all(|n| way.x * n.x + way.y * n.y < 0.0);
                if !touching.is_empty() && away {
                    tally.off_an_edge += 1;
                    if capri.position() == from {
                        return fail(
                            format!("image {frame} : immobile pendant que le curseur s'éloigne du bord"),
                            &story,
                        );
                    }
                }
                let now_gap = look.grip(&capri).map(|grip| (grip - cursor).length());
                // en partie hors de l'écran (prise pendant une intro) : le curseur va franchement vers
                // l'intérieur, elle le suit (son glissé vers sa mèche en prend au plus la moitié)
                for n in beyond(from, capri.size()) {
                    let inward = -(way.x * n.x + way.y * n.y);
                    if inward > way.length() * 0.75 + 0.5 {
                        tally.gliding_outside += 1;
                        let followed =
                            -((capri.position() - from).x * n.x + (capri.position() - from).y * n.y);
                        if followed <= 0.0 {
                            return fail(
                                format!("image {frame} : dehors, elle ne suit pas le curseur vers l'écran"),
                                &story,
                            );
                        }
                    }
                }
                if touching.is_empty()
                    && edges(&capri).is_empty()
                    && let (Some(g0), Some(g1)) = (gap, now_gap)
                {
                    tally.gliding += 1;
                    if g1 > (g0 - way.length() / 2.0).max(0.0) + 3.0 {
                        return fail(
                            format!(
                                "image {frame} : sa mèche à {g1} du curseur, à {g0} avant (curseur {})",
                                way.length()
                            ),
                            &story,
                        );
                    }
                }
            }
        }
        last_cursor = desk.cursor;
        let feet_before = capri.feet().x;
        let (stance_before, position_before) = (capri.stance(), capri.position());
        capri.step(1.0 / 24.0, frame);
        // son corps ne va jamais plus vite que ce qui la porte : sa marche, sa chute, son retour
        // dans l'écran ; immobile sinon (hors prise et lancer, qui suivent la main)
        let moved = capri.position() - position_before;
        let most = match stance_before {
            Stance::Walking { speed, .. } => speed / 24.0,
            Stance::Falling { from, to, t } => {
                let next = intro::act::fall_at(from, to, t + 1.0 / 24.0);
                (next - intro::act::fall_at(from, to, t)).abs()
            }
            Stance::Floating => capri_core::body::RETURN * capri.size().y / 24.0,
            Stance::Perched { .. } => 0.0,
            // sa glissade, adoucie (au plus 1,5 fois sa vitesse moyenne)
            Stance::Lurking { lurk, .. } => {
                let fit = capri.lurk_fit();
                1.5 * fit.slide[lurk.spot.side.index()] * capri.size().x / GLIDE / 24.0
            }
            Stance::Held { .. } | Stance::Thrown { .. } => f32::INFINITY,
        };
        if moved.length() > most + 0.01 {
            return fail(
                format!("image {frame} : un saut de {:?} en {stance_before:?} ({most} au plus)", moved),
                &story,
            );
        }
        tally.walked_in += usize::from(
            matches!(stance_before, Stance::Walking { .. })
                && position_before.x + capri.size().x > SCREEN.max().x,
        );
        let stature = look.stature();
        // `perch_view::tick` : lâchée doucement, ou au bout d'un lancer, elle se pose peut-être
        let now_moving = carried(&capri);
        if std::mem::replace(&mut moving, now_moving)
            && capri.stance() == Stance::Floating
            && desk.perch
            && let Some(perch) = perch::landing(&desk.windows, SCREEN, stature, capri.feet())
        {
            capri.land(perch);
        }
        let lying_before = matches!(look.rest(), Rest::Lying | Rest::Asleep { lying: true });
        let lurked = capri.lurking().is_some();
        ride(&mut capri, &desk, stature, look.lurk_stature());
        if lying_before && capri.perched().is_none() && !capri.is_held() {
            tally.ledge_gone_lying += 1;
            story.push(format!("{frame} : son bord parti sous elle allongée"));
        }
        if lurked && capri.lurking().is_none() {
            tally.flank_gone += 1;
            story.push(format!("{frame} : son côté parti sous elle"));
            // un côté perdu : elle se choisit vite une autre place, debout comprise
            lurk_arrival = None;
            floor_arrival = None;
            // debout, entière : dans l'écran
            if outside(&capri) > 0.01 {
                return fail(format!("image {frame} : son côté parti, et hors de l'écran"), &story);
            }
        }
        // la place pour elle allongée, d'après cette lecture (`perch_view::room_to_lie`)
        let room = room_to_lie(&look, &capri, &desk);
        look.set_room_to_lie(room);
        let music_on = music && look.wants_music(&capri);
        let acting = look.gesture();
        look.update(&mut capri, frame, talking, music_on);
        if talking && matches!(look.rest(), Rest::Asleep { .. }) {
            return fail(format!("image {frame} : endormie pendant qu'on lui parle"), &story);
        }
        if acting.is_some() && talking {
            tally.talked_in_gesture += 1;
            if look.gesture().is_some() {
                return fail(format!("image {frame} : on se parle, et {acting:?} continue"), &story);
            }
        }
        if look.gesture() == Some(Gesture::Rise) && acting != Some(Gesture::Rise) {
            tally.rises += 1;
        }
        // une intro qu'elle joue : finie ou pas (`Running::tick`, `Running::end_intro`)
        if let Some(end) = look.take_performed() {
            if let Err(what) = performed(end, &mut performing, &look, &capri, hidden, tally) {
                return fail(format!("image {frame} : {what}"), &story);
            }
            if let Err(what) = suite_over(&mut suite, end, &mut look, &mut capri, frame) {
                return fail(what, &story);
            }
            story.push(format!("{frame} : son intro finie : {end:?}"));
            roamer.came_back(now, &capri);
            asked = false;
            away_since = None;
        }
        if let Some((act, steps, _, _)) = performing.as_mut() {
            *steps += 1;
            if *steps > PERFORM_LONGEST || !look.is_performing() {
                return fail(format!("image {frame} : son intro {act:?} dure depuis {steps} pas"), &story);
            }
            let drawn = look.picture(&capri).drawing;
            let fits = match (look.performing(), drawn) {
                (Some(Act::Fall), Drawing::Life(cell)) => cell.strip == life::Strip::Dangle,
                (Some(Act::Walk), Drawing::Life(cell)) => {
                    matches!(cell.strip, life::Strip::Walk | life::Strip::Arrive)
                }
                (Some(Act::Gather), Drawing::Gather(_)) => true,
                _ => false,
            };
            if !fits || look.performing() != Some(*act) || capri.perched().is_some() {
                return fail(format!("image {frame} : son intro {act:?}, et {drawn:?}"), &story);
            }
        }
        // sa marche : jamais à reculons, toujours vers où elle regarde
        if look.is_walking(&capri) || walk.is_some() {
            let feet = capri.feet().x;
            if feet > feet_before + 1e-3 || (look.is_walking(&capri) && !life::FACING.allows(-1.0)) {
                return fail(
                    format!("image {frame} : elle marche à reculons ({feet_before} → {feet})"),
                    &story,
                );
            }
        }
        let hovered = desk.cursor.is_some_and(|cursor| look.is_over(&capri, cursor));
        // ce qui la retient, vu juste avant son pas
        let (here, grooving, held_back) = (look.is_here(), look.is_grooving(), carried(&capri));
        let walking_before = look.is_walking(&capri);
        let seated_before = look.is_seated(&capri) && capri.perched().is_some();
        let perched_before = capri.perched().is_some();
        let rest_before = look.rest();
        tally.grooving += usize::from(grooving);
        tally.talking += usize::from(talking && here);
        tally.hovered += usize::from(hovered && here);
        let busy = Busy { talking, hovered };
        let heading_before = roamer.heading();
        let events = roamer.step(&mut look, &mut capri, &mut desk, busy, now, frame);
        // une prise faite au relâchement : tenue pendant ce pas, lâchée à sa fin
        if let Some(at) = hand.let_go() {
            capri.release(at);
        }

        // --- ce qui doit tenir ---
        for event in events.iter().copied() {
            story.push(format!("{frame} : {event:?}"));
            // elle part, ou apparaît : une nouvelle suite commence
            if matches!(event, Event::Leaves { .. } | Event::Arrived { .. } | Event::TurnedBack) {
                away_since = None;
            }
            format!("{event:?}").hash(&mut print);
            match event {
                Event::Leaves { to, asked: by_hand, walking, .. } => {
                    decisions += 1;
                    if walking {
                        tally.walks += 1;
                        let Choice::Floor(to) = to else {
                            return fail(format!("image {frame} : en marchant vers un bord"), &story);
                        };
                        let feet = capri.feet();
                        let speed = look.walk_speed().unwrap_or(0.0);
                        if !look.is_walking(&capri)
                            || to.x >= feet.x
                            || (to.y - feet.y).abs() > 1.0
                            || f64::from((feet.x - to.x) / speed) > WALK_LONGEST
                        {
                            return fail(format!("image {frame} : une marche de {feet:?} à {to:?}"), &story);
                        }
                        walk = Some((0, feet.x, to.x));
                    }
                    // jamais pendant qu'elle est retenue ; d'elle-même, jamais le curseur sur elle,
                    // jamais la case décochée
                    if !here
                        || talking
                        || grooving
                        || held_back
                        || (!by_hand && (hovered || !roamer.is_enabled()))
                    {
                        return fail(
                            format!(
                                "image {frame} : partie, retenue (là {here}, parole {talking}, musique {grooving}, \
                                 tenue {held_back}, curseur {hovered})"
                            ),
                            &story,
                        );
                    }
                    if by_hand && !std::mem::take(&mut asked) {
                        return fail(format!("image {frame} : partie « à la demande » sans demande"), &story);
                    }
                    let height = look.stature().height;
                    match to {
                        Choice::Ledge(perch) => {
                            tally.to_ledge += 1;
                            // posée là, elle y resterait (au centième de pixel près : `Spot` garde
                            // une distance au côté de sa fenêtre, recalculée en flottants)
                            let fits = perch::follow(&desk.windows, SCREEN, look.stature(), perch.spot);
                            let stays = |p: &perch::Perch| {
                                p.spot.window == perch.spot.window && (p.feet - perch.feet).length() < 0.01
                            };
                            if !fits.as_ref().is_ok_and(stays) {
                                return fail(
                                    format!("image {frame} : un bord où elle ne tient pas : {fits:?}"),
                                    &story,
                                );
                            }
                            if under_cursor(desk.cursor, perch.feet, look.stature().reach, height) {
                                return fail(format!("image {frame} : sur un bord, sous le curseur"), &story);
                            }
                        }
                        Choice::Floor(feet) => {
                            tally.to_floor += 1;
                            let standing =
                                Stature::standing(look.standing_size(), look.standing_feet(), height);
                            let floor = SCREEN.max().y - FLOOR * height - standing.reach.down;
                            if (feet.y - floor).abs() > 0.5 {
                                return fail(format!("image {frame} : debout ailleurs qu'en bas"), &story);
                            }
                            if under_cursor(desk.cursor, feet, standing.reach, height) {
                                return fail(format!("image {frame} : debout, sous le curseur"), &story);
                            }
                            // d'elle-même, debout, de loin en loin seulement ; guettant, pas avant une
                            // attente ordinaire
                            if !by_hand && floor_arrival.is_some_and(|at| now - at < SEATED.min) {
                                return fail(format!("image {frame} : debout, elle saute trop tôt"), &story);
                            }
                            if !by_hand && lurk_arrival.is_some_and(|at| now - at < LURK_STAY.min) {
                                return fail(format!("image {frame} : guettant, debout trop tôt"), &story);
                            }
                        }
                        Choice::Flank(lurk) => {
                            tally.to_flank += 1;
                            // seulement sans aucun bord (où elle ne serait pas sous le curseur ; assise,
                            // sans autre que le sien, si on le lui demande), en vivant sa vie,
                            // « S'asseoir » cochée
                            let Event::Leaves { benches, .. } = event else { unreachable!() };
                            let open = perch::benches(&desk.windows, SCREEN, look.stature())
                                .iter()
                                .any(|bench| free_bench(bench, desk.cursor, look.stature()));
                            let fair = benches == 0 || !open || (by_hand && perched_before);
                            if !fair || !roamer.is_enabled() || !desk.perch || !look.has_stand() {
                                return fail(
                                    format!(
                                        "image {frame} : guetter, avec {benches} bords, sa vie {}",
                                        roamer.is_enabled()
                                    ),
                                    &story,
                                );
                            }
                            // d'elle-même, pas plus tôt que debout, ni, guettant, qu'une attente ordinaire
                            let early = floor_arrival.is_some_and(|at| now - at < SEATED.min)
                                || lurk_arrival.is_some_and(|at| now - at < LURK_STAY.min);
                            if !by_hand && early {
                                return fail(format!("image {frame} : vers un côté trop tôt"), &story);
                            }
                            // là, elle y resterait ; jamais sous le curseur
                            let lurking = look.lurk_stature().expect("ses planches debout");
                            let stays = perch::follow_flank(&desk.windows, SCREEN, lurking, lurk.spot);
                            if !stays.as_ref().is_ok_and(|l| (l.at - lurk.at).length() < 0.01) {
                                return fail(
                                    format!("image {frame} : un côté où elle ne tient pas : {stays:?}"),
                                    &story,
                                );
                            }
                            if under_cursor(desk.cursor, lurk.at, lurking.on(lurk.spot.side), height) {
                                return fail(format!("image {frame} : sur un côté, sous le curseur"), &story);
                            }
                        }
                        Choice::Stay => {
                            return fail(format!("image {frame} : en chemin vers nulle part"), &story);
                        }
                    }
                }
                Event::Stays { asked: by_hand, .. } => {
                    decisions += 1;
                    tally.stays += 1;
                    // sur cet écran, le bas est toujours libre quelque part : une demande ne finit
                    // jamais en « nulle part où aller »
                    if by_hand {
                        return fail(format!("image {frame} : demandée, elle reste où elle est"), &story);
                    }
                }
                Event::Arrived { seated, lurking, lost } => {
                    if lurking {
                        tally.lurks += 1;
                        let lurk = capri.lurking().expect("guettant");
                        let need = look.lurk_stature().expect("ses planches debout");
                        if seated || perch::follow_flank(&desk.windows, SCREEN, need, lurk.spot).is_err() {
                            return fail(
                                format!("image {frame} : arrivée sur un côté qui ne la porte pas"),
                                &story,
                            );
                        }
                    }
                    tally.arrived += 1;
                    // on lui parle : elle revient sur ses pas, elle n'apparaît pas ailleurs
                    if talking {
                        return fail(
                            format!("image {frame} : apparue ailleurs pendant qu'on lui parle"),
                            &story,
                        );
                    }
                    if seated {
                        tally.arrived_seated += 1;
                        let spot = capri.perched().expect("assise, donc posée");
                        if perch::follow(&desk.windows, SCREEN, look.stature(), spot).is_err() {
                            return fail(
                                format!("image {frame} : arrivée sur un bord qui ne la porte pas"),
                                &story,
                            );
                        }
                    } else if lurking {
                        // sur un côté : son attente de guetteuse remplace celle d'avant
                        lurk_arrival = Some(now);
                        floor_arrival = None;
                    } else if !lost {
                        floor_arrival = Some(now);
                    }
                    if lost {
                        tally.arrived_lost += 1;
                        // son côté parti pendant qu'elle y allait : elle se choisit vite une autre place
                        if matches!(heading_before, Some(Choice::Flank(_))) {
                            lurk_arrival = None;
                            floor_arrival = None;
                        }
                    }
                }
                Event::TurnedBack => {
                    tally.turned_back += 1;
                    if !talking {
                        return fail(
                            format!("image {frame} : revenue sur ses pas sans qu'on lui parle"),
                            &story,
                        );
                    }
                }
                Event::Halted => {
                    tally.halted += 1;
                    if !talking || !walking_before || look.is_walking(&capri) {
                        return fail(
                            format!("image {frame} : arrêtée en chemin sans qu'on lui parle"),
                            &story,
                        );
                    }
                }
                Event::Fidgeted => {
                    tally.fidgets += 1;
                    if !seated_before || talking || grooving || music_on || !roamer.is_enabled() {
                        return fail(
                            format!("image {frame} : un éclat alors qu'elle n'est pas libre"),
                            &story,
                        );
                    }
                    if look.gesture() != Some(Gesture::PerchGlitch) {
                        return fail(format!("image {frame} : un éclat qui ne se joue pas"), &story);
                    }
                }
                Event::LiesDown => {
                    tally.lay_downs += 1;
                    // assise, libre, sans musique, sa vie cochée, la place, ses planches là
                    if !seated_before
                        || rest_before != Rest::Awake
                        || talking
                        || music_on
                        || !roamer.is_enabled()
                        || !room
                        || !look.dressed().lying
                        || look.rest() != Rest::Lying
                    {
                        return fail(
                            format!("image {frame} : allongée alors qu'elle ne pouvait pas"),
                            &story,
                        );
                    }
                }
                Event::FallsAsleep { lying } => {
                    if lying {
                        tally.dozes_lying += 1
                    } else {
                        tally.dozes_standing += 1
                    }
                    if talking || music_on || !roamer.is_enabled() || look.rest() != (Rest::Asleep { lying })
                    {
                        return fail(
                            format!("image {frame} : endormie alors qu'elle ne pouvait pas"),
                            &story,
                        );
                    }
                }
                Event::WakesUp { lying, woken } => {
                    if woken {
                        tally.woken += 1
                    } else {
                        tally.woke_alone += 1
                    }
                    if matches!(look.rest(), Rest::Asleep { .. })
                        || (!lying && !woken && look.rest() != Rest::Waking)
                    {
                        return fail(format!("image {frame} : réveillée, et toujours endormie"), &story);
                    }
                }
                Event::Unsettled { why, .. } => {
                    tally.unsettled += 1;
                    // jamais pour rien : plus posée (pas cachée : c'est un geste), ou rassise sans la
                    // place
                    let fair = match why {
                        Unsettled::LedgeGone => {
                            !look.is_away() && (capri.perched().is_none() || !look.is_seated(&capri))
                        }
                        Unsettled::NoRoom => !room,
                        Unsettled::Undressed => !look.dressed().lying,
                    };
                    if !fair || look.rest() != Rest::Awake {
                        return fail(format!("image {frame} : {why:?} sans raison"), &story);
                    }
                }
                Event::SitsUp { .. } => {
                    tally.sit_ups += 1;
                    if look.rest() != Rest::Awake || capri.perched().is_none() {
                        return fail(format!("image {frame} : relevée, et {:?}", look.rest()), &story);
                    }
                }
                Event::Refused(hold) => {
                    tally.refused += 1;
                    let fair = match hold {
                        Hold::Carried => held_back,
                        Hold::Away => !here,
                        Hold::Talking => talking,
                        Hold::Music => grooving,
                        Hold::Walking => walking_before,
                    };
                    if !fair || !std::mem::take(&mut asked) {
                        return fail(format!("image {frame} : refus sans raison : {hold:?}"), &story);
                    }
                }
            }
        }
        // effacée pour « Voir une intro » : la suite se joue, le film d'abord (s'il est là), puis
        // l'entrée (« se matérialise » : elle revient tout de suite)
        if let Some(entry) = encore.filter(|_| look.is_offstage()) {
            encore = None;
            tally.encores_started += 1;
            away_since = None;
            let mut playing = Suite::new(entry, with_film, false);
            if with_film {
                film = Some(frame + intro::FILM.frames());
                tally.encore_films += 1;
            } else {
                let place = capri_place(&capri);
                playing.begin_entry(&mut look, &mut capri, frame);
                if let Err(what) = entry_begun(&playing, &look, place, frame, &mut performing, tally) {
                    return fail(what, &story);
                }
                if playing.is_over() {
                    roamer.came_back(now, &capri);
                    asked = false;
                }
            }
            suite = (!playing.is_over()).then_some(playing);
            story.push(format!("{frame} : l'intro {} commence", entry.key));
        }
        if look.is_between() {
            return fail(format!("image {frame} : effacée entre deux places, et pas déplacée"), &story);
        }
        if hidden != look.is_hidden() {
            return fail(
                format!("image {frame} : cachée {hidden}, mais is_hidden {}", look.is_hidden()),
                &story,
            );
        }
        if roamer.heading().is_some() && !look.is_traveling() {
            return fail(format!("image {frame} : une destination, mais pas en chemin"), &story);
        }
        if desk.reads > decisions {
            return fail(format!("{} lectures pour {decisions} décisions", desk.reads), &story);
        }
        // ne guettant plus (son côté perdu, une prise, une intro…), son attente de guetteuse ne compte
        // plus (une prise : 5 minutes, plus longue)
        if capri.lurking().is_none() && !look.is_traveling() {
            lurk_arrival = None;
        }
        // elle guette : sortie tout entière une fois là, ce qui se voit d'elle dans l'écran et non
        // vide, son côté sous elle ; sa glissade finit ; ni geste ni repos ; sa boucle, penchée
        if let Some(lurk) = capri.lurking() {
            if let Stance::Lurking { toward, .. } = capri.stance()
                && capri.is_gliding()
            {
                tally.glide_steps += 1;
                // une glissade : dans un sens, d'un côté (revenue sur ses pas, ou ailleurs, une autre)
                let same = glide_to.is_some_and(|(to, spot)| to == toward && spot.window == lurk.spot.window);
                gliding_for = if same { gliding_for + 1 } else { 1 };
                glide_to = Some((toward, lurk.spot));
                if gliding_for > (GLIDE * 24.0).ceil() as u64 + 1 {
                    return fail(format!("image {frame} : elle glisse depuis {gliding_for} pas"), &story);
                }
            } else {
                gliding_for = 0;
                glide_to = None;
            }
            if look.gesture().is_some() || look.rest() != Rest::Awake {
                return fail(
                    format!("image {frame} : guettant, {:?} et {:?}", look.gesture(), look.rest()),
                    &story,
                );
            }
            if !look.is_gone() {
                let picture = look.picture(&capri);
                let leaning =
                    picture.lean.is_some_and(|lean| lean.wall == lurk.at.x && lean.side == lurk.spot.side);
                if !matches!(picture.drawing, Drawing::Stand(_)) || !leaning {
                    return fail(format!("image {frame} : guettant, dessinée {picture:?}"), &story);
                }
            }
            if look.is_here() {
                tally.lurk_steps += 1;
                let (seen, shown) = (look.visible(&capri), look.shown(&capri));
                // tout ce qui se voit d'elle dans l'écran, et la place de son bandeau dedans
                let within = |inner: Rect| {
                    inner.min.x >= seen.min.x - 0.01
                        && inner.max().x <= seen.max().x + 0.01
                        && inner.min.y >= seen.min.y - 0.01
                        && inner.max().y <= seen.max().y + 0.01
                };
                if capri.lurk_out() != Some(1.0)
                    || seen.size.x <= 0.0
                    || seen.size.y <= 0.0
                    || outside_seen(&capri, &look) > 0.01
                    || !within(shown)
                {
                    return fail(
                        format!(
                            "image {frame} : là, guettant, sortie {:?}, on voit {seen:?}, son bandeau sur {shown:?}",
                            capri.lurk_out()
                        ),
                        &story,
                    );
                }
                let need = look.lurk_stature().expect("ses planches debout");
                if !desk.perch || perch::follow_flank(&desk.windows, SCREEN, need, lurk.spot).is_err() {
                    return fail(format!("image {frame} : guettant sans côté sous elle"), &story);
                }
            }
        } else {
            gliding_for = 0;
            glide_to = None;
        }
        // un geste à la fois, et il finit ; ni en marchant, ni tenue
        if let (Some(gesture), Some(since)) = (look.gesture(), look.gesture_since()) {
            if frame >= since + gesture.frames() || gesture.frames() > LONGEST_GESTURE {
                return fail(format!("image {frame} : {gesture:?} depuis l'image {since}, pas fini"), &story);
            }
            if look.is_walking(&capri) || carried(&capri) || !look.is_here() {
                return fail(format!("image {frame} : {gesture:?} en marchant, tenue ou partie"), &story);
            }
        }
        let shown = look.picture(&capri);
        let drawn = match shown.drawing {
            Drawing::Life(cell) => match cell.strip {
                life::Strip::Glitch => Some(Gesture::Rise),
                life::Strip::Grin => Some(Gesture::Grin),
                _ => None,
            },
            Drawing::Perch(cell) if cell.strip == sheets::Strip::PerchGlitch => Some(Gesture::PerchGlitch),
            _ => None,
        };
        if drawn.is_some() && drawn != look.gesture() {
            return fail(format!("image {frame} : {drawn:?} dessiné, {:?} en cours", look.gesture()), &story);
        }
        // tenue, avec sa planche en l'air, elle pend (sa mèche vérifiée au glissé, plus haut)
        if look.is_dangling(&capri) {
            tally.dangling += 1;
        } else if capri.is_held() && look.is_here() && look.has_move(life::Strip::Dangle) {
            return fail(format!("image {frame} : tenue sans pendre"), &story);
        }
        // sa marche finit : elle arrive, ou elle s'arrête (on lui parle, une prise, cachée)
        if look.is_walking(&capri) {
            tally.walking += 1;
            if let Some((steps, _, _)) = walk.as_mut() {
                *steps += 1;
                if *steps > (WALK_LONGEST * 24.0) as u64 + 2 {
                    return fail(format!("image {frame} : elle marche depuis {steps} pas"), &story);
                }
            }
        } else if let Some((_, _, to)) = walk.take() {
            // une marche finie sans rien pour la déranger : elle arrive là où elle allait, ou au
            // bord de l'écran, et c'est une arrivée debout
            let arrived = events.contains(&Event::Arrived { seated: false, lurking: false, lost: false });
            if calm_life && !carried(&capri) && look.is_here() && !hidden {
                if !arrived {
                    return fail(format!("image {frame} : sa marche finie sans arriver"), &story);
                }
                let feet = capri.feet().x;
                if (feet - to).abs() > 1.0 && edges(&capri).is_empty() {
                    return fail(format!("image {frame} : arrivée en {feet}, elle allait en {to}"), &story);
                }
                if capri.stance() != Stance::Floating {
                    return fail(format!("image {frame} : arrivée, et {:?}", capri.stance()), &story);
                }
            }
            tally.walks_arrived += usize::from(arrived);
        }
        // jamais coincée (une intro qu'elle joue est bornée plus haut)
        // (sa glissade de derrière une fenêtre se compte en pas, plus haut, pas en images : quand le
        // temps passe par à-coups, son corps n'avance que d'un pas)
        if !hidden && !look.is_here() && !look.is_performing() && !capri.is_gliding() {
            let since = *away_since.get_or_insert(frame);
            if frame - since > SETTLED_WITHIN && frame - toggled_at > SETTLED_WITHIN {
                return fail(format!("image {frame} : montrée, mais pas là depuis l'image {since}"), &story);
            }
        } else {
            away_since = None;
        }
        if hidden && !look.is_gone() && frame - toggled_at > SETTLED_WITHIN && !capri.is_gliding() {
            return fail(format!("image {frame} : cachée, mais à moitié effacée"), &story);
        }
        // son repos : jamais endormie pendant qu'on lui parle ; allongée, toujours sur un bord où elle
        // tient allongée, ses planches là ; endormie debout, immobile ; et elle finit toujours
        let rest = look.rest();
        if talking && matches!(rest, Rest::Asleep { .. }) {
            return fail(format!("image {frame} : endormie pendant qu'on lui parle"), &story);
        }
        match rest {
            Rest::Lying | Rest::Asleep { lying: true } => {
                tally.lying_steps += 1;
                tally.asleep_steps += usize::from(rest != Rest::Lying);
                if capri.perched().is_none() || !look.dressed().lying || !room_to_lie(&look, &capri, &desk) {
                    return fail(
                        format!(
                            "image {frame} : allongée hors d'un bord (posée {}, planches {}, place {})",
                            capri.perched().is_some(),
                            look.dressed().lying,
                            room_to_lie(&look, &capri, &desk)
                        ),
                        &story,
                    );
                }
                let since = *lain_at.get_or_insert(now);
                if now - since > LIE_LONGEST + 180.0 {
                    return fail(format!("image {frame} : allongée depuis {} s", now - since), &story);
                }
            }
            _ => lain_at = None,
        }
        match rest {
            Rest::Asleep { lying: false } | Rest::Waking => {
                tally.asleep_steps += usize::from(rest != Rest::Waking);
                if capri.stance() != Stance::Floating || capri.perched().is_some() || !look.dressed().sleeping
                {
                    return fail(format!("image {frame} : endormie debout, et {:?}", capri.stance()), &story);
                }
                let since = *dozing_at.get_or_insert(now);
                if now - since > UP_ASLEEP.max + 180.0 {
                    return fail(format!("image {frame} : endormie debout depuis {} s", now - since), &story);
                }
            }
            _ => dozing_at = None,
        }
        // jamais dessinée dans une planche qui n'est pas là
        match look.picture(&capri).drawing {
            Drawing::Lie(_) if !look.dressed().lying => {
                return fail(format!("image {frame} : allongée sans ses planches"), &story);
            }
            Drawing::Life(cell) if life::Strip::NAP.contains(&cell.strip) && !look.dressed().sleeping => {
                return fail(format!("image {frame} : endormie debout sans ses planches"), &story);
            }
            _ => {}
        }
        // là et debout, éveillée, son tour avance (un geste l'arrête au plus le temps du plus long)
        let upright = capri.stance() == Stance::Floating || capri.lurking().is_some();
        if !hidden && look.is_here() && upright && !look.is_resting() {
            let mut probe = capri.clone();
            probe.step(0.0, frame + LONGEST_GESTURE + 1);
            if probe.tour() == capri.tour() {
                return fail(format!("image {frame} : là, debout, son tour figé"), &story);
            }
        }
        // posée, avec ses planches assises : dessinée assise (sauf en train de s'effacer debout,
        // posée entre-temps), et toujours sur son bord, même en train d'y apparaître ou de s'en effacer
        if look.has_perch()
            && !capri.is_held()
            && let Some(seat) = capri.seat()
        {
            let picture = look.picture(&capri);
            let seat_uv = match picture.drawing {
                Drawing::Perch(cell) => Some(cell.strip.seat_uv()),
                Drawing::Lie(cell) => Some(cell.strip.seat_uv()),
                _ => None,
            };
            match seat_uv {
                Some(uv) => {
                    let line =
                        picture.rect.min + Vec2::new(picture.rect.size.x * uv.x, picture.rect.size.y * uv.y);
                    if (line - seat).length() > 2.0 {
                        return fail(
                            format!("image {frame} : assise à {line:?}, son bord en {seat:?}"),
                            &story,
                        );
                    }
                }
                _ if !look.is_away() => {
                    return fail(
                        format!("image {frame} : posée, mais dessinée debout : {picture:?}"),
                        &story,
                    );
                }
                _ => {}
            }
        }
        // hors d'une intro qu'elle joue, jamais plus hors de l'écran qu'au pas précédent ; et lâchée en
        // partie dehors, elle revient dedans à chaque pas (guettant : ce qui se voit d'elle)
        let out = outside_seen(&capri, &look);
        if look.is_performing() {
            last_out = None;
        } else {
            if let Some(last) = last_out
                && out > last + 0.01
            {
                return fail(format!("image {frame} : plus hors de l'écran ({last} → {out})"), &story);
            }
            if out > 0.5 {
                tally.outside += 1;
                if capri.stance() == Stance::Floating && last_out.is_some_and(|last| out >= last) {
                    return fail(format!("image {frame} : lâchée dehors, elle n'y revient pas"), &story);
                }
            }
            last_out = Some(out);
        }
        // son empreinte : où elle est, ce qu'on dessine
        let picture = look.picture(&capri);
        (picture.rect.min.x as i64, picture.rect.min.y as i64, capri.perched().map(|spot| spot.window))
            .hash(&mut print);
    }
    Ok(print.finish())
}

/// À la prise, son corps ne saute pas : debout, il ne bouge que de ce que la prise rejoue du
/// curseur (son glissé vers sa mèche compris), même en partie hors de l'écran ; assise, elle se
/// lève sous le curseur (ailleurs).
fn no_jump(
    capri: &Capri,
    grab: &capri_core::input::Grab,
    seated: bool,
    before: Vec2,
    tally: &mut Tally,
) -> Result<(), String> {
    if seated {
        return Ok(());
    }
    let moved = (capri.position() - before).length();
    if moved > 1.5 * trail_length(grab) + 1.0 {
        return Err(format!("un saut de {moved} px à la prise (curseur : {} px)", trail_length(grab)));
    }
    tally.grabbed_outside += usize::from(outside(capri) > 0.5);
    Ok(())
}

/// Guettant (dessinée penchée), le point de son image droite que tient le curseur de la prise,
/// depuis le coin haut gauche de son image ; `None` si elle n'est pas penchée.
fn held_point(look: &Look, capri: &Capri, grab: &capri_core::input::Grab) -> Option<Vec2> {
    let picture = look.picture(capri);
    picture.lean.map(|lean| lean.upright(grab.at) - picture.rect.min)
}

/// Prise guettant : elle se redresse autour de ce que tient le curseur, qui reste sous lui ; il ne
/// s'en écarte que de ce que la prise rejoue du curseur (son glissé vers sa mèche compris), même
/// en partie hors de l'écran (redressée, son image peut dépasser de l'écran, comme une prise en
/// partie dehors pendant une intro : ensuite, jamais plus, et lâchée, elle y revient).
fn no_jump_lurking(capri: &Capri, grab: &capri_core::input::Grab, held: Vec2) -> Result<(), String> {
    let gap = (capri.position() + held - grab.at).length();
    if gap > 1.5 * trail_length(grab) + 0.01 {
        return Err(format!(
            "guettant, le point tenu à {gap} px du curseur (curseur : {} px)",
            trail_length(grab)
        ));
    }
    Ok(())
}

/// Sa place pour une intro qu'elle joue, telle que `Look::perform` la prend (ses pieds, debout).
fn capri_place(capri: &Capri) -> Vec2 {
    let mut probe = capri.clone();
    probe.set_at(probe.feet());
    probe.feet()
}

/// Une intro qu'elle jouait a fini par `end` : ce qui doit tenir alors.
fn performed(
    end: End,
    performing: &mut Option<(Act, u64, Vec2, bool)>,
    look: &Look,
    capri: &Capri,
    hidden: bool,
    tally: &mut Tally,
) -> Result<(), String> {
    let Some((act, _, place, _)) = performing.take() else {
        return Err(format!("une intro finie sans intro : {end:?}"));
    };
    if look.is_performing() {
        return Err(format!("{act:?} finie et toujours jouée"));
    }
    match end {
        End::Played { .. } => {
            tally.played += 1;
            // jouée jusqu'au bout : à sa place (une autre taille l'aurait coupée)
            if (capri.feet() - place).length() > 1.0 {
                return Err(format!("{act:?} jouée, et en {:?} au lieu de {place:?}", capri.feet()));
            }
        }
        End::Cut { why: intro::Cut::Hidden, .. } => {
            tally.cut_hidden += 1;
            if !hidden || !look.is_gone() {
                return Err(format!("{act:?} coupée en la cachant, et pas cachée"));
            }
        }
        End::Cut { why: intro::Cut::Talked, .. } => {
            tally.cut_talked += 1;
            // matérialisée à sa place
            if (capri.feet() - place).length() > 1.0 || capri.stance() != Stance::Floating {
                return Err(format!("{act:?} coupée en lui parlant, et en {:?}", capri.feet()));
            }
        }
        End::Cut { why: intro::Cut::Resized, .. } => {
            tally.cut_resized += 1;
            // à sa place, à sa nouvelle taille (dans l'écran), sans se relever
            let mut probe = capri.clone();
            probe.set_at(place);
            if (capri.feet() - probe.feet()).length() > 1.0 || capri.stance() != Stance::Floating {
                return Err(format!("{act:?} coupée par sa taille, et en {:?}", capri.feet()));
            }
            if look.gesture().is_some() {
                return Err(format!("{act:?} coupée par sa taille, et {:?}", look.gesture()));
            }
        }
        End::Cut { why: intro::Cut::Grabbed, .. } => {
            tally.cut_grabbed += 1;
            if !capri.is_held() && !matches!(capri.stance(), Stance::Thrown { .. }) {
                return Err(format!("{act:?} coupée en l'attrapant, et pas tenue : {:?}", capri.stance()));
            }
        }
        other => return Err(format!("{act:?} : une fin qui n'est pas celle d'une entrée, {other:?}")),
    }
    Ok(())
}

/// Un bord où elle pourrait aller sans être sous le curseur (`roam::choose`, la marge de
/// `roam::AVOID`), quelque part entre ses deux bouts ?
fn free_bench(bench: &perch::Bench, cursor: Option<Vec2>, stature: Stature) -> bool {
    let Some(c) = cursor else { return true };
    let (reach, margin) = (stature.reach, AVOID * stature.height);
    let y = bench.y();
    let level = y - reach.up - margin <= c.y && c.y <= y + reach.down + margin;
    !level || bench.lo <= c.x - reach.right - margin || bench.hi >= c.x + reach.left + margin
}

/// Le curseur serait-il sur elle, le point posé en `at`, la place `reach` autour, avec la marge de
/// `roam::AVOID` ?
fn under_cursor(cursor: Option<Vec2>, at: Vec2, reach: perch::Reach, height: f32) -> bool {
    let margin = AVOID * height - 0.5;
    cursor.is_some_and(|c| {
        at.x - reach.left - margin < c.x
            && c.x < at.x + reach.right + margin
            && at.y - reach.up - margin < c.y
            && c.y < at.y + reach.down + margin
    })
}

/// Debout, assise, ses planches de vie, ses gestes (bits de [`ALL_MOVES`]) et son repos (bits de
/// [`RESTS`]).
const WARDROBES: [(bool, bool, bool, u8); 10] = [
    (true, true, true, ALL_MOVES | RESTS),
    (true, true, true, 0),
    // sans pose assise : pas allongée ; sans pose debout : pas de sommeil debout
    (true, false, true, ALL_MOVES | RESTS),
    (false, true, true, ALL_MOVES | RESTS),
    (true, true, false, ALL_MOVES | RESTS),
    (false, false, false, RESTS),
    // la marche et sa planche en l'air seulement ; ses gestes seulement
    (true, true, true, 0b00_0011),
    (true, true, true, 0b11_1100 | RESTS),
    // sans sa planche en l'air ni se relever
    (true, true, true, 0b11_1001),
    // allongée seulement, sans l'éclat de sa pose assise (elle se rassoit d'un coup)
    (true, true, true, 0b0110_1111),
];

#[test]
fn random_gestures_never_leave_her_stuck_hidden_or_off_her_edge() {
    let mut failures = Vec::new();
    let mut total = Tally::default();
    for wardrobe in WARDROBES {
        let mut tally = Tally::default();
        let before = failures.len();
        for seed in 0..SEEDS {
            if let Err(what) = live(seed, wardrobe, &mut tally) {
                failures.push(format!("graine {seed}, garde-robe {wardrobe:?} : {what}"));
            }
        }
        println!("{wardrobe:?} : {tally:?}");
        // la cause d'un échec d'abord : les seuils ensuite
        for failure in failures[before..].iter().take(4) {
            println!("{failure}");
        }
        assert!(failures.is_empty(), "{} vies en échec ; voir plus haut", failures.len());
        // dans chaque garde-robe, des départs vérifiés, des arrivées, des retenues
        let (stand_sheets, perch_sheets, _, moves) = wardrobe;
        let on = |bit: u8| moves & (1 << bit) != 0;
        // et chacun de ses gestes, quand sa planche est là
        if stand_sheets {
            assert!(
                !on(0) || (tally.walks >= 3 && tally.walking > 100 && tally.walks_arrived >= 3),
                "{wardrobe:?} : {tally:?}"
            );
            assert!(!on(1) || (tally.gliding > 100 && tally.off_an_edge > 5), "{wardrobe:?} : {tally:?}");
            assert!(on(0) || tally.walks == 0, "{wardrobe:?} : {tally:?}");
            assert!(!on(1) || tally.dangling > 1000, "{wardrobe:?} : {tally:?}");
            assert!(on(1) || tally.dangling == 0, "{wardrobe:?} : {tally:?}");
            assert!(!on(2) || tally.rises > 50, "{wardrobe:?} : {tally:?}");
            assert!(!on(3) || (tally.cheers > 50 && tally.clicked > 50), "{wardrobe:?} : {tally:?}");
        }
        assert!(perch_sheets && on(4) || tally.fidgets == 0, "{wardrobe:?} : {tally:?}");
        assert!(tally.clicks > 100 && tally.turn_ends > 100, "{wardrobe:?} : {tally:?}");
        assert!(tally.grabs_by_step > 100 && tally.grabs_by_release > 10, "{wardrobe:?} : {tally:?}");
        assert!(tally.to_ledge > 50 && tally.to_floor > 10 && tally.arrived > 50, "{wardrobe:?} : {tally:?}");
        assert!(
            tally.talking > 1000 && tally.hovered > 100 && tally.turned_back + tally.refused > 50,
            "{tally:?}"
        );
        assert!(!perch_sheets || tally.grooving > 100, "{wardrobe:?} : {tally:?}");
        // son repos, quand ses planches et la pose qui les porte sont là ; jamais sinon
        let (lying, sleeping) = (perch_sheets && on(6), stand_sheets && on(7));
        assert!(
            !lying || (tally.lay_downs > 10 && tally.dozes_lying > 5 && tally.sit_ups > 3 && tally.woken > 3),
            "{wardrobe:?} : {tally:?}"
        );
        assert!(lying || tally.lay_downs + tally.dozes_lying == 0, "{wardrobe:?} : {tally:?}");
        assert!(!sleeping || tally.dozes_standing > 3, "{wardrobe:?} : {tally:?}");
        assert!(sleeping || tally.dozes_standing == 0, "{wardrobe:?} : {tally:?}");
        assert!(tally.calm > 100, "{wardrobe:?} : {tally:?}");
        // elle guette, avec ses planches debout ; jamais sans
        assert!(
            !stand_sheets
                || (tally.to_flank > 20
                    && tally.lurks > 20
                    && tally.lurk_steps > 500
                    && tally.glide_steps > 200),
            "{wardrobe:?} : {tally:?}"
        );
        assert!(stand_sheets || tally.to_flank + tally.lurks == 0, "{wardrobe:?} : {tally:?}");
        total.to_ledge += tally.to_ledge;
        total.to_floor += tally.to_floor;
        total.stays += tally.stays;
        total.arrived_seated += tally.arrived_seated;
        total.arrived_lost += tally.arrived_lost;
        total.refused += tally.refused;
        total.turned_back += tally.turned_back;
        total.hidden_on_the_way += tally.hidden_on_the_way;
        total.grabbed_appearing += tally.grabbed_appearing;
        total.touched_off += tally.touched_off;
        total.grabbed_in_gesture += tally.grabbed_in_gesture;
        total.grabbed_walking += tally.grabbed_walking;
        total.talked_in_gesture += tally.talked_in_gesture;
        total.fidgets += tally.fidgets;
        total.halted += tally.halted;
        total.walks += tally.walks;
        total.walks_arrived += tally.walks_arrived;
        total.stuck += tally.stuck;
        total.unstuck += tally.unstuck;
        for (sum, count) in total.performed.iter_mut().zip(tally.performed) {
            *sum += count;
        }
        total.films += tally.films;
        total.film_cut_hidden += tally.film_cut_hidden;
        total.film_cut_talked += tally.film_cut_talked;
        total.encore_films += tally.encore_films;
        total.played += tally.played;
        total.cut_hidden += tally.cut_hidden;
        total.cut_talked += tally.cut_talked;
        total.cut_grabbed += tally.cut_grabbed;
        total.cut_resized += tally.cut_resized;
        total.encores += tally.encores;
        total.encores_started += tally.encores_started;
        total.encores_refused += tally.encores_refused;
        total.encores_turned_back += tally.encores_turned_back;
        total.grabbed_outside += tally.grabbed_outside;
        total.gliding_outside += tally.gliding_outside;
        total.outside += tally.outside;
        total.walked_in += tally.walked_in;
        total.lay_downs += tally.lay_downs;
        total.dozes_lying += tally.dozes_lying;
        total.dozes_standing += tally.dozes_standing;
        total.sit_ups += tally.sit_ups;
        total.woke_alone += tally.woke_alone;
        total.woken += tally.woken;
        total.woken_by_claude += tally.woken_by_claude;
        total.lying_steps += tally.lying_steps;
        total.asleep_steps += tally.asleep_steps;
        total.ledge_gone_lying += tally.ledge_gone_lying;
        total.hidden_lying += tally.hidden_lying;
        total.grabbed_resting += tally.grabbed_resting;
        total.waited_for_sheets += tally.waited_for_sheets;
        total.sheets_failed += tally.sheets_failed;
        total.unsettled += tally.unsettled;
        total.flank_gone += tally.flank_gone;
        total.hidden_lurking += tally.hidden_lurking;
        total.grabbed_lurking += tally.grabbed_lurking;
        total.talked_lurking += tally.talked_lurking;
        total.touched_lurking += tally.touched_lurking;
        // « Voir une intro », servi et refusé, dans chaque garde-robe
        assert!(tally.encores_started > 30 && tally.encores_refused > 10, "{wardrobe:?} : {tally:?}");
    }
    println!("en tout : {total:?}");
    assert!(failures.is_empty(), "{} vies en échec sur {}", failures.len(), SEEDS as usize * WARDROBES.len());
    // chaque chemin a bien été exercé
    assert!(total.stays > 5 && total.arrived_seated > 50 && total.arrived_lost > 0, "{total:?}");
    assert!(
        total.turned_back > 10 && total.hidden_on_the_way > 0 && total.grabbed_appearing > 0,
        "{total:?}"
    );
    assert!(total.touched_off > 10, "{total:?}");
    // une prise pendant un geste, pendant sa marche ; on lui parle pendant un geste
    assert!(
        total.grabbed_in_gesture > 20 && total.grabbed_walking > 5 && total.talked_in_gesture > 20,
        "{total:?}"
    );
    // ses éclats assise, des marches arrêtées en chemin parce qu'on lui parle, d'autres allées au
    // bout ; des relâchements perdus, et des clics qui l'ont lâchée
    assert!(
        total.fidgets > 5 && total.halted > 2 && total.walks > 20 && total.walks_arrived > 20,
        "{total:?}"
    );
    assert!(total.stuck > 20 && total.unstuck > 10, "{total:?}");
    // chacune de ses intros jouées, jusqu'au bout, et coupée de chaque façon
    assert!(total.performed.iter().all(|&count| count > 20), "{total:?}");
    assert!(
        total.played > 50 && total.cut_hidden > 3 && total.cut_talked > 3 && total.cut_grabbed > 3,
        "{total:?}"
    );
    assert!(total.encores_turned_back > 3 && total.cut_resized > 3, "{total:?}");
    // le film : joué puis l'entrée, coupé de chaque façon, et rejoué pour « Voir une intro »
    assert!(
        total.films > 200
            && total.film_cut_hidden > 50
            && total.film_cut_talked > 20
            && total.encore_films > 100,
        "{total:?}"
    );
    // son repos, par chaque chemin : réveillée d'elle-même et par quelqu'un (dont Claude Code),
    // relevée, son bord parti sous elle allongée, cachée allongée, attrapée au repos, ses planches
    // attendues, et en échec
    assert!(
        total.woke_alone > 3
            && total.woken_by_claude > 3
            && total.sit_ups > 10
            && total.ledge_gone_lying > 0
            && total.hidden_lying > 0
            && total.grabbed_resting > 0
            && total.waited_for_sheets > 100
            && total.sheets_failed > 0
            && total.unsettled > 10,
        "{total:?}"
    );
    // guettant : son côté parti sous elle, cachée, attrapée, on lui parle, décochée ou trop grande
    assert!(
        total.flank_gone > 10
            && total.hidden_lurking > 10
            && total.grabbed_lurking > 10
            && total.talked_lurking > 10
            && total.touched_lurking > 3,
        "{total:?}"
    );
    // prise en partie hors de l'écran sans saut, tenue dehors, revenue dedans ; entrée en marchant
    assert!(
        total.grabbed_outside > 10
            && total.gliding_outside > 3
            && total.outside > 10
            && total.walked_in > 100,
        "{total:?}"
    );
}

#[test]
fn the_same_seed_gives_the_same_life() {
    let mut tally = Tally::default();
    for seed in [3, 17] {
        let first = live(seed, WARDROBES[0], &mut tally).expect("une vie");
        assert_eq!(live(seed, WARDROBES[0], &mut tally), Ok(first), "graine {seed}");
    }
    assert_ne!(live(3, WARDROBES[0], &mut tally), live(17, WARDROBES[0], &mut tally));
}
