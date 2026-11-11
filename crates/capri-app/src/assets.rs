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

/// Les planches de sa pose assise (`assets/capri/perch/`, voir CREDITS.md et
/// `capri_core::sheets`).
pub mod perch {
    #![allow(dead_code, reason = "branché par le lot qui la fera s'asseoir avec ces planches")]

    use std::path::Path;

    use capri_core::sheets::{self, Strip};

    /// Une planche remise en grille (`sheets::repack`) : RGBA 8 bits non prémultiplié, de taille
    /// `strip.grid_size()` ; la case `i` a son coin haut gauche en `strip.cell_origin(i)`.
    pub struct PerchStrip {
        pub strip: Strip,
        pub width: u32,
        pub height: u32,
        pub rgba: Vec<u8>,
    }

    /// Toutes les planches de sa pose assise, chacune à la bonne taille.
    pub struct PerchSheets {
        /// Dans l'ordre de `Strip::ALL`.
        strips: Vec<PerchStrip>,
    }

    impl PerchSheets {
        pub fn get(&self, strip: Strip) -> &PerchStrip {
            let index = Strip::ALL.iter().position(|&s| s == strip).unwrap_or(0);
            &self.strips[index]
        }
    }

    /// **Tolérant** : s'il en manque une, si l'une est illisible ou n'a pas la taille attendue,
    /// rend `None` (une ligne dans le journal, sans rien d'autre que le nom du fichier et des
    /// nombres), et elle se pose avec son dessin debout, comme avant. Environ 70 Mo en mémoire une
    /// fois lues : à lâcher dès qu'elles sont confiées au GPU.
    pub fn load() -> Option<PerchSheets> {
        load_from(&super::assets_dir()?)
    }

    pub(super) fn load_from(dir: &Path) -> Option<PerchSheets> {
        let folder = dir.join(sheets::DIR);
        let mut strips = Vec::with_capacity(Strip::ALL.len());
        for strip in Strip::ALL {
            let path = folder.join(strip.file());
            if !path.is_file() {
                log::info!("pose assise : {} absente, elle reste debout sur les fenêtres", strip.file());
                return None;
            }
            let image = match image::open(&path) {
                Ok(image) => image.to_rgba8(),
                Err(err) => {
                    log::warn!("pose assise : {} illisible ({err}), elle reste debout", strip.file());
                    return None;
                }
            };
            let (width, height) = image.dimensions();
            match sheets::repack(strip, width, height, image.as_raw()) {
                Ok(rgba) => {
                    let (width, height) = strip.grid_size();
                    strips.push(PerchStrip { strip, width, height, rgba });
                }
                Err(wrong) => {
                    log::warn!(
                        "pose assise : {} mesure {} × {} au lieu de {} × {}, elle reste debout",
                        strip.file(),
                        wrong.found.0,
                        wrong.found.1,
                        wrong.expected.0,
                        wrong.expected.1
                    );
                    return None;
                }
            }
        }
        log::info!("pose assise : {} planches", strips.len());
        Some(PerchSheets { strips })
    }
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use capri_core::sheets::{self, Strip};

    use super::perch;

    fn directory(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("capri-test-planches-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(sheets::DIR)).unwrap();
        dir
    }

    /// Une planche transparente de la taille donnée, son coin haut gauche marqué d'un pixel plein.
    fn write(dir: &Path, strip: Strip, (width, height): (u32, u32)) {
        let mut image = image::RgbaImage::new(width, height);
        image.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        image.save(dir.join(sheets::DIR).join(strip.file())).unwrap();
    }

    #[test]
    fn without_every_sheet_she_stays_standing() {
        let dir = directory("incompletes");
        assert!(perch::load_from(&dir).is_none(), "aucune planche");
        for strip in &Strip::ALL[..5] {
            write(&dir, *strip, strip.strip_size());
        }
        assert!(perch::load_from(&dir).is_none(), "il en manque une");
        write(&dir, Strip::PerchLeave, (220, 420));
        assert!(perch::load_from(&dir).is_none(), "une planche de la mauvaise taille");
        std::fs::write(dir.join(sheets::DIR).join(Strip::PerchLeave.file()), b"pas une image").unwrap();
        assert!(perch::load_from(&dir).is_none(), "une planche illisible");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_sheets_are_put_in_grids() {
        let dir = directory("completes");
        for strip in Strip::ALL {
            write(&dir, strip, strip.strip_size());
        }
        let sheets = perch::load_from(&dir).expect("toutes les planches");
        for strip in Strip::ALL {
            let grid = sheets.get(strip);
            assert_eq!(grid.strip, strip);
            assert_eq!((grid.width, grid.height), strip.grid_size());
            assert_eq!(grid.rgba.len(), (grid.width * grid.height * 4) as usize);
            assert_eq!(grid.rgba[..4], [255, 0, 0, 255], "la première case reste en haut à gauche");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Les vraies planches, si `tools/fetch-assets.sh` les a téléchargées (rien n'est téléchargé
    /// ici) : la description de `capri_core::sheets` doit leur correspondre.
    #[test]
    fn the_real_sheets_match_their_description_when_present() {
        let Some(dir) = super::assets_dir() else { return };
        if !Strip::ALL.iter().all(|strip| dir.join(sheets::DIR).join(strip.file()).is_file()) {
            return;
        }
        let sheets = perch::load_from(&dir).expect("les vraies planches ont la taille décrite");
        // sur la ligne du bord, l'image 0 de l'assise est pleine sous sa main et sous son genou,
        // et vide au-delà
        let perch = sheets.get(Strip::Perch);
        let alpha = |x: u32, y: u32| perch.rgba[((y * perch.width + x) * 4 + 3) as usize];
        let (rest, seat) = (sheets::REST, sheets::SEAT_Y);
        assert!(alpha(rest.0 + 3, seat) > 96 && alpha(rest.1 - 3, seat) > 96);
        assert!(alpha(rest.0 - 6, seat) == 0 && alpha(rest.1 + 6, seat) == 0);
        // rien sous ce qui pend
        assert!((0..sheets::NARROW).all(|x| alpha(x, seat + sheets::HANG) == 0));
    }
}
