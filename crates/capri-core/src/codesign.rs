//! La signature de `Capri.app`, pour la refaire après une mise à jour **comme celle de l'app
//! installée** : ce que disent `codesign` et `security`, lu sans rien garder de plus que
//! nécessaire, le choix de l'identité, et les arguments qu'on leur passe. Les appels eux-mêmes
//! sont dans `capri-platform`.
//!
//! - l'app installée est signée « ad hoc » : la nouvelle l'est aussi ;
//! - elle est signée par une identité : on cherche, parmi les identités de signature **valides**
//!   du trousseau (`security find-identity -v -p codesigning`, dont seuls les noms et les
//!   empreintes sont lus), celle qui porte le même nom. Une seule : on signe avec elle (par son
//!   empreinte, sans ambiguïté). Aucune (expirée, retirée) : « ad hoc », et macOS redemandera le
//!   micro. Plusieurs : on ne choisit pas.
//!
//! **Le nom d'une identité est celui d'une personne** : [`IdentityName`] et [`IdentityHash`] n'ont
//! pas de `Display`, leur `Debug` les masque, et rien ici ne les écrit. Ils ne servent qu'à
//! comparer, et l'empreinte qu'à être passée à `codesign`.
//!
//! Signer se fait sur la machine, **sans rien envoyer** : toujours `--timestamp=none` (pas
//! d'horodatage par le serveur d'Apple), et jamais de notarisation.

use std::ffi::OsString;
use std::fmt;
use std::path::Path;

/// `codesign`, par son chemin absolu : jamais par le `PATH`.
pub const CODESIGN: &str = "/usr/bin/codesign";
/// `security`, par son chemin absolu.
pub const SECURITY: &str = "/usr/bin/security";
/// Les identités de signature valides du trousseau : leurs empreintes et leurs noms, rien d'autre.
pub const FIND_IDENTITY: [&str; 4] = ["find-identity", "-v", "-p", "codesigning"];

/// Le nom d'une identité de signature (« Developer ID Application: … ») : celui d'une personne.
#[derive(Clone, PartialEq, Eq)]
pub struct IdentityName(String);

impl fmt::Debug for IdentityName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityName(…)")
    }
}

/// L'empreinte SHA-1 du certificat d'une identité, telle que `security` l'écrit : 40 chiffres
/// hexadécimaux majuscules.
#[derive(Clone, PartialEq, Eq)]
pub struct IdentityHash(String);

impl IdentityHash {
    /// Pour `codesign --sign`, et nulle part ailleurs.
    pub fn for_codesign(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for IdentityHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityHash(…)")
    }
}

/// Comment une app est signée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signature {
    AdHoc,
    /// Par une identité : le nom de son certificat (le premier `Authority=`).
    Identity(IdentityName),
}

/// Pourquoi la signature d'une app n'est pas connue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureError {
    /// `codesign` dit qu'elle n'est pas signée.
    Unsigned,
    /// Sa sortie ne dit ni « ad hoc » ni une identité.
    Unreadable,
}

/// La signature d'une app, d'après ce que `codesign -d --verbose=2` écrit sur sa sortie d'erreur
/// ([`describe_args`]). Seules deux sortes de lignes sont lues : `Signature=adhoc`, et la
/// première `Authority=…` (le certificat qui a signé ; les suivantes sont ceux qui l'ont émis).
pub fn signature(text: &str) -> Result<Signature, SignatureError> {
    let mut authority = None;
    for line in text.lines() {
        if line.trim_end() == "Signature=adhoc" {
            return Ok(Signature::AdHoc);
        }
        if authority.is_none()
            && let Some(name) = line.strip_prefix("Authority=")
        {
            authority = Some(name.trim_end());
        }
    }
    match authority {
        Some(name) if !name.is_empty() => Ok(Signature::Identity(IdentityName(name.to_owned()))),
        _ if text.contains("is not signed at all") => Err(SignatureError::Unsigned),
        _ => Err(SignatureError::Unreadable),
    }
}

/// Les identités de signature valides, d'après ce que `security find-identity -v -p codesigning`
/// écrit sur sa sortie standard : une ligne `  1) <empreinte> "<nom>"` par identité. Toute autre
/// ligne est ignorée (« 1 valid identities found »…).
pub fn identities(text: &str) -> Vec<(IdentityHash, IdentityName)> {
    text.lines()
        .filter_map(|line| {
            let (number, rest) = line.trim_start().split_once(") ")?;
            if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let (hash, name) = rest.split_once(' ')?;
            let hash_ok = hash.len() == 40 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'F'));
            let name = name.strip_prefix('"')?.strip_suffix('"')?;
            (hash_ok && !name.is_empty())
                .then(|| (IdentityHash(hash.to_owned()), IdentityName(name.to_owned())))
        })
        .collect()
}

/// Avec quoi signer la nouvelle app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    /// L'app installée est signée « ad hoc » : la nouvelle aussi.
    AdHoc,
    /// La seule identité valide du même nom.
    Identity(IdentityHash),
    /// Aucune identité valide de ce nom (expirée, retirée) : « ad hoc », et macOS redemandera le
    /// micro.
    Missing,
    /// Plusieurs : on ne choisit pas.
    Several(usize),
}

/// Le choix, pour une app installée signée `installed`. `identities` n'est appelée que si elle est
/// signée par une identité : le trousseau n'est pas lu pour une app « ad hoc ».
pub fn choose<E>(
    installed: &Signature,
    identities: impl FnOnce() -> Result<Vec<(IdentityHash, IdentityName)>, E>,
) -> Result<Choice, E> {
    let Signature::Identity(name) = installed else { return Ok(Choice::AdHoc) };
    let mut same: Vec<IdentityHash> =
        identities()?.into_iter().filter(|(_, found)| found == name).map(|(hash, _)| hash).collect();
    Ok(match same.len() {
        0 => Choice::Missing,
        1 => Choice::Identity(same.remove(0)),
        many => Choice::Several(many),
    })
}

/// Ce que reçoit `codesign --sign`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signer {
    AdHoc,
    Identity(IdentityHash),
}

impl Choice {
    /// Avec quoi signer ; `None` quand on ne choisit pas.
    pub fn signer(&self) -> Option<Signer> {
        match self {
            Self::AdHoc | Self::Missing => Some(Signer::AdHoc),
            Self::Identity(hash) => Some(Signer::Identity(hash.clone())),
            Self::Several(_) => None,
        }
    }
}

/// Les arguments de `codesign` pour signer `path` : toujours `--timestamp=none`, et `--force`
/// (les programmes publiés portent déjà la signature « ad hoc » que l'éditeur de liens leur donne).
pub fn sign_args(signer: &Signer, path: &Path) -> Vec<OsString> {
    let identity = match signer {
        Signer::AdHoc => "-",
        Signer::Identity(hash) => hash.for_codesign(),
    };
    let mut args: Vec<OsString> =
        ["--force", "--timestamp=none", "--sign", identity].map(OsString::from).to_vec();
    args.push(path.into());
    args
}

/// … pour vérifier la signature de `path`, strictement.
pub fn verify_args(path: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["--verify", "--strict"].map(OsString::from).to_vec();
    args.push(path.into());
    args
}

/// … pour dire comment `path` est signée (sur la sortie d'erreur ; `--verbose=2` pour les lignes
/// `Authority=`).
pub fn describe_args(path: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-d", "--verbose=2"].map(OsString::from).to_vec();
    args.push(path.into());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une sortie de `codesign -d --verbose=2` pour une app signée par une identité (inventée).
    const SIGNED: &str = "Executable=/Applications/Capri.app/Contents/MacOS/capri
Identifier=local.capri-daemon.capri
Format=app bundle with Mach-O thin (arm64)
CodeDirectory v=20500 size=1234 flags=0x0(none) hashes=30+3 location=embedded
Signature size=9000
Authority=Developer ID Application: Personne Inventée (ABCDE12345)
Authority=Developer ID Certification Authority
Authority=Apple Root CA
Signed Time=23 nov. 2026 at 10:00:00
Info.plist entries=13
TeamIdentifier=ABCDE12345
Sealed Resources version=2 rules=13 files=12
Internal requirements count=1 size=200
";

    /// … « ad hoc » (relevée sur ce Mac, le 23 novembre 2026, pour une app d'essai).
    const AD_HOC: &str = "Executable=/tmp/x/Capri.app/Contents/MacOS/capri
Identifier=local.capri-daemon.capri
Format=app bundle with Mach-O universal (x86_64 arm64e arm64e.x1)
CodeDirectory v=20400 size=273 flags=0x2(adhoc) hashes=2+3 location=embedded
Signature=adhoc
Info.plist entries=13
TeamIdentifier=not set
Sealed Resources version=2 rules=13 files=3
Internal requirements count=0 size=12
";

    const FOUND: &str = "  1) 0123456789ABCDEF0123456789ABCDEF01234567 \"Developer ID Application: Personne Inventée (ABCDE12345)\"
  2) 89ABCDEF0123456789ABCDEF0123456789ABCDEF \"Apple Development: Personne Inventée (ZZZZZ99999)\"
     2 valid identities found
";

    fn name(text: &str) -> IdentityName {
        IdentityName(text.to_owned())
    }

    #[test]
    fn how_an_app_is_signed_is_read_from_two_kinds_of_lines() {
        assert_eq!(signature(AD_HOC), Ok(Signature::AdHoc));
        assert_eq!(
            signature(SIGNED),
            Ok(Signature::Identity(name("Developer ID Application: Personne Inventée (ABCDE12345)")))
        );
        assert_eq!(
            signature("/tmp/x/Capri.app: code object is not signed at all\n"),
            Err(SignatureError::Unsigned)
        );
        assert_eq!(signature(""), Err(SignatureError::Unreadable));
        assert_eq!(signature("Authority=\n"), Err(SignatureError::Unreadable));
    }

    #[test]
    fn only_valid_identities_lines_are_read() {
        let found = identities(FOUND);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0.for_codesign(), "0123456789ABCDEF0123456789ABCDEF01234567");
        assert_eq!(found[1].1, name("Apple Development: Personne Inventée (ZZZZZ99999)"));
        for refused in [
            "  1) 0123456789abcdef0123456789abcdef01234567 \"minuscules\"",
            "  1) 0123 \"trop court\"",
            "  x) 0123456789ABCDEF0123456789ABCDEF01234567 \"pas un numéro\"",
            "  1) 0123456789ABCDEF0123456789ABCDEF01234567 sans guillemets",
            "  1) 0123456789ABCDEF0123456789ABCDEF01234567 \"\"",
            "     0 valid identities found",
        ] {
            assert!(identities(refused).is_empty(), "{refused}");
        }
    }

    #[test]
    fn the_identity_is_the_one_of_the_installed_app_or_none() {
        let never =
            || -> Result<Vec<(IdentityHash, IdentityName)>, ()> { panic!("le trousseau n'est pas lu") };
        assert_eq!(choose(&Signature::AdHoc, never), Ok(Choice::AdHoc), "ad hoc : le trousseau n'est pas lu");
        let installed = signature(SIGNED).expect("signée");
        let listed = || Ok::<_, ()>(identities(FOUND));
        let Ok(Choice::Identity(hash)) = choose(&installed, listed) else { panic!("une seule") };
        assert_eq!(hash.for_codesign(), "0123456789ABCDEF0123456789ABCDEF01234567");
        // aucune du même nom : ad hoc ; plusieurs : on ne choisit pas
        let other = Signature::Identity(name("Developer ID Application: Quelqu'un d'autre (QQQQQ11111)"));
        assert_eq!(choose(&other, listed), Ok(Choice::Missing));
        let twice = format!("{FOUND}{}", FOUND.lines().next().expect("une ligne").replace("0123", "4567"));
        assert_eq!(choose(&installed, || Ok::<_, ()>(identities(&twice))), Ok(Choice::Several(2)));
        assert_eq!(choose(&installed, || Err::<Vec<_>, _>("illisible")), Err("illisible"));
        assert_eq!(Choice::Missing.signer(), Some(Signer::AdHoc));
        assert_eq!(Choice::Several(2).signer(), None);
    }

    #[test]
    fn a_name_and_a_hash_never_show() {
        let installed = signature(SIGNED).expect("signée");
        let shown = format!("{installed:?} {:?}", identities(FOUND));
        assert!(
            !shown.contains("Personne") && !shown.contains("ABCDE") && !shown.contains("0123"),
            "{shown}"
        );
    }

    #[test]
    fn signing_never_leaves_the_machine() {
        let path = Path::new("/tmp/x/next");
        let hash = identities(FOUND).remove(0).0;
        for signer in [Signer::AdHoc, Signer::Identity(hash)] {
            let args = sign_args(&signer, path);
            assert_eq!(args[..3], ["--force", "--timestamp=none", "--sign"].map(OsString::from));
            assert_eq!(args.last(), Some(&OsString::from("/tmp/x/next")));
        }
        assert_eq!(sign_args(&Signer::AdHoc, path)[3], "-");
        let every = [sign_args(&Signer::AdHoc, path), verify_args(path), describe_args(path)].concat();
        let every: Vec<String> = every.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        for tool in ["notarytool", "altool", "stapler", "--timestamp=", "notar"] {
            let found: Vec<&String> =
                every.iter().filter(|arg| arg.contains(tool) && *arg != "--timestamp=none").collect();
            assert!(found.is_empty(), "{tool} : {found:?}");
        }
        assert!(CODESIGN.starts_with('/') && SECURITY.starts_with('/'));
        assert_eq!(FIND_IDENTITY, ["find-identity", "-v", "-p", "codesigning"]);
    }
}
