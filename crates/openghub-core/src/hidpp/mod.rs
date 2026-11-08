//! Transport layer of Logitech's HID++ 2.0 protocol over a single hidraw
//! stream. Ported from `internal/hidpp/hidpp.go`; see that file's package
//! comment for the wire format. One [`HidppConn`] can carry traffic for
//! several device indices at once, as when several peripherals are paired to
//! the same wireless receiver.

pub mod battery;
pub mod name;
pub mod root;

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use tokio::sync::{mpsc, oneshot};

use crate::error::{Error, HidppError};
use crate::sys::{read_retry, write_all};

const REPORT_ID_SHORT: u8 = 0x10;
const REPORT_ID_LONG: u8 = 0x11;
const LONG_REPORT_LEN: usize = 20;

/// Addresses a device connected directly (wired USB or Bluetooth), as
/// opposed to one paired through a receiver.
pub const DEVICE_INDEX_DIRECT: u8 = 0xff;

/// The Root feature is always at feature index 0 per the spec.
pub const ROOT_FEATURE_INDEX: u8 = 0x00;

/// An unsolicited HID++ report: one that didn't match any pending call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub device_index: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CallKey {
    device_index: u8,
    feature_index: u8,
    func_sw_id: u8,
}

type CallResult = Result<Vec<u8>, Error>;

#[derive(Default)]
struct ConnState {
    closed: bool,
    timeout: Duration,
    next_sw: u8,
    pending: std::collections::HashMap<CallKey, oneshot::Sender<CallResult>>,
    reader_started: bool,
}

fn closed_error() -> Error {
    Error::Io(io::Error::new(
        io::ErrorKind::NotConnected,
        "hidpp: connection closed",
    ))
}

/// The HID++ 2.0 transport. Clone-free; one connection owns one fd.
pub struct HidppConn {
    fd: Arc<OwnedFd>,
    state: Arc<Mutex<ConnState>>,
    cancel: Arc<AtomicBool>,
    notifications_tx: mpsc::Sender<Notification>,
    notifications_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<Notification>>>,
    reader: Mutex<Option<JoinHandle<()>>>,
}

impl HidppConn {
    pub fn new(fd: OwnedFd) -> Self {
        let (tx, rx) = mpsc::channel::<Notification>(16);
        Self {
            fd: Arc::new(fd),
            state: Arc::new(Mutex::new(ConnState {
                timeout: Duration::from_secs(2),
                next_sw: 1,
                ..Default::default()
            })),
            cancel: Arc::new(AtomicBool::new(false)),
            notifications_tx: tx,
            notifications_rx: Arc::new(tokio::sync::Mutex::new(rx)),
            reader: Mutex::new(None),
        }
    }

    /// Sets how long subsequent calls wait for a response. Useful to shorten
    /// it while probing several device indices behind a receiver.
    pub fn set_timeout(&self, timeout: Duration) {
        let mut st = self.state.lock().unwrap();
        st.timeout = timeout;
    }

    /// Receives the next unsolicited notification, if any ever arrives.
    pub async fn next_notification(&self) -> Option<Notification> {
        self.notifications_rx.lock().await.recv().await
    }

    /// Invokes `function` (pre-shifted into the high nibble, e.g. 0x10 for
    /// function 1) on `feature_index` for `device_index`, returning the
    /// response's parameter bytes.
    pub async fn call(
        &self,
        device_index: u8,
        feature_index: u8,
        function: u8,
        parameters: &[u8],
    ) -> Result<Vec<u8>, Error> {
        if parameters.len() > LONG_REPORT_LEN - 4 {
            return Err(Error::invalid_data(format!(
                "too many parameters ({}) > {}",
                parameters.len(),
                LONG_REPORT_LEN - 4
            )));
        }

        let sw_id = self.alloc_software_id();
        let func_sw_id = (function & 0xf0) | sw_id;
        let key = CallKey {
            device_index,
            feature_index,
            func_sw_id,
        };

        let (tx, rx) = oneshot::channel();
        let timeout = {
            let mut st = self.state.lock().unwrap();
            if st.closed {
                return Err(Error::invalid_operation("hidpp: connection closed"));
            }
            let timeout = st.timeout;
            st.pending.insert(key, tx);
            timeout
        };

        let result = self
            .issue(
                device_index,
                feature_index,
                func_sw_id,
                parameters,
                timeout,
                rx,
            )
            .await;

        {
            let mut st = self.state.lock().unwrap();
            st.pending.remove(&key);
        }

        result
    }

    async fn issue(
        &self,
        device_index: u8,
        feature_index: u8,
        func_sw_id: u8,
        parameters: &[u8],
        timeout: Duration,
        rx: oneshot::Receiver<CallResult>,
    ) -> Result<Vec<u8>, Error> {
        let mut report = [0u8; LONG_REPORT_LEN];
        report[0] = REPORT_ID_LONG;
        report[1] = device_index;
        report[2] = feature_index;
        report[3] = func_sw_id;
        report[4..4 + parameters.len()].copy_from_slice(parameters);

        write_all(self.fd.as_fd(), &report)?;
        self.ensure_reader_started();

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(closed_error()),
            Err(_) => Err(Error::Timeout {
                device_index,
                feature_index,
                function: function_for_key(func_sw_id),
            }),
        }
    }

    fn alloc_software_id(&self) -> u8 {
        let mut st = self.state.lock().unwrap();
        let id = st.next_sw;
        st.next_sw = if st.next_sw >= 0x0f {
            1
        } else {
            st.next_sw + 1
        };
        id
    }

    /// Starts the background reader on first successful write. Dead nodes
    /// (writes fail with EPIPE/EINVAL — e.g. a webcam's interface) never get a
    /// reader, so no thread is ever stranded in an uninterruptible blocking read.
    fn ensure_reader_started(&self) {
        let mut st = self.state.lock().unwrap();
        if st.reader_started || st.closed {
            return;
        }
        st.reader_started = true;

        let fd = Arc::clone(&self.fd);
        let state = Arc::clone(&self.state);
        let cancel = Arc::clone(&self.cancel);
        let tx = self.notifications_tx.clone();

        let handle = std::thread::Builder::new()
            .name("hidpp-reader".to_string())
            .spawn(move || read_loop(fd, state, cancel, tx))
            .ok();

        *self.reader.lock().unwrap() = handle;
    }
}

impl Drop for HidppConn {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(handle) = self.reader.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
}

fn function_for_key(func_sw_id: u8) -> u8 {
    func_sw_id & 0xf0
}

/// Continuously reads reports, delivering each to whichever call is waiting,
/// or else to the notifications channel. Reads are gated on `poll(2)` with a
/// 500 ms timeout: a blocking `read()` on a char device can't be cancelled, so
/// polling keeps shutdown prompt.
fn read_loop(
    fd: Arc<OwnedFd>,
    state: Arc<Mutex<ConnState>>,
    cancel: Arc<AtomicBool>,
    tx: mpsc::Sender<Notification>,
) {
    let raw = fd.as_fd();
    let mut buffer = [0u8; LONG_REPORT_LEN];

    while !cancel.load(Ordering::Relaxed) {
        let mut pfds = [PollFd::new(raw, PollFlags::POLLIN)];
        let ready = loop {
            match poll(&mut pfds, PollTimeout::from(500u16)) {
                Ok(n) => break n,
                Err(nix::errno::Errno::EINTR) => continue,
                Err(_) => break 0,
            }
        };
        if ready == 0 {
            continue;
        }

        let revents = pfds[0].revents().unwrap_or(PollFlags::empty());
        if !revents.contains(PollFlags::POLLIN) {
            break; // POLLERR / POLLHUP / POLLNVAL — device went away
        }

        let n = match read_retry(raw, &mut buffer) {
            Ok(n) => n,
            Err(_) => break,
        };
        if n == 0 {
            break;
        }
        if n < 4 {
            continue;
        }

        let report_id = buffer[0];
        if report_id != REPORT_ID_SHORT && report_id != REPORT_ID_LONG {
            continue; // not a HID++ report, e.g. a plain HID report on a shared interface
        }
        let device_index = buffer[1];

        // HID++ 2.0 error responses are always a short report with 0xFF in the
        // feature-index position.
        if report_id == REPORT_ID_SHORT && n >= 6 && buffer[2] == 0xff {
            let error_key = CallKey {
                device_index,
                feature_index: buffer[3],
                func_sw_id: buffer[4],
            };
            deliver(
                &state,
                error_key,
                Err(Error::Hidpp(HidppError {
                    feature_index: buffer[3],
                    function: buffer[4] & 0xf0,
                    code: buffer[5],
                })),
            );
            continue;
        }

        let key = CallKey {
            device_index,
            feature_index: buffer[2],
            func_sw_id: buffer[3],
        };
        let delivered = deliver(&state, key, Ok(buffer[4..n].to_vec()));
        if !delivered {
            let _ = tx.try_send(Notification {
                device_index,
                data: buffer[..n].to_vec(),
            });
        }
    }

    shutdown(&state);
}

fn deliver(state: &Arc<Mutex<ConnState>>, key: CallKey, result: CallResult) -> bool {
    let tx = {
        let mut st = state.lock().unwrap();
        // Try exact match first.
        if let Some(tx) = st.pending.remove(&key) {
            tx
        } else {
            // Some kernel drivers (e.g. logitech-hidpp-device) translate the
            // device index in responses, so a call sent to 0xFF may come back
            // under the device's real index. Fall back to matching on
            // (feature_index, func_sw_id) alone.
            let mut found_key = None;
            for k in st.pending.keys() {
                if k.feature_index == key.feature_index && k.func_sw_id == key.func_sw_id {
                    found_key = Some(*k);
                    break;
                }
            }
            match found_key {
                Some(k) => match st.pending.remove(&k) {
                    Some(tx) => tx,
                    None => return false,
                },
                None => return false,
            }
        }
    };
    let _ = tx.send(result);
    true
}

fn shutdown(state: &Arc<Mutex<ConnState>>) {
    let to_fail = {
        let mut st = state.lock().unwrap();
        if st.closed {
            return;
        }
        st.closed = true;
        st.pending.drain().map(|(_, tx)| tx).collect::<Vec<_>>()
    };
    for tx in to_fail {
        let _ = tx.send(Err(closed_error()));
    }
}
