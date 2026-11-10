//! Le miroir : la ligne que Claude écrit pour Capri, de sa réponse jusqu'à sa bulle.
//!
//! Avec le style « Capri » (`crate::style`), Claude finit chaque réponse par une ligne qui commence
//! par [`MARKER`]. Quand l'utilisateur a activé le miroir (il est **désactivé par défaut**),
//! `capri-hook`, sur l'événement `Stop` de la session principale (jamais `SubagentStop`), cherche
//! cette ligne dans `last_assistant_message` et ne transmet qu'elle, dans le message de fin de tour
//! (`crate::protocol::Said`). Capri la dit dans sa bulle, et à voix haute si sa voix est activée.
//!
//! C'est la seule exception à « le bandeau ne transmet jamais ce qu'écrit Claude », et elle est
//! étroite : activée par l'utilisateur seulement, elle ne transmet que cette ligne, bornée, par le
//! canal local réservé à l'utilisateur ; rien n'est gardé ni envoyé ailleurs. Ce qu'on ne peut pas
//! garantir : cette ligne est écrite par Claude, qui pourrait y citer un nom de fichier ou un bout
//! de résultat malgré les consignes du style. Miroir désactivé, le hook ne lit même pas la
//! réponse de Claude.
//!
//! Ce module est pur : le réglage et son format, l'extraction de la ligne, et l'attente d'une
//! ligne arrivée pendant que Capri parle déjà.

use serde::{Deserialize, Serialize};

use crate::protocol::Said;
pub use crate::style::MARKER;
use crate::text::one_line;

/// Le fichier du réglage, dans le dossier de Capri (`crate::places::capri_dir`).
pub const SETTINGS_FILE: &str = "claude-code.json";
/// Au-delà, le fichier du réglage n'est pas lu : le miroir compte pour désactivé.
pub const MAX_SETTINGS_BYTES: u64 = 4 * 1024;
/// Longueur maximale de la phrase transmise, en caractères (points de suspension compris). Le
/// style en demande moins au modèle (`style::ASKED_CHARS`) : il a de la marge.
pub const MAX_SAID_CHARS: usize = 240;
/// Et en octets : de quoi tenir, avec le reste du message, dans `protocol::MAX_LINE_BYTES`.
pub const MAX_SAID_BYTES: usize = 1024;
const _: () = assert!(crate::style::ASKED_CHARS < MAX_SAID_CHARS);
const _: () =
    assert!(4 * MAX_SAID_CHARS <= MAX_SAID_BYTES && MAX_SAID_BYTES < crate::protocol::MAX_LINE_BYTES / 4);
/// Une phrase qui attend que Capri ait fini de parler n'attend pas plus longtemps : elle parlerait
/// de quelque chose de périmé.
pub const WAIT_FOR_MS: u64 = 2 * 60 * 1000;

/// Le réglage du miroir.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorSettings {
    /// Capri dit-elle la ligne que Claude écrit pour elle ? Non, par défaut.
    #[serde(default, rename = "mirror")]
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("réglage du miroir illisible")]
pub struct InvalidMirrorSettings;

impl MirrorSettings {
    /// Relit le fichier : un objet JSON. Un champ inconnu est ignoré ; un champ du mauvais type,
    /// ou autre chose qu'un objet, est une erreur.
    pub fn from_json(text: &str) -> Result<Self, InvalidMirrorSettings> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|_| InvalidMirrorSettings)?;
        if !value.is_object() {
            return Err(InvalidMirrorSettings);
        }
        serde_json::from_value(value).map_err(|_| InvalidMirrorSettings)
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("un booléen se sérialise toujours");
        text.push('\n');
        text
    }
}

/// La ligne que Claude a écrite pour Capri dans `message` : la dernière ligne qui commence par
/// [`MARKER`], hors des blocs de code, sans son marqueur, sur une seule ligne, sans caractère de
/// contrôle, bornée à [`MAX_SAID_CHARS`] caractères et [`MAX_SAID_BYTES`] octets (coupée sur une
/// frontière de caractère). `None` s'il n'y en a pas, ou si elle ne dit rien.
///
/// Le Markdown qu'elle contiendrait encore (`**`, `` ` ``) est retiré par Capri avant de
/// l'afficher, comme pour ses propres réponses (`capri_chat::plain`).
pub fn marked_line(message: &str) -> Option<Said> {
    let mut fence: Option<char> = None;
    let mut found = None;
    for line in message.lines() {
        let line = line.trim_start();
        let opens = if line.starts_with("```") {
            Some('`')
        } else if line.starts_with("~~~") {
            Some('~')
        } else {
            None
        };
        if let Some(sign) = opens {
            match fence {
                None => fence = Some(sign),
                Some(open) if open == sign => fence = None,
                Some(_) => {}
            }
            continue;
        }
        if fence.is_none()
            && let Some(rest) = after_marker(line)
        {
            found = Some(rest);
        }
    }
    let said = bounded(one_line(found?, MAX_SAID_CHARS));
    (!said.is_empty()).then(|| Said::new(said))
}

/// Ce qui suit le marqueur, si `line` commence par lui. Tolère une mise en forme autour du
/// marqueur (`**[Capri]**`) et la casse ; exige qu'il soit suivi d'un blanc, de deux-points ou
/// d'un tiret, pour ne pas prendre un lien Markdown (`[Capri](…)`) pour une ligne de Capri.
fn after_marker(line: &str) -> Option<&str> {
    let line = line.trim_start_matches(['*', '_']);
    if !line.get(..MARKER.len())?.eq_ignore_ascii_case(MARKER) {
        return None;
    }
    let rest = line[MARKER.len()..].trim_start_matches(['*', '_']);
    let separator = |c: char| c.is_whitespace() || matches!(c, ':' | '—' | '–');
    rest.starts_with(separator).then(|| rest.trim_start_matches(separator))
}

/// Coupe à [`MAX_SAID_BYTES`] octets au plus, sur une frontière de caractère.
fn bounded(mut text: String) -> String {
    if text.len() > MAX_SAID_BYTES {
        let mut cut = MAX_SAID_BYTES;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    text
}

/// La phrase de Claude qui attend que Capri ait fini de parler : la plus récente seulement, et
/// pas plus de [`WAIT_FOR_MS`]. L'horloge est celle de l'appelant, en millisecondes.
#[derive(Debug, Default)]
pub struct Waiting {
    line: Option<(Said, u64)>,
}

impl Waiting {
    pub fn new() -> Self {
        Self::default()
    }

    /// Une phrase arrive : elle remplace celle qui attendait.
    pub fn offer(&mut self, said: Said, now_ms: u64) {
        self.line = Some((said, now_ms));
    }

    /// La phrase à dire maintenant, s'il y en a une et qu'elle n'est pas périmée. Elle ne sera
    /// rendue qu'une fois.
    pub fn take(&mut self, now_ms: u64) -> Option<Said> {
        let (said, since) = self.line.take()?;
        (now_ms.saturating_sub(since) < WAIT_FOR_MS).then_some(said)
    }

    /// Oublie la phrase qui attendait (on a fait taire Capri).
    pub fn clear(&mut self) {
        self.line = None;
    }
}

/// Ce que fait le miroir, dit à l'utilisateur avant qu'il le coche : la même phrase dans la
/// fenêtre de réglages et pour `capri hook`.
pub const ABOUT: &str = "\
Quand le miroir est activé, à la fin de chaque réponse de Claude Code (dans la session principale, \
pas dans un sous-agent), le hook cherche la dernière ligne qui commence par [Capri] et ne transmet \
qu'elle à Capri, par le canal local réservé à ton utilisateur. Capri la dit dans sa bulle, et à voix \
haute si sa voix est activée. Rien d'autre de la réponse n'est transmis, rien n'est gardé ni envoyé \
ailleurs. Cette ligne est écrite par Claude : il pourrait y citer un nom de fichier ou un bout de \
résultat. Sans le style Capri, Claude n'en écrit pas, et Capri n'a rien à dire.";

/// Le miroir vient d'être activé.
pub const TURNED_ON: &str =
    "Miroir activé : Capri dira la ligne que Claude écrit pour elle à la fin de chaque réponse.";
/// Le miroir vient d'être désactivé.
pub const TURNED_OFF: &str =
    "Miroir désactivé : le hook ne lit plus les réponses de Claude, et Capri n'en dit plus rien.";
/// Miroir activé, mais le hook n'est pas branché.
pub const NOT_PLUGGED: &str = "Claude Code n'est pas branché sur Capri : rien n'arrivera jusqu'à elle.";
/// Miroir activé, mais le style n'est pas installé.
pub const NO_STYLE: &str =
    "Le style Capri n'est pas installé : sans lui, Claude n'écrit pas de ligne pour elle.";

#[cfg(test)]
mod tests {
    use super::*;

    fn said(message: &str) -> Option<String> {
        marked_line(message).map(Said::into_text)
    }

    #[test]
    fn the_setting_is_off_unless_the_file_says_otherwise() {
        assert_eq!(MirrorSettings::default(), MirrorSettings { enabled: false });
        assert_eq!(MirrorSettings::from_json("{}"), Ok(MirrorSettings { enabled: false }));
        assert_eq!(MirrorSettings::from_json(r#"{"mirror":true}"#), Ok(MirrorSettings { enabled: true }));
        assert_eq!(
            MirrorSettings::from_json(r#"{"mirror":true,"futur":[1]}"#),
            Ok(MirrorSettings { enabled: true }),
            "un champ d'une version future n'empêche pas de lire"
        );
        for broken in ["", "{ cassé", r#"{"mirror":"oui"}"#, "[]", "[true]", "true"] {
            assert_eq!(MirrorSettings::from_json(broken), Err(InvalidMirrorSettings), "{broken}");
        }
        let on = MirrorSettings { enabled: true };
        assert_eq!(on.to_json(), "{\n  \"mirror\": true\n}\n");
        assert_eq!(MirrorSettings::from_json(&on.to_json()), Ok(on));
    }

    #[test]
    fn only_the_last_marked_line_is_kept() {
        let answer = "J'ai corrigé le test de `main.rs` et relancé la suite.\n\n\
                      Résultat : 312 tests passent.\n\n[Capri] C'est réparé, et tout passe. On respire !";
        assert_eq!(said(answer).as_deref(), Some("C'est réparé, et tout passe. On respire !"));
        let two = "[Capri] Première.\nDu travail.\n[Capri] Seconde, la bonne.\nUne ligne de plus.";
        assert_eq!(said(two).as_deref(), Some("Seconde, la bonne."), "la dernière marquée, où qu'elle soit");
        assert_eq!(said("Rien pour elle ici.\nCapri : pas ce marqueur-là."), None);
        assert_eq!(said(""), None);
    }

    #[test]
    fn the_marker_must_open_the_line_and_be_followed_by_a_separator() {
        assert_eq!(said("  [Capri] Indentée.").as_deref(), Some("Indentée."));
        assert_eq!(said("**[Capri]** En gras.").as_deref(), Some("En gras."));
        assert_eq!(said("[capri]: casse et deux-points").as_deref(), Some("casse et deux-points"));
        assert_eq!(said("[Capri] — avec un tiret").as_deref(), Some("avec un tiret"));
        assert_eq!(said("[Capri]\tune tabulation").as_deref(), Some("une tabulation"));
        // pas en début de ligne, ou pas le marqueur
        assert_eq!(said("Le style demande une ligne [Capri] à la fin."), None);
        assert_eq!(said("- [Capri] dans une liste"), None);
        assert_eq!(said("> [Capri] dans une citation"), None);
        assert_eq!(said("`[Capri] dans du code`"), None);
        assert_eq!(said("[Capri](https://exemple.fr) est un lien"), None);
        assert_eq!(said("[Capricorne] autre chose"), None);
        assert_eq!(said("[Capri]collé"), None);
        // le marqueur seul ne dit rien
        assert_eq!(said("[Capri]"), None);
        assert_eq!(said("[Capri]   "), None);
        assert_eq!(said("[Capri] **  **").as_deref(), Some("** **"), "le Markdown est retiré par l'app");
    }

    #[test]
    fn lines_inside_code_blocks_are_not_hers() {
        let example = "Voici le format attendu :\n```text\n[Capri] Exemple dans du code.\n```\nVoilà.";
        assert_eq!(said(example), None);
        let tilde =
            "~~~\n[Capri] Dans un bloc à tildes.\n```\n[Capri] Toujours dedans.\n~~~\n[Capri] Dehors.";
        assert_eq!(said(tilde).as_deref(), Some("Dehors."));
        // un bloc jamais refermé garde tout ce qui suit
        assert_eq!(said("```\n[Capri] Perdue."), None);
    }

    #[test]
    fn the_line_is_one_line_without_control_characters_and_bounded() {
        assert_eq!(
            said("[Capri] Des   blancs\u{7}  et\u{1b}[31m du contrôle").as_deref(),
            Some("Des blancs et [31m du contrôle")
        );
        assert_eq!(said("[Capri] Fin Windows\r\nautre").as_deref(), Some("Fin Windows"));

        let long = format!("[Capri] {}", "parole ".repeat(200));
        let cut = said(&long).unwrap();
        assert_eq!(cut.chars().count(), MAX_SAID_CHARS);
        assert!(cut.ends_with('…'));

        // des caractères de quatre octets : la borne en octets tient, sur une frontière
        let wide = format!("[Capri] {}", "😀".repeat(1_000));
        let cut = said(&wide).unwrap();
        assert!(cut.len() <= MAX_SAID_BYTES && cut.chars().count() <= MAX_SAID_CHARS);
        assert_eq!(bounded("é".repeat(MAX_SAID_BYTES)).len(), MAX_SAID_BYTES, "coupée entre deux « é »");
        assert_eq!(bounded(format!("a{}", "é".repeat(MAX_SAID_BYTES))).len(), MAX_SAID_BYTES - 1);
    }

    #[test]
    fn a_huge_answer_is_read_through_and_gives_a_small_line() {
        let huge = format!("{}\n[Capri] Au bout du tunnel.", "une très longue réponse ".repeat(200_000));
        assert!(huge.len() > 4_000_000);
        assert_eq!(said(&huge).as_deref(), Some("Au bout du tunnel."));
    }

    #[test]
    fn only_the_latest_waiting_line_is_said_and_not_too_late() {
        let line = |text: &str| Said::new(text.to_owned());
        let mut waiting = Waiting::new();
        assert_eq!(waiting.take(0), None);
        waiting.offer(line("première"), 0);
        waiting.offer(line("seconde"), 1_000);
        assert_eq!(waiting.take(5_000), Some(line("seconde")), "seule la plus récente");
        assert_eq!(waiting.take(5_000), None, "une seule fois");

        waiting.offer(line("périmée"), 10_000);
        assert_eq!(waiting.take(10_000 + WAIT_FOR_MS), None);
        waiting.offer(line("tue"), 20_000);
        waiting.clear();
        assert_eq!(waiting.take(20_001), None);
    }

    #[test]
    fn the_phrases_never_send_to_a_terminal() {
        for text in [ABOUT, TURNED_ON, TURNED_OFF, NOT_PLUGGED, NO_STYLE] {
            assert!(!text.contains("capri ") && !text.contains("--"), "{text}");
        }
        assert!(ABOUT.contains(MARKER) && ABOUT.contains("il pourrait y citer un nom de fichier"));
    }
}
