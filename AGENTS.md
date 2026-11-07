# AGENTS.md

Open G Hub — a Logitech G HUB alternative, with hardware talked to directly
(hidraw, V4L2): no libhidapi, no libv4l, no cgo. Implemented in Rust, with a
native GPU-rendered UI on [Rune](https://github.com/anomalyco/rune). This
Rust codebase replaced the repo's earlier Go/Fyne and .NET/Avalonia
implementations entirely.

## Repo layout

- `crates/openghub-core` — the hardware library: hidraw device I/O, the
  HID++ transport (feature calls, notifications), the device drivers (G Pro
  mouse, PRO X wireless headset, Litra lights, UVC webcams), V4L2 controls
  **and** capture streaming. Raw `libc`/`nix` syscalls only, zero
  third-party libs. Modules mirror the Go lineage (`src/devices/mouse.rs`,
  `src/hidpp/mod.rs`, `src/v4l2/mod.rs`).
- `crates/openghub-app` — the Rune UI: dark G HUB-style dashboard with one
  card per device, G HUB-mirroring section pages (Sensitivity, Assignments,
  Lighting, Sound, EQ, Camera/Image tabs), and a live 640×480 webcam preview
  at ~15 fps. Async HID++ calls are bridged to Rune's sync view loop via a
  background tokio runtime + `AppHandle::update`.
- `crates/openghub-poc` — console probe for hardware iterations
  (`cargo run -p openghub-poc -- --only=/dev/video0 --preview [-yuyv]`,
  `--set-rgb=`, `--set-sidetone=`, `--set-eq=`).
- `install/`, `packaging/`, `.github/` — udev rule + .desktop entry, nfpm
  .deb config / macOS Info.plist / CI (Linux binary + .deb, Windows .exe,
  macOS .app, tag releases).

## Rust conventions

- Rust **2024** edition, pinned to **1.99.1** in `rust-toolchain.toml`
  (Rune requires it). `cargo fmt` + `cargo clippy --all-targets` must be
  clean before committing.
- One module per device/concern in `openghub-core`; the app keeps the view,
  state, and widgets in `crates/openghub-app/src/main.rs`.
- Prepend comments with `Ported from internal/...` where the Go lineage
  matters (protocol constants, ioctl layouts, timer periods still follow
  Go's numbers — trust them over recollection).
- Device I/O in `openghub-core` is raw `libc`/`nix` syscalls only; the UI
  layer (Rune) stays in `openghub-app`.
- The HID++ transport reader runs in a `poll`-gated thread (500 ms) so
  shutdown stays prompt; write failures (dead nodes) never strand a reader.
- Rune is a git dependency pinned to a `dev` rev in
  `crates/openghub-app/Cargo.toml`; `dev` is never rewritten, so bump the
  rev deliberately.
- Everything is verified against the real hardware before commit — run
  `cargo run -p openghub-poc` and the app. The webcam preview's MJPEG and
  forced-YUYV paths are both exercised by `--preview` / `--preview-yuyv`.

## Build

- `cargo build` at the repo root for the working tree.
- `make publish` builds a release binary to `bin/openghub`; `make install`
  / `./install.sh` installs binary + desktop entry + udev rule
  (`~/.local/bin`, plus sudo for `/etc/udev/rules.d`).
- Rune links `-lxkbcommon`; on Debian/Ubuntu install `libxkbcommon-dev`
  (or symlink the runtime `.so.0` and add it to `LIBRARY_PATH`). A Vulkan
  driver is required to render.
- Headless UI checks (no window) use Rune's flags, e.g.
  `cargo run -p openghub-app -- --scene demo --snap /tmp/demo.png`.

## Hardware, verified

G Pro Wireless (via Lightspeed receiver pid c539), PRO X Wireless Gaming
Headset (pid 0aba), Litra Glow (c900) + Beam (c901), C922 Pro Stream
(/dev/video0). Known quirks: the receiver-connected GPW firmware exposes no
buttons/remap feature (0x1b04 probe comes back empty) — the Assignments page
stays a stub for it; mic noise reduction can time out on headset firmware
that predates it, so the toggle hides rather than erroring. The mouse's
Cycle/Breathing LED effect writes follow the 0x8070 spec but are not yet
verified against powered hardware (the test mouse sleeps).
