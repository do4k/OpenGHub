# AGENTS.md

Open G Hub — a Logitech G HUB alternative, with hardware talked to directly
(hidraw, V4L2): no cgo, no libhidapi, no libv4l. Implemented as a
. NET/Avalonia solution that replaced the repo's original Go/Fyne
implementation entirely (source layout at the repo root since then).

## Repo layout

- `src/OpenGHub.Core` — the hardware library: hidraw device I/O, the HID++
  transport (feature calls, notifications), the device drivers (G Pro
  mouse, PRO X wireless headset, Litra lights, UVC webcams), V4L2 controls
  **and** capture streaming. BCL + P/Invoke only, zero third-party libs.
  Namespaces mirror the Go lineage (`Devices/Mouse/GProMouse.cs`,
  `Hidpp/HidppConn.cs`, `V4l2/V4l2Device.cs`).
- `src/OpenGHub.App` — the Avalonia UI (CommunityToolkit.Mvvm,
  Svg.Controls.Skia.Avalonia, SkiaSharp): dark G HUB-style dashboard with
  one card per device, G HUB-mirroring section pages (Sensitivity,
  Assignments, Lighting, Sound, Camera/Image tabs), and a live 640x480
  webcam preview at ~15 fps.
- `src/OpenGHub.Poc` — console probe for hardware iterations (`dotnet run
  --project src/OpenGHub.Poc -- --only=/dev/video0 --preview [-yuyv]`,
  `--set-rgb=`, `--set-sidetone=`, `--set-eq=`).
- `install/`, `packaging/`, `.github/` — udev rule + .desktop entry, nfpm
  .deb config / macOS Info.plist / CI (Linux binary + .deb, Windows .exe,
  macOS .app, tag releases).

## C# conventions (src/)

- **Use `var` whenever the compiler allows it.** Only fall back to an
  explicit type where `var` is illegal (e.g. `[...]` collection expressions,
  `= null` initializers) or where a deconstruction needs its element names.
- **One model/type per class file**, named after the type. No multi-type
  files — records, enums and small helpers each get their own file.
- XML doc comments use block form: `/// <summary>` on its own line, body,
  then `/// </summary>` on its own line (and likewise for `<param>`/`<returns>`
  where used).
- Prepend comments with `Ported from internal/...` where the Go lineage
  matters (protocol constants, ioctl layouts, timer periods still follow
  Go's numbers — trust them over recollection).
- Device I/O in `OpenGHub.Core` is BCL + P/Invoke only, no third-party
  libs; the UI layer (Avalonia, CommunityToolkit.Mvvm, Svg.*, SkiaSharp)
  stays in `OpenGHub.App`.
- Slider-driven hardware writes are debounced (see
  `DeviceCardViewModel.DebounceWriteAsync`); reads must tolerate asleep
  devices (timeouts) and surface that via card `Status`, not exceptions.
- Everything is verified against the real hardware before commit — run
  `dotnet run --project src/OpenGHub.Poc` and the app (webcam preview
  includes the `--preview` probe for both MJPEG and forced-YUYV paths).

## Hardware, verified

G Pro Wireless (via Lightspeed receiver pid c539), PRO X Wireless Gaming
Headset (pid 0aba), Litra Glow (c900) + Beam (c901), C922 Pro Stream
(/dev/video0). Known quirks: the receiver-connected GPW firmware exposes
no buttons/remap feature (0x1b04 probe comes back empty) — the Assignments
page stays a stub for it; mic noise reduction (0x1147) times out on the
headset's current firmware, so the section hides rather than erroring.

## Build

- `dotnet build` at the repo root for the working tree.
- `dotnet publish src/OpenGHub.App -p:PublishProfile=linux-x64` for the
  single-file trimmed binary (`Properties/PublishProfiles/linux-x64.pubxml`);
  `make publish` renames the output to `bin/openghub`.
- `make install` / `./install.sh` installs binary + desktop entry + udev
  rule (`~/.local/bin`, plus sudo for `/etc/udev/rules.d`).