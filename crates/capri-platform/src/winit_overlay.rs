//! L'overlay porté par une fenêtre winit : Windows et les autres systèmes.
//!
//! Les réglages propres à chaque système restent dans `windows.rs` et `other.rs` ; ce fichier
//! ne fait que les enchaîner et traduire les événements de winit.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::Arc;

use raw_window_handle::{HandleError, HasWindowHandle, WindowHandle};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowLevel};

use crate::{OverlayError, OverlayEvent, imp};

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
