//! Le seul crate de Capri qui parle au réseau.
//!
//! Il envoie une conversation au fournisseur choisi et lit la réponse au fil de l'eau. Ce qu'il
//! faut envoyer et comment lire ce qui revient est décidé par `capri-chat`, qui ne se connecte à
//! rien (le format de chaque fournisseur : `capri_chat::Wire`) ; ici, on se connecte, et rien
//! d'autre.
//!
//! Ce que ce crate garantit :
//! - il ne contacte que l'adresse du cerveau choisi, que `capri_chat::Endpoint` a vérifiée : pas
//!   de proxy, même si l'environnement en déclare un, et aucune redirection suivie (une réponse
//!   qui renvoie ailleurs est un refus) ;
//! - il ne part que ce qu'on lui confie (la personnalité et la conversation, et la clé d'API du
//!   fournisseur s'il en faut une), et rien n'est écrit dans le journal de ce qui a été dit ;
//! - une clé ne part que vers l'adresse pour laquelle elle a été donnée, et seulement en
//!   `https://`, ou en `http://` vers cette machine même : les règles de `capri_chat::key`,
//!   vérifiées avant toute connexion. Une clé donnée pour une autre adresse compte pour absente.
//!   L'en-tête qui la porte (`Authorization` ou, pour Anthropic, `x-api-key`) est choisi par
//!   `capri_chat::key::key_header` : ici, on le pose tel quel.
//!   Elle ne va jamais à un fournisseur qui n'en prend pas, et n'apparaît dans aucun message : si
//!   un fournisseur la répète dans une erreur, elle y est masquée ;
//! - une réponse s'arrête dès qu'on le demande, et le serveur l'apprend : la connexion est fermée ;
//! - tout est borné : le temps de connexion, le silence du serveur, la taille d'un événement, la
//!   longueur d'une réponse ;
//! - en `https://`, la connexion passe par le TLS du système (Security.framework sur macOS,
//!   SChannel sur Windows) et son magasin de certificats : un certificat refusé arrête tout avant
//!   le moindre octet de la conversation, et rien ne repasse jamais en clair.
//!
//! Il sait aussi frapper à une adresse **de cette machine seulement** ([`knock`]) : une connexion
//! ouverte puis fermée, sans rien envoyer, pour savoir si Ollama tourne avant de le lancer.
//!
//! Ailleurs (Linux), pas de TLS du tout : il y faudrait OpenSSL. Une adresse en `https://` y est
//! refusée avec un message clair, avant toute connexion.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use capri_chat::key::KeyHeader;
use capri_chat::sse::SseDecoder;
use capri_chat::wire::End;
use capri_chat::{
    BoundKey, Brain, Endpoint, KeyError, KeyJob, KeyNeed, Place, Provider, Reach, ReplyEvent, Turn, Wire,
    key, phrases,
};

/// Ce système sait-il parler `https://` ?
pub const TLS: bool = cfg!(any(target_os = "macos", windows));

/// Taille maximale lue d'un corps d'erreur, et de la liste des modèles. Celle d'un fournisseur
/// distant décrit des centaines de modèles (plus de 800 Ko chez OpenRouter en novembre 2026).
const MAX_ERROR_BYTES: u64 = 64 * 1024;
const MAX_LISTING_BYTES: u64 = 8 * 1024 * 1024;
/// Ce qu'on garde du message d'erreur d'un fournisseur.
const MAX_REASON_CHARS: usize = 160;
/// Le nombre de pages de la liste des modèles lues au plus. Anthropic en donne jusqu'à mille par
/// page : une seule suffit aujourd'hui.
const MAX_LISTING_PAGES: usize = 5;

/// Les bornes d'un échange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Temps laissé pour établir la connexion.
    pub connect: Duration,
    /// Silence maximal du serveur, avant sa réponse comme entre deux morceaux. Large par
    /// défaut : un modèle local met du temps à se charger, et certains réfléchissent longtemps
    /// avant leur premier mot.
    pub silence: Duration,
    /// Longueur maximale d'une réponse, en caractères. Au-delà, on arrête de lire : Capri parle
    /// dans une bulle, pas dans un livre.
    pub reply_chars: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { connect: Duration::from_secs(10), silence: Duration::from_secs(120), reply_chars: 6_000 }
    }
}

/// Pourquoi une réponse n'est pas arrivée, ou pas en entier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// L'adresse est en `https://`, que ce crate ne sait pas parler sur ce système.
    NoTls,
    /// Personne ne répond à cette adresse.
    Unreachable,
    /// La connexion chiffrée n'a pas pu s'établir : certificat refusé, ou serveur qui ne parle pas
    /// `https`. Rien n'a été envoyé.
    Insecure,
    /// Le serveur est resté silencieux trop longtemps.
    TimedOut,
    /// Le fournisseur demande une clé, et Capri n'en a pas pour cette adresse : rien n'a été
    /// envoyé.
    NoKey,
    /// La clé partirait en clair (`http://` vers une autre machine) : rien n'a été envoyé.
    KeyInClear,
    /// La clé envoyée est refusée (401), ou ne donne pas droit à ce qui est demandé (403).
    KeyRefused { status: u16, reason: Option<String> },
    /// Sans clé, le serveur refuse (401 ou 403) : il en demande une.
    KeyNeeded { status: u16, reason: Option<String> },
    /// Le serveur a répondu autre chose qu'un succès, avec son explication s'il en donne une.
    Refused { status: u16, reason: Option<String> },
    /// Le fournisseur a abandonné en cours de réponse, et dit pourquoi.
    GaveUp(String),
    /// La connexion a été coupée en cours de réponse.
    Cut,
    /// Ce qui est revenu n'est pas une réponse dans le format attendu.
    Unreadable,
    /// Le modèle a répondu sans rien dire.
    Empty,
    /// Le modèle a décliné la demande : la réponse est arrivée sans erreur, mais il refuse d'y
    /// répondre (chez Anthropic, `stop_reason: "refusal"`). Ce qu'il avait commencé à écrire ne
    /// compte pas.
    Declined,
}

impl Problem {
    /// Ce que le journal en garde : la sorte de problème et le code HTTP, jamais le message du
    /// fournisseur, qui pourrait citer ce qu'on lui a dit (un refus de modération, par exemple).
    fn for_log(&self) -> String {
        let kind = match self {
            Self::KeyRefused { status, .. } => return format!("KeyRefused({status})"),
            Self::KeyNeeded { status, .. } => return format!("KeyNeeded({status})"),
            Self::Refused { status, .. } => return format!("Refused({status})"),
            Self::GaveUp(_) => "GaveUp",
            Self::NoTls => "NoTls",
            Self::Unreachable => "Unreachable",
            Self::Insecure => "Insecure",
            Self::TimedOut => "TimedOut",
            Self::NoKey => "NoKey",
            Self::KeyInClear => "KeyInClear",
            Self::Cut => "Cut",
            Self::Unreadable => "Unreadable",
            Self::Empty => "Empty",
            Self::Declined => "Declined",
        };
        kind.to_owned()
    }

    /// La phrase que Capri dit dans sa bulle, qu'affiche la fenêtre de réglages ou la ligne de
    /// commande. Elle ne cite
    /// jamais une clé : le message d'un fournisseur y arrive déjà masqué, et celui d'un refus de
    /// clé (401) n'est pas cité du tout.
    ///
    /// `place` : où elle s'affiche. Le conseil qui la termine en dépend ([`Place`]) : un terminal
    /// reçoit une commande, la bulle et la fenêtre jamais.
    ///
    /// `chosen` : le cerveau des réglages, s'il y en a un. La commande proposée à un terminal pour
    /// donner une clé à `endpoint` n'a besoin de `--url` que si, sans lui, elle viserait une autre
    /// adresse.
    pub fn explain(
        &self,
        place: Place,
        provider: Provider,
        endpoint: &Endpoint,
        chosen: Option<&Brain>,
    ) -> String {
        let host = endpoint.display_host();
        // qui parle : le fournisseur, ou, pour un serveur personnalisé, son adresse
        let (who, whom) = match provider {
            Provider::Custom => (host.clone(), host.clone()),
            _ => (provider.name().to_owned(), format!("{} ({host})", provider.name())),
        };
        let command = key::set_key_command(provider, endpoint, chosen);
        let how_to = |job| place.how_to_key(job, &command);
        match self {
            Self::NoTls => "Je ne sais pas me connecter en https sur ce système.".to_owned(),
            Self::Unreachable if endpoint.reach() == Reach::Internet => {
                format!("Je n'arrive pas à joindre {whom}. La connexion à Internet marche ?")
            }
            Self::Unreachable => format!("Je n'arrive pas à joindre {whom}. Il est lancé ?"),
            Self::Insecure => {
                format!("Je n'ai pas pu établir de connexion sûre avec {whom} : je ne lui ai rien envoyé.")
            }
            Self::TimedOut => format!("{who} met trop de temps à répondre."),
            Self::NoKey => {
                format!(
                    "{who} demande une clé, et je n'en ai pas pour {host}. Pour me la donner : {}",
                    how_to(KeyJob::Give)
                )
            }
            Self::KeyInClear => {
                format!("Je n'envoie pas ma clé en clair à {host} : seulement en https, ou à cette machine.")
            }
            Self::KeyRefused { status: 401, reason } => {
                format!(
                    "{who} refuse ma clé (401){}. Pour la remettre : {}",
                    quote(reason),
                    how_to(KeyJob::PutBack)
                )
            }
            Self::KeyRefused { status, reason } => {
                format!(
                    "{who} refuse ({status}){}. Si c'est ma clé, pour la remettre : {}",
                    quote(reason),
                    how_to(KeyJob::PutBack)
                )
            }
            Self::KeyNeeded { status, reason } => {
                format!(
                    "{who} demande une clé ({status}){}. Pour me la donner : {}",
                    quote(reason),
                    how_to(KeyJob::Give)
                )
            }
            Self::Refused { status, reason: Some(reason) } => {
                format!("{who} refuse ({status}) : {}", brief(reason))
            }
            Self::Refused { status, reason: None } => format!("{who} refuse ({status})."),
            Self::GaveUp(reason) => format!("{who} a abandonné : {}", brief(reason)),
            Self::Cut => "La réponse a été coupée.".to_owned(),
            Self::Unreadable => format!("{who} a répondu quelque chose que je n'ai pas compris."),
            Self::Empty => format!("{who} n'a rien répondu."),
            Self::Declined => phrases::declined(provider),
        }
    }
}

/// « : son message », à glisser avant la suite d'une phrase. Rien s'il n'y a pas de message.
fn quote(reason: &Option<String>) -> String {
    reason.as_deref().map(|reason| format!(" : {}", brief(reason).trim_end_matches('.'))).unwrap_or_default()
}

/// Le message d'un fournisseur, ramené à une ligne courte.
fn brief(reason: &str) -> String {
    let line = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MAX_REASON_CHARS {
        return line;
    }
    let mut short: String = line.chars().take(MAX_REASON_CHARS - 1).collect();
    short.push('…');
    short
}

/// Comment une réponse s'est terminée, quand rien n'a mal tourné.
enum Outcome {
    Complete,
    Cancelled,
}

/// Ce qu'on demande au cerveau, et pour qui.
#[derive(Clone, Copy)]
pub struct Ask<'a> {
    /// Où s'affichera le message de panne : le conseil qui le termine en dépend
    /// ([`Problem::explain`]).
    pub place: Place,
    /// Le cerveau des réglages : c'est pour lui que sont écrits les conseils d'un message de panne
    /// (« Pour me la donner : … »).
    pub brain: &'a Brain,
    /// La clé du fournisseur, s'il en a une. Elle part dans son en-tête (`Authorization`, ou
    /// `x-api-key` pour Anthropic), seulement si le fournisseur en prend une, si elle a été donnée
    /// pour l'adresse du cerveau, et si cette adresse a le droit de la recevoir.
    pub key: Option<&'a BoundKey>,
    pub persona: &'a str,
    pub turns: &'a [Turn],
}

pub struct Client {
    agent: ureq::Agent,
    limits: Limits,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    pub fn with_limits(limits: Limits) -> Self {
        let config = ureq::Agent::config_builder()
            // pas de proxy, même si HTTP_PROXY ou ALL_PROXY sont définies : la conversation ne
            // passe par aucun intermédiaire
            .proxy(None)
            // aucune redirection : on ne parle qu'à l'adresse vérifiée
            .max_redirects(0)
            // un refus se lit (il porte l'explication du fournisseur) au lieu de devenir une erreur
            .http_status_as_error(false)
            .timeout_connect(Some(limits.connect))
            .timeout_per_read(Some(limits.silence));
        // le TLS du système, et les certificats racines du système : ceux que l'utilisateur et
        // son système ont choisi de croire, pas une liste embarquée
        #[cfg(any(target_os = "macos", windows))]
        let config = config.tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        );
        Self { agent: ureq::Agent::new_with_config(config.build()), limits }
    }

    /// Demande au cerveau de répondre à la conversation, et rend sa réponse par morceaux : des
    /// `Delta`, puis `Done` ou `Failed`. Bloque jusqu'à la fin : à appeler hors du thread
    /// principal.
    ///
    /// Ce qu'il faut savoir de la demande : voir [`Ask`].
    ///
    /// Dès que `cancelled` passe à vrai, la lecture s'arrête, la connexion est fermée (le serveur
    /// cesse alors d'écrire) et plus rien n'est émis, pas même `Done`.
    pub fn answer(&self, ask: &Ask, cancelled: &AtomicBool, emit: &mut dyn FnMut(ReplyEvent)) {
        let Ask { place, brain, key, persona, turns } = *ask;
        let outcome =
            self.stream(brain, key, persona, turns, cancelled, &mut |text| emit(ReplyEvent::Delta(text)));
        match outcome {
            Ok(Outcome::Cancelled) => {}
            Ok(Outcome::Complete) => emit(ReplyEvent::Done),
            Err(problem) => {
                log::warn!("pas de réponse du cerveau : {}", problem.for_log());
                emit(ReplyEvent::Failed(problem.explain(
                    place,
                    brain.provider(),
                    brain.endpoint(),
                    Some(brain),
                )));
            }
        }
    }

    /// Les modèles que le serveur de `provider` propose (`GET /models`), avec sa clé s'il en
    /// faut une : la même règle que pour [`Client::answer`]. Une liste en plusieurs pages
    /// (Anthropic) est lue page par page, `MAX_LISTING_PAGES` au plus.
    pub fn models(
        &self,
        provider: Provider,
        endpoint: &Endpoint,
        key: Option<&BoundKey>,
    ) -> Result<Vec<String>, Problem> {
        let wire = provider.wire();
        let header = authorize(provider, endpoint, key)?;
        let sent = key.filter(|_| header.is_some());
        let mut models = Vec::new();
        let mut after: Option<String> = None;
        for _ in 0..MAX_LISTING_PAGES {
            let Some(path) = wire.models_path(after.as_deref()) else { break };
            let mut request = self.agent.get(endpoint.url(&path)).header("Accept", "application/json");
            request = with_headers(request, wire, header.as_ref());
            let response = request.call().map_err(before_the_reply)?;
            let status = response.status().as_u16();
            let body = read_capped(response.into_body().into_reader(), MAX_LISTING_BYTES);
            if !(200..300).contains(&status) {
                return Err(refusal(status, &body, provider, sent));
            }
            let page = wire.model_page(&body).ok_or(Problem::Unreadable)?;
            models.extend(page.ids);
            match page.next {
                Some(next) if after.as_deref() != Some(next.as_str()) => after = Some(next),
                _ => return Ok(models),
            }
        }
        if after.is_some() {
            log::warn!("liste des modèles arrêtée à {} noms", models.len());
        }
        Ok(models)
    }

    fn stream(
        &self,
        brain: &Brain,
        key: Option<&BoundKey>,
        persona: &str,
        turns: &[Turn],
        cancelled: &AtomicBool,
        on_text: &mut dyn FnMut(String),
    ) -> Result<Outcome, Problem> {
        let endpoint = brain.endpoint();
        let wire = brain.provider().wire();
        let header = authorize(brain.provider(), endpoint, key)?;
        // la clé effectivement envoyée : c'est elle qu'il faut masquer dans ce qui revient
        let sent = key.filter(|_| header.is_some());
        let body = wire.request_body(brain.model(), persona, turns);
        let request = self
            .agent
            .post(endpoint.url(wire.reply_path()))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream");
        let request = with_headers(request, wire, header.as_ref());
        let response = request.send(&body).map_err(before_the_reply)?;
        if cancelled.load(Ordering::Relaxed) {
            return Ok(Outcome::Cancelled);
        }
        let status = response.status().as_u16();
        let mut reader = response.into_body().into_reader();
        if !(200..300).contains(&status) {
            return Err(refusal(status, &read_capped(reader, MAX_ERROR_BYTES), brain.provider(), sent));
        }

        let mut decoder = SseDecoder::new();
        let mut packet = [0u8; 4096];
        let (mut said, mut events) = (0usize, 0usize);
        // Lâcher `reader` en sortant d'ici ferme la connexion si la réponse n'est pas finie :
        // c'est ce qui dit au serveur d'arrêter d'écrire.
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Ok(Outcome::Cancelled);
            }
            let received = match reader.read(&mut packet) {
                Ok(0) => break,
                Ok(received) => received,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(during_the_reply(err.into())),
            };
            for data in decoder.feed(&packet[..received]).map_err(|_| Problem::Unreadable)? {
                if cancelled.load(Ordering::Relaxed) {
                    return Ok(Outcome::Cancelled);
                }
                events += 1;
                let piece = wire.interpret(&data);
                if !piece.text.is_empty() {
                    said += piece.text.chars().count();
                    on_text(piece.text);
                }
                match piece.end {
                    Some(End::Failed(reason)) => return Err(Problem::GaveUp(masked(&reason, sent))),
                    Some(End::Declined) => return Err(Problem::Declined),
                    Some(End::Complete) => return finished(said, events),
                    None if said >= self.limits.reply_chars => {
                        log::warn!("réponse arrêtée à {said} caractères");
                        return Ok(Outcome::Complete);
                    }
                    None => {}
                }
            }
        }
        // le serveur a fermé le flux sans annoncer la fin : ce qui est arrivé est la réponse
        if let Some(data) = decoder.finish() {
            events += 1;
            let piece = wire.interpret(&data);
            if !piece.text.is_empty() {
                said += piece.text.chars().count();
                on_text(piece.text);
            }
            match piece.end {
                Some(End::Failed(reason)) => return Err(Problem::GaveUp(masked(&reason, sent))),
                Some(End::Declined) => return Err(Problem::Declined),
                _ => {}
            }
        }
        finished(said, events)
    }
}

/// Les en-têtes propres au format du fournisseur, puis celui de la clé s'il y en a une : tels
/// que `capri-chat` les a décidés.
fn with_headers<B>(
    mut request: ureq::RequestBuilder<B>,
    wire: Wire,
    key: Option<&KeyHeader>,
) -> ureq::RequestBuilder<B> {
    for (name, value) in wire.headers() {
        request = request.header(*name, *value);
    }
    if let Some(key) = key {
        request = request.header(key.name(), key.reveal_for_request());
    }
    request
}

/// L'en-tête de la clé à envoyer, s'il en faut un, une fois vérifié tout ce qui doit l'être avant
/// de se connecter : le https possible sur ce système, la clé donnée pour cette adresse, présente
/// si le fournisseur l'exige, et jamais en clair sur le réseau. Une clé donnée pour une autre
/// adresse compte pour absente. Un fournisseur qui ne prend pas de clé n'en reçoit jamais, même si
/// on lui en confie une.
fn authorize(
    provider: Provider,
    endpoint: &Endpoint,
    key: Option<&BoundKey>,
) -> Result<Option<KeyHeader>, Problem> {
    if endpoint.secure && !TLS {
        return Err(Problem::NoTls);
    }
    if provider.key_need() == KeyNeed::None {
        return Ok(None);
    }
    let authorization = match key.map(|key| (key, key::key_header(key, provider, endpoint))) {
        None => None,
        Some((_, Ok(header))) => Some(header),
        Some((_, Err(KeyError::InClear))) => return Err(Problem::KeyInClear),
        Some((key, Err(_))) => {
            log::warn!(
                "clé donnée pour {}, pas pour {} : elle n'est pas envoyée",
                key.account().address(),
                endpoint.display_host()
            );
            None
        }
    };
    match (authorization, provider.key_need()) {
        (None, KeyNeed::Required) => Err(Problem::NoKey),
        (authorization, _) => Ok(authorization),
    }
}

/// Le refus d'un serveur (`status` hors de 2xx), avec son explication masquée. `sent` : la clé
/// envoyée avec la requête, s'il y en avait une.
fn refusal(status: u16, body: &str, provider: Provider, sent: Option<&BoundKey>) -> Problem {
    let said = provider.wire().error_message(body);
    let reason = said.as_deref().map(|reason| masked(reason, sent));
    match (status, sent) {
        // La clé vient d'être refusée, et la raison du fournisseur dit pourquoi (inconnue, mal
        // formée, révoquée) : on la cite. Certains y rappellent le début ou la fin de la clé :
        // ici, le moindre morceau est masqué.
        (401, Some(key)) => {
            Problem::KeyRefused { status, reason: said.map(|reason| key.redact_any_trace(&brief(&reason))) }
        }
        _ => refusal_of(status, reason, provider, sent),
    }
}

fn refusal_of(status: u16, reason: Option<String>, provider: Provider, sent: Option<&BoundKey>) -> Problem {
    match status {
        401 | 403 if sent.is_some() => Problem::KeyRefused { status, reason },
        401 | 403 if provider.key_need() != KeyNeed::None => Problem::KeyNeeded { status, reason },
        _ => Problem::Refused { status, reason },
    }
}

/// Le message d'un fournisseur, court, et sans rien de la clé envoyée.
fn masked(reason: &str, sent: Option<&BoundKey>) -> String {
    let short = brief(reason);
    match sent {
        Some(key) => key.redact(&short),
        None => short,
    }
}

/// Une réponse terminée sans erreur n'en est une que si elle dit quelque chose.
fn finished(said: usize, events: usize) -> Result<Outcome, Problem> {
    match (said, events) {
        (0, 0) => Err(Problem::Unreadable),
        (0, _) => Err(Problem::Empty),
        _ => Ok(Outcome::Complete),
    }
}

/// Lit un corps de réponse sans dépasser `limit` octets. Ce qui est illisible donne un texte
/// vide ou partiel : c'est à celui qui l'interprète de dire qu'il n'y comprend rien.
fn read_capped(reader: impl Read, limit: u64) -> String {
    let mut bytes = Vec::new();
    if let Err(err) = reader.take(limit).read_to_end(&mut bytes) {
        log::debug!("corps de réponse incomplet : {err}");
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Une erreur survenue avant que le serveur ait commencé à répondre.
fn before_the_reply(error: ureq::Error) -> Problem {
    log::debug!("échec avant la réponse : {error}");
    match error {
        ureq::Error::Timeout(_) => Problem::TimedOut,
        ureq::Error::Io(err) if err.kind() == std::io::ErrorKind::TimedOut => Problem::TimedOut,
        ureq::Error::Protocol(_) | ureq::Error::LargeResponseHeader(..) => Problem::Unreadable,
        ureq::Error::Tls(_) => Problem::Insecure,
        #[cfg(any(target_os = "macos", windows))]
        ureq::Error::NativeTls(_) => Problem::Insecure,
        // connexion refusée, hôte introuvable ou injoignable, connexion fermée sans un mot
        _ => Problem::Unreachable,
    }
}

/// Le temps laissé à une adresse de cette machine pour accepter une connexion.
const KNOCK_TIMEOUT: Duration = Duration::from_secs(1);

/// Ce qu'on trouve à une adresse de cette machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// Quelque chose y accepte les connexions : un serveur tourne là (Ollama, ou un autre).
    Answers,
    /// Personne : la connexion est refusée, ou n'aboutit pas.
    Nobody,
}

/// Pourquoi [`knock`] ne frappe pas à une adresse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnockRefused {
    /// Elle n'est pas sur cette machine : on ne frappe jamais ailleurs.
    NotThisMachine,
    /// Elle est en `https://`, ce que ne sert pas un serveur qu'elle saurait lancer.
    Encrypted,
}

/// Quelque chose répond-il à cette adresse **de cette machine** ? Une connexion TCP, aussitôt
/// fermée : **rien n'est envoyé**, ni requête, ni conversation, ni clé. Sert à savoir s'il faut
/// lancer Ollama (`capri_chat::startup`). Une adresse d'une autre machine, ou en `https://`, est
/// refusée avant toute connexion ; `localhost` n'est pas résolu par le système des noms : ce sont
/// `127.0.0.1` puis `::1`. Bloque au plus une seconde par adresse : hors du thread principal.
pub fn knock(endpoint: &Endpoint) -> Result<Presence, KnockRefused> {
    if endpoint.reach() != Reach::ThisMachine {
        return Err(KnockRefused::NotThisMachine);
    }
    if endpoint.secure {
        return Err(KnockRefused::Encrypted);
    }
    let addresses: Vec<std::net::IpAddr> = match endpoint.host.as_str() {
        "localhost" => vec![std::net::Ipv4Addr::LOCALHOST.into(), std::net::Ipv6Addr::LOCALHOST.into()],
        host => host.parse().into_iter().collect(),
    };
    for address in addresses {
        // `reach` l'a déjà vérifié ; on ne se fie pas qu'à lui avant de se connecter
        let here = match address {
            std::net::IpAddr::V4(v4) => v4.is_loopback(),
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
            }
        };
        if !here {
            return Err(KnockRefused::NotThisMachine);
        }
        let target = std::net::SocketAddr::new(address, endpoint.port);
        match std::net::TcpStream::connect_timeout(&target, KNOCK_TIMEOUT) {
            Ok(_) => return Ok(Presence::Answers),
            Err(err) => log::debug!("personne à {target} : {}", err.kind()),
        }
    }
    Ok(Presence::Nobody)
}

/// Une erreur survenue pendant la lecture de la réponse.
fn during_the_reply(error: ureq::Error) -> Problem {
    log::debug!("échec pendant la réponse : {error}");
    match error {
        ureq::Error::Timeout(_) => Problem::TimedOut,
        ureq::Error::Io(err) if err.kind() == std::io::ErrorKind::TimedOut => Problem::TimedOut,
        ureq::Error::Protocol(_) => Problem::Unreadable,
        _ => Problem::Cut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_conversation_goes_through_no_proxy_and_follows_no_redirect() {
        let client = Client::new();
        let config = client.agent.config();
        assert!(config.proxy().is_none());
        assert_eq!(config.max_redirects(), 0);
        assert!(!config.http_status_as_error());
        assert_eq!(config.timeouts().connect, Some(Limits::default().connect));
        assert_eq!(config.timeout_per_read(), Some(Limits::default().silence));
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn https_goes_through_the_system_s_tls_and_its_certificates() {
        let client = Client::new();
        let tls = client.agent.config().tls_config();
        assert_eq!(tls.provider(), ureq::tls::TlsProvider::NativeTls);
        assert!(matches!(tls.root_certs(), ureq::tls::RootCerts::PlatformVerifier));
        assert!(!tls.disable_verification(), "un certificat est toujours vérifié");
        assert!(tls.use_sni());
    }

    /// Une clé de test, donnée pour cette adresse de ce fournisseur.
    fn bound(provider: Provider, url: &str) -> BoundKey {
        let account = capri_chat::KeyAccount::new(provider, &Endpoint::parse(url).expect("valide"));
        BoundKey::new(account, capri_chat::ApiKey::parse("sk-test-0123456789").expect("une clé"))
    }

    #[test]
    fn each_problem_is_said_in_one_plain_sentence() {
        let endpoint = Endpoint::parse("http://localhost:11434/v1").expect("valide");
        let say = |problem: Problem| problem.explain(Place::Terminal, Provider::Ollama, &endpoint, None);
        assert_eq!(
            say(Problem::Unreachable),
            "Je n'arrive pas à joindre Ollama (localhost:11434). Il est lancé ?"
        );
        assert_eq!(say(Problem::TimedOut), "Ollama met trop de temps à répondre.");
        assert_eq!(
            say(Problem::Refused {
                status: 404,
                reason: Some("model \"x\" not found,\n  try pulling it first".to_owned())
            }),
            "Ollama refuse (404) : model \"x\" not found, try pulling it first"
        );
        assert_eq!(say(Problem::Refused { status: 500, reason: None }), "Ollama refuse (500).");
        assert_eq!(say(Problem::GaveUp("out of memory".to_owned())), "Ollama a abandonné : out of memory");
        assert_eq!(say(Problem::Cut), "La réponse a été coupée.");
        assert_eq!(say(Problem::Unreadable), "Ollama a répondu quelque chose que je n'ai pas compris.");
        assert_eq!(say(Problem::Empty), "Ollama n'a rien répondu.");
        assert_eq!(
            say(Problem::Declined),
            "Le modèle d'Ollama a décliné cette demande. Essaie de la dire autrement."
        );
        assert_eq!(say(Problem::NoTls), "Je ne sais pas me connecter en https sur ce système.");
        assert_eq!(say(Problem::Refused { status: 401, reason: None }), "Ollama refuse (401).");
        assert_eq!(
            say(Problem::Insecure),
            "Je n'ai pas pu établir de connexion sûre avec Ollama (localhost:11434) : je ne lui ai rien envoyé."
        );
    }

    #[test]
    fn a_key_problem_says_how_to_fix_it_without_quoting_the_key() {
        let openrouter = Endpoint::parse("https://openrouter.ai/api/v1").expect("valide");
        let say =
            |problem: Problem| problem.explain(Place::Terminal, Provider::OpenRouter, &openrouter, None);
        assert_eq!(
            say(Problem::NoKey),
            "OpenRouter demande une clé, et je n'en ai pas pour openrouter.ai:443. Pour me la donner : capri chat key openrouter"
        );
        assert_eq!(
            say(Problem::KeyRefused { status: 401, reason: Some("User not found.".to_owned()) }),
            "OpenRouter refuse ma clé (401) : User not found. Pour la remettre : capri chat key openrouter",
            "la raison du fournisseur est citée : c'est elle qui dit si la clé est inconnue ou mal formée"
        );
        assert_eq!(
            say(Problem::KeyRefused { status: 401, reason: None }),
            "OpenRouter refuse ma clé (401). Pour la remettre : capri chat key openrouter"
        );
        assert_eq!(
            say(Problem::KeyRefused { status: 403, reason: Some("Your input was flagged.".to_owned()) }),
            "OpenRouter refuse (403) : Your input was flagged. Si c'est ma clé, pour la remettre : capri chat key openrouter"
        );
        assert_eq!(
            say(Problem::KeyRefused { status: 403, reason: None }),
            "OpenRouter refuse (403). Si c'est ma clé, pour la remettre : capri chat key openrouter"
        );
        assert_eq!(
            say(Problem::Unreachable),
            "Je n'arrive pas à joindre OpenRouter (openrouter.ai:443). La connexion à Internet marche ?"
        );

        let lan = Endpoint::parse("http://192.168.1.20:1234/v1").expect("valide");
        assert_eq!(
            Problem::KeyInClear.explain(Place::Terminal, Provider::Custom, &lan, None),
            "Je n'envoie pas ma clé en clair à 192.168.1.20:1234 : seulement en https, ou à cette machine."
        );
        // un serveur personnalisé est désigné par son adresse, et sa clé se donne avec elle
        assert_eq!(
            Problem::KeyNeeded { status: 401, reason: None }.explain(
                Place::Terminal,
                Provider::Custom,
                &lan,
                None
            ),
            "192.168.1.20:1234 demande une clé (401). Pour me la donner : capri chat key custom --url http://192.168.1.20:1234/v1"
        );
        // … sauf si c'est le cerveau choisi : `capri chat key custom` vise alors son adresse
        let chosen = Brain::new(Provider::Custom, Some("http://192.168.1.20:1234/v1"), "m").expect("valide");
        assert_eq!(
            Problem::KeyNeeded { status: 401, reason: None }.explain(
                Place::Terminal,
                Provider::Custom,
                &lan,
                Some(&chosen)
            ),
            "192.168.1.20:1234 demande une clé (401). Pour me la donner : capri chat key custom"
        );
        assert_eq!(
            Problem::TimedOut.explain(Place::Terminal, Provider::Custom, &lan, None),
            "192.168.1.20:1234 met trop de temps à répondre."
        );
        assert_eq!(
            Problem::Unreachable.explain(Place::Terminal, Provider::Custom, &lan, None),
            "Je n'arrive pas à joindre 192.168.1.20:1234. Il est lancé ?"
        );
        // OpenRouter joint ailleurs que chez lui : sa commande dit où
        let proxy = Endpoint::parse("https://proxy.example/api/v1").expect("valide");
        assert_eq!(
            Problem::NoKey.explain(Place::Terminal, Provider::OpenRouter, &proxy, None),
            "OpenRouter demande une clé, et je n'en ai pas pour proxy.example:443. Pour me la donner : capri chat key openrouter --url https://proxy.example/api/v1"
        );
    }

    #[test]
    fn only_a_terminal_is_sent_to_a_command_to_fix_a_key() {
        let openrouter = Endpoint::parse("https://openrouter.ai/api/v1").expect("valide");
        let lan = Endpoint::parse("http://192.168.1.20:1234/v1").expect("valide");
        let say = |place, problem: Problem| problem.explain(place, Provider::OpenRouter, &openrouter, None);
        let refused = || Problem::KeyRefused { status: 401, reason: None };
        let flagged = || Problem::KeyRefused { status: 403, reason: None };
        // la bulle renvoie aux réglages
        assert_eq!(
            say(Place::Bubble, Problem::NoKey),
            "OpenRouter demande une clé, et je n'en ai pas pour openrouter.ai:443. Pour me la donner : ouvre Réglages… dans mon menu."
        );
        assert_eq!(
            say(Place::Bubble, refused()),
            "OpenRouter refuse ma clé (401). Pour la remettre : ouvre Réglages… dans mon menu."
        );
        assert_eq!(
            say(Place::Bubble, flagged()),
            "OpenRouter refuse (403). Si c'est ma clé, pour la remettre : ouvre Réglages… dans mon menu."
        );
        assert_eq!(
            Problem::KeyNeeded { status: 401, reason: None }.explain(
                Place::Bubble,
                Provider::Custom,
                &lan,
                None
            ),
            "192.168.1.20:1234 demande une clé (401). Pour me la donner : ouvre Réglages… dans mon menu."
        );
        // la fenêtre dit quoi faire sur place
        assert_eq!(
            say(Place::Window, refused()),
            "OpenRouter refuse ma clé (401). Pour la remettre : remets-la ci-dessus."
        );
        assert_eq!(
            say(Place::Window, Problem::NoKey),
            "OpenRouter demande une clé, et je n'en ai pas pour openrouter.ai:443. Pour me la donner : saisis-la ci-dessus."
        );
        // seul le conseil change : le reste de la phrase est le même partout
        for problem in
            [Problem::NoKey, refused(), flagged(), Problem::KeyNeeded { status: 403, reason: None }]
        {
            let terminal = say(Place::Terminal, problem.clone());
            let advice = terminal.rfind(" : ").map(|at| &terminal[..at]).expect("un conseil");
            for place in [Place::Bubble, Place::Window] {
                let said = say(place, problem.clone());
                assert!(said.starts_with(advice) && !said.contains("capri chat"), "{said}");
            }
        }
    }

    #[test]
    fn a_key_is_only_sent_where_it_belongs() {
        let at = |url: &str| Endpoint::parse(url).expect("valide");
        // l'en-tête qui partirait : son nom et sa valeur
        let authorize = |provider, url: &str, key: Option<&BoundKey>| {
            authorize(provider, &at(url), key).map(|header| {
                header.map(|header| format!("{}: {}", header.name(), header.reveal_for_request()))
            })
        };
        let bearer = Ok(Some("Authorization: Bearer sk-test-0123456789".to_owned()));
        // sans TLS sur ce système (Linux), toute adresse en https est refusée avant même de
        // regarder la clé : c'est ce refus-là qu'on y attend
        let over_tls =
            |expected: Result<Option<String>, Problem>| if TLS { expected } else { Err(Problem::NoTls) };

        let openrouter = bound(Provider::OpenRouter, "https://openrouter.ai/api/v1");
        assert_eq!(
            authorize(Provider::OpenRouter, "https://openrouter.ai/api/v1", Some(&openrouter)),
            over_tls(bearer.clone())
        );
        let lmstudio = bound(Provider::LmStudio, "http://localhost:1234/v1");
        assert_eq!(
            authorize(Provider::LmStudio, "http://localhost:1234/v1", Some(&lmstudio)),
            bearer,
            "en clair, mais sans quitter cette machine"
        );
        // jamais en clair sur le réseau, même privé : refusé avant toute connexion
        for url in ["http://192.168.1.20:1234/v1", "http://10.0.0.5:8080", "http://[fd00::1]:1234"] {
            let key = bound(Provider::Custom, url);
            assert_eq!(authorize(Provider::Custom, url, Some(&key)), Err(Problem::KeyInClear), "{url}");
        }
        // une clé donnée pour une autre adresse compte pour absente : rien n'est envoyé
        assert_eq!(
            authorize(Provider::OpenRouter, "https://proxy.example/api/v1", Some(&openrouter)),
            over_tls(Err(Problem::NoKey))
        );
        assert_eq!(authorize(Provider::LmStudio, "http://localhost:1235/v1", Some(&lmstudio)), Ok(None));
        assert_eq!(authorize(Provider::Custom, "http://localhost:1234/v1", Some(&lmstudio)), Ok(None));
        // un fournisseur qui ne prend pas de clé n'en reçoit jamais
        let ollama = bound(Provider::Ollama, "http://localhost:11434/v1");
        assert_eq!(authorize(Provider::Ollama, "http://localhost:11434/v1", Some(&ollama)), Ok(None));
        let ollama = bound(Provider::Ollama, "http://192.168.1.20:11434/v1");
        assert_eq!(authorize(Provider::Ollama, "http://192.168.1.20:11434/v1", Some(&ollama)), Ok(None));
        // sans clé : rien à envoyer, sauf si le fournisseur l'exige
        assert_eq!(authorize(Provider::Custom, "http://192.168.1.20:1234/v1", None), Ok(None));
        for (provider, url) in [
            (Provider::DeepSeek, "https://api.deepseek.com"),
            (Provider::Anthropic, "https://api.anthropic.com/v1"),
        ] {
            assert_eq!(authorize(provider, url, None), over_tls(Err(Problem::NoKey)), "{url}");
        }
        // Anthropic : la clé seule, dans son propre en-tête, et seulement chez lui
        let anthropic = bound(Provider::Anthropic, "https://api.anthropic.com/v1");
        assert_eq!(
            authorize(Provider::Anthropic, "https://api.anthropic.com/v1", Some(&anthropic)),
            over_tls(Ok(Some("x-api-key: sk-test-0123456789".to_owned())))
        );
        assert_eq!(
            authorize(Provider::Anthropic, "https://proxy.example/v1", Some(&anthropic)),
            over_tls(Err(Problem::NoKey))
        );
        assert_eq!(
            authorize(Provider::Custom, "https://api.anthropic.com/v1", Some(&anthropic)),
            over_tls(Ok(None)),
            "ni à un autre fournisseur à la même adresse"
        );
    }

    #[test]
    fn a_refusal_knows_whether_a_key_was_involved() {
        let key = bound(Provider::OpenRouter, "https://openrouter.ai/api/v1");
        let body = r#"{"error":{"message":"bad key sk-test-0123456789"}}"#;
        let masked = Some("bad key [clé masquée]".to_owned());
        assert_eq!(
            refusal(401, body, Provider::OpenRouter, Some(&key)),
            Problem::KeyRefused { status: 401, reason: masked.clone() }
        );
        assert_eq!(
            refusal(403, body, Provider::Custom, Some(&key)),
            Problem::KeyRefused { status: 403, reason: masked.clone() }
        );
        assert_eq!(
            refusal(401, "", Provider::LmStudio, None),
            Problem::KeyNeeded { status: 401, reason: None }
        );
        // Ollama ne prend pas de clé : un 401 n'est qu'un refus parmi d'autres
        // un refus de la clé qui en rappelle le début et la fin : rien n'en reste
        let echoed = r#"{"error":{"message":"Incorrect API key provided: sk-t****6789."}}"#;
        assert_eq!(
            refusal(401, echoed, Provider::OpenRouter, Some(&key)),
            Problem::KeyRefused {
                status: 401,
                reason: Some("Incorrect API key provided: [clé masquée]****[clé masquée].".to_owned())
            }
        );
        // la raison qui dit pourquoi, telle qu'OpenRouter la donne pour une clé inconnue
        assert_eq!(
            refusal(
                401,
                r#"{"error":{"message":"User not found.","code":401}}"#,
                Provider::OpenRouter,
                Some(&key)
            ),
            Problem::KeyRefused { status: 401, reason: Some("User not found.".to_owned()) }
        );
        assert_eq!(refusal(401, "", Provider::Ollama, None), Problem::Refused { status: 401, reason: None });
        assert_eq!(
            refusal(429, body, Provider::OpenRouter, Some(&key)),
            Problem::Refused { status: 429, reason: masked }
        );
    }

    #[test]
    fn the_log_never_keeps_what_a_provider_said() {
        // un refus de modération qui cite ce que l'utilisateur a dit
        let quoted = Some("Your input « mon secret de famille » was flagged".to_owned());
        let problems = [
            Problem::Refused { status: 400, reason: quoted.clone() },
            Problem::KeyRefused { status: 403, reason: quoted.clone() },
            Problem::KeyNeeded { status: 401, reason: quoted },
            Problem::GaveUp("mon secret de famille".to_owned()),
        ];
        let logged: Vec<String> = problems.iter().map(Problem::for_log).collect();
        assert_eq!(logged, ["Refused(400)", "KeyRefused(403)", "KeyNeeded(401)", "GaveUp"]);
        assert_eq!(Problem::Unreachable.for_log(), "Unreachable");
        assert_eq!(Problem::Declined.for_log(), "Declined");
    }

    #[test]
    fn a_provider_s_long_message_is_cut_to_one_short_line() {
        let long = "mot ".repeat(200);
        let short = brief(&long);
        assert_eq!(short.chars().count(), MAX_REASON_CHARS);
        assert!(short.ends_with('…') && !short.contains('\n'));
        assert_eq!(brief("  déjà\tcourt\n"), "déjà court");
    }

    #[test]
    fn an_answer_must_say_something() {
        assert!(matches!(finished(12, 3), Ok(Outcome::Complete)));
        assert_eq!(finished(0, 3).err(), Some(Problem::Empty));
        assert_eq!(finished(0, 0).err(), Some(Problem::Unreadable));
    }
}
