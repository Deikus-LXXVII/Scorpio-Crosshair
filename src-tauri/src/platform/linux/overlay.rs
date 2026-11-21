//! The crosshair window: override-redirect (so no window manager, taskbar or
//! focus), 32-bit ARGB, and an empty input shape while locked so every click
//! goes to the game underneath.

use super::super::Event;
use crate::core::raster::Bitmap;
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event as XEvent;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::NONE;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Pointer travel (pixels) after which a press is a drag, not a click.
const DRAG_THRESHOLD: i32 = 3;

struct Drag {
    start: (i32, i32),
    origin: (i32, i32),
    moved: bool,
}

pub struct Overlay {
    pub window: Window,
    gc: Gcontext,
    compositor: Atom,
    swap_bytes: bool,
    /// Pixels as the server wants them, kept to repaint on Expose.
    pixels: Vec<u8>,
    width: u16,
    height: u16,
    x: i32,
    y: i32,
    mapped: bool,
    unlocked: bool,
    button: Option<(i32, i32, i32, i32)>,
    drag: Option<Drag>,
}

impl Overlay {
    pub fn create(conn: &impl Connection, screen_num: usize) -> Result<Overlay> {
        let screen = &conn.setup().roots[screen_num];
        let visual = screen
            .allowed_depths
            .iter()
            .filter(|d| d.depth == 32)
            .flat_map(|d| &d.visuals)
            .find(|v| v.class == VisualClass::TRUE_COLOR)
            .ok_or("the X server has no 32-bit TrueColor visual")?
            .visual_id;
        let colormap = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual)?;
        let window = conn.generate_id()?;
        // A depth other than the root's needs an explicit border pixel and colormap.
        let aux = CreateWindowAux::new()
            .background_pixel(0)
            .border_pixel(0)
            .override_redirect(1)
            .colormap(colormap)
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::BUTTON1_MOTION);
        conn.create_window(32, window, screen.root, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, visual, &aux)?;
        conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, b"Scorpio overlay")?;
        conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"scorpio\0Scorpio\0")?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, window, &CreateGCAux::new())?;
        let compositor = conn.intern_atom(false, format!("_NET_WM_CM_S{screen_num}").as_bytes())?.reply()?.atom;
        Ok(Overlay {
            window,
            gc,
            compositor,
            swap_bytes: conn.setup().image_byte_order == ImageOrder::MSB_FIRST,
            pixels: Vec::new(),
            width: 0,
            height: 0,
            x: 0,
            y: 0,
            mapped: false,
            unlocked: false,
            button: None,
            drag: None,
        })
    }

    pub fn show(&mut self, conn: &impl Connection, bitmap: &Bitmap, x: i32, y: i32, unlocked: bool, button: Option<(i32, i32, i32, i32)>) -> Result<()> {
        let (Ok(width @ 1..), Ok(height @ 1..)) = (u16::try_from(bitmap.width), u16::try_from(bitmap.height)) else {
            return self.hide(conn);
        };
        (self.width, self.height, self.x, self.y) = (width, height, x, y);
        (self.unlocked, self.button) = (unlocked, button);
        if !unlocked {
            self.drag = None;
        }
        conn.configure_window(
            self.window,
            &ConfigureWindowAux::new().x(x).y(y).width(u32::from(width)).height(u32::from(height)).stack_mode(StackMode::ABOVE),
        )?;
        if unlocked {
            conn.shape_mask(SO::SET, SK::INPUT, self.window, 0, 0, NONE)?;
        } else {
            conn.shape_rectangles(SO::SET, SK::INPUT, ClipOrdering::UNSORTED, self.window, 0, 0, &[])?;
        }
        // Without a compositor alpha is ignored and the window would be a black
        // square: cut it to the painted pixels instead.
        if conn.get_selection_owner(self.compositor)?.reply()?.owner == NONE {
            conn.shape_rectangles(SO::SET, SK::BOUNDING, ClipOrdering::YX_BANDED, self.window, 0, 0, &painted(bitmap))?;
        } else {
            conn.shape_mask(SO::SET, SK::BOUNDING, self.window, 0, 0, NONE)?;
        }
        self.pixels.clone_from(&bitmap.data);
        if self.swap_bytes {
            self.pixels.chunks_exact_mut(4).for_each(|p| p.reverse());
        }
        if !self.mapped {
            conn.map_window(self.window)?;
            self.mapped = true;
        }
        self.paint(conn)
    }

    pub fn hide(&mut self, conn: &impl Connection) -> Result<()> {
        self.drag = None;
        if self.mapped {
            conn.unmap_window(self.window)?;
            self.mapped = false;
        }
        Ok(())
    }

    fn paint(&self, conn: &impl Connection) -> Result<()> {
        let stride = usize::from(self.width) * 4;
        if !self.mapped || stride == 0 {
            return Ok(());
        }
        // Large bitmaps are sent in bands that fit the server's request limit.
        let rows = ((conn.maximum_request_bytes() - 64) / stride).max(1);
        for (band, data) in self.pixels.chunks(rows * stride).enumerate() {
            let y = i16::try_from(band * rows)?;
            let height = u16::try_from(data.len() / stride)?;
            conn.put_image(ImageFormat::Z_PIXMAP, self.window, self.gc, self.width, height, 0, y, 0, 32, data)?;
        }
        Ok(())
    }

    /// Handles events for the overlay window; returns what the app should hear.
    pub fn handle(&mut self, conn: &impl Connection, event: &XEvent) -> Result<Option<Event>> {
        match event {
            XEvent::Expose(e) if e.window == self.window && e.count == 0 => self.paint(conn)?,
            XEvent::ButtonPress(e) if e.event == self.window && e.detail == 1 && self.unlocked => {
                self.drag = Some(Drag { start: (e.root_x.into(), e.root_y.into()), origin: (self.x, self.y), moved: false });
            }
            XEvent::MotionNotify(e) if e.event == self.window => {
                if let Some(drag) = &mut self.drag {
                    let (dx, dy) = (i32::from(e.root_x) - drag.start.0, i32::from(e.root_y) - drag.start.1);
                    drag.moved |= dx.abs() + dy.abs() > DRAG_THRESHOLD;
                    if drag.moved {
                        (self.x, self.y) = (drag.origin.0 + dx, drag.origin.1 + dy);
                        conn.configure_window(self.window, &ConfigureWindowAux::new().x(self.x).y(self.y))?;
                    }
                }
            }
            XEvent::ButtonRelease(e) if e.event == self.window && e.detail == 1 => {
                if let Some(drag) = self.drag.take() {
                    if drag.moved {
                        return Ok(Some(Event::Moved { x: self.x, y: self.y }));
                    }
                    let (px, py) = (i32::from(e.event_x), i32::from(e.event_y));
                    if self.button.is_some_and(|(bx, by, bw, bh)| px >= bx && px < bx + bw && py >= by && py < by + bh) {
                        return Ok(Some(Event::CenterClicked));
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }
}

/// Runs of non-transparent pixels, one rectangle per run, row by row.
fn painted(bitmap: &Bitmap) -> Vec<Rectangle> {
    let width = bitmap.width as usize;
    let mut rects = Vec::new();
    for (y, row) in bitmap.data.chunks_exact(width * 4).enumerate() {
        let mut x = 0;
        while x < width {
            if row[x * 4 + 3] == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < width && row[x * 4 + 3] != 0 {
                x += 1;
            }
            rects.push(Rectangle { x: start as i16, y: y as i16, width: (x - start) as u16, height: 1 });
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painted_runs() {
        let mut bitmap = Bitmap::transparent(4, 2);
        for i in [0, 1, 3, 6] {
            bitmap.data[i * 4 + 3] = 255;
        }
        let rects: Vec<_> = painted(&bitmap).iter().map(|r| (r.x, r.y, r.width)).collect();
        assert_eq!(rects, [(0, 0, 2), (3, 0, 1), (2, 1, 1)]);
    }
}
