//! Des gestes au hasard, à graines fixes, dans l'ordre où la boucle de l'app les traite : entre
//! deux pas, les menus, les raccourcis et la souris (cacher, montrer, attraper, cliquer, lâcher,
//! « Changer de place », sa taille, ses cases), Claude Code qui finit son tour ; puis le pas
//! lui-même, comme `Running::tick` (son pas, le suivi de sa fenêtre comme `perch_view`, son dessin,
//! sa vie) ; et, autour, un bureau qui bouge (des fenêtres qui glissent, se ferment, s'ouvrent,
//! passent devant) et le temps qui passe, par à-coups. Ses gestes (sourire, éclat, se relever), sa
//! marche et sa planche en l'air s'y mêlent : attrapée, lâchée, cachée, on lui parle pendant
//! chacun.
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
//! - jamais deux gestes à la fois : ce qui est dessiné est le geste en cours, ou aucun ; ni en
//!   marchant, ni tenue ; un geste finit toujours (au plus tard à sa dernière image) ;
//! - la main (`capri_core::input::Hand`, comme l'app) : un appui bref est un clic, un appui long
//!   ou déplacé une prise, un glissé plus court qu'un pas une prise lâchée à la fin du pas suivant ;
//!   un clic sur elle restée tenue (le relâchement perdu) la lâche, sans geste ;
//! - tenue, avec sa planche en l'air : jamais immobile pendant que le curseur s'éloigne d'un bord ;
//!   loin des bords, sa mèche se rapproche du curseur d'au moins la moitié de son chemin à chaque
//!   pas, jusqu'à être sous lui ;
//! - elle ne marche jamais à reculons, toujours vers la gauche (là où elle regarde), et sa marche
//!   finit ; une vie sur trois la laisse aller au bout (rien ne la dérange en marchant), et elle
//!   arrive là où elle allait ou au bord de l'écran ; elle ne s'arrête en chemin que si on lui
//!   parle ;
//! - la même graine donne la même vie.
//!
//! Rapide (quelques secondes en version de débogage), sans écran ni bruit.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use capri_core::input::{Hand, Let};
use capri_core::look::{Drawing, Gesture, LONGEST_GESTURE, Look, Moves, PerchMasks, Wardrobe};
use capri_core::perch::{self, Seen, Stature};
use capri_core::roam::{
    AVOID, BY_HAND, Busy, Choice, Event, FLOOR, Hold, Rng, Roamer, SEATED, WALK_LONGEST, World,
};
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

/// Ses planches de gestes et de variantes, chacune là ou non : la marche, en l'air, se relever,
/// sourire, l'éclat assise, ses clignements glitchés (un bit chacune, dans cet ordre).
const ALL_MOVES: u8 = 0b11_1111;

fn wardrobe((stand_, perch_, life_, moves): (bool, bool, bool, u8)) -> Wardrobe {
    let on = |bit: u8| moves & (1 << bit) != 0;
    let body = || full(life::CELL_WIDTH, sheets::CELL_HEIGHT);
    Wardrobe {
        cutout: full(220, 420),
        stand: stand_.then(|| full(stand::CELL_WIDTH, sheets::CELL_HEIGHT)),
        perch: perch_.then(|| PerchMasks {
            narrow: full(sheets::NARROW, sheets::CELL_HEIGHT),
            wide: full(sheets::WIDE, sheets::CELL_HEIGHT),
        }),
        life: life_.then(|| full(life::CELL_WIDTH, sheets::CELL_HEIGHT)),
        moves: Moves {
            walk: on(0).then(body),
            dangle: on(1).then(body),
            glitch: on(2).then(body),
            grin: on(3).then(body),
            perch_glitch: on(4),
            perch_blink_glitch: on(5),
            stand_blink_glitch: on(5),
        },
    }
}

/// Les bords de l'écran que touche son image debout : leur normale vers l'extérieur.
fn edges(capri: &Capri) -> Vec<Vec2> {
    let (min, max) = (capri.position(), capri.position() + capri.size());
    let mut touching = Vec::new();
    if min.x <= 0.5 {
        touching.push(Vec2::new(-1.0, 0.0));
    }
    if min.y <= 0.5 {
        touching.push(Vec2::new(0.0, -1.0));
    }
    if max.x >= SCREEN.max().x - 0.5 {
        touching.push(Vec2::new(1.0, 0.0));
    }
    if max.y >= SCREEN.max().y - 0.5 {
        touching.push(Vec2::new(0.0, 1.0));
    }
    touching
}

/// Le seuil d'une prise, en pixels (`CLICK_SLOP` sur un écran Retina).
const SLOP: f32 = 8.0;

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
    /// Des clics (et ceux qui ont joué un geste), des fins de tour de Claude Code (et ses
    /// sourires), des fois où elle s'est relevée, des éclats d'elle-même.
    clicks: usize,
    clicked: usize,
    turn_ends: usize,
    cheers: usize,
    rises: usize,
    fidgets: usize,
    /// Des pas où elle pendait, où elle marchait ; ses marches, arrêtées en chemin ou non.
    dangling: usize,
    walking: usize,
    walks: usize,
    halted: usize,
    /// Des prises pendant un geste, pendant sa marche ; des fois où on lui parlait pendant un geste.
    grabbed_in_gesture: usize,
    grabbed_walking: usize,
    talked_in_gesture: usize,
    /// Ses marches allées au bout ; des pas tenue, loin des bords, où sa mèche s'est rapprochée ;
    /// des pas où elle s'est éloignée d'un bord avec le curseur.
    walks_arrived: usize,
    gliding: usize,
    off_an_edge: usize,
    /// Des prises nées d'un appui (par le pas, par le relâchement), des relâchements perdus, et des
    /// clics qui l'ont lâchée.
    grabs_by_step: usize,
    grabs_by_release: usize,
    stuck: usize,
    unstuck: usize,
}

/// Une vie : `seed` pour ses gestes et sa vie. Rend une empreinte de ce qui s'est passé, ou ce
/// qui n'a pas tenu.
fn live(seed: u64, dressed: (bool, bool, bool, u8), tally: &mut Tally) -> Result<u64, String> {
    let mut dice = Dice(Rng::new(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ 0x5eed));
    let mut look = Look::new(wardrobe(dressed), HEIGHTS[2]);
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
    // une vie sur cinq commence cachée pendant l'intro du lancement : partie d'un coup, sans
    // s'être jamais montrée (`Running::end_intro`, `Look::hide_unseen`) ; les autres, l'intro
    // jouée ou pas, commencent par se matérialiser
    if seed % 5 == 4 {
        look.hide_unseen(frame);
        hidden = true;
    } else {
        look.enter(&mut capri, frame);
    }
    // depuis quand elle n'est pas là, montrée ; depuis quand on l'a cachée
    let mut away_since: Option<u64> = None;
    let mut toggled_at = 0u64;
    let mut decisions = 0usize;
    // « Changer de place » demandé et pas encore servi ; sa dernière arrivée debout en bas de l'écran
    let mut asked = false;
    let mut floor_arrival: Option<f64> = None;
    // sa marche en cours : combien de pas de la boucle elle dure (chacun avance son corps d'un
    // vingt-quatrième de seconde, même quand le temps passe par à-coups), et d'où elle est partie
    let mut walk: Option<(u64, f32, f32)> = None;
    // une vie sur trois laisse ses marches aller au bout : rien ne la dérange pendant qu'elle marche
    let calm_life = seed.is_multiple_of(3);
    // la main ; l'appui en cours : quand le relâcher, et de combien le curseur bouge à chaque pas ;
    // le relâchement perdu (son menu ouvert pendant qu'on la tenait) : elle reste tenue
    let mut hand = Hand::new();
    let mut pressing: Option<(f64, Vec2)> = None;
    let mut stuck = false;
    // le curseur au pas précédent
    let mut last_cursor: Option<Vec2> = None;
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
        // une vie calme : rien ne la dérange pendant qu'elle marche
        let calm = calm_life && look.is_walking(&capri);
        if !calm && dice.chance(120) {
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
        if !calm && dice.chance(200) {
            hidden = !hidden;
            toggled_at = frame;
            away_since = None;
            if hidden {
                hand.cancel();
                pressing = None;
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
        // sa taille (`Running::resize` : `perch_view::resized`, puis `roam_view::touched`) ; pas
        // pendant qu'on la tient (la molette l'ignore alors, et son menu ne s'ouvre pas sous un
        // bouton enfoncé)
        if !calm && !capri.is_held() && dice.chance(300) {
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
        if !hidden && !calm && dice.chance(80) {
            roamer.ask();
            asked = true;
            story.push(format!("{frame} : changer de place"));
        }
        // on se parle (`Running::tick` : `Look::set_conversing`, avant son pas)
        look.set_conversing(talking);
        // Claude Code finit son tour (`Running::tick`, `BannerView::take_turn_end`)
        if dice.chance(150) {
            tally.turn_ends += 1;
            if look.cheer(&mut capri, frame) {
                tally.cheers += 1;
                story.push(format!("{frame} : Claude a fini, elle sourit"));
            }
        }
        // un appui sur elle, là où elle est dessinée, si quelque chose l'attrape (`handle_overlay`) :
        // relâché plus tard, le curseur immobile ou non ; ou appui, glissé et relâchement dans le
        // même passage. Restée tenue sans bouton (son relâchement perdu), un clic la lâche
        if !hidden && !calm && pressing.is_none() && (!capri.is_held() || stuck) && dice.chance(40) {
            let picture = look.picture(&capri);
            let cursor = picture.rect.min + picture.rect.size * 0.5;
            if look.is_over(&capri, cursor) {
                hand.press(cursor, now);
                desk.cursor = Some(cursor);
                let drift = if dice.chance(3) { 12.0 } else { 1.0 };
                let drift = Vec2::new(dice.between(-drift, drift), dice.between(-drift, drift));
                let quick = dice.chance(6);
                pressing = Some((if quick { now } else { now + f64::from(dice.between(0.0, 0.5)) }, drift));
                if quick {
                    desk.cursor = Some(cursor + Vec2::new(30.0, -10.0));
                }
                story.push(format!("{frame} : un appui"));
            }
        }
        // l'appui en cours : le curseur bouge un peu, ou pas ; relâché à son heure
        if let Some((release_at, drift)) = pressing {
            if now < release_at {
                desk.cursor = desk.cursor.map(|c| c + drift);
            } else {
                pressing = None;
                let held = capri.is_held();
                let gesture = look.gesture();
                match hand.release(desk.cursor, now, SLOP, held) {
                    Let::Click => {
                        tally.clicks += 1;
                        if let Some(started) = look.poke(&mut capri, frame) {
                            tally.clicked += 1;
                            if gesture.is_some() {
                                return fail(
                                    format!("image {frame} : {started:?} par-dessus {gesture:?}"),
                                    &story,
                                );
                            }
                            story.push(format!("{frame} : un clic, {}", started.describe()));
                        }
                    }
                    Let::Go => {
                        // un clic sur elle restée tenue : elle est lâchée, sans geste ; ou elle
                        // était déjà prise (l'appui devenu prise au pas), et c'est le lâcher
                        capri.release(now);
                        if stuck {
                            tally.unstuck += 1;
                            stuck = false;
                            if look.gesture() != gesture {
                                return fail(
                                    format!("image {frame} : un clic qui la lâche et la fait sourire"),
                                    &story,
                                );
                            }
                        }
                        story.push(format!("{frame} : relâchée"));
                    }
                    Let::Grab(grab) => {
                        // un glissé plus court qu'un pas : prise maintenant, lâchée à la fin du pas
                        tally.grabs_by_release += 1;
                        stuck = false;
                        if look.hold(&mut capri, &grab) {
                            story.push(format!("{frame} : prise et lâchée au même passage"));
                        }
                    }
                }
                if capri.is_held() && !hand.is_letting_go() {
                    return fail(format!("image {frame} : relâchée, et toujours tenue"), &story);
                }
            }
        }
        // tenue : le curseur l'emmène, parfois près du haut d'une fenêtre ; puis lâchée (parfois le
        // relâchement se perd : elle reste tenue, jusqu'à un clic sur elle)
        if capri.is_held()
            && pressing.is_none()
            && !stuck
            && !hand.is_letting_go()
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
                if dice.chance(3) {
                    stuck = true;
                    tally.stuck += 1;
                    story.push(format!("{frame} : relâchement perdu"));
                } else {
                    match hand.release(desk.cursor, now + 0.03, SLOP, true) {
                        Let::Go => capri.release(now + 0.03),
                        other => return fail(format!("image {frame} : lâchée, et {other:?}"), &story),
                    }
                    story.push(format!("{frame} : lâchée{}", if fast { ", lancée" } else { "" }));
                }
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
        // un appui qui dure ou se déplace devient une prise, faite là et quand il a commencé
        if let Some(grab) = hand.step(desk.cursor, now, SLOP) {
            tally.grabs_by_step += 1;
            if look.is_traveling() || !look.is_here() {
                tally.grabbed_appearing += 1;
            }
            tally.grabbed_in_gesture += usize::from(look.gesture().is_some());
            tally.grabbed_walking += usize::from(look.is_walking(&capri));
            stuck = false;
            if look.hold(&mut capri, &grab) {
                story.push(format!("{frame} : attrapée"));
                // le curseur que son corps connaît : le dernier relevé depuis l'appui
                last_cursor = grab.trail.last().map(|&(_, cursor)| cursor).or(Some(grab.at));
            }
        }
        // tenue, le curseur l'emmène ; avec sa planche en l'air, elle glisse vers sa mèche sous lui
        let dangling = capri.is_held() && look.is_dangling(&capri);
        let cursor_before = last_cursor;
        // l'écart de sa mèche au curseur, là où il était au pas précédent
        let (from, gap, touching) = (
            capri.position(),
            look.grip(&capri).zip(cursor_before).map(|(grip, cursor)| (grip - cursor).length()),
            edges(&capri),
        );
        if capri.is_held()
            && !hand.is_letting_go()
            && let Some(cursor) = desk.cursor
        {
            capri.drag_to(cursor, now);
            if dangling
                && let Some(before) = cursor_before
                && (cursor - before).length() > 0.5
            {
                let way = cursor - before;
                let away = touching.iter().all(|n| way.x * n.x + way.y * n.y < 0.0);
                if !touching.is_empty() && away {
                    tally.off_an_edge += 1;
                    if capri.position() == from {
                        return fail(
                            format!("image {frame} : immobile pendant que le curseur s'éloigne du bord"),
                            &story,
                        );
                    }
                }
                let now_gap = look.grip(&capri).map(|grip| (grip - cursor).length());
                if touching.is_empty()
                    && edges(&capri).is_empty()
                    && let (Some(g0), Some(g1)) = (gap, now_gap)
                {
                    tally.gliding += 1;
                    if g1 > (g0 - way.length() / 2.0).max(0.0) + 3.0 {
                        return fail(
                            format!(
                                "image {frame} : sa mèche à {g1} du curseur, à {g0} avant (curseur {})",
                                way.length()
                            ),
                            &story,
                        );
                    }
                }
            }
        }
        last_cursor = desk.cursor;
        let feet_before = capri.feet().x;
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
        let music_on = music && look.wants_music(&capri);
        let acting = look.gesture();
        look.update(&mut capri, frame, talking, music_on);
        if acting.is_some() && talking {
            tally.talked_in_gesture += 1;
            if look.gesture().is_some() {
                return fail(format!("image {frame} : on se parle, et {acting:?} continue"), &story);
            }
        }
        if look.gesture() == Some(Gesture::Rise) && acting != Some(Gesture::Rise) {
            tally.rises += 1;
        }
        // sa marche : jamais à reculons, toujours vers où elle regarde
        if look.is_walking(&capri) || walk.is_some() {
            let feet = capri.feet().x;
            if feet > feet_before + 1e-3 || (look.is_walking(&capri) && !life::FACING.allows(-1.0)) {
                return fail(
                    format!("image {frame} : elle marche à reculons ({feet_before} → {feet})"),
                    &story,
                );
            }
        }
        let hovered = desk.cursor.is_some_and(|cursor| look.is_over(&capri, cursor));
        // ce qui la retient, vu juste avant son pas
        let (here, grooving, held_back) = (look.is_here(), look.is_grooving(), carried(&capri));
        let walking_before = look.is_walking(&capri);
        let seated_before = look.is_seated(&capri) && capri.perched().is_some();
        tally.grooving += usize::from(grooving);
        tally.talking += usize::from(talking && here);
        tally.hovered += usize::from(hovered && here);
        let busy = Busy { talking, hovered };
        let events = roamer.step(&mut look, &mut capri, &mut desk, busy, now, frame);
        // une prise faite au relâchement : tenue pendant ce pas, lâchée à sa fin
        if let Some(at) = hand.let_go() {
            capri.release(at);
        }

        // --- ce qui doit tenir ---
        for event in events.iter().copied() {
            story.push(format!("{frame} : {event:?}"));
            // elle part, ou apparaît : une nouvelle suite commence
            if matches!(event, Event::Leaves { .. } | Event::Arrived { .. } | Event::TurnedBack) {
                away_since = None;
            }
            format!("{event:?}").hash(&mut print);
            match event {
                Event::Leaves { to, asked: by_hand, walking, .. } => {
                    decisions += 1;
                    if walking {
                        tally.walks += 1;
                        let Choice::Floor(to) = to else {
                            return fail(format!("image {frame} : en marchant vers un bord"), &story);
                        };
                        let feet = capri.feet();
                        let speed = look.walk_speed().unwrap_or(0.0);
                        if !look.is_walking(&capri)
                            || to.x >= feet.x
                            || (to.y - feet.y).abs() > 1.0
                            || f64::from((feet.x - to.x) / speed) > WALK_LONGEST
                        {
                            return fail(format!("image {frame} : une marche de {feet:?} à {to:?}"), &story);
                        }
                        walk = Some((0, feet.x, to.x));
                    }
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
                Event::Halted => {
                    tally.halted += 1;
                    if !talking || !walking_before || look.is_walking(&capri) {
                        return fail(
                            format!("image {frame} : arrêtée en chemin sans qu'on lui parle"),
                            &story,
                        );
                    }
                }
                Event::Fidgeted => {
                    tally.fidgets += 1;
                    if !seated_before || talking || grooving || music_on || !roamer.is_enabled() {
                        return fail(
                            format!("image {frame} : un éclat alors qu'elle n'est pas libre"),
                            &story,
                        );
                    }
                    if look.gesture() != Some(Gesture::PerchGlitch) {
                        return fail(format!("image {frame} : un éclat qui ne se joue pas"), &story);
                    }
                }
                Event::Refused(hold) => {
                    tally.refused += 1;
                    let fair = match hold {
                        Hold::Carried => held_back,
                        Hold::Away => !here,
                        Hold::Talking => talking,
                        Hold::Music => grooving,
                        Hold::Walking => walking_before,
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
        // un geste à la fois, et il finit ; ni en marchant, ni tenue
        if let (Some(gesture), Some(since)) = (look.gesture(), look.gesture_since()) {
            if frame >= since + gesture.frames() || gesture.frames() > LONGEST_GESTURE {
                return fail(format!("image {frame} : {gesture:?} depuis l'image {since}, pas fini"), &story);
            }
            if look.is_walking(&capri) || carried(&capri) || !look.is_here() {
                return fail(format!("image {frame} : {gesture:?} en marchant, tenue ou partie"), &story);
            }
        }
        let shown = look.picture(&capri);
        let drawn = match shown.drawing {
            Drawing::Life(cell) => match cell.strip {
                life::Strip::Glitch => Some(Gesture::Rise),
                life::Strip::Grin => Some(Gesture::Grin),
                _ => None,
            },
            Drawing::Perch(cell) if cell.strip == sheets::Strip::PerchGlitch => Some(Gesture::PerchGlitch),
            _ => None,
        };
        if drawn.is_some() && drawn != look.gesture() {
            return fail(format!("image {frame} : {drawn:?} dessiné, {:?} en cours", look.gesture()), &story);
        }
        // tenue, avec sa planche en l'air, elle pend (sa mèche vérifiée au glissé, plus haut)
        if look.is_dangling(&capri) {
            tally.dangling += 1;
        } else if capri.is_held() && look.is_here() && look.has_move(life::Strip::Dangle) {
            return fail(format!("image {frame} : tenue sans pendre"), &story);
        }
        // sa marche finit : elle arrive, ou elle s'arrête (on lui parle, une prise, cachée)
        if look.is_walking(&capri) {
            tally.walking += 1;
            if let Some((steps, _, _)) = walk.as_mut() {
                *steps += 1;
                if *steps > (WALK_LONGEST * 24.0) as u64 + 2 {
                    return fail(format!("image {frame} : elle marche depuis {steps} pas"), &story);
                }
            }
        } else if let Some((_, _, to)) = walk.take() {
            // une marche finie sans rien pour la déranger : elle arrive là où elle allait, ou au
            // bord de l'écran, et c'est une arrivée debout
            let arrived = events.contains(&Event::Arrived { seated: false, lost: false });
            if calm_life && !carried(&capri) && look.is_here() && !hidden {
                if !arrived {
                    return fail(format!("image {frame} : sa marche finie sans arriver"), &story);
                }
                let feet = capri.feet().x;
                if (feet - to).abs() > 1.0 && edges(&capri).is_empty() {
                    return fail(format!("image {frame} : arrivée en {feet}, elle allait en {to}"), &story);
                }
                if capri.stance() != Stance::Floating {
                    return fail(format!("image {frame} : arrivée, et {:?}", capri.stance()), &story);
                }
            }
            tally.walks_arrived += usize::from(arrived);
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
        // là et debout, son tour avance (un geste l'arrête au plus le temps du plus long)
        if !hidden && look.is_here() && capri.stance() == Stance::Floating {
            let mut probe = capri.clone();
            probe.step(0.0, frame + LONGEST_GESTURE + 1);
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

/// Debout, assise, ses planches de vie, et ses gestes (bits de [`ALL_MOVES`]).
const WARDROBES: [(bool, bool, bool, u8); 9] = [
    (true, true, true, ALL_MOVES),
    (true, true, true, 0),
    (true, false, true, ALL_MOVES),
    (false, true, true, ALL_MOVES),
    (true, true, false, ALL_MOVES),
    (false, false, false, 0),
    // la marche et sa planche en l'air seulement ; ses gestes seulement
    (true, true, true, 0b00_0011),
    (true, true, true, 0b11_1100),
    // sans sa planche en l'air ni se relever
    (true, true, true, 0b11_1001),
];

#[test]
fn random_gestures_never_leave_her_stuck_hidden_or_off_her_edge() {
    let mut failures = Vec::new();
    let mut total = Tally::default();
    for wardrobe in WARDROBES {
        let mut tally = Tally::default();
        let before = failures.len();
        for seed in 0..SEEDS {
            if let Err(what) = live(seed, wardrobe, &mut tally) {
                failures.push(format!("graine {seed}, garde-robe {wardrobe:?} : {what}"));
            }
        }
        println!("{wardrobe:?} : {tally:?}");
        // la cause d'un échec d'abord : les seuils ensuite
        for failure in failures[before..].iter().take(4) {
            println!("{failure}");
        }
        assert!(failures.is_empty(), "{} vies en échec ; voir plus haut", failures.len());
        // dans chaque garde-robe, des départs vérifiés, des arrivées, des retenues
        let (stand_sheets, perch_sheets, _, moves) = wardrobe;
        let on = |bit: u8| moves & (1 << bit) != 0;
        // et chacun de ses gestes, quand sa planche est là
        if stand_sheets {
            assert!(
                !on(0) || (tally.walks >= 3 && tally.walking > 100 && tally.walks_arrived >= 3),
                "{wardrobe:?} : {tally:?}"
            );
            assert!(!on(1) || (tally.gliding > 100 && tally.off_an_edge > 5), "{wardrobe:?} : {tally:?}");
            assert!(on(0) || tally.walks == 0, "{wardrobe:?} : {tally:?}");
            assert!(!on(1) || tally.dangling > 1000, "{wardrobe:?} : {tally:?}");
            assert!(on(1) || tally.dangling == 0, "{wardrobe:?} : {tally:?}");
            assert!(!on(2) || tally.rises > 50, "{wardrobe:?} : {tally:?}");
            assert!(!on(3) || (tally.cheers > 50 && tally.clicked > 50), "{wardrobe:?} : {tally:?}");
        }
        assert!(perch_sheets && on(4) || tally.fidgets == 0, "{wardrobe:?} : {tally:?}");
        assert!(tally.clicks > 100 && tally.turn_ends > 100, "{wardrobe:?} : {tally:?}");
        assert!(tally.grabs_by_step > 100 && tally.grabs_by_release > 10, "{wardrobe:?} : {tally:?}");
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
        total.grabbed_in_gesture += tally.grabbed_in_gesture;
        total.grabbed_walking += tally.grabbed_walking;
        total.talked_in_gesture += tally.talked_in_gesture;
        total.fidgets += tally.fidgets;
        total.halted += tally.halted;
        total.walks += tally.walks;
        total.walks_arrived += tally.walks_arrived;
        total.stuck += tally.stuck;
        total.unstuck += tally.unstuck;
    }
    println!("en tout : {total:?}");
    assert!(failures.is_empty(), "{} vies en échec sur {}", failures.len(), SEEDS as usize * WARDROBES.len());
    // chaque chemin a bien été exercé
    assert!(total.stays > 5 && total.arrived_seated > 50 && total.arrived_lost > 0, "{total:?}");
    assert!(
        total.turned_back > 10 && total.hidden_on_the_way > 0 && total.grabbed_appearing > 0,
        "{total:?}"
    );
    assert!(total.touched_off > 10, "{total:?}");
    // une prise pendant un geste, pendant sa marche ; on lui parle pendant un geste
    assert!(
        total.grabbed_in_gesture > 20 && total.grabbed_walking > 5 && total.talked_in_gesture > 20,
        "{total:?}"
    );
    // ses éclats assise, des marches arrêtées en chemin parce qu'on lui parle, d'autres allées au
    // bout ; des relâchements perdus, et des clics qui l'ont lâchée
    assert!(
        total.fidgets > 5 && total.halted > 2 && total.walks > 20 && total.walks_arrived > 20,
        "{total:?}"
    );
    assert!(total.stuck > 20 && total.unstuck > 10, "{total:?}");
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
