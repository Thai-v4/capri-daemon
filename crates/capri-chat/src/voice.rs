//! Ce qu'il faut pour que Capri dise ses réponses à voix haute, sans rien toucher au système.
//!
//! - [`Sentences`] découpe une réponse qui arrive par morceaux en phrases entières : la première
//!   peut être dite pendant que le modèle écrit encore la suite ;
//! - [`speakable`] donne ce qu'une voix doit prononcer d'une phrase affichée (sans émojis) ;
//! - [`pick`] choisit une voix parmi celles du système ;
//! - [`VoiceSettings`] est le contenu du fichier de réglages.
//!
//! Faire parler la machine est le travail de `capri_platform::Voice`.

use serde_json::{Value, json};

/// Une voix installée sur le système, telle que `capri-platform` la décrit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceInfo {
    /// L'identifiant que le système attend pour la désigner.
    pub id: String,
    pub name: String,
    /// La langue, sous la forme `fr-FR`.
    pub language: String,
    /// De 1 (voix compacte, livrée avec le système) à 3 (la meilleure, à télécharger).
    pub quality: u8,
    pub female: bool,
}

impl VoiceInfo {
    /// Une voix de fantaisie ou de synthèse à l'ancienne : intelligible, mais robotique. Ce sont
    /// les familles « eloquence » et « speech.synthesis » des identifiants d'Apple.
    fn is_robotic(&self) -> bool {
        self.id.contains(".eloquence.") || self.id.contains(".speech.synthesis.")
    }
}

/// La voix à utiliser pour la langue `language` (`fr`), parmi `voices`.
///
/// `wanted` est le choix de l'utilisateur, par identifiant ou par nom : il l'emporte s'il existe
/// encore. Sinon, la meilleure voix de la langue : une voix naturelle plutôt que robotique, une
/// voix de femme (c'est Capri qui parle), la meilleure qualité, puis le français de France. Le
/// choix ne dépend pas de l'ordre dans lequel le système liste ses voix.
pub fn pick<'a>(voices: &'a [VoiceInfo], language: &str, wanted: Option<&str>) -> Option<&'a VoiceInfo> {
    if let Some(wanted) = wanted.map(str::trim).filter(|wanted| !wanted.is_empty()) {
        let chosen = voices
            .iter()
            .find(|voice| voice.id == wanted)
            .or_else(|| voices.iter().find(|voice| voice.name.eq_ignore_ascii_case(wanted)));
        if chosen.is_some() {
            return chosen;
        }
    }
    let prefix = format!("{}-", language.to_ascii_lowercase());
    let home = format!("{0}-{1}", language.to_ascii_lowercase(), language.to_ascii_uppercase());
    voices
        .iter()
        .filter(|voice| {
            let spoken = voice.language.to_ascii_lowercase().replace('_', "-");
            spoken == language.to_ascii_lowercase() || spoken.starts_with(&prefix)
        })
        .max_by_key(|voice| {
            let at_home = voice.language.replace('_', "-").eq_ignore_ascii_case(&home);
            // une même voix existe parfois en deux tailles : la plus petite sonne moins bien
            let full_size = !voice.id.contains(".super-compact.");
            // à égalité, le nom puis l'identifiant départagent
            let by_name = std::cmp::Reverse((voice.name.clone(), voice.id.clone()));
            (!voice.is_robotic(), voice.female, voice.quality, full_size, at_home, by_name)
        })
}

/// Découpe une réponse en phrases, au fil de l'eau.
#[derive(Debug, Default)]
pub struct Sentences {
    pending: String,
}

impl Sentences {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute un morceau de texte et rend les phrases qu'il termine.
    ///
    /// Une phrase finit sur un point, un point d'exclamation, d'interrogation ou de suspension
    /// suivi d'un blanc (« 3.14 » ne coupe rien), ou sur un saut de ligne. Tant que le blanc
    /// n'est pas arrivé, la phrase attend.
    pub fn push(&mut self, text: &str) -> Vec<String> {
        self.pending.push_str(text);
        let mut sentences = Vec::new();
        while let Some(end) = sentence_end(&self.pending) {
            let rest = self.pending.split_off(end);
            let sentence = std::mem::replace(&mut self.pending, rest);
            if !sentence.trim().is_empty() {
                sentences.push(sentence.trim().to_owned());
            }
        }
        sentences
    }

    /// La réponse est finie : rend ce qui restait, même sans point final.
    pub fn finish(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.pending);
        (!rest.trim().is_empty()).then(|| rest.trim().to_owned())
    }
}

/// Où finit la première phrase complète de `text`, en octets, s'il y en a une.
fn sentence_end(text: &str) -> Option<usize> {
    let mut ended = false;
    for (at, c) in text.char_indices() {
        if c == '\n' {
            return Some(at + 1);
        }
        if matches!(c, '.' | '!' | '?' | '…') {
            ended = true;
        } else if ended && c.is_whitespace() {
            return Some(at);
        } else if !(ended && matches!(c, '»' | '"' | '”' | ')' | ']' | '\'' | '’')) {
            // un guillemet ou une parenthèse fermante appartient encore à la phrase
            ended = false;
        }
    }
    None
}

/// Ce qu'une voix doit prononcer de `text` : sans émojis ni puces, que les voix lisent en toutes
/// lettres, et avec des blancs simples. Vide s'il ne reste aucun mot à dire.
pub fn speakable(text: &str) -> String {
    let cleaned: String = text.chars().map(|c| if is_pictograph(c) { ' ' } else { c }).collect();
    let spoken = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if spoken.chars().any(char::is_alphanumeric) { spoken } else { String::new() }
}

/// Un émoji, un pictogramme, ou un caractère invisible qui les assemble.
fn is_pictograph(c: char) -> bool {
    matches!(
        u32::from(c),
        0x2022                  // la puce « • »
        | 0x200D                // liant sans chasse
        | 0x20E3                // touche entourée
        | 0x2190..=0x21FF       // flèches
        | 0x2300..=0x23FF       // signes techniques
        | 0x25A0..=0x27BF       // formes, symboles divers, casseaux
        | 0x2900..=0x297F       // flèches supplémentaires
        | 0x2B00..=0x2BFF       // symboles et flèches
        | 0x3030 | 0x303D | 0x3297 | 0x3299
        | 0xFE00..=0xFE0F       // sélecteurs de variante
        | 0x1F000..=0x1FAFF     // émojis et pictogrammes
        | 0xE0020..=0xE007F // étiquettes (drapeaux régionaux)
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("les réglages de la voix ne sont pas du JSON valide")]
pub struct InvalidVoiceSettings;

/// Le contenu du fichier de réglages de la voix. Sans fichier, Capri ne parle pas à voix haute.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoiceSettings {
    pub enabled: bool,
    /// La voix choisie, par identifiant ou par nom. `None` : la meilleure voix française.
    pub voice: Option<String>,
}

impl VoiceSettings {
    /// Lit le fichier de réglages. Ce qui manque prend sa valeur par défaut, ce qui est inconnu
    /// est ignoré.
    pub fn from_json(text: &str) -> Result<Self, InvalidVoiceSettings> {
        let settings: Value = serde_json::from_str(text).map_err(|_| InvalidVoiceSettings)?;
        if !settings.is_object() {
            return Err(InvalidVoiceSettings);
        }
        Ok(Self {
            enabled: settings["enabled"].as_bool().unwrap_or(false),
            voice: settings["voice"].as_str().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned),
        })
    }

    pub fn to_json(&self) -> String {
        let settings = json!({ "enabled": self.enabled, "voice": self.voice });
        let mut text =
            serde_json::to_string_pretty(&settings).expect("un objet JSON simple s'écrit toujours");
        text.push('\n');
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(id: &str, name: &str, language: &str, quality: u8, female: bool) -> VoiceInfo {
        VoiceInfo { id: id.to_owned(), name: name.to_owned(), language: language.to_owned(), quality, female }
    }

    /// Des voix comme en liste un Mac, identifiants compris.
    fn system_voices() -> Vec<VoiceInfo> {
        vec![
            voice("com.apple.voice.compact.en-US.Samantha", "Samantha", "en-US", 1, true),
            voice("com.apple.eloquence.fr-FR.Flo", "Flo", "fr-FR", 1, true),
            voice("com.apple.eloquence.fr-FR.Grandma", "Grandma", "fr-FR", 1, true),
            voice("com.apple.voice.compact.fr-FR.Thomas", "Thomas", "fr-FR", 1, false),
            voice("com.apple.voice.compact.fr-CA.Amelie", "Amélie", "fr-CA", 1, true),
            voice("com.apple.speech.synthesis.voice.Zarvox", "Zarvox", "fr-FR", 1, true),
        ]
    }

    #[test]
    fn picks_a_natural_french_voice_a_woman_s_if_there_is_one() {
        let mut voices = system_voices();
        assert_eq!(
            pick(&voices, "fr", None).map(|v| v.name.as_str()),
            Some("Amélie"),
            "ni robot, ni anglais"
        );

        // une meilleure voix de femme installée depuis : elle passe devant
        voices.push(voice("com.apple.voice.enhanced.fr-FR.Aurelie", "Aurélie (optimisée)", "fr-FR", 2, true));
        assert_eq!(pick(&voices, "fr", None).map(|v| v.quality), Some(2));
        voices.push(voice("com.apple.voice.premium.fr-FR.Audrey", "Audrey (premium)", "fr-FR", 3, true));
        assert_eq!(pick(&voices, "fr", None).map(|v| v.quality), Some(3));
        // une voix d'homme, même meilleure, ne passe pas devant une voix de femme
        voices.push(voice("com.apple.voice.premium.fr-FR.Thomas", "Thomas (premium)", "fr-FR", 3, false));
        assert_eq!(pick(&voices, "fr", None).map(|v| v.name.as_str()), Some("Audrey (premium)"));

        // à qualité égale, le français de France, puis le nom
        let twins = [
            voice("b", "Zoé", "fr-CA", 2, true),
            voice("c", "Marie", "fr_FR", 2, true),
            voice("a", "Anna", "fr-FR", 2, true),
        ];
        assert_eq!(pick(&twins, "fr", None).map(|v| v.name.as_str()), Some("Anna"));
        assert_eq!(pick(&twins[..2], "FR", None).map(|v| v.name.as_str()), Some("Marie"));

        // la même voix en deux tailles : la plus grande, quel que soit l'ordre de la liste
        let mut sizes = vec![
            voice("com.apple.voice.super-compact.fr-CA.Amelie", "Amélie", "fr-CA", 1, true),
            voice("com.apple.voice.compact.fr-CA.Amelie", "Amélie", "fr-CA", 1, true),
        ];
        assert_eq!(
            pick(&sizes, "fr", None).map(|v| v.id.as_str()),
            Some("com.apple.voice.compact.fr-CA.Amelie")
        );
        sizes.reverse();
        assert_eq!(
            pick(&sizes, "fr", None).map(|v| v.id.as_str()),
            Some("com.apple.voice.compact.fr-CA.Amelie")
        );
    }

    #[test]
    fn falls_back_on_what_exists() {
        let only_robots = [voice("com.apple.eloquence.fr-FR.Flo", "Flo", "fr-FR", 1, true)];
        assert_eq!(
            pick(&only_robots, "fr", None).map(|v| v.name.as_str()),
            Some("Flo"),
            "mieux que le silence"
        );
        let only_men = [voice("x", "Thomas", "fr-FR", 1, false), voice("y", "Jacques", "fr-FR", 1, false)];
        assert_eq!(pick(&only_men, "fr", None).map(|v| v.name.as_str()), Some("Jacques"));
        assert_eq!(pick(&system_voices(), "ja", None), None, "aucune voix de cette langue");
        assert_eq!(pick(&[], "fr", None), None);
        // « fr » ne prend pas une langue dont le code commence pareil
        assert_eq!(pick(&[voice("z", "Frysk", "fry-NL", 3, true)], "fr", None), None);
    }

    #[test]
    fn the_user_s_choice_wins_while_it_exists() {
        let voices = system_voices();
        let by_id = pick(&voices, "fr", Some("com.apple.voice.compact.fr-FR.Thomas"));
        assert_eq!(by_id.map(|v| v.name.as_str()), Some("Thomas"));
        assert_eq!(
            pick(&voices, "fr", Some(" thomas ")).map(|v| v.name.as_str()),
            Some("Thomas"),
            "par son nom"
        );
        assert_eq!(pick(&voices, "fr", Some("Samantha")).map(|v| v.language.as_str()), Some("en-US"));
        // une voix désinstallée depuis : on retombe sur la meilleure
        assert_eq!(pick(&voices, "fr", Some("Audrey")).map(|v| v.name.as_str()), Some("Amélie"));
        assert_eq!(pick(&voices, "fr", Some("  ")).map(|v| v.name.as_str()), Some("Amélie"));
    }

    /// Les phrases d'un texte livré par morceaux de `size` caractères.
    fn sentences_by(text: &str, size: usize) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut sentences = Sentences::new();
        let mut spoken = Vec::new();
        for piece in chars.chunks(size) {
            spoken.extend(sentences.push(&piece.iter().collect::<String>()));
        }
        spoken.extend(sentences.finish());
        spoken
    }

    #[test]
    fn a_reply_is_cut_into_whole_sentences_whatever_the_pieces() {
        let reply = "Canberra, bien sûr ! Mais tu savais que c'est une ville construite exprès ? Elle fait 3.14 fois rien… Enfin « presque ». Voilà\nEt une dernière sans point";
        let expected = [
            "Canberra, bien sûr !",
            "Mais tu savais que c'est une ville construite exprès ?",
            "Elle fait 3.14 fois rien…",
            "Enfin « presque ».",
            "Voilà",
            "Et une dernière sans point",
        ];
        for size in [1, 2, 5, 17, 1000] {
            assert_eq!(sentences_by(reply, size), expected, "morceaux de {size} caractères");
        }
    }

    #[test]
    fn a_sentence_waits_for_the_blank_that_ends_it() {
        let mut sentences = Sentences::new();
        assert!(sentences.push("Ça marche").is_empty());
        assert!(sentences.push(" bien.").is_empty(), "peut-être « bien.net » : on attend la suite");
        assert_eq!(sentences.push(" Et").as_slice(), ["Ça marche bien."]);
        assert!(sentences.push(" toi ?!").is_empty());
        assert_eq!(sentences.push("\" Non.) ").as_slice(), ["Et toi ?!\"", "Non.)"]);
        assert_eq!(
            sentences.push("\n\n  \n"),
            Vec::<String>::new(),
            "des lignes vides ne sont pas des phrases"
        );
        assert_eq!(sentences.finish(), None);
        assert_eq!(sentences_by("", 3), Vec::<String>::new());
    }

    #[test]
    fn only_words_are_spoken() {
        assert_eq!(speakable("Salut toi ! 😉"), "Salut toi !");
        assert_eq!(
            speakable("• Écoute une chanson :  danse\t2 minutes ⏱️ 🕺🏽"),
            "Écoute une chanson : danse 2 minutes"
        );
        assert_eq!(speakable("J'ai 👍 fini → tout va bien ✅"), "J'ai fini tout va bien");
        assert_eq!(speakable("Ça c'est « noté », l'accent : déjà."), "Ça c'est « noté », l'accent : déjà.");
        for silent in ["😉", "…", " ! ", "", "🇫🇷 👨‍👩‍👧", "— — —"] {
            assert_eq!(speakable(silent), "", "{silent:?}");
        }
    }

    #[test]
    fn voice_settings_are_written_and_read_back() {
        assert_eq!(
            VoiceSettings::default(),
            VoiceSettings { enabled: false, voice: None },
            "muette par défaut"
        );
        for settings in [
            VoiceSettings { enabled: true, voice: None },
            VoiceSettings { enabled: false, voice: Some("com.apple.voice.compact.fr-CA.Amelie".to_owned()) },
        ] {
            assert_eq!(VoiceSettings::from_json(&settings.to_json()), Ok(settings));
        }
        assert_eq!(VoiceSettings::from_json("{}"), Ok(VoiceSettings::default()));
        assert_eq!(
            VoiceSettings::from_json(r#"{"enabled":true,"voice":"  ","futur":1}"#),
            Ok(VoiceSettings { enabled: true, voice: None })
        );
        for broken in ["", "pas du json", "[]", "true"] {
            assert_eq!(VoiceSettings::from_json(broken), Err(InvalidVoiceSettings), "{broken:?}");
        }
    }
}
