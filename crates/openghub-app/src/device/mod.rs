use std::sync::Arc;

use openghub_core::devices::{camera, headset, litra, mouse};
use openghub_core::hid::HidBackend;

use crate::app::STORE;
use crate::state::State;

pub(crate) mod content;
pub(crate) mod preview;

pub(crate) use content::{card_tile, device_page};
pub(crate) use preview::poll;
#[cfg(unix)]
pub(crate) const ART_GPRO: &[u8] = include_bytes!("../../assets/gpro.svg");
#[cfg(unix)]
pub(crate) const ART_PROX: &[u8] = include_bytes!("../../assets/prox.svg");
#[cfg(unix)]
pub(crate) const ART_LITRA_GLOW: &[u8] = include_bytes!("../../assets/litra-glow.svg");
#[cfg(unix)]
pub(crate) const ART_LITRA_BEAM: &[u8] = include_bytes!("../../assets/litra-beam.svg");
#[cfg(unix)]
pub(crate) const ART_WEBCAM: &[u8] = include_bytes!("../../assets/webcam.svg");
#[cfg(unix)]
pub(crate) struct Card {
    pub(crate) name: String,
    pub(crate) subtitle: String,
    pub(crate) status: String,
    pub(crate) serial: String,
    pub(crate) art: &'static [u8],
    pub(crate) selected_section: usize,
    pub(crate) device: Device,
}

#[cfg(unix)]
pub(crate) enum Device {
    Mouse(Arc<mouse::GProMouse>, MouseUi),
    Headset(Arc<headset::ProxHeadset>, HeadsetUi),
    Litra(Arc<litra::LitraLight>, LitraUi),
    Camera(Arc<camera::V4l2Camera>, CameraUi),
    Error(String),
}

#[cfg(unix)]
pub(crate) struct MouseUi {
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
#[cfg(unix)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum RgbEffect {
    #[default]
    Static,
    Cycle,
    Breathe,
}

#[cfg(unix)]
pub(crate) struct HeadsetUi {
    sidetone: i32,
    noise_reduction: bool,
    has_noise_reduction: bool,
    eq: Option<EqUi>,
    battery: Option<(i32, bool)>,
}

#[cfg(unix)]
pub(crate) struct EqUi {
    frequencies: Vec<i32>,
    min_db: i32,
    max_db: i32,
    levels: Vec<i32>,
}

#[cfg(unix)]
pub(crate) struct LitraUi {
    on: bool,
    brightness: i32,
    temperature: i32,
}

#[cfg(unix)]
pub(crate) struct CameraUi {
    controls: Vec<CamControl>,
    preview: Arc<camera::WebcamPreview>,
}

#[cfg(unix)]
pub(crate) struct CamControl {
    name: String,
    min: i32,
    max: i32,
    step: i32,
    value: i32,
    is_bool: bool,
}

#[cfg(unix)]
impl Card {
    pub(crate) fn mouse_mut(&mut self) -> (&Arc<mouse::GProMouse>, &mut MouseUi) {
        match &mut self.device {
            Device::Mouse(m, ui) => (m, ui),
            _ => unreachable!(),
        }
    }
    pub(crate) fn headset_mut(&mut self) -> (&Arc<headset::ProxHeadset>, &mut HeadsetUi) {
        match &mut self.device {
            Device::Headset(h, ui) => (h, ui),
            _ => unreachable!(),
        }
    }
    pub(crate) fn litra_mut(&mut self) -> (&Arc<litra::LitraLight>, &mut LitraUi) {
        match &mut self.device {
            Device::Litra(l, ui) => (l, ui),
            _ => unreachable!(),
        }
    }
    pub(crate) fn camera_mut(&mut self) -> (&Arc<camera::V4l2Camera>, &mut CameraUi) {
        match &mut self.device {
            Device::Camera(c, ui) => (c, ui),
            _ => unreachable!(),
        }
    }

    /// One-line dashboard tile summary, derived from live state.
    pub(crate) fn summary(&self) -> String {
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
#[cfg(unix)]
pub(crate) fn sections(card: &Card) -> Vec<(&'static str, &'static str)> {
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

#[cfg(unix)]
pub(crate) fn display_name(name: &str) -> String {
    name.to_uppercase()
}

// ---------------------------------------------------------------------------
// Discovery (async, run once on a background task)
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub(crate) async fn discover() -> Vec<Card> {
    let backend = HidBackend;
    let mut cards = Vec::new();

    for info in backend.enumerate(0x046d, 0) {
        if info.product_id == litra::PRODUCT_GLOW || info.product_id == litra::PRODUCT_BEAM {
            cards.push(litra_card(&backend, &info));
        } else if info.product_id == headset::PRODUCT_ID {
            match headset_card(&backend, &info).await {
                Ok(Some(c)) => {
                    // Deduplicate by subtitle (serial).
                    if !cards
                        .iter()
                        .any(|existing: &Card| existing.subtitle == c.subtitle)
                    {
                        cards.push(c);
                    }
                }
                Ok(None) => eprintln!("openghub: headset at {} is asleep/unreachable", info.path),
                Err(e) => eprintln!("openghub: headset at {} failed: {e}", info.path),
            }
        } else if mouse::GProMouse::is_mouse_product(info.product_id) {
            match mouse_card(&backend, &info).await {
                Ok(Some(c)) => {
                    // Deduplicate: the receiver (c539) and the direct device
                    // (4079) are the same mouse.
                    if !cards
                        .iter()
                        .any(|existing: &Card| existing.subtitle == c.subtitle)
                    {
                        cards.push(c);
                    }
                }
                Ok(None) => eprintln!("openghub: mouse at {} is a duplicate sibling", info.path),
                Err(e) => eprintln!("openghub: mouse at {} failed: {e}", info.path),
            }
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

#[cfg(unix)]
pub(crate) fn litra_art(product_id: u16) -> &'static [u8] {
    if product_id == litra::PRODUCT_BEAM {
        ART_LITRA_BEAM
    } else {
        ART_LITRA_GLOW
    }
}

#[cfg(unix)]
pub(crate) const STAGE_PRESETS: [i32; 7] = [400, 800, 1600, 3200, 6400, 12800, 25600];

#[cfg(unix)]
pub(crate) fn default_stages(min: i32, max: i32) -> Vec<i32> {
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

#[cfg(unix)]
pub(crate) fn sanitize_stages(input: Vec<i32>, min: i32, max: i32) -> Vec<i32> {
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

#[cfg(unix)]
pub(crate) fn litra_card(backend: &HidBackend, info: &openghub_core::hid::DeviceInfo) -> Card {
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

#[cfg(unix)]
pub(crate) async fn headset_card(
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

#[cfg(unix)]
pub(crate) async fn mouse_card(
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

#[cfg(unix)]
pub(crate) fn camera_card(c: camera::V4l2Camera) -> Card {
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
#[cfg(unix)]
pub(crate) fn demo_litra() -> Card {
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

#[cfg(unix)]
pub(crate) fn demo_cards() -> Vec<Card> {
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

#[cfg(unix)]
pub(crate) fn demo_state(s: &mut State) {
    s.cards = demo_cards();
    s.selected = Some(0);
}

#[cfg(unix)]
pub(crate) fn demo_camera_state(s: &mut State) {
    s.cards = demo_cards();
    s.selected = Some(s.cards.len().saturating_sub(1));
}

#[cfg(unix)]
pub(crate) fn demo_dash_state(s: &mut State) {
    s.cards = demo_cards();
    s.selected = None;
}
