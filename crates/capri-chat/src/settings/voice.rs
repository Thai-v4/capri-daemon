//! La rubrique « Voix » : dire ses réponses à voix haute ou non, avec quelle voix, et
//! l'écouter.
//!
//! Les mêmes règles que `capri voice` : les voix sont celles du système, la française de
//! meilleure qualité par défaut ([`pick`]), et le choix est écrit dans `voice.json`. Rien n'est
//! téléchargé ni envoyé.

use capri_core::form::{Button, Control, FormEvent, Row, Tone};

use super::{Effect, Environment, Outcome, Rubric};
use crate::voice::{
    BETTER_VOICES, LANGUAGE, TEST_SENTENCE, VOICE_OFF, VOICE_ON, VoiceInfo, VoiceSettings, french, pick,
    quality, speakable,
};

pub const ID: &str = "voice";

pub const ENABLED: &str = "voice.enabled";
pub const CHOICE: &str = "voice.choice";
pub const NOTE: &str = "voice.note";
pub const ACTIONS: &str = "voice.actions";
pub const LISTEN: &str = "voice.listen";
pub const SAVED: &str = "voice.saved";

pub struct VoiceRubric {
    /// Capri a-t-elle une voix sur ce système ?
    available: bool,
    /// `None` : pas encore lu.
    settings: Option<Result<VoiceSettings, String>>,
    voices: Vec<VoiceInfo>,
    saved: Option<(Tone, String)>,
}

impl VoiceRubric {
    pub fn new(environment: &Environment) -> Self {
        Self { available: environment.voice, settings: None, voices: Vec::new(), saved: None }
    }

    fn current(&self) -> Option<&VoiceSettings> {
        self.settings.as_ref().and_then(|settings| settings.as_ref().ok())
    }

    /// Les voix de la liste, après « automatique ».
    fn listed(&self) -> Vec<&VoiceInfo> {
        french(&self.voices)
    }

    /// La voix qui parlera, d'après les réglages.
    fn speaking(&self) -> Option<&VoiceInfo> {
        pick(&self.voices, LANGUAGE, self.current().and_then(|settings| settings.voice.as_deref()))
    }

    /// La voix choisie à la main, si elle est encore installée.
    fn wanted(&self) -> Option<&VoiceInfo> {
        let wanted = self.current()?.voice.as_deref()?;
        pick(&self.voices, LANGUAGE, Some(wanted))
            .filter(|voice| voice.id == wanted || voice.name.eq_ignore_ascii_case(wanted.trim()))
    }

    fn save(&mut self, settings: VoiceSettings) -> Vec<Effect> {
        self.saved = Some((Tone::Busy, "Un instant…".to_owned()));
        vec![Effect::SaveVoice(settings)]
    }
}

/// « Amélie (fr-CA, qualité de base) ».
fn label(voice: &VoiceInfo) -> String {
    format!("{} ({}, {})", voice.name, voice.language, quality(voice))
}

impl Rubric for VoiceRubric {
    fn id(&self) -> &'static str {
        ID
    }

    fn title(&self) -> &'static str {
        "Voix"
    }

    fn rows(&self) -> Vec<Row> {
        let usable = self.available && self.current().is_some();
        let enabled = self.current().is_some_and(|settings| settings.enabled);
        let mut rows = vec![Row::new(
            ENABLED,
            "",
            Control::Check {
                title: "Dire ses réponses à voix haute".to_owned(),
                checked: enabled,
                enabled: usable,
            },
        )];

        let listed = self.listed();
        let automatic = match pick(&self.voices, LANGUAGE, None) {
            Some(best) => format!("Automatique : la meilleure installée ({})", label(best)),
            None => "Automatique : la voix du système".to_owned(),
        };
        let wanted = self.wanted();
        let selected = wanted
            .and_then(|wanted| listed.iter().position(|voice| voice.id == wanted.id))
            .map_or(0, |index| index + 1);
        let options = std::iter::once(automatic).chain(listed.iter().map(|voice| label(voice))).collect();
        rows.push(Row::new(CHOICE, "Voix", Control::Choice { options, selected, enabled: usable }));

        let note = match &self.settings {
            _ if !self.available => {
                Some((Tone::Quiet, "Capri n'a pas encore de voix sur ce système.".to_owned()))
            }
            None => Some((Tone::Busy, "Lecture de ses réglages…".to_owned())),
            Some(Err(problem)) => Some((Tone::Bad, problem.clone())),
            Some(Ok(settings)) => {
                let lost = settings.voice.as_ref().filter(|_| wanted.is_none()).map(|lost| {
                    format!("La voix choisie, « {lost} », n'est plus installée : Capri prend la meilleure à sa place. ")
                });
                let advice = match self.speaking() {
                    _ if listed.is_empty() => {
                        Some(format!("Aucune voix française n'est installée. {BETTER_VOICES}"))
                    }
                    Some(voice) if voice.quality <= 1 => Some(BETTER_VOICES.to_owned()),
                    _ => None,
                };
                match (lost, advice) {
                    (None, None) => None,
                    (lost, advice) => Some((
                        Tone::Quiet,
                        format!("{}{}", lost.unwrap_or_default(), advice.unwrap_or_default())
                            .trim()
                            .to_owned(),
                    )),
                }
            }
        };
        if let Some((tone, text)) = note {
            rows.push(Row::new(NOTE, "", Control::Note { text, tone }));
        }
        rows.push(Row::new(
            ACTIONS,
            "",
            Control::Buttons(vec![Button::new(LISTEN, "Écouter", self.available && self.settings.is_some())]),
        ));
        if let Some((tone, text)) = &self.saved {
            rows.push(Row::new(SAVED, "", Control::Note { text: text.clone(), tone: *tone }));
        }
        rows
    }

    fn open(&mut self) -> Vec<Effect> {
        vec![Effect::LoadVoice]
    }

    fn event(&mut self, event: FormEvent) -> Vec<Effect> {
        let Some(current) = self.current().cloned().filter(|_| self.available) else { return Vec::new() };
        match event {
            FormEvent::Toggled { id, checked } if id == ENABLED => {
                if checked == current.enabled {
                    return Vec::new();
                }
                self.save(VoiceSettings { enabled: checked, ..current })
            }
            FormEvent::Chose { id, index } if id == CHOICE => {
                let voice = match index {
                    0 => None,
                    index => match self.listed().get(index - 1) {
                        Some(voice) => Some(voice.id.clone()),
                        None => return Vec::new(),
                    },
                };
                if voice == current.voice {
                    return Vec::new();
                }
                self.save(VoiceSettings { voice, ..current })
            }
            FormEvent::Pressed { id, .. } if id == LISTEN => {
                let voice = self.speaking().map(|voice| voice.id.clone());
                vec![Effect::SayAloud { voice, text: speakable(TEST_SENTENCE) }]
            }
            _ => Vec::new(),
        }
    }

    fn done(&mut self, outcome: Outcome) -> Vec<Effect> {
        match outcome {
            Outcome::VoiceLoaded { settings, voices } => {
                self.settings = Some(settings);
                self.voices = voices;
            }
            Outcome::VoiceSaved(Ok(settings)) => {
                let before = self.current().cloned();
                let text = if before.as_ref().is_some_and(|before| before.enabled != settings.enabled) {
                    if settings.enabled { VOICE_ON.to_owned() } else { VOICE_OFF.to_owned() }
                } else {
                    match pick(&self.voices, LANGUAGE, settings.voice.as_deref()) {
                        Some(voice) => format!("Sa voix : {}.", label(voice)),
                        None => format!("Aucune voix française n'est installée. {BETTER_VOICES}"),
                    }
                };
                self.saved = Some((Tone::Good, text));
                self.settings = Some(Ok(settings));
            }
            Outcome::VoiceSaved(Err(problem)) => self.saved = Some((Tone::Bad, problem)),
            _ => {}
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(id: &str, name: &str, language: &str, quality: u8) -> VoiceInfo {
        VoiceInfo {
            id: id.to_owned(),
            name: name.to_owned(),
            language: language.to_owned(),
            quality,
            female: true,
        }
    }

    fn voices() -> Vec<VoiceInfo> {
        vec![
            voice("voix.en.une", "Samantha", "en-US", 2),
            voice("voix.fr.une", "Amélie", "fr-CA", 1),
            voice("voix.fr.deux", "Audrey", "fr-FR", 3),
        ]
    }

    fn opened(settings: VoiceSettings, voices: Vec<VoiceInfo>) -> VoiceRubric {
        let mut rubric = VoiceRubric::new(&Environment { keychain: None, tls: true, voice: true });
        assert!(matches!(rubric.open().as_slice(), [Effect::LoadVoice]));
        rubric.done(Outcome::VoiceLoaded { settings: Ok(settings), voices });
        rubric
    }

    fn control(rubric: &VoiceRubric, id: &str) -> Option<Control> {
        rubric.rows().into_iter().find(|row| row.id == id).map(|row| row.control)
    }

    #[test]
    fn french_voices_are_offered_with_their_quality_the_best_first() {
        let rubric = opened(VoiceSettings::default(), voices());
        assert_eq!(
            control(&rubric, ENABLED),
            Some(Control::Check {
                title: "Dire ses réponses à voix haute".to_owned(),
                checked: false,
                enabled: true,
            })
        );
        assert_eq!(
            control(&rubric, CHOICE),
            Some(Control::Choice {
                options: vec![
                    "Automatique : la meilleure installée (Audrey (fr-FR, qualité supérieure))".to_owned(),
                    "Audrey (fr-FR, qualité supérieure)".to_owned(),
                    "Amélie (fr-CA, qualité de base)".to_owned(),
                ],
                selected: 0,
                enabled: true,
            })
        );
        assert_eq!(control(&rubric, NOTE), None, "une bonne voix est là : rien à conseiller");
    }

    #[test]
    fn switching_choosing_and_listening_follow_capri_voice() {
        let mut rubric = opened(VoiceSettings::default(), voices());
        let effects = rubric.event(FormEvent::Toggled { id: ENABLED.to_owned(), checked: true });
        let [Effect::SaveVoice(saved)] = effects.as_slice() else { panic!("{effects:?}") };
        assert_eq!(saved, &VoiceSettings { enabled: true, ..VoiceSettings::default() });
        rubric.done(Outcome::VoiceSaved(Ok(saved.clone())));
        assert_eq!(
            control(&rubric, SAVED),
            Some(Control::Note { text: VOICE_ON.to_owned(), tone: Tone::Good })
        );

        let effects = rubric.event(FormEvent::Chose { id: CHOICE.to_owned(), index: 2 });
        let [Effect::SaveVoice(saved)] = effects.as_slice() else { panic!("{effects:?}") };
        assert_eq!(
            saved,
            &VoiceSettings {
                enabled: true,
                voice: Some("voix.fr.une".to_owned()),
                ..VoiceSettings::default()
            }
        );
        rubric.done(Outcome::VoiceSaved(Ok(saved.clone())));
        assert!(matches!(control(&rubric, CHOICE), Some(Control::Choice { selected: 2, .. })));
        assert_eq!(
            control(&rubric, SAVED),
            Some(Control::Note {
                text: "Sa voix : Amélie (fr-CA, qualité de base).".to_owned(),
                tone: Tone::Good
            })
        );
        assert!(matches!(control(&rubric, NOTE), Some(Control::Note { text, .. }) if text == BETTER_VOICES));

        let effects = rubric.event(FormEvent::Pressed { id: LISTEN.to_owned(), secret: None });
        assert!(
            matches!(effects.as_slice(), [Effect::SayAloud { voice: Some(id), text }]
                if id == "voix.fr.une" && text == TEST_SENTENCE),
            "{effects:?}"
        );
        // revenir au choix automatique
        let effects = rubric.event(FormEvent::Chose { id: CHOICE.to_owned(), index: 0 });
        assert!(matches!(effects.as_slice(), [Effect::SaveVoice(VoiceSettings { voice: None, .. })]));
        // ne rien changer ne réécrit rien
        assert!(rubric.event(FormEvent::Toggled { id: ENABLED.to_owned(), checked: true }).is_empty());
    }

    #[test]
    fn a_voice_gone_since_or_no_french_voice_at_all_is_said() {
        let rubric = opened(
            VoiceSettings {
                enabled: true,
                voice: Some("voix.partie".to_owned()),
                ..VoiceSettings::default()
            },
            voices(),
        );
        assert!(matches!(control(&rubric, CHOICE), Some(Control::Choice { selected: 0, .. })));
        assert!(matches!(control(&rubric, NOTE), Some(Control::Note { text, .. })
            if text.starts_with("La voix choisie, « voix.partie », n'est plus installée")));

        let rubric = opened(VoiceSettings::default(), vec![voice("voix.en.une", "Samantha", "en-US", 2)]);
        assert!(matches!(control(&rubric, NOTE), Some(Control::Note { text, .. })
            if text.starts_with("Aucune voix française n'est installée.")));
        assert!(matches!(control(&rubric, CHOICE), Some(Control::Choice { options, .. })
            if options == ["Automatique : la voix du système"]));
    }

    #[test]
    fn without_a_voice_on_this_system_nothing_can_be_changed() {
        let mut rubric = VoiceRubric::new(&Environment { keychain: None, tls: false, voice: false });
        rubric.done(Outcome::VoiceLoaded { settings: Ok(VoiceSettings::default()), voices: Vec::new() });
        assert!(matches!(control(&rubric, ENABLED), Some(Control::Check { enabled: false, .. })));
        assert!(rubric.event(FormEvent::Toggled { id: ENABLED.to_owned(), checked: true }).is_empty());
        assert!(rubric.event(FormEvent::Pressed { id: LISTEN.to_owned(), secret: None }).is_empty());

        // des réglages illisibles se disent, et ne sont pas écrasés
        let mut rubric = VoiceRubric::new(&Environment { keychain: None, tls: true, voice: true });
        rubric
            .done(Outcome::VoiceLoaded {
                settings: Err("réglages illisibles".to_owned()), voices: voices()
            });
        assert!(matches!(control(&rubric, NOTE), Some(Control::Note { tone: Tone::Bad, .. })));
        assert!(rubric.event(FormEvent::Toggled { id: ENABLED.to_owned(), checked: true }).is_empty());
    }
}
