//! One mmap'd capture buffer from a V4L2 buffer ring: the kernel writes frame
//! data into the region, the driver resumes owning it on `QBUF`.
//! Ported from `internal/v4l2/v4l2.go`'s mmap path (`MapBuffer`/`UnmapBuffer`).

use std::os::fd::RawFd;

use crate::error::Error;

const PROT_READ_WRITE: libc::c_int = libc::PROT_READ | libc::PROT_WRITE;

/// A single mmap'd V4L2 buffer region.
pub struct MappedBuffer {
    address: *mut u8,
    length: usize,
}

// The mmap'd region is a plain byte region shared with the kernel; it is safe
// to move the pointer across threads (the producer/consumer discipline is
// enforced by the V4L2 QBUF/DQBUF handshake in `WebcamPreview`).
unsafe impl Send for MappedBuffer {}

impl MappedBuffer {
    /// Maps buffer index's region (kernel offset and length come from
    /// `QUERYBUF`) into this process with `mmap`.
    pub fn map(fd: RawFd, offset: libc::off_t, length: usize, path: &str) -> Result<Self, Error> {
        let address = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                length,
                PROT_READ_WRITE,
                libc::MAP_SHARED,
                fd,
                offset,
            )
        };
        if address == libc::MAP_FAILED {
            return Err(Error::Io(std::io::Error::last_os_error()));
        }
        let _ = path;
        Ok(Self {
            address: address as *mut u8,
            length,
        })
    }

    /// Copies the first `count` bytes of the region into `destination`.
    pub fn read(&self, destination: &mut [u8], count: usize) -> Result<(), Error> {
        if count > self.length {
            return Err(Error::invalid_data(format!(
                "v4l2: frame of {count} bytes exceeds mapped buffer of {}",
                self.length
            )));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(self.address, destination.as_mut_ptr(), count);
        }
        Ok(())
    }
}

impl Drop for MappedBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.address as *mut libc::c_void, self.length);
        }
    }
}
