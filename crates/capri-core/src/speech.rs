//! Ce que Capri dit : un texte qui arrive par morceaux, montré page par page.
//!
//! Une réponse peut être plus longue que sa bulle. Elle est donc découpée en pages, et chaque
//! page reste à l'écran le temps de la lire avant de céder la place à la suivante, un peu comme
//! des sous-titres. La lecture avance à son rythme, indépendamment de la vitesse à laquelle le
//! texte arrive : une réponse écrite très vite ne défile pas plus vite pour autant.

/// Nombre de caractères qu'une page peut porter.
pub const PAGE_CHARS: usize = 220;
/// Ce qui est montré tant qu'aucun mot n'est arrivé : elle réfléchit.
pub const THINKING: &str = "…";

/// Temps de lecture d'une page, en millisecondes.
const MIN_READ_MS: u64 = 1_800;
const READ_MS_PER_CHAR: u64 = 55;
const MAX_READ_MS: u64 = 9_000;
/// Une page complète reste encore un instant après son dernier mot.
const LINGER_MS: u64 = 700;

/// La page à montrer maintenant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechView<'a> {
    pub text: &'a str,
    /// D'autres pages suivent celle-ci.
    pub more: bool,
}

/// Une prise de parole en cours.
#[derive(Clone, Debug)]
pub struct Speech {
    text: String,
    finished: bool,
    /// Page courante, et depuis quand elle est à l'écran.
    page: usize,
    page_since_ms: u64,
    /// Depuis quand la page courante est complète (plus aucun mot ne s'y ajoutera).
    complete_since_ms: Option<u64>,
}

impl Speech {
    /// Une prise de parole qui commence à l'instant `now_ms`, encore sans texte.
    pub fn new(now_ms: u64) -> Self {
        Self { text: String::new(), finished: false, page: 0, page_since_ms: now_ms, complete_since_ms: None }
    }

    /// Ajoute un morceau de texte au bout.
    pub fn push(&mut self, delta: &str) {
        if !self.finished {
            self.text.push_str(delta);
        }
    }

    /// Plus rien n'arrivera après l'instant `now_ms` : la dernière page pourra s'effacer une fois
    /// lue.
    pub fn finish(&mut self, now_ms: u64) {
        if self.finished {
            return;
        }
        self.finished = true;
        // si c'est la dernière page qui est à l'écran, elle est complète dès maintenant, et non
        // au moment où quelqu'un la regardera
        if self.page + 1 >= paginate(&self.text, PAGE_CHARS).len() {
            self.complete_since_ms.get_or_insert(now_ms);
        }
    }

    /// Tout le texte arrivé jusqu'ici.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// La page à montrer à l'instant `now_ms`. Fait avancer la lecture ; `None` quand tout a été
    /// dit et lu.
    pub fn view(&mut self, now_ms: u64) -> Option<SpeechView<'_>> {
        loop {
            let pages = paginate(&self.text, PAGE_CHARS);
            let Some(last) = pages.len().checked_sub(1) else {
                // rien à lire : elle réfléchit encore, ou elle n'avait rien à dire
                return (!self.finished).then_some(SpeechView { text: THINKING, more: false });
            };
            let page = self.page.min(last);
            if page < last || self.finished {
                let complete_since = *self.complete_since_ms.get_or_insert(now_ms);
                let read_at = self.page_since_ms + reading_time(pages[page]);
                if now_ms >= read_at.max(complete_since + LINGER_MS) {
                    if page == last {
                        return None;
                    }
                    self.page = page + 1;
                    self.page_since_ms = now_ms;
                    self.complete_since_ms = None;
                    continue;
                }
            }
            self.page = page;
            return Some(SpeechView { text: pages[page], more: page < last });
        }
    }
}

fn reading_time(page: &str) -> u64 {
    (page.chars().count() as u64 * READ_MS_PER_CHAR).clamp(MIN_READ_MS, MAX_READ_MS)
}

/// Découpe `text` en pages d'au plus `max_chars` caractères, de préférence à la fin d'une phrase,
/// sinon entre deux mots, et en dernier recours au milieu d'un mot trop long. Ajouter du texte au
/// bout ne change que la dernière page.
pub fn paginate(text: &str, max_chars: usize) -> Vec<&str> {
    let max_chars = max_chars.max(1);
    let mut pages = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        // l'octet où commence le premier caractère qui ne tient plus sur la page
        let Some((limit, _)) = rest.char_indices().nth(max_chars) else {
            pages.push(rest);
            break;
        };
        let head = &rest[..limit];
        let cut = sentence_end(head, max_chars / 2)
            .or_else(|| head.rfind(char::is_whitespace).filter(|&at| at > 0))
            .unwrap_or(limit);
        pages.push(rest[..cut].trim_end());
        rest = rest[cut..].trim_start();
    }
    pages
}

/// L'octet qui suit la dernière fin de phrase de `head`, si elle tombe après `min_chars`
/// caractères : couper plus tôt gâcherait la page.
fn sentence_end(head: &str, min_chars: usize) -> Option<usize> {
    let mut found = None;
    let mut chars = head.char_indices().enumerate().peekable();
    while let Some((count, (at, c))) = chars.next() {
        let ends_sentence = matches!(c, '.' | '!' | '?' | '…');
        let before_space = chars.peek().is_some_and(|(_, (_, next))| next.is_whitespace());
        if ends_sentence && before_space && count >= min_chars {
            found = Some(at + c.len_utf8());
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_text_is_one_page() {
        assert_eq!(paginate("  Salut !  ", 220), ["Salut !"]);
        assert!(paginate("", 220).is_empty());
        assert!(paginate(" \n ", 220).is_empty());
    }

    #[test]
    fn pages_break_at_sentence_ends_when_one_falls_late_enough() {
        let text = "Première phrase assez longue. Deuxième phrase qui dépasse la page.";
        assert_eq!(
            paginate(text, 40),
            ["Première phrase assez longue.", "Deuxième phrase qui dépasse la page."]
        );
        // une fin de phrase trop tôt dans la page ne compte pas : on coupe entre deux mots
        assert_eq!(
            paginate("Oui. Et ensuite une longue suite de mots sans point", 30),
            ["Oui. Et ensuite une longue", "suite de mots sans point"]
        );
    }

    #[test]
    fn a_word_longer_than_a_page_is_cut_and_nothing_is_lost() {
        let pages = paginate("abcdefghij klm", 4);
        assert_eq!(pages, ["abcd", "efgh", "ij", "klm"]);
        let long = "é".repeat(50);
        assert_eq!(paginate(&long, 20).concat(), long, "coupé entre deux caractères, jamais au milieu d'un");
    }

    #[test]
    fn appending_text_only_ever_changes_the_last_page() {
        let full =
            "Un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze quinze seize.";
        let mut previous: Vec<String> = Vec::new();
        for end in (0..=full.len()).filter(|&i| full.is_char_boundary(i)) {
            let pages: Vec<String> = paginate(&full[..end], 24).into_iter().map(str::to_owned).collect();
            if previous.len() > 1 {
                let stable = previous.len() - 1;
                assert_eq!(pages[..stable], previous[..stable], "après {:?}", &full[..end]);
            }
            previous = pages;
        }
        assert!(previous.len() > 3);
    }

    #[test]
    fn she_thinks_until_the_first_word_then_speaks() {
        let mut speech = Speech::new(0);
        assert_eq!(speech.view(500), Some(SpeechView { text: "…", more: false }));
        speech.push("Bon");
        speech.push("jour !");
        assert_eq!(speech.view(600).map(|v| v.text), Some("Bonjour !"));
        assert_eq!(speech.text(), "Bonjour !");
    }

    #[test]
    fn the_last_page_stays_long_enough_to_be_read_then_goes() {
        let mut speech = Speech::new(0);
        speech.push("Bonjour !");
        assert!(speech.view(60_000).is_some(), "tant que la réponse n'est pas finie, elle reste");
        speech.finish(60_000);
        assert!(speech.view(60_000).is_some());
        assert!(speech.view(60_000 + LINGER_MS - 1).is_some(), "un instant de plus après le dernier mot");
        assert_eq!(speech.view(60_000 + LINGER_MS), None);

        // une réponse courte et immédiate reste au moins le temps minimal de lecture
        // (sans qu'il faille l'avoir regardée entre-temps pour que le compte commence)
        let mut quick = Speech::new(1_000);
        quick.push("Oui.");
        quick.finish(1_000);
        assert!(quick.view(1_000 + MIN_READ_MS - 1).is_some());
        assert_eq!(quick.view(1_000 + MIN_READ_MS), None);
    }

    #[test]
    fn pages_follow_one_another_at_reading_pace_not_at_writing_pace() {
        let text: String = (1..=10)
            .map(|i| format!("Voici la phrase numéro {i}, d'une cinquantaine de caractères. "))
            .collect();
        let mut speech = Speech::new(0);
        speech.push(&text); // tout arrive d'un coup : plusieurs pages
        speech.finish(0);
        let first = speech.view(0).unwrap();
        assert!(first.more && first.text.chars().count() <= PAGE_CHARS);
        let first_text = first.text.to_owned();

        assert_eq!(speech.view(1_000).unwrap().text, first_text, "pas encore lue");
        let read = reading_time(&first_text);
        assert_eq!(speech.view(read - 1).unwrap().text, first_text);
        let second = speech.view(read).unwrap().text.to_owned();
        assert_ne!(second, first_text, "page suivante");

        // jusqu'à la dernière, puis plus rien
        let mut now = read;
        let mut pages = vec![first_text, second];
        while let Some(view) = speech.view(now) {
            if view.text != pages.last().unwrap() {
                pages.push(view.text.to_owned());
            }
            now += 100;
        }
        assert!(pages.len() >= 3, "{} pages", pages.len());
        assert_eq!(pages.join(" "), text.trim(), "tout a été montré, dans l'ordre");
        assert!(now < 60_000);
    }

    #[test]
    fn nothing_to_say_shows_nothing() {
        let mut speech = Speech::new(0);
        speech.finish(5);
        assert_eq!(speech.view(10), None);
        // et ce qui arrive après la fin est ignoré
        speech.push("trop tard");
        assert_eq!(speech.text(), "");
    }
}
