//! The settings file: what the player chose, with defaults for everything and
//! validation on the way in, so the rest of the app trusts these types.

use crate::core::crosshair::Crosshair;
use crate::core::focus::Fingerprint;
use crate::platform::MouseButton;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const NAME_LIMIT: usize = 48;
pub const SAVED_LIMIT: usize = 100;
pub const GAMES_LIMIT: usize = 200;

/// One of the player's own crosshairs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Saved {
    pub name: String,
    pub crosshair: Crosshair,
}

impl Default for Saved {
    fn default() -> Self {
        Saved { name: "Crosshair".into(), crosshair: Crosshair::default() }
    }
}

/// A game the player added. Its own crosshair replaces the main one while it
/// is in front; without one the main crosshair shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Game {
    pub name: String,
    pub fingerprint: Fingerprint,
    pub crosshair: Option<Crosshair>,
}

/// Which windows count as a game without being added.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Detect {
    /// A fullscreen window (not the desktop or the shell).
    pub fullscreen: bool,
    /// A program inside a game folder that Steam, Epic, GOG and others installed.
    pub launchers: bool,
}

impl Default for Detect {
    fn default() -> Self {
        Detect { fullscreen: true, launchers: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AimMode {
    /// Hidden while the button is held.
    Hold,
    /// Each press hides or shows it.
    Toggle,
}

/// Hiding the crosshair while aiming down sights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Aim {
    pub hide: bool,
    pub button: MouseButton,
    pub mode: AimMode,
}

impl Default for Aim {
    fn default() -> Self {
        Aim { hide: false, button: MouseButton::Right, mode: AimMode::Hold }
    }
}

/// Global shortcuts as accelerators ("Ctrl+Shift+Alt+X"); empty means none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Keys {
    pub lock: String,
    pub center: String,
    pub hide: String,
    pub next: String,
    pub previous: String,
    pub add_game: String,
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            lock: "Ctrl+Shift+Alt+X".into(),
            center: "Ctrl+Shift+Alt+C".into(),
            hide: "Ctrl+Shift+Alt+H".into(),
            next: "Ctrl+Shift+Alt+Period".into(),
            previous: "Ctrl+Shift+Alt+Comma".into(),
            add_game: "Ctrl+Shift+Alt+G".into(),
        }
    }
}

impl Keys {
    pub fn all(&self) -> [(&'static str, &str); 6] {
        [
            ("lock", &self.lock),
            ("center", &self.center),
            ("hide", &self.hide),
            ("next", &self.next),
            ("previous", &self.previous),
            ("addGame", &self.add_game),
        ]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub crosshair: Crosshair,
    pub saved: Vec<Saved>,
    pub games: Vec<Game>,
    pub detect: Detect,
    pub aim: Aim,
    pub keys: Keys,
    /// Offset of the crosshair's centre from the display's centre, physical
    /// pixels. (0, 0) is exactly centred.
    pub offset: (i32, i32),
    pub start_with_system: bool,
    /// Windows only; off by default (see the plan's anti-cheat table).
    pub hide_from_capture: bool,
    /// Off by default: updates install by themselves, between games.
    pub update_manually: bool,
}

fn clean_name(name: &str, fallback: &str) -> String {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(NAME_LIMIT).collect();
    if name.is_empty() { fallback.to_string() } else { name }
}

impl Settings {
    /// Every value in range, names tidy, lists bounded. Idempotent.
    pub fn validated(mut self) -> Settings {
        self.crosshair = self.crosshair.normalized();
        self.saved.truncate(SAVED_LIMIT);
        for saved in &mut self.saved {
            saved.name = clean_name(&saved.name, "Crosshair");
            saved.crosshair = saved.crosshair.normalized();
        }
        self.games.retain(|game| game.fingerprint != Fingerprint::default());
        self.games.truncate(GAMES_LIMIT);
        for game in &mut self.games {
            let fallback = if game.fingerprint.name.is_empty() { "Game" } else { &game.fingerprint.name };
            game.name = clean_name(&game.name, fallback);
            game.crosshair = game.crosshair.as_ref().map(Crosshair::normalized);
        }
        const LIMIT: i32 = 4000;
        self.offset = (self.offset.0.clamp(-LIMIT, LIMIT), self.offset.1.clamp(-LIMIT, LIMIT));
        self
    }
}

/// Reads the settings; a missing file gives defaults, an unreadable one is
/// kept beside as `.broken` so nothing the player made is lost silently.
pub fn load(path: &Path) -> Settings {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
        Err(error) => {
            // Unreadable but present: keep a copy before defaults replace it.
            log::warn!("settings could not be read ({error}); starting from defaults");
            let _ = fs::copy(path, broken_name(path));
            return Settings::default();
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(settings) => settings.validated(),
        Err(error) => {
            log::warn!("settings unreadable ({error}); starting from defaults");
            let _ = fs::rename(path, broken_name(path));
            Settings::default()
        }
    }
}

/// `settings.broken-<seconds>.json`, so an earlier broken copy is never lost.
fn broken_name(path: &Path) -> std::path::PathBuf {
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    path.with_extension(format!("broken-{seconds}.json"))
}

/// Writes through a temporary file and a rename, so a crash mid-write never
/// leaves half a file.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(settings).expect("settings serialize"))?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("scorpio-store-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_gives_defaults() {
        assert_eq!(load(&folder().join("none.json")), Settings::default());
    }

    #[test]
    fn round_trip_keeps_everything() {
        let path = folder().join("settings.json");
        let mut settings = Settings::default();
        settings.saved.push(Saved { name: "Mine".into(), crosshair: Crosshair { gap: -2, ..Crosshair::default() } });
        settings.games.push(Game {
            name: "CS2".into(),
            fingerprint: Fingerprint { name: "cs2".into(), path: "c:\\games\\cs2.exe".into(), ..Fingerprint::default() },
            crosshair: Some(Crosshair { color: [255, 0, 0], ..Crosshair::default() }),
        });
        settings.aim = Aim { hide: true, button: MouseButton::Back, mode: AimMode::Toggle };
        settings.offset = (3, -1);
        save(&path, &settings).unwrap();
        assert_eq!(load(&path), settings.validated());
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let path = folder().join("settings.json");
        fs::write(&path, r#"{"aim":{"hide":true},"crosshair":{"gap":-3}}"#).unwrap();
        let settings = load(&path);
        assert!(settings.aim.hide);
        assert_eq!(settings.aim.button, MouseButton::Right);
        assert_eq!(settings.crosshair.gap, -3);
        assert_eq!(settings.keys, Keys::default());
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let dir = folder();
        let path = dir.join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        let kept = fs::read_dir(&dir).unwrap().filter_map(Result::ok).any(|e| e.file_name().to_string_lossy().starts_with("settings.broken-"));
        assert!(kept);
    }

    #[test]
    fn validation_cleans_and_is_idempotent() {
        let mut settings = Settings::default();
        settings.saved.push(Saved { name: "  \u{7}".into(), crosshair: Crosshair::default() });
        settings.saved.push(Saved { name: "x".repeat(200), crosshair: Crosshair::default() });
        settings.games.push(Game::default());
        settings.games.push(Game { name: String::new(), fingerprint: Fingerprint { name: "cs2".into(), ..Fingerprint::default() }, crosshair: None });
        settings.offset = (99_999, -99_999);
        let once = settings.validated();
        assert_eq!(once.saved[0].name, "Crosshair");
        assert_eq!(once.saved[1].name.chars().count(), NAME_LIMIT);
        assert_eq!(once.games.len(), 1);
        assert_eq!(once.games[0].name, "cs2");
        assert_eq!(once.offset, (4000, -4000));
        assert_eq!(once.clone().validated(), once);
    }
}
