//! Drawings -> premultiplied BGRA bitmaps the overlay windows show as they are.

use super::crosshair::{Drawing, Piece};

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
    let mut canvas = Canvas::new(drawing.size, drawing.size);
    for piece in &drawing.pieces {
        match *piece {
            Piece::Rect { x, y, w, h, rgba } => canvas.rect(x, y, w, h, straight(rgba)),
            Piece::Ring { cx, cy, r, width, rgba } => canvas.ring(cx, cy, r, width, straight(rgba)),
        }
    }
    canvas.into_bitmap(opacity)
}

/// What the crosshair window shows while unlocked: a stone slab held by iron
/// cramps, the crosshair centred on it, and a centre button. Returns the bitmap
/// and the button's rectangle (x, y, w, h) so the platform can hit-test it.
pub fn slab(crosshair: &Bitmap, width: u32, height: u32) -> (Bitmap, (i32, i32, i32, i32)) {
    let (w, h) = (width as f32, height as f32);
    let mut canvas = Canvas::new(width, height);
    canvas.rect(0.0, 0.0, w, h, STONE);
    canvas.edge(0.0, 0.0, w, h, MARBLE_EDGE);
    canvas.image((width as i64 - crosshair.width as i64) / 2, (height as i64 - crosshair.height as i64) / 2, crosshair);
    for x in [(w / 4.0).floor() - CRAMP.0 / 2.0, (w * 3.0 / 4.0).floor() - CRAMP.0 / 2.0] {
        canvas.cramp(x, 0.0);
        canvas.cramp(x, h - CRAMP.1);
    }
    let button = (((w - BUTTON) / 2.0).floor(), h - BUTTON - BUTTON_MARGIN);
    canvas.button(button.0, button.1);
    (canvas.into_bitmap(1.0), (button.0 as i32, button.1 as i32, BUTTON as i32, BUTTON as i32))
}

/// Straight RGBA, each channel 0..1.
type Colour = [f32; 4];

const STONE: Colour = [11.0 / 255.0, 10.0 / 255.0, 9.0 / 255.0, 0.62];
const MARBLE_EDGE: Colour = [235.0 / 255.0, 229.0 / 255.0, 218.0 / 255.0, 0.22];
const MARBLE: Colour = [235.0 / 255.0, 229.0 / 255.0, 218.0 / 255.0, 1.0];
const IRON: Colour = [0x4a as f32 / 255.0, 0x3d as f32 / 255.0, 0x2c as f32 / 255.0, 1.0];
const RIVET: Colour = [0xa0 as f32 / 255.0, 0x8a as f32 / 255.0, 0x63 as f32 / 255.0, 1.0];
const BUTTON_FILL: Colour = [6.0 / 255.0, 6.0 / 255.0, 5.0 / 255.0, 1.0];
/// Width and height of an iron cramp.
const CRAMP: (f32, f32) = (24.0, 7.0);
const RIVET_SIZE: f32 = 3.0;
const BUTTON: f32 = 26.0;
const BUTTON_MARGIN: f32 = 10.0;

fn straight([r, g, b, a]: [u8; 4]) -> Colour {
    [r, g, b, a].map(|channel| f32::from(channel) / 255.0)
}

/// How much of [a0, a1] lies inside [b0, b1].
fn overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// Premultiplied RGBA, each channel 0..1, top row first.
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Colour>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Canvas {
        Canvas { width, height, pixels: vec![[0.0; 4]; (width * height) as usize] }
    }

    /// Source over, with the source already premultiplied.
    fn blend(&mut self, x: i64, y: i64, source: Colour) {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return;
        }
        let pixel = &mut self.pixels[(y as usize) * self.width as usize + x as usize];
        let keep = 1.0 - source[3];
        for (out, value) in pixel.iter_mut().zip(source) {
            *out = value + *out * keep;
        }
    }

    fn paint(&mut self, x: i64, y: i64, [r, g, b, a]: Colour, coverage: f32) {
        let alpha = a * coverage;
        if alpha > 0.0 {
            self.blend(x, y, [r * alpha, g * alpha, b * alpha, alpha]);
        }
    }

    /// Each pixel covered by the exact fraction of its area inside the rect.
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, colour: Colour) {
        let columns = (x.floor().max(0.0) as i64)..(x + w).ceil().min(self.width as f32) as i64;
        let rows = (y.floor().max(0.0) as i64)..(y + h).ceil().min(self.height as f32) as i64;
        for py in rows {
            let cover_y = overlap(py as f32, py as f32 + 1.0, y, y + h);
            for px in columns.clone() {
                let cover_x = overlap(px as f32, px as f32 + 1.0, x, x + w);
                self.paint(px, py, colour, cover_x * cover_y);
            }
        }
    }

    /// Coverage from each pixel centre's distance to the circle: the share of a
    /// one-pixel span across the ring's line that falls on the line.
    fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, colour: Colour) {
        let reach = r + width / 2.0 + 1.0;
        let (x0, x1) = ((cx - reach).floor().max(0.0) as i64, (cx + reach).ceil().min(self.width as f32) as i64);
        let (y0, y1) = ((cy - reach).floor().max(0.0) as i64, (cy + reach).ceil().min(self.height as f32) as i64);
        for py in y0..y1 {
            for px in x0..x1 {
                let d = (px as f32 + 0.5 - cx).hypot(py as f32 + 0.5 - cy) - r;
                self.paint(px, py, colour, overlap(d - 0.5, d + 0.5, -width / 2.0, width / 2.0));
            }
        }
    }

    /// A one-pixel line just inside the rect's border.
    fn edge(&mut self, x: f32, y: f32, w: f32, h: f32, colour: Colour) {
        self.rect(x, y, w, 1.0, colour);
        self.rect(x, y + h - 1.0, w, 1.0, colour);
        self.rect(x, y + 1.0, 1.0, h - 2.0, colour);
        self.rect(x + w - 1.0, y + 1.0, 1.0, h - 2.0, colour);
    }

    fn cramp(&mut self, x: f32, y: f32) {
        let (w, h) = CRAMP;
        self.rect(x, y, w, h, IRON);
        let top = y + ((h - RIVET_SIZE) / 2.0).floor();
        self.rect(x + RIVET_SIZE, top, RIVET_SIZE, RIVET_SIZE, RIVET);
        self.rect(x + w - 2.0 * RIVET_SIZE, top, RIVET_SIZE, RIVET_SIZE, RIVET);
    }

    /// The centre button: a dark square with a small marble cross in it.
    fn button(&mut self, x: f32, y: f32) {
        self.rect(x, y, BUTTON, BUTTON, BUTTON_FILL);
        self.edge(x, y, BUTTON, BUTTON, MARBLE_EDGE);
        let (c, gap, arm, t) = (BUTTON / 2.0, 2.0, 5.0, 2.0);
        let (cx, cy) = (x + c, y + c);
        self.rect(cx + gap, cy - t / 2.0, arm, t, MARBLE);
        self.rect(cx - gap - arm, cy - t / 2.0, arm, t, MARBLE);
        self.rect(cx - t / 2.0, cy + gap, t, arm, MARBLE);
        self.rect(cx - t / 2.0, cy - gap - arm, t, arm, MARBLE);
    }

    /// Lays a premultiplied BGRA bitmap over the canvas.
    fn image(&mut self, x: i64, y: i64, bitmap: &Bitmap) {
        for (i, bgra) in bitmap.data.chunks_exact(4).enumerate() {
            let (px, py) = ((i as u32 % bitmap.width) as i64, (i as u32 / bitmap.width) as i64);
            let [b, g, r, a] = [bgra[0], bgra[1], bgra[2], bgra[3]].map(|v| f32::from(v) / 255.0);
            if a > 0.0 {
                self.blend(x + px, y + py, [r, g, b, a]);
            }
        }
    }

    fn into_bitmap(self, opacity: f32) -> Bitmap {
        let opacity = opacity.clamp(0.0, 1.0);
        let byte = |value: f32| (value * opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        let data = self.pixels.iter().flat_map(|&[r, g, b, a]| [byte(b), byte(g), byte(r), byte(a)]).collect();
        Bitmap { width: self.width, height: self.height, data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [u8; 4] = [255, 255, 255, 255];

    fn drawing(size: u32, pieces: Vec<Piece>) -> Drawing {
        let pixels = crate::core::crosshair::Pixels { length: 0, thickness: 0.0, gap: 0, outline: 1 };
        Drawing { size, pieces, pixels }
    }

    fn rect(x: f32, y: f32, w: f32, h: f32, rgba: [u8; 4]) -> Piece {
        Piece::Rect { x, y, w, h, rgba }
    }

    fn at(bitmap: &Bitmap, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * bitmap.width + x) * 4) as usize;
        [bitmap.data[i], bitmap.data[i + 1], bitmap.data[i + 2], bitmap.data[i + 3]]
    }

    #[test]
    fn a_one_and_a_half_pixel_bar_paints_its_last_row_at_half() {
        let bitmap = rasterize(&drawing(6, vec![rect(1.0, 1.0, 3.0, 1.5, WHITE)]), 1.0);
        assert_eq!(at(&bitmap, 2, 1), [255; 4]);
        assert_eq!(at(&bitmap, 2, 2), [128; 4], "0.5 of 255 rounds to 128, premultiplied");
        assert_eq!(at(&bitmap, 2, 3), [0; 4]);
        assert_eq!(at(&bitmap, 0, 1), [0; 4]);
        assert_eq!(at(&bitmap, 4, 1), [0; 4]);
    }

    #[test]
    fn a_fractional_left_edge_covers_by_area() {
        let bitmap = rasterize(&drawing(4, vec![rect(0.25, 0.0, 1.0, 1.0, WHITE)]), 1.0);
        assert_eq!(at(&bitmap, 0, 0)[3], 191, "0.75 of 255");
        assert_eq!(at(&bitmap, 1, 0)[3], 64, "0.25 of 255");
    }

    #[test]
    fn bytes_are_premultiplied_bgra_and_opacity_scales_them() {
        let red = rasterize(&drawing(2, vec![rect(0.0, 0.0, 2.0, 2.0, [255, 0, 0, 128])]), 1.0);
        assert_eq!(at(&red, 1, 1), [0, 0, 128, 128]);
        let faded = rasterize(&drawing(2, vec![rect(0.0, 0.0, 2.0, 2.0, WHITE)]), 0.5);
        assert_eq!(at(&faded, 0, 0), [128; 4]);
        assert_eq!(rasterize(&drawing(2, vec![rect(0.0, 0.0, 2.0, 2.0, WHITE)]), 0.0).data, vec![0; 16]);
    }

    #[test]
    fn later_pieces_go_over_earlier_ones() {
        let half_black = [0, 0, 0, 128];
        let half_white = [255, 255, 255, 128];
        let bitmap = rasterize(&drawing(1, vec![rect(0.0, 0.0, 1.0, 1.0, half_black), rect(0.0, 0.0, 1.0, 1.0, half_white)]), 1.0);
        // alpha 0.502 + 0.502 * 0.498; colour only from the white
        assert_eq!(at(&bitmap, 0, 0), [128, 128, 128, 192]);
    }

    #[test]
    fn rects_outside_the_bitmap_are_clipped() {
        let bitmap = rasterize(&drawing(2, vec![rect(-5.0, -5.0, 6.0, 6.0, WHITE), rect(1.5, 1.0, 9.0, 9.0, WHITE)]), 1.0);
        assert_eq!(at(&bitmap, 0, 0), [255; 4]);
        assert_eq!(at(&bitmap, 1, 1), [128; 4]);
        assert_eq!(at(&bitmap, 1, 0), [0; 4]);
    }

    #[test]
    fn a_ring_is_anti_aliased_by_distance_from_its_circle() {
        let ring = Piece::Ring { cx: 10.5, cy: 10.5, r: 5.0, width: 2.0, rgba: WHITE };
        let bitmap = rasterize(&drawing(21, vec![ring]), 1.0);
        assert_eq!(at(&bitmap, 10, 15)[3], 255, "on the circle");
        assert_eq!(at(&bitmap, 13, 14)[3], 255, "3-4-5: on the circle too");
        assert_eq!(at(&bitmap, 10, 16)[3], 128, "one pixel out: half on the 2 px line");
        assert_eq!(at(&bitmap, 10, 17)[3], 0);
        assert_eq!(at(&bitmap, 10, 10)[3], 0, "the middle is empty");
    }

    #[test]
    fn a_whole_crosshair_rasterizes_to_its_drawing_size() {
        let drawing = crate::core::crosshair::draw(&crate::core::crosshair::Crosshair::default(), 1080);
        let bitmap = rasterize(&drawing, 1.0);
        assert_eq!((bitmap.width, bitmap.height, bitmap.data.len()), (drawing.size, drawing.size, (drawing.size * drawing.size * 4) as usize));
        let c = drawing.size / 2;
        // Default: 2 px thick, gap 4 -> the right bar starts at c - 1 + 2 + 4
        assert_eq!(at(&bitmap, c + 5, c), [50, 250, 50, 255]);
        assert_eq!(at(&bitmap, c + 4, c), [0; 4]);
    }

    #[test]
    fn the_slab_has_stone_an_edge_cramps_the_crosshair_and_a_button() {
        let crosshair = Bitmap { width: 10, height: 10, data: [255u8; 4].repeat(100) };
        let (bitmap, button) = slab(&crosshair, 200, 120);
        assert_eq!((bitmap.width, bitmap.height), (200, 120));
        assert_eq!(button, (87, 84, 26, 26), "bottom centre");

        assert_eq!(at(&bitmap, 5, 60), [6, 6, 7, 158], "stone, premultiplied at 0.62");
        assert_eq!(at(&bitmap, 0, 60)[3], 179, "the marble edge over the stone: 0.62 + 0.22 * 0.38");
        assert_eq!(at(&bitmap, 100, 60), [255; 4], "the crosshair in the middle");
        assert_eq!(at(&bitmap, 40, 1), [0x2c, 0x3d, 0x4a, 255], "a top cramp at a quarter of the width");
        assert_eq!(at(&bitmap, 42, 3), [0x63, 0x8a, 0xa0, 255], "its left rivet");
        assert_eq!(at(&bitmap, 140, 116), [0x2c, 0x3d, 0x4a, 255], "a bottom cramp at three quarters");

        let (x, y) = (button.0 as u32, button.1 as u32);
        assert_eq!(at(&bitmap, x + 3, y + 3), [5, 6, 6, 255], "button fill");
        assert_eq!(at(&bitmap, x, y)[3], 255, "a sharp corner, not cut away");
        assert_eq!(at(&bitmap, x + 16, y + 12), [218, 229, 235, 255], "the marble glyph's right arm");
    }
}
