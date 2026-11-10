//! Le cerveau de Capri côté fichiers, et la commande `capri chat …`.
//!
//! Deux fichiers dans le dossier de Capri : `chat.json` dit quel cerveau utiliser (son contenu est
//! décidé et vérifié par `capri_chat::brain`), et `persona.md`, facultatif, remplace sa
//! personnalité. Sans `chat.json`, Capri n'envoie rien nulle part : elle répète ce qu'on lui dit.
//!
//! Rien n'est gardé en mémoire : les fichiers sont relus à chaque message, si bien qu'un
//! changement fait en ligne de commande vaut dès la phrase suivante, sans relancer Capri.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use capri_chat::plain::PlainStream;
use capri_chat::{Brain, Endpoint, Provider, ReplyEvent, Role, Turn, persona};
use capri_net::Client;

const BRAIN_FILE: &str = "chat.json";
const PERSONA_FILE: &str = "persona.md";
/// La question de `capri chat test` quand on n'en donne pas.
const TEST_QUESTION: &str = "Présente-toi en une phrase.";

const USAGE: &str = "\
usage : capri chat status             quel cerveau Capri utilise
        capri chat use ollama [--model <nom>] [--url <adresse>]
                                      lui donner un modèle local, servi par Ollama
        capri chat test [<message>]   lui parler depuis le terminal
        capri chat off                lui retirer son cerveau";

/// De quoi répondre : le cerveau choisi et la personnalité à lui donner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    pub brain: Brain,
    pub persona: String,
}

/// Le dossier des réglages de Capri : `CAPRI_CONFIG_DIR` si elle est définie, sinon le dossier
/// habituel du système (sur macOS, celui où vit déjà sa socket).
pub fn config_dir() -> Result<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    if let Some(directory) = var("CAPRI_CONFIG_DIR") {
        return Ok(directory);
    }
    let home = || var("HOME").context("HOME n'est pas définie");
    if cfg!(target_os = "macos") {
        Ok(home()?.join("Library/Application Support/Capri"))
    } else if cfg!(windows) {
        Ok(var("LOCALAPPDATA").context("LOCALAPPDATA n'est pas définie")?.join("Capri"))
    } else {
        Ok(var("XDG_CONFIG_HOME").map_or_else(|| home().map(|home| home.join(".config")), Ok)?.join("capri"))
    }
}

/// Lit les réglages du dossier. `None` : aucun cerveau n'a été choisi.
pub fn load(directory: &Path) -> Result<Option<Setup>> {
    let file = directory.join(BRAIN_FILE);
    let Some(text) = read_text(&file)? else { return Ok(None) };
    let brain =
        Brain::from_json(&text).with_context(|| format!("réglages illisibles ({})", file.display()))?;
    Ok(Some(Setup { brain, persona: load_persona(directory) }))
}

/// La personnalité à utiliser. Un fichier `persona.md` illisible ou trop long ne fait pas taire
/// Capri : elle garde sa personnalité par défaut, et le journal dit pourquoi.
fn load_persona(directory: &Path) -> String {
    match custom_persona(directory) {
        Ok(Some(text)) => text,
        Ok(None) => persona::DEFAULT.to_owned(),
        Err(err) => {
            log::warn!("personnalité par défaut : {err:#}");
            persona::DEFAULT.to_owned()
        }
    }
}

/// Le texte de `persona.md`, s'il existe et dit quelque chose.
fn custom_persona(directory: &Path) -> Result<Option<String>> {
    let file = directory.join(PERSONA_FILE);
    let Some(text) = read_text(&file)? else { return Ok(None) };
    let chosen =
        persona::choose(Some(&text)).with_context(|| format!("{} n'est pas utilisé", file.display()))?;
    Ok((!text.trim().is_empty()).then(|| chosen.to_owned()))
}

/// Le texte d'un fichier, ou `None` s'il n'existe pas.
pub fn read_text(file: &Path) -> Result<Option<String>> {
    match std::fs::read(file) {
        Ok(bytes) => {
            String::from_utf8(bytes).map(Some).map_err(|_| anyhow!("{} n'est pas de l'UTF-8", file.display()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("lecture de {}", file.display())),
    }
}

/// Écrit le cerveau choisi.
pub fn save(directory: &Path, brain: &Brain) -> Result<PathBuf> {
    write_file(directory, BRAIN_FILE, &brain.to_json())
}

/// Écrit un fichier de réglages d'un seul coup : un fichier temporaire, puis un renommage. Le
/// dossier est créé s'il manque.
pub fn write_file(directory: &Path, name: &str, contents: &str) -> Result<PathBuf> {
    create_private_dir(directory).with_context(|| format!("création de {}", directory.display()))?;
    let file = directory.join(name);
    let temporary = directory.join(format!(".{name}.tmp-{}", std::process::id()));
    let written = (|| -> std::io::Result<()> {
        let mut handle = std::fs::File::create(&temporary)?;
        handle.write_all(contents.as_bytes())?;
        handle.sync_all()?;
        drop(handle);
        std::fs::rename(&temporary, &file)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written.with_context(|| format!("écriture de {}", file.display()))?;
    Ok(file)
}

/// Retire le cerveau choisi. Rend `false` s'il n'y en avait pas.
pub fn forget(directory: &Path) -> Result<bool> {
    let file = directory.join(BRAIN_FILE);
    match std::fs::remove_file(&file) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("suppression de {}", file.display())),
    }
}

/// Crée le dossier s'il manque, lisible par l'utilisateur seul là où le système le permet.
fn create_private_dir(directory: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory)
}

/// La commande `capri chat <action>`. Rend le code de sortie du processus.
pub fn run_cli(args: &[String]) -> i32 {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = match args.as_slice() {
        ["status"] => status(),
        ["use", provider, options @ ..] => match parse_use(provider, options) {
            Some((provider, model, url)) => choose(provider, model, url),
            None => return usage(),
        },
        ["test", words @ ..] => test(&words.join(" ")),
        ["off"] => off(),
        _ => return usage(),
    };
    match outcome {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("capri : {err:#}");
            1
        }
    }
}

fn usage() -> i32 {
    eprintln!("{USAGE}");
    2
}

/// Lit `ollama [--model <nom>] [--url <adresse>]`.
fn parse_use<'a>(
    provider: &str,
    options: &[&'a str],
) -> Option<(Provider, Option<&'a str>, Option<&'a str>)> {
    let provider = Provider::from_id(provider)?;
    let (mut model, mut url) = (None, None);
    let mut options = options.iter();
    while let Some(option) = options.next() {
        let value = options.next().filter(|value| !value.starts_with("--"))?;
        match *option {
            "--model" if model.is_none() => model = Some(*value),
            "--url" if url.is_none() => url = Some(*value),
            _ => return None,
        }
    }
    Some((provider, model, url))
}

fn status() -> Result<()> {
    let directory = config_dir()?;
    println!("réglages : {}", directory.join(BRAIN_FILE).display());
    let Some(setup) = load(&directory)? else {
        println!("Capri n'a pas de cerveau : elle répète ce qu'on lui dit, et rien ne sort de ta machine.");
        println!("Pour lui en donner un : capri chat use ollama");
        return Ok(());
    };
    let (brain, endpoint) = (&setup.brain, setup.brain.endpoint());
    println!("cerveau  : {}, modèle {}", brain.provider().name(), brain.model());
    println!("adresse  : {}", endpoint.url(""));
    let persona_file = directory.join(PERSONA_FILE);
    match custom_persona(&directory) {
        Ok(Some(_)) => println!("persona  : {}", persona_file.display()),
        Ok(None) => println!(
            "persona  : par défaut (pour la changer, écris la tienne dans {})",
            persona_file.display()
        ),
        Err(err) => println!("persona  : par défaut, car {err:#}"),
    }
    match Client::new().models(brain.provider(), endpoint, None) {
        Ok(models) if has_model(&models, brain.model()) => {
            println!("serveur  : répond, et le modèle est là");
        }
        Ok(models) => {
            println!("serveur  : répond, MAIS n'a pas le modèle « {} »", brain.model());
            println!("           {}", installed(&models));
            println!("           {}", how_to_get(brain.provider(), brain.model()));
        }
        Err(problem) => {
            println!("serveur  : {}", problem.explain(brain.provider(), endpoint));
            println!("           {}", how_to_start(brain.provider()));
        }
    }
    Ok(())
}

/// Choisit un cerveau, après avoir vérifié auprès du serveur que le modèle existe.
fn choose(provider: Provider, model: Option<&str>, url: Option<&str>) -> Result<()> {
    let url = url.or(provider.default_url()).context("il faut donner son adresse : --url <adresse>")?;
    let endpoint = Endpoint::parse(url.trim()).context("adresse refusée")?;
    let models = Client::new()
        .models(provider, &endpoint, None)
        .map_err(|problem| anyhow!("{}\n{}", problem.explain(provider, &endpoint), how_to_start(provider)))?;
    let model = match model {
        Some(model) if has_model(&models, model) => model,
        Some(model) => bail!(
            "{} n'a pas le modèle « {model} ».\n{}\n{}",
            provider.name(),
            installed(&models),
            how_to_get(provider, model)
        ),
        None => match models.as_slice() {
            [only] => only.as_str(),
            [] => bail!("{}\n{}", installed(&models), how_to_get(provider, "<nom>")),
            _ => bail!(
                "{}\nChoisis-en un : capri chat use {} --model <nom>",
                installed(&models),
                provider.id()
            ),
        },
    };
    let brain = Brain::new(provider, Some(&endpoint.url("")), model)?;
    let file = save(&config_dir()?, &brain)?;
    println!(
        "Capri pense maintenant avec {} ({}, {}).",
        brain.model(),
        provider.name(),
        endpoint.display_host()
    );
    println!("réglages : {}", file.display());
    println!(
        "Rien à relancer : c'est pris en compte dès ta prochaine phrase. Pour essayer : capri chat test"
    );
    Ok(())
}

/// Pose une question au cerveau choisi et écrit sa réponse au fil de l'eau.
fn test(message: &str) -> Result<()> {
    let directory = config_dir()?;
    let setup = load(&directory)?
        .context("Capri n'a pas de cerveau. Pour lui en donner un : capri chat use ollama")?;
    let message = if message.trim().is_empty() { TEST_QUESTION } else { message.trim() };
    let turns = [Turn { role: Role::User, text: message.to_owned() }];
    println!("> {message}");

    let started = Instant::now();
    let mut first_word = None;
    let mut failure = None;
    // le même nettoyage que pour sa bulle : on voit ici ce qu'elle dirait
    let mut plain = PlainStream::new();
    Client::new().answer(&setup.brain, None, &setup.persona, &turns, &AtomicBool::new(false), &mut |event| {
        plain.filter(event, &mut |event| match event {
            ReplyEvent::Delta(text) => {
                first_word.get_or_insert_with(|| started.elapsed());
                print!("{text}");
                let _ = std::io::stdout().flush();
            }
            ReplyEvent::Done => {}
            ReplyEvent::Failed(reason) => failure = Some(reason),
        });
    });
    if first_word.is_some() {
        println!();
    }
    if let Some(reason) = failure {
        bail!("{reason}");
    }
    eprintln!(
        "({}, modèle {} : premier mot en {:.1} s, réponse en {:.1} s)",
        setup.brain.provider().name(),
        setup.brain.model(),
        first_word.unwrap_or_default().as_secs_f32(),
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

fn off() -> Result<()> {
    if forget(&config_dir()?)? {
        println!("Capri n'a plus de cerveau : elle répète ce qu'on lui dit, et rien ne sort de ta machine.");
    } else {
        println!("Capri n'avait pas de cerveau.");
    }
    Ok(())
}

/// Le serveur a-t-il ce modèle ? Un nom sans étiquette désigne `:latest`, comme pour Ollama.
fn has_model(models: &[String], model: &str) -> bool {
    models.iter().any(|known| known == model || (!model.contains(':') && *known == format!("{model}:latest")))
}

fn installed(models: &[String]) -> String {
    if models.is_empty() {
        "Aucun modèle n'est installé.".to_owned()
    } else {
        format!("Modèles installés : {}", models.join(", "))
    }
}

fn how_to_get(provider: Provider, model: &str) -> String {
    match provider {
        Provider::Ollama => format!("Pour le télécharger : ollama pull {model}"),
        Provider::LmStudio => {
            "Pour en installer un : dans LM Studio, onglet de recherche des modèles".to_owned()
        }
        _ => format!("Choisis-en un que {} propose.", provider.name()),
    }
}

fn how_to_start(provider: Provider) -> &'static str {
    match provider {
        Provider::Ollama => {
            "Pour le lancer : ollama serve (ou, pour qu'il démarre tout seul : brew services start ollama)"
        }
        Provider::LmStudio => "Pour le lancer : dans LM Studio, démarre son serveur local.",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un dossier vide, propre à ce test.
    fn scratch_dir(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("capri-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("dossier de test");
        directory
    }

    fn brain(model: &str) -> Brain {
        Brain::new(Provider::Ollama, None, model).expect("valide")
    }

    #[test]
    fn a_brain_is_saved_read_back_and_forgotten() {
        let directory = scratch_dir("cerveau").join("pas").join("encore");
        assert_eq!(load(&directory).expect("lisible"), None, "pas de dossier : pas de cerveau");
        assert!(!forget(&directory).expect("rien à retirer"));

        let file = save(&directory, &brain("premier:8b")).expect("écrit");
        assert_eq!(file, directory.join("chat.json"));
        save(&directory, &brain("second:4b")).expect("remplacé");
        let setup = load(&directory).expect("lisible").expect("un cerveau");
        assert_eq!(setup.brain, brain("second:4b"));
        assert_eq!(setup.persona, persona::DEFAULT);
        let leftovers: Vec<_> =
            std::fs::read_dir(&directory).expect("dossier").flatten().map(|e| e.file_name()).collect();
        assert_eq!(leftovers, ["chat.json"], "pas de fichier temporaire oublié");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&directory).expect("dossier").permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "le dossier n'est ouvert qu'à l'utilisateur");
        }

        assert!(forget(&directory).expect("retiré"));
        assert_eq!(load(&directory).expect("lisible"), None);
        let _ = std::fs::remove_dir_all(scratch_dir("cerveau"));
    }

    #[test]
    fn a_broken_settings_file_is_an_error_not_a_silent_fallback() {
        let directory = scratch_dir("casse");
        std::fs::write(directory.join("chat.json"), "{ pas du json").expect("écrit");
        let error = format!("{:#}", load(&directory).expect_err("illisible"));
        assert!(error.contains("réglages illisibles") && error.contains("chat.json"), "{error}");

        // une adresse modifiée à la main vers Internet en clair est refusée à la lecture
        let unsafe_settings = r#"{"provider":"ollama","base_url":"http://example.com/v1","model":"m"}"#;
        std::fs::write(directory.join("chat.json"), unsafe_settings).expect("écrit");
        let error = format!("{:#}", load(&directory).expect_err("refusé"));
        assert!(error.contains("en clair"), "{error}");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_persona_file_replaces_the_default_one_when_it_says_something() {
        let directory = scratch_dir("persona");
        save(&directory, &brain("m")).expect("écrit");
        let persona_of = || load(&directory).expect("lisible").expect("un cerveau").persona;

        std::fs::write(directory.join("persona.md"), "\n  Tu es Capri, et tu rimes.\n").expect("écrit");
        assert_eq!(persona_of(), "Tu es Capri, et tu rimes.");
        std::fs::write(directory.join("persona.md"), "  \n").expect("écrit");
        assert_eq!(persona_of(), persona::DEFAULT, "un fichier vide ne compte pas");
        // trop longue ou illisible : elle garde sa personnalité par défaut plutôt que de se taire
        std::fs::write(directory.join("persona.md"), "x".repeat(persona::MAX_BYTES + 1)).expect("écrit");
        assert_eq!(persona_of(), persona::DEFAULT);
        std::fs::write(directory.join("persona.md"), [0xFF, 0xFE, 0x00]).expect("écrit");
        assert_eq!(persona_of(), persona::DEFAULT);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn reads_the_options_of_capri_chat_use() {
        assert_eq!(parse_use("ollama", &[]), Some((Provider::Ollama, None, None)));
        assert_eq!(
            parse_use("ollama", &["--url", "http://10.0.0.5:11434/v1", "--model", "m:8b"]),
            Some((Provider::Ollama, Some("m:8b"), Some("http://10.0.0.5:11434/v1")))
        );
        for wrong in [
            &["--model"][..],
            &["--model", "--url"],
            &["--model", "a", "--model", "b"],
            &["--autre", "x"],
            &["m:8b"],
        ] {
            assert_eq!(parse_use("ollama", wrong), None, "{wrong:?}");
        }
        assert_eq!(parse_use("skynet", &[]), None);
    }

    #[test]
    fn a_model_without_a_tag_means_latest() {
        let models = ["un:8b".to_owned(), "deux:latest".to_owned()];
        assert!(has_model(&models, "un:8b"));
        assert!(has_model(&models, "deux") && has_model(&models, "deux:latest"));
        assert!(!has_model(&models, "un") && !has_model(&models, "un:4b") && !has_model(&models, "trois"));
    }
}
