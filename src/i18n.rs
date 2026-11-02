//! Idioma de la interfaz y textos traducidos.
//!
//! El idioma se elige una vez al arrancar con [`fijar`], a partir de la opción
//! `--idioma`, la clave `idioma` de la configuración o, si ambas son `auto`, del entorno
//! ([`detectar`]). Los textos se obtienen con [`t`], que devuelve la tabla del idioma activo.
//!
//! Para añadir un texto: crea el campo en [`Textos`] y dale valor en [`ES`] y [`EN`].
//! El compilador obliga a traducirlo en los dos idiomas.

use std::{env, str::FromStr, sync::OnceLock};

use serde::Deserialize;

/// Idiomas disponibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    Es,
    En,
}

/// Idioma pedido por el usuario en la configuración o en `--idioma`.
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

static IDIOMA: OnceLock<Idioma> = OnceLock::new();

/// Fija el idioma de la interfaz. Se llama una vez al arrancar, antes del primer dibujo.
pub fn fijar(idioma: Idioma) {
    let _ = IDIOMA.set(idioma);
}

/// Idioma activo: el fijado con [`fijar`] o, si no se fijó, el del entorno.
pub fn actual() -> Idioma {
    *IDIOMA.get_or_init(detectar)
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

    // Panel de imagen
    pub foto_otra: &'static str,
    pub foto_tus_fotos: &'static str,
    pub foto_no_se_pudo_abrir: &'static str,
    pub foto_dibujo_vacio: &'static str,

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

    // Panel de imagen
    foto_otra: "f: otra",
    foto_tus_fotos: "tus fotos:",
    foto_no_se_pudo_abrir: "No se pudo abrir",
    foto_dibujo_vacio: "dibujo vacío",

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

    // Panel de imagen
    foto_otra: "f: next",
    foto_tus_fotos: "your photos:",
    foto_no_se_pudo_abrir: "Could not open",
    foto_dibujo_vacio: "empty drawing",

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
