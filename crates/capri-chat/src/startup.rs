//! Se lancer à l'ouverture de session, avec Ollama si c'est son cerveau : ce qu'il faut décider,
//! sans rien toucher.
//!
//! Une seule option, la case « Se lancer à l'ouverture de session » de son menu. Cochée, Capri est
//! inscrite (`capri_core::login`), et **à chaque lancement** (à l'ouverture de session ou à la
//! main) elle regarde si son cerveau est Ollama **sur cette machine même** ; si rien ne répond à
//! son adresse, elle lance Ollama. Décochée, elle ne lance jamais rien. Elle ne lance qu'Ollama,
//! cherché à des emplacements fixes ([`candidates`], jamais par le `PATH`, presque vide quand le
//! système lance une app), jamais vers une autre machine, et ne l'arrête jamais : lancé par elle,
//! il continue de tourner quand on la quitte. LM Studio n'est pas pris en charge.
//!
//! Ce que dit la documentation d'Ollama (relue le 12 novembre 2026) :
//! - « Ollama binds 127.0.0.1 port 11434 by default. Change the bind address with the
//!   `OLLAMA_HOST` environment variable » (<https://docs.ollama.com/faq>) ; `ollama serve --help`
//!   (version 0.40.14) : « OLLAMA_HOST  IP Address for the ollama server (default
//!   127.0.0.1:11434) » ;
//! - l'app : « drag-and-drop the Ollama application to the system-wide Applications folder », et
//!   son programme en ligne de commande est `Ollama.app/Contents/Resources/ollama`, que l'app
//!   propose de lier dans `/usr/local/bin` (<https://docs.ollama.com/macos>) ;
//! - lancée comme app, elle prend ses variables par `launchctl setenv`, pas par l'environnement
//!   de qui la lance (<https://docs.ollama.com/faq>) : l'app n'est donc choisie que pour l'adresse
//!   par défaut ; ailleurs, c'est son programme qui est lancé, avec `OLLAMA_HOST`.
//!
//! Homebrew range le programme dans `/opt/homebrew/bin` (Apple Silicon) ou `/usr/local/bin`
//! (Intel) : vérifié sur ce Mac pour le premier.
//!
//! Savoir si quelque chose répond est le travail de `capri-net`, et lancer un programme celui de
//! `capri-platform` ; ici, on décide.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::brain::{Brain, Provider};
use crate::endpoint::{Endpoint, Reach};

/// Le port d'Ollama par défaut.
pub const DEFAULT_PORT: u16 = 11434;

/// Pourquoi elle ne lancera pas Ollama.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    /// La case n'est pas cochée : elle ne lance jamais rien.
    Unchecked,
    /// Elle n'a pas de cerveau, ou il est illisible.
    NoBrain,
    /// Son cerveau n'est pas Ollama (LM Studio non plus n'est pas lancé).
    NotOllama,
    /// Ollama tourne sur une autre machine : ce n'est pas à elle de le lancer.
    NotThisMachine,
    /// L'adresse est en `https://` : ce n'est pas ce que sert `ollama serve`.
    Encrypted,
}

/// Ce qu'il faut faire au lancement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Rien.
    Leave(Leave),
    /// Regarder si quelque chose répond à cette adresse de cette machine, et sinon lancer Ollama.
    Watch(Endpoint),
}

/// Faut-il veiller à ce qu'Ollama tourne ? `registered` : la case est cochée (l'inscription est
/// là, et c'est la nôtre) ; `brain` : son cerveau, s'il en a un de lisible.
pub fn plan(registered: bool, brain: Option<&Brain>) -> Plan {
    let Some(brain) = brain.filter(|_| registered) else {
        return Plan::Leave(if registered { Leave::NoBrain } else { Leave::Unchecked });
    };
    let endpoint = brain.endpoint();
    if brain.provider() != Provider::Ollama {
        Plan::Leave(Leave::NotOllama)
    } else if endpoint.reach() != Reach::ThisMachine {
        Plan::Leave(Leave::NotThisMachine)
    } else if endpoint.secure {
        Plan::Leave(Leave::Encrypted)
    } else {
        Plan::Watch(endpoint.clone())
    }
}

/// L'adresse où Ollama doit écouter, pour `OLLAMA_HOST` : l'hôte et le port de l'adresse réglée.
/// `localhost` devient `127.0.0.1`, son adresse par défaut, pour qu'Ollama n'écoute que sur la
/// boucle locale quoi qu'en dise la résolution des noms. Une adresse IPv6 est entre crochets.
pub fn ollama_host(endpoint: &Endpoint) -> String {
    match endpoint.host.as_str() {
        "localhost" => format!("127.0.0.1:{}", endpoint.port),
        host if host.contains(':') => format!("[{host}]:{}", endpoint.port),
        host => format!("{host}:{}", endpoint.port),
    }
}

/// Est-ce l'adresse où Ollama écoute de lui-même ? Seule celle-ci peut être confiée à l'app.
pub fn is_default_address(endpoint: &Endpoint) -> bool {
    endpoint.port == DEFAULT_PORT && matches!(endpoint.host.as_str(), "localhost" | "127.0.0.1")
}

/// Un Ollama qu'on peut lancer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ollama {
    /// `Ollama.app` : lancée comme un double-clic, elle sert l'adresse par défaut.
    App(PathBuf),
    /// Le programme `ollama`, lancé avec `serve` et `OLLAMA_HOST`.
    Program(PathBuf),
}

impl Ollama {
    pub fn path(&self) -> &Path {
        match self {
            Self::App(path) | Self::Program(path) => path,
        }
    }

    /// Ce qu'on passe au programme : `serve`, rien pour l'app.
    pub fn args(&self) -> &'static [&'static str] {
        match self {
            Self::App(_) => &[],
            Self::Program(_) => &["serve"],
        }
    }

    /// L'environnement à lui donner : `OLLAMA_HOST` pour le programme, rien pour l'app (qui ne le
    /// lirait pas).
    pub fn env(&self, endpoint: &Endpoint) -> Vec<(&'static str, String)> {
        match self {
            Self::App(_) => Vec::new(),
            Self::Program(_) => vec![("OLLAMA_HOST", ollama_host(endpoint))],
        }
    }
}

/// Où chercher Ollama, dans l'ordre : l'app d'abord (seulement pour l'adresse par défaut), puis le
/// programme de Homebrew, celui que l'app lie dans `/usr/local/bin`, et celui qu'elle contient.
/// Des emplacements fixes : jamais le `PATH`. `home` : le dossier de l'utilisateur, s'il est
/// connu (pour `~/Applications`).
pub fn candidates(home: Option<&Path>, endpoint: &Endpoint) -> Vec<Ollama> {
    let mut apps = vec![PathBuf::from("/Applications/Ollama.app")];
    apps.extend(home.map(|home| home.join("Applications/Ollama.app")));
    let mut found = Vec::new();
    if is_default_address(endpoint) {
        found.extend(apps.iter().cloned().map(Ollama::App));
    }
    found.extend(
        [PathBuf::from("/opt/homebrew/bin/ollama"), PathBuf::from("/usr/local/bin/ollama")]
            .into_iter()
            .chain(apps.iter().map(|app| app.join("Contents/Resources/ollama")))
            .map(Ollama::Program),
    );
    found
}

/// Avant de le lancer, combien de temps laisser à un Ollama qui est peut-être en train de
/// démarrer (à l'ouverture de session, `brew services` le lance en même temps qu'elle).
pub const GRACE: Duration = Duration::from_secs(6);
/// Le temps entre deux coups d'œil à son adresse.
pub const LOOK_EVERY: Duration = Duration::from_secs(1);
/// Combien de temps attendre qu'un Ollama qu'elle vient de lancer réponde, avant de dire qu'il
/// ne répond pas encore.
pub const STARTUP_WAIT: Duration = Duration::from_secs(30);

/// Le pas suivant, après un coup d'œil à son adresse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Regarder de nouveau dans ce délai.
    Wait(Duration),
    /// Rien ne répond, même après [`GRACE`] : le lancer.
    Launch,
    /// Fini.
    Done(Ending),
}

/// Comment ça s'est fini.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// Quelque chose répondait déjà (Ollama, ou un autre serveur sur ce port) : rien n'est lancé.
    AlreadyThere,
    /// Elle l'a lancé, et il répond.
    Launched,
    /// Elle l'a lancé, mais il ne répond pas au bout de [`STARTUP_WAIT`].
    LaunchedSilent,
}

/// Le déroulé : regarder, attendre un peu, lancer une seule fois, attendre qu'il réponde. Le temps
/// est passé en paramètre (depuis le premier coup d'œil).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wake {
    launched_at: Option<Duration>,
}

impl Wake {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un coup d'œil vient d'être donné, `elapsed` après le premier : quelque chose a-t-il
    /// répondu ?
    pub fn looked(&mut self, answered: bool, elapsed: Duration) -> Step {
        match self.launched_at {
            None if answered => Step::Done(Ending::AlreadyThere),
            None if elapsed < GRACE => Step::Wait(LOOK_EVERY),
            None => Step::Launch,
            Some(_) if answered => Step::Done(Ending::Launched),
            Some(at) if elapsed.saturating_sub(at) < STARTUP_WAIT => Step::Wait(LOOK_EVERY),
            Some(_) => Step::Done(Ending::LaunchedSilent),
        }
    }

    /// Il vient d'être lancé, `elapsed` après le premier coup d'œil. Une seule fois : ensuite, on
    /// ne fait plus qu'attendre qu'il réponde.
    pub fn launched(&mut self, elapsed: Duration) {
        self.launched_at.get_or_insert(elapsed);
    }
}

/// Ce qu'elle dit dans son bandeau. Jamais une commande de terminal : la bulle et le bandeau
/// renvoient à son menu, ou disent quoi faire.
pub mod say {
    /// Elle vient de lancer Ollama, et il répond.
    pub const LAUNCHED: &str = "J'ai lancé Ollama, mon cerveau est prêt";
    /// Elle l'a lancé, mais il ne répond pas encore.
    pub const LAUNCHED_SILENT: &str = "J'ai lancé Ollama, mais il ne répond pas encore";
    /// Rien ne répond, et Ollama n'est à aucun des emplacements connus.
    pub const NOT_FOUND: &str = "Ollama ne répond pas, et je ne le trouve pas sur ce Mac";
    /// Le lancement a échoué.
    pub const LAUNCH_FAILED: &str = "Je n'ai pas pu lancer Ollama, détails dans le journal";

    /// La case vient d'être cochée. `local_ollama` : son cerveau est Ollama sur cette machine.
    pub fn registered(local_ollama: bool) -> &'static str {
        if local_ollama {
            "Je me lancerai à l'ouverture de ta session, et Ollama avec moi"
        } else {
            "Je me lancerai à l'ouverture de ta session"
        }
    }

    /// La case vient d'être décochée.
    pub const UNREGISTERED: &str = "Je ne me lancerai plus à l'ouverture de ta session";
    /// Lancée hors de `Capri.app` (le binaire de `cargo run`) : elle n'inscrit pas un binaire nu.
    pub const ONLY_FROM_APP: &str = "Pour m'inscrire, lance-moi depuis Capri.app";
    /// Un fichier qui n'est pas le nôtre porte le nom de son inscription.
    pub const NOT_OURS: &str = "Un fichier qui n'est pas à moi porte déjà ce nom, je n'y touche pas";
    /// Écrire ou retirer l'inscription a échoué.
    pub const FAILED: &str = "Je n'ai pas pu changer mon inscription, détails dans le journal";
    /// Ailleurs que sur macOS.
    pub const UNAVAILABLE: &str = "Je ne sais pas encore me lancer toute seule sur ce système";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brain(provider: Provider, url: &str) -> Brain {
        Brain::new(provider, Some(url), "m").expect("valide")
    }

    #[test]
    fn only_a_checked_box_and_an_ollama_on_this_machine_get_watched() {
        let here = brain(Provider::Ollama, "http://localhost:11434/v1");
        assert_eq!(plan(true, Some(&here)), Plan::Watch(here.endpoint().clone()));
        for url in ["http://127.0.0.1:11500/v1", "http://[::1]:11434/v1", "http://127.0.0.2:11434"] {
            assert!(matches!(plan(true, Some(&brain(Provider::Ollama, url))), Plan::Watch(_)), "{url}");
        }
        // décochée : jamais rien, quel que soit le cerveau
        assert_eq!(plan(false, Some(&here)), Plan::Leave(Leave::Unchecked));
        assert_eq!(plan(false, None), Plan::Leave(Leave::Unchecked));
        assert_eq!(plan(true, None), Plan::Leave(Leave::NoBrain));
        // ni LM Studio, ni un fournisseur, ni un Ollama d'une autre machine
        let refused = [
            (brain(Provider::LmStudio, "http://localhost:1234/v1"), Leave::NotOllama),
            (brain(Provider::Custom, "http://localhost:11434/v1"), Leave::NotOllama),
            (Brain::new(Provider::OllamaCloud, None, "m").expect("valide"), Leave::NotOllama),
            (brain(Provider::Ollama, "http://192.168.1.20:11434/v1"), Leave::NotThisMachine),
            (brain(Provider::Ollama, "http://100.64.0.1:11434/v1"), Leave::NotThisMachine),
            (brain(Provider::Ollama, "https://ollama.example.com/v1"), Leave::NotThisMachine),
            (brain(Provider::Ollama, "https://localhost:11434/v1"), Leave::Encrypted),
        ];
        for (brain, why) in refused {
            assert_eq!(plan(true, Some(&brain)), Plan::Leave(why), "{brain:?}");
        }
    }

    #[test]
    fn ollama_listens_where_capri_will_knock_and_only_on_this_machine() {
        let at = |url: &str| Endpoint::parse(url).expect("valide");
        assert_eq!(ollama_host(&at("http://localhost:11434/v1")), "127.0.0.1:11434");
        assert_eq!(ollama_host(&at("http://127.0.0.1:11500")), "127.0.0.1:11500");
        assert_eq!(ollama_host(&at("http://127.0.0.2:8080/v1")), "127.0.0.2:8080");
        assert_eq!(ollama_host(&at("http://[::1]:11434/v1")), "[::1]:11434");
        assert!(is_default_address(&at("http://localhost:11434/v1")));
        assert!(is_default_address(&at("http://127.0.0.1:11434")));
        assert!(!is_default_address(&at("http://localhost:11500/v1")));
        assert!(!is_default_address(&at("http://[::1]:11434/v1")), "l'app n'écoute pas là d'elle-même");
    }

    #[test]
    fn ollama_is_looked_for_in_fixed_places_and_the_app_only_serves_its_own_address() {
        let home = Path::new("/Users/moi");
        let default = Endpoint::parse("http://localhost:11434/v1").expect("valide");
        let paths = |found: &[Ollama]| {
            found.iter().map(|o| o.path().to_string_lossy().into_owned()).collect::<Vec<_>>()
        };
        let found = candidates(Some(home), &default);
        assert_eq!(
            paths(&found),
            [
                "/Applications/Ollama.app",
                "/Users/moi/Applications/Ollama.app",
                "/opt/homebrew/bin/ollama",
                "/usr/local/bin/ollama",
                "/Applications/Ollama.app/Contents/Resources/ollama",
                "/Users/moi/Applications/Ollama.app/Contents/Resources/ollama",
            ]
        );
        assert!(matches!(found[0], Ollama::App(_)) && matches!(found[2], Ollama::Program(_)));
        // tous absolus : rien n'est cherché dans le PATH
        assert!(found.iter().all(|ollama| ollama.path().is_absolute()));

        let elsewhere = Endpoint::parse("http://127.0.0.1:11500/v1").expect("valide");
        let found = candidates(None, &elsewhere);
        assert!(found.iter().all(|ollama| matches!(ollama, Ollama::Program(_))), "{found:?}");
        assert_eq!(found.len(), 3, "sans dossier de l'utilisateur, ni ~/Applications");

        let program = Ollama::Program(PathBuf::from("/opt/homebrew/bin/ollama"));
        assert_eq!(program.args(), ["serve"]);
        assert_eq!(program.env(&elsewhere), [("OLLAMA_HOST", "127.0.0.1:11500".to_owned())]);
        let app = Ollama::App(PathBuf::from("/Applications/Ollama.app"));
        assert!(app.args().is_empty() && app.env(&default).is_empty());
    }

    #[test]
    fn an_ollama_already_starting_is_given_time_and_ollama_is_launched_once() {
        let s = Duration::from_secs;
        // déjà là : rien n'est lancé
        assert_eq!(Wake::new().looked(true, s(0)), Step::Done(Ending::AlreadyThere));
        // arrivé pendant le délai de grâce (brew services à l'ouverture de session) : rien non plus
        let mut wake = Wake::new();
        assert_eq!(wake.looked(false, s(0)), Step::Wait(LOOK_EVERY));
        assert_eq!(wake.looked(false, s(3)), Step::Wait(LOOK_EVERY));
        assert_eq!(wake.looked(true, s(4)), Step::Done(Ending::AlreadyThere));
        // personne, même après le délai : lancé, puis on attend qu'il réponde
        let mut wake = Wake::new();
        assert_eq!(wake.looked(false, GRACE - s(1)), Step::Wait(LOOK_EVERY));
        assert_eq!(wake.looked(false, GRACE), Step::Launch);
        wake.launched(GRACE);
        wake.launched(GRACE + s(5)); // un second appel ne repousse pas l'attente
        assert_eq!(wake.looked(false, GRACE + s(2)), Step::Wait(LOOK_EVERY));
        assert_eq!(wake.looked(true, GRACE + s(3)), Step::Done(Ending::Launched));
        // lancé, mais muet
        let mut wake = Wake::new();
        assert_eq!(wake.looked(false, GRACE), Step::Launch);
        wake.launched(GRACE);
        assert_eq!(wake.looked(false, GRACE + STARTUP_WAIT - s(1)), Step::Wait(LOOK_EVERY));
        assert_eq!(wake.looked(false, GRACE + STARTUP_WAIT), Step::Done(Ending::LaunchedSilent));
        // lancé : on ne demande plus jamais de le relancer
        for t in 0..60 {
            assert_ne!(wake.looked(false, GRACE + s(t)), Step::Launch);
        }
    }

    #[test]
    fn what_she_says_is_short_and_never_a_terminal_command() {
        let said = [
            say::LAUNCHED,
            say::LAUNCHED_SILENT,
            say::NOT_FOUND,
            say::LAUNCH_FAILED,
            say::registered(true),
            say::registered(false),
            say::UNREGISTERED,
            say::ONLY_FROM_APP,
            say::NOT_OURS,
            say::FAILED,
            say::UNAVAILABLE,
        ];
        for text in said {
            assert!(!text.contains("capri ") && !text.contains("--") && !text.contains("brew"), "{text}");
            assert!(!text.contains("ollama serve") && !text.contains("ollama pull"), "{text}");
            assert!(text.chars().count() <= 70, "le bandeau reste court : {text}");
        }
    }
}
