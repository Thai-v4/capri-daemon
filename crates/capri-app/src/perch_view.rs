//! Elle s'assoit sur tes fenêtres (jalon 3) : quand lire la liste des fenêtres, et ce que Capri
//! en fait. Les décisions sont dans `capri_core::perch`, la liste dans `capri_platform::desk`.
//!
//! La liste n'est lue que quand elle sert :
//! - une fois quand on la lâche doucement ou qu'un lancer s'arrête, pour chercher un bord sous ses
//!   pieds ;
//! - pendant qu'elle est posée, à la cadence de `perch::Watch` : 4 fois par seconde quand rien ne
//!   bouge, à chaque rafraîchissement de l'écran (120 fois par seconde en ProMotion) pendant que sa
//!   fenêtre bouge ou qu'un bouton de la souris est enfoncé, et encore une demi-seconde après. La
//!   boucle se réveille alors aussi souvent (`hurried`, et `capri_core::pace`). Une lecture coûte
//!   0,16 à 0,17 ms en médiane et 0,25 à 0,4 ms en moyenne sur un MacBook Pro M5 Pro ; pendant
//!   qu'une fenêtre bouge, elle dure 3,5 ms, dont 0,15 ms de processeur : le reste est l'attente du
//!   serveur de fenêtres (voir le README) ;
//! - une fois quand elle choisit d'elle-même où aller, ou qu'on lui demande de changer de place
//!   (`roam_view`, [`PerchView::windows`]) ; et, quand elle y va s'asseoir, juste avant qu'elle
//!   apparaisse sur son bord ([`PerchView::arrived`]) : c'est la première lecture de sa veille
//!   posée, avancée, pour qu'elle n'apparaisse pas sur une fenêtre partie ou déplacée depuis
//!   qu'elle l'a choisie.
//!
//! Jamais pendant qu'on la tient ou qu'elle vole, jamais quand elle est cachée (la boucle ne
//! l'appelle pas), jamais quand c'est désactivé. Le journal n'en dit que des nombres.
//!
//! **Pour s'allonger** : de la lecture qu'elle fait déjà posée (ou après un changement de taille),
//! elle déduit aussi s'il y a la place pour elle allongée là où elle est
//! (`capri_core::perch::fits` : [`PerchView::room_to_lie`]). Aucune lecture de plus.

use capri_core::perch::{self, Seen, Stature, Watch};
use capri_core::{Capri, Rect, Stance, Vec2};
use capri_platform::Overlay;
use capri_platform::desk::{self, DeskError};

pub struct PerchView {
    enabled: bool,
    watch: Watch,
    /// D'après la dernière lecture faite posée : il y a la place pour elle allongée, là où elle est.
    room_to_lie: bool,
    /// Au pas précédent, on la tenait ou elle volait.
    moving: bool,
    /// Ce système n'a pas de liste des fenêtres, et le journal l'a déjà dit.
    told_unavailable: bool,
    /// En version de débogage, pendant `CAPRI_PERCH_REHEARSAL` : chaque lecture et chaque image
    /// dessinée pendant qu'elle est posée, pour mesurer son retard sur sa fenêtre.
    #[cfg(debug_assertions)]
    pub trace: Option<crate::perch_rehearsal::Trace>,
    /// En version de débogage : combien de fois la liste a été lue, et combien d'images ont été
    /// dessinées, depuis le lancement.
    #[cfg(debug_assertions)]
    pub reads: u64,
    #[cfg(debug_assertions)]
    pub drawn: u64,
    /// En version de débogage, pendant `CAPRI_PERCH_REHEARSAL` : faire comme si un bouton de la
    /// souris était enfoncé.
    #[cfg(debug_assertions)]
    pub pretend_pressed: bool,
}

impl PerchView {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            watch: Watch::new(),
            room_to_lie: false,
            moving: false,
            told_unavailable: false,
            #[cfg(debug_assertions)]
            trace: None,
            #[cfg(debug_assertions)]
            reads: 0,
            #[cfg(debug_assertions)]
            drawn: 0,
            #[cfg(debug_assertions)]
            pretend_pressed: false,
        }
    }

    /// Un bouton de la souris est-il enfoncé (l'indice qu'une fenêtre va peut-être bouger) ?
    fn mouse_down(&self) -> bool {
        #[cfg(debug_assertions)]
        if self.pretend_pressed {
            return true;
        }
        desk::mouse_down()
    }

    /// À chaque pas de la boucle, après celui de Capri, à l'instant `now` (secondes) : si elle
    /// vient de s'arrêter (lâchée doucement, ou au bout d'un lancer), se pose-t-elle sur un bord ?
    /// Posée, elle suit sa fenêtre, ou la quitte. `period` : la durée d'un rafraîchissement de
    /// l'écran, en secondes ; `stature` : ce qu'il lui faut sur un bord (`Look::stature`) ;
    /// `lying` : pour s'y allonger (`Look::lying_stature`), si elle le peut.
    pub fn tick(
        &mut self,
        capri: &mut Capri,
        overlay: &Overlay,
        now: f64,
        period: f64,
        stature: Stature,
        lying: Option<Stature>,
    ) {
        let moving = matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. });
        if std::mem::replace(&mut self.moving, moving) && capri.stance() == Stance::Floating {
            self.settle(capri, overlay, now, stature);
        }
        self.ride(capri, overlay, now, period, stature, lying);
    }

    /// D'après la dernière lecture faite posée : y a-t-il la place pour elle allongée, là où elle
    /// est (`perch::fits`) ? Jamais quand elle n'est pas posée.
    pub fn room_to_lie(&self) -> bool {
        self.room_to_lie
    }

    /// De cette lecture (`seen`), posée en `spot` : la place pour elle allongée (`lying`), ou non.
    fn measure_room(&mut self, seen: &[Seen], overlay: &Overlay, spot: perch::Spot, lying: Option<Stature>) {
        self.room_to_lie = lying.is_some_and(|lying| perch::fits(seen, screen(overlay), lying, spot));
    }

    /// Posée, est-elle pressée (un bouton de la souris enfoncé, ou sa fenêtre qui vient de
    /// bouger) ? Alors la boucle doit se réveiller à chaque rafraîchissement de l'écran.
    pub fn hurried(&self, capri: &Capri, now: f64) -> bool {
        #[cfg(debug_assertions)]
        if self.trace.as_ref().is_some_and(|trace| trace.legacy) {
            return false; // la répétition « avant » : la boucle à 60 Hz, comme avant ce réglage
        }
        self.enabled && capri.perched().is_some() && self.watch.hurried(now, self.mouse_down())
    }

    /// S'assoit-elle sur les fenêtres ? Le réglage se change dans son menu, qui n'existe que sur
    /// macOS et Windows.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Le lui permettre, ou le lui interdire : posée, elle repart aussitôt.
    #[cfg(any(target_os = "macos", windows))]
    pub fn set_enabled(&mut self, enabled: bool, capri: &mut Capri) {
        self.enabled = enabled;
        if !enabled && capri.perched().is_some() {
            log::debug!("quitte le bord : désactivé");
            capri.take_off();
        }
    }

    /// Elle vient de changer de taille : posée, la place au-dessus de son bord lui suffit-elle
    /// encore ? Oui : elle y reste, ses pieds glissés s'il le faut pour tenir dans l'écran ; non
    /// (ou la liste illisible) : elle repart flotter, là où elle est. Et allongée (`lying`) ?
    pub fn resized(
        &mut self,
        capri: &mut Capri,
        overlay: &Overlay,
        now: f64,
        stature: Stature,
        lying: Option<Stature>,
    ) {
        self.room_to_lie = false;
        let Some(spot) = capri.perched() else { return };
        let Some(seen) = self.read(stature, overlay) else {
            log::debug!("quitte le bord : liste des fenêtres illisible");
            return capri.take_off();
        };
        match perch::follow(&seen, screen(overlay), stature, spot) {
            Ok(perch) => {
                capri.ride(perch);
                self.watch.saw(now, rect_of(&seen, spot.window));
                self.measure_room(&seen, overlay, perch.spot, lying);
            }
            Err(why) => {
                log::debug!("quitte le bord après avoir changé de taille : {}", why.describe());
                capri.take_off();
            }
        }
    }

    /// Elle revient d'être cachée : posée, elle relit la liste tout de suite.
    pub fn wake(&mut self) {
        self.watch.wake();
    }

    /// Elle vient d'être posée sur le bord qu'elle a choisi (`roam_view`), à l'instant `now`, et va
    /// y apparaître : sa fenêtre relue tout de suite, comme le ferait sa veille posée. Toujours
    /// là, elle la suit ; partie, déplacée hors de sa portée, la liste illisible, ou si elle ne
    /// s'assoit plus sur les fenêtres : elle la quitte, et apparaîtra debout.
    pub fn arrived(&mut self, capri: &mut Capri, overlay: &Overlay, now: f64, stature: Stature) {
        // la place pour s'allonger sera sue à la lecture suivante de sa veille posée
        self.room_to_lie = false;
        let Some(spot) = capri.perched() else { return };
        if !self.enabled {
            return capri.take_off();
        }
        self.watch.wake();
        let Some(seen) = self.read(stature, overlay) else {
            log::debug!("ne se pose pas sur le bord choisi : liste des fenêtres illisible");
            return capri.take_off();
        };
        match perch::follow(&seen, screen(overlay), stature, spot) {
            Ok(perch) => {
                capri.ride(perch);
                self.watch.saw(now, rect_of(&seen, spot.window));
            }
            Err(why) => {
                log::debug!("ne se pose pas sur le bord choisi : {}", why.describe());
                capri.take_off();
            }
        }
    }

    /// Les fenêtres qui comptent, lues maintenant, pour choisir où aller (`roam_view`) ; `None` si
    /// le système ne les donne pas.
    pub fn windows(&mut self, stature: Stature, overlay: &Overlay) -> Option<Vec<Seen>> {
        self.read(stature, overlay)
    }

    /// En version de débogage, pour `CAPRI_PERCH_REHEARSAL` : comme si on venait de la lâcher là
    /// où elle est.
    #[cfg(debug_assertions)]
    pub fn rehearse_settle(&mut self, capri: &mut Capri, overlay: &Overlay, now: f64, stature: Stature) {
        self.settle(capri, overlay, now, stature);
    }

    /// Elle vient de s'arrêter, à l'instant `now`, debout : se pose-t-elle sur un bord ? Sur un
    /// bord près de ses pieds, qui lui laisse la place une fois posée (`stature`).
    fn settle(&mut self, capri: &mut Capri, overlay: &Overlay, now: f64, stature: Stature) {
        self.room_to_lie = false;
        if !self.enabled || capri.stance() != Stance::Floating {
            return;
        }
        let Some(seen) = self.read(stature, overlay) else { return };
        let desk = overlay.desk_frame();
        let counted = format!(
            "{} fenêtres comptées ; overlay à ({}, {}) dans leur repère, {} px par unité",
            seen.len(),
            desk.origin.x,
            desk.origin.y,
            desk.pixels_per_unit
        );
        let Some(perch) = perch::landing(&seen, screen(overlay), stature, capri.feet()) else {
            log::debug!("aucun bord sous ses pieds ({counted})");
            return;
        };
        capri.land(perch);
        self.watch.wake();
        self.watch.saw(now, rect_of(&seen, perch.spot.window));
        log::debug!(
            "posée sur un bord : ses pieds en ({:.0}, {:.0}) px ({counted})",
            perch.feet.x,
            perch.feet.y
        );
    }

    /// Posée, elle suit sa fenêtre, ou la quitte ; et sait s'il y a la place pour elle allongée.
    fn ride(
        &mut self,
        capri: &mut Capri,
        overlay: &Overlay,
        now: f64,
        period: f64,
        stature: Stature,
        lying: Option<Stature>,
    ) {
        let Some(spot) = capri.perched() else {
            self.room_to_lie = false;
            return;
        };
        if !self.enabled {
            self.room_to_lie = false;
            return capri.take_off();
        }
        // la répétition « avant » : la cadence d'avant ce réglage, une lecture par image d'animation
        #[cfg(debug_assertions)]
        let period = match self.trace.as_mut() {
            Some(trace) if trace.legacy => {
                if trace.same_frame_as_before(now) {
                    return;
                }
                1.0 / capri_core::FRAME_RATE
            }
            _ => period,
        };
        if !self.watch.due(now, self.mouse_down(), period) {
            return;
        }
        let Some(seen) = self.read(stature, overlay) else {
            log::debug!("quitte le bord : liste des fenêtres illisible");
            self.room_to_lie = false;
            return capri.take_off();
        };
        #[cfg(debug_assertions)]
        if let Some(trace) = self.trace.as_mut() {
            trace.read(rect_of(&seen, spot.window));
        }
        match perch::follow(&seen, screen(overlay), stature, spot) {
            Ok(perch) => {
                capri.ride(perch);
                self.watch.saw(now, rect_of(&seen, spot.window));
                self.measure_room(&seen, overlay, perch.spot, lying);
            }
            Err(why) => {
                log::debug!("quitte le bord : {}", why.describe());
                self.room_to_lie = false;
                capri.take_off();
            }
        }
    }

    /// Les fenêtres qui comptent, en pixels de l'overlay ; `None` si le système ne les donne pas.
    fn read(&mut self, stature: Stature, overlay: &Overlay) -> Option<Vec<Seen>> {
        #[cfg(debug_assertions)]
        let (started, cpu) = {
            self.reads += 1;
            (std::time::Instant::now(), desk::thread_cpu_time())
        };
        let listed = desk::windows();
        #[cfg(debug_assertions)]
        if let Some(trace) = self.trace.as_mut() {
            trace.timed(started.elapsed(), cpu, desk::thread_cpu_time());
        }
        match listed {
            Ok(windows) => Some(perch::sift(&windows, overlay.desk_frame(), screen(overlay), stature.height)),
            Err(DeskError::Unavailable) => {
                if !std::mem::replace(&mut self.told_unavailable, true) {
                    log::info!("pas de liste des fenêtres sur ce système : elle ne s'assoit pas dessus");
                }
                None
            }
            Err(err) => {
                log::debug!("fenêtres : {err}");
                None
            }
        }
    }
}

/// L'overlay, en ses pixels : la zone où elle vit.
pub fn screen(overlay: &Overlay) -> Rect {
    let (width, height) = overlay.size();
    Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32))
}

fn rect_of(seen: &[Seen], window: u64) -> Option<Rect> {
    seen.iter().find(|seen| seen.id == window).map(|seen| seen.rect)
}
