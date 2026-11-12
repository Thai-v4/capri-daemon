//! Chargement des images de Capri.
//!
//! Les images ne sont pas compilées dans le binaire : elles se remplacent sans recompiler
//! (voir CREDITS.md). On les cherche, dans l'ordre ([`candidates`]) :
//! 1. dans le dossier indiqué par la variable d'environnement `CAPRI_ASSETS` ;
//! 2. à côté de l'exécutable (`assets/`), ou dans `Contents/Resources/assets` d'une app macOS
//!    (celle que fabrique `tools/bundle-macos.sh`, qui les y copie) ;
//! 3. dans le dossier `assets/` du dépôt, pour le développement.
//!
//! Le premier dossier qui contient son dessin indispensable (`capri/capri-cutout.png`) l'emporte :
//! une app installée lit ses propres images, même si le dépôt d'où elle vient existe encore.

use std::ffi::OsString;
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

/// Le masque de clic d'une pose : l'union de ses dessins (un pixel compte s'il compte dans au
/// moins l'un d'eux), des cases de `cell` pixels prises dans leurs grilles (`grid`, de `width`
/// pixels de large, la case à `origin`). Dilaté comme celui de son dessin d'avant.
fn union_mask<'a>(cell: (u32, u32), cells: impl Iterator<Item = (&'a [u8], u32, (u32, u32))>) -> AlphaMask {
    let (cw, ch) = cell;
    let mut rgba = vec![0u8; (cw * ch * 4) as usize];
    for (grid, width, (gx, gy)) in cells {
        for y in 0..ch {
            for x in 0..cw {
                let alpha = grid[(((gy + y) * width + gx + x) * 4 + 3) as usize];
                let union = &mut rgba[((y * cw + x) * 4 + 3) as usize];
                *union = (*union).max(alpha);
            }
        }
    }
    AlphaMask::from_rgba(cw, ch, &rgba, ALPHA_THRESHOLD, GRAB_MARGIN)
}

/// Les planches de sa pose assise (`assets/capri/perch/`, voir CREDITS.md et
/// `capri_core::sheets`).
pub mod perch {
    use std::path::Path;

    use capri_core::AlphaMask;
    use capri_core::look::PerchMasks;
    use capri_core::sheets::{self, Strip};

    /// Une planche remise en grille (`sheets::repack`) : RGBA 8 bits non prémultiplié, de taille
    /// `strip.grid_size()` (`width` de large) ; la case `i` a son coin haut gauche en
    /// `strip.cell_origin(i)`.
    pub struct PerchStrip {
        pub strip: Strip,
        pub width: u32,
        pub rgba: Vec<u8>,
    }

    /// Toutes les planches de sa pose assise, chacune à la bonne taille.
    pub struct PerchSheets {
        /// Dans l'ordre de `Strip::ALL`.
        strips: Vec<PerchStrip>,
    }

    impl PerchSheets {
        pub fn get(&self, strip: Strip) -> &PerchStrip {
            // `load_from` les range toutes, dans l'ordre : la recherche trouve toujours
            self.strips.iter().find(|grid| grid.strip == strip).unwrap_or(&self.strips[0])
        }

        /// Les masques de clic de sa pose assise : l'union des quatre boucles (cases étroites),
        /// et celle de l'arrivée et du départ (cases larges).
        pub fn masks(&self) -> PerchMasks {
            let loops = [Strip::Perch, Strip::PerchBlink, Strip::PerchTalk, Strip::PerchMusic];
            PerchMasks {
                narrow: self.union(&loops),
                wide: self.union(&[Strip::PerchArrive, Strip::PerchLeave]),
            }
        }

        /// L'union des dessins de ces planches, qui ont toutes des cases de la même taille.
        fn union(&self, strips: &[Strip]) -> AlphaMask {
            let cells = strips.iter().flat_map(|&strip| {
                let grid = self.get(strip);
                (0..strip.count())
                    .map(move |index| (grid.rgba.as_slice(), grid.width, strip.cell_origin(index)))
            });
            super::union_mask(strips[0].cell(), cells)
        }
    }

    /// **Tolérant** : s'il en manque une, si l'une est illisible ou n'a pas la taille attendue,
    /// rend `None` (une ligne dans le journal, sans rien d'autre que le nom du fichier et des
    /// nombres), et elle se pose debout, comme avant. Environ 68 Mo en mémoire une fois lues et
    /// remises en grilles : à lâcher dès qu'elles sont confiées au GPU.
    pub fn load() -> Option<PerchSheets> {
        load_from(&super::assets_dir()?)
    }

    /// Ses planches assises sont-elles toutes là (sans les lire) ?
    #[cfg(debug_assertions)]
    pub fn present() -> bool {
        super::assets_dir().is_some_and(|dir| {
            Strip::ALL.iter().all(|strip| dir.join(sheets::DIR).join(strip.file()).is_file())
        })
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
                    let width = strip.grid_size().0;
                    strips.push(PerchStrip { strip, width, rgba });
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

/// Les planches de sa pose debout (`assets/capri/stand/`, voir CREDITS.md et
/// `capri_core::sheets::stand`), du même artiste que sa pose assise.
pub mod stand {
    use std::path::Path;

    use capri_core::AlphaMask;
    use capri_core::sheets::stand::{self, Strip};

    /// Une planche remise en grille (`stand::repack`) : RGBA 8 bits non prémultiplié, de taille
    /// `strip.grid_size()` (`width` de large) ; la case `i` a son coin haut gauche en
    /// `strip.cell_origin(i)`.
    pub struct StandStrip {
        pub strip: Strip,
        pub width: u32,
        pub rgba: Vec<u8>,
    }

    /// Les trois planches de sa pose debout, chacune à la bonne taille.
    pub struct StandSheets {
        /// Dans l'ordre de `Strip::ALL`.
        strips: Vec<StandStrip>,
    }

    impl StandSheets {
        pub fn get(&self, strip: Strip) -> &StandStrip {
            // `load_from` les range toutes, dans l'ordre : la recherche trouve toujours
            self.strips.iter().find(|grid| grid.strip == strip).unwrap_or(&self.strips[0])
        }

        /// Le masque de clic de sa pose debout : l'union de la boucle et du clignement (sa bouche
        /// reste dans son visage).
        pub fn mask(&self) -> AlphaMask {
            let cells = [Strip::Stand, Strip::StandBlink].into_iter().flat_map(|strip| {
                let grid = self.get(strip);
                (0..strip.count())
                    .map(move |index| (grid.rgba.as_slice(), grid.width, strip.cell_origin(index)))
            });
            super::union_mask(Strip::Stand.cell(), cells)
        }
    }

    /// **Tolérant** : s'il en manque une, si l'une est illisible ou n'a pas la taille attendue,
    /// rend `None` (une ligne dans le journal, sans rien d'autre que le nom du fichier et des
    /// nombres), et elle garde son dessin debout d'avant (`capri-cutout.png`). Environ 13 Mo en
    /// mémoire une fois lues (la boucle seule en fait 10,7) : à lâcher dès qu'elles sont confiées
    /// au GPU.
    pub fn load() -> Option<StandSheets> {
        load_from(&super::assets_dir()?)
    }

    /// Ses planches debout sont-elles toutes là (sans les lire) ?
    #[cfg(debug_assertions)]
    pub fn present() -> bool {
        super::assets_dir().is_some_and(|dir| {
            Strip::ALL.iter().all(|strip| dir.join(stand::DIR).join(strip.file()).is_file())
        })
    }

    pub(super) fn load_from(dir: &Path) -> Option<StandSheets> {
        let folder = dir.join(stand::DIR);
        let mut strips = Vec::with_capacity(Strip::ALL.len());
        for strip in Strip::ALL {
            let path = folder.join(strip.file());
            if !path.is_file() {
                log::info!("pose debout : {} absente, elle garde son dessin d'avant", strip.file());
                return None;
            }
            let image = match image::open(&path) {
                Ok(image) => image.to_rgba8(),
                Err(err) => {
                    log::warn!(
                        "pose debout : {} illisible ({err}), elle garde son dessin d'avant",
                        strip.file()
                    );
                    return None;
                }
            };
            let (width, height) = image.dimensions();
            match stand::repack(strip, width, height, image.as_raw()) {
                Ok(rgba) => {
                    let width = strip.grid_size().0;
                    strips.push(StandStrip { strip, width, rgba });
                }
                Err(wrong) => {
                    log::warn!(
                        "pose debout : {} mesure {} × {} au lieu de {} × {}, elle garde son dessin d'avant",
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
        log::info!("pose debout : {} planches", strips.len());
        Some(StandSheets { strips })
    }
}

fn assets_dir() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    first_with_sprite(candidates(std::env::var_os("CAPRI_ASSETS"), executable.as_deref(), &repository))
}

/// Les dossiers où chercher les images, dans l'ordre : celui de `CAPRI_ASSETS` (`variable` ; vide,
/// elle ne compte pas), puis `assets/` à côté de l'exécutable (`executable`), puis
/// `Contents/Resources/assets` de l'app qui le contient (pour `…/Capri.app/Contents/MacOS/capri`),
/// puis celui du dépôt (`repository`). Ne regarde pas s'ils existent.
fn candidates(variable: Option<OsString>, executable: Option<&Path>, repository: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::with_capacity(4);
    if let Some(dir) = variable.filter(|dir| !dir.is_empty()) {
        candidates.push(PathBuf::from(dir));
    }
    if let Some(beside) = executable.and_then(Path::parent) {
        candidates.push(beside.join("assets"));
        candidates.push(beside.join("..").join("Resources").join("assets"));
    }
    candidates.push(repository.to_owned());
    candidates
}

/// Le premier de ces dossiers qui contient son dessin indispensable ([`SPRITE`]).
fn first_with_sprite(candidates: Vec<PathBuf>) -> Option<PathBuf> {
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
            let (width, height) = strip.grid_size();
            assert_eq!(grid.width, width);
            assert_eq!(grid.rgba.len(), (width * height * 4) as usize);
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
        // rien hors de sa largeur mesurée, dans aucun dessin des quatre boucles
        let (from, to) = sheets::SPAN;
        for strip in [Strip::Perch, Strip::PerchBlink, Strip::PerchTalk, Strip::PerchMusic] {
            for index in 0..strip.count() {
                let cell = sheets::cell_pixels(strip, &sheets.get(strip).rgba, index).unwrap();
                let outside = (0..sheets::CELL_HEIGHT)
                    .flat_map(|y| (0..from).chain(to..sheets::NARROW).map(move |x| (x, y)))
                    .any(|(x, y)| cell[((y * sheets::NARROW + x) * 4 + 3) as usize] != 0);
                assert!(!outside, "{strip:?} {index}");
            }
        }
        // ses masques de clic : sur elle là où elle est assise, pas à côté
        let masks = sheets.masks();
        let uv = |x: u32, y: u32| capri_core::Vec2::new(x as f32 / 196.0, y as f32 / 384.0);
        assert!(masks.narrow.hit(uv(rest.0 + 10, seat - 2)), "sa main sur le bord");
        assert!(!masks.narrow.hit(uv(2, 10)), "le coin de la case");
        assert!((0.1..0.6).contains(&masks.narrow.coverage()), "{}", masks.narrow.coverage());
    }

    /// Où elle cherche ses images : l'ordre, puis de vrais dossiers.
    mod search {
        use std::ffi::OsString;
        use std::path::{Path, PathBuf};

        use super::super::{SPRITE, candidates, first_with_sprite};

        #[test]
        fn the_variable_then_beside_the_program_then_its_app_then_the_repository() {
            let program = Path::new("/Applications/Capri.app/Contents/MacOS/capri");
            let beside = Path::new("/Applications/Capri.app/Contents/MacOS");
            let repository = Path::new("/depot/capri-daemon/assets");
            assert_eq!(
                candidates(Some(OsString::from("/mes/images")), Some(program), repository),
                [
                    PathBuf::from("/mes/images"),
                    beside.join("assets"),
                    beside.join("..").join("Resources").join("assets"),
                    repository.to_owned()
                ]
            );
            // une variable vide ne compte pas : elle désignerait le dossier courant
            for variable in [None, Some(OsString::new())] {
                let found = candidates(variable, Some(program), repository);
                assert_eq!(found.len(), 3);
                assert_eq!(found[0], beside.join("assets"));
            }
            // sans exécutable connu, le dépôt seul
            assert_eq!(candidates(None, None, repository), [repository.to_owned()]);
        }

        /// Son dessin indispensable, dans le dossier d'images `dir`.
        fn sprite_in(dir: &Path) {
            let file = dir.join(SPRITE);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, b"une image").unwrap();
        }

        /// Le même fichier, quel que soit le chemin qui y mène (`MacOS/../Resources`).
        fn same_sprite(found: Option<PathBuf>, expected: &Path) -> bool {
            found.is_some_and(|dir| {
                dir.join(SPRITE).canonicalize().unwrap() == expected.join(SPRITE).canonicalize().unwrap()
            })
        }

        #[test]
        fn an_installed_app_reads_its_own_images_before_those_of_the_repository() {
            let base = std::env::temp_dir().join(format!("capri-test-recherche-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let app = base.join("Applications").join("Capri.app");
            let macos = app.join("Contents").join("MacOS");
            // `MacOS/..` ne se suit que si `MacOS` existe
            std::fs::create_dir_all(&macos).unwrap();
            let program = macos.join("capri");
            let bundled = app.join("Contents").join("Resources").join("assets");
            let repository = base.join("capri-daemon").join("assets");
            sprite_in(&bundled);
            sprite_in(&repository);

            let search = |variable: Option<&Path>| {
                first_with_sprite(candidates(variable.map(Into::into), Some(&program), &repository))
            };
            assert!(same_sprite(search(None), &bundled), "les siennes d'abord");
            // la variable l'emporte toujours, si son dossier a le dessin
            let mine = base.join("mes-images");
            sprite_in(&mine);
            assert_eq!(search(Some(&mine)), Some(mine));
            let empty = base.join("vide");
            std::fs::create_dir_all(&empty).unwrap();
            assert!(same_sprite(search(Some(&empty)), &bundled), "un dossier sans le dessin ne compte pas");
            // une app sans images : celles du dépôt
            std::fs::remove_dir_all(&bundled).unwrap();
            assert_eq!(search(None), Some(repository.clone()));
            // nulle part
            std::fs::remove_dir_all(&repository).unwrap();
            assert_eq!(search(None), None);
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    mod standing {
        use std::path::Path;

        use capri_core::AlphaMask;
        use capri_core::sheets::stand::{self, CELL_WIDTH, Strip};

        use super::super::stand as loader;
        use super::directory;

        fn write(dir: &Path, strip: Strip, (width, height): (u32, u32)) {
            std::fs::create_dir_all(dir.join(stand::DIR)).unwrap();
            let mut image = image::RgbaImage::new(width, height);
            image.put_pixel(0, 0, image::Rgba([0, 255, 0, 255]));
            image.save(dir.join(stand::DIR).join(strip.file())).unwrap();
        }

        #[test]
        fn without_every_sheet_she_keeps_her_old_drawing() {
            let dir = directory("debout-incompletes");
            assert!(loader::load_from(&dir).is_none(), "aucune planche");
            write(&dir, Strip::Stand, Strip::Stand.strip_size());
            write(&dir, Strip::StandBlink, Strip::StandBlink.strip_size());
            assert!(loader::load_from(&dir).is_none(), "il manque la bouche");
            write(&dir, Strip::StandMouth, (220, 420));
            assert!(loader::load_from(&dir).is_none(), "une planche de la mauvaise taille");
            std::fs::write(dir.join(stand::DIR).join(Strip::StandMouth.file()), b"pas une image").unwrap();
            assert!(loader::load_from(&dir).is_none(), "une planche illisible");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn complete_sheets_are_put_in_grids() {
            let dir = directory("debout-completes");
            for strip in Strip::ALL {
                write(&dir, strip, strip.strip_size());
            }
            let sheets = loader::load_from(&dir).expect("toutes les planches");
            for strip in Strip::ALL {
                let grid = sheets.get(strip);
                assert_eq!(grid.strip, strip);
                let (width, height) = strip.grid_size();
                assert_eq!(grid.width, width);
                assert_eq!(grid.rgba.len(), (width * height * 4) as usize);
                assert_eq!(grid.rgba[..4], [0, 255, 0, 255], "la première case reste en haut à gauche");
            }
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Les vraies planches, si `tools/fetch-assets.sh` les a téléchargées (rien n'est
        /// téléchargé ici) : les mesures de `capri_core::sheets::stand` doivent leur correspondre.
        #[test]
        fn the_real_sheets_match_their_measures_when_present() {
            let Some(dir) = super::super::assets_dir() else { return };
            if !Strip::ALL.iter().all(|strip| dir.join(stand::DIR).join(strip.file()).is_file()) {
                return;
            }
            let sheets = loader::load_from(&dir).expect("les vraies planches ont la taille décrite");
            let cell =
                |strip: Strip, index: u32| stand::cell_pixels(strip, &sheets.get(strip).rgba, index).unwrap();
            let alpha = |pixels: &[u8], x: u32, y: u32| pixels[((y * CELL_WIDTH + x) * 4 + 3) as usize];
            let row_empty = |pixels: &[u8], y: u32| (0..CELL_WIDTH).all(|x| alpha(pixels, x, y) == 0);

            let first = cell(Strip::Stand, 0);
            let (mut tops, mut lowest) = (Vec::new(), 0);
            for index in 0..Strip::Stand.count() {
                let pixels = cell(Strip::Stand, index);
                let top = (0..stand::BOTTOM).find(|&y| !row_empty(&pixels, y)).unwrap();
                let bottom = (0..stand::BOTTOM).rev().find(|&y| !row_empty(&pixels, y)).unwrap();
                tops.push(top);
                lowest = lowest.max(bottom);
                // rien sous la pointe de son pied
                assert!((stand::BOTTOM..384).all(|y| row_empty(&pixels, y)), "dessin {index}");
                // ses pieds, comme le masque de clic les verra
                let mask = AlphaMask::from_rgba(CELL_WIDTH, 384, &pixels, 96, 0);
                let feet = mask.feet();
                assert!((feet - stand::feet_uv()).length() < 1e-4, "dessin {index} : {feet:?}");
                // pas de flottement : de la taille aux pieds, le dessin 0 décalé d'une ligne ou
                // plus lui ressemble moins que le dessin 0 tel quel
                let moved = |dy: i32| {
                    (190..stand::BOTTOM as i32 - 4)
                        .flat_map(|y| (0..CELL_WIDTH).map(move |x| (x, y)))
                        .filter(|&(x, y)| {
                            (alpha(&pixels, x, y as u32) > 96) != (alpha(&first, x, (y + dy) as u32) > 96)
                        })
                        .count()
                };
                let still = moved(0);
                for dy in [-3, -2, -1, 1, 2, 3] {
                    assert!(
                        still < moved(dy),
                        "dessin {index} : décalé de {dy} lignes, il ressemble plus au dessin 0"
                    );
                }
            }
            assert_eq!(lowest + 1, stand::BOTTOM);
            assert_eq!(tops.iter().min(), Some(&stand::TOP));
            assert_eq!(tops.iter().max(), Some(&(stand::TOP + stand::HAIR_DROP)));

            // la bouche ne dessine rien hors de sa boîte
            let ((x0, y0), (x1, y1)) = stand::MOUTH_BOX;
            for index in 0..Strip::StandMouth.count() {
                let pixels = cell(Strip::StandMouth, index);
                for y in 0..384 {
                    for x in 0..CELL_WIDTH {
                        if !(x0..=x1).contains(&x) || !(y0..=y1).contains(&y) {
                            assert_eq!(alpha(&pixels, x, y), 0, "bouche {index} en ({x}, {y})");
                        }
                    }
                }
            }
        }
    }
}
