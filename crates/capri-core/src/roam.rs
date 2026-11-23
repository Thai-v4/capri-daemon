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
//! Les distances sont en fractions de sa hauteur debout, comme dans `body.rs` et `perch.rs`.

use crate::body::{Capri, Stance};
use crate::geom::{Rect, Vec2};
use crate::look::Look;
use crate::perch::{self, Bench, Perch, Reach, Seen, Stature};

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
}

impl Hold {
    /// Pour le journal.
    pub fn describe(self) -> &'static str {
        match self {
            Hold::Carried => "on la tient, ou elle vole",
            Hold::Away => "elle apparaît, s'efface ou est cachée",
            Hold::Talking => "on se parle",
            Hold::Music => "elle se balance sur la musique",
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
    /// `benches` : les morceaux de bord où elle tiendrait ; `asked` : « Changer de place ».
    Leaves { to: Choice, benches: usize, asked: bool },
    /// Aucune autre place : elle reste.
    Stays { benches: usize, asked: bool },
    /// Arrivée, assise ou debout ; `lost` : debout parce que le bord où elle allait n'y était plus.
    Arrived { seated: bool, lost: bool },
    /// On lui parle pendant qu'elle s'efface pour partir : elle revient sur ses pas, là où elle
    /// était, et n'ira pas plus loin.
    TurnedBack,
    /// « Changer de place » demandé, et quelque chose l'en empêche.
    Refused(Hold),
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

    /// « Changer de place » : au prochain pas, elle part si rien ne l'en empêche.
    pub fn ask(&mut self) {
        self.asked = true;
    }

    /// On vient de la montrer, à l'instant `now` : elle reste un moment là où elle réapparaît,
    /// comme arrivée d'elle-même, et jamais moins que ce qu'elle attendait déjà (posée à la main
    /// juste avant d'être cachée, elle garde ses 5 minutes). Un « Changer de place » demandé
    /// pendant qu'elle était cachée est oublié.
    pub fn came_back(&mut self, now: f64, capri: &Capri) {
        self.asked = false;
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

    /// Un pas, à l'instant `now` et à l'image d'animation `frame` : après `Look::update`, dans
    /// l'ordre de la boucle. Effacée pour changer de place, elle est déplacée et apparaît là-bas ;
    /// sinon, s'il est temps (ou qu'on le lui demande) et que rien ne la retient, elle choisit où
    /// aller (la seule lecture des fenêtres qu'elle demande, [`World::seen`]) et commence à
    /// s'effacer.
    pub fn step(
        &mut self,
        look: &mut Look,
        capri: &mut Capri,
        world: &mut dyn World,
        busy: Busy,
        now: f64,
        frame: u64,
    ) -> Vec<Event> {
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
        if std::mem::take(&mut self.asked) {
            return match hold {
                Some(hold) => vec![Event::Refused(hold)],
                None => self.leave(look, capri, world, now, frame, true),
            };
        }
        if !self.enabled {
            return Vec::new();
        }
        let free = hold.is_none() && !busy.hovered;
        if self.roam.due(now, free) { self.leave(look, capri, world, now, frame, false) } else { Vec::new() }
    }

    /// L'utilisateur vient d'agir sur elle à l'instant `now` (sa taille, la case « S'asseoir sur
    /// les fenêtres ») ; `capri` : où elle en est après.
    /// Si cela lui a fait quitter son bord, c'est un geste, pas un bord perdu : elle reste là
    /// comme posée à la main ([`BY_HAND`]). Seul un bord parti sous elle (sa fenêtre fermée,
    /// déplacée hors de portée, recouverte) vaut [`LOST`].
    pub fn touched(&mut self, now: f64, capri: &Capri) {
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
        if choice == Choice::Stay || !look.travel(capri, frame) {
            self.roam.stay(now, Stay::Nowhere);
            return vec![Event::Stays { benches: count, asked }];
        }
        self.trip = Some(choice);
        let leaves = Event::Leaves { to: choice, benches: count, asked };
        // sans ses planches de vie, elle est déjà effacée : elle apparaît là-bas tout de suite
        if look.is_between() {
            return vec![leaves, self.arrive(look, capri, world, now, frame)];
        }
        vec![leaves]
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
    use crate::look::{PerchMasks, Wardrobe};
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
            look.update(capri, frame, false, music && look.wants_music(capri));
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
            Event::Leaves { to, benches, asked } => Some((matches!(to, Choice::Ledge(_)), benches, asked)),
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
}
