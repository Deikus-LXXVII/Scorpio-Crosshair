//! What the window manager says about a top-level window. Everything here is
//! window-manager data (`user32`/`dwmapi`); the owning process is never opened.

use super::process;
use crate::core::focus::WindowInfo;
use crate::platform::Event;
use std::ffi::c_void;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, IsZoomed, GA_ROOT, GWL_STYLE, GW_OWNER, WS_CAPTION,
};

/// The event for a window that came to the front, or None when the event is
/// not about a real top-level window and should be ignored.
pub fn foreground_event(hwnd: HWND) -> Option<Event> {
    if hwnd.is_invalid() {
        return Some(Event::Foreground(None));
    }
    // SAFETY: both calls only read window-manager state and accept stale handles.
    let top_level = unsafe { IsWindowVisible(hwnd).as_bool() && GetAncestor(hwnd, GA_ROOT) == hwnd };
    if !top_level {
        return None;
    }
    let info = describe(hwnd);
    Some(Event::Foreground((!is_desktop(&info)).then_some(info)))
}

/// The shell's desktop window: in front when nothing else is.
fn is_desktop(info: &WindowInfo) -> bool {
    matches!(info.window_class.as_str(), "Progman" | "WorkerW")
}

pub fn describe(hwnd: HWND) -> WindowInfo {
    let mut pid = 0u32;
    let mut title = [0u16; 512];
    let mut class = [0u16; 256];
    // SAFETY: the out-pointer and both buffers are valid for the duration of the calls.
    let (title_length, class_length) = unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        (GetWindowTextW(hwnd, &mut title), GetClassNameW(hwnd, &mut class))
    };
    let (name, path) = process::image(pid);
    let (fullscreen, monitor) = placement(hwnd);
    WindowInfo {
        pid,
        name,
        path,
        title: String::from_utf16_lossy(&title[..title_length.max(0) as usize]),
        window_class: String::from_utf16_lossy(&class[..class_length.max(0) as usize]),
        fullscreen,
        monitor,
    }
}

/// Whether the window is fullscreen, and the monitor it is on. Borderless or
/// exclusive fullscreen: the window covers its whole monitor without being a
/// maximised or captioned window (a maximised browser is not a game).
fn placement(hwnd: HWND) -> (bool, Option<[i32; 4]>) {
    let mut window = RECT::default();
    let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: the out-structures are valid and `cbSize` is set as GetMonitorInfoW requires.
    let (measured, zoomed, style) = unsafe {
        let measured = GetWindowRect(hwnd, &mut window).is_ok()
            && GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut monitor).as_bool();
        (measured, IsZoomed(hwnd).as_bool(), GetWindowLongPtrW(hwnd, GWL_STYLE) as u32)
    };
    let screen = monitor.rcMonitor;
    let covers = window.left <= screen.left
        && window.top <= screen.top
        && window.right >= screen.right
        && window.bottom >= screen.bottom;
    let rect = measured.then_some([screen.left, screen.top, screen.right - screen.left, screen.bottom - screen.top]);
    (measured && covers && !zoomed && style & WS_CAPTION.0 != WS_CAPTION.0, rect)
}

/// Visible, unowned, titled top-level windows of other processes, without the desktop.
pub fn list() -> Vec<WindowInfo> {
    let mut windows: Vec<WindowInfo> = Vec::new();
    // SAFETY: the callback runs only during this call, while `windows` is alive
    // and not otherwise borrowed. The thread's DPI context is switched so the
    // window and monitor rectangles are both in physical pixels, then restored.
    unsafe {
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut Vec<WindowInfo> as isize));
        if !previous.0.is_null() {
            SetThreadDpiAwarenessContext(previous);
        }
    }
    windows
}

unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
    // SAFETY: `list` is the pointer `list()` passed in, valid for the whole enumeration.
    let windows = unsafe { &mut *(list.0 as *mut Vec<WindowInfo>) };
    let mut cloaked = 0u32;
    // SAFETY: read-only window-manager queries; `cloaked` is a valid 4-byte out-value.
    let candidate = unsafe {
        let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast::<c_void>(), 4);
        IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).map_or(true, |owner| owner.is_invalid())
    };
    // Cloaked windows are "visible" but not on screen: suspended store apps,
    // windows on another virtual desktop.
    if candidate && cloaked == 0 {
        let info = describe(hwnd);
        if !info.title.is_empty() && info.pid != std::process::id() && !is_desktop(&info) {
            windows.push(info);
        }
    }
    true.into()
}
