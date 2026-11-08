//! A system tray icon: live battery percentages for wireless peripherals in
//! the tooltip, plus Show/Quit actions. Backed by Rune's tray (SNI over
//! dbus on Linux); silently absent where no tray host runs.

use std::sync::OnceLock;

use rune::tray::{Tray, TrayEvent, TrayIcon, TrayItem, TraySpec};

use crate::app::HANDLE;
use crate::device::{Card, Device};

static TRAY: OnceLock<Tray> = OnceLock::new();

/// Creates the tray icon (once). Failures only log — the app runs fine
/// without a tray host.
pub(crate) fn init() {
    let spec = TraySpec::new("openghub", "Open G Hub", icon())
        .tooltip("Open G Hub")
        .menu(vec![
            TrayItem::action("show", "Show"),
            TrayItem::Separator,
            TrayItem::action("quit", "Quit"),
        ]);
    match Tray::new(spec, |event| match event {
        TrayEvent::Activate { .. } => {
            if let Some(h) = HANDLE.get() {
                h.open();
            }
        }
        TrayEvent::Menu(id) if id == "show" => {
            if let Some(h) = HANDLE.get() {
                h.open();
            }
        }
        TrayEvent::Menu(id) if id == "quit" => std::process::exit(0),
        _ => {}
    }) {
        Ok(tray) => {
            let _ = TRAY.set(tray);
        }
        Err(e) => eprintln!("openghub: no system tray ({e})"),
    }
}

/// Refreshes the tooltip + menu from the current device list: one line per
/// wireless device with its battery, as text (disabled entries).
pub(crate) fn sync(cards: &[Card]) {
    let Some(tray) = TRAY.get() else {
        return;
    };
    let mut lines = vec!["Open G Hub".to_string()];
    let mut menu = vec![TrayItem::action("show", "Show"), TrayItem::Separator];
    for card in cards {
        let battery = match &card.device {
            Device::Mouse(_, ui) => ui.battery(),
            Device::Headset(_, ui) => ui.battery(),
            _ => None,
        };
        if let Some((percent, charging)) = battery {
            let state = if charging { "charging" } else { "discharging" };
            lines.push(format!("{}: {percent}% ({state})", card.name));
            menu.push(
                TrayItem::action(
                    format!("battery-{}", card.serial),
                    format!("{} — {percent}%", card.name),
                )
                .enabled(false),
            );
        }
    }
    menu.push(TrayItem::Separator);
    menu.push(TrayItem::action("quit", "Quit"));
    tray.set_tooltip(lines.join("\n"));
    tray.set_menu(menu);
}

/// A simple 64px tray glyph: accent disc with a white core (drawn, no image
/// decoding needed).
fn icon() -> TrayIcon {
    let size = 64u32;
    let accent = rune::tok::accent(0, true);
    let white = rune::Color::WHITE;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let (c, r_out, r_in) = (size as f32 / 2.0, size as f32 * 0.42, size as f32 * 0.20);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt();
            let color = if d <= r_in {
                white
            } else if d <= r_out {
                accent
            } else {
                continue;
            };
            let o = ((y * size + x) * 4) as usize;
            pixels[o] = (color.r * 255.0) as u8;
            pixels[o + 1] = (color.g * 255.0) as u8;
            pixels[o + 2] = (color.b * 255.0) as u8;
            pixels[o + 3] = 255;
        }
    }
    TrayIcon::rgba(size, size, pixels)
}
