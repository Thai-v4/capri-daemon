//! Lui parler au micro : le déroulé d'une prise de parole, sans rien toucher au système.
//!
//! Le système écoute et transcrit au fil de l'eau (`capri_platform::Ear`) ; ce module décide de
//! ce que cela veut dire. On commence à parler, des transcriptions provisoires arrivent, on
//! s'arrête, et le système envoie un dernier texte, définitif, un court instant plus tard. S'il
//! ne vient pas, on se contente du dernier texte provisoire plutôt que d'attendre sans fin.

/// Après la fin de la prise de parole, temps laissé au système pour rendre son dernier texte.
pub const LAST_WORD_MS: u64 = 2_500;

/// Ce qu'une prise de parole a donné.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Said {
    /// Ce qui a été dit, à envoyer à Capri.
    Message(String),
    /// Rien d'intelligible.
    Nothing,
}

#[derive(Debug, Default)]
pub struct Dictation {
    state: State,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    /// On parle. `heard` : la transcription provisoire.
    Listening { heard: String },
    /// On a fini de parler : le dernier texte du système est attendu jusqu'à `until_ms`.
    Finishing { heard: String, until_ms: u64 },
}

impl Dictation {
    pub fn new() -> Self {
        Self::default()
    }

    /// L'écoute vient de commencer.
    pub fn begin(&mut self) {
        self.state = State::Listening { heard: String::new() };
    }

    /// Une prise de parole est-elle en cours, ou son dernier texte encore attendu ?
    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Est-on en train de parler (le micro est ouvert) ?
    pub fn is_listening(&self) -> bool {
        matches!(self.state, State::Listening { .. })
    }

    /// La transcription en cours, pour l'afficher ; vide tant que rien n'a été compris.
    pub fn heard(&self) -> Option<&str> {
        match &self.state {
            State::Idle => None,
            State::Listening { heard } | State::Finishing { heard, .. } => Some(heard),
        }
    }

    /// Le système a transcrit `text` (tout ce qui a été dit jusqu'ici). `last` : c'est son
    /// dernier mot, la prise de parole est finie, même si on ne l'avait pas encore arrêtée.
    pub fn transcribed(&mut self, text: &str, last: bool) -> Option<Said> {
        let (State::Listening { heard } | State::Finishing { heard, .. }) = &mut self.state else {
            return None; // un texte arrivé trop tard, après l'abandon
        };
        // un dernier texte vide ne vaut pas mieux que ce qu'on avait déjà compris
        if !text.trim().is_empty() || !last {
            text.trim().clone_into(heard);
        }
        last.then(|| self.conclude())
    }

    /// On a fini de parler : le micro est fermé, le dernier texte est attendu.
    pub fn end(&mut self, now_ms: u64) {
        if let State::Listening { heard } = &mut self.state {
            self.state = State::Finishing { heard: std::mem::take(heard), until_ms: now_ms + LAST_WORD_MS };
        }
    }

    /// L'écoute s'est arrêtée sur une erreur (rien entendu, micro perdu) : on garde ce qui avait
    /// été compris.
    pub fn failed(&mut self) -> Option<Said> {
        self.is_active().then(|| self.conclude())
    }

    /// À appeler régulièrement : conclut si le dernier texte se fait trop attendre.
    pub fn tick(&mut self, now_ms: u64) -> Option<Said> {
        matches!(self.state, State::Finishing { until_ms, .. } if now_ms >= until_ms).then(|| self.conclude())
    }

    /// Abandonne la prise de parole : rien ne sera envoyé.
    pub fn cancel(&mut self) {
        self.state = State::Idle;
    }

    fn conclude(&mut self) -> Said {
        match std::mem::take(&mut self.state) {
            State::Listening { heard } | State::Finishing { heard, .. } if !heard.is_empty() => {
                Said::Message(heard)
            }
            _ => Said::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(text: &str) -> Option<Said> {
        Some(Said::Message(text.to_owned()))
    }

    #[test]
    fn what_was_said_is_sent_once_the_system_gives_its_last_word() {
        let mut dictation = Dictation::new();
        assert!(!dictation.is_active() && dictation.heard().is_none());
        dictation.begin();
        assert!(dictation.is_listening());
        assert_eq!(dictation.heard(), Some(""), "elle écoute, rien n'est encore compris");
        assert_eq!(dictation.transcribed("Quelle", false), None);
        assert_eq!(dictation.transcribed(" Quelle heure est-il ", false), None);
        assert_eq!(dictation.heard(), Some("Quelle heure est-il"));

        dictation.end(1_000);
        assert!(dictation.is_active() && !dictation.is_listening(), "le micro est fermé, on attend la fin");
        assert_eq!(dictation.tick(1_500), None);
        assert_eq!(dictation.transcribed("Quelle heure est-il ?", true), message("Quelle heure est-il ?"));
        assert!(!dictation.is_active());
        assert_eq!(dictation.tick(99_999), None, "rien n'est envoyé deux fois");
        assert_eq!(dictation.transcribed("trop tard", true), None);
    }

    #[test]
    fn a_last_word_that_never_comes_is_not_waited_for_forever() {
        let mut dictation = Dictation::new();
        dictation.begin();
        dictation.transcribed("Bonjour Capri", false);
        dictation.end(10_000);
        assert_eq!(dictation.tick(10_000 + LAST_WORD_MS - 1), None);
        assert_eq!(dictation.tick(10_000 + LAST_WORD_MS), message("Bonjour Capri"));

        // même chose si l'écoute échoue après coup, ou si le dernier texte arrive vide
        dictation.begin();
        dictation.transcribed("Encore moi", false);
        dictation.end(0);
        assert_eq!(dictation.failed(), message("Encore moi"));
        dictation.begin();
        dictation.transcribed("Et là", false);
        assert_eq!(dictation.transcribed("", true), message("Et là"), "le système a conclu de lui-même");
    }

    #[test]
    fn silence_sends_nothing() {
        let mut dictation = Dictation::new();
        dictation.begin();
        dictation.end(0);
        assert_eq!(dictation.tick(LAST_WORD_MS), Some(Said::Nothing), "une touche effleurée");
        dictation.begin();
        assert_eq!(dictation.failed(), Some(Said::Nothing), "rien entendu");
        dictation.begin();
        dictation.transcribed("  ", false);
        assert_eq!(dictation.transcribed(" ", true), Some(Said::Nothing));
        assert_eq!(dictation.failed(), None, "rien n'était en cours");
    }

    #[test]
    fn a_correction_replaces_what_was_understood_and_a_cancel_drops_it() {
        let mut dictation = Dictation::new();
        dictation.begin();
        dictation.transcribed("Qu'elle heure", false);
        dictation.transcribed("Quelle heure", false);
        assert_eq!(dictation.heard(), Some("Quelle heure"), "le système se reprend en avançant");
        dictation.transcribed("", false);
        assert_eq!(dictation.heard(), Some(""), "et peut tout effacer");
        dictation.transcribed("Autre chose", false);
        dictation.cancel();
        assert!(!dictation.is_active());
        assert_eq!(dictation.tick(u64::MAX), None);
        dictation.end(0); // sans effet hors d'une écoute
        assert!(!dictation.is_active());
    }
}
