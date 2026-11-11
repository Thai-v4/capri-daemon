//! Ce qu'il faut pour que Capri dise ses réponses à voix haute, sans rien toucher au système.
//!
//! - [`Sentences`] découpe une réponse qui arrive par morceaux en phrases entières : la première
//!   peut être dite pendant que le modèle écrit encore la suite ;
//! - [`speakable`] donne ce qu'une voix doit prononcer d'une phrase affichée (sans émojis) ;
//! - [`pick`] choisit une voix parmi celles du système ;
//! - [`VoiceEffect`], [`Intensity`], [`Pitch`] et [`Rate`] sont ce qu'on règle de sa façon de
//!   parler : des préréglages d'effets écrits pour elle, et des niveaux nommés ;
//! - [`delivery`] en fait la description neutre que la voix du système applique
//!   (`capri_core::sound::Delivery`) ;
//! - [`VoiceSettings`] est le contenu du fichier de réglages.
//!
//! Les effets sont un traitement du son appliqué après la synthèse, sur la machine : ils font
//! entendre que Capri est une construct, une conscience numérique, sans imiter la voix de la
//! comédienne de la série (ce qu'on ne fera pas). L'idée d'effets de lecture vient de
//! CapriChooms/CapriCore ; les préréglages ci-dessous sont les nôtres, ni son code ni ses valeurs.
//!
//! Faire parler la machine est le travail de `capri_platform::Voice`.

use capri_core::sound::{self, Band, Delivery, DistortionKind, Filter, Room};
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

/// La langue dans laquelle Capri parle.
pub const LANGUAGE: &str = "fr";
/// Ce qu'elle dit quand on veut l'écouter.
pub const TEST_SENTENCE: &str = "Salut, c'est Capri. Tu m'entends bien ?";
/// Où télécharger de meilleures voix (libellés de macOS 27).
pub const BETTER_VOICES: &str = "Pour une voix plus naturelle : Réglages Système > Accessibilité > Lire et énoncer \
                                 > Voix système, et télécharge une voix française de meilleure qualité.";

/// Sa voix vient d'être activée.
pub const VOICE_ON: &str = "Capri dira ses réponses à voix haute, dès la prochaine.";
/// Sa voix vient d'être coupée.
pub const VOICE_OFF: &str = "Capri ne répondra plus que par écrit.";

/// Une voix en une ligne : « Amélie [fr-CA, voix de femme, qualité de base] com.exemple.voix ».
pub fn describe(voice: &VoiceInfo) -> String {
    format!("{} [{}, {}, {}] {}", voice.name, voice.language, gender(voice), quality(voice), voice.id)
}

/// « qualité de base », « qualité améliorée », « qualité supérieure ».
pub fn quality(voice: &VoiceInfo) -> &'static str {
    match voice.quality {
        3.. => "qualité supérieure",
        2 => "qualité améliorée",
        _ => "qualité de base",
    }
}

fn gender(voice: &VoiceInfo) -> &'static str {
    if voice.female { "voix de femme" } else { "autre voix" }
}

/// Les voix de la langue de Capri, de la meilleure qualité à la moins bonne, puis par nom.
pub fn french(voices: &[VoiceInfo]) -> Vec<&VoiceInfo> {
    let mut french: Vec<&VoiceInfo> =
        voices.iter().filter(|voice| pick(std::slice::from_ref(voice), LANGUAGE, None).is_some()).collect();
    french.sort_by(|a, b| (b.quality, &a.name, &a.language).cmp(&(a.quality, &b.name, &b.language)));
    french
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

/// Un traitement de sa voix, appliqué après la synthèse. « Naturelle » n'en applique aucun : la
/// voix du système, exactement comme sans ce réglage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoiceEffect {
    #[default]
    Natural,
    /// Discret : un léger grain électronique et une petite salle.
    Construct,
    /// Une voix serrée et un peu saturée, comme dans un casque de liaison.
    Radio,
    /// Plus marqué : des échos métalliques rapprochés, du grain et une réverbération brillante.
    Hologram,
}

impl VoiceEffect {
    pub const ALL: [Self; 4] = [Self::Natural, Self::Construct, Self::Radio, Self::Hologram];

    /// Son nom dans `voice.json` et en ligne de commande. Ne jamais le changer : un fichier
    /// existant ne serait plus compris.
    pub fn id(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::Construct => "construct",
            Self::Radio => "radio",
            Self::Hologram => "hologram",
        }
    }

    /// Son nom affiché.
    pub fn name(self) -> &'static str {
        match self {
            Self::Natural => "Naturelle",
            Self::Construct => "Construct",
            Self::Radio => "Liaison radio",
            Self::Hologram => "Hologramme",
        }
    }

    /// Ce qu'on devrait entendre, en quelques mots.
    pub fn summary(self) -> &'static str {
        match self {
            Self::Natural => "sans effet, la voix telle que le système la dit",
            Self::Construct => "un léger grain électronique et une petite salle",
            Self::Radio => "une voix serrée et un peu saturée, comme dans un casque de liaison",
            Self::Hologram => "des échos métalliques rapprochés, du grain et une réverbération brillante",
        }
    }

    /// Son identifiant, ou son nom en français, sans tenir compte de la casse ni des accents.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = fold(text);
        Self::ALL.into_iter().find(|effect| wanted == effect.id() || wanted == fold(effect.name())).or(
            match wanted.as_str() {
                "aucun" | "sans" | "naturel" => Some(Self::Natural),
                "hologramme" => Some(Self::Hologram),
                _ => None,
            },
        )
    }

    /// Les traitements de ce préréglage, à cette intensité, dans l'ordre où le son les traverse.
    /// Les valeurs sont les nôtres, choisies pour une voix française d'environ 16 à 22 kHz.
    pub fn effects(self, intensity: Intensity) -> Vec<sound::Effect> {
        use sound::Effect::{Delay, Distortion, Equalizer, Reverb};
        let k = intensity.scale();
        let band = |filter, frequency, bandwidth, gain| Band { filter, frequency, bandwidth, gain };
        match self {
            Self::Natural => Vec::new(),
            Self::Construct => vec![
                // le grain : un peu de son « décimé » (moins de bits, moins d'échantillons)
                Distortion { kind: DistortionKind::MultiDecimated1, pre_gain: -6.0, mix: 8.0 * k },
                Equalizer {
                    bands: vec![
                        band(Filter::HighPass, 110.0, 1.0, 0.0),
                        // un peu de présence, comme une voix qui sort d'un circuit
                        band(Filter::Parametric, 2_800.0, 1.2, 2.0 * k),
                    ],
                    gain: 0.0,
                },
                Reverb { room: Room::SmallRoom, mix: 10.0 * k },
            ],
            Self::Radio => vec![
                Distortion { kind: DistortionKind::MultiDistortedCubed, pre_gain: -6.0, mix: 12.0 * k },
                Equalizer {
                    bands: vec![
                        // la bande d'un émetteur : plus étroite à mesure que l'intensité monte
                        band(Filter::HighPass, 250.0 + 150.0 * k, 1.0, 0.0),
                        band(Filter::LowPass, 4_200.0 - 800.0 * k, 1.0, 0.0),
                        band(Filter::Parametric, 1_700.0, 1.0, 4.0 * k),
                    ],
                    // le filtrage enlève de l'énergie, la bosse de présence en rend : à peine de gain en plus,
                    // sans saturer (crête mesurée sous 1 à chaque intensité)
                    gain: 0.5,
                },
            ],
            Self::Hologram => vec![
                Distortion { kind: DistortionKind::MultiDecimated2, pre_gain: -6.0, mix: 10.0 * k },
                Equalizer {
                    bands: vec![
                        band(Filter::HighPass, 160.0, 1.0, 0.0),
                        band(Filter::HighShelf, 5_000.0, 1.0, 3.0 * k),
                    ],
                    // un peu de marge : les échos réinjectés montent les crêtes
                    gain: -1.0,
                },
                // des échos très rapprochés et réinjectés : un timbre de peigne, métallique
                Delay { seconds: 0.022, feedback: 40.0 * k, cutoff: 7_000.0, mix: 25.0 * k },
                Reverb { room: Room::Plate, mix: 14.0 * k },
            ],
        }
    }
}

/// L'intensité d'un effet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Intensity {
    Light,
    #[default]
    Medium,
    Strong,
}

impl Intensity {
    pub const ALL: [Self; 3] = [Self::Light, Self::Medium, Self::Strong];

    /// Son niveau dans `voice.json` et en ligne de commande : de 1 à 3.
    pub fn level(self) -> i64 {
        match self {
            Self::Light => 1,
            Self::Medium => 2,
            Self::Strong => 3,
        }
    }

    /// Le niveau le plus proche de `level`.
    pub fn from_level(level: i64) -> Self {
        match level {
            ..=1 => Self::Light,
            2 => Self::Medium,
            3.. => Self::Strong,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Light => "légère",
            Self::Medium => "moyenne",
            Self::Strong => "forte",
        }
    }

    /// Son niveau (1 à 3) ou son nom (« forte »), comme on le tape au terminal.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = fold(text);
        Self::ALL
            .into_iter()
            .find(|level| wanted == level.level().to_string() || wanted == fold(level.name()))
    }

    /// Ce par quoi multiplier la part d'effet d'un préréglage.
    fn scale(self) -> f32 {
        match self {
            Self::Light => 0.6,
            Self::Medium => 1.0,
            Self::Strong => 1.5,
        }
    }
}

/// La hauteur de sa voix, par rapport à celle de la voix du système.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pitch {
    Lower,
    Low,
    #[default]
    Normal,
    High,
    Higher,
}

impl Pitch {
    pub const ALL: [Self; 5] = [Self::Lower, Self::Low, Self::Normal, Self::High, Self::Higher];

    /// Son cran dans `voice.json` et en ligne de commande : de -2 à 2.
    pub fn step(self) -> i64 {
        match self {
            Self::Lower => -2,
            Self::Low => -1,
            Self::Normal => 0,
            Self::High => 1,
            Self::Higher => 2,
        }
    }

    /// Le cran le plus proche de `step`.
    pub fn from_step(step: i64) -> Self {
        match step {
            ..=-2 => Self::Lower,
            -1 => Self::Low,
            0 => Self::Normal,
            1 => Self::High,
            2.. => Self::Higher,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Lower => "plus grave",
            Self::Low => "un peu grave",
            Self::Normal => "normale",
            Self::High => "un peu aiguë",
            Self::Higher => "plus aiguë",
        }
    }

    /// Son cran (-2 à 2) ou son nom (« un peu grave »), comme on le tape au terminal.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = fold(text);
        Self::ALL.into_iter().find(|level| step_or_name(&wanted, level.step(), level.name()))
    }

    /// Le multiplicateur demandé au système. Il monte fidèlement mais descend moins qu'annoncé
    /// sur les voix françaises de base : mesuré sur ce Mac (fréquence fondamentale des voix
    /// « compactes » Amélie et Thomas), 0,8 la baisse d'environ 5 %, 0,55 d'environ 12 %, quand
    /// 1,06 la monte de 6 % et 1,14 de 14 %. Les voix « eloquence » descendent davantage.
    pub fn multiplier(self) -> f32 {
        match self {
            Self::Lower => 0.55,
            Self::Low => 0.8,
            Self::Normal => 1.0,
            Self::High => 1.06,
            Self::Higher => 1.14,
        }
    }
}

/// Son débit, par rapport à celui de la voix du système.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rate {
    Slower,
    Slow,
    #[default]
    Normal,
    Fast,
    Faster,
}

impl Rate {
    pub const ALL: [Self; 5] = [Self::Slower, Self::Slow, Self::Normal, Self::Fast, Self::Faster];

    /// Son cran dans `voice.json` et en ligne de commande : de -2 à 2.
    pub fn step(self) -> i64 {
        match self {
            Self::Slower => -2,
            Self::Slow => -1,
            Self::Normal => 0,
            Self::Fast => 1,
            Self::Faster => 2,
        }
    }

    /// Le cran le plus proche de `step`.
    pub fn from_step(step: i64) -> Self {
        match step {
            ..=-2 => Self::Slower,
            -1 => Self::Slow,
            0 => Self::Normal,
            1 => Self::Fast,
            2.. => Self::Faster,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Slower => "lent",
            Self::Slow => "posé",
            Self::Normal => "normal",
            Self::Fast => "vif",
            Self::Faster => "rapide",
        }
    }

    /// Son cran (-2 à 2) ou son nom (« posé »), comme on le tape au terminal.
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = fold(text);
        Self::ALL.into_iter().find(|level| step_or_name(&wanted, level.step(), level.name()))
    }

    /// Le débit demandé au système, relatif à sa plage (`capri_core::sound::RATE`). Sa plage
    /// n'est pas linéaire : mesuré sur ce Mac (durée d'une même phrase), -0,26 la dit environ
    /// 1,2 fois plus lentement, -0,12 1,1 fois, quand +0,04 la dit 1,08 fois plus vite et +0,09
    /// 1,23 fois.
    pub fn relative(self) -> f32 {
        match self {
            Self::Slower => -0.26,
            Self::Slow => -0.12,
            Self::Normal => 0.0,
            Self::Fast => 0.04,
            Self::Faster => 0.09,
        }
    }
}

/// Comment dire ses phrases avec ces réglages et cette voix (`None` : celle du système).
/// « Naturelle », à la hauteur et au débit normaux, ne demande rien de plus que la voix.
pub fn delivery(settings: &VoiceSettings, voice: Option<String>) -> Delivery {
    Delivery {
        voice,
        pitch: settings.pitch.multiplier(),
        rate: settings.rate.relative(),
        effects: settings.effect.effects(settings.intensity),
    }
}

/// « Construct (intensité moyenne), hauteur un peu grave, débit normal ».
pub fn describe_style(settings: &VoiceSettings) -> String {
    let effect = match settings.effect {
        VoiceEffect::Natural => VoiceEffect::Natural.name().to_owned(),
        effect => format!("{} (intensité {})", effect.name(), settings.intensity.name()),
    };
    format!("{effect}, hauteur {}, débit {}", settings.pitch.name(), settings.rate.name())
}

/// Ce qu'on a tapé (déjà replié par [`fold`]) désigne-t-il ce cran, par son nombre (`+1` compris)
/// ou par son nom ?
fn step_or_name(wanted: &str, step: i64, name: &str) -> bool {
    wanted.strip_prefix('+').unwrap_or(wanted) == step.to_string() || wanted == fold(name)
}

/// En minuscules et sans accents, pour comparer ce qu'on tape à un nom.
fn fold(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("les réglages de la voix ne sont pas du JSON valide")]
pub struct InvalidVoiceSettings;

/// Le contenu du fichier de réglages de la voix. Sans fichier, Capri ne parle pas à voix haute.
/// Un fichier écrit avant les effets, sans `effect`, `intensity`, `pitch` ni `rate`, donne la
/// voix d'avant : naturelle, à sa hauteur et à son débit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoiceSettings {
    pub enabled: bool,
    /// La voix choisie, par identifiant ou par nom. `None` : la meilleure voix française.
    pub voice: Option<String>,
    pub effect: VoiceEffect,
    pub intensity: Intensity,
    pub pitch: Pitch,
    pub rate: Rate,
}

impl VoiceSettings {
    /// Lit le fichier de réglages. Ce qui manque prend sa valeur par défaut, ce qui est inconnu
    /// est ignoré, un niveau hors bornes est ramené au plus proche.
    pub fn from_json(text: &str) -> Result<Self, InvalidVoiceSettings> {
        let settings: Value = serde_json::from_str(text).map_err(|_| InvalidVoiceSettings)?;
        if !settings.is_object() {
            return Err(InvalidVoiceSettings);
        }
        // un nombre, même écrit 2.0 ; arrondi, et rien d'autre
        let level = |key: &str| {
            settings[key].as_i64().or_else(|| {
                settings[key].as_f64().filter(|value| value.is_finite()).map(|value| value.round() as i64)
            })
        };
        Ok(Self {
            enabled: settings["enabled"].as_bool().unwrap_or(false),
            voice: settings["voice"].as_str().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned),
            effect: settings["effect"].as_str().and_then(VoiceEffect::parse).unwrap_or_default(),
            intensity: level("intensity").map(Intensity::from_level).unwrap_or_default(),
            pitch: level("pitch").map(Pitch::from_step).unwrap_or_default(),
            rate: level("rate").map(Rate::from_step).unwrap_or_default(),
        })
    }

    pub fn to_json(&self) -> String {
        let settings = json!({
            "enabled": self.enabled,
            "voice": self.voice,
            "effect": self.effect.id(),
            "intensity": self.intensity.level(),
            "pitch": self.pitch.step(),
            "rate": self.rate.step(),
        });
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
    fn a_voice_is_described_in_one_line_and_french_ones_come_best_first() {
        let amelie = voice("com.exemple.voix", "Amélie", "fr-CA", 1, true);
        assert_eq!(describe(&amelie), "Amélie [fr-CA, voix de femme, qualité de base] com.exemple.voix");
        let mut voices = system_voices();
        voices.push(voice("com.apple.voice.premium.fr-FR.Audrey", "Audrey", "fr-FR", 3, true));
        let names: Vec<&str> = french(&voices).iter().map(|voice| voice.name.as_str()).collect();
        assert_eq!(names, ["Audrey", "Amélie", "Flo", "Grandma", "Thomas", "Zarvox"], "sans Samantha");
        assert_eq!(quality(&voices[6]), "qualité supérieure");
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
            VoiceSettings {
                enabled: false,
                voice: None,
                effect: VoiceEffect::Natural,
                intensity: Intensity::Medium,
                pitch: Pitch::Normal,
                rate: Rate::Normal,
            },
            "muette par défaut, et naturelle"
        );
        for settings in [
            VoiceSettings { enabled: true, ..VoiceSettings::default() },
            VoiceSettings {
                voice: Some("com.apple.voice.compact.fr-CA.Amelie".to_owned()),
                ..VoiceSettings::default()
            },
        ] {
            assert_eq!(VoiceSettings::from_json(&settings.to_json()), Ok(settings));
        }
        // toutes les combinaisons
        for effect in VoiceEffect::ALL {
            for intensity in Intensity::ALL {
                for (pitch, rate) in Pitch::ALL.into_iter().zip(Rate::ALL.into_iter().rev()) {
                    let settings =
                        VoiceSettings { enabled: true, voice: None, effect, intensity, pitch, rate };
                    assert_eq!(VoiceSettings::from_json(&settings.to_json()), Ok(settings));
                }
            }
        }
        assert_eq!(VoiceSettings::from_json("{}"), Ok(VoiceSettings::default()));
        assert_eq!(
            VoiceSettings::from_json(r#"{"enabled":true,"voice":"  ","futur":1}"#),
            Ok(VoiceSettings { enabled: true, ..VoiceSettings::default() })
        );
        for broken in ["", "pas du json", "[]", "true"] {
            assert_eq!(VoiceSettings::from_json(broken), Err(InvalidVoiceSettings), "{broken:?}");
        }
    }

    #[test]
    fn a_file_written_before_the_effects_gives_the_voice_of_before() {
        let before = r#"{
  "enabled": true,
  "voice": "com.apple.voice.compact.fr-CA.Amelie"
}
"#;
        let settings = VoiceSettings::from_json(before).expect("lisible");
        assert_eq!(settings.effect, VoiceEffect::Natural);
        let delivery = delivery(&settings, Some("com.apple.voice.compact.fr-CA.Amelie".to_owned()));
        assert_eq!(delivery, Delivery::plain(Some("com.apple.voice.compact.fr-CA.Amelie".to_owned())));
        assert!(delivery.natural_prosody(), "rien n'est demandé de plus au système");
    }

    #[test]
    fn unknown_or_out_of_range_values_break_nothing() {
        let settings = VoiceSettings::from_json(
            r#"{"enabled":true,"effect":"chorale","intensity":9,"pitch":-40,"rate":2.6}"#,
        )
        .expect("lisible");
        assert_eq!(
            (settings.effect, settings.intensity, settings.pitch, settings.rate),
            (VoiceEffect::Natural, Intensity::Strong, Pitch::Lower, Rate::Faster)
        );
        let settings =
            VoiceSettings::from_json(r#"{"effect":42,"intensity":"fort","pitch":null,"rate":[1],"voice":7}"#)
                .expect("lisible");
        assert_eq!(settings, VoiceSettings::default(), "d'autres types : les valeurs par défaut");
        let settings =
            VoiceSettings::from_json(r#"{"intensity":0,"pitch":1.4,"rate":-1e300}"#).expect("lisible");
        assert_eq!(
            (settings.intensity, settings.pitch, settings.rate),
            (Intensity::Light, Pitch::High, Rate::Slower)
        );
        assert_eq!(
            VoiceSettings::from_json(r#"{"effect":" Hologramme "}"#).map(|settings| settings.effect),
            Ok(VoiceEffect::Hologram),
            "un nom français, mal tapé, est compris"
        );
    }

    #[test]
    fn effects_are_named_and_found_by_id_or_french_name() {
        for effect in VoiceEffect::ALL {
            assert_eq!(VoiceEffect::parse(effect.id()), Some(effect));
            assert_eq!(VoiceEffect::parse(&effect.name().to_uppercase()), Some(effect));
        }
        assert_eq!(VoiceEffect::parse("liaison radio"), Some(VoiceEffect::Radio));
        assert_eq!(VoiceEffect::parse("hologramme"), Some(VoiceEffect::Hologram));
        assert_eq!(VoiceEffect::parse("aucun"), Some(VoiceEffect::Natural));
        assert_eq!(VoiceEffect::parse("chorale"), None);
        assert_eq!(VoiceEffect::parse(""), None);
        let ids: Vec<&str> = VoiceEffect::ALL.iter().map(|effect| effect.id()).collect();
        assert_eq!(
            ids,
            ["natural", "construct", "radio", "hologram"],
            "ces noms sont écrits dans voice.json"
        );
    }

    #[test]
    fn natural_is_no_processing_at_all_and_the_others_stay_within_bounds() {
        for intensity in Intensity::ALL {
            assert!(VoiceEffect::Natural.effects(intensity).is_empty());
        }
        for effect in [VoiceEffect::Construct, VoiceEffect::Radio, VoiceEffect::Hologram] {
            let chains: Vec<Vec<sound::Effect>> =
                Intensity::ALL.iter().map(|&intensity| effect.effects(intensity)).collect();
            for chain in &chains {
                assert!(!chain.is_empty() && chain.len() <= sound::MAX_EFFECTS);
                let delivery = Delivery { effects: chain.clone(), ..Delivery::plain(None) };
                assert_eq!(delivery.bounded(), delivery, "{effect:?} : rien n'est hors bornes");
            }
            // plus d'intensité, plus d'effet mélangé au son d'origine
            let mixed = |chain: &Vec<sound::Effect>| -> f32 {
                chain
                    .iter()
                    .map(|effect| match effect {
                        sound::Effect::Reverb { mix, .. }
                        | sound::Effect::Delay { mix, .. }
                        | sound::Effect::Distortion { mix, .. } => *mix,
                        sound::Effect::Equalizer { .. } => 0.0,
                    })
                    .sum()
            };
            assert!(
                mixed(&chains[0]) < mixed(&chains[1]) && mixed(&chains[1]) < mixed(&chains[2]),
                "{effect:?}"
            );
        }
        let all: Vec<Vec<sound::Effect>> =
            VoiceEffect::ALL.iter().map(|effect| effect.effects(Intensity::Medium)).collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "deux préréglages identiques");
            }
        }
    }

    #[test]
    fn levels_are_ordered_and_normal_asks_nothing() {
        let pitches: Vec<f32> = Pitch::ALL.iter().map(|pitch| pitch.multiplier()).collect();
        assert!(pitches.windows(2).all(|pair| pair[0] < pair[1]), "{pitches:?}");
        assert!(pitches.iter().all(|pitch| sound::PITCH.contains(pitch)));
        let rates: Vec<f32> = Rate::ALL.iter().map(|rate| rate.relative()).collect();
        assert!(rates.windows(2).all(|pair| pair[0] < pair[1]), "{rates:?}");
        assert!(rates.iter().all(|rate| sound::RATE.contains(rate)));
        assert_eq!((Pitch::Normal.multiplier(), Rate::Normal.relative()), (1.0, 0.0));
        for step in -2..=2 {
            assert_eq!(Pitch::from_step(step).step(), step);
            assert_eq!(Rate::from_step(step).step(), step);
        }
        for level in 1..=3 {
            assert_eq!(Intensity::from_level(level).level(), level);
        }
        assert_eq!((Pitch::from_step(i64::MIN), Rate::from_step(i64::MAX)), (Pitch::Lower, Rate::Faster));
        assert_eq!(
            (Intensity::from_level(-5), Intensity::from_level(99)),
            (Intensity::Light, Intensity::Strong)
        );
    }

    #[test]
    fn levels_are_found_by_number_or_by_name() {
        assert_eq!(Intensity::parse("3"), Some(Intensity::Strong));
        assert_eq!(Intensity::parse(" Légère "), Some(Intensity::Light));
        assert_eq!(Intensity::parse("legere"), Some(Intensity::Light));
        assert_eq!(Pitch::parse("-2"), Some(Pitch::Lower));
        assert_eq!(Pitch::parse("+1"), Some(Pitch::High));
        assert_eq!(Pitch::parse("un peu aigue"), Some(Pitch::High));
        assert_eq!(Pitch::parse("normale"), Some(Pitch::Normal));
        assert_eq!(Rate::parse("posé"), Some(Rate::Slow));
        assert_eq!(Rate::parse("2"), Some(Rate::Faster));
        for wrong in ["4", "0", "fort", ""] {
            assert_eq!(Intensity::parse(wrong), None, "{wrong:?}");
        }
        for wrong in ["3", "-3", "aiguë", "1.5", "++1"] {
            assert_eq!(Pitch::parse(wrong), None, "{wrong:?}");
            assert_eq!(Rate::parse(wrong), None, "{wrong:?}");
        }
    }

    #[test]
    fn the_settings_become_a_description_for_the_system() {
        let settings = VoiceSettings {
            enabled: true,
            voice: None,
            effect: VoiceEffect::Radio,
            intensity: Intensity::Strong,
            pitch: Pitch::Low,
            rate: Rate::Fast,
        };
        let delivery = delivery(&settings, Some("une.voix".to_owned()));
        assert_eq!(delivery.voice.as_deref(), Some("une.voix"));
        assert_eq!((delivery.pitch, delivery.rate), (Pitch::Low.multiplier(), Rate::Fast.relative()));
        assert_eq!(delivery.effects, VoiceEffect::Radio.effects(Intensity::Strong));
        assert_eq!(
            describe_style(&settings),
            "Liaison radio (intensité forte), hauteur un peu grave, débit vif"
        );
        assert_eq!(describe_style(&VoiceSettings::default()), "Naturelle, hauteur normale, débit normal");
    }
}
