//! Drawings -> premultiplied BGRA bitmaps the overlay windows show as they are.

use super::crosshair::Drawing;

/// Premultiplied BGRA, top row first, `width * 4` bytes per row: the layout
/// Windows' `UpdateLayeredWindow` and an X11 ARGB visual both take.
#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Bitmap {
    pub fn transparent(width: u32, height: u32) -> Bitmap {
        Bitmap { width, height, data: vec![0; (width * height * 4) as usize] }
    }
}

/// Paints the pieces in order with "source over", each rect edge covering a
/// pixel by its exact fractional area, rings anti-aliased by distance.
/// `opacity` (0..1) scales the whole result.
pub fn rasterize(drawing: &Drawing, opacity: f32) -> Bitmap {
    todo!()
}

/// What the crosshair window shows while unlocked: a stone slab held by iron
/// cramps, the crosshair centred on it, and a centre button. Returns the bitmap
/// and the button's rectangle (x, y, w, h) so the platform can hit-test it.
pub fn slab(crosshair: &Bitmap, width: u32, height: u32) -> (Bitmap, (i32, i32, i32, i32)) {
    todo!()
}
