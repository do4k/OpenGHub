//! Open G Hub core — a Logitech G HUB alternative, with hardware talked to
//! directly (hidraw, V4L2) and no libhidapi / libv4l.
//!
//! This crate is the Rust port of the `OpenGHub.Core` C# library (itself a
//! port of the original Go implementation). It mirrors the Go lineage:
//! `hid` (hidraw backend), `hidpp` (HID++ transport + features), `v4l2`
//! (Video4Linux2 controls and capture), `devices` (mouse / headset / litra /
//! webcam drivers) and `config` (persisted last-known state).

pub mod config;
pub mod devices;
pub mod error;
pub mod hid;
pub mod hidpp;
pub mod v4l2;

mod sys;

pub use error::{Error, HidppError};
