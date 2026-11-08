//! Logo de memtop para el panel izquierdo.
//!
//! Es pixel art incluido en el binario, guardado como un [`Sprite`] que se reescala al
//! tamaño exacto del panel.

use ratatui::style::Color;

/// Imagen de píxeles que se dibuja con medios bloques (`▀`/`▄`):
/// cada celda de la terminal son dos píxeles, arriba y abajo.
pub struct Sprite {
    pub ancho: usize,
    pub alto: usize,
    /// `None` = transparente.
    pub px: Vec<Option<Color>>,
}

impl Sprite {
    /// Escala que hace caber el sprite en `cols × filas` celdas. Si un factor entero
    /// (×2, ×3…) llena al menos el 85 % del espacio se usa ese, porque el pixel art
    /// se ve más nítido; si no, el factor exacto para que ocupe todo el panel.
    pub fn escala(&self, cols: u16, filas: u16) -> f64 {
        let s = (cols as f64 / self.ancho as f64).min(filas as f64 * 2.0 / self.alto as f64);
        if s >= 1.0 && s.floor() / s >= 0.85 {
            s.floor()
        } else {
            s
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Option<Color> {
        self.px.get(y * self.ancho + x).copied().flatten()
    }

    /// Logo de memtop: un monitor con una gráfica de línea y barras.
    pub fn logo() -> Self {
        const MARCO: Color = Color::Rgb(120, 132, 156);
        const PANTALLA: Color = Color::Rgb(18, 22, 34);
        const LINEA: Color = Color::Rgb(90, 220, 150);
        const BARRA: Color = Color::Rgb(80, 196, 232);
        const PIE: Color = Color::Rgb(90, 100, 120);
        const BOTONES: [Color; 3] = [
            Color::Rgb(240, 90, 90),
            Color::Rgb(240, 200, 80),
            Color::Rgb(90, 220, 150),
        ];
        // Alturas (en px desde la base de la gráfica) de la línea y de las barras.
        const CURVA: [usize; 22] = [
            1, 2, 2, 3, 5, 6, 5, 4, 4, 5, 7, 8, 7, 6, 5, 6, 8, 9, 9, 8, 7, 8,
        ];
        const BARRAS: [usize; 11] = [2, 3, 2, 4, 3, 4, 2, 3, 4, 3, 2];

        let (ancho, alto) = (28, 23);
        let mut sp = Self {
            ancho,
            alto,
            px: vec![None; ancho * alto],
        };
        let mut poner = |x: usize, y: usize, c: Color| sp.px[y * ancho + x] = Some(c);

        // Monitor: marco y pantalla.
        for y in 0..18 {
            for x in 0..ancho {
                let borde = y == 0 || y == 17 || x == 0 || x == ancho - 1;
                poner(x, y, if borde { MARCO } else { PANTALLA });
            }
        }
        for (i, c) in BOTONES.iter().enumerate() {
            poner(2 + i * 2, 2, *c);
        }
        // Gráfica de línea (base en y = 13).
        for (i, h) in CURVA.iter().enumerate() {
            poner(3 + i, 13 - h, LINEA);
        }
        // Barras (base en y = 16).
        for (i, h) in BARRAS.iter().enumerate() {
            for dy in 0..*h {
                poner(3 + i * 2, 16 - dy, BARRA);
            }
        }
        // Pie del monitor.
        for y in 18..21 {
            for x in 12..16 {
                poner(x, y, PIE);
            }
        }
        for x in 7..21 {
            poner(x, 21, PIE);
            poner(x, 22, PIE);
        }
        sp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rojo() -> Color {
        Color::Rgb(255, 0, 0)
    }

    #[test]
    fn la_escala_prefiere_factores_enteros_si_llenan_el_panel() {
        let sp = Sprite {
            ancho: 10,
            alto: 10,
            px: vec![Some(rojo()); 100],
        };
        // Cabe ×3.0 exacto en 30 columnas × 15 filas (30 px de alto).
        assert_eq!(sp.escala(30, 15), 3.0);
        // ×2.2 posible: el entero ×2 llena el 91 %, así que se usa ×2.
        assert_eq!(sp.escala(22, 11), 2.0);
        // ×1.6 posible: ×1 solo llenaría el 62 %, así que se usa la escala exacta.
        assert_eq!(sp.escala(16, 8), 1.6);
        // Más pequeño que el sprite: se reduce.
        assert_eq!(sp.escala(5, 10), 0.5);
    }

    #[test]
    fn el_logo_tiene_el_tamano_esperado() {
        let logo = Sprite::logo();
        assert_eq!((logo.ancho, logo.alto), (28, 23));
        assert_eq!(logo.px.len(), 28 * 23);
    }
}
