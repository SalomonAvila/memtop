//! Configuration file (`$XDG_CONFIG_HOME/memtop/config.toml`).
//!
//! Precedence: defaults < configuration file < command-line options. Every key is optional;
//! unknown ones are rejected so a typo is not silently ignored. The documented template is
//! `config.example.toml` (also available with `memtop --print-config`).

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
    i18n::{self, LanguageSetting},
};

/// Documented template; it matches the defaults (a test checks it).
pub const EXAMPLE: &str = include_str!("../config.example.toml");

/// Minimum interval between refreshes, in seconds.
pub const MIN_INTERVAL: f64 = 0.2;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Interface language.
    pub language: LanguageSetting,
    /// Seconds between refreshes.
    pub interval: f64,
    pub start_tab: Pestana,
    /// Show the memtop logo above the System panel. Without it, the System panel takes the
    /// whole left column.
    pub logo: bool,
    pub processes: ProcessSettings,
    pub colors: Theme,
}

/// `[processes]` section.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProcessSettings {
    pub sort: OrdenProc,
    pub mine_only: bool,
}

/// Interface colors (`[colors]` section).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    #[serde(deserialize_with = "color")]
    pub accent: Color,
    #[serde(deserialize_with = "color")]
    pub muted: Color,
    #[serde(deserialize_with = "color")]
    pub label: Color,
    #[serde(deserialize_with = "color")]
    pub selection: Color,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: LanguageSetting::Auto,
            interval: 1.0,
            start_tab: Pestana::Resumen,
            logo: false,
            processes: ProcessSettings::default(),
            colors: Theme::default(),
        }
    }
}

impl Default for ProcessSettings {
    fn default() -> Self {
        Self {
            sort: OrdenProc::Cpu,
            mine_only: false,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            muted: Color::DarkGray,
            label: Color::LightBlue,
            selection: Color::Rgb(0x23, 0x37, 0x55),
        }
    }
}

impl Config {
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs_f64(self.interval)
    }

    fn validate(self) -> Result<Self> {
        if !self.interval.is_finite() || self.interval < MIN_INTERVAL {
            bail!("{} {MIN_INTERVAL}", i18n::t().config_min_interval);
        }
        Ok(self)
    }
}

fn color<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
    let text = String::deserialize(d)?;
    text.parse().map_err(|_| {
        let t = i18n::t();
        de::Error::custom(format!(
            "{}: {text:?} ({})",
            t.config_invalid_color, t.config_color_formats
        ))
    })
}

/// memtop's configuration directory: `$XDG_CONFIG_HOME/memtop` or `~/.config/memtop`.
pub fn dir() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("memtop")
}

pub fn default_path() -> PathBuf {
    dir().join("config.toml")
}

/// Parses the contents of a configuration file.
pub fn parse(text: &str) -> Result<Config> {
    let config: Config = toml::from_str(text)?;
    config.validate()
}

/// Loads the configuration.
///
/// - With an explicit `path` (`--config`), the file must exist.
/// - Without one, the default path is used, and if it does not exist the defaults apply.
pub fn load(path: Option<&Path>) -> Result<Config> {
    let (path, required) = match path {
        Some(p) => (p.to_path_buf(), true),
        None => (default_path(), false),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == ErrorKind::NotFound && !required => return Ok(Config::default()),
        Err(e) => {
            return Err(e).with_context(|| {
                format!("{} {}", i18n::t().config_could_not_read, path.display())
            });
        }
    };
    parse(&text).with_context(|| format!("{} {}", i18n::t().config_error_in, path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_matches_the_defaults() {
        assert_eq!(parse(EXAMPLE).unwrap(), Config::default());
    }

    #[test]
    fn empty_file_uses_the_defaults() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn reads_every_key() {
        let c = parse(
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
        assert_eq!(c.language, LanguageSetting::En);
        assert_eq!(c.refresh_interval(), Duration::from_millis(2500));
        assert_eq!(c.start_tab, Pestana::Procesos);
        assert!(c.logo);
        assert_eq!(c.processes.sort, OrdenProc::Memoria);
        assert!(c.processes.mine_only);
        assert_eq!(c.colors.accent, Color::LightMagenta);
        assert_eq!(c.colors.muted, Color::Indexed(244));
        assert_eq!(c.colors.label, Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(c.colors.selection, Color::Blue);
    }

    #[test]
    fn partial_sections_keep_the_rest() {
        let c = parse("[colors]\naccent = \"green\"").unwrap();
        assert_eq!(c.colors.accent, Color::Green);
        assert_eq!(c.colors.muted, Theme::default().muted);
        assert_eq!(c.interval, 1.0);
    }

    #[test]
    fn rejects_the_old_spanish_keys() {
        for old in [
            "idioma = \"es\"",
            "intervalo = 2.0",
            "pestana_inicial = \"cpu\"",
            "pokemon = true",
            "[procesos]\norden = \"cpu\"",
            "[colores]\nacento = \"green\"",
        ] {
            assert!(parse(old).is_err(), "{old}");
        }
    }

    #[test]
    fn rejects_unknown_keys() {
        let e = parse("intervals = 2").unwrap_err().to_string();
        assert!(e.contains("intervals"), "{e}");
        assert!(parse("[processes]\nmine = true").is_err());
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(parse("interval = 0.1").is_err());
        assert!(parse("language = \"fr\"").is_err());
        assert!(parse("interval = \"1\"").is_err());
        assert!(parse("start_tab = \"resumen\"").is_err());
        assert!(parse("[processes]\nsort = \"memoria\"").is_err());
        let e = parse("[colors]\naccent = \"azulito\"")
            .unwrap_err()
            .to_string();
        assert!(e.contains("azulito"), "{e}");
    }

    #[test]
    fn errors_point_to_the_line() {
        let e = parse("interval = 1.0\nlanguage = \"es\"\nstart_tab = si\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains('3'), "{e}");
    }

    #[test]
    fn missing_explicit_path_is_an_error() {
        let missing = Path::new("/nonexistent/memtop/config.toml");
        assert!(load(Some(missing)).is_err());
    }
}
