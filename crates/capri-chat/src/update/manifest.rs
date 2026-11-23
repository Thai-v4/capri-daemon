//! Le manifeste d'une version publiée, `latest.json` : sa lecture stricte.
//!
//! Le format (figé, écrit avec ce qui publie les versions) :
//!
//! ```json
//! {
//!   "format": 1,
//!   "version": "0.2.0",
//!   "bundle": 1,
//!   "targets": {
//!     "macos-arm64": {
//!       "files": [
//!         { "name": "capri", "asset": "capri-macos-arm64", "size": 12345678, "sha256": "…" }
//!       ]
//!     }
//!   }
//! }
//! ```
//!
//! - `format` : celui du manifeste. Un format que ce lecteur ne connaît pas n'est lu que pour sa
//!   `version` : on sait qu'elle existe, pas comment l'installer ([`Manifest::Unknown`]) ;
//! - `version` : `MAJEUR.MINEUR.CORRECTIF` ([`Version`]) ;
//! - `bundle` : le format de ce que `Capri.app` contient en plus de ses programmes (ses images) ;
//! - `targets` : par système, les fichiers de la version : leur nom dans l'app (`name`), leur nom
//!   parmi les fichiers de la version publiée (`asset`), leur taille et leur empreinte SHA-256.
//!
//! Le lecteur **ignore les champs inconnus**, à tous les niveaux (le manifeste n'évoluera que par
//! des champs facultatifs), et **refuse tout le reste**, en entier, plutôt que d'en deviner une
//! partie : trop gros, pas de l'UTF-8, pas du JSON, un champ attendu absent ou du mauvais type, une
//! version mal écrite, un nom hors de `[A-Za-z0-9._-]` (ou fait de points seulement : `.` et `..`
//! désigneraient un dossier), une empreinte qui n'est pas de 64 chiffres hexadécimaux minuscules,
//! une taille nulle ou déraisonnable, un fichier en double dans une cible.
//!
//! Rien ici ne télécharge ni ne vérifie un fichier : l'adresse d'un fichier se calcule
//! ([`super::asset_url`]), elle ne se lit jamais dans le manifeste.

use serde_json::{Map, Value};

use super::Version;

/// Le format de manifeste que ce lecteur connaît.
pub const FORMAT: u64 = 1;
/// La taille d'un manifeste, au plus (celui qui les publie en écrit de moins de 64 Kio).
pub const MAX_BYTES: usize = 64 * 1024;
/// La taille d'un fichier d'une version, au plus : un programme de Capri pèse une dizaine de Mo.
/// Au-delà, ce n'est pas un des siens.
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// La longueur d'un nom, au plus.
const MAX_NAME_CHARS: usize = 128;
/// Le nombre de cibles d'un manifeste, et de fichiers d'une cible, au plus.
const MAX_TARGETS: usize = 32;
const MAX_FILES: usize = 32;

/// Pourquoi un manifeste est refusé. Ne cite jamais ce qu'il contient : seulement le champ en
/// cause, par son nom dans le format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("plus de {MAX_BYTES} octets")]
    TooLarge,
    #[error("ce n'est pas de l'UTF-8")]
    NotUtf8,
    #[error("ce n'est pas du JSON")]
    NotJson,
    #[error("ce n'est pas un objet JSON")]
    NotAnObject,
    #[error("« {0} » manque")]
    Missing(&'static str),
    #[error("« {0} » est mal formé")]
    Invalid(&'static str),
    #[error("« {0} » en double")]
    Duplicate(&'static str),
}

/// Un manifeste lu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Manifest {
    /// Un format que ce lecteur connaît, lu en entier.
    Known(Release),
    /// Un format plus récent : seule la version est lue, le reste ne l'est pas.
    Unknown { format: u64, version: Version },
}

impl Manifest {
    /// Lit un manifeste (voir le haut du module : ce qui est ignoré, ce qui est refusé).
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        if bytes.len() > MAX_BYTES {
            return Err(ManifestError::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| ManifestError::NotUtf8)?;
        let value: Value = serde_json::from_str(text).map_err(|_| ManifestError::NotJson)?;
        let object = value.as_object().ok_or(ManifestError::NotAnObject)?;
        let format = field(object, "format")?.as_u64().filter(|&format| format >= 1);
        let format = format.ok_or(ManifestError::Invalid("format"))?;
        let version = field(object, "version")?.as_str().and_then(Version::parse);
        let version = version.ok_or(ManifestError::Invalid("version"))?;
        if format != FORMAT {
            return Ok(Self::Unknown { format, version });
        }
        let bundle = field(object, "bundle")?.as_u64().and_then(|bundle| u32::try_from(bundle).ok());
        let bundle = bundle.filter(|&bundle| bundle >= 1).ok_or(ManifestError::Invalid("bundle"))?;
        let listed = field(object, "targets")?.as_object().ok_or(ManifestError::Invalid("targets"))?;
        if listed.len() > MAX_TARGETS {
            return Err(ManifestError::Invalid("targets"));
        }
        let targets =
            listed.iter().map(|(name, target)| Target::parse(name, target)).collect::<Result<_, _>>()?;
        Ok(Self::Known(Release { version, bundle, targets }))
    }

    /// La version publiée, quel que soit le format.
    pub fn version(&self) -> Version {
        match self {
            Self::Known(release) => release.version,
            Self::Unknown { version, .. } => *version,
        }
    }
}

/// Une version publiée, dans le format que ce lecteur connaît.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// Le format de ce que `Capri.app` contient en plus de ses programmes.
    pub bundle: u32,
    pub targets: Vec<Target>,
}

impl Release {
    /// Les fichiers de la version pour ce système, s'il y en a.
    pub fn target(&self, name: &str) -> Option<&Target> {
        self.targets.iter().find(|target| target.name.as_str() == name)
    }
}

/// Les fichiers d'une version pour un système (`macos-arm64`, `windows-x86_64`…).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub name: Name,
    pub files: Vec<Entry>,
}

impl Target {
    fn parse(name: &str, value: &Value) -> Result<Self, ManifestError> {
        let name = Name::parse(name).ok_or(ManifestError::Invalid("targets"))?;
        let object = value.as_object().ok_or(ManifestError::Invalid("targets"))?;
        let listed = field(object, "files")?.as_array().ok_or(ManifestError::Invalid("files"))?;
        if listed.is_empty() || listed.len() > MAX_FILES {
            return Err(ManifestError::Invalid("files"));
        }
        let files: Vec<Entry> = listed.iter().map(Entry::parse).collect::<Result<_, _>>()?;
        for (index, file) in files.iter().enumerate() {
            if files[..index].iter().any(|before| before.name == file.name) {
                return Err(ManifestError::Duplicate("name"));
            }
        }
        Ok(Self { name, files })
    }
}

/// Un fichier d'une version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Son nom dans l'app installée (`capri`, `capri-hook`, `Info.plist`…).
    pub name: Name,
    /// Son nom parmi les fichiers de la version publiée : son adresse en est calculée.
    pub asset: Name,
    /// Sa taille, en octets.
    pub size: u64,
    pub sha256: Sha256,
}

impl Entry {
    fn parse(value: &Value) -> Result<Self, ManifestError> {
        let object = value.as_object().ok_or(ManifestError::Invalid("files"))?;
        let name = |key: &'static str| {
            field(object, key)?.as_str().and_then(Name::parse).ok_or(ManifestError::Invalid(key))
        };
        let size = field(object, "size")?.as_u64().filter(|size| (1..=MAX_FILE_BYTES).contains(size));
        let sha256 = field(object, "sha256")?.as_str().and_then(Sha256::parse);
        Ok(Self {
            name: name("name")?,
            asset: name("asset")?,
            size: size.ok_or(ManifestError::Invalid("size"))?,
            sha256: sha256.ok_or(ManifestError::Invalid("sha256"))?,
        })
    }
}

/// Un nom de fichier ou de cible : `[A-Za-z0-9._-]`, de 1 à 128 caractères, jamais fait de
/// points seulement (`.`, `..`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name(String);

impl Name {
    pub fn parse(text: &str) -> Option<Self> {
        let allowed = text.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        let dots_only = text.bytes().all(|b| b == b'.');
        (allowed && !dots_only && text.len() <= MAX_NAME_CHARS).then(|| Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Une empreinte SHA-256, telle qu'écrite dans le manifeste : 64 chiffres hexadécimaux minuscules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha256(String);

impl Sha256 {
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        hex.then(|| Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Un champ attendu.
fn field<'a>(object: &'a Map<String, Value>, key: &'static str) -> Result<&'a Value, ManifestError> {
    object.get(key).ok_or(ManifestError::Missing(key))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Le manifeste du contrat, avec de vraies empreintes.
    pub(crate) fn example(version: &str, bundle: u32) -> String {
        format!(
            r#"{{
  "format": 1,
  "version": "{version}",
  "bundle": {bundle},
  "targets": {{
    "macos-arm64": {{
      "files": [
        {{ "name": "capri", "asset": "capri-macos-arm64", "size": 12345678, "sha256": "{A}" }},
        {{ "name": "capri-hook", "asset": "capri-hook-macos-arm64", "size": 123456, "sha256": "{A}" }},
        {{ "name": "Info.plist", "asset": "Info.plist", "size": 1234, "sha256": "{A}" }}
      ]
    }},
    "windows-x86_64": {{ "files": [
      {{ "name": "capri.exe", "asset": "capri-windows-x86_64.exe", "size": 1, "sha256": "{A}" }},
      {{ "name": "capri-hook.exe", "asset": "capri-hook-windows-x86_64.exe", "size": 1, "sha256": "{A}" }}
    ] }}
  }}
}}"#
        )
    }

    fn parse(text: &str) -> Result<Manifest, ManifestError> {
        Manifest::parse(text.as_bytes())
    }

    /// Le manifeste du contrat, dont un champ est remplacé : `"cle": ancienne` devient `"cle": nouvelle`.
    fn with(old: &str, new: &str) -> String {
        let text = example("0.2.0", 1);
        assert!(text.contains(old), "{old}");
        text.replacen(old, new, 1)
    }

    #[test]
    fn the_contract_s_example_is_read_entirely() {
        let Ok(Manifest::Known(release)) = parse(&example("0.2.0", 1)) else { panic!("refusé") };
        assert_eq!(release.version, Version::new(0, 2, 0));
        assert_eq!(release.bundle, 1);
        let mac = release.target("macos-arm64").expect("la cible macOS");
        let names: Vec<&str> = mac.files.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(names, ["capri", "capri-hook", "Info.plist"]);
        assert_eq!(mac.files[0].asset.as_str(), "capri-macos-arm64");
        assert_eq!(mac.files[0].size, 12_345_678);
        assert_eq!(mac.files[0].sha256.as_str(), A);
        assert_eq!(release.target("windows-x86_64").expect("la cible Windows").files.len(), 2);
        assert_eq!(release.target("linux-x86_64"), None);
    }

    #[test]
    fn unknown_fields_are_ignored_at_every_level() {
        let text = example("0.3.1", 2)
            .replacen("\"format\": 1,", "\"format\": 1, \"notes\": \"…\", \"published\": 17,", 1)
            .replacen("\"files\": [", "\"signature\": null, \"files\": [", 1)
            .replacen("\"size\": 1234,", "\"size\": 1234, \"mode\": \"0644\",", 1);
        let Ok(Manifest::Known(release)) = parse(&text) else { panic!("refusé") };
        assert_eq!((release.version, release.bundle), (Version::new(0, 3, 1), 2));
        assert_eq!(release.target("macos-arm64").expect("là").files.len(), 3);
    }

    #[test]
    fn a_newer_format_is_read_only_for_its_version() {
        // le reste peut avoir changé du tout au tout : il n'est pas lu
        let text = r#"{"format": 2, "version": "1.0.0", "targets": [1, 2, 3]}"#;
        assert_eq!(parse(text), Ok(Manifest::Unknown { format: 2, version: Version::new(1, 0, 0) }));
        assert_eq!(parse(text).expect("lu").version(), Version::new(1, 0, 0));
        // sa version, elle, doit être lisible
        assert_eq!(parse(r#"{"format": 2, "version": "1.0"}"#), Err(ManifestError::Invalid("version")));
        assert_eq!(parse(r#"{"format": 2}"#), Err(ManifestError::Missing("version")));
    }

    #[test]
    fn anything_else_is_refused_whole() {
        use ManifestError::*;
        let cases: Vec<(String, ManifestError)> = vec![
            ("".to_owned(), NotJson),
            ("{".to_owned(), NotJson),
            ("[]".to_owned(), NotAnObject),
            ("\"0.2.0\"".to_owned(), NotAnObject),
            ("{}".to_owned(), Missing("format")),
            (format!("{}x", example("0.2.0", 1)), NotJson),
            (with("\"format\": 1", "\"format\": 0"), Invalid("format")),
            (with("\"format\": 1", "\"format\": \"1\""), Invalid("format")),
            (with("\"format\": 1", "\"format\": 1.0"), Invalid("format")),
            (with("\"format\": 1", "\"format\": -1"), Invalid("format")),
            (with("\"version\": \"0.2.0\"", "\"version\": \"v0.2.0\""), Invalid("version")),
            (with("\"version\": \"0.2.0\"", "\"version\": \"0.2.0-beta\""), Invalid("version")),
            (with("\"version\": \"0.2.0\"", "\"version\": 2"), Invalid("version")),
            (with("\"version\": \"0.2.0\",", ""), Missing("version")),
            (with("\"bundle\": 1,", ""), Missing("bundle")),
            (with("\"bundle\": 1", "\"bundle\": 0"), Invalid("bundle")),
            (with("\"bundle\": 1", "\"bundle\": 1.5"), Invalid("bundle")),
            (with("\"bundle\": 1", "\"bundle\": 4294967296"), Invalid("bundle")),
            (with("\"targets\": {", "\"targets\": [], \"x\": {"), Invalid("targets")),
            (with("\"macos-arm64\"", "\"macos/arm64\""), Invalid("targets")),
            (with("\"macos-arm64\": {", "\"macos-arm64\": [], \"x\": {"), Invalid("targets")),
            (with("\"files\": [", "\"fichiers\": ["), Missing("files")),
            (with("\"files\": [", "\"files\": {}, \"x\": ["), Invalid("files")),
            (with("\"name\": \"capri\"", "\"name\": \"../capri\""), Invalid("name")),
            (with("\"name\": \"capri\"", "\"name\": \"..\""), Invalid("name")),
            (with("\"name\": \"capri\"", "\"name\": \".\""), Invalid("name")),
            (with("\"name\": \"capri\"", "\"name\": \"\""), Invalid("name")),
            (with("\"name\": \"capri\"", "\"name\": \"ca pri\""), Invalid("name")),
            (with("\"name\": \"capri\"", "\"name\": \"capri\\\\x\""), Invalid("name")),
            (with("\"name\": \"capri\"", "\"name\": \"caprí\""), Invalid("name")),
            (with("\"name\": \"capri\"", &format!("\"name\": \"{}\"", "c".repeat(129))), Invalid("name")),
            (with("\"name\": \"capri\"", "\"name\": 7"), Invalid("name")),
            (with("\"asset\": \"capri-macos-arm64\"", "\"asset\": \"x?y=1\""), Invalid("asset")),
            (with("\"asset\": \"capri-macos-arm64\", ", ""), Missing("asset")),
            (with("\"size\": 12345678", "\"size\": 0"), Invalid("size")),
            (with("\"size\": 12345678", "\"size\": -1"), Invalid("size")),
            (with("\"size\": 12345678", "\"size\": 1e3"), Invalid("size")),
            (with("\"size\": 12345678", "\"size\": \"12\""), Invalid("size")),
            (with("\"size\": 12345678", &format!("\"size\": {}", MAX_FILE_BYTES + 1)), Invalid("size")),
            (
                with(&format!("\"sha256\": \"{A}\""), &format!("\"sha256\": \"{}\"", A.to_uppercase())),
                Invalid("sha256"),
            ),
            (
                with(&format!("\"sha256\": \"{A}\""), &format!("\"sha256\": \"{}\"", &A[1..])),
                Invalid("sha256"),
            ),
            (
                with(&format!("\"sha256\": \"{A}\""), &format!("\"sha256\": \"{}g\"", &A[1..])),
                Invalid("sha256"),
            ),
            (with("\"name\": \"capri-hook\"", "\"name\": \"capri\""), Duplicate("name")),
        ];
        for (text, error) in cases {
            assert_eq!(parse(&text), Err(error), "{text}");
        }
        // une cible sans fichier, ou avec trop de fichiers
        let empty =
            r#"{"format": 1, "version": "0.2.0", "bundle": 1, "targets": {"macos-arm64": {"files": []}}}"#;
        assert_eq!(parse(empty), Err(Invalid("files")));
        let file = format!(r#"{{"name": "f", "asset": "f", "size": 1, "sha256": "{A}"}}"#);
        let many: Vec<String> =
            (0..=MAX_FILES).map(|n| file.replace("\"f\"", &format!("\"f{n}\""))).collect();
        let many = format!(
            r#"{{"format": 1, "version": "0.2.0", "bundle": 1, "targets": {{"t": {{"files": [{}]}}}}}}"#,
            many.join(",")
        );
        assert_eq!(parse(&many), Err(Invalid("files")));
        // sans cible du tout : lisible (et aucun système n'y trouvera la sienne)
        let none = r#"{"format": 1, "version": "0.2.0", "bundle": 1, "targets": {}}"#;
        assert!(matches!(parse(none), Ok(Manifest::Known(release)) if release.targets.is_empty()));
    }

    #[test]
    fn what_is_not_utf8_or_too_big_is_refused_before_being_read() {
        assert_eq!(Manifest::parse(b"{\"format\": 1, \"version\": \"\xff\"}"), Err(ManifestError::NotUtf8));
        assert_eq!(Manifest::parse("\u{feff}{}".as_bytes()), Err(ManifestError::NotJson), "pas de BOM");
        // du JSON valide, mais trop gros : refusé sans être lu
        let padded = format!("{}{}", example("0.2.0", 1), " ".repeat(MAX_BYTES));
        assert_eq!(parse(&padded), Err(ManifestError::TooLarge));
        let text = example("0.2.0", 1);
        let exact = format!("{text}{}", " ".repeat(MAX_BYTES - text.len()));
        assert_eq!(exact.len(), MAX_BYTES);
        assert!(matches!(parse(&exact), Ok(Manifest::Known(_))), "64 Kio tout juste : lu");
    }

    #[test]
    fn a_refusal_names_a_field_and_never_quotes_the_manifest() {
        let said = ManifestError::Invalid("sha256").to_string();
        assert_eq!(said, "« sha256 » est mal formé");
        assert_eq!(ManifestError::TooLarge.to_string(), "plus de 65536 octets");
        assert_eq!(ManifestError::Duplicate("name").to_string(), "« name » en double");
    }
}
