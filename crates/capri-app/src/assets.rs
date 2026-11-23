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
use capri_core::sheets::WrongSize;

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

/// Une planche facultative, lue et remise en grille par `repack` ; `None` si elle est absente,
/// illisible ou n'a pas la taille attendue (une ligne au journal : `what`, le nom du fichier, des
/// nombres, et ce que son absence change, `without`). Jamais une panne.
fn optional(
    folder: &Path,
    file: &str,
    what: &str,
    without: &str,
    repack: impl FnOnce(u32, u32, &[u8]) -> Result<Vec<u8>, WrongSize>,
) -> Option<Vec<u8>> {
    let path = folder.join(file);
    if !path.is_file() {
        log::info!("{what} : {file} absente, {without}");
        return None;
    }
    let image = match image::open(&path) {
        Ok(image) => image.to_rgba8(),
        Err(err) => {
            log::warn!("{what} : {file} illisible ({err}), {without}");
            return None;
        }
    };
    let (width, height) = image.dimensions();
    match repack(width, height, image.as_raw()) {
        Ok(rgba) => Some(rgba),
        Err(wrong) => {
            log::warn!(
                "{what} : {file} mesure {} × {} au lieu de {} × {}, {without}",
                wrong.found.0,
                wrong.found.1,
                wrong.expected.0,
                wrong.expected.1
            );
            None
        }
    }
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

    /// Toutes les planches de sa pose assise, chacune à la bonne taille ; et celles des planches
    /// facultatives qui sont là.
    pub struct PerchSheets {
        /// Dans l'ordre de `Strip::SET`.
        strips: Vec<PerchStrip>,
        /// Parmi `Strip::EXTRA`, celles qui sont là.
        extras: Vec<PerchStrip>,
    }

    impl PerchSheets {
        /// Une planche de `Strip::SET`.
        pub fn get(&self, strip: Strip) -> &PerchStrip {
            // `load_from` les range toutes, dans l'ordre : la recherche trouve toujours
            self.strips.iter().find(|grid| grid.strip == strip).unwrap_or(&self.strips[0])
        }

        /// Une planche facultative (`Strip::EXTRA`), si elle est là.
        pub fn extra(&self, strip: Strip) -> Option<&PerchStrip> {
            self.extras.iter().find(|grid| grid.strip == strip)
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
            Strip::SET.iter().all(|strip| dir.join(sheets::DIR).join(strip.file()).is_file())
        })
    }

    /// Ce que l'absence d'une planche facultative change, pour le journal.
    fn without(strip: Strip) -> &'static str {
        match strip {
            Strip::PerchGlitch => "assise, pas d'éclat",
            _ => "assise, ses clignements d'avant",
        }
    }

    pub(super) fn load_from(dir: &Path) -> Option<PerchSheets> {
        let folder = dir.join(sheets::DIR);
        let mut strips = Vec::with_capacity(Strip::SET.len());
        for strip in Strip::SET {
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
        let extras: Vec<PerchStrip> = Strip::EXTRA
            .into_iter()
            .filter_map(|strip| {
                let repack = |width, height, rgba: &[u8]| sheets::repack(strip, width, height, rgba);
                let rgba = super::optional(&folder, strip.file(), "pose assise", without(strip), repack)?;
                Some(PerchStrip { strip, width: strip.grid_size().0, rgba })
            })
            .collect();
        log::info!("pose assise : {} planches, et {} facultatives", strips.len(), extras.len());
        Some(PerchSheets { strips, extras })
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

    /// Les trois planches de sa pose debout, chacune à la bonne taille ; et la variante de son
    /// clignement, si elle est là.
    pub struct StandSheets {
        /// Dans l'ordre de `Strip::SET`.
        strips: Vec<StandStrip>,
        /// Parmi `Strip::EXTRA`, celles qui sont là.
        extras: Vec<StandStrip>,
    }

    impl StandSheets {
        /// Une planche de `Strip::SET`.
        pub fn get(&self, strip: Strip) -> &StandStrip {
            // `load_from` les range toutes, dans l'ordre : la recherche trouve toujours
            self.strips.iter().find(|grid| grid.strip == strip).unwrap_or(&self.strips[0])
        }

        /// Une planche facultative (`Strip::EXTRA`), si elle est là.
        pub fn extra(&self, strip: Strip) -> Option<&StandStrip> {
            self.extras.iter().find(|grid| grid.strip == strip)
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
            Strip::SET.iter().all(|strip| dir.join(stand::DIR).join(strip.file()).is_file())
        })
    }

    pub(super) fn load_from(dir: &Path) -> Option<StandSheets> {
        let folder = dir.join(stand::DIR);
        let mut strips = Vec::with_capacity(Strip::SET.len());
        for strip in Strip::SET {
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
        let extras: Vec<StandStrip> = Strip::EXTRA
            .into_iter()
            .filter_map(|strip| {
                let repack = |width, height, rgba: &[u8]| stand::repack(strip, width, height, rgba);
                let without = "debout, ses clignements d'avant";
                let rgba = super::optional(&folder, strip.file(), "pose debout", without, repack)?;
                Some(StandStrip { strip, width: strip.grid_size().0, rgba })
            })
            .collect();
        log::info!("pose debout : {} planches, et {} facultatives", strips.len(), extras.len());
        Some(StandSheets { strips, extras })
    }
}

/// Les planches de sa vie (`assets/capri/life/`, voir CREDITS.md et `capri_core::sheets::life`) :
/// elle se matérialise, elle s'efface (les deux ensemble), et ses gestes, chacun à part : elle
/// marche, elle pend, elle se relève, elle sourit. Du même artiste que ses poses.
pub mod life {
    use std::path::Path;

    use capri_core::AlphaMask;
    use capri_core::sheets::life::{self, Strip};

    /// Une planche remise en grille (`life::repack`) : RGBA 8 bits non prémultiplié, de taille
    /// `strip.grid_size()` (`width` de large) ; la case `i` a son coin haut gauche en
    /// `strip.cell_origin(i)`.
    pub struct LifeStrip {
        pub strip: Strip,
        pub width: u32,
        pub rgba: Vec<u8>,
    }

    /// Celles des planches de sa vie qui sont là, chacune à la bonne taille.
    pub struct LifeSheets {
        /// Dans l'ordre de `Strip::ALL`, sans celles qui manquent.
        strips: Vec<LifeStrip>,
    }

    impl LifeSheets {
        /// Cette planche, si elle est là.
        pub fn get(&self, strip: Strip) -> Option<&LifeStrip> {
            self.strips.iter().find(|grid| grid.strip == strip)
        }

        /// Son arrivée et son départ sont-ils là tous les deux ? Sinon, elle apparaît et
        /// disparaît d'un coup.
        pub fn appears(&self) -> bool {
            Strip::APPEAR.iter().all(|&strip| self.get(strip).is_some())
        }

        /// Le masque de clic d'une planche : l'union de ses dessins (pour son arrivée et son départ,
        /// ceux de son arrivée seulement : quand elle s'efface, rien ne l'attrape). `None` si elle
        /// n'est pas là.
        pub fn mask(&self, strip: Strip) -> Option<AlphaMask> {
            let strip = if strip == Strip::Leave { Strip::Arrive } else { strip };
            let grid = self.get(strip)?;
            let cells =
                (0..strip.count()).map(|index| (grid.rgba.as_slice(), grid.width, strip.cell_origin(index)));
            Some(super::union_mask(strip.cell(), cells))
        }
    }

    /// Ce que l'absence d'une planche change, pour le journal.
    fn without(strip: Strip) -> &'static str {
        match strip {
            Strip::Arrive | Strip::Leave => "elle apparaîtra et disparaîtra d'un coup",
            Strip::Walk => "elle changera de place sans marcher",
            Strip::Dangle => "tenue, elle gardera sa boucle debout",
            Strip::Glitch => "lâchée, elle ne se relèvera pas",
            Strip::Grin => "elle ne sourira pas",
        }
    }

    /// **Tolérant**, planche par planche : une absente, illisible ou qui n'a pas la taille
    /// attendue est laissée de côté (une ligne dans le journal, sans rien d'autre que le nom du
    /// fichier, des nombres et ce que son absence change) ; `None` s'il n'y en a aucune. Environ
    /// 10 Mo en mémoire pour l'arrivée et le départ, 23 Mo pour les quatre autres, une fois lues :
    /// à lâcher dès qu'elles sont confiées au GPU.
    pub fn load() -> Option<LifeSheets> {
        load_from(&super::assets_dir()?)
    }

    pub(super) fn load_from(dir: &Path) -> Option<LifeSheets> {
        let folder = dir.join(life::DIR);
        let strips: Vec<LifeStrip> = Strip::ALL
            .into_iter()
            .filter_map(|strip| {
                let repack = |width, height, rgba: &[u8]| life::repack(strip, width, height, rgba);
                let rgba = super::optional(&folder, strip.file(), "planches de vie", without(strip), repack)?;
                Some(LifeStrip { strip, width: strip.grid_size().0, rgba })
            })
            .collect();
        log::info!("planches de vie : {} sur {}", strips.len(), Strip::ALL.len());
        (!strips.is_empty()).then_some(LifeSheets { strips })
    }
}

/// L'intro du lancement (`assets/capri/intro/`, voir CREDITS.md et `capri_core::intro`) : de
/// grandes images, **jamais lues toutes ensemble**. Elles sont vérifiées d'abord (toutes là, et
/// toutes à la bonne taille, d'après leur seul en-tête), puis décodées une à une, à la demande, par
/// le thread de `intro_view`.
pub mod intro {
    use std::path::{Path, PathBuf};

    use capri_core::intro::{self, Missing};

    /// Ses images : où, combien, de quelle taille.
    pub struct Sequence {
        pub dir: PathBuf,
        pub count: u32,
        pub size: (u32, u32),
    }

    impl Sequence {
        /// La vraie intro, dans le dossier d'images `assets` (celui qui contient `capri/`).
        pub fn real(assets: &Path) -> Self {
            Self { dir: assets.join(intro::DIR), count: intro::COUNT, size: intro::SIZE }
        }

        fn path(&self, index: u32) -> PathBuf {
            self.dir.join(intro::file(index))
        }

        /// Toutes là, et toutes de la taille attendue ? D'après leur en-tête seulement : rien
        /// n'est décodé ici. Le journal n'en dira que des nombres (`intro_view`).
        pub fn check(&self) -> Result<(), Missing> {
            let found = (0..self.count).filter(|&index| self.path(index).is_file()).count() as u32;
            if found < self.count {
                return Err(Missing::Absent { found });
            }
            for index in 0..self.count {
                let size = image::ImageReader::open(self.path(index))
                    .ok()
                    .and_then(|reader| reader.into_dimensions().ok());
                match size {
                    None => return Err(Missing::Unreadable { index }),
                    Some(found) if found != self.size => return Err(Missing::WrongSize { index, found }),
                    Some(_) => {}
                }
            }
            Ok(())
        }

        /// L'image n° `index`, décodée : RGBA 8 bits non prémultiplié, de la taille attendue ;
        /// `None` si elle est illisible (ou d'une autre taille que celle de son en-tête).
        pub fn decode(&self, index: u32) -> Option<Vec<u8>> {
            let image = image::ImageReader::open(self.path(index)).ok()?.decode().ok()?;
            // sans copie si elle est déjà en RGBA 8 bits (le cas de ces images à fond transparent)
            let image = image.into_rgba8();
            (image.dimensions() == self.size).then(|| image.into_raw())
        }
    }

    /// Ses images, dans le premier dossier d'images trouvé ; `None` s'il n'y en a aucun.
    pub fn sequence() -> Option<Sequence> {
        Some(Sequence::real(&super::assets_dir()?))
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
        for strip in Strip::SET {
            write(&dir, strip, strip.strip_size());
        }
        // ses planches facultatives : l'une de la mauvaise taille, l'autre absente ; sa pose
        // assise est là quand même
        write(&dir, Strip::PerchGlitch, (220, 420));
        let sheets = perch::load_from(&dir).expect("toutes les planches nécessaires");
        assert!(Strip::EXTRA.iter().all(|&strip| sheets.extra(strip).is_none()));
        for strip in Strip::EXTRA {
            write(&dir, strip, strip.strip_size());
        }
        let sheets = perch::load_from(&dir).expect("toutes les planches");
        for strip in Strip::EXTRA {
            let grid = sheets.extra(strip).expect("là");
            assert_eq!((grid.strip, grid.width), (strip, strip.grid_size().0));
            assert_eq!(grid.rgba[..4], [255, 0, 0, 255], "la première case reste en haut à gauche");
        }
        for strip in Strip::SET {
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
        if !Strip::SET.iter().all(|strip| dir.join(sheets::DIR).join(strip.file()).is_file()) {
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

        /// `tools/bundle-macos.sh` copie dans `Capri.app` les images que lit le code, et
        /// seulement elles : sa liste `IMAGES` suit le code.
        #[test]
        fn the_app_bundle_carries_exactly_the_images_the_code_reads() {
            use capri_core::intro;
            use capri_core::sheets::{self, life, stand};

            let script = include_str!("../../../tools/bundle-macos.sh");
            let mut listed: Vec<&str> = script
                .lines()
                .map(str::trim)
                .skip_while(|line| *line != "IMAGES=(")
                .skip(1)
                .take_while(|line| *line != ")")
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .collect();
            let mut read = vec![SPRITE.to_owned()];
            read.extend(sheets::Strip::ALL.iter().map(|strip| format!("{}/{}", sheets::DIR, strip.file())));
            read.extend(stand::Strip::ALL.iter().map(|strip| format!("{}/{}", stand::DIR, strip.file())));
            read.extend(life::Strip::ALL.iter().map(|strip| format!("{}/{}", life::DIR, strip.file())));
            read.extend((0..intro::COUNT).map(|index| format!("{}/{}", intro::DIR, intro::file(index))));
            listed.sort_unstable();
            read.sort_unstable();
            assert_eq!(listed, read);
        }

        #[test]
        fn the_app_icon_is_named_alike_by_the_plist_and_the_script_and_is_no_image_the_code_reads() {
            let plist = include_str!("../../../tools/macos/Info.plist");
            let script = include_str!("../../../tools/bundle-macos.sh");
            let after_key =
                plist.split("<key>CFBundleIconFile</key>").nth(1).expect("la clé CFBundleIconFile");
            let declared = after_key
                .split("<string>")
                .nth(1)
                .and_then(|rest| rest.split("</string>").next())
                .expect("son nom");
            assert!(declared.ends_with(".icns"), "{declared}");
            assert!(
                script.contains(&format!("\"$APP/Contents/Resources/{declared}\"")),
                "le script ne fabrique pas {declared} dans Contents/Resources"
            );
            // elle est tirée de capri-face.png, que le code ne lit pas : jamais dans IMAGES
            assert!(script.contains("FACE=\"$ASSETS/capri/capri-face.png\""));
            let images: Vec<&str> = script
                .lines()
                .map(str::trim)
                .skip_while(|line| *line != "IMAGES=(")
                .skip(1)
                .take_while(|line| *line != ")")
                .collect();
            assert!(images.iter().all(|line| !line.contains("capri-face")), "{images:?}");
        }

        /// Signer `Capri.app` ne sort jamais de la machine, avec ou sans identité de signature :
        /// chaque signature du script refuse l'horodatage par un serveur, et aucun des deux
        /// scripts ne connaît un outil qui téléverserait l'app chez Apple (la notariser serait la
        /// distribuer, avec ses images).
        #[test]
        fn signing_the_app_bundle_never_leaves_the_machine() {
            let bundle = include_str!("../../../tools/bundle-macos.sh");
            let install = include_str!("../../../tools/install-macos.sh");
            let code = |script: &'static str| {
                script.lines().map(str::trim).filter(|line| !line.starts_with('#')).collect::<Vec<_>>()
            };
            let signings: Vec<&str> =
                code(bundle).into_iter().filter(|line| line.contains("--sign ")).collect();
            assert!(!signings.is_empty(), "le script ne signe plus l'app ?");
            for line in signings {
                assert!(line.contains("--timestamp=none"), "une signature horodatée par un serveur : {line}");
            }
            assert!(
                code(install).iter().all(|line| !line.contains("--sign ")),
                "l'installation ne signe pas"
            );
            for line in code(bundle).into_iter().chain(code(install)) {
                for tool in ["notarytool", "altool", "stapler"] {
                    assert!(!line.contains(tool), "{tool} téléverserait l'app : {line}");
                }
            }
        }
    }

    /// Les lignes d'un script ou d'un workflow, sans les commentaires ni les blancs.
    fn code_lines(text: &str) -> Vec<&str> {
        text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect()
    }

    /// La CI et la publication ne fabriquent, ne signent ni ne joignent jamais `Capri.app`, ses
    /// images ou son icône : aucun de leurs gestes ne nomme ce qui les porterait.
    #[test]
    fn the_workflows_never_touch_the_app_its_images_or_a_signature() {
        let ci = include_str!("../../../.github/workflows/ci.yml");
        let release = include_str!("../../../.github/workflows/release.yml");
        for (name, text) in [("ci.yml", ci), ("release.yml", release)] {
            for line in code_lines(text) {
                for banned in [
                    "fetch-assets",
                    "bundle-macos",
                    "install-macos",
                    "codesign",
                    "assets/",
                    "Capri.app",
                    ".icns",
                    "secrets.",
                ] {
                    assert!(!line.contains(banned), "{name} nomme {banned} : {line}");
                }
            }
        }
        // seul le job de publication peut écrire dans le dépôt
        let writes: Vec<&str> =
            code_lines(release).into_iter().filter(|line| line.contains("contents: write")).collect();
        assert_eq!(writes.len(), 1, "{writes:?}");
        assert!(code_lines(ci).iter().all(|line| !line.contains("write")), "ci.yml n'écrit nulle part");
    }

    /// Le contrat de la publication : les noms de fichiers de la version sont ceux du workflow
    /// et ceux du script du manifeste, qui écrit aussi les champs que l'app lira.
    #[test]
    fn the_release_files_are_named_alike_by_the_workflow_and_the_manifest_script() {
        let release = include_str!("../../../.github/workflows/release.yml");
        let script = include_str!("../../../tools/release-manifest.sh");
        let workflow = code_lines(release).join("\n");
        let manifest = code_lines(script).join("\n");
        let assets = [
            "capri-macos-arm64",
            "capri-hook-macos-arm64",
            "capri-windows-x86_64.exe",
            "capri-hook-windows-x86_64.exe",
            "Info.plist",
            "latest.json",
        ];
        for asset in assets {
            assert!(workflow.contains(asset), "release.yml ne nomme pas {asset}");
        }
        // le manifeste décrit les cinq fichiers, sous leur nom installé
        for (name, asset) in [
            ("capri", "capri-macos-arm64"),
            ("capri-hook", "capri-hook-macos-arm64"),
            ("Info.plist", "Info.plist"),
            ("capri.exe", "capri-windows-x86_64.exe"),
            ("capri-hook.exe", "capri-hook-windows-x86_64.exe"),
        ] {
            assert!(manifest.contains(&format!("file_entry {name} {asset}\n")), "{name} {asset}");
        }
        for field in [
            "\"format\"",
            "\"version\"",
            "\"bundle\"",
            "\"targets\"",
            "\"macos-arm64\"",
            "\"windows-x86_64\"",
            "\"files\"",
            "\"name\"",
            "\"asset\"",
            "\"size\"",
            "\"sha256\"",
        ] {
            assert!(manifest.contains(field), "le script n'écrit pas {field}");
        }
        // le manifeste est fabriqué par le script, avec le numéro de bundle du dépôt
        assert!(workflow.contains("tools/release-manifest.sh release"));
        assert!(workflow.contains("tools/macos/bundle-format"));
        // une version se publie en brouillon complet, jamais comme préversion
        assert!(workflow.contains("--draft"));
        assert!(workflow.contains("--prerelease=false"));
        assert!(!workflow.contains("--prerelease\n") && !workflow.contains("--prerelease "));
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
            for strip in Strip::SET {
                write(&dir, strip, strip.strip_size());
            }
            // sans la variante de son clignement : sa pose debout quand même
            let sheets = loader::load_from(&dir).expect("toutes les planches nécessaires");
            assert!(sheets.extra(Strip::StandBlinkGlitch).is_none());
            std::fs::write(dir.join(stand::DIR).join(Strip::StandBlinkGlitch.file()), b"pas une image")
                .unwrap();
            let sheets = loader::load_from(&dir).expect("une variante illisible ne change rien");
            assert!(sheets.extra(Strip::StandBlinkGlitch).is_none());
            write(&dir, Strip::StandBlinkGlitch, Strip::StandBlinkGlitch.strip_size());
            let sheets = loader::load_from(&dir).expect("toutes les planches");
            let glitchy = sheets.extra(Strip::StandBlinkGlitch).expect("là");
            assert_eq!(glitchy.rgba.len(), (Strip::StandBlinkGlitch.grid_size().0 * 384 * 4) as usize);
            for strip in Strip::SET {
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
            if !Strip::SET.iter().all(|strip| dir.join(stand::DIR).join(strip.file()).is_file()) {
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

    /// Les vraies images de l'intro, si `tools/fetch-assets.sh` les a téléchargées (rien n'est
    /// téléchargé ici) : toutes là, à la taille décrite, en RGBA 8 bits (décodées sans copie), à
    /// fond transparent. Seules la première et la dernière sont décodées : les 22 seraient lentes
    /// en version de débogage.
    #[test]
    fn the_real_intro_matches_its_description_when_present() {
        use capri_core::intro;

        let Some(dir) = super::assets_dir() else { return };
        let sequence = super::intro::Sequence::real(&dir);
        if !(0..intro::COUNT).all(|index| sequence.dir.join(intro::file(index)).is_file()) {
            return;
        }
        assert_eq!(sequence.check(), Ok(()));
        for index in [0, intro::COUNT - 1] {
            let path = sequence.dir.join(intro::file(index));
            let decoded = image::ImageReader::open(&path).unwrap().decode().unwrap();
            assert_eq!(decoded.color(), image::ColorType::Rgba8, "image {index}");
            let pixels = sequence.decode(index).expect("lisible");
            assert_eq!(pixels.len(), (intro::SIZE.0 * intro::SIZE.1 * 4) as usize);
        }
        let last = sequence.decode(intro::COUNT - 1).expect("lisible");
        assert!(last.as_chunks::<4>().0.iter().any(|pixel| pixel[3] == 0), "à fond transparent");
    }

    mod alive {
        use std::path::Path;

        use capri_core::sheets::life::{self, CELL_WIDTH, Strip};
        use capri_core::sheets::stand;

        use super::super::life as loader;
        use super::super::stand as stand_loader;
        use super::directory;

        fn write(dir: &Path, strip: Strip, (width, height): (u32, u32)) {
            std::fs::create_dir_all(dir.join(life::DIR)).unwrap();
            let mut image = image::RgbaImage::new(width, height);
            image.put_pixel(0, 0, image::Rgba([0, 0, 255, 255]));
            image.save(dir.join(life::DIR).join(strip.file())).unwrap();
        }

        #[test]
        fn without_every_sheet_she_comes_and_goes_at_once() {
            let dir = directory("vie-incompletes");
            assert!(loader::load_from(&dir).is_none(), "aucune planche");
            write(&dir, Strip::Arrive, Strip::Arrive.strip_size());
            let alone = loader::load_from(&dir).expect("son arrivée seule");
            assert!(!alone.appears(), "il manque le départ");
            write(&dir, Strip::Leave, (220, 420));
            assert!(!loader::load_from(&dir).unwrap().appears(), "une planche de la mauvaise taille");
            std::fs::write(dir.join(life::DIR).join(Strip::Leave.file()), b"pas une image").unwrap();
            assert!(!loader::load_from(&dir).unwrap().appears(), "une planche illisible");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn each_move_is_loaded_on_its_own() {
            let dir = directory("vie-gestes");
            // sa planche en l'air seule, sans son arrivée ni son départ : elle est là quand même
            write(&dir, Strip::Dangle, Strip::Dangle.strip_size());
            write(&dir, Strip::Walk, (220, 420));
            std::fs::write(dir.join(life::DIR).join(Strip::Grin.file()), b"pas une image").unwrap();
            let sheets = loader::load_from(&dir).expect("une planche");
            assert!(!sheets.appears());
            assert!(sheets.get(Strip::Dangle).is_some() && sheets.mask(Strip::Dangle).is_some());
            for strip in [Strip::Walk, Strip::Glitch, Strip::Grin, Strip::Arrive, Strip::Leave] {
                assert!(sheets.get(strip).is_none() && sheets.mask(strip).is_none(), "{strip:?}");
            }
            let mask = sheets.mask(Strip::Dangle).unwrap();
            assert_eq!(mask.size(), (CELL_WIDTH, 384));
            assert!(mask.hit(capri_core::Vec2::new(0.0, 0.0)), "la marque du coin de sa première case");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Une planche transparente de la taille donnée, un pixel plein à chacun des points
        /// `marks` (en pixels de la bande).
        fn write_marked(dir: &Path, strip: Strip, (width, height): (u32, u32), marks: &[(u32, u32)]) {
            std::fs::create_dir_all(dir.join(life::DIR)).unwrap();
            let mut image = image::RgbaImage::new(width, height);
            for &(x, y) in marks {
                image.put_pixel(x, y, image::Rgba([0, 0, 255, 255]));
            }
            image.save(dir.join(life::DIR).join(strip.file())).unwrap();
        }

        #[test]
        fn complete_sheets_are_put_in_grids_and_only_her_arrival_catches_her() {
            // des marques à des places différentes dans les deux planches : dans l'arrivée, le coin
            // de sa première case et le milieu de sa dernière ; dans le départ, deux autres points
            let last = (Strip::Arrive.count() - 1) * CELL_WIDTH;
            let arrive = [(0, 0), (last + 120, 200)];
            let leave = [(200, 300), (5 * CELL_WIDTH + 60, 100)];
            let dir = directory("vie-completes");
            write_marked(&dir, Strip::Arrive, Strip::Arrive.strip_size(), &arrive);
            write_marked(&dir, Strip::Leave, Strip::Leave.strip_size(), &leave);
            let sheets = loader::load_from(&dir).expect("toutes les planches");
            assert!(sheets.appears());
            for (strip, (x, y)) in [(Strip::Arrive, arrive[0]), (Strip::Leave, leave[0])] {
                let grid = sheets.get(strip).expect("là");
                assert_eq!(grid.strip, strip);
                let (width, height) = strip.grid_size();
                assert_eq!(grid.width, width);
                assert_eq!(grid.rgba.len(), (width * height * 4) as usize);
                let at = ((y * width + x) * 4) as usize;
                assert_eq!(
                    grid.rgba[at..at + 4],
                    [0, 0, 255, 255],
                    "{strip:?} : la première case reste en haut à gauche"
                );
            }
            // le masque : l'union de toutes les cases de son arrivée, et seulement d'elle (celui de
            // son départ aussi : quand elle s'efface, rien d'autre ne l'attrape)
            let mask = sheets.mask(Strip::Arrive).expect("là");
            let leaving = sheets.mask(Strip::Leave).expect("là");
            for (x, y) in arrive.into_iter().chain(leave) {
                let uv = capri_core::Vec2::new((x % CELL_WIDTH) as f32 / CELL_WIDTH as f32, y as f32 / 384.0);
                assert_eq!(mask.hit(uv), leaving.hit(uv));
            }
            assert_eq!(mask.size(), (CELL_WIDTH, 384));
            let uv = |x: u32, y: u32| {
                capri_core::Vec2::new((x % CELL_WIDTH) as f32 / CELL_WIDTH as f32, y as f32 / 384.0)
            };
            for (x, y) in arrive {
                assert!(mask.hit(uv(x, y)), "la marque de l'arrivée en ({x}, {y})");
            }
            for (x, y) in leave {
                assert!(!mask.hit(uv(x, y)), "la marque du départ en ({x}, {y}) ne l'attrape pas");
            }
            assert!(!mask.hit(capri_core::Vec2::new(0.5, 0.5)), "rien ailleurs");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Les colonnes pleines (alpha > 96) de la dernière ligne qui en a : le point le plus bas
        /// de sa silhouette, (milieu, ligne).
        fn lowest(pixels: &[u8], width: u32) -> (f32, u32) {
            let full = |x: u32, y: u32| pixels[((y * width + x) * 4 + 3) as usize] > 96;
            let row = (0..384).rev().find(|&y| (0..width).any(|x| full(x, y))).expect("une silhouette");
            let columns: Vec<u32> = (0..width).filter(|&x| full(x, row)).collect();
            let (first, last) = (columns[0], columns[columns.len() - 1]);
            ((first + last + 1) as f32 / 2.0, row)
        }

        /// Les lignes dessinées (alpha > 0), de la première à la dernière.
        fn rows(pixels: &[u8], width: u32) -> (u32, u32) {
            let drawn = |y: u32| (0..width).any(|x| pixels[((y * width + x) * 4 + 3) as usize] > 0);
            let top = (0..384).find(|&y| drawn(y)).expect("une silhouette");
            let bottom = (0..384).rev().find(|&y| drawn(y)).expect("une silhouette");
            (top, bottom)
        }

        /// Le centre du masque vert de ses yeux, lignes 90 à 139 (voir `life::EYES`).
        fn eyes(pixels: &[u8], width: u32) -> (f32, f32) {
            let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
            for y in 90..140 {
                for x in 0..width {
                    let p = &pixels[((y * width + x) * 4) as usize..][..4];
                    if p[3] > 200 && p[1] > 150 && p[0] < 140 && p[2] < 190 {
                        (sx, sy, n) = (sx + x as f32, sy + y as f32, n + 1.0);
                    }
                }
            }
            (sx / n, sy / n)
        }

        /// Les vraies planches, si `tools/fetch-assets.sh` les a téléchargées (rien n'est
        /// téléchargé ici) : les mesures de `capri_core::sheets::life` doivent leur correspondre,
        /// et la case de sa boucle debout, décalée de `life::STAND_LEFT`, doit mettre ses pieds sur
        /// ceux de la fin de son arrivée, à la même hauteur.
        #[test]
        fn the_real_life_sheets_match_their_measures_when_present() {
            let Some(dir) = super::super::assets_dir() else { return };
            let present = |folder: &str, file: &str| dir.join(folder).join(file).is_file();
            if !Strip::APPEAR.iter().all(|strip| present(life::DIR, strip.file()))
                || !stand::Strip::ALL.iter().all(|strip| present(stand::DIR, strip.file()))
            {
                return;
            }
            let sheets = loader::load_from(&dir).expect("les vraies planches ont la taille décrite");
            let standing = stand_loader::load_from(&dir).expect("les vraies planches debout");
            let cell = |strip: Strip, index: u32| {
                life::cell_pixels(strip, &sheets.get(strip).expect("là").rgba, index).unwrap()
            };
            let loop_cell = |index: u32| {
                stand::cell_pixels(stand::Strip::Stand, &standing.get(stand::Strip::Stand).rgba, index)
                    .unwrap()
            };
            let last = Strip::Arrive.count() - 1;
            let end = cell(Strip::Arrive, last);
            assert_eq!(
                end,
                cell(Strip::Arrive, last - 1),
                "les deux derniers dessins de l'arrivée sont les mêmes"
            );

            // ses pieds : le point le plus bas, sur la même ligne, au demi-pixel près une fois décalé
            let (x, row) = lowest(&end, CELL_WIDTH);
            assert_eq!((x, row), life::LOWEST);
            for index in [0, 58] {
                let (sx, srow) = lowest(&loop_cell(index), stand::CELL_WIDTH);
                assert_eq!((sx, srow), life::STAND_LOWEST, "dessin {index} de la boucle");
            }
            for index in (0..stand::Strip::Stand.count()).step_by(2) {
                let (sx, srow) = lowest(&loop_cell(index), stand::CELL_WIDTH);
                assert_eq!(srow, row, "dessin {index} : ses pieds sur la même ligne");
                assert!((sx + life::STAND_LEFT as f32 - x).abs() <= 1.0, "dessin {index} : {sx}");
            }
            // sa hauteur : la même dernière ligne, le haut de ses cheveux à 2 px près
            assert_eq!(rows(&end, CELL_WIDTH), (life::TOP, life::BOTTOM - 1));
            assert_eq!(rows(&loop_cell(0), stand::CELL_WIDTH), (stand::TOP, stand::BOTTOM - 1));
            // ses yeux le confirment
            let (ex, ey) = eyes(&end, CELL_WIDTH);
            let (sx, sy) = eyes(&loop_cell(0), stand::CELL_WIDTH);
            assert!((ex - life::EYES.0).abs() < 0.1 && (ey - life::EYES.1).abs() < 0.1, "({ex}, {ey})");
            assert!(
                (sx - life::STAND_EYES.0).abs() < 0.1 && (sy - life::STAND_EYES.1).abs() < 0.1,
                "({sx}, {sy})"
            );

            // le début de l'arrivée et la fin du départ : un trait, puis plus rien
            let (top, bottom) = rows(&cell(Strip::Arrive, 0), CELL_WIDTH);
            assert!(bottom - top < 16, "un trait : lignes {top} à {bottom}");
            let leave_last = cell(Strip::Leave, Strip::Leave.count() - 1);
            assert!(
                leave_last.as_chunks::<4>().0.iter().all(|p| p[3] == 0),
                "le dernier dessin du départ est vide"
            );
            // le départ commence là où l'arrivée finit : ses yeux au même endroit, au pixel près
            let (lx, ly) = eyes(&cell(Strip::Leave, 0), CELL_WIDTH);
            assert!((lx - ex).abs() < 1.0 && (ly - ey).abs() < 1.0, "({lx}, {ly})");

            // le masque de son arrivée l'attrape sur elle, pas dans le coin
            let mask = sheets.mask(Strip::Arrive).expect("là");
            let uv = |x: f32, y: f32| capri_core::Vec2::new(x / CELL_WIDTH as f32, y / 384.0);
            assert!(mask.hit(uv(life::EYES.0, life::EYES.1)), "son visage");
            assert!(!mask.hit(uv(2.0, 380.0)), "le coin de la case");
        }

        /// Les pixels pleins (alpha > 96) d'une case de `width` px de large.
        fn solid(pixels: &[u8], width: u32) -> std::collections::HashSet<(i32, i32)> {
            (0..384u32)
                .flat_map(|y| (0..width).map(move |x| (x, y)))
                .filter(|&(x, y)| pixels[((y * width + x) * 4 + 3) as usize] > 96)
                .map(|(x, y)| (x as i32, y as i32))
                .collect()
        }

        /// Combien de pixels pleins diffèrent entre `a` et `b` décalé de `dx`.
        fn apart(
            a: &std::collections::HashSet<(i32, i32)>,
            b: &std::collections::HashSet<(i32, i32)>,
            dx: i32,
        ) -> usize {
            let moved: std::collections::HashSet<(i32, i32)> = b.iter().map(|&(x, y)| (x + dx, y)).collect();
            a.symmetric_difference(&moved).count()
        }

        /// Le pied le plus bas parmi les colonnes `columns` : (milieu, ligne), alpha > 96.
        fn foot(pixels: &[u8], columns: std::ops::Range<u32>) -> (f32, u32) {
            let full = |x: u32, y: u32| pixels[((y * CELL_WIDTH + x) * 4 + 3) as usize] > 96;
            let row = (0..384).rev().find(|&y| columns.clone().any(|x| full(x, y))).expect("un pied");
            let hit: Vec<u32> = columns.filter(|&x| full(x, row)).collect();
            ((hit[0] + hit[hit.len() - 1] + 1) as f32 / 2.0, row)
        }

        /// Les vraies planches de ses gestes, si `tools/fetch-assets.sh` les a téléchargées (rien
        /// n'est téléchargé ici) : les mesures de `capri_core::sheets::life`, `sheets` et
        /// `sheets::stand` doivent leur correspondre. Elles s'alignent sur sa pose debout comme son
        /// arrivée (ses yeux au même endroit) ; sa marche se fait sur place, l'écart de ses pieds
        /// au sol est un pas, et elle regarde vers la gauche ; tenue en l'air, la mèche par où on la
        /// tient est au même endroit dans tous ses dessins ; ses variantes prennent la place des
        /// dessins qu'elles remplacent.
        #[test]
        fn the_real_moves_match_their_measures_when_present() {
            use capri_core::sheets::{self as perch, life::Facing};

            let Some(dir) = super::super::assets_dir() else { return };
            let present = |folder: &str, file: &str| dir.join(folder).join(file).is_file();
            let moves_here = Strip::ALL.iter().all(|strip| present(life::DIR, strip.file()))
                && perch::Strip::ALL.iter().all(|strip| present(perch::DIR, strip.file()))
                && stand::Strip::ALL.iter().all(|strip| present(stand::DIR, strip.file()));
            if !moves_here {
                return;
            }
            let sheets = loader::load_from(&dir).expect("les vraies planches ont la taille décrite");
            assert!(Strip::ALL.iter().all(|&strip| sheets.get(strip).is_some()));
            let cell = |strip: Strip, index: u32| {
                life::cell_pixels(strip, &sheets.get(strip).expect("là").rgba, index).unwrap()
            };

            // sur la toile de son arrivée : ses yeux au même endroit, au pixel près
            for (strip, index) in [(Strip::Walk, 0), (Strip::Glitch, 9), (Strip::Grin, 0), (Strip::Grin, 15)]
            {
                let (x, y) = eyes(&cell(strip, index), CELL_WIDTH);
                assert!(
                    (x - life::EYES.0).abs() < 1.5 && (y - life::EYES.1).abs() < 1.5,
                    "{strip:?} {index} : ({x}, {y})"
                );
            }
            // le glitch finit sur la fin de son arrivée, à quelques pixels près ; son sourire aussi
            let end = solid(&cell(Strip::Arrive, 13), CELL_WIDTH);
            for (strip, index, most) in [(Strip::Glitch, 9, 0.02), (Strip::Grin, 0, 0.08)] {
                let differ = apart(&solid(&cell(strip, index), CELL_WIDTH), &end, 0);
                assert!((differ as f64) < most * end.len() as f64, "{strip:?} {index} : {differ}");
            }

            // sa marche : sur place. Le pied posé ne bouge pas pendant que l'autre se lève
            let (left, right) = (0..145, 145..CELL_WIDTH);
            for index in 1..=5 {
                let (x, _) = foot(&cell(Strip::Walk, index), right.clone());
                assert!((164.0..=166.5).contains(&x), "dessin {index} : pied droit en {x}");
            }
            for index in 7..=11 {
                let (x, _) = foot(&cell(Strip::Walk, index), left.clone());
                assert!((127.0..=130.0).contains(&x), "dessin {index} : pied gauche en {x}");
            }
            // un pas : l'écart de ses pieds quand tous deux touchent le sol, aux mêmes places aux
            // dessins 0 et 6 (le pied levé retombe là d'où il est parti)
            for index in [0, 6] {
                let contact = cell(Strip::Walk, index);
                let ((lx, ly), (rx, ry)) = (foot(&contact, left.clone()), foot(&contact, right.clone()));
                assert_eq!((lx, ly, rx, ry), (126.5, 363, 168.0, 362), "dessin {index}");
                assert_eq!(rx - lx, life::WALK_STEP);
            }
            // elle regarde vers la gauche : dans le masque vert de son visage, son œil gauche (le
            // plus loin, en trois quarts) est plus petit que l'autre et collé au bord du masque
            let pixels = cell(Strip::Walk, 0);
            let (mut mask, mut near, mut far) = (Vec::new(), Vec::new(), Vec::new());
            for y in 110..135 {
                for x in 110..160 {
                    let p = &pixels[((y * CELL_WIDTH + x) * 4) as usize..][..4];
                    if p[3] > 200 && p[1] > 150 && p[0] < 140 && p[2] < 190 {
                        mask.push(x);
                    }
                    if p[3] > 200 && p[2] > 150 && p[0] < 120 && i32::from(p[2]) > i32::from(p[1]) + 20 {
                        if x < 132 { far.push(x) } else { near.push(x) }
                    }
                }
            }
            let (left, right) = (mask.iter().min().copied().unwrap(), mask.iter().max().copied().unwrap());
            let (far_edge, near_edge) =
                (far.iter().min().copied().unwrap(), near.iter().max().copied().unwrap());
            assert!(far.len() < near.len(), "son œil gauche {} px, le droit {} px", far.len(), near.len());
            assert!(
                far_edge - left < right - near_edge,
                "son œil gauche à {} px du bord du masque, le droit à {} px",
                far_edge - left,
                right - near_edge
            );
            assert_eq!(life::FACING, Facing::Left);

            // tenue en l'air : la mèche commune à ses 24 dessins, en haut
            let all: Vec<_> = (0..Strip::Dangle.count())
                .map(|index| solid(&cell(Strip::Dangle, index), CELL_WIDTH))
                .collect();
            let common: Vec<(i32, i32)> = all[0]
                .iter()
                .filter(|&&(x, y)| (20..41).contains(&y) && all.iter().all(|cells| cells.contains(&(x, y))))
                .cloned()
                .collect();
            let n = common.len() as f32;
            let grip = (
                common.iter().map(|&(x, _)| x as f32).sum::<f32>() / n,
                common.iter().map(|&(_, y)| y as f32).sum::<f32>() / n,
            );
            assert_eq!(common.len(), 119);
            assert!((grip.0 - life::GRIP.0).abs() < 0.1 && (grip.1 - life::GRIP.1).abs() < 0.1, "{grip:?}");
            assert_eq!(common.iter().map(|&(_, y)| y).min(), Some(28));
            // sept dessins identiques à d'autres
            for (a, b) in [(0, 12), (4, 16), (5, 17), (6, 18), (8, 20), (9, 21), (11, 23)] {
                assert_eq!(cell(Strip::Dangle, a), cell(Strip::Dangle, b), "{a} et {b}");
            }

            // son éclat assise : la même case large que son arrivée, finissant sur son dessin 0
            let perched = super::super::perch::load_from(&dir).expect("sa pose assise");
            let glitch = perched.extra(perch::Strip::PerchGlitch).expect("son éclat");
            let last =
                solid(&perch::cell_pixels(perch::Strip::PerchGlitch, &glitch.rgba, 9).unwrap(), perch::WIDE);
            let seated = solid(
                &perch::cell_pixels(perch::Strip::Perch, &perched.get(perch::Strip::Perch).rgba, 0).unwrap(),
                perch::NARROW,
            );
            let at = |dx: i32| apart(&last, &seated, dx);
            let shift = perch::WIDE_SHIFT as i32;
            assert!(
                at(shift) < at(shift - 6) && at(shift) < at(shift + 6),
                "{} {} {}",
                at(shift - 6),
                at(shift),
                at(shift + 6)
            );
            assert!((at(shift) as f64) < 0.15 * last.len() as f64, "{}", at(shift));
            // ses clignements glitchés : chaque case la plus proche du dessin qu'elle remplace
            let closest = |pixels: &[u8], width: u32, of: &dyn Fn(u32) -> Vec<u8>, count: u32| {
                let cells = solid(pixels, width);
                (0..count).min_by_key(|&index| apart(&cells, &solid(&of(index), width), 0)).unwrap()
            };
            let glitchy = perched.extra(perch::Strip::PerchBlinkGlitch).expect("son clignement glitché");
            let sitting = |index| {
                perch::cell_pixels(perch::Strip::Perch, &perched.get(perch::Strip::Perch).rgba, index)
                    .unwrap()
            };
            for index in 0..8 {
                let pixels =
                    perch::cell_pixels(perch::Strip::PerchBlinkGlitch, &glitchy.rgba, index).unwrap();
                assert_eq!(
                    closest(&pixels, perch::NARROW, &sitting, 60),
                    perch::BLINK_FROM + index,
                    "assise, case {index}"
                );
            }
            let standing = super::super::stand::load_from(&dir).expect("sa pose debout");
            let glitchy = standing.extra(stand::Strip::StandBlinkGlitch).expect("son clignement glitché");
            let idle = |index| {
                stand::cell_pixels(stand::Strip::Stand, &standing.get(stand::Strip::Stand).rgba, index)
                    .unwrap()
            };
            for index in 0..8 {
                let pixels =
                    stand::cell_pixels(stand::Strip::StandBlinkGlitch, &glitchy.rgba, index).unwrap();
                assert_eq!(
                    closest(&pixels, stand::CELL_WIDTH, &idle, 60),
                    stand::BLINK_FROM + index,
                    "debout, case {index}"
                );
            }

            // leurs masques de clic : sur elle, pas dans le coin
            for strip in Strip::MOVES {
                let mask = sheets.mask(strip).expect("là");
                assert!(
                    mask.hit(capri_core::Vec2::new(life::EYES.0 / 240.0, life::EYES.1 / 384.0)),
                    "{strip:?}"
                );
                assert!(!mask.hit(capri_core::Vec2::new(2.0 / 240.0, 380.0 / 384.0)), "{strip:?}");
            }
        }
    }
}
