# Open G Hub — Rust port

A Rust port of the Logitech G HUB alternative, talking to hardware directly
(hidraw, V4L2 — no libhidapi / libv4l) with a native GPU-rendered UI built on
[Rune](https://github.com/anomalyco/rune). This mirrors the existing
`.NET/Avalonia` code under `src/`; the Rust workspace lives alongside it.

## Layout

- `crates/openghub-core` — the hardware library: hidraw device I/O, the HID++
  transport (feature calls, notifications), the device drivers (G Pro mouse,
  PRO X Wireless headset, Litra lights, UVC webcams), V4L2 controls **and**
  capture streaming, plus the config store. Raw `libc`/`nix` syscalls only.
- `crates/openghub-poc` — console probe (`cargo run -p openghub-poc -- ...`).
- `crates/openghub-app` — the Rune UI: a dark G HUB-style dashboard with one
  card per device, G HUB-mirroring section pages (Sensitivity, Assignments,
  Lighting, Sound, EQ, Camera/Image tabs).

## Prerequisites

- Rust **1.99.1** (pinned in `rust-toolchain.toml`; Rune requires it).
- A Vulkan driver (any ICD, including software `lavapipe`).
- `libxkbcommon` (Rune links `-lxkbcommon`). On Debian/Ubuntu:
  `sudo apt install libxkbcommon-dev`. If you can't install the dev package,
  point the linker at the runtime library:
  ```sh
  mkdir -p ~/.local/lib
  ln -s /usr/lib/x86_64-linux-gnu/libxkbcommon.so.0 ~/.local/lib/libxkbcommon.so
  export LIBRARY_PATH="$HOME/.local/lib"
  ```

## Build & run

```sh
cd rust
cargo build                      # build everything
cargo run -p openghub-app        # launch the UI
cargo run -p openghub-poc        # probe hardware (no UI)

# Headless render (no window) — useful for checking the UI:
cargo run -p openghub-app -- --scene demo --snap /tmp/demo.png
cargo run -p openghub-app -- --snap /tmp/dash.png   # dashboard of live devices
```

### Demo scenes

`--scene demo` and `--scene demo-camera` seed cards without hardware (the
Litra writes go to `/dev/null`), so the full control surface can be inspected
on a machine with no Logitech devices attached.

### PoC flags

`--only=PATH`, `--verbose`, `--demo`, `--set-dpi=N`, `--set-rgb=RRGGBB`,
`--set-sidetone=N`, `--set-eq=db,db,...`, `--preview`, `--preview-yuyv`.

## Status

The core library and console probe are verified against real hardware (C922
webcam controls + preview, Litra Glow). The Rune UI renders the dashboard with
SVG device artwork, G HUB-style device pages, and a live webcam preview
(~15 fps, YUYV path). Mouse/headset surfaces are present and populate live once
those devices are awake.

Remaining: config-store persistence in the UI (the store itself is ported),
slider write debouncing, and live verification of the mouse/headset drivers
(those devices are asleep on the test machine).
