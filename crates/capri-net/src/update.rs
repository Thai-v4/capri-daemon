//! Lire le manifeste de la dernière version publiée : une entrée **à part** de la conversation.
//!
//! Elle ne sait faire qu'une chose : un `GET` sans corps, vers l'adresse que la règle permet
//! (`capri_chat::update::Route`, prise en paramètre : celle de GitHub en vrai, celle de cette
//! machine dans les essais), en suivant elle-même, une à une, les redirections que la règle permet
//! (ureq reste à zéro redirection), et rend les octets du manifeste. Ce qu'ils disent est décidé
//! par `capri_chat::update`.
//!
//! Ce qu'elle garantit :
//! - **aucune clé ne peut partir** par ici : ni paramètre de clé, ni [`crate::Ask`], ni en-tête
//!   d'autorisation, et aucun magasin de cookies (ureq est compilé sans) ;
//! - **rien sur l'utilisateur ni sur la machine** : la requête n'a que sa ligne et l'en-tête
//!   `Host`. ureq enverrait de lui-même `User-Agent: ureq/<version>` et `Accept: */*` : les deux
//!   sont retirés (sans `Accept`, le serveur comprend « n'importe quel type », RFC 9110,
//!   section 12.5.1 ; et GitHub sert ces fichiers sans `User-Agent`, vérifié le 23 novembre 2026).
//!   Aucun en-tête ne présente l'app. `Accept-Encoding` n'est pas envoyé (ureq est compilé sans
//!   décompression) ;
//! - pas de proxy, même si l'environnement en déclare un ; aucune connexion gardée ouverte entre
//!   deux vérifications ;
//! - chaque adresse (celle de départ, puis chaque redirection) est vérifiée par la règle **avant**
//!   de s'y connecter : `https` (le clair seulement vers cette machine), hôtes d'une liste fermée,
//!   port de la règle, pas d'identifiants, trois redirections au plus ;
//! - tout est borné : le temps de connexion, la durée de chaque échange, la taille lue
//!   (`capri_chat::update::manifest::MAX_BYTES`, refusée dès l'en-tête `Content-Length` s'il
//!   l'annonce plus grande) ;
//! - une adresse signée (celle du fichier, chez GitHub) n'est jamais écrite : ni dans un message,
//!   ni dans le journal, qui ne dit que l'hôte d'une redirection. ureq, de son côté, n'écrit au
//!   niveau `debug` que l'hôte (le chemin et l'en-tête `Location` y sont masqués, `util.rs` de
//!   ureq 3.5) ; ses traces `trace`, qui les écriraient, sont refusées par le journal de l'app.
//!
//! Sous Linux, pas de `https` : la règle de GitHub y est refusée avant toute connexion.

use std::io::Read;
use std::time::Duration;

use capri_chat::update::manifest::MAX_BYTES;
use capri_chat::update::{Failure, Route, RouteError};
use ureq::config::AutoHeaderValue;

/// Les bornes d'une lecture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Temps laissé pour établir chaque connexion, TLS compris.
    pub connect: Duration,
    /// Durée de chaque échange (une redirection ou le manifeste), de la connexion à la fin de la
    /// lecture.
    pub per_call: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self { connect: Duration::from_secs(10), per_call: Duration::from_secs(30) }
    }
}

/// Lit le manifeste à `url`, en suivant les redirections que `route` permet. Rend ses octets, au
/// plus `MAX_BYTES`. Bloque le temps de la lecture (bornée par `limits`) : hors du thread
/// principal.
pub fn fetch(route: &Route, url: &str, limits: Limits) -> Result<Vec<u8>, Failure> {
    let mut address = route.check(url).map_err(Failure::NotAllowed)?;
    let agent = agent(route, limits);
    let mut redirects = 0;
    loop {
        if address.is_secure() && !crate::TLS {
            return Err(Failure::NoTls);
        }
        let response = agent.get(address.reveal_for_request()).call().map_err(failure)?;
        match response.status().as_u16() {
            200 => return body(response),
            301 | 302 | 303 | 307 | 308 => {
                if redirects >= route.max_redirects() {
                    return Err(Failure::TooManyRedirects);
                }
                redirects += 1;
                let location = response.headers().get("location").and_then(|value| value.to_str().ok());
                let location = location.ok_or(Failure::Unreadable)?;
                address = route.follow(&address, location).map_err(Failure::NotAllowed)?;
                log::debug!("mises à jour : redirection n° {redirects} vers {}", address.host());
            }
            status => return Err(Failure::Status(status)),
        }
    }
}

/// L'agent d'une lecture : rien de plus qu'un `GET` (voir le haut du module).
fn agent(route: &Route, limits: Limits) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        // pas de proxy, même si HTTP_PROXY ou ALL_PROXY sont définies
        .proxy(None)
        // les redirections sont suivies ici, une à une, par la règle : jamais par ureq
        .max_redirects(0)
        .http_status_as_error(false)
        .https_only(route.is_secure())
        // rien qui présente l'app ni la machine
        .user_agent(AutoHeaderValue::None)
        .accept(AutoHeaderValue::None)
        .accept_encoding(AutoHeaderValue::None)
        // aucune connexion gardée ouverte : la suivante est dans un jour
        .max_idle_connections(0)
        .timeout_connect(Some(limits.connect))
        .timeout_per_call(Some(limits.per_call));
    // le TLS du système et ses certificats racines, comme pour la conversation
    #[cfg(any(target_os = "macos", windows))]
    let config = config.tls_config(
        ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build(),
    );
    ureq::Agent::new_with_config(config.build())
}

/// Le corps d'une réponse 200 : au plus `MAX_BYTES` octets, sinon un refus.
fn body(response: ureq::http::Response<ureq::Body>) -> Result<Vec<u8>, Failure> {
    let limit = MAX_BYTES as u64;
    if response.body().content_length().is_some_and(|length| length > limit) {
        return Err(Failure::TooLarge);
    }
    let mut bytes = Vec::new();
    let read = response.into_body().into_reader().take(limit + 1).read_to_end(&mut bytes);
    match read {
        Ok(_) if bytes.len() as u64 > limit => Err(Failure::TooLarge),
        Ok(_) => Ok(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::TimedOut => Err(Failure::TimedOut),
        // coupée en route, ou plus courte que ce qu'elle annonçait
        Err(_) => Err(Failure::Unreadable),
    }
}

/// Une erreur de ureq, en cause courte. Son texte n'est jamais écrit : celui d'une adresse
/// refusée (`BadUri`) la citerait en entier.
fn failure(error: ureq::Error) -> Failure {
    match error {
        ureq::Error::Timeout(_) => Failure::TimedOut,
        ureq::Error::Io(err) if err.kind() == std::io::ErrorKind::TimedOut => Failure::TimedOut,
        ureq::Error::Tls(_) => Failure::Insecure,
        #[cfg(any(target_os = "macos", windows))]
        ureq::Error::NativeTls(_) => Failure::Insecure,
        ureq::Error::Protocol(_) | ureq::Error::LargeResponseHeader(..) => Failure::Unreadable,
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => Failure::NotAllowed(RouteError::Malformed),
        ureq::Error::RequireHttpsOnly(_) => Failure::NotAllowed(RouteError::Scheme),
        // connexion refusée, hôte introuvable ou injoignable, connexion fermée sans un mot
        _ => Failure::Unreachable,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::mpsc::{Receiver, channel};
    use std::time::Instant;

    use capri_chat::update::{Manifest, manifest_url};

    use super::*;

    /// Ce que répond le faux serveur à une connexion.
    enum Reply {
        /// Ces octets, puis la connexion est fermée.
        Send(String),
        /// Rien : la connexion reste ouverte, muette.
        Silent,
    }

    /// Un faux serveur, sur cette machine : une connexion par réponse, dans l'ordre ; il rend
    /// l'en-tête de chaque requête reçue. Rien ne sort de la machine.
    fn serve(replies: Vec<Reply>) -> (SocketAddr, Receiver<String>) {
        serve_at(move |_| replies)
    }

    /// Le même, dont les réponses connaissent son adresse (une redirection écrite en entier).
    fn serve_at(replies: impl FnOnce(SocketAddr) -> Vec<Reply>) -> (SocketAddr, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
        let address = listener.local_addr().expect("son adresse");
        let replies = replies(address);
        let (sender, requests) = channel();
        std::thread::spawn(move || {
            let mut silent: Vec<TcpStream> = Vec::new();
            for (stream, reply) in listener.incoming().zip(replies) {
                let Ok(mut stream) = stream else { return };
                let (mut bytes, mut buffer) = (Vec::new(), [0u8; 4096]);
                while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(received) if received > 0 => bytes.extend_from_slice(&buffer[..received]),
                        _ => return,
                    }
                }
                let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
                match reply {
                    Reply::Send(response) => {
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Reply::Silent => silent.push(stream),
                }
            }
            // les connexions muettes restent ouvertes le temps que l'essai abandonne
            std::thread::sleep(Duration::from_secs(10));
            drop(silent);
        });
        (address, requests)
    }

    fn redirect(status: &str, location: &str) -> Reply {
        Reply::Send(format!(
            "HTTP/1.1 {status}\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ))
    }

    fn ok(body: &str) -> Reply {
        Reply::Send(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ))
    }

    fn here(port: u16) -> Route {
        Route::this_machine(port).expect("la règle de cette machine")
    }

    /// Les essais attendent vite : une seconde au plus par échange.
    const QUICK: Limits = Limits { connect: Duration::from_secs(1), per_call: Duration::from_secs(1) };

    const MANIFEST: &str = r#"{"format": 1, "version": "0.2.0", "bundle": 1, "targets": {}}"#;

    #[test]
    fn the_manifest_is_reached_through_the_redirections_the_rule_permits() {
        // comme GitHub : la dernière version, puis la version, puis une adresse signée
        let (address, requests) = serve(vec![
            redirect("302 Found", "/Thai-v4/capri-daemon/releases/download/v0.2.0/latest.json"),
            redirect("302 Found", "/signed/latest.json?sig=SeCrEt%3D&se=2026"),
            ok(MANIFEST),
        ]);
        let start = format!("http://{address}/Thai-v4/capri-daemon/releases/latest/download/latest.json");
        let bytes = fetch(&here(address.port()), &start, QUICK).expect("le manifeste");
        assert_eq!(bytes, MANIFEST.as_bytes());
        assert!(matches!(Manifest::parse(&bytes), Ok(Manifest::Known(_))));
        let received: Vec<String> = requests.try_iter().collect();
        let lines: Vec<&str> =
            received.iter().map(|request| request.lines().next().unwrap_or_default()).collect();
        assert_eq!(
            lines,
            [
                "GET /Thai-v4/capri-daemon/releases/latest/download/latest.json HTTP/1.1",
                "GET /Thai-v4/capri-daemon/releases/download/v0.2.0/latest.json HTTP/1.1",
                "GET /signed/latest.json?sig=SeCrEt%3D&se=2026 HTTP/1.1",
            ]
        );
        // rien d'autre que sa ligne et l'hôte : ni l'app, ni la machine, ni clé, ni corps
        for request in &received {
            let headers: Vec<String> = request
                .lines()
                .skip(1)
                .filter(|line| !line.is_empty())
                .map(|line| line.split(':').next().unwrap_or_default().to_ascii_lowercase())
                .collect();
            assert_eq!(headers, ["host"], "{request}");
            assert!(request.ends_with("\r\n\r\n"), "aucun corps : {request:?}");
        }
    }

    #[test]
    fn an_absolute_redirection_to_the_same_machine_and_port_is_followed() {
        // une redirection écrite en entier, vers la même machine et le même port
        let (address, requests) = serve_at(|address| {
            vec![
                redirect("301 Moved Permanently", &format!("http://{address}/v0.2.0/latest.json")),
                ok(MANIFEST),
            ]
        });
        let bytes = fetch(&here(address.port()), &format!("http://{address}/latest.json"), QUICK);
        assert_eq!(bytes.expect("suivie"), MANIFEST.as_bytes());
        let received: Vec<String> = requests.try_iter().collect();
        assert_eq!(received.len(), 2);
        assert!(received[1].starts_with("GET /v0.2.0/latest.json HTTP/1.1\r\n"), "{}", received[1]);
    }

    #[test]
    fn a_redirection_the_rule_does_not_permit_is_refused_before_connecting() {
        for (location, refusal) in [
            ("http://localhost:1/x", RouteError::Host("localhost".to_owned())),
            ("http://127.0.0.2:1/x", RouteError::Host("127.0.0.2".to_owned())),
            ("https://github.com/x", RouteError::Malformed),
            ("http://user@127.0.0.1:1/x", RouteError::Credentials),
            ("//evil.example/x", RouteError::Malformed),
        ] {
            let (address, requests) = serve(vec![redirect("302 Found", location)]);
            let url = format!("http://{address}/latest.json");
            assert_eq!(
                fetch(&here(address.port()), &url, QUICK),
                Err(Failure::NotAllowed(refusal)),
                "{location}"
            );
            assert_eq!(requests.try_iter().count(), 1, "rien d'autre n'est demandé : {location}");
        }
        // le port aussi est celui de la règle
        let (address, _) = serve(vec![redirect("307 Temporary Redirect", "http://127.0.0.1:1/x")]);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::NotAllowed(RouteError::Port)));
        // l'adresse de départ, de même : refusée sans connexion
        assert_eq!(
            fetch(&Route::github(), "http://127.0.0.1:1/latest.json", QUICK),
            Err(Failure::NotAllowed(RouteError::Scheme))
        );
        assert_eq!(
            fetch(&here(1), "http://127.0.0.1:2/latest.json", QUICK),
            Err(Failure::NotAllowed(RouteError::Port))
        );
    }

    #[test]
    fn more_than_three_redirections_are_not_followed() {
        let hops = (0..4).map(|n| redirect("302 Found", &format!("/saut-{n}"))).collect();
        let (address, requests) = serve(hops);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::TooManyRedirects));
        assert_eq!(requests.try_iter().count(), 4, "le départ et trois redirections suivies");
        // et une règle qui n'en permet aucune
        let (address, _) = serve(vec![redirect("302 Found", "/x")]);
        let none = Route::new(false, &["127.0.0.1"], address.port(), 0).expect("une règle");
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&none, &url, QUICK), Err(Failure::TooManyRedirects));
    }

    #[test]
    fn a_response_too_big_for_a_manifest_is_refused() {
        let big = "x".repeat(MAX_BYTES + 1);
        // annoncée trop grosse
        let (address, _) = serve(vec![ok(&big)]);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::TooLarge));
        // sans le dire : lue jusqu'à la borne, pas au-delà
        let (address, _) =
            serve(vec![Reply::Send(format!("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{big}"))]);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::TooLarge));
        // tout juste à la borne : lue
        let (address, _) = serve(vec![ok(&big[1..])]);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK).map(|bytes| bytes.len()), Ok(MAX_BYTES));
    }

    #[test]
    fn a_silent_server_is_given_up_on_in_time() {
        let (address, _) = serve(vec![Reply::Silent]);
        let url = format!("http://{address}/latest.json");
        let started = Instant::now();
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::TimedOut));
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    }

    #[test]
    fn other_answers_are_said_not_guessed() {
        let (address, _) = serve(vec![Reply::Send(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot Found".to_owned(),
        )]);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::Status(404)));
        // une redirection sans adresse
        let (address, _) = serve(vec![Reply::Send(
            "HTTP/1.1 302 Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
        )]);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::Unreadable));
        // un autre succès que 200 n'est pas un manifeste
        let (address, _) =
            serve(vec![Reply::Send("HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_owned())]);
        let url = format!("http://{address}/latest.json");
        assert_eq!(fetch(&here(address.port()), &url, QUICK), Err(Failure::Status(204)));
        // personne. Sous Windows, la connexion à un port de cette machine où personne n'écoute
        // n'échoue pas tout de suite : la borne d'une seconde tombe d'abord (observé dans la CI le
        // 23 novembre 2026, `TimedOut` au lieu d'`Unreachable`). Les deux disent « personne ».
        let port = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("un port").port();
        let url = format!("http://127.0.0.1:{port}/latest.json");
        let nobody = fetch(&here(port), &url, QUICK);
        if cfg!(windows) {
            assert!(matches!(nobody, Err(Failure::Unreachable | Failure::TimedOut)), "{nobody:?}");
        } else {
            assert_eq!(nobody, Err(Failure::Unreachable));
        }
    }

    #[test]
    fn https_never_falls_back_to_clear_and_is_refused_where_there_is_no_tls() {
        // une règle en https vers cette machine, face à un serveur qui répond en clair dès la
        // connexion, sans attendre la poignée de main du TLS
        let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
        let address = listener.local_addr().expect("son adresse");
        std::thread::spawn(move || {
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                let _ = stream.write_all(
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{MANIFEST}", MANIFEST.len())
                        .as_bytes(),
                );
                std::thread::sleep(Duration::from_secs(2));
            }
        });
        let secure = Route::new(true, &["127.0.0.1"], address.port(), 3).expect("une règle");
        let url = format!("https://{address}/latest.json");
        let expected = if crate::TLS { Failure::Insecure } else { Failure::NoTls };
        assert_eq!(fetch(&secure, &url, QUICK), Err(expected));
        // la règle de GitHub, sous Linux : refusée avant toute connexion
        if !crate::TLS {
            assert_eq!(fetch(&Route::github(), &manifest_url(), QUICK), Err(Failure::NoTls));
        }
    }

    #[test]
    fn the_agent_carries_nothing_but_the_request() {
        let github = agent(&Route::github(), Limits::default());
        let config = github.config();
        assert!(config.proxy().is_none());
        assert_eq!(config.max_redirects(), 0);
        assert!(!config.http_status_as_error());
        assert!(config.https_only());
        assert!(matches!(config.user_agent(), AutoHeaderValue::None));
        assert!(matches!(config.accept(), AutoHeaderValue::None));
        assert!(matches!(config.accept_encoding(), AutoHeaderValue::None));
        assert_eq!(config.max_idle_connections(), 0);
        assert_eq!(config.timeouts().connect, Some(Limits::default().connect));
        assert_eq!(config.timeouts().per_call, Some(Limits::default().per_call));
        #[cfg(any(target_os = "macos", windows))]
        {
            let tls = config.tls_config();
            assert_eq!(tls.provider(), ureq::tls::TlsProvider::NativeTls);
            assert!(matches!(tls.root_certs(), ureq::tls::RootCerts::PlatformVerifier));
            assert!(!tls.disable_verification());
        }
        assert!(!agent(&here(1), Limits::default()).config().https_only(), "les essais, en clair");
    }
}
