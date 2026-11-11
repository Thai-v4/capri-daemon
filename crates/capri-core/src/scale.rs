//! Sa taille, choisie par l'utilisateur.
//!
//! **Une échelle, pas des dimensions.** Sa taille est une échelle [`Scale`] rapportée à sa hauteur
//! **debout** d'origine ([`STANDING_POINTS`], 300 points) : 1 est la taille d'origine, 0,5 la
//! moitié. Elle ne dit pas « tant de pixels de haut » : elle dit combien de pixels d'écran vaut un
//! pixel de son dessin debout ([`Scale::pixels_per_image_pixel`]). Un autre dessin d'elle (assise,
//! par exemple), d'autres proportions, se dessine à la **même** densité ([`Scale::display_size`]) :
//! son visage et son trait gardent la même taille d'un dessin à l'autre.
//!
//! **Les bornes.** De [`MIN`] (40 %, 120 points : elle se pose sur bien plus de fenêtres, et son
//! dessin, réduit, reste lisible) à [`MAX`] (140 %, 420 points). La borne haute a été choisie pour
//! son dessin d'avant, de 420 pixels de haut : à 140 %, sur un écran Retina (2 pixels par point),
//! un pixel du dessin y couvre 2 pixels de l'écran, et il est déjà flou. Ses planches (sa silhouette
//! debout : 347 pixels, `sheets::stand::STATURE`) le sont davantage : agrandies 1,73 fois à la
//! taille d'origine, 2,42 fois à 140 %. La borne n'a pas changé avec elles.
//!
//! **Les pas.** Les menus avancent par crans de 10 % (« Plus grande », « Plus petite », alignés sur
//! les dizaines) ; la molette, par crans de 2 % de sa taille du moment, assez fins pour sembler
//! libres. Toute taille est arrondie au centième.

use crate::geom::Vec2;

/// Sa hauteur debout à la taille d'origine, en points (pixels logiques).
pub const STANDING_POINTS: f32 = 300.0;
/// La plus petite échelle : 120 points de haut.
pub const MIN: f32 = 0.4;
/// La plus grande : 420 points de haut (voir plus haut pourquoi pas davantage).
pub const MAX: f32 = 1.4;
/// Un cran des menus.
pub const MENU_STEP: f32 = 0.1;
/// Un cran de molette : 2 % de sa taille du moment.
pub const WHEEL_STEP: f32 = 0.02;

/// Sa taille, rapportée à sa hauteur debout d'origine : toujours dans [`MIN`, `MAX`], au centième.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale(f32);

impl Default for Scale {
    fn default() -> Self {
        Self::ORIGINAL
    }
}

impl Scale {
    /// La taille d'origine.
    pub const ORIGINAL: Self = Self(1.0);

    /// Une échelle quelconque, ramenée dans les bornes et au centième ; ce qui n'est pas un nombre
    /// donne la taille d'origine.
    pub fn new(value: f32) -> Self {
        if value.is_nan() {
            return Self::ORIGINAL;
        }
        Self(((value.clamp(MIN, MAX) * 100.0).round() / 100.0).clamp(MIN, MAX))
    }

    pub fn get(self) -> f32 {
        self.0
    }

    /// En pour cent de la taille d'origine, pour l'afficher.
    pub fn percent(self) -> u32 {
        (self.0 * 100.0).round() as u32
    }

    /// Le cran de menu au-dessus : la dizaine de pour cent suivante (87 % → 90 %, 90 % → 100 %).
    pub fn larger(self) -> Self {
        let tenths = (self.0 / MENU_STEP + 1e-3).floor();
        Self::new((tenths + 1.0) * MENU_STEP)
    }

    /// Le cran de menu en dessous : la dizaine de pour cent précédente (87 % → 80 %).
    pub fn smaller(self) -> Self {
        let tenths = (self.0 / MENU_STEP - 1e-3).ceil();
        Self::new((tenths - 1.0) * MENU_STEP)
    }

    /// `steps` crans de molette : positif, plus grande ; négatif, plus petite. Chaque cran change
    /// sa taille de [`WHEEL_STEP`] de ce qu'elle est : le même geste fait le même effet à l'œil,
    /// petite ou grande.
    pub fn zoomed(self, steps: f64) -> Self {
        if !steps.is_finite() {
            return self;
        }
        let factor = (1.0 + f64::from(WHEEL_STEP)).powf(steps.clamp(-100.0, 100.0));
        let next = Self::new((f64::from(self.0) * factor) as f32);
        // un cran, si petit soit-il, fait bouger d'au moins un centième, dans son sens
        match next.0.partial_cmp(&self.0) {
            Some(std::cmp::Ordering::Equal) if steps > 0.0 => Self::new(self.0 + 0.01),
            Some(std::cmp::Ordering::Equal) if steps < 0.0 => Self::new(self.0 - 0.01),
            _ => next,
        }
    }

    pub fn is_largest(self) -> bool {
        self.0 >= MAX
    }

    pub fn is_smallest(self) -> bool {
        self.0 <= MIN
    }

    /// Sa hauteur debout, en points.
    pub fn standing_points(self) -> f32 {
        STANDING_POINTS * self.0
    }

    /// Combien de pixels d'écran vaut un pixel de son dessin, pour un dessin debout de
    /// `standing_image_height` pixels de haut et un écran de `density` pixels par point. C'est
    /// cette densité que partagent tous ses dessins.
    pub fn pixels_per_image_pixel(self, standing_image_height: u32, density: f64) -> f32 {
        self.standing_points() * density as f32 / standing_image_height.max(1) as f32
    }

    /// La taille à l'écran, en pixels physiques, d'un dessin de `image` pixels (le debout, ou un
    /// autre), à la même densité que son dessin debout de `standing_image_height` pixels de haut.
    pub fn display_size(self, image: (u32, u32), standing_image_height: u32, density: f64) -> Vec2 {
        let k = self.pixels_per_image_pixel(standing_image_height, density);
        Vec2::new(image.0 as f32 * k, image.1 as f32 * k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_value_is_brought_within_bounds_to_the_hundredth() {
        assert_eq!(Scale::new(1.0), Scale::ORIGINAL);
        assert_eq!(Scale::default(), Scale::ORIGINAL);
        assert_eq!(Scale::new(0.1).get(), MIN);
        assert_eq!(Scale::new(9.0).get(), MAX);
        assert_eq!(Scale::new(-1.0).get(), MIN);
        assert_eq!(Scale::new(f32::NAN), Scale::ORIGINAL);
        assert_eq!(Scale::new(f32::INFINITY).get(), MAX);
        assert_eq!(Scale::new(0.876).get(), 0.88);
        assert_eq!(Scale::new(0.876).percent(), 88);
    }

    #[test]
    fn the_menus_step_by_tens_of_percent_and_stop_at_the_bounds() {
        let at = |percent: f32| Scale::new(percent / 100.0);
        assert_eq!(at(100.0).larger(), at(110.0));
        assert_eq!(at(100.0).smaller(), at(90.0));
        // une taille venue de la molette rejoint la dizaine la plus proche dans le sens voulu
        assert_eq!(at(87.0).larger(), at(90.0));
        assert_eq!(at(87.0).smaller(), at(80.0));
        // malgré les arrondis des flottants, 70 % ne saute pas 80 %
        assert_eq!(at(70.0).larger(), at(80.0));
        assert_eq!(at(30.0).smaller().get(), MIN);
        // aux bornes, elle n'en sort pas
        assert!(at(140.0).is_largest() && at(140.0).larger().is_largest());
        assert!(at(40.0).is_smallest() && at(40.0).smaller().is_smallest());
        assert_eq!(at(135.0).larger().get(), MAX);
        // de la plus petite à la plus grande, puis retour, onze tailles
        let mut scale = at(40.0);
        let mut seen = vec![scale.percent()];
        while !scale.is_largest() {
            scale = scale.larger();
            seen.push(scale.percent());
        }
        assert_eq!(seen, [40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140]);
        while !scale.is_smallest() {
            scale = scale.smaller();
        }
        assert_eq!(scale.percent(), 40);
    }

    #[test]
    fn the_wheel_changes_her_size_by_small_proportional_steps() {
        let one = Scale::ORIGINAL.zoomed(1.0);
        assert_eq!(one.percent(), 102);
        assert_eq!(Scale::ORIGINAL.zoomed(-1.0).percent(), 98);
        // dix crans dans un sens puis dix dans l'autre : à un centième près, d'où elle partait
        let there_and_back = Scale::ORIGINAL.zoomed(10.0).zoomed(-10.0);
        assert!((there_and_back.get() - 1.0).abs() <= 0.011, "{there_and_back:?}");
        // un geste d'un dixième de cran (pavé tactile) fait quand même bouger, dans son sens
        assert!(Scale::ORIGINAL.zoomed(0.1).get() > 1.0);
        assert!(Scale::ORIGINAL.zoomed(-0.1).get() < 1.0);
        // jamais hors des bornes, même à grands coups ; ce qui n'est pas un nombre ne change rien
        assert_eq!(Scale::ORIGINAL.zoomed(1e9).get(), MAX);
        assert_eq!(Scale::ORIGINAL.zoomed(-1e9).get(), MIN);
        assert_eq!(Scale::new(0.7).zoomed(f64::NAN), Scale::new(0.7));
        assert_eq!(Scale::new(MAX).zoomed(1.0).get(), MAX);
        assert_eq!(Scale::new(MIN).zoomed(-1.0).get(), MIN);
    }

    #[test]
    fn every_drawing_shares_the_density_of_her_standing_one() {
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        // son dessin debout : 220 × 420 pixels ; à l'origine, 300 points de haut, sur un écran Retina
        let standing = Scale::ORIGINAL.display_size((220, 420), 420, 2.0);
        assert!(near(standing.y, 600.0) && near(standing.x, 314.2857), "{standing:?}");
        // à 50 %, deux fois plus petite ; sur un écran standard, deux fois moins de pixels
        assert!(near(Scale::new(0.5).display_size((220, 420), 420, 2.0).y, 300.0));
        assert!(near(Scale::ORIGINAL.display_size((220, 420), 420, 1.0).y, 300.0));
        // un autre dessin (assise, 300 × 300) à la même densité : le même nombre de pixels d'écran
        // par pixel de dessin
        let sitting = Scale::new(0.5).display_size((300, 300), 420, 2.0);
        let k = Scale::new(0.5).pixels_per_image_pixel(420, 2.0);
        assert_eq!(sitting, Vec2::new(300.0 * k, 300.0 * k));
        // la borne haute : 2 pixels d'écran par pixel du dessin sur un écran Retina
        assert!((Scale::new(MAX).pixels_per_image_pixel(420, 2.0) - 2.0).abs() < 1e-5);
        assert!(near(Scale::new(MAX).standing_points(), 420.0));
        assert!(near(Scale::new(MIN).standing_points(), 120.0));
        // une hauteur de dessin nulle ne divise pas par zéro
        assert!(Scale::ORIGINAL.pixels_per_image_pixel(0, 2.0).is_finite());
    }
}
