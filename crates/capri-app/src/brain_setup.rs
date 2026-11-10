//! Le cerveau de Capri côté fichiers, et la commande `capri chat …`.
//!
//! Deux fichiers dans le dossier de Capri : `chat.json` dit quel cerveau utiliser (son contenu est
//! décidé et vérifié par `capri_chat::brain`), et `persona.md`, facultatif, remplace sa
//! personnalité. Sans `chat.json`, Capri n'envoie rien nulle part : elle répète ce qu'on lui dit.
//!
//! La clé d'un fournisseur n'est jamais dans ces fichiers : elle vit dans le trousseau du système
//! (`keys`), et aucune commande ne l'affiche, ne la prend en argument ni ne l'écrit ailleurs.
//!
//! Rien n'est gardé en mémoire : les fichiers sont relus à chaque message, si bien qu'un
//! changement fait en ligne de commande vaut dès la phrase suivante, sans relancer Capri.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use capri_chat::plain::PlainStream;
use capri_chat::{
    ApiKey, Brain, Endpoint, KeyError, KeyNeed, Provider, Reach, ReplyEvent, Role, Turn, persona,
};
use capri_net::{Client, Problem};

use crate::keys::{self, KeyStore, SystemKeychain};

const BRAIN_FILE: &str = "chat.json";
const PERSONA_FILE: &str = "persona.md";
/// La question de `capri chat test` quand on n'en donne pas.
const TEST_QUESTION: &str = "Présente-toi en une phrase.";

/// Le nombre de modèles montrés au plus par `capri chat models`.
const MAX_LISTED: usize = 30;

const USAGE: &str = "\
usage : capri chat status                  quel cerveau Capri utilise, et où part ce qu'on lui dit
        capri chat use <fournisseur> [--model <nom>] [--url <adresse>]
                                           lui choisir un cerveau
        capri chat key <fournisseur>       lui donner la clé d'API d'un fournisseur (tapée sans
                                           s'afficher, ou lue sur l'entrée standard)
        capri chat key <fournisseur> --forget
                                           retirer cette clé du trousseau
        capri chat models <fournisseur> [<mot>] [--url <adresse>]
                                           chercher un modèle chez un fournisseur
        capri chat test [<message>]        lui parler depuis le terminal
        capri chat off                     lui retirer son cerveau (les clés restent)

fournisseurs : ollama, lmstudio      sur ta machine, sans compte
               openrouter, deepseek, proxyapi
                                     sur Internet, avec une clé d'API
               custom                un autre serveur compatible, avec --url (clé facultative)";

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
    let directory = match config_dir() {
        Ok(directory) => directory,
        Err(err) => {
            eprintln!("capri : {err:#}");
            return 1;
        }
    };
    let keys = SystemKeychain::from_env();
    let mut read_key = keys::read_from_user;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut cli =
        Cli { directory, keys: &keys, client: Client::new(), read_key: &mut read_key, out: &mut out };
    match cli.run(&args) {
        None => {
            eprintln!("{USAGE}");
            2
        }
        Some(Ok(())) => 0,
        Some(Err(err)) => {
            eprintln!("capri : {err:#}");
            1
        }
    }
}

/// Lit `<fournisseur> [--model <nom>] [--url <adresse>]`.
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

/// Lit `<fournisseur> [<mot>] [--url <adresse>]`.
fn parse_models<'a>(
    provider: &str,
    options: &[&'a str],
) -> Option<(Provider, Option<&'a str>, Option<&'a str>)> {
    let provider = Provider::from_id(provider)?;
    let (mut word, mut url) = (None, None);
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match *option {
            "--url" if url.is_none() => url = Some(*options.next().filter(|value| !value.starts_with("--"))?),
            word_given if !word_given.starts_with("--") && word.is_none() => word = Some(word_given),
            _ => return None,
        }
    }
    Some((provider, word, url))
}

/// Ce dont les commandes ont besoin : où sont les réglages et les clés, d'où vient une clé que
/// l'on tape, où écrire. Les tests y mettent un dossier temporaire, un trousseau en mémoire et
/// une saisie simulée.
struct Cli<'a> {
    directory: PathBuf,
    keys: &'a dyn KeyStore,
    client: Client,
    /// Lit la clé que donne l'utilisateur ; reçoit l'invite à afficher.
    read_key: &'a mut dyn FnMut(&str) -> Result<String>,
    out: &'a mut dyn Write,
}

impl Cli<'_> {
    /// `None` : ce n'est pas une commande connue.
    fn run(&mut self, args: &[&str]) -> Option<Result<()>> {
        Some(match args {
            ["status"] => self.status(),
            ["use", provider, options @ ..] => {
                let (provider, model, url) = parse_use(provider, options)?;
                self.choose(provider, model, url)
            }
            ["key", provider] => self.key(Provider::from_id(provider)?, false),
            ["key", provider, "--forget"] => self.key(Provider::from_id(provider)?, true),
            ["models", provider, options @ ..] => {
                let (provider, word, url) = parse_models(provider, options)?;
                self.models(provider, word, url)
            }
            ["test", words @ ..] => self.test(&words.join(" ")),
            ["off"] => self.off(),
            _ => return None,
        })
    }

    fn status(&mut self) -> Result<()> {
        writeln!(self.out, "réglages : {}", self.directory.join(BRAIN_FILE).display())?;
        let Some(setup) = load(&self.directory)? else {
            writeln!(
                self.out,
                "Capri n'a pas de cerveau : elle répète ce qu'on lui dit, et rien ne sort de ta machine."
            )?;
            writeln!(
                self.out,
                "Pour lui en donner un : capri chat use ollama (ou un autre fournisseur : capri chat)"
            )?;
            let stored: Vec<&str> = Provider::ALL
                .into_iter()
                .filter(|&provider| provider.key_need() != KeyNeed::None && self.keys.available())
                .filter(|&provider| self.keys.contains(provider).unwrap_or(false))
                .map(Provider::id)
                .collect();
            if !stored.is_empty() {
                writeln!(
                    self.out,
                    "clés     : rangées dans {} pour {}",
                    self.keys.name(),
                    stored.join(", ")
                )?;
            }
            return Ok(());
        };
        let (brain, endpoint) = (&setup.brain, setup.brain.endpoint());
        let provider = brain.provider();
        writeln!(self.out, "cerveau  : {}, modèle {}", provider.name(), brain.model())?;
        writeln!(self.out, "adresse  : {}", endpoint.url(""))?;
        writeln!(self.out, "envoi    : {}", destination(provider, endpoint))?;
        writeln!(self.out, "clé      : {}", self.key_status(provider, endpoint))?;
        let persona_file = self.directory.join(PERSONA_FILE);
        match custom_persona(&self.directory) {
            Ok(Some(_)) => writeln!(self.out, "persona  : {}", persona_file.display())?,
            Ok(None) => writeln!(
                self.out,
                "persona  : par défaut (pour la changer, écris la tienne dans {})",
                persona_file.display()
            )?,
            Err(err) => writeln!(self.out, "persona  : par défaut, car {err:#}")?,
        }
        let key = match keys::key_for(self.keys, provider) {
            Ok(key) => key,
            Err(err) => {
                writeln!(self.out, "serveur  : pas interrogé, faute de pouvoir lire sa clé ({err:#})")?;
                return Ok(());
            }
        };
        match self.client.models(provider, endpoint, key.as_ref()) {
            Ok(models) if has_model(&models, brain.model()) => {
                writeln!(self.out, "serveur  : répond, et le modèle est là")?;
            }
            Ok(models) => {
                writeln!(self.out, "serveur  : répond, MAIS ne propose pas le modèle « {} »", brain.model())?;
                for line in missing_model(provider, endpoint, &models, brain.model()) {
                    writeln!(self.out, "           {line}")?;
                }
            }
            Err(problem) => {
                writeln!(self.out, "serveur  : {}", problem.explain(provider, endpoint))?;
                if let Some(hint) = how_to_fix(provider, &problem) {
                    writeln!(self.out, "           {hint}")?;
                }
            }
        }
        Ok(())
    }

    /// Ce que `status` dit de la clé, sans jamais la montrer ni la lire.
    fn key_status(&self, provider: Provider, endpoint: &Endpoint) -> String {
        let (name, id) = (provider.name(), provider.id());
        if provider.key_need() == KeyNeed::None {
            return format!("aucune, {name} n'en prend pas");
        }
        if !self.keys.available() {
            return "pas de trousseau sur ce système, donc aucune".to_owned();
        }
        match (self.keys.contains(provider), provider.key_need()) {
            (Ok(true), _) if !endpoint.may_carry_secret() => format!(
                "rangée dans {}, MAIS elle ne partira pas en clair vers {}",
                self.keys.name(),
                endpoint.display_host()
            ),
            (Ok(true), _) => format!("rangée dans {} (elle ne s'affiche jamais)", self.keys.name()),
            (Ok(false), KeyNeed::Required) => {
                format!("AUCUNE : {name} en demande une. Pour la donner : capri chat key {id}")
            }
            (Ok(false), _) => format!("aucune (s'il en demande une : capri chat key {id})"),
            (Err(err), _) => format!("{err:#}"),
        }
    }

    /// Choisit un cerveau, après avoir vérifié auprès du serveur que le modèle existe.
    fn choose(&mut self, provider: Provider, model: Option<&str>, url: Option<&str>) -> Result<()> {
        let endpoint = endpoint_for(provider, url)?;
        let key = key_for_server(self.keys, provider, &endpoint)?;
        let models = self.client.models(provider, &endpoint, key.as_ref()).map_err(|problem| {
            let hint = how_to_fix(provider, &problem).map(|hint| format!("\n{hint}")).unwrap_or_default();
            anyhow!("{}{hint}", problem.explain(provider, &endpoint))
        })?;
        let model = pick_model(provider, &endpoint, &models, model)?;
        let brain = Brain::new(provider, Some(&endpoint.url("")), &model)?;
        let file = save(&self.directory, &brain)?;
        writeln!(
            self.out,
            "Capri pense maintenant avec {} ({}, {}).",
            brain.model(),
            provider.name(),
            endpoint.display_host()
        )?;
        writeln!(self.out, "{}", destination(provider, &endpoint))?;
        if key.is_some() {
            writeln!(
                self.out,
                "Sa clé est lue dans {} à chaque message, et n'est écrite nulle part ailleurs.",
                self.keys.name()
            )?;
        }
        writeln!(self.out, "réglages : {}", file.display())?;
        writeln!(
            self.out,
            "Rien à relancer : c'est pris en compte dès ta prochaine phrase. Pour essayer : capri chat test"
        )?;
        Ok(())
    }

    /// Range la clé d'un fournisseur dans le trousseau, ou l'en retire.
    fn key(&mut self, provider: Provider, forget: bool) -> Result<()> {
        let of = of_provider(provider);
        if provider.key_need() == KeyNeed::None {
            bail!("{} ne prend pas de clé : il n'y a rien à ranger.", provider.name());
        }
        if !self.keys.available() {
            bail!(
                "pas de trousseau sur ce système : Capri ne sait pas y ranger de clé, et n'en écrit jamais ailleurs"
            );
        }
        if forget {
            if self.keys.forget(provider)? {
                writeln!(self.out, "Clé {of} retirée : elle n'est plus dans {}.", self.keys.name())?;
            } else {
                writeln!(self.out, "Il n'y avait pas de clé {of} dans {}.", self.keys.name())?;
            }
            return Ok(());
        }
        let prompt = format!("Clé d'API {of} (elle ne s'affiche pas pendant la frappe) : ");
        let key = match ApiKey::parse(&(self.read_key)(&prompt)?) {
            Ok(key) => key,
            Err(KeyError::Empty) => bail!("aucune clé lue : rien n'est changé"),
            Err(err) => bail!("ce n'est pas une clé ({err}) : rien n'est changé"),
        };
        self.keys.save(provider, &key)?;
        writeln!(
            self.out,
            "Clé {of} rangée dans {}. Elle n'est écrite nulle part ailleurs.",
            self.keys.name()
        )?;
        match load(&self.directory) {
            Ok(Some(setup)) if setup.brain.provider() == provider => {
                if setup.brain.endpoint().may_carry_secret() {
                    writeln!(
                        self.out,
                        "Capri s'en sert dès ta prochaine phrase. Pour essayer : capri chat test"
                    )?;
                } else {
                    writeln!(
                        self.out,
                        "MAIS elle ne partira pas en clair vers {} : il faut une adresse en https.",
                        setup.brain.endpoint().display_host()
                    )?;
                }
            }
            _ => {
                let url = if provider.default_url().is_none() { " --url <adresse>" } else { "" };
                writeln!(
                    self.out,
                    "Pour qu'elle s'en serve : capri chat use {}{url} --model <nom>",
                    provider.id()
                )?;
            }
        }
        Ok(())
    }

    /// Cherche, parmi les modèles d'un fournisseur, ceux dont le nom contient `word`.
    fn models(&mut self, provider: Provider, word: Option<&str>, url: Option<&str>) -> Result<()> {
        let endpoint = endpoint_for(provider, url)?;
        let key = key_for_server(self.keys, provider, &endpoint)?;
        let models = self
            .client
            .models(provider, &endpoint, key.as_ref())
            .map_err(|problem| anyhow!("{}", problem.explain(provider, &endpoint)))?;
        let word = word.map(str::to_lowercase);
        let mut found: Vec<&String> = printable(&models)
            .filter(|model| word.as_deref().is_none_or(|word| model.to_lowercase().contains(word)))
            .collect();
        found.sort();
        let (name, id) = (provider.name(), provider.id());
        if found.is_empty() {
            match &word {
                Some(word) => writeln!(
                    self.out,
                    "Aucun des {} modèles de {name} ne contient « {word} ».",
                    models.len()
                )?,
                None => writeln!(self.out, "{name} ne propose aucun modèle.")?,
            }
            return Ok(());
        }
        for model in found.iter().take(MAX_LISTED) {
            writeln!(self.out, "{model}")?;
        }
        if found.len() > MAX_LISTED {
            writeln!(
                self.out,
                "… et {} autres : précise ta recherche (capri chat models {id} <mot>).",
                found.len() - MAX_LISTED
            )?;
        }
        let url = url.map(|url| format!(" --url {url}")).unwrap_or_default();
        writeln!(self.out, "Pour en choisir un : capri chat use {id} --model <nom>{url}")?;
        Ok(())
    }

    /// Pose une question au cerveau choisi et écrit sa réponse au fil de l'eau.
    fn test(&mut self, message: &str) -> Result<()> {
        let setup = load(&self.directory)?
            .context("Capri n'a pas de cerveau. Pour lui en donner un : capri chat use ollama")?;
        let provider = setup.brain.provider();
        let key = keys::key_for(self.keys, provider)
            .with_context(|| format!("pas de réponse : la clé {} est illisible", of_provider(provider)))?;
        let message = if message.trim().is_empty() { TEST_QUESTION } else { message.trim() };
        let turns = [Turn { role: Role::User, text: message.to_owned() }];
        writeln!(self.out, "> {message}")?;

        let started = Instant::now();
        let mut first_word = None;
        let mut failure = None;
        // le même nettoyage que pour sa bulle : on voit ici ce qu'elle dirait
        let mut plain = PlainStream::new();
        let Self { client, out, .. } = self;
        client.answer(
            &setup.brain,
            key.as_ref(),
            &setup.persona,
            &turns,
            &AtomicBool::new(false),
            &mut |event| {
                plain.filter(event, &mut |event| match event {
                    ReplyEvent::Delta(text) => {
                        first_word.get_or_insert_with(|| started.elapsed());
                        let _ = write!(out, "{text}");
                        let _ = out.flush();
                    }
                    ReplyEvent::Done => {}
                    ReplyEvent::Failed(reason) => failure = Some(reason),
                });
            },
        );
        if first_word.is_some() {
            writeln!(self.out)?;
        }
        if let Some(reason) = failure {
            bail!("{reason}");
        }
        eprintln!(
            "({}, modèle {} : premier mot en {:.1} s, réponse en {:.1} s)",
            provider.name(),
            setup.brain.model(),
            first_word.unwrap_or_default().as_secs_f32(),
            started.elapsed().as_secs_f32()
        );
        Ok(())
    }

    fn off(&mut self) -> Result<()> {
        if forget(&self.directory)? {
            writeln!(
                self.out,
                "Capri n'a plus de cerveau : elle répète ce qu'on lui dit, et rien ne sort de ta machine."
            )?;
        } else {
            writeln!(self.out, "Capri n'avait pas de cerveau.")?;
        }
        if self.keys.available() {
            writeln!(
                self.out,
                "Les clés d'API restent dans {} ; pour en retirer une : capri chat key <fournisseur> --forget",
                self.keys.name()
            )?;
        }
        Ok(())
    }
}

/// L'adresse à joindre : celle donnée par `--url`, ou celle du fournisseur.
fn endpoint_for(provider: Provider, url: Option<&str>) -> Result<Endpoint> {
    let url = url.or(provider.default_url()).with_context(|| {
        format!("{} n'a pas d'adresse habituelle : donne-la avec --url <adresse>", provider.name())
    })?;
    Endpoint::parse(url.trim()).context("adresse refusée")
}

/// La clé à envoyer à ce serveur, après avoir vérifié qu'il en faut une et qu'elle a le droit d'y
/// aller : jamais en clair sur le réseau. Rien n'est contacté ici.
fn key_for_server(keys: &dyn KeyStore, provider: Provider, endpoint: &Endpoint) -> Result<Option<ApiKey>> {
    let key = keys::key_for(keys, provider)
        .with_context(|| format!("la clé {} est illisible", of_provider(provider)))?;
    match (provider.key_need(), &key) {
        (KeyNeed::Required, None) => {
            bail!("{} demande une clé. Range-la d'abord : capri chat key {}", provider.name(), provider.id())
        }
        (_, Some(_)) if !endpoint.may_carry_secret() => bail!(
            "la clé {} ne partirait pas en clair vers {} : donne une adresse en https, ou retire la clé (capri chat key {} --forget)",
            of_provider(provider),
            endpoint.display_host(),
            provider.id()
        ),
        _ => Ok(key),
    }
}

/// Le modèle à utiliser, d'après la liste du serveur. Pour un serveur sur Internet, il faut le
/// nommer : on ne choisit pas au hasard parmi des centaines, et on ne les affiche pas toutes.
fn pick_model(
    provider: Provider,
    endpoint: &Endpoint,
    models: &[String],
    model: Option<&str>,
) -> Result<String> {
    let (name, id) = (provider.name(), provider.id());
    let remote = endpoint.reach() == Reach::Internet;
    match model {
        Some(model) if has_model(models, model) => Ok(model.to_owned()),
        Some(model) if remote => bail!(
            "{name} ne propose pas « {model} » ({} modèles en tout).\nPour chercher un nom : {}",
            models.len(),
            search_command(provider, endpoint)
        ),
        Some(model) => {
            bail!(
                "{name} n'a pas le modèle « {model} ».\n{}\n{}",
                installed(models),
                how_to_get(provider, model)
            )
        }
        None if remote => bail!(
            "{name} propose {} modèles : choisis-en un avec --model <nom>.\nPour chercher un nom : {}",
            models.len(),
            search_command(provider, endpoint)
        ),
        None => match models {
            [only] => Ok(only.clone()),
            [] => bail!("{}\n{}", installed(models), how_to_get(provider, "<nom>")),
            _ => bail!("{}\nChoisis-en un : capri chat use {id} --model <nom>", installed(models)),
        },
    }
}

/// La commande qui cherche un modèle chez ce fournisseur, à cette adresse.
fn search_command(provider: Provider, endpoint: &Endpoint) -> String {
    let url = match provider.default_url() {
        Some(url) if Endpoint::parse(url).as_ref() == Ok(endpoint) => String::new(),
        _ => format!(" --url {}", endpoint.url("")),
    };
    format!("capri chat models {} <mot>{url}", provider.id())
}

/// Où part ce qu'on dit à Capri avec ce cerveau, en une phrase.
fn destination(provider: Provider, endpoint: &Endpoint) -> String {
    let host = endpoint.display_host();
    let how = if endpoint.secure { "chiffré (https)" } else { "en clair (http)" };
    match endpoint.reach() {
        Reach::ThisMachine => "Ce que tu lui dis reste sur cette machine.".to_owned(),
        Reach::PrivateNetwork => format!("Ce que tu lui dis part vers {host}, sur ton réseau privé, {how}."),
        Reach::Internet => {
            let whom = match provider {
                Provider::Custom => host,
                _ => format!("{} ({host})", provider.name()),
            };
            format!(
                "Attention : ce que tu lui dis partira chez {whom}, sur Internet, à chaque message, avec sa \
                 personnalité. Rien d'autre de ta machine : ni ce que fait Claude Code, ni tes fichiers."
            )
        }
    }
}

/// « d'OpenRouter », « de DeepSeek », « du serveur personnalisé ».
fn of_provider(provider: Provider) -> String {
    match provider {
        Provider::Custom => "du serveur personnalisé".to_owned(),
        _ if provider.name().starts_with(['A', 'E', 'I', 'O', 'U']) => format!("d'{}", provider.name()),
        _ => format!("de {}", provider.name()),
    }
}

/// Le serveur a-t-il ce modèle ? Un nom sans étiquette désigne `:latest`, comme pour Ollama.
fn has_model(models: &[String], model: &str) -> bool {
    models.iter().any(|known| known == model || (!model.contains(':') && *known == format!("{model}:latest")))
}

/// Les noms que l'on peut afficher sans risque : un nom venu du serveur qui contiendrait des
/// caractères de contrôle (de quoi piloter le terminal) est laissé de côté.
fn printable(models: &[String]) -> impl Iterator<Item = &String> {
    models.iter().filter(|model| !model.chars().any(char::is_control))
}

fn installed(models: &[String]) -> String {
    let models: Vec<&str> = printable(models).map(String::as_str).collect();
    if models.is_empty() {
        "Aucun modèle n'est installé.".to_owned()
    } else {
        format!("Modèles installés : {}", models.join(", "))
    }
}

/// Ce qu'il faut dire quand le serveur n'a pas le modèle choisi.
fn missing_model(provider: Provider, endpoint: &Endpoint, models: &[String], model: &str) -> Vec<String> {
    if endpoint.reach() == Reach::Internet {
        vec![format!(
            "{} modèles proposés. Pour chercher un nom : {}",
            models.len(),
            search_command(provider, endpoint)
        )]
    } else {
        vec![installed(models), how_to_get(provider, model)]
    }
}

fn how_to_get(provider: Provider, model: &str) -> String {
    match provider {
        Provider::Ollama => format!("Pour le télécharger : ollama pull {model}"),
        Provider::LmStudio => {
            "Pour en installer un : dans LM Studio, cherche-le parmi ses modèles.".to_owned()
        }
        _ => format!("Choisis-en un que {} propose.", provider.name()),
    }
}

/// Que faire après ce problème, quand il y a quelque chose à faire de notre côté.
fn how_to_fix(provider: Provider, problem: &Problem) -> Option<&'static str> {
    match (provider, problem) {
        (Provider::Ollama, Problem::Unreachable) => Some(
            "Pour le lancer : ollama serve (ou, pour qu'il démarre tout seul : brew services start ollama)",
        ),
        (Provider::LmStudio, Problem::Unreachable) => {
            Some("Pour le lancer : dans LM Studio, démarre son serveur local.")
        }
        _ => None,
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

    // --- La ligne de commande, avec un trousseau en mémoire et un faux serveur local ---

    use crate::fake_server::{self, header};
    use crate::keys::memory::MemoryKeys;

    /// Une clé qui n'existe nulle part ailleurs que dans ces tests.
    const KEY: &str = "sk-test-capri-cli-0123456789abcdef";

    /// Ce qu'une commande a donné.
    struct Ran {
        /// Son message d'erreur, tel que `capri` l'afficherait.
        error: Option<String>,
        out: String,
        /// La clé a-t-elle été demandée à l'utilisateur ?
        asked: bool,
    }

    /// Lance `capri chat <args>` sur ce dossier et ce trousseau ; `typed` est ce que l'utilisateur
    /// taperait si on lui demandait sa clé.
    fn run(directory: &Path, keys: &dyn KeyStore, typed: Option<&str>, args: &[&str]) -> Ran {
        let mut out = Vec::new();
        let mut asked = false;
        let outcome = {
            let mut read_key = |_prompt: &str| -> Result<String> {
                asked = true;
                typed.map(str::to_owned).context("aucune saisie prévue")
            };
            let mut cli = Cli {
                directory: directory.to_owned(),
                keys,
                client: Client::new(),
                read_key: &mut read_key,
                out: &mut out,
            };
            cli.run(args).expect("une commande connue")
        };
        Ran {
            error: outcome.err().map(|err| format!("{err:#}")),
            out: String::from_utf8(out).expect("du texte"),
            asked,
        }
    }

    /// Un fichier du dossier contient-il ce texte ?
    fn any_file_contains(directory: &Path, needle: &str) -> bool {
        std::fs::read_dir(directory)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    std::fs::read(entry.path())
                        .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(needle))
                })
            })
            .unwrap_or(false)
    }

    fn stored(keys: &MemoryKeys, provider: Provider) -> Option<String> {
        keys.load(provider).expect("lisible").map(|key| key.reveal_for_keychain().to_owned())
    }

    fn at(url: &str) -> Endpoint {
        Endpoint::parse(url).expect("adresse valable")
    }

    #[test]
    fn a_key_is_typed_stored_and_forgotten_without_ever_being_shown() {
        let directory = scratch_dir("cle-rangee");
        let keys = MemoryKeys::default();
        let ran = run(&directory, &keys, Some(&format!("  {KEY}\n")), &["key", "openrouter"]);
        assert_eq!(ran.error, None);
        assert!(ran.asked);
        assert!(ran.out.contains("Clé d'OpenRouter rangée dans le trousseau de test."), "{}", ran.out);
        assert!(
            ran.out.contains("Pour qu'elle s'en serve : capri chat use openrouter --model <nom>"),
            "{}",
            ran.out
        );
        assert_eq!(stored(&keys, Provider::OpenRouter).as_deref(), Some(KEY), "sans les blancs autour");
        assert!(!ran.out.contains(KEY));
        assert!(!any_file_contains(&directory, KEY), "rien n'est écrit dans le dossier des réglages");

        let ran = run(&directory, &keys, Some("sk-une-cle-de-serveur"), &["key", "custom"]);
        assert!(
            ran.out
                .ends_with("Pour qu'elle s'en serve : capri chat use custom --url <adresse> --model <nom>\n"),
            "{}",
            ran.out
        );

        let ran = run(&directory, &keys, Some("sk-une-autre-cle-de-test"), &["key", "openrouter"]);
        assert_eq!(ran.error, None);
        assert_eq!(
            stored(&keys, Provider::OpenRouter).as_deref(),
            Some("sk-une-autre-cle-de-test"),
            "remplacée"
        );

        let ran = run(&directory, &keys, None, &["key", "openrouter", "--forget"]);
        assert_eq!((ran.error, ran.asked), (None, false));
        assert_eq!(ran.out, "Clé d'OpenRouter retirée : elle n'est plus dans le trousseau de test.\n");
        assert_eq!(stored(&keys, Provider::OpenRouter), None);
        let ran = run(&directory, &keys, None, &["key", "openrouter", "--forget"]);
        assert_eq!(ran.out, "Il n'y avait pas de clé d'OpenRouter dans le trousseau de test.\n");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn what_is_not_a_key_changes_nothing_and_is_never_repeated() {
        let directory = scratch_dir("pas-une-cle");
        let keys = MemoryKeys::with(Provider::DeepSeek, KEY);
        for typed in ["deux mots secrets", "clé-accentuée-secrète", "ligne\nsuivante-secrète"] {
            let ran = run(&directory, &keys, Some(typed), &["key", "deepseek"]);
            let error = ran.error.expect("refusé");
            assert!(error.contains("ce n'est pas une clé") && error.contains("rien n'est changé"), "{error}");
            assert!(
                !error.contains("secr") && !ran.out.contains("secr"),
                "ce qui a été tapé n'est pas répété : {error}"
            );
        }
        let ran = run(&directory, &keys, Some("  \n"), &["key", "deepseek"]);
        assert_eq!(ran.error.as_deref(), Some("aucune clé lue : rien n'est changé"));
        assert_eq!(stored(&keys, Provider::DeepSeek).as_deref(), Some(KEY), "l'ancienne clé est restée");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn no_key_is_asked_for_where_none_can_be_kept() {
        let directory = scratch_dir("sans-trousseau");
        let ran = run(&directory, &MemoryKeys::default(), Some(KEY), &["key", "ollama"]);
        assert_eq!(ran.error.as_deref(), Some("Ollama ne prend pas de clé : il n'y a rien à ranger."));
        assert!(!ran.asked);
        for args in [&["key", "openrouter"][..], &["key", "openrouter", "--forget"]] {
            let ran = run(&directory, &MemoryKeys::unavailable(), Some(KEY), args);
            let error = ran.error.expect("refusé");
            assert!(
                error.contains("pas de trousseau sur ce système")
                    && error.contains("n'en écrit jamais ailleurs")
            );
            assert!(!ran.asked, "on ne demande pas une clé qu'on ne pourrait pas ranger");
        }
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn choosing_a_server_with_a_key_sends_it_in_its_header_and_writes_it_nowhere() {
        let directory = scratch_dir("choix-avec-cle");
        let (address, requests) =
            fake_server::serve(vec![fake_server::listing(&["labo/un-modele", "labo/autre"])]);
        let keys = MemoryKeys::with(Provider::Custom, KEY);
        let url = format!("http://{address}/v1");
        let ran =
            run(&directory, &keys, None, &["use", "custom", "--url", &url, "--model", "labo/un-modele"]);
        assert_eq!(ran.error, None);
        assert!(
            ran.out.starts_with(&format!(
                "Capri pense maintenant avec labo/un-modele (Serveur personnalisé, {address}).\n\
                 Ce que tu lui dis reste sur cette machine.\n\
                 Sa clé est lue dans le trousseau de test à chaque message, et n'est écrite nulle part ailleurs.\n"
            )),
            "{}",
            ran.out
        );
        let (head, _) =
            requests.recv_timeout(std::time::Duration::from_secs(5)).expect("la liste des modèles");
        assert!(head.starts_with("GET /v1/models HTTP/1.1\r\n"), "{head}");
        assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));

        let settings = std::fs::read_to_string(directory.join("chat.json")).expect("réglages écrits");
        assert!(settings.contains("\"custom\"") && settings.contains(&url), "{settings}");
        assert!(
            !ran.out.contains(KEY) && !any_file_contains(&directory, KEY),
            "la clé n'est écrite nulle part"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_provider_that_needs_a_key_is_not_chosen_or_contacted_without_one() {
        let directory = scratch_dir("choix-sans-cle");
        let (address, requests) = fake_server::serve(vec![fake_server::listing(&["labo/un-modele"])]);
        let url = format!("http://{address}/api/v1");
        let ran = run(
            &directory,
            &MemoryKeys::default(),
            None,
            &["use", "openrouter", "--url", &url, "--model", "labo/un-modele"],
        );
        assert_eq!(
            ran.error.as_deref(),
            Some("OpenRouter demande une clé. Range-la d'abord : capri chat key openrouter")
        );
        assert_eq!(load(&directory).expect("lisible"), None, "aucun cerveau choisi");
        assert!(
            requests.recv_timeout(std::time::Duration::from_millis(100)).is_err(),
            "personne n'a été contacté"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_key_never_goes_in_clear_to_another_machine() {
        // vérifié avant tout contact : ce test n'ouvre aucune connexion
        let keys = MemoryKeys::with(Provider::Custom, KEY);
        for (url, host) in [
            ("http://192.168.1.20:1234/v1", "192.168.1.20:1234"),
            ("http://10.0.0.5:8080/v1", "10.0.0.5:8080"),
        ] {
            let error =
                format!("{:#}", key_for_server(&keys, Provider::Custom, &at(url)).expect_err("refusé"));
            assert_eq!(
                error,
                format!(
                    "la clé du serveur personnalisé ne partirait pas en clair vers {host} : donne une adresse en https, ou retire la clé (capri chat key custom --forget)"
                )
            );
        }
        assert!(
            key_for_server(&keys, Provider::Custom, &at("https://192.168.1.20/v1"))
                .expect("permis")
                .is_some()
        );
        assert!(
            key_for_server(&keys, Provider::Custom, &at("http://localhost:1234/v1"))
                .expect("permis")
                .is_some()
        );
        // sans clé, un serveur du réseau privé reste possible ; Ollama n'en reçoit jamais
        let none = MemoryKeys::default();
        assert!(
            key_for_server(&none, Provider::Custom, &at("http://192.168.1.20:1234/v1"))
                .expect("permis")
                .is_none()
        );
        let ollama_keys = MemoryKeys::with(Provider::Ollama, KEY);
        assert!(
            key_for_server(&ollama_keys, Provider::Ollama, &at("http://192.168.1.20:11434/v1"))
                .expect("permis")
                .is_none()
        );
    }

    #[test]
    fn a_remote_provider_needs_a_model_name_and_its_hundreds_of_models_are_not_listed() {
        let openrouter = at("https://openrouter.ai/api/v1");
        let models: Vec<String> = (0..467).map(|number| format!("labo/modele-{number}")).collect();
        let error = pick_model(Provider::OpenRouter, &openrouter, &models, None)
            .expect_err("il faut le nommer")
            .to_string();
        assert_eq!(
            error,
            "OpenRouter propose 467 modèles : choisis-en un avec --model <nom>.\nPour chercher un nom : capri chat models openrouter <mot>"
        );
        let only = vec!["labo/le-seul".to_owned()];
        assert!(
            pick_model(Provider::OpenRouter, &openrouter, &only, None).is_err(),
            "même seul, il n'est pas pris d'office"
        );
        assert_eq!(
            pick_model(Provider::OpenRouter, &openrouter, &models, Some("labo/modele-7")).expect("présent"),
            "labo/modele-7"
        );
        let error = pick_model(Provider::OpenRouter, &openrouter, &models, Some("absent"))
            .expect_err("absent")
            .to_string();
        assert!(error.starts_with("OpenRouter ne propose pas « absent » (467 modèles en tout)."), "{error}");
        assert!(!error.contains("modele-1"), "aucun nom n'est affiché : {error}");

        // sur cette machine, rien ne change : le seul modèle est pris, plusieurs sont listés
        let local = at("http://localhost:11434/v1");
        assert_eq!(pick_model(Provider::Ollama, &local, &only, None).expect("le seul"), "labo/le-seul");
        let error =
            pick_model(Provider::Ollama, &local, &models[..2], None).expect_err("à choisir").to_string();
        assert!(error.starts_with("Modèles installés : labo/modele-0, labo/modele-1"), "{error}");
    }

    #[test]
    fn choosing_a_brain_says_where_the_conversation_goes() {
        assert_eq!(
            destination(Provider::Ollama, &at("http://localhost:11434/v1")),
            "Ce que tu lui dis reste sur cette machine."
        );
        assert_eq!(
            destination(Provider::Ollama, &at("http://192.168.1.20:11434/v1")),
            "Ce que tu lui dis part vers 192.168.1.20:11434, sur ton réseau privé, en clair (http)."
        );
        assert_eq!(
            destination(Provider::OpenRouter, &at("https://openrouter.ai/api/v1")),
            "Attention : ce que tu lui dis partira chez OpenRouter (openrouter.ai:443), sur Internet, à chaque message, \
             avec sa personnalité. Rien d'autre de ta machine : ni ce que fait Claude Code, ni tes fichiers."
        );
        assert!(
            destination(Provider::Custom, &at("https://llm.example.com/v1"))
                .contains("partira chez llm.example.com:443,")
        );
        // un nom qui mène peut-être chez soi compte pour Internet : rien ne le garantit
        assert!(
            destination(Provider::Custom, &at("https://tesseract.local:8443/v1")).starts_with("Attention")
        );
    }

    #[test]
    fn status_says_whether_a_key_is_there_without_showing_it() {
        let directory = scratch_dir("statut-cle");
        let (address, requests) = fake_server::serve(vec![
            fake_server::listing(&["modele-de-test"]),
            fake_server::listing(&["modele-de-test"]),
        ]);
        let custom = Brain::new(Provider::Custom, Some(&format!("http://{address}/v1")), "modele-de-test")
            .expect("valide");
        save(&directory, &custom).expect("écrit");

        let ran = run(&directory, &MemoryKeys::with(Provider::Custom, KEY), None, &["status"]);
        assert_eq!(ran.error, None);
        for line in [
            "cerveau  : Serveur personnalisé, modèle modele-de-test\n",
            "envoi    : Ce que tu lui dis reste sur cette machine.\n",
            "clé      : rangée dans le trousseau de test (elle ne s'affiche jamais)\n",
            "serveur  : répond, et le modèle est là\n",
        ] {
            assert!(ran.out.contains(line), "{line:?} manque dans :\n{}", ran.out);
        }
        assert!(!ran.out.contains(KEY));
        let (head, _) = requests.recv_timeout(std::time::Duration::from_secs(5)).expect("une requête");
        assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));

        let ran = run(&directory, &MemoryKeys::default(), None, &["status"]);
        assert!(
            ran.out.contains("clé      : aucune (s'il en demande une : capri chat key custom)\n"),
            "{}",
            ran.out
        );
        let (head, _) = requests.recv_timeout(std::time::Duration::from_secs(5)).expect("une requête");
        assert_eq!(header(&head, "authorization"), None);

        // un fournisseur qui exige une clé, sans clé : il le dit, et ne contacte personne
        let openrouter =
            Brain::new(Provider::OpenRouter, Some(&format!("http://{address}/api/v1")), "m").expect("valide");
        save(&directory, &openrouter).expect("écrit");
        let ran = run(&directory, &MemoryKeys::default(), None, &["status"]);
        assert!(
            ran.out.contains(
                "clé      : AUCUNE : OpenRouter en demande une. Pour la donner : capri chat key openrouter\n"
            ),
            "{}",
            ran.out
        );
        assert!(ran.out.contains("serveur  : OpenRouter demande une clé, et je n'en ai pas."), "{}", ran.out);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn without_a_brain_status_lists_the_stored_keys_by_name_only() {
        let directory = scratch_dir("statut-vide");
        let keys = MemoryKeys::with(Provider::DeepSeek, KEY);
        keys.save(Provider::ProxyApi, &ApiKey::parse("sk-une-autre-cle-de-test").expect("une clé"))
            .expect("rangée");
        let ran = run(&directory, &keys, None, &["status"]);
        assert!(ran.out.contains("Capri n'a pas de cerveau"), "{}", ran.out);
        assert!(
            ran.out.ends_with("clés     : rangées dans le trousseau de test pour deepseek, proxyapi\n"),
            "{}",
            ran.out
        );
        assert!(!ran.out.contains(KEY));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn off_forgets_the_brain_but_not_the_keys_and_says_so() {
        let directory = scratch_dir("eteinte");
        save(&directory, &brain("m")).expect("écrit");
        let keys = MemoryKeys::with(Provider::OpenRouter, KEY);
        let ran = run(&directory, &keys, None, &["off"]);
        assert_eq!(ran.error, None);
        assert_eq!(
            ran.out,
            "Capri n'a plus de cerveau : elle répète ce qu'on lui dit, et rien ne sort de ta machine.\n\
             Les clés d'API restent dans le trousseau de test ; pour en retirer une : capri chat key <fournisseur> --forget\n"
        );
        assert_eq!(stored(&keys, Provider::OpenRouter).as_deref(), Some(KEY), "la clé n'est pas touchée");
        assert_eq!(load(&directory).expect("lisible"), None);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn test_talks_with_the_key_and_a_refused_key_says_how_to_put_it_back() {
        let directory = scratch_dir("essai-cle");
        let refusal = format!(r#"{{"error":"bad key {KEY}"}}"#);
        let (address, requests) = fake_server::serve(vec![
            fake_server::reply("Salut, **toi**."),
            fake_server::response("401 Unauthorized", "application/json", &refusal),
        ]);
        let custom = Brain::new(Provider::Custom, Some(&format!("http://{address}/v1")), "modele-de-test")
            .expect("valide");
        save(&directory, &custom).expect("écrit");
        let keys = MemoryKeys::with(Provider::Custom, KEY);

        let ran = run(&directory, &keys, None, &["test", "Coucou"]);
        assert_eq!(
            (ran.error, ran.out.as_str()),
            (None, "> Coucou\nSalut, toi.\n"),
            "le même texte que sa bulle"
        );
        let (head, body) = requests.recv_timeout(std::time::Duration::from_secs(5)).expect("une requête");
        assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));
        assert!(!body.contains(KEY));

        let ran = run(&directory, &keys, None, &["test"]);
        assert_eq!(
            ran.error.as_deref(),
            Some(format!("{address} refuse ma clé (401). Pour la remettre : capri chat key custom").as_str())
        );
        assert!(!ran.out.contains(KEY));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn models_are_searched_and_never_listed_by_the_hundred() {
        let directory = scratch_dir("recherche");
        let mut names: Vec<String> = (0..120).map(|number| format!("labo/modele-{number:03}")).collect();
        names.push("autre/chose".to_owned());
        names.push("piege/\u{1b}[31mrouge".to_owned());
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let (address, _requests) = fake_server::serve(vec![fake_server::listing(&names); 3]);
        let url = format!("http://{address}/v1");

        let ran = run(&directory, &MemoryKeys::default(), None, &["models", "custom", "--url", &url]);
        assert_eq!(ran.error, None);
        let lines: Vec<&str> = ran.out.lines().collect();
        assert_eq!(lines.len(), MAX_LISTED + 2, "{}", ran.out);
        assert_eq!(lines[0], "autre/chose", "dans l'ordre alphabétique");
        assert_eq!(
            lines[MAX_LISTED],
            "… et 91 autres : précise ta recherche (capri chat models custom <mot>)."
        );
        assert_eq!(
            lines[MAX_LISTED + 1],
            format!("Pour en choisir un : capri chat use custom --model <nom> --url {url}")
        );
        assert!(!ran.out.contains('\u{1b}'), "un nom qui piloterait le terminal n'est pas affiché");

        let ran =
            run(&directory, &MemoryKeys::default(), None, &["models", "custom", "CHOSE", "--url", &url]);
        assert_eq!(ran.out.lines().next(), Some("autre/chose"));
        assert_eq!(ran.out.lines().count(), 2);
        let ran = run(
            &directory,
            &MemoryKeys::default(),
            None,
            &["models", "custom", "introuvable", "--url", &url],
        );
        assert_eq!(ran.out, "Aucun des 122 modèles de Serveur personnalisé ne contient « introuvable ».\n");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn what_is_not_a_command_is_refused_with_the_usage() {
        let directory = scratch_dir("usage");
        let keys = MemoryKeys::default();
        let (mut out, mut read_key) = (Vec::new(), |_: &str| -> Result<String> { panic!("rien à lire") });
        let mut cli = Cli {
            directory: directory.clone(),
            keys: &keys,
            client: Client::new(),
            read_key: &mut read_key,
            out: &mut out,
        };
        for args in [
            &["key"][..],
            &["key", "skynet"],
            &["key", "openrouter", KEY],
            &["key", "openrouter", "--forget", "encore"],
            &["models"],
            &["models", "custom", "un", "deux"],
            &["models", "custom", "--url"],
            &["use"],
            &["use", "skynet"],
            &["frobnicate"],
        ] {
            assert!(cli.run(args).is_none(), "{args:?}");
        }
        drop(cli);
        assert!(out.is_empty());
        assert_eq!(parse_models("openrouter", &["gpt"]), Some((Provider::OpenRouter, Some("gpt"), None)));
        assert_eq!(
            parse_models("custom", &["--url", "http://localhost:8080/v1", "mot"]),
            Some((Provider::Custom, Some("mot"), Some("http://localhost:8080/v1")))
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
