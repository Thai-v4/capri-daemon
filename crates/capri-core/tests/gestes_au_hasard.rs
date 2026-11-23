//! Des gestes au hasard, à graines fixes, dans l'ordre où la boucle de l'app les traite : entre
//! deux pas, les menus, les raccourcis et la souris (cacher, montrer, attraper, lâcher, « Changer
//! de place », sa taille, ses cases) ; puis le pas lui-même, comme `Running::tick` (son pas, le
//! suivi de sa fenêtre comme `perch_view`, son dessin, sa vie) ; et, autour, un bureau qui bouge
//! (des fenêtres qui glissent, se ferment, s'ouvrent, passent devant) et le temps qui passe, par à-coups.
//!
//! Ce qui doit toujours tenir, quoi qu'il arrive :
//! - « cachée » et son état sont d'accord (`Look::is_hidden`), et jamais elle n'est cachée, ni
//!   entre deux places, sans qu'on l'ait cachée : la fenêtre ne disparaît pas sous elle ;
//! - jamais coincée : montrée, elle est de nouveau là peu après ; cachée, elle finit de s'effacer ;
//! - là et debout, son tour avance (son flottement et sa boucle ne sont jamais figés) ;
//! - posée, avec ses planches assises, elle est dessinée assise, sa ligne d'assise sur son bord ;
//! - en chemin, elle va sur un bord où elle tient, ou debout en bas de l'écran, jamais sous le
//!   curseur ; la liste des fenêtres n'est lue qu'une fois par décision (et relue à l'arrivée sur
//!   un bord) ;
//! - la même graine donne la même vie.
//!
//! Rapide (quelques secondes en version de débogage), sans écran ni bruit.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use capri_core::look::{Drawing, Look, PerchMasks, Wardrobe};
use capri_core::perch::{self, Seen, Stature};
use capri_core::roam::{AVOID, BY_HAND, Busy, Choice, Event, FLOOR, Hold, Rng, Roamer, SEATED, World};
use capri_core::sheets::{self, life, stand};
use capri_core::{AlphaMask, Capri, Rect, Stance, Vec2};

const SCREEN: Rect = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
/// Ses tailles possibles sur un écran Retina, de 40 % à 140 % (600 : sa taille d'origine).
const HEIGHTS: [f32; 5] = [240.0, 360.0, 600.0, 780.0, 840.0];
/// Combien de pas par vie, et combien de vies par garde-robe.
const STEPS: usize = 4000;
const SEEDS: u64 = 100;
/// Montrée, elle est de nouveau là en moins de ça (en images) ; cachée, partie en moins de ça.
const SETTLED_WITHIN: u64 = 150;

fn full(width: u32, height: u32) -> AlphaMask {
    AlphaMask::from_rgba(width, height, &vec![255; (width * height * 4) as usize], 96, 0)
}

fn wardrobe(stand_: bool, perch_: bool, life_: bool) -> Wardrobe {
    Wardrobe {
        cutout: full(220, 420),
        stand: stand_.then(|| full(stand::CELL_WIDTH, sheets::CELL_HEIGHT)),
        perch: perch_.then(|| PerchMasks {
            narrow: full(sheets::NARROW, sheets::CELL_HEIGHT),
            wide: full(sheets::WIDE, sheets::CELL_HEIGHT),
        }),
        life: life_.then(|| full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
    }
}

/// Le bureau : ses fenêtres (de la plus en avant à la plus en arrière), le curseur, la case
/// « S'asseoir sur les fenêtres », et combien de fois la liste a été lue.
struct Desk {
    windows: Vec<Seen>,
    cursor: Option<Vec2>,
    perch: bool,
    reads: usize,
}

impl World for Desk {
    fn screen(&self) -> Rect {
        SCREEN
    }
    fn cursor(&self) -> Option<Vec2> {
        self.cursor
    }
    fn perch_allowed(&self) -> bool {
        self.perch
    }
    fn seen(&mut self, _stature: Stature) -> Option<Vec<Seen>> {
        self.reads += 1;
        Some(self.windows.clone())
    }
    fn recheck(&mut self, capri: &mut Capri, stature: Stature) {
        ride(capri, self, stature);
    }
}

/// Posée, elle suit sa fenêtre ou la quitte, comme `perch_view` (à chaque pas : sa cadence ne
/// change rien à ce qui est vérifié ici).
fn ride(capri: &mut Capri, desk: &Desk, stature: Stature) {
    let Some(spot) = capri.perched() else { return };
    if !desk.perch {
        return capri.take_off();
    }
    match perch::follow(&desk.windows, SCREEN, stature, spot) {
        Ok(perch) => capri.ride(perch),
        Err(_) => capri.take_off(),
    }
}

fn carried(capri: &Capri) -> bool {
    matches!(capri.stance(), Stance::Held { .. } | Stance::Thrown { .. })
}

/// Le petit hasard des gestes, à part de celui de sa vie.
struct Dice(Rng);

impl Dice {
    fn chance(&mut self, one_in: u64) -> bool {
        self.0.next_u64().is_multiple_of(one_in)
    }
    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.0.unit() as f32
    }
    fn below(&mut self, n: usize) -> usize {
        (self.0.next_u64() % n as u64) as usize
    }
}

/// Ce que les vies ont exercé, pour s'assurer que l'essai passe bien par chaque chemin.
#[derive(Debug, Default)]
struct Tally {
    to_ledge: usize,
    to_floor: usize,
    stays: usize,
    arrived: usize,
    arrived_seated: usize,
    arrived_lost: usize,
    refused: usize,
    turned_back: usize,
    hidden_on_the_way: usize,
    grabbed_appearing: usize,
    /// Des pas où elle se balançait, où l'on se parlait, où le curseur était sur elle.
    grooving: usize,
    talking: usize,
    hovered: usize,
    /// Des gestes qui lui ont fait quitter son bord.
    touched_off: usize,
}

/// Une vie : `seed` pour ses gestes et sa vie. Rend une empreinte de ce qui s'est passé, ou ce
/// qui n'a pas tenu.
fn live(seed: u64, (stand_, perch_, life_): (bool, bool, bool), tally: &mut Tally) -> Result<u64, String> {
    let mut dice = Dice(Rng::new(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ 0x5eed));
    let mut look = Look::new(wardrobe(stand_, perch_, life_), HEIGHTS[2]);
    let mut capri = Capri::new(look.standing_size(), SCREEN, Vec2::new(2400.0, 1200.0));
    capri.set_feet(look.standing_feet());
    capri.set_leave_frames(look.leave_frames());
    let mut desk = Desk {
        windows: vec![
            Seen { id: 1, rect: Rect::new(Vec2::new(300.0, 900.0), Vec2::new(1400.0, 900.0)) },
            Seen { id: 2, rect: Rect::new(Vec2::new(1500.0, 700.0), Vec2::new(1200.0, 1000.0)) },
        ],
        cursor: Some(Vec2::new(100.0, 100.0)),
        perch: true,
        reads: 0,
    };
    let mut next_id = 3;
    let mut roamer = Roamer::new(seed, 0.0, true);
    let mut moving = false;
    let (mut hidden, mut talking, mut music) = (false, false, false);
    let mut frame = 0u64;
    capri.step(0.0, frame);
    look.enter(&mut capri, frame);
    // depuis quand elle n'est pas là, montrée ; depuis quand on l'a cachée
    let mut away_since: Option<u64> = None;
    let mut toggled_at = 0u64;
    let mut decisions = 0usize;
    // « Changer de place » demandé et pas encore servi ; sa dernière arrivée debout en bas de l'écran
    let mut asked = false;
    let mut floor_arrival: Option<f64> = None;
    let mut print = DefaultHasher::new();
    let mut story: Vec<String> = Vec::new();
    let fail = |what: String, story: &[String]| {
        Err(format!("{what} ; derniers gestes : {:?}", &story[story.len().saturating_sub(10)..]))
    };

    for _ in 0..STEPS {
        // le temps passe, par à-coups
        frame += 1 + dice.below(3) as u64;
        if dice.chance(400) {
            frame += 24 * (30 + dice.below(400) as u64);
            story.push(format!("{frame} : le temps passe"));
            // ce qui se jouait est fini : ce qui suit est nouveau
            away_since = None;
        }
        let now = frame as f64 / 24.0;

        // --- entre deux pas : le bureau, les menus, les raccourcis, la souris ---
        if dice.chance(20) {
            desk.cursor = Some(Vec2::new(dice.between(0.0, 3024.0), dice.between(0.0, 1964.0)));
        }
        if !desk.windows.is_empty() && dice.chance(40) {
            let index = dice.below(desk.windows.len());
            let (dx, dy) = (dice.between(-200.0, 200.0), dice.between(-150.0, 150.0));
            desk.windows[index].rect.min += Vec2::new(dx, dy);
        }
        if !desk.windows.is_empty() && dice.chance(250) {
            let index = dice.below(desk.windows.len());
            desk.windows.remove(index);
            story.push(format!("{frame} : une fenêtre se ferme"));
        }
        if desk.windows.len() < 6 && dice.chance(150) {
            let min = Vec2::new(dice.between(-200.0, 2800.0), dice.between(0.0, 1800.0));
            let size = Vec2::new(dice.between(200.0, 2000.0), dice.between(100.0, 1200.0));
            desk.windows.insert(0, Seen { id: next_id, rect: Rect::new(min, size) });
            next_id += 1;
        }
        if desk.windows.len() > 1 && dice.chance(200) {
            let index = dice.below(desk.windows.len());
            let raised = desk.windows.remove(index);
            desk.windows.insert(0, raised);
        }
        if dice.chance(120) {
            talking = !talking;
        }
        if dice.chance(150) {
            music = !music;
        }
        // « S'asseoir sur les fenêtres » (`App::toggle_perch` : `perch_view::set_enabled`, puis
        // `roam_view::touched`)
        if dice.chance(500) {
            desk.perch = !desk.perch;
            let was = capri.perched().is_some();
            if !desk.perch && was {
                capri.take_off();
            }
            roamer.touched(now, &capri);
            if was && capri.perched().is_none() {
                tally.touched_off += 1;
                if (roamer.next() - (now + BY_HAND.min)).abs() > 1e-6 {
                    return fail(
                        format!("image {frame} : décochée, elle repartira à {}", roamer.next()),
                        &story,
                    );
                }
            }
            story.push(format!("{frame} : s'asseoir sur les fenêtres {}", desk.perch));
        }
        if dice.chance(600) {
            roamer.set_enabled(!roamer.is_enabled(), now);
        }
        // cacher, montrer (`App::toggle_hidden`, `Running::vanish`, `Running::come_back`)
        if dice.chance(200) {
            hidden = !hidden;
            toggled_at = frame;
            away_since = None;
            if hidden {
                if look.is_traveling() {
                    tally.hidden_on_the_way += 1;
                }
                look.vanish(&mut capri, frame);
            } else {
                if look.is_gone() {
                    capri.step(0.0, frame);
                    let stature = look.stature();
                    ride(&mut capri, &desk, stature);
                }
                look.appear(&mut capri, frame);
                let waiting = roamer.next();
                roamer.came_back(now, &capri);
                asked = false;
                if roamer.next() < waiting {
                    return fail(format!("image {frame} : revenue, son attente raccourcie"), &story);
                }
            }
            story.push(format!("{frame} : cachée {hidden}"));
        }
        // sa taille (`Running::resize` : `perch_view::resized`, puis `roam_view::touched`)
        if dice.chance(300) {
            look.set_height(HEIGHTS[dice.below(HEIGHTS.len())]);
            capri.set_size(look.standing_size());
            let stature = look.stature();
            let was = capri.perched().is_some();
            ride(&mut capri, &desk, stature);
            roamer.touched(now, &capri);
            if was && capri.perched().is_none() {
                tally.touched_off += 1;
                if (roamer.next() - (now + BY_HAND.min)).abs() > 1e-6 {
                    return fail(
                        format!("image {frame} : trop grande, elle repartira à {}", roamer.next()),
                        &story,
                    );
                }
            }
            story.push(format!("{frame} : taille"));
        }
        // « Changer de place » (cachée, l'app n'en fait rien)
        if !hidden && dice.chance(80) {
            roamer.ask();
            asked = true;
            story.push(format!("{frame} : changer de place"));
        }
        // attraper : là où elle est dessinée, si quelque chose l'attrape (`handle_overlay`)
        if !hidden && !capri.is_held() && dice.chance(60) {
            let picture = look.picture(&capri);
            let cursor = picture.rect.min + picture.rect.size * 0.5;
            if look.is_over(&capri, cursor) {
                if look.is_traveling() || !look.is_here() {
                    tally.grabbed_appearing += 1;
                }
                desk.cursor = Some(cursor);
                match look.grab_origin(&capri, cursor) {
                    Some(origin) => capri.grab_at(cursor, origin, now),
                    None => capri.grab(cursor, now),
                }
                story.push(format!("{frame} : attrapée"));
            }
        }
        // tenue : le curseur l'emmène, parfois près du haut d'une fenêtre ; puis lâchée
        if capri.is_held()
            && let Some(cursor) = desk.cursor
        {
            let grab = cursor - capri.position();
            let target = match desk.windows.first() {
                Some(window) if dice.chance(3) => {
                    Vec2::new(dice.between(window.rect.min.x, window.rect.max().x), window.rect.min.y - 20.0)
                }
                _ => capri.feet() + Vec2::new(dice.between(-80.0, 80.0), dice.between(-80.0, 80.0)),
            };
            let feet = capri.feet() - capri.position();
            desk.cursor = Some(target - feet + grab);
            if dice.chance(8) {
                let fast = dice.chance(2);
                if fast {
                    let away = desk.cursor.map(|c| c + Vec2::new(900.0, -300.0));
                    if let Some(c) = away {
                        capri.drag_to(c, now + 0.03);
                        desk.cursor = Some(c);
                    }
                }
                capri.release(now + 0.03);
                story.push(format!("{frame} : lâchée{}", if fast { ", lancée" } else { "" }));
            }
        }
        if hidden != look.is_hidden() {
            return fail(
                format!("image {frame} : cachée {hidden}, mais is_hidden {}", look.is_hidden()),
                &story,
            );
        }

        // --- le pas (`Running::tick`) : la boucle n'en fait pas quand elle est partie ---
        if look.is_gone() {
            continue;
        }
        if capri.is_held()
            && let Some(cursor) = desk.cursor
        {
            capri.drag_to(cursor, now);
        }
        capri.step(1.0 / 24.0, frame);
        let stature = look.stature();
        // `perch_view::tick` : lâchée doucement, ou au bout d'un lancer, elle se pose peut-être
        let now_moving = carried(&capri);
        if std::mem::replace(&mut moving, now_moving)
            && capri.stance() == Stance::Floating
            && desk.perch
            && let Some(perch) = perch::landing(&desk.windows, SCREEN, stature, capri.feet())
        {
            capri.land(perch);
        }
        ride(&mut capri, &desk, stature);
        look.update(&capri, frame, talking, music && look.wants_music(&capri));
        let hovered = desk.cursor.is_some_and(|cursor| look.is_over(&capri, cursor));
        // ce qui la retient, vu juste avant son pas
        let (here, grooving, held_back) = (look.is_here(), look.is_grooving(), carried(&capri));
        tally.grooving += usize::from(grooving);
        tally.talking += usize::from(talking && here);
        tally.hovered += usize::from(hovered && here);
        let busy = Busy { talking, hovered };
        let events = roamer.step(&mut look, &mut capri, &mut desk, busy, now, frame);

        // --- ce qui doit tenir ---
        for event in events {
            story.push(format!("{frame} : {event:?}"));
            // elle part, ou apparaît : une nouvelle suite commence
            if matches!(event, Event::Leaves { .. } | Event::Arrived { .. } | Event::TurnedBack) {
                away_since = None;
            }
            format!("{event:?}").hash(&mut print);
            match event {
                Event::Leaves { to, asked: by_hand, .. } => {
                    decisions += 1;
                    // jamais pendant qu'elle est retenue ; d'elle-même, jamais le curseur sur elle,
                    // jamais la case décochée
                    if !here
                        || talking
                        || grooving
                        || held_back
                        || (!by_hand && (hovered || !roamer.is_enabled()))
                    {
                        return fail(
                            format!(
                                "image {frame} : partie, retenue (là {here}, parole {talking}, musique {grooving}, \
                                 tenue {held_back}, curseur {hovered})"
                            ),
                            &story,
                        );
                    }
                    if by_hand && !std::mem::take(&mut asked) {
                        return fail(format!("image {frame} : partie « à la demande » sans demande"), &story);
                    }
                    let height = look.stature().height;
                    match to {
                        Choice::Ledge(perch) => {
                            tally.to_ledge += 1;
                            // posée là, elle y resterait (au centième de pixel près : `Spot` garde
                            // une distance au côté de sa fenêtre, recalculée en flottants)
                            let fits = perch::follow(&desk.windows, SCREEN, look.stature(), perch.spot);
                            let stays = |p: &perch::Perch| {
                                p.spot.window == perch.spot.window && (p.feet - perch.feet).length() < 0.01
                            };
                            if !fits.as_ref().is_ok_and(stays) {
                                return fail(
                                    format!("image {frame} : un bord où elle ne tient pas : {fits:?}"),
                                    &story,
                                );
                            }
                            if under_cursor(desk.cursor, perch.feet, look.stature().reach, height) {
                                return fail(format!("image {frame} : sur un bord, sous le curseur"), &story);
                            }
                        }
                        Choice::Floor(feet) => {
                            tally.to_floor += 1;
                            let standing =
                                Stature::standing(look.standing_size(), look.standing_feet(), height);
                            let floor = SCREEN.max().y - FLOOR * height - standing.reach.down;
                            if (feet.y - floor).abs() > 0.5 {
                                return fail(format!("image {frame} : debout ailleurs qu'en bas"), &story);
                            }
                            if under_cursor(desk.cursor, feet, standing.reach, height) {
                                return fail(format!("image {frame} : debout, sous le curseur"), &story);
                            }
                            // d'elle-même, debout, de loin en loin seulement
                            if !by_hand && floor_arrival.is_some_and(|at| now - at < SEATED.min) {
                                return fail(format!("image {frame} : debout, elle saute trop tôt"), &story);
                            }
                        }
                        Choice::Stay => {
                            return fail(format!("image {frame} : en chemin vers nulle part"), &story);
                        }
                    }
                }
                Event::Stays { asked: by_hand, .. } => {
                    decisions += 1;
                    tally.stays += 1;
                    // sur cet écran, le bas est toujours libre quelque part : une demande ne finit
                    // jamais en « nulle part où aller »
                    if by_hand {
                        return fail(format!("image {frame} : demandée, elle reste où elle est"), &story);
                    }
                }
                Event::Arrived { seated, lost } => {
                    tally.arrived += 1;
                    // on lui parle : elle revient sur ses pas, elle n'apparaît pas ailleurs
                    if talking {
                        return fail(
                            format!("image {frame} : apparue ailleurs pendant qu'on lui parle"),
                            &story,
                        );
                    }
                    if seated {
                        tally.arrived_seated += 1;
                        let spot = capri.perched().expect("assise, donc posée");
                        if perch::follow(&desk.windows, SCREEN, look.stature(), spot).is_err() {
                            return fail(
                                format!("image {frame} : arrivée sur un bord qui ne la porte pas"),
                                &story,
                            );
                        }
                    } else if !lost {
                        floor_arrival = Some(now);
                    }
                    if lost {
                        tally.arrived_lost += 1;
                    }
                }
                Event::TurnedBack => {
                    tally.turned_back += 1;
                    if !talking {
                        return fail(
                            format!("image {frame} : revenue sur ses pas sans qu'on lui parle"),
                            &story,
                        );
                    }
                }
                Event::Refused(hold) => {
                    tally.refused += 1;
                    let fair = match hold {
                        Hold::Carried => held_back,
                        Hold::Away => !here,
                        Hold::Talking => talking,
                        Hold::Music => grooving,
                    };
                    if !fair || !std::mem::take(&mut asked) {
                        return fail(format!("image {frame} : refus sans raison : {hold:?}"), &story);
                    }
                }
            }
        }
        if look.is_between() {
            return fail(format!("image {frame} : effacée entre deux places, et pas déplacée"), &story);
        }
        if hidden != look.is_hidden() {
            return fail(
                format!("image {frame} : cachée {hidden}, mais is_hidden {}", look.is_hidden()),
                &story,
            );
        }
        if roamer.heading().is_some() && !look.is_traveling() {
            return fail(format!("image {frame} : une destination, mais pas en chemin"), &story);
        }
        if desk.reads > decisions {
            return fail(format!("{} lectures pour {decisions} décisions", desk.reads), &story);
        }
        // jamais coincée
        if !hidden && !look.is_here() {
            let since = *away_since.get_or_insert(frame);
            if frame - since > SETTLED_WITHIN && frame - toggled_at > SETTLED_WITHIN {
                return fail(format!("image {frame} : montrée, mais pas là depuis l'image {since}"), &story);
            }
        } else {
            away_since = None;
        }
        if hidden && !look.is_gone() && frame - toggled_at > SETTLED_WITHIN {
            return fail(format!("image {frame} : cachée, mais à moitié effacée"), &story);
        }
        // là et debout, son tour avance
        if !hidden && look.is_here() && capri.stance() == Stance::Floating {
            let mut probe = capri.clone();
            probe.step(0.0, frame + 60);
            if probe.tour() == capri.tour() {
                return fail(format!("image {frame} : là, debout, son tour figé"), &story);
            }
        }
        // posée, avec ses planches assises : dessinée assise (sauf en train de s'effacer debout,
        // posée entre-temps), et toujours sur son bord, même en train d'y apparaître ou de s'en effacer
        if look.has_perch()
            && !capri.is_held()
            && let Some(seat) = capri.seat()
        {
            let picture = look.picture(&capri);
            match picture.drawing {
                Drawing::Perch(cell) => {
                    let uv = cell.strip.seat_uv();
                    let line =
                        picture.rect.min + Vec2::new(picture.rect.size.x * uv.x, picture.rect.size.y * uv.y);
                    if (line - seat).length() > 2.0 {
                        return fail(
                            format!("image {frame} : assise à {line:?}, son bord en {seat:?}"),
                            &story,
                        );
                    }
                }
                _ if !look.is_away() => {
                    return fail(
                        format!("image {frame} : posée, mais dessinée debout : {picture:?}"),
                        &story,
                    );
                }
                _ => {}
            }
        }
        // son empreinte : où elle est, ce qu'on dessine
        let picture = look.picture(&capri);
        (picture.rect.min.x as i64, picture.rect.min.y as i64, capri.perched().map(|spot| spot.window))
            .hash(&mut print);
    }
    Ok(print.finish())
}

/// Le curseur serait-il sur elle, le point posé en `at`, la place `reach` autour, avec la marge de
/// `roam::AVOID` ?
fn under_cursor(cursor: Option<Vec2>, at: Vec2, reach: perch::Reach, height: f32) -> bool {
    let margin = AVOID * height - 0.5;
    cursor.is_some_and(|c| {
        at.x - reach.left - margin < c.x
            && c.x < at.x + reach.right + margin
            && at.y - reach.up - margin < c.y
            && c.y < at.y + reach.down + margin
    })
}

const WARDROBES: [(bool, bool, bool); 5] = [
    (true, true, true),
    (true, false, true),
    (false, true, true),
    (true, true, false),
    (false, false, false),
];

#[test]
fn random_gestures_never_leave_her_stuck_hidden_or_off_her_edge() {
    let mut failures = Vec::new();
    let mut total = Tally::default();
    for wardrobe in WARDROBES {
        let mut tally = Tally::default();
        for seed in 0..SEEDS {
            if let Err(what) = live(seed, wardrobe, &mut tally) {
                failures.push(format!("graine {seed}, garde-robe {wardrobe:?} : {what}"));
            }
        }
        println!("{wardrobe:?} : {tally:?}");
        // dans chaque garde-robe, des départs vérifiés, des arrivées, des retenues
        let (_, perch_sheets, _) = wardrobe;
        assert!(tally.to_ledge > 50 && tally.to_floor > 10 && tally.arrived > 50, "{wardrobe:?} : {tally:?}");
        assert!(
            tally.talking > 1000 && tally.hovered > 100 && tally.turned_back + tally.refused > 50,
            "{tally:?}"
        );
        assert!(!perch_sheets || tally.grooving > 100, "{wardrobe:?} : {tally:?}");
        total.to_ledge += tally.to_ledge;
        total.to_floor += tally.to_floor;
        total.stays += tally.stays;
        total.arrived_seated += tally.arrived_seated;
        total.arrived_lost += tally.arrived_lost;
        total.refused += tally.refused;
        total.turned_back += tally.turned_back;
        total.hidden_on_the_way += tally.hidden_on_the_way;
        total.grabbed_appearing += tally.grabbed_appearing;
        total.touched_off += tally.touched_off;
    }
    for failure in failures.iter().take(8) {
        println!("{failure}");
    }
    assert!(failures.is_empty(), "{} vies en échec sur {}", failures.len(), SEEDS as usize * WARDROBES.len());
    // chaque chemin a bien été exercé
    assert!(total.stays > 5 && total.arrived_seated > 50 && total.arrived_lost > 0, "{total:?}");
    assert!(
        total.turned_back > 10 && total.hidden_on_the_way > 0 && total.grabbed_appearing > 0,
        "{total:?}"
    );
    assert!(total.touched_off > 10, "{total:?}");
}

#[test]
fn the_same_seed_gives_the_same_life() {
    let mut tally = Tally::default();
    for seed in [3, 17] {
        let first = live(seed, WARDROBES[0], &mut tally).expect("une vie");
        assert_eq!(live(seed, WARDROBES[0], &mut tally), Ok(first), "graine {seed}");
    }
    assert_ne!(live(3, WARDROBES[0], &mut tally), live(17, WARDROBES[0], &mut tally));
}
