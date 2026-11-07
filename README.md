# Open G Hub

A native GUI for controlling Logitech devices — the Logitech G HUB
equivalent, built for Linux first. Implemented in Rust, with a native
GPU-rendered UI built on [Rune](https://github.com/anomalyco/rune).

On Linux, Open G Hub talks to hardware directly over the `hidraw` interface
and to webcams through V4L2 — no `libhidapi`, no `libv4l`, no cgo — so
installation needs nothing beyond the published binary (plus a Vulkan
driver to render).

## Status

- **Litra Glow / Litra Beam** key lights — power, brightness, color
  temperature, and one-tap presets. State is write-only over USB, so it is
  remembered in `~/.config/openghub/state.json` and re-seeded on launch.
- **G Pro Wireless, PRO X Superlight, and PRO X Superlight 2** mice
  (direct or via a Lightspeed receiver) — DPI stages and report (polling)
  rate, logo color (Static/Cycle/Breathing effects on units that expose
  them), and battery. Newer mice report DPI and report rate through
  different HID++ features than the G Pro Wireless does (up to 8000 Hz);
  Open G Hub uses whichever the connected mouse implements.
  **Caveat:** Superlight support is decoded from third-party protocol
  references and the G Pro Wireless's own hardware, not yet verified
  against physical Superlight units; the Cycle/Breathing effect writes are
  likewise unverified against powered hardware.
- **PRO X Wireless Gaming Headset** — battery, sidetone, a full per-band
  equalizer (band count/frequencies/dB range read from the device), and a
  hardware mic noise-reduction toggle on units whose firmware supports it.
- **C920 / C920 HD Pro / C922 Pro Stream / C930e** webcams — zoom, pan/tilt,
  autofocus and manual focus, auto and manual exposure, image tuning, and a
  **live preview** streamed straight off the capture node at ~15 fps.

The UI is styled after Logitech's own G HUB: the **Dashboard** shows every
connected device as a card (SVG render, name, battery); clicking one opens
that device's page with a left icon rail grouping settings into sections
(Sensitivity, Assignments, Lightsync, Acoustics, EQ, Camera/Image, …). Only
the sections a device supports appear. Button remapping (`REPROG_CONTROLS`)
stays a stub on the receiver-connected G Pro Wireless — that firmware
exposes no `0x1b04` feature, and the uinput emission layer isn't wired up.

## Quick start

```bash
git clone <this-repo>
cd OpenGHub
./install.sh
```

`install.sh` builds the release binary, installs it to
`~/.local/bin/openghub` with a desktop launcher entry, and drops a udev rule
granting access to supported `hidraw` nodes. Then **unplug and replug your
device** so the rule applies.

### Building from source

Prerequisites: Rust **1.99.1** (pinned in `rust-toolchain.toml`), a Vulkan
driver (any ICD, including software `lavapipe`), and `libxkbcommon`
(Rune links `-lxkbcommon`):

```sh
sudo apt install libxkbcommon-dev   # Debian/Ubuntu
cargo build                          # build the workspace
cargo run -p openghub-app            # launch the UI
cargo run -p openghub-poc -- --help  # console hardware probe
```

If you can't install the `-dev` package, point the linker at the runtime
library instead:

```sh
mkdir -p ~/.local/lib
ln -s /usr/lib/x86_64-linux-gnu/libxkbcommon.so.0 ~/.local/lib/libxkbcommon.so
export LIBRARY_PATH="$HOME/.local/lib"
```

### Prebuilt packages

Every push builds packages for all three platforms in CI (see
`.github/workflows/build.yml`), and tagging `vX.Y.Z` attaches them to a
GitHub Release: a Debian `.deb` (+ tarball), a zipped Windows `.exe`, and a
zipped unsigned macOS `.app`.

> **Platform support caveat.** Open G Hub is built and hardware-tested for
> Linux, where device support is complete. It also builds on Windows and
> macOS, but those builds ship a HID backend that finds no devices yet
> (`hidraw` is Linux-only); webcam control stays Linux-only regardless.

## Architecture

```
crates/openghub-core/         Hardware library: hidraw I/O, HID++ transport,
                               device drivers, V4L2 controls + capture
crates/openghub-core/src/hid/  Linux hidraw backend (sysfs discovery + /dev/hidrawN)
crates/openghub-core/src/hidpp/ HID++ 2.0 transport: feature calls, notifications,
                               battery/name helpers
crates/openghub-core/src/devices/ Drivers: Litra / GProMouse / Prox / V4l2Camera
                               + WebcamPreview streaming loop
crates/openghub-core/src/v4l2/ Raw V4L2 ioctls (controls + format/buffer/stream)
crates/openghub-core/src/config/ JSON-backed last-known-state store
crates/openghub-app/          Rune GUI: dashboard, device pages, section rail,
                               live webcam preview, hardware bridge
crates/openghub-poc/          Console probe for hardware iteration
packaging/                    nfpm .deb config, Debian postinstall, macOS plist
install/                      udev rules and .desktop launcher entry
.github/workflows/            CI: build + per-OS publishes (.deb, .exe, .app)
```

**Structure.** `openghub-core` is raw `libc`/`nix` syscalls only: each
supported product is a driver opening its own `hidraw` node (or a
`/dev/videoN` node, discovered via `/sys/class/video4linux`). The HID++
transport lives once in `hidpp/` (a `poll`-gated reader thread matching
request/response by software id, plus a notifications channel), and both the
mouse and headset drivers call into it. The Rune app bridges async HID++
calls to Rune's synchronous view loop through a background tokio runtime and
`AppHandle::update`.

## Credit

All product artwork is original, drawn for this project. "Logitech",
"G HUB", and product names are Logitech trademarks, used only to identify the
hardware being controlled.

The Litra USB protocol was reverse-engineered by
[kharyam/go-litra-driver](https://github.com/kharyam/go-litra-driver). HID++
2.0 byte layouts were verified against
[pwr-Solaar/Solaar](https://github.com/pwr-Solaar/Solaar) and
[AprilNEA/OpenLogi](https://github.com/AprilNEA/OpenLogi).

Open G Hub was previously implemented in Go, then in .NET/Avalonia; this Rust
codebase replaces both, and its V4L2/HID++ constants and quirks still follow
that verified lineage where noted in the code.

## License

MIT — see [LICENSE](LICENSE).
