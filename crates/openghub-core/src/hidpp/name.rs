//! DEVICE_TYPE_AND_NAME (0x0005): the device's own marketing name, readable in
//! fixed-size chunks. Ported from `name.go`.

use crate::error::Error;
use crate::hidpp::{HidppConn, root};

pub const FEATURE_DEVICE_TYPE_AND_NAME: u16 = 0x0005;

const NAME_FUNC_GET_COUNT: u8 = 0x00; // -> [nameLength]
const NAME_FUNC_GET_NAME: u8 = 0x10; // [charIndex] -> up to 16 name bytes

/// Guards against a device reporting an implausible name length and this
/// issuing dozens of calls for it.
const MAX_DEVICE_NAME_LEN: usize = 64;

/// Reads the device's marketing name (e.g. "PRO X SUPERLIGHT"), paging through
/// the name until the reported length is collected. Errors if the device
/// doesn't expose the feature; callers treat the name as a nice-to-have and
/// fall back to a product-ID-derived name.
pub async fn get_device_name(conn: &HidppConn, device_index: u8) -> Result<String, Error> {
    let idx = root::get_feature_index(conn, device_index, FEATURE_DEVICE_TYPE_AND_NAME)
        .await?
        .ok_or_else(|| {
            Error::invalid_operation("hidpp: device has no DEVICE_TYPE_AND_NAME feature")
        })?;

    let resp = conn
        .call(device_index, idx, NAME_FUNC_GET_COUNT, &[])
        .await?;
    if resp.is_empty() {
        return Err(Error::invalid_data(
            "hidpp: short device name length response",
        ));
    }
    let count = resp[0] as usize;
    if count == 0 || count > MAX_DEVICE_NAME_LEN {
        return Err(Error::invalid_data(format!(
            "hidpp: implausible device name length {count}"
        )));
    }

    let mut name = Vec::with_capacity(count);
    while name.len() < count {
        let resp = conn
            .call(device_index, idx, NAME_FUNC_GET_NAME, &[name.len() as u8])
            .await?;
        if resp.is_empty() {
            return Err(Error::invalid_data("hidpp: empty device name chunk"));
        }
        let remaining = count - name.len();
        name.extend_from_slice(&resp[..resp.len().min(remaining)]);
    }

    // Some firmwares pad the advertised length with NULs; trim them.
    while name.last() == Some(&0) {
        name.pop();
    }
    if name.is_empty() {
        return Err(Error::invalid_data("hidpp: device reported an empty name"));
    }
    String::from_utf8(name).map_err(|_| Error::invalid_data("hidpp: device name is not UTF-8"))
}
