//! memtop: información del sistema y monitor de recursos en una sola pantalla de terminal.
//!
//! Estructura:
//! - [`system`]: lectura de CPU, memoria, discos, red, procesos, sensores y batería.
//! - [`gpu`] y [`pci`]: detección de GPUs y sus nombres comerciales.
//! - [`logo`]: logo de memtop en pixel art para el panel izquierdo.
//! - [`config`]: archivo de configuración.
//! - [`i18n`]: idioma de la interfaz y textos traducidos.
//! - [`app`]: estado de la interfaz y manejo de teclas.
//! - [`ui`]: dibujo de cada pestaña con ratatui.

mod app;
mod config;
mod gpu;
mod i18n;
mod logo;
mod pci;
mod system;
mod ui;

use std::{
    env,
    io::{IsTerminal, stdout},
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use app::App;
use config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Opciones de la línea de comandos. `None` = no se indicó y manda la configuración.
#[derive(Debug, Default, PartialEq)]
struct Opciones {
    config: Option<PathBuf>,
    intervalo: Option<f64>,
    idioma: Option<i18n::LanguageSetting>,
}

impl Opciones {
    /// Aplica sobre la configuración las opciones indicadas en la línea de comandos.
    fn aplicar(self, mut c: Config) -> Config {
        if let Some(i) = self.intervalo {
            c.interval = i;
        }
        if let Some(i) = self.idioma {
            c.language = i;
        }
        c
    }
}

/// Qué hacer después de leer las opciones.
#[derive(Debug, PartialEq)]
enum Accion {
    Ejecutar,
    Ayuda,
    Version,
    ConfigEjemplo,
}

/// Lee las opciones. `--help`, `--version` y `--print-config` se resuelven al final para
/// que un `--lang` en cualquier posición cambie el idioma de la ayuda.
fn leer_opciones(args: impl IntoIterator<Item = String>) -> Result<(Accion, Opciones), String> {
    let t = i18n::t();
    let mut o = Opciones::default();
    let mut accion = None;
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => accion = accion.or(Some(Accion::Ayuda)),
            "-V" | "--version" => accion = accion.or(Some(Accion::Version)),
            "--print-config" => accion = accion.or(Some(Accion::ConfigEjemplo)),
            "-c" | "--config" => {
                o.config = Some(PathBuf::from(args.next().ok_or(t.cli_missing_path)?))
            }
            "--lang" => {
                let v = args.next().ok_or(t.cli_missing_lang)?;
                let i = v
                    .parse()
                    .map_err(|_| format!("{}: {v:?}", t.cli_invalid_lang))?;
                o.idioma = Some(i);
            }
            "-i" | "--interval" => {
                let s: f64 = args
                    .next()
                    .and_then(|v| v.replace(',', ".").parse().ok())
                    .filter(|v: &f64| v.is_finite() && *v >= config::MIN_INTERVAL)
                    .ok_or(t.cli_invalid_interval)?;
                o.intervalo = Some(s);
            }
            otro => return Err(format!("{}: {otro}", t.cli_unknown_option)),
        }
    }
    Ok((accion.unwrap_or(Accion::Ejecutar), o))
}

fn main() -> ExitCode {
    // Hasta leer las opciones, los mensajes salen en el idioma del entorno.
    let (accion, opciones) = match leer_opciones(env::args().skip(1)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("memtop: {e}\n{}", i18n::t().cli_try_help);
            return ExitCode::from(2);
        }
    };
    // Desde aquí, la ayuda y los errores del archivo de configuración respetan `--lang`.
    if let Some(i) = opciones.idioma {
        i18n::set(i.resolve());
    }
    match accion {
        Accion::Ejecutar => {}
        Accion::Ayuda => {
            print!("{}", i18n::t().cli_help);
            return ExitCode::SUCCESS;
        }
        Accion::Version => {
            println!("memtop {VERSION}");
            return ExitCode::SUCCESS;
        }
        Accion::ConfigEjemplo => {
            print!("{}", config::EXAMPLE);
            return ExitCode::SUCCESS;
        }
    }

    let config = match config::load(opciones.config.as_deref()) {
        Ok(c) => opciones.aplicar(c),
        Err(e) => {
            eprintln!("memtop: {e:#}");
            return ExitCode::from(2);
        }
    };

    // Idioma definitivo: archivo de configuración y, por encima, `--lang`.
    i18n::set(config.language.resolve());

    if !stdout().is_terminal() {
        eprintln!("memtop: {}", i18n::t().cli_no_terminal);
        return ExitCode::FAILURE;
    }

    match iniciar(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("memtop: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn iniciar(config: Config) -> Result<()> {
    ui::set_theme(config.colors.clone());
    // Primera lectura antes de entrar a la interfaz: el uso de CPU necesita dos muestras.
    let monitor = system::Monitor::new()?;

    // `ratatui::init` activa el modo raw y la pantalla alternativa, e instala un hook
    // que restaura la terminal si el programa entra en pánico.
    let mut terminal = ratatui::init();
    let app = App::new(monitor, &config);
    let resultado = ejecutar(&mut terminal, app, config.refresh_interval());
    ratatui::restore();
    resultado
}

/// Bucle principal: dibuja, espera teclas hasta el siguiente tick y actualiza los datos.
fn ejecutar(terminal: &mut DefaultTerminal, mut app: App, intervalo: Duration) -> Result<()> {
    let mut siguiente_tick = Instant::now() + intervalo;

    while !app.quit {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let espera = siguiente_tick.saturating_duration_since(Instant::now());
        if event::poll(espera)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => app.key(k),
                _ => {}
            }
        }
        if Instant::now() >= siguiente_tick {
            app.tick();
            siguiente_tick = Instant::now() + intervalo;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leer(args: &[&str]) -> Result<Accion, String> {
        leer_opciones(args.iter().map(|s| s.to_string())).map(|(a, _)| a)
    }

    fn opciones(args: &[&str]) -> Opciones {
        match leer_opciones(args.iter().map(|s| s.to_string())) {
            Ok((Accion::Ejecutar, o)) => o,
            otro => panic!("debería ejecutar: {otro:?}"),
        }
    }

    #[test]
    fn sin_argumentos_no_cambia_la_configuracion() {
        assert_eq!(opciones(&[]), Opciones::default());
        assert_eq!(opciones(&[]).aplicar(Config::default()), Config::default());
    }

    #[test]
    fn interpreta_todas_las_opciones() {
        let o = opciones(&["--lang", "en", "-i", "2,5", "-c", "/tmp/c.toml"]);
        assert_eq!(o.config, Some(PathBuf::from("/tmp/c.toml")));
        let c = o.aplicar(Config::default());
        assert_eq!(c.language, i18n::LanguageSetting::En);
        assert_eq!(c.refresh_interval(), Duration::from_millis(2500));
    }

    #[test]
    fn las_opciones_tienen_prioridad_sobre_el_archivo() {
        let archivo =
            config::parse("language = \"es\"\ninterval = 5.0\nstart_tab = \"network\"").unwrap();
        let c = opciones(&["-i", "2", "--lang", "en"]).aplicar(archivo);
        assert_eq!(c.interval, 2.0);
        assert_eq!(c.language, i18n::LanguageSetting::En);
        // Lo que no se indicó en la línea de comandos sale del archivo.
        assert_eq!(c.start_tab, app::Tab::Network);
    }

    #[test]
    fn rechaza_las_opciones_antiguas_en_espanol() {
        for antigua in [
            "--intervalo",
            "--idioma",
            "--config-ejemplo",
            "--sin-pokemon",
        ] {
            assert!(leer(&[antigua, "1"]).is_err(), "{antigua}");
        }
    }

    #[test]
    fn acciones_que_no_abren_la_interfaz() {
        assert_eq!(leer(&["--help"]), Ok(Accion::Ayuda));
        assert_eq!(leer(&["-V"]), Ok(Accion::Version));
        assert_eq!(leer(&["--print-config"]), Ok(Accion::ConfigEjemplo));
        // --lang se conserva aunque vaya después de --help; la primera acción manda.
        let (accion, o) = leer_opciones(["--help", "--lang", "es"].map(String::from)).unwrap();
        assert_eq!(accion, Accion::Ayuda);
        assert_eq!(o.idioma, Some(i18n::LanguageSetting::Es));
        assert_eq!(leer(&["-V", "--help"]), Ok(Accion::Version));
        // Un error en otra opción se informa aunque haya --help.
        assert!(leer(&["--help", "--lang", "fr"]).is_err());
    }

    #[test]
    fn rechaza_valores_invalidos() {
        assert!(leer(&["--config"]).is_err());
        assert!(leer(&["--lang"]).is_err());
        assert!(leer(&["--lang", "fr"]).is_err());
        assert!(leer(&["-i", "0.1"]).is_err());
        assert!(leer(&["-i", "nan"]).is_err());
        assert!(leer(&["--desconocida"]).is_err());
    }
}
