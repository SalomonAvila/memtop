//! Nombres de GPU a partir de sus identificadores PCI, sin depender de `lspci`.
//!
//! El binario incluye `data/pci-gpu.ids`, un extracto de la base pública
//! [pci.ids](https://pci-ids.ucw.cz) con solo los fabricantes de gráficas. Se
//! regenera con `scripts/update-pci-ids.sh`.

use std::{collections::HashMap, sync::OnceLock};

const BASE: &str = include_str!("../data/pci-gpu.ids");

struct Fabricante {
    nombre: &'static str,
    dispositivos: HashMap<u16, &'static str>,
}

fn base() -> &'static HashMap<u16, Fabricante> {
    static BASE_PCI: OnceLock<HashMap<u16, Fabricante>> = OnceLock::new();
    BASE_PCI.get_or_init(|| interpretar(BASE))
}

/// Interpreta el formato de pci.ids:
///
/// ```text
/// 10de  NVIDIA Corporation
/// <TAB>2504  GA106 [GeForce RTX 3060 Lite Hash Rate]
/// ```
fn interpretar(texto: &'static str) -> HashMap<u16, Fabricante> {
    let mut mapa: HashMap<u16, Fabricante> = HashMap::new();
    let mut actual: Option<u16> = None;
    for linea in texto.lines() {
        if linea.starts_with('#') || linea.trim().is_empty() || linea.starts_with("\t\t") {
            continue;
        }
        let (dispositivo, resto) = match linea.strip_prefix('\t') {
            Some(r) => (true, r),
            None => (false, linea),
        };
        let Some((id, nombre)) = resto.split_once("  ") else {
            continue;
        };
        let Ok(id) = u16::from_str_radix(id.trim(), 16) else {
            continue;
        };
        let nombre = nombre.trim();
        if dispositivo {
            if let Some(f) = actual.and_then(|v| mapa.get_mut(&v)) {
                f.dispositivos.insert(id, nombre);
            }
        } else {
            actual = Some(id);
            mapa.insert(
                id,
                Fabricante {
                    nombre,
                    dispositivos: HashMap::new(),
                },
            );
        }
    }
    mapa
}

/// Nombre corto del fabricante: `"NVIDIA Corporation"` → `"NVIDIA"`.
pub fn fabricante(vendor: u16) -> Option<String> {
    let nombre = base().get(&vendor)?.nombre;
    Some(acortar_fabricante(nombre))
}

/// Nombre comercial del dispositivo: `"GA106 [GeForce RTX 3060]"` → `"GeForce RTX 3060"`.
pub fn dispositivo(vendor: u16, device: u16) -> Option<String> {
    let nombre = base().get(&vendor)?.dispositivos.get(&device)?;
    Some(nombre_comercial(nombre))
}

fn acortar_fabricante(nombre: &str) -> String {
    if nombre.contains("AMD") || nombre.contains("ATI") {
        return "AMD".into();
    }
    let mut n = nombre.to_string();
    for sufijo in [
        " Corporation",
        " Corp.",
        ", Inc.",
        " Inc.",
        " Ltd.",
        " Technology",
    ] {
        n = n.replace(sufijo, "");
    }
    n.trim().to_string()
}

/// Lo que va entre corchetes es el nombre de venta; fuera, el nombre en clave del chip.
fn nombre_comercial(nombre: &str) -> String {
    match (nombre.find('['), nombre.rfind(']')) {
        (Some(a), Some(b)) if b > a + 1 => nombre[a + 1..b].to_string(),
        _ => nombre.to_string(),
    }
}

/// Lee un identificador de `/sys` (`"0x10de"`).
pub fn leer_hex(texto: &str) -> Option<u16> {
    u16::from_str_radix(texto.trim().trim_start_matches("0x"), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_base_incluida_tiene_los_fabricantes_principales() {
        assert_eq!(fabricante(0x10de).as_deref(), Some("NVIDIA"));
        assert_eq!(fabricante(0x1002).as_deref(), Some("AMD"));
        assert_eq!(fabricante(0x8086).as_deref(), Some("Intel"));
    }

    #[test]
    fn interpreta_fabricantes_y_dispositivos() {
        let base = interpretar(
            "# comentario\n10de  NVIDIA Corporation\n\t2504  GA106 [GeForce RTX 3060]\n\t\t1043 8851  subsistema\nabcd  Otro\n",
        );
        assert_eq!(
            base[&0x10de].dispositivos[&0x2504],
            "GA106 [GeForce RTX 3060]"
        );
        assert_eq!(base[&0xabcd].nombre, "Otro");
        assert!(base[&0xabcd].dispositivos.is_empty());
    }

    #[test]
    fn extrae_el_nombre_comercial() {
        assert_eq!(
            nombre_comercial("GA106 [GeForce RTX 3060]"),
            "GeForce RTX 3060"
        );
        assert_eq!(nombre_comercial("Kaveri"), "Kaveri");
        assert_eq!(nombre_comercial("Raro []"), "Raro []");
    }

    #[test]
    fn acorta_fabricantes() {
        assert_eq!(
            acortar_fabricante("Advanced Micro Devices, Inc. [AMD/ATI]"),
            "AMD"
        );
        assert_eq!(acortar_fabricante("Intel Corporation"), "Intel");
        assert_eq!(
            acortar_fabricante("Matrox Electronics Systems Ltd."),
            "Matrox Electronics Systems"
        );
    }

    #[test]
    fn lee_ids_de_sys() {
        assert_eq!(leer_hex("0x10de\n"), Some(0x10de));
        assert_eq!(leer_hex("zz"), None);
    }
}
