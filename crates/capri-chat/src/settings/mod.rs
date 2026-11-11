//! Le modèle de la fenêtre de réglages : de la logique pure, testée sans écran.
//!
//! À partir des réglages actuels, de ce que l'utilisateur fait dans la fenêtre
//! ([`FormEvent`]) et du résultat des opérations lancées ([`Outcome`]), il produit le formulaire
//! à afficher ([`Settings::form`]) et les opérations à lancer ([`Effect`]) : relire un fichier,
//! demander la liste des modèles, ranger une clé, essayer un cerveau… Il ne touche à rien
//! lui-même : `capri-app` exécute les effets (hors du thread principal quand ils attendent le
//! réseau ou le trousseau) et lui rend leurs résultats. C'est ici que vivent les règles et les
//! textes de la fenêtre, à partir des mêmes règles et des mêmes phrases que la ligne de commande
//! (`brain`, `key`, `phrases`, `voice`).
//!
//! La fenêtre est faite de rubriques ([`Rubric`]), une par onglet : [`brain`], [`voice`],
//! [`claude`]. Ajouter une rubrique, c'est écrire un module comme eux et l'inscrire dans
//! [`Settings::new`] ; si elle
//! demande au système quelque chose qu'aucun [`Effect`] ne sait encore faire, il faut aussi la
//! variante et son exécution dans `capri-app` (`settings_view.rs`).
//!
//! Les identifiants des lignes commencent par celui de leur rubrique (`brain.provider`) : c'est
//! ainsi qu'un événement trouve la sienne.

use capri_claude::settings::Status;
use capri_claude::style::{Installed, Removed, StyleState};
use capri_core::form::{Form, FormEvent, Row, Section};
use capri_core::sound::Delivery;

use crate::brain::{Brain, Provider};
use crate::endpoint::Endpoint;
use crate::key::{ApiKey, KeyAccount};
use crate::voice::{VoiceInfo, VoiceSettings};

pub mod brain;
pub mod claude;
pub mod voice;

/// Le titre de la fenêtre.
pub const TITLE: &str = "Réglages de Capri";

/// Ce que le système sait faire, une fois pour toutes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    /// Le nom du trousseau du système, s'il en a un dont Capri sait se servir : « le trousseau de
    /// session ».
    pub keychain: Option<String>,
    /// Ce système sait-il parler `https://` ?
    pub tls: bool,
    /// Capri a-t-elle une voix sur ce système ?
    pub voice: bool,
}

/// Une opération à lancer pour une rubrique. Ce qui demande le réseau ou le trousseau se fait
/// hors du thread principal ; chaque effet a son [`Outcome`], sauf [`Effect::SayAloud`].
#[derive(Debug)]
pub enum Effect {
    /// Relire le cerveau choisi (`chat.json`) : [`Outcome::BrainLoaded`].
    LoadBrain,
    /// Le choisir : l'écrire dans `chat.json` (`capri chat use`) : [`Outcome::BrainUsed`].
    UseBrain(Brain),
    /// Lui retirer son cerveau (`capri chat off`) : [`Outcome::BrainDropped`].
    DropBrain,
    /// Une clé est-elle rangée pour ce compte ? Sans la lire : [`Outcome::KeyChecked`].
    CheckKey(KeyAccount),
    /// Ranger cette clé pour ce compte (`capri chat key`) : [`Outcome::KeyStored`].
    StoreKey { account: KeyAccount, key: ApiKey },
    /// La retirer (`capri chat key --forget`) : [`Outcome::KeyForgotten`].
    ForgetKey(KeyAccount),
    /// Demander au serveur la liste de ses modèles (`capri chat models`), avec la clé rangée pour
    /// cette adresse s'il en faut une : [`Outcome::ModelsListed`]. `chosen` : le cerveau des
    /// réglages, pour les conseils des messages.
    ListModels { provider: Provider, endpoint: Endpoint, chosen: Option<Brain> },
    /// Dire `message` à ce cerveau (`capri chat test`) : des [`Outcome::TalkPiece`], puis
    /// [`Outcome::Talked`].
    Talk { brain: Brain, message: String },
    /// Relire les réglages de la voix (`voice.json`) et les voix installées :
    /// [`Outcome::VoiceLoaded`].
    LoadVoice,
    /// Écrire ces réglages de la voix : [`Outcome::VoiceSaved`].
    SaveVoice(VoiceSettings),
    /// Dire `text` à voix haute, ainsi (voix, hauteur, débit, effets). Sans résultat.
    SayAloud { delivery: Delivery, text: String },
    /// Relire ce que sait la rubrique « Claude Code » (`capri hook status`) : le branchement dans
    /// `settings.json`, le miroir, le style : [`Outcome::ClaudeLoaded`].
    LoadClaude,
    /// Brancher le hook (`capri hook install`), ou le débrancher (`capri hook uninstall`) :
    /// [`Outcome::HookChanged`].
    ChangeHook { plug: bool },
    /// Activer ou désactiver le miroir (`capri hook mirror on|off`) : [`Outcome::MirrorSaved`].
    SaveMirror(bool),
    /// Installer le style de sortie (`capri hook style install`) : [`Outcome::StyleInstalled`].
    InstallStyle,
    /// Le retirer (`capri hook style uninstall`) : [`Outcome::StyleRemoved`].
    RemoveStyle,
}

/// Ce que la fenêtre sait du branchement de Claude Code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookState {
    /// `settings.json`, tel qu'il s'affiche.
    pub file: String,
    pub status: Status,
    /// Un des binaires qu'appellent ces hooks est introuvable.
    pub missing_binary: bool,
}

/// Ce qu'a fait un branchement ou un débranchement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookChange {
    /// L'état relu après coup.
    pub state: HookState,
    /// Le fichier a-t-il changé ?
    pub changed: bool,
    /// La copie de l'original, s'il y en a une.
    pub backup: Option<String>,
}

/// Le fichier du style de sortie, et ce qu'on y trouve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleFile {
    /// Son chemin, tel qu'il s'affiche.
    pub file: String,
    pub state: StyleState,
}

/// Le résultat d'un [`Effect`]. Une erreur est le message à montrer, tel que le dirait la ligne
/// de commande.
#[derive(Debug)]
pub enum Outcome {
    BrainLoaded(Result<Option<Brain>, String>),
    BrainUsed(Result<Brain, String>),
    /// `true` : il y avait un cerveau à retirer.
    BrainDropped(Result<bool, String>),
    /// `legacy` : une ancienne clé de ce fournisseur, rangée sans adresse, est là aussi.
    KeyChecked {
        account: KeyAccount,
        present: Result<bool, String>,
        legacy: bool,
    },
    KeyStored {
        account: KeyAccount,
        result: Result<LegacyKey, String>,
    },
    KeyForgotten {
        account: KeyAccount,
        result: Result<Forgotten, String>,
    },
    ModelsListed {
        provider: Provider,
        endpoint: Endpoint,
        result: Result<Vec<String>, String>,
    },
    /// Un morceau de la réponse à l'essai, déjà nettoyé comme pour sa bulle.
    TalkPiece {
        brain: Brain,
        text: String,
    },
    /// L'essai est fini : en tant de secondes, ou sur cette panne.
    Talked {
        brain: Brain,
        result: Result<f32, String>,
    },
    VoiceLoaded {
        settings: Result<VoiceSettings, String>,
        voices: Vec<VoiceInfo>,
    },
    VoiceSaved(Result<VoiceSettings, String>),
    /// `mirror` : le miroir est-il activé ?
    ClaudeLoaded {
        hook: Result<HookState, String>,
        mirror: Result<bool, String>,
        style: Result<StyleFile, String>,
    },
    HookChanged {
        plug: bool,
        result: Result<HookChange, String>,
    },
    /// Le miroir est maintenant activé (`true`) ou désactivé.
    MirrorSaved(Result<bool, String>),
    StyleInstalled(Result<(StyleFile, Installed), String>),
    StyleRemoved(Result<(StyleFile, Removed), String>),
}

/// Ce qu'il est advenu d'une ancienne clé, rangée sans adresse, quand on en range une nouvelle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LegacyKey {
    None,
    Removed,
    /// Elle n'a pas pu être retirée, pour cette raison.
    Kept(String),
}

/// Ce qu'un retrait de clé a trouvé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forgotten {
    pub key: bool,
    pub legacy: bool,
}

/// Un effet, avec la rubrique à qui rendre son résultat.
#[derive(Debug)]
pub struct Task {
    pub rubric: &'static str,
    pub effect: Effect,
}

/// Un résultat, pour la rubrique qui l'a demandé.
#[derive(Debug)]
pub struct Done {
    pub rubric: &'static str,
    pub outcome: Outcome,
}

/// Une rubrique de la fenêtre : un onglet.
pub trait Rubric {
    /// Son identifiant, qui commence celui de chacune de ses lignes.
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    /// Ses lignes, telles qu'elles doivent s'afficher maintenant.
    fn rows(&self) -> Vec<Row>;
    /// La fenêtre s'ouvre : ce qu'il faut relire.
    fn open(&mut self) -> Vec<Effect>;
    /// L'utilisateur a agi sur une de ses lignes.
    fn event(&mut self, event: FormEvent) -> Vec<Effect>;
    /// Un de ses effets est terminé.
    fn done(&mut self, outcome: Outcome) -> Vec<Effect>;
}

/// Toute la fenêtre : ses rubriques, dans l'ordre des onglets.
pub struct Settings {
    rubrics: Vec<Box<dyn Rubric>>,
}

impl Settings {
    /// Les rubriques de la fenêtre. Une nouvelle rubrique s'inscrit ici.
    pub fn new(environment: &Environment) -> Self {
        Self::with(vec![
            Box::new(brain::BrainRubric::new(environment)),
            Box::new(voice::VoiceRubric::new(environment)),
            Box::new(claude::ClaudeRubric::new()),
        ])
    }

    /// Une fenêtre faite de ces rubriques.
    pub fn with(rubrics: Vec<Box<dyn Rubric>>) -> Self {
        Self { rubrics }
    }

    /// Le formulaire à afficher maintenant.
    pub fn form(&self) -> Form {
        Form {
            title: TITLE.to_owned(),
            sections: self
                .rubrics
                .iter()
                .map(|rubric| Section {
                    id: rubric.id().to_owned(),
                    title: rubric.title().to_owned(),
                    rows: rubric.rows(),
                })
                .collect(),
        }
    }

    /// La fenêtre s'ouvre : ce que chaque rubrique veut relire.
    pub fn open(&mut self) -> Vec<Task> {
        let mut tasks = Vec::new();
        for rubric in &mut self.rubrics {
            let id = rubric.id();
            tasks.extend(rubric.open().into_iter().map(|effect| Task { rubric: id, effect }));
        }
        tasks
    }

    /// L'utilisateur a agi : la rubrique de la ligne concernée s'en occupe.
    pub fn event(&mut self, event: FormEvent) -> Vec<Task> {
        let Some(owner) = event.id().and_then(|id| id.split('.').next()) else { return Vec::new() };
        let Some(rubric) = self.rubrics.iter_mut().find(|rubric| rubric.id() == owner) else {
            return Vec::new();
        };
        let id = rubric.id();
        rubric.event(event).into_iter().map(|effect| Task { rubric: id, effect }).collect()
    }

    /// Un effet est terminé : sa rubrique en tient compte.
    pub fn done(&mut self, done: Done) -> Vec<Task> {
        let Some(rubric) = self.rubrics.iter_mut().find(|rubric| rubric.id() == done.rubric) else {
            return Vec::new();
        };
        let id = rubric.id();
        rubric.done(done.outcome).into_iter().map(|effect| Task { rubric: id, effect }).collect()
    }
}

#[cfg(test)]
mod tests {
    use capri_core::form::Control;

    use super::*;

    /// Une rubrique qui note ce qu'on lui envoie.
    struct Probe {
        id: &'static str,
        events: Vec<String>,
    }

    impl Rubric for Probe {
        fn id(&self) -> &'static str {
            self.id
        }

        fn title(&self) -> &'static str {
            "Sonde"
        }

        fn rows(&self) -> Vec<Row> {
            vec![Row::new(format!("{}.seen", self.id), "", Control::Text { text: self.events.join(",") })]
        }

        fn open(&mut self) -> Vec<Effect> {
            vec![Effect::LoadVoice]
        }

        fn event(&mut self, event: FormEvent) -> Vec<Effect> {
            self.events.push(event.id().unwrap_or("?").to_owned());
            vec![Effect::DropBrain]
        }

        fn done(&mut self, _outcome: Outcome) -> Vec<Effect> {
            self.events.push("fait".to_owned());
            Vec::new()
        }
    }

    fn probe(id: &'static str) -> Box<dyn Rubric> {
        Box::new(Probe { id, events: Vec::new() })
    }

    #[test]
    fn each_event_and_each_result_goes_to_its_own_rubric() {
        let mut settings = Settings::with(vec![probe("un"), probe("deux")]);
        let opened = settings.open();
        assert_eq!(opened.iter().map(|task| task.rubric).collect::<Vec<_>>(), ["un", "deux"]);

        let tasks = settings.event(FormEvent::Toggled { id: "deux.case".to_owned(), checked: true });
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].rubric, "deux");
        // ni une ligne inconnue, ni la fermeture, ne concernent une rubrique
        assert!(settings.event(FormEvent::Toggled { id: "trois.case".to_owned(), checked: true }).is_empty());
        assert!(settings.event(FormEvent::Closed).is_empty());
        // un identifiant qui commence seulement pareil n'est pas à elle
        assert!(
            settings.event(FormEvent::Toggled { id: "deuxieme.case".to_owned(), checked: true }).is_empty()
        );

        settings.done(Done { rubric: "un", outcome: Outcome::BrainDropped(Ok(true)) });
        let form = settings.form();
        assert_eq!(form.title, TITLE);
        assert_eq!(
            form.sections.iter().map(|section| section.id.as_str()).collect::<Vec<_>>(),
            ["un", "deux"]
        );
        assert_eq!(
            form.row("un.seen").map(|row| &row.control),
            Some(&Control::Text { text: "fait".to_owned() })
        );
        assert_eq!(
            form.row("deux.seen").map(|row| &row.control),
            Some(&Control::Text { text: "deux.case".to_owned() })
        );
    }

    #[test]
    fn the_real_window_has_its_rubrics_and_no_id_twice() {
        let environment =
            Environment { keychain: Some("le trousseau de test".to_owned()), tls: true, voice: true };
        let mut settings = Settings::new(&environment);
        let tasks = settings.open();
        assert!(matches!(
            tasks.as_slice(),
            [
                Task { rubric: "brain", effect: Effect::LoadBrain },
                Task { rubric: "voice", effect: Effect::LoadVoice },
                Task { rubric: "claude", effect: Effect::LoadClaude },
            ]
        ));
        let form = settings.form();
        assert_eq!(
            form.sections.iter().map(|section| section.title.as_str()).collect::<Vec<_>>(),
            ["Cerveau", "Voix", "Claude Code"]
        );
        assert!(form.duplicate_ids().is_empty(), "{:?}", form.duplicate_ids());
        for section in &form.sections {
            for row in &section.rows {
                assert!(row.id.starts_with(&format!("{}.", section.id)), "{}", row.id);
            }
        }
    }
}
