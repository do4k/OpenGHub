use rune::tok::{self, TextStyle, radius, space, type_};
use rune::{Color, El, Image, ImageCache, ScrollSpec, SurfaceFit, col, el, row, text};

use super::{
    CamControl, CameraUi, Card, Device, HeadsetUi, LitraUi, MouseUi, RgbEffect, default_stages,
    display_name, sections,
};
use crate::app::{RT, persist};
use crate::debounce::debounce;
use crate::state::{Paint, State};
use crate::widgets::{chip, dpi_track, labeled_slider, pill, radio, slider, toggle_row};
use openghub_core::devices::camera;

/// G HUB-style large section heading.
#[cfg(unix)]
pub(crate) const fn heading() -> TextStyle {
    TextStyle::new(18.0, 400.0)
}
#[cfg(unix)]
pub(crate) fn card_tile(
    p: Paint,
    i: usize,
    card: &Card,
    images: Option<&ImageCache>,
    scale: f32,
) -> El {
    let t = p.t;
    let name = display_name(&card.name);
    let summary = card.summary();
    let art = (!card.art.is_empty())
        .then_some(images)
        .flatten()
        .map(|im| im.svg(card.art, 160.0, scale));
    col()
        .key(format!("card-{i}"))
        .role("button")
        .label(&name)
        .size(230.0, 260.0)
        .px(space::S3)
        .py(space::S3)
        .gap(space::S2)
        .radius(10.0)
        .fill(t.card)
        .hover(t.hover)
        .press(t.press)
        .focusable(true)
        .on_press(move |s: &mut State| s.selected = Some(i))
        .child(
            text(&name)
                .key("name")
                .font(type_::TITLE)
                .color(t.text)
                .wrap(),
        )
        .child(
            text(&summary)
                .key("summary")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap(),
        )
        .child(
            el().key("art").grow(1.0).center().child(match art {
                Some(image) => el()
                    .size(180.0, 150.0)
                    .pixels_fit(image.pixels(), SurfaceFit::Fit),
                None => text("⚙")
                    .key("glyph")
                    .font(type_::TITLE)
                    .color(t.text_secondary),
            }),
        )
        .child(
            row().key("foot").w_full().justify_end().child(
                text(&card.status)
                    .key("status")
                    .font(type_::LABEL)
                    .color(tok::accent(4, true)),
            ),
        )
}

#[cfg(unix)]
pub(crate) fn device_page(s: &State, i: usize, p: Paint, scale: f32) -> El {
    let t = p.t;
    let card = &s.cards[i];
    let secs = sections(card);

    let battery = match &card.device {
        Device::Mouse(_, ui) => ui.battery,
        Device::Headset(_, ui) => ui.battery,
        _ => None,
    };
    let status_text = match battery {
        Some((percent, _charging)) => format!("{percent}% · Connected"),
        None => "Connected".to_string(),
    };

    let header = row()
        .key("header")
        .w_full()
        .center_y()
        .justify_between()
        .child(
            row()
                .key("left")
                .center_y()
                .gap(space::S3)
                .child(
                    row()
                        .key("back")
                        .role("button")
                        .label("Back")
                        .h(28.0)
                        .px(space::S3)
                        .center()
                        .radius(radius::MD)
                        .fill(t.card)
                        .border(1.0, t.card_border)
                        .focusable(true)
                        .on_press(move |s: &mut State| s.selected = None)
                        .child(text("←").key("glyph").font(type_::BODY).color(t.text)),
                )
                .child(
                    col()
                        .key("titles")
                        .gap(2.0)
                        .child(
                            text(display_name(&card.name))
                                .key("name")
                                .font(TextStyle::new(22.0, 600.0))
                                .color(t.text),
                        )
                        .child(
                            text(&card.subtitle)
                                .key("subtitle")
                                .font(type_::LABEL)
                                .color(t.text_secondary),
                        ),
                ),
        )
        .child(
            row()
                .key("status")
                .center_y()
                .gap(6.0)
                .child(
                    el().key("dot")
                        .size(8.0, 8.0)
                        .radius(4.0)
                        .fill(tok::accent(4, true)),
                )
                .child(text(status_text).font(type_::LABEL).color(t.text_secondary)),
        );

    col()
        .key("device")
        .w_full()
        .h_full()
        .fill(t.bg)
        .px(space::S6)
        .py(space::S6)
        .child(header)
        .child(
            row()
                .key("body")
                .grow(1.0)
                .min_h(0.0)
                .pt(space::S3)
                .gap(space::S3)
                .child(section_rail(s, i, p, &secs))
                .child(
                    col()
                        .key("content")
                        .grow(1.0)
                        .min_h(0.0)
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .gap(space::S3)
                        .child(device_content(s, i, p, scale)),
                ),
        )
}

#[cfg(unix)]
pub(crate) fn section_rail(
    s: &State,
    i: usize,
    p: Paint,
    secs: &[(&'static str, &'static str)],
) -> El {
    let t = p.t;
    let mut rail = col().key("rail").gap(2.0);
    for (j, (title, glyph)) in secs.iter().enumerate() {
        let selected = s.cards[i].selected_section == j;
        rail = rail.child(
            col()
                .key(format!("section-{j}"))
                .role("button")
                .label(*title)
                .checked(selected)
                .w(68.0)
                .py(space::S2)
                .center()
                .radius(radius::MD)
                .hover(t.hover)
                .focusable(true)
                .on_press(move |s: &mut State| s.cards[i].selected_section = j)
                .child(
                    el().key("icon")
                        .size(30.0, 30.0)
                        .radius(8.0)
                        .fill(if selected {
                            p.accent
                        } else {
                            Color::TRANSPARENT
                        })
                        .center()
                        .child(text(*glyph).font(type_::BODY).color(if selected {
                            Color::WHITE
                        } else {
                            t.text_secondary
                        })),
                )
                .child(
                    text(*title)
                        .key("title")
                        .font(type_::LABEL)
                        .color(if selected { t.text } else { t.text_secondary })
                        .wrap(),
                ),
        );
    }
    rail
}

#[cfg(unix)]
pub(crate) fn device_content(s: &State, i: usize, p: Paint, scale: f32) -> El {
    let card = &s.cards[i];
    let section = card.selected_section;
    match &card.device {
        Device::Mouse(_, ui) => mouse_content(s, i, ui, section, p, s.clock, scale),
        Device::Headset(_, ui) => headset_content(i, ui, section, p),
        Device::Litra(_, ui) => litra_content(i, ui, section, p),
        Device::Camera(_, ui) => camera_content(s, i, ui, section, p, s.preview_image.as_ref()),
        Device::Error(msg) => col().key("error").child(
            text(msg)
                .key("msg")
                .font(type_::BODY)
                .color(p.t.text_secondary)
                .wrap(),
        ),
    }
}

// ---------------------------------------------------------------------------
// Device section content
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub(crate) fn mouse_content(
    s: &State,
    i: usize,
    ui: &MouseUi,
    section: usize,
    p: Paint,
    clock: f64,
    scale: f32,
) -> El {
    match section {
        0 => {
            let t = p.t;
            let mut c = col().key("sensitivity").gap(space::S3);
            c = c.child(
                text("DPI SPEEDS")
                    .key("dpi-title")
                    .font(heading())
                    .color(t.text),
            );
            c = c.child(
                text("Click a speed to make it current; drag it to change it.")
                    .key("dpi-hint")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
            let (min_dpi, max_dpi, step) = (ui.dpi_min, ui.dpi_max, ui.dpi_step);
            c = c.child(dpi_track(
                p,
                "dpi-track",
                &ui.stages,
                ui.dpi,
                min_dpi,
                max_dpi,
                step,
                move |s: &mut State, j| {
                    let serial = s.cards[i].serial.clone();
                    let (m, ui) = s.cards[i].mouse_mut();
                    let stage = ui.stages[j];
                    ui.dpi = stage;
                    let stages = ui.stages.clone();
                    let m = m.clone();
                    RT.spawn(async move {
                        let _ = m.set_dpi(stage).await;
                    });
                    persist(&serial, |st| {
                        st.dpi = stage;
                        st.dpi_stages = Some(stages);
                    });
                },
                move |s: &mut State, j, v| {
                    let serial = s.cards[i].serial.clone();
                    let (m, ui) = s.cards[i].mouse_mut();
                    ui.stages[j] = v;
                    ui.dpi = v;
                    let stages = ui.stages.clone();
                    let m = m.clone();
                    let key = format!("{serial}/dpi");
                    debounce(key, async move {
                        let _ = m.set_dpi(v).await;
                        persist(&serial, |st| {
                            st.dpi = v;
                            st.dpi_stages = Some(stages);
                        });
                    });
                },
            ));
            c = c.child(match ui.battery {
                Some((percent, charging)) => text(format!(
                    "{percent}% battery{}",
                    if charging { " (charging)" } else { "" }
                ))
                .key("battery")
                .font(type_::LABEL)
                .color(t.text_secondary),
                None => text("battery n/a")
                    .key("battery")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            });
            if !ui.rate_options.is_empty() {
                c = c.child(
                    text("REPORT RATE (PER SECOND)")
                        .key("rate-title")
                        .font(heading())
                        .color(t.text)
                        .pt(space::S3),
                );
                let mut rates = row().key("rates").gap(space::S6);
                for (j, &rate) in ui.rate_options.iter().enumerate() {
                    let active = ui.rate == rate;
                    rates = rates.child(radio(
                        p,
                        format!("rate-{j}"),
                        &rate.to_string(),
                        active,
                        move |s: &mut State| {
                            let serial = s.cards[i].serial.clone();
                            let (m, ui) = s.cards[i].mouse_mut();
                            ui.rate = rate;
                            let m = m.clone();
                            RT.spawn(async move {
                                let _ = m.set_report_rate(rate).await;
                            });
                            persist(&serial, |st| st.report_rate = rate);
                        },
                    ));
                }
                c = c.child(rates);
            }
            c = c.child(row().key("restore").pt(space::S3).child(pill(
                p,
                "restore-defaults",
                "Restore default settings",
                false,
                move |s: &mut State| {
                    let serial = s.cards[i].serial.clone();
                    let (m, ui) = s.cards[i].mouse_mut();
                    ui.stages = default_stages(ui.dpi_min, ui.dpi_max);
                    ui.dpi = 1600.clamp(ui.dpi_min, ui.dpi_max);
                    ui.rate = ui.rate_options.first().copied().unwrap_or(1000);
                    let (dpi, rate, stages) = (ui.dpi, ui.rate, ui.stages.clone());
                    let m = m.clone();
                    RT.spawn(async move {
                        let _ = m.set_dpi(dpi).await;
                        let _ = m.set_report_rate(rate).await;
                    });
                    persist(&serial, |st| {
                        st.dpi = dpi;
                        st.dpi_stages = Some(stages);
                        st.report_rate = rate;
                    });
                },
            )));
            c
        }
        1 => crate::device::assign::assignments_view(s, i, p, scale),
        _ => {
            let t = p.t;
            let (r, g, b) = (ui.rgb[0], ui.rgb[1], ui.rgb[2]);
            let [r, g, b] = effective_rgb(ui.effect, [r, g, b], clock);
            let swatch = Color {
                r: srgb(r),
                g: srgb(g),
                b: srgb(b),
                a: 1.0,
            };
            let mut c = col().key("lighting").gap(space::S2);
            c = c.child(text("LIGHTSYNC").key("title").font(heading()).color(t.text));
            c = c.child(
                text("Set the logo LED color and light effects.")
                    .key("desc")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
            let mut effects = row().key("effects").gap(space::S2).flex_wrap();
            effects = effects.child(pill(
                p,
                "effect-static",
                "Static",
                ui.effect == RgbEffect::Static,
                move |s: &mut State| apply_rgb_effect(s, i, RgbEffect::Static),
            ));
            if ui.cycle_supported {
                effects = effects.child(pill(
                    p,
                    "effect-cycle",
                    "Cycle",
                    ui.effect == RgbEffect::Cycle,
                    move |s: &mut State| apply_rgb_effect(s, i, RgbEffect::Cycle),
                ));
            }
            if ui.breathe_supported {
                effects = effects.child(pill(
                    p,
                    "effect-breathe",
                    "Breathing",
                    ui.effect == RgbEffect::Breathe,
                    move |s: &mut State| apply_rgb_effect(s, i, RgbEffect::Breathe),
                ));
            }
            c = c.child(effects);
            c = c.child(
                row()
                    .key("mouse-preview-row")
                    .justify_center()
                    .child(mouse_preview(s, i, swatch, scale)),
            );
            if ui.effect == RgbEffect::Cycle {
                c = c.child(
                    text("Cycle sweeps the full spectrum; it has no per-color control.")
                        .key("note")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .wrap(),
                );
            } else {
                c = c.child(rgb_row(p, "red", "Red", r as f32, 0, i));
                c = c.child(rgb_row(p, "green", "Green", g as f32, 1, i));
                c = c.child(rgb_row(p, "blue", "Blue", b as f32, 2, i));
            }
            c = c.child(
                text("The hardware has no color read-back, so the color seeds from the last state Open G Hub sent.")
                    .key("note2")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap(),
            );
            c
        }
    }
}

/// Renders the mouse art with an animated LED glow dot over the logo.
/// The SVG viewBox is 200×230; the G logo ring is centered at (114, 160)
/// with radius ~20 units (the logo zone). Falls back to a flat swatch when
/// the card has no art or the image cache isn't ready.
#[cfg(unix)]
fn mouse_preview(s: &State, i: usize, color: Color, scale: f32) -> El {
    let card = &s.cards[i];
    let image = (!card.art.is_empty())
        .then_some(s.images.as_ref())
        .flatten()
        .map(|im| im.svg(card.art, 200.0, scale));
    let (w, h) = (240.0, 276.0);
    // SVG viewBox is 200×230; the G logo is a large-arc stroke with
    // endpoints (114,146) and (114,174). The true arc center sits
    // sqrt(20^2 - 14^2) = 14.28 units from the chord midpoint along the
    // perpendicular — on its left, so (99.7, 160). Using the midpoint
    // (114, 160) visibly misses the ring.
    let (cx, cy, r) = (99.7 / 200.0 * w, 160.0 / 230.0 * h, 18.0 / 200.0 * w);
    let glow_r = r * 1.45;
    match image {
        Some(image) => el()
            .key("mouse-preview")
            .size(w, h)
            .child(el().size(w, h).pixels_fit(image.pixels(), SurfaceFit::Fit))
            .child(
                el().size(glow_r * 2.0, glow_r * 2.0)
                    .absolute(cx - glow_r, cy - glow_r)
                    .radius(glow_r)
                    .fill(color.alpha(0.22)),
            )
            .child(
                el().size(r * 2.0, r * 2.0)
                    .absolute(cx - r, cy - r)
                    .radius(r)
                    .fill(color),
            ),
        None => el().key("swatch").size(64.0, 40.0).radius(6.0).fill(color),
    }
}

/// Applies the selected video mode: stores it as the preview's preferred
/// mode and restarts the stream (poll starts it again on the next frame).
#[cfg(unix)]
pub(crate) fn apply_video_mode(s: &mut State, i: usize) {
    let (preview, sel_mode, fmt) = {
        let (_, ui) = s.cards[i].camera_mut();
        let fmt = ui.video.as_ref().map(|v| v.pixel_format).unwrap_or(0);
        (ui.preview.clone(), ui.sel_mode, fmt)
    };
    let Some((w, h, num, den)) = sel_mode else {
        return;
    };
    preview.set_preferred_mode(w, h, num, den, fmt);
    preview.stop();
}

#[cfg(unix)]
pub(crate) fn rgb_row(
    p: Paint,
    key: &'static str,
    label: &str,
    value: f32,
    channel: usize,
    card: usize,
) -> El {
    labeled_slider(
        p,
        key,
        label,
        40.0,
        value,
        0.0,
        255.0,
        1.0,
        format!("{value:.0}"),
        move |s: &mut State, v| {
            let serial = s.cards[card].serial.clone();
            let (m, ui) = s.cards[card].mouse_mut();
            ui.rgb[channel] = v as u8;
            let m = m.clone();
            let (r, g, b) = (ui.rgb[0], ui.rgb[1], ui.rgb[2]);
            let effect = ui.effect;
            let key = format!("{serial}/rgb");
            debounce(key, async move {
                let _ = match effect {
                    RgbEffect::Static => m.set_color(r, g, b).await,
                    RgbEffect::Breathe => m.set_breathe(r, g, b).await,
                    RgbEffect::Cycle => m.set_cycle().await,
                };
                persist(&serial, |st| {
                    st.red = r;
                    st.green = g;
                    st.blue = b;
                });
            });
        },
    )
}

#[cfg(unix)]
pub(crate) fn apply_rgb_effect(s: &mut State, card: usize, effect: RgbEffect) {
    let (m, ui) = s.cards[card].mouse_mut();
    ui.effect = effect;
    let m = m.clone();
    let (r, g, b) = (ui.rgb[0], ui.rgb[1], ui.rgb[2]);
    RT.spawn(async move {
        let _ = match effect {
            RgbEffect::Static => m.set_color(r, g, b).await,
            RgbEffect::Breathe => m.set_breathe(r, g, b).await,
            RgbEffect::Cycle => m.set_cycle().await,
        };
    });
}

/// EQ presets in the spirit of G HUB (10-band dB shapes). The active pill
/// is derived by comparing the stored levels, so a hand-tweaked curve
/// reads as "Custom" without extra persisted state.
#[cfg(unix)]
pub(crate) const EQ_PRESETS: [(&str, [i32; 10]); 3] = [
    ("Flat", [0; 10]),
    ("Bass Boost", [6, 5, 4, 3, 2, 1, 0, 0, 0, 0]),
    ("Treble Boost", [0, 0, 0, 0, 1, 2, 3, 4, 5, 6]),
];

/// Index of the preset matching `levels`, or `None` (→ "Custom").
#[cfg(unix)]
pub(crate) fn match_eq_preset(levels: &[i32]) -> Option<usize> {
    EQ_PRESETS
        .iter()
        .position(|(_, preset)| preset.as_slice() == levels)
}

#[cfg(unix)]
pub(crate) fn headset_content(i: usize, ui: &HeadsetUi, section: usize, p: Paint) -> El {
    let t = p.t;
    match section {
        0 => {
            let mut c = col().key("acoustics").gap(space::S3);
            c = c.child(text("Sidetone").key("title").font(heading()).color(t.text));
            c = c.child(
                text("Hear your own microphone in real time — turn it up to check your levels.")
                    .key("desc")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
            let sidetone = ui.sidetone as f32;
            c = c.child(slider(
                p,
                "sidetone",
                "Sidetone",
                sidetone,
                0.0,
                100.0,
                1.0,
                format!("{sidetone:.0}%"),
                move |s: &mut State, v| {
                    let serial = s.cards[i].serial.clone();
                    let (h, ui) = s.cards[i].headset_mut();
                    ui.sidetone = v as i32;
                    let h = h.clone();
                    let key = format!("{serial}/sidetone");
                    debounce(key, async move {
                        let _ = h.set_sidetone(v as i32).await;
                        persist(&serial, |st| st.sidetone = v as i32);
                    });
                },
            ));
            if ui.has_noise_reduction {
                let on = ui.noise_reduction;
                c = c.child(
                    toggle_row(p, "nr", "Mic Noise Reduction", on, move |s: &mut State| {
                        let (h, ui) = s.cards[i].headset_mut();
                        ui.noise_reduction = !ui.noise_reduction;
                        let on = ui.noise_reduction;
                        let h = h.clone();
                        RT.spawn(async move {
                            let _ = h.set_noise_reduction(on).await;
                        });
                    })
                    .pt(space::S3),
                );
            }
            c
        }
        1 => match &ui.eq {
            None => text("No equalizer feature on this unit")
                .key("no-eq")
                .font(type_::BODY)
                .color(t.text_secondary),
            Some(eq) => {
                let mut c = col().key("eq").gap(space::S2);
                let range = format!(
                    "{} · {}..{} dB",
                    freq_label(eq.frequencies[0]),
                    eq.min_db,
                    eq.max_db
                );
                c = c.child(text("Equalizer").key("title").font(heading()).color(t.text));
                c = c.child(
                    text("Shape the sound across 10 bands, or start from a preset.")
                        .key("desc")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                );
                c = c.child(
                    text(&range)
                        .key("range")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                );
                if eq.frequencies.len() == EQ_PRESETS[0].1.len() {
                    let active = match_eq_preset(&eq.levels);
                    let mut presets = row().key("eq-presets").gap(space::S2).flex_wrap();
                    for (k, (name, preset)) in EQ_PRESETS.iter().enumerate() {
                        let preset = *preset;
                        presets = presets.child(pill(
                            p,
                            format!("preset-{k}"),
                            name,
                            active == Some(k),
                            move |s: &mut State| {
                                let serial = s.cards[i].serial.clone();
                                let (h, ui) = s.cards[i].headset_mut();
                                if let Some(eq) = &mut ui.eq {
                                    let levels: Vec<i32> = preset
                                        .iter()
                                        .map(|&v| v.clamp(eq.min_db, eq.max_db))
                                        .collect();
                                    eq.levels = levels.clone();
                                    let h = h.clone();
                                    let key = format!("{serial}/eq");
                                    debounce(key, async move {
                                        let _ = h.set_eq_levels(&levels).await;
                                        persist(&serial, |st| st.equalizer_levels = Some(levels));
                                    });
                                }
                            },
                        ));
                    }
                    presets = presets.child(pill(
                        p,
                        "preset-custom",
                        "Custom",
                        active.is_none(),
                        |_s: &mut State| {},
                    ));
                    c = c.child(presets.pt(space::S2));
                }
                let mut bands = col().key("bands").gap(space::S1).pt(space::S3);
                for (j, &freq) in eq.frequencies.iter().enumerate() {
                    let value = eq.levels[j] as f32;
                    bands = bands.child(band_row(
                        p,
                        i,
                        j,
                        &freq_label(freq),
                        value,
                        eq.min_db as f32,
                        eq.max_db as f32,
                    ));
                }
                c = c.child(bands);
                c
            }
        },
        2 => stub_content(
            p,
            "Surround",
            "DTS Headphone:X 2.0 7.1 surround is licensed hardware DSP; the PRO X Wireless doesn't expose it over HID++, so this page mirrors G HUB's surface without any controls.",
            &["DTS Headphone:X 2.0"],
        ),
        3 => stub_content(
            p,
            "Mic",
            "Blue VO!CE microphone effects and samples run inside G HUB's licensed DSP. Open G Hub can drive the mic gain sample rate once the audio stack lands; the G HUB spec surface is shown here for reference.",
            &["Sample Rate", "Mic Gain", "Blue VO!CE"],
        ),
        _ => stub_content(
            p,
            "Device Settings",
            "Firmware updates, sleep timers and connection info are out of scope for a hardware-only port. This page keeps G HUB's navigation visible.",
            &["Firmware", "Sleep Timer", "Connection Info"],
        ),
    }
}

#[cfg(unix)]
pub(crate) fn band_row(
    p: Paint,
    card: usize,
    band: usize,
    label: &str,
    value: f32,
    min: f32,
    max: f32,
) -> El {
    labeled_slider(
        p,
        format!("band-{band}"),
        label,
        56.0,
        value,
        min,
        max,
        1.0,
        format!("{value:+.0} dB"),
        move |s: &mut State, v| {
            let serial = s.cards[card].serial.clone();
            let (h, ui) = s.cards[card].headset_mut();
            if let Some(eq) = &mut ui.eq {
                eq.levels[band] = v as i32;
                let levels = eq.levels.clone();
                let h = h.clone();
                let key = format!("{serial}/eq");
                debounce(key, async move {
                    let _ = h.set_eq_levels(&levels).await;
                    persist(&serial, |st| st.equalizer_levels = Some(levels));
                });
            }
        },
    )
}

#[cfg(unix)]
pub(crate) fn litra_content(i: usize, ui: &LitraUi, section: usize, p: Paint) -> El {
    let t = p.t;
    match section {
        0 => {
            let mut c = col().key("lighting").gap(space::S3);
            c = c.child(text("Lighting").key("title").font(heading()).color(t.text));
            c = c.child(
                text("Control power, brightness and color temperature.")
                    .key("desc")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
            let on = ui.on;
            c = c.child(toggle_row(p, "power", "Power", on, move |s: &mut State| {
                flip_litra_power(s, i);
            }));
            let brightness = ui.brightness as f32;
            c = c.child(
                text("Brightness")
                    .key("brightness-title")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .pt(space::S3),
            );
            c = c.child(slider(
                p,
                "brightness",
                "Brightness",
                brightness,
                0.0,
                100.0,
                1.0,
                format!("{brightness:.0}%"),
                move |s: &mut State, v| {
                    let serial = s.cards[i].serial.clone();
                    let (l, ui) = s.cards[i].litra_mut();
                    ui.brightness = v as i32;
                    let l = l.clone();
                    let key = format!("{serial}/brightness");
                    debounce(key, async move {
                        let _ = l.set_brightness(v as i32);
                        persist(&serial, |st| st.brightness = v as i32);
                    });
                },
            ));
            let temperature = ui.temperature as f32;
            c = c.child(
                text("Color temperature")
                    .key("temp-title")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .pt(space::S3),
            );
            c = c.child(slider(
                p,
                "temperature",
                "Temperature",
                temperature,
                2700.0,
                6500.0,
                100.0,
                format!("{temperature:.0} K"),
                move |s: &mut State, v| {
                    let serial = s.cards[i].serial.clone();
                    let (l, ui) = s.cards[i].litra_mut();
                    ui.temperature = v as i32;
                    let l = l.clone();
                    let key = format!("{serial}/temperature");
                    debounce(key, async move {
                        let _ = l.set_temperature(v as i32);
                        persist(&serial, |st| st.temperature = v as i32);
                    });
                },
            ));
            c
        }
        _ => {
            let mut c = col().key("presets").gap(space::S2);
            c = c.child(
                text("Presets")
                    .key("title")
                    .font(type_::SECTION)
                    .color(t.text),
            );
            c = c.child(
                text("One-tap temperature/brightness combinations. The light can't report state back, so each preset writes its full target.")
                    .key("note")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap(),
            );
            let mut pills = row()
                .key("preset-pills")
                .gap(space::S2)
                .flex_wrap()
                .pt(space::S2);
            pills = pills.child(pill(
                p,
                "preset-reading",
                "Reading",
                false,
                move |s: &mut State| {
                    apply_litra_preset(s, i, true, 50, 2700);
                },
            ));
            pills = pills.child(pill(
                p,
                "preset-working",
                "Working",
                false,
                move |s: &mut State| {
                    apply_litra_preset(s, i, true, 80, 4000);
                },
            ));
            pills = pills.child(pill(
                p,
                "preset-daylight",
                "Daylight",
                false,
                move |s: &mut State| {
                    apply_litra_preset(s, i, true, 100, 6500);
                },
            ));
            pills = pills.child(pill(p, "preset-off", "Off", false, move |s: &mut State| {
                apply_litra_preset(s, i, false, 0, 0);
            }));
            c = c.child(pills);
            c
        }
    }
}

/// Applies a Litra lighting preset (writes power + brightness + temperature and
/// mirrors the values back into the card's state so the Lighting page follows).
#[cfg(unix)]
pub(crate) fn apply_litra_preset(
    s: &mut State,
    i: usize,
    on: bool,
    brightness: i32,
    temperature: i32,
) {
    let serial = s.cards[i].serial.clone();
    let (l, ui) = s.cards[i].litra_mut();
    if on {
        ui.on = true;
        ui.brightness = brightness;
        ui.temperature = temperature;
        let _ = l.set_power(true);
        let _ = l.set_brightness(brightness);
        let _ = l.set_temperature(temperature);
    } else {
        ui.on = false;
        let _ = l.set_power(false);
    }
    let (on, b, t) = (ui.on, ui.brightness, ui.temperature);
    persist(&serial, |st| {
        st.power = on;
        st.brightness = b;
        st.temperature = t;
    });
}

/// Flips the power state of the Litra at card index `j`, writes to hardware
/// and persists. Shared by the Litra page and the camera-page quick toggle.
#[cfg(unix)]
pub(crate) fn flip_litra_power(s: &mut State, j: usize) {
    let serial = s.cards[j].serial.clone();
    let (l, ui) = s.cards[j].litra_mut();
    ui.on = !ui.on;
    let on = ui.on;
    let _ = l.set_power(on);
    persist(&serial, |st| st.power = on);
}

/// Power toggles for every connected Litra, for display on the camera page.
/// Returns `None` when no Litra is plugged in.
#[cfg(unix)]
pub(crate) fn litra_toggles(s: &State, p: Paint) -> Option<El> {
    let litras: Vec<(usize, String, bool)> = s
        .cards
        .iter()
        .enumerate()
        .filter_map(|(j, card)| match &card.device {
            Device::Litra(_, lui) => Some((j, card.name.clone(), lui.on)),
            _ => None,
        })
        .collect();
    if litras.is_empty() {
        return None;
    }

    let t = p.t;
    let mut c = col().key("lights").gap(space::S2).child(
        text("Lights")
            .key("lights-title")
            .font(type_::LABEL)
            .color(t.text_secondary)
            .pt(space::S3),
    );
    for (j, name, on) in litras {
        c = c.child(toggle_row(
            p,
            format!("litra-power-{j}"),
            &name,
            on,
            move |s: &mut State| {
                flip_litra_power(s, j);
            },
        ));
    }
    Some(c)
}

#[cfg(unix)]
pub(crate) fn camera_content(
    s: &State,
    i: usize,
    ui: &CameraUi,
    section: usize,
    p: Paint,
    preview: Option<&Image>,
) -> El {
    let t = p.t;
    let image_adjustments =
        |name: &str| matches!(name, "Brightness" | "Contrast" | "Saturation" | "Sharpness");
    let selected: Vec<&CamControl> = match section {
        0 => ui
            .controls
            .iter()
            .filter(|c| !image_adjustments(&c.name))
            .collect(),
        2 => ui
            .controls
            .iter()
            .filter(|c| image_adjustments(&c.name))
            .collect(),
        _ => {
            // G HUB's Video tab is resolution + frame rate. Field of view
            // is the Zoom slider (CAMERA tab); shutter speed and ISO are
            // Windows-plugin features this hardware doesn't expose on Linux.
            let t = p.t;
            let mut c = col().key("video").gap(space::S3);
            c = c.child(text("Video").key("title").font(heading()).color(t.text));
            let Some(video) = ui.video.clone() else {
                c = c.child(
                    text("This driver reported no video modes.")
                        .key("note")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                );
                return c;
            };
            let sel = ui.sel_mode.unwrap_or_default();
            c = c.child(
                text("Resolution")
                    .key("res-title")
                    .font(type_::SECTION)
                    .color(t.text),
            );
            let mut res = row().key("res").gap(space::S2).flex_wrap();
            for size in &video.sizes {
                let label = format!("{}×{}", size.width, size.height);
                let active = (sel.0, sel.1) == (size.width, size.height);
                let width = size.width;
                let height = size.height;
                res = res.child(pill(
                    p,
                    format!("res-{width}x{height}"),
                    &label,
                    active,
                    move |s: &mut State| {
                        let (_, ui) = s.cards[i].camera_mut();
                        let intervals = ui
                            .video
                            .as_ref()
                            .and_then(|v| {
                                v.sizes
                                    .iter()
                                    .find(|sz| (sz.width, sz.height) == (width, height))
                            })
                            .map(|sz| sz.intervals.clone())
                            .unwrap_or_default();
                        let (num, den) = intervals.first().copied().unwrap_or((1, 30));
                        ui.sel_mode = Some((width, height, num, den));
                        apply_video_mode(s, i);
                    },
                ));
            }
            c = c.child(res);
            c = c.child(
                text("Frame rate")
                    .key("fps-title")
                    .font(type_::SECTION)
                    .color(t.text)
                    .pt(space::S3),
            );
            let intervals = video
                .sizes
                .iter()
                .find(|sz| (sz.width, sz.height) == (sel.0, sel.1))
                .map(|sz| sz.intervals.clone())
                .unwrap_or_default();
            let mut fps_row = row().key("fps").gap(space::S2).flex_wrap();
            for (k, &(num, den)) in intervals.iter().enumerate() {
                let rate = camera::fps_of(num, den);
                let label = if rate.fract() == 0.0 {
                    format!("{rate:.0} fps")
                } else {
                    format!("{rate:.1} fps")
                };
                fps_row = fps_row.child(pill(
                    p,
                    format!("fps-{k}"),
                    &label,
                    (sel.2, sel.3) == (num, den),
                    move |s: &mut State| {
                        let (_, ui) = s.cards[i].camera_mut();
                        let (w, h, _, _) = ui.sel_mode.unwrap_or_default();
                        ui.sel_mode = Some((w, h, num, den));
                        apply_video_mode(s, i);
                    },
                ));
            }
            c = c.child(fps_row);
            c = c.child(
                text("Applies immediately — the preview restarts at the new mode. Field of view is the Zoom slider on the CAMERA tab; shutter speed/ISO are G HUB-plugin features this device doesn't expose on Linux.")
                    .key("note")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .pt(space::S3),
            );
            return c;
        }
    };

    let mut c = col().key("camera").gap(space::S2);
    match section {
        0 => {
            c = c.child(
                el().w_full()
                    .center()
                    .child(text("Streaming").key("title").font(heading()).color(t.text)),
            );
            c = c.child(
                el().w_full().center().child(
                    text("Pan and tilt with the arrows; adjust zoom, focus and exposure below.")
                        .key("desc")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
            );
        }
        2 => {
            c = c.child(text("Image").key("title").font(heading()).color(t.text));
            c = c.child(
                text("Fine-tune brightness, contrast, saturation and sharpness.")
                    .key("desc")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
        }
        _ => {}
    }
    if section == 0 {
        c = c.child(
            row()
                .key("preview-row")
                .justify_center()
                .child(camera_preview(i, p, preview)),
        );
        // Quick light switch: toggle any connected Litra right from the
        // preview, so the studio can be lit without leaving the camera page.
        if let Some(toggles) = litra_toggles(s, p) {
            c = c.child(toggles);
        }
    }
    for ctl in &selected {
        if ctl.is_bool {
            let name = ctl.name.clone();
            let key = name.clone();
            let label = name.clone();
            let name_c = name;
            let on = ctl.value != 0;
            c = c.child(toggle_row(p, key, &label, on, move |s: &mut State| {
                let (cam, ui) = s.cards[i].camera_mut();
                for c in &mut ui.controls {
                    if c.name == name_c {
                        c.value = if c.value == 0 { 1 } else { 0 };
                        let v = c.value;
                        let cam = cam.clone();
                        let n = name_c.clone();
                        let _ = cam.set_control(&n, v);
                    }
                }
            }));
        } else {
            let name = ctl.name.clone();
            let key = name.clone();
            let label = name.clone();
            let name_c = name;
            let value = ctl.value as f32;
            let min = ctl.min as f32;
            let max = ctl.max as f32;
            let step = ctl.step.max(1) as f32;
            c = c.child(slider(
                p,
                key,
                &label,
                value,
                min,
                max,
                step,
                format!("{value:.0}"),
                move |s: &mut State, v| {
                    let serial = s.cards[i].serial.clone();
                    let (cam, ui) = s.cards[i].camera_mut();
                    for c in &mut ui.controls {
                        if c.name == name_c {
                            c.value = v as i32;
                            let val = v as i32;
                            let cam = cam.clone();
                            let n = name_c.clone();
                            let key = format!("{serial}/camera-{n}");
                            debounce(key, async move {
                                let _ = cam.set_control(&n, val);
                            });
                        }
                    }
                },
            ));
        }
    }
    // Restore all camera controls to the device's factory defaults.
    if !selected.is_empty() {
        c = c.child(
            row()
                .key("restore")
                .justify_center()
                .pt(space::S3)
                .child(pill(
                    p,
                    "restore-camera",
                    "Restore camera defaults",
                    false,
                    move |s: &mut State| {
                        let (cam, ui) = s.cards[i].camera_mut();
                        for ctl in &mut ui.controls {
                            let v = ctl.default;
                            if ctl.value != v {
                                ctl.value = v;
                                let cam = cam.clone();
                                let n = ctl.name.clone();
                                let _ = cam.set_control(&n, v);
                            }
                        }
                    },
                )),
        );
    }
    c
}

/// The 480×360 preview with G HUB's pan/tilt arrows on its edges.
#[cfg(unix)]
fn camera_preview(i: usize, p: Paint, preview: Option<&rune::Image>) -> El {
    let t = p.t;
    let shot = match preview {
        Some(image) => el()
            .key("preview")
            .size(480.0, 360.0)
            .radius(8.0)
            .pixels_fit(image.pixels(), rune::SurfaceFit::Fit),
        None => el()
            .key("preview")
            .size(480.0, 360.0)
            .radius(8.0)
            .fill(Color::BLACK.alpha(0.3))
            .child(
                text("preview")
                    .key("ph")
                    .font(type_::BODY)
                    .color(t.text_secondary),
            ),
    };
    let mut box_ = el().key("preview-box").size(480.0, 360.0).child(shot);
    for (key, glyph, x, y) in [
        ("pan-left", "◀", 6.0, 166.0),
        ("pan-right", "▶", 446.0, 166.0),
        ("tilt-up", "▲", 226.0, 6.0),
        ("tilt-down", "▼", 226.0, 326.0),
    ] {
        let name = match key {
            "pan-left" | "pan-right" => ("Pan", if key == "pan-left" { -1 } else { 1 }),
            _ => ("Tilt", if key == "tilt-up" { 1 } else { -1 }),
        };
        box_ = box_.child(
            row()
                .key(key)
                .role("button")
                .label(key)
                .size(28.0, 28.0)
                .absolute(x, y)
                .center()
                .radius(14.0)
                .fill(Color::BLACK.alpha(0.45))
                .focusable(true)
                .on_press(move |s: &mut State| nudge_camera(s, i, name.0, name.1))
                .child(text(glyph).font(type_::LABEL).color(Color::WHITE)),
        );
    }
    box_
}

/// Moves a discrete camera control (Pan/Tilt) by one device step.
#[cfg(unix)]
fn nudge_camera(s: &mut State, i: usize, name: &str, dir: i32) {
    let (cam, ui) = s.cards[i].camera_mut();
    if let Some(ctl) = ui.controls.iter_mut().find(|c| c.name == name) {
        let step = ctl.step.max(1);
        let v = (ctl.value + dir * step).clamp(ctl.min, ctl.max);
        ctl.value = v;
        let cam = cam.clone();
        let n = ctl.name.clone();
        let _ = cam.set_control(&n, v);
    }
}

#[cfg(unix)]
pub(crate) fn stub_content(p: Paint, title: &str, note: &str, items: &[&str]) -> El {
    let t = p.t;
    let mut c = col().key("stub").gap(space::S3);
    c = c.child(text(title).key("title").font(type_::SECTION).color(t.text));
    c = c.child(
        text(note)
            .key("note")
            .font(type_::BODY)
            .color(t.text_secondary)
            .wrap(),
    );
    let mut chips = row().key("chips").gap(space::S2).flex_wrap();
    for (j, item) in items.iter().enumerate() {
        chips = chips.child(chip(p, format!("chip-{j}"), item));
    }
    c = c.child(chips);
    c
}

#[cfg(unix)]
pub(crate) fn srgb(v: u8) -> f32 {
    rune::color::srgb_decode(v as f32 / 255.0)
}

/// The LED effect period the hardware runs, in seconds (driver writes
/// 5000 ms for both Cycle and Breathe).
#[cfg(unix)]
const LED_PERIOD_S: f64 = 5.0;

/// The color the hardware is currently showing, computed from the last
/// written state — the device has no color read-back. Cycle sweeps the
/// sRGB spectrum; Breathe pulses the user's color with a sine.
#[cfg(unix)]
pub(crate) fn effective_rgb(effect: RgbEffect, rgb: [u8; 3], t: f64) -> [u8; 3] {
    match effect {
        RgbEffect::Static => rgb,
        RgbEffect::Cycle => hsv_to_rgb((t / LED_PERIOD_S * 360.0).rem_euclid(360.0), 1.0, 1.0),
        RgbEffect::Breathe => {
            let k = 0.5 + 0.5 * (2.0 * std::f64::consts::PI * t / LED_PERIOD_S).sin();
            let mul = |v: u8| (v as f64 * k).round().clamp(0.0, 255.0) as u8;
            [mul(rgb[0]), mul(rgb[1]), mul(rgb[2])]
        }
    }
}

/// sRGB HSV (hue in degrees) → RGB bytes, for the Cycle spectrum preview.
#[cfg(unix)]
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0).floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_u8 = |ch: f64| ((ch + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    [to_u8(r), to_u8(g), to_u8(b)]
}

#[cfg(unix)]
pub(crate) fn freq_label(hz: i32) -> String {
    if hz >= 1000 {
        format!("{} kHz", hz as f32 / 1000.0)
    } else {
        format!("{hz} Hz")
    }
}

#[cfg(test)]
mod tests {
    use super::match_eq_preset;
    use crate::device::RgbEffect;
    use crate::device::content::effective_rgb;

    #[test]
    fn flat_and_boosts_match_their_presets() {
        assert_eq!(match_eq_preset(&[0; 10]), Some(0));
        assert_eq!(match_eq_preset(&[6, 5, 4, 3, 2, 1, 0, 0, 0, 0]), Some(1));
        assert_eq!(match_eq_preset(&[0, 0, 0, 0, 1, 2, 3, 4, 5, 6]), Some(2));
    }

    #[test]
    fn hand_tweaked_is_custom() {
        assert_eq!(match_eq_preset(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 1]), None);
    }

    #[test]
    fn wrong_band_count_never_matches() {
        assert_eq!(match_eq_preset(&[0; 5]), None);
        assert_eq!(match_eq_preset(&[]), None);
    }

    #[test]
    fn static_is_identity() {
        assert_eq!(
            effective_rgb(RgbEffect::Static, [10, 20, 30], 3.0),
            [10, 20, 30]
        );
    }

    #[test]
    fn breathe_hits_peak_and_floor() {
        // Period 5 s: quarter-period = peak (color), three-quarters = dark.
        assert_eq!(
            effective_rgb(RgbEffect::Breathe, [200, 100, 50], 1.25),
            [200, 100, 50]
        );
        assert_eq!(
            effective_rgb(RgbEffect::Breathe, [200, 100, 50], 3.75),
            [0, 0, 0]
        );
    }

    #[test]
    fn cycle_sweeps_spectrum() {
        // t=0 → hue 0 (red); half period → hue 180 (cyan).
        assert_eq!(effective_rgb(RgbEffect::Cycle, [0, 0, 0], 0.0), [255, 0, 0]);
        assert_eq!(
            effective_rgb(RgbEffect::Cycle, [0, 0, 0], 2.5),
            [0, 255, 255]
        );
    }
}
