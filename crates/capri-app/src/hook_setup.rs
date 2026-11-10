//! Brancher et débrancher le hook dans le `settings.json` de Claude Code : la partie fichiers,
//! et la commande `capri hook …`, qui règle aussi le miroir (`claude-code.json`, dans le dossier
//! de Capri) et installe le style de sortie (`style_setup`).
//!
//! Ce que le fichier doit contenir est décidé par `capri_claude::settings`, qui ne lit ni n'écrit
//! rien. Ici, on trouve le fichier, on le sauvegarde, et on le remplace d'un seul coup.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use capri_claude::mirror::{self, MirrorSettings};
use capri_claude::places;
use capri_claude::settings::{self, BACKUP_PREFIX, SettingsError, Status};
use capri_claude::style::StyleState;

use crate::{brain_setup, style_setup};

/// Nombre de sauvegardes gardées à côté du fichier.
const KEPT_BACKUPS: usize = 5;

/// Ce qu'une opération a fait, ou ferait.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Le fichier réellement visé, liens symboliques suivis.
    pub settings: PathBuf,
    /// Le contenu change-t-il ?
    pub changed: bool,
    /// La copie de l'original, si le fichier existait et a été modifié.
    pub backup: Option<PathBuf>,
    pub lines_before: usize,
    pub lines_after: usize,
}

/// Le dossier de Claude Code : `CLAUDE_CONFIG_DIR` si elle est définie, sinon `~/.claude`
/// (`%USERPROFILE%\.claude` sous Windows).
pub fn claude_dir() -> Result<PathBuf> {
    Ok(places::claude_dir(|name| std::env::var_os(name), places::System::CURRENT)?)
}

/// Le `settings.json` de l'utilisateur, dans le dossier de Claude Code.
pub fn settings_path() -> Result<PathBuf> {
    Ok(claude_dir()?.join("settings.json"))
}

/// Là où le binaire `capri-hook` est attendu : à côté de l'exécutable courant. Ne vérifie pas
/// qu'il y est.
pub fn hook_binary_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("emplacement de l'exécutable")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    Ok(exe.with_file_name(format!("{}{}", settings::HOOK_BINARY, std::env::consts::EXE_SUFFIX)))
}

/// Le binaire `capri-hook`, attendu à côté de l'exécutable courant (pour un terminal : l'erreur
/// dit comment le construire).
pub fn hook_binary() -> Result<PathBuf> {
    let hook = hook_binary_path()?;
    if !hook.is_file() {
        bail!("{} est introuvable : compile-le avec `cargo build --release`", hook.display());
    }
    Ok(hook)
}

/// Le réglage du miroir, dans le dossier de Capri (celui que relit `capri-hook`). Sans fichier :
/// désactivé. Un fichier illisible est une erreur ici ; le hook, lui, le prend pour « désactivé ».
pub fn load_mirror(directory: &Path) -> Result<MirrorSettings> {
    let file = directory.join(mirror::SETTINGS_FILE);
    let Some(text) = brain_setup::read_text(&file)? else { return Ok(MirrorSettings::default()) };
    if text.len() as u64 > mirror::MAX_SETTINGS_BYTES {
        bail!("{} est trop gros pour être le réglage du miroir : il compte pour désactivé", file.display());
    }
    MirrorSettings::from_json(&text).with_context(|| format!("{} : il compte pour désactivé", file.display()))
}

/// Écrit le réglage du miroir, d'un seul coup.
pub fn save_mirror(directory: &Path, settings: MirrorSettings) -> Result<PathBuf> {
    brain_setup::write_file(directory, mirror::SETTINGS_FILE, &settings.to_json())
}

pub fn install(settings_file: &Path, hook: &Path, dry_run: bool) -> Result<Outcome> {
    let hook = hook.to_str().with_context(|| format!("le chemin {} n'est pas de l'UTF-8", hook.display()))?;
    rewrite(settings_file, dry_run, |text| settings::install(text, hook))
}

pub fn uninstall(settings_file: &Path, dry_run: bool) -> Result<Outcome> {
    rewrite(settings_file, dry_run, settings::uninstall)
}

pub fn status(settings_file: &Path) -> Result<Status> {
    let text = read(settings_file)?.map(|(_, text)| text).unwrap_or_default();
    settings::status(&text).with_context(|| format!("lecture de {}", settings_file.display()))
}

/// Le fichier réel et son texte, ou `None` s'il n'existe pas.
fn read(settings_file: &Path) -> Result<Option<(PathBuf, String)>> {
    if !settings_file.exists() {
        return Ok(None);
    }
    // c'est le vrai fichier qu'on remplacera, pas le lien qui y mène
    let target =
        settings_file.canonicalize().with_context(|| format!("résolution de {}", settings_file.display()))?;
    let bytes = std::fs::read(&target).with_context(|| format!("lecture de {}", target.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| anyhow!("{} n'est pas de l'UTF-8", target.display()))?;
    Ok(Some((target, text)))
}

fn rewrite(
    settings_file: &Path,
    dry_run: bool,
    edit: impl Fn(&str) -> Result<String, SettingsError>,
) -> Result<Outcome> {
    let existing = read(settings_file)?;
    let (target, original) = match &existing {
        Some((target, text)) => (target.clone(), text.as_str()),
        None => (settings_file.to_owned(), ""),
    };
    let updated = edit(original).with_context(|| format!("{} n'a pas été modifié", target.display()))?;
    let mut outcome = Outcome {
        settings: target.clone(),
        changed: updated != original,
        backup: None,
        lines_before: original.lines().count(),
        lines_after: updated.lines().count(),
    };
    if !outcome.changed || dry_run {
        return Ok(outcome);
    }

    let directory = target.parent().context("le fichier de réglages n'a pas de dossier")?;
    std::fs::create_dir_all(directory).with_context(|| format!("création de {}", directory.display()))?;
    if existing.is_some() {
        outcome.backup = Some(save_backup(directory, &target, original.as_bytes())?);
    }
    replace_atomically(&target, existing.is_some().then_some(original.as_bytes()), updated.as_bytes())?;
    prune_backups(directory);
    Ok(outcome)
}

/// Copie l'original à côté du fichier, sous un nom daté, sans jamais écraser une copie existante.
fn save_backup(directory: &Path, target: &Path, original: &[u8]) -> Result<PathBuf> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let name = settings::backup_name(now);
    for attempt in 1..=9 {
        let path = directory.join(if attempt == 1 { name.clone() } else { format!("{name}-{attempt}") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(original)
                    .and_then(|()| file.sync_all())
                    .with_context(|| format!("écriture de {}", path.display()))?;
                if let Ok(meta) = std::fs::metadata(target) {
                    let _ = std::fs::set_permissions(&path, meta.permissions());
                }
                return Ok(path);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).with_context(|| format!("création de {}", path.display())),
        }
    }
    bail!("trop de sauvegardes portent déjà le nom {name}")
}

/// Remplace `target` par `updated` d'un seul coup : un fichier temporaire, écrit et synchronisé,
/// puis renommé. `original` : le contenu lu au départ (`None` si le fichier n'existait pas) ; s'il
/// a changé entre-temps, on n'écrit rien. Sert aussi au style de sortie (`style_setup`).
pub(crate) fn replace_atomically(target: &Path, original: Option<&[u8]>, updated: &[u8]) -> Result<()> {
    let directory = target.parent().context("le fichier n'a pas de dossier")?;
    let name = target.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
    let temporary = directory.join(format!(".{name}.capri-tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);

    let written = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("création de {}", temporary.display()))?;
        file.write_all(updated).with_context(|| format!("écriture de {}", temporary.display()))?;
        if original.is_some() {
            let meta =
                std::fs::metadata(target).with_context(|| format!("lecture de {}", target.display()))?;
            std::fs::set_permissions(&temporary, meta.permissions())?;
        }
        file.sync_all()?;
        drop(file);

        // Claude Code ou un autre outil a pu réécrire le fichier pendant ce temps
        let current = match std::fs::read(target) {
            Ok(bytes) => Some(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err).with_context(|| format!("relecture de {}", target.display())),
        };
        if current.as_deref() != original {
            bail!("{} a changé pendant l'opération : rien n'a été écrit, réessaie", target.display());
        }
        std::fs::rename(&temporary, target).with_context(|| format!("remplacement de {}", target.display()))
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

/// Ne garde que nos sauvegardes les plus récentes. Les fichiers des autres ne sont pas touchés.
fn prune_backups(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else { return };
    let mut backups: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(BACKUP_PREFIX))
        })
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(KEPT_BACKUPS);
    for old in &backups[..excess] {
        let _ = std::fs::remove_file(old);
    }
}

const USAGE: &str = "\
usage : capri hook install [--dry-run]    brancher Capri sur Claude Code
        capri hook uninstall [--dry-run]  la débrancher
        capri hook status                 où en sont le branchement, le miroir et le style
        capri hook mirror on | off        qu'elle dise, ou non, la ligne que Claude écrit pour elle
                                          (désactivé par défaut)
        capri hook style install          installer le style de sortie « Capri » de Claude Code
        capri hook style uninstall        le retirer";

/// La commande `capri hook <action>`. Rend le code de sortie du processus.
pub fn run_cli(args: &[String]) -> i32 {
    let done = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [action @ ("install" | "uninstall")] => cli(action, false),
        [action @ ("install" | "uninstall"), "--dry-run"] => cli(action, true),
        ["status"] => cli("status", false),
        ["mirror", "on"] => mirror_cli(true),
        ["mirror", "off"] => mirror_cli(false),
        ["style", "install"] => style_cli(true),
        ["style", "uninstall"] => style_cli(false),
        _ => {
            eprintln!("{USAGE}");
            return 2;
        }
    };
    match done {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("capri : {err:#}");
            if matches!(err.downcast_ref(), Some(SettingsError::NotCanonical | SettingsError::InvalidJson(_)))
                && let Ok(hook) = hook_binary()
            {
                eprintln!(
                    "\nPour brancher Capri à la main, ajoute ces groupes aux événements du même nom dans la \
                     clé « hooks » du fichier :\n{}",
                    settings::manual_snippet(&hook.to_string_lossy())
                );
            }
            1
        }
    }
}

fn cli(action: &str, dry_run: bool) -> Result<()> {
    let settings_file = settings_path()?;
    if action == "status" {
        hook_status(&settings_file)?;
        mirror_status()?;
        return style_status();
    }

    let outcome = match action {
        "install" => install(&settings_file, &hook_binary()?, dry_run)?,
        _ => uninstall(&settings_file, dry_run)?,
    };
    let delta = outcome.lines_after as i64 - outcome.lines_before as i64;
    let verb = if action == "install" { "branché" } else { "débranché" };
    match (outcome.changed, dry_run) {
        (false, _) => println!("Rien à faire : Claude Code est déjà {verb}."),
        (true, true) => println!(
            "Simulation, rien n'est écrit : {} passerait de {} à {} lignes ({delta:+}).",
            outcome.settings.display(),
            outcome.lines_before,
            outcome.lines_after
        ),
        (true, false) => {
            println!("Claude Code est {verb}.");
            println!("réglages   : {} ({delta:+} lignes)", outcome.settings.display());
            if let Some(backup) = &outcome.backup {
                println!("sauvegarde : {}", backup.display());
            }
            println!("Les sessions ouvertes le prennent en compte toutes seules ; vérifie avec /hooks.");
        }
    }
    Ok(())
}

/// Ce que `capri hook status` dit du branchement.
fn hook_status(settings_file: &Path) -> Result<()> {
    let status = status(settings_file)?;
    println!("réglages : {}", settings_file.display());
    if !status.is_installed() {
        println!("{} Pour le brancher : capri hook install", status.describe());
        return Ok(());
    }
    println!("{}", status.describe());
    for command in &status.commands {
        let found = if Path::new(command).is_file() {
            "présent"
        } else {
            "INTROUVABLE : rebranche avec capri hook install"
        };
        println!("binaire  : {command} ({found})");
    }
    if status.all_hooks_disabled {
        println!("{}", settings::ALL_HOOKS_DISABLED);
    }
    Ok(())
}

/// Ce que `capri hook status` dit du miroir.
fn mirror_status() -> Result<()> {
    let directory = brain_setup::config_dir()?;
    match load_mirror(&directory) {
        Ok(settings) if settings.enabled => println!(
            "miroir   : activé ({}). Pour le désactiver : capri hook mirror off",
            directory.join(mirror::SETTINGS_FILE).display()
        ),
        Ok(_) => println!("miroir   : désactivé. Pour l'activer : capri hook mirror on"),
        Err(err) => println!("miroir   : {err:#}"),
    }
    Ok(())
}

/// `capri hook mirror on` ou `capri hook mirror off`.
fn mirror_cli(enabled: bool) -> Result<()> {
    let file = save_mirror(&brain_setup::config_dir()?, MirrorSettings { enabled })?;
    if !enabled {
        println!("{}", mirror::TURNED_OFF);
        println!("réglage  : {}", file.display());
        return Ok(());
    }
    println!("{}", mirror::TURNED_ON);
    println!("{}", mirror::ABOUT);
    println!("réglage  : {}", file.display());
    // ce qui manque encore pour qu'une phrase arrive jusqu'à elle
    if settings_path().and_then(|file| status(&file)).is_ok_and(|status| !status.is_installed()) {
        println!("{} Pour le brancher : capri hook install", mirror::NOT_PLUGGED);
    }
    let style = style_setup::styles_dir().and_then(|directory| style_setup::status(&directory));
    if style.is_ok_and(|found| !matches!(found.state, StyleState::Ours { .. })) {
        println!("{} Pour l'installer : capri hook style install", mirror::NO_STYLE);
    }
    Ok(())
}

/// Ce que `capri hook status` dit du style de sortie.
fn style_status() -> Result<()> {
    let found = style_setup::status(&style_setup::styles_dir()?)?;
    println!("style    : {}", capri_claude::style::describe(found.state, &found.file.display().to_string()));
    match found.state {
        StyleState::Absent => println!("           Pour l'installer : capri hook style install"),
        StyleState::Ours { current: false } => {
            println!("           Pour le mettre à jour : capri hook style install")
        }
        StyleState::Ours { current: true } => println!("           {}", capri_claude::style::HOW_TO_CHOOSE),
        StyleState::Foreign => {}
    }
    Ok(())
}

/// `capri hook style install` ou `capri hook style uninstall`.
fn style_cli(install: bool) -> Result<()> {
    let directory = style_setup::styles_dir()?;
    if install {
        let (file, done) = style_setup::install(&directory)?;
        println!("{}", capri_claude::style::installed(done, &file.display().to_string()));
        println!("{}", capri_claude::style::HOW_TO_CHOOSE);
    } else {
        let (file, done) = style_setup::uninstall(&directory)?;
        println!("{}", capri_claude::style::removed(done, &file.display().to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    const ORIGINAL: &str = "{\n  \"theme\": \"dark\",\n  \"hooks\": {\n    \"Stop\": [\n      {\n        \"hooks\": [\n          {\n            \"type\": \"command\",\n            \"command\": \"/opt/autre/notify.sh\"\n          }\n        ]\n      }\n    ]\n  }\n}\n";

    fn scratch_dir(name: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique =
            format!("capri-setup-{}-{}-{name}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
        let dir = std::env::temp_dir().join(unique);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // sous macOS, le dossier temporaire est derrière un lien symbolique
        dir.canonicalize().unwrap()
    }

    fn backups(dir: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.file_name().unwrap().to_string_lossy().starts_with(BACKUP_PREFIX))
            .collect();
        found.sort();
        found
    }

    fn hook() -> PathBuf {
        PathBuf::from("/Applications/Capri/capri-hook")
    }

    #[test]
    fn install_backs_up_then_writes_and_uninstall_restores_the_exact_bytes() {
        let dir = scratch_dir("aller-retour");
        let file = dir.join("settings.json");
        std::fs::write(&file, ORIGINAL).unwrap();

        let done = install(&file, &hook(), false).unwrap();
        assert!(done.changed);
        assert_eq!(done.settings, file);
        let backup = done.backup.expect("une sauvegarde de l'original");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), ORIGINAL);
        assert!(status(&file).unwrap().is_complete());
        assert!(done.lines_after > done.lines_before);

        let undone = uninstall(&file, false).unwrap();
        assert!(undone.changed);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), ORIGINAL);
        assert_eq!(backups(&dir).len(), 2, "chaque écriture a sa sauvegarde");
        assert!(!status(&file).unwrap().is_installed());
        // aucun fichier temporaire ne traîne
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("capri-tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn a_dry_run_or_a_no_op_writes_nothing() {
        let dir = scratch_dir("simulation");
        let file = dir.join("settings.json");
        std::fs::write(&file, ORIGINAL).unwrap();

        let simulated = install(&file, &hook(), true).unwrap();
        assert!(simulated.changed && simulated.backup.is_none());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), ORIGINAL);
        assert!(backups(&dir).is_empty());

        let nothing = uninstall(&file, false).unwrap();
        assert!(!nothing.changed && nothing.backup.is_none());
        assert!(backups(&dir).is_empty(), "rien à débrancher : pas de sauvegarde inutile");

        install(&file, &hook(), false).unwrap();
        let again = install(&file, &hook(), false).unwrap();
        assert!(!again.changed, "déjà branché sur ce binaire");
        assert_eq!(backups(&dir).len(), 1);
    }

    #[test]
    fn a_missing_file_and_its_directory_are_created() {
        let dir = scratch_dir("absent");
        let file = dir.join("pas-encore/settings.json");
        assert!(!status(&file).unwrap().is_installed());
        let done = install(&file, &hook(), false).unwrap();
        assert!(done.changed && done.backup.is_none(), "rien à sauvegarder");
        assert!(status(&file).unwrap().is_complete());
        uninstall(&file, false).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}\n");
    }

    #[test]
    fn a_file_we_cannot_rewrite_is_left_untouched() {
        let dir = scratch_dir("refus");
        let file = dir.join("settings.json");
        for content in
            ["{\n    \"theme\": \"dark\"\n}\n", "{ pas du json", "{\n  \"a\": 1, // commentaire\n}\n"]
        {
            std::fs::write(&file, content).unwrap();
            let error = install(&file, &hook(), false).unwrap_err();
            assert!(error.downcast_ref::<SettingsError>().is_some(), "{error:#}");
            assert_eq!(std::fs::read_to_string(&file).unwrap(), content);
            assert!(backups(&dir).is_empty());
        }
        std::fs::write(&file, [0xff, 0xfe, 0x00]).unwrap();
        assert!(install(&file, &hook(), false).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), [0xff, 0xfe, 0x00]);
    }

    #[test]
    fn a_change_made_meanwhile_is_never_overwritten() {
        let dir = scratch_dir("course");
        let file = dir.join("settings.json");
        std::fs::write(&file, "{\n  \"theme\": \"light\"\n}\n").unwrap();
        // on croyait remplacer un autre contenu : quelqu'un a écrit entre-temps
        let error = replace_atomically(&file, Some(ORIGINAL.as_bytes()), b"{}\n").unwrap_err();
        assert!(format!("{error:#}").contains("a changé pendant l'opération"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{\n  \"theme\": \"light\"\n}\n");
        // même chose si le fichier est apparu alors qu'il n'existait pas
        assert!(replace_atomically(&file, None, b"{}\n").is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "le fichier temporaire est retiré");
    }

    #[test]
    fn only_the_latest_backups_are_kept_and_other_files_are_left_alone() {
        let dir = scratch_dir("menage");
        let file = dir.join("settings.json");
        std::fs::write(&file, ORIGINAL).unwrap();
        std::fs::write(dir.join("settings.json.bak"), "à quelqu'un d'autre").unwrap();
        for stamp in 1..=8 {
            std::fs::write(dir.join(format!("{BACKUP_PREFIX}2026010{stamp}-000000")), "vieux").unwrap();
        }
        install(&file, &hook(), false).unwrap();
        let kept = backups(&dir);
        assert_eq!(kept.len(), KEPT_BACKUPS);
        assert!(
            kept.iter().all(|path| !path.to_string_lossy().contains("20260101-")),
            "les plus anciennes partent"
        );
        assert_eq!(
            std::fs::read_to_string(kept.last().unwrap()).unwrap(),
            ORIGINAL,
            "la plus récente est la nôtre"
        );
        assert_eq!(std::fs::read_to_string(dir.join("settings.json.bak")).unwrap(), "à quelqu'un d'autre");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_followed_not_replaced_and_permissions_are_kept() {
        use std::os::unix::fs::PermissionsExt;

        let dir = scratch_dir("lien");
        let real_dir = dir.join("dotfiles");
        std::fs::create_dir(&real_dir).unwrap();
        let real = real_dir.join("claude-settings.json");
        std::fs::write(&real, ORIGINAL).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("settings.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let done = install(&link, &hook(), false).unwrap();
        assert_eq!(done.settings, real, "c'est le vrai fichier qui est visé");
        assert!(
            std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(),
            "le lien est toujours un lien"
        );
        assert!(status(&link).unwrap().is_complete());
        assert_eq!(std::fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o600);
        let backup = done.backup.unwrap();
        assert_eq!(backup.parent().unwrap(), real_dir, "la sauvegarde est à côté du vrai fichier");
        assert_eq!(std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn the_mirror_is_off_until_written_and_read_back_like_the_hook_reads_it() {
        let dir = scratch_dir("miroir").join("capri");
        assert_eq!(load_mirror(&dir).unwrap(), MirrorSettings { enabled: false }, "sans fichier");
        let file = save_mirror(&dir, MirrorSettings { enabled: true }).unwrap();
        assert_eq!(file, dir.join("claude-code.json"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{\n  \"mirror\": true\n}\n");
        assert!(load_mirror(&dir).unwrap().enabled);
        save_mirror(&dir, MirrorSettings { enabled: false }).unwrap();
        assert!(!load_mirror(&dir).unwrap().enabled);

        // illisible ou trop gros : une erreur qui dit qu'il compte pour désactivé, comme pour le hook
        for broken in ["{ cassé".to_owned(), format!("{{\"mirror\":true,\"x\":\"{}\"}}", "x".repeat(5_000))]
        {
            std::fs::write(&file, broken).unwrap();
            let error = format!("{:#}", load_mirror(&dir).unwrap_err());
            assert!(error.contains("compte pour désactivé") && error.contains("claude-code.json"), "{error}");
        }
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
