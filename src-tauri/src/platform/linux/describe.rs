//! Describing a top-level window from window-manager properties, plus the
//! executable read from `/proc` (readlink of `exe`, `cmdline` for Wine). No
//! other process is ever opened or signalled.

use crate::core::focus::WindowInfo;
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, GetPropertyReply, Window};

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        _NET_ACTIVE_WINDOW,
        _NET_CLIENT_LIST,
        _NET_WM_PID,
        _NET_WM_NAME,
        _NET_WM_STATE,
        _NET_WM_STATE_FULLSCREEN,
        UTF8_STRING,
        _SCORPIO_WAKE,
    }
}

fn property(conn: &impl Connection, window: Window, name: Atom, kind: Atom) -> Option<GetPropertyReply> {
    conn.get_property(false, window, name, kind, 0, 4096).ok()?.reply().ok()
}

/// `None` when the window no longer exists.
pub fn describe(conn: &impl Connection, atoms: &Atoms, window: Window) -> Option<WindowInfo> {
    let class = property(conn, window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())?;
    // WM_CLASS is "instance\0class\0".
    let window_class = class.value.split(|&b| b == 0).nth(1).map(lossy).unwrap_or_default();
    let pid = property(conn, window, atoms._NET_WM_PID, AtomEnum::CARDINAL.into())
        .and_then(|r| r.value32()?.next())
        .unwrap_or(0);
    let title = property(conn, window, atoms._NET_WM_NAME, atoms.UTF8_STRING)
        .filter(|r| !r.value.is_empty())
        .or_else(|| property(conn, window, AtomEnum::WM_NAME.into(), AtomEnum::ANY.into()))
        .map(|r| lossy(&r.value))
        .unwrap_or_default();
    let path = if pid == 0 { String::new() } else { process_path(pid) };
    let (client, monitor, fullscreen) = geometry(conn, atoms, window);
    Some(WindowInfo { pid, name: file_stem(&path), path, title, window_class, fullscreen, monitor, client })
}

/// Where the window is: its client area on the root (the X window inside
/// any frame the window manager drew), its monitor, and whether the window
/// manager calls it fullscreen.
pub fn geometry(conn: &impl Connection, atoms: &Atoms, window: Window) -> crate::platform::Geometry {
    let fullscreen = property(conn, window, atoms._NET_WM_STATE, AtomEnum::ATOM.into())
        .and_then(|r| r.value32().map(|mut states| states.any(|s| s == atoms._NET_WM_STATE_FULLSCREEN)))
        .unwrap_or(false);
    let client = client_area(conn, window);
    (client.map(|(rect, _)| rect), client.and_then(|([x, y, w, h], root)| monitor_at(conn, root, x + w / 2, y + h / 2)), fullscreen)
}

fn client_area(conn: &impl Connection, window: Window) -> Option<([i32; 4], Window)> {
    let geometry = conn.get_geometry(window).ok()?.reply().ok()?;
    let origin = conn.translate_coordinates(window, geometry.root, 0, 0).ok()?.reply().ok()?;
    Some(([i32::from(origin.dst_x), i32::from(origin.dst_y), i32::from(geometry.width), i32::from(geometry.height)], geometry.root))
}

/// The top-level window the window manager put `window` in (its frame), or
/// the window itself when it has none.
pub fn frame_of(conn: &impl Connection, window: Window) -> Window {
    let mut current = window;
    for _ in 0..16 {
        let Some(tree) = conn.query_tree(current).ok().and_then(|cookie| cookie.reply().ok()) else { break };
        if tree.parent == tree.root || tree.parent == x11rb::NONE {
            break;
        }
        current = tree.parent;
    }
    current
}

/// The RandR monitor holding the point (the window's centre), else the primary one.
fn monitor_at(conn: &impl Connection, root: Window, cx: i32, cy: i32) -> Option<[i32; 4]> {
    let monitors = conn.randr_get_monitors(root, true).ok()?.reply().ok()?.monitors;
    let rect = |m: &x11rb::protocol::randr::MonitorInfo| [i32::from(m.x), i32::from(m.y), i32::from(m.width), i32::from(m.height)];
    let holds = |m: &&x11rb::protocol::randr::MonitorInfo| {
        let [x, y, w, h] = rect(m);
        (x..x + w).contains(&cx) && (y..y + h).contains(&cy)
    };
    monitors.iter().find(holds).or_else(|| monitors.iter().find(|m| m.primary)).map(rect)
}

/// The window manager's list of managed top-level windows, minus our own.
pub fn list(conn: &impl Connection, atoms: &Atoms, root: Window) -> Vec<WindowInfo> {
    let own = std::process::id();
    let Some(reply) = property(conn, root, atoms._NET_CLIENT_LIST, AtomEnum::WINDOW.into()) else { return Vec::new() };
    let Some(windows) = reply.value32() else { return Vec::new() };
    windows.filter_map(|w| describe(conn, atoms, w)).filter(|info| info.pid != own).collect()
}

pub fn active_window(conn: &impl Connection, atoms: &Atoms, root: Window) -> Option<Window> {
    property(conn, root, atoms._NET_ACTIVE_WINDOW, AtomEnum::WINDOW.into())?.value32()?.next().filter(|&w| w != 0)
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn process_path(pid: u32) -> String {
    let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe")) else { return String::new() };
    let exe = exe.to_string_lossy();
    let cmdline = if is_wine(&exe) { std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default() } else { Vec::new() };
    exe_path(&exe, &cmdline)
}

// Covers wine, wine64, wine-preloader and wine64-preloader (Proton included).
fn is_wine(exe: &str) -> bool {
    exe.rsplit('/').next().is_some_and(|file| file.starts_with("wine"))
}

/// The game's own path: for Wine the `.exe` it was started with, since `exe`
/// only names the Wine loader.
fn exe_path(exe: &str, cmdline: &[u8]) -> String {
    if !is_wine(exe) {
        return exe.to_string();
    }
    cmdline
        .split(|&b| b == 0)
        .map(String::from_utf8_lossy)
        .find(|arg| arg.to_ascii_lowercase().ends_with(".exe"))
        .map(|arg| match arg.get(..3) {
            Some(drive) if drive.eq_ignore_ascii_case("z:\\") => format!("/{}", arg[3..].replace('\\', "/")),
            _ => arg.into_owned(),
        })
        .unwrap_or_else(|| exe.to_string())
}

fn file_stem(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or("");
    match file.rfind('.') {
        Some(dot) if dot > 0 => file[..dot].to_string(),
        _ => file.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_binaries_keep_their_path() {
        assert_eq!(exe_path("/usr/bin/xterm", b""), "/usr/bin/xterm");
        assert_eq!(file_stem("/usr/bin/xterm"), "xterm");
    }

    #[test]
    fn wine_takes_the_exe_from_cmdline() {
        let cmd = b"Z:\\home\\me\\Games\\cs2.exe\0-novid\0";
        assert_eq!(exe_path("/opt/proton/bin/wine64-preloader", cmd), "/home/me/Games/cs2.exe");
        let cmd = b"C:\\Program Files\\Game\\Game.EXE\0";
        assert_eq!(exe_path("/usr/bin/wine64", cmd), "C:\\Program Files\\Game\\Game.EXE");
        assert_eq!(file_stem("C:\\Program Files\\Game\\Game.EXE"), "Game");
        assert_eq!(exe_path("/usr/bin/wineserver", b"wineserver\0"), "/usr/bin/wineserver");
    }

    #[test]
    fn stems() {
        assert_eq!(file_stem(""), "");
        assert_eq!(file_stem("/a/.hidden"), ".hidden");
        assert_eq!(file_stem("/a/b.tar.gz"), "b.tar");
    }
}
