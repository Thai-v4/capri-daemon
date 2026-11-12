//! Du texte brut à partir de ce qu'écrit un modèle.
//!
//! Sa personnalité demande à Capri de ne pas écrire de Markdown, mais un modèle l'oublie souvent :
//! `*insistance*`, `**gras**`, `` `code` ``, titres en `#`, blocs entre ```` ``` ````. Dans une
//! bulle comme à voix haute, ces signes n'ont aucun sens : on les retire, en gardant les mots.
//!
//! La réponse arrive par morceaux, et ce qui est montré ne doit jamais être repris : le texte
//! nettoyé d'un début de réponse est toujours le début du texte nettoyé de la réponse entière.
//! Quand un signe ne peut pas encore être jugé (il faut voir le caractère suivant), il attend.

use crate::reply::ReplyEvent;

/// Nettoie une réponse au fil de l'eau.
#[derive(Debug, Default)]
pub struct PlainStream {
    raw: String,
    /// Longueur, en octets, du texte nettoyé déjà rendu.
    emitted: usize,
}

impl PlainStream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute un morceau de la réponse et rend le texte brut qu'il permet d'ajouter, parfois vide.
    pub fn push(&mut self, piece: &str) -> String {
        self.raw.push_str(piece);
        self.fresh(false)
    }

    /// La réponse est terminée : rend ce qui attendait encore d'être jugé.
    pub fn finish(&mut self) -> String {
        self.fresh(true)
    }

    /// Fait passer par le nettoyage un événement de la réponse d'un modèle : `emit` reçoit les
    /// mêmes événements, en texte brut. Un morceau qui ne donne rien à montrer n'est pas transmis,
    /// et ce qui attendait d'être jugé part juste avant la fin, que la réponse aboutisse ou non.
    pub fn filter(&mut self, event: ReplyEvent, emit: &mut dyn FnMut(ReplyEvent)) {
        let (fresh, end) = match event {
            ReplyEvent::Delta(piece) => (self.push(&piece), None),
            end => (self.finish(), Some(end)),
        };
        if !fresh.is_empty() {
            emit(ReplyEvent::Delta(fresh));
        }
        if let Some(end) = end {
            emit(end);
        }
    }

    fn fresh(&mut self, complete: bool) -> String {
        let plain = plain_text(&self.raw, complete);
        // le texte nettoyé ne fait que s'allonger : ce qui dépasse est nouveau
        let fresh = plain.get(self.emitted..).unwrap_or_default().to_owned();
        self.emitted = self.emitted.max(plain.len());
        fresh
    }
}

/// Le texte brut de `markdown`. `complete` : la réponse est-elle entière ? Sinon, un signe placé
/// tout au bout, qu'on ne peut pas encore juger, est laissé de côté.
///
/// Ce qui est retiré : les accents graves, les lignes qui ouvrent ou ferment un bloc de code, les
/// astérisques et les tirets bas d'insistance, les dièses des titres. Une puce `* ` devient `• `.
/// Ce qui est gardé : `2 * 3`, `snake_case`, `#mot`, le contenu d'un bloc de code tel quel, et
/// tout le reste. Les lignes vides disparaissent, et le texte ne commence jamais par un blanc.
pub fn plain_text(markdown: &str, complete: bool) -> String {
    let chars: Vec<char> = markdown.chars().collect();
    let mut out = Output::default();
    let mut line = Line::default();
    let mut in_code = false;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let previous = at.checked_sub(1).map(|before| chars[before]);
        let next = chars.get(at + 1).copied();
        // le caractère suivant n'est pas encore arrivé : ce qui en dépend doit attendre
        let pending = next.is_none() && !complete;
        at += 1;

        if c == '\n' {
            out.end_line();
            line = Line::default();
            continue;
        }
        if line.is_fence {
            continue; // le nom du langage, après ```
        }
        if c == '`' {
            if line.ticks_may_open {
                line.ticks += 1;
                if line.ticks == 3 {
                    line.is_fence = true;
                    in_code = !in_code;
                }
            }
            continue;
        }
        if c.is_whitespace() {
            // un blanc clôt la série d'accents graves qui ouvrait la ligne
            line.ticks_may_open &= line.ticks == 0;
            out.space(c);
            continue;
        }

        let opens_the_line = std::mem::replace(&mut line.blank, false);
        line.ticks_may_open = false;
        let spaced = next.is_some_and(char::is_whitespace);
        match c {
            _ if in_code => {}
            '*' | '_' if pending => break,
            '*' => {
                if opens_the_line && spaced {
                    out.push('•');
                } else if !opens_the_line && spaced && previous.is_some_and(char::is_whitespace) {
                    out.push('*'); // « 2 * 3 »
                }
                continue; // sinon : un signe d'insistance
            }
            '_' => {
                // gardé seulement au milieu d'un mot
                if previous.is_some_and(char::is_alphanumeric) && next.is_some_and(char::is_alphanumeric) {
                    out.push('_');
                }
                continue;
            }
            '#' if opens_the_line => {
                // un titre : des dièses en début de ligne, suivis d'une espace
                let run = chars[at - 1..].iter().take_while(|&&c| c == '#').count();
                match chars.get(at - 1 + run) {
                    None if !complete => break,
                    Some(' ') => {
                        at += run; // les dièses, et leur espace
                        line.blank = true;
                        continue;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        out.push(c);
    }
    if complete {
        out.text.truncate(out.text.trim_end().len());
    }
    out.text
}

/// Ce qu'on sait de la ligne en cours du texte d'origine.
struct Line {
    /// Rien d'autre que des blancs et des accents graves jusqu'ici.
    blank: bool,
    /// Les accents graves qui ouvrent la ligne, et si leur série peut encore s'allonger.
    ticks: usize,
    ticks_may_open: bool,
    /// La ligne ouvre ou ferme un bloc de code : rien n'en est montré.
    is_fence: bool,
}

impl Default for Line {
    fn default() -> Self {
        Self { blank: true, ticks: 0, ticks_may_open: true, is_fence: false }
    }
}

/// Le texte nettoyé, en construction.
#[derive(Default)]
struct Output {
    text: String,
    /// Les blancs qui ouvrent la ligne : ils ne sont écrits que si quelque chose les suit.
    indent: String,
    line_has_text: bool,
}

impl Output {
    fn push(&mut self, c: char) {
        if !std::mem::replace(&mut self.line_has_text, true) && !self.text.is_empty() {
            self.text.push_str(&self.indent);
        }
        self.text.push(c);
    }

    fn space(&mut self, c: char) {
        if self.line_has_text {
            self.text.push(c);
        } else {
            self.indent.push(c);
        }
    }

    /// Pas de ligne vide, ni de saut de ligne en tête du texte.
    fn end_line(&mut self) {
        self.indent.clear();
        self.line_has_text = false;
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(markdown: &str) -> String {
        plain_text(markdown, true)
    }

    #[test]
    fn emphasis_marks_go_and_the_words_stay() {
        assert_eq!(plain("une ville construite *exprès* pour ça"), "une ville construite exprès pour ça");
        assert_eq!(plain("C'est **très** important, __vraiment__."), "C'est très important, vraiment.");
        assert_eq!(plain("***Attention*** : _ça_ glisse"), "Attention : ça glisse");
        assert_eq!(plain("Lance `cargo test`, puis ``relis``."), "Lance cargo test, puis relis.");
        assert_eq!(plain("du levain du *canard* !"), "du levain du canard !");
    }

    #[test]
    fn signs_that_mean_something_are_kept() {
        assert_eq!(plain("2 * 3 = 6"), "2 * 3 = 6");
        assert_eq!(
            plain("la variable max_turns et le fichier mon_fichier_2.rs"),
            "la variable max_turns et le fichier mon_fichier_2.rs"
        );
        assert_eq!(
            plain("le canal #général et le C# de Microsoft"),
            "le canal #général et le C# de Microsoft"
        );
        assert_eq!(plain("#hashtag en tête de ligne"), "#hashtag en tête de ligne");
        assert_eq!(plain("Ça marche, l'« accent » ? 🙂 — oui."), "Ça marche, l'« accent » ? 🙂 — oui.");
        assert_eq!(plain("1. un\n2. deux\n- trois"), "1. un\n2. deux\n- trois");
    }

    #[test]
    fn headings_bullets_and_code_fences_become_plain_lines() {
        assert_eq!(plain("# Titre\n## Sous-titre\nDu texte."), "Titre\nSous-titre\nDu texte.");
        assert_eq!(plain("* un\n* deux\n  * trois"), "• un\n• deux\n  • trois");
        let code = "Exemple :\n```rust\nlet compteur = 0;\nlock(mutex)\n```\nVoilà.";
        assert_eq!(plain(code), "Exemple :\nlet compteur = 0;\nlock(mutex)\nVoilà.");
        assert_eq!(plain("```\nseul\n```"), "seul");
        assert_eq!(plain("  ```python\n  x = 1\n  ```\nfin"), "x = 1\nfin");
    }

    #[test]
    fn code_inside_a_fence_is_left_alone() {
        let code = "Regarde :\n```python\ndef __init__(self, **kwargs):\n    # un commentaire\n    return a * b\n```\nC'est *tout*.";
        assert_eq!(
            plain(code),
            "Regarde :\ndef __init__(self, **kwargs):\n    # un commentaire\n    return a * b\nC'est tout."
        );
        // un bloc jamais refermé : la suite reste telle quelle
        assert_eq!(plain("```\nx = __y__"), "x = __y__");
    }

    #[test]
    fn blank_lines_and_leading_whitespace_are_squeezed() {
        assert_eq!(plain("\n\n  Bonjour.\n\n\nÇa va ?\n\n"), "Bonjour.\nÇa va ?");
        assert_eq!(plain("un\n   \n\t\ndeux"), "un\ndeux", "une ligne de blancs est une ligne vide");
        assert_eq!(plain("   "), "");
        assert_eq!(plain(""), "");
        assert_eq!(plain("un\r\ndeux"), "un\r\ndeux", "un retour chariot est un blanc comme un autre");
    }

    #[test]
    fn what_cannot_be_judged_yet_waits() {
        // en cours de réponse, un signe tout au bout n'est ni montré ni retiré
        assert_eq!(plain_text("un mot *", false), "un mot ");
        assert_eq!(plain_text("un mot * ", false), "un mot * ");
        assert_eq!(plain_text("snake_", false), "snake");
        assert_eq!(plain_text("snake_case", false), "snake_case");
        assert_eq!(plain_text("Texte\n##", false), "Texte\n");
        assert_eq!(plain_text("Texte\n## Titre", false), "Texte\nTitre");
        assert_eq!(plain_text("Texte\n##mot", false), "Texte\n##mot");
        // la réponse finie, il est jugé comme s'il était suivi d'un blanc
        assert_eq!(plain("un mot *"), "un mot");
        assert_eq!(plain("snake_"), "snake");
        assert_eq!(plain("Texte\n##"), "Texte\n##");
    }

    /// Des réponses comme en écrivent les modèles, Markdown compris.
    const REPLIES: [&str; 6] = [
        "Canberra, bien sûr ! Mais tu savais que c'est une ville construite *exprès* pour être la capitale ? T'es en train de bosser sur quoi, toi ?",
        "Un mutex, c'est comme une porte.\n\nExemple en pseudo-code (pour un compteur partagé) :\n```\nmutex = créer un mutex\ncompteur = 0\n\nthread 1 :\n  lock(mutex)\n  compteur = compteur + 1\n```\nSans mutex, un thread pourrait lire `compteur=0`, l'incrémenter à `1`.",
        "## Trois idées\n\n* **Première** : dormir\n* **Deuxième** : 2 * 3 = 6\n* _Troisième_ : relire mon_fichier.rs\n\n# Fin",
        "Parce qu'ils se lèvent du *canard* ! (comme \"bonjour\" mais en plus rigolo) 😉",
        "***\n___\n`` ` ``\n#\n* \n_ _ * *\n",
        "",
    ];

    #[test]
    fn what_was_shown_is_never_taken_back() {
        for reply in REPLIES {
            let whole = plain(reply);
            let mut shown = String::new();
            for (end, _) in reply.char_indices().chain([(reply.len(), ' ')]) {
                let partial = plain_text(&reply[..end], false);
                assert!(
                    partial.starts_with(&shown),
                    "{:?} : {shown:?} était montré, puis {partial:?}",
                    &reply[..end]
                );
                assert!(
                    whole.starts_with(partial.trim_end()),
                    "{:?} : {partial:?} n'ouvre pas {whole:?}",
                    &reply[..end]
                );
                shown = partial;
            }
        }
    }

    #[test]
    fn a_stream_gives_the_same_text_whatever_the_size_of_its_pieces() {
        for reply in REPLIES {
            let chars: Vec<char> = reply.chars().collect();
            for size in [1, 2, 3, 7, 50, 10_000] {
                let mut stream = PlainStream::new();
                let mut text = String::new();
                for piece in chars.chunks(size) {
                    text.push_str(&stream.push(&piece.iter().collect::<String>()));
                }
                text.push_str(&stream.finish());
                assert_eq!(text.trim_end(), plain(reply), "morceaux de {size} caractères");
            }
        }
        let mut stream = PlainStream::new();
        assert_eq!(stream.push("Salut **to"), "Salut to");
        assert_eq!(stream.push("i** ! *"), "i ! ");
        assert_eq!(stream.push(""), "");
        assert_eq!(stream.finish(), "");
    }

    #[test]
    fn reply_events_come_out_as_plain_text() {
        let filtered = |events: Vec<ReplyEvent>| {
            let (mut stream, mut out) = (PlainStream::new(), Vec::new());
            for event in events {
                stream.filter(event, &mut |event| out.push(event));
            }
            out
        };
        let delta = |text: &str| ReplyEvent::Delta(text.to_owned());
        assert_eq!(
            filtered(vec![
                delta("Voici "),
                delta("**"),
                delta("le"),
                delta("** fichier mon"),
                delta("_"),
                delta("fichier"),
                ReplyEvent::Done
            ]),
            [delta("Voici "), delta("le"), delta(" fichier mon"), delta("_fichier"), ReplyEvent::Done],
            "un morceau fait seulement de signes ne donne rien, et un signe en suspens attend la suite"
        );
        // ce qui attendait part avant la fin, même quand la réponse échoue
        let failed = ReplyEvent::Failed("coupé".to_owned());
        assert_eq!(filtered(vec![delta("Titre\n#"), failed.clone()]), [delta("Titre\n"), delta("#"), failed]);
        assert_eq!(filtered(vec![ReplyEvent::Done]), [ReplyEvent::Done]);
    }
}
