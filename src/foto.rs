//! Panel de la izquierda.
//!
//! Fuentes, en este orden:
//! 1. Imágenes de `--foto RUTA` o, si no se indica, de `~/.config/memtop/fotos/`.
//! 2. Un Pokémon al azar si `pokemon-colorscripts` está instalado (opcional).
//! 3. Si no hay ninguna de las dos, el logo de memtop incluido en el binario.
//!
//! Las imágenes se muestran con el mejor protocolo que admita la terminal (kitty, sixel,
//! iTerm2 o medios bloques). El pixel art (Pokémon y logo) se convierte en un [`Sprite`]
//! que se reescala al tamaño exacto del panel.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ansi_to_tui::IntoText;
use image::ImageReader;
use ratatui::{style::Color, text::Text};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

/// Extensiones de imagen que se cargan desde la carpeta de fotos.
pub const EXT_IMAGEN: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif"];

/// Lado máximo (px) al que se reduce cada imagen al cargarla, para que redimensionar
/// en cada cambio de tamaño de la ventana sea rápido aunque el original sea enorme.
const MAX_PX: u32 = 1400;

enum Fuente {
    Imagen(PathBuf),
    Pokemon,
}

pub enum Contenido {
    Imagen(Box<StatefulProtocol>),
    /// Pixel art (p. ej. un Pokémon) que se reescala al panel.
    Pixeles(Sprite),
    Error(String),
}

pub struct Fotos {
    fuentes: Vec<Fuente>,
    idx: usize,
    pub actual: Contenido,
    pub titulo: String,
    pub picker: Picker,
    /// Se incrementa cada vez que cambia la foto (para limpiar restos de sixel).
    pub version: u64,
}

pub fn carpeta_por_defecto() -> PathBuf {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("memtop/fotos")
}

/// La carpeta de fotos para mostrarla al usuario, con `~` en lugar de `$HOME`.
pub fn carpeta_mostrada() -> String {
    let ruta = carpeta_por_defecto().display().to_string();
    match env::var("HOME") {
        Ok(home) if !home.is_empty() && ruta.starts_with(&home) => {
            format!("~{}", &ruta[home.len()..])
        }
        _ => ruta,
    }
}

fn es_imagen(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| EXT_IMAGEN.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

fn hay_pokemon() -> bool {
    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|d| d.join("pokemon-colorscripts").is_file())
    })
}

impl Fotos {
    pub fn new(picker: Picker, ruta: Option<PathBuf>, pokemon: bool) -> Self {
        let ruta = ruta.unwrap_or_else(carpeta_por_defecto);
        let mut imagenes: Vec<PathBuf> = if ruta.is_file() {
            vec![ruta]
        } else {
            fs::read_dir(&ruta)
                .map(|d| {
                    d.flatten()
                        .map(|e| e.path())
                        .filter(|p| p.is_file() && es_imagen(p))
                        .collect()
                })
                .unwrap_or_default()
        };
        imagenes.sort();

        let mut fuentes: Vec<Fuente> = imagenes.into_iter().map(Fuente::Imagen).collect();
        if pokemon && hay_pokemon() {
            fuentes.push(Fuente::Pokemon);
        }
        let mut f = Self {
            fuentes,
            idx: 0,
            actual: Contenido::Pixeles(Sprite::logo()),
            titulo: " memtop ".into(),
            picker,
            version: 0,
        };
        f.cargar();
        f
    }

    pub fn cantidad(&self) -> usize {
        self.fuentes.len()
    }

    pub fn siguiente(&mut self) {
        if !self.fuentes.is_empty() {
            self.idx = (self.idx + 1) % self.fuentes.len();
            self.cargar();
        }
    }

    pub fn anterior(&mut self) {
        if !self.fuentes.is_empty() {
            self.idx = (self.idx + self.fuentes.len() - 1) % self.fuentes.len();
            self.cargar();
        }
    }

    fn cargar(&mut self) {
        self.version += 1;
        let n = self.fuentes.len();
        let pos = if n > 1 {
            format!(" {}/{n}", self.idx + 1)
        } else {
            String::new()
        };
        let Some(fuente) = self.fuentes.get(self.idx) else {
            self.actual = Contenido::Pixeles(Sprite::logo());
            self.titulo = " memtop ".into();
            return;
        };
        match fuente {
            Fuente::Imagen(p) => {
                let nombre = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.titulo = format!(" {nombre}{pos} ");
                self.actual = match abrir_imagen(p) {
                    Ok(img) => Contenido::Imagen(Box::new(self.picker.new_resize_protocol(img))),
                    Err(e) => Contenido::Error(format!("No se pudo abrir {}:\n{e}", p.display())),
                };
            }
            Fuente::Pokemon => {
                let (nombre, contenido) = pokemon();
                self.titulo = format!(" {nombre}{pos} ");
                self.actual = contenido;
            }
        }
    }
}

fn abrir_imagen(p: &Path) -> Result<image::DynamicImage, String> {
    let img = ImageReader::open(p)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    Ok(if img.width() > MAX_PX || img.height() > MAX_PX {
        img.thumbnail(MAX_PX, MAX_PX)
    } else {
        img
    })
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
    let contenido = match dibujo.as_bytes().into_text() {
        Ok(t) => match Sprite::desde_texto(&t) {
            Some(sp) => Contenido::Pixeles(sp),
            None => Contenido::Error("dibujo vacío".into()),
        },
        Err(e) => Contenido::Error(e.to_string()),
    };
    (capitalizar(nombre), contenido)
}

fn capitalizar(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
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
    fn desde_texto(t: &Text) -> Option<Self> {
        let alto = t.lines.len() * 2;
        let ancho = t.lines.iter().map(|l| l.width()).max()?;
        let mut px = vec![None; ancho * alto];
        for (fila, linea) in t.lines.iter().enumerate() {
            let mut x = 0;
            for span in &linea.spans {
                let st = linea.style.patch(span.style);
                let (fg, bg) = (
                    st.fg.filter(|c| *c != Color::Reset),
                    st.bg.filter(|c| *c != Color::Reset),
                );
                for ch in span.content.chars() {
                    let (arriba, abajo) = match ch {
                        '▀' => (fg, bg),
                        '▄' => (bg, fg),
                        '█' => (fg, fg),
                        _ => (bg, bg),
                    };
                    if x < ancho {
                        px[fila * 2 * ancho + x] = arriba;
                        px[(fila * 2 + 1) * ancho + x] = abajo;
                    }
                    x += 1;
                }
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
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};

    fn rojo() -> Color {
        Color::Rgb(255, 0, 0)
    }

    #[test]
    fn convierte_medios_bloques_en_pixeles_y_recorta_bordes() {
        // "  ▀ " con fg rojo: un píxel arriba, el resto transparente.
        let t = Text::from(vec![
            Line::from("    "),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("▀", Style::new().fg(rojo())),
                Span::raw(" "),
            ]),
        ]);
        let sp = Sprite::desde_texto(&t).unwrap();
        assert_eq!((sp.ancho, sp.alto), (1, 1));
        assert_eq!(sp.pixel(0, 0), Some(rojo()));
    }

    #[test]
    fn un_dibujo_vacio_no_produce_sprite() {
        assert!(Sprite::desde_texto(&Text::from("   ")).is_none());
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

    #[test]
    fn reconoce_extensiones_de_imagen() {
        assert!(es_imagen(Path::new("foto.JPG")));
        assert!(es_imagen(Path::new("a/b.webp")));
        assert!(!es_imagen(Path::new("notas.txt")));
        assert!(!es_imagen(Path::new("sin_extension")));
    }
}
