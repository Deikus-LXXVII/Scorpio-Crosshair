//! Counter-Strike 2's crosshair: its settings, its share codes, and its shapes
//! in physical pixels for the player's screen.
//!
//! Reference behaviour: `reference/js/crosshair.js` and its tests in
//! `reference/test/unit/crosshair.test.js`. Share code byte layouts are the
//! ones documented by akiver/csgo-sharecode (doc/code_layout.md, MIT).

use serde::{Deserialize, Serialize};

/// The static styles Scorpio offers; a dynamic style in an imported code
/// becomes the static style it rests as (0, 2, 5 -> 4; 1 -> 3; 7 -> 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Cross,
    Circle,
    Dot,
    Square,
    Quad,
}

/// (CS2 style id, display name, shape), in the order the UI lists them.
pub const STYLES: [(u8, &str, Shape); 5] = [
    (4, "Static Cross", Shape::Cross),
    (3, "Static Circle", Shape::Circle),
    (6, "Dot Only", Shape::Dot),
    (8, "Static Square", Shape::Square),
    (9, "Static Quad", Shape::Quad),
];

/// Values a share code carries that an overlay does not draw; kept so a code
/// survives a round trip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Extra {
    pub follow_recoil: bool,
    pub spread_limit: u8,
    pub split_distance: u8,
    pub inner_split_alpha: f32,
    pub outer_split_alpha: f32,
    pub split_size_ratio: f32,
    pub scope_dot_scale: f32,
    pub scope_dot_uses_color: bool,
}

/// The settings, in CS2's units. `normalized()` clamps every field into range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Crosshair {
    pub style: u8,
    pub length: u8,
    /// Half steps (Scorpio's own precision); share codes carry it rounded.
    pub thickness: f32,
    pub gap: i16,
    pub dot: bool,
    pub t_shape: bool,
    pub color: [u8; 3],
    pub alpha: u8,
    /// 0 none, 1 full, 2 top and left edges only.
    pub outline: u8,
    /// Scorpio's own: CS2 draws outlines 1 px wide. Not in share codes.
    pub outline_width: u8,
    pub outline_color: [u8; 3],
    pub outline_alpha: u8,
    /// The screen height the sizes were made at.
    pub made_for: u16,
    pub extra: Extra,
}

impl Default for Extra {
    fn default() -> Self {
        Extra {
            follow_recoil: false,
            spread_limit: 255,
            split_distance: 7,
            inner_split_alpha: 1.0,
            outer_split_alpha: 0.4,
            split_size_ratio: 0.3,
            scope_dot_scale: 1.0,
            scope_dot_uses_color: false,
        }
    }
}

impl Default for Crosshair {
    fn default() -> Self {
        Crosshair {
            style: 4,
            length: 8,
            thickness: 2.0,
            gap: 4,
            dot: false,
            t_shape: false,
            color: [50, 250, 50],
            alpha: 255,
            outline: 0,
            outline_width: 1,
            outline_color: [0, 0, 0],
            outline_alpha: 255,
            made_for: 1080,
            extra: Extra::default(),
        }
    }
}

impl Crosshair {
    /// Every field clamped into range; style set to a static style.
    pub fn normalized(&self) -> Crosshair {
        todo!()
    }

    pub fn shape(&self) -> Shape {
        todo!()
    }
}

/// A dynamic style id becomes the static one it rests as.
pub fn resting_style(style: u8) -> u8 {
    todo!()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// "cs2-v1", "legacy-v1", "legacy-v3" or "legacy-v4".
    pub format: &'static str,
    /// The style the code had before a dynamic one was set to rest.
    pub code_style: u8,
    pub crosshair: Crosshair,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotACode;

/// Reads any CS2 or CS:GO crosshair code. `screen_height` places an old
/// (legacy-v1, sizes in tenths) code at the player's resolution.
pub fn read_code(code: &str, screen_height: u32) -> Result<Decoded, NotACode> {
    todo!()
}

/// The cs2-v1 code CS2 imports today.
pub fn write_code(crosshair: &Crosshair) -> String {
    todo!()
}

/// Sizes in physical pixels on a screen `screen_height` pixels tall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pixels {
    pub length: i32,
    /// Quarter pixels; the fraction is drawn as a soft far edge.
    pub thickness: f32,
    pub gap: i32,
    pub outline: i32,
}

pub fn pixels(crosshair: &Crosshair, screen_height: u32) -> Pixels {
    todo!()
}

/// One filled piece of the drawing, in physical pixels; colour is straight
/// (not premultiplied) RGBA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    Rect { x: f32, y: f32, w: f32, h: f32, rgba: [u8; 4] },
    Ring { cx: f32, cy: f32, r: f32, width: f32, rgba: [u8; 4] },
}

/// A square `size` pixels wide whose centre is the crosshair's centre (between
/// the two middle pixels). Outline pieces come first.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub size: u32,
    pub pieces: Vec<Piece>,
    pub pixels: Pixels,
}

pub fn draw(crosshair: &Crosshair, screen_height: u32) -> Drawing {
    todo!()
}

/// The same drawing as SVG, for previews in the settings window.
pub fn to_svg(drawing: &Drawing) -> String {
    todo!()
}
