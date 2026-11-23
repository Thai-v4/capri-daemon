//! Installer une version, **sur un geste** : ce qu'il faut décider, sans rien toucher.
//!
//! - [`plan`] : ce qu'il faut télécharger, et où le mettre dans la nouvelle app. Seulement une
//!   version installable (`Verdict::Installable`), dont la cible macOS donne **exactement**
//!   `capri`, `capri-hook` et `Info.plist` ;
//! - [`check_info`] : l'`Info.plist` reçu porte notre identifiant de bundle, la version du
//!   manifeste, et nomme notre programme ;
//! - [`Installation`] : le déroulé, une machine à états essayée sans système. La ligne du menu
//!   choisie ([`Installation::chosen`]) lance la préparation (télécharger, vérifier, refaire l'app
//!   à côté de l'installée, la signer), ce que la préparation rapporte la fait avancer, et l'app
//!   prête, la même ligne propose de redémarrer pour finir. Ce qu'elle dit à chaque pas, et ce que
//!   dit la ligne, en viennent ;
//! - [`Refusal`] : chaque refus, avec sa phrase (`say`) et sa forme pour le journal.
//!
//! Le travail (réseau, disque, signature) est fait ailleurs, hors du thread principal ; rien ne
//! se fait sans geste : d'elle-même, elle ne fait que lire le manifeste.

use capri_core::bundle::{self, BUNDLE_ID, HOOK_PROGRAM, PROGRAM};

use super::manifest::{Entry, Manifest};
use super::{Failure, Installed, Obstacle, Verdict, Version, verdict};

/// La cible des versions que l'app sait installer elle-même.
pub const MACOS_TARGET: &str = "macos-arm64";
/// Le nom de l'`Info.plist` parmi les fichiers d'une version.
pub const INFO_PLIST: &str = "Info.plist";
/// La taille d'un `Info.plist`, au plus : il est relu en entier pour être vérifié.
pub const MAX_INFO_BYTES: u64 = 64 * 1024;

/// Où va un fichier dans la nouvelle app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// `Contents/MacOS/capri`, exécutable.
    Program,
    /// `Contents/MacOS/capri-hook`, exécutable.
    HookProgram,
    /// `Contents/Info.plist`.
    Info,
}

impl Slot {
    /// Son chemin dans l'app, morceau par morceau.
    pub fn components(self) -> &'static [&'static str] {
        match self {
            Self::Program => &["Contents", "MacOS", PROGRAM],
            Self::HookProgram => &["Contents", "MacOS", HOOK_PROGRAM],
            Self::Info => &["Contents", INFO_PLIST],
        }
    }

    pub fn is_program(self) -> bool {
        !matches!(self, Self::Info)
    }

    fn of(name: &str) -> Option<Self> {
        match name {
            PROGRAM => Some(Self::Program),
            HOOK_PROGRAM => Some(Self::HookProgram),
            INFO_PLIST => Some(Self::Info),
            _ => None,
        }
    }
}

/// Un fichier à télécharger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub entry: Entry,
    pub slot: Slot,
}

/// Ce qu'il faut télécharger : les fichiers d'une version, dans l'ordre où les télécharger (les
/// programmes, puis l'`Info.plist`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub version: Version,
    pub files: Vec<Planned>,
}

/// Ce qu'il faut télécharger pour passer à la version du manifeste, depuis `installed`. Refusé :
/// une version pas plus récente, une version qui ne s'installe pas depuis l'app (son verdict), un
/// autre système que macOS, une cible qui ne donne pas exactement nos trois fichiers.
pub fn plan(manifest: &Manifest, installed: &Installed) -> Result<Plan, Refusal> {
    let latest = match verdict(manifest, installed) {
        Verdict::UpToDate { .. } => return Err(Refusal::UpToDate),
        Verdict::NotInstallable { latest, why } => return Err(Refusal::NotInstallable { latest, why }),
        Verdict::Installable { latest } => latest,
    };
    if installed.target != Some(MACOS_TARGET) {
        return Err(Refusal::Unsupported);
    }
    let Manifest::Known(release) = manifest else { return Err(Refusal::Files { latest }) };
    let target = release.target(MACOS_TARGET).ok_or(Refusal::Files { latest })?;
    let mut files: Vec<Planned> = Vec::new();
    for entry in &target.files {
        let slot = Slot::of(entry.name.as_str()).ok_or(Refusal::Files { latest })?;
        if slot == Slot::Info && entry.size > MAX_INFO_BYTES {
            return Err(Refusal::Files { latest });
        }
        files.push(Planned { entry: entry.clone(), slot });
    }
    // exactement les trois (le manifeste refuse déjà un nom en double)
    if files.len() != 3 {
        return Err(Refusal::Files { latest });
    }
    files.sort_by_key(|planned| planned.slot as u8);
    Ok(Plan { version: latest, files })
}

/// Ce qui ne va pas dans un `Info.plist` reçu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoProblem {
    /// Illisible (pas de l'UTF-8, pas un `Info.plist` en XML).
    Unreadable,
    /// Pas notre identifiant de bundle.
    NotOurs,
    /// Pas la version du manifeste.
    Version,
    /// Il ne lance pas notre programme.
    Executable,
}

/// L'`Info.plist` reçu (ses octets) est-il celui de Capri, à la version `version` ?
/// `CFBundleIdentifier` est le nôtre, `CFBundleShortVersionString` et `CFBundleVersion` sont la
/// version, `CFBundleExecutable` est notre programme.
pub fn check_info(bytes: &[u8], version: Version) -> Result<(), InfoProblem> {
    let text = std::str::from_utf8(bytes).map_err(|_| InfoProblem::Unreadable)?;
    let strings = bundle::info_strings(text).map_err(|_| InfoProblem::Unreadable)?;
    let get = |key: &str| strings.iter().find(|(found, _)| found == key).map(|(_, value)| value.as_str());
    if get("CFBundleIdentifier") != Some(BUNDLE_ID) {
        return Err(InfoProblem::NotOurs);
    }
    let version = version.to_string();
    if get("CFBundleShortVersionString") != Some(&version) || get("CFBundleVersion") != Some(&version) {
        return Err(InfoProblem::Version);
    }
    if get("CFBundleExecutable") != Some(PROGRAM) {
        return Err(InfoProblem::Executable);
    }
    Ok(())
}

/// Pourquoi la nouvelle app n'a pas pu être signée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignProblem {
    /// On ne sait pas comment l'app installée est signée.
    Unknown,
    /// Plusieurs identités valides portent le nom de la sienne : on ne choisit pas.
    Ambiguous(usize),
    /// Lire les identités du trousseau a échoué.
    Keychain,
    /// `codesign` a refusé de la signer (ou l'accès à la clé a été refusé).
    Failed,
    /// Sa signature ne se vérifie pas.
    Invalid,
}

/// Pourquoi rien n'est installé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Ailleurs que sur macOS : elle ne sait pas s'installer seule.
    Unsupported,
    /// Elle ne tourne pas depuis une `Capri.app`.
    NotInApp,
    /// Le dossier de son app n'est pas inscriptible pour elle.
    NotWritable,
    /// Elle est déjà à jour.
    UpToDate,
    /// La version ne s'installe pas depuis l'app.
    NotInstallable { latest: Version, why: Obstacle },
    /// La version ne donne pas exactement les fichiers attendus.
    Files { latest: Version },
    /// Son `Info.plist` n'est pas le nôtre, ou pas de cette version.
    Info(InfoProblem),
    /// Le téléchargement n'a pas abouti (taille ou empreinte fausses comprises).
    Download(Failure),
    /// L'app installée n'a pas pu être relue (sa signature ne se vérifie plus, ou elle contient
    /// autre chose que des fichiers et des dossiers) : ses ressources ne sont pas recopiées.
    Installed,
    /// La nouvelle app n'a pas pu être signée.
    Sign(SignProblem),
    /// Écrire sur le disque a échoué.
    Disk,
    /// On l'a arrêtée (case décochée, ou Capri quittée).
    Cancelled,
    /// La nouvelle version n'a pas pris le relais : l'ancienne reste.
    Handover,
}

impl Refusal {
    /// Bulle : une phrase courte, jamais une commande de terminal, sauf pour une version qui
    /// change ce que contient l'app (l'app ne sait pas faire cette installation-là).
    pub fn say(&self) -> String {
        match self {
            Self::Unsupported => "Je ne sais pas encore m'installer seule sur ce système".to_owned(),
            Self::NotInApp => "Je ne tourne pas depuis mon app : je ne peux pas m'y installer".to_owned(),
            Self::NotWritable => "Je n'ai pas le droit d'écrire à côté de mon app".to_owned(),
            Self::UpToDate => "Je suis déjà à jour".to_owned(),
            Self::NotInstallable { latest, why: Obstacle::Bundle { .. } } => {
                format!("La {latest} change mon app : réinstalle-moi par tools/install-macos.sh")
            }
            Self::NotInstallable { latest, why } => {
                super::say::detail(&Verdict::NotInstallable { latest: *latest, why: *why })
            }
            Self::Files { latest } => format!("La {latest} n'a pas les fichiers que j'attends"),
            Self::Info(_) => "Cette version ne se présente pas comme moi : je ne l'installe pas".to_owned(),
            Self::Download(Failure::WrongSize | Failure::WrongDigest) => {
                "Un fichier reçu est abîmé : je n'installe rien".to_owned()
            }
            Self::Download(_) => "Je n'ai pas pu télécharger la nouvelle version".to_owned(),
            Self::Installed => "Je n'arrive pas à relire mon app : je n'installe rien".to_owned(),
            Self::Sign(SignProblem::Ambiguous(_)) => {
                "Plusieurs identités portent le nom de ma signature : je ne choisis pas".to_owned()
            }
            Self::Sign(_) => "Je n'ai pas pu signer la nouvelle version".to_owned(),
            Self::Disk => "Je n'ai pas pu écrire la nouvelle version sur le disque".to_owned(),
            Self::Cancelled => "J'arrête l'installation".to_owned(),
            Self::Handover => "La nouvelle version n'a pas pris le relais : je reste".to_owned(),
        }
    }

    /// Journal : la cause, courte. Jamais une adresse, jamais une identité.
    pub fn for_log(&self) -> String {
        match self {
            Self::Unsupported => "pas sur ce système".to_owned(),
            Self::NotInApp => "pas lancée depuis une Capri.app".to_owned(),
            Self::NotWritable => "dossier de l'app non inscriptible".to_owned(),
            Self::UpToDate => "déjà à jour".to_owned(),
            Self::NotInstallable { latest, why } => {
                format!("la {latest} n'est pas installable depuis l'app : {}", super::say::obstacle(why))
            }
            Self::Files { latest } => {
                format!("la {latest} ne donne pas exactement capri, capri-hook et Info.plist pour macOS")
            }
            Self::Info(InfoProblem::Unreadable) => "Info.plist reçu illisible".to_owned(),
            Self::Info(InfoProblem::NotOurs) => {
                "Info.plist reçu : pas notre identifiant de bundle".to_owned()
            }
            Self::Info(InfoProblem::Version) => "Info.plist reçu : pas la version du manifeste".to_owned(),
            Self::Info(InfoProblem::Executable) => "Info.plist reçu : pas notre programme".to_owned(),
            Self::Download(failure) => format!("téléchargement impossible ({})", failure.for_log()),
            Self::Installed => "app installée illisible ou signature invalide".to_owned(),
            Self::Sign(SignProblem::Unknown) => "signature de l'app installée inconnue".to_owned(),
            Self::Sign(SignProblem::Ambiguous(count)) => {
                format!("{count} identités valides portent le nom de celle de l'app installée")
            }
            Self::Sign(SignProblem::Keychain) => "identités de signature illisibles".to_owned(),
            Self::Sign(SignProblem::Failed) => "codesign a refusé de signer".to_owned(),
            Self::Sign(SignProblem::Invalid) => {
                "la signature de la nouvelle app ne se vérifie pas".to_owned()
            }
            Self::Disk => "écriture impossible".to_owned(),
            Self::Cancelled => "arrêtée".to_owned(),
            Self::Handover => "le programme de fin n'a pas pris le relais".to_owned(),
        }
    }
}

/// Une étape de la préparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Le fichier `file` (à partir de 1) sur `of`, en cours de téléchargement.
    Download { file: usize, of: usize },
    /// L'app refaite à côté de l'installée : l'`Info.plist` vérifié, ses ressources recopiées.
    Assemble,
    /// La signature.
    Sign,
}

/// Comment la nouvelle app a été signée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signed {
    /// « Ad hoc », comme l'installée.
    AdHoc,
    /// Par la même identité que l'installée.
    Identity,
    /// « Ad hoc », faute de l'identité de l'installée dans le trousseau.
    AdHocFallback,
}

impl Signed {
    /// macOS redemandera-t-il le micro ? Une signature « ad hoc » change à chaque fois.
    pub fn asks_microphone_again(self) -> bool {
        !matches!(self, Self::Identity)
    }

    /// Journal.
    pub fn for_log(self) -> &'static str {
        match self {
            Self::AdHoc => "signée « ad hoc », comme l'app installée",
            Self::Identity => "signée par la même identité que l'app installée",
            Self::AdHocFallback => {
                "signée « ad hoc » : l'identité de l'app installée n'est plus dans le trousseau"
            }
        }
    }
}

/// Où en est l'installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Rien en cours.
    Idle,
    /// La préparation de `version` tourne.
    Preparing { version: Version, step: Step },
    /// La nouvelle app est prête, à côté : elle attend le geste qui redémarre Capri.
    Ready { version: Version, signed: Signed },
    /// Le programme de fin est lancé : s'il prend le relais, Capri se quitte.
    Restarting { version: Version, signed: Signed },
}

/// Ce que l'app doit lancer après un geste.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Order {
    /// Rien.
    Nothing,
    /// La préparation de ce plan, hors du thread principal.
    Prepare(Plan),
    /// Le programme de fin, puis se quitter s'il prend le relais.
    Restart,
}

/// La réponse à un geste : quoi lancer, et quoi dire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub order: Order,
    pub say: Option<String>,
}

impl Reply {
    fn say(text: impl Into<String>) -> Self {
        Self { order: Order::Nothing, say: Some(text.into()) }
    }
}

/// Ce que l'app sait quand on choisit la ligne du menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Here {
    /// Ce système sait-il l'installer (macOS) ?
    pub supported: bool,
    /// Tourne-t-elle depuis une `Capri.app` ?
    pub in_app: bool,
    pub installed: Installed,
}

/// Le déroulé d'une installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installation {
    state: State,
}

impl Default for Installation {
    fn default() -> Self {
        Self::new()
    }
}

impl Installation {
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// La ligne du menu vient d'être choisie. `found` : la dernière version trouvée et son
    /// manifeste, s'il y en a une.
    pub fn chosen(&mut self, found: Option<(&Verdict, &Manifest)>, here: &Here) -> Reply {
        match self.state {
            State::Preparing { version, .. } => {
                Reply::say(format!("Je suis déjà en train d'installer la {version}"))
            }
            State::Restarting { version, .. } => Reply::say(format!("Je redémarre avec la {version}…")),
            State::Ready { version, signed } => {
                self.state = State::Restarting { version, signed };
                Reply { order: Order::Restart, say: Some(format!("Je redémarre avec la {version}…")) }
            }
            State::Idle => {
                let Some((verdict, manifest)) = found else {
                    return Reply { order: Order::Nothing, say: None };
                };
                let refused = match *verdict {
                    Verdict::UpToDate { .. } => Some(Refusal::UpToDate),
                    Verdict::NotInstallable { latest, why } => Some(Refusal::NotInstallable { latest, why }),
                    Verdict::Installable { .. } if !here.supported => Some(Refusal::Unsupported),
                    Verdict::Installable { .. } if !here.in_app => Some(Refusal::NotInApp),
                    Verdict::Installable { .. } => None,
                };
                if let Some(refusal) = refused {
                    return Reply::say(refusal.say());
                }
                match plan(manifest, &here.installed) {
                    Ok(plan) => {
                        let (version, of) = (plan.version, plan.files.len());
                        self.state = State::Preparing { version, step: Step::Download { file: 1, of } };
                        Reply {
                            order: Order::Prepare(plan),
                            say: Some(format!("Je télécharge la {version}…")),
                        }
                    }
                    Err(refusal) => Reply::say(refusal.say()),
                }
            }
        }
    }

    /// La préparation rapporte une étape : ce qu'elle en dit, à chaque nouvelle sorte d'étape.
    pub fn progressed(&mut self, step: Step) -> Option<String> {
        let State::Preparing { version, step: before } = self.state else { return None };
        self.state = State::Preparing { version, step };
        let new_kind = std::mem::discriminant(&before) != std::mem::discriminant(&step);
        match step {
            Step::Download { .. } => None,
            Step::Assemble if new_kind => Some(format!("Je refais mon app avec la {version}…")),
            Step::Sign if new_kind => Some(format!("Je signe la {version}…")),
            Step::Assemble | Step::Sign => None,
        }
    }

    /// La préparation est finie : la nouvelle app est prête. Ce qu'elle en dit.
    pub fn prepared(&mut self, signed: Signed) -> Option<String> {
        let State::Preparing { version, .. } = self.state else { return None };
        self.state = State::Ready { version, signed };
        Some(if signed.asks_microphone_again() {
            format!("La {version} est prête ; macOS te redemandera le micro")
        } else {
            format!("La {version} est prête : redémarre-moi depuis mon menu")
        })
    }

    /// La préparation a échoué, ou le relais : ce qu'elle en dit (rien pour un arrêt demandé).
    pub fn failed(&mut self, refusal: &Refusal) -> Option<String> {
        match (self.state, refusal) {
            (State::Restarting { version, signed }, Refusal::Handover) => {
                // l'app prête reste là : on pourra réessayer
                self.state = State::Ready { version, signed };
                Some(refusal.say())
            }
            (State::Idle, _) => None,
            (_, Refusal::Cancelled) => {
                self.state = State::Idle;
                None
            }
            _ => {
                self.state = State::Idle;
                Some(refusal.say())
            }
        }
    }

    /// Tout est abandonné (case décochée, Capri quittée) : elle oublie.
    pub fn abandon(&mut self) {
        self.state = State::Idle;
    }

    /// La ligne du menu, d'après ce qui a été trouvé et où en est l'installation. `supported` :
    /// ce système sait-il l'installer ? Ailleurs, la ligne reste une information.
    pub fn menu_line(&self, found: Option<&Verdict>, supported: bool) -> Option<String> {
        match self.state {
            State::Preparing { version, .. } => Some(format!("Installation de la {version}…")),
            State::Ready { version, .. } => Some(format!("Redémarrer pour passer à la {version}")),
            State::Restarting { .. } => Some("Redémarrage…".to_owned()),
            State::Idle => match *found? {
                Verdict::UpToDate { .. } => None,
                Verdict::Installable { latest } if supported => {
                    Some(format!("Installer la version {latest}"))
                }
                ref verdict => verdict.newer().map(super::say::menu_line),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Failure;
    use super::super::manifest::tests::{A, example};
    use super::*;

    const HERE: Here = Here {
        supported: true,
        in_app: true,
        installed: Installed { version: Version::new(0, 1, 0), bundle: 1, target: Some(MACOS_TARGET) },
    };

    fn manifest(version: &str, bundle: u32) -> Manifest {
        Manifest::parse(example(version, bundle).as_bytes()).expect("lisible")
    }

    fn found(manifest: &Manifest) -> Verdict {
        verdict(manifest, &HERE.installed)
    }

    fn info(version: &str) -> String {
        include_str!("../../../../tools/macos/Info.plist").replace("@VERSION@", version)
    }

    #[test]
    fn a_plan_takes_exactly_our_three_files_programs_first() {
        let plan = plan(&manifest("0.2.0", 1), &HERE.installed).expect("installable");
        assert_eq!(plan.version, Version::new(0, 2, 0));
        let slots: Vec<Slot> = plan.files.iter().map(|planned| planned.slot).collect();
        assert_eq!(slots, [Slot::Program, Slot::HookProgram, Slot::Info]);
        assert_eq!(plan.files[0].entry.asset.as_str(), "capri-macos-arm64");
        assert_eq!(plan.files[2].entry.sha256.as_str(), A);
        assert_eq!(Slot::Program.components(), ["Contents", "MacOS", "capri"]);
        assert_eq!(Slot::Info.components(), ["Contents", "Info.plist"]);
        assert!(Slot::HookProgram.is_program() && !Slot::Info.is_program());
    }

    #[test]
    fn a_plan_is_refused_for_anything_but_an_installable_macos_version_with_our_three_files() {
        let latest = Version::new(0, 2, 0);
        assert_eq!(plan(&manifest("0.1.0", 1), &HERE.installed), Err(Refusal::UpToDate));
        assert!(matches!(
            plan(&manifest("0.2.0", 2), &HERE.installed),
            Err(Refusal::NotInstallable { why: Obstacle::Bundle { published: 2, installed: 1 }, .. })
        ));
        let windows = Installed { target: Some("windows-x86_64"), ..HERE.installed };
        assert_eq!(plan(&manifest("0.2.0", 1), &windows), Err(Refusal::Unsupported));
        // une cible macOS à qui il manque un fichier, ou qui en a un de trop, ou un inconnu
        let text = example("0.2.0", 1);
        let hook =
            r#"{ "name": "capri-hook", "asset": "capri-hook-macos-arm64", "size": 123456, "sha256": ""#;
        let line = text.lines().find(|line| line.contains(hook)).expect("la ligne du second programme");
        let cases = [
            text.replacen(&format!("{line}\n"), "", 1),
            text.replacen(
                hook,
                r#"{ "name": "capri-bis", "asset": "capri-hook-macos-arm64", "size": 123456, "sha256": ""#,
                1,
            ),
            text.replacen(
                r#"{ "name": "Info.plist", "asset": "Info.plist", "size": 1234,"#,
                &format!(
                    r#"{{ "name": "Info.plist", "asset": "Info.plist", "size": {},"#,
                    MAX_INFO_BYTES + 1
                ),
                1,
            ),
            text.replacen(
                r#""files": ["#,
                &format!(
                    r#""files": [ {{ "name": "autre", "asset": "autre", "size": 1, "sha256": "{A}" }},"#
                ),
                1,
            ),
        ];
        for case in cases {
            let manifest = Manifest::parse(case.as_bytes()).expect("lisible");
            assert_eq!(plan(&manifest, &HERE.installed), Err(Refusal::Files { latest }), "{case}");
        }
        let without_mac = Manifest::parse(
            format!(
                r#"{{"format": 1, "version": "0.2.0", "bundle": 1, "targets": {{"linux": {{"files": [
                {{"name": "capri", "asset": "x", "size": 1, "sha256": "{A}"}}]}}}}}}"#
            )
            .as_bytes(),
        )
        .expect("lisible");
        assert!(matches!(
            plan(&without_mac, &HERE.installed),
            Err(Refusal::NotInstallable { why: Obstacle::NoTarget, .. })
        ));
    }

    #[test]
    fn the_received_info_plist_must_be_ours_at_this_version() {
        let version = Version::new(0, 2, 0);
        assert_eq!(check_info(info("0.2.0").as_bytes(), version), Ok(()));
        assert_eq!(check_info(info("0.2.1").as_bytes(), version), Err(InfoProblem::Version));
        let other = info("0.2.0").replace(BUNDLE_ID, "com.example.autre");
        assert_eq!(check_info(other.as_bytes(), version), Err(InfoProblem::NotOurs));
        let half = info("0.2.0").replacen("<string>0.2.0</string>", "<string>0.1.0</string>", 1);
        assert_eq!(check_info(half.as_bytes(), version), Err(InfoProblem::Version));
        let launcher = info("0.2.0").replace("<string>capri</string>", "<string>autre</string>");
        assert_eq!(check_info(launcher.as_bytes(), version), Err(InfoProblem::Executable));
        for unreadable in [&b"\xff\xfe"[..], b"bplist00", b""] {
            assert_eq!(check_info(unreadable, version), Err(InfoProblem::Unreadable));
        }
    }

    #[test]
    fn the_menu_line_starts_the_preparation_then_offers_to_restart() {
        let manifest = manifest("0.2.0", 1);
        let verdict = found(&manifest);
        let mut install = Installation::new();
        assert_eq!(install.menu_line(Some(&verdict), true).as_deref(), Some("Installer la version 0.2.0"));
        let reply = install.chosen(Some((&verdict, &manifest)), &HERE);
        let Order::Prepare(plan) = reply.order else { panic!("la préparation : {reply:?}") };
        assert_eq!(plan.files.len(), 3);
        assert_eq!(reply.say.as_deref(), Some("Je télécharge la 0.2.0…"));
        assert_eq!(install.menu_line(Some(&verdict), true).as_deref(), Some("Installation de la 0.2.0…"));
        // la ligne choisie de nouveau pendant la préparation : rien de plus n'est lancé
        let again = install.chosen(Some((&verdict, &manifest)), &HERE);
        assert_eq!(again.order, Order::Nothing);
        assert_eq!(again.say.as_deref(), Some("Je suis déjà en train d'installer la 0.2.0"));
        // les étapes : une phrase par sorte d'étape seulement
        assert_eq!(install.progressed(Step::Download { file: 2, of: 3 }), None);
        assert_eq!(install.progressed(Step::Download { file: 3, of: 3 }), None);
        assert_eq!(install.progressed(Step::Assemble).as_deref(), Some("Je refais mon app avec la 0.2.0…"));
        assert_eq!(install.progressed(Step::Assemble), None);
        assert_eq!(install.progressed(Step::Sign).as_deref(), Some("Je signe la 0.2.0…"));
        assert_eq!(
            install.prepared(Signed::Identity).as_deref(),
            Some("La 0.2.0 est prête : redémarre-moi depuis mon menu")
        );
        assert_eq!(
            install.menu_line(Some(&verdict), true).as_deref(),
            Some("Redémarrer pour passer à la 0.2.0")
        );
        // le geste qui redémarre
        let restart = install.chosen(Some((&verdict, &manifest)), &HERE);
        assert_eq!(restart.order, Order::Restart);
        assert_eq!(
            install.state(),
            State::Restarting { version: Version::new(0, 2, 0), signed: Signed::Identity }
        );
        assert_eq!(
            install.chosen(Some((&verdict, &manifest)), &HERE).order,
            Order::Nothing,
            "une seule fois"
        );
        // la nouvelle ne prend pas le relais : l'app prête reste, on peut réessayer
        assert!(install.failed(&Refusal::Handover).is_some());
        assert_eq!(
            install.state(),
            State::Ready { version: Version::new(0, 2, 0), signed: Signed::Identity }
        );
        assert_eq!(install.chosen(Some((&verdict, &manifest)), &HERE).order, Order::Restart);
    }

    #[test]
    fn an_ad_hoc_signature_says_macos_will_ask_for_the_microphone_again() {
        let manifest = manifest("0.2.0", 1);
        let verdict = found(&manifest);
        for signed in [Signed::AdHoc, Signed::AdHocFallback] {
            let mut install = Installation::new();
            install.chosen(Some((&verdict, &manifest)), &HERE);
            let said = install.prepared(signed).expect("une phrase");
            assert!(said.contains("micro"), "{said}");
            assert!(signed.asks_microphone_again());
        }
        assert!(!Signed::Identity.asks_microphone_again());
        assert!(Signed::AdHocFallback.for_log().contains("trousseau"));
    }

    #[test]
    fn each_refusal_is_said_and_the_installation_goes_back_to_rest() {
        let manifest = manifest("0.2.0", 1);
        let verdict = found(&manifest);
        for refusal in refusals() {
            let mut install = Installation::new();
            install.chosen(Some((&verdict, &manifest)), &HERE);
            let said = install.failed(&refusal);
            assert_eq!(install.state(), State::Idle, "{refusal:?}");
            match refusal {
                Refusal::Cancelled => assert_eq!(said, None, "un arrêt demandé ne se dit pas"),
                _ => assert_eq!(said, Some(refusal.say())),
            }
            assert_eq!(
                install.menu_line(Some(&verdict), true).as_deref(),
                Some("Installer la version 0.2.0")
            );
        }
        // rien en cours : un rapport tardif ne fait rien
        let mut idle = Installation::new();
        assert_eq!(idle.failed(&Refusal::Disk), None);
        assert_eq!(idle.prepared(Signed::AdHoc), None);
        assert_eq!(idle.progressed(Step::Sign), None);
    }

    #[test]
    fn the_line_refuses_on_the_spot_what_cannot_be_installed_here() {
        let installable = manifest("0.2.0", 1);
        let verdict = found(&installable);
        let cases = [
            (Here { supported: false, ..HERE }, Refusal::Unsupported),
            (Here { in_app: false, ..HERE }, Refusal::NotInApp),
        ];
        for (here, refusal) in cases {
            let mut install = Installation::new();
            let reply = install.chosen(Some((&verdict, &installable)), &here);
            assert_eq!(reply, Reply { order: Order::Nothing, say: Some(refusal.say()) });
            assert_eq!(install.state(), State::Idle);
        }
        // ailleurs que sur macOS, la ligne reste une information
        assert_eq!(
            Installation::new().menu_line(Some(&verdict), false).as_deref(),
            Some("Version 0.2.0 disponible")
        );
        // une version qui change ce que contient l'app : la phrase renvoie au script d'installation
        let bundle = manifest("0.2.0", 2);
        let mut install = Installation::new();
        let reply = install.chosen(Some((&found(&bundle), &bundle)), &HERE);
        assert_eq!(reply.order, Order::Nothing);
        assert!(
            reply.say.as_deref().is_some_and(|said| said.contains("tools/install-macos.sh")),
            "{reply:?}"
        );
        assert_eq!(
            install.menu_line(Some(&found(&bundle)), true).as_deref(),
            Some("Version 0.2.0 disponible")
        );
        // à jour : pas de ligne
        let same = manifest("0.1.0", 1);
        assert_eq!(Installation::new().menu_line(Some(&found(&same)), true), None);
        assert_eq!(Installation::new().menu_line(None, true), None);
        assert_eq!(Installation::new().chosen(None, &HERE), Reply { order: Order::Nothing, say: None });
    }

    fn refusals() -> Vec<Refusal> {
        let latest = Version::new(10, 20, 30);
        vec![
            Refusal::Unsupported,
            Refusal::NotInApp,
            Refusal::NotWritable,
            Refusal::UpToDate,
            Refusal::NotInstallable { latest, why: Obstacle::Bundle { published: 2, installed: 1 } },
            Refusal::NotInstallable { latest, why: Obstacle::NoTarget },
            Refusal::NotInstallable { latest, why: Obstacle::Format(2) },
            Refusal::Files { latest },
            Refusal::Info(InfoProblem::Unreadable),
            Refusal::Info(InfoProblem::NotOurs),
            Refusal::Info(InfoProblem::Version),
            Refusal::Info(InfoProblem::Executable),
            Refusal::Download(Failure::Unreachable),
            Refusal::Download(Failure::WrongSize),
            Refusal::Download(Failure::WrongDigest),
            Refusal::Download(Failure::Disk),
            Refusal::Installed,
            Refusal::Sign(SignProblem::Unknown),
            Refusal::Sign(SignProblem::Ambiguous(2)),
            Refusal::Sign(SignProblem::Keychain),
            Refusal::Sign(SignProblem::Failed),
            Refusal::Sign(SignProblem::Invalid),
            Refusal::Disk,
            Refusal::Cancelled,
            Refusal::Handover,
        ]
    }

    #[test]
    fn her_phrases_stay_short_and_never_send_to_a_terminal_but_once() {
        let manifest = manifest("10.20.30", 1);
        let verdict = verdict(&manifest, &HERE.installed);
        let mut said: Vec<String> = refusals().iter().map(Refusal::say).collect();
        let mut install = Installation::new();
        said.extend(install.chosen(Some((&verdict, &manifest)), &HERE).say);
        said.extend(install.progressed(Step::Assemble));
        said.extend(install.progressed(Step::Sign));
        said.extend(install.prepared(Signed::AdHoc));
        said.extend(install.menu_line(Some(&verdict), true));
        said.extend(install.chosen(Some((&verdict, &manifest)), &HERE).say);
        said.extend(install.menu_line(Some(&verdict), true));
        let mut to_the_script = 0;
        for text in &said {
            assert!(text.chars().count() <= 70, "le bandeau reste court : {text}");
            if text.contains("tools/install-macos.sh") {
                to_the_script += 1;
                continue;
            }
            for command in ["capri ", "tools/", ".sh", "--", "http", "github", "GitHub", "codesign", "sudo"] {
                assert!(!text.contains(command), "{text}");
            }
        }
        assert_eq!(to_the_script, 1, "seule une version qui change l'app renvoie au script");
        for refusal in refusals() {
            let line = refusal.for_log();
            assert!(!line.contains("://") && !line.contains('?'), "{line}");
        }
    }
}
