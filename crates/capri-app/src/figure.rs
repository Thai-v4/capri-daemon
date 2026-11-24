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
//!
//! **Elle se rassemble** (une intro, `capri_core::intro::act`) : rien de plus sur la carte
//! graphique, des parts de la planche de son arrivée déjà confiée, une bande de lignes chacune,
//! étirées vers les bords de l'écran (huit dessins de plus pendant 0,75 s).
//!
//! **Ses planches de repos** (sa pose allongée, son sommeil debout) ne sont jamais confiées au
//! lancement : chargées à la demande (`on_demand`), quand son dessin en veut
//! (`capri_core::look::Look::wants`), et lâchées une minute après qu'il n'en veut plus
//! ([`Figure::sync_rests`]). Allongée : 40,9 Mo de textures de plus (ses trois boucles de 372 ×
//! 250 px, un dessin sur deux, son sommeil un sur trois, son clignement, son éclat, son départ) ;
//! endormie debout : 11,1 Mo (son sommeil et son réveil, de 240 px).

use std::path::PathBuf;
use std::time::Instant;

use capri_core::Rect;
use capri_core::intro::act;
use capri_core::look::{Drawing, Lean, Look, Moves, Picture, Wants, Wardrobe};
use capri_core::perch::Side;
use capri_core::sheets::{self, Shown, lie, life, stand};
use capri_render::{Crop, GHOST, ImageId, Keep, RenderError, SpriteDraw, SpriteImage, Tint, Turn};

use crate::assets::Sprite;
use crate::assets::life::LifeSheets;
use crate::assets::perch::PerchSheets;
use crate::assets::rest::{Lying, Napping};
use crate::assets::stand::StandSheets;
use crate::intro_view::Canvas;
use crate::on_demand::{Change, OnDemand};

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
    /// Sa pose allongée, chargée à la demande.
    lying: OnDemand<Lying>,
    /// Son sommeil et son réveil debout, chargés à la demande.
    napping: OnDemand<Napping>,
}

/// Où sont ses planches de repos (le dossier des images, d'après leurs en-têtes lus au lancement :
/// `assets::rest`), si elles y sont.
#[derive(Clone, Debug, Default)]
pub struct Rests {
    pub lying: Option<PathBuf>,
    pub sleeping: Option<PathBuf>,
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
                            life::Strip::Arrive
                            | life::Strip::Leave
                            | life::Strip::Sleep
                            | life::Strip::Wake => {}
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
        Ok(Self {
            look: Look::new(wardrobe, height),
            images,
            lying: OnDemand::new(None),
            napping: OnDemand::new(None),
        })
    }

    /// Ses planches de repos, là où elles sont (rien n'est lu ici) : elle pourra s'allonger
    /// (avec sa pose assise), s'endormir debout (avec sa pose debout, sur laquelle elles
    /// s'alignent). Sans elles, elle ne le fait jamais.
    pub fn offer_rests(&mut self, rests: Rests) {
        self.look.offer(Wants { lying: rests.lying.is_some(), sleeping: rests.sleeping.is_some() });
        let offered = self.look.offered();
        self.lying = OnDemand::new(rests.lying.filter(|_| offered.lying));
        self.napping = OnDemand::new(rests.sleeping.filter(|_| offered.sleeping));
    }

    /// Un pas de ses planches de repos, à l'instant `now` : celles que son dessin veut sont
    /// chargées (hors du thread principal) et confiées à `gpu` à mesure qu'elles arrivent, puis
    /// données à son dessin ; celles qu'il ne veut plus depuis une minute, lâchées et reprises. Un
    /// chargement qui échoue : elle s'en passe jusqu'à son prochain lancement. Ne bloque jamais.
    pub fn sync_rests(&mut self, gpu: &mut impl Canvas<Id = ImageId>, now: Instant) {
        let wants = self.look.wants();
        match self.lying.step(wants.lying, now, gpu) {
            Change::Nothing => {}
            Change::Dressed(masks) => self.look.dress_lying(Some(masks)),
            Change::Undressed => self.look.dress_lying(None),
            Change::Failed => {
                self.look.dress_lying(None);
                self.look.offer(Wants { lying: false, ..self.look.offered() });
            }
        }
        match self.napping.step(wants.sleeping, now, gpu) {
            Change::Nothing => {}
            Change::Dressed(masks) => self.look.dress_sleeping(Some(masks)),
            Change::Undressed => self.look.dress_sleeping(None),
            Change::Failed => {
                self.look.dress_sleeping(None);
                self.look.offer(Wants { sleeping: false, ..self.look.offered() });
            }
        }
    }

    /// Ce qu'il faut dessiner pour `picture`, dans l'ordre : elle, puis sa bouche par-dessus ;
    /// quand elle se rassemble (une intro), ses bandes, étirées vers les bords de l'écran `screen`.
    /// Guettant, elle et sa bouche penchées autour de sa hanche, et coupées net au bord de sa
    /// fenêtre (ses images telles quelles : la carte graphique les tourne et les découpe).
    pub fn layers(&self, picture: &Picture, screen: Rect) -> Vec<(ImageId, SpriteDraw)> {
        let rect = picture.rect;
        let (turn, keep) = leaning(picture.lean);
        let plain = SpriteDraw {
            min: [rect.min.x, rect.min.y],
            size: [rect.size.x, rect.size.y],
            ghost: [0.0, 0.0],
            ghost_strength: 0.0,
            boil_seed: 0,
            boil: 0.0,
            opacity: 1.0,
            crop: Crop::WHOLE,
            tint: Tint::NONE,
            turn,
            keep,
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
            // son sommeil et son réveil debout : chargés à la demande
            let id = image_of(&self.images.life, cell.strip).or_else(|| self.napping.image(cell.strip))?;
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
            // sa pose allongée, chargée à la demande : `look` ne la dessine que confiée ; par
            // prudence, rien plutôt que son dessin d'avant déformé à sa taille
            Drawing::Lie(cell) => {
                let shown = lie::shown(cell.strip);
                let drawn = self.lying.image(cell.strip).zip(shown.origin(cell.index)).map(|(id, origin)| {
                    let crop = Crop::pixels(origin, cell.strip.cell(), shown.grid_size());
                    (id, SpriteDraw { crop, ..plain })
                });
                drawn.into_iter().collect()
            }
            // sans ces images, `look` ne dessine jamais ses planches de vie ; par prudence, rien
            // plutôt que son dessin d'avant déformé à leur taille
            Drawing::Life(cell) => life_cell(cell).unwrap_or_default(),
            // des parts de son arrivée : une bande de lignes de sa case, chacune étirée
            Drawing::Gather(key) => {
                let strip = life::Strip::Arrive;
                let Some(id) = image_of(&self.images.life, strip) else { return Vec::new() };
                let (index, _) = act::GATHER[usize::from(key).min(act::GATHER.len() - 1)];
                let shown = life::shown(strip);
                let Some((x, y)) = shown.origin(index) else { return Vec::new() };
                act::bands(rect, key, screen)
                    .iter()
                    .map(|band| {
                        let crop = Crop::pixels(
                            (x, y + band.rows.0),
                            (life::CELL_WIDTH, band.rows.1),
                            shown.grid_size(),
                        );
                        let min = [band.rect.min.x, band.rect.min.y];
                        let size = [band.rect.size.x, band.rect.size.y];
                        (id, SpriteDraw { min, size, crop, ..plain })
                    })
                    .collect()
            }
        }
    }
}

/// L'image confiée pour cette planche, si elle l'a été.
fn image_of<T: PartialEq>(images: &[(T, ImageId)], strip: T) -> Option<ImageId> {
    images.iter().find(|(confided, _)| *confided == strip).map(|&(_, id)| id)
}

/// Guettant (`lean`), comment la carte graphique la dessine : tournée comme son masque de clic la
/// voit (`Lean::turned`), et gardée là où elle se voit (`Lean::shows`) : à gauche du bord sur le
/// côté gauche d'une fenêtre, à droite sur le droit. Sinon, droite et entière.
fn leaning(lean: Option<Lean>) -> (Turn, Keep) {
    let Some(lean) = lean else { return (Turn::NONE, Keep::ALL) };
    let turn = Turn { angle: lean.angle, pivot: [lean.pivot.x, lean.pivot.y] };
    let keep = match lean.side {
        Side::Left => Keep { min: [f32::MIN, f32::MIN], max: [lean.wall, f32::MAX] },
        Side::Right => Keep { min: [lean.wall, f32::MIN], max: [f32::MAX, f32::MAX] },
    };
    (turn, keep)
}

#[cfg(test)]
mod tests {
    use capri_core::Vec2;
    use capri_core::look::Lean;
    use capri_core::perch::Side;
    use capri_core::sheets::stand;
    use capri_render::{Keep, Turn};

    use super::leaning;

    #[test]
    fn leaning_she_is_turned_and_kept_exactly_as_her_click_mask_sees_her() {
        for side in Side::ALL {
            let lean = Lean { angle: stand::lean(side), pivot: Vec2::new(500.0, 600.0), wall: 450.0, side };
            let (turn, keep) = leaning(Some(lean));
            // tournée du même angle autour du même point : sa tête vers l'extérieur de la fenêtre
            assert_eq!(turn, Turn { angle: lean.angle, pivot: [500.0, 600.0] });
            let head = lean.turned(Vec2::new(500.0, 300.0));
            assert!((head.x - 500.0) * side.outward() > 0.0, "{side:?}");
            // gardée exactement là où elle se voit (`min` compris, `max` exclu)
            for x in [0.0, 440.0, 449.0, 449.9, 450.0, 450.1, 451.0, 460.0, 3000.0] {
                let kept = keep.min[0] <= x && x < keep.max[0];
                assert_eq!(kept, lean.shows(Vec2::new(x, 600.0)), "{side:?} en x = {x}");
            }
            assert!(keep.min[1] <= -1e9 && keep.max[1] >= 1e9, "toute la hauteur");
        }
        assert_eq!(leaning(None), (Turn::NONE, Keep::ALL), "droite et entière");
    }
}
