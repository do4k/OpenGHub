//! Console probe for hardware iterations — the Rust port of `OpenGHub.Poc`.
//! Run with `cargo run -p openghub-poc -- [flags]`.
//!
//! Flags (mirror the C# PoC):
//!   --only=PATH            only probe hidraw nodes whose path contains PATH
//!   --verbose              print per-index ping failures
//!   --demo                 power the Litra on (60% / 3200 K)
//!   --set-dpi=N            write a mouse DPI and read it back
//!   --set-rgb=RRGGBB       write a static LED color
//!   --set-sidetone=N       write headset sidetone and read it back
//!   --set-eq=db,db,...     write headset EQ levels and read them back
//!   --preview              stream 640x480 webcam preview (~2 s)
//!   --preview-yuyv         force the YUYV preview path

use std::time::{Duration, Instant};

use openghub_core::Error;
use openghub_core::devices::{camera, headset, litra, mouse};
use openghub_core::hid::{self, HidBackend};
use openghub_core::hidpp::{self, battery, root};
use openghub_core::v4l2::{PIX_FMT_MJPEG, PIX_FMT_YUYV};

const VENDOR_ID: u16 = 0x046d;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let value_of = |prefix: &str| -> Option<String> {
        args.iter()
            .find_map(|a| a.strip_prefix(prefix).map(|s| s.to_string()))
    };

    let demo = has("--demo");
    let verbose = has("--verbose");
    let only = value_of("--only=").or_else(|| {
        args.iter()
            .position(|a| a == "--only")
            .and_then(|i| args.get(i + 1).cloned())
    });

    let set_dpi = value_of("--set-dpi=").and_then(|s| s.parse::<i32>().ok());
    let set_rgb = value_of("--set-rgb=").and_then(|s| {
        if s.len() == 6 {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some((r, g, b))
        } else {
            None
        }
    });
    let set_sidetone = value_of("--set-sidetone=").and_then(|s| s.parse::<i32>().ok());
    let set_eq = value_of("--set-eq=");
    let preview = has("--preview");
    let preview_yuyv = has("--preview-yuyv");
    let wake = has("--wake");

    let backend = HidBackend;
    let mut devices = backend.enumerate(VENDOR_ID, 0);
    if let Some(only) = &only {
        devices.retain(|d| d.path.contains(only.as_str()));
    }

    println!("Found {} Logitech HID devices:", devices.len());
    for d in &devices {
        println!(
            "  {:14} v:{:04x} p:{:04x}  serial='{}'",
            d.path,
            d.vendor_id,
            d.product_id,
            d.serial.as_deref().unwrap_or("")
        );
    }

    if has("--probe-sensors") {
        for d in backend.enumerate(VENDOR_ID, 0) {
            if d.product_id != 0x4079 {
                continue;
            }
            println!("Probing sensors on {}", d.path);
            if let Ok(fd) = backend.open(&d) {
                let c = hidpp::HidppConn::new(fd);
                c.set_timeout(Duration::from_secs(1));
                probe_sensor_features(&c).await;
                // Second pass: with hidden features enabled (0x1e00).
                if let Ok(Some(hi)) =
                    hidpp::root::get_feature_index(&c, hidpp::DEVICE_INDEX_DIRECT, 0x1e00).await
                {
                    println!("  enabling hidden features (0x1e00 @ 0x{hi:02x})");
                    let _ = c.call(hidpp::DEVICE_INDEX_DIRECT, hi, 0x10, &[0x01]).await;
                    probe_sensor_features(&c).await;
                    let _ = c.call(hidpp::DEVICE_INDEX_DIRECT, hi, 0x10, &[0x00]).await;
                } else {
                    println!("  no hidden-features switch (0x1e00)");
                }
            }
        }
        println!("Done.");
        return;
    }

    if has("--probe-headset") {
        for d in backend.enumerate(VENDOR_ID, 0) {
            if d.product_id == headset::PRODUCT_ID {
                println!("Probing headset receiver {}", d.path);
                if let Ok(conn) = backend.open(&d) {
                    let c = hidpp::HidppConn::new(conn);
                    c.set_timeout(Duration::from_secs(2));
                    match hidpp::root::ping(&c, hidpp::DEVICE_INDEX_DIRECT).await {
                        Ok((a, b)) => println!("  0xFF responded: {:02x} {:02x}", a, b),
                        Err(e) => println!("  0xFF: {}", e),
                    }
                }
            }
        }
        println!("Done.");
        return;
    }

    if has("--probe-receiver") {
        for d in backend.enumerate(VENDOR_ID, 0) {
            if d.product_id == 0xc539 {
                println!("=== Receiver node {} ===", d.path);
                if let Ok(conn) = backend.open(&d) {
                    let c = hidpp::HidppConn::new(conn);
                    c.set_timeout(Duration::from_secs(2));
                    for idx in 0..=7u8 {
                        match hidpp::root::ping(&c, idx).await {
                            Ok((a, b)) => println!("  idx {} responded: {:02x} {:02x}", idx, a, b),
                            Err(e) => println!("  idx {}: {}", idx, e),
                        }
                    }
                } else {
                    println!("  could not open");
                }
            }
        }
        println!("Done.");
        return;
    }

    if wake {
        println!();
        println!("=== WAKE MODE: sending pings for 15 seconds ===");
        println!("Press a button on the mouse / power on the headset NOW.");
        println!();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut found_mouse = false;
        let mut found_headset = false;
        while Instant::now() < deadline {
            // Mouse via receiver
            for d in backend.enumerate(VENDOR_ID, 0) {
                if d.product_id == 0xc539
                    && let Ok(conn) = backend.open(&d)
                {
                    let c = hidpp::HidppConn::new(conn);
                    c.set_timeout(Duration::from_millis(200));
                    for idx in 1..=6u8 {
                        if root::ping(&c, idx).await.is_ok() && !found_mouse {
                            println!("  MOUSE RESPONDED on {} at index {}", d.path, idx);
                            found_mouse = true;
                        }
                    }
                }
            }
            // Mouse direct (Bluetooth)
            for d in backend.enumerate(VENDOR_ID, 0) {
                if d.product_id == 0x4079
                    && let Ok(conn) = backend.open(&d)
                {
                    let c = hidpp::HidppConn::new(conn);
                    c.set_timeout(Duration::from_millis(200));
                    if hidpp::root::ping(&c, hidpp::DEVICE_INDEX_DIRECT)
                        .await
                        .is_ok()
                        && !found_mouse
                    {
                        println!("  MOUSE (direct) RESPONDED on {}", d.path);
                        found_mouse = true;
                    }
                }
            }
            // Headset
            for d in backend.enumerate(VENDOR_ID, 0) {
                if d.product_id == headset::PRODUCT_ID
                    && let Ok(conn) = backend.open(&d)
                {
                    let c = hidpp::HidppConn::new(conn);
                    c.set_timeout(Duration::from_millis(200));
                    if hidpp::root::ping(&c, hidpp::DEVICE_INDEX_DIRECT)
                        .await
                        .is_ok()
                        && !found_headset
                    {
                        println!("  HEADSET RESPONDED on {}", d.path);
                        found_headset = true;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        println!();
        if !found_mouse {
            println!("  MOUSE: never responded (still asleep or not paired)");
        }
        if !found_headset {
            println!("  HEADSET: never responded (still asleep or powered off)");
        }
        println!("Done.");
        return;
    }

    for d in devices {
        if d.product_id == litra::PRODUCT_GLOW || d.product_id == litra::PRODUCT_BEAM {
            probe_litra(&backend, &d, demo);
            continue;
        }
        if d.product_id == headset::PRODUCT_ID {
            probe_headset(&backend, &d, set_sidetone, set_eq.as_deref()).await;
            continue;
        }
        if mouse::GProMouse::is_mouse_product(d.product_id) {
            probe_mouse(&backend, &d, set_dpi, set_rgb).await;
            continue;
        }
        probe_hidpp(&backend, &d, verbose).await;
    }

    for node in camera::V4l2Camera::enumerate_nodes() {
        probe_camera(&node, preview || preview_yuyv, preview_yuyv);
    }

    println!("Done.");
}

fn is_permission_denied(e: &Error) -> bool {
    matches!(e, Error::Io(io) if io.raw_os_error() == Some(libc::EACCES))
}

fn probe_camera(node: &str, preview: bool, force_yuyv: bool) {
    match camera::V4l2Camera::open(node) {
        Ok(None) => println!("  /dev/{node}: not a capture interface"),
        Ok(Some(cam)) => {
            println!("  /dev/{node}: {} (serial '{}')", cam.name(), cam.serial());
            for c in cam.controls() {
                match cam.get_control(&c.name) {
                    Ok(v) => println!(
                        "    {:<14} range [{}, {}] step {} default {}  now {v}",
                        c.name, c.min, c.max, c.step, c.default
                    ),
                    Err(e) => println!(
                        "    {:<14} range [{}, {}]  (read failed: {e})",
                        c.name, c.min, c.max
                    ),
                }
            }

            if !preview {
                return;
            }
            let p = cam.create_preview();
            let fmt = if force_yuyv {
                PIX_FMT_YUYV
            } else {
                PIX_FMT_MJPEG
            };
            if let Err(e) = p.start(fmt) {
                println!("    preview: failed to start: {e}");
                return;
            }
            if force_yuyv {
                println!("    preview: forcing YUYV");
            }
            let start = Instant::now();
            let mut last_seq = 0u64;
            let mut frames = 0;
            let (mut width, mut height) = (0, 0);
            let mut jpeg = false;
            while start.elapsed() < Duration::from_secs(2) {
                if let Some(frame) = p.try_get_frame()
                    && frame.sequence != last_seq
                {
                    last_seq = frame.sequence;
                    frames += 1;
                    width = frame.width;
                    height = frame.height;
                    jpeg = frame.is_jpeg;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            p.stop();
            println!(
                "    preview: {frames} frames in {:.1} s @ {width}x{height} ({})",
                start.elapsed().as_secs_f64(),
                if jpeg { "MJPEG" } else { "YUYV" }
            );
            if force_yuyv && last_seq != 0 && p.start(PIX_FMT_YUYV).is_ok() {
                let mut f = None;
                for _ in 0..30 {
                    if let Some(frame) = p.try_get_frame() {
                        f = Some(frame);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                p.stop();
                if let Some(f) = f {
                    let sample: Vec<String> =
                        f.pixels.iter().take(12).map(|b| b.to_string()).collect();
                    println!("    yuyv pixel probe: {}", sample.join(","));
                }
            }
        }
        Err(e) => {
            println!("  /dev/{node}: {e}");
        }
    }
}

async fn probe_mouse(
    backend: &HidBackend,
    d: &hid::DeviceInfo,
    set_dpi: Option<i32>,
    set_rgb: Option<(u8, u8, u8)>,
) {
    let mouse = match mouse::GProMouse::open(backend, d).await {
        Ok(Some(m)) => m,
        Ok(None) => return, // duplicate receiver sibling
        Err(e) if is_permission_denied(&e) => {
            println!(
                "  {}: permission denied — run the PoC with sudo (see note above)",
                d.path
            );
            return;
        }
        Err(e) => {
            println!("  {}: {e}", d.path);
            return;
        }
    };

    let (min, max, step) = mouse.dpi_range();
    println!(
        "  {}: {} (serial '{}', product {:04x})",
        d.path,
        mouse.name(),
        mouse.serial(),
        mouse.product_id()
    );

    match mouse.get_dpi().await {
        Ok(dpi) => println!("    DPI: {dpi} (range {min}-{max}, step {step})"),
        Err(e) => println!("    DPI: {e}"),
    }

    match mouse.get_report_rate().await {
        Ok(rate) => {
            let options: Vec<String> = mouse
                .report_rate_options()
                .iter()
                .map(|o| o.to_string())
                .collect();
            println!(
                "    report rate: {rate} Hz (options: {})",
                options.join(", ")
            );
        }
        Err(e) => println!("    report rate: {e}"),
    }

    match mouse.get_battery().await {
        Ok((percent, charging)) => println!(
            "    battery: {percent}% {}",
            if charging {
                "(charging)"
            } else {
                "(discharging)"
            }
        ),
        Err(e) => println!("    battery: {e}"),
    }

    if let Some(target) = set_dpi {
        match mouse.get_dpi().await {
            Ok(original) => match mouse.set_dpi(target).await {
                Ok(()) => match mouse.get_dpi().await {
                    Ok(after) => {
                        println!("    set DPI {target}: read back {after} (restoring {original})");
                        let _ = mouse.set_dpi(original).await;
                    }
                    Err(e) => println!("    set DPI: {e}"),
                },
                Err(e) => println!("    set DPI: {e}"),
            },
            Err(e) => println!("    set DPI: {e}"),
        }
    } else {
        println!("    pass --set-dpi=N to write a DPI and read it back");
    }

    if mouse.rgb_supported() {
        println!("    RGB: supported (logo zone)");
        if let Some((r, g, b)) = set_rgb {
            match mouse.set_color(r, g, b).await {
                Ok(()) => {
                    println!("    set RGB #{r:02x}{g:02x}{b:02x}: written (no read-back protocol)")
                }
                Err(e) => println!("    set RGB: {e}"),
            }
        } else {
            println!("    pass --set-rgb=RRGGBB to write a static LED color");
        }
    } else {
        println!("    RGB: not supported on this unit");
    }

    match mouse.get_buttons().await {
        Ok(buttons) => {
            let names: Vec<String> = buttons
                .iter()
                .map(|b| format!("{}{}", b.name, if b.is_divertable { "*" } else { "" }))
                .collect();
            println!(
                "    buttons: {} controls ({})",
                buttons.len(),
                names.join(", ")
            );
        }
        Err(e) => println!("    buttons: {e}"),
    }
}

async fn probe_hidpp(backend: &HidBackend, d: &hid::DeviceInfo, verbose: bool) {
    let fd = match backend.open(d) {
        Ok(fd) => fd,
        Err(e) if is_permission_denied(&e) => {
            println!(
                "  {}: permission denied — hidraw needs root or a udev rule (see the Open G Hub install/); run the PoC with sudo",
                d.path
            );
            return;
        }
        Err(e) => {
            println!("  {}: {e}", d.path);
            return;
        }
    };

    let conn = hidpp::HidppConn::new(fd);
    conn.set_timeout(Duration::from_millis(250));

    let mut live = Vec::new();
    let indices = [
        hidpp::DEVICE_INDEX_DIRECT,
        0x01,
        0x02,
        0x03,
        0x04,
        0x05,
        0x06,
    ];
    for idx in indices {
        match hidpp::root::ping(&conn, idx).await {
            Ok((major, minor)) => {
                live.push(idx);
                println!(
                    "  {}: HID++ device at index 0x{idx:02x}, protocol {major}.{minor}",
                    d.path
                );
            }
            Err(Error::Timeout { .. }) => {}
            Err(Error::Hidpp(e)) => println!("  {}: index 0x{idx:02x} rejected ping: {e}", d.path),
            Err(e) => {
                if verbose {
                    println!("  {}: ping 0x{idx:02x}: {e}", d.path);
                }
            }
        }
    }

    if live.is_empty() {
        println!("  {}: no HID++ device responded on any index", d.path);
        return;
    }

    for idx in live {
        match hidpp::name::get_device_name(&conn, idx).await {
            Ok(name) => {
                let feature_set = hidpp::root::get_feature_index(&conn, idx, 0x0001).await;
                let feature_set = match feature_set {
                    Ok(Some(i)) => format!("0x{i:02x}"),
                    _ => "??".to_string(),
                };
                println!(
                    "  {}: device 0x{idx:02x} is \"{name}\" (feature-set index {feature_set})",
                    d.path
                );
            }
            Err(e) => {
                if verbose {
                    println!("  {}: 0x{idx:02x} name/feature-set lookup: {e}", d.path);
                }
            }
        }

        match battery::resolve_battery_feature(&conn, idx).await {
            Ok((feature_idx, kind)) if kind != battery::BatteryKind::None => {
                match battery::read_battery(&conn, idx, feature_idx, kind).await {
                    Ok((percent, charging)) => println!(
                        "  {}: device 0x{idx:02x} battery: {percent}% {} [via {kind:?}]",
                        d.path,
                        if charging {
                            "(charging)"
                        } else {
                            "(discharging)"
                        }
                    ),
                    Err(e) => {
                        if verbose {
                            println!("  {}: 0x{idx:02x} battery: {e}", d.path);
                        }
                    }
                }
            }
            Ok(_) => println!(
                "  {}: device 0x{idx:02x} battery: no supported feature",
                d.path
            ),
            Err(e) => {
                if verbose {
                    println!("  {}: 0x{idx:02x} battery: {e}", d.path);
                }
            }
        }
    }
}

async fn probe_sensor_features(c: &hidpp::HidppConn) {
    for feat in [0x2230u16, 0x2240u16] {
        match hidpp::root::get_feature_index(c, hidpp::DEVICE_INDEX_DIRECT, feat).await {
            Ok(Some(fi)) => {
                println!("  0x{feat:04x} at feature index 0x{fi:02x}");
                for func in [0x00u8, 0x10u8, 0x20u8, 0x30u8] {
                    match c.call(hidpp::DEVICE_INDEX_DIRECT, fi, func, &[]).await {
                        Ok(resp) => println!("    func 0x{func:02x} -> {resp:?}"),
                        Err(e) => println!("    func 0x{func:02x}: {e}"),
                    }
                }
            }
            Ok(None) => println!("  0x{feat:04x}: not present"),
            Err(e) => println!("  0x{feat:04x}: {e}"),
        }
    }
}

async fn probe_headset(
    backend: &HidBackend,
    d: &hid::DeviceInfo,
    set_sidetone: Option<i32>,
    set_eq: Option<&str>,
) {
    let headset = match headset::ProxHeadset::open(backend, d).await {
        Ok(Some(h)) => h,
        Ok(None) => {
            println!("  {}: device did not respond to ping (asleep/off)", d.path);
            return;
        }
        Err(e) if is_permission_denied(&e) => {
            println!(
                "  {}: permission denied — run the PoC with sudo (see note above)",
                d.path
            );
            return;
        }
        Err(e) => {
            println!("  {}: {e}", d.path);
            return;
        }
    };

    println!(
        "  {}: {} (product {:04x})",
        d.path,
        headset.name(),
        headset::PRODUCT_ID
    );

    match headset.get_battery().await {
        Ok((percent, charging)) => println!(
            "    battery: {percent}% {}",
            if charging {
                "(charging)"
            } else {
                "(discharging)"
            }
        ),
        Err(e) => println!("    battery: {e}"),
    }

    match headset.get_sidetone().await {
        Ok(sidetone) => {
            println!("    sidetone: {sidetone}/100");
            if let Some(target) = set_sidetone {
                match headset.set_sidetone(target).await {
                    Ok(()) => match headset.get_sidetone().await {
                        Ok(after) => {
                            println!(
                                "    set sidetone {target}: read back {after} (restoring {sidetone})"
                            );
                            let _ = headset.set_sidetone(sidetone).await;
                        }
                        Err(e) => println!("    set sidetone: {e}"),
                    },
                    Err(e) => println!("    set sidetone: {e}"),
                }
            } else {
                println!("    pass --set-sidetone=N to write sidetone and read it back");
            }
        }
        Err(e) => println!("    sidetone: {e}"),
    }

    let freqs: Vec<String> = headset
        .eq_frequencies()
        .iter()
        .map(|f| format!("{f} Hz"))
        .collect();
    let (min_db, max_db) = headset.eq_range();
    match headset.get_eq_levels().await {
        Ok(levels) => {
            println!(
                "    equalizer: {} bands ({}) range {min_db}..{max_db} dB",
                headset.eq_frequencies().len(),
                freqs.join(", ")
            );
            let levels_str: Vec<String> = levels.iter().map(|l| l.to_string()).collect();
            println!("    eq levels: [{}]", levels_str.join(", "));
            if let Some(eq_arg) = set_eq {
                let targets: Vec<i32> = eq_arg
                    .split(',')
                    .filter_map(|s| s.trim().parse::<i32>().ok())
                    .collect();
                if targets.len() == headset.eq_frequencies().len() {
                    match headset.set_eq_levels(&targets).await {
                        Ok(()) => match headset.get_eq_levels().await {
                            Ok(after) => {
                                let after_str: Vec<String> =
                                    after.iter().map(|l| l.to_string()).collect();
                                let targets_str: Vec<String> =
                                    targets.iter().map(|l| l.to_string()).collect();
                                println!(
                                    "    set eq [{}]: read back [{}] (restoring [{}])",
                                    targets_str.join(", "),
                                    after_str.join(", "),
                                    levels_str.join(", ")
                                );
                                let _ = headset.set_eq_levels(&levels).await;
                            }
                            Err(e) => println!("    set eq: {e}"),
                        },
                        Err(e) => println!("    set eq: {e}"),
                    }
                } else {
                    println!(
                        "    --set-eq needs {} band values, got {}",
                        headset.eq_frequencies().len(),
                        targets.len()
                    );
                }
            } else {
                println!("    pass --set-eq=db,db,... to write EQ levels and read them back");
            }
        }
        Err(e) => println!("    equalizer: {e}"),
    }

    match headset.get_noise_reduction().await {
        Ok(nr) => println!("    noise reduction: {nr}"),
        Err(e) => println!("    noise reduction: not reported by this unit ({e})"),
    }
}

fn probe_litra(backend: &HidBackend, d: &hid::DeviceInfo, demo: bool) {
    let fd = match backend.open(d) {
        Ok(fd) => fd,
        Err(e) if is_permission_denied(&e) => {
            println!(
                "  {}: permission denied — run the PoC with sudo (see note above)",
                d.path
            );
            return;
        }
        Err(e) => {
            println!("  {}: {e}", d.path);
            return;
        }
    };

    let light = litra::LitraLight::new(fd);
    println!(
        "  {}: {} (write-only protocol, no state read-back)",
        d.path,
        litra::LitraLight::friendly_name(d.product_id)
    );

    if !demo {
        println!(
            "  pass --demo to power it on at 60% brightness / 3200 K so you can see writes working"
        );
        return;
    }

    let _ = light.set_power(true);
    let _ = light.set_brightness(60);
    let _ = light.set_temperature(3200);
    println!("  sent: power on, brightness 60%, temperature 3200 K — the light should be on now");
}
