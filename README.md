# memtop

Tu **fastfetch + htop** en una sola pantalla de terminal, hecho en Rust con
[ratatui](https://ratatui.rs), la misma librería de TUI que usa llmfit.

```
╭ memtop ─────────────────────────────────────────────────────────── usuario@equipo ╮
│ 1 Resumen │ 2 CPU │ 3 RAM │ 4 Almacenamiento │ 5 GPU │ 6 Red │ 7 Procesos         │
╰───────────────────────────────────────────────────────────────────────────────────╯
╭ Persian ───────────────╮╭ CPU ────────────────────────────────────────────────────╮
│      ▄▀▀▄      ▄▄▀▀▀▄▄ ││Uso  ▕█████░░░░░░░░░░░░░░░░░░░▏  21.6%  86°C  4.10 GHz   │
│  ▄▀▀▄▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▄ ││▃▁▂▅▃▂▁▁▂▃▄▂▁                                            │
│   ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀  │╰─────────────────────────────────────────────────────────╯
╰─────────────── f: otra ╯╭ Memoria ────────────────────────────────────────────────╮
╭ Sistema ───────────────╮│RAM  ▕████████░░░░░░░░░░░░░░░░░░▏ 11.0 GiB / 31.0 GiB    │
│usuario@equipo          ││Swap ▕░░░░░░░░░░░░░░░░░░░░░░░░░░▏ 23.7 MiB / 8.0 GiB     │
│Equipo  Mi Laptop 15    │╰─────────────────────────────────────────────────────────╯
│SO      Fedora Linux 44 │  …
╰────────────────────────╯
```

- **Izquierda:** una foto (tuya o un Pokémon de `pokemon-colorscripts`) que se ajusta sola
  al tamaño del panel y, debajo, el resumen del sistema al estilo fastfetch.
- **Derecha:** pestañas con datos en vivo, que se actualizan cada segundo.

## Instalación

Necesitas **Rust 1.95 o superior** (lo exige `sysinfo`). Compruébalo con `rustc --version`
y actualiza con `rustup update`. Solo funciona en **Linux** (lee `/proc` y `/sys`).

```sh
git clone https://github.com/<tu-usuario>/memtop.git
cd memtop
cargo install --path .
memtop
```

`cargo install` deja el binario en `~/.cargo/bin/memtop`, que ya está en el `PATH` si
instalaste Rust con rustup. Para actualizarlo tras cambiar el código, repite
`cargo install --path .`.

Para probarlo sin instalar: `cargo run --release`.

### Dependencias opcionales

| Programa | Para qué | Sin él |
|---|---|---|
| `lspci` (paquete `pciutils`) | Nombre comercial de las GPU ("GeForce RTX 3050 Mobile") | Se muestra el ID del dispositivo |
| `nvidia-smi` | Uso, VRAM, temperatura y consumo de GPUs NVIDIA | Solo se ve el nombre y el estado de energía |
| `pokemon-colorscripts` | El Pokémon del panel de fotos | Solo se muestran tus imágenes |

## Uso

```sh
memtop                                # tus fotos + un Pokémon
memtop --foto ~/Imágenes/mi-pc.jpg    # una imagen concreta
memtop --foto ~/Imágenes/fondos/      # todas las imágenes de una carpeta
memtop --sin-pokemon --intervalo 2    # solo tus fotos, actualizar cada 2 s
```

| Opción | Qué hace |
|---|---|
| `--foto RUTA` | Imagen o carpeta para el panel izquierdo (por defecto `~/.config/memtop/fotos/`) |
| `--sin-pokemon` | No añadir el Pokémon de `pokemon-colorscripts` |
| `--halfblocks` | Dibujar las fotos con caracteres `▀` aunque la terminal soporte imágenes |
| `-i`, `--intervalo SEG` | Segundos entre actualizaciones (por defecto 1, mínimo 0.2) |
| `-h`, `--help` | Mostrar la ayuda |

## Pestañas

| # | Pestaña | Qué muestra |
|---|---|---|
| 1 | Resumen | CPU, RAM, swap, particiones, GPU, red, batería y procesos con más CPU |
| 2 | CPU | Modelo, núcleos e hilos, frecuencias, carga media, uso por hilo, gráfica de 120 s y sensores de temperatura |
| 3 | RAM | Usada / caché / swap, detalle de `/proc/meminfo`, gráfica y procesos con más memoria |
| 4 | Almacenamiento | Discos físicos (modelo, tipo, temperatura, lectura/escritura) y particiones montadas |
| 5 | GPU | Cada GPU: driver, estado de energía, uso, VRAM, frecuencia, temperatura y consumo |
| 6 | Red | Gráficas de bajada y subida, e interfaces con IP, MAC y totales |
| 7 | Procesos | Lista estilo htop: buscar, ordenar, ver solo los tuyos y terminar procesos |

## Panel de fotos

**Tus imágenes:** copia archivos **png, jpg/jpeg, webp, gif, bmp o tiff** a
`~/.config/memtop/fotos/` (o usa `--foto`) y cambia entre ellos con `f` / `F`.
Se escalan al panel manteniendo la proporción y quedan centrados.

- En **Kitty, WezTerm, Ghostty, Konsole o foot** se ven con resolución real (protocolos kitty / sixel).
- En las demás terminales (por ejemplo Ptyxis o GNOME Terminal) se dibujan con caracteres `▀`.

La detección es automática.

**Pokémon:** si tienes `pokemon-colorscripts`, se añade un Pokémon al azar al final de la lista
(cada vez que llegas a él sale otro). El dibujo se convierte en píxeles, se le recortan los
bordes vacíos y se reescala para llenar el panel: si cabe agrandado usa un factor entero
(×2, ×3…) para que se vea nítido, y si no, la escala exacta. Se reajusta al cambiar el
tamaño de la ventana.

## GPU NVIDIA en portátiles híbridos

Consultar `nvidia-smi` despierta la gráfica dedicada y gasta batería. Por eso memtop
**solo la consulta mientras estás en la pestaña GPU**, y nunca si está suspendida. En el resto
de pestañas solo lee su estado de energía desde `/sys`.

## Teclas

| Tecla | Acción |
|---|---|
| `← →` / `Tab` / `h l` | Cambiar de pestaña |
| `1` … `7` | Ir a una pestaña |
| `f` / `F` | Foto siguiente / anterior |
| `espacio` | Pausar / reanudar la actualización |
| `?` | Ayuda |
| `q` / `Esc` / `Ctrl+C` | Salir |

En la pestaña **Procesos**:

| Tecla | Acción |
|---|---|
| `↑ ↓` / `j k` | Moverse (`PgUp`/`PgDn` de 15 en 15, `g`/`G` al inicio o al final) |
| `/` | Buscar por nombre, comando, PID o usuario (`Enter` acepta, `Esc` borra) |
| `c` `m` `p` `n` `u` | Ordenar por CPU, memoria, PID, nombre o usuario (repetir la tecla invierte el orden) |
| `o` | Solo mis procesos |
| `x` | Quitar filtros |
| `t` / `F9` | Terminar el proceso (SIGTERM), con confirmación |
| `K` | Matar el proceso (SIGKILL), con confirmación |

Solo puedes terminar tus propios procesos; los de `root` u otros usuarios darán "sin permiso".

## De dónde salen los datos

Todo se lee localmente, sin privilegios de root:

- CPU, procesos, red, particiones y temperaturas: crate [`sysinfo`](https://crates.io/crates/sysinfo).
- Memoria: `/proc/meminfo`.
- Discos físicos, velocidades y temperatura de NVMe: `/sys/block`.
- GPU: `/sys/bus/pci/devices`, `lspci` y `nvidia-smi`.
- Batería: `/sys/class/power_supply`.
- Equipo y SO: `/sys/class/dmi/id` y `/etc/os-release`.

## Estructura del repositorio

```
memtop/
├── .gitignore
├── Cargo.toml
├── Cargo.lock
├── LICENSE
├── README.md
└── src/
    ├── main.rs      Opciones de línea de comandos y bucle principal
    ├── app.rs       Estado: pestañas, lista de procesos, teclas
    ├── ui.rs        Todo el dibujo (pestañas, barras, gráficas, tablas, ventanas)
    ├── sistema.rs   Lectura de CPU, RAM, discos, red, procesos, sensores y batería
    ├── gpu.rs       Detección de GPUs y consulta a nvidia-smi en un hilo aparte
    └── foto.rs      Panel de fotos: imágenes y Pokémon reescalable
```

## Problemas comunes

- **Las fotos se ven pixeladas:** tu terminal no soporta imágenes; usa Kitty, WezTerm, Ghostty, Konsole o foot.
- **Quedan restos de la foto anterior** (en terminales con sixel): cambia el tamaño de la ventana para forzar un redibujado, y avisa del caso para corregirlo.
- **No aparece el Pokémon:** comprueba que `pokemon-colorscripts -r` funciona en tu terminal y que no usaste `--sin-pokemon`.
- **La GPU NVIDIA sale sin datos:** entra en la pestaña 5. Si está "En reposo", es normal: no se la despierta a propósito.

## Licencia

[MIT](LICENSE)
