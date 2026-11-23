//! Le programme de fin d'une mise à jour : `capri update finish <app installée>`, une sous-commande
//! réservée à cela. C'est le `capri` de la **nouvelle** app, dans le dossier caché à côté de
//! l'installée, que l'ancienne Capri lance sur le geste qui la redémarre
//! (`capri_platform::launch::hand_over`) ; rien d'autre ne le lance. Le protocole du relais est
//! décrit dans `capri_core::bundle` : c'est un contrat entre deux versions.
//!
//! 1. Il vérifie sa place : son programme est celui de la nouvelle app d'un dossier caché à nous,
//!    dans le même dossier que l'app installée qu'on lui donne. Puis il dit qu'il prend le relais
//!    (une ligne sur sa sortie).
//! 2. Il attend que l'ancienne soit sortie, **sans jamais signaler aucun processus** : son entrée
//!    est un tube dont l'ancienne garde l'autre bout ; elle y écrit « go » juste avant de se
//!    quitter, et le tube se ferme quand elle sort. Au plus [`FINISH_WAIT`] : passé ce délai, ou
//!    le tube fermé sans « go », il renonce et ne touche à rien.
//! 3. Il met la nouvelle app en place **comme `tools/install-macos.sh`** : le dossier caché
//!    verrouillé (personne d'autre ne s'en sert), la nouvelle vérifiée (une Capri, sa signature
//!    valide), l'installée vérifiée (une Capri, pas un lien), mise de côté par un renommage dans
//!    son dossier, la nouvelle renommée à sa place, sa signature vérifiée, l'ancienne retirée ; au
//!    moindre échec, l'ancienne est remise en place. Jamais une demi-app, jamais par-dessus autre
//!    chose qu'une Capri, jamais à travers un lien, jamais `sudo`.
//! 4. Il la relance par `/usr/bin/open` (l'ancienne, remise en place, après un échec). Pas avec
//!    `CAPRI_CONFIG_DIR` : `open` ne lui passerait pas les lieux d'essai, et pourrait retrouver une
//!    autre Capri du même identifiant ; un essai ne relance rien.
//!
//! Son journal : le fichier de l'app (`journal::start_finisher`), ouvert seulement une fois
//! l'ancienne sortie. Il ne dit que les étapes et la conclusion.
//!
//! Et, en version de débogage, `capri update rehearse` : la même installation de bout en bout,
//! depuis un terminal, contre un faux serveur de cette machine seulement (`CAPRI_UPDATE_URL`),
//! avec un dossier d'essai (`CAPRI_CONFIG_DIR`) : le plan, la préparation, le relais, puis elle
//! sort, et le programme de fin finit.

/// Combien attendre que l'ancienne Capri soit sortie.
#[cfg(target_os = "macos")]
const FINISH_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// `capri update finish <app>`. Rend le code de sortie du processus.
pub fn run_finish(args: &[String]) -> i32 {
    #[cfg(target_os = "macos")]
    {
        mac::finish(args)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        eprintln!("capri update finish : elle ne sait pas encore s'installer seule sur ce système");
        2
    }
}

/// `capri update rehearse` (version de débogage, macOS).
#[cfg(debug_assertions)]
pub fn run_rehearse() -> i32 {
    #[cfg(target_os = "macos")]
    {
        mac::rehearse()
    }
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("capri update rehearse : macOS seulement");
        2
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};

    use capri_core::bundle::{self, HANDOVER_GO, HANDOVER_READY, Leftover, NEXT, PROGRAM};

    use super::FINISH_WAIT;
    use crate::update_install::{is_capri, open_marker};

    /// Où il est : le dossier de l'app installée, le dossier caché, la nouvelle app, l'installée.
    struct Place {
        folder: PathBuf,
        root: PathBuf,
        next: PathBuf,
        app: PathBuf,
        pid: u32,
    }

    impl Place {
        /// D'après son propre programme et l'app qu'on lui donne (voir le haut du module).
        fn find(app: &str) -> Result<Self, &'static str> {
            let exe = std::env::current_exe()
                .and_then(|exe| exe.canonicalize())
                .map_err(|_| "son programme est introuvable")?;
            let named = |path: Option<&Path>, name: &str| {
                path.and_then(Path::file_name).is_some_and(|found| found == name)
            };
            let macos = exe.parent();
            let contents = macos.and_then(Path::parent);
            let next = contents.and_then(Path::parent);
            let root = next.and_then(Path::parent);
            if !(named(Some(exe.as_path()), PROGRAM)
                && named(macos, "MacOS")
                && named(contents, "Contents")
                && named(next, NEXT))
            {
                return Err("il n'est pas le programme d'une nouvelle app");
            }
            let (next, root) =
                (next.expect("nommé").to_owned(), root.ok_or("pas de dossier caché")?.to_owned());
            let Some(Leftover::Staging(pid)) =
                root.file_name().and_then(|name| name.to_str()).and_then(bundle::leftover)
            else {
                return Err("son dossier n'est pas celui d'une mise à jour");
            };
            let folder = root.parent().ok_or("pas de dossier")?.to_owned();
            let given = Path::new(app);
            let file_name = given
                .file_name()
                .filter(|_| given.is_absolute())
                .ok_or("l'app donnée n'est pas un chemin absolu")?;
            if given.extension().is_none_or(|extension| extension != "app") {
                return Err("l'app donnée n'est pas une app");
            }
            let parent = given.parent().and_then(|parent| parent.canonicalize().ok());
            if parent.as_deref() != Some(folder.as_path()) {
                return Err("l'app donnée n'est pas dans le dossier de la mise à jour");
            }
            Ok(Self { app: folder.join(file_name), folder, root, next, pid })
        }
    }

    pub fn finish(args: &[String]) -> i32 {
        let [app] = args else {
            eprintln!("capri update finish : réservé à la fin d'une mise à jour, lancé par Capri elle-même");
            return 2;
        };
        let place = match Place::find(app) {
            Ok(place) => place,
            Err(why) => {
                eprintln!("capri update finish : {why} : rien n'est fait");
                return 2;
            }
        };
        // le relais : il dit qu'il le prend, puis attend la sortie de l'ancienne
        let said = std::io::stdout()
            .lock()
            .write_all(HANDOVER_READY.as_bytes())
            .and_then(|()| std::io::stdout().flush());
        if said.is_err() {
            return 2;
        }
        match wait_for_exit() {
            Waited::Exited => {}
            // le tube fermé sans « go » (elle a renoncé : elle tourne encore), ou elle n'est pas
            // sortie à temps : rien n'est fait, et rien n'est écrit dans son journal, qu'elle tient
            // encore ; c'est elle qui le dit
            Waited::NoGo | Waited::TimedOut => return 1,
        }
        // elle est sortie : il peut écrire dans son journal
        crate::journal::start_finisher();
        log::info!("mise à jour : l'ancienne Capri est sortie, mise en place");
        match put_in_place(&place) {
            Ok(()) => {
                log::info!("mise à jour : la nouvelle version est en place");
                relaunch(&place.app);
                0
            }
            Err(Failed { why, relaunch_installed }) => {
                log::warn!("mise à jour : pas mise en place ({why}) ; l'installation précédente reste");
                if relaunch_installed {
                    relaunch(&place.app);
                }
                1
            }
        }
    }

    enum Waited {
        Exited,
        NoGo,
        TimedOut,
    }

    /// Attend la fermeture de son entrée (l'ancienne Capri est sortie), au plus [`FINISH_WAIT`] ;
    /// elle a dû y écrire « go » d'abord.
    fn wait_for_exit() -> Waited {
        let (sender, received) = std::sync::mpsc::channel();
        let reading = std::thread::Builder::new().name("capri-attend".to_owned()).spawn(move || {
            let mut stdin = std::io::stdin().lock();
            let (mut head, mut buffer) = (Vec::new(), [0u8; 64]);
            loop {
                match stdin.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) if head.len() < 16 => head.extend_from_slice(&buffer[..read]),
                    Ok(_) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = sender.send(head);
        });
        if reading.is_err() {
            return Waited::TimedOut;
        }
        match received.recv_timeout(FINISH_WAIT) {
            Ok(head) if head == HANDOVER_GO.as_bytes() => Waited::Exited,
            Ok(_) => Waited::NoGo,
            Err(_) => Waited::TimedOut,
        }
    }

    struct Failed {
        why: &'static str,
        /// L'installée est là, et c'est une Capri : la relancer.
        relaunch_installed: bool,
    }

    fn failed(why: &'static str, relaunch_installed: bool) -> Failed {
        Failed { why, relaunch_installed }
    }

    /// Met la nouvelle app à la place de l'installée (voir le haut du module).
    fn put_in_place(place: &Place) -> Result<(), Failed> {
        let marker =
            open_marker(&place.root).ok_or_else(|| failed("le dossier caché n'a pas notre marque", true))?;
        marker.try_lock().map_err(|_| failed("le dossier caché est tenu par un autre", true))?;
        log::info!("mise à jour : dossier caché verrouillé");
        let discard = |why: &'static str, relaunch_installed: bool| {
            let _ = std::fs::remove_dir_all(&place.root);
            failed(why, relaunch_installed)
        };
        if !is_capri(&place.app) {
            // ni remplacée, ni relancée : ce n'est pas une Capri (ou c'est un lien)
            return Err(discard("l'app installée n'est pas une Capri, ou c'est un lien", false));
        }
        if !is_capri(&place.next) || capri_platform::codesign::verify(&place.next).is_err() {
            return Err(discard("la nouvelle app n'est pas une Capri signée", true));
        }
        let aside = place.folder.join(bundle::previous_name(place.pid));
        if std::fs::symlink_metadata(&aside).is_ok() {
            return Err(discard("une installation mise de côté est déjà là", true));
        }
        // mise de côté, dans le même dossier : un renommage, d'un seul coup
        std::fs::rename(&place.app, &aside)
            .map_err(|_| discard("l'installée n'a pas pu être mise de côté", true))?;
        log::info!("mise à jour : installation précédente mise de côté");
        let restore = |why: &'static str| {
            if std::fs::rename(&aside, &place.app).is_ok() {
                log::info!("mise à jour : installation précédente remise en place");
                let _ = std::fs::remove_dir_all(&place.root);
                failed(why, true)
            } else {
                // les deux restent dans le dossier caché et à côté : retirés ou remis à la main
                log::error!("mise à jour : installation précédente pas remise en place ({why})");
                failed(why, false)
            }
        };
        if std::fs::symlink_metadata(&place.app).is_ok() {
            // réapparue entre-temps : on ne met rien par-dessus
            log::error!("mise à jour : quelque chose a pris la place de l'app pendant le remplacement");
            return Err(failed("l'emplacement de l'app a été repris", false));
        }
        if std::fs::rename(&place.next, &place.app).is_err() {
            return Err(restore("la nouvelle n'a pas pu prendre sa place"));
        }
        if capri_platform::codesign::verify(&place.app).is_err() {
            let _ = std::fs::rename(&place.app, &place.next);
            return Err(restore("la signature de la nouvelle ne se vérifie plus à sa place"));
        }
        log::info!("mise à jour : nouvelle app à sa place, signature vérifiée");
        // l'ancienne retirée ; si elle ne l'est pas, le dossier caché reste avec elle, pour que
        // le prochain lancement les reconnaisse et les retire
        if std::fs::remove_dir_all(&aside).is_err() {
            log::warn!("mise à jour : installation précédente pas retirée : elle le sera au lancement");
        } else if std::fs::remove_dir_all(&place.root).is_err() {
            log::warn!("mise à jour : dossier caché pas retiré : il le sera au lancement");
        }
        drop(marker);
        // le Finder garde les icônes en mémoire : changer la date du dossier l'incite à relire la
        // sienne (comme le script d'installation ; le dossier de l'app n'est pas scellé)
        let _ =
            std::fs::File::open(&place.app).and_then(|app| app.set_modified(std::time::SystemTime::now()));
        Ok(())
    }

    /// Relance l'app par `/usr/bin/open`, sauf pendant un essai.
    fn relaunch(app: &Path) {
        if std::env::var_os(capri_claude::places::CAPRI_DIR_ENV).is_some_and(|value| !value.is_empty()) {
            log::info!(
                "mise à jour : relance sautée (dossier d'essai) : open ne lui passerait pas ses lieux d'essai"
            );
            return;
        }
        match capri_platform::launch::open_app(app) {
            Ok(()) => log::info!("mise à jour : relancée"),
            Err(err) => log::warn!("mise à jour : pas relancée : {err}"),
        }
    }

    /// `capri update rehearse` (version de débogage) : voir le haut du module.
    #[cfg(debug_assertions)]
    pub fn rehearse() -> i32 {
        use std::sync::atomic::AtomicBool;
        use std::time::Instant;

        use capri_chat::update::install::{Here, Installation, Order};
        use capri_chat::update::{Manifest, Source};

        use crate::update_install::{self, HANDOVER_WAIT, System, complete_relay, prepare, start_relay};

        crate::journal::start_without_file();
        let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
        let source = crate::update_setup::source(|name| std::env::var_os(name));
        let source = match source {
            Ok(source @ Source::ThisMachine { .. }) if set(capri_claude::places::CAPRI_DIR_ENV) => source,
            _ => {
                println!(
                    "La répétition demande un faux serveur de cette machine ({}) et un dossier d'essai ({}).",
                    crate::update_setup::TEST_URL_ENV,
                    capri_claude::places::CAPRI_DIR_ENV
                );
                return 2;
            }
        };
        let Some(app) = crate::login_setup::this_app().map(PathBuf::from) else {
            println!("{}", capri_chat::update::install::Refusal::NotInApp.say());
            return 1;
        };
        let removed = update_install::clear_leftovers(&app);
        println!("Restes retirés : {removed}");
        let installed = crate::update_setup::installed();
        let fetched =
            capri_net::update::fetch(&source.route(), &source.url(), capri_net::update::Limits::default());
        let manifest = match fetched
            .map_err(|failure| failure.explain(&source))
            .and_then(|body| Manifest::parse(&body).map_err(|why| format!("manifeste refusé : {why}")))
        {
            Ok(manifest) => manifest,
            Err(why) => {
                println!("Manifeste : {why}");
                return 1;
            }
        };
        let verdict = capri_chat::update::verdict(&manifest, &installed);
        let here = Here { supported: capri_platform::codesign::AVAILABLE, in_app: true, installed };
        let mut installation = Installation::new();
        let reply = installation.chosen(Some((&verdict, &manifest)), &here);
        println!("Bulle : {}", reply.say.as_deref().unwrap_or("(rien)"));
        let Order::Prepare(plan) = reply.order else { return 1 };
        let mut shop = System { route: source.route() };
        let prepared = prepare(&plan, &source, &app, &mut shop, &AtomicBool::new(false), &mut |step| {
            println!("Étape : {step:?}");
            if let Some(said) = installation.progressed(step) {
                println!("Bulle : {said}");
            }
        });
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(refusal) => {
                println!("Refus : {} ({})", refusal.say(), refusal.for_log());
                return 1;
            }
        };
        println!("Bulle : {}", installation.prepared(prepared.signed).unwrap_or_default());
        println!("Menu : {}", installation.menu_line(Some(&verdict), true).unwrap_or_default());
        let reply = installation.chosen(Some((&verdict, &manifest)), &here);
        println!("Bulle : {}", reply.say.as_deref().unwrap_or("(rien)"));
        let relay = match start_relay(&prepared.staging, &app) {
            Ok(relay) => relay,
            Err(refusal) => {
                println!("Refus : {}", refusal.say());
                return 1;
            }
        };
        let deadline = Instant::now() + HANDOVER_WAIT;
        while relay.ready().is_none() && Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if relay.ready() != Some(true) {
            println!("Refus : {}", capri_chat::update::install::Refusal::Handover.say());
            return 1;
        }
        if let Err(refusal) = complete_relay(relay, prepared.staging) {
            println!("Refus : {}", refusal.say());
            return 1;
        }
        println!("Relais pris : elle sort, le programme de fin finit.");
        0
    }
}
