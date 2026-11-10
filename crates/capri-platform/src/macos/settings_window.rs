//! macOS : la fenêtre de réglages, un `NSPanel` à onglets qui affiche un `capri_core::form::Form`.
//!
//! Chaque ligne du formulaire devient un libellé et un contrôle d'AppKit, posés à la main dans une
//! page qui défile (une par onglet). À chaque nouveau formulaire, un contrôle dont l'identifiant
//! et la sorte n'ont pas changé est gardé et mis à jour sur place : le champ où l'on tape garde
//! le clavier et ce qu'il contient. Un champ en cours de saisie n'est jamais réécrit.
//!
//! Le panneau est non activant, comme celui du champ de saisie : il devient fenêtre clé sans
//! faire passer Capri au premier plan, et en se fermant il rend le clavier à la fenêtre d'avant.
//! Sa vue de contenu fait marcher ⌘X, ⌘C, ⌘V, ⌘A, ⌘Z et ⌘W (`edit_keys`). Échap le ferme.
//!
//! Un champ secret n'est jamais rempli par le formulaire ; son contenu n'est lu qu'au moment où
//! l'on presse le bouton qui l'emporte (ou Entrée), puis le champ est vidé. Rien n'en est écrit
//! dans le journal.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use capri_core::form::{Button, Control, Form, FormEvent, Kind, Row, Secret, Section, Tone};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBackingStoreType, NSBorderType, NSButton, NSColor, NSControl,
    NSControlStateValueOff, NSControlStateValueOn, NSControlTextEditingDelegate, NSFont, NSMenuItem, NSPanel,
    NSPopUpButton, NSScrollView, NSSearchField, NSSearchFieldDelegate, NSSecureTextField, NSTabView,
    NSTabViewItem, NSTextAlignment, NSTextField, NSTextFieldDelegate, NSView, NSWindowCollectionBehavior,
    NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::SettingsWindowError;
use crate::macos::edit_keys::EditKeysView;

/// Taille de la fenêtre, en points.
const WIDTH: f64 = 640.0;
const HEIGHT: f64 = 600.0;
/// Marges et colonnes d'une page.
const MARGIN: f64 = 20.0;
const LABEL_WIDTH: f64 = 110.0;
const COLUMN_GAP: f64 = 12.0;
const ROW_GAP: f64 = 8.0;
const FIELD_HEIGHT: f64 = 22.0;
const CHOICE_HEIGHT: f64 = 26.0;
const CHECK_HEIGHT: f64 = 18.0;
const LABEL_HEIGHT: f64 = 17.0;
const BUTTON_GAP: f64 = 8.0;

/// Ce que désigne l'étiquette (`tag`) d'un contrôle.
#[derive(Clone, Debug)]
struct Binding {
    id: String,
    role: Role,
}

#[derive(Clone, Debug)]
enum Role {
    Field,
    /// Entrée presse le bouton `submit`.
    Secret {
        submit: Option<String>,
    },
    Choice,
    Check,
    /// Le bouton emporte le contenu de ce champ secret.
    Button {
        secret: Option<String>,
    },
}

/// Ce que les rappels d'AppKit et la fenêtre partagent. Tout se passe sur le thread principal ;
/// les rappels n'empruntent qu'avec `try_borrow`, au cas où AppKit rappellerait pendant une mise
/// à jour.
#[derive(Default)]
struct Shared {
    events: RefCell<VecDeque<FormEvent>>,
    bindings: RefCell<HashMap<isize, Binding>>,
    /// Les champs de texte (pas les secrets), par identifiant : pour valider une saisie en cours.
    fields: RefCell<HashMap<String, Retained<NSTextField>>>,
    /// Les champs secrets, par identifiant : pour les lire au moment d'un appui, et les vider.
    secrets: RefCell<HashMap<String, Retained<NSTextField>>>,
    /// Les boutons, par identifiant, et le champ secret que chacun emporte.
    buttons: RefCell<HashMap<String, Option<String>>>,
    open: Cell<bool>,
    next_tag: Cell<isize>,
}

impl Shared {
    fn push(&self, event: FormEvent) {
        if let Ok(mut events) = self.events.try_borrow_mut() {
            events.push_back(event);
        }
    }

    fn bind(&self, control: &NSControl, id: &str, role: Role) {
        let tag = self.next_tag.get() + 1;
        self.next_tag.set(tag);
        control.setTag(tag);
        self.bindings.borrow_mut().insert(tag, Binding { id: id.to_owned(), role });
    }

    fn binding(&self, control: &NSControl) -> Option<Binding> {
        self.bindings.try_borrow().ok()?.get(&control.tag()).cloned()
    }

    /// Une saisie en cours est validée avant toute action : l'action porte sur ce qui est affiché.
    fn commit_pending(&self) {
        let fields: Vec<(String, Retained<NSTextField>)> = match self.fields.try_borrow() {
            Ok(fields) => fields.iter().map(|(id, field)| (id.clone(), field.clone())).collect(),
            Err(_) => return,
        };
        for (id, field) in fields {
            if field.currentEditor().is_some() {
                field.validateEditing();
                self.push(FormEvent::Committed { id, value: field.stringValue().to_string() });
            }
        }
    }

    /// Lit le champ secret `id` et le vide.
    fn take_secret(&self, id: &str) -> Option<Secret> {
        let field = self.secrets.try_borrow().ok()?.get(id).cloned()?;
        field.validateEditing();
        let secret = Secret::new(field.stringValue().to_string());
        // l'éditeur de texte du champ garde sa copie tant que la saisie n'est pas abandonnée
        field.abortEditing();
        field.setStringValue(&NSString::from_str(""));
        // et ⌘Z ne doit pas pouvoir remettre la clé dans le champ
        if let Some(undo) = field.window().and_then(|window| window.undoManager()) {
            undo.removeAllActions();
        }
        Some(secret)
    }

    fn press(&self, id: String, secret: Option<String>) {
        self.commit_pending();
        let secret = secret.and_then(|secret| self.take_secret(&secret));
        self.push(FormEvent::Pressed { id, secret });
    }
}

define_class!(
    // SAFETY:
    // - `NSPanel` accepte d'être sous-classé ; les méthodes surchargées n'utilisent que des
    //   méthodes publiques du panneau.
    // - `SettingsPanel` n'implémente pas `Drop`.
    #[unsafe(super(NSPanel))]
    #[name = "CapriSettingsPanel"]
    struct SettingsPanel;

    impl SettingsPanel {
        /// Elle existe pour recevoir le clavier, quand on l'ouvre.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        /// Échap la ferme.
        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            self.performClose(None);
        }
    }
);

define_class!(
    // SAFETY:
    // - `NSView` accepte d'être sous-classé ; la seule méthode surchargée rend une constante.
    // - `PageView` n'implémente pas `Drop`.
    #[unsafe(super(NSView))]
    #[name = "CapriSettingsPage"]
    struct PageView;

    impl PageView {
        /// Les lignes se posent de haut en bas.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

define_class!(
    // SAFETY:
    // - `NSObject` n'a pas d'exigence de sous-classement ; les méthodes ajoutées ne touchent qu'à
    //   nos variables d'instance et aux contrôles qui nous les envoient.
    // - `Target` n'implémente pas `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CapriSettingsTarget"]
    #[ivars = Rc<Shared>]
    struct Target;

    impl Target {
        #[unsafe(method(capriPressed:))]
        fn pressed(&self, sender: Option<&AnyObject>) {
            if let Some(Binding { id, role: Role::Button { secret } }) = self.binding(sender) {
                self.ivars().press(id, secret);
            }
        }

        /// Entrée dans un champ secret : le bouton qu'il désigne.
        #[unsafe(method(capriSecretEntered:))]
        fn secret_entered(&self, sender: Option<&AnyObject>) {
            let Some(Binding { role: Role::Secret { submit: Some(submit) }, .. }) = self.binding(sender) else {
                return;
            };
            let secret = self.ivars().buttons.try_borrow().ok().and_then(|buttons| buttons.get(&submit).cloned());
            if let Some(secret) = secret {
                self.ivars().press(submit, secret);
            }
        }

        /// Un champ de recherche envoie son action à chaque frappe, et quand on le vide par sa
        /// croix, ce que son délégué ne voit pas.
        #[unsafe(method(capriSearched:))]
        fn searched(&self, sender: Option<&AnyObject>) {
            let Some(field) = sender.and_then(|sender| sender.downcast_ref::<NSTextField>()) else { return };
            if let Some(Binding { id, role: Role::Field }) = self.ivars().binding(field) {
                let value = field.stringValue().to_string();
                self.ivars().push(FormEvent::Edited { id, value });
            }
        }

        #[unsafe(method(capriChose:))]
        fn chose(&self, sender: Option<&AnyObject>) {
            let Some(popup) = sender.and_then(|sender| sender.downcast_ref::<NSPopUpButton>()) else { return };
            if let Some(Binding { id, role: Role::Choice }) = self.ivars().binding(popup) {
                self.ivars().commit_pending();
                if let Ok(index) = usize::try_from(popup.indexOfSelectedItem()) {
                    self.ivars().push(FormEvent::Chose { id, index });
                }
            }
        }

        #[unsafe(method(capriToggled:))]
        fn toggled(&self, sender: Option<&AnyObject>) {
            let Some(check) = sender.and_then(|sender| sender.downcast_ref::<NSButton>()) else { return };
            if let Some(Binding { id, role: Role::Check }) = self.ivars().binding(check) {
                self.ivars().commit_pending();
                let checked = check.state() == NSControlStateValueOn;
                self.ivars().push(FormEvent::Toggled { id, checked });
            }
        }
    }

    unsafe impl NSObjectProtocol for Target {}

    unsafe impl NSControlTextEditingDelegate for Target {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, notification: &NSNotification) {
            self.text_changed(notification, false);
        }

        #[unsafe(method(controlTextDidEndEditing:))]
        fn control_text_did_end_editing(&self, notification: &NSNotification) {
            self.text_changed(notification, true);
        }
    }

    unsafe impl NSTextFieldDelegate for Target {}

    unsafe impl NSSearchFieldDelegate for Target {}

    unsafe impl NSWindowDelegate for Target {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            if self.ivars().open.replace(false) {
                self.ivars().push(FormEvent::Closed);
            }
        }
    }
);

impl Target {
    fn create(mtm: MainThreadMarker, shared: Rc<Shared>) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(shared);
        // SAFETY: l'initialiseur de `NSObject`, sans argument.
        unsafe { msg_send![super(this), init] }
    }

    fn binding(&self, sender: Option<&AnyObject>) -> Option<Binding> {
        self.ivars().binding(sender?.downcast_ref::<NSControl>()?)
    }

    /// Le texte d'un champ a changé, ou sa saisie est finie. Jamais un champ secret : ils n'ont
    /// pas de délégué.
    fn text_changed(&self, notification: &NSNotification, ended: bool) {
        let Some(object) = notification.object() else { return };
        let Some(field) = object.downcast_ref::<NSTextField>() else { return };
        let Some(Binding { id, role: Role::Field }) = self.ivars().binding(field) else { return };
        // pendant la saisie, le texte est dans l'éditeur du champ
        let value = match field.currentEditor() {
            Some(editor) => editor.string().to_string(),
            None => field.stringValue().to_string(),
        };
        self.ivars().push(if ended {
            FormEvent::Committed { id, value }
        } else {
            FormEvent::Edited { id, value }
        });
    }
}

/// Le contrôle d'une ligne.
enum Widget {
    /// Texte ou note.
    Label(Retained<NSTextField>),
    Field(Retained<NSTextField>),
    Secret(Retained<NSTextField>),
    Choice(Retained<NSPopUpButton>, Vec<String>),
    Check(Retained<NSButton>),
    Buttons(Vec<(String, Retained<NSButton>)>),
}

impl Widget {
    fn views(&self) -> Vec<&NSView> {
        match self {
            Self::Label(field) | Self::Field(field) | Self::Secret(field) => vec![field],
            Self::Choice(popup, _) => vec![popup],
            Self::Check(check) => vec![check],
            Self::Buttons(buttons) => buttons.iter().map(|(_, button)| -> &NSView { button }).collect(),
        }
    }
}

struct RowView {
    id: String,
    kind: Kind,
    label: Retained<NSTextField>,
    widget: Widget,
}

/// Un onglet : une page qui défile.
struct Page {
    id: String,
    item: Retained<NSTabViewItem>,
    scroll: Retained<NSScrollView>,
    view: Retained<PageView>,
    rows: Vec<RowView>,
}

pub struct SettingsWindow {
    mtm: MainThreadMarker,
    pages: RefCell<Vec<Page>>,
    tabs: Retained<NSTabView>,
    panel: Retained<SettingsPanel>,
    shared: Rc<Shared>,
    target: Retained<Target>,
}

impl SettingsWindow {
    pub fn open(form: &Form) -> Result<Self, SettingsWindowError> {
        let mtm = MainThreadMarker::new().ok_or(SettingsWindowError::NotMainThread)?;
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT));
        // une vraie fenêtre (titre, bouton de fermeture), mais non activante : elle prend le
        // clavier sans faire passer Capri au premier plan
        let style =
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::NonactivatingPanel;
        let panel = mtm.alloc::<SettingsPanel>().set_ivars(());
        // SAFETY: initialiseur désigné de `NSPanel`, avec les types d'arguments de sa signature
        // (NSRect, NSWindowStyleMask, NSBackingStoreType, BOOL), appelé sur le thread principal.
        let panel: Retained<SettingsPanel> = unsafe {
            msg_send![
                super(panel),
                initWithContentRect: frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false,
            ]
        };
        // SAFETY: le panneau est tenu par un `Retained` ; sans ce réglage, AppKit le libérerait
        // une seconde fois à sa fermeture.
        unsafe { panel.setReleasedWhenClosed(false) };
        // un panneau se cache par défaut quand son app n'est pas active : la nôtre ne l'est jamais
        panel.setHidesOnDeactivate(false);
        panel.setTitle(&NSString::from_str(&form.title));
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::MoveToActiveSpace // sur le bureau où l'on est
                | NSWindowCollectionBehavior::FullScreenAuxiliary, // même au-dessus du plein écran
        );
        panel.setAutorecalculatesKeyViewLoop(true);

        let content = EditKeysView::create(mtm, frame, true);
        let tabs = NSTabView::initWithFrame(
            mtm.alloc(),
            NSRect::new(NSPoint::new(12.0, 12.0), NSSize::new(WIDTH - 24.0, HEIGHT - 24.0)),
        );
        tabs.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        content.addSubview(&tabs);
        panel.setContentView(Some(&content));

        let shared = Rc::new(Shared::default());
        let target = Target::create(mtm, Rc::clone(&shared));
        panel.setDelegate(Some(ProtocolObject::from_ref(&*target)));

        let window = Self { mtm, pages: RefCell::new(Vec::new()), tabs, panel, shared, target };
        window.update(form);
        window.panel.center();
        window.show();
        Ok(window)
    }

    pub fn show(&self) {
        self.shared.open.set(true);
        // au premier plan même si Capri n'est pas l'app active, puis le clavier, sans activer l'app
        self.panel.orderFrontRegardless();
        self.panel.makeKeyWindow();
    }

    pub fn is_open(&self) -> bool {
        self.shared.open.get()
    }

    pub fn close(&self) {
        self.panel.close();
    }

    pub fn drain(&self) -> Vec<FormEvent> {
        self.shared.events.borrow_mut().drain(..).collect()
    }

    pub fn has_keyboard(&self) -> bool {
        self.panel.isKeyWindow()
    }

    #[cfg(debug_assertions)]
    pub fn rehearse_typing(&self, id: &str, text: &str) {
        let field = self.shared.secrets.borrow().get(id).or(self.shared.fields.borrow().get(id)).cloned();
        if let Some(field) = field {
            field.setStringValue(&NSString::from_str(text));
        }
    }

    #[cfg(debug_assertions)]
    pub fn rehearse_press(&self, id: &str) {
        let secret = self.shared.buttons.borrow().get(id).cloned();
        if let Some(secret) = secret {
            self.shared.press(id.to_owned(), secret);
        }
    }

    pub fn update(&self, form: &Form) {
        self.panel.setTitle(&NSString::from_str(&form.title));
        let mut pages = self.pages.borrow_mut();
        let same_tabs = pages.len() == form.sections.len()
            && pages.iter().zip(&form.sections).all(|(page, section)| page.id == section.id);
        if !same_tabs {
            for page in pages.drain(..) {
                for row in page.rows {
                    self.forget(&row);
                }
                self.tabs.removeTabViewItem(&page.item);
            }
            for section in &form.sections {
                pages.push(self.new_page(section));
            }
        }
        for (page, section) in pages.iter_mut().zip(&form.sections) {
            page.item.setLabel(&NSString::from_str(&section.title));
            self.update_page(page, section);
        }
    }

    fn new_page(&self, section: &Section) -> Page {
        let mtm = self.mtm;
        // SAFETY: l'identifiant est une NSString, que l'onglet retient.
        let item =
            unsafe { NSTabViewItem::initWithIdentifier(mtm.alloc(), Some(&NSString::from_str(&section.id))) };
        item.setLabel(&NSString::from_str(&section.title));
        let scroll = NSScrollView::initWithFrame(mtm.alloc(), self.tabs.contentRect());
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setBorderType(NSBorderType::NoBorder);
        scroll.setDrawsBackground(false);
        scroll.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        let view = mtm.alloc::<PageView>().set_ivars(());
        // SAFETY: initialiseur désigné de `NSView`, qui prend un NSRect, appelé sur le thread
        // principal.
        let view: Retained<PageView> = unsafe {
            msg_send![super(view), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), scroll.contentSize())]
        };
        scroll.setDocumentView(Some(&view));
        item.setView(Some(&scroll));
        self.tabs.addTabViewItem(&item);
        Page { id: section.id.clone(), item, scroll, view, rows: Vec::new() }
    }

    /// Met la page à jour : les contrôles gardés le sont sur place, les autres sont créés ou
    /// retirés, puis tout est reposé de haut en bas.
    fn update_page(&self, page: &mut Page, section: &Section) {
        let mut old: Vec<RowView> = std::mem::take(&mut page.rows);
        let mut rows = Vec::with_capacity(section.rows.len());
        for row in &section.rows {
            let kind = row.control.kind();
            let kept =
                old.iter().position(|view| view.id == row.id && view.kind == kind).map(|at| old.remove(at));
            let view = match kept {
                Some(mut view) => {
                    self.refresh(&mut view, row);
                    view
                }
                None => self.create(&page.view, row),
            };
            rows.push(view);
        }
        for gone in old {
            self.forget(&gone);
            gone.label.removeFromSuperview();
            for view in gone.widget.views() {
                view.removeFromSuperview();
            }
        }
        page.rows = rows;
        layout(page);
    }

    /// Oublie les liens d'une ligne qui disparaît.
    fn forget(&self, row: &RowView) {
        let tags: Vec<isize> = match &row.widget {
            Widget::Label(_) => Vec::new(),
            Widget::Field(field) | Widget::Secret(field) => vec![field.tag()],
            Widget::Choice(popup, _) => vec![popup.tag()],
            Widget::Check(check) => vec![check.tag()],
            Widget::Buttons(buttons) => buttons.iter().map(|(_, button)| button.tag()).collect(),
        };
        let mut bindings = self.shared.bindings.borrow_mut();
        for tag in tags {
            bindings.remove(&tag);
        }
        drop(bindings);
        self.shared.fields.borrow_mut().remove(&row.id);
        self.shared.secrets.borrow_mut().remove(&row.id);
        if let Widget::Buttons(buttons) = &row.widget {
            let mut known = self.shared.buttons.borrow_mut();
            for (id, _) in buttons {
                known.remove(id);
            }
        }
    }

    fn target(&self) -> &AnyObject {
        &self.target
    }

    fn create(&self, page: &PageView, row: &Row) -> RowView {
        let mtm = self.mtm;
        let label = NSTextField::labelWithString(&NSString::from_str(&row.label), mtm);
        label.setAlignment(NSTextAlignment::Right);
        label.setTextColor(Some(&NSColor::labelColor()));
        page.addSubview(&label);
        let widget = match &row.control {
            Control::Text { .. } | Control::Note { .. } => {
                let text = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
                text.setSelectable(true);
                Widget::Label(text)
            }
            Control::Field { search, .. } => {
                let field: Retained<NSTextField> = if *search {
                    let search = NSSearchField::new(mtm);
                    // SAFETY: la cible survit au champ (la fenêtre la retient), et AppKit ne garde
                    // qu'une référence faible sur son délégué et sa cible ; le sélecteur désigne
                    // `capriSearched:`, définie plus haut, qui prend l'expéditeur en argument.
                    unsafe {
                        search.setDelegate(Some(ProtocolObject::from_ref(&*self.target)));
                        search.setTarget(Some(self.target()));
                        search.setAction(Some(sel!(capriSearched:)));
                    }
                    Retained::into_super(search)
                } else {
                    let field = NSTextField::textFieldWithString(&NSString::from_str(""), mtm);
                    // SAFETY: comme ci-dessus.
                    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&*self.target))) };
                    field
                };
                self.shared.bind(&field, &row.id, Role::Field);
                self.shared.fields.borrow_mut().insert(row.id.clone(), field.clone());
                Widget::Field(field)
            }
            Control::Secret { submit, .. } => {
                let secret: Retained<NSTextField> = Retained::into_super(NSSecureTextField::new(mtm));
                // l'action ne part que sur Entrée, pas quand le champ perd le clavier
                if let Some(cell) = secret.cell() {
                    cell.setSendsActionOnEndEditing(false);
                }
                // SAFETY: la cible survit au champ ; le sélecteur désigne `capriSecretEntered:`,
                // définie plus haut, qui prend l'expéditeur en argument comme toute action.
                unsafe {
                    secret.setTarget(Some(self.target()));
                    secret.setAction(Some(sel!(capriSecretEntered:)));
                }
                self.shared.bind(&secret, &row.id, Role::Secret { submit: submit.clone() });
                self.shared.secrets.borrow_mut().insert(row.id.clone(), secret.clone());
                Widget::Secret(secret)
            }
            Control::Choice { .. } => {
                let popup = NSPopUpButton::initWithFrame_pullsDown(mtm.alloc(), NSRect::ZERO, false);
                popup.setAutoenablesItems(false);
                // SAFETY: la cible survit à la liste ; le sélecteur désigne `capriChose:`.
                unsafe {
                    popup.setTarget(Some(self.target()));
                    popup.setAction(Some(sel!(capriChose:)));
                }
                self.shared.bind(&popup, &row.id, Role::Choice);
                Widget::Choice(popup, Vec::new())
            }
            Control::Check { title, .. } => {
                // SAFETY: la cible survit à la case ; le sélecteur désigne `capriToggled:`.
                let check = unsafe {
                    NSButton::checkboxWithTitle_target_action(
                        &NSString::from_str(title),
                        Some(self.target()),
                        Some(sel!(capriToggled:)),
                        mtm,
                    )
                };
                self.shared.bind(&check, &row.id, Role::Check);
                Widget::Check(check)
            }
            Control::Buttons(buttons) => {
                Widget::Buttons(buttons.iter().map(|button| self.new_button(button)).collect())
            }
        };
        for view in widget.views() {
            page.addSubview(view);
        }
        let mut view = RowView { id: row.id.clone(), kind: row.control.kind(), label, widget };
        self.refresh(&mut view, row);
        view
    }

    fn new_button(&self, button: &Button) -> (String, Retained<NSButton>) {
        // SAFETY: la cible survit au bouton ; le sélecteur désigne `capriPressed:`.
        let view = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(&button.title),
                Some(self.target()),
                Some(sel!(capriPressed:)),
                self.mtm,
            )
        };
        self.shared.bind(&view, &button.id, Role::Button { secret: button.secret.clone() });
        (button.id.clone(), view)
    }

    /// Accorde les contrôles d'une ligne à sa description.
    fn refresh(&self, view: &mut RowView, row: &Row) {
        set_text(&view.label, &row.label);
        match (&mut view.widget, &row.control) {
            (Widget::Label(label), Control::Text { text }) => {
                set_text(label, text);
                label.setFont(Some(&NSFont::systemFontOfSize(NSFont::systemFontSize())));
                label.setTextColor(Some(&NSColor::labelColor()));
            }
            (Widget::Label(label), Control::Note { text, tone }) => {
                set_text(label, text);
                label.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
                label.setTextColor(Some(&tone_color(*tone)));
            }
            (Widget::Field(field), Control::Field { value, placeholder, enabled, .. }) => {
                // ce qu'on est en train de taper n'est jamais réécrit
                if field.currentEditor().is_none() {
                    set_text(field, value);
                }
                field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
                field.setEnabled(*enabled);
            }
            (Widget::Secret(secret), Control::Secret { placeholder, enabled, submit }) => {
                secret.setPlaceholderString(Some(&NSString::from_str(placeholder)));
                secret.setEnabled(*enabled);
                if let Some(binding) = self.shared.bindings.borrow_mut().get_mut(&secret.tag()) {
                    binding.role = Role::Secret { submit: submit.clone() };
                }
            }
            (Widget::Choice(popup, shown), Control::Choice { options, selected, enabled }) => {
                if shown != options {
                    popup.removeAllItems();
                    if let Some(menu) = popup.menu() {
                        for option in options {
                            // un menu, et pas `addItemWithTitle:` : deux options de même nom
                            // restent deux options
                            // SAFETY: un élément sans action, avec un titre et un équivalent clavier
                            // vides valides ; le menu le retient.
                            let item = unsafe {
                                NSMenuItem::initWithTitle_action_keyEquivalent(
                                    self.mtm.alloc(),
                                    &NSString::from_str(option),
                                    None,
                                    &NSString::from_str(""),
                                )
                            };
                            menu.addItem(&item);
                        }
                    }
                    shown.clone_from(options);
                }
                if let Ok(selected) = isize::try_from(*selected)
                    && selected < popup.numberOfItems()
                    && popup.indexOfSelectedItem() != selected
                {
                    popup.selectItemAtIndex(selected);
                }
                popup.setEnabled(*enabled && !options.is_empty());
            }
            (Widget::Check(check), Control::Check { title, checked, enabled }) => {
                if check.title().to_string() != *title {
                    check.setTitle(&NSString::from_str(title));
                }
                check.setState(if *checked { NSControlStateValueOn } else { NSControlStateValueOff });
                check.setEnabled(*enabled);
            }
            (Widget::Buttons(views), Control::Buttons(buttons)) => {
                for ((id, view), button) in views.iter_mut().zip(buttons) {
                    if *id != button.id {
                        // un autre bouton à la même place : il change de lien
                        self.shared.buttons.borrow_mut().remove(id);
                        self.shared.bindings.borrow_mut().remove(&view.tag());
                        self.shared.bind(view, &button.id, Role::Button { secret: button.secret.clone() });
                        id.clone_from(&button.id);
                    } else if let Some(binding) = self.shared.bindings.borrow_mut().get_mut(&view.tag()) {
                        binding.role = Role::Button { secret: button.secret.clone() };
                    }
                    self.shared.buttons.borrow_mut().insert(button.id.clone(), button.secret.clone());
                    if view.title().to_string() != button.title {
                        view.setTitle(&NSString::from_str(&button.title));
                    }
                    view.setEnabled(button.enabled);
                }
            }
            // la sorte ne change jamais pour une même vue : `update_page` en crée une autre
            _ => {}
        }
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        // plus aucun rappel vers nous, puis la fenêtre disparaît
        self.panel.setDelegate(None);
        self.shared.open.set(false);
        self.panel.close();
    }
}

/// Change le texte d'un champ s'il est différent : réécrire le même texte ferait perdre la
/// sélection.
fn set_text(field: &NSTextField, text: &str) {
    if field.stringValue().to_string() != text {
        field.setStringValue(&NSString::from_str(text));
    }
}

/// La couleur d'une note : celles du système, qui suivent le thème clair ou sombre.
fn tone_color(tone: Tone) -> Retained<NSColor> {
    match tone {
        Tone::Quiet | Tone::Busy => NSColor::secondaryLabelColor(),
        Tone::Good => NSColor::systemGreenColor(),
        Tone::Warning => NSColor::systemOrangeColor(),
        Tone::Bad => NSColor::systemRedColor(),
    }
}

/// Pose les lignes de la page de haut en bas : le libellé à gauche, le contrôle à droite.
fn layout(page: &Page) {
    let width = page.scroll.contentSize().width;
    let control_x = MARGIN + LABEL_WIDTH + COLUMN_GAP;
    let control_width = (width - control_x - MARGIN).max(100.0);
    let mut y = MARGIN;
    for row in &page.rows {
        let height = match &row.widget {
            Widget::Label(text) => {
                let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(control_width, 10_000.0));
                let height =
                    text.cell().map_or(LABEL_HEIGHT, |cell| cell.cellSizeForBounds(bounds).height.ceil());
                let height = if text.stringValue().length() == 0 { 0.0 } else { height };
                text.setFrame(NSRect::new(NSPoint::new(control_x, y), NSSize::new(control_width, height)));
                height
            }
            Widget::Field(field) | Widget::Secret(field) => {
                field.setFrame(NSRect::new(
                    NSPoint::new(control_x, y),
                    NSSize::new(control_width, FIELD_HEIGHT),
                ));
                FIELD_HEIGHT
            }
            Widget::Choice(popup, _) => {
                popup.setFrame(NSRect::new(
                    NSPoint::new(control_x - 3.0, y),
                    NSSize::new(control_width + 6.0, CHOICE_HEIGHT),
                ));
                CHOICE_HEIGHT
            }
            Widget::Check(check) => {
                check.setFrame(NSRect::new(
                    NSPoint::new(control_x, y),
                    NSSize::new(control_width, CHECK_HEIGHT),
                ));
                CHECK_HEIGHT
            }
            Widget::Buttons(buttons) => {
                let mut x = control_x - 6.0;
                let mut height: f64 = 0.0;
                for (_, button) in buttons {
                    button.sizeToFit();
                    let size = button.frame().size;
                    button.setFrame(NSRect::new(NSPoint::new(x, y), size));
                    x += size.width + BUTTON_GAP;
                    height = height.max(size.height);
                }
                height
            }
        };
        // le libellé, aligné sur la première ligne du contrôle
        let label_y = match row.widget {
            Widget::Label(_) => y,
            _ => y + ((height.min(CHOICE_HEIGHT) - LABEL_HEIGHT) / 2.0).max(0.0),
        };
        row.label
            .setFrame(NSRect::new(NSPoint::new(MARGIN, label_y), NSSize::new(LABEL_WIDTH, LABEL_HEIGHT)));
        if height > 0.0 {
            y += height + ROW_GAP;
        }
    }
    let content = page.scroll.contentSize();
    page.view
        .setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, (y + MARGIN).max(content.height))));
}
