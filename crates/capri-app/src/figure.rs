//! Ses dessins, côté application : son dessin d'avant et ses planches, confiés une fois pour
//! toutes à la carte graphique puis lâchés, les masques de clic de chaque pose, et ce que
//! `capri_core::look` décide de montrer, traduit en dessins pour `capri-render`.
//!
//! **Le repli** : une planche qui manque, illisible ou que la carte graphique refuse, et la pose
//! qu'elle porte est remplacée par son dessin d'avant (`capri-cutout.png`), comme avant ; seul
//! son dessin d'avant est indispensable.
//!
//! **Ce que ça pèse** : les planches lues font environ 95 Mo en mémoire (remises en grilles :
//! 81 Mo, plus son dessin d'avant) ; elles ne sont gardées que le temps de faire les masques et de
//! les confier à la carte graphique, où elles pèsent autant (RGBA 8 bits, sans mipmaps). Les masques
//! de clic, eux, restent : un booléen par pixel d'une case de chaque pose, 230 Ko en tout.

use capri_core::look::{Drawing, Look, Picture, Wardrobe};
use capri_core::sheets::{self, stand};
use capri_render::{Crop, GHOST, ImageId, RenderError, SpriteDraw, SpriteImage};

use crate::assets::Sprite;
use crate::assets::perch::PerchSheets;
use crate::assets::stand::StandSheets;

/// Amplitude du « boil » de son dessin d'avant, en pixels de cette image (420 px de haut).
const BOIL: f32 = 1.0;

/// Ses images, confiées à la carte graphique.
struct Images {
    cutout: ImageId,
    /// Dans l'ordre de `stand::Strip::ALL`.
    stand: Option<Vec<ImageId>>,
    /// Dans l'ordre de `sheets::Strip::ALL`.
    perch: Option<Vec<ImageId>>,
}

pub struct Figure {
    pub look: Look,
    images: Images,
}

impl Figure {
    /// Confie ses images à `upload` (la carte graphique), fait les masques de clic, puis lâche les
    /// planches. `height` : sa hauteur debout, en pixels de l'overlay. Seul son dessin d'avant est
    /// indispensable : s'il ne peut être confié, c'est une erreur.
    pub fn new(
        sprite: Sprite,
        stand: Option<StandSheets>,
        perch: Option<PerchSheets>,
        height: f32,
        mut upload: impl FnMut(SpriteImage<'_>) -> Result<ImageId, RenderError>,
    ) -> Result<Self, RenderError> {
        let cutout = upload(SpriteImage { width: sprite.width, height: sprite.height, rgba: &sprite.rgba })?;
        let stand = stand.and_then(|sheets| {
            let ids = stand::Strip::ALL.iter().map(|&strip| {
                let grid = sheets.get(strip);
                upload(SpriteImage { width: grid.width, height: grid.height, rgba: &grid.rgba })
            });
            match ids.collect::<Result<Vec<_>, _>>() {
                Ok(ids) => Some((sheets.mask(), ids)),
                Err(err) => {
                    log::warn!(
                        "pose debout : la carte graphique la refuse ({err}), elle garde son dessin d'avant"
                    );
                    None
                }
            }
            // les planches sont lâchées ici
        });
        let perch = perch.and_then(|sheets| {
            let ids = sheets::Strip::ALL.iter().map(|&strip| {
                let grid = sheets.get(strip);
                upload(SpriteImage { width: grid.width, height: grid.height, rgba: &grid.rgba })
            });
            match ids.collect::<Result<Vec<_>, _>>() {
                Ok(ids) => Some((sheets.masks(), ids)),
                Err(err) => {
                    log::warn!("pose assise : la carte graphique la refuse ({err}), elle reste debout");
                    None
                }
            }
        });
        let (stand_mask, stand_ids) = stand.unzip();
        let (perch_masks, perch_ids) = perch.unzip();
        let wardrobe = Wardrobe { cutout: sprite.mask, stand: stand_mask, perch: perch_masks };
        Ok(Self {
            look: Look::new(wardrobe, height),
            images: Images { cutout, stand: stand_ids, perch: perch_ids },
        })
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
            let id = self.images.stand.as_ref()?.get(position(&stand::Strip::ALL, cell.strip))?;
            let crop =
                Crop::pixels(cell.strip.cell_origin(cell.index), cell.strip.cell(), cell.strip.grid_size());
            Some((*id, SpriteDraw { crop, ..plain }))
        };
        match picture.drawing {
            Drawing::Cutout => cutout(),
            Drawing::Stand(cell) => match stand_cell(cell) {
                Some(body) => std::iter::once(body).chain(picture.mouth.and_then(stand_cell)).collect(),
                None => cutout(),
            },
            Drawing::Perch(cell) => {
                let id = self
                    .images
                    .perch
                    .as_ref()
                    .and_then(|ids| ids.get(position(&sheets::Strip::ALL, cell.strip)));
                match id {
                    Some(id) => {
                        let crop = Crop::pixels(
                            cell.strip.cell_origin(cell.index),
                            cell.strip.cell(),
                            cell.strip.grid_size(),
                        );
                        vec![(*id, SpriteDraw { crop, ..plain })]
                    }
                    None => cutout(),
                }
            }
        }
    }
}

fn position<T: PartialEq>(all: &[T], wanted: T) -> usize {
    all.iter().position(|item| *item == wanted).unwrap_or(0)
}
