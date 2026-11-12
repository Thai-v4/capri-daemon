//! Son journal dans un fichier : quand passer au fichier suivant, et la borne d'une ligne.
//!
//! Elle tourne des jours entiers : le fichier ne doit jamais grossir sans limite, même si une
//! ligne se répète en boucle. Deux fichiers au plus, le courant et le précédent, chacun d'au plus
//! [`MAX_BYTES`] octets : avant d'écrire une ligne qui ferait dépasser la borne, le courant
//! devient le précédent (l'ancien précédent disparaît) et un nouveau commence. Au lancement de
//! même, si le fichier laissé par la fois d'avant dépasse déjà la borne.
//!
//! Ici, seulement les décisions, pures ; l'écriture (le fichier, ses droits, ses pannes) est dans
//! `capri-app` (`journal.rs`), et sa place dans `capri_claude::places::log_file`.

use std::borrow::Cow;

/// La taille maximale d'un fichier du journal : 1 Mio (le courant et le précédent : 2 Mio au
/// plus). Au niveau `info`, elle n'écrit que quelques dizaines de lignes par lancement, d'une
/// centaine d'octets chacune.
pub const MAX_BYTES: u64 = 1024 * 1024;

/// Ce qui termine une ligne rognée ([`clip`]).
pub const CLIPPED: &str = "…\n";

/// Faut-il passer au fichier suivant avant d'écrire `incoming` octets dans un fichier qui en a
/// déjà `len`, pour qu'il n'en ait jamais plus de `max` ? Un fichier vide ne passe jamais la main :
/// une ligne plus longue que la borne y est d'abord rognée ([`clip`]). Au lancement, `incoming`
/// vaut zéro : le fichier d'avant ne tourne que s'il dépasse déjà la borne.
pub fn must_rotate(len: u64, incoming: u64, max: u64) -> bool {
    len > 0 && len.saturating_add(incoming) > max
}

/// `line`, telle quelle si elle tient dans un fichier de `max` octets ; sinon rognée pour y tenir,
/// terminée par [`CLIPPED`], sans jamais couper un caractère en deux. Vide si même [`CLIPPED`] ne
/// tient pas.
pub fn clip(line: &[u8], max: u64) -> Cow<'_, [u8]> {
    let max = usize::try_from(max).unwrap_or(usize::MAX);
    if line.len() <= max {
        return Cow::Borrowed(line);
    }
    let Some(room) = max.checked_sub(CLIPPED.len()) else { return Cow::Borrowed(&[]) };
    // `room < line.len()` : l'octet à `cut` existe. Un octet de suite d'UTF-8 commence par 10 :
    // on recule jusqu'au début d'un caractère.
    let mut cut = room;
    while cut > 0 && line[cut] & 0xC0 == 0x80 {
        cut -= 1;
    }
    let mut clipped = Vec::with_capacity(cut + CLIPPED.len());
    clipped.extend_from_slice(&line[..cut]);
    clipped.extend_from_slice(CLIPPED.as_bytes());
    Cow::Owned(clipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_rotates_just_before_the_line_that_would_cross_the_bound() {
        assert!(!must_rotate(90, 10, 100), "pile à la borne : elle tient");
        assert!(must_rotate(90, 11, 100));
        assert!(!must_rotate(0, 10_000, 100), "un fichier vide ne tourne jamais");
        assert!(!must_rotate(u64::MAX - 1, 0, u64::MAX));
        assert!(must_rotate(u64::MAX, u64::MAX, 1), "sans débordement");
    }

    #[test]
    fn at_launch_only_a_file_already_over_the_bound_rotates() {
        assert!(!must_rotate(0, 0, MAX_BYTES));
        assert!(!must_rotate(MAX_BYTES / 2, 0, MAX_BYTES));
        assert!(!must_rotate(MAX_BYTES, 0, MAX_BYTES), "pile à la borne : la ligne suivante tournera");
        assert!(must_rotate(MAX_BYTES + 1, 0, MAX_BYTES));
    }

    #[test]
    fn a_short_line_is_kept_as_is() {
        let line = b"[2026-11-12T10:00:00Z INFO  capri_app] images lues en 12 ms\n";
        assert!(matches!(clip(line, 100), Cow::Borrowed(kept) if kept == line));
        assert!(matches!(clip(line, line.len() as u64), Cow::Borrowed(kept) if kept == line));
    }

    #[test]
    fn a_long_line_is_clipped_to_fit_and_says_so() {
        let line = "x".repeat(500);
        let clipped = clip(line.as_bytes(), 100);
        assert_eq!(clipped.len(), 100);
        assert!(clipped.ends_with(CLIPPED.as_bytes()));
        assert!(clipped.starts_with(b"xxxx"));
    }

    #[test]
    fn a_character_is_never_cut_in_two() {
        // deux, trois et quatre octets par caractère
        let line = "é€😺".repeat(40);
        for max in 0..=line.len() as u64 + 2 {
            let clipped = clip(line.as_bytes(), max);
            assert!(clipped.len() as u64 <= max, "{max} : {} octets", clipped.len());
            assert!(std::str::from_utf8(&clipped).is_ok(), "{max} : coupé au milieu d'un caractère");
        }
        assert!(clip(line.as_bytes(), 3).is_empty(), "même la marque ne tient pas");
        assert_eq!(&*clip(line.as_bytes(), 4), CLIPPED.as_bytes());
    }

    #[test]
    fn written_in_a_loop_neither_file_ever_crosses_the_bound() {
        // ce que ferait l'écriture : tourner quand il le faut, puis écrire la ligne rognée
        let max = 300;
        let lines = [10_usize, 120, 299, 300, 301, 5_000, 1, 0, 150, 150, 151];
        let (mut current, mut previous, mut rotations) = (0_u64, 0_u64, 0);
        for round in 0..1_000 {
            let line = vec![b'a'; lines[round % lines.len()]];
            let line = clip(&line, max);
            if must_rotate(current, line.len() as u64, max) {
                (previous, current) = (current, 0);
                rotations += 1;
            }
            current += line.len() as u64;
            assert!(current <= max && previous <= max, "tour {round} : {current} et {previous}");
        }
        assert!(rotations > 100, "{rotations} rotations");
        // la même ligne, en boucle
        let (mut current, line) = (0_u64, [b'b'; 70]);
        for _ in 0..10_000 {
            if must_rotate(current, line.len() as u64, max) {
                current = 0;
            }
            current += line.len() as u64;
            assert!(current <= max);
        }
    }
}
