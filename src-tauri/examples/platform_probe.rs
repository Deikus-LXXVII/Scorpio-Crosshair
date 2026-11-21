//! Drives the platform layer from stdin so a test harness can check it from
//! outside. Prints every event as `EVENT <debug>`. Commands, one per line:
//!   show <w> <h> <x> <y> locked|unlocked   (button = the middle third; unlocked draws a slab)
//!   hide | list | quit
//!   listen <buttons,comma> <keys,comma>     (e.g. `listen Right,Back H,Shift`)
//!   inspect <window id>                     (Linux: shapes and override-redirect)

use scorpio_lib::core::raster::Bitmap;
use scorpio_lib::platform::{self, Command, MouseButton};
use std::io::{BufRead, Write};

/// A red cross; `slab` fills the rest with translucent dark grey, like the
/// unlocked slab does.
fn cross(width: u32, height: u32, slab: bool) -> Bitmap {
    let mut bitmap = Bitmap::transparent(width, height);
    if slab {
        bitmap.data.chunks_exact_mut(4).for_each(|p| p.copy_from_slice(&[7, 6, 7, 158]));
    }
    for y in 0..height {
        for x in 0..width {
            if x == width / 2 || y == height / 2 {
                let i = ((y * width + x) * 4) as usize;
                bitmap.data[i..i + 4].copy_from_slice(&[0, 0, 255, 255]);
            }
        }
    }
    bitmap
}

fn button(name: &str) -> Option<MouseButton> {
    Some(match name {
        "Left" => MouseButton::Left,
        "Right" => MouseButton::Right,
        "Middle" => MouseButton::Middle,
        "Back" => MouseButton::Back,
        "Forward" => MouseButton::Forward,
        _ => return None,
    })
}

fn list(arg: Option<&str>) -> Vec<&str> {
    arg.unwrap_or("").split(',').filter(|s| !s.is_empty() && *s != "-").collect()
}

fn main() {
    let (sender, events) = std::sync::mpsc::channel();
    let platform = match platform::start(sender) {
        Ok(p) => p,
        Err(e) => {
            println!("UNSUPPORTED {}", e.0);
            return;
        }
    };
    std::thread::spawn(move || {
        for event in events {
            println!("EVENT {event:?}");
            std::io::stdout().flush().ok();
        }
    });
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap_or_default();
        let words: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| words.get(i).and_then(|w| w.parse::<i32>().ok()).unwrap_or(0);
        match words.first().copied() {
            Some("show") => {
                let (w, h) = (num(1), num(2));
                let unlocked = words.get(5) == Some(&"unlocked");
                platform.send(Command::Show {
                    bitmap: cross(w as u32, h as u32, unlocked),
                    x: num(3),
                    y: num(4),
                    unlocked,
                    button: Some((w / 3, h / 3, w / 3, h / 3)),
                });
            }
            Some("hide") => platform.send(Command::Hide),
            Some("listen") => platform.send(Command::ListenTo {
                mouse: list(words.get(1).copied()).into_iter().filter_map(button).collect(),
                keys: list(words.get(2).copied()).into_iter().map(String::from).collect(),
            }),
            Some("list") => {
                for window in (platform.list_windows)() {
                    println!("WINDOW {window:?}");
                }
            }
            Some("inspect") => inspect(words.get(1).copied().unwrap_or("0")),
            Some("quit") => {
                platform.send(Command::Quit);
                break;
            }
            _ => println!("? {line}"),
        }
        println!("OK {line}");
        std::io::stdout().flush().ok();
    }
}

#[cfg(target_os = "linux")]
fn inspect(id: &str) {
    use x11rb::connection::Connection;
    use x11rb::protocol::shape::{ConnectionExt as _, SK};
    use x11rb::protocol::xproto::ConnectionExt as _;
    let id = u32::from_str_radix(id.trim_start_matches("0x"), 16).unwrap_or(0);
    let (conn, _) = x11rb::connect(None).expect("connect");
    let rects = |kind| conn.shape_get_rectangles(id, kind).ok().and_then(|c| c.reply().ok()).map(|r| r.rectangles.len());
    let attrs = conn.get_window_attributes(id).ok().and_then(|c| c.reply().ok());
    println!(
        "INSPECT input_rects={:?} bounding_rects={:?} override_redirect={:?} depth={:?}",
        rects(SK::INPUT),
        rects(SK::BOUNDING),
        attrs.map(|a| a.override_redirect),
        conn.get_geometry(id).ok().and_then(|c| c.reply().ok()).map(|g| g.depth),
    );
    let _ = conn.flush();
}

#[cfg(not(target_os = "linux"))]
fn inspect(_: &str) {}
