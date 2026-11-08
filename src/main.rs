//! memtop: información del sistema y monitor de recursos en una sola pantalla de terminal.
//!
//! Estructura:
//! - [`sistema`]: lectura de CPU, memoria, discos, red, procesos, sensores y batería.
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
mod sistema;
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

const AYUDA: &str = "\
memtop — información del sistema y monitor de recursos en la terminal

USO:
    memtop [OPCIONES]

OPCIONES:
    -i, --interval SEG      Segundos entre actualizaciones (mínimo 0.2)
    --lang IDIOMA           Idioma de la interfaz: auto (según LANG), es o en
    -c, --config RUTA       Usar este archivo de configuración
    --print-config          Imprimir una configuración de ejemplo comentada
    -V, --version           Mostrar la versión
    -h, --help              Mostrar esta ayuda

CONFIGURACIÓN:
    Se lee $XDG_CONFIG_HOME/memtop/config.toml (normalmente ~/.config/memtop/config.toml)
    si existe. Las opciones de arriba tienen prioridad sobre el archivo. Para crearlo:

        mkdir -p ~/.config/memtop && memtop --print-config > ~/.config/memtop/config.toml

Dentro de memtop pulsa ? para ver todas las teclas.
";

/// Opciones de la línea de comandos. `None` = no se indicó y manda la configuración.
#[derive(Debug, Default, PartialEq)]
struct Opciones {
    config: Option<PathBuf>,
    intervalo: Option<f64>,
    idioma: Option<i18n::Preferencia>,
}

impl Opciones {
    /// Aplica sobre la configuración las opciones indicadas en la línea de comandos.
    fn aplicar(self, mut c: Config) -> Config {
        if let Some(i) = self.intervalo {
            c.intervalo = i;
        }
        if let Some(i) = self.idioma {
            c.idioma = i;
        }
        c
    }
}

#[derive(Debug, PartialEq)]
enum Accion {
    Ejecutar(Opciones),
    Ayuda,
    Version,
    ConfigEjemplo,
}

fn leer_opciones(args: impl IntoIterator<Item = String>) -> Result<Accion, String> {
    let mut o = Opciones::default();
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => return Ok(Accion::Ayuda),
            "-V" | "--version" => return Ok(Accion::Version),
            "--print-config" => return Ok(Accion::ConfigEjemplo),
            "-c" | "--config" => {
                o.config = Some(PathBuf::from(
                    args.next().ok_or("--config necesita una ruta")?,
                ))
            }
            "--lang" => {
                let v = args
                    .next()
                    .ok_or("--lang necesita un valor: auto, es o en")?;
                let i = v
                    .parse()
                    .map_err(|_| format!("--lang no admite {v:?}: usa auto, es o en"))?;
                o.idioma = Some(i);
            }
            "-i" | "--interval" => {
                let s: f64 = args
                    .next()
                    .and_then(|v| v.replace(',', ".").parse().ok())
                    .filter(|v: &f64| v.is_finite() && *v >= config::INTERVALO_MINIMO)
                    .ok_or("--interval necesita un número de segundos (mínimo 0.2)")?;
                o.intervalo = Some(s);
            }
            otro => return Err(format!("opción desconocida: {otro}")),
        }
    }
    Ok(Accion::Ejecutar(o))
}

fn main() -> ExitCode {
    let opciones = match leer_opciones(env::args().skip(1)) {
        Ok(Accion::Ejecutar(o)) => o,
        Ok(Accion::Ayuda) => {
            print!("{AYUDA}");
            return ExitCode::SUCCESS;
        }
        Ok(Accion::Version) => {
            println!("memtop {VERSION}");
            return ExitCode::SUCCESS;
        }
        Ok(Accion::ConfigEjemplo) => {
            print!("{}", config::EJEMPLO);
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("memtop: {e}\nPrueba `memtop --help`.");
            return ExitCode::from(2);
        }
    };

    let config = match config::cargar(opciones.config.as_deref()) {
        Ok(c) => opciones.aplicar(c),
        Err(e) => {
            eprintln!("memtop: {e:#}");
            return ExitCode::from(2);
        }
    };

    if !stdout().is_terminal() {
        eprintln!("memtop: necesita ejecutarse en una terminal interactiva.");
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
    i18n::fijar(config.idioma.resolver());
    ui::fijar_tema(config.colores.clone());
    // Primera lectura antes de entrar a la interfaz: el uso de CPU necesita dos muestras.
    let monitor = sistema::Monitor::new()?;

    // `ratatui::init` activa el modo raw y la pantalla alternativa, e instala un hook
    // que restaura la terminal si el programa entra en pánico.
    let mut terminal = ratatui::init();
    let app = App::new(monitor, &config);
    let resultado = ejecutar(&mut terminal, app, config.intervalo());
    ratatui::restore();
    resultado
}

/// Bucle principal: dibuja, espera teclas hasta el siguiente tick y actualiza los datos.
fn ejecutar(terminal: &mut DefaultTerminal, mut app: App, intervalo: Duration) -> Result<()> {
    let mut siguiente_tick = Instant::now() + intervalo;

    while !app.salir {
        terminal.draw(|f| ui::dibujar(f, &mut app))?;

        let espera = siguiente_tick.saturating_duration_since(Instant::now());
        if event::poll(espera)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => app.tecla(k),
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
        leer_opciones(args.iter().map(|s| s.to_string()))
    }

    fn opciones(args: &[&str]) -> Opciones {
        match leer(args) {
            Ok(Accion::Ejecutar(o)) => o,
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
        assert_eq!(c.idioma, i18n::Preferencia::En);
        assert_eq!(c.intervalo(), Duration::from_millis(2500));
    }

    #[test]
    fn las_opciones_tienen_prioridad_sobre_el_archivo() {
        let archivo =
            config::interpretar("language = \"es\"\ninterval = 5.0\nstart_tab = \"network\"")
                .unwrap();
        let c = opciones(&["-i", "2", "--lang", "en"]).aplicar(archivo);
        assert_eq!(c.intervalo, 2.0);
        assert_eq!(c.idioma, i18n::Preferencia::En);
        // Lo que no se indicó en la línea de comandos sale del archivo.
        assert_eq!(c.pestana_inicial, app::Pestana::Red);
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
