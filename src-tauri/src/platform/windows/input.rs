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

/// (button, RI_MOUSE_*_DOWN flag, RI_MOUSE_*_UP flag, name as a key)
const BUTTONS: [(MouseButton, u16, u16, &str); 5] = [
    (MouseButton::Left, 0x001, 0x002, "Mouse1"),
    (MouseButton::Right, 0x004, 0x008, "Mouse2"),
    (MouseButton::Middle, 0x010, 0x020, "Mouse3"),
    (MouseButton::Back, 0x040, 0x080, "Mouse4"),
    (MouseButton::Forward, 0x100, 0x200, "Mouse5"),
];

#[derive(Default)]
pub struct Listener {
    mouse: Vec<MouseButton>,
    keys: Vec<String>,
    /// Physical keys now down, with their names: tells a repeat from a press,
    /// and keeps "Shift" down until both Shift keys are up.
    held: Vec<(u32, String)>,
    mouse_registered: bool,
    keyboard_registered: bool,
}

impl Listener {
    /// Replaces what is listened to; `sink` is the hidden window that receives WM_INPUT.
    pub fn listen(&mut self, sink: HWND, mouse: Vec<MouseButton>, keys: Vec<String>) {
        self.mouse = mouse;
        self.keys = keys;
        self.held.clear();
        // A keybind may be a mouse button ("Mouse4"); that needs the mouse too.
        let want_mouse = !self.mouse.is_empty() || BUTTONS.iter().any(|button| self.keys.iter().any(|key| key == button.3));
        let want_keyboard = self.keys.iter().any(|key| !key.starts_with("Mouse"));
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
            for (button, down_flag, up_flag, name) in BUTTONS {
                if flags & (down_flag | up_flag) == 0 {
                    continue;
                }
                let down = flags & down_flag != 0;
                if self.mouse.contains(&button) {
                    report(Event::Mouse { button, down });
                }
                if self.keys.iter().any(|key| key == name) {
                    report(Event::Key { code: name.to_string(), down });
                }
            }
        } else if raw.header.dwType == RIM_TYPEKEYBOARD.0 {
            // SAFETY: the header says this packet is the keyboard member of the union.
            let key = unsafe { raw.data.keyboard };
            let Some(name) = key_name(key.VKey) else { return };
            if !self.keys.contains(&name) {
                return;
            }
            let physical = u32::from(key.MakeCode) | u32::from(key.Flags & KEY_E0) << 16 | u32::from(key.VKey) << 20;
            let was_down = self.held.iter().any(|(_, held)| *held == name);
            if key.Flags & KEY_UP == 0 {
                if self.held.iter().any(|(id, _)| *id == physical) {
                    return; // auto-repeat
                }
                self.held.push((physical, name.clone()));
                if !was_down {
                    report(Event::Key { code: name, down: true });
                }
            } else {
                self.held.retain(|(id, _)| *id != physical);
                if was_down && !self.held.iter().any(|(_, held)| *held == name) {
                    report(Event::Key { code: name, down: false });
                }
            }
        }
    }
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

/// The platform-neutral name of a virtual key, as keybinds store it. Left and
/// right Shift, Control and Alt share one name. Punctuation is named by its
/// US-layout character.
pub fn key_name(vkey: u16) -> Option<String> {
    let fixed = match vkey {
        0x41..=0x5A | 0x30..=0x39 => return Some(char::from(vkey as u8).to_string()),
        0x70..=0x87 => return Some(format!("F{}", vkey - 0x6F)),
        0x60..=0x69 => return Some(format!("Numpad{}", vkey - 0x60)),
        0x10 | 0xA0 | 0xA1 => "Shift",
        0x11 | 0xA2 | 0xA3 => "Control",
        0x12 | 0xA4 | 0xA5 => "Alt",
        0x5B | 0x5C => "Super",
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x13 => "Pause",
        0x14 => "CapsLock",
        0x1B => "Escape",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2C => "PrintScreen",
        0x2D => "Insert",
        0x2E => "Delete",
        0x6A => "NumpadMultiply",
        0x6B => "NumpadAdd",
        0x6D => "NumpadSubtract",
        0x6E => "NumpadDecimal",
        0x6F => "NumpadDivide",
        0x90 => "NumLock",
        0x91 => "ScrollLock",
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        _ => return None, // includes 0xFF, the filler in multi-part scan codes
    };
    Some(fixed.to_string())
}
