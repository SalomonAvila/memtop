//! memtop logo for the left panel.
//!
//! It is pixel art embedded in the binary, stored as a [`Sprite`] that is rescaled to the
//! exact size of the panel.

use ratatui::style::Color;

/// Pixel image drawn with half blocks (`▀`/`▄`): each terminal cell holds two pixels,
/// one on top and one at the bottom.
pub struct Sprite {
    pub width: usize,
    pub height: usize,
    /// `None` = transparent.
    pub pixels: Vec<Option<Color>>,
}

impl Sprite {
    /// Scale that fits the sprite into `cols × rows` cells. If an integer factor (×2, ×3…)
    /// fills at least 85 % of the space it is used, because pixel art looks sharper;
    /// otherwise, the exact factor that makes it fill the whole panel.
    pub fn scale(&self, cols: u16, rows: u16) -> f64 {
        let s = (cols as f64 / self.width as f64).min(rows as f64 * 2.0 / self.height as f64);
        if s >= 1.0 && s.floor() / s >= 0.85 {
            s.floor()
        } else {
            s
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Option<Color> {
        self.pixels.get(y * self.width + x).copied().flatten()
    }

    /// memtop logo: a monitor with a line chart and bars.
    pub fn logo() -> Self {
        const FRAME: Color = Color::Rgb(120, 132, 156);
        const SCREEN: Color = Color::Rgb(18, 22, 34);
        const LINE: Color = Color::Rgb(90, 220, 150);
        const BAR: Color = Color::Rgb(80, 196, 232);
        const STAND: Color = Color::Rgb(90, 100, 120);
        const BUTTONS: [Color; 3] = [
            Color::Rgb(240, 90, 90),
            Color::Rgb(240, 200, 80),
            Color::Rgb(90, 220, 150),
        ];
        // Heights (in px from the base of the chart) of the line and the bars.
        const CURVE: [usize; 22] = [
            1, 2, 2, 3, 5, 6, 5, 4, 4, 5, 7, 8, 7, 6, 5, 6, 8, 9, 9, 8, 7, 8,
        ];
        const BARS: [usize; 11] = [2, 3, 2, 4, 3, 4, 2, 3, 4, 3, 2];

        let (width, height) = (28, 23);
        let mut sp = Self {
            width,
            height,
            pixels: vec![None; width * height],
        };
        let mut set = |x: usize, y: usize, c: Color| sp.pixels[y * width + x] = Some(c);

        // Monitor: frame and screen.
        for y in 0..18 {
            for x in 0..width {
                let border = y == 0 || y == 17 || x == 0 || x == width - 1;
                set(x, y, if border { FRAME } else { SCREEN });
            }
        }
        for (i, c) in BUTTONS.iter().enumerate() {
            set(2 + i * 2, 2, *c);
        }
        // Line chart (base at y = 13).
        for (i, h) in CURVE.iter().enumerate() {
            set(3 + i, 13 - h, LINE);
        }
        // Bars (base at y = 16).
        for (i, h) in BARS.iter().enumerate() {
            for dy in 0..*h {
                set(3 + i * 2, 16 - dy, BAR);
            }
        }
        // Monitor stand.
        for y in 18..21 {
            for x in 12..16 {
                set(x, y, STAND);
            }
        }
        for x in 7..21 {
            set(x, 21, STAND);
            set(x, 22, STAND);
        }
        sp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Color {
        Color::Rgb(255, 0, 0)
    }

    #[test]
    fn scale_prefers_integer_factors_that_fill_the_panel() {
        let sp = Sprite {
            width: 10,
            height: 10,
            pixels: vec![Some(red()); 100],
        };
        // Fits exactly at ×3.0 in 30 columns × 15 rows (30 px tall).
        assert_eq!(sp.scale(30, 15), 3.0);
        // ×2.2 possible: the integer ×2 fills 91 %, so ×2 is used.
        assert_eq!(sp.scale(22, 11), 2.0);
        // ×1.6 possible: ×1 would only fill 62 %, so the exact scale is used.
        assert_eq!(sp.scale(16, 8), 1.6);
        // Smaller than the sprite: it shrinks.
        assert_eq!(sp.scale(5, 10), 0.5);
    }

    #[test]
    fn logo_has_the_expected_size() {
        let logo = Sprite::logo();
        assert_eq!((logo.width, logo.height), (28, 23));
        assert_eq!(logo.pixels.len(), 28 * 23);
    }
}
