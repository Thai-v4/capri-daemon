//! Petits outils pour les textes affichés dans le bandeau.

/// La première ligne non vide de `text`, espaces resserrés, coupée à `max_chars` caractères
/// (points de suspension compris). Jamais de saut de ligne ni de caractère de contrôle.
pub fn one_line(text: &str, max_chars: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut out = String::with_capacity(line.len().min(max_chars * 4));
    let mut pending_space = false;
    for c in line.chars() {
        if c.is_whitespace() || c.is_control() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(c);
        }
    }
    truncate(&out, max_chars)
}

/// Coupe à `max_chars` caractères, en remplaçant la fin par « … » si ça dépasse.
pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.truncate(out.trim_end().len());
    out.push('…');
    out
}

/// Le nom du fichier au bout d'un chemin, qu'il soit écrit avec `/` ou avec `\`.
pub fn file_name(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['/', '\\']);
    trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_keeps_the_first_line_and_squeezes_spaces() {
        assert_eq!(one_line("  cargo   test \t --workspace\nrm -rf /", 80), "cargo test --workspace");
        assert_eq!(one_line("\n\n  deuxième ligne\n", 80), "deuxième ligne");
        assert_eq!(one_line("a\u{7}b", 80), "a b");
        assert_eq!(one_line("", 80), "");
    }

    #[test]
    fn truncates_on_characters_not_bytes() {
        assert_eq!(truncate("éééééé", 6), "éééééé");
        assert_eq!(truncate("ééééééé", 6), "ééééé…");
        assert_eq!(truncate("un deux trois", 8), "un deux…");
        assert_eq!(one_line("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn file_name_understands_both_separators() {
        assert_eq!(file_name("/Users/moi/projet/src/main.rs"), "main.rs");
        assert_eq!(file_name("C:\\projet\\src\\index.ts"), "index.ts");
        assert_eq!(file_name("/Users/moi/projet/"), "projet");
        assert_eq!(file_name("seul.txt"), "seul.txt");
    }
}
