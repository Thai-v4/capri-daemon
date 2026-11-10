//! Ce qui s'affiche au-dessus de Capri, côté application : une ligne de bandeau ou sa bulle de
//! parole, son image, et où elle se dessine.
//!
//! Trois sources se partagent cette place, de la plus à la moins prioritaire :
//! 1. ce que Capri est en train de dire (`chat::Chat`) : on vient de lui parler, on attend sa
//!    réponse, et rien ne doit la masquer — pas même Claude Code qui attend l'utilisateur, ce qui
//!    peut durer des minutes ;
//! 2. un message de l'app elle-même, pendant quelques secondes ;
//! 3. l'activité de Claude Code (`claude::Feed`), qui reprend la place dès qu'elle se tait.
//!
//! L'animation et le placement sont dans `capri-core`, le tracé du texte dans `capri-render`.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use capri_claude::{Headline, Tone};
use capri_core::{Banner, Rect, Vec2, place_banner};
use capri_render::banner::{BannerPainter, BannerStyle};
use capri_render::{Renderer, SpriteDraw, SpriteImage};

use crate::brain_setup;
use crate::chat::Chat;
use crate::claude::Feed;

/// Le suivi de Claude Code est relu à cette cadence (en pas de 60 Hz) même sans nouveau message :
/// c'est ce qui fait expirer un texte.
const REFRESH_EVERY: u32 = 15;
/// Durée d'un message de l'app elle-même.
const NOTICE_FOR: Duration = Duration::from_secs(5);
/// Écart entre le bandeau et sa tête, et décalage de sa copie violette, en points.
const GAP_POINTS: f32 = 8.0;
const GHOST_POINTS: f32 = 1.5;

const NIGHT: [u8; 4] = [20, 16, 36, 240];
/// Les couleurs de sa bulle de parole.
const SPEECH_STYLE: BannerStyle =
    BannerStyle { text: [250, 246, 255, 255], fill: NIGHT, border: [214, 170, 255, 255] };

/// Ce qui occupe la place au-dessus d'elle.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Shown {
    /// Une ligne : l'activité de Claude Code, ou un message de l'app.
    Line(Headline),
    /// Une page de ce que Capri dit ; `more` : une autre suit.
    Speech { text: String, more: bool },
}

pub struct BannerView {
    feed: Option<Feed>,
    chat: Chat,
    banner: Banner<Shown>,
    /// Un message de l'app, affiché jusqu'à l'instant donné.
    notice: Option<(Headline, Instant)>,
    painter: Painter,
    painted: Option<Painted>,
    ticks: u32,
    refresh: bool,
    /// Dernier dessin (deux images d'animation) pour lequel sa parole a été relue.
    spoken_drawing: u64,
}

/// Le traceur de texte, dont la création (lecture des polices du système) se fait en tâche de fond.
enum Painter {
    Loading(Receiver<BannerPainter>),
    Ready(Box<BannerPainter>),
    Unavailable,
}

/// L'image actuellement confiée au rendu.
#[derive(Clone, Copy)]
struct Painted {
    generation: u64,
    size: Vec2,
}

impl BannerView {
    /// Commence à écouter Claude Code et à charger les polices.
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        let loading = std::thread::Builder::new().name("capri-polices".to_owned()).spawn(move || {
            let started = Instant::now();
            let painter = BannerPainter::new();
            log::debug!("polices du bandeau prêtes en {:?}", started.elapsed());
            let _ = sender.send(painter);
        });
        let painter = match loading {
            Ok(_) => Painter::Loading(receiver),
            Err(err) => {
                log::warn!("pas de bandeau : {err}");
                Painter::Unavailable
            }
        };
        let settings = brain_setup::config_dir()
            .inspect_err(|err| log::warn!("pas de dossier de réglages, donc pas de cerveau : {err:#}"))
            .ok();
        Self {
            feed: Feed::start(),
            chat: Chat::new(settings),
            banner: Banner::new(),
            notice: None,
            painter,
            painted: None,
            ticks: 0,
            refresh: false,
            spoken_drawing: u64::MAX,
        }
    }

    /// Quand Capri est cachée : continuer à suivre Claude Code, sans rien dessiner. Elle ne
    /// parle pas dans le vide : ce qu'elle disait attend son retour.
    pub fn listen_only(&mut self) {
        if let Some(feed) = &mut self.feed {
            feed.pump();
        }
        self.chat.pump();
        self.chat.hold();
        self.refresh = true;
    }

    /// Fait dire quelque chose à Capri pendant quelques secondes.
    pub fn notify(&mut self, text: impl Into<String>, tone: Tone) {
        self.notice = Some((Headline { text: text.into(), tone }, Instant::now() + NOTICE_FOR));
        self.refresh = true;
    }

    /// L'utilisateur lui dit quelque chose : elle répond dans sa bulle.
    pub fn say(&mut self, message: &str) {
        log::debug!("message de {} caractères", message.chars().count());
        self.chat.say(message);
        self.refresh = true;
    }

    /// La fait taire : elle arrête de répondre, et sa bulle s'efface.
    #[cfg(any(target_os = "macos", windows))]
    pub fn hush(&mut self) {
        self.chat.hush();
        self.refresh = true;
    }

    /// Arrête sa voix tout de suite, sans toucher à sa bulle.
    #[cfg(any(target_os = "macos", windows))]
    pub fn silence(&mut self) {
        self.chat.silence();
    }

    /// Repart de zéro : elle se tait et oublie la conversation.
    #[cfg(any(target_os = "macos", windows))]
    pub fn forget(&mut self) {
        self.chat.forget();
        self.notify("C'est oublié, on repart de zéro", Tone::Done);
    }

    /// La densité de l'écran a changé : l'image est à retracer.
    pub fn rescaled(&mut self) {
        self.painted = None;
    }

    /// À chaque pas : met à jour ce qui s'affiche et, si cela a changé, son image.
    pub fn update(&mut self, frame: u64, scale: f64, renderer: &mut Renderer) {
        let fresh = self.feed.as_mut().is_some_and(Feed::pump);
        self.chat.pump();
        self.ticks = self.ticks.wrapping_add(1);
        // sa parole avance « en deux », comme le reste de l'animation
        let speaking = self.chat.is_speaking() && frame / 2 != self.spoken_drawing;
        if fresh || speaking || self.refresh || self.ticks.is_multiple_of(REFRESH_EVERY) {
            self.refresh = false;
            self.spoken_drawing = frame / 2;
            self.choose(frame);
        }

        if let Painter::Loading(receiver) = &self.painter {
            match receiver.try_recv() {
                Ok(painter) => self.painter = Painter::Ready(Box::new(painter)),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    log::warn!("pas de bandeau : le chargement des polices a échoué");
                    self.painter = Painter::Unavailable;
                }
            }
        }

        match self.banner.pose(frame) {
            Some(pose) if self.painted.is_none_or(|painted| painted.generation != pose.generation) => {
                if let (Painter::Ready(painter), Some(shown)) =
                    (&mut self.painter, self.banner.content(frame))
                {
                    let image = match shown {
                        Shown::Line(headline) => {
                            painter.paint(&headline.text, &style(headline.tone), scale as f32)
                        }
                        Shown::Speech { text, more } => {
                            painter.paint_speech(text, *more, &SPEECH_STYLE, scale as f32)
                        }
                    };
                    let sprite = SpriteImage { width: image.width, height: image.height, rgba: &image.rgba };
                    match renderer.set_banner(Some(sprite)) {
                        Ok(()) => {
                            let size = Vec2::new(image.width as f32, image.height as f32);
                            self.painted = Some(Painted { generation: pose.generation, size });
                        }
                        Err(err) => log::warn!("bandeau non affiché : {err}"),
                    }
                }
            }
            Some(_) => {}
            None => {
                if self.painted.take().is_some() {
                    let _ = renderer.set_banner(None);
                }
            }
        }
    }

    /// Décide de ce qui occupe la place au-dessus d'elle à partir de l'image `frame`.
    fn choose(&mut self, frame: u64) {
        if self.notice.as_ref().is_some_and(|(_, until)| Instant::now() >= *until) {
            self.notice = None;
        }
        if let Some((text, more)) = self.chat.page() {
            let page = Shown::Speech { text, more };
            // un texte qui s'allonge ou une page qui tourne : pas d'à-coup
            let already_speaking = matches!(self.banner.content(frame), Some(Shown::Speech { .. }));
            return if already_speaking {
                self.banner.revise(page, frame)
            } else {
                self.banner.set(Some(page), frame)
            };
        }
        let line = match &self.notice {
            Some((headline, _)) => Some(headline.clone()),
            None => self.feed.as_mut().and_then(Feed::headline),
        };
        self.banner.set(line.map(Shown::Line), frame);
    }

    /// Où et comment dessiner le bandeau à l'image `frame`, s'il y a lieu. `capri` : là où elle
    /// se tient (sans son flottement, pour que le texte reste lisible) ; `bounds` : l'écran.
    pub fn draw(&self, frame: u64, capri: Rect, bounds: Rect, scale: f64) -> Option<SpriteDraw> {
        let pose = self.banner.pose(frame)?;
        let painted = self.painted.filter(|painted| painted.generation == pose.generation)?;
        let unit = scale as f32;
        let at = place_banner(capri, painted.size, bounds, GAP_POINTS * unit);
        let size = painted.size * pose.scale;
        let min = at + (painted.size - size) * 0.5;
        let ghost = (GHOST_POINTS * unit).round();
        Some(SpriteDraw {
            min: [min.x.round(), min.y.round()],
            size: [size.x.round(), size.y.round()],
            ghost: [ghost, ghost],
            // le texte ne « bout » pas : il doit rester lisible, et ne coûte rien au repos
            boil_seed: 0,
            boil: 0.0,
            opacity: pose.opacity,
        })
    }
}

/// Les couleurs du bandeau selon ce qu'il annonce.
fn style(tone: Tone) -> BannerStyle {
    match tone {
        Tone::Working => BannerStyle { text: [246, 240, 255, 255], fill: NIGHT, border: [115, 41, 224, 255] },
        // le rouge de Capri : c'est le seul cas où elle réclame ton attention
        Tone::Waiting => {
            BannerStyle { text: [255, 255, 255, 255], fill: [224, 35, 78, 245], border: [255, 214, 224, 255] }
        }
        Tone::Done => BannerStyle { text: [236, 255, 248, 255], fill: NIGHT, border: [64, 220, 170, 255] },
        Tone::Failed => BannerStyle { text: [255, 244, 236, 255], fill: NIGHT, border: [255, 138, 64, 255] },
    }
}
