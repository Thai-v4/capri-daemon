//! La seule couche qui parle au système d'exploitation.
//!
//! Capri vit dans une fenêtre transparente qui couvre l'écran : l'[`Overlay`]. Pour qu'elle ne
//! gêne jamais :
//! - la fenêtre laisse passer les clics, sauf quand le curseur est sur elle ;
//! - elle ne vole pas le focus et n'apparaît ni dans le Dock ni dans la barre des tâches ;
//! - sur macOS, elle suit tous les bureaux (Spaces) et passe au-dessus des apps en plein écran.
//!
//! Comme la fenêtre ignore la souris la plupart du temps, elle ne reçoit pas les mouvements :
//! on lit donc la position du curseur directement auprès du système.
//!
//! Deux implémentations :
//! - macOS (`macos.rs`) : un `NSPanel` non activant créé par nos soins, sans fenêtre winit ;
//! - ailleurs (`winit_overlay.rs`) : une fenêtre winit, réglée par `windows.rs` ou `other.rs`.
//!
//! C'est aussi ici que vivent sa voix ([`Voice`], la synthèse vocale du système) et son oreille
//! ([`Ear`], le micro et la reconnaissance vocale sur l'appareil).

use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopBuilder};

mod ear;
mod voice;
pub use ear::{Access, Ear, EarError, EarEvent, EarStatus, Permission};
pub use voice::{SystemVoice, Voice};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(target_os = "macos")]
use macos::Overlay as PlatformOverlay;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

#[cfg(not(any(target_os = "macos", windows)))]
mod other;
#[cfg(not(any(target_os = "macos", windows)))]
use other as imp;

#[cfg(not(target_os = "macos"))]
mod winit_overlay;
#[cfg(not(target_os = "macos"))]
use winit_overlay::Overlay as PlatformOverlay;

/// Ce système sait-il lire le curseur et laisser passer les clics au pixel près ?
/// Sinon, Capri s'affiche mais ne peut pas être attrapée.
pub const INTERACTIVE: bool = cfg!(any(target_os = "macos", windows));

/// Sur ce système, Ctrl + clic principal vaut-il un clic secondaire ? Sur macOS oui, et le
/// système l'envoie à l'app comme un clic principal avec Ctrl enfoncé.
pub const CTRL_CLICK_IS_SECONDARY: bool = cfg!(target_os = "macos");

/// Réglages de la boucle d'événements (macOS : pas d'icône dans le Dock, pas de prise de focus
/// au lancement).
pub fn configure_event_loop<T: 'static>(builder: &mut EventLoopBuilder<T>) {
    imp::configure_event_loop(builder);
}

/// Ce que l'overlay rapporte à l'application.
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayEvent {
    /// Un bouton de souris vient d'être enfoncé sur la fenêtre. `ctrl` : la touche Ctrl était
    /// tenue à ce moment-là.
    Pressed { button: MouseButton, ctrl: bool },
    /// Un bouton de souris vient d'être relâché.
    Released { button: MouseButton },
    /// Nouvelle taille de la fenêtre, en pixels physiques.
    Resized { width: u32, height: u32 },
    /// Nouvelle densité de l'écran (2.0 en Retina).
    ScaleFactorChanged(f64),
    /// Il est temps de dessiner, à la suite d'un [`Overlay::request_redraw`].
    RedrawRequested,
    /// Le système demande la fermeture de la fenêtre.
    CloseRequested,
    /// Le champ de saisie a été validé (Entrée), avec ce qu'il contenait.
    InputSubmitted(String),
    /// Le champ de saisie a été fermé sans valider : Échap, ou un clic ailleurs.
    InputCancelled,
}

#[derive(Debug, thiserror::Error)]
pub enum OverlayError {
    #[error("aucun écran détecté")]
    NoScreen,
    #[error("la fenêtre overlay doit être créée sur le thread principal")]
    NotMainThread,
    #[error("impossible de créer la fenêtre overlay : {0}")]
    Window(#[from] winit::error::OsError),
}

/// La fenêtre transparente de Capri, qui couvre l'écran principal.
pub struct Overlay(PlatformOverlay);

impl Overlay {
    /// Crée la fenêtre, encore cachée. À appeler une fois la boucle d'événements lancée, sur le
    /// thread principal.
    pub fn new(event_loop: &ActiveEventLoop) -> Result<Self, OverlayError> {
        PlatformOverlay::new(event_loop).map(Self)
    }

    /// De quoi créer la surface de rendu (à passer à `capri_render::Renderer::new`).
    pub fn surface_handle(&self) -> impl HasWindowHandle + HasDisplayHandle + Send + Sync + 'static {
        self.0.surface_handle()
    }

    /// Taille de la fenêtre, en pixels physiques.
    pub fn size(&self) -> (u32, u32) {
        self.0.size()
    }

    /// Densité de l'écran (2.0 en Retina).
    pub fn scale_factor(&self) -> f64 {
        self.0.scale_factor()
    }

    /// Position du curseur dans la fenêtre, en pixels physiques, même quand elle ignore la souris.
    pub fn cursor(&self) -> Option<(f64, f64)> {
        self.0.cursor()
    }

    /// `true` : les clics traversent la fenêtre ; `false` : elle les reçoit.
    pub fn set_click_through(&self, through: bool) {
        self.0.set_click_through(through);
    }

    /// Afficher ou cacher sans voler le focus de l'app au premier plan.
    pub fn set_shown(&self, shown: bool) {
        self.0.set_shown(shown);
    }

    /// Demande un [`OverlayEvent::RedrawRequested`].
    pub fn request_redraw(&self) {
        self.0.request_redraw();
    }

    /// À appeler juste avant de présenter une image (winit le demande pour bien les caler).
    pub fn pre_present_notify(&self) {
        self.0.pre_present_notify();
    }

    /// Ouvre un champ de saisie d'une ligne près de Capri et lui donne le clavier : c'est le seul
    /// moment où Capri le prend, et seulement parce qu'on le lui demande. Le champ se ferme sur
    /// Entrée ([`OverlayEvent::InputSubmitted`]), sur Échap ou sur un clic ailleurs
    /// ([`OverlayEvent::InputCancelled`]), et rend alors le clavier à la fenêtre d'avant.
    ///
    /// `place` reçoit la taille du champ et rend son coin haut gauche, le tout en pixels
    /// physiques de l'overlay. Rend `false`, sans rien ouvrir, sur les systèmes qui n'ont pas
    /// encore de champ de saisie.
    pub fn show_input(&self, placeholder: &str, place: impl FnOnce(f64, f64) -> (f64, f64)) -> bool {
        self.0.show_input(placeholder, place)
    }

    /// Ferme le champ de saisie s'il est ouvert, comme un abandon.
    pub fn hide_input(&self) {
        self.0.hide_input();
    }

    pub fn input_is_open(&self) -> bool {
        self.0.input_is_open()
    }

    /// À appeler pour chaque événement de fenêtre reçu de winit. Sur macOS, la fenêtre n'est pas
    /// une fenêtre winit : il n'en arrive aucun, et cet appel ne fait rien.
    pub fn on_window_event(&self, event: &WindowEvent) {
        self.0.on_window_event(event);
    }

    /// Les événements arrivés depuis le dernier appel, dans l'ordre.
    pub fn drain_events(&self) -> Vec<OverlayEvent> {
        self.0.drain_events()
    }
}

/// Pour ouvrir un menu contextuel sur la fenêtre.
impl HasWindowHandle for Overlay {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}
