//! Capri : elle flotte sur ton bureau, on peut l'attraper, la poser et la lancer (jalon 1), un
//! bandeau au-dessus d'elle dit ce que fait Claude Code (jalon 2), elle s'assoit sur tes fenêtres
//! (jalon 3), et on peut lui parler : elle répond dans une bulle, avec le modèle qu'on lui a donné,
//! local ou chez un fournisseur (jalon 4).
//!
//! Une seule fenêtre transparente, l'overlay de `capri-platform`, couvre l'écran principal. Elle
//! laisse passer la souris partout, sauf sur Capri elle-même. Le curseur est lu auprès du système
//! 60 fois par seconde (la fenêtre ne reçoit pas les mouvements quand elle se laisse traverser),
//! à chaque rafraîchissement de l'écran quand on la tient, qu'elle vole ou que sa fenêtre bouge
//! (`capri_core::pace`), mais on ne redessine que quand sa pose, sa position ou son bandeau
//! change : environ 12 fois par seconde au repos.
//!
//! `capri hook install | uninstall | status | mirror … | style …` branche Claude Code sur Capri,
//! règle le miroir et installe son style de sortie,
//! `capri chat use | key | models | status | test | off` lui choisit un cerveau (et range la clé
//! d'un fournisseur dans le trousseau du système),
//! `capri voice on | off | status | list | use | effect | intensity | pitch | rate | test` lui
//! donne une voix et sa façon de parler, et `capri ear status` dit si elle peut écouter, sans
//! lancer la fenêtre.
//! Sur macOS, « Réglages… » dans son menu ouvre une fenêtre qui fait la même chose sans terminal
//! (`settings_view`), et « Se lancer à l'ouverture de session » l'inscrit auprès du système
//! (`login_setup`) ; cochée, elle lance aussi Ollama à chaque lancement s'il est son cerveau sur
//! cette machine et que rien n'y répond (`ollama_view`).

mod assets;
mod banner_view;
mod brain_setup;
mod chat;
mod claude;
mod ear_setup;
#[cfg(test)]
mod fake_server;
mod figure;
mod hook_setup;
mod keys;
mod login_setup;
#[cfg(any(target_os = "macos", windows))]
mod menu;
#[cfg(debug_assertions)]
mod music_rehearsal;
mod music_setup;
mod music_view;
mod ollama_view;
#[cfg(debug_assertions)]
mod perch_rehearsal;
mod perch_setup;
mod perch_view;
#[cfg(test)]
mod portraits;
#[cfg(any(target_os = "macos", windows))]
mod settings_view;
mod size_setup;
mod style_setup;
#[cfg(debug_assertions)]
mod voice_rehearsal;
mod voice_setup;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use banner_view::BannerView;
use capri_core::look::Picture;
use capri_core::{
    Button, Capri, Press, Rect, Scale, Stance, Vec2, frame_at, interpret_press, pace, place_input,
};
use capri_platform::{Overlay, OverlayEvent};
use capri_render::{Renderer, SpriteDraw};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

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
        // en version de débogage : la vraie liste des fenêtres, sans rien afficher
        #[cfg(debug_assertions)]
        Some("perch") => std::process::exit(perch_rehearsal::run_cli(&args[1..])),
        // en version de débogage : une annonce d'essai d'un lecteur, guettée comme Capri le fait
        #[cfg(debug_assertions)]
        Some("music") => std::process::exit(music_rehearsal::run_cli(&args[1..])),
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

    init_log();
    let loaded = Instant::now();
    let sprite = assets::load_sprite()?;
    // ses planches debout et assise ; sans elles, son dessin d'avant
    let (stand, perch) = (assets::stand::load(), assets::perch::load());
    log::info!("images lues en {} ms", loaded.elapsed().as_millis());

    let mut builder = EventLoop::builder();
    capri_platform::configure_event_loop(&mut builder);
    let event_loop = builder.build().context("création de la boucle d'événements")?;

    let mut app = App { images: Some(Images { sprite, stand, perch }), ..App::default() };
    event_loop.run_app(&mut app)?;
    app.fatal.map_or(Ok(()), Err)
}

/// Le journal, réglé par `RUST_LOG` (par défaut : `info`), sans jamais ce qui passe sur le réseau.
fn init_log() {
    let logger = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).build();
    log::set_max_level(logger.filter());
    if let Err(err) = log::set_boxed_logger(Box::new(NoWireLog(logger))) {
        eprintln!("capri : pas de journal ({err})");
    }
}

/// Le journal de l'app, privé de ce qui passe sur le réseau.
///
/// Au niveau `trace`, ureq écrit les octets échangés tels quels (« NOT REDACTED », dit sa
/// documentation) : la conversation, et l'en-tête qui porte la clé d'un fournisseur. Ces lignes ne
/// sont jamais écrites, quel que soit `RUST_LOG`. Ce que ureq écrit au niveau `debug` (adresse,
/// état de la connexion, en-têtes d'une liste sûre) reste visible.
struct NoWireLog(env_logger::Logger);

impl NoWireLog {
    fn allows(metadata: &log::Metadata<'_>) -> bool {
        let from_ureq = metadata.target() == "ureq"
            || metadata.target().starts_with("ureq::")
            || metadata.target().starts_with("ureq_proto");
        !(from_ureq && metadata.level() == log::Level::Trace)
    }
}

impl log::Log for NoWireLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        Self::allows(metadata) && self.0.enabled(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        if Self::allows(record.metadata()) {
            self.0.log(record);
        }
    }

    fn flush(&self) {
        self.0.flush();
    }
}

/// Ses images lues au lancement, en attendant d'être confiées à la carte graphique.
struct Images {
    sprite: assets::Sprite,
    stand: Option<assets::stand::StandSheets>,
    perch: Option<assets::perch::PerchSheets>,
}

#[derive(Default)]
struct App {
    images: Option<Images>,
    running: Option<Running>,
    hidden: bool,
    hotkeys: Option<Hotkeys>,
    #[cfg(any(target_os = "macos", windows))]
    menus: Option<menu::Menus>,
    /// La fenêtre de réglages, tant qu'elle est ouverte.
    #[cfg(any(target_os = "macos", windows))]
    settings: Option<settings_view::SettingsView>,
    /// Une répétition de la fenêtre de réglages, pour les essais (version de débogage).
    #[cfg(all(debug_assertions, any(target_os = "macos", windows)))]
    rehearsal: Option<Rehearsal>,
    /// La répétition du suivi d'une fenêtre (`CAPRI_PERCH_REHEARSAL`, version de débogage).
    #[cfg(debug_assertions)]
    follow: Option<perch_rehearsal::Follow>,
    /// La répétition des annonces de musique (`CAPRI_MUSIC_REHEARSAL`, version de débogage).
    #[cfg(debug_assertions)]
    music_rehearsal: Option<music_rehearsal::InApp>,
    /// Veiller à ce qu'Ollama tourne, au lancement, case « Se lancer à l'ouverture de session »
    /// cochée : ce qu'elle en dira, une fois.
    ollama: Option<ollama_view::OllamaWatch>,
    fatal: Option<anyhow::Error>,
}

/// En version de débogage, `CAPRI_SETTINGS_REHEARSAL=1` ouvre la fenêtre de réglages au
/// lancement, y choisit chaque fournisseur l'un après l'autre comme le ferait l'utilisateur, puis
/// quitte : de quoi vérifier, sans la toucher, que la fenêtre s'ouvre et se refait sans planter.
/// N'existe pas dans la version livrée.
#[cfg(all(debug_assertions, any(target_os = "macos", windows)))]
struct Rehearsal {
    next: Instant,
    step: usize,
}

/// Les raccourcis globaux : ils répondent depuis n'importe quelle app.
struct Hotkeys {
    /// À garder en vie : les raccourcis disparaissent avec lui.
    _manager: GlobalHotKeyManager,
    /// ⌃⌥C : la cacher ou la faire revenir.
    toggle: Option<u32>,
    /// ⌃⌥P : lui parler.
    talk: Option<u32>,
    /// ⌃⌥D, maintenu : lui parler au micro.
    listen: Option<u32>,
}

struct Running {
    overlay: Overlay,
    renderer: Renderer,
    capri: Capri,
    /// Ses dessins : lequel montrer, où, et ce qui l'attrape.
    figure: figure::Figure,
    start: Instant,
    last_step: Instant,
    cursor: Option<Vec2>,
    click_through: bool,
    banner: BannerView,
    /// S'asseoir sur les fenêtres : quand lire leur liste, et ce qu'elle en fait.
    perch: perch_view::PerchView,
    /// Se balancer sur la musique, assise : les annonces des lecteurs, et ce qu'elle en conclut.
    music: music_view::MusicView,
    /// Image d'animation courante.
    frame: u64,
    /// Ce qui est à l'écran : son dessin et, s'il est là, son bandeau.
    drawn: Option<(Picture, Option<SpriteDraw>)>,
    /// La fréquence de son écran, telle que le système la dit (relue quand l'écran change).
    refresh: Option<f64>,
    /// Le réveil des pas pressés, là où l'écran ne réveille pas la boucle lui-même.
    clock: pace::Clock,
    /// C'est l'écran qui cadence la boucle ; à défaut de ses nouvelles, un pas est fait quand
    /// même passé `fallback`.
    by_screen: bool,
    fallback: Instant,
    /// Sa taille, choisie par l'utilisateur (`capri_core::scale`).
    scale: Scale,
    /// Changée à la molette et pas encore retenue : depuis quand on n'y a plus touché.
    unsaved_scale: Option<Instant>,
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
        #[cfg(any(target_os = "macos", windows))]
        self.handle_settings(event_loop);
        self.handle_hotkey();
        self.handle_ollama();
        self.handle_overlay(event_loop);
        if let Some(r) = self.running.as_mut() {
            if self.hidden {
                r.banner.listen_only();
                r.music.tick(Instant::now(), false); // cachée, elle ne se balance pas
            } else if r.step_due() {
                r.tick();
            }
            r.remember_scale_once_still();
        }
        // sur macOS, le redessin que `tick` vient de demander se fait ici, sans attendre un tour
        self.handle_overlay(event_loop);
        #[cfg(debug_assertions)]
        if let (Some(follow), Some(r)) = (self.follow.as_mut(), self.running.as_mut())
            && follow.step(r)
        {
            event_loop.exit();
        }
        #[cfg(debug_assertions)]
        if let (Some(rehearsal), Some(r)) = (self.music_rehearsal.as_mut(), self.running.as_ref())
            && rehearsal.step(r)
        {
            event_loop.exit();
        }
        let hidden = self.hidden;
        let wake = self.running.as_mut().map_or_else(|| Instant::now() + pace::REST, |r| r.next_wake(hidden));
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}

impl App {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running> {
        let Images { sprite, stand, perch } = self.images.take().context("démarrage déjà fait")?;
        let overlay = Overlay::new(event_loop)?;

        let (width, height) = overlay.size();
        let mut renderer = Renderer::new(overlay.surface_handle(), width, height)?;
        if !renderer.is_transparent() {
            log::warn!("la fenêtre ne peut pas être transparente sur cette machine");
        }

        let bounds = Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32));
        let scale = brain_setup::config_dir().and_then(|directory| size_setup::load(&directory));
        let scale = scale.unwrap_or_else(|err| {
            log::warn!("elle prend sa taille d'origine : {err:#}");
            Scale::ORIGINAL
        });
        log::info!("taille : {} %", scale.percent());
        let standing = standing_height(overlay.scale_factor(), scale);
        // ses images confiées à la carte graphique, puis lâchées
        let figure = figure::Figure::new(sprite, stand, perch, standing, |image| renderer.add_image(image))?;
        let look = &figure.look;
        log::info!(
            "dessins : debout {}, sur les fenêtres {} ; mémoire graphique : {:.1} Mo",
            if look.has_stand() { "ses planches" } else { "son dessin d'avant" },
            if look.has_perch() { "assise" } else { "debout" },
            renderer.image_bytes() as f64 / 1_000_000.0
        );
        let body = figure.look.standing_size();
        // en bas à droite, au-dessus du Dock ou de la barre des tâches
        let start_at = Vec2::new(bounds.size.x - body.x * 1.8, bounds.size.y - body.y * 1.3);
        let mut capri = Capri::new(body, bounds, start_at);
        capri.set_feet(figure.look.standing_feet());
        capri.set_leave_frames(figure.look.leave_frames());
        let perch_enabled = brain_setup::config_dir().and_then(|directory| perch_setup::load(&directory));
        let perch_enabled = perch_enabled.unwrap_or_else(|err| {
            log::warn!("réglage « S'asseoir sur les fenêtres » illisible, elle s'y assoit : {err:#}");
            true
        });
        let music_enabled = brain_setup::config_dir().and_then(|directory| music_setup::load(&directory));
        let music_enabled = music_enabled.unwrap_or_else(|err| {
            log::warn!("réglage « Se balancer sur la musique » illisible, elle s'y balance : {err:#}");
            true
        });

        overlay.set_click_through(true);
        overlay.set_shown(true);
        overlay.request_redraw();

        self.hotkeys = register_hotkeys();
        // l'inscription à l'ouverture de session, lue dans le système (et réparée si l'app a changé
        // de place) ; cochée, Ollama est veillé sur un thread à part
        let registered = capri_platform::login::user_agents_dir()
            .inspect_err(|err| log::debug!("ouverture de session : {err}"))
            .is_ok_and(|directory| login_setup::at_launch(&directory, login_setup::this_app().as_deref()));
        self.ollama = ollama_view::OllamaWatch::start(registered);
        #[cfg(any(target_os = "macos", windows))]
        {
            self.menus = Some(menu::Menus::new());
            self.refresh_hook_menu();
            self.refresh_voice_menu();
            if let Some(menus) = &self.menus {
                menus.set_perch_enabled(perch_enabled);
                menus.set_music_enabled(music_enabled);
                menus.set_login_registered(registered);
                menus.set_size(scale);
            }
        }
        #[cfg(all(debug_assertions, any(target_os = "macos", windows)))]
        if std::env::var_os("CAPRI_SETTINGS_REHEARSAL").is_some() {
            log::info!("répétition de la fenêtre de réglages");
            self.open_settings();
            self.rehearsal = Some(Rehearsal { next: Instant::now() + Duration::from_millis(800), step: 0 });
        }
        if !capri_platform::INTERACTIVE {
            log::warn!("système pas encore pris en charge : Capri s'affiche mais ne peut pas être attrapée");
        }
        #[cfg(debug_assertions)]
        {
            self.follow = perch_rehearsal::Follow::from_env();
            self.music_rehearsal = music_rehearsal::InApp::from_env();
        }
        // abonnée aux annonces des lecteurs dès le lancement, si c'est permis (sur ce thread)
        #[cfg(debug_assertions)]
        let music = match &self.music_rehearsal {
            Some(rehearsal) => rehearsal.view(music_enabled),
            None => music_view::MusicView::new(music_enabled),
        };
        #[cfg(not(debug_assertions))]
        let music = music_view::MusicView::new(music_enabled);
        let refresh = overlay.refresh_rate();
        log::info!("écran : {refresh:?} Hz");

        let now = Instant::now();
        Ok(Running {
            overlay,
            renderer,
            capri,
            figure,
            start: now,
            last_step: now,
            cursor: None,
            click_through: true,
            banner: BannerView::new(),
            perch: perch_view::PerchView::new(perch_enabled),
            music,
            frame: 0,
            drawn: None,
            refresh,
            clock: pace::Clock::new(),
            by_screen: false,
            fallback: now,
            scale,
            unsaved_scale: None,
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
                    r.refresh = r.overlay.refresh_rate();
                }
                OverlayEvent::ScaleFactorChanged(scale) => {
                    log::debug!("overlay : densité {scale}");
                    r.refresh = r.overlay.refresh_rate();
                    r.apply_size(scale);
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
                        // assise, elle se lève sous le curseur, tenue par le même endroit d'elle
                        (Press::Grab, Some(c)) => match r.figure.look.grab_origin(&r.capri, c) {
                            Some(origin) => r.capri.grab_at(c, origin, r.seconds()),
                            None => r.capri.grab(c, r.seconds()),
                        },
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
                // pas pendant qu'on la tient : elle sauterait sous le curseur
                OverlayEvent::Zoom { steps } if !r.capri.is_held() => {
                    let scale = r.scale.zoomed(steps);
                    if r.resize(scale) {
                        r.unsaved_scale = Some(Instant::now());
                        #[cfg(any(target_os = "macos", windows))]
                        if let Some(menus) = &self.menus {
                            menus.set_size(scale);
                        }
                    }
                }
                OverlayEvent::Zoom { .. } => {}
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
        let shown = r.figure.look.shown(&r.capri);
        let gap = INPUT_GAP_POINTS * r.overlay.scale_factor() as f32;
        let opened = r.overlay.show_input("Dis quelque chose à Capri…", |w, h| {
            let at = place_input(shown, Vec2::new(w as f32, h as f32), bounds, gap);
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
                    menu::Action::TogglePerch => self.toggle_perch(),
                    menu::Action::ToggleMusic => self.toggle_music(),
                    menu::Action::Larger => self.choose_size(Scale::larger),
                    menu::Action::Smaller => self.choose_size(Scale::smaller),
                    menu::Action::OriginalSize => self.choose_size(|_| Scale::ORIGINAL),
                    menu::Action::ToggleClaudeHook => self.toggle_claude_hook(),
                    menu::Action::ToggleLogin => self.toggle_login(),
                    menu::Action::Settings => self.open_settings(),
                    menu::Action::Quit => event_loop.exit(),
                }
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        let _ = event_loop;
    }

    /// Ouvre la fenêtre de réglages, ou la ramène devant si elle l'est déjà : une seule à la fois.
    #[cfg(any(target_os = "macos", windows))]
    fn open_settings(&mut self) {
        if let Some(view) = self.settings.as_ref().filter(|view| view.is_open()) {
            return view.show();
        }
        match settings_view::SettingsView::open() {
            Ok(view) => self.settings = Some(view),
            Err(err) => {
                log::warn!("réglages : {err}");
                self.with_banner(|banner| {
                    banner.notify(
                        "Pas encore de réglages ici : capri chat et capri voice",
                        capri_claude::Tone::Failed,
                    )
                });
            }
        }
    }

    /// Ce qui s'est passé dans la fenêtre de réglages ; elle disparaît une fois fermée.
    #[cfg(any(target_os = "macos", windows))]
    fn handle_settings(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(debug_assertions)]
        self.rehearse(event_loop);
        #[cfg(not(debug_assertions))]
        let _ = event_loop;
        let Some(view) = self.settings.as_mut() else { return };
        let changes = view.pump();
        if !view.is_open() {
            self.settings = None;
        }
        for change in changes {
            match change {
                settings_view::Change::Voice { enabled } => {
                    if enabled {
                        // effet, hauteur, débit ou voix : dès sa phrase suivante
                        self.with_banner(BannerView::refresh_voice);
                    } else {
                        self.with_banner(BannerView::silence);
                    }
                    self.refresh_voice_menu();
                }
                settings_view::Change::Hook => self.refresh_hook_menu(),
            }
        }
    }

    /// Un pas de la répétition des réglages, s'il y en a une : chaque fournisseur, du dernier au
    /// premier, puis une clé factice collée dans le champ secret d'Anthropic, le dernier
    /// fournisseur venu, rangée (dans le trousseau que désigne `CAPRI_KEYCHAIN_SERVICE`) et
    /// retirée. Personne n'est contacté : Anthropic est sur Internet, et la fenêtre ne lui demande
    /// rien d'elle-même. Le journal dit ce que la
    /// fenêtre affiche, jamais la clé. Puis, seulement si `CLAUDE_CONFIG_DIR` et
    /// `CAPRI_CONFIG_DIR` désignent des dossiers d'essai, la rubrique « Claude Code » : le style
    /// installé et retiré, Claude Code branché et débranché, le miroir activé et désactivé, par
    /// les boutons de la vraie fenêtre. Enfin, seulement si `CAPRI_CONFIG_DIR` désigne un dossier
    /// d'essai, la rubrique « Voix » : un effet, son intensité, la hauteur et le débit choisis dans
    /// la vraie fenêtre, puis « Naturelle » de nouveau ; « Écouter » n'est jamais pressé.
    #[cfg(all(debug_assertions, any(target_os = "macos", windows)))]
    fn rehearse(&mut self, event_loop: &ActiveEventLoop) {
        use capri_chat::settings::{brain, claude, voice};
        /// L'étape (après les fournisseurs) où commence la rubrique « Voix ».
        const VOICE_STEPS: usize = 11;
        use capri_core::form::FormEvent;
        let Some(rehearsal) = self.rehearsal.as_mut() else { return };
        if Instant::now() < rehearsal.next {
            return;
        }
        let Some(view) = self.settings.as_mut() else {
            log::error!("répétition : la fenêtre de réglages ne s'est pas ouverte");
            return event_loop.exit();
        };
        let providers = capri_chat::Provider::ALL.len();
        let keyed = capri_chat::Provider::Anthropic;
        let keyed_index =
            1 + capri_chat::Provider::ALL.iter().position(|&provider| provider == keyed).unwrap_or_default();
        let step = rehearsal.step;
        rehearsal.step += 1;
        rehearsal.next = Instant::now() + Duration::from_millis(700);
        if step == 0 {
            log::info!("répétition : la fenêtre a le clavier : {}", view.has_keyboard());
        }
        let note =
            |view: &settings_view::SettingsView, id| view.note(id).unwrap_or_else(|| "(aucune)".to_owned());
        match step.checked_sub(providers + 1) {
            None => {
                let provider = providers - step;
                view.rehearse(provider);
                log::info!("répétition : fournisseur n° {provider} choisi, {} lignes", view.rows());
            }
            Some(0) => {
                view.rehearse(keyed_index);
                log::info!("répétition : {} choisi", keyed.name());
            }
            Some(1) => {
                log::info!("répétition : clé avant : {}", note(view, brain::KEY_STATUS));
                view.rehearse_typing_then_press(brain::KEY, "sk-test-repetition-0123456789", brain::KEY_SAVE);
            }
            Some(2) => {
                log::info!("répétition : après « Enregistrer la clé » : {}", note(view, brain::KEY_NOTE));
                log::info!("répétition : état de la clé : {}", note(view, brain::KEY_STATUS));
                view.rehearse_press(brain::KEY_FORGET);
            }
            Some(3) => {
                log::info!("répétition : après « Retirer la clé » : {}", note(view, brain::KEY_NOTE));
                log::info!("répétition : état de la clé : {}", note(view, brain::KEY_STATUS));
                log::info!("répétition : la fenêtre a le clavier : {}", view.has_keyboard());
            }
            Some(4) => {
                let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
                if !(set("CLAUDE_CONFIG_DIR") && set("CAPRI_CONFIG_DIR")) {
                    log::info!(
                        "répétition : rubrique Claude Code sautée, faute de CLAUDE_CONFIG_DIR et CAPRI_CONFIG_DIR d'essai"
                    );
                    rehearsal.step = providers + 1 + VOICE_STEPS;
                    return;
                }
                log::info!("répétition : branchement : {}", note(view, claude::HOOK));
                log::info!("répétition : style : {}", note(view, claude::STYLE));
                view.rehearse_press(claude::INSTALL_STYLE);
            }
            Some(5) => {
                log::info!(
                    "répétition : après « Installer le style Capri » : {}",
                    note(view, claude::STYLE_NOTE)
                );
                view.rehearse_press(claude::REMOVE_STYLE);
            }
            Some(6) => {
                log::info!("répétition : après « Retirer » : {}", note(view, claude::STYLE_NOTE));
                view.rehearse_press(claude::PLUG);
            }
            Some(7) => {
                log::info!("répétition : après « Brancher » : {}", note(view, claude::HOOK_NOTE));
                log::info!("répétition : branchement : {}", note(view, claude::HOOK));
                view.rehearse_press(claude::UNPLUG);
            }
            Some(8) => {
                log::info!("répétition : après « Débrancher » : {}", note(view, claude::HOOK_NOTE));
                view.rehearse_event(FormEvent::Toggled { id: claude::MIRROR.to_owned(), checked: true });
            }
            Some(9) => {
                log::info!("répétition : miroir coché : {}", note(view, claude::MIRROR_NOTE));
                view.rehearse_event(FormEvent::Toggled { id: claude::MIRROR.to_owned(), checked: false });
            }
            Some(10) => {
                log::info!("répétition : miroir décoché : {}", note(view, claude::MIRROR_NOTE));
            }
            Some(VOICE_STEPS) => {
                if !std::env::var_os("CAPRI_CONFIG_DIR").is_some_and(|value| !value.is_empty()) {
                    log::info!("répétition : rubrique Voix sautée, faute de CAPRI_CONFIG_DIR d'essai");
                    rehearsal.step = usize::MAX - 1;
                    return;
                }
                log::info!("répétition : voix, {} lignes avant", view.rows());
                view.rehearse_event(FormEvent::Chose { id: voice::EFFECT.to_owned(), index: 1 });
            }
            Some(12..=15) => {
                log::info!("répétition : voix : {}", note(view, voice::SAVED));
                let (id, index) = match step - (providers + 1) {
                    12 => (voice::INTENSITY, 2),
                    13 => (voice::PITCH, 1),
                    14 => (voice::RATE, 3),
                    _ => (voice::EFFECT, 0),
                };
                view.rehearse_event(FormEvent::Chose { id: id.to_owned(), index });
            }
            Some(16) => {
                log::info!("répétition : voix : {}", note(view, voice::SAVED));
                log::info!("répétition : voix, {} lignes après", view.rows());
            }
            Some(_) => {
                log::info!("répétition finie");
                event_loop.exit();
            }
        }
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
        if let Some(view) = self.settings.as_mut() {
            view.reload_voice();
        }
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

    /// Lui permet de s'asseoir sur les fenêtres, ou le lui interdit ; le choix est retenu dans
    /// son dossier. Si on ne peut pas le retenir, rien ne change.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_perch(&mut self) {
        let Some(r) = self.running.as_mut() else { return };
        let enabled = !r.perch.is_enabled();
        match brain_setup::config_dir().and_then(|directory| perch_setup::save(&directory, enabled)) {
            Ok(_) => {
                r.perch.set_enabled(enabled, &mut r.capri);
                let text = if enabled {
                    "Lâche-moi sur le haut d'une fenêtre : je m'y assiérai"
                } else {
                    "Je ne m'assieds plus sur tes fenêtres"
                };
                r.banner.notify(text, capri_claude::Tone::Done);
            }
            Err(err) => {
                log::error!("réglage « S'asseoir sur les fenêtres » : {err:#}");
                r.banner.notify(
                    "Je n'ai pas pu retenir ce choix, détails dans le journal",
                    capri_claude::Tone::Failed,
                );
            }
        }
        if let Some(menus) = &self.menus {
            menus.set_perch_enabled(r.perch.is_enabled());
        }
    }

    /// Lui permet de se balancer sur la musique, assise, ou le lui interdit ; le choix est retenu
    /// dans son dossier. Si on ne peut pas le retenir, rien ne change.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_music(&mut self) {
        let Some(r) = self.running.as_mut() else { return };
        let enabled = !r.music.is_enabled();
        match brain_setup::config_dir().and_then(|directory| music_setup::save(&directory, enabled)) {
            Ok(_) => {
                r.music.set_enabled(enabled);
                let text = if enabled {
                    "Assise, je me balancerai quand tu mets de la musique"
                } else {
                    "Je ne me balance plus sur ta musique"
                };
                r.banner.notify(text, capri_claude::Tone::Done);
            }
            Err(err) => {
                log::error!("réglage « Se balancer sur la musique » : {err:#}");
                r.banner.notify(
                    "Je n'ai pas pu retenir ce choix, détails dans le journal",
                    capri_claude::Tone::Failed,
                );
            }
        }
        if let Some(menus) = &self.menus {
            menus.set_music_enabled(r.music.is_enabled());
        }
    }

    /// Sa taille, choisie dans son menu : appliquée tout de suite, et retenue dans son dossier.
    #[cfg(any(target_os = "macos", windows))]
    fn choose_size(&mut self, choose: impl FnOnce(Scale) -> Scale) {
        let Some(r) = self.running.as_mut() else { return };
        let scale = choose(r.scale);
        r.resize(scale);
        r.remember_scale();
        if let Some(menus) = &self.menus {
            menus.set_size(r.scale);
        }
    }

    /// L'inscrit à l'ouverture de session, ou l'en retire. Cochée, elle veille aussi à ce
    /// qu'Ollama tourne, tout de suite, si c'est son cerveau sur cette machine.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_login(&mut self) {
        use capri_chat::startup::{self, say};
        let done = capri_platform::login::user_agents_dir()
            .map_err(login_setup::ToggleError::Login)
            .and_then(|directory| login_setup::toggle(&directory, login_setup::this_app().as_deref()));
        let (text, tone) = match &done {
            Ok(login_setup::Toggled::Registered) => {
                let brain = brain_setup::config_dir()
                    .and_then(|directory| brain_setup::load(&directory))
                    .ok()
                    .flatten()
                    .map(|setup| setup.brain);
                let local = matches!(startup::plan(true, brain.as_ref()), startup::Plan::Watch(_));
                (say::registered(local), capri_claude::Tone::Done)
            }
            Ok(login_setup::Toggled::Unregistered) => (say::UNREGISTERED, capri_claude::Tone::Done),
            Err(err) => {
                log::error!("ouverture de session : {err}");
                (login_setup::refusal(err), capri_claude::Tone::Failed)
            }
        };
        self.with_banner(|banner| banner.notify(text, tone));
        let registered = matches!(done, Ok(login_setup::Toggled::Registered));
        if registered && self.ollama.is_none() {
            self.ollama = ollama_view::OllamaWatch::start(true);
        }
        let shown = capri_platform::login::user_agents_dir()
            .and_then(|directory| capri_platform::login::status(&directory))
            .is_ok_and(|state| state.is_registered());
        if let Some(menus) = &self.menus {
            menus.set_login_registered(shown);
        }
    }

    /// Ce qu'elle a à dire d'Ollama, une fois la veille finie.
    fn handle_ollama(&mut self) {
        let Some(heard) = self.ollama.as_ref().map(ollama_view::OllamaWatch::heard) else { return };
        match heard {
            ollama_view::Heard::Nothing => {}
            ollama_view::Heard::Over => self.ollama = None,
            ollama_view::Heard::Said((text, tone)) => {
                self.ollama = None;
                if let Some(r) = self.running.as_mut() {
                    r.banner.notify(text, tone);
                }
            }
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
        if let Some(view) = self.settings.as_mut() {
            view.reload_claude();
        }
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
                // le seul dont le relâchement compte : appui bref, l'écoute reste ouverte ;
                // appui maintenu, le relâchement la termine
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
            r.banner.listen_press();
        } else {
            r.banner.listen_release();
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
            r.perch.wake(); // sa fenêtre a pu bouger pendant ce temps
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

    /// Le curseur est-il sur elle (au pixel près, d'après la pose dessinée, avec une petite
    /// marge) ?
    fn is_over(&self, cursor: Vec2) -> bool {
        self.figure.look.is_over(&self.capri, cursor)
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
        self.banner.update(self.frame, self.overlay.scale_factor(), &mut self.renderer);
        // sa fenêtre est lue le plus tard possible avant de dessiner : le dessin la montre plus fraîche
        let (seconds, period) = (self.seconds(), pace::refresh_period(self.refresh).as_secs_f64());
        let stature = self.figure.look.stature();
        self.perch.tick(&mut self.capri, &self.overlay, seconds, period, stature);
        let talking = self.banner.is_talking();
        let music = self.music.tick(now, self.figure.look.wants_music(&self.capri));
        self.figure.look.update(&self.capri, self.frame, talking, music);

        // La souris traverse la fenêtre partout, sauf sur elle ou tant qu'on la tient.
        let over = self.cursor.is_some_and(|c| self.is_over(c));
        let through = !capri_platform::INTERACTIVE || !(over || self.capri.is_held());
        if through != self.click_through {
            self.overlay.set_click_through(through);
            self.click_through = through;
        }

        if self.drawn != Some(self.scene()) {
            self.overlay.request_redraw();
        }
    }

    /// Lui donne la taille `scale`, tout de suite : ses pieds restent où ils sont, elle reste
    /// entière à l'écran (`Capri::set_size`), son bandeau et sa bulle se replacent au dessin
    /// suivant ; posée, elle reste sur son bord s'il lui laisse la place, sinon elle repart flotter.
    /// Rend `false` si c'était déjà sa taille.
    fn resize(&mut self, scale: Scale) -> bool {
        if scale == self.scale {
            return false;
        }
        log::debug!("taille : {} %", scale.percent());
        self.scale = scale;
        self.apply_size(self.overlay.scale_factor());
        let (seconds, stature) = (self.seconds(), self.figure.look.stature());
        self.perch.resized(&mut self.capri, &self.overlay, seconds, stature);
        true
    }

    /// Sa hauteur debout à l'écran a changé (sa taille, ou la densité `density` de l'écran) : tous
    /// ses dessins suivent, ses pieds restent où ils sont.
    fn apply_size(&mut self, density: f64) {
        let look = &mut self.figure.look;
        look.set_height(standing_height(density, self.scale));
        self.capri.set_size(look.standing_size());
    }

    /// Retient sa taille dans son dossier ; si on ne peut pas, elle le dit, et la garde quand même
    /// jusqu'à ce qu'on la quitte.
    fn remember_scale(&mut self) {
        self.unsaved_scale = None;
        let saved = brain_setup::config_dir().and_then(|directory| size_setup::save(&directory, self.scale));
        if let Err(err) = saved {
            log::error!("taille : {err:#}");
            self.banner.notify(
                "Je n'ai pas pu retenir ma taille, détails dans le journal",
                capri_claude::Tone::Failed,
            );
        }
    }

    /// Changée à la molette : retenue une demi-seconde après le dernier cran, pas à chaque cran.
    fn remember_scale_once_still(&mut self) {
        if self.unsaved_scale.is_some_and(|at| at.elapsed() >= Duration::from_millis(500)) {
            self.remember_scale();
        }
    }

    /// Quand se réveiller ensuite. Au repos (elle flotte, ou posée sur une fenêtre immobile), dans
    /// un pas de 60 Hz, comme avant. Pressée (on la tient, elle vole, ou sa fenêtre bouge), à
    /// chaque rafraîchissement de l'écran : sur macOS, c'est l'écran qui réveille la boucle ;
    /// ailleurs, un minuteur à sa fréquence. Cachée, jamais pressée.
    fn next_wake(&mut self, hidden: bool) -> Instant {
        let now = Instant::now();
        let carried = matches!(self.capri.stance(), Stance::Held { .. } | Stance::Thrown { .. });
        let hurried = !hidden && (carried || self.perch.hurried(&self.capri, self.seconds()));
        self.by_screen = self.overlay.follow_refresh(hurried);
        if !hurried {
            self.clock.reset();
            return now + pace::REST;
        }
        let period = pace::refresh_period(self.refresh);
        if self.by_screen {
            // l'écran la réveillera avant : ce réveil-ci ne sert que s'il cessait de le faire
            self.clock.reset();
            self.fallback = now + period * 3;
            self.fallback
        } else {
            self.clock.next(now, period)
        }
    }

    /// Faut-il faire un pas à ce réveil ? Toujours, sauf quand c'est l'écran qui cadence la
    /// boucle : alors seulement quand c'est lui qui l'a réveillée (ou que son réveil de secours
    /// est passé). Les autres réveils (sur macOS, il en vient un après chaque image présentée) ne
    /// refont pas le pas, qui n'aurait rien de neuf à montrer.
    fn step_due(&mut self) -> bool {
        let refreshed = self.overlay.refreshed();
        !self.by_screen || refreshed || Instant::now() >= self.fallback
    }

    /// Ce qu'il faut dessiner maintenant : son dessin, et son bandeau s'il y en a un, au-dessus de
    /// ce qu'elle occupe (sans son flottement, pour que le texte reste lisible).
    fn scene(&self) -> (Picture, Option<SpriteDraw>) {
        let (width, height) = self.overlay.size();
        let bounds = Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32));
        let shown = self.figure.look.shown(&self.capri);
        let banner = self.banner.draw(self.frame, shown, bounds, self.overlay.scale_factor());
        (self.figure.look.picture(&self.capri), banner)
    }

    fn draw(&mut self) {
        let (picture, banner) = self.scene();
        let layers = self.figure.layers(&picture);
        let overlay = &self.overlay;
        #[cfg(debug_assertions)]
        let drawing = perch_rehearsal::follow::wall_us();
        self.renderer.render(&layers, banner.as_ref(), || overlay.pre_present_notify());
        #[cfg(debug_assertions)]
        {
            self.perch.drawn += 1;
            if let Some(trace) = self.perch.trace.as_mut() {
                trace.drawn(drawing, &self.capri);
            }
        }
        self.drawn = Some((picture, banner));
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

/// Sa hauteur debout à l'écran, en pixels physiques, pour un écran de `density` pixels par point,
/// à l'échelle `scale` choisie par l'utilisateur : tous ses dessins s'y rapportent (voir
/// `capri_core::scale` et `capri_core::look`).
fn standing_height(density: f64, scale: Scale) -> f32 {
    scale.standing_points() * density as f32
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
    // des lettres qui ne changent pas de place entre AZERTY et QWERTY : voir `ear_setup::SHORTCUT`
    let listen = register(Code::KeyD, ear_setup::SHORTCUT);
    Some(Hotkeys { _manager: manager, toggle, talk, listen })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use log::Log;

    use super::*;

    /// Ce que le journal a écrit.
    #[derive(Clone, Default)]
    struct Written(Arc<Mutex<Vec<u8>>>);

    impl Write for Written {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("verrou").extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn what_crosses_the_network_is_never_logged_even_at_trace_level() {
        let written = Written::default();
        let logger = NoWireLog(
            env_logger::Builder::new()
                .parse_filters("trace,ureq_proto::util=trace")
                .target(env_logger::Target::Pipe(Box::new(written.clone())))
                .build(),
        );
        let log = |target: &str, level: log::Level, text: &str| {
            logger.log(
                &log::Record::builder().target(target).level(level).args(format_args!("{text}")).build(),
            );
        };
        for target in ["ureq", "ureq::run", "ureq::tls::native_tls", "ureq_proto", "ureq_proto::util"] {
            log(target, log::Level::Trace, "Authorization: Bearer secret");
            assert!(
                !logger.enabled(&log::Metadata::builder().target(target).level(log::Level::Trace).build())
            );
        }
        assert!(written.0.lock().expect("verrou").is_empty(), "rien du fil n'est écrit");

        // le reste du journal, lui, est bien écrit
        log("ureq::run", log::Level::Debug, "connexion établie");
        log("capri_net", log::Level::Trace, "détail de Capri");
        let text = String::from_utf8(written.0.lock().expect("verrou").clone()).expect("du texte");
        assert!(text.contains("connexion établie") && text.contains("détail de Capri"), "{text}");
        assert!(!text.contains("secret"), "{text}");
    }
}
