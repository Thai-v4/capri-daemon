//! Capri, jalon 1 : elle flotte sur ton bureau, on peut l'attraper, la poser et la lancer.
//!
//! Une seule fenêtre transparente couvre l'écran principal. Elle laisse passer la souris
//! partout, sauf sur Capri elle-même. Le curseur est lu auprès du système 60 fois par seconde
//! (la fenêtre ne reçoit pas les mouvements quand elle se laisse traverser), mais on ne
//! redessine que quand sa pose change : environ 12 fois par seconde au repos.

mod assets;
#[cfg(any(target_os = "macos", windows))]
mod menu;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use capri_core::{AlphaMask, Capri, Pose, Rect, Vec2, frame_at};
use capri_render::{Renderer, SpriteDraw, SpriteImage};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowLevel};

/// Sa hauteur à l'écran, en points (pixels logiques).
const HEIGHT_POINTS: f64 = 300.0;
/// Lecture du curseur et pas de simulation.
const TICK: Duration = Duration::from_micros(16_667);
/// Amplitude du « boil », en pixels du sprite d'origine.
const BOIL: f32 = 1.0;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let sprite = assets::load_sprite()?;

    let mut builder = EventLoop::builder();
    capri_platform::configure_event_loop(&mut builder);
    let event_loop = builder.build().context("création de la boucle d'événements")?;

    let mut app = App { sprite: Some(sprite), ..App::default() };
    event_loop.run_app(&mut app)?;
    app.fatal.map_or(Ok(()), Err)
}

#[derive(Default)]
struct App {
    sprite: Option<assets::Sprite>,
    running: Option<Running>,
    hidden: bool,
    hotkey: Option<(GlobalHotKeyManager, u32)>,
    #[cfg(any(target_os = "macos", windows))]
    menus: Option<menu::Menus>,
    fatal: Option<anyhow::Error>,
}

struct Running {
    window: Arc<Window>,
    renderer: Renderer,
    capri: Capri,
    mask: AlphaMask,
    start: Instant,
    last_step: Instant,
    cursor: Option<Vec2>,
    click_through: bool,
    drawn: Option<Pose>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match self.start(event_loop) {
            Ok(running) => self.running = Some(running),
            Err(err) => {
                self.fatal = Some(err);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(r) = self.running.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                r.renderer.resize(size.width, size.height);
                r.capri.set_bounds(Rect::new(Vec2::ZERO, Vec2::new(size.width as f32, size.height as f32)));
                r.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                r.capri.set_size(display_size(r.mask.size(), scale_factor));
            }
            WindowEvent::CursorMoved { position, .. } => {
                r.cursor = Some(Vec2::new(position.x as f32, position.y as f32));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                // le curseur le plus frais, même si aucun mouvement n'est arrivé depuis
                r.read_cursor();
                let now = r.seconds();
                match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => {
                        if let Some(c) = r.cursor.filter(|&c| r.is_over(c)) {
                            r.capri.grab(c, now);
                        }
                    }
                    (MouseButton::Left, ElementState::Released) => r.capri.release(now),
                    (MouseButton::Right, ElementState::Pressed) => {
                        #[cfg(any(target_os = "macos", windows))]
                        if r.cursor.is_some_and(|c| r.is_over(c))
                            && let Some(menus) = &self.menus
                        {
                            menus.show_context(&r.window);
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => r.draw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_menus(event_loop);
        self.handle_hotkey();
        if !self.hidden
            && let Some(r) = self.running.as_mut()
        {
            r.tick();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + TICK));
    }
}

impl App {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running> {
        let sprite = self.sprite.take().context("démarrage déjà fait")?;
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .context("aucun écran détecté")?;

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
        let window = Arc::new(event_loop.create_window(capri_platform::overlay_attributes(attrs))?);
        capri_platform::prepare_overlay(&window);

        let size = window.inner_size();
        let renderer = Renderer::new(
            window.clone(),
            size.width,
            size.height,
            SpriteImage { width: sprite.width, height: sprite.height, rgba: &sprite.rgba },
        )?;
        if !renderer.is_transparent() {
            log::warn!("la fenêtre ne peut pas être transparente sur cette machine");
        }

        let bounds = Rect::new(Vec2::ZERO, Vec2::new(size.width as f32, size.height as f32));
        let body = display_size(sprite.mask.size(), window.scale_factor());
        // en bas à droite, au-dessus du Dock ou de la barre des tâches
        let start_at = Vec2::new(bounds.size.x - body.x * 1.8, bounds.size.y - body.y * 1.3);
        let capri = Capri::new(body, bounds, start_at);

        capri_platform::set_click_through(&window, true);
        capri_platform::set_shown(&window, true);
        window.request_redraw();

        self.hotkey = register_hotkey();
        #[cfg(any(target_os = "macos", windows))]
        {
            self.menus = Some(menu::Menus::new());
        }
        if !capri_platform::INTERACTIVE {
            log::warn!("système pas encore pris en charge : Capri s'affiche mais ne peut pas être attrapée");
        }

        let now = Instant::now();
        Ok(Running {
            window,
            renderer,
            capri,
            mask: sprite.mask,
            start: now,
            last_step: now,
            cursor: None,
            click_through: true,
            drawn: None,
        })
    }

    fn handle_menus(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(any(target_os = "macos", windows))]
        {
            let actions = self.menus.as_ref().map(menu::Menus::pending_actions).unwrap_or_default();
            for action in actions {
                match action {
                    menu::Action::ToggleVisible => self.toggle_hidden(),
                    menu::Action::Quit => event_loop.exit(),
                }
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        let _ = event_loop;
    }

    fn handle_hotkey(&mut self) {
        let Some((_, id)) = &self.hotkey else { return };
        let id = *id;
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == id && event.state == HotKeyState::Pressed {
                self.toggle_hidden();
            }
        }
    }

    fn toggle_hidden(&mut self) {
        let Some(r) = self.running.as_mut() else { return };
        self.hidden = !self.hidden;
        capri_platform::set_click_through(&r.window, true);
        r.click_through = true;
        capri_platform::set_shown(&r.window, !self.hidden);
        if !self.hidden {
            r.drawn = None; // redessiner en revenant
            r.last_step = Instant::now();
        }
        #[cfg(any(target_os = "macos", windows))]
        if let Some(menus) = &self.menus {
            menus.set_visible(!self.hidden);
        }
    }
}

impl Running {
    fn seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn read_cursor(&mut self) {
        if let Some((x, y)) = capri_platform::cursor_in_window(&self.window) {
            self.cursor = Some(Vec2::new(x as f32, y as f32));
        }
    }

    /// Le curseur est-il sur elle (au pixel près, avec une petite marge) ?
    fn is_over(&self, cursor: Vec2) -> bool {
        self.capri.pose().rect.to_unit(cursor).is_some_and(|uv| self.mask.hit(uv))
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_step).as_secs_f32().min(0.1);
        self.last_step = now;

        self.read_cursor();
        if self.capri.is_held()
            && let Some(c) = self.cursor
        {
            self.capri.drag_to(c, self.seconds());
        }
        self.capri.step(dt, frame_at(now - self.start));

        // La souris traverse la fenêtre partout, sauf sur elle ou tant qu'on la tient.
        let over = self.cursor.is_some_and(|c| self.is_over(c));
        let through = !capri_platform::INTERACTIVE || !(over || self.capri.is_held());
        if through != self.click_through {
            capri_platform::set_click_through(&self.window, through);
            self.click_through = through;
        }

        if self.drawn != Some(self.capri.pose()) {
            self.window.request_redraw();
        }
    }

    fn draw(&mut self) {
        let pose = self.capri.pose();
        let unit = pose.rect.size.y / 420.0;
        let draw = SpriteDraw {
            min: [pose.rect.min.x, pose.rect.min.y],
            size: [pose.rect.size.x, pose.rect.size.y],
            ghost: [pose.ghost.x, pose.ghost.y],
            boil_seed: pose.boil_seed,
            boil: BOIL * unit,
            opacity: 1.0,
        };
        let window = Arc::clone(&self.window);
        self.renderer.render(&draw, || window.pre_present_notify());
        self.drawn = Some(pose);
    }
}

/// Sa taille à l'écran en pixels physiques, en gardant les proportions du sprite.
fn display_size((width, height): (u32, u32), scale_factor: f64) -> Vec2 {
    let h = (HEIGHT_POINTS * scale_factor) as f32;
    Vec2::new(h * width as f32 / height as f32, h)
}

/// ⌃⌥C (Ctrl+Alt+C) : la cacher ou la faire revenir, depuis n'importe quelle app.
fn register_hotkey() -> Option<(GlobalHotKeyManager, u32)> {
    let manager = GlobalHotKeyManager::new()
        .inspect_err(|err| log::warn!("raccourci global indisponible : {err}"))
        .ok()?;
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyC);
    manager.register(hotkey).inspect_err(|err| log::warn!("raccourci ⌃⌥C déjà pris : {err}")).ok()?;
    Some((manager, hotkey.id()))
}
