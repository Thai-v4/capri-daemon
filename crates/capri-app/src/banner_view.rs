//! Ce qui s'affiche au-dessus de Capri, côté application : une ligne de bandeau ou sa bulle de
//! parole, son image, et où elle se dessine.
//!
//! Quatre sources se partagent cette place, de la plus à la moins prioritaire :
//! 1. ce qu'on est en train de lui dire au micro : elle écoute, et montre ce qu'elle comprend ;
//! 2. ce que Capri est en train de dire (`chat::Chat`) : on vient de lui parler, on attend sa
//!    réponse, et rien ne doit la masquer — pas même Claude Code qui attend l'utilisateur, ce qui
//!    peut durer des minutes ;
//! 3. un message de l'app elle-même, pendant quelques secondes ;
//! 4. l'activité de Claude Code (`claude::Feed`), qui reprend la place dès qu'elle se tait.
//!
//! Avec le miroir, la ligne que Claude a écrite pour elle devient une prise de parole (2), mais
//! seulement quand elle est libre : personne ne lui parle au micro, elle ne répond pas à
//! l'utilisateur, et elle a fini ce qu'elle disait. Sinon la ligne attend (la plus récente
//! seulement, pas trop longtemps : `capri_claude::mirror::Waiting`). Cachée, elle ne la dit pas.
//!
//! L'animation et le placement sont dans `capri-core`, le tracé du texte dans `capri-render`.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use capri_chat::dictation::{Dictation, EndOfSpeech, Said};
use capri_claude::{Headline, Tone};
use capri_core::{Banner, Rect, Vec2, place_banner};
use capri_platform::{Ear, EarEvent};
use capri_render::banner::{BannerPainter, BannerStyle};
use capri_render::{Renderer, SpriteDraw, SpriteImage};

use crate::chat::Chat;
use crate::claude::Feed;
use crate::{brain_setup, ear_setup, hook_setup};

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
/// Les couleurs de ce qu'elle entend : c'est toi qui parles, pas elle.
const HEARING_STYLE: BannerStyle =
    BannerStyle { text: [232, 250, 255, 255], fill: NIGHT, border: [86, 204, 242, 255] };
/// Ce qu'elle affiche tant qu'elle n'a encore rien compris.
const LISTENING: &str = "Je t'écoute…";

/// Ce qui occupe la place au-dessus d'elle.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Shown {
    /// Une ligne : l'activité de Claude Code, ou un message de l'app.
    Line(Headline),
    /// Une page de ce que Capri dit ; `more` : une autre suit.
    Speech { text: String, more: bool },
    /// Ce qu'elle comprend de ce qu'on lui dit au micro.
    Hearing(String),
}

pub struct BannerView {
    feed: Option<Feed>,
    chat: Chat,
    /// Le dossier de ses réglages, où relire si le miroir est encore activé.
    settings: Option<PathBuf>,
    /// Son oreille, créée la première fois qu'on lui parle au micro.
    ear: Option<Ear>,
    /// La prise de parole en cours, s'il y en a une.
    dictation: Dictation,
    started: Instant,
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
            chat: Chat::new(settings.clone()),
            settings,
            ear: None,
            dictation: Dictation::new(),
            started: Instant::now(),
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
        self.pump_ear();
        self.chat.hold();
        self.refresh = true;
    }

    /// Le raccourci vient d'être enfoncé. Si elle écoute déjà en écoute libre, ce nouvel appui
    /// termine tout de suite et envoie ce qui a été compris ; si la touche est déjà enfoncée
    /// (répétition), rien ; sinon elle commence à écouter, la touche enfoncée : le relâchement
    /// dira si c'était un appui bref (écoute libre) ou maintenu (`listen_release`).
    pub fn listen_press(&mut self) {
        if self.dictation.is_free() {
            self.listen_stop();
        } else if !self.dictation.is_listening() {
            self.listen_begin(false);
        }
    }

    /// Le raccourci vient d'être relâché. Maintenu, cela termine ; après un appui bref, le micro
    /// reste ouvert et c'est la détection de fin de phrase (ou un nouvel appui) qui le fermera.
    pub fn listen_release(&mut self) {
        if self.dictation.released(self.now_ms()) {
            self.listen_stop();
        }
    }

    /// Le menu : un clic commence à écouter en écoute libre, un second termine tout de suite.
    #[cfg(any(target_os = "macos", windows))]
    pub fn listen_toggle(&mut self) {
        if self.dictation.is_listening() { self.listen_stop() } else { self.listen_begin(true) }
    }

    /// Elle se tait et écoute. Si elle ne peut pas écouter, elle dit pourquoi. `free` : écoute
    /// libre d'emblée (le menu), sans touche à maintenir.
    fn listen_begin(&mut self, free: bool) {
        self.chat.hush(); // on ne s'écoute pas parler, et on ne lui coupe pas la parole à moitié
        if self.ear.is_none() {
            self.ear = Ear::new();
        }
        let started = match &mut self.ear {
            Some(ear) => ear.start().map_err(|err| {
                log::info!("pas d'écoute : {err}");
                ear_setup::explain(&err)
            }),
            None => Err("Je ne sais pas encore t'écouter sur ce système"),
        };
        match started {
            Ok(()) if free => self.dictation.begin_free(self.now_ms()),
            Ok(()) => self.dictation.begin_held(self.now_ms()),
            Err(why) => self.notify(why, Tone::Failed),
        }
        self.refresh = true;
    }

    /// On a fini de lui parler : le micro se ferme, et ce qui a été dit part dès que le système
    /// a rendu son dernier mot.
    fn listen_stop(&mut self) {
        if !self.dictation.is_listening() {
            return;
        }
        if let Some(ear) = &mut self.ear {
            ear.stop();
        }
        self.dictation.end(self.now_ms());
        self.refresh = true;
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Prend en compte ce que son oreille a compris depuis le dernier appel.
    fn pump_ear(&mut self) {
        let Some(ear) = &mut self.ear else { return };
        let mut said = None;
        for event in ear.drain() {
            self.refresh = true;
            match event {
                EarEvent::Heard { text, last } => {
                    // la longueur seulement : ce qui se dit n'est jamais écrit dans le journal
                    log::debug!("entendu : {} caractères (définitif : {last})", text.chars().count());
                    said = said.or(self.dictation.transcribed(
                        &text,
                        last,
                        self.started.elapsed().as_millis() as u64,
                    ));
                }
                EarEvent::Failed => {
                    log::debug!("l'écoute s'est terminée sans texte définitif");
                    said = said.or(self.dictation.failed());
                }
                EarEvent::PermissionAnswered => {
                    let text = format!("C'est noté. Appuie sur {} pour me parler", ear_setup::SHORTCUT);
                    self.notice = Some((Headline { text, tone: Tone::Done }, Instant::now() + NOTICE_FOR));
                }
            }
        }
        let now = self.started.elapsed().as_millis() as u64;
        // la fin de la phrase, détectée seule (écoute libre), ou la borne dure : le micro se
        // ferme, il n'est jamais ouvert plus longtemps que cela
        if said.is_none()
            && let Some(why) = self.dictation.due(now)
        {
            log::debug!("fin de l'écoute : {why:?}");
            if why == EndOfSpeech::NothingHeard {
                // pas un mot : on ne laisse pas au système le temps d'un dernier texte
                ear.cancel();
                said = self.dictation.failed();
            } else {
                ear.stop();
                self.dictation.end(now);
            }
            self.refresh = true;
        }
        if said.is_none() {
            said = self.dictation.tick(now);
            if said.is_some() {
                ear.cancel(); // le dernier mot n'est pas venu : on n'attend plus rien de cette écoute
            }
        }
        match said {
            Some(Said::Message(message)) => self.say(&message),
            Some(Said::Nothing) => self.notify("Je n'ai rien entendu", Tone::Failed),
            None => {}
        }
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

    /// La fait taire : elle arrête de répondre, sa bulle s'efface, et la ligne de Claude Code qui
    /// attendait son tour est oubliée.
    #[cfg(any(target_os = "macos", windows))]
    pub fn hush(&mut self) {
        self.chat.hush();
        if let Some(feed) = &mut self.feed {
            feed.drop_said();
        }
        self.refresh = true;
    }

    /// Si elle est libre, elle dit la ligne que Claude Code a écrite pour elle : personne ne lui
    /// parle au micro, elle ne répond pas à l'utilisateur et ne parle pas déjà. Sinon la ligne
    /// attend son tour.
    fn relay_claude(&mut self) {
        if self.dictation.is_active() || self.chat.is_busy() {
            return;
        }
        let Some(said) = self.feed.as_mut().and_then(Feed::take_said) else { return };
        // le miroir a pu être désactivé depuis que le hook l'a lu : dans le doute, elle se tait
        let enabled = self
            .settings
            .as_deref()
            .is_some_and(|directory| hook_setup::load_mirror(directory).is_ok_and(|mirror| mirror.enabled));
        if !enabled {
            log::debug!("phrase de Claude Code ignorée : le miroir est désactivé");
            return;
        }
        // la longueur seulement : ce que Claude a écrit n'est jamais écrit dans le journal
        log::debug!("Claude Code lui fait dire {} caractères", said.text().chars().count());
        self.chat.relay(said.text());
        self.refresh = true;
    }

    /// Arrête sa voix tout de suite, sans toucher à sa bulle.
    #[cfg(any(target_os = "macos", windows))]
    pub fn silence(&mut self) {
        self.chat.silence();
    }

    /// Ses réglages de la voix ont changé : ce qu'elle dit continue avec les nouveaux.
    #[cfg(any(target_os = "macos", windows))]
    pub fn refresh_voice(&mut self) {
        self.chat.refresh_voice();
    }

    /// Repart de zéro : elle se tait et oublie la conversation.
    #[cfg(any(target_os = "macos", windows))]
    pub fn forget(&mut self) {
        self.chat.forget();
        if let Some(feed) = &mut self.feed {
            feed.drop_said();
        }
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
        self.pump_ear();
        self.relay_claude();
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
                        Shown::Hearing(text) => {
                            painter.paint_speech(text, false, &HEARING_STYLE, scale as f32)
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
        if let Some(heard) = self.dictation.heard() {
            let hearing =
                Shown::Hearing(if heard.is_empty() { LISTENING.to_owned() } else { heard.to_owned() });
            // ce qu'elle comprend s'allonge et se corrige : pas d'à-coup
            let already_hearing = matches!(self.banner.content(frame), Some(Shown::Hearing(_)));
            return if already_hearing {
                self.banner.revise(hearing, frame)
            } else {
                self.banner.set(Some(hearing), frame)
            };
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
