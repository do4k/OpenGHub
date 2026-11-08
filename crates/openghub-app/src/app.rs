//! App bootstrap: the tokio runtime, the device-state store, the entry point,
//! and the AppSpec.
#[cfg(unix)]
use std::sync::{Arc, LazyLock, OnceLock};

#[cfg(unix)]
use openghub_core::config::{ConfigStore, DeviceState};

use rune::{AppSpec, ImageCache, Key, KeyEvent, Tx};

#[cfg(unix)]
use crate::device::{demo_camera_state, demo_dash_state, demo_state, discover, poll};
use crate::state::State;
use crate::view::view;
/// One shared tokio runtime for async HID++ calls (mouse/headset) and discovery.
#[cfg(unix)]
pub(crate) static RT: LazyLock<tokio::runtime::Runtime> =
    LazyLock::new(|| tokio::runtime::Runtime::new().expect("tokio runtime"));

/// The app handle, used by the Refresh button to trigger re-discovery.
#[cfg(unix)]
pub(crate) static HANDLE: OnceLock<rune::AppHandle<State>> = OnceLock::new();

/// The persisted last-known device state, keyed by serial number.
#[cfg(unix)]
pub(crate) static STORE: LazyLock<Arc<ConfigStore>> = LazyLock::new(|| {
    let path = ConfigStore::default_path();
    match ConfigStore::open(&path) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("openghub: ignoring unreadable state file {path}: {e}");
            Arc::new(ConfigStore::open("/tmp/openghub-state.json").expect("writable state"))
        }
    }
});

/// Read-modify-write one device's saved state.
#[cfg(unix)]
pub(crate) fn persist(serial: &str, mutate: impl FnOnce(&mut DeviceState)) {
    let store = &*STORE;
    let mut state = store.try_get(serial).unwrap_or_default();
    mutate(&mut state);
    let _ = store.set(serial, state);
}
#[cfg(unix)]
pub(crate) fn on_key(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.key == Key::Escape {
        s.selected = None;
        return true;
    }
    false
}

#[cfg(not(unix))]
pub(crate) fn on_key(_s: &mut State, _ev: &KeyEvent, _tx: &mut Tx) -> bool {
    false
}

/// A Litra card backed by `/dev/null` (writes are no-ops), so the demo scene can
pub(crate) fn run() {
    let app = rune::App::new(app_spec());

    #[cfg(unix)]
    {
        let handle = app.handle();
        let _ = HANDLE.set(handle.clone());
        RT.spawn(async move {
            let cards = discover().await;
            handle.update(move |s, _| s.cards = cards);
        });
    }

    app.run();
}

#[cfg(unix)]
pub(crate) fn app_spec() -> AppSpec<State> {
    AppSpec {
        min_size: Some((760.0, 480.0)),
        scenes: &[
            ("demo", demo_state),
            ("demo-camera", demo_camera_state),
            ("demo-dash", demo_dash_state),
        ],
        setup: Some(|s, waker| s.images = Some(ImageCache::new(waker))),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("Open G Hub", (1000.0, 700.0), State::default, view)
    }
}

#[cfg(not(unix))]
pub(crate) fn app_spec() -> AppSpec<State> {
    AppSpec {
        min_size: Some((760.0, 480.0)),
        setup: Some(|s, waker| s.images = Some(ImageCache::new(waker))),
        on_key: Some(on_key),
        ..AppSpec::new("Open G Hub", (1000.0, 700.0), State::default, view)
    }
}

/// Re-runs device discovery: clears the current cards (disposing their device
/// handles) then re-discovers and installs the fresh set. This is how a device
/// that was asleep at launch (e.g. the G Pro mouse) appears once it wakes.
#[cfg(unix)]
pub(crate) fn refresh() {
    let Some(handle) = HANDLE.get().cloned() else {
        return;
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let clear = handle.clone();
    clear.update(move |s, _| {
        s.cards.clear();
        s.selected = None;
        s.preview_image = None;
        s.preview_pending = None;
        let _ = tx.send(());
    });
    RT.spawn(async move {
        // Wait for the clear to commit (old handles disposed) before opening
        // new ones, so a receiver's responses aren't stolen by a stale reader.
        let _ = rx.await;
        let cards = discover().await;
        handle.update(move |s, _| s.cards = cards);
    });
}
