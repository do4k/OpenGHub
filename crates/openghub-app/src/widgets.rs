use std::sync::Arc;

use rune::tok::{motion, space, type_};
use rune::{Align, Color, Drag, El, Phase, el, row, text};

use crate::state::{Paint, State};
#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn slider(
    p: Paint,
    key: impl Into<String>,
    label: &str,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    value_text: String,
    on_change: impl Fn(&mut State, f32) + 'static,
) -> El {
    let key = key.into();
    let (sk, tk, fk, kk, vk) = (
        format!("{key}/s"),
        format!("{key}/t"),
        format!("{key}/f"),
        format!("{key}/k"),
        format!("{key}/v"),
    );
    let (w, h, k, bar) = (240.0, 20.0, 16.0, 4.0);
    let travel = w - k;
    let frac = ((value - min) / (max - min)).clamp(0.0, 1.0);
    let to_value = move |x: f32| {
        let f = ((x - k / 2.0) / travel).clamp(0.0, 1.0);
        ((min + f * (max - min)) / step).round() * step
    };
    let to_value = move |x: f32| to_value(x).clamp(min, max);

    row()
        .key(key)
        .gap(space::S3)
        .center_y()
        .child(
            el().key(sk)
                .role("slider")
                .label(label)
                .range_value(f64::from(value), f64::from(min), f64::from(max))
                .range_step(f64::from(step))
                .size(w, h)
                .motion(motion::SNAPPY)
                .on_drag(move |s: &mut State, d: &mut Drag, tx| match d.phase {
                    Phase::Down => {
                        d.data = frac;
                        let knob_x = k / 2.0 + frac * travel;
                        if (d.x - knob_x).abs() > k / 2.0 + 2.0 {
                            on_change(s, to_value(d.x));
                        }
                    }
                    Phase::Move => {
                        let knob_x = k / 2.0 + d.data * travel;
                        let grab = if (d.start_x - knob_x).abs() <= k / 2.0 + 2.0 {
                            d.start_x - knob_x
                        } else {
                            0.0
                        };
                        on_change(s, to_value(d.x - grab));
                        tx.instant();
                    }
                    Phase::Up => {
                        if d.cancelled() {
                            on_change(s, min + d.data * (max - min));
                        }
                    }
                })
                .child(
                    el().key(tk)
                        .size(w, bar)
                        .absolute(0.0, (h - bar) / 2.0)
                        .radius(bar / 2.0)
                        .fill(p.t.control_off),
                )
                .child(
                    el().key(fk)
                        .size(k / 2.0 + frac * travel, bar)
                        .absolute(0.0, (h - bar) / 2.0)
                        .radius(bar / 2.0)
                        .fill(p.accent),
                )
                .child(
                    el().key(kk)
                        .size(k, k)
                        .absolute(frac * travel, (h - k) / 2.0)
                        .radius(k / 2.0)
                        .fill(p.t.knob)
                        .shadow(1.0, 3.0, p.t.shadow),
                ),
        )
        .child(
            text(value_text)
                .key(vk)
                .w(44.0)
                .font(type_::MONO_CAPTION)
                .color(p.t.text_secondary)
                .align(Align::End),
        )
}

#[cfg(unix)]
pub(crate) fn toggle_row(
    p: Paint,
    key: impl Into<String>,
    label: &str,
    on: bool,
    flip: impl Fn(&mut State) + 'static,
) -> El {
    let key = key.into();
    let (sk, kk) = (format!("{key}/sw"), format!("{key}/knob"));
    let (w, h, k) = (38.0, 22.0, 18.0);
    let inset = (h - k) / 2.0;
    row()
        .key(key)
        .role("switch")
        .label(label)
        .checked(on)
        .h(32.0)
        .gap(space::S3)
        .center_y()
        .focusable(true)
        .on_press(flip)
        .child(text(label).key("label").font(type_::BODY).color(p.t.text))
        .child(
            el().key(sk)
                .size(w, h)
                .radius(h / 2.0)
                .fill(if on { p.accent } else { p.t.control_off })
                .motion(motion::SNAPPY)
                .child(
                    el().key(kk)
                        .size(k, k)
                        .absolute(if on { w - k - inset } else { inset }, inset)
                        .radius(k / 2.0)
                        .fill(p.t.knob)
                        .shadow(1.0, 3.0, p.t.knob_shadow),
                ),
        )
}

#[cfg(unix)]
pub(crate) fn pill(
    p: Paint,
    key: impl Into<String>,
    label: &str,
    active: bool,
    press: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .key(key)
        .role("button")
        .label(label)
        .h(30.0)
        .px(space::S3)
        .center()
        .radius(18.0)
        .fill(if active { p.accent } else { p.t.control_off })
        .hover(if active { p.accent } else { p.t.hover })
        .focusable(true)
        .on_press(press)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(if active { Color::BLACK } else { p.t.text }),
        )
}

#[cfg(unix)]
pub(crate) fn chip(p: Paint, key: impl Into<String>, label: &str) -> El {
    el().key(key)
        .h(26.0)
        .px(space::S3)
        .center()
        .radius(12.0)
        .fill(p.t.card)
        .child(text(label).font(type_::LABEL).color(p.t.text_secondary))
}

/// A slider with a left-aligned name label (RGB channels, EQ bands).
#[allow(clippy::too_many_arguments)]
#[cfg(unix)]
pub(crate) fn labeled_slider(
    p: Paint,
    key: impl Into<String>,
    label: &str,
    label_w: f32,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    value_text: String,
    on_change: impl Fn(&mut State, f32) + 'static,
) -> El {
    let key = key.into();
    let inner = format!("{key}-s");
    row()
        .key(key)
        .gap(space::S3)
        .center_y()
        .child(
            text(label)
                .font(type_::LABEL)
                .color(p.t.text_secondary)
                .w(label_w)
                .align(Align::End),
        )
        .child(slider(
            p, inner, label, value, min, max, step, value_text, on_change,
        ))
}

/// Per-speed colors in the spirit of G HUB (orange, teal, yellow, purple…),
/// assigned by stage position.
const STAGE_PALETTE: [(u8, u8, u8); 7] = [
    (255, 107, 53),
    (46, 196, 182),
    (249, 199, 79),
    (179, 136, 235),
    (241, 91, 181),
    (76, 201, 240),
    (128, 237, 153),
];

#[cfg(unix)]
fn stage_color(i: usize) -> Color {
    let (r, g, b) = STAGE_PALETTE[i % STAGE_PALETTE.len()];
    Color {
        r: rune::color::srgb_decode(r as f32 / 255.0),
        g: rune::color::srgb_decode(g as f32 / 255.0),
        b: rune::color::srgb_decode(b as f32 / 255.0),
        a: 1.0,
    }
}

/// A DPI stage track in the spirit of G HUB: every configured speed is a
/// colored dot on a logarithmic track with its value above; the active
/// speed is underlined. Click a dot (or its value) to activate it, drag
/// it to change it.
#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn dpi_track(
    p: Paint,
    key: impl Into<String>,
    stages: &[i32],
    active: i32,
    min_dpi: i32,
    max_dpi: i32,
    step: i32,
    on_activate: impl Fn(&mut State, usize) + 'static,
    on_change: impl Fn(&mut State, usize, i32) + 'static,
) -> El {
    let key = key.into();
    let (w, h, k, bar_y) = (600.0, 70.0, 12.0, 40.0);
    let travel = w - k;
    let (lo, hi) = (min_dpi as f32, max_dpi as f32);
    let frac_of = |v: i32| ((v as f32).ln() - lo.ln()) / (hi.ln() - lo.ln());
    let x_of = |v: i32| frac_of(v) * travel;
    let v_of = move |x: f32| {
        let f = ((x - k / 2.0) / travel).clamp(0.0, 1.0);
        let v = lo * (hi / lo).powf(f);
        let snapped = ((v - lo) / step as f32).round() * step as f32 + lo;
        (snapped as i32).clamp(min_dpi, max_dpi)
    };

    let on_activate = Arc::new(on_activate);
    let on_change = Arc::new(on_change);

    // The dots are visual only; the drag handler lives on the full-width
    // track root — `Drag::x` is in the *handler element's* coordinates, so a
    // handler on the 12px dot would report x ≈ 0..12 and slam every drag to
    // the track minimum (k/2, exactly like `slider` does).
    let xs: Vec<f32> = stages.iter().map(|&v| x_of(v)).collect();
    let fracs: Vec<f32> = stages.iter().map(|&v| frac_of(v)).collect();
    let activate = Arc::clone(&on_activate);
    let change = Arc::clone(&on_change);

    let mut track = el()
        .key(key.clone())
        .role("slider")
        .label("DPI speeds")
        .focusable(true)
        .motion(motion::SNAPPY)
        .range_value(f64::from(active), f64::from(min_dpi), f64::from(max_dpi))
        .range_step(f64::from(step))
        .size(w, h)
        .on_drag(move |s: &mut State, d: &mut Drag, tx| match d.phase {
            Phase::Down => {
                // Grab the nearest dot within reach. `data` encodes
                // (index, frac-at-down) as `index * 2 + frac`; -1 = nothing.
                let mut best: Option<usize> = None;
                let mut best_dist = f32::INFINITY;
                for (i, &sx) in xs.iter().enumerate() {
                    let dist = (d.x - (sx + k / 2.0)).abs();
                    if dist < best_dist {
                        best_dist = dist;
                        best = Some(i);
                    }
                }
                match best.filter(|_| best_dist <= k / 2.0 + 8.0) {
                    Some(i) => {
                        d.data = i as f32 * 2.0 + fracs[i];
                        activate(s, i);
                    }
                    None => d.data = -1.0,
                }
            }
            Phase::Move => {
                if d.data < 0.0 {
                    return;
                }
                let i = (d.data / 2.0) as usize;
                let frac = d.data - 2.0 * i as f32;
                let knob_x = k / 2.0 + frac * travel;
                let grab = if (d.start_x - knob_x).abs() <= k / 2.0 + 2.0 {
                    d.start_x - knob_x
                } else {
                    0.0
                };
                change(s, i, v_of(d.x - grab));
                tx.instant();
            }
            Phase::Up => {
                if d.cancelled() && d.data >= 0.0 {
                    // Restore the speed as it was when the drag began.
                    let i = (d.data / 2.0) as usize;
                    let frac = d.data - 2.0 * i as f32;
                    let v = (lo * (hi / lo).powf(frac)).round();
                    let snapped = ((v - lo) / step as f32).round() * step as f32 + lo;
                    change(s, i, (snapped as i32).clamp(min_dpi, max_dpi));
                }
            }
        })
        .child(
            el().key(format!("{key}/bar"))
                .size(w, 3.0)
                .absolute(0.0, bar_y)
                .radius(1.5)
                .fill(p.t.control_off),
        );

    // Tick marks + labels at G HUB-style milestones.
    for v in [100, 1000, 2000, 3000, 4000, 14800, 25600] {
        if !(min_dpi..=max_dpi).contains(&v) {
            continue;
        }
        let x = x_of(v);
        let cx = x + k / 2.0;
        track = track
            .child(
                el().key(format!("{key}/tick-{v}"))
                    .size(2.0, 6.0)
                    .absolute(cx - 1.0, bar_y + 5.0)
                    .radius(1.0)
                    .fill(p.t.control_off),
            )
            .child(
                el().key(format!("{key}/ticklbl-{v}"))
                    .size(44.0, 14.0)
                    .absolute(cx - 22.0, bar_y + 14.0)
                    .child(
                        text(v.to_string())
                            .font(type_::MONO_CAPTION)
                            .color(p.t.text_secondary)
                            .align(Align::Center),
                    ),
            );
    }

    for (i, &stage) in stages.iter().enumerate() {
        let x = x_of(stage);
        let cx = x + k / 2.0;
        let color = stage_color(i);
        let is_active = stage == active;

        // Value label above the dot (+ underline bar when active).
        let activate = Arc::clone(&on_activate);
        let mut label = el()
            .key(format!("{key}/lbl-{i}"))
            .role("button")
            .label(stage.to_string())
            .size(48.0, 16.0)
            .absolute(cx - 24.0, 8.0)
            .focusable(true)
            .on_press(move |s: &mut State| activate(s, i))
            .child(
                text(stage.to_string())
                    .font(type_::LABEL)
                    .color(color)
                    .align(Align::Center),
            );
        if is_active {
            label = label.child(
                el().key(format!("{key}/ul-{i}"))
                    .size(18.0, 2.0)
                    .absolute(15.0, 18.0)
                    .radius(1.0)
                    .fill(color),
            );
        }

        track = track.child(label).child(
            el().key(format!("{key}/dot-{i}"))
                .size(k, k)
                .absolute(x, bar_y - k / 2.0)
                .radius(k / 2.0)
                .fill(color)
                .shadow(1.0, 3.0, p.t.shadow),
        );
    }

    track
}

/// A radio choice: circular indicator + label, for discrete options like
/// report rate. The active option's circle is filled with the accent.
#[cfg(unix)]
pub(crate) fn radio(
    p: Paint,
    key: impl Into<String>,
    label: &str,
    active: bool,
    press: impl Fn(&mut State) + 'static,
) -> El {
    let key = key.into();
    let circle_key = format!("{key}/c");
    row()
        .key(key)
        .role("radio")
        .label(label)
        .checked(active)
        .gap(space::S2)
        .center_y()
        .focusable(true)
        .on_press(press)
        .child(
            el().key(circle_key)
                .size(14.0, 14.0)
                .radius(7.0)
                .border(1.5, if active { p.accent } else { p.t.control_off })
                .fill(if active { p.accent } else { Color::TRANSPARENT }),
        )
        .child(text(label).font(type_::LABEL).color(if active {
            p.t.text
        } else {
            p.t.text_secondary
        }))
}
