//! Panel de la izquierda.
//!
//! Muestra un Pokémon al azar si `pokemon-colorscripts` está instalado (opcional) y, si no,
//! el logo de memtop incluido en el binario. El pixel art se convierte en un [`Sprite`]
//! que se reescala al tamaño exacto del panel.

use std::{
    env,
    process::{Command, Stdio},
};

use ratatui::style::Color;

pub enum Contenido {
    /// Pixel art (un Pokémon o el logo) que se reescala al panel.
    Pixeles(Sprite),
    Error(String),
}

pub struct Fotos {
    /// `true` si se pidió Pokémon y `pokemon-colorscripts` está instalado.
    pokemon: bool,
    pub actual: Contenido,
    pub titulo: String,
}

fn hay_pokemon() -> bool {
    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|d| d.join("pokemon-colorscripts").is_file())
    })
}

impl Fotos {
    pub fn new(pokemon: bool) -> Self {
        let mut f = Self {
            pokemon: pokemon && hay_pokemon(),
            actual: Contenido::Pixeles(Sprite::logo()),
            titulo: " memtop ".into(),
        };
        f.siguiente();
        f
    }

    /// `true` si `f` cambia el dibujo (hay Pokémon disponibles).
    pub fn hay_pokemon(&self) -> bool {
        self.pokemon
    }

    /// Carga otro Pokémon al azar. Sin Pokémon no hace nada: se queda el logo.
    pub fn siguiente(&mut self) {
        if self.pokemon {
            let (nombre, contenido) = pokemon();
            self.titulo = format!(" {nombre} ");
            self.actual = contenido;
        }
    }
}

fn pokemon() -> (String, Contenido) {
    let out = match Command::new("pokemon-colorscripts")
        .arg("-r")
        .stdin(Stdio::null())
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            return (
                "Pokémon".into(),
                Contenido::Error(String::from_utf8_lossy(&o.stderr).into()),
            );
        }
        Err(e) => return ("Pokémon".into(), Contenido::Error(e.to_string())),
    };
    // La primera línea es el nombre (sin --no-title); el resto, el dibujo.
    let txt = String::from_utf8_lossy(&out);
    let (nombre, dibujo) = txt.split_once('\n').unwrap_or(("Pokémon", &txt));
    let nombre = nombre.trim();
    let nombre = if nombre.is_empty() || nombre.contains('\x1b') {
        "Pokémon"
    } else {
        nombre
    };
    let dibujo = dibujo.trim_end_matches('\n');
    let contenido = match Sprite::desde_ansi(dibujo) {
        Some(sp) => Contenido::Pixeles(sp),
        None => Contenido::Error(crate::i18n::t().foto_dibujo_vacio.into()),
    };
    (capitalizar(nombre), contenido)
}

fn capitalizar(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Una celda de la terminal: carácter, color de texto y color de fondo.
type Celda = (char, Option<Color>, Option<Color>);

/// Aplica los parámetros de una secuencia SGR (`ESC [ … m`) a los colores actuales.
/// Admite colores de 24 bits (`38;2;r;g;b`), de 256 (`38;5;n`) y los 16 básicos; el resto
/// de atributos (negrita, subrayado…) no afecta al dibujo y se ignora.
fn aplicar_sgr(params: &str, fg: &mut Option<Color>, bg: &mut Option<Color>) {
    let mut n = params.split(';').map(|p| p.parse::<u8>().unwrap_or(0));
    while let Some(p) = n.next() {
        match p {
            0 => (*fg, *bg) = (None, None),
            39 => *fg = None,
            49 => *bg = None,
            30..=37 => *fg = Some(Color::Indexed(p - 30)),
            40..=47 => *bg = Some(Color::Indexed(p - 40)),
            90..=97 => *fg = Some(Color::Indexed(p - 90 + 8)),
            100..=107 => *bg = Some(Color::Indexed(p - 100 + 8)),
            38 | 48 => {
                let color = match n.next() {
                    Some(2) => match (n.next(), n.next(), n.next()) {
                        (Some(r), Some(g), Some(b)) => Some(Color::Rgb(r, g, b)),
                        _ => None,
                    },
                    Some(5) => n.next().map(Color::Indexed),
                    _ => None,
                };
                if p == 38 { *fg = color } else { *bg = color }
            }
            _ => {}
        }
    }
}

/// Imagen de píxeles sacada de arte ANSI con medios bloques (`▀`/`▄`):
/// cada celda de la terminal son dos píxeles, arriba y abajo.
pub struct Sprite {
    pub ancho: usize,
    pub alto: usize,
    /// `None` = transparente.
    pub px: Vec<Option<Color>>,
}

impl Sprite {
    /// Convierte el texto con colores ANSI de `pokemon-colorscripts` en píxeles.
    fn desde_ansi(ansi: &str) -> Option<Self> {
        // Primero, cada fila como lista de celdas (carácter, color de texto, color de fondo).
        let mut filas: Vec<Vec<Celda>> = vec![Vec::new()];
        let (mut fg, mut bg) = (None, None);
        let mut chars = ansi.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\x1b' => {
                    // Secuencia CSI: ESC [ parámetros letra-final. Solo interesa la `m` (SGR).
                    if chars.next_if_eq(&'[').is_none() {
                        continue;
                    }
                    let mut params = String::new();
                    for c in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&c) {
                            if c == 'm' {
                                aplicar_sgr(&params, &mut fg, &mut bg);
                            }
                            break;
                        }
                        params.push(c);
                    }
                }
                '\n' => filas.push(Vec::new()),
                '\r' => {}
                c => filas.last_mut()?.push((c, fg, bg)),
            }
        }

        let alto = filas.len() * 2;
        let ancho = filas.iter().map(Vec::len).max()?;
        let mut px = vec![None; ancho * alto];
        for (fila, celdas) in filas.iter().enumerate() {
            for (x, &(ch, fg, bg)) in celdas.iter().enumerate() {
                let (arriba, abajo) = match ch {
                    '▀' => (fg, bg),
                    '▄' => (bg, fg),
                    '█' => (fg, fg),
                    _ => (bg, bg),
                };
                px[fila * 2 * ancho + x] = arriba;
                px[(fila * 2 + 1) * ancho + x] = abajo;
            }
        }
        Self { ancho, alto, px }.recortado()
    }

    /// Quita los bordes transparentes para aprovechar todo el panel.
    fn recortado(self) -> Option<Self> {
        let lleno = |x: usize, y: usize| self.px[y * self.ancho + x].is_some();
        let filas: Vec<usize> = (0..self.alto)
            .filter(|&y| (0..self.ancho).any(|x| lleno(x, y)))
            .collect();
        let cols: Vec<usize> = (0..self.ancho)
            .filter(|&x| (0..self.alto).any(|y| lleno(x, y)))
            .collect();
        let (y0, y1, x0, x1) = (
            *filas.first()?,
            *filas.last()?,
            *cols.first()?,
            *cols.last()?,
        );
        let (ancho, alto) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut px = Vec::with_capacity(ancho * alto);
        for y in y0..=y1 {
            px.extend_from_slice(&self.px[y * self.ancho + x0..=y * self.ancho + x1]);
        }
        Some(Self { ancho, alto, px })
    }

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
    fn convierte_medios_bloques_en_pixeles_y_recorta_bordes() {
        // "  ▀ " con fg rojo: un píxel arriba, el resto transparente.
        let sp = Sprite::desde_ansi("    \n  \x1b[38;2;255;0;0m▀\x1b[0m ").unwrap();
        assert_eq!((sp.ancho, sp.alto), (1, 1));
        assert_eq!(sp.pixel(0, 0), Some(rojo()));
    }

    #[test]
    fn lee_fondo_y_colores_de_256_y_basicos() {
        // ▀ con fg 256 (índice 196) y fondo básico azul (44): dos píxeles distintos.
        let sp = Sprite::desde_ansi("\x1b[38;5;196;44m▀\x1b[0m").unwrap();
        assert_eq!((sp.ancho, sp.alto), (1, 2));
        assert_eq!(sp.pixel(0, 0), Some(Color::Indexed(196)));
        assert_eq!(sp.pixel(0, 1), Some(Color::Indexed(4)));
        // ▄ invierte: el color de texto va abajo.
        let sp = Sprite::desde_ansi("\x1b[91;48;2;255;0;0m▄").unwrap();
        assert_eq!(sp.pixel(0, 0), Some(rojo()));
        assert_eq!(sp.pixel(0, 1), Some(Color::Indexed(9)));
    }

    #[test]
    fn el_reset_y_las_secuencias_desconocidas_no_pintan() {
        // Tras ESC[0m el bloque no tiene color; ESC[2K no es SGR y se ignora.
        let sp = Sprite::desde_ansi("\x1b[2K\x1b[38;2;255;0;0m█\x1b[0m█").unwrap();
        assert_eq!((sp.ancho, sp.alto), (1, 2));
        assert_eq!(sp.pixel(0, 1), Some(rojo()));
    }

    #[test]
    fn un_dibujo_vacio_no_produce_sprite() {
        assert!(Sprite::desde_ansi("   ").is_none());
        assert!(Sprite::desde_ansi("").is_none());
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
