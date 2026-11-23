//! Une version de Capri : `MAJEUR.MINEUR.CORRECTIF`, des chiffres seulement.
//!
//! L'écriture est canonique (pas de zéro en tête, sauf « 0 » lui-même) : l'adresse d'un fichier
//! d'une version est calculée à partir d'elle (`v<version>`), et doit retomber exactement sur
//! l'étiquette publiée. « 01.2.3 » est donc refusée plutôt que lue comme « 1.2.3 ».

use std::fmt;

/// Le nombre de chiffres d'un composant, au plus : bien assez, et jamais de dépassement.
const MAX_DIGITS: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    // l'ordre des champs fait l'ordre des versions (dérivé) : majeur, puis mineur, puis correctif
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Lit `MAJEUR.MINEUR.CORRECTIF`. Refuse tout le reste : un composant vide, autre chose que
    /// des chiffres ASCII (ni signe, ni espace, ni suffixe `-beta`, ni préfixe `v`), un zéro en
    /// tête, plus de [`MAX_DIGITS`] chiffres, plus ou moins de trois composants.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.');
        let mut next = || parts.next().and_then(component);
        let version = Self::new(next()?, next()?, next()?);
        parts.next().is_none().then_some(version)
    }
}

fn component(text: &str) -> Option<u32> {
    let canonical = !text.is_empty()
        && text.len() <= MAX_DIGITS
        && text.bytes().all(|b| b.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    canonical.then(|| text.parse().ok()).flatten()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_three_plain_numbers_written_once_and_for_all() {
        assert_eq!(Version::parse("0.2.0"), Some(Version::new(0, 2, 0)));
        assert_eq!(Version::parse("10.0.123"), Some(Version::new(10, 0, 123)));
        assert_eq!(Version::parse("999999999.0.0"), Some(Version::new(999_999_999, 0, 0)));
        // l'écriture relue est celle d'origine : c'est elle qui fait l'étiquette `v<version>`
        for text in ["0.0.0", "0.2.0", "1.10.3", "999999999.999999999.999999999"] {
            assert_eq!(Version::parse(text).expect(text).to_string(), text);
        }
        for refused in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "1..3",
            ".1.2",
            "1.2.",
            "v1.2.3",
            "1.2.3-beta",
            "1.2.3+1",
            " 1.2.3",
            "1.2.3 ",
            "+1.2.3",
            "-1.2.3",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "00.0.0",
            "1.2.x",
            "１.2.3",
            "1000000000.0.0",
            "4294967296.0.0",
            "1,2,3",
            "1.2.3\n",
        ] {
            assert_eq!(Version::parse(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn versions_are_ordered_number_by_number_not_as_text() {
        let v = |text| Version::parse(text).expect(text);
        assert!(v("0.10.0") > v("0.9.0"), "10 après 9, même si « 1 » vient avant « 9 »");
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("0.2.1") > v("0.2.0"));
        assert!(v("0.2.0") == v("0.2.0"));
        assert!(v("2.0.0") > v("1.999.999"));
    }
}
