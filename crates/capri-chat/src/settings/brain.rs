//! La rubrique « Cerveau » : fournisseur, adresse, clé, modèle, essai.
//!
//! Les mêmes règles que `capri chat` :
//! - l'adresse passe par [`Endpoint::parse`] ; celle d'un fournisseur est préremplie (celle du
//!   cerveau choisi s'il est de ce fournisseur, sinon son adresse habituelle : [`key_endpoint`]) ;
//! - la phrase qui dit où part ce qu'on dit à Capri ([`destination`]) s'affiche dès qu'un
//!   fournisseur est choisi dans la liste, avant qu'il devienne son cerveau ;
//! - la clé est liée à l'adresse affichée ([`KeyAccount`]) ; le champ secret n'est jamais
//!   prérempli, la fenêtre dit seulement s'il y en a une, et une clé qui ne pourrait partir qu'en
//!   clair n'est pas rangée ;
//! - aucun nom de modèle n'est écrit ici : la liste vient du serveur, bornée et cherchable ;
//! - personne n'est contacté sans la clé qu'il exige, et la fenêtre ne demande d'elle-même la
//!   liste des modèles qu'à un serveur de cette machine : ailleurs, il faut appuyer sur le
//!   bouton ;
//! - le cerveau choisi est écrit dans `chat.json` : il vaut dès la phrase suivante.

use capri_core::form::{Button, Control, FormEvent, Row, Tone};

use super::{Effect, Environment, Forgotten, LegacyKey, Outcome, Rubric};
use crate::brain::{Brain, KeyNeed, MAX_LISTED_MODELS, Provider, has_model, search_models};
use crate::endpoint::{Endpoint, Reach};
use crate::key::{ApiKey, KeyAccount, key_endpoint};
use crate::phrases::{
    NO_BRAIN, NO_MORE_BRAIN, TEST_QUESTION, destination, goes_on_internet, how_to_get, is_ollama_cloud_model,
    key_forgotten, key_stored, legacy_key_removed, no_key_in_clear, not_a_key, of_provider, server,
    thinks_now_with, thinks_with,
};

pub const ID: &str = "brain";

// Les lignes et les boutons de la rubrique.
pub const NOW: &str = "brain.now";
pub const PROVIDER: &str = "brain.provider";
pub const URL: &str = "brain.url";
pub const URL_NOTE: &str = "brain.url.note";
pub const WHERE: &str = "brain.where";
pub const KEY_STATUS: &str = "brain.key.status";
pub const KEY: &str = "brain.key";
pub const KEY_ACTIONS: &str = "brain.key.actions";
pub const KEY_SAVE: &str = "brain.key.save";
pub const KEY_FORGET: &str = "brain.key.forget";
pub const KEY_NOTE: &str = "brain.key.note";
pub const SEARCH: &str = "brain.models.search";
pub const MODEL: &str = "brain.model";
pub const MODELS_NOTE: &str = "brain.models.note";
pub const MODELS_ACTIONS: &str = "brain.models.actions";
pub const MODELS_ASK: &str = "brain.models.ask";
pub const TALK_ACTIONS: &str = "brain.talk.actions";
pub const TALK: &str = "brain.talk";
pub const TALK_TEXT: &str = "brain.talk.text";
pub const TALK_NOTE: &str = "brain.talk.note";
pub const USE_ACTIONS: &str = "brain.use.actions";
pub const USE: &str = "brain.use";
pub const DROP: &str = "brain.drop";
pub const USE_NOTE: &str = "brain.use.note";

/// Ce que l'on sait de la clé de l'adresse affichée.
#[derive(Clone, Debug, PartialEq, Eq)]
enum KeyState {
    /// Pas encore demandé au trousseau.
    Unknown,
    Checking,
    /// `legacy` : une ancienne clé, rangée sans adresse, est là aussi.
    Present {
        legacy: bool,
    },
    Absent {
        legacy: bool,
    },
    /// Le trousseau n'a pas répondu.
    Unreadable(String),
}

/// La liste des modèles du serveur affiché.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Models {
    NotAsked,
    Asking,
    Listed(Vec<String>),
    Failed(String),
}

/// L'essai en cours ou le dernier.
#[derive(Clone, Debug, PartialEq)]
enum Trial {
    None,
    Talking { brain: Brain, text: String },
    Answered { text: String, seconds: f32 },
    Failed { text: String, problem: String },
}

pub struct BrainRubric {
    keychain: Option<String>,
    tls: bool,
    /// Les réglages ont-ils été lus ?
    loaded: bool,
    /// Le cerveau de `chat.json`.
    chosen: Option<Brain>,
    /// Pourquoi `chat.json` est illisible, s'il l'est.
    unreadable: Option<String>,
    /// Ce que montre la fenêtre. `None` : aucun cerveau.
    provider: Option<Provider>,
    /// L'adresse telle qu'elle est tapée.
    url: String,
    /// L'adresse validée (Entrée, ou le clavier passé ailleurs) et ce qu'on en a lu.
    committed: String,
    endpoint: Result<Endpoint, String>,
    key: KeyState,
    key_note: Option<(Tone, String)>,
    models: Models,
    search: String,
    model: Option<String>,
    trial: Trial,
    use_note: Option<(Tone, String)>,
}

impl BrainRubric {
    pub fn new(environment: &Environment) -> Self {
        Self {
            keychain: environment.keychain.clone(),
            tls: environment.tls,
            loaded: false,
            chosen: None,
            unreadable: None,
            provider: None,
            url: String::new(),
            committed: String::new(),
            endpoint: Err(String::new()),
            key: KeyState::Unknown,
            key_note: None,
            models: Models::NotAsked,
            search: String::new(),
            model: None,
            trial: Trial::None,
            use_note: None,
        }
    }

    /// Montre ce fournisseur, à l'adresse que prendrait `capri chat key` : celle du cerveau
    /// choisi s'il est de ce fournisseur, sinon son adresse habituelle. Rend les effets à lancer.
    fn show_provider(&mut self, provider: Option<Provider>) -> Vec<Effect> {
        self.provider = provider;
        self.use_note = None;
        let url = provider
            .and_then(|provider| key_endpoint(provider, None, self.chosen.as_ref()))
            .map(|endpoint| endpoint.url(""))
            .unwrap_or_default();
        self.url = url.clone();
        self.show_address(url)
    }

    /// Prend l'adresse validée ; si elle change le serveur joint, tout ce qui en dépend est à
    /// redemander. Rend les effets à lancer.
    fn show_address(&mut self, url: String) -> Vec<Effect> {
        let Some(provider) = self.provider else { return Vec::new() };
        let endpoint = if url.trim().is_empty() {
            Err(match provider.default_url() {
                Some(_) => "il faut une adresse".to_owned(),
                None => "un serveur personnalisé n'a pas d'adresse habituelle : donne la sienne".to_owned(),
            })
        } else {
            Endpoint::parse(url.trim()).map_err(|err| format!("adresse refusée : {err}"))
        };
        self.committed = url;
        self.endpoint = endpoint;
        self.key = KeyState::Unknown;
        self.key_note = None;
        self.models = Models::NotAsked;
        self.search.clear();
        self.trial = Trial::None;
        // le modèle du cerveau choisi reste proposé tant qu'on regarde le même serveur
        self.model = self
            .chosen
            .as_ref()
            .filter(|chosen| chosen.provider() == provider && Ok(chosen.endpoint()) == self.endpoint.as_ref())
            .map(|chosen| chosen.model().to_owned());
        let mut effects = Vec::new();
        if let Some(account) = self.account()
            && self.keychain.is_some()
        {
            self.key = KeyState::Checking;
            effects.push(Effect::CheckKey(account));
        }
        effects.extend(self.list_if_local());
        effects
    }

    /// La liste des modèles, demandée d'elle-même seulement à un serveur de cette machine, et
    /// seulement si rien ne manque pour le joindre.
    fn list_if_local(&mut self) -> Option<Effect> {
        let local = self.endpoint.as_ref().is_ok_and(|endpoint| endpoint.reach() == Reach::ThisMachine);
        if local && self.models == Models::NotAsked && self.cannot_contact().is_none() {
            self.ask_models()
        } else {
            None
        }
    }

    fn ask_models(&mut self) -> Option<Effect> {
        let (provider, endpoint) = (self.provider?, self.endpoint.clone().ok()?);
        self.models = Models::Asking;
        Some(Effect::ListModels { provider, endpoint, chosen: self.chosen.clone() })
    }

    /// Le compte de la clé de l'adresse affichée, si ce fournisseur en prend une.
    fn account(&self) -> Option<KeyAccount> {
        let provider = self.provider.filter(|provider| provider.key_need() != KeyNeed::None)?;
        Some(KeyAccount::new(provider, self.endpoint.as_ref().ok()?))
    }

    /// Pourquoi le serveur affiché ne peut pas être contacté maintenant, s'il y a une raison :
    /// les règles de `capri chat` (aucun contact sans la clé exigée, aucune clé en clair).
    fn cannot_contact(&self) -> Option<String> {
        let provider = self.provider?;
        let endpoint = match &self.endpoint {
            Ok(endpoint) => endpoint,
            Err(problem) => return Some(problem.clone()),
        };
        if endpoint.secure && !self.tls {
            return Some("Je ne sais pas me connecter en https sur ce système.".to_owned());
        }
        let address = KeyAccount::new(provider, endpoint).address().to_owned();
        match (provider.key_need(), &self.key, &self.keychain) {
            (KeyNeed::None, ..) => None,
            (KeyNeed::Required, _, None) => Some(format!(
                "{} demande une clé, et ce système n'a pas de trousseau où la ranger.",
                provider.name()
            )),
            (_, KeyState::Unknown | KeyState::Checking, Some(_)) => {
                Some("Un instant : je regarde s'il y a une clé pour cette adresse.".to_owned())
            }
            (_, KeyState::Present { .. }, _) if !endpoint.may_carry_secret() => Some(format!(
                "La clé {} rangée pour {address} ne partirait pas en clair : donne une adresse en https, ou retire \
                 la clé.",
                of_provider(provider)
            )),
            (KeyNeed::Required, KeyState::Absent { .. }, _) => Some(format!(
                "{} demande une clé, et il n'y en a pas pour {address} : enregistre-la d'abord.",
                provider.name()
            )),
            (KeyNeed::Required, KeyState::Unreadable(problem), _) => Some(problem.clone()),
            _ => None,
        }
    }

    /// Le cerveau que forment le fournisseur, l'adresse et le modèle affichés.
    fn candidate(&self) -> Option<Brain> {
        let endpoint = self.endpoint.as_ref().ok()?;
        Brain::new(self.provider?, Some(&endpoint.url("")), self.model.as_deref()?).ok()
    }

    /// Les modèles de la liste déroulante, dans l'ordre ; `None` : la ligne d'invite.
    fn model_options(&self) -> Vec<Option<String>> {
        let mut shown: Vec<Option<String>> = match &self.models {
            Models::Listed(models) => search_models(models, &self.search)
                .into_iter()
                .take(MAX_LISTED_MODELS)
                .map(|model| Some(model.clone()))
                .collect(),
            _ => Vec::new(),
        };
        match &self.model {
            Some(model) if !shown.contains(&Some(model.clone())) => shown.insert(0, Some(model.clone())),
            Some(_) => {}
            None => shown.insert(0, None),
        }
        shown
    }

    fn model_label(&self, model: Option<&str>) -> String {
        match model {
            Some(model) if self.provider == Some(Provider::Ollama) && is_ollama_cloud_model(model) => {
                format!("{model} (relayé vers les serveurs d'Ollama)")
            }
            Some(model) => model.to_owned(),
            None if matches!(&self.models, Models::Listed(models) if !models.is_empty()) => {
                "Choisis un modèle…".to_owned()
            }
            None => "Aucun modèle pour l'instant".to_owned(),
        }
    }

    fn now_row(&self) -> Row {
        let (tone, text) = match (&self.unreadable, &self.chosen) {
            _ if !self.loaded => (Tone::Busy, "Lecture de ses réglages…".to_owned()),
            (Some(problem), _) => (Tone::Bad, format!("Ses réglages sont illisibles : {problem}")),
            (None, None) => (Tone::Quiet, NO_BRAIN.to_owned()),
            (None, Some(brain)) => (Tone::Good, thinks_with(brain)),
        };
        Row::new(NOW, "", Control::Note { text, tone })
    }

    fn provider_row(&self) -> Row {
        let options = std::iter::once("Aucun : elle répète ce qu'on lui dit".to_owned())
            .chain(Provider::ALL.into_iter().map(provider_label))
            .collect();
        let selected = self
            .provider
            .and_then(|provider| Provider::ALL.iter().position(|&known| known == provider))
            .map_or(0, |index| index + 1);
        Row::new(PROVIDER, "Fournisseur", Control::Choice { options, selected, enabled: self.loaded })
    }

    fn address_rows(&self, provider: Provider, rows: &mut Vec<Row>) {
        let placeholder = provider.default_url().unwrap_or("https://ton-serveur.example/v1").to_owned();
        rows.push(Row::new(
            URL,
            "Adresse",
            Control::Field { value: self.url.clone(), placeholder, enabled: true, search: false },
        ));
        if let Err(problem) = &self.endpoint {
            rows.push(Row::new(URL_NOTE, "", Control::Note { text: problem.clone(), tone: Tone::Bad }));
        }
        let (tone, text) = match &self.endpoint {
            _ if self.url.trim() != self.committed.trim() => (
                Tone::Quiet,
                "Valide l'adresse (Entrée) pour savoir où partira ce que tu lui dis.".to_owned(),
            ),
            Ok(endpoint) => {
                let model = self.model.as_deref();
                let tone =
                    if goes_on_internet(provider, endpoint, model) { Tone::Warning } else { Tone::Quiet };
                (tone, destination(provider, endpoint, model))
            }
            Err(_) => return,
        };
        rows.push(Row::new(WHERE, "", Control::Note { text, tone }));
    }

    fn key_rows(&self, provider: Provider, rows: &mut Vec<Row>) {
        if provider.key_need() == KeyNeed::None {
            return;
        }
        let endpoint = self.endpoint.as_ref().ok();
        let address = endpoint.map(|endpoint| KeyAccount::new(provider, endpoint).address().to_owned());
        let in_clear = endpoint.is_some_and(|endpoint| !endpoint.may_carry_secret());
        let legacy =
            matches!(self.key, KeyState::Present { legacy: true } | KeyState::Absent { legacy: true });
        let (tone, mut text) = match (&self.keychain, &address, &self.key) {
            (None, ..) => (
                Tone::Bad,
                "Pas de trousseau sur ce système : Capri ne sait pas y ranger de clé, et n'en écrit jamais ailleurs."
                    .to_owned(),
            ),
            (_, None, _) => (Tone::Quiet, "Donne d'abord une adresse valable.".to_owned()),
            (_, Some(address), KeyState::Present { .. }) if in_clear => (
                Tone::Warning,
                format!(
                    "Une clé est rangée pour {address}, mais elle ne partira pas en clair : donne une adresse en \
                     https, ou retire la clé."
                ),
            ),
            (_, Some(address), _) if in_clear => {
                (Tone::Quiet, format!("{} Il faut une adresse en https.", capitalized(&no_key_in_clear(address))))
            }
            (_, _, KeyState::Unknown | KeyState::Checking) => {
                (Tone::Busy, "Je regarde s'il y a une clé pour cette adresse…".to_owned())
            }
            (Some(keychain), Some(address), KeyState::Present { .. }) => (
                Tone::Good,
                format!("Une clé est rangée pour {address}, dans {keychain}. Elle ne s'affiche jamais."),
            ),
            (_, _, KeyState::Absent { .. }) if provider.key_need() == KeyNeed::Required => (
                Tone::Warning,
                format!("Aucune clé pour cette adresse : {} en demande une.", provider.name()),
            ),
            (_, _, KeyState::Absent { .. }) => (
                Tone::Quiet,
                "Aucune clé pour cette adresse (il n'en faut une que si le serveur en demande).".to_owned(),
            ),
            (_, _, KeyState::Unreadable(problem)) => (Tone::Bad, problem.clone()),
        };
        if legacy {
            text.push_str(&format!(
                " Une ancienne clé {}, rangée sans adresse par une version précédente de Capri, n'est jamais \
                 envoyée : en enregistrer une la retire.",
                of_provider(provider)
            ));
        }
        rows.push(Row::new(KEY_STATUS, "Clé d'API", Control::Note { text, tone }));

        let can_store = self.keychain.is_some() && endpoint.is_some_and(Endpoint::may_carry_secret);
        rows.push(Row::new(
            KEY,
            "",
            Control::Secret {
                placeholder: "Colle la clé ici : elle ne s'affiche pas".to_owned(),
                enabled: can_store,
                submit: Some(KEY_SAVE.to_owned()),
            },
        ));
        let mut save = Button::new(KEY_SAVE, "Enregistrer la clé", can_store);
        save.secret = Some(KEY.to_owned());
        let can_forget = self.keychain.is_some()
            && address.is_some()
            && matches!(self.key, KeyState::Present { .. } | KeyState::Absent { legacy: true });
        rows.push(Row::new(
            KEY_ACTIONS,
            "",
            Control::Buttons(vec![save, Button::new(KEY_FORGET, "Retirer la clé", can_forget)]),
        ));
        if let Some((tone, text)) = &self.key_note {
            rows.push(Row::new(KEY_NOTE, "", Control::Note { text: text.clone(), tone: *tone }));
        }
    }

    fn model_rows(&self, provider: Provider, rows: &mut Vec<Row>) {
        let listed = match &self.models {
            Models::Listed(models) => Some(models),
            _ => None,
        };
        if listed.is_some_and(|models| models.len() > MAX_LISTED_MODELS) {
            rows.push(Row::new(
                SEARCH,
                "Chercher",
                Control::Field {
                    value: self.search.clone(),
                    placeholder: "Une partie du nom".to_owned(),
                    enabled: true,
                    search: true,
                },
            ));
        }
        let options = self.model_options();
        let selected = options.iter().position(|option| option.as_ref() == self.model.as_ref()).unwrap_or(0);
        let labels = options.iter().map(|option| self.model_label(option.as_deref())).collect();
        let enabled = listed.is_some_and(|models| !models.is_empty());
        rows.push(Row::new(MODEL, "Modèle", Control::Choice { options: labels, selected, enabled }));

        let name = provider.name();
        let whom = self.endpoint.as_ref().map(|endpoint| server(provider, endpoint)).unwrap_or_default();
        let local = self.endpoint.as_ref().is_ok_and(|endpoint| endpoint.reach() != Reach::Internet);
        let blocked = self.cannot_contact();
        let (tone, text) = match (&self.models, &blocked) {
            (Models::Asking, _) => (Tone::Busy, format!("Liste demandée à {whom}…")),
            (Models::Failed(problem), _) => (Tone::Bad, problem.clone()),
            (Models::Listed(models), _) => self.listed_note(provider, models),
            (Models::NotAsked, Some(reason)) => (Tone::Quiet, reason.clone()),
            (Models::NotAsked, None) => {
                (Tone::Quiet, format!("La liste des modèles n'a pas encore été demandée à {whom}."))
            }
        };
        let mut text = text;
        if let (Some(model), Some(models)) = (&self.model, listed)
            && !has_model(models, model)
        {
            text = format!("{name} ne propose pas « {model} ». {text}");
            if local {
                text.push(' ');
                text.push_str(&how_to_get(provider, model));
            }
        }
        rows.push(Row::new(MODELS_NOTE, "", Control::Note { text, tone }));
        let title = if self.models == Models::NotAsked { "Demander la liste" } else { "Redemander la liste" };
        let can_ask = blocked.is_none() && self.models != Models::Asking;
        rows.push(Row::new(
            MODELS_ACTIONS,
            "",
            Control::Buttons(vec![Button::new(MODELS_ASK, title, can_ask)]),
        ));
    }

    /// Ce qu'on dit d'une liste reçue.
    fn listed_note(&self, provider: Provider, models: &[String]) -> (Tone, String) {
        let name = provider.name();
        let local = self.endpoint.as_ref().is_ok_and(|endpoint| endpoint.reach() != Reach::Internet);
        if models.is_empty() {
            let mut text = format!("{name} ne propose aucun modèle.");
            if local {
                text.push(' ');
                text.push_str(&how_to_get(provider, "<nom>"));
            }
            return (Tone::Warning, text);
        }
        let found = search_models(models, &self.search).len();
        let word = self.search.trim();
        if found == 0 {
            (Tone::Warning, format!("Aucun des {} modèles de {name} ne contient « {word} ».", models.len()))
        } else if found > MAX_LISTED_MODELS {
            (
                Tone::Quiet,
                format!(
                    "{found} modèles sur {} : les {MAX_LISTED_MODELS} premiers sont dans la liste, précise ta \
                     recherche.",
                    models.len()
                ),
            )
        } else if word.is_empty() {
            (Tone::Quiet, format!("{} modèles proposés par {name}.", models.len()))
        } else {
            (Tone::Quiet, format!("{found} modèles sur {} contiennent « {word} ».", models.len()))
        }
    }

    fn trial_rows(&self, rows: &mut Vec<Row>) {
        let talking = matches!(self.trial, Trial::Talking { .. });
        let can_talk = !talking && self.candidate().is_some() && self.cannot_contact().is_none();
        rows.push(Row::new(
            TALK_ACTIONS,
            "Essai",
            Control::Buttons(vec![Button::new(TALK, "Essayer", can_talk)]),
        ));
        let (text, note) = match &self.trial {
            Trial::None => {
                let hint = if self.candidate().is_some() {
                    format!("« {TEST_QUESTION} », pour voir ce qu'elle répond avec ce cerveau.")
                } else {
                    "Choisis d'abord un modèle.".to_owned()
                };
                (None, (Tone::Quiet, hint))
            }
            Trial::Talking { brain, text } => {
                (Some(text), (Tone::Busy, format!("{} écrit sa réponse…", brain.model())))
            }
            Trial::Answered { text, seconds } => {
                (Some(text), (Tone::Good, format!("Réponse en {}.", seconds_fr(*seconds))))
            }
            Trial::Failed { text, problem } => (Some(text), (Tone::Bad, problem.clone())),
        };
        if let Some(text) = text {
            rows.push(Row::new(TALK_TEXT, "", Control::Text { text: format!("> {TEST_QUESTION}\n{text}") }));
        }
        rows.push(Row::new(TALK_NOTE, "", Control::Note { text: note.1, tone: note.0 }));
    }

    fn use_rows(&self, rows: &mut Vec<Row>) {
        let button = match self.provider {
            None => Button::new(DROP, "Lui retirer son cerveau", self.chosen.is_some()),
            Some(_) => {
                let candidate = self.candidate();
                let changes = candidate.is_some() && candidate != self.chosen;
                Button::new(USE, "Utiliser ce cerveau", changes && self.cannot_contact().is_none())
            }
        };
        rows.push(Row::new(USE_ACTIONS, "", Control::Buttons(vec![button])));
        if let Some((tone, text)) = &self.use_note {
            rows.push(Row::new(USE_NOTE, "", Control::Note { text: text.clone(), tone: *tone }));
        }
    }
}

impl Rubric for BrainRubric {
    fn id(&self) -> &'static str {
        ID
    }

    fn title(&self) -> &'static str {
        "Cerveau"
    }

    fn rows(&self) -> Vec<Row> {
        let mut rows = vec![self.now_row(), self.provider_row()];
        if let Some(provider) = self.provider {
            self.address_rows(provider, &mut rows);
            self.key_rows(provider, &mut rows);
            self.model_rows(provider, &mut rows);
            self.trial_rows(&mut rows);
        }
        self.use_rows(&mut rows);
        rows
    }

    fn open(&mut self) -> Vec<Effect> {
        vec![Effect::LoadBrain]
    }

    fn event(&mut self, event: FormEvent) -> Vec<Effect> {
        match event {
            FormEvent::Chose { id, index } if id == PROVIDER => {
                let provider = index.checked_sub(1).and_then(|index| Provider::ALL.get(index).copied());
                if provider == self.provider {
                    return Vec::new();
                }
                self.show_provider(provider)
            }
            FormEvent::Edited { id, value } if id == URL => {
                self.url = value;
                Vec::new()
            }
            FormEvent::Committed { id, value } if id == URL => {
                self.url = value.clone();
                if value.trim() == self.committed.trim() {
                    return Vec::new();
                }
                self.use_note = None;
                self.show_address(value)
            }
            FormEvent::Edited { id, value } | FormEvent::Committed { id, value } if id == SEARCH => {
                self.search = value;
                Vec::new()
            }
            FormEvent::Chose { id, index } if id == MODEL => {
                if let Some(Some(model)) = self.model_options().get(index).cloned()
                    && self.model.as_ref() != Some(&model)
                {
                    self.model = Some(model);
                    self.trial = Trial::None;
                    self.use_note = None;
                }
                Vec::new()
            }
            FormEvent::Pressed { id, secret } if id == KEY_SAVE => {
                let (Some(account), Some(secret)) = (self.account(), secret) else { return Vec::new() };
                if !self.endpoint.as_ref().is_ok_and(Endpoint::may_carry_secret) {
                    let text = format!(
                        "{} Il faut une adresse en https.",
                        capitalized(&no_key_in_clear(account.address()))
                    );
                    self.key_note = Some((Tone::Bad, text));
                    return Vec::new();
                }
                match ApiKey::parse(secret.reveal()) {
                    Ok(key) => {
                        self.key_note = Some((Tone::Busy, "Rangement dans le trousseau…".to_owned()));
                        vec![Effect::StoreKey { account, key }]
                    }
                    Err(error) => {
                        self.key_note = Some((Tone::Bad, capitalized(&not_a_key(error))));
                        Vec::new()
                    }
                }
            }
            FormEvent::Pressed { id, .. } if id == KEY_FORGET => {
                let Some(account) = self.account() else { return Vec::new() };
                self.key_note = Some((Tone::Busy, "Retrait du trousseau…".to_owned()));
                vec![Effect::ForgetKey(account)]
            }
            FormEvent::Pressed { id, .. } if id == MODELS_ASK => {
                if self.cannot_contact().is_some() {
                    return Vec::new();
                }
                self.ask_models().into_iter().collect()
            }
            FormEvent::Pressed { id, .. } if id == TALK => {
                let Some(brain) = self.candidate().filter(|_| self.cannot_contact().is_none()) else {
                    return Vec::new();
                };
                self.trial = Trial::Talking { brain: brain.clone(), text: String::new() };
                vec![Effect::Talk { brain, message: TEST_QUESTION.to_owned() }]
            }
            FormEvent::Pressed { id, .. } if id == USE => match self.candidate() {
                Some(brain) if self.cannot_contact().is_none() => {
                    self.use_note = Some((Tone::Busy, "Un instant…".to_owned()));
                    vec![Effect::UseBrain(brain)]
                }
                _ => Vec::new(),
            },
            FormEvent::Pressed { id, .. } if id == DROP => {
                self.use_note = Some((Tone::Busy, "Un instant…".to_owned()));
                vec![Effect::DropBrain]
            }
            _ => Vec::new(),
        }
    }

    fn done(&mut self, outcome: Outcome) -> Vec<Effect> {
        match outcome {
            Outcome::BrainLoaded(loaded) => {
                self.loaded = true;
                let (chosen, unreadable) = match loaded {
                    Ok(chosen) => (chosen, None),
                    Err(problem) => (None, Some(problem)),
                };
                self.chosen = chosen;
                self.unreadable = unreadable;
                let provider = self.chosen.as_ref().map(Brain::provider);
                self.show_provider(provider)
            }
            Outcome::KeyChecked { account, present, legacy } if Some(&account) == self.account().as_ref() => {
                self.key = match present {
                    Ok(true) => KeyState::Present { legacy },
                    Ok(false) => KeyState::Absent { legacy },
                    Err(problem) => KeyState::Unreadable(problem),
                };
                self.list_if_local().into_iter().collect()
            }
            Outcome::KeyStored { account, result } if Some(&account) == self.account().as_ref() => {
                let provider = account.provider();
                let keychain = self.keychain.clone().unwrap_or_default();
                match result {
                    Ok(legacy) => {
                        let mut text = key_stored(provider, &keychain, account.address());
                        match legacy {
                            LegacyKey::None => {}
                            LegacyKey::Removed => {
                                text.push(' ');
                                text.push_str(&legacy_key_removed(provider, false));
                            }
                            LegacyKey::Kept(problem) => text.push_str(&format!(
                                " L'ancienne clé {}, rangée sans adresse, n'a pas pu être retirée ({problem}) : elle \
                                 n'est jamais envoyée.",
                                of_provider(provider)
                            )),
                        }
                        self.key_note = Some((Tone::Good, text));
                        self.models = Models::NotAsked;
                        // ce qui est affiché vient du trousseau, pas de ce qu'on croit y avoir mis
                        self.key = KeyState::Checking;
                        vec![Effect::CheckKey(account)]
                    }
                    Err(problem) => {
                        self.key_note = Some((Tone::Bad, problem));
                        Vec::new()
                    }
                }
            }
            Outcome::KeyForgotten { account, result } if Some(&account) == self.account().as_ref() => {
                let provider = account.provider();
                let keychain = self.keychain.clone().unwrap_or_default();
                match result {
                    Ok(Forgotten { key, legacy }) => {
                        let mut text = key_forgotten(provider, &keychain, account.address(), key);
                        if legacy {
                            text.push(' ');
                            text.push_str(&legacy_key_removed(provider, true));
                        }
                        self.key_note = Some((Tone::Good, text));
                        self.key = KeyState::Checking;
                        return vec![Effect::CheckKey(account)];
                    }
                    Err(problem) => self.key_note = Some((Tone::Bad, problem)),
                }
                Vec::new()
            }
            Outcome::ModelsListed { provider, endpoint, result }
                if Some(provider) == self.provider && Ok(&endpoint) == self.endpoint.as_ref() =>
            {
                self.models = match result {
                    Ok(models) => {
                        // sur cette machine, un seul modèle installé est pris d'office, comme le fait
                        // `capri chat use`
                        if self.model.is_none() && endpoint.reach() != Reach::Internet && models.len() == 1 {
                            self.model = models.first().cloned();
                        }
                        Models::Listed(models)
                    }
                    Err(problem) => Models::Failed(problem),
                };
                Vec::new()
            }
            Outcome::TalkPiece { brain, text } => {
                if let Trial::Talking { brain: asked, text: so_far } = &mut self.trial
                    && *asked == brain
                {
                    so_far.push_str(&text);
                }
                Vec::new()
            }
            Outcome::Talked { brain, result } => {
                if let Trial::Talking { brain: asked, text } = &self.trial
                    && *asked == brain
                {
                    let text = text.clone();
                    self.trial = match result {
                        Ok(seconds) => Trial::Answered { text, seconds },
                        Err(problem) => Trial::Failed { text, problem },
                    };
                }
                Vec::new()
            }
            Outcome::BrainUsed(used) => {
                match used {
                    Ok(brain) => {
                        self.use_note = Some((
                            Tone::Good,
                            format!(
                                "{} Rien à relancer : c'est pris en compte dès ta prochaine phrase.",
                                thinks_now_with(&brain)
                            ),
                        ));
                        self.chosen = Some(brain);
                        self.unreadable = None;
                    }
                    Err(problem) => self.use_note = Some((Tone::Bad, problem)),
                }
                Vec::new()
            }
            Outcome::BrainDropped(dropped) => {
                match dropped {
                    Ok(_) => {
                        self.use_note = Some((Tone::Good, NO_MORE_BRAIN.to_owned()));
                        self.chosen = None;
                        self.unreadable = None;
                    }
                    Err(problem) => self.use_note = Some((Tone::Bad, problem)),
                }
                Vec::new()
            }
            // un résultat pour une adresse qu'on ne regarde plus : périmé
            _ => Vec::new(),
        }
    }
}

/// Le nom d'un fournisseur dans la liste, avec où il vit.
fn provider_label(provider: Provider) -> String {
    let name = provider.name();
    match (provider, provider.key_need()) {
        (Provider::Custom, _) => format!("{name} (une adresse à donner)"),
        (_, KeyNeed::Required) => format!("{name} — sur Internet, avec une clé"),
        _ => format!("{name} — sur ta machine, sans compte"),
    }
}

/// Une phrase de la ligne de commande, qui commence en minuscule, en tête de note.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// « 2,3 s ».
fn seconds_fr(seconds: f32) -> String {
    format!("{seconds:.1} s").replace('.', ",")
}

#[cfg(test)]
mod tests {
    use capri_core::form::Secret;

    use super::*;
    use crate::settings::{Done, Settings, Task};

    const KEY_TEXT: &str = "sk-test-fenetre-0123456789abcdef";
    const OPENROUTER: &str = "https://openrouter.ai/api/v1";

    fn environment() -> Environment {
        Environment { keychain: Some("le trousseau de test".to_owned()), tls: true, voice: true }
    }

    /// Une fenêtre ouverte, réglages lus : `chosen` est le cerveau de `chat.json`.
    fn opened(chosen: Option<Brain>) -> (BrainRubric, Vec<Effect>) {
        let mut rubric = BrainRubric::new(&environment());
        assert!(matches!(rubric.open().as_slice(), [Effect::LoadBrain]));
        let effects = rubric.done(Outcome::BrainLoaded(Ok(chosen)));
        (rubric, effects)
    }

    fn note(rubric: &BrainRubric, id: &str) -> Option<(Tone, String)> {
        rubric.rows().into_iter().find(|row| row.id == id).and_then(|row| match row.control {
            Control::Note { text, tone } => Some((tone, text)),
            _ => None,
        })
    }

    fn control(rubric: &BrainRubric, id: &str) -> Option<Control> {
        rubric.rows().into_iter().find(|row| row.id == id).map(|row| row.control)
    }

    fn button(rubric: &BrainRubric, id: &str) -> Option<Button> {
        rubric.rows().into_iter().find_map(|row| match row.control {
            Control::Buttons(buttons) => buttons.into_iter().find(|button| button.id == id),
            _ => None,
        })
    }

    fn choose_provider(rubric: &mut BrainRubric, provider: Provider) -> Vec<Effect> {
        let index = Provider::ALL.iter().position(|&known| known == provider).expect("connu") + 1;
        rubric.event(FormEvent::Chose { id: PROVIDER.to_owned(), index })
    }

    fn key_checked(rubric: &mut BrainRubric, provider: Provider, url: &str, present: bool) -> Vec<Effect> {
        let account = KeyAccount::new(provider, &Endpoint::parse(url).expect("valable"));
        rubric.done(Outcome::KeyChecked { account, present: Ok(present), legacy: false })
    }

    fn models(count: usize) -> Vec<String> {
        (0..count).map(|number| format!("labo/modele-{number:03}")).collect()
    }

    fn listed(rubric: &mut BrainRubric, provider: Provider, url: &str, names: Vec<String>) -> Vec<Effect> {
        let endpoint = Endpoint::parse(url).expect("valable");
        rubric.done(Outcome::ModelsListed { provider, endpoint, result: Ok(names) })
    }

    fn press(rubric: &mut BrainRubric, id: &str, secret: Option<&str>) -> Vec<Effect> {
        rubric.event(FormEvent::Pressed {
            id: id.to_owned(),
            secret: secret.map(|s| Secret::new(s.to_owned())),
        })
    }

    #[test]
    fn without_a_brain_she_says_so_and_offers_nothing_to_remove() {
        let (rubric, effects) = opened(None);
        assert!(effects.is_empty(), "rien à demander à personne : {effects:?}");
        assert_eq!(note(&rubric, NOW), Some((Tone::Quiet, NO_BRAIN.to_owned())));
        let Some(Control::Choice { options, selected, .. }) = control(&rubric, PROVIDER) else { panic!() };
        assert_eq!((options.len(), selected), (Provider::ALL.len() + 1, 0));
        assert!(button(&rubric, DROP).is_some_and(|button| !button.enabled));
        assert!(control(&rubric, URL).is_none() && control(&rubric, KEY).is_none());
    }

    #[test]
    fn a_remote_provider_says_where_the_conversation_goes_before_being_chosen_and_contacts_nobody() {
        let (mut rubric, _) = opened(None);
        let effects = choose_provider(&mut rubric, Provider::OpenRouter);
        // le trousseau est consulté (sans lire la clé), le réseau ne l'est pas
        assert!(
            matches!(effects.as_slice(), [Effect::CheckKey(account)] if account.name() == "openrouter@openrouter.ai:443"),
            "{effects:?}"
        );
        assert_eq!(
            control(&rubric, URL),
            Some(Control::Field {
                value: OPENROUTER.to_owned(),
                placeholder: OPENROUTER.to_owned(),
                enabled: true,
                search: false,
            })
        );
        let (tone, text) = note(&rubric, WHERE).expect("la destination est dite");
        assert_eq!(tone, Tone::Warning);
        assert!(
            text.starts_with("Attention : ce que tu lui dis partira chez OpenRouter (openrouter.ai:443)"),
            "{text}"
        );
        assert_eq!(rubric.chosen, None, "rien n'est encore choisi");
        assert!(button(&rubric, USE).is_some_and(|button| !button.enabled));

        // pas de clé : on le dit, on ne propose ni la liste, ni l'essai, ni de l'utiliser
        assert!(key_checked(&mut rubric, Provider::OpenRouter, OPENROUTER, false).is_empty());
        assert_eq!(
            note(&rubric, KEY_STATUS),
            Some((Tone::Warning, "Aucune clé pour cette adresse : OpenRouter en demande une.".to_owned()))
        );
        assert!(button(&rubric, MODELS_ASK).is_some_and(|button| !button.enabled));
        assert!(
            press(&mut rubric, MODELS_ASK, None).is_empty(),
            "personne n'est contacté sans la clé exigée"
        );
        assert!(button(&rubric, TALK).is_some_and(|button| !button.enabled));
        assert!(note(&rubric, MODELS_NOTE).is_some_and(|(_, text)| text.contains("enregistre-la d'abord")));

        // avec une clé : la liste n'est demandée qu'à la main, sur Internet
        assert!(key_checked(&mut rubric, Provider::OpenRouter, OPENROUTER, true).is_empty());
        assert_eq!(
            note(&rubric, KEY_STATUS),
            Some((
                Tone::Good,
                "Une clé est rangée pour openrouter.ai:443, dans le trousseau de test. Elle ne s'affiche jamais."
                    .to_owned()
            ))
        );
        let asked = press(&mut rubric, MODELS_ASK, None);
        assert!(
            matches!(asked.as_slice(), [Effect::ListModels { provider: Provider::OpenRouter, .. }]),
            "{asked:?}"
        );
        assert_eq!(note(&rubric, MODELS_NOTE).map(|(tone, _)| tone), Some(Tone::Busy));
    }

    #[test]
    fn the_secret_field_is_never_filled_and_the_key_appears_nowhere() {
        let (mut rubric, _) = opened(None);
        choose_provider(&mut rubric, Provider::OpenRouter);
        key_checked(&mut rubric, Provider::OpenRouter, OPENROUTER, true);
        assert_eq!(
            control(&rubric, KEY),
            Some(Control::Secret {
                placeholder: "Colle la clé ici : elle ne s'affiche pas".to_owned(),
                enabled: true,
                submit: Some(KEY_SAVE.to_owned()),
            })
        );
        assert_eq!(button(&rubric, KEY_SAVE).and_then(|button| button.secret), Some(KEY.to_owned()));

        let effects = press(&mut rubric, KEY_SAVE, Some(&format!("  {KEY_TEXT}\n")));
        let [Effect::StoreKey { account, key }] = effects.as_slice() else { panic!("{effects:?}") };
        assert_eq!(account.name(), "openrouter@openrouter.ai:443", "liée à l'adresse affichée");
        assert_eq!(key.reveal_for_keychain(), KEY_TEXT, "sans les blancs autour");
        assert!(!format!("{effects:?}").contains("fenetre"), "{effects:?}");

        let account = account.clone();
        let effects =
            rubric.done(Outcome::KeyStored { account: account.clone(), result: Ok(LegacyKey::Removed) });
        assert!(
            matches!(effects.as_slice(), [Effect::CheckKey(checked)] if *checked == account),
            "l'état affiché est relu dans le trousseau : {effects:?}"
        );
        let (tone, text) = note(&rubric, KEY_NOTE).expect("une note");
        assert_eq!(tone, Tone::Good);
        assert_eq!(
            text,
            "Clé d'OpenRouter rangée dans le trousseau de test, pour openrouter.ai:443 seulement : elle ne partira \
             vers aucune autre adresse, et n'est écrite nulle part ailleurs. L'ancienne clé d'OpenRouter, rangée \
             sans adresse par une version précédente de Capri, est retirée."
        );
        let shown = format!("{:?}", rubric.rows());
        assert!(!shown.contains("fenetre") && !shown.contains("0123456789abcdef"), "{shown}");

        // ce qui n'est pas une clé n'est ni rangé, ni répété
        for typed in ["deux mots secrets", "clé-accentuée-secrète", "  "] {
            assert!(press(&mut rubric, KEY_SAVE, Some(typed)).is_empty(), "{typed:?}");
            let (tone, text) = note(&rubric, KEY_NOTE).expect("une note");
            assert_eq!(tone, Tone::Bad);
            assert!(text.ends_with("rien n'est changé") && !text.contains("secr"), "{text}");
        }
    }

    #[test]
    fn a_key_is_bound_to_the_address_shown_and_never_stored_for_one_reached_in_clear() {
        let (mut rubric, _) = opened(None);
        choose_provider(&mut rubric, Provider::Custom);
        assert!(
            note(&rubric, URL_NOTE).is_some_and(|(_, text)| text.contains("n'a pas d'adresse habituelle"))
        );
        assert!(button(&rubric, KEY_SAVE).is_some_and(|button| !button.enabled));

        let lan = "http://192.168.1.20:1234/v1";
        rubric.event(FormEvent::Edited { id: URL.to_owned(), value: lan.to_owned() });
        assert!(
            note(&rubric, WHERE).is_some_and(|(_, text)| text.starts_with("Valide l'adresse")),
            "rien n'est affirmé tant que l'adresse n'est pas validée"
        );
        let effects = rubric.event(FormEvent::Committed { id: URL.to_owned(), value: lan.to_owned() });
        assert!(
            matches!(effects.as_slice(), [Effect::CheckKey(account)] if account.name() == "custom@192.168.1.20:1234")
        );
        key_checked(&mut rubric, Provider::Custom, lan, false);
        assert!(note(&rubric, KEY_STATUS).is_some_and(|(_, text)| text.contains("n'est joint qu'en http")));
        assert!(matches!(control(&rubric, KEY), Some(Control::Secret { enabled: false, .. })));
        assert!(press(&mut rubric, KEY_SAVE, Some(KEY_TEXT)).is_empty(), "rien n'est rangé");
        assert!(note(&rubric, KEY_NOTE).is_some_and(|(tone, _)| tone == Tone::Bad));

        // un résultat qui arrive pour une adresse qu'on ne regarde plus est ignoré
        rubric.event(FormEvent::Committed { id: URL.to_owned(), value: "https://llm.example/v1".to_owned() });
        key_checked(&mut rubric, Provider::Custom, lan, true);
        assert_eq!(rubric.key, KeyState::Checking);
        listed(&mut rubric, Provider::Custom, lan, models(3));
        assert_eq!(rubric.models, Models::NotAsked);

        // une adresse refusée l'est avec la raison, et rien n'est demandé
        let effects = rubric
            .event(FormEvent::Committed { id: URL.to_owned(), value: "http://example.com/v1".to_owned() });
        assert!(effects.is_empty());
        assert!(note(&rubric, URL_NOTE).is_some_and(|(_, text)| text.starts_with("adresse refusée : ")));
    }

    #[test]
    fn hundreds_of_models_are_searched_and_never_all_listed() {
        let (mut rubric, _) = opened(None);
        choose_provider(&mut rubric, Provider::OpenRouter);
        key_checked(&mut rubric, Provider::OpenRouter, OPENROUTER, true);
        press(&mut rubric, MODELS_ASK, None);
        let mut names = models(467);
        names.push("autre/chose".to_owned());
        names.push("piege/\u{1b}[31mrouge".to_owned());
        listed(&mut rubric, Provider::OpenRouter, OPENROUTER, names);

        let Some(Control::Choice { options, selected, enabled }) = control(&rubric, MODEL) else { panic!() };
        assert!(enabled);
        assert_eq!(options.len(), MAX_LISTED_MODELS + 1, "l'invite, puis une liste bornée");
        assert_eq!((options[0].as_str(), selected), ("Choisis un modèle…", 0));
        assert_eq!(options[1], "autre/chose");
        assert!(!options.iter().any(|option| option.contains('\u{1b}')));
        assert!(note(&rubric, MODELS_NOTE).is_some_and(|(_, text)| text.contains("précise ta recherche")));
        assert!(control(&rubric, SEARCH).is_some(), "un champ de recherche");

        rubric.event(FormEvent::Edited { id: SEARCH.to_owned(), value: "MODELE-12".to_owned() });
        let Some(Control::Choice { options, .. }) = control(&rubric, MODEL) else { panic!() };
        assert_eq!(
            &options[1..],
            [
                "labo/modele-120",
                "labo/modele-121",
                "labo/modele-122",
                "labo/modele-123",
                "labo/modele-124",
                "labo/modele-125",
                "labo/modele-126",
                "labo/modele-127",
                "labo/modele-128",
                "labo/modele-129"
            ]
        );
        rubric.event(FormEvent::Chose { id: MODEL.to_owned(), index: 1 });
        assert_eq!(rubric.model.as_deref(), Some("labo/modele-120"));

        // le modèle choisi reste dans la liste même quand la recherche ne le trouve plus
        rubric.event(FormEvent::Edited { id: SEARCH.to_owned(), value: "introuvable".to_owned() });
        let Some(Control::Choice { options, selected, .. }) = control(&rubric, MODEL) else { panic!() };
        assert_eq!((options, selected), (vec!["labo/modele-120".to_owned()], 0));
        assert_eq!(
            note(&rubric, MODELS_NOTE).map(|(_, text)| text).as_deref(),
            Some("Aucun des 469 modèles de OpenRouter ne contient « introuvable ».")
        );
    }

    #[test]
    fn a_local_server_is_asked_for_its_models_by_itself_and_a_lone_model_is_taken() {
        let local = "http://localhost:11434/v1";
        let (mut rubric, effects) = opened(None);
        assert!(effects.is_empty());
        let effects = choose_provider(&mut rubric, Provider::Ollama);
        assert!(
            matches!(effects.as_slice(), [Effect::ListModels { provider: Provider::Ollama, .. }]),
            "Ollama ne prend pas de clé, et vit ici : {effects:?}"
        );
        assert!(note(&rubric, WHERE).is_some_and(|(_, text)| text.contains("sauf avec un modèle « cloud »")));
        listed(&mut rubric, Provider::Ollama, local, vec!["seul:8b".to_owned()]);
        assert_eq!(rubric.model.as_deref(), Some("seul:8b"));
        assert_eq!(
            note(&rubric, WHERE),
            Some((Tone::Quiet, "Ce que tu lui dis reste sur cette machine.".to_owned()))
        );
        assert!(button(&rubric, USE).is_some_and(|button| button.enabled));

        // un modèle « cloud » : la phrase change, et la liste le signale
        listed(&mut rubric, Provider::Ollama, local, vec!["seul:8b".to_owned(), "autre:cloud".to_owned()]);
        let Some(Control::Choice { options, .. }) = control(&rubric, MODEL) else { panic!() };
        assert!(
            options.contains(&"autre:cloud (relayé vers les serveurs d'Ollama)".to_owned()),
            "{options:?}"
        );
        let index = options.iter().position(|option| option.starts_with("autre:cloud")).expect("là");
        rubric.event(FormEvent::Chose { id: MODEL.to_owned(), index });
        let (tone, text) = note(&rubric, WHERE).expect("dite");
        assert_eq!(tone, Tone::Warning);
        assert!(text.starts_with("Attention : avec ce modèle « cloud »"), "{text}");
    }

    #[test]
    fn the_chosen_brain_is_shown_tried_and_replaced() {
        let local = "http://localhost:11434/v1";
        let chosen = Brain::new(Provider::Ollama, None, "un:8b").expect("valable");
        let (mut rubric, effects) = opened(Some(chosen.clone()));
        assert!(matches!(effects.as_slice(), [Effect::ListModels { .. }]));
        assert_eq!(
            note(&rubric, NOW),
            Some((Tone::Good, "Capri pense avec un:8b (Ollama, localhost:11434).".to_owned()))
        );
        assert_eq!(rubric.model.as_deref(), Some("un:8b"));
        assert!(button(&rubric, USE).is_some_and(|button| !button.enabled), "c'est déjà lui");

        listed(&mut rubric, Provider::Ollama, local, vec!["un:8b".to_owned(), "deux:4b".to_owned()]);
        // l'essai : la phrase d'essai, la réponse au fil de l'eau, puis sa durée
        let effects = press(&mut rubric, TALK, None);
        let [Effect::Talk { brain, message }] = effects.as_slice() else { panic!("{effects:?}") };
        assert_eq!((brain, message.as_str()), (&chosen, TEST_QUESTION));
        assert!(button(&rubric, TALK).is_some_and(|button| !button.enabled), "un essai à la fois");
        rubric.done(Outcome::TalkPiece { brain: chosen.clone(), text: "Salut, ".to_owned() });
        rubric.done(Outcome::TalkPiece { brain: chosen.clone(), text: "c'est Capri.".to_owned() });
        rubric.done(Outcome::Talked { brain: chosen.clone(), result: Ok(2.34) });
        assert_eq!(
            control(&rubric, TALK_TEXT),
            Some(Control::Text { text: format!("> {TEST_QUESTION}\nSalut, c'est Capri.") })
        );
        assert_eq!(note(&rubric, TALK_NOTE), Some((Tone::Good, "Réponse en 2,3 s.".to_owned())));

        // une panne se dit avec le message de `capri chat test`
        press(&mut rubric, TALK, None);
        rubric.done(Outcome::Talked {
            brain: chosen.clone(),
            result: Err("Ollama met trop de temps à répondre.".to_owned()),
        });
        assert_eq!(
            note(&rubric, TALK_NOTE),
            Some((Tone::Bad, "Ollama met trop de temps à répondre.".to_owned()))
        );

        // un autre modèle : on peut l'utiliser
        rubric.event(FormEvent::Chose { id: MODEL.to_owned(), index: 0 });
        assert_eq!(rubric.model.as_deref(), Some("deux:4b"), "dans l'ordre alphabétique");
        let effects = press(&mut rubric, USE, None);
        let [Effect::UseBrain(brain)] = effects.as_slice() else { panic!("{effects:?}") };
        assert_eq!(brain.model(), "deux:4b");
        let brain = brain.clone();
        rubric.done(Outcome::BrainUsed(Ok(brain.clone())));
        assert_eq!(rubric.chosen.as_ref(), Some(&brain));
        assert!(note(&rubric, USE_NOTE).is_some_and(|(tone, text)| tone == Tone::Good
            && text.starts_with("Capri pense maintenant avec deux:4b (Ollama, localhost:11434).")));

        // lui retirer son cerveau
        rubric.event(FormEvent::Chose { id: PROVIDER.to_owned(), index: 0 });
        let effects = press(&mut rubric, DROP, None);
        assert!(matches!(effects.as_slice(), [Effect::DropBrain]));
        rubric.done(Outcome::BrainDropped(Ok(true)));
        assert_eq!(note(&rubric, USE_NOTE), Some((Tone::Good, NO_MORE_BRAIN.to_owned())));
        assert_eq!(note(&rubric, NOW), Some((Tone::Quiet, NO_BRAIN.to_owned())));
    }

    #[test]
    fn a_stale_answer_to_an_old_trial_is_ignored() {
        let chosen = Brain::new(Provider::Ollama, None, "un:8b").expect("valable");
        let (mut rubric, _) = opened(Some(chosen.clone()));
        press(&mut rubric, TALK, None);
        choose_provider(&mut rubric, Provider::LmStudio);
        rubric.done(Outcome::Talked { brain: chosen, result: Ok(1.0) });
        assert_eq!(rubric.trial, Trial::None);
    }

    #[test]
    fn an_unreadable_settings_file_is_said_and_another_brain_can_still_be_chosen() {
        let mut rubric = BrainRubric::new(&environment());
        rubric.done(Outcome::BrainLoaded(Err("réglages illisibles (chat.json)".to_owned())));
        assert!(
            note(&rubric, NOW).is_some_and(|(tone, text)| tone == Tone::Bad && text.contains("illisibles"))
        );
        assert!(!choose_provider(&mut rubric, Provider::Ollama).is_empty());
    }

    #[test]
    fn without_a_keychain_no_key_is_asked_for() {
        let mut rubric = BrainRubric::new(&Environment { keychain: None, tls: true, voice: false });
        rubric.done(Outcome::BrainLoaded(Ok(None)));
        let effects = choose_provider(&mut rubric, Provider::DeepSeek);
        assert!(effects.is_empty(), "{effects:?}");
        assert!(
            note(&rubric, KEY_STATUS)
                .is_some_and(|(_, text)| text.starts_with("Pas de trousseau sur ce système"))
        );
        assert!(matches!(control(&rubric, KEY), Some(Control::Secret { enabled: false, .. })));
        assert!(button(&rubric, MODELS_ASK).is_some_and(|button| !button.enabled));
    }

    #[test]
    fn forgetting_a_key_says_what_capri_chat_key_forget_says() {
        let (mut rubric, _) = opened(None);
        choose_provider(&mut rubric, Provider::OpenRouter);
        key_checked(&mut rubric, Provider::OpenRouter, OPENROUTER, true);
        let effects = press(&mut rubric, KEY_FORGET, None);
        let [Effect::ForgetKey(account)] = effects.as_slice() else { panic!("{effects:?}") };
        let account = account.clone();
        let effects = rubric.done(Outcome::KeyForgotten {
            account: account.clone(),
            result: Ok(Forgotten { key: true, legacy: false }),
        });
        assert!(matches!(effects.as_slice(), [Effect::CheckKey(checked)] if *checked == account));
        assert_eq!(
            note(&rubric, KEY_NOTE),
            Some((
                Tone::Good,
                "Clé d'OpenRouter pour openrouter.ai:443 retirée : elle n'est plus dans le trousseau de test.".to_owned()
            ))
        );
        assert!(button(&rubric, KEY_FORGET).is_some_and(|button| !button.enabled));
    }

    #[test]
    fn the_whole_rubric_goes_through_the_settings() {
        let mut settings = Settings::new(&environment());
        settings.open();
        let tasks = settings.done(Done { rubric: ID, outcome: Outcome::BrainLoaded(Ok(None)) });
        assert!(tasks.is_empty());
        let index =
            Provider::ALL.iter().position(|&known| known == Provider::OllamaCloud).expect("connu") + 1;
        let tasks = settings.event(FormEvent::Chose { id: PROVIDER.to_owned(), index });
        assert!(matches!(tasks.as_slice(), [Task { rubric: ID, effect: Effect::CheckKey(account) }]
            if account.name() == "ollama-cloud@ollama.com:443"));
        let form = settings.form();
        assert!(form.duplicate_ids().is_empty());
        assert!(
            matches!(&form.row(WHERE).expect("dite").control, Control::Note { text, tone: Tone::Warning }
            if text.contains("Ollama en ligne (ollama.com:443)"))
        );
    }
}
