//! Elle vit sa vie : elle change d'elle-même de place de temps en temps (« Vivre sa vie » dans
//! son menu, cochée par défaut), et sur un geste (« Changer de place »). Ce qu'elle fait et quand
//! est décidé par `capri_core::roam` ; ici, ce que l'app lui donne (la liste des fenêtres par
//! `perch_view`, qui la lit et la trie, l'overlay, le curseur), et ce qu'elle en dit.
//!
//! **La liste des fenêtres** n'est lue, en plus de ce que fait déjà `perch_view`, qu'une fois par
//! décision : quand elle choisit où aller, et jamais si elle ne s'assoit pas sur les fenêtres.
//! Quand elle va s'asseoir, sa fenêtre est relue juste avant qu'elle apparaisse sur son bord
//! (`PerchView::arrived`) : c'est la première lecture de sa veille posée, avancée d'au plus un
//! quart de seconde, pas une lecture de plus. Jamais quand elle est cachée : la boucle ne fait alors
//! aucun pas. Aucun réveil de la boucle en plus, aucun thread : à chaque pas, elle compare une
//! heure.
//!
//! **Pendant une répétition** de la vraie app (`update_setup::REHEARSALS`), elle ne part jamais
//! d'elle-même (`roam_setup::rehearsing`) : « Changer de place » reste permis. `CAPRI_CONFIG_DIR`
//! seule ne l'en empêche pas.
//!
//! **Ailleurs que sur macOS et Windows**, il n'y a ni liste des fenêtres ni curseur : elle ne
//! change de place que debout, en bas de l'écran, sans éviter le curseur, et seulement de loin en
//! loin (comme partout où elle n'a aucun bord : voir `capri_core::roam`), sans menu pour la
//! décocher (`roam.json`).
//!
//! **Les gestes de l'utilisateur** qui la font descendre de son bord (sa taille, « S'asseoir sur
//! les fenêtres » décochée) le lui disent par [`RoamView::touched`] : elle reste alors là comme
//! posée à la main, au lieu de croire son bord perdu.
//!
//! Le journal n'en dit que ce qu'elle décide et des nombres (« elle change de place : 4 bords
//! possibles »), jamais rien d'une fenêtre.

use capri_claude::Tone;
use capri_core::look::Look;
use capri_core::perch::{LurkStature, Seen, Stature};
use capri_core::roam::{Busy, Choice, Event, Hold, Roamer, World};
use capri_core::{Capri, Rect, Vec2};
use capri_platform::Overlay;

use crate::perch_view::{self, PerchView};

pub struct RoamView {
    roamer: Roamer,
    /// La case de son menu, telle que retenue (le menu n'existe que sur macOS et Windows).
    #[cfg(any(target_os = "macos", windows))]
    enabled: bool,
    /// Une répétition de la vraie app : elle ne part pas d'elle-même, case cochée ou non.
    #[cfg(any(target_os = "macos", windows))]
    rehearsing: bool,
}

impl RoamView {
    /// Lancée (l'horloge de l'app à 0) : `enabled`, la case de son menu ; `seed`, la graine de sa
    /// vie.
    pub fn new(enabled: bool, rehearsing: bool, seed: u64) -> Self {
        Self {
            roamer: Roamer::new(seed, 0.0, enabled && !rehearsing),
            #[cfg(any(target_os = "macos", windows))]
            enabled,
            #[cfg(any(target_os = "macos", windows))]
            rehearsing,
        }
    }

    /// Le réglage se change dans son menu, qui n'existe que sur macOS et Windows.
    #[cfg(any(target_os = "macos", windows))]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Le lui permettre (elle repart bientôt), ou le lui interdire (elle reste où elle est ; une
    /// place déjà en chemin est atteinte).
    #[cfg(any(target_os = "macos", windows))]
    pub fn set_enabled(&mut self, enabled: bool, now: f64) {
        self.enabled = enabled;
        self.roamer.set_enabled(enabled && !self.rehearsing, now);
    }

    /// « Changer de place » : au prochain pas, si rien ne l'en empêche.
    #[cfg(any(target_os = "macos", windows))]
    pub fn ask(&mut self) {
        self.roamer.ask();
    }

    /// On vient de la montrer : elle reste un moment là où elle réapparaît.
    pub fn came_back(&mut self, now: f64, capri: &Capri) {
        self.roamer.came_back(now, capri);
    }

    /// Un pas, après `Look::update` : elle part, ou apparaît là où elle allait. Rend ce qu'elle a à
    /// dire au-dessus de sa tête, s'il y a lieu (seulement quand on lui a demandé de changer de
    /// place et qu'elle ne le peut pas).
    pub fn step(
        &mut self,
        look: &mut Look,
        capri: &mut Capri,
        desk: &mut Desk,
        busy: Busy,
        now: f64,
        frame: u64,
    ) -> Option<(&'static str, Tone)> {
        let events = self.roamer.step(look, capri, desk, busy, now, frame);
        // chaque événement écrit sa ligne du journal ; elle dit le dernier qui a quelque chose à dire
        let mut said = None;
        for event in events {
            said = tell(event).or(said);
        }
        said
    }

    /// L'utilisateur vient d'agir sur elle (sa taille, la case « S'asseoir sur les fenêtres ») :
    /// si cela l'a fait quitter son bord, elle reste là, comme posée à la main.
    pub fn touched(&mut self, now: f64, capri: &Capri) {
        self.roamer.touched(now, capri);
    }
}

/// Ce que l'app lui donne, le temps d'un pas.
pub struct Desk<'a> {
    pub perch: &'a mut PerchView,
    pub overlay: &'a Overlay,
    /// L'horloge de l'app, en secondes.
    pub now: f64,
    /// Le curseur, en pixels de l'overlay, tel que lu à ce pas.
    pub cursor: Option<Vec2>,
}

impl World for Desk<'_> {
    fn screen(&self) -> Rect {
        perch_view::screen(self.overlay)
    }

    fn cursor(&self) -> Option<Vec2> {
        self.cursor
    }

    fn perch_allowed(&self) -> bool {
        self.perch.is_enabled()
    }

    fn seen(&mut self, stature: Stature) -> Option<Vec<Seen>> {
        self.perch.windows(stature, self.overlay)
    }

    fn recheck(&mut self, capri: &mut Capri, stature: Stature, lurking: Option<LurkStature>) {
        self.perch.arrived(capri, self.overlay, self.now, stature, lurking);
    }
}

/// Le journal (ce qu'elle décide, et des nombres), et ce qu'elle dit quand on lui a demandé de
/// changer de place sans qu'elle le puisse.
fn tell(event: Event) -> Option<(&'static str, Tone)> {
    let benches =
        |count: usize| format!("{count} {}", if count <= 1 { "bord possible" } else { "bords possibles" });
    let sides =
        |count: usize| format!("{count} {}", if count <= 1 { "côté possible" } else { "côtés possibles" });
    match event {
        Event::Leaves { to, benches: count, flanks, asked, walking } => {
            log::info!(
                "elle change de place{} : {}, {}, {}",
                if asked { " (demandé)" } else { "" },
                benches(count),
                sides(flanks),
                match (to, walking) {
                    (Choice::Ledge(_), _) => "elle va s'asseoir sur l'un d'eux".to_owned(),
                    (Choice::Flank(lurk), _) => {
                        format!(
                            "elle va guetter derrière le côté {} d'une fenêtre",
                            lurk.spot.side.describe()
                        )
                    }
                    (_, true) => "elle va en marchant ailleurs en bas de l'écran".to_owned(),
                    (_, false) => "elle va se tenir debout en bas de l'écran".to_owned(),
                }
            );
            None
        }
        Event::Stays { benches: count, flanks, asked } => {
            log::debug!(
                "elle reste où elle est{} : {}, {}, aucune autre place",
                if asked { " (demandé)" } else { "" },
                benches(count),
                sides(flanks)
            );
            asked.then_some(("Je ne vois pas d'autre place où aller", Tone::Done))
        }
        Event::Arrived { seated, lurking, lost } => {
            log::debug!(
                "arrivée {}",
                match (seated, lurking, lost) {
                    (true, _, _) => "assise",
                    (false, true, _) => "derrière le côté d'une fenêtre : elle guette",
                    (false, false, true) => "debout : le bord ou le côté où elle allait n'y était plus",
                    (false, false, false) => "debout",
                }
            );
            None
        }
        Event::TurnedBack => {
            log::debug!("on lui parle en chemin : elle revient sur ses pas");
            None
        }
        Event::Halted => {
            log::debug!("on lui parle pendant qu'elle marche : elle s'arrête là");
            None
        }
        Event::Fidgeted => {
            log::debug!("assise, un éclat");
            None
        }
        Event::Refused(hold) => {
            log::debug!("elle ne change pas de place : {}", hold.describe());
            match hold {
                Hold::Talking => Some(("Pas pendant qu'on se parle", Tone::Done)),
                Hold::Music => Some(("Pas maintenant, j'écoute ta musique", Tone::Done)),
                Hold::Carried | Hold::Away | Hold::Walking => None,
            }
        }
        Event::LiesDown => {
            log::info!("repos : elle s'allonge sur son bord");
            None
        }
        Event::FallsAsleep { lying } => {
            log::info!("repos : elle s'endort {}", if lying { "allongée" } else { "debout" });
            None
        }
        Event::WakesUp { lying, woken } => {
            log::info!(
                "repos : elle se réveille {}, {}",
                if lying { "allongée" } else { "debout" },
                if woken { "on s'est occupé d'elle" } else { "d'elle-même" }
            );
            None
        }
        Event::SitsUp { asleep } => {
            log::info!("repos : elle se rassoit{}", if asleep { ", réveillée" } else { "" });
            None
        }
        Event::Unsettled { why, asleep } => {
            log::info!(
                "repos : {}, elle n'est plus allongée{}",
                why.describe(),
                if asleep { " (elle dormait)" } else { "" }
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn she_only_speaks_when_asked_and_unable() {
        assert_eq!(
            tell(Event::Leaves {
                to: Choice::Floor(Vec2::ZERO),
                benches: 4,
                flanks: 0,
                asked: true,
                walking: true
            }),
            None
        );
        assert_eq!(tell(Event::TurnedBack), None);
        assert_eq!(tell(Event::Halted), None);
        assert_eq!(tell(Event::Fidgeted), None);
        assert_eq!(tell(Event::Refused(Hold::Walking)), None, "elle arrive bientôt");
        assert_eq!(tell(Event::Arrived { seated: false, lurking: false, lost: true }), None);
        assert_eq!(tell(Event::Arrived { seated: false, lurking: true, lost: false }), None);
        assert_eq!(
            tell(Event::Stays { benches: 0, flanks: 2, asked: false }),
            None,
            "d'elle-même, sans un mot"
        );
        assert!(tell(Event::Stays { benches: 1, flanks: 0, asked: true }).is_some());
        assert!(tell(Event::Refused(Hold::Talking)).is_some());
        assert!(tell(Event::Refused(Hold::Music)).is_some());
        assert_eq!(tell(Event::Refused(Hold::Away)), None);
        assert_eq!(tell(Event::Refused(Hold::Carried)), None);
        // son repos : rien au-dessus de sa tête
        for event in [
            Event::LiesDown,
            Event::FallsAsleep { lying: true },
            Event::FallsAsleep { lying: false },
            Event::WakesUp { lying: true, woken: true },
            Event::WakesUp { lying: false, woken: false },
            Event::SitsUp { asleep: true },
            Event::Unsettled { why: capri_core::look::Unsettled::NoRoom, asleep: true },
            Event::Unsettled { why: capri_core::look::Unsettled::LedgeGone, asleep: false },
        ] {
            assert_eq!(tell(event), None, "{event:?}");
        }
    }
}
