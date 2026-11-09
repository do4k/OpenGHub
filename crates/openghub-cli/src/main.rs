//! User-facing CLI for Open G Hub — scripting-friendly wrapper over the
//! same verified hardware paths the GUI uses. Run `openghub-cli --help`.

use openghub_core::devices::{headset, litra, mouse};
use openghub_core::hid::{self, HidBackend};

const VENDOR_ID: u16 = 0x046d;

fn usage() -> &'static str {
    "openghub-cli — control Logitech devices from the shell.

Usage: openghub-cli [OPTIONS] <command> [args]

Commands:
  devices                  list detected devices
  battery                  battery % for wireless devices (mouse, headset)
  dpi get                  read the mouse's current DPI
  dpi set <N>              set the mouse's DPI, then read it back
  report-rate get          read the mouse's polling rate (Hz)
  report-rate set <Hz>     set the polling rate, then read it back
  rgb set <RRGGBB>         set the logo LED to a static color (no read-back)
  sidetone get             read the headset sidetone (%)
  sidetone set <N>         set sidetone 0..100, then read it back
  eq get                   read the headset equalizer levels (dB)
  eq set <csv>             set EQ levels, comma-separated dB, then read back
  lights on|off            power a Litra light on/off
  lights set <0..100>      set a Litra light's brightness (%)

Options:
  --json                   machine-parseable JSON output
  --help, -h               show this help

Exit codes: 0 ok, 2 usage error, 1 hardware error (e.g. device asleep)."
}

#[derive(Default)]
struct Opts {
    json: bool,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut rest = Vec::new();
    let mut opt = Opts::default();
    for a in &args {
        match a.as_str() {
            "--json" => opt.json = true,
            "--help" | "-h" | "help" => {
                println!("{}", usage());
                return;
            }
            _ => rest.push(a.clone()),
        }
    }
    if rest.is_empty() {
        eprintln!("{}", usage());
        std::process::exit(2);
    }
    let cmd = rest[0].clone();
    let argv = &rest[1..];
    let backend = HidBackend;
    match cmd.as_str() {
        "devices" => devices(&backend, &opt).await,
        "battery" => battery(&backend, &opt).await,
        "dpi" => dpi(&backend, &opt, argv).await,
        "report-rate" => report_rate(&backend, &opt, argv).await,
        "rgb" => rgb(&backend, &opt, argv).await,
        "sidetone" => sidetone(&backend, &opt, argv).await,
        "eq" => eq(&backend, &opt, argv).await,
        "lights" => lights(&backend, &opt, argv).await,
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

/// Escapes a string for JSON output.
fn jstr(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn fail(msg: String) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
}

fn usage_err(msg: String) -> ! {
    eprintln!("error: {msg}");
    eprintln!("{}", usage());
    std::process::exit(2);
}

fn need_device(what: &str) -> ! {
    fail(format!("no {what} found — is it powered on and awake?"))
}

/// Opens the first reachable mouse (dedupe is handled inside `open`).
async fn find_mouse(backend: &HidBackend) -> Option<mouse::GProMouse> {
    for info in backend.enumerate(VENDOR_ID, 0) {
        if mouse::GProMouse::is_mouse_product(info.product_id)
            && let Ok(Some(m)) = mouse::GProMouse::open(backend, &info).await
        {
            return Some(m);
        }
    }
    None
}

async fn find_headset(backend: &HidBackend) -> Option<headset::ProxHeadset> {
    for info in backend.enumerate(VENDOR_ID, 0) {
        if info.product_id == headset::PRODUCT_ID
            && let Ok(Some(h)) = headset::ProxHeadset::open(backend, &info).await
        {
            return Some(h);
        }
    }
    None
}

fn find_litra(backend: &HidBackend) -> Option<(hid::DeviceInfo, litra::LitraLight)> {
    for info in backend.enumerate(VENDOR_ID, 0) {
        if (info.product_id == litra::PRODUCT_GLOW || info.product_id == litra::PRODUCT_BEAM)
            && let Ok(fd) = backend.open(&info)
        {
            return Some((info, litra::LitraLight::new(fd)));
        }
    }
    None
}

async fn devices(backend: &HidBackend, opt: &Opts) {
    let devices = backend.enumerate(VENDOR_ID, 0);
    if opt.json {
        let mut items = Vec::new();
        for d in &devices {
            items.push(format!(
                "{{\"path\":\"{}\",\"vendor\":\"{:04x}\",\"product\":\"{:04x}\",\"serial\":\"{}\"}}",
                d.path,
                d.vendor_id,
                d.product_id,
                jstr(d.serial.as_deref().unwrap_or(""))
            ));
        }
        println!("[{}]", items.join(","));
    } else {
        println!("{} Logitech HID devices:", devices.len());
        for d in &devices {
            println!(
                "  {:14} {:04x}:{:04x} serial='{}'",
                d.path,
                d.vendor_id,
                d.product_id,
                d.serial.as_deref().unwrap_or("")
            );
        }
    }
}

async fn battery(backend: &HidBackend, opt: &Opts) {
    #[derive(Debug)]
    struct Row {
        name: String,
        percent: i32,
        charging: bool,
    }
    let mut rows = Vec::new();
    if let Some(m) = find_mouse(backend).await
        && let Ok((percent, charging)) = m.get_battery().await
    {
        rows.push(Row {
            name: m.name().to_string(),
            percent,
            charging,
        });
    }
    if let Some(h) = find_headset(backend).await
        && let Ok((percent, charging)) = h.get_battery().await
    {
        rows.push(Row {
            name: h.name().to_string(),
            percent,
            charging,
        });
    }
    if opt.json {
        let items: Vec<String> = rows
            .iter()
            .map(|r| {
                format!(
                    "{{\"device\":\"{}\",\"percent\":{},\"charging\":{}}}",
                    jstr(&r.name),
                    r.percent,
                    r.charging
                )
            })
            .collect();
        println!("[{}]", items.join(","));
    } else {
        for r in &rows {
            println!(
                "{}: {}% ({})",
                r.name,
                r.percent,
                if r.charging {
                    "charging"
                } else {
                    "discharging"
                }
            );
        }
        if rows.is_empty() {
            println!("no battery-reporting device found");
        }
    }
}

async fn dpi(backend: &HidBackend, opt: &Opts, argv: &[String]) {
    let Some(m) = find_mouse(backend).await else {
        need_device("mouse");
    };
    if argv.len() == 1 && argv[0] == "get" {
        match m.get_dpi().await {
            Ok(dpi) => {
                let (min, max, step) = m.dpi_range();
                if opt.json {
                    println!("{{\"dpi\":{dpi},\"min\":{min},\"max\":{max},\"step\":{step}}}");
                } else {
                    println!("DPI: {dpi} (range {min}-{max}, step {step})");
                }
            }
            Err(e) => fail(e.to_string()),
        }
    } else if argv.len() == 2 && argv[0] == "set" {
        let n = &argv[1];
        {
            let Ok(v) = n.parse::<i32>() else {
                usage_err(format!("invalid DPI: {n}"));
            };
            if let Err(e) = m.set_dpi(v).await {
                fail(e.to_string());
            }
            match m.get_dpi().await {
                Ok(back) => {
                    if opt.json {
                        println!("{{\"set\":{v},\"readback\":{back}}}");
                    } else {
                        println!("DPI set to {v} (read back {back})");
                    }
                }
                Err(_) => {
                    if opt.json {
                        println!("{{\"set\":{v}}}");
                    } else {
                        println!("DPI written, read-back failed");
                    }
                }
            }
        }
    } else {
        usage_err("dpi subcommand: get | set <N>".into())
    }
}

async fn report_rate(backend: &HidBackend, opt: &Opts, argv: &[String]) {
    let Some(m) = find_mouse(backend).await else {
        need_device("mouse");
    };
    if argv.len() == 1 && argv[0] == "get" {
        match m.get_report_rate().await {
            Ok(hz) => {
                let options: Vec<String> = m
                    .report_rate_options()
                    .iter()
                    .map(|o| o.to_string())
                    .collect();
                if opt.json {
                    println!("{{\"hz\":{hz},\"options\":[{}]}}", options.join(","));
                } else {
                    println!("report rate: {hz} Hz (options: {})", options.join(", "));
                }
            }
            Err(e) => fail(e.to_string()),
        }
    } else if argv.len() == 2 && argv[0] == "set" {
        let n = &argv[1];
        {
            let Ok(v) = n.parse::<i32>() else {
                usage_err(format!("invalid rate: {n}"));
            };
            if let Err(e) = m.set_report_rate(v).await {
                fail(e.to_string());
            }
            match m.get_report_rate().await {
                Ok(back) => {
                    if opt.json {
                        println!("{{\"set\":{v},\"readback\":{back}}}");
                    } else {
                        println!("report rate set to {v} (read back {back})");
                    }
                }
                Err(_) => {
                    if opt.json {
                        println!("{{\"set\":{v}}}");
                    } else {
                        println!("report rate written, read-back failed");
                    }
                }
            }
        }
    } else {
        usage_err("report-rate subcommand: get | set <Hz>".into())
    }
}

async fn rgb(backend: &HidBackend, opt: &Opts, argv: &[String]) {
    let Some(m) = find_mouse(backend).await else {
        need_device("mouse");
    };
    if argv.len() == 2 && argv[0] == "set" {
        let hex = &argv[1];
        {
            let Some((r, g, b)) = parse_hex6(hex) else {
                usage_err(format!("expected RRGGBB, got: {hex}"));
            };
            if let Err(e) = m.set_color(r, g, b).await {
                fail(e.to_string());
            }
            if opt.json {
                println!("{{\"r\":{r},\"g\":{g},\"b\":{b}}}");
            } else {
                println!("logo LED set to #{r:02x}{g:02x}{b:02x} (no read-back protocol)");
            }
        }
    } else {
        usage_err("rgb subcommand: set <RRGGBB>".into())
    }
}

async fn sidetone(backend: &HidBackend, opt: &Opts, argv: &[String]) {
    let Some(h) = find_headset(backend).await else {
        need_device("headset");
    };
    if argv.len() == 1 && argv[0] == "get" {
        match h.get_sidetone().await {
            Ok(v) => {
                if opt.json {
                    println!("{{\"sidetone\":{v}}}");
                } else {
                    println!("sidetone: {v}/100");
                }
            }
            Err(e) => fail(e.to_string()),
        }
    } else if argv.len() == 2 && argv[0] == "set" {
        let n = &argv[1];
        {
            let Ok(v) = n.parse::<i32>() else {
                usage_err(format!("invalid sidetone: {n}"));
            };
            if let Err(e) = h.set_sidetone(v).await {
                fail(e.to_string());
            }
            match h.get_sidetone().await {
                Ok(back) => {
                    if opt.json {
                        println!("{{\"set\":{v},\"readback\":{back}}}");
                    } else {
                        println!("sidetone set to {v} (read back {back})");
                    }
                }
                Err(_) => {
                    if opt.json {
                        println!("{{\"set\":{v}}}");
                    } else {
                        println!("sidetone written, read-back failed");
                    }
                }
            }
        }
    } else {
        usage_err("sidetone subcommand: get | set <0..100>".into())
    }
}

async fn eq(backend: &HidBackend, opt: &Opts, argv: &[String]) {
    let Some(h) = find_headset(backend).await else {
        need_device("headset");
    };
    if argv.len() == 1 && argv[0] == "get" {
        match h.get_eq_levels().await {
            Ok(levels) => {
                let (min, max) = h.eq_range();
                let bands: Vec<String> = levels.iter().map(|v| v.to_string()).collect();
                if opt.json {
                    let freqs: Vec<String> =
                        h.eq_frequencies().iter().map(|v| v.to_string()).collect();
                    println!(
                        "{{\"bands\":[{}],\"freqs_hz\":[{}],\"min_db\":{min},\"max_db\":{max}}}",
                        bands.join(","),
                        freqs.join(",")
                    );
                } else {
                    println!(
                        "equalizer: {} bands ({}..{max} dB) levels: [{}]",
                        levels.len(),
                        min,
                        bands.join(", ")
                    );
                }
            }
            Err(e) => fail(e.to_string()),
        }
    } else if argv.len() == 2 && argv[0] == "set" {
        let csv = &argv[1];
        {
            let Some(levels) = parse_csv(csv) else {
                usage_err(format!("invalid EQ list: {csv}"));
            };
            if let Err(e) = h.set_eq_levels(&levels).await {
                fail(e.to_string());
            }
            match h.get_eq_levels().await {
                Ok(back) => {
                    let bands: Vec<String> = back.iter().map(|v| v.to_string()).collect();
                    if opt.json {
                        println!("{{\"set\":[{}],\"readback\":[{}]}}", csv, bands.join(","));
                    } else {
                        println!("EQ written: [{}] (read back [{}])", csv, bands.join(", "));
                    }
                }
                Err(_) => {
                    if opt.json {
                        println!("{{\"set\":[{csv}]}}");
                    } else {
                        println!("EQ written, read-back failed");
                    }
                }
            }
        }
    } else {
        usage_err("eq subcommand: get | set <csv>".into())
    }
}

async fn lights(backend: &HidBackend, opt: &Opts, argv: &[String]) {
    let Some((info, l)) = find_litra(backend) else {
        need_device("Litra");
    };
    let name = litra::LitraLight::friendly_name(info.product_id);
    if argv.len() == 1 && argv[0] == "on" {
        if let Err(e) = l.set_power(true) {
            fail(e.to_string());
        }
        if opt.json {
            println!("{{\"light\":\"{}\",\"on\":true}}", jstr(&name));
        } else {
            println!("{name} on");
        }
    } else if argv.len() == 1 && argv[0] == "off" {
        if let Err(e) = l.set_power(false) {
            fail(e.to_string());
        }
        if opt.json {
            println!("{{\"light\":\"{}\",\"on\":false}}", jstr(&name));
        } else {
            println!("{name} off");
        }
    } else if argv.len() == 2 && argv[0] == "set" {
        let n = &argv[1];
        {
            let Ok(v) = n.parse::<i32>() else {
                usage_err(format!("invalid brightness: {n}"));
            };
            if let Err(e) = l.set_brightness(v) {
                fail(e.to_string());
            }
            if opt.json {
                println!("{{\"light\":\"{}\",\"brightness\":{v}}}", jstr(&name));
            } else {
                println!("{name} brightness set to {v}% (write-only, no read-back)");
            }
        }
    } else {
        usage_err("lights subcommand: on | off | set <0..100>".into())
    }
}

/// Parses "RRGGBB" into (r, g, b).
fn parse_hex6(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Parses "1,2,-3" into a vector of i32.
fn parse_csv(s: &str) -> Option<Vec<i32>> {
    s.split(',').map(|x| x.trim().parse::<i32>().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_csv, parse_hex6};

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex6("00B0F5"), Some((0, 176, 245)));
        assert_eq!(parse_hex6("#ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex6("fff"), None);
        assert_eq!(parse_hex6("zzzzzz"), None);
    }

    #[test]
    fn csv_parsing() {
        assert_eq!(parse_csv("1,2,-3"), Some(vec![1, 2, -3]));
        assert_eq!(parse_csv("1, 2"), Some(vec![1, 2]));
        assert_eq!(parse_csv("a"), None);
        assert_eq!(parse_csv(""), None);
    }
}
