//! Aim buttons and hide keys from XInput2 raw events on the root window: the
//! same stream games read. Selected only while the app asks for something.

use super::super::{Event, MouseButton};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::{self, ConnectionExt as _, XIEventMask};
use x11rb::protocol::xproto::Window;
use x11rb::protocol::Event as XEvent;

const ALL_MASTER_DEVICES: u16 = 1;

pub struct Input {
    available: bool,
    mouse: Vec<MouseButton>,
    keys: Vec<String>,
    held_buttons: Vec<MouseButton>,
    held_keys: Vec<&'static str>,
}

impl Input {
    pub fn new(conn: &impl Connection) -> Input {
        // Raw events reach a root-window listener without a grab from 2.1 on.
        let available = conn
            .xinput_xi_query_version(2, 2)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|v| (v.major_version, v.minor_version) >= (2, 1));
        Input { available, mouse: Vec::new(), keys: Vec::new(), held_buttons: Vec::new(), held_keys: Vec::new() }
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
        self.held_keys.retain(|name| keys.iter().any(|key| key == name));
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

    /// A key by its X keycode, which is the evdev code plus 8: a position on
    /// the keyboard, the same on every layout, as the settings window binds it.
    fn key(&mut self, keycode: u32, down: bool) -> Option<Event> {
        let evdev = u16::try_from(keycode.checked_sub(8)?).ok()?;
        let name = super::super::KEYS.iter().find(|key| key.2 == evdev)?.0;
        if !self.keys.iter().any(|key| key == name) || self.held_keys.contains(&name) == down {
            return None;
        }
        if down {
            self.held_keys.push(name);
        } else {
            self.held_keys.retain(|held| *held != name);
        }
        Some(Event::Key { code: name.to_string(), down })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_changes_only_and_keys_by_position() {
        let mut input = Input {
            available: false,
            mouse: vec![MouseButton::Right],
            keys: vec!["ShiftLeft".into(), "AltRight".into()],
            held_buttons: Vec::new(),
            held_keys: Vec::new(),
        };
        assert_eq!(input.button(3, true), Some(Event::Mouse { button: MouseButton::Right, down: true }));
        assert_eq!(input.button(3, true), None);
        assert_eq!(input.button(1, true), None);
        let shift = |down: bool| Some(Event::Key { code: "ShiftLeft".into(), down });
        assert_eq!(input.key(50, true), shift(true), "X keycode 50 is evdev 42, the left Shift");
        assert_eq!(input.key(50, true), None, "a repeat");
        assert_eq!(input.key(62, true), None, "the right Shift is another key");
        assert_eq!(input.key(50, false), shift(false));
        assert!(input.key(108, true).is_some(), "the right Alt, AltGr on many layouts, by its position");
        assert_eq!(input.key(3, true), None, "below the keycode range");
    }
}
