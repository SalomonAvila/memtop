//! Archivo de configuración (`$XDG_CONFIG_HOME/memtop/config.toml`).
//!
//! Orden de prioridad: valores por defecto < archivo de configuración < opciones de la
//! línea de comandos. Todas las claves son opcionales; las desconocidas se rechazan para
//! que una errata no se ignore en silencio. La plantilla documentada está en
//! `config.example.toml` (también disponible con `memtop --print-config`).

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
    i18n::{self, Preferencia},
};

/// Plantilla documentada; coincide con los valores por defecto (lo comprueba una prueba).
pub const EJEMPLO: &str = include_str!("../config.example.toml");

/// Intervalo mínimo entre actualizaciones, en segundos.
pub const INTERVALO_MINIMO: f64 = 0.2;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Idioma de la interfaz.
    #[serde(rename = "language")]
    pub idioma: Preferencia,
    /// Segundos entre actualizaciones.
    #[serde(rename = "interval")]
    pub intervalo: f64,
    #[serde(rename = "start_tab")]
    pub pestana_inicial: Pestana,
    /// Mostrar el logo de memtop encima del panel Sistema. Sin él, Sistema ocupa toda la
    /// columna izquierda.
    pub logo: bool,
    #[serde(rename = "processes")]
    pub procesos: Procesos,
    #[serde(rename = "colors")]
    pub colores: Tema,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Procesos {
    #[serde(rename = "sort")]
    pub orden: OrdenProc,
    #[serde(rename = "mine_only")]
    pub solo_mios: bool,
}

/// Colores de la interfaz.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tema {
    #[serde(rename = "accent", deserialize_with = "color")]
    pub acento: Color,
    #[serde(rename = "muted", deserialize_with = "color")]
    pub tenue: Color,
    #[serde(rename = "label", deserialize_with = "color")]
    pub etiqueta: Color,
    #[serde(rename = "selection", deserialize_with = "color")]
    pub seleccion: Color,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idioma: Preferencia::Auto,
            intervalo: 1.0,
            pestana_inicial: Pestana::Resumen,
            logo: false,
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
            bail!("{} {INTERVALO_MINIMO}", i18n::t().config_intervalo_minimo);
        }
        Ok(self)
    }
}

fn color<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
    let texto = String::deserialize(d)?;
    texto.parse().map_err(|_| {
        let t = i18n::t();
        de::Error::custom(format!(
            "{}: {texto:?} ({})",
            t.config_color_invalido, t.config_color_formatos
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
        Err(e) => {
            return Err(e).with_context(|| {
                format!("{} {}", i18n::t().config_no_se_pudo_leer, ruta.display())
            });
        }
    };
    interpretar(&texto).with_context(|| format!("{} {}", i18n::t().config_error_en, ruta.display()))
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
            language = "en"
            interval = 2.5
            start_tab = "processes"
            logo = true
            [processes]
            sort = "memory"
            mine_only = true
            [colors]
            accent = "light-magenta"
            muted = "244"
            label = "#ff8800"
            selection = "blue"
            "##,
        )
        .unwrap();
        assert_eq!(c.idioma, Preferencia::En);
        assert_eq!(c.intervalo(), Duration::from_millis(2500));
        assert_eq!(c.pestana_inicial, Pestana::Procesos);
        assert!(c.logo);
        assert_eq!(c.procesos.orden, OrdenProc::Memoria);
        assert!(c.procesos.solo_mios);
        assert_eq!(c.colores.acento, Color::LightMagenta);
        assert_eq!(c.colores.tenue, Color::Indexed(244));
        assert_eq!(c.colores.etiqueta, Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(c.colores.seleccion, Color::Blue);
    }

    #[test]
    fn las_secciones_parciales_conservan_el_resto() {
        let c = interpretar("[colors]\naccent = \"green\"").unwrap();
        assert_eq!(c.colores.acento, Color::Green);
        assert_eq!(c.colores.tenue, Tema::default().tenue);
        assert_eq!(c.intervalo, 1.0);
    }

    #[test]
    fn rechaza_las_claves_antiguas_en_espanol() {
        for antigua in [
            "idioma = \"es\"",
            "intervalo = 2.0",
            "pestana_inicial = \"cpu\"",
            "pokemon = true",
            "[procesos]\norden = \"cpu\"",
            "[colores]\nacento = \"green\"",
        ] {
            assert!(interpretar(antigua).is_err(), "{antigua}");
        }
    }

    #[test]
    fn rechaza_claves_desconocidas() {
        let e = interpretar("intervals = 2").unwrap_err().to_string();
        assert!(e.contains("intervals"), "{e}");
        assert!(interpretar("[processes]\nmine = true").is_err());
    }

    #[test]
    fn rechaza_valores_invalidos() {
        assert!(interpretar("interval = 0.1").is_err());
        assert!(interpretar("language = \"fr\"").is_err());
        assert!(interpretar("interval = \"1\"").is_err());
        assert!(interpretar("start_tab = \"resumen\"").is_err());
        assert!(interpretar("[processes]\nsort = \"memoria\"").is_err());
        let e = interpretar("[colors]\naccent = \"azulito\"")
            .unwrap_err()
            .to_string();
        assert!(e.contains("azulito"), "{e}");
    }

    #[test]
    fn los_errores_indican_la_linea() {
        let e = interpretar("interval = 1.0\nlanguage = \"es\"\nstart_tab = si\n")
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
