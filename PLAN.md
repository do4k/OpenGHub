# Open G Hub — Feature Parity Plan vs Logitech G HUB

## Current state (v0.1)

Working: dashboard with device cards, per-device pages with section rails,
config persistence, live webcam preview, auto-refresh (60s, additive),
manual Refresh button (merge-preserving), RGB effects (Static/Cycle/Breathe),
Litra presets. Detected: G Pro Wireless mouse, PRO X Wireless headset,
Litra Glow/Beam, C922 webcam. All controllable.

### What was fixed this session

- `PollFd` copy bug (reads never happened) — mouse/headset now detect.
- Kernel `logitech-hidpp-device` index translation — responses matched.
- Refresh/periodic re-discovery no longer wipes cards or kicks you out
  of the device page; empty discovery keeps existing devices; dedupe
  prevents the same mouse appearing twice (receiver + Bluetooth paths).

## Feature comparison vs Logitech G HUB

| Area | G HUB | Open G Hub | Notes |
|------|-------|------------|-------|
| Mouse DPI stages | yes | ✅ | pills; persisted |
| Mouse report rate | yes | ✅ | 125–1000 Hz options |
| Mouse battery | yes | ✅ | readout |
| Mouse RGB | yes | ✅ | static/cycle/breathe, logo zone |
| Mouse button remapping | yes | ❌ stub | receiver exposes no 0x1b04 on this HW |
| Mouse shift DPI / angle snapping | yes | ❌ | Tier 1, HID++ 0x2201/0x2220 |
| Mouse surface tuning | yes (HERO) | ❌ | maybe unsupported on GPW |
| Mouse on-board profiles | yes | ❌ | Tier 2, feature 0x8100 |
| Headset sidetone | yes | ✅ | slider, persisted |
| Headset EQ (custom bands) | yes | ✅ | 10-band levels |
| Headset EQ presets | yes | ❌ | Tier 1 — Flat/Bass/Treble/Custom |
| Headset noise reduction | yes | ✅ | hides if firmware too old |
| Headset battery | yes | ✅ | readout |
| Headset surround (DTS X) | yes | ❌ | licensed DSP, out of scope |
| Headset Blue VO!CE / mic effects | yes | ❌ | licensed DSP, out of scope |
| Headset volume / mic gain | yes | ❌ | Tier 1 — probe HID++ features |
| Litra power/brightness/temp | yes | ✅ | sliders |
| Litra presets | yes | ✅ | Reading/Working/Daylight/Off |
| Camera preview + V4L2 controls | n/a (G HUB is capture app) | ✅ | exceeds G HUB here |
| Camera res/framerate | yes | ❌ | Tier 1, VIDIOC_S_FMT |
| Game profiles (per-app) | yes | ❌ | Tier 2, needs focus tracking |
| Macros | yes | ❌ | Tier 2, needs uinput |
| Firmware updates | yes | ❌ | Tier 2+, risky |
| LIGHTSYNC effects library | yes | partial | static/cycle/breathe only |

## Tier 1 — high value, feasible with current HID++/V4L2 layer

1. Slider write debouncing (~100 ms coalesce) — fixes HID++ call floods.
2. EQ presets (Flat / Bass Boost / Treble Boost / Custom) + persist.
3. Shift DPI toggle + value (feature 0x2201).
4. Headset volume + mic gain (probe first; hide if unsupported).
5. Camera resolution/framerate selector.
6. Mouse RGB: check for additional zones / per-zone color.

## Tier 2 — medium value, new subsystems

7. Per-app game profiles (process/focus watcher + profile store).
8. Macros with uinput injection.
9. On-board profile management.
10. LIGHTSYNC-style effects library (pulse, wave, color-cycle extras).
11. Firmware version readout (safe) before any update flow.

## Tier 3 — out of scope / licensed

- Blue VO!CE, DTS Headphone:X (licensed DSP).
- Firmware flashing.
- Windows-only G HUB agent integrations.

## Immediate next actions

- [ ] Ship + verify refresh fix live (running now).
- [ ] Debounce sliders; add EQ presets.
- [ ] Shift DPI; headset volume/mic gain probe.
- [ ] Camera resolution selector.
- [ ] Re-test Cycle/Breathe on powered mouse hardware.
