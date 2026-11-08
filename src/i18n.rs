//! Interface language and translated text.
//!
//! The language is chosen at startup with [`set`], from the `--lang` option, the `language`
//! configuration key or, when both are `auto`, the environment ([`detect`]). Text is read
//! with [`t`], which returns the table for the active language.
//!
//! To add a text: create the field in [`Texts`] and give it a value in [`ES`] and [`EN`].
//! The compiler makes sure it is translated in both languages.

use std::{
    env,
    str::FromStr,
    sync::atomic::{AtomicU8, Ordering},
};

use serde::Deserialize;

/// Available languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Es,
    En,
}

/// Language requested by the user in the configuration (`language`) or with `--lang`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageSetting {
    /// Detect from the environment (`LANG`…).
    #[default]
    Auto,
    Es,
    En,
}

impl LanguageSetting {
    pub fn resolve(self) -> Language {
        match self {
            LanguageSetting::Auto => detect(),
            LanguageSetting::Es => Language::Es,
            LanguageSetting::En => Language::En,
        }
    }
}

impl FromStr for LanguageSetting {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(LanguageSetting::Auto),
            "es" => Ok(LanguageSetting::Es),
            "en" => Ok(LanguageSetting::En),
            _ => Err(()),
        }
    }
}

/// Active language: `0` = not set yet (detected from the environment), `1` = Spanish,
/// `2` = English.
///
/// It can be set more than once because the language is refined at startup: first the
/// environment's (option errors), then `--lang` (configuration file errors) and finally the
/// result of combining the file and the options.
static LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// Sets the language of the text from now on.
pub fn set(language: Language) {
    let v = match language {
        Language::Es => 1,
        Language::En => 2,
    };
    LANGUAGE.store(v, Ordering::Relaxed);
}

/// Active language: the one given to [`set`] or, if none was, the environment's.
pub fn current() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::Es,
        2 => Language::En,
        _ => {
            let language = detect();
            set(language);
            language
        }
    }
}

/// Text for the active language.
pub fn t() -> &'static Texts {
    match current() {
        Language::Es => &ES,
        Language::En => &EN,
    }
}

/// Language from the environment variables, in gettext's order of precedence:
/// `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG`. Spanish if the language is `es`;
/// English otherwise (including the `C`/`POSIX` locale).
pub fn detect() -> Language {
    detect_with(|key| env::var(key).ok())
}

fn detect_with(var: impl Fn(&str) -> Option<String>) -> Language {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| var(k).filter(|v| !v.is_empty()));
    // gettext ignores LANGUAGE when the locale is C/POSIX.
    let is_c = locale
        .as_deref()
        .is_none_or(|l| l == "C" || l == "POSIX" || l.starts_with("C."));
    let preferred = var("LANGUAGE")
        .filter(|v| !v.is_empty() && !is_c)
        .and_then(|v| v.split(':').next().map(str::to_string));
    match preferred.or(locale) {
        Some(v) if language_code(&v) == "es" => Language::Es,
        _ => Language::En,
    }
}

/// `"es_CO.UTF-8@euro"` → `"es"`.
fn language_code(locale: &str) -> String {
    locale
        .split(['_', '.', '@', '-'])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

/// Every text of the interface in one language.
pub struct Texts {
    // Tabs
    pub tab_summary: &'static str,
    pub tab_cpu: &'static str,
    pub tab_ram: &'static str,
    pub tab_storage: &'static str,
    pub tab_gpu: &'static str,
    pub tab_network: &'static str,
    pub tab_processes: &'static str,

    // Top bar
    pub paused: &'static str,

    // System panel
    pub title_system: &'static str,
    pub machine: &'static str,
    pub os: &'static str,
    pub kernel: &'static str,
    pub uptime: &'static str,
    pub desktop: &'static str,
    pub shell: &'static str,
    pub terminal: &'static str,
    pub cpu: &'static str,
    pub gpu: &'static str,
    pub memory: &'static str,
    pub root_disk: &'static str,
    pub battery: &'static str,

    // Battery state
    pub battery_charging: &'static str,
    pub battery_discharging: &'static str,
    pub battery_full: &'static str,
    pub battery_not_charging: &'static str,
    pub battery_unknown: &'static str,

    // Summary tab
    pub usage: &'static str,
    pub swap: &'static str,
    pub title_gpu_network_power: &'static str,
    pub title_top_cpu: &'static str,
    pub network: &'static str,
    pub network_disconnected: &'static str,
    pub gpu_active: &'static str,
    pub gpu_idle: &'static str,

    // Process columns
    pub col_pid: &'static str,
    pub col_name: &'static str,
    pub col_cpu: &'static str,
    pub col_mem: &'static str,
    pub col_res: &'static str,
    pub col_user: &'static str,

    // CPU tab
    pub title_processor: &'static str,
    pub model: &'static str,
    pub cores: &'static str,
    pub physical: &'static str,
    pub threads: &'static str,
    pub frequency: &'static str,
    pub load_average: &'static str,
    pub architecture: &'static str,
    pub temperature: &'static str,
    pub total_usage: &'static str,
    pub last: &'static str,
    pub title_per_thread: &'static str,
    pub title_sensors: &'static str,

    // RAM tab
    pub used: &'static str,
    pub cache: &'static str,
    pub total: &'static str,
    pub available: &'static str,
    pub free: &'static str,
    pub buffers: &'static str,
    pub shared: &'static str,
    pub swap_total: &'static str,
    pub swap_used: &'static str,
    pub title_details: &'static str,
    pub title_top_memory: &'static str,

    // Storage tab
    pub title_disks: &'static str,
    pub col_disk: &'static str,
    pub col_type: &'static str,
    pub col_size: &'static str,
    pub col_temp: &'static str,
    pub col_read: &'static str,
    pub col_write: &'static str,
    pub title_partitions: &'static str,
    pub used_space: &'static str,
    pub free_of: &'static str,

    // GPU tab
    pub gpu_none: &'static str,
    pub vendor: &'static str,
    pub driver: &'static str,
    pub pci: &'static str,
    pub state: &'static str,
    pub gpu_state_active: &'static str,
    pub gpu_state_idle: &'static str,
    pub vram: &'static str,
    pub power: &'static str,
    pub gpu_querying: &'static str,
    pub gpu_not_waking: &'static str,

    // Network tab
    pub download: &'static str,
    pub upload: &'static str,
    pub title_interfaces: &'static str,
    pub col_interface: &'static str,
    pub col_now: &'static str,
    pub col_total: &'static str,

    // Processes tab
    pub col_cpu_time: &'static str,
    pub col_command: &'static str,
    pub mine_only: &'static str,

    // Process states
    pub proc_running: &'static str,
    pub proc_sleeping: &'static str,
    pub proc_idle: &'static str,
    pub proc_disk_wait: &'static str,
    pub proc_stopped: &'static str,
    pub proc_zombie: &'static str,
    pub proc_other: &'static str,

    // Shortcut footer
    pub footer_accept: &'static str,
    pub footer_clear: &'static str,
    pub footer_move: &'static str,
    pub footer_search: &'static str,
    pub footer_sort: &'static str,
    pub footer_terminate: &'static str,
    pub footer_kill: &'static str,
    pub footer_tab: &'static str,
    pub footer_go_to: &'static str,
    pub footer_pause: &'static str,
    pub footer_help: &'static str,
    pub footer_quit: &'static str,
    pub key_space: &'static str,

    // Confirmation dialog
    pub confirm_title: &'static str,
    pub confirm_send: &'static str,
    pub confirm_to: &'static str,
    pub confirm_force: &'static str,
    pub confirm_options: &'static str,

    // Notices when sending signals
    pub notice_sent_to: &'static str,
    pub notice_could_not_terminate: &'static str,
    pub signal_no_such_process: &'static str,
    pub signal_permission_denied: &'static str,
    pub signal_unsupported: &'static str,

    // Help dialog
    pub help_title: &'static str,
    pub help_general: &'static str,
    pub help_switch_tab: &'static str,
    pub help_go_to_tab: &'static str,
    pub help_pause: &'static str,
    pub help_quit: &'static str,
    pub help_move: &'static str,
    pub help_search: &'static str,
    pub help_sort: &'static str,
    pub help_reverse: &'static str,
    pub help_mine_only: &'static str,
    pub help_clear_filters: &'static str,
    pub help_terminate: &'static str,
    pub help_kill: &'static str,

    // Command line (`--help` and errors before the interface opens)
    pub cli_help: &'static str,
    pub cli_try_help: &'static str,
    pub cli_missing_path: &'static str,
    pub cli_missing_lang: &'static str,
    pub cli_invalid_lang: &'static str,
    pub cli_invalid_interval: &'static str,
    pub cli_unknown_option: &'static str,
    pub cli_no_terminal: &'static str,

    // Configuration file errors
    pub config_min_interval: &'static str,
    pub config_invalid_color: &'static str,
    pub config_color_formats: &'static str,
    pub config_could_not_read: &'static str,
    pub config_error_in: &'static str,

    // System read errors
    pub system_no_info: &'static str,
    pub system_no_disks: &'static str,
    pub system_no_network: &'static str,
    pub system_no_sensors: &'static str,

    // GPU missing from the PCI database
    pub unknown_vendor: &'static str,
}

pub const ES: Texts = Texts {
    tab_summary: "Resumen",
    tab_cpu: "CPU",
    tab_ram: "RAM",
    tab_storage: "Almacenamiento",
    tab_gpu: "GPU",
    tab_network: "Red",
    tab_processes: "Procesos",

    // Top bar
    paused: "PAUSA",

    // System panel
    title_system: "Sistema",
    machine: "Equipo",
    os: "SO",
    kernel: "Kernel",
    uptime: "Encendido",
    desktop: "Escritorio",
    shell: "Shell",
    terminal: "Terminal",
    cpu: "CPU",
    gpu: "GPU",
    memory: "Memoria",
    root_disk: "Disco (/)",
    battery: "Batería",

    // Battery state
    battery_charging: "Cargando",
    battery_discharging: "Descargando",
    battery_full: "Completa",
    battery_not_charging: "Conectada, sin cargar",
    battery_unknown: "Desconocido",

    // Summary tab
    usage: "Uso",
    swap: "Swap",
    title_gpu_network_power: "GPU · Red · Energía",
    title_top_cpu: "Procesos con más CPU",
    network: "Red",
    network_disconnected: "sin conexión",
    gpu_active: "activa",
    gpu_idle: "en reposo",

    // Process columns
    col_pid: "PID",
    col_name: "Nombre",
    col_cpu: "CPU%",
    col_mem: "MEM%",
    col_res: "RES",
    col_user: "Usuario",

    // CPU tab
    title_processor: "Procesador",
    model: "Modelo",
    cores: "Núcleos",
    physical: "físicos",
    threads: "hilos",
    frequency: "Frecuencia",
    load_average: "Carga media",
    architecture: "Arquitectura",
    temperature: "Temperatura",
    total_usage: "Uso total",
    last: "últimos",
    title_per_thread: "Por hilo",
    title_sensors: "Sensores",

    // RAM tab
    used: "Usada",
    cache: "Caché",
    total: "Total",
    available: "Disponible",
    free: "Libre",
    buffers: "Buffers",
    shared: "Compartida",
    swap_total: "Swap total",
    swap_used: "Swap usada",
    title_details: "Detalle",
    title_top_memory: "Procesos con más memoria",

    // Storage tab
    title_disks: "Discos físicos",
    col_disk: "Disco",
    col_type: "Tipo",
    col_size: "Tamaño",
    col_temp: "Temp",
    col_read: "Lectura",
    col_write: "Escritura",
    title_partitions: "Particiones montadas",
    used_space: "usados",
    free_of: "libres de",

    // GPU tab
    gpu_none: "No se encontró ninguna GPU",
    vendor: "Fabricante",
    driver: "Driver",
    pci: "PCI",
    state: "Estado",
    gpu_state_active: "Activa",
    gpu_state_idle: "En reposo (ahorrando energía)",
    vram: "VRAM",
    power: "Consumo",
    gpu_querying: "Consultando nvidia-smi…",
    gpu_not_waking: "No se le consulta nada para no despertarla.",

    // Network tab
    download: "Descarga",
    upload: "Subida",
    title_interfaces: "Interfaces",
    col_interface: "Interfaz",
    col_now: "ahora",
    col_total: "total",

    // Processes tab
    col_cpu_time: "T. CPU",
    col_command: "Comando",
    mine_only: "solo míos",

    // Process states
    proc_running: "Activo",
    proc_sleeping: "Durmiendo",
    proc_idle: "Inactivo",
    proc_disk_wait: "E/S disco",
    proc_stopped: "Detenido",
    proc_zombie: "Zombi",
    proc_other: "Otro",

    // Shortcut footer
    footer_accept: "aceptar",
    footer_clear: "borrar",
    footer_move: "mover",
    footer_search: "buscar",
    footer_sort: "ordenar",
    footer_terminate: "terminar",
    footer_kill: "matar",
    footer_tab: "pestaña",
    footer_go_to: "ir a",
    footer_pause: "pausa",
    footer_help: "ayuda",
    footer_quit: "salir",
    key_space: "espacio",

    // Confirmation dialog
    confirm_title: "Terminar proceso",
    confirm_send: "¿Enviar",
    confirm_to: "a",
    confirm_force: "(forzar)",
    confirm_options: "s / Enter: sí      cualquier otra tecla: no",

    // Notices when sending signals
    notice_sent_to: "enviado a",
    notice_could_not_terminate: "No se pudo terminar",
    signal_no_such_process: "el proceso ya no existe",
    signal_permission_denied: "sin permiso (¿es de otro usuario?)",
    signal_unsupported: "señal no soportada",

    // Help dialog
    help_title: "Ayuda — cualquier tecla para cerrar",
    help_general: "General",
    help_switch_tab: "Cambiar de pestaña",
    help_go_to_tab: "Ir a una pestaña",
    help_pause: "Pausar / reanudar la actualización",
    help_quit: "Salir",
    help_move: "Moverse (PgUp/PgDn, g/G)",
    help_search: "Buscar por nombre, comando, PID o usuario",
    help_sort: "Ordenar por CPU, memoria, PID, nombre, usuario",
    help_reverse: "(repetir la tecla invierte el orden)",
    help_mine_only: "Solo mis procesos",
    help_clear_filters: "Quitar filtros",
    help_terminate: "Terminar (SIGTERM), pide confirmación",
    help_kill: "Matar (SIGKILL), pide confirmación",

    // Command line
    cli_help: "\
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
    cli_try_help: "Prueba `memtop --help`.",
    cli_missing_path: "--config necesita una ruta",
    cli_missing_lang: "--lang necesita un valor: auto, en o es",
    cli_invalid_lang: "--lang solo admite auto, en o es",
    cli_invalid_interval: "--interval necesita un número de segundos (mínimo 0.2)",
    cli_unknown_option: "opción desconocida",
    cli_no_terminal: "necesita ejecutarse en una terminal interactiva.",

    // Configuration file errors
    config_min_interval: "interval debe ser un número de segundos mayor o igual que",
    config_invalid_color: "color no válido",
    config_color_formats: "usa un nombre como \"cyan\", \"#rrggbb\" o un número de 0 a 255",
    config_could_not_read: "no se pudo leer",
    config_error_in: "error en",

    // System read errors
    system_no_info: "no se pudo leer la información del sistema",
    system_no_disks: "no se pudieron leer los discos",
    system_no_network: "no se pudo leer la red",
    system_no_sensors: "no se pudieron leer los sensores",

    // GPU missing from the PCI database
    unknown_vendor: "Desconocido",
};

pub const EN: Texts = Texts {
    tab_summary: "Summary",
    tab_cpu: "CPU",
    tab_ram: "RAM",
    tab_storage: "Storage",
    tab_gpu: "GPU",
    tab_network: "Network",
    tab_processes: "Processes",

    // Top bar
    paused: "PAUSED",

    // System panel
    title_system: "System",
    machine: "Host",
    os: "OS",
    kernel: "Kernel",
    uptime: "Uptime",
    desktop: "Desktop",
    shell: "Shell",
    terminal: "Terminal",
    cpu: "CPU",
    gpu: "GPU",
    memory: "Memory",
    root_disk: "Disk (/)",
    battery: "Battery",

    // Battery state
    battery_charging: "Charging",
    battery_discharging: "Discharging",
    battery_full: "Full",
    battery_not_charging: "Plugged in, not charging",
    battery_unknown: "Unknown",

    // Summary tab
    usage: "Usage",
    swap: "Swap",
    title_gpu_network_power: "GPU · Network · Power",
    title_top_cpu: "Top processes by CPU",
    network: "Network",
    network_disconnected: "not connected",
    gpu_active: "active",
    gpu_idle: "suspended",

    // Process columns
    col_pid: "PID",
    col_name: "Name",
    col_cpu: "CPU%",
    col_mem: "MEM%",
    col_res: "RES",
    col_user: "User",

    // CPU tab
    title_processor: "Processor",
    model: "Model",
    cores: "Cores",
    physical: "physical",
    threads: "threads",
    frequency: "Frequency",
    load_average: "Load average",
    architecture: "Architecture",
    temperature: "Temperature",
    total_usage: "Total usage",
    last: "last",
    title_per_thread: "Per thread",
    title_sensors: "Sensors",

    // RAM tab
    used: "Used",
    cache: "Cache",
    total: "Total",
    available: "Available",
    free: "Free",
    buffers: "Buffers",
    shared: "Shared",
    swap_total: "Swap total",
    swap_used: "Swap used",
    title_details: "Details",
    title_top_memory: "Top processes by memory",

    // Storage tab
    title_disks: "Physical disks",
    col_disk: "Disk",
    col_type: "Type",
    col_size: "Size",
    col_temp: "Temp",
    col_read: "Read",
    col_write: "Write",
    title_partitions: "Mounted partitions",
    used_space: "used",
    free_of: "free of",

    // GPU tab
    gpu_none: "No GPU found",
    vendor: "Vendor",
    driver: "Driver",
    pci: "PCI",
    state: "State",
    gpu_state_active: "Active",
    gpu_state_idle: "Suspended (saving power)",
    vram: "VRAM",
    power: "Power draw",
    gpu_querying: "Querying nvidia-smi…",
    gpu_not_waking: "Not queried, to avoid waking it up.",

    // Network tab
    download: "Download",
    upload: "Upload",
    title_interfaces: "Interfaces",
    col_interface: "Interface",
    col_now: "now",
    col_total: "total",

    // Processes tab
    col_cpu_time: "CPU time",
    col_command: "Command",
    mine_only: "mine only",

    // Process states
    proc_running: "Running",
    proc_sleeping: "Sleeping",
    proc_idle: "Idle",
    proc_disk_wait: "Disk I/O",
    proc_stopped: "Stopped",
    proc_zombie: "Zombie",
    proc_other: "Other",

    // Shortcut footer
    footer_accept: "accept",
    footer_clear: "clear",
    footer_move: "move",
    footer_search: "search",
    footer_sort: "sort",
    footer_terminate: "end",
    footer_kill: "kill",
    footer_tab: "tab",
    footer_go_to: "go to",
    footer_pause: "pause",
    footer_help: "help",
    footer_quit: "quit",
    key_space: "space",

    // Confirmation dialog
    confirm_title: "End process",
    confirm_send: "Send",
    confirm_to: "to",
    confirm_force: "(force)",
    confirm_options: "y / Enter: yes      any other key: no",

    // Notices when sending signals
    notice_sent_to: "sent to",
    notice_could_not_terminate: "Could not end",
    signal_no_such_process: "the process no longer exists",
    signal_permission_denied: "permission denied (owned by another user?)",
    signal_unsupported: "signal not supported",

    // Help dialog
    help_title: "Help — press any key to close",
    help_general: "General",
    help_switch_tab: "Switch tab",
    help_go_to_tab: "Go to a tab",
    help_pause: "Pause / resume updates",
    help_quit: "Quit",
    help_move: "Move (PgUp/PgDn, g/G)",
    help_search: "Search by name, command, PID or user",
    help_sort: "Sort by CPU, memory, PID, name, user",
    help_reverse: "(press again to reverse the order)",
    help_mine_only: "Only my processes",
    help_clear_filters: "Clear filters",
    help_terminate: "End (SIGTERM), asks for confirmation",
    help_kill: "Kill (SIGKILL), asks for confirmation",

    // Command line
    cli_help: "\
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
    cli_try_help: "Try `memtop --help`.",
    cli_missing_path: "--config needs a path",
    cli_missing_lang: "--lang needs a value: auto, en or es",
    cli_invalid_lang: "--lang only accepts auto, en or es",
    cli_invalid_interval: "--interval needs a number of seconds (minimum 0.2)",
    cli_unknown_option: "unknown option",
    cli_no_terminal: "must be run in an interactive terminal.",

    // Configuration file errors
    config_min_interval: "interval must be a number of seconds greater than or equal to",
    config_invalid_color: "invalid color",
    config_color_formats: "use a name like \"cyan\", \"#rrggbb\" or a number from 0 to 255",
    config_could_not_read: "could not read",
    config_error_in: "error in",

    // System read errors
    system_no_info: "could not read system information",
    system_no_disks: "could not read the disks",
    system_no_network: "could not read the network",
    system_no_sensors: "could not read the sensors",

    // GPU missing from the PCI database
    unknown_vendor: "Unknown",
};

#[cfg(test)]
mod tests {
    use super::*;

    fn with(vars: &[(&str, &str)]) -> Language {
        detect_with(|k| {
            vars.iter()
                .find(|(c, _)| *c == k)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn detects_spanish_in_any_country() {
        assert_eq!(with(&[("LANG", "es_CO.UTF-8")]), Language::Es);
        assert_eq!(with(&[("LANG", "es_ES@euro")]), Language::Es);
        assert_eq!(with(&[("LANG", "es")]), Language::Es);
    }

    #[test]
    fn any_other_language_uses_english() {
        assert_eq!(with(&[("LANG", "en_US.UTF-8")]), Language::En);
        assert_eq!(with(&[("LANG", "fr_FR.UTF-8")]), Language::En);
        assert_eq!(with(&[("LANG", "C.UTF-8")]), Language::En);
        assert_eq!(with(&[]), Language::En);
    }

    #[test]
    fn respects_variable_precedence() {
        assert_eq!(
            with(&[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "es_MX.UTF-8")]),
            Language::Es
        );
        assert_eq!(
            with(&[("LC_MESSAGES", "es_MX.UTF-8"), ("LC_ALL", "en_GB.UTF-8")]),
            Language::En
        );
        assert_eq!(
            with(&[("LANG", "en_US.UTF-8"), ("LANGUAGE", "es:en")]),
            Language::Es
        );
        assert_eq!(
            with(&[("LANG", ""), ("LC_ALL", ""), ("LC_MESSAGES", "es_AR")]),
            Language::Es
        );
    }

    #[test]
    fn explicit_setting_ignores_the_environment() {
        assert_eq!(LanguageSetting::Es.resolve(), Language::Es);
        assert_eq!(LanguageSetting::En.resolve(), Language::En);
    }

    #[test]
    fn parses_the_setting_from_text() {
        assert_eq!("auto".parse(), Ok(LanguageSetting::Auto));
        assert_eq!("ES".parse(), Ok(LanguageSetting::Es));
        assert_eq!("en".parse(), Ok(LanguageSetting::En));
        assert_eq!("fr".parse::<LanguageSetting>(), Err(()));
    }

    #[test]
    fn language_is_ignored_with_the_c_locale() {
        assert_eq!(with(&[("LANG", "C"), ("LANGUAGE", "es")]), Language::En);
    }
}
