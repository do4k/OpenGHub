//! The Assignments screen: the data model, key library, plain-text macro
//! programs, and the two-column UI (category library ⇄ mouse canvas).
//!
//! Hardware writes (0x1b04 native remap / uinput engine) are NOT wired up
//! yet: the G Pro Wireless firmware exposes no reprogrammable-controls
//! feature (#25), so bindings are stored locally until the engine lands
//! (#15). The UI is honest about that.

use rune::tok::{space, type_};
use rune::{Color, El, text};

use crate::app::persist;
use crate::device::MouseUi;
use crate::state::{Paint, State};
use crate::widgets::{chip, pill, toggle_row};
use openghub_core::config::MacroRecord;

/// Physical buttons on the mouse, 1-based (MB1/MB2 = left/right click,
/// MB3 = wheel, MB4/MB5 = the two side buttons).
pub(crate) const BUTTONS: [u8; 5] = [1, 2, 3, 4, 5];

pub(crate) fn button_name(b: u8) -> &'static str {
    match b {
        1 => "MB1",
        2 => "MB2",
        3 => "MB3",
        4 => "MB4",
        5 => "MB5",
        _ => "MB?",
    }
}

/// Key table for the Keys library and the macro editor: (label, Linux
/// input-event code) from `linux/input-event-codes.h`.
pub(crate) const KEYS: [(&str, u16); 26] = [
    ("E", 18),
    ("Q", 16),
    ("R", 19),
    ("F", 33),
    ("X", 45),
    ("C", 46),
    ("A", 30),
    ("S", 31),
    ("D", 32),
    ("W", 17),
    ("Space", 57),
    ("Shift", 42),
    ("Tab", 15),
    ("Esc", 1),
    ("Enter", 28),
    ("Backspace", 14),
    ("Up", 103),
    ("Down", 108),
    ("Left", 105),
    ("Right", 106),
    ("F1", 59),
    ("F2", 60),
    ("F3", 61),
    ("F4", 62),
    ("L", 38),
    ("G", 34),
];

pub(crate) fn key_code(label: &str) -> Option<u16> {
    KEYS.iter().find(|(k, _)| *k == label).map(|(_, c)| *c)
}

/// One macro step. Text form: `kt:E` (tap), `kd:E` (down), `ku:E` (up),
/// `d:50` (delay ms), comma-separated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MacroStep {
    Tap(&'static str),
    Down(&'static str),
    Up(&'static str),
    DelayMs(u32),
}

pub(crate) fn serialize_macro(steps: &[MacroStep]) -> String {
    steps
        .iter()
        .map(|s| match s {
            MacroStep::Tap(k) => format!("kt:{k}"),
            MacroStep::Down(k) => format!("kd:{k}"),
            MacroStep::Up(k) => format!("ku:{k}"),
            MacroStep::DelayMs(ms) => format!("d:{ms}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn parse_macro(text: &str) -> Result<Vec<MacroStep>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        let (op, arg) = part
            .split_once(':')
            .ok_or_else(|| format!("bad step: {part}"))?;
        match op {
            "kt" | "kd" | "ku" => {
                key_code(arg).ok_or_else(|| format!("unknown key: {arg}"))?;
                let key = KEYS
                    .iter()
                    .find(|(k, _)| *k == arg)
                    .map(|(k, _)| *k)
                    .unwrap();
                out.push(match op {
                    "kt" => MacroStep::Tap(key),
                    "kd" => MacroStep::Down(key),
                    _ => MacroStep::Up(key),
                });
            }
            "d" => {
                let ms: u32 = arg.parse().map_err(|_| format!("bad delay: {arg}"))?;
                out.push(MacroStep::DelayMs(ms));
            }
            _ => return Err(format!("unknown op: {op}")),
        }
    }
    Ok(out)
}

/// Human label for a stored action string (`key:E` → "E").
pub(crate) fn action_label(action: &str) -> String {
    match action.split_once(':') {
        Some((_, rest)) => rest.to_string(),
        None => action.to_string(),
    }
}

/// Storage key for a button + layer (`layer`: 0 base, 1 G-Shift).
fn binding_key(button: u8, layer: u8) -> String {
    format!("{button}:{layer}")
}

fn layer_of(ui: &MouseUi) -> u8 {
    if ui.gshift_enabled { 1 } else { 0 }
}

fn current_layer_label(ui: &MouseUi) -> String {
    if ui.gshift_enabled {
        format!("G-Shift layer (hold {})", button_name(ui.gshift_button))
    } else {
        "Base layer".to_string()
    }
}

/// Writes the assignment state (bindings, G-Shift, named macros) to the
/// persisted device state.
pub(crate) fn save_assignments(s: &mut State, i: usize) {
    let serial = s.cards[i].serial.clone();
    let (_, ui) = s.cards[i].mouse_mut();
    let bindings = ui.bindings.clone();
    let gshift_enabled = ui.gshift_enabled;
    let gshift_button = ui.gshift_button;
    let macros = ui.macros.clone();
    persist(&serial, |st| {
        st.button_bindings = Some(bindings);
        st.gshift_enabled = gshift_enabled;
        st.gshift_button = Some(gshift_button);
        st.macros = Some(macros);
    });
}

// ---------------------------------------------------------------------------
// Library

/// (label, action, note-for-disabled) per library category.
fn library(category: usize, ui: &MouseUi) -> Vec<(String, Option<String>, Option<&'static str>)> {
    match category {
        0 => [
            ("Copy", "cmd:Ctrl+C"),
            ("Paste", "cmd:Ctrl+V"),
            ("Cut", "cmd:Ctrl+X"),
            ("Undo", "cmd:Ctrl+Z"),
            ("Alt+Tab", "cmd:Alt+Tab"),
            ("Show Desktop", "cmd:Super+D"),
            ("Select All", "cmd:Ctrl+A"),
            ("Find", "cmd:Ctrl+F"),
        ]
        .iter()
        .map(|(l, a)| (l.to_string(), Some(a.to_string()), None))
        .collect(),
        1 => KEYS
            .iter()
            .map(|(l, _)| (l.to_string(), Some(format!("key:{l}")), None))
            .collect(),
        2 => [
            ("Mute Discord Mic", "act:discord-mute"),
            ("Toggle OBS Record", "act:obs-record"),
            ("Deafen", "act:discord-deafen"),
        ]
        .iter()
        .map(|(l, a)| {
            (
                l.to_string(),
                Some(a.to_string()),
                Some("needs the background agent"),
            )
        })
        .collect(),
        3 => [
            "DPI Cycle",
            "DPI Shift (Sniper)",
            "Profile Cycle",
            "Battery Status",
            "Play/Pause",
            "Volume Up",
            "Volume Down",
            "Next Track",
        ]
        .iter()
        .map(|l| (l.to_string(), Some(format!("sys:{l}")), None))
        .collect(),
        4 => ui
            .macros
            .iter()
            .map(|m| (m.name.clone(), Some(format!("macro:{}", m.name)), None))
            .collect(),
        _ => Vec::new(),
    }
}

const CATEGORIES: [&str; 5] = ["Commands", "Keys", "Actions", "System", "Macros"];

// ---------------------------------------------------------------------------
// View

pub(crate) fn assignments_view(s: &State, i: usize, p: Paint, scale: f32) -> El {
    let t = p.t;
    let card = &s.cards[i];
    let ui = match &card.device {
        super::Device::Mouse(_, ui) => ui,
        _ => return rune::el(),
    };

    let mut root = rune::row().key("assignments").gap(space::S3);

    // Left: library.
    let mut left = rune::col().key("lib").w(230.0).gap(space::S2);
    left = left
        .child(
            text("ASSIGNMENTS")
                .key("title")
                .font(super::content::heading())
                .color(t.text),
        )
        .child(
            text("Bind this mouse's buttons to keys, commands, macros or system actions.")
                .key("desc")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(current_layer_label(ui))
                .key("layer")
                .font(type_::LABEL)
                .color(p.accent),
        );

    let mut cats = rune::row().key("cats").gap(space::S2).flex_wrap();
    for (k, name) in CATEGORIES.iter().enumerate() {
        cats = cats.child(pill(
            p,
            format!("cat-{k}"),
            name,
            ui.assign_category == k,
            move |s: &mut State| s.cards[i].mouse_mut().1.assign_category = k,
        ));
    }
    left = left.child(cats);

    // Pending pick context (click-to-bind flow).
    if let Some(action) = &ui.pending_action {
        left = left.child(
            text(format!(
                "Click a mouse button to bind: {}",
                action_label(action)
            ))
            .key("pending")
            .font(type_::LABEL)
            .color(p.accent)
            .wrap(),
        );
    }

    let items = library(ui.assign_category, ui);
    let mut col_items = rune::col().key("items").gap(2.0);
    for (k, (label, action, note)) in items.iter().enumerate() {
        match note {
            Some(why) => {
                col_items =
                    col_items.child(chip(p, format!("item-{k}"), &format!("{label} ({why})")));
            }
            None => {
                let is_pending = ui.pending_action.as_deref() == action.as_deref();
                let action = std::sync::Arc::new(action.clone().unwrap_or_default());
                col_items = col_items.child(pill(p, format!("item-{k}"), label, is_pending, {
                    let action = std::sync::Arc::clone(&action);
                    move |s: &mut State| {
                        let (_, ui) = s.cards[i].mouse_mut();
                        if ui.pending_action.as_deref() == Some(action.as_str()) {
                            ui.pending_action = None;
                        } else {
                            ui.pending_action = Some((*action).to_string());
                        }
                    }
                }));
            }
        }
    }
    left = left.child(col_items);

    if ui.assign_category == 4 {
        left = left.child(macro_editor(s, i, p));
    }
    root = root.child(left);

    // Right: mouse canvas.
    let mut right = rune::col().key("canvas").grow(1.0).gap(space::S2);
    right = right.child(
        text("MOUSE")
            .key("canvas-title")
            .font(super::content::heading())
            .color(t.text),
    );
    right = right.child(canvas(s, i, p, scale));
    // G-Shift controls.
    right = right.child(toggle_row(
        p,
        "gshift",
        &format!("G-Shift ({})", button_name(ui.gshift_button)),
        ui.gshift_enabled,
        move |s: &mut State| {
            let (_, ui) = s.cards[i].mouse_mut();
            ui.gshift_enabled = !ui.gshift_enabled;
            ui.pending_action = None;
            save_assignments(s, i);
        },
    ));
    let mut mods = rune::row().key("gshift-mod").gap(space::S2);
    for b in BUTTONS {
        mods = mods.child(pill(
            p,
            format!("mod-{b}"),
            button_name(b),
            ui.gshift_button == b,
            move |s: &mut State| {
                let (_, ui) = s.cards[i].mouse_mut();
                ui.gshift_button = b;
                save_assignments(s, i);
            },
        ));
    }
    right = right.child(mods);
    right = right.child(
        text("Bindings are stored locally. Native button remap (0x1b04) is not exposed by this unit's firmware, so they are not yet written to the mouse — the software engine ships next (#15).")
            .key("note")
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap(),
    );
    root.child(right)
}

/// The mouse diagram with clickable button hotspots. Each hotspot shows the
/// button name + its binding for the currently viewed layer (a floating tag,
/// per the canvas spec).
/// One canvas click: binds the pending library action (current layer), or
/// selects the button (and unbinds on the second click).
fn canvas_click(s: &mut State, i: usize, b: u8) {
    let pending = s.cards[i].mouse_mut().1.pending_action.take();
    if let Some(action) = pending {
        let (_, ui) = s.cards[i].mouse_mut();
        let layer = layer_of(ui);
        if ui.gshift_enabled && b == ui.gshift_button {
            ui.pending_action = Some(action); // the modifier can't hold a G-Shift binding
            return;
        }
        ui.bindings.insert(binding_key(b, layer), action);
        save_assignments(s, i);
        return;
    }
    let (_, ui) = s.cards[i].mouse_mut();
    if ui.selected_button == Some(b) {
        let layer = layer_of(ui);
        ui.bindings.remove(&binding_key(b, layer));
        ui.selected_button = None;
        save_assignments(s, i);
    } else {
        ui.selected_button = Some(b);
    }
}

/// The mouse diagram with clickable button hotspots. Each hotspot is a
/// floating tag: button name + its binding for the viewed layer.
fn canvas(s: &State, i: usize, p: Paint, scale: f32) -> El {
    let t = p.t;
    let (w, h) = (240.0, 276.0);
    let art = (!s.cards[i].art.is_empty())
        .then_some(s.images.as_ref())
        .flatten()
        .map(|im| im.svg(s.cards[i].art, 200.0, scale));
    let ui = match &s.cards[i].device {
        super::Device::Mouse(_, ui) => ui,
        _ => return rune::el(),
    };
    let layer = layer_of(ui);

    // Hotspots in display points (SVG 200x230 at 1.2x): (button, cx, cy, w, h).
    let hits: [(u8, f32, f32, f32, f32); 5] = [
        (1, 84.0, 72.0, 64.0, 56.0),
        (2, 156.0, 72.0, 64.0, 56.0),
        (3, 120.0, 86.0, 28.0, 44.0),
        (4, 47.0, 121.0, 30.0, 38.0),
        (5, 47.0, 144.0, 30.0, 38.0),
    ];

    let mut board = rune::el().key("board").size(w, h);
    if let Some(image) = art {
        board = board.child(
            rune::el()
                .size(w, h)
                .pixels_fit(image.pixels(), rune::SurfaceFit::Fit),
        );
    }
    for (b, cx, cy, hw, hh) in hits {
        let bound = ui.bindings.get(&binding_key(b, layer)).cloned();
        let selected = ui.selected_button == Some(b);
        let tag = bound
            .as_deref()
            .map(action_label)
            .unwrap_or_else(|| "—".to_string());
        let ink = if selected { Color::BLACK } else { t.text };
        board = board.child(
            rune::row()
                .key(format!("hit-{b}"))
                .role("button")
                .label(button_name(b))
                .size(hw, hh)
                .absolute(cx - hw / 2.0, cy - hh / 2.0)
                .center()
                .radius(10.0)
                .fill(if selected { p.accent } else { t.card })
                .border(1.0, t.card_border)
                .focusable(true)
                .on_press(move |s: &mut State| canvas_click(s, i, b))
                .child(
                    rune::col()
                        .center()
                        .child(text(button_name(b)).font(type_::LABEL).color(ink))
                        .child(text(tag).font(type_::BODY).color(ink).wrap()),
                ),
        );
    }
    rune::row().justify_center().child(board)
}

// ---------------------------------------------------------------------------
// Macro editor

fn describe_step(step: &MacroStep) -> String {
    match step {
        MacroStep::Tap(k) => format!("tap {k}"),
        MacroStep::Down(k) => format!("hold {k}"),
        MacroStep::Up(k) => format!("release {k}"),
        MacroStep::DelayMs(ms) => format!("wait {ms} ms"),
    }
}

const BEHAVIORS: [(&str, &str); 3] = [
    ("once", "No Repeat"),
    ("repeat_hold", "Repeat While Holding"),
    ("toggle", "Toggle"),
];

fn macro_editor(s: &State, i: usize, p: Paint) -> El {
    let t = p.t;
    let ui = match &s.cards[i].device {
        super::Device::Mouse(_, ui) => ui,
        _ => return rune::el(),
    };

    let mut c = rune::col().key("macro-edit").gap(space::S2).pt(space::S3);
    // Saved macros + new.
    let mut list = rune::row().key("macros").gap(space::S2).flex_wrap();
    for (k, m) in ui.macros.iter().enumerate() {
        list = list.child(pill(
            p,
            format!("macro-{k}"),
            &m.name,
            ui.editing_macro == Some(k),
            move |s: &mut State| {
                let (_, ui) = s.cards[i].mouse_mut();
                ui.editing_macro = Some(k);
                ui.draft_steps = parse_macro(&ui.macros[k].steps).unwrap_or_default();
                ui.draft_behavior = ui.macros[k].behavior.clone();
            },
        ));
    }
    list = list.child(pill(
        p,
        "macro-new",
        "+ New",
        ui.editing_macro.is_none(),
        move |s: &mut State| {
            let (_, ui) = s.cards[i].mouse_mut();
            ui.editing_macro = None;
            ui.draft_steps = Vec::new();
        },
    ));
    c = c.child(list);

    // Behavior.
    let mut beh = rune::row().key("behaviors").gap(space::S2).flex_wrap();
    for (k, (value, label)) in BEHAVIORS.iter().enumerate() {
        let value = *value;
        beh = beh.child(pill(
            p,
            format!("beh-{k}"),
            label,
            ui.draft_behavior == *value,
            move |s: &mut State| s.cards[i].mouse_mut().1.draft_behavior = value.to_string(),
        ));
    }
    c = c.child(beh);

    // Steps with delete.
    let mut steps = rune::col().key("steps").gap(2.0);
    for (k, step) in ui.draft_steps.iter().enumerate() {
        steps = steps.child(
            rune::row()
                .key(format!("step-{k}"))
                .gap(space::S2)
                .center_y()
                .child(text(describe_step(step)).font(type_::LABEL).color(t.text))
                .child(pill(
                    p,
                    format!("step-{k}-del"),
                    "✕",
                    false,
                    move |s: &mut State| {
                        let (_, ui) = s.cards[i].mouse_mut();
                        ui.draft_steps.remove(k);
                    },
                )),
        );
    }
    if ui.draft_steps.is_empty() {
        steps = steps.child(
            text("Empty — add a step below.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    c = c.child(steps);

    // Add form: op, then key or delay.
    let mut ops = rune::row().key("ops").gap(space::S2);
    for (k, op) in ["tap", "down", "up", "delay"].iter().enumerate() {
        let op = *op;
        ops = ops.child(pill(
            p,
            format!("op-{k}"),
            op,
            ui.draft_op == op,
            move |s: &mut State| s.cards[i].mouse_mut().1.draft_op = op,
        ));
    }
    c = c.child(ops);

    if ui.draft_op == "delay" {
        let mut delays = rune::row().key("delays").gap(space::S2);
        for d in [25u32, 50, 100, 250, 500] {
            delays = delays.child(pill(
                p,
                format!("delay-{d}"),
                &format!("{d} ms"),
                ui.draft_delay == d,
                move |s: &mut State| s.cards[i].mouse_mut().1.draft_delay = d,
            ));
        }
        c = c.child(delays);
    } else {
        let mut keys = rune::row().key("keys").gap(space::S2).flex_wrap();
        for (k, (label, _)) in KEYS.iter().enumerate() {
            let label = *label;
            keys = keys.child(pill(
                p,
                format!("key-{k}"),
                label,
                ui.draft_key == label,
                move |s: &mut State| s.cards[i].mouse_mut().1.draft_key = label,
            ));
        }
        c = c.child(keys);
    }

    // Add + save (+ delete when editing).
    let mut actions = rune::row().key("macro-actions").gap(space::S2);
    actions = actions.child(pill(
        p,
        "add-step",
        "Add step",
        false,
        move |s: &mut State| {
            let (draft_op, draft_key, draft_delay) = {
                let (_, ui) = s.cards[i].mouse_mut();
                (ui.draft_op, ui.draft_key, ui.draft_delay)
            };
            let (_, ui) = s.cards[i].mouse_mut();
            ui.draft_steps.push(match draft_op {
                "tap" => MacroStep::Tap(draft_key),
                "down" => MacroStep::Down(draft_key),
                "up" => MacroStep::Up(draft_key),
                _ => MacroStep::DelayMs(draft_delay),
            });
        },
    ));
    actions = actions.child(pill(
        p,
        "save-macro",
        "Save macro",
        true,
        move |s: &mut State| {
            let (_, ui) = s.cards[i].mouse_mut();
            let record = MacroRecord {
                name: ui.editing_macro.map_or_else(
                    || {
                        let mut n = ui.macros.len() + 1;
                        while ui.macros.iter().any(|m| m.name == format!("Macro {n}")) {
                            n += 1;
                        }
                        format!("Macro {n}")
                    },
                    |k| ui.macros[k].name.clone(),
                ),
                behavior: ui.draft_behavior.clone(),
                steps: serialize_macro(&ui.draft_steps),
            };
            match ui.editing_macro {
                Some(k) => ui.macros[k] = record,
                None => ui.macros.push(record),
            }
            ui.editing_macro = Some(ui.macros.len() - 1);
            save_assignments(s, i);
        },
    ));
    if let Some(k) = ui.editing_macro {
        actions = actions.child(pill(
            p,
            "delete-macro",
            "Delete",
            false,
            move |s: &mut State| {
                let (_, ui) = s.cards[i].mouse_mut();
                if ui.editing_macro == Some(k) {
                    ui.macros.remove(k);
                }
                ui.editing_macro = None;
                ui.draft_steps = Vec::new();
                save_assignments(s, i);
            },
        ));
    }
    c.child(actions)
}

#[cfg(test)]
mod tests {
    use super::{KEYS, MacroStep, action_label, key_code, parse_macro, serialize_macro};

    #[test]
    fn macro_serialize_roundtrip() {
        let steps = vec![
            MacroStep::Tap("E"),
            MacroStep::DelayMs(50),
            MacroStep::Down("R"),
            MacroStep::Up("R"),
        ];
        assert_eq!(serialize_macro(&steps), "kt:E,d:50,kd:R,ku:R");
        assert_eq!(parse_macro(&serialize_macro(&steps)), Ok(steps));
    }

    #[test]
    fn macro_parse_rejects_garbage() {
        assert_eq!(parse_macro(""), Ok(Vec::new()));
        assert!(parse_macro("bogus").is_err());
        assert!(parse_macro("kt:ZZZ").is_err());
        assert!(parse_macro("d:abc").is_err());
        assert_eq!(parse_macro(" d:10 "), Ok(vec![MacroStep::DelayMs(10)]));
    }

    #[test]
    fn key_table_is_complete() {
        assert_eq!(key_code("E"), Some(18));
        assert_eq!(key_code("R"), Some(19));
        assert_eq!(key_code("Space"), Some(57));
        assert!(KEYS.iter().all(|(_, c)| *c > 0));
    }

    #[test]
    fn action_labels_strip_kinds() {
        assert_eq!(action_label("key:E"), "E");
        assert_eq!(action_label("cmd:Ctrl+C"), "Ctrl+C");
        assert_eq!(action_label("plain"), "plain");
    }
}
