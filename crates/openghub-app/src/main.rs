//! Open G Hub — a Logitech G HUB alternative. Rune UI, Rust port of the
//! Avalonia `OpenGHub.App`. Talks to hardware through `openghub-core`.

use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use openghub_core::config::{ConfigStore, DeviceState};
use openghub_core::devices::{camera, headset, litra, mouse};
use openghub_core::hid::HidBackend;
use openghub_core::v4l2::PIX_FMT_YUYV;

use rune::tok::{self, motion, radius, space, type_};
use rune::{
    Align, AppSpec, Color, Cx, Drag, El, Image, ImageCache, Key, KeyEvent, Phase, ScrollSpec,
    SurfaceFit, Tx, col, el, row, text,
};

const ART_GPRO: &[u8] = include_bytes!("../assets/gpro.svg");
const ART_PROX: &[u8] = include_bytes!("../assets/prox.svg");
const ART_LITRA_GLOW: &[u8] = include_bytes!("../assets/litra-glow.svg");
const ART_LITRA_BEAM: &[u8] = include_bytes!("../assets/litra-beam.svg");
const ART_WEBCAM: &[u8] = include_bytes!("../assets/webcam.svg");

/// One shared tokio runtime for async HID++ calls (mouse/headset) and discovery.
static RT: LazyLock<tokio::runtime::Runtime> =
    LazyLock::new(|| tokio::runtime::Runtime::new().expect("tokio runtime"));

/// The app handle, used by the Refresh button to trigger re-discovery.
static HANDLE: OnceLock<rune::AppHandle<State>> = OnceLock::new();

/// The persisted last-known device state, keyed by serial number.
static STORE: LazyLock<Arc<ConfigStore>> = LazyLock::new(|| {
    let path = ConfigStore::default_path();
    match ConfigStore::open(&path) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("openghub: ignoring unreadable state file {path}: {e}");
            Arc::new(ConfigStore::open("/tmp/openghub-state.json").expect("writable state"))
        }
    }
});

/// Read-modify-write one device's saved state.
fn persist(serial: &str, mutate: impl FnOnce(&mut DeviceState)) {
    let store = &*STORE;
    let mut state = store.try_get(serial).unwrap_or_default();
    mutate(&mut state);
    let _ = store.set(serial, state);
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    cards: Vec<Card>,
    selected: Option<usize>,
    images: Option<ImageCache>,
    preview_image: Option<Image>,
    preview_pending: Option<Image>,
    preview_seq: u64,
}

struct Card {
    name: String,
    subtitle: String,
    status: String,
    serial: String,
    art: &'static [u8],
    selected_section: usize,
    device: Device,
}

enum Device {
    Mouse(Arc<mouse::GProMouse>, MouseUi),
    Headset(Arc<headset::ProxHeadset>, HeadsetUi),
    Litra(Arc<litra::LitraLight>, LitraUi),
    Camera(Arc<camera::V4l2Camera>, CameraUi),
    Error(String),
}

struct MouseUi {
    dpi: i32,
    stages: Vec<i32>,
    rate_options: Vec<i32>,
    rate: i32,
    battery: Option<(i32, bool)>,
    rgb: [u8; 3],
    rgb_supported: bool,
    cycle_supported: bool,
    breathe_supported: bool,
    effect: RgbEffect,
}

/// Which logo-LED effect is selected on the Lightsync page.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum RgbEffect {
    #[default]
    Static,
    Cycle,
    Breathe,
}

struct HeadsetUi {
    sidetone: i32,
    noise_reduction: bool,
    has_noise_reduction: bool,
    eq: Option<EqUi>,
    battery: Option<(i32, bool)>,
}

struct EqUi {
    frequencies: Vec<i32>,
    min_db: i32,
    max_db: i32,
    levels: Vec<i32>,
}

struct LitraUi {
    on: bool,
    brightness: i32,
    temperature: i32,
}

struct CameraUi {
    controls: Vec<CamControl>,
    preview: Arc<camera::WebcamPreview>,
}

struct CamControl {
    name: String,
    min: i32,
    max: i32,
    step: i32,
    value: i32,
    is_bool: bool,
}

impl Card {
    fn mouse_mut(&mut self) -> (&Arc<mouse::GProMouse>, &mut MouseUi) {
        match &mut self.device {
            Device::Mouse(m, ui) => (m, ui),
            _ => unreachable!(),
        }
    }
    fn headset_mut(&mut self) -> (&Arc<headset::ProxHeadset>, &mut HeadsetUi) {
        match &mut self.device {
            Device::Headset(h, ui) => (h, ui),
            _ => unreachable!(),
        }
    }
    fn litra_mut(&mut self) -> (&Arc<litra::LitraLight>, &mut LitraUi) {
        match &mut self.device {
            Device::Litra(l, ui) => (l, ui),
            _ => unreachable!(),
        }
    }
    fn camera_mut(&mut self) -> (&Arc<camera::V4l2Camera>, &mut CameraUi) {
        match &mut self.device {
            Device::Camera(c, ui) => (c, ui),
            _ => unreachable!(),
        }
    }

    /// One-line dashboard tile summary, derived from live state.
    fn summary(&self) -> String {
        match &self.device {
            Device::Mouse(_, ui) => match ui.battery {
                Some((p, _)) => format!("{p}% battery · {} DPI", ui.dpi),
                None => format!("{} DPI", ui.dpi),
            },
            Device::Headset(_, ui) => match ui.battery {
                Some((p, _)) => format!("{p}% battery"),
                None => "battery n/a".to_string(),
            },
            Device::Litra(_, ui) => format!(
                "{} · {}% · {} K",
                if ui.on { "On" } else { "Off" },
                ui.brightness,
                ui.temperature
            ),
            Device::Camera(_, ui) => format!("{} controls", ui.controls.len()),
            Device::Error(_) => String::new(),
        }
    }
}

/// The G HUB-style section rail for a card: (title, glyph) pairs.
fn sections(card: &Card) -> Vec<(&'static str, &'static str)> {
    match &card.device {
        Device::Mouse(_, ui) => {
            let mut s = vec![("SENSITIVITY", "⊕"), ("ASSIGNMENTS", "✥")];
            if ui.rgb_supported {
                s.push(("LIGHTSYNC", "◉"));
            }
            s
        }
        Device::Headset(_, _) => vec![
            ("ACOUSTICS", "♫"),
            ("EQ", "▤"),
            ("SURROUND", "△"),
            ("MIC", "●"),
            ("DEVICE SETTINGS", "⚙"),
        ],
        Device::Litra(_, _) => vec![("LIGHTING", "☀"), ("PRESETS", "▦")],
        Device::Camera(_, _) => vec![("CAMERA", "☐"), ("VIDEO", "▤"), ("IMAGE", "◧")],
        Device::Error(_) => vec![],
    }
}

fn display_name(name: &str) -> String {
    name.to_uppercase()
}

// ---------------------------------------------------------------------------
// Discovery (async, run once on a background task)
// ---------------------------------------------------------------------------

async fn discover() -> Vec<Card> {
    let backend = HidBackend;
    let mut cards = Vec::new();

    for info in backend.enumerate(0x046d, 0) {
        if info.product_id == litra::PRODUCT_GLOW || info.product_id == litra::PRODUCT_BEAM {
            cards.push(litra_card(&backend, &info));
        } else if info.product_id == headset::PRODUCT_ID {
            if let Ok(Some(c)) = headset_card(&backend, &info).await {
                cards.push(c);
            }
        } else if mouse::GProMouse::is_mouse_product(info.product_id)
            && let Ok(Some(c)) = mouse_card(&backend, &info).await
        {
            cards.push(c);
        }
    }

    for node in camera::V4l2Camera::enumerate_nodes() {
        if let Ok(Some(c)) = camera::V4l2Camera::open(&node) {
            cards.push(camera_card(c));
        }
    }

    eprintln!("openghub: discovered {} device(s)", cards.len());
    for card in &cards {
        eprintln!("openghub:   - {} ({})", card.name, card.subtitle);
    }

    cards
}

fn litra_art(product_id: u16) -> &'static [u8] {
    if product_id == litra::PRODUCT_BEAM {
        ART_LITRA_BEAM
    } else {
        ART_LITRA_GLOW
    }
}

const STAGE_PRESETS: [i32; 7] = [400, 800, 1600, 3200, 6400, 12800, 25600];

fn default_stages(min: i32, max: i32) -> Vec<i32> {
    let stages: Vec<i32> = STAGE_PRESETS
        .into_iter()
        .filter(|&v| v >= min && v <= max)
        .collect();
    if stages.len() >= 2 {
        stages
    } else {
        vec![min, max]
    }
}

fn sanitize_stages(input: Vec<i32>, min: i32, max: i32) -> Vec<i32> {
    let mut output = Vec::new();
    for v in input {
        let clamped = v.clamp(min, max);
        if !output.contains(&clamped) {
            output.push(clamped);
        }
    }
    output.sort_unstable();
    if output.is_empty() {
        default_stages(min, max)
    } else {
        output
    }
}

fn litra_card(backend: &HidBackend, info: &openghub_core::hid::DeviceInfo) -> Card {
    let name = litra::LitraLight::friendly_name(info.product_id);
    let subtitle = format!("serial {}", info.serial.as_deref().unwrap_or("n/a"));
    let serial = info.serial.clone().unwrap_or_default();
    let saved = &STORE.try_get(&serial);
    match backend.open(info) {
        Ok(fd) => Card {
            name,
            subtitle,
            serial,
            status: "ready".to_string(),
            art: litra_art(info.product_id),
            selected_section: 0,
            device: Device::Litra(
                Arc::new(litra::LitraLight::new(fd)),
                LitraUi {
                    on: saved.as_ref().is_some_and(|s| s.power),
                    brightness: saved
                        .as_ref()
                        .map(|s| s.brightness)
                        .filter(|&b| b > 0)
                        .unwrap_or(60),
                    temperature: saved
                        .as_ref()
                        .map(|s| s.temperature)
                        .filter(|&t| t > 0)
                        .unwrap_or(3200),
                },
            ),
        },
        Err(e) => Card {
            name,
            subtitle,
            serial,
            status: e.to_string(),
            art: litra_art(info.product_id),
            selected_section: 0,
            device: Device::Error(e.to_string()),
        },
    }
}

async fn headset_card(
    backend: &HidBackend,
    info: &openghub_core::hid::DeviceInfo,
) -> Result<Option<Card>, openghub_core::Error> {
    let Some(h) = headset::ProxHeadset::open(backend, info).await? else {
        return Ok(None);
    };

    let serial = info
        .serial
        .clone()
        .unwrap_or_else(|| format!("prox-{:04x}", info.product_id));
    let saved = &STORE.try_get(&serial);

    let mut ui = HeadsetUi {
        sidetone: saved.as_ref().map(|s| s.sidetone).unwrap_or(0),
        noise_reduction: false,
        has_noise_reduction: false,
        eq: None,
        battery: None,
    };
    if let Ok(v) = h.get_sidetone().await {
        ui.sidetone = v;
    }
    ui.has_noise_reduction = h
        .get_noise_reduction()
        .await
        .map(|v| ui.noise_reduction = v)
        .is_ok();
    if h.has_equalizer() {
        let freqs = h.eq_frequencies().to_vec();
        let (min_db, max_db) = h.eq_range();
        let mut levels = vec![0; freqs.len()];
        if let Ok(live) = h.get_eq_levels().await
            && live.len() == freqs.len()
        {
            levels = live;
        } else if let Some(s) = &saved
            && let Some(lv) = &s.equalizer_levels
            && lv.len() == freqs.len()
        {
            levels = lv.clone();
        }
        ui.eq = Some(EqUi {
            frequencies: freqs,
            min_db,
            max_db,
            levels,
        });
    }
    ui.battery = h.get_battery().await.ok();

    Ok(Some(Card {
        name: h.name().to_string(),
        subtitle: format!("serial {}", info.serial.as_deref().unwrap_or("n/a")),
        serial,
        status: "ready".to_string(),
        art: ART_PROX,
        selected_section: 0,
        device: Device::Headset(Arc::new(h), ui),
    }))
}

async fn mouse_card(
    backend: &HidBackend,
    info: &openghub_core::hid::DeviceInfo,
) -> Result<Option<Card>, openghub_core::Error> {
    let Some(m) = mouse::GProMouse::open(backend, info).await? else {
        return Ok(None);
    };

    let serial = m.serial().to_string();
    let saved = &STORE.try_get(&serial);

    let (min, max, _step) = m.dpi_range();
    let mut stages = sanitize_stages(
        saved
            .as_ref()
            .and_then(|s| s.dpi_stages.clone())
            .unwrap_or_default(),
        min,
        max,
    );

    let mut dpi = saved.as_ref().map(|s| s.dpi).unwrap_or(0);
    if let Ok(live) = m.get_dpi().await {
        dpi = live;
    }
    if dpi <= 0 {
        dpi = stages[0];
    }
    if (min..=max).contains(&dpi) && !stages.contains(&dpi) {
        stages.push(dpi);
        stages = sanitize_stages(stages, min, max);
    }

    let rate_options = m.report_rate_options().to_vec();
    let mut rate = saved
        .as_ref()
        .and_then(|s| (s.report_rate > 0).then_some(s.report_rate))
        .or_else(|| rate_options.first().copied())
        .unwrap_or(1000);
    if let Ok(live) = m.get_report_rate().await {
        rate = live;
    }

    let rgb_supported = m.rgb_supported();
    let rgb = match &saved {
        Some(s) if s.red as u32 + s.green as u32 + s.blue as u32 > 0 => [s.red, s.green, s.blue],
        _ => [0xff, 0xff, 0xff],
    };
    let battery = m.get_battery().await.ok();
    let cycle_supported = rgb_supported && m.has_cycle();
    let breathe_supported = rgb_supported && m.has_breathe();

    Ok(Some(Card {
        name: m.name().to_string(),
        subtitle: format!("serial {}", m.serial()),
        serial,
        status: "ready".to_string(),
        art: ART_GPRO,
        selected_section: 0,
        device: Device::Mouse(
            Arc::new(m),
            MouseUi {
                dpi,
                stages,
                rate_options,
                rate,
                battery,
                rgb,
                rgb_supported,
                cycle_supported,
                breathe_supported,
                effect: RgbEffect::Static,
            },
        ),
    }))
}

fn camera_card(c: camera::V4l2Camera) -> Card {
    let subtitle = format!("serial {} · {}", c.serial(), c.device_path());
    let controls: Vec<CamControl> = c
        .controls()
        .iter()
        .map(|ctl| {
            let is_bool = ctl.min == 0 && ctl.max == 1 && ctl.step == 1;
            let value = c.get_control(&ctl.name).unwrap_or(ctl.default);
            CamControl {
                name: ctl.name.clone(),
                min: ctl.min,
                max: ctl.max,
                step: ctl.step,
                value,
                is_bool,
            }
        })
        .collect();
    let preview = Arc::new(c.create_preview());
    Card {
        name: c.name().to_string(),
        subtitle,
        serial: c.serial().to_string(),
        status: "ready".to_string(),
        art: ART_WEBCAM,
        selected_section: 0,
        device: Device::Camera(Arc::new(c), CameraUi { controls, preview }),
    }
}

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Paint {
    t: tok::Theme,
    accent: Color,
}

/// A horizontal slider in value space, with a numeric readout to its right.
#[allow(clippy::too_many_arguments)]
fn slider(
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

fn toggle_row(
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

fn pill(
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

fn chip(p: Paint, key: impl Into<String>, label: &str) -> El {
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
fn labeled_slider(
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

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = cx.system_dark();
    let p = Paint {
        t: tok::theme(dark),
        accent: accent(dark),
    };
    let scale = cx.env().scale;
    match s.selected {
        Some(i) => device_page(s, i, p),
        None => dashboard(s, p, scale),
    }
}

fn accent(_dark: bool) -> Color {
    tok::accent(0, true)
}

fn dashboard(s: &State, p: Paint, scale: f32) -> El {
    let t = p.t;
    col()
        .key("dashboard")
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
                .justify_between()
                .child(
                    text("Open G Hub")
                        .key("title")
                        .font(type_::TITLE)
                        .color(t.text),
                )
                .child(
                    row()
                        .key("refresh")
                        .role("button")
                        .label("Refresh")
                        .h(28.0)
                        .px(space::S3)
                        .center()
                        .radius(radius::MD)
                        .fill(t.card)
                        .border(1.0, t.card_border)
                        .focusable(true)
                        .on_press(|_s: &mut State| refresh())
                        .child(text("Refresh").font(type_::LABEL).color(t.text)),
                ),
        )
        .child(
            col()
                .key("grid")
                .grow(1.0)
                .pt(space::S3)
                .child(if s.cards.is_empty() {
                    empty_state(p)
                } else {
                    let mut r = row().key("cards").gap(space::S3).flex_wrap();
                    for (i, card) in s.cards.iter().enumerate() {
                        r = r.child(card_tile(p, i, card, s.images.as_ref(), scale));
                    }
                    r
                }),
        )
}

fn empty_state(p: Paint) -> El {
    let t = p.t;
    col()
        .key("empty")
        .grow(1.0)
        .center()
        .child(
            text("No supported Logitech devices found.\nMake sure the device is plugged in and the udev rule has been applied (see the README).")
                .key("hint")
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        )
}

fn card_tile(p: Paint, i: usize, card: &Card, images: Option<&ImageCache>, scale: f32) -> El {
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

fn device_page(s: &State, i: usize, p: Paint) -> El {
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

fn section_rail(s: &State, i: usize, p: Paint, secs: &[(&'static str, &'static str)]) -> El {
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

fn device_content(s: &State, i: usize, p: Paint) -> El {
    let card = &s.cards[i];
    let section = card.selected_section;
    match &card.device {
        Device::Mouse(_, ui) => mouse_content(i, ui, section, p),
        Device::Headset(_, ui) => headset_content(i, ui, section, p),
        Device::Litra(_, ui) => litra_content(i, ui, section, p),
        Device::Camera(_, ui) => camera_content(i, ui, section, p, s.preview_image.as_ref()),
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

fn mouse_content(i: usize, ui: &MouseUi, section: usize, p: Paint) -> El {
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

fn rgb_row(
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
            RT.spawn(async move {
                let _ = match effect {
                    RgbEffect::Static => m.set_color(r, g, b).await,
                    RgbEffect::Breathe => m.set_breathe(r, g, b).await,
                    RgbEffect::Cycle => m.set_cycle().await,
                };
            });
            persist(&serial, |st| {
                st.red = r;
                st.green = g;
                st.blue = b;
            });
        },
    )
}

fn apply_rgb_effect(s: &mut State, card: usize, effect: RgbEffect) {
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

fn headset_content(i: usize, ui: &HeadsetUi, section: usize, p: Paint) -> El {
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
                    RT.spawn(async move {
                        let _ = h.set_sidetone(v as i32).await;
                    });
                    persist(&serial, |st| st.sidetone = v as i32);
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

fn band_row(p: Paint, card: usize, band: usize, label: &str, value: f32, min: f32, max: f32) -> El {
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
                let saved = levels.clone();
                let h = h.clone();
                RT.spawn(async move {
                    let _ = h.set_eq_levels(&levels).await;
                });
                persist(&serial, |st| st.equalizer_levels = Some(saved));
            }
        },
    )
}

fn litra_content(i: usize, ui: &LitraUi, section: usize, p: Paint) -> El {
    let t = p.t;
    match section {
        0 => {
            let mut c = col().key("lighting").gap(space::S3);
            let on = ui.on;
            c = c.child(toggle_row(p, "power", "Power", on, move |s: &mut State| {
                let serial = s.cards[i].serial.clone();
                let (l, ui) = s.cards[i].litra_mut();
                ui.on = !ui.on;
                let on = ui.on;
                let _ = l.set_power(on);
                persist(&serial, |st| st.power = on);
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
                    let _ = l.set_brightness(v as i32);
                    persist(&serial, |st| st.brightness = v as i32);
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
                    let _ = l.set_temperature(v as i32);
                    persist(&serial, |st| st.temperature = v as i32);
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
fn apply_litra_preset(s: &mut State, i: usize, on: bool, brightness: i32, temperature: i32) {
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

fn camera_content(
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
                    let (cam, ui) = s.cards[i].camera_mut();
                    for c in &mut ui.controls {
                        if c.name == name_c {
                            c.value = v as i32;
                            let val = v as i32;
                            let cam = cam.clone();
                            let n = name_c.clone();
                            let _ = cam.set_control(&n, val);
                        }
                    }
                },
            ));
        }
    }
    c
}

fn stub_content(p: Paint, title: &str, note: &str, items: &[&str]) -> El {
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

fn srgb(v: u8) -> f32 {
    rune::color::srgb_decode(v as f32 / 255.0)
}

fn freq_label(hz: i32) -> String {
    if hz >= 1000 {
        format!("{} kHz", hz as f32 / 1000.0)
    } else {
        format!("{hz} Hz")
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn on_key(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.key == Key::Escape {
        s.selected = None;
        return true;
    }
    false
}

/// A Litra card backed by `/dev/null` (writes are no-ops), so the demo scene can
/// show the full control surface without hardware.
fn demo_litra() -> Card {
    let fd: std::os::fd::OwnedFd = std::fs::File::open("/dev/null").expect("/dev/null").into();
    Card {
        name: "Litra Glow".to_string(),
        subtitle: "serial DEMO0001".to_string(),
        serial: "DEMO0001".to_string(),
        status: "ready".to_string(),
        art: ART_LITRA_GLOW,
        selected_section: 0,
        device: Device::Litra(
            Arc::new(litra::LitraLight::new(fd)),
            LitraUi {
                on: true,
                brightness: 75,
                temperature: 4200,
            },
        ),
    }
}

fn demo_cards() -> Vec<Card> {
    let mut cards = vec![demo_litra()];
    if let Some(node) = camera::V4l2Camera::enumerate_nodes().into_iter().find(|n| {
        camera::V4l2Camera::open(n)
            .map(|c| c.is_some())
            .unwrap_or(false)
    }) && let Ok(Some(c)) = camera::V4l2Camera::open(&node)
    {
        cards.push(camera_card(c));
    }
    cards
}

fn demo_state(s: &mut State) {
    s.cards = demo_cards();
    s.selected = Some(0);
}

fn demo_camera_state(s: &mut State) {
    s.cards = demo_cards();
    s.selected = Some(s.cards.len().saturating_sub(1));
}

fn demo_dash_state(s: &mut State) {
    s.cards = demo_cards();
    s.selected = None;
}

fn stop_camera_previews(s: &State) {
    for card in &s.cards {
        if let Device::Camera(_, ui) = &card.device {
            ui.preview.stop();
        }
    }
}

fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = bgra.to_vec();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    rgba
}

/// Drives the live webcam preview: starts the selected camera's stream and
/// publishes each new frame as an [`Image`] for the view to draw.
fn poll(s: &mut State, _clock: f64, tx: &mut Tx) -> bool {
    let Some(i) = s.selected else {
        stop_camera_previews(s);
        s.preview_image = None;
        s.preview_pending = None;
        return false;
    };

    let selected = match &s.cards[i].device {
        Device::Camera(_, ui) => Some(Arc::clone(&ui.preview)),
        _ => None,
    };
    for (j, card) in s.cards.iter().enumerate() {
        if j != i
            && let Device::Camera(_, ui) = &card.device
        {
            ui.preview.stop();
        }
    }

    let Some(preview) = selected else {
        s.preview_image = None;
        s.preview_pending = None;
        return false;
    };

    // Keep polling at ~15 fps while the camera page is open.
    tx.wake_after(Duration::from_millis(66));
    let _ = preview.start(PIX_FMT_YUYV);

    // Promote a freshly-decoded frame only once it's ready, so the view never
    // draws a pending (blank) image — the source of the flicker.
    let promoted = s.preview_pending.as_ref().is_some_and(|im| im.is_ready());
    if promoted {
        s.preview_image = s.preview_pending.take();
    }

    // Read the latest frame into the pending slot.
    if let Some(frame) = preview.try_get_frame()
        && frame.sequence != s.preview_seq
    {
        s.preview_seq = frame.sequence;
        if let Some(images) = &s.images {
            let image = if frame.is_jpeg {
                images.load(frame.pixels)
            } else {
                let rgba = bgra_to_rgba(&frame.pixels);
                images.rgba(frame.width as u32, frame.height as u32, rgba)
            };
            s.preview_pending = Some(image);
        }
    }

    promoted
}

fn main() {
    let spec = AppSpec::<State> {
        min_size: Some((760.0, 480.0)),
        scenes: &[
            ("demo", demo_state),
            ("demo-camera", demo_camera_state),
            ("demo-dash", demo_dash_state),
        ],
        setup: Some(|s, waker| s.images = Some(ImageCache::new(waker))),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("Open G Hub", (1000.0, 700.0), State::default, view)
    };

    let app = rune::App::new(spec);
    let handle = app.handle();
    let _ = HANDLE.set(handle.clone());
    RT.spawn(async move {
        let cards = discover().await;
        handle.update(move |s, _| s.cards = cards);
    });

    app.run();
}

/// Re-runs device discovery: clears the current cards (disposing their device
/// handles) then re-discovers and installs the fresh set. This is how a device
/// that was asleep at launch (e.g. the G Pro mouse) appears once it wakes.
fn refresh() {
    let Some(handle) = HANDLE.get().cloned() else {
        return;
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let clear = handle.clone();
    clear.update(move |s, _| {
        s.cards.clear();
        s.selected = None;
        s.preview_image = None;
        s.preview_pending = None;
        let _ = tx.send(());
    });
    RT.spawn(async move {
        // Wait for the clear to commit (old handles disposed) before opening
        // new ones, so a receiver's responses aren't stolen by a stale reader.
        let _ = rx.await;
        let cards = discover().await;
        handle.update(move |s, _| s.cards = cards);
    });
}
