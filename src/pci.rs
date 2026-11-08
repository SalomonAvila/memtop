//! GPU names from their PCI identifiers, without depending on `lspci`.
//!
//! The binary embeds `data/pci-gpu.ids`, an extract of the public
//! [pci.ids](https://pci-ids.ucw.cz) database with only the graphics vendors. It is
//! regenerated with `scripts/update-pci-ids.sh`.

use std::{collections::HashMap, sync::OnceLock};

const DATABASE: &str = include_str!("../data/pci-gpu.ids");

struct Vendor {
    name: &'static str,
    devices: HashMap<u16, &'static str>,
}

fn database() -> &'static HashMap<u16, Vendor> {
    static PCI_DATABASE: OnceLock<HashMap<u16, Vendor>> = OnceLock::new();
    PCI_DATABASE.get_or_init(|| parse(DATABASE))
}

/// Parses the pci.ids format:
///
/// ```text
/// 10de  NVIDIA Corporation
/// <TAB>2504  GA106 [GeForce RTX 3060 Lite Hash Rate]
/// ```
fn parse(text: &'static str) -> HashMap<u16, Vendor> {
    let mut map: HashMap<u16, Vendor> = HashMap::new();
    let mut current: Option<u16> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() || line.starts_with("\t\t") {
            continue;
        }
        let (is_device, rest) = match line.strip_prefix('\t') {
            Some(r) => (true, r),
            None => (false, line),
        };
        let Some((id, name)) = rest.split_once("  ") else {
            continue;
        };
        let Ok(id) = u16::from_str_radix(id.trim(), 16) else {
            continue;
        };
        let name = name.trim();
        if is_device {
            if let Some(v) = current.and_then(|v| map.get_mut(&v)) {
                v.devices.insert(id, name);
            }
        } else {
            current = Some(id);
            map.insert(
                id,
                Vendor {
                    name,
                    devices: HashMap::new(),
                },
            );
        }
    }
    map
}

/// Short vendor name: `"NVIDIA Corporation"` → `"NVIDIA"`.
pub fn vendor_name(vendor: u16) -> Option<String> {
    let name = database().get(&vendor)?.name;
    Some(shorten_vendor(name))
}

/// Marketing name of the device: `"GA106 [GeForce RTX 3060]"` → `"GeForce RTX 3060"`.
pub fn device_name(vendor: u16, device: u16) -> Option<String> {
    let name = database().get(&vendor)?.devices.get(&device)?;
    Some(marketing_name(name))
}

fn shorten_vendor(name: &str) -> String {
    if name.contains("AMD") || name.contains("ATI") {
        return "AMD".into();
    }
    let mut n = name.to_string();
    for suffix in [
        " Corporation",
        " Corp.",
        ", Inc.",
        " Inc.",
        " Ltd.",
        " Technology",
    ] {
        n = n.replace(suffix, "");
    }
    n.trim().to_string()
}

/// The part in brackets is the retail name; outside them, the chip's code name.
fn marketing_name(name: &str) -> String {
    match (name.find('['), name.rfind(']')) {
        (Some(a), Some(b)) if b > a + 1 => name[a + 1..b].to_string(),
        _ => name.to_string(),
    }
}

/// Parses an identifier from `/sys` (`"0x10de"`).
pub fn parse_hex(text: &str) -> Option<u16> {
    u16::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_database_has_the_main_vendors() {
        assert_eq!(vendor_name(0x10de).as_deref(), Some("NVIDIA"));
        assert_eq!(vendor_name(0x1002).as_deref(), Some("AMD"));
        assert_eq!(vendor_name(0x8086).as_deref(), Some("Intel"));
    }

    #[test]
    fn parses_vendors_and_devices() {
        let db = parse(
            "# comment\n10de  NVIDIA Corporation\n\t2504  GA106 [GeForce RTX 3060]\n\t\t1043 8851  subsystem\nabcd  Other\n",
        );
        assert_eq!(db[&0x10de].devices[&0x2504], "GA106 [GeForce RTX 3060]");
        assert_eq!(db[&0xabcd].name, "Other");
        assert!(db[&0xabcd].devices.is_empty());
    }

    #[test]
    fn extracts_the_marketing_name() {
        assert_eq!(
            marketing_name("GA106 [GeForce RTX 3060]"),
            "GeForce RTX 3060"
        );
        assert_eq!(marketing_name("Kaveri"), "Kaveri");
        assert_eq!(marketing_name("Odd []"), "Odd []");
    }

    #[test]
    fn shortens_vendors() {
        assert_eq!(
            shorten_vendor("Advanced Micro Devices, Inc. [AMD/ATI]"),
            "AMD"
        );
        assert_eq!(shorten_vendor("Intel Corporation"), "Intel");
        assert_eq!(
            shorten_vendor("Matrox Electronics Systems Ltd."),
            "Matrox Electronics Systems"
        );
    }

    #[test]
    fn parses_ids_from_sys() {
        assert_eq!(parse_hex("0x10de\n"), Some(0x10de));
        assert_eq!(parse_hex("zz"), None);
    }
}
