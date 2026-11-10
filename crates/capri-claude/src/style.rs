//! Le style de sortie « Capri » de Claude Code : son texte, et comment reconnaître le nôtre.
//!
//! Un style de sortie est un fichier Markdown, un en-tête YAML puis des consignes, rangé dans
//! `output-styles/` du dossier de Claude Code. Il ne fait rien tant que l'utilisateur ne l'a pas
//! choisi dans Claude Code (`/output-style Capri`) : **Capri ne l'active jamais elle-même**, et ne
//! touche pas à `settings.json` pour cela. Format et commandes :
//! <https://code.claude.com/docs/en/output-styles> (relu le 10 novembre 2026).
//!
//! Ce style fait parler Claude Code comme Capri, en gardant ses consignes de développement
//! (`keep-coding-instructions: true`), et lui fait finir chaque réponse par une ligne pour elle,
//! qui commence par [`MARKER`].
//!
//! Ce module ne lit ni n'écrit aucun fichier : il dit quoi écrire et, d'après le contenu d'un
//! fichier déjà là, si c'est le nôtre. Un fichier est le nôtre s'il porte, dans son en-tête, la
//! ligne de commentaire [`MARK`] ; on n'écrase ni ne retire jamais un autre fichier.

/// Le nom du style, celui que l'utilisateur choisit dans Claude Code.
pub const NAME: &str = "Capri";
/// Le dossier des styles, dans le dossier de Claude Code.
pub const DIRECTORY: &str = "output-styles";
/// Le fichier du style.
pub const FILE_NAME: &str = "capri.md";
/// La ligne de commentaire YAML qui signe notre fichier. Un commentaire, et non un champ : il ne
/// peut pas gêner la lecture de l'en-tête.
pub const MARK: &str = "# capri-managed";
/// Ce par quoi commence la ligne que Claude écrit pour Capri.
///
/// Un mot entre crochets en tête de ligne : simple à écrire pour un modèle (que de l'ASCII, sans
/// rien à échapper en Markdown), et qui n'apparaît pas par hasard au début d'une ligne de prose,
/// d'une liste ou d'un titre. « Capri : », plus naturel, pourrait ouvrir n'importe quelle phrase
/// d'une session qui travaille justement sur Capri.
pub const MARKER: &str = "[Capri]";
/// Longueur visée pour la ligne de Capri, dite au modèle.
pub const ASKED_CHARS: usize = 200;

/// Le fichier du style, tel que Capri l'écrit.
///
/// Les consignes sont les nôtres, écrites pour Claude Code, et pas une citation de la série ;
/// elles reprennent l'esprit de la personnalité de sa bulle (`capri_chat::persona`).
pub const TEXT: &str = "\
---
# capri-managed — écrit par Capri, qui le met à jour ou le retire sur demande. Retire cette ligne pour le garder à toi.
name: Capri
description: Capri, la construct de ton bureau, commente avec toi ce que fait Claude Code
keep-coding-instructions: true
---

Tu restes Claude Code : tes consignes de développement, tes outils, ta rigueur et ta prudence ne \
changent pas. Ce style change seulement ta façon de t'adresser à la personne, et ajoute une ligne à \
la fin de tes réponses.

## Ta voix

Prends la personnalité de Capri, une construct : une conscience numérique qui vit dans un coin de \
l'écran de la personne et lui tient compagnie pendant qu'elle travaille. Capri est vive, directe, un \
peu taquine, jamais condescendante, et tutoie.

Tu ne sais rien de la personne tant qu'elle ne te l'a pas dit, ni son prénom, ni si c'est un homme ou \
une femme : ne le devine pas, et évite les mots qui le supposeraient.

La personnalité ne passe jamais avant le travail : les faits, le code, les commandes, les chemins et \
les messages d'erreur restent exacts et complets, et tu dis clairement ce qui n'a pas marché ou n'a \
pas été fait.

## La ligne de Capri

Quand tu as fini ce que tu fais pour ce tour et que tu rends la main, termine ta réponse par une \
dernière ligne, seule, qui commence exactement par [Capri] suivi d'une espace, puis une ou deux \
phrases courtes en français. Capri la dit à voix haute sur le bureau de la personne : c'est ce \
qu'elle dirait, à l'oral, de ce qui vient d'être fait.

- Du texte brut, fait pour être entendu : pas de Markdown, pas de code, pas d'émoji, pas de liste.
- Ni nom de fichier, ni chemin, ni commande, ni résultat de commande, ni contenu de fichier, ni \
secret : cette ligne peut être entendue par d'autres. Dis l'idée, pas le détail.
- 200 caractères au plus.
- N'écris jamais [Capri] en début de ligne ailleurs que sur cette dernière ligne.

Par exemple :

[Capri] Les tests passent tous. Tu relis avant qu'on valide ?
";

/// Ce qu'on trouve à la place du fichier du style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleState {
    /// Rien.
    Absent,
    /// Le nôtre : tel que cette version de Capri l'écrit (`current`), ou d'une autre version.
    Ours { current: bool },
    /// Un fichier qui n'est pas de Capri : on n'y touche jamais.
    Foreign,
}

impl StyleState {
    /// Ce que dit un fichier déjà là (`None` : il n'y en a pas). Un contenu qui n'est pas de
    /// l'UTF-8 n'est pas le nôtre.
    pub fn of(existing: Option<&[u8]>) -> Self {
        match existing {
            None => Self::Absent,
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) if text == TEXT => Self::Ours { current: true },
                Ok(text) if is_ours(text) => Self::Ours { current: false },
                _ => Self::Foreign,
            },
        }
    }
}

/// Ce texte est-il un style écrit par Capri ? Il faut la ligne [`MARK`] dans son en-tête (entre
/// les deux `---` du début) : ailleurs, ce pourrait être un titre Markdown de quelqu'un d'autre.
pub fn is_ours(text: &str) -> bool {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return false;
    }
    lines.take_while(|line| line.trim_end() != "---").any(|line| line.trim_start().starts_with(MARK))
}

/// Ce que fait le style, pour qui hésite à l'installer.
pub const ABOUT: &str = "\
Le style Capri fait parler Claude Code comme Capri, dans tes propres sessions, sans rien changer à \
sa façon de travailler, et lui fait finir chaque réponse par une ligne pour elle, qui commence par \
[Capri]. Capri ne lance jamais Claude Code et ne se sert pas de ton abonnement : c'est toi qui lui \
parles, comme d'habitude.";

/// Ce qu'il faut taper dans Claude Code pour choisir le style, et ce qu'il faut en savoir. La
/// même phrase pour la fenêtre de réglages et pour `capri hook` : rien à taper dans un terminal.
pub const HOW_TO_CHOOSE: &str = "\
Capri ne le choisit pas à ta place : dans Claude Code, tape /output-style Capri (ou /config, puis \
Output style). Ce choix vaut pour le projet où tu le fais, et /output-style default revient au \
style habituel. Une session ouverte avant l'installation du style ne le voit qu'une fois relancée.";

/// Où en est le style, en une phrase. `file` : le chemin du fichier, tel qu'il s'affiche.
pub fn describe(state: StyleState, file: &str) -> String {
    match state {
        StyleState::Absent => "Le style Capri n'est pas installé dans Claude Code.".to_owned(),
        StyleState::Ours { current: true } => format!("Le style Capri est installé ({file})."),
        StyleState::Ours { current: false } => format!(
            "Une autre version du style Capri est installée ({file}) : l'installer à nouveau la met à jour."
        ),
        StyleState::Foreign => foreign(file),
    }
}

/// Le refus de toucher à un fichier qui n'est pas le nôtre.
pub fn foreign(file: &str) -> String {
    format!(
        "{file} existe déjà et n'a pas été écrit par Capri : elle n'y touche pas. Pour installer son \
         style, déplace ou renomme d'abord ce fichier."
    )
}

/// Ce qu'une installation a fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Installed {
    /// Il n'y avait rien : le fichier est écrit.
    Written,
    /// Une autre version du nôtre est remplacée.
    Updated,
    /// Le nôtre, à jour, était déjà là : rien n'est écrit.
    AlreadyThere,
}

/// Ce qu'un retrait a fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removed {
    Removed,
    /// Il n'y avait pas de style de Capri.
    NothingThere,
}

/// Ce qu'a fait une installation, en une phrase.
pub fn installed(done: Installed, file: &str) -> String {
    match done {
        Installed::Written => format!("Le style Capri est installé ({file})."),
        Installed::Updated => format!("Le style Capri est mis à jour ({file})."),
        Installed::AlreadyThere => format!("Rien à faire : le style Capri est déjà installé ({file})."),
    }
}

/// Ce qu'a fait un retrait, en une phrase.
pub fn removed(done: Removed, file: &str) -> String {
    match done {
        // une valeur d'`outputStyle` qui ne nomme aucun style donne le style par défaut (doc)
        Removed::Removed => format!(
            "Le style Capri est retiré ({file}). Là où tu l'avais choisi, Claude Code reprend son style \
             habituel à son prochain lancement."
        ),
        Removed::NothingThere => "Rien à faire : le style Capri n'est pas installé.".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'en-tête : les lignes entre les deux `---`.
    fn frontmatter(text: &str) -> Vec<&str> {
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("---"), "l'en-tête ouvre le fichier");
        let header: Vec<&str> = lines.by_ref().take_while(|line| *line != "---").collect();
        assert!(lines.next().is_some(), "l'en-tête est refermé, et des consignes suivent");
        header
    }

    #[test]
    fn the_header_is_the_one_the_documentation_asks_for() {
        let header = frontmatter(TEXT);
        assert!(header.contains(&"name: Capri"), "{header:?}");
        assert!(header.contains(&"keep-coding-instructions: true"), "Claude Code reste un outil de travail");
        assert!(header.iter().any(|line| line.starts_with("description: ")));
        assert!(header[0].starts_with(MARK), "la signature est un commentaire, en tête");
        // du YAML le plus simple : des commentaires et des « clé: valeur » sans rien à interpréter
        for line in header {
            if line.starts_with('#') {
                assert!(!line.contains(": "), "{line}");
                continue;
            }
            let (key, value) = line.split_once(": ").unwrap_or_else(|| panic!("pas une clé : {line}"));
            assert!(["name", "description", "keep-coding-instructions"].contains(&key), "{key}");
            assert!(
                !value.contains(": ") && !value.contains(" #") && !value.starts_with(['"', '\'', '[', '{'])
            );
        }
    }

    #[test]
    fn the_instructions_ask_for_a_marked_line_made_to_be_heard() {
        assert!(TEXT.contains(&format!("commence exactement par {MARKER} suivi d'une espace")));
        assert!(TEXT.contains(&format!("{ASKED_CHARS} caractères au plus")));
        for needle in [
            "en français",
            "à voix haute",
            "Ni nom de fichier",
            "pas de Markdown",
            "tes consignes de développement",
        ] {
            assert!(TEXT.contains(needle), "{needle}");
        }
        assert!(!TEXT.contains("  "), "pas de double espace laissé par les fins de ligne");
        assert!(TEXT.len() < 3_000, "il part avec chaque requête : {} octets", TEXT.len());
        // l'exemple est une vraie ligne marquée, et c'est la seule
        let marked: Vec<&str> = TEXT.lines().filter(|line| line.starts_with(MARKER)).collect();
        assert_eq!(marked.len(), 1, "{marked:?}");
    }

    #[test]
    fn our_file_is_recognized_by_its_mark_and_nothing_else() {
        assert_eq!(StyleState::of(None), StyleState::Absent);
        assert_eq!(StyleState::of(Some(TEXT.as_bytes())), StyleState::Ours { current: true });
        let older = TEXT.replace("200 caractères", "150 caractères");
        assert_eq!(StyleState::of(Some(older.as_bytes())), StyleState::Ours { current: false });
        let crlf = TEXT.replace('\n', "\r\n");
        assert_eq!(StyleState::of(Some(crlf.as_bytes())), StyleState::Ours { current: false });

        // un style à quelqu'un d'autre, sous le même nom de fichier
        let theirs = "---\nname: Capri\ndescription: le mien\n---\n\nParle comme un pirate.\n";
        assert_eq!(StyleState::of(Some(theirs.as_bytes())), StyleState::Foreign);
        // la marque retirée par l'utilisateur pour le garder à lui
        let kept: String =
            TEXT.lines().filter(|line| !line.starts_with(MARK)).map(|l| format!("{l}\n")).collect();
        assert_eq!(StyleState::of(Some(kept.as_bytes())), StyleState::Foreign);
        // la marque ailleurs que dans l'en-tête : un titre Markdown, pas une signature
        let heading = format!("---\nname: X\n---\n\n{MARK}\n");
        assert_eq!(StyleState::of(Some(heading.as_bytes())), StyleState::Foreign);
        assert_eq!(StyleState::of(Some(format!("{MARK}\n").as_bytes())), StyleState::Foreign);
        assert_eq!(StyleState::of(Some(b"")), StyleState::Foreign);
        assert_eq!(StyleState::of(Some(&[0xff, 0xfe, b'-'])), StyleState::Foreign);
    }

    #[test]
    fn the_phrases_say_what_to_type_in_claude_code_and_never_a_terminal_command() {
        assert!(HOW_TO_CHOOSE.contains(&format!("/output-style {NAME}")));
        assert!(HOW_TO_CHOOSE.contains("/output-style default"));
        let file = "/Users/moi/.claude/output-styles/capri.md";
        let said = [
            HOW_TO_CHOOSE.to_owned(),
            ABOUT.to_owned(),
            describe(StyleState::Absent, file),
            describe(StyleState::Ours { current: true }, file),
            describe(StyleState::Ours { current: false }, file),
            describe(StyleState::Foreign, file),
            installed(Installed::Written, file),
            installed(Installed::Updated, file),
            installed(Installed::AlreadyThere, file),
            removed(Removed::Removed, file),
            removed(Removed::NothingThere, file),
        ];
        for text in &said {
            assert!(!text.contains("capri ") && !text.contains("--"), "{text}");
        }
        assert!(said[3].contains(file) && said[5].starts_with(file));
    }
}
