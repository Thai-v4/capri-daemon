//! macOS : les annonces des lecteurs, par `NSDistributedNotificationCenter`, sans aucune
//! permission.
//!
//! Ce que dit `NSDistributedNotificationCenter.h` (SDK de macOS), et qui décide de tout ici :
//! - les notifications distribuées arrivent **sur le thread principal**, par sa boucle (« always
//!   delivered to the main thread », dans un mode « common ») : l'observateur ne vit que là ;
//! - « `NSApplication` automatically suspends distributed notification delivery when the
//!   application is not active » : Capri est une app accessoire, jamais active. Abonnée
//!   ordinairement, elle ne recevrait rien. D'où
//!   `NSNotificationSuspensionBehaviorDeliverImmediately` : « The server will deliver
//!   notifications matching this registration irrespective of whether setSuspended:YES has been
//!   called » ;
//! - « The receiver does not retain observer. Therefore, you should always send
//!   `removeObserver:` [...] before releasing the observer » : le guetteur se désabonne en
//!   disparaissant (`Drop`), avant de lâcher son observateur ;
//! - les données d'une annonce viennent de n'importe quel processus : « treat any data passed in
//!   the notification as untrusted ». La valeur de l'état n'est prise que si c'est une chaîne
//!   (`downcast`), et seules trois valeurs exactes comptent (`capri_core::music`).
//!
//! **Rien d'autre n'est lu de l'annonce** : ni son objet, ni les autres clés de son dictionnaire
//! (titre, artiste, album…), qui n'est jamais parcouru ; jamais de `Debug` ni de `description` de
//! l'annonce ou de son dictionnaire, qui les écriraient. Le nom de l'annonce est comparé aux
//! nôtres, rien de plus.
//!
//! Le lecteur disparu : `+[NSRunningApplication runningApplicationsWithBundleIdentifier:]`
//! (« An array of currently running applications with the given bundle identifier, or an empty
//! array if no apps match »), avec notre identifiant ; seul le nombre d'apps rendues compte.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSRunningApplication;
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationSuspensionBehavior, NSString,
};

use super::{MusicError, Player, PlayerState, Watched};
use capri_core::music::STATE_KEY;

/// Un lecteur guetté, et son dernier état annoncé.
struct Entry {
    player: Player,
    announcement: Retained<NSString>,
    bundle: Retained<NSString>,
    state: Cell<PlayerState>,
    /// En version de débogage : combien d'annonces de ce lecteur sont arrivées.
    #[cfg(debug_assertions)]
    received: Cell<u32>,
}

define_class!(
    // SAFETY: NSObject n'impose rien à ses sous-classes, et `Observer` n'implémente pas `Drop`.
    // Il ne vit que sur le thread principal, où le centre lui livre les annonces (d'après
    // l'en-tête) : ses `Cell` ne sont jamais touchées d'un autre thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CapriAnnouncementObserver"]
    #[ivars = Vec<Entry>]
    struct Observer;

    // SAFETY: NSObjectProtocol n'a rien à implémenter de plus pour un NSObject.
    unsafe impl NSObjectProtocol for Observer {}

    impl Observer {
        /// Une annonce d'un lecteur guetté : seul l'état est lu.
        #[unsafe(method(announced:))]
        fn announced(&self, notification: &NSNotification) {
            let name = notification.name();
            let Some(entry) = self.ivars().iter().find(|entry| *entry.announcement == *name) else { return };
            let key = NSString::from_str(STATE_KEY);
            let value = notification
                .userInfo()
                .and_then(|info| info.objectForKey(&key))
                .and_then(|value| value.downcast::<NSString>().ok())
                .map(|value| value.to_string());
            entry.state.set(PlayerState::announced(value.as_deref()));
            #[cfg(debug_assertions)]
            entry.received.set(entry.received.get().saturating_add(1));
        }
    }
);

pub struct Watch {
    observer: Retained<Observer>,
}

impl Watch {
    /// S'abonne à l'annonce de chaque lecteur de `watched`, avec la livraison immédiate si
    /// `immediately`, sinon comme une app ordinaire (pour comparer, en répétition).
    pub fn new(watched: &[Watched<'_>], immediately: bool) -> Result<Self, MusicError> {
        let mtm = MainThreadMarker::new().ok_or(MusicError::NotMainThread)?;
        let entries = watched
            .iter()
            .map(|watched| Entry {
                player: watched.player,
                announcement: NSString::from_str(watched.announcement),
                bundle: NSString::from_str(watched.bundle),
                state: Cell::new(PlayerState::Unknown),
                #[cfg(debug_assertions)]
                received: Cell::new(0),
            })
            .collect();
        let observer = mtm.alloc::<Observer>().set_ivars(entries);
        // SAFETY: l'initialiseur de NSObject, sur un objet tout juste alloué.
        let observer: Retained<Observer> = unsafe { msg_send![super(observer), init] };
        let center = NSDistributedNotificationCenter::defaultCenter();
        for entry in observer.ivars() {
            let behavior = if immediately {
                NSNotificationSuspensionBehavior::DeliverImmediately
            } else {
                NSNotificationSuspensionBehavior::Coalesce
            };
            // SAFETY: `observer` est un objet valide, non nul, dont la méthode `announced:`,
            // définie plus haut, prend bien une NSNotification ; le sélecteur n'est pas nul ; le
            // nom est une NSString. Le centre ne retient pas l'observateur : `Drop` le désabonne
            // avant de le lâcher, comme l'exige l'en-tête.
            unsafe {
                center.addObserver_selector_name_object_suspensionBehavior(
                    &observer,
                    sel!(announced:),
                    Some(&entry.announcement),
                    None,
                    behavior,
                );
            }
        }
        Ok(Self { observer })
    }

    pub fn states(&self) -> impl Iterator<Item = (Player, PlayerState)> + '_ {
        self.observer.ivars().iter().map(|entry| (entry.player, entry.state.get()))
    }

    pub fn forget_gone(&self) -> usize {
        let mut forgotten = 0;
        for entry in self.observer.ivars().iter().filter(|entry| entry.state.get().plays()) {
            if !running(&entry.bundle) {
                entry.state.set(PlayerState::Unknown);
                forgotten += 1;
            }
        }
        forgotten
    }

    #[cfg(debug_assertions)]
    pub fn received(&self) -> impl Iterator<Item = (Player, u32)> + '_ {
        self.observer.ivars().iter().map(|entry| (entry.player, entry.received.get()))
    }
}

/// Une app de cet identifiant de bundle tourne-t-elle ? Seul le nombre d'apps rendues compte.
fn running(bundle: &NSString) -> bool {
    NSRunningApplication::runningApplicationsWithBundleIdentifier(bundle).count() > 0
}

#[cfg(debug_assertions)]
pub fn is_running(bundle: &str) -> bool {
    running(&NSString::from_str(bundle))
}

impl Drop for Watch {
    fn drop(&mut self) {
        // SAFETY: `observer` est l'objet abonné plus haut, encore vivant (nous le tenons) ; le
        // retirer de toutes ses entrées (nom et objet nuls) est ce que l'en-tête demande avant de
        // le lâcher.
        unsafe {
            NSDistributedNotificationCenter::defaultCenter().removeObserver_name_object(
                &self.observer,
                None,
                None,
            );
        }
    }
}

/// Poste une annonce d'essai, sans livraison immédiate (voir `super::announce`).
#[cfg(debug_assertions)]
pub fn announce(announcement: &str, state: Option<&str>, others: &[(&str, &str)]) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSDictionary;

    let mut pairs: Vec<(Retained<NSString>, Retained<NSString>)> =
        others.iter().map(|(key, value)| (NSString::from_str(key), NSString::from_str(value))).collect();
    if let Some(state) = state {
        pairs.push((NSString::from_str(STATE_KEY), NSString::from_str(state)));
    }
    let keys: Vec<&NSString> = pairs.iter().map(|(key, _)| &**key).collect();
    let values: Vec<&NSString> = pairs.iter().map(|(_, value)| &**value).collect();
    let info = NSDictionary::from_slices(&keys, &values);
    let name = NSString::from_str(announcement);
    // SAFETY: des clés et des valeurs NSString se lisent aussi bien comme des objets quelconques
    // (`AnyObject`), le type que prend l'appel ; le dictionnaire n'est fait que de chaînes, que le
    // centre sait sérialiser en liste de propriétés (l'en-tête l'exige) ; le nom n'est pas nul.
    unsafe {
        let info: &NSDictionary<AnyObject, AnyObject> = info.cast_unchecked();
        NSDistributedNotificationCenter::defaultCenter()
            .postNotificationName_object_userInfo_deliverImmediately(&name, None, Some(info), false);
    }
}

/// Fait tourner la boucle du thread courant pendant `duration`.
#[cfg(debug_assertions)]
pub fn run_for(duration: std::time::Duration) {
    use objc2_foundation::{NSDate, NSRunLoop};

    let until = std::time::Instant::now() + duration;
    let run_loop = NSRunLoop::currentRunLoop();
    while let Some(left) =
        until.checked_duration_since(std::time::Instant::now()).filter(|left| !left.is_zero())
    {
        let started = std::time::Instant::now();
        run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(left.as_secs_f64().min(0.05)));
        // sans source sur la boucle, elle rend la main tout de suite : ne pas tourner à vide
        if started.elapsed() < std::time::Duration::from_millis(1) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

#[cfg(debug_assertions)]
pub fn set_suspended(suspended: bool) {
    NSDistributedNotificationCenter::defaultCenter().setSuspended(suspended);
}

#[cfg(debug_assertions)]
pub fn delivery() -> Option<super::Delivery> {
    use objc2_app_kit::NSApplication;

    let mtm = MainThreadMarker::new()?;
    Some(super::Delivery {
        app_active: NSApplication::sharedApplication(mtm).isActive(),
        suspended: NSDistributedNotificationCenter::defaultCenter().suspended(),
    })
}
