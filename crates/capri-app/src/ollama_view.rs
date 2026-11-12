//! Au lancement, case « Se lancer à l'ouverture de session » cochée : si son cerveau est Ollama
//! sur cette machine et que rien n'y répond, elle le lance. Sur un thread à part (« capri-ollama ») :
//! rien ne retarde son apparition à l'écran ; ce qu'elle en dit est relevé par la boucle.
//!
//! Ce qu'il faut faire est décidé par `capri_chat::startup` (le plan, le délai laissé à un Ollama
//! qui démarre, où le chercher) ; savoir si quelque chose répond, par `capri_net::knock` (cette
//! machine seulement, rien d'envoyé) ; le lancer, par `capri_platform::launch`. Elle ne lance
//! qu'Ollama, une fois, et ne l'arrête jamais.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use capri_chat::Endpoint;
use capri_chat::startup::{self, Ending, Ollama, Plan, Step, Wake, say};
use capri_claude::Tone;

use crate::brain_setup;

/// Ce qu'elle dit à la fin.
pub type Notice = (&'static str, Tone);

/// Le travail en cours, tant qu'il n'a pas dit son dernier mot.
pub struct OllamaWatch {
    notices: Receiver<Notice>,
}

impl OllamaWatch {
    /// Lance la veille, si la case est cochée (`registered`) ; le cerveau est lu sur le thread.
    pub fn start(registered: bool) -> Option<Self> {
        if !registered {
            return None;
        }
        let (send, notices) = channel();
        let job = move || {
            let brain = brain_setup::config_dir()
                .and_then(|directory| brain_setup::load(&directory))
                .inspect_err(|err| log::warn!("Ollama : cerveau illisible, rien n'est lancé : {err:#}"))
                .ok()
                .flatten()
                .map(|setup| setup.brain);
            let endpoint = match startup::plan(true, brain.as_ref()) {
                Plan::Watch(endpoint) => endpoint,
                Plan::Leave(why) => return log::debug!("Ollama : rien à lancer ({why:?})"),
            };
            let home = std::env::var_os("HOME").filter(|home| !home.is_empty()).map(PathBuf::from);
            if let Some(notice) = watch(&endpoint, home.as_deref(), &mut ThisMac::new()) {
                let _ = send.send(notice);
            }
        };
        match std::thread::Builder::new().name("capri-ollama".to_owned()).spawn(job) {
            Ok(_) => Some(Self { notices }),
            Err(err) => {
                log::warn!("Ollama : pas de thread pour le veiller : {err}");
                None
            }
        }
    }

    /// Où en est la veille.
    pub fn heard(&self) -> Heard {
        match self.notices.try_recv() {
            Ok(notice) => Heard::Said(notice),
            Err(TryRecvError::Empty) => Heard::Nothing,
            Err(TryRecvError::Disconnected) => Heard::Over,
        }
    }
}

/// Ce que la veille a donné depuis la dernière fois.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heard {
    /// Elle continue.
    Nothing,
    /// Finie, avec quelque chose à dire.
    Said(Notice),
    /// Finie, sans rien à dire.
    Over,
}

/// Ce que la veille demande à la machine : de quoi la remplacer dans les essais.
trait Machine {
    /// Quelque chose répond-il à cette adresse de cette machine ?
    fn answers(&mut self, endpoint: &Endpoint) -> bool;
    /// Cet Ollama est-il installé là ?
    fn has(&self, ollama: &Ollama) -> bool;
    fn launch(&mut self, ollama: &Ollama, endpoint: &Endpoint) -> Result<(), String>;
    fn sleep(&mut self, duration: Duration);
    /// Le temps passé depuis le début.
    fn elapsed(&self) -> Duration;
}

/// Regarde, laisse un peu de temps à un Ollama qui démarre, le lance une fois s'il le faut, et
/// attend qu'il réponde. Rend ce qu'elle doit dire, s'il y a quelque chose à dire.
fn watch(endpoint: &Endpoint, home: Option<&Path>, machine: &mut impl Machine) -> Option<Notice> {
    let mut wake = Wake::new();
    loop {
        let answered = machine.answers(endpoint);
        match wake.looked(answered, machine.elapsed()) {
            Step::Wait(duration) => machine.sleep(duration),
            Step::Launch => {
                let found =
                    startup::candidates(home, endpoint).into_iter().find(|ollama| machine.has(ollama));
                let Some(ollama) = found else {
                    log::warn!(
                        "Ollama ne répond pas à {}, et il n'est à aucun emplacement connu",
                        endpoint.display_host()
                    );
                    return Some((say::NOT_FOUND, Tone::Failed));
                };
                if let Err(err) = machine.launch(&ollama, endpoint) {
                    log::error!("Ollama pas lancé ({}) : {err}", ollama.path().display());
                    return Some((say::LAUNCH_FAILED, Tone::Failed));
                }
                log::info!("Ollama lancé ({}) pour {}", ollama.path().display(), endpoint.display_host());
                wake.launched(machine.elapsed());
            }
            Step::Done(Ending::AlreadyThere) => {
                log::debug!("quelque chose répond déjà à {} : rien n'est lancé", endpoint.display_host());
                return None;
            }
            Step::Done(Ending::Launched) => return Some((say::LAUNCHED, Tone::Done)),
            Step::Done(Ending::LaunchedSilent) => {
                log::warn!("Ollama lancé, mais rien ne répond à {}", endpoint.display_host());
                return Some((say::LAUNCHED_SILENT, Tone::Failed));
            }
        }
    }
}

/// Le vrai Mac : `capri_net` pour frapper, `capri_platform` pour lancer.
struct ThisMac {
    started: Instant,
}

impl ThisMac {
    fn new() -> Self {
        Self { started: Instant::now() }
    }
}

impl Machine for ThisMac {
    fn answers(&mut self, endpoint: &Endpoint) -> bool {
        match capri_net::knock(endpoint) {
            Ok(presence) => presence == capri_net::Presence::Answers,
            // `startup::plan` ne donne que des adresses de cette machine : ceci ne devrait pas
            // arriver, et dans le doute on ne lance rien
            Err(refused) => {
                log::warn!("Ollama : adresse refusée ({refused:?})");
                true
            }
        }
    }

    fn has(&self, ollama: &Ollama) -> bool {
        match ollama {
            Ollama::App(app) => capri_platform::launch::is_app(app),
            Ollama::Program(program) => capri_platform::launch::is_program(program),
        }
    }

    fn launch(&mut self, ollama: &Ollama, endpoint: &Endpoint) -> Result<(), String> {
        let launched = match ollama {
            Ollama::App(app) => capri_platform::launch::open_app(app),
            Ollama::Program(program) => {
                capri_platform::launch::start_program(program, ollama.args(), &ollama.env(endpoint)).map(
                    |pid| {
                        log::info!("Ollama : processus {pid}");
                    },
                )
            }
        };
        launched.map_err(|err| err.to_string())
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    /// Une fausse machine, à l'horloge factice : rien n'est lancé pour de vrai.
    struct Fake {
        clock: Duration,
        /// Le moment où quelque chose commence à répondre, s'il vient.
        answers_from: Option<Duration>,
        installed: Vec<PathBuf>,
        /// Ce qui a été lancé, avec son environnement.
        launched: Vec<(PathBuf, Vec<(&'static str, String)>)>,
        /// Après un lancement, le temps qu'il met à répondre (`None` : jamais).
        starts_in: Option<Duration>,
        fails: bool,
    }

    impl Fake {
        fn new(installed: &[&str]) -> Self {
            Self {
                clock: Duration::ZERO,
                answers_from: None,
                installed: installed.iter().map(PathBuf::from).collect(),
                launched: Vec::new(),
                starts_in: Some(Duration::from_secs(2)),
                fails: false,
            }
        }
    }

    impl Machine for Fake {
        fn answers(&mut self, _: &Endpoint) -> bool {
            self.clock += Duration::from_millis(5); // un coup d'œil prend un peu de temps
            self.answers_from.is_some_and(|from| self.clock >= from)
        }

        fn has(&self, ollama: &Ollama) -> bool {
            self.installed.iter().any(|path| path == ollama.path())
        }

        fn launch(&mut self, ollama: &Ollama, endpoint: &Endpoint) -> Result<(), String> {
            if self.fails {
                return Err("refusé".to_owned());
            }
            self.launched.push((ollama.path().to_owned(), ollama.env(endpoint)));
            self.answers_from = self.starts_in.map(|delay| self.clock + delay);
            Ok(())
        }

        fn sleep(&mut self, duration: Duration) {
            self.clock += duration;
        }

        fn elapsed(&self) -> Duration {
            self.clock
        }
    }

    fn here() -> Endpoint {
        Endpoint::parse("http://localhost:11434/v1").expect("valide")
    }

    #[test]
    fn an_ollama_already_running_is_left_alone_and_nothing_is_said() {
        let mut mac = Fake::new(&["/opt/homebrew/bin/ollama"]);
        mac.answers_from = Some(Duration::ZERO);
        assert_eq!(watch(&here(), None, &mut mac), None);
        assert!(mac.launched.is_empty());
    }

    #[test]
    fn an_ollama_starting_on_its_own_is_given_time_and_not_launched_twice() {
        // brew services le lance à l'ouverture de session, en même temps qu'elle
        let mut mac = Fake::new(&["/opt/homebrew/bin/ollama"]);
        mac.answers_from = Some(Duration::from_secs(3));
        assert_eq!(watch(&here(), None, &mut mac), None);
        assert!(mac.launched.is_empty());
    }

    #[test]
    fn nobody_there_homebrew_s_ollama_is_launched_once_for_the_chosen_address() {
        let mut mac = Fake::new(&["/opt/homebrew/bin/ollama", "/usr/local/bin/ollama"]);
        assert_eq!(
            watch(&here(), Some(Path::new("/Users/moi")), &mut mac),
            Some((say::LAUNCHED, Tone::Done))
        );
        assert_eq!(
            mac.launched,
            [(
                PathBuf::from("/opt/homebrew/bin/ollama"),
                vec![("OLLAMA_HOST", "127.0.0.1:11434".to_owned())]
            )]
        );
        assert!(mac.clock >= startup::GRACE, "le délai de grâce est passé avant : {:?}", mac.clock);

        // une autre adresse de cette machine : il y écoute
        let mut mac = Fake::new(&["/opt/homebrew/bin/ollama"]);
        let elsewhere = Endpoint::parse("http://127.0.0.1:11500/v1").expect("valide");
        watch(&elsewhere, None, &mut mac);
        assert_eq!(mac.launched[0].1, [("OLLAMA_HOST", "127.0.0.1:11500".to_owned())]);
    }

    #[test]
    fn the_app_is_preferred_for_its_own_address_and_its_program_elsewhere() {
        let installed = ["/Applications/Ollama.app", "/Applications/Ollama.app/Contents/Resources/ollama"];
        let mut mac = Fake::new(&installed);
        watch(&here(), None, &mut mac);
        assert_eq!(mac.launched, [(PathBuf::from("/Applications/Ollama.app"), vec![])]);

        let mut mac = Fake::new(&installed);
        let elsewhere = Endpoint::parse("http://127.0.0.1:11500/v1").expect("valide");
        watch(&elsewhere, None, &mut mac);
        assert_eq!(mac.launched.len(), 1);
        assert_eq!(mac.launched[0].0, PathBuf::from("/Applications/Ollama.app/Contents/Resources/ollama"));
    }

    #[test]
    fn what_goes_wrong_is_said_without_a_terminal_command() {
        let mut mac = Fake::new(&[]);
        assert_eq!(watch(&here(), None, &mut mac), Some((say::NOT_FOUND, Tone::Failed)));
        assert!(mac.launched.is_empty());

        let mut mac = Fake::new(&["/opt/homebrew/bin/ollama"]);
        mac.fails = true;
        assert_eq!(watch(&here(), None, &mut mac), Some((say::LAUNCH_FAILED, Tone::Failed)));

        let mut mac = Fake::new(&["/opt/homebrew/bin/ollama"]);
        mac.starts_in = None;
        assert_eq!(watch(&here(), None, &mut mac), Some((say::LAUNCHED_SILENT, Tone::Failed)));
        assert_eq!(mac.launched.len(), 1, "lancé une seule fois, même muet");
        assert!(mac.clock >= startup::GRACE + startup::STARTUP_WAIT);
    }

    /// La vraie façon de frapper (`capri_net::knock`), contre un faux Ollama local qui n'écoute
    /// qu'une fois « lancé » : rien n'est lancé pour de vrai.
    #[test]
    fn the_real_knock_sees_a_server_appear_after_the_launch() {
        struct Half {
            fake: Fake,
            port: u16,
            listener: Option<TcpListener>,
        }
        impl Machine for Half {
            fn answers(&mut self, _: &Endpoint) -> bool {
                let endpoint = Endpoint::parse(&format!("http://127.0.0.1:{}", self.port)).expect("valide");
                ThisMac::new().answers(&endpoint)
            }
            fn has(&self, ollama: &Ollama) -> bool {
                self.fake.has(ollama)
            }
            fn launch(&mut self, ollama: &Ollama, endpoint: &Endpoint) -> Result<(), String> {
                self.listener =
                    Some(TcpListener::bind(("127.0.0.1", self.port)).map_err(|err| err.to_string())?);
                self.fake.launch(ollama, endpoint)
            }
            fn sleep(&mut self, duration: Duration) {
                self.fake.sleep(duration);
            }
            fn elapsed(&self) -> Duration {
                self.fake.elapsed()
            }
        }
        let port = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("un port").port();
        let mut half = Half { fake: Fake::new(&["/opt/homebrew/bin/ollama"]), port, listener: None };
        let endpoint = Endpoint::parse(&format!("http://127.0.0.1:{port}/v1")).expect("valide");
        assert_eq!(watch(&endpoint, None, &mut half), Some((say::LAUNCHED, Tone::Done)));
        assert!(half.listener.is_some());
    }
}
