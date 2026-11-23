//! La règle des adresses que suit la lecture du manifeste : celle de départ, et chaque
//! redirection.
//!
//! `capri-net` ne suit aucune redirection de lui-même (ureq y reste à zéro) : c'est lui qui les
//! suit, une à une, en demandant à cette règle si la suivante est permise. Elle est **prise en
//! paramètre** : la vraie ([`Route::github`]) ne permet que GitHub ; les essais en donnent une
//! autre, qui ne permet que cette machine ([`Route::this_machine`]).
//!
//! Ce qu'elle exige, d'une adresse comme de chaque redirection :
//! - au plus [`MAX_REDIRECTS`] redirections ;
//! - `https` (en clair, seulement vers une adresse de boucle locale de cette machine, écrite en
//!   chiffres : c'est le cas des essais, et de lui seul) ;
//! - un hôte d'une liste fermée, comparé en entier (pas de sous-domaine, pas de point final) ;
//! - le port de la règle (443 pour GitHub), écrit ou non ;
//! - pas d'identifiants dans l'adresse (`utilisateur@hôte`) ;
//! - une adresse courte (au plus [`MAX_URL_CHARS`] caractères), faite de caractères permis dans
//!   une adresse (RFC 3986), sans espace ni caractère de contrôle.
//!
//! Une redirection relative (`/chemin…`) reste chez le même hôte ; toute autre forme relative est
//! refusée.
//!
//! Une adresse permise ([`Address`]) peut être signée (celles des fichiers de GitHub le sont :
//! leur requête porte une signature) : elle ne s'écrit jamais en entier dans un message, ni dans
//! le journal. Son `Debug` et les refus de cette règle n'en montrent que l'hôte.

use std::fmt;
use std::net::Ipv4Addr;

/// Le nombre de redirections suivies, au plus.
pub const MAX_REDIRECTS: u8 = 3;
/// La longueur d'une adresse, au plus. Celles que signe GitHub pour un fichier font un bon millier
/// de caractères.
pub const MAX_URL_CHARS: usize = 8 * 1024;

/// Les hôtes qui servent les fichiers d'une version publiée sur GitHub.
///
/// - `github.com` : l'adresse de départ (`/releases/latest/download/…`), puis celle de la version
///   (`/releases/download/<étiquette>/…`), vers lesquelles GitHub renvoie (relevé le 23 novembre
///   2026 sur la version publique d'un autre projet) ;
/// - `release-assets.githubusercontent.com` : l'adresse signée du fichier lui-même. La
///   documentation de GitHub le nomme comme le seul domaine « Needed for downloading release
///   assets » (*Self-hosted runners reference*, section « Accessible domains by function »,
///   <https://docs.github.com/en/actions/reference/runners/self-hosted-runners>, lue le
///   23 novembre 2026). La même page cite `objects.githubusercontent.com` et
///   `github-releases.githubusercontent.com`, mais « for runner version updates » : ils ne sont
///   pas dans la liste.
pub const GITHUB_HOSTS: [&str; 2] = ["github.com", "release-assets.githubusercontent.com"];

/// Pourquoi une adresse n'est pas suivie. Ne cite jamais que l'hôte, jamais le chemin ni la
/// requête (celle d'une adresse signée).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
    #[error("pas en https")]
    Scheme,
    #[error("des identifiants dans l'adresse")]
    Credentials,
    #[error("hôte hors de la liste ({0})")]
    Host(String),
    #[error("port hors de la règle")]
    Port,
    #[error("adresse trop longue")]
    TooLong,
    #[error("adresse mal formée")]
    Malformed,
}

/// Une règle mal écrite (par un essai : la vraie est écrite ici).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("plus de {MAX_REDIRECTS} redirections")]
    TooManyRedirects,
    #[error("aucun hôte, ou un hôte mal écrit")]
    Hosts,
    #[error("en clair ailleurs que vers cette machine")]
    PlainHttp,
    #[error("port nul")]
    Port,
}

/// La règle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    secure: bool,
    hosts: Vec<String>,
    port: u16,
    max_redirects: u8,
}

impl Route {
    /// Une règle : `https` (`secure`) ou non, ses hôtes, son port, et combien de redirections au
    /// plus. Refusée si elle permettrait plus de [`MAX_REDIRECTS`] redirections, ou le clair vers
    /// autre chose qu'une adresse de boucle locale écrite en chiffres (`127.0.0.1`).
    pub fn new(secure: bool, hosts: &[&str], port: u16, max_redirects: u8) -> Result<Self, RuleError> {
        if max_redirects > MAX_REDIRECTS {
            return Err(RuleError::TooManyRedirects);
        }
        if hosts.is_empty() || !hosts.iter().all(|host| is_host(host)) {
            return Err(RuleError::Hosts);
        }
        let loopback = |host: &&str| host.parse::<Ipv4Addr>().is_ok_and(|ip| ip.is_loopback());
        if !secure && !hosts.iter().all(loopback) {
            return Err(RuleError::PlainHttp);
        }
        if port == 0 {
            return Err(RuleError::Port);
        }
        let hosts = hosts.iter().map(|host| (*host).to_owned()).collect();
        Ok(Self { secure, hosts, port, max_redirects })
    }

    /// La vraie : GitHub seulement ([`GITHUB_HOSTS`]), en `https`, au port 443, trois redirections
    /// au plus.
    pub fn github() -> Self {
        Self::new(true, &GITHUB_HOSTS, 443, MAX_REDIRECTS).expect("la règle de GitHub est bien écrite")
    }

    /// Celle des essais et des répétitions : `http://127.0.0.1:<port>` seulement, trois
    /// redirections au plus. Rien ne sort de la machine.
    pub fn this_machine(port: u16) -> Result<Self, RuleError> {
        Self::new(false, &["127.0.0.1"], port, MAX_REDIRECTS)
    }

    /// `https` exigé ?
    pub fn is_secure(&self) -> bool {
        self.secure
    }

    pub fn max_redirects(&self) -> u8 {
        self.max_redirects
    }

    /// L'adresse de départ est-elle permise ?
    pub fn check(&self, url: &str) -> Result<Address, RouteError> {
        if url.len() > MAX_URL_CHARS {
            return Err(RouteError::TooLong);
        }
        let (scheme, rest) = url.split_once("://").ok_or(RouteError::Malformed)?;
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            _ => return Err(RouteError::Scheme),
        };
        if secure != self.secure {
            return Err(if self.secure { RouteError::Scheme } else { RouteError::Malformed });
        }
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, target) = rest.split_at(end);
        if authority.contains('@') {
            return Err(RouteError::Credentials);
        }
        // une adresse IPv6, entre crochets : aucune règle n'en permet
        if authority.contains(['[', ']']) {
            return Err(RouteError::Malformed);
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => {
                let port = (!port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| port.parse::<u16>().ok())
                    .flatten()
                    .ok_or(RouteError::Port)?;
                (host, port)
            }
            None => (authority, if secure { 443 } else { 80 }),
        };
        let host = host.to_ascii_lowercase();
        if !is_host(&host) {
            return Err(RouteError::Malformed);
        }
        if !self.hosts.contains(&host) {
            return Err(RouteError::Host(host));
        }
        if port != self.port {
            return Err(RouteError::Port);
        }
        Ok(Address { secure, host, port, target: target_of(target)? })
    }

    /// La redirection `location` (l'en-tête `Location`), reçue de `from` : est-elle permise ? Une
    /// adresse relative au même hôte (`/chemin…`) l'est aux mêmes conditions qu'une adresse entière.
    /// Le nombre de redirections est compté par celui qui les suit.
    pub fn follow(&self, from: &Address, location: &str) -> Result<Address, RouteError> {
        if location.len() > MAX_URL_CHARS {
            return Err(RouteError::TooLong);
        }
        if location.starts_with('/') && !location.starts_with("//") {
            let target = target_of(location)?;
            return Ok(Address { target, ..from.clone() });
        }
        if !location.contains("://") {
            return Err(RouteError::Malformed);
        }
        self.check(location)
    }
}

/// Le chemin et la requête d'une adresse, sans son fragment (`#…`, qui ne part jamais) : des
/// caractères permis par la RFC 3986 seulement.
fn target_of(target: &str) -> Result<String, RouteError> {
    let target = target.split('#').next().unwrap_or_default();
    let permitted = |b: u8| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'%'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
                    | b'/'
                    | b'?'
            )
    };
    if !target.bytes().all(permitted) || !(target.is_empty() || target.starts_with(['/', '?'])) {
        return Err(RouteError::Malformed);
    }
    Ok(match target {
        "" => "/".to_owned(),
        query if query.starts_with('?') => format!("/{query}"),
        path => path.to_owned(),
    })
}

/// Un nom d'hôte (lettres, chiffres, `-` et `.`, en minuscules) ou une adresse IPv4 écrite en
/// chiffres. Ni IPv6, ni point final, ni morceau vide.
fn is_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Une adresse permise par la règle.
#[derive(Clone, PartialEq, Eq)]
pub struct Address {
    secure: bool,
    host: String,
    port: u16,
    /// Le chemin et la requête : peut porter une signature.
    target: String,
}

impl Address {
    /// L'adresse entière, pour s'y connecter et nulle part ailleurs : elle peut être signée.
    pub fn reveal_for_request(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        let default = if self.secure { 443 } else { 80 };
        let port = if self.port == default { String::new() } else { format!(":{}", self.port) };
        format!("{scheme}://{}{port}{}", self.host, self.target)
    }

    pub fn is_secure(&self) -> bool {
        self.secure
    }

    /// L'hôte : la seule partie de l'adresse qui peut s'écrire.
    pub fn host(&self) -> &str {
        &self.host
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // jamais le chemin ni la requête : celle d'une adresse signée est un secret
        write!(f, "Address({}:{}/…)", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "https://github.com/Thai-v4/capri-daemon/releases/latest/download/latest.json";
    const SIGNED: &str = "https://release-assets.githubusercontent.com/github-production-release-asset/1/\
        abc?sp=r&sv=2018-11-09&sr=b&spr=https&se=2026-11-23T10%3A00%3A00Z&rscd=attachment%3B+filename%3D\
        latest.json&rsct=application%2Foctet-stream&sig=SeCrEtSiGnAtUrE%3D&jwt=eyJ0eXAi.eyJp.c2ln";

    #[test]
    fn github_s_two_redirections_are_followed() {
        let route = Route::github();
        let start = route.check(START).expect("le départ");
        assert_eq!(start.reveal_for_request(), START);
        let release = route
            .follow(&start, "https://github.com/Thai-v4/capri-daemon/releases/download/v0.2.0/latest.json")
            .expect("la version");
        let signed = route.follow(&release, SIGNED).expect("le fichier signé");
        assert_eq!(signed.reveal_for_request(), SIGNED);
        assert_eq!(signed.host(), "release-assets.githubusercontent.com");
        // une redirection relative reste chez le même hôte
        let relative = route.follow(&start, "/Thai-v4/capri-daemon/releases/download/v0.2.0/latest.json");
        assert_eq!(relative.expect("relative").host(), "github.com");
        // le port par défaut, écrit ou non ; la casse de l'hôte et du protocole n'y fait rien
        assert!(route.check("https://github.com:443/x").is_ok());
        assert!(route.check("HTTPS://GitHub.com/x").is_ok());
        // le fragment ne part jamais
        let fragment = route.check("https://github.com/x#y").expect("permise");
        assert_eq!(fragment.reveal_for_request(), "https://github.com/x");
        assert_eq!(
            route.check("https://github.com").expect("permise").reveal_for_request(),
            "https://github.com/"
        );
        assert_eq!(route.max_redirects(), 3);
        assert!(route.is_secure());
    }

    #[test]
    fn anything_but_https_to_a_listed_host_on_its_port_is_refused() {
        let route = Route::github();
        let refusals = [
            ("http://github.com/x", RouteError::Scheme),
            ("ftp://github.com/x", RouteError::Scheme),
            ("file:///etc/passwd", RouteError::Scheme),
            ("https://evil.example/x", RouteError::Host("evil.example".to_owned())),
            ("https://github.com.evil.example/x", RouteError::Host("github.com.evil.example".to_owned())),
            ("https://api.github.com/x", RouteError::Host("api.github.com".to_owned())),
            (
                "https://objects.githubusercontent.com/x",
                RouteError::Host("objects.githubusercontent.com".to_owned()),
            ),
            ("https://github.com./x", RouteError::Malformed),
            ("https://user:pass@github.com/x", RouteError::Credentials),
            ("https://github.com@evil.example/x", RouteError::Credentials),
            ("https://github.com:8443/x", RouteError::Port),
            ("https://github.com:/x", RouteError::Port),
            ("https://github.com:99999/x", RouteError::Port),
            ("https://[::1]/x", RouteError::Malformed),
            ("https://140.82.121.4/x", RouteError::Host("140.82.121.4".to_owned())),
            ("https://github.com/a b", RouteError::Malformed),
            ("https://github.com/a\nb", RouteError::Malformed),
            ("https://github.com/a\\b", RouteError::Malformed),
            ("https://github.com/é", RouteError::Malformed),
            ("https://git_hub.com/x", RouteError::Malformed),
            ("github.com/x", RouteError::Malformed),
            ("", RouteError::Malformed),
        ];
        for (url, refusal) in refusals {
            assert_eq!(route.check(url), Err(refusal), "{url:?}");
        }
        let long = format!("https://github.com/{}", "a".repeat(MAX_URL_CHARS));
        assert_eq!(route.check(&long), Err(RouteError::TooLong));
        // et dans une redirection, les mêmes refus, plus les formes relatives autres que /chemin
        let start = route.check(START).expect("le départ");
        for location in ["//evil.example/x", "evil.example/x", "x", "../x", "?q", "http://github.com/x"] {
            assert!(route.follow(&start, location).is_err(), "{location:?}");
        }
        assert_eq!(route.follow(&start, "/a b"), Err(RouteError::Malformed));
    }

    #[test]
    fn a_rule_for_this_machine_never_lets_anything_out_in_clear() {
        let route = Route::this_machine(8123).expect("cette machine");
        assert!(!route.is_secure());
        assert!(route.check("http://127.0.0.1:8123/latest.json").is_ok());
        assert_eq!(route.check("http://127.0.0.1:8124/x"), Err(RouteError::Port));
        assert_eq!(route.check("http://127.0.0.1/x"), Err(RouteError::Port), "le port 80 n'est pas le sien");
        assert_eq!(route.check("http://localhost:8123/x"), Err(RouteError::Host("localhost".to_owned())));
        assert_eq!(route.check("https://127.0.0.1:8123/x"), Err(RouteError::Malformed));
        assert_eq!(route.check("http://github.com:8123/x"), Err(RouteError::Host("github.com".to_owned())));
        // une règle en clair vers autre chose que la boucle locale ne s'écrit pas
        assert_eq!(Route::new(false, &["github.com"], 80, 3), Err(RuleError::PlainHttp));
        assert_eq!(Route::new(false, &["192.168.1.2"], 80, 3), Err(RuleError::PlainHttp));
        assert_eq!(Route::new(false, &["127.0.0.1", "10.0.0.1"], 80, 3), Err(RuleError::PlainHttp));
        assert_eq!(Route::new(true, &["github.com"], 443, 4), Err(RuleError::TooManyRedirects));
        assert_eq!(Route::new(true, &[], 443, 3), Err(RuleError::Hosts));
        assert_eq!(Route::new(true, &["GitHub.com"], 443, 3), Err(RuleError::Hosts));
        assert_eq!(Route::new(true, &["github.com"], 0, 3), Err(RuleError::Port));
        assert_eq!(Route::this_machine(0), Err(RuleError::Port));
    }

    #[test]
    fn a_signed_address_never_shows_more_than_its_host() {
        let route = Route::github();
        let signed = route.check(SIGNED).expect("permise");
        let shown = format!("{signed:?}");
        assert_eq!(shown, "Address(release-assets.githubusercontent.com:443/…)");
        assert!(!shown.contains("SeCrEt") && !shown.contains("sig="));
        // un refus non plus : seulement l'hôte, jamais la requête
        let refused = route.check(&SIGNED.replace("release-assets.githubusercontent.com", "evil.example"));
        let said = refused.expect_err("refusée").to_string();
        assert_eq!(said, "hôte hors de la liste (evil.example)");
    }
}
