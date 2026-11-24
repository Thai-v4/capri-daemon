//! Elle vit sa vie : quand elle change de place, et où elle va. Tout est décidé ici, sans rien
//! savoir du système (à ne pas confondre avec `sheets::life`, ses planches pour apparaître et
//! s'effacer).
//!
//! **Ce qu'elle reçoit** : une horloge passée en paramètre (des secondes), les bords où elle
//! tiendrait ([`perch::benches`], d'après les seuls numéros et rectangles des fenêtres, que l'app
//! lit au moment où elle choisit : [`World::seen`]), la place du curseur, et ce qui la retient
//! ([`Busy`], et ce qu'elle sait déjà d'elle-même). Le hasard vient d'un générateur à graine écrit
//! ici ([`Rng`]) : la même graine donne la même vie, ce qui rend les essais déterministes.
//!
//! **Le voyage** : elle s'efface là où elle est, puis apparaît là où elle va ([`Look::travel`],
//! [`Look::arrive`]), sans que la fenêtre de l'overlay se cache : elle n'est pas « cachée ».
//! Assise, ce sont ses dessins assis ; debout, ses planches de vie. Juste avant qu'elle apparaisse
//! sur un bord, sa fenêtre est relue ([`World::recheck`]) : bougée, elle la suit ; partie, elle
//! apparaît debout là où elle allait, et se choisit vite une autre place ([`LOST`]). Jamais
//! invisible : effacée pour changer de place, elle est déplacée et apparaît dans le même pas. Si
//! on lui parle pendant qu'elle s'efface pour partir (le champ de saisie ou le micro, ouverts là où
//! elle est), elle revient sur ses pas et n'ira pas plus loin ([`Look::turn_back`]).
//!
//! **Où** ([`choose`]) : elle préfère être assise. Ses dessins assis sont de son nouveau dessin,
//! comme sa boucle debout ; ses planches de vie, qui la font apparaître debout, montrent un dessin
//! plus ancien : d'un bord à un autre, on ne voit que le nouveau. Un bord est donc tiré au sort,
//! avec des poids, parmi ceux où elle tient : jamais celui où elle est quand un autre existe,
//! moins souvent un autre morceau de sa propre fenêtre ([`SAME_WINDOW`]), un peu plus souvent un
//! long morceau, et jamais là où le curseur serait sur elle ([`AVOID`]). Sans aucun bord, elle va
//! se tenir debout en bas de l'écran ([`FLOOR`]) ; assise, sans autre bord, elle reste, sauf si on
//! lui demande de changer de place : alors elle va debout en bas de l'écran.
//!
//! **Quand** ([`Roam`]) : après un moment tiré dans une plage, qui dépend de ce qui vient de se
//! passer ([`Stay`]). Pendant qu'on la tient ou qu'elle vole, qu'on lui parle au micro, qu'elle
//! parle ou répond, que le champ de saisie est ouvert, qu'elle se balance sur la musique, qu'elle
//! apparaît ou s'efface, qu'elle est cachée, ou que le curseur est sur elle, elle ne part pas
//! d'elle-même, et son départ recule ([`PATIENCE`]). « Changer de place » ([`Roamer::ask`]) la
//! fait partir tout de suite, sauf si l'une de ces raisons l'en empêche (le curseur sur elle
//! excepté : c'est là qu'on ouvre son menu). Debout sans aucun bord, elle revient chercher un bord
//! à la cadence de [`STANDING`], mais ne change d'elle-même de place debout qu'une fois le temps de
//! [`SEATED`] passé depuis sa dernière arrivée debout : sinon, toutes fenêtres agrandies (ou sous
//! Linux), elle sauterait le long du bas de l'écran toute la journée. Ce que fait l'utilisateur ne
//! compte jamais comme un bord perdu ([`Roamer::touched`]), et la montrer après l'avoir cachée ne
//! raccourcit jamais une attente en cours ([`Roamer::came_back`]).
//!
//! **Elle marche** : debout en bas de l'écran, quand elle va debout ailleurs en bas de l'écran
//! ([`Choice::Floor`]), dans le sens où regarde le dessin de sa marche
//! ([`crate::sheets::life::FACING`]), et pas plus de [`WALK_LONGEST`], elle y va en marchant
//! ([`Look::walk`]) au lieu de s'effacer ; sinon (sans sa planche de marche, ou dans l'autre sens),
//! elle s'efface et réapparaît, comme avant. Attrapée en chemin, elle pend et reste où on la
//! lâche ; cachée, elle s'arrête et s'efface ; quand on lui parle, elle s'arrête là
//! ([`Event::Halted`]) et reprend sa boucle. « Changer de place » pendant qu'elle marche est refusé
//! ([`Hold::Walking`]) : elle arrive bientôt.
//!
//! **Elle varie** : assise et libre, de temps en temps ([`FIDGET`], tiré d'un hasard à part mais
//! de la même graine, pour que ses départs restent ceux d'avant), un éclat ([`Look::fidget`]) ;
//! jamais pendant qu'on se parle, qu'elle se balance ou que la musique joue, ni pendant un autre
//! geste ; retenue, il recule de [`PATIENCE`]. Seulement quand elle vit sa vie (la case cochée, et
//! jamais pendant une répétition). Un geste en cours retient ses départs le temps qu'il finisse
//! (4 s au plus, [`crate::look::LONGEST_GESTURE`]), sans les reculer : elle part juste après.
//!
//! **Elle se repose** ([`Look::lie_down`], [`Look::doze`], [`Look::sit_up`], [`Look::rouse`]) :
//! tranquille depuis un moment ([`LIE_AFTER`] : personne ne lui parle, ne clique sur elle, ne la
//! tient ; posée, pas de musique ; ses propres trajets ne comptent pas), assise depuis au moins
//! [`PATIENCE`] et s'il y a la place, elle s'allonge sur son bord ; toujours tranquille, allongée
//! depuis [`DOZE_AFTER`], elle s'endort. Debout et immobile, tranquille depuis [`STAND_DOZE_AFTER`],
//! elle s'endort debout : là, la musique ne change rien (elle ne la guette que pour se balancer
//! assise, et l'app ne la lui dit que posée). Elle se rassoit d'elle-même après [`UP_AWAKE`]
//! allongée éveillée, ou [`UP_ASLEEP`] de sommeil (elle se réveille alors, puis se rassoit, dans le
//! même pas), et jamais plus de [`LIE_LONGEST`] après s'être allongée ; debout, elle se réveille
//! après [`UP_ASLEEP`]. Réveillée par quelqu'un, elle reste allongée, et se relève [`UP_AWAKE`]
//! plus tard au plus. Rassise d'elle-même (ou parce que la place n'y est plus), ou réveillée
//! debout, elle reste là une attente ordinaire ([`SEATED`], [`STANDING`]) avant de reprendre sa
//! vie. Son repos allongée arrêté sans qu'on s'occupe d'elle (plus la place, plus posée) se dit
//! comme tel ([`Event::Unsettled`]), jamais comme un réveil.
//! Tant qu'elle se repose, elle ne change pas de place d'elle-même ; « Changer de place » la
//! réveille et la fait partir. Elle ne commence à se reposer que si elle vit sa vie (la case
//! cochée, jamais pendant une répétition) ; elle finit toujours. Un hasard à part ([`NAP_SEED`]),
//! de la même graine. Ses planches de repos sont chargées à la demande : tant qu'elles ne sont pas
//! prêtes, elle attend comme elle est, et redit son envie à chaque pas.
//!
//! Les distances sont en fractions de sa hauteur debout, comme dans `body.rs` et `perch.rs`.

use crate::body::{Capri, Stance};
use crate::geom::{Rect, Vec2};
use crate::look::{Look, Rest, Unsettled};
use crate::perch::{self, Bench, Perch, Reach, Seen, Stature};
use crate::sheets::life;

/// Une plage de durées, en secondes : un moment y est tiré au sort, uniformément.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub min: f64,
    pub max: f64,
}

impl Span {
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Un moment de la plage.
    pub fn pick(self, rng: &mut Rng) -> f64 {
        self.min + (self.max - self.min) * rng.unit()
    }
}

/// Le premier départ après son lancement (ou la case « Vivre sa vie » recochée) : assez tôt pour
/// qu'on la voie vivre peu après l'avoir lancée.
pub const FIRST: Span = Span::new(20.0, 40.0);
/// Assise, arrivée d'elle-même (ou revenue, assise, d'être cachée) : de 3 à 8 minutes.
pub const SEATED: Span = Span::new(180.0, 480.0);
/// Debout, arrivée d'elle-même : de 30 secondes à 2 minutes avant de chercher un bord, puisqu'elle
/// préfère être assise. Sans bord, elle ne change de place debout qu'après [`SEATED`].
pub const STANDING: Span = Span::new(30.0, 120.0);
/// Posée ou lâchée par l'utilisateur, qui l'a voulue là, ou descendue de son bord par un geste de
/// sa part (sa taille, « S'asseoir sur les fenêtres » décochée) : 5 minutes.
pub const BY_HAND: Span = Span::new(300.0, 300.0);
/// Son bord est parti sous elle (sa fenêtre fermée, recouverte, déplacée hors de portée), sans geste
/// de l'utilisateur : elle se choisit vite un nouvel endroit.
pub const LOST: Span = Span::new(5.0, 20.0);
/// Rien de possible (aucune autre place, ou la liste illisible) : un nouvel essai plus tard.
/// (Debout, sans bord, et pas encore le temps de changer de place debout : [`STANDING`].)
pub const NOWHERE: Span = Span::new(30.0, 30.0);
/// Ce qui la retenait vient de cesser (on lui parlait, la musique, le curseur sur elle) : elle
/// attend encore au moins ça avant de partir d'elle-même. Pas de départ juste après une réponse.
pub const PATIENCE: f64 = 10.0;
/// Assise, un éclat de temps en temps : de 1 à 4 minutes après le précédent, ou après qu'elle s'est
/// assise.
pub const FIDGET: Span = Span::new(60.0, 240.0);
/// Elle ne marche pas plus longtemps que ça (en secondes) : plus loin, elle s'efface et réapparaît
/// (à sa taille d'origine sur un écran Retina, 143,5 px par seconde : 1 435 px au plus).
pub const WALK_LONGEST: f64 = 10.0;
/// Le hasard de ses éclats : sa graine, mêlée à ce nombre (« fidget »), pour ne rien changer au
/// hasard de ses départs.
const FIDGET_SEED: u64 = 0x6669_6467_6574;
/// Tranquille depuis tant, assise, elle s'allonge : de 6 à 12 minutes.
pub const LIE_AFTER: Span = Span::new(360.0, 720.0);
/// Allongée et tranquille depuis tant, elle s'endort : de 3 à 6 minutes.
pub const DOZE_AFTER: Span = Span::new(180.0, 360.0);
/// Allongée éveillée depuis tant (depuis qu'elle s'est allongée, ou qu'on l'a réveillée), elle se
/// rassoit : de 10 à 20 minutes.
pub const UP_AWAKE: Span = Span::new(600.0, 1200.0);
/// Endormie depuis tant, elle se réveille (allongée, et se rassoit) : de 20 à 40 minutes.
pub const UP_ASLEEP: Span = Span::new(1200.0, 2400.0);
/// Debout, immobile et tranquille depuis tant, elle s'endort : de 10 à 20 minutes.
pub const STAND_DOZE_AFTER: Span = Span::new(600.0, 1200.0);
/// Jamais allongée (endormie comprise) plus longtemps d'affilée : une heure, réveillée par
/// quelqu'un ou non.
pub const LIE_LONGEST: f64 = 3600.0;
/// Le hasard de son repos : sa graine, mêlée à ce nombre (« repos »), pour ne rien changer au
/// hasard de ses départs ni de ses éclats.
pub const NAP_SEED: u64 = 0x0072_6570_6f73;

/// Elle n'apparaît pas sous la main de l'utilisateur : le curseur reste à plus de cette marge
/// (0,5 de sa hauteur debout) de la place qu'elle occupera.
pub const AVOID: f32 = 0.5;
/// Sur le même bord ou de nouveau debout, elle va au moins aussi loin (0,75 de sa hauteur).
pub const HOP: f32 = 0.75;
/// Un autre morceau de sa propre fenêtre pèse ça face à un morceau d'une autre fenêtre.
pub const SAME_WINDOW: f32 = 0.3;
/// Debout en bas de l'écran : le bas de son image à cette hauteur au-dessus du bas de l'écran (0,3
/// de sa hauteur, 90 points à sa taille d'origine), comme au lancement, au-dessus du Dock ou de la
/// barre des tâches.
pub const FLOOR: f32 = 0.3;

/// Un générateur pseudo-aléatoire à graine : SplitMix64 (Steele, Lea et Flood, « Fast Splittable
/// Pseudorandom Number Generators », 2014), dans la forme qu'en donne `splitmix64.c` de Sebastiano
/// Vigna. Rien de cryptographique : de quoi varier sa vie, et la rejouer à l'identique dans les
/// essais. Toute graine convient, 0 compris.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Le nombre suivant, sur 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Un nombre de [0, 1), à 53 bits.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Un indice tiré au sort d'après les poids (un poids négatif, ou qui n'est pas un nombre,
    /// compte pour 0) ; `None` s'ils sont tous nuls.
    pub fn pick(&mut self, weights: &[f32]) -> Option<usize> {
        let weight = |w: f32| f64::from(w.max(0.0));
        let total: f64 = weights.iter().map(|&w| weight(w)).sum();
        if total <= 0.0 || !total.is_finite() {
            return None;
        }
        let mut target = self.unit() * total;
        for (index, &w) in weights.iter().enumerate() {
            if target < weight(w) {
                return Some(index);
            }
            target -= weight(w);
        }
        // l'arrondi : le dernier poids qui compte
        weights.iter().rposition(|&w| weight(w) > 0.0)
    }
}

/// Pourquoi elle reste là un moment : la plage où est tiré son prochain départ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stay {
    /// Lancée, ou la case « Vivre sa vie » recochée : [`FIRST`].
    Start,
    /// Arrivée assise d'elle-même, ou revenue assise d'être cachée : [`SEATED`].
    Seated,
    /// De même, debout : [`STANDING`].
    Standing,
    /// Posée ou lâchée par l'utilisateur, ou descendue de son bord par un de ses gestes : [`BY_HAND`].
    ByHand,
    /// Son bord est parti sous elle : [`LOST`].
    Lost,
    /// Nulle part où aller : [`NOWHERE`].
    Nowhere,
}

impl Stay {
    pub fn span(self) -> Span {
        match self {
            Stay::Start => FIRST,
            Stay::Seated => SEATED,
            Stay::Standing => STANDING,
            Stay::ByHand => BY_HAND,
            Stay::Lost => LOST,
            Stay::Nowhere => NOWHERE,
        }
    }
}

/// Quand elle repart : l'heure de son prochain départ, d'après une horloge passée en paramètre
/// (des secondes), et le générateur qui tire ses durées et ses places.
#[derive(Clone, Debug)]
pub struct Roam {
    rng: Rng,
    next: f64,
}

impl Roam {
    /// Lancée à l'instant `now` : son premier départ dans [`FIRST`].
    pub fn new(seed: u64, now: f64) -> Self {
        let mut roam = Self { rng: Rng::new(seed), next: now };
        roam.stay(now, Stay::Start);
        roam
    }

    /// Elle reste là un moment, à partir de l'instant `now`, pour la raison `why`.
    pub fn stay(&mut self, now: f64, why: Stay) {
        self.next = now + why.span().pick(&mut self.rng);
    }

    /// L'heure de son prochain départ.
    pub fn next(&self) -> f64 {
        self.next
    }

    /// Est-il temps de partir, à l'instant `now` ? Tant que quelque chose la retient (`free`
    /// faux), son départ recule : jamais moins de [`PATIENCE`] après. Un instant qui n'est pas un
    /// nombre ne la fait jamais partir.
    pub fn due(&mut self, now: f64, free: bool) -> bool {
        if !free {
            self.next = self.next.max(now + PATIENCE);
            return false;
        }
        now >= self.next
    }
}

/// D'où elle part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin {
    /// Posée sur la fenêtre n° `window`, le point posé en `at`.
    Seated { window: u64, at: Vec2 },
    /// Debout, ses pieds en `feet`.
    Standing { feet: Vec2 },
}

/// Ce qu'il faut pour choisir où aller.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// Les bords où elle tiendrait (`perch::benches`) ; aucun si elle ne s'assoit pas sur les
    /// fenêtres, ou si leur liste est illisible.
    pub benches: &'a [Bench],
    /// L'overlay, en ses pixels.
    pub screen: Rect,
    /// Ce qu'il lui faut sur un bord (`Look::stature`) ; sa hauteur debout mesure les distances.
    pub seated: Stature,
    /// Debout : la place de son image autour de ses pieds (`Stature::standing`).
    pub standing: Stature,
    /// Le curseur, en pixels de l'overlay, s'il est connu.
    pub cursor: Option<Vec2>,
    pub from: Origin,
    /// On le lui demande (« Changer de place ») : assise sans autre bord, elle va debout en bas de
    /// l'écran plutôt que rester.
    pub asked: bool,
}

/// Où elle va.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    /// S'asseoir sur un bord, à cette place.
    Ledge(Perch),
    /// Se tenir debout, ses pieds là, en bas de l'écran.
    Floor(Vec2),
    /// Nulle part ailleurs : elle reste où elle est.
    Stay,
}

/// Un intervalle où peut aller le point posé, et son poids au tirage.
#[derive(Clone, Copy, Debug)]
struct Room {
    bench: Option<Bench>,
    y: f32,
    lo: f32,
    hi: f32,
    weight: f32,
}

/// Où aller ensuite (voir le module) : un bord, sinon debout en bas de l'écran, sinon rester.
pub fn choose(rng: &mut Rng, options: &Options) -> Choice {
    let Options { benches, seated, cursor, from, .. } = *options;
    let height = seated.height;
    let own = match from {
        Origin::Seated { window, at } => Some((window, at)),
        Origin::Standing { .. } => None,
    };
    // le morceau de bord où elle est assise
    let is_own = |bench: &Bench| {
        own.is_some_and(|(window, at)| {
            bench.window.id == window && bench.lo - 1.0 <= at.x && at.x <= bench.hi + 1.0
        })
    };
    let rooms = |own_piece: bool| -> Vec<Room> {
        let mut rooms = Vec::new();
        for bench in benches.iter().filter(|bench| is_own(bench) == own_piece) {
            let mut pieces = vec![(bench.lo, bench.hi)];
            if let Some((_, at)) = own.filter(|_| own_piece) {
                pieces = without(pieces, at.x - HOP * height, at.x + HOP * height);
            }
            let same_window = own.is_some_and(|(window, _)| window == bench.window.id);
            let factor = if same_window { SAME_WINDOW } else { 1.0 };
            let y = bench.y();
            for (lo, hi) in away_from(pieces, y, seated.reach, cursor, AVOID * height) {
                rooms.push(Room { bench: Some(*bench), y, lo, hi, weight: factor * (hi - lo + height) });
            }
        }
        rooms
    };
    // jamais deux fois de suite le même bord quand un autre existe ; sinon plus loin sur le sien
    let mut found = rooms(false);
    if found.is_empty() && own.is_some() {
        found = rooms(true);
    }
    if let Some((room, x)) = draw(rng, &found)
        && let Some(bench) = room.bench
    {
        return Choice::Ledge(bench.at(x));
    }
    // assise, elle préfère y rester que se tenir debout, sauf si on lui demande de changer de place
    if own.is_some() && !options.asked {
        return Choice::Stay;
    }
    floor(rng, options)
}

/// Debout en bas de l'écran ([`FLOOR`]), ailleurs que là où elle est et loin du curseur.
fn floor(rng: &mut Rng, options: &Options) -> Choice {
    let Options { screen, standing, cursor, from, .. } = *options;
    let (height, reach) = (standing.height, standing.reach);
    let y = screen.max().y - FLOOR * height - reach.down;
    let (lo, hi) = (screen.min.x + reach.left, screen.max().x - reach.right);
    if !(y - reach.up >= screen.min.y && lo <= hi) {
        return Choice::Stay;
    }
    let mut pieces = vec![(lo, hi)];
    if let Origin::Standing { feet } = from {
        pieces = without(pieces, feet.x - HOP * height, feet.x + HOP * height);
    }
    let rooms: Vec<Room> = away_from(pieces, y, reach, cursor, AVOID * height)
        .into_iter()
        .map(|(lo, hi)| Room { bench: None, y, lo, hi, weight: hi - lo + height })
        .collect();
    match draw(rng, &rooms) {
        Some((room, x)) => Choice::Floor(Vec2::new(x, room.y)),
        None => Choice::Stay,
    }
}

/// Un intervalle tiré au sort d'après les poids, et un point de cet intervalle, uniformément.
fn draw(rng: &mut Rng, rooms: &[Room]) -> Option<(Room, f32)> {
    let weights: Vec<f32> = rooms.iter().map(|room| room.weight).collect();
    let room = rooms[rng.pick(&weights)?];
    let x = room.lo + (room.hi - room.lo) * rng.unit() as f32;
    Some((room, x.max(room.lo).min(room.hi)))
}

/// Les morceaux de `pieces` où son corps (la place `reach` autour du point posé, à la hauteur
/// `y`) reste à plus de `margin` du curseur.
fn away_from(
    pieces: Vec<(f32, f32)>,
    y: f32,
    reach: Reach,
    cursor: Option<Vec2>,
    margin: f32,
) -> Vec<(f32, f32)> {
    let Some(cursor) = cursor else { return pieces };
    let level = y - reach.up - margin <= cursor.y && cursor.y <= y + reach.down + margin;
    if !level {
        return pieces;
    }
    without(pieces, cursor.x - reach.right - margin, cursor.x + reach.left + margin)
}

/// `pieces` privés de l'intervalle ouvert `(a, b)` (`a` < `b`) : chacun donne zéro, un ou deux
/// morceaux, jamais vides (`lo` ≤ `hi`). Des bornes qui ne sont pas des nombres ne laissent rien.
fn without(pieces: Vec<(f32, f32)>, a: f32, b: f32) -> Vec<(f32, f32)> {
    pieces
        .into_iter()
        .flat_map(|(lo, hi)| {
            let left = (lo <= a).then(|| (lo, hi.min(a)));
            let right = (b <= hi).then(|| (lo.max(b), hi));
            left.into_iter().chain(right)
        })
        .collect()
}

/// Ce qui la retient de partir et que seule l'app sait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Busy {
    /// On lui parle au micro, elle parle ou répond, ou le champ de saisie est ouvert.
    pub talking: bool,
    /// Le curseur est sur elle : on va peut-être l'attraper. Ne retient que ses départs
    /// d'elle-même.
    pub hovered: bool,
}

/// Pourquoi elle ne change pas de place maintenant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// On la tient, ou elle vole.
    Carried,
    /// Elle apparaît, s'efface, est en chemin, ou cachée.
    Away,
    /// On lui parle, elle parle ou répond, ou le champ de saisie est ouvert.
    Talking,
    /// Elle se balance sur la musique.
    Music,
    /// Elle marche vers sa nouvelle place.
    Walking,
}

impl Hold {
    /// Pour le journal.
    pub fn describe(self) -> &'static str {
        match self {
            Hold::Carried => "on la tient, ou elle vole",
            Hold::Away => "elle apparaît, s'efface ou est cachée",
            Hold::Talking => "on se parle",
            Hold::Music => "elle se balance sur la musique",
            Hold::Walking => "elle marche",
        }
    }
}

/// Ce que l'app lui donne, au moment où elle en a besoin : la liste des fenêtres n'est lue que là.
pub trait World {
    /// L'overlay, en ses pixels.
    fn screen(&self) -> Rect;
    /// Le curseur, en pixels de l'overlay, s'il est connu.
    fn cursor(&self) -> Option<Vec2>;
    /// Lui est-il permis de s'asseoir sur les fenêtres (la case de son menu) ?
    fn perch_allowed(&self) -> bool;
    /// Les fenêtres qui comptent (`perch::sift`, `stature` : ce qu'il lui faut sur un bord), lues
    /// maintenant ; `None` si le système ne les donne pas.
    fn seen(&mut self, stature: Stature) -> Option<Vec<Seen>>;
    /// Elle vient d'être posée sur un bord pour y apparaître : sa fenêtre relue tout de suite,
    /// comme une lecture de sa veille posée ; elle la suit (`Capri::ride`), ou la quitte
    /// (`Capri::take_off`) si elle est partie, si son bord ne lui laisse plus la place, ou si elle
    /// ne s'assoit plus sur les fenêtres.
    fn recheck(&mut self, capri: &mut Capri, stature: Stature);
}

/// Ce qui vient de se passer, pour le journal (des décisions et des nombres) et pour ce qu'elle en
/// dit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Elle part vers `to` (un bord, ou debout en bas de l'écran ; jamais `Choice::Stay`) ;
    /// `benches` : les morceaux de bord où elle tiendrait ; `asked` : « Changer de place » ;
    /// `walking` : elle y va en marchant (sinon elle s'efface).
    Leaves { to: Choice, benches: usize, asked: bool, walking: bool },
    /// Aucune autre place : elle reste.
    Stays { benches: usize, asked: bool },
    /// Arrivée, assise ou debout ; `lost` : debout parce que le bord où elle allait n'y était plus.
    Arrived { seated: bool, lost: bool },
    /// On lui parle pendant qu'elle s'efface pour partir : elle revient sur ses pas, là où elle
    /// était, et n'ira pas plus loin.
    TurnedBack,
    /// On lui parle pendant qu'elle marche : elle s'arrête là, et n'ira pas plus loin.
    Halted,
    /// Assise, un éclat.
    Fidgeted,
    /// « Changer de place » demandé, et quelque chose l'en empêche.
    Refused(Hold),
    /// Assise et tranquille depuis un moment, elle s'allonge sur son bord.
    LiesDown,
    /// Tranquille, elle s'endort, allongée ou debout.
    FallsAsleep { lying: bool },
    /// Elle se réveille : d'elle-même (`woken` faux), ou parce qu'on s'est occupé d'elle.
    WakesUp { lying: bool, woken: bool },
    /// Allongée depuis un moment, elle se rassoit d'elle-même (`asleep` : elle dormait).
    SitsUp { asleep: bool },
    /// Son repos allongée s'arrête sans qu'on s'occupe d'elle (`why` ; `asleep` : elle dormait).
    Unsettled { why: Unsettled, asleep: bool },
}

/// Elle vit sa vie : à chaque pas de la boucle, partir, ou apparaître là où elle allait. Le seul
/// endroit qui fait changer de place `Capri` et son dessin ensemble, dans l'ordre de la boucle.
#[derive(Clone, Debug)]
pub struct Roamer {
    roam: Roam,
    /// Elle change de place d'elle-même (la case « Vivre sa vie »).
    enabled: bool,
    /// « Changer de place » demandé, pas encore servi.
    asked: bool,
    /// Où elle va, pendant qu'elle s'efface pour y aller.
    trip: Option<Choice>,
    /// D'elle-même, elle ne change pas de place debout avant cette heure ([`SEATED`] après sa
    /// dernière arrivée debout en bas de l'écran) : sans bord, elle chercherait sinon une autre
    /// place debout toutes les 30 s à 2 min, toute la journée.
    floor_next: f64,
    /// Au pas précédent : on la tenait ou elle volait ; elle était posée.
    was_carried: bool,
    was_perched: bool,
    /// Où elle va en marchant, pendant qu'elle marche.
    walk: Option<Vec2>,
    /// Ses éclats, assise.
    fidget: Fidget,
    /// Son repos.
    nap: Nap,
}

/// Quand elle se repose (voir le module) : un hasard à part ([`NAP_SEED`]), depuis quand elle est
/// tranquille, assise, allongée ou endormie debout, et son repos au pas précédent.
#[derive(Clone, Debug)]
struct Nap {
    rng: Rng,
    /// Tranquille depuis un moment : quand elle s'allongera (assise), quand elle s'endormira
    /// (debout). `None` : on vient de s'occuper d'elle.
    quiet: Option<Quiet>,
    /// Assise (là, posée, dessinée sur son bord, éveillée) depuis cette heure.
    seated_since: Option<f64>,
    /// Allongée : quand elle s'endort (une fois tranquille), quand elle se rassoit, et jamais
    /// après `cap`.
    lain: Option<Lain>,
    /// Endormie debout : quand elle se réveille.
    dozing_until: Option<f64>,
    /// Son repos au pas précédent.
    last: Rest,
}

#[derive(Clone, Copy, Debug)]
struct Quiet {
    lie: f64,
    doze: f64,
}

#[derive(Clone, Copy, Debug)]
struct Lain {
    sleep_at: Option<f64>,
    up_at: f64,
    cap: f64,
}

impl Nap {
    /// Tranquille à partir de l'instant `now` : ses seuils tirés.
    fn quiet_from(&mut self, now: f64) {
        let lie = now + LIE_AFTER.pick(&mut self.rng);
        let doze = now + STAND_DOZE_AFTER.pick(&mut self.rng);
        self.quiet = Some(Quiet { lie, doze });
    }
}

/// Quand elle a un éclat, assise : un hasard à part ([`FIDGET_SEED`]), l'heure du prochain, et si
/// elle était assise au pas précédent (s'asseoir remet l'attente à zéro).
#[derive(Clone, Debug)]
struct Fidget {
    rng: Rng,
    next: f64,
    seated: bool,
}

impl Roamer {
    /// Lancée à l'instant `now` (secondes de l'horloge de l'appelant). `enabled` : elle change de
    /// place d'elle-même ; sinon seulement sur un geste.
    pub fn new(seed: u64, now: f64, enabled: bool) -> Self {
        Self {
            roam: Roam::new(seed, now),
            enabled,
            asked: false,
            trip: None,
            floor_next: now,
            was_carried: false,
            was_perched: false,
            walk: None,
            fidget: Fidget { rng: Rng::new(seed ^ FIDGET_SEED), next: now, seated: false },
            nap: Nap {
                rng: Rng::new(seed ^ NAP_SEED),
                quiet: None,
                seated_since: None,
                lain: None,
                dozing_until: None,
                last: Rest::Awake,
            },
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Le lui permettre, ou le lui interdire (une place déjà en chemin est atteinte). Permise de
    /// nouveau, elle repart bientôt ([`FIRST`]).
    pub fn set_enabled(&mut self, enabled: bool, now: f64) {
        if enabled && !self.enabled {
            self.roam.stay(now, Stay::Start);
        }
        self.enabled = enabled;
    }

    /// « Changer de place » : au prochain pas, elle part si rien ne l'en empêche (endormie ou
    /// allongée, elle se réveille et part). On s'occupe d'elle : elle n'est plus tranquille.
    pub fn ask(&mut self) {
        self.asked = true;
        self.nap.quiet = None;
    }

    /// On vient de la montrer, à l'instant `now` : elle reste un moment là où elle réapparaît,
    /// comme arrivée d'elle-même, et jamais moins que ce qu'elle attendait déjà (posée à la main
    /// juste avant d'être cachée, elle garde ses 5 minutes). Un « Changer de place » demandé
    /// pendant qu'elle était cachée est oublié.
    pub fn came_back(&mut self, now: f64, capri: &Capri) {
        self.asked = false;
        // on vient de la montrer : elle n'est plus tranquille
        self.nap.quiet = None;
        // cachée en marchant : sa marche s'est arrêtée là, ce n'est pas une arrivée
        self.walk = None;
        let why = if capri.perched().is_some() { Stay::Seated } else { Stay::Standing };
        // revenir ne raccourcit jamais une attente en cours (posée à la main, par exemple)
        let waiting = self.roam.next;
        self.roam.stay(now, why);
        self.roam.next = self.roam.next.max(waiting);
    }

    /// L'heure de son prochain départ d'elle-même.
    pub fn next(&self) -> f64 {
        self.roam.next()
    }

    /// Où elle va, pendant qu'elle s'efface pour y aller.
    pub fn heading(&self) -> Option<Choice> {
        self.trip
    }

    /// Où elle va, pendant qu'elle y marche.
    pub fn walking_to(&self) -> Option<Vec2> {
        self.walk
    }

    /// Un pas, à l'instant `now` et à l'image d'animation `frame` : après `Look::update`, dans
    /// l'ordre de la boucle. Effacée pour changer de place, elle est déplacée et apparaît là-bas ;
    /// en marche, elle arrive, ou s'arrête si on lui parle ; sinon, s'il est temps (ou qu'on le lui
    /// demande) et que rien ne la retient, elle choisit où aller (la seule lecture des fenêtres
    /// qu'elle demande, [`World::seen`]) et commence à s'effacer, ou à marcher. Si rien de cela,
    /// assise, peut-être un éclat.
    pub fn step(
        &mut self,
        look: &mut Look,
        capri: &mut Capri,
        world: &mut dyn World,
        busy: Busy,
        now: f64,
        frame: u64,
    ) -> Vec<Event> {
        let mut events = self.decide(look, capri, world, busy, now, frame);
        if events.is_empty() && self.fidgets(look, capri, busy, now, frame) {
            events.push(Event::Fidgeted);
        }
        // son repos est suivi à chaque pas ; il ne commence ni ne finit que si rien d'autre ne
        // s'est passé à ce pas
        let quiet = events.is_empty();
        events.extend(self.rests(look, capri, busy, now, frame, quiet));
        events
    }

    /// Son repos, à ce pas (voir le module) : depuis quand elle est tranquille, et, si `act` (rien
    /// d'autre ne s'est passé à ce pas), s'allonger, s'endormir, se rasseoir, se réveiller.
    fn rests(
        &mut self,
        look: &mut Look,
        capri: &mut Capri,
        busy: Busy,
        now: f64,
        frame: u64,
        act: bool,
    ) -> Option<Event> {
        // un instant qui n'est pas un nombre ne fait rien
        if now.is_nan() {
            return None;
        }
        let carried = matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. });
        // ce qui compte comme s'occuper d'elle : ses propres trajets et ses éclats, non ; la musique
        // n'est dite que posée (debout, elle ne change rien)
        let attended =
            look.take_attention() || busy.talking || carried || look.hears_music() || look.is_grooving();
        let unsettled = look.take_unsettled();
        if attended {
            self.nap.quiet = None;
        } else if self.nap.quiet.is_none() {
            self.nap.quiet_from(now);
        }
        let rest = look.rest();
        let was = std::mem::replace(&mut self.nap.last, rest);
        let seated =
            rest == Rest::Awake && look.is_here() && capri.perched().is_some() && look.is_seated(capri);
        if !seated {
            self.nap.seated_since = None;
        } else if self.nap.seated_since.is_none() {
            self.nap.seated_since = Some(now);
        }
        // réveillée par quelqu'un (quand c'est sa vie qui la réveille, elle le retient tout de suite ;
        // la place partie, ou plus posée, ce n'est pas un réveil : `Look::take_unsettled`)
        let woken =
            unsettled.is_none() && matches!(was, Rest::Asleep { .. }) && !matches!(rest, Rest::Asleep { .. });
        match rest {
            Rest::Lying | Rest::Asleep { lying: true } => {
                self.nap.dozing_until = None;
                let asleep = rest != Rest::Lying;
                let mut lain = match self.nap.lain {
                    Some(lain) => lain,
                    None => {
                        let up_at = now + UP_AWAKE.pick(&mut self.nap.rng);
                        Lain { sleep_at: None, up_at, cap: now + LIE_LONGEST }
                    }
                };
                let mut event = None;
                if woken {
                    lain.up_at = (now + UP_AWAKE.pick(&mut self.nap.rng)).min(lain.cap);
                    lain.sleep_at = None;
                    event = Some(Event::WakesUp { lying: true, woken: true });
                }
                // le moment de se rasseoir (endormie, elle se réveille d'abord)
                if act && event.is_none() && now >= lain.up_at.min(lain.cap) {
                    if asleep {
                        look.rouse(capri, frame);
                    }
                    if look.sit_up(capri, frame) {
                        self.nap.lain = None;
                        self.nap.last = look.rest();
                        self.nap.quiet_from(now);
                        // rassise, elle reste là une attente ordinaire avant de reprendre sa vie
                        self.roam.stay(now, Stay::Seated);
                        return Some(Event::SitsUp { asleep });
                    }
                }
                if !asleep && look.rest() == Rest::Lying {
                    if attended {
                        lain.sleep_at = None;
                    } else if lain.sleep_at.is_none() {
                        lain.sleep_at = Some(now + DOZE_AFTER.pick(&mut self.nap.rng));
                    }
                    let due = lain.sleep_at.is_some_and(|at| now >= at);
                    if act && event.is_none() && self.enabled && due && look.doze(capri, frame) {
                        lain.up_at = (now + UP_ASLEEP.pick(&mut self.nap.rng)).min(lain.cap);
                        self.nap.last = look.rest();
                        event = Some(Event::FallsAsleep { lying: true });
                    }
                }
                self.nap.lain = Some(lain);
                event
            }
            Rest::Asleep { lying: false } => {
                self.nap.lain = None;
                let until = match self.nap.dozing_until {
                    Some(until) => until,
                    None => now + UP_ASLEEP.pick(&mut self.nap.rng),
                };
                self.nap.dozing_until = Some(until);
                if act && now >= until && look.rouse(capri, frame) {
                    self.nap.dozing_until = None;
                    self.nap.last = look.rest();
                    self.nap.quiet_from(now);
                    return Some(Event::WakesUp { lying: false, woken: false });
                }
                None
            }
            Rest::Waking => {
                self.nap.lain = None;
                self.nap.dozing_until = None;
                woken.then_some(Event::WakesUp { lying: false, woken: true })
            }
            Rest::Awake => {
                self.nap.lain = None;
                self.nap.dozing_until = None;
                // son repos allongée s'est arrêté de lui-même : sans la place, elle s'est rassise, et
                // reste là une attente ordinaire ; plus posée, `decide` a compté un bord perdu
                if let Some((why, asleep)) = unsettled {
                    if why != Unsettled::LedgeGone {
                        self.roam.stay(now, Stay::Seated);
                    }
                    return Some(Event::Unsettled { why, asleep });
                }
                // réveillée debout (d'elle-même ou non), là : une attente ordinaire, debout ; tenue,
                // en chemin, ce qui suit en décide
                let here = look.is_here() && !look.is_traveling() && self.trip.is_none();
                if matches!(was, Rest::Asleep { lying: false } | Rest::Waking) && !carried && here {
                    self.roam.stay(now, Stay::Standing);
                }
                if woken {
                    return Some(Event::WakesUp { lying: was == Rest::Asleep { lying: true }, woken: true });
                }
                if !act || !self.enabled || self.trip.is_some() || self.walk.is_some() {
                    return None;
                }
                let quiet = self.nap.quiet?;
                if !look.is_here() || look.gesture().is_some() {
                    return None;
                }
                // assise depuis un moment, tranquille : elle s'allonge (ses planches pas encore
                // là, elle en a envie, et attend assise)
                if self.nap.seated_since.is_some_and(|since| now >= since + PATIENCE) && now >= quiet.lie {
                    if look.lie_down(capri, frame) {
                        self.nap.last = look.rest();
                        return Some(Event::LiesDown);
                    }
                    return None;
                }
                // debout, immobile et tranquille : elle s'endort debout
                if !seated
                    && capri.stance() == Stance::Floating
                    && now >= quiet.doze
                    && look.doze(capri, frame)
                {
                    self.nap.dozing_until = Some(now + UP_ASLEEP.pick(&mut self.nap.rng));
                    self.nap.last = look.rest();
                    return Some(Event::FallsAsleep { lying: false });
                }
                None
            }
        }
    }

    /// Assise et libre, un éclat s'il est temps ; voir le module.
    fn fidgets(&mut self, look: &mut Look, capri: &mut Capri, busy: Busy, now: f64, frame: u64) -> bool {
        let seated = self.enabled && look.is_seated(capri) && capri.perched().is_some() && !look.is_away();
        if !seated {
            self.fidget.seated = false;
            return false;
        }
        if !std::mem::replace(&mut self.fidget.seated, true) {
            // elle vient de s'asseoir (ou de se remettre à vivre sa vie) : l'attente commence
            self.fidget.next = now + FIDGET.pick(&mut self.fidget.rng);
            return false;
        }
        let held = !look.is_here() || busy.talking || look.is_grooving() || look.gesture().is_some();
        if held || self.trip.is_some() {
            self.fidget.next = self.fidget.next.max(now + PATIENCE);
            return false;
        }
        // un instant qui n'est pas un nombre ne fait rien
        if now.is_nan() || now < self.fidget.next {
            return false;
        }
        if look.fidget(capri, frame) {
            self.fidget.next = now + FIDGET.pick(&mut self.fidget.rng);
            true
        } else {
            // la musique, ou sans la planche : plus tard
            self.fidget.next = now + PATIENCE;
            false
        }
    }

    /// Ce qui peut la faire changer de place à ce pas (voir [`Roamer::step`]).
    fn decide(
        &mut self,
        look: &mut Look,
        capri: &mut Capri,
        world: &mut dyn World,
        busy: Busy,
        now: f64,
        frame: u64,
    ) -> Vec<Event> {
        // en marche : elle arrive, ou s'arrête si on lui parle ; « Changer de place » attend
        if self.walk.is_some() {
            if look.is_walking(capri) {
                if busy.talking {
                    look.halt(capri);
                    self.walk = None;
                    self.roam.stay(now, Stay::Standing);
                    return vec![Event::Halted];
                }
                if std::mem::take(&mut self.asked) {
                    return vec![Event::Refused(Hold::Walking)];
                }
                return Vec::new();
            }
            self.walk = None;
            // arrivée, ou arrêtée par le bord de l'écran ou une nouvelle taille : c'est une arrivée
            // debout, et elle s'y repose ; attrapée, ou cachée, en chemin : la suite, comme d'habitude
            let carried = matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. });
            if !carried && look.is_here() {
                return vec![self.walked(now)];
            }
        }
        // on lui parle pendant qu'elle s'efface pour partir (le champ de saisie, ou le micro, s'est
        // ouvert là où elle est) : elle revient sur ses pas, et n'ira pas plus loin
        if look.is_traveling() && busy.talking {
            look.turn_back(capri, frame);
            self.trip = None;
            let why = if capri.perched().is_some() { Stay::Seated } else { Stay::Standing };
            self.roam.stay(now, why);
            return vec![Event::TurnedBack];
        }
        if look.is_between() {
            return vec![self.arrive(look, capri, world, now, frame)];
        }
        // cachée en chemin : elle n'ira pas plus loin
        if !look.is_traveling() {
            self.trip = None;
        }
        let carried = matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. });
        let perched = capri.perched().is_some();
        let was_carried = std::mem::replace(&mut self.was_carried, carried);
        let was_perched = std::mem::replace(&mut self.was_perched, perched);
        if was_carried && !carried {
            self.roam.stay(now, Stay::ByHand);
        } else if was_perched && !perched && !carried && !look.is_traveling() {
            self.roam.stay(now, Stay::Lost);
        }
        let hold = if carried {
            Some(Hold::Carried)
        } else if !look.is_here() {
            Some(Hold::Away)
        } else if busy.talking {
            Some(Hold::Talking)
        } else if look.is_grooving() {
            Some(Hold::Music)
        } else {
            None
        };
        // un geste en cours retient ses départs le temps qu'il finisse (quelques secondes au plus),
        // sans reculer le prochain ni oublier une demande : elle part juste après
        if hold.is_none() && look.gesture().is_some() {
            return Vec::new();
        }
        if std::mem::take(&mut self.asked) {
            return match hold {
                Some(hold) => vec![Event::Refused(hold)],
                None => self.leave(look, capri, world, now, frame, true),
            };
        }
        if !self.enabled {
            return Vec::new();
        }
        // allongée, endormie, ou en train de se réveiller : elle ne part pas d'elle-même
        let free = hold.is_none() && !busy.hovered && !look.is_resting();
        if self.roam.due(now, free) { self.leave(look, capri, world, now, frame, false) } else { Vec::new() }
    }

    /// L'utilisateur vient d'agir sur elle à l'instant `now` (sa taille, la case « S'asseoir sur
    /// les fenêtres ») ; `capri` : où elle en est après.
    /// Si cela lui a fait quitter son bord, c'est un geste, pas un bord perdu : elle reste là
    /// comme posée à la main ([`BY_HAND`]). Seul un bord parti sous elle (sa fenêtre fermée,
    /// déplacée hors de portée, recouverte) vaut [`LOST`].
    pub fn touched(&mut self, now: f64, capri: &Capri) {
        self.nap.quiet = None;
        let perched = capri.perched().is_some();
        if std::mem::replace(&mut self.was_perched, perched) && !perched {
            self.roam.stay(now, Stay::ByHand);
        }
    }

    /// Elle choisit où aller et commence à s'effacer ; ou elle reste, faute d'autre place. Elle
    /// est là, et libre.
    fn leave(
        &mut self,
        look: &mut Look,
        capri: &mut Capri,
        world: &mut dyn World,
        now: f64,
        frame: u64,
        asked: bool,
    ) -> Vec<Event> {
        let stature = look.stature();
        let screen = world.screen();
        let benches = if world.perch_allowed() {
            world.seen(stature).map(|seen| perch::benches(&seen, screen, stature)).unwrap_or_default()
        } else {
            Vec::new()
        };
        let from = match capri.stance() {
            Stance::Perched { spot, seat } => Origin::Seated { window: spot.window, at: seat },
            _ => Origin::Standing { feet: capri.feet() },
        };
        let standing = Stature::standing(look.standing_size(), look.standing_feet(), stature.height);
        let options = Options {
            benches: &benches,
            screen,
            seated: stature,
            standing,
            cursor: world.cursor(),
            from,
            asked,
        };
        let choice = choose(&mut self.roam.rng, &options);
        let count = benches.len();
        // debout sans bord : d'elle-même, elle ne change de place debout que de loin en loin ; elle
        // revient chercher un bord à la cadence courte, sans lire la liste si elle ne s'assoit pas
        if matches!(choice, Choice::Floor(_)) && !asked && now < self.floor_next {
            self.roam.stay(now, Stay::Standing);
            if !world.perch_allowed() {
                self.roam.next = self.roam.next.max(self.floor_next);
            }
            return vec![Event::Stays { benches: count, asked }];
        }
        // debout en bas de l'écran, vers un autre endroit du bas, dans le sens où elle regarde, et
        // pas trop loin : elle y va en marchant
        if let Choice::Floor(to) = choice
            && self.walkable(look, capri, to)
            && look.walk(capri, frame, to.x)
        {
            self.walk = Some(to);
            return vec![Event::Leaves { to: choice, benches: count, asked, walking: true }];
        }
        if choice == Choice::Stay || !look.travel(capri, frame) {
            self.roam.stay(now, Stay::Nowhere);
            return vec![Event::Stays { benches: count, asked }];
        }
        self.trip = Some(choice);
        let leaves = Event::Leaves { to: choice, benches: count, asked, walking: false };
        // sans ses planches de vie, elle est déjà effacée : elle apparaît là-bas tout de suite
        if look.is_between() {
            return vec![leaves, self.arrive(look, capri, world, now, frame)];
        }
        vec![leaves]
    }

    /// Peut-elle aller en marchant jusqu'à `to` ? Debout en flottant, déjà en bas de l'écran (ses
    /// pieds à la hauteur de `to`, au pixel près), dans le sens où elle regarde, en
    /// [`WALK_LONGEST`] au plus, et sa planche de marche là.
    fn walkable(&self, look: &Look, capri: &Capri, to: Vec2) -> bool {
        let Some(speed) = look.walk_speed() else { return false };
        let feet = capri.feet();
        let dx = to.x - feet.x;
        capri.stance() == Stance::Floating
            && (to.y - feet.y).abs() <= 1.0
            && life::FACING.allows(dx)
            && f64::from(dx.abs() / speed) <= WALK_LONGEST
    }

    /// Arrivée en marchant : elle y reste un moment, debout, et ne change pas de place debout
    /// d'elle-même avant longtemps, comme arrivée en s'effaçant.
    fn walked(&mut self, now: f64) -> Event {
        self.floor_next = now + SEATED.pick(&mut self.roam.rng);
        self.roam.stay(now, Stay::Standing);
        self.was_carried = false;
        self.was_perched = false;
        Event::Arrived { seated: false, lost: false }
    }

    /// Effacée : déplacée là où elle allait (sa fenêtre relue, s'il s'agit d'un bord), elle y
    /// apparaît ; sans destination, là où elle est. Puis elle y reste un moment.
    fn arrive(
        &mut self,
        look: &mut Look,
        capri: &mut Capri,
        world: &mut dyn World,
        now: f64,
        frame: u64,
    ) -> Event {
        let to = self.trip.take();
        match to {
            Some(Choice::Ledge(perch)) => {
                capri.set_on(perch);
                world.recheck(capri, look.stature());
            }
            Some(Choice::Floor(feet)) => {
                capri.set_at(feet);
                self.floor_next = now + SEATED.pick(&mut self.roam.rng);
            }
            Some(Choice::Stay) | None => {}
        }
        look.arrive(capri, frame);
        let seated = capri.perched().is_some();
        let lost = matches!(to, Some(Choice::Ledge(_))) && !seated;
        let why = if lost {
            Stay::Lost
        } else if seated {
            Stay::Seated
        } else {
            Stay::Standing
        };
        self.roam.stay(now, why);
        self.was_carried = false;
        self.was_perched = seated;
        Event::Arrived { seated, lost }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{Moves, PerchMasks, Wardrobe};
    use crate::mask::AlphaMask;
    use crate::sheets::{self, life, stand};

    const SCREEN: Rect = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
    /// Debout, 300 points de haut sur un écran Retina.
    const HEIGHT: f32 = 600.0;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(Vec2::new(x, y), Vec2::new(w, h))
    }

    fn seen(id: u64, r: Rect) -> Seen {
        Seen { id, rect: r }
    }

    fn sitting() -> Stature {
        crate::look::seated_stature(HEIGHT)
    }

    fn upright() -> Stature {
        Stature::standing(stand::cell_size(HEIGHT), stand::feet_uv(), HEIGHT)
    }

    fn options<'a>(benches: &'a [Bench], cursor: Option<Vec2>, from: Origin) -> Options<'a> {
        Options {
            benches,
            screen: SCREEN,
            seated: sitting(),
            standing: upright(),
            cursor,
            from,
            asked: false,
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_life() {
        let draws = |seed| {
            let mut rng = Rng::new(seed);
            (0..64).map(|_| rng.next_u64()).collect::<Vec<_>>()
        };
        assert_eq!(draws(7), draws(7));
        assert_ne!(draws(7), draws(8));
        assert_ne!(draws(0)[0], draws(0)[1], "la graine 0 aussi");
        // [0, 1), et à peu près uniforme
        let mut rng = Rng::new(42);
        let units: Vec<f64> = (0..20_000).map(|_| rng.unit()).collect();
        assert!(units.iter().all(|u| (0.0..1.0).contains(u)));
        let mean = units.iter().sum::<f64>() / units.len() as f64;
        assert!((mean - 0.5).abs() < 0.01, "{mean}");
        let low = units.iter().filter(|&&u| u < 0.1).count();
        assert!((1800..2200).contains(&low), "{low}");
    }

    #[test]
    fn weights_decide_how_often() {
        let mut rng = Rng::new(3);
        let mut counts = [0usize; 4];
        for _ in 0..30_000 {
            counts[rng.pick(&[1.0, 3.0, 0.0, -2.0]).expect("un poids")] += 1;
        }
        assert_eq!((counts[2], counts[3]), (0, 0), "un poids nul ou négatif n'est jamais tiré");
        let ratio = counts[1] as f64 / counts[0] as f64;
        assert!((2.8..3.2).contains(&ratio), "{counts:?}");
        assert_eq!(rng.pick(&[]), None);
        assert_eq!(rng.pick(&[0.0, f32::NAN, -1.0]), None);
    }

    #[test]
    fn she_waits_a_drawn_while_and_never_leaves_while_held_back() {
        let mut roam = Roam::new(11, 100.0);
        assert!((100.0 + FIRST.min..=100.0 + FIRST.max).contains(&roam.next()));
        let next = roam.next();
        assert!(!roam.due(next - 0.1, true));
        // retenue au moment de partir : pas avant PATIENCE après qu'elle est libre
        assert!(!roam.due(next + 1.0, false));
        assert!(!roam.due(next + 1.0 + PATIENCE - 0.1, true));
        assert!(roam.due(next + 1.0 + PATIENCE, true));
        // retenue loin de son départ : rien ne change
        roam.stay(0.0, Stay::Seated);
        let later = roam.next();
        assert!((SEATED.min..=SEATED.max).contains(&later));
        assert!(!roam.due(1.0, false));
        assert_eq!(roam.next(), later);
        // chaque raison tire dans sa plage
        for why in [Stay::Start, Stay::Seated, Stay::Standing, Stay::ByHand, Stay::Lost, Stay::Nowhere] {
            for _ in 0..200 {
                roam.stay(50.0, why);
                let span = why.span();
                assert!((50.0 + span.min..=50.0 + span.max).contains(&roam.next()), "{why:?}");
            }
        }
        // un instant qui n'est pas un nombre ne la fait pas partir, ni ne la fige
        assert!(!roam.due(f64::NAN, true));
        assert!(!roam.due(f64::NAN, false));
        assert!(roam.due(1000.0, true));
    }

    /// Deux fenêtres : celle de devant (n° 2) à cheval sur le bord de celle de derrière (n° 1).
    fn desk() -> Vec<Seen> {
        vec![seen(2, rect(800.0, 900.0, 400.0, 400.0)), seen(1, rect(200.0, 1000.0, 1600.0, 800.0))]
    }

    #[test]
    fn standing_she_goes_to_sit_on_an_edge_and_varies() {
        let benches = perch::benches(&desk(), SCREEN, sitting());
        assert_eq!(benches.len(), 3);
        let mut rng = Rng::new(5);
        let mut windows = std::collections::HashMap::new();
        for _ in 0..3000 {
            let from = Origin::Standing { feet: Vec2::new(2500.0, 1700.0) };
            let Choice::Ledge(perch) = choose(&mut rng, &options(&benches, None, from)) else {
                panic!("debout, elle va s'asseoir")
            };
            let bench = benches
                .iter()
                .find(|bench| {
                    bench.window.id == perch.spot.window
                        && bench.lo <= perch.feet.x
                        && perch.feet.x <= bench.hi
                })
                .expect("sur un morceau où elle tient");
            assert_eq!(perch, bench.at(perch.feet.x));
            *windows.entry(perch.spot.window).or_insert(0usize) += 1;
        }
        // les trois morceaux comptent ; les deux de la fenêtre de derrière, plus longs, un peu plus
        assert!(windows[&2] > 500 && windows[&1] > windows[&2], "{windows:?}");
    }

    #[test]
    fn seated_she_never_takes_the_same_edge_twice_and_rather_another_window() {
        let benches = perch::benches(&desk(), SCREEN, sitting());
        let mut rng = Rng::new(9);
        // assise sur le premier morceau de la fenêtre de derrière
        let at = Vec2::new(500.0, 1000.0);
        let from = Origin::Seated { window: 1, at };
        let (mut front, mut other_piece) = (0usize, 0usize);
        for _ in 0..4000 {
            let Choice::Ledge(perch) = choose(&mut rng, &options(&benches, None, from)) else { panic!() };
            assert!(!(perch.spot.window == 1 && perch.feet.x <= 705.0), "jamais le même bord : {perch:?}");
            if perch.spot.window == 2 { front += 1 } else { other_piece += 1 }
        }
        // l'autre morceau de sa fenêtre pèse 0,3 fois ce qu'il pèserait sur une autre fenêtre
        let expected = (SAME_WINDOW * (410.0 + HEIGHT)) / (210.0 + HEIGHT);
        let ratio = other_piece as f32 / front as f32;
        assert!((ratio - expected).abs() < 0.08, "{ratio} contre {expected}");
    }

    #[test]
    fn on_her_only_edge_she_moves_along_it_or_stays_seated() {
        let alone = [seen(1, rect(200.0, 1000.0, 2400.0, 800.0))];
        let benches = perch::benches(&alone, SCREEN, sitting());
        assert_eq!(benches.len(), 1);
        let mut rng = Rng::new(1);
        let at = Vec2::new(1000.0, 1000.0);
        for _ in 0..500 {
            let choice = choose(&mut rng, &options(&benches, None, Origin::Seated { window: 1, at }));
            let Choice::Ledge(perch) = choice else { panic!("{choice:?}") };
            assert!((perch.feet.x - at.x).abs() >= HOP * HEIGHT, "assez loin : {perch:?}");
        }
        // un bord trop court pour aller plus loin : elle reste assise plutôt que debout
        let short = [seen(1, rect(800.0, 1000.0, 500.0, 800.0))];
        let benches = perch::benches(&short, SCREEN, sitting());
        let from = Origin::Seated { window: 1, at: Vec2::new(1050.0, 1000.0) };
        assert_eq!(choose(&mut rng, &options(&benches, None, from)), Choice::Stay);
        // aucun bord du tout (liste illisible) : elle reste aussi
        assert_eq!(choose(&mut rng, &options(&[], None, from)), Choice::Stay);
    }

    #[test]
    fn she_never_appears_under_the_cursor() {
        let benches = perch::benches(&desk(), SCREEN, sitting());
        let reach = sitting().reach;
        let margin = AVOID * HEIGHT;
        // le curseur au-dessus du bord de la fenêtre de derrière, à gauche
        let cursor = Vec2::new(450.0, 900.0);
        let mut rng = Rng::new(21);
        for _ in 0..2000 {
            let from = Origin::Standing { feet: Vec2::new(2800.0, 1700.0) };
            let Choice::Ledge(perch) = choose(&mut rng, &options(&benches, Some(cursor), from)) else {
                panic!()
            };
            let (x, y) = (perch.feet.x, perch.feet.y);
            let inside = x - reach.left - margin < cursor.x
                && cursor.x < x + reach.right + margin
                && y - reach.up - margin < cursor.y
                && cursor.y < y + reach.down + margin;
            assert!(!inside, "sous le curseur : {perch:?}");
        }
        // sur le seul bord, le curseur au milieu : de part et d'autre, jamais dessous
        let alone = [seen(1, rect(0.0, 1000.0, 3024.0, 800.0))];
        let benches = perch::benches(&alone, SCREEN, sitting());
        let cursor = Vec2::new(1500.0, 1100.0);
        let (mut left, mut right) = (0, 0);
        for _ in 0..1000 {
            let from = Origin::Standing { feet: Vec2::new(100.0, 1700.0) };
            let Choice::Ledge(perch) = choose(&mut rng, &options(&benches, Some(cursor), from)) else {
                panic!()
            };
            assert!(
                perch.feet.x <= cursor.x - reach.right - margin
                    || perch.feet.x >= cursor.x + reach.left + margin
            );
            if perch.feet.x < cursor.x { left += 1 } else { right += 1 }
        }
        assert!(left > 300 && right > 300, "{left} {right}");
    }

    #[test]
    fn without_any_edge_she_stands_at_the_bottom_of_the_screen_elsewhere() {
        let standing = upright();
        let floor_y = SCREEN.max().y - FLOOR * HEIGHT - standing.reach.down;
        let mut rng = Rng::new(2);
        let feet = Vec2::new(1500.0, floor_y);
        let cursor = Vec2::new(2500.0, 1800.0);
        for _ in 0..1000 {
            let choice = choose(&mut rng, &options(&[], Some(cursor), Origin::Standing { feet }));
            let Choice::Floor(to) = choice else { panic!("{choice:?}") };
            assert_eq!(to.y, floor_y);
            // entière dans l'écran, ailleurs, loin du curseur
            assert!(to.x - standing.reach.left >= 0.0 && to.x + standing.reach.right <= SCREEN.max().x);
            assert!((to.x - feet.x).abs() >= HOP * HEIGHT);
            let margin = AVOID * HEIGHT;
            assert!(
                to.x + standing.reach.right + margin <= cursor.x
                    || to.x - standing.reach.left - margin >= cursor.x
            );
        }
        // un écran trop bas pour elle : elle reste
        let tiny = Options {
            screen: rect(0.0, 0.0, 3024.0, 400.0),
            ..options(&[], None, Origin::Standing { feet })
        };
        assert_eq!(choose(&mut rng, &tiny), Choice::Stay);
    }

    #[test]
    fn the_cut_keeps_what_is_outside_and_nothing_twice() {
        assert_eq!(without(vec![(0.0, 10.0)], 3.0, 5.0), [(0.0, 3.0), (5.0, 10.0)]);
        assert_eq!(without(vec![(0.0, 10.0)], 12.0, 15.0), [(0.0, 10.0)]);
        assert_eq!(without(vec![(0.0, 10.0)], -5.0, -1.0), [(0.0, 10.0)]);
        assert_eq!(without(vec![(0.0, 10.0)], -5.0, 15.0), []);
        assert_eq!(without(vec![(4.0, 4.0)], 0.0, 3.0), [(4.0, 4.0)], "un seul point compte");
        assert_eq!(without(vec![(0.0, 10.0)], f32::NAN, 5.0), [(5.0, 10.0)]);
        assert_eq!(without(vec![(0.0, 10.0)], f32::NAN, f32::NAN), []);
    }

    // --- ce que fait `Roamer`, dans l'ordre de la boucle ---

    fn full(width: u32, height: u32) -> AlphaMask {
        AlphaMask::from_rgba(width, height, &vec![255; (width * height * 4) as usize], 96, 0)
    }

    fn dressed() -> Look {
        dressed_with(true)
    }

    /// Avec ou sans ses planches de vie (sans elles, elle change de place d'un coup).
    fn dressed_with(life_sheets: bool) -> Look {
        Look::new(
            Wardrobe {
                cutout: full(220, 420),
                stand: Some(full(stand::CELL_WIDTH, sheets::CELL_HEIGHT)),
                perch: Some(PerchMasks {
                    narrow: full(sheets::NARROW, sheets::CELL_HEIGHT),
                    wide: full(sheets::WIDE, sheets::CELL_HEIGHT),
                }),
                life: life_sheets.then(|| full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
                moves: Moves::default(),
            },
            HEIGHT,
        )
    }

    fn body(look: &Look) -> Capri {
        let mut capri = Capri::new(look.standing_size(), SCREEN, Vec2::new(2400.0, 1200.0));
        capri.set_feet(look.standing_feet());
        capri.set_leave_frames(look.leave_frames());
        capri
    }

    /// Un faux bureau : des fenêtres, le curseur, et combien de fois la liste a été lue.
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
        fn recheck(&mut self, capri: &mut Capri, stature: Stature) {
            let Some(spot) = capri.perched() else { return };
            match perch::follow(&self.windows, SCREEN, stature, spot) {
                Ok(perch) if self.perch => capri.ride(perch),
                _ => capri.take_off(),
            }
        }
    }

    fn world() -> Desk {
        Desk { windows: desk(), cursor: None, perch: true, reads: 0 }
    }

    /// La boucle, de l'image `from` à l'image `to` (exclue) : son pas, son dessin, sa vie.
    fn run(
        roamer: &mut Roamer,
        look: &mut Look,
        capri: &mut Capri,
        desk: &mut Desk,
        busy: Busy,
        frames: std::ops::Range<u64>,
    ) -> Vec<Event> {
        timed(roamer, look, capri, desk, busy, false, frames).into_iter().map(|(_, event)| event).collect()
    }

    /// De même, de la musique jouant ou non, et l'heure (en secondes) de chaque événement.
    fn timed(
        roamer: &mut Roamer,
        look: &mut Look,
        capri: &mut Capri,
        desk: &mut Desk,
        busy: Busy,
        music: bool,
        frames: std::ops::Range<u64>,
    ) -> Vec<(f64, Event)> {
        let mut events = Vec::new();
        for frame in frames {
            capri.step(1.0 / 24.0, frame);
            let music = music && look.wants_music(capri);
            look.update(capri, frame, false, music);
            let now = frame as f64 / 24.0;
            events.extend(
                roamer.step(look, capri, desk, busy, now, frame).into_iter().map(|event| (now, event)),
            );
        }
        events
    }

    /// Un départ, réduit à : vers un bord ?, combien de bords, demandé ?
    fn ledge(event: &Event) -> Option<(bool, usize, bool)> {
        match *event {
            Event::Leaves { to, benches, asked, .. } => {
                Some((matches!(to, Choice::Ledge(_)), benches, asked))
            }
            _ => None,
        }
    }

    /// Lancée à l'image 0, là.
    fn launched(seed: u64, enabled: bool) -> (Roamer, Look, Capri) {
        let mut look = dressed();
        let mut capri = body(&look);
        capri.step(0.0, 0);
        look.enter(&mut capri, 0);
        (Roamer::new(seed, 0.0, enabled), look, capri)
    }

    #[test]
    fn soon_after_launch_she_fades_out_and_appears_seated_on_an_edge_window_shown() {
        let (mut roamer, mut look, mut capri) = launched(4, true);
        let mut desk = world();
        let first = (roamer.next() * 24.0).ceil() as u64;
        assert!(run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..first).is_empty());
        assert_eq!(desk.reads, 0, "la liste n'est pas lue avant qu'elle choisisse");
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), first..first + 1);
        assert_eq!(events.len(), 1);
        assert_eq!(ledge(&events[0]), Some((true, 3, false)));
        assert_eq!(desk.reads, 1, "une lecture pour choisir");
        let Some(Choice::Ledge(to)) = roamer.heading() else { panic!() };
        // elle s'efface debout, là où elle était, la fenêtre montrée : pas cachée
        assert!(look.is_traveling() && !look.is_hidden() && !look.is_gone());
        let mut arrived = Vec::new();
        for frame in first + 1..first + 60 {
            arrived.extend(run(
                &mut roamer,
                &mut look,
                &mut capri,
                &mut desk,
                Busy::default(),
                frame..frame + 1,
            ));
            assert!(!look.is_gone() && !look.is_between(), "jamais cachée ni entre deux, image {frame}");
            if arrived.is_empty() {
                assert!(capri.perched().is_none(), "pas encore déplacée");
            }
        }
        assert_eq!(arrived, [Event::Arrived { seated: true, lost: false }]);
        assert_eq!(capri.seat(), Some(to.feet));
        assert!(look.is_here() && look.is_seated(&capri));
        // elle y reste de 3 à 8 minutes
        let now = (first + 59) as f64 / 24.0;
        assert!(roamer.next() - now >= SEATED.min - 3.0 && roamer.next() - now <= SEATED.max);
        assert_eq!(desk.reads, 1);
    }

    #[test]
    fn nothing_moves_her_while_she_is_held_back_and_by_hand_she_stays_five_minutes() {
        for busy in [Busy { talking: true, hovered: false }, Busy { talking: false, hovered: true }] {
            let (mut roamer, mut look, mut capri) = launched(4, true);
            let mut desk = world();
            let events = run(&mut roamer, &mut look, &mut capri, &mut desk, busy, 1..24 * 120);
            assert!(events.is_empty(), "{busy:?} : {events:?}");
            assert_eq!(desk.reads, 0);
            // libre : pas avant PATIENCE
            let at = 24 * 120;
            let events = run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), at..at + 24 * 9);
            assert!(events.is_empty());
            let events = run(
                &mut roamer,
                &mut look,
                &mut capri,
                &mut desk,
                Busy::default(),
                at + 24 * 9..at + 24 * 11,
            );
            assert!(matches!(events[..], [Event::Leaves { .. }, ..]), "{events:?}");
        }
        // tenue, puis lâchée : elle reste 5 minutes là où on l'a mise
        let (mut roamer, mut look, mut capri) = launched(4, true);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..40);
        capri.grab(capri.position() + Vec2::new(30.0, 30.0), 1.0);
        assert!(run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 40..24 * 60).is_empty());
        capri.release(70.0);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 24 * 60..24 * 60 + 1);
        assert!((roamer.next() - 60.0 - BY_HAND.min).abs() < 0.1, "{}", roamer.next());
    }

    #[test]
    fn asked_she_leaves_at_once_unless_something_holds_her() {
        let (mut roamer, mut look, mut capri) = launched(4, false);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..24 * 120);
        assert_eq!(desk.reads, 0, "décochée : elle ne part jamais d'elle-même");
        roamer.ask();
        let talking = Busy { talking: true, hovered: false };
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, talking, 24 * 120..24 * 120 + 1);
        assert_eq!(events, [Event::Refused(Hold::Talking)]);
        // le curseur sur elle (on vient d'ouvrir son menu) ne la retient pas
        roamer.ask();
        let hovered = Busy { talking: false, hovered: true };
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, hovered, 24 * 121..24 * 121 + 1);
        assert_eq!((events.len(), ledge(&events[0])), (1, Some((true, 3, true))));
        // en chemin, une autre demande attend qu'elle soit arrivée… et se refuse
        roamer.ask();
        let events =
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 24 * 121 + 1..24 * 121 + 2);
        assert_eq!(events, [Event::Refused(Hold::Away)]);
    }

    #[test]
    fn hidden_on_the_way_she_goes_no_further_and_shown_she_is_where_she_was() {
        let (mut roamer, mut look, mut capri) = launched(4, false);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
        let feet = capri.feet();
        roamer.ask();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 100..110);
        assert!(look.is_traveling());
        look.vanish(&mut capri, 110);
        assert!(look.is_hidden() && !look.is_traveling());
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 110..140);
        assert!(look.is_gone() && roamer.heading().is_none());
        assert_eq!(capri.feet(), feet, "pas déplacée");
        capri.step(0.0, 500);
        look.appear(&mut capri, 500);
        roamer.came_back(500.0 / 24.0, &capri);
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 500..600);
        assert!(events.is_empty(), "{events:?}");
        assert!(look.is_here() && capri.feet() == feet);
    }

    #[test]
    fn her_destination_gone_when_she_appears_she_appears_standing_and_soon_moves_again() {
        let (mut roamer, mut look, mut capri) = launched(4, false);
        let mut desk = Desk { windows: vec![seen(1, rect(200.0, 1000.0, 1600.0, 800.0))], ..world() };
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
        roamer.ask();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 100..101);
        let Some(Choice::Ledge(to)) = roamer.heading() else { panic!() };
        // sa fenêtre se ferme pendant qu'elle s'efface
        desk.windows.clear();
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 101..140);
        assert_eq!(events, [Event::Arrived { seated: false, lost: true }]);
        assert!(capri.perched().is_none());
        assert!((capri.feet() - to.feet).length() < 1.0, "debout là où elle allait");
        let now = 139.0 / 24.0;
        assert!(roamer.next() - now <= LOST.max);
        // puis, sans bord, debout en bas de l'écran
        roamer.ask();
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 200..201);
        assert_eq!((events.len(), ledge(&events[0])), (1, Some((false, 0, true))));
    }

    #[test]
    fn without_perching_she_only_moves_standing_and_never_reads_the_list() {
        let (mut roamer, mut look, mut capri) = launched(4, false);
        let mut desk = Desk { perch: false, ..world() };
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
        for round in 0..5u64 {
            roamer.ask();
            let start = 100 + round * 100;
            let events =
                run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start..start + 100);
            assert_eq!(events.len(), 2, "{events:?}");
            assert_eq!(ledge(&events[0]), Some((false, 0, true)));
            assert_eq!(events[1], Event::Arrived { seated: false, lost: false });
        }
        assert_eq!(desk.reads, 0);
    }

    #[test]
    fn a_gesture_that_takes_her_off_her_edge_is_not_a_lost_edge() {
        let (mut roamer, mut look, mut capri) = launched(4, true);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..40);
        let bench = perch::benches(&desk.windows, SCREEN, sitting())[0];
        capri.set_on(bench.at(bench.lo));
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 40..41);
        // « S'asseoir sur les fenêtres » décochée, ou une taille qui ne tient plus sur ce bord :
        // elle reste là, comme posée à la main
        capri.take_off();
        let now = 41.0 / 24.0;
        roamer.touched(now, &capri);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 41..42);
        assert!(
            (roamer.next() - (now + BY_HAND.min)).abs() < 1e-9,
            "{} contre {}",
            roamer.next(),
            now + 300.0
        );
        // un geste qui la laisse sur son bord, ou debout, ne change rien
        let next = roamer.next();
        roamer.touched(42.0 / 24.0, &capri);
        assert_eq!(roamer.next(), next);
        // sa fenêtre fermée sous elle, sans geste : un bord perdu
        capri.set_on(bench.at(bench.lo));
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 42..43);
        capri.take_off();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 43..44);
        let now = 43.0 / 24.0;
        assert!((now + LOST.min..=now + LOST.max).contains(&roamer.next()), "{}", roamer.next());
    }

    #[test]
    fn standing_without_any_edge_she_moves_only_now_and_then() {
        for perch in [false, true] {
            let (mut roamer, mut look, mut capri) = launched(6, true);
            // aucune fenêtre où tenir, ou elle ne s'assoit pas sur les fenêtres
            let mut desk = Desk { windows: Vec::new(), perch, ..world() };
            let events =
                timed(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), false, 1..24 * 60 * 40);
            let departures: Vec<f64> =
                events.iter().filter(|(_, event)| ledge(event).is_some()).map(|&(at, _)| at).collect();
            assert!(departures.len() >= 4, "en 40 minutes : {departures:?}");
            for pair in departures.windows(2) {
                assert!(pair[1] - pair[0] >= SEATED.min, "debout, elle saute trop souvent : {departures:?}");
            }
            if perch {
                // elle continue de chercher un bord, à la cadence courte
                assert!(desk.reads >= 2 * departures.len(), "{} lectures", desk.reads);
            } else {
                assert_eq!(desk.reads, 0, "sans s'asseoir, la liste n'est jamais lue");
            }
        }
    }

    #[test]
    fn talked_to_while_she_fades_to_leave_she_comes_back_on_her_steps() {
        for frames in [10, 24] {
            let (mut roamer, mut look, mut capri) = launched(4, false);
            let mut desk = world();
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
            let position = capri.position();
            roamer.ask();
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 100..100 + frames);
            assert!(look.is_traveling(), "en chemin");
            // le champ de saisie s'ouvre (ou le micro), sous elle, là où elle s'efface
            let talking = Busy { talking: true, hovered: false };
            let at = 100 + frames;
            let events = run(&mut roamer, &mut look, &mut capri, &mut desk, talking, at..at + 1);
            assert_eq!(events, [Event::TurnedBack], "après {frames} images");
            assert!(!look.is_traveling() && roamer.heading().is_none());
            let events = run(&mut roamer, &mut look, &mut capri, &mut desk, talking, at + 1..at + 100);
            assert!(events.is_empty(), "elle n'apparaît pas ailleurs : {events:?}");
            assert!(look.is_here() && capri.position() == position && capri.perched().is_none());
        }
    }

    #[test]
    fn coming_back_never_shortens_her_wait() {
        let (mut roamer, mut look, mut capri) = launched(4, true);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..40);
        // tenue, puis lâchée à l'image 40 : elle reste 5 minutes
        capri.grab(capri.position() + Vec2::new(30.0, 30.0), 1.0);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 40..41);
        capri.release(2.0);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 41..42);
        let by_hand = roamer.next();
        assert!((by_hand - (41.0 / 24.0 + BY_HAND.min)).abs() < 1e-9);
        // cachée, puis montrée bien avant : son attente ne raccourcit pas
        look.vanish(&mut capri, 42);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 42..80);
        assert!(look.is_gone());
        capri.step(0.0, 24 * 100);
        look.appear(&mut capri, 24 * 100);
        roamer.came_back(100.0, &capri);
        assert_eq!(roamer.next(), by_hand);
        // montrée après son départ prévu : une attente normale, depuis maintenant
        roamer.came_back(1000.0, &capri);
        assert!((1000.0 + STANDING.min..=1000.0 + STANDING.max).contains(&roamer.next()));
    }

    #[test]
    fn without_her_life_sheets_she_arrives_at_once_and_says_so() {
        let mut look = dressed_with(false);
        let mut capri = body(&look);
        capri.step(0.0, 0);
        look.enter(&mut capri, 0);
        let mut roamer = Roamer::new(4, 0.0, false);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..10);
        roamer.ask();
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 10..11);
        assert_eq!(events.len(), 2, "{events:?}");
        let Event::Leaves { to: Choice::Ledge(perch), .. } = events[0] else { panic!("{events:?}") };
        assert_eq!(events[1], Event::Arrived { seated: true, lost: false });
        assert_eq!(capri.seat(), Some(perch.feet));
    }

    #[test]
    fn music_holds_her_seated_and_she_says_so_when_asked() {
        let (mut roamer, mut look, mut capri) = launched(4, true);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..40);
        let bench = perch::benches(&desk.windows, SCREEN, sitting())[0];
        capri.set_on(bench.at(bench.lo));
        // de la musique : elle se balance, assise, bien au-delà de son départ prévu
        let events =
            timed(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), true, 40..24 * 600);
        assert!(look.is_grooving());
        assert!(events.is_empty(), "{events:?}");
        roamer.ask();
        let events = timed(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            true,
            24 * 600..24 * 600 + 1,
        );
        assert_eq!(events, [(600.0, Event::Refused(Hold::Music))]);
        // la musique s'arrête : elle finit son tour, puis attend encore PATIENCE
        let after = 24 * 600 + 1;
        let events = timed(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            after..after + 24 * 20,
        );
        let (at, event) = events.first().copied().expect("elle repart");
        assert!(ledge(&event).is_some() && at >= 600.0 + PATIENCE, "{events:?}");
    }

    #[test]
    fn asked_on_her_only_short_edge_she_goes_to_stand_at_the_bottom() {
        let short = [seen(1, rect(800.0, 1000.0, 500.0, 800.0))];
        let benches = perch::benches(&short, SCREEN, sitting());
        let from = Origin::Seated { window: 1, at: Vec2::new(1050.0, 1000.0) };
        let mut rng = Rng::new(3);
        assert_eq!(choose(&mut rng, &options(&benches, None, from)), Choice::Stay, "d'elle-même, elle reste");
        let asked = Options { asked: true, ..options(&benches, None, from) };
        assert!(matches!(choose(&mut rng, &asked), Choice::Floor(_)), "demandé, elle va debout");
        // dans la boucle : « Changer de place » la fait descendre, sans dire qu'il n'y a nulle part
        let (mut roamer, mut look, mut capri) = launched(4, false);
        let mut desk = Desk { windows: short.to_vec(), ..world() };
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..40);
        capri.set_on(benches[0].at(1050.0));
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 40..100);
        roamer.ask();
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 100..101);
        assert_eq!(events.len(), 1);
        assert_eq!(ledge(&events[0]), Some((false, 1, true)), "{events:?}");
    }

    #[test]
    fn a_lost_edge_makes_her_choose_soon() {
        let (mut roamer, mut look, mut capri) = launched(4, false);
        let mut desk = world();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..40);
        capri.set_on(perch::benches(&desk.windows, SCREEN, sitting())[0].at(1000.0));
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 40..41);
        roamer.set_enabled(true, 41.0 / 24.0);
        capri.take_off();
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 41..42);
        let now = 41.0 / 24.0;
        assert!((now + LOST.min..=now + LOST.max).contains(&roamer.next()), "{}", roamer.next());
        // d'un lancement à l'autre, la même graine donne la même vie
        let ledge = |seed| {
            let (mut roamer, mut look, mut capri) = launched(seed, true);
            let mut desk = world();
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..24 * 45);
            (roamer.next(), capri.seat())
        };
        assert_eq!(ledge(77), ledge(77));
    }

    // --- sa marche, ses éclats ---

    /// Avec toutes ses planches de gestes.
    fn moving() -> Look {
        moving_with(true)
    }

    /// Avec toutes ses planches de gestes, avec ou sans ses planches pour apparaître et s'effacer.
    fn moving_with(life_sheets: bool) -> Look {
        Look::new(
            Wardrobe {
                cutout: full(220, 420),
                stand: Some(full(stand::CELL_WIDTH, sheets::CELL_HEIGHT)),
                perch: Some(PerchMasks {
                    narrow: full(sheets::NARROW, sheets::CELL_HEIGHT),
                    wide: full(sheets::WIDE, sheets::CELL_HEIGHT),
                }),
                life: life_sheets.then(|| full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
                moves: Moves {
                    walk: Some(full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
                    dangle: Some(full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
                    glitch: Some(full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
                    grin: Some(full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
                    perch_glitch: true,
                    perch_blink_glitch: true,
                    stand_blink_glitch: true,
                },
            },
            HEIGHT,
        )
    }

    /// Lancée à l'image 0, là, avec toutes ses planches de gestes.
    fn launched_moving(seed: u64, enabled: bool) -> (Roamer, Look, Capri) {
        let mut look = moving();
        let mut capri = body(&look);
        capri.step(0.0, 0);
        look.enter(&mut capri, 0);
        (Roamer::new(seed, 0.0, enabled), look, capri)
    }

    /// « Changer de place » jusqu'à ce qu'elle parte en marchant (sans bord, debout en bas de
    /// l'écran) ; rend l'image où elle part, et où elle va.
    fn until_she_walks(
        roamer: &mut Roamer,
        look: &mut Look,
        capri: &mut Capri,
        desk: &mut Desk,
        from: u64,
    ) -> (u64, Vec2) {
        for round in 0..60 {
            let start = from + round * 400;
            roamer.ask();
            let events = run(roamer, look, capri, desk, Busy::default(), start..start + 1);
            if let [Event::Leaves { to: Choice::Floor(to), walking: true, .. }] = events[..] {
                return (start, to);
            }
            run(roamer, look, capri, desk, Busy::default(), start + 1..start + 400);
        }
        panic!("elle ne part jamais en marchant");
    }

    #[test]
    fn at_the_bottom_she_walks_left_to_her_new_place_and_fades_otherwise() {
        let (mut roamer, mut look, mut capri) = launched_moving(4, false);
        let mut desk = Desk { perch: false, ..world() };
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
        let speed = look.walk_speed().expect("sa planche de marche");
        let (mut walked, mut faded, mut faded_at_floor) = (0, 0, 0);
        for round in 0..60 {
            let start = 100 + round * 400;
            let feet = capri.feet();
            roamer.ask();
            let events =
                run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start..start + 1);
            let [Event::Leaves { to: Choice::Floor(to), walking, .. }] = events[..] else {
                panic!("{events:?}")
            };
            let at_floor = (feet.y - to.y).abs() <= 1.0;
            if walking {
                walked += 1;
                assert!(at_floor && to.x < feet.x, "de là où elle est, vers la gauche : {feet:?} → {to:?}");
                assert!(f64::from((feet.x - to.x) / speed) <= WALK_LONGEST);
                assert_eq!((roamer.walking_to(), roamer.heading()), (Some(to), None));
                assert!(look.is_walking(&capri) && look.is_here() && !look.is_traveling());
                let mut last = capri.feet().x;
                let mut arrived = Vec::new();
                for frame in start + 1..start + 400 {
                    arrived =
                        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), frame..frame + 1);
                    assert!(capri.feet().x <= last + 1e-3, "jamais à reculons, image {frame}");
                    assert!(look.is_here() && !look.is_gone(), "toujours là, image {frame}");
                    last = capri.feet().x;
                    if !arrived.is_empty() {
                        break;
                    }
                }
                assert_eq!(arrived, [Event::Arrived { seated: false, lost: false }]);
                assert!((capri.feet().x - to.x).abs() <= 1.0 && !look.is_walking(&capri));
                assert_eq!(roamer.walking_to(), None);
            } else {
                faded += 1;
                if at_floor {
                    faded_at_floor += 1;
                    // vers la droite (elle regarde à gauche), ou trop loin pour y marcher
                    assert!(
                        to.x > feet.x || f64::from((feet.x - to.x) / speed) > WALK_LONGEST,
                        "{feet:?} → {to:?}"
                    );
                }
                assert!(look.is_traveling());
            }
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 1..start + 400);
            assert!(look.is_here() && !look.is_walking(&capri));
        }
        assert!(walked >= 5 && faded >= 5 && faded_at_floor >= 3, "{walked} {faded} {faded_at_floor}");

        // sans sa planche de marche : elle s'efface toujours, comme avant
        let (mut roamer, mut look, mut capri) = launched(4, false);
        let mut desk = Desk { perch: false, ..world() };
        for round in 0..20 {
            let start = 100 + round * 400;
            roamer.ask();
            let events =
                run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start..start + 1);
            assert!(matches!(events[..], [Event::Leaves { walking: false, .. }]), "{events:?}");
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 1..start + 400);
        }
    }

    #[test]
    fn talked_to_while_walking_she_stops_there_and_asked_she_refuses() {
        let (mut roamer, mut look, mut capri) = launched_moving(4, false);
        let mut desk = Desk { perch: false, ..world() };
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
        let (start, to) = until_she_walks(&mut roamer, &mut look, &mut capri, &mut desk, 100);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 1..start + 12);
        roamer.ask();
        let events =
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 12..start + 13);
        assert_eq!(events, [Event::Refused(Hold::Walking)]);
        assert!(look.is_walking(&capri));
        // on lui parle : elle s'arrête là, sa boucle reprend, et n'arrive jamais
        let talking = Busy { talking: true, hovered: false };
        let events = run(&mut roamer, &mut look, &mut capri, &mut desk, talking, start + 13..start + 14);
        assert_eq!(events, [Event::Halted]);
        let x = capri.feet().x;
        assert!(x > to.x + 1.0 && !look.is_walking(&capri) && roamer.walking_to().is_none());
        let events =
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 14..start + 400);
        assert!(events.is_empty(), "{events:?}");
        assert_eq!(capri.feet().x, x);
        assert!(look.is_here() && capri.stance() == Stance::Floating);

        // attrapée en marche : elle pend ; lâchée, elle reste là où on la met (5 minutes)
        let (start, _) = until_she_walks(&mut roamer, &mut look, &mut capri, &mut desk, start + 400);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 1..start + 6);
        let cursor = capri.position() + Vec2::new(50.0, 50.0);
        let grab = crate::input::Grab { at: cursor, since: (start + 6) as f64 / 24.0, trail: Vec::new() };
        assert!(look.hold(&mut capri, &grab));
        let events =
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 6..start + 30);
        assert!(events.is_empty() && look.is_dangling(&capri) && roamer.walking_to().is_none(), "{events:?}");
        capri.release((start + 300) as f64 / 24.0);
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 30..start + 31);
        let now = (start + 30) as f64 / 24.0;
        assert!((roamer.next() - (now + BY_HAND.min)).abs() < 1e-6, "{}", roamer.next());
        assert_eq!(look.gesture(), Some(crate::look::Gesture::Rise), "lâchée, elle se relève");
    }

    #[test]
    fn seated_she_fidgets_now_and_then_never_while_held_back() {
        let seated = |busy: Busy, music: bool, enabled: bool| {
            let (mut roamer, mut look, mut capri) = launched_moving(8, enabled);
            // une seule fenêtre, assez courte pour qu'elle n'en reparte que debout, ou pas
            let mut desk = Desk { windows: vec![seen(1, rect(800.0, 1000.0, 500.0, 800.0))], ..world() };
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..40);
            let bench = perch::benches(&desk.windows, SCREEN, sitting())[0];
            capri.set_on(bench.at(1050.0));
            timed(&mut roamer, &mut look, &mut capri, &mut desk, busy, music, 40..24 * 60 * 30)
                .into_iter()
                .filter(|(_, event)| *event == Event::Fidgeted)
                .map(|(at, _)| at)
                .collect::<Vec<f64>>()
        };
        let free = seated(Busy::default(), false, true);
        assert!(free.len() >= 5, "en 30 minutes : {free:?}");
        assert!(free[0] >= 40.0 / 24.0 + FIDGET.min - 1.0, "{free:?}");
        for pair in free.windows(2) {
            assert!(pair[1] - pair[0] >= FIDGET.min, "{free:?}");
            assert!(pair[1] - pair[0] <= FIDGET.max + PATIENCE + 5.0, "{free:?}");
        }
        assert!(seated(Busy { talking: true, hovered: false }, false, true).is_empty(), "on se parle");
        assert!(seated(Busy::default(), true, true).is_empty(), "la musique");
        assert!(seated(Busy::default(), false, false).is_empty(), "elle ne vit pas sa vie");
        // la même graine, les mêmes éclats
        assert_eq!(free, seated(Busy::default(), false, true));
    }

    #[test]
    fn a_walk_stopped_by_the_edge_or_a_new_size_is_an_arrival_and_she_rests() {
        for stop in ["bord", "taille"] {
            let (mut roamer, mut look, mut capri) = launched_moving(4, false);
            let mut desk = Desk { perch: false, ..world() };
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
            let (start, to) = until_she_walks(&mut roamer, &mut look, &mut capri, &mut desk, 100);
            roamer.set_enabled(true, start as f64 / 24.0);
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 1..start + 12);
            assert!(look.is_walking(&capri));
            let x = capri.feet().x;
            if stop == "bord" {
                // l'écran rétrécit : son bord passe entre elle et là où elle va
                let left = (x + to.x) / 2.0 - capri.size().x * look.standing_feet().x;
                capri.set_bounds(Rect::new(
                    Vec2::new(left, 0.0),
                    Vec2::new(SCREEN.size.x - left, SCREEN.size.y),
                ));
            } else {
                // elle grandit en marchant : elle s'arrête là, sans glisser
                look.set_height(HEIGHT * 1.2);
                capri.set_size(look.standing_size());
            }
            let events = timed(
                &mut roamer,
                &mut look,
                &mut capri,
                &mut desk,
                Busy::default(),
                false,
                start + 12..start + 12 + 24 * 20,
            );
            assert_eq!(
                events.first().map(|(_, event)| *event),
                Some(Event::Arrived { seated: false, lost: false }),
                "{stop} : {events:?}"
            );
            assert!(events.len() == 1, "{stop} : elle se repose là, sans repartir : {events:?}");
            assert!(!look.is_walking(&capri) && look.is_here() && roamer.walking_to().is_none());
            let feet = capri.feet().x;
            if stop == "bord" {
                assert!(feet < x && feet > to.x + 1.0, "arrêtée en chemin, par le bord : {feet}");
            } else {
                assert!((feet - x).abs() < 1e-3, "arrêtée là, sans glisser : {feet} contre {x}");
            }
        }
    }

    #[test]
    fn hidden_while_walking_she_stops_and_shown_again_it_is_no_arrival() {
        for life_sheets in [true, false] {
            let mut look = moving_with(life_sheets);
            let mut capri = body(&look);
            capri.step(0.0, 0);
            look.enter(&mut capri, 0);
            let mut roamer = Roamer::new(4, 0.0, false);
            let mut desk = Desk { perch: false, ..world() };
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..100);
            let (start, _) = until_she_walks(&mut roamer, &mut look, &mut capri, &mut desk, 100);
            run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), start + 1..start + 10);
            let x = capri.feet().x;
            look.vanish(&mut capri, start + 10);
            // la boucle ne fait plus de pas une fois qu'elle est partie
            let mut frame = start + 10;
            while !look.is_gone() {
                run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), frame..frame + 1);
                frame += 1;
            }
            assert!(!capri.is_walking(), "vie {life_sheets}");
            // montrée bien plus tard, pendant qu'on lui parle : ni arrivée, ni départ
            capri.step(0.0, start + 500);
            look.appear(&mut capri, start + 500);
            roamer.came_back((start + 500) as f64 / 24.0, &capri);
            let talking = Busy { talking: true, hovered: false };
            let events =
                run(&mut roamer, &mut look, &mut capri, &mut desk, talking, start + 500..start + 600);
            assert!(events.is_empty(), "vie {life_sheets} : {events:?}");
            assert_eq!(capri.feet().x, x, "vie {life_sheets} : là où elle s'est arrêtée");
            assert_eq!(roamer.walking_to(), None);
        }
    }

    #[test]
    fn a_gesture_holds_her_departure_only_while_it_plays() {
        let (mut roamer, mut look, mut capri) = launched_moving(4, true);
        let mut desk = world();
        let first = (roamer.next() * 24.0).ceil() as u64;
        run(&mut roamer, &mut look, &mut capri, &mut desk, Busy::default(), 1..first - 1);
        // elle sourit juste avant de partir
        assert!(look.poke(&mut capri, first - 1).is_some());
        let next = roamer.next();
        let events = timed(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            first - 1..first + 200,
        );
        let (at, event) = events.first().copied().expect("elle part");
        assert!(matches!(event, Event::Leaves { .. }), "{event:?}");
        let ends = (first - 1 + crate::look::LONGEST_GESTURE) as f64 / 24.0;
        assert!(
            at >= next && at <= ends + 1.0 / 24.0,
            "partie à {at}, prévu à {next}, le geste fini à {ends}"
        );
        assert!((at - ends).abs() <= 1.0 / 24.0, "juste après son sourire : {at} contre {ends}");
    }

    // --- son repos ---

    use crate::look::{LieMasks, NapMasks, Rest, Wants};
    use crate::sheets::lie;

    /// Lancée à l'image 0, là, ses planches de repos offertes et chargées (sauf `undressed`).
    fn resting(seed: u64, enabled: bool, undressed: bool) -> (Roamer, Look, Capri) {
        let mut look = dressed();
        look.offer(Wants { lying: true, sleeping: true });
        if !undressed {
            look.dress_lying(Some(LieMasks {
                narrow: full(lie::NARROW, lie::CELL_HEIGHT),
                wide: full(lie::WIDE, lie::CELL_HEIGHT),
            }));
            look.dress_sleeping(Some(NapMasks {
                sleep: full(life::CELL_WIDTH, sheets::CELL_HEIGHT),
                wake: full(life::CELL_WIDTH, sheets::CELL_HEIGHT),
            }));
        }
        let mut capri = body(&look);
        capri.step(0.0, 0);
        look.enter(&mut capri, 0);
        (Roamer::new(seed, 0.0, enabled), look, capri)
    }

    /// La boucle, comme `timed`, une image sur `every` : posée, elle suit sa fenêtre (`World::recheck`,
    /// comme la veille posée de l'app) et sait s'il y a la place pour s'allonger ; `poke` : à ces
    /// heures-là (en secondes), on s'occupe d'elle (`Look::stir`).
    #[allow(clippy::too_many_arguments)]
    fn lived(
        roamer: &mut Roamer,
        look: &mut Look,
        capri: &mut Capri,
        desk: &mut Desk,
        busy: Busy,
        music: bool,
        frames: std::ops::Range<u64>,
        every: u64,
        poke: &dyn Fn(f64) -> bool,
    ) -> Vec<(f64, Event)> {
        let mut events = Vec::new();
        for frame in frames.step_by(every as usize) {
            let now = frame as f64 / 24.0;
            capri.step(every as f32 / 24.0, frame);
            desk.recheck(capri, look.stature());
            let room = look
                .lying_stature()
                .zip(capri.perched())
                .is_some_and(|(stature, spot)| perch::fits(&desk.windows, SCREEN, stature, spot));
            look.set_room_to_lie(room);
            look.set_conversing(busy.talking);
            let music = music && look.wants_music(capri);
            look.update(capri, frame, busy.talking, music);
            if poke(now) {
                look.stir(capri, frame);
            }
            events.extend(
                roamer.step(look, capri, desk, busy, now, frame).into_iter().map(|event| (now, event)),
            );
            // allongée, toujours posée, sur son bord, avec la place
            if matches!(look.rest(), Rest::Lying | Rest::Asleep { lying: true }) {
                assert!(capri.perched().is_some() && room, "image {frame} : allongée hors d'un bord");
                assert!(look.wants().lying);
            }
            assert!(
                !(busy.talking && matches!(look.rest(), Rest::Asleep { .. })),
                "image {frame} : endormie pendant qu'on lui parle"
            );
        }
        events
    }

    /// L'heure du premier événement qui convient, après `after`.
    fn first(events: &[(f64, Event)], after: f64, which: impl Fn(&Event) -> bool) -> Option<f64> {
        events.iter().find(|(at, event)| *at >= after && which(event)).map(|&(at, _)| at)
    }

    /// Une longue fenêtre seule : un bord où elle tient allongée presque partout.
    fn long_desk() -> Desk {
        Desk { windows: vec![seen(1, rect(100.0, 1000.0, 2800.0, 800.0))], ..world() }
    }

    #[test]
    fn quiet_she_lies_down_on_her_edge_falls_asleep_then_sits_up_by_herself() {
        let (mut roamer, mut look, mut capri) = resting(4, true, false);
        let mut desk = long_desk();
        let never = |_: f64| false;
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            1..24 * 7200,
            4,
            &never,
        );
        let lay = first(&events, 0.0, |event| *event == Event::LiesDown).expect("elle s'allonge");
        assert!((LIE_AFTER.min..LIE_AFTER.max + SEATED.max).contains(&lay), "{lay}");
        let slept =
            first(&events, lay, |event| *event == Event::FallsAsleep { lying: true }).expect("elle s'endort");
        assert!(slept - lay >= DOZE_AFTER.min && slept - lay <= DOZE_AFTER.max + 1.0, "{lay} → {slept}");
        let up =
            first(&events, slept, |event| matches!(event, Event::SitsUp { .. })).expect("elle se rassoit");
        assert_eq!(first(&events, slept, |event| *event == Event::SitsUp { asleep: true }), Some(up));
        assert!(up - slept >= UP_ASLEEP.min && up - slept <= UP_ASLEEP.max + 1.0, "{slept} → {up}");
        // allongée ou endormie, elle ne change pas de place d'elle-même ; ensuite, sa vie reprend
        let left = |event: &Event| matches!(event, Event::Leaves { .. });
        assert!(first(&events, lay, left).is_none_or(|at| at > up), "{events:?}");
        // puis elle repart, après une attente ordinaire, assise
        let next = first(&events, up, left).expect("puis elle repart");
        assert!(next - up >= SEATED.min, "rassise à {up}, repartie à {next}");
    }

    #[test]
    fn woken_by_herself_standing_she_stays_an_ordinary_while_before_going_on() {
        let (mut roamer, mut look, mut capri) = resting(6, true, false);
        let mut desk = Desk { windows: Vec::new(), perch: false, ..world() };
        let never = |_: f64| false;
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            1..24 * 7200,
            4,
            &never,
        );
        let woke = first(&events, 0.0, |event| *event == Event::WakesUp { lying: false, woken: false })
            .expect("elle se réveille d'elle-même");
        let next =
            first(&events, woke, |event| matches!(event, Event::Leaves { .. })).expect("puis elle repart");
        assert!(next - woke >= STANDING.min, "réveillée à {woke}, repartie à {next}");
    }

    #[test]
    fn her_room_or_her_edge_gone_while_she_rests_is_said_as_such_not_as_being_woken() {
        for (gone, asleep) in [("place", true), ("bord", true), ("place", false), ("bord", false)] {
            let (mut roamer, mut look, mut capri) = resting(4, true, false);
            let mut desk = long_desk();
            let never = |_: f64| false;
            let want = if asleep { Rest::Asleep { lying: true } } else { Rest::Lying };
            let mut frame = 1;
            while look.rest() != want {
                lived(
                    &mut roamer,
                    &mut look,
                    &mut capri,
                    &mut desk,
                    Busy::default(),
                    false,
                    frame..frame + 4,
                    4,
                    &never,
                );
                frame += 4;
                assert!(frame < 24 * 3600, "jamais {want:?}");
            }
            // allongée éveillée : après son éclat
            if !asleep {
                let to = frame + 4 * 12;
                lived(
                    &mut roamer,
                    &mut look,
                    &mut capri,
                    &mut desk,
                    Busy::default(),
                    false,
                    frame..to,
                    4,
                    &never,
                );
                frame = to;
            }
            let seat = capri.seat().expect("posée");
            if gone == "place" {
                // une fenêtre devant, sur ce qui repose à sa gauche allongée, pas assise
                desk.windows.insert(0, seen(9, rect(seat.x - 245.0, 900.0, 125.0, 200.0)));
            } else {
                desk.windows.clear();
            }
            let events = lived(
                &mut roamer,
                &mut look,
                &mut capri,
                &mut desk,
                Busy::default(),
                false,
                frame..frame + 24,
                4,
                &never,
            );
            let woken = events.iter().any(|(_, event)| matches!(event, Event::WakesUp { woken: true, .. }));
            assert!(!woken, "{gone} : {events:?}");
            // la vraie raison, une fois, endormie ou non
            let why = if gone == "place" { Unsettled::NoRoom } else { Unsettled::LedgeGone };
            let said: Vec<Event> = events
                .iter()
                .map(|&(_, event)| event)
                .filter(|event| matches!(event, Event::Unsettled { .. }))
                .collect();
            assert_eq!(said, [Event::Unsettled { why, asleep }], "{gone} : {events:?}");
            assert_eq!(look.rest(), Rest::Awake, "{gone}");
            assert_eq!(capri.perched().is_some(), gone == "place", "{gone}");
        }
    }

    #[test]
    fn talking_music_a_turned_off_life_or_no_sheets_keep_her_from_resting() {
        let never = |_: f64| false;
        let rests = |events: &[(f64, Event)]| {
            events.iter().any(|(_, event)| matches!(event, Event::LiesDown | Event::FallsAsleep { .. }))
        };
        // on se parle tout du long
        let (mut roamer, mut look, mut capri) = resting(4, true, false);
        let talking = Busy { talking: true, hovered: false };
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut long_desk(),
            talking,
            false,
            1..24 * 2400,
            4,
            &never,
        );
        assert!(!rests(&events), "{events:?}");
        // de la musique tout du long
        let (mut roamer, mut look, mut capri) = resting(4, true, false);
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut long_desk(),
            Busy::default(),
            true,
            1..24 * 2400,
            4,
            &never,
        );
        assert!(!rests(&events), "{events:?}");
        // « Vivre sa vie » décochée : elle ne commence pas à se reposer
        let (mut roamer, mut look, mut capri) = resting(4, false, false);
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut long_desk(),
            Busy::default(),
            false,
            1..24 * 2400,
            4,
            &never,
        );
        assert!(!rests(&events), "{events:?}");
        // ses planches pas offertes : jamais, et elle n'en veut pas
        let (mut roamer, mut look, mut capri) = resting(4, true, false);
        look.offer(Wants::default());
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut long_desk(),
            Busy::default(),
            false,
            1..24 * 2400,
            4,
            &never,
        );
        assert!(!rests(&events) && look.wants() == Wants::default(), "{events:?}");
    }

    #[test]
    fn her_sheets_not_ready_yet_she_waits_seated_and_lies_down_once_they_come() {
        let (mut roamer, mut look, mut capri) = resting(4, true, true);
        let mut desk = long_desk();
        let never = |_: f64| false;
        let mut wished = None;
        let mut frame = 1;
        while wished.is_none() {
            let events = lived(
                &mut roamer,
                &mut look,
                &mut capri,
                &mut desk,
                Busy::default(),
                false,
                frame..frame + 4,
                4,
                &never,
            );
            assert!(!events.iter().any(|(_, event)| *event == Event::LiesDown), "pas encore");
            if look.wants().lying {
                wished = Some(frame);
            }
            frame += 4;
            assert!(frame < 24 * 1800, "elle n'en a jamais envie");
        }
        // assise en attendant, elle en a envie à chaque pas
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            frame..frame + 240,
            4,
            &never,
        );
        assert!(events.is_empty() && look.wants().lying && look.rest() == Rest::Awake, "{events:?}");
        assert!(look.is_seated(&capri));
        // elles arrivent : elle s'allonge au pas suivant
        look.dress_lying(Some(LieMasks {
            narrow: full(lie::NARROW, lie::CELL_HEIGHT),
            wide: full(lie::WIDE, lie::CELL_HEIGHT),
        }));
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            frame + 240..frame + 248,
            4,
            &never,
        );
        assert_eq!(events.first().map(|(_, event)| *event), Some(Event::LiesDown), "{events:?}");
    }

    #[test]
    fn woken_again_and_again_she_still_sits_up_before_the_longest() {
        let (mut roamer, mut look, mut capri) = resting(4, true, false);
        let mut desk = long_desk();
        let never = |_: f64| false;
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            1..24 * 900,
            4,
            &never,
        );
        let lay = first(&events, 0.0, |event| *event == Event::LiesDown).expect("elle s'allonge");
        // on la réveille toutes les 7 minutes : elle se rendort, mais se rassoit avant l'heure
        let poke = |now: f64| now > lay && ((now - lay) % 420.0) < 4.0 / 24.0;
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            24 * 900..24 * 7200,
            4,
            &poke,
        );
        let woken =
            events.iter().filter(|(_, event)| *event == Event::WakesUp { lying: true, woken: true }).count();
        assert!(woken >= 2, "{events:?}");
        let up = first(&events, 0.0, |event| matches!(event, Event::SitsUp { .. })).expect("elle se rassoit");
        assert!(up <= lay + LIE_LONGEST + 1.0, "{lay} → {up}");
    }

    #[test]
    fn standing_with_no_edge_she_dozes_then_wakes_by_herself_or_when_asked_to_move() {
        let (mut roamer, mut look, mut capri) = resting(6, true, false);
        let mut desk = Desk { windows: Vec::new(), perch: false, ..world() };
        let never = |_: f64| false;
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            1..24 * 4800,
            4,
            &never,
        );
        let slept = first(&events, 0.0, |event| *event == Event::FallsAsleep { lying: false })
            .expect("elle s'endort");
        assert!((STAND_DOZE_AFTER.min..STAND_DOZE_AFTER.max + 60.0).contains(&slept), "{slept}");
        let woke =
            first(&events, slept, |event| matches!(event, Event::WakesUp { .. })).expect("elle se réveille");
        assert_eq!(
            first(&events, slept, |event| *event == Event::WakesUp { lying: false, woken: false }),
            Some(woke)
        );
        assert!(woke - slept >= UP_ASLEEP.min && woke - slept <= UP_ASLEEP.max + 1.0, "{slept} → {woke}");
        let left = |event: &Event| matches!(event, Event::Leaves { .. });
        assert!(first(&events, slept, left).is_none_or(|at| at > woke), "endormie, elle ne part pas");
        // endormie, on lui demande de changer de place : elle se réveille et part
        let (mut roamer, mut look, mut capri) = resting(6, true, false);
        let mut desk = Desk { windows: Vec::new(), perch: false, ..world() };
        let mut frame = 1;
        while look.rest() != (Rest::Asleep { lying: false }) {
            lived(
                &mut roamer,
                &mut look,
                &mut capri,
                &mut desk,
                Busy::default(),
                false,
                frame..frame + 4,
                4,
                &never,
            );
            frame += 4;
            assert!(frame < 24 * 2400, "elle ne s'endort jamais");
        }
        roamer.ask();
        let events = lived(
            &mut roamer,
            &mut look,
            &mut capri,
            &mut desk,
            Busy::default(),
            false,
            frame..frame + 4,
            4,
            &never,
        );
        assert!(
            matches!(
                events[..],
                [(_, Event::Leaves { asked: true, .. }), (_, Event::WakesUp { woken: true, .. })]
            ),
            "{events:?}"
        );
        assert!(look.is_traveling() && look.rest() == Rest::Awake);
    }
}
