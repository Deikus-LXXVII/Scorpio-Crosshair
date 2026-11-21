//! The crosshair window: a separate layered window above the game, the same
//! kind Discord's and Steam's overlays-as-windows use. Nothing is ever drawn
//! into the game or its swap chain. It never takes the focus (`NOACTIVATE`),
//! so the game stays the foreground window, and while locked it is
//! click-through (`TRANSPARENT`), so every click reaches the game.

use super::create_window;
use crate::core::raster::Bitmap;
use crate::platform::Event;
use std::cell::{Cell, OnceCell};
use std::sync::mpsc::Sender;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
    BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DestroyWindow, GetCursorPos, GetWindowLongPtrW, GetWindowRect, IsWindowVisible,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, UpdateLayeredWindow, GWL_EXSTYLE, HWND_TOPMOST, MA_NOACTIVATE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA,
    GetForegroundWindow, WM_DISPLAYCHANGE, WM_CAPTURECHANGED, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

struct Overlay {
    hwnd: HWND,
    events: Sender<Event>,
    unlocked: Cell<bool>,
    /// The centre button, (x, y, w, h) inside the window; only while unlocked.
    button: Cell<Option<(i32, i32, i32, i32)>>,
    drag: Cell<Option<Drag>>,
}

/// What the left button is doing between press and release.
#[derive(Clone, Copy)]
enum Drag {
    /// Pressed on the centre button: a click if released there too.
    Button,
    /// Moving the window: where the cursor holds it, and where it started.
    Window { grip: (i32, i32), from: (i32, i32) },
}

// The window procedure is re-entered while this thread is inside calls such as
// SetWindowPos, so the state is in `Cell`s and no borrow is ever held.
thread_local! {
    static OVERLAY: OnceCell<Overlay> = const { OnceCell::new() };
}

fn with(action: impl FnOnce(&Overlay)) {
    OVERLAY.with(|overlay| overlay.get().map(action));
}

/// Creates the hidden overlay window on this thread.
pub fn create(events: Sender<Event>) -> Result<(), String> {
    let style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT;
    let hwnd = create_window(w!("ScorpioOverlay"), procedure, style, WS_POPUP, None)?;
    let overlay = Overlay { hwnd, events, unlocked: Cell::new(false), button: Cell::new(None), drag: Cell::new(None) };
    OVERLAY.with(|cell| cell.set(overlay).map_err(|_| "the overlay already exists on this thread".to_string()))
}

pub fn show(bitmap: &Bitmap, x: i32, y: i32, unlocked: bool, button: Option<(i32, i32, i32, i32)>) {
    with(|overlay| {
        let expected = bitmap.width as usize * bitmap.height as usize * 4;
        if expected == 0 || bitmap.data.len() != expected {
            log::warn!("overlay bitmap {}x{} with {} bytes was not shown", bitmap.width, bitmap.height, bitmap.data.len());
            return;
        }
        overlay.set_unlocked(unlocked);
        overlay.button.set(button.filter(|_| unlocked));
        // SAFETY: `hwnd` is this thread's window; `paint` documents its own pointers.
        unsafe {
            if let Err(error) = paint(overlay.hwnd, bitmap, x, y) {
                log::warn!("overlay could not be painted: {error}");
            }
            // Again on every show: other topmost windows may have gone above since.
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW;
            if let Err(error) = SetWindowPos(overlay.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags) {
                log::warn!("overlay could not be shown: {error}");
            }
        }
    });
}

pub fn hide() {
    with(|overlay| {
        overlay.stop_drag();
        // SAFETY: `hwnd` is this thread's window.
        let _ = unsafe { ShowWindow(overlay.hwnd, SW_HIDE) };
    });
}

pub fn raise() {
    with(|overlay| {
        // SAFETY: `hwnd` is this thread's window.
        unsafe {
            if !IsWindowVisible(overlay.hwnd).as_bool() {
                return;
            }
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
            if let Err(error) = SetWindowPos(overlay.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags) {
                log::warn!("overlay could not be raised: {error}");
            }
        }
    });
}

pub fn destroy() {
    with(|overlay| {
        overlay.stop_drag();
        // SAFETY: `hwnd` is this thread's window and is not used afterwards (the thread is ending).
        if let Err(error) = unsafe { DestroyWindow(overlay.hwnd) } {
            log::warn!("overlay could not be destroyed: {error}");
        }
    });
}

/// Hands the premultiplied BGRA pixels to the window manager, which composes
/// them with per-pixel alpha, and places the window at (x, y).
///
/// SAFETY (caller): `hwnd` is a layered window of this thread and `bitmap.data`
/// holds exactly `width * height * 4` bytes.
unsafe fn paint(hwnd: HWND, bitmap: &Bitmap, x: i32, y: i32) -> windows::core::Result<()> {
    let (width, height) = (bitmap.width as i32, bitmap.height as i32);
    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height, // negative: top row first, as `Bitmap` is
        biPlanes: 1,
        biBitCount: 32,
        ..Default::default()
    };
    let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
    let dc = CreateCompatibleDC(None);
    let mut pixels = std::ptr::null_mut();
    let result = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixels, None, 0).and_then(|dib| {
        // The section is `width * height * 4` bytes, the length checked by the caller.
        std::ptr::copy_nonoverlapping(bitmap.data.as_ptr(), pixels.cast::<u8>(), bitmap.data.len());
        let previous = SelectObject(dc, dib.into());
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let updated = UpdateLayeredWindow(
            hwnd,
            None,
            Some(&POINT { x, y }),
            Some(&SIZE { cx: width, cy: height }),
            Some(dc),
            Some(&POINT::default()),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        // The window manager keeps its own copy, so nothing stays allocated here.
        SelectObject(dc, previous);
        let _ = DeleteObject(dib.into());
        updated
    });
    let _ = DeleteDC(dc);
    result
}

impl Overlay {
    fn set_unlocked(&self, unlocked: bool) {
        if !unlocked {
            self.stop_drag();
        }
        self.unlocked.set(unlocked);
        // SAFETY: `hwnd` is this thread's window; only its own style bits change.
        unsafe {
            let current = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let click_through = WS_EX_TRANSPARENT.0 as isize;
            let wanted = if unlocked { current & !click_through } else { current | click_through };
            if wanted != current {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, wanted);
            }
        }
    }

    /// Ends a drag in progress; WM_CAPTURECHANGED then reports where it ended.
    fn stop_drag(&self) {
        if self.drag.get().is_some() {
            // SAFETY: no arguments; releases this thread's own mouse capture.
            let _ = unsafe { ReleaseCapture() };
        }
    }

    fn origin(&self) -> (i32, i32) {
        let mut rect = RECT::default();
        // SAFETY: `rect` is a valid out-structure.
        let _ = unsafe { GetWindowRect(self.hwnd, &mut rect) };
        (rect.left, rect.top)
    }

    fn on_button(&self, (x, y): (i32, i32)) -> bool {
        self.button.get().is_some_and(|(left, top, width, height)| {
            x >= left && x < left + width && y >= top && y < top + height
        })
    }

    /// Handles a message of the overlay window; None leaves it to the default.
    fn message(&self, message: u32, lparam: LPARAM) -> Option<LRESULT> {
        // Client coordinates of mouse messages: signed 16-bit halves of lparam.
        let point = ((lparam.0 & 0xFFFF) as i16 as i32, ((lparam.0 >> 16) & 0xFFFF) as i16 as i32);
        match message {
            // A click on the unlocked overlay must not pull the focus from the game.
            WM_MOUSEACTIVATE => return Some(LRESULT(MA_NOACTIVATE as isize)),
            WM_LBUTTONDOWN if self.unlocked.get() => {
                let drag = if self.on_button(point) {
                    Drag::Button
                } else {
                    let mut cursor = POINT::default();
                    // SAFETY: `cursor` is a valid out-structure.
                    let _ = unsafe { GetCursorPos(&mut cursor) };
                    let from = self.origin();
                    Drag::Window { grip: (cursor.x - from.0, cursor.y - from.1), from }
                };
                // SAFETY: `hwnd` is this thread's window. Capture keeps the
                // moves coming when the cursor outruns the window.
                unsafe { SetCapture(self.hwnd) };
                self.drag.set(Some(drag));
            }
            WM_MOUSEMOVE => {
                if let Some(Drag::Window { grip, .. }) = self.drag.get() {
                    let mut cursor = POINT::default();
                    let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
                    // SAFETY: `cursor` is a valid out-structure and `hwnd` is this thread's window.
                    unsafe {
                        if GetCursorPos(&mut cursor).is_ok() {
                            let _ = SetWindowPos(self.hwnd, None, cursor.x - grip.0, cursor.y - grip.1, 0, 0, flags);
                        }
                    }
                }
            }
            WM_LBUTTONUP => {
                if matches!(self.drag.get(), Some(Drag::Button)) && self.on_button(point) {
                    let _ = self.events.send(Event::CenterClicked);
                }
                self.stop_drag();
            }
            // Sent on release and when anything else ends the drag (locking, hiding, Alt+Tab).
            WM_CAPTURECHANGED => {
                if let Some(Drag::Window { from, .. }) = self.drag.take() {
                    let (x, y) = self.origin();
                    if (x, y) != from {
                        let _ = self.events.send(Event::Moved { x, y });
                    }
                }
            }
            _ => return None,
        }
        Some(LRESULT(0))
    }
}

unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // A resolution change (a game going exclusive fullscreen) moves the
    // centre: describe the window in front again. Top-level windows get it.
    if message == WM_DISPLAYCHANGE {
        // SAFETY: no arguments; reads which window is in front right now.
        super::report_foreground(unsafe { GetForegroundWindow() });
    }
    let handled = OVERLAY.with(|overlay| overlay.get().and_then(|overlay| overlay.message(message, lparam)));
    // SAFETY: forwards the untouched arguments of this window procedure.
    handled.unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, message, wparam, lparam) })
}
