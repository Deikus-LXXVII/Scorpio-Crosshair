//! Linux hardware, display, presentation and own-process cost queries for
//! the problem report. Everything reads `/proc/self`, `/proc/{cpu,mem}info`,
//! `/sys/class/drm` or X11 RandR / EWMH properties; no other process is opened.

use crate::report::{self, DisplayFact, GpuFact, PresentationFacts, ProcessCost, SystemFacts};
use std::fs;
use std::path::Path;
use x11rb::connection::Connection;
use x11rb::protocol::randr::{Connection as RandrConnection, ConnectionExt as _, ModeFlag};
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, Window};
use x11rb::rust_connection::RustConnection;

/// Scorpio's own CPU time and private memory from `/proc/self`.
pub fn process_cost() -> ProcessCost {
    let cpu_ms = fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| report::parse_proc_stat_ticks(&stat))
        .map(|ticks| ticks.saturating_mul(1000) / clock_ticks_per_sec());

    let private_memory_bytes = fs::read_to_string("/proc/self/smaps_rollup")
        .ok()
        .and_then(|smaps| report::parse_smaps_rollup_private_bytes(&smaps))
        .or_else(|| {
            fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|status| report::parse_status_rss_bytes(&status))
        });

    ProcessCost { cpu_ms, private_memory_bytes }
}

/// Reads `AT_CLKTCK` (key 17) from `/proc/self/auxv`, falling back to 100 Hz.
fn clock_ticks_per_sec() -> u64 {
    const AT_CLKTCK: usize = 17;
    let Ok(bytes) = fs::read("/proc/self/auxv") else { return 100 };
    let word = size_of::<usize>();
    for pair in bytes.chunks_exact(word * 2) {
        let key = usize::from_ne_bytes(pair[..word].try_into().unwrap());
        let val = usize::from_ne_bytes(pair[word..].try_into().unwrap());
        if key == 0 {
            break;
        }
        if key == AT_CLKTCK && val > 0 {
            return val as u64;
        }
    }
    100
}

/// Hardware, display and presentation facts, gathered inside `--system-facts`.
pub fn system_facts() -> SystemFacts {
    let processor = fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| report::parse_cpuinfo(&s));
    let memory_mb = fs::read_to_string("/proc/meminfo").ok().and_then(|s| report::parse_meminfo_mb(&s));
    let gpus = read_drm_gpus(Path::new("/sys/class/drm"));

    let session_type = Some(std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into()));
    let mut displays = Vec::new();
    let mut notes = Vec::new();
    let mut compositor = None;

    if std::env::var_os("DISPLAY").is_some() {
        match RustConnection::connect(None) {
            Ok((conn, screen_num)) => {
                let root = conn.setup().roots[screen_num].root;
                displays = read_randr_displays(&conn, root);
                compositor = Some(read_compositor(&conn, root, screen_num));
            }
            Err(error) => {
                notes.push(format!("X11 display query failed: {error}"));
                compositor = Some("unknown (X11 unavailable)".into());
            }
        }
    } else {
        compositor = Some("unknown (DISPLAY not set)".into());
    }

    let presentation = PresentationFacts { session_type, compositor, ..Default::default() };
    SystemFacts { processor, memory_mb, gpus, displays, presentation, notes }
}

fn read_drm_gpus(drm_root: &Path) -> Vec<GpuFact> {
    let Ok(entries) = fs::read_dir(drm_root) else { return Vec::new() };
    let mut cards: Vec<(String, std::path::PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rest = name.strip_prefix("card")?;
            (!rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())).then_some((name, entry.path()))
        })
        .collect();
    cards.sort_by(|a, b| a.0.cmp(&b.0));

    let mut gpus = Vec::new();
    for (card, path) in cards {
        let device_dir = path.join("device");
        if !device_dir.is_dir() {
            continue;
        }
        let vendor = read_trimmed(&device_dir.join("vendor"));
        let device = read_trimmed(&device_dir.join("device"));
        let driver = fs::read_link(device_dir.join("driver"))
            .ok()
            .and_then(|target| target.file_name().map(|n| n.to_string_lossy().into_owned()))
            .or_else(|| {
                fs::read_to_string(device_dir.join("uevent"))
                    .ok()
                    .and_then(|u| u.lines().find_map(|l| l.strip_prefix("DRIVER=").map(str::to_string)))
            });
        let driver_version = driver
            .as_deref()
            .and_then(|drv| read_trimmed(&Path::new("/sys/module").join(drv).join("version")));
        let dedicated_memory_mb = read_trimmed(&device_dir.join("mem_info_vram_total"))
            .and_then(|s| s.parse::<u64>().ok())
            .map(|bytes| (bytes + 524_288) / 1_048_576);
        let name = match (vendor.as_deref(), device.as_deref()) {
            (Some(v), Some(d)) => format!("{card} ({v}:{d})"),
            (Some(v), None) => format!("{card} ({v})"),
            _ => card,
        };
        gpus.push(GpuFact { name, dedicated_memory_mb, driver, driver_version, software: false });
    }
    gpus
}

fn read_trimmed(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn read_randr_displays(conn: &RustConnection, root: Window) -> Vec<DisplayFact> {
    let primary = conn.randr_get_output_primary(root).ok().and_then(|c| c.reply().ok()).map(|r| r.output);
    let res = conn
        .randr_get_screen_resources_current(root)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| (r.config_timestamp, r.outputs, r.modes))
        .or_else(|| {
            conn.randr_get_screen_resources(root)
                .ok()
                .and_then(|c| c.reply().ok())
                .map(|r| (r.config_timestamp, r.outputs, r.modes))
        });
    let Some((config_timestamp, outputs, modes)) = res else { return Vec::new() };

    let mut displays = Vec::new();
    for output in outputs {
        let Some(info) = conn.randr_get_output_info(output, config_timestamp).ok().and_then(|c| c.reply().ok()) else { continue };
        if info.connection != RandrConnection::CONNECTED || info.crtc == 0 {
            continue;
        }
        let Some(crtc) = conn.randr_get_crtc_info(info.crtc, config_timestamp).ok().and_then(|c| c.reply().ok()) else { continue };
        let name = String::from_utf8_lossy(&info.name).into_owned();
        let refresh_hz = modes.iter().find(|m| m.id == crtc.mode).and_then(|m| {
            if m.htotal == 0 || m.vtotal == 0 {
                return None;
            }
            let flags = u32::from(m.mode_flags);
            let mut vtotal = f64::from(m.vtotal);
            if flags & u32::from(ModeFlag::DOUBLE_SCAN) != 0 {
                vtotal *= 2.0;
            }
            if flags & u32::from(ModeFlag::INTERLACE) != 0 {
                vtotal /= 2.0;
            }
            let hz = (f64::from(m.dot_clock) / (f64::from(m.htotal) * vtotal)).round();
            (hz > 0.0).then_some(hz as u32)
        });
        displays.push(DisplayFact {
            name,
            width: u32::from(crtc.width),
            height: u32::from(crtc.height),
            refresh_hz,
            hdr: None,
            overlay_planes: None,
            primary: primary == Some(output),
        });
    }
    displays
}

fn read_compositor(conn: &RustConnection, root: Window, screen_num: usize) -> String {
    let intern = |name: &str| conn.intern_atom(false, name.as_bytes()).ok().and_then(|c| c.reply().ok()).map(|r| r.atom);
    let cm_atom = intern(&format!("_NET_WM_CM_S{screen_num}"));
    let net_wm_name = intern("_NET_WM_NAME");
    let utf8 = intern("UTF8_STRING");
    let supporting = intern("_NET_SUPPORTING_WM_CHECK");

    let window_name = |win: Window| -> Option<String> {
        if let (Some(a), Some(u)) = (net_wm_name, utf8) {
            if let Some(reply) = conn.get_property(false, win, a, u, 0, 1024).ok().and_then(|c| c.reply().ok()) {
                let s = String::from_utf8_lossy(&reply.value).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        if let Some(reply) = conn
            .get_property(false, win, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into(), 0, 1024)
            .ok()
            .and_then(|c| c.reply().ok())
        {
            let class = reply.value.split(|&b| b == 0).nth(1).map(|b| String::from_utf8_lossy(b).trim().to_string()).unwrap_or_default();
            if !class.is_empty() {
                return Some(class);
            }
        }
        if let Some(reply) = conn
            .get_property(false, win, AtomEnum::WM_NAME.into(), AtomEnum::ANY.into(), 0, 1024)
            .ok()
            .and_then(|c| c.reply().ok())
        {
            let s = String::from_utf8_lossy(&reply.value).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    };

    let supporting_wm = supporting.and_then(|atom: Atom| {
        let reply = conn.get_property(false, root, atom, AtomEnum::WINDOW.into(), 0, 1).ok()?.reply().ok()?;
        let win = reply.value32()?.next().filter(|&w| w != 0)?;
        window_name(win)
    });

    let cm_owner = cm_atom.and_then(|atom| conn.get_selection_owner(atom).ok()?.reply().ok()).map(|r| r.owner).filter(|&w| w != 0);

    if let Some(owner) = cm_owner {
        window_name(owner).or(supporting_wm).unwrap_or_else(|| "active (unnamed)".into())
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        supporting_wm.map_or_else(|| "Wayland".into(), |wm| format!("Wayland ({wm})"))
    } else if let Some(wm) = supporting_wm {
        format!("none (window manager: {wm})")
    } else {
        "none".into()
    }
}
