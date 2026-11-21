//! Deciding whether the window in front is a game. Reference behaviour:
//! `reference/js/focus/match.js` and `reference/test/unit/focus.test.js`.

use serde::{Deserialize, Serialize};

/// What the platform reports about a top-level window. `path` may be empty
/// when the system won't say (never worked around by opening the process).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowInfo {
    pub pid: u32,
    pub name: String,
    pub path: String,
    pub title: String,
    pub window_class: String,
    pub fullscreen: bool,
}

/// The four checks of a saved game.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Fingerprint {
    pub name: String,
    pub path: String,
    pub title: String,
    pub window_class: String,
}

/// Each check: Some(true/false), or None when either side is empty.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub name: Option<bool>,
    pub path: Option<bool>,
    pub title: Option<bool>,
    pub window_class: Option<bool>,
    pub agree: u8,
    pub matches: bool,
}

pub const AGREE: u8 = 2;

pub fn fingerprint(window: &WindowInfo) -> Fingerprint {
    todo!()
}

/// Names and paths compare without case (and paths without slash style);
/// titles and classes exactly. Two agreeing checks make a match.
pub fn compare(saved: &Fingerprint, window: &WindowInfo) -> Verdict {
    todo!()
}

/// A program can be told apart only by its name or its executable.
pub fn identifiable(window: &WindowInfo) -> bool {
    todo!()
}

/// Index of the saved fingerprint this window is, the one with most agreeing checks.
pub fn find_game(saved: &[Fingerprint], window: &WindowInfo) -> Option<(usize, Verdict)> {
    todo!()
}

/// A borderless or fullscreen window counts as a game unless it is the desktop
/// or the shell (Progman, WorkerW, Shell_TrayWnd, explorer, plasmashell,
/// gnome-shell...). Returns the key used to exclude it: "exe:<path or name, lower case>".
pub fn fullscreen_key(window: &WindowInfo) -> Option<String> {
    todo!()
}

#[derive(Clone, Debug, PartialEq)]
pub struct FocusState {
    pub visible: bool,
    /// Index into the saved games.
    pub game: Option<usize>,
}

/// After the window in front changes. Scorpio's own windows show the crosshair
/// and keep the game that was in focus. `detected` says whether an unsaved
/// window counts as a game (launcher install or fullscreen, not excluded).
pub fn next_state(previous: &FocusState, front: Option<&WindowInfo>, own_pid: u32, saved: &[Fingerprint], detected: impl Fn(&WindowInfo) -> bool) -> FocusState {
    todo!()
}
