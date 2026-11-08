//! Error types shared across the core crate, mirroring the C# port's typed
//! exceptions (`TimeoutException`, `HidppErrorException`, `InvalidDataException`,
//! `IOException`, `InvalidOperationException`).

use std::io;

/// A HID++ error response: the device rejected a call instead of returning
/// a normal result. `function` is in pre-shifted form (0x00, 0x10, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidppError {
    pub feature_index: u8,
    pub function: u8,
    pub code: u8,
}

impl std::fmt::Display for HidppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hidpp: device error 0x{:02x} (feature index 0x{:02x}, function 0x{:02x})",
            self.code, self.feature_index, self.function
        )
    }
}

impl std::error::Error for HidppError {}

/// The unified error type for hardware operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "hidpp: timed out waiting for response (device 0x{device_index:02x}, feature index 0x{feature_index:02x}, function 0x{function:x})"
    )]
    Timeout {
        device_index: u8,
        feature_index: u8,
        function: u8,
    },

    #[error(transparent)]
    Hidpp(#[from] HidppError),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("{0}")]
    InvalidOperation(String),
}

impl Error {
    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Error::InvalidData(msg.into())
    }

    pub fn invalid_operation(msg: impl Into<String>) -> Self {
        Error::InvalidOperation(msg.into())
    }
}

#[cfg(unix)]
impl From<nix::errno::Errno> for Error {
    fn from(e: nix::errno::Errno) -> Self {
        Error::Io(io::Error::from(e))
    }
}
