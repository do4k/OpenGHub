//! Control device plugin for Logitech UVC webcams (C920 family, C922, C930e),
//! ported from `internal/device/webcam`. Not a HID device: connected cameras are
//! found by scanning `/sys/class/video4linux` modalias files and controlled
//! through `/dev/videoN` with the V4L2 ioctls in [`crate::v4l2::V4l2Device`].
//! Control ranges are queried per device, never assumed.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::error::Error;
use crate::v4l2::{self, MappedBuffer, PIX_FMT_MJPEG, PIX_FMT_YUYV, V4l2Device};

pub const VENDOR_ID: u16 = 0x046d;

const CLASS_PATH: &str = "/sys/class/video4linux";
const DEV_PATH: &str = "/dev";

/// Known Logitech UVC webcams (name allowlist; ranges come from the driver).
fn product_name(product_id: u16) -> Option<&'static str> {
    match product_id {
        0x082d => Some("C920"),
        0x0892 => Some("C920 HD Pro"),
        0x085c => Some("C922 Pro Stream"),
        0x0843 => Some("C930e"),
        _ => None,
    }
}

/// Every control we know how to render, in display order. Names are stable
/// identifiers the GUI keys on.
const CANDIDATE_CONTROLS: [(&str, u32); 11] = [
    ("Zoom", v4l2::CTRL_ZOOM_ABSOLUTE),
    ("Pan", v4l2::CTRL_PAN_ABSOLUTE),
    ("Tilt", v4l2::CTRL_TILT_ABSOLUTE),
    ("Auto Focus", v4l2::CTRL_FOCUS_AUTO),
    ("Focus", v4l2::CTRL_FOCUS_ABSOLUTE),
    ("Auto Exposure", v4l2::CTRL_EXPOSURE_AUTO),
    ("Exposure", v4l2::CTRL_EXPOSURE_ABSOLUTE),
    ("Brightness", v4l2::CTRL_BRIGHTNESS),
    ("Contrast", v4l2::CTRL_CONTRAST),
    ("Saturation", v4l2::CTRL_SATURATION),
    ("Sharpness", v4l2::CTRL_SHARPNESS),
];

/// One camera control: name, driver-reported range and default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraControl {
    pub name: String,
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub default: i32,
}

/// One published preview frame from [`WebcamPreview`].
#[derive(Debug, Clone)]
pub struct PreviewFrame {
    pub pixels: Vec<u8>,
    pub width: i32,
    pub height: i32,
    pub is_jpeg: bool,
    pub sequence: u64,
}

/// A Logitech UVC webcam.
pub struct V4l2Camera {
    dev: Arc<V4l2Device>,
    ids: HashMap<String, u32>,
    name: String,
    serial: String,
    product_id: u16,
    device_path: String,
    controls: Vec<CameraControl>,
}

impl V4l2Camera {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    pub fn device_path(&self) -> &str {
        &self.device_path
    }

    pub fn controls(&self) -> &[CameraControl] {
        &self.controls
    }

    /// A live preview sharing this camera's already-open device node.
    pub fn create_preview(&self) -> WebcamPreview {
        WebcamPreview::new(Arc::clone(&self.dev), self.name.clone())
    }

    /// Product name for a videoN node (for error cards), or `None` if the node
    /// isn't a known Logitech camera.
    pub fn product_name_for_node(node: &str) -> Option<String> {
        let modalias_path = format!("{CLASS_PATH}/{node}/device/modalias");
        let (vendor, product) = try_usb_ids(&modalias_path)?;
        (vendor == VENDOR_ID)
            .then(|| product_name(product))
            .flatten()
            .map(|s| s.to_string())
    }

    /// Opens one device node. Returns `None` for nodes that belong to a known
    /// camera but aren't its video-capture interface (UVC also exposes metadata
    /// nodes).
    pub fn open(node: &str) -> Result<Option<Self>, Error> {
        let path = format!("{DEV_PATH}/{node}");

        // Find out who owns this node without opening it: the USB vendor/product
        // live in the device's modalias (usb:vVENDpPROD...).
        let modalias_path = format!("{CLASS_PATH}/{node}/device/modalias");
        let Some((vendor, product)) = try_usb_ids(&modalias_path) else {
            return Ok(None);
        };
        if vendor != VENDOR_ID {
            return Ok(None);
        }
        let Some(product_name) = product_name(product) else {
            return Ok(None);
        };

        let dev = Arc::new(V4l2Device::open(&path)?);
        let (_, caps) = dev.query_cap()?;
        if caps & v4l2::CAP_VIDEO_CAPTURE == 0 {
            return Ok(None); // e.g. a UVC metadata node, not the capture interface
        }

        let mut controls = Vec::new();
        let mut ids = HashMap::new();
        for (name, id) in CANDIDATE_CONTROLS {
            let Some((mut min, mut max, mut step, mut default)) = dev.try_query_control(id)? else {
                continue; // this unit doesn't have the control; not an error
            };
            if id == v4l2::CTRL_EXPOSURE_AUTO {
                // Driver reports a menu (1=manual, 3=aperture-priority auto);
                // present it as a boolean.
                (min, max, step, default) = (0, 1, 1, 1);
            }
            controls.push(CameraControl {
                name: name.to_string(),
                min,
                max,
                step,
                default,
            });
            ids.insert(name.to_string(), id);
        }
        if controls.is_empty() {
            return Ok(None);
        }

        Ok(Some(Self {
            dev,
            ids,
            name: product_name.to_string(),
            serial: serial_for(node),
            product_id: product,
            device_path: path,
            controls,
        }))
    }

    /// Enumerates known Logitech webcam nodes via `/sys/class/video4linux`.
    /// Returns nodes of matching cameras, capture or not — callers `open()` them
    /// and get `None` back from the non-capture ones.
    pub fn enumerate_nodes() -> Vec<String> {
        let mut nodes = Vec::new();
        let class_path = Path::new(CLASS_PATH);
        let entries = match fs::read_dir(class_path) {
            Ok(e) => e,
            Err(_) => return nodes,
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let modalias_path = entry.path().join("device").join("modalias");
            if let Some((vendor, product)) = try_usb_ids(modalias_path.to_str().unwrap_or_default())
                && vendor == VENDOR_ID
                && product_name(product).is_some()
            {
                nodes.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        nodes
    }

    pub fn get_control(&self, name: &str) -> Result<i32, Error> {
        let id = self.ids[name];
        let value = self.dev.get(id)?;
        if id == v4l2::CTRL_EXPOSURE_AUTO {
            Ok(if value == v4l2::EXPOSURE_MANUAL { 0 } else { 1 })
        } else {
            Ok(value)
        }
    }

    pub fn set_control(&self, name: &str, value: i32) -> Result<(), Error> {
        let id = self.ids[name];
        let value = if id == v4l2::CTRL_EXPOSURE_AUTO {
            if value == 0 {
                v4l2::EXPOSURE_MANUAL
            } else {
                v4l2::EXPOSURE_APERTURE_PRIO
            }
        } else {
            value
        };
        self.dev.set(id, value)
    }
}

/// Parses a sysfs modalias like "usb:v046Dp085Cd0016..." into vendor/product IDs.
fn try_usb_ids(modalias_path: &str) -> Option<(u16, u16)> {
    let s = fs::read_to_string(modalias_path).ok()?;
    let s = s.trim();
    if !s.starts_with("usb:v") || s.len() < 14 || s.as_bytes()[9] != b'p' {
        return None;
    }
    let vendor = u16::from_str_radix(&s[5..9], 16).ok()?;
    let product = u16::from_str_radix(&s[10..14], 16).ok()?;
    Some((vendor, product))
}

/// Reads the USB device serial from sysfs: the videoN symlink points at the USB
/// interface directory; the serial lives one level up. Falls back to the node.
fn serial_for(node: &str) -> String {
    let path = format!("{CLASS_PATH}/{node}/device/../serial");
    if let Ok(s) = fs::read_to_string(&path) {
        let s = s.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    node.to_string()
}

/// Requested preview frame size.
pub const PREVIEW_WIDTH: i32 = 640;
pub const PREVIEW_HEIGHT: i32 = 480;

const PREVIEW_BUFFER_COUNT: i32 = 4;

#[derive(Default)]
struct PreviewState {
    running: bool,
    buffers: Vec<MappedBuffer>,
    frame_pixels: Vec<u8>,
    frame_width: i32,
    frame_height: i32,
    frame_is_jpeg: bool,
    frame_seq: u64,
}

/// A live viewfinder on top of a [`V4l2Device`]: negotiates 640x480 (MJPEG,
/// falling back to whatever the driver gives us), streams through a four-buffer
/// mmap ring and publishes every decoded frame for the UI to poll.
pub struct WebcamPreview {
    dev: Arc<V4l2Device>,
    name: String,
    state: Arc<Mutex<PreviewState>>,
    capture: Mutex<Option<JoinHandle<()>>>,
}

impl WebcamPreview {
    fn new(dev: Arc<V4l2Device>, name: String) -> Self {
        Self {
            dev,
            name,
            state: Arc::new(Mutex::new(PreviewState::default())),
            capture: Mutex::new(None),
        }
    }

    /// Starts streaming; a second call while running is a no-op.
    pub fn start(&self, pixel_format: u32) -> Result<(), Error> {
        let mut st = self.state.lock().unwrap();
        if st.running {
            return Ok(());
        }

        let format = match self
            .dev
            .set_format(PREVIEW_WIDTH, PREVIEW_HEIGHT, pixel_format)
        {
            Ok(f) => f,
            Err(Error::Io(_)) => {
                // Some drivers (notably PipeWire's shim) reject SetFormat but
                // honour GetFormat; accept what we actually got.
                self.dev.get_format()?
            }
            Err(e) => return Err(e),
        };

        match format.pixel_format {
            PIX_FMT_MJPEG | PIX_FMT_YUYV => {}
            other => {
                return Err(Error::invalid_operation(format!(
                    "webcam: {} has no usable preview pixel format (negotiated 0x{other:08x})",
                    self.name
                )));
            }
        }

        if let Err(e) = setup_ring(&self.dev, &self.name, &mut st) {
            release_ring(&self.dev, &mut st);
            return Err(e);
        }
        st.running = true;

        let dev = Arc::clone(&self.dev);
        let state = Arc::clone(&self.state);
        let (width, height, pfmt) = (format.width, format.height, format.pixel_format);
        let handle = std::thread::Builder::new()
            .name("webcam-capture".to_string())
            .spawn(move || capture_loop(dev, state, width, height, pfmt))
            .map_err(|e| Error::Io(io::Error::other(e)))?;
        *self.capture.lock().unwrap() = Some(handle);

        Ok(())
    }

    /// Stops streaming and frees the ring; concurrent Start/Stop are safe, and a
    /// stop while never started is a no-op.
    pub fn stop(&self) {
        let task = {
            let mut st = self.state.lock().unwrap();
            if !st.running {
                return;
            }
            st.running = false;
            let _ = self.dev.stream_off();
            self.capture.lock().unwrap().take()
        };
        if let Some(handle) = task {
            let _ = handle.join();
        }
        let mut st = self.state.lock().unwrap();
        release_ring(&self.dev, &mut st);
        st.frame_pixels = Vec::new();
        st.frame_width = 0;
        st.frame_height = 0;
        st.frame_is_jpeg = false;
        st.frame_seq = 0;
    }

    /// Copies the newest frame, if any arrived; returns `None` until the first
    /// frame is published.
    pub fn try_get_frame(&self) -> Option<PreviewFrame> {
        let st = self.state.lock().unwrap();
        if st.frame_seq == 0 {
            return None;
        }
        Some(PreviewFrame {
            pixels: st.frame_pixels.clone(),
            width: st.frame_width,
            height: st.frame_height,
            is_jpeg: st.frame_is_jpeg,
            sequence: st.frame_seq,
        })
    }
}

impl Drop for WebcamPreview {
    fn drop(&mut self) {
        self.stop();
    }
}

fn setup_ring(dev: &V4l2Device, name: &str, st: &mut PreviewState) -> Result<(), Error> {
    let granted = dev.request_buffers(PREVIEW_BUFFER_COUNT)?;
    if granted != PREVIEW_BUFFER_COUNT {
        return Err(Error::Io(io::Error::other(format!(
            "webcam: {name} granted only {granted}/{PREVIEW_BUFFER_COUNT} buffers"
        ))));
    }
    for i in 0..granted {
        st.buffers.push(dev.map_buffer(i)?);
        dev.queue_buffer(i)?;
    }
    dev.stream_on()
}

fn release_ring(dev: &V4l2Device, st: &mut PreviewState) {
    st.buffers.clear(); // munmap via Drop
    let _ = dev.request_buffers(0);
}

fn capture_loop(
    dev: Arc<V4l2Device>,
    state: Arc<Mutex<PreviewState>>,
    width: i32,
    height: i32,
    pixel_format: u32,
) {
    loop {
        let (index, bytes_used) = match dev.dequeue_buffer() {
            Ok(v) => v,
            Err(_) => return, // stream stopped or device lost
        };

        {
            let mut st = state.lock().unwrap();
            if index >= 0 && (index as usize) < st.buffers.len() {
                let mut raw = vec![0u8; bytes_used as usize];
                if st.buffers[index as usize]
                    .read(&mut raw, bytes_used as usize)
                    .is_ok()
                {
                    if pixel_format == PIX_FMT_MJPEG {
                        st.frame_pixels = raw;
                        st.frame_is_jpeg = true;
                    } else {
                        st.frame_pixels = decode_yuyv_to_bgra(&raw, width, height);
                        st.frame_is_jpeg = false;
                    }
                    st.frame_width = width;
                    st.frame_height = height;
                    st.frame_seq += 1;
                }
            }
        }

        if dev.queue_buffer(index).is_err() {
            return;
        }
    }
}

/// Converts a YUYV (4:2:2 pack) frame to BGRA, two pixels per four bytes, with
/// ITU-R BT.601 luma-to-RGB.
fn decode_yuyv_to_bgra(yuyv: &[u8], width: i32, height: i32) -> Vec<u8> {
    let mut bgra = vec![0u8; (width * height * 4) as usize];
    let mut src = 0usize;
    let mut dst = 0usize;
    for _ in 0..(width * height / 2) {
        let y0 = yuyv[src];
        src += 1;
        let u = yuyv[src];
        src += 1;
        let y1 = yuyv[src];
        src += 1;
        let v = yuyv[src];
        src += 1;
        write_pixel(&mut bgra, dst, y0, u, v);
        dst += 4;
        write_pixel(&mut bgra, dst, y1, u, v);
        dst += 4;
    }
    bgra
}

fn write_pixel(bgra: &mut [u8], offset: usize, y: u8, u: u8, v: u8) {
    let c = y as i32 - 16;
    let d = u as i32 - 128;
    let e = v as i32 - 128;
    let r = (298 * c + 409 * e + 128) >> 8;
    let g = (298 * c - 100 * d - 208 * e + 128) >> 8;
    let b = (298 * c + 516 * d + 128) >> 8;
    bgra[offset] = b.clamp(0, 255) as u8;
    bgra[offset + 1] = g.clamp(0, 255) as u8;
    bgra[offset + 2] = r.clamp(0, 255) as u8;
    bgra[offset + 3] = 255;
}
