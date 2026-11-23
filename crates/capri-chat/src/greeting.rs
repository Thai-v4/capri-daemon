//! Son bonjour, après l'intro du lancement : une courte phrase, à elle, dans sa bulle (et à voix
//! haute si sa voix est activée).
//!
//! **Écrites par nous**, d'après sa personnalité ([`crate::persona::DEFAULT`] : vive, directe, un
//! peu taquine, au tutoiement, sans rien supposer de la personne en face, pas même son genre) ;
//! **jamais une citation de la série**. Aucun modèle n'est appelé pour la dire, et elle n'entre
//! pas dans la conversation envoyée à son cerveau (`capri-app` la dit par le même chemin que la
//! ligne du miroir).
//!
//! **Laquelle** : tirée au sort avec la graine de sa vie ([`pick`]), une nouvelle à chaque
//! lancement. Rien n'est retenu d'un lancement à l'autre : deux lancements de suite peuvent tomber
//! sur la même, une fois sur [`GREETINGS`]`.len()`.
//!
//! **Quand** ([`Hello`]) : attendu dès que l'intro commence ; dit une fois l'intro jouée, quand elle
//! est là et libre. Lui parler, la faire taire, repartir de zéro ou la cacher le fait oublier, que
//! ce soit pendant l'intro ou après : il n'y aura pas de bonjour (l'intro, elle, continue).

use capri_core::roam::Rng;

/// Ses bonjours : courts (une bulle, une respiration), en texte brut, sans points de suspension
/// (sa voix les dirait mal).
pub const GREETINGS: [&str; 10] = [
    "Coucou ! Je ne regardais pas, promis.",
    "Me revoilà. Tu m'as manqué, un peu.",
    "Salut ! On s'y remet ?",
    "Hop, me voilà. Qu'est-ce qu'on fait aujourd'hui ?",
    "C'est moi ! J'ai pris la meilleure place.",
    "Bonjour ! Ton écran a l'air calme. Pour l'instant.",
    "Me voilà. Fais comme si je n'étais pas là.",
    "Salut toi. Je garde un œil sur ton bureau.",
    "Coucou ! Tu as vu ma plus belle grimace ?",
    "Ça y est, je suis là. On commence quand tu veux.",
];

/// Son bonjour pour la graine `seed` (celle de sa vie, prise au lancement) : la même graine donne
/// toujours le même.
pub fn pick(seed: u64) -> &'static str {
    // une graine à part de celle de ses trajets : le premier tirage de sa vie ne décide pas des deux
    let mut rng = Rng::new(seed ^ 0xb0_2b_0a_ce_1a_11_c0_de);
    let index = (rng.unit() * GREETINGS.len() as f64) as usize;
    GREETINGS[index.min(GREETINGS.len() - 1)]
}

/// Son bonjour, du début de l'intro à ce qu'elle le dise (ou pas).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hello {
    /// L'intro se joue : il viendra après elle, si rien ne le fait oublier entre-temps.
    Expected(&'static str),
    /// L'intro s'est jouée : il attend qu'elle soit là et libre.
    Waiting(&'static str),
    /// Dit, oublié, ou pas de bonjour du tout (pas d'intro).
    #[default]
    Done,
}

impl Hello {
    /// L'intro commence : son bonjour, `line`, viendra après.
    pub fn expect(line: &'static str) -> Self {
        Hello::Expected(line)
    }

    /// L'intro est finie ; `greets` : elle s'est jouée (`capri_core::intro::End::greets`). Seul un
    /// bonjour encore attendu se met à attendre son tour ; sinon, pas de bonjour.
    pub fn intro_over(&mut self, greets: bool) {
        *self = match *self {
            Hello::Expected(line) if greets => Hello::Waiting(line),
            _ => Hello::Done,
        };
    }

    /// Un geste qui le fait oublier (lui parler, la faire taire, repartir de zéro, la cacher),
    /// pendant l'intro comme après.
    pub fn forget(&mut self) {
        *self = Hello::Done;
    }

    /// Le bonjour à dire maintenant, si elle est libre (`free` : là, personne ne lui parle, elle ne
    /// parle pas) ; rendu une fois, il est dit.
    pub fn take(&mut self, free: bool) -> Option<&'static str> {
        match *self {
            Hello::Waiting(line) if free => {
                *self = Hello::Done;
                Some(line)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn her_greetings_are_short_plain_and_never_guess_who_she_talks_to() {
        for line in GREETINGS {
            let length = line.chars().count();
            assert!((8..=60).contains(&length), "trop long ou trop court pour sa bulle : {line}");
            assert_eq!(line.trim(), line);
            assert!(line.ends_with(['.', '!', '?']), "{line}");
            // du texte brut, sans points de suspension
            for mark in ["…", "...", "*", "_", "`", "#", "  "] {
                assert!(!line.contains(mark), "{mark} dans : {line}");
            }
            // au tutoiement, sans rien supposer de la personne (pas d'accord au masculin ni au féminin)
            let words: Vec<String> =
                line.split(|c: char| !c.is_alphabetic() && c != '\'').map(str::to_lowercase).collect();
            for word in ["vous", "votre", "vos", "prêt", "prête", "content", "contente", "seul", "seule"] {
                assert!(!words.iter().any(|w| w == word), "{word} dans : {line}");
            }
        }
        let mut unique = GREETINGS.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), GREETINGS.len(), "pas deux fois la même");
    }

    #[test]
    fn she_says_hello_once_after_the_intro_when_free() {
        let mut hello = Hello::expect(GREETINGS[0]);
        assert_eq!(hello.take(true), None, "pas pendant l'intro");
        hello.intro_over(true);
        assert_eq!(hello.take(false), None, "elle se matérialise encore : il attend");
        assert_eq!(hello.take(true), Some(GREETINGS[0]));
        assert_eq!(hello.take(true), None, "une fois");
        // l'intro coupée, ou pas d'intro : pas de bonjour
        let mut hello = Hello::expect(GREETINGS[1]);
        hello.intro_over(false);
        assert_eq!(hello.take(true), None);
        let mut hello = Hello::default();
        hello.intro_over(true);
        assert_eq!(hello.take(true), None, "sans intro, sans bonjour");
    }

    #[test]
    fn hushed_or_forgotten_during_the_intro_she_does_not_say_hello_after_it() {
        // « La faire taire » ou « Nouvelle conversation » pendant l'intro, qui continue
        let mut hello = Hello::expect(GREETINGS[2]);
        hello.forget();
        hello.intro_over(true);
        assert_eq!(hello.take(true), None);
        // ou une fois l'intro finie, avant qu'elle ait pu le dire
        let mut hello = Hello::expect(GREETINGS[2]);
        hello.intro_over(true);
        hello.forget();
        assert_eq!(hello.take(true), None);
    }

    #[test]
    fn the_seed_of_her_life_picks_one_and_every_one_comes_up() {
        assert_eq!(pick(42), pick(42), "la même graine, le même bonjour");
        let mut seen = std::collections::HashSet::new();
        for seed in 0..1000u64 {
            seen.insert(pick(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        }
        assert_eq!(seen.len(), GREETINGS.len(), "chacun finit par venir");
        // des graines proches (deux lancements à quelques nanosecondes) n'en donnent pas qu'un
        let close: std::collections::HashSet<_> =
            (0..50u64).map(|n| pick(1_763_900_000_000_000_000 + n)).collect();
        assert!(close.len() > 3, "{close:?}");
    }
}
