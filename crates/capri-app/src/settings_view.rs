//! La fenêtre de réglages côté application : le modèle (`capri_chat::settings`), la fenêtre
//! (`capri_platform::SettingsWindow`), et l'exécution de ce que le modèle demande.
//!
//! Les effets passent par les mêmes fonctions que la ligne de commande : `brain_setup` (fichier
//! `chat.json`, liste des modèles, essai), `keys` (trousseau) et `voice_setup` (`voice.json`,
//! voix installées). Ce qui attend le réseau ou le trousseau (qui peut attendre une réponse de
//! l'utilisateur) tourne sur un thread à part ; les fichiers, petits, et la voix, qui vit sur le
//! thread principal, sont traités sur place. Les résultats sont relevés à chaque pas de la boucle,
//! comme le reste de l'app.
//!
//! Le journal ne garde que la sorte de chaque opération : jamais une clé, ni ce que répond le
//! cerveau à l'essai.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use capri_chat::persona;
use capri_chat::settings::{Done, Effect, Environment, Forgotten, LegacyKey, Outcome, Settings, Task};
use capri_core::form::{Form, FormEvent};
use capri_net::Client;
use capri_platform::{SettingsWindow, SettingsWindowError, Voice};

use crate::keys::{self, KeyStore};
use crate::{brain_setup, voice_setup};

/// Ce que l'app doit savoir de ce qui vient de changer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Sa voix vient d'être activée ou coupée.
    Voice { enabled: bool },
}

pub struct SettingsView {
    window: SettingsWindow,
    settings: Settings,
    /// Le formulaire affiché.
    shown: Form,
    results: Receiver<Done>,
    effects: Effects,
}

/// Ce qui exécute les effets du modèle : les dossiers, le trousseau, le réseau et la voix.
struct Effects {
    sender: Sender<Done>,
    directory: Result<PathBuf, String>,
    keys: Arc<dyn KeyStore>,
    client: Arc<Client>,
    /// Sa voix, pour « Écouter ».
    voice: Option<Voice>,
    /// Levé à la fermeture : l'essai en cours s'arrête.
    closing: Arc<AtomicBool>,
}

impl SettingsView {
    /// Ouvre la fenêtre, avec les réglages tels qu'ils sont sur le disque et dans le trousseau.
    pub fn open() -> Result<Self, SettingsWindowError> {
        let keys = keys::SystemKeychain::shared();
        let voice = Voice::new();
        let environment = Environment {
            keychain: keys.available().then(|| keys.name().to_owned()),
            tls: capri_net::TLS,
            voice: voice.is_some(),
        };
        let mut settings = Settings::new(&environment);
        let opening = settings.open();
        let shown = settings.form();
        let window = SettingsWindow::open(&shown)?;
        let (sender, results) = channel();
        let directory = brain_setup::config_dir().map_err(|err| format!("{err:#}"));
        let effects = Effects {
            sender,
            directory,
            keys,
            client: Arc::new(Client::new()),
            voice,
            closing: Arc::new(AtomicBool::new(false)),
        };
        let mut view = Self { window, settings, shown, results, effects };
        for task in opening {
            let _ = view.effects.run(task);
        }
        view.pump();
        Ok(view)
    }

    /// La ramène devant.
    pub fn show(&self) {
        self.window.show();
    }

    pub fn is_open(&self) -> bool {
        self.window.is_open()
    }

    /// Ses réglages de la voix ont changé ailleurs (le menu) : elle les relit.
    pub fn reload_voice(&mut self) {
        let changes =
            self.effects.run(Task { rubric: capri_chat::settings::voice::ID, effect: Effect::LoadVoice });
        debug_assert!(changes.is_none());
    }

    /// À chaque pas : ce que l'utilisateur a fait, ce qui est revenu des opérations, et le
    /// formulaire à jour. Rend ce qui a changé pour le reste de l'app.
    pub fn pump(&mut self) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut dirty = false;
        for event in self.window.drain() {
            dirty = true;
            if event == FormEvent::Closed {
                self.effects.closing.store(true, Ordering::Relaxed);
                continue;
            }
            for task in self.settings.event(event) {
                changes.extend(self.effects.run(task));
            }
        }
        while let Ok(done) = self.results.try_recv() {
            dirty = true;
            for task in self.settings.done(done) {
                changes.extend(self.effects.run(task));
            }
        }
        if dirty && self.window.is_open() {
            let form = self.settings.form();
            if form != self.shown {
                self.window.update(&form);
                self.shown = form;
            }
        }
        changes
    }

    /// Pour les essais en version de débogage : fait comme si l'utilisateur choisissait ce
    /// fournisseur dans la liste (0 : aucun).
    #[cfg(debug_assertions)]
    pub fn rehearse(&mut self, provider: usize) {
        let event =
            FormEvent::Chose { id: capri_chat::settings::brain::PROVIDER.to_owned(), index: provider };
        for task in self.settings.event(event) {
            let _ = self.effects.run(task);
        }
        let form = self.settings.form();
        self.window.update(&form);
        self.shown = form;
    }

    /// Le nombre de lignes affichées, pour les essais.
    #[cfg(debug_assertions)]
    pub fn rows(&self) -> usize {
        self.shown.sections.iter().map(|section| section.rows.len()).sum()
    }

    /// Pour les essais : colle `text` dans ce champ, puis presse ce bouton, par la fenêtre elle-même.
    #[cfg(debug_assertions)]
    pub fn rehearse_typing_then_press(&self, field: &str, text: &str, button: &str) {
        self.window.rehearse_typing(field, text);
        self.window.rehearse_press(button);
    }

    #[cfg(debug_assertions)]
    pub fn rehearse_press(&self, button: &str) {
        self.window.rehearse_press(button);
    }

    /// Le texte affiché d'une note, pour les essais.
    #[cfg(debug_assertions)]
    pub fn note(&self, id: &str) -> Option<String> {
        match &self.shown.row(id)?.control {
            capri_core::form::Control::Note { text, .. } => Some(text.clone()),
            _ => None,
        }
    }

    pub fn has_keyboard(&self) -> bool {
        self.window.has_keyboard()
    }
}

impl Effects {
    /// Exécute un effet. Son résultat arrive par le canal des résultats.
    fn run(&mut self, task: Task) -> Option<Change> {
        let Task { rubric, effect } = task;
        let sender = self.sender.clone();
        let send = move |outcome: Outcome| {
            let _ = sender.send(Done { rubric, outcome });
        };
        let directory = self.directory.clone();
        match effect {
            Effect::LoadBrain => {
                log::debug!("réglages : relecture du cerveau");
                let loaded = directory.and_then(|directory| {
                    brain_setup::load(&directory).map(|setup| setup.map(|setup| setup.brain)).map_err(text)
                });
                send(Outcome::BrainLoaded(loaded));
            }
            Effect::UseBrain(brain) => {
                log::info!("réglages : nouveau cerveau ({}, {})", brain.provider().id(), brain.model());
                let used = directory
                    .and_then(|directory| brain_setup::save(&directory, &brain).map_err(text))
                    .map(|_| brain);
                send(Outcome::BrainUsed(used));
            }
            Effect::DropBrain => {
                log::info!("réglages : plus de cerveau");
                send(Outcome::BrainDropped(
                    directory.and_then(|directory| brain_setup::forget(&directory).map_err(text)),
                ));
            }
            Effect::CheckKey(account) => {
                let keys = Arc::clone(&self.keys);
                in_background("capri-trousseau", move || {
                    let present = keys.contains(&account.name()).map_err(text);
                    let legacy = keys::has_legacy_key(&*keys, account.provider());
                    send(Outcome::KeyChecked { account, present, legacy });
                });
            }
            Effect::StoreKey { account, key } => {
                log::info!("réglages : rangement d'une clé pour {}", account.name());
                let keys = Arc::clone(&self.keys);
                in_background("capri-trousseau", move || {
                    let result = keys::store(&*keys, &account, &key)
                        .map(|legacy| match legacy {
                            keys::Legacy::None => LegacyKey::None,
                            keys::Legacy::Removed => LegacyKey::Removed,
                            keys::Legacy::Kept(err) => LegacyKey::Kept(text(err)),
                        })
                        .map_err(text);
                    drop(key);
                    send(Outcome::KeyStored { account, result });
                });
            }
            Effect::ForgetKey(account) => {
                log::info!("réglages : retrait de la clé de {}", account.name());
                let keys = Arc::clone(&self.keys);
                in_background("capri-trousseau", move || {
                    let result = keys::forget(&*keys, &account)
                        .map(|forgotten| Forgotten { key: forgotten.key, legacy: forgotten.legacy })
                        .map_err(text);
                    send(Outcome::KeyForgotten { account, result });
                });
            }
            Effect::ListModels { provider, endpoint, chosen } => {
                log::debug!("réglages : liste des modèles de {}", endpoint.display_host());
                let (keys, client) = (Arc::clone(&self.keys), Arc::clone(&self.client));
                in_background("capri-modeles", move || {
                    let result =
                        brain_setup::list_models(&*keys, &client, provider, &endpoint, chosen.as_ref())
                            .map(|listed| listed.models)
                            .map_err(text);
                    send(Outcome::ModelsListed { provider, endpoint, result });
                });
            }
            Effect::Talk { brain, message } => {
                log::debug!("réglages : essai de {}", brain.model());
                let (keys, client) = (Arc::clone(&self.keys), Arc::clone(&self.client));
                let closing = Arc::clone(&self.closing);
                in_background("capri-essai", move || {
                    // la même personnalité que dans sa bulle
                    let persona = directory
                        .as_deref()
                        .map_or_else(|_| persona::DEFAULT.to_owned(), brain_setup::load_persona);
                    let talked = brain_setup::talk(
                        &*keys,
                        &client,
                        &brain,
                        &persona,
                        &message,
                        &closing,
                        &mut |piece| {
                            send(Outcome::TalkPiece { brain: brain.clone(), text: piece.to_owned() });
                        },
                    );
                    let result = talked.map(|talked| talked.total.as_secs_f32()).map_err(text);
                    send(Outcome::Talked { brain, result });
                });
            }
            Effect::LoadVoice => {
                let settings = directory.and_then(|directory| voice_setup::load(&directory).map_err(text));
                send(Outcome::VoiceLoaded { settings, voices: voice_setup::installed() });
            }
            Effect::SaveVoice(settings) => {
                log::info!("réglages : voix {}", if settings.enabled { "activée" } else { "coupée" });
                let saved = directory
                    .and_then(|directory| voice_setup::save(&directory, &settings).map_err(text))
                    .map(|_| settings.clone());
                let change = saved.is_ok().then_some(Change::Voice { enabled: settings.enabled });
                send(Outcome::VoiceSaved(saved));
                return change;
            }
            Effect::SayAloud { voice, text } => {
                if let Some(speaker) = &self.voice {
                    speaker.stop();
                    speaker.say(&text, voice.as_deref());
                }
            }
        }
        None
    }
}

impl Drop for Effects {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Relaxed);
        if let Some(voice) = &self.voice {
            voice.stop();
        }
    }
}

/// Le message d'une erreur, tel que l'afficherait la ligne de commande.
fn text(error: anyhow::Error) -> String {
    format!("{error:#}")
}

/// Lance `job` sur un thread à part. S'il ne peut pas être lancé, la fenêtre reste sur « en
/// cours », et le journal le dit.
fn in_background(name: &str, job: impl FnOnce() + Send + 'static) {
    if let Err(err) = std::thread::Builder::new().name(name.to_owned()).spawn(job) {
        log::error!("réglages : impossible de lancer {name} : {err}");
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use capri_chat::settings::brain as rubric;
    use capri_chat::settings::voice::ENABLED;
    use capri_chat::{Brain, Endpoint, KeyAccount, Provider};
    use capri_core::form::{Control, Secret};

    use super::*;
    use crate::fake_server::{self, header};
    use crate::keys::memory::MemoryKeys;

    /// Une clé qui n'existe nulle part ailleurs que dans ces tests.
    const KEY: &str = "sk-test-capri-reglages-0123456789";

    /// Un dossier de réglages vide, propre à ce test (aucun autre test n'utilise ce préfixe).
    fn scratch_dir(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("capri-test-reglages-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    /// La fenêtre sans la fenêtre : le modèle, et ce qui exécute ses effets, avec un trousseau en
    /// mémoire. Rien ne s'affiche, rien ne parle.
    struct Harness {
        settings: Settings,
        effects: Effects,
        results: Receiver<Done>,
    }

    impl Harness {
        fn open(directory: &Path, keys: Arc<MemoryKeys>) -> Self {
            let environment =
                Environment { keychain: Some(keys.name().to_owned()), tls: capri_net::TLS, voice: false };
            let (sender, results) = channel();
            let effects = Effects {
                sender,
                directory: Ok(directory.to_owned()),
                keys,
                client: Arc::new(Client::new()),
                voice: None,
                closing: Arc::new(AtomicBool::new(false)),
            };
            let mut harness = Self { settings: Settings::new(&environment), effects, results };
            let tasks = harness.settings.open();
            harness.run(tasks);
            harness.settle();
            harness
        }

        fn run(&mut self, tasks: Vec<Task>) {
            for task in tasks {
                let _ = self.effects.run(task);
            }
        }

        fn act(&mut self, event: FormEvent) {
            let tasks = self.settings.event(event);
            self.run(tasks);
            self.settle();
        }

        /// Laisse finir ce qui a été lancé, en relevant les résultats comme la fenêtre.
        fn settle(&mut self) {
            let deadline = Instant::now() + Duration::from_secs(10);
            while let Ok(done) = self.results.recv_timeout(Duration::from_millis(300)) {
                assert!(Instant::now() < deadline, "les effets ne finissent pas");
                let tasks = self.settings.done(done);
                self.run(tasks);
            }
        }

        fn choose_provider(&mut self, provider: Provider) {
            let index = 1 + Provider::ALL.iter().position(|&known| known == provider).expect("connu");
            self.act(FormEvent::Chose { id: rubric::PROVIDER.to_owned(), index });
        }

        fn press(&mut self, id: &str, secret: Option<&str>) {
            self.act(FormEvent::Pressed {
                id: id.to_owned(),
                secret: secret.map(|s| Secret::new(s.to_owned())),
            });
        }

        fn control(&self, id: &str) -> Control {
            self.settings.form().row(id).unwrap_or_else(|| panic!("pas de ligne {id}")).control.clone()
        }

        fn note(&self, id: &str) -> String {
            match self.control(id) {
                Control::Note { text, .. } | Control::Text { text } => text,
                other => panic!("{id} n'est pas un texte : {other:?}"),
            }
        }
    }

    /// Un fichier du dossier contient-il ce texte ?
    fn any_file_contains(directory: &Path, needle: &str) -> bool {
        std::fs::read_dir(directory).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                std::fs::read(entry.path())
                    .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(needle))
            })
        })
    }

    fn next(requests: &Receiver<fake_server::Request>) -> fake_server::Request {
        requests.recv_timeout(Duration::from_secs(5)).expect("une requête")
    }

    #[test]
    fn a_key_given_in_the_window_is_kept_for_the_address_shown_and_written_nowhere_else() {
        let directory = scratch_dir("cle");
        let (address, requests) =
            fake_server::serve(vec![fake_server::listing(&["labo/un"]), fake_server::listing(&["labo/un"])]);
        let url = format!("http://{address}/v1");
        let keys = Arc::new(MemoryKeys::default());
        let mut window = Harness::open(&directory, Arc::clone(&keys));

        window.choose_provider(Provider::Custom);
        window.act(FormEvent::Committed { id: rubric::URL.to_owned(), value: url.clone() });
        assert_eq!(
            window.note(rubric::KEY_STATUS),
            "Aucune clé pour cette adresse (il n'en faut une que si le serveur en demande)."
        );
        // un serveur de cette machine : sa liste est demandée d'elle-même, sans clé
        assert_eq!(header(&next(&requests).0, "authorization"), None);

        window.press(rubric::KEY_SAVE, Some(&format!("  {KEY}\n")));
        assert_eq!(keys.accounts(), [format!("custom@{address}")], "liée à l'adresse affichée");
        assert_eq!(keys.secret(&format!("custom@{address}")).as_deref(), Some(KEY), "sans les blancs autour");
        assert_eq!(
            window.note(rubric::KEY_NOTE),
            format!(
                "Clé du serveur personnalisé rangée dans le trousseau de test, pour {address} seulement : elle ne \
                 partira vers aucune autre adresse, et n'est écrite nulle part ailleurs."
            )
        );
        assert_eq!(
            window.note(rubric::KEY_STATUS),
            format!(
                "Une clé est rangée pour {address}, dans le trousseau de test. Elle ne s'affiche jamais."
            ),
            "relu dans le trousseau"
        );

        // elle part avec la liste des modèles, dans son en-tête
        window.press(rubric::MODELS_ASK, None);
        assert_eq!(header(&next(&requests).0, "authorization"), Some(format!("Bearer {KEY}")));

        // le seul modèle du serveur est pris d'office ; le cerveau choisi est celui que lit la
        // ligne de commande
        assert!(
            matches!(window.control(rubric::MODEL), Control::Choice { options, selected: 0, .. } if options == ["labo/un"])
        );
        window.press(rubric::USE, None);
        let chosen = brain_setup::load(&directory).expect("lisible").expect("un cerveau").brain;
        assert_eq!(chosen, Brain::new(Provider::Custom, Some(&url), "labo/un").expect("valable"));

        assert!(!any_file_contains(&directory, KEY), "la clé n'est écrite dans aucun fichier");
        assert!(!format!("{:?}", window.settings.form()).contains(KEY), "ni dans la fenêtre");

        // la retirer
        window.press(rubric::KEY_FORGET, None);
        assert!(keys.accounts().is_empty());
        assert!(window.note(rubric::KEY_STATUS).starts_with("Aucune clé pour cette adresse"));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn the_window_shows_what_the_command_line_chose_and_tries_it_like_capri_chat_test() {
        let directory = scratch_dir("essai");
        let refusal = format!(r#"{{"error":"bad key {KEY}"}}"#);
        let (address, requests) = fake_server::serve(vec![
            fake_server::listing(&["modele-de-test"]),
            fake_server::reply("Salut, **toi**."),
            fake_server::response("401 Unauthorized", "application/json", &refusal),
        ]);
        let url = format!("http://{address}/v1");
        brain_setup::save(
            &directory,
            &Brain::new(Provider::Custom, Some(&url), "modele-de-test").expect("valable"),
        )
        .expect("écrit");
        let keys = Arc::new(MemoryKeys::with(Provider::Custom, &url, KEY));
        let mut window = Harness::open(&directory, keys);

        assert_eq!(
            window.note(rubric::NOW),
            format!("Capri pense avec modele-de-test (Serveur personnalisé, {address}).")
        );
        assert_eq!(window.note(rubric::WHERE), "Ce que tu lui dis reste sur cette machine.");
        assert!(window.note(rubric::KEY_STATUS).starts_with(&format!("Une clé est rangée pour {address}")));
        let (head, _) = next(&requests);
        assert!(head.starts_with("GET /v1/models HTTP/1.1"), "{head}");

        window.press(rubric::TALK, None);
        assert_eq!(
            window.note(rubric::TALK_TEXT),
            "> Présente-toi en une phrase.\nSalut, toi.",
            "comme sa bulle"
        );
        assert!(window.note(rubric::TALK_NOTE).starts_with("Réponse en "));
        let (head, body) = next(&requests);
        assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));
        assert!(!body.contains(KEY));

        // une clé refusée : le message de `capri chat test`, sans la clé
        window.press(rubric::TALK, None);
        assert_eq!(
            window.note(rubric::TALK_NOTE),
            format!("{address} refuse ma clé (401). Pour la remettre : capri chat key custom")
        );
        assert!(!format!("{:?}", window.settings.form()).contains(KEY));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_key_kept_for_one_port_is_never_sent_to_another_from_the_window() {
        let directory = scratch_dir("autre-port");
        let (given_for, given_requests) = fake_server::serve(vec![fake_server::listing(&["m"])]);
        let (other, requests) =
            fake_server::serve(vec![fake_server::listing(&["m"]), fake_server::reply("Oui.")]);
        let keys = Arc::new(MemoryKeys::with(Provider::Custom, &format!("http://{given_for}/v1"), KEY));
        let mut window = Harness::open(&directory, Arc::clone(&keys));
        window.choose_provider(Provider::Custom);
        window.act(FormEvent::Committed { id: rubric::URL.to_owned(), value: format!("http://{other}/v1") });
        assert!(window.note(rubric::KEY_STATUS).starts_with("Aucune clé pour cette adresse"));
        let (head, body) = next(&requests);
        assert!(header(&head, "authorization").is_none() && !head.contains(KEY) && !body.contains(KEY));
        window.act(FormEvent::Chose { id: rubric::MODEL.to_owned(), index: 0 });
        window.press(rubric::TALK, None);
        let (head, body) = next(&requests);
        assert!(header(&head, "authorization").is_none() && !head.contains(KEY) && !body.contains(KEY));
        assert!(
            given_requests.recv_timeout(Duration::from_millis(100)).is_err(),
            "ni là où elle a été donnée"
        );
        let account = KeyAccount::new(
            Provider::Custom,
            &Endpoint::parse(&format!("http://{given_for}/v1")).expect("valable"),
        );
        assert_eq!(keys.accounts(), [account.name()], "rien d'autre n'a été rangé");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_provider_that_needs_a_key_is_not_contacted_from_the_window_without_one() {
        let directory = scratch_dir("sans-cle");
        let mut window = Harness::open(&directory, Arc::new(MemoryKeys::default()));
        window.choose_provider(Provider::OllamaCloud);
        assert!(
            window
                .note(rubric::WHERE)
                .starts_with("Attention : ce que tu lui dis partira chez Ollama en ligne")
        );
        assert_eq!(
            window.note(rubric::KEY_STATUS),
            "Aucune clé pour cette adresse : Ollama en ligne en demande une."
        );
        // le bouton est éteint, et l'appuyer quand même ne contacte personne
        assert!(
            matches!(window.control(rubric::MODELS_ACTIONS), Control::Buttons(buttons) if !buttons[0].enabled)
        );
        window.press(rubric::MODELS_ASK, None);
        window.press(rubric::TALK, None);
        assert!(
            !window.note(rubric::MODELS_NOTE).contains("demandée à"),
            "{}",
            window.note(rubric::MODELS_NOTE)
        );
        assert_eq!(brain_setup::load(&directory).expect("lisible"), None, "aucun cerveau choisi");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn the_voice_switched_in_the_window_is_what_capri_voice_reads() {
        let directory = scratch_dir("voix");
        let mut window = Harness::open(&directory, Arc::new(MemoryKeys::default()));
        // sans voix sur ce « système » (aucune n'est créée en test), rien ne change
        window.act(FormEvent::Toggled { id: ENABLED.to_owned(), checked: true });
        assert!(!voice_setup::load(&directory).expect("lisible").enabled);
        // avec : le fichier de `capri voice` est écrit
        window.settings = Settings::new(&Environment { keychain: None, tls: true, voice: true });
        let tasks = window.settings.open();
        window.run(tasks);
        window.settle();
        let tasks = window.settings.event(FormEvent::Toggled { id: ENABLED.to_owned(), checked: true });
        let changes: Vec<Change> = tasks.into_iter().filter_map(|task| window.effects.run(task)).collect();
        window.settle();
        assert_eq!(changes, [Change::Voice { enabled: true }], "l'app doit accorder son menu");
        assert!(voice_setup::load(&directory).expect("lisible").enabled);
        let _ = std::fs::remove_dir_all(directory);
    }
}
