//! Son journal : sur la sortie d'erreur, comme toujours, et, quand elle tourne comme app à
//! fenêtre, dans un fichier aussi (sa place : `capri_claude::places::log_file`), que « Montrer le
//! journal » de son menu montre dans le Finder ([`show`]). Ses commandes de terminal
//! (`capri hook …`, `capri chat …`…) n'y écrivent pas : elles n'appellent pas [`start`].
//!
//! Le même filtre pour les deux (`RUST_LOG`, par défaut `info`), le même format (celui
//! d'`env_logger` : la date et l'heure en temps universel, le niveau, l'origine, le message ; sans
//! couleurs dans le fichier), et le même refus : jamais les traces `trace` de ureq ([`NoWireLog`]).
//! La sortie d'erreur ne change pas.
//!
//! Le fichier ([`LogFile`]) :
//! - **borné** : deux fichiers au plus, `capri.log` et [`PREVIOUS_NAME`], chacun d'au plus
//!   `capri_core::journal::MAX_BYTES` octets ; la décision est dans `capri_core::journal`, et elle
//!   vaut au lancement comme en cours de route ;
//! - **réservé à l'utilisateur** : le dossier est créé en `0700`, le fichier en `0600` (resserré
//!   s'il existait déjà), sous Unix. Jamais écrit à travers un lien symbolique, ni par-dessus autre
//!   chose qu'un fichier ordinaire : un fichier neuf est créé par `create_new`, qui échoue s'il y a
//!   déjà quelque chose à sa place, lien compris ; un fichier existant est rouvert sans être créé,
//!   puis comparé (appareil et numéro de fichier, sous Unix) à ce que désigne son chemin, avant
//!   chaque ligne ;
//! - **jamais une panne d'elle** : dossier impossible à créer, disque plein, fichier retiré ou
//!   remplacé pendant qu'elle tourne, rotation impossible : il cesse d'écrire, le dit une fois sur
//!   la sortie d'erreur, et elle continue avec la seule sortie d'erreur. Il ne panique jamais (le
//!   verrou qu'`env_logger` tient autour de lui serait empoisonné, et chaque ligne suivante, de
//!   n'importe quel thread, paniquerait) ;
//! - **chaque ligne écrite tout de suite**, d'un seul `write`, sans tampon : un plantage ne perd
//!   pas les dernières lignes, que le système a déjà. Pas de `fsync` : une coupure de courant
//!   pourrait perdre les toutes dernières.
//!
//! Rien n'en part sur le réseau, et elle ne le relit jamais.

use std::fs::{File, Metadata, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use capri_claude::places::{self, System};
use capri_core::journal::{self, MAX_BYTES};

/// Le fichier précédent, à côté du courant : il garde l'extension `.log` des journaux.
pub const PREVIOUS_NAME: &str = "capri.old.log";

/// Lance le journal de l'app à fenêtre : la sortie d'erreur, et le fichier. Rend la place du
/// fichier, qu'il ait pu s'ouvrir ou non ([`show`] regarde s'il est là), ou `None` si elle est
/// inconnue.
pub fn start() -> Option<PathBuf> {
    let place = places::log_file(|name| std::env::var_os(name), System::CURRENT);
    let opened = place
        .as_ref()
        .map_err(|missing| anyhow!("sa place est inconnue ({missing})"))
        .and_then(|path| LogFile::open(path, MAX_BYTES));
    match opened {
        Ok(file) => {
            init(Some(file));
            if let Ok(path) = &place {
                log::info!("Capri {} lancée ; journal : {}", env!("CARGO_PKG_VERSION"), path.display());
            }
        }
        Err(err) => {
            init(None);
            log::warn!("pas de journal dans un fichier, seulement sur la sortie d'erreur : {err:#}");
        }
    }
    place.ok()
}

/// Ce qu'elle dit quand elle n'a aucun fichier à montrer : sa place est inconnue, ou il n'a pas pu
/// s'écrire (il n'est pas là, ou ce n'est pas un fichier ordinaire).
#[cfg(any(target_os = "macos", windows, test))]
pub const NOTHING_TO_SHOW: &str = "Je n'ai pas de journal à te montrer : il n'a pas pu s'écrire";
/// … quand ce système ne sait pas montrer un fichier (la ligne de son menu est alors grisée).
#[cfg(any(target_os = "macos", windows, test))]
pub const CANNOT_SHOW_HERE: &str = "Je ne sais pas encore te montrer mon journal sur ce système";
/// … quand le montrer a échoué.
#[cfg(any(target_os = "macos", windows, test))]
pub const NOT_SHOWN: &str = "Je n'ai pas pu te montrer mon journal";

/// « Montrer le journal » : montre `file` (sa place, si elle est connue) par `reveal`, s'il est là
/// et ordinaire (jamais un lien). Rend ce qu'elle doit dire dans son bandeau, ou `None` si le
/// fichier est montré.
#[cfg(any(target_os = "macos", windows, test))]
pub fn show(
    file: Option<&Path>,
    reveal: impl FnOnce(&Path) -> Result<(), capri_platform::launch::LaunchError>,
) -> Option<&'static str> {
    use capri_platform::launch::LaunchError;
    let there =
        file.filter(|file| std::fs::symlink_metadata(file).is_ok_and(|meta| meta.file_type().is_file()));
    match there.map(reveal) {
        None => Some(NOTHING_TO_SHOW),
        Some(Ok(())) => None,
        Some(Err(LaunchError::Unavailable)) => Some(CANNOT_SHOW_HERE),
        Some(Err(err)) => {
            log::warn!("journal : {err}");
            Some(NOT_SHOWN)
        }
    }
}

/// Installe le journal : la sortie d'erreur, réglée par `RUST_LOG` (par défaut : `info`), et
/// `file` s'il a pu s'ouvrir, réglé de même.
fn init(file: Option<LogFile>) {
    let builder = || env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    let logger = NoWireLog::new(builder().build(), file.map(|file| to_file(builder(), file)));
    log::set_max_level(logger.filter());
    if let Err(err) = log::set_boxed_logger(Box::new(logger)) {
        let _ = writeln!(std::io::stderr(), "capri : pas de journal ({err})");
    }
}

/// Le journal réglé par `builder` (son filtre), écrit dans `file`, sans couleurs même si
/// `RUST_LOG_STYLE` en demande.
fn to_file(mut builder: env_logger::Builder, file: LogFile) -> env_logger::Logger {
    builder
        .target(env_logger::Target::Pipe(Box::new(file)))
        .write_style(env_logger::WriteStyle::Never)
        .build()
}

/// Le journal de l'app, privé de ce qui passe sur le réseau.
///
/// Au niveau `trace`, ureq écrit les octets échangés tels quels (« NOT REDACTED », dit sa
/// documentation) : la conversation, et l'en-tête qui porte la clé d'un fournisseur. Ces lignes ne
/// sont jamais écrites, ni sur la sortie d'erreur ni dans le fichier, quel que soit `RUST_LOG`. Ce
/// que ureq écrit au niveau `debug` (adresse, état de la connexion, en-têtes d'une liste sûre)
/// reste visible.
struct NoWireLog {
    /// La sortie d'erreur, comme avant.
    stderr: env_logger::Logger,
    /// Le fichier, quand elle tourne comme app à fenêtre et qu'il a pu s'ouvrir.
    file: Option<env_logger::Logger>,
}

impl NoWireLog {
    fn new(stderr: env_logger::Logger, file: Option<env_logger::Logger>) -> Self {
        Self { stderr, file }
    }

    fn allows(metadata: &log::Metadata<'_>) -> bool {
        let from_ureq = metadata.target() == "ureq"
            || metadata.target().starts_with("ureq::")
            || metadata.target().starts_with("ureq_proto");
        !(from_ureq && metadata.level() == log::Level::Trace)
    }

    /// Le niveau le plus bavard des deux (c'est le même filtre).
    fn filter(&self) -> log::LevelFilter {
        let stderr = self.stderr.filter();
        self.file.as_ref().map_or(stderr, |file| file.filter().max(stderr))
    }
}

impl log::Log for NoWireLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        Self::allows(metadata)
            && (self.stderr.enabled(metadata)
                || self.file.as_ref().is_some_and(|file| file.enabled(metadata)))
    }

    fn log(&self, record: &log::Record<'_>) {
        if Self::allows(record.metadata()) {
            self.stderr.log(record);
            if let Some(file) = &self.file {
                file.log(record);
            }
        }
    }

    fn flush(&self) {
        self.stderr.flush();
        if let Some(file) = &self.file {
            file.flush();
        }
    }
}

/// Le fichier du journal : borné, réservé à l'utilisateur, et qui ne gêne jamais (voir le haut de
/// ce module). `env_logger` lui donne chaque ligne entière, d'un seul `write`, sous son verrou :
/// une ligne à la fois, quel que soit le thread.
struct LogFile {
    path: PathBuf,
    /// Le fichier précédent, à côté.
    previous: PathBuf,
    /// La taille maximale de chacun des deux.
    max: u64,
    /// `None` : il a cessé d'écrire, et l'a dit.
    open: Option<File>,
    /// Où dire qu'il cesse d'écrire : la sortie d'erreur (jamais `log`, dont on tient le verrou).
    complaints: Box<dyn Write + Send>,
}

impl LogFile {
    /// Ouvre le journal `path` (son dossier créé s'il manque), après l'avoir fait tourner s'il
    /// dépasse déjà `max` octets. Refuse un lien, ou autre chose qu'un fichier ordinaire.
    fn open(path: &Path, max: u64) -> Result<Self> {
        let directory = path
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
            .with_context(|| format!("{} n'a pas de dossier", path.display()))?;
        private_dir(directory)?;
        let mut log = Self {
            path: path.to_owned(),
            previous: path.with_file_name(PREVIOUS_NAME),
            max,
            open: None,
            complaints: Box::new(std::io::stderr()),
        };
        match look(path)? {
            None => log.open = Some(create(path)?),
            Some(meta) if !meta.file_type().is_file() => {
                bail!("{} n'est pas un fichier ordinaire : elle n'y écrit pas", path.display())
            }
            Some(meta) if journal::must_rotate(meta.len(), 0, max) => log.rotate()?,
            Some(_) => {
                // sans le créer : s'il a été remplacé entre-temps par un lien, la comparaison qui
                // suit le refuse avant toute écriture
                let file = OpenOptions::new()
                    .append(true)
                    .open(path)
                    .with_context(|| format!("ouverture de {}", path.display()))?;
                same_file(path, &file)?;
                restrict(&file)?;
                log.open = Some(file);
            }
        }
        Ok(log)
    }

    /// Écrit une ligne, en passant d'abord au fichier suivant s'il le faut.
    fn append(&mut self, line: &[u8]) -> Result<()> {
        let Some(file) = &self.open else { return Ok(()) };
        let len = same_file(&self.path, file)?;
        let line = journal::clip(line, self.max);
        if journal::must_rotate(len, line.len() as u64, self.max) {
            self.rotate()?;
        }
        let file = self.open.as_mut().context("journal fermé")?;
        file.write_all(&line).with_context(|| format!("écriture dans {}", self.path.display()))
    }

    /// Le courant devient le précédent (l'ancien précédent disparaît), et un nouveau commence.
    fn rotate(&mut self) -> Result<()> {
        // fermé avant d'être renommé : Windows refuse parfois de renommer un fichier ouvert
        self.open = None;
        if let Some(meta) = look(&self.previous)?
            && !meta.file_type().is_file()
        {
            bail!("{} n'est pas un fichier ordinaire : elle ne le remplace pas", self.previous.display());
        }
        std::fs::rename(&self.path, &self.previous).with_context(|| {
            format!("renommage de {} en {}", self.path.display(), self.previous.display())
        })?;
        self.open = Some(create(&self.path)?);
        Ok(())
    }
}

impl Write for LogFile {
    /// Ne rend jamais d'erreur : une panne le fait taire, une fois dite, et la sortie d'erreur
    /// continue.
    fn write(&mut self, line: &[u8]) -> std::io::Result<usize> {
        if self.open.is_some()
            && let Err(err) = self.append(line)
        {
            self.open = None;
            let _ = writeln!(
                self.complaints,
                "capri : le journal ne s'écrit plus dans {} ({err:#}) : seulement sur la sortie d'erreur",
                self.path.display()
            );
        }
        Ok(line.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Ce qu'il y a à la place de `path`, sans suivre un lien ; `None` s'il n'y a rien.
fn look(path: &Path) -> Result<Option<Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("lecture de {}", path.display())),
    }
}

/// Crée le dossier s'il manque (en `0700` sous Unix), et refuse qu'il soit un lien ou autre chose
/// qu'un dossier. Un dossier qui existait déjà garde ses droits : c'est le fichier qui est
/// réservé à l'utilisateur.
fn private_dir(directory: &Path) -> Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory).with_context(|| format!("création de {}", directory.display()))?;
    match look(directory)? {
        Some(meta) if meta.file_type().is_dir() => Ok(()),
        _ => bail!("{} n'est pas un dossier (un lien ?) : elle n'y écrit pas", directory.display()),
    }
}

/// Crée le fichier, réservé à l'utilisateur sous Unix. `create_new` échoue s'il y a déjà quelque
/// chose à cette place, lien compris (`O_CREAT | O_EXCL` ne suit pas un lien, `CREATE_NEW` sous
/// Windows) : jamais un fichier créé ailleurs à travers un lien.
fn create(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.append(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path).with_context(|| format!("création de {}", path.display()))
}

/// Le chemin désigne-t-il toujours ce fichier-ci, ordinaire, et pas un lien ? Rend sa taille. Sous
/// Windows, seulement qu'il est là et ordinaire : son numéro de fichier n'y est pas lisible sans
/// une API instable.
fn same_file(path: &Path, file: &File) -> Result<u64> {
    let there =
        std::fs::symlink_metadata(path).with_context(|| format!("{} n'est plus là", path.display()))?;
    let held = file.metadata().with_context(|| format!("lecture de {}", path.display()))?;
    if !there.file_type().is_file() {
        bail!("{} a été remplacé par autre chose qu'un fichier ordinaire", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (there.dev(), there.ino()) != (held.dev(), held.ino()) {
            bail!("{} a été remplacé par un autre fichier", path.display());
        }
    }
    Ok(held.len())
}

/// Lisible et modifiable par l'utilisateur seul, sous Unix (sur le fichier ouvert : aucun lien
/// suivi). Ailleurs, ce sont les droits du dossier de l'utilisateur qui le protègent.
fn restrict(file: &File) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600)).context("droits du journal")?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use log::Log;

    use super::*;

    /// Ce que la sortie d'erreur a reçu.
    #[derive(Clone, Default)]
    struct Written(Arc<Mutex<Vec<u8>>>);

    impl Written {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().expect("verrou").clone()).expect("du texte")
        }
    }

    impl Write for Written {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("verrou").extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Un dossier d'essai, retiré avec tout ce qu'il contient quand l'essai le lâche, même en cas
    /// d'échec (ses dossiers rendus inscriptibles d'abord, s'il en a fermé un).
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = format!(
                "capri-test-journal-{}-{}-{name}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            );
            let directory = std::env::temp_dir().join(unique);
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }

        /// Le fichier du journal, dans un sous-dossier qui n'existe pas encore.
        fn journal(&self) -> PathBuf {
            self.0.join("logs").join("capri.log")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            #[cfg(unix)]
            reopen_all(&self.0);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Rend inscriptibles tous les dossiers sous `directory`, sans suivre de lien.
    #[cfg(unix)]
    fn reopen_all(directory: &Path) {
        use std::os::unix::fs::PermissionsExt;
        if look(directory).ok().flatten().is_some_and(|meta| meta.file_type().is_dir()) {
            let _ = std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700));
            for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
                reopen_all(&entry.path());
            }
        }
    }

    /// Le journal `path`, borné à `max`, qui dit ses pannes dans `complaints` plutôt que sur la
    /// vraie sortie d'erreur.
    fn open(path: &Path, max: u64, complaints: &Written) -> LogFile {
        let mut file = LogFile::open(path, max).expect("journal ouvert");
        file.complaints = Box::new(complaints.clone());
        file
    }

    /// Le journal de l'app réglé par `filters`, sa sortie d'erreur dans `stderr`, et `file`.
    fn logger(filters: &str, stderr: &Written, file: Option<LogFile>) -> NoWireLog {
        let builder = || {
            let mut builder = env_logger::Builder::new();
            builder.parse_filters(filters);
            builder
        };
        let mut to_stderr = builder();
        to_stderr.target(env_logger::Target::Pipe(Box::new(stderr.clone())));
        NoWireLog::new(to_stderr.build(), file.map(|file| to_file(builder(), file)))
    }

    fn write(logger: &NoWireLog, target: &str, level: log::Level, text: &str) {
        logger.log(&log::Record::builder().target(target).level(level).args(format_args!("{text}")).build());
    }

    fn read(file: &Path) -> String {
        std::fs::read_to_string(file).unwrap_or_default()
    }

    fn size(file: &Path) -> u64 {
        std::fs::metadata(file).map_or(0, |meta| meta.len())
    }

    fn names(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(directory)
            .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Les lignes sans leur date : chacun des deux journaux lit l'heure lui-même, et une seconde
    /// peut passer entre les deux.
    fn undated(text: &str) -> Vec<&str> {
        text.lines().map(|line| line.split_once(' ').map_or(line, |(_, rest)| rest)).collect()
    }

    /// Combien de fois le journal a dit qu'il cessait d'écrire.
    fn stops(complaints: &Written) -> usize {
        complaints.text().matches("le journal ne s'écrit plus").count()
    }

    #[test]
    fn what_crosses_the_network_is_never_logged_even_at_trace_level() {
        let scratch = Scratch::new("fil");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let logger =
            logger("trace,ureq_proto::util=trace", &stderr, Some(open(&path, MAX_BYTES, &complaints)));
        for target in ["ureq", "ureq::run", "ureq::tls::native_tls", "ureq_proto", "ureq_proto::util"] {
            write(&logger, target, log::Level::Trace, "Authorization: Bearer secret");
            assert!(
                !logger.enabled(&log::Metadata::builder().target(target).level(log::Level::Trace).build())
            );
        }
        assert!(stderr.text().is_empty(), "rien du fil n'est écrit sur la sortie d'erreur");
        assert_eq!(read(&path), "", "ni dans le fichier");

        // le reste du journal, lui, est bien écrit, aux deux endroits
        write(&logger, "ureq::run", log::Level::Debug, "connexion établie");
        write(&logger, "capri_net", log::Level::Trace, "détail de Capri");
        for text in [stderr.text(), read(&path)] {
            assert!(text.contains("connexion établie") && text.contains("détail de Capri"), "{text}");
            assert!(!text.contains("secret"), "{text}");
        }
        assert_eq!(undated(&read(&path)), undated(&stderr.text()), "les mêmes lignes");
        assert!(complaints.text().is_empty());
    }

    #[test]
    fn without_a_file_the_network_is_never_logged_either() {
        let stderr = Written::default();
        let logger = logger("trace", &stderr, None);
        write(&logger, "ureq::unversioned::transport", log::Level::Trace, "Authorization: Bearer xyz");
        write(&logger, "ureq", log::Level::Info, "connexion");
        assert!(!stderr.text().contains("Bearer"), "{}", stderr.text());
        assert!(stderr.text().contains("ureq] connexion"), "{}", stderr.text());
        assert_eq!(logger.filter(), log::LevelFilter::Trace);
    }

    #[test]
    fn a_line_lands_in_the_file_with_its_date_level_and_origin() {
        let scratch = Scratch::new("ligne");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let logger = logger("info", &stderr, Some(open(&path, MAX_BYTES, &complaints)));
        write(&logger, "capri_app::essai", log::Level::Warn, "réglage illisible");
        let text = read(&path);
        // [2026-11-12T10:00:00Z WARN  capri_app::essai] réglage illisible
        let line = text.lines().next().expect("une ligne");
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(line.starts_with('['), "{line}");
        let (date, rest) = line[1..].split_once(' ').expect("la date d'abord");
        assert_eq!(date.len(), "2026-11-12T10:00:00Z".len(), "{line}");
        assert!(date.as_bytes()[4] == b'-' && date.as_bytes()[10] == b'T' && date.ends_with('Z'), "{line}");
        assert!(date[..4].bytes().all(|b| b.is_ascii_digit()), "{line}");
        assert_eq!(rest, "WARN  capri_app::essai] réglage illisible");
        assert!(text.ends_with('\n'));
        assert!(stderr.text().ends_with("WARN  capri_app::essai] réglage illisible\n"), "{}", stderr.text());
        // les autres niveaux aussi, chacun le sien
        write(&logger, "capri_app", log::Level::Error, "en panne");
        write(&logger, "capri_app", log::Level::Info, "lancée");
        let text = read(&path);
        assert!(
            text.contains(" ERROR capri_app] en panne\n") && text.contains(" INFO  capri_app] lancée\n"),
            "{text}"
        );
    }

    #[test]
    fn the_file_has_no_colours_even_when_they_are_asked_for() {
        let scratch = Scratch::new("couleurs");
        let (complaints, path) = (Written::default(), scratch.journal());
        let mut colourful = env_logger::Builder::new();
        colourful.parse_filters("info").write_style(env_logger::WriteStyle::Always);
        let file = to_file(colourful, open(&path, MAX_BYTES, &complaints));
        file.log(
            &log::Record::builder()
                .target("capri_app")
                .level(log::Level::Error)
                .args(format_args!("x"))
                .build(),
        );
        let text = read(&path);
        assert!(text.contains("ERROR capri_app] x"), "{text:?}");
        assert!(!text.contains('\u{1b}'), "aucune séquence de couleur : {text:?}");
    }

    #[test]
    fn the_filter_holds_for_the_file_too() {
        let scratch = Scratch::new("filtre");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let logger = logger("info,capri_net=debug", &stderr, Some(open(&path, MAX_BYTES, &complaints)));
        assert_eq!(logger.filter(), log::LevelFilter::Debug);
        write(&logger, "capri_app", log::Level::Debug, "détail");
        write(&logger, "capri_app", log::Level::Info, "important");
        write(&logger, "capri_net", log::Level::Debug, "réseau en détail");
        write(&logger, "capri_net", log::Level::Trace, "réseau en trace");
        let text = read(&path);
        assert!(text.contains("important") && text.contains("réseau en détail"), "{text}");
        assert!(!text.contains("] détail") && !text.contains("en trace"), "{text}");
        assert_eq!(undated(&text), undated(&stderr.text()), "le même filtre aux deux endroits");
    }

    #[test]
    fn at_launch_a_file_over_the_bound_becomes_the_previous_one() {
        let scratch = Scratch::new("lancement");
        let (complaints, path) = (Written::default(), scratch.journal());
        let previous = path.with_file_name(PREVIOUS_NAME);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "a".repeat(150)).unwrap();
        std::fs::write(&previous, "très ancien").unwrap();
        let mut log = open(&path, 100, &complaints);
        assert_eq!(read(&previous), "a".repeat(150), "l'ancien précédent a disparu");
        assert_eq!(read(&path), "", "un nouveau commence");
        log.write_all(b"nouvelle ligne\n").unwrap();
        assert_eq!(read(&path), "nouvelle ligne\n");

        // sous la borne : la suite du même fichier
        drop(log);
        let mut log = open(&path, 100, &complaints);
        log.write_all(b"suite\n").unwrap();
        assert_eq!(read(&path), "nouvelle ligne\nsuite\n");
        assert_eq!(read(&previous), "a".repeat(150));
        assert_eq!(names(path.parent().unwrap()), ["capri.log", PREVIOUS_NAME]);
        assert!(complaints.text().is_empty());
    }

    #[test]
    fn on_the_way_it_rotates_and_neither_file_ever_crosses_the_bound() {
        let scratch = Scratch::new("borne");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let previous = path.with_file_name(PREVIOUS_NAME);
        let max = 400;
        let logger = logger("info", &stderr, Some(open(&path, max, &complaints)));
        let long = "é".repeat(2_000);
        for round in 0..600 {
            let text = match round % 7 {
                0 => long.clone(),
                1 => "la même ligne, en boucle".to_owned(),
                _ => format!("ligne {round}"),
            };
            write(&logger, "capri_app", log::Level::Info, &text);
            assert!(
                size(&path) <= max && size(&previous) <= max,
                "tour {round} : {} et {}",
                size(&path),
                size(&previous)
            );
            assert!(names(path.parent().unwrap()).len() <= 2);
        }
        write(&logger, "capri_app", log::Level::Info, "la dernière");
        assert!(read(&path).ends_with("capri_app] la dernière\n"), "{}", read(&path));
        assert!(read(&previous).contains("capri_app] "), "le précédent garde des lignes");
        // la ligne trop longue est rognée, sans couper un caractère (le fichier se lit en UTF-8)
        assert!(std::fs::read_to_string(&path).is_ok() && std::fs::read_to_string(&previous).is_ok());
        assert_eq!(names(path.parent().unwrap()), ["capri.log", PREVIOUS_NAME]);
        // la sortie d'erreur, elle, a tout reçu, sans rien rogner
        assert!(stderr.text().contains(&long));
        assert!(complaints.text().is_empty(), "aucune panne : {}", complaints.text());
    }

    #[test]
    fn the_same_line_in_a_loop_at_the_real_bound_never_crosses_it() {
        let scratch = Scratch::new("boucle");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let previous = path.with_file_name(PREVIOUS_NAME);
        let logger = logger("info", &stderr, Some(open(&path, MAX_BYTES, &complaints)));
        let line = "x".repeat(1_000);
        // plus de quatre fois ce que tiennent les deux fichiers ensemble
        for _ in 0..(9 * MAX_BYTES / 1_000) {
            write(&logger, "capri_app", log::Level::Warn, &line);
        }
        assert!(size(&path) <= MAX_BYTES && size(&previous) <= MAX_BYTES);
        assert!(size(&previous) > MAX_BYTES - 1_100, "le précédent a été rempli : {}", size(&previous));
        assert_eq!(names(path.parent().unwrap()), ["capri.log", PREVIOUS_NAME]);
        assert!(complaints.text().is_empty());
    }

    #[test]
    fn a_folder_that_cannot_be_created_leaves_only_the_standard_error() {
        let scratch = Scratch::new("dossier");
        // un fichier ordinaire là où il faudrait un dossier
        std::fs::write(scratch.0.join("logs"), "pas un dossier").unwrap();
        assert!(LogFile::open(&scratch.journal(), MAX_BYTES).is_err());
        assert_eq!(read(&scratch.0.join("logs")), "pas un dossier");
        // sans fichier, la sortie d'erreur continue
        let stderr = Written::default();
        let logger = logger("info", &stderr, None);
        write(&logger, "capri_app", log::Level::Info, "elle continue");
        assert!(stderr.text().contains("elle continue"));
        assert!(LogFile::open(Path::new("capri.log"), MAX_BYTES).is_err(), "sans dossier");
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_she_may_not_write_in_leaves_only_the_standard_error() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("ferme");
        std::fs::set_permissions(&scratch.0, std::fs::Permissions::from_mode(0o500)).unwrap();
        if std::fs::write(scratch.0.join("sonde"), "").is_ok() {
            return; // sous root, un dossier fermé ne l'est pas : rien à essayer ici
        }
        assert!(LogFile::open(&scratch.journal(), MAX_BYTES).is_err());
        assert!(look(&scratch.0.join("logs")).unwrap().is_none(), "rien n'est créé");
    }

    #[test]
    fn something_else_than_a_regular_file_is_never_written() {
        let scratch = Scratch::new("pas-un-fichier");
        let path = scratch.journal();
        std::fs::create_dir_all(&path).unwrap();
        assert!(LogFile::open(&path, MAX_BYTES).is_err(), "un dossier à sa place");
        assert!(path.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_never_written_through_nor_replaced() {
        let scratch = Scratch::new("lien");
        let path = scratch.journal();
        let directory = path.parent().unwrap().to_owned();
        std::fs::create_dir_all(&directory).unwrap();
        let elsewhere = scratch.0.join("ailleurs.txt");
        std::fs::write(&elsewhere, "à quelqu'un d'autre").unwrap();

        // le fichier est un lien, vers un fichier ou vers rien
        std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
        assert!(LogFile::open(&path, MAX_BYTES).is_err());
        assert_eq!(read(&elsewhere), "à quelqu'un d'autre");
        std::fs::remove_file(&path).unwrap();
        let nowhere = scratch.0.join("absent.txt");
        std::os::unix::fs::symlink(&nowhere, &path).unwrap();
        assert!(LogFile::open(&path, MAX_BYTES).is_err());
        assert!(look(&nowhere).unwrap().is_none(), "rien n'est créé au bout du lien");
        std::fs::remove_file(&path).unwrap();

        // le dossier est un lien
        let other = scratch.0.join("autre-dossier");
        std::fs::create_dir(&other).unwrap();
        let linked = scratch.0.join("relie");
        std::os::unix::fs::symlink(&other, &linked).unwrap();
        assert!(LogFile::open(&linked.join("capri.log"), MAX_BYTES).is_err());
        assert!(names(&other).is_empty(), "rien n'est écrit dans le dossier qu'il désigne");

        // le précédent est un lien : la rotation est refusée, rien n'est écrit au bout, et le
        // journal cesse d'écrire en le disant une fois
        let previous = path.with_file_name(PREVIOUS_NAME);
        std::os::unix::fs::symlink(&elsewhere, &previous).unwrap();
        let (stderr, complaints) = (Written::default(), Written::default());
        let logger = logger("info", &stderr, Some(open(&path, 120, &complaints)));
        for round in 0..20 {
            write(&logger, "capri_app", log::Level::Info, &format!("ligne {round}"));
        }
        assert!(std::fs::symlink_metadata(&previous).unwrap().file_type().is_symlink());
        assert_eq!(read(&elsewhere), "à quelqu'un d'autre");
        assert!(size(&path) <= 120);
        assert!(stderr.text().contains("ligne 19"), "la sortie d'erreur continue");
        assert_eq!(stops(&complaints), 1, "{}", complaints.text());
    }

    #[test]
    fn a_file_removed_on_the_way_never_stops_her() {
        let scratch = Scratch::new("retire");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let logger = logger("info", &stderr, Some(open(&path, MAX_BYTES, &complaints)));
        write(&logger, "capri_app", log::Level::Info, "avant");
        assert!(read(&path).contains("avant"));
        // sous Windows, un fichier ouvert ne se retire pas toujours : l'essai ne l'exige pas
        let removed = std::fs::remove_file(&path).is_ok();
        for round in 0..5 {
            write(&logger, "capri_app", log::Level::Info, &format!("après {round}"));
        }
        assert!(stderr.text().contains("avant") && stderr.text().contains("après 4"), "{}", stderr.text());
        if removed && cfg!(unix) {
            assert!(look(&path).unwrap().is_none(), "il n'est pas recréé");
            assert_eq!(stops(&complaints), 1, "dit une fois : {}", complaints.text());
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_file_replaced_on_the_way_is_left_alone() {
        let scratch = Scratch::new("remplace");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let logger = logger("info", &stderr, Some(open(&path, MAX_BYTES, &complaints)));
        write(&logger, "capri_app", log::Level::Info, "avant");
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, "un autre fichier\n").unwrap();
        write(&logger, "capri_app", log::Level::Info, "après");
        write(&logger, "capri_app", log::Level::Info, "encore après");
        assert_eq!(
            read(&path),
            "un autre fichier\n",
            "elle n'écrit pas dans un fichier qui n'est plus le sien"
        );
        assert!(stderr.text().contains("encore après"));
        assert_eq!(stops(&complaints), 1, "{}", complaints.text());
    }

    #[cfg(unix)]
    #[test]
    fn a_rotation_that_fails_on_the_way_never_stops_her() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("panne");
        let (stderr, complaints, path) = (Written::default(), Written::default(), scratch.journal());
        let directory = path.parent().unwrap().to_owned();
        let logger = logger("info", &stderr, Some(open(&path, 150, &complaints)));
        write(&logger, "capri_app", log::Level::Info, "première");
        // le dossier fermé : la rotation (un renommage) échouera
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
        if std::fs::write(directory.join("sonde"), "").is_ok() {
            return; // sous root, un dossier fermé ne l'est pas : rien à essayer ici
        }
        for round in 0..10 {
            write(&logger, "capri_app", log::Level::Info, &format!("ligne {round}"));
        }
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(stderr.text().contains("ligne 9"), "la sortie d'erreur continue : {}", stderr.text());
        assert!(size(&path) <= 150);
        assert!(!read(&path).contains("ligne 9"), "il a cessé d'écrire");
        assert!(!directory.join(PREVIOUS_NAME).exists());
        assert_eq!(stops(&complaints), 1, "{}", complaints.text());
    }

    #[cfg(unix)]
    #[test]
    fn the_folder_and_the_file_are_the_user_s_only() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        let scratch = Scratch::new("droits");
        let (complaints, path) = (Written::default(), scratch.journal());
        let mut log = open(&path, 100, &complaints);
        assert_eq!(mode(path.parent().unwrap()), 0o700, "le dossier");
        assert_eq!(mode(&path), 0o600, "le fichier");
        // après une rotation : le nouveau comme l'ancien
        log.write_all(&[b'x'; 90]).unwrap();
        log.write_all(&[b'y'; 90]).unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&path.with_file_name(PREVIOUS_NAME)), 0o600);
        drop(log);
        // un fichier laissé plus ouvert par quelqu'un d'autre est resserré
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _log = open(&path, 100, &complaints);
        assert_eq!(mode(&path), 0o600);
        // au lancement, un fichier trop gros qui tourne : le nouveau aussi
        drop(_log);
        std::fs::write(&path, [b'z'; 150]).unwrap();
        let _log = open(&path, 100, &complaints);
        assert_eq!((mode(&path), size(&path)), (0o600, 0));
        assert!(complaints.text().is_empty());
    }

    #[test]
    fn only_a_regular_file_is_shown_and_otherwise_she_says_why() {
        use capri_platform::launch::LaunchError;
        let scratch = Scratch::new("montre");
        let (complaints, path) = (Written::default(), scratch.journal());
        let never = |_: &Path| -> Result<(), LaunchError> { panic!("rien à montrer : jamais appelé") };
        // pas de place connue, ou pas encore de fichier (journal en panne) : elle le dit
        assert_eq!(show(None, never), Some(NOTHING_TO_SHOW));
        assert_eq!(show(Some(&path), never), Some(NOTHING_TO_SHOW));
        let _log = open(&path, MAX_BYTES, &complaints);
        assert_eq!(show(Some(path.parent().unwrap()), never), Some(NOTHING_TO_SHOW), "pas un dossier");
        #[cfg(unix)]
        {
            let link = scratch.0.join("lien.log");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert_eq!(show(Some(&link), never), Some(NOTHING_TO_SHOW), "pas un lien");
        }
        // le fichier est là : il est montré, lui
        let mut shown = None;
        let reveal = |file: &Path| {
            shown = Some(file.to_owned());
            Ok(())
        };
        assert_eq!(show(Some(&path), reveal), None, "montré : rien à dire");
        assert_eq!(shown.as_deref(), Some(path.as_path()));
        // et si le montrer échoue, elle le dit
        assert_eq!(show(Some(&path), |_| Err(LaunchError::Unavailable)), Some(CANNOT_SHOW_HERE));
        let failed = |file: &Path| {
            Err(LaunchError::Io { path: file.to_owned(), source: std::io::ErrorKind::NotFound.into() })
        };
        assert_eq!(show(Some(&path), failed), Some(NOT_SHOWN));
        // aucune de ses phrases ne renvoie à une commande de terminal
        for said in [NOTHING_TO_SHOW, CANNOT_SHOW_HERE, NOT_SHOWN] {
            assert!(!said.contains("capri ") && !said.contains('`'), "{said}");
        }
    }
}
