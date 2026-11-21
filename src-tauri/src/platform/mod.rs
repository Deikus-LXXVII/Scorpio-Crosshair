//! The only code that talks to the operating system. Everything here must pass
//! the anti-cheat review in docs/plans/scorpio.md: never open another process,
//! never hook or inject, never send input, nothing that polls.
//!
//! Each platform runs one background thread that owns the overlay window and
//! listens for system events; the app talks to it through `Platform`.

use crate::core::focus::WindowInfo;
use crate::core::raster::Bitmap;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

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
    /// The unlocked overlay was dragged to this position (physical pixels).
    Moved { x: i32, y: i32 },
    /// The unlocked overlay's centre button was clicked.
    CenterClicked,
}

/// What the app tells the platform thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Show this bitmap with its top-left at (x, y), physical pixels, topmost,
    /// click-through when `unlocked` is false. When `unlocked` is true the
    /// window takes the mouse: dragging moves it (reported as `Moved`), a click
    /// inside `button` (x, y, w, h relative to the bitmap) reports `CenterClicked`.
    Show { bitmap: Bitmap, x: i32, y: i32, unlocked: bool, button: Option<(i32, i32, i32, i32)> },
    /// Hide the overlay without destroying it.
    Hide,
    /// Keep the overlay out of screenshots and recordings where the OS allows it.
    ExcludeFromCapture(bool),
    /// Mouse buttons and keys to report; nothing is listened to otherwise.
    ListenTo { mouse: Vec<MouseButton>, keys: Vec<String> },
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
