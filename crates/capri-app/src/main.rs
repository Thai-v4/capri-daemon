//! Capri : elle flotte sur ton bureau, on peut l'attraper, la poser et la lancer (jalon 1), un
//! bandeau au-dessus d'elle dit ce que fait Claude Code (jalon 2), et on peut lui parler : elle
//! répond dans une bulle, avec le modèle local qu'on lui a donné (jalon 4).
//!
//! Une seule fenêtre transparente, l'overlay de `capri-platform`, couvre l'écran principal. Elle
//! laisse passer la souris partout, sauf sur Capri elle-même. Le curseur est lu auprès du système
//! 60 fois par seconde (la fenêtre ne reçoit pas les mouvements quand elle se laisse traverser),
//! mais on ne redessine que quand sa pose ou son bandeau change : environ 12 fois par seconde au
//! repos.
//!
//! `capri hook install | uninstall | status` branche Claude Code sur Capri,
//! `capri chat use | status | test | off` lui choisit un cerveau et
//! `capri voice on | off | status | list | use | test` lui donne une voix, sans lancer la fenêtre.

mod assets;
mod banner_view;
mod brain_setup;
mod chat;
mod claude;
mod ear_setup;
mod hook_setup;
#[cfg(any(target_os = "macos", windows))]
mod menu;
mod voice_setup;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use banner_view::BannerView;
use capri_core::{AlphaMask, Button, Capri, Pose, Press, Rect, Vec2, frame_at, interpret_press, place_input};
use capri_platform::{Overlay, OverlayEvent};
use capri_render::{Renderer, SpriteDraw, SpriteImage};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

/// Sa hauteur à l'écran, en points (pixels logiques).
const HEIGHT_POINTS: f64 = 300.0;
/// Lecture du curseur et pas de simulation.
const TICK: Duration = Duration::from_micros(16_667);
/// Amplitude du « boil », en pixels du sprite d'origine.
const BOIL: f32 = 1.0;
/// Écart entre ses pieds et le champ de saisie, en points.
const INPUT_GAP_POINTS: f32 = 10.0;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {}
        Some("hook") => std::process::exit(hook_setup::run_cli(&args[1..])),
        Some("chat") => std::process::exit(brain_setup::run_cli(&args[1..])),
        Some("voice") => std::process::exit(voice_setup::run_cli(&args[1..])),
        Some("ear") => std::process::exit(ear_setup::run_cli(&args[1..])),
        Some(_) => {
            eprintln!(
                "usage : capri                  lancer Capri\n\
                 \x20       capri hook <action>    brancher Claude Code (voir capri hook)\n\
                 \x20       capri chat <action>    lui choisir un cerveau (voir capri chat)\n\
                 \x20       capri voice <action>   lui donner une voix (voir capri voice)\n\
                 \x20       capri ear status       si elle peut t'écouter au micro"
            );
            std::process::exit(2);
        }
    }

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
    hotkeys: Option<Hotkeys>,
    #[cfg(any(target_os = "macos", windows))]
    menus: Option<menu::Menus>,
    fatal: Option<anyhow::Error>,
}

/// Les raccourcis globaux : ils répondent depuis n'importe quelle app.
struct Hotkeys {
    /// À garder en vie : les raccourcis disparaissent avec lui.
    _manager: GlobalHotKeyManager,
    /// ⌃⌥C : la cacher ou la faire revenir.
    toggle: Option<u32>,
    /// ⌃⌥P : lui parler.
    talk: Option<u32>,
    /// ⌃⌥M, maintenu : lui parler au micro.
    listen: Option<u32>,
}

struct Running {
    overlay: Overlay,
    renderer: Renderer,
    capri: Capri,
    mask: AlphaMask,
    start: Instant,
    last_step: Instant,
    cursor: Option<Vec2>,
    click_through: bool,
    banner: BannerView,
    /// Image d'animation courante.
    frame: u64,
    /// Ce qui est à l'écran : sa pose et, s'il est là, son bandeau.
    drawn: Option<(Pose, Option<SpriteDraw>)>,
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
        // Là où l'overlay est une fenêtre winit (Windows, Linux), ses événements passent par ici.
        if let Some(r) = &self.running {
            r.overlay.on_window_event(&event);
        }
        self.handle_overlay(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_menus(event_loop);
        self.handle_hotkey();
        self.handle_overlay(event_loop);
        if let Some(r) = self.running.as_mut() {
            if self.hidden {
                r.banner.listen_only();
            } else {
                r.tick();
            }
        }
        // sur macOS, le redessin que `tick` vient de demander se fait ici, sans attendre un tour
        self.handle_overlay(event_loop);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + TICK));
    }
}

impl App {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running> {
        let sprite = self.sprite.take().context("démarrage déjà fait")?;
        let overlay = Overlay::new(event_loop)?;

        let (width, height) = overlay.size();
        let renderer = Renderer::new(
            overlay.surface_handle(),
            width,
            height,
            SpriteImage { width: sprite.width, height: sprite.height, rgba: &sprite.rgba },
        )?;
        if !renderer.is_transparent() {
            log::warn!("la fenêtre ne peut pas être transparente sur cette machine");
        }

        let bounds = Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32));
        let body = display_size(sprite.mask.size(), overlay.scale_factor());
        // en bas à droite, au-dessus du Dock ou de la barre des tâches
        let start_at = Vec2::new(bounds.size.x - body.x * 1.8, bounds.size.y - body.y * 1.3);
        let capri = Capri::new(body, bounds, start_at);

        overlay.set_click_through(true);
        overlay.set_shown(true);
        overlay.request_redraw();

        self.hotkeys = register_hotkeys();
        #[cfg(any(target_os = "macos", windows))]
        {
            self.menus = Some(menu::Menus::new());
            self.refresh_hook_menu();
            self.refresh_voice_menu();
        }
        if !capri_platform::INTERACTIVE {
            log::warn!("système pas encore pris en charge : Capri s'affiche mais ne peut pas être attrapée");
        }

        let now = Instant::now();
        Ok(Running {
            overlay,
            renderer,
            capri,
            mask: sprite.mask,
            start: now,
            last_step: now,
            cursor: None,
            click_through: true,
            banner: BannerView::new(),
            frame: 0,
            drawn: None,
        })
    }

    /// Traite ce que l'overlay a rapporté : clics, changements d'écran, redessin.
    fn handle_overlay(&mut self, event_loop: &ActiveEventLoop) {
        let hidden = self.hidden;
        let Some(r) = self.running.as_mut() else { return };
        for event in r.overlay.drain_events() {
            match event {
                OverlayEvent::CloseRequested => event_loop.exit(),
                OverlayEvent::Resized { width, height } => {
                    log::debug!("overlay : {width}×{height} px");
                    r.renderer.resize(width, height);
                    r.capri.set_bounds(Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32)));
                    r.overlay.request_redraw();
                }
                OverlayEvent::ScaleFactorChanged(scale) => {
                    log::debug!("overlay : densité {scale}");
                    r.capri.set_size(display_size(r.mask.size(), scale));
                    r.banner.rescaled();
                }
                OverlayEvent::Pressed { button, ctrl } => {
                    // le curseur le plus frais, même si aucun mouvement n'est arrivé depuis
                    r.read_cursor();
                    let on_her = r.cursor.filter(|&c| r.is_over(c));
                    let press =
                        interpret_press(core_button(button), ctrl, capri_platform::CTRL_CLICK_IS_SECONDARY);
                    log::debug!(
                        "appui {button:?} (Ctrl : {ctrl}, sur elle : {}) → {press:?}",
                        on_her.is_some()
                    );
                    match (press, on_her) {
                        (Press::Grab, Some(c)) => r.capri.grab(c, r.seconds()),
                        (Press::Menu, Some(_)) =>
                        {
                            #[cfg(any(target_os = "macos", windows))]
                            if let Some(menus) = &self.menus {
                                menus.show_context(&r.overlay);
                            }
                        }
                        _ => {}
                    }
                }
                OverlayEvent::Released { button } => {
                    if button == MouseButton::Left {
                        r.read_cursor();
                        r.capri.release(r.seconds());
                    }
                }
                OverlayEvent::RedrawRequested => {
                    if !hidden {
                        r.draw();
                    }
                }
                OverlayEvent::InputSubmitted(message) => r.banner.say(&message),
                OverlayEvent::InputCancelled => log::debug!("saisie abandonnée"),
            }
        }
    }

    /// Ouvre le champ de saisie sous ses pieds pour lui parler, ou le referme s'il est ouvert.
    fn talk(&mut self) {
        if self.hidden {
            self.toggle_hidden(); // on ne parle pas à quelqu'un qu'on ne voit pas
        }
        let Some(r) = self.running.as_mut() else { return };
        if r.overlay.input_is_open() {
            return r.overlay.hide_input();
        }
        let (width, height) = r.overlay.size();
        let bounds = Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32));
        let standing = Rect::new(r.capri.position(), r.capri.size());
        let gap = INPUT_GAP_POINTS * r.overlay.scale_factor() as f32;
        let opened = r.overlay.show_input("Dis quelque chose à Capri…", |w, h| {
            let at = place_input(standing, Vec2::new(w as f32, h as f32), bounds, gap);
            (f64::from(at.x), f64::from(at.y))
        });
        if !opened {
            log::warn!("pas encore de champ de saisie sur ce système");
            r.banner.notify("Je ne sais pas encore t'écouter sur ce système", capri_claude::Tone::Failed);
        }
    }

    fn handle_menus(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(any(target_os = "macos", windows))]
        {
            let actions = self.menus.as_ref().map(menu::Menus::pending_actions).unwrap_or_default();
            for action in actions {
                match action {
                    menu::Action::Talk => self.talk(),
                    menu::Action::Listen => {
                        if self.hidden {
                            self.toggle_hidden();
                        }
                        self.with_banner(BannerView::listen_toggle);
                    }
                    menu::Action::Hush => self.with_banner(BannerView::hush),
                    menu::Action::Forget => self.with_banner(BannerView::forget),
                    menu::Action::ToggleVoice => self.toggle_voice(),
                    menu::Action::ToggleVisible => self.toggle_hidden(),
                    menu::Action::ToggleClaudeHook => self.toggle_claude_hook(),
                    menu::Action::Quit => event_loop.exit(),
                }
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        let _ = event_loop;
    }

    /// Agit sur ce qui s'affiche au-dessus d'elle, si elle est lancée.
    #[cfg(any(target_os = "macos", windows))]
    fn with_banner(&mut self, act: impl FnOnce(&mut BannerView)) {
        if let Some(r) = self.running.as_mut() {
            act(&mut r.banner);
        }
    }

    /// Lui donne la parole à voix haute, ou la lui retire.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_voice(&mut self) {
        let done = (|| -> Result<bool> {
            let directory = brain_setup::config_dir()?;
            let settings = voice_setup::load(&directory)?;
            let enabled = !settings.enabled;
            if enabled && capri_platform::Voice::new().is_none() {
                anyhow::bail!("pas encore de voix sur ce système");
            }
            voice_setup::save(&directory, &capri_chat::voice::VoiceSettings { enabled, ..settings })?;
            Ok(enabled)
        })();
        let (text, tone) = match &done {
            Ok(true) => ("Je te répondrai à voix haute", capri_claude::Tone::Done),
            Ok(false) => ("Je ne réponds plus que par écrit", capri_claude::Tone::Done),
            Err(err) => {
                log::error!("voix : {err:#}");
                ("Je n'ai pas pu changer ma voix, détails dans le journal", capri_claude::Tone::Failed)
            }
        };
        self.with_banner(|banner| {
            if matches!(done, Ok(false)) {
                banner.silence();
            }
            banner.notify(text, tone);
        });
        self.refresh_voice_menu();
    }

    /// Accorde le menu à ce que disent les réglages de la voix.
    #[cfg(any(target_os = "macos", windows))]
    fn refresh_voice_menu(&self) {
        let enabled = brain_setup::config_dir()
            .and_then(|directory| voice_setup::load(&directory))
            .inspect_err(|err| log::warn!("réglages de la voix illisibles : {err:#}"))
            .is_ok_and(|settings| settings.enabled);
        if let Some(menus) = &self.menus {
            menus.set_voice_enabled(enabled);
        }
    }

    /// Branche Claude Code sur Capri, ou le débranche s'il l'est déjà.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_claude_hook(&mut self) {
        let done = (|| -> Result<bool> {
            let file = hook_setup::settings_path()?;
            if hook_setup::status(&file)?.is_installed() {
                let outcome = hook_setup::uninstall(&file, false)?;
                log::info!("Claude Code débranché ({})", outcome.settings.display());
                Ok(false)
            } else {
                let outcome = hook_setup::install(&file, &hook_setup::hook_binary()?, false)?;
                log::info!(
                    "Claude Code branché ({}), sauvegarde : {:?}",
                    outcome.settings.display(),
                    outcome.backup
                );
                Ok(true)
            }
        })();
        let (text, tone) = match &done {
            Ok(true) => ("Claude Code est branché", capri_claude::Tone::Done),
            Ok(false) => ("Claude Code est débranché", capri_claude::Tone::Done),
            Err(err) => {
                log::error!("branchement de Claude Code : {err:#}");
                ("Branchement impossible, détails dans le journal", capri_claude::Tone::Failed)
            }
        };
        if let Some(r) = self.running.as_mut() {
            r.banner.notify(text, tone);
        }
        self.refresh_hook_menu();
    }

    /// Accorde le menu à ce que dit le fichier de réglages de Claude Code.
    #[cfg(any(target_os = "macos", windows))]
    fn refresh_hook_menu(&self) {
        let installed = hook_setup::settings_path()
            .and_then(|file| hook_setup::status(&file))
            .inspect_err(|err| log::warn!("réglages de Claude Code illisibles : {err:#}"))
            .map(|status| status.is_installed())
            .ok();
        if let Some(menus) = &self.menus {
            menus.set_hook_installed(installed);
        }
    }

    fn handle_hotkey(&mut self) {
        let Some(Hotkeys { toggle, talk, listen, .. }) = self.hotkeys else { return };
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if Some(event.id) == listen {
                // le seul raccourci qui se maintient : on parle tant qu'il est enfoncé
                self.listen(event.state == HotKeyState::Pressed);
                continue;
            }
            if event.state != HotKeyState::Pressed {
                continue;
            }
            if Some(event.id) == toggle {
                self.toggle_hidden();
            } else if Some(event.id) == talk {
                self.talk();
            }
        }
    }

    /// La touche pour lui parler au micro vient d'être enfoncée (`true`) ou relâchée.
    fn listen(&mut self, pressed: bool) {
        if pressed && self.hidden {
            self.toggle_hidden(); // on ne parle pas à quelqu'un qu'on ne voit pas
        }
        let Some(r) = self.running.as_mut() else { return };
        if pressed {
            r.overlay.hide_input(); // on ne tape pas et on ne parle pas en même temps
            r.banner.listen_start();
        } else {
            r.banner.listen_stop();
        }
    }

    fn toggle_hidden(&mut self) {
        let Some(r) = self.running.as_mut() else { return };
        self.hidden = !self.hidden;
        if self.hidden {
            r.overlay.hide_input();
        }
        r.overlay.set_click_through(true);
        r.click_through = true;
        r.overlay.set_shown(!self.hidden);
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
        if let Some((x, y)) = self.overlay.cursor() {
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
        self.frame = frame_at(now - self.start);
        self.capri.step(dt, self.frame);

        // La souris traverse la fenêtre partout, sauf sur elle ou tant qu'on la tient.
        let over = self.cursor.is_some_and(|c| self.is_over(c));
        let through = !capri_platform::INTERACTIVE || !(over || self.capri.is_held());
        if through != self.click_through {
            self.overlay.set_click_through(through);
            self.click_through = through;
        }

        self.banner.update(self.frame, self.overlay.scale_factor(), &mut self.renderer);
        if self.drawn != Some(self.scene()) {
            self.overlay.request_redraw();
        }
    }

    /// Ce qu'il faut dessiner maintenant : sa pose, et son bandeau s'il y en a un.
    fn scene(&self) -> (Pose, Option<SpriteDraw>) {
        let (width, height) = self.overlay.size();
        let bounds = Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32));
        let standing = Rect::new(self.capri.position(), self.capri.size());
        let banner = self.banner.draw(self.frame, standing, bounds, self.overlay.scale_factor());
        (self.capri.pose(), banner)
    }

    fn draw(&mut self) {
        let (pose, banner) = self.scene();
        let unit = pose.rect.size.y / 420.0;
        let draw = SpriteDraw {
            min: [pose.rect.min.x, pose.rect.min.y],
            size: [pose.rect.size.x, pose.rect.size.y],
            ghost: [pose.ghost.x, pose.ghost.y],
            boil_seed: pose.boil_seed,
            boil: BOIL * unit,
            opacity: 1.0,
        };
        let overlay = &self.overlay;
        self.renderer.render(&draw, banner.as_ref(), || overlay.pre_present_notify());
        self.drawn = Some((pose, banner));
    }
}

/// Le bouton de winit, dans le vocabulaire de `capri-core`.
fn core_button(button: MouseButton) -> Button {
    match button {
        MouseButton::Left => Button::Primary,
        MouseButton::Right => Button::Secondary,
        _ => Button::Other,
    }
}

/// Sa taille à l'écran en pixels physiques, en gardant les proportions du sprite.
fn display_size((width, height): (u32, u32), scale_factor: f64) -> Vec2 {
    let h = (HEIGHT_POINTS * scale_factor) as f32;
    Vec2::new(h * width as f32 / height as f32, h)
}

/// ⌃⌥C (Ctrl+Alt+C) la cache ou la fait revenir, ⌃⌥P ouvre le champ pour lui parler, depuis
/// n'importe quelle app. Un raccourci déjà pris par une autre app n'empêche pas l'autre.
fn register_hotkeys() -> Option<Hotkeys> {
    let manager = GlobalHotKeyManager::new()
        .inspect_err(|err| log::warn!("raccourcis globaux indisponibles : {err}"))
        .ok()?;
    let register = |code: Code, label: &str| {
        let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), code);
        manager
            .register(hotkey)
            .inspect_err(|err| log::warn!("raccourci {label} indisponible : {err}"))
            .ok()
            .map(|()| hotkey.id())
    };
    let (toggle, talk) = (register(Code::KeyC, "⌃⌥C"), register(Code::KeyP, "⌃⌥P"));
    let listen = register(Code::KeyM, ear_setup::SHORTCUT);
    Some(Hotkeys { _manager: manager, toggle, talk, listen })
}
