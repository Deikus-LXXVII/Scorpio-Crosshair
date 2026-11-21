//! What the overlay should be doing, decided from what happened. Pure: it
//! takes events and returns commands for the platform thread, so every rule
//! here is tested without a screen.

use crate::core::center::{corner, twice_middle};
use crate::core::crosshair::{draw, to_svg, Crosshair};
use crate::core::focus::{self, FocusState, WindowInfo};
use crate::core::launchers::{game_at, InstalledGame};
use crate::core::raster::{rasterize, slab};
use crate::picture::Picture;
use crate::platform::{Command, Event, MouseButton};
use crate::store::{AimMode, Game, Image, Settings};
use std::collections::HashMap;
use std::sync::Arc;

/// The settings window's title; Scorpio's other windows (the tray's) never
/// count as being in front.
pub const SETTINGS_TITLE: &str = "Scorpio";

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

impl Display {
    fn from_rect([x, y, width, height]: [i32; 4]) -> Option<Display> {
        (width > 0 && height > 0).then_some(Display { x, y, width: width as u32, height: height as u32 })
    }

    /// Its middle, doubled so an odd width loses nothing.
    fn twice_middle(&self) -> (i64, i64) {
        (twice_middle(self.x, self.width), twice_middle(self.y, self.height))
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

/// What is drawn: a CS2 crosshair, or one of the player's images.
#[derive(Clone, Debug, PartialEq)]
enum Look {
    Cs2(Crosshair),
    Image { rendered: String, opacity: u8 },
}

/// What the overlay currently shows, to send a command only on a change.
/// It keeps the middle it is centred on, not the area, so a window resized
/// around the same middle sends nothing.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    Hidden,
    Crosshair {
        look: Look,
        /// The height the CS2 sizes scale with: the monitor's, or the game
        /// window's when centring on it.
        height: u32,
        middle: (i64, i64),
        offset: (i32, i32),
        unlocked: bool,
    },
}

pub struct App {
    pub settings: Settings,
    pub installed: Vec<InstalledGame>,
    /// Decoded images by file name, the ones the settings use.
    pub pictures: HashMap<String, Arc<Picture>>,
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
    /// Width and height of the bitmap last shown.
    shown_size: (u32, u32),
    listening: Option<Vec<MouseButton>>,
    following: Option<bool>,
    /// The window in front changed since the overlay was last raised.
    raise: bool,
    capture_hidden: Option<bool>,
    saved_index: Option<usize>,
}

impl App {
    pub fn new(settings: Settings, own_pid: u32) -> App {
        App {
            settings,
            installed: Vec::new(),
            pictures: HashMap::new(),
            own_pid,
            display: Display::default(),
            focus: FocusState { visible: false, game: None },
            front: None,
            last_outside: None,
            locked: true,
            hidden: false,
            aiming: false,
            shown: Shown::Hidden,
            shown_size: (0, 0),
            listening: None,
            following: None,
            raise: false,
            capture_hidden: None,
            saved_index: None,
        }
    }

    /// The game in front whose own crosshair replaces the main one.
    fn own_game(&self) -> Option<usize> {
        self.focus.game.filter(|index| self.settings.games.get(*index).is_some_and(|game| game.crosshair.is_some()))
    }

    /// The crosshair for what is in front: the game's own, else the main one.
    pub fn active_crosshair(&self) -> &Crosshair {
        match self.own_game() {
            Some(index) => self.settings.games[index].crosshair.as_ref().expect("own_game checked it"),
            None => &self.settings.crosshair,
        }
    }

    fn active_crosshair_mut(&mut self) -> &mut Crosshair {
        match self.own_game() {
            Some(index) => self.settings.games[index].crosshair.as_mut().expect("own_game checked it"),
            None => &mut self.settings.crosshair,
        }
    }

    /// The image settings that go with the active crosshair.
    pub fn active_image(&self) -> &Image {
        match self.own_game() {
            Some(index) => &self.settings.games[index].image,
            None => &self.settings.image,
        }
    }

    fn active_image_mut(&mut self) -> &mut Image {
        match self.own_game() {
            Some(index) => &mut self.settings.games[index].image,
            None => &mut self.settings.image,
        }
    }

    /// The image to draw instead of the CS2 crosshair: switched on, and its
    /// file read. A missing file falls back to the CS2 crosshair.
    fn active_picture(&self) -> Option<(&Image, &Arc<Picture>)> {
        let image = self.active_image();
        image.on.then(|| self.pictures.get(&image.rendered)).flatten().map(|picture| (image, picture))
    }

    fn detected(&self, window: &WindowInfo) -> bool {
        let detect = &self.settings.detect;
        (detect.fullscreen && focus::fullscreen_key(window).is_some())
            || (detect.launchers && !window.path.is_empty() && game_at(&window.path, &self.installed).is_some())
    }

    fn refocus(&mut self) {
        let saved: Vec<_> = self.settings.games.iter().map(|game| game.fingerprint.clone()).collect();
        let hidden = FocusState { visible: false, game: None };
        let next = match self.front.clone() {
            // The settings window shows the crosshair so it can be tuned, for
            // the game last in front, worked out again so a removed game
            // never leaves a stale index behind.
            Some(window) if window.pid == self.own_pid && window.title == SETTINGS_TITLE => {
                let behind = self.last_outside.clone();
                let base = focus::next_state(&hidden, behind.as_ref(), self.own_pid, &saved, |w| self.detected(w));
                FocusState { visible: true, game: base.game }
            }
            // The tray menu and other windows of Scorpio's own change nothing.
            Some(window) if window.pid == self.own_pid => return,
            front => focus::next_state(&self.focus, front.as_ref(), self.own_pid, &saved, |w| self.detected(w)),
        };
        if next.visible != self.focus.visible || next.game != self.focus.game {
            log::debug!(
                "focus: {} ({})",
                if next.visible { "shown" } else { "hidden" },
                match (&next.game, self.front.as_ref()) {
                    (Some(index), _) => format!("added game {index}"),
                    (None, Some(w)) if next.visible => format!("detected: {} class {} fullscreen {}", w.name, w.window_class, w.fullscreen),
                    (None, Some(w)) => format!("not a game: {} class {}", w.name, w.window_class),
                    (None, None) => "nothing in front".into(),
                }
            );
        }
        // Aiming starts over when the crosshair goes away or the game changes.
        if !(next.visible || self.settings.detect.always) || next.game != self.focus.game {
            self.aiming = false;
        }
        self.focus = next;
    }

    /// Over a game, or over anything with "Always show" on.
    fn over_something(&self) -> bool {
        self.focus.visible || self.settings.detect.always
    }

    fn should_show(&self) -> bool {
        !self.locked || (self.over_something() && !self.hidden && !(self.settings.aim.hide && self.aiming))
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
                self.raise = true;
                self.refocus();
                effects.status_changed = true;
            }
            Event::Geometry { pid, client, monitor, fullscreen } => {
                let mut refocus = false;
                for window in [self.front.as_mut(), self.last_outside.as_mut()].into_iter().flatten() {
                    // pid 0 (a window that does not say) could be any window.
                    if window.pid == pid && pid != 0 && pid != self.own_pid {
                        refocus |= window.fullscreen != fullscreen;
                        window.client = client;
                        window.monitor = monitor.or(window.monitor);
                        window.fullscreen = fullscreen;
                    }
                }
                // A game that went borderless fullscreen after it came to the front.
                if refocus {
                    self.refocus();
                    effects.status_changed = true;
                }
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
                // A drag cut short by locking reports the slab's corner; the
                // crosshair has already gone back to its own size.
                if !self.locked {
                    let (width, height) = self.shown_size;
                    let middle = self.target_area().twice_middle();
                    self.settings.offset = (x - corner(middle.0, width), y - corner(middle.1, height));
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
                    // A saved crosshair is a CS2 one: choosing it shows it.
                    self.active_image_mut().on = false;
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
        self.settings.games.push(Game { name, fingerprint: focus::fingerprint(window), ..Game::default() });
        self.settings = std::mem::take(&mut self.settings).validated();
        self.refocus();
        true
    }

    /// New settings from the settings window.
    pub fn replace_settings(&mut self, settings: Settings) -> Effects {
        let mut effects = Effects { settings_changed: true, ..Effects::default() };
        self.settings = settings.validated();
        // Decoded images no setting names any more (a replaced image, an old
        // size) are dropped; their files go at the next start.
        let used: Vec<String> = self.settings.image_files().into_iter().map(String::from).collect();
        self.pictures.retain(|name, _| used.contains(name));
        self.saved_index = None;
        self.refocus();
        self.sync(&mut effects);
        effects
    }

    /// A decoded image became available (read at start, or just made).
    pub fn add_picture(&mut self, name: String, picture: Arc<Picture>) -> Effects {
        let mut effects = Effects::default();
        self.pictures.insert(name, picture);
        self.sync(&mut effects);
        effects
    }

    /// Image files the settings use that are not decoded yet.
    pub fn missing_pictures(&self) -> Vec<String> {
        let mut names: Vec<String> = std::iter::once(&self.settings.image)
            .chain(self.settings.games.iter().map(|game| &game.image))
            .map(|image| image.rendered.clone())
            .filter(|name| !name.is_empty() && !self.pictures.contains_key(name))
            .collect();
        names.dedup();
        names
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

    /// The window the crosshair goes with: the one in front, the one behind
    /// the settings window, or with nothing in front (the desktop) the last
    /// one, so "Always show" stays on the monitor last used.
    fn placement_window(&self) -> Option<&WindowInfo> {
        match &self.front {
            Some(window) if window.pid != self.own_pid => Some(window),
            _ => self.last_outside.as_ref(),
        }
    }

    /// The monitor of the game (or of the window behind the settings), else
    /// the primary display.
    pub fn target_display(&self) -> Display {
        self.placement_window().and_then(|window| window.monitor).and_then(Display::from_rect).unwrap_or(self.display)
    }

    /// What the crosshair is centred on: the game window's client area when
    /// "Centre on the game window" is on and a game is in front, else the
    /// monitor.
    pub fn target_area(&self) -> Display {
        if self.settings.centre_on_window && self.focus.visible {
            if let Some(client) = self.placement_window().and_then(|window| window.client).and_then(Display::from_rect) {
                return client;
            }
        }
        self.target_display()
    }

    /// The unlocked slab is square and keeps the crosshair's parity, so the
    /// crosshair sits on the same pixels locked and unlocked.
    fn slab_size(size: u32) -> u32 {
        let mut slab = (size + 2 * SLAB_PAD).max(SLAB_MIN);
        if (slab - size) % 2 == 1 {
            slab += 1;
        }
        slab
    }

    fn wanted(&self) -> Shown {
        if !self.should_show() {
            return Shown::Hidden;
        }
        let area = self.target_area();
        let look = match self.active_picture() {
            Some((image, _)) => Look::Image { rendered: image.rendered.clone(), opacity: image.opacity },
            None => Look::Cs2(self.active_crosshair().clone()),
        };
        Shown::Crosshair { look, height: area.height, middle: area.twice_middle(), offset: self.settings.offset, unlocked: !self.locked }
    }

    /// Sends what changed since the last step.
    fn sync(&mut self, effects: &mut Effects) {
        let want = self.wanted();
        // A Show raises the overlay too; otherwise raise it when the window
        // in front changed and the crosshair stays.
        let raise = std::mem::take(&mut self.raise);
        match (&self.shown, &want) {
            (old, new) if old == new => {
                if raise && *old != Shown::Hidden {
                    effects.commands.push(Command::Raise);
                }
            }
            // Only the place changed (a window dragged, the offset set): the
            // same bitmap moves, nothing is drawn again.
            (
                Shown::Crosshair { look, height, unlocked, .. },
                Shown::Crosshair { look: new_look, height: new_height, unlocked: new_unlocked, middle, offset },
            ) if look == new_look && height == new_height && unlocked == new_unlocked => {
                let (x, y) = self.place(*middle, *offset);
                effects.commands.push(Command::Move { x, y });
                if raise {
                    effects.commands.push(Command::Raise);
                }
                self.shown = want;
            }
            _ => {
                let command = self.render(&want);
                if let Command::Show { bitmap, .. } = &command {
                    self.shown_size = (bitmap.width, bitmap.height);
                    log::debug!("overlay: show {}x{}", bitmap.width, bitmap.height);
                } else {
                    log::debug!("overlay: hide");
                }
                effects.commands.push(command);
                self.shown = want;
            }
        }

        if self.capture_hidden != Some(self.settings.hide_from_capture) {
            effects.commands.push(Command::ExcludeFromCapture(self.settings.hide_from_capture));
            self.capture_hidden = Some(self.settings.hide_from_capture);
        }

        let listen = if self.settings.aim.hide && self.over_something() && self.locked { vec![self.settings.aim.button] } else { Vec::new() };
        if self.listening.as_ref() != Some(&listen) {
            effects.commands.push(Command::ListenTo { mouse: listen.clone(), keys: Vec::new() });
            self.listening = Some(listen);
        }

        // The game window is followed only while it matters: centring on it,
        // with a game in front.
        let follow = self.settings.centre_on_window && self.focus.visible;
        if self.following != Some(follow) {
            effects.commands.push(Command::Follow(follow));
            self.following = Some(follow);
        }
    }

    /// The overlay's top-left corner for a bitmap of the shown size.
    fn place(&self, middle: (i64, i64), offset: (i32, i32)) -> (i32, i32) {
        let (width, height) = self.shown_size;
        (corner(middle.0, width) + offset.0, corner(middle.1, height) + offset.1)
    }

    fn render(&mut self, shown: &Shown) -> Command {
        let Shown::Crosshair { look, height, middle, offset, unlocked } = shown else {
            return Command::Hide;
        };
        let bitmap = match look {
            Look::Image { rendered, opacity } => self.pictures[rendered].bitmap(*opacity),
            Look::Cs2(crosshair) => rasterize(&draw(crosshair, *height), 1.0),
        };
        let (bitmap, button) = if *unlocked {
            let size = App::slab_size(bitmap.width.max(bitmap.height));
            let (bitmap, button) = slab(&bitmap, size, size);
            (bitmap, Some(button))
        } else {
            (bitmap, None)
        };
        self.shown_size = (bitmap.width, bitmap.height);
        let (x, y) = self.place(*middle, *offset);
        Command::Show { bitmap, x, y, unlocked: *unlocked, button }
    }

    /// What a stream should show, for the OBS page: the crosshair as drawn
    /// on the game's screen (never the unlocked slab), or nothing. An image
    /// is fetched by its address, so the events stay small.
    pub fn stream_view(&self) -> StreamView {
        let hidden = StreamView { visible: false, svg: String::new(), image: None, offset: [0, 0], height: self.target_area().height };
        let Shown::Crosshair { look, height, offset, .. } = &self.shown else { return hidden };
        let mut view = StreamView { visible: true, svg: String::new(), image: None, offset: [offset.0, offset.1], height: *height };
        match look {
            Look::Cs2(crosshair) => view.svg = to_svg(&draw(crosshair, *height)),
            Look::Image { rendered, opacity } => {
                let Some(picture) = self.pictures.get(rendered) else { return hidden };
                // The page shows the PNG as stored, without the overlay's
                // padding: half the padding moves it to the same middle.
                let (width, height) = picture.stored_size();
                view.image = Some(StreamImage {
                    url: format!("/image/{rendered}"),
                    width,
                    height,
                    nudge: [(picture.width - width) as f32 / 2.0, (picture.height - height) as f32 / 2.0],
                    opacity: f32::from(*opacity) / 255.0,
                });
            }
        }
        view
    }

    /// The PNG the stream view points at, for the OBS server.
    pub fn stream_png(&self) -> Option<(String, Arc<Vec<u8>>)> {
        match &self.shown {
            Shown::Crosshair { look: Look::Image { rendered, .. }, .. } => self.pictures.get(rendered).map(|picture| (rendered.clone(), Arc::clone(&picture.png))),
            _ => None,
        }
    }

    /// Plain words for the settings window's status line.
    pub fn status(&self) -> Status {
        let game = self.focus.game.and_then(|index| self.settings.games.get(index)).map(|game| game.name.clone());
        let front = self.front.as_ref().filter(|window| window.pid != self.own_pid).cloned().or_else(|| self.last_outside.clone());
        Status { visible: self.focus.visible, always: self.settings.detect.always, game, front, locked: self.locked, hidden: self.hidden }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct StreamImage {
    pub url: String,
    /// The PNG's own size.
    pub width: u32,
    pub height: u32,
    /// How far its middle sits right of and below the overlay's, in pixels.
    pub nudge: [f32; 2],
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct StreamView {
    pub visible: bool,
    pub svg: String,
    pub image: Option<StreamImage>,
    pub offset: [i32; 2],
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub visible: bool,
    /// "Always show" is on.
    pub always: bool,
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
            monitor: None,
            client: None,
        }
    }

    fn app() -> App {
        let mut settings = Settings::default();
        settings.detect.fullscreen = false;
        settings.detect.launchers = false;
        settings.games.push(Game { name: "Game".into(), fingerprint: focus::fingerprint(&window(1, "game", false)), ..Game::default() });
        let mut app = App::new(settings, OWN);
        app.capture_hidden = Some(false);
        app.listening = Some(Vec::new());
        app.following = Some(false);
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
        assert_eq!(app.event(Event::Foreground(Some(window(1, "game", false)))).commands, vec![Command::Raise], "only raised again");
    }

    #[test]
    fn own_window_keeps_the_crosshair_and_add_game_ignores_it() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(1, "game", false))));
        let effects = app.event(Event::Foreground(Some(WindowInfo { title: SETTINGS_TITLE.into(), ..window(OWN, "scorpio", false) })));
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

    #[test]
    fn the_desktop_hides_and_closing_settings_over_it_stays_hidden() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(1, "game", false))));
        assert_eq!(shows(&app.event(Event::Foreground(None))), Some(false));
        let settings = WindowInfo { title: SETTINGS_TITLE.into(), ..window(OWN, "scorpio", false) };
        assert_eq!(shows(&app.event(Event::Foreground(Some(settings)))), Some(true));
        assert_eq!(shows(&app.event(Event::Foreground(None))), Some(false));
    }

    #[test]
    fn the_tray_menu_changes_nothing() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(2, "browser", false))));
        let tray = WindowInfo { title: String::new(), ..window(OWN, "scorpio", false) };
        assert_eq!(shows(&app.event(Event::Foreground(Some(tray)))), None);
        assert!(!app.focus.visible);
    }

    #[test]
    fn removing_a_game_while_tuning_keeps_the_right_game() {
        let mut app = app();
        for name in ["b", "c"] {
            app.settings.games.push(Game { name: name.into(), fingerprint: focus::fingerprint(&window(5, name, false)), ..Game::default() });
        }
        app.event(Event::Foreground(Some(window(5, "b", false))));
        assert_eq!(app.focus.game, Some(1));
        app.event(Event::Foreground(Some(WindowInfo { title: SETTINGS_TITLE.into(), ..window(OWN, "scorpio", false) })));
        let mut settings = app.settings.clone();
        settings.games.remove(0);
        app.replace_settings(settings);
        assert_eq!(app.focus.game, Some(0), "b moved to index 0");
        assert_eq!(app.settings.games[0].name, "b");
    }

    #[test]
    fn aiming_resets_when_the_crosshair_goes_away() {
        let mut app = app();
        app.settings.aim.hide = true;
        app.settings.detect.fullscreen = true;
        app.event(Event::Foreground(Some(window(4, "x", true))));
        app.event(Event::Mouse { button: MouseButton::Right, down: true });
        app.event(Event::Foreground(Some(window(2, "browser", false))));
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(4, "x", true))))), Some(true));
    }

    #[test]
    fn a_move_after_locking_is_ignored() {
        let mut app = app();
        app.event(Event::Foreground(Some(window(1, "game", false))));
        app.hotkey(Hotkey::Lock);
        app.hotkey(Hotkey::Lock);
        assert!(!app.event(Event::Moved { x: 1, y: 1 }).settings_changed);
        assert_eq!(app.settings.offset, (0, 0));
    }

    #[test]
    fn the_crosshair_is_centred_on_the_games_monitor() {
        let mut app = app();
        app.set_display(Display { x: 0, y: 0, width: 1920, height: 1080 });
        let on_second = WindowInfo { monitor: Some([1920, 0, 2560, 1440]), ..window(1, "game", false) };
        let effects = app.event(Event::Foreground(Some(on_second)));
        let Some(Command::Show { bitmap, x, y, .. }) = effects.commands.first() else { panic!() };
        assert_eq!(2 * x + bitmap.width as i32, 2 * 1920 + 2560);
        assert_eq!(2 * y + bitmap.height as i32, 1440);
        let settings = WindowInfo { title: SETTINGS_TITLE.into(), monitor: Some([0, 0, 1920, 1080]), ..window(OWN, "scorpio", false) };
        assert!(app.event(Event::Foreground(Some(settings))).commands.iter().all(|c| !matches!(c, Command::Show { .. })), "tuning keeps it on the game's monitor");
    }

    #[test]
    fn the_stream_follows_the_overlay() {
        let mut app = app();
        assert!(!app.stream_view().visible);
        app.event(Event::Foreground(Some(window(1, "game", false))));
        let view = app.stream_view();
        assert!(view.visible && view.svg.starts_with("<svg"));
        app.hotkey(Hotkey::Hide);
        assert!(!app.stream_view().visible);
    }

    fn shown_at(effects: &Effects) -> Option<(i32, i32, u32, u32)> {
        effects.commands.iter().find_map(|command| match command {
            Command::Show { bitmap, x, y, .. } => Some((*x, *y, bitmap.width, bitmap.height)),
            _ => None,
        })
    }

    #[test]
    fn always_show_keeps_it_over_any_window_and_the_desktop() {
        let mut app = app();
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(2, "browser", false))))), None, "hidden, as it was");
        let mut settings = app.settings.clone();
        settings.detect.always = true;
        assert_eq!(shows(&app.replace_settings(settings)), Some(true));
        assert!(!app.focus.visible, "a browser is still not a game");
        assert!(app.status().always);
        assert_eq!(shows(&app.event(Event::Foreground(None))), None, "the desktop keeps it");
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(1, "game", false))))), None, "a game keeps it");
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(2, "browser", false))))), None);
    }

    #[test]
    fn always_show_still_hides_by_shortcut_and_while_aiming() {
        let mut app = app();
        app.settings.detect.always = true;
        app.settings.aim.hide = true;
        let effects = app.event(Event::Foreground(Some(window(2, "browser", false))));
        assert_eq!(shows(&effects), Some(true));
        assert!(effects.commands.contains(&Command::ListenTo { mouse: vec![MouseButton::Right], keys: vec![] }), "the aim button is read outside games too");
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: true })), Some(false));
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: false })), Some(true));
        assert_eq!(shows(&app.hotkey(Hotkey::Hide)), Some(false));
        assert_eq!(shows(&app.event(Event::Foreground(None))), None, "still hidden by the shortcut");
        assert_eq!(shows(&app.hotkey(Hotkey::Hide)), Some(true));
    }

    #[test]
    fn always_show_uses_the_monitor_in_front_then_the_last_then_the_primary() {
        let mut app = app();
        app.settings.detect.always = true;
        let first = shown_at(&app.set_display(Display { x: 0, y: 0, width: 1920, height: 1080 })).expect("shown on the primary display");
        assert_eq!(2 * first.0 + first.2 as i32, 1920);
        let second = WindowInfo { monitor: Some([1920, 0, 2560, 1440]), ..window(2, "browser", false) };
        app.event(Event::Foreground(Some(second)));
        let on_second = app.event(Event::Foreground(None));
        assert!(on_second.commands.iter().all(|c| !matches!(c, Command::Show { .. } | Command::Move { .. })), "the desktop keeps the last monitor");
        let (x, y) = app.place(app.target_area().twice_middle(), (0, 0));
        assert_eq!((2 * x + app.shown_size.0 as i32, 2 * y + app.shown_size.1 as i32), (2 * 1920 + 2560, 1440));
    }

    fn with_picture(app: &mut App, name: &str, width: u32, height: u32) {
        app.add_picture(name.into(), Arc::new(crate::picture::solid(width, height, [255, 255, 255, 255])));
    }

    #[test]
    fn an_image_replaces_the_cs2_crosshair_exactly_centred() {
        let mut app = app();
        with_picture(&mut app, "0123456789abcdef.png", 40, 24);
        app.event(Event::Foreground(Some(window(1, "game", false))));
        let mut settings = app.settings.clone();
        settings.image = Image { on: true, rendered: "0123456789abcdef.png".into(), opacity: 128, ..Image::default() };
        let effects = app.replace_settings(settings);
        let Some(Command::Show { bitmap, x, y, .. }) = effects.commands.first() else { panic!("{effects:?}") };
        assert_eq!((bitmap.width, bitmap.height), (40, 24));
        assert_eq!((2 * x + 40, 2 * y + 24), (1920, 1080), "the image's middle is the display's");
        assert_eq!(&bitmap.data[0..4], &[128, 128, 128, 128], "opacity applied, premultiplied");

        let mut settings = app.settings.clone();
        settings.image.opacity = 255;
        assert_eq!(shows(&app.replace_settings(settings)), Some(true), "a new opacity draws it again");

        let unlocked = app.hotkey(Hotkey::Lock);
        let Some(Command::Show { bitmap, x, y, .. }) = unlocked.commands.first() else { panic!() };
        assert_eq!((2 * x + bitmap.width as i32, 2 * y + bitmap.height as i32), (1920, 1080), "the slab keeps the middle");
        app.hotkey(Hotkey::Lock);

        let mut settings = app.settings.clone();
        settings.image.on = false;
        let effects = app.replace_settings(settings);
        let Some(Command::Show { bitmap, .. }) = effects.commands.first() else { panic!() };
        assert_ne!((bitmap.width, bitmap.height), (40, 24), "back to the CS2 crosshair");
    }

    #[test]
    fn an_image_not_read_yet_shows_the_cs2_crosshair_until_it_is() {
        let mut app = app();
        app.settings.image = Image { on: true, rendered: "0123456789abcdef.png".into(), ..Image::default() };
        assert_eq!(app.missing_pictures(), vec!["0123456789abcdef.png".to_string()]);
        app.event(Event::Foreground(Some(window(1, "game", false))));
        assert!(app.stream_view().svg.starts_with("<svg"));
        let effects = app.add_picture("0123456789abcdef.png".into(), Arc::new(crate::picture::solid(8, 8, [1, 2, 3, 255])));
        assert_eq!(shown_at(&effects).map(|s| (s.2, s.3)), Some((8, 8)));
        assert!(app.missing_pictures().is_empty());
        let view = app.stream_view();
        assert!(view.visible && view.svg.is_empty());
        assert_eq!(view.image.map(|i| (i.url, i.width, i.nudge)), Some(("/image/0123456789abcdef.png".into(), 8, [0.0, 0.0])));
        assert_eq!(app.stream_png().map(|(name, _)| name), Some("0123456789abcdef.png".into()));
    }

    #[test]
    fn a_games_own_crosshair_brings_its_own_image() {
        let mut app = app();
        with_picture(&mut app, "aaaaaaaaaaaaaaaa.png", 16, 16);
        with_picture(&mut app, "bbbbbbbbbbbbbbbb.png", 32, 32);
        app.settings.image = Image { on: true, rendered: "aaaaaaaaaaaaaaaa.png".into(), ..Image::default() };
        app.settings.games[0].image = Image { on: true, rendered: "bbbbbbbbbbbbbbbb.png".into(), ..Image::default() };
        // Without its own crosshair the game shows the main image.
        let main = app.event(Event::Foreground(Some(window(1, "game", false))));
        assert_eq!(shown_at(&main).map(|s| s.2), Some(16));
        let mut settings = app.settings.clone();
        settings.games[0].crosshair = Some(Crosshair::default());
        assert_eq!(shown_at(&app.replace_settings(settings)).map(|s| s.2), Some(32));
        app.settings.saved.push(crate::store::Saved::default());
        let next = app.hotkey(Hotkey::Next);
        assert!(!app.settings.games[0].image.on && app.settings.image.on, "a saved crosshair turns only the game's image off");
        assert_ne!(shown_at(&next).map(|s| s.2), Some(32));
    }

    fn windowed(client: [i32; 4]) -> WindowInfo {
        WindowInfo { monitor: Some([0, 0, 2560, 1440]), client: Some(client), ..window(1, "game", false) }
    }

    #[test]
    fn centring_on_the_game_window_uses_its_client_area_and_follows_it() {
        let mut app = app();
        app.settings.centre_on_window = true;
        let effects = app.event(Event::Foreground(Some(windowed([100, 50, 1281, 720]))));
        assert!(effects.commands.contains(&Command::Follow(true)));
        let (x, y, width, height) = shown_at(&effects).expect("shown");
        // Odd width: the centre pixel is 100 + 640, right of the bitmap's middle.
        assert_eq!(x + width as i32 / 2, 740);
        assert_eq!(2 * y + height as i32, 2 * 50 + 720);
        let scaled = draw(app.active_crosshair(), 720).size;
        assert_eq!(width, scaled, "CS2 sizes scale with the window's height, as in the game");

        // Dragged: only a move, no new bitmap.
        let moved = app.event(Event::Geometry { pid: 1, client: Some([300, 80, 1281, 720]), monitor: Some([0, 0, 2560, 1440]), fullscreen: false });
        assert_eq!(moved.commands, vec![Command::Move { x: x + 200, y: y + 30 }]);
        // The same middle again (a burst during a drag): nothing at all.
        assert!(app.event(Event::Geometry { pid: 1, client: Some([300, 80, 1281, 720]), monitor: None, fullscreen: false }).commands.is_empty());
        // Wider by two around the same middle, same height: nothing either.
        assert!(app.event(Event::Geometry { pid: 1, client: Some([299, 80, 1283, 720]), monitor: None, fullscreen: false }).commands.is_empty());
        // Another window's geometry is ignored.
        assert!(app.event(Event::Geometry { pid: 9, client: Some([0, 0, 640, 480]), monitor: None, fullscreen: false }).commands.is_empty());
        // A new height draws it again at the new scale.
        assert_eq!(shows(&app.event(Event::Geometry { pid: 1, client: Some([300, 80, 1920, 1080]), monitor: None, fullscreen: false })), Some(true));

        // Off: back on the monitor, and the window is no longer followed.
        let mut settings = app.settings.clone();
        settings.centre_on_window = false;
        let effects = app.replace_settings(settings);
        assert!(effects.commands.contains(&Command::Follow(false)));
        let (x, _, width, _) = shown_at(&effects).expect("drawn at the monitor's height");
        assert_eq!(2 * x + width as i32, 2560);
    }

    #[test]
    fn centring_on_the_window_keeps_the_game_while_tuning_and_ignores_non_games() {
        let mut app = app();
        app.settings.centre_on_window = true;
        app.settings.detect.always = true;
        app.event(Event::Foreground(Some(windowed([100, 100, 800, 600]))));
        let settings_window = WindowInfo { title: SETTINGS_TITLE.into(), ..window(OWN, "scorpio", false) };
        let effects = app.event(Event::Foreground(Some(settings_window)));
        assert!(effects.commands.iter().all(|c| !matches!(c, Command::Show { .. } | Command::Move { .. })), "stays on the game window");
        // The game moves behind the settings window: followed.
        let moved = app.event(Event::Geometry { pid: 1, client: Some([200, 100, 800, 600]), monitor: None, fullscreen: false });
        assert!(matches!(moved.commands.as_slice(), [Command::Move { .. }]));
        // A browser is not a game: "Always show" centres on its monitor, not on it.
        let browser = WindowInfo { monitor: Some([0, 0, 2560, 1440]), client: Some([0, 0, 500, 500]), ..window(2, "browser", false) };
        let effects = app.event(Event::Foreground(Some(browser)));
        assert!(effects.commands.contains(&Command::Follow(false)));
        let (x, _, width, _) = shown_at(&effects).expect("drawn again at the monitor's height");
        assert_eq!(2 * x + width as i32, 2560);
    }

    #[test]
    fn a_window_that_turns_fullscreen_in_front_is_seen() {
        let mut app = app();
        app.settings.detect.fullscreen = true;
        app.settings.centre_on_window = true;
        let unknown = WindowInfo { client: Some([0, 0, 800, 600]), ..window(4, "unknown", false) };
        app.event(Event::Foreground(Some(unknown)));
        assert!(!app.focus.visible);
        let effects = app.event(Event::Geometry { pid: 4, client: Some([0, 0, 1920, 1080]), monitor: Some([0, 0, 1920, 1080]), fullscreen: true });
        assert!(effects.status_changed);
        assert_eq!(shows(&effects), Some(true));
    }

    #[test]
    fn with_always_show_a_toggled_aim_survives_switching_between_other_windows() {
        let mut app = app();
        app.settings.detect.always = true;
        app.settings.aim = crate::store::Aim { hide: true, button: MouseButton::Right, mode: AimMode::Toggle };
        app.event(Event::Foreground(Some(window(2, "browser", false))));
        assert_eq!(shows(&app.event(Event::Mouse { button: MouseButton::Right, down: true })), Some(false));
        assert_eq!(shows(&app.event(Event::Foreground(Some(window(3, "editor", false))))), None, "still aiming");
    }

    #[test]
    fn a_window_without_a_pid_never_moves_the_crosshair() {
        let mut app = app();
        app.settings.centre_on_window = true;
        app.settings.games[0].fingerprint.name = "game".into();
        app.event(Event::Foreground(Some(WindowInfo { pid: 0, ..windowed([0, 0, 800, 600]) })));
        assert!(app.event(Event::Geometry { pid: 0, client: Some([500, 0, 800, 600]), monitor: None, fullscreen: false }).commands.is_empty());
    }

    #[test]
    fn images_no_setting_names_are_let_go() {
        let mut app = app();
        with_picture(&mut app, "aaaaaaaaaaaaaaaa.png", 8, 8);
        with_picture(&mut app, "bbbbbbbbbbbbbbbb.png", 8, 8);
        let mut settings = app.settings.clone();
        settings.image = Image { on: true, rendered: "bbbbbbbbbbbbbbbb.png".into(), ..Image::default() };
        app.replace_settings(settings);
        assert_eq!(app.pictures.keys().collect::<Vec<_>>(), vec!["bbbbbbbbbbbbbbbb.png"]);
    }
}
