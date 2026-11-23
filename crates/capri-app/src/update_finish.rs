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
//!    chose qu'une Capri, jamais à travers un lien, jamais `sudo`. Si la place est prise quand il
//!    faut la remettre (la nouvelle, dont la signature ne se vérifie plus à sa place, n'a pas pu en
//!    être retirée), rien n'est mis par-dessus : l'ancienne reste de côté avec le dossier caché,
//!    rien n'est relancé, et le journal dit qu'elle est à remettre à la main ; au lancement suivant,
//!    elle n'est retirée que si l'app en place se vérifie (`update_install::clear_leftovers`).
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
/// Combien attendre, ensuite, que le verrou du dossier caché soit libre : l'ancienne le tient
/// jusqu'à sa sortie, et le système peut fermer le tube un instant avant de lâcher le verrou.
#[cfg(target_os = "macos")]
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

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

    use super::{FINISH_WAIT, LOCK_WAIT};
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
        match put_in_place(&place, &mut RealGround) {
            Ok(()) => {
                log::info!("mise à jour : la nouvelle version est en place");
                relaunch(&place.app);
                0
            }
            Err(Failed { why, left: Left::Installed }) => {
                log::warn!("mise à jour : pas mise en place ({why}) ; l'installation précédente reste");
                relaunch(&place.app);
                1
            }
            Err(Failed { why, left: Left::Untouched }) => {
                log::warn!(
                    "mise à jour : pas mise en place ({why}) ; ce qui est à la place de l'app n'est ni \
                     touché ni relancé"
                );
                1
            }
            Err(Failed { why, left: Left::Aside }) => {
                log::error!(
                    "mise à jour : pas mise en place ({why}) ; l'installation précédente est restée de \
                     côté, à remettre à la main : {}",
                    bundle::previous_name(place.pid)
                );
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

    #[derive(Debug)]
    struct Failed {
        why: &'static str,
        /// Ce qu'il y a à la place de l'app après l'échec.
        left: Left,
    }

    /// Ce qu'il y a à la place de l'app après un échec de la mise en place.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Left {
        /// L'installation précédente, une Capri : elle est relancée.
        Installed,
        /// Ce qui y était, qui n'est pas une Capri (ou c'est un lien) : ni touché, ni relancé.
        Untouched,
        /// Pas l'installation précédente : elle est restée de côté, et rien n'est relancé.
        Aside,
    }

    fn failed(why: &'static str, left: Left) -> Failed {
        Failed { why, left }
    }

    /// Ce que la mise en place demande au système : de quoi le remplacer dans les essais.
    trait Ground {
        /// La signature de `app` se vérifie-t-elle, strictement ?
        fn verify(&mut self, app: &Path) -> bool;
        /// Renomme `from` en `to`, d'un seul coup.
        fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()>;
    }

    /// Le vrai système : `codesign`, et le renommage du système de fichiers.
    struct RealGround;

    impl Ground for RealGround {
        fn verify(&mut self, app: &Path) -> bool {
            capri_platform::codesign::verify(app).is_ok()
        }

        fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
            std::fs::rename(from, to)
        }
    }

    /// Met la nouvelle app à la place de l'installée (voir le haut du module).
    fn put_in_place(place: &Place, ground: &mut impl Ground) -> Result<(), Failed> {
        let marker = open_marker(&place.root)
            .ok_or_else(|| failed("le dossier caché n'a pas notre marque", Left::Installed))?;
        if !lock_within(&marker, LOCK_WAIT) {
            return Err(failed("le dossier caché est tenu par un autre", Left::Installed));
        }
        log::info!("mise à jour : dossier caché verrouillé");
        let discard = |why: &'static str, left: Left| {
            let _ = std::fs::remove_dir_all(&place.root);
            failed(why, left)
        };
        if !is_capri(&place.app) {
            // ni remplacée, ni relancée : ce n'est pas une Capri (ou c'est un lien)
            return Err(discard("l'app installée n'est pas une Capri, ou c'est un lien", Left::Untouched));
        }
        if !is_capri(&place.next) || !ground.verify(&place.next) {
            return Err(discard("la nouvelle app n'est pas une Capri signée", Left::Installed));
        }
        let aside = place.folder.join(bundle::previous_name(place.pid));
        if std::fs::symlink_metadata(&aside).is_ok() {
            return Err(discard("une installation mise de côté est déjà là", Left::Installed));
        }
        // mise de côté, dans le même dossier : un renommage, d'un seul coup
        ground
            .rename(&place.app, &aside)
            .map_err(|_| discard("l'installée n'a pas pu être mise de côté", Left::Installed))?;
        log::info!("mise à jour : installation précédente mise de côté");
        if std::fs::symlink_metadata(&place.app).is_ok() {
            // réapparue entre-temps : on ne met rien par-dessus
            log::error!("mise à jour : quelque chose a pris la place de l'app pendant le remplacement");
            return Err(failed("l'emplacement de l'app a été repris", Left::Aside));
        }
        if ground.rename(&place.next, &place.app).is_err() {
            return Err(restore(place, &aside, ground, "la nouvelle n'a pas pu prendre sa place"));
        }
        if !ground.verify(&place.app) {
            let why = "la signature de la nouvelle ne se vérifie plus à sa place";
            if ground.rename(&place.app, &place.next).is_err() {
                log::error!(
                    "mise à jour : la nouvelle app, dont la signature ne se vérifie plus, n'a pas pu \
                     être retirée de sa place"
                );
            }
            return Err(restore(place, &aside, ground, why));
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

    /// Remet à sa place l'installation précédente, mise de côté en `aside`, après l'échec `why`.
    /// La place prise (la nouvelle n'a pas pu en être retirée, ou autre chose l'a prise), rien
    /// n'est mis par-dessus : l'ancienne reste de côté, et le dossier caché avec elle, pour que le
    /// prochain lancement les reconnaisse (il ne les retire que si l'app en place se vérifie).
    fn restore(place: &Place, aside: &Path, ground: &mut impl Ground, why: &'static str) -> Failed {
        if std::fs::symlink_metadata(&place.app).is_ok() {
            log::error!(
                "mise à jour : installation précédente pas remise en place, sa place est prise ({why})"
            );
            return failed(why, Left::Aside);
        }
        if ground.rename(aside, &place.app).is_ok() {
            log::info!("mise à jour : installation précédente remise en place");
            let _ = std::fs::remove_dir_all(&place.root);
            failed(why, Left::Installed)
        } else {
            // les deux restent dans le dossier caché et à côté : retirés ou remis à la main
            log::error!("mise à jour : installation précédente pas remise en place ({why})");
            failed(why, Left::Aside)
        }
    }

    /// Verrouille la marque du dossier caché, en réessayant au plus `wait` : l'ancienne Capri
    /// tient ce verrou jusqu'à sa sortie, et le tube qui annonce sa sortie peut se fermer un
    /// instant avant que le système ne lâche le verrou. Sans cela, une mise à jour prête serait
    /// abandonnée pour quelques millisecondes.
    fn lock_within(marker: &std::fs::File, wait: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + wait;
        loop {
            if marker.try_lock().is_ok() {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
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

    #[cfg(test)]
    mod tests {
        use std::path::{Path, PathBuf};
        use std::time::{Duration, Instant};

        use capri_core::bundle::{self, BUNDLE_ID, MARKER, MARKER_TEXT, NEXT};

        use super::{Failed, Ground, Left, Place, lock_within, put_in_place};
        use crate::update_install::clear_leftovers_with;

        /// Un dossier d'essai, retiré à la fin.
        struct Scratch(PathBuf);

        impl Scratch {
            fn new(name: &str) -> Self {
                let directory = std::env::temp_dir()
                    .join(format!("capri-test-mise-en-place-{name}-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&directory);
                std::fs::create_dir_all(&directory).expect("le dossier d'essai");
                Self(directory.canonicalize().expect("son vrai chemin"))
            }
        }

        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        fn info(version: &str) -> String {
            include_str!("../../../tools/macos/Info.plist").replace("@VERSION@", version)
        }

        /// Une app d'essai : un faux programme, et cet `Info.plist`.
        fn make_app(app: &Path, info: &str) {
            std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
            std::fs::write(app.join("Contents/MacOS/capri"), "programme").unwrap();
            std::fs::write(app.join("Contents/Info.plist"), info).unwrap();
        }

        /// La version d'une app, d'après son `Info.plist` : de quoi distinguer l'ancienne de la
        /// nouvelle.
        fn version(app: &Path) -> Option<String> {
            let text = std::fs::read_to_string(app.join("Contents/Info.plist")).ok()?;
            bundle::info_string(&text, "CFBundleShortVersionString").ok().flatten()
        }

        /// Une mise à jour prête à être mise en place : l'installée en 0.1.0, la nouvelle en 0.2.0
        /// dans le dossier caché à côté, avec sa marque.
        fn ready(scratch: &Scratch) -> Place {
            let pid = std::process::id();
            let folder = scratch.0.clone();
            let root = folder.join(bundle::staging_name(pid));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join(MARKER), MARKER_TEXT).unwrap();
            let place = Place { next: root.join(NEXT), app: folder.join("Capri.app"), root, folder, pid };
            make_app(&place.app, &info("0.1.0"));
            make_app(&place.next, &info("0.2.0"));
            place
        }

        /// L'installation mise de côté.
        fn aside(place: &Place) -> PathBuf {
            place.folder.join(bundle::previous_name(place.pid))
        }

        /// Ce qui est dans le dossier de l'app, à part elle.
        fn beside(place: &Place) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&place.folder)
                .unwrap()
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name != "Capri.app")
                .collect();
            names.sort();
            names
        }

        /// Un faux système : la vérification décidée par l'essai (la nouvelle dans le dossier
        /// caché, puis l'app à sa place), les renommages faits pour de bon, sauf celui qu'il refuse.
        struct Fake {
            next: PathBuf,
            next_verifies: bool,
            in_place_verifies: bool,
            refused: Option<(PathBuf, PathBuf)>,
            verified: Vec<PathBuf>,
            renamed: Vec<(PathBuf, PathBuf)>,
        }

        impl Ground for Fake {
            fn verify(&mut self, app: &Path) -> bool {
                self.verified.push(app.to_owned());
                if app == self.next { self.next_verifies } else { self.in_place_verifies }
            }

            fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
                if self
                    .refused
                    .as_ref()
                    .is_some_and(|(refused_from, refused_to)| refused_from == from && refused_to == to)
                {
                    return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
                }
                std::fs::rename(from, to)?;
                self.renamed.push((from.to_owned(), to.to_owned()));
                Ok(())
            }
        }

        fn fake(place: &Place) -> Fake {
            Fake {
                next: place.next.clone(),
                next_verifies: true,
                in_place_verifies: true,
                refused: None,
                verified: Vec::new(),
                renamed: Vec::new(),
            }
        }

        fn left(done: Result<(), Failed>) -> Option<Left> {
            done.err().map(|failed| failed.left)
        }

        #[test]
        fn the_new_app_takes_its_place_and_nothing_stays_beside() {
            let scratch = Scratch::new("en-place");
            let place = ready(&scratch);
            let mut ground = fake(&place);
            assert_eq!(left(put_in_place(&place, &mut ground)), None);
            assert_eq!(version(&place.app).as_deref(), Some("0.2.0"));
            assert!(beside(&place).is_empty(), "{:?}", beside(&place));
            // la nouvelle vérifiée à côté, puis à sa place
            assert_eq!(ground.verified, [place.next.clone(), place.app.clone()]);
        }

        #[test]
        fn a_signature_that_no_longer_verifies_in_place_brings_the_old_app_back() {
            let scratch = Scratch::new("pas-verifiee");
            let place = ready(&scratch);
            let mut ground = Fake { in_place_verifies: false, ..fake(&place) };
            assert_eq!(left(put_in_place(&place, &mut ground)), Some(Left::Installed));
            assert_eq!(version(&place.app).as_deref(), Some("0.1.0"));
            assert!(beside(&place).is_empty(), "{:?}", beside(&place));
        }

        /// Le cas de TES-76 : la nouvelle, dont la signature ne se vérifie plus à sa place, ne peut
        /// pas en être retirée. Rien n'est mis par-dessus : l'ancienne reste de côté, avec le
        /// dossier caché ; au lancement suivant, elle n'est retirée que si l'app en place se
        /// vérifie.
        #[test]
        fn a_new_app_that_cannot_leave_its_place_leaves_the_old_one_aside() {
            let scratch = Scratch::new("place-prise");
            let place = ready(&scratch);
            let mut ground = Fake {
                in_place_verifies: false,
                refused: Some((place.app.clone(), place.next.clone())),
                ..fake(&place)
            };
            assert_eq!(left(put_in_place(&place, &mut ground)), Some(Left::Aside));
            assert_eq!(version(&place.app).as_deref(), Some("0.2.0"), "la nouvelle à la place");
            assert_eq!(version(&aside(&place)).as_deref(), Some("0.1.0"), "l'ancienne de côté");
            assert_eq!(std::fs::read_to_string(place.root.join(MARKER)).unwrap(), MARKER_TEXT);
            let mut expected = vec![bundle::staging_name(place.pid), bundle::previous_name(place.pid)];
            expected.sort();
            assert_eq!(beside(&place), expected);
            // l'app en place ne se vérifie pas : rien n'est retiré
            assert_eq!(clear_leftovers_with(&place.app, |_| false), 0);
            assert_eq!(beside(&place), expected);
            assert_eq!(version(&aside(&place)).as_deref(), Some("0.1.0"));
            // elle se vérifie : l'installation mise de côté et le dossier caché sont retirés
            assert_eq!(clear_leftovers_with(&place.app, |_| true), 2);
            assert!(beside(&place).is_empty(), "{:?}", beside(&place));
            assert_eq!(version(&place.app).as_deref(), Some("0.2.0"));
        }

        #[test]
        fn a_new_app_refused_its_place_brings_the_old_app_back() {
            let scratch = Scratch::new("refusee");
            let place = ready(&scratch);
            let mut ground = Fake { refused: Some((place.next.clone(), place.app.clone())), ..fake(&place) };
            assert_eq!(left(put_in_place(&place, &mut ground)), Some(Left::Installed));
            assert_eq!(version(&place.app).as_deref(), Some("0.1.0"));
            assert!(beside(&place).is_empty(), "{:?}", beside(&place));
        }

        #[test]
        fn an_installed_app_that_is_not_a_capri_is_left_untouched() {
            let scratch = Scratch::new("autre-app");
            let place = ready(&scratch);
            let other = info("0.1.0").replace(BUNDLE_ID, "com.example.autre");
            std::fs::write(place.app.join("Contents/Info.plist"), &other).unwrap();
            let mut ground = fake(&place);
            assert_eq!(left(put_in_place(&place, &mut ground)), Some(Left::Untouched));
            assert_eq!(std::fs::read_to_string(place.app.join("Contents/Info.plist")).unwrap(), other);
            assert_eq!(std::fs::read_to_string(place.app.join("Contents/MacOS/capri")).unwrap(), "programme");
            assert!(ground.renamed.is_empty(), "{:?}", ground.renamed);
            assert!(ground.verified.is_empty(), "{:?}", ground.verified);
            // la nouvelle, avec son dossier caché, est retirée
            assert!(beside(&place).is_empty(), "{:?}", beside(&place));
        }

        /// Le verrou tenu par un autre (ici, une autre ouverture du même fichier, comme celle de
        /// l'ancienne Capri) : attendu, pris dès qu'il est lâché ; jamais lâché, on renonce au
        /// bout du délai, pas avant.
        #[test]
        fn the_lock_is_waited_for_then_given_up() {
            let directory = std::env::temp_dir().join(format!("capri-test-verrou-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(&directory).expect("le dossier d'essai");
            let path = directory.join("capri-update");
            std::fs::write(&path, "marque\n").expect("la marque");
            let held = std::fs::File::open(&path).expect("ouverte");
            held.try_lock().expect("tenue");
            let ours = std::fs::File::open(&path).expect("ouverte de nouveau");
            let started = Instant::now();
            assert!(!lock_within(&ours, Duration::from_millis(150)), "tenue par l'autre : on renonce");
            assert!(started.elapsed() >= Duration::from_millis(150), "pas avant le délai");
            let release = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(120));
                drop(held);
            });
            assert!(lock_within(&ours, Duration::from_secs(5)), "lâchée : on la prend");
            release.join().expect("le thread qui la lâche");
            let _ = std::fs::remove_dir_all(&directory);
        }
    }
}
