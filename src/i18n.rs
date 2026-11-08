//! Idioma de la interfaz y textos traducidos.
//!
//! El idioma se elige al arrancar con [`fijar`], a partir de la opción
//! `--lang`, la clave `language` de la configuración o, si ambas son `auto`, del entorno
//! ([`detectar`]). Los textos se obtienen con [`t`], que devuelve la tabla del idioma activo.
//!
//! Para añadir un texto: crea el campo en [`Textos`] y dale valor en [`ES`] y [`EN`].
//! El compilador obliga a traducirlo en los dos idiomas.

use std::{
    env,
    str::FromStr,
    sync::atomic::{AtomicU8, Ordering},
};

use serde::Deserialize;

/// Idiomas disponibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    Es,
    En,
}

/// Idioma pedido por el usuario en la configuración (`language`) o en `--lang`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preferencia {
    /// Detectar del entorno (`LANG`…).
    #[default]
    Auto,
    Es,
    En,
}

impl Preferencia {
    pub fn resolver(self) -> Idioma {
        match self {
            Preferencia::Auto => detectar(),
            Preferencia::Es => Idioma::Es,
            Preferencia::En => Idioma::En,
        }
    }
}

impl FromStr for Preferencia {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Preferencia::Auto),
            "es" => Ok(Preferencia::Es),
            "en" => Ok(Preferencia::En),
            _ => Err(()),
        }
    }
}

/// Idioma activo: `0` = aún sin fijar (se detecta del entorno), `1` = español, `2` = inglés.
///
/// Se puede fijar más de una vez porque el idioma se va precisando al arrancar: primero el
/// del entorno (errores de las opciones), luego el de `--lang` (errores del archivo de
/// configuración) y por último el resultado de combinar archivo y opciones.
static IDIOMA: AtomicU8 = AtomicU8::new(0);

/// Fija el idioma de los textos a partir de este momento.
pub fn fijar(idioma: Idioma) {
    let v = match idioma {
        Idioma::Es => 1,
        Idioma::En => 2,
    };
    IDIOMA.store(v, Ordering::Relaxed);
}

/// Idioma activo: el fijado con [`fijar`] o, si no se fijó, el del entorno.
pub fn actual() -> Idioma {
    match IDIOMA.load(Ordering::Relaxed) {
        1 => Idioma::Es,
        2 => Idioma::En,
        _ => {
            let idioma = detectar();
            fijar(idioma);
            idioma
        }
    }
}

/// Textos del idioma activo.
pub fn t() -> &'static Textos {
    match actual() {
        Idioma::Es => &ES,
        Idioma::En => &EN,
    }
}

/// Idioma según las variables de entorno, en el orden de prioridad de gettext:
/// `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` y `LANG`. Español si el idioma es `es`;
/// inglés en cualquier otro caso (incluido el locale `C`/`POSIX`).
pub fn detectar() -> Idioma {
    detectar_con(|clave| env::var(clave).ok())
}

fn detectar_con(var: impl Fn(&str) -> Option<String>) -> Idioma {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| var(k).filter(|v| !v.is_empty()));
    // gettext ignora LANGUAGE si el locale es C/POSIX.
    let es_c = locale
        .as_deref()
        .is_none_or(|l| l == "C" || l == "POSIX" || l.starts_with("C."));
    let preferido = var("LANGUAGE")
        .filter(|v| !v.is_empty() && !es_c)
        .and_then(|v| v.split(':').next().map(str::to_string));
    match preferido.or(locale) {
        Some(v) if codigo(&v) == "es" => Idioma::Es,
        _ => Idioma::En,
    }
}

/// `"es_CO.UTF-8@euro"` → `"es"`.
fn codigo(locale: &str) -> String {
    locale
        .split(['_', '.', '@', '-'])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

/// Todos los textos de la interfaz en un idioma.
pub struct Textos {
    // Pestañas
    pub pestana_resumen: &'static str,
    pub pestana_cpu: &'static str,
    pub pestana_ram: &'static str,
    pub pestana_almacenamiento: &'static str,
    pub pestana_gpu: &'static str,
    pub pestana_red: &'static str,
    pub pestana_procesos: &'static str,

    // Barra superior
    pub pausa: &'static str,

    // Panel de sistema
    pub titulo_sistema: &'static str,
    pub equipo: &'static str,
    pub so: &'static str,
    pub kernel: &'static str,
    pub encendido: &'static str,
    pub escritorio: &'static str,
    pub shell: &'static str,
    pub terminal: &'static str,
    pub cpu: &'static str,
    pub gpu: &'static str,
    pub memoria: &'static str,
    pub disco_raiz: &'static str,
    pub bateria: &'static str,

    // Estado de la batería
    pub bateria_cargando: &'static str,
    pub bateria_descargando: &'static str,
    pub bateria_completa: &'static str,
    pub bateria_sin_cargar: &'static str,
    pub bateria_desconocido: &'static str,

    // Pestaña Resumen
    pub uso: &'static str,
    pub swap: &'static str,
    pub titulo_gpu_red_energia: &'static str,
    pub titulo_top_cpu: &'static str,
    pub red: &'static str,
    pub red_sin_conexion: &'static str,
    pub gpu_activa: &'static str,
    pub gpu_en_reposo: &'static str,

    // Columnas de procesos
    pub col_pid: &'static str,
    pub col_nombre: &'static str,
    pub col_cpu: &'static str,
    pub col_mem: &'static str,
    pub col_res: &'static str,
    pub col_usuario: &'static str,

    // Pestaña CPU
    pub titulo_procesador: &'static str,
    pub modelo: &'static str,
    pub nucleos: &'static str,
    pub fisicos: &'static str,
    pub hilos: &'static str,
    pub frecuencia: &'static str,
    pub carga_media: &'static str,
    pub arquitectura: &'static str,
    pub temperatura: &'static str,
    pub uso_total: &'static str,
    pub ultimos: &'static str,
    pub titulo_por_hilo: &'static str,
    pub titulo_sensores: &'static str,

    // Pestaña RAM
    pub usada: &'static str,
    pub cache: &'static str,
    pub total: &'static str,
    pub disponible: &'static str,
    pub libre: &'static str,
    pub buffers: &'static str,
    pub compartida: &'static str,
    pub swap_total: &'static str,
    pub swap_usada: &'static str,
    pub titulo_detalle: &'static str,
    pub titulo_top_memoria: &'static str,

    // Pestaña Almacenamiento
    pub titulo_discos: &'static str,
    pub col_disco: &'static str,
    pub col_tipo: &'static str,
    pub col_tamano: &'static str,
    pub col_temp: &'static str,
    pub col_lectura: &'static str,
    pub col_escritura: &'static str,
    pub titulo_particiones: &'static str,
    pub usados: &'static str,
    pub libres_de: &'static str,

    // Pestaña GPU
    pub gpu_ninguna: &'static str,
    pub fabricante: &'static str,
    pub driver: &'static str,
    pub pci: &'static str,
    pub estado: &'static str,
    pub gpu_estado_activa: &'static str,
    pub gpu_estado_reposo: &'static str,
    pub vram: &'static str,
    pub consumo: &'static str,
    pub gpu_consultando: &'static str,
    pub gpu_no_despertar: &'static str,

    // Pestaña Red
    pub descarga: &'static str,
    pub subida: &'static str,
    pub titulo_interfaces: &'static str,
    pub col_interfaz: &'static str,
    pub col_ahora: &'static str,
    pub col_total: &'static str,

    // Pestaña Procesos
    pub col_tiempo_cpu: &'static str,
    pub col_comando: &'static str,
    pub solo_mios: &'static str,

    // Estados de proceso
    pub proc_ejecutando: &'static str,
    pub proc_durmiendo: &'static str,
    pub proc_inactivo: &'static str,
    pub proc_espera_disco: &'static str,
    pub proc_detenido: &'static str,
    pub proc_zombi: &'static str,
    pub proc_otro: &'static str,

    // Pie de atajos
    pub pie_aceptar: &'static str,
    pub pie_borrar: &'static str,
    pub pie_mover: &'static str,
    pub pie_buscar: &'static str,
    pub pie_ordenar: &'static str,
    pub pie_terminar: &'static str,
    pub pie_matar: &'static str,
    pub pie_pestana: &'static str,
    pub pie_ir_a: &'static str,
    pub pie_pausa: &'static str,
    pub pie_ayuda: &'static str,
    pub pie_salir: &'static str,
    pub tecla_espacio: &'static str,

    // Ventana de confirmación
    pub confirmar_titulo: &'static str,
    pub confirmar_enviar: &'static str,
    pub confirmar_a: &'static str,
    pub confirmar_forzar: &'static str,
    pub confirmar_opciones: &'static str,

    // Avisos al enviar señales
    pub aviso_enviado_a: &'static str,
    pub aviso_no_se_pudo_terminar: &'static str,
    pub senal_no_existe: &'static str,
    pub senal_sin_permiso: &'static str,
    pub senal_no_soportada: &'static str,

    // Ventana de ayuda
    pub ayuda_titulo: &'static str,
    pub ayuda_general: &'static str,
    pub ayuda_cambiar_pestana: &'static str,
    pub ayuda_ir_pestana: &'static str,
    pub ayuda_pausa: &'static str,
    pub ayuda_salir: &'static str,
    pub ayuda_mover: &'static str,
    pub ayuda_buscar: &'static str,
    pub ayuda_ordenar: &'static str,
    pub ayuda_invertir: &'static str,
    pub ayuda_solo_mios: &'static str,
    pub ayuda_quitar_filtros: &'static str,
    pub ayuda_terminar: &'static str,
    pub ayuda_matar: &'static str,

    // Línea de comandos (`--help` y errores antes de abrir la interfaz)
    pub cli_ayuda: &'static str,
    pub cli_prueba_ayuda: &'static str,
    pub cli_falta_ruta: &'static str,
    pub cli_falta_idioma: &'static str,
    pub cli_idioma_invalido: &'static str,
    pub cli_intervalo_invalido: &'static str,
    pub cli_opcion_desconocida: &'static str,
    pub cli_sin_terminal: &'static str,

    // Errores del archivo de configuración
    pub config_intervalo_minimo: &'static str,
    pub config_color_invalido: &'static str,
    pub config_color_formatos: &'static str,
    pub config_no_se_pudo_leer: &'static str,
    pub config_error_en: &'static str,

    // Errores al leer el sistema
    pub sistema_sin_info: &'static str,
    pub sistema_sin_discos: &'static str,
    pub sistema_sin_red: &'static str,
    pub sistema_sin_sensores: &'static str,

    // GPU sin nombre en la base PCI
    pub fabricante_desconocido: &'static str,
}

pub const ES: Textos = Textos {
    pestana_resumen: "Resumen",
    pestana_cpu: "CPU",
    pestana_ram: "RAM",
    pestana_almacenamiento: "Almacenamiento",
    pestana_gpu: "GPU",
    pestana_red: "Red",
    pestana_procesos: "Procesos",

    // Barra superior
    pausa: "PAUSA",

    // Panel de sistema
    titulo_sistema: "Sistema",
    equipo: "Equipo",
    so: "SO",
    kernel: "Kernel",
    encendido: "Encendido",
    escritorio: "Escritorio",
    shell: "Shell",
    terminal: "Terminal",
    cpu: "CPU",
    gpu: "GPU",
    memoria: "Memoria",
    disco_raiz: "Disco (/)",
    bateria: "Batería",

    // Estado de la batería
    bateria_cargando: "Cargando",
    bateria_descargando: "Descargando",
    bateria_completa: "Completa",
    bateria_sin_cargar: "Conectada, sin cargar",
    bateria_desconocido: "Desconocido",

    // Pestaña Resumen
    uso: "Uso",
    swap: "Swap",
    titulo_gpu_red_energia: "GPU · Red · Energía",
    titulo_top_cpu: "Procesos con más CPU",
    red: "Red",
    red_sin_conexion: "sin conexión",
    gpu_activa: "activa",
    gpu_en_reposo: "en reposo",

    // Columnas de procesos
    col_pid: "PID",
    col_nombre: "Nombre",
    col_cpu: "CPU%",
    col_mem: "MEM%",
    col_res: "RES",
    col_usuario: "Usuario",

    // Pestaña CPU
    titulo_procesador: "Procesador",
    modelo: "Modelo",
    nucleos: "Núcleos",
    fisicos: "físicos",
    hilos: "hilos",
    frecuencia: "Frecuencia",
    carga_media: "Carga media",
    arquitectura: "Arquitectura",
    temperatura: "Temperatura",
    uso_total: "Uso total",
    ultimos: "últimos",
    titulo_por_hilo: "Por hilo",
    titulo_sensores: "Sensores",

    // Pestaña RAM
    usada: "Usada",
    cache: "Caché",
    total: "Total",
    disponible: "Disponible",
    libre: "Libre",
    buffers: "Buffers",
    compartida: "Compartida",
    swap_total: "Swap total",
    swap_usada: "Swap usada",
    titulo_detalle: "Detalle",
    titulo_top_memoria: "Procesos con más memoria",

    // Pestaña Almacenamiento
    titulo_discos: "Discos físicos",
    col_disco: "Disco",
    col_tipo: "Tipo",
    col_tamano: "Tamaño",
    col_temp: "Temp",
    col_lectura: "Lectura",
    col_escritura: "Escritura",
    titulo_particiones: "Particiones montadas",
    usados: "usados",
    libres_de: "libres de",

    // Pestaña GPU
    gpu_ninguna: "No se encontró ninguna GPU",
    fabricante: "Fabricante",
    driver: "Driver",
    pci: "PCI",
    estado: "Estado",
    gpu_estado_activa: "Activa",
    gpu_estado_reposo: "En reposo (ahorrando energía)",
    vram: "VRAM",
    consumo: "Consumo",
    gpu_consultando: "Consultando nvidia-smi…",
    gpu_no_despertar: "No se le consulta nada para no despertarla.",

    // Pestaña Red
    descarga: "Descarga",
    subida: "Subida",
    titulo_interfaces: "Interfaces",
    col_interfaz: "Interfaz",
    col_ahora: "ahora",
    col_total: "total",

    // Pestaña Procesos
    col_tiempo_cpu: "T. CPU",
    col_comando: "Comando",
    solo_mios: "solo míos",

    // Estados de proceso
    proc_ejecutando: "Activo",
    proc_durmiendo: "Durmiendo",
    proc_inactivo: "Inactivo",
    proc_espera_disco: "E/S disco",
    proc_detenido: "Detenido",
    proc_zombi: "Zombi",
    proc_otro: "Otro",

    // Pie de atajos
    pie_aceptar: "aceptar",
    pie_borrar: "borrar",
    pie_mover: "mover",
    pie_buscar: "buscar",
    pie_ordenar: "ordenar",
    pie_terminar: "terminar",
    pie_matar: "matar",
    pie_pestana: "pestaña",
    pie_ir_a: "ir a",
    pie_pausa: "pausa",
    pie_ayuda: "ayuda",
    pie_salir: "salir",
    tecla_espacio: "espacio",

    // Ventana de confirmación
    confirmar_titulo: "Terminar proceso",
    confirmar_enviar: "¿Enviar",
    confirmar_a: "a",
    confirmar_forzar: "(forzar)",
    confirmar_opciones: "s / Enter: sí      cualquier otra tecla: no",

    // Avisos al enviar señales
    aviso_enviado_a: "enviado a",
    aviso_no_se_pudo_terminar: "No se pudo terminar",
    senal_no_existe: "el proceso ya no existe",
    senal_sin_permiso: "sin permiso (¿es de otro usuario?)",
    senal_no_soportada: "señal no soportada",

    // Ventana de ayuda
    ayuda_titulo: "Ayuda — cualquier tecla para cerrar",
    ayuda_general: "General",
    ayuda_cambiar_pestana: "Cambiar de pestaña",
    ayuda_ir_pestana: "Ir a una pestaña",
    ayuda_pausa: "Pausar / reanudar la actualización",
    ayuda_salir: "Salir",
    ayuda_mover: "Moverse (PgUp/PgDn, g/G)",
    ayuda_buscar: "Buscar por nombre, comando, PID o usuario",
    ayuda_ordenar: "Ordenar por CPU, memoria, PID, nombre, usuario",
    ayuda_invertir: "(repetir la tecla invierte el orden)",
    ayuda_solo_mios: "Solo mis procesos",
    ayuda_quitar_filtros: "Quitar filtros",
    ayuda_terminar: "Terminar (SIGTERM), pide confirmación",
    ayuda_matar: "Matar (SIGKILL), pide confirmación",

    // Línea de comandos
    cli_ayuda: "\
memtop — información del sistema y monitor de recursos en la terminal

USO:
    memtop [OPCIONES]

OPCIONES:
    -i, --interval SEG      Segundos entre actualizaciones (mínimo 0.2)
    --lang IDIOMA           Idioma de la interfaz: auto (según LANG), en o es
    -c, --config RUTA       Usar este archivo de configuración
    --print-config          Imprimir una configuración de ejemplo comentada
    -V, --version           Mostrar la versión
    -h, --help              Mostrar esta ayuda

CONFIGURACIÓN:
    Se lee $XDG_CONFIG_HOME/memtop/config.toml (normalmente ~/.config/memtop/config.toml)
    si existe. Las opciones de arriba tienen prioridad sobre el archivo. Para crearlo:

        mkdir -p ~/.config/memtop && memtop --print-config > ~/.config/memtop/config.toml

Dentro de memtop pulsa ? para ver todas las teclas.
",
    cli_prueba_ayuda: "Prueba `memtop --help`.",
    cli_falta_ruta: "--config necesita una ruta",
    cli_falta_idioma: "--lang necesita un valor: auto, en o es",
    cli_idioma_invalido: "--lang solo admite auto, en o es",
    cli_intervalo_invalido: "--interval necesita un número de segundos (mínimo 0.2)",
    cli_opcion_desconocida: "opción desconocida",
    cli_sin_terminal: "necesita ejecutarse en una terminal interactiva.",

    // Errores del archivo de configuración
    config_intervalo_minimo: "interval debe ser un número de segundos mayor o igual que",
    config_color_invalido: "color no válido",
    config_color_formatos: "usa un nombre como \"cyan\", \"#rrggbb\" o un número de 0 a 255",
    config_no_se_pudo_leer: "no se pudo leer",
    config_error_en: "error en",

    // Errores al leer el sistema
    sistema_sin_info: "no se pudo leer la información del sistema",
    sistema_sin_discos: "no se pudieron leer los discos",
    sistema_sin_red: "no se pudo leer la red",
    sistema_sin_sensores: "no se pudieron leer los sensores",

    // GPU sin nombre en la base PCI
    fabricante_desconocido: "Desconocido",
};

pub const EN: Textos = Textos {
    pestana_resumen: "Overview",
    pestana_cpu: "CPU",
    pestana_ram: "Memory",
    pestana_almacenamiento: "Storage",
    pestana_gpu: "GPU",
    pestana_red: "Network",
    pestana_procesos: "Processes",

    // Barra superior
    pausa: "PAUSED",

    // Panel de sistema
    titulo_sistema: "System",
    equipo: "Host",
    so: "OS",
    kernel: "Kernel",
    encendido: "Uptime",
    escritorio: "Desktop",
    shell: "Shell",
    terminal: "Terminal",
    cpu: "CPU",
    gpu: "GPU",
    memoria: "Memory",
    disco_raiz: "Disk (/)",
    bateria: "Battery",

    // Estado de la batería
    bateria_cargando: "Charging",
    bateria_descargando: "Discharging",
    bateria_completa: "Full",
    bateria_sin_cargar: "Plugged in, not charging",
    bateria_desconocido: "Unknown",

    // Pestaña Resumen
    uso: "Usage",
    swap: "Swap",
    titulo_gpu_red_energia: "GPU · Network · Power",
    titulo_top_cpu: "Top processes by CPU",
    red: "Network",
    red_sin_conexion: "not connected",
    gpu_activa: "active",
    gpu_en_reposo: "suspended",

    // Columnas de procesos
    col_pid: "PID",
    col_nombre: "Name",
    col_cpu: "CPU%",
    col_mem: "MEM%",
    col_res: "RES",
    col_usuario: "User",

    // Pestaña CPU
    titulo_procesador: "Processor",
    modelo: "Model",
    nucleos: "Cores",
    fisicos: "physical",
    hilos: "threads",
    frecuencia: "Frequency",
    carga_media: "Load average",
    arquitectura: "Architecture",
    temperatura: "Temperature",
    uso_total: "Total usage",
    ultimos: "last",
    titulo_por_hilo: "Per thread",
    titulo_sensores: "Sensors",

    // Pestaña RAM
    usada: "Used",
    cache: "Cache",
    total: "Total",
    disponible: "Available",
    libre: "Free",
    buffers: "Buffers",
    compartida: "Shared",
    swap_total: "Swap total",
    swap_usada: "Swap used",
    titulo_detalle: "Details",
    titulo_top_memoria: "Top processes by memory",

    // Pestaña Almacenamiento
    titulo_discos: "Physical disks",
    col_disco: "Disk",
    col_tipo: "Type",
    col_tamano: "Size",
    col_temp: "Temp",
    col_lectura: "Read",
    col_escritura: "Write",
    titulo_particiones: "Mounted partitions",
    usados: "used",
    libres_de: "free of",

    // Pestaña GPU
    gpu_ninguna: "No GPU found",
    fabricante: "Vendor",
    driver: "Driver",
    pci: "PCI",
    estado: "State",
    gpu_estado_activa: "Active",
    gpu_estado_reposo: "Suspended (saving power)",
    vram: "VRAM",
    consumo: "Power draw",
    gpu_consultando: "Querying nvidia-smi…",
    gpu_no_despertar: "Not queried, to avoid waking it up.",

    // Pestaña Red
    descarga: "Download",
    subida: "Upload",
    titulo_interfaces: "Interfaces",
    col_interfaz: "Interface",
    col_ahora: "now",
    col_total: "total",

    // Pestaña Procesos
    col_tiempo_cpu: "CPU time",
    col_comando: "Command",
    solo_mios: "mine only",

    // Estados de proceso
    proc_ejecutando: "Running",
    proc_durmiendo: "Sleeping",
    proc_inactivo: "Idle",
    proc_espera_disco: "Disk I/O",
    proc_detenido: "Stopped",
    proc_zombi: "Zombie",
    proc_otro: "Other",

    // Pie de atajos
    pie_aceptar: "accept",
    pie_borrar: "clear",
    pie_mover: "move",
    pie_buscar: "search",
    pie_ordenar: "sort",
    pie_terminar: "end",
    pie_matar: "kill",
    pie_pestana: "tab",
    pie_ir_a: "go to",
    pie_pausa: "pause",
    pie_ayuda: "help",
    pie_salir: "quit",
    tecla_espacio: "space",

    // Ventana de confirmación
    confirmar_titulo: "End process",
    confirmar_enviar: "Send",
    confirmar_a: "to",
    confirmar_forzar: "(force)",
    confirmar_opciones: "y / Enter: yes      any other key: no",

    // Avisos al enviar señales
    aviso_enviado_a: "sent to",
    aviso_no_se_pudo_terminar: "Could not end",
    senal_no_existe: "the process no longer exists",
    senal_sin_permiso: "permission denied (owned by another user?)",
    senal_no_soportada: "signal not supported",

    // Ventana de ayuda
    ayuda_titulo: "Help — press any key to close",
    ayuda_general: "General",
    ayuda_cambiar_pestana: "Switch tab",
    ayuda_ir_pestana: "Go to a tab",
    ayuda_pausa: "Pause / resume updates",
    ayuda_salir: "Quit",
    ayuda_mover: "Move (PgUp/PgDn, g/G)",
    ayuda_buscar: "Search by name, command, PID or user",
    ayuda_ordenar: "Sort by CPU, memory, PID, name, user",
    ayuda_invertir: "(press again to reverse the order)",
    ayuda_solo_mios: "Only my processes",
    ayuda_quitar_filtros: "Clear filters",
    ayuda_terminar: "End (SIGTERM), asks for confirmation",
    ayuda_matar: "Kill (SIGKILL), asks for confirmation",

    // Command line
    cli_ayuda: "\
memtop — system information and resource monitor for the terminal

USAGE:
    memtop [OPTIONS]

OPTIONS:
    -i, --interval SECS     Seconds between refreshes (minimum 0.2)
    --lang LANG             Interface language: auto (from LANG), en or es
    -c, --config PATH       Use this configuration file
    --print-config          Print a commented example configuration
    -V, --version           Show the version
    -h, --help              Show this help

CONFIGURATION:
    $XDG_CONFIG_HOME/memtop/config.toml (usually ~/.config/memtop/config.toml) is read
    if it exists. The options above take precedence over the file. To create it:

        mkdir -p ~/.config/memtop && memtop --print-config > ~/.config/memtop/config.toml

Inside memtop, press ? to see every key.
",
    cli_prueba_ayuda: "Try `memtop --help`.",
    cli_falta_ruta: "--config needs a path",
    cli_falta_idioma: "--lang needs a value: auto, en or es",
    cli_idioma_invalido: "--lang only accepts auto, en or es",
    cli_intervalo_invalido: "--interval needs a number of seconds (minimum 0.2)",
    cli_opcion_desconocida: "unknown option",
    cli_sin_terminal: "must be run in an interactive terminal.",

    // Configuration file errors
    config_intervalo_minimo: "interval must be a number of seconds greater than or equal to",
    config_color_invalido: "invalid color",
    config_color_formatos: "use a name like \"cyan\", \"#rrggbb\" or a number from 0 to 255",
    config_no_se_pudo_leer: "could not read",
    config_error_en: "error in",

    // System read errors
    sistema_sin_info: "could not read system information",
    sistema_sin_discos: "could not read the disks",
    sistema_sin_red: "could not read the network",
    sistema_sin_sensores: "could not read the sensors",

    // GPU sin nombre en la base PCI
    fabricante_desconocido: "Unknown",
};

#[cfg(test)]
mod tests {
    use super::*;

    fn con(vars: &[(&str, &str)]) -> Idioma {
        detectar_con(|k| {
            vars.iter()
                .find(|(c, _)| *c == k)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn detecta_espanol_en_cualquier_pais() {
        assert_eq!(con(&[("LANG", "es_CO.UTF-8")]), Idioma::Es);
        assert_eq!(con(&[("LANG", "es_ES@euro")]), Idioma::Es);
        assert_eq!(con(&[("LANG", "es")]), Idioma::Es);
    }

    #[test]
    fn cualquier_otro_idioma_usa_ingles() {
        assert_eq!(con(&[("LANG", "en_US.UTF-8")]), Idioma::En);
        assert_eq!(con(&[("LANG", "fr_FR.UTF-8")]), Idioma::En);
        assert_eq!(con(&[("LANG", "C.UTF-8")]), Idioma::En);
        assert_eq!(con(&[]), Idioma::En);
    }

    #[test]
    fn respeta_la_prioridad_de_las_variables() {
        assert_eq!(
            con(&[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "es_MX.UTF-8")]),
            Idioma::Es
        );
        assert_eq!(
            con(&[("LC_MESSAGES", "es_MX.UTF-8"), ("LC_ALL", "en_GB.UTF-8")]),
            Idioma::En
        );
        assert_eq!(
            con(&[("LANG", "en_US.UTF-8"), ("LANGUAGE", "es:en")]),
            Idioma::Es
        );
        assert_eq!(
            con(&[("LANG", ""), ("LC_ALL", ""), ("LC_MESSAGES", "es_AR")]),
            Idioma::Es
        );
    }

    #[test]
    fn la_preferencia_explicita_ignora_el_entorno() {
        assert_eq!(Preferencia::Es.resolver(), Idioma::Es);
        assert_eq!(Preferencia::En.resolver(), Idioma::En);
    }

    #[test]
    fn lee_la_preferencia_de_texto() {
        assert_eq!("auto".parse(), Ok(Preferencia::Auto));
        assert_eq!("ES".parse(), Ok(Preferencia::Es));
        assert_eq!("en".parse(), Ok(Preferencia::En));
        assert_eq!("fr".parse::<Preferencia>(), Err(()));
    }

    #[test]
    fn language_se_ignora_con_el_locale_c() {
        assert_eq!(con(&[("LANG", "C"), ("LANGUAGE", "es")]), Idioma::En);
    }
}
