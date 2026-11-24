//! Des planches chargées à la demande, pour ce qu'elle ne fait qu'une petite partie du temps (se
//! reposer : sa pose allongée, son sommeil debout ; plus tard, d'autres jeux). Elles ne sont pas
//! gardées sur la carte graphique en permanence.
//!
//! **Au lancement**, seuls leurs en-têtes sont lus (`assets::rest`) : de quoi savoir si elle
//! pourra s'en servir, sans rien décoder. Quand elle en veut (`capri_core::look::Look::wants`) :
//! - un thread (« capri-repos ») lit chaque planche, la décode, n'en garde que les dessins montrés,
//!   serrés en grille directement depuis la bande (`Shown::pack_strip` : jamais la grille
//!   complète), les prémultiplie, et la passe à la boucle, une à la fois (un canal d'une place : il
//!   n'y a jamais plus d'une grille qui attend, et d'une en route) ; il fait aussi leurs masques de
//!   clic (l'union de leurs dessins), puis s'arrête ;
//! - la boucle confie chaque grille à la carte graphique dès qu'elle arrive (une toile neuve,
//!   peinte en place : une copie en mémoire, sans attendre), **sans jamais attendre le thread** ; la
//!   dernière arrivée, elle donne ses masques à son dessin, qui peut alors s'en servir.
//!
//! **Quand elle n'en veut plus** depuis [`KEEP`], ses textures sont lâchées, et la carte graphique
//! les rend tout de suite (`Renderer::settle`), même si plus rien ne se dessine ensuite.
//!
//! **Le repli** : une planche absente, illisible, d'une autre taille (le fichier a pu changer
//! depuis le lancement), ou refusée par la carte graphique, et tout ce qui a été confié est lâché,
//! une ligne au journal (le nom du fichier et des nombres) ; elle s'en passe jusqu'à son prochain
//! lancement. Jamais une panne.
//!
//! **Pour un autre jeu** : un type qui décrit ses planches ([`Kind`] : leurs fichiers, leurs
//! grilles montrées, les masques auxquels chacune s'ajoute), et un `OnDemand` de plus.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::time::{Duration, Instant};

use capri_core::AlphaMask;
use capri_core::sheets::Shown;
use capri_render::ImageId;

use crate::intro_view::Canvas;

/// Ses planches sont gardées tant de temps après qu'elle n'en veut plus : une minute (elle se
/// rassoit puis se rallonge, la carte graphique ne recommence pas tout).
pub const KEEP: Duration = Duration::from_secs(60);

/// Un jeu de planches chargé à la demande.
pub trait Kind: 'static {
    /// Une de ses planches.
    type Strip: Copy + PartialEq + Send + 'static;
    /// Ses masques de clic, faits sur le thread.
    type Masks: Send + 'static;
    /// Pour le journal : « pose allongée ».
    const NAME: &'static str;
    /// La taille des cases de chacun de ses masques de clic.
    const MASKS: &'static [(u32, u32)];
    /// Ses planches, toutes nécessaires, dans l'ordre où elles sont lues.
    fn strips() -> &'static [Self::Strip];
    /// Son fichier, dans le dossier des images (`capri/laydown/laydown.png`).
    fn file(strip: Self::Strip) -> String;
    /// Ses dessins montrés, serrés dans la grille confiée à la carte graphique.
    fn shown(strip: Self::Strip) -> Shown;
    /// Le masque de clic (un indice de [`Kind::MASKS`]) auquel s'ajoutent ses dessins, s'il y en a un.
    fn mask(strip: Self::Strip) -> Option<usize>;
    /// Ses masques, faits.
    fn masks(masks: Vec<AlphaMask>) -> Self::Masks;
}

/// Ce que le thread passe à la boucle.
enum Loaded<K: Kind> {
    /// Une planche, en grille, prémultipliée.
    Grid { strip: K::Strip, size: (u32, u32), pixels: Vec<u8> },
    /// Toutes là : leurs masques de clic.
    Done(K::Masks),
    /// Une planche manque, est illisible, ou n'a pas la bonne taille : pour le journal.
    Failed(String),
}

enum State<K: Kind, Id> {
    /// Ses planches manquaient au lancement, ou un chargement a échoué : jamais.
    Never,
    /// Rien de chargé.
    Idle,
    /// Le thread les prépare ; celles déjà confiées.
    Loading { from: Receiver<Loaded<K>>, ids: Vec<(K::Strip, Id)>, started: Instant },
    /// Toutes confiées ; depuis quand elle n'en veut plus.
    Ready { ids: Vec<(K::Strip, Id)>, unwanted: Option<Instant> },
}

/// Ce qui vient de changer, à dire à son dessin.
pub enum Change<M> {
    Nothing,
    /// Toutes confiées : leurs masques de clic.
    Dressed(M),
    /// Lâchées.
    Undressed,
    /// Un chargement a échoué : elle s'en passe jusqu'à son prochain lancement.
    Failed,
}

/// Un jeu de planches chargé à la demande, confié à une carte graphique dont les images portent
/// des numéros `Id` (ceux de `capri-render`, ou d'une fausse dans les essais).
pub struct OnDemand<K: Kind, Id: Copy = ImageId> {
    state: State<K, Id>,
    /// Le dossier des images, où ses planches étaient au lancement.
    dir: PathBuf,
}

impl<K: Kind, Id: Copy> OnDemand<K, Id> {
    /// `dir` : le dossier des images, si ses planches y sont (leurs en-têtes, lus au lancement) ;
    /// `None` : jamais.
    pub fn new(dir: Option<PathBuf>) -> Self {
        match dir {
            Some(dir) => Self { state: State::Idle, dir },
            None => Self { state: State::Never, dir: PathBuf::new() },
        }
    }

    /// Ses planches sont-elles là, et rien n'a échoué (pour les essais) ?
    #[cfg(test)]
    pub fn is_available(&self) -> bool {
        !matches!(self.state, State::Never)
    }

    /// L'image confiée pour cette planche, une fois toutes confiées.
    pub fn image(&self, strip: K::Strip) -> Option<Id> {
        match &self.state {
            State::Ready { ids, .. } => {
                ids.iter().find(|(confided, _)| *confided == strip).map(|&(_, id)| id)
            }
            State::Never | State::Idle | State::Loading { .. } => None,
        }
    }

    /// Le thread est-il en route (pour les essais) ?
    #[cfg(test)]
    pub fn is_loading(&self) -> bool {
        matches!(self.state, State::Loading { .. })
    }

    /// Un pas, à l'instant `now` : `wanted`, en veut-elle maintenant ? Commence le chargement,
    /// confie ce qui est prêt à la carte graphique `gpu`, ou lâche tout. Ne bloque jamais sur le
    /// thread.
    pub fn step(&mut self, wanted: bool, now: Instant, gpu: &mut impl Canvas<Id = Id>) -> Change<K::Masks> {
        match &mut self.state {
            State::Never => Change::Nothing,
            State::Idle if !wanted => Change::Nothing,
            State::Idle => match spawn::<K>(self.dir.clone()) {
                Ok(from) => {
                    log::debug!("{} : chargement", K::NAME);
                    self.state = State::Loading { from, ids: Vec::new(), started: now };
                    Change::Nothing
                }
                Err(err) => self.fail(&format!("pas de thread pour les lire ({err})"), gpu),
            },
            State::Loading { from, ids, started } => loop {
                match from.try_recv() {
                    Ok(Loaded::Grid { strip, size, pixels }) => {
                        let confided = gpu.create(size).and_then(|id| {
                            ids.push((strip, id));
                            gpu.paint(id, &pixels)
                        });
                        if let Err(err) = confided {
                            return self.fail(&format!("refusée par la carte graphique ({err})"), gpu);
                        }
                    }
                    Ok(Loaded::Done(masks)) => {
                        let bytes: u64 = K::strips()
                            .iter()
                            .map(|&strip| {
                                let (w, h) = K::shown(strip).grid_size();
                                u64::from(w) * u64::from(h) * 4
                            })
                            .sum();
                        log::info!(
                            "{} : chargée en {} ms, {:.1} Mo de mémoire graphique",
                            K::NAME,
                            now.saturating_duration_since(*started).as_millis(),
                            bytes as f64 / 1_000_000.0
                        );
                        let ids = std::mem::take(ids);
                        self.state = State::Ready { ids, unwanted: (!wanted).then_some(now) };
                        return Change::Dressed(masks);
                    }
                    Ok(Loaded::Failed(why)) => return self.fail(&why, gpu),
                    Err(TryRecvError::Empty) => return Change::Nothing,
                    // le thread s'est arrêté sans rien dire
                    Err(TryRecvError::Disconnected) => return self.fail("le thread s'est arrêté", gpu),
                }
            },
            State::Ready { unwanted, .. } if wanted => {
                *unwanted = None;
                Change::Nothing
            }
            State::Ready { ids, unwanted } => {
                let since = *unwanted.get_or_insert(now);
                if now.saturating_duration_since(since) < KEEP {
                    return Change::Nothing;
                }
                for &(_, id) in ids.iter() {
                    gpu.remove(id);
                }
                // rendues tout de suite, même si plus rien ne se dessine ensuite (cachée)
                gpu.settle();
                log::info!("{} : rendue à la carte graphique", K::NAME);
                self.state = State::Idle;
                Change::Undressed
            }
        }
    }

    /// Le chargement a échoué (`why`, pour le journal) : ce qui a été confié est lâché, et elle
    /// s'en passe jusqu'à son prochain lancement.
    fn fail(&mut self, why: &str, gpu: &mut impl Canvas<Id = Id>) -> Change<K::Masks> {
        if let State::Loading { ids, .. } | State::Ready { ids, .. } = &self.state {
            for &(_, id) in ids {
                gpu.remove(id);
            }
            gpu.settle();
        }
        log::warn!("{} : {why} ; elle s'en passe jusqu'à son prochain lancement", K::NAME);
        self.state = State::Never;
        Change::Failed
    }
}

/// Lance le thread qui prépare les planches de `K`, prises dans le dossier d'images `dir`.
fn spawn<K: Kind>(dir: PathBuf) -> std::io::Result<Receiver<Loaded<K>>> {
    // une place : une grille attend la boucle pendant que le thread fait la suivante, pas plus
    let (sender, from) = sync_channel(1);
    std::thread::Builder::new().name("capri-repos".to_owned()).spawn(move || prepare::<K>(&dir, &sender))?;
    Ok(from)
}

/// Le travail du thread : chaque planche lue, décodée, serrée en grille depuis sa bande,
/// prémultipliée et passée à la boucle ; puis leurs masques. Il s'arrête à la première planche qui
/// manque, ou dès que plus personne n'attend (la boucle a lâché le canal).
fn prepare<K: Kind>(dir: &Path, sender: &SyncSender<Loaded<K>>) {
    let mut unions: Vec<Vec<u8>> =
        K::MASKS.iter().map(|&(w, h)| vec![0u8; (w as usize) * (h as usize)]).collect();
    for &strip in K::strips() {
        let file = K::file(strip);
        let shown = K::shown(strip);
        let image = match image::open(dir.join(&file)) {
            Ok(image) => image.into_rgba8(),
            Err(err) => {
                let _ = sender.send(Loaded::Failed(format!("{file} absente ou illisible ({err})")));
                return;
            }
        };
        let (width, height) = image.dimensions();
        let mut pixels = match shown.pack_strip(width, height, image.as_raw()) {
            Ok(pixels) => pixels,
            Err(wrong) => {
                let (w, h) = wrong.expected;
                let _ = sender
                    .send(Loaded::Failed(format!("{file} mesure {width} × {height} au lieu de {w} × {h}")));
                return;
            }
        };
        if let Some(index) = K::mask(strip)
            && let (Some(union), Some(&cell)) = (unions.get_mut(index), K::MASKS.get(index))
        {
            accumulate(union, cell, &image, shown);
        }
        // la bande lâchée avant d'envoyer : jamais plus d'une bande en mémoire
        drop(image);
        capri_render::premultiply_in_place(&mut pixels);
        if sender.send(Loaded::Grid { strip, size: shown.grid_size(), pixels }).is_err() {
            return;
        }
    }
    let masks = unions
        .iter()
        .zip(K::MASKS)
        .map(|(union, &(w, h))| {
            let rgba: Vec<u8> = union.iter().flat_map(|&alpha| [0, 0, 0, alpha]).collect();
            AlphaMask::from_rgba(w, h, &rgba, crate::assets::ALPHA_THRESHOLD, crate::assets::GRAB_MARGIN)
        })
        .collect();
    let _ = sender.send(Loaded::Done(K::masks(masks)));
}

/// Ajoute à `union` (l'alpha d'une case de `cell` pixels) l'alpha de chaque dessin de la bande
/// `strip` (tous, montrés ou non, comme les masques de ses autres poses), si ses cases ont cette
/// taille.
fn accumulate(union: &mut [u8], cell: (u32, u32), strip: &image::RgbaImage, shown: Shown) {
    let (cw, ch) = cell;
    if shown.cell() != cell || union.len() != (cw as usize) * (ch as usize) {
        return;
    }
    for index in 0..shown.drawings() {
        for y in 0..ch {
            for x in 0..cw {
                let alpha = strip.get_pixel(index * cw + x, y)[3];
                let at = &mut union[(y * cw + x) as usize];
                *at = (*at).max(alpha);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use capri_core::sheets::life::{self, Strip};
    use capri_render::RenderError;

    use super::*;

    /// Un petit jeu d'essai : le sommeil et le réveil debout, de vraies tailles, dans un dossier
    /// d'essai.
    struct Nap;

    impl Kind for Nap {
        type Strip = Strip;
        type Masks = Vec<AlphaMask>;
        const NAME: &'static str = "essai";
        const MASKS: &'static [(u32, u32)] = &[(240, 384)];
        fn strips() -> &'static [Strip] {
            &Strip::NAP
        }
        fn file(strip: Strip) -> String {
            format!("{}/{}", life::DIR, strip.file())
        }
        fn shown(strip: Strip) -> Shown {
            life::shown(strip)
        }
        fn mask(strip: Strip) -> Option<usize> {
            (strip == Strip::Sleep).then_some(0)
        }
        fn masks(masks: Vec<AlphaMask>) -> Vec<AlphaMask> {
            masks
        }
    }

    /// Une fausse carte graphique.
    #[derive(Default)]
    struct Fake {
        made: Vec<(u32, u32)>,
        painted: usize,
        removed: Vec<u32>,
        settled: usize,
        refuse: bool,
    }

    impl Canvas for Fake {
        type Id = u32;
        fn create(&mut self, size: (u32, u32)) -> Result<u32, RenderError> {
            if self.refuse {
                return Err(RenderError::TooLarge { width: size.0, height: size.1, max: 16 });
            }
            self.made.push(size);
            Ok(self.made.len() as u32)
        }
        fn paint(&mut self, id: u32, pixels: &[u8]) -> Result<(), RenderError> {
            assert!(id >= 1 && !pixels.is_empty());
            self.painted += 1;
            Ok(())
        }
        fn remove(&mut self, id: u32) {
            self.removed.push(id);
        }
        fn settle(&mut self) {
            self.settled += 1;
        }
    }

    /// Un dossier d'essai, retiré à la fin, même en cas d'échec.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("capri-test-a-la-demande-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join(life::DIR)).unwrap();
            Self(dir)
        }

        fn write(&self, strip: Strip, size: (u32, u32)) {
            let mut image = image::RgbaImage::new(size.0, size.1);
            image.put_pixel(5, 5, image::Rgba([255, 0, 0, 255]));
            image.save(self.0.join(life::DIR).join(strip.file())).unwrap();
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Fait des pas jusqu'à ce que quelque chose change (cinq secondes au plus).
    fn until_change(demand: &mut OnDemand<Nap, u32>, gpu: &mut Fake, now: Instant) -> Change<Vec<AlphaMask>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match demand.step(true, now, gpu) {
                Change::Nothing => {}
                change => return change,
            }
            assert!(Instant::now() < deadline, "rien n'arrive");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn its_sheets_are_loaded_when_wanted_kept_a_while_then_let_go() {
        let scratch = Scratch::new("chargees");
        scratch.write(Strip::Sleep, Strip::Sleep.strip_size());
        scratch.write(Strip::Wake, Strip::Wake.strip_size());
        let mut gpu = Fake::default();
        let mut demand = OnDemand::<Nap, u32>::new(Some(scratch.0.clone()));
        let start = Instant::now();
        // pas voulues : rien
        assert!(matches!(demand.step(false, start, &mut gpu), Change::Nothing));
        assert!(!demand.is_loading());
        // voulues : le thread, puis les deux grilles confiées, et le masque
        let Change::Dressed(masks) = until_change(&mut demand, &mut gpu, start) else {
            panic!("elles devaient être là")
        };
        assert_eq!(gpu.made, [life::shown(Strip::Sleep).grid_size(), life::shown(Strip::Wake).grid_size()]);
        assert_eq!(gpu.painted, 2);
        assert_eq!(masks.len(), 1);
        assert!(
            masks[0].hit(capri_core::Vec2::new(5.5 / 240.0, 5.5 / 384.0)),
            "le point plein de chaque case"
        );
        assert!(demand.image(Strip::Sleep).is_some() && demand.image(Strip::Wake).is_some());
        // plus voulues : gardées un moment, puis lâchées et rendues
        assert!(matches!(demand.step(false, start, &mut gpu), Change::Nothing));
        assert!(matches!(demand.step(false, start + KEEP / 2, &mut gpu), Change::Nothing));
        // voulues de nouveau entre-temps : l'attente repart de zéro
        assert!(matches!(demand.step(true, start + KEEP / 2, &mut gpu), Change::Nothing));
        assert!(matches!(demand.step(false, start + KEEP, &mut gpu), Change::Nothing));
        assert!(gpu.removed.is_empty());
        assert!(matches!(demand.step(false, start + KEEP * 2, &mut gpu), Change::Undressed));
        assert_eq!(gpu.removed.len(), 2);
        assert_eq!(gpu.settled, 1);
        assert!(demand.image(Strip::Sleep).is_none());
    }

    #[test]
    fn a_missing_or_wrong_sheet_or_a_refusal_lets_everything_go_and_never_again() {
        // le réveil absent (disparu depuis le lancement)
        let scratch = Scratch::new("absente");
        scratch.write(Strip::Sleep, Strip::Sleep.strip_size());
        let mut gpu = Fake::default();
        let mut demand = OnDemand::<Nap, u32>::new(Some(scratch.0.clone()));
        let now = Instant::now();
        assert!(matches!(until_change(&mut demand, &mut gpu, now), Change::Failed));
        assert_eq!(gpu.removed.len(), gpu.made.len(), "ce qui a été confié est lâché");
        assert!(!demand.is_available());
        // jamais plus, même voulues
        assert!(matches!(demand.step(true, now, &mut gpu), Change::Nothing));
        assert!(!demand.is_loading());
        // d'une autre taille
        scratch.write(Strip::Wake, (100, 100));
        let mut gpu = Fake::default();
        let mut demand = OnDemand::<Nap, u32>::new(Some(scratch.0.clone()));
        assert!(matches!(until_change(&mut demand, &mut gpu, now), Change::Failed));
        assert_eq!(gpu.removed.len(), gpu.made.len());
        // illisible
        std::fs::write(scratch.0.join(life::DIR).join(Strip::Wake.file()), b"pas une image").unwrap();
        let mut gpu = Fake::default();
        let mut demand = OnDemand::<Nap, u32>::new(Some(scratch.0.clone()));
        assert!(matches!(until_change(&mut demand, &mut gpu, now), Change::Failed));
        assert_eq!(gpu.removed.len(), gpu.made.len());
        // refusées par la carte graphique
        scratch.write(Strip::Wake, Strip::Wake.strip_size());
        let mut gpu = Fake { refuse: true, ..Fake::default() };
        let mut demand = OnDemand::<Nap, u32>::new(Some(scratch.0.clone()));
        assert!(matches!(until_change(&mut demand, &mut gpu, now), Change::Failed));
        assert!(gpu.made.is_empty() && gpu.removed.is_empty());
        // absentes au lancement : jamais rien
        let mut demand = OnDemand::<Nap, u32>::new(None);
        assert!(matches!(demand.step(true, now, &mut Fake::default()), Change::Nothing));
        assert!(!demand.is_loading() && !demand.is_available());
    }
}
