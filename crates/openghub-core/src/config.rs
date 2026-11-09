//! JSON-backed, serial-number-keyed table of [`DeviceState`], persisted to disk
//! on every `set`. Writes are atomic (temp file in the same directory, then
//! rename over the target). Ported from `internal/config/config.go`.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// A named macro as stored on disk: `steps` is a plain-text program
/// (`kd:30,kt:30,ku:30,d:50`); `behavior` is `once`, `repeat_hold`, or
/// `toggle`. The executor lives in the assignment engine (#15).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroRecord {
    pub name: String,
    pub behavior: String,
    pub steps: String,
}

/// Last-known state of a single device, keyed by serial number in
/// [`ConfigStore`]. Litra lights can't report their current brightness, color
/// temperature, or power state back over USB, so Open G Hub remembers the last
/// values it sent and uses them to seed the GUI and to compute relative
/// adjustments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DeviceState {
    pub power: bool,
    pub brightness: i32,
    pub temperature: i32,
    pub dpi: i32,
    #[serde(rename = "report_rate")]
    pub report_rate: i32,
    #[serde(rename = "dpi_stages")]
    pub dpi_stages: Option<Vec<i32>>,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    #[serde(rename = "button_remaps")]
    pub button_remaps: Option<HashMap<u16, u16>>,
    /// Assignment bindings `"button:layer" -> action` (`layer` 0 = base,
    /// 1 = G-Shift; action e.g. `key:E`, `cmd:Ctrl+C`, `sys:DPI Cycle`,
    /// `macro:Name`).
    #[serde(rename = "button_bindings")]
    pub button_bindings: Option<HashMap<String, String>>,
    /// The physical button acting as the G-Shift modifier (1-based).
    #[serde(rename = "gshift_button")]
    pub gshift_button: Option<u8>,
    #[serde(rename = "gshift_enabled")]
    pub gshift_enabled: bool,
    #[serde(rename = "macros")]
    pub macros: Option<Vec<MacroRecord>>,
    pub sidetone: i32,
    #[serde(rename = "equalizer_levels")]
    pub equalizer_levels: Option<Vec<i32>>,
}

/// A serial-number-keyed table of [`DeviceState`], persisted to disk.
pub struct ConfigStore {
    path: String,
    states: Mutex<HashMap<String, DeviceState>>,
}

impl ConfigStore {
    /// The path of the state file this store is backed by.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the standard location for Open G Hub's state file, honoring
    /// `XDG_CONFIG_HOME` with the usual `~/.config` fallback.
    pub fn default_path() -> String {
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            format!("{home}/.config")
        });
        format!("{config_home}/openghub/state.json")
    }

    /// Loads the state file at `path`, if it exists, and returns a store backed
    /// by it. A missing or empty file is not an error; the store starts empty
    /// and creates the file on the first `set`.
    pub fn open(path: &str) -> Result<Self, Error> {
        let store = Self {
            path: path.to_string(),
            states: Mutex::new(HashMap::new()),
        };

        if !Path::new(path).exists() {
            return Ok(store);
        }

        let data = fs::read(path).map_err(|e| {
            Error::Io(io::Error::new(
                e.kind(),
                format!("config: read {path}: {e}"),
            ))
        })?;
        if data.is_empty() {
            return Ok(store);
        }

        let states: HashMap<String, DeviceState> = serde_json::from_slice(&data)
            .map_err(|e| Error::invalid_data(format!("config: parse {path}: {e}")))?;
        *store.states.lock().unwrap() = states;
        Ok(store)
    }

    /// Returns the last-known state for `serial`, if any.
    pub fn try_get(&self, serial: &str) -> Option<DeviceState> {
        self.states.lock().unwrap().get(serial).cloned()
    }

    /// Records state for `serial` and persists the whole store to disk
    /// atomically.
    pub fn set(&self, serial: &str, state: DeviceState) -> Result<(), Error> {
        let mut states = self.states.lock().unwrap();
        states.insert(serial.to_string(), state);
        save_locked(&self.path, &states)
    }
}

/// Writes the store to disk atomically (write to a temp file in the same
/// directory, then rename over the target). Callers must hold the store lock.
fn save_locked(path: &str, states: &HashMap<String, DeviceState>) -> Result<(), Error> {
    let dir = Path::new(path).parent().ok_or_else(|| {
        Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("config: resolve directory of {path}"),
        ))
    })?;

    fs::create_dir_all(dir).map_err(|e| {
        Error::Io(io::Error::new(
            e.kind(),
            format!("config: create {}: {e}", dir.display()),
        ))
    })?;

    let data = serde_json::to_vec_pretty(states).map_err(|e| Error::Io(io::Error::other(e)))?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".state-{nanos}.json.tmp"));

    if let Err(e) = fs::write(&tmp, &data) {
        let _ = fs::remove_file(&tmp);
        return Err(Error::Io(io::Error::new(
            e.kind(),
            format!("config: write temp file: {e}"),
        )));
    }

    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(Error::Io(io::Error::new(
            e.kind(),
            format!("config: rename into place: {e}"),
        )));
    }

    Ok(())
}
