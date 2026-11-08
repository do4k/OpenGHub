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
