//! Archivo de configuración (`$XDG_CONFIG_HOME/memtop/config.toml`).
//!
//! Orden de prioridad: valores por defecto < archivo de configuración < opciones de la
//! línea de comandos. Todas las claves son opcionales; las desconocidas se rechazan para
//! que una errata no se ignore en silencio. La plantilla documentada está en
//! `config.ejemplo.toml` (también disponible con `memtop --config-ejemplo`).

use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use ratatui::style::Color;
use serde::{Deserialize, Deserializer, de};

use crate::{
    app::{OrdenProc, Pestana},
    i18n::Preferencia,
};

/// Plantilla documentada; coincide con los valores por defecto (lo comprueba una prueba).
pub const EJEMPLO: &str = include_str!("../config.ejemplo.toml");

/// Intervalo mínimo entre actualizaciones, en segundos.
pub const INTERVALO_MINIMO: f64 = 0.2;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Idioma de la interfaz.
    pub idioma: Preferencia,
    /// Segundos entre actualizaciones.
    pub intervalo: f64,
    pub pestana_inicial: Pestana,
    /// Mostrar un Pokémon al azar si `pokemon-colorscripts` está instalado.
    pub pokemon: bool,
    pub procesos: Procesos,
    pub colores: Tema,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Procesos {
    pub orden: OrdenProc,
    pub solo_mios: bool,
}

/// Colores de la interfaz.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tema {
    #[serde(deserialize_with = "color")]
    pub acento: Color,
    #[serde(deserialize_with = "color")]
    pub tenue: Color,
    #[serde(deserialize_with = "color")]
    pub etiqueta: Color,
    #[serde(deserialize_with = "color")]
    pub seleccion: Color,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idioma: Preferencia::Auto,
            intervalo: 1.0,
            pestana_inicial: Pestana::Resumen,
            pokemon: true,
            procesos: Procesos::default(),
            colores: Tema::default(),
        }
    }
}

impl Default for Procesos {
    fn default() -> Self {
        Self {
            orden: OrdenProc::Cpu,
            solo_mios: false,
        }
    }
}

impl Default for Tema {
    fn default() -> Self {
        Self {
            acento: Color::Cyan,
            tenue: Color::DarkGray,
            etiqueta: Color::LightBlue,
            seleccion: Color::Rgb(0x23, 0x37, 0x55),
        }
    }
}

impl Config {
    pub fn intervalo(&self) -> Duration {
        Duration::from_secs_f64(self.intervalo)
    }

    fn validar(self) -> Result<Self> {
        if !self.intervalo.is_finite() || self.intervalo < INTERVALO_MINIMO {
            bail!("intervalo debe ser un número de segundos mayor o igual que {INTERVALO_MINIMO}");
        }
        Ok(self)
    }
}

fn color<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
    let texto = String::deserialize(d)?;
    texto.parse().map_err(|_| {
        de::Error::custom(format!(
            "color no válido: {texto:?} (usa un nombre como \"cyan\", \"#rrggbb\" o un número de 0 a 255)"
        ))
    })
}

/// Carpeta de configuración de memtop: `$XDG_CONFIG_HOME/memtop` o `~/.config/memtop`.
pub fn carpeta() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("memtop")
}

pub fn ruta_por_defecto() -> PathBuf {
    carpeta().join("config.toml")
}

/// Interpreta el contenido de un archivo de configuración.
pub fn interpretar(texto: &str) -> Result<Config> {
    let config: Config = toml::from_str(texto)?;
    config.validar()
}

/// Carga la configuración.
///
/// - Con `ruta` explícita (`--config`), el archivo debe existir.
/// - Sin ella, se usa la ruta por defecto, y si no existe se usan los valores por defecto.
pub fn cargar(ruta: Option<&Path>) -> Result<Config> {
    let (ruta, obligatoria) = match ruta {
        Some(r) => (r.to_path_buf(), true),
        None => (ruta_por_defecto(), false),
    };
    let texto = match fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) if e.kind() == ErrorKind::NotFound && !obligatoria => return Ok(Config::default()),
        Err(e) => return Err(e).with_context(|| format!("no se pudo leer {}", ruta.display())),
    };
    interpretar(&texto).with_context(|| format!("error en {}", ruta.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_plantilla_coincide_con_los_valores_por_defecto() {
        assert_eq!(interpretar(EJEMPLO).unwrap(), Config::default());
    }

    #[test]
    fn un_archivo_vacio_usa_los_valores_por_defecto() {
        assert_eq!(interpretar("").unwrap(), Config::default());
    }

    #[test]
    fn lee_todas_las_claves() {
        let c = interpretar(
            r##"
            idioma = "en"
            intervalo = 2.5
            pestana_inicial = "procesos"
            pokemon = false
            [procesos]
            orden = "memoria"
            solo_mios = true
            [colores]
            acento = "light-magenta"
            tenue = "244"
            etiqueta = "#ff8800"
            seleccion = "blue"
            "##,
        )
        .unwrap();
        assert_eq!(c.idioma, Preferencia::En);
        assert_eq!(c.intervalo(), Duration::from_millis(2500));
        assert_eq!(c.pestana_inicial, Pestana::Procesos);
        assert!(!c.pokemon);
        assert_eq!(c.procesos.orden, OrdenProc::Memoria);
        assert!(c.procesos.solo_mios);
        assert_eq!(c.colores.acento, Color::LightMagenta);
        assert_eq!(c.colores.tenue, Color::Indexed(244));
        assert_eq!(c.colores.etiqueta, Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(c.colores.seleccion, Color::Blue);
    }

    #[test]
    fn las_secciones_parciales_conservan_el_resto() {
        let c = interpretar("[colores]\nacento = \"green\"").unwrap();
        assert_eq!(c.colores.acento, Color::Green);
        assert_eq!(c.colores.tenue, Tema::default().tenue);
        assert_eq!(c.intervalo, 1.0);
    }

    #[test]
    fn rechaza_claves_desconocidas() {
        let e = interpretar("intervalos = 2").unwrap_err().to_string();
        assert!(e.contains("intervalos"), "{e}");
        assert!(interpretar("pokemn = false").is_err());
        assert!(interpretar("[procesos]\nsolo_mio = true").is_err());
    }

    #[test]
    fn rechaza_valores_invalidos() {
        assert!(interpretar("intervalo = 0.1").is_err());
        assert!(interpretar("idioma = \"fr\"").is_err());
        assert!(interpretar("intervalo = \"1\"").is_err());
        assert!(interpretar("pestana_inicial = \"inicio\"").is_err());
        assert!(interpretar("[procesos]\norden = \"tamaño\"").is_err());
        let e = interpretar("[colores]\nacento = \"azulito\"")
            .unwrap_err()
            .to_string();
        assert!(e.contains("azulito"), "{e}");
    }

    #[test]
    fn los_errores_indican_la_linea() {
        let e = interpretar("intervalo = 1.0\nidioma = \"es\"\npokemon = si\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains('3'), "{e}");
    }

    #[test]
    fn una_ruta_explicita_inexistente_es_un_error() {
        let inexistente = Path::new("/nonexistent/memtop/config.toml");
        assert!(cargar(Some(inexistente)).is_err());
    }
}
