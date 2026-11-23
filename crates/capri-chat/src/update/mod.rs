//! Savoir qu'une version plus récente de Capri existe : ce qu'il faut décider, sans rien toucher.
//!
//! Comme Droppy, Capri regarde d'elle-même, une fois par jour, si une version plus récente a été
//! publiée, et le dit (la case « Chercher les mises à jour » de son menu, cochée par défaut).
//! D'elle-même, elle ne fait que cela : télécharger et installer une version est un geste de
//! l'utilisateur (la ligne « Installer la version … » de son menu, sur macOS).
//!
//! Ce qui part d'elle-même : une lecture du seul fichier [`MANIFEST_NAME`] de la dernière version
//! publiée du dépôt ([`manifest_url`]), un `GET` sans corps, sans rien sur l'utilisateur ni sur la
//! machine (GitHub voit l'adresse IP, comme pour toute connexion). Le réseau est le travail de
//! `capri-net` ; ici, on décide :
//! - [`Version`] : la version et sa comparaison ;
//! - [`manifest`] : la lecture stricte du manifeste ;
//! - [`verdict`] : à jour, une version installable dans l'app, ou une version qui ne l'est pas, et
//!   pourquoi ([`Obstacle`]) ;
//! - [`route`] : la règle des redirections que `capri-net` suit, prise en paramètre ;
//! - [`schedule`] : quand regarder ;
//! - [`source`] : où regarder, et jamais GitHub depuis un essai ou une répétition ;
//! - [`say`] : ce qu'elle en dit, dans sa bulle, son menu, un terminal et son journal ;
//! - [`digest`] : l'empreinte SHA-256 d'un fichier téléchargé, calculée au fil de sa lecture ;
//! - [`install`] : sur un geste, ce qu'il faut télécharger, ce qu'il faut vérifier, le déroulé de
//!   l'installation et ses refus.

pub mod digest;
pub mod install;
pub mod manifest;
pub mod route;
pub mod say;
pub mod schedule;
mod version;

pub use manifest::{Manifest, ManifestError};
pub use route::{Address, Route, RouteError};
pub use schedule::Schedule;
pub use version::Version;

/// Le dépôt public de Capri : son adresse, écrite une seule fois. Celle du manifeste et celle des
/// fichiers d'une version en sont calculées.
pub const REPOSITORY: &str = "https://github.com/Thai-v4/capri-daemon";
/// Le manifeste d'une version, parmi ses fichiers.
pub const MANIFEST_NAME: &str = "latest.json";

/// Après l'adresse du dépôt : le chemin du manifeste de la dernière version publiée.
const LATEST_MANIFEST: &str = "/releases/latest/download/latest.json";

/// Le manifeste de la dernière version publiée : GitHub y renvoie, par deux redirections, vers le
/// fichier de la version la plus récente.
pub fn manifest_url() -> String {
    format!("{REPOSITORY}{LATEST_MANIFEST}")
}

/// L'adresse d'un fichier d'une version : **calculée** à partir de la version et du nom du
/// fichier, jamais lue dans le manifeste.
pub fn asset_url(version: Version, asset: &manifest::Name) -> String {
    asset_under(REPOSITORY, version, asset)
}

/// L'adresse d'un fichier d'une version, sous l'adresse `base` d'un dépôt.
fn asset_under(base: &str, version: Version, asset: &manifest::Name) -> String {
    format!("{base}/releases/download/v{version}/{asset}")
}

/// La cible de ce programme dans le manifeste, s'il y en a une : Capri n'est publiée que pour
/// macOS sur Apple Silicon et pour Windows x86-64.
pub const TARGET: Option<&str> = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    Some("macos-arm64")
} else if cfg!(all(windows, target_arch = "x86_64")) {
    Some("windows-x86_64")
} else {
    None
};

/// Le format de ce que `Capri.app` contient en plus de ses programmes, tel qu'écrit dans
/// `tools/macos/bundle-format` : un entier, au moins 1, suivi ou non d'un saut de ligne. Celui de
/// Windows (`\r\n`) aussi : la CI y extrait le dépôt avec ses fins de ligne, et ce fichier est lu
/// par `include_str!` dans l'app compilée là.
pub fn bundle_format(text: &str) -> Option<u32> {
    let text = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text);
    let digits = !text.is_empty() && text.len() <= 9 && text.bytes().all(|b| b.is_ascii_digit());
    digits.then(|| text.parse().ok()).flatten().filter(|&format| format >= 1)
}

/// Ce qui tourne ici.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Installed {
    pub version: Version,
    /// Le format de ce que son app contient en plus de ses programmes.
    pub bundle: u32,
    /// Sa cible dans le manifeste ([`TARGET`]).
    pub target: Option<&'static str>,
}

/// Pourquoi une version plus récente ne pourra pas s'installer depuis l'app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Obstacle {
    /// Le manifeste est d'un format que ce lecteur ne connaît pas.
    Format(u64),
    /// Elle change ce que l'app contient en plus de ses programmes (ses images) : une mise à jour
    /// qui ne remplace que les programmes ne suffirait pas.
    Bundle { published: u32, installed: u32 },
    /// Elle n'est pas publiée pour ce système (ou ce système n'a pas de cible).
    NoTarget,
}

/// Ce qu'elle conclut du manifeste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// La dernière publiée n'est pas plus récente que la sienne.
    UpToDate { latest: Version },
    /// Une version plus récente existe, et pourra s'installer dans l'app.
    Installable { latest: Version },
    /// Une version plus récente existe, mais pas installable dans l'app.
    NotInstallable { latest: Version, why: Obstacle },
}

impl Verdict {
    /// La dernière version publiée.
    pub fn latest(&self) -> Version {
        match *self {
            Self::UpToDate { latest }
            | Self::Installable { latest }
            | Self::NotInstallable { latest, .. } => latest,
        }
    }

    /// Une version plus récente que la sienne, s'il y en a une.
    pub fn newer(&self) -> Option<Version> {
        match *self {
            Self::UpToDate { .. } => None,
            Self::Installable { latest } | Self::NotInstallable { latest, .. } => Some(latest),
        }
    }
}

/// Le verdict, pour ce qui tourne ici. Dans l'ordre : la version (pas plus récente : à jour, quoi
/// qu'il en soit du reste), le format du manifeste, celui du contenu de l'app, la cible.
pub fn verdict(manifest: &Manifest, installed: &Installed) -> Verdict {
    let latest = manifest.version();
    if latest <= installed.version {
        return Verdict::UpToDate { latest };
    }
    let release = match manifest {
        Manifest::Unknown { format, .. } => {
            return Verdict::NotInstallable { latest, why: Obstacle::Format(*format) };
        }
        Manifest::Known(release) => release,
    };
    if release.bundle != installed.bundle {
        let why = Obstacle::Bundle { published: release.bundle, installed: installed.bundle };
        return Verdict::NotInstallable { latest, why };
    }
    match installed.target.and_then(|target| release.target(target)) {
        Some(_) => Verdict::Installable { latest },
        None => Verdict::NotInstallable { latest, why: Obstacle::NoTarget },
    }
}

/// Le verdict d'un manifeste reçu (ses octets, tels que lus) : lu strictement, puis jugé.
pub fn conclude(body: &[u8], installed: &Installed) -> Result<Verdict, Failure> {
    let manifest = Manifest::parse(body).map_err(Failure::Manifest)?;
    Ok(verdict(&manifest, installed))
}

/// Pourquoi une vérification n'a pas abouti. `capri-net` rend les causes du réseau ; celle du
/// manifeste vient d'ici.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Pas de `https` sur ce système (Linux) : rien n'est parti.
    NoTls,
    /// L'adresse de départ, ou une redirection, n'est pas permise par la règle : rien n'y est parti.
    NotAllowed(RouteError),
    /// Plus de redirections que la règle n'en permet.
    TooManyRedirects,
    /// Personne ne répond (pas de réseau, hôte introuvable, connexion refusée).
    Unreachable,
    /// La connexion chiffrée n'a pas pu s'établir (certificat refusé…).
    Insecure,
    /// Trop long.
    TimedOut,
    /// Le serveur répond autre chose qu'un succès ou une redirection.
    Status(u16),
    /// La réponse dépasse la taille d'un manifeste.
    TooLarge,
    /// Ce qui revient n'est pas une réponse lisible (une redirection sans adresse…).
    Unreadable,
    /// Le manifeste est refusé.
    Manifest(ManifestError),
    /// Un fichier téléchargé n'a pas la taille que dit le manifeste (plus, ou moins).
    WrongSize,
    /// Un fichier téléchargé n'a pas l'empreinte que dit le manifeste.
    WrongDigest,
    /// Écrire le fichier téléchargé a échoué.
    Disk,
    /// Le téléchargement a été arrêté (case décochée, ou Capri quittée).
    Cancelled,
}

/// Où regarder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// Le vrai : le manifeste du dépôt sur GitHub, par la règle de GitHub.
    GitHub,
    /// Un faux serveur de cette machine (une répétition, en version de débogage) : son adresse, et
    /// la règle qui ne permet que lui.
    ThisMachine { url: String, route: Route },
}

impl Source {
    /// L'adresse de départ. Jamais signée : elle peut s'écrire.
    pub fn url(&self) -> String {
        match self {
            Self::GitHub => manifest_url(),
            Self::ThisMachine { url, .. } => url.clone(),
        }
    }

    /// La règle des redirections.
    pub fn route(&self) -> Route {
        match self {
            Self::GitHub => Route::github(),
            Self::ThisMachine { route, .. } => route.clone(),
        }
    }

    /// De qui parler dans un message.
    pub fn name(&self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::ThisMachine { .. } => "le serveur d'essai",
        }
    }

    /// L'adresse d'un fichier de la version `version`, calculée, jamais lue dans le manifeste :
    /// chez GitHub, [`asset_url`] ; chez le serveur d'essai, la même forme sous son adresse, si
    /// celle du manifeste a la forme de celle de GitHub (`…/releases/latest/download/latest.json`),
    /// sinon aucune (il ne sert pas de fichiers).
    pub fn asset_url(&self, version: Version, asset: &manifest::Name) -> Option<String> {
        match self {
            Self::GitHub => Some(asset_url(version, asset)),
            Self::ThisMachine { url, .. } => {
                url.strip_suffix(LATEST_MANIFEST).map(|base| asset_under(base, version, asset))
            }
        }
    }
}

/// Pourquoi elle ne regarde nulle part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offline {
    /// Un dossier d'essai (`CAPRI_CONFIG_DIR`) ou une répétition : rien ne part vers GitHub.
    Rehearsal,
    /// L'adresse d'essai n'est pas un serveur de cette machine (`http://127.0.0.1:<port>/…`) :
    /// refusée, et pas remplacée par GitHub.
    NotThisMachine(RouteError),
}

/// Où regarder. `rehearsal` : un dossier d'essai ou une répétition est en cours ; `test_url` :
/// l'adresse d'un faux serveur de cette machine (version de débogage seulement : l'appelant ne la
/// lit pas ailleurs). Une adresse d'essai passe avant tout, et seule une adresse de cette machine
/// est permise ; sans elle, un essai ne regarde nulle part ; sinon, GitHub.
pub fn source(rehearsal: bool, test_url: Option<&str>) -> Result<Source, Offline> {
    match test_url {
        Some(url) => this_machine(url).map_err(Offline::NotThisMachine),
        None if rehearsal => Err(Offline::Rehearsal),
        None => Ok(Source::GitHub),
    }
}

/// Un faux serveur de cette machine : `http://127.0.0.1:<port>/…`, et la règle qui ne permet que
/// lui.
fn this_machine(url: &str) -> Result<Source, RouteError> {
    let rest = url.strip_prefix("http://127.0.0.1:").ok_or(RouteError::Malformed)?;
    let port = rest.split(['/', '?', '#']).next().and_then(|port| port.parse::<u16>().ok());
    let route = port.and_then(|port| Route::this_machine(port).ok()).ok_or(RouteError::Port)?;
    route.check(url)?;
    Ok(Source::ThisMachine { url: url.to_owned(), route })
}

#[cfg(test)]
mod tests {
    use super::manifest::tests::example;
    use super::*;

    const HERE: Installed =
        Installed { version: Version::new(0, 2, 0), bundle: 1, target: Some("macos-arm64") };

    fn judged(text: &str, installed: &Installed) -> Verdict {
        conclude(text.as_bytes(), installed).expect("lisible")
    }

    #[test]
    fn the_addresses_are_computed_from_the_repository_never_read_from_the_manifest() {
        assert_eq!(
            manifest_url(),
            "https://github.com/Thai-v4/capri-daemon/releases/latest/download/latest.json"
        );
        let asset = manifest::Name::parse("capri-macos-arm64").expect("un nom");
        assert_eq!(
            asset_url(Version::new(0, 2, 0), &asset),
            "https://github.com/Thai-v4/capri-daemon/releases/download/v0.2.0/capri-macos-arm64"
        );
        // l'une comme l'autre passe la règle de GitHub
        let route = Route::github();
        assert!(route.check(&manifest_url()).is_ok());
        assert!(route.check(&asset_url(Version::new(0, 2, 0), &asset)).is_ok());
    }

    #[test]
    fn only_a_newer_version_counts_and_it_says_whether_the_app_can_take_it() {
        let newer = example("0.3.0", 1);
        assert_eq!(judged(&newer, &HERE), Verdict::Installable { latest: Version::new(0, 3, 0) });
        // la même, ou une plus ancienne : à jour, quoi qu'il en soit du reste
        for latest in ["0.2.0", "0.1.9"] {
            let text = example(latest, 7);
            assert_eq!(
                judged(&text, &HERE),
                Verdict::UpToDate { latest: Version::parse(latest).expect("v") }
            );
        }
        // un contenu d'app d'un autre format : les programmes seuls ne suffiraient pas
        assert_eq!(
            judged(&example("0.3.0", 2), &HERE),
            Verdict::NotInstallable {
                latest: Version::new(0, 3, 0),
                why: Obstacle::Bundle { published: 2, installed: 1 }
            }
        );
        // pas pour ce système
        for target in [Some("linux-x86_64"), None] {
            let installed = Installed { target, ..HERE };
            assert_eq!(
                judged(&newer, &installed),
                Verdict::NotInstallable { latest: Version::new(0, 3, 0), why: Obstacle::NoTarget }
            );
        }
        let windows = Installed { target: Some("windows-x86_64"), ..HERE };
        assert!(matches!(judged(&newer, &windows), Verdict::Installable { .. }));
        // un manifeste d'un format inconnu : on sait seulement qu'elle existe
        let unknown = r#"{"format": 2, "version": "1.0.0"}"#;
        assert_eq!(
            judged(unknown, &HERE),
            Verdict::NotInstallable { latest: Version::new(1, 0, 0), why: Obstacle::Format(2) }
        );
        let older_unknown = r#"{"format": 9, "version": "0.1.0"}"#;
        assert_eq!(judged(older_unknown, &HERE), Verdict::UpToDate { latest: Version::new(0, 1, 0) });
        // illisible : un échec, jamais un verdict deviné
        assert_eq!(conclude(b"{}", &HERE), Err(Failure::Manifest(ManifestError::Missing("format"))));
        assert_eq!(judged(&newer, &HERE).newer(), Some(Version::new(0, 3, 0)));
        assert_eq!(judged(&example("0.2.0", 1), &HERE).newer(), None);
        assert_eq!(judged(&example("0.2.0", 1), &HERE).latest(), Version::new(0, 2, 0));
    }

    #[test]
    fn the_bundle_format_is_a_plain_number() {
        assert_eq!(bundle_format("1\n"), Some(1));
        assert_eq!(bundle_format("1\r\n"), Some(1), "extrait sous Windows");
        assert_eq!(bundle_format("12"), Some(12));
        for refused in
            ["", "\n", "0\n", "1\n\n", " 1\n", "1 \n", "x\n", "-1\n", "1\r", "1\n\r", "9999999999\n"]
        {
            assert_eq!(bundle_format(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn a_rehearsal_never_looks_at_github() {
        assert_eq!(source(false, None), Ok(Source::GitHub));
        assert_eq!(source(true, None), Err(Offline::Rehearsal), "un dossier d'essai : nulle part");
        // une adresse d'essai : ce faux serveur seulement, avec ou sans dossier d'essai
        let url = "http://127.0.0.1:8123/releases/latest/download/latest.json";
        for rehearsal in [false, true] {
            let Ok(found) = source(rehearsal, Some(url)) else { panic!("refusée") };
            assert_eq!(found.url(), url);
            assert_eq!(found.route(), Route::this_machine(8123).expect("la règle"));
            assert_eq!(found.name(), "le serveur d'essai");
        }
        // une adresse d'essai qui n'est pas cette machine : refusée, jamais remplacée par GitHub
        for url in [
            "https://github.com/Thai-v4/capri-daemon/releases/latest/download/latest.json",
            "http://localhost:8123/x",
            "http://192.168.1.2:8123/x",
            "http://127.0.0.1/x",
            "http://127.0.0.1:0/x",
            "http://127.0.0.1:8123@evil.example/x",
            "",
        ] {
            for rehearsal in [false, true] {
                assert!(matches!(source(rehearsal, Some(url)), Err(Offline::NotThisMachine(_))), "{url}");
            }
        }
        assert_eq!(Source::GitHub.url(), manifest_url());
        assert_eq!(Source::GitHub.route(), Route::github());
    }

    #[test]
    fn a_file_s_address_is_computed_from_the_source_never_read() {
        let asset = manifest::Name::parse("capri-macos-arm64").expect("un nom");
        let version = Version::new(0, 2, 0);
        assert_eq!(Source::GitHub.asset_url(version, &asset), Some(asset_url(version, &asset)));
        assert!(manifest_url().ends_with(MANIFEST_NAME));
        // le serveur d'essai, rangé comme GitHub : ses fichiers sous la même forme
        let Ok(local) = source(
            true,
            Some("http://127.0.0.1:8123/Thai-v4/capri-daemon/releases/latest/download/latest.json"),
        ) else {
            panic!("refusée")
        };
        let url = local.asset_url(version, &asset).expect("une adresse");
        assert_eq!(
            url,
            "http://127.0.0.1:8123/Thai-v4/capri-daemon/releases/download/v0.2.0/capri-macos-arm64"
        );
        assert!(local.route().check(&url).is_ok(), "la même règle que le manifeste");
        // rangé autrement : il ne sert pas de fichiers
        let Ok(elsewhere) = source(true, Some("http://127.0.0.1:8123/x")) else { panic!("refusée") };
        assert_eq!(elsewhere.asset_url(version, &asset), None);
    }

    #[test]
    fn this_program_knows_its_target_where_capri_is_published() {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(TARGET, Some("macos-arm64"));
        } else if cfg!(all(windows, target_arch = "x86_64")) {
            assert_eq!(TARGET, Some("windows-x86_64"));
        } else {
            assert_eq!(TARGET, None);
        }
    }
}
