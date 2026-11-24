//! Son dessin : quelle case de quelle planche montrer à chaque image, où, et ce qui l'attrape.
//!
//! **Ses dessins.** Debout, la boucle de sa pose debout ([`stand`]) ; posée sur une fenêtre,
//! celles de sa pose assise ([`sheets`]), du même artiste. Quand des planches manquent, son dessin
//! d'avant ([`Drawing::Cutout`]) les remplace, comme avant : sans planches debout, c'est lui
//! qu'on voit debout ; sans planches assises, elle se tient debout sur les fenêtres.
//!
//! **Debout** : la boucle, tenue en deux, avec le flottement que calcule `body.rs` (la boucle de
//! l'artiste ne flotte pas, voir [`stand`]) ; elle cligne des yeux à certains tours seulement
//! ([`BLINKS`]). Tenue ou lancée : la même boucle, écrasée et étirée par `body.rs`. Quand elle
//! parle : sa bouche posée par-dessus ([`stand::talking`]), au même endroit.
//!
//! **Assise** : posée, elle apparaît sur le bord ([`sheets::arriving`]), puis sa boucle, où elle
//! cligne aux mêmes tours. Elle parle avec [`sheets::talking`] (la même boucle, la bouche en plus),
//! et se balance quand de la musique joue ([`sheets::music`]) : parler d'abord, puis la musique,
//! puis la boucle. Elle entre dans la musique et en sort au début d'un tour de sa boucle, là où
//! leurs dessins se ressemblent le plus ; elle se met à parler tout de suite. Quand elle repart
//! d'elle-même, elle s'efface là où elle était ([`sheets::leaving`]), puis flotte de nouveau,
//! debout ; quand on l'attrape, elle est debout tout de suite, sous le curseur
//! ([`Look::grab_origin`]).
//!
//! **Le bord de la fenêtre passe sur sa ligne d'assise** ([`sheets::SEAT_Y`]) : ses jambes pendent
//! devant la fenêtre. Ce qu'il lui faut de place, assise, est dans [`Look::stature`].
//!
//! **Les deux poses à la même échelle** : sa hauteur debout ([`stand::STATURE`], des cheveux à la
//! pointe du pied) vaut la hauteur choisie, et un pixel de ses planches assises vaut
//! `1 / STAND_OVER_PERCH` pixel de ses planches debout ([`stand::perch_scale`]).
//!
//! **Apparaître et s'effacer** ([`life`]) : au lancement ([`Look::enter`]), et quand on la montre
//! après l'avoir cachée ([`Look::appear`]), elle se matérialise ([`life::arriving`]) à la place de
//! son image debout, ses pieds sur ses pieds et à la même taille ([`life::over_stand`]), sans
//! flotter (son tour s'arrête : `Capri::still`) ; puis sa boucle, par une coupe franche (ce n'est
//! pas le même dessin, voir [`life`]), et son flottement reprend là où il s'était arrêté. Quand on la
//! cache ([`Look::vanish`]), elle s'efface ([`life::leaving`]), son tour arrêté de même, puis il
//! n'y a plus rien à montrer ([`Look::is_gone`]). Assise (avec ses planches assises), elle s'efface
//! de son bord ([`sheets::leaving`]) et, toujours posée quand on la montre, y réapparaît
//! ([`sheets::arriving`]) ; sa fenêtre partie entre-temps, elle se matérialise debout. Cachée ou
//! montrée au milieu de l'une de ces suites, elle revient sur ses pas : la même suite, à l'envers,
//! du dessin où elle en est (pas de saut). Attrapée pendant qu'elle apparaît, elle est debout tout
//! de suite, là où elle apparaissait ; pendant qu'elle s'efface, rien ne l'attrape. Tant qu'une de
//! ces suites dure, elle n'est pas libre ([`Look::is_here`]). Sans ses planches de vie, ou sans sa
//! pose debout sur laquelle elles s'alignent, elle apparaît et disparaît d'un coup, comme avant.
//! Cachée pendant l'intro du lancement ([`crate::intro`]), avant d'être jamais apparue, elle est
//! partie d'un coup ([`Look::hide_unseen`]) ; montrée, elle se matérialise comme au lancement.
//!
//! **Ses intros jouées par elle** ([`crate::intro::act`], [`Look::perform`]) : à son entrée en
//! scène, ou effacée pour « Voir une intro » ([`Look::bow_out`], [`Look::is_offstage`] : comme
//! pour changer de place, la fenêtre reste montrée), elle tombe du ciel (sa planche en l'air, son
//! corps qui tombe, puis elle se relève comme lâchée), arrive en marchant (de hors de l'écran, ou
//! matérialisée plus près, puis sa marche), ou se rassemble (ses bandes, [`Drawing::Gather`], puis
//! la fin de son arrivée). Tant qu'elle la joue, elle n'est pas là ([`Look::is_here`]) mais rien
//! ne lui échappe : l'attraper la coupe (elle est tenue), la cacher la coupe net (elle est cachée,
//! à sa place), lui parler aussi ([`Look::cut_performance`] : elle se matérialise à sa place), et
//! une autre taille de même (son menu, la molette, la densité de l'écran). Comment elle a fini :
//! [`Look::take_performed`]. Après l'intro du lancement, tant que son bonjour attend, elle ne
//! commence aucun geste d'elle-même ([`Look::set_greeting_due`]).
//!
//! **Changer de place** ([`crate::roam`]) : elle s'efface où elle est, de la même façon
//! ([`Look::travel`]), mais elle n'est pas cachée : la fenêtre reste montrée. Une fois effacée
//! ([`Look::is_between`]), on la déplace, puis elle apparaît là-bas ([`Look::arrive`]), assise si
//! elle y est posée, debout sinon. Cachée en chemin, elle finit de s'effacer là où elle était et n'ira
//! pas plus loin : montrée, elle y réapparaît. « Cachée » ([`Look::is_hidden`]) et « en chemin »
//! ([`Look::is_traveling`]) ne se confondent jamais ; ni l'une ni l'autre n'est libre
//! ([`Look::is_away`]).
//!
//! **Elle guette** de derrière le côté d'une fenêtre (`Stance::Lurking`, où et quand : `perch` et
//! `roam`) : sa boucle debout, ses clignements (glitché compris) et sa bouche, rien d'autre, penchés
//! vers l'extérieur de la fenêtre et coupés net à son bord ([`Lean`], calculé à l'affichage : ses
//! images ne sont jamais modifiées, ni retournées, le même dessin des deux côtés) ; ce qui se voit
//! d'elle est dit dans [`stand`] (sa tête et ses cheveux ; à gauche, aussi son épaule, son haut, son
//! bras et un bout de sa jupe, le long de la coupe). Elle y arrive cachée derrière la fenêtre et en sort
//! en glissant ([`Clip::Slide`] : son corps glisse à la cadence de l'écran, `Capri::emerge`), et
//! s'y retire de même avant de partir ou d'être cachée ; jamais ses planches de vie sur un côté.
//! Ses gestes ne s'y jouent pas (sourire, se relever, s'endormir : dessinés dans ses planches de
//! vie, d'un dessin plus ancien, ils ne s'alignent pas sur sa hanche ni ne garantissent ses yeux) ;
//! un clic sur elle n'y fait rien. Seul ce qui se voit d'elle attrape le curseur
//! ([`Look::is_over`]) ; attrapée, elle se redresse autour de ce que tient le curseur
//! ([`Look::grab_origin`]) ; son bandeau se place au-dessus de sa tête penchée ([`Look::shown`]).
//! Son côté parti, elle ne guette plus : debout, entière, là où elle est.
//!
//! **Ses gestes** ([`Gesture`]) : une suite jouée une fois par-dessus sa boucle, puis sa boucle
//! reprend, par une coupe franche. Lâchée debout après avoir été tenue ou lancée, elle se relève
//! ([`life::rising`], dans `update`) ; un clic sur elle ([`Look::poke`]) la fait sourire debout
//! ([`life::grinning`], deux tours) ou lui donne un éclat assise ([`sheets::glitching`]) ; quand
//! Claude Code finit son tour, elle sourit, debout seulement, pas plus d'une fois par
//! [`CHEER_EVERY`] ([`Look::cheer`]) ; assise, sa vie lui donne de temps en temps un éclat
//! ([`Look::fidget`], appelée par `roam`). Debout, son tour s'arrête le temps du geste (elle ne
//! flotte pas), puis reprend là où il était ; assise, sa boucle repart de son dessin 0, sur lequel
//! l'éclat finit. **Les priorités** : une prise passe avant tout (le geste s'arrête net, elle pend) ;
//! puis ce qu'on lui dit et ce qu'elle répond (`talking`, [`Look::set_conversing`] : le geste
//! s'arrête, elle parle avec sa boucle et sa bouche, et rien ne commence) ; puis ses gestes ; puis
//! sa boucle. Jamais deux gestes à la fois : un geste demandé pendant un autre ne se joue pas. Un
//! geste ne se joue que si elle est là ([`Look::is_here`]), et finit toujours, au plus tard après
//! [`LONGEST_GESTURE`] images.
//!
//! **Tenue ou lancée** : avec sa planche en l'air ([`life::dangling`]), elle pend, tenue par sa
//! mèche ([`life::GRIP`]), et garde ce dessin en vol ; sans, sa boucle debout écrasée et étirée,
//! comme avant. La prise ([`Look::hold`]) ne la fait pas sauter : son corps ne bouge pas quand on
//! l'attrape (assise, elle se lève sous le curseur, comme avant), puis, à mesure que le curseur
//! bouge, elle glisse pour amener sa mèche sous lui (`Capri::hold_toward`) ; [`Look::grip`] dit où
//! sa mèche est dessinée.
//!
//! **Elle marche** ([`Look::walk`], décidé par `roam`) : sa planche de marche ([`life::walking`]),
//! par-dessus son image debout, son tour arrêté ; sa position avance à la cadence de l'écran
//! (`Capri::walk_to`), à la vitesse de [`life::walk_speed`], dans le sens où elle regarde
//! ([`life::FACING`]). Attrapée, elle pend ; cachée, elle s'arrête et s'efface ; quand on lui
//! parle, `roam` l'arrête ([`Look::halt`]) et elle reprend sa boucle.
//!
//! **Ses clignements** : aux tours de [`BLINKS`] ; au tour [`GLITCHY_BLINK`] de chaque série (le
//! dernier des quatre clignements, toutes les 20 s), la variante glitchée de sa planche, debout
//! comme assise, si elle est là.
//!
//! **Le repli** : chacune de ces planches est facultative ([`Moves`]) ; sans l'une, ce qu'elle porte
//! ne se joue pas, et le reste est comme avant : sans sa planche en l'air, elle est tenue comme
//! avant ; sans sa marche, elle change de place en s'effaçant ; sans le glitch ou le sourire, rien
//! ne se joue ; sans les variantes, ses clignements d'avant. Ses planches de vie de 240 px
//! s'alignent sur sa pose debout : sans elle, aucune ne sert.
//!
//! **Elle se repose** ([`Rest`] ; quand, c'est `roam` qui le décide) : assise, elle s'allonge sur
//! son bord ([`Look::lie_down`] : l'éclat de sa pose allongée, [`lie::glitching`], par une coupe
//! franche depuis sa boucle assise, puis sa boucle allongée, la hanche à la même place, voir
//! [`lie`]), seulement s'il y a la place pour elle allongée ([`Look::set_room_to_lie`], que l'app
//! sait de la lecture des fenêtres qu'elle fait déjà) ; allongée, elle cligne, parle avec sa
//! bouche allongée, a un éclat quand on clique sur elle ([`Gesture::LieGlitch`]), et s'endort
//! ([`Look::doze`] : sa boucle de sommeil, par une coupe franche). Debout et immobile, elle s'endort
//! aussi ([`life::sleeping`], son tour arrêté : elle ne flotte plus). Elle se réveille
//! ([`Look::rouse`], [`Look::stir`]) quand on lui parle, qu'on clique sur elle, ou que Claude Code
//! attend une réponse : allongée, elle redevient allongée éveillée (coupe franche) ; debout, elle
//! se réveille ([`life::waking`]), puis sa boucle par une coupe franche. Attrapée, elle est debout
//! tout de suite, comme assise. Elle se rassoit ([`Look::sit_up`] : l'éclat de sa pose assise,
//! [`sheets::glitching`], qui finit sur sa boucle ; sans lui, une coupe franche), et, s'il n'y a
//! plus la place pour elle allongée, d'elle-même. Son bord parti sous elle, elle s'en efface
//! allongée ([`lie::leaving`]) et flotte, comme assise ; cachée ou en chemin, de même. Assise, la
//! musique l'empêche de s'allonger, et allongée, de s'endormir ; allongée, elle ne se balance pas.
//! Debout, la musique ne change rien : l'app ne la lui dit que posée (`wants_music`), pour se
//! balancer ; elle peut donc s'endormir debout pendant qu'une musique joue. Quand son repos
//! allongée s'arrête sans qu'on s'occupe d'elle (plus la place, plus posée), elle le dit
//! ([`Look::take_unsettled`]) ; sans la place, elle ne se rassoit qu'une fois là (en train de
//! réapparaître allongée, elle finit d'abord).
//!
//! **Ses planches de repos sont chargées à la demande** (elles pèsent lourd, et elle se repose peu)
//! : ce qu'elle en veut ([`Look::wants`]), dès qu'elle voudrait s'allonger ou s'endormir et tant
//! qu'elle les montre ; l'app les charge hors du thread principal et les lui donne
//! ([`Look::dress_lying`], [`Look::dress_sleeping`]), ou les reprend. Tant qu'elles ne sont pas là,
//! elle ne s'allonge pas ni ne s'endort debout (elle reste comme elle est, et attend) ; reprises,
//! elle se rassoit ou se réveille d'un coup. Celles qui manquent dès le lancement
//! ([`Look::offer`]) : elle ne le fait jamais.
//!
//! **Le temps** : l'image d'animation à 24 i/s ; ses dessins sont tenus deux images (12 par
//! seconde), son sourire trois (8 par seconde, sa cadence d'origine), son sommeil allongée trois (8
//! par seconde), son sommeil debout quatre (6 par seconde, sa cadence d'origine). Rien n'est tiré
//! au hasard : la même image donne toujours le même dessin.

use crate::anim::Sheet;
use crate::body::{Capri, LurkFit, Stance};
use crate::geom::{Rect, Vec2};
use crate::input::Grab;
use crate::intro::{self, Act, End, act};
use crate::mask::AlphaMask;
use crate::perch::{FOOT, LurkStature, Reach, Side, Stature};
use crate::roam::Hold;
use crate::sheets::{self, lie, life, stand};

/// Un tour de sa boucle, debout comme assise : 60 images, 2,5 s.
pub const TOUR: u64 = 60;
/// Les tours de sa boucle où elle cligne des yeux, en boucle sur 8 tours (20 s) : quatre
/// clignements, espacés de 5 s, 2,5 s, 5 s puis 7,5 s. Assez pour qu'elle ait l'air vivante, pas
/// à chaque tour, qui ferait mécanique, et jamais au hasard : la même image donne le même dessin.
pub const BLINKS: [bool; 8] = [true, false, true, true, false, true, false, false];

/// Cligne-t-elle des yeux au tour n° `tour` de sa boucle ?
pub fn blinks(tour: u64) -> bool {
    BLINKS[(tour % BLINKS.len() as u64) as usize]
}

/// Le tour de chaque série de [`BLINKS`] où elle cligne avec la variante glitchée de son
/// clignement (s'il y en a une) : le dernier des quatre clignements, une fois toutes les 20 s. Une
/// règle fixe, comme ses clignements : la même image donne le même dessin.
pub const GLITCHY_BLINK: u64 = 5;

/// Cligne-t-elle avec la variante glitchée au tour n° `tour` ?
pub fn blinks_glitchy(tour: u64) -> bool {
    tour % BLINKS.len() as u64 == GLITCHY_BLINK
}

/// Quand Claude Code finit son tour, elle ne sourit pas plus d'une fois en tant d'images : une
/// minute.
pub const CHEER_EVERY: u64 = 60 * 24;

/// Un geste : une suite jouée une fois par-dessus sa boucle (voir le module).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gesture {
    /// Debout, lâchée après avoir été tenue ou lancée : elle se relève ([`life::rising`]).
    Rise,
    /// Debout : elle sourit ([`life::grinning`], deux tours).
    Grin,
    /// Assise : un éclat ([`sheets::glitching`]). Aussi celui avec lequel elle se rassoit.
    PerchGlitch,
    /// Allongée : un éclat ([`lie::glitching`]). Aussi celui avec lequel elle s'allonge.
    LieGlitch,
}

impl Gesture {
    pub const ALL: [Gesture; 4] = [Gesture::Rise, Gesture::Grin, Gesture::PerchGlitch, Gesture::LieGlitch];

    /// Combien d'images il dure, joué jusqu'au bout.
    pub fn frames(self) -> u64 {
        match self {
            Gesture::Rise => u64::from(life::Strip::Glitch.count() * life::Strip::Glitch.hold()),
            Gesture::Grin => life::grin_frames(),
            Gesture::PerchGlitch => u64::from(sheets::Strip::PerchGlitch.count() * sheets::HOLD),
            Gesture::LieGlitch => u64::from(lie::Strip::LieGlitch.count() * sheets::HOLD),
        }
    }

    /// Se joue-t-il sur son bord (assise ou allongée) ? Son tour ne s'arrête pas alors.
    pub fn seated(self) -> bool {
        matches!(self, Gesture::PerchGlitch | Gesture::LieGlitch)
    }

    /// Pour le journal.
    pub fn describe(self) -> &'static str {
        match self {
            Gesture::Rise => "elle se relève",
            Gesture::Grin => "elle sourit",
            Gesture::PerchGlitch => "un éclat, assise",
            Gesture::LieGlitch => "un éclat, allongée",
        }
    }
}

/// Une envie de se reposer, ses planches pas encore là, reste une envie pendant tant d'images
/// (2 s) : `roam` la redit à chaque pas tant qu'elle tient ; ensuite, l'app peut reprendre ses
/// planches.
pub const WISH: u64 = 48;

/// Son repos, vu du dehors (voir le module).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rest {
    /// Éveillée, ni allongée ni endormie.
    Awake,
    /// Allongée sur son bord, éveillée (en train de s'allonger compris).
    Lying,
    /// Endormie, allongée ou debout.
    Asleep { lying: bool },
    /// Debout, elle se réveille.
    Waking,
}

/// Pourquoi son repos allongée s'est arrêté sans que personne ne s'occupe d'elle
/// ([`Look::take_unsettled`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsettled {
    /// Plus la place pour elle allongée là où elle est (son bord raccourci ou recouvert, l'écran) :
    /// elle se rassoit.
    NoRoom,
    /// Elle n'est plus posée sur son bord (sa fenêtre partie, recouverte, « S'asseoir sur les
    /// fenêtres » décochée) : elle s'en efface allongée, puis flotte.
    LedgeGone,
    /// Ses planches allongées reprises (ce que l'app ne fait pas tant qu'elle en veut) : elle se
    /// rassoit.
    Undressed,
}

impl Unsettled {
    /// Pour le journal.
    pub fn describe(self) -> &'static str {
        match self {
            Unsettled::NoRoom => "plus la place pour elle allongée",
            Unsettled::LedgeGone => "elle n'est plus posée sur son bord",
            Unsettled::Undressed => "ses planches allongées reprises",
        }
    }
}

/// Les jeux de planches qu'elle charge à la demande : sa pose allongée ([`lie`]), son sommeil et
/// son réveil debout ([`life::Strip::NAP`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wants {
    pub lying: bool,
    pub sleeping: bool,
}

/// Les masques de clic de sa pose allongée, chargée à la demande : l'union de ses boucles et de son
/// clignement (cases étroites), et celle de son éclat et de son départ (cases larges).
pub struct LieMasks {
    pub narrow: AlphaMask,
    pub wide: AlphaMask,
}

/// Les masques de clic de son sommeil debout et de son réveil, chargés à la demande.
pub struct NapMasks {
    pub sleep: AlphaMask,
    pub wake: AlphaMask,
}

/// Où en est son repos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Repose {
    Awake,
    /// Allongée sur son bord, éveillée : sa boucle allongée, au dessin 0 à l'image `since` (son
    /// éclat d'abord, s'il se joue : [`Gesture::LieGlitch`]).
    Lying {
        since: u64,
    },
    /// Allongée, endormie : sa boucle de sommeil, au dessin 0 à l'image `since`.
    Sleeping {
        since: u64,
    },
    /// Debout, endormie depuis l'image `since`, son tour arrêté.
    Dozing {
        since: u64,
    },
    /// Debout, elle se réveille depuis l'image `since` ; son tour reprend à la fin.
    Waking {
        since: u64,
    },
}

/// De combien de pixels de ses planches son masque de clic est dilaté, dans chaque sens (un carré :
/// la marge de l'app, `GRAB_MARGIN`, pour attraper ses membres fins ; un essai de l'app vérifie
/// qu'elles sont égales).
pub const MASK_GROW: f32 = 3.0;

/// Guettant, rien ne l'attrape à moins de tant de pixels de ses planches du bord de sa fenêtre,
/// dehors : la dilatation de son masque ([`MASK_GROW`], un carré), penchée de `angle`, porte au
/// plus de `MASK_GROW · (|cos| + |sin|)` en largeur (3,9 px à 22°) ; ce qui est derrière la
/// fenêtre, juste au bord, ne doit pas déborder dehors. L'essai des vraies planches de l'app la
/// compare à la vraie portée.
pub fn lurk_edge(angle: f32) -> f32 {
    MASK_GROW * (angle.cos().abs() + angle.sin().abs())
}

/// Le plus long de ses gestes, en images : son sourire, 4 s. Aucun ne dure plus.
pub const LONGEST_GESTURE: u64 = 96;

/// Un dessin d'elle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Drawing {
    /// Son dessin d'avant, entier (`capri-cutout.png`).
    Cutout,
    /// Une case de sa pose debout.
    Stand(stand::Cell),
    /// Une case de sa pose assise.
    Perch(sheets::Cell),
    /// Une case de sa pose allongée (chargée à la demande).
    Lie(lie::Cell),
    /// Une case de ses planches de vie (elle apparaît, elle s'efface), sur la toile de sa boucle
    /// debout.
    Life(life::Cell),
    /// Elle se rassemble (une intro) : la pose clé n° `key` de `intro::act::GATHER`, ses bandes
    /// étirées vers les bords de l'écran (`intro::act::bands`), sa toile en `rect`.
    Gather(u8),
}

/// Ce qu'on dessine à une image donnée.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    pub drawing: Drawing,
    /// Où, et à quelle taille (pixels entiers).
    pub rect: Rect,
    /// Sa bouche, posée par-dessus, dans le même rectangle.
    pub mouth: Option<stand::Cell>,
    /// Le trait qui « bout » et la copie violette : pour son dessin d'avant seulement (les planches
    /// sont nettes, et ont déjà leur liseré).
    pub boil_seed: u32,
    pub ghost: Vec2,
    /// Elle guette : penchée et coupée net au bord de sa fenêtre, sa bouche avec elle ; `None` :
    /// dessinée telle quelle.
    pub lean: Option<Lean>,
}

/// Guettant, comment son dessin se montre, calculé à l'affichage (ses images ne sont jamais
/// modifiées) : tourné de `angle` radians autour de `pivot` (sa hanche, en pixels de l'overlay ; à
/// l'écran, positif dans le sens des aiguilles d'une montre), et seul ce qui est dehors du bord de
/// sa fenêtre, en x = `wall`, se voit (à gauche du bord sur le côté gauche, à droite sur le droit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lean {
    pub angle: f32,
    pub pivot: Vec2,
    pub wall: f32,
    pub side: Side,
}

impl Lean {
    /// Le point `p` de l'écran se voit-il (est-il dehors du bord) ?
    pub fn shows(&self, p: Vec2) -> bool {
        match self.side {
            Side::Left => p.x < self.wall,
            Side::Right => p.x >= self.wall,
        }
    }

    /// Le point de son image droite qui se dessine en `p` (le dessin redressé autour de sa hanche).
    pub fn upright(&self, p: Vec2) -> Vec2 {
        stand::turn(p, self.pivot, -self.angle)
    }

    /// Où se dessine le point `p` de son image droite.
    pub fn turned(&self, p: Vec2) -> Vec2 {
        stand::turn(p, self.pivot, self.angle)
    }
}

/// Les masques de clic de sa pose assise : l'union des quatre boucles (cases étroites), et celle
/// de l'arrivée et du départ (cases larges).
pub struct PerchMasks {
    pub narrow: AlphaMask,
    pub wide: AlphaMask,
}

/// Ce dont elle dispose : son dessin d'avant (toujours là), et les planches chargées, chacune
/// avec le masque de clic de sa pose (l'union des dessins de sa boucle).
pub struct Wardrobe {
    pub cutout: AlphaMask,
    /// Sa pose debout : l'union de la boucle et du clignement.
    pub stand: Option<AlphaMask>,
    pub perch: Option<PerchMasks>,
    /// Ses planches de vie : l'union des dessins de son arrivée (rien ne l'attrape quand elle
    /// s'efface).
    pub life: Option<AlphaMask>,
    /// Ses gestes et ses variantes, chacun facultatif.
    pub moves: Moves,
}

/// Ses gestes et ses variantes : chaque planche est là ou non, une à une (voir le module). Celles
/// de sa vie, de 240 px, ont chacune le masque de clic de ses dessins (l'union de ses cases) ;
/// celles de ses poses prennent le masque de leur pose.
#[derive(Default)]
pub struct Moves {
    /// Elle marche.
    pub walk: Option<AlphaMask>,
    /// Tenue ou lancée, elle pend.
    pub dangle: Option<AlphaMask>,
    /// Lâchée, elle se relève.
    pub glitch: Option<AlphaMask>,
    /// Elle sourit.
    pub grin: Option<AlphaMask>,
    /// Assise, un éclat (dans des cases larges : le masque des cases larges de sa pose assise).
    pub perch_glitch: bool,
    /// Les variantes glitchées de ses clignements, assise et debout.
    pub perch_blink_glitch: bool,
    pub stand_blink_glitch: bool,
}

/// Une suite jouée une fois pour apparaître ou s'effacer : ses planches de vie, par-dessus son
/// image debout, ou celles de sa pose assise, posée sur le bord en `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Clip {
    Arrive,
    Leave,
    PerchArrive {
        at: Vec2,
    },
    PerchLeave {
        at: Vec2,
    },
    /// Elle s'efface de son bord allongée ([`lie::leaving`]) ; à l'envers, elle y réapparaît
    /// allongée.
    LieLeave {
        at: Vec2,
    },
    /// Guettant, elle sort de derrière sa fenêtre (à l'endroit) ou s'y retire (à l'envers) : sa
    /// position glisse (`Capri::emerge`, `Capri::withdraw`), ses dessins restent sa boucle debout ;
    /// c'est son corps qui dit quand c'est fini, pas une feuille d'exposition.
    Slide,
}

/// Guettant, sa glissade, à l'image `frame` : elle sort de derrière sa fenêtre (`out`) ou s'y
/// retire, d'où elle en est (son corps glisse ; voir [`Clip::Slide`]).
fn slide(capri: &mut Capri, out: bool, frame: u64) -> Run {
    if out {
        capri.emerge();
    } else {
        capri.withdraw();
    }
    Run { clip: Clip::Slide, since: frame, from: 0, len: 1, backward: !out }
}

/// Une suite jouée une fois, dans un sens ou dans l'autre : à l'image d'animation `since`, elle en
/// est à l'image `from` de sa feuille d'exposition (`len` images) ; à l'envers (`backward`), elle
/// revient sur ses pas jusqu'à l'image 0.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Run {
    clip: Clip,
    since: u64,
    from: u64,
    len: u64,
    backward: bool,
}

impl Run {
    /// Depuis le début, à l'endroit, à partir de l'image `since`.
    fn new(clip: Clip, since: u64, len: u64) -> Self {
        Self { clip, since, from: 0, len, backward: false }
    }

    /// L'image de sa feuille montrée à l'image d'animation `frame`.
    fn position(self, frame: u64) -> u64 {
        let elapsed = frame.saturating_sub(self.since);
        if self.backward {
            self.from.saturating_sub(elapsed)
        } else {
            (self.from + elapsed).min(self.len.saturating_sub(1))
        }
    }

    /// Combien d'images il lui reste à l'image `frame` ; 0 : finie.
    fn left(self, frame: u64) -> u64 {
        let elapsed = frame.saturating_sub(self.since);
        if self.backward {
            (self.from + 1).saturating_sub(elapsed)
        } else {
            self.len.saturating_sub(self.from + elapsed)
        }
    }

    /// La même, repartie dans l'autre sens à l'image `frame`, du dessin où elle en est.
    fn turned(self, frame: u64) -> Self {
        Self { since: frame, from: self.position(frame), backward: !self.backward, ..self }
    }

    /// Assise : là où elle est posée.
    fn seat(self) -> Option<Vec2> {
        match self.clip {
            Clip::PerchArrive { at } | Clip::PerchLeave { at } | Clip::LieLeave { at } => Some(at),
            Clip::Arrive | Clip::Leave | Clip::Slide => None,
        }
    }

    /// Allongée.
    fn lying(self) -> bool {
        matches!(self.clip, Clip::LieLeave { .. })
    }
}

/// Pourquoi elle s'efface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Exit {
    /// On la cache : une fois partie, la fenêtre peut se cacher.
    Hidden,
    /// Elle change de place : la fenêtre reste montrée ; une fois effacée, on la déplace, puis
    /// elle apparaît là-bas ([`Look::arrive`]).
    Trip,
    /// « Voir une intro » : la fenêtre reste montrée ; une fois effacée ([`Look::is_offstage`]),
    /// une intro se joue, puis elle revient.
    Encore,
}

/// Est-elle là ?
#[derive(Clone, Copy, Debug, PartialEq)]
enum Presence {
    /// Là : ses dessins de tous les jours.
    Here,
    /// Elle apparaît.
    Appearing(Run),
    /// Elle s'efface.
    Vanishing(Run, Exit),
    /// Partie, après la suite qui l'a effacée (aucune, sans ses planches de vie) : plus rien à
    /// montrer.
    Gone(Option<Run>, Exit),
    /// Elle joue une intro (voir [`Look::perform`]).
    Entering(Entrance),
}

/// Une intro qu'elle joue elle-même (`crate::intro::act`) : laquelle, depuis quelle image, sa place
/// (ses pieds, là où elle finit), et où elle en est.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entrance {
    act: Act,
    since: u64,
    place: Vec2,
    /// Sa hauteur debout quand elle l'a commencée : une autre taille la coupe.
    height: f32,
    stage: Stage,
}

/// Où en est une intro qu'elle joue.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    /// Elle tombe du ciel, en pendant (`Stance::Falling`).
    Falling,
    /// Elle se matérialise là où sa marche commence.
    Appearing(Run),
    /// Elle marche jusqu'à sa place, depuis l'image `since`.
    Walking { since: u64 },
    /// Ses bandes se resserrent en elle.
    Gathering,
}

/// Où en est sa pose assise.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seat {
    /// Debout : rien de sa pose assise à montrer.
    Standing,
    /// Elle apparaît sur le bord depuis l'image `since`.
    Arriving { since: u64 },
    /// Assise ; sa boucle est au dessin 0 à l'image `since`.
    Sitting { since: u64 },
    /// Elle s'efface depuis l'image `since`, là où elle était assise (`lying` : allongée).
    Leaving { since: u64, at: Vec2, lying: bool },
}

/// Ses feuilles d'exposition, faites une fois.
struct Sheets {
    standing: Sheet<stand::Cell>,
    standing_blink: Sheet<stand::Cell>,
    standing_glitchy: Sheet<stand::Cell>,
    mouth: Sheet<stand::Cell>,
    sitting: Sheet<sheets::Cell>,
    sitting_blink: Sheet<sheets::Cell>,
    sitting_glitchy: Sheet<sheets::Cell>,
    talking: Sheet<sheets::Cell>,
    music: Sheet<sheets::Cell>,
    arriving: Sheet<sheets::Cell>,
    leaving: Sheet<sheets::Cell>,
    perch_glitch: Sheet<sheets::Cell>,
    life_arriving: Sheet<life::Cell>,
    life_leaving: Sheet<life::Cell>,
    walk: Sheet<life::Cell>,
    dangle: Sheet<life::Cell>,
    rise: Sheet<life::Cell>,
    grin: Sheet<life::Cell>,
    lying: Sheet<lie::Cell>,
    lying_blink: Sheet<lie::Cell>,
    lying_talk: Sheet<lie::Cell>,
    lying_asleep: Sheet<lie::Cell>,
    lying_glitch: Sheet<lie::Cell>,
    lying_leave: Sheet<lie::Cell>,
    nap: Sheet<life::Cell>,
    wake: Sheet<life::Cell>,
}

pub struct Look {
    wardrobe: Wardrobe,
    /// Sa hauteur debout, en pixels de l'overlay.
    height: f32,
    /// Apparaître et s'effacer.
    presence: Presence,
    seat: Seat,
    /// Le point posé sur le bord à la dernière mise à jour où elle l'était : là où elle s'efface.
    last_seat: Option<Vec2>,
    frame: u64,
    talking: bool,
    /// Assise, elle se balance (la musique se prend et se quitte au début d'un tour).
    grooving: bool,
    /// De la musique joue (que sa boucle l'ait déjà prise ou pas encore).
    music: bool,
    /// On se parle (au micro ou par écrit ; elle parle, répond ou va répondre) : un geste s'arrête,
    /// et rien ne commence ([`Look::set_conversing`]).
    conversing: bool,
    /// Le geste en cours, et l'image où il a commencé.
    act: Option<(Gesture, u64)>,
    /// Depuis quelle image on la tient (ou qu'elle vole) : le tour de sa planche en l'air.
    carried: Option<u64>,
    /// Depuis quelle image elle marche.
    walking: Option<u64>,
    /// La dernière fois qu'elle a souri parce que Claude Code finissait son tour.
    cheered: Option<u64>,
    /// Comment a fini la dernière intro jouée par elle, pas encore relevé ([`Look::take_performed`]).
    performed: Option<End>,
    /// Son bonjour attend d'être dit : pas de geste d'elle-même ([`Look::set_greeting_due`]).
    greeting_due: bool,
    /// Les jeux de planches de repos qui existent (leurs en-têtes lus au lancement : [`Look::offer`]).
    offered: Wants,
    /// Ses planches de repos, chargées (avec leurs masques de clic), ou non.
    lie: Option<LieMasks>,
    nap: Option<NapMasks>,
    /// Il y a la place pour elle allongée, là où elle est posée ([`Look::set_room_to_lie`]).
    room_to_lie: bool,
    /// Son repos.
    repose: Repose,
    /// Elle voudrait s'allonger, ou s'endormir debout, ses planches pas encore là : depuis l'image.
    lie_wish: Option<u64>,
    nap_wish: Option<u64>,
    /// On s'est occupé d'elle (un clic, un réveil) depuis que `roam` l'a demandé
    /// ([`Look::take_attention`]).
    attended: bool,
    /// Son repos allongée s'est arrêté sans qu'on s'occupe d'elle, et endormie ou non, depuis que
    /// `roam` l'a demandé ([`Look::take_unsettled`]).
    unsettled: Option<(Unsettled, bool)>,
    sheets: Sheets,
}

impl Look {
    /// `height` : sa hauteur debout, en pixels de l'overlay. Elle est là tout de suite ; pour
    /// qu'elle apparaisse, [`Look::enter`].
    pub fn new(wardrobe: Wardrobe, height: f32) -> Self {
        Self {
            wardrobe,
            height,
            presence: Presence::Here,
            seat: Seat::Standing,
            last_seat: None,
            frame: 0,
            talking: false,
            grooving: false,
            music: false,
            conversing: false,
            act: None,
            carried: None,
            walking: None,
            cheered: None,
            performed: None,
            greeting_due: false,
            offered: Wants::default(),
            lie: None,
            nap: None,
            room_to_lie: false,
            repose: Repose::Awake,
            lie_wish: None,
            nap_wish: None,
            attended: false,
            unsettled: None,
            sheets: Sheets {
                standing: stand::standing(false),
                standing_blink: stand::standing(true),
                standing_glitchy: stand::standing_glitchy(),
                mouth: stand::talking(),
                sitting: sheets::sitting(false),
                sitting_blink: sheets::sitting(true),
                sitting_glitchy: sheets::sitting_glitchy(),
                talking: sheets::talking(),
                music: sheets::music(),
                arriving: sheets::arriving(),
                leaving: sheets::leaving(),
                perch_glitch: sheets::glitching(),
                life_arriving: life::arriving(),
                life_leaving: life::leaving(),
                walk: life::walking(),
                dangle: life::dangling(),
                rise: life::rising(),
                grin: life::grinning(),
                lying: lie::lying(false),
                lying_blink: lie::lying(true),
                lying_talk: lie::talking(),
                lying_asleep: lie::sleeping(),
                lying_glitch: lie::glitching(),
                lying_leave: lie::leaving(),
                nap: life::sleeping(),
                wake: life::waking(),
            },
        }
    }

    /// Ses planches de vie sont-elles là, avec sa pose debout sur laquelle elles s'alignent ?
    /// Sinon, elle apparaît et disparaît d'un coup.
    pub fn has_life(&self) -> bool {
        self.wardrobe.life.is_some() && self.has_stand()
    }

    /// Elle entre en scène (au lancement), à l'image `frame` : elle apparaît, debout, ou assise si
    /// elle est posée.
    pub fn enter(&mut self, capri: &mut Capri, frame: u64) {
        self.frame = self.frame.max(frame);
        // une intro qu'elle jouait s'arrête là, comme quand on lui parle
        if let Presence::Entering(entrance) = self.presence {
            self.finish(entrance, Some(intro::Cut::Talked), frame);
        }
        self.leave_stage(capri);
        self.presence = self.entrance(capri, frame);
    }

    /// On la montre, à l'image `frame` : cachée, elle apparaît ([`Look::enter`]) ; en train de
    /// s'effacer, elle revient sur ses pas. `capri` doit être à jour (sa fenêtre relue) : c'est
    /// lui qui dit si elle réapparaît assise.
    pub fn appear(&mut self, capri: &mut Capri, frame: u64) {
        self.frame = self.frame.max(frame);
        // pas cachée : là, ou en chemin vers ailleurs
        if self.is_hidden() {
            self.reverse(capri, frame);
        }
    }

    /// En chemin vers une autre place (on lui parle pendant qu'elle s'efface pour partir), ou en
    /// train de s'effacer pour une intro (on lui parle avant qu'elle ne commence), elle revient sur
    /// ses pas à l'image `frame`, comme montrée pendant qu'elle s'efface : la même suite, à
    /// l'envers, là où elle était ; effacée, elle y réapparaît. Sans effet sinon.
    pub fn turn_back(&mut self, capri: &mut Capri, frame: u64) {
        self.frame = self.frame.max(frame);
        if self.is_traveling() || self.is_bowing_out() {
            self.reverse(capri, frame);
        }
    }

    /// Elle revient sur ses pas, là où elle était : la suite qui l'efface, repartie à l'envers du
    /// dessin où elle en est ; partie, elle réapparaît.
    fn reverse(&mut self, capri: &mut Capri, frame: u64) {
        self.presence = match self.presence {
            Presence::Here | Presence::Appearing(_) | Presence::Entering(_) => return,
            Presence::Gone(..) => self.entrance(capri, frame),
            // elle s'effaçait de son bord, qu'elle a quitté depuis : elle se matérialise debout
            Presence::Vanishing(run, _) if run.seat().is_some() && capri.seat().is_none() => {
                self.entrance(capri, frame)
            }
            // elle se retirait derrière sa fenêtre : elle en ressort, d'où elle en est ; plus sur son
            // côté (son côté parti), elle apparaît comme d'habitude
            Presence::Vanishing(run, _) if run.clip == Clip::Slide => match capri.lurking() {
                Some(_) => Presence::Appearing(slide(capri, true, frame)),
                None => self.entrance(capri, frame),
            },
            Presence::Vanishing(run, _) => {
                let run = run.turned(frame);
                self.resume_after(capri, run, frame);
                // elle s'effaçait allongée : elle y réapparaît allongée, et le reste
                if run.lying() && self.lie.is_some() {
                    self.repose = Repose::Lying { since: frame + run.left(frame) };
                }
                Presence::Appearing(run)
            }
        };
    }

    /// On la cache, à l'image `frame` : elle s'efface, de là où elle est dessinée ; en train
    /// d'apparaître, elle revient sur ses pas ; en train de s'effacer pour changer de place, elle
    /// finit de s'effacer là où elle était, et n'ira pas plus loin. Sans ses planches de vie, elle
    /// est partie tout de suite ([`Look::is_gone`]).
    pub fn vanish(&mut self, capri: &mut Capri, frame: u64) {
        let picture = self.picture(capri);
        self.frame = self.frame.max(frame);
        if self.is_here() {
            // son geste s'arrête, sa marche aussi : elle s'efface de là où elle est
            self.stop(capri);
        }
        self.presence = match self.presence {
            Presence::Vanishing(_, Exit::Hidden) | Presence::Gone(_, Exit::Hidden) => return,
            Presence::Vanishing(run, Exit::Trip | Exit::Encore) => Presence::Vanishing(run, Exit::Hidden),
            Presence::Gone(run, Exit::Trip | Exit::Encore) => Presence::Gone(run, Exit::Hidden),
            // une intro qu'elle joue : coupée net, et elle est cachée
            Presence::Entering(entrance) => {
                self.finish(entrance, Some(intro::Cut::Hidden), frame);
                self.leave_stage(capri);
                Presence::Gone(None, Exit::Hidden)
            }
            // elle sortait de derrière sa fenêtre : elle s'y retire, d'où elle en est
            Presence::Appearing(run) if run.clip == Clip::Slide => {
                Presence::Vanishing(slide(capri, false, frame), Exit::Hidden)
            }
            Presence::Appearing(run) => {
                if run.seat().is_none() {
                    capri.still(None);
                }
                Presence::Vanishing(run.turned(frame), Exit::Hidden)
            }
            Presence::Here => match self.leaving(capri, picture.drawing, frame) {
                Some(run) => Presence::Vanishing(run, Exit::Hidden),
                None => Presence::Gone(None, Exit::Hidden),
            },
        };
    }

    /// Cachée avant d'être apparue (l'intro du lancement coupée en la cachant, voir
    /// [`crate::intro`]) : partie d'un coup à l'image `frame`, sans rien jouer. Montrée ensuite
    /// ([`Look::appear`]), elle se matérialise comme au lancement.
    pub fn hide_unseen(&mut self, capri: &mut Capri, frame: u64) {
        self.frame = self.frame.max(frame);
        self.act = None;
        self.wake_up(capri);
        if let Presence::Entering(entrance) = self.presence {
            self.finish(entrance, Some(intro::Cut::Hidden), frame);
            self.leave_stage(capri);
        }
        self.presence = Presence::Gone(None, Exit::Hidden);
    }

    /// Elle change de place, à l'image `frame` : elle s'efface de là où elle est dessinée, comme
    /// cachée, mais la fenêtre reste montrée. Une fois effacée ([`Look::is_between`]), on la
    /// déplace, puis elle apparaît là-bas ([`Look::arrive`]). Seulement si elle est là
    /// ([`Look::is_here`]) : rend `false` sinon, et rien ne change. Sans ses planches de vie, elle
    /// est effacée tout de suite.
    pub fn travel(&mut self, capri: &mut Capri, frame: u64) -> bool {
        if !self.is_here() {
            return false;
        }
        let picture = self.picture(capri);
        self.frame = self.frame.max(frame);
        self.stop(capri);
        self.presence = match self.leaving(capri, picture.drawing, frame) {
            Some(run) => Presence::Vanishing(run, Exit::Trip),
            None => Presence::Gone(None, Exit::Trip),
        };
        true
    }

    /// Effacée pour changer de place ([`Look::is_between`]), elle apparaît à l'image `frame` là où
    /// `capri` est maintenant : assise si elle y est posée (avec ses planches assises), debout
    /// sinon ; d'un coup sans ses planches de vie. Sans effet si elle n'est pas entre deux places.
    pub fn arrive(&mut self, capri: &mut Capri, frame: u64) {
        if !self.is_between() {
            return;
        }
        self.frame = self.frame.max(frame);
        self.act = None;
        self.walking = None;
        self.wake_up(capri);
        // ce qu'elle faisait là où elle était ne compte plus : ni un départ de son bord à finir,
        // ni une boucle assise à reprendre
        match capri.seat().filter(|_| self.has_perch()) {
            Some(at) => {
                self.last_seat = Some(at);
                self.seat = Seat::Sitting { since: frame };
            }
            None => self.seat = Seat::Standing,
        }
        self.presence = self.entrance(capri, frame);
    }

    /// Est-elle là, ni en train d'apparaître ni de s'effacer ? Sinon, elle n'est pas libre : ce
    /// qui attend son tour au-dessus de sa tête attend encore.
    pub fn is_here(&self) -> bool {
        self.presence == Presence::Here
    }

    /// S'efface-t-elle, ou est-elle partie, cachée ou en chemin ? Ce qu'elle dit attend son
    /// retour, et rien ne l'attrape.
    pub fn is_away(&self) -> bool {
        matches!(self.presence, Presence::Vanishing(..) | Presence::Gone(..))
    }

    /// Est-elle partie, cachée : plus rien à montrer, la fenêtre peut se cacher. Jamais quand elle
    /// change de place.
    pub fn is_gone(&self) -> bool {
        matches!(self.presence, Presence::Gone(_, Exit::Hidden))
    }

    /// Cachée : elle s'efface parce qu'on la cache, ou elle est partie.
    pub fn is_hidden(&self) -> bool {
        matches!(self.presence, Presence::Vanishing(_, Exit::Hidden) | Presence::Gone(_, Exit::Hidden))
    }

    /// En chemin vers une autre place : elle s'efface pour y aller, ou elle est effacée et attend
    /// d'être déplacée.
    pub fn is_traveling(&self) -> bool {
        matches!(self.presence, Presence::Vanishing(_, Exit::Trip) | Presence::Gone(_, Exit::Trip))
    }

    /// Effacée pour changer de place : il est temps de la déplacer, puis de la faire apparaître
    /// ([`Look::arrive`]).
    pub fn is_between(&self) -> bool {
        matches!(self.presence, Presence::Gone(_, Exit::Trip))
    }

    /// « Voir une intro », à l'image `frame` : elle s'efface de là où elle est dessinée, comme
    /// cachée, mais la fenêtre reste montrée ; une fois effacée ([`Look::is_offstage`]), la suite
    /// se joue : le film, puis l'entrée ([`Look::perform`], ou `Look::enter` pour « se
    /// matérialise », ou le film coupé en lui parlant). Seulement
    /// si elle est là et libre : ni tenue ni lancée, ni en marche, et on ne se parle pas
    /// ([`Look::set_conversing`], à dire juste avant) ; sinon rend
    /// ce qui la retient, et rien ne change. Un geste en cours s'arrête. Sans ses planches de vie,
    /// elle est effacée tout de suite. Cachée ensuite, elle finit de s'effacer et n'ira pas plus
    /// loin ; on lui parle avant que l'intro ne commence, elle revient sur ses pas
    /// ([`Look::turn_back`]).
    pub fn bow_out(&mut self, capri: &mut Capri, frame: u64) -> Result<(), Hold> {
        if carried(capri) {
            return Err(Hold::Carried);
        }
        if !self.is_here() {
            return Err(Hold::Away);
        }
        if self.conversing {
            return Err(Hold::Talking);
        }
        if capri.is_walking() {
            return Err(Hold::Walking);
        }
        let picture = self.picture(capri);
        self.frame = self.frame.max(frame);
        self.stop(capri);
        self.presence = match self.leaving(capri, picture.drawing, frame) {
            Some(run) => Presence::Vanishing(run, Exit::Encore),
            None => Presence::Gone(None, Exit::Encore),
        };
        Ok(())
    }

    /// S'efface-t-elle pour une intro (« Voir une intro »), ou est-elle effacée et l'attend-elle ?
    pub fn is_bowing_out(&self) -> bool {
        matches!(self.presence, Presence::Vanishing(_, Exit::Encore) | Presence::Gone(_, Exit::Encore))
    }

    /// Effacée pour une intro : il est temps de la jouer.
    pub fn is_offstage(&self) -> bool {
        matches!(self.presence, Presence::Gone(_, Exit::Encore))
    }

    /// Peut-elle jouer cette intro, d'où elle est (`capri`) ? Tomber du ciel : sa planche en l'air
    /// et se relever ; arriver en marchant : sa marche, et assez de place pour marcher au moins
    /// `act::WALK_SHORTEST` jusqu'à sa place ; se rassembler : son arrivée.
    pub fn can_perform(&self, act: Act, capri: &Capri) -> bool {
        self.lacks(act, capri).is_none()
    }

    /// Ce qui lui manque pour jouer cette intro d'où elle est : ses planches
    /// (`Missing::Absent { found: 0 }`), ou la place (`Missing::NoRoom`).
    fn lacks(&self, act: Act, capri: &Capri) -> Option<intro::Missing> {
        let sheets = match act {
            Act::Fall => self.has_move(life::Strip::Dangle) && self.has_move(life::Strip::Glitch),
            Act::Walk => self.has_move(life::Strip::Walk),
            Act::Gather => self.has_life(),
        };
        if !sheets {
            return Some(intro::Missing::Absent { found: 0 });
        }
        (act == Act::Walk && self.walk_in(capri).is_none()).then_some(intro::Missing::NoRoom)
    }

    /// Pourquoi [`Look::perform`] refuserait cette intro maintenant : ses planches lui manquent
    /// (`Missing::Absent { found: 0 }`), ou la place d'où elle est (`Missing::NoRoom`), ou elle
    /// n'est pas prête (`Missing::NotReady` : ni à son entrée en scène ni effacée pour une intro,
    /// tenue, en marche) ; `None` si elle la jouerait. Pour que le journal dise la vraie raison.
    pub fn cannot_perform(&self, act: Act, capri: &Capri) -> Option<intro::Missing> {
        let ready = matches!(self.presence, Presence::Here | Presence::Gone(_, Exit::Encore));
        self.lacks(act, capri)
            .or_else(|| (!ready || carried(capri) || capri.is_walking()).then_some(intro::Missing::NotReady))
    }

    /// D'où elle part pour arriver en marchant à sa place (ses pieds, debout, dans l'écran), et
    /// si elle s'y matérialise d'abord (voir `act::walk_from`) ; `None` sans sa marche, ou si cette
    /// intro n'est pas possible depuis là.
    fn walk_in(&self, capri: &Capri) -> Option<(f32, bool)> {
        let speed = self.walk_speed()?;
        let mut probe = capri.clone();
        probe.set_at(probe.feet());
        let place = probe.feet();
        // sa toile de vie commence là, à gauche de ses pieds, et finit là, à leur droite
        let k = probe.size().x / stand::CELL_WIDTH as f32;
        let left = probe.size().x * self.standing_feet().x + life::STAND_LEFT as f32 * k;
        let right = life::CELL_WIDTH as f32 * k - left;
        let (offscreen, inside) =
            (act::offscreen_right(probe.bounds(), left), act::inside_right(probe.bounds(), right));
        act::walk_from(place.x, offscreen, inside, speed)
    }

    /// Elle joue l'intro `act` à l'image `frame` (voir `crate::intro::act`), vers sa place : là où
    /// sont ses pieds, debout (posée, le point posé sur son bord). Seulement à son entrée en scène
    /// (au lancement, pas encore montrée) ou effacée pour une intro ([`Look::is_offstage`]), et si
    /// ses planches le permettent ([`Look::can_perform`]) : rend `false` sinon, et rien ne change.
    /// Elle n'est pas là tant qu'elle la joue ([`Look::is_performing`]) ; l'attraper la coupe (elle
    /// est tenue), la cacher la coupe net ([`Look::vanish`]), lui parler aussi
    /// ([`Look::cut_performance`]). Comment elle a fini : [`Look::take_performed`].
    pub fn perform(&mut self, capri: &mut Capri, act: Act, frame: u64) -> bool {
        let ready = matches!(self.presence, Presence::Here | Presence::Gone(_, Exit::Encore));
        if !ready || !self.can_perform(act, capri) || carried(capri) || capri.is_walking() {
            return false;
        }
        self.frame = self.frame.max(frame);
        self.act = None;
        self.walking = None;
        self.wake_up(capri);
        self.seat = Seat::Standing;
        // debout là où sont ses pieds : sa place, dans l'écran
        capri.set_at(capri.feet());
        let place = capri.feet();
        capri.still(None);
        let stage = match act {
            Act::Fall => {
                let from = act::fall_from(place, capri.size(), self.standing_feet(), capri.bounds());
                capri.fall(from, place.y);
                Stage::Falling
            }
            Act::Walk => {
                let speed = self.walk_speed().unwrap_or(0.0);
                // `can_perform` l'a vérifié, d'ici
                let Some((x, appears)) = self.walk_in(capri) else { return false };
                capri.set_beyond(Vec2::new(x, place.y));
                if appears && self.has_life() {
                    Stage::Appearing(Run::new(Clip::Arrive, frame, self.sheets.life_arriving.len()))
                } else {
                    capri.walk_to(place.x, speed);
                    self.walking = Some(frame);
                    Stage::Walking { since: frame }
                }
            }
            Act::Gather => Stage::Gathering,
        };
        self.presence = Presence::Entering(Entrance { act, since: frame, place, height: self.height, stage });
        true
    }

    /// Joue-t-elle une intro ([`Look::perform`]) ?
    pub fn is_performing(&self) -> bool {
        matches!(self.presence, Presence::Entering(_))
    }

    /// L'intro qu'elle joue, s'il y en a une.
    pub fn performing(&self) -> Option<Act> {
        match self.presence {
            Presence::Entering(entrance) => Some(entrance.act),
            _ => None,
        }
    }

    /// Comment a fini l'intro qu'elle jouait, une fois : jouée jusqu'au bout, ou coupée (en images
    /// d'animation depuis son début, voir `crate::intro::End`).
    pub fn take_performed(&mut self) -> Option<End> {
        self.performed.take()
    }

    /// On lui parle pendant une intro qu'elle joue, à l'image `frame` : elle s'arrête, et elle se
    /// matérialise tout de suite à sa place. Sans effet si elle n'en joue pas.
    pub fn cut_performance(&mut self, capri: &mut Capri, frame: u64) {
        let Presence::Entering(entrance) = self.presence else { return };
        self.frame = self.frame.max(frame);
        self.finish(entrance, Some(intro::Cut::Talked), frame);
        self.leave_stage(capri);
        self.presence = self.entrance(capri, frame);
    }

    /// Retient comment l'intro `entrance` a fini, à l'image `frame` : jouée, ou coupée (`cut`).
    fn finish(&mut self, entrance: Entrance, cut: Option<intro::Cut>, frame: u64) {
        let lasted = frame.saturating_sub(entrance.since).min(u64::from(u32::MAX)) as u32;
        self.performed = Some(match cut {
            None => End::Played { shown: lasted },
            Some(why) => End::Cut { why, at: Some(lasted) },
        });
    }

    /// Une intro qu'elle jouait s'arrête là : debout à sa place (sauf tenue), sa marche ou sa
    /// chute arrêtées, son tour repris (une suite qui commence l'arrête de nouveau). Sans effet si
    /// elle n'en jouait pas ; sa présence est à l'appelant.
    fn leave_stage(&mut self, capri: &mut Capri) {
        let Presence::Entering(entrance) = self.presence else { return };
        self.walking = None;
        if !capri.is_held() {
            capri.set_at(entrance.place);
            // sa chute arrêtée n'est pas un lâcher : elle ne se relève pas
            self.carried = None;
        }
        capri.unstill();
    }

    /// Un pas d'une intro qu'elle joue, à l'image `frame`, dans `update` : ce qui suit.
    fn on_stage(&mut self, capri: &mut Capri, entrance: Entrance, frame: u64) -> Presence {
        // attrapée : tenue, comme d'habitude
        if capri.is_held() {
            self.finish(entrance, Some(intro::Cut::Grabbed), frame);
            self.walking = None;
            return Presence::Here;
        }
        // sa taille a changé (son menu, la molette, la densité de l'écran) : comme quand on lui
        // parle, elle est à sa place tout de suite
        if self.height != entrance.height {
            self.finish(entrance, Some(intro::Cut::Resized), frame);
            self.leave_stage(capri);
            return self.entrance(capri, frame);
        }
        let going = Presence::Entering(entrance);
        match entrance.stage {
            Stage::Falling if capri.is_falling() => going,
            Stage::Walking { .. } if capri.is_walking() => going,
            Stage::Appearing(run) if run.left(frame) > 0 => going,
            Stage::Gathering if frame < entrance.since + act::GATHER_FRAMES => going,
            // en bas de sa chute : son tour reprend, et elle se relève (`gestures`, comme lâchée)
            Stage::Falling => {
                self.finish(entrance, None, frame);
                capri.unstill();
                Presence::Here
            }
            // matérialisée là où sa marche commence : elle marche jusqu'à sa place, en
            // `act::WALK_LONGEST` au plus (une autre taille l'aurait coupée, plus haut)
            Stage::Appearing(_) => {
                let walks = self.walk_speed().is_some_and(|speed| capri.walk_to(entrance.place.x, speed));
                if walks {
                    self.walking = Some(frame);
                    Presence::Entering(Entrance { stage: Stage::Walking { since: frame }, ..entrance })
                } else {
                    self.finish(entrance, None, frame);
                    capri.unstill();
                    Presence::Here
                }
            }
            // arrivée (ou arrêtée par une nouvelle taille) : sa boucle reprend (`gestures`)
            Stage::Walking { .. } => {
                self.finish(entrance, None, frame);
                Presence::Here
            }
            // rassemblée : son arrivée reprend après la dernière case de ses bandes
            Stage::Gathering => {
                self.finish(entrance, None, frame);
                let run = Run {
                    clip: Clip::Arrive,
                    since: frame,
                    from: act::ARRIVE_FROM,
                    len: self.sheets.life_arriving.len(),
                    backward: false,
                };
                self.resume_after(capri, run, frame);
                Presence::Appearing(run)
            }
        }
    }

    /// Ce qu'elle dessine pendant une intro qu'elle joue.
    fn on_stage_picture(&self, capri: &Capri, entrance: Entrance) -> Picture {
        let frame = self.frame;
        match entrance.stage {
            Stage::Falling => self.alive(capri, self.sheets.dangle.at(frame.saturating_sub(entrance.since))),
            Stage::Appearing(run) => self.played(capri, run),
            Stage::Walking { since } => self.alive(capri, self.sheets.walk.at(frame.saturating_sub(since))),
            Stage::Gathering => Picture {
                drawing: Drawing::Gather(act::gather_key(frame.saturating_sub(entrance.since))),
                rect: life::over_stand(capri.pose().rect),
                mouth: None,
                boil_seed: 0,
                ghost: Vec2::ZERO,
                lean: None,
            },
        }
    }

    /// Son geste et sa marche s'arrêtent là, et son tour reprend s'ils l'avaient arrêté (s'effacer
    /// l'arrête de nouveau, voir [`Look::exit`]).
    fn stop(&mut self, capri: &mut Capri) {
        let standing = self.act.take().is_some_and(|(gesture, _)| !gesture.seated());
        let walked = self.walking.take().is_some();
        if walked {
            capri.halt();
        }
        if standing || walked {
            capri.unstill();
        }
        // son repos aussi : elle s'efface de là où elle est dessinée (allongée : de son bord)
        self.wake_up(capri);
    }

    /// Son repos s'arrête là, d'un coup : ni allongée, ni endormie, ni envie de l'être. Endormie
    /// debout, son tour reprend (une suite qui commence l'arrête de nouveau).
    fn wake_up(&mut self, capri: &mut Capri) {
        if matches!(self.repose, Repose::Dozing { .. } | Repose::Waking { .. }) {
            capri.unstill();
        }
        self.repose = Repose::Awake;
        self.lie_wish = None;
        self.nap_wish = None;
    }

    /// Comment elle apparaît : assise sur son bord si elle y est posée (avec ses planches
    /// assises), sinon debout, son tour arrêté le temps de son arrivée ; d'un coup sans ses
    /// planches de vie.
    fn entrance(&mut self, capri: &mut Capri, frame: u64) -> Presence {
        self.act = None;
        self.wake_up(capri);
        // guettant : elle sort de derrière sa fenêtre, sa boucle debout en marche (rien de ses
        // planches de vie ne se montre sur un côté)
        if capri.lurking().is_some() {
            self.seat = Seat::Standing;
            capri.unstill();
            return Presence::Appearing(slide(capri, true, frame));
        }
        if !self.has_life() {
            return Presence::Here;
        }
        let run = match capri.seat().filter(|_| self.has_perch()) {
            Some(at) => {
                self.last_seat = Some(at);
                Run::new(Clip::PerchArrive { at }, frame, self.sheets.arriving.len())
            }
            None => {
                self.seat = Seat::Standing;
                Run::new(Clip::Arrive, frame, self.sheets.life_arriving.len())
            }
        };
        self.resume_after(capri, run, frame);
        Presence::Appearing(run)
    }

    /// Ce qui reprend une fois la suite `run` finie : son tour, toujours, là où il s'était
    /// arrêté (même assise : il a pu s'arrêter pendant qu'elle s'effaçait debout, avant d'être
    /// posée, et doit tourner quand elle repartira de son bord) ; assise, sa boucle aussi, au
    /// dessin 0, que la fin de l'arrivée et le début du départ recouvrent.
    fn resume_after(&mut self, capri: &mut Capri, run: Run, frame: u64) {
        let end = frame + run.left(frame);
        capri.still(Some(end));
        if run.seat().is_some() {
            self.seat = Seat::Sitting { since: end };
        }
    }

    /// Où dessiner une suite assise commencée sur le bord en `was` : sur son bord tant qu'elle y
    /// est posée (il suit sa fenêtre), sinon là où elle l'était en dernier, comme son départ
    /// ordinaire ([`Seat::Leaving`]).
    fn run_seat(&self, capri: &Capri, was: Vec2) -> Vec2 {
        capri.seat().or(self.last_seat).unwrap_or(was)
    }

    /// Comment elle s'en va de là où elle est (`drawing` : ce qui est dessiné) : guettant, elle se
    /// retire derrière sa fenêtre ; sinon elle s'efface ([`Look::exit`]) ; sans ses planches de vie,
    /// `None` : elle est partie tout de suite.
    fn leaving(&self, capri: &mut Capri, drawing: Drawing, frame: u64) -> Option<Run> {
        if capri.lurking().is_some() {
            return Some(slide(capri, false, frame));
        }
        self.has_life().then(|| self.exit(capri, drawing, frame))
    }

    /// Comment elle s'efface, d'après ce qui est dessiné (`drawing`) : assise, de son bord (en
    /// train d'y arriver, elle revient sur ses pas ; en train d'en repartir, elle continue) ;
    /// debout, ses planches de vie, son tour arrêté.
    fn exit(&self, capri: &mut Capri, drawing: Drawing, frame: u64) -> Run {
        let (arrive, leave) = (self.sheets.arriving.len(), self.sheets.leaving.len());
        let lain = self.sheets.lying_leave.len();
        match (drawing, self.seat, self.seated_at(capri)) {
            // allongée : de son bord, allongée ; en train d'en repartir, elle continue
            (Drawing::Lie(_), Seat::Leaving { since, at, .. }, _) => {
                Run::new(Clip::LieLeave { at }, since, lain)
            }
            (Drawing::Lie(_), _, Some(at)) => Run::new(Clip::LieLeave { at }, frame, lain),
            (Drawing::Perch(_), Seat::Arriving { since }, Some(at)) => {
                Run::new(Clip::PerchArrive { at }, since, arrive).turned(frame)
            }
            (Drawing::Perch(_), Seat::Leaving { since, at, .. }, _) => {
                Run::new(Clip::PerchLeave { at }, since, leave)
            }
            (Drawing::Perch(_), _, Some(at)) => Run::new(Clip::PerchLeave { at }, frame, leave),
            _ => {
                capri.still(None);
                Run::new(Clip::Leave, frame, self.sheets.life_leaving.len())
            }
        }
    }

    /// Ses planches debout sont-elles là ?
    pub fn has_stand(&self) -> bool {
        self.wardrobe.stand.is_some()
    }

    /// Ses planches assises sont-elles là ?
    pub fn has_perch(&self) -> bool {
        self.wardrobe.perch.is_some()
    }

    /// Cette planche de sa vie est-elle là, avec sa pose debout sur laquelle elle s'aligne ?
    pub fn has_move(&self, strip: life::Strip) -> bool {
        self.has_stand() && self.life_mask(strip).is_some()
    }

    /// Ses gestes et ses variantes, tels que chargés.
    pub fn moves(&self) -> &Moves {
        &self.wardrobe.moves
    }

    /// Le masque de clic d'une planche de sa vie, si elle est là.
    fn life_mask(&self, strip: life::Strip) -> Option<&AlphaMask> {
        let moves = &self.wardrobe.moves;
        match strip {
            life::Strip::Arrive | life::Strip::Leave => self.wardrobe.life.as_ref(),
            life::Strip::Walk => moves.walk.as_ref(),
            life::Strip::Dangle => moves.dangle.as_ref(),
            life::Strip::Glitch => moves.glitch.as_ref(),
            life::Strip::Grin => moves.grin.as_ref(),
            life::Strip::Sleep => self.nap.as_ref().map(|masks| &masks.sleep),
            life::Strip::Wake => self.nap.as_ref().map(|masks| &masks.wake),
        }
    }

    /// Ce geste peut-il se jouer (sa planche est-elle là) ?
    pub fn can(&self, gesture: Gesture) -> bool {
        match gesture {
            Gesture::Rise => self.has_move(life::Strip::Glitch),
            Gesture::Grin => self.has_move(life::Strip::Grin),
            Gesture::PerchGlitch => self.has_perch() && self.wardrobe.moves.perch_glitch,
            Gesture::LieGlitch => self.has_perch() && self.lie.is_some(),
        }
    }

    /// Le geste en cours, s'il y en a un.
    pub fn gesture(&self) -> Option<Gesture> {
        self.act.map(|(gesture, _)| gesture)
    }

    /// L'image où le geste en cours a commencé, s'il y en a un.
    pub fn gesture_since(&self) -> Option<u64> {
        self.act.map(|(_, since)| since)
    }

    /// On se parle (on lui parle au micro ou par écrit, elle parle, répond ou va répondre) : un
    /// geste en cours s'arrête au prochain pas, et aucun ne commence. À dire avant chaque pas.
    pub fn set_conversing(&mut self, conversing: bool) {
        self.conversing = conversing;
    }

    /// Son bonjour attend-il d'être dit (après l'intro du lancement) ? Tant qu'il attend, elle ne
    /// commence aucun geste d'elle-même (sourire pour Claude Code, éclat de sa vie) : le bonjour
    /// passe avant. Un clic, et se relever en bas de sa chute (qui fait partie de l'intro), si. À
    /// dire avant chaque pas.
    pub fn set_greeting_due(&mut self, due: bool) {
        self.greeting_due = due;
    }

    /// Libre pour un geste ou une marche : là, aucun geste en cours, on ne se parle pas, ni tenue,
    /// ni lancée, ni en marche.
    fn free(&self, capri: &Capri) -> bool {
        self.is_here()
            && self.act.is_none()
            && !self.talking
            && !self.conversing
            && !carried(capri)
            && !capri.is_walking()
    }

    /// Assise (avec ses planches), sa boucle en cours : ni en train d'arriver ni de repartir, ni
    /// allongée.
    fn is_sitting(&self, capri: &Capri) -> bool {
        matches!(self.seat, Seat::Sitting { .. })
            && self.has_perch()
            && capri.seat().is_some()
            && self.repose == Repose::Awake
    }

    /// Allongée sur son bord, éveillée (ses planches là).
    fn is_lying(&self, capri: &Capri) -> bool {
        matches!(self.seat, Seat::Sitting { .. })
            && capri.seat().is_some()
            && self.lie.is_some()
            && matches!(self.repose, Repose::Lying { .. })
    }

    /// Debout et immobile, éveillée : elle flotte, ou se tient debout sur un bord faute de pose
    /// assise.
    fn standing_still(&self, capri: &Capri) -> bool {
        matches!(self.seat, Seat::Standing)
            && self.repose == Repose::Awake
            && match capri.stance() {
                Stance::Floating => true,
                Stance::Perched { .. } => !self.has_perch(),
                // guettant, ses gestes ne se jouent pas (voir le module)
                Stance::Held { .. }
                | Stance::Thrown { .. }
                | Stance::Walking { .. }
                | Stance::Falling { .. }
                | Stance::Lurking { .. } => false,
            }
    }

    /// Le geste `gesture` commence à l'image `frame`, si sa planche est là et qu'elle est libre ;
    /// debout, son tour s'arrête le temps du geste.
    fn begin(&mut self, capri: &mut Capri, gesture: Gesture, frame: u64) -> bool {
        let posed = match gesture {
            Gesture::PerchGlitch => self.is_sitting(capri),
            Gesture::LieGlitch => self.is_lying(capri),
            Gesture::Rise | Gesture::Grin => self.standing_still(capri),
        };
        if !posed || !self.can(gesture) || !self.free(capri) {
            return false;
        }
        self.frame = self.frame.max(frame);
        self.act = Some((gesture, frame));
        if !gesture.seated() {
            capri.still(Some(frame + gesture.frames()));
        }
        true
    }

    /// Un clic sur elle (un appui bref, relâché sans la déplacer), à l'image `frame` : assise, un
    /// éclat ; allongée, un éclat allongée ; debout, elle sourit. Seulement si elle est libre et
    /// que la planche est là. Endormie, elle se réveille, sans geste. Rend le geste joué.
    pub fn poke(&mut self, capri: &mut Capri, frame: u64) -> Option<Gesture> {
        if self.stir(capri, frame) {
            return None;
        }
        let gesture = if self.is_lying(capri) {
            Gesture::LieGlitch
        } else if self.is_sitting(capri) {
            Gesture::PerchGlitch
        } else {
            Gesture::Grin
        };
        self.begin(capri, gesture, frame).then_some(gesture)
    }

    /// On s'occupe d'elle à l'image `frame` (un clic, ou Claude Code qui attend une réponse) :
    /// endormie, elle se réveille ([`Look::rouse`]). Compte comme une attention
    /// ([`Look::take_attention`]). Rend `true` si elle dormait.
    pub fn stir(&mut self, capri: &mut Capri, frame: u64) -> bool {
        self.attended = true;
        self.rouse(capri, frame)
    }

    /// Endormie, elle se réveille à l'image `frame` : allongée, elle est allongée éveillée (une
    /// coupe franche) ; debout, elle se réveille ([`life::waking`]), puis sa boucle. Rend `true` si
    /// elle dormait.
    pub fn rouse(&mut self, capri: &mut Capri, frame: u64) -> bool {
        self.frame = self.frame.max(frame);
        match self.repose {
            Repose::Sleeping { .. } => self.repose = Repose::Lying { since: frame },
            Repose::Dozing { .. } => {
                capri.still(Some(frame + self.sheets.wake.len()));
                self.repose = Repose::Waking { since: frame };
            }
            Repose::Awake | Repose::Lying { .. } | Repose::Waking { .. } => return false,
        }
        true
    }

    /// A-t-on fait attention à elle (un clic, un réveil) depuis le dernier appel ? Rendu une
    /// seule fois. `roam` en tient compte pour savoir depuis quand elle est tranquille.
    pub fn take_attention(&mut self) -> bool {
        std::mem::take(&mut self.attended)
    }

    /// Son repos allongée s'est-il arrêté sans qu'on s'occupe d'elle depuis le dernier appel : pourquoi,
    /// et dormait-elle ? Rendu une seule fois. `roam` le dit au journal, plutôt qu'un réveil.
    pub fn take_unsettled(&mut self) -> Option<(Unsettled, bool)> {
        self.unsettled.take()
    }

    /// Les jeux de planches de repos qui existent (leurs en-têtes lus au lancement, et la pose qui
    /// les porte : assise pour s'allonger, debout pour dormir debout). Sans eux, elle ne s'allonge
    /// ni ne s'endort debout. Sans effet sur ce qu'elle fait déjà.
    pub fn offer(&mut self, wants: Wants) {
        self.offered =
            Wants { lying: wants.lying && self.has_perch(), sleeping: wants.sleeping && self.has_stand() };
    }

    /// Les jeux de planches de repos qui existent ([`Look::offer`]).
    pub fn offered(&self) -> Wants {
        self.offered
    }

    /// Ses planches allongées sont là (leurs masques de clic), ou plus : reprises pendant qu'elle
    /// est allongée, elle se rassoit d'un coup, au pas suivant.
    pub fn dress_lying(&mut self, masks: Option<LieMasks>) {
        self.lie = masks;
    }

    /// Ses planches pour dormir debout sont là, ou plus : reprises pendant qu'elle dort debout, elle
    /// est éveillée d'un coup, au pas suivant.
    pub fn dress_sleeping(&mut self, masks: Option<NapMasks>) {
        self.nap = masks;
    }

    /// Ses planches allongées, son sommeil debout : sont-elles là ?
    pub fn dressed(&self) -> Wants {
        Wants { lying: self.lie.is_some(), sleeping: self.nap.is_some() }
    }

    /// Ce qu'elle veut de ses planches de repos maintenant : celles qu'elle montre (allongée,
    /// endormie, en train de s'effacer ou de réapparaître allongée ; endormie ou se réveillant
    /// debout), et celles qu'elle voudrait, pour s'allonger ou s'endormir ([`WISH`]). L'app les
    /// charge, ou les garde ; les autres, elle peut les reprendre.
    pub fn wants(&self) -> Wants {
        let wished = |wish: Option<u64>| wish.is_some_and(|at| self.frame < at.saturating_add(WISH));
        let run_lying = match self.presence {
            Presence::Appearing(run) | Presence::Vanishing(run, _) => run.lying(),
            _ => false,
        };
        let lying = matches!(self.repose, Repose::Lying { .. } | Repose::Sleeping { .. })
            || matches!(self.seat, Seat::Leaving { lying: true, .. })
            || run_lying
            || wished(self.lie_wish);
        let sleeping =
            matches!(self.repose, Repose::Dozing { .. } | Repose::Waking { .. }) || wished(self.nap_wish);
        Wants { lying, sleeping }
    }

    /// Là où elle est posée, il y a la place pour elle allongée, ou plus (son bord, l'écran) : l'app
    /// le sait de sa dernière lecture des fenêtres, à dire avant chaque pas. Allongée, sans la
    /// place, elle se rassoit.
    pub fn set_room_to_lie(&mut self, room: bool) {
        self.room_to_lie = room;
    }

    /// Ce qu'il lui faut sur son bord pour s'y allonger (`perch::fits`), si elle peut s'allonger.
    pub fn lying_stature(&self) -> Option<Stature> {
        self.offered.lying.then(|| lying_stature(self.height))
    }

    /// Son repos (voir le module).
    pub fn rest(&self) -> Rest {
        match self.repose {
            Repose::Awake => Rest::Awake,
            Repose::Lying { .. } => Rest::Lying,
            Repose::Sleeping { .. } => Rest::Asleep { lying: true },
            Repose::Dozing { .. } => Rest::Asleep { lying: false },
            Repose::Waking { .. } => Rest::Waking,
        }
    }

    /// Allongée, endormie, ou en train de se réveiller : elle ne change pas de place d'elle-même.
    pub fn is_resting(&self) -> bool {
        self.repose != Repose::Awake
    }

    /// De la musique joue-t-elle (ce qu'a dit le dernier pas) ? L'app ne le dit que posée
    /// (`wants_music`) : debout, toujours faux.
    pub fn hears_music(&self) -> bool {
        self.music
    }

    /// Assise, libre, sans musique, et la place pour elle allongée : elle s'allonge sur son bord à
    /// l'image `frame` ([`Gesture::LieGlitch`], puis sa boucle allongée). Ses planches pas encore
    /// là, elle en a envie ([`Look::wants`]) et reste assise : rend `false`, comme quand elle ne
    /// le peut pas. Rend `true` si elle s'allonge.
    pub fn lie_down(&mut self, capri: &mut Capri, frame: u64) -> bool {
        if !self.offered.lying
            || !self.room_to_lie
            || !self.is_sitting(capri)
            || !self.free(capri)
            || self.music
            || self.grooving
        {
            return false;
        }
        self.frame = self.frame.max(frame);
        if self.lie.is_none() {
            self.lie_wish = Some(frame);
            return false;
        }
        self.lie_wish = None;
        self.repose = Repose::Lying { since: frame + Gesture::LieGlitch.frames() };
        self.act = Some((Gesture::LieGlitch, frame));
        true
    }

    /// Elle s'endort à l'image `frame` : allongée et libre (sa boucle de sommeil, par une coupe
    /// franche), ou debout, immobile et libre (son sommeil debout, son tour arrêté ; ses planches
    /// pas encore là, elle en a envie et reste éveillée). Jamais pendant qu'on se parle ; allongée,
    /// jamais sur la musique (debout, la musique ne change rien : l'app ne la lui dit que posée).
    /// Rend `true` si elle s'endort.
    pub fn doze(&mut self, capri: &mut Capri, frame: u64) -> bool {
        if !self.free(capri) || self.music {
            return false;
        }
        self.frame = self.frame.max(frame);
        if self.is_lying(capri) {
            self.repose = Repose::Sleeping { since: frame };
            return true;
        }
        if !self.offered.sleeping || !self.standing_still(capri) || capri.stance() != Stance::Floating {
            return false;
        }
        if self.nap.is_none() {
            self.nap_wish = Some(frame);
            return false;
        }
        self.nap_wish = None;
        capri.still(None);
        self.repose = Repose::Dozing { since: frame };
        true
    }

    /// Allongée (endormie : elle se réveille dans le même pas), elle se rassoit à l'image `frame`,
    /// là où elle était assise : l'éclat de sa pose assise s'il est là, sinon une coupe franche.
    /// Seulement si elle est là, et pendant aucun autre geste que son éclat allongée. Rend `true`
    /// si elle se rassoit.
    pub fn sit_up(&mut self, capri: &mut Capri, frame: u64) -> bool {
        let lying = matches!(self.repose, Repose::Lying { .. } | Repose::Sleeping { .. });
        let busy = self.act.is_some_and(|(gesture, _)| gesture != Gesture::LieGlitch);
        if !lying || busy || !self.is_here() || capri.seat().is_none() {
            return false;
        }
        self.frame = self.frame.max(frame);
        self.seated_again(frame);
        true
    }

    /// Elle est assise de nouveau, à l'image `frame`, sa boucle au dessin 0 (l'éclat de sa pose
    /// assise d'abord, s'il est là : il finit sur ce dessin).
    fn seated_again(&mut self, frame: u64) {
        self.repose = Repose::Awake;
        self.seat = Seat::Sitting { since: frame };
        self.act = self.can(Gesture::PerchGlitch).then_some((Gesture::PerchGlitch, frame));
    }

    /// Claude Code vient de finir son tour (dans la session principale), à l'image `frame` :
    /// debout et libre, elle sourit, pas plus d'une fois par [`CHEER_EVERY`] ; assise, ou son bonjour
    /// pas encore dit ([`Look::set_greeting_due`]), rien. Rend `true` si elle sourit.
    pub fn cheer(&mut self, capri: &mut Capri, frame: u64) -> bool {
        if self.greeting_due || self.cheered.is_some_and(|at| frame < at.saturating_add(CHEER_EVERY)) {
            return false;
        }
        let cheered = self.begin(capri, Gesture::Grin, frame);
        if cheered {
            self.cheered = Some(frame);
        }
        cheered
    }

    /// Assise et libre, sans musique, son bonjour dit s'il y en avait un : un éclat, à l'image
    /// `frame` (sa vie le lui fait faire de temps en temps, voir `roam`). Rend `true` s'il se joue.
    pub fn fidget(&mut self, capri: &mut Capri, frame: u64) -> bool {
        !self.greeting_due && !self.music && !self.grooving && self.begin(capri, Gesture::PerchGlitch, frame)
    }

    /// Sa vitesse de marche, en pixels par seconde, si elle sait marcher (sa planche est là).
    pub fn walk_speed(&self) -> Option<f32> {
        self.has_move(life::Strip::Walk).then(|| life::walk_speed(self.height))
    }

    /// Elle marche jusqu'à ce que ses pieds soient en x = `to`, à partir de l'image `frame` :
    /// seulement debout, libre, en flottant, et si elle sait marcher. Son tour s'arrête le temps de
    /// la marche. Rend `false` sinon, et rien ne change.
    pub fn walk(&mut self, capri: &mut Capri, frame: u64, to: f32) -> bool {
        let Some(speed) = self.walk_speed() else { return false };
        if !self.free(capri)
            || capri.stance() != Stance::Floating
            || !matches!(self.seat, Seat::Standing)
            || self.repose != Repose::Awake
        {
            return false;
        }
        if !capri.walk_to(to, speed) {
            return false;
        }
        self.frame = self.frame.max(frame);
        capri.still(None);
        self.walking = Some(frame);
        true
    }

    /// Marche-t-elle ?
    pub fn is_walking(&self, capri: &Capri) -> bool {
        self.walking.is_some() && capri.is_walking()
    }

    /// Elle s'arrête de marcher là où elle est, et reprend sa boucle (on lui parle). Sans effet si
    /// elle ne marche pas.
    pub fn halt(&mut self, capri: &mut Capri) {
        if self.walking.take().is_some() {
            capri.halt();
            if !capri.is_held() {
                capri.unstill();
            }
        }
    }

    /// Tenue ou lancée, avec sa planche en l'air : elle pend.
    pub fn is_dangling(&self, capri: &Capri) -> bool {
        carried(capri) && self.has_move(life::Strip::Dangle) && self.is_here()
    }

    /// Tenue ou lancée, avec sa planche en l'air : là où est dessiné le point par où on la tient
    /// ([`life::GRIP`]), qui doit être sous le curseur quand on la tient.
    pub fn grip(&self, capri: &Capri) -> Option<Vec2> {
        if !self.is_dangling(capri) {
            return None;
        }
        let rect = life::over_stand(capri.pose().rect);
        Some(
            rect.min
                + Vec2::new(
                    life::GRIP.0 * rect.size.x / life::CELL_WIDTH as f32,
                    life::GRIP.1 * rect.size.y / sheets::CELL_HEIGHT as f32,
                ),
        )
    }

    /// Sa hauteur debout a changé (sa taille, ou la densité de l'écran).
    pub fn set_height(&mut self, height: f32) {
        self.height = height;
    }

    /// La taille à l'écran de son image debout (une case de sa boucle, ou son dessin d'avant),
    /// pour qu'elle mesure sa hauteur debout.
    pub fn standing_size(&self) -> Vec2 {
        if self.has_stand() {
            return stand::cell_size(self.height);
        }
        let (width, height) = self.wardrobe.cutout.size();
        Vec2::new(width as f32 * self.height / height.max(1) as f32, self.height)
    }

    /// Où sont ses pieds dans son image debout (coordonnées normalisées).
    pub fn standing_feet(&self) -> Vec2 {
        if self.has_stand() { stand::feet_uv() } else { self.wardrobe.cutout.feet() }
    }

    /// Combien d'images dure son départ d'un bord (voir `Capri::set_leave_frames`) : le temps de
    /// s'effacer, si elle a une pose assise.
    pub fn leave_frames(&self) -> u64 {
        if self.has_perch() { self.sheets.leaving.len() } else { 0 }
    }

    /// Ce qu'il lui faut sur un bord pour s'y poser, ou y rester : assise, sa ligne d'assise sur
    /// le bord, la place de ses cheveux au-dessus et de ses jambes en dessous, et ce qui repose sur
    /// le bord (de sa main à son genou) ; sans planches assises, debout, comme avant.
    pub fn stature(&self) -> Stature {
        if self.has_perch() {
            seated_stature(self.height)
        } else {
            Stature::standing(self.standing_size(), self.standing_feet(), self.height)
        }
    }

    /// Ce qu'il lui faut sur le côté d'une fenêtre pour y guetter ([`lurking_stature`]), si elle le
    /// peut : seulement avec ses planches debout, les seules qu'elle montre là.
    pub fn lurk_stature(&self) -> Option<LurkStature> {
        self.has_stand().then(|| lurking_stature(self.height))
    }

    /// Guettant, comment son image se place ([`lurk_fit`]), si elle peut guetter (ses planches
    /// debout là).
    pub fn lurk_fit(&self) -> Option<LurkFit> {
        self.has_stand().then(lurk_fit)
    }

    /// Posée, avec ses planches assises : c'est alors seulement que la musique peut la faire se
    /// balancer.
    pub fn wants_music(&self, capri: &Capri) -> bool {
        self.has_perch() && capri.perched().is_some()
    }

    /// Se balance-t-elle en ce moment ?
    pub fn is_grooving(&self) -> bool {
        self.grooving
            && matches!(self.seat, Seat::Sitting { .. })
            && !self.talking
            && self.is_here()
            && self.act.is_none()
            && self.repose == Repose::Awake
    }

    /// À chaque pas, après celui de Capri et le suivi de sa fenêtre : l'image d'animation
    /// `frame`, si elle parle, et si de la musique joue. Son geste finit ou s'arrête ici, et, lâchée
    /// debout après avoir été tenue ou lancée, elle se relève.
    pub fn update(&mut self, capri: &mut Capri, frame: u64, talking: bool, music: bool) {
        let previous = std::mem::replace(&mut self.frame, frame);
        self.talking = talking;
        self.music = music;
        let seat = capri.seat().filter(|_| self.has_perch());
        let (arrive, leave) = (self.sheets.arriving.len(), self.sheets.leaving.len());
        // allongée (ses planches là), elle s'efface de son bord allongée
        let lying =
            matches!(self.repose, Repose::Lying { .. } | Repose::Sleeping { .. }) && self.lie.is_some();
        self.seat = match (self.seat, seat) {
            // on l'attrape : debout tout de suite, sans s'effacer
            (_, None) if capri.is_held() => Seat::Standing,
            (Seat::Standing | Seat::Leaving { .. }, Some(_)) => Seat::Arriving { since: frame },
            (Seat::Arriving { since }, Some(_)) if frame >= since + arrive => {
                Seat::Sitting { since: since + arrive }
            }
            (Seat::Arriving { .. } | Seat::Sitting { .. }, None) => match self.last_seat {
                Some(at) => Seat::Leaving { since: frame, at, lying },
                None => Seat::Standing,
            },
            (Seat::Leaving { since, .. }, None) if frame >= since + leave => Seat::Standing,
            (unchanged, _) => unchanged,
        };
        if seat.is_some() {
            self.last_seat = seat;
        }
        // apparaître ou s'effacer : jusqu'au bout de la suite, sauf si on l'attrape pendant
        // qu'elle apparaît, ou si elle n'est plus là où elle apparaissait (posée, ou plus posée)
        self.presence = match self.presence {
            Presence::Appearing(_) if capri.is_held() => Presence::Here,
            // sortie de derrière sa fenêtre : là ; ou elle ne guette plus (son côté parti), et elle
            // est là, debout
            Presence::Appearing(run) if run.clip == Clip::Slide => match capri.lurk_out() {
                Some(out) if out < 1.0 => Presence::Appearing(run),
                _ => Presence::Here,
            },
            // retirée derrière sa fenêtre, rien ne se voit d'elle : partie ; son côté parti en
            // chemin, debout, elle s'efface comme d'habitude
            Presence::Vanishing(run, exit) if run.clip == Clip::Slide => match capri.lurk_out() {
                Some(out) if out > 0.0 => Presence::Vanishing(run, exit),
                Some(_) => Presence::Gone(Some(run), exit),
                None => match self.leaving(capri, Drawing::Cutout, frame) {
                    Some(run) => Presence::Vanishing(run, exit),
                    None => Presence::Gone(None, exit),
                },
            },
            Presence::Appearing(run) if run.seat().is_some() != seat.is_some() => Presence::Here,
            Presence::Appearing(run) if run.left(frame) == 0 => Presence::Here,
            Presence::Vanishing(run, exit) if run.left(frame) == 0 => Presence::Gone(Some(run), exit),
            Presence::Entering(entrance) => self.on_stage(capri, entrance, frame),
            unchanged => unchanged,
        };
        self.resting(capri, frame, talking);
        self.gestures(capri, frame, talking);
        // la musique se prend et se quitte au début d'un tour de sa boucle ; en parlant, elle ne
        // se balance pas, et n'y revient qu'au tour suivant ; allongée, elle ne se balance pas
        match self.seat {
            Seat::Sitting { .. } if talking || self.repose != Repose::Awake => self.grooving = false,
            Seat::Sitting { since } => {
                let tour = |f: u64| f.saturating_sub(since) / TOUR;
                if previous < since || tour(frame) != tour(previous) {
                    self.grooving = music;
                }
            }
            _ => self.grooving = false,
        }
    }

    /// Un pas de son repos, dans `update` : ce qui la réveille, ou la fait se rasseoir.
    fn resting(&mut self, capri: &mut Capri, frame: u64, talking: bool) {
        let disturbed = talking || self.conversing;
        let carried = carried(capri);
        // en train de réapparaître allongée (elle revient sur ses pas), elle l'est encore ; assise
        // (`self.seat`, revu plus haut dans `update`), elle est posée, et donc ni tenue ni lancée
        let on_ledge = matches!(self.seat, Seat::Sitting { .. }) && !self.is_away();
        let standing = matches!(self.seat, Seat::Standing)
            && capri.stance() == Stance::Floating
            && !self.is_away()
            && !carried;
        let asleep = matches!(self.repose, Repose::Sleeping { .. });
        match self.repose {
            Repose::Awake => {}
            // attrapée (on s'occupe d'elle : `roam` le voit), cachée de nouveau pendant qu'elle
            // réapparaissait allongée (un geste aussi), ou plus posée sur son bord (elle s'en efface
            // allongée, `Seat::Leaving`) : elle n'est plus allongée
            Repose::Lying { .. } | Repose::Sleeping { .. } if !on_ledge => {
                if !carried && !self.is_away() {
                    self.unsettled = Some((Unsettled::LedgeGone, asleep));
                }
                self.repose = Repose::Awake;
            }
            // ses planches reprises, ou plus la place pour elle allongée : elle se rassoit, une fois
            // là (en train de réapparaître allongée, elle finit d'abord : sa boucle assise repart
            // alors de son début)
            Repose::Lying { .. } | Repose::Sleeping { .. }
                if (self.lie.is_none() || !self.room_to_lie) && self.is_here() =>
            {
                let why = if self.lie.is_none() { Unsettled::Undressed } else { Unsettled::NoRoom };
                self.unsettled = Some((why, asleep));
                self.seated_again(frame);
            }
            // on lui parle : elle se réveille, allongée
            Repose::Sleeping { .. } if disturbed => self.repose = Repose::Lying { since: frame },
            // attrapée (son tour a déjà repris), plus debout immobile, ou ses planches reprises
            Repose::Dozing { .. } | Repose::Waking { .. } if !standing || self.nap.is_none() => {
                if !carried {
                    capri.unstill();
                }
                self.repose = Repose::Awake;
            }
            // on lui parle : elle se réveille, debout
            Repose::Dozing { .. } if disturbed => {
                self.rouse(capri, frame);
            }
            Repose::Waking { since } if frame >= since + self.sheets.wake.len() => {
                self.repose = Repose::Awake
            }
            Repose::Lying { .. }
            | Repose::Sleeping { .. }
            | Repose::Dozing { .. }
            | Repose::Waking { .. } => {}
        }
    }

    /// Un pas de ses gestes, dans `update` : tenue ou lancée, depuis quand ; sa marche finie ; le
    /// geste en cours fini ou interrompu (une prise, on se parle, elle n'est plus là, ou plus dans
    /// la pose du geste) ; lâchée debout, elle se relève.
    fn gestures(&mut self, capri: &mut Capri, frame: u64, talking: bool) {
        let carried = carried(capri);
        let released = match (self.carried, carried) {
            (None, true) => {
                self.carried = Some(frame);
                false
            }
            (Some(_), false) => {
                self.carried = None;
                true
            }
            _ => false,
        };
        // arrivée, arrêtée au bord de l'écran, attrapée : sa marche est finie, et son tour reprend
        if self.walking.is_some() && !capri.is_walking() {
            self.walking = None;
            if self.is_here() && !carried {
                capri.unstill();
            }
        }
        if let Some((gesture, since)) = self.act {
            let end = since.saturating_add(gesture.frames());
            let sitting = matches!(self.seat, Seat::Sitting { .. });
            let lying = sitting && matches!(self.repose, Repose::Lying { .. });
            let posed = match gesture {
                Gesture::PerchGlitch => sitting && self.repose == Repose::Awake,
                Gesture::LieGlitch => lying,
                Gesture::Rise | Gesture::Grin => matches!(self.seat, Seat::Standing),
            };
            let cut = carried || talking || self.conversing || !self.is_here() || !posed;
            if cut || frame >= end {
                self.act = None;
                if gesture == Gesture::LieGlitch && lying {
                    // sa boucle allongée repart de son dessin 0, sur lequel l'éclat finit
                    self.repose = Repose::Lying { since: end.min(frame) };
                } else if gesture.seated() && sitting && self.repose == Repose::Awake {
                    // sa boucle repart de son dessin 0, sur lequel l'éclat finit
                    self.seat = Seat::Sitting { since: end.min(frame) };
                } else if !gesture.seated() && cut && !carried && self.is_here() {
                    // fini plus tôt : son tour reprend tout de suite (une prise l'a déjà repris)
                    capri.unstill();
                }
            }
        }
        if released && capri.stance() == Stance::Floating && self.act.is_none() {
            self.begin(capri, Gesture::Rise, frame);
        }
    }

    /// Ce qu'on dessine maintenant.
    pub fn picture(&self, capri: &Capri) -> Picture {
        let frame = self.frame;
        match self.presence {
            // une intro qu'elle joue (tenue, elle pend, plus bas)
            Presence::Entering(entrance) if !capri.is_held() => {
                return self.on_stage_picture(capri, entrance);
            }
            // tenue, elle est debout tout de suite
            Presence::Appearing(run) if !capri.is_held() => return self.played(capri, run),
            Presence::Vanishing(run, _) | Presence::Gone(Some(run), _) => return self.played(capri, run),
            _ => {}
        }
        // tenue ou lancée : elle pend, tenue par sa mèche
        if carried(capri) && self.has_move(life::Strip::Dangle) {
            let since = self.carried.unwrap_or(frame);
            return self.alive(capri, self.sheets.dangle.at(frame.saturating_sub(since)));
        }
        if !capri.is_held() {
            let at = capri.seat().or(self.last_seat);
            match (self.seat, at) {
                (Seat::Arriving { since }, Some(at)) => {
                    let cell = at_most(&self.sheets.arriving, frame.saturating_sub(since));
                    return self.seated(at, cell);
                }
                (Seat::Sitting { since }, Some(at)) => {
                    if let Some(picture) = self.lying_picture(at) {
                        return picture;
                    }
                    if let Some((Gesture::PerchGlitch, start)) = self.act {
                        let cell = at_most(&self.sheets.perch_glitch, frame.saturating_sub(start));
                        return self.seated(at, cell);
                    }
                    let f = frame.saturating_sub(since);
                    let tour = f / TOUR;
                    let sheet = if self.talking {
                        &self.sheets.talking
                    } else if self.grooving {
                        &self.sheets.music
                    } else if blinks_glitchy(tour) && self.wardrobe.moves.perch_blink_glitch {
                        &self.sheets.sitting_glitchy
                    } else if blinks(tour) {
                        &self.sheets.sitting_blink
                    } else {
                        &self.sheets.sitting
                    };
                    return self.seated(at, sheet.at(f));
                }
                (Seat::Leaving { since, at, lying: true }, _) if self.lie.is_some() => {
                    return self.lain(at, at_most(&self.sheets.lying_leave, frame.saturating_sub(since)));
                }
                (Seat::Leaving { since, at, .. }, _) => {
                    return self.seated(at, at_most(&self.sheets.leaving, frame.saturating_sub(since)));
                }
                _ => {}
            }
        }
        self.standing(capri)
    }

    /// Allongée sur son bord en `at` (ses planches là) : son éclat, sa boucle (parlante, ou ses
    /// clignements aux tours de [`BLINKS`]), ou son sommeil. `None` si elle n'est pas allongée.
    fn lying_picture(&self, at: Vec2) -> Option<Picture> {
        let frame = self.frame;
        self.lie.as_ref()?;
        match self.repose {
            Repose::Lying { since } => {
                if let Some((Gesture::LieGlitch, start)) = self.act {
                    return Some(
                        self.lain(at, at_most(&self.sheets.lying_glitch, frame.saturating_sub(start))),
                    );
                }
                let f = frame.saturating_sub(since);
                let sheet = if self.talking {
                    &self.sheets.lying_talk
                } else if blinks(f / TOUR) {
                    &self.sheets.lying_blink
                } else {
                    &self.sheets.lying
                };
                Some(self.lain(at, sheet.at(f)))
            }
            Repose::Sleeping { since } => {
                Some(self.lain(at, self.sheets.lying_asleep.at(frame.saturating_sub(since))))
            }
            Repose::Awake | Repose::Dozing { .. } | Repose::Waking { .. } => None,
        }
    }

    /// Est-elle dessinée sur son bord (en train d'y arriver, assise, allongée, ou de s'effacer) ?
    pub fn is_seated(&self, capri: &Capri) -> bool {
        matches!(self.picture(capri).drawing, Drawing::Perch(_) | Drawing::Lie(_))
    }

    /// Le curseur est-il sur elle ? Au pixel près, d'après le masque de la pose dessinée (avec
    /// une petite marge pour attraper ses membres fins). Jamais quand elle s'efface.
    pub fn is_over(&self, capri: &Capri, cursor: Vec2) -> bool {
        if self.is_away() {
            return false;
        }
        let picture = self.picture(capri);
        let cutout = &self.wardrobe.cutout;
        let mask = match picture.drawing {
            Drawing::Cutout => cutout,
            Drawing::Stand(_) => self.wardrobe.stand.as_ref().unwrap_or(cutout),
            Drawing::Perch(cell) => match &self.wardrobe.perch {
                Some(masks) if cell.strip.cell().0 == sheets::WIDE => &masks.wide,
                Some(masks) => &masks.narrow,
                None => cutout,
            },
            Drawing::Lie(cell) => match &self.lie {
                Some(masks) if cell.strip.cell().0 == lie::WIDE => &masks.wide,
                Some(masks) => &masks.narrow,
                None => return false,
            },
            Drawing::Life(cell) => match self.life_mask(cell.strip) {
                Some(mask) => mask,
                None => return false,
            },
            // ses bandes, d'un bord à l'autre de l'écran : rien ne l'attrape, elle n'est pas là
            Drawing::Gather(_) => return false,
        };
        // guettant : ce qui est derrière sa fenêtre laisse passer le clic, et la marge de son masque
        // ne passe pas le bord ; ce qui se voit, son dessin redressé le dit
        let cursor = match picture.lean {
            Some(lean) => {
                let edge = lurk_edge(lean.angle) * picture.rect.size.x / stand::CELL_WIDTH as f32;
                if !lean.shows(Vec2::new(cursor.x - lean.side.outward() * edge, cursor.y)) {
                    return false;
                }
                lean.upright(cursor)
            }
            None => cursor,
        };
        picture.rect.to_unit(cursor).is_some_and(|uv| mask.hit(uv))
    }

    /// Ce qu'elle occupe à l'écran, sans son flottement ni ses gestes : de quoi placer son
    /// bandeau au-dessus d'elle et le champ de saisie sous ses pieds.
    pub fn shown(&self, capri: &Capri) -> Rect {
        let picture = self.picture(capri);
        if let Some(lean) = picture.lean {
            return self.lurk_shown(picture.rect, lean);
        }
        let drawing = picture.drawing;
        if let (Drawing::Lie(_), Some(at)) = (drawing, self.seated_at(capri)) {
            let k = lie::scale(self.height);
            let left = (lie::SEAT_X - lie::SPAN.0 as f32) * k;
            return Rect::new(
                Vec2::new(at.x - left, at.y - (lie::SEAT_Y - lie::TOP) as f32 * k),
                Vec2::new((lie::SPAN.1 - lie::SPAN.0) as f32 * k, (lie::BOTTOM - lie::TOP) as f32 * k),
            );
        }
        if let (Drawing::Perch(_), Some(at)) = (drawing, self.seated_at(capri)) {
            let k = stand::perch_scale(self.height);
            let left = (sheets::SEAT_X - sheets::SPAN.0) as f32 * k;
            return Rect::new(
                Vec2::new(at.x - left, at.y - sheets::HEADROOM as f32 * k),
                Vec2::new(
                    (sheets::SPAN.1 - sheets::SPAN.0) as f32 * k,
                    (sheets::HEADROOM + sheets::HANG) as f32 * k,
                ),
            );
        }
        let (top, tall) = self.standing_content();
        Rect::new(capri.position() + Vec2::new(0.0, top), Vec2::new(capri.size().x, tall))
    }

    /// Ce qui se voit d'elle à l'écran : guettant, sa part dehors de sa fenêtre (sa silhouette
    /// penchée, un peu plus large que le vrai) ; sinon, son dessin entier (`Picture::rect`). Pas ce
    /// qui place son bandeau ([`Look::shown`]).
    pub fn visible(&self, capri: &Capri) -> Rect {
        let picture = self.picture(capri);
        match picture.lean {
            Some(lean) => lurk_seen(picture.rect, lean),
            None => picture.rect,
        }
    }

    /// Guettant, de quoi placer son bandeau et son champ de saisie : la hauteur de ce qui se voit
    /// d'elle ([`lurk_seen`]) ; en largeur, autour de sa tête penchée, aussi loin que ce qui se voit
    /// d'elle de part et d'autre : son bandeau se centre au-dessus de sa tête, son champ de saisie en
    /// dessous.
    fn lurk_shown(&self, rect: Rect, lean: Lean) -> Rect {
        let seen = lurk_seen(rect, lean);
        let (left, right) = (seen.min.x, seen.max().x);
        let k = rect.size.x / stand::CELL_WIDTH as f32;
        // sa tête : le milieu de ses yeux, penché
        let ((x0, y0), (x1, y1)) = stand::EYES;
        let eyes = Vec2::new((x0 + x1 + 1) as f32 / 2.0, (y0 + y1 + 1) as f32 / 2.0);
        let head = lean.turned(rect.min + eyes * k).x.clamp(left, right);
        let half = (head - left).min(right - head);
        Rect::new(Vec2::new(head - half, seen.min.y), Vec2::new(2.0 * half, seen.size.y))
    }

    /// Une prise (`input::Grab`, faite par l'app au moment où un appui en devient une) : là où
    /// l'appui a commencé, son corps sans bouger (assise, elle se lève sous le curseur :
    /// [`Look::grab_origin`]) ; avec sa planche en l'air, elle glisse ensuite vers sa mèche sous le
    /// curseur ([`Look::grip_offset`]). Rien si elle s'efface ou est partie (rien ne l'attrape
    /// alors) : rend `false`.
    pub fn hold(&self, capri: &mut Capri, grab: &Grab) -> bool {
        if self.is_away() {
            return false;
        }
        let origin = self.grab_origin(capri, grab.at);
        let toward = self.grip_offset(capri);
        grab.take(capri, origin, toward);
        true
    }

    /// Avec sa planche en l'air : l'écart entre le curseur et le coin haut gauche de son image
    /// debout qui met sa mèche sous le curseur ([`life::grip`]).
    pub fn grip_offset(&self, capri: &Capri) -> Option<Vec2> {
        self.has_move(life::Strip::Dangle).then(|| life::grip(capri.size()))
    }

    /// On l'attrape assise, le curseur en `cursor` : où mettre le coin haut gauche de son image
    /// debout pour que le curseur tienne le même endroit d'elle (sa tête, ses pieds…), à
    /// proportion de sa silhouette. Guettant : son image redressée autour du point d'elle que
    /// tient le curseur, qui reste sous lui. `None` si elle n'est dessinée ni assise ni penchée.
    pub fn grab_origin(&self, capri: &Capri, cursor: Vec2) -> Option<Vec2> {
        let picture = self.picture(capri);
        // guettant : elle se redresse autour de ce que tient le curseur, qui reste sous lui
        if let Some(lean) = picture.lean {
            return Some(picture.rect.min + (cursor - lean.upright(cursor)));
        }
        if !self.is_seated(capri) {
            return None;
        }
        let from = self.shown(capri);
        let u = ((cursor.x - from.min.x) / from.size.x.max(1.0)).clamp(0.0, 1.0);
        let v = ((cursor.y - from.min.y) / from.size.y.max(1.0)).clamp(0.0, 1.0);
        let (top, tall) = self.standing_content();
        Some(Vec2::new(cursor.x - u * capri.size().x, cursor.y - top - v * tall))
    }

    /// Dans son image debout : où commence sa silhouette en hauteur, et combien elle mesure.
    fn standing_content(&self) -> (f32, f32) {
        let size = self.standing_size();
        if self.has_stand() {
            let s = size.y / sheets::CELL_HEIGHT as f32;
            (stand::TOP as f32 * s, stand::STATURE as f32 * s)
        } else {
            (0.0, size.y)
        }
    }

    /// Là où elle est (ou était) assise.
    fn seated_at(&self, capri: &Capri) -> Option<Vec2> {
        if let Presence::Appearing(run) | Presence::Vanishing(run, _) | Presence::Gone(Some(run), _) =
            self.presence
            && let Some(at) = run.seat()
        {
            return Some(self.run_seat(capri, at));
        }
        match self.seat {
            Seat::Leaving { at, .. } => Some(at),
            _ => capri.seat().or(self.last_seat),
        }
    }

    /// Debout : la boucle (ou son dessin d'avant) là où `body.rs` la met.
    fn standing(&self, capri: &Capri) -> Picture {
        let pose = capri.pose();
        if !self.has_stand() {
            return Picture {
                drawing: Drawing::Cutout,
                rect: pose.rect,
                mouth: None,
                boil_seed: pose.boil_seed,
                ghost: pose.ghost,
                lean: None,
            };
        }
        // en marche, ou un geste : ses planches de vie, par-dessus son image debout
        if let Some(since) = self.walking.filter(|_| capri.is_walking() && self.has_move(life::Strip::Walk)) {
            return self.alive(capri, self.sheets.walk.at(self.frame.saturating_sub(since)));
        }
        if let Some((gesture, since)) = self.act {
            let f = self.frame.saturating_sub(since).min(gesture.frames().saturating_sub(1));
            match gesture {
                Gesture::Rise => return self.alive(capri, at_most(&self.sheets.rise, f)),
                Gesture::Grin => return self.alive(capri, self.sheets.grin.at(f)),
                Gesture::PerchGlitch | Gesture::LieGlitch => {}
            }
        }
        // endormie debout, ou en train de se réveiller (ses planches là)
        if self.nap.is_some() {
            match self.repose {
                Repose::Dozing { since } => {
                    return self.alive(capri, self.sheets.nap.at(self.frame.saturating_sub(since)));
                }
                Repose::Waking { since } => {
                    return self.alive(capri, at_most(&self.sheets.wake, self.frame.saturating_sub(since)));
                }
                Repose::Awake | Repose::Lying { .. } | Repose::Sleeping { .. } => {}
            }
        }
        // la boucle suit son tour, que partage son flottement (il s'arrête quand elle apparaît ou
        // s'efface) ; sa bouche, l'image d'animation
        let tour = capri.tour();
        let sheet = if blinks_glitchy(tour / TOUR) && self.wardrobe.moves.stand_blink_glitch {
            &self.sheets.standing_glitchy
        } else if blinks(tour / TOUR) {
            &self.sheets.standing_blink
        } else {
            &self.sheets.standing
        };
        Picture {
            drawing: Drawing::Stand(sheet.at(tour)),
            rect: pose.rect,
            mouth: self.talking.then(|| self.sheets.mouth.at(self.frame)),
            boil_seed: 0,
            ghost: Vec2::ZERO,
            lean: lean(capri, pose.rect),
        }
    }

    /// Une suite jouée pour apparaître ou s'effacer, au dessin où elle en est. Assise, sur son
    /// bord ([`Look::run_seat`]).
    fn played(&self, capri: &Capri, run: Run) -> Picture {
        let at = run.position(self.frame);
        let seat = |was: Vec2| self.run_seat(capri, was);
        match run.clip {
            Clip::Arrive => self.alive(capri, self.sheets.life_arriving.at(at)),
            Clip::Leave => self.alive(capri, self.sheets.life_leaving.at(at)),
            Clip::PerchArrive { at: was } => self.seated(seat(was), self.sheets.arriving.at(at)),
            Clip::PerchLeave { at: was } => self.seated(seat(was), self.sheets.leaving.at(at)),
            // ses planches allongées reprises entre-temps (ce que l'app ne fait pas tant qu'elle les
            // veut) : son départ assis, au même endroit, plutôt que rien
            Clip::LieLeave { at: was } if self.lie.is_some() => {
                self.lain(seat(was), self.sheets.lying_leave.at(at))
            }
            Clip::LieLeave { at: was } => self.seated(seat(was), self.sheets.leaving.at(at)),
            // sa boucle debout, penchée et coupée, là où sa glissade la met
            Clip::Slide => self.standing(capri),
        }
    }

    /// Allongée, la case `cell` posée sur le bord en `at` : sa hanche là où elle était assise,
    /// la ligne du bord de ses planches sur le bord ([`lie::Strip::seat`]).
    fn lain(&self, at: Vec2, cell: lie::Cell) -> Picture {
        let k = lie::scale(self.height);
        let ((w, h), (x, y)) = (cell.strip.cell(), cell.strip.seat());
        Picture {
            drawing: Drawing::Lie(cell),
            rect: Rect::new(
                Vec2::new((at.x - x * k).round(), (at.y - y as f32 * k).round()),
                Vec2::new((w as f32 * k).round(), (h as f32 * k).round()),
            ),
            mouth: None,
            boil_seed: 0,
            ghost: Vec2::ZERO,
            lean: None,
        }
    }

    /// Une case de ses planches de vie, à la place de son image debout telle que `body.rs` la
    /// place : ses pieds sur ses pieds.
    fn alive(&self, capri: &Capri, cell: life::Cell) -> Picture {
        Picture {
            drawing: Drawing::Life(cell),
            rect: life::over_stand(capri.pose().rect),
            mouth: None,
            boil_seed: 0,
            ghost: Vec2::ZERO,
            lean: None,
        }
    }

    /// Assise, la case `cell` posée sur le bord en `at` : sa ligne d'assise sur le bord.
    fn seated(&self, at: Vec2, cell: sheets::Cell) -> Picture {
        let k = stand::perch_scale(self.height);
        let ((w, h), (x, y)) = (cell.strip.cell(), cell.strip.seat());
        Picture {
            drawing: Drawing::Perch(cell),
            rect: Rect::new(
                Vec2::new((at.x - x as f32 * k).round(), (at.y - y as f32 * k).round()),
                Vec2::new((w as f32 * k).round(), (h as f32 * k).round()),
            ),
            mouth: None,
            boil_seed: 0,
            ghost: Vec2::ZERO,
            lean: None,
        }
    }
}

/// Guettant, ce qui se voit d'elle, son dessin debout en `rect` penché selon `lean` : sa silhouette
/// penchée ([`stand::lurk_bounds`], un peu plus large que le vrai), là où sa glissade la met, et
/// rien derrière sa fenêtre (en train de glisser, ce qui se voit est plus étroit ; cachée, vide).
fn lurk_seen(rect: Rect, lean: Lean) -> Rect {
    let k = rect.size.x / stand::CELL_WIDTH as f32;
    let side = lean.side;
    // son point d'accroche dans sa case penchée : sur la coupe, à la hauteur de sa hanche (qui ne
    // bouge pas quand elle se penche)
    let anchor = lean.pivot + Vec2::new((stand::cut(side) - stand::HIP.0) * k, 0.0);
    let bounds = stand::lurk_bounds(side);
    let (lo, hi) = (anchor + bounds.min * k, anchor + bounds.max() * k);
    let (left, right) = match side {
        Side::Left => (lo.x.min(lean.wall), hi.x.min(lean.wall)),
        Side::Right => (lo.x.max(lean.wall), hi.x.max(lean.wall)),
    };
    Rect::new(Vec2::new(left, lo.y), Vec2::new(right - left, hi.y - lo.y))
}

/// Guettant (`capri` derrière le côté d'une fenêtre), son dessin debout en `rect` : penché autour de
/// sa hanche vers l'extérieur de la fenêtre, et coupé au bord ; `None` sinon.
fn lean(capri: &Capri, rect: Rect) -> Option<Lean> {
    let lurk = capri.lurking()?;
    let side = lurk.spot.side;
    let pivot = rect.min
        + Vec2::new(
            stand::HIP.0 * rect.size.x / stand::CELL_WIDTH as f32,
            stand::HIP.1 * rect.size.y / sheets::CELL_HEIGHT as f32,
        );
    Some(Lean { angle: stand::lean(side), pivot, wall: lurk.at.x, side })
}

/// Ce qui se voit d'elle quand elle guette, pour une hauteur debout de `height` pixels, autour de
/// son point d'accroche (sur le côté de la fenêtre, à la hauteur de sa hanche), de chaque côté
/// ([`stand::lurk_bounds`]), et un pixel de plus ([`ROUNDING`] : son image est posée au pixel
/// entier) ; rien derrière la fenêtre.
pub fn lurking_stature(height: f32) -> LurkStature {
    let k = stand::scale(height);
    let reach = |side: Side| {
        let bounds = stand::lurk_bounds(side);
        let (lo, hi) = (bounds.min * k, bounds.max() * k);
        let out = |v: f32| if v > 0.0 { v + ROUNDING } else { 0.0 };
        Reach { left: out(-lo.x), up: -lo.y + ROUNDING, right: out(hi.x), down: hi.y + ROUNDING }
    };
    LurkStature { height, left: reach(Side::Left), right: reach(Side::Right) }
}

/// Son image est posée au pixel entier ([`crate::body::Pose`]) : ce qui se voit d'elle peut être
/// jusqu'à un demi-pixel plus loin que prévu. Guettant, la place qu'il lui faut compte ce pixel.
pub const ROUNDING: f32 = 1.0;

/// Guettant, comment son image debout se place, pour [`Capri::lurk_on`] : sa hanche
/// ([`stand::HIP`]), de combien elle est dehors ([`stand::hip_out`]), et de combien elle glisse pour
/// que rien ne se voie d'elle (ce qui se voit d'elle de plus loin, et un pixel de ses planches).
pub fn lurk_fit() -> LurkFit {
    let w = stand::CELL_WIDTH as f32;
    let reach = |side: Side| {
        let bounds = stand::lurk_bounds(side);
        (-bounds.min.x).max(bounds.max().x)
    };
    LurkFit {
        hip: Vec2::new(stand::HIP.0 / w, stand::HIP.1 / sheets::CELL_HEIGHT as f32),
        hip_out: Side::ALL.map(|side| stand::hip_out(side) / w),
        slide: Side::ALL.map(|side| (reach(side) + 1.0) / w),
    }
}

/// Ce qu'il lui faut sur un bord, assise, pour une hauteur debout de `height` pixels : sa ligne
/// d'assise sur le bord, la place de ses cheveux au-dessus ([`sheets::HEADROOM`]) et de ses
/// jambes en dessous ([`sheets::HANG`]), sa largeur ([`sheets::SPAN`]), et ce qui repose sur le
/// bord, de sa main à son genou ([`sheets::REST`]), jamais moins que pour ses pieds debout.
pub fn seated_stature(height: f32) -> Stature {
    let k = stand::perch_scale(height);
    let px = |pixels: u32| pixels as f32 * k;
    let foot = FOOT * height;
    Stature {
        height,
        reach: Reach {
            left: px(sheets::SEAT_X - sheets::SPAN.0),
            up: px(sheets::HEADROOM),
            right: px(sheets::SPAN.1 - sheets::SEAT_X),
            down: px(sheets::HANG),
        },
        rest: (px(sheets::SEAT_X - sheets::REST.0).max(foot), px(sheets::REST.1 - sheets::SEAT_X).max(foot)),
    }
}

/// Ce qu'il lui faut sur un bord pour s'y allonger, là où elle est assise, pour une hauteur debout
/// de `height` pixels : la place de ses cheveux au-dessus ([`lie::TOP`]) et de ses jambes et ses
/// cheveux en dessous ([`lie::BOTTOM`]), sa largeur ([`lie::SPAN`], surtout à gauche du point
/// posé), et ce qui repose sur le bord, de son épaule ([`lie::SHOULDER`]) au point posé, jamais
/// moins que pour ses pieds debout.
pub fn lying_stature(height: f32) -> Stature {
    let k = lie::scale(height);
    let foot = FOOT * height;
    Stature {
        height,
        reach: Reach {
            left: (lie::SEAT_X - lie::SPAN.0 as f32) * k,
            up: (lie::SEAT_Y - lie::TOP) as f32 * k,
            right: (lie::SPAN.1 as f32 - lie::SEAT_X) * k,
            down: (lie::BOTTOM - lie::SEAT_Y) as f32 * k,
        },
        rest: (((lie::SEAT_X - lie::SHOULDER as f32) * k).max(foot), foot),
    }
}

/// Une suite jouée une fois : le dessin de l'image `frame`, ou le dernier une fois finie.
fn at_most<T: Copy>(sheet: &Sheet<T>, frame: u64) -> T {
    sheet.at(frame.min(sheet.len() - 1))
}

/// Tenue, lancée, ou en train de tomber du ciel (une intro) : en bas, elle se relève, comme
/// lâchée.
fn carried(capri: &Capri) -> bool {
    matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. } | Stance::Falling { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Stance;
    use crate::input::Grab;
    use crate::perch::{Perch, Spot};
    use crate::sheets::Strip;
    use crate::sheets::stand::Strip as StandStrip;

    /// Un masque plein, de la taille donnée.
    fn full(width: u32, height: u32) -> AlphaMask {
        AlphaMask::from_rgba(width, height, &vec![255; (width * height * 4) as usize], 96, 0)
    }

    /// Un masque vide sauf un rectangle plein (en pixels de l'image).
    fn block(width: u32, height: u32, (x0, y0, x1, y1): (u32, u32, u32, u32)) -> AlphaMask {
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for y in y0..y1 {
            for x in x0..x1 {
                rgba[((y * width + x) * 4 + 3) as usize] = 255;
            }
        }
        AlphaMask::from_rgba(width, height, &rgba, 96, 0)
    }

    fn wardrobe(stand: bool, perch: bool, life: bool) -> Wardrobe {
        Wardrobe {
            cutout: full(220, 420),
            stand: stand.then(|| full(stand::CELL_WIDTH, sheets::CELL_HEIGHT)),
            perch: perch.then(|| PerchMasks {
                narrow: block(sheets::NARROW, sheets::CELL_HEIGHT, (10, 40, 150, 370)),
                wide: full(sheets::WIDE, sheets::CELL_HEIGHT),
            }),
            // l'union de son arrivée : plein de (30, 15) à (210, 370), comme les vraies planches
            life: life.then(|| block(life::CELL_WIDTH, sheets::CELL_HEIGHT, (30, 15, 210, 370))),
            moves: Moves::default(),
        }
    }

    /// Debout, 300 points de haut sur un écran Retina.
    const HEIGHT: f32 = 600.0;

    /// Avec ses planches de vie : tant qu'on ne la cache ni ne la fait entrer, elles ne changent
    /// rien.
    fn dressed(stand: bool, perch: bool) -> Look {
        Look::new(wardrobe(stand, perch, true), HEIGHT)
    }

    fn capri(look: &Look) -> Capri {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        let mut capri = Capri::new(look.standing_size(), screen, Vec2::new(1000.0, 800.0));
        capri.set_feet(look.standing_feet());
        capri.set_leave_frames(look.leave_frames());
        capri
    }

    const SPOT: Spot = Spot { window: 7, along: 400.0, left: 600.0, width: 1600.0 };
    const EDGE: Vec2 = Vec2::new(1000.0, 1200.0);

    /// Fait avancer Capri et son dessin jusqu'à l'image `frame`.
    fn at(look: &mut Look, capri: &mut Capri, frame: u64, talking: bool, music: bool) -> Picture {
        capri.step(0.0, frame);
        look.update(capri, frame, talking, music);
        look.picture(capri)
    }

    #[test]
    fn standing_she_plays_her_loop_in_twos_and_blinks_only_on_chosen_tours() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        let mut blinked_tours = Vec::new();
        for frame in 0..TOUR * 8 {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Stand(cell) = picture.drawing else { panic!("debout : {picture:?}") };
            let drawing = (frame % TOUR) as u32 / 2 * 2;
            if cell.strip == StandStrip::StandBlink {
                assert_eq!(cell.index, drawing - stand::BLINK_FROM, "image {frame}");
                if blinked_tours.last() != Some(&(frame / TOUR)) {
                    blinked_tours.push(frame / TOUR);
                }
            } else {
                assert_eq!(cell, stand::Cell { strip: StandStrip::Stand, index: drawing }, "image {frame}");
            }
            assert_eq!((picture.mouth, picture.boil_seed, picture.ghost), (None, 0, Vec2::ZERO));
        }
        assert_eq!(blinked_tours, [0, 2, 3, 5], "quatre clignements en 20 s, toujours aux mêmes tours");
        // la même image donne le même dessin
        let again = look.picture(&c);
        look.update(&mut c, look.frame, false, false);
        assert_eq!(look.picture(&c), again);
    }

    #[test]
    fn her_float_and_her_hair_share_one_tour() {
        let (mut look, mut c) = (dressed(true, false), capri(&dressed(true, false)));
        let tops: Vec<f32> =
            (0..TOUR).map(|frame| at(&mut look, &mut c, frame, false, false).rect.min.y).collect();
        let lowest = tops.iter().cloned().fold(f32::MIN, f32::max);
        let highest = tops.iter().cloned().fold(f32::MAX, f32::min);
        // au plus bas (y le plus grand) quand ses cheveux sont au plus bas (dessins 24 à 30)
        assert!((24..30).all(|frame| tops[frame] == lowest), "{tops:?}");
        assert!((54..60).all(|frame| tops[frame] == highest), "{tops:?}");
        // et le tour suivant recommence pareil
        assert_eq!(at(&mut look, &mut c, TOUR + 26, false, false).rect.min.y, lowest);
    }

    #[test]
    fn standing_she_moves_her_mouth_only_while_she_talks() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        let quiet = at(&mut look, &mut c, 10, false, false);
        assert_eq!(quiet.mouth, None);
        let talking = at(&mut look, &mut c, 10, true, false);
        assert_eq!(talking.drawing, quiet.drawing, "la boucle continue dessous");
        assert_eq!(talking.rect, quiet.rect, "la bouche au même endroit qu'elle");
        let mouths: std::collections::HashSet<u32> = (10..40)
            .map(|frame| at(&mut look, &mut c, frame, true, false).mouth.expect("elle parle").index)
            .collect();
        assert!(mouths.len() >= 4, "{mouths:?}");
        // tenue, elle parle encore ; la musique ne change rien debout
        c.grab(Vec2::new(1050.0, 900.0), 0.0);
        let held = at(&mut look, &mut c, 41, true, true);
        assert!(matches!(held.drawing, Drawing::Stand(_)) && held.mouth.is_some());
    }

    /// Lâchée près du bord : elle s'y pose.
    fn land(look: &mut Look, c: &mut Capri, frame: u64) {
        c.step(0.0, frame);
        c.land(Perch { spot: SPOT, feet: EDGE });
        look.update(c, frame, false, false);
    }

    #[test]
    fn landing_she_appears_on_the_edge_then_sits_her_seat_on_it() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        at(&mut look, &mut c, 0, false, false);
        land(&mut look, &mut c, 5);
        let arrive = sheets::arriving().len();
        for frame in 5..5 + arrive {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Perch(cell) = picture.drawing else { panic!("{picture:?}") };
            assert_eq!(cell, sheets::Cell { strip: Strip::PerchArrive, index: (frame - 5) as u32 / 2 });
            // sa ligne d'assise sur le bord, à moins d'un pixel
            let seat = picture.rect.min + Vec2::new(picture.rect.size.x * cell.strip.seat_uv().x, 0.0);
            let seat_y = picture.rect.min.y + picture.rect.size.y * cell.strip.seat_uv().y;
            assert!((seat.x - EDGE.x).abs() <= 1.0 && (seat_y - EDGE.y).abs() <= 1.0, "{picture:?}");
        }
        // puis sa boucle, du dessin 0
        let first = at(&mut look, &mut c, 5 + arrive, false, false);
        assert_eq!(first.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        let k = stand::perch_scale(HEIGHT);
        assert_eq!(first.rect.size, Vec2::new((196.0 * k).round(), (384.0 * k).round()));
        let seat_y = first.rect.min.y + first.rect.size.y * Strip::Perch.seat_uv().y;
        assert!((seat_y - EDGE.y).abs() <= 1.0);
        // sa fenêtre bouge : elle suit
        c.ride(Perch { spot: SPOT, feet: EDGE + Vec2::new(30.0, -10.0) });
        let moved = at(&mut look, &mut c, 5 + arrive + 1, false, false);
        assert_eq!(moved.rect.min, first.rect.min + Vec2::new(30.0, -10.0));
    }

    #[test]
    fn she_leaves_by_fading_out_where_she_sat_but_stands_at_once_when_grabbed() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        land(&mut look, &mut c, 0);
        let sitting = 100;
        at(&mut look, &mut c, sitting, false, false);
        let seated_rect = look.picture(&c).rect;
        // sa fenêtre se ferme : elle s'efface là où elle était, puis flotte debout
        c.take_off();
        look.update(&mut c, sitting, false, false);
        let leave = sheets::leaving().len();
        for frame in sitting..sitting + leave {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Perch(cell) = picture.drawing else { panic!("image {frame} : {picture:?}") };
            assert_eq!(cell.strip, Strip::PerchLeave);
            assert_eq!(picture.rect.min.y, seated_rect.min.y, "au même endroit");
            assert_eq!(c.stance(), Stance::Floating);
            let size = Vec2::new(c.size().x.round(), c.size().y.round());
            assert_eq!(c.pose().rect.size, size, "pas d'étirement tant qu'elle s'efface");
        }
        let after = at(&mut look, &mut c, sitting + leave, false, false);
        assert!(matches!(after.drawing, Drawing::Stand(_)), "{after:?}");
        assert!(after.rect.size.y > c.size().y, "puis un petit étirement");
        assert!((c.feet() - EDGE).length() < 0.5, "debout, les pieds là où elle était assise");

        // assise de nouveau, on l'attrape : debout tout de suite, sous le curseur
        land(&mut look, &mut c, 300);
        at(&mut look, &mut c, 400, false, false);
        let cursor = look.shown(&c).min + Vec2::new(10.0, 10.0); // le haut de ses cheveux
        let origin = look.grab_origin(&c, cursor).expect("assise");
        c.grab_at(cursor, origin, 0.0);
        let held = at(&mut look, &mut c, 401, false, false);
        assert!(matches!(held.drawing, Drawing::Stand(_)), "{held:?}");
        let top = look.shown(&c).min.y;
        assert!(cursor.y - top < 30.0 && cursor.y >= top, "elle tient par les cheveux ({top})");
    }

    #[test]
    fn seated_she_talks_then_sways_to_music_on_a_tour_boundary_and_talking_comes_first() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        land(&mut look, &mut c, 0);
        let since = sheets::arriving().len();
        let strip =
            |look: &mut Look, c: &mut Capri, frame, talking, music| match at(look, c, frame, talking, music)
                .drawing
            {
                Drawing::Perch(cell) => (cell.strip, cell.index),
                other => panic!("{other:?}"),
            };
        // assise, elle parle : la même boucle, la bouche en plus, image pour image
        assert_eq!(strip(&mut look, &mut c, since + 10, true, false), (Strip::PerchTalk, 10));
        assert_eq!(strip(&mut look, &mut c, since + 11, false, false), (Strip::Perch, 10));
        // la musique commence au milieu d'un tour : elle attend le début du suivant
        assert_eq!(strip(&mut look, &mut c, since + 20, false, true).0, Strip::Perch);
        assert_eq!(strip(&mut look, &mut c, since + TOUR - 1, false, true).0, Strip::Perch);
        assert_eq!(strip(&mut look, &mut c, since + TOUR, false, true), (Strip::PerchMusic, 0));
        assert!(look.is_grooving());
        // elle se met à parler : tout de suite
        assert_eq!(strip(&mut look, &mut c, since + TOUR + 8, true, true), (Strip::PerchTalk, 8));
        // elle se tait : sa boucle, jusqu'au tour suivant, où la musique reprend
        assert_eq!(strip(&mut look, &mut c, since + TOUR + 20, false, true).0, Strip::Perch);
        assert_eq!(strip(&mut look, &mut c, since + 2 * TOUR, false, true), (Strip::PerchMusic, 0));
        // la musique s'arrête : elle finit son tour, puis revient à sa boucle
        assert_eq!(strip(&mut look, &mut c, since + 2 * TOUR + 30, false, false).0, Strip::PerchMusic);
        assert_eq!(strip(&mut look, &mut c, since + 3 * TOUR, false, false).0, Strip::Perch);
        assert!(!look.is_grooving());
        // une image sautée (elle était cachée) ne la perd pas
        assert_eq!(strip(&mut look, &mut c, since + 7 * TOUR + 3, false, true).0, Strip::PerchMusic);
    }

    #[test]
    fn seated_she_blinks_on_the_same_tours_as_standing() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        land(&mut look, &mut c, 0);
        let since = sheets::arriving().len();
        for tour in 0..8 {
            let frame = since + tour * TOUR + u64::from(sheets::BLINK_FROM) + 2;
            let Drawing::Perch(cell) = at(&mut look, &mut c, frame, false, false).drawing else { panic!() };
            assert_eq!(cell.strip == Strip::PerchBlink, blinks(tour), "tour {tour}");
        }
    }

    #[test]
    fn without_her_sheets_she_is_drawn_as_before() {
        // ni debout ni assise : son dessin d'avant, qui bout, avec sa copie violette, et elle se
        // tient debout sur les fenêtres
        let (mut look, mut c) = (dressed(false, false), capri(&dressed(false, false)));
        assert_eq!(look.standing_size(), Vec2::new(220.0 * HEIGHT / 420.0, HEIGHT));
        let picture = at(&mut look, &mut c, 3, true, true);
        assert_eq!(picture.drawing, Drawing::Cutout);
        assert_eq!(picture.mouth, None, "pas de bouche sur son dessin d'avant");
        assert!(picture.ghost != Vec2::ZERO);
        assert_eq!(look.leave_frames(), 0);
        land(&mut look, &mut c, 10);
        let perched = at(&mut look, &mut c, 11, false, true);
        assert_eq!(perched.drawing, Drawing::Cutout);
        assert!(!look.wants_music(&c), "sans pose assise, la musique ne sert à rien");
        assert_eq!(look.stature(), Stature::standing(look.standing_size(), look.standing_feet(), HEIGHT));

        // debout seulement : sa boucle debout, debout sur les fenêtres
        let (mut look, mut c) = (dressed(true, false), capri(&dressed(true, false)));
        land(&mut look, &mut c, 10);
        assert!(matches!(at(&mut look, &mut c, 40, false, false).drawing, Drawing::Stand(_)));
        assert_eq!(c.pose().rect, look.picture(&c).rect, "debout sur le bord, comme avant");

        // assise seulement : son dessin d'avant debout, ses planches assises sur les fenêtres
        let (mut look, mut c) = (dressed(false, true), capri(&dressed(false, true)));
        assert_eq!(at(&mut look, &mut c, 1, false, false).drawing, Drawing::Cutout);
        land(&mut look, &mut c, 2);
        assert!(matches!(at(&mut look, &mut c, 3, false, false).drawing, Drawing::Perch(_)));
        assert!(look.wants_music(&c));
    }

    #[test]
    fn seated_she_needs_much_less_room_above_the_edge() {
        let look = dressed(true, true);
        let stature = look.stature();
        let k = stand::perch_scale(HEIGHT);
        assert_eq!(stature.height, HEIGHT);
        assert!((stature.reach.up - 237.0 * k).abs() < 1e-3 && (stature.reach.down - 114.0 * k).abs() < 1e-3);
        // à la taille d'origine sur un écran Retina : 341 pixels (171 points) au-dessus du bord,
        // contre 300 points (et plus, avec la marge de sa case) debout
        assert!((341.0..342.0).contains(&stature.reach.up), "{stature:?}");
        let standing = Stature::standing(look.standing_size(), look.standing_feet(), HEIGHT);
        assert!(standing.reach.up > HEIGHT && stature.reach.up < HEIGHT * 0.62, "{standing:?}");
        // de sa main à son genou, sur le bord : 66 pixels de planche de chaque côté
        assert!((stature.rest.0 - 66.0 * k).abs() < 1e-3 && (stature.rest.1 - 66.0 * k).abs() < 1e-3);
        // et toujours plus que ce qu'il faut à ses pieds debout
        assert!(stature.rest.0 >= FOOT * HEIGHT);
    }

    #[test]
    fn she_is_caught_by_the_mask_of_the_pose_drawn() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        let standing = at(&mut look, &mut c, 0, false, false).rect;
        let middle = standing.min + standing.size * 0.5;
        assert!(look.is_over(&c, middle));
        assert!(!look.is_over(&c, standing.min - Vec2::new(5.0, 5.0)));
        land(&mut look, &mut c, 1);
        let since = 1 + sheets::arriving().len();
        let seated = at(&mut look, &mut c, since + 2, false, false).rect;
        let px = |x: u32, y: u32| {
            seated.min + Vec2::new(seated.size.x * x as f32 / 196.0, seated.size.y * y as f32 / 384.0)
        };
        // le masque assis : plein de (10, 40) à (150, 370) seulement
        assert!(look.is_over(&c, px(80, 200)));
        assert!(!look.is_over(&c, px(170, 200)), "hors de sa silhouette assise");
        assert!(!look.is_over(&c, px(80, 20)));
    }

    /// Le dessin de ses planches de vie, s'il en est un.
    fn life_cell(picture: &Picture) -> Option<life::Cell> {
        match picture.drawing {
            Drawing::Life(cell) => Some(cell),
            _ => None,
        }
    }

    #[test]
    fn at_launch_she_materializes_over_her_standing_image_then_floats_without_a_jump() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.enter(&mut c, 0);
        let arrive = life::arriving().len();
        let still = c.pose().rect;
        for frame in 0..arrive {
            let picture = at(&mut look, &mut c, frame, false, false);
            let cell = life_cell(&picture).unwrap_or_else(|| panic!("image {frame} : {picture:?}"));
            assert_eq!(cell, life::Cell { strip: life::Strip::Arrive, index: frame as u32 / 2 }, "en deux");
            // par-dessus son image debout, qui ne flotte pas pendant ce temps
            assert_eq!(c.pose().rect, still, "image {frame}");
            assert_eq!(picture.rect, life::over_stand(still));
            assert!(!look.is_here(), "pas libre tant qu'elle apparaît");
            assert_eq!(picture.mouth, None, "même en parlant, pas de bouche posée sur l'arrivée");
        }
        // puis sa boucle, du début de son tour, à la même place : la toile de l'arrivée recouvre
        // exactement son image debout
        let first = at(&mut look, &mut c, arrive, false, false);
        assert_eq!(first.drawing, Drawing::Stand(stand::Cell { strip: StandStrip::Stand, index: 0 }));
        assert_eq!(first.rect, still, "pas de saut");
        assert_eq!(life::over_stand(first.rect), life::over_stand(still));
        assert!(look.is_here());
        // et son flottement reprend : le dessin suivant six images plus tard
        assert_eq!(at(&mut look, &mut c, arrive + 5, false, false).rect, still);
        assert_ne!(at(&mut look, &mut c, arrive + 6, false, false).rect, still);
    }

    /// Elle entre à l'image 0, puis va jusqu'à l'image `frame`, là.
    fn entered(look: &mut Look, c: &mut Capri, frame: u64) -> Picture {
        c.step(0.0, 0);
        look.enter(c, 0);
        for f in [1, frame] {
            at(look, c, f, false, false);
        }
        assert!(look.is_here());
        look.picture(c)
    }

    #[test]
    fn hidden_she_fades_out_where_she_stands_and_shown_again_she_materializes_there() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let before = entered(&mut look, &mut c, 100);
        let tour = c.tour();
        look.vanish(&mut c, 100);
        let leave = life::leaving().len();
        for frame in 100..100 + leave {
            let picture = at(&mut look, &mut c, frame, false, false);
            let cell = life_cell(&picture).unwrap_or_else(|| panic!("image {frame} : {picture:?}"));
            assert_eq!(cell, life::Cell { strip: life::Strip::Leave, index: (frame - 100) as u32 / 2 });
            assert_eq!(picture.rect, life::over_stand(before.rect), "là où elle était, sans flotter");
            assert!(!look.is_here() && !look.is_gone());
            // rien ne l'attrape, pas même au milieu de son dessin
            let middle = picture.rect.min + picture.rect.size * 0.5;
            assert!(!look.is_over(&c, middle) && !look.is_over(&c, before.rect.min + before.rect.size * 0.5));
        }
        at(&mut look, &mut c, 100 + leave, false, false);
        assert!(look.is_gone(), "puis plus rien à montrer");
        // on la montre bien plus tard : elle se matérialise au même endroit, et son tour reprend
        // là où il s'était arrêté
        c.step(0.0, 900);
        look.appear(&mut c, 900);
        let back = at(&mut look, &mut c, 900, false, false);
        assert_eq!(life_cell(&back), Some(life::Cell { strip: life::Strip::Arrive, index: 0 }));
        assert_eq!(back.rect, life::over_stand(before.rect));
        let arrive = life::arriving().len();
        let standing = at(&mut look, &mut c, 900 + arrive, false, false);
        assert_eq!(standing.rect, before.rect, "pas de saut");
        assert_eq!(c.tour(), tour, "son tour reprend là où il s'était arrêté");
        assert!(look.is_here());
    }

    #[test]
    fn shown_or_hidden_halfway_she_goes_back_on_her_steps_without_a_jump() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let here = entered(&mut look, &mut c, 100);
        look.vanish(&mut c, 100);
        let leave = |index| Some(life::Cell { strip: life::Strip::Leave, index });
        assert_eq!(life_cell(&at(&mut look, &mut c, 110, false, false)), leave(5));
        // montrée de nouveau : le même dessin, puis les précédents, jusqu'au premier
        look.appear(&mut c, 110);
        assert!(!look.is_away() && !look.is_here());
        let shown: Vec<_> =
            (110..121).map(|frame| life_cell(&at(&mut look, &mut c, frame, false, false))).collect();
        assert_eq!(shown[0], leave(5));
        assert_eq!(shown[1], leave(4));
        assert_eq!(shown[10], leave(0));
        let standing = at(&mut look, &mut c, 121, false, false);
        assert!(matches!(standing.drawing, Drawing::Stand(_)) && look.is_here());
        assert_eq!(standing.rect, here.rect, "elle n'a pas bougé");

        // cachée pendant qu'elle apparaît : l'arrivée à l'envers, jusqu'au trait, puis plus rien
        c.step(0.0, 300);
        look.vanish(&mut c, 300);
        at(&mut look, &mut c, 330, false, false);
        assert!(look.is_gone());
        c.step(0.0, 400);
        look.appear(&mut c, 400);
        let arrive = |index| Some(life::Cell { strip: life::Strip::Arrive, index });
        assert_eq!(life_cell(&at(&mut look, &mut c, 407, false, false)), arrive(3));
        look.vanish(&mut c, 407);
        assert!(look.is_away());
        assert_eq!(life_cell(&at(&mut look, &mut c, 407, false, false)), arrive(3));
        assert_eq!(life_cell(&at(&mut look, &mut c, 409, false, false)), arrive(2));
        assert_eq!(life_cell(&at(&mut look, &mut c, 414, false, false)), arrive(0));
        assert!(!look.is_gone());
        at(&mut look, &mut c, 415, false, false);
        assert!(look.is_gone());
        // cacher ce qui est caché, montrer ce qui est là : rien ne change
        look.vanish(&mut c, 416);
        assert!(look.is_gone());
        look.appear(&mut c, 500);
        let appearing = look.presence;
        look.appear(&mut c, 501);
        assert_eq!(look.presence, appearing);
    }

    #[test]
    fn hidden_before_she_ever_appeared_she_is_gone_at_once_and_materializes_when_shown() {
        // l'intro coupée en la cachant : elle n'était pas encore là, rien ne se joue
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        c.step(0.0, 30);
        look.hide_unseen(&mut c, 30);
        assert!(look.is_gone() && look.is_hidden() && look.is_away() && !look.is_traveling());
        assert!(!look.is_over(&c, c.position() + c.size() * 0.5), "rien ne l'attrape");
        // la cacher encore ne change rien ; montrée, elle se matérialise là où elle se tient
        look.vanish(&mut c, 40);
        assert!(look.is_gone());
        c.step(0.0, 200);
        look.appear(&mut c, 200);
        let back = at(&mut look, &mut c, 200, false, false);
        assert_eq!(life_cell(&back), Some(life::Cell { strip: life::Strip::Arrive, index: 0 }));
        at(&mut look, &mut c, 200 + life::arriving().len(), false, false);
        assert!(look.is_here());
        // sans ses planches de vie : là d'un coup, comme avant
        let mut look = dressed(true, false);
        look.wardrobe.life = None;
        let mut c = capri(&look);
        look.hide_unseen(&mut c, 0);
        assert!(look.is_gone());
        look.appear(&mut c, 10);
        assert!(look.is_here());
    }

    #[test]
    fn grabbed_while_she_materializes_she_stands_at_once_where_she_was() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.enter(&mut c, 0);
        let picture = at(&mut look, &mut c, 12, false, false);
        assert!(life_cell(&picture).is_some());
        // le masque de son arrivée l'attrape, pas le coin de sa case
        let middle = picture.rect.min + picture.rect.size * 0.5;
        assert!(look.is_over(&c, middle));
        assert!(!look.is_over(&c, picture.rect.min + Vec2::new(2.0, 2.0)));
        assert_eq!(look.grab_origin(&c, middle), None, "debout : elle reste où elle est");
        let (position, tour) = (c.position(), c.tour());
        c.grab(middle, 0.0);
        let held = at(&mut look, &mut c, 13, false, false);
        assert!(matches!(held.drawing, Drawing::Stand(_)), "debout tout de suite : {held:?}");
        assert!(look.is_here());
        assert_eq!(c.position(), position);
        // lâchée, elle flotte : son tour a repris dès qu'on l'a attrapée, à l'image 12
        c.release(0.01);
        at(&mut look, &mut c, 20, false, false);
        assert_eq!(c.tour(), tour + 8);
    }

    #[test]
    fn seated_she_fades_from_her_edge_and_comes_back_on_it_if_it_is_still_there() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        let sitting = at(&mut look, &mut c, 100, false, false);
        assert!(matches!(sitting.drawing, Drawing::Perch(cell) if cell.strip == Strip::Perch));
        let seat_line = |picture: &Picture| {
            let Drawing::Perch(cell) = picture.drawing else { panic!("{picture:?}") };
            let uv = cell.strip.seat_uv();
            picture.rect.min + Vec2::new(picture.rect.size.x * uv.x, picture.rect.size.y * uv.y)
        };
        look.vanish(&mut c, 100);
        let leave = sheets::leaving().len();
        for frame in 100..100 + leave {
            let picture = at(&mut look, &mut c, frame, false, false);
            assert_eq!(
                picture.drawing,
                Drawing::Perch(sheets::Cell { strip: Strip::PerchLeave, index: (frame - 100) as u32 / 2 })
            );
            assert!((seat_line(&picture) - EDGE).length() <= 1.0, "sur son bord");
            assert!(!look.is_over(&c, seat_line(&picture) - Vec2::new(0.0, 40.0)));
        }
        at(&mut look, &mut c, 100 + leave, false, false);
        assert!(look.is_gone());
        assert!(c.perched().is_some(), "toujours posée, cachée, comme avant");
        // montrée, toujours posée : elle réapparaît sur son bord, puis sa boucle du début
        look.appear(&mut c, 300);
        let arrive = sheets::arriving().len();
        for frame in 300..300 + arrive {
            let picture = at(&mut look, &mut c, frame, false, false);
            assert_eq!(
                picture.drawing,
                Drawing::Perch(sheets::Cell { strip: Strip::PerchArrive, index: (frame - 300) as u32 / 2 })
            );
            assert!(!look.is_here());
        }
        let back = at(&mut look, &mut c, 300 + arrive, false, false);
        assert_eq!(back.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        assert!(look.is_here() && (seat_line(&back) - EDGE).length() <= 1.0);
        // sa fenêtre bouge pendant qu'elle y apparaît ou s'en efface : elle la suit
        let moved = EDGE + Vec2::new(40.0, -20.0);
        look.vanish(&mut c, 400);
        c.ride(Perch { spot: SPOT, feet: moved });
        assert!((seat_line(&at(&mut look, &mut c, 404, false, false)) - moved).length() <= 1.0);
        look.appear(&mut c, 406);
        c.ride(Perch { spot: SPOT, feet: EDGE });
        assert!((seat_line(&at(&mut look, &mut c, 408, false, false)) - EDGE).length() <= 1.0);
        at(&mut look, &mut c, 420, false, false);
        assert!(look.is_here());

        // cachée assise, puis sa fenêtre part pendant qu'elle est cachée : montrée, elle se
        // matérialise debout, ses pieds là où elle était assise
        look.vanish(&mut c, 500);
        at(&mut look, &mut c, 500 + leave, false, false);
        assert!(look.is_gone());
        c.take_off();
        c.step(0.0, 700);
        look.appear(&mut c, 700);
        let standing = at(&mut look, &mut c, 700, false, false);
        assert_eq!(life_cell(&standing), Some(life::Cell { strip: life::Strip::Arrive, index: 0 }));
        assert!((c.feet() - EDGE).length() < 0.5);
    }

    #[test]
    fn hidden_while_she_lands_or_takes_off_she_fades_from_the_drawing_shown() {
        // en train d'arriver sur son bord : l'arrivée assise à l'envers, du dessin montré
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        at(&mut look, &mut c, 0, false, false);
        land(&mut look, &mut c, 1);
        assert_eq!(
            at(&mut look, &mut c, 9, false, false).drawing,
            Drawing::Perch(sheets::Cell { strip: Strip::PerchArrive, index: 4 })
        );
        look.vanish(&mut c, 9);
        assert_eq!(
            at(&mut look, &mut c, 11, false, false).drawing,
            Drawing::Perch(sheets::Cell { strip: Strip::PerchArrive, index: 3 })
        );
        at(&mut look, &mut c, 18, false, false);
        assert!(look.is_gone());

        // en train d'en repartir : son départ continue, là où il en était
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        c.take_off();
        look.update(&mut c, 100, false, false);
        at(&mut look, &mut c, 110, false, false);
        look.vanish(&mut c, 110);
        assert_eq!(
            at(&mut look, &mut c, 112, false, false).drawing,
            Drawing::Perch(sheets::Cell { strip: Strip::PerchLeave, index: 6 })
        );
        at(&mut look, &mut c, 124, false, false);
        assert!(look.is_gone());
        // montrée : sa fenêtre est loin, elle se matérialise debout
        look.appear(&mut c, 200);
        assert!(life_cell(&at(&mut look, &mut c, 200, false, false)).is_some());
    }

    #[test]
    fn without_her_life_sheets_she_comes_and_goes_at_once_in_every_wardrobe() {
        for stand in [false, true] {
            for perch in [false, true] {
                for life in [false, true] {
                    let case = format!("debout {stand}, assise {perch}, vie {life}");
                    let mut look = Look::new(wardrobe(stand, perch, life), HEIGHT);
                    let mut c = capri(&look);
                    c.step(0.0, 0);
                    look.enter(&mut c, 0);
                    let animated = stand && life;
                    assert_eq!(look.has_life(), animated, "{case}");
                    let first = at(&mut look, &mut c, 0, false, false);
                    assert_eq!(life_cell(&first).is_some(), animated, "{case} : {first:?}");
                    assert_eq!(look.is_here(), !animated, "{case}");
                    let here = at(&mut look, &mut c, 40, false, false);
                    assert!(look.is_here() && life_cell(&here).is_none(), "{case}");
                    // debout, puis posée sur une fenêtre
                    for (frame, perched) in [(40, false), (200, true)] {
                        if perched {
                            land(&mut look, &mut c, frame - 100);
                            at(&mut look, &mut c, frame, false, false);
                        }
                        look.vanish(&mut c, frame);
                        assert_eq!(look.is_gone(), !animated, "{case}, posée {perched}");
                        at(&mut look, &mut c, frame + 30, false, false);
                        assert!(look.is_gone(), "{case}, posée {perched}");
                        look.appear(&mut c, frame + 40);
                        assert_eq!(look.is_here(), !animated, "{case}, posée {perched}");
                        at(&mut look, &mut c, frame + 80, false, false);
                        assert!(look.is_here(), "{case}, posée {perched}");
                        if !animated {
                            // comme avant : son dessin d'avant, ses planches debout ou assises
                            let picture = look.picture(&c);
                            assert!(!matches!(picture.drawing, Drawing::Life(_)), "{case} : {picture:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn landed_while_she_fades_shown_seated_then_off_her_edge_she_floats_again() {
        // on la traîne et on la cache avant de la lâcher : elle s'efface debout, son tour arrêté
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        c.grab(c.position() + Vec2::new(50.0, 50.0), 0.0);
        at(&mut look, &mut c, 100, false, false);
        look.vanish(&mut c, 100);
        // lâchée doucement sur le haut d'une fenêtre pendant qu'elle s'efface : elle s'y pose
        c.step(0.0, 105);
        c.release(10.0);
        c.land(Perch { spot: SPOT, feet: EDGE });
        look.update(&mut c, 105, false, false);
        at(&mut look, &mut c, 140, false, false);
        assert!(look.is_gone());
        // montrée, toujours posée : elle réapparaît assise
        c.step(0.0, 500);
        look.appear(&mut c, 500);
        at(&mut look, &mut c, 600, false, false);
        assert!(look.is_here() && look.is_seated(&c));
        // sa fenêtre se ferme : debout, elle flotte de nouveau, sa boucle et son tour avancent
        c.take_off();
        look.update(&mut c, 600, false, false);
        let picture = at(&mut look, &mut c, 640, false, false);
        let tour = c.tour();
        let later = at(&mut look, &mut c, 700, false, false);
        assert_eq!(c.tour(), tour + 60, "son tour avance");
        assert!(matches!(later.drawing, Drawing::Stand(_)));
        let heights: std::collections::HashSet<i64> =
            (700..760).map(|frame| at(&mut look, &mut c, frame, false, false).rect.min.y as i64).collect();
        assert!(heights.len() > 1, "elle flotte : {heights:?} ; {picture:?}");
    }

    #[test]
    fn her_edge_moving_then_leaving_while_she_fades_she_stays_where_it_went() {
        let seat_line = |picture: &Picture| {
            let Drawing::Perch(cell) = picture.drawing else { panic!("{picture:?}") };
            let uv = cell.strip.seat_uv();
            picture.rect.min + Vec2::new(picture.rect.size.x * uv.x, picture.rect.size.y * uv.y)
        };
        let moved = EDGE + Vec2::new(300.0, 0.0);
        // cachée assise : elle s'efface de son bord ; il glisse, puis sa fenêtre se ferme
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        look.vanish(&mut c, 100);
        c.step(0.0, 104);
        c.ride(Perch { spot: SPOT, feet: moved });
        look.update(&mut c, 104, false, false);
        assert!((seat_line(&look.picture(&c)) - moved).length() <= 1.0, "elle suit son bord");
        c.step(0.0, 106);
        c.take_off();
        assert!((seat_line(&look.picture(&c)) - moved).length() <= 1.0, "avant le pas suivant");
        look.update(&mut c, 106, false, false);
        let picture = look.picture(&c);
        assert!((seat_line(&picture) - moved).length() <= 1.0, "là où il est allé : {picture:?}");

        // cachée pendant qu'elle arrive sur son bord : de même
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        at(&mut look, &mut c, 0, false, false);
        land(&mut look, &mut c, 1);
        at(&mut look, &mut c, 9, false, false);
        look.vanish(&mut c, 9);
        c.ride(Perch { spot: SPOT, feet: moved });
        look.update(&mut c, 10, false, false);
        c.take_off();
        look.update(&mut c, 11, false, false);
        assert!((seat_line(&look.picture(&c)) - moved).length() <= 1.0);
    }

    /// Là où passe sa ligne d'assise, dessinée assise.
    fn seat_line(picture: &Picture) -> Vec2 {
        let Drawing::Perch(cell) = picture.drawing else { panic!("{picture:?}") };
        let uv = cell.strip.seat_uv();
        picture.rect.min + Vec2::new(picture.rect.size.x * uv.x, picture.rect.size.y * uv.y)
    }

    #[test]
    fn changing_place_she_fades_out_here_then_appears_there_never_hidden() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        // d'un bord à un autre : son départ assis là où elle est, la fenêtre montrée
        assert!(look.travel(&mut c, 100));
        let leave = sheets::leaving().len();
        for frame in 100..100 + leave {
            let picture = at(&mut look, &mut c, frame, false, false);
            assert_eq!(
                picture.drawing,
                Drawing::Perch(sheets::Cell { strip: Strip::PerchLeave, index: (frame - 100) as u32 / 2 })
            );
            assert!((seat_line(&picture) - EDGE).length() <= 1.0);
            assert!(look.is_traveling() && look.is_away() && !look.is_hidden() && !look.is_gone());
            assert!(!look.is_over(&c, seat_line(&picture) - Vec2::new(0.0, 40.0)), "rien ne l'attrape");
        }
        assert!(!look.travel(&mut c, 110), "déjà en chemin");
        at(&mut look, &mut c, 100 + leave, false, false);
        assert!(look.is_between() && !look.is_gone(), "effacée, pas cachée : la fenêtre reste");
        // déplacée sur un autre bord : elle y apparaît assise, puis sa boucle, du début
        let there = Vec2::new(2000.0, 900.0);
        c.set_on(Perch { spot: Spot { window: 9, ..SPOT }, feet: there });
        look.arrive(&mut c, 100 + leave);
        assert!(!look.is_between() && !look.is_traveling() && !look.is_here());
        let arrive = sheets::arriving().len();
        let start = 100 + leave;
        for frame in start..start + arrive {
            let picture = at(&mut look, &mut c, frame, false, false);
            assert_eq!(
                picture.drawing,
                Drawing::Perch(sheets::Cell { strip: Strip::PerchArrive, index: (frame - start) as u32 / 2 })
            );
            assert!((seat_line(&picture) - there).length() <= 1.0, "là-bas, jamais ici");
        }
        let seated = at(&mut look, &mut c, start + arrive, false, false);
        assert_eq!(seated.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        assert!(look.is_here());

        // d'un bord à debout : son départ assis, puis ses planches de vie, et son tour reprend
        let start = start + arrive + 50;
        at(&mut look, &mut c, start, false, false);
        assert!(look.travel(&mut c, start));
        at(&mut look, &mut c, start + leave, false, false);
        assert!(look.is_between());
        let feet = Vec2::new(600.0, 1800.0);
        c.set_at(feet);
        look.arrive(&mut c, start + leave);
        let start = start + leave;
        let first = at(&mut look, &mut c, start, false, false);
        assert_eq!(life_cell(&first), Some(life::Cell { strip: life::Strip::Arrive, index: 0 }));
        assert!((c.feet() - feet).length() < 0.5);
        let tour = c.tour();
        let standing = at(&mut look, &mut c, start + life::arriving().len() + 30, false, false);
        assert!(matches!(standing.drawing, Drawing::Stand(_)) && look.is_here());
        assert_eq!(c.tour(), tour + 30, "son tour a repris à la fin de son arrivée");
    }

    /// Sur le côté `side` d'une fenêtre (n° 4), son point d'accroche en (1500, 900).
    fn side_of(side: Side) -> crate::perch::Lurk {
        let spot = crate::perch::FlankSpot { window: 4, side, along: 700.0, top: 200.0, height: 1400.0 };
        crate::perch::Lurk { spot, at: Vec2::new(1500.0, 900.0) }
    }

    /// Effacée pour changer de place, puis mise derrière le côté `side` d'une fenêtre : elle y
    /// arrive à l'image rendue.
    fn to_a_side(look: &mut Look, c: &mut Capri, side: Side) -> u64 {
        entered(look, c, 100);
        assert!(look.travel(c, 100));
        let mut frame = 100;
        while !look.is_between() {
            frame += 1;
            at(look, c, frame, false, false);
            assert!(frame < 300, "effacée");
        }
        c.lurk_on(side_of(side), look.lurk_fit().expect("ses planches debout"));
        look.arrive(c, frame);
        frame
    }

    /// Un pas : son corps avance d'un vingt-quatrième de seconde, puis son dessin.
    fn glide(look: &mut Look, c: &mut Capri, frame: u64) -> Picture {
        c.step(1.0 / 24.0, frame);
        look.update(c, frame, false, false);
        look.picture(c)
    }

    #[test]
    fn lurking_she_glides_out_leaning_cut_at_the_edge_then_back_and_never_her_life_sheets() {
        for side in Side::ALL {
            let mut look = dressed(true, true);
            let mut c = capri(&look);
            let mut frame = to_a_side(&mut look, &mut c, side);
            let o = side.outward();
            assert!(!look.is_here() && !look.is_traveling() && !look.is_hidden());
            // elle sort de derrière la fenêtre : sa boucle debout, penchée, coupée au bord ; sa
            // position glisse vers l'extérieur, sans jamais reculer
            let mut last = None;
            let mut steps = 0;
            while !look.is_here() {
                frame += 1;
                steps += 1;
                let picture = glide(&mut look, &mut c, frame);
                assert!(matches!(picture.drawing, Drawing::Stand(_)), "{side:?} : {picture:?}");
                let lean = picture.lean.expect("penchée");
                assert_eq!((lean.wall, lean.side, lean.angle), (1500.0, side, stand::lean(side)));
                if let Some(last) = last {
                    assert!((lean.pivot.x - last) * o >= 0.0, "{side:?} : elle recule");
                }
                last = Some(lean.pivot.x);
                // derrière la fenêtre, rien ne l'attrape
                assert!(!look.is_over(&c, Vec2::new(1500.0 - o * 3.0, lean.pivot.y)));
                assert!(steps < 40, "{side:?} : elle n'en finit pas de sortir");
            }
            assert!((steps as f32 / 24.0 - crate::body::GLIDE).abs() < 0.1, "{side:?} : {steps} pas");
            // sortie : sa hanche derrière le bord, ses yeux dehors, à `EYE_MARGIN` du bord au plus près
            let picture = look.picture(&c);
            let lean = picture.lean.expect("penchée");
            let k = picture.rect.size.x / stand::CELL_WIDTH as f32;
            assert!(((lean.pivot.x - 1500.0) * o - stand::hip_out(side) * k).abs() <= 1.0, "{lean:?}");
            assert!((lean.pivot.y - 900.0).abs() <= 1.0);
            let ((x0, y0), (x1, y1)) = stand::EYES;
            for (x, y) in [(x0, y0), (x1 + 1, y0), (x1 + 1, y1 + 1), (x0, y1 + 1)] {
                let eye = lean.turned(picture.rect.min + Vec2::new(x as f32, y as f32) * k);
                assert!((eye.x - 1500.0) * o >= stand::EYE_MARGIN * k - 1.0, "{side:?} : {eye:?}");
                assert!(lean.shows(eye));
            }
            // ce qui se voit d'elle l'attrape : le milieu de ses yeux
            let eyes = Vec2::new((x0 + x1 + 1) as f32 / 2.0, (y0 + y1 + 1) as f32 / 2.0);
            let face = lean.turned(picture.rect.min + eyes * k);
            assert!(look.is_over(&c, face), "{side:?}");
            // son bandeau : au-dessus de sa tête penchée, rien derrière la fenêtre
            let shown = look.shown(&c);
            assert!(shown.min.x <= face.x && face.x <= shown.max().x, "{side:?} : {shown:?}");
            assert!((shown.min.x - 1500.0) * o >= -0.01 && (shown.max().x - 1500.0) * o >= -0.01);
            assert!(shown.min.y < face.y - 50.0 * k && shown.max().y > face.y, "{side:?} : {shown:?}");
            // ni sourire, ni sommeil, ni marche : un clic n'y fait rien
            assert_eq!(look.poke(&mut c, frame), None);
            assert!(!look.cheer(&mut c, frame) && !look.walk(&mut c, frame, 100.0));
            // elle parle : sa bouche, penchée avec elle
            c.step(1.0 / 24.0, frame + 1);
            look.update(&mut c, frame + 1, true, false);
            let talking = look.picture(&c);
            assert!(talking.mouth.is_some() && talking.lean == Some(lean));
            // elle repart : elle se retire derrière la fenêtre, toujours sa boucle, jamais sa vie
            frame += 2;
            assert!(look.travel(&mut c, frame));
            let mut steps = 0;
            while !look.is_between() {
                frame += 1;
                steps += 1;
                let picture = glide(&mut look, &mut c, frame);
                assert!(
                    matches!(picture.drawing, Drawing::Stand(_)) && picture.lean.is_some(),
                    "{picture:?}"
                );
                assert!(look.is_traveling() && !look.is_here());
                assert!(steps < 40);
            }
            // partie : rien ne se voit d'elle
            let shown = look.shown(&c);
            assert!(shown.size.x.abs() < 0.01, "{side:?} : {shown:?}");
        }
    }

    #[test]
    fn hidden_or_shown_while_she_glides_she_turns_back_from_where_she_is() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let mut frame = to_a_side(&mut look, &mut c, Side::Left);
        for _ in 0..5 {
            frame += 1;
            glide(&mut look, &mut c, frame);
        }
        let out = c.lurk_out().unwrap();
        assert!(out > 0.2 && out < 0.9, "{out}");
        // cachée en sortant : elle se retire d'où elle en est
        let before = c.position();
        look.vanish(&mut c, frame);
        assert!(look.is_hidden() && !look.is_gone());
        frame += 1;
        glide(&mut look, &mut c, frame);
        assert!(c.lurk_out().unwrap() < out && (c.position() - before).length() < 30.0);
        // montrée en se retirant : elle ressort
        look.appear(&mut c, frame);
        assert!(!look.is_hidden());
        let back = c.lurk_out().unwrap();
        frame += 1;
        glide(&mut look, &mut c, frame);
        assert!(c.lurk_out().unwrap() > back);
        // cachée jusqu'au bout : partie, la fenêtre peut se cacher ; montrée, elle ressort
        look.vanish(&mut c, frame);
        while !look.is_gone() {
            frame += 1;
            glide(&mut look, &mut c, frame);
            assert!(frame < 1000);
        }
        assert_eq!(c.lurk_out(), Some(0.0));
        look.appear(&mut c, frame);
        while !look.is_here() {
            frame += 1;
            glide(&mut look, &mut c, frame);
            assert!(frame < 1000);
        }
        assert_eq!(c.lurk_out(), Some(1.0));
    }

    #[test]
    fn her_side_gone_while_she_glides_she_stands_whole_where_she_is() {
        // en sortant : debout, là, tout de suite
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let mut frame = to_a_side(&mut look, &mut c, Side::Right);
        frame += 3;
        glide(&mut look, &mut c, frame);
        c.take_off();
        let picture = glide(&mut look, &mut c, frame + 1);
        assert!(look.is_here() && picture.lean.is_none() && matches!(picture.drawing, Drawing::Stand(_)));
        // en se retirant pour partir : debout, elle s'efface comme d'habitude, et part
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let mut frame = to_a_side(&mut look, &mut c, Side::Right);
        while !look.is_here() {
            frame += 1;
            glide(&mut look, &mut c, frame);
        }
        assert!(look.travel(&mut c, frame));
        frame += 2;
        glide(&mut look, &mut c, frame);
        c.take_off();
        frame += 1;
        let picture = glide(&mut look, &mut c, frame);
        assert!(life_cell(&picture).is_some_and(|cell| cell.strip == life::Strip::Leave), "{picture:?}");
        while !look.is_between() {
            frame += 1;
            glide(&mut look, &mut c, frame);
            assert!(frame < 1000);
        }
        // partie derrière sa fenêtre, qui s'en va : rien à montrer, comme d'habitude
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let mut frame = to_a_side(&mut look, &mut c, Side::Left);
        look.vanish(&mut c, frame);
        while !look.is_gone() {
            frame += 1;
            glide(&mut look, &mut c, frame);
        }
        c.take_off();
        glide(&mut look, &mut c, frame + 1);
        assert!(look.is_gone());
    }

    #[test]
    fn lurking_her_click_mask_stops_short_of_the_edge_by_its_turned_margin() {
        // la dilatation de son masque (un carré), penchée de 22°, porte de 3·(cos + sin) en largeur
        let edge = lurk_edge(stand::lean(Side::Left));
        assert!((edge - 3.0 * (22f32.to_radians().cos() + 22f32.to_radians().sin())).abs() < 1e-4);
        assert!((edge - 3.906).abs() < 1e-3 && lurk_edge(0.0) == MASK_GROW);
        for side in Side::ALL {
            let mut look = dressed(true, true);
            let mut c = capri(&look);
            let mut frame = to_a_side(&mut look, &mut c, side);
            while !look.is_here() {
                frame += 1;
                glide(&mut look, &mut c, frame);
            }
            // son masque est plein : seule la distance au bord décide, à la hauteur de sa hanche
            let picture = look.picture(&c);
            let k = picture.rect.size.x / stand::CELL_WIDTH as f32;
            let lean = picture.lean.unwrap();
            let at = |gap: f32| Vec2::new(1500.0 + side.outward() * gap, lean.pivot.y);
            assert!(!look.is_over(&c, at(edge * k - 0.2)), "{side:?} : dans la marge");
            assert!(look.is_over(&c, at(edge * k + 0.2)), "{side:?} : au-delà");
        }
    }

    #[test]
    fn grabbed_lurking_she_straightens_around_what_the_cursor_holds() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let mut frame = to_a_side(&mut look, &mut c, Side::Left);
        while !look.is_here() {
            frame += 1;
            glide(&mut look, &mut c, frame);
        }
        let picture = look.picture(&c);
        let lean = picture.lean.unwrap();
        // le curseur sur ses yeux
        let k = picture.rect.size.x / stand::CELL_WIDTH as f32;
        let held = Vec2::new(65.0, 115.0);
        let cursor = lean.turned(picture.rect.min + held * k);
        let origin = look.grab_origin(&c, cursor).expect("guettant");
        c.grab_at(cursor, origin, 0.0);
        let after = at(&mut look, &mut c, frame + 1, false, false);
        assert!(after.lean.is_none() && c.lurking().is_none() && c.is_held());
        // le même endroit d'elle sous le curseur, à un pixel près (son image arrondie)
        let now = after.rect.min + held * k;
        assert!((now - cursor).length() <= 1.5, "{now:?} / {cursor:?}");
    }

    #[test]
    fn grabbed_lurking_near_the_edges_of_the_screen_what_the_cursor_holds_stays_under_it() {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        for height in [240.0, 480.0, 840.0] {
            let mut look = Look::new(wardrobe(true, true, true), height);
            let (need, fit) = (look.lurk_stature().unwrap(), look.lurk_fit().unwrap());
            // son côté au plus près du bord de l'écran qu'il permet, et le haut de la fenêtre tout en haut
            let left = need.on(Side::Left).left + 0.5;
            let right = screen.max().x - need.on(Side::Right).right - 0.5;
            let windows = [
                crate::perch::Seen {
                    id: 1,
                    rect: Rect::new(Vec2::new(left, 0.0), Vec2::new(1500.0, 1964.0)),
                },
                crate::perch::Seen {
                    id: 2,
                    rect: Rect::new(Vec2::new(right - 1500.0, 0.0), Vec2::new(1500.0, 1964.0)),
                },
            ];
            for (index, window) in windows.iter().enumerate() {
                let flanks = crate::perch::flanks(&windows[index..=index], screen, need);
                for flank in &flanks {
                    for y in [flank.lo, flank.hi] {
                        let mut c = Capri::new(look.standing_size(), screen, Vec2::new(1000.0, 800.0));
                        c.set_feet(look.standing_feet());
                        c.step(0.0, 0);
                        look.enter(&mut c, 0);
                        c.lurk_on(flank.at(y), fit);
                        c.emerge();
                        for frame in 1..40 {
                            glide(&mut look, &mut c, frame);
                        }
                        assert!(look.is_here(), "{window:?}");
                        let picture = look.picture(&c);
                        let lean = picture.lean.unwrap();
                        let k = picture.rect.size.x / stand::CELL_WIDTH as f32;
                        for held in [
                            Vec2::new(66.0, 30.0),
                            Vec2::new(66.0, 60.0),
                            Vec2::new(40.0, 80.0),
                            Vec2::new(66.0, 115.0),
                        ] {
                            let cursor = lean.turned(picture.rect.min + held * k);
                            if !look.is_over(&c, cursor) {
                                continue;
                            }
                            let mut h = c.clone();
                            assert!(look.hold(&mut h, &Grab { at: cursor, since: 0.0, trail: Vec::new() }));
                            let now = h.position() + held * k;
                            assert!(
                                (now - cursor).length() < 1e-2,
                                "{height} px, {:?} : le point tenu à {} px du curseur",
                                flank.side,
                                (now - cursor).length()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn she_glides_without_her_life_sheets_and_cannot_lurk_without_her_standing_ones() {
        let mut look = Look::new(wardrobe(true, true, false), HEIGHT);
        let mut c = capri(&look);
        at(&mut look, &mut c, 0, false, false);
        assert!(look.travel(&mut c, 0) && look.is_between(), "partie d'un coup");
        c.lurk_on(side_of(Side::Right), look.lurk_fit().unwrap());
        look.arrive(&mut c, 0);
        assert!(!look.is_here(), "elle glisse quand même");
        let mut frame = 0;
        while !look.is_here() {
            frame += 1;
            glide(&mut look, &mut c, frame);
            assert!(frame < 40);
        }
        // sans ses planches debout, rien
        let bare = Look::new(wardrobe(false, true, true), HEIGHT);
        assert!(bare.lurk_fit().is_none() && bare.lurk_stature().is_none());
    }

    #[test]
    fn what_she_needs_to_lurk_scales_with_her_and_hides_her_whole_at_the_start() {
        let fit = lurk_fit();
        for height in [240.0, 600.0, 840.0] {
            let need = lurking_stature(height);
            let k = stand::scale(height);
            for side in Side::ALL {
                let bounds = stand::lurk_bounds(side);
                let reach = need.on(side);
                assert!((reach.up + bounds.min.y * k - ROUNDING).abs() < 1e-3);
                assert_eq!(reach.left.min(reach.right), 0.0, "rien derrière la fenêtre");
                let out = (-bounds.min.x).max(bounds.max().x) * k + ROUNDING;
                assert!((reach.left.max(reach.right) - out).abs() < 1e-3);
            }
        }
        // cachée au départ de sa glissade : chaque coin de sa silhouette penchée derrière le bord
        for side in Side::ALL {
            let i = side.index();
            let w = stand::CELL_WIDTH as f32;
            let shift = (fit.hip_out[i] - fit.slide[i]) * w; // hanche dehors, glissée vers l'intérieur
            let hip = Vec2::new(stand::HIP.0, stand::HIP.1);
            for (band, &(x0, x1)) in stand::OUTLINE.iter().enumerate() {
                let y = (stand::TOP + band as u32 * stand::BAND) as f32;
                for corner in [Vec2::new(x0 as f32, y), Vec2::new((x1 + 1) as f32, y + stand::BAND as f32)] {
                    let p = stand::turn(corner, hip, stand::lean(side));
                    // en x, depuis le bord, vers l'extérieur
                    let out = (p.x - stand::HIP.0) * side.outward() + shift;
                    assert!(out < 0.0, "{side:?} : {corner:?} dépasse de {out}");
                }
            }
        }
    }

    #[test]
    fn hidden_on_the_way_she_finishes_fading_where_she_was_and_comes_back_there() {
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        let before = entered(&mut look, &mut c, 100);
        let position = c.position();
        assert!(look.travel(&mut c, 100));
        let leave = |index| Some(life::Cell { strip: life::Strip::Leave, index });
        assert_eq!(life_cell(&at(&mut look, &mut c, 110, false, false)), leave(5));
        // cachée : la même suite continue (elle ne revient pas sur ses pas), cachée cette fois
        look.vanish(&mut c, 110);
        assert!(look.is_hidden() && !look.is_traveling());
        assert_eq!(life_cell(&at(&mut look, &mut c, 112, false, false)), leave(6));
        // montrée de nouveau pendant qu'elle s'efface : elle revient sur ses pas, là où elle était
        look.appear(&mut c, 112);
        assert!(!look.is_hidden() && !look.is_traveling());
        assert_eq!(life_cell(&at(&mut look, &mut c, 114, false, false)), leave(5));
        at(&mut look, &mut c, 140, false, false);
        assert!(look.is_here() && c.position() == position, "pas déplacée");
        // cachée une fois effacée, entre deux places : partie, et elle réapparaîtra ici
        assert!(look.travel(&mut c, 200));
        at(&mut look, &mut c, 224, false, false);
        assert!(look.is_between());
        look.vanish(&mut c, 224);
        assert!(look.is_gone() && !look.is_between());
        look.arrive(&mut c, 225);
        assert!(look.is_gone(), "cachée, elle n'apparaît pas ailleurs");
        look.appear(&mut c, 300);
        let back = at(&mut look, &mut c, 300, false, false);
        assert!(life_cell(&back).is_some() && c.position() == position, "elle réapparaît là où elle était");
        assert_eq!(back.rect.min.x, life::over_stand(before.rect).min.x);
        // montrer ce qui n'est pas caché ne change rien, en chemin non plus
        at(&mut look, &mut c, 400, false, false);
        assert!(look.travel(&mut c, 400));
        look.appear(&mut c, 401);
        assert!(look.is_traveling());
    }

    #[test]
    fn turning_back_on_the_way_she_reappears_where_she_was() {
        // assise : son départ de son bord, à l'envers, du dessin où elle en est
        let mut look = dressed(true, true);
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        assert!(look.travel(&mut c, 100));
        let cell = |picture: Picture| match picture.drawing {
            Drawing::Perch(cell) => (cell.strip, cell.index),
            other => panic!("{other:?}"),
        };
        assert_eq!(cell(at(&mut look, &mut c, 110, false, false)), (Strip::PerchLeave, 5));
        look.turn_back(&mut c, 110);
        assert!(!look.is_traveling() && !look.is_hidden() && !look.is_here());
        assert_eq!(cell(at(&mut look, &mut c, 112, false, false)), (Strip::PerchLeave, 4));
        let back = at(&mut look, &mut c, 130, false, false);
        assert!(look.is_here() && (seat_line(&back) - EDGE).length() <= 1.0, "sur son bord");
        // effacée, pas encore déplacée : elle réapparaît là ; ni cachée, ni en chemin, sans effet
        assert!(look.travel(&mut c, 200));
        at(&mut look, &mut c, 200 + sheets::leaving().len(), false, false);
        assert!(look.is_between());
        look.turn_back(&mut c, 224);
        assert!(!look.is_between() && !look.is_here());
        let again = at(&mut look, &mut c, 224, false, false);
        assert_eq!(cell(again), (Strip::PerchArrive, 0));
        let presence = look.presence;
        look.turn_back(&mut c, 225);
        assert_eq!(look.presence, presence);
        look.vanish(&mut c, 300);
        look.turn_back(&mut c, 301);
        assert!(look.is_hidden(), "cachée, elle ne revient pas pour autant");
    }

    #[test]
    fn without_her_life_sheets_she_changes_place_at_once_and_leaves_nothing_behind() {
        let mut look = Look::new(wardrobe(true, true, false), HEIGHT);
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        assert!(look.travel(&mut c, 100));
        assert!(look.is_between(), "effacée d'un coup");
        // debout ailleurs : rien de sa pose assise ne reste à finir là où elle était
        c.set_at(Vec2::new(600.0, 1800.0));
        look.arrive(&mut c, 100);
        assert!(look.is_here());
        for frame in 100..130 {
            assert!(
                matches!(at(&mut look, &mut c, frame, false, false).drawing, Drawing::Stand(_)),
                "{frame}"
            );
        }
        // assise ailleurs : sa boucle assise, d'un coup, là-bas
        assert!(look.travel(&mut c, 130));
        let there = Vec2::new(2000.0, 900.0);
        c.set_on(Perch { spot: SPOT, feet: there });
        look.arrive(&mut c, 130);
        let seated = at(&mut look, &mut c, 131, false, false);
        assert_eq!(seated.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        assert!((seat_line(&seated) - there).length() <= 1.0);
    }

    #[test]
    fn a_run_plays_once_either_way() {
        let run = Run::new(Clip::Leave, 10, 24);
        assert_eq!((run.position(10), run.position(33), run.position(90)), (0, 23, 23));
        assert_eq!((run.left(10), run.left(33), run.left(34)), (24, 1, 0));
        let back = run.turned(15);
        assert_eq!((back.position(15), back.position(20), back.position(21)), (5, 0, 0));
        assert_eq!((back.left(15), back.left(20), back.left(21)), (6, 1, 0));
        let again = back.turned(17);
        assert_eq!((again.position(17), again.position(18), again.left(17)), (3, 4, 21));
    }

    // --- ses gestes, tenue en l'air, sa marche ---

    /// Toutes ses planches de gestes et de variantes, ou aucune.
    fn every_move(all: bool) -> Moves {
        let body = || block(life::CELL_WIDTH, sheets::CELL_HEIGHT, (30, 15, 210, 370));
        Moves {
            walk: all.then(body),
            dangle: all.then(body),
            glitch: all.then(body),
            grin: all.then(body),
            perch_glitch: all,
            perch_blink_glitch: all,
            stand_blink_glitch: all,
        }
    }

    /// Debout et assise, ses planches de vie, et ses gestes.
    fn moving() -> Look {
        let mut wardrobe = wardrobe(true, true, true);
        wardrobe.moves = every_move(true);
        Look::new(wardrobe, HEIGHT)
    }

    /// Le dessin de ses planches de vie : (planche, case).
    fn life_of(picture: &Picture) -> Option<(life::Strip, u32)> {
        life_cell(picture).map(|cell| (cell.strip, cell.index))
    }

    /// Une prise faite à l'appui même (aucun pas entre l'appui et la prise).
    fn grab_at(at: Vec2, since: f64) -> Grab {
        Grab { at, since, trail: Vec::new() }
    }

    /// Le curseur va de `cursor` par pas de `step`, de l'image `from` à l'image `to` : sa mèche
    /// s'en approche à chaque pas sans jamais s'en éloigner, son corps bouge à chaque pas, et sa
    /// planche en l'air tourne, tenue en deux depuis l'image `since`. Rend le curseur, et l'écart
    /// de sa mèche à la fin.
    fn drag(
        look: &mut Look,
        c: &mut Capri,
        cursor: Vec2,
        step: Vec2,
        frames: std::ops::Range<u64>,
        since: u64,
    ) -> (Vec2, f32) {
        let mut cursor = cursor;
        let mut gap = (look.grip(c).expect("elle pend") - cursor).length();
        for frame in frames {
            let before = c.position();
            cursor += step;
            c.drag_to(cursor, frame as f64 / 24.0);
            let picture = at(look, c, frame, false, false);
            assert_eq!(
                life_of(&picture),
                Some((life::Strip::Dangle, ((frame - since) % 48) as u32 / 2)),
                "image {frame}"
            );
            assert_eq!(picture.mouth, None);
            assert!(c.position() != before, "image {frame} : son corps n'a pas bougé");
            let now = (look.grip(c).expect("elle pend") - cursor).length();
            assert!(now <= gap.max(1.5) + 1.5, "image {frame} : sa mèche s'éloigne ({gap} → {now})");
            gap = now;
        }
        (cursor, gap)
    }

    #[test]
    fn held_she_dangles_and_glides_until_her_grip_is_under_the_cursor_and_keeps_it_in_flight() {
        let mut look = moving();
        let mut c = capri(&look);
        let before = entered(&mut look, &mut c, 100);
        // attrapée par les pieds : son corps ne bouge pas, elle pend
        let cursor = before.rect.min + Vec2::new(100.0, 600.0);
        assert!(look.is_over(&c, cursor));
        let position = c.position();
        assert!(look.hold(&mut c, &grab_at(cursor, 100.0 / 24.0)));
        assert_eq!(c.position(), position, "pas de saut à la prise");
        let held = at(&mut look, &mut c, 101, false, false);
        assert_eq!(life_of(&held), Some((life::Strip::Dangle, 0)));
        assert_eq!(held.rect, life::over_stand(c.pose().rect), "par-dessus son image debout");
        assert!(look.is_dangling(&c) && look.is_here());
        // le curseur l'emmène : elle le suit à chaque pas, et sa mèche finit sous lui
        let (cursor, gap) = drag(&mut look, &mut c, cursor, Vec2::new(7.0, -3.0), 102..260, 101);
        assert!(gap <= 1.5, "sa mèche sous le curseur : {gap}");
        // curseur immobile : elle ne bouge pas
        let still = c.position();
        c.drag_to(cursor, 10.82);
        assert_eq!(c.position(), still);
        // lancée : sa planche en l'air, étirée dans le sens de sa course
        c.drag_to(cursor + Vec2::new(400.0, 0.0), 10.87);
        c.release(10.88);
        assert!(matches!(c.stance(), Stance::Thrown { .. }));
        let flying = at(&mut look, &mut c, 260, false, false);
        assert_eq!(life_of(&flying).map(|(strip, _)| strip), Some(life::Strip::Dangle));
        assert_eq!(flying.rect, life::over_stand(c.pose().rect));
        // ce qu'on dit pendant qu'on la tient ne change rien : elle pend
        c.grab(cursor, 400.0);
        assert_eq!(
            life_of(&at(&mut look, &mut c, 261, true, false)).map(|(s, _)| s),
            Some(life::Strip::Dangle)
        );

        // assise, on l'attrape : elle se lève sous le curseur (comme avant), pend, puis glisse
        let mut look = moving();
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        let cursor = look.shown(&c).min + Vec2::new(60.0, 200.0);
        let origin = look.grab_origin(&c, cursor).expect("assise");
        assert!(look.hold(&mut c, &grab_at(cursor, 100.0 / 24.0)));
        assert_eq!(c.position(), origin);
        at(&mut look, &mut c, 101, false, false);
        assert!(look.is_dangling(&c) && c.perched().is_none());
        let (_, gap) = drag(&mut look, &mut c, cursor, Vec2::new(-4.0, -5.0), 102..220, 101);
        assert!(gap <= 1.5, "sa mèche sous le curseur : {gap}");
        // s'effaçant, rien ne l'attrape
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        look.vanish(&mut c, 100);
        let grab = grab_at(c.position(), 5.0);
        assert!(!look.hold(&mut c, &grab) && !c.is_held());
    }

    #[test]
    fn grabbed_near_an_edge_she_never_jumps_nor_waits() {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        for dangle in [false, true] {
            let mut wardrobe = wardrobe(true, true, true);
            wardrobe.moves = Moves {
                dangle: dangle.then(|| full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
                ..Moves::default()
            };
            let mut look = Look::new(wardrobe, HEIGHT);
            // debout en bas de l'écran, à sa place habituelle : prise de la tête aux pieds
            let mut c = capri(&look);
            let floor = Stature::standing(look.standing_size(), look.standing_feet(), HEIGHT);
            c.set_at(Vec2::new(1500.0, screen.max().y - crate::roam::FLOOR * HEIGHT - floor.reach.down));
            at(&mut look, &mut c, 0, false, false);
            for v in [0.2f32, 0.4, 0.6, 0.8, 0.95] {
                let mut held = c.clone();
                let before = held.position();
                let cursor = before + Vec2::new(held.size().x * 0.5, held.size().y * v);
                assert!(look.hold(&mut held, &grab_at(cursor, 0.0)));
                assert_eq!(held.position(), before, "en l'air {dangle}, prise à {v} : pas de saut");
                held.drag_to(cursor - Vec2::new(0.0, 1.0), 0.02);
                assert!(held.position().y < before.y, "en l'air {dangle}, prise à {v} : elle monte aussitôt");
            }
            // assise sur une fenêtre tout en haut de l'écran : prise à mi-corps ou aux pieds, elle
            // se lève sous le curseur et le suit dès qu'il descend
            let stature = look.stature();
            let top = stature.reach.up + 2.0;
            let mut c = capri(&look);
            c.step(0.0, 0);
            c.land(Perch { spot: SPOT, feet: Vec2::new(1500.0, top) });
            for frame in 0..40 {
                at(&mut look, &mut c, frame, false, false);
            }
            assert!(look.is_seated(&c));
            let shown = look.shown(&c);
            for v in [0.5f32, 0.9] {
                let mut held = c.clone();
                let cursor = shown.min + Vec2::new(shown.size.x * 0.5, shown.size.y * v);
                assert!(look.hold(&mut held, &grab_at(cursor, 0.0)));
                let at_grab = held.position();
                held.drag_to(cursor + Vec2::new(0.0, 1.0), 0.02);
                assert!(
                    held.position().y > at_grab.y,
                    "en l'air {dangle}, assise, prise à {v} : elle descend aussitôt"
                );
            }
        }
    }

    /// Tenue à l'image `frame`, par le milieu de son image debout.
    fn hold(look: &mut Look, c: &mut Capri, frame: u64) -> Vec2 {
        let picture = at(look, c, frame, false, false);
        let cursor = picture.rect.min + picture.rect.size * 0.5;
        assert!(look.hold(c, &grab_at(cursor, frame as f64 / 24.0)));
        at(look, c, frame, false, false);
        cursor
    }

    #[test]
    fn let_go_standing_she_rises_once_then_her_loop_and_on_an_edge_nothing_changes() {
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        hold(&mut look, &mut c, 100);
        c.release(100.0); // lâchée doucement, bien plus tard
        let start = at(&mut look, &mut c, 120, false, false);
        assert_eq!(look.gesture(), Some(Gesture::Rise));
        assert_eq!(life_of(&start), Some((life::Strip::Glitch, 0)));
        let (rect, tour, standing) = (start.rect, c.tour(), c.pose().rect);
        assert_eq!(rect, life::over_stand(standing));
        for frame in 120..140 {
            let picture = at(&mut look, &mut c, frame, false, false);
            assert_eq!(
                life_of(&picture),
                Some((life::Strip::Glitch, (frame - 120) as u32 / 2)),
                "image {frame}"
            );
            assert_eq!(picture.rect, rect, "elle ne flotte pas pendant qu'elle se relève");
            assert!(!look.is_grooving() && look.is_here());
        }
        // puis sa boucle, son tour repris là où il était
        let after = at(&mut look, &mut c, 140, false, false);
        assert!(matches!(after.drawing, Drawing::Stand(_)), "{after:?}");
        assert_eq!(look.gesture(), None);
        assert_eq!(c.tour(), tour);
        assert_eq!(c.pose().rect, standing, "pas de saut");
        assert_eq!(at(&mut look, &mut c, 150, false, false).drawing, Drawing::Stand(standing_cell(c.tour())));

        // lancée, elle pend en vol, puis se relève là où elle s'arrête
        hold(&mut look, &mut c, 200);
        let grip = c.position() + Vec2::new(10.0, 10.0);
        c.drag_to(grip + Vec2::new(300.0, -50.0), 200.0 / 24.0 + 0.04);
        c.release(200.0 / 24.0 + 0.05);
        assert!(matches!(c.stance(), Stance::Thrown { .. }));
        let mut frame = 201;
        while matches!(c.stance(), Stance::Thrown { .. }) {
            c.step(1.0 / 24.0, frame);
            look.update(&mut c, frame, false, false);
            if matches!(c.stance(), Stance::Thrown { .. }) {
                assert_eq!(life_of(&look.picture(&c)).map(|(s, _)| s), Some(life::Strip::Dangle));
            }
            frame += 1;
            assert!(frame < 600, "le lancer finit");
        }
        assert_eq!(look.gesture(), Some(Gesture::Rise));

        // lâchée sur le haut d'une fenêtre : elle s'y pose, comme avant
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        hold(&mut look, &mut c, 100);
        c.release(100.0);
        c.land(Perch { spot: SPOT, feet: EDGE });
        let landed = at(&mut look, &mut c, 101, false, false);
        assert_eq!(look.gesture(), None);
        assert!(
            matches!(landed.drawing, Drawing::Perch(cell) if cell.strip == Strip::PerchArrive),
            "{landed:?}"
        );
    }

    /// Le dessin de sa boucle debout au tour `tour`, sans clignement (essais hors des tours où elle
    /// cligne).
    fn standing_cell(tour: u64) -> stand::Cell {
        let sheet = if blinks_glitchy(tour / TOUR) {
            stand::standing_glitchy()
        } else {
            stand::standing(blinks(tour / TOUR))
        };
        sheet.at(tour)
    }

    #[test]
    fn a_click_makes_her_grin_standing_and_glitch_seated_and_nothing_else_moves() {
        let mut look = moving();
        let mut c = capri(&look);
        let before = entered(&mut look, &mut c, 100);
        let (position, tour) = (c.position(), c.tour());
        assert_eq!(look.poke(&mut c, 100), Some(Gesture::Grin));
        let grin = at(&mut look, &mut c, 100, false, false);
        let rect = grin.rect;
        assert_eq!(rect, life::over_stand(before.rect));
        for frame in 100..100 + Gesture::Grin.frames() {
            let picture = at(&mut look, &mut c, frame, false, false);
            // tenu en trois, deux tours
            assert_eq!(
                life_of(&picture),
                Some((life::Strip::Grin, ((frame - 100) % 48) as u32 / 3)),
                "image {frame}"
            );
            assert_eq!(picture.rect, rect);
            // jamais deux gestes à la fois
            assert_eq!(look.poke(&mut c, frame), None);
            assert!(!look.cheer(&mut c, frame));
        }
        let after = at(&mut look, &mut c, 196, false, false);
        assert!(matches!(after.drawing, Drawing::Stand(_)));
        assert_eq!(
            (c.position(), c.tour()),
            (position, tour),
            "pas déplacée, son tour repris là où il était"
        );
        assert_eq!(after.rect, before.rect);

        // assise : un éclat sur son bord, puis sa boucle de son dessin 0 ; elle reste posée
        land(&mut look, &mut c, 300);
        at(&mut look, &mut c, 400, false, false);
        assert_eq!(look.poke(&mut c, 400), Some(Gesture::PerchGlitch));
        for frame in 400..420 {
            let picture = at(&mut look, &mut c, frame, false, false);
            assert_eq!(
                picture.drawing,
                Drawing::Perch(sheets::Cell { strip: Strip::PerchGlitch, index: (frame - 400) as u32 / 2 })
            );
            assert!((seat_line(&picture) - EDGE).length() <= 1.0, "sur son bord");
            assert!(
                look.is_over(&c, seat_line(&picture) - Vec2::new(0.0, 40.0)),
                "le masque des cases larges"
            );
            assert!(!look.is_grooving());
        }
        let back = at(&mut look, &mut c, 420, false, false);
        assert_eq!(back.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        assert!(c.perched().is_some() && look.gesture().is_none());
    }

    #[test]
    fn talking_or_a_grab_stops_a_gesture_at_once_and_none_starts_while_we_talk() {
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        let tour = c.tour();
        assert_eq!(look.poke(&mut c, 100), Some(Gesture::Grin));
        at(&mut look, &mut c, 110, false, false);
        // elle parle : sa boucle et sa bouche, tout de suite, et son tour reprend
        let talking = at(&mut look, &mut c, 111, true, false);
        assert!(matches!(talking.drawing, Drawing::Stand(_)) && talking.mouth.is_some(), "{talking:?}");
        assert_eq!(look.gesture(), None);
        assert_eq!(at(&mut look, &mut c, 121, true, false).drawing, Drawing::Stand(standing_cell(tour + 10)));
        assert_eq!(c.tour(), tour + 10);
        // pendant qu'elle parle, ou qu'on lui parle, aucun geste ne commence
        assert_eq!(look.poke(&mut c, 121), None);
        at(&mut look, &mut c, 130, false, false);
        look.set_conversing(true);
        assert_eq!(look.poke(&mut c, 130), None);
        assert!(!look.cheer(&mut c, 130));
        look.set_conversing(false);
        // on lui parle pendant qu'elle sourit : il s'arrête au pas suivant
        assert_eq!(look.poke(&mut c, 140), Some(Gesture::Grin));
        look.set_conversing(true);
        at(&mut look, &mut c, 141, false, false);
        assert_eq!(look.gesture(), None);
        look.set_conversing(false);
        // une prise : elle pend tout de suite
        assert_eq!(look.poke(&mut c, 150), Some(Gesture::Grin));
        hold(&mut look, &mut c, 152);
        assert_eq!(look.gesture(), None);
        assert!(look.is_dangling(&c));

        // assise : elle parle pendant son éclat, sa boucle parlante repart de son dessin 0
        let mut look = moving();
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        assert!(look.fidget(&mut c, 100));
        at(&mut look, &mut c, 106, false, false);
        let talk = at(&mut look, &mut c, 107, true, false);
        assert_eq!(talk.drawing, Drawing::Perch(sheets::Cell { strip: Strip::PerchTalk, index: 0 }));
        // cachée pendant un éclat : son départ de son bord, de là
        at(&mut look, &mut c, 150, false, false);
        assert!(look.fidget(&mut c, 200));
        at(&mut look, &mut c, 204, false, false);
        look.vanish(&mut c, 204);
        assert_eq!(look.gesture(), None);
        let leaving = at(&mut look, &mut c, 204, false, false);
        assert_eq!(leaving.drawing, Drawing::Perch(sheets::Cell { strip: Strip::PerchLeave, index: 0 }));
    }

    #[test]
    fn when_claude_finishes_she_grins_standing_at_most_once_a_minute() {
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        assert!(look.cheer(&mut c, 100));
        assert_eq!(look.gesture(), Some(Gesture::Grin));
        at(&mut look, &mut c, 300, false, false);
        assert_eq!(look.gesture(), None);
        // moins d'une minute plus tard : rien ; puis de nouveau
        assert!(!look.cheer(&mut c, 100 + CHEER_EVERY - 1));
        assert!(look.gesture().is_none());
        assert!(look.cheer(&mut c, 100 + CHEER_EVERY));
        // un clic n'y compte pas
        at(&mut look, &mut c, 100 + CHEER_EVERY + 100, false, false);
        assert_eq!(look.poke(&mut c, 100 + CHEER_EVERY + 100), Some(Gesture::Grin));
        // assise, rien
        let mut look = moving();
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 100, false, false);
        assert!(!look.cheer(&mut c, 100));
        assert!(look.gesture().is_none());
        // en train d'apparaître : pas libre
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.enter(&mut c, 0);
        at(&mut look, &mut c, 5, false, false);
        assert!(!look.cheer(&mut c, 5));
    }

    #[test]
    fn while_her_greeting_waits_she_starts_no_gesture_of_her_own() {
        // debout : pas de sourire pour Claude Code tant que son bonjour attend ; un clic, si
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        look.set_greeting_due(true);
        assert!(!look.cheer(&mut c, 100), "le bonjour passe avant");
        assert!(look.gesture().is_none());
        assert_eq!(look.poke(&mut c, 100), Some(Gesture::Grin), "un clic est un geste de l'utilisateur");
        // son bonjour dit : de nouveau
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        look.set_greeting_due(true);
        assert!(!look.cheer(&mut c, 100));
        look.set_greeting_due(false);
        assert!(look.cheer(&mut c, 101), "pas compté comme un sourire : la minute ne commence pas");
        // tombée du ciel, elle se relève quand même : cela fait partie de l'intro
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        assert!(look.perform(&mut c, Act::Fall, 0));
        look.set_greeting_due(true);
        let mut frame = 0;
        while look.is_performing() {
            frame += 1;
            staged(&mut look, &mut c, frame);
        }
        assert_eq!(look.gesture(), Some(Gesture::Rise));
        // assise : pas d'éclat d'elle-même tant qu'il attend
        let mut look = moving();
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        let since = sheets::arriving().len();
        at(&mut look, &mut c, since + 3 * TOUR, false, false);
        look.set_greeting_due(true);
        assert!(!look.fidget(&mut c, since + 3 * TOUR));
        look.set_greeting_due(false);
        assert!(look.fidget(&mut c, since + 3 * TOUR));
    }

    #[test]
    fn she_fidgets_seated_only_when_free_and_without_music() {
        let mut look = moving();
        let mut c = capri(&look);
        assert!(!look.fidget(&mut c, 0), "debout");
        land(&mut look, &mut c, 0);
        at(&mut look, &mut c, 10, false, false);
        assert!(!look.fidget(&mut c, 10), "elle arrive encore sur son bord");
        let since = sheets::arriving().len();
        at(&mut look, &mut c, since + 5, false, true);
        assert!(!look.fidget(&mut c, since + 5), "la musique joue");
        at(&mut look, &mut c, since + TOUR + 5, false, true);
        assert!(look.is_grooving() && !look.fidget(&mut c, since + TOUR + 5));
        at(&mut look, &mut c, since + 3 * TOUR, false, false);
        assert!(look.fidget(&mut c, since + 3 * TOUR));
        assert!(!look.fidget(&mut c, since + 3 * TOUR + 1), "un seul à la fois");
    }

    #[test]
    fn her_glitchy_blink_comes_on_its_tour_when_its_sheet_is_there() {
        for with in [false, true] {
            let mut wardrobe = wardrobe(true, true, true);
            wardrobe.moves = Moves { stand_blink_glitch: with, perch_blink_glitch: with, ..Moves::default() };
            let mut look = Look::new(wardrobe, HEIGHT);
            let mut c = capri(&look);
            let mut glitchy = Vec::new();
            for tour in 0..16 {
                let frame = tour * TOUR + u64::from(stand::BLINK_FROM) + 2;
                let Drawing::Stand(cell) = at(&mut look, &mut c, frame, false, false).drawing else {
                    panic!()
                };
                if cell.strip == StandStrip::StandBlinkGlitch {
                    glitchy.push(tour);
                }
                assert_eq!(cell.strip != StandStrip::Stand, blinks(tour), "tour {tour}");
            }
            assert_eq!(glitchy, if with { vec![5, 13] } else { vec![] }, "debout");
            land(&mut look, &mut c, 16 * TOUR);
            let since = 16 * TOUR + sheets::arriving().len();
            let mut glitchy = Vec::new();
            for tour in 0..16 {
                let frame = since + tour * TOUR + u64::from(sheets::BLINK_FROM) + 2;
                let Drawing::Perch(cell) = at(&mut look, &mut c, frame, false, false).drawing else {
                    panic!()
                };
                if cell.strip == Strip::PerchBlinkGlitch {
                    glitchy.push(tour);
                }
                assert_eq!(cell.strip != Strip::Perch, blinks(tour), "tour {tour}");
            }
            assert_eq!(glitchy, if with { vec![5, 13] } else { vec![] }, "assise");
        }
        assert!(blinks(GLITCHY_BLINK), "c'est un tour où elle cligne");
    }

    #[test]
    fn she_walks_over_her_standing_image_then_her_loop_resumes() {
        let mut look = moving();
        let mut c = capri(&look);
        let before = entered(&mut look, &mut c, 100);
        let (tour, feet) = (c.tour(), c.feet());
        let to = feet.x - 300.0;
        assert!(look.walk(&mut c, 100, to));
        assert!(look.is_walking(&c) && c.is_walking());
        let speed = look.walk_speed().unwrap();
        let mut last = feet.x;
        let mut frame = 100;
        let mut seconds = 0.0;
        while c.is_walking() {
            c.step(1.0 / 24.0, frame);
            seconds += 1.0 / 24.0;
            look.update(&mut c, frame, false, false);
            let picture = look.picture(&c);
            if c.is_walking() {
                assert_eq!(
                    life_of(&picture),
                    Some((life::Strip::Walk, ((frame - 100) % 24) as u32 / 2)),
                    "image {frame}"
                );
                assert_eq!(picture.rect.min.y, life::over_stand(before.rect).min.y, "à la même hauteur");
                assert!(c.feet().x <= last, "jamais à reculons");
                assert!(!look.is_over(&c, Vec2::new(-1.0, -1.0)));
            }
            last = c.feet().x;
            frame += 1;
            assert!(frame < 2000);
        }
        assert!((seconds - 300.0 / f64::from(speed)).abs() < 0.1, "{seconds} s à {speed} px/s");
        assert!((c.feet().x - to).abs() < 1e-3, "{:?}", c.feet());
        let arrived = at(&mut look, &mut c, frame, false, false);
        assert!(matches!(arrived.drawing, Drawing::Stand(_)) && !look.is_walking(&c));
        // son tour a repris là où il était, à l'image de son arrivée (une image plus tôt)
        assert_eq!(c.tour(), tour + 1);
        // pas pendant qu'elle parle
        at(&mut look, &mut c, frame + 1, true, false);
        assert!(!look.walk(&mut c, frame + 1, to - 100.0), "elle parle");

        // arrêtée en chemin : sa boucle, là
        let mut look = moving();
        let mut c = capri(&look);
        entered(&mut look, &mut c, 100);
        let to = c.feet().x - 300.0;
        assert!(look.walk(&mut c, 100, to));
        for frame in 100..110 {
            c.step(1.0 / 24.0, frame);
            look.update(&mut c, frame, false, false);
        }
        let x = c.feet().x;
        look.halt(&mut c);
        let stopped = at(&mut look, &mut c, 111, false, false);
        assert!(
            matches!(stopped.drawing, Drawing::Stand(_)) && c.feet().x == x && c.stance() == Stance::Floating
        );
        // attrapée en chemin : elle pend ; cachée en chemin : elle s'efface là
        let to = c.feet().x - 300.0;
        assert!(look.walk(&mut c, 120, to));
        at(&mut look, &mut c, 125, false, false);
        hold(&mut look, &mut c, 126);
        assert!(look.is_dangling(&c) && !c.is_walking());
        c.release(1000.0);
        at(&mut look, &mut c, 200, false, false);
        at(&mut look, &mut c, 260, false, false);
        let to = c.feet().x - 300.0;
        assert!(look.walk(&mut c, 260, to));
        at(&mut look, &mut c, 265, false, false);
        let x = c.feet().x;
        look.vanish(&mut c, 265);
        let leaving = at(&mut look, &mut c, 266, false, false);
        assert_eq!(life_of(&leaving).map(|(s, _)| s), Some(life::Strip::Leave));
        assert!(!c.is_walking() && c.feet().x == x);
    }

    #[test]
    fn every_move_is_optional_one_by_one() {
        // les sept planches, chacune là ou non, avec ou sans sa pose debout
        for bits in 0..(1u32 << 7) {
            for stand_sheets in [true, false] {
                let on = |bit: u32| bits & (1 << bit) != 0;
                let body = || block(life::CELL_WIDTH, sheets::CELL_HEIGHT, (30, 15, 210, 370));
                let mut wardrobe = wardrobe(stand_sheets, true, true);
                wardrobe.moves = Moves {
                    walk: on(0).then(body),
                    dangle: on(1).then(body),
                    glitch: on(2).then(body),
                    grin: on(3).then(body),
                    perch_glitch: on(4),
                    perch_blink_glitch: on(5),
                    stand_blink_glitch: on(6),
                };
                let case = format!("planches {bits:07b}, debout {stand_sheets}");
                let mut look = Look::new(wardrobe, HEIGHT);
                let mut c = capri(&look);
                entered(&mut look, &mut c, 100);
                let lives = stand_sheets;
                // un clic debout : son sourire, ou rien
                assert_eq!(look.poke(&mut c, 100).is_some(), lives && on(3), "{case}");
                at(&mut look, &mut c, 300, false, false);
                // tenue : en l'air, ou sa boucle (ou son dessin d'avant)
                hold(&mut look, &mut c, 300);
                let held = look.picture(&c);
                assert_eq!(life_of(&held).is_some(), lives && on(1), "{case} : {held:?}");
                assert_eq!(look.grip(&c).is_some(), lives && on(1), "{case}");
                // lâchée : elle se relève, ou rien
                c.release(1000.0);
                at(&mut look, &mut c, 301, false, false);
                assert_eq!(look.gesture() == Some(Gesture::Rise), lives && on(2), "{case}");
                at(&mut look, &mut c, 400, false, false);
                // elle marche, ou pas
                let to = c.feet().x - 100.0;
                let walks = look.walk(&mut c, 400, to);
                assert_eq!(walks, lives && on(0), "{case}");
                look.halt(&mut c);
                // assise : un éclat, ou rien
                land(&mut look, &mut c, 500);
                at(&mut look, &mut c, 600, false, false);
                assert_eq!(look.poke(&mut c, 600).is_some(), on(4), "{case}");
                // rien ne plante, rien ne reste coincé
                for frame in 600..800 {
                    let picture = at(&mut look, &mut c, frame, false, false);
                    if let Drawing::Life(cell) = picture.drawing {
                        assert!(look.has_move(cell.strip) || look.has_life(), "{case} : {picture:?}");
                    }
                }
                assert!(look.gesture().is_none() && look.is_here(), "{case}");
            }
        }
    }

    /// Un pas de l'app pendant une intro qu'elle joue : son corps à la cadence de l'animation.
    fn staged(look: &mut Look, c: &mut Capri, frame: u64) -> Picture {
        c.step(1.0 / 24.0, frame);
        look.update(c, frame, false, false);
        look.picture(c)
    }

    #[test]
    fn falling_from_the_sky_she_dangles_down_to_her_place_then_rises_and_loops() {
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        let place = c.feet();
        assert!(look.perform(&mut c, Act::Fall, 0));
        assert!(look.is_performing() && !look.is_here() && !look.is_away() && !look.is_hidden());
        assert_eq!(look.performing(), Some(Act::Fall));
        let first = look.picture(&c);
        assert_eq!(life_of(&first).map(|(strip, _)| strip), Some(life::Strip::Dangle));
        assert!(first.rect.max().y <= 0.0, "au-dessus de l'écran : {:?}", first.rect);
        let mut frame = 0;
        let mut last = c.feet().y;
        while look.is_performing() {
            frame += 1;
            assert!(frame <= 31, "elle finit de tomber");
            let picture = staged(&mut look, &mut c, frame);
            if look.is_performing() {
                assert_eq!(life_of(&picture).map(|(strip, _)| strip), Some(life::Strip::Dangle), "{frame}");
                assert!(c.feet().y > last, "elle descend : image {frame}");
                assert!(look.gesture().is_none());
            }
            last = c.feet().y;
        }
        assert!((c.feet() - place).length() < 1e-3, "à sa place : {:?} contre {place:?}", c.feet());
        assert_eq!(look.take_performed(), Some(End::Played { shown: frame as u32 }));
        assert_eq!(look.take_performed(), None, "une fois");
        // en bas, elle se relève, puis sa boucle
        assert_eq!(look.gesture(), Some(Gesture::Rise));
        assert!(look.is_here());
        let end = frame + Gesture::Rise.frames();
        let picture = at(&mut look, &mut c, end, false, false);
        assert!(matches!(picture.drawing, Drawing::Stand(_)), "{picture:?}");
        let tour = c.tour();
        at(&mut look, &mut c, end + 10, false, false);
        assert!(c.tour() > tour, "son tour reprend");
    }

    #[test]
    fn grabbed_hidden_or_talked_to_while_she_falls_her_intro_stops() {
        // attrapée : tenue, comme d'habitude, et elle pend
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        let place = c.feet();
        assert!(look.perform(&mut c, Act::Fall, 0));
        let mut picture = look.picture(&c);
        for frame in 1..=26 {
            picture = staged(&mut look, &mut c, frame);
        }
        let middle = picture.rect.min + picture.rect.size * 0.5;
        assert!(look.is_over(&c, middle), "on peut l'attraper en l'air");
        assert!(look.hold(&mut c, &grab_at(middle, 1.0)));
        staged(&mut look, &mut c, 27);
        assert!(look.is_here() && c.is_held() && look.is_dangling(&c));
        assert_eq!(look.take_performed(), Some(End::Cut { why: intro::Cut::Grabbed, at: Some(27) }));
        // on lui parle : elle se matérialise tout de suite à sa place
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        assert!(look.perform(&mut c, Act::Fall, 0));
        staged(&mut look, &mut c, 5);
        look.cut_performance(&mut c, 5);
        assert!(!look.is_performing() && (c.feet() - place).length() < 1e-3);
        assert_eq!(c.stance(), Stance::Floating);
        assert_eq!(look.take_performed(), Some(End::Cut { why: intro::Cut::Talked, at: Some(5) }));
        let picture = at(&mut look, &mut c, 6, false, false);
        assert_eq!(life_of(&picture).map(|(strip, _)| strip), Some(life::Strip::Arrive));
        at(&mut look, &mut c, 6 + life::arriving().len(), false, false);
        assert!(look.is_here() && look.gesture().is_none(), "elle ne se relève pas : elle n'est pas tombée");
        // cachée : coupée net, cachée ; montrée, elle se matérialise à sa place
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        assert!(look.perform(&mut c, Act::Fall, 0));
        staged(&mut look, &mut c, 5);
        look.vanish(&mut c, 5);
        assert!(look.is_gone() && look.is_hidden() && !look.is_performing());
        assert_eq!(look.take_performed(), Some(End::Cut { why: intro::Cut::Hidden, at: Some(5) }));
        assert!((c.feet() - place).length() < 1e-3 && c.stance() == Stance::Floating);
        look.appear(&mut c, 50);
        assert_eq!(
            life_of(&at(&mut look, &mut c, 50, false, false)).map(|(s, _)| s),
            Some(life::Strip::Arrive)
        );
    }

    #[test]
    fn grabbed_while_she_enters_partly_off_the_screen_she_does_not_jump() {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        for (act, frame) in [(Act::Fall, 5u64), (Act::Walk, 24)] {
            let mut look = moving();
            let mut c = Capri::new(look.standing_size(), screen, Vec2::new(2700.0, 1300.0));
            c.set_feet(look.standing_feet());
            c.step(0.0, 0);
            assert!(look.perform(&mut c, act, 0));
            let mut picture = look.picture(&c);
            for f in 1..=frame {
                picture = staged(&mut look, &mut c, f);
            }
            // la part d'elle qui se voit
            let visible =
                Rect::new(Vec2::new(picture.rect.min.x.max(0.0), picture.rect.min.y.max(0.0)), Vec2::ZERO);
            let seen = Vec2::new(
                (visible.min.x + picture.rect.max().x.min(3024.0)) * 0.5,
                (visible.min.y + picture.rect.max().y.min(1964.0)) * 0.5,
            );
            let before = c.position();
            assert!(before.y < 0.0 || before.x + c.size().x > 3024.0, "{act:?} : en partie dehors");
            assert!(look.is_over(&c, seen), "{act:?} : on l'attrape par ce qui se voit");
            assert!(look.hold(&mut c, &grab_at(seen, 1.0)));
            assert_eq!(c.position(), before, "{act:?} : pas de saut à la prise");
            staged(&mut look, &mut c, frame + 1);
            assert_eq!(
                look.take_performed(),
                Some(End::Cut { why: intro::Cut::Grabbed, at: Some(frame as u32 + 1) })
            );
            // le curseur va vers l'intérieur de l'écran : elle le suit, sans saut
            let inward = if act == Act::Fall { Vec2::new(0.0, 20.0) } else { Vec2::new(-20.0, 0.0) };
            c.drag_to(seen + inward, 1.1);
            let moved = c.position() - before;
            // au plus le chemin du curseur, et la moitié de plus (son glissé vers sa mèche)
            assert!(moved.length() > 0.0 && moved.length() <= 30.0 + 1e-3, "{act:?} : {moved:?}");
        }
    }

    #[test]
    fn walking_in_from_closer_she_always_appears_whole_on_the_screen() {
        // sa place à 400 px du bord droit : 4 s de marche la feraient apparaître en partie dehors
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        let mut look = moving();
        let size = look.standing_size();
        let feet = look.standing_feet();
        let mut c = Capri::new(size, screen, Vec2::new(2624.0 - size.x * feet.x, 1300.0));
        c.set_feet(feet);
        c.step(0.0, 0);
        assert!(look.perform(&mut c, Act::Walk, 0));
        let appearing = look.picture(&c);
        assert_eq!(life_of(&appearing), Some((life::Strip::Arrive, 0)), "elle se matérialise d'abord");
        assert!(appearing.rect.max().x <= 3024.0 + 1.0, "entière dans l'écran : {:?}", appearing.rect);
        assert!(appearing.rect.min.x >= 0.0);
    }

    #[test]
    fn a_new_size_while_she_plays_her_intro_cuts_it_and_puts_her_at_her_place() {
        for (act, frame) in [(Act::Fall, 10u64), (Act::Walk, 40), (Act::Gather, 6)] {
            let mut look = moving();
            let mut c = capri(&look);
            c.step(0.0, 0);
            let place = c.feet();
            assert!(look.perform(&mut c, act, 0));
            for f in 1..=frame {
                staged(&mut look, &mut c, f);
            }
            assert!(look.is_performing(), "{act:?}");
            // sa taille change (son menu, la molette, ou la densité de l'écran), comme dans l'app
            look.set_height(360.0);
            c.set_size(look.standing_size());
            staged(&mut look, &mut c, frame + 1);
            assert_eq!(
                look.take_performed(),
                Some(End::Cut { why: intro::Cut::Resized, at: Some(frame as u32 + 1) }),
                "{act:?}"
            );
            assert!((c.feet() - place).length() < 1e-3, "{act:?} : à sa place, {:?}", c.feet());
            assert!(!look.is_performing() && look.gesture().is_none(), "{act:?} : ni intro ni geste");
            assert_eq!(c.stance(), Stance::Floating);
            // elle se matérialise à sa place, comme quand on lui parle
            let picture = look.picture(&c);
            assert_eq!(life_of(&picture).map(|(strip, _)| strip), Some(life::Strip::Arrive), "{act:?}");
        }
        // sans ses planches de vie (elle est là d'un coup) : coupée en tombant, par sa taille ou
        // parce qu'on lui parle, elle ne se relève pas pour autant (ce n'est pas un lâcher)
        for resized in [true, false] {
            let mut wardrobe = wardrobe(true, true, false);
            wardrobe.moves = every_move(true);
            let mut look = Look::new(wardrobe, HEIGHT);
            let mut c = capri(&look);
            c.step(0.0, 0);
            assert!(look.perform(&mut c, Act::Fall, 0));
            staged(&mut look, &mut c, 10);
            if resized {
                look.set_height(360.0);
                c.set_size(look.standing_size());
            } else {
                look.cut_performance(&mut c, 10);
            }
            staged(&mut look, &mut c, 11);
            staged(&mut look, &mut c, 12);
            assert!(look.is_here() && look.gesture().is_none(), "taille {resized} : {:?}", look.gesture());
        }
    }

    #[test]
    fn walking_in_she_enters_from_the_right_edge_or_appears_closer_then_walks_to_her_place() {
        // sa place près du bord droit : elle entre en marchant, de hors de l'écran
        let mut look = moving();
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        let mut c = Capri::new(look.standing_size(), screen, Vec2::new(2700.0, 1300.0));
        c.set_feet(look.standing_feet());
        c.step(0.0, 0);
        let place = c.feet();
        assert!(look.perform(&mut c, Act::Walk, 0));
        let first = look.picture(&c);
        assert_eq!(life_of(&first).map(|(strip, _)| strip), Some(life::Strip::Walk));
        assert!(first.rect.min.x >= 3024.0, "hors de l'écran : {:?}", first.rect);
        let mut frame = 0;
        let mut last = c.feet().x;
        while look.is_performing() {
            frame += 1;
            assert!(frame <= 4 * 24 + 1, "4 s au plus");
            let picture = staged(&mut look, &mut c, frame);
            assert!(c.feet().x < last || !look.is_performing(), "vers la gauche : image {frame}");
            last = c.feet().x;
            if look.is_performing() {
                assert_eq!(life_of(&picture).map(|(strip, _)| strip), Some(life::Strip::Walk));
            }
        }
        assert!((c.feet() - place).length() < 1e-3);
        assert!(matches!(look.take_performed(), Some(End::Played { .. })));
        assert!(look.is_here() && c.stance() == Stance::Floating && !look.is_walking(&c));
        let tour = c.tour();
        at(&mut look, &mut c, frame + 10, false, false);
        assert!(c.tour() > tour, "sa boucle reprend");

        // sa place loin du bord : elle se matérialise à 4 s de marche, puis marche
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        let place = c.feet();
        assert!(look.perform(&mut c, Act::Walk, 0));
        let speed = look.walk_speed().expect("sa marche");
        assert!((c.feet().x - place.x - act::WALK_LONGEST * speed).abs() < 1e-2, "{:?}", c.feet());
        let appearing = look.picture(&c);
        assert_eq!(life_of(&appearing), Some((life::Strip::Arrive, 0)));
        let mut frame = 0;
        while look.is_performing() {
            frame += 1;
            assert!(frame <= life::arriving().len() + 4 * 24 + 1);
            let picture = staged(&mut look, &mut c, frame);
            if look.is_performing() && frame > life::arriving().len() {
                assert_eq!(life_of(&picture).map(|(strip, _)| strip), Some(life::Strip::Walk), "{frame}");
            }
        }
        assert!((c.feet() - place).length() < 1e-3);
        assert!(look.is_here());
    }

    #[test]
    fn gathering_her_bands_close_in_on_her_then_her_arrival_ends() {
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        assert!(look.perform(&mut c, Act::Gather, 0));
        let mut keys = Vec::new();
        for frame in 0..act::GATHER_FRAMES {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Gather(key) = picture.drawing else { panic!("{picture:?}") };
            keys.push(key);
            assert!(!look.is_over(&c, picture.rect.min + picture.rect.size * 0.5), "rien ne l'attrape");
            assert_eq!(picture.rect, life::over_stand(c.pose().rect));
        }
        assert_eq!(keys, [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8]);
        // puis son arrivée, de sa case 9 à la fin, et sa boucle
        let picture = at(&mut look, &mut c, act::GATHER_FRAMES, false, false);
        assert_eq!(life_of(&picture), Some((life::Strip::Arrive, 9)));
        assert_eq!(look.take_performed(), Some(End::Played { shown: act::GATHER_FRAMES as u32 }));
        assert!(!look.is_performing() && !look.is_here());
        at(&mut look, &mut c, act::GATHER_FRAMES + 10, false, false);
        assert!(look.is_here());
    }

    #[test]
    fn see_an_intro_she_fades_out_then_plays_it_and_is_refused_when_busy() {
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.update(&mut c, 0, false, false);
        let place = c.feet();
        assert_eq!(look.bow_out(&mut c, 0), Ok(()));
        assert!(look.is_bowing_out() && !look.is_offstage() && !look.is_hidden() && !look.is_traveling());
        assert_eq!(look.bow_out(&mut c, 1), Err(Hold::Away), "déjà en train de s'effacer");
        at(&mut look, &mut c, life::leaving().len(), false, false);
        assert!(look.is_offstage() && !look.is_gone() && !look.is_between());
        assert!(look.perform(&mut c, Act::Gather, 30));
        assert!(look.is_performing() && (c.feet() - place).length() < 1e-3);
        // « se matérialise », ou le film coupé en lui parlant : elle se matérialise ensuite
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.bow_out(&mut c, 0).expect("libre");
        at(&mut look, &mut c, 40, false, false);
        look.enter(&mut c, 40);
        at(&mut look, &mut c, 40 + life::arriving().len(), false, false);
        assert!(look.is_here());
        // on lui parle avant que l'intro ne commence : elle revient sur ses pas
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.bow_out(&mut c, 0).expect("libre");
        at(&mut look, &mut c, 6, false, false);
        look.turn_back(&mut c, 6);
        assert!(!look.is_bowing_out());
        // cachée pendant qu'elle s'efface : cachée, sans intro
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.bow_out(&mut c, 0).expect("libre");
        look.vanish(&mut c, 3);
        assert!(look.is_hidden() && !look.is_bowing_out());
        at(&mut look, &mut c, 60, false, false);
        assert!(look.is_gone() && !look.is_offstage());
        assert!(!look.perform(&mut c, Act::Fall, 60), "cachée, pas d'intro");
        // refusée : tenue, on se parle, en marche
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.update(&mut c, 0, false, false);
        look.set_conversing(true);
        assert_eq!(look.bow_out(&mut c, 1), Err(Hold::Talking));
        look.set_conversing(false);
        let to = c.feet().x - 300.0;
        assert!(look.walk(&mut c, 1, to));
        assert_eq!(look.bow_out(&mut c, 2), Err(Hold::Walking));
        look.halt(&mut c);
        c.grab(c.position() + Vec2::new(10.0, 10.0), 0.0);
        assert_eq!(look.bow_out(&mut c, 3), Err(Hold::Carried));
        assert!(look.is_here(), "refusée, rien n'a changé");
        // sans ses planches de vie : effacée tout de suite
        let mut look = dressed(true, false);
        look.wardrobe.life = None;
        let mut c = capri(&look);
        assert_eq!(look.bow_out(&mut c, 0), Ok(()));
        assert!(look.is_offstage());
    }

    #[test]
    fn an_intro_she_plays_needs_its_sheets_and_otherwise_changes_nothing() {
        let mut bare = dressed(true, true);
        let mut c = capri(&bare);
        for act in Act::ALL {
            assert_eq!(bare.can_perform(act, &c), act == Act::Gather, "{act:?}");
        }
        assert!(!bare.perform(&mut c, Act::Fall, 0));
        assert!(!bare.perform(&mut c, Act::Walk, 0));
        assert!(bare.is_here() && !bare.is_performing());
        let mut lifeless = Look::new(Wardrobe { life: None, ..wardrobe(true, true, false) }, HEIGHT);
        assert!(!lifeless.perform(&mut c, Act::Gather, 0));
        // seulement à son entrée en scène, ou effacée pour une intro
        let mut look = moving();
        let mut c = capri(&look);
        look.enter(&mut c, 0);
        assert!(!look.perform(&mut c, Act::Fall, 1), "en train d'apparaître");
        look.hide_unseen(&mut c, 2);
        assert!(!look.perform(&mut c, Act::Fall, 3), "cachée");
        // assise : elle joue l'intro debout, ses pieds sur le point où elle était posée
        let mut look = moving();
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        assert!(c.perched().is_some());
        look.bow_out(&mut c, 100).expect("libre");
        at(&mut look, &mut c, 200, false, false);
        assert!(look.perform(&mut c, Act::Walk, 200));
        assert!(c.perched().is_none() && !look.is_seated(&c));
    }

    /// Elle, effacée pour « Voir une intro » (`encore`), ou telle qu'au lancement, pas encore
    /// entrée en scène : prête pour la suite d'une intro, à l'image rendue.
    fn before_the_suite(look: &mut Look, c: &mut Capri, encore: bool) -> u64 {
        c.step(0.0, 0);
        if !encore {
            return 0;
        }
        look.bow_out(c, 0).expect("libre");
        let frame = life::leaving().len() + 2;
        at(look, c, frame, false, false);
        assert!(look.is_offstage());
        frame
    }

    /// Elle apparaît (son arrivée, d'abord), puis elle est là, en moins de deux secondes : jamais
    /// laissée là où elle était (pas encore entrée en scène, ou effacée pour toujours).
    fn appears(look: &mut Look, c: &mut Capri, from: u64, what: &str) {
        assert!(!look.is_here() && !look.is_away() && !look.is_offstage(), "{what} : elle n'apparaît pas");
        assert!(!look.is_performing(), "{what}");
        let here = (from + 1..from + 48).find(|&frame| {
            at(look, c, frame, false, false);
            look.is_here()
        });
        assert!(here.is_some(), "{what} : jamais là");
    }

    #[test]
    fn after_the_film_the_suite_makes_her_appear_play_her_entry_or_stay_hidden() {
        use crate::intro::{After, Cut, Missing, Suite, by_key, plain};
        let played = End::Played { shown: 22 };
        let falls = by_key("tombe").expect("tombe du ciel");
        let walks = by_key("marche").expect("arrive en marchant");
        // arriver en marchant n'est jamais impossible faute de place d'où elle est, dans l'écran :
        // il faudrait ses pieds à la fois à plus de 4 s de marche du bord droit, sa toile de vie
        // comprise, et à moins d'une seconde de là où sa toile y tient entière ; ses proportions
        // (à gauche et à droite de ses pieds, sa vitesse) l'excluent, à toute taille
        let look = moving();
        for width in [3024.0, 1400.0, 700.0, 300.0] {
            let screen = Rect::new(Vec2::ZERO, Vec2::new(width, 1964.0));
            for x in (0..width as u32).step_by(3) {
                let mut c = Capri::new(look.standing_size(), screen, Vec2::new(0.0, 800.0));
                c.set_feet(look.standing_feet());
                c.set_at(Vec2::new(x as f32, 1500.0));
                assert_ne!(look.cannot_perform(Act::Walk, &c), Some(Missing::NoRoom), "{width} {x}");
            }
        }
        for encore in [false, true] {
            let when = if encore { "« Voir une intro »" } else { "au lancement" };
            // le film joué, puis « se matérialise » : elle apparaît, la suite est finie
            let (mut look, mut c) = (moving(), capri(&moving()));
            let frame = before_the_suite(&mut look, &mut c, encore);
            let mut suite = Suite::new(plain(), true, !encore);
            assert_eq!(suite.film_over(played, &mut look, &mut c, frame), Some(After::Entry));
            assert!(suite.is_over() && suite.greets() == !encore, "{when}");
            assert_eq!(suite.entry_end(), Some(End::Played { shown: 0 }));
            appears(&mut look, &mut c, frame, &format!("{when}, après le film, « se matérialise »"));
            // le film coupé parce qu'on lui parle : elle apparaît à sa place, sans entrée
            let (mut look, mut c) = (moving(), capri(&moving()));
            let frame = before_the_suite(&mut look, &mut c, encore);
            let mut suite = Suite::new(falls, true, !encore);
            let talked = End::Cut { why: Cut::Talked, at: Some(5) };
            assert_eq!(suite.film_over(talked, &mut look, &mut c, frame), Some(After::Here));
            assert!(suite.is_over() && !suite.greets() && suite.entry_end().is_none(), "{when}");
            appears(&mut look, &mut c, frame, &format!("{when}, le film coupé en lui parlant"));
            // le film coupé en la cachant : cachée, sans s'être montrée
            let (mut look, mut c) = (moving(), capri(&moving()));
            let frame = before_the_suite(&mut look, &mut c, encore);
            let mut suite = Suite::new(falls, true, !encore);
            let hidden = End::Cut { why: Cut::Hidden, at: Some(5) };
            assert_eq!(suite.film_over(hidden, &mut look, &mut c, frame), Some(After::Hidden));
            assert!(look.is_gone() && look.is_hidden() && suite.hides(), "{when}");
            // le film joué, puis une entrée qu'elle joue : elle la joue ; finie, la suite aussi
            let (mut look, mut c) = (moving(), capri(&moving()));
            let frame = before_the_suite(&mut look, &mut c, encore);
            let mut suite = Suite::new(falls, true, !encore);
            suite.film_over(played, &mut look, &mut c, frame);
            assert!(look.is_performing() && !suite.is_over(), "{when}");
            let end = (frame + 1..frame + 200).find_map(|frame| {
                c.step(1.0 / 24.0, frame);
                look.update(&mut c, frame, false, false);
                look.take_performed().map(|end| (end, frame))
            });
            let (end, ended) = end.expect("finie");
            suite.entry_over(end, &mut look, &mut c, ended);
            assert!(suite.is_over() && suite.greets() == !encore, "{when} : {end:?}");
            // sans le film (ses images absentes) : l'entrée tout de suite
            let (mut look, mut c) = (moving(), capri(&moving()));
            let frame = before_the_suite(&mut look, &mut c, encore);
            let mut alone = Suite::new(plain(), false, !encore);
            alone.begin_entry(&mut look, &mut c, frame);
            assert!(alone.is_over());
            appears(&mut look, &mut c, frame, &format!("{when}, sans film"));
        }
        // une entrée qu'elle n'est pas prête à jouer (déjà en train d'apparaître) : elle se
        // matérialise, et le journal dira pourquoi (pas « pas de planches ») ; elle dit bonjour
        // (le film s'est joué)
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        look.enter(&mut c, 0);
        let mut suite = Suite::new(falls, true, true);
        suite.film_over(played, &mut look, &mut c, 1);
        assert_eq!(suite.entry_end(), Some(End::Missing(Missing::NotReady)));
        assert!(suite.greets() && !look.is_performing());
        // sans sa planche de marche : « pas de planches »
        let mut bare = dressed(true, true);
        let mut c = capri(&bare);
        c.step(0.0, 0);
        assert_eq!(bare.cannot_perform(Act::Walk, &c), Some(Missing::Absent { found: 0 }));
        let mut suite = Suite::new(walks, true, true);
        suite.film_over(played, &mut bare, &mut c, 0);
        assert_eq!(suite.entry_end(), Some(End::Missing(Missing::Absent { found: 0 })));
        appears(&mut bare, &mut c, 0, "sans sa planche de marche");
        // prête, rien ne manque ; en train d'apparaître, pas prête
        let mut look = moving();
        let mut c = capri(&look);
        c.step(0.0, 0);
        assert_eq!(look.cannot_perform(Act::Fall, &c), None);
        look.enter(&mut c, 0);
        assert_eq!(look.cannot_perform(Act::Fall, &c), Some(Missing::NotReady));
    }

    #[test]
    fn no_gesture_lasts_longer_than_the_longest() {
        let longest = Gesture::ALL.iter().map(|gesture| gesture.frames()).max();
        assert_eq!(longest, Some(LONGEST_GESTURE));
        assert_eq!(Gesture::Rise.frames(), life::rising().len());
        assert_eq!(Gesture::PerchGlitch.frames(), sheets::glitching().len());
        assert_eq!(Gesture::LieGlitch.frames(), lie::glitching().len());
        assert_eq!(Gesture::Grin.frames(), life::grinning().len() * life::GRIN_TOURS);
    }

    // --- son repos ---

    fn lie_masks() -> LieMasks {
        LieMasks { narrow: full(lie::NARROW, lie::CELL_HEIGHT), wide: full(lie::WIDE, lie::CELL_HEIGHT) }
    }

    fn nap_masks() -> NapMasks {
        NapMasks {
            sleep: full(life::CELL_WIDTH, sheets::CELL_HEIGHT),
            wake: full(life::CELL_WIDTH, sheets::CELL_HEIGHT),
        }
    }

    /// Assise sur `EDGE`, sa boucle en cours (à l'image rendue), ses planches de repos offertes
    /// (pas encore chargées), l'éclat de sa pose assise là, et la place pour s'allonger.
    fn seated_to_rest() -> (Look, Capri, u64) {
        let mut look = dressed(true, true);
        look.wardrobe.moves.perch_glitch = true;
        look.offer(Wants { lying: true, sleeping: true });
        look.set_room_to_lie(true);
        let mut c = capri(&look);
        land(&mut look, &mut c, 0);
        let frame = sheets::arriving().len() + 10;
        at(&mut look, &mut c, frame, false, false);
        assert!(look.is_sitting(&c));
        (look, c, frame)
    }

    /// Le point de son dessin qui est sur la ligne du bord, à l'écran.
    fn ledge_point(picture: &Picture) -> Vec2 {
        let uv = match picture.drawing {
            Drawing::Perch(cell) => cell.strip.seat_uv(),
            Drawing::Lie(cell) => cell.strip.seat_uv(),
            other => panic!("pas sur un bord : {other:?}"),
        };
        picture.rect.min + Vec2::new(picture.rect.size.x * uv.x, picture.rect.size.y * uv.y)
    }

    #[test]
    fn seated_she_lies_down_once_her_sheets_are_there_her_hip_where_it_was() {
        let (mut look, mut c, frame) = seated_to_rest();
        let sitting = look.picture(&c);
        // pas encore chargées : elle en a envie, et reste assise
        assert!(!look.lie_down(&mut c, frame));
        assert_eq!(look.wants(), Wants { lying: true, sleeping: false });
        assert!(matches!(at(&mut look, &mut c, frame + 1, false, false).drawing, Drawing::Perch(_)));
        // une envie qui n'est pas redite passe
        at(&mut look, &mut c, frame + WISH + 1, false, false);
        assert_eq!(look.wants(), Wants::default());
        // ce qui l'en empêche : pas la place, la musique, on se parle
        look.dress_lying(Some(lie_masks()));
        let start = frame + WISH + 2;
        at(&mut look, &mut c, start, false, false);
        look.set_room_to_lie(false);
        assert!(!look.lie_down(&mut c, start));
        look.set_room_to_lie(true);
        look.set_conversing(true);
        assert!(!look.lie_down(&mut c, start));
        look.set_conversing(false);
        at(&mut look, &mut c, start, false, true);
        assert!(!look.lie_down(&mut c, start), "la musique");
        at(&mut look, &mut c, start, false, false);
        // elle s'allonge : son éclat, en deux, puis sa boucle allongée, du dessin 0
        assert!(look.lie_down(&mut c, start));
        assert_eq!((look.rest(), look.gesture()), (Rest::Lying, Some(Gesture::LieGlitch)));
        for frame in start..start + Gesture::LieGlitch.frames() {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Lie(cell) = picture.drawing else { panic!("{picture:?}") };
            assert_eq!(cell, lie::Cell { strip: lie::Strip::LieGlitch, index: ((frame - start) / 2) as u32 });
            assert!((ledge_point(&picture) - EDGE).length() <= 1.0, "sur son bord : {picture:?}");
        }
        let lying = at(&mut look, &mut c, start + Gesture::LieGlitch.frames(), false, false);
        assert_eq!(lying.drawing, Drawing::Lie(lie::Cell { strip: lie::Strip::Lie, index: 0 }));
        assert_eq!(look.gesture(), None);
        // la ligne du bord de ses planches sur son bord, sa hanche là où elle était assise
        assert!((ledge_point(&lying) - EDGE).length() <= 1.0);
        let k = (stand::perch_scale(HEIGHT), lie::scale(HEIGHT));
        let seated_hip = sitting.rect.min.x + lie::PERCH_HIP * k.0;
        let lying_hip = lying.rect.min.x + lie::HIP * k.1;
        assert!((seated_hip - lying_hip).abs() <= 1.5, "{seated_hip} contre {lying_hip}");
        assert!(look.is_seated(&c) && look.wants().lying && look.is_resting());
        // ce qu'il lui faut sur un bord pour s'y asseoir ne change pas
        assert_eq!(look.stature(), seated_stature(HEIGHT));
        // elle se balance pas allongée, même sur la musique
        for frame in start + 40..start + 200 {
            at(&mut look, &mut c, frame, false, true);
            assert!(!look.is_grooving() && look.rest() == Rest::Lying);
        }
    }

    /// Allongée sur `EDGE`, sa boucle en cours, à l'image rendue.
    fn lying() -> (Look, Capri, u64) {
        let (mut look, mut c, frame) = seated_to_rest();
        look.dress_lying(Some(lie_masks()));
        assert!(look.lie_down(&mut c, frame));
        let frame = frame + Gesture::LieGlitch.frames();
        at(&mut look, &mut c, frame, false, false);
        assert_eq!(look.rest(), Rest::Lying);
        (look, c, frame)
    }

    #[test]
    fn lying_she_blinks_talks_falls_asleep_and_wakes_when_talked_to_or_clicked() {
        let (mut look, mut c, since) = lying();
        // ses clignements, aux tours de BLINKS
        for tour in 0..8 {
            let frame = since + tour * TOUR + u64::from(lie::BLINK_FROM) + 2;
            let Drawing::Lie(cell) = at(&mut look, &mut c, frame, false, false).drawing else { panic!() };
            assert_eq!(cell.strip == lie::Strip::LieBlink, blinks(tour), "tour {tour}");
        }
        // elle parle allongée : sa bouche allongée, dessin pour dessin
        let frame = since + 8 * TOUR + 10;
        let Drawing::Lie(cell) = at(&mut look, &mut c, frame, true, false).drawing else { panic!() };
        assert_eq!(cell, lie::Cell { strip: lie::Strip::LieTalk, index: 10 });
        // elle ne s'endort pas pendant qu'on se parle
        look.set_conversing(true);
        assert!(!look.doze(&mut c, frame));
        look.set_conversing(false);
        let frame = frame + 1;
        at(&mut look, &mut c, frame, false, false);
        assert!(look.doze(&mut c, frame));
        assert_eq!(look.rest(), Rest::Asleep { lying: true });
        // endormie : son sommeil, un dessin sur trois, tenu trois images ; sur son bord
        let mut drawings = Vec::new();
        for f in frame..frame + 60 {
            let picture = at(&mut look, &mut c, f, false, false);
            let Drawing::Lie(cell) = picture.drawing else { panic!() };
            assert_eq!(cell.strip, lie::Strip::LieSleep);
            assert!((ledge_point(&picture) - EDGE).length() <= 1.0);
            drawings.push(cell.index);
        }
        assert!(drawings.chunks(3).all(|held| held.iter().all(|&index| index == held[0])), "{drawings:?}");
        assert_eq!(drawings.iter().collect::<std::collections::HashSet<_>>().len(), 20);
        // on lui parle : elle se réveille, allongée, et parle
        let frame = frame + 60;
        let Drawing::Lie(cell) = at(&mut look, &mut c, frame, true, false).drawing else { panic!() };
        assert_eq!((look.rest(), cell.strip), (Rest::Lying, lie::Strip::LieTalk));
        // endormie de nouveau, un clic la réveille, sans geste ; le suivant, un éclat allongée
        let frame = frame + 1;
        at(&mut look, &mut c, frame, false, false);
        assert!(look.doze(&mut c, frame));
        look.take_attention();
        assert_eq!(look.poke(&mut c, frame), None);
        assert_eq!(look.rest(), Rest::Lying);
        assert!(look.take_attention(), "un clic compte");
        assert!(!look.take_attention(), "une seule fois");
        assert_eq!(look.poke(&mut c, frame), Some(Gesture::LieGlitch));
        let Drawing::Lie(cell) = at(&mut look, &mut c, frame + 1, false, false).drawing else { panic!() };
        assert_eq!(cell.strip, lie::Strip::LieGlitch);
        // Claude Code attend une réponse : endormie, elle se réveille
        let frame = frame + Gesture::LieGlitch.frames() + 1;
        at(&mut look, &mut c, frame, false, false);
        assert!(look.doze(&mut c, frame));
        assert!(look.stir(&mut c, frame));
        assert_eq!(look.rest(), Rest::Lying);
        assert!(!look.stir(&mut c, frame), "réveillée, rien");
    }

    #[test]
    fn she_sits_up_with_her_seated_glitch_or_at_once_and_when_room_or_sheets_go() {
        let (mut look, mut c, frame) = lying();
        assert!(look.sit_up(&mut c, frame));
        assert_eq!((look.rest(), look.gesture()), (Rest::Awake, Some(Gesture::PerchGlitch)));
        let picture = at(&mut look, &mut c, frame, false, false);
        assert_eq!(picture.drawing, Drawing::Perch(sheets::Cell { strip: Strip::PerchGlitch, index: 0 }));
        let end = frame + Gesture::PerchGlitch.frames();
        let picture = at(&mut look, &mut c, end, false, false);
        assert_eq!(picture.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        assert!(!look.sit_up(&mut c, end), "assise, rien");
        // sans l'éclat de sa pose assise : une coupe franche
        let (mut look, mut c, frame) = lying();
        look.wardrobe.moves.perch_glitch = false;
        assert!(look.sit_up(&mut c, frame));
        assert_eq!(look.gesture(), None);
        assert_eq!(
            at(&mut look, &mut c, frame, false, false).drawing,
            Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 })
        );
        // endormie, elle se rassoit aussi
        let (mut look, mut c, frame) = lying();
        assert!(look.doze(&mut c, frame));
        assert!(look.sit_up(&mut c, frame + 1));
        assert_eq!(look.rest(), Rest::Awake);
        // plus la place pour elle allongée : elle se rassoit d'elle-même, au pas suivant
        let (mut look, mut c, frame) = lying();
        look.set_room_to_lie(false);
        at(&mut look, &mut c, frame + 1, false, false);
        assert_eq!((look.rest(), look.gesture()), (Rest::Awake, Some(Gesture::PerchGlitch)));
        assert_eq!(look.take_unsettled(), Some((Unsettled::NoRoom, false)), "elle dit pourquoi");
        assert_eq!(look.take_unsettled(), None, "une seule fois");
        // ses planches reprises (ce que l'app ne fait pas tant qu'elle les veut) : de même
        let (mut look, mut c, frame) = lying();
        look.dress_lying(None);
        let picture = at(&mut look, &mut c, frame + 1, false, false);
        assert!(matches!(picture.drawing, Drawing::Perch(_)), "{picture:?}");
        assert_eq!(look.rest(), Rest::Awake);
        assert_eq!(look.take_unsettled(), Some((Unsettled::Undressed, false)));
    }

    #[test]
    fn her_edge_gone_under_her_she_fades_out_lying_then_floats() {
        let (mut look, mut c, frame) = lying();
        c.take_off();
        look.update(&mut c, frame, false, false);
        assert_eq!(look.rest(), Rest::Awake, "elle n'est plus allongée");
        assert_eq!(look.take_unsettled(), Some((Unsettled::LedgeGone, false)), "elle dit pourquoi");
        let leave = lie::leaving().len();
        for f in frame..frame + leave {
            let picture = at(&mut look, &mut c, f, false, false);
            let Drawing::Lie(cell) = picture.drawing else { panic!("image {f} : {picture:?}") };
            assert_eq!(cell, lie::Cell { strip: lie::Strip::LieLeave, index: ((f - frame) / 2) as u32 });
            assert!((ledge_point(&picture) - EDGE).length() <= 1.0, "là où elle était");
            assert!(look.wants().lying, "ses planches gardées le temps de s'effacer");
        }
        let after = at(&mut look, &mut c, frame + leave, false, false);
        assert!(matches!(after.drawing, Drawing::Stand(_)), "{after:?}");
        assert_eq!(look.wants(), Wants::default());
        assert!((c.feet() - EDGE).length() < 0.5);
    }

    #[test]
    fn grabbed_lying_or_asleep_she_stands_at_once_under_the_cursor() {
        for asleep in [false, true] {
            let (mut look, mut c, frame) = lying();
            if asleep {
                assert!(look.doze(&mut c, frame));
            }
            let shown = look.shown(&c);
            // son rectangle allongé : bien plus large que haut
            assert!(shown.size.x > 1.4 * shown.size.y, "{shown:?}");
            let cursor = shown.min + shown.size * 0.5;
            let grab = Grab { at: cursor, since: 0.0, trail: Vec::new() };
            assert!(look.hold(&mut c, &grab));
            let held = at(&mut look, &mut c, frame + 1, false, false);
            assert!(matches!(held.drawing, Drawing::Stand(_)), "{held:?}");
            assert_eq!(look.rest(), Rest::Awake);
            assert!(c.is_held());
            assert_eq!(look.take_unsettled(), None, "on s'occupe d'elle : pas un repos arrêté de lui-même");
        }
    }

    #[test]
    fn hidden_lying_she_fades_out_lying_and_shown_halfway_she_lies_again() {
        let (mut look, mut c, frame) = lying();
        look.vanish(&mut c, frame);
        assert_eq!(look.rest(), Rest::Awake);
        let leave = lie::leaving().len();
        for f in frame..frame + 10 {
            let Drawing::Lie(cell) = at(&mut look, &mut c, f, false, false).drawing else { panic!() };
            assert_eq!(cell.strip, lie::Strip::LieLeave);
            assert!(look.wants().lying);
        }
        // montrée au milieu : elle revient sur ses pas, allongée, et le reste
        look.appear(&mut c, frame + 10);
        let mut f = frame + 10;
        loop {
            let picture = at(&mut look, &mut c, f, false, false);
            let Drawing::Lie(cell) = picture.drawing else { panic!("image {f} : {picture:?}") };
            if look.is_here() {
                assert_eq!(cell, lie::Cell { strip: lie::Strip::Lie, index: 0 }, "puis sa boucle allongée");
                break;
            }
            assert_eq!(cell.strip, lie::Strip::LieLeave);
            assert!(look.wants().lying);
            f += 1;
            assert!(f < frame + 40);
        }
        assert_eq!(look.rest(), Rest::Lying);
        // cachée jusqu'au bout : partie ; montrée bien plus tard, elle apparaît assise
        look.vanish(&mut c, f + 2);
        at(&mut look, &mut c, f + 2 + leave, false, false);
        assert!(look.is_gone());
        assert_eq!(look.wants(), Wants::default());
        c.step(0.0, 5000);
        look.appear(&mut c, 5000);
        let shown = at(&mut look, &mut c, 5000, false, false);
        assert!(
            matches!(shown.drawing, Drawing::Perch(cell) if cell.strip == Strip::PerchArrive),
            "{shown:?}"
        );
        assert_eq!(look.rest(), Rest::Awake);
    }

    #[test]
    fn hidden_again_while_she_appears_lying_she_fades_out_lying_and_rests_no_more() {
        let (mut look, mut c, frame) = lying();
        assert_eq!(look.take_unsettled(), None);
        look.vanish(&mut c, frame);
        for f in frame..frame + 10 {
            at(&mut look, &mut c, f, false, false);
        }
        // montrée au milieu : elle revient sur ses pas, allongée…
        look.appear(&mut c, frame + 10);
        for f in frame + 10..frame + 14 {
            at(&mut look, &mut c, f, false, false);
        }
        assert!(!look.is_here() && !look.is_away(), "elle réapparaît encore");
        assert_eq!(look.rest(), Rest::Lying);
        // … et cachée de nouveau avant d'être revenue : elle s'efface allongée, et ne se repose plus
        look.vanish(&mut c, frame + 14);
        let leave = lie::leaving().len();
        let mut f = frame + 14;
        while !look.is_gone() {
            let picture = at(&mut look, &mut c, f, false, false);
            let Drawing::Lie(cell) = picture.drawing else { panic!("image {f} : {picture:?}") };
            assert_eq!(cell.strip, lie::Strip::LieLeave);
            assert_eq!(look.rest(), Rest::Awake, "image {f} : cachée, elle ne se repose plus");
            f += 1;
            assert!(f <= frame + 14 + leave, "image {f} : toujours pas partie");
        }
        assert_eq!(look.wants(), Wants::default(), "partie : ses planches peuvent être reprises");
        assert_eq!(look.take_unsettled(), None, "cachée : ce n'est pas son bord parti");
    }

    #[test]
    fn standing_she_dozes_without_floating_and_wakes_with_her_wake_sheet() {
        let mut look = dressed(true, true);
        look.wardrobe.moves = Moves {
            walk: Some(full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
            grin: Some(full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
            ..Moves::default()
        };
        look.offer(Wants { lying: true, sleeping: true });
        let mut c = capri(&look);
        at(&mut look, &mut c, 10, false, false);
        // pas encore chargées : elle en a envie, et reste éveillée
        assert!(!look.doze(&mut c, 10));
        assert_eq!(look.wants(), Wants { lying: false, sleeping: true });
        look.dress_sleeping(Some(nap_masks()));
        assert!(look.doze(&mut c, 10));
        assert_eq!(look.rest(), Rest::Asleep { lying: false });
        let tour = c.tour();
        let rect = at(&mut look, &mut c, 11, false, false).rect;
        let mut drawings = Vec::new();
        for f in 14..14 + 128 {
            let picture = at(&mut look, &mut c, f, false, false);
            let Drawing::Life(cell) = picture.drawing else { panic!("{picture:?}") };
            assert_eq!(cell.strip, life::Strip::Sleep);
            assert_eq!(picture.rect, rect, "elle ne flotte pas en dormant");
            drawings.push(cell.index);
        }
        assert_eq!(c.tour(), tour, "son tour arrêté");
        assert!(drawings.chunks(4).all(|held| held.iter().all(|&index| index == held[0])));
        // endormie : ni sourire, ni marche
        assert!(!look.cheer(&mut c, 140));
        let to = c.feet().x - 200.0;
        assert!(!look.walk(&mut c, 140, to));
        // on lui parle : elle se réveille, sa planche de réveil, puis sa boucle, son tour repris
        let wake = life::waking().len();
        at(&mut look, &mut c, 140, true, false);
        assert_eq!(look.rest(), Rest::Waking);
        for f in 140..140 + wake {
            let Drawing::Life(cell) = at(&mut look, &mut c, f, false, false).drawing else { panic!() };
            assert_eq!(cell, life::Cell { strip: life::Strip::Wake, index: ((f - 140) / 2) as u32 });
        }
        let after = at(&mut look, &mut c, 140 + wake, false, false);
        assert!(matches!(after.drawing, Drawing::Stand(_)), "{after:?}");
        assert_eq!(look.rest(), Rest::Awake);
        at(&mut look, &mut c, 140 + wake + 30, false, false);
        assert!(c.tour() > tour, "son tour reprend");
        // endormie, un clic la réveille ; attrapée, elle est réveillée tout de suite
        assert!(look.doze(&mut c, 300));
        assert_eq!(look.poke(&mut c, 301), None);
        assert_eq!(look.rest(), Rest::Waking);
        at(&mut look, &mut c, 400, false, false);
        assert!(look.doze(&mut c, 400));
        c.grab(c.position() + Vec2::new(20.0, 20.0), 0.0);
        let held = at(&mut look, &mut c, 401, false, false);
        assert_eq!(look.rest(), Rest::Awake);
        assert!(matches!(held.drawing, Drawing::Stand(_)), "{held:?}");
    }

    #[test]
    fn without_her_rest_sheets_she_never_rests() {
        let (mut look, mut c, frame) = seated_to_rest();
        look.offer(Wants::default());
        look.dress_lying(Some(lie_masks()));
        assert!(!look.lie_down(&mut c, frame));
        assert_eq!(look.wants(), Wants::default());
        // sa pose assise absente : elle ne peut pas s'allonger, même offerte
        let mut look = dressed(true, false);
        look.offer(Wants { lying: true, sleeping: true });
        assert_eq!(look.offered(), Wants { lying: false, sleeping: true });
        assert!(look.lying_stature().is_none());
        // sa pose debout absente : pas de sommeil debout
        let mut look = dressed(false, true);
        look.offer(Wants { lying: true, sleeping: true });
        assert_eq!(look.offered(), Wants { lying: true, sleeping: false });
        let mut c = capri(&look);
        at(&mut look, &mut c, 1, false, false);
        assert!(!look.doze(&mut c, 1));
        assert_eq!(look.wants(), Wants::default());
    }

    /// Combien de fois son dessin change en `frames` images, à partir de l'image `from` (ce que la
    /// boucle redessine, son bandeau à part).
    fn changes(look: &mut Look, c: &mut Capri, from: u64, frames: u64, talking: bool) -> usize {
        let mut last = None;
        let mut changes = 0;
        for frame in from..from + frames {
            let picture = at(look, c, frame, talking, false);
            if last.is_some_and(|last| last != picture) {
                changes += 1;
            }
            last = Some(picture);
        }
        changes
    }

    #[test]
    fn asleep_she_is_redrawn_less_often_than_awake() {
        // allongée : éveillée, 12 dessins par seconde ; endormie, 8
        let (mut look, mut c, frame) = lying();
        let awake = changes(&mut look, &mut c, frame, 240, false);
        assert!(look.doze(&mut c, frame + 240));
        let asleep = changes(&mut look, &mut c, frame + 240, 240, false);
        assert_eq!((awake, asleep), (119, 79));
        // debout : éveillée, sa boucle et son flottement ; endormie, 6 dessins par seconde, sans flotter
        let mut look = dressed(true, true);
        look.offer(Wants { lying: false, sleeping: true });
        look.dress_sleeping(Some(nap_masks()));
        let mut c = capri(&look);
        let awake = changes(&mut look, &mut c, 0, 240, false);
        assert!(look.doze(&mut c, 240));
        let asleep = changes(&mut look, &mut c, 240, 240, false);
        assert_eq!((awake, asleep), (119, 59));
    }

    #[test]
    fn lying_she_never_falls_asleep_to_music() {
        let (mut look, mut c, frame) = lying();
        at(&mut look, &mut c, frame + 1, false, true);
        assert!(look.hears_music());
        assert!(!look.doze(&mut c, frame + 1), "la musique la tient éveillée");
        assert_eq!(look.rest(), Rest::Lying);
        assert!(!look.is_grooving(), "allongée, elle ne se balance pas");
        at(&mut look, &mut c, frame + 2, false, false);
        assert!(look.doze(&mut c, frame + 2));
        assert_eq!(look.rest(), Rest::Asleep { lying: true });
    }

    #[test]
    fn shown_again_lying_without_room_she_appears_lying_then_sits_up_from_the_start() {
        let (mut look, mut c, frame) = lying();
        look.vanish(&mut c, frame);
        for f in frame..frame + 10 {
            at(&mut look, &mut c, f, false, false);
        }
        // montrée au milieu de son départ allongée, et la place partie entre-temps
        look.appear(&mut c, frame + 10);
        look.set_room_to_lie(false);
        let mut f = frame + 10;
        loop {
            let picture = at(&mut look, &mut c, f, false, false);
            if look.is_here() {
                // elle ne se rassoit qu'une fois là : l'éclat de sa pose assise, de son début
                let glitch = sheets::Cell { strip: Strip::PerchGlitch, index: 0 };
                assert_eq!(picture.drawing, Drawing::Perch(glitch), "image {f}");
                break;
            }
            let Drawing::Lie(cell) = picture.drawing else { panic!("image {f} : {picture:?}") };
            assert_eq!(cell.strip, lie::Strip::LieLeave, "elle réapparaît allongée");
            f += 1;
            assert!(f < frame + 40);
        }
        // puis sa boucle assise, de son dessin 0 : pas de saut au milieu d'un tour
        let picture = at(&mut look, &mut c, f + Gesture::PerchGlitch.frames(), false, false);
        assert_eq!(picture.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        assert_eq!(look.rest(), Rest::Awake);
    }

    #[test]
    fn lying_she_needs_room_to_her_left_on_her_edge() {
        let stature = lying_stature(HEIGHT);
        let seated = seated_stature(HEIGHT);
        let k = lie::scale(HEIGHT);
        // surtout à gauche du point posé : sa tête et ses cheveux
        assert!(stature.reach.left > 3.0 * seated.reach.left, "{stature:?}");
        assert!(stature.reach.right < seated.reach.right);
        assert!((stature.reach.up - 140.0 * k).abs() < 1e-3 && stature.reach.up < seated.reach.up);
        // ce qui repose sur le bord : de son épaule au point posé
        assert!((stature.rest.0 - (lie::SEAT_X - lie::SHOULDER as f32) * k).abs() < 1e-3);
        assert!((stature.rest.1 - FOOT * HEIGHT).abs() < 1e-3);
        assert_eq!(stature.height, HEIGHT);
    }
}
