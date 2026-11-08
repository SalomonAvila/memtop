//! GPUs read from `/sys/bus/pci`, without waking up the discrete GPU.
//!
//! - Name and vendor: PCI database embedded in the binary ([`crate::pci`]).
//! - AMD (`amdgpu`): usage and VRAM from `/sys`.
//! - Intel (`i915`/`xe`): current and maximum clock from `/sys`.
//! - NVIDIA (proprietary driver): `nvidia-smi`, which is installed with the driver.
//!
//! On laptops with hybrid graphics the NVIDIA GPU is usually suspended to save battery,
//! and querying it wakes it up. That is why it is only queried while [`QUERY_NVIDIA`] is
//! set (GPU tab visible), never while it is suspended, and from a separate thread so the
//! interface does not freeze.

use crate::{i18n, pci};

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

/// nvidia-smi is only called while this is `true` (GPU tab visible): querying it often
/// would keep the discrete GPU from suspending.
pub static QUERY_NVIDIA: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default)]
pub struct NvidiaData {
    pub usage: Option<f32>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub temperature: Option<f32>,
    pub power_w: Option<f32>,
    pub clock_mhz: Option<u32>,
}

pub struct Gpu {
    pub name: String,
    pub vendor: String,
    pub driver: Option<String>,
    pub address: String,
    path: PathBuf,
    /// `active`, `suspended`… (`/power/runtime_status`)
    pub power_state: Option<String>,
    pub usage: Option<f32>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub temperature: Option<f32>,
    pub power_w: Option<f32>,
    pub clock_mhz: Option<u32>,
    pub max_clock_mhz: Option<u32>,
    nvidia: Option<Arc<Mutex<Option<NvidiaData>>>>,
}

impl Gpu {
    pub fn is_suspended(&self) -> bool {
        self.power_state.as_deref() == Some("suspended")
    }
}

fn read_trimmed(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn detect() -> Vec<Gpu> {
    let Ok(dir) = fs::read_dir("/sys/bus/pci/devices") else {
        return Vec::new();
    };
    let mut gpus: Vec<Gpu> = dir
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            // Class 0x03xxxx = display controller.
            if !read_trimmed(path.join("class"))?.starts_with("0x03") {
                return None;
            }
            let address = e.file_name().to_string_lossy().into_owned();
            let vendor = read_trimmed(path.join("vendor")).and_then(|v| pci::parse_hex(&v));
            let device = read_trimmed(path.join("device")).and_then(|v| pci::parse_hex(&v));
            let driver = fs::read_link(path.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
            let vendor_name = vendor
                .and_then(pci::vendor_name)
                .unwrap_or_else(|| i18n::t().fabricante_desconocido.into());
            let name = vendor
                .zip(device)
                .and_then(|(v, d)| pci::device_name(v, d))
                .unwrap_or_else(|| {
                    format!(
                        "GPU {:04x}:{:04x}",
                        vendor.unwrap_or(0),
                        device.unwrap_or(0)
                    )
                });
            let is_nvidia = vendor == Some(0x10de) && driver.as_deref() == Some("nvidia");
            let nvidia = is_nvidia.then(|| {
                let data = Arc::new(Mutex::new(None));
                spawn_nvidia_thread(path.clone(), address.clone(), data.clone());
                data
            });
            Some(Gpu {
                name,
                vendor: vendor_name,
                driver,
                address,
                path,
                power_state: None,
                usage: None,
                vram_used: None,
                vram_total: None,
                temperature: None,
                power_w: None,
                clock_mhz: None,
                max_clock_mhz: None,
                nvidia,
            })
        })
        .collect();
    // Integrated GPU (bus 00) first.
    gpus.sort_by(|a, b| a.address.cmp(&b.address));
    refresh(&mut gpus);
    gpus
}

pub fn refresh(gpus: &mut [Gpu]) {
    for g in gpus {
        g.power_state = read_trimmed(g.path.join("power/runtime_status"));

        if let Some(data) = &g.nvidia {
            let d = data.lock().ok().and_then(|d| d.clone());
            let d = if g.is_suspended() { None } else { d };
            let d = d.unwrap_or_default();
            g.usage = d.usage;
            g.vram_used = d.vram_used;
            g.vram_total = d.vram_total;
            g.temperature = d.temperature;
            g.power_w = d.power_w;
            g.clock_mhz = d.clock_mhz;
            continue;
        }

        // AMD (amdgpu)
        g.usage = read_trimmed(g.path.join("gpu_busy_percent")).and_then(|s| s.parse().ok());
        g.vram_used = read_trimmed(g.path.join("mem_info_vram_used")).and_then(|s| s.parse().ok());
        g.vram_total =
            read_trimmed(g.path.join("mem_info_vram_total")).and_then(|s| s.parse().ok());

        // Intel (i915/xe): current and maximum clock in drm/cardN.
        if let Some(card) = fs::read_dir(g.path.join("drm")).ok().and_then(|d| {
            d.flatten().map(|e| e.path()).find(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("card"))
            })
        }) {
            let f = |n: &str| {
                read_trimmed(card.join(n))
                    .or_else(|| read_trimmed(card.join("gt/gt0").join(n.replace("gt_", "rps_"))))
                    .and_then(|s| s.parse().ok())
            };
            g.clock_mhz = f("gt_act_freq_mhz")
                .filter(|v| *v > 0)
                .or_else(|| f("gt_cur_freq_mhz"));
            g.max_clock_mhz = f("gt_max_freq_mhz");
        }

        // Temperature from the device's hwmon (amdgpu, nouveau…).
        g.temperature = fs::read_dir(g.path.join("hwmon")).ok().and_then(|d| {
            d.flatten().find_map(|e| {
                read_trimmed(e.path().join("temp1_input"))?
                    .parse::<f32>()
                    .ok()
                    .map(|t| t / 1000.0)
            })
        });
    }
}

fn spawn_nvidia_thread(path: PathBuf, address: String, data: Arc<Mutex<Option<NvidiaData>>>) {
    thread::spawn(move || {
        loop {
            if QUERY_NVIDIA.load(Ordering::Relaxed) {
                let active =
                    read_trimmed(path.join("power/runtime_status")).as_deref() != Some("suspended");
                let latest = if active {
                    query_nvidia_smi(&address)
                } else {
                    None
                };
                if let Ok(mut d) = data.lock() {
                    *d = latest;
                }
                thread::sleep(Duration::from_secs(2));
            } else {
                thread::sleep(Duration::from_millis(250));
            }
        }
    });
}

fn query_nvidia_smi(address: &str) -> Option<NvidiaData> {
    let out = Command::new("nvidia-smi")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .args([
            "-i",
            address,
            "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,clocks.gr",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_nvidia_smi(&String::from_utf8_lossy(&out.stdout))
}

/// Parses one CSV line from `nvidia-smi` (`"12, 301, 4096, 51, 9.87, 1170"`).
/// Unavailable fields (`[N/A]`) are left as `None`.
fn parse_nvidia_smi(text: &str) -> Option<NvidiaData> {
    let c: Vec<&str> = text.lines().next()?.split(',').map(str::trim).collect();
    let num = |i: usize| c.get(i).and_then(|v| v.parse::<f64>().ok());
    Some(NvidiaData {
        usage: num(0).map(|v| v as f32),
        vram_used: num(1).map(|v| (v * 1024.0 * 1024.0) as u64),
        vram_total: num(2).map(|v| (v * 1024.0 * 1024.0) as u64),
        temperature: num(3).map(|v| v as f32),
        power_w: num(4).map(|v| v as f32),
        clock_mhz: num(5).map(|v| v as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_output() {
        let d = parse_nvidia_smi("12, 301, 4096, 51, 9.87, 1170\n").unwrap();
        assert_eq!(d.usage, Some(12.0));
        assert_eq!(d.vram_used, Some(301 * 1024 * 1024));
        assert_eq!(d.vram_total, Some(4096 * 1024 * 1024));
        assert_eq!(d.temperature, Some(51.0));
        assert_eq!(d.clock_mhz, Some(1170));
    }

    #[test]
    fn unavailable_fields_are_empty() {
        let d = parse_nvidia_smi("[N/A], 301, 4096, 51, [N/A], 1170").unwrap();
        assert_eq!(d.usage, None);
        assert_eq!(d.power_w, None);
        assert_eq!(d.temperature, Some(51.0));
    }
}
