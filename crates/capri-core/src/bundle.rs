//! Ce que Capri sait de son app, `Capri.app`, pour s'y installer elle-même : ce qu'une version y
//! remplace, la lecture de son `Info.plist`, les noms de ce qu'une mise à jour fabrique à côté
//! d'elle, et le relais entre l'ancienne Capri et le programme qui finit la mise à jour.
//!
//! **À côté de l'app installée**, une mise à jour ne fabrique qu'un dossier caché
//! ([`staging_name`] : `.Capri.app.capri-update-<processus>`, qui ne finit pas par `.app`), et le
//! programme de fin, le temps du remplacement, l'installation précédente mise de côté
//! ([`previous_name`]). Dans le dossier : une marque ([`MARKER`], au texte [`MARKER_TEXT`]), que
//! celui qui se sert du dossier tient verrouillée, et la nouvelle app ([`NEXT`], qui ne finit pas
//! par `.app` non plus : ni le Finder ni macOS ne la prennent pour une app). Rien d'autre n'est
//! écrit à côté de l'app.
//!
//! **Le relais** : l'ancienne Capri lance le `capri` de la nouvelle app (`capri update finish
//! <app installée>`), son entrée branchée sur un tube dont elle garde l'autre bout. Le programme de
//! fin dit qu'il a pris le relais par une ligne sur sa sortie ([`HANDOVER_READY`]) ; l'ancienne
//! écrit alors [`HANDOVER_GO`] dans le tube, et se quitte : le tube se ferme quand elle sort, et
//! le programme de fin ne commence qu'à ce moment-là, sans jamais signaler aucun processus. Ce
//! protocole est un contrat **entre deux versions** (l'ancienne lance le programme de la
//! nouvelle) : il ne change pas, ou par des ajouts que l'autre côté ignore.

pub use crate::login::BUNDLE_ID;

/// Le programme de l'app (`CFBundleExecutable`), dans `Contents/MacOS/`.
pub const PROGRAM: &str = "capri";
/// Le second programme de l'app, celui des hooks de Claude Code, à côté du premier.
pub const HOOK_PROGRAM: &str = "capri-hook";

/// Le début du nom du dossier caché d'une mise à jour, à côté de l'app installée ; le numéro du
/// processus qui l'a créé suit.
pub const STAGING_PREFIX: &str = ".Capri.app.capri-update-";
/// Ajouté au nom du dossier de la mise à jour : l'installation précédente, mise de côté le temps du
/// remplacement, à côté de l'app (un renommage dans le même dossier : il ne demande que le droit
/// d'écrire dans ce dossier).
pub const PREVIOUS_SUFFIX: &str = "-previous";
/// Dans le dossier de la mise à jour : la marque, et la nouvelle app.
pub const MARKER: &str = "capri-update";
pub const NEXT: &str = "next";
/// Le texte de la marque : rien d'autre que cette ligne.
pub const MARKER_TEXT: &str = "capri-managed : une mise à jour de Capri, faite par elle\n";

/// Le programme de fin a pris le relais (sa première ligne, sur sa sortie standard).
pub const HANDOVER_READY: &str = "capri-update-finish 1\n";
/// L'ancienne Capri lui dit d'y aller, juste avant de se quitter.
pub const HANDOVER_GO: &str = "go\n";

/// Le dossier d'une mise à jour faite par le processus `pid`.
pub fn staging_name(pid: u32) -> String {
    format!("{STAGING_PREFIX}{pid}")
}

/// L'installation précédente mise de côté pendant le remplacement, pour la mise à jour `pid`.
pub fn previous_name(pid: u32) -> String {
    format!("{STAGING_PREFIX}{pid}{PREVIOUS_SUFFIX}")
}

/// Ce que peut être un nom trouvé à côté de l'app installée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leftover {
    /// Le dossier d'une mise à jour.
    Staging(u32),
    /// Une installation mise de côté.
    Previous(u32),
}

/// Le nom `name` est-il un des nôtres ? Seulement ceux de [`staging_name`] et [`previous_name`],
/// au chiffre près : un numéro écrit en chiffres, sans zéro en tête.
pub fn leftover(name: &str) -> Option<Leftover> {
    let rest = name.strip_prefix(STAGING_PREFIX)?;
    let (digits, previous) = match rest.strip_suffix(PREVIOUS_SUFFIX) {
        Some(digits) => (digits, true),
        None => (rest, false),
    };
    let canonical = !digits.is_empty()
        && digits.len() <= 10
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'));
    let pid = canonical.then(|| digits.parse::<u32>().ok()).flatten()?;
    Some(if previous { Leftover::Previous(pid) } else { Leftover::Staging(pid) })
}

/// Pourquoi un `Info.plist` n'a pas pu être lu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlistError {
    /// Pas un `Info.plist` écrit en XML (`<plist>` puis `<dict>`), ou un XML mal formé.
    Malformed,
    /// Une clé deux fois dans le dictionnaire de premier niveau.
    Duplicate,
}

impl std::fmt::Display for PlistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Malformed => "pas un Info.plist lisible",
            Self::Duplicate => "une clé en double",
        })
    }
}

/// Les chaînes du dictionnaire de premier niveau d'un `Info.plist` écrit en XML, dans l'ordre :
/// `<key>…</key>` suivi de `<string>…</string>`. Les valeurs d'une autre sorte (`<true/>`, un
/// tableau, un dictionnaire imbriqué…) sont sautées ; les commentaires, la déclaration XML et le
/// `DOCTYPE` sont ignorés. Refusé : tout ce qui n'est pas un `<plist>` contenant un `<dict>`, une
/// balise mal fermée, une entité autre que les cinq de XML, une clé en double.
pub fn info_strings(text: &str) -> Result<Vec<(String, String)>, PlistError> {
    let tokens = tokens(text)?;
    let mut tokens = tokens.into_iter().filter(|token| !matches!(token, Token::Text(text) if is_blank(text)));
    if tokens.next() != Some(Token::Open("plist")) || tokens.next() != Some(Token::Open("dict")) {
        return Err(PlistError::Malformed);
    }
    let mut found: Vec<(String, String)> = Vec::new();
    loop {
        match tokens.next().ok_or(PlistError::Malformed)? {
            Token::Close("dict") => break,
            Token::Open("key") => {}
            _ => return Err(PlistError::Malformed),
        }
        let key = match tokens.next().ok_or(PlistError::Malformed)? {
            Token::Text(text) => {
                if tokens.next() != Some(Token::Close("key")) {
                    return Err(PlistError::Malformed);
                }
                unescape(text)?
            }
            Token::Close("key") => String::new(),
            _ => return Err(PlistError::Malformed),
        };
        let value = match tokens.next().ok_or(PlistError::Malformed)? {
            Token::Empty("string") => Some(String::new()),
            Token::Empty(_) => None,
            Token::Open("string") => match tokens.next().ok_or(PlistError::Malformed)? {
                Token::Close("string") => Some(String::new()),
                Token::Text(text) if tokens.next() == Some(Token::Close("string")) => Some(unescape(text)?),
                _ => return Err(PlistError::Malformed),
            },
            Token::Open(name) => {
                skip(&mut tokens, name)?;
                None
            }
            _ => return Err(PlistError::Malformed),
        };
        if found.iter().any(|(known, _)| *known == key) {
            return Err(PlistError::Duplicate);
        }
        if let Some(value) = value {
            found.push((key, value));
        }
    }
    if tokens.next() != Some(Token::Close("plist")) || tokens.next().is_some() {
        return Err(PlistError::Malformed);
    }
    Ok(found)
}

/// La chaîne de la clé `key` d'un `Info.plist` (voir [`info_strings`]), si elle y est.
pub fn info_string(text: &str, key: &str) -> Result<Option<String>, PlistError> {
    Ok(info_strings(text)?.into_iter().find(|(found, _)| found == key).map(|(_, value)| value))
}

/// Un morceau d'XML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token<'a> {
    /// `<nom …>`
    Open(&'a str),
    /// `</nom>`
    Close(&'a str),
    /// `<nom/>`
    Empty(&'a str),
    /// Du texte entre deux balises.
    Text(&'a str),
}

fn is_blank(text: &str) -> bool {
    text.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

/// Les morceaux d'un texte XML, sans la déclaration, les commentaires ni le `DOCTYPE`.
fn tokens(text: &str) -> Result<Vec<Token<'_>>, PlistError> {
    let mut found = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let Some(tag) = rest.strip_prefix('<') else {
            let end = rest.find('<').unwrap_or(rest.len());
            found.push(Token::Text(&rest[..end]));
            rest = &rest[end..];
            continue;
        };
        if let Some(after) = tag.strip_prefix("!--") {
            let end = after.find("-->").ok_or(PlistError::Malformed)?;
            rest = &after[end + 3..];
            continue;
        }
        let end = tag.find('>').ok_or(PlistError::Malformed)?;
        let inside = &tag[..end];
        rest = &tag[end + 1..];
        if inside.starts_with('?') {
            // la déclaration XML : `<?xml …?>`
            if !inside.ends_with('?') {
                return Err(PlistError::Malformed);
            }
            continue;
        }
        if let Some(doctype) = inside.strip_prefix('!') {
            // le DOCTYPE, sans sous-ensemble interne (qui pourrait déclarer des entités)
            if !doctype.starts_with("DOCTYPE") || doctype.contains('[') {
                return Err(PlistError::Malformed);
            }
            continue;
        }
        let token = if let Some(name) = inside.strip_prefix('/') {
            Token::Close(name.trim_end())
        } else if let Some(name) = inside.strip_suffix('/') {
            Token::Empty(element_name(name))
        } else {
            Token::Open(element_name(inside))
        };
        let name = match token {
            Token::Open(name) | Token::Close(name) | Token::Empty(name) => name,
            Token::Text(_) => unreachable!("une balise"),
        };
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        {
            return Err(PlistError::Malformed);
        }
        found.push(token);
    }
    Ok(found)
}

/// Le nom d'un élément, sans ses attributs (`plist version="1.0"` : `plist`).
fn element_name(inside: &str) -> &str {
    inside.split([' ', '\t', '\n', '\r']).next().unwrap_or_default()
}

/// Saute la valeur ouverte par `<name>`, jusqu'à sa balise fermante, imbrications comprises.
fn skip<'a>(tokens: &mut impl Iterator<Item = Token<'a>>, name: &str) -> Result<(), PlistError> {
    let mut open = vec![name];
    while let Some(innermost) = open.last().copied() {
        match tokens.next().ok_or(PlistError::Malformed)? {
            Token::Open(inner) => open.push(inner),
            Token::Close(closed) if closed == innermost => {
                open.pop();
            }
            Token::Close(_) => return Err(PlistError::Malformed),
            Token::Empty(_) | Token::Text(_) => {}
        }
    }
    Ok(())
}

/// Le texte d'un élément, ses entités remplacées : les cinq de XML, et seulement elles.
fn unescape(text: &str) -> Result<String, PlistError> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let (entity, after) = rest[at..].split_once(';').ok_or(PlistError::Malformed)?;
        out.push(match entity {
            "&amp" => '&',
            "&lt" => '<',
            "&gt" => '>',
            "&quot" => '"',
            "&apos" => '\'',
            _ => return Err(PlistError::Malformed),
        });
        rest = after;
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'`Info.plist` de l'app, tel que `tools/bundle-macos.sh` et la publication l'écrivent.
    fn ours(version: &str) -> String {
        include_str!("../../../tools/macos/Info.plist").replace("@VERSION@", version)
    }

    #[test]
    fn our_info_plist_is_read() {
        let text = ours("0.2.0");
        assert_eq!(info_string(&text, "CFBundleIdentifier"), Ok(Some(BUNDLE_ID.to_owned())));
        assert_eq!(info_string(&text, "CFBundleShortVersionString"), Ok(Some("0.2.0".to_owned())));
        assert_eq!(info_string(&text, "CFBundleVersion"), Ok(Some("0.2.0".to_owned())));
        assert_eq!(info_string(&text, "CFBundleExecutable"), Ok(Some(PROGRAM.to_owned())));
        // une valeur qui n'est pas une chaîne est sautée, pas lue
        assert_eq!(info_string(&text, "LSUIElement"), Ok(None));
        assert_eq!(info_string(&text, "Absente"), Ok(None));
        let strings = info_strings(&text).expect("lisible");
        assert!(
            strings
                .iter()
                .any(|(key, value)| key == "NSMicrophoneUsageDescription" && value.contains("Rien"))
        );
    }

    #[test]
    fn nested_values_entities_and_comments_are_understood() {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<!-- un commentaire <key>CFBundleIdentifier</key><string>faux</string> -->
<dict>
  <key>Imbriqué</key>
  <dict><key>CFBundleIdentifier</key><string>dedans</string><key>x</key><array><string>a</string></array></dict>
  <key>CFBundleIdentifier</key>
  <string>a&amp;b &lt;c&gt; &quot;d&quot; &apos;e&apos;</string>
  <key>Vide</key><string/>
  <key>Vide aussi</key><string></string>
  <key>Vrai</key><true/>
</dict>
</plist>
"#;
        assert_eq!(
            info_strings(text),
            Ok(vec![
                ("CFBundleIdentifier".to_owned(), "a&b <c> \"d\" 'e'".to_owned()),
                ("Vide".to_owned(), String::new()),
                ("Vide aussi".to_owned(), String::new()),
            ])
        );
    }

    #[test]
    fn anything_else_is_refused() {
        let good = ours("0.2.0");
        let cases = [
            String::new(),
            "pas du XML".to_owned(),
            "bplist00".to_owned(),
            good.replacen("<dict>", "<array>", 1),
            good.replacen("</plist>", "", 1),
            format!("{good}<plist/>"),
            good.replacen("</dict>", "", 1),
            good.replacen("<string>Capri</string>", "<string>Capri", 1),
            good.replacen("<string>Capri</string>", "<string>Capri</strin>", 1),
            good.replacen("<string>Capri</string>", "<string>&eacute;</string>", 1),
            good.replacen("<string>Capri</string>", "<string>&#67;</string>", 1),
            good.replacen("<string>Capri</string>", "<string>a & b</string>", 1),
            good.replacen("<!DOCTYPE plist", "<!DOCTYPE plist [<!ENTITY x \"y\">]", 1),
            good.replacen("<key>CFBundleName</key>", "texte perdu <key>CFBundleName</key>", 1),
            good.replacen("<key>CFBundleName</key>", "<key>CFBundleName</key><key>x</key>", 1),
            good.replacen("</dict>", "</dict><!-- sans fin", 1),
        ];
        for text in cases {
            assert_eq!(info_strings(&text), Err(PlistError::Malformed), "{text}");
        }
        let twice = good.replacen(
            "<key>CFBundleName</key>",
            "<key>CFBundleIdentifier</key><string>autre</string><key>CFBundleName</key>",
            1,
        );
        assert_eq!(info_string(&twice, "CFBundleIdentifier"), Err(PlistError::Duplicate));
    }

    #[test]
    fn only_our_own_names_are_recognized() {
        assert_eq!(staging_name(4321), ".Capri.app.capri-update-4321");
        assert_eq!(previous_name(4321), ".Capri.app.capri-update-4321-previous");
        assert_eq!(leftover(&staging_name(4321)), Some(Leftover::Staging(4321)));
        assert_eq!(leftover(&previous_name(4321)), Some(Leftover::Previous(4321)));
        assert_eq!(leftover(&staging_name(0)), Some(Leftover::Staging(0)));
        for name in [
            "Capri.app",
            ".Capri.app",
            ".Capri.app.capri-update-",
            ".Capri.app.capri-update--previous",
            ".Capri.app.capri-update-012",
            ".Capri.app.capri-update-12x",
            ".Capri.app.capri-update-12.app",
            ".Capri.app.capri-update-12-previous-previous",
            ".Capri.app.capri-update-99999999999",
            ".Capri.app.capri-install-12",
            ".Capri.app.capri-previous-12",
            ".capri.app.capri-update-12",
        ] {
            assert_eq!(leftover(name), None, "{name}");
        }
        // ni le dossier ni l'app qu'il contient ne finissent par .app
        for name in [staging_name(7), previous_name(7), NEXT.to_owned()] {
            assert!(!name.ends_with(".app"), "{name}");
        }
        assert!(MARKER_TEXT.starts_with("capri-managed") && MARKER_TEXT.ends_with('\n'));
        assert!(HANDOVER_READY.ends_with('\n') && HANDOVER_GO.ends_with('\n'));
    }
}
