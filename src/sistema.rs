//! Lectura de la información del equipo.
//!
//! Lo estático (modelo, SO, CPU…) se lee una vez al arrancar; lo dinámico (uso de CPU,
//! memoria, discos, red, procesos, sensores) en cada llamada a [`Monitor::actualizar`].
//!
//! Fuentes: el crate `sysinfo` para CPU, procesos, red, particiones y sensores, y lectura
//! directa de `/proc/meminfo`, `/sys/block`, `/sys/class/power_supply`, `/sys/class/dmi/id`
//! y `/etc/os-release` para el resto. No necesita privilegios de root.

use std::{
    collections::{HashMap, VecDeque},
    env, fs,
    path::Path,
    time::Instant,
};

use anyhow::{Context, Result};
use sysinfo::{Components, Disks, Networks, Pid, ProcessStatus, ProcessesToUpdate, System, Users};

use crate::{
    gpu::{self, Gpu},
    i18n,
};

/// Segundos de historial que se guardan para las gráficas.
pub const HISTORIAL: usize = 120;

pub struct Info {
    pub usuario: String,
    pub host: String,
    pub modelo: Option<String>,
    pub so: String,
    pub kernel: String,
    pub escritorio: Option<String>,
    pub shell: Option<String>,
    pub terminal: Option<String>,
    pub cpu_modelo: String,
    pub nucleos_fisicos: Option<usize>,
    pub hilos: usize,
    pub arquitectura: String,
}

pub struct Memoria {
    pub total: u64,
    pub disponible: u64,
    pub libre: u64,
    pub buffers: u64,
    pub cache: u64,
    pub compartida: u64,
    pub swap_total: u64,
    pub swap_libre: u64,
}

impl Memoria {
    pub fn usada(&self) -> u64 {
        self.total.saturating_sub(self.disponible)
    }
    pub fn swap_usada(&self) -> u64 {
        self.swap_total.saturating_sub(self.swap_libre)
    }
}

/// Disco físico (de /sys/block).
pub struct DiscoFisico {
    pub nombre: String,
    pub modelo: Option<String>,
    pub tamano: u64,
    pub tipo: &'static str,
    pub temperatura: Option<f32>,
    pub lectura_bps: f64,
    pub escritura_bps: f64,
    sectores_previos: Option<(u64, u64)>,
}

/// Sistema de archivos montado.
pub struct Montaje {
    pub dispositivo: String,
    pub punto: String,
    pub fs: String,
    pub total: u64,
    pub usado: u64,
}

pub struct Interfaz {
    pub nombre: String,
    /// Tiene hardware detrás (no es docker0, veth, puente virtual…).
    pub fisica: bool,
    pub mac: String,
    pub ips: Vec<String>,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

pub struct Proceso {
    pub pid: u32,
    pub usuario: String,
    pub nombre: String,
    pub comando: String,
    pub cpu: f32,
    pub memoria: u64,
    pub estado: EstadoProceso,
    /// Segundos de CPU consumidos (TIME+ de htop).
    pub tiempo: u64,
}

/// Estado de un proceso, agrupando los de `sysinfo` en nombres cortos y traducibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoProceso {
    Ejecutando,
    Durmiendo,
    Inactivo,
    /// Esperando E/S de disco sin poder interrumpirse (estado `D`).
    EsperaDisco,
    Detenido,
    Zombi,
    Otro,
}

impl From<ProcessStatus> for EstadoProceso {
    fn from(s: ProcessStatus) -> Self {
        match s {
            ProcessStatus::Run | ProcessStatus::Waking => Self::Ejecutando,
            ProcessStatus::Sleep => Self::Durmiendo,
            ProcessStatus::Idle | ProcessStatus::Parked => Self::Inactivo,
            ProcessStatus::UninterruptibleDiskSleep => Self::EsperaDisco,
            ProcessStatus::Stop | ProcessStatus::Tracing => Self::Detenido,
            ProcessStatus::Zombie | ProcessStatus::Dead => Self::Zombi,
            _ => Self::Otro,
        }
    }
}

/// Por qué no se pudo enviar una señal a un proceso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSenal {
    NoExiste,
    SinPermiso,
    NoSoportada,
}

pub struct Bateria {
    pub porcentaje: u8,
    pub estado: EstadoBateria,
}

/// Estado de carga, según `/sys/class/power_supply/*/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoBateria {
    Cargando,
    Descargando,
    Completa,
    /// Conectada a la corriente pero sin cargar (p. ej. por un límite de carga).
    SinCargar,
    Desconocido,
}

impl EstadoBateria {
    fn desde_sys(texto: &str) -> Self {
        match texto {
            "Charging" => Self::Cargando,
            "Discharging" => Self::Descargando,
            "Full" => Self::Completa,
            "Not charging" => Self::SinCargar,
            _ => Self::Desconocido,
        }
    }
}

pub struct Temperatura {
    pub etiqueta: String,
    pub grados: f32,
    pub critica: Option<f32>,
}

pub struct Monitor {
    sys: System,
    discos_sys: Disks,
    redes: Networks,
    componentes: Components,
    nombres_usuario: HashMap<u32, String>,
    ultimo: Instant,

    pub info: Info,
    pub gpus: Vec<Gpu>,

    pub cpu_total: f32,
    pub cpu_nucleos: Vec<f32>,
    pub cpu_freq_mhz: Vec<u64>,
    pub carga: (f64, f64, f64),
    pub uptime: u64,
    pub memoria: Memoria,
    pub discos: Vec<DiscoFisico>,
    pub montajes: Vec<Montaje>,
    pub interfaces: Vec<Interfaz>,
    pub procesos: Vec<Proceso>,
    pub temperaturas: Vec<Temperatura>,
    pub bateria: Option<Bateria>,

    pub hist_cpu: VecDeque<u64>,
    pub hist_ram: VecDeque<u64>,
    pub hist_rx: VecDeque<u64>,
    pub hist_tx: VecDeque<u64>,
}

impl Monitor {
    /// Hace la primera lectura completa. Solo falla si el sistema no expone `/proc`.
    pub fn new() -> Result<Self> {
        let mut sys = System::new_all().context(i18n::t().system_no_info)?;
        sys.refresh_all();
        let info = leer_info(&sys);
        let nombres_usuario = Users::new_with_refreshed_list()
            .map(|u| u.iter().map(|u| (**u.id(), u.name().to_string())).collect())
            .unwrap_or_default();

        let mut m = Self {
            discos_sys: Disks::new_with_refreshed_list().context(i18n::t().system_no_disks)?,
            redes: Networks::new_with_refreshed_list().context(i18n::t().system_no_network)?,
            componentes: Components::new_with_refreshed_list()
                .context(i18n::t().system_no_sensors)?,
            nombres_usuario,
            ultimo: Instant::now(),
            info,
            gpus: gpu::detect(),
            sys,
            cpu_total: 0.0,
            cpu_nucleos: Vec::new(),
            cpu_freq_mhz: Vec::new(),
            carga: (0.0, 0.0, 0.0),
            uptime: 0,
            memoria: leer_meminfo(),
            discos: detectar_discos(),
            montajes: Vec::new(),
            interfaces: Vec::new(),
            procesos: Vec::new(),
            temperaturas: Vec::new(),
            bateria: None,
            hist_cpu: VecDeque::with_capacity(HISTORIAL),
            hist_ram: VecDeque::with_capacity(HISTORIAL),
            hist_rx: VecDeque::with_capacity(HISTORIAL),
            hist_tx: VecDeque::with_capacity(HISTORIAL),
        };
        m.actualizar();
        Ok(m)
    }

    pub fn actualizar(&mut self) {
        let dt = self.ultimo.elapsed().as_secs_f64().max(0.001);
        self.ultimo = Instant::now();

        self.sys.refresh_cpu_all();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.cpu_total = self.sys.global_cpu_usage();
        self.cpu_nucleos = self.sys.cpus().iter().map(|c| c.usage()).collect();
        self.cpu_freq_mhz = self.sys.cpus().iter().map(|c| c.frequency()).collect();
        if let Ok(l) = System::load_average() {
            self.carga = (l.one, l.five, l.fifteen);
        }
        self.uptime = System::uptime().unwrap_or(0);
        self.memoria = leer_meminfo();

        // Montajes (sin repetir el mismo dispositivo, p. ej. subvolúmenes btrfs).
        self.discos_sys.refresh(true);
        let mut vistos = HashMap::new();
        let mut montajes: Vec<Montaje> = Vec::new();
        for d in &self.discos_sys {
            let dispositivo = d.name().to_string_lossy().into_owned();
            let punto = d.mount_point().to_string_lossy().into_owned();
            if d.total_space() == 0 || punto.starts_with("/run/") || punto.starts_with("/snap/") {
                continue;
            }
            let m = Montaje {
                fs: d.file_system().to_string_lossy().into_owned(),
                total: d.total_space(),
                usado: d.total_space().saturating_sub(d.available_space()),
                dispositivo: dispositivo.clone(),
                punto,
            };
            match vistos.get(&dispositivo) {
                Some(&i) => {
                    let actual: &mut Montaje = &mut montajes[i];
                    if m.punto.len() < actual.punto.len() {
                        *actual = m;
                    }
                }
                None => {
                    vistos.insert(dispositivo, montajes.len());
                    montajes.push(m);
                }
            }
        }
        montajes.sort_by(|a, b| a.punto.cmp(&b.punto));
        self.montajes = montajes;

        for d in &mut self.discos {
            actualizar_disco(d, dt);
        }

        // Red
        self.redes.refresh(true);
        let mut interfaces: Vec<Interfaz> = self
            .redes
            .iter()
            .filter(|(n, _)| n.to_str() != Some("lo"))
            .map(|(n, d)| Interfaz {
                fisica: Path::new("/sys/class/net").join(n).join("device").exists(),
                nombre: n.to_string_lossy().into_owned(),
                mac: d.mac_address().to_string(),
                ips: d
                    .ip_networks()
                    .iter()
                    .map(|ip| format!("{}/{}", ip.addr, ip.prefix))
                    .collect(),
                rx_bps: d.received() as f64 / dt,
                tx_bps: d.transmitted() as f64 / dt,
                rx_total: d.total_received(),
                tx_total: d.total_transmitted(),
            })
            .collect();
        // Las que tienen IP (las que se usan) primero.
        interfaces.sort_by(|a, b| {
            (!a.fisica, a.ips.is_empty(), &a.nombre).cmp(&(!b.fisica, b.ips.is_empty(), &b.nombre))
        });
        self.interfaces = interfaces;

        // Procesos
        let nucleos = self.sys.cpus().len().max(1) as f32;
        self.procesos = self
            .sys
            .processes()
            .values()
            // Hilos de usuario: htop los oculta por defecto.
            .filter(|p| p.thread_kind().is_none())
            .map(|p| {
                let uid = p.user_id().map(|u| **u);
                let comando = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                Proceso {
                    pid: p.pid().as_u32(),
                    usuario: uid
                        .and_then(|u| self.nombres_usuario.get(&u).cloned())
                        .unwrap_or_else(|| uid.map(|u| u.to_string()).unwrap_or_default()),
                    nombre: p.name().to_string_lossy().into_owned(),
                    comando,
                    cpu: p.cpu_usage().min(100.0 * nucleos),
                    memoria: p.memory(),
                    estado: EstadoProceso::from(p.status()),
                    tiempo: p.accumulated_cpu_time() / 1000,
                }
            })
            .collect();

        // Temperaturas
        self.componentes.refresh(true);
        self.temperaturas = self
            .componentes
            .iter()
            .filter_map(|c| {
                Some(Temperatura {
                    etiqueta: c.label().to_string(),
                    grados: c.temperature().filter(|t| *t > 0.0)?,
                    critica: c.critical(),
                })
            })
            .collect();

        self.bateria = leer_bateria();
        gpu::refresh(&mut self.gpus);

        let empujar = |h: &mut VecDeque<u64>, v: u64| {
            if h.len() == HISTORIAL {
                h.pop_front();
            }
            h.push_back(v);
        };
        empujar(&mut self.hist_cpu, self.cpu_total.round() as u64);
        let ram_pct = self.memoria.usada() * 100 / self.memoria.total.max(1);
        empujar(&mut self.hist_ram, ram_pct);
        empujar(
            &mut self.hist_rx,
            self.interfaces.iter().map(|i| i.rx_bps).sum::<f64>() as u64,
        );
        empujar(
            &mut self.hist_tx,
            self.interfaces.iter().map(|i| i.tx_bps).sum::<f64>() as u64,
        );
    }

    /// Envía SIGTERM (o SIGKILL si `forzar`) a un proceso.
    pub fn senal(&self, pid: u32, forzar: bool) -> Result<(), ErrorSenal> {
        let p = self
            .sys
            .process(Pid::from_u32(pid))
            .ok_or(ErrorSenal::NoExiste)?;
        let s = if forzar {
            sysinfo::Signal::Kill
        } else {
            sysinfo::Signal::Term
        };
        match p.kill_with(s) {
            Some(true) => Ok(()),
            Some(false) => Err(ErrorSenal::SinPermiso),
            None => Err(ErrorSenal::NoSoportada),
        }
    }

    pub fn temperatura_cpu(&self) -> Option<f32> {
        self.temperaturas
            .iter()
            .find(|t| {
                t.etiqueta.contains("Package")
                    || t.etiqueta.contains("Tctl")
                    || t.etiqueta.contains("Tdie")
            })
            .or_else(|| {
                self.temperaturas
                    .iter()
                    .find(|t| t.etiqueta.starts_with("coretemp"))
            })
            .map(|t| t.grados)
    }
}

fn leer(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn leer_info(sys: &System) -> Info {
    let modelo = leer("/sys/class/dmi/id/product_name")
        .and_then(|p| limpiar_modelo(leer("/sys/class/dmi/id/sys_vendor").as_deref(), &p));
    let so = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| v.trim_matches('"').to_string())
        })
        .or_else(|| System::long_os_version().ok())
        .unwrap_or_else(|| "Linux".into());

    let shell = env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(str::to_string));
    let escritorio = env::var("XDG_CURRENT_DESKTOP")
        .ok()
        // Ubuntu usa "ubuntu:GNOME", Pop!_OS "pop:GNOME": quedarse con el escritorio real.
        .map(|d| d.rsplit(':').next().unwrap_or(&d).to_string())
        .filter(|d| !d.is_empty())
        .map(|d| match env::var("XDG_SESSION_TYPE") {
            Ok(t) => format!("{d} ({t})"),
            Err(_) => d,
        });

    Info {
        usuario: env::var("USER")
            .ok()
            .or_else(|| {
                // Sin $USER (cron, contenedores…): buscar el nombre por el UID del proceso.
                let pid = sysinfo::get_current_pid().ok()?;
                let uid = sys.process(pid)?.user_id()?.clone();
                Users::new_with_refreshed_list()
                    .ok()?
                    .get_user_by_id(&uid)
                    .map(|u| u.name().to_string())
            })
            .unwrap_or_else(|| "?".into()),
        host: System::host_name().unwrap_or_else(|_| "?".into()),
        modelo,
        so,
        kernel: System::kernel_version().unwrap_or_default(),
        escritorio,
        shell,
        terminal: detectar_terminal(sys),
        cpu_modelo: sys
            .cpus()
            .first()
            .map(|c| limpiar_cpu(c.brand()))
            .unwrap_or_else(|| "?".into()),
        nucleos_fisicos: System::physical_core_count().ok(),
        hilos: sys.cpus().len(),
        arquitectura: System::cpu_arch(),
    }
}

/// Nombre del equipo a partir de DMI, sin repeticiones ni textos de relleno.
///
/// - `("Fabricante S.A.", "Modelo X1 AB12_AB12")` → `"Fabricante S.A. Modelo X1 AB12"`
/// - `("ACME COMPUTER INC.", "ACME Book 14")` → `"ACME Book 14"` (el producto ya dice la marca)
fn limpiar_modelo(fabricante: Option<&str>, producto: &str) -> Option<String> {
    const RELLENO: &[&str] = &[
        "To Be Filled",
        "To be filled",
        "System Product Name",
        "Default string",
        "Not Applicable",
    ];
    let producto = producto.trim();
    if producto.is_empty() || RELLENO.iter().any(|r| producto.contains(r)) {
        return None;
    }
    // Algunos fabricantes repiten el código de modelo: "... AB12_AB12".
    let producto = match producto.rsplit_once(' ') {
        Some((inicio, codigo)) => match codigo.split_once('_') {
            Some((a, b)) if a == b => format!("{inicio} {a}"),
            _ => producto.to_string(),
        },
        None => producto.to_string(),
    };
    let primera = producto.split(' ').next().unwrap_or("").to_lowercase();
    match fabricante.map(str::trim) {
        Some(f)
            if !f.is_empty()
                && !RELLENO.iter().any(|r| f.contains(r))
                && !f.to_lowercase().starts_with(&primera) =>
        {
            Some(format!("{f} {producto}"))
        }
        _ => Some(producto),
    }
}

/// Nombre de CPU sin marcas registradas ni texto sobrante.
///
/// - `"11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz"` → `"Intel Core i7-1165G7"`
/// - `"AMD Ryzen 7 5800X 8-Core Processor"` → `"AMD Ryzen 7 5800X"`
fn limpiar_cpu(s: &str) -> String {
    let s = s.replace("(R)", "").replace("(TM)", "").replace("(tm)", "");
    // Lo que va tras " @ " (frecuencia) o " with "/" w/ " (gráfica integrada) sobra.
    let s = [" @ ", " with ", " w/ "]
        .iter()
        .fold(s.as_str(), |acc, sep| acc.split(sep).next().unwrap_or(acc));
    let palabras: Vec<&str> = s.split_whitespace().collect();
    // "12th Gen", "2nd Gen", "3rd Gen"… al principio.
    let inicio = match palabras.as_slice() {
        [n, "Gen", ..] if n.chars().next().is_some_and(|c| c.is_ascii_digit()) => 2,
        _ => 0,
    };
    palabras[inicio..]
        .iter()
        .filter(|p| !matches!(**p, "CPU" | "Processor"))
        .filter(|p| !p.ends_with("-Core"))
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Sube por los procesos padre hasta encontrar algo que no sea un shell.
fn detectar_terminal(sys: &System) -> Option<String> {
    if let Ok(t) = env::var("TERM_PROGRAM") {
        return Some(t);
    }
    const NO: &[&str] = &[
        "bash",
        "zsh",
        "fish",
        "sh",
        "dash",
        "sudo",
        "su",
        "memtop",
        "cargo",
        "nu",
        "tmux: client",
    ];
    let mut pid = sysinfo::get_current_pid().ok()?;
    for _ in 0..10 {
        let p = sys.process(pid)?;
        let nombre = p.name().to_string_lossy().into_owned();
        if !NO.contains(&nombre.as_str()) && pid != sysinfo::get_current_pid().ok()? {
            // "tmux: server" → "tmux", "gnome-terminal-server" → "gnome-terminal".
            let limpio = nombre
                .split(':')
                .next()
                .unwrap_or(&nombre)
                .trim_end_matches("-server")
                .to_string();
            return Some(limpio);
        }
        pid = p.parent()?;
    }
    None
}

fn leer_meminfo() -> Memoria {
    let mut m = HashMap::new();
    if let Ok(t) = fs::read_to_string("/proc/meminfo") {
        for l in t.lines() {
            let mut it = l.split_whitespace();
            if let (Some(k), Some(v)) = (it.next(), it.next()) {
                m.insert(
                    k.trim_end_matches(':').to_string(),
                    v.parse::<u64>().unwrap_or(0) * 1024,
                );
            }
        }
    }
    let g = |k: &str| m.get(k).copied().unwrap_or(0);
    Memoria {
        total: g("MemTotal"),
        disponible: g("MemAvailable"),
        libre: g("MemFree"),
        buffers: g("Buffers"),
        cache: g("Cached") + g("SReclaimable"),
        compartida: g("Shmem"),
        swap_total: g("SwapTotal"),
        swap_libre: g("SwapFree"),
    }
}

fn detectar_discos() -> Vec<DiscoFisico> {
    let Ok(dir) = fs::read_dir("/sys/block") else {
        return Vec::new();
    };
    let mut v: Vec<DiscoFisico> = dir
        .flatten()
        .filter_map(|e| {
            let nombre = e.file_name().to_string_lossy().into_owned();
            if ["loop", "ram", "zram", "dm-", "md", "sr", "fd"]
                .iter()
                .any(|p| nombre.starts_with(p))
            {
                return None;
            }
            let base = e.path();
            let sectores: u64 = leer(base.join("size"))?.parse().ok()?;
            if sectores == 0 {
                return None;
            }
            let rotacional = leer(base.join("queue/rotational")).as_deref() == Some("1");
            let removible = leer(base.join("removable")).as_deref() == Some("1");
            let tipo = if nombre.starts_with("nvme") {
                "NVMe"
            } else if removible {
                "USB"
            } else if rotacional {
                "HDD"
            } else {
                "SSD"
            };
            Some(DiscoFisico {
                modelo: leer(base.join("device/model")),
                tamano: sectores * 512,
                tipo,
                temperatura: None,
                lectura_bps: 0.0,
                escritura_bps: 0.0,
                sectores_previos: None,
                nombre,
            })
        })
        .collect();
    v.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    v
}

fn actualizar_disco(d: &mut DiscoFisico, dt: f64) {
    let base = Path::new("/sys/block").join(&d.nombre);
    if let Some(stat) = leer(base.join("stat")) {
        let c: Vec<u64> = stat
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if c.len() >= 7 {
            let (r, w) = (c[2], c[6]);
            if let Some((pr, pw)) = d.sectores_previos {
                d.lectura_bps = r.saturating_sub(pr) as f64 * 512.0 / dt;
                d.escritura_bps = w.saturating_sub(pw) as f64 * 512.0 / dt;
            }
            d.sectores_previos = Some((r, w));
        }
    }
    // Temperatura: /sys/block/X/device/hwmon*/temp1_input (NVMe) o .../device/device/hwmon*.
    d.temperatura = ["device", "device/device"].iter().find_map(|sub| {
        fs::read_dir(base.join(sub)).ok()?.flatten().find_map(|e| {
            let n = e.file_name();
            if !n.to_string_lossy().starts_with("hwmon") {
                return None;
            }
            leer(e.path().join("temp1_input"))?
                .parse::<f32>()
                .ok()
                .map(|t| t / 1000.0)
        })
    });
}

fn leer_bateria() -> Option<Bateria> {
    let dir = fs::read_dir("/sys/class/power_supply").ok()?;
    dir.flatten().find_map(|e| {
        let p = e.path();
        if leer(p.join("type")).as_deref() != Some("Battery")
            || leer(p.join("scope")).as_deref() == Some("Device")
        {
            return None;
        }
        Some(Bateria {
            porcentaje: leer(p.join("capacity"))?.parse().ok()?,
            estado: EstadoBateria::desde_sys(&leer(p.join("status"))?),
        })
    })
}

pub fn fmt_bytes(b: u64) -> String {
    const U: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", U[i])
    } else {
        format!("{v:.1} {}", U[i])
    }
}

pub fn fmt_tasa(bps: f64) -> String {
    format!("{}/s", fmt_bytes(bps as u64))
}

pub fn fmt_duracion(s: u64) -> String {
    let (d, h, m) = (s / 86400, s % 86400 / 3600, s % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{m} min"),
        (0, _) => format!("{h} h {m} min"),
        _ => format!("{d} d {h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limpia_nombres_de_cpu() {
        assert_eq!(
            limpiar_cpu("11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz"),
            "Intel Core i7-1165G7"
        );
        assert_eq!(
            limpiar_cpu("Intel(R) Core(TM) i5-2500K CPU @ 3.30GHz"),
            "Intel Core i5-2500K"
        );
        assert_eq!(
            limpiar_cpu("AMD Ryzen 7 5800X 8-Core Processor"),
            "AMD Ryzen 7 5800X"
        );
        assert_eq!(
            limpiar_cpu("AMD Ryzen 5 5600G with Radeon Graphics"),
            "AMD Ryzen 5 5600G"
        );
        assert_eq!(
            limpiar_cpu("AMD Ryzen 7 PRO 7840U w/ Radeon 780M Graphics"),
            "AMD Ryzen 7 PRO 7840U"
        );
        assert_eq!(limpiar_cpu("Apple M2"), "Apple M2");
    }

    #[test]
    fn limpia_nombres_de_equipo() {
        assert_eq!(
            limpiar_modelo(Some("Fabricante S.A."), "Modelo X1 AB12_AB12").as_deref(),
            Some("Fabricante S.A. Modelo X1 AB12")
        );
        assert_eq!(
            limpiar_modelo(Some("ACME COMPUTER INC."), "ACME Book 14").as_deref(),
            Some("ACME Book 14")
        );
        assert_eq!(
            limpiar_modelo(Some("Default string"), "Torre 5").as_deref(),
            Some("Torre 5")
        );
        assert_eq!(limpiar_modelo(Some("ACME"), "System Product Name"), None);
        assert_eq!(limpiar_modelo(None, "  "), None);
    }

    #[test]
    fn formatea_bytes() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1536), "1.5 KiB");
        assert_eq!(fmt_bytes(250 * 1024 * 1024), "250 MiB");
        assert_eq!(fmt_bytes(31 * 1024 * 1024 * 1024), "31.0 GiB");
        assert_eq!(fmt_tasa(2048.0), "2.0 KiB/s");
    }

    #[test]
    fn formatea_duraciones() {
        assert_eq!(fmt_duracion(59), "0 min");
        assert_eq!(fmt_duracion(3 * 3600 + 5 * 60), "3 h 5 min");
        assert_eq!(fmt_duracion(2 * 86400 + 3600), "2 d 1 h 0 min");
    }

    #[test]
    fn agrupa_los_estados_de_proceso() {
        assert_eq!(
            EstadoProceso::from(ProcessStatus::Run),
            EstadoProceso::Ejecutando
        );
        assert_eq!(
            EstadoProceso::from(ProcessStatus::Sleep),
            EstadoProceso::Durmiendo
        );
        assert_eq!(
            EstadoProceso::from(ProcessStatus::UninterruptibleDiskSleep),
            EstadoProceso::EsperaDisco
        );
        assert_eq!(
            EstadoProceso::from(ProcessStatus::Zombie),
            EstadoProceso::Zombi
        );
        assert_eq!(
            EstadoProceso::from(ProcessStatus::Unknown(99)),
            EstadoProceso::Otro
        );
    }

    #[test]
    fn interpreta_el_estado_de_la_bateria() {
        assert_eq!(
            EstadoBateria::desde_sys("Charging"),
            EstadoBateria::Cargando
        );
        assert_eq!(
            EstadoBateria::desde_sys("Not charging"),
            EstadoBateria::SinCargar
        );
        assert_eq!(EstadoBateria::desde_sys("Full"), EstadoBateria::Completa);
        assert_eq!(EstadoBateria::desde_sys("???"), EstadoBateria::Desconocido);
    }

    #[test]
    fn la_memoria_usada_no_puede_ser_negativa() {
        let m = Memoria {
            total: 100,
            disponible: 150,
            libre: 0,
            buffers: 0,
            cache: 0,
            compartida: 0,
            swap_total: 0,
            swap_libre: 10,
        };
        assert_eq!(m.usada(), 0);
        assert_eq!(m.swap_usada(), 0);
    }
}
