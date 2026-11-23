//! Ce qu'elle dit des versions, écrit une fois :
//! - dans sa **bulle** et son **menu** : jamais une commande de terminal, jamais une adresse ;
//! - dans un **terminal** (`capri update check`) : la version installée, la dernière publiée, le
//!   verdict, et ce qu'il faut faire ;
//! - dans son **journal** : la conclusion seulement (à jour, telle version disponible,
//!   vérification impossible et une cause courte) ; jamais une adresse, signée ou non, ni rien du
//!   manifeste au-delà de sa version.

use super::{Failure, Installed, Obstacle, Source, Verdict, Version};

/// Bulle : une version plus récente vient d'être trouvée.
pub fn available(latest: Version) -> String {
    format!("Une nouvelle version de moi est sortie : la {latest}")
}

/// Menu : la ligne qui le rappelle, tant qu'elle est la plus récente connue.
pub fn menu_line(latest: Version) -> String {
    format!("Version {latest} disponible")
}

/// Bulle : quand on choisit cette ligne du menu.
pub fn detail(verdict: &Verdict) -> String {
    match *verdict {
        Verdict::UpToDate { latest } => format!("Je suis à jour : la {latest} est la dernière"),
        Verdict::Installable { latest } => {
            format!("La {latest} est sortie : je peux m'installer depuis mon menu")
        }
        Verdict::NotInstallable { latest, why: Obstacle::Bundle { .. } } => {
            format!("La {latest} change ce que contient mon app : il faudra me réinstaller")
        }
        Verdict::NotInstallable { latest, why: Obstacle::NoTarget } => {
            format!("La {latest} est sortie, mais pas encore pour ce système")
        }
        Verdict::NotInstallable { latest, why: Obstacle::Format(_) } => {
            format!("La {latest} est sortie, dans un format que je ne lis pas encore")
        }
    }
}

/// Bulle : la case « Chercher les mises à jour » vient d'être cochée.
pub const CHECKING: &str = "Je regarderai chaque jour si une nouvelle version de moi existe";
/// … décochée.
pub const NOT_CHECKING: &str = "Je ne regarde plus si une nouvelle version de moi existe";
/// … cochée, mais avec un dossier d'essai ou pendant une répétition : rien ne part.
pub const NOT_IN_REHEARSAL: &str = "Pendant un essai, je ne regarde pas les versions";
/// … cochée, sur un système où elle ne sait pas parler `https`.
pub const NO_TLS: &str = "Je ne sais pas regarder les versions sur ce système";

/// Terminal et journal : pourquoi une version plus récente ne s'installera pas depuis l'app.
pub fn obstacle(why: &Obstacle) -> String {
    match *why {
        Obstacle::Format(format) => {
            format!("son manifeste est au format {format}, que cette version ne lit pas")
        }
        Obstacle::Bundle { published, installed } => format!(
            "elle change ce que contient Capri.app (contenu au format {published}, celui-ci au format {installed})"
        ),
        Obstacle::NoTarget => "elle n'est pas publiée pour ce système".to_owned(),
    }
}

/// Terminal : ce qui tourne ici.
pub fn installed(installed: &Installed) -> String {
    format!(
        "Version installée : {} (contenu de l'app au format {}, cible {})",
        installed.version,
        installed.bundle,
        installed.target.unwrap_or("aucune")
    )
}

/// Terminal : la dernière publiée, puis le verdict et ce qu'il faut faire.
pub fn terminal(verdict: &Verdict) -> String {
    let latest = format!("Dernière publiée  : {}", verdict.latest());
    let conclusion = match verdict {
        Verdict::UpToDate { .. } => "Capri est à jour.".to_owned(),
        Verdict::Installable { .. } => "Une version plus récente existe. Sur macOS, Capri l'installe \
                                        elle-même, sur un geste : « Installer la version … » dans son menu."
            .to_owned(),
        Verdict::NotInstallable { why, .. } => format!(
            "Une version plus récente existe, mais elle ne pourra pas s'installer depuis son menu : {}. \
             Pour l'avoir, réinstalle Capri depuis un dépôt à jour (sur macOS : tools/install-macos.sh).",
            obstacle(why)
        ),
    };
    format!("{latest}\n{conclusion}")
}

/// Journal : la conclusion.
pub fn for_log(verdict: &Verdict, installed: &Installed) -> String {
    match verdict {
        Verdict::UpToDate { latest } => {
            format!("à jour (installée {}, dernière publiée {latest})", installed.version)
        }
        Verdict::Installable { latest } => format!("version {latest} disponible"),
        Verdict::NotInstallable { latest, why } => {
            format!("version {latest} disponible, pas installable depuis l'app : {}", obstacle(why))
        }
    }
}

impl Failure {
    /// Journal : la cause, courte. Jamais une adresse (un refus de la règle ne cite que l'hôte).
    pub fn for_log(&self) -> String {
        match self {
            Self::NoTls => "pas de https sur ce système".to_owned(),
            Self::NotAllowed(why) => format!("adresse refusée : {why}"),
            Self::TooManyRedirects => "trop de redirections".to_owned(),
            Self::Unreachable => "injoignable".to_owned(),
            Self::Insecure => "connexion pas sûre".to_owned(),
            Self::TimedOut => "délai dépassé".to_owned(),
            Self::Status(status) => format!("réponse {status}"),
            Self::TooLarge => "réponse trop grosse".to_owned(),
            Self::Unreadable => "réponse illisible".to_owned(),
            Self::Manifest(why) => format!("manifeste refusé : {why}"),
            Self::WrongSize => "taille différente de celle du manifeste".to_owned(),
            Self::WrongDigest => "empreinte différente de celle du manifeste".to_owned(),
            Self::Disk => "écriture du fichier impossible".to_owned(),
            Self::Cancelled => "arrêté".to_owned(),
        }
    }

    /// Terminal : une phrase. `source` : où elle regardait.
    pub fn explain(&self, source: &Source) -> String {
        let who = source.name();
        match self {
            Self::NoTls => "Je ne sais pas me connecter en https sur ce système.".to_owned(),
            Self::NotAllowed(why) => {
                format!(
                    "La réponse de {who} renvoie vers une adresse que je ne suis pas ({why}) : je n'y suis pas allée."
                )
            }
            Self::TooManyRedirects => format!("Trop de redirections depuis {who}."),
            Self::Unreachable if *source == Source::GitHub => {
                "Je n'arrive pas à joindre GitHub. La connexion à Internet marche ?".to_owned()
            }
            Self::Unreachable => format!("Je n'arrive pas à joindre {who}."),
            Self::Insecure => format!("Je n'ai pas pu établir de connexion sûre avec {who}."),
            Self::TimedOut => format!("La réponse de {who} met trop de temps à venir."),
            Self::Status(404) => format!("Aucune version n'est publiée avec son manifeste chez {who} (404)."),
            Self::Status(status) => format!("La réponse de {who} est un refus ({status})."),
            Self::TooLarge => format!("La réponse de {who} est trop grosse pour être un manifeste."),
            Self::Unreadable => format!("La réponse de {who} est illisible."),
            Self::Manifest(why) => format!("Le manifeste de {who} est refusé : {why}."),
            Self::WrongSize => format!("Un fichier reçu de {who} n'a pas la taille que dit le manifeste."),
            Self::WrongDigest => {
                format!("Un fichier reçu de {who} n'a pas l'empreinte que dit le manifeste.")
            }
            Self::Disk => "Je n'ai pas pu écrire le fichier reçu.".to_owned(),
            Self::Cancelled => "Arrêté.".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ManifestError, Route, RouteError};
    use super::*;

    fn verdicts(latest: Version) -> [Verdict; 5] {
        [
            Verdict::UpToDate { latest },
            Verdict::Installable { latest },
            Verdict::NotInstallable { latest, why: Obstacle::Bundle { published: 2, installed: 1 } },
            Verdict::NotInstallable { latest, why: Obstacle::NoTarget },
            Verdict::NotInstallable { latest, why: Obstacle::Format(2) },
        ]
    }

    fn failures() -> Vec<Failure> {
        vec![
            Failure::NoTls,
            Failure::NotAllowed(RouteError::Host("evil.example".to_owned())),
            Failure::NotAllowed(RouteError::Scheme),
            Failure::TooManyRedirects,
            Failure::Unreachable,
            Failure::Insecure,
            Failure::TimedOut,
            Failure::Status(404),
            Failure::Status(503),
            Failure::TooLarge,
            Failure::Unreadable,
            Failure::Manifest(ManifestError::Invalid("sha256")),
            Failure::WrongSize,
            Failure::WrongDigest,
            Failure::Disk,
            Failure::Cancelled,
        ]
    }

    #[test]
    fn her_bubble_and_menu_never_send_to_a_terminal_and_stay_short() {
        let latest = Version::new(10, 20, 30);
        let mut said = vec![available(latest), menu_line(latest)];
        said.extend(verdicts(latest).iter().map(detail));
        said.extend([CHECKING, NOT_CHECKING, NOT_IN_REHEARSAL, NO_TLS].map(str::to_owned));
        for text in said {
            for command in ["capri ", "tools/", ".sh", "--", "http", "github", "GitHub", "brew"] {
                assert!(!text.contains(command), "{text}");
            }
            assert!(text.chars().count() <= 70, "le bandeau reste court : {text}");
        }
        assert_eq!(available(Version::new(0, 2, 0)), "Une nouvelle version de moi est sortie : la 0.2.0");
        assert_eq!(menu_line(Version::new(0, 2, 0)), "Version 0.2.0 disponible");
    }

    #[test]
    fn a_terminal_gets_the_versions_the_verdict_and_what_to_do() {
        let here = Installed { version: Version::new(0, 1, 0), bundle: 1, target: Some("macos-arm64") };
        assert_eq!(
            installed(&here),
            "Version installée : 0.1.0 (contenu de l'app au format 1, cible macos-arm64)"
        );
        assert_eq!(installed(&Installed { target: None, ..here }).rsplit(' ').next(), Some("aucune)"));
        let [up, installable, bundle, ..] = verdicts(Version::new(0, 2, 0));
        assert_eq!(terminal(&up), "Dernière publiée  : 0.2.0\nCapri est à jour.");
        assert!(terminal(&installable).ends_with("« Installer la version … » dans son menu."));
        assert!(terminal(&bundle).contains("tools/install-macos.sh"), "un terminal reçoit sa commande");
        assert!(terminal(&bundle).contains("(contenu au format 2, celui-ci au format 1)"));
    }

    #[test]
    fn the_journal_keeps_the_conclusion_and_never_an_address() {
        let here = Installed { version: Version::new(0, 1, 0), bundle: 1, target: Some("macos-arm64") };
        let latest = Version::new(0, 2, 0);
        let logged: Vec<String> = verdicts(latest).iter().map(|verdict| for_log(verdict, &here)).collect();
        assert_eq!(logged[0], "à jour (installée 0.1.0, dernière publiée 0.2.0)");
        assert_eq!(logged[1], "version 0.2.0 disponible");
        assert_eq!(
            logged[3],
            "version 0.2.0 disponible, pas installable depuis l'app : elle n'est pas publiée pour ce système"
        );
        let mut lines = logged;
        lines.extend(failures().iter().map(Failure::for_log));
        for line in lines {
            assert!(!line.contains("://") && !line.contains('?') && !line.contains('/'), "{line}");
        }
        assert_eq!(Failure::TimedOut.for_log(), "délai dépassé");
        assert_eq!(
            Failure::NotAllowed(RouteError::Host("evil.example".to_owned())).for_log(),
            "adresse refusée : hôte hors de la liste (evil.example)"
        );
    }

    #[test]
    fn each_failure_is_one_plain_sentence() {
        let rehearsal = Source::ThisMachine {
            url: "http://127.0.0.1:1/x".to_owned(),
            route: Route::this_machine(1).expect("la règle"),
        };
        for source in [Source::GitHub, rehearsal] {
            for failure in failures() {
                let said = failure.explain(&source);
                assert!(said.ends_with(['.', '?']) && !said.contains('\n'), "{said}");
                assert!(!said.contains("://"), "{said}");
            }
        }
        assert_eq!(
            Failure::Unreachable.explain(&Source::GitHub),
            "Je n'arrive pas à joindre GitHub. La connexion à Internet marche ?"
        );
        assert_eq!(
            Failure::Status(404).explain(&Source::GitHub),
            "Aucune version n'est publiée avec son manifeste chez GitHub (404)."
        );
    }
}
