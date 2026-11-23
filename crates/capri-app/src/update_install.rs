//! Installer une version, sur un geste (macOS) : la préparation, sur un thread à elle
//! (« capri-installe »), et ce qu'une mise à jour laisse à côté de l'app installée.
//!
//! **La préparation** ([`prepare`]) : à côté de l'app installée, dans son propre dossier, un
//! dossier caché à nous ([`Staging`] : `.Capri.app.capri-update-<processus>`, en `0700`, sa marque
//! tenue verrouillée tant qu'on s'en sert), où elle refait l'app :
//! 1. les trois fichiers de la version téléchargés (`capri_net::update::download` : taille exacte,
//!    empreinte vérifiée), chacun dans un fichier qu'elle crée, à sa place ;
//! 2. l'`Info.plist` reçu vérifié (notre identifiant de bundle, la version du manifeste, notre
//!    programme) ;
//! 3. la signature de l'app installée vérifiée, puis son `Contents/Resources` recopié tel quel
//!    (fichiers et dossiers seulement : un lien, ou autre chose, et rien n'est installé) ; jamais
//!    son `_CodeSignature` ;
//! 4. signée comme l'app installée (`capri_core::codesign::choose`), le second programme d'abord,
//!    puis l'app, et sa signature vérifiée.
//!
//! Au moindre refus, ou si on l'arrête, le dossier caché est retiré : il ne reste rien. Prête, la
//! nouvelle app attend le geste qui redémarre Capri ; le programme de fin ([`crate::update_finish`])
//! la met en place.
//!
//! **Les restes** d'une mise à jour interrompue ([`clear_leftovers`]), au lancement : un dossier
//! caché retiré seulement s'il est à nous sans doute possible (un vrai dossier, notre nom, notre
//! marque au texte exact, et son verrou libre : personne ne s'en sert) ; sinon laissé, avec une
//! ligne au journal.
//!
//! Le journal ne dit que les étapes et la conclusion : jamais une adresse signée (`capri-net` n'en
//! écrit que l'hôte), jamais une identité (`capri_core::codesign` les masque).

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use capri_chat::update::install::{
    MAX_INFO_BYTES, Plan, Refusal, SignProblem, Signed, Slot, Step, check_info,
};
use capri_chat::update::manifest::Entry;
use capri_chat::update::{Failure, Route, RouteError, Source};
use capri_core::bundle::{self, BUNDLE_ID, Leftover, MARKER, MARKER_TEXT, NEXT};
use capri_core::codesign::{self, Choice, IdentityHash, IdentityName, Signature, Signer};

/// Combien attendre, en quittant, que la préparation s'arrête d'elle-même.
pub const STOP_WAIT: Duration = Duration::from_millis(1500);
/// Combien attendre que le programme de fin dise qu'il prend le relais.
pub const HANDOVER_WAIT: Duration = Duration::from_secs(5);
/// La profondeur des ressources recopiées, au plus.
const MAX_DEPTH: usize = 16;

/// Le dossier caché d'une mise à jour, à côté de l'app installée : à nous, sa marque verrouillée
/// tant qu'on s'en sert. Lâché sans avoir été passé au programme de fin, il est retiré.
pub struct Staging {
    root: PathBuf,
    /// La marque, ouverte et verrouillée.
    lock: Option<File>,
    handed: bool,
}

impl Staging {
    /// Le crée à côté de l'app `app`, pour ce processus : un dossier en `0700`, et sa marque.
    fn create(app: &Path) -> Result<Self, Refusal> {
        let folder = app.parent().ok_or(Refusal::NotInApp)?;
        let root = folder.join(bundle::staging_name(std::process::id()));
        std::fs::DirBuilder::new().mode(0o700).create(&root).map_err(|err| match err.kind() {
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem => {
                Refusal::NotWritable
            }
            _ => {
                log::warn!("mise à jour : dossier caché pas créé : {err}");
                Refusal::Disk
            }
        })?;
        // dès ici, il est à nous : lâché, il sera retiré
        let mut staging = Self { root, lock: None, handed: false };
        let mut marker = create_new(&staging.root.join(MARKER), 0o600).map_err(|_| Refusal::Disk)?;
        marker.write_all(MARKER_TEXT.as_bytes()).map_err(|_| Refusal::Disk)?;
        marker.try_lock().map_err(|_| Refusal::Disk)?;
        staging.lock = Some(marker);
        Ok(staging)
    }

    /// La nouvelle app, dans le dossier.
    pub fn next(&self) -> PathBuf {
        self.root.join(NEXT)
    }

    /// Le programme de fin : le `capri` de la nouvelle app.
    pub fn finisher(&self) -> PathBuf {
        slot_path(&self.next(), Slot::Program)
    }

    /// Passé au programme de fin : le dossier n'est plus retiré, et son verrou reste tenu jusqu'à
    /// la sortie de ce processus (le programme de fin le prend alors).
    pub fn hand_over(mut self) {
        self.handed = true;
        if let Some(lock) = self.lock.take() {
            std::mem::forget(lock);
        }
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.handed {
            return;
        }
        match std::fs::remove_dir_all(&self.root) {
            Ok(()) => log::info!("mise à jour : dossier caché retiré"),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                log::warn!("mise à jour : dossier caché pas retiré ({err}) : il le sera au lancement")
            }
        }
    }
}

/// Un fichier neuf (jamais par-dessus quoi que ce soit, lien compris), avec ce mode.
fn create_new(path: &Path, mode: u32) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).mode(mode).open(path)
}

/// Un dossier neuf, avec ce mode (son parent doit exister : rien n'est recréé derrière un dossier
/// retiré pendant qu'on travaille).
fn create_dir(path: &Path, mode: u32) -> Result<(), Refusal> {
    std::fs::DirBuilder::new().mode(mode).create(path).map_err(|_| Refusal::Disk)
}

/// La place d'un fichier dans l'app `app`.
pub fn slot_path(app: &Path, slot: Slot) -> PathBuf {
    slot.components().iter().fold(app.to_owned(), |path, part| path.join(part))
}

/// Ce que la préparation demande au système : de quoi le remplacer dans les essais.
pub trait Workshop {
    /// Télécharge `url` dans `file`, créé à `path` (voir `capri_net::update::download`).
    fn download(
        &mut self,
        url: &str,
        entry: &Entry,
        file: File,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), Failure>;
    /// La signature de `app` se vérifie-t-elle, strictement ?
    fn verify(&mut self, app: &Path) -> bool;
    /// Comment `app` est signée.
    fn signature(&mut self, app: &Path) -> Option<Signature>;
    /// Les identités de signature valides du trousseau.
    fn identities(&mut self) -> Option<Vec<(IdentityHash, IdentityName)>>;
    /// Signe `path`.
    fn sign(&mut self, path: &Path, signer: &Signer) -> bool;
}

/// Le vrai système : le réseau par la règle de la source, `codesign` et `security`.
pub struct System {
    pub route: Route,
}

impl Workshop for System {
    fn download(
        &mut self,
        url: &str,
        entry: &Entry,
        file: File,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), Failure> {
        let expected = capri_net::update::Expected { size: entry.size, sha256: &entry.sha256 };
        let limits = capri_net::update::DownloadLimits::default();
        capri_net::update::download(&self.route, url, expected, file, path, limits, cancel)
    }

    fn verify(&mut self, app: &Path) -> bool {
        capri_platform::codesign::verify(app).inspect_err(|err| log::warn!("mise à jour : {err}")).is_ok()
    }

    fn signature(&mut self, app: &Path) -> Option<Signature> {
        capri_platform::codesign::describe(app).inspect_err(|err| log::warn!("mise à jour : {err}")).ok()
    }

    fn identities(&mut self) -> Option<Vec<(IdentityHash, IdentityName)>> {
        capri_platform::codesign::identities().inspect_err(|err| log::warn!("mise à jour : {err}")).ok()
    }

    fn sign(&mut self, path: &Path, signer: &Signer) -> bool {
        capri_platform::codesign::sign(path, signer)
            .inspect_err(|err| log::warn!("mise à jour : {err}"))
            .is_ok()
    }
}

/// La nouvelle app, prête, à côté de l'installée.
pub struct Prepared {
    pub staging: Staging,
    pub signed: Signed,
}

/// Refait l'app installée `app` à la version de `plan`, à côté d'elle (voir le haut du module).
/// `report` reçoit chaque étape. Bloque le temps du travail : hors du thread principal.
pub fn prepare(
    plan: &Plan,
    source: &Source,
    app: &Path,
    shop: &mut impl Workshop,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Step),
) -> Result<Prepared, Refusal> {
    let staging = Staging::create(app)?;
    log::info!("mise à jour : la {} se prépare à côté de l'app installée", plan.version);
    let next = staging.next();
    for directory in [next.clone(), next.join("Contents"), next.join("Contents").join("MacOS")] {
        create_dir(&directory, 0o755)?;
    }
    let of = plan.files.len();
    for (index, planned) in plan.files.iter().enumerate() {
        stop_if(cancel)?;
        report(Step::Download { file: index + 1, of });
        // calculée, jamais lue dans le manifeste
        let url = source
            .asset_url(plan.version, &planned.entry.asset)
            .ok_or(Refusal::Download(Failure::NotAllowed(RouteError::Malformed)))?;
        let path = slot_path(&next, planned.slot);
        let file = create_new(&path, 0o600).map_err(|_| Refusal::Disk)?;
        shop.download(&url, &planned.entry, file, &path, cancel).map_err(|failure| match failure {
            Failure::Cancelled => Refusal::Cancelled,
            other => Refusal::Download(other),
        })?;
        log::info!(
            "mise à jour : {} reçu ({} sur {of}, {} octets, empreinte vérifiée)",
            planned.entry.name,
            index + 1,
            planned.entry.size
        );
    }
    stop_if(cancel)?;
    report(Step::Assemble);
    let info = read_bounded(&slot_path(&next, Slot::Info), MAX_INFO_BYTES).ok_or(Refusal::Disk)?;
    check_info(&info, plan.version).map_err(Refusal::Info)?;
    if !shop.verify(app) {
        return Err(Refusal::Installed);
    }
    copy_tree(&app.join("Contents").join("Resources"), &next.join("Contents").join("Resources"), cancel, 0)?;
    for planned in &plan.files {
        let mode = if planned.slot.is_program() { 0o755 } else { 0o644 };
        std::fs::set_permissions(slot_path(&next, planned.slot), std::fs::Permissions::from_mode(mode))
            .map_err(|_| Refusal::Disk)?;
    }
    log::info!(
        "mise à jour : app refaite à côté (Info.plist vérifié, ressources de l'app installée recopiées)"
    );
    stop_if(cancel)?;
    report(Step::Sign);
    let signature = shop.signature(app).ok_or(Refusal::Sign(SignProblem::Unknown))?;
    let choice = codesign::choose(&signature, || shop.identities().ok_or(()))
        .map_err(|()| Refusal::Sign(SignProblem::Keychain))?;
    let (signer, signed) = match &choice {
        Choice::AdHoc => (Signer::AdHoc, Signed::AdHoc),
        Choice::Identity(hash) => (Signer::Identity(hash.clone()), Signed::Identity),
        Choice::Missing => (Signer::AdHoc, Signed::AdHocFallback),
        Choice::Several(count) => return Err(Refusal::Sign(SignProblem::Ambiguous(*count))),
    };
    if !shop.sign(&slot_path(&next, Slot::HookProgram), &signer) || !shop.sign(&next, &signer) {
        return Err(Refusal::Sign(SignProblem::Failed));
    }
    if !shop.verify(&next) {
        return Err(Refusal::Sign(SignProblem::Invalid));
    }
    log::info!("mise à jour : la {} est prête, {}", plan.version, signed.for_log());
    Ok(Prepared { staging, signed })
}

fn stop_if(cancel: &AtomicBool) -> Result<(), Refusal> {
    if cancel.load(Ordering::Relaxed) { Err(Refusal::Cancelled) } else { Ok(()) }
}

/// Le contenu d'un fichier ordinaire (jamais à travers un lien), s'il fait au plus `limit` octets.
pub fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    File::open(path).ok()?.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

/// Recopie le dossier `from` (de l'app installée) en `to`, tel quel : ses dossiers et ses fichiers
/// ordinaires, rien d'autre (un lien, ou autre chose : l'app installée n'est pas la nôtre telle
/// qu'on la connaît, et rien n'est installé). Les fichiers sont recopiés octet par octet, sans
/// leurs attributs étendus (que la signature refuserait).
fn copy_tree(from: &Path, to: &Path, cancel: &AtomicBool, depth: usize) -> Result<(), Refusal> {
    let meta = std::fs::symlink_metadata(from).map_err(|_| Refusal::Installed)?;
    if !meta.is_dir() || depth > MAX_DEPTH {
        return Err(Refusal::Installed);
    }
    create_dir(to, 0o755)?;
    let entries = std::fs::read_dir(from).map_err(|_| Refusal::Installed)?;
    for entry in entries {
        stop_if(cancel)?;
        let entry = entry.map_err(|_| Refusal::Installed)?;
        let kind = entry.file_type().map_err(|_| Refusal::Installed)?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target, cancel, depth + 1)?;
        } else if kind.is_file() {
            let mut source = File::open(entry.path()).map_err(|_| Refusal::Installed)?;
            let mut copy = create_new(&target, 0o644).map_err(|_| Refusal::Disk)?;
            std::io::copy(&mut source, &mut copy).map_err(|_| Refusal::Disk)?;
        } else {
            return Err(Refusal::Installed);
        }
    }
    Ok(())
}

/// Ce que la préparation rapporte à la boucle.
pub enum Report {
    Step(Step),
    Done(Result<Prepared, Refusal>),
}

/// La préparation en cours, sur son thread.
pub struct Preparation {
    reports: Receiver<Report>,
    cancel: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Le dossier caché qu'elle crée : retiré à l'arrêt si elle ne l'a pas fait elle-même à temps.
    root: PathBuf,
}

impl Preparation {
    /// Lance la préparation de `plan`, pour l'app `app`, par la source `source`.
    pub fn start(plan: Plan, source: Source, app: PathBuf) -> Result<Self, Refusal> {
        let root = app.parent().ok_or(Refusal::NotInApp)?.join(bundle::staging_name(std::process::id()));
        let (sender, reports) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        let job = move || {
            let mut shop = System { route: source.route() };
            let steps = sender.clone();
            let done = prepare(&plan, &source, &app, &mut shop, &stop, &mut |step| {
                let _ = steps.send(Report::Step(step));
            });
            if let Err(refusal) = &done {
                log::warn!("mise à jour : rien n'est installé ({})", refusal.for_log());
            }
            // plus personne n'écoute : ce qui est prêt est lâché, donc retiré
            let _ = sender.send(Report::Done(done));
        };
        let thread =
            std::thread::Builder::new().name("capri-installe".to_owned()).spawn(job).map_err(|err| {
                log::warn!("mise à jour : pas de thread pour la préparer : {err}");
                Refusal::Disk
            })?;
        Ok(Self { reports, cancel, thread: Some(thread), root })
    }

    /// Ce qu'elle a rapporté depuis l'appel précédent.
    pub fn heard(&self) -> Vec<Report> {
        self.reports.try_iter().collect()
    }

    /// L'arrête : elle s'arrête d'elle-même au prochain morceau lu ou à la prochaine étape, et
    /// retire son dossier ; on l'attend au plus `wait`. Passé ce délai, le dossier est retiré
    /// d'ici, s'il est à nous. Ce qu'elle aurait fini entre-temps est lâché, donc retiré.
    pub fn stop(mut self, wait: Duration) {
        self.cancel.store(true, Ordering::Relaxed);
        let deadline = Instant::now() + wait;
        while self.thread.as_ref().is_some_and(|thread| !thread.is_finished()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        match self.thread.take() {
            Some(thread) if thread.is_finished() => {
                let _ = thread.join();
                drop(self.heard());
            }
            _ => {
                log::warn!(
                    "mise à jour : la préparation ne s'est pas arrêtée à temps, son dossier est retiré d'ici"
                );
                if marker_is_ours(&self.root) {
                    let _ = std::fs::remove_dir_all(&self.root);
                }
            }
        }
        log::info!("mise à jour : installation arrêtée");
    }
}

/// La marque du dossier caché `root` est-elle la nôtre : un vrai dossier, une marque ordinaire,
/// au texte exact ?
pub fn marker_is_ours(root: &Path) -> bool {
    open_marker(root).is_some()
}

/// La marque du dossier caché `root`, ouverte, si elle est la nôtre (voir [`marker_is_ours`]).
pub fn open_marker(root: &Path) -> Option<File> {
    let meta = std::fs::symlink_metadata(root).ok()?;
    if !meta.is_dir() {
        return None;
    }
    let marker = root.join(MARKER);
    let text = read_bounded(&marker, 1024)?;
    if text != MARKER_TEXT.as_bytes() {
        return None;
    }
    File::open(&marker).ok()
}

/// L'app `app` est-elle une Capri : un vrai dossier (pas un lien), dont `Contents` n'est pas un
/// lien, et dont l'`Info.plist` porte notre identifiant de bundle ?
pub fn is_capri(app: &Path) -> bool {
    let real_dir = |path: &Path| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir());
    if !real_dir(app) || !real_dir(&app.join("Contents")) {
        return false;
    }
    let Some(info) = read_bounded(&slot_path(app, Slot::Info), MAX_INFO_BYTES) else { return false };
    let text = String::from_utf8_lossy(&info);
    bundle::info_string(&text, "CFBundleIdentifier").is_ok_and(|found| found.as_deref() == Some(BUNDLE_ID))
}

/// Au lancement, à côté de l'app `app` : les restes d'une mise à jour interrompue, retirés s'ils
/// sont à nous sans doute possible (voir le haut du module), laissés sinon. Rend combien ont été
/// retirés.
pub fn clear_leftovers(app: &Path) -> usize {
    let Some(folder) = app.parent() else { return 0 };
    let Ok(entries) = std::fs::read_dir(folder) else { return 0 };
    let mut names: Vec<String> =
        entries.flatten().filter_map(|entry| entry.file_name().into_string().ok()).collect();
    names.sort();
    let mut removed = 0;
    for name in &names {
        match bundle::leftover(name) {
            None => {}
            Some(Leftover::Staging(pid)) => {
                let root = folder.join(name);
                let Some(marker) = open_marker(&root) else {
                    log::warn!("mise à jour : reste laissé à côté de l'app, sans notre marque : {name}");
                    continue;
                };
                if marker.try_lock().is_err() {
                    log::info!("mise à jour : {name} est encore utilisé, laissé");
                    continue;
                }
                // l'installation mise de côté par un programme de fin interrompu, d'abord
                let previous = folder.join(bundle::previous_name(pid));
                if std::fs::symlink_metadata(&previous).is_ok() {
                    if !is_capri(&previous) || std::fs::remove_dir_all(&previous).is_err() {
                        log::warn!(
                            "mise à jour : installation mise de côté laissée : {}",
                            bundle::previous_name(pid)
                        );
                        continue;
                    }
                    removed += 1;
                }
                drop(marker);
                match std::fs::remove_dir_all(&root) {
                    Ok(()) => removed += 1,
                    Err(err) => log::warn!("mise à jour : reste pas retiré ({name}) : {err}"),
                }
            }
            // avec son dossier : vue avec lui, plus haut
            Some(Leftover::Previous(pid)) if !names.contains(&bundle::staging_name(pid)) => {
                log::warn!("mise à jour : installation mise de côté laissée, sans son dossier : {name}");
            }
            Some(Leftover::Previous(_)) => {}
        }
    }
    if removed > 0 {
        log::info!("mise à jour : {removed} reste(s) d'une mise à jour interrompue retiré(s)");
    }
    removed
}

/// Le geste qui redémarre : lance le programme de fin de la nouvelle app, pour l'app installée
/// `app`. Il dira s'il prend le relais ([`capri_platform::launch::Relay::ready`]).
pub fn start_relay(staging: &Staging, app: &Path) -> Result<capri_platform::launch::Relay, Refusal> {
    use std::ffi::OsStr;
    let args = [OsStr::new("update"), OsStr::new("finish"), app.as_os_str()];
    capri_platform::launch::hand_over(&staging.finisher(), &args).map_err(|err| {
        log::warn!("mise à jour : programme de fin pas lancé : {err}");
        Refusal::Handover
    })
}

/// Il a pris le relais : « vas-y », le tube gardé ouvert jusqu'à la sortie de Capri, et le dossier
/// caché lui est laissé. Ensuite, Capri se quitte.
pub fn complete_relay(relay: capri_platform::launch::Relay, staging: Staging) -> Result<(), Refusal> {
    let gone = relay.go().map_err(|err| {
        log::warn!("mise à jour : relais pas donné : {err}");
        Refusal::Handover
    })?;
    gone.keep_until_exit();
    staging.hand_over();
    log::info!("mise à jour : le programme de fin a pris le relais, elle se quitte");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use capri_chat::update::install::plan;
    use capri_chat::update::manifest::Manifest;
    use capri_chat::update::{Installed, Version, digest};

    use super::*;

    /// Un dossier d'essai, retiré à la fin.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory =
                std::env::temp_dir().join(format!("capri-test-installe-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(&directory).expect("le dossier d'essai");
            Self(directory.canonicalize().expect("son vrai chemin"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Une app installée d'essai : de faux programmes, notre Info.plist, des ressources.
    fn installed_app(folder: &Path) -> PathBuf {
        let app = folder.join("Capri.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(app.join("Contents/Resources/assets/capri/perch")).unwrap();
        std::fs::create_dir_all(app.join("Contents/_CodeSignature")).unwrap();
        std::fs::write(app.join("Contents/MacOS/capri"), "ancien").unwrap();
        std::fs::write(app.join("Contents/MacOS/capri-hook"), "ancien").unwrap();
        std::fs::write(app.join("Contents/Info.plist"), info("0.1.0")).unwrap();
        std::fs::write(app.join("Contents/_CodeSignature/CodeResources"), "ancienne signature").unwrap();
        std::fs::write(app.join("Contents/Resources/CREDITS.md"), "crédits\n").unwrap();
        std::fs::write(
            app.join("Contents/Resources/assets/capri/perch/perch.png"),
            [137u8, 80, 78, 71, 0, 1, 2],
        )
        .unwrap();
        app
    }

    fn info(version: &str) -> String {
        include_str!("../../../tools/macos/Info.plist").replace("@VERSION@", version)
    }

    /// Les trois fichiers d'une version, et son manifeste.
    fn release(version: &str) -> (HashMap<&'static str, Vec<u8>>, Manifest) {
        let files: HashMap<&'static str, Vec<u8>> = [
            ("capri-macos-arm64", b"nouveau programme".to_vec()),
            ("capri-hook-macos-arm64", b"nouveau hook".to_vec()),
            ("Info.plist", info(version).into_bytes()),
        ]
        .into();
        let entry = |name: &str, asset: &str| {
            let bytes = &files[asset];
            format!(
                r#"{{"name": "{name}", "asset": "{asset}", "size": {}, "sha256": "{}"}}"#,
                bytes.len(),
                digest::hex(&digest::sha256(bytes))
            )
        };
        let text = format!(
            r#"{{"format": 1, "version": "{version}", "bundle": 1, "targets": {{"macos-arm64": {{"files": [{}, {}, {}]}}}}}}"#,
            entry("capri", "capri-macos-arm64"),
            entry("capri-hook", "capri-hook-macos-arm64"),
            entry("Info.plist", "Info.plist")
        );
        (files.clone(), Manifest::parse(text.as_bytes()).expect("lisible"))
    }

    const HERE: Installed =
        Installed { version: Version::new(0, 1, 0), bundle: 1, target: Some("macos-arm64") };

    /// Un faux système : les fichiers servis d'une table, une fausse signature qui laisse sa trace.
    #[derive(Default)]
    struct Fake {
        files: HashMap<&'static str, Vec<u8>>,
        installed_signature: Option<Signature>,
        identities: Vec<(IdentityHash, IdentityName)>,
        installed_valid: bool,
        sign_fails: bool,
        downloaded: Vec<String>,
        signed: Vec<(PathBuf, Signer)>,
        identities_read: bool,
        /// Arrête la préparation après ce nombre de fichiers.
        cancel_after: Option<usize>,
    }

    impl Workshop for Fake {
        fn download(
            &mut self,
            url: &str,
            entry: &Entry,
            mut file: File,
            path: &Path,
            cancel: &AtomicBool,
        ) -> Result<(), Failure> {
            self.downloaded.push(url.to_owned());
            let bytes = self.files.get(entry.asset.as_str()).cloned().unwrap_or_default();
            if bytes.len() as u64 != entry.size {
                let _ = std::fs::remove_file(path);
                return Err(Failure::WrongSize);
            }
            file.write_all(&bytes).unwrap();
            if self.cancel_after == Some(self.downloaded.len()) {
                cancel.store(true, Ordering::Relaxed);
            }
            Ok(())
        }
        fn verify(&mut self, app: &Path) -> bool {
            if app.ends_with(NEXT) { self.signed.len() == 2 } else { self.installed_valid }
        }
        fn signature(&mut self, _app: &Path) -> Option<Signature> {
            self.installed_signature.clone()
        }
        fn identities(&mut self) -> Option<Vec<(IdentityHash, IdentityName)>> {
            self.identities_read = true;
            Some(self.identities.clone())
        }
        fn sign(&mut self, path: &Path, signer: &Signer) -> bool {
            self.signed.push((path.to_owned(), signer.clone()));
            !self.sign_fails
        }
    }

    fn fake(version: &str) -> (Fake, Plan) {
        let (files, manifest) = release(version);
        let plan = plan(&manifest, &HERE).expect("installable");
        (
            Fake {
                files,
                installed_signature: Some(Signature::AdHoc),
                installed_valid: true,
                ..Fake::default()
            },
            plan,
        )
    }

    fn source() -> Source {
        capri_chat::update::source(
            true,
            Some("http://127.0.0.1:8123/Thai-v4/capri-daemon/releases/latest/download/latest.json"),
        )
        .expect("une source d'essai")
    }

    /// Ce qui reste dans le dossier, à part l'app installée.
    fn others(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "Capri.app")
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_app_is_remade_beside_the_installed_one_with_its_resources_and_signed_alike() {
        let scratch = Scratch::new("refaite");
        let app = installed_app(&scratch.0);
        let (mut shop, plan) = fake("0.2.0");
        let mut steps = Vec::new();
        let prepared =
            prepare(&plan, &source(), &app, &mut shop, &AtomicBool::new(false), &mut |step| steps.push(step))
                .expect("prête");
        assert_eq!(
            steps,
            [
                Step::Download { file: 1, of: 3 },
                Step::Download { file: 2, of: 3 },
                Step::Download { file: 3, of: 3 },
                Step::Assemble,
                Step::Sign
            ]
        );
        assert_eq!(prepared.signed, Signed::AdHoc);
        // les adresses sont calculées, sous le serveur d'essai, à la forme de GitHub
        assert_eq!(
            shop.downloaded[0],
            "http://127.0.0.1:8123/Thai-v4/capri-daemon/releases/download/v0.2.0/capri-macos-arm64"
        );
        // un dossier caché à côté, qui ne finit pas par .app, en 0700, avec sa marque
        let root = scratch.0.join(bundle::staging_name(std::process::id()));
        assert_eq!(others(&scratch.0), [bundle::staging_name(std::process::id())]);
        assert_eq!(std::fs::metadata(&root).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::read_to_string(root.join(MARKER)).unwrap(), MARKER_TEXT);
        let next = prepared.staging.next();
        assert_eq!(next, root.join(NEXT));
        // les fichiers reçus à leur place, les programmes exécutables
        assert_eq!(std::fs::read(next.join("Contents/MacOS/capri")).unwrap(), b"nouveau programme");
        assert_eq!(std::fs::read(next.join("Contents/MacOS/capri-hook")).unwrap(), b"nouveau hook");
        assert_eq!(std::fs::read_to_string(next.join("Contents/Info.plist")).unwrap(), info("0.2.0"));
        let mode = |path: &str| std::fs::metadata(next.join(path)).unwrap().permissions().mode() & 0o777;
        assert_eq!((mode("Contents/MacOS/capri"), mode("Contents/MacOS/capri-hook")), (0o755, 0o755));
        assert_eq!(mode("Contents/Info.plist"), 0o644);
        // les ressources de l'installée, telles quelles ; jamais son ancienne signature
        assert_eq!(std::fs::read_to_string(next.join("Contents/Resources/CREDITS.md")).unwrap(), "crédits\n");
        assert_eq!(
            std::fs::read(next.join("Contents/Resources/assets/capri/perch/perch.png")).unwrap(),
            [137u8, 80, 78, 71, 0, 1, 2]
        );
        assert!(!next.join("Contents/_CodeSignature").exists());
        // signée « ad hoc » comme l'installée : le second programme, puis l'app ; le trousseau pas lu
        assert_eq!(
            shop.signed,
            [(next.join("Contents/MacOS/capri-hook"), Signer::AdHoc), (next.clone(), Signer::AdHoc)]
        );
        assert!(!shop.identities_read);
        assert_eq!(prepared.staging.finisher(), next.join("Contents/MacOS/capri"));
        // l'app installée n'a pas bougé
        assert_eq!(std::fs::read_to_string(app.join("Contents/MacOS/capri")).unwrap(), "ancien");
        // lâchée sans être passée au programme de fin : il ne reste rien
        drop(prepared);
        assert!(others(&scratch.0).is_empty(), "{:?}", others(&scratch.0));
    }

    #[test]
    fn the_identity_of_the_installed_app_is_looked_for_and_never_chosen_among_several() {
        let line = |hash: char, name: &str| format!("  1) {} \"{name}\"\n", hash.to_string().repeat(40));
        let name = "Developer ID Application: Personne Inventée (ABCDE12345)";
        let installed = codesign::signature(&format!("Authority={name}\n")).expect("signée");
        let cases = [
            (line('A', name), Some(Signed::Identity)),
            (
                line('B', "Developer ID Application: Quelqu'un d'autre (QQQQQ11111)"),
                Some(Signed::AdHocFallback),
            ),
            (format!("{}{}", line('A', name), line('C', name)), None),
        ];
        for (listed, expected) in cases {
            let scratch = Scratch::new("identite");
            let app = installed_app(&scratch.0);
            let (mut shop, plan) = fake("0.2.0");
            shop.installed_signature = Some(installed.clone());
            shop.identities = codesign::identities(&listed);
            let done = prepare(&plan, &source(), &app, &mut shop, &AtomicBool::new(false), &mut |_| {});
            assert!(shop.identities_read);
            match expected {
                Some(signed) => {
                    let prepared = done.expect("prête");
                    assert_eq!(prepared.signed, signed);
                    let signer = &shop.signed[0].1;
                    match signed {
                        Signed::Identity => {
                            assert!(
                                matches!(signer, Signer::Identity(hash) if hash.for_codesign() == "A".repeat(40))
                            )
                        }
                        _ => assert_eq!(*signer, Signer::AdHoc),
                    }
                }
                None => {
                    assert!(matches!(done, Err(Refusal::Sign(SignProblem::Ambiguous(2)))));
                    assert!(shop.signed.is_empty(), "rien n'est signé");
                    assert!(others(&scratch.0).is_empty(), "rien ne reste");
                }
            }
        }
    }

    #[test]
    fn every_refusal_leaves_nothing_beside_the_app() {
        type Spoil = fn(&mut Fake, &Path);
        let cases: Vec<(&str, Spoil, Refusal)> = vec![
            (
                "abîmé",
                |shop, _| shop.files.get_mut("capri-macos-arm64").unwrap().push(0),
                Refusal::Download(Failure::WrongSize),
            ),
            (
                "autre plist",
                |shop, _| {
                    let other = info("0.2.0").replace(BUNDLE_ID, "com.example.autre");
                    shop.files.insert("Info.plist", other.into_bytes());
                },
                Refusal::Download(Failure::WrongSize),
            ),
            ("installée invalide", |shop, _| shop.installed_valid = false, Refusal::Installed),
            (
                "signature inconnue",
                |shop, _| shop.installed_signature = None,
                Refusal::Sign(SignProblem::Unknown),
            ),
            ("signature refusée", |shop, _| shop.sign_fails = true, Refusal::Sign(SignProblem::Failed)),
            (
                "un lien dans les ressources",
                |_, app| {
                    std::os::unix::fs::symlink("/etc/hosts", app.join("Contents/Resources/lien")).unwrap()
                },
                Refusal::Installed,
            ),
            (
                "sans ressources",
                |_, app| std::fs::remove_dir_all(app.join("Contents/Resources")).unwrap(),
                Refusal::Installed,
            ),
            ("arrêtée", |shop, _| shop.cancel_after = Some(2), Refusal::Cancelled),
        ];
        for (name, spoil, refusal) in cases {
            let scratch = Scratch::new("refus");
            let app = installed_app(&scratch.0);
            let (mut shop, plan) = fake("0.2.0");
            spoil(&mut shop, &app);
            let done = prepare(&plan, &source(), &app, &mut shop, &AtomicBool::new(false), &mut |_| {});
            assert_eq!(done.err(), Some(refusal), "{name}");
            assert!(others(&scratch.0).is_empty(), "{name} : {:?}", others(&scratch.0));
            assert_eq!(
                std::fs::read_to_string(app.join("Contents/Info.plist")).unwrap(),
                info("0.1.0"),
                "{name}"
            );
        }
        // un Info.plist d'une autre version, de la bonne taille et de la bonne empreinte : refusé
        let scratch = Scratch::new("version");
        let app = installed_app(&scratch.0);
        let (files, manifest) = release("0.2.0");
        let mut shop = Fake {
            files,
            installed_signature: Some(Signature::AdHoc),
            installed_valid: true,
            ..Fake::default()
        };
        let mut plan = plan(&manifest, &HERE).expect("installable");
        plan.version = Version::new(0, 3, 0);
        let done = prepare(&plan, &source(), &app, &mut shop, &AtomicBool::new(false), &mut |_| {});
        assert!(
            matches!(done, Err(Refusal::Info(capri_chat::update::install::InfoProblem::Version))),
            "{:?}",
            done.err()
        );
        assert!(others(&scratch.0).is_empty());
    }

    #[test]
    fn a_folder_she_cannot_write_in_is_said() {
        let scratch = Scratch::new("lecture-seule");
        let app = installed_app(&scratch.0);
        std::fs::set_permissions(&scratch.0, std::fs::Permissions::from_mode(0o555)).unwrap();
        let (mut shop, plan) = fake("0.2.0");
        let done = prepare(&plan, &source(), &app, &mut shop, &AtomicBool::new(false), &mut |_| {});
        assert_eq!(done.err(), Some(Refusal::NotWritable));
        assert!(shop.downloaded.is_empty(), "rien n'est téléchargé");
    }

    #[test]
    fn leftovers_are_removed_only_when_ours_beyond_doubt() {
        let scratch = Scratch::new("restes");
        let app = installed_app(&scratch.0);
        let folder = &scratch.0;
        let make = |pid: u32, marker: Option<&str>| {
            let root = folder.join(bundle::staging_name(pid));
            std::fs::create_dir_all(root.join(NEXT)).unwrap();
            if let Some(text) = marker {
                std::fs::write(root.join(MARKER), text).unwrap();
            }
            root
        };
        // à nous, abandonné : retiré, avec l'installation mise de côté qui l'accompagne
        make(11, Some(MARKER_TEXT));
        let previous = folder.join(bundle::previous_name(11));
        std::fs::create_dir_all(previous.join("Contents")).unwrap();
        std::fs::write(previous.join("Contents/Info.plist"), info("0.1.0")).unwrap();
        // sans marque, ou une autre marque : laissés
        make(12, None);
        make(13, Some("autre chose\n"));
        // à nous, mais encore utilisé (son verrou tenu) : laissé
        let used = make(14, Some(MARKER_TEXT));
        let held = File::open(used.join(MARKER)).unwrap();
        held.try_lock().unwrap();
        // une mise de côté qui n'est pas une Capri, avec son dossier à nous : laissée, et le dossier aussi
        make(15, Some(MARKER_TEXT));
        std::fs::create_dir_all(folder.join(bundle::previous_name(15))).unwrap();
        // un lien au nom d'un des nôtres : laissé
        std::os::unix::fs::symlink(
            folder.join(bundle::staging_name(11)),
            folder.join(bundle::staging_name(16)),
        )
        .unwrap();
        // d'autres noms : jamais regardés
        std::fs::create_dir_all(folder.join(".Capri.app.capri-install-17")).unwrap();
        std::fs::create_dir_all(folder.join("Autre.app")).unwrap();
        assert_eq!(clear_leftovers(&app), 2);
        let mut expected = vec![
            ".Capri.app.capri-install-17".to_owned(),
            bundle::staging_name(12),
            bundle::staging_name(13),
            bundle::staging_name(14),
            bundle::staging_name(15),
            bundle::previous_name(15),
            bundle::staging_name(16),
            "Autre.app".to_owned(),
        ];
        expected.sort();
        assert_eq!(others(folder), expected);
        drop(held);
        // son verrou lâché, le dossier encore utilisé est retiré au lancement suivant
        assert_eq!(clear_leftovers(&app), 1);
        assert!(!folder.join(bundle::staging_name(14)).exists());
    }

    /// La préparation sur son thread, par le vrai réseau contre un faux serveur de cette machine,
    /// arrêtée en route : elle s'arrête, et il ne reste rien.
    #[test]
    fn stopping_the_preparation_leaves_nothing() {
        let scratch = Scratch::new("arret");
        let app = installed_app(&scratch.0);
        let (_, manifest) = release("0.2.0");
        let plan = plan(&manifest, &HERE).expect("installable");
        // un serveur qui envoie l'en-tête du premier fichier et son début, puis se tait, la
        // connexion ouverte
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("un port libre");
        let address = listener.local_addr().expect("son adresse");
        std::thread::spawn(move || {
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 17\r\nConnection: close\r\n\r\nnouveau");
                std::thread::sleep(Duration::from_secs(10));
            }
        });
        let url = format!("http://{address}/Thai-v4/capri-daemon/releases/latest/download/latest.json");
        let source = capri_chat::update::source(true, Some(&url)).expect("une source d'essai");
        let preparation = Preparation::start(plan, source, app.clone()).expect("lancée");
        let root = scratch.0.join(bundle::staging_name(std::process::id()));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !root.join(NEXT).join("Contents/MacOS/capri").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(root.exists(), "le téléchargement a commencé");
        let started = Instant::now();
        preparation.stop(STOP_WAIT);
        assert!(started.elapsed() < STOP_WAIT + Duration::from_secs(1));
        assert!(others(&scratch.0).is_empty(), "{:?}", others(&scratch.0));
    }
}
