//! Ce que veut dire un appui de bouton sur elle, indépendamment du système.
//!
//! Sur macOS, « Ctrl + clic » est un clic secondaire à part entière, mais le système l'envoie à
//! l'app comme un clic principal avec la touche Ctrl enfoncée : c'est à l'app de le reconnaître.
//!
//! **Un clic ou une prise** ([`Hand`], que l'app et les essais appellent de la même façon) : un
//! appui principal sur elle n'est pas encore une prise. Le curseur s'éloigne de plus de
//! [`CLICK_SLOP`] points, ou l'appui dure [`CLICK_LONGEST`] : c'est une prise ([`Grab`]), faite là
//! et quand l'appui a commencé, le curseur relevé depuis rejoué ([`Grab::take`]) : elle est là où
//! elle aurait été prise à l'appui, et un lancer a exactement la vitesse d'avant (celle des 80
//! dernières millisecondes), quelle que soit la pause avant le geste. Relâché avant, sans bouger :
//! un clic ([`Let::Click`]) ; mais un clic sur elle quand elle est restée tenue (le bouton relâché
//! sans qu'on le sache : son menu ouvert pendant qu'on la tenait) la lâche ([`Let::Go`]). Un appui
//! devenu prise au relâchement même (un glissé plus court qu'un pas de la boucle) est une prise
//! suivie d'un lâcher : prise tout de suite, lâchée à la fin du pas suivant ([`Hand::let_go`]),
//! pour que tout ce qui suit une prise la voie tenue (se poser, se relever, rester là où on l'a
//! mise).

use crate::body::Capri;
use crate::geom::Vec2;

/// Un appui plus long que ça (en secondes) est une prise, même sans bouger.
pub const CLICK_LONGEST: f64 = 0.3;
/// Un curseur qui s'éloigne de plus que ça (en points) du lieu de l'appui en fait une prise.
pub const CLICK_SLOP: f32 = 4.0;

/// Un appui principal sur elle, à `at` (pixels de l'overlay), commencé à l'instant `since`
/// (secondes), pas encore une prise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    pub at: Vec2,
    pub since: f64,
}

impl Touch {
    pub fn new(at: Vec2, since: f64) -> Self {
        Self { at, since }
    }

    /// Devient-il une prise, le curseur en `cursor` à l'instant `now` ? `slop` : [`CLICK_SLOP`] en
    /// pixels de l'overlay. Un instant qui n'est pas un nombre ne décide rien.
    pub fn grabs(&self, cursor: Option<Vec2>, now: f64, slop: f32) -> bool {
        now - self.since >= CLICK_LONGEST || cursor.is_some_and(|cursor| (cursor - self.at).length() > slop)
    }
}

/// Une prise : là et quand l'appui a commencé, et le curseur relevé à chaque pas depuis.
#[derive(Clone, Debug, PartialEq)]
pub struct Grab {
    pub at: Vec2,
    pub since: f64,
    /// (instant, curseur), dans l'ordre.
    pub trail: Vec<(f64, Vec2)>,
}

impl Grab {
    /// La prise faite sur `capri` : là et quand l'appui a commencé, son image debout replacée
    /// d'abord en `origin` s'il y en a une (assise, elle se lève sous le curseur), tenue vers
    /// l'écart `toward` s'il y en a un (sa mèche, voir `Capri::hold_toward`), puis le curseur
    /// relevé depuis, rejoué comme si on l'avait tenue dès l'appui.
    pub fn take(&self, capri: &mut Capri, origin: Option<Vec2>, toward: Option<Vec2>) {
        match origin {
            Some(origin) => capri.grab_at(self.at, origin, self.since),
            None => capri.grab(self.at, self.since),
        }
        if let Some(offset) = toward {
            capri.hold_toward(offset);
        }
        for &(t, cursor) in &self.trail {
            capri.drag_to(cursor, t);
        }
    }
}

/// Ce que fait le relâchement du bouton principal.
#[derive(Clone, Debug, PartialEq)]
pub enum Let {
    /// Un clic sur elle.
    Click,
    /// La lâcher maintenant (on la tenait).
    Go,
    /// Une prise faite au relâchement même : à prendre tout de suite, et à lâcher à la fin du pas
    /// suivant ([`Hand::let_go`]).
    Grab(Grab),
}

/// La main de l'utilisateur sur elle, de l'appui principal au relâchement (voir le module).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hand {
    /// Un appui pas encore devenu une prise, et le curseur relevé depuis, pas par pas.
    touch: Option<(Touch, Vec<(f64, Vec2)>)>,
    /// Une prise faite au relâchement : à lâcher à la fin du pas suivant, à cet instant.
    letting_go: Option<f64>,
}

impl Hand {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un appui principal sur elle, à `at`, à l'instant `now`.
    pub fn press(&mut self, at: Vec2, now: f64) {
        self.touch = Some((Touch::new(at, now), Vec::new()));
    }

    /// Un appui en attente, ou une prise faite au relâchement pas encore lâchée : la souris ne doit
    /// pas traverser la fenêtre.
    pub fn is_pressing(&self) -> bool {
        self.touch.is_some() || self.letting_go.is_some()
    }

    /// Une prise faite au relâchement attend d'être lâchée : le pas ne la traîne pas (le curseur a
    /// pu bouger depuis le relâchement).
    pub fn is_letting_go(&self) -> bool {
        self.letting_go.is_some()
    }

    /// L'appui en attente est oublié (elle est cachée).
    pub fn cancel(&mut self) {
        self.touch = None;
    }

    /// Au début d'un pas, le curseur relu en `cursor` à l'instant `now` : la prise, dès que l'appui
    /// en devient une ; sinon le curseur est relevé, pour la rejouer. `slop` : [`CLICK_SLOP`] en
    /// pixels de l'overlay.
    pub fn step(&mut self, cursor: Option<Vec2>, now: f64, slop: f32) -> Option<Grab> {
        let (touch, trail) = self.touch.as_mut()?;
        if touch.grabs(cursor, now, slop) {
            let (touch, trail) = self.touch.take()?;
            return Some(Grab { at: touch.at, since: touch.since, trail });
        }
        if let Some(cursor) = cursor {
            trail.push((now, cursor));
        }
        None
    }

    /// Le bouton principal relâché, le curseur en `cursor`, à l'instant `now` ; `held` : on la
    /// tient (pour son pas, elle l'est encore).
    pub fn release(&mut self, cursor: Option<Vec2>, now: f64, slop: f32, held: bool) -> Let {
        let Some((touch, mut trail)) = self.touch.take() else { return Let::Go };
        if touch.grabs(cursor, now, slop) {
            if let Some(cursor) = cursor {
                trail.push((now, cursor));
            }
            self.letting_go = Some(now);
            return Let::Grab(Grab { at: touch.at, since: touch.since, trail });
        }
        if held { Let::Go } else { Let::Click }
    }

    /// À la fin du pas : l'instant où lâcher une prise faite au relâchement, s'il y en a une.
    pub fn let_go(&mut self) -> Option<f64> {
        self.letting_go.take()
    }
}

/// Bouton de souris, tel que le système le rapporte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// Bouton principal (gauche).
    Primary,
    /// Bouton secondaire (droit, ou clic à deux doigts).
    Secondary,
    /// Molette, boutons latéraux…
    Other,
}

/// Ce qu'un appui sur elle déclenche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// On l'attrape.
    Grab,
    /// On ouvre son menu.
    Menu,
    /// Rien.
    Nothing,
}

/// Interprète un appui. `ctrl_click_is_secondary` : sur ce système, Ctrl + clic principal vaut
/// clic secondaire (vrai sur macOS).
pub fn interpret_press(button: Button, ctrl: bool, ctrl_click_is_secondary: bool) -> Press {
    match button {
        Button::Secondary => Press::Menu,
        Button::Primary if ctrl && ctrl_click_is_secondary => Press::Menu,
        Button::Primary => Press::Grab,
        Button::Other => Press::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_grabs_and_secondary_opens_the_menu() {
        for convention in [false, true] {
            assert_eq!(interpret_press(Button::Primary, false, convention), Press::Grab);
            assert_eq!(interpret_press(Button::Secondary, false, convention), Press::Menu);
            assert_eq!(interpret_press(Button::Other, false, convention), Press::Nothing);
        }
    }

    #[test]
    fn a_short_press_without_moving_is_a_click_anything_else_a_grab() {
        let touch = Touch::new(Vec2::new(100.0, 200.0), 10.0);
        // relâché tout de suite, ou un peu plus tard, sans bouger : un clic
        assert!(!touch.grabs(Some(Vec2::new(100.0, 200.0)), 10.0, 8.0));
        assert!(!touch.grabs(Some(Vec2::new(103.0, 205.0)), 10.29, 8.0));
        assert!(!touch.grabs(None, 10.1, 8.0), "le curseur inconnu ne décide rien");
        // le curseur s'éloigne : une prise, même vite
        assert!(touch.grabs(Some(Vec2::new(109.0, 200.0)), 10.01, 8.0));
        // l'appui dure : une prise, même sans bouger
        assert!(touch.grabs(Some(Vec2::new(100.0, 200.0)), 10.0 + CLICK_LONGEST, 8.0));
        assert!(!touch.grabs(None, f64::NAN, 8.0));
    }

    use crate::body::Stance;
    use crate::geom::Rect;

    fn body() -> Capri {
        let screen = Rect::new(Vec2::ZERO, Vec2::new(3024.0, 1964.0));
        Capri::new(Vec2::new(200.0, 664.0), screen, Vec2::new(1400.0, 600.0))
    }

    /// Le curseur : immobile `pause` s, puis 200 px en 60 ms, lâché aussitôt.
    fn flick(t: f64, pause: f64, from: Vec2) -> Vec2 {
        let k = ((t - pause) / 0.06).clamp(0.0, 1.0) as f32;
        from + Vec2::new(200.0 * k, -40.0 * k)
    }

    /// Ce qu'elle devient, lâchée.
    fn speed(capri: &Capri) -> Option<Vec2> {
        match capri.stance() {
            Stance::Thrown { vel } => Some(vel),
            _ => None,
        }
    }

    #[test]
    fn a_throw_after_a_pause_keeps_exactly_the_speed_it_had_before_the_click_could_wait() {
        let from = Vec2::new(1500.0, 900.0);
        for pause in [0.0, 0.05, 0.1, 0.2, 0.28] {
            let end = pause + 0.06;
            // avant : prise à l'appui, chaque pas de 60 Hz la traîne
            let mut before = body();
            before.grab(from, 0.0);
            let mut t = 0.0;
            while t < end {
                t += 1.0 / 60.0;
                before.drag_to(flick(t, pause, from), t);
            }
            before.release(t);
            // maintenant : un appui, qui devient une prise quand le curseur s'éloigne (ou dure)
            let mut now = body();
            let mut hand = Hand::new();
            hand.press(from, 0.0);
            let mut t = 0.0;
            while t < end {
                t += 1.0 / 60.0;
                let cursor = flick(t, pause, from);
                if let Some(grab) = hand.step(Some(cursor), t, 8.0) {
                    grab.take(&mut now, None, None);
                }
                if now.is_held() {
                    now.drag_to(cursor, t);
                }
            }
            match hand.release(Some(flick(t, pause, from)), t, 8.0, now.is_held()) {
                Let::Go => now.release(t),
                other => panic!("pause {pause} : {other:?}"),
            }
            let (was, is) = (speed(&before), speed(&now));
            assert!(was.is_some(), "pause {pause} : lancée, avant");
            assert_eq!(is, was, "pause {pause} : la même vitesse");
            assert_eq!(now.position(), before.position(), "pause {pause} : au même endroit");
        }
    }

    #[test]
    fn a_click_on_her_while_she_is_still_held_lets_her_go() {
        let mut hand = Hand::new();
        hand.press(Vec2::new(10.0, 10.0), 1.0);
        assert!(hand.is_pressing());
        assert_eq!(hand.step(Some(Vec2::new(11.0, 10.0)), 1.05, 8.0), None);
        // pas tenue : un clic ; tenue (le relâchement d'avant perdu) : elle est lâchée
        assert_eq!(hand.clone().release(Some(Vec2::new(11.0, 10.0)), 1.1, 8.0, false), Let::Click);
        assert_eq!(hand.release(Some(Vec2::new(11.0, 10.0)), 1.1, 8.0, true), Let::Go);
        assert!(!hand.is_pressing());
        // sans appui en attente : la lâcher, comme toujours
        assert_eq!(Hand::new().release(None, 2.0, 8.0, true), Let::Go);
    }

    #[test]
    fn a_drag_shorter_than_a_step_is_a_grab_let_go_at_the_end_of_the_next_step() {
        let mut hand = Hand::new();
        let at = Vec2::new(500.0, 500.0);
        hand.press(at, 3.0);
        // appui, glissé de 30 px et relâchement dans le même passage, sans aucun pas entre
        let Let::Grab(grab) = hand.release(Some(at + Vec2::new(30.0, 0.0)), 3.01, 8.0, false) else {
            panic!("une prise")
        };
        assert_eq!(
            (grab.at, grab.since, grab.trail.clone()),
            (at, 3.0, vec![(3.01, at + Vec2::new(30.0, 0.0))])
        );
        assert!(hand.is_pressing() && hand.is_letting_go(), "à lâcher à la fin du pas");
        let mut capri = body();
        let start = capri.position();
        grab.take(&mut capri, None, None);
        assert!(capri.is_held());
        assert_eq!(capri.position(), start + Vec2::new(30.0, 0.0), "elle a suivi le glissé");
        assert_eq!(hand.let_go(), Some(3.01));
        assert_eq!(hand.let_go(), None);
        assert!(!hand.is_pressing());
        // un appui oublié (elle est cachée) ne devient rien
        hand.press(at, 4.0);
        hand.cancel();
        assert_eq!(hand.step(Some(at + Vec2::new(90.0, 0.0)), 5.0, 8.0), None);
        assert_eq!(hand.release(Some(at), 5.0, 8.0, false), Let::Go);
    }

    #[test]
    fn ctrl_click_opens_the_menu_only_where_it_is_the_convention() {
        // macOS : Ctrl + clic arrive comme un clic principal avec Ctrl
        assert_eq!(interpret_press(Button::Primary, true, true), Press::Menu);
        // ailleurs, Ctrl + clic reste un clic ordinaire
        assert_eq!(interpret_press(Button::Primary, true, false), Press::Grab);
        // Ctrl ne change rien aux autres boutons
        assert_eq!(interpret_press(Button::Secondary, true, true), Press::Menu);
        assert_eq!(interpret_press(Button::Other, true, true), Press::Nothing);
    }
}
