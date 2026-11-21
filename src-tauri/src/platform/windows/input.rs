//! Mouse buttons and keys from Raw Input.
//!
//! Raw Input is the documented way for a process to read devices itself; games
//! and voice chat use it. Nothing is installed in any other process, unlike
//! the low-level hooks (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`) anti-cheats look for,
//! which are banned here. `RIDEV_INPUTSINK` is what delivers the input while
//! the game, not Scorpio, has the focus. A device class is registered only
//! while an action needs it, so nothing is read otherwise.
//!
//! One process has one Raw Input target per device class. Tao (Tauri's window
//! library) registers its own when its event loop is created, so `ListenTo`
//! must be sent after that; Scorpio's registration then replaces Tao's.

use crate::platform::{Event, MouseButton};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK,
    RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
};

const USAGE_PAGE_GENERIC: u16 = 1;
const USAGE_MOUSE: u16 = 2;
const USAGE_KEYBOARD: u16 = 6;
const KEY_UP: u16 = 1; // RI_KEY_BREAK
const KEY_E0: u16 = 2; // RI_KEY_E0: the right-hand or navigation twin of a key
const KEY_E1: u16 = 4; // RI_KEY_E1: the first half of Pause, which reads as Control

/// (button, RI_MOUSE_*_DOWN flag, RI_MOUSE_*_UP flag)
const BUTTONS: [(MouseButton, u16, u16); 5] = [
    (MouseButton::Left, 0x001, 0x002),
    (MouseButton::Right, 0x004, 0x008),
    (MouseButton::Middle, 0x010, 0x020),
    (MouseButton::Back, 0x040, 0x080),
    (MouseButton::Forward, 0x100, 0x200),
];

#[derive(Default)]
pub struct Listener {
    mouse: Vec<MouseButton>,
    keys: Vec<String>,
    /// Keys now down: tells a held key's repeats from a new press.
    held: Vec<&'static str>,
    mouse_registered: bool,
    keyboard_registered: bool,
}

impl Listener {
    /// Replaces what is listened to; `sink` is the hidden window that receives WM_INPUT.
    pub fn listen(&mut self, sink: HWND, mouse: Vec<MouseButton>, keys: Vec<String>) {
        self.mouse = mouse;
        self.keys = keys;
        self.held.clear();
        let want_mouse = !self.mouse.is_empty();
        let want_keyboard = !self.keys.is_empty();
        register(sink, USAGE_MOUSE, want_mouse, &mut self.mouse_registered);
        register(sink, USAGE_KEYBOARD, want_keyboard, &mut self.keyboard_registered);
    }

    /// Reads one WM_INPUT message and reports what was asked for.
    pub fn read(&mut self, input: LPARAM, mut report: impl FnMut(Event)) {
        let mut raw = RAWINPUT::default();
        let mut size = size_of::<RAWINPUT>() as u32;
        // SAFETY: `raw` is a writable RAWINPUT of `size` bytes; mouse and keyboard
        // packets always fit, and anything larger fails here and is dropped.
        let read = unsafe {
            GetRawInputData(
                HRAWINPUT(input.0 as _),
                RID_INPUT,
                Some((&mut raw as *mut RAWINPUT).cast()),
                &mut size,
                size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if read == u32::MAX {
            return;
        }
        if raw.header.dwType == RIM_TYPEMOUSE.0 {
            // SAFETY: the header says this packet is the mouse member of the union.
            let flags = unsafe { raw.data.mouse.Anonymous.Anonymous.usButtonFlags };
            for (button, down_flag, up_flag) in BUTTONS {
                if flags & (down_flag | up_flag) != 0 && self.mouse.contains(&button) {
                    report(Event::Mouse { button, down: flags & down_flag != 0 });
                }
            }
        } else if raw.header.dwType == RIM_TYPEKEYBOARD.0 {
            // SAFETY: the header says this packet is the keyboard member of the union.
            let key = unsafe { raw.data.keyboard };
            let Some(scan) = scan_of(key.VKey, key.MakeCode, key.Flags) else { return };
            if let Some(event) = self.key(scan, key.Flags & KEY_UP == 0) {
                report(event);
            }
        }
    }

    /// A key at this scan code went down or up: an event when it is one
    /// listened to, and not a held key's repeat.
    fn key(&mut self, scan: u16, down: bool) -> Option<Event> {
        let name = code_of(scan).filter(|name| self.keys.iter().any(|wanted| wanted == name))?;
        if down == self.held.contains(&name) {
            return None;
        }
        if down {
            self.held.push(name);
        } else {
            self.held.retain(|held| *held != name);
        }
        Some(Event::Key { code: name.to_string(), down })
    }
}

/// The table's scan code for a Raw Input key packet. Raw Input gives
/// hardware codes: NumLock comes as 0x45, the code Pause has in the table's
/// (Chromium's) convention; Pause itself comes as an E1 packet and then one
/// with no key (0xFF), like the fake Shifts around PrintScreen. Neither Pause
/// nor those is a key here.
fn scan_of(vkey: u16, make: u16, flags: u16) -> Option<u16> {
    match vkey {
        0x90 => Some(0xE045),
        0xFF => None,
        _ if flags & KEY_E1 != 0 => None,
        _ => Some(make | if flags & KEY_E0 != 0 { 0xE000 } else { 0 }),
    }
}

/// The key at this scan code (0xE0.. for an extended key), by its position.
fn code_of(scan: u16) -> Option<&'static str> {
    crate::platform::KEYS.iter().find(|key| key.1 == scan).map(|key| key.0)
}

fn register(sink: HWND, usage: u16, wanted: bool, registered: &mut bool) {
    if wanted == *registered {
        return;
    }
    let device = RAWINPUTDEVICE {
        usUsagePage: USAGE_PAGE_GENERIC,
        usUsage: usage,
        // RIDEV_REMOVE requires a null target window.
        dwFlags: if wanted { RIDEV_INPUTSINK } else { RIDEV_REMOVE },
        hwndTarget: if wanted { sink } else { HWND::default() },
    };
    // SAFETY: one valid RAWINPUTDEVICE and its true size.
    match unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) } {
        Ok(()) => *registered = wanted,
        Err(error) => log::warn!("raw input registration (usage {usage}, on {wanted}) failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_read_by_position() {
        assert_eq!(code_of(0x2C), Some("KeyZ"), "the key left of X, whatever the layout prints on it");
        assert_eq!(code_of(0x47), Some("Numpad7"), "the keypad with NumLock off too");
        assert_eq!(code_of(0xE047), Some("Home"));
        assert_eq!(code_of(0xE02A), None, "the fake Shift before PrintScreen");
        assert_eq!(scan_of(0x90, 0x45, 0).and_then(code_of), Some("NumLock"), "not Pause");
        assert_eq!(scan_of(0x13, 0x1D, KEY_E1), None, "Pause's first half is not Control");
        assert_eq!(scan_of(0xFF, 0x45, 0), None);
        assert_eq!(scan_of(0x24, 0x47, KEY_E0).and_then(code_of), Some("Home"));
    }

    #[test]
    fn a_held_key_counts_once_and_others_are_ignored() {
        let mut listener = Listener { keys: vec!["ShiftLeft".into()], ..Listener::default() };
        let key = |down: bool| Some(Event::Key { code: "ShiftLeft".into(), down });
        assert_eq!(listener.key(0x2A, true), key(true));
        assert_eq!(listener.key(0x2A, true), None, "a repeat");
        assert_eq!(listener.key(0x36, true), None, "the right Shift is another key");
        assert_eq!(listener.key(0x2A, false), key(false));
    }
}
