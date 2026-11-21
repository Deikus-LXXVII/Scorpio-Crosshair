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
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible};

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
        platform.send(Command::ExcludeFromCapture(true));
        platform.send(Command::Show { bitmap: bitmap.clone(), x: 200, y: 200, unlocked: false, button: None });
        pause(400);
        note(format!("after locked show: overlay {}", overlay_state()));
        platform.send(Command::ListenTo { mouse: vec![MouseButton::Right], keys: vec!["H".into(), "Mouse4".into()] });
        platform.send(Command::Show { bitmap, x: 240, y: 240, unlocked: true, button: Some((12, 12, 8, 8)) });
        pause(400);
        note(format!("after unlocked show: overlay {}", overlay_state()));
        platform.send(Command::ListenTo { mouse: vec![], keys: vec![] });
        platform.send(Command::ExcludeFromCapture(false));
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
