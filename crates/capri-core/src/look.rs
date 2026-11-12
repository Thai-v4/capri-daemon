//! Son dessin : quelle case de quelle planche montrer à chaque image, où, et ce qui l'attrape.
//!
//! **Ses dessins.** Debout, la boucle de sa pose debout ([`stand`]) ; posée sur une fenêtre,
//! celles de sa pose assise ([`sheets`]), du même artiste. Quand des planches manquent, son dessin
//! d'avant ([`Drawing::Cutout`]) les remplace, comme avant : sans planches debout, c'est lui
//! qu'on voit debout ; sans planches assises, elle se tient debout sur les fenêtres.
//!
//! **Debout** : la boucle, tenue en deux, avec le flottement que calcule `body.rs` (la boucle de
//! l'artiste ne flotte pas, voir [`stand`]) ; elle cligne des yeux à certains tours seulement
//! ([`BLINKS`]). Tenue ou lancée : la même boucle, écrasée et étirée par `body.rs`. Quand elle
//! parle : sa bouche posée par-dessus ([`stand::talking`]), au même endroit.
//!
//! **Assise** : posée, elle apparaît sur le bord ([`sheets::arriving`]), puis sa boucle, où elle
//! cligne aux mêmes tours. Elle parle avec [`sheets::talking`] (la même boucle, la bouche en plus),
//! et se balance quand de la musique joue ([`sheets::music`]) : parler d'abord, puis la musique,
//! puis la boucle. Elle entre dans la musique et en sort au début d'un tour de sa boucle, là où
//! leurs dessins se ressemblent le plus ; elle se met à parler tout de suite. Quand elle repart
//! d'elle-même, elle s'efface là où elle était ([`sheets::leaving`]), puis flotte de nouveau,
//! debout ; quand on l'attrape, elle est debout tout de suite, sous le curseur
//! ([`Look::grab_origin`]).
//!
//! **Le bord de la fenêtre passe sur sa ligne d'assise** ([`sheets::SEAT_Y`]) : ses jambes pendent
//! devant la fenêtre. Ce qu'il lui faut de place, assise, est dans [`Look::stature`].
//!
//! **Les deux poses à la même échelle** : sa hauteur debout ([`stand::STATURE`], des cheveux à la
//! pointe du pied) vaut la hauteur choisie, et un pixel de ses planches assises vaut
//! `1 / STAND_OVER_PERCH` pixel de ses planches debout ([`stand::perch_scale`]).
//!
//! **Le temps** : l'image d'animation à 24 i/s ; tous ses dessins sont tenus deux images (12 par
//! seconde). Rien n'est tiré au hasard : la même image donne toujours le même dessin.

use crate::anim::Sheet;
use crate::body::Capri;
use crate::geom::{Rect, Vec2};
use crate::mask::AlphaMask;
use crate::perch::{FOOT, Reach, Stature};
use crate::sheets::{self, stand};

/// Un tour de sa boucle, debout comme assise : 60 images, 2,5 s.
pub const TOUR: u64 = 60;
/// Les tours de sa boucle où elle cligne des yeux, en boucle sur 8 tours (20 s) : quatre
/// clignements, espacés de 5 s, 2,5 s, 5 s puis 7,5 s. Assez pour qu'elle ait l'air vivante, pas
/// à chaque tour, qui ferait mécanique, et jamais au hasard : la même image donne le même dessin.
pub const BLINKS: [bool; 8] = [true, false, true, true, false, true, false, false];

/// Cligne-t-elle des yeux au tour n° `tour` de sa boucle ?
pub fn blinks(tour: u64) -> bool {
    BLINKS[(tour % BLINKS.len() as u64) as usize]
}

/// Un dessin d'elle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Drawing {
    /// Son dessin d'avant, entier (`capri-cutout.png`).
    Cutout,
    /// Une case de sa pose debout.
    Stand(stand::Cell),
    /// Une case de sa pose assise.
    Perch(sheets::Cell),
}

/// Ce qu'on dessine à une image donnée.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    pub drawing: Drawing,
    /// Où, et à quelle taille (pixels entiers).
    pub rect: Rect,
    /// Sa bouche, posée par-dessus, dans le même rectangle.
    pub mouth: Option<stand::Cell>,
    /// Le trait qui « bout » et la copie violette : pour son dessin d'avant seulement (les planches
    /// sont nettes, et ont déjà leur liseré).
    pub boil_seed: u32,
    pub ghost: Vec2,
}

/// Les masques de clic de sa pose assise : l'union des quatre boucles (cases étroites), et celle
/// de l'arrivée et du départ (cases larges).
pub struct PerchMasks {
    pub narrow: AlphaMask,
    pub wide: AlphaMask,
}

/// Ce dont elle dispose : son dessin d'avant (toujours là), et les planches chargées, chacune
/// avec le masque de clic de sa pose (l'union des dessins de sa boucle).
pub struct Wardrobe {
    pub cutout: AlphaMask,
    /// Sa pose debout : l'union de la boucle et du clignement.
    pub stand: Option<AlphaMask>,
    pub perch: Option<PerchMasks>,
}

/// Où en est sa pose assise.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seat {
    /// Debout : rien de sa pose assise à montrer.
    Standing,
    /// Elle apparaît sur le bord depuis l'image `since`.
    Arriving { since: u64 },
    /// Assise ; sa boucle est au dessin 0 à l'image `since`.
    Sitting { since: u64 },
    /// Elle s'efface depuis l'image `since`, là où elle était assise.
    Leaving { since: u64, at: Vec2 },
}

/// Ses feuilles d'exposition, faites une fois.
struct Sheets {
    standing: Sheet<stand::Cell>,
    standing_blink: Sheet<stand::Cell>,
    mouth: Sheet<stand::Cell>,
    sitting: Sheet<sheets::Cell>,
    sitting_blink: Sheet<sheets::Cell>,
    talking: Sheet<sheets::Cell>,
    music: Sheet<sheets::Cell>,
    arriving: Sheet<sheets::Cell>,
    leaving: Sheet<sheets::Cell>,
}

pub struct Look {
    wardrobe: Wardrobe,
    /// Sa hauteur debout, en pixels de l'overlay.
    height: f32,
    seat: Seat,
    /// Le point posé sur le bord à la dernière mise à jour où elle l'était : là où elle s'efface.
    last_seat: Option<Vec2>,
    frame: u64,
    talking: bool,
    /// Assise, elle se balance (la musique se prend et se quitte au début d'un tour).
    grooving: bool,
    sheets: Sheets,
}

impl Look {
    /// `height` : sa hauteur debout, en pixels de l'overlay.
    pub fn new(wardrobe: Wardrobe, height: f32) -> Self {
        Self {
            wardrobe,
            height,
            seat: Seat::Standing,
            last_seat: None,
            frame: 0,
            talking: false,
            grooving: false,
            sheets: Sheets {
                standing: stand::standing(false),
                standing_blink: stand::standing(true),
                mouth: stand::talking(),
                sitting: sheets::sitting(false),
                sitting_blink: sheets::sitting(true),
                talking: sheets::talking(),
                music: sheets::music(),
                arriving: sheets::arriving(),
                leaving: sheets::leaving(),
            },
        }
    }

    /// Ses planches debout sont-elles là ?
    pub fn has_stand(&self) -> bool {
        self.wardrobe.stand.is_some()
    }

    /// Ses planches assises sont-elles là ?
    pub fn has_perch(&self) -> bool {
        self.wardrobe.perch.is_some()
    }

    /// Sa hauteur debout a changé (sa taille, ou la densité de l'écran).
    pub fn set_height(&mut self, height: f32) {
        self.height = height;
    }

    /// La taille à l'écran de son image debout (une case de sa boucle, ou son dessin d'avant),
    /// pour qu'elle mesure sa hauteur debout.
    pub fn standing_size(&self) -> Vec2 {
        if self.has_stand() {
            return stand::cell_size(self.height);
        }
        let (width, height) = self.wardrobe.cutout.size();
        Vec2::new(width as f32 * self.height / height.max(1) as f32, self.height)
    }

    /// Où sont ses pieds dans son image debout (coordonnées normalisées).
    pub fn standing_feet(&self) -> Vec2 {
        if self.has_stand() { stand::feet_uv() } else { self.wardrobe.cutout.feet() }
    }

    /// Combien d'images dure son départ d'un bord (voir `Capri::set_leave_frames`) : le temps de
    /// s'effacer, si elle a une pose assise.
    pub fn leave_frames(&self) -> u64 {
        if self.has_perch() { self.sheets.leaving.len() } else { 0 }
    }

    /// Ce qu'il lui faut sur un bord pour s'y poser, ou y rester : assise, sa ligne d'assise sur
    /// le bord, la place de ses cheveux au-dessus et de ses jambes en dessous, et ce qui repose sur
    /// le bord (de sa main à son genou) ; sans planches assises, debout, comme avant.
    pub fn stature(&self) -> Stature {
        if self.has_perch() {
            seated_stature(self.height)
        } else {
            Stature::standing(self.standing_size(), self.standing_feet(), self.height)
        }
    }

    /// Posée, avec ses planches assises : c'est alors seulement que la musique peut la faire se
    /// balancer.
    pub fn wants_music(&self, capri: &Capri) -> bool {
        self.has_perch() && capri.perched().is_some()
    }

    /// Se balance-t-elle en ce moment ?
    pub fn is_grooving(&self) -> bool {
        self.grooving && matches!(self.seat, Seat::Sitting { .. }) && !self.talking
    }

    /// À chaque pas, après celui de Capri et le suivi de sa fenêtre : l'image d'animation
    /// `frame`, si elle parle, et si de la musique joue.
    pub fn update(&mut self, capri: &Capri, frame: u64, talking: bool, music: bool) {
        let previous = std::mem::replace(&mut self.frame, frame);
        self.talking = talking;
        let seat = capri.seat().filter(|_| self.has_perch());
        let (arrive, leave) = (self.sheets.arriving.len(), self.sheets.leaving.len());
        self.seat = match (self.seat, seat) {
            // on l'attrape : debout tout de suite, sans s'effacer
            (_, None) if capri.is_held() => Seat::Standing,
            (Seat::Standing | Seat::Leaving { .. }, Some(_)) => Seat::Arriving { since: frame },
            (Seat::Arriving { since }, Some(_)) if frame >= since + arrive => {
                Seat::Sitting { since: since + arrive }
            }
            (Seat::Arriving { .. } | Seat::Sitting { .. }, None) => match self.last_seat {
                Some(at) => Seat::Leaving { since: frame, at },
                None => Seat::Standing,
            },
            (Seat::Leaving { since, .. }, None) if frame >= since + leave => Seat::Standing,
            (unchanged, _) => unchanged,
        };
        if seat.is_some() {
            self.last_seat = seat;
        }
        // la musique se prend et se quitte au début d'un tour de sa boucle ; en parlant, elle ne
        // se balance pas, et n'y revient qu'au tour suivant
        match self.seat {
            Seat::Sitting { .. } if talking => self.grooving = false,
            Seat::Sitting { since } => {
                let tour = |f: u64| f.saturating_sub(since) / TOUR;
                if previous < since || tour(frame) != tour(previous) {
                    self.grooving = music;
                }
            }
            _ => self.grooving = false,
        }
    }

    /// Ce qu'on dessine maintenant.
    pub fn picture(&self, capri: &Capri) -> Picture {
        let frame = self.frame;
        if !capri.is_held() {
            let at = capri.seat().or(self.last_seat);
            match (self.seat, at) {
                (Seat::Arriving { since }, Some(at)) => {
                    let cell = at_most(&self.sheets.arriving, frame.saturating_sub(since));
                    return self.seated(at, cell);
                }
                (Seat::Sitting { since }, Some(at)) => {
                    let f = frame.saturating_sub(since);
                    let sheet = if self.talking {
                        &self.sheets.talking
                    } else if self.grooving {
                        &self.sheets.music
                    } else if blinks(f / TOUR) {
                        &self.sheets.sitting_blink
                    } else {
                        &self.sheets.sitting
                    };
                    return self.seated(at, sheet.at(f));
                }
                (Seat::Leaving { since, at }, _) => {
                    return self.seated(at, at_most(&self.sheets.leaving, frame.saturating_sub(since)));
                }
                _ => {}
            }
        }
        self.standing(capri)
    }

    /// Est-elle dessinée assise (en train d'arriver, assise, ou de s'effacer) ?
    pub fn is_seated(&self, capri: &Capri) -> bool {
        matches!(self.picture(capri).drawing, Drawing::Perch(_))
    }

    /// Le curseur est-il sur elle ? Au pixel près, d'après le masque de la pose dessinée (avec
    /// une petite marge pour attraper ses membres fins).
    pub fn is_over(&self, capri: &Capri, cursor: Vec2) -> bool {
        let picture = self.picture(capri);
        let cutout = &self.wardrobe.cutout;
        let mask = match picture.drawing {
            Drawing::Cutout => cutout,
            Drawing::Stand(_) => self.wardrobe.stand.as_ref().unwrap_or(cutout),
            Drawing::Perch(cell) => match &self.wardrobe.perch {
                Some(masks) if cell.strip.cell().0 == sheets::WIDE => &masks.wide,
                Some(masks) => &masks.narrow,
                None => cutout,
            },
        };
        picture.rect.to_unit(cursor).is_some_and(|uv| mask.hit(uv))
    }

    /// Ce qu'elle occupe à l'écran, sans son flottement ni ses gestes : de quoi placer son
    /// bandeau au-dessus d'elle et le champ de saisie sous ses pieds.
    pub fn shown(&self, capri: &Capri) -> Rect {
        if let (Drawing::Perch(_), Some(at)) = (self.picture(capri).drawing, self.seated_at(capri)) {
            let k = stand::perch_scale(self.height);
            let left = (sheets::SEAT_X - sheets::SPAN.0) as f32 * k;
            return Rect::new(
                Vec2::new(at.x - left, at.y - sheets::HEADROOM as f32 * k),
                Vec2::new(
                    (sheets::SPAN.1 - sheets::SPAN.0) as f32 * k,
                    (sheets::HEADROOM + sheets::HANG) as f32 * k,
                ),
            );
        }
        let (top, tall) = self.standing_content();
        Rect::new(capri.position() + Vec2::new(0.0, top), Vec2::new(capri.size().x, tall))
    }

    /// On l'attrape assise, le curseur en `cursor` : où mettre le coin haut gauche de son image
    /// debout pour que le curseur tienne le même endroit d'elle (sa tête, ses pieds…), à
    /// proportion de sa silhouette. `None` si elle n'est pas dessinée assise.
    pub fn grab_origin(&self, capri: &Capri, cursor: Vec2) -> Option<Vec2> {
        if !self.is_seated(capri) {
            return None;
        }
        let from = self.shown(capri);
        let u = ((cursor.x - from.min.x) / from.size.x.max(1.0)).clamp(0.0, 1.0);
        let v = ((cursor.y - from.min.y) / from.size.y.max(1.0)).clamp(0.0, 1.0);
        let (top, tall) = self.standing_content();
        Some(Vec2::new(cursor.x - u * capri.size().x, cursor.y - top - v * tall))
    }

    /// Dans son image debout : où commence sa silhouette en hauteur, et combien elle mesure.
    fn standing_content(&self) -> (f32, f32) {
        let size = self.standing_size();
        if self.has_stand() {
            let s = size.y / sheets::CELL_HEIGHT as f32;
            (stand::TOP as f32 * s, stand::STATURE as f32 * s)
        } else {
            (0.0, size.y)
        }
    }

    /// Là où elle est (ou était) assise.
    fn seated_at(&self, capri: &Capri) -> Option<Vec2> {
        match self.seat {
            Seat::Leaving { at, .. } => Some(at),
            _ => capri.seat().or(self.last_seat),
        }
    }

    /// Debout : la boucle (ou son dessin d'avant) là où `body.rs` la met.
    fn standing(&self, capri: &Capri) -> Picture {
        let pose = capri.pose();
        if !self.has_stand() {
            return Picture {
                drawing: Drawing::Cutout,
                rect: pose.rect,
                mouth: None,
                boil_seed: pose.boil_seed,
                ghost: pose.ghost,
            };
        }
        let frame = self.frame;
        let sheet = if blinks(frame / TOUR) { &self.sheets.standing_blink } else { &self.sheets.standing };
        Picture {
            drawing: Drawing::Stand(sheet.at(frame)),
            rect: pose.rect,
            mouth: self.talking.then(|| self.sheets.mouth.at(frame)),
            boil_seed: 0,
            ghost: Vec2::ZERO,
        }
    }

    /// Assise, la case `cell` posée sur le bord en `at` : sa ligne d'assise sur le bord.
    fn seated(&self, at: Vec2, cell: sheets::Cell) -> Picture {
        let k = stand::perch_scale(self.height);
        let ((w, h), (x, y)) = (cell.strip.cell(), cell.strip.seat());
        Picture {
            drawing: Drawing::Perch(cell),
            rect: Rect::new(
                Vec2::new((at.x - x as f32 * k).round(), (at.y - y as f32 * k).round()),
                Vec2::new((w as f32 * k).round(), (h as f32 * k).round()),
            ),
            mouth: None,
            boil_seed: 0,
            ghost: Vec2::ZERO,
        }
    }
}

/// Ce qu'il lui faut sur un bord, assise, pour une hauteur debout de `height` pixels : sa ligne
/// d'assise sur le bord, la place de ses cheveux au-dessus ([`sheets::HEADROOM`]) et de ses
/// jambes en dessous ([`sheets::HANG`]), sa largeur ([`sheets::SPAN`]), et ce qui repose sur le
/// bord, de sa main à son genou ([`sheets::REST`]), jamais moins que pour ses pieds debout.
pub fn seated_stature(height: f32) -> Stature {
    let k = stand::perch_scale(height);
    let px = |pixels: u32| pixels as f32 * k;
    let foot = FOOT * height;
    Stature {
        height,
        reach: Reach {
            left: px(sheets::SEAT_X - sheets::SPAN.0),
            up: px(sheets::HEADROOM),
            right: px(sheets::SPAN.1 - sheets::SEAT_X),
            down: px(sheets::HANG),
        },
        rest: (px(sheets::SEAT_X - sheets::REST.0).max(foot), px(sheets::REST.1 - sheets::SEAT_X).max(foot)),
    }
}

/// Une suite jouée une fois : le dessin de l'image `frame`, ou le dernier une fois finie.
fn at_most(sheet: &Sheet<sheets::Cell>, frame: u64) -> sheets::Cell {
    sheet.at(frame.min(sheet.len() - 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Stance;
    use crate::perch::{Perch, Spot};
    use crate::sheets::Strip;
    use crate::sheets::stand::Strip as StandStrip;

    /// Un masque plein, de la taille donnée.
    fn full(width: u32, height: u32) -> AlphaMask {
        AlphaMask::from_rgba(width, height, &vec![255; (width * height * 4) as usize], 96, 0)
    }

    /// Un masque vide sauf un rectangle plein (en pixels de l'image).
    fn block(width: u32, height: u32, (x0, y0, x1, y1): (u32, u32, u32, u32)) -> AlphaMask {
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for y in y0..y1 {
            for x in x0..x1 {
                rgba[((y * width + x) * 4 + 3) as usize] = 255;
            }
        }
        AlphaMask::from_rgba(width, height, &rgba, 96, 0)
    }

    fn wardrobe(stand: bool, perch: bool) -> Wardrobe {
        Wardrobe {
            cutout: full(220, 420),
            stand: stand.then(|| full(stand::CELL_WIDTH, sheets::CELL_HEIGHT)),
            perch: perch.then(|| PerchMasks {
                narrow: block(sheets::NARROW, sheets::CELL_HEIGHT, (10, 40, 150, 370)),
                wide: full(sheets::WIDE, sheets::CELL_HEIGHT),
            }),
        }
    }

    /// Debout, 300 points de haut sur un écran Retina.
    const HEIGHT: f32 = 600.0;

    fn dressed(stand: bool, perch: bool) -> Look {
        Look::new(wardrobe(stand, perch), HEIGHT)
    }

    fn capri(look: &Look) -> Capri {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        let mut capri = Capri::new(look.standing_size(), screen, Vec2::new(1000.0, 800.0));
        capri.set_feet(look.standing_feet());
        capri.set_leave_frames(look.leave_frames());
        capri
    }

    const SPOT: Spot = Spot { window: 7, along: 400.0, left: 600.0, width: 1600.0 };
    const EDGE: Vec2 = Vec2::new(1000.0, 1200.0);

    /// Fait avancer Capri et son dessin jusqu'à l'image `frame`.
    fn at(look: &mut Look, capri: &mut Capri, frame: u64, talking: bool, music: bool) -> Picture {
        capri.step(0.0, frame);
        look.update(capri, frame, talking, music);
        look.picture(capri)
    }

    #[test]
    fn standing_she_plays_her_loop_in_twos_and_blinks_only_on_chosen_tours() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        let mut blinked_tours = Vec::new();
        for frame in 0..TOUR * 8 {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Stand(cell) = picture.drawing else { panic!("debout : {picture:?}") };
            let drawing = (frame % TOUR) as u32 / 2 * 2;
            if cell.strip == StandStrip::StandBlink {
                assert_eq!(cell.index, drawing - stand::BLINK_FROM, "image {frame}");
                if blinked_tours.last() != Some(&(frame / TOUR)) {
                    blinked_tours.push(frame / TOUR);
                }
            } else {
                assert_eq!(cell, stand::Cell { strip: StandStrip::Stand, index: drawing }, "image {frame}");
            }
            assert_eq!((picture.mouth, picture.boil_seed, picture.ghost), (None, 0, Vec2::ZERO));
        }
        assert_eq!(blinked_tours, [0, 2, 3, 5], "quatre clignements en 20 s, toujours aux mêmes tours");
        // la même image donne le même dessin
        let again = look.picture(&c);
        look.update(&c, look.frame, false, false);
        assert_eq!(look.picture(&c), again);
    }

    #[test]
    fn her_float_and_her_hair_share_one_tour() {
        let (mut look, mut c) = (dressed(true, false), capri(&dressed(true, false)));
        let tops: Vec<f32> =
            (0..TOUR).map(|frame| at(&mut look, &mut c, frame, false, false).rect.min.y).collect();
        let lowest = tops.iter().cloned().fold(f32::MIN, f32::max);
        let highest = tops.iter().cloned().fold(f32::MAX, f32::min);
        // au plus bas (y le plus grand) quand ses cheveux sont au plus bas (dessins 24 à 30)
        assert!((24..30).all(|frame| tops[frame] == lowest), "{tops:?}");
        assert!((54..60).all(|frame| tops[frame] == highest), "{tops:?}");
        // et le tour suivant recommence pareil
        assert_eq!(at(&mut look, &mut c, TOUR + 26, false, false).rect.min.y, lowest);
    }

    #[test]
    fn standing_she_moves_her_mouth_only_while_she_talks() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        let quiet = at(&mut look, &mut c, 10, false, false);
        assert_eq!(quiet.mouth, None);
        let talking = at(&mut look, &mut c, 10, true, false);
        assert_eq!(talking.drawing, quiet.drawing, "la boucle continue dessous");
        assert_eq!(talking.rect, quiet.rect, "la bouche au même endroit qu'elle");
        let mouths: std::collections::HashSet<u32> = (10..40)
            .map(|frame| at(&mut look, &mut c, frame, true, false).mouth.expect("elle parle").index)
            .collect();
        assert!(mouths.len() >= 4, "{mouths:?}");
        // tenue, elle parle encore ; la musique ne change rien debout
        c.grab(Vec2::new(1050.0, 900.0), 0.0);
        let held = at(&mut look, &mut c, 41, true, true);
        assert!(matches!(held.drawing, Drawing::Stand(_)) && held.mouth.is_some());
    }

    /// Lâchée près du bord : elle s'y pose.
    fn land(look: &mut Look, c: &mut Capri, frame: u64) {
        c.step(0.0, frame);
        c.land(Perch { spot: SPOT, feet: EDGE });
        look.update(c, frame, false, false);
    }

    #[test]
    fn landing_she_appears_on_the_edge_then_sits_her_seat_on_it() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        at(&mut look, &mut c, 0, false, false);
        land(&mut look, &mut c, 5);
        let arrive = sheets::arriving().len();
        for frame in 5..5 + arrive {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Perch(cell) = picture.drawing else { panic!("{picture:?}") };
            assert_eq!(cell, sheets::Cell { strip: Strip::PerchArrive, index: (frame - 5) as u32 / 2 });
            // sa ligne d'assise sur le bord, à moins d'un pixel
            let seat = picture.rect.min + Vec2::new(picture.rect.size.x * cell.strip.seat_uv().x, 0.0);
            let seat_y = picture.rect.min.y + picture.rect.size.y * cell.strip.seat_uv().y;
            assert!((seat.x - EDGE.x).abs() <= 1.0 && (seat_y - EDGE.y).abs() <= 1.0, "{picture:?}");
        }
        // puis sa boucle, du dessin 0
        let first = at(&mut look, &mut c, 5 + arrive, false, false);
        assert_eq!(first.drawing, Drawing::Perch(sheets::Cell { strip: Strip::Perch, index: 0 }));
        let k = stand::perch_scale(HEIGHT);
        assert_eq!(first.rect.size, Vec2::new((196.0 * k).round(), (384.0 * k).round()));
        let seat_y = first.rect.min.y + first.rect.size.y * Strip::Perch.seat_uv().y;
        assert!((seat_y - EDGE.y).abs() <= 1.0);
        // sa fenêtre bouge : elle suit
        c.ride(Perch { spot: SPOT, feet: EDGE + Vec2::new(30.0, -10.0) });
        let moved = at(&mut look, &mut c, 5 + arrive + 1, false, false);
        assert_eq!(moved.rect.min, first.rect.min + Vec2::new(30.0, -10.0));
    }

    #[test]
    fn she_leaves_by_fading_out_where_she_sat_but_stands_at_once_when_grabbed() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        land(&mut look, &mut c, 0);
        let sitting = 100;
        at(&mut look, &mut c, sitting, false, false);
        let seated_rect = look.picture(&c).rect;
        // sa fenêtre se ferme : elle s'efface là où elle était, puis flotte debout
        c.take_off();
        look.update(&c, sitting, false, false);
        let leave = sheets::leaving().len();
        for frame in sitting..sitting + leave {
            let picture = at(&mut look, &mut c, frame, false, false);
            let Drawing::Perch(cell) = picture.drawing else { panic!("image {frame} : {picture:?}") };
            assert_eq!(cell.strip, Strip::PerchLeave);
            assert_eq!(picture.rect.min.y, seated_rect.min.y, "au même endroit");
            assert_eq!(c.stance(), Stance::Floating);
            let size = Vec2::new(c.size().x.round(), c.size().y.round());
            assert_eq!(c.pose().rect.size, size, "pas d'étirement tant qu'elle s'efface");
        }
        let after = at(&mut look, &mut c, sitting + leave, false, false);
        assert!(matches!(after.drawing, Drawing::Stand(_)), "{after:?}");
        assert!(after.rect.size.y > c.size().y, "puis un petit étirement");
        assert!((c.feet() - EDGE).length() < 0.5, "debout, les pieds là où elle était assise");

        // assise de nouveau, on l'attrape : debout tout de suite, sous le curseur
        land(&mut look, &mut c, 300);
        at(&mut look, &mut c, 400, false, false);
        let cursor = look.shown(&c).min + Vec2::new(10.0, 10.0); // le haut de ses cheveux
        let origin = look.grab_origin(&c, cursor).expect("assise");
        c.grab_at(cursor, origin, 0.0);
        let held = at(&mut look, &mut c, 401, false, false);
        assert!(matches!(held.drawing, Drawing::Stand(_)), "{held:?}");
        let top = look.shown(&c).min.y;
        assert!(cursor.y - top < 30.0 && cursor.y >= top, "elle tient par les cheveux ({top})");
    }

    #[test]
    fn seated_she_talks_then_sways_to_music_on_a_tour_boundary_and_talking_comes_first() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        land(&mut look, &mut c, 0);
        let since = sheets::arriving().len();
        let strip =
            |look: &mut Look, c: &mut Capri, frame, talking, music| match at(look, c, frame, talking, music)
                .drawing
            {
                Drawing::Perch(cell) => (cell.strip, cell.index),
                other => panic!("{other:?}"),
            };
        // assise, elle parle : la même boucle, la bouche en plus, image pour image
        assert_eq!(strip(&mut look, &mut c, since + 10, true, false), (Strip::PerchTalk, 10));
        assert_eq!(strip(&mut look, &mut c, since + 11, false, false), (Strip::Perch, 10));
        // la musique commence au milieu d'un tour : elle attend le début du suivant
        assert_eq!(strip(&mut look, &mut c, since + 20, false, true).0, Strip::Perch);
        assert_eq!(strip(&mut look, &mut c, since + TOUR - 1, false, true).0, Strip::Perch);
        assert_eq!(strip(&mut look, &mut c, since + TOUR, false, true), (Strip::PerchMusic, 0));
        assert!(look.is_grooving());
        // elle se met à parler : tout de suite
        assert_eq!(strip(&mut look, &mut c, since + TOUR + 8, true, true), (Strip::PerchTalk, 8));
        // elle se tait : sa boucle, jusqu'au tour suivant, où la musique reprend
        assert_eq!(strip(&mut look, &mut c, since + TOUR + 20, false, true).0, Strip::Perch);
        assert_eq!(strip(&mut look, &mut c, since + 2 * TOUR, false, true), (Strip::PerchMusic, 0));
        // la musique s'arrête : elle finit son tour, puis revient à sa boucle
        assert_eq!(strip(&mut look, &mut c, since + 2 * TOUR + 30, false, false).0, Strip::PerchMusic);
        assert_eq!(strip(&mut look, &mut c, since + 3 * TOUR, false, false).0, Strip::Perch);
        assert!(!look.is_grooving());
        // une image sautée (elle était cachée) ne la perd pas
        assert_eq!(strip(&mut look, &mut c, since + 7 * TOUR + 3, false, true).0, Strip::PerchMusic);
    }

    #[test]
    fn seated_she_blinks_on_the_same_tours_as_standing() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        land(&mut look, &mut c, 0);
        let since = sheets::arriving().len();
        for tour in 0..8 {
            let frame = since + tour * TOUR + u64::from(sheets::BLINK_FROM) + 2;
            let Drawing::Perch(cell) = at(&mut look, &mut c, frame, false, false).drawing else { panic!() };
            assert_eq!(cell.strip == Strip::PerchBlink, blinks(tour), "tour {tour}");
        }
    }

    #[test]
    fn without_her_sheets_she_is_drawn_as_before() {
        // ni debout ni assise : son dessin d'avant, qui bout, avec sa copie violette, et elle se
        // tient debout sur les fenêtres
        let (mut look, mut c) = (dressed(false, false), capri(&dressed(false, false)));
        assert_eq!(look.standing_size(), Vec2::new(220.0 * HEIGHT / 420.0, HEIGHT));
        let picture = at(&mut look, &mut c, 3, true, true);
        assert_eq!(picture.drawing, Drawing::Cutout);
        assert_eq!(picture.mouth, None, "pas de bouche sur son dessin d'avant");
        assert!(picture.ghost != Vec2::ZERO);
        assert_eq!(look.leave_frames(), 0);
        land(&mut look, &mut c, 10);
        let perched = at(&mut look, &mut c, 11, false, true);
        assert_eq!(perched.drawing, Drawing::Cutout);
        assert!(!look.wants_music(&c), "sans pose assise, la musique ne sert à rien");
        assert_eq!(look.stature(), Stature::standing(look.standing_size(), look.standing_feet(), HEIGHT));

        // debout seulement : sa boucle debout, debout sur les fenêtres
        let (mut look, mut c) = (dressed(true, false), capri(&dressed(true, false)));
        land(&mut look, &mut c, 10);
        assert!(matches!(at(&mut look, &mut c, 40, false, false).drawing, Drawing::Stand(_)));
        assert_eq!(c.pose().rect, look.picture(&c).rect, "debout sur le bord, comme avant");

        // assise seulement : son dessin d'avant debout, ses planches assises sur les fenêtres
        let (mut look, mut c) = (dressed(false, true), capri(&dressed(false, true)));
        assert_eq!(at(&mut look, &mut c, 1, false, false).drawing, Drawing::Cutout);
        land(&mut look, &mut c, 2);
        assert!(matches!(at(&mut look, &mut c, 3, false, false).drawing, Drawing::Perch(_)));
        assert!(look.wants_music(&c));
    }

    #[test]
    fn seated_she_needs_much_less_room_above_the_edge() {
        let look = dressed(true, true);
        let stature = look.stature();
        let k = stand::perch_scale(HEIGHT);
        assert_eq!(stature.height, HEIGHT);
        assert!((stature.reach.up - 237.0 * k).abs() < 1e-3 && (stature.reach.down - 114.0 * k).abs() < 1e-3);
        // à la taille d'origine sur un écran Retina : 341 pixels (171 points) au-dessus du bord,
        // contre 300 points (et plus, avec la marge de sa case) debout
        assert!((341.0..342.0).contains(&stature.reach.up), "{stature:?}");
        let standing = Stature::standing(look.standing_size(), look.standing_feet(), HEIGHT);
        assert!(standing.reach.up > HEIGHT && stature.reach.up < HEIGHT * 0.62, "{standing:?}");
        // de sa main à son genou, sur le bord : 66 pixels de planche de chaque côté
        assert!((stature.rest.0 - 66.0 * k).abs() < 1e-3 && (stature.rest.1 - 66.0 * k).abs() < 1e-3);
        // et toujours plus que ce qu'il faut à ses pieds debout
        assert!(stature.rest.0 >= FOOT * HEIGHT);
    }

    #[test]
    fn she_is_caught_by_the_mask_of_the_pose_drawn() {
        let (mut look, mut c) = (dressed(true, true), capri(&dressed(true, true)));
        let standing = at(&mut look, &mut c, 0, false, false).rect;
        let middle = standing.min + standing.size * 0.5;
        assert!(look.is_over(&c, middle));
        assert!(!look.is_over(&c, standing.min - Vec2::new(5.0, 5.0)));
        land(&mut look, &mut c, 1);
        let since = 1 + sheets::arriving().len();
        let seated = at(&mut look, &mut c, since + 2, false, false).rect;
        let px = |x: u32, y: u32| {
            seated.min + Vec2::new(seated.size.x * x as f32 / 196.0, seated.size.y * y as f32 / 384.0)
        };
        // le masque assis : plein de (10, 40) à (150, 370) seulement
        assert!(look.is_over(&c, px(80, 200)));
        assert!(!look.is_over(&c, px(170, 200)), "hors de sa silhouette assise");
        assert!(!look.is_over(&c, px(80, 20)));
    }
}
