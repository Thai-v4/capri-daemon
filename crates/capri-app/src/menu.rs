//! L'icône de la barre des menus (macOS) / de la zone de notification (Windows), et le même
//! menu en clic droit sur Capri.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
use tray_icon::menu::{ContextMenu as _, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Ouvrir le champ de saisie pour lui parler.
    Talk,
    /// La faire taire : elle arrête de répondre, et sa bulle s'efface.
    Hush,
    /// Repartir de zéro : elle oublie la conversation.
    Forget,
    ToggleVisible,
    /// Brancher ou débrancher le hook de Claude Code.
    ToggleClaudeHook,
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
}

/// Un menu et ses éléments, avec ce que chacun déclenche.
struct Built {
    menu: Menu,
    items: Vec<(Action, MenuItem)>,
}

impl Menus {
    /// À créer une fois la boucle d'événements lancée (exigence de macOS).
    pub fn new() -> Self {
        let (a, b) = (build_menu(), build_menu());
        let builder = TrayIconBuilder::new().with_menu(Box::new(a.menu)).with_tooltip("Capri");
        // macOS : icône « modèle », teintée par le système selon le thème clair ou sombre
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(tray_icon());
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(tray_icon());
        let tray =
            builder.build().inspect_err(|err| log::warn!("pas d'icône de barre des menus : {err}")).ok();
        let items: Vec<(Action, MenuItem)> = a.items.into_iter().chain(b.items).collect();
        let items_for = |wanted: Action| -> Vec<MenuItem> {
            items.iter().filter(|(action, _)| *action == wanted).map(|(_, item)| item.clone()).collect()
        };
        Self {
            _tray: tray,
            context: b.menu,
            actions: items.iter().map(|(action, item)| (item.id().clone(), *action)).collect(),
            toggles: items_for(Action::ToggleVisible),
            hooks: items_for(Action::ToggleClaudeHook),
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
    let hush = MenuItem::new("La faire taire", true, None);
    let forget = MenuItem::new("Nouvelle conversation", true, None);
    let shortcut = Accelerator::new(Modifiers::CONTROL | Modifiers::ALT, Code::KeyC);
    let toggle = MenuItem::new("Cacher Capri", true, Some(shortcut));
    let hook = MenuItem::new("Brancher Claude Code", true, None);
    let quit = MenuItem::new("Quitter", true, None);
    let menu = Menu::new();
    let separator = PredefinedMenuItem::separator;
    let (first, second, third) = (separator(), separator(), separator());
    if let Err(err) =
        menu.append_items(&[&title, &first, &talk, &hush, &forget, &second, &toggle, &hook, &third, &quit])
    {
        log::warn!("menu incomplet : {err}");
    }
    let items = vec![
        (Action::Talk, talk),
        (Action::Hush, hush),
        (Action::Forget, forget),
        (Action::ToggleVisible, toggle),
        (Action::ToggleClaudeHook, hook),
        (Action::Quit, quit),
    ];
    Built { menu, items }
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
