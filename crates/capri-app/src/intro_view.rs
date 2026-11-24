//! Le film du lancement, « de tout près », côté application : ses images décodées une à une sur
//! un thread à part (« capri-intro »), peintes l'une après l'autre sur une seule toile de la carte
//! graphique, au rythme que décide sa feuille d'exposition (`capri_core::intro`), et dessinées avec
//! leur glitch (`capri_core::intro::glitch`) : la toile en bandes, chacune décalée, et derrière
//! elles leurs doubles teintés ([`film_layers`]).
//!
//! **Ce que ça pèse.** Une image de « de tout près » (1 920 × 1 080) en RGBA fait 8,3 Mo, les 22 en
//! feraient 182 : elles ne sont jamais toutes en mémoire, ni sur la carte graphique. Le thread ne
//! décode que l'image qu'on lui demande, une seule à la fois, et l'image suivante n'est demandée
//! qu'une fois la précédente peinte et lâchée : il n'existe jamais plus d'une image décodée. La
//! carte graphique n'en a qu'une, la toile, réécrite en place (`Renderer::paint_canvas`). Au pire, à
//! l'instant où une image est peinte : la toile (de la mémoire graphique, qui compte dans celle de
//! l'app sur un Mac à puce Apple), l'image décodée et la copie qu'en fait wgpu pour l'envoyer (rendue
//! une fois la copie faite), trois fois la taille d'une image : 24,9 Mo ; le reste du temps, la toile
//! et au plus une image qui attend son heure. En plus : le fichier compressé (de 10 à 140 Ko) et les
//! tampons du décodeur PNG. Elle est prémultipliée sur le thread, pas sur la boucle.
//!
//! **Son glitch** ne coûte rien de plus en mémoire : la même toile, dessinée en au plus
//! `glitch::MAX_BANDS` bandes, chacune avec ses deux doubles (une teinte du shader), soit au plus 39
//! dessins par image, recalculés seulement quand le dessin change. Sans glitch (force nulle), la
//! toile est dessinée d'un seul tenant, exactement comme avant.
//!
//! **La boucle n'attend jamais** : elle essaie de prendre l'image décodée (`try_recv`) quand son
//! heure est venue ; pas prête, le dessin précédent reste.
//!
//! **Une fois fini ou coupé** ([`IntroView::close`]), tout est lâché : la toile, l'image qui
//! attendait, et le thread, qui s'arrête de lui-même quand plus personne ne lui demande rien (après
//! avoir fini, au plus, l'image qu'il décodait). Rien ne reste : ni thread, ni réveil, ni mémoire.
//!
//! **Le journal** n'en dit qu'une ligne pour toute la suite (le film, puis l'entrée), avec nos noms
//! et des nombres ([`journal_line`]) : « intro : jouée (de tout près, puis tombe du ciel) ; … ».

use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use capri_core::intro::glitch::{self, Glitch};
use capri_core::intro::{Cut, End, Film, Intro, Missing, Suite, place};
use capri_core::{Rect, Vec2};
#[cfg(any(target_os = "macos", windows, test))]
use capri_core::{intro::Entry, roam::Hold};
use capri_render::{Crop, ImageId, RenderError, Renderer, SpriteDraw, Tint};

use crate::assets::intro::Sequence;

/// Là où le film se peint : la carte graphique de l'app, ou une fausse dans les essais.
pub trait Canvas {
    type Id: Copy;
    /// Une toile transparente de cette taille.
    fn create(&mut self, size: (u32, u32)) -> Result<Self::Id, RenderError>;
    /// La toile réécrite avec ces pixels, déjà prémultipliés.
    fn paint(&mut self, id: Self::Id, premultiplied: &[u8]) -> Result<(), RenderError>;
    /// La toile lâchée.
    fn remove(&mut self, id: Self::Id);
    /// Ce qui a été lâché est rendu tout de suite (voir `Renderer::settle`) ; rien à faire pour une
    /// fausse carte graphique.
    fn settle(&mut self) {}
}

impl Canvas for Renderer {
    type Id = ImageId;

    fn create(&mut self, (width, height): (u32, u32)) -> Result<ImageId, RenderError> {
        self.add_canvas(width, height)
    }

    fn paint(&mut self, id: ImageId, premultiplied: &[u8]) -> Result<(), RenderError> {
        self.paint_canvas(id, premultiplied)
    }

    fn remove(&mut self, id: ImageId) {
        self.remove_image(id);
    }

    fn settle(&mut self) {
        Renderer::settle(self);
    }
}

/// Ce que le thread rend.
enum Decoded {
    /// L'image n° `index`, décodée et prémultipliée.
    Image { index: u32, pixels: Vec<u8> },
    /// Pas de film : il manque des images, ou l'une n'a pas la bonne taille.
    Missing(Missing),
    /// L'image n° `index` est illisible : le thread s'arrête là.
    Unreadable { index: u32 },
}

pub struct IntroView<Id: Copy = ImageId> {
    intro: Intro,
    size: (u32, u32),
    /// La graine de sa vie : celle de son glitch.
    seed: u64,
    /// Les demandes au thread : le numéro de l'image à décoder.
    requests: Sender<u32>,
    decoded: Receiver<Decoded>,
    /// L'image demandée et pas encore reçue : jamais plus d'une à la fois.
    asked: Option<u32>,
    /// La toile, une fois la première image arrivée.
    canvas: Option<Id>,
    started: Instant,
    /// Combien de temps sa première image a mis à arriver à l'écran.
    first_after: Option<Duration>,
    thread: Option<JoinHandle<()>>,
}

impl<Id: Copy> IntroView<Id> {
    /// Lance le film à l'image d'animation `frame`, son glitch tiré de la graine `seed` (celle de
    /// sa vie) : le thread est lancé, et la première image demandée. `Err` s'il n'a pas pu l'être :
    /// pas de film.
    pub fn start(sequence: Sequence, frame: u64, seed: u64) -> Result<Self, End> {
        let mut intro = Intro::new(sequence.film, frame);
        let size = sequence.film.size;
        let (requests, asked) = channel();
        // une place : l'image décodée attend son heure là, sans bloquer le thread, qui n'a de
        // toute façon rien d'autre à faire tant qu'on ne lui demande pas la suivante
        let (sender, decoded) = sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("capri-intro".to_owned())
            .spawn(move || decode_on_demand(&sequence, &asked, &sender));
        let Ok(thread) = thread else { return Err(intro.missing(Missing::NoDecoder)) };
        let mut view = Self {
            intro,
            size,
            seed,
            requests,
            decoded,
            asked: None,
            canvas: None,
            started: Instant::now(),
            first_after: None,
            thread: Some(thread),
        };
        view.ask(0);
        Ok(view)
    }

    /// Demande l'image `index` au thread (une seule demande en cours à la fois).
    fn ask(&mut self, index: u32) {
        debug_assert!(self.asked.is_none(), "une seule image demandée à la fois");
        // le thread arrêté, plus rien n'arrivera : la boucle le verra (`Disconnected`)
        if self.requests.send(index).is_ok() {
            self.asked = Some(index);
        }
    }

    /// Un pas, à l'image d'animation `frame` : l'image décodée, si son heure est venue, est
    /// peinte sur la toile de `canvas`. Rend `Some` quand le film est fini ; il faut alors le
    /// fermer ([`IntroView::close`]).
    pub fn step(&mut self, frame: u64, canvas: &mut impl Canvas<Id = Id>) -> Option<End> {
        if self.intro.wants(frame) {
            match self.decoded.try_recv() {
                Ok(Decoded::Image { index, pixels }) => {
                    self.asked = None;
                    if let Err(end) = self.paint(index, &pixels, canvas) {
                        return Some(end);
                    }
                    // lâchée avant de demander la suivante : jamais deux images décodées à la fois
                    drop(pixels);
                    if self.first_after.is_none() {
                        self.first_after = Some(self.started.elapsed());
                    }
                    if let Some(next) = self.intro.shown(index, frame) {
                        self.ask(next);
                    }
                }
                Ok(Decoded::Missing(why)) => return Some(self.intro.missing(why)),
                Ok(Decoded::Unreadable { index }) => return Some(self.intro.broken(index)),
                Err(TryRecvError::Empty) => {}
                // le thread s'est arrêté sans rien dire : comme une image illisible là où il en était
                Err(TryRecvError::Disconnected) => {
                    return Some(match (self.asked, self.intro.showing()) {
                        (_, None) => self.intro.missing(Missing::NoDecoder),
                        (asked, Some(shown)) => self.intro.broken(asked.unwrap_or(shown + 1)),
                    });
                }
            }
        }
        self.intro.step(frame)
    }

    /// Peint l'image `index` sur la toile, créée à la première. Refusée par la carte graphique :
    /// pas de film (à la première), ou il s'arrête là.
    fn paint(&mut self, index: u32, pixels: &[u8], canvas: &mut impl Canvas<Id = Id>) -> Result<(), End> {
        let painted = match self.canvas {
            Some(id) => canvas.paint(id, pixels),
            None => canvas.create(self.size).and_then(|id| {
                self.canvas = Some(id);
                canvas.paint(id, pixels)
            }),
        };
        painted.map_err(|_| match self.intro.showing() {
            None => self.intro.missing(Missing::Refused),
            Some(_) => self.intro.broken(index),
        })
    }

    /// Coupé (voir `capri_core::intro::Cut`) : fini tout de suite ; il faut le fermer.
    pub fn cut(&mut self, why: Cut) -> End {
        self.intro.cut(why)
    }

    /// Ce qu'il faut dessiner sur un écran de `screen` pixels : la toile et où, et le numéro du
    /// dessin qui y est (de quoi savoir s'il faut redessiner) ; `None` avant la première image.
    pub fn picture(&self, screen: Vec2) -> Option<(Id, u32, Rect)> {
        Some((self.canvas?, self.intro.showing()?, place(screen, self.size)))
    }

    /// Le glitch du dessin n° `position` : de la graine de sa vie, à la force `glitch::STRENGTH`.
    pub fn glitch(&self, position: u32) -> Glitch {
        Glitch::of(self.intro.film(), position, self.seed, glitch::STRENGTH)
    }

    /// Le ferme : la toile est lâchée, le thread s'arrêtera de lui-même. Rend le temps qu'a mis sa
    /// première image à arriver, pour le journal ([`journal_line`]).
    pub fn close(mut self, canvas: &mut impl Canvas<Id = Id>) -> Option<Duration> {
        if let Some(id) = self.canvas.take() {
            canvas.remove(id);
        }
        // pas attendu : il s'arrête de lui-même, les canaux lâchés ici (il attend une demande, ou
        // finit l'image qu'il décode, que plus personne ne prendra)
        drop(self.thread.take());
        self.first_after
    }

    /// Le thread, pour vérifier dans un essai qu'il s'arrête une fois le film fermé.
    #[cfg(test)]
    fn take_thread(&mut self) -> Option<JoinHandle<()>> {
        self.thread.take()
    }
}

impl IntroView<ImageId> {
    /// Ce qu'il faut dessiner sur un écran de `screen` pixels : la toile, avec le glitch du dessin
    /// qui y est ([`film_layers`]) ; rien avant la première image.
    pub fn layers(&self, screen: Vec2) -> Vec<(ImageId, SpriteDraw)> {
        self.picture(screen)
            .map(|(id, position, rect)| film_layers(id, rect, self.size, &self.glitch(position)))
            .unwrap_or_default()
    }
}

/// Le dessin de la toile `id`, entière, en `rect`.
pub fn layer<Id: Copy>(id: Id, rect: Rect) -> (Id, SpriteDraw) {
    (
        id,
        SpriteDraw {
            min: [rect.min.x, rect.min.y],
            size: [rect.size.x, rect.size.y],
            ghost: [0.0, 0.0],
            ghost_strength: 0.0,
            boil_seed: 0,
            boil: 0.0,
            opacity: 1.0,
            crop: Crop::WHOLE,
            tint: Tint::NONE,
        },
    )
}

/// Les dessins de la toile `id` (une image de `image` pixels), placée en `rect`, avec le glitch
/// `glitch` : d'abord les doubles de chaque bande (le vert, puis le violet), puis chaque bande
/// d'elle par-dessus, décalée de côté (et toute l'image, de son tremblement) ; une bande manquante
/// n'est pas dessinée, ni ses doubles. Les décalages sont en pixels entiers de l'écran ; la hauteur
/// des bandes suit l'image au pixel près (une bande immobile se lit exactement comme l'image
/// entière). Sans glitch qui se voie, exactement le dessin d'avant ([`layer`]), d'un seul tenant.
pub fn film_layers<Id: Copy>(
    id: Id,
    rect: Rect,
    image: (u32, u32),
    glitch: &Glitch,
) -> Vec<(Id, SpriteDraw)> {
    if glitch.is_still() {
        return vec![layer(id, rect)];
    }
    let k = Vec2::new(rect.size.x / image.0.max(1) as f32, rect.size.y / image.1.max(1) as f32);
    let pixels = |offset: i32, scale: f32| (offset as f32 * scale).round();
    let shake = Vec2::new(pixels(glitch.shake.0, k.x), pixels(glitch.shake.1, k.y));
    let (_, plain) = layer(id, rect);
    let bands: Vec<SpriteDraw> = glitch
        .bands
        .iter()
        .filter(|band| !band.missing)
        .map(|band| {
            let top = rect.min.y + band.rows.0 as f32 * k.y;
            let bottom = rect.min.y + (band.rows.0 + band.rows.1) as f32 * k.y;
            SpriteDraw {
                min: [rect.min.x + pixels(band.shift, k.x) + shake.x, top + shake.y],
                size: [rect.size.x, bottom - top],
                crop: Crop::pixels((0, band.rows.0), (image.0, band.rows.1), image),
                ..plain
            }
        })
        .collect();
    let mut layers = Vec::with_capacity(bands.len() * 3);
    for double in glitch.doubles.iter().rev().filter(|double| double.opacity > 0.0) {
        let offset = [pixels(double.offset.0, k.x), pixels(double.offset.1, k.y)];
        layers.extend(bands.iter().map(|band| {
            let min = [band.min[0] + offset[0], band.min[1] + offset[1]];
            (id, SpriteDraw { min, opacity: double.opacity, tint: Tint::of(double.color, 1.0), ..*band })
        }));
    }
    layers.extend(bands.into_iter().map(|band| (id, band)));
    layers
}

/// Le travail du thread « capri-intro » : à la première demande, vérifier toutes les images
/// (présentes, à la bonne taille, d'après leur en-tête) ; puis décoder chaque image demandée,
/// une à la fois, et la rendre prémultipliée. Il s'arrête quand plus personne ne demande ni
/// n'attend rien, ou sur une image illisible.
fn decode_on_demand(sequence: &Sequence, requests: &Receiver<u32>, decoded: &SyncSender<Decoded>) {
    let mut checked = false;
    while let Ok(index) = requests.recv() {
        if !checked {
            if let Err(why) = sequence.check() {
                let _ = decoded.send(Decoded::Missing(why));
                return;
            }
            checked = true;
        }
        let Some(mut pixels) = sequence.decode(index) else {
            let _ = decoded.send(Decoded::Unreadable { index });
            return;
        };
        capri_render::premultiply_in_place(&mut pixels);
        if decoded.send(Decoded::Image { index, pixels }).is_err() {
            return;
        }
    }
}

/// Ce que fait « Voir une intro », d'après la réponse de `Look::bow_out` pour l'entrée `entry`
/// (son menu, qui n'existe que sur macOS et Windows).
#[cfg(any(target_os = "macos", windows, test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encore {
    /// Elle s'efface, et la suite se jouera.
    pub accepted: bool,
    /// Son bonjour, s'il attendait encore, est oublié.
    pub forget_greeting: bool,
    /// La phrase à dire au-dessus de sa tête, s'il y en a une.
    pub say: Option<&'static str>,
    /// La ligne du journal (niveau `debug`).
    pub log: String,
}

/// « Voir une intro » : acceptée, elle s'efface, le film (`film` : le film `FILM`, ses images
/// là) puis l'entrée `entry` se joueront sans bonjour, et un bonjour qui attendait encore est
/// oublié (il ne serait dit qu'à son retour) ; refusée, rien ne change, et seul « on se parle » se
/// dit, comme pour « Changer de place ».
#[cfg(any(target_os = "macos", windows, test))]
pub fn encore(answer: Result<(), Hold>, entry: &Entry, film: Option<&Film>) -> Encore {
    match answer {
        Ok(()) => Encore {
            accepted: true,
            forget_greeting: true,
            say: None,
            log: match film {
                Some(film) => format!("« Voir une intro » : {}, puis {}", film.name, entry.name),
                None => format!("« Voir une intro » : {}", entry.name),
            },
        },
        Err(hold) => Encore {
            accepted: false,
            forget_greeting: false,
            say: (hold == Hold::Talking).then_some("Pas pendant qu'on se parle"),
            log: format!("« Voir une intro » refusé : {}", hold.describe()),
        },
    }
}

/// Pourquoi c'est coupé, pour le journal.
fn why(cut: Cut) -> &'static str {
    match cut {
        Cut::Hidden => "on la cache",
        Cut::Talked => "on lui parle",
        Cut::Grabbed => "on l'attrape",
        Cut::Resized => "sa taille change",
    }
}

/// La ligne du journal pour la suite `suite`, finie, dont le film est `film` (sa première image
/// arrivée à l'écran au bout de `first_after`). Nos noms et des nombres, rien d'autre : ce qu'elle
/// a été (jouée, coupée, ou rien de joué), puis ce qu'a fait le film, puis l'entrée (en images
/// d'animation).
pub fn journal_line(suite: &Suite, film: &Film, first_after: Option<Duration>) -> String {
    let entry = suite.entry().name;
    let names = if suite.with_film() { format!("{}, puis {entry}", film.name) } else { entry.to_owned() };
    let played = [suite.film_end(), suite.entry_end()].into_iter().flatten().any(End::greets);
    let verdict = match (suite.was_cut(), played) {
        (true, _) => "coupée",
        (false, true) => "jouée",
        (false, false) => "pas jouée",
    };
    let mut parts = vec![format!("intro : {verdict} ({names})")];
    if let Some(end) = suite.film_end() {
        parts.push(film_part(film, end, first_after));
    }
    match suite.entry_end() {
        Some(end) => parts.push(entry_part(entry, end)),
        None => parts.push(format!("{entry} : pas jouée")),
    }
    parts.join(" ; ")
}

/// Ce qu'a fait le film, pour le journal.
fn film_part(film: &Film, end: End, first_after: Option<Duration>) -> String {
    let (name, count) = (film.name, film.len());
    let first = first_after
        .map_or_else(String::new, |after| format!(", la première prête en {} ms", after.as_millis()));
    match end {
        End::Played { shown } => format!("{name} : {shown} images sur {count} montrées{first}"),
        End::Cut { why: cut, at: Some(at) } => {
            format!("{name} : coupé à l'image {} sur {count} ({}){first}", at + 1, why(cut))
        }
        End::Cut { why: cut, at: None } => format!("{name} : coupé avant sa première image ({})", why(cut)),
        End::Broken { at } => format!("{name} : arrêté à l'image {} sur {count} (illisible){first}", at + 1),
        End::Missing(missing) => format!(
            "{name} : pas d'images ({})",
            match missing {
                Missing::Absent { found } => format!("{found} sur {}", film.files.len()),
                Missing::Unreadable { index } => format!("l'image {} est illisible", index + 1),
                Missing::WrongSize { index, found } => format!(
                    "l'image {} mesure {} × {} au lieu de {} × {}",
                    index + 1,
                    found.0,
                    found.1,
                    film.size.0,
                    film.size.1
                ),
                Missing::Refused => "refusées par la carte graphique".to_owned(),
                Missing::Late => format!(
                    "la première n'est pas prête après {} s",
                    capri_core::intro::START_WAIT / capri_core::anim::FRAME_RATE as u64
                ),
                Missing::NoDecoder => "rien pour les lire".to_owned(),
                // ce que seule une entrée dit : jamais d'un film
                Missing::NoRoom => "pas la place".to_owned(),
                Missing::NotReady => "pas prête".to_owned(),
            }
        ),
    }
}

/// Ce qu'a fait l'entrée (ses nombres : des images d'animation), pour le journal.
fn entry_part(name: &str, end: End) -> String {
    match end {
        End::Played { shown: 0 } => format!("{name} : jouée"),
        End::Played { shown } => format!("{name} : en {shown} images"),
        End::Cut { why: cut, at: Some(at) } => format!("{name} : coupée après {at} images ({})", why(cut)),
        End::Cut { why: cut, at: None } => format!("{name} : coupée avant de commencer ({})", why(cut)),
        // refusée : elle s'est matérialisée à la place, et la vraie raison est dite
        End::Missing(Missing::NoRoom) => format!("{name} : pas la place d'où elle est"),
        End::Missing(Missing::NotReady) => format!("{name} : pas prête"),
        End::Broken { .. } | End::Missing(_) => format!("{name} : pas de planches"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use capri_core::intro::HOLD;

    use super::*;

    /// Un dossier d'essai, retiré à la fin, même en cas d'échec.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("capri-test-intro-vue-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// De petites images d'essai (8 × 6), chacune d'une couleur à elle : la n° `i` a son rouge à
    /// `10 * i + 10`, son alpha à 255.
    const SIZE: (u32, u32) = (8, 6);

    fn write(dir: &Path, index: u32, size: (u32, u32)) {
        let color = image::Rgba([10 * index as u8 + 10, 0, 0, 255]);
        image::RgbaImage::from_pixel(size.0, size.1, color)
            .save(dir.join(capri_core::intro::file(index)))
            .unwrap();
    }

    /// De petits films d'essai, chaque image tenue deux images : cinq ou six images.
    static FIVE: Film = Film {
        name: "essai",
        dir: "",
        files: &["01.png", "02.png", "03.png", "04.png", "05.png"],
        size: SIZE,
        sheet: &[(0, 2), (1, 2), (2, 2), (3, 2), (4, 2)],
        glitch: None,
    };
    static SIX: Film = Film {
        name: "essai",
        dir: "",
        files: &["01.png", "02.png", "03.png", "04.png", "05.png", "06.png"],
        size: SIZE,
        sheet: &[(0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2)],
        glitch: None,
    };
    /// Un film qui revient sur une image : 0, 1, 0, 2.
    static AGAIN: Film = Film {
        name: "essai",
        dir: "",
        files: &["01.png", "02.png", "03.png"],
        size: SIZE,
        sheet: &[(0, 2), (1, 2), (0, 3), (2, 2)],
        glitch: None,
    };

    fn sequence(dir: &Path, count: u32) -> Sequence {
        let film = match count {
            5 => &FIVE,
            6 => &SIX,
            _ => &AGAIN,
        };
        Sequence { dir: dir.to_owned(), film }
    }

    /// Une fausse carte graphique : ce qu'on y a fait.
    #[derive(Default)]
    struct Fake {
        created: u32,
        /// Le rouge du premier pixel de chaque image peinte : son numéro.
        painted: Vec<u32>,
        removed: Vec<u32>,
        refuse: bool,
    }

    impl Canvas for Fake {
        type Id = u32;

        fn create(&mut self, size: (u32, u32)) -> Result<u32, RenderError> {
            assert_eq!(size, SIZE);
            if self.refuse {
                return Err(RenderError::TooLarge { width: size.0, height: size.1, max: 4 });
            }
            self.created += 1;
            Ok(self.created)
        }

        fn paint(&mut self, id: u32, pixels: &[u8]) -> Result<(), RenderError> {
            assert_eq!(id, self.created, "toujours la même toile");
            assert_eq!(pixels.len(), (SIZE.0 * SIZE.1 * 4) as usize);
            self.painted.push((u32::from(pixels[0]) - 10) / 10);
            Ok(())
        }

        fn remove(&mut self, id: u32) {
            self.removed.push(id);
        }
    }

    /// Fait des pas à l'image `frame` jusqu'à ce que `done` soit vrai (le thread a le temps de
    /// décoder), cinq secondes au plus ; rend la fin s'il y en a une.
    fn until(
        view: &mut IntroView<u32>,
        frame: u64,
        fake: &mut Fake,
        done: impl Fn(&IntroView<u32>, &Fake) -> bool,
    ) -> Option<End> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(end) = view.step(frame, fake) {
                return Some(end);
            }
            if done(view, fake) {
                return None;
            }
            assert!(Instant::now() < deadline, "le thread ne rend rien");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Le thread d'une intro fermée s'arrête de lui-même.
    fn stops(thread: JoinHandle<()>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !thread.is_finished() {
            assert!(Instant::now() < deadline, "le thread ne s'arrête pas");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn each_image_is_decoded_on_demand_painted_on_one_canvas_in_turn_then_all_is_let_go() {
        let scratch = Scratch::new("jouee");
        for index in 0..5 {
            write(&scratch.0, index, SIZE);
        }
        let mut fake = Fake::default();
        let mut view = IntroView::start(sequence(&scratch.0, 5), 100, 0).expect("lancée");
        let thread = view.take_thread().expect("son thread");
        // rien à l'écran avant la première image
        assert!(view.picture(Vec2::new(16.0, 9.0)).is_none());
        // la première arrive quand elle arrive : l'horloge part de là (image 110)
        assert_eq!(until(&mut view, 110, &mut fake, |view, _| view.intro.showing().is_some()), None);
        assert_eq!(fake.painted, [0]);
        assert_eq!(view.asked, Some(1), "la suivante est demandée, et elle seule");
        let (id, shown, rect) = view.picture(Vec2::new(16.0, 9.0)).expect("la toile");
        assert_eq!((id, shown), (1, 0));
        assert_eq!(rect, place(Vec2::new(16.0, 9.0), SIZE));
        // la suivante, décodée tôt, attend son heure sans être peinte
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(view.step(111, &mut fake), None);
        assert_eq!(fake.painted, [0], "pas avant son heure");
        // puis chacune à son heure, deux images d'animation chacune
        for index in 1..5 {
            let frame = 110 + u64::from(index) * HOLD;
            until(&mut view, frame, &mut fake, |view, _| view.intro.showing() == Some(index));
            assert_eq!(view.asked, (index + 1 < 5).then_some(index + 1));
        }
        assert_eq!(fake.painted, [0, 1, 2, 3, 4]);
        assert_eq!(fake.created, 1, "une seule toile");
        // à l'heure, elle est finie, et tout est lâché
        let end = view.step(110 + 5 * HOLD, &mut fake).expect("finie");
        assert_eq!(end, End::Played { shown: 5 });
        assert!(view.close(&mut fake).is_some(), "le temps de sa première image, pour le journal");
        assert_eq!(fake.removed, [1]);
        stops(thread);
    }

    #[test]
    fn a_late_image_holds_the_previous_one_and_the_intro_still_ends_on_time() {
        let scratch = Scratch::new("en-retard");
        for index in 0..6 {
            write(&scratch.0, index, SIZE);
        }
        let mut fake = Fake::default();
        let mut view = IntroView::start(sequence(&scratch.0, 6), 0, 0).expect("lancée");
        until(&mut view, 0, &mut fake, |view, _| view.intro.showing().is_some());
        // la boucle ne revient qu'à l'image 7 (l'heure de la n° 3) : la n° 1, prête depuis, est
        // peinte (elle est plus récente), puis c'est la n° 3 qui est demandée
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(view.step(7, &mut fake), None);
        assert_eq!(fake.painted, [0, 1]);
        assert_eq!(view.asked, Some(3));
        until(&mut view, 8, &mut fake, |_, fake| fake.painted.len() == 3);
        assert_eq!(fake.painted, [0, 1, 3]);
        // l'heure de la fin vient : finie, même si toutes n'ont pas été montrées
        let end = until(&mut view, 12, &mut fake, |_, _| false).expect("finie à l'heure");
        assert_eq!(end, End::Played { shown: 3 });
        view.close(&mut fake);
    }

    #[test]
    fn an_image_that_comes_back_in_the_sheet_is_decoded_again() {
        let scratch = Scratch::new("revient");
        for index in 0..3 {
            write(&scratch.0, index, SIZE);
        }
        let mut fake = Fake::default();
        let mut view = IntroView::start(sequence(&scratch.0, 3), 0, 0).expect("lancée");
        until(&mut view, 0, &mut fake, |view, _| view.intro.showing().is_some());
        for (position, frame) in [(1u32, 2u64), (2, 4), (3, 7)] {
            until(&mut view, frame, &mut fake, |view, _| view.intro.showing() == Some(position));
        }
        // ses images, dans l'ordre de sa feuille : la première, revenue
        assert_eq!(fake.painted, [0, 1, 0, 2]);
        let end = until(&mut view, 9, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Played { shown: 4 });
        view.close(&mut fake);
    }

    #[test]
    fn without_all_its_images_at_the_right_size_there_is_no_intro() {
        let scratch = Scratch::new("absentes");
        for index in 0..4 {
            write(&scratch.0, index, SIZE);
        }
        let mut fake = Fake::default();
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0, 0).expect("lancée");
        let end = until(&mut view, 0, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::Absent { found: 4 }));
        assert_eq!(view.close(&mut fake), None);
        assert!(fake.painted.is_empty() && fake.removed.is_empty() && fake.created == 0);

        // une image d'une autre taille
        write(&scratch.0, 4, (16, 9));
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0, 0).expect("lancée");
        let end = until(&mut view, 0, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::WrongSize { index: 4, found: (16, 9) }));

        // une image illisible dès son en-tête
        std::fs::write(scratch.0.join(capri_core::intro::file(2)), b"pas une image").unwrap();
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0, 0).expect("lancée");
        let end = until(&mut view, 0, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::Unreadable { index: 2 }));

        // la carte graphique refuse la toile
        write(&scratch.0, 2, SIZE);
        write(&scratch.0, 4, SIZE);
        let mut refusing = Fake { refuse: true, ..Fake::default() };
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0, 0).expect("lancée");
        let end = until(&mut view, 0, &mut refusing, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::Refused));
        assert_eq!(view.close(&mut refusing), None);
        assert!(refusing.removed.is_empty());
        // pas de dossier du tout
        let mut view = IntroView::start(sequence(&scratch.0.join("nulle-part"), 5), 0, 0).expect("lancée");
        let end = until(&mut view, 0, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::Absent { found: 0 }));
    }

    #[test]
    fn an_image_unreadable_halfway_stops_the_intro_there() {
        let scratch = Scratch::new("illisible");
        for index in 0..5 {
            write(&scratch.0, index, SIZE);
        }
        // l'en-tête de la n° 2 est bon (sa taille se lit), mais ses pixels s'arrêtent net
        let path = scratch.0.join(capri_core::intro::file(2));
        let bytes = std::fs::read(&path).unwrap();
        let pixels = bytes.windows(4).position(|chunk| chunk == b"IDAT").expect("ses pixels");
        std::fs::write(&path, &bytes[..pixels + 6]).unwrap();
        let mut fake = Fake::default();
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0, 0).expect("lancée");
        let thread = view.take_thread().expect("son thread");
        until(&mut view, 0, &mut fake, |view, _| view.intro.showing().is_some());
        until(&mut view, 2, &mut fake, |view, _| view.intro.showing() == Some(1));
        let end = until(&mut view, 4, &mut fake, |_, _| false).expect("arrêtée");
        assert_eq!(end, End::Broken { at: 2 });
        assert!(end.greets(), "joué jusque-là : l'entrée suit, puis son bonjour");
        view.close(&mut fake);
        assert_eq!((fake.painted.as_slice(), fake.removed.as_slice()), (&[0, 1][..], &[1][..]));
        stops(thread);
    }

    #[test]
    fn cut_at_any_time_everything_is_let_go_and_the_thread_stops() {
        let scratch = Scratch::new("coupee");
        for index in 0..5 {
            write(&scratch.0, index, SIZE);
        }
        // avant la première image
        let mut fake = Fake::default();
        let mut view: IntroView<u32> = IntroView::start(sequence(&scratch.0, 5), 0, 0).expect("lancée");
        let thread = view.take_thread().expect("son thread");
        let end = view.cut(Cut::Hidden);
        assert_eq!(end, End::Cut { why: Cut::Hidden, at: None });
        assert_eq!(view.close(&mut fake), None);
        assert!(fake.removed.is_empty(), "pas de toile à lâcher");
        stops(thread);
        // au milieu, une image décodée attendant son heure
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0, 0).expect("lancée");
        let thread = view.take_thread().expect("son thread");
        until(&mut view, 0, &mut fake, |view, _| view.intro.showing().is_some());
        std::thread::sleep(Duration::from_millis(30));
        let end = view.cut(Cut::Talked);
        assert_eq!(end, End::Cut { why: Cut::Talked, at: Some(0) });
        assert!(view.close(&mut fake).is_some());
        assert_eq!(fake.removed, [1]);
        stops(thread);
    }

    /// L'écran d'un MacBook Pro 14", et l'image du film.
    const SCREEN: Vec2 = Vec2::new(3024.0, 1964.0);
    const IMAGE: (u32, u32) = capri_core::intro::SIZE;

    #[test]
    fn without_glitch_the_film_is_drawn_exactly_as_before() {
        use capri_core::intro::FILM;
        let rect = place(SCREEN, IMAGE);
        assert_eq!(film_layers(7u32, rect, IMAGE, &Glitch::none(IMAGE)), vec![layer(7u32, rect)]);
        // à force nulle, chaque dessin, quelle que soit la graine
        for seed in 0..50u64 {
            for position in 0..FILM.len() {
                let glitch = Glitch::of(FILM, position, seed, 0.0);
                assert_eq!(film_layers(7u32, rect, IMAGE, &glitch), vec![layer(7u32, rect)]);
            }
        }
        // la toile entière, telle qu'avant le glitch : ni décalée, ni teintée
        let (_, draw) = layer(7u32, rect);
        assert_eq!((draw.crop, draw.tint, draw.opacity), (Crop::WHOLE, Tint::NONE, 1.0));
    }

    #[test]
    fn its_bands_and_doubles_stay_within_their_bounds() {
        use capri_core::intro::FILM;
        let mut most = 0;
        for screen in [SCREEN, Vec2::new(1920.0, 1080.0), Vec2::new(1440.0, 900.0)] {
            let rect = place(screen, IMAGE);
            let k = Vec2::new(rect.size.x / IMAGE.0 as f32, rect.size.y / IMAGE.1 as f32);
            let (reach_x, reach_y) = glitch::reach(IMAGE);
            let (slack_x, slack_y) = (reach_x as f32 * k.x + 2.0, reach_y as f32 * k.y + 2.0);
            for seed in 0..200u64 {
                for position in 0..FILM.len() {
                    let glitch = Glitch::of(FILM, position, seed, glitch::STRENGTH);
                    let layers = film_layers(1u32, rect, IMAGE, &glitch);
                    most = most.max(layers.len());
                    assert!(layers.len() <= 3 * glitch::MAX_BANDS, "{}", layers.len());
                    let at = format!("{screen:?}, graine {seed}, dessin {}", position + 1);
                    for (_, draw) in &layers {
                        // jamais plus loin que le décalage borné, de chaque côté
                        assert!(draw.min[0] >= rect.min.x - slack_x, "{at} : {draw:?}");
                        assert!(draw.min[0] + draw.size[0] <= rect.max().x + slack_x, "{at} : {draw:?}");
                        assert!(draw.min[1] >= rect.min.y - slack_y, "{at} : {draw:?}");
                        assert!(draw.min[1] + draw.size[1] <= rect.max().y + slack_y, "{at} : {draw:?}");
                        // toujours de sa largeur, et décalée en pixels entiers de l'écran
                        assert_eq!(draw.size[0], rect.size.x, "{at}");
                        assert_eq!((draw.min[0] - rect.min.x).fract(), 0.0, "{at}");
                        assert!((0.0..=1.0).contains(&draw.opacity), "{at}");
                    }
                    if glitch.is_still() {
                        continue;
                    }
                    // ses doubles d'abord (le vert, puis le violet), elle par-dessus
                    let bodies: Vec<&SpriteDraw> =
                        layers.iter().map(|(_, draw)| draw).filter(|draw| draw.tint == Tint::NONE).collect();
                    let first_body = layers.iter().position(|(_, draw)| draw.tint == Tint::NONE);
                    assert!(
                        layers[first_body.unwrap_or(0)..].iter().all(|(_, draw)| draw.tint == Tint::NONE)
                    );
                    let shown: Vec<_> = glitch.bands.iter().filter(|band| !band.missing).collect();
                    assert_eq!(bodies.len(), shown.len(), "{at} : une bande manquante n'est pas dessinée");
                    for (draw, band) in bodies.iter().zip(&shown) {
                        // sa hauteur suit l'image ; immobile, elle est exactement à sa place
                        let top = rect.min.y + band.rows.0 as f32 * k.y;
                        assert!(
                            (draw.min[1] - top - (glitch.shake.1 as f32 * k.y).round()).abs() < 1e-3,
                            "{at}"
                        );
                        if band.shift == 0 && glitch.shake.0 == 0 {
                            assert_eq!(draw.min[0], rect.min.x, "{at} : pas de bord pour rien");
                        }
                    }
                    for (_, draw) in &layers[..first_body.unwrap_or(layers.len())] {
                        let color = [glitch::GREEN, glitch::VIOLET].map(|rgb| Tint::of(rgb, 1.0));
                        assert!(color.contains(&draw.tint), "{at} : {:?}", draw.tint);
                    }
                }
            }
        }
        println!("au plus {most} dessins pour une image du film");
    }

    #[test]
    fn see_an_intro_accepted_forgets_a_waiting_greeting_and_refused_changes_nothing() {
        use capri_core::intro::{FILM, LIST};
        let entry = &LIST[0];
        let accepted = encore(Ok(()), entry, Some(FILM));
        assert!(accepted.accepted && accepted.forget_greeting && accepted.say.is_none());
        assert_eq!(accepted.log, "« Voir une intro » : de tout près, puis tombe du ciel");
        assert_eq!(encore(Ok(()), entry, None).log, "« Voir une intro » : tombe du ciel");
        for hold in [Hold::Carried, Hold::Away, Hold::Walking, Hold::Talking] {
            let refused = encore(Err(hold), entry, Some(FILM));
            assert!(!refused.accepted && !refused.forget_greeting, "{hold:?}");
            assert_eq!(refused.say.is_some(), hold == Hold::Talking, "{hold:?}");
        }
    }

    #[test]
    fn the_journal_says_the_suite_with_our_names_numbers_and_nothing_else() {
        use capri_core::intro::{FILM, by_key};
        let falls = by_key("tombe").expect("tombe du ciel");
        let walks = by_key("marche").expect("arrive en marchant");
        let plain = capri_core::intro::plain();
        let suite = |entry, film: Option<End>, entered: Option<End>| {
            let mut suite = Suite::new(entry, film.is_some(), true);
            if let Some(end) = film {
                suite.record_film(end);
            }
            if let Some(end) = entered {
                suite.record_entry(end);
            }
            suite
        };
        let ms = |n| Some(Duration::from_millis(n));
        let played = Some(End::Played { shown: 22 });
        let lines = [
            journal_line(&suite(falls, played, Some(End::Played { shown: 30 })), FILM, ms(31)),
            journal_line(&suite(plain, played, Some(End::Played { shown: 0 })), FILM, ms(12)),
            journal_line(&suite(falls, Some(End::Cut { why: Cut::Hidden, at: Some(8) }), None), FILM, ms(5)),
            journal_line(&suite(walks, Some(End::Cut { why: Cut::Talked, at: None }), None), FILM, None),
            journal_line(
                &suite(walks, played, Some(End::Cut { why: Cut::Grabbed, at: Some(41) })),
                FILM,
                ms(20),
            ),
            journal_line(
                &suite(
                    falls,
                    Some(End::Missing(Missing::Absent { found: 0 })),
                    Some(End::Played { shown: 30 }),
                ),
                FILM,
                None,
            ),
            journal_line(&suite(falls, None, Some(End::Played { shown: 30 })), FILM, None),
            journal_line(
                &suite(falls, Some(End::Broken { at: 3 }), Some(End::Cut { why: Cut::Resized, at: Some(2) })),
                FILM,
                ms(5),
            ),
            journal_line(
                &suite(
                    falls,
                    Some(End::Missing(Missing::WrongSize { index: 0, found: (1280, 720) })),
                    Some(End::Missing(Missing::Absent { found: 0 })),
                ),
                FILM,
                None,
            ),
            journal_line(
                &suite(falls, Some(End::Missing(Missing::Late)), Some(End::Played { shown: 30 })),
                FILM,
                None,
            ),
            journal_line(&suite(walks, played, Some(End::Missing(Missing::NoRoom))), FILM, ms(9)),
            journal_line(&suite(falls, played, Some(End::Missing(Missing::NotReady))), FILM, ms(9)),
        ];
        assert_eq!(
            lines[0],
            "intro : jouée (de tout près, puis tombe du ciel) ; de tout près : 22 images sur 22 montrées, la \
             première prête en 31 ms ; tombe du ciel : en 30 images"
        );
        assert_eq!(
            lines[1],
            "intro : jouée (de tout près, puis se matérialise) ; de tout près : 22 images sur 22 montrées, la \
             première prête en 12 ms ; se matérialise : jouée"
        );
        assert_eq!(
            lines[2],
            "intro : coupée (de tout près, puis tombe du ciel) ; de tout près : coupé à l'image 9 sur 22 (on la \
             cache), la première prête en 5 ms ; tombe du ciel : pas jouée"
        );
        assert_eq!(
            lines[3],
            "intro : coupée (de tout près, puis arrive en marchant) ; de tout près : coupé avant sa première \
             image (on lui parle) ; arrive en marchant : pas jouée"
        );
        assert_eq!(
            lines[4],
            "intro : coupée (de tout près, puis arrive en marchant) ; de tout près : 22 images sur 22 montrées, \
             la première prête en 20 ms ; arrive en marchant : coupée après 41 images (on l'attrape)"
        );
        assert_eq!(
            lines[5],
            "intro : jouée (de tout près, puis tombe du ciel) ; de tout près : pas d'images (0 sur 22) ; tombe du \
             ciel : en 30 images"
        );
        assert_eq!(lines[6], "intro : jouée (tombe du ciel) ; tombe du ciel : en 30 images");
        assert_eq!(
            lines[7],
            "intro : coupée (de tout près, puis tombe du ciel) ; de tout près : arrêté à l'image 4 sur 22 \
             (illisible), la première prête en 5 ms ; tombe du ciel : coupée après 2 images (sa taille change)"
        );
        assert_eq!(
            lines[8],
            "intro : pas jouée (de tout près, puis tombe du ciel) ; de tout près : pas d'images (l'image 1 mesure \
             1280 × 720 au lieu de 1920 × 1080) ; tombe du ciel : pas de planches"
        );
        assert_eq!(
            lines[9],
            "intro : jouée (de tout près, puis tombe du ciel) ; de tout près : pas d'images (la première n'est \
             pas prête après 2 s) ; tombe du ciel : en 30 images"
        );
        // une entrée refusée dit pourquoi : la place, pas prête ; « pas de planches » seulement
        // quand ce sont elles
        assert_eq!(
            lines[10],
            "intro : jouée (de tout près, puis arrive en marchant) ; de tout près : 22 images sur 22 montrées, \
             la première prête en 9 ms ; arrive en marchant : pas la place d'où elle est"
        );
        assert_eq!(
            lines[11],
            "intro : jouée (de tout près, puis tombe du ciel) ; de tout près : 22 images sur 22 montrées, la \
             première prête en 9 ms ; tombe du ciel : pas prête"
        );
        for line in lines {
            assert!(!line.contains('/') && !line.contains(".png") && !line.contains("upclose"), "{line}");
        }
    }
}
