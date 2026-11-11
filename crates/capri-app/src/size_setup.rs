//! Sa taille, côté fichiers.
//!
//! `size.json`, dans le dossier de Capri, ne dit qu'une chose : `{ "scale": 0.8 }`, son échelle
//! rapportée à sa hauteur debout d'origine (`capri_core::scale`). Sans ce fichier, c'est la taille
//! d'origine. Il ne contient rien d'autre. Une valeur hors des bornes y est ramenée ; un fichier
//! illisible est signalé par l'appelant, qui prend alors la taille d'origine.

use std::path::Path;

use anyhow::{Context, Result, bail};
use capri_core::Scale;

use crate::brain_setup::read_text;

const SIZE_FILE: &str = "size.json";

/// Sa taille retenue. Sans fichier : la taille d'origine.
pub fn load(directory: &Path) -> Result<Scale> {
    let file = directory.join(SIZE_FILE);
    let Some(text) = read_text(&file)? else { return Ok(Scale::ORIGINAL) };
    parse(&text).with_context(|| format!("taille illisible ({})", file.display()))
}

/// Retient la taille choisie (dans son menu, ou d'un geste sur elle).
pub fn save(directory: &Path, scale: Scale) -> Result<std::path::PathBuf> {
    crate::brain_setup::write_file(directory, SIZE_FILE, &to_json(scale))
}

/// Un objet JSON ; `scale` absent vaut la taille d'origine, un champ inconnu est ignoré.
fn parse(text: &str) -> Result<Scale> {
    let value: serde_json::Value = serde_json::from_str(text).context("ce n'est pas du JSON")?;
    let Some(object) = value.as_object() else { bail!("ce n'est pas un objet JSON") };
    match object.get("scale") {
        None => Ok(Scale::ORIGINAL),
        Some(serde_json::Value::Number(number)) => match number.as_f64() {
            Some(scale) if scale.is_finite() => Ok(Scale::new(scale as f32)),
            _ => bail!("« scale » n'est pas un nombre"),
        },
        Some(_) => bail!("« scale » n'est pas un nombre"),
    }
}

fn to_json(scale: Scale) -> String {
    format!("{{\n  \"scale\": {:.2}\n}}\n", scale.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!("capri-test-size-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn the_original_size_by_default_and_remembered_from_one_launch_to_the_next() {
        let directory = temporary("retenue");
        assert_eq!(load(&directory).expect("sans fichier"), Scale::ORIGINAL);
        let file = save(&directory, Scale::new(0.7)).expect("écrite");
        assert_eq!(file, directory.join("size.json"));
        assert_eq!(std::fs::read_to_string(&file).expect("lue"), "{\n  \"scale\": 0.70\n}\n");
        assert_eq!(load(&directory).expect("relue"), Scale::new(0.7), "d'un lancement à l'autre");
        save(&directory, Scale::new(1.23)).expect("écrite");
        assert_eq!(load(&directory).expect("relue"), Scale::new(1.23));
        std::fs::remove_dir_all(&directory).expect("nettoyé");
    }

    #[test]
    fn a_strange_file_is_said_and_an_odd_number_brought_within_bounds() {
        assert_eq!(parse("{}").expect("vide"), Scale::ORIGINAL);
        assert_eq!(parse(r#"{"scale": 0.5, "autre": 3}"#).expect("champ inconnu ignoré"), Scale::new(0.5));
        // un nombre hors des bornes : ramené dedans, pas refusé
        assert_eq!(parse(r#"{"scale": 12}"#).expect("trop grand"), Scale::new(capri_core::scale::MAX));
        assert_eq!(parse(r#"{"scale": 0}"#).expect("trop petit"), Scale::new(capri_core::scale::MIN));
        for text in ["", "0.8", "[]", r#"{"scale": "grande"}"#, r#"{"scale": true}"#, r#"{"scale": null}"#] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}
