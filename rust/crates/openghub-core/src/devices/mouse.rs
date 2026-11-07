//! G Pro Wireless / PRO X Superlight mouse family over HID++ 2.0. Every
//! capability is a feature discovered at runtime via the Root feature.
//! Ported from `internal/device/gpro` (gpro.go, dpi.go, dpi_extended.go,
//! reportrate.go, reportrate_extended.go, rgb.go, buttons.go).

use std::time::Duration;

use crate::error::Error;
use crate::hid::{self, HidBackend};
use crate::hidpp::{DEVICE_INDEX_DIRECT, HidppConn, battery, name, root};

pub const VENDOR_ID: u16 = 0x046d;

/// The Logitech Lightspeed USB receiver the mice are typically paired to.
pub const PRODUCT_RECEIVER: u16 = 0xc539;
// Direct-connection product IDs: wired (Bluetooth), wired USB cable, and the
// newer Superlight units.
pub const PRODUCT_WIRED: u16 = 0x4079;
pub const PRODUCT_WIRED_USB: u16 = 0xc088;
pub const PRODUCT_SUPERLIGHT: u16 = 0xc094;
pub const PRODUCT_SUPERLIGHT2: u16 = 0xc09b;

const FEATURE_ADJUSTABLE_DPI: u16 = 0x2201;
const FEATURE_EXTENDED_DPI: u16 = 0x2202; // successor to 0x2201 on newer mice
const FEATURE_REPORT_RATE: u16 = 0x8060;
const FEATURE_EXTENDED_REPORT_RATE: u16 = 0x8061; // successor to 0x8060
const FEATURE_COLOR_LED_EFFECTS: u16 = 0x8070;
const FEATURE_REPROG_CONTROLS_V4: u16 = 0x1b04;

// Probe timeout bounds each Ping while hunting for which hidraw interface and
// device index the mouse is reachable on; operation timeout applies after.
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

// DPI.
const DPI_SENSOR_INDEX: u8 = 0x00;
const DPI_FUNC_GET_DPI: u8 = 0x20;
const DPI_FUNC_SET_DPI: u8 = 0x30;

// The G Pro Wireless has a single HERO 25K sensor: 100-25600 DPI in steps of 50.
const CLASSIC_DPI_MIN: i32 = 100;
const CLASSIC_DPI_MAX: i32 = 25600;
const CLASSIC_DPI_STEP: i32 = 50;

// Extended DPI (0x2202).
const EXT_DPI_FUNC_GET_SENSOR_CAPABILITIES: u8 = 0x10;
const EXT_DPI_FUNC_GET_DPI_RANGES: u8 = 0x20;
const EXT_DPI_FUNC_GET_DPI_PARAMETERS: u8 = 0x50;
const EXT_DPI_FUNC_SET_DPI_PARAMETERS: u8 = 0x60;

const EXT_DPI_CAP_HAS_Y: u8 = 1 << 0; // sensor supports independent Y-axis DPI
const EXT_DPI_RANGE_HYPHEN: u16 = 0b111 << 13;
const EXT_DPI_MAX_RANGE_PAGES: u8 = 16;
const EXT_DPI_DIRECTION_X: u8 = 0;

// Report rate.
const REPORT_RATE_FUNC_GET_LIST: u8 = 0x00;
const REPORT_RATE_FUNC_GET: u8 = 0x10;
const REPORT_RATE_FUNC_SET: u8 = 0x20;

// Extended report rate (0x8061).
const EXT_RATE_FUNC_GET_RATE_LIST: u8 = 0x10;
const EXT_RATE_FUNC_GET_RATE: u8 = 0x20;
const EXT_RATE_FUNC_SET_RATE: u8 = 0x30;

const EXT_RATE_CONN_WIRED: u8 = 0;
const EXT_RATE_CONN_GAMING_WIRELESS: u8 = 1; // Lightspeed

/// Maps the feature's rate index (0-6) to Hz.
const EXT_RATE_HZ: [i32; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];

// COLOR_LED_EFFECTS (0x8070).
const LED_FUNC_GET_INFO: u8 = 0x00;
const LED_FUNC_GET_ZONE_INFO: u8 = 0x10;
const LED_FUNC_GET_ZONE_EFFECT_INFO: u8 = 0x20;
const LED_FUNC_SET_ZONE_EFFECT: u8 = 0x30;
const LED_FUNC_SET_CONTROL: u8 = 0x80;

const LED_ZONE_LOCATION_LOGO: u16 = 0x02;
const LED_EFFECT_ID_STATIC: u16 = 0x01;

// REPROG_CONTROLS_V4 (0x1b04).
const BUTTONS_FUNC_GET_COUNT: u8 = 0x00;
const BUTTONS_FUNC_GET_CID_INFO: u8 = 0x10;
const BUTTONS_FUNC_SET_CID_REPORTING: u8 = 0x30;

const KEY_FLAG_DIVERTABLE: u16 = 0x0020;
const MAPPING_FLAG_DIVERTED: u8 = 0x01;

fn product_name(product_id: u16) -> Option<&'static str> {
    match product_id {
        PRODUCT_WIRED => Some("G Pro Wireless"),
        PRODUCT_WIRED_USB => Some("G Pro Wireless"),
        PRODUCT_SUPERLIGHT => Some("PRO X Superlight"),
        PRODUCT_SUPERLIGHT2 => Some("PRO X Superlight 2"),
        _ => None,
    }
}

/// Maps known control IDs to human-readable labels; unlisted IDs fall back to
/// "Control 0xNNNN". Curated from Solaar's special_keys.py CONTROL table,
/// limited to controls relevant to a mouse.
fn control_name(cid: u16) -> String {
    match cid {
        0x0050 => "Left Button".to_string(),
        0x0051 => "Right Button".to_string(),
        0x0052 => "Middle Button".to_string(),
        0x0053 => "Back Button".to_string(),
        0x0056 => "Forward Button".to_string(),
        0x0059 => "Button 6".to_string(),
        0x005a => "Button 7".to_string(),
        0x005b => "Left Tilt".to_string(),
        0x005c => "Button 8".to_string(),
        0x005d => "Right Tilt".to_string(),
        0x00c3 => "Gesture Button".to_string(),
        0x00ed => "DPI Change".to_string(),
        0x00fd => "DPI Switch".to_string(),
        _ => format!("Control 0x{cid:04x}"),
    }
}

/// One reprogrammable control discovered via GetCidInfo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonInfo {
    pub id: u16,
    pub name: String,
    pub is_divertable: bool,
}

#[derive(Debug, Clone, Copy)]
struct ExtDpiInfo {
    has_y: bool,
    min: i32,
    max: i32,
    step: i32,
}

/// A G Pro mouse.
pub struct GProMouse {
    conn: HidppConn,
    device_index: u8,
    name: String,
    serial: String,
    product_id: u16,

    dpi_feature_index: u8,
    ext_dpi_feature_index: u8,
    ext_dpi: ExtDpiInfo,

    battery_feature_index: u8,
    battery_kind: battery::BatteryKind,

    report_rate_feature_index: u8,
    ext_rate_feature_index: u8,
    report_rate_options: Vec<i32>, // Hz, fastest (highest Hz) first

    led_feature_index: u8,
    led_zone_index: u8,
    led_static_effect_index: u8,

    buttons_feature_index: u8,
}

impl GProMouse {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    pub fn is_mouse_product(product_id: u16) -> bool {
        product_name(product_id).is_some() || product_id == PRODUCT_RECEIVER
    }

    /// Opens a mouse for `info`. Returns `None` when this node is a duplicate
    /// sibling of a receiver's canonical node. Other errors (permission,
    /// transport, "no paired device") propagate.
    pub async fn open(backend: &HidBackend, info: &hid::DeviceInfo) -> Result<Option<Self>, Error> {
        let (conn, device_index, serial) = if info.product_id == PRODUCT_RECEIVER {
            // The receiver exposes one hidraw node per USB interface, all with
            // the same product ID and serial; only one actually carries HID++
            // long reports, and the mouse's device index (1-6) is assigned at
            // pairing time — both must be discovered by probing.
            let mut siblings: Vec<hid::DeviceInfo> = backend
                .enumerate(VENDOR_ID, PRODUCT_RECEIVER)
                .into_iter()
                .filter(|s| s.serial == info.serial)
                .collect();
            siblings.sort_by(|a, b| a.path.cmp(&b.path));
            if siblings.is_empty() || siblings[0].path != info.path {
                return Ok(None); // duplicate sibling; the canonical node does the work
            }

            let mut found: Option<u8> = None;
            let mut conn: Option<HidppConn> = None;
            for candidate in &siblings {
                let c = HidppConn::new(backend.open(candidate)?);
                c.set_timeout(PROBE_TIMEOUT);
                for idx in 1..=6u8 {
                    match root::ping(&c, idx).await {
                        Ok(_) => {
                            found = Some(idx);
                            break;
                        }
                        Err(Error::Timeout { .. }) | Err(Error::Hidpp(_)) => {}
                        Err(Error::Io(_)) => break, // write failed — this node is dead
                        Err(e) => return Err(e),
                    }
                }
                if found.is_some() {
                    conn = Some(c);
                    break;
                }
                // `c` is dropped here (reader joined, fd closed).
            }
            let Some(idx) = found else {
                return Err(Error::invalid_operation(
                    "gpro: no paired device responded on receiver (is the mouse asleep? try moving it)",
                ));
            };
            let conn = conn.unwrap();
            let serial = resolve_mouse_serial(backend, info.serial.clone().unwrap_or_default());
            (conn, idx, serial)
        } else {
            // Directly connected (wired/Bluetooth), addressed at device index 0xFF.
            let conn = HidppConn::new(backend.open(info)?);
            conn.set_timeout(PROBE_TIMEOUT);
            match root::ping(&conn, DEVICE_INDEX_DIRECT).await {
                Ok(_) => {}
                Err(Error::Timeout { .. }) => {
                    return Err(Error::invalid_operation(
                        "gpro: ping direct-connected mouse: no response",
                    ));
                }
                Err(e) => return Err(e),
            }
            (
                conn,
                DEVICE_INDEX_DIRECT,
                info.serial.clone().unwrap_or_default(),
            )
        };

        conn.set_timeout(OPERATION_TIMEOUT);
        build(conn, device_index, serial, info.product_id)
            .await
            .map(Some)
    }

    /// The sensor's supported DPI range.
    pub fn dpi_range(&self) -> (i32, i32, i32) {
        if self.ext_dpi_feature_index != 0 {
            (self.ext_dpi.min, self.ext_dpi.max, self.ext_dpi.step)
        } else {
            (CLASSIC_DPI_MIN, CLASSIC_DPI_MAX, CLASSIC_DPI_STEP)
        }
    }

    /// Reads the sensor's current DPI live from the device.
    pub async fn get_dpi(&self) -> Result<i32, Error> {
        if self.ext_dpi_feature_index != 0 {
            self.get_extended_dpi().await
        } else {
            self.get_classic_dpi().await
        }
    }

    /// Sets DPI, clamped to the sensor's range and rounded to the nearest step.
    pub async fn set_dpi(&self, dpi: i32) -> Result<(), Error> {
        if self.ext_dpi_feature_index != 0 {
            self.set_extended_dpi(dpi).await?;
            return Ok(());
        }
        let dpi = clamp_to_step(dpi, CLASSIC_DPI_MIN, CLASSIC_DPI_MAX, CLASSIC_DPI_STEP);
        let payload = [DPI_SENSOR_INDEX, (dpi >> 8) as u8, dpi as u8];
        self.conn
            .call(
                self.device_index,
                self.dpi_feature_index,
                DPI_FUNC_SET_DPI,
                &payload,
            )
            .await?;
        Ok(())
    }

    /// The supported report rates in Hz, fastest first.
    pub fn report_rate_options(&self) -> &[i32] {
        &self.report_rate_options
    }

    /// Reads the active report rate in Hz.
    pub async fn get_report_rate(&self) -> Result<i32, Error> {
        if self.ext_rate_feature_index != 0 {
            self.get_extended_report_rate().await
        } else {
            self.get_classic_report_rate().await
        }
    }

    /// Sets report rate, snapping hz to whichever supported option is closest.
    pub async fn set_report_rate(&self, hz: i32) -> Result<(), Error> {
        if self.ext_rate_feature_index != 0 {
            self.set_extended_report_rate(hz).await?;
            return Ok(());
        }
        let interval_ms = 1000 / closest_option(hz, &self.report_rate_options);
        self.conn
            .call(
                self.device_index,
                self.report_rate_feature_index,
                REPORT_RATE_FUNC_SET,
                &[interval_ms as u8],
            )
            .await?;
        Ok(())
    }

    /// Reads battery percent/charging via whichever feature the device resolved
    /// to.
    pub async fn get_battery(&self) -> Result<(i32, bool), Error> {
        battery::read_battery(
            &self.conn,
            self.device_index,
            self.battery_feature_index,
            self.battery_kind,
        )
        .await
    }

    /// Superlight models have no RGB lighting at all.
    pub fn rgb_supported(&self) -> bool {
        self.led_feature_index != 0
    }

    /// Sets the Logo LED to a static color.
    pub async fn set_color(&self, r: u8, g: u8, b: u8) -> Result<(), Error> {
        if self.led_feature_index == 0 {
            return Err(Error::invalid_operation(format!(
                "gpro: {} has no RGB lighting",
                self.name
            )));
        }
        claim_led_control(&self.conn, self.device_index, self.led_feature_index).await?;

        // zoneIndex, effectListIndex, then a 10-byte effect parameter block
        // (color R,G,B, ramp, and 6 zero-padding bytes for the Static effect).
        let payload = [
            self.led_zone_index,
            self.led_static_effect_index,
            r,
            g,
            b,
            0x00, // ramp: Default
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
        ];
        self.conn
            .call(
                self.device_index,
                self.led_feature_index,
                LED_FUNC_SET_ZONE_EFFECT,
                &payload,
            )
            .await?;
        Ok(())
    }

    /// Enumerates the reprogrammable controls.
    pub async fn get_buttons(&self) -> Result<Vec<ButtonInfo>, Error> {
        if self.buttons_feature_index == 0 {
            return Err(Error::invalid_operation(
                "gpro: device has no reprogrammable controls feature",
            ));
        }
        let resp = self
            .conn
            .call(
                self.device_index,
                self.buttons_feature_index,
                BUTTONS_FUNC_GET_COUNT,
                &[],
            )
            .await?;
        if resp.is_empty() {
            return Err(Error::invalid_data("gpro: short control count response"));
        }

        let mut controls = Vec::with_capacity(resp[0] as usize);
        for i in 0..resp[0] {
            let info = self
                .conn
                .call(
                    self.device_index,
                    self.buttons_feature_index,
                    BUTTONS_FUNC_GET_CID_INFO,
                    &[i],
                )
                .await?;
            if info.len() < 9 {
                continue;
            }
            let cid = ((info[0] as u16) << 8) | info[1] as u16;
            // Divertable flag is byte 4 | byte 8 (<<8): non-contiguous in the
            // response; replicate the Go bit assembly exactly.
            let flags = info[4] as u16 | ((info[8] as u16) << 8);
            controls.push(ButtonInfo {
                id: cid,
                name: control_name(cid),
                is_divertable: flags & KEY_FLAG_DIVERTABLE != 0,
            });
        }
        Ok(controls)
    }

    /// SetCidReporting with only the DIVERTED flag touched, and no device-side
    /// remap target (remap=0): Open G Hub always translates diverted presses
    /// itself rather than asking the device to natively remap.
    pub async fn set_button_remap(&self, cid: u16, divert: bool) -> Result<(), Error> {
        if self.buttons_feature_index == 0 {
            return Err(Error::invalid_operation(
                "gpro: device has no reprogrammable controls feature",
            ));
        }
        let mut bfield = MAPPING_FLAG_DIVERTED << 1; // the "I am specifying this bit" write-mask bit
        if divert {
            bfield |= MAPPING_FLAG_DIVERTED;
        }
        let payload = [(cid >> 8) as u8, cid as u8, bfield, 0x00, 0x00];
        self.conn
            .call(
                self.device_index,
                self.buttons_feature_index,
                BUTTONS_FUNC_SET_CID_REPORTING,
                &payload,
            )
            .await?;
        Ok(())
    }

    // ---- Classic DPI ----

    async fn get_classic_dpi(&self) -> Result<i32, Error> {
        let resp = self
            .conn
            .call(
                self.device_index,
                self.dpi_feature_index,
                DPI_FUNC_GET_DPI,
                &[DPI_SENSOR_INDEX],
            )
            .await?;
        if resp.len() < 5 {
            return Err(Error::invalid_data("gpro: short DPI response"));
        }
        let mut current = ((resp[1] as i32) << 8) | resp[2] as i32;
        if current == 0 {
            current = ((resp[3] as i32) << 8) | resp[4] as i32;
        }
        Ok(current)
    }

    // ---- Extended DPI (0x2202) ----

    async fn get_extended_dpi(&self) -> Result<i32, Error> {
        let resp = self
            .conn
            .call(
                self.device_index,
                self.ext_dpi_feature_index,
                EXT_DPI_FUNC_GET_DPI_PARAMETERS,
                &[DPI_SENSOR_INDEX],
            )
            .await?;
        if resp.len() < 10 {
            return Err(Error::invalid_data("gpro: short extended DPI response"));
        }
        let mut current = ((resp[1] as i32) << 8) | resp[2] as i32;
        if current == 0 {
            current = ((resp[3] as i32) << 8) | resp[4] as i32;
        }
        Ok(current)
    }

    async fn set_extended_dpi(&self, dpi: i32) -> Result<(), Error> {
        let dpi = clamp_to_step(dpi, self.ext_dpi.min, self.ext_dpi.max, self.ext_dpi.step);

        // The set call writes DPI and lift-off distance together, so current LOD
        // is read first and preserved.
        let mut lod = 0u8;
        if let Ok(resp) = self
            .conn
            .call(
                self.device_index,
                self.ext_dpi_feature_index,
                EXT_DPI_FUNC_GET_DPI_PARAMETERS,
                &[DPI_SENSOR_INDEX],
            )
            .await
            && resp.len() >= 10
        {
            lod = resp[9];
        }

        let y = if self.ext_dpi.has_y { dpi } else { 0 };
        let payload = [
            DPI_SENSOR_INDEX,
            (dpi >> 8) as u8,
            dpi as u8,
            (y >> 8) as u8,
            y as u8,
            lod,
        ];
        self.conn
            .call(
                self.device_index,
                self.ext_dpi_feature_index,
                EXT_DPI_FUNC_SET_DPI_PARAMETERS,
                &payload,
            )
            .await?;
        Ok(())
    }

    // ---- Classic report rate ----

    async fn get_classic_report_rate(&self) -> Result<i32, Error> {
        let resp = self
            .conn
            .call(
                self.device_index,
                self.report_rate_feature_index,
                REPORT_RATE_FUNC_GET,
                &[],
            )
            .await?;
        if resp.is_empty() || resp[0] == 0 {
            return Err(Error::invalid_data("gpro: invalid report rate response"));
        }
        Ok(1000 / resp[0] as i32)
    }

    // ---- Extended report rate (0x8061) ----

    async fn get_extended_report_rate(&self) -> Result<i32, Error> {
        let (first, second) = if self.device_index == DEVICE_INDEX_DIRECT {
            (EXT_RATE_CONN_WIRED, EXT_RATE_CONN_GAMING_WIRELESS)
        } else {
            (EXT_RATE_CONN_GAMING_WIRELESS, EXT_RATE_CONN_WIRED)
        };

        let mut last_err: Option<Error> = None;
        for conn_type in [first, second] {
            match self
                .conn
                .call(
                    self.device_index,
                    self.ext_rate_feature_index,
                    EXT_RATE_FUNC_GET_RATE,
                    &[conn_type],
                )
                .await
            {
                Ok(resp) => {
                    if resp.is_empty() || (resp[0] as usize) >= EXT_RATE_HZ.len() {
                        last_err = Some(Error::invalid_data(
                            "gpro: invalid extended report rate response",
                        ));
                        continue;
                    }
                    return Ok(EXT_RATE_HZ[resp[0] as usize]);
                }
                Err(Error::Timeout { .. }) | Err(Error::Hidpp(_)) => {
                    last_err = Some(Error::invalid_operation("gpro: get extended report rate"));
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| Error::invalid_operation("gpro: get extended report rate")))
    }

    async fn set_extended_report_rate(&self, hz: i32) -> Result<(), Error> {
        let hz = closest_option(hz, &self.report_rate_options);
        let index = EXT_RATE_HZ.iter().position(|&h| h == hz).ok_or_else(|| {
            Error::invalid_operation(format!("gpro: unsupported report rate {hz} Hz"))
        })?;
        self.conn
            .call(
                self.device_index,
                self.ext_rate_feature_index,
                EXT_RATE_FUNC_SET_RATE,
                &[index as u8],
            )
            .await?;
        Ok(())
    }
}

/// Picks the best available identifier for a receiver-paired mouse: a virtual
/// per-peripheral hidraw node (product 0x4079) often carries a real hardware ID
/// even when the receiver has no USB serial descriptor.
fn resolve_mouse_serial(backend: &HidBackend, receiver_serial: String) -> String {
    for wi in backend.enumerate(VENDOR_ID, PRODUCT_WIRED) {
        if let Some(s) = wi.serial
            && !s.is_empty()
        {
            return s;
        }
    }
    receiver_serial
}

/// Resolves every feature the driver uses: DPI and report rate are expected
/// (classic or extended), battery optional.
async fn build(
    conn: HidppConn,
    device_index: u8,
    serial: String,
    product_id: u16,
) -> Result<GProMouse, Error> {
    // DPI: classic 0x2201 or its successor 0x2202.
    let classic_dpi_idx =
        root::get_feature_index(&conn, device_index, FEATURE_ADJUSTABLE_DPI).await?;
    let (dpi_idx, ext_dpi_idx, ext_dpi) = if let Some(idx) = classic_dpi_idx {
        (
            idx,
            0,
            ExtDpiInfo {
                has_y: false,
                min: 0,
                max: 0,
                step: 0,
            },
        )
    } else {
        let ext_idx = require_feature(
            &conn,
            device_index,
            FEATURE_EXTENDED_DPI,
            "Extended Adjustable DPI",
        )
        .await?;
        let ext_dpi = discover_extended_dpi(&conn, device_index, ext_idx).await?;
        (0, ext_idx, ext_dpi)
    };

    let (battery_idx, battery_kind) = battery::resolve_battery_feature(&conn, device_index).await?;

    // Report rate: same classic-or-extended split.
    let classic_rate_idx =
        root::get_feature_index(&conn, device_index, FEATURE_REPORT_RATE).await?;
    let (rate_idx, ext_rate_idx, rate_options) = if let Some(idx) = classic_rate_idx {
        let options = get_report_rate_options(&conn, device_index, idx).await?;
        (idx, 0, options)
    } else {
        let ext_idx = require_feature(
            &conn,
            device_index,
            FEATURE_EXTENDED_REPORT_RATE,
            "Extended Report Rate",
        )
        .await?;
        let options = get_extended_report_rate_options(&conn, device_index, ext_idx).await?;
        (0, ext_idx, options)
    };

    // The logo LED is optional (Superlight models have no RGB); never blocks.
    let led_idx = root::get_feature_index(&conn, device_index, FEATURE_COLOR_LED_EFFECTS)
        .await?
        .unwrap_or(0);
    let (mut led_idx, mut led_zone_idx, mut led_static_effect_idx) = (led_idx, 0, 0);
    if led_idx != 0 {
        match discover_logo_zone(&conn, device_index, led_idx).await {
            Ok((zone, effect)) => {
                led_zone_idx = zone;
                led_static_effect_idx = effect;
            }
            Err(_) => led_idx = 0,
        }
    }

    // Button remapping is optional; not every unit exposes REPROG_CONTROLS_V4.
    let buttons_idx = discover_buttons_feature_index(&conn, device_index)
        .await
        .unwrap_or(0);

    // Prefer the device's self-reported marketing name.
    let mut name = product_name(product_id)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("G Pro ({product_id:04x})"));
    if let Ok(self_name) = name::get_device_name(&conn, device_index).await
        && !self_name.is_empty()
    {
        name = self_name;
    }

    Ok(GProMouse {
        conn,
        device_index,
        name,
        serial,
        product_id,
        dpi_feature_index: dpi_idx,
        ext_dpi_feature_index: ext_dpi_idx,
        ext_dpi,
        battery_feature_index: battery_idx,
        battery_kind,
        report_rate_feature_index: rate_idx,
        ext_rate_feature_index: ext_rate_idx,
        report_rate_options: rate_options,
        led_feature_index: led_idx,
        led_zone_index: led_zone_idx,
        led_static_effect_index: led_static_effect_idx,
        buttons_feature_index: buttons_idx,
    })
}

async fn require_feature(
    conn: &HidppConn,
    device_index: u8,
    feature_id: u16,
    name: &str,
) -> Result<u8, Error> {
    root::get_feature_index(conn, device_index, feature_id)
        .await?
        .ok_or_else(|| {
            Error::invalid_operation(format!(
                "gpro: device does not support the {name} feature (0x{feature_id:04x})"
            ))
        })
}

/// Reads the sensor's capabilities and supported DPI range.
async fn discover_extended_dpi(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
) -> Result<ExtDpiInfo, Error> {
    let caps = conn
        .call(
            device_index,
            feature_index,
            EXT_DPI_FUNC_GET_SENSOR_CAPABILITIES,
            &[DPI_SENSOR_INDEX],
        )
        .await?;
    if caps.len() < 3 {
        return Err(Error::invalid_data(
            "gpro: short extended DPI capabilities response",
        ));
    }
    let has_y = caps[2] & EXT_DPI_CAP_HAS_Y != 0;

    let mut stream = Vec::new();
    for page in 0..EXT_DPI_MAX_RANGE_PAGES {
        let resp = conn
            .call(
                device_index,
                feature_index,
                EXT_DPI_FUNC_GET_DPI_RANGES,
                &[DPI_SENSOR_INDEX, EXT_DPI_DIRECTION_X, page],
            )
            .await?;
        // The response echoes the addressing before the page body; verify it so
        // a mismatched page can't corrupt the accumulated stream.
        if resp.len() < 4
            || resp[0] != DPI_SENSOR_INDEX
            || resp[1] != EXT_DPI_DIRECTION_X
            || resp[2] != page
        {
            return Err(Error::invalid_data(
                "gpro: unexpected extended DPI ranges response",
            ));
        }
        stream.extend_from_slice(&resp[3..]);

        let (min, max, step, done) = parse_extended_dpi_ranges(&stream)?;
        if done {
            return Ok(ExtDpiInfo {
                has_y,
                min,
                max,
                step,
            });
        }
    }
    Err(Error::invalid_data(
        "gpro: extended DPI range list never terminated",
    ))
}

/// Decodes the supported-DPI word stream; `done=false` (no error) while the
/// 0x0000 terminator hasn't arrived yet.
fn parse_extended_dpi_ranges(stream: &[u8]) -> Result<(i32, i32, i32, bool), Error> {
    let mut words = Vec::new();
    let mut terminated = false;
    let mut off = 0;
    while off + 2 <= stream.len() {
        let w = ((stream[off] as u16) << 8) | stream[off + 1] as u16;
        if w == 0 {
            terminated = true;
            break;
        }
        words.push(w);
        off += 2;
    }
    if !terminated {
        return Ok((0, 0, 0, false));
    }

    let mut min = 0i32;
    let mut max = 0i32;
    let mut step = 0i32;
    let mut pending = 0u16;
    let mut has_pending = false;

    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if w >= EXT_DPI_RANGE_HYPHEN {
            let s = (w & !EXT_DPI_RANGE_HYPHEN) as i32;
            if !has_pending
                || s == 0
                || i + 1 >= words.len()
                || words[i + 1] >= EXT_DPI_RANGE_HYPHEN
            {
                return Err(Error::invalid_data(
                    "gpro: malformed extended DPI range stream",
                ));
            }
            let to = words[i + 1] as i32;
            if to < pending as i32 {
                return Err(Error::invalid_data(
                    "gpro: malformed extended DPI range stream",
                ));
            }
            if min == 0 || to < min {
                min = to;
            }
            if to > max {
                max = to;
            }
            if step == 0 || s < step {
                step = s;
            }
            pending = words[i + 1];
            i += 1;
        } else {
            if min == 0 || (w as i32) < min {
                min = w as i32;
            }
            if (w as i32) > max {
                max = w as i32;
            }
            pending = w;
            has_pending = true;
        }
        i += 1;
    }

    if max == 0 {
        return Err(Error::invalid_data("gpro: empty extended DPI range list"));
    }
    if step == 0 {
        step = 50; // fixed values only; every known gaming sensor uses 50-DPI granularity
    }
    Ok((min, max, step, true))
}

/// Queries which report intervals (1-8 ms, bitmask over bit i = interval i+1)
/// the device supports, returned as Hz ordered fastest first.
async fn get_report_rate_options(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
) -> Result<Vec<i32>, Error> {
    let resp = conn
        .call(device_index, feature_index, REPORT_RATE_FUNC_GET_LIST, &[])
        .await?;
    if resp.is_empty() {
        return Err(Error::invalid_data("gpro: short report rate list response"));
    }
    let bitmask = resp[0];
    let mut options = Vec::new();
    for i in 0..8 {
        if bitmask & (1 << i) == 0 {
            continue;
        }
        let interval_ms = i + 1;
        options.push(1000 / interval_ms);
    }
    if options.is_empty() {
        return Err(Error::invalid_data(
            "gpro: device reports no supported report rates",
        ));
    }
    Ok(options)
}

/// Reads the bitmask of rates supported on the current connection
/// (bit i = EXT_RATE_HZ[i]) as Hz, fastest first.
async fn get_extended_report_rate_options(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
) -> Result<Vec<i32>, Error> {
    let resp = conn
        .call(
            device_index,
            feature_index,
            EXT_RATE_FUNC_GET_RATE_LIST,
            &[],
        )
        .await?;
    if resp.len() < 2 {
        return Err(Error::invalid_data(
            "gpro: short extended report rate list response",
        ));
    }
    let mask = ((resp[0] as i32) << 8) | resp[1] as i32;

    let mut options = Vec::new();
    for i in (0..EXT_RATE_HZ.len()).rev() {
        if mask & (1 << i) != 0 {
            options.push(EXT_RATE_HZ[i]);
        }
    }
    if options.is_empty() {
        return Err(Error::invalid_data(
            "gpro: device reports no supported report rates",
        ));
    }
    Ok(options)
}

/// Locates the mouse's Logo LED zone and its "Static" effect slot.
async fn discover_logo_zone(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
) -> Result<(u8, u8), Error> {
    let info = conn
        .call(device_index, feature_index, LED_FUNC_GET_INFO, &[])
        .await?;
    if info.is_empty() {
        return Err(Error::invalid_data("gpro: short LED GetInfo response"));
    }
    let zone_count = info[0];

    for zone in 0..zone_count {
        let zi = conn
            .call(
                device_index,
                feature_index,
                LED_FUNC_GET_ZONE_INFO,
                &[zone, 0xff, 0x00],
            )
            .await?;
        if zi.len() < 4 {
            return Err(Error::invalid_data("gpro: short LED GetZoneInfo response"));
        }
        let location = ((zi[1] as u16) << 8) | zi[2] as u16;
        if location != LED_ZONE_LOCATION_LOGO {
            continue;
        }
        let effect_count = zi[3];

        for e in 0..effect_count {
            let ei = conn
                .call(
                    device_index,
                    feature_index,
                    LED_FUNC_GET_ZONE_EFFECT_INFO,
                    &[zone, e, 0x00],
                )
                .await?;
            if ei.len() < 4 {
                return Err(Error::invalid_data(
                    "gpro: short LED GetZoneEffectInfo response",
                ));
            }
            let effect_id = ((ei[2] as u16) << 8) | ei[3] as u16;
            if effect_id == LED_EFFECT_ID_STATIC {
                return Ok((zone, e));
            }
        }
        return Err(Error::invalid_operation(
            "gpro: Logo LED zone has no Static effect",
        ));
    }
    Err(Error::invalid_operation("gpro: no Logo LED zone found"))
}

/// Switches the LED zones to host control.
async fn claim_led_control(
    conn: &HidppConn,
    device_index: u8,
    feature_index: u8,
) -> Result<(), Error> {
    conn.call(device_index, feature_index, LED_FUNC_SET_CONTROL, &[0x01])
        .await?;
    Ok(())
}

/// Resolves REPROG_CONTROLS_V4 (0x1b04); returns 0 when absent.
async fn discover_buttons_feature_index(conn: &HidppConn, device_index: u8) -> Result<u8, Error> {
    Ok(
        root::get_feature_index(conn, device_index, FEATURE_REPROG_CONTROLS_V4)
            .await?
            .unwrap_or(0),
    )
}

fn clamp_to_step(v: i32, min: i32, max: i32, step: i32) -> i32 {
    let v = v.clamp(min, max);
    min + (v - min + step / 2) / step * step
}

fn closest_option(hz: i32, options: &[i32]) -> i32 {
    let mut best = options[0];
    for &o in options {
        if (o - hz).abs() < (best - hz).abs() {
            best = o;
        }
    }
    best
}
