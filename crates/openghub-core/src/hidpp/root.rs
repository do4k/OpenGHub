//! Root feature (0x0000) helpers. The Root feature is always at feature index
//! 0, so these calls never need a prior lookup. Ported from `root.go`.

use crate::error::Error;
use crate::hidpp::{HidppConn, ROOT_FEATURE_INDEX};

const ROOT_FUNC_GET_FEATURE: u8 = 0x00;
const ROOT_FUNC_GET_PROTOCOL_VERSION: u8 = 0x10;

/// Asks the Root feature which feature index implements `feature_id` on
/// `device_index`. Returns `None` if the device doesn't support the feature at
/// all (a legitimate, expected outcome — not an error).
pub async fn get_feature_index(
    conn: &HidppConn,
    device_index: u8,
    feature_id: u16,
) -> Result<Option<u8>, Error> {
    let resp = conn
        .call(
            device_index,
            ROOT_FEATURE_INDEX,
            ROOT_FUNC_GET_FEATURE,
            &[(feature_id >> 8) as u8, feature_id as u8],
        )
        .await?;
    Ok((!resp.is_empty() && resp[0] != 0).then_some(resp[0]))
}

/// Confirms `device_index` is present and speaks HID++ 2.0, returning its
/// protocol version. Also the standard way to probe which device indices
/// behind a receiver currently have a paired device.
pub async fn ping(conn: &HidppConn, device_index: u8) -> Result<(u8, u8), Error> {
    let resp = conn
        .call(
            device_index,
            ROOT_FEATURE_INDEX,
            ROOT_FUNC_GET_PROTOCOL_VERSION,
            &[0x00, 0x00, 0xaa],
        )
        .await?;
    if resp.len() < 2 {
        return Err(Error::invalid_data(format!(
            "hidpp: short ping response from device 0x{device_index:02x}"
        )));
    }
    Ok((resp[0], resp[1]))
}
