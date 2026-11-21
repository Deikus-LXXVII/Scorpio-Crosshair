//! Aim buttons and hide keys from XInput2 raw events on the root window: the
//! same stream games read. Selected only while the app asks for something.

use super::super::{Event, MouseButton};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::{self, ConnectionExt as _, XIEventMask};
use x11rb::protocol::xproto::{ConnectionExt as _, Keycode, Keysym, Window};
use x11rb::protocol::Event as XEvent;

const ALL_MASTER_DEVICES: u16 = 1;

pub struct Input {
    available: bool,
    mouse: Vec<MouseButton>,
    keys: Vec<String>,
    held_buttons: Vec<MouseButton>,
    held_keys: Vec<(u32, String)>,
    min_keycode: Keycode,
    per_keycode: usize,
    keysyms: Vec<Keysym>,
}

impl Input {
    pub fn new(conn: &impl Connection) -> Input {
        // Raw events reach a root-window listener without a grab from 2.1 on.
        let available = conn
            .xinput_xi_query_version(2, 2)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|v| (v.major_version, v.minor_version) >= (2, 1));
        let mut input = Input {
            available,
            mouse: Vec::new(),
            keys: Vec::new(),
            held_buttons: Vec::new(),
            held_keys: Vec::new(),
            min_keycode: 0,
            per_keycode: 1,
            keysyms: Vec::new(),
        };
        input.load_keymap(conn);
        input
    }

    pub fn load_keymap(&mut self, conn: &impl Connection) {
        let setup = conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        if let Some(map) = conn.get_keyboard_mapping(min, max - min + 1).ok().and_then(|c| c.reply().ok()) {
            self.min_keycode = min;
            self.per_keycode = usize::from(map.keysyms_per_keycode.max(1));
            self.keysyms = map.keysyms;
        }
    }

    pub fn listen(&mut self, conn: &impl Connection, root: Window, mouse: Vec<MouseButton>, keys: Vec<String>) {
        let mut mask = XIEventMask::from(0u32);
        if !mouse.is_empty() {
            mask = mask | XIEventMask::RAW_BUTTON_PRESS | XIEventMask::RAW_BUTTON_RELEASE;
        }
        if !keys.is_empty() {
            mask = mask | XIEventMask::RAW_KEY_PRESS | XIEventMask::RAW_KEY_RELEASE;
        }
        self.held_buttons.retain(|b| mouse.contains(b));
        self.held_keys.retain(|(_, name)| keys.contains(name));
        self.mouse = mouse;
        self.keys = keys;
        if !self.available {
            if !(self.mouse.is_empty() && self.keys.is_empty()) {
                log::warn!("XInput 2.1 is missing: aim buttons and hide keys are not available");
            }
            return;
        }
        let masks = [xinput::EventMask { deviceid: ALL_MASTER_DEVICES, mask: vec![mask] }];
        let _ = conn.xinput_xi_select_events(root, &masks);
    }

    /// The change this event makes to what the app listens to, if any.
    pub fn handle(&mut self, event: &XEvent) -> Option<Event> {
        match event {
            XEvent::XinputRawButtonPress(e) => self.button(e.detail, true),
            XEvent::XinputRawButtonRelease(e) => self.button(e.detail, false),
            XEvent::XinputRawKeyPress(e) => self.key(e.detail, true),
            XEvent::XinputRawKeyRelease(e) => self.key(e.detail, false),
            _ => None,
        }
    }

    fn button(&mut self, detail: u32, down: bool) -> Option<Event> {
        let button = match detail {
            1 => MouseButton::Left,
            2 => MouseButton::Middle,
            3 => MouseButton::Right,
            8 => MouseButton::Back,
            9 => MouseButton::Forward,
            _ => return None,
        };
        if !self.mouse.contains(&button) || self.held_buttons.contains(&button) == down {
            return None;
        }
        if down {
            self.held_buttons.push(button);
        } else {
            self.held_buttons.retain(|&b| b != button);
        }
        Some(Event::Mouse { button, down })
    }

    fn key(&mut self, keycode: u32, down: bool) -> Option<Event> {
        let index = (keycode.checked_sub(u32::from(self.min_keycode))? as usize) * self.per_keycode;
        let code = key_name(*self.keysyms.get(index)?)?;
        if !self.keys.contains(&code) {
            return None;
        }
        // Left and right Shift share a name: report the first press and the last release.
        let was_held = self.held_keys.iter().any(|(_, name)| *name == code);
        self.held_keys.retain(|&(k, _)| k != keycode);
        if down {
            self.held_keys.push((keycode, code.clone()));
        }
        let is_held = self.held_keys.iter().any(|(_, name)| *name == code);
        (was_held != is_held).then_some(Event::Key { code, down })
    }
}

/// Keysym -> the platform-neutral names keybinds use.
fn key_name(sym: Keysym) -> Option<String> {
    let name = match sym {
        0x30..=0x39 | 0x41..=0x5a => return Some(char::from(sym as u8).to_string()),
        0x61..=0x7a => return Some(char::from(sym as u8 - 0x20).to_string()),
        0xffbe..=0xffd5 => return Some(format!("F{}", sym - 0xffbe + 1)),
        0xffe1 | 0xffe2 => "Shift",
        0xffe3 | 0xffe4 => "Control",
        0xffe9 | 0xffea => "Alt",
        0xffeb | 0xffec => "Super",
        0x20 => "Space",
        0xff0d => "Enter",
        0xff1b => "Escape",
        0xff09 => "Tab",
        0xff08 => "Backspace",
        0xffe5 => "CapsLock",
        0xff51 => "Left",
        0xff52 => "Up",
        0xff53 => "Right",
        0xff54 => "Down",
        0xff63 => "Insert",
        0xffff => "Delete",
        0xff50 => "Home",
        0xff57 => "End",
        0xff55 => "PageUp",
        0xff56 => "PageDown",
        _ => return None,
    };
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(key_name(0x68).as_deref(), Some("H"));
        assert_eq!(key_name(0xffc2).as_deref(), Some("F5"));
        assert_eq!(key_name(0xffe2).as_deref(), Some("Shift"));
        assert_eq!(key_name(0x1234), None);
    }

    #[test]
    fn reports_changes_only() {
        let mut input = Input {
            available: false,
            mouse: vec![MouseButton::Right],
            keys: vec!["Shift".into()],
            held_buttons: Vec::new(),
            held_keys: Vec::new(),
            min_keycode: 8,
            per_keycode: 1,
            keysyms: vec![0xffe1, 0xffe2, 0x61],
        };
        assert_eq!(input.button(3, true), Some(Event::Mouse { button: MouseButton::Right, down: true }));
        assert_eq!(input.button(3, true), None);
        assert_eq!(input.button(1, true), None);
        assert!(input.key(8, true).is_some());
        assert_eq!(input.key(9, true), None);
        assert_eq!(input.key(8, false), None);
        assert_eq!(input.key(9, false), Some(Event::Key { code: "Shift".into(), down: false }));
        assert_eq!(input.key(10, true), None);
    }
}
