//! Reading the machine's information.
//!
//! Static data (model, OS, CPU…) is read once at startup; dynamic data (CPU usage, memory,
//! disks, network, processes, sensors) on every call to [`Monitor::refresh`].
//!
//! Sources: the `sysinfo` crate for CPU, processes, network, partitions and sensors, and
//! direct reads of `/proc/meminfo`, `/sys/block`, `/sys/class/power_supply`,
//! `/sys/class/dmi/id` and `/etc/os-release` for the rest. It needs no root privileges.

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

/// Seconds of history kept for the charts.
pub const HISTORY_LEN: usize = 120;

pub struct Info {
    pub user: String,
    pub host: String,
    pub model: Option<String>,
    pub os: String,
    pub kernel: String,
    pub desktop: Option<String>,
    pub shell: Option<String>,
    pub terminal: Option<String>,
    pub cpu_model: String,
    pub physical_cores: Option<usize>,
    pub threads: usize,
    pub arch: String,
}

pub struct Memory {
    pub total: u64,
    pub available: u64,
    pub free: u64,
    pub buffers: u64,
    pub cache: u64,
    pub shared: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

impl Memory {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
    pub fn swap_used(&self) -> u64 {
        self.swap_total.saturating_sub(self.swap_free)
    }
}

/// Physical disk (from /sys/block).
pub struct PhysicalDisk {
    pub name: String,
    pub model: Option<String>,
    pub size: u64,
    pub kind: &'static str,
    pub temperature: Option<f32>,
    pub read_bps: f64,
    pub write_bps: f64,
    prev_sectors: Option<(u64, u64)>,
}

/// Mounted file system.
pub struct Mount {
    pub device: String,
    pub mount_point: String,
    pub fs: String,
    pub total: u64,
    pub used: u64,
}

pub struct Interface {
    pub name: String,
    /// Backed by hardware (not docker0, veth, a virtual bridge…).
    pub physical: bool,
    pub mac: String,
    pub ips: Vec<String>,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

pub struct Process {
    pub pid: u32,
    pub user: String,
    pub name: String,
    pub command: String,
    pub cpu: f32,
    pub memory: u64,
    pub state: ProcessState,
    /// Seconds of CPU used (htop's TIME+).
    pub cpu_time: u64,
}

/// Process state, grouping `sysinfo`'s states into short, translatable names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Sleeping,
    Idle,
    /// Waiting for disk I/O without being interruptible (state `D`).
    DiskWait,
    Stopped,
    Zombie,
    Other,
}

impl From<ProcessStatus> for ProcessState {
    fn from(s: ProcessStatus) -> Self {
        match s {
            ProcessStatus::Run | ProcessStatus::Waking => Self::Running,
            ProcessStatus::Sleep => Self::Sleeping,
            ProcessStatus::Idle | ProcessStatus::Parked => Self::Idle,
            ProcessStatus::UninterruptibleDiskSleep => Self::DiskWait,
            ProcessStatus::Stop | ProcessStatus::Tracing => Self::Stopped,
            ProcessStatus::Zombie | ProcessStatus::Dead => Self::Zombie,
            _ => Self::Other,
        }
    }
}

/// Why a signal could not be sent to a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalError {
    NoSuchProcess,
    PermissionDenied,
    Unsupported,
}

pub struct Battery {
    pub percent: u8,
    pub state: BatteryState,
}

/// Charge state, from `/sys/class/power_supply/*/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    /// Plugged in but not charging (e.g. because of a charge limit).
    NotCharging,
    Unknown,
}

impl BatteryState {
    fn from_sys(text: &str) -> Self {
        match text {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }
}

pub struct Temperature {
    pub label: String,
    pub celsius: f32,
    pub critical: Option<f32>,
}

pub struct Monitor {
    sys: System,
    sys_disks: Disks,
    networks: Networks,
    components: Components,
    user_names: HashMap<u32, String>,
    last_refresh: Instant,

    pub info: Info,
    pub gpus: Vec<Gpu>,

    pub cpu_total: f32,
    pub cpu_per_thread: Vec<f32>,
    pub cpu_freq_mhz: Vec<u64>,
    pub load: (f64, f64, f64),
    pub uptime: u64,
    pub memory: Memory,
    pub disks: Vec<PhysicalDisk>,
    pub mounts: Vec<Mount>,
    pub interfaces: Vec<Interface>,
    pub processes: Vec<Process>,
    pub temperatures: Vec<Temperature>,
    pub battery: Option<Battery>,

    pub hist_cpu: VecDeque<u64>,
    pub hist_ram: VecDeque<u64>,
    pub hist_rx: VecDeque<u64>,
    pub hist_tx: VecDeque<u64>,
}

impl Monitor {
    /// Takes the first full reading. It only fails if the system does not expose `/proc`.
    pub fn new() -> Result<Self> {
        let mut sys = System::new_all().context(i18n::t().system_no_info)?;
        sys.refresh_all();
        let info = read_info(&sys);
        let user_names = Users::new_with_refreshed_list()
            .map(|u| u.iter().map(|u| (**u.id(), u.name().to_string())).collect())
            .unwrap_or_default();

        let mut m = Self {
            sys_disks: Disks::new_with_refreshed_list().context(i18n::t().system_no_disks)?,
            networks: Networks::new_with_refreshed_list().context(i18n::t().system_no_network)?,
            components: Components::new_with_refreshed_list()
                .context(i18n::t().system_no_sensors)?,
            user_names,
            last_refresh: Instant::now(),
            info,
            gpus: gpu::detect(),
            sys,
            cpu_total: 0.0,
            cpu_per_thread: Vec::new(),
            cpu_freq_mhz: Vec::new(),
            load: (0.0, 0.0, 0.0),
            uptime: 0,
            memory: read_meminfo(),
            disks: detect_disks(),
            mounts: Vec::new(),
            interfaces: Vec::new(),
            processes: Vec::new(),
            temperatures: Vec::new(),
            battery: None,
            hist_cpu: VecDeque::with_capacity(HISTORY_LEN),
            hist_ram: VecDeque::with_capacity(HISTORY_LEN),
            hist_rx: VecDeque::with_capacity(HISTORY_LEN),
            hist_tx: VecDeque::with_capacity(HISTORY_LEN),
        };
        m.refresh();
        Ok(m)
    }

    pub fn refresh(&mut self) {
        let dt = self.last_refresh.elapsed().as_secs_f64().max(0.001);
        self.last_refresh = Instant::now();

        self.sys.refresh_cpu_all();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.cpu_total = self.sys.global_cpu_usage();
        self.cpu_per_thread = self.sys.cpus().iter().map(|c| c.usage()).collect();
        self.cpu_freq_mhz = self.sys.cpus().iter().map(|c| c.frequency()).collect();
        if let Ok(l) = System::load_average() {
            self.load = (l.one, l.five, l.fifteen);
        }
        self.uptime = System::uptime().unwrap_or(0);
        self.memory = read_meminfo();

        // Mounts (without repeating the same device, e.g. btrfs subvolumes).
        self.sys_disks.refresh(true);
        let mut seen = HashMap::new();
        let mut mounts: Vec<Mount> = Vec::new();
        for d in &self.sys_disks {
            let device = d.name().to_string_lossy().into_owned();
            let mount_point = d.mount_point().to_string_lossy().into_owned();
            if d.total_space() == 0
                || mount_point.starts_with("/run/")
                || mount_point.starts_with("/snap/")
            {
                continue;
            }
            let m = Mount {
                fs: d.file_system().to_string_lossy().into_owned(),
                total: d.total_space(),
                used: d.total_space().saturating_sub(d.available_space()),
                device: device.clone(),
                mount_point,
            };
            match seen.get(&device) {
                Some(&i) => {
                    let existing: &mut Mount = &mut mounts[i];
                    if m.mount_point.len() < existing.mount_point.len() {
                        *existing = m;
                    }
                }
                None => {
                    seen.insert(device, mounts.len());
                    mounts.push(m);
                }
            }
        }
        mounts.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        self.mounts = mounts;

        for d in &mut self.disks {
            refresh_disk(d, dt);
        }

        // Network
        self.networks.refresh(true);
        let mut interfaces: Vec<Interface> = self
            .networks
            .iter()
            .filter(|(n, _)| n.to_str() != Some("lo"))
            .map(|(n, d)| Interface {
                physical: Path::new("/sys/class/net").join(n).join("device").exists(),
                name: n.to_string_lossy().into_owned(),
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
        // Interfaces with an IP (the ones in use) first.
        interfaces.sort_by(|a, b| {
            (!a.physical, a.ips.is_empty(), &a.name).cmp(&(!b.physical, b.ips.is_empty(), &b.name))
        });
        self.interfaces = interfaces;

        // Processes
        let threads = self.sys.cpus().len().max(1) as f32;
        self.processes = self
            .sys
            .processes()
            .values()
            // User threads: htop hides them by default.
            .filter(|p| p.thread_kind().is_none())
            .map(|p| {
                let uid = p.user_id().map(|u| **u);
                let command = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                Process {
                    pid: p.pid().as_u32(),
                    user: uid
                        .and_then(|u| self.user_names.get(&u).cloned())
                        .unwrap_or_else(|| uid.map(|u| u.to_string()).unwrap_or_default()),
                    name: p.name().to_string_lossy().into_owned(),
                    command,
                    cpu: p.cpu_usage().min(100.0 * threads),
                    memory: p.memory(),
                    state: ProcessState::from(p.status()),
                    cpu_time: p.accumulated_cpu_time() / 1000,
                }
            })
            .collect();

        // Temperatures
        self.components.refresh(true);
        self.temperatures = self
            .components
            .iter()
            .filter_map(|c| {
                Some(Temperature {
                    label: c.label().to_string(),
                    celsius: c.temperature().filter(|t| *t > 0.0)?,
                    critical: c.critical(),
                })
            })
            .collect();

        self.battery = read_battery();
        gpu::refresh(&mut self.gpus);

        let push = |h: &mut VecDeque<u64>, v: u64| {
            if h.len() == HISTORY_LEN {
                h.pop_front();
            }
            h.push_back(v);
        };
        push(&mut self.hist_cpu, self.cpu_total.round() as u64);
        let ram_pct = self.memory.used() * 100 / self.memory.total.max(1);
        push(&mut self.hist_ram, ram_pct);
        push(
            &mut self.hist_rx,
            self.interfaces.iter().map(|i| i.rx_bps).sum::<f64>() as u64,
        );
        push(
            &mut self.hist_tx,
            self.interfaces.iter().map(|i| i.tx_bps).sum::<f64>() as u64,
        );
    }

    /// Sends SIGTERM (or SIGKILL if `force`) to a process.
    pub fn signal(&self, pid: u32, force: bool) -> Result<(), SignalError> {
        let p = self
            .sys
            .process(Pid::from_u32(pid))
            .ok_or(SignalError::NoSuchProcess)?;
        let s = if force {
            sysinfo::Signal::Kill
        } else {
            sysinfo::Signal::Term
        };
        match p.kill_with(s) {
            Some(true) => Ok(()),
            Some(false) => Err(SignalError::PermissionDenied),
            None => Err(SignalError::Unsupported),
        }
    }

    pub fn cpu_temperature(&self) -> Option<f32> {
        self.temperatures
            .iter()
            .find(|t| {
                t.label.contains("Package") || t.label.contains("Tctl") || t.label.contains("Tdie")
            })
            .or_else(|| {
                self.temperatures
                    .iter()
                    .find(|t| t.label.starts_with("coretemp"))
            })
            .map(|t| t.celsius)
    }
}

fn read_trimmed(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_info(sys: &System) -> Info {
    let model = read_trimmed("/sys/class/dmi/id/product_name")
        .and_then(|p| clean_model(read_trimmed("/sys/class/dmi/id/sys_vendor").as_deref(), &p));
    let os = fs::read_to_string("/etc/os-release")
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
    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .ok()
        // Ubuntu uses "ubuntu:GNOME", Pop!_OS "pop:GNOME": keep the actual desktop.
        .map(|d| d.rsplit(':').next().unwrap_or(&d).to_string())
        .filter(|d| !d.is_empty())
        .map(|d| match env::var("XDG_SESSION_TYPE") {
            Ok(t) => format!("{d} ({t})"),
            Err(_) => d,
        });

    Info {
        user: env::var("USER")
            .ok()
            .or_else(|| {
                // Without $USER (cron, containers…): look the name up by the process UID.
                let pid = sysinfo::get_current_pid().ok()?;
                let uid = sys.process(pid)?.user_id()?.clone();
                Users::new_with_refreshed_list()
                    .ok()?
                    .get_user_by_id(&uid)
                    .map(|u| u.name().to_string())
            })
            .unwrap_or_else(|| "?".into()),
        host: System::host_name().unwrap_or_else(|_| "?".into()),
        model,
        os,
        kernel: System::kernel_version().unwrap_or_default(),
        desktop,
        shell,
        terminal: detect_terminal(sys),
        cpu_model: sys
            .cpus()
            .first()
            .map(|c| clean_cpu(c.brand()))
            .unwrap_or_else(|| "?".into()),
        physical_cores: System::physical_core_count().ok(),
        threads: sys.cpus().len(),
        arch: System::cpu_arch(),
    }
}

/// Machine name from DMI, without repetitions or placeholder text.
///
/// - `("Vendor Ltd.", "Model X1 AB12_AB12")` → `"Vendor Ltd. Model X1 AB12"`
/// - `("ACME COMPUTER INC.", "ACME Book 14")` → `"ACME Book 14"` (the product already names
///   the brand)
fn clean_model(vendor: Option<&str>, product: &str) -> Option<String> {
    const PLACEHOLDERS: &[&str] = &[
        "To Be Filled",
        "To be filled",
        "System Product Name",
        "Default string",
        "Not Applicable",
    ];
    let product = product.trim();
    if product.is_empty() || PLACEHOLDERS.iter().any(|r| product.contains(r)) {
        return None;
    }
    // Some vendors repeat the model code: "... AB12_AB12".
    let product = match product.rsplit_once(' ') {
        Some((start, code)) => match code.split_once('_') {
            Some((a, b)) if a == b => format!("{start} {a}"),
            _ => product.to_string(),
        },
        None => product.to_string(),
    };
    let first_word = product.split(' ').next().unwrap_or("").to_lowercase();
    match vendor.map(str::trim) {
        Some(v)
            if !v.is_empty()
                && !PLACEHOLDERS.iter().any(|r| v.contains(r))
                && !v.to_lowercase().starts_with(&first_word) =>
        {
            Some(format!("{v} {product}"))
        }
        _ => Some(product),
    }
}

/// CPU name without trademarks or leftover text.
///
/// - `"11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz"` → `"Intel Core i7-1165G7"`
/// - `"AMD Ryzen 7 5800X 8-Core Processor"` → `"AMD Ryzen 7 5800X"`
fn clean_cpu(s: &str) -> String {
    let s = s.replace("(R)", "").replace("(TM)", "").replace("(tm)", "");
    // Whatever follows " @ " (frequency) or " with "/" w/ " (integrated graphics) is dropped.
    let s = [" @ ", " with ", " w/ "]
        .iter()
        .fold(s.as_str(), |acc, sep| acc.split(sep).next().unwrap_or(acc));
    let words: Vec<&str> = s.split_whitespace().collect();
    // "12th Gen", "2nd Gen", "3rd Gen"… at the start.
    let start = match words.as_slice() {
        [n, "Gen", ..] if n.chars().next().is_some_and(|c| c.is_ascii_digit()) => 2,
        _ => 0,
    };
    words[start..]
        .iter()
        .filter(|p| !matches!(**p, "CPU" | "Processor"))
        .filter(|p| !p.ends_with("-Core"))
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Walks up the parent processes until it finds something that is not a shell.
fn detect_terminal(sys: &System) -> Option<String> {
    if let Ok(t) = env::var("TERM_PROGRAM") {
        return Some(t);
    }
    const SKIP: &[&str] = &[
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
        let name = p.name().to_string_lossy().into_owned();
        if !SKIP.contains(&name.as_str()) && pid != sysinfo::get_current_pid().ok()? {
            // "tmux: server" → "tmux", "gnome-terminal-server" → "gnome-terminal".
            let clean = name
                .split(':')
                .next()
                .unwrap_or(&name)
                .trim_end_matches("-server")
                .to_string();
            return Some(clean);
        }
        pid = p.parent()?;
    }
    None
}

fn read_meminfo() -> Memory {
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
    Memory {
        total: g("MemTotal"),
        available: g("MemAvailable"),
        free: g("MemFree"),
        buffers: g("Buffers"),
        cache: g("Cached") + g("SReclaimable"),
        shared: g("Shmem"),
        swap_total: g("SwapTotal"),
        swap_free: g("SwapFree"),
    }
}

fn detect_disks() -> Vec<PhysicalDisk> {
    let Ok(dir) = fs::read_dir("/sys/block") else {
        return Vec::new();
    };
    let mut v: Vec<PhysicalDisk> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if ["loop", "ram", "zram", "dm-", "md", "sr", "fd"]
                .iter()
                .any(|p| name.starts_with(p))
            {
                return None;
            }
            let base = e.path();
            let sectors: u64 = read_trimmed(base.join("size"))?.parse().ok()?;
            if sectors == 0 {
                return None;
            }
            let rotational = read_trimmed(base.join("queue/rotational")).as_deref() == Some("1");
            let removable = read_trimmed(base.join("removable")).as_deref() == Some("1");
            let kind = if name.starts_with("nvme") {
                "NVMe"
            } else if removable {
                "USB"
            } else if rotational {
                "HDD"
            } else {
                "SSD"
            };
            Some(PhysicalDisk {
                model: read_trimmed(base.join("device/model")),
                size: sectors * 512,
                kind,
                temperature: None,
                read_bps: 0.0,
                write_bps: 0.0,
                prev_sectors: None,
                name,
            })
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

fn refresh_disk(d: &mut PhysicalDisk, dt: f64) {
    let base = Path::new("/sys/block").join(&d.name);
    if let Some(stat) = read_trimmed(base.join("stat")) {
        let c: Vec<u64> = stat
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if c.len() >= 7 {
            let (r, w) = (c[2], c[6]);
            if let Some((pr, pw)) = d.prev_sectors {
                d.read_bps = r.saturating_sub(pr) as f64 * 512.0 / dt;
                d.write_bps = w.saturating_sub(pw) as f64 * 512.0 / dt;
            }
            d.prev_sectors = Some((r, w));
        }
    }
    // Temperature: /sys/block/X/device/hwmon*/temp1_input (NVMe) or .../device/device/hwmon*.
    d.temperature = ["device", "device/device"].iter().find_map(|sub| {
        fs::read_dir(base.join(sub)).ok()?.flatten().find_map(|e| {
            let n = e.file_name();
            if !n.to_string_lossy().starts_with("hwmon") {
                return None;
            }
            read_trimmed(e.path().join("temp1_input"))?
                .parse::<f32>()
                .ok()
                .map(|t| t / 1000.0)
        })
    });
}

fn read_battery() -> Option<Battery> {
    let dir = fs::read_dir("/sys/class/power_supply").ok()?;
    dir.flatten().find_map(|e| {
        let p = e.path();
        if read_trimmed(p.join("type")).as_deref() != Some("Battery")
            || read_trimmed(p.join("scope")).as_deref() == Some("Device")
        {
            return None;
        }
        Some(Battery {
            percent: read_trimmed(p.join("capacity"))?.parse().ok()?,
            state: BatteryState::from_sys(&read_trimmed(p.join("status"))?),
        })
    })
}

pub fn fmt_bytes(b: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn fmt_rate(bps: f64) -> String {
    format!("{}/s", fmt_bytes(bps as u64))
}

pub fn fmt_duration(s: u64) -> String {
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
    fn cleans_cpu_names() {
        assert_eq!(
            clean_cpu("11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz"),
            "Intel Core i7-1165G7"
        );
        assert_eq!(
            clean_cpu("Intel(R) Core(TM) i5-2500K CPU @ 3.30GHz"),
            "Intel Core i5-2500K"
        );
        assert_eq!(
            clean_cpu("AMD Ryzen 7 5800X 8-Core Processor"),
            "AMD Ryzen 7 5800X"
        );
        assert_eq!(
            clean_cpu("AMD Ryzen 5 5600G with Radeon Graphics"),
            "AMD Ryzen 5 5600G"
        );
        assert_eq!(
            clean_cpu("AMD Ryzen 7 PRO 7840U w/ Radeon 780M Graphics"),
            "AMD Ryzen 7 PRO 7840U"
        );
        assert_eq!(clean_cpu("Apple M2"), "Apple M2");
    }

    #[test]
    fn cleans_machine_names() {
        assert_eq!(
            clean_model(Some("Vendor Ltd."), "Model X1 AB12_AB12").as_deref(),
            Some("Vendor Ltd. Model X1 AB12")
        );
        assert_eq!(
            clean_model(Some("ACME COMPUTER INC."), "ACME Book 14").as_deref(),
            Some("ACME Book 14")
        );
        assert_eq!(
            clean_model(Some("Default string"), "Tower 5").as_deref(),
            Some("Tower 5")
        );
        assert_eq!(clean_model(Some("ACME"), "System Product Name"), None);
        assert_eq!(clean_model(None, "  "), None);
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1536), "1.5 KiB");
        assert_eq!(fmt_bytes(250 * 1024 * 1024), "250 MiB");
        assert_eq!(fmt_bytes(31 * 1024 * 1024 * 1024), "31.0 GiB");
        assert_eq!(fmt_rate(2048.0), "2.0 KiB/s");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(fmt_duration(59), "0 min");
        assert_eq!(fmt_duration(3 * 3600 + 5 * 60), "3 h 5 min");
        assert_eq!(fmt_duration(2 * 86400 + 3600), "2 d 1 h 0 min");
    }

    #[test]
    fn groups_process_states() {
        assert_eq!(
            ProcessState::from(ProcessStatus::Run),
            ProcessState::Running
        );
        assert_eq!(
            ProcessState::from(ProcessStatus::Sleep),
            ProcessState::Sleeping
        );
        assert_eq!(
            ProcessState::from(ProcessStatus::UninterruptibleDiskSleep),
            ProcessState::DiskWait
        );
        assert_eq!(
            ProcessState::from(ProcessStatus::Zombie),
            ProcessState::Zombie
        );
        assert_eq!(
            ProcessState::from(ProcessStatus::Unknown(99)),
            ProcessState::Other
        );
    }

    #[test]
    fn parses_the_battery_state() {
        assert_eq!(BatteryState::from_sys("Charging"), BatteryState::Charging);
        assert_eq!(
            BatteryState::from_sys("Not charging"),
            BatteryState::NotCharging
        );
        assert_eq!(BatteryState::from_sys("Full"), BatteryState::Full);
        assert_eq!(BatteryState::from_sys("???"), BatteryState::Unknown);
    }

    #[test]
    fn used_memory_cannot_be_negative() {
        let m = Memory {
            total: 100,
            available: 150,
            free: 0,
            buffers: 0,
            cache: 0,
            shared: 0,
            swap_total: 0,
            swap_free: 10,
        };
        assert_eq!(m.used(), 0);
        assert_eq!(m.swap_used(), 0);
    }
}
