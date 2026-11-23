//! Ses dessins, côté application : son dessin d'avant et ses planches, confiés une fois pour
//! toutes à la carte graphique puis lâchés, les masques de clic de chaque pose, et ce que
//! `capri_core::look` décide de montrer, traduit en dessins pour `capri-render`.
//!
//! **Le repli** : une planche qui manque, illisible ou que la carte graphique refuse, et la pose
//! qu'elle porte est remplacée par son dessin d'avant (`capri-cutout.png`), comme avant ; seul
//! son dessin d'avant est indispensable. Ses planches facultatives (ses gestes, sa marche, tenue en
//! l'air, les variantes de ses clignements, `capri_core::look::Moves`) sont confiées une à une :
//! une refusée, et seul ce qu'elle porte ne se joue pas. Celles de sa vie ne servent qu'avec sa
//! pose debout, sur laquelle elles s'alignent, et ses variantes assises qu'avec sa pose assise.
//!
//! **Ce que ça pèse** : les planches lues et remises en grilles font 81 Mo en mémoire, plus 10 Mo
//! pour son arrivée et son départ, 23 Mo pour ses quatre gestes de 240 px et 8 Mo pour ses trois
//! planches facultatives assises et debout ; elles ne sont gardées que le temps de faire les
//! masques de clic et de les confier à la carte graphique. Seuls les dessins
//! montrés y vont (`sheets::shown` : un sur deux pour les boucles et les clignements, tenus « en
//! deux » ; tous pour les suites jouées une fois) : 47 Mo de textures (RGBA 8 bits, sans mipmaps),
//! plus 9,6 Mo pour son arrivée et son départ, et 29,2 Mo pour les sept planches de ses gestes
//! (sa marche 4,4 Mo, tenue en l'air 8,8, se relever 3,7, sourire 5,9, son éclat assise 4,4, ses
//! clignements glitchés 1,2 et 0,7), qui comptent dans la mémoire de l'app sur un Mac à puce Apple
//! (mémoire unifiée). Les masques de clic, eux, restent : un booléen par pixel d'une case de chaque
//! pose et de chaque planche de vie, 690 Ko en tout.

use capri_core::look::{Drawing, Look, Moves, Picture, Wardrobe};
use capri_core::sheets::{self, Shown, life, stand};
use capri_render::{Crop, GHOST, ImageId, RenderError, SpriteDraw, SpriteImage};

use crate::assets::Sprite;
use crate::assets::life::LifeSheets;
use crate::assets::perch::PerchSheets;
use crate::assets::stand::StandSheets;

/// Amplitude du « boil » de son dessin d'avant, en pixels de cette image (420 px de haut).
const BOIL: f32 = 1.0;

/// Ses images, confiées à la carte graphique : chaque planche confiée, et son image.
struct Images {
    cutout: ImageId,
    stand: Vec<(stand::Strip, ImageId)>,
    perch: Vec<(sheets::Strip, ImageId)>,
    life: Vec<(life::Strip, ImageId)>,
}

pub struct Figure {
    pub look: Look,
    images: Images,
}

/// Confie à `upload` les seuls dessins montrés d'une planche remise en grille (`grid`), serrés dans
/// la grille de `shown`.
fn upload_shown(
    upload: &mut impl FnMut(SpriteImage<'_>) -> Result<ImageId, RenderError>,
    shown: Shown,
    grid: &[u8],
) -> Result<ImageId, RenderError> {
    let (width, height) = shown.grid_size();
    let packed = shown.pack(grid).unwrap_or_default();
    upload(SpriteImage { width, height, rgba: &packed })
}

impl Figure {
    /// Confie ses images à `upload` (la carte graphique), fait les masques de clic, puis lâche les
    /// planches. `height` : sa hauteur debout, en pixels de l'overlay. Seul son dessin d'avant est
    /// indispensable : s'il ne peut être confié, c'est une erreur. Ses planches de vie ne servent
    /// qu'avec sa pose debout, sur laquelle elles s'alignent : sans elle, elles ne sont pas confiées.
    pub fn new(
        sprite: Sprite,
        stand: Option<StandSheets>,
        perch: Option<PerchSheets>,
        life: Option<LifeSheets>,
        height: f32,
        mut upload: impl FnMut(SpriteImage<'_>) -> Result<ImageId, RenderError>,
    ) -> Result<Self, RenderError> {
        let cutout = upload(SpriteImage { width: sprite.width, height: sprite.height, rgba: &sprite.rgba })?;
        let mut moves = Moves::default();
        let mut images = Images { cutout, stand: Vec::new(), perch: Vec::new(), life: Vec::new() };

        // sa pose debout : les trois ensemble, puis sa variante, à part
        let stand_mask = stand.and_then(|sheets| {
            let ids = stand::Strip::SET.iter().map(|&strip| {
                upload_shown(&mut upload, stand::shown(strip), &sheets.get(strip).rgba).map(|id| (strip, id))
            });
            match ids.collect::<Result<Vec<_>, _>>() {
                Ok(ids) => images.stand = ids,
                Err(err) => {
                    log::warn!(
                        "pose debout : la carte graphique la refuse ({err}), elle garde son dessin d'avant"
                    );
                    return None;
                }
            }
            for strip in stand::Strip::EXTRA {
                let Some(grid) = sheets.extra(strip) else { continue };
                match upload_shown(&mut upload, stand::shown(strip), &grid.rgba) {
                    Ok(id) => {
                        images.stand.push((strip, id));
                        moves.stand_blink_glitch = true;
                    }
                    Err(err) => log::warn!(
                        "pose debout : la carte graphique refuse {} ({err}), ses clignements d'avant",
                        strip.file()
                    ),
                }
            }
            Some(sheets.mask())
            // les planches sont lâchées ici
        });

        // sa pose assise : les six ensemble, puis ses planches facultatives, chacune à part
        let perch_masks = perch.and_then(|sheets| {
            let ids = sheets::Strip::SET.iter().map(|&strip| {
                upload_shown(&mut upload, sheets::shown(strip), &sheets.get(strip).rgba).map(|id| (strip, id))
            });
            match ids.collect::<Result<Vec<_>, _>>() {
                Ok(ids) => images.perch = ids,
                Err(err) => {
                    log::warn!("pose assise : la carte graphique la refuse ({err}), elle reste debout");
                    return None;
                }
            }
            for strip in sheets::Strip::EXTRA {
                let Some(grid) = sheets.extra(strip) else { continue };
                match upload_shown(&mut upload, sheets::shown(strip), &grid.rgba) {
                    Ok(id) => {
                        images.perch.push((strip, id));
                        match strip {
                            sheets::Strip::PerchGlitch => moves.perch_glitch = true,
                            _ => moves.perch_blink_glitch = true,
                        }
                    }
                    Err(err) => log::warn!(
                        "pose assise : la carte graphique refuse {} ({err}), elle s'en passe",
                        strip.file()
                    ),
                }
            }
            Some(sheets.masks())
        });

        // ses planches de vie, alignées sur sa pose debout : son arrivée et son départ ensemble,
        // puis chacun de ses gestes à part
        let mut life_mask = None;
        if let Some(sheets) = life.filter(|_| stand_mask.is_some()) {
            if sheets.appears() {
                let ids = life::Strip::APPEAR.iter().map(|&strip| {
                    let grid = sheets.get(strip).map(|grid| grid.rgba.as_slice()).unwrap_or_default();
                    upload_shown(&mut upload, life::shown(strip), grid).map(|id| (strip, id))
                });
                match ids.collect::<Result<Vec<_>, _>>() {
                    Ok(ids) => {
                        images.life.extend(ids);
                        life_mask = sheets.mask(life::Strip::Arrive);
                    }
                    Err(err) => log::warn!(
                        "planches de vie : la carte graphique les refuse ({err}), elle apparaîtra d'un coup"
                    ),
                }
            }
            for strip in life::Strip::MOVES {
                let Some(grid) = sheets.get(strip) else { continue };
                match upload_shown(&mut upload, life::shown(strip), &grid.rgba) {
                    Ok(id) => {
                        images.life.push((strip, id));
                        let mask = sheets.mask(strip);
                        match strip {
                            life::Strip::Walk => moves.walk = mask,
                            life::Strip::Dangle => moves.dangle = mask,
                            life::Strip::Glitch => moves.glitch = mask,
                            life::Strip::Grin => moves.grin = mask,
                            life::Strip::Arrive | life::Strip::Leave => {}
                        }
                    }
                    Err(err) => log::warn!(
                        "planches de vie : la carte graphique refuse {} ({err}), elle s'en passe",
                        strip.file()
                    ),
                }
            }
        }
        let wardrobe =
            Wardrobe { cutout: sprite.mask, stand: stand_mask, perch: perch_masks, life: life_mask, moves };
        Ok(Self { look: Look::new(wardrobe, height), images })
    }

    /// Ce qu'il faut dessiner pour `picture`, dans l'ordre : elle, puis sa bouche par-dessus.
    pub fn layers(&self, picture: &Picture) -> Vec<(ImageId, SpriteDraw)> {
        let rect = picture.rect;
        let plain = SpriteDraw {
            min: [rect.min.x, rect.min.y],
            size: [rect.size.x, rect.size.y],
            ghost: [0.0, 0.0],
            ghost_strength: 0.0,
            boil_seed: 0,
            boil: 0.0,
            opacity: 1.0,
            crop: Crop::WHOLE,
        };
        let cutout = || {
            let unit = rect.size.y / 420.0; // son dessin d'avant fait 420 px de haut
            vec![(
                self.images.cutout,
                SpriteDraw {
                    ghost: [picture.ghost.x, picture.ghost.y],
                    ghost_strength: GHOST,
                    boil_seed: picture.boil_seed,
                    boil: BOIL * unit,
                    ..plain
                },
            )]
        };
        let stand_cell = |cell: stand::Cell| {
            let id = image_of(&self.images.stand, cell.strip)?;
            let shown = stand::shown(cell.strip);
            let crop = Crop::pixels(shown.origin(cell.index)?, cell.strip.cell(), shown.grid_size());
            Some((id, SpriteDraw { crop, ..plain }))
        };
        let perch_cell = |cell: sheets::Cell| {
            let id = image_of(&self.images.perch, cell.strip)?;
            let shown = sheets::shown(cell.strip);
            let crop = Crop::pixels(shown.origin(cell.index)?, cell.strip.cell(), shown.grid_size());
            Some(vec![(id, SpriteDraw { crop, ..plain })])
        };
        let life_cell = |cell: life::Cell| {
            let id = image_of(&self.images.life, cell.strip)?;
            let shown = life::shown(cell.strip);
            let crop = Crop::pixels(shown.origin(cell.index)?, cell.strip.cell(), shown.grid_size());
            Some(vec![(id, SpriteDraw { crop, ..plain })])
        };
        match picture.drawing {
            Drawing::Cutout => cutout(),
            Drawing::Stand(cell) => match stand_cell(cell) {
                Some(body) => std::iter::once(body).chain(picture.mouth.and_then(stand_cell)).collect(),
                None => cutout(),
            },
            Drawing::Perch(cell) => perch_cell(cell).unwrap_or_else(cutout),
            // sans ces images, `look` ne dessine jamais ses planches de vie ; par prudence, rien
            // plutôt que son dessin d'avant déformé à leur taille
            Drawing::Life(cell) => life_cell(cell).unwrap_or_default(),
        }
    }
}

/// L'image confiée pour cette planche, si elle l'a été.
fn image_of<T: PartialEq>(images: &[(T, ImageId)], strip: T) -> Option<ImageId> {
    images.iter().find(|(confided, _)| *confided == strip).map(|&(_, id)| id)
}
