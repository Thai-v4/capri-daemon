//! L'overlay porté par une fenêtre winit : Windows et les autres systèmes.
//!
//! Les réglages propres à chaque système restent dans `windows.rs` et `other.rs` ; ce fichier
//! ne fait que les enchaîner et traduire les événements de winit.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::Arc;

use raw_window_handle::{HandleError, HasWindowHandle, WindowHandle};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowLevel};

use crate::{OverlayError, OverlayEvent, imp};

/// Sur un pavé tactile, combien de points de défilement font un cran de molette.
const PIXELS_PER_STEP: f64 = 10.0;

pub struct Overlay {
    window: Arc<Window>,
    events: RefCell<VecDeque<OverlayEvent>>,
    /// Touches modificatrices enfoncées, telles que la fenêtre les a vues en dernier.
    modifiers: Cell<ModifiersState>,
}

impl Overlay {
    pub fn new(event_loop: &ActiveEventLoop) -> Result<Self, OverlayError> {
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .ok_or(OverlayError::NoScreen)?;
        let attrs = Window::default_attributes()
            .with_title("Capri")
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_active(false)
            .with_visible(false)
            .with_position(monitor.position())
            .with_inner_size(monitor.size());
        let window = Arc::new(event_loop.create_window(imp::overlay_attributes(attrs))?);
        imp::prepare_overlay(&window);
        Ok(Self {
            window,
            events: RefCell::new(VecDeque::new()),
            modifiers: Cell::new(ModifiersState::empty()),
        })
    }

    pub fn surface_handle(&self) -> Arc<Window> {
        Arc::clone(&self.window)
    }

    pub fn size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width, size.height)
    }

    pub fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn cursor(&self) -> Option<(f64, f64)> {
        imp::cursor_in_window(&self.window)
    }

    pub fn set_click_through(&self, through: bool) {
        imp::set_click_through(&self.window, through);
    }

    pub fn desk_frame(&self) -> capri_core::perch::DeskFrame {
        imp::desk_frame(&self.window)
    }

    pub fn set_shown(&self, shown: bool) {
        imp::set_shown(&self.window, shown);
    }

    /// Celle de l'écran où est la fenêtre, d'après winit (sous Windows : `EnumDisplaySettingsExW`).
    pub fn refresh_rate(&self) -> Option<f64> {
        let millihertz = self.window.current_monitor()?.refresh_rate_millihertz()?;
        Some(f64::from(millihertz) / 1000.0)
    }

    /// Pas de réveil au rythme de l'écran avec une fenêtre winit : l'app règle son minuteur sur
    /// [`Overlay::refresh_rate`], et la présentation (`Fifo`) se cale sur l'écran.
    pub fn follow_refresh(&self, _on: bool) -> bool {
        false
    }

    pub fn refreshed(&self) -> bool {
        false
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn pre_present_notify(&self) {
        self.window.pre_present_notify();
    }

    /// Pas encore de champ de saisie avec une fenêtre winit : ni Windows ni Linux n'en ont.
    pub fn show_input(&self, _placeholder: &str, _place: impl FnOnce(f64, f64) -> (f64, f64)) -> bool {
        false
    }

    pub fn hide_input(&self) {}

    pub fn input_is_open(&self) -> bool {
        false
    }

    pub fn on_window_event(&self, event: &WindowEvent) {
        let translated = match event {
            WindowEvent::CloseRequested => OverlayEvent::CloseRequested,
            WindowEvent::Resized(size) => OverlayEvent::Resized { width: size.width, height: size.height },
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                OverlayEvent::ScaleFactorChanged(*scale_factor)
            }
            // winit met cet état à jour à chaque clic, juste avant `MouseInput`.
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers.set(modifiers.state());
                return;
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {
                OverlayEvent::Pressed { button: *button, ctrl: self.modifiers.get().control_key() }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button, .. } => {
                OverlayEvent::Released { button: *button }
            }
            WindowEvent::RedrawRequested => OverlayEvent::RedrawRequested,
            // la molette sur elle (la fenêtre ne reçoit la souris que là), la touche tenue : la
            // fenêtre n'a jamais le clavier, son état vient donc du système, au moment même
            WindowEvent::MouseWheel { delta, .. } if imp::zoom_modifier_down() => {
                // winit : positif quand le contenu descendrait, c'est-à-dire la molette vers le haut
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(*y),
                    MouseScrollDelta::PixelDelta(position) => {
                        position.y / (PIXELS_PER_STEP * self.scale_factor())
                    }
                };
                if !(steps.is_finite() && steps != 0.0) {
                    return;
                }
                OverlayEvent::Zoom { steps }
            }
            _ => return,
        };
        self.events.borrow_mut().push_back(translated);
    }

    pub fn drain_events(&self) -> Vec<OverlayEvent> {
        // On vide la file avant de rendre la main : traiter un événement peut ouvrir un menu,
        // dont la boucle imbriquée fait arriver d'autres événements dans cette même file.
        self.events.borrow_mut().drain(..).collect()
    }

    pub fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.window.window_handle()
    }
}
