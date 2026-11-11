//! La rubrique « Voix » : dire ses réponses à voix haute ou non, avec quelle voix, quel effet,
//! à quelle hauteur et quel débit, et l'écouter.
//!
//! Les mêmes règles que `capri voice` : les voix sont celles du système, la française de
//! meilleure qualité par défaut ([`pick`]), les effets sont les préréglages de
//! [`crate::voice::VoiceEffect`], et tout est écrit dans `voice.json`. Rien n'est téléchargé ni
//! envoyé. Un changement vaut dès sa phrase suivante.

use capri_core::form::{Button, Control, FormEvent, Row, Tone};

use super::{Effect, Environment, Outcome, Rubric};
use crate::voice::{
    BETTER_VOICES, Intensity, LANGUAGE, Pitch, Rate, TEST_SENTENCE, VOICE_OFF, VOICE_ON, VoiceEffect,
    VoiceInfo, VoiceSettings, delivery, describe_style, french, pick, quality, speakable,
};

pub const ID: &str = "voice";

pub const ENABLED: &str = "voice.enabled";
pub const CHOICE: &str = "voice.choice";
pub const NOTE: &str = "voice.note";
pub const EFFECT: &str = "voice.effect";
pub const INTENSITY: &str = "voice.intensity";
pub const PITCH: &str = "voice.pitch";
pub const RATE: &str = "voice.rate";
pub const EFFECT_NOTE: &str = "voice.effect.note";
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

    /// L'effet, son intensité (seulement s'il y a un effet), la hauteur et le débit.
    fn style_rows(&self, usable: bool) -> Vec<Row> {
        let settings = self.current().cloned().unwrap_or_default();
        let choice = |options: Vec<String>, selected: usize, enabled: bool| Control::Choice {
            options,
            selected,
            enabled,
        };
        let position = |found: Option<usize>| found.unwrap_or_default();
        vec![
            Row::new(
                EFFECT,
                "Effet",
                choice(
                    VoiceEffect::ALL
                        .iter()
                        .map(|effect| format!("{} : {}", effect.name(), effect.summary()))
                        .collect(),
                    position(VoiceEffect::ALL.iter().position(|&effect| effect == settings.effect)),
                    usable,
                ),
            ),
            Row::new(
                INTENSITY,
                "Intensité",
                choice(
                    Intensity::ALL.iter().map(|level| capitalized(level.name())).collect(),
                    position(Intensity::ALL.iter().position(|&level| level == settings.intensity)),
                    usable && settings.effect != VoiceEffect::Natural,
                ),
            ),
            Row::new(
                PITCH,
                "Hauteur",
                choice(
                    Pitch::ALL.iter().map(|level| capitalized(level.name())).collect(),
                    position(Pitch::ALL.iter().position(|&level| level == settings.pitch)),
                    usable,
                ),
            ),
            Row::new(
                RATE,
                "Débit",
                choice(
                    Rate::ALL.iter().map(|level| capitalized(level.name())).collect(),
                    position(Rate::ALL.iter().position(|&level| level == settings.rate)),
                    usable,
                ),
            ),
            Row::new(EFFECT_NOTE, "", Control::Note { text: EFFECTS_ARE.to_owned(), tone: Tone::Quiet }),
        ]
    }
}

/// Ce que sont les effets, et ce qu'ils ne sont pas.
pub const EFFECTS_ARE: &str = "Les effets traitent le son sur cette machine, après la synthèse : ils font entendre une \
                               construct, sans imiter la voix de la série. Si le système ne peut pas les appliquer, \
                               Capri parle sans effet.";

/// « Légère » pour « légère ».
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
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
        rows.extend(self.style_rows(usable));
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
            FormEvent::Chose { id, index } if id == EFFECT => match VoiceEffect::ALL.get(index) {
                Some(&effect) if effect != current.effect => self.save(VoiceSettings { effect, ..current }),
                _ => Vec::new(),
            },
            FormEvent::Chose { id, index } if id == INTENSITY => match Intensity::ALL.get(index) {
                Some(&intensity)
                    if intensity != current.intensity && current.effect != VoiceEffect::Natural =>
                {
                    self.save(VoiceSettings { intensity, ..current })
                }
                _ => Vec::new(),
            },
            FormEvent::Chose { id, index } if id == PITCH => match Pitch::ALL.get(index) {
                Some(&pitch) if pitch != current.pitch => self.save(VoiceSettings { pitch, ..current }),
                _ => Vec::new(),
            },
            FormEvent::Chose { id, index } if id == RATE => match Rate::ALL.get(index) {
                Some(&rate) if rate != current.rate => self.save(VoiceSettings { rate, ..current }),
                _ => Vec::new(),
            },
            FormEvent::Pressed { id, .. } if id == LISTEN => {
                let voice = self.speaking().map(|voice| voice.id.clone());
                vec![Effect::SayAloud { delivery: delivery(&current, voice), text: speakable(TEST_SENTENCE) }]
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
                } else if before.as_ref().is_some_and(|before| before.voice == settings.voice) {
                    format!("Sa façon de parler : {}. Dès sa prochaine phrase.", describe_style(&settings))
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
            matches!(effects.as_slice(), [Effect::SayAloud { delivery, text }]
                if delivery.voice.as_deref() == Some("voix.fr.une") && text == TEST_SENTENCE),
            "{effects:?}"
        );
        // revenir au choix automatique
        let effects = rubric.event(FormEvent::Chose { id: CHOICE.to_owned(), index: 0 });
        assert!(matches!(effects.as_slice(), [Effect::SaveVoice(VoiceSettings { voice: None, .. })]));
        // ne rien changer ne réécrit rien
        assert!(rubric.event(FormEvent::Toggled { id: ENABLED.to_owned(), checked: true }).is_empty());
    }

    #[test]
    fn the_effect_its_intensity_the_pitch_and_the_rate_are_offered_natural_and_normal_first() {
        let rubric = opened(VoiceSettings::default(), voices());
        let Some(Control::Choice { options, selected: 0, enabled: true }) = control(&rubric, EFFECT) else {
            panic!("{:?}", control(&rubric, EFFECT))
        };
        assert_eq!(options.len(), VoiceEffect::ALL.len());
        assert_eq!(options[0], "Naturelle : sans effet, la voix telle que le système la dit");
        assert!(options[1].starts_with("Construct : "), "{options:?}");
        assert_eq!(
            control(&rubric, INTENSITY),
            Some(Control::Choice {
                options: vec!["Légère".to_owned(), "Moyenne".to_owned(), "Forte".to_owned()],
                selected: 1,
                enabled: false,
            }),
            "sans effet, pas d'intensité à régler"
        );
        assert_eq!(
            control(&rubric, PITCH),
            Some(Control::Choice {
                options: ["Plus grave", "Un peu grave", "Normale", "Un peu aiguë", "Plus aiguë"]
                    .map(str::to_owned)
                    .to_vec(),
                selected: 2,
                enabled: true,
            })
        );
        assert!(matches!(control(&rubric, RATE), Some(Control::Choice { selected: 2, enabled: true, .. })));
        assert_eq!(
            control(&rubric, EFFECT_NOTE),
            Some(Control::Note { text: EFFECTS_ARE.to_owned(), tone: Tone::Quiet })
        );
        assert!(!EFFECTS_ARE.contains("capri "), "pas de commande de terminal dans la fenêtre");
    }

    #[test]
    fn choosing_an_effect_its_intensity_the_pitch_and_the_rate_writes_them_and_listening_uses_them() {
        let mut rubric = opened(VoiceSettings { enabled: true, ..VoiceSettings::default() }, voices());
        let choose = |rubric: &mut VoiceRubric, id: &str, index: usize| -> VoiceSettings {
            let effects = rubric.event(FormEvent::Chose { id: id.to_owned(), index });
            let [Effect::SaveVoice(saved)] = effects.as_slice() else { panic!("{id} : {effects:?}") };
            rubric.done(Outcome::VoiceSaved(Ok(saved.clone())));
            saved.clone()
        };
        let saved = choose(&mut rubric, EFFECT, 2);
        assert_eq!(saved.effect, VoiceEffect::Radio);
        assert!(matches!(control(&rubric, INTENSITY), Some(Control::Choice { enabled: true, .. })));
        assert_eq!(
            control(&rubric, SAVED),
            Some(Control::Note {
                text: "Sa façon de parler : Liaison radio (intensité moyenne), hauteur normale, débit normal. Dès sa \
                       prochaine phrase."
                    .to_owned(),
                tone: Tone::Good
            })
        );
        assert_eq!(choose(&mut rubric, INTENSITY, 2).intensity, Intensity::Strong);
        assert_eq!(choose(&mut rubric, PITCH, 0).pitch, Pitch::Lower);
        let saved = choose(&mut rubric, RATE, 4);
        assert_eq!(
            saved,
            VoiceSettings {
                enabled: true,
                voice: None,
                effect: VoiceEffect::Radio,
                intensity: Intensity::Strong,
                pitch: Pitch::Lower,
                rate: Rate::Faster,
            },
            "chaque choix garde les autres"
        );
        // « Écouter » fait entendre ce qui est choisi
        let effects = rubric.event(FormEvent::Pressed { id: LISTEN.to_owned(), secret: None });
        let [Effect::SayAloud { delivery: heard, .. }] = effects.as_slice() else { panic!("{effects:?}") };
        assert_eq!(heard, &delivery(&saved, Some("voix.fr.deux".to_owned())));
        assert!(!heard.effects.is_empty() && heard.pitch < 1.0 && heard.rate > 0.0);
        // ne rien changer ne réécrit rien, une option qui n'existe pas non plus
        assert!(rubric.event(FormEvent::Chose { id: EFFECT.to_owned(), index: 2 }).is_empty());
        assert!(rubric.event(FormEvent::Chose { id: PITCH.to_owned(), index: 9 }).is_empty());
        // revenir à « Naturelle » : l'intensité n'est plus à régler, et n'est pas changée
        assert_eq!(choose(&mut rubric, EFFECT, 0).effect, VoiceEffect::Natural);
        assert!(matches!(
            control(&rubric, INTENSITY),
            Some(Control::Choice { enabled: false, selected: 2, .. })
        ));
        assert!(rubric.event(FormEvent::Chose { id: INTENSITY.to_owned(), index: 0 }).is_empty());
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
        for id in [EFFECT, INTENSITY, PITCH, RATE] {
            assert!(matches!(control(&rubric, id), Some(Control::Choice { enabled: false, .. })), "{id}");
            assert!(rubric.event(FormEvent::Chose { id: id.to_owned(), index: 1 }).is_empty(), "{id}");
        }
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
