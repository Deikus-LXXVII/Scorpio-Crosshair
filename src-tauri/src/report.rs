//! "Report a problem": a plain text the player reads in full before
//! anything leaves the computer, and then copies, saves, or attaches to a
//! GitHub issue themselves. Nothing is ever sent by Scorpio.
//!
//! The same text comes from `scorpio --report`, for the player's own tools.

use crate::logging;
use crate::store::Settings;
use std::path::{Path, PathBuf};

pub const ISSUES: &str = "https://github.com/Deikus-LXXVII/Scorpio-Crosshair/issues/new";
/// Log lines in a report.
pub const LOG_LINES: usize = 200;
/// A report the settings window may ask to save, at most.
pub const TEXT_LIMIT: usize = 2 * 1024 * 1024;

/// What a report says, gathered at the boundary; `text` only formats it.
pub struct Facts {
    pub version: &'static str,
    pub system: String,
    pub made: String,
    /// What the running app is doing, in a few lines; or why it is not known.
    pub state: String,
    /// The settings, or why they could not be read.
    pub settings: Result<Settings, String>,
    pub log: Vec<String>,
}

pub fn gather(state: String, settings: Result<Settings, String>) -> Facts {
    let log = logging::paths::log_dir().map(|dir| logging::tail(&dir, LOG_LINES)).unwrap_or_default();
    Facts { version: env!("CARGO_PKG_VERSION"), system: system(), made: logging::now(), state, settings, log }
}

/// The report `scorpio --report` prints, made without starting the app.
pub fn from_files() -> String {
    let settings = match logging::paths::config_dir().map(|dir| crate::store::peek(&dir.join("settings.json"))) {
        Some(Ok(Some(settings))) => Ok(settings),
        Some(Ok(None)) => Err("no settings file yet: Scorpio uses its defaults".into()),
        Some(Err(error)) => Err(format!("the settings file is {error}")),
        None => Err("this system has no config folder".into()),
    };
    text(&gather("Not known: made with --report, without the running app.".into(), settings))
}

/// The report, with the player's user folder and name taken out.
pub fn text(facts: &Facts) -> String {
    let settings = match &facts.settings {
        Ok(settings) => serde_json::to_string_pretty(settings).unwrap_or_default(),
        Err(why) => format!("({why})"),
    };
    let log = if facts.log.is_empty() { "(the log is empty)".to_string() } else { facts.log.join("\n") };
    let text = format!(
        "Scorpio problem report\n\
         Made {made}. Scorpio did not send it anywhere.\n\
         \n\
         Version: {version}\n\
         System: {system}\n\
         \n\
         Now:\n\
         {state}\n\
         \n\
         Settings ({paths}):\n\
         {settings}\n\
         \n\
         The last {count} lines of the log:\n\
         {log}\n",
        made = facts.made,
        version = facts.version,
        system = facts.system,
        state = facts.state,
        paths = "your user folder is written as ~ and your user name as <user>",
        count = facts.log.len(),
    );
    redact(&text, dirs::home_dir().as_deref(), user_name().as_deref())
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
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
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

    #[test]
    fn the_user_folder_and_name_are_taken_out() {
        let text = r#"path C:\Users\Lena\Games\cs2.exe, also c:\users\lena\x and "C:\\Users\\Lena\\y" and /home/lena/z, Lena"#;
        let windows = redact(text, Some(Path::new(r"C:\Users\Lena")), Some("Lena"));
        assert_eq!(windows, r#"path ~\Games\cs2.exe, also ~\x and "~\\y" and /home/<user>/z, <user>"#);
        let linux = redact("/home/lena/.steam and lena", Some(Path::new("/home/lena")), Some("lena"));
        assert_eq!(linux, "~/.steam and <user>");
        assert_eq!(redact("Bob is a bob", None, Some("Bo")), "Bob is a bob", "short names are left alone");
        assert_eq!(redact("Linux, lin and LIN-2", None, Some("lin")), "Linux, <user> and <user>-2", "only the name as a word of its own");
        let odd = redact("game İstanbul at c:\\users\\lena\\x", Some(Path::new(r"C:\Users\Lena")), None);
        assert_eq!(odd, "game İstanbul at ~\\x", "a letter that changes length when lower-cased does not stop the match");
    }

    #[test]
    fn the_report_holds_the_facts_and_no_home_folder() {
        let mut settings = Settings::default();
        settings.games.push(crate::store::Game { name: "cs2".into(), ..Default::default() });
        let facts = Facts {
            version: "9.9.9",
            system: "Testix 1".into(),
            made: "2026-11-21T00:00:00Z".into(),
            state: "Shown over cs2".into(),
            settings: Ok(settings),
            log: vec!["a".into(), "b".into()],
        };
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
}
