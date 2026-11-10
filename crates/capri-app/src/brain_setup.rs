//! Le cerveau de Capri côté fichiers, et la commande `capri chat …`.
//!
//! Deux fichiers dans le dossier de Capri : `chat.json` dit quel cerveau utiliser (son contenu est
//! décidé et vérifié par `capri_chat::brain`), et `persona.md`, facultatif, remplace sa
//! personnalité. Sans `chat.json`, Capri n'envoie rien nulle part : elle répète ce qu'on lui dit.
//!
//! La clé d'un fournisseur n'est jamais dans ces fichiers : elle vit dans le trousseau du système
//! (`keys`), et aucune commande ne l'affiche, ne la prend en argument ni ne l'écrit ailleurs. Elle
//! y est rangée pour une adresse (`capri chat key <fournisseur> [--url <adresse>]`), et chaque
//! commande ne lit que celle de l'adresse qu'elle joint.
//!
//! Rien n'est gardé en mémoire : les fichiers sont relus à chaque message, si bien qu'un
//! changement fait en ligne de commande vaut dès la phrase suivante, sans relancer Capri.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use capri_chat::brain::{MAX_LISTED_MODELS, has_model, printable, search_models};
use capri_chat::phrases::{
    NO_BRAIN, NO_MORE_BRAIN, TEST_QUESTION, destination, how_to_get, key_forgotten, key_stored,
    legacy_key_removed, no_key_in_clear, not_a_key, of_provider, server, thinks_now_with,
};
use capri_chat::plain::PlainStream;
use capri_chat::{
    ApiKey, BoundKey, Brain, Endpoint, KeyAccount, KeyJob, KeyNeed, Place, Provider, Reach, ReplyEvent, Role,
    Turn, key, persona,
};
use capri_net::{Ask, Client, Problem};

use crate::keys::{self, KeyStore, SystemKeychain};

const BRAIN_FILE: &str = "chat.json";
const PERSONA_FILE: &str = "persona.md";

const USAGE: &str = "\
usage : capri chat status                  quel cerveau Capri utilise, et où part ce qu'on lui dit
        capri chat use <fournisseur> [--model <nom>] [--url <adresse>]
                                           lui choisir un cerveau
        capri chat key <fournisseur> [--url <adresse>]
                                           lui donner la clé d'API d'un fournisseur (tapée sans
                                           s'afficher, ou lue sur l'entrée standard), pour cette
                                           adresse seulement : par défaut, celle du cerveau choisi
                                           s'il est de ce fournisseur, sinon son adresse habituelle
        capri chat key <fournisseur> [--url <adresse>] --forget
                                           retirer cette clé du trousseau
        capri chat models <fournisseur> [<mot>] [--url <adresse>]
                                           chercher un modèle chez un fournisseur
        capri chat test [<message>]        lui parler depuis le terminal
        capri chat off                     lui retirer son cerveau (les clés restent)

fournisseurs : ollama, lmstudio      sur ta machine, sans compte
               openrouter, deepseek, proxyapi, ollama-cloud
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
pub fn load_persona(directory: &Path) -> String {
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

/// Lit `<fournisseur> [--url <adresse>] [--forget]`, dans n'importe quel ordre. Rend le
/// fournisseur, l'adresse donnée, et s'il faut retirer la clé.
fn parse_key<'a>(provider: &str, options: &[&'a str]) -> Option<(Provider, Option<&'a str>, bool)> {
    let provider = Provider::from_id(provider)?;
    let (mut url, mut forget) = (None, false);
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match *option {
            "--url" if url.is_none() => url = Some(*options.next().filter(|value| !value.starts_with("--"))?),
            "--forget" if !forget => forget = true,
            _ => return None,
        }
    }
    Some((provider, url, forget))
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
            ["key", provider, options @ ..] => {
                let (provider, url, forget) = parse_key(provider, options)?;
                self.key(provider, url, forget)
            }
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
            writeln!(self.out, "{NO_BRAIN}")?;
            writeln!(
                self.out,
                "Pour lui en donner un : capri chat use ollama (ou un autre fournisseur : capri chat)"
            )?;
            // les clés des adresses habituelles : une clé donnée pour une autre adresse se voit
            // une fois le cerveau choisi
            let takes_key =
                |provider: &Provider| provider.key_need() != KeyNeed::None && self.keys.available();
            let stored: Vec<String> = Provider::ALL
                .into_iter()
                .filter(takes_key)
                .filter_map(|provider| {
                    let usual = Endpoint::parse(provider.default_url()?).ok()?;
                    let account = KeyAccount::new(provider, &usual);
                    self.keys
                        .contains(&account.name())
                        .unwrap_or(false)
                        .then(|| format!("{} ({})", provider.id(), account.address()))
                })
                .collect();
            let legacy: Vec<&str> = Provider::ALL
                .into_iter()
                .filter(|&provider| keys::has_legacy_key(self.keys, provider))
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
            if !legacy.is_empty() {
                writeln!(
                    self.out,
                    "{}rangées sans adresse par une version précédente de Capri, jamais envoyées : {}. \
                     Pour en redonner une : capri chat key <fournisseur> [--url <adresse>]",
                    if stored.is_empty() { "clés     : " } else { "           " },
                    legacy.join(", ")
                )?;
            }
            return Ok(());
        };
        let (brain, endpoint) = (&setup.brain, setup.brain.endpoint());
        let provider = brain.provider();
        writeln!(self.out, "cerveau  : {}, modèle {}", provider.name(), brain.model())?;
        writeln!(self.out, "adresse  : {}", endpoint.url(""))?;
        writeln!(self.out, "envoi    : {}", destination(provider, endpoint, Some(brain.model())))?;
        writeln!(self.out, "clé      : {}", self.key_status(brain))?;
        if keys::has_legacy_key(self.keys, provider) {
            writeln!(
                self.out,
                "           une ancienne clé {}, rangée sans adresse par une version précédente de Capri, \
                 n'est jamais envoyée. Pour la redonner : {}",
                of_provider(provider),
                key::set_key_command(provider, endpoint, Some(brain))
            )?;
        }
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
        let key = match keys::key_for(self.keys, provider, endpoint) {
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
                writeln!(
                    self.out,
                    "serveur  : {}",
                    problem.explain(Place::Terminal, provider, endpoint, Some(brain))
                )?;
                if let Some(hint) = how_to_fix(provider, &problem) {
                    writeln!(self.out, "           {hint}")?;
                }
            }
        }
        Ok(())
    }

    /// Ce que `status` dit de la clé du cerveau choisi, celle de son adresse, sans jamais la
    /// montrer ni la lire.
    fn key_status(&self, brain: &Brain) -> String {
        let (provider, endpoint) = (brain.provider(), brain.endpoint());
        let name = provider.name();
        if provider.key_need() == KeyNeed::None {
            return format!("aucune, {name} n'en prend pas");
        }
        if !self.keys.available() {
            return "pas de trousseau sur ce système, donc aucune".to_owned();
        }
        let account = KeyAccount::new(provider, endpoint);
        let (keychain, address) = (self.keys.name(), account.address());
        let set_key = key::set_key_command(provider, endpoint, Some(brain));
        match (self.keys.contains(&account.name()), provider.key_need()) {
            (Ok(true), _) if !endpoint.may_carry_secret() => {
                format!("rangée dans {keychain} pour {address}, MAIS elle ne partira pas en clair")
            }
            (Ok(true), _) => format!("rangée dans {keychain} pour {address} (elle ne s'affiche jamais)"),
            (Ok(false), KeyNeed::Required) => {
                format!("AUCUNE pour {address} : {name} en demande une. Pour la donner : {set_key}")
            }
            (Ok(false), _) => format!("aucune pour {address} (s'il en demande une : {set_key})"),
            (Err(err), _) => format!("{err:#}"),
        }
    }

    /// Le cerveau choisi, pour les conseils d'une commande qui en joint peut-être un autre. Des
    /// réglages illisibles n'empêchent pas d'en choisir un nouveau : ils comptent pour aucun.
    fn chosen(&self) -> Option<Brain> {
        load(&self.directory).ok().flatten().map(|setup| setup.brain)
    }

    /// Choisit un cerveau, après avoir vérifié auprès du serveur que le modèle existe.
    fn choose(&mut self, provider: Provider, model: Option<&str>, url: Option<&str>) -> Result<()> {
        let endpoint = endpoint_for(provider, url)?;
        let chosen = self.chosen();
        let listed =
            list_models(self.keys, &self.client, Place::Terminal, provider, &endpoint, chosen.as_ref())?;
        let model = pick_model(provider, &endpoint, &listed.models, model)?;
        let brain = Brain::new(provider, Some(&endpoint.url("")), &model)?;
        let file = save(&self.directory, &brain)?;
        writeln!(self.out, "{}", thinks_now_with(&brain))?;
        writeln!(self.out, "{}", destination(provider, &endpoint, Some(brain.model())))?;
        if listed.with_key {
            writeln!(
                self.out,
                "Sa clé, rangée pour {}, est lue dans {} à chaque message, et n'est écrite nulle part ailleurs.",
                KeyAccount::new(provider, &endpoint).address(),
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

    /// Range la clé d'un fournisseur dans le trousseau, ou l'en retire, pour une adresse : celle
    /// de `url`, sinon celle du cerveau choisi s'il est de ce fournisseur, sinon son adresse
    /// habituelle (`capri_chat::key::key_endpoint`).
    fn key(&mut self, provider: Provider, url: Option<&str>, forget: bool) -> Result<()> {
        let (of, id) = (of_provider(provider), provider.id());
        if provider.key_need() == KeyNeed::None {
            bail!("{} ne prend pas de clé : il n'y a rien à ranger.", provider.name());
        }
        if !self.keys.available() {
            bail!(
                "pas de trousseau sur ce système : Capri ne sait pas y ranger de clé, et n'en écrit jamais ailleurs"
            );
        }
        let given = url.map(|url| Endpoint::parse(url.trim()).context("adresse refusée")).transpose()?;
        // sans --url, l'adresse dépend du cerveau choisi : il faut pouvoir le lire
        let chosen = match load(&self.directory) {
            Ok(setup) => setup.map(|setup| setup.brain),
            Err(err) if given.is_none() => {
                return Err(err.context(format!(
                    "impossible de savoir pour quelle adresse est cette clé : donne-la avec capri chat key {id} --url <adresse>"
                )));
            }
            Err(_) => None,
        };
        let Some(endpoint) = key::key_endpoint(provider, given.as_ref(), chosen.as_ref()) else {
            match provider {
                Provider::Custom => bail!(
                    "pour quel serveur est cette clé ? Un serveur personnalisé n'a pas d'adresse habituelle, \
                     et Capri n'en utilise aucun : donne la sienne, avec capri chat key {id} --url <adresse>"
                ),
                _ => bail!(
                    "{} n'a pas d'adresse habituelle : donne-la avec capri chat key {id} --url <adresse>",
                    provider.name()
                ),
            }
        };
        let account = KeyAccount::new(provider, &endpoint);
        let (keychain, address) = (self.keys.name().to_owned(), account.address());

        if forget {
            let forgotten = keys::forget(self.keys, &account)?;
            writeln!(self.out, "{}", key_forgotten(provider, &keychain, address, forgotten.key))?;
            if forgotten.legacy {
                writeln!(self.out, "{}", legacy_key_removed(provider, true))?;
            }
            return Ok(());
        }
        // inutile de demander une clé qui ne pourrait jamais partir
        if !endpoint.may_carry_secret() {
            bail!(
                "{} Il faut une adresse en https (capri chat key {id} --url https://…).",
                no_key_in_clear(address)
            );
        }
        let prompt = format!("Clé d'API {of} pour {address} (elle ne s'affiche pas pendant la frappe) : ");
        let key = ApiKey::parse(&(self.read_key)(&prompt)?).map_err(|err| anyhow!(not_a_key(err)))?;
        let legacy = keys::store(self.keys, &account, &key)?;
        writeln!(self.out, "{}", key_stored(provider, &keychain, address))?;
        // l'ancienne clé sans adresse est remplacée par celle-ci : elle n'aurait plus jamais servi
        match legacy {
            keys::Legacy::Removed => writeln!(self.out, "{}", legacy_key_removed(provider, false))?,
            keys::Legacy::None => {}
            keys::Legacy::Kept(err) => writeln!(
                self.out,
                "L'ancienne clé {of}, rangée sans adresse, n'a pas pu être retirée ({err:#}) : elle n'est \
                 jamais envoyée."
            )?,
        }
        match chosen.as_ref().filter(|brain| brain.provider() == provider) {
            Some(brain) if KeyAccount::new(provider, brain.endpoint()) == account => {
                if brain.endpoint().may_carry_secret() {
                    writeln!(
                        self.out,
                        "Capri s'en sert dès ta prochaine phrase. Pour essayer : capri chat test"
                    )?;
                } else {
                    writeln!(
                        self.out,
                        "MAIS Capri joint {address} en http : elle ne la lui enverra pas en clair. Pour qu'elle \
                         s'en serve : capri chat use {id} --url https://… --model {}",
                        brain.model()
                    )?;
                }
            }
            Some(brain) => writeln!(
                self.out,
                "Capri pense avec {} : cette clé n'y est pas envoyée. Pour qu'elle s'en serve : capri chat use \
                 {id} --url {} --model <nom>",
                server(provider, brain.endpoint()),
                endpoint.url("")
            )?,
            None => writeln!(
                self.out,
                "Pour qu'elle s'en serve : capri chat use {id}{} --model <nom>",
                url_option(provider, &endpoint)
            )?,
        }
        Ok(())
    }

    /// Cherche, parmi les modèles d'un fournisseur, ceux dont le nom contient `word`.
    fn models(&mut self, provider: Provider, word: Option<&str>, url: Option<&str>) -> Result<()> {
        let endpoint = endpoint_for(provider, url)?;
        let chosen = self.chosen();
        let models =
            list_models(self.keys, &self.client, Place::Terminal, provider, &endpoint, chosen.as_ref())?
                .models;
        let word = word.map(str::to_lowercase);
        let found = search_models(&models, word.as_deref().unwrap_or(""));
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
        for model in found.iter().take(MAX_LISTED_MODELS) {
            writeln!(self.out, "{model}")?;
        }
        if found.len() > MAX_LISTED_MODELS {
            writeln!(
                self.out,
                "… et {} autres : précise ta recherche (capri chat models {id} <mot>).",
                found.len() - MAX_LISTED_MODELS
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
        let message = if message.trim().is_empty() { TEST_QUESTION } else { message.trim() };
        writeln!(self.out, "> {message}")?;
        let mut wrote = false;
        let Self { keys, client, out, .. } = self;
        let talked = talk(
            *keys,
            client,
            &Speaker { place: Place::Terminal, brain: &setup.brain, persona: &setup.persona },
            message,
            &AtomicBool::new(false),
            &mut |text| {
                wrote = true;
                let _ = write!(out, "{text}");
                let _ = out.flush();
            },
        );
        if wrote {
            writeln!(self.out)?;
        }
        let talked = talked?;
        eprintln!(
            "({}, modèle {} : premier mot en {:.1} s, réponse en {:.1} s)",
            setup.brain.provider().name(),
            setup.brain.model(),
            talked.first_word.unwrap_or_default().as_secs_f32(),
            talked.total.as_secs_f32()
        );
        Ok(())
    }

    fn off(&mut self) -> Result<()> {
        if forget(&self.directory)? {
            writeln!(self.out, "{NO_MORE_BRAIN}")?;
        } else {
            writeln!(self.out, "Capri n'avait pas de cerveau.")?;
        }
        if self.keys.available() {
            writeln!(
                self.out,
                "Les clés d'API restent dans {} ; pour en retirer une : capri chat key <fournisseur> [--url <adresse>] --forget",
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

/// Ce que propose un serveur.
pub struct Listed {
    pub models: Vec<String>,
    /// Demandé avec la clé rangée pour son adresse.
    pub with_key: bool,
}

/// Les modèles que propose le serveur de `provider` à `endpoint`, demandés avec la clé rangée pour
/// cette adresse s'il en faut une (les règles de [`key_for_server`] : personne n'est contacté
/// sans la clé exigée). `capri chat use`, `capri chat models` et la fenêtre de réglages passent
/// par ici. `place` : où s'afficheront les messages, pour leurs conseils ([`Place`]). `chosen` : le
/// cerveau choisi, pour les commandes que proposent les messages d'un terminal.
/// Bloque : hors du thread principal.
pub fn list_models(
    keys: &dyn KeyStore,
    client: &Client,
    place: Place,
    provider: Provider,
    endpoint: &Endpoint,
    chosen: Option<&Brain>,
) -> Result<Listed> {
    let key = key_for_server(keys, place, provider, endpoint, chosen)?;
    let models = client.models(provider, endpoint, key.as_ref()).map_err(|problem| {
        let hint = how_to_fix(provider, &problem).map(|hint| format!("\n{hint}")).unwrap_or_default();
        anyhow!("{}{hint}", problem.explain(place, provider, endpoint, chosen))
    })?;
    Ok(Listed { models, with_key: key.is_some() })
}

/// Ce qu'a duré un échange avec le cerveau.
pub struct Talked {
    pub first_word: Option<Duration>,
    pub total: Duration,
}

/// Qui parle dans [`talk`], et où s'affichera sa panne.
#[derive(Clone, Copy)]
pub struct Speaker<'a> {
    /// Où s'affichera la panne, pour son conseil ([`Place`]).
    pub place: Place,
    pub brain: &'a Brain,
    pub persona: &'a str,
}

/// Dit `message` à `brain`, avec la clé rangée pour son adresse s'il en prend une, et passe sa
/// réponse à `on_text` au fil de l'eau, nettoyée comme pour sa bulle. Une panne est l'erreur
/// rendue, dans les mots de sa bulle. `capri chat test` et l'essai de la fenêtre de réglages
/// passent par ici. Bloque : hors du thread principal.
pub fn talk(
    keys: &dyn KeyStore,
    client: &Client,
    speaker: &Speaker,
    message: &str,
    cancelled: &AtomicBool,
    on_text: &mut dyn FnMut(&str),
) -> Result<Talked> {
    let Speaker { place, brain, persona } = *speaker;
    let provider = brain.provider();
    let key = keys::key_for(keys, provider, brain.endpoint())
        .with_context(|| format!("pas de réponse : la clé {} est illisible", of_provider(provider)))?;
    let turns = [Turn { role: Role::User, text: message.to_owned() }];
    let started = Instant::now();
    let mut first_word = None;
    let mut failure = None;
    // le même nettoyage que pour sa bulle : on voit ici ce qu'elle dirait
    let mut plain = PlainStream::new();
    let ask = Ask { place, brain, key: key.as_ref(), persona, turns: &turns };
    client.answer(&ask, cancelled, &mut |event| {
        plain.filter(event, &mut |event| match event {
            ReplyEvent::Delta(text) => {
                first_word.get_or_insert_with(|| started.elapsed());
                on_text(&text);
            }
            ReplyEvent::Done => {}
            ReplyEvent::Failed(reason) => failure = Some(reason),
        });
    });
    if let Some(reason) = failure {
        bail!("{reason}");
    }
    Ok(Talked { first_word, total: started.elapsed() })
}

/// « --url <adresse> » à ajouter à une commande pour viser `endpoint`, sauf si c'est l'adresse
/// habituelle du fournisseur.
fn url_option(provider: Provider, endpoint: &Endpoint) -> String {
    let usual = provider.default_url().and_then(|url| Endpoint::parse(url).ok());
    match usual {
        Some(usual) if KeyAccount::new(provider, &usual) == KeyAccount::new(provider, endpoint) => {
            String::new()
        }
        _ => format!(" --url {}", endpoint.url("")),
    }
}

/// La clé à envoyer à ce serveur, celle rangée pour son adresse, après avoir vérifié qu'il en faut
/// une et qu'elle a le droit d'y aller : jamais en clair sur le réseau. Rien n'est contacté ici.
/// `place` : où s'affichera le message ; `chosen` : le cerveau choisi, pour la commande à proposer
/// à un terminal.
fn key_for_server(
    keys: &dyn KeyStore,
    place: Place,
    provider: Provider,
    endpoint: &Endpoint,
    chosen: Option<&Brain>,
) -> Result<Option<BoundKey>> {
    let address = KeyAccount::new(provider, endpoint).address().to_owned();
    let key = keys::key_for(keys, provider, endpoint)
        .with_context(|| format!("la clé {} pour {address} est illisible", of_provider(provider)))?;
    let command = key::set_key_command(provider, endpoint, chosen);
    let how_to = |job| place.how_to_key(job, &command);
    match (provider.key_need(), &key) {
        (KeyNeed::Required, None) => bail!(
            "{} demande une clé, et il n'y en a pas pour {address}. Range-la d'abord : {}",
            provider.name(),
            how_to(KeyJob::Give)
        ),
        (_, Some(_)) if !endpoint.may_carry_secret() => bail!(
            "la clé {} rangée pour {address} ne partirait pas en clair : donne une adresse en https, ou retire la clé ({})",
            of_provider(provider),
            how_to(KeyJob::Forget)
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

    /// Le compte d'une clé de ce fournisseur pour cette adresse.
    fn account(provider: Provider, url: &str) -> String {
        KeyAccount::new(provider, &at(url)).name()
    }

    /// La clé rangée sous ce compte, en clair : pour les vérifications des tests seulement.
    fn stored(keys: &MemoryKeys, provider: Provider, url: &str) -> Option<String> {
        keys.secret(&account(provider, url))
    }

    fn at(url: &str) -> Endpoint {
        Endpoint::parse(url).expect("adresse valable")
    }

    const OPENROUTER: &str = "https://openrouter.ai/api/v1";

    #[test]
    fn a_key_is_typed_stored_and_forgotten_without_ever_being_shown() {
        let directory = scratch_dir("cle-rangee");
        let keys = MemoryKeys::default();
        let mut prompts = Vec::new();
        let ran = {
            let mut out = Vec::new();
            let mut read_key = |prompt: &str| -> Result<String> {
                prompts.push(prompt.to_owned());
                Ok(format!("  {KEY}\n"))
            };
            let mut cli = Cli {
                directory: directory.clone(),
                keys: &keys,
                client: Client::new(),
                read_key: &mut read_key,
                out: &mut out,
            };
            let outcome = cli.run(&["key", "openrouter"]).expect("une commande connue");
            (outcome.err().map(|err| format!("{err:#}")), String::from_utf8(out).expect("du texte"))
        };
        assert_eq!(ran.0, None);
        assert_eq!(
            prompts,
            ["Clé d'API d'OpenRouter pour openrouter.ai:443 (elle ne s'affiche pas pendant la frappe) : "]
        );
        assert_eq!(
            ran.1,
            "Clé d'OpenRouter rangée dans le trousseau de test, pour openrouter.ai:443 seulement : elle ne partira \
             vers aucune autre adresse, et n'est écrite nulle part ailleurs.\n\
             Pour qu'elle s'en serve : capri chat use openrouter --model <nom>\n"
        );
        assert_eq!(keys.accounts(), ["openrouter@openrouter.ai:443"]);
        assert_eq!(
            stored(&keys, Provider::OpenRouter, OPENROUTER).as_deref(),
            Some(KEY),
            "sans les blancs autour"
        );
        assert!(!ran.1.contains(KEY));
        assert!(!any_file_contains(&directory, KEY), "rien n'est écrit dans le dossier des réglages");

        let ran = run(&directory, &keys, Some("sk-une-autre-cle-de-test"), &["key", "openrouter"]);
        assert_eq!(ran.error, None);
        assert_eq!(
            stored(&keys, Provider::OpenRouter, OPENROUTER).as_deref(),
            Some("sk-une-autre-cle-de-test"),
            "remplacée"
        );

        let ran = run(&directory, &keys, None, &["key", "openrouter", "--forget"]);
        assert_eq!((ran.error, ran.asked), (None, false));
        assert_eq!(
            ran.out,
            "Clé d'OpenRouter pour openrouter.ai:443 retirée : elle n'est plus dans le trousseau de test.\n"
        );
        assert!(keys.accounts().is_empty());
        let ran = run(&directory, &keys, None, &["key", "openrouter", "--forget"]);
        assert_eq!(
            ran.out,
            "Il n'y avait pas de clé d'OpenRouter pour openrouter.ai:443 dans le trousseau de test.\n"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn the_key_command_says_which_address_the_key_is_for() {
        let directory = scratch_dir("cle-adresse");
        let keys = MemoryKeys::default();

        // un serveur personnalisé n'a pas d'adresse habituelle : il faut la donner
        let ran = run(&directory, &keys, Some(KEY), &["key", "custom"]);
        assert_eq!(
            ran.error.as_deref(),
            Some(
                "pour quel serveur est cette clé ? Un serveur personnalisé n'a pas d'adresse habituelle, et Capri \
                 n'en utilise aucun : donne la sienne, avec capri chat key custom --url <adresse>"
            )
        );
        assert!(!ran.asked, "rien n'est demandé");
        let ran = run(&directory, &keys, None, &["key", "custom", "--forget"]);
        assert!(ran.error.expect("refusé").contains("--url <adresse>"));

        // --url : cette adresse-là
        let ran =
            run(&directory, &keys, Some(KEY), &["key", "custom", "--url", "https://LLM.example:8443/v1"]);
        assert_eq!(ran.error, None);
        assert!(ran.out.contains("pour llm.example:8443 seulement"), "{}", ran.out);
        assert!(
            ran.out.ends_with("Pour qu'elle s'en serve : capri chat use custom --url https://llm.example:8443/v1 --model <nom>\n"),
            "{}",
            ran.out
        );
        let ran = run(
            &directory,
            &keys,
            Some(KEY),
            &["key", "openrouter", "--url", "https://proxy.example/api/v1"],
        );
        assert!(
            ran.out.ends_with("Pour qu'elle s'en serve : capri chat use openrouter --url https://proxy.example/api/v1 --model <nom>\n"),
            "{}",
            ran.out
        );
        assert_eq!(keys.accounts(), ["custom@llm.example:8443", "openrouter@proxy.example:443"]);

        // sans --url : l'adresse du cerveau choisi, s'il est de ce fournisseur
        save(
            &directory,
            &Brain::new(Provider::Custom, Some("http://127.0.0.1:8080/v1"), "m").expect("valide"),
        )
        .expect("écrit");
        let ran = run(&directory, &keys, Some(KEY), &["key", "custom"]);
        assert_eq!(ran.error, None);
        assert!(ran.out.contains("pour 127.0.0.1:8080 seulement"), "{}", ran.out);
        assert!(
            ran.out.ends_with("Capri s'en sert dès ta prochaine phrase. Pour essayer : capri chat test\n"),
            "{}",
            ran.out
        );
        // … sinon l'adresse habituelle
        let ran = run(&directory, &keys, Some(KEY), &["key", "deepseek"]);
        assert!(ran.out.contains("pour api.deepseek.com:443 seulement"), "{}", ran.out);
        // une clé pour une autre adresse que celle du cerveau choisi : elle n'y part pas, et c'est dit
        let ran = run(&directory, &keys, Some(KEY), &["key", "custom", "--url", "http://127.0.0.1:9090/v1"]);
        assert!(
            ran.out.ends_with(
                "Capri pense avec 127.0.0.1:8080 : cette clé n'y est pas envoyée. Pour qu'elle s'en serve : capri \
                 chat use custom --url http://127.0.0.1:9090/v1 --model <nom>\n"
            ),
            "{}",
            ran.out
        );
        // une adresse en clair sur le réseau : pas de clé, puisqu'elle n'y partirait jamais
        let ran =
            run(&directory, &keys, Some(KEY), &["key", "custom", "--url", "http://192.168.1.20:1234/v1"]);
        assert!(ran.error.expect("refusé").contains("192.168.1.20:1234 n'est joint qu'en http"));
        assert!(!ran.asked);
        // une adresse refusée l'est avant toute question
        let ran = run(&directory, &keys, Some(KEY), &["key", "custom", "--url", "http://example.com/v1"]);
        assert!(ran.error.expect("refusé").starts_with("adresse refusée"));
        assert!(!ran.asked);

        // des réglages illisibles : sans --url, on ne devine pas l'adresse
        std::fs::write(directory.join("chat.json"), "{ cassé").expect("écrit");
        let ran = run(&directory, &keys, Some(KEY), &["key", "custom"]);
        assert!(ran.error.expect("refusé").contains("donne-la avec capri chat key custom --url <adresse>"));
        assert!(!ran.asked);
        let ran =
            run(&directory, &keys, None, &["key", "custom", "--url", "http://127.0.0.1:8080/v1", "--forget"]);
        assert_eq!(ran.error, None, "avec --url, ils ne servent à rien");
        assert!(!ran.out.contains(KEY) && !any_file_contains(&directory, KEY));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn what_is_not_a_key_changes_nothing_and_is_never_repeated() {
        let directory = scratch_dir("pas-une-cle");
        let deepseek = "https://api.deepseek.com";
        let keys = MemoryKeys::with(Provider::DeepSeek, deepseek, KEY);
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
        assert_eq!(
            stored(&keys, Provider::DeepSeek, deepseek).as_deref(),
            Some(KEY),
            "l'ancienne clé est restée"
        );
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
        let url = format!("http://{address}/v1");
        let keys = MemoryKeys::with(Provider::Custom, &url, KEY);
        let ran =
            run(&directory, &keys, None, &["use", "custom", "--url", &url, "--model", "labo/un-modele"]);
        assert_eq!(ran.error, None);
        assert!(
            ran.out.starts_with(&format!(
                "Capri pense maintenant avec labo/un-modele (Serveur personnalisé, {address}).\n\
                 Ce que tu lui dis reste sur cette machine.\n\
                 Sa clé, rangée pour {address}, est lue dans le trousseau de test à chaque message, et n'est écrite nulle part ailleurs.\n"
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
            ran.error,
            Some(format!(
                "OpenRouter demande une clé, et il n'y en a pas pour {address}. Range-la d'abord : capri chat key openrouter --url {url}"
            ))
        );
        assert_eq!(load(&directory).expect("lisible"), None, "aucun cerveau choisi");
        assert!(
            requests.recv_timeout(std::time::Duration::from_millis(100)).is_err(),
            "personne n'a été contacté"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn the_window_never_sends_the_user_to_a_terminal_for_a_key() {
        // aucun contact : la clé manque, ou ne partirait pas en clair
        let openrouter = Endpoint::parse("https://openrouter.ai/api/v1").expect("adresse valable");
        let said = |place, keys: &MemoryKeys, provider, endpoint: &Endpoint| {
            format!("{:#}", key_for_server(keys, place, provider, endpoint, None).expect_err("refusé"))
        };
        let empty = MemoryKeys::default();
        assert_eq!(
            said(Place::Terminal, &empty, Provider::OpenRouter, &openrouter),
            "OpenRouter demande une clé, et il n'y en a pas pour openrouter.ai:443. Range-la d'abord : capri chat key openrouter"
        );
        assert_eq!(
            said(Place::Window, &empty, Provider::OpenRouter, &openrouter),
            "OpenRouter demande une clé, et il n'y en a pas pour openrouter.ai:443. Range-la d'abord : saisis-la ci-dessus."
        );
        let lan = Endpoint::parse("http://192.168.1.20:1234/v1").expect("adresse valable");
        let keys = MemoryKeys::with(Provider::Custom, "https://192.168.1.20:1234/v1", KEY);
        let window = said(Place::Window, &keys, Provider::Custom, &lan);
        assert!(window.ends_with("ou retire la clé (bouton « Retirer la clé »)"), "{window}");
        assert!(!window.contains("capri chat"), "{window}");
    }

    #[test]
    fn a_key_never_goes_in_clear_to_another_machine() {
        // vérifié avant tout contact : ce test n'ouvre aucune connexion. Une clé donnée en https
        // pour un serveur du réseau privé est celle du même hôte et du même port en http.
        let keys = MemoryKeys::with(Provider::Custom, "https://192.168.1.20:1234/v1", KEY);
        keys.add(Provider::Custom, "https://10.0.0.5:8080/v1", KEY);
        keys.add(Provider::Custom, "http://localhost:1234/v1", KEY);
        for (url, host) in [
            ("http://192.168.1.20:1234/v1", "192.168.1.20:1234"),
            ("http://10.0.0.5:8080/v1", "10.0.0.5:8080"),
        ] {
            let error = format!(
                "{:#}",
                key_for_server(&keys, Place::Terminal, Provider::Custom, &at(url), None).expect_err("refusé")
            );
            assert_eq!(
                error,
                format!(
                    "la clé du serveur personnalisé rangée pour {host} ne partirait pas en clair : donne une adresse en https, ou retire la clé (capri chat key custom --url {url} --forget)"
                )
            );
        }
        let permitted = |keys: &MemoryKeys, provider, url: &str| {
            key_for_server(keys, Place::Terminal, provider, &at(url), None).expect("permis").is_some()
        };
        assert!(permitted(&keys, Provider::Custom, "https://192.168.1.20:1234/v1"));
        assert!(permitted(&keys, Provider::Custom, "http://localhost:1234/v1"));
        // sans clé pour cette adresse, un serveur du réseau privé reste possible
        assert!(!permitted(&keys, Provider::Custom, "http://192.168.1.20:1235/v1"));
        assert!(!permitted(&MemoryKeys::default(), Provider::Custom, "http://192.168.1.20:1234/v1"));
        // Ollama n'en reçoit jamais
        let ollama_keys = MemoryKeys::with(Provider::Ollama, "http://192.168.1.20:11434/v1", KEY);
        assert!(!permitted(&ollama_keys, Provider::Ollama, "http://192.168.1.20:11434/v1"));
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
    fn a_cloud_model_of_the_local_ollama_is_said_to_leave_the_machine() {
        let directory = scratch_dir("modele-cloud");
        let (address, _requests) = fake_server::serve(vec![
            fake_server::listing(&["un-modele:cloud", "un-autre:8b"]),
            fake_server::listing(&["un-modele:cloud", "un-autre:8b"]),
        ]);
        let url = format!("http://{address}/v1");
        let keys = MemoryKeys::default();
        let ran =
            run(&directory, &keys, None, &["use", "ollama", "--url", &url, "--model", "un-modele:cloud"]);
        assert_eq!(ran.error, None);
        assert!(
            ran.out.contains(
                "Attention : avec ce modèle « cloud », l'Ollama de cette machine relaie ce que tu lui dis vers les \
                 serveurs d'Ollama, sur Internet"
            ),
            "{}",
            ran.out
        );
        assert!(!ran.out.contains("reste sur cette machine"), "{}", ran.out);
        let ran = run(&directory, &keys, None, &["status"]);
        assert!(ran.out.contains("envoi    : Attention : avec ce modèle « cloud »"), "{}", ran.out);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn status_says_whether_a_key_is_there_without_showing_it() {
        let directory = scratch_dir("statut-cle");
        let (address, requests) = fake_server::serve(vec![
            fake_server::listing(&["modele-de-test"]),
            fake_server::listing(&["modele-de-test"]),
        ]);
        let url = format!("http://{address}/v1");
        let custom = Brain::new(Provider::Custom, Some(&url), "modele-de-test").expect("valide");
        save(&directory, &custom).expect("écrit");

        let ran = run(&directory, &MemoryKeys::with(Provider::Custom, &url, KEY), None, &["status"]);
        assert_eq!(ran.error, None);
        for line in [
            "cerveau  : Serveur personnalisé, modèle modele-de-test\n".to_owned(),
            "envoi    : Ce que tu lui dis reste sur cette machine.\n".to_owned(),
            format!(
                "clé      : rangée dans le trousseau de test pour {address} (elle ne s'affiche jamais)\n"
            ),
            "serveur  : répond, et le modèle est là\n".to_owned(),
        ] {
            assert!(ran.out.contains(&line), "{line:?} manque dans :\n{}", ran.out);
        }
        assert!(!ran.out.contains(KEY) && !ran.out.contains("ancienne"));
        let (head, _) = requests.recv_timeout(std::time::Duration::from_secs(5)).expect("une requête");
        assert_eq!(header(&head, "authorization"), Some(format!("Bearer {KEY}")));

        let ran = run(&directory, &MemoryKeys::default(), None, &["status"]);
        assert!(
            ran.out.contains(&format!(
                "clé      : aucune pour {address} (s'il en demande une : capri chat key custom)\n"
            )),
            "{}",
            ran.out
        );
        let (head, _) = requests.recv_timeout(std::time::Duration::from_secs(5)).expect("une requête");
        assert_eq!(header(&head, "authorization"), None);

        // un fournisseur qui exige une clé, sans clé pour cette adresse : il le dit, et ne contacte
        // personne, même avec la clé de son adresse habituelle
        let openrouter =
            Brain::new(Provider::OpenRouter, Some(&format!("http://{address}/api/v1")), "m").expect("valide");
        save(&directory, &openrouter).expect("écrit");
        let ran =
            run(&directory, &MemoryKeys::with(Provider::OpenRouter, OPENROUTER, KEY), None, &["status"]);
        assert!(
            ran.out.contains(&format!(
                "clé      : AUCUNE pour {address} : OpenRouter en demande une. Pour la donner : capri chat key openrouter\n"
            )),
            "{}",
            ran.out
        );
        assert!(
            ran.out.contains(&format!(
                "serveur  : OpenRouter demande une clé, et je n'en ai pas pour {address}."
            )),
            "{}",
            ran.out
        );
        assert!(
            requests.recv_timeout(std::time::Duration::from_millis(100)).is_err(),
            "personne n'a été contacté"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn without_a_brain_status_lists_the_stored_keys_by_name_and_address_only() {
        let directory = scratch_dir("statut-vide");
        let keys = MemoryKeys::with(Provider::DeepSeek, "https://api.deepseek.com", KEY);
        keys.add(Provider::ProxyApi, "https://api.proxyapi.ru/v1", "sk-une-autre-cle-de-test");
        let ran = run(&directory, &keys, None, &["status"]);
        assert!(ran.out.contains("Capri n'a pas de cerveau"), "{}", ran.out);
        assert!(
            ran.out.ends_with(
                "clés     : rangées dans le trousseau de test pour deepseek (api.deepseek.com:443), proxyapi (api.proxyapi.ru:443)\n"
            ),
            "{}",
            ran.out
        );
        assert!(!ran.out.contains(KEY));

        // et celles d'une version précédente, sans adresse
        keys.add_legacy(Provider::OpenRouter, KEY);
        let ran = run(&directory, &keys, None, &["status"]);
        assert!(
            ran.out.ends_with(
                "           rangées sans adresse par une version précédente de Capri, jamais envoyées : openrouter. \
                 Pour en redonner une : capri chat key <fournisseur> [--url <adresse>]\n"
            ),
            "{}",
            ran.out
        );
        let only_old = MemoryKeys::default();
        only_old.add_legacy(Provider::Custom, KEY);
        let ran = run(&directory, &only_old, None, &["status"]);
        assert!(
            ran.out.contains("clés     : rangées sans adresse par une version précédente de Capri, jamais envoyées : custom."),
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
        let keys = MemoryKeys::with(Provider::OpenRouter, OPENROUTER, KEY);
        let ran = run(&directory, &keys, None, &["off"]);
        assert_eq!(ran.error, None);
        assert_eq!(
            ran.out,
            "Capri n'a plus de cerveau : elle répète ce qu'on lui dit, et rien ne sort de ta machine.\n\
             Les clés d'API restent dans le trousseau de test ; pour en retirer une : capri chat key <fournisseur> [--url <adresse>] --forget\n"
        );
        assert_eq!(
            stored(&keys, Provider::OpenRouter, OPENROUTER).as_deref(),
            Some(KEY),
            "la clé n'est pas touchée"
        );
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
        let url = format!("http://{address}/v1");
        let custom = Brain::new(Provider::Custom, Some(&url), "modele-de-test").expect("valide");
        save(&directory, &custom).expect("écrit");
        let keys = MemoryKeys::with(Provider::Custom, &url, KEY);

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
        assert_eq!(lines.len(), MAX_LISTED_MODELS + 2, "{}", ran.out);
        assert_eq!(lines[0], "autre/chose", "dans l'ordre alphabétique");
        assert_eq!(
            lines[MAX_LISTED_MODELS],
            "… et 91 autres : précise ta recherche (capri chat models custom <mot>)."
        );
        assert_eq!(
            lines[MAX_LISTED_MODELS + 1],
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
            &["key", "openrouter", "--forget", "--forget"],
            &["key", "openrouter", "--url"],
            &["key", "openrouter", "--url", "--forget"],
            &["key", "openrouter", "--url", "https://a.example", "--url", "https://b.example"],
            &["key", "openrouter", "--model", "m"],
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
        assert_eq!(parse_key("deepseek", &[]), Some((Provider::DeepSeek, None, false)));
        assert_eq!(
            parse_key("custom", &["--forget", "--url", "http://localhost:8080/v1"]),
            Some((Provider::Custom, Some("http://localhost:8080/v1"), true))
        );
        assert_eq!(
            parse_key("custom", &["--url", "http://localhost:8080/v1"]),
            Some((Provider::Custom, Some("http://localhost:8080/v1"), false))
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    // --- Une clé est liée à l'adresse pour laquelle elle a été donnée ---

    /// La requête suivante reçue par un faux serveur.
    fn next(requests: &std::sync::mpsc::Receiver<fake_server::Request>) -> fake_server::Request {
        requests.recv_timeout(std::time::Duration::from_secs(5)).expect("une requête")
    }

    /// Aucune requête n'est (encore) arrivée à ce faux serveur.
    fn untouched(requests: &std::sync::mpsc::Receiver<fake_server::Request>) -> bool {
        requests.recv_timeout(std::time::Duration::from_millis(100)).is_err()
    }

    /// La requête ne porte aucune clé : ni en-tête `Authorization`, ni le texte de `key` nulle part.
    fn carries_no_key((head, body): &fake_server::Request, key: &str) -> bool {
        header(head, "authorization").is_none() && !head.contains(key) && !body.contains(key)
    }

    #[test]
    fn a_key_given_for_one_address_never_reaches_another_port() {
        let directory = scratch_dir("cle-un-port");
        let keys = MemoryKeys::default();
        // deux serveurs sur cette machine, qui ne diffèrent que par leur port
        let (given_for, given_requests) = fake_server::serve(vec![fake_server::listing(&["m"])]);
        let (other, requests) = fake_server::serve(vec![
            fake_server::listing(&["m"]),
            fake_server::listing(&["m"]),
            fake_server::reply("Salut."),
            fake_server::listing(&["m"]),
        ]);
        let (given_url, other_url) = (format!("http://{given_for}/v1"), format!("http://{other}/v1"));

        let ran = run(&directory, &keys, Some(KEY), &["key", "custom", "--url", &given_url]);
        assert_eq!(ran.error, None);
        assert!(ran.out.contains(&format!("pour {given_for} seulement")), "{}", ran.out);
        assert_eq!(keys.accounts(), [format!("custom@{given_for}")]);

        // chercher un modèle ailleurs
        let ran = run(&directory, &keys, None, &["models", "custom", "--url", &other_url]);
        assert_eq!(ran.error, None);
        assert!(carries_no_key(&next(&requests), KEY), "models");
        // choisir ce serveur-là
        let ran = run(&directory, &keys, None, &["use", "custom", "--url", &other_url, "--model", "m"]);
        assert_eq!(ran.error, None);
        assert!(!ran.out.contains("Sa clé"), "{}", ran.out);
        assert!(carries_no_key(&next(&requests), KEY), "use");
        // lui parler
        let ran = run(&directory, &keys, None, &["test", "Coucou"]);
        assert_eq!((ran.error, ran.out.as_str()), (None, "> Coucou\nSalut.\n"));
        assert!(carries_no_key(&next(&requests), KEY), "test");
        // demander où on en est
        let ran = run(&directory, &keys, None, &["status"]);
        assert!(
            ran.out.contains(&format!(
                "clé      : aucune pour {other} (s'il en demande une : capri chat key custom)\n"
            )),
            "{}",
            ran.out
        );
        assert!(carries_no_key(&next(&requests), KEY), "status");
        assert!(untouched(&given_requests), "l'adresse de la clé n'a pas été jointe non plus");

        // et là où elle a été donnée, elle part bien
        let ran = run(&directory, &keys, None, &["use", "custom", "--url", &given_url, "--model", "m"]);
        assert_eq!(ran.error, None);
        assert_eq!(header(&next(&given_requests).0, "authorization"), Some(format!("Bearer {KEY}")));
        assert!(!ran.out.contains(KEY) && !any_file_contains(&directory, KEY));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn another_url_for_a_named_provider_does_not_take_its_usual_key_along() {
        let directory = scratch_dir("cle-adresse-habituelle");
        let keys = MemoryKeys::default();
        // les clés des adresses habituelles : rien n'est contacté pour les ranger
        for provider in ["openrouter", "lmstudio"] {
            let ran = run(&directory, &keys, Some(KEY), &["key", provider]);
            assert_eq!(ran.error, None, "{provider}");
        }
        assert_eq!(keys.accounts(), ["lmstudio@localhost:1234", "openrouter@openrouter.ai:443"]);

        let (elsewhere, requests) = fake_server::serve(vec![fake_server::listing(&["m"])]);
        // OpenRouter exige une clé : sans clé pour cette adresse, il n'est même pas contacté
        let url = format!("http://{elsewhere}/api/v1");
        for args in [
            &["use", "openrouter", "--url", &url, "--model", "m"][..],
            &["models", "openrouter", "--url", &url],
        ] {
            let ran = run(&directory, &keys, None, args);
            assert_eq!(
                ran.error,
                Some(format!(
                    "OpenRouter demande une clé, et il n'y en a pas pour {elsewhere}. Range-la d'abord : capri chat key openrouter --url {url}"
                )),
                "{args:?}"
            );
        }
        assert!(untouched(&requests), "personne n'a été contacté");
        // LM Studio, à clé facultative, est joint, mais sans la clé de son adresse habituelle
        let url = format!("http://{elsewhere}/v1");
        let ran = run(&directory, &keys, None, &["use", "lmstudio", "--url", &url, "--model", "m"]);
        assert_eq!(ran.error, None);
        assert!(carries_no_key(&next(&requests), KEY));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_key_kept_without_an_address_is_never_sent_and_status_says_so() {
        let directory = scratch_dir("ancienne-cle");
        let keys = MemoryKeys::default();
        keys.add_legacy(Provider::Custom, KEY);
        keys.add_legacy(Provider::OpenRouter, KEY);
        let (address, requests) = fake_server::serve(vec![
            fake_server::listing(&["m"]),
            fake_server::listing(&["m"]),
            fake_server::listing(&["m"]),
            fake_server::reply("Salut."),
        ]);
        let url = format!("http://{address}/v1");

        let ran = run(&directory, &keys, None, &["models", "custom", "--url", &url]);
        assert_eq!(ran.error, None);
        assert!(carries_no_key(&next(&requests), KEY), "models");
        let ran = run(&directory, &keys, None, &["use", "custom", "--url", &url, "--model", "m"]);
        assert_eq!(ran.error, None);
        assert!(carries_no_key(&next(&requests), KEY), "use");
        let ran = run(&directory, &keys, None, &["status"]);
        assert!(
            ran.out.contains(
                "           une ancienne clé du serveur personnalisé, rangée sans adresse par une version précédente \
                 de Capri, n'est jamais envoyée. Pour la redonner : capri chat key custom\n"
            ),
            "{}",
            ran.out
        );
        assert!(carries_no_key(&next(&requests), KEY), "status");
        let ran = run(&directory, &keys, None, &["test"]);
        assert_eq!(ran.error, None);
        assert!(carries_no_key(&next(&requests), KEY), "test");

        // OpenRouter, qui l'exige : pas de clé pour son adresse, donc personne n'est contacté (une
        // adresse locale : sous Linux, une adresse en https serait refusée avant, faute de TLS)
        let openrouter =
            Brain::new(Provider::OpenRouter, Some(&format!("http://{address}/api/v1")), "m").expect("valide");
        save(&directory, &openrouter).expect("écrit");
        let ran = run(&directory, &keys, None, &["test"]);
        assert_eq!(
            ran.error,
            Some(format!(
                "OpenRouter demande une clé, et je n'en ai pas pour {address}. Pour me la donner : capri chat key openrouter"
            ))
        );
        let ran = run(&directory, &keys, None, &["status"]);
        assert!(ran.out.contains(&format!("clé      : AUCUNE pour {address}")), "{}", ran.out);
        assert!(ran.out.contains("une ancienne clé d'OpenRouter, rangée sans adresse"), "{}", ran.out);
        assert!(!ran.out.contains(KEY));
        assert!(untouched(&requests), "personne n'a été contacté");

        // la redonner remplace l'ancienne ; --forget retire l'ancienne aussi
        let ran = run(&directory, &keys, Some("sk-une-autre-cle-de-test"), &["key", "openrouter"]);
        assert!(
            ran.out.contains(
                "L'ancienne clé d'OpenRouter, rangée sans adresse par une version précédente de Capri, est retirée.\n"
            ),
            "{}",
            ran.out
        );
        assert_eq!(keys.accounts(), ["custom".to_owned(), format!("openrouter@{address}")]);
        let ran = run(&directory, &keys, None, &["key", "custom", "--url", &url, "--forget"]);
        assert_eq!(
            ran.out,
            format!(
                "Il n'y avait pas de clé du serveur personnalisé pour {address} dans le trousseau de test.\n\
                 L'ancienne clé du serveur personnalisé, rangée sans adresse par une version précédente de Capri, est retirée aussi.\n"
            )
        );
        assert_eq!(keys.accounts(), [format!("openrouter@{address}")]);
        assert!(
            !keys::has_legacy_key(&keys, Provider::OpenRouter)
                && !keys::has_legacy_key(&keys, Provider::Custom)
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
