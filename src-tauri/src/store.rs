//! The settings file: what the player chose, with defaults for everything and
//! validation on the way in, so the rest of the app trusts these types.

use crate::core::crosshair::Crosshair;
use crate::core::focus::Fingerprint;
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

/// The player's own picture as the crosshair. The settings window turns the
/// chosen file into a PNG at the exact size wanted; both files live in the
/// config folder's `images` folder under names Scorpio made, which are all
/// the settings keep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Image {
    /// Shown instead of the CS2 crosshair.
    pub on: bool,
    /// The chosen file's own name, for the settings window only.
    pub name: String,
    /// The original file, kept so a new size is made from it, not from a copy.
    pub source: String,
    /// The PNG the overlay shows.
    pub rendered: String,
    /// The longer side, in physical pixels.
    pub size: u16,
    pub opacity: u8,
}

impl Default for Image {
    fn default() -> Self {
        Image { on: false, name: String::new(), source: String::new(), rendered: String::new(), size: 64, opacity: 255 }
    }
}

impl Image {
    pub fn validated(mut self) -> Image {
        self.name = self.name.trim().chars().filter(|c| !c.is_control() && !matches!(c, '/' | '\\')).take(120).collect();
        if !crate::picture::valid_file_name(&self.source) {
            self.source.clear();
        }
        if !crate::picture::valid_file_name(&self.rendered) || !self.rendered.ends_with(".png") {
            self.rendered.clear();
        }
        self.size = self.size.clamp(crate::picture::SIZE_MIN as u16, crate::picture::SIZE_LIMIT as u16);
        self.on &= !self.rendered.is_empty();
        self
    }

    /// The image files these settings use.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        [self.source.as_str(), self.rendered.as_str()].into_iter().filter(|name| !name.is_empty())
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
    /// Used with the game's own crosshair only.
    pub image: Image,
}

/// Which windows count as a game without being added.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Detect {
    /// A fullscreen window (not the desktop or the shell).
    pub fullscreen: bool,
    /// A program inside a game folder that Steam, Epic, GOG and others installed.
    pub launchers: bool,
    /// Every window, the desktop included: the crosshair stays on whatever is
    /// in front. Hiding while aiming and the hide shortcut still apply.
    pub always: bool,
}

impl Default for Detect {
    fn default() -> Self {
        Detect { fullscreen: true, launchers: true, always: false }
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
    /// Any mouse button or key: "Mouse2" (the right button), "Mouse4", or a
    /// key by its position, "ShiftLeft", "KeyV"... (`platform::KEYS`).
    #[serde(deserialize_with = "aim_button")]
    pub button: String,
    pub mode: AimMode,
}

impl Default for Aim {
    fn default() -> Self {
        Aim { hide: false, button: DEFAULT_AIM.into(), mode: AimMode::Hold }
    }
}

const DEFAULT_AIM: &str = "Mouse2";

/// Reads the aim button, also as 0.1.2 and earlier saved it: a mouse button
/// by its old name ("Right", "Back"...).
fn aim_button<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let name = String::deserialize(deserializer)?;
    let old = ["Left", "Right", "Middle", "Back", "Forward"].iter().position(|old| *old == name);
    Ok(match old {
        Some(index) => ["Mouse1", "Mouse2", "Mouse3", "Mouse4", "Mouse5"][index].into(),
        None => name,
    })
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
    /// Centre on the game window's client area instead of its monitor, for
    /// windowed games; the crosshair follows the window.
    pub centre_on_window: bool,
    /// The player's own picture instead of the CS2 crosshair.
    pub image: Image,
    pub start_with_system: bool,
    /// Windows only; off by default (see the plan's anti-cheat table).
    pub hide_from_capture: bool,
    /// Serve the crosshair to OBS as a Browser source (127.0.0.1 only).
    pub obs: bool,
    /// Off by default: updates install by themselves, between games.
    pub update_manually: bool,
    pub language: Language,
}

/// "auto" (the system's, the default) or a code from `i18n::LANGUAGES`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Language(pub String);

impl Default for Language {
    fn default() -> Self {
        Language(crate::i18n::AUTO.into())
    }
}

fn clean_name(name: &str, fallback: &str) -> String {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(NAME_LIMIT).collect();
    if name.is_empty() { fallback.to_string() } else { name }
}

impl Settings {
    /// Every value in range, names tidy, lists bounded. Idempotent.
    pub fn validated(mut self) -> Settings {
        self.crosshair = self.crosshair.normalized();
        if !crate::i18n::is_setting(&self.language.0) {
            self.language = Language::default();
        }
        if !crate::platform::is_key_name(&self.aim.button) {
            self.aim.button = DEFAULT_AIM.into();
        }
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
            game.image = std::mem::take(&mut game.image).validated();
        }
        self.image = std::mem::take(&mut self.image).validated();
        const LIMIT: i32 = 4000;
        self.offset = (self.offset.0.clamp(-LIMIT, LIMIT), self.offset.1.clamp(-LIMIT, LIMIT));
        self
    }

    /// Every image file the settings use, the main crosshair's and the games'.
    pub fn image_files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self.image.files().collect();
        files.extend(self.games.iter().flat_map(|game| game.image.files()));
        files
    }
}

/// Reads the settings; a missing file gives defaults, an unreadable one is
/// kept beside as `.broken` so nothing the player made is lost silently.
/// The flag is false when defaults stand in for a file that exists: then
/// nothing may be cleaned up on the strength of these settings.
pub fn load(path: &Path) -> (Settings, bool) {
    // A lock by an antivirus or a sync tool at start-up passes: try again.
    let mut read = fs::read_to_string(path);
    for _ in 0..5 {
        match &read {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                std::thread::sleep(std::time::Duration::from_millis(200));
                read = fs::read_to_string(path);
            }
            _ => break,
        }
    }
    let text = match read {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), true),
        Err(error) => {
            // Unreadable but present: keep a copy before defaults replace it.
            log::warn!("settings could not be read ({error}); starting from defaults");
            if let Err(error) = keep_aside(path) {
                log::warn!("a copy of the settings could not be kept aside yet: {error}");
            }
            return (Settings::default(), false);
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(settings) => (settings.validated(), true),
        Err(error) => {
            log::warn!("settings unreadable ({error}); starting from defaults");
            if let Err(error) = keep_aside(path) {
                log::warn!("a copy of the settings could not be kept aside yet: {error}");
            }
            (Settings::default(), false)
        }
    }
}

/// Reads the settings without touching the file, for a report: None when
/// there is no file yet.
pub fn peek(path: &Path) -> Result<Option<Settings>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<Settings>(&text).map(|s| Some(s.validated())).map_err(|error| format!("unreadable: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("could not be read: {error}")),
    }
}

/// `settings.broken-<seconds>.json`, so an earlier broken copy is never lost.
/// The image files the copies of unreadable settings name, so a cleanup
/// never removes what restoring such a copy would need.
pub fn images_kept_aside(path: &Path) -> Vec<String> {
    let (Some(folder), Some(stem)) = (path.parent(), path.file_stem().and_then(|stem| stem.to_str())) else { return Vec::new() };
    let Ok(entries) = fs::read_dir(folder) else { return Vec::new() };
    let prefix = format!("{stem}.broken-");
    let mut names = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        if !entry.file_name().to_string_lossy().starts_with(&prefix) {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let words = text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'));
        names.extend(words.filter(|word| crate::picture::valid_file_name(word)).map(str::to_string));
    }
    names
}

/// Copies settings that could not be used to `settings.broken-<time>.json`.
pub fn keep_aside(path: &Path) -> std::io::Result<u64> {
    fs::copy(path, broken_name(path))
}

/// Whether a copy of unusable settings was kept aside at or after `since`
/// (seconds since 1970), so this run's file is the one copied.
pub fn kept_aside(path: &Path, since: u64) -> bool {
    let (Some(folder), Some(stem)) = (path.parent(), path.file_stem().and_then(|stem| stem.to_str())) else { return false };
    let prefix = format!("{stem}.broken-");
    let Ok(entries) = fs::read_dir(folder) else { return false };
    entries.filter_map(Result::ok).any(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        let seconds = name.strip_prefix(&prefix).and_then(|rest| rest.strip_suffix(".json")).and_then(|n| n.parse::<u64>().ok());
        seconds.is_some_and(|seconds| seconds >= since)
    })
}

/// Seconds since 1970, the stamp in a kept-aside copy's name.
pub fn now_seconds() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn broken_name(path: &Path) -> std::path::PathBuf {
    path.with_extension(format!("broken-{}.json", now_seconds()))
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
        assert_eq!(load(&folder().join("none.json")), (Settings::default(), true));
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
            image: Image { on: true, name: "mine.svg".into(), source: "0123456789abcdef.svg".into(), rendered: "fedcba9876543210.png".into(), size: 48, opacity: 200 },
        });
        settings.detect.always = true;
        settings.centre_on_window = true;
        settings.aim = Aim { hide: true, button: "Mouse4".into(), mode: AimMode::Toggle };
        settings.offset = (3, -1);
        save(&path, &settings).unwrap();
        assert_eq!(load(&path), (settings.validated(), true));
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let path = folder().join("settings.json");
        fs::write(&path, r#"{"aim":{"hide":true},"crosshair":{"gap":-3}}"#).unwrap();
        let (settings, _) = load(&path);
        assert!(settings.aim.hide);
        assert_eq!(settings.aim.button, "Mouse2");
        assert_eq!(settings.crosshair.gap, -3);
        assert_eq!(settings.keys, Keys::default());
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let dir = folder();
        let path = dir.join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), (Settings::default(), false), "defaults stand in, not to be trusted");
        let kept = fs::read_dir(&dir).unwrap().filter_map(Result::ok).any(|e| e.file_name().to_string_lossy().starts_with("settings.broken-"));
        assert!(kept);
    }

    #[test]
    fn only_a_copy_from_this_run_counts_as_kept_aside() {
        let dir = folder();
        let path = dir.join("settings.json");
        fs::write(dir.join("settings.broken-100.json"), "old").unwrap();
        assert!(!kept_aside(&path, 200), "an older incident's copy is not this file's");
        fs::write(&path, "{ unreadable").unwrap();
        let started = now_seconds();
        keep_aside(&path).unwrap();
        assert!(kept_aside(&path, started));
    }

    #[test]
    fn images_named_in_a_broken_copy_are_kept() {
        let dir = folder();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"image":{"rendered":"fedcba9876543210.png","source":"0123456789abcdef.svg" , broken"#).unwrap();
        assert!(!load(&path).1);
        let mut kept = images_kept_aside(&path);
        kept.sort();
        assert_eq!(kept, ["0123456789abcdef.svg", "fedcba9876543210.png"]);
    }

    #[test]
    fn the_aim_button_is_any_key_and_old_names_still_load() {
        let path = folder().join("settings.json");
        let old = [("Back", "Mouse4"), ("Right", "Mouse2"), ("Left", "Mouse1")];
        let new = [("KeyV", "KeyV"), ("ShiftLeft", "ShiftLeft"), ("ArrowRight", "ArrowRight"), ("ArrowLeft", "ArrowLeft"), ("Mouse5", "Mouse5"), ("F13", "F13")];
        for (saved, read) in old.into_iter().chain(new).chain([("V", "Mouse2"), ("bogus", "Mouse2"), ("", "Mouse2")]) {
            fs::write(&path, format!(r#"{{"aim":{{"hide":true,"button":"{saved}"}}}}"#)).unwrap();
            assert_eq!(load(&path).0.aim.button, read, "{saved}");
        }
    }

    #[test]
    fn validation_cleans_and_is_idempotent() {
        let mut settings = Settings::default();
        settings.saved.push(Saved { name: "  \u{7}".into(), crosshair: Crosshair::default() });
        settings.saved.push(Saved { name: "x".repeat(200), crosshair: Crosshair::default() });
        settings.games.push(Game::default());
        settings.games.push(Game { name: String::new(), fingerprint: Fingerprint { name: "cs2".into(), ..Fingerprint::default() }, ..Game::default() });
        settings.offset = (99_999, -99_999);
        let once = settings.validated();
        assert_eq!(once.saved[0].name, "Crosshair");
        assert_eq!(once.saved[1].name.chars().count(), NAME_LIMIT);
        assert_eq!(once.games.len(), 1);
        assert_eq!(once.games[0].name, "cs2");
        assert_eq!(once.offset, (4000, -4000));
        assert_eq!(once.clone().validated(), once);
    }

    #[test]
    fn image_file_names_from_the_file_cannot_point_anywhere_else() {
        let image = Image {
            on: true,
            name: "../../x\\y.svg".into(),
            source: "../../etc/passwd".into(),
            rendered: "0123456789abcdef.svg".into(),
            size: 9000,
            opacity: 10,
        }
        .validated();
        assert_eq!((image.source.as_str(), image.rendered.as_str()), ("", ""), "only Scorpio's own names, and a PNG to show");
        assert!(!image.on, "nothing to show turns it off");
        assert_eq!(image.size, crate::picture::SIZE_LIMIT as u16);
        assert!(!image.name.contains('/') && !image.name.contains('\\'));
        let fine = Image { on: true, source: "0123456789abcdef.webp".into(), rendered: "fedcba9876543210.png".into(), ..Image::default() };
        assert_eq!(fine.clone().validated(), fine);
    }

    #[test]
    fn a_new_player_follows_the_system_language() {
        assert_eq!(Settings::default().language.0, "auto");
        assert_eq!(load(&folder().join("no-language.json")).0.language.0, "auto");
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.language.0, "auto", "settings from before languages");
        let wrong: Settings = serde_json::from_str(r#"{"language":"xx"}"#).unwrap();
        assert_eq!(wrong.validated().language.0, "auto");
    }
}
