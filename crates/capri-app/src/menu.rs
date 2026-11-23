//! L'icône de la barre des menus (macOS) / de la zone de notification (Windows), et le même
//! menu en clic droit sur Capri.

use std::cell::Cell;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
use tray_icon::menu::{
    CheckMenuItem, ContextMenu as _, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Ouvrir le champ de saisie pour lui parler.
    Talk,
    /// Lui parler au micro : commencer à écouter, ou arrêter.
    Listen,
    /// La faire taire : elle arrête de répondre, et sa bulle s'efface.
    Hush,
    /// Repartir de zéro : elle oublie la conversation.
    Forget,
    /// Lui donner ou lui retirer la parole à voix haute.
    ToggleVoice,
    ToggleVisible,
    /// Lui permettre de s'asseoir sur les fenêtres, ou le lui interdire.
    TogglePerch,
    /// Lui permettre de se balancer sur la musique, assise, ou le lui interdire.
    ToggleMusic,
    /// Lui permettre de changer d'elle-même de place de temps en temps, ou le lui interdire.
    ToggleRoam,
    /// « Changer de place » : tout de suite, si rien ne l'en empêche.
    Wander,
    /// Sa taille : un cran au-dessus, un cran en dessous, ou celle d'origine.
    Larger,
    Smaller,
    OriginalSize,
    /// Brancher ou débrancher le hook de Claude Code.
    ToggleClaudeHook,
    /// L'inscrire à l'ouverture de session (avec Ollama si c'est son cerveau ici), ou l'en retirer.
    ToggleLogin,
    /// Jouer l'intro au lancement (et dire bonjour après), ou plus.
    ToggleIntro,
    /// Lui permettre de regarder chaque jour s'il existe une version plus récente, ou le lui
    /// interdire.
    ToggleUpdates,
    /// La ligne des mises à jour, là seulement quand une version plus récente existe : sur macOS,
    /// « Installer la version … » (le geste qui l'installe), puis « Redémarrer pour passer à la … »
    /// (celui qui finit) ; ailleurs, ou si elle ne s'installe pas depuis l'app, « Version …
    /// disponible », qui dit pourquoi.
    UpdateLine,
    /// Ouvrir la fenêtre de réglages, ou la ramener devant.
    Settings,
    /// Montrer le fichier de son journal dans le Finder.
    ShowJournal,
    Quit,
}

pub struct Menus {
    _tray: Option<TrayIcon>,
    context: Menu,
    /// Ce que déclenche chaque élément, dans les deux menus.
    actions: Vec<(MenuId, Action)>,
    /// Les éléments dont le texte change.
    toggles: Vec<MenuItem>,
    hooks: Vec<MenuItem>,
    voices: Vec<MenuItem>,
    /// La case « S'asseoir sur les fenêtres », dans les deux menus.
    perches: Vec<CheckMenuItem>,
    /// La case « Se balancer sur la musique », dans les deux menus.
    musics: Vec<CheckMenuItem>,
    /// La case « Vivre sa vie », dans les deux menus.
    roams: Vec<CheckMenuItem>,
    /// La case « Se lancer à l'ouverture de session », dans les deux menus.
    logins: Vec<CheckMenuItem>,
    /// La case « Intro au démarrage », dans les deux menus.
    intros: Vec<CheckMenuItem>,
    /// La case « Chercher les mises à jour », dans les deux menus.
    updates: Vec<CheckMenuItem>,
    /// Dans chacun des deux menus, la ligne « Version … disponible », glissée juste sous la case
    /// des mises à jour quand une version plus récente existe, et retirée sinon.
    update_lines: Vec<(Menu, MenuItem)>,
    /// La ligne est-elle dans les menus ?
    update_line_shown: Cell<bool>,
    /// Le sous-menu « Taille », dans les deux menus : son titre dit la taille du moment.
    sizes: Vec<Submenu>,
    larger: Vec<MenuItem>,
    smaller: Vec<MenuItem>,
}

/// Un menu et ses éléments, avec ce que chacun déclenche.
struct Built {
    menu: Menu,
    items: Vec<(Action, MenuItem)>,
    perch: CheckMenuItem,
    music: CheckMenuItem,
    roam: CheckMenuItem,
    login: CheckMenuItem,
    intro: CheckMenuItem,
    updates: CheckMenuItem,
    /// Pas encore dans le menu.
    update_line: MenuItem,
    size: Submenu,
}

impl Menus {
    /// À créer une fois la boucle d'événements lancée (exigence de macOS).
    pub fn new() -> Self {
        let (a, b) = (build_menu(), build_menu());
        // un menu de muda partage son contenu avec ses copies : celle-ci reste pour y glisser la
        // ligne des mises à jour
        let tray_menu = a.menu.clone();
        let builder = TrayIconBuilder::new().with_menu(Box::new(a.menu)).with_tooltip("Capri");
        // macOS : icône « modèle », teintée par le système selon le thème clair ou sombre
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(tray_icon());
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(tray_icon());
        let tray =
            builder.build().inspect_err(|err| log::warn!("pas d'icône de barre des menus : {err}")).ok();
        let perches = vec![a.perch, b.perch];
        let musics = vec![a.music, b.music];
        let roams = vec![a.roam, b.roam];
        let logins = vec![a.login, b.login];
        let intros = vec![a.intro, b.intro];
        let updates = vec![a.updates, b.updates];
        let update_lines = vec![(tray_menu, a.update_line), (b.menu.clone(), b.update_line)];
        let sizes = vec![a.size, b.size];
        let items: Vec<(Action, MenuItem)> = a.items.into_iter().chain(b.items).collect();
        let items_for = |wanted: Action| -> Vec<MenuItem> {
            items.iter().filter(|(action, _)| *action == wanted).map(|(_, item)| item.clone()).collect()
        };
        Self {
            _tray: tray,
            context: b.menu,
            actions: items
                .iter()
                .map(|(action, item)| (item.id().clone(), *action))
                .chain(perches.iter().map(|item| (item.id().clone(), Action::TogglePerch)))
                .chain(musics.iter().map(|item| (item.id().clone(), Action::ToggleMusic)))
                .chain(roams.iter().map(|item| (item.id().clone(), Action::ToggleRoam)))
                .chain(logins.iter().map(|item| (item.id().clone(), Action::ToggleLogin)))
                .chain(intros.iter().map(|item| (item.id().clone(), Action::ToggleIntro)))
                .chain(updates.iter().map(|item| (item.id().clone(), Action::ToggleUpdates)))
                .chain(update_lines.iter().map(|(_, item)| (item.id().clone(), Action::UpdateLine)))
                .collect(),
            toggles: items_for(Action::ToggleVisible),
            hooks: items_for(Action::ToggleClaudeHook),
            voices: items_for(Action::ToggleVoice),
            perches,
            musics,
            roams,
            logins,
            intros,
            updates,
            update_lines,
            update_line_shown: Cell::new(false),
            sizes,
            larger: items_for(Action::Larger),
            smaller: items_for(Action::Smaller),
        }
    }

    /// Sa taille du moment, dans le titre du sous-menu « Taille » des deux menus ; « Plus
    /// grande » et « Plus petite » s'éteignent aux bornes.
    pub fn set_size(&self, scale: capri_core::Scale) {
        for submenu in &self.sizes {
            submenu.set_text(size_title(scale));
        }
        for item in &self.larger {
            item.set_enabled(!scale.is_largest());
        }
        for item in &self.smaller {
            item.set_enabled(!scale.is_smallest());
        }
    }

    /// Les choix faits dans un menu depuis le dernier appel.
    pub fn pending_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            actions.extend(self.actions.iter().find(|(id, _)| *id == event.id).map(|(_, action)| *action));
        }
        actions
    }

    pub fn set_visible(&self, visible: bool) {
        for item in &self.toggles {
            item.set_text(if visible { "Cacher Capri" } else { "Montrer Capri" });
        }
    }

    /// Sa voix est-elle activée ?
    pub fn set_voice_enabled(&self, enabled: bool) {
        for item in &self.voices {
            item.set_text(if enabled { "Couper sa voix" } else { "Activer sa voix" });
        }
    }

    /// S'assoit-elle sur les fenêtres ? Coche ou décoche la case des deux menus (le système coche
    /// d'office celle qu'on vient de choisir : c'est le réglage retenu qui décide).
    pub fn set_perch_enabled(&self, enabled: bool) {
        for item in &self.perches {
            item.set_checked(enabled);
        }
    }

    /// Se balance-t-elle sur la musique ? Coche ou décoche la case des deux menus. Là où elle ne
    /// sait pas si un son joue (ailleurs que sur macOS), la case est grisée.
    pub fn set_music_enabled(&self, enabled: bool) {
        for item in &self.musics {
            item.set_checked(enabled);
            item.set_enabled(capri_platform::music::AVAILABLE);
        }
    }

    /// Vit-elle sa vie (elle change d'elle-même de place de temps en temps) ? Coche ou décoche la
    /// case des deux menus (c'est le réglage retenu qui décide).
    pub fn set_roam_enabled(&self, enabled: bool) {
        for item in &self.roams {
            item.set_checked(enabled);
        }
    }

    /// Est-elle inscrite à l'ouverture de session ? Coche ou décoche la case des deux menus (c'est
    /// l'inscription lue dans le système qui décide). Là où elle ne sait pas s'inscrire (ailleurs
    /// que sur macOS), la case est grisée.
    pub fn set_login_registered(&self, registered: bool) {
        for item in &self.logins {
            item.set_checked(registered);
            item.set_enabled(capri_platform::login::AVAILABLE);
        }
    }

    /// L'intro se joue-t-elle au lancement ? Coche ou décoche la case des deux menus (c'est le
    /// réglage retenu qui décide).
    pub fn set_intro_enabled(&self, enabled: bool) {
        for item in &self.intros {
            item.set_checked(enabled);
        }
    }

    /// Regarde-t-elle chaque jour s'il existe une version plus récente ? Coche ou décoche la case
    /// des deux menus (c'est le réglage retenu qui décide).
    pub fn set_updates_enabled(&self, enabled: bool) {
        for item in &self.updates {
            item.set_checked(enabled);
        }
    }

    /// La ligne qui rappelle qu'une version plus récente existe (`Some` : son texte), glissée
    /// sous la case des mises à jour des deux menus ; `None` la retire.
    pub fn set_update_line(&self, text: Option<&str>) {
        let shown = self.update_line_shown.get();
        for ((menu, line), anchor) in self.update_lines.iter().zip(&self.updates) {
            match text {
                Some(text) => {
                    line.set_text(text);
                    if shown {
                        continue;
                    }
                    let at = menu.items().iter().position(|item| item.id() == anchor.id());
                    let inserted = menu.insert(line, at.map_or(0, |at| at + 1));
                    if let Err(err) = inserted {
                        log::warn!("menu : ligne des mises à jour pas ajoutée : {err}");
                    }
                }
                None if shown => {
                    if let Err(err) = menu.remove(line) {
                        log::warn!("menu : ligne des mises à jour pas retirée : {err}");
                    }
                }
                None => {}
            }
        }
        self.update_line_shown.set(text.is_some());
    }

    /// `Some(true)` : le hook de Claude Code est branché ; `None` : impossible de le savoir.
    pub fn set_hook_installed(&self, installed: Option<bool>) {
        for item in &self.hooks {
            item.set_enabled(installed.is_some());
            item.set_text(match installed {
                Some(true) => "Débrancher Claude Code",
                Some(false) => "Brancher Claude Code",
                None => "Claude Code : réglages illisibles",
            });
        }
    }

    /// Le menu en clic droit, sous le curseur.
    pub fn show_context(&self, window: &impl HasWindowHandle) {
        let Ok(handle) = window.window_handle() else { return };
        match handle.as_raw() {
            #[cfg(target_os = "macos")]
            RawWindowHandle::AppKit(h) => {
                // SAFETY: `ns_view` est la vue vivante de la fenêtre, utilisée sur le thread principal.
                let picked = unsafe { self.context.show_context_menu_for_nsview(h.ns_view.as_ptr(), None) };
                log::debug!("menu contextuel refermé (choix fait : {picked})");
            }
            #[cfg(windows)]
            RawWindowHandle::Win32(h) => {
                // SAFETY: `hwnd` est la fenêtre vivante, utilisée sur le thread de sa boucle.
                unsafe { self.context.show_context_menu_for_hwnd(h.hwnd.get(), None) };
            }
            _ => {}
        }
    }
}

fn build_menu() -> Built {
    let title = MenuItem::new("Capri — prototype", false, None);
    let talk_shortcut = Accelerator::new(Modifiers::CONTROL | Modifiers::ALT, Code::KeyP);
    let talk = MenuItem::new("Parler à Capri", true, Some(talk_shortcut));
    let listen_shortcut = Accelerator::new(Modifiers::CONTROL | Modifiers::ALT, Code::KeyD);
    let listen = MenuItem::new("Lui parler au micro", true, Some(listen_shortcut));
    let hush = MenuItem::new("La faire taire", true, None);
    let forget = MenuItem::new("Nouvelle conversation", true, None);
    let voice = MenuItem::new("Activer sa voix", true, None);
    let shortcut = Accelerator::new(Modifiers::CONTROL | Modifiers::ALT, Code::KeyC);
    let toggle = MenuItem::new("Cacher Capri", true, Some(shortcut));
    let perch = CheckMenuItem::new("S'asseoir sur les fenêtres", true, true, None);
    let music = CheckMenuItem::new("Se balancer sur la musique", true, true, None);
    // cochée par défaut ; le réglage retenu décide au lancement
    let roam = CheckMenuItem::new("Vivre sa vie", true, true, None);
    let wander = MenuItem::new("Changer de place", true, None);
    let larger = MenuItem::new("Plus grande", true, None);
    let smaller = MenuItem::new("Plus petite", true, None);
    let original = MenuItem::new("Taille d'origine", true, None);
    let size = Submenu::new(size_title(capri_core::Scale::ORIGINAL), true);
    if let Err(err) = size.append_items(&[&larger, &smaller, &original]) {
        log::warn!("menu « Taille » incomplet : {err}");
    }
    let hook = MenuItem::new("Brancher Claude Code", true, None);
    let login = CheckMenuItem::new(
        "Se lancer à l'ouverture de session",
        capri_platform::login::AVAILABLE,
        false,
        None,
    );
    // cochée par défaut ; le réglage retenu décide au lancement
    let intro = CheckMenuItem::new("Intro au démarrage", true, true, None);
    // cochée par défaut ; le réglage retenu décide au lancement
    let updates = CheckMenuItem::new("Chercher les mises à jour", true, true, None);
    let update_line = MenuItem::new("Version disponible", true, None);
    let settings = MenuItem::new("Réglages…", true, None);
    // grisée là où elle ne sait pas montrer un fichier (ailleurs que sur macOS)
    let journal = MenuItem::new("Montrer le journal", capri_platform::launch::AVAILABLE, None);
    let quit = MenuItem::new("Quitter", true, None);
    let menu = Menu::new();
    let separator = PredefinedMenuItem::separator;
    let (first, second, third) = (separator(), separator(), separator());
    if let Err(err) = menu.append_items(&[
        &title, &first, &talk, &listen, &hush, &forget, &voice, &second, &toggle, &wander, &perch, &music,
        &roam, &size, &hook, &login, &intro, &updates, &settings, &journal, &third, &quit,
    ]) {
        log::warn!("menu incomplet : {err}");
    }
    let items = vec![
        (Action::Talk, talk),
        (Action::Listen, listen),
        (Action::Hush, hush),
        (Action::Forget, forget),
        (Action::ToggleVoice, voice),
        (Action::ToggleVisible, toggle),
        (Action::Wander, wander),
        (Action::Larger, larger),
        (Action::Smaller, smaller),
        (Action::OriginalSize, original),
        (Action::ToggleClaudeHook, hook),
        (Action::Settings, settings),
        (Action::ShowJournal, journal),
        (Action::Quit, quit),
    ];
    Built { menu, items, perch, music, roam, login, intro, updates, update_line, size }
}

/// « Taille : 80 % ».
fn size_title(scale: capri_core::Scale) -> String {
    format!("Taille : {} %", scale.percent())
}

/// Un masque stylisé (visage en losange, deux yeux en amande), dessiné à la volée :
/// pas d'image de la série dans l'icône système.
fn tray_icon() -> Icon {
    const SIZE: u32 = 44; // 22 pt en Retina
    let color = if cfg!(target_os = "macos") { [0, 0, 0] } else { [224, 35, 78] };
    let rgba = mask_icon_rgba(SIZE, color);
    Icon::from_rgba(rgba, SIZE, SIZE).expect("taille d'icône cohérente")
}

fn mask_icon_rgba(size: u32, color: [u8; 3]) -> Vec<u8> {
    const SS: u32 = 4; // suréchantillonnage, pour des bords lisses
    let inside = |x: f32, y: f32| {
        let face = x.abs() * 1.2 + (y + 0.05).abs() * 0.95 <= 0.92;
        let eye = |cx: f32, tilt: f32| {
            let dx = x - cx;
            let dy = y + 0.12 - tilt * dx;
            let half = 0.24;
            dx.abs() < half && dy.abs() < 0.08 * (1.0 - (dx / half).powi(2)).max(0.0).sqrt()
        };
        face && !eye(-0.33, -0.4) && !eye(0.33, 0.4)
    };
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = ((px * SS + sx) as f32 + 0.5) / (size * SS) as f32 * 2.0 - 1.0;
                    let y = ((py * SS + sy) as f32 + 0.5) / (size * SS) as f32 * 2.0 - 1.0;
                    hits += u32::from(inside(x, y));
                }
            }
            out.extend_from_slice(&color);
            out.push((hits * 255 / (SS * SS)) as u8);
        }
    }
    out
}
