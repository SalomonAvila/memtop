//! memtop: información del sistema y monitor de recursos en una sola pantalla de terminal.
//!
//! Estructura:
//! - [`sistema`]: lectura de CPU, memoria, discos, red, procesos, sensores y batería.
//! - [`gpu`] y [`pci`]: detección de GPUs y sus nombres comerciales.
//! - [`foto`]: panel de imágenes y pixel art.
//! - [`app`]: estado de la interfaz y manejo de teclas.
//! - [`ui`]: dibujo de cada pestaña con ratatui.

mod app;
mod foto;
mod gpu;
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
use ratatui_image::picker::{Picker, ProtocolType};

use app::App;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const AYUDA: &str = "\
memtop — información del sistema y monitor de recursos en la terminal

USO:
    memtop [OPCIONES]

OPCIONES:
    --foto RUTA             Imagen o carpeta de imágenes para el panel izquierdo
                            (por defecto: $XDG_CONFIG_HOME/memtop/fotos o ~/.config/memtop/fotos)
    --sin-pokemon           No mostrar Pokémon aunque pokemon-colorscripts esté instalado
    --halfblocks            Dibujar las imágenes con caracteres ▀ aunque la terminal admita gráficos
    -i, --intervalo SEG     Segundos entre actualizaciones (por defecto 1, mínimo 0.2)
    -V, --version           Mostrar la versión
    -h, --help              Mostrar esta ayuda

Dentro de memtop pulsa ? para ver todas las teclas.
";

#[derive(Debug, PartialEq)]
struct Opciones {
    foto: Option<PathBuf>,
    pokemon: bool,
    halfblocks: bool,
    intervalo: Duration,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            foto: None,
            pokemon: true,
            halfblocks: false,
            intervalo: Duration::from_secs(1),
        }
    }
}

#[derive(Debug, PartialEq)]
enum Accion {
    Ejecutar(Opciones),
    Ayuda,
    Version,
}

fn leer_opciones(args: impl IntoIterator<Item = String>) -> Result<Accion, String> {
    let mut o = Opciones::default();
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => return Ok(Accion::Ayuda),
            "-V" | "--version" => return Ok(Accion::Version),
            "--foto" => {
                o.foto = Some(PathBuf::from(
                    args.next().ok_or("--foto necesita una ruta")?,
                ))
            }
            "--sin-pokemon" => o.pokemon = false,
            "--halfblocks" => o.halfblocks = true,
            "-i" | "--intervalo" => {
                let s: f64 = args
                    .next()
                    .and_then(|v| v.replace(',', ".").parse().ok())
                    .filter(|v: &f64| v.is_finite() && *v >= 0.2)
                    .ok_or("--intervalo necesita un número de segundos (mínimo 0.2)")?;
                o.intervalo = Duration::from_secs_f64(s);
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
        Err(e) => {
            eprintln!("memtop: {e}\nPrueba `memtop --help`.");
            return ExitCode::from(2);
        }
    };

    if !stdout().is_terminal() {
        eprintln!("memtop: necesita ejecutarse en una terminal interactiva.");
        return ExitCode::FAILURE;
    }

    match iniciar(opciones) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("memtop: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn iniciar(opciones: Opciones) -> Result<()> {
    // Primera lectura antes de entrar a la interfaz: el uso de CPU necesita dos muestras.
    let monitor = sistema::Monitor::new()?;

    // `ratatui::init` activa el modo raw y la pantalla alternativa, e instala un hook
    // que restaura la terminal si el programa entra en pánico.
    let mut terminal = ratatui::init();
    // La consulta del protocolo de imágenes necesita el modo raw ya activo.
    let picker = if opciones.halfblocks {
        Picker::halfblocks()
    } else {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    };
    let fotos = foto::Fotos::new(picker, opciones.foto, opciones.pokemon);
    let resultado = ejecutar(&mut terminal, App::new(monitor, fotos), opciones.intervalo);
    ratatui::restore();
    resultado
}

/// Bucle principal: dibuja, espera teclas hasta el siguiente tick y actualiza los datos.
fn ejecutar(terminal: &mut DefaultTerminal, mut app: App, intervalo: Duration) -> Result<()> {
    // Sixel e iTerm2 pintan píxeles encima de las celdas: al cambiar de imagen o de
    // tamaño hay que limpiar la pantalla o quedan restos de la anterior.
    let limpiar = matches!(
        app.fotos.picker.protocol_type(),
        ProtocolType::Sixel | ProtocolType::Iterm2
    );
    let mut version_foto = app.fotos.version;
    let mut siguiente_tick = Instant::now() + intervalo;

    while !app.salir {
        if limpiar && app.fotos.version != version_foto {
            terminal.clear()?;
            version_foto = app.fotos.version;
        }
        terminal.draw(|f| ui::dibujar(f, &mut app))?;

        let espera = siguiente_tick.saturating_duration_since(Instant::now());
        if event::poll(espera)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => app.tecla(k),
                Event::Resize(..) if limpiar => terminal.clear()?,
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

    #[test]
    fn sin_argumentos_usa_los_valores_por_defecto() {
        assert_eq!(leer(&[]), Ok(Accion::Ejecutar(Opciones::default())));
    }

    #[test]
    fn interpreta_todas_las_opciones() {
        let Ok(Accion::Ejecutar(o)) = leer(&[
            "--foto",
            "/tmp/a.png",
            "--sin-pokemon",
            "--halfblocks",
            "-i",
            "2,5",
        ]) else {
            panic!("debería ejecutar");
        };
        assert_eq!(o.foto, Some(PathBuf::from("/tmp/a.png")));
        assert!(!o.pokemon);
        assert!(o.halfblocks);
        assert_eq!(o.intervalo, Duration::from_millis(2500));
    }

    #[test]
    fn ayuda_y_version() {
        assert_eq!(leer(&["--help"]), Ok(Accion::Ayuda));
        assert_eq!(leer(&["-V"]), Ok(Accion::Version));
    }

    #[test]
    fn rechaza_valores_invalidos() {
        assert!(leer(&["--foto"]).is_err());
        assert!(leer(&["-i", "0.1"]).is_err());
        assert!(leer(&["-i", "nan"]).is_err());
        assert!(leer(&["--desconocida"]).is_err());
    }
}
