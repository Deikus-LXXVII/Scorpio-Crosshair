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
    let x = fit_axis(area.x, area.width, width, scale);
    let y = fit_axis(area.y, area.height, height, scale);
    Placed { x: x.position, y: y.position, width: x.size, height: y.size, miss: x.miss.max(y.miss) }
}

struct Fit {
    position: i32,
    size: u32,
    miss: f64,
}

/// JavaScript's Math.round, which the reference placement was tuned with.
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

fn fit_axis(start: f64, length: f64, size: u32, scale: f64) -> Fit {
    let middle = (js_round(start * scale) * 2.0 + js_round(length * scale)) / 2.0;
    [size, size.saturating_sub(1), size.saturating_sub(2)]
        .map(|candidate| {
            let span = js_round(f64::from(candidate) * scale);
            let position = js_round((middle - span / 2.0) / scale);
            let miss = (position * scale + span / 2.0 - middle).abs();
            Fit { position: position as i32, size: candidate, miss }
        })
        .into_iter()
        .reduce(|best, fit| if fit.miss < best.miss - 1e-9 { fit } else { best })
        .expect("three candidates")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_100_percent_the_middle_is_the_displays_middle_exactly() {
        let areas = [(0.0, 0.0, 2560.0, 1440.0), (1920.0, 1080.0, 1920.0, 1080.0), (-1920.0, 0.0, 1920.0, 1080.0)];
        for (x, y, width, height) in areas {
            let b = centred(Area { x, y, width, height }, 201, 125, 1.0);
            let middle = (f64::from(b.x) + f64::from(b.width) / 2.0, f64::from(b.y) + f64::from(b.height) / 2.0);
            assert_eq!((middle, b.miss), ((x + width / 2.0, y + height / 2.0), 0.0), "{b:?}");
        }
    }

    #[test]
    fn at_125_150_and_175_percent_within_half_a_pixel() {
        for scale in [1.25_f64, 1.5, 1.75] {
            let area = Area { x: 0.0, y: 0.0, width: (2560.0 / scale).round(), height: (1440.0 / scale).round() };
            let b = centred(area, 160, 160, scale);
            assert!(b.miss <= 0.5, "{scale}: {b:?}");
            assert!(b.width >= 158 && b.height >= 158, "{scale}: {b:?}");
        }
    }

    #[test]
    fn at_200_percent_exact() {
        let b = centred(Area { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0 }, 160, 160, 2.0);
        assert_eq!((b.x, b.y, b.miss), (880, 460, 0.0));
    }
}
