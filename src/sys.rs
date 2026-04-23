use std::{
    fs, io,
    path::{Path, PathBuf},
};

use dmidecode::{EntryPoint, PhysicalMemoryArray, Structure, memory_device::MemoryTechnology};

const PCI_DEVICES_ROOT: &str = "/sys/bus/pci/devices";
const DMI_ENTRY_POINT_PATH: &str = "/sys/firmware/dmi/tables/smbios_entry_point";
const DMI_TABLE_PATH: &str = "/sys/firmware/dmi/tables/DMI";
// const INTEL_VENDOR_ID: u16 = 0x8086;
// const MEMORY_CONTROLLER_CLASS: u32 = 0x05;
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

#[derive(Debug, Clone)]
pub struct DmiDecodedData {
    pub entry_point: EntryPoint,
    pub dmi_table: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct DmiMemoryInfo {
    pub arrays: Vec<PhysicalMemoryArray>,
    pub devices: Vec<MemoryDeviceStatic>,
}

#[derive(Debug, Clone)]
pub struct MemoryDeviceStatic {
    pub total_width: u16,
    pub ecc: bool, //如果total_width != data_width则为true
    pub size: u64, //检测字段是否为7FFFh如果是则使用extended_size
    pub memory_type: dmidecode::memory_device::Type,
    pub max_speed: u16, //MT/s
    pub manufacturer: String,
    pub bank_locator: String,
    pub serial_number: String,
    pub part_number: String,
    pub configured_speed: u16, //MT/s
    pub min_voltage: u16,
    pub max_voltage: u16,
    pub configured_voltage: u16,
    pub trchnology: MemoryTechnology,

}

#[derive(Debug, Clone)]
pub struct DmiPhysicalMemoryArrayInfo {
    pub handle: u16,
    pub location: String,
    pub usage: String,
    pub error_correction: String,
    pub max_capacity_bytes: Option<u64>,
    pub device_slots: u16,
}

pub fn decode_dmi() -> io::Result<DmiDecodedData> {
    let entry_point_buffer = fs::read(DMI_ENTRY_POINT_PATH)?;
    let entry_point = EntryPoint::search(&entry_point_buffer).map_err(invalid_dmi_data)?;
    let dmi_table = fs::read(DMI_TABLE_PATH)?;

    Ok(DmiDecodedData {
        entry_point,
        dmi_table,
    })
}

pub fn extract_memory_structures(decoded: &DmiDecodedData) -> io::Result<DmiMemoryInfo> {
    let mut last_error = None;

    for buffer in dmi_decode_candidates(decoded) {
        match parse_memory_structures(&decoded.entry_point, buffer) {
            Ok(memory_info) => return Ok(memory_info),
            Err(err) => last_error = Some(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "failed to decode DMI memory structures".to_string()),
    ))
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

pub fn get_gpu() -> io::Result<Vec<PciGpuDevice>> {
    let pci_devices = get_pci_devices()?;
    Ok(pci_devices
        .into_iter()
        .filter(|x| x.class_code >> 16 == DISPLAY_CONTROLLER_CLASS)
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

fn parse_memory_structures(
    entry_point: &EntryPoint,
    dmi_table: &[u8],
) -> Result<DmiMemoryInfo, dmidecode::MalformedStructureError> {
    let mut arrays = Vec::new();
    let mut devices = Vec::new();

    for structure in entry_point.structures(dmi_table) {
        match structure? {
            Structure::PhysicalMemoryArray(array) => {
                arrays.push(array);
            }
            Structure::MemoryDevice(device) => {
                if device.memory_type == dmidecode::memory_device::Type::Unknown {
                    continue;
                }
                let ecc = if let Some(total) = device.total_width {
                    device.data_width.unwrap_or_default() == total
                } else {false};
                let size = if let Some(size) = device.size {
                    if size == 0x7FFF {
                        device.extended_size as u64 * 1024 * 1024
                    } else {
                        size as u64 * 1024 * 1024
                    }
                } else { 0 };
                devices.push(MemoryDeviceStatic {
                    total_width: device.total_width.unwrap_or_default(),
                    ecc: ecc,
                    size: size,
                    memory_type: device.memory_type,
                    max_speed: device.speed.unwrap_or_default(),
                    manufacturer: String::from(device.manufacturer),
                    bank_locator: String::from(device.bank_locator),
                    serial_number: String::from(device.serial),
                    part_number: String::from(device.part_number),
                    configured_speed: device.configured_memory_speed.unwrap_or_default(),
                    min_voltage: device.minimum_voltage.unwrap_or_default(),
                    max_voltage: device.maximum_voltage.unwrap_or_default(),
                    configured_voltage: device.configured_voltage.unwrap_or_default(),
                    trchnology: device.memory_technology.unwrap_or_default(),
                });
            }
            _ => {}
        }
    }

    Ok(DmiMemoryInfo { arrays, devices })
}

fn dmi_decode_candidates(decoded: &DmiDecodedData) -> Vec<&[u8]> {
    let mut candidates = Vec::with_capacity(2);
    let address = decoded.entry_point.smbios_address() as usize;
    if address != 0 && address < decoded.dmi_table.len() {
        candidates.push(&decoded.dmi_table[address..]);
    }
    candidates.push(decoded.dmi_table.as_slice());
    candidates
}

fn invalid_dmi_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
