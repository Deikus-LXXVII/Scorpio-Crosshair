//! The only code that talks to the operating system. Everything here must pass
//! the anti-cheat review in docs/plans/scorpio.md: never open another process,
//! never hook or inject, never send input, nothing that polls.
//!
//! Each platform runs one background thread that owns the overlay window and
//! listens for system events; the app talks to it through `Platform`.

use crate::core::focus::WindowInfo;
use crate::core::raster::Bitmap;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::Sender;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::start;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use self::linux::start;

#[cfg(not(any(windows, target_os = "linux")))]
mod stub;
#[cfg(not(any(windows, target_os = "linux")))]
pub use self::stub::start;

#[cfg(windows)]
pub use self::windows::system::{attach_console, open_url, version as windows_version};
/// The program name and path of a process, from the system's process list
/// (no handle is opened). For the probes.
#[cfg(windows)]
pub use self::windows::process_image;

/// Opens an address in the default browser.
#[cfg(not(windows))]
pub fn open_url(url: &str) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(opener).arg(url).spawn().map(drop).map_err(|error| format!("{opener} could not be started ({error})"))
}

#[cfg(not(windows))]
pub fn attach_console() {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl MouseButton {
    /// The button a key name stands for: "Mouse1" left, "Mouse2" right,
    /// "Mouse3" middle, "Mouse4" back, "Mouse5" forward.
    pub fn from_name(name: &str) -> Option<MouseButton> {
        Some(match name {
            "Mouse1" => MouseButton::Left,
            "Mouse2" => MouseButton::Right,
            "Mouse3" => MouseButton::Middle,
            "Mouse4" => MouseButton::Back,
            "Mouse5" => MouseButton::Forward,
            _ => return None,
        })
    }
}

/// Every key the aim can be, by physical position: its name (the W3C
/// `KeyboardEvent.code` the settings window binds), its scan code on Windows
/// (0xE0.. for the extended keys, as Raw Input's `MakeCode` and `RI_KEY_E0`
/// give it), and its evdev code on Linux (the X keycode less 8). A position,
/// not a letter, so the binding holds on every keyboard layout. Not here:
/// Escape, which cancels binding, and Pause, which sends no release.
pub const KEYS: [(&str, u16, u16); 115] = [
    ("Digit1", 0x0002, 2),
    ("Digit2", 0x0003, 3),
    ("Digit3", 0x0004, 4),
    ("Digit4", 0x0005, 5),
    ("Digit5", 0x0006, 6),
    ("Digit6", 0x0007, 7),
    ("Digit7", 0x0008, 8),
    ("Digit8", 0x0009, 9),
    ("Digit9", 0x000A, 10),
    ("Digit0", 0x000B, 11),
    ("Minus", 0x000C, 12),
    ("Equal", 0x000D, 13),
    ("Backspace", 0x000E, 14),
    ("Tab", 0x000F, 15),
    ("KeyQ", 0x0010, 16),
    ("KeyW", 0x0011, 17),
    ("KeyE", 0x0012, 18),
    ("KeyR", 0x0013, 19),
    ("KeyT", 0x0014, 20),
    ("KeyY", 0x0015, 21),
    ("KeyU", 0x0016, 22),
    ("KeyI", 0x0017, 23),
    ("KeyO", 0x0018, 24),
    ("KeyP", 0x0019, 25),
    ("BracketLeft", 0x001A, 26),
    ("BracketRight", 0x001B, 27),
    ("Enter", 0x001C, 28),
    ("ControlLeft", 0x001D, 29),
    ("KeyA", 0x001E, 30),
    ("KeyS", 0x001F, 31),
    ("KeyD", 0x0020, 32),
    ("KeyF", 0x0021, 33),
    ("KeyG", 0x0022, 34),
    ("KeyH", 0x0023, 35),
    ("KeyJ", 0x0024, 36),
    ("KeyK", 0x0025, 37),
    ("KeyL", 0x0026, 38),
    ("Semicolon", 0x0027, 39),
    ("Quote", 0x0028, 40),
    ("Backquote", 0x0029, 41),
    ("ShiftLeft", 0x002A, 42),
    ("Backslash", 0x002B, 43),
    ("KeyZ", 0x002C, 44),
    ("KeyX", 0x002D, 45),
    ("KeyC", 0x002E, 46),
    ("KeyV", 0x002F, 47),
    ("KeyB", 0x0030, 48),
    ("KeyN", 0x0031, 49),
    ("KeyM", 0x0032, 50),
    ("Comma", 0x0033, 51),
    ("Period", 0x0034, 52),
    ("Slash", 0x0035, 53),
    ("ShiftRight", 0x0036, 54),
    ("NumpadMultiply", 0x0037, 55),
    ("AltLeft", 0x0038, 56),
    ("Space", 0x0039, 57),
    ("CapsLock", 0x003A, 58),
    ("F1", 0x003B, 59),
    ("F2", 0x003C, 60),
    ("F3", 0x003D, 61),
    ("F4", 0x003E, 62),
    ("F5", 0x003F, 63),
    ("F6", 0x0040, 64),
    ("F7", 0x0041, 65),
    ("F8", 0x0042, 66),
    ("F9", 0x0043, 67),
    ("F10", 0x0044, 68),
    ("ScrollLock", 0x0046, 70),
    ("Numpad7", 0x0047, 71),
    ("Numpad8", 0x0048, 72),
    ("Numpad9", 0x0049, 73),
    ("NumpadSubtract", 0x004A, 74),
    ("Numpad4", 0x004B, 75),
    ("Numpad5", 0x004C, 76),
    ("Numpad6", 0x004D, 77),
    ("NumpadAdd", 0x004E, 78),
    ("Numpad1", 0x004F, 79),
    ("Numpad2", 0x0050, 80),
    ("Numpad3", 0x0051, 81),
    ("Numpad0", 0x0052, 82),
    ("NumpadDecimal", 0x0053, 83),
    ("IntlBackslash", 0x0056, 86),
    ("F11", 0x0057, 87),
    ("F12", 0x0058, 88),
    ("F13", 0x0064, 183),
    ("F14", 0x0065, 184),
    ("F15", 0x0066, 185),
    ("F16", 0x0067, 186),
    ("F17", 0x0068, 187),
    ("F18", 0x0069, 188),
    ("F19", 0x006A, 189),
    ("F20", 0x006B, 190),
    ("F21", 0x006C, 191),
    ("F22", 0x006D, 192),
    ("F23", 0x006E, 193),
    ("F24", 0x0076, 194),
    ("NumpadEnter", 0xE01C, 96),
    ("ControlRight", 0xE01D, 97),
    ("NumpadDivide", 0xE035, 98),
    ("PrintScreen", 0xE037, 99),
    ("AltRight", 0xE038, 100),
    ("NumLock", 0xE045, 69),
    ("Home", 0xE047, 102),
    ("ArrowUp", 0xE048, 103),
    ("PageUp", 0xE049, 104),
    ("ArrowLeft", 0xE04B, 105),
    ("ArrowRight", 0xE04D, 106),
    ("End", 0xE04F, 107),
    ("ArrowDown", 0xE050, 108),
    ("PageDown", 0xE051, 109),
    ("Insert", 0xE052, 110),
    ("Delete", 0xE053, 111),
    ("MetaLeft", 0xE05B, 125),
    ("MetaRight", 0xE05C, 126),
    ("ContextMenu", 0xE05D, 127),
];

/// Whether `name` is a mouse button ("Mouse1" to "Mouse5") or a key in `KEYS`.
pub fn is_key_name(name: &str) -> bool {
    MouseButton::from_name(name).is_some() || KEYS.iter().any(|key| key.0 == name)
}

/// A window's client area, its monitor (x, y, width, height, physical
/// pixels) and whether it is fullscreen: what following it reports.
pub type Geometry = (Option<[i32; 4]>, Option<[i32; 4]>, bool);

/// What the platform thread tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The window in front changed; None when nothing (or the desktop) is.
    Foreground(Option<WindowInfo>),
    /// A mouse button anywhere, read from raw input (never a hook).
    Mouse { button: MouseButton, down: bool },
    /// A key anywhere, read from raw input; `code` is a platform-neutral key
    /// name as in keybinds ("H", "F5", "Shift", "Mouse4"...).
    Key { code: String, down: bool },
    /// The followed window (see `Command::Follow`) moved or changed size:
    /// its client area and monitor now, physical pixels. Sent only when
    /// something changed, so a drag sends one event per real change.
    Geometry { pid: u32, client: Option<[i32; 4]>, monitor: Option<[i32; 4]>, fullscreen: bool },
    /// The unlocked overlay was dragged to this position (physical pixels).
    Moved { x: i32, y: i32 },
    /// The unlocked overlay's centre button was clicked.
    CenterClicked,
    /// Whether the fullscreen window in front runs in exclusive fullscreen,
    /// where Windows shows no other window over it. Windows only: asked of
    /// the shell a few times after such a window comes to the front.
    Exclusive(bool),
}

/// What the app tells the platform thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Show this bitmap with its top-left at (x, y), physical pixels, topmost,
    /// click-through when `unlocked` is false. When `unlocked` is true the
    /// window takes the mouse: dragging moves it (reported as `Moved`), a click
    /// inside `button` (x, y, w, h relative to the bitmap) reports `CenterClicked`.
    Show { bitmap: Bitmap, x: i32, y: i32, unlocked: bool, button: Option<(i32, i32, i32, i32)> },
    /// Put the shown bitmap's top-left at (x, y) without drawing it again.
    Move { x: i32, y: i32 },
    /// Hide the overlay without destroying it.
    Hide,
    /// Put the shown overlay back above every other window; sent when the
    /// window in front changes, since a newly active window may go above it.
    Raise,
    /// Keep the overlay out of screenshots and recordings where the OS allows
    /// it (Windows 10 2004+). Off unless the player turns it on.
    ExcludeFromCapture(bool),
    /// Mouse buttons and keys to report; nothing is listened to otherwise.
    ListenTo { mouse: Vec<MouseButton>, keys: Vec<String> },
    /// Report moves and resizes of the window in front (the last one that was
    /// not Scorpio's) as `Event::Geometry`. Nothing is watched while false.
    Follow(bool),
    Quit,
}

/// A handle to the running platform thread.
pub struct Platform {
    pub commands: Sender<Command>,
    /// Wakes the platform thread after a command was sent.
    pub wake: Box<dyn Fn() + Send + Sync>,
    /// Top-level windows a game could be, for "add a running game". Reads only
    /// window-manager data and the system's process list.
    pub list_windows: Box<dyn Fn() -> Vec<WindowInfo> + Send + Sync>,
}

impl Platform {
    pub fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            (self.wake)();
        }
    }
}

/// Why the platform could not start (for example Wayland without X11).
#[derive(Debug)]
pub struct Unsupported(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_window_binds_exactly_these_keys() {
        let ui = include_str!("../../../ui/ui.js");
        let list = ui.split("const AIM_CODES = [").nth(1).and_then(|rest| rest.split(']').next()).expect("AIM_CODES in ui.js");
        let bound: Vec<&str> = list.split(',').map(|item| item.trim().trim_matches('\'')).filter(|item| !item.is_empty()).collect();
        let read: Vec<&str> = KEYS.iter().map(|key| key.0).collect();
        assert_eq!(bound, read);
    }

    #[test]
    fn each_position_and_code_is_one_key() {
        for (i, a) in KEYS.iter().enumerate() {
            for b in &KEYS[i + 1..] {
                assert!(a.0 != b.0 && a.1 != b.1 && a.2 != b.2, "{a:?} and {b:?}");
            }
        }
        assert!(is_key_name("KeyV") && is_key_name("Mouse4") && !is_key_name("V") && !is_key_name("Mouse6"));
    }
}
