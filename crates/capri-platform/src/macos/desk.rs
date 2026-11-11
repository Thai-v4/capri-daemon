//! macOS : la liste des fenêtres de CoreGraphics, sans aucune permission.
//!
//! Ce qu'en dit `CGWindow.h` (SDK de macOS), et pourquoi aucune permission n'est en jeu :
//! - `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID)` rend « toutes
//!   les fenêtres à l'écran de cette session, de la plus en avant à la plus en arrière », un
//!   dictionnaire par fenêtre ; c'est une fonction « Copy » : le tableau nous appartient et doit
//!   être libéré (ici par `CFRetained`, à la sortie). `kCGWindowListExcludeDesktopElements` écarte
//!   le bureau lui-même (son image, ses icônes) ;
//! - les seules clés lues sont dans la liste des clés présentes dans **tout** dictionnaire de
//!   fenêtre : `kCGWindowNumber` (entier 32 bits), `kCGWindowLayer` (entier 32 bits),
//!   `kCGWindowBounds` (un rectangle, en points, « origine en haut à gauche de l'écran
//!   principal »), `kCGWindowAlpha` (« 1,0 : opaque ; 0,0 : invisible »), `kCGWindowOwnerPID`
//!   (entier 32 bits, comparé au nôtre puis oublié) ; plus `kCGWindowIsOnscreen`, facultative
//!   (« absente : la fenêtre n'est pas à l'écran ») ;
//! - le titre (`kCGWindowName`) et le nom de l'app (`kCGWindowOwnerName`) sont des clés
//!   facultatives (« si présente ») : macOS ne donne le titre des fenêtres des autres apps qu'aux
//!   apps qui ont l'Enregistrement de l'écran. Ces deux clés ne sont jamais demandées ici ;
//! - la seule permission dont parle l'en-tête est celle de capturer l'écran
//!   (`CGPreflightScreenCaptureAccess`, `CGRequestScreenCaptureAccess`), pour les images des
//!   fenêtres (`CGWindowListCreateImage`, obsolète) : Capri ne capture rien et ne la demande pas.
//!   L'Accessibilité n'y apparaît pas.

use capri_core::geom::{Rect, Vec2};
use capri_core::perch::DeskWindow;
use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect,
};
use objc2_core_graphics::{
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID,
    kCGWindowAlpha, kCGWindowBounds, kCGWindowIsOnscreen, kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
};

use super::DeskError;

/// Les clés lues, et rien d'autre.
struct Keys {
    number: &'static CFString,
    layer: &'static CFString,
    bounds: &'static CFString,
    alpha: &'static CFString,
    owner: &'static CFString,
    on_screen: &'static CFString,
}

pub fn windows() -> Result<Vec<DeskWindow>, DeskError> {
    let options = CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    // « Copy » : le tableau nous appartient ; `CFRetained` le libère quand `list` disparaît.
    let list: CFRetained<CFArray> =
        CGWindowListCopyWindowInfo(options, kCGNullWindowID).ok_or(DeskError::Refused)?;
    // SAFETY: tout élément d'un CFArray est un objet CoreFoundation : le voir comme un `CFType`
    // est toujours valable ; chaque élément est ensuite vérifié (`downcast_ref`) avant usage.
    let list: &CFArray<CFType> = unsafe { list.cast_unchecked() };
    // SAFETY: constantes exportées par CoreGraphics, initialisées au chargement du framework et
    // valables toute la vie du processus ; on ne fait que les lire.
    let keys = unsafe {
        Keys {
            number: kCGWindowNumber,
            layer: kCGWindowLayer,
            bounds: kCGWindowBounds,
            alpha: kCGWindowAlpha,
            owner: kCGWindowOwnerPID,
            on_screen: kCGWindowIsOnscreen,
        }
    };
    let me = i64::from(std::process::id());
    let mut windows = Vec::with_capacity(list.len());
    for item in list.iter() {
        let Some(info) = item.downcast_ref::<CFDictionary>() else { continue };
        // SAFETY: les clés d'un dictionnaire de fenêtre sont des CFString (CGWindow.h) et ses
        // valeurs des objets CoreFoundation ; chaque valeur est vérifiée (`downcast`) avant usage.
        let info: &CFDictionary<CFString, CFType> = unsafe { info.cast_unchecked() };
        if let Some(window) = describe(info, &keys, me) {
            windows.push(window);
        }
    }
    Ok(windows)
}

pub fn mouse_down() -> bool {
    // l'état des boutons pour tout le système, sans permission : bit 0 gauche, bit 1 droit…
    objc2_app_kit::NSEvent::pressedMouseButtons() != 0
}

/// Une fenêtre, d'après son dictionnaire : `None` s'il manque une clé que l'en-tête promet, ou si
/// elle n'est pas à l'écran.
fn describe(info: &CFDictionary<CFString, CFType>, keys: &Keys, me: i64) -> Option<DeskWindow> {
    let number = |key: &CFString| info.get(key)?.downcast::<CFNumber>().ok();
    let on_screen = info.get(keys.on_screen).and_then(|value| value.downcast::<CFBoolean>().ok());
    if !on_screen.is_some_and(|value| value.as_bool()) {
        return None;
    }
    let id = u64::try_from(number(keys.number)?.as_i64()?).ok()?;
    let layer = number(keys.layer)?.as_i64()?;
    let opacity = number(keys.alpha)?.as_f64()?;
    let owner = number(keys.owner)?.as_i64()?;
    let bounds = info.get(keys.bounds)?.downcast::<CFDictionary>().ok()?;
    let mut rect = CGRect::ZERO;
    // SAFETY: `bounds` est un vrai CFDictionary (vérifié ci-dessus), vivant pendant l'appel ;
    // `rect` est un CGRect à nous, où la fonction écrit. Elle rend `false` si le dictionnaire ne
    // décrit pas un rectangle.
    if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &mut rect) } {
        return None;
    }
    Some(DeskWindow {
        id,
        rect: Rect::new(
            Vec2::new(rect.origin.x as f32, rect.origin.y as f32),
            Vec2::new(rect.size.width as f32, rect.size.height as f32),
        ),
        ordinary: layer == 0,
        opacity: opacity as f32,
        ours: owner == me,
    })
}
