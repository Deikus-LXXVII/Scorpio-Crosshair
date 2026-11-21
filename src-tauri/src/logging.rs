//! The log file: plain lines, one event each, in the system's log folder for
//! apps. It is capped: past 1 MB the file becomes `scorpio.old.log` (the
//! previous one is dropped) and a new one starts, so at most 2 MB are kept.
//! Nothing in it leaves the computer unless the player sends a report.
//!
//! A line: `2026-11-21T07:01:02Z INFO  scorpio_lib::app: message`.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const FILE: &str = "scorpio.log";
pub const OLD_FILE: &str = "scorpio.old.log";
const LIMIT: u64 = 1024 * 1024;

/// Where Scorpio keeps its files; the same folders Tauri resolves for the
/// identifier `com.lxxvii.scorpio`, so `--report` finds them without starting
/// the app.
pub mod paths {
    use std::path::PathBuf;

    pub const IDENTIFIER: &str = "com.lxxvii.scorpio";

    /// `%APPDATA%\com.lxxvii.scorpio` on Windows, `~/.config/com.lxxvii.scorpio` on Linux.
    pub fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join(IDENTIFIER))
    }

    /// `%LOCALAPPDATA%\com.lxxvii.scorpio\logs` on Windows,
    /// `~/.local/share/com.lxxvii.scorpio/logs` on Linux.
    pub fn log_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        return dirs::home_dir().map(|dir| dir.join("Library/Logs").join(IDENTIFIER));
        #[cfg(not(target_os = "macos"))]
        return dirs::data_local_dir().map(|dir| dir.join(IDENTIFIER).join("logs"));
    }
}

struct Sink {
    path: PathBuf,
    old: PathBuf,
    file: Option<File>,
    size: u64,
}

impl Sink {
    fn open(path: PathBuf) -> Sink {
        let old = path.with_file_name(OLD_FILE);
        let file = OpenOptions::new().create(true).append(true).open(&path).ok();
        let size = file.as_ref().and_then(|f| f.metadata().ok()).map_or(0, |m| m.len());
        Sink { path, old, file, size }
    }

    fn write(&mut self, line: &str) {
        if self.size + line.len() as u64 > LIMIT {
            self.file = None;
            let _ = fs::rename(&self.path, &self.old);
            self.file = OpenOptions::new().create(true).write(true).truncate(true).open(&self.path).ok();
            self.size = 0;
        }
        // Unbuffered: a line is on disk before a panic aborts the process.
        if let Some(file) = &mut self.file {
            if file.write_all(line.as_bytes()).is_ok() {
                self.size += line.len() as u64;
            }
        }
    }
}

struct Logger {
    sink: Mutex<Sink>,
}

/// Scorpio's own lines from debug up; the libraries' only when they warn.
fn wanted(metadata: &log::Metadata) -> bool {
    let own = metadata.target().starts_with("scorpio");
    metadata.level() <= if own { log::Level::Debug } else { log::Level::Warn }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        wanted(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !wanted(record.metadata()) {
            return;
        }
        let line = line(SystemTime::now(), record.level(), record.target(), &record.args().to_string());
        if let Ok(mut sink) = self.sink.lock() {
            sink.write(&line);
        }
    }

    fn flush(&self) {}
}

/// One line, whatever the message holds.
fn line(time: SystemTime, level: log::Level, target: &str, message: &str) -> String {
    let message = message.replace("\r\n", " | ").replace(['\n', '\r'], " | ");
    format!("{} {level:<5} {target}: {message}\n", timestamp(time))
}

/// Now, as the log writes it.
pub fn now() -> String {
    timestamp(SystemTime::now())
}

/// UTC, to the second: `2026-11-21T07:01:02Z`.
fn timestamp(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// Starts writing the log, and logs panics before the process ends. Without
/// a log folder Scorpio runs as before, silently.
pub fn start(folder: &Path) {
    if fs::create_dir_all(folder).is_err() {
        return;
    }
    let logger = Logger { sink: Mutex::new(Sink::open(folder.join(FILE))) };
    if log::set_logger(Box::leak(Box::new(logger))).is_ok() {
        log::set_max_level(log::LevelFilter::Debug);
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        previous(info);
    }));
}

/// The last `count` lines of the log, the old file's end first when the
/// current one is short.
pub fn tail(folder: &Path, count: usize) -> Vec<String> {
    let read = |name: &str| fs::read_to_string(folder.join(name)).unwrap_or_default();
    let (old, current) = (read(OLD_FILE), read(FILE));
    let lines: Vec<&str> = old.lines().chain(current.lines()).collect();
    lines[lines.len().saturating_sub(count)..].iter().map(|line| (*line).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn lines_are_plain_and_one_per_event() {
        let time = UNIX_EPOCH + Duration::from_secs(1_795_000_000);
        assert_eq!(line(time, log::Level::Info, "scorpio_lib", "two\nlines"), "2026-11-18T11:06:40Z INFO  scorpio_lib: two | lines\n");
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(UNIX_EPOCH + Duration::from_secs(951_782_400)), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn the_file_rotates_at_its_cap_and_tail_reads_across_both() {
        let dir = std::env::temp_dir().join(format!("scorpio-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut sink = Sink::open(dir.join(FILE));
        let row = format!("{}\n", "x".repeat(99));
        for i in 0..11_000 {
            sink.write(&if i == 10_999 { "last\n".to_string() } else { row.clone() });
        }
        let size = |name: &str| fs::metadata(dir.join(name)).unwrap().len();
        assert!(size(FILE) <= LIMIT && size(OLD_FILE) <= LIMIT, "capped");
        assert!(size(FILE) + size(OLD_FILE) > LIMIT, "the previous file is kept");
        assert_eq!(tail(&dir, 1), vec!["last".to_string()]);
        assert_eq!(tail(&dir, 3).len(), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn libraries_only_warn() {
        let meta = |target, level| log::Metadata::builder().target(target).level(level).build();
        assert!(wanted(&meta("scorpio_lib::app", log::Level::Debug)));
        assert!(!wanted(&meta("scorpio_lib::app", log::Level::Trace)));
        assert!(!wanted(&meta("tao::platform", log::Level::Info)));
        assert!(wanted(&meta("tao::platform", log::Level::Warn)));
    }
}
