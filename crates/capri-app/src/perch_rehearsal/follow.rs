//! `CAPRI_PERCH_REHEARSAL=1` au lancement de la vraie app (version de débogage) : mesurer, sans
//! personne devant l'écran, comment elle suit une fenêtre qu'on déplace, et ce que ça coûte.
//!
//! 1. elle flotte quelques secondes : le processeur au repos ;
//! 2. un second processus (`capri perch window`) ouvre une petite fenêtre d'essai à lui, au milieu
//!    de l'écran, sans prendre le clavier ; Capri y est posée comme si on l'avait lâchée juste
//!    au-dessus de son bord ;
//! 3. posée, la fenêtre immobile : 0,6 s au repos, puis 0,6 s comme si un bouton de la souris
//!    était enfoncé (l'indice qui la presse ; aucun vrai clic) : le processeur dans les deux cas ;
//! 4. la fenêtre fait un va-et-vient par programme pendant deux secondes, le « bouton » toujours
//!    enfoncé comme pendant un vrai déplacement : le processeur pendant le suivi, combien de
//!    positions elle a prises, et son retard sur la fenêtre ;
//! 5. la fenêtre se ferme, Capri quitte. En tout, la fenêtre d'essai reste à l'écran moins de
//!    quatre secondes.
//!
//! `CAPRI_PERCH_REHEARSAL=avant` fait la même chose avec la cadence d'avant ce réglage (au plus une
//! lecture par image d'animation, la boucle à 60 Hz), pour comparer. Le journal ne dit que des
//! nombres.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use capri_core::perch::DeskFrame;
use capri_core::{Capri, Rect, Vec2};
use capri_platform::desk;

/// La fenêtre d'essai au repos : son coin haut gauche et sa taille, en points depuis le haut
/// gauche de l'écran principal. Son bord à 560 points du haut laisse la place à Capri.
#[cfg(target_os = "macos")]
const WINDOW: (f64, f64, f64, f64) = (556.0, 560.0, 400.0, 240.0);
/// Son va-et-vient : amplitude (points) et période (secondes) ; jusqu'à 785 points par seconde.
#[cfg(target_os = "macos")]
const SWING: f64 = 250.0;
#[cfg(target_os = "macos")]
const SWING_PERIOD: f64 = 2.0;
/// Durées des étapes, en secondes.
const WARMUP: f64 = 1.5;
const FLOATING: f64 = 5.0;
const LANDED: f64 = 0.25;
const STILL: f64 = 0.6;
const PRESSED: f64 = 0.6;
const MOVING: f64 = 2.0;
const GIVE_UP: f64 = 25.0;

/// L'heure, en microsecondes : la seule horloge que les deux processus partagent.
pub fn wall_us() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_micros() as u64)
}

/// Ce que Capri a lu et dessiné pendant que la fenêtre bougeait.
#[derive(Debug, Default)]
pub struct Trace {
    /// La cadence d'avant ce réglage.
    pub legacy: bool,
    looked_at: Option<u64>,
    /// (heure, bord gauche lu de sa fenêtre, en pixels)
    reads: Vec<(u64, f32)>,
    /// (heure avant le dessin, heure après sa présentation, bord gauche de sa fenêtre qu'elle
    /// suivait alors, en pixels)
    draws: Vec<(u64, u64, f32)>,
    /// Ce qu'a pris chaque lecture : (durée, temps de processeur du fil), en microsecondes.
    took: Vec<(u64, u64)>,
}

impl Trace {
    pub fn timed(&mut self, wall: Duration, before: Option<Duration>, after: Option<Duration>) {
        let cpu = after.zip(before).map_or(0, |(after, before)| (after - before).as_micros() as u64);
        self.took.push((wall.as_micros() as u64, cpu));
    }

    pub fn read(&mut self, rect: Option<Rect>) {
        if let Some(rect) = rect {
            self.reads.push((wall_us(), rect.min.x));
        }
    }

    pub fn drawn(&mut self, before: u64, capri: &Capri) {
        if let Some(spot) = capri.perched() {
            self.draws.push((before, wall_us(), spot.left));
        }
    }

    /// La cadence d'avant : au plus une lecture par image d'animation (24 par seconde).
    pub fn same_frame_as_before(&mut self, now: f64) -> bool {
        let frame = (now * capri_core::FRAME_RATE) as u64;
        self.looked_at.replace(frame) == Some(frame)
    }
}

#[derive(Clone, Copy)]
enum Stage {
    Warmup,
    Floating { since: f64 },
    Opening,
    Landed { since: f64 },
    Still { since: f64 },
    Pressed { since: f64 },
    Moving { since: f64 },
    Closing,
}

pub struct Follow {
    legacy: bool,
    start: Instant,
    stage: Stage,
    /// (étape qui commence, heure depuis le début, temps de processeur du processus, réveils de la
    /// boucle, lectures de la liste, images dessinées)
    marks: Vec<(&'static str, f64, Duration, u64, u64, u64)>,
    wakes: u64,
    /// Les lectures et les images comptées par `PerchView`, au dernier pas.
    counted: (u64, u64),
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    lines: Option<Receiver<String>>,
    /// Pas de la boucle pendant que la fenêtre bougeait.
    ticks: u64,
    trace: Option<Trace>,
    /// (heure, bord gauche de la fenêtre, en points), tel que l'autre processus l'a posé
    timeline: Vec<(u64, f64)>,
    staleness: Option<String>,
    frame: Option<DeskFrame>,
}

impl Follow {
    /// `None` si `CAPRI_PERCH_REHEARSAL` n'est pas là.
    pub fn from_env() -> Option<Self> {
        let value = std::env::var("CAPRI_PERCH_REHEARSAL").ok()?;
        let legacy = value == "avant";
        log::info!(
            "répétition du suivi d'une fenêtre ({})",
            if legacy { "cadence d'avant" } else { "nouvelle cadence" }
        );
        Some(Self {
            legacy,
            start: Instant::now(),
            stage: Stage::Warmup,
            marks: Vec::new(),
            wakes: 0,
            counted: (0, 0),
            child: None,
            stdin: None,
            lines: None,
            ticks: 0,
            trace: None,
            timeline: Vec::new(),
            staleness: None,
            frame: None,
        })
    }

    /// Un pas, à chaque tour de boucle ; `true` quand c'est fini et qu'il faut quitter.
    pub fn step(&mut self, r: &mut crate::Running) -> bool {
        let t = self.start.elapsed().as_secs_f64();
        self.wakes += 1;
        self.counted = (r.perch.reads, r.perch.drawn);
        if t > GIVE_UP {
            log::error!("répétition du suivi : trop longue, abandonnée");
            self.stop_child();
            return true;
        }
        let line = self.lines.as_ref().and_then(|lines| lines.try_recv().ok());
        match self.stage {
            Stage::Warmup if t >= WARMUP => {
                self.mark("flottante", t);
                self.stage = Stage::Floating { since: t };
            }
            Stage::Floating { since } if t - since >= FLOATING => {
                self.mark("fenêtre demandée, en attente", t);
                if let Err(err) = self.spawn(r.overlay.refresh_rate()) {
                    log::error!("répétition du suivi : {err}");
                    return true;
                }
                self.stage = Stage::Opening;
            }
            Stage::Opening => {
                let Some(line) = line else { return false };
                let Some(number) = line.strip_prefix("fenetre ").and_then(|n| n.parse().ok()) else {
                    log::error!("répétition du suivi : la fenêtre d'essai ne s'est pas ouverte");
                    self.stop_child();
                    return true;
                };
                self.mark("posée, son atterrissage", t);
                if !self.land(r, number) {
                    log::error!("répétition du suivi : elle ne s'est pas posée sur la fenêtre d'essai");
                    self.stop_child();
                    return true;
                }
                self.stage = Stage::Landed { since: t };
            }
            Stage::Landed { since } if t - since >= LANDED => {
                self.mark("posée, fenêtre immobile", t);
                self.stage = Stage::Still { since: t };
            }
            Stage::Still { since } if t - since >= STILL => {
                self.mark("posée, un bouton de la souris enfoncé, fenêtre immobile", t);
                r.perch.pretend_pressed = true;
                self.stage = Stage::Pressed { since: t };
            }
            Stage::Pressed { since } if t - since >= PRESSED => {
                if r.capri.perched().is_none() {
                    log::error!("répétition du suivi : elle a quitté la fenêtre immobile");
                    self.stop_child();
                    return true;
                }
                self.mark("suivi de la fenêtre qui bouge, bouton enfoncé", t);
                r.perch.trace = Some(Trace { legacy: self.legacy, ..Trace::default() });
                let told = self.stdin.as_mut().is_some_and(|stdin| writeln!(stdin, "bouge").is_ok());
                if !told {
                    self.stop_child();
                    return true;
                }
                self.ticks = 0;
                self.stage = Stage::Moving { since: t };
            }
            Stage::Moving { since } => {
                self.ticks += 1;
                if line.as_deref() == Some("arret") || t - since > MOVING + 2.0 {
                    self.mark("fin", t);
                    r.perch.pretend_pressed = false;
                    self.trace = r.perch.trace.take();
                    self.frame = Some(r.overlay.desk_frame());
                    self.stage = Stage::Closing;
                }
            }
            Stage::Closing => {
                let rest = self.lines.as_ref().map(|lines| lines.try_iter().collect::<Vec<_>>());
                for line in line.into_iter().chain(rest.unwrap_or_default()) {
                    if let Some(rest) = line.strip_prefix("pos ") {
                        let mut numbers = rest.split(' ');
                        let us = numbers.next().and_then(|n| n.parse().ok());
                        let x = numbers.next().and_then(|n| n.parse().ok());
                        if let (Some(us), Some(x)) = (us, x) {
                            self.timeline.push((us, x));
                        }
                    } else if let Some(rest) = line.strip_prefix("reflet ") {
                        self.staleness = Some(rest.to_owned());
                    } else if line == "fin" {
                        self.report(r.overlay.refresh_rate());
                        self.stop_child();
                        return true;
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn mark(&mut self, stage: &'static str, t: f64) {
        let (reads, drawn) = self.counted;
        self.marks.push((stage, t, desk::cpu_time().unwrap_or_default(), self.wakes, reads, drawn));
    }

    fn spawn(&mut self, hz: Option<f64>) -> std::io::Result<()> {
        let hz = hz.unwrap_or(60.0);
        let mut child = Command::new(std::env::current_exe()?)
            .args(["perch", "window", &hz.to_string()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| std::io::Error::other("pas de sortie"))?;
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        self.stdin = child.stdin.take();
        self.child = Some(child);
        self.lines = Some(receiver);
        Ok(())
    }

    fn stop_child(&mut self) {
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            // elle a fini seule, ou on l'arrête : sa fenêtre disparaît avec elle
            let ended = (0..20).any(|_| {
                std::thread::sleep(Duration::from_millis(50));
                child.try_wait().ok().flatten().is_some()
            });
            if !ended {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    /// Comme si on l'avait lâchée doucement, les pieds 8 pixels au-dessus du milieu du bord de
    /// la fenêtre d'essai.
    fn land(&mut self, r: &mut crate::Running, number: u64) -> bool {
        let found = desk::windows().ok().and_then(|ws| ws.into_iter().find(|w| w.id == number));
        let Some(window) = found else { return false };
        let rect = r.overlay.desk_frame().to_overlay(window.rect);
        let feet = Vec2::new(rect.min.x + rect.size.x / 2.0, rect.min.y - 8.0);
        let (now, at) = (r.seconds(), r.capri.position());
        r.capri.grab(at, now);
        r.capri.drag_to(at + (feet - r.capri.feet()), now);
        r.capri.release(now + 1.0);
        r.perch.rehearse_settle(&mut r.capri, &r.overlay, now);
        r.capri.perched().is_some()
    }

    fn report(&self, hz: Option<f64>) {
        log::info!("répétition du suivi : écran à {hz:?} Hz");
        for pair in self.marks.windows(2) {
            let ((stage, t0, cpu0, wakes0, reads0, drawn0), (_, t1, cpu1, wakes1, reads1, drawn1)) =
                (pair[0], pair[1]);
            let (cpu, seconds) = ((cpu1 - cpu0).as_secs_f64(), t1 - t0);
            log::info!(
                "répétition du suivi : {stage}, {seconds:.2} s : {:.1} ms de processeur, {:.2} % d'un cœur ; \
                 par seconde : {:.0} réveils, {:.1} lectures, {:.1} images",
                cpu * 1000.0,
                cpu / seconds * 100.0,
                (wakes1 - wakes0) as f64 / seconds,
                (reads1 - reads0) as f64 / seconds,
                (drawn1 - drawn0) as f64 / seconds
            );
        }
        if let Some(staleness) = &self.staleness {
            log::info!(
                "répétition du suivi : la liste des fenêtres après un déplacement par programme (justes du \
                 premier coup, mesurés, médiane µs, 90 % µs, max µs, jamais) : {staleness}"
            );
        }
        let (Some(trace), Some(frame)) = (&self.trace, self.frame) else { return };
        let (Some(&(first, _)), Some(&(last, _))) = (self.timeline.first(), self.timeline.last()) else {
            return;
        };
        let seconds = (last - first) as f64 / 1e6;
        let settled = first + 300_000;
        let points = |px: f32| f64::from(px / frame.pixels_per_unit + frame.origin.x);
        // depuis quand l'autre processus avait posé la fenêtre là (au plus 300 ms plus tôt)
        let age = |us: u64, x: f64| {
            self.timeline
                .iter()
                .rev()
                .filter(|&&(t, _)| t <= us && us - t < 300_000)
                .find(|&&(_, set)| (set - x).abs() < 0.3)
                .map(|&(t, _)| us - t)
        };
        let during = |us: u64| (first..=last + 20_000).contains(&us);
        let draws: Vec<(u64, u64, f32)> = trace.draws.iter().copied().filter(|d| during(d.1)).collect();
        let reads: Vec<(u64, f32)> = trace.reads.iter().copied().filter(|r| during(r.0)).collect();
        let mut positions: Vec<f32> = draws.iter().map(|d| d.2).collect();
        positions.dedup();
        let per_second = |n: usize| n as f64 / seconds;
        log::info!(
            "répétition du suivi : {seconds:.2} s de mouvement ; réveils de la boucle : {} ({:.0} par seconde) ; \
             lectures : {} ({:.0}/s) ; images : {} ({:.0}/s) ; positions différentes : {} ({:.0}/s) ; \
             la fenêtre déplacée {} fois ({:.0}/s)",
            self.ticks,
            self.ticks as f64 / seconds,
            reads.len(),
            per_second(reads.len()),
            draws.len(),
            per_second(draws.len()),
            positions.len(),
            per_second(positions.len()),
            self.timeline.len(),
            per_second(self.timeline.len())
        );
        let read_age: Vec<u64> =
            reads.iter().filter(|r| r.0 >= settled).filter_map(|&(us, x)| age(us, points(x))).collect();
        let shown_age: Vec<u64> =
            draws.iter().filter(|d| d.1 >= settled).filter_map(|&(_, us, x)| age(us, points(x))).collect();
        let render: Vec<u64> = draws.iter().map(|&(before, after, _)| after - before).collect();
        let read_to_shown: Vec<u64> = draws
            .iter()
            .filter_map(|&(_, after, _)| reads.iter().rev().find(|r| r.0 <= after).map(|r| after - r.0))
            .collect();
        // Sans bouton de la souris enfoncé (la fenêtre bouge par programme), le premier mouvement
        // n'est vu qu'à la lecture lente suivante, jusqu'à un quart de seconde plus tard : dit à
        // part, puis écarté du reste. À la main, le bouton enfoncé la presse avant le mouvement.
        if let Some(shown) = draws.iter().find(|d| (points(d.2) - self.timeline[0].1).abs() > 0.3) {
            log::info!(
                "répétition du suivi : premier mouvement montré {} ms après le début du déplacement",
                (shown.1 - first) / 1000
            );
        }
        // à chaque déplacement de la fenêtre, l'écart entre elle et la dernière image présentée
        let gap: Vec<u64> = self
            .timeline
            .iter()
            .filter(|&&(us, _)| us >= settled)
            .filter_map(|&(us, x)| {
                let shown = draws.iter().rev().find(|d| d.1 <= us)?;
                Some(((x - points(shown.2)).abs() * 10.0).round() as u64)
            })
            .collect();
        log::info!(
            "répétition du suivi : à partir de 0,3 s, écart entre la fenêtre posée et la dernière image \
             (dixièmes de point) : {}",
            stats(gap)
        );
        log::info!("répétition du suivi : âge de la position lue (µs) : {}", stats(read_age));
        log::info!(
            "répétition du suivi : âge de la position dessinée, à la présentation (µs) : {}",
            stats(shown_age)
        );
        log::info!("répétition du suivi : de la lecture à la présentation (µs) : {}", stats(read_to_shown));
        log::info!("répétition du suivi : dessin et présentation (µs) : {}", stats(render));
        log::info!(
            "répétition du suivi : une lecture de la liste, durée (µs) : {}",
            stats(trace.took.iter().map(|t| t.0).collect())
        );
        log::info!(
            "répétition du suivi : une lecture de la liste, processeur du fil (µs) : {}",
            stats(trace.took.iter().map(|t| t.1).collect())
        );
    }
}

/// « n valeurs, médiane, 90 %, max ».
fn stats(mut values: Vec<u64>) -> String {
    if values.is_empty() {
        return "aucune".to_owned();
    }
    values.sort_unstable();
    let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
    format!("{} valeurs, médiane {}, 90 % sous {}, max {}", values.len(), at(0.5), at(0.9), at(1.0))
}

/// `capri perch window <Hz>` : la fenêtre d'essai, dans un processus à part. Elle s'ouvre, dit son
/// numéro, attend « bouge » (cinq secondes au plus), fait son va-et-vient pendant deux secondes à
/// la cadence donnée, en notant après chaque déplacement quand la liste des fenêtres le montre,
/// puis dit ce qu'elle a fait et se ferme.
#[cfg(target_os = "macos")]
pub fn moving_window(hz: f64) -> i32 {
    let (x0, y, width, height) = WINDOW;
    let Some(window) = desk::TestWindow::open(x0, y, width, height) else {
        println!("pas de fenêtre d'essai ici");
        return 1;
    };
    window.pump(Duration::from_millis(300));
    let number = window.number();
    println!("fenetre {number}");
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while receiver.try_recv().ok().as_deref() != Some("bouge") {
        if Instant::now() > deadline {
            return 1;
        }
        window.pump(Duration::from_millis(10));
    }

    let step = Duration::from_secs_f64(1.0 / hz.clamp(24.0, 240.0));
    let x_now = || {
        let found = desk::windows().ok().and_then(|ws| ws.into_iter().find(|w| w.id == number));
        found.map(|w| f64::from(w.rect.min.x))
    };
    let (mut timeline, mut seen_after, mut never, mut first_try) = (Vec::new(), Vec::new(), 0, 0);
    let start = Instant::now();
    let mut next = start;
    while start.elapsed().as_secs_f64() < MOVING {
        let t = start.elapsed().as_secs_f64();
        let x = (x0 + SWING * (std::f64::consts::TAU * t / SWING_PERIOD).sin()).round();
        window.move_to(x, y);
        timeline.push((wall_us(), x));
        // quand la liste des fenêtres le montre-t-elle ?
        let moved = Instant::now();
        let mut tries = 0;
        loop {
            tries += 1;
            if x_now() == Some(x) {
                seen_after.push(moved.elapsed().as_micros() as u64);
                first_try += usize::from(tries == 1);
                break;
            }
            if moved.elapsed() > Duration::from_millis(25) {
                never += 1;
                break;
            }
            window.pump(Duration::from_micros(500));
        }
        next += step;
        let now = Instant::now();
        if next > now {
            window.pump(next - now);
        } else {
            next = now;
        }
    }
    println!("arret");
    let measured = seen_after.len();
    seen_after.sort_unstable();
    let at = |q: f64| {
        let index = ((measured.max(1) - 1) as f64 * q).round() as usize;
        seen_after.get(index).copied().unwrap_or(0)
    };
    println!("reflet {first_try} {measured} {} {} {} {never}", at(0.5), at(0.9), at(1.0));
    for (us, x) in timeline {
        println!("pos {us} {x}");
    }
    drop(window);
    println!("fin");
    0
}

#[cfg(not(target_os = "macos"))]
pub fn moving_window(_hz: f64) -> i32 {
    println!("pas de fenêtre d'essai ici");
    1
}
