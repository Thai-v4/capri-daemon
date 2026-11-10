//! Chargement des images de Capri.
//!
//! Les images ne sont pas compilées dans le binaire : elles se remplacent sans recompiler
//! (voir CREDITS.md). On les cherche, dans l'ordre :
//! 1. dans le dossier indiqué par la variable d'environnement `CAPRI_ASSETS` ;
//! 2. à côté de l'exécutable (`assets/`), ou dans `Contents/Resources/assets` d'une app macOS ;
//! 3. dans le dossier `assets/` du dépôt, pour le développement.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use capri_core::AlphaMask;

/// Le sprite principal : le détourage d'origine, 220 × 420 px.
const SPRITE: &str = "capri/capri-cutout.png";
/// Seuil d'alpha au-dessus duquel un pixel « compte » pour attraper Capri.
const ALPHA_THRESHOLD: u8 = 96;
/// Marge (en pixels du sprite) autour de sa silhouette pour attraper ses membres fins.
const GRAB_MARGIN: u32 = 3;

pub struct Sprite {
    pub width: u32,
    pub height: u32,
    /// RGBA 8 bits non prémultiplié.
    pub rgba: Vec<u8>,
    pub mask: AlphaMask,
}

pub fn load_sprite() -> Result<Sprite> {
    let dir = assets_dir().context(
        "images de Capri introuvables. Lance d'abord `tools/fetch-assets.sh` (macOS) ou \
         `tools\\fetch-assets.ps1` (Windows), ou indique leur dossier avec CAPRI_ASSETS.",
    )?;
    let path = dir.join(SPRITE);
    let image = image::open(&path).with_context(|| format!("lecture de {}", path.display()))?.to_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    let mask = AlphaMask::from_rgba(width, height, &rgba, ALPHA_THRESHOLD, GRAB_MARGIN);
    if mask.coverage() > 0.95 {
        bail!("{} n'a pas de transparence : ce n'est pas le bon fichier", path.display());
    }
    log::info!("sprite : {} ({width}×{height})", path.display());
    Ok(Sprite { width, height, rgba, mask })
}

fn assets_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("CAPRI_ASSETS") {
        candidates.push(PathBuf::from(dir));
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        candidates.push(exe_dir.join("assets"));
        candidates.push(exe_dir.join("../Resources/assets"));
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    candidates.into_iter().find(|dir| dir.join(SPRITE).is_file())
}
