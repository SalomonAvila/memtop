# memtop

Información del sistema y monitor de recursos en una sola pantalla de terminal, para Linux.

memtop combina dos cosas que normalmente se usan por separado: un resumen del equipo
(modelo, sistema operativo, CPU, GPU, memoria…) y un monitor en vivo de CPU, memoria,
discos, GPU, red y procesos. Está escrito en Rust con [ratatui](https://ratatui.rs) y se
distribuye como un único binario estático, sin dependencias del sistema.

## Contenido

- [Pantalla](#pantalla)
- [Instalación](#instalación)
- [Uso](#uso)
- [Configuración](#configuración)
- [Teclas](#teclas)
- [Datos y permisos](#datos-y-permisos)
- [Desarrollo](#desarrollo)
- [Licencia](#licencia)

## Pantalla

```
┌──────────────────────────────────────────────────────────────────────┐
│ Pestañas: 1 Resumen · 2 CPU · 3 RAM · 4 Almacenamiento · 5 GPU · …   │
├──────────────────────┬───────────────────────────────────────────────┤
│   Sistema            │                                               │
│   usuario@equipo,    │   Contenido de la pestaña activa              │
│   SO, kernel, CPU,   │   (barras, gráficas de historial, tablas)     │
│   GPU, memoria…      │                                               │
│                      │                                               │
│                      │                                               │
├──────────────────────┴───────────────────────────────────────────────┤
│ Atajos de teclado de la pestaña actual                               │
└──────────────────────────────────────────────────────────────────────┘
```

Con `logo = true` en la [configuración](#configuración), el logo de memtop aparece encima
del panel Sistema.

| # | Pestaña | Contenido |
|---|---|---|
| 1 | Resumen | Uso de CPU con historial, RAM y swap, particiones, estado de las GPU, red, batería y procesos con más CPU |
| 2 | CPU | Modelo, núcleos e hilos, frecuencias, carga media, uso por hilo, historial de 120 s y sensores de temperatura |
| 3 | RAM | Usada, caché y swap; detalle de `/proc/meminfo`; historial y procesos con más memoria |
| 4 | Almacenamiento | Discos físicos (modelo, tipo, tamaño, temperatura, velocidad de lectura y escritura) y particiones montadas |
| 5 | GPU | Por cada GPU: fabricante, driver, dirección PCI, estado de energía, uso, VRAM, frecuencia, temperatura y consumo |
| 6 | Red | Historial de descarga y subida, e interfaces con IP, MAC, velocidad actual y totales |
| 7 | Procesos | Lista estilo htop con búsqueda, orden por columnas, filtro de procesos propios y envío de señales |

Los datos se actualizan cada segundo (configurable con `--interval` o en el
[archivo de configuración](#configuración)).

## Instalación

### Binario precompilado (recomendado)

Cada [release](https://github.com/SalomonAvila/memtop/releases) incluye binarios **estáticos**
para `x86_64` y `aarch64`. Funcionan en cualquier distribución Linux (Ubuntu, Debian,
Fedora, Arch, Alpine…) y cualquier versión de `glibc`, porque no dependen de librerías del
sistema.

```sh
arch="$(uname -m)"   # x86_64 o aarch64
curl -LO "https://github.com/SalomonAvila/memtop/releases/latest/download/memtop-${arch}-unknown-linux-musl.tar.gz"
tar xzf "memtop-${arch}-unknown-linux-musl.tar.gz"
install -Dm755 "memtop-${arch}-unknown-linux-musl/memtop" ~/.local/bin/memtop
```

Asegúrate de que `~/.local/bin` está en tu `PATH`. Cada archivo publicado tiene al lado su
suma `.sha256` para verificar la descarga:

```sh
sha256sum -c "memtop-${arch}-unknown-linux-musl.tar.gz.sha256"
```

### Desde el código fuente

Requiere Rust **1.95 o superior**, instalado con [rustup](https://rustup.rs) (las versiones
de Rust que traen los gestores de paquetes de muchas distribuciones son más antiguas), y un
enlazador de C:

| Distribución | Enlazador |
|---|---|
| Ubuntu / Debian | `sudo apt install build-essential` |
| Fedora | `sudo dnf install gcc` |
| Arch | `sudo pacman -S base-devel` |

```sh
git clone https://github.com/SalomonAvila/memtop.git
cd memtop
cargo install --path . --locked
```

El binario queda en `~/.cargo/bin/memtop`. Todas las dependencias son crates de Rust que
Cargo descarga y compila; no hace falta ninguna librería de desarrollo del sistema.

Para generar el mismo binario estático que se publica:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --locked --target x86_64-unknown-linux-musl
# resultado: target/x86_64-unknown-linux-musl/release/memtop
```

### Programa opcional

memtop funciona sin él; solo amplía lo que muestra.

| Programa | Qué añade | De dónde sale |
|---|---|---|
| `nvidia-smi` | Uso, VRAM, temperatura, consumo y frecuencia de GPUs NVIDIA | Se instala junto con el driver propietario de NVIDIA |

## Uso

```sh
memtop                                  # abre en la pestaña Resumen
memtop --interval 2 --lang en           # actualiza cada 2 s, interfaz en inglés
```

| Opción | Descripción |
|---|---|
| `-i`, `--interval SEG` | Segundos entre actualizaciones. Por defecto `1`, mínimo `0.2` |
| `--lang IDIOMA` | Idioma de la interfaz: `auto` (según `LANG`), `es` o `en` |
| `-c`, `--config RUTA` | Usar este archivo de configuración en lugar del de por defecto |
| `--print-config` | Imprimir una configuración de ejemplo comentada y salir |
| `-V`, `--version` | Mostrar la versión |
| `-h`, `--help` | Mostrar la ayuda |

Códigos de salida: `0` correcto, `1` error en tiempo de ejecución, `2` opción inválida o
error en el archivo de configuración.

## Configuración

memtop lee `$XDG_CONFIG_HOME/memtop/config.toml` (normalmente
`~/.config/memtop/config.toml`) si existe. Si no existe, usa los valores por defecto. Para
crearlo con todas las opciones comentadas:

```sh
mkdir -p ~/.config/memtop
memtop --print-config > ~/.config/memtop/config.toml
```

La plantilla completa está en [`config.example.toml`](config.example.toml). Todas las claves
son opcionales:

| Clave | Valores | Por defecto |
|---|---|---|
| `language` | `"auto"` (según `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` o `LANG`), `"es"`, `"en"` | `"auto"` |
| `interval` | Segundos entre actualizaciones, `0.2` o más | `1.0` |
| `start_tab` | `"summary"`, `"cpu"`, `"ram"`, `"storage"`, `"gpu"`, `"network"`, `"processes"` | `"summary"` |
| `logo` | `true` muestra el logo de memtop encima del panel Sistema; `false` deja que Sistema ocupe toda la columna | `false` |
| `[processes] sort` | `"cpu"`, `"memory"`, `"pid"`, `"name"`, `"user"` | `"cpu"` |
| `[processes] mine_only` | `true` / `false` | `false` |
| `[colors] accent` | Color de títulos, pestaña activa y teclas | `"cyan"` |
| `[colors] muted` | Color de bordes y textos secundarios | `"darkgray"` |
| `[colors] label` | Color de los nombres de campo | `"lightblue"` |
| `[colors] selection` | Fondo de la fila seleccionada en Procesos | `"#233755"` |

Los colores aceptan nombres (`red`, `lightcyan`, `darkgray`…), hexadecimal (`"#rrggbb"`) o
un índice de la paleta de 256 colores (`"0"` a `"255"`).

Por ejemplo, para abrir siempre en Procesos, ordenado por memoria y con acento verde:

```toml
start_tab = "processes"

[processes]
sort = "memory"

[colors]
accent = "green"
```

**Prioridad:** valores por defecto < archivo < opciones de la línea de comandos. Por ejemplo,
`memtop -i 2` usa un intervalo de 2 s aunque el archivo diga otra cosa, pero respeta el resto
del archivo.

**Errores:** si el archivo tiene un error de sintaxis, una clave desconocida (por ejemplo una
errata como `intervals`) o un valor no válido, memtop no arranca y muestra la línea exacta:

```
memtop: error en ~/.config/memtop/config.toml: TOML parse error at line 1, column 1
  |
1 | intervals = 2
  | ^^^^^^^^^
unknown field `intervals`, expected one of `language`, `interval`, `start_tab`, `logo`, `processes`, `colors`
```

## Teclas

Pulsa `?` dentro de memtop para ver esta lista.

| Tecla | Acción |
|---|---|
| `←` `→`, `Tab`, `Shift+Tab`, `h` `l` | Pestaña anterior / siguiente |
| `1` … `7` | Ir a una pestaña |
| `espacio` | Pausar o reanudar la actualización |
| `?` | Ayuda |
| `q`, `Esc`, `Ctrl+C` | Salir |

En la pestaña **Procesos**:

| Tecla | Acción |
|---|---|
| `↑` `↓`, `j` `k` | Mover la selección |
| `PgUp` `PgDn` | Mover de 15 en 15 |
| `g` / `G`, `Inicio` / `Fin` | Ir al primero / al último |
| `/` | Buscar por nombre, línea de comandos, PID o usuario (`Enter` acepta, `Esc` borra) |
| `c` `m` `p` `n` `u` | Ordenar por CPU, memoria, PID, nombre o usuario. Repetir la tecla invierte el orden |
| `o` | Mostrar solo los procesos del usuario actual |
| `x` | Quitar búsqueda y filtros |
| `t`, `F9` | Enviar SIGTERM al proceso seleccionado (pide confirmación) |
| `K` | Enviar SIGKILL al proceso seleccionado (pide confirmación) |

Al buscar, filtrar u ordenar, la selección vuelve al primer proceso de la lista para que
nunca quede seleccionado uno que no se ve.

## Datos y permisos

memtop solo **lee** información local: no necesita root, no escribe archivos y no se conecta
a la red. La única acción que modifica el sistema es enviar una señal a un proceso, y siempre
pide confirmación; como usuario normal solo afecta a tus propios procesos.

| Dato | Fuente |
|---|---|
| CPU, procesos, red, particiones, temperaturas | crate [`sysinfo`](https://crates.io/crates/sysinfo) (`/proc`, `/sys`) |
| Memoria | `/proc/meminfo` |
| Discos físicos, velocidad y temperatura de NVMe | `/sys/block` |
| Modelo del equipo | `/sys/class/dmi/id` |
| Sistema operativo | `/etc/os-release` |
| Batería | `/sys/class/power_supply` |
| GPU (nombre, driver, estado de energía, datos AMD e Intel) | `/sys/bus/pci/devices` y la base PCI incluida en `data/pci-gpu.ids` |
| GPU NVIDIA (uso, VRAM, temperatura, consumo) | `nvidia-smi`, si está instalado |

**Portátiles con gráfica híbrida.** Consultar `nvidia-smi` despierta la gráfica dedicada y
aumenta el consumo. memtop solo la consulta mientras la pestaña GPU está visible y nunca
cuando está suspendida; en el resto de pestañas solo lee su estado de energía desde `/sys`.

**Contenedores.** Dentro de un contenedor los datos de `/proc` y `/sys` pueden ser los del
anfitrión (kernel, discos, sensores) o estar limitados (red, procesos).

## Desarrollo

```sh
cargo run --release          # ejecutar
cargo test                   # pruebas
cargo fmt                    # formato
cargo clippy --all-targets -- -D warnings
```

### Estructura

```
memtop/
├── .github/workflows/
│   ├── ci.yml                  Formato, clippy, pruebas y binario estático en cada push
│   └── release.yml             Publica binarios al subir una etiqueta vX.Y.Z
├── data/
│   └── pci-gpu.ids             Extracto de pci.ids con los fabricantes de GPU
├── scripts/
│   └── actualizar-pci-ids.sh   Regenera data/pci-gpu.ids
├── src/
│   ├── main.rs                 Opciones de línea de comandos y bucle principal
│   ├── config.rs               Archivo de configuración
│   ├── app.rs                  Estado de la interfaz y teclas
│   ├── ui.rs                   Dibujo de todas las pestañas
│   ├── sistema.rs              Lectura de CPU, memoria, discos, red, procesos, sensores y batería
│   ├── gpu.rs                  Detección de GPUs y consulta a nvidia-smi
│   ├── pci.rs                  Nombres de GPU a partir de la base PCI incluida
│   └── logo.rs                 Logo de memtop en pixel art
├── config.example.toml         Plantilla de configuración (también en memtop --print-config)
├── Cargo.toml
├── Cargo.lock
├── LICENSE
└── THIRD_PARTY_NOTICES.md      Licencias de los datos y crates de terceros
```

### Actualizar la base de GPUs

`data/pci-gpu.ids` se incluye en el binario al compilar. Para añadir modelos nuevos:

```sh
scripts/actualizar-pci-ids.sh                 # descarga la última versión de pci-ids.ucw.cz
scripts/actualizar-pci-ids.sh /usr/share/hwdata/pci.ids   # o usa una copia local
```

### Publicar una versión

1. Actualiza `version` en `Cargo.toml`.
2. Crea y sube la etiqueta:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. El flujo `release.yml` comprueba que la etiqueta coincide con `Cargo.toml`, crea la
   release y adjunta los binarios estáticos con sus sumas SHA-256.

## Licencia

Código bajo licencia [MIT](LICENSE).

`data/pci-gpu.ids` es un extracto de la base [pci.ids](https://pci-ids.ucw.cz), distribuida
bajo GPL-2.0-or-later o BSD-3-Clause a elección; memtop la usa bajo BSD-3-Clause. El texto
completo está en [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), que se incluye en cada
release.
