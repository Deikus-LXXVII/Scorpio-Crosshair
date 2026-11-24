//! "Report a problem": a plain text the player reads in full before
//! anything leaves the computer, and then copies, saves, attaches to a
//! GitHub issue, or sends with Send to Scorpio's own report service
//! (`REPORTS`). Nothing is sent without that press.
//!
//! The same text comes from `scorpio --report`, for the player's own tools.
//! Hardware, display and presentation facts are gathered in a short-lived
//! `scorpio --system-facts` child process so `dxgi.dll` is never imported by
//! or loaded into the main Scorpio process.

use crate::logging;
use crate::platform::counters;
use crate::store::Settings;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const ISSUES: &str = "https://github.com/Deikus-LXXVII/Scorpio-Crosshair/issues/new";
/// Scorpio's own report service (`server/reports`), for Send.
pub const REPORTS: &str = "https://reports.lxxvll.com/v1/reports";
/// The service takes 512 KB; a report is far smaller (200 log lines).
const SEND_LIMIT: usize = 500 * 1024;
/// Log lines in a report.
pub const LOG_LINES: usize = 200;
/// A report the settings window may ask to save, at most.
pub const TEXT_LIMIT: usize = 2 * 1024 * 1024;
/// Maximum characters for "What happened".
pub const DESCRIPTION_LIMIT: usize = 4000;
/// Maximum characters for "How to reach you".
pub const CONTACT_LIMIT: usize = 200;
/// Timeout waiting for `scorpio --system-facts`.
pub const SYSTEM_FACTS_TIMEOUT: Duration = Duration::from_secs(5);

/// Hardware, display and presentation facts printed as JSON by `--system-facts`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemFacts {
    #[serde(default)]
    pub processor: Option<String>,
    #[serde(default)]
    pub memory_mb: Option<u64>,
    #[serde(default)]
    pub gpus: Vec<GpuFact>,
    #[serde(default)]
    pub displays: Vec<DisplayFact>,
    #[serde(default)]
    pub presentation: PresentationFacts,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuFact {
    pub name: String,
    #[serde(default)]
    pub dedicated_memory_mb: Option<u64>,
    #[serde(default)]
    pub driver: Option<String>,
    #[serde(default)]
    pub driver_version: Option<String>,
    #[serde(default)]
    pub software: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayFact {
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub refresh_hz: Option<u32>,
    #[serde(default)]
    pub hdr: Option<bool>,
    #[serde(default)]
    pub overlay_planes: Option<bool>,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PresentationFacts {
    #[serde(default)]
    pub hags: Option<String>,
    #[serde(default)]
    pub windowed_optimisations: Option<String>,
    #[serde(default)]
    pub game_mode: Option<String>,
    #[serde(default)]
    pub compat_layer: Option<String>,
    #[serde(default)]
    pub session_type: Option<String>,
    #[serde(default)]
    pub compositor: Option<String>,
}

/// Scorpio's own process CPU time and private memory usage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessCost {
    pub cpu_ms: Option<u64>,
    pub private_memory_bytes: Option<u64>,
}

/// Scorpio's own resource cost and wake-up counts since start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CostFacts {
    pub process: ProcessCost,
    pub counters: counters::Snapshot,
}

/// What a report says, gathered at the boundary; `text` only formats it.
pub struct Facts {
    pub version: &'static str,
    pub system: String,
    pub made: String,
    /// What the running app is doing, in a few lines; or why it is not known.
    pub state: String,
    /// Hardware, displays and presentation settings from `scorpio --system-facts`.
    pub system_facts: Result<SystemFacts, String>,
    /// Scorpio's own cost and wake-ups since start; None for offline `--report`.
    pub cost: Option<CostFacts>,
    /// Optional player-typed description ("What happened"), unredacted.
    pub description: String,
    /// Optional player-typed contact ("How to reach you"), unredacted.
    pub contact: String,
    /// The settings, or why they could not be read.
    pub settings: Result<Settings, String>,
    pub log: Vec<String>,
}

/// Prints the JSON payload for `scorpio --system-facts`.
pub fn system_facts_json() -> String {
    serde_json::to_string(&crate::platform::system_facts()).unwrap_or_else(|_| "{}".into())
}

/// Runs `scorpio --system-facts` in a child process with a short timeout.
pub fn collect_system_facts() -> Result<SystemFacts, String> {
    let exe = std::env::current_exe().map_err(|error| format!("could not locate scorpio executable: {error}"))?;
    let mut cmd = Command::new(exe);
    cmd.arg("--system-facts");
    let stdout = run_child_with_timeout(cmd, SYSTEM_FACTS_TIMEOUT)?;
    parse_system_facts(&stdout)
}

pub fn parse_system_facts(json: &str) -> Result<SystemFacts, String> {
    let trimmed = json.trim();
    if trimmed.is_empty() {
        return Err("helper produced no output".into());
    }
    serde_json::from_str::<SystemFacts>(trimmed).map_err(|error| format!("helper returned invalid JSON: {error}"))
}

pub fn run_child_with_timeout(mut cmd: Command, timeout: Duration) -> Result<String, String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|error| format!("could not start helper process: {error}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stdout {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stderr {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out_reader.join();
                    let _ = err_reader.join();
                    return Err(format!("timed out after {} s", timeout.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_reader.join();
                let _ = err_reader.join();
                return Err(format!("could not wait for helper process: {error}"));
            }
        }
    };

    let out = out_reader.join().unwrap_or_default();
    let err = err_reader.join().unwrap_or_default();
    if !status.success() {
        let detail = String::from_utf8_lossy(&err).trim().to_string();
        return Err(if detail.is_empty() {
            format!("helper exited with {status}")
        } else {
            format!("helper exited with {status}: {detail}")
        });
    }
    String::from_utf8(out).map_err(|error| format!("helper output was not UTF-8: {error}"))
}

pub fn gather(state: String, settings: Result<Settings, String>, cost: Option<CostFacts>) -> Facts {
    let log = logging::paths::log_dir().map(|dir| logging::tail(&dir, LOG_LINES)).unwrap_or_default();
    let system_facts = collect_system_facts();
    Facts {
        version: env!("CARGO_PKG_VERSION"),
        system: system(),
        made: logging::now(),
        state,
        system_facts,
        cost,
        description: String::new(),
        contact: String::new(),
        settings,
        log,
    }
}

/// The report `scorpio --report` prints, made without starting the app.
pub fn from_files() -> String {
    let settings = match logging::paths::config_dir().map(|dir| crate::store::peek(&dir.join("settings.json"))) {
        Some(Ok(Some(settings))) => Ok(settings),
        Some(Ok(None)) => Err("no settings file yet: Scorpio uses its defaults".into()),
        Some(Err(error)) => Err(format!("the settings file is {error}")),
        None => Err("this system has no config folder".into()),
    };
    text(&gather("Not known: made with --report, without the running app.".into(), settings, None))
}

/// The report, with the player's user folder and name taken out of the
/// gathered sections; player-typed description and contact are shown as typed.
pub fn text(facts: &Facts) -> String {
    text_with_redaction(facts, dirs::home_dir().as_deref(), user_name().as_deref())
}

fn text_with_redaction(facts: &Facts, home: Option<&Path>, user: Option<&str>) -> String {
    draft_with_redaction(facts, home, user).with(&facts.description, &facts.contact)
}

/// A report made and redacted, waiting for the player's notes: what the
/// dialog shows, and what Send sends, is always `with` of the same draft.
#[derive(Clone, Debug)]
pub struct Draft {
    head: String,
    body: String,
    pub version: &'static str,
    pub system: String,
}

impl Draft {
    pub fn with(&self, description: &str, contact: &str) -> String {
        compose(&self.head, description, contact, &self.body)
    }
}

pub fn draft(facts: &Facts) -> Draft {
    draft_with_redaction(facts, dirs::home_dir().as_deref(), user_name().as_deref())
}

fn draft_with_redaction(facts: &Facts, home: Option<&Path>, user: Option<&str>) -> Draft {
    let settings = match &facts.settings {
        Ok(settings) => serde_json::to_string_pretty(settings).unwrap_or_default(),
        Err(why) => format!("({why})"),
    };
    let log = if facts.log.is_empty() { "(the log is empty)".to_string() } else { facts.log.join("\n") };
    let head = format!(
        "Scorpio problem report\n\
         Made {made}.\n",
        made = facts.made,
    );
    let system_sections = format_system_facts(&facts.system_facts);
    let cost_section = format_cost(facts.cost.as_ref());
    let body = format!(
        "Version: {version}\n\
         System: {system}\n\
         \n\
         Now:\n\
         {state}\n\
         \n\
         {system_sections}\
         \n\
         {cost_section}\
         \n\
         Settings ({paths}):\n\
         {settings}\n\
         \n\
         The last {count} lines of the log:\n\
         {log}\n",
        version = facts.version,
        system = facts.system,
        state = facts.state,
        paths = "your user folder is written as ~ and your user name as <user>",
        count = facts.log.len(),
    );
    Draft { head, body: redact(&body, home, user), version: facts.version, system: redact(&facts.system, home, user) }
}

/// Combines the report header, optional player-typed fields (unredacted,
/// clipped to their limits), and the redacted body.
pub fn compose(head: &str, description: &str, contact: &str, body: &str) -> String {
    let mut out = String::from(head);
    let description = clip(description.trim(), DESCRIPTION_LIMIT);
    let contact = clip(contact.trim(), CONTACT_LIMIT);
    if !description.is_empty() {
        out.push_str("\nWhat happened:\n");
        out.push_str(description);
        out.push('\n');
    }
    if !contact.is_empty() {
        out.push_str("\nHow to reach you:\n");
        out.push_str(contact);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(body);
    out
}

fn clip(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((idx, _)) => &text[..idx],
        None => text,
    }
}

fn format_system_facts(facts: &Result<SystemFacts, String>) -> String {
    match facts {
        Err(why) => format!(
            "Hardware, displays and presentation settings:\n\
             (could not be gathered: {why})\n"
        ),
        Ok(sf) => {
            let mut out = String::new();
            out.push_str("Hardware:\n");
            out.push_str(&format!("Processor: {}\n", sf.processor.as_deref().unwrap_or("unknown")));
            out.push_str(&format!("Memory: {}\n", sf.memory_mb.map_or_else(|| "unknown".into(), format_mb)));

            out.push_str("\nGraphics cards:\n");
            if sf.gpus.is_empty() {
                out.push_str("(none detected)\n");
            } else {
                for gpu in &sf.gpus {
                    out.push_str("- ");
                    out.push_str(&gpu.name);
                    if gpu.software {
                        out.push_str(" [software]");
                    }
                    if let Some(mb) = gpu.dedicated_memory_mb {
                        out.push_str(&format!(", {} dedicated", format_mb(mb)));
                    }
                    match (gpu.driver.as_deref(), gpu.driver_version.as_deref()) {
                        (Some(drv), Some(ver)) => out.push_str(&format!(", driver {drv} {ver}")),
                        (Some(drv), None) => out.push_str(&format!(", driver {drv}")),
                        (None, Some(ver)) => out.push_str(&format!(", driver {ver}")),
                        (None, None) => {}
                    }
                    out.push('\n');
                }
            }

            out.push_str("\nDisplays:\n");
            if sf.displays.is_empty() {
                out.push_str("(none detected)\n");
            } else {
                for display in &sf.displays {
                    out.push_str(&format!("- {}: {}x{}", display.name, display.width, display.height));
                    if let Some(hz) = display.refresh_hz {
                        out.push_str(&format!(" @ {hz} Hz"));
                    }
                    if let Some(hdr) = display.hdr {
                        out.push_str(if hdr { ", HDR on" } else { ", HDR off" });
                    }
                    if let Some(mpo) = display.overlay_planes {
                        out.push_str(if mpo { ", overlay planes yes" } else { ", overlay planes no" });
                    }
                    if display.primary {
                        out.push_str(" (primary)");
                    }
                    out.push('\n');
                }
            }

            out.push_str("\nPresentation settings:\n");
            let mut pres_lines = Vec::new();
            if let Some(v) = &sf.presentation.hags {
                pres_lines.push(format!("HAGS: {v}"));
            }
            if let Some(v) = &sf.presentation.windowed_optimisations {
                pres_lines.push(format!("Windowed-game optimisations: {v}"));
            }
            if let Some(v) = &sf.presentation.game_mode {
                pres_lines.push(format!("Game Mode: {v}"));
            }
            if let Some(v) = &sf.presentation.compat_layer {
                pres_lines.push(format!("__COMPAT_LAYER: {v}"));
            }
            if let Some(v) = &sf.presentation.session_type {
                pres_lines.push(format!("Session type: {v}"));
            }
            if let Some(v) = &sf.presentation.compositor {
                pres_lines.push(format!("Compositor: {v}"));
            }
            if pres_lines.is_empty() {
                out.push_str("(not available)\n");
            } else {
                for line in pres_lines {
                    out.push_str(&line);
                    out.push('\n');
                }
            }

            if !sf.notes.is_empty() {
                out.push_str("Notes: ");
                out.push_str(&sf.notes.join("; "));
                out.push('\n');
            }
            out
        }
    }
}

fn format_cost(cost: Option<&CostFacts>) -> String {
    let Some(cost) = cost else {
        return "Scorpio's cost:\n(not known: made without the running app)\n".into();
    };
    let cpu = cost.process.cpu_ms.map_or_else(|| "unknown".into(), format_cpu_ms);
    let mem = cost
        .process
        .private_memory_bytes
        .map_or_else(|| "unknown".into(), |bytes| format!("{:.1} MB", bytes as f64 / 1_048_576.0));
    let c = cost.counters;
    format!(
        "Scorpio's cost:\n\
         Uptime: {} (game in front: {})\n\
         CPU time: {cpu}; private memory: {mem}\n\
         Wake-ups: {} raw input (peak {}/s), {} foreground, {} window location, {} timers\n",
        format_duration(c.uptime),
        format_duration(c.game_in_front),
        c.raw_input,
        c.raw_input_peak_per_sec,
        c.foreground,
        c.window_location,
        c.timers,
    )
}

fn format_mb(mb: u64) -> String {
    if mb >= 1024 {
        format!("{:.1} GB ({mb} MB)", mb as f64 / 1024.0)
    } else {
        format!("{mb} MB")
    }
}

fn format_cpu_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.2} s", ms as f64 / 1000.0)
    }
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// Parses `DirectXUserGlobalSettings` (semicolon-separated `Key=Value` pairs)
/// for `SwapEffectUpgradeEnable`, which controls Windows' windowed-game optimisations.
#[allow(dead_code)]
pub(crate) fn parse_directx_user_global_settings(raw: &str) -> Option<String> {
    for part in raw.split(';') {
        let Some((key, val)) = part.trim().split_once('=') else { continue };
        if key.trim().eq_ignore_ascii_case("SwapEffectUpgradeEnable") {
            return Some(match val.trim() {
                "1" => "on (SwapEffectUpgradeEnable=1)".into(),
                "0" => "off (SwapEffectUpgradeEnable=0)".into(),
                other => format!("unknown (SwapEffectUpgradeEnable={other})"),
            });
        }
    }
    None
}

#[allow(dead_code)]
pub(crate) fn parse_cpuinfo(content: &str) -> Option<String> {
    for key in ["model name", "Hardware", "Processor"] {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim().eq_ignore_ascii_case(key) {
                    let v = v.trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
pub(crate) fn parse_meminfo_mb(content: &str) -> Option<u64> {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            return Some((kb + 512) / 1024);
        }
    }
    None
}

/// Parses utime + stime (fields 14 and 15) from `/proc/self/stat`.
#[allow(dead_code)]
pub(crate) fn parse_proc_stat_ticks(content: &str) -> Option<u64> {
    let after_comm = content.rsplit_once(')')?.1;
    let mut fields = after_comm.split_whitespace();
    // Field 3 (state) is index 0 -> field 14 (utime) is index 11, field 15 (stime) is index 12.
    let utime = fields.nth(11)?.parse::<u64>().ok()?;
    let stime = fields.next()?.parse::<u64>().ok()?;
    Some(utime.saturating_add(stime))
}

/// Parses `Private_Clean + Private_Dirty` (in bytes) from `/proc/self/smaps_rollup`.
#[allow(dead_code)]
pub(crate) fn parse_smaps_rollup_private_bytes(content: &str) -> Option<u64> {
    let mut found = false;
    let mut total_kb = 0u64;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Private_Clean:").or_else(|| line.strip_prefix("Private_Dirty:")) {
            if let Some(kb) = rest.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()) {
                total_kb = total_kb.saturating_add(kb);
                found = true;
            }
        }
    }
    found.then_some(total_kb.saturating_mul(1024))
}

/// Fallback memory reader for `/proc/self/status` (`RssAnon`, else `VmRSS`).
#[allow(dead_code)]
pub(crate) fn parse_status_rss_bytes(content: &str) -> Option<u64> {
    for key in ["RssAnon:", "VmRSS:"] {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix(key) {
                if let Some(kb) = rest.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()) {
                    return Some(kb.saturating_mul(1024));
                }
            }
        }
    }
    None
}

fn user_name() -> Option<String> {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).ok()
}

/// Replaces the home folder with `~` (either slash style, any case on
/// Windows) and the user name, where it is long enough and stands as a word
/// of its own (so a user called `lin` leaves `Linux` alone), with `<user>`.
pub fn redact(text: &str, home: Option<&Path>, user: Option<&str>) -> String {
    let mut out = text.to_string();
    if let Some(home) = home.map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 1) {
        for form in [home.clone(), home.replace('\\', "/"), home.replace('\\', "\\\\")] {
            out = replace_ignoring_case(&out, &form, "~", false);
        }
    }
    if let Some(user) = user.filter(|u| u.chars().count() >= 3) {
        out = replace_ignoring_case(&out, user, "<user>", true);
    }
    out
}

/// ASCII case only: it never changes a string's length, so positions found
/// in the lower-cased copy hold in the original whatever else it contains.
fn replace_ignoring_case(text: &str, from: &str, to: &str, whole_word: bool) -> String {
    let (lower, needle) = (text.to_ascii_lowercase(), from.to_ascii_lowercase());
    if needle.is_empty() {
        return text.to_string();
    }
    // Letters only: a name followed by digits (lena99, DESKTOP-LENA2) is still taken out.
    let word = |c: Option<char>| c.is_some_and(char::is_alphabetic);
    let mut out = String::with_capacity(text.len());
    let mut rest = 0;
    for (at, _) in lower.match_indices(&needle) {
        let end = at + needle.len();
        let alone = !whole_word || (!word(text[..at].chars().next_back()) && !word(text[end..].chars().next()));
        if at >= rest && alone {
            out.push_str(&text[rest..at]);
            out.push_str(to);
            rest = end;
        }
    }
    out.push_str(&text[rest..]);
    out
}

/// What Send posts: the report exactly as the dialog shows it, and the
/// fields the service lists reports by.
/// What Send posts, and the text in it: the report exactly as the dialog
/// shows it, and the fields the service lists reports by.
pub fn payload(draft: &Draft, description: &str, contact: &str) -> Result<(Vec<u8>, String), String> {
    let text = draft.with(description, contact);
    // The same trimming and clipping `compose` applied to the text.
    let description = clip(description.trim(), DESCRIPTION_LIMIT);
    if description.is_empty() {
        return Err(crate::i18n::tr("Write what happened first.", &[]));
    }
    let body = serde_json::json!({
        "version": draft.version,
        "system": draft.system,
        "description": description,
        "contact": clip(contact.trim(), CONTACT_LIMIT),
        "text": text,
    })
    .to_string()
    .into_bytes();
    if body.len() > SEND_LIMIT {
        return Err(too_long());
    }
    Ok((body, text))
}

fn too_long() -> String {
    crate::i18n::tr("The report is too long to send. Save it and attach it on GitHub.", &[])
}

fn unreachable() -> String {
    crate::i18n::tr("The report service could not be reached. Save the report and attach it on GitHub.", &[])
}

/// A report number as the service makes them: `SCR-` and six letters or digits.
fn is_report_id(id: &str) -> bool {
    id.len() == 10 && id.starts_with("SCR-") && id[4..].bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Sends a report the player read and asked to send; the report's number.
/// The bundled root certificates first; the system's only when the bundled
/// ones could not make the connection (an antivirus or proxy that re-signs
/// HTTPS), so a report that reached the service is never posted twice.
pub async fn send(payload: Vec<u8>) -> Result<String, String> {
    send_to(REPORTS, payload).await
}

async fn send_to(url: &str, payload: Vec<u8>) -> Result<String, String> {
    // reqwest is built without a TLS provider; the updater installs ring the
    // same way, on its first check, which may not have happened yet.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let client = |bundled: bool| {
        let builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(20));
        // `build`, never `Client::new`: the system's roots can fail to load, and a panic aborts Scorpio.
        if bundled { builder.tls_certs_only(crate::update::bundled_roots()) } else { builder }.build().map_err(|error| {
            log::warn!("report client: {error}");
            unreachable()
        })
    };
    let post = |client: reqwest::Client| client.post(url).header("Content-Type", "application/json").body(payload.clone()).send();
    let response = match post(client(true)?).await {
        Ok(response) => response,
        Err(error) if error.is_connect() => post(client(false)?).await.map_err(|error| {
            log::warn!("report not sent: {error}");
            unreachable()
        })?,
        Err(error) => {
            log::warn!("report not sent: {error}");
            return Err(unreachable());
        }
    };
    let status = response.status().as_u16();
    // The answer is a few bytes; read at most 4 KB of it, with or without a length.
    let mut response = response;
    let mut bytes = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > 4096 {
            break;
        }
    }
    let answer: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    match (status, answer.get("id").and_then(|id| id.as_str())) {
        (201, Some(id)) if is_report_id(id) => {
            log::info!("problem report sent as {id}");
            Ok(id.to_string())
        }
        (413, _) => Err(too_long()),
        (429, _) => Err(crate::i18n::tr("Too many reports from this network. Try again in an hour.", &[])),
        _ => {
            log::warn!("report service answered {status}");
            Err(crate::i18n::tr("The report service did not take the report. Save it and attach it on GitHub.", &[]))
        }
    }
}

/// The GitHub page for a new issue, with a short body: an address has a
/// length limit, so the report itself is attached as a file.
pub fn issue_url(version: &str, system: &str, file: &str) -> String {
    let title = "Problem: ";
    let body = format!(
        "What happened:\n\n\nWhat you expected:\n\n\nScorpio {version} on {system}.\n\n\
         Attach the report you saved ({file}): drag the file into this box. Read it first; it holds your settings and the end of Scorpio's log."
    );
    format!("{ISSUES}?title={}&body={}", encode(title), encode(&body))
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Where a saved report goes: the Downloads folder, else the home folder.
pub fn save(text: &str) -> std::io::Result<PathBuf> {
    let folder = dirs::download_dir().or_else(dirs::home_dir).ok_or_else(|| std::io::Error::other("no Downloads or home folder"))?;
    let stamp: String = logging::now().chars().filter(char::is_ascii_digit).collect();
    let path = folder.join(format!("scorpio-report-{stamp}.txt"));
    std::fs::write(&path, text)?;
    Ok(path)
}

/// The system in a few words: name, version, and on Linux the session.
pub fn system() -> String {
    #[cfg(target_os = "linux")]
    {
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let name = release
            .lines()
            .find_map(|line| line.strip_prefix("PRETTY_NAME="))
            .map(|name| name.trim_matches('"').to_string())
            .unwrap_or_else(|| "Linux".into());
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
        format!("{name}, kernel {}, {} session, {desktop} desktop, {}", kernel.trim(), session, std::env::consts::ARCH)
    }
    #[cfg(windows)]
    {
        crate::platform::windows_version()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_facts() -> Facts {
        let mut settings = Settings::default();
        settings.games.push(crate::store::Game { name: "cs2".into(), ..Default::default() });
        Facts {
            version: "9.9.9",
            system: "Testix 1".into(),
            made: "2026-11-21T00:00:00Z".into(),
            state: "Shown over cs2".into(),
            system_facts: Ok(SystemFacts::default()),
            cost: None,
            description: String::new(),
            contact: String::new(),
            settings: Ok(settings),
            log: vec!["a".into(), "b".into()],
        }
    }

    #[test]
    fn the_user_folder_and_name_are_taken_out() {
        let text = r#"path C:\Users\Lena\Games\cs2.exe, also c:\users\lena\x and "C:\\Users\\Lena\\y" and /home/lena/z, Lena"#;
        let windows = redact(text, Some(Path::new(r"C:\Users\Lena")), Some("Lena"));
        assert_eq!(windows, r#"path ~\Games\cs2.exe, also ~\x and "~\\y" and /home/<user>/z, <user>"#);
        let linux = redact("/home/lena/.steam and lena", Some(Path::new("/home/lena")), Some("lena"));
        assert_eq!(linux, "~/.steam and <user>");
        assert_eq!(redact("Bob is a bob", None, Some("Bo")), "Bob is a bob", "short names are left alone");
        assert_eq!(redact("Linux, lin and LIN-2", None, Some("lin")), "Linux, <user> and <user>-2", "only the name as a word of its own");
        assert_eq!(redact("DESKTOP-LENA2 and lena99@", None, Some("lena")), "DESKTOP-<user>2 and <user>99@", "digits next to the name do not hide it");
        let odd = redact("game İstanbul at c:\\users\\lena\\x", Some(Path::new(r"C:\Users\Lena")), None);
        assert_eq!(odd, "game İstanbul at ~\\x", "a letter that changes length when lower-cased does not stop the match");
    }

    #[test]
    fn the_report_holds_the_facts_and_no_home_folder() {
        let facts = sample_facts();
        let text = text(&facts);
        assert!(text.starts_with("Scorpio problem report\n"));
        assert!(text.contains("Version: 9.9.9\nSystem: Testix 1\n\nNow:\nShown over cs2\n"));
        assert!(text.contains("\"name\": \"cs2\""));
        assert!(text.contains("The last 2 lines of the log:\na\nb\n"));
        if let Some(home) = dirs::home_dir() {
            assert!(!text.contains(&*home.to_string_lossy()));
        }
    }

    #[test]
    fn the_issue_address_is_short_and_encoded() {
        let url = issue_url("0.1.1", "Windows 11 (build 26100)", "scorpio-report-20261121.txt");
        assert!(url.starts_with("https://github.com/Deikus-LXXVII/Scorpio-Crosshair/issues/new?title=Problem%3A%20&body=What%20happened%3A"));
        assert!(url.contains("scorpio-report-20261121.txt"));
        assert!(url.len() < 2000, "{}", url.len());
        assert!(!url.contains(' ') && !url.contains('\n'));
    }

    #[test]
    fn new_sections_and_player_notes_format_and_keep_contact_unredacted() {
        let mut facts = sample_facts();
        facts.description = "FPS dropped 1% in CS2.".into();
        facts.contact = "lena@example.com".into();
        facts.log = vec![r"opened C:\Users\Lena\Games\cs2.exe for Lena".into()];
        facts.system_facts = Ok(SystemFacts {
            processor: Some("AMD Ryzen 7 7800X3D 8-Core Processor".into()),
            memory_mb: Some(32768),
            gpus: vec![
                GpuFact {
                    name: "NVIDIA GeForce RTX 4080".into(),
                    dedicated_memory_mb: Some(16384),
                    driver: None,
                    driver_version: Some("32.0.15.6614".into()),
                    software: false,
                },
                GpuFact {
                    name: "Microsoft Basic Render Driver".into(),
                    dedicated_memory_mb: Some(0),
                    driver: None,
                    driver_version: None,
                    software: true,
                },
            ],
            displays: vec![DisplayFact {
                name: r"\\.\DISPLAY1".into(),
                width: 2560,
                height: 1440,
                refresh_hz: Some(240),
                hdr: Some(false),
                overlay_planes: Some(true),
                primary: true,
            }],
            presentation: PresentationFacts {
                hags: Some("on (HwSchMode=2)".into()),
                windowed_optimisations: Some("on (SwapEffectUpgradeEnable=1)".into()),
                game_mode: Some("on".into()),
                compat_layer: Some("not set".into()),
                ..Default::default()
            },
            notes: Vec::new(),
        });
        facts.cost = Some(CostFacts {
            process: ProcessCost { cpu_ms: Some(1250), private_memory_bytes: Some(7_340_032) },
            counters: counters::Snapshot {
                uptime: Duration::from_secs(3665),
                game_in_front: Duration::from_secs(125),
                raw_input: 14200,
                raw_input_peak_per_sec: 1002,
                foreground: 18,
                window_location: 4,
                timers: 3,
            },
        });

        let report = text_with_redaction(&facts, Some(Path::new(r"C:\Users\Lena")), Some("Lena"));
        assert!(report.contains("What happened:\nFPS dropped 1% in CS2.\n"));
        assert!(report.contains("How to reach you:\nlena@example.com\n"), "contact is kept as typed");
        assert!(report.contains(r"opened ~\Games\cs2.exe for <user>"), "rest of report is still redacted");
        assert!(report.contains("Hardware:\nProcessor: AMD Ryzen 7 7800X3D 8-Core Processor\nMemory: 32.0 GB (32768 MB)\n"));
        assert!(report.contains("- NVIDIA GeForce RTX 4080, 16.0 GB (16384 MB) dedicated, driver 32.0.15.6614\n"));
        assert!(report.contains("- Microsoft Basic Render Driver [software], 0 MB dedicated\n"));
        assert!(report.contains(r"- \\.\DISPLAY1: 2560x1440 @ 240 Hz, HDR off, overlay planes yes (primary)"));
        assert!(report.contains("Presentation settings:\nHAGS: on (HwSchMode=2)\nWindowed-game optimisations: on (SwapEffectUpgradeEnable=1)\nGame Mode: on\n__COMPAT_LAYER: not set\n"));
        assert!(report.contains("Scorpio's cost:\nUptime: 1h 1m 5s (game in front: 2m 5s)\nCPU time: 1.25 s; private memory: 7.0 MB\nWake-ups: 14200 raw input (peak 1002/s), 18 foreground, 4 window location, 3 timers\n"));
    }

    #[test]
    fn failed_child_explains_why_and_keeps_the_rest_of_the_report() {
        let mut facts = sample_facts();
        facts.system_facts = Err("timed out after 5 s".into());
        facts.cost = Some(CostFacts {
            process: ProcessCost { cpu_ms: Some(42), private_memory_bytes: Some(7_000_000) },
            counters: counters::Snapshot { uptime: Duration::from_secs(12), ..Default::default() },
        });
        let report = text_with_redaction(&facts, None, None);
        assert!(report.contains("Hardware, displays and presentation settings:\n(could not be gathered: timed out after 5 s)\n"));
        assert!(report.contains("CPU time: 42 ms"));
        assert!(report.contains("\"name\": \"cs2\""));
        assert!(report.contains("The last 2 lines of the log:\na\nb\n"));
    }

    #[test]
    fn system_facts_json_parses_valid_and_rejects_garbled_or_empty() {
        let original = SystemFacts {
            processor: Some("Intel Core i7".into()),
            memory_mb: Some(16384),
            gpus: vec![GpuFact {
                name: "card0 (0x1002:0x744c)".into(),
                dedicated_memory_mb: Some(24576),
                driver: Some("amdgpu".into()),
                driver_version: None,
                software: false,
            }],
            displays: vec![DisplayFact {
                name: "DP-1".into(),
                width: 2560,
                height: 1440,
                refresh_hz: Some(144),
                hdr: None,
                overlay_planes: None,
                primary: true,
            }],
            presentation: PresentationFacts {
                session_type: Some("x11".into()),
                compositor: Some("picom".into()),
                ..Default::default()
            },
            notes: Vec::new(),
        };
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(parse_system_facts(&json).unwrap(), original);
        assert_eq!(parse_system_facts("{}").unwrap(), SystemFacts::default());
        assert!(parse_system_facts("").unwrap_err().contains("no output"));
        assert!(parse_system_facts("not json at all").unwrap_err().contains("invalid JSON"));
        assert!(parse_system_facts("{\"gpus\": \"wrong\"}").unwrap_err().contains("invalid JSON"));
    }

    #[cfg(unix)]
    #[test]
    fn child_runner_handles_success_failure_garbled_and_timeout() {
        let mut ok_cmd = Command::new("sh");
        ok_cmd.args(["-c", r#"printf '{"processor":"Test CPU","memory_mb":8192}\n'"#]);
        let stdout = run_child_with_timeout(ok_cmd, Duration::from_secs(2)).unwrap();
        let parsed = parse_system_facts(&stdout).unwrap();
        assert_eq!(parsed.processor.as_deref(), Some("Test CPU"));
        assert_eq!(parsed.memory_mb, Some(8192));

        let mut fail_cmd = Command::new("sh");
        fail_cmd.args(["-c", "echo boom >&2; exit 2"]);
        let err = run_child_with_timeout(fail_cmd, Duration::from_secs(2)).unwrap_err();
        assert!(err.contains("boom"), "{err}");

        let mut garbled_cmd = Command::new("sh");
        garbled_cmd.args(["-c", "printf '<<<garbled>>>'"]);
        let out = run_child_with_timeout(garbled_cmd, Duration::from_secs(2)).unwrap();
        assert!(parse_system_facts(&out).is_err());

        let mut slow_cmd = Command::new("sh");
        slow_cmd.args(["-c", "sleep 5"]);
        let err = run_child_with_timeout(slow_cmd, Duration::from_millis(120)).unwrap_err();
        assert!(err.contains("timed out"), "{err}");
    }

    #[test]
    fn parsers_read_directx_and_proc_files() {
        assert_eq!(
            parse_directx_user_global_settings("VRROptimizeEnable=0;SwapEffectUpgradeEnable=1;").as_deref(),
            Some("on (SwapEffectUpgradeEnable=1)")
        );
        assert_eq!(
            parse_directx_user_global_settings("SwapEffectUpgradeEnable=0").as_deref(),
            Some("off (SwapEffectUpgradeEnable=0)")
        );
        assert_eq!(parse_directx_user_global_settings("VRROptimizeEnable=1"), None);

        assert_eq!(
            parse_cpuinfo("processor\t: 0\nmodel name\t: AMD Ryzen 9 7950X\n").as_deref(),
            Some("AMD Ryzen 9 7950X")
        );
        assert_eq!(parse_meminfo_mb("MemTotal:       32864256 kB\nMemFree: 1000 kB\n"), Some(32094));
        assert_eq!(parse_proc_stat_ticks("1234 (scorpio (1)) S 1 2 3 4 5 6 7 8 9 10 150 25 0 0 20 0"), Some(175));
        assert_eq!(
            parse_smaps_rollup_private_bytes("Rss: 12000 kB\nPrivate_Clean: 1000 kB\nPrivate_Dirty: 6000 kB\n"),
            Some(7_000 * 1024)
        );
        assert_eq!(parse_status_rss_bytes("Name:\tscorpio\nVmRSS:\t8000 kB\nRssAnon:\t5000 kB\n"), Some(5000 * 1024));
    }

    #[test]
    fn compose_clips_player_fields_to_their_limits() {
        let long_desc = "a".repeat(DESCRIPTION_LIMIT + 50);
        let long_contact = "b".repeat(CONTACT_LIMIT + 20);
        let out = compose("Head\n", &long_desc, &long_contact, "Body\n");
        let desc_line = out.split("What happened:\n").nth(1).unwrap().lines().next().unwrap();
        let contact_line = out.split("How to reach you:\n").nth(1).unwrap().lines().next().unwrap();
        assert_eq!(desc_line.chars().count(), DESCRIPTION_LIMIT);
        assert_eq!(contact_line.chars().count(), CONTACT_LIMIT);
    }

    fn sample_draft() -> Draft {
        Draft { head: "Scorpio problem report\nMade now.\n".into(), body: "Version: 9.9.9\n".into(), version: "9.9.9", system: "Testix 1".into() }
    }

    #[test]
    fn send_posts_exactly_the_shown_text() {
        let draft = sample_draft();
        assert!(payload(&draft, "   ", "x").is_err(), "nothing is sent without a description");
        let (bytes, text) = payload(&draft, " FPS drops ", " me@example.com ").unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(text, draft.with(" FPS drops ", " me@example.com "), "the text is the one the dialog shows");
        assert_eq!(body["text"], text);
        assert_eq!(body["description"], "FPS drops");
        assert_eq!(body["contact"], "me@example.com");
        assert_eq!(body["version"], "9.9.9");
        assert_eq!(body["system"], "Testix 1");

        let edge = format!("{}   x", "a".repeat(DESCRIPTION_LIMIT - 3));
        let (bytes, text) = payload(&draft, &edge, "").unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(text, draft.with(&edge, ""), "spaces at the 4000-character edge do not make shown and sent differ");
        assert!(text.contains(body["description"].as_str().unwrap()), "the listed description is the one in the text");

        let quotes = Draft { body: "\\\"".repeat(SEND_LIMIT / 4), ..sample_draft() };
        assert!(quotes.with("a", "").len() < SEND_LIMIT, "the text alone fits");
        assert!(payload(&quotes, "a", "").is_err(), "but escaped as JSON it does not, so it is not sent");
    }

    #[test]
    fn the_listed_system_is_redacted_like_the_text() {
        let mut facts = Facts {
            version: "9.9.9",
            system: "Linux custom-lena-kernel x86_64".into(),
            made: "now".into(),
            state: String::new(),
            system_facts: Err("none".into()),
            cost: None,
            description: String::new(),
            contact: String::new(),
            settings: Err("none".into()),
            log: Vec::new(),
        };
        facts.description.clear();
        let draft = draft_with_redaction(&facts, None, Some("lena"));
        assert_eq!(draft.system, "Linux custom-<user>-kernel x86_64");
        assert!(!draft.with("a", "").contains("lena"));
    }

    #[test]
    fn only_a_report_number_counts_as_one() {
        assert!(is_report_id("SCR-F3MZNA"));
        for bad in ["SCR-F3MZN", "SCR-F3MZNAA", "SCR-f3mzna", "SCR-F3\nZNA", "XYZ-F3MZNA"] {
            assert!(!is_report_id(bad), "{bad:?}");
        }
    }

    /// One HTTP exchange on a local port: the request it got, and `answer` sent back.
    fn one_answer(answer: &'static str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/reports", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
                    if got.len() >= end + 4 + length || n == 0 {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let _ = stream.write_all(answer.as_bytes());
            String::from_utf8_lossy(&got).to_string()
        });
        (url, server)
    }

    #[test]
    fn send_reads_the_number_and_turns_refusals_into_advice() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let (body, _) = payload(&sample_draft(), "FPS drops", "").unwrap();

        let (url, server) = one_answer("HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{\"id\": \"SCR-F3MZNA\"}");
        assert_eq!(runtime.block_on(send_to(&url, body.clone())), Ok("SCR-F3MZNA".into()));
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /v1/reports "), "{request}");
        assert!(request.to_ascii_lowercase().contains("content-type: application/json"));
        assert!(request.contains("\"description\":\"FPS drops\""));

        let (url, server) = one_answer("HTTP/1.1 201 Created\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n14\r\n{\"id\": \"SCR-F3MZNA\"}\r\n0\r\n\r\n");
        assert_eq!(runtime.block_on(send_to(&url, body.clone())), Ok("SCR-F3MZNA".into()), "an answer without a length is read too");
        server.join().unwrap();

        let (url, server) = one_answer("HTTP/1.1 429 Too Many Requests\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
        assert!(runtime.block_on(send_to(&url, body.clone())).unwrap_err().contains("Try again in an hour"));
        server.join().unwrap();

        let (url, server) = one_answer("HTTP/1.1 201 Created\r\nContent-Length: 24\r\nConnection: close\r\n\r\n{\"id\": \"<script>x</sc\"}");
        assert!(runtime.block_on(send_to(&url, body.clone())).is_err(), "an answer that is not a report number is not shown");
        server.join().unwrap();

        let (url, server) = one_answer("HTTP/1.1 413 Payload Too Large\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
        assert!(runtime.block_on(send_to(&url, body.clone())).unwrap_err().contains("too long"));
        server.join().unwrap();

        let (url, server) = one_answer("HTTP/1.1 308 Permanent Redirect\r\nLocation: http://127.0.0.1:9/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        assert!(runtime.block_on(send_to(&url, body.clone())).is_err(), "a redirect is not followed");
        server.join().unwrap();

        // The service took the report and the answer was lost: one POST, no second try.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/reports", listener.local_addr().unwrap());
        let counter = std::thread::spawn(move || {
            listener.set_nonblocking(false).unwrap();
            let mut posts = 0;
            let (stream, _) = listener.accept().unwrap();
            posts += 1;
            drop(stream);
            listener.set_nonblocking(true).unwrap();
            std::thread::sleep(Duration::from_millis(500));
            while listener.accept().is_ok() {
                posts += 1;
            }
            posts
        });
        assert!(runtime.block_on(send_to(&url, body)).is_err());
        assert_eq!(counter.join().unwrap(), 1, "a lost answer is not followed by a second POST");
    }

}
