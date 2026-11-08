//! Per-control write debouncing. Slider drags fire their change callback
//! per move event; without this, every tick floods the device with HID++
//! calls. UI state stays immediate — only the hardware write (and persist)
//! defers until the control has been quiet for 100 ms.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::app::RT;

/// Quiet period before a debounced slider write fires. Caps a dragging
/// slider at ~10 hardware writes/sec.
const DEBOUNCE_DELAY: Duration = Duration::from_millis(100);

/// Pending debounced writes keyed by control id; the value is
/// (generation, task). A newer call for the same key supersedes the older.
type DebounceMap = HashMap<String, (u64, Option<JoinHandle<()>>)>;

static DEBOUNCE: LazyLock<Mutex<DebounceMap>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Runs `future` only if no newer write for `key` arrives within
/// [`DEBOUNCE_DELAY`].
pub(crate) fn debounce<Fut>(key: impl Into<String>, future: Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    let key = key.into();
    let generation = {
        let mut map = DEBOUNCE.lock().unwrap();
        let (g, pending) = map.entry(key.clone()).or_insert((0, None));
        *g += 1;
        let generation = *g;
        if let Some(h) = pending.take() {
            h.abort();
        }
        generation
    };
    RT.spawn(async move {
        tokio::time::sleep(DEBOUNCE_DELAY).await;
        let still_latest = DEBOUNCE
            .lock()
            .unwrap()
            .get(&key)
            .is_some_and(|(g, _)| *g == generation);
        if still_latest {
            future.await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::debounce;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Rapid calls for the same key: only the latest write fires.
    #[tokio::test]
    async fn superseded_writes_are_dropped() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        for v in 1..=5 {
            let calls = calls.clone();
            debounce("test-supersede", async move {
                calls.lock().unwrap().push(v);
            });
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(*calls.lock().unwrap(), vec![5]);
    }

    /// Writes that settle between calls all fire.
    #[tokio::test]
    async fn settled_writes_all_fire() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        for v in 1..=3 {
            let calls = calls.clone();
            debounce("test-settled", async move {
                calls.lock().unwrap().push(v);
            });
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(*calls.lock().unwrap(), vec![1, 2, 3]);
    }

    /// Different keys are independent.
    #[tokio::test]
    async fn keys_are_independent() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        for (key, v) in [("test-a", 1), ("test-b", 2)] {
            let calls = calls.clone();
            debounce(key, async move {
                calls.lock().unwrap().push(v);
            });
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        let mut got = calls.lock().unwrap().clone();
        got.sort_unstable();
        assert_eq!(got, vec![1, 2]);
    }
}
