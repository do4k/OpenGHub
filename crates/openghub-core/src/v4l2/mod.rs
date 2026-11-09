//! Minimal Video4Linux2 device API: querying and setting controls (zoom,
//! pan/tilt, focus, exposure, image tuning) plus capture streaming
//! (`VIDIOC_S_FMT` / `REQBUFS` / `QUERYBUF` / `QBUF` / `DQBUF` / `STREAMON`),
//! mirroring `internal/v4l2/v4l2.go`. No libv4l — pure raw syscalls.

pub mod format;
pub mod mapped_buffer;

pub use format::{FormatInfo, V4l2Format};
pub use mapped_buffer::MappedBuffer;

use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};

use crate::error::Error;
use crate::sys::open_owned;

// Control IDs, from linux/v4l2-controls.h. User class is 0x00980900 + n;
// camera class is 0x009a0900 + n.
pub const CTRL_BRIGHTNESS: u32 = 0x00980900;
pub const CTRL_CONTRAST: u32 = 0x00980901;
pub const CTRL_SATURATION: u32 = 0x00980902;
pub const CTRL_SHARPNESS: u32 = 0x00980900 + 27;

pub const CTRL_EXPOSURE_AUTO: u32 = 0x009a0901;
pub const CTRL_EXPOSURE_ABSOLUTE: u32 = 0x009a0902;
pub const CTRL_PAN_ABSOLUTE: u32 = 0x009a0908;
pub const CTRL_TILT_ABSOLUTE: u32 = 0x009a0909;
pub const CTRL_FOCUS_ABSOLUTE: u32 = 0x009a090a;
pub const CTRL_FOCUS_AUTO: u32 = 0x009a090c;
pub const CTRL_ZOOM_ABSOLUTE: u32 = 0x009a090d;

/// `v4l2_exposure_auto_type` menu values for `CTRL_EXPOSURE_AUTO`.
pub const EXPOSURE_MANUAL: i32 = 1;
pub const EXPOSURE_APERTURE_PRIO: i32 = 3;

/// Capability flag (`v4l2_capability.device_caps`): the capture node among a
/// webcam's several `/dev/videoN` siblings.
pub const CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;

/// FourCC pixel formats for capture negotiation.
pub const PIX_FMT_MJPEG: u32 = 0x4750_4a4d; // 'M','J','P','G'
pub const PIX_FMT_YUYV: u32 = 0x5659_5559; // 'Y','U','Y','V'

const FLAG_DISABLED: u32 = 0x0001;

// Struct sizes fixed by the kernel ABI (64-bit).
const SIZEOF_CAPABILITY: u32 = 104;
const SIZEOF_QUERYCTRL: u32 = 68;
const SIZEOF_CONTROL: u32 = 8;

// Streaming struct sizes.
const SIZEOF_FORMAT: u32 = 208;
const SIZEOF_REQUESTBUFFERS: u32 = 20;
const SIZEOF_BUFFER: u32 = 88;

// ioctl request-code construction, mirroring the _IOC macro: dir 30-31,
// size 16-29, type 8-15, nr 0-7.
const IOC_READ: u32 = 2;
const IOC_WRITE: u32 = 1;
const IOC_TYPE_V: u32 = 0x56; // 'V'

const fn vidioc(dir: u32, nr: u32, size: u32) -> u64 {
    ((dir as u64) << 30) | ((size as u64) << 16) | ((IOC_TYPE_V as u64) << 8) | (nr as u64)
}

const VIDIOC_QUERYCAP: u64 = vidioc(IOC_READ, 0, SIZEOF_CAPABILITY);
const VIDIOC_G_CTRL: u64 = vidioc(IOC_READ | IOC_WRITE, 27, SIZEOF_CONTROL);
const VIDIOC_S_CTRL: u64 = vidioc(IOC_READ | IOC_WRITE, 28, SIZEOF_CONTROL);
const VIDIOC_QUERYCTRL: u64 = vidioc(IOC_READ | IOC_WRITE, 36, SIZEOF_QUERYCTRL);

const VIDIOC_G_FMT: u64 = vidioc(IOC_READ, 4, SIZEOF_FORMAT);
const VIDIOC_S_FMT: u64 = vidioc(IOC_READ | IOC_WRITE, 5, SIZEOF_FORMAT);

// Mode enumeration struct sizes (linux/videodev2.h: the unions are the
// 24-byte stepwise forms).
const SIZEOF_FMTDESC: u32 = 64;
const SIZEOF_FRMSIZEENUM: u32 = 44;
const SIZEOF_FRMIVALENUM: u32 = 52;
const SIZEOF_STREAMPARM: u32 = 204;

const VIDIOC_ENUM_FMT: u64 = vidioc(IOC_READ | IOC_WRITE, 2, SIZEOF_FMTDESC);
const VIDIOC_S_PARM: u64 = vidioc(IOC_READ | IOC_WRITE, 22, SIZEOF_STREAMPARM);
const VIDIOC_ENUM_FRAMESIZES: u64 = vidioc(IOC_READ | IOC_WRITE, 74, SIZEOF_FRMSIZEENUM);
const VIDIOC_ENUM_FRAMEINTERVALS: u64 = vidioc(IOC_READ | IOC_WRITE, 75, SIZEOF_FRMIVALENUM);

const FRMSIZE_DISCRETE: u32 = 1;
const FRMIVAL_DISCRETE: u32 = 1;
const VIDIOC_REQBUFS: u64 = vidioc(IOC_READ | IOC_WRITE, 8, SIZEOF_REQUESTBUFFERS);
const VIDIOC_QUERYBUF: u64 = vidioc(IOC_READ | IOC_WRITE, 9, SIZEOF_BUFFER);
const VIDIOC_QBUF: u64 = vidioc(IOC_READ | IOC_WRITE, 15, SIZEOF_BUFFER);
const VIDIOC_DQBUF: u64 = vidioc(IOC_READ | IOC_WRITE, 17, SIZEOF_BUFFER);
const VIDIOC_STREAMON: u64 = vidioc(IOC_WRITE, 18, 4);
const VIDIOC_STREAMOFF: u64 = vidioc(IOC_WRITE, 19, 4);

const BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
const MEMORY_MMAP: u32 = 1;

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn get_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn get_i32(buf: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

/// A minimal V4L2 device.
pub struct V4l2Device {
    fd: OwnedFd,
    path: String,
}

impl V4l2Device {
    pub fn open(path: &str) -> Result<Self, Error> {
        let fd = open_owned(path)?;
        Ok(Self {
            fd,
            path: path.to_string(),
        })
    }

    fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    fn ioctl(&self, request: u64, data: &mut [u8]) -> Result<(), Error> {
        let ret =
            unsafe { libc::ioctl(self.raw_fd(), request as libc::c_ulong, data.as_mut_ptr()) };
        if ret != 0 {
            let e = io::Error::last_os_error();
            return Err(Error::Io(io::Error::new(
                e.kind(),
                format!("v4l2: ioctl on {}: {e}", self.path),
            )));
        }
        Ok(())
    }

    /// Returns the device's card name and `device_caps` flags.
    pub fn query_cap(&self) -> Result<(String, u32), Error> {
        let mut buf = vec![0u8; SIZEOF_CAPABILITY as usize];
        self.ioctl(VIDIOC_QUERYCAP, &mut buf)?;
        Ok((cstring(&buf, 16, 32), get_u32(&buf, 88)))
    }

    /// Range of a control, or `None` if the device lacks it (or it is disabled).
    pub fn try_query_control(&self, id: u32) -> Result<Option<(i32, i32, i32, i32)>, Error> {
        let mut buf = vec![0u8; SIZEOF_QUERYCTRL as usize];
        put_u32(&mut buf, 0, id);
        if self.ioctl(VIDIOC_QUERYCTRL, &mut buf).is_err() {
            return Ok(None); // control not present
        }
        if get_u32(&buf, 56) & FLAG_DISABLED != 0 {
            return Ok(None); // present but disabled (e.g. auto mode owns it)
        }
        Ok(Some((
            get_i32(&buf, 40),
            get_i32(&buf, 44),
            get_i32(&buf, 48),
            get_i32(&buf, 52),
        )))
    }

    /// Reads a control's current value.
    pub fn get(&self, id: u32) -> Result<i32, Error> {
        let mut buf = vec![0u8; SIZEOF_CONTROL as usize];
        put_u32(&mut buf, 0, id);
        self.ioctl(VIDIOC_G_CTRL, &mut buf)?;
        Ok(get_i32(&buf, 4))
    }

    /// Writes a control's value.
    pub fn set(&self, id: u32, value: i32) -> Result<(), Error> {
        let mut buf = vec![0u8; SIZEOF_CONTROL as usize];
        put_u32(&mut buf, 0, id);
        put_u32(&mut buf, 4, value as u32);
        self.ioctl(VIDIOC_S_CTRL, &mut buf)
    }

    /// Asks the driver to switch the device to the given frame size and pixel
    /// format; returns what was actually negotiated.
    pub fn set_format(
        &self,
        width: i32,
        height: i32,
        pixel_format: u32,
    ) -> Result<V4l2Format, Error> {
        let mut buf = vec![0u8; SIZEOF_FORMAT as usize];
        put_u32(&mut buf, 0, BUF_TYPE_VIDEO_CAPTURE);
        put_u32(&mut buf, 8, width as u32);
        put_u32(&mut buf, 12, height as u32);
        put_u32(&mut buf, 16, pixel_format);
        self.ioctl(VIDIOC_S_FMT, &mut buf)?;
        Ok(parse_format(&buf))
    }

    /// Lists the pixel formats the driver offers for video capture.
    ///
    /// Iteration ends on the first `EINVAL` (the documented end-of-list
    /// signal); any other error aborts the enumeration.
    pub fn enumerate_formats(&self) -> Result<Vec<FormatInfo>, Error> {
        let mut out = Vec::new();
        for index in 0..16u32 {
            let mut buf = vec![0u8; SIZEOF_FMTDESC as usize];
            put_u32(&mut buf, 0, index);
            put_u32(&mut buf, 4, BUF_TYPE_VIDEO_CAPTURE);
            match self.ioctl(VIDIOC_ENUM_FMT, &mut buf) {
                Ok(()) => out.push(FormatInfo {
                    pixel_format: get_u32(&buf, 44),
                    description: cstring(&buf, 12, 32),
                }),
                Err(crate::error::Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput => {
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Lists the discrete frame sizes offered for one pixel format.
    pub fn enumerate_frame_sizes(&self, pixel_format: u32) -> Result<Vec<(i32, i32)>, Error> {
        let mut out = Vec::new();
        for index in 0..32u32 {
            let mut buf = vec![0u8; SIZEOF_FRMSIZEENUM as usize];
            put_u32(&mut buf, 0, index);
            put_u32(&mut buf, 4, pixel_format);
            match self.ioctl(VIDIOC_ENUM_FRAMESIZES, &mut buf) {
                Ok(()) => {
                    if get_u32(&buf, 8) != FRMSIZE_DISCRETE {
                        continue;
                    }
                    out.push((get_i32(&buf, 12), get_i32(&buf, 16)));
                }
                Err(crate::error::Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput => {
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Lists the discrete frame intervals (numerator, denominator) offered
    /// for one size. The frame rate is `denominator / numerator` fps.
    pub fn enumerate_frame_intervals(
        &self,
        pixel_format: u32,
        width: i32,
        height: i32,
    ) -> Result<Vec<(u32, u32)>, Error> {
        let mut out = Vec::new();
        for index in 0..32u32 {
            let mut buf = vec![0u8; SIZEOF_FRMIVALENUM as usize];
            put_u32(&mut buf, 0, index);
            put_u32(&mut buf, 4, pixel_format);
            put_u32(&mut buf, 8, width as u32);
            put_u32(&mut buf, 12, height as u32);
            match self.ioctl(VIDIOC_ENUM_FRAMEINTERVALS, &mut buf) {
                Ok(()) => {
                    if get_u32(&buf, 16) != FRMIVAL_DISCRETE {
                        continue;
                    }
                    out.push((get_u32(&buf, 20), get_u32(&buf, 24)));
                }
                Err(crate::error::Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput => {
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Sets the frame interval (seconds per frame = `numerator/denominator`).
    pub fn set_frame_interval(&self, numerator: u32, denominator: u32) -> Result<(), Error> {
        let mut buf = vec![0u8; SIZEOF_STREAMPARM as usize];
        put_u32(&mut buf, 0, BUF_TYPE_VIDEO_CAPTURE);
        put_u32(&mut buf, 12, numerator);
        put_u32(&mut buf, 16, denominator);
        self.ioctl(VIDIOC_S_PARM, &mut buf)
    }

    /// Reads the currently negotiated capture format.
    pub fn get_format(&self) -> Result<V4l2Format, Error> {
        let mut buf = vec![0u8; SIZEOF_FORMAT as usize];
        self.ioctl(VIDIOC_G_FMT, &mut buf)?;
        Ok(parse_format(&buf))
    }

    /// Allocates (or frees, with count 0) the kernel-side mmap buffer ring.
    pub fn request_buffers(&self, count: i32) -> Result<i32, Error> {
        let mut buf = vec![0u8; SIZEOF_REQUESTBUFFERS as usize];
        put_u32(&mut buf, 0, count as u32);
        put_u32(&mut buf, 4, BUF_TYPE_VIDEO_CAPTURE);
        put_u32(&mut buf, 8, MEMORY_MMAP);
        self.ioctl(VIDIOC_REQBUFS, &mut buf)?;
        Ok(get_i32(&buf, 0))
    }

    /// mmaps buffer index's region into the process.
    pub fn map_buffer(&self, index: i32) -> Result<MappedBuffer, Error> {
        let mut buf = vec![0u8; SIZEOF_BUFFER as usize];
        put_u32(&mut buf, 0, index as u32);
        put_u32(&mut buf, 4, BUF_TYPE_VIDEO_CAPTURE);
        self.ioctl(VIDIOC_QUERYBUF, &mut buf)?;
        MappedBuffer::map(
            self.raw_fd(),
            get_u32(&buf, 64) as libc::off_t,
            get_i32(&buf, 72) as usize,
            &self.path,
        )
    }

    /// Hands buffer index back to the driver for the next frame.
    pub fn queue_buffer(&self, index: i32) -> Result<(), Error> {
        let mut buf = vec![0u8; SIZEOF_BUFFER as usize];
        put_u32(&mut buf, 0, index as u32);
        put_u32(&mut buf, 4, BUF_TYPE_VIDEO_CAPTURE);
        put_u32(&mut buf, 60, MEMORY_MMAP);
        self.ioctl(VIDIOC_QBUF, &mut buf)
    }

    /// Blocks until the driver delivers a filled buffer; returns its index and
    /// the frame's byte count (`bytesused`).
    pub fn dequeue_buffer(&self) -> Result<(i32, i32), Error> {
        let mut buf = vec![0u8; SIZEOF_BUFFER as usize];
        put_u32(&mut buf, 4, BUF_TYPE_VIDEO_CAPTURE);
        self.ioctl(VIDIOC_DQBUF, &mut buf)?;
        Ok((get_i32(&buf, 0), get_i32(&buf, 8)))
    }

    /// Starts the capture pipeline.
    pub fn stream_on(&self) -> Result<(), Error> {
        let mut buf = vec![0u8; 4];
        put_u32(&mut buf, 0, BUF_TYPE_VIDEO_CAPTURE);
        self.ioctl(VIDIOC_STREAMON, &mut buf)
    }

    /// Stops the capture pipeline.
    pub fn stream_off(&self) -> Result<(), Error> {
        let mut buf = vec![0u8; 4];
        put_u32(&mut buf, 0, BUF_TYPE_VIDEO_CAPTURE);
        self.ioctl(VIDIOC_STREAMOFF, &mut buf)
    }
}

fn parse_format(buf: &[u8]) -> V4l2Format {
    V4l2Format {
        width: get_i32(buf, 8),
        height: get_i32(buf, 12),
        pixel_format: get_u32(buf, 16),
        bytes_per_line: get_i32(buf, 24),
        size_image: get_i32(buf, 28),
    }
}

fn cstring(buf: &[u8], offset: usize, length: usize) -> String {
    let end = (offset..offset + length)
        .find(|&i| buf[i] == 0)
        .unwrap_or(offset + length);
    String::from_utf8_lossy(&buf[offset..end]).into_owned()
}
