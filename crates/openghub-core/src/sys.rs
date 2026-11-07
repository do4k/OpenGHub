//! Shared raw-syscall helpers (blocking read/write with EINTR retry, and fd
//! opening).

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use nix::fcntl::{OFlag, open};
use nix::sys::stat::Mode;
use nix::unistd::{read, write};

use crate::error::Error;

/// Opens `path` read/write and takes ownership of the returned fd.
pub(crate) fn open_owned(path: &str) -> Result<OwnedFd, Error> {
    let raw = open(path, OFlag::O_RDWR, Mode::empty())?;
    // Safety: `open` has just returned a fresh, valid fd owned by this process.
    Ok(unsafe { OwnedFd::from_raw_fd(raw) })
}

/// Writes `buf` fully, retrying partial writes and `EINTR`.
pub(crate) fn write_all(fd: BorrowedFd<'_>, mut buf: &[u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        match write(fd, buf) {
            Ok(0) => {
                return Err(Error::Io(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "short write",
                )));
            }
            Ok(n) => buf = &buf[n..],
            Err(nix::errno::Errno::EINTR) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Reads into `buf`, retrying `EINTR`.
pub(crate) fn read_retry(fd: BorrowedFd<'_>, buf: &mut [u8]) -> Result<usize, Error> {
    loop {
        match read(fd.as_raw_fd(), buf) {
            Err(nix::errno::Errno::EINTR) => continue,
            Ok(n) => return Ok(n),
            Err(e) => return Err(e.into()),
        }
    }
}
