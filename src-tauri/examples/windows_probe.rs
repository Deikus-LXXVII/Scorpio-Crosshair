//! Drives the platform layer for a few seconds and writes what it saw to the
//! file named by the first argument (or stdout): the window in front, the
//! window list, and whether the overlay window was created and destroyed.
//! Windows only; `cargo run --example windows_probe -- probe.txt`.

#[cfg(not(windows))]
fn main() {
    eprintln!("windows_probe only does something on Windows");
}

#[cfg(windows)]
fn main() {
    probe::run();
}

#[cfg(windows)]
mod probe {
    use scorpio_lib::core::raster::Bitmap;
    use scorpio_lib::platform::{start, Command, MouseButton};
    use std::fmt::Write as _;
    use std::sync::mpsc::channel;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, FindWindowW, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
        IsWindowVisible, PeekMessageW, SetWindowPos, EVENT_OBJECT_LOCATIONCHANGE, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOSIZE,
        SWP_NOZORDER, WINEVENT_OUTOFCONTEXT, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    thread_local! {
        /// Location events seen by the probe's own hook: object id -> count.
        static SEEN: RefCell<BTreeMap<i32, u32>> = RefCell::default();
    }

    unsafe extern "system" fn counted(_: HWINEVENTHOOK, _: u32, _: HWND, object: i32, _: i32, _: u32, _: u32) {
        SEEN.with(|seen| *seen.borrow_mut().entry(object).or_default() += 1);
    }

    /// Pumps this thread's messages for `ms`, so out-of-context events arrive.
    fn pump(ms: u64) {
        let end = Instant::now() + Duration::from_millis(ms);
        let mut message = MSG::default();
        while Instant::now() < end {
            // SAFETY: `message` is a valid out-structure.
            unsafe {
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Hooks location changes scoped to one process and thread, the way
    /// Scorpio follows a game window, and counts what arrives by object id
    /// (0 window, -9 cursor, -8 caret, others parts of windows).
    fn count_location_events(pid: u32, thread: u32, ms: u64, during: impl FnOnce()) -> BTreeMap<i32, u32> {
        SEEN.with(|seen| seen.borrow_mut().clear());
        // SAFETY: plain callback; unhooked below on this thread.
        let hook = unsafe { SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, None, Some(counted), pid, thread, WINEVENT_OUTOFCONTEXT) };
        during();
        pump(ms);
        // SAFETY: the hook belongs to this thread.
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
        SEEN.with(|seen| seen.borrow().clone())
    }

    fn overlay_rect() -> Option<(i32, i32)> {
        let mut rect = RECT::default();
        // SAFETY: read-only lookups; `rect` is a valid out-structure.
        unsafe { FindWindowW(w!("ScorpioOverlay"), None).ok().filter(|h| GetWindowRect(*h, &mut rect).is_ok()).map(|_| (rect.left, rect.top)) }
    }

    static REPORT: Mutex<String> = Mutex::new(String::new());

    fn note(line: String) {
        let _ = writeln!(REPORT.lock().unwrap(), "{line}");
    }

    /// Collects the platform layer's warnings into the report.
    struct Warnings;
    impl log::Log for Warnings {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            note(format!("LOG {} {}", record.level(), record.args()));
        }
        fn flush(&self) {}
    }

    fn overlay_state() -> &'static str {
        // SAFETY: looks a window up by class name and reads its visibility; read-only.
        unsafe {
            match FindWindowW(w!("ScorpioOverlay"), None) {
                Ok(hwnd) if IsWindowVisible(hwnd).as_bool() => "exists, visible",
                Ok(_) => "exists, hidden",
                Err(_) => "does not exist",
            }
        }
    }

    pub fn run() {
        let _ = log::set_logger(&Warnings);
        log::set_max_level(log::LevelFilter::Trace);
        let begun = Instant::now();
        let (events, received) = channel();
        let platform = match start(events) {
            Ok(platform) => platform,
            Err(error) => {
                note(format!("START FAILED {error:?}"));
                return finish();
            }
        };
        note(format!("started in {:?}; own pid {}; overlay {}", begun.elapsed(), std::process::id(), overlay_state()));

        // A faint 32x32 square, premultiplied: alpha 64, colour scaled to match.
        let mut bitmap = Bitmap::transparent(32, 32);
        for pixel in bitmap.data.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0, 64, 0, 64]);
        }
        let pause = |ms| std::thread::sleep(Duration::from_millis(ms));
        platform.send(Command::Raise);
        platform.send(Command::Show { bitmap: bitmap.clone(), x: 200, y: 200, unlocked: false, button: None });
        pause(400);
        note(format!("after locked show: overlay {}", overlay_state()));
        platform.send(Command::ListenTo { mouse: vec![MouseButton::Right], keys: vec!["H".into(), "Mouse4".into()] });
        platform.send(Command::Show { bitmap, x: 240, y: 240, unlocked: true, button: Some((12, 12, 8, 8)) });
        pause(400);
        note(format!("after unlocked show: overlay {}", overlay_state()));
        platform.send(Command::Move { x: 260, y: 270 });
        pause(200);
        note(format!("after move to (260, 270): overlay at {:?}", overlay_rect()));
        platform.send(Command::Follow(true));
        pause(200);
        platform.send(Command::Follow(false));
        platform.send(Command::ListenTo { mouse: vec![], keys: vec![] });
        platform.send(Command::Raise);
        platform.send(Command::Hide);
        pause(200);
        note(format!("after hide: overlay {}", overlay_state()));

        let listed = Instant::now();
        let windows = (platform.list_windows)();
        note(format!("list_windows: {} windows in {:?}", windows.len(), listed.elapsed()));
        for window in &windows {
            note(format!("  {window:?}"));
        }

        platform.send(Command::Quit);
        pause(300);
        note(format!("after quit: overlay {}", overlay_state()));
        for event in received.try_iter() {
            note(format!("EVENT {event:?}"));
        }

        // The location hook, scoped as Scorpio scopes it, on a window of the
        // probe's own: moving it is reported; nothing else is.
        // SAFETY: a tiny tool window of this thread at the screen's corner, destroyed below.
        let own = unsafe {
            CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, w!("STATIC"), w!("probe"), WS_POPUP | WS_VISIBLE, 0, 0, 4, 4, None, None, None, None)
        };
        if let Ok(own) = own {
            let mut pid = 0;
            // SAFETY: read-only; valid out-pointer.
            let thread = unsafe { GetWindowThreadProcessId(own, Some(&mut pid)) };
            let seen = count_location_events(pid, thread, 300, || {
                for x in [1, 2, 3] {
                    // SAFETY: the probe's own window.
                    let _ = unsafe { SetWindowPos(own, None, x, 0, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
                }
            });
            note(format!("own window moved 3 times: location events by object id {seen:?}"));
            // SAFETY: the probe's own window, not used again.
            let _ = unsafe { DestroyWindow(own) };
        }

        // The window in front now: how often would following it wake
        // Scorpio while it is used? Only for programs that are surely not games.
        // SAFETY: read-only window-manager queries.
        let front = unsafe { GetForegroundWindow() };
        let mut pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(front, Some(&mut pid)) };
        let (name, _) = scorpio_lib::platform::process_image(pid);
        const SAFE: [&str; 12] = ["explorer", "chrome", "msedge", "firefox", "brave", "code", "windowsterminal", "discord", "obs64", "notepad", "telegram", "cmd"];
        if SAFE.contains(&name.to_lowercase().as_str()) {
            let seconds = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(8);
            let seen = count_location_events(pid, thread, seconds * 1000, || {});
            note(format!("{name} in front for {seconds} s, hook scoped to its pid and thread: location events by object id {seen:?}"));
            let seen = count_location_events(pid, 0, seconds * 1000, || {});
            note(format!("{name} in front for {seconds} s, hook scoped to its pid only: location events by object id {seen:?}"));
        } else {
            note(format!("window in front is {name:?}: not measured (only known non-games are)"));
        }
        finish();
    }

    fn finish() {
        let report = REPORT.lock().unwrap();
        match std::env::args().nth(1) {
            Some(path) => std::fs::write(path, report.as_bytes()).expect("the report could not be written"),
            None => print!("{report}"),
        }
    }
}
