//! Décodeur de « server-sent events » : le format dans lequel les fournisseurs envoient une
//! réponse au fil de l'eau.
//!
//! Le réseau livre des octets par paquets de taille quelconque : une ligne, un caractère accentué
//! ou une fin de ligne `\r\n` peuvent être coupés n'importe où. Ce décodeur avale les octets comme
//! ils viennent et rend les événements complets, c'est-à-dire le contenu de leurs lignes `data:`.
//! Il ne sait rien de ce que ce contenu veut dire.
//!
//! Suit la spécification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>) :
//! fins de ligne `\n`, `\r\n` ou `\r`, commentaires (lignes qui commencent par `:`), plusieurs
//! lignes `data:` jointes par des retours à la ligne, événement clos par une ligne vide.

/// Taille maximale d'un événement. Un morceau de réponse fait quelques centaines d'octets : au-delà
/// de cette limite, ce qui arrive n'est pas une réponse, et on ne le garde pas en mémoire.
pub const MAX_EVENT_BYTES: usize = 256 * 1024;

/// Un événement dépasse [`MAX_EVENT_BYTES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("événement de plus de {MAX_EVENT_BYTES} octets dans le flux")]
pub struct TooLong;

#[derive(Debug, Default)]
pub struct SseDecoder {
    /// La ligne en cours, sans sa fin de ligne.
    line: Vec<u8>,
    /// Les lignes `data:` de l'événement en cours, jointes par des `\n`.
    data: Vec<u8>,
    has_data: bool,
    /// Le dernier octet était un `\r` : un `\n` juste après appartient à la même fin de ligne.
    after_cr: bool,
    /// Une première ligne a-t-elle déjà été lue ? (La toute première peut commencer par un BOM.)
    started: bool,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prend les octets qui viennent d'arriver et rend les événements qu'ils terminent, dans
    /// l'ordre.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<String>, TooLong> {
        let mut events = Vec::new();
        for &byte in bytes {
            if std::mem::take(&mut self.after_cr) && byte == b'\n' {
                continue;
            }
            match byte {
                b'\n' => self.end_line(&mut events),
                b'\r' => {
                    self.end_line(&mut events);
                    self.after_cr = true;
                }
                _ => {
                    if self.line.len() + self.data.len() >= MAX_EVENT_BYTES {
                        return Err(TooLong);
                    }
                    self.line.push(byte);
                }
            }
        }
        Ok(events)
    }

    /// Le flux est terminé. Rend l'événement en cours s'il portait des données : des serveurs
    /// ferment la connexion sans la ligne vide finale, et il serait dommage de perdre leur
    /// dernier mot. Un événement coupé en plein milieu est rendu tel quel : à celui qui
    /// l'interprète de voir qu'il est illisible.
    pub fn finish(&mut self) -> Option<String> {
        let mut events = Vec::new();
        if !self.line.is_empty() {
            self.end_line(&mut events);
        }
        self.end_event(&mut events);
        self.after_cr = false;
        events.pop()
    }

    fn end_line(&mut self, events: &mut Vec<String>) {
        let mut line = std::mem::take(&mut self.line);
        self.interpret(&line, events);
        // on garde la mémoire déjà réservée pour la ligne suivante
        line.clear();
        self.line = line;
    }

    fn interpret(&mut self, mut line: &[u8], events: &mut Vec<String>) {
        if !std::mem::replace(&mut self.started, true) {
            line = line.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(line);
        }
        if line.is_empty() {
            return self.end_event(events);
        }
        let (field, value) = match line.iter().position(|&byte| byte == b':') {
            Some(0) => return, // un commentaire, souvent là pour tenir la connexion ouverte
            Some(colon) => {
                let value = &line[colon + 1..];
                (&line[..colon], value.strip_prefix(b" ").unwrap_or(value))
            }
            None => (line, &[][..]),
        };
        // `event:`, `id:` et `retry:` ne nous apprennent rien : les fournisseurs répètent le type
        // de l'événement dans ses données.
        if field == b"data" {
            if self.has_data {
                self.data.push(b'\n');
            }
            self.data.extend_from_slice(value);
            self.has_data = true;
        }
    }

    fn end_event(&mut self, events: &mut Vec<String>) {
        if std::mem::take(&mut self.has_data) {
            events.push(String::from_utf8_lossy(&self.data).into_owned());
            self.data.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Décode `stream` livré par paquets de `size` octets.
    fn decode_by(stream: &[u8], size: usize) -> Vec<String> {
        let mut decoder = SseDecoder::new();
        let mut events = Vec::new();
        for packet in stream.chunks(size) {
            events.extend(decoder.feed(packet).expect("flux raisonnable"));
        }
        events.extend(decoder.finish());
        events
    }

    #[test]
    fn events_survive_any_packet_boundary() {
        let stream = "data: {\"a\":\"Ça marche, déjà 🙂\"}\r\n\r\n: un commentaire\r\n\r\ndata: deux\n\ndata: [DONE]\n\n";
        let whole = decode_by(stream.as_bytes(), stream.len());
        assert_eq!(whole, ["{\"a\":\"Ça marche, déjà 🙂\"}", "deux", "[DONE]"]);
        // coupé partout, y compris au milieu d'un accent, d'un émoji et d'un `\r\n`
        for size in 1..stream.len() {
            assert_eq!(decode_by(stream.as_bytes(), size), whole, "paquets de {size} octets");
        }
    }

    #[test]
    fn the_three_line_endings_mean_the_same() {
        for ending in ["\n", "\r\n", "\r"] {
            let stream = format!("data: un{ending}{ending}data: deux{ending}{ending}");
            for size in 1..=stream.len() {
                assert_eq!(decode_by(stream.as_bytes(), size), ["un", "deux"], "{ending:?} par {size}");
            }
        }
    }

    #[test]
    fn several_data_lines_make_one_event() {
        assert_eq!(decode_by(b"data: un\ndata:deux\ndata:\ndata:  trois\n\n", 7), ["un\ndeux\n\n trois"]);
    }

    #[test]
    fn comments_other_fields_and_empty_events_are_skipped() {
        let stream = b": OPENROUTER PROCESSING\n\nevent: message\nid: 7\nretry: 10\ndata: utile\n\n\n\nsans deux-points\n\n";
        assert_eq!(decode_by(stream, 5), ["utile"]);
    }

    #[test]
    fn a_leading_byte_order_mark_is_ignored() {
        assert_eq!(
            decode_by(b"\xEF\xBB\xBFdata: un\n\ndata: \xEF\xBB\xBFdeux\n\n", 3),
            ["un", "\u{feff}deux"]
        );
    }

    #[test]
    fn a_stream_that_ends_without_a_blank_line_keeps_its_last_event() {
        assert_eq!(decode_by(b"data: un\n\ndata: dernier", 4), ["un", "dernier"]);
        assert_eq!(decode_by(b"data: un\n\ndata: dernier\n", 4), ["un", "dernier"]);
        assert_eq!(decode_by(b"data: un\n\n: rien d'autre", 4), ["un"]);
        assert!(decode_by(b"", 1).is_empty());
    }

    #[test]
    fn bytes_that_are_not_utf8_do_not_break_the_stream() {
        assert_eq!(decode_by(b"data: a\xFFb\n\ndata: suite\n\n", 2), ["a\u{fffd}b", "suite"]);
    }

    #[test]
    fn an_endless_line_is_refused_instead_of_filling_memory() {
        let mut decoder = SseDecoder::new();
        let packet = [b'x'; 64 * 1024];
        let outcome = (0..8).try_for_each(|_| decoder.feed(&packet).map(drop));
        assert_eq!(outcome, Err(TooLong));

        // même limite pour un événement fait de beaucoup de petites lignes
        let mut decoder = SseDecoder::new();
        let line = format!("data: {}\n", "y".repeat(1000));
        let outcome = (0..400).try_for_each(|_| decoder.feed(line.as_bytes()).map(drop));
        assert_eq!(outcome, Err(TooLong));
    }
}
