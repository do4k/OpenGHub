//! App bootstrap: the tokio runtime, the device-state store, the entry point,
//! and the AppSpec.
#[cfg(unix)]
use std::sync::{Arc, LazyLock, OnceLock};
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use openghub_core::config::{ConfigStore, DeviceState};

use rune::{AppSpec, ImageCache, Key, KeyEvent, Tx};

#[cfg(unix)]
use crate::device::{Card, demo_camera_state, demo_dash_state, demo_state, discover, poll};
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
        rune::set_app_id("openghub");
        let handle = app.handle();
        let _ = HANDLE.set(handle.clone());
        let handle_init = handle.clone();
        RT.spawn(async move {
            let cards = discover().await;
            handle_init.update(move |s, _| s.cards = cards);
        });
        // Periodic re-discovery so devices that were asleep at launch (the
        // G Pro mouse, the headset) appear automatically once they wake.
        // Additive only: never clears existing cards or changes the selected
        // page, so the user is not kicked out of a device settings page.
        let handle_periodic = handle.clone();
        RT.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let new_cards = discover().await;
                handle_periodic.update(move |s, _| {
                    // Only add devices not already present (by subtitle/serial).
                    for nc in new_cards {
                        let already = s.cards.iter().any(|c| c.subtitle == nc.subtitle);
                        if !already {
                            s.cards.push(nc);
                        }
                    }
                });
            }
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

/// Re-runs device discovery, merging the fresh set into the existing cards
/// without discarding device handles that are still valid. Preserves the
/// currently selected device page when the device is still present.
#[cfg(unix)]
pub(crate) fn refresh() {
    let Some(handle) = HANDLE.get().cloned() else {
        return;
    };
    RT.spawn(async move {
        let new_cards = discover().await;
        handle.update(move |s, _| {
            if new_cards.is_empty() {
                return; // keep existing on failed/empty discovery
            }
            // Remember the subtitle of the currently selected card.
            let selected_subtitle = s
                .selected
                .and_then(|i| s.cards.get(i))
                .map(|c| c.subtitle.clone());

            // Merge: replace existing cards in place, add new ones, drop stale.
            let mut merged: Vec<Card> = Vec::new();
            for nc in new_cards {
                // If a card with this subtitle already exists, replace it
                // (fresh handle) at the same position.
                if let Some(pos) = merged.iter().position(|c| c.subtitle == nc.subtitle) {
                    merged[pos] = nc;
                } else if let Some(pos) = s.cards.iter().position(|c| c.subtitle == nc.subtitle) {
                    // Keep position from old list for stability.
                    let _ = pos;
                    merged.push(nc);
                } else {
                    merged.push(nc);
                }
            }

            // Preserve selected index by subtitle.
            let new_selected = selected_subtitle
                .as_ref()
                .and_then(|sub| merged.iter().position(|c| &c.subtitle == sub));

            s.cards = merged;
            s.selected = new_selected;
            s.preview_image = None;
            s.preview_pending = None;
        });
    });
}
