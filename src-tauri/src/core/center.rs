//! Where a window goes so its middle sits on the display's middle, between the
//! two centre pixels where a game draws its own crosshair.
//!
//! Reference behaviour: `reference/js/center.js`. Windows are placed in whole
//! logical pixels; at 125% or 150% scaling each axis also tries one and two
//! logical pixels smaller and keeps the size whose middle lands closest.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// How far the middle is off, in physical pixels.
    pub miss: f64,
}

pub fn centred(area: Area, width: u32, height: u32, scale: f64) -> Placed {
    todo!()
}
