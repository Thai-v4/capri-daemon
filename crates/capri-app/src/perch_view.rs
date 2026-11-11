//! Elle s'assoit sur tes fenêtres (jalon 3) : quand lire la liste des fenêtres, et ce que Capri
//! en fait. Les décisions sont dans `capri_core::perch`, la liste dans `capri_platform::desk`.
//!
//! La liste n'est lue que quand elle sert :
//! - une fois quand on la lâche doucement ou qu'un lancer s'arrête, pour chercher un bord sous ses
//!   pieds ;
//! - pendant qu'elle est posée, à la cadence de `perch::Watch` : toutes les 6 images (4 fois par
//!   seconde) quand rien ne bouge, à chaque image (24 fois par seconde) pendant que sa fenêtre
//!   bouge ou qu'un bouton de la souris est enfoncé. Une lecture coûte environ 0,2 ms sur un
//!   MacBook Pro M5 Pro (voir le README).
//!
//! Jamais pendant qu'on la tient ou qu'elle vole, jamais quand elle est cachée (la boucle ne
//! l'appelle pas), jamais quand c'est désactivé. Le journal n'en dit que des nombres.

use capri_core::perch::{self, Seen, Watch};
use capri_core::{Capri, Rect, Stance, Vec2};
use capri_platform::Overlay;
use capri_platform::desk::{self, DeskError};

pub struct PerchView {
    enabled: bool,
    watch: Watch,
    /// Au pas précédent, on la tenait ou elle volait.
    moving: bool,
    /// La dernière image où elle a regardé s'il fallait relire : une fois par image suffit.
    looked_at: Option<u64>,
    /// Ce système n'a pas de liste des fenêtres, et le journal l'a déjà dit.
    told_unavailable: bool,
}

impl PerchView {
    pub fn new(enabled: bool) -> Self {
        Self { enabled, watch: Watch::new(), moving: false, looked_at: None, told_unavailable: false }
    }

    /// À chaque pas de la boucle, après celui de Capri, à l'image `frame` : si elle vient de
    /// s'arrêter (lâchée doucement, ou au bout d'un lancer), se pose-t-elle sur un bord ? Posée,
    /// elle suit sa fenêtre, ou la quitte.
    pub fn tick(&mut self, capri: &mut Capri, overlay: &Overlay, frame: u64) {
        let moving = matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. });
        if std::mem::replace(&mut self.moving, moving) && capri.stance() == Stance::Floating {
            self.settle(capri, overlay, frame);
        }
        self.ride(capri, overlay, frame);
    }

    /// Le réglage se change dans son menu, qui n'existe que sur macOS et Windows.
    #[cfg(any(target_os = "macos", windows))]
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

    /// Elle revient d'être cachée : posée, elle relit la liste tout de suite.
    pub fn wake(&mut self) {
        self.watch.wake();
    }

    /// Elle vient de s'arrêter, à l'image `frame` : se pose-t-elle sur un bord ?
    fn settle(&mut self, capri: &mut Capri, overlay: &Overlay, frame: u64) {
        if !self.enabled || capri.stance() != Stance::Floating {
            return;
        }
        let Some(seen) = self.read(capri, overlay) else { return };
        let desk = overlay.desk_frame();
        let counted = format!(
            "{} fenêtres comptées ; overlay à ({}, {}) dans leur repère, {} px par unité",
            seen.len(),
            desk.origin.x,
            desk.origin.y,
            desk.pixels_per_unit
        );
        let Some(perch) = perch::landing(&seen, screen(overlay), capri.stature(), capri.feet()) else {
            log::debug!("aucun bord sous ses pieds ({counted})");
            return;
        };
        capri.land(perch);
        self.watch.wake();
        self.watch.saw(frame, rect_of(&seen, perch.spot.window));
        log::debug!(
            "posée sur un bord : ses pieds en ({:.0}, {:.0}) px ({counted})",
            perch.feet.x,
            perch.feet.y
        );
    }

    /// Posée, elle suit sa fenêtre, ou la quitte.
    fn ride(&mut self, capri: &mut Capri, overlay: &Overlay, frame: u64) {
        let Some(spot) = capri.perched() else { return };
        if !self.enabled {
            return capri.take_off();
        }
        if self.looked_at == Some(frame) {
            return;
        }
        self.looked_at = Some(frame);
        if !self.watch.due(frame, desk::mouse_down()) {
            return;
        }
        let Some(seen) = self.read(capri, overlay) else {
            log::debug!("quitte le bord : liste des fenêtres illisible");
            return capri.take_off();
        };
        match perch::follow(&seen, screen(overlay), capri.stature(), spot) {
            Ok(perch) => {
                capri.ride(perch);
                self.watch.saw(frame, rect_of(&seen, spot.window));
            }
            Err(why) => {
                log::debug!("quitte le bord : {}", why.describe());
                capri.take_off();
            }
        }
    }

    /// Les fenêtres qui comptent, en pixels de l'overlay ; `None` si le système ne les donne pas.
    fn read(&mut self, capri: &Capri, overlay: &Overlay) -> Option<Vec<Seen>> {
        match desk::windows() {
            Ok(windows) => Some(perch::sift(&windows, overlay.desk_frame(), screen(overlay), capri.size().y)),
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
fn screen(overlay: &Overlay) -> Rect {
    let (width, height) = overlay.size();
    Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32))
}

fn rect_of(seen: &[Seen], window: u64) -> Option<Rect> {
    seen.iter().find(|seen| seen.id == window).map(|seen| seen.rect)
}
