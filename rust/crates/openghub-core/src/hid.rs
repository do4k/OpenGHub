//! Linux hidraw backend: device discovery via `/sys/class/hidraw` and I/O via
//! `/dev/hidrawN`, using only raw syscalls (no libhidapi).
//! Ported from `internal/hid/hidraw_linux.go`.

use std::fs;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::sys::open_owned;

const CLASS_PATH: &str = "/sys/class/hidraw";

/// Describes a discovered HID device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
}

/// Linux hidraw backend.
#[derive(Debug, Default)]
pub struct HidBackend;

#[derive(Debug, Clone)]
struct HidUevent {
    vendor: u16,
    product: u16,
    uniq: Option<String>,
}

impl HidBackend {
    /// Enumerates all HID devices matching `vendor_id`; a `product_id` of 0
    /// matches any product.
    pub fn enumerate(&self, vendor_id: u16, product_id: u16) -> Vec<DeviceInfo> {
        let mut results = Vec::new();
        let class_path = Path::new(CLASS_PATH);
        let entries = match fs::read_dir(class_path) {
            Ok(e) => e,
            Err(_) => return results,
        };

        for entry in entries.flatten() {
            let dev_dir = entry.path().join("device");
            let Some(u) = read_uevent(&dev_dir) else {
                continue;
            };
            if u.vendor != vendor_id {
                continue;
            }
            if product_id != 0 && u.product != product_id {
                continue;
            }

            // Fall back to HID_UNIQ when there's no USB iSerialNumber (common on
            // receivers and on per-peripheral virtual hidraw nodes).
            let serial = read_serial(&dev_dir).or_else(|| u.uniq.clone());
            results.push(DeviceInfo {
                path: format!("/dev/{}", entry.file_name().to_string_lossy()),
                vendor_id: u.vendor,
                product_id: u.product,
                serial,
            });
        }

        results.sort_by(|a, b| a.path.cmp(&b.path));
        results
    }

    /// Opens the device for read/write.
    pub fn open(&self, info: &DeviceInfo) -> Result<OwnedFd, Error> {
        open_owned(&info.path)
    }
}

/// Parses vendor/product IDs from `HID_ID` ("<bus>:<vendor>:<product>") and
/// captures `HID_UNIQ`, mirroring `readUevent` in `hidraw_linux.go`.
fn read_uevent(dev_dir: &Path) -> Option<HidUevent> {
    let path = dev_dir.join("uevent");
    let contents = fs::read_to_string(path).ok()?;

    let mut vendor = None;
    let mut product = None;
    let mut uniq = None;

    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("HID_ID=") {
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() == 3
                && let (Ok(v), Ok(p)) = (
                    u16::from_str_radix(parts[1], 16),
                    u16::from_str_radix(parts[2], 16),
                )
            {
                vendor = Some(v);
                product = Some(p);
            }
        } else if let Some(rest) = line.strip_prefix("HID_UNIQ=") {
            uniq = Some(rest.to_string());
        }
    }

    match (vendor, product) {
        (Some(v), Some(p)) => Some(HidUevent {
            vendor: v,
            product: p,
            uniq,
        }),
        _ => None,
    }
}

/// Walks up the sysfs hierarchy from the HID device directory to the owning
/// USB device directory (identified by an `idVendor` file) and reads its
/// `serial` attribute. Stops at the first such directory whether or not the
/// serial exists there, mirroring `readSerial` in `hidraw_linux.go`.
fn read_serial(dev_dir: &Path) -> Option<String> {
    let real: PathBuf = fs::canonicalize(dev_dir).unwrap_or_else(|_| dev_dir.to_path_buf());
    let mut dir = real.as_path();

    for _ in 0..8 {
        if dir == Path::new("/") || dir == Path::new(".") {
            break;
        }
        if dir.join("idVendor").exists() {
            return fs::read_to_string(dir.join("serial"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
        dir = dir.parent().unwrap_or(Path::new("/"));
    }
    None
}
