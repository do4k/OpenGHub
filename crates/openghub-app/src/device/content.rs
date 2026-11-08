use rune::tok::{radius, space, type_};
use rune::{Color, El, Image, ImageCache, ScrollSpec, SurfaceFit, col, el, row, text};

use super::{
    CamControl, CameraUi, Card, Device, HeadsetUi, LitraUi, MouseUi, RgbEffect, display_name,
    sections,
};
use crate::app::{RT, persist};
use crate::debounce::debounce;
use crate::state::{Paint, State};
use crate::widgets::{chip, labeled_slider, pill, slider, toggle_row};
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
                .font(type_::BODY)
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
                    .size(160.0, 130.0)
                    .pixels_fit(image.pixels(), SurfaceFit::Fit),
                None => text("⚙")
                    .key("glyph")
                    .font(type_::TITLE)
                    .color(t.text_secondary),
            }),
        )
        .child(
            row()
                .key("foot")
                .w_full()
                .justify_between()
                .child(
                    text("⚙")
                        .key("gear")
                        .font(type_::BODY)
                        .color(t.text_secondary),
                )
                .child(
                    text(&card.status)
                        .key("status")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
}

#[cfg(unix)]
pub(crate) fn device_page(s: &State, i: usize, p: Paint) -> El {
    let t = p.t;
    let card = &s.cards[i];
    let secs = sections(card);

    col()
        .key("device")
        .w_full()
        .h_full()
        .fill(t.bg)
        .px(space::S6)
        .py(space::S6)
        .child(
            row()
                .key("header")
                .w_full()
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
                                .font(type_::TITLE)
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
                        .child(
                            text(&card.status)
                                .key("status")
                                .font(type_::LABEL)
                                .color(p.accent),
                        )
                        .child(device_content(s, i, p)),
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
                .fill(if selected {
                    t.press
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .focusable(true)
                .on_press(move |s: &mut State| s.cards[i].selected_section = j)
                .child(
                    text(*glyph)
                        .key("glyph")
                        .font(type_::BODY)
                        .color(if selected { t.text } else { t.text_secondary }),
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
pub(crate) fn device_content(s: &State, i: usize, p: Paint) -> El {
    let card = &s.cards[i];
    let section = card.selected_section;
    match &card.device {
        Device::Mouse(_, ui) => mouse_content(i, ui, section, p),
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
pub(crate) fn mouse_content(i: usize, ui: &MouseUi, section: usize, p: Paint) -> El {
    match section {
        0 => {
            let t = p.t;
            let mut c = col().key("sensitivity").gap(space::S3);
            c = c.child(
                text("DPI Speed")
                    .key("dpi-title")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
            let mut stages = row().key("stages").gap(space::S2).flex_wrap();
            for (j, &stage) in ui.stages.iter().enumerate() {
                let active = ui.dpi == stage;
                stages = stages.child(pill(
                    p,
                    format!("stage-{j}"),
                    &stage.to_string(),
                    active,
                    move |s: &mut State| {
                        let serial = s.cards[i].serial.clone();
                        let (m, ui) = s.cards[i].mouse_mut();
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
                ));
            }
            c = c.child(stages);
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
                    text("Report Rate (per second)")
                        .key("rate-title")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .pt(space::S3),
                );
                let mut rates = row().key("rates").gap(space::S2).flex_wrap();
                for (j, &rate) in ui.rate_options.iter().enumerate() {
                    let active = ui.rate == rate;
                    rates = rates.child(pill(
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
            c
        }
        1 => stub_content(
            p,
            "Assignments",
            "Receiver-connected units don't expose button remapping over HID++ (0x1b04 comes back empty), so Assignments stays a stub.",
            &["DPI Switch", "Left Button", "Right Button"],
        ),
        _ => {
            let t = p.t;
            let (r, g, b) = (ui.rgb[0], ui.rgb[1], ui.rgb[2]);
            let swatch = Color {
                r: srgb(r),
                g: srgb(g),
                b: srgb(b),
                a: 1.0,
            };
            let mut c = col().key("lighting").gap(space::S2);
            c = c.child(
                text("LIGHTSYNC")
                    .key("title")
                    .font(type_::SECTION)
                    .color(t.text),
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
            c = c.child(el().key("swatch").size(64.0, 40.0).radius(6.0).fill(swatch));
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

#[cfg(unix)]
pub(crate) fn headset_content(i: usize, ui: &HeadsetUi, section: usize, p: Paint) -> El {
    let t = p.t;
    match section {
        0 => {
            let mut c = col().key("acoustics").gap(space::S3);
            c = c.child(
                text("Sidetone")
                    .key("title")
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
                c = c.child(
                    text("Equalizer")
                        .key("title")
                        .font(type_::SECTION)
                        .color(t.text),
                );
                c = c.child(
                    text(&range)
                        .key("range")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                );
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
            return stub_content(
                p,
                "Video",
                "Frame rate, field of view, shutter speed, ISO and HDR are hardware/encoder settings that Windows-only G HUB drives through its own camera plugin. On Linux, V4L2 exposes the capture and image controls on the other tabs; this tab mirrors G HUB's spec surface.",
                &["FOV", "Shutter Speed", "ISO", "HDR"],
            );
        }
    };

    let mut c = col().key("camera").gap(space::S2);
    if section == 0 {
        c = c.child(match preview {
            Some(image) => el()
                .key("preview")
                .size(480.0, 360.0)
                .radius(8.0)
                .pixels_fit(image.pixels(), SurfaceFit::Fit),
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
        });
        // Quick light switch: toggle any connected Litra right from the
        // preview, so the studio can be lit without leaving the camera page.
        if let Some(toggles) = litra_toggles(s, p) {
            c = c.child(toggles);
        }
    }
    for ctl in selected {
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
    c
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

#[cfg(unix)]
pub(crate) fn freq_label(hz: i32) -> String {
    if hz >= 1000 {
        format!("{} kHz", hz as f32 / 1000.0)
    } else {
        format!("{hz} Hz")
    }
}
