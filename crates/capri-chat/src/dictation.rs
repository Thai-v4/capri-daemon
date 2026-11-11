//! Lui parler au micro : le déroulé d'une prise de parole, sans rien toucher au système.
//!
//! Le système écoute et transcrit au fil de l'eau (`capri_platform::Ear`) ; ce module décide de
//! ce que cela veut dire. On commence à parler, des transcriptions provisoires arrivent, on
//! s'arrête, et le système envoie un dernier texte, définitif, un court instant plus tard. S'il
//! ne vient pas, on se contente du dernier texte provisoire plutôt que d'attendre sans fin.
//!
//! Qui décide qu'on s'arrête ? Deux gestes. **Maintenu** : la touche est enfoncée, c'est
//! l'utilisateur, qui la relâche. **Bref** : un appui qui ne dure pas (ou un clic du menu) ouvre une
//! *écoute libre*, que ferme soit un nouvel appui, soit la détection de fin de phrase : plus de
//! texte nouveau depuis un moment, rien du tout au bout d'un autre, ou la durée maximale. La
//! détection ne mesure pas le son : elle regarde quand le dernier texte provisoire a changé, sur
//! une horloge (des millisecondes) que l'appelant passe à chaque appel, donc testable sans micro
//! ni attente.
//!
//! Une écoute se ferme toujours, dans les deux gestes : relâchement, nouvel appui, fin de phrase
//! ou borne dure ([`MAX_LISTENING_MS`]).

/// Après la fin de la prise de parole, temps laissé au système pour rendre son dernier texte.
pub const LAST_WORD_MS: u64 = 2_500;

/// Un appui qui dure moins que cela est un geste bref : la touche, relâchée, laisse le micro
/// ouvert (écoute libre). Au-delà, c'est la touche maintenue, et le relâchement termine, comme
/// toujours.
///
/// Un appui voulu bref dure 100 à 200 ms ; quelqu'un qui maintient pour dire ne serait-ce que
/// « oui » garde la touche plus d'une demi-seconde, d'autant que le micro met un instant à
/// s'ouvrir (ce temps compte dans la durée de l'appui). Trop court : un appui un peu lent coupe la
/// parole ; trop long : un « oui » maintenu part en écoute libre.
pub const TAP_MS: u64 = 350;

/// En écoute libre, silence qui termine la phrase : le temps écoulé depuis que le dernier texte
/// provisoire a changé.
///
/// Le système transcrit avec un retard de quelques centaines de millisecondes : après 1,5 s sans
/// nouveau texte, l'utilisateur a réellement fini de parler depuis environ 1 à 1,2 s. Une pause
/// pour respirer dure moins d'une demi-seconde, une hésitation pour chercher un mot de 0,5 à 1 s :
/// 1,5 s les laisse passer sans trop faire attendre à la fin. À augmenter si elle coupe en plein
/// milieu d'une phrase, à diminuer si elle met trop longtemps à envoyer.
pub const SILENCE_MS: u64 = 1_500;

/// En écoute libre, temps au bout duquel, sans un seul mot compris, le micro se ferme et rien
/// n'est envoyé. Compté depuis l'ouverture du micro : le temps de lire la bulle « Je t'écoute… »
/// puis de commencer. À diminuer si un appui par erreur laisse le micro ouvert trop longtemps.
pub const NOTHING_HEARD_MS: u64 = 7_000;

/// Borne dure : une écoute ne dure jamais plus que cela, maintenue ou libre, et ce qui a été
/// compris est envoyé. Apple documente une limite d'une minute pour une tâche de reconnaissance
/// (`SFSpeechRecognizer.h` : « Plan for a one-minute limit on audio duration ») : le système la
/// coupe alors de lui-même, sans qu'on sache à l'avance ce qu'il rend (dernier texte ou erreur).
/// Cinq secondes de marge permettent de conclure proprement, avec le texte compris, avant lui.
pub const MAX_LISTENING_MS: u64 = 55_000;

/// Pourquoi une écoute doit être terminée sans que l'utilisateur l'ait demandé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndOfSpeech {
    /// Des mots ont été compris, puis plus rien de nouveau pendant [`SILENCE_MS`].
    Silence,
    /// Pas un mot au bout de [`NOTHING_HEARD_MS`].
    NothingHeard,
    /// [`MAX_LISTENING_MS`] atteint.
    TooLong,
}

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

/// Qui terminera l'écoute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// La touche est enfoncée : on parle tant qu'elle l'est, et son relâchement termine.
    Held,
    /// Écoute libre : un nouvel appui, ou la détection de fin de phrase, termine.
    Free,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    /// On parle. `heard` : la transcription provisoire ; `changed_ms` : l'heure où elle a changé
    /// pour la dernière fois.
    Listening { heard: String, mode: Mode, started_ms: u64, changed_ms: u64 },
    /// On a fini de parler : le dernier texte du système est attendu jusqu'à `until_ms`.
    Finishing { heard: String, until_ms: u64 },
}

impl Dictation {
    pub fn new() -> Self {
        Self::default()
    }

    /// L'écoute vient de commencer, la touche enfoncée : sa suite dépend de la durée de l'appui
    /// (voir [`Dictation::released`]).
    pub fn begin_held(&mut self, now_ms: u64) {
        self.begin(Mode::Held, now_ms);
    }

    /// L'écoute vient de commencer sans touche à maintenir (le menu) : écoute libre.
    pub fn begin_free(&mut self, now_ms: u64) {
        self.begin(Mode::Free, now_ms);
    }

    fn begin(&mut self, mode: Mode, now_ms: u64) {
        self.state = State::Listening { heard: String::new(), mode, started_ms: now_ms, changed_ms: now_ms };
    }

    /// La touche est relâchée. Rend `true` si l'écoute doit se terminer maintenant (appui long,
    /// comme toujours). Un appui bref ([`TAP_MS`]) la laisse ouverte, en écoute libre ; hors d'une
    /// écoute maintenue (déjà libre, déjà finie), rien ne change.
    pub fn released(&mut self, now_ms: u64) -> bool {
        let State::Listening { mode: mode @ Mode::Held, started_ms, .. } = &mut self.state else {
            return false;
        };
        if now_ms.saturating_sub(*started_ms) < TAP_MS {
            *mode = Mode::Free;
            false
        } else {
            true
        }
    }

    /// L'écoute est-elle libre : un nouvel appui sur la touche la termine-t-il ?
    pub fn is_free(&self) -> bool {
        matches!(self.state, State::Listening { mode: Mode::Free, .. })
    }

    /// À appeler régulièrement pendant l'écoute : faut-il la terminer maintenant, sans que
    /// l'utilisateur l'ait demandé ? La durée maximale vaut pour les deux gestes ; le silence et
    /// l'attente d'un premier mot seulement pour l'écoute libre (la touche maintenue, c'est
    /// l'utilisateur qui décide). À celui qui l'appelle d'arrêter le micro puis d'appeler
    /// [`Dictation::end`].
    pub fn due(&self, now_ms: u64) -> Option<EndOfSpeech> {
        let State::Listening { heard, mode, started_ms, changed_ms } = &self.state else { return None };
        if now_ms.saturating_sub(*started_ms) >= MAX_LISTENING_MS {
            return Some(EndOfSpeech::TooLong);
        }
        match (mode, heard.is_empty()) {
            (Mode::Held, _) => None,
            (Mode::Free, false) => {
                (now_ms.saturating_sub(*changed_ms) >= SILENCE_MS).then_some(EndOfSpeech::Silence)
            }
            (Mode::Free, true) => {
                (now_ms.saturating_sub(*started_ms) >= NOTHING_HEARD_MS).then_some(EndOfSpeech::NothingHeard)
            }
        }
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
            State::Listening { heard, .. } | State::Finishing { heard, .. } => Some(heard),
        }
    }

    /// Le système a transcrit `text` (tout ce qui a été dit jusqu'ici), à l'heure `now_ms`.
    /// `last` : c'est son dernier mot, la prise de parole est finie, même si on ne l'avait pas
    /// encore arrêtée.
    ///
    /// Seul un texte **différent** de celui qu'on avait compte comme nouveau pour la détection du
    /// silence : le système peut très bien répéter le même.
    pub fn transcribed(&mut self, text: &str, last: bool, now_ms: u64) -> Option<Said> {
        let text = text.trim();
        match &mut self.state {
            State::Idle => return None, // un texte arrivé trop tard, après l'abandon
            State::Listening { heard, changed_ms, .. } => {
                // un dernier texte vide ne vaut pas mieux que ce qu'on avait déjà compris
                if (!text.is_empty() || !last) && heard != text {
                    text.clone_into(heard);
                    *changed_ms = now_ms;
                }
            }
            State::Finishing { heard, .. } => {
                if !text.is_empty() || !last {
                    text.clone_into(heard);
                }
            }
        }
        last.then(|| self.conclude())
    }

    /// On a fini de parler : le micro est fermé, le dernier texte est attendu.
    pub fn end(&mut self, now_ms: u64) {
        if let State::Listening { heard, .. } = &mut self.state {
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
            State::Listening { heard, .. } | State::Finishing { heard, .. } if !heard.is_empty() => {
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
        dictation.begin_held(0);
        assert!(dictation.is_listening());
        assert_eq!(dictation.heard(), Some(""), "elle écoute, rien n'est encore compris");
        assert_eq!(dictation.transcribed("Quelle", false, 100), None);
        assert_eq!(dictation.transcribed(" Quelle heure est-il ", false, 200), None);
        assert_eq!(dictation.heard(), Some("Quelle heure est-il"));

        dictation.end(1_000);
        assert!(dictation.is_active() && !dictation.is_listening(), "le micro est fermé, on attend la fin");
        assert_eq!(dictation.tick(1_500), None);
        assert_eq!(
            dictation.transcribed("Quelle heure est-il ?", true, 1_600),
            message("Quelle heure est-il ?")
        );
        assert!(!dictation.is_active());
        assert_eq!(dictation.tick(99_999), None, "rien n'est envoyé deux fois");
        assert_eq!(dictation.transcribed("trop tard", true, 100_000), None);
    }

    #[test]
    fn a_last_word_that_never_comes_is_not_waited_for_forever() {
        let mut dictation = Dictation::new();
        dictation.begin_held(0);
        dictation.transcribed("Bonjour Capri", false, 100);
        dictation.end(10_000);
        assert_eq!(dictation.tick(10_000 + LAST_WORD_MS - 1), None);
        assert_eq!(dictation.tick(10_000 + LAST_WORD_MS), message("Bonjour Capri"));

        // même chose si l'écoute échoue après coup, ou si le dernier texte arrive vide
        dictation.begin_held(0);
        dictation.transcribed("Encore moi", false, 100);
        dictation.end(0);
        assert_eq!(dictation.failed(), message("Encore moi"));
        dictation.begin_held(0);
        dictation.transcribed("Et là", false, 100);
        assert_eq!(dictation.transcribed("", true, 200), message("Et là"), "le système a conclu de lui-même");
    }

    #[test]
    fn silence_sends_nothing() {
        let mut dictation = Dictation::new();
        dictation.begin_held(0);
        dictation.end(0);
        assert_eq!(dictation.tick(LAST_WORD_MS), Some(Said::Nothing), "une touche effleurée");
        dictation.begin_held(0);
        assert_eq!(dictation.failed(), Some(Said::Nothing), "rien entendu");
        dictation.begin_held(0);
        dictation.transcribed("  ", false, 100);
        assert_eq!(dictation.transcribed(" ", true, 200), Some(Said::Nothing));
        assert_eq!(dictation.failed(), None, "rien n'était en cours");
    }

    #[test]
    fn a_correction_replaces_what_was_understood_and_a_cancel_drops_it() {
        let mut dictation = Dictation::new();
        dictation.begin_held(0);
        dictation.transcribed("Qu'elle heure", false, 100);
        dictation.transcribed("Quelle heure", false, 200);
        assert_eq!(dictation.heard(), Some("Quelle heure"), "le système se reprend en avançant");
        dictation.transcribed("", false, 300);
        assert_eq!(dictation.heard(), Some(""), "et peut tout effacer");
        dictation.transcribed("Autre chose", false, 400);
        dictation.cancel();
        assert!(!dictation.is_active());
        assert_eq!(dictation.tick(u64::MAX), None);
        dictation.end(0); // sans effet hors d'une écoute
        assert!(!dictation.is_active());
    }

    // ---- le geste bref et la détection de fin de phrase, sur une horloge factice ----

    /// Une écoute libre où un texte est arrivé à `at` ms.
    fn free_hearing(text: &str, at: u64) -> Dictation {
        let mut dictation = Dictation::new();
        dictation.begin_free(0);
        dictation.transcribed(text, false, at);
        dictation
    }

    #[test]
    fn a_short_press_keeps_the_microphone_open_and_a_long_one_ends_it() {
        let mut brief = Dictation::new();
        brief.begin_held(1_000);
        assert!(!brief.is_free(), "tant que la touche est enfoncée, c'est le relâchement qui décide");
        assert!(!brief.released(1_000 + TAP_MS - 1), "appui bref : le micro reste ouvert");
        assert!(brief.is_listening() && brief.is_free());
        assert!(!brief.released(5_000), "un second relâchement n'est pas un geste");
        assert!(brief.is_free());

        let mut held = Dictation::new();
        held.begin_held(1_000);
        assert!(held.released(1_000 + TAP_MS), "appui maintenu : le relâchement termine, comme avant");
        assert!(!held.is_free(), "elle n'est pas passée en écoute libre");

        let mut idle = Dictation::new();
        assert!(!idle.released(0), "rien à terminer hors d'une écoute");
        idle.begin_held(0);
        idle.end(0);
        assert!(!idle.released(10_000), "ni quand le micro est déjà fermé");
    }

    #[test]
    fn a_held_press_is_never_cut_short_by_silence() {
        let mut dictation = Dictation::new();
        dictation.begin_held(0);
        assert_eq!(
            dictation.due(NOTHING_HEARD_MS + 1_000),
            None,
            "personne n'a encore parlé, il tient la touche"
        );
        dictation.transcribed("Bonjour", false, 1_000);
        assert_eq!(dictation.due(1_000 + SILENCE_MS * 4), None, "il réfléchit, la touche est enfoncée");
    }

    #[test]
    fn free_listening_ends_after_a_silence_that_follows_words() {
        let mut dictation = free_hearing("Quelle heure", 2_000);
        assert_eq!(dictation.due(2_000 + SILENCE_MS - 1), None);
        // chaque texte nouveau repousse la fin : elle ne coupe pas quelqu'un qui continue
        dictation.transcribed("Quelle heure est-il", false, 3_000);
        assert_eq!(dictation.due(2_000 + SILENCE_MS), None);
        assert_eq!(dictation.due(3_000 + SILENCE_MS - 1), None);
        assert_eq!(dictation.due(3_000 + SILENCE_MS), Some(EndOfSpeech::Silence));

        // l'appelant ferme le micro, puis laisse au système le temps de rendre son dernier texte
        dictation.end(3_000 + SILENCE_MS);
        assert_eq!(dictation.due(u64::MAX), None, "plus d'écoute, plus rien à terminer");
        assert_eq!(
            dictation.transcribed("Quelle heure est-il ?", true, 5_000),
            message("Quelle heure est-il ?")
        );
    }

    #[test]
    fn the_same_text_repeated_does_not_count_as_something_new() {
        let mut dictation = free_hearing("Bonjour", 1_000);
        dictation.transcribed("Bonjour", false, 2_000);
        dictation.transcribed(" Bonjour ", false, 2_200);
        assert_eq!(dictation.due(1_000 + SILENCE_MS), Some(EndOfSpeech::Silence));
    }

    #[test]
    fn a_short_press_then_silence_ends_on_its_own() {
        // le scénario complet : appui bref, on parle, plus rien
        let mut dictation = Dictation::new();
        dictation.begin_held(0);
        assert!(!dictation.released(150));
        assert_eq!(dictation.due(3_000), None, "rien de compris, c'est encore tôt");
        dictation.transcribed("Dis-moi", false, 3_000);
        dictation.transcribed("Dis-moi bonjour", false, 3_800);
        assert_eq!(dictation.due(3_800 + SILENCE_MS), Some(EndOfSpeech::Silence));
    }

    #[test]
    fn free_listening_without_a_word_closes_and_sends_nothing() {
        let mut dictation = Dictation::new();
        dictation.begin_free(10_000);
        assert_eq!(dictation.due(10_000 + NOTHING_HEARD_MS - 1), None);
        assert_eq!(dictation.due(10_000 + NOTHING_HEARD_MS), Some(EndOfSpeech::NothingHeard));
        dictation.transcribed("   ", false, 12_000);
        assert_eq!(
            dictation.due(10_000 + NOTHING_HEARD_MS),
            Some(EndOfSpeech::NothingHeard),
            "des blancs ne sont pas un mot"
        );

        dictation.end(10_000 + NOTHING_HEARD_MS);
        assert_eq!(dictation.failed(), Some(Said::Nothing), "rien n'est envoyé");
    }

    #[test]
    fn a_word_that_comes_late_but_in_time_is_kept() {
        let mut dictation = Dictation::new();
        dictation.begin_free(0);
        dictation.transcribed("Oui", false, NOTHING_HEARD_MS - 100);
        assert_eq!(
            dictation.due(NOTHING_HEARD_MS),
            None,
            "un mot est arrivé : c'est le silence qui compte, depuis lui"
        );
        assert_eq!(dictation.due(NOTHING_HEARD_MS - 100 + SILENCE_MS), Some(EndOfSpeech::Silence));
    }

    #[test]
    fn a_retracted_text_goes_back_to_waiting_for_a_first_word() {
        let mut dictation = free_hearing("Euh", 1_000);
        dictation.transcribed("", false, 2_000);
        assert_eq!(dictation.due(1_000 + SILENCE_MS), None, "plus rien de compris : on n'envoie pas du vide");
        assert_eq!(dictation.due(NOTHING_HEARD_MS), Some(EndOfSpeech::NothingHeard));
    }

    #[test]
    fn no_listening_lasts_longer_than_the_hard_limit_whatever_the_gesture() {
        // l'écoute libre, qui n'arrête pas de recevoir du texte : le silence ne vient jamais
        let mut free = Dictation::new();
        free.begin_free(1_000);
        for step in 1..=60 {
            free.transcribed(&format!("mot {step}"), false, 1_000 + step * 900);
        }
        assert_eq!(free.due(1_000 + MAX_LISTENING_MS - 1), None);
        assert_eq!(free.due(1_000 + MAX_LISTENING_MS), Some(EndOfSpeech::TooLong));
        free.end(1_000 + MAX_LISTENING_MS);
        assert_eq!(free.failed(), message("mot 60"), "ce qui a été compris est envoyé");

        // la touche maintenue aussi (le système, lui, coupe à une minute)
        let mut held = Dictation::new();
        held.begin_held(0);
        held.transcribed("Un très long discours", false, 50_000);
        assert_eq!(held.due(MAX_LISTENING_MS - 1), None);
        assert_eq!(held.due(MAX_LISTENING_MS), Some(EndOfSpeech::TooLong));

        // et la borne tient sous la minute documentée par Apple, avec de la marge
        const { assert!(MAX_LISTENING_MS + 2_000 < 60_000) };
    }

    #[test]
    fn the_thresholds_are_in_a_sensible_order() {
        const { assert!(TAP_MS < SILENCE_MS) }; // un appui bref est plus court qu'un silence de fin
        const { assert!(SILENCE_MS < NOTHING_HEARD_MS && NOTHING_HEARD_MS < MAX_LISTENING_MS) };
    }

    #[test]
    fn a_clock_that_goes_backwards_does_not_panic() {
        let dictation = free_hearing("Salut", 5_000);
        assert_eq!(dictation.due(0), None);
    }
}
