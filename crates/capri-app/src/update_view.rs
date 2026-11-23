//! Chercher les mises à jour, case cochée : sur un thread à elle (« capri-maj »), elle regarde un
//! peu après son lancement, puis une fois par jour, s'il existe une version plus récente. Rien ne
//! retarde son apparition (le thread attend son heure) ni son départ (personne ne l'attend : en
//! quittant, l'app ne le rejoint pas). Ce qu'elle trouve est relevé par la boucle
//! (`App::handle_updates`).
//!
//! Quand regarder est décidé par `capri_chat::update::schedule`, quoi conclure par
//! `capri_chat::update` ; lire le manifeste, par `capri_net::update::fetch`, et rien d'autre.
//! D'elle-même, elle ne télécharge ni n'installe rien : le manifeste lu est rendu avec le verdict,
//! pour le geste qui installera (`install_view`). Le journal ne dit que la conclusion (à jour,
//! telle version disponible, vérification impossible et une cause courte), jamais une adresse.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use capri_chat::update::{self, Failure, Installed, Manifest, Schedule, Source, Verdict, say};

/// Ce que la veille a trouvé : le verdict, et le manifeste dont il vient.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub verdict: Verdict,
    pub manifest: Manifest,
}

/// La veille, tant que la case est cochée. La lâcher l'arrête (à son prochain réveil, ou à la fin
/// d'une lecture en cours, bornée).
pub struct UpdateWatch {
    found: Receiver<Found>,
    /// Lâché avec elle : le thread l'apprend en attendant, et s'arrête.
    _stop: Sender<()>,
}

impl UpdateWatch {
    /// Lance la veille, d'après l'environnement : jamais depuis un essai ni une répétition (voir
    /// `update_setup::source`), jamais en `https` là où ce système ne le parle pas. Rend `Err` avec
    /// ce qu'elle dirait de la case si elle ne regarde pas.
    pub fn start() -> Result<Self, &'static str> {
        let source = match crate::update_setup::source(|name| std::env::var_os(name)) {
            Ok(source) => source,
            Err(update::Offline::Rehearsal) => {
                log::info!("mises à jour : dossier d'essai ou répétition, aucune vérification");
                return Err(say::NOT_IN_REHEARSAL);
            }
            Err(update::Offline::NotThisMachine(err)) => {
                log::warn!("mises à jour : adresse d'essai refusée ({err}), aucune vérification");
                return Err(say::NOT_IN_REHEARSAL);
            }
        };
        if source.route().is_secure() && !capri_net::TLS {
            log::info!("mises à jour : pas de https sur ce système, aucune vérification");
            return Err(say::NO_TLS);
        }
        let schedule = match source {
            Source::GitHub => Schedule::new(),
            Source::ThisMachine { .. } => Schedule::with_grace(crate::update_setup::REHEARSAL_GRACE),
        };
        let (found_sender, found) = channel();
        let (stop, stopped) = channel();
        let installed = crate::update_setup::installed();
        log::info!(
            "mises à jour : {} regardé dans {} s, puis une fois par jour (installée : {})",
            source.name(),
            schedule.wait(Duration::ZERO).as_secs(),
            installed.version
        );
        let job = move || {
            let mut line = Online { source, stopped, started: Instant::now() };
            watch(&mut line, &installed, schedule, &mut |verdict| found_sender.send(verdict).is_ok());
            log::debug!("mises à jour : veille arrêtée");
        };
        match std::thread::Builder::new().name("capri-maj".to_owned()).spawn(job) {
            Ok(_) => Ok(Self { found, _stop: stop }),
            Err(err) => {
                log::warn!("mises à jour : pas de thread pour regarder : {err}");
                Err(say::NOT_CHECKING)
            }
        }
    }

    /// Ce qu'elle a trouvé en dernier depuis l'appel précédent, s'il y a du nouveau.
    pub fn heard(&self) -> Option<Found> {
        let mut last = None;
        loop {
            match self.found.try_recv() {
                Ok(verdict) => last = Some(verdict),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return last,
            }
        }
    }
}

/// Ce que la veille demande au monde : de quoi la remplacer dans les essais.
trait Line {
    /// Lit le manifeste.
    fn fetch(&mut self) -> Result<Vec<u8>, Failure>;
    /// Attend `duration` ; rend `false` si on lui demande de s'arrêter.
    fn wait(&mut self, duration: Duration) -> bool;
    /// Le temps passé depuis le début de la veille.
    fn elapsed(&self) -> Duration;
}

/// Regarde à l'heure dite, conclut, dit ce qu'elle a trouvé (`report` : `false` quand plus
/// personne n'écoute), et recommence, jusqu'à ce qu'on l'arrête.
fn watch(
    line: &mut impl Line,
    installed: &Installed,
    mut schedule: Schedule,
    report: &mut dyn FnMut(Found) -> bool,
) {
    loop {
        if !line.wait(schedule.wait(line.elapsed())) {
            return;
        }
        let concluded = line.fetch().and_then(|body| Manifest::parse(&body).map_err(Failure::Manifest));
        let succeeded = concluded.is_ok();
        let next = schedule.checked(line.elapsed(), succeeded);
        match concluded {
            Ok(manifest) => {
                let verdict = update::verdict(&manifest, installed);
                log::info!("mises à jour : {}", say::for_log(&verdict, installed));
                if !report(Found { verdict, manifest }) {
                    return;
                }
            }
            Err(failure) => log::warn!(
                "mises à jour : vérification impossible ({}), nouvel essai dans {} min",
                failure.for_log(),
                next.as_secs() / 60
            ),
        }
    }
}

/// Le vrai réseau : `capri_net`, par la règle de la source.
struct Online {
    source: Source,
    stopped: Receiver<()>,
    started: Instant,
}

impl Line for Online {
    fn fetch(&mut self) -> Result<Vec<u8>, Failure> {
        capri_net::update::fetch(
            &self.source.route(),
            &self.source.url(),
            capri_net::update::Limits::default(),
        )
    }

    fn wait(&mut self, duration: Duration) -> bool {
        // rien n'est jamais envoyé sur ce canal : seule sa fermeture compte (la veille lâchée)
        matches!(self.stopped.recv_timeout(duration), Err(RecvTimeoutError::Timeout))
    }

    fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use capri_chat::update::{Obstacle, Route, Version, schedule};

    use super::*;
    use crate::fake_server;

    const HERE: Installed =
        Installed { version: Version::new(0, 1, 0), bundle: 1, target: Some("macos-arm64") };

    fn manifest(version: &str) -> Vec<u8> {
        format!(
            r#"{{"format": 1, "version": "{version}", "bundle": 1, "targets": {{"macos-arm64": {{"files": [
                {{"name": "capri", "asset": "capri-macos-arm64", "size": 10, "sha256": "{}"}}]}}}}}}"#,
            "a".repeat(64)
        )
        .into_bytes()
    }

    /// Un faux monde, à l'horloge factice : chaque lecture rend la réponse suivante.
    struct Fake {
        clock: Duration,
        answers: Vec<Result<Vec<u8>, Failure>>,
        /// Les moments des lectures.
        fetched_at: Vec<Duration>,
        /// Arrêtée quand l'horloge dépasse ce moment.
        until: Duration,
    }

    impl Fake {
        fn new(answers: Vec<Result<Vec<u8>, Failure>>, until: Duration) -> Self {
            Self { clock: Duration::ZERO, answers, fetched_at: Vec::new(), until }
        }
    }

    impl Line for Fake {
        fn fetch(&mut self) -> Result<Vec<u8>, Failure> {
            self.fetched_at.push(self.clock);
            self.clock += Duration::from_millis(300); // une lecture prend un peu de temps
            if self.answers.is_empty() { Err(Failure::Unreachable) } else { self.answers.remove(0) }
        }

        fn wait(&mut self, duration: Duration) -> bool {
            self.clock += duration;
            self.clock <= self.until
        }

        fn elapsed(&self) -> Duration {
            self.clock
        }
    }

    #[test]
    fn she_looks_after_her_launch_then_once_a_day_and_says_what_she_found() {
        let mut fake = Fake::new(vec![Ok(manifest("0.1.0")), Ok(manifest("0.2.0"))], 3 * schedule::EVERY);
        let mut found = Vec::new();
        watch(&mut fake, &HERE, Schedule::new(), &mut |verdict| {
            found.push(verdict.verdict);
            true
        });
        assert_eq!(fake.fetched_at[0], schedule::GRACE, "pas avant le délai de grâce");
        assert!(fake.fetched_at.windows(2).all(|pair| pair[1] - pair[0] >= schedule::RETRY_FIRST));
        assert_eq!(fake.fetched_at[1] - fake.fetched_at[0], schedule::EVERY + Duration::from_millis(300));
        assert_eq!(
            found,
            [
                Verdict::UpToDate { latest: Version::new(0, 1, 0) },
                Verdict::Installable { latest: Version::new(0, 2, 0) }
            ]
        );
    }

    #[test]
    fn a_failure_is_retried_later_never_quickly_and_reports_nothing() {
        let answers = vec![
            Err(Failure::TimedOut),
            Err(Failure::Unreachable),
            Ok(b"{}".to_vec()),
            Ok(manifest("0.3.0")),
        ];
        let mut fake = Fake::new(answers, 2 * schedule::EVERY);
        let mut found = Vec::new();
        watch(&mut fake, &HERE, Schedule::new(), &mut |verdict| {
            found.push(verdict.verdict);
            true
        });
        let gaps: Vec<Duration> = fake.fetched_at.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert!(gaps.iter().all(|gap| *gap >= schedule::RETRY_FIRST), "{gaps:?}");
        assert!(gaps[1] > gaps[0], "de plus en plus tard : {gaps:?}");
        // un manifeste illisible est un échec : rien n'est dit, et elle réessaie plus tard
        assert_eq!(found, [Verdict::Installable { latest: Version::new(0, 3, 0) }]);
        assert!(fake.fetched_at.len() < 12, "{} lectures en deux jours", fake.fetched_at.len());
    }

    #[test]
    fn she_stops_when_asked_or_when_nobody_listens_any_more() {
        // arrêtée avant même la première lecture : rien n'est lu
        let mut fake = Fake::new(vec![Ok(manifest("0.2.0"))], schedule::GRACE - Duration::from_secs(1));
        watch(&mut fake, &HERE, Schedule::new(), &mut |_| true);
        assert!(fake.fetched_at.is_empty());
        // plus personne n'écoute : elle s'arrête après sa conclusion
        let mut fake = Fake::new(vec![Ok(manifest("0.2.0")), Ok(manifest("0.2.0"))], 10 * schedule::EVERY);
        watch(&mut fake, &HERE, Schedule::new(), &mut |_| false);
        assert_eq!(fake.fetched_at.len(), 1);
    }

    #[test]
    fn a_version_the_app_cannot_take_is_still_said() {
        let mut fake = Fake::new(vec![Ok(manifest("0.2.0"))], schedule::GRACE + Duration::from_secs(1));
        let windows_only = Installed { target: Some("linux-x86_64"), ..HERE };
        let mut found = Vec::new();
        watch(&mut fake, &windows_only, Schedule::new(), &mut |verdict| {
            found.push(verdict.verdict);
            true
        });
        assert_eq!(
            found,
            [Verdict::NotInstallable { latest: Version::new(0, 2, 0), why: Obstacle::NoTarget }]
        );
    }

    /// La vraie lecture (`capri_net::update::fetch`), contre un faux serveur de cette machine qui
    /// redirige deux fois comme GitHub ; l'horloge et l'attente restent factices.
    #[test]
    fn the_real_fetch_follows_a_local_server_like_github() {
        struct Half {
            fake: Fake,
            url: String,
            route: Route,
        }
        impl Line for Half {
            fn fetch(&mut self) -> Result<Vec<u8>, Failure> {
                self.fake.fetched_at.push(self.fake.clock);
                capri_net::update::fetch(&self.route, &self.url, capri_net::update::Limits::default())
            }
            fn wait(&mut self, duration: Duration) -> bool {
                self.fake.wait(duration)
            }
            fn elapsed(&self) -> Duration {
                self.fake.elapsed()
            }
        }
        let redirect = |location: &str| {
            format!(
                "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
        };
        let body = String::from_utf8(manifest("0.4.2")).expect("utf-8");
        let (address, requests) = fake_server::serve(vec![
            redirect("/releases/download/v0.4.2/latest.json"),
            redirect("/signe/latest.json?sig=SeCrEt"),
            fake_server::response("200 OK", "application/octet-stream", &body),
        ]);
        let mut half = Half {
            fake: Fake::new(vec![], schedule::GRACE + Duration::from_secs(1)),
            url: format!("http://{address}/releases/latest/download/latest.json"),
            route: Route::this_machine(address.port()).expect("la règle"),
        };
        let mut found = Vec::new();
        watch(&mut half, &HERE, Schedule::new(), &mut |verdict| {
            found.push(verdict.verdict);
            true
        });
        assert_eq!(found, [Verdict::Installable { latest: Version::new(0, 4, 2) }]);
        assert_eq!(requests.try_iter().count(), 3);
    }
}
