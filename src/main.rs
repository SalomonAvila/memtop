mod app;
mod foto;
mod gpu;
mod sistema;
mod ui;

use std::{
    env,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui_image::picker::{Picker, ProtocolType};

use app::App;

const AYUDA: &str = "\
memtop — tu fastfetch + htop en una sola pantalla

USO:
    memtop [opciones]

OPCIONES:
    --foto RUTA     Imagen o carpeta de imágenes para el panel izquierdo
                    (por defecto: ~/.config/memtop/fotos)
    --sin-pokemon   No añadir un Pokémon de pokemon-colorscripts
    --halfblocks    Dibujar las fotos con caracteres ▀ en vez de sixel/kitty
    -i, --intervalo SEG   Segundos entre actualizaciones (por defecto 1)
    -h, --help      Mostrar esta ayuda
";

struct Opciones {
    foto: Option<PathBuf>,
    pokemon: bool,
    halfblocks: bool,
    intervalo: Duration,
}

fn leer_opciones() -> Result<Option<Opciones>, String> {
    let mut o = Opciones {
        foto: None,
        pokemon: true,
        halfblocks: false,
        intervalo: Duration::from_secs(1),
    };
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => return Ok(None),
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
                    .filter(|v: &f64| *v >= 0.2)
                    .ok_or("--intervalo necesita un número de segundos (mínimo 0.2)")?;
                o.intervalo = Duration::from_secs_f64(s);
            }
            otro => return Err(format!("opción desconocida: {otro}")),
        }
    }
    Ok(Some(o))
}

fn main() -> ExitCode {
    let opciones = match leer_opciones() {
        Ok(Some(o)) => o,
        Ok(None) => {
            print!("{AYUDA}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{AYUDA}");
            return ExitCode::FAILURE;
        }
    };

    // Primera lectura antes de entrar a la TUI (el uso de CPU necesita dos muestras).
    let monitor = sistema::Monitor::new();

    let mut terminal = ratatui::init();
    // Preguntar a la terminal qué protocolo de imágenes soporta (tras entrar en modo raw).
    let picker = if opciones.halfblocks {
        Picker::halfblocks()
    } else {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    };
    let fotos = foto::Fotos::new(picker, opciones.foto, opciones.pokemon);
    let resultado = ejecutar(&mut terminal, App::new(monitor, fotos), opciones.intervalo);
    ratatui::restore();

    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn ejecutar(terminal: &mut DefaultTerminal, mut app: App, intervalo: Duration) -> Result<()> {
    // Sixel/iTerm2 pintan píxeles encima de las celdas: al cambiar de foto hay
    // que limpiar la pantalla o quedan restos de la anterior.
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
