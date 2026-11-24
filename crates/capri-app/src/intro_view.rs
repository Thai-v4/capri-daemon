//! Une intro en images, côté application : ses images décodées une à une sur un thread à part
//! (« capri-intro »), peintes l'une après l'autre sur une seule toile de la carte graphique, au
//! rythme que décide sa feuille d'exposition (`capri_core::intro`).
//!
//! **Ce que ça pèse.** Une image de « Up close » (1 920 × 1 080) en RGBA fait 8,3 Mo, les 22 en
//! feraient 182 ; une du coucou (1 672 × 941), 6,3 Mo, les 9 en feraient 57 : elles ne sont jamais
//! toutes en mémoire, ni sur la carte graphique. Le thread ne décode que l'image qu'on lui demande,
//! une seule à la fois, et l'image suivante n'est demandée qu'une fois la précédente peinte et
//! lâchée : il n'existe jamais plus d'une image décodée (une image qui revient dans la feuille est
//! décodée de nouveau). La carte graphique n'en a qu'une, la toile, réécrite en place
//! (`Renderer::paint_canvas`). Au pire, à l'instant où une image est peinte : la toile (de la
//! mémoire graphique, qui compte dans celle de l'app sur un Mac à puce Apple), l'image décodée et
//! la copie qu'en fait wgpu pour l'envoyer (rendue une fois la copie faite), trois fois la taille
//! d'une image : 24,9 Mo pour « Up close », 18,9 Mo pour le coucou ; le reste du temps, la toile et
//! au plus une image qui attend son heure. En plus : le fichier compressé (de 10 à 140 Ko) et les
//! tampons du décodeur PNG ou WebP. Elle est prémultipliée sur le thread, pas sur la boucle.
//!
//! **La boucle n'attend jamais** : elle essaie de prendre l'image décodée (`try_recv`) quand son
//! heure est venue ; pas prête, le dessin précédent reste.
//!
//! **Une fois finie ou coupée** ([`IntroView::close`]), tout est lâché : la toile, l'image qui
//! attendait, et le thread, qui s'arrête de lui-même quand plus personne ne lui demande rien (après
//! avoir fini, au plus, l'image qu'il décodait). Rien ne reste : ni thread, ni réveil, ni mémoire.
//!
//! **Le journal** n'en dit qu'une ligne, avec son nom à nous et des nombres ([`journal_line`],
//! qui sert aussi aux intros qu'elle joue) : « intro : jouée (…) », « intro : coupée (…) » ou
//! « intro : pas d'images (…) ».

use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use capri_core::intro::{Cut, End, Film, Intro, Missing, place};
use capri_core::{Rect, Vec2};
#[cfg(any(target_os = "macos", windows, test))]
use capri_core::{intro::Entry, roam::Hold};
use capri_render::{Crop, ImageId, RenderError, Renderer, SpriteDraw};

use crate::assets::intro::Sequence;

/// Là où l'intro se peint : la carte graphique de l'app, ou une fausse dans les essais.
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
    /// Pas d'intro : il manque des images, ou l'une n'a pas la bonne taille.
    Missing(Missing),
    /// L'image n° `index` est illisible : le thread s'arrête là.
    Unreadable { index: u32 },
}

pub struct IntroView<Id: Copy = ImageId> {
    intro: Intro,
    size: (u32, u32),
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
    /// Lance l'intro à l'image d'animation `frame` : le thread est lancé, et la première image
    /// demandée. `Err` s'il n'a pas pu l'être : pas d'intro.
    pub fn start(sequence: Sequence, frame: u64) -> Result<Self, End> {
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
    /// peinte sur la toile de `canvas`. Rend `Some` quand l'intro est finie ; il faut alors la
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
    /// pas d'intro (à la première), ou elle s'arrête là.
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

    /// Coupée (voir `capri_core::intro::Cut`) : finie tout de suite ; il faut la fermer.
    pub fn cut(&mut self, why: Cut) -> End {
        self.intro.cut(why)
    }

    /// Ce qu'il faut dessiner sur un écran de `screen` pixels : la toile et où, et le numéro de
    /// l'image qui y est (de quoi savoir s'il faut redessiner) ; `None` avant la première image.
    pub fn picture(&self, screen: Vec2) -> Option<(Id, u32, Rect)> {
        Some((self.canvas?, self.intro.showing()?, place(screen, self.size)))
    }

    /// La ferme, finie par `end` : la toile est lâchée, le thread s'arrêtera de lui-même. Rend la
    /// ligne du journal, au nom de `name`.
    pub fn close(mut self, end: End, name: &str, canvas: &mut impl Canvas<Id = Id>) -> String {
        if let Some(id) = self.canvas.take() {
            canvas.remove(id);
        }
        // pas attendu : il s'arrête de lui-même, les canaux lâchés ici (il attend une demande, ou
        // finit l'image qu'il décode, que plus personne ne prendra)
        drop(self.thread.take());
        journal_line(name, Some(self.intro.film()), end, self.first_after)
    }

    /// Le thread, pour vérifier dans un essai qu'il s'arrête une fois l'intro fermée.
    #[cfg(test)]
    fn take_thread(&mut self) -> Option<JoinHandle<()>> {
        self.thread.take()
    }
}

/// Le dessin de la toile `id`, en `rect`.
pub fn layer(id: ImageId, rect: Rect) -> (ImageId, SpriteDraw) {
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
        },
    )
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

/// Ce qu'il faut faire à la fin d'une intro, une fois sa ligne écrite au journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closing {
    /// Présenter une image vide avant de cacher la fenêtre : rien de l'intro n'y reste, qu'elle
    /// montrerait un instant quand on la montre de nouveau.
    pub blank: bool,
    /// Elle se matérialise à sa place (après une intro en images ; une intro qu'elle joue l'a déjà
    /// fait, et « rien » aussi).
    pub enter: bool,
}

/// La fin d'une intro finie par `end` : en images (`film`), ou non.
/// Coupée en la cachant, une image vide avant de cacher la fenêtre, **pour toutes** (ses bandes
/// étirées sur tout l'écran, par exemple, réapparaîtraient un instant) ; sinon, après des images,
/// elle se matérialise.
pub fn closing(end: End, film: bool) -> Closing {
    Closing { blank: end.hides(), enter: film && !end.hides() }
}

/// Ce que fait « Voir une intro », d'après la réponse de `Look::bow_out` pour l'intro `entry`
/// (son menu, qui n'existe que sur macOS et Windows).
#[cfg(any(target_os = "macos", windows, test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encore {
    /// Elle s'efface, et l'intro se jouera.
    pub accepted: bool,
    /// Son bonjour, s'il attendait encore, est oublié.
    pub forget_greeting: bool,
    /// La phrase à dire au-dessus de sa tête, s'il y en a une.
    pub say: Option<&'static str>,
    /// La ligne du journal (niveau `debug`).
    pub log: String,
}

/// « Voir une intro » : acceptée, elle s'efface, l'intro se jouera sans bonjour, et un bonjour qui
/// attendait encore est oublié (il ne serait dit qu'à son retour) ; refusée, rien ne change, et
/// seul « on se parle » se dit, comme pour « Changer de place ».
#[cfg(any(target_os = "macos", windows, test))]
pub fn encore(answer: Result<(), Hold>, entry: &Entry) -> Encore {
    match answer {
        Ok(()) => Encore {
            accepted: true,
            forget_greeting: true,
            say: None,
            log: format!("« Voir une intro » : {}", entry.name),
        },
        Err(hold) => Encore {
            accepted: false,
            forget_greeting: false,
            say: (hold == Hold::Talking).then_some("Pas pendant qu'on se parle"),
            log: format!("« Voir une intro » refusé : {}", hold.describe()),
        },
    }
}

/// La ligne du journal pour l'intro `name` finie par `end` : en images (`film` : ses dessins ; la
/// première arrivée à l'écran au bout de `first_after`), ou jouée par elle (`None` : les nombres
/// de `end` sont des images d'animation). Son nom à nous et des nombres, rien d'autre.
pub fn journal_line(name: &str, film: Option<&Film>, end: End, first_after: Option<Duration>) -> String {
    let why = |why: Cut| match why {
        Cut::Hidden => "on la cache",
        Cut::Talked => "on lui parle",
        Cut::Grabbed => "on l'attrape",
        Cut::Resized => "sa taille change",
    };
    let Some(film) = film else {
        return match end {
            End::Played { shown: 0 } => format!("intro : jouée ({name})"),
            End::Played { shown } => format!("intro : jouée ({name}), en {shown} images"),
            End::Cut { why: cut, at: Some(at) } => {
                format!("intro : coupée ({name}) après {at} images ({})", why(cut))
            }
            End::Cut { why: cut, at: None } => {
                format!("intro : coupée ({name}) avant de commencer ({})", why(cut))
            }
            End::Broken { .. } | End::Missing(_) => format!("intro : pas de planches ({name})"),
        };
    };
    let count = film.len();
    let first = first_after
        .map_or_else(String::new, |after| format!(", la première prête en {} ms", after.as_millis()));
    match end {
        End::Played { shown } => {
            format!("intro : jouée ({name}), {shown} images sur {count} montrées{first}")
        }
        End::Cut { why: cut, at: Some(at) } => {
            format!("intro : coupée ({name}) à l'image {} sur {count} ({}){first}", at + 1, why(cut))
        }
        End::Cut { why: cut, at: None } => {
            format!("intro : coupée ({name}) avant sa première image ({})", why(cut))
        }
        End::Broken { at } => {
            format!("intro : coupée ({name}) à l'image {} sur {count} (illisible){first}", at + 1)
        }
        End::Missing(why) => format!(
            "intro : pas d'images ({name} : {})",
            match why {
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
            }
        ),
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

    /// De petites intros d'essai, chaque image tenue deux images : cinq ou six images.
    static FIVE: Film = Film {
        dir: "",
        files: &["01.png", "02.png", "03.png", "04.png", "05.png"],
        size: SIZE,
        sheet: &[(0, 2), (1, 2), (2, 2), (3, 2), (4, 2)],
    };
    static SIX: Film = Film {
        dir: "",
        files: &["01.png", "02.png", "03.png", "04.png", "05.png", "06.png"],
        size: SIZE,
        sheet: &[(0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2)],
    };
    /// Une intro qui revient sur une image : 0, 1, 0, 2.
    static AGAIN: Film = Film {
        dir: "",
        files: &["01.png", "02.png", "03.png"],
        size: SIZE,
        sheet: &[(0, 2), (1, 2), (0, 3), (2, 2)],
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
        let mut view = IntroView::start(sequence(&scratch.0, 5), 100).expect("lancée");
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
        let line = view.close(end, "essai", &mut fake);
        assert!(
            line.starts_with("intro : jouée (essai), 5 images sur 5 montrées, la première prête en "),
            "{line}"
        );
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
        let mut view = IntroView::start(sequence(&scratch.0, 6), 0).expect("lancée");
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
        view.close(end, "essai", &mut fake);
    }

    #[test]
    fn an_image_that_comes_back_in_the_sheet_is_decoded_again() {
        let scratch = Scratch::new("revient");
        for index in 0..3 {
            write(&scratch.0, index, SIZE);
        }
        let mut fake = Fake::default();
        let mut view = IntroView::start(sequence(&scratch.0, 3), 0).expect("lancée");
        until(&mut view, 0, &mut fake, |view, _| view.intro.showing().is_some());
        for (position, frame) in [(1u32, 2u64), (2, 4), (3, 7)] {
            until(&mut view, frame, &mut fake, |view, _| view.intro.showing() == Some(position));
        }
        // ses images, dans l'ordre de sa feuille : la première, revenue
        assert_eq!(fake.painted, [0, 1, 0, 2]);
        let end = until(&mut view, 9, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Played { shown: 4 });
        view.close(end, "essai", &mut fake);
    }

    #[test]
    fn without_all_its_images_at_the_right_size_there_is_no_intro() {
        let scratch = Scratch::new("absentes");
        for index in 0..4 {
            write(&scratch.0, index, SIZE);
        }
        let mut fake = Fake::default();
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0).expect("lancée");
        let end = until(&mut view, 0, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::Absent { found: 4 }));
        assert_eq!(view.close(end, "essai", &mut fake), "intro : pas d'images (essai : 4 sur 5)");
        assert!(fake.painted.is_empty() && fake.removed.is_empty() && fake.created == 0);

        // une image d'une autre taille
        write(&scratch.0, 4, (16, 9));
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0).expect("lancée");
        let end = until(&mut view, 0, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::WrongSize { index: 4, found: (16, 9) }));

        // une image illisible dès son en-tête
        std::fs::write(scratch.0.join(capri_core::intro::file(2)), b"pas une image").unwrap();
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0).expect("lancée");
        let end = until(&mut view, 0, &mut fake, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::Unreadable { index: 2 }));

        // la carte graphique refuse la toile
        write(&scratch.0, 2, SIZE);
        write(&scratch.0, 4, SIZE);
        let mut refusing = Fake { refuse: true, ..Fake::default() };
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0).expect("lancée");
        let end = until(&mut view, 0, &mut refusing, |_, _| false).expect("finie");
        assert_eq!(end, End::Missing(Missing::Refused));
        assert_eq!(
            view.close(end, "essai", &mut refusing),
            "intro : pas d'images (essai : refusées par la carte graphique)"
        );
        // pas de dossier du tout
        let mut view = IntroView::start(sequence(&scratch.0.join("nulle-part"), 5), 0).expect("lancée");
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
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0).expect("lancée");
        let thread = view.take_thread().expect("son thread");
        until(&mut view, 0, &mut fake, |view, _| view.intro.showing().is_some());
        until(&mut view, 2, &mut fake, |view, _| view.intro.showing() == Some(1));
        let end = until(&mut view, 4, &mut fake, |_, _| false).expect("arrêtée");
        assert_eq!(end, End::Broken { at: 2 });
        assert!(end.greets(), "jouée jusque-là : elle dit bonjour");
        let line = view.close(end, "essai", &mut fake);
        assert!(line.starts_with("intro : coupée (essai) à l'image 3 sur 5 (illisible)"), "{line}");
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
        let mut view: IntroView<u32> = IntroView::start(sequence(&scratch.0, 5), 0).expect("lancée");
        let thread = view.take_thread().expect("son thread");
        let end = view.cut(Cut::Hidden);
        assert_eq!(
            view.close(end, "essai", &mut fake),
            "intro : coupée (essai) avant sa première image (on la cache)"
        );
        assert!(fake.removed.is_empty(), "pas de toile à lâcher");
        stops(thread);
        // au milieu, une image décodée attendant son heure
        let mut view = IntroView::start(sequence(&scratch.0, 5), 0).expect("lancée");
        let thread = view.take_thread().expect("son thread");
        until(&mut view, 0, &mut fake, |view, _| view.intro.showing().is_some());
        std::thread::sleep(Duration::from_millis(30));
        let end = view.cut(Cut::Talked);
        assert_eq!(end, End::Cut { why: Cut::Talked, at: Some(0) });
        let line = view.close(end, "essai", &mut fake);
        assert!(line.starts_with("intro : coupée (essai) à l'image 1 sur 5 (on lui parle)"), "{line}");
        assert_eq!(fake.removed, [1]);
        stops(thread);
    }

    #[test]
    fn hidden_during_any_intro_an_empty_image_comes_before_the_window_is_hidden() {
        let hidden = End::Cut { why: Cut::Hidden, at: Some(3) };
        for film in [true, false] {
            let closing = closing(hidden, film);
            assert!(closing.blank && !closing.enter, "en images {film} : {closing:?}");
        }
        for end in [
            End::Played { shown: 30 },
            End::Cut { why: Cut::Talked, at: Some(3) },
            End::Cut { why: Cut::Grabbed, at: Some(3) },
            End::Cut { why: Cut::Resized, at: Some(3) },
        ] {
            assert!(!closing(end, true).blank && closing(end, true).enter, "{end:?}");
            assert!(!closing(end, false).blank && !closing(end, false).enter, "{end:?}");
        }
        assert!(closing(End::Missing(Missing::Late), true).enter, "sans intro, elle se matérialise");
    }

    #[test]
    fn see_an_intro_accepted_forgets_a_waiting_greeting_and_refused_changes_nothing() {
        let entry = &capri_core::intro::LIST[0];
        let accepted = encore(Ok(()), entry);
        assert!(accepted.accepted && accepted.forget_greeting && accepted.say.is_none());
        assert_eq!(accepted.log, "« Voir une intro » : Up close");
        for hold in [Hold::Carried, Hold::Away, Hold::Walking, Hold::Talking] {
            let refused = encore(Err(hold), entry);
            assert!(!refused.accepted && !refused.forget_greeting, "{hold:?}");
            assert_eq!(refused.say.is_some(), hold == Hold::Talking, "{hold:?}");
        }
    }

    #[test]
    fn the_journal_says_its_name_numbers_and_nothing_else() {
        use capri_core::intro::UP_CLOSE;
        /// Une autre intro en images, de neuf images d'une autre taille.
        static NINE: Film = Film {
            dir: "",
            files: &["1", "2", "3", "4", "5", "6", "7", "8", "9"],
            size: (1672, 941),
            sheet: &[(0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (7, 2), (8, 2)],
        };
        let up = Some(&UP_CLOSE);
        let lines = [
            journal_line("Up close", up, End::Played { shown: 22 }, Some(Duration::from_millis(31))),
            journal_line("Up close", up, End::Cut { why: Cut::Hidden, at: Some(8) }, None),
            journal_line("Up close", up, End::Broken { at: 3 }, Some(Duration::from_millis(5))),
            journal_line("Up close", up, End::Missing(Missing::Absent { found: 0 }), None),
            journal_line(
                "Up close",
                up,
                End::Missing(Missing::WrongSize { index: 0, found: (1280, 720) }),
                None,
            ),
            journal_line("Up close", up, End::Missing(Missing::Late), None),
            journal_line("neuf", Some(&NINE), End::Missing(Missing::Absent { found: 3 }), None),
            journal_line("tombe du ciel", None, End::Played { shown: 30 }, None),
            journal_line("arrive en marchant", None, End::Cut { why: Cut::Grabbed, at: Some(41) }, None),
            journal_line("se rassemble", None, End::Cut { why: Cut::Talked, at: Some(3) }, None),
            journal_line("rien", None, End::Played { shown: 0 }, None),
        ];
        assert_eq!(
            lines[0],
            "intro : jouée (Up close), 22 images sur 22 montrées, la première prête en 31 ms"
        );
        assert_eq!(lines[1], "intro : coupée (Up close) à l'image 9 sur 22 (on la cache)");
        assert_eq!(
            lines[2],
            "intro : coupée (Up close) à l'image 4 sur 22 (illisible), la première prête en 5 ms"
        );
        assert_eq!(lines[3], "intro : pas d'images (Up close : 0 sur 22)");
        assert_eq!(
            lines[4],
            "intro : pas d'images (Up close : l'image 1 mesure 1280 × 720 au lieu de 1920 × 1080)"
        );
        assert_eq!(lines[5], "intro : pas d'images (Up close : la première n'est pas prête après 2 s)");
        assert_eq!(lines[6], "intro : pas d'images (neuf : 3 sur 9)");
        assert_eq!(lines[7], "intro : jouée (tombe du ciel), en 30 images");
        assert_eq!(lines[8], "intro : coupée (arrive en marchant) après 41 images (on l'attrape)");
        assert_eq!(lines[9], "intro : coupée (se rassemble) après 3 images (on lui parle)");
        assert_eq!(lines[10], "intro : jouée (rien)");
        for line in lines {
            assert!(!line.contains('/') && !line.contains(".png") && !line.contains(".webp"), "{line}");
        }
    }
}
