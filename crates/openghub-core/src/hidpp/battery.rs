//! Battery feature IDs, tried in this priority order (most direct/precise
//! first) by [`resolve_battery_feature`], since not every device implements the
//! same one. Ported from `internal/hidpp/battery.go`.

use crate::error::Error;
use crate::hidpp::{HidppConn, root};

pub const FEATURE_UNIFIED_BATTERY: u16 = 0x1004;
pub const FEATURE_BATTERY_STATUS: u16 = 0x1000; // legacy: percent + status, but 0% means "unknown"
pub const FEATURE_BATTERY_VOLTAGE: u16 = 0x1001; // voltage-based estimate
pub const FEATURE_ADC_MEASUREMENT: u16 = 0x1f20; // voltage-based estimate, last resort

/// Battery status byte values, shared by BATTERY_STATUS and UNIFIED_BATTERY.
pub const STATUS_DISCHARGING: u8 = 0x00;
pub const STATUS_RECHARGING: u8 = 0x01;
pub const STATUS_ALMOST_FULL: u8 = 0x02;
pub const STATUS_FULL: u8 = 0x03;
pub const STATUS_SLOW_RECHARGE: u8 = 0x04;

const FUNC_GET_STATUS_UNIFIED: u8 = 0x10;
const FUNC_GET_STATUS_LEGACY: u8 = 0x00;
const FUNC_GET_VOLTAGE: u8 = 0x00;
const FUNC_GET_ADC: u8 = 0x00;

/// Identifies which battery feature a device resolved to, since each has a
/// different function number and response layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryKind {
    None,
    Unified,
    LegacyStatus,
    Voltage,
    Adc,
}

/// Tries each known battery feature in turn and returns whichever the device
/// supports. Errors only on an actual transport failure; a device with no
/// battery feature at all yields [`BatteryKind::None`].
pub async fn resolve_battery_feature(
    conn: &HidppConn,
    device_index: u8,
) -> Result<(u8, BatteryKind), Error> {
    const CANDIDATES: [(u16, BatteryKind); 4] = [
        (FEATURE_UNIFIED_BATTERY, BatteryKind::Unified),
        (FEATURE_BATTERY_STATUS, BatteryKind::LegacyStatus),
        (FEATURE_BATTERY_VOLTAGE, BatteryKind::Voltage),
        (FEATURE_ADC_MEASUREMENT, BatteryKind::Adc),
    ];

    for (id, kind) in CANDIDATES {
        if let Some(idx) = root::get_feature_index(conn, device_index, id).await? {
            return Ok((idx, kind));
        }
    }
    Ok((0, BatteryKind::None))
}

/// Reads the current percent/charging state using a feature index and kind
/// previously resolved by [`resolve_battery_feature`].
pub async fn read_battery(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
    kind: BatteryKind,
) -> Result<(i32, bool), Error> {
    match kind {
        BatteryKind::Unified => {
            from_status_call(
                conn,
                device_index,
                feature_index,
                FUNC_GET_STATUS_UNIFIED,
                false,
            )
            .await
        }
        BatteryKind::LegacyStatus => {
            from_status_call(
                conn,
                device_index,
                feature_index,
                FUNC_GET_STATUS_LEGACY,
                true,
            )
            .await
        }
        BatteryKind::Voltage => {
            from_voltage_call(conn, device_index, feature_index, FUNC_GET_VOLTAGE, false).await
        }
        BatteryKind::Adc => {
            from_voltage_call(conn, device_index, feature_index, FUNC_GET_ADC, true).await
        }
        BatteryKind::None => Err(Error::invalid_operation(
            "hidpp: device has no supported battery feature",
        )),
    }
}

/// Handles UNIFIED_BATTERY and BATTERY_STATUS, whose responses both start with
/// `[percent, _, status]`. `zero_means_unknown` is only true for the legacy
/// feature.
async fn from_status_call(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
    function: u8,
    zero_means_unknown: bool,
) -> Result<(i32, bool), Error> {
    let resp = conn
        .call(device_index, feature_index, function, &[])
        .await?;
    if resp.len() < 3 {
        return Err(Error::invalid_data("hidpp: short battery status response"));
    }

    let percent = resp[0] as i32;
    if percent == 0 && zero_means_unknown {
        return Err(Error::invalid_data("hidpp: battery percentage unavailable"));
    }
    Ok((percent, is_charging_status(resp[2])))
}

/// Handles BATTERY_VOLTAGE and ADC_MEASUREMENT, whose responses are both
/// `[voltage(2B BE, mV), flags(1B)]`; ADC requires checking a "valid" bit first,
/// and the two features use different flag bits for "charging".
async fn from_voltage_call(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
    function: u8,
    is_adc: bool,
) -> Result<(i32, bool), Error> {
    let resp = conn
        .call(device_index, feature_index, function, &[])
        .await?;
    if resp.len() < 3 {
        return Err(Error::invalid_data("hidpp: short battery voltage response"));
    }

    let millivolts = ((resp[0] as i32) << 8) | resp[1] as i32;
    let flags = resp[2];

    let charging = if is_adc {
        if flags & 0x01 == 0 {
            return Err(Error::invalid_data(
                "hidpp: no valid battery reading available",
            ));
        }
        flags & 0x02 != 0
    } else {
        flags & 0x80 != 0
    };

    Ok((estimate_battery_percent(millivolts), charging))
}

fn is_charging_status(status: u8) -> bool {
    matches!(
        status,
        STATUS_RECHARGING | STATUS_ALMOST_FULL | STATUS_FULL | STATUS_SLOW_RECHARGE
    )
}

/// Maps single-cell Li-ion/Li-Po voltage (millivolts) to an estimated charge
/// percentage, for devices that only report raw voltage. Approximate
/// discharge-curve values, consistent with the mapping widely used for this
/// purpose across Linux Logitech tooling.
const BATTERY_VOLTAGE_CURVE: [(i32, i32); 13] = [
    (4186, 100),
    (4067, 90),
    (3989, 80),
    (3922, 70),
    (3859, 60),
    (3811, 50),
    (3778, 40),
    (3751, 30),
    (3717, 20),
    (3671, 10),
    (3646, 5),
    (3579, 2),
    (3500, 0),
];

/// Estimates a charge percentage from a millivolt reading via linear
/// interpolation over [`BATTERY_VOLTAGE_CURVE`].
pub fn estimate_battery_percent(millivolts: i32) -> i32 {
    if millivolts >= BATTERY_VOLTAGE_CURVE[0].0 {
        return BATTERY_VOLTAGE_CURVE[0].1;
    }
    let last = BATTERY_VOLTAGE_CURVE[BATTERY_VOLTAGE_CURVE.len() - 1];
    if millivolts <= last.0 {
        return last.1;
    }

    for i in 0..BATTERY_VOLTAGE_CURVE.len() - 1 {
        let hi = BATTERY_VOLTAGE_CURVE[i].0; // higher-voltage end
        let lo = BATTERY_VOLTAGE_CURVE[i + 1].0; // lower-voltage end
        if (lo..=hi).contains(&millivolts) {
            let span = hi - lo;
            let frac = (millivolts - lo) as f64 / span as f64;
            let hi_pct = BATTERY_VOLTAGE_CURVE[i].1;
            let lo_pct = BATTERY_VOLTAGE_CURVE[i + 1].1;
            return lo_pct + (frac * (hi_pct - lo_pct) as f64 + 0.5) as i32;
        }
    }
    0
}
