# LogiTux

A native GUI for controlling Logitech devices — the Logitech G HUB
equivalent, built for Linux first. Implemented as a .NET/Avalonia
application. It also builds and runs on Windows and macOS (with a
device-support caveat — see [Prebuilt packages](#prebuilt-packages)).

On Linux, LogiTux talks to hardware directly over the `hidraw` interface
and to webcams through V4L2 — no `libhidapi`, no `libv4l`, no cgo — so
installation needs nothing beyond the published single-file binary.

## Status

- **Litra Glow / Litra Beam** key lights — power, brightness, color
  temperature.
- **G Pro Wireless, PRO X Superlight, and PRO X Superlight 2** mice
  (direct or via a Lightspeed receiver) — DPI and report (polling) rate,
  logo color on units that have one (the Superlight family doesn't),
  battery, and button remapping on units whose firmware exposes the
  corresponding HID++ feature. Newer mice report DPI and report rate
  through different HID++ features than the G Pro Wireless does (up to
  8000 Hz, with DPI ranges read from the device); LogiTux uses whichever
  the connected mouse implements. **Caveat:** the Superlight-family
  support is implemented from a third-party reference implementation's
  protocol decoding and from the G Pro Wireless's own hardware, not yet
  verified against physical Superlight hardware.
- **PRO X Wireless Gaming Headset** — battery, sidetone, a full per-band
  equalizer (band count/frequencies/dB range are read from the device),
  and a hardware mic noise-reduction toggle on units whose firmware
  supports it (probed with a read; the toggle hides otherwise).
- **C920 / C920 HD Pro / C922 Pro Stream / C930e** webcams — zoom,
  pan/tilt position, autofocus and manual focus, auto and manual
  exposure, image tuning (brightness, contrast, saturation, sharpness),
  and a **live preview** streamed straight off the capture node. Webcams
  aren't HID devices, so these go through a pure .NET V4L2 layer rather
  than hidraw; which controls exist is queried from the driver per
  device. The preview negotiates MJPEG (YUYV fallback) and renders a
  640×480 feed at ~15 fps.

The UI is styled after Logitech's own G HUB, navigation included: the
**Dashboard** — a near-black home screen with a cyan-blue accent — shows
every connected device as a clickable card (product render, name, battery
level if it has one, a settings button); clicking one opens that device's
page, and the back arrow in the page's top-left returns to the Dashboard.
A device's page only exists while it's actually connected. Device pages
are laid out like G HUB's: an icon rail on the left groups each device's
settings into sections — Sensitivity (DPI, report rate), Assignments
(button remapping), Lighting (power, temperature, RGB logo color), Sound
(sidetone, mic noise reduction, equalizer), Video/Camera/Image for
webcams — and only the sections a device actually supports appear. DPI is
presented as G HUB's **DPI stages**: a row of preset buttons with a *DPI
Speeds* track plotting each stage. The webcam page carries a live
preview above the Camera and Image tabs.

## Quick start

```bash
git clone <this-repo>
cd LogiTux
./install.sh
```

`install.sh` will, on Debian/Ubuntu-based distros (including Linux Mint):

1. Check for the .NET 10 SDK and tell you how to get it if it's missing.
2. Publish the trimmed single-file binary.
3. Install it to `~/.local/bin/logitux`, a desktop launcher entry, and a
   udev rule granting your user access to supported devices' `hidraw`
   nodes and `/dev/uinput` (used only for button remapping when it lands).

After installing, **unplug and replug your device** so the new udev rule
applies, then launch LogiTux from your application menu or run
`~/.local/bin/logitux`.

To remove everything `install.sh` set up, run `./uninstall.sh`.

### Prebuilt packages

Every push builds installable packages for all three platforms in CI (see
`.github/workflows/build.yml`), and tagging a release (`vX.Y.Z`) attaches
them to a GitHub Release:

- **Debian/Ubuntu** — a `.deb` (`sudo apt install ./logitux_*_amd64.deb`),
  which drops the binary in `/usr/bin`, installs the udev rule and desktop
  entry, and reloads udev for you. A plain binary tarball is also built.
- **Windows** — a zipped `logitux.exe`.
- **macOS** — a zipped `LogiTux.app`. It's unsigned, so the first launch
  needs a right-click → **Open** to get past Gatekeeper.

> **Platform support caveat.** LogiTux was built for Linux, where device
> support is complete and hardware-tested. It also **builds and runs** on
> Windows and macOS — the full G HUB-style UI comes up — but those builds
> ship a HID backend that finds no devices yet (hidraw is Linux-only;
> the enumeration simply returns nothing elsewhere). Webcam control stays
> Linux-only regardless, since it uses V4L2.

## Usage

LogiTux polls for supported devices every few seconds. Each connected
device gets its own page, opened from its Dashboard card (and gone once
the device unplugs), with whichever controls it supports — a power
checkbox, sliders, dropdowns, a color picker. Changes are applied
immediately.

Devices that can't report their own state back over USB (Litra lights;
the G Pro Wireless's DPI/report-rate/battery/color often *can* be read
back live and are) fall back to the last value LogiTux sent, kept in
`$XDG_CONFIG_HOME/logitux/state.json` (typically
`~/.config/logitux/state.json`). Most settings are only ever *applied*
when you interact with them.

### Custom product images

Each supported product ships with an original render drawn for LogiTux
(Logitech's own product photography is copyrighted, so an MIT-licensed
project can't redistribute it). If you'd rather see the official images,
you can supply your own: drop a file into
`$XDG_CONFIG_HOME/logitux/images/` (typically `~/.config/logitux/images/`)
named after the product — lowercased, spaces as dashes — with a `.png`,
`.jpg`, `.jpeg`, or `.svg` extension, e.g. `g-pro-wireless.png`,
`litra-glow.png`, or `pro-x-wireless-gaming-headset.png`. LogiTux picks
these up at startup and uses them everywhere the built-in render would
appear. Such images are for your own local use; don't commit them to a
public fork.

### Button remapping

If the mouse's firmware exposes the `REPROG_CONTROLS` feature (0x1B00
through 0x1B04) — not universal; see [Status](#status) — its remappable
buttons each get a dropdown to retarget them to another mouse button or a
keyboard key, or restore "Default". If the feature isn't present, this
section just doesn't appear; the rest of the mouse's controls are
unaffected. The software-side emission layer (uinput) that makes remaps
work while the app runs is still to be wired up, so on the current
hardware this page carries the G HUB-spec enumeration as a stub.

## Architecture

```
src/LogiTux.Core/         Hardware library: hidraw I/O, HID++ transport,
                            device drivers, V4L2 controls + capture
src/LogiTux.Core/Hid/     Linux hidraw backend: /sys/class/hidraw discovery
                            + /dev/hidrawN I/O
src/LogiTux.Core/Hidpp/   HID++ 2.0 transport: feature calls, notifications,
                            shared battery logic, device self-naming
src/LogiTux.Core/Devices/ Device drivers: Litra / GProMouse / Prox / V4l2Camera
                            (each a sealed class owning its hidraw node or
                            /dev/videoN) + WebcamPreview streaming loop
src/LogiTux.Core/V4l2/    Pure .NET V4L2 ioctls (controls + format/buffer/
                            stream), struct layouts from the kernel ABI
src/LogiTux.Core/Config/  JSON-backed last-known-state store
src/LogiTux.App/          Avalonia GUI: dashboard, device pages, section
                            rail, webcam preview, debounced slider writes
src/LogiTux.Poc/          Console probe for hardware iteration
packaging/                nfpm .deb config, Debian postinstall, macOS plist
install/                  udev rules and .desktop launcher entry
.github/workflows/        CI: build + per-OS publishes (.deb, .exe, .app)
```

**Structure.** `LogiTux.Core` is pure .NET (BCL + P/Invoke only): each
supported product is a sealed driver class opening its own `hidraw`
node (or a `/dev/videoN` node for webcams, discovered via
`/sys/class/video4linux`). The HID++ transport (feature discovery via the
Root feature, request/response matching, unsolicited notifications) lives
once in `Hidpp/`, and both the mouse and headset drivers call into it;
battery support is likewise shared (`HidppBattery.cs`). The GUI never
references a specific product — it type-asserts each discovered device
against the capability surface and renders whatever applies. Sliders and
other hardware writes are debounced (see `DeviceCardViewModel`); reads
tolerate asleep devices and surface that via card `Status` rather than
exceptions. The webcam's live preview runs a V4L2 mmap ring in Core
(`WebcamPreview`) and renders into a WriteableBitmap in the App
(`CameraPreviewViewModel`), with both MJPEG and YUYV paths verified on
hardware.

## Development

```bash
dotnet build                          # build the solution
make build                            # alias for the above
make publish                          # single-file binary -> bin/logitux
make run                              # build and run the app
dotnet run --project src/LogiTux.Poc  # probe hardware (see AGENTS.md)
```

A console probe (`src/LogiTux.Poc`) exercises every hardware path without
the GUI: enumerate HID devices, read/write DPI, RGB, sidetone, EQ bands,
camera controls, and stream preview frames (`--preview`,
`--preview-yuyv`).

## Credit

All product artwork in LogiTux (the dashboard renders and generic device
icons) is original, drawn for this project; none of it is Logitech's
imagery. "Logitech", "G HUB", and the product names are Logitech
trademarks, used here only to identify the hardware being controlled.

The Litra USB protocol was originally reverse-engineered by
[kharyam/go-litra-driver](https://github.com/kharyam/go-litra-driver) (and
its Python predecessor, [kharyam/litra-driver](https://github.com/kharyam/litra-driver)).
LogiTux's Litra driver is an independent implementation of that protocol,
built on its own pure-BCL hidraw backend rather than `libhidapi`.

HID++ 2.0 feature byte layouts used by the G Pro Wireless driver were
verified against [pwr-Solaar/Solaar](https://github.com/pwr-Solaar/Solaar),
the most complete existing Linux HID++ implementation, rather than guessed
from general protocol documentation. The Superlight family's extended DPI
(`0x2202`) and report rate (`0x8061`) features were cross-referenced
against [AprilNEA/OpenLogi](https://github.com/AprilNEA/OpenLogi)
(MIT/Apache-2.0), an independently written Rust HID++ implementation.

LogiTux itself was previously implemented in Go; the current .NET codebase
replaced it entirely, and its V4L2/HID++ constants and quirks still follow
that verified lineage where noted in the code.

## License

MIT — see [LICENSE](LICENSE).