//! GPUs leídas de `/sys/bus/pci`, sin despertar la gráfica dedicada.
//!
//! - Nombre y fabricante: base PCI incluida en el binario ([`crate::pci`]).
//! - AMD (`amdgpu`): uso y VRAM desde `/sys`.
//! - Intel (`i915`/`xe`): frecuencia actual y máxima desde `/sys`.
//! - NVIDIA (driver propietario): `nvidia-smi`, que se instala junto con el driver.
//!
//! En portátiles con gráfica híbrida la NVIDIA suele estar suspendida para ahorrar
//! batería y consultarla la despierta. Por eso solo se pregunta mientras
//! [`CONSULTAR_NVIDIA`] está activo (pestaña GPU visible), nunca si está suspendida,
//! y desde un hilo aparte para no congelar la interfaz.

use crate::pci;

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

/// Solo se llama a nvidia-smi mientras esto sea `true` (pestaña GPU visible):
/// consultarla seguido impediría que la gráfica dedicada se suspenda.
pub static CONSULTAR_NVIDIA: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default)]
pub struct DatosNvidia {
    pub uso: Option<f32>,
    pub vram_usada: Option<u64>,
    pub vram_total: Option<u64>,
    pub temperatura: Option<f32>,
    pub consumo_w: Option<f32>,
    pub frecuencia_mhz: Option<u32>,
}

pub struct Gpu {
    pub nombre: String,
    pub fabricante: String,
    pub driver: Option<String>,
    pub direccion: String,
    ruta: PathBuf,
    /// `active`, `suspended`… (`/power/runtime_status`)
    pub estado: Option<String>,
    pub uso: Option<f32>,
    pub vram_usada: Option<u64>,
    pub vram_total: Option<u64>,
    pub temperatura: Option<f32>,
    pub consumo_w: Option<f32>,
    pub frecuencia_mhz: Option<u32>,
    pub frecuencia_max_mhz: Option<u32>,
    nvidia: Option<Arc<Mutex<Option<DatosNvidia>>>>,
}

impl Gpu {
    pub fn suspendida(&self) -> bool {
        self.estado.as_deref() == Some("suspended")
    }
}

fn leer(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn detectar() -> Vec<Gpu> {
    let Ok(dir) = fs::read_dir("/sys/bus/pci/devices") else {
        return Vec::new();
    };
    let mut gpus: Vec<Gpu> = dir
        .flatten()
        .filter_map(|e| {
            let ruta = e.path();
            // Clase 0x03xxxx = controlador de pantalla.
            if !leer(ruta.join("class"))?.starts_with("0x03") {
                return None;
            }
            let direccion = e.file_name().to_string_lossy().into_owned();
            let vendor = leer(ruta.join("vendor")).and_then(|v| pci::parse_hex(&v));
            let device = leer(ruta.join("device")).and_then(|v| pci::parse_hex(&v));
            let driver = fs::read_link(ruta.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
            let fabricante = vendor
                .and_then(pci::vendor_name)
                .unwrap_or_else(|| "Desconocido".into());
            let nombre = vendor
                .zip(device)
                .and_then(|(v, d)| pci::device_name(v, d))
                .unwrap_or_else(|| {
                    format!(
                        "GPU {:04x}:{:04x}",
                        vendor.unwrap_or(0),
                        device.unwrap_or(0)
                    )
                });
            let es_nvidia = vendor == Some(0x10de) && driver.as_deref() == Some("nvidia");
            let nvidia = es_nvidia.then(|| {
                let datos = Arc::new(Mutex::new(None));
                lanzar_hilo_nvidia(ruta.clone(), direccion.clone(), datos.clone());
                datos
            });
            Some(Gpu {
                nombre,
                fabricante,
                driver,
                direccion,
                ruta,
                estado: None,
                uso: None,
                vram_usada: None,
                vram_total: None,
                temperatura: None,
                consumo_w: None,
                frecuencia_mhz: None,
                frecuencia_max_mhz: None,
                nvidia,
            })
        })
        .collect();
    // Integrada (bus 00) primero.
    gpus.sort_by(|a, b| a.direccion.cmp(&b.direccion));
    actualizar(&mut gpus);
    gpus
}

pub fn actualizar(gpus: &mut [Gpu]) {
    for g in gpus {
        g.estado = leer(g.ruta.join("power/runtime_status"));

        if let Some(datos) = &g.nvidia {
            let d = datos.lock().ok().and_then(|d| d.clone());
            let d = if g.suspendida() { None } else { d };
            let d = d.unwrap_or_default();
            g.uso = d.uso;
            g.vram_usada = d.vram_usada;
            g.vram_total = d.vram_total;
            g.temperatura = d.temperatura;
            g.consumo_w = d.consumo_w;
            g.frecuencia_mhz = d.frecuencia_mhz;
            continue;
        }

        // AMD (amdgpu)
        g.uso = leer(g.ruta.join("gpu_busy_percent")).and_then(|s| s.parse().ok());
        g.vram_usada = leer(g.ruta.join("mem_info_vram_used")).and_then(|s| s.parse().ok());
        g.vram_total = leer(g.ruta.join("mem_info_vram_total")).and_then(|s| s.parse().ok());

        // Intel (i915/xe): frecuencia actual y máxima en drm/cardN.
        if let Some(card) = fs::read_dir(g.ruta.join("drm")).ok().and_then(|d| {
            d.flatten().map(|e| e.path()).find(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("card"))
            })
        }) {
            let f = |n: &str| {
                leer(card.join(n))
                    .or_else(|| leer(card.join("gt/gt0").join(n.replace("gt_", "rps_"))))
                    .and_then(|s| s.parse().ok())
            };
            g.frecuencia_mhz = f("gt_act_freq_mhz")
                .filter(|v| *v > 0)
                .or_else(|| f("gt_cur_freq_mhz"));
            g.frecuencia_max_mhz = f("gt_max_freq_mhz");
        }

        // Temperatura desde hwmon del dispositivo (amdgpu, nouveau…).
        g.temperatura = fs::read_dir(g.ruta.join("hwmon")).ok().and_then(|d| {
            d.flatten().find_map(|e| {
                leer(e.path().join("temp1_input"))?
                    .parse::<f32>()
                    .ok()
                    .map(|t| t / 1000.0)
            })
        });
    }
}

fn lanzar_hilo_nvidia(ruta: PathBuf, direccion: String, datos: Arc<Mutex<Option<DatosNvidia>>>) {
    thread::spawn(move || {
        loop {
            if CONSULTAR_NVIDIA.load(Ordering::Relaxed) {
                let activa =
                    leer(ruta.join("power/runtime_status")).as_deref() != Some("suspended");
                let nuevo = if activa {
                    consultar_nvidia_smi(&direccion)
                } else {
                    None
                };
                if let Ok(mut d) = datos.lock() {
                    *d = nuevo;
                }
                thread::sleep(Duration::from_secs(2));
            } else {
                thread::sleep(Duration::from_millis(250));
            }
        }
    });
}

fn consultar_nvidia_smi(direccion: &str) -> Option<DatosNvidia> {
    let out = Command::new("nvidia-smi")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .args([
            "-i",
            direccion,
            "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,clocks.gr",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    interpretar_nvidia_smi(&String::from_utf8_lossy(&out.stdout))
}

/// Interpreta una línea CSV de `nvidia-smi` (`"12, 301, 4096, 51, 9.87, 1170"`).
/// Los campos no disponibles (`[N/A]`) quedan en `None`.
fn interpretar_nvidia_smi(txt: &str) -> Option<DatosNvidia> {
    let c: Vec<&str> = txt.lines().next()?.split(',').map(str::trim).collect();
    let num = |i: usize| c.get(i).and_then(|v| v.parse::<f64>().ok());
    Some(DatosNvidia {
        uso: num(0).map(|v| v as f32),
        vram_usada: num(1).map(|v| (v * 1024.0 * 1024.0) as u64),
        vram_total: num(2).map(|v| (v * 1024.0 * 1024.0) as u64),
        temperatura: num(3).map(|v| v as f32),
        consumo_w: num(4).map(|v| v as f32),
        frecuencia_mhz: num(5).map(|v| v as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_la_salida_de_nvidia_smi() {
        let d = interpretar_nvidia_smi("12, 301, 4096, 51, 9.87, 1170\n").unwrap();
        assert_eq!(d.uso, Some(12.0));
        assert_eq!(d.vram_usada, Some(301 * 1024 * 1024));
        assert_eq!(d.vram_total, Some(4096 * 1024 * 1024));
        assert_eq!(d.temperatura, Some(51.0));
        assert_eq!(d.frecuencia_mhz, Some(1170));
    }

    #[test]
    fn los_campos_no_disponibles_quedan_vacios() {
        let d = interpretar_nvidia_smi("[N/A], 301, 4096, 51, [N/A], 1170").unwrap();
        assert_eq!(d.uso, None);
        assert_eq!(d.consumo_w, None);
        assert_eq!(d.temperatura, Some(51.0));
    }
}
