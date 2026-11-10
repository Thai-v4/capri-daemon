//! À quelle adresse Capri a le droit d'envoyer une conversation.
//!
//! Ce qu'on lui dit part chez le fournisseur choisi : l'adresse compte. Deux règles, vérifiées
//! avant toute connexion :
//! - `https://` vers n'importe où ;
//! - `http://`, en clair, seulement vers cette machine ou vers un réseau privé, donné par son
//!   adresse IP écrite en entier. Un nom d'hôte autre que `localhost` est refusé en clair : rien
//!   ne garantit vers quoi il se résout.
//!
//! L'adresse est lue par un analyseur volontairement étroit, qui refuse ce qu'il ne connaît pas
//! (identifiants dans l'adresse, paramètres, caractères inattendus) plutôt que de deviner.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EndpointError {
    #[error("l'adresse doit commencer par http:// ou https://")]
    Scheme,
    #[error("l'adresse est mal formée")]
    Malformed,
    #[error("l'adresse ne doit pas contenir d'identifiants")]
    Credentials,
    #[error("l'adresse ne doit contenir ni « ? » ni « # »")]
    Query,
    #[error(
        "« {0} » n'est ni cette machine ni une adresse de réseau privé : en http://, la conversation \
         partirait en clair"
    )]
    PlainHttpToPublicHost(String),
}

/// Une adresse de base acceptée, par exemple `http://localhost:11434/v1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    /// `true` pour `https://`.
    pub secure: bool,
    /// Nom d'hôte ou adresse IP, en minuscules, sans les crochets d'une adresse IPv6.
    pub host: String,
    pub port: u16,
    /// Chemin de base, sans barre finale : `/v1`, ou vide.
    pub path: String,
}

impl Endpoint {
    /// Lit une adresse de base et vérifie qu'on a le droit d'y envoyer une conversation.
    pub fn parse(url: &str) -> Result<Self, EndpointError> {
        let (scheme, rest) = url.split_once("://").ok_or(EndpointError::Scheme)?;
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "http" => false,
            "https" => true,
            _ => return Err(EndpointError::Scheme),
        };
        if rest.contains(['?', '#']) {
            return Err(EndpointError::Query);
        }
        let (authority, path) = match rest.find('/') {
            Some(slash) => rest.split_at(slash),
            None => (rest, ""),
        };
        if authority.contains('@') {
            return Err(EndpointError::Credentials);
        }
        let (host, port) = split_host_port(authority)?;
        let port = match port {
            Some(port) => port.parse::<u16>().ok().filter(|&p| p != 0).ok_or(EndpointError::Malformed)?,
            None if secure => 443,
            None => 80,
        };
        let path_is_plain =
            path.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~'));
        if !path_is_plain || path.contains("//") || path.split('/').any(|part| part == "..") {
            return Err(EndpointError::Malformed);
        }
        if !secure && !is_local_or_private(&host) {
            return Err(EndpointError::PlainHttpToPublicHost(host));
        }
        Ok(Self { secure, host, port, path: path.trim_end_matches('/').to_owned() })
    }

    /// L'adresse complète d'une ressource sous l'adresse de base : `url("/chat/completions")`.
    pub fn url(&self, resource: &str) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        let default_port = if self.secure { 443 } else { 80 };
        let port = if self.port == default_port { String::new() } else { format!(":{}", self.port) };
        format!("{scheme}://{host}{port}{}{resource}", self.path)
    }

    /// De quoi désigner le serveur dans un message : `localhost:11434`.
    pub fn display_host(&self) -> String {
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        format!("{host}:{}", self.port)
    }
}

/// Sépare l'hôte du port. Rend l'hôte en minuscules, sans crochets.
fn split_host_port(authority: &str) -> Result<(String, Option<&str>), EndpointError> {
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (address, after) = bracketed.split_once(']').ok_or(EndpointError::Malformed)?;
        let address: Ipv6Addr = address.parse().map_err(|_| EndpointError::Malformed)?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':').ok_or(EndpointError::Malformed)?),
        };
        return Ok((address.to_string(), port));
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let labels_are_plain = host
        .split('.')
        .all(|label| !label.is_empty() && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    if host.len() > 253 || !labels_are_plain {
        return Err(EndpointError::Malformed);
    }
    Ok((host.to_ascii_lowercase(), port))
}

/// Cette machine, ou une adresse qui ne sort pas d'un réseau privé ?
fn is_local_or_private(host: &str) -> bool {
    // `localhost` exactement : le système le résout sans rien demander au réseau, ce qui n'est pas
    // garanti pour `quelquechose.localhost`.
    if host == "localhost" {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => is_private_v4(address),
        Ok(IpAddr::V6(address)) => match address.to_ipv4_mapped() {
            Some(mapped) => is_private_v4(mapped),
            None => {
                address.is_loopback()
                    || address.segments()[0] & 0xfe00 == 0xfc00 // fc00::/7, adresses locales uniques
                    || address.segments()[0] & 0xffc0 == 0xfe80 // fe80::/10, lien local
            }
        },
        // un nom : on ne sait pas où il mène
        Err(_) => false,
    }
}

fn is_private_v4(address: Ipv4Addr) -> bool {
    let [a, b, ..] = address.octets();
    address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        // 100.64.0.0/10 : les adresses partagées qu'utilisent les réseaux privés virtuels (Tailscale)
        || (a == 100 && b & 0xc0 == 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(url: &str) -> Endpoint {
        Endpoint::parse(url).unwrap_or_else(|err| panic!("{url} devrait passer : {err}"))
    }

    #[test]
    fn reads_scheme_host_port_and_path() {
        let ollama = ok("http://localhost:11434/v1");
        assert_eq!(
            ollama,
            Endpoint { secure: false, host: "localhost".to_owned(), port: 11434, path: "/v1".to_owned() }
        );
        assert_eq!(ollama.url("/chat/completions"), "http://localhost:11434/v1/chat/completions");
        assert_eq!(ollama.display_host(), "localhost:11434");

        let remote = ok("HTTPS://OpenRouter.ai/api/v1/");
        assert_eq!((remote.secure, remote.host.as_str(), remote.port), (true, "openrouter.ai", 443));
        assert_eq!(remote.url("/models"), "https://openrouter.ai/api/v1/models", "sans double barre");

        assert_eq!(
            ok("https://api.deepseek.com").url("/chat/completions"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(ok("http://127.0.0.1").port, 80);
        assert_eq!(ok("http://[::1]:1234/v1").url("/models"), "http://[::1]:1234/v1/models");
        assert_eq!(ok("https://example.com:8443").url(""), "https://example.com:8443");
    }

    #[test]
    fn plain_http_only_reaches_this_machine_or_a_private_network() {
        for url in [
            "http://localhost:11434/v1",
            "http://127.0.0.1:1234/v1",
            "http://127.8.9.10",
            "http://[::1]:11434",
            "http://10.0.0.5:11434",
            "http://172.16.0.1",
            "http://172.31.255.254",
            "http://192.168.1.20:11434/v1",
            "http://169.254.10.10",
            "http://100.64.0.1:11434",
            "http://100.127.255.254",
            "http://[fd7a:115c:a1e0::1]:11434",
            "http://[fe80::1]",
            "http://[::ffff:192.168.1.20]",
        ] {
            assert!(!ok(url).secure, "{url}");
        }
        for url in [
            "http://example.com/v1",
            "http://api.openai.com",
            "http://tesseract:11434", // un nom : on ne sait pas où il mène
            "http://tesseract.local:11434",
            "http://localhost.example.com",
            "http://ollama.localhost:11434",
            "http://8.8.8.8",
            "http://172.32.0.1",
            "http://100.128.0.1",
            "http://100.63.255.255",
            "http://0.0.0.0:11434",
            "http://[2001:db8::1]",
            "http://[::ffff:8.8.8.8]",
            "http://[::]",
        ] {
            assert!(
                matches!(Endpoint::parse(url), Err(EndpointError::PlainHttpToPublicHost(_))),
                "{url} : {:?}",
                Endpoint::parse(url)
            );
        }
        // en https, la destination est libre
        assert!(ok("https://example.com/v1").secure);
        assert!(ok("https://8.8.8.8").secure);
    }

    #[test]
    fn refuses_what_it_does_not_understand() {
        use EndpointError::*;
        for (url, expected) in [
            ("localhost:11434", Scheme),
            ("ftp://localhost", Scheme),
            ("file:///etc/passwd", Scheme),
            ("//localhost", Scheme),
            ("", Scheme),
            ("http://", Malformed),
            ("http://:11434", Malformed),
            ("http://localhost:", Malformed),
            ("http://localhost:0", Malformed),
            ("http://localhost:99999", Malformed),
            ("http://localhost:11434x", Malformed),
            ("http://local host", Malformed),
            ("http://localhost/v 1", Malformed),
            ("http://localhost/v1/../admin", Malformed),
            ("http://localhost//v1", Malformed),
            ("http://localhost/%2e%2e", Malformed),
            ("http://exa_mple.com", Malformed),
            ("http://.localhost", Malformed),
            ("http://[::1", Malformed),
            ("http://[::1]11434", Malformed),
            ("http://[pas-une-adresse]", Malformed),
            ("http://localhost\\@example.com", Credentials),
            ("http://user:secret@localhost:11434", Credentials),
            ("https://clé@example.com", Credentials),
            ("http://localhost/v1?key=secret", Query),
            ("http://localhost/v1#ancre", Query),
        ] {
            assert_eq!(Endpoint::parse(url), Err(expected), "{url}");
        }
    }

    #[test]
    fn a_public_host_cannot_pass_for_a_local_one() {
        // des écritures qu'un analyseur plus laxiste lirait autrement
        for url in [
            "http://127.0.0.1.example.com",
            "http://2130706433",
            "http://0x7f.1",
            "http://127.1",
            "http://010.0.0.1", // 8.0.0.1 pour qui lit de l'octal
            "http://192.168.1",
        ] {
            assert!(Endpoint::parse(url).is_err(), "{url}");
        }
        assert_eq!(
            Endpoint::parse("http://127.0.0.1.example.com"),
            Err(EndpointError::PlainHttpToPublicHost("127.0.0.1.example.com".to_owned()))
        );
    }
}
