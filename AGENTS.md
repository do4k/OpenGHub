# AGENTS.md

LogiTux — a Logitech G HUB alternative for Linux, with hardware talked to
directly (hidraw, V4L2): no cgo, no libhidapi, no libv4l.

## Repo layout

- `internal/`, `cmd/logitux` — the original Go implementation (Fyne GUI). The
  well-tested authority for protocols, device quirks and UI design; port from
  it, don't re-derive.
- `csharp/` — the .NET port (Avalonia GUI). `src/LogiTux.Core` is the
  hardware library, `src/LogiTux.App` the UI, `src/LogiTux.Poc` a console
  probe for hardware iterations.
- `images/`, `install/`, `packaging/` — assets and packaging.

## C# conventions (csharp/)

- **Use `var` whenever the compiler allows it.** Only fall back to an
  explicit type where `var` is illegal (e.g. `[...]` collection expressions,
  `= null` initializers) or where a deconstruction needs its element names.
- **One model/type per class file**, named after the type. No multi-type
  files — records, enums and small helpers each get their own file.
- XML doc comments use block form: `/// <summary>` on its own line, body,
  then `/// </summary>` on its own line (and likewise for `<param>`/`<returns>`
  where used).
- Keep the Go porting pattern: namespaces/classes mirror the Go package
  layout (`Devices/Mouse/GProMouse.cs`, `Hidpp/HidppConn.cs`, ...); prepend
  comments with `Ported from internal/...` where useful.
- Device I/O in `LogiTux.Core` is BCL + P/Invoke only, no third-party
  libs; the UI layer (Avalonia, CommunityToolkit.Mvvm, Svg.*) stays in
  `LogiTux.App`.
- Slider-driven hardware writes are debounced (see
  `DeviceCardViewModel.DebounceWriteAsync`); reads must tolerate asleep
  devices (timeouts) and surface that via card `Status`, not exceptions.
- Everything is verified against the real hardware before commit — run
  `dotnet run --project src/LogiTux.Poc` and the app.

## Build

- `dotnet build` at `csharp/` for the working tree.
- `dotnet publish src/LogiTux.App -p:PublishProfile=linux-x64` for the
  single-file trimmed binary (`Properties/PublishProfiles/linux-x64.pubxml`).