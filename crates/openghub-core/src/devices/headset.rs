//! PRO X Wireless Gaming Headset (product 0x0aba, its Lightspeed receiver).
//! HID++ device is addressed at the direct index. Ported from
//! `internal/device/prox`.

use std::time::Duration;

use crate::error::Error;
use crate::hid::{self, HidBackend};
use crate::hidpp::{DEVICE_INDEX_DIRECT, HidppConn, battery, name, root};

pub const VENDOR_ID: u16 = 0x046d;
pub const PRODUCT_ID: u16 = 0x0aba;

const FEATURE_SIDETONE: u16 = 0x8300;
const FEATURE_EQUALIZER: u16 = 0x8310;

const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

// EQUALIZER (0x8310) function numbers.
const EQ_FUNC_GET_INFO: u8 = 0x00;
const EQ_FUNC_GET_BAND_FREQUENCIES: u8 = 0x10;
const EQ_FUNC_GET_LEVELS: u8 = 0x20;
const EQ_FUNC_SET_LEVELS: u8 = 0x30;
const EQ_FUNC_GET_NOISE_REDUCTION: u8 = 0x40;
const EQ_FUNC_SET_NOISE_REDUCTION: u8 = 0x50;

// The get/set-levels calls take a leading selector byte. Get uses a gain
// *location* (0 reads the custom EQ stored in EEPROM, 1 the active EQ in RAM);
// set uses a gain *persistence* (0 RAM only, 1 RAM + EEPROM, 2 EEPROM only).
// Values verified against real hardware.
const EQ_GAIN_LOCATION_EEPROM: u8 = 0x00;
const EQ_PERSIST_NON_VOLATILE_WRITE: u8 = 0x02;

// SIDETONE (0x8300) function numbers (both are HID++'s defaults: read 0, write
// 0x10 — this feature doesn't override them).
const SIDETONE_FUNC_GET: u8 = 0x00;
const SIDETONE_FUNC_SET: u8 = 0x10;

/// A PRO X Wireless headset.
pub struct ProxHeadset {
    conn: HidppConn,
    battery_feature_index: u8,
    battery_kind: battery::BatteryKind,
    name: String,
    sidetone_feature_index: u8,
    eq_feature_index: u8,
    eq_frequencies_hz: Vec<i32>,
    eq_min_db: i32,
    eq_max_db: i32,
}

impl ProxHeadset {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Opens the headset's receiver node. Returns `None` when the device isn't
    /// reachable (headset off/asleep). Open/permission errors propagate.
    pub async fn open(backend: &HidBackend, info: &hid::DeviceInfo) -> Result<Option<Self>, Error> {
        let conn = HidppConn::new(backend.open(info)?);
        match probe(conn).await {
            Ok(h) => Ok(Some(h)),
            Err(_) => Ok(None),
        }
    }

    /// Throws [`Error::InvalidOperation`] when the device has no supported
    /// battery feature.
    pub async fn get_battery(&self) -> Result<(i32, bool), Error> {
        battery::read_battery(
            &self.conn,
            DEVICE_INDEX_DIRECT,
            self.battery_feature_index,
            self.battery_kind,
        )
        .await
    }

    /// Reads the current sidetone (microphone monitoring) volume, as a percent.
    pub async fn get_sidetone(&self) -> Result<i32, Error> {
        if self.sidetone_feature_index == 0 {
            return Err(Error::invalid_operation(
                "prox: device has no sidetone feature",
            ));
        }
        let resp = self
            .conn
            .call(
                DEVICE_INDEX_DIRECT,
                self.sidetone_feature_index,
                SIDETONE_FUNC_GET,
                &[],
            )
            .await?;
        if resp.is_empty() {
            return Err(Error::invalid_data("prox: short sidetone response"));
        }
        Ok(resp[0] as i32)
    }

    /// Sets the sidetone (microphone monitoring) volume, as a percent clamped
    /// to 0..100.
    pub async fn set_sidetone(&self, percent: i32) -> Result<(), Error> {
        if self.sidetone_feature_index == 0 {
            return Err(Error::invalid_operation(
                "prox: device has no sidetone feature",
            ));
        }
        let clamped = percent.clamp(0, 100) as u8;
        self.conn
            .call(
                DEVICE_INDEX_DIRECT,
                self.sidetone_feature_index,
                SIDETONE_FUNC_SET,
                &[clamped],
            )
            .await?;
        Ok(())
    }

    /// Whether the device exposes the equalizer feature.
    pub fn has_equalizer(&self) -> bool {
        self.eq_feature_index != 0
    }

    /// The device's fixed band frequencies, in Hz, one per band in order.
    pub fn eq_frequencies(&self) -> &[i32] {
        &self.eq_frequencies_hz
    }

    /// The device's dB range for equalizer band levels.
    pub fn eq_range(&self) -> (i32, i32) {
        (self.eq_min_db, self.eq_max_db)
    }

    /// Reads the headset's current equalizer band levels, in dB, from the
    /// custom EQ stored in EEPROM.
    pub async fn get_eq_levels(&self) -> Result<Vec<i32>, Error> {
        self.ensure_equalizer()?;
        let resp = self
            .conn
            .call(
                DEVICE_INDEX_DIRECT,
                self.eq_feature_index,
                EQ_FUNC_GET_LEVELS,
                &[EQ_GAIN_LOCATION_EEPROM],
            )
            .await?;
        let count = self.eq_frequencies_hz.len();
        if resp.len() < count {
            return Err(Error::invalid_data("prox: short equalizer levels response"));
        }
        Ok(resp[..count].iter().map(|b| *b as i8 as i32).collect())
    }

    /// Sets the equalizer band levels, in dB, persisting them to EEPROM. Each
    /// level is clamped to the device's dB range.
    pub async fn set_eq_levels(&self, levels_db: &[i32]) -> Result<(), Error> {
        self.ensure_equalizer()?;
        if levels_db.len() != self.eq_frequencies_hz.len() {
            return Err(Error::invalid_data(format!(
                "prox: expected {} equalizer levels, got {}",
                self.eq_frequencies_hz.len(),
                levels_db.len()
            )));
        }

        let mut params = Vec::with_capacity(1 + levels_db.len());
        params.push(EQ_PERSIST_NON_VOLATILE_WRITE);
        for &level in levels_db {
            params.push(level.clamp(self.eq_min_db, self.eq_max_db) as u8);
        }
        self.conn
            .call(
                DEVICE_INDEX_DIRECT,
                self.eq_feature_index,
                EQ_FUNC_SET_LEVELS,
                &params,
            )
            .await?;
        Ok(())
    }

    /// Whether the headset's hardware microphone noise reduction is enabled.
    pub async fn get_noise_reduction(&self) -> Result<bool, Error> {
        self.ensure_equalizer()?;
        let resp = self
            .conn
            .call(
                DEVICE_INDEX_DIRECT,
                self.eq_feature_index,
                EQ_FUNC_GET_NOISE_REDUCTION,
                &[],
            )
            .await?;
        if resp.is_empty() {
            return Err(Error::invalid_data("prox: short noise reduction response"));
        }
        Ok(resp[0] != 0)
    }

    /// Enables or disables the headset's hardware microphone noise reduction.
    pub async fn set_noise_reduction(&self, enabled: bool) -> Result<(), Error> {
        self.ensure_equalizer()?;
        let value = if enabled { 1u8 } else { 0u8 };
        self.conn
            .call(
                DEVICE_INDEX_DIRECT,
                self.eq_feature_index,
                EQ_FUNC_SET_NOISE_REDUCTION,
                &[value],
            )
            .await?;
        Ok(())
    }

    fn ensure_equalizer(&self) -> Result<(), Error> {
        if self.eq_feature_index == 0 {
            return Err(Error::invalid_operation(
                "prox: device has no equalizer feature",
            ));
        }
        Ok(())
    }
}

async fn probe(conn: HidppConn) -> Result<ProxHeadset, Error> {
    conn.set_timeout(PROBE_TIMEOUT);
    root::ping(&conn, DEVICE_INDEX_DIRECT).await?;
    conn.set_timeout(OPERATION_TIMEOUT);

    let mut name = "PRO X Wireless Headset".to_string();
    if let Ok(self_name) = name::get_device_name(&conn, DEVICE_INDEX_DIRECT).await
        && !self_name.is_empty()
    {
        name = self_name;
    }

    let (battery_feature_index, battery_kind) =
        battery::resolve_battery_feature(&conn, DEVICE_INDEX_DIRECT).await?;

    // Sidetone and EQ are optional: a unit without them shouldn't block battery.
    let sidetone_feature_index =
        root::get_feature_index(&conn, DEVICE_INDEX_DIRECT, FEATURE_SIDETONE)
            .await?
            .unwrap_or(0);

    let mut eq_feature_index = 0;
    let mut eq_frequencies_hz = Vec::new();
    let mut eq_min_db = 0;
    let mut eq_max_db = 0;
    if let Some(eq) = root::get_feature_index(&conn, DEVICE_INDEX_DIRECT, FEATURE_EQUALIZER).await?
        && let Ok((freqs, min_db, max_db)) = discover_equalizer(&conn, eq).await
    {
        eq_frequencies_hz = freqs;
        eq_min_db = min_db;
        eq_max_db = max_db;
        eq_feature_index = eq;
    }

    Ok(ProxHeadset {
        conn,
        battery_feature_index,
        battery_kind,
        name,
        sidetone_feature_index,
        eq_feature_index,
        eq_frequencies_hz,
        eq_min_db,
        eq_max_db,
    })
}

/// Reads the device's fixed band layout (frequencies) and dB range.
async fn discover_equalizer(
    conn: &HidppConn,
    feature_index: u8,
) -> Result<(Vec<i32>, i32, i32), Error> {
    let info = conn
        .call(DEVICE_INDEX_DIRECT, feature_index, EQ_FUNC_GET_INFO, &[])
        .await?;
    if info.len() < 5 {
        return Err(Error::invalid_data("prox: short equalizer info response"));
    }

    let count = info[0] as usize;
    let db_range = info[1] as i8 as i32;
    let mut min_db = info[3] as i8 as i32;
    let mut max_db = info[4] as i8 as i32;
    if min_db == 0 {
        min_db = -db_range;
    }
    if max_db == 0 {
        max_db = db_range;
    }

    let mut frequencies = vec![0i32; count];
    let mut group = 0;
    while group * 7 < count {
        let resp = conn
            .call(
                DEVICE_INDEX_DIRECT,
                feature_index,
                EQ_FUNC_GET_BAND_FREQUENCIES,
                &[(group * 7) as u8],
            )
            .await?;
        for b in 0..7 {
            let band_index = group * 7 + b;
            if band_index >= count {
                break;
            }
            let offset = 1 + 2 * b;
            if offset + 2 > resp.len() {
                return Err(Error::invalid_data(
                    "prox: short equalizer band frequency response",
                ));
            }
            frequencies[band_index] = ((resp[offset] as i32) << 8) | resp[offset + 1] as i32;
        }
        group += 1;
    }

    Ok((frequencies, min_db, max_db))
}
