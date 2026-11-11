//! `capri music rehearse`, en version de débogage seulement : guette les annonces comme Capri le
//! fait, et prouve, sans dépendre de Spotify ni de Musique, qu'une annonce d'essai postée par un
//! autre processus est reçue et que son état est lu. L'annonce d'essai porte de faux titre,
//! artiste et album (`capri music rehearse <titre> <artiste> <album>` pour les choisir), qui ne
//! sont jamais imprimés : rien ne les lit.
//!
//! N'imprime que des phrases écrites ici, l'état lu, des oui, des non et des nombres ; elle dit
//! aussi combien d'annonces chacun des lecteurs connus a faites pendant ce temps, et s'il tourne.
//! Ne joue aucun son, ne lit et n'écrit aucun réglage, ne demande aucune permission, ne pilote
//! aucun lecteur. Rend 0 si tout est conforme (ou si ce système ne sait pas guetter, ce qu'elle
//! dit), 1 sinon.
//!
//! `capri music announce …` est le poste d'essai qu'elle lance, dans un autre processus : il
//! poste une annonce, comme un lecteur, sous le nom d'essai. Pas à lancer à la main.
//!
//! Et, dans la vraie app, `CAPRI_MUSIC_REHEARSAL=1` (ou `=titre;artiste;album`) : voir [`InApp`].

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use capri_core::music::{ALIVE_EVERY, PlayerState};
use capri_platform::music::{self, Announcements, MusicError, Player};

/// Un identifiant de bundle qu'aucune app n'a : le lecteur d'essai « ne tourne pas », de quoi
/// exercer l'oubli d'un lecteur disparu.
pub const ABSENT_BUNDLE: &str = "fr.capri.essai.lecteur-introuvable";
/// Les faux champs de l'annonce d'essai, faute d'autres.
const FAKES: [&str; 3] = ["Faux titre d'essai", "Faux artiste d'essai", "Faux album d'essai"];
/// Leurs clés, celles des lecteurs.
const FAKE_KEYS: [&str; 3] = ["Name", "Artist", "Album"];
/// Combien de temps attendre une annonce.
const WAIT: Duration = Duration::from_secs(3);

/// Le nom de l'annonce d'essai : propre au processus qui guette, pour qu'aucune autre répétition
/// (ni aucune autre Capri) n'y réponde.
pub fn announcement() -> String {
    format!("fr.capri.essai.annonce.{}", std::process::id())
}

pub fn run_cli(args: &[String]) -> i32 {
    match args {
        [action] if action == "rehearse" => run([FAKES[0], FAKES[1], FAKES[2]]),
        [action, title, artist, album] if action == "rehearse" => run([title, artist, album]),
        // lancé par la répétition, pas à la main
        [action, name, state, title, artist, album] if action == "announce" => {
            let state = (state != "-").then_some(state.as_str());
            let others: Vec<(&str, &str)> =
                FAKE_KEYS.into_iter().zip([title.as_str(), artist.as_str(), album.as_str()]).collect();
            i32::from(music::announce(name, state, &others).is_err())
        }
        _ => {
            eprintln!(
                "usage : capri music rehearse [<titre> <artiste> <album>]   guetter une annonce d'essai"
            );
            2
        }
    }
}

/// Poste l'annonce d'essai `name` depuis un autre processus (ce même binaire), et attend qu'il ait
/// fini. Sa sortie n'est pas reprise.
fn post(name: &str, state: Option<&str>, fakes: [&str; 3]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let status = Command::new(exe)
        .args(["music", "announce", name, state.unwrap_or("-"), fakes[0], fakes[1], fakes[2]])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("code {status}")) }
}

fn state_label(state: PlayerState) -> &'static str {
    match state {
        PlayerState::Playing => "en lecture",
        PlayerState::Paused => "en pause",
        PlayerState::Stopped => "arrêté",
        PlayerState::Unknown => "inconnu",
    }
}

fn player_label(player: Player) -> &'static str {
    match player {
        Player::Spotify => "Spotify",
        Player::Music => "Musique",
        Player::Rehearsal => "le lecteur d'essai",
    }
}

fn yes(value: bool) -> &'static str {
    if value { "oui" } else { "non" }
}

fn of(watch: &Announcements, player: Player) -> (PlayerState, u32) {
    let state = watch.states().find(|(p, _)| *p == player).map_or(PlayerState::Unknown, |(_, s)| s);
    let received = watch.received().find(|(p, _)| *p == player).map_or(0, |(_, n)| n);
    (state, received)
}

fn run(fakes: [&str; 3]) -> i32 {
    let name = announcement();
    let watch = match Announcements::rehearsal(&name, ABSENT_BUNDLE, true) {
        Ok(watch) => watch,
        Err(MusicError::Unavailable) => {
            println!("elle ne sait pas, sur ce système, si de la musique joue");
            return i32::from(music::AVAILABLE);
        }
        Err(err) => {
            println!("ÉCHEC : {err}");
            return 1;
        }
    };
    println!("abonnée aux annonces de Spotify, de Musique et du lecteur d'essai (un autre processus)");
    let mut failed = 0;
    let mut check = |ok: bool, what: String| {
        println!("{} : {what}", if ok { "ok" } else { "ÉCHEC" });
        failed += usize::from(!ok);
    };

    // chaque annonce, postée par un autre processus : reçue, et son état lu
    let steps: [(Option<&str>, &str); 8] = [
        (Some("Playing"), "lecture"),
        (Some("Paused"), "pause"),
        (Some("Playing"), "lecture"),
        (Some("Playing"), "morceau suivant, sans pause"),
        (Some("Stopped"), "arrêt"),
        (Some("Rewinding"), "une valeur inconnue"),
        (Some("Playing"), "lecture"),
        (None, "sans état"),
    ];
    for (state, what) in steps {
        let (_, before) = of(&watch, Player::Rehearsal);
        let sent = Instant::now();
        if let Err(err) = post(&name, state, fakes) {
            check(false, format!("le poste d'essai a échoué ({err})"));
            continue;
        }
        while of(&watch, Player::Rehearsal).1 == before && sent.elapsed() < WAIT {
            music::run_for(Duration::from_millis(10));
        }
        let (read, after) = of(&watch, Player::Rehearsal);
        let expected = PlayerState::announced(state);
        let swaying = capri_core::music::playing(watch.states().map(|(_, s)| s));
        check(
            after == before + 1 && read == expected,
            format!(
                "annonce d'essai « {what} » : reçue {} en {:.1} ms ; état lu : {} ; elle se balancerait : {}",
                yes(after > before),
                sent.elapsed().as_secs_f64() * 1000.0,
                state_label(read),
                yes(swaying)
            ),
        );
    }

    // le lecteur disparu : cru en lecture, introuvable, oublié
    let (_, before) = of(&watch, Player::Rehearsal);
    let sent = Instant::now();
    let posted = post(&name, Some("Playing"), fakes);
    while of(&watch, Player::Rehearsal).1 == before && sent.elapsed() < WAIT {
        music::run_for(Duration::from_millis(10));
    }
    let believed = of(&watch, Player::Rehearsal).0 == PlayerState::Playing;
    let asked = Instant::now();
    let forgotten = watch.forget_gone();
    let took = asked.elapsed();
    check(
        posted.is_ok()
            && believed
            && forgotten >= 1
            && of(&watch, Player::Rehearsal).0 == PlayerState::Unknown,
        format!(
            "le lecteur d'essai, cru en lecture : {} ; introuvable, oublié : {} ({forgotten} oublié(s), \
             demandé au système en {:.2} ms)",
            yes(believed),
            yes(of(&watch, Player::Rehearsal).0 == PlayerState::Unknown),
            took.as_secs_f64() * 1000.0
        ),
    );

    // les vrais lecteurs : seulement ce qu'elle en sait, et s'ils tournent (par notre constante)
    for known in music::KNOWN {
        let (state, received) = of(&watch, known.player);
        let running = music::is_running(known.bundle).map_or("on ne sait pas", yes);
        println!(
            "{} : {received} annonce(s) pendant la répétition, état connu : {} ; tourne en ce moment : {running}",
            player_label(known.player),
            state_label(state)
        );
    }
    let asked = Instant::now();
    for known in music::KNOWN {
        let _ = music::is_running(known.bundle);
    }
    println!(
        "vérifier que les {} lecteurs connus tournent : {:.2} ms (toutes les {} s, seulement pendant une lecture)",
        music::KNOWN.len(),
        asked.elapsed().as_secs_f64() * 1000.0,
        ALIVE_EVERY.as_secs()
    );
    if failed == 0 {
        println!("tout est conforme");
        0
    } else {
        println!("{failed} vérification(s) en échec");
        1
    }
}

/// En version de débogage, `CAPRI_MUSIC_REHEARSAL=1` au lancement de la vraie app (avec des
/// lieux d'essai) : l'app guette aussi le lecteur d'essai, poste ses annonces depuis un autre
/// processus, et écrit dans le journal ce qu'elle en a reçu, avec ce qu'en disent AppKit (l'app
/// est-elle active ? la livraison est-elle suspendue ?) et un abonnement ordinaire, pour comparer.
/// Puis elle quitte. `=titre;artiste;album` choisit les faux champs de l'annonce, jamais écrits.
///
/// Les étapes : une lecture (reçue alors que l'app n'est pas active ?), l'oubli du lecteur
/// d'essai introuvable au bout de quelques secondes (la vérification de l'app elle-même), puis,
/// la livraison suspendue à la main comme AppKit le fait quand une app cesse d'être active, une
/// autre lecture (Capri la reçoit-elle, et l'abonnement ordinaire ?), et la pause.
pub struct InApp {
    announcement: String,
    fakes: [String; 3],
    ordinary: Option<Announcements>,
    /// Les vrais lecteurs, guettés à part : seulement pour dire combien d'annonces ils ont faites.
    known: Option<Announcements>,
    /// Ce que l'abonnement ordinaire avait reçu avant la suspension.
    ordinary_before: u32,
    stage: Stage,
    since: Instant,
    failed: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Warmup,
    Playing,
    Forgotten,
    /// Livraison suspendue : une annonce pour l'abonnement ordinaire, qui doit être retenue.
    Held,
    Suspended,
    Paused,
    Done,
}

/// Le temps laissé à l'abonnement ordinaire pour recevoir une annonce qu'on retient.
const HOLD: Duration = Duration::from_millis(500);

impl InApp {
    pub fn from_env() -> Option<Self> {
        let value = std::env::var("CAPRI_MUSIC_REHEARSAL").ok()?;
        let parts: Vec<&str> = value.split(';').collect();
        let fakes = match parts.as_slice() {
            [title, artist, album] => [(*title).to_owned(), (*artist).to_owned(), (*album).to_owned()],
            _ => FAKES.map(str::to_owned),
        };
        log::info!("répétition de la musique : le lecteur d'essai, depuis un autre processus");
        Some(Self {
            announcement: announcement(),
            fakes,
            ordinary: None,
            known: None,
            ordinary_before: 0,
            stage: Stage::Warmup,
            since: Instant::now(),
            failed: 0,
        })
    }

    /// La vue de la musique de l'app, qui guette aussi le lecteur d'essai.
    pub fn view(&self, enabled: bool) -> crate::music_view::MusicView {
        crate::music_view::MusicView::rehearsal(enabled, self.announcement.clone(), ABSENT_BUNDLE)
    }

    /// Le nom d'essai de l'abonnement ordinaire : un autre que celui de Capri, sans quoi la
    /// livraison immédiate de son annonce viderait aussi sa file (« it has the effect of first
    /// flushing any queued notifications », dit l'en-tête).
    fn ordinary_name(&self) -> String {
        format!("{}.ordinaire", self.announcement)
    }

    fn post(&mut self, state: &str) {
        let name = self.announcement.clone();
        self.post_to(&name, state);
    }

    fn post_to(&mut self, name: &str, state: &str) {
        let fakes = [self.fakes[0].as_str(), self.fakes[1].as_str(), self.fakes[2].as_str()];
        if let Err(err) = post(name, Some(state), fakes) {
            log::error!("répétition de la musique : ÉCHEC, le poste d'essai a échoué ({err})");
            self.failed += 1;
        }
        self.since = Instant::now();
    }

    fn fail(&mut self, what: &str) -> bool {
        log::error!("répétition de la musique : ÉCHEC, {what}");
        self.failed += 1;
        self.finish()
    }

    fn finish(&mut self) -> bool {
        music::set_suspended(false);
        self.stage = Stage::Done;
        if let Some(known) = &self.known {
            for (player, received) in known.received() {
                log::info!(
                    "répétition de la musique : {} a fait {received} annonce(s) pendant ce temps",
                    player_label(player)
                );
            }
        }
        if self.failed == 0 {
            log::info!("répétition de la musique : tout est conforme");
        } else {
            log::error!("répétition de la musique : {} vérification(s) en échec", self.failed);
        }
        true
    }

    /// Un pas, à chaque tour de boucle ; `true` quand c'est fini et qu'il faut quitter.
    pub fn step(&mut self, r: &crate::Running) -> bool {
        let elapsed = self.since.elapsed();
        let playing = r.music.playing();
        match self.stage {
            // laisser l'app s'installer
            Stage::Warmup if elapsed < Duration::from_millis(800) => false,
            Stage::Warmup => {
                match Announcements::ordinary(&self.ordinary_name(), ABSENT_BUNDLE) {
                    Ok(ordinary) => self.ordinary = Some(ordinary),
                    Err(err) => log::warn!("répétition de la musique : pas d'abonnement ordinaire ({err})"),
                }
                self.known = Announcements::new().ok();
                self.post("Playing");
                self.stage = Stage::Playing;
                false
            }
            Stage::Playing if playing => {
                let delivery = music::delivery();
                let ordinary = self.ordinary_received();
                let inactive = delivery.is_some_and(|d| !d.app_active);
                log::info!(
                    "répétition de la musique : annonce d'essai reçue (en lecture) en {:.1} ms{} ; app active : {} ; \
                     livraison suspendue par AppKit : {} ; reçues par l'abonnement ordinaire : {}",
                    elapsed.as_secs_f64() * 1000.0,
                    if inactive { ", alors que l'app n'est pas active" } else { "" },
                    delivery.map_or("on ne sait pas", |d| yes(d.app_active)),
                    delivery.map_or("on ne sait pas", |d| yes(d.suspended)),
                    ordinary.map_or_else(|| "on ne sait pas".to_owned(), |n| n.to_string())
                );
                self.since = Instant::now();
                self.stage = Stage::Forgotten;
                false
            }
            Stage::Playing if elapsed > WAIT => self.fail("l'annonce de lecture n'est pas arrivée"),
            Stage::Forgotten if !playing => {
                log::info!(
                    "répétition de la musique : le lecteur d'essai, introuvable, oublié au bout de {:.1} s",
                    elapsed.as_secs_f64()
                );
                // comme AppKit quand une app cesse d'être active
                music::set_suspended(true);
                self.ordinary_before = self.ordinary_received().unwrap_or(0);
                log::info!(
                    "répétition de la musique : livraison suspendue à la main : {}",
                    music::delivery().map_or("on ne sait pas", |d| yes(d.suspended))
                );
                let ordinary = self.ordinary_name();
                self.post_to(&ordinary, "Playing");
                self.stage = Stage::Held;
                false
            }
            Stage::Forgotten if elapsed > ALIVE_EVERY + WAIT => {
                self.fail("le lecteur d'essai introuvable n'a pas été oublié")
            }
            Stage::Held if elapsed < HOLD => false,
            Stage::Held => {
                let held = self.ordinary_received().map(|n| n.saturating_sub(self.ordinary_before));
                log::info!(
                    "répétition de la musique : livraison suspendue, annonce postée pour l'abonnement ordinaire : \
                     reçue au bout de {:.1} s : {}",
                    elapsed.as_secs_f64(),
                    held.map_or("on ne sait pas", |n| yes(n > 0))
                );
                self.post("Playing");
                self.stage = Stage::Suspended;
                false
            }
            Stage::Suspended if playing => {
                let ordinary = self.ordinary_received().map(|n| n.saturating_sub(self.ordinary_before));
                log::info!(
                    "répétition de la musique : livraison suspendue, annonce d'essai reçue par Capri (en lecture) en \
                     {:.1} ms ; l'abonnement ordinaire a reçu depuis la suspension : {} (sa file vidée par la \
                     livraison immédiate, comme le dit l'en-tête)",
                    elapsed.as_secs_f64() * 1000.0,
                    ordinary.map_or_else(|| "on ne sait pas".to_owned(), |n| n.to_string())
                );
                self.post("Paused");
                self.stage = Stage::Paused;
                false
            }
            Stage::Suspended if elapsed > WAIT => {
                self.fail("livraison suspendue, l'annonce de lecture n'est pas arrivée")
            }
            Stage::Paused if !playing => {
                log::info!(
                    "répétition de la musique : livraison suspendue, annonce d'essai reçue par Capri (en pause) en \
                     {:.1} ms",
                    elapsed.as_secs_f64() * 1000.0
                );
                self.finish()
            }
            Stage::Paused if elapsed > WAIT => self.fail("l'annonce de pause n'est pas arrivée"),
            _ => false,
        }
    }

    /// Combien d'annonces l'abonnement ordinaire a reçues (un nombre, rien de leur contenu).
    fn ordinary_received(&self) -> Option<u32> {
        self.ordinary.as_ref().map(|ordinary| ordinary.received().map(|(_, n)| n).sum())
    }
}
