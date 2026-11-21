//! What the overlay should be doing, decided from what happened. Pure: it
//! takes events and returns commands for the platform thread, so every rule
//! here is tested without a screen.

use crate::core::center::{centred, Area};
use crate::core::crosshair::{draw, Crosshair};
use crate::core::focus::{self, FocusState, WindowInfo};
use crate::core::launchers::{game_at, InstalledGame};
use crate::core::raster::{rasterize, slab};
use crate::platform::{Command, Event, MouseButton};
use crate::store::{AimMode, Game, Settings};

/// Space around the crosshair on the unlocked slab, at least.
const SLAB_PAD: u32 = 40;
const SLAB_MIN: u32 = 132;

/// The display the crosshair is centred on, physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for Display {
    fn default() -> Self {
        Display { x: 0, y: 0, width: 1920, height: 1080 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hotkey {
    Lock,
    Center,
    Hide,
    Next,
    Previous,
    AddGame,
}

impl Hotkey {
    pub fn from_id(id: &str) -> Option<Hotkey> {
        Some(match id {
            "lock" => Hotkey::Lock,
            "center" => Hotkey::Center,
            "hide" => Hotkey::Hide,
            "next" => Hotkey::Next,
            "previous" => Hotkey::Previous,
            "addGame" => Hotkey::AddGame,
            _ => return None,
        })
    }
}

/// What a step changed besides the overlay.
#[derive(Debug, Default, PartialEq)]
pub struct Effects {
    pub commands: Vec<Command>,
    /// The settings changed and should be saved and shown.
    pub settings_changed: bool,
    /// Something the settings window shows changed (focus, lock, hidden).
    pub status_changed: bool,
}

/// What the overlay currently shows, to send a command only on a change.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    Hidden,
    Crosshair { crosshair: Crosshair, display: Display, offset: (i32, i32), unlocked: bool },
}

pub struct App {
    pub settings: Settings,
    pub installed: Vec<InstalledGame>,
    pub own_pid: u32,
    pub display: Display,
    pub focus: FocusState,
    pub front: Option<WindowInfo>,
    /// The last window in front that was not Scorpio's, for "add this game".
    pub last_outside: Option<WindowInfo>,
    pub locked: bool,
    pub hidden: bool,
    aiming: bool,
    shown: Shown,
    listening: Option<Vec<MouseButton>>,
    capture_hidden: Option<bool>,
    saved_index: Option<usize>,
}

impl App {
    pub fn new(settings: Settings, own_pid: u32) -> App {
        App {
            settings,
            installed: Vec::new(),
            own_pid,
            display: Display::default(),
            focus: FocusState { visible: false, game: None },
            front: None,
            last_outside: None,
            locked: true,
            hidden: false,
            aiming: false,
            shown: Shown::Hidden,
            listening: None,
            capture_hidden: None,
            saved_index: None,
        }
    }

    /// The crosshair for what is in front: the game's own, else the main one.
    pub fn active_crosshair(&self) -> &Crosshair {
        self.focus
            .game
            .and_then(|index| self.settings.games.get(index))
            .and_then(|game| game.crosshair.as_ref())
            .unwrap_or(&self.settings.crosshair)
    }

    fn active_crosshair_mut(&mut self) -> &mut Crosshair {
        let game = self.focus.game.filter(|index| self.settings.games.get(*index).is_some_and(|game| game.crosshair.is_some()));
        match game {
            Some(index) => self.settings.games[index].crosshair.as_mut().expect("checked above"),
            None => &mut self.settings.crosshair,
        }
    }

    fn detected(&self, window: &WindowInfo) -> bool {
        let detect = &self.settings.detect;
        (detect.fullscreen && focus::fullscreen_key(window).is_some())
            || (detect.launchers && !window.path.is_empty() && game_at(&window.path, &self.installed).is_some())
    }

    fn refocus(&mut self) {
        let saved: Vec<_> = self.settings.games.iter().map(|game| game.fingerprint.clone()).collect();
        let front = self.front.clone();
        let next = focus::next_state(&self.focus, front.as_ref(), self.own_pid, &saved, |window| self.detected(window));
        if next.game != self.focus.game {
            self.aiming = false;
        }
        self.focus = next;
    }

    fn should_show(&self) -> bool {
        !self.locked || (self.focus.visible && !self.hidden && !(self.settings.aim.hide && self.aiming))
    }

    pub fn event(&mut self, event: Event) -> Effects {
        let mut effects = Effects::default();
        match event {
            Event::Foreground(front) => {
                if let Some(window) = &front {
                    if window.pid != self.own_pid {
                        self.last_outside = Some(window.clone());
                    }
                }
                self.front = front;
                self.refocus();
                effects.status_changed = true;
            }
            Event::Mouse { button, down } => {
                if self.settings.aim.hide && button == self.settings.aim.button {
                    match self.settings.aim.mode {
                        AimMode::Hold => self.aiming = down,
                        AimMode::Toggle if down => self.aiming = !self.aiming,
                        AimMode::Toggle => {}
                    }
                }
            }
            Event::Key { .. } => {}
            Event::Moved { x, y } => {
                if let Shown::Crosshair { .. } = self.shown {
                    let size = self.slab_size();
                    let (cx, cy) = self.display_centre_corner(size);
                    self.settings.offset = (x - cx, y - cy);
                    self.settings = std::mem::take(&mut self.settings).validated();
                    effects.settings_changed = true;
                }
            }
            Event::CenterClicked => {
                self.settings.offset = (0, 0);
                effects.settings_changed = true;
            }
        }
        self.sync(&mut effects);
        effects
    }

    pub fn hotkey(&mut self, key: Hotkey) -> Effects {
        let mut effects = Effects::default();
        match key {
            Hotkey::Lock => {
                self.locked = !self.locked;
                effects.status_changed = true;
            }
            Hotkey::Center => {
                self.settings.offset = (0, 0);
                effects.settings_changed = true;
            }
            Hotkey::Hide => {
                self.hidden = !self.hidden;
                effects.status_changed = true;
            }
            Hotkey::Next | Hotkey::Previous => {
                let count = self.settings.saved.len();
                if count > 0 {
                    let index = match (self.saved_index, key) {
                        (None, Hotkey::Next) => 0,
                        (None, _) => count - 1,
                        (Some(i), Hotkey::Next) => (i + 1) % count,
                        (Some(i), _) => (i + count - 1) % count,
                    };
                    self.saved_index = Some(index);
                    let chosen = self.settings.saved[index].crosshair.clone();
                    *self.active_crosshair_mut() = chosen;
                    effects.settings_changed = true;
                }
            }
            Hotkey::AddGame => {
                if self.add_game_in_front() {
                    effects.settings_changed = true;
                    effects.status_changed = true;
                }
            }
        }
        self.sync(&mut effects);
        effects
    }

    /// Adds the window in front as a game; never Scorpio's own window, and
    /// never one already added. True when one was added.
    pub fn add_game_in_front(&mut self) -> bool {
        let target = match &self.front {
            Some(front) if front.pid != self.own_pid => front.clone(),
            _ => return false,
        };
        self.add_game(&target)
    }

    pub fn add_game(&mut self, window: &WindowInfo) -> bool {
        if !focus::identifiable(window) {
            return false;
        }
        let saved: Vec<_> = self.settings.games.iter().map(|game| game.fingerprint.clone()).collect();
        if focus::find_game(&saved, window).is_some() {
            return false;
        }
        let name = if window.name.is_empty() { window.title.clone() } else { window.name.clone() };
        self.settings.games.push(Game { name, fingerprint: focus::fingerprint(window), crosshair: None });
        self.settings = std::mem::take(&mut self.settings).validated();
        self.refocus();
        true
    }

    /// New settings from the settings window.
    pub fn replace_settings(&mut self, settings: Settings) -> Effects {
        let mut effects = Effects { settings_changed: true, ..Effects::default() };
        self.settings = settings.validated();
        self.saved_index = None;
        self.refocus();
        self.sync(&mut effects);
        effects
    }

    pub fn set_display(&mut self, display: Display) -> Effects {
        let mut effects = Effects::default();
        self.display = display;
        self.sync(&mut effects);
        effects
    }

    pub fn set_installed(&mut self, installed: Vec<InstalledGame>) -> Effects {
        let mut effects = Effects { status_changed: true, ..Effects::default() };
        self.installed = installed;
        self.refocus();
        self.sync(&mut effects);
        effects
    }

    pub fn set_locked(&mut self, locked: bool) -> Effects {
        if self.locked == locked {
            return Effects::default();
        }
        self.hotkey(Hotkey::Lock)
    }

    fn crosshair_size(&self) -> u32 {
        draw(self.active_crosshair(), self.display.height).size
    }

    /// The unlocked slab is square and keeps the crosshair's parity, so the
    /// crosshair sits on the same pixels locked and unlocked.
    fn slab_size(&self) -> u32 {
        let size = self.crosshair_size();
        if self.locked {
            return size;
        }
        let mut slab = (size + 2 * SLAB_PAD).max(SLAB_MIN);
        if (slab - size) % 2 == 1 {
            slab += 1;
        }
        slab
    }

    /// Top-left corner that centres a square of `size` on the display.
    fn display_centre_corner(&self, size: u32) -> (i32, i32) {
        let d = self.display;
        let area = Area { x: d.x as f64, y: d.y as f64, width: d.width as f64, height: d.height as f64 };
        let placed = centred(area, size, size, 1.0);
        (placed.x, placed.y)
    }

    /// Sends what changed since the last step.
    fn sync(&mut self, effects: &mut Effects) {
        let want = if self.should_show() {
            Shown::Crosshair {
                crosshair: self.active_crosshair().clone(),
                display: self.display,
                offset: self.settings.offset,
                unlocked: !self.locked,
            }
        } else {
            Shown::Hidden
        };
        if want != self.shown {
            effects.commands.push(self.render(&want));
            self.shown = want;
        }

        let listen = if self.settings.aim.hide && self.focus.visible && self.locked { vec![self.settings.aim.button] } else { Vec::new() };
        if self.listening.as_ref() != Some(&listen) {
            effects.commands.push(Command::ListenTo { mouse: listen.clone(), keys: Vec::new() });
            self.listening = Some(listen);
        }

        if self.capture_hidden != Some(self.settings.hide_from_capture) {
            effects.commands.push(Command::ExcludeFromCapture(self.settings.hide_from_capture));
            self.capture_hidden = Some(self.settings.hide_from_capture);
        }
    }

    fn render(&self, shown: &Shown) -> Command {
        let Shown::Crosshair { crosshair, display, offset, unlocked } = shown else {
            return Command::Hide;
        };
        let bitmap = rasterize(&draw(crosshair, display.height), 1.0);
        let (bitmap, button) = if *unlocked {
            let size = self.slab_size();
            let (bitmap, button) = slab(&bitmap, size, size);
            (bitmap, Some(button))
        } else {
            (bitmap, None)
        };
        let (x, y) = self.display_centre_corner(bitmap.width);
        Command::Show { bitmap, x: x + offset.0, y: y + offset.1, unlocked: *unlocked, button }
    }

    /// Plain words for the settings window's status line.
    pub fn status(&self) -> Status {
        let game = self.focus.game.and_then(|index| self.settings.games.get(index)).map(|game| game.name.clone());
        let front = self.front.as_ref().filter(|window| window.pid != self.own_pid).cloned().or_else(|| self.last_outside.clone());
        Status { visible: self.focus.visible, game, front, locked: self.locked, hidden: self.hidden }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub visible: bool,
    pub game: Option<String>,
    pub front: Option<WindowInfo>,
    pub locked: bool,
    pub hidden: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: u32 = 7;

    fn window(pid: u32, name: &str, fullscreen: bool) -> WindowInfo {
        WindowInfo {
            pid,
            name: name.into(),
            path: format!("c:\\games\\{name}.exe"),
            title: name.to_uppercase(),
            window_class: format!("{name}-class"),
            fullscreen,
        }
    }

    fn app() -> App {
        let mut settings = Settings::default();
        settings.detect.fullscreen = false;
        settings.detect.launchers = false;
        settings.games.push(Game { name: "Game".into(), fingerprint: focus::fingerprint(&window(1, "game", false)), crosshair: None });
        let mut app = App::new(settings, OWN);
        app.capture_hidden = Some(false);
        app.listening = Some(Vec::new());
        app
    }

    fn shows(effects: &Effects) -> Option<bool> {
        effects.commands.iter().rev().find_map(|command| match command {
            Command::Show { .. } => Some(true),
            Command::Hide => Some(false),
            _ => None,
        })
    }

    #[test]
    fn shows_only_while_an_added_game_is_in_front() {
        let mut app = app();
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(1, "game", false))))), Some(true));
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(2, "browser", false))))), Some(false));
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(1, "game", false))))), Some(true));
    }

    #[test]
    fn nothing_is_sent_when_nothing_changes() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(1, "game", false))));
        assert!(app.event(Event::Foreground(Some(window(1, "game", false)))).commands.is_empty());
    }

    #[test]
    fn own_window_keeps_the_crosshair_and_add_game_ignores_it() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(1, "game", false))));
        let effects = app.event(Event::Foreground(Some(window(OWN, "scorpio", false))));
        assert_eq!(shows(&effects), None);
        assert!(app.focus.visible);
        assert!(!app.add_game_in_front());
        assert_eq!(app.settings.games.len(), 1);
    }

    #[test]
    fn add_game_adds_the_window_in_front_once() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(3, "other", false))));
        assert!(!app.focus.visible);
        let effects = app.hotkey(Hotkey::AddGame);
        assert!(effects.settings_changed);
        assert_eq!(shows(&effects), Some(true));
        assert_eq!(app.settings.games.len(), 2);
        assert!(!app.add_game_in_front());
    }

    #[test]
    fn fullscreen_detection_follows_the_setting() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(4, "unknown", true))));
        assert!(!app.focus.visible);
        let mut settings = app.settings.clone();
        settings.detect.fullscreen = true;
        assert_eq!(shows(&app.replace_settings(settings)), Some(true));
    }

    #[test]
    fn game_crosshair_replaces_the_main_one() {
        let mut app = app();
        let red = Crosshair { color: [255, 0, 0], ..Crosshair::default() };
        app.settings.games[0].crosshair = Some(red.clone());
        app.event(Event::Foreground(Some(window(1, "game", false))));
        assert_eq!(app.active_crosshair(), &red);
        app.event(Event::Foreground(Some(window(2, "browser", false))));
        assert_eq!(app.active_crosshair(), &app.settings.crosshair);
    }

    #[test]
    fn aim_hold_and_toggle() {
        let mut app = app();
        app.settings.aim.hide = true;
        let effects = app.event(Event::Foreground(Some(window(1, "game", false))));
        assert!(effects.commands.contains(&Command::ListenTo { mouse: vec![MouseButton::Right], keys: vec![] }));
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: true })), Some(false));
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: false })), Some(true));
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Left, down: true })), None);

        app.settings.aim.mode = AimMode::Toggle;
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: true })), Some(false));
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: false })), None);
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: true })), Some(true));
    }

    #[test]
    fn nothing_is_listened_to_outside_a_game() {
        let mut app = app();
        app.settings.aim.hide = true;
        app.event(Event::Foreground(Some(window(1, "game", false))));
        let effects = app.event(Event::Foreground(Some(window(2, "browser", false))));
        assert!(effects.commands.contains(&Command::ListenTo { mouse: vec![], keys: vec![] }));
    }

    #[test]
    fn hide_hotkey_and_unlock() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(1, "game", false))));
        assert_eq!(shows(&app.hotkey(Hotkey::Hide)), Some(false));
        let effects = app.hotkey(Hotkey::Lock);
        let Some(Command::Show { unlocked, button, .. }) = effects.commands.first() else { panic!("unlocked shows the slab") };
        assert!(*unlocked && button.is_some());
        app.hotkey(Hotkey::Lock);
        assert!(app.locked);
    }

    #[test]
    fn locked_crosshair_is_exactly_centred_and_slab_keeps_its_centre() {
        let mut app = app();
        app.set_display(Display { x: 1920, y: 0, width: 2560, height: 1440 });
        let locked = app.event(Event::Foreground(Some(window(1, "game", false))));
        let Some(Command::Show { bitmap, x, y, .. }) = locked.commands.first() else { panic!() };
        assert_eq!(2 * x + bitmap.width as i32, 2 * 1920 + 2560);
        assert_eq!(2 * y + bitmap.height as i32, 1440);
        let (cx, cy) = (2 * x + bitmap.width as i32, 2 * y + bitmap.height as i32);

        let unlocked = app.hotkey(Hotkey::Lock);
        let Some(Command::Show { bitmap, x, y, .. }) = unlocked.commands.first() else { panic!() };
        assert_eq!((2 * x + bitmap.width as i32, 2 * y + bitmap.height as i32), (cx, cy));
    }

    #[test]
    fn dragging_sets_the_offset_and_centre_button_resets_it() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(1, "game", false))));
        let effects = app.hotkey(Hotkey::Lock);
        let Some(Command::Show { x, y, .. }) = effects.commands.first().cloned() else { panic!() };
        let moved = app.event(Event::Moved { x: x + 5, y: y - 3 });
        assert!(moved.settings_changed);
        assert_eq!(app.settings.offset, (5, -3));
        app.event(Event::CenterClicked);
        assert_eq!(app.settings.offset, (0, 0));
    }

    #[test]
    fn next_and_previous_cycle_saved_crosshairs() {
        let mut app = app();
        for gap in [1, 2, 3] {
            app.settings.saved.push(crate::store::Saved { name: format!("{gap}"), crosshair: Crosshair { gap, ..Crosshair::default() } });
        }
        app.hotkey(Hotkey::Next);
        assert_eq!(app.settings.crosshair.gap, 1);
        app.hotkey(Hotkey::Previous);
        assert_eq!(app.settings.crosshair.gap, 3);
        app.hotkey(Hotkey::Next);
        assert_eq!(app.settings.crosshair.gap, 1);
    }

    #[test]
    fn unidentifiable_window_is_not_added() {
        let mut app = app();
        let nameless = WindowInfo { pid: 9, title: "Untitled".into(), ..WindowInfo::default() };
        assert!(!app.add_game(&nameless));
    }
}
