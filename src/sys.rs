use std::{
    fs, io,
    path::{Path, PathBuf},
};

const PCI_DEVICES_ROOT: &str = "/sys/bus/pci/devices";
// const INTEL_VENDOR_ID: u16 = 0x8086;
const DISPLAY_CONTROLLER_CLASS: u32 = 0x03;
const PCI_IDS_PATHS: [&str; 2] = ["/usr/share/misc/pci.ids", "/usr/share/hwdata/pci.ids"];

#[derive(Debug, Clone)]
pub struct PciGpuDevice {
    pub pci_address: String,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u32,
    pub vendor_name: Option<String>,
    pub device_name: Option<String>,
    pub driver: Option<String>,
    pub drm_nodes: Vec<String>,
}

pub fn get_pci_devices() -> io::Result<Vec<PciGpuDevice>> {
    let pci_ids = load_pci_path();
    let mut gpus = Vec::new();

    for entry in fs::read_dir(PCI_DEVICES_ROOT)? {
        let entry = entry?;
        let path = entry.path();

        let Some(vendor_id) = read_hex_u16(&path.join("vendor")) else {
            continue;
        };
        let Some(device_id) = read_hex_u16(&path.join("device")) else {
            continue;
        };
        let Some(class_code) = read_hex_u32(&path.join("class")) else {
            continue;
        };

        if class_code >> 16 != DISPLAY_CONTROLLER_CLASS {
            continue;
        }

        let (vendor_name, device_name) = if let Some(ids) = pci_ids.as_deref() {
            find_pci_names(ids, vendor_id, device_id)
        } else {
            (None, None)
        };

        gpus.push(PciGpuDevice {
            pci_address: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            vendor_id,
            device_id,
            class_code,
            vendor_name,
            device_name,
            driver: read_driver_name(&path),
            drm_nodes: read_drm_nodes(&path),
        });
    }

    Ok(gpus)
}

pub fn get_intel_gpu() -> io::Result<Vec<PciGpuDevice>> {
    Ok(get_pci_devices()?
        .into_iter()
        .collect())
}

fn read_hex_u16(path: &Path) -> Option<u16> {
    let text = fs::read_to_string(path).ok()?;
    parse_hex_u16(text.trim())
}

fn read_hex_u32(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    parse_hex_u32(text.trim())
}

fn parse_hex_u16(text: &str) -> Option<u16> {
    let value = text.trim_start_matches("0x");
    u16::from_str_radix(value, 16).ok()
}

fn parse_hex_u32(text: &str) -> Option<u32> {
    let value = text.trim_start_matches("0x");
    u32::from_str_radix(value, 16).ok()
}

fn read_driver_name(device_path: &Path) -> Option<String> {
    let link = fs::read_link(device_path.join("driver")).ok()?;
    link.file_name()
        .map(|name| name.to_string_lossy().to_string())
}

fn read_drm_nodes(device_path: &Path) -> Vec<String> {
    let drm_path = device_path.join("drm");
    let Ok(entries) = fs::read_dir(drm_path) else {
        return Vec::new();
    };

    let mut nodes = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    nodes.sort();
    nodes
}

fn load_pci_path() -> Option<String> {
    let path = PCI_IDS_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())?;
    fs::read_to_string(path).ok()
}

fn find_pci_names(
    content: &str,
    vendor_id: u16,
    device_id: u16,
) -> (Option<String>, Option<String>) {
    let mut in_vendor_section = false;
    let mut vendor_name = None;
    let mut device_name = None;

    for line in content.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if !line.starts_with('\t') {
            let Some((id_hex, name)) = split_id_and_name(line.trim_start()) else {
                continue;
            };
            let Some(id) = parse_hex_u16(id_hex) else {
                continue;
            };

            in_vendor_section = id == vendor_id;
            if in_vendor_section {
                vendor_name = Some(name.to_string());
            }
            continue;
        }

        if !in_vendor_section || line.starts_with("\t\t") {
            continue;
        }

        let line = line.trim_start_matches('\t');
        let Some((id_hex, name)) = split_id_and_name(line) else {
            continue;
        };
        let Some(id) = parse_hex_u16(id_hex) else {
            continue;
        };

        if id == device_id {
            device_name = Some(name.to_string());
            break;
        }
    }

    (vendor_name, device_name)
}

fn split_id_and_name(line: &str) -> Option<(&str, &str)> {
    let split_at = line.find(char::is_whitespace)?;
    let (id, rest) = line.split_at(split_at);
    Some((id, rest.trim()))
}
