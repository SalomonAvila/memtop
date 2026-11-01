//! Panel de la izquierda: tus fotos (de ~/.config/memtop/fotos o `--foto`)
//! y, si tienes `pokemon-colorscripts`, un Pokémon al azar como en tu fastfetch.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use ansi_to_tui::IntoText;
use image::ImageReader;
use ratatui::{style::Color, text::Text};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

const EXT_IMAGEN: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif"];
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
    Vacio,
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
            actual: Contenido::Vacio,
            titulo: " Foto ".into(),
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
            self.actual = Contenido::Vacio;
            self.titulo = " Foto ".into();
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
    let out = match Command::new("pokemon-colorscripts").arg("-r").output() {
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
}
