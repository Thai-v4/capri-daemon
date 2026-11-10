//! L'image du bandeau : un texte sur une ligne, dans une pastille arrondie.
//!
//! Le texte est mis en forme et tracé sur le processeur par cosmic-text, avec les polices du
//! système (aucune police n'est embarquée). L'image obtenue est ensuite dessinée par le même
//! shader que Capri. On ne la retrace que quand le texte change.

use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};

/// Dimensions du bandeau, en points (pixels logiques).
const FONT_SIZE: f32 = 13.0;
const LINE_HEIGHT: f32 = 17.0;
const PADDING_X: f32 = 12.0;
const PADDING_Y: f32 = 6.0;
const BORDER: f32 = 1.5;
/// Largeur maximale du texte ; au-delà il est coupé.
const MAX_TEXT_WIDTH: f32 = 420.0;

/// Polices sans empattement préférées, de la plus souhaitable à la moins ; à défaut, celle que
/// le système désigne.
const PREFERRED_FAMILIES: [&str; 6] =
    ["SF Pro Text", "Helvetica Neue", "Segoe UI", "Inter", "Noto Sans", "DejaVu Sans"];

/// Couleurs du bandeau, en RGBA 8 bits non prémultiplié.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerStyle {
    pub text: [u8; 4],
    pub fill: [u8; 4],
    pub border: [u8; 4],
}

/// Une image de bandeau, en RGBA 8 bits **non** prémultiplié, comme un sprite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BannerImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Trace les bandeaux. Sa création parcourt les polices du système et peut prendre jusqu'à une
/// seconde : à faire une seule fois, hors du thread qui anime Capri.
pub struct BannerPainter {
    fonts: FontSystem,
    cache: SwashCache,
}

impl Default for BannerPainter {
    fn default() -> Self {
        Self::new()
    }
}

impl BannerPainter {
    pub fn new() -> Self {
        let mut fonts = FontSystem::new();
        let available = |fonts: &FontSystem, family: &str| {
            fonts.db().faces().any(|face| face.families.iter().any(|(name, _)| name == family))
        };
        match PREFERRED_FAMILIES.into_iter().find(|family| available(&fonts, family)) {
            Some(family) => {
                log::debug!("police du bandeau : {family}");
                fonts.db_mut().set_sans_serif_family(family);
            }
            None => log::debug!("police du bandeau : celle du système par défaut"),
        }
        Self { fonts, cache: SwashCache::new() }
    }

    /// Trace `text` sur une ligne. `scale` : densité de l'écran (2.0 en Retina).
    pub fn paint(&mut self, text: &str, style: &BannerStyle, scale: f32) -> BannerImage {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        let (mut buffer, text_width) = self.fit(text, scale);

        let pad_x = (PADDING_X * scale).round();
        let pad_y = (PADDING_Y * scale).round();
        let line_height = (LINE_HEIGHT * scale).ceil();
        let width = (text_width.ceil() + 2.0 * pad_x) as u32;
        let height = (line_height + 2.0 * pad_y) as u32;
        let mut image = pill(width, height, BORDER * scale, style);

        let [r, g, b, a] = style.text;
        let (offset_x, offset_y) = (pad_x as i32, pad_y as i32);
        buffer.draw(&mut self.fonts, &mut self.cache, Color::rgba(r, g, b, a), |x, y, w, h, color| {
            for py in y..y.saturating_add_unsigned(h) {
                for px in x..x.saturating_add_unsigned(w) {
                    image.blend(px.saturating_add(offset_x), py.saturating_add(offset_y), color.as_rgba());
                }
            }
        });
        image
    }

    /// Met le texte en forme sur une ligne, en le coupant tant qu'il dépasse la largeur maximale.
    /// Rend le texte prêt à tracer et sa largeur en pixels.
    fn fit(&mut self, text: &str, scale: f32) -> (Buffer, f32) {
        let max_width = MAX_TEXT_WIDTH * scale;
        let metrics = Metrics::new(FONT_SIZE * scale, LINE_HEIGHT * scale);
        let attrs = Attrs::new().family(Family::SansSerif).weight(Weight::SEMIBOLD);
        // une seule ligne, sans caractère qui en ouvrirait une autre
        let mut shown: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        loop {
            let mut buffer = Buffer::new(&mut self.fonts, metrics);
            buffer.set_wrap(Wrap::None);
            buffer.set_size(None, Some(LINE_HEIGHT * scale));
            buffer.set_text(&shown, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(&mut self.fonts, false);
            let width = buffer.layout_runs().map(|run| run.line_w).fold(0.0, f32::max);
            let chars = shown.chars().count();
            if width <= max_width || chars <= 1 {
                return (buffer, width.min(max_width));
            }
            // trop large : garder la part qui tient, moins un caractère pour les points de suspension
            let keep = ((chars as f32 * max_width / width) as usize).min(chars - 1).saturating_sub(1).max(1);
            shown = shown.chars().take(keep).collect::<String>().trim_end().to_owned();
            shown.push('…');
        }
    }
}

impl BannerImage {
    /// Pose une couleur (non prémultipliée) par-dessus le pixel `(x, y)`, s'il est dans l'image.
    fn blend(&mut self, x: i32, y: i32, [r, g, b, a]: [u8; 4]) {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height || a == 0 {
            return;
        }
        let at = (y as usize * self.width as usize + x as usize) * 4;
        let pixel = &mut self.rgba[at..at + 4];
        let src_a = f32::from(a) / 255.0;
        let dst_a = f32::from(pixel[3]) / 255.0;
        let out_a = src_a + dst_a * (1.0 - src_a);
        for (channel, src) in pixel[..3].iter_mut().zip([r, g, b]) {
            let mixed = (f32::from(src) * src_a + f32::from(*channel) * dst_a * (1.0 - src_a)) / out_a;
            *channel = mixed.round() as u8;
        }
        pixel[3] = (out_a * 255.0).round() as u8;
    }
}

/// Une pastille aux bouts ronds, avec son liseré, aux bords lissés.
fn pill(width: u32, height: u32, border: f32, style: &BannerStyle) -> BannerImage {
    let mut image = BannerImage { width, height, rgba: vec![0; width as usize * height as usize * 4] };
    let (half_w, half_h) = (width as f32 / 2.0, height as f32 / 2.0);
    let radius = half_h.min(half_w);
    for y in 0..height {
        for x in 0..width {
            // distance signée au bord de la pastille : négative à l'intérieur
            let dx = ((x as f32 + 0.5 - half_w).abs() - (half_w - radius)).max(0.0);
            let dy = ((y as f32 + 0.5 - half_h).abs() - (half_h - radius)).max(0.0);
            let distance = dx.hypot(dy) - radius;
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            // le liseré occupe la bande extérieure, le fond le reste
            let ring = (distance + border + 0.5).clamp(0.0, 1.0);
            let mix =
                |fill: u8, edge: u8| (f32::from(fill) * (1.0 - ring) + f32::from(edge) * ring).round() as u8;
            let at = (y as usize * width as usize + x as usize) * 4;
            for channel in 0..4 {
                image.rgba[at + channel] = mix(style.fill[channel], style.border[channel]);
            }
            image.rgba[at + 3] = (f32::from(image.rgba[at + 3]) * coverage).round() as u8;
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::*;

    const STYLE: BannerStyle =
        BannerStyle { text: [255, 255, 255, 255], fill: [20, 16, 36, 240], border: [115, 41, 224, 255] };

    /// Un seul traceur pour tous les tests : sa création parcourt les polices du système.
    fn painter() -> MutexGuard<'static, BannerPainter> {
        static PAINTER: OnceLock<Mutex<BannerPainter>> = OnceLock::new();
        PAINTER
            .get_or_init(|| Mutex::new(BannerPainter::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn pixel(image: &BannerImage, x: u32, y: u32) -> [u8; 4] {
        let at = (y as usize * image.width as usize + x as usize) * 4;
        image.rgba[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn the_pill_is_round_at_the_ends_and_edged() {
        let image = pill(200, 60, 3.0, &STYLE);
        assert_eq!(pixel(&image, 0, 0)[3], 0, "coin transparent");
        assert_eq!(pixel(&image, 199, 59)[3], 0);
        assert_eq!(pixel(&image, 100, 30), STYLE.fill, "centre : le fond");
        assert_eq!(pixel(&image, 100, 1), STYLE.border, "bord haut : le liseré");
        assert_eq!(pixel(&image, 1, 30), STYLE.border, "bout gauche : le liseré");
        let partial = pixel(&image, 100, 0)[3];
        assert!(partial > 0, "le bord est lissé, pas coupé net");
    }

    #[test]
    fn the_image_has_the_size_of_its_text() {
        let mut painter = painter();
        let short = painter.paint("Claude a fini", &STYLE, 2.0);
        let long = painter.paint("Claude lance : cargo test --workspace --all-targets", &STYLE, 2.0);
        assert_eq!(short.rgba.len(), short.width as usize * short.height as usize * 4);
        assert_eq!(short.height, long.height, "une seule ligne dans les deux cas");
        assert_eq!(short.height, (17.0f32 * 2.0 + 2.0 * 12.0) as u32);
        assert!(long.width > short.width + 100, "{} contre {}", long.width, short.width);

        // la densité de l'écran change la taille de l'image, pas sa forme
        let low = painter.paint("Claude a fini", &STYLE, 1.0);
        assert!(low.width * 2 >= short.width.saturating_sub(4) && low.width * 2 <= short.width + 4);
        assert_eq!(low.height * 2, short.height);
    }

    #[test]
    fn the_text_is_actually_drawn() {
        let mut painter = painter();
        let with_text = painter.paint("Claude réfléchit…", &STYLE, 2.0);
        let white = with_text
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 230 && p[1] > 230 && p[2] > 230 && p[3] > 230)
            .count();
        assert!(white > 200, "{white} pixels de texte : une police du système est-elle disponible ?");
        // et il reste dans la pastille : les marges ne portent que le fond
        let middle = with_text.height / 2;
        assert_eq!(pixel(&with_text, 10, middle), STYLE.fill);
        assert_eq!(pixel(&with_text, with_text.width - 11, middle), STYLE.fill);
    }

    #[test]
    fn a_text_too_long_is_cut_to_the_maximum_width() {
        let mut painter = painter();
        let image = painter.paint(&"très long ".repeat(60), &STYLE, 2.0);
        let max = (MAX_TEXT_WIDTH * 2.0 + 2.0 * PADDING_X * 2.0) as u32;
        assert!(image.width <= max + 2, "{} px pour {max} au plus", image.width);
        assert!(image.width > max / 2, "coupé, pas vidé : {} px", image.width);
    }

    #[test]
    fn odd_inputs_still_give_an_image() {
        let mut painter = painter();
        for text in ["", " ", "a\nb\tc", "日本語のテキスト", "🙂 emoji", "\u{202e}droite à gauche"]
        {
            let image = painter.paint(text, &STYLE, 2.0);
            assert!(image.width >= 2 * 24 && image.height > 0, "{text:?} : {}×{}", image.width, image.height);
            assert_eq!(image.rgba.len(), image.width as usize * image.height as usize * 4);
        }
        // une densité absurde ne fait pas tout planter
        let image = painter.paint("x", &STYLE, f32::NAN);
        assert!(image.width > 0 && image.height > 0);
    }

    /// À lancer à la main pour regarder le résultat :
    /// `CAPRI_BANNER_DIR=/tmp/bandeaux cargo test -p capri-render banner_samples -- --ignored`
    #[test]
    #[ignore = "écrit des images PNG dans CAPRI_BANNER_DIR"]
    fn banner_samples() {
        let dir = std::path::PathBuf::from(std::env::var("CAPRI_BANNER_DIR").expect("CAPRI_BANNER_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        let mut painter = painter();
        let samples = [
            ("travail", "Claude lance : cargo test --workspace", STYLE),
            (
                "attente",
                "Claude attend ton accord : Remove node_modules directory",
                BannerStyle {
                    text: [255, 255, 255, 255],
                    fill: [224, 35, 78, 245],
                    border: [255, 214, 224, 255],
                },
            ),
            (
                "fini",
                "Claude a fini",
                BannerStyle {
                    text: [236, 255, 248, 255],
                    fill: [20, 16, 36, 240],
                    border: [64, 220, 170, 255],
                },
            ),
        ];
        for (name, text, style) in samples {
            let image = painter.paint(text, &style, 2.0);
            image::save_buffer(
                dir.join(format!("{name}.png")),
                &image.rgba,
                image.width,
                image.height,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        }
    }
}
