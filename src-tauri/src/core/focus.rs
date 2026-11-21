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
    Fingerprint {
        name: window.name.trim().to_owned(),
        path: window.path.trim().to_owned(),
        title: window.title.trim().to_owned(),
        window_class: window.window_class.trim().to_owned(),
    }
}

fn program_name(value: &str) -> String {
    let name = value.trim().to_lowercase();
    name.strip_suffix(".exe").map_or(name.clone(), str::to_owned)
}

fn same_path(a: &str, b: &str) -> bool {
    let norm = |p: &str| p.trim().to_lowercase().replace('/', "\\");
    norm(a) == norm(b)
}

/// None when either side has nothing to compare.
fn check(saved: &str, seen: &str, same: impl Fn(&str, &str) -> bool) -> Option<bool> {
    (!saved.trim().is_empty() && !seen.trim().is_empty()).then(|| same(saved, seen))
}

/// Names and paths compare without case (and paths without slash style);
/// titles and classes exactly. Two agreeing checks make a match.
pub fn compare(saved: &Fingerprint, window: &WindowInfo) -> Verdict {
    let exact = |a: &str, b: &str| a.trim() == b.trim();
    let name = check(&saved.name, &window.name, |a, b| program_name(a) == program_name(b));
    let path = check(&saved.path, &window.path, same_path);
    let title = check(&saved.title, &window.title, exact);
    let window_class = check(&saved.window_class, &window.window_class, exact);
    let agree = [name, path, title, window_class].iter().filter(|c| **c == Some(true)).count() as u8;
    Verdict { name, path, title, window_class, agree, matches: agree >= AGREE }
}

/// A program can be told apart only by its name or its executable.
pub fn identifiable(window: &WindowInfo) -> bool {
    !window.name.trim().is_empty() || !window.path.trim().is_empty()
}

/// Index of the saved fingerprint this window is, the one with most agreeing checks.
pub fn find_game(saved: &[Fingerprint], window: &WindowInfo) -> Option<(usize, Verdict)> {
    let mut best: Option<(usize, Verdict)> = None;
    for (index, game) in saved.iter().enumerate() {
        let verdict = compare(game, window);
        if verdict.matches && best.as_ref().map_or(true, |(_, b)| verdict.agree > b.agree) {
            best = Some((index, verdict));
        }
    }
    best
}

/// Window classes of the desktop and the shell, which can fill a monitor too.
const SHELL_CLASSES: [&str; 7] = ["progman", "workerw", "shell_traywnd", "windows.ui.core.corewindow", "desktop_window", "xfdesktop", "nautilus-desktop"];
const SHELL_PROGRAMS: [&str; 3] = ["explorer", "plasmashell", "gnome-shell"];

/// A borderless or fullscreen window counts as a game unless it is the desktop
/// or the shell (Progman, WorkerW, Shell_TrayWnd, explorer, plasmashell,
/// gnome-shell...). Returns the key used to exclude it: "exe:<path or name, lower case>".
pub fn fullscreen_key(window: &WindowInfo) -> Option<String> {
    let class = window.window_class.trim().to_lowercase();
    let shell = SHELL_CLASSES.contains(&class.as_str()) || SHELL_PROGRAMS.contains(&program_name(&window.name).as_str());
    if !window.fullscreen || shell {
        return None;
    }
    let path = window.path.trim();
    let program = if path.is_empty() { window.name.trim() } else { path };
    Some(format!("exe:{}", program.to_lowercase()))
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
    let Some(front) = front else {
        return previous.clone();
    };
    if front.pid == own_pid {
        return FocusState { visible: true, game: previous.game };
    }
    match find_game(saved, front) {
        Some((index, _)) => FocusState { visible: true, game: Some(index) },
        None => FocusState { visible: detected(front), game: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: u32 = 4242;

    fn window(pid: u32, name: &str, path: &str, title: &str, class: &str, fullscreen: bool) -> WindowInfo {
        WindowInfo { pid, name: name.into(), path: path.into(), title: title.into(), window_class: class.into(), fullscreen }
    }

    fn cs2() -> WindowInfo {
        window(20, "cs2", r"C:\Steam\steamapps\common\Counter-Strike Global Offensive\game\bin\win64\cs2.exe", "Counter-Strike 2", "SDL_app", true)
    }

    fn finals() -> WindowInfo {
        window(30, "Discovery", r"C:\Steam\steamapps\common\The Finals\Discovery.exe", "THE FINALS", "UnrealWindow", true)
    }

    fn browser() -> WindowInfo {
        window(10, "brave", r"C:\Brave\brave.exe", "New Tab - Brave", "Chrome_WidgetWin_1", false)
    }

    fn games() -> Vec<Fingerprint> {
        vec![fingerprint(&cs2()), fingerprint(&finals())]
    }

    #[test]
    fn two_of_four_checks_make_a_match_empty_checks_do_not_vote() {
        assert_eq!(compare(&fingerprint(&cs2()), &cs2()).agree, 4);
        let moved = WindowInfo { title: "CS2 - 300 fps".into(), path: r"D:\moved\cs2.exe".into(), ..cs2() };
        assert!(compare(&fingerprint(&cs2()), &moved).matches);
        assert!(!compare(&fingerprint(&cs2()), &WindowInfo { window_class: "SDL_app".into(), ..browser() }).matches);
        let bare = WindowInfo { pid: 1, name: "CS2.EXE".into(), path: cs2().path.to_uppercase().replace('\\', "/"), ..WindowInfo::default() };
        let v = compare(&fingerprint(&cs2()), &bare);
        assert_eq!((v.name, v.path, v.title, v.window_class), (Some(true), Some(true), None, None));
    }

    #[test]
    fn titles_and_classes_compare_exactly() {
        let shouting = WindowInfo { title: "COUNTER-STRIKE 2".into(), window_class: "sdl_app".into(), ..cs2() };
        let v = compare(&fingerprint(&cs2()), &shouting);
        assert_eq!((v.title, v.window_class, v.agree), (Some(false), Some(false), 2));
    }

    #[test]
    fn the_best_agreeing_saved_game_wins() {
        let twin = Fingerprint { title: "x".into(), window_class: "y".into(), ..fingerprint(&cs2()) };
        let saved = [vec![twin], games()].concat();
        assert_eq!(find_game(&saved, &cs2()).map(|(i, _)| i), Some(1));
        assert_eq!(find_game(&games(), &browser()), None);
    }

    #[test]
    fn focus_games_show_it_others_hide_it_scorpio_shows_it_and_keeps_the_game() {
        let saved = games();
        let never = |_: &WindowInfo| false;
        let step = |previous: &FocusState, front: Option<&WindowInfo>| next_state(previous, front, OWN, &saved, never);
        let own = WindowInfo { pid: OWN, name: "Scorpio".into(), ..WindowInfo::default() };

        let mut s = FocusState { visible: true, game: None };
        s = step(&s, Some(&browser()));
        assert_eq!(s, FocusState { visible: false, game: None });
        s = step(&s, Some(&cs2()));
        assert_eq!(s, FocusState { visible: true, game: Some(0) });
        s = step(&s, Some(&own));
        assert_eq!(s, FocusState { visible: true, game: Some(0) }, "settings in front: shown, same game");
        assert!(step(&step(&s, Some(&browser())), Some(&own)).visible, "settings opened over the browser: shown to tune it");
        assert_eq!(step(&s, None), s);

        let other = WindowInfo { pid: 77, ..browser() };
        assert_eq!(next_state(&s, Some(&other), OWN, &saved, |w| w.pid == 77), FocusState { visible: true, game: None });
    }

    #[test]
    fn a_fullscreen_window_is_a_game_unless_it_is_the_desktop_or_the_shell() {
        assert_eq!(fullscreen_key(&cs2()), Some(format!("exe:{}", cs2().path.to_lowercase())));
        assert_eq!(fullscreen_key(&browser()), None);
        assert_eq!(fullscreen_key(&WindowInfo { name: "explorer".into(), window_class: "Progman".into(), ..cs2() }), None);
        assert_eq!(fullscreen_key(&WindowInfo { name: "plasmashell".into(), window_class: "plasmashell".into(), ..cs2() }), None);
        assert_eq!(fullscreen_key(&WindowInfo { name: "Game.exe".into(), path: String::new(), ..cs2() }), Some("exe:game.exe".into()));
    }

    #[test]
    fn only_a_name_or_a_path_identifies_a_program() {
        assert!(!identifiable(&WindowInfo { pid: 1, name: " ".into(), title: "t".into(), ..WindowInfo::default() }));
        assert!(identifiable(&cs2()));
    }
}
