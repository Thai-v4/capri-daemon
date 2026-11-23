//! Capri : elle flotte sur ton bureau, on peut l'attraper, la poser et la lancer (jalon 1), un
//! bandeau au-dessus d'elle dit ce que fait Claude Code (jalon 2), elle s'assoit sur tes fenêtres
//! (jalon 3), et on peut lui parler : elle répond dans une bulle, avec le modèle qu'on lui a donné,
//! local ou chez un fournisseur (jalon 4). Elle vit sa vie : de temps en temps, elle s'efface là où
//! elle est et apparaît ailleurs, de préférence assise sur une autre fenêtre (`roam_view`).
//! Au lancement, avant qu'elle n'apparaisse, une intro prend l'écran (`intro_view`), puis elle se
//! matérialise à sa place et dit bonjour ; « Intro au démarrage » de son menu la retire
//! (`intro_setup`).
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
//! Lancée comme app à fenêtre (pas pour ses commandes), elle écrit son journal dans un fichier
//! aussi, borné (`journal`), que « Montrer le journal » de son menu montre dans le Finder.
//! « Chercher les mises à jour » (cochée par défaut) lui fait regarder une fois par jour s'il
//! existe une version plus récente, sur un thread à part (`update_view`), et le dire ;
//! `capri update check` le fait sur demande. D'elle-même, elle ne télécharge ni n'installe rien :
//! sur macOS, « Installer la version … » de son menu la télécharge, la vérifie et refait son app
//! à côté de l'installée, hors du thread principal (`install_view`, `update_install`), et
//! « Redémarrer pour passer à la … » la met en place, par le programme de fin de la nouvelle app
//! (`capri update finish`, `update_finish`).

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
#[cfg(any(target_os = "macos", windows))]
mod install_view;
mod intro_setup;
mod intro_view;
mod journal;
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
mod roam_setup;
mod roam_view;
#[cfg(any(target_os = "macos", windows))]
mod settings_view;
mod size_setup;
mod style_setup;
mod update_finish;
#[cfg(target_os = "macos")]
mod update_install;
mod update_setup;
mod update_view;
#[cfg(debug_assertions)]
mod voice_rehearsal;
mod voice_setup;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use banner_view::BannerView;
use capri_core::intro::{Cut, End};
use capri_core::look::Picture;
use capri_core::{
    Button, Capri, Press, Rect, Scale, Stance, Vec2, frame_at, interpret_press, pace, place_input,
};
use capri_platform::{Overlay, OverlayEvent};
use capri_render::{ImageId, Renderer, SpriteDraw};
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
        Some("update") => std::process::exit(update_setup::run_cli(&args[1..])),
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
                 \x20       capri ear status       si elle peut t'écouter au micro\n\
                 \x20       capri update check     s'il existe une version plus récente"
            );
            std::process::exit(2);
        }
    }

    // l'app à fenêtre seulement : ses commandes de terminal, plus haut, n'écrivent aucun fichier
    let journal_file = journal::start();
    // les restes d'une mise à jour interrompue, à côté de son app, s'ils sont à nous
    #[cfg(target_os = "macos")]
    if let Some(app) = login_setup::this_app() {
        update_install::clear_leftovers(std::path::Path::new(&app));
    }
    let loaded = Instant::now();
    let sprite = assets::load_sprite()?;
    // ses planches debout et assise ; sans elles, son dessin d'avant ; et celles de sa vie (elle
    // apparaît, elle s'efface) : sans elles, elle apparaît et disparaît d'un coup
    let (stand, perch, life) = (assets::stand::load(), assets::perch::load(), assets::life::load());
    log::info!("images lues en {} ms", loaded.elapsed().as_millis());

    let mut builder = EventLoop::builder();
    capri_platform::configure_event_loop(&mut builder);
    let event_loop = builder.build().context("création de la boucle d'événements")?;

    let mut app = App { images: Some(Images { sprite, stand, perch, life }), ..App::default() };
    #[cfg(any(target_os = "macos", windows))]
    {
        app.journal_file = journal_file;
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = journal_file; // pas de menu ici pour le montrer
    event_loop.run_app(&mut app)?;
    app.fatal.map_or(Ok(()), Err)
}

/// Ses images lues au lancement, en attendant d'être confiées à la carte graphique.
struct Images {
    sprite: assets::Sprite,
    stand: Option<assets::stand::StandSheets>,
    perch: Option<assets::perch::PerchSheets>,
    life: Option<assets::life::LifeSheets>,
}

#[derive(Default)]
struct App {
    images: Option<Images>,
    running: Option<Running>,
    /// On l'a cachée (son menu le dit tout de suite) ; elle s'efface d'abord, et la fenêtre ne se
    /// cache qu'une fois qu'elle est partie (`Look::is_gone`).
    hidden: bool,
    hotkeys: Option<Hotkeys>,
    #[cfg(any(target_os = "macos", windows))]
    menus: Option<menu::Menus>,
    /// La fenêtre de réglages, tant qu'elle est ouverte.
    #[cfg(any(target_os = "macos", windows))]
    settings: Option<settings_view::SettingsView>,
    /// La place du fichier de son journal, que « Montrer le journal » montre s'il est là.
    #[cfg(any(target_os = "macos", windows))]
    journal_file: Option<std::path::PathBuf>,
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
    /// La case « Chercher les mises à jour », telle que retenue.
    updates_enabled: bool,
    /// La case « Intro au démarrage », telle que retenue (elle ne compte qu'au lancement).
    #[cfg(any(target_os = "macos", windows))]
    intro_enabled: bool,
    /// Regarder chaque jour s'il existe une version plus récente, case cochée : son thread.
    updates: Option<update_view::UpdateWatch>,
    /// La version plus récente trouvée, s'il y en a une, et son manifeste : la ligne de menu la
    /// rappelle, et l'installe sur un geste.
    update_found: Option<update_view::Found>,
    /// L'installation d'une version, sur un geste : la préparation, l'app prête, le relais.
    #[cfg(any(target_os = "macos", windows))]
    install: install_view::InstallView,
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
    /// Vivre sa vie : changer d'elle-même de place, et sur un geste.
    roam: roam_view::RoamView,
    /// L'intro du lancement, tant qu'elle se joue : elle prend l'écran, et Capri n'est pas encore
    /// là. Une fois finie ou coupée, tout ce qu'elle tenait est lâché.
    intro: Option<intro_view::IntroView>,
    /// Image d'animation courante.
    frame: u64,
    /// Ce qui est à l'écran.
    drawn: Option<Scene>,
    /// La fenêtre vient d'être montrée (au lancement, ou de nouveau après avoir été cachée), et
    /// rien n'y a encore été présenté : sur macOS, wgpu refuse de dessiner tant que le système ne
    /// la voit pas (`Occluded`), et elle montrerait jusque-là ce qu'elle montrait avant d'être
    /// cachée. Tant que c'est vrai, un dessin qui n'a rien présenté est refait au pas suivant.
    awaiting_present: bool,
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

    /// La boucle s'arrête (« Quitter », la fenêtre fermée, ou le relais d'une mise à jour pris) :
    /// une installation en cours est abandonnée, et il ne reste rien à côté de son app (sauf ce qui
    /// vient d'être confié au programme de fin).
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(any(target_os = "macos", windows))]
        self.install.abandon();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_menus(event_loop);
        #[cfg(any(target_os = "macos", windows))]
        self.handle_settings(event_loop);
        self.handle_hotkey();
        self.handle_ollama();
        self.handle_updates();
        #[cfg(any(target_os = "macos", windows))]
        self.handle_install(event_loop);
        self.handle_overlay(event_loop);
        if let Some(r) = self.running.as_mut() {
            if r.figure.look.is_gone() {
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
        let wake = self.running.as_mut().map_or_else(
            || Instant::now() + pace::REST,
            |r| {
                let gone = r.figure.look.is_gone();
                r.next_wake(gone)
            },
        );
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}

impl App {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running> {
        let Images { sprite, stand, perch, life } = self.images.take().context("démarrage déjà fait")?;
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
        let mut figure =
            figure::Figure::new(sprite, stand, perch, life, standing, |image| renderer.add_image(image))?;
        let look = &figure.look;
        log::info!(
            "dessins : debout {}, sur les fenêtres {}, elle apparaît {} ; mémoire graphique : {:.1} Mo",
            if look.has_stand() { "ses planches" } else { "son dessin d'avant" },
            if look.has_perch() { "assise" } else { "debout" },
            if look.has_life() { "par ses planches" } else { "d'un coup" },
            renderer.image_bytes() as f64 / 1_000_000.0
        );
        let body = figure.look.standing_size();
        // en bas à droite, au-dessus du Dock ou de la barre des tâches
        let start_at = Vec2::new(bounds.size.x - body.x * 1.8, bounds.size.y - body.y * 1.3);
        let mut capri = Capri::new(body, bounds, start_at);
        capri.set_feet(figure.look.standing_feet());
        capri.set_leave_frames(figure.look.leave_frames());
        // l'intro d'abord, au lancement seulement, case cochée et jamais pendant un essai : elle
        // se matérialisera quand l'intro finira ; sans intro, elle se matérialise dès l'image 0 (le
        // premier dessin est déjà celui de son arrivée)
        let intro_enabled = brain_setup::config_dir().and_then(|directory| intro_setup::load(&directory));
        let intro_enabled = intro_enabled.unwrap_or_else(|err| {
            log::warn!("réglage « Intro au démarrage » illisible, elle la joue : {err:#}");
            true
        });
        let testing = intro_setup::testing(|name| std::env::var_os(name));
        let intro = if intro_enabled && !testing { start_intro() } else { None };
        if intro.is_none() {
            figure.look.enter(&mut capri, 0);
        }
        #[cfg(any(target_os = "macos", windows))]
        {
            self.intro_enabled = intro_enabled;
        }
        // la graine de sa vie : ses trajets, et son bonjour, attendu dès que l'intro commence
        let seed = life_seed();
        let mut banner = BannerView::new();
        if intro.is_some() {
            banner.expect_greeting(capri_chat::greeting::pick(seed));
        }
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
        let roam_enabled = brain_setup::config_dir().and_then(|directory| roam_setup::load(&directory));
        let roam_enabled = roam_enabled.unwrap_or_else(|err| {
            log::warn!("réglage « Vivre sa vie » illisible, elle vit sa vie : {err:#}");
            true
        });
        let rehearsing = roam_setup::rehearsing(|name| std::env::var_os(name));
        log::info!(
            "vivre sa vie : {}",
            match (roam_enabled, rehearsing) {
                (false, _) => "non",
                (true, false) => "oui",
                (true, true) => "pas d'elle-même pendant une répétition",
            }
        );

        overlay.set_click_through(true);
        overlay.set_shown(true);
        overlay.request_redraw();

        self.hotkeys = register_hotkeys();
        // l'inscription à l'ouverture de session, lue dans le système (et réparée si l'app a changé
        // de place), ou dans le dossier d'essai que donne `CAPRI_CONFIG_DIR` ; cochée, Ollama est
        // veillé sur un thread à part
        let registered = login_setup::checked_at_launch(login_setup::this_app().as_deref());
        self.ollama = ollama_view::OllamaWatch::start(registered);
        // case cochée, le thread des mises à jour attend son heure : rien ne part avant
        // `capri_chat::update::schedule::GRACE`, et jamais depuis un essai
        self.updates_enabled = update_setup::enabled();
        if self.updates_enabled {
            self.updates = update_view::UpdateWatch::start().ok();
        }
        #[cfg(any(target_os = "macos", windows))]
        {
            self.menus = Some(menu::Menus::new());
            self.refresh_hook_menu();
            self.refresh_voice_menu();
            if let Some(menus) = &self.menus {
                menus.set_perch_enabled(perch_enabled);
                menus.set_music_enabled(music_enabled);
                menus.set_roam_enabled(roam_enabled);
                menus.set_login_registered(registered);
                menus.set_intro_enabled(intro_enabled);
                menus.set_updates_enabled(self.updates_enabled);
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
            banner,
            perch: perch_view::PerchView::new(perch_enabled),
            music,
            roam: roam_view::RoamView::new(roam_enabled, rehearsing, seed),
            intro,
            frame: 0,
            drawn: None,
            awaiting_present: true,
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
                    // partie (la fenêtre cachée), rien à dessiner ; en train de s'effacer, si
                    if !r.figure.look.is_gone() {
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
        // on lui parle : l'intro s'arrête, elle se matérialise tout de suite, et son bonjour est
        // oublié
        r.cut_intro(Cut::Talked);
        r.banner.forget_greeting();
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
                        // on lui parle : l'intro s'arrête, elle se matérialise tout de suite
                        if let Some(r) = self.running.as_mut() {
                            r.cut_intro(Cut::Talked);
                        }
                        self.with_banner(BannerView::listen_toggle);
                    }
                    menu::Action::Hush => self.with_banner(BannerView::hush),
                    menu::Action::Forget => self.with_banner(BannerView::forget),
                    menu::Action::ToggleVoice => self.toggle_voice(),
                    menu::Action::ToggleVisible => self.toggle_hidden(),
                    menu::Action::TogglePerch => self.toggle_perch(),
                    menu::Action::ToggleMusic => self.toggle_music(),
                    menu::Action::ToggleRoam => self.toggle_roam(),
                    menu::Action::Wander => self.wander(),
                    menu::Action::Larger => self.choose_size(Scale::larger),
                    menu::Action::Smaller => self.choose_size(Scale::smaller),
                    menu::Action::OriginalSize => self.choose_size(|_| Scale::ORIGINAL),
                    menu::Action::ToggleClaudeHook => self.toggle_claude_hook(),
                    menu::Action::ToggleLogin => self.toggle_login(),
                    menu::Action::ToggleIntro => self.toggle_intro(),
                    menu::Action::ToggleUpdates => self.toggle_updates(),
                    menu::Action::UpdateLine => self.choose_update(),
                    menu::Action::Settings => self.open_settings(),
                    menu::Action::ShowJournal => self.show_journal(),
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

    /// Montre le fichier de son journal dans le Finder ; s'il n'est pas là (journal en panne), ou
    /// si le montrer échoue, elle le dit dans son bandeau au lieu de ne rien faire.
    #[cfg(any(target_os = "macos", windows))]
    fn show_journal(&mut self) {
        let said = journal::show(self.journal_file.as_deref(), capri_platform::launch::reveal);
        if let Some(text) = said {
            self.with_banner(|banner| banner.notify(text, capri_claude::Tone::Failed));
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
                // décochée, elle a quitté son bord : c'est un geste, pas un bord perdu
                let now = r.seconds();
                r.roam.touched(now, &r.capri);
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

    /// Lui permet de changer d'elle-même de place de temps en temps, ou le lui interdit ; le
    /// choix est retenu dans son dossier. Si on ne peut pas le retenir, rien ne change.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_roam(&mut self) {
        let Some(r) = self.running.as_mut() else { return };
        let enabled = !r.roam.is_enabled();
        match brain_setup::config_dir().and_then(|directory| roam_setup::save(&directory, enabled)) {
            Ok(_) => {
                let now = r.seconds();
                r.roam.set_enabled(enabled, now);
                let text = if enabled {
                    "Je vais vivre ma vie : je changerai de place de temps en temps"
                } else {
                    "Je reste là où tu me mets"
                };
                r.banner.notify(text, capri_claude::Tone::Done);
            }
            Err(err) => {
                log::error!("réglage « Vivre sa vie » : {err:#}");
                r.banner.notify(
                    "Je n'ai pas pu retenir ce choix, détails dans le journal",
                    capri_claude::Tone::Failed,
                );
            }
        }
        if let Some(menus) = &self.menus {
            menus.set_roam_enabled(r.roam.is_enabled());
        }
    }

    /// Joue l'intro au prochain lancement (et dit bonjour après), ou plus ; le choix est retenu dans
    /// son dossier. L'intro en cours, s'il y en a une, n'en est pas changée. Si on ne peut pas le
    /// retenir, rien ne change.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_intro(&mut self) {
        let enabled = !self.intro_enabled;
        let (text, tone) =
            match brain_setup::config_dir().and_then(|directory| intro_setup::save(&directory, enabled)) {
                Ok(_) => {
                    self.intro_enabled = enabled;
                    let text = if enabled {
                        "Au prochain lancement, je ferai mon entrée"
                    } else {
                        "Au prochain lancement, j'apparaîtrai sans intro"
                    };
                    (text, capri_claude::Tone::Done)
                }
                Err(err) => {
                    log::error!("réglage « Intro au démarrage » : {err:#}");
                    ("Je n'ai pas pu retenir ce choix, détails dans le journal", capri_claude::Tone::Failed)
                }
            };
        self.with_banner(|banner| banner.notify(text, tone));
        if let Some(menus) = &self.menus {
            menus.set_intro_enabled(self.intro_enabled);
        }
    }

    /// « Changer de place » : elle part au prochain pas, si rien ne l'en empêche. Cachée, rien :
    /// on ne la voit pas partir.
    #[cfg(any(target_os = "macos", windows))]
    fn wander(&mut self) {
        if self.hidden {
            log::debug!("« Changer de place » : elle est cachée, rien ne change");
            return;
        }
        // pendant l'intro, elle n'est pas encore là : rien ne change
        if let Some(r) = self.running.as_mut().filter(|r| r.intro.is_none()) {
            r.roam.ask();
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

    /// L'inscrit à l'ouverture de session, ou l'en retire (dans le dossier d'essai si
    /// `CAPRI_CONFIG_DIR` est posée). Cochée, elle veille aussi à ce qu'Ollama tourne, tout de
    /// suite, si c'est son cerveau sur cette machine.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_login(&mut self) {
        use capri_chat::startup::{self, say};
        let done = login_setup::agents_dir()
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
        let shown = login_setup::agents_dir()
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

    /// Ce que la veille des mises à jour a trouvé. Une version plus récente, elle la dit une fois
    /// au-dessus de sa tête (à son tour : `BannerView::announce`), et une ligne de ses menus la
    /// rappelle ; la même, trouvée de nouveau le lendemain, n'est pas redite.
    fn handle_updates(&mut self) {
        use capri_chat::update::say;
        let Some(found) = self.updates.as_ref().and_then(update_view::UpdateWatch::heard) else { return };
        let before = self.update_found.as_ref().and_then(|found| found.verdict.newer());
        let newer = found.verdict.newer();
        self.update_found = newer.map(|_| found);
        #[cfg(any(target_os = "macos", windows))]
        self.refresh_update_line();
        if let Some(version) = newer.filter(|&version| before != Some(version))
            && let Some(r) = self.running.as_mut()
        {
            r.banner.announce(say::available(version), capri_claude::Tone::Done);
        }
    }

    /// La ligne des mises à jour de ses menus : la version trouvée, ou où en est son installation.
    #[cfg(any(target_os = "macos", windows))]
    fn refresh_update_line(&self) {
        let text = self.install.menu_line(self.update_found.as_ref().map(|found| &found.verdict));
        if let Some(menus) = &self.menus {
            menus.set_update_line(text.as_deref());
        }
    }

    /// Ce que l'installation d'une version a rapporté : dit au-dessus de sa tête, à son tour ; et,
    /// le relais pris par le programme de fin, elle se quitte, par le même chemin que « Quitter ».
    #[cfg(any(target_os = "macos", windows))]
    fn handle_install(&mut self, event_loop: &ActiveEventLoop) {
        let heard = self.install.pump();
        if heard.said.is_empty() && !heard.quit {
            return;
        }
        for (text, tone) in heard.said {
            self.with_banner(|banner| banner.announce(text, tone));
        }
        self.refresh_update_line();
        if heard.quit {
            event_loop.exit();
        }
    }

    /// Lui permet de regarder chaque jour s'il existe une version plus récente, ou le lui
    /// interdit ; le choix est retenu dans son dossier. Décochée, la veille s'arrête, et elle
    /// oublie ce qu'elle avait trouvé. Si on ne peut pas le retenir, rien ne change.
    #[cfg(any(target_os = "macos", windows))]
    fn toggle_updates(&mut self) {
        use capri_chat::update::say;
        use capri_claude::Tone;
        let enabled = !self.updates_enabled;
        let saved = brain_setup::config_dir().and_then(|directory| update_setup::save(&directory, enabled));
        let (text, tone) = match saved {
            Err(err) => {
                log::error!("réglage « Chercher les mises à jour » : {err:#}");
                ("Je n'ai pas pu retenir ce choix, détails dans le journal", Tone::Failed)
            }
            Ok(_) if enabled => {
                self.updates_enabled = true;
                match update_view::UpdateWatch::start() {
                    Ok(watch) => {
                        self.updates = Some(watch);
                        (say::CHECKING, Tone::Done)
                    }
                    Err(why) => (why, Tone::Done),
                }
            }
            Ok(_) => {
                self.updates_enabled = false;
                self.updates = None;
                self.update_found = None;
                // une installation en cours est arrêtée, et rien ne reste à côté de son app
                self.install.abandon();
                self.refresh_update_line();
                self.with_banner(BannerView::forget_announcement);
                (say::NOT_CHECKING, Tone::Done)
            }
        };
        self.with_banner(|banner| banner.notify(text, tone));
        if let Some(menus) = &self.menus {
            menus.set_updates_enabled(self.updates_enabled);
        }
    }

    /// La ligne des mises à jour de son menu, choisie : le geste qui installe la version trouvée,
    /// puis celui qui la redémarre pour finir ; ou ce qui l'empêche (`install_view`).
    #[cfg(any(target_os = "macos", windows))]
    fn choose_update(&mut self) {
        let said = self.install.chosen(self.update_found.as_ref());
        self.refresh_update_line();
        if let Some((text, tone)) = said {
            self.with_banner(|banner| banner.announce(text, tone));
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
            // on lui parle : l'intro s'arrête, elle se matérialise tout de suite
            r.cut_intro(Cut::Talked);
            r.overlay.hide_input(); // on ne tape pas et on ne parle pas en même temps
            r.banner.listen_press();
        } else {
            r.banner.listen_release();
        }
    }

    /// La cache, ou la fait revenir : elle s'efface, ou apparaît (ses planches de vie ; sans
    /// elles, d'un coup, comme avant). Cachée ou montrée de nouveau pendant ce temps, elle revient
    /// sur ses pas. Quitter, lui, reste immédiat : la boucle s'arrête sans rien jouer.
    fn toggle_hidden(&mut self) {
        let Some(r) = self.running.as_mut() else { return };
        self.hidden = !self.hidden;
        r.overlay.set_click_through(true);
        r.click_through = true;
        if self.hidden {
            r.overlay.hide_input();
            r.vanish();
        } else {
            r.come_back();
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
        // pendant l'intro, elle n'est pas encore là : rien ne s'attrape
        self.intro.is_none() && self.figure.look.is_over(&self.capri, cursor)
    }

    /// On la cache : elle s'efface, là où elle est dessinée ; la fenêtre se cache une fois
    /// qu'elle est partie (`tick`), ou tout de suite sans ses planches de vie, comme avant.
    fn vanish(&mut self) {
        self.banner.forget_greeting();
        // pendant l'intro : elle s'arrête net, et Capri, qui n'était pas encore là, est cachée
        if self.intro.is_some() {
            return self.cut_intro(Cut::Hidden);
        }
        self.figure.look.vanish(&mut self.capri, self.frame);
        if self.figure.look.is_gone() {
            self.overlay.set_shown(false);
        }
    }

    /// On la montre. Partie : la fenêtre se montre, sa fenêtre à elle est relue tout de suite
    /// (elle a pu bouger ou se fermer pendant ce temps), et elle apparaît, assise si elle y est
    /// encore posée, debout sinon. En train de s'effacer : elle revient sur ses pas.
    fn come_back(&mut self) {
        if self.figure.look.is_gone() {
            let now = Instant::now();
            self.overlay.set_shown(true);
            self.drawn = None; // redessiner en revenant
            self.awaiting_present = true;
            self.last_step = now;
            self.frame = frame_at(now - self.start);
            self.capri.step(0.0, self.frame);
            self.perch.wake();
            let (seconds, period) = (self.seconds(), pace::refresh_period(self.refresh).as_secs_f64());
            let stature = self.figure.look.stature();
            self.perch.tick(&mut self.capri, &self.overlay, seconds, period, stature);
        }
        self.figure.look.appear(&mut self.capri, self.frame);
        let seconds = self.seconds();
        self.roam.came_back(seconds, &self.capri);
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
        if self.intro.is_some() {
            return self.intro_tick(now);
        }
        // en train de s'effacer, comme cachée : elle suit Claude Code sans rien dire ; en train
        // d'apparaître, ce qui attend son tour attend encore
        if self.figure.look.is_away() {
            self.banner.listen_only();
        } else {
            let here = self.figure.look.is_here();
            self.banner.update(self.frame, self.overlay.scale_factor(), &mut self.renderer, here);
        }
        // sa fenêtre est lue le plus tard possible avant de dessiner : le dessin la montre plus fraîche
        let (seconds, period) = (self.seconds(), pace::refresh_period(self.refresh).as_secs_f64());
        let stature = self.figure.look.stature();
        self.perch.tick(&mut self.capri, &self.overlay, seconds, period, stature);
        let talking = self.banner.is_talking();
        let music = self.music.tick(now, self.figure.look.wants_music(&self.capri));
        self.figure.look.update(&self.capri, self.frame, talking, music);
        // sa vie : elle part d'elle-même ailleurs, ou y apparaît, effacée d'ici
        self.roam_step(seconds);
        // partie : la fenêtre se cache, et plus rien ne se dessine jusqu'à son retour
        if self.figure.look.is_gone() {
            self.overlay.set_shown(false);
            return;
        }

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

    /// Un pas pendant l'intro : elle seule à l'écran, et rien ne s'attrape. Capri n'est pas encore
    /// là : rien au-dessus de sa tête (son bandeau suit Claude Code sans rien dessiner), ni sa
    /// fenêtre suivie, ni sa vie, ni la musique.
    fn intro_tick(&mut self, now: Instant) {
        self.banner.listen_only();
        self.music.tick(now, false);
        let Some(view) = self.intro.as_mut() else { return };
        if let Some(end) = view.step(self.frame, &mut self.renderer) {
            return self.end_intro(end);
        }
        if self.drawn != Some(self.scene()) {
            self.overlay.request_redraw();
        }
    }

    /// L'intro est coupée (`why`), si elle se joue encore.
    fn cut_intro(&mut self, why: Cut) {
        if let Some(view) = self.intro.as_mut() {
            let end = view.cut(why);
            self.end_intro(end);
        }
    }

    /// L'intro est finie (`end`) : tout ce qu'elle tenait est lâché, et sa ligne écrite au journal.
    /// Coupée en la cachant, Capri est cachée d'un coup ; sinon elle se matérialise à sa place, et
    /// dit bonjour si l'intro s'est jouée.
    fn end_intro(&mut self, end: End) {
        let Some(view) = self.intro.take() else { return };
        log::info!("{}", view.close(end, &mut self.renderer));
        self.drawn = None;
        // son bonjour, s'il est encore attendu, attend qu'elle soit là et libre ; ou il n'y en a pas
        self.banner.intro_over(end.greets());
        if end.hides() {
            // une image vide avant de cacher la fenêtre : rien de l'intro n'y reste, qu'elle
            // montrerait un instant quand on la montre de nouveau
            let overlay = &self.overlay;
            self.renderer.render(&[], None, || overlay.pre_present_notify());
        }
        // la toile lâchée est rendue tout de suite, même si plus rien ne se dessine ensuite (cachée,
        // ou l'écran verrouillé) : wgpu ne le ferait qu'à la prochaine soumission
        self.renderer.settle();
        if end.hides() {
            self.figure.look.hide_unseen(self.frame);
            self.overlay.set_shown(false);
            return;
        }
        self.figure.look.enter(&mut self.capri, self.frame);
        let seconds = self.seconds();
        self.roam.came_back(seconds, &self.capri);
        self.overlay.request_redraw();
    }

    /// Un pas de sa vie, à l'instant `now`, après son dessin : changer de place si c'est le moment
    /// (ou qu'on le lui a demandé) et que rien ne la retient, ou apparaître là où elle allait. Ce
    /// qu'elle a à en dire passe par son bandeau.
    fn roam_step(&mut self, now: f64) {
        let talking = self.banner.is_engaged() || self.overlay.input_is_open();
        let hovered = self.cursor.is_some_and(|c| self.is_over(c));
        let mut desk =
            roam_view::Desk { perch: &mut self.perch, overlay: &self.overlay, now, cursor: self.cursor };
        let busy = capri_core::roam::Busy { talking, hovered };
        let said = self.roam.step(&mut self.figure.look, &mut self.capri, &mut desk, busy, now, self.frame);
        if let Some((text, tone)) = said {
            self.banner.notify(text, tone);
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
        // trop grande pour son bord, elle l'a quitté : c'est un geste, pas un bord perdu
        self.roam.touched(seconds, &self.capri);
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

    /// Ce qu'il faut dessiner maintenant : pendant l'intro, son image à l'écran ; sinon son dessin,
    /// et son bandeau s'il y en a un, au-dessus de ce qu'elle occupe (sans son flottement, pour
    /// que le texte reste lisible).
    fn scene(&self) -> Scene {
        let (width, height) = self.overlay.size();
        if let Some(view) = &self.intro {
            return Scene::Intro(view.picture(Vec2::new(width as f32, height as f32)));
        }
        let bounds = Rect::new(Vec2::ZERO, Vec2::new(width as f32, height as f32));
        let shown = self.figure.look.shown(&self.capri);
        // en train de s'effacer, son bandeau s'en va avec elle
        let banner = if self.figure.look.is_away() {
            None
        } else {
            self.banner.draw(self.frame, shown, bounds, self.overlay.scale_factor())
        };
        Scene::Her(self.figure.look.picture(&self.capri), banner)
    }

    fn draw(&mut self) {
        let scene = self.scene();
        let (layers, banner) = match scene {
            Scene::Her(picture, banner) => (self.figure.layers(&picture), banner),
            Scene::Intro(shown) => {
                (shown.map(|(id, _, rect)| vec![intro_view::layer(id, rect)]).unwrap_or_default(), None)
            }
        };
        let overlay = &self.overlay;
        #[cfg(debug_assertions)]
        let drawing = perch_rehearsal::follow::wall_us();
        let presented = self.renderer.render(&layers, banner.as_ref(), || overlay.pre_present_notify());
        #[cfg(debug_assertions)]
        {
            self.perch.drawn += 1;
            if let Some(trace) = self.perch.trace.as_mut() {
                trace.drawn(drawing, &self.capri);
            }
        }
        // juste après l'avoir montrée, seul un dessin présenté compte (voir `awaiting_present`) ;
        // ensuite, comme avant : une fenêtre occultée longtemps (écran verrouillé) ne fait pas
        // redessiner à chaque pas
        if presented {
            self.awaiting_present = false;
        }
        self.drawn = (presented || !self.awaiting_present).then_some(scene);
    }
}

/// Ce qui est à l'écran.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scene {
    /// Elle, et son bandeau s'il y en a un.
    Her(Picture, Option<SpriteDraw>),
    /// L'intro : sa toile, le numéro de l'image qui y est peinte, et où ; rien avant la première.
    Intro(Option<(ImageId, u32, Rect)>),
}

/// L'intro du lancement, si ses images sont là (sinon une ligne au journal, et pas d'intro).
fn start_intro() -> Option<intro_view::IntroView> {
    let started = match assets::intro::sequence() {
        Some(sequence) => intro_view::IntroView::start(sequence, 0),
        None => Err(End::Missing(capri_core::intro::Missing::Absent { found: 0 })),
    };
    started
        .inspect_err(|&end| log::info!("{}", intro_view::journal_line(end, capri_core::intro::COUNT, None)))
        .ok()
}

/// Le bouton de winit, dans le vocabulaire de `capri-core`.
fn core_button(button: MouseButton) -> Button {
    match button {
        MouseButton::Left => Button::Primary,
        MouseButton::Right => Button::Secondary,
        _ => Button::Other,
    }
}

/// La graine de sa vie (`capri_core::roam::Rng`), prise à l'horloge du système au lancement : une
/// vie différente à chaque lancement. Rien de secret : de quoi ne pas refaire chaque jour les mêmes
/// trajets.
fn life_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64);
    nanos ^ (u64::from(std::process::id()) << 32)
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
