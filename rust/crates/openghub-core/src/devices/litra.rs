//! Logitech Litra Glow / Litra Beam key lights. These speak a simple
//! vendor-defined USB HID protocol (not HID++): each command is a fixed
//! 20-byte output report starting with report ID 0x11 and a 0xff 0x04 feature
//! prefix, followed by a one-byte command and up to two payload bytes. There is
//! no way to read state back, so callers track last-known state themselves.
//! Ported from `internal/device/litra/litra.go`.

use std::os::fd::{AsFd, OwnedFd};

use crate::error::Error;
use crate::sys::write_all;

pub const VENDOR_ID: u16 = 0x046d;
pub const PRODUCT_GLOW: u16 = 0xc900;
pub const PRODUCT_BEAM: u16 = 0xc901;

// Raw brightness values the device accepts; exposed as 0-100 percent and
// scaled at the boundary.
const MIN_BRIGHTNESS_RAW: u8 = 0x14;
const MAX_BRIGHTNESS_RAW: u8 = 0xfa;

pub const MIN_TEMPERATURE_K: i32 = 2700;
pub const MAX_TEMPERATURE_K: i32 = 6500;

/// A Litra key light.
pub struct LitraLight {
    fd: OwnedFd,
}

impl LitraLight {
    pub fn new(fd: OwnedFd) -> Self {
        Self { fd }
    }

    pub fn friendly_name(product_id: u16) -> String {
        match product_id {
            PRODUCT_GLOW => "Litra Glow".to_string(),
            PRODUCT_BEAM => "Litra Beam".to_string(),
            _ => format!("Litra ({product_id:04x})"),
        }
    }

    pub fn set_power(&self, on: bool) -> Result<(), Error> {
        self.send(0x1c, &[if on { 0x01 } else { 0x00 }])
    }

    /// `percent` is clamped to 0-100 and scaled to the device's raw brightness
    /// range.
    pub fn set_brightness(&self, percent: i32) -> Result<(), Error> {
        let percent = percent.clamp(0, 100);
        let span = (MAX_BRIGHTNESS_RAW - MIN_BRIGHTNESS_RAW) as f32;
        let raw = MIN_BRIGHTNESS_RAW as f32 + (percent as f32 / 100.0 * span).round();
        self.send(0x4c, &[0x00, raw as u8])
    }

    /// `kelvin` is clamped to the device's supported range before being sent.
    pub fn set_temperature(&self, kelvin: i32) -> Result<(), Error> {
        let kelvin = kelvin.clamp(MIN_TEMPERATURE_K, MAX_TEMPERATURE_K);
        self.send(0x9c, &[(kelvin >> 8) as u8, kelvin as u8])
    }

    /// Builds and writes a 20-byte Litra report: report ID 0x11, the
    /// 0xff 0x04 lighting-feature prefix, a one-byte command, then payload bytes
    /// zero-padded to the device's fixed report length.
    fn send(&self, command: u8, payload: &[u8]) -> Result<(), Error> {
        let mut report = [0u8; 20];
        report[0] = 0x11;
        report[1] = 0xff;
        report[2] = 0x04;
        report[3] = command;
        report[4..4 + payload.len()].copy_from_slice(payload);
        write_all(self.fd.as_fd(), &report)
    }
}
