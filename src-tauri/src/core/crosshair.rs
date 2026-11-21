//! Counter-Strike 2's crosshair: its settings, its share codes, and its shapes
//! in physical pixels for the player's screen.
//!
//! Share code byte layouts are the ones documented by akiver/csgo-sharecode (doc/code_layout.md, MIT).

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
        let e = &self.extra;
        Crosshair {
            style: resting_style(self.style),
            thickness: halves(self.thickness, 0.0, 32.0, 2.0),
            gap: self.gap.clamp(-3840, 3840),
            outline: self.outline.min(2),
            outline_width: self.outline_width.clamp(1, 6),
            made_for: self.made_for.clamp(240, 8640),
            extra: Extra {
                split_distance: e.split_distance.min(127),
                inner_split_alpha: hundredths(e.inner_split_alpha, 0.0, 1.0, 1.0),
                outer_split_alpha: hundredths(e.outer_split_alpha, 0.3, 1.0, 0.4),
                split_size_ratio: hundredths(e.split_size_ratio, 0.0, 1.0, 0.3),
                scope_dot_scale: hundredths(e.scope_dot_scale, 0.1, 2.0, 1.0),
                ..e.clone()
            },
            ..self.clone()
        }
    }

    pub fn shape(&self) -> Shape {
        let style = resting_style(self.style);
        STYLES.iter().find(|entry| entry.0 == style).map_or(Shape::Cross, |entry| entry.2)
    }
}

/// A dynamic style id becomes the static one it rests as.
pub fn resting_style(style: u8) -> u8 {
    match style {
        _ if STYLES.iter().any(|entry| entry.0 == style) => style,
        1 => 3,
        7 => 9,
        _ => 4,
    }
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

/// JavaScript's Math.round: halves go up, also below zero.
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

fn within(value: f32, min: f32, max: f32, fallback: f32) -> f64 {
    f64::from(if value.is_finite() { value.clamp(min, max) } else { fallback })
}

fn halves(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    (js_round(within(value, min, max, fallback) * 2.0) / 2.0) as f32
}

fn hundredths(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    (js_round(within(value, min, max, fallback) * 100.0) / 100.0) as f32
}

/* Share codes: a number in base 57 written least significant digit first; its
bytes are big-endian and byte 0 is the sum of the rest, mod 256. */

const ALPHABET: &[u8; 57] = b"ABCDEFGHJKLMNOPQRSTUVWXYZabcdefhijkmnopqrstuvwxyz23456789";

fn digit(c: u8) -> Option<u32> {
    ALPHABET.iter().position(|&a| a == c).map(|i| i as u32)
}

fn to_bytes(digits: &[u8], size: usize) -> Option<Vec<u8>> {
    let mut bytes = vec![0u8; size];
    for &c in digits.iter().rev() {
        let mut carry = digit(c)?;
        for byte in bytes.iter_mut().rev() {
            let t = u32::from(*byte) * 57 + carry;
            *byte = t as u8;
            carry = t >> 8;
        }
        if carry != 0 {
            return None;
        }
    }
    Some(bytes)
}

fn to_digits(bytes: &[u8], count: usize) -> String {
    let mut value = bytes.to_vec();
    (0..count)
        .map(|_| {
            let mut rest = 0u32;
            for byte in value.iter_mut() {
                let t = (rest << 8) | u32::from(*byte);
                *byte = (t / 57) as u8;
                rest = t % 57;
            }
            ALPHABET[rest as usize] as char
        })
        .collect()
}

fn sum_ok(bytes: &[u8]) -> bool {
    bytes[0] == bytes[1..].iter().fold(0u8, |total, &b| total.wrapping_add(b))
}

fn uint32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn bits(value: u32, from: u32, count: u32) -> u32 {
    (value >> from) & ((1 << count) - 1)
}

/// Round half to even, as CS:GO did when it scaled sizes.
fn round_even(value: f64) -> f64 {
    let floor = value.floor();
    if (value - floor - 0.5).abs() < 1e-9 {
        floor + floor.rem_euclid(2.0)
    } else {
        value.round()
    }
}

/// cs2-v1, CS2 since 30/09/2026: 32 bytes behind "CS".
fn read_cs2(b: &[u8]) -> Crosshair {
    let field = uint32(b, 18);
    Crosshair {
        made_for: u16::from_le_bytes([b[2], b[3]]),
        style: b[4] & 0x1f,
        dot: b[4] & 0x40 != 0,
        t_shape: b[4] & 0x80 != 0,
        color: [b[5], b[6], b[7]],
        alpha: b[8],
        outline_color: [b[9], b[10], b[11]],
        outline_alpha: b[12],
        thickness: f32::from(b[13] & 0x3f),
        outline: b[13] >> 6,
        gap: i16::from_le_bytes([b[14], b[15]]),
        length: b[16],
        extra: Extra {
            follow_recoil: b[4] & 0x20 != 0,
            spread_limit: b[17],
            split_distance: bits(field, 0, 7) as u8,
            inner_split_alpha: bits(field, 7, 7) as f32 / 100.0,
            outer_split_alpha: (bits(field, 14, 7) + 30) as f32 / 100.0,
            split_size_ratio: bits(field, 21, 7) as f32 / 100.0,
            scope_dot_uses_color: bits(field, 28, 1) == 1,
            scope_dot_scale: (f32::from(b[22]) + 10.0) / 100.0,
        },
        ..Crosshair::default()
    }
}

/// legacy-v3 (23/09/2026) and legacy-v4 (24/09/2026): pixels at `made_for`.
fn read_pixel_era(b: &[u8], version: u8) -> Crosshair {
    let field = uint32(b, 10);
    Crosshair {
        style: b[2] & 0x0f,
        dot: b[2] & 0x40 != 0,
        t_shape: b[2] & 0x80 != 0,
        outline: if version == 3 { u8::from(b[2] & 0x20 != 0) } else { bits(field, 28, 2) as u8 },
        color: [b[3], b[4], b[5]],
        alpha: b[6],
        gap: i16::from(b[7]),
        length: b[8],
        thickness: bits(field, 23, 5) as f32,
        made_for: u16::from_le_bytes([b[14], b[15]]),
        extra: Extra {
            follow_recoil: b[2] & 0x10 != 0,
            spread_limit: b[9],
            split_distance: bits(field, 0, 7) as u8,
            inner_split_alpha: bits(field, 7, 5) as f32 * 0.05,
            outer_split_alpha: 0.3 + bits(field, 12, 4) as f32 * 0.05,
            split_size_ratio: bits(field, 16, 7) as f32 / 100.0,
            ..Extra::default()
        },
        ..Crosshair::default()
    }
}

/// legacy-v1, CS:GO and CS2 before 23/09/2026: sizes in tenths, drawn by the
/// old renderer. Placed at `screen_height` with the community's reading of that
/// renderer: length and thickness scale by height / 480, the gap is 4 + gap.
fn read_tenths(b: &[u8], screen_height: u32) -> Crosshair {
    let scale = f64::from(screen_height) / 480.0;
    let scaled = |tenths: u8| round_even(f64::from(tenths) / 10.0 * scale);
    Crosshair {
        style: (b[13] & 0x0e) >> 1,
        dot: b[13] & 0x10 != 0,
        t_shape: b[13] & 0x80 != 0,
        color: [b[4], b[5], b[6]],
        alpha: if b[13] & 0x40 != 0 { b[7] } else { 255 },
        outline: u8::from(b[10] & 0x08 != 0),
        outline_width: (b[3] / 2 + b[3] % 2).max(1),
        gap: (4.0 + f64::from(b[2] as i8) / 10.0).trunc() as i16,
        length: scaled(b[14]) as u8,
        thickness: scaled(b[12]).max(1.0) as f32,
        made_for: screen_height.min(u32::from(u16::MAX)) as u16,
        extra: Extra {
            follow_recoil: b[8] & 0x80 != 0,
            split_distance: b[8] & 0x07,
            inner_split_alpha: f32::from(b[10] >> 4) / 10.0,
            outer_split_alpha: (f32::from(b[11] & 0x0f) / 10.0).max(0.3),
            split_size_ratio: f32::from(b[11] >> 4) / 10.0,
            ..Extra::default()
        },
        ..Crosshair::default()
    }
}

/// The 44 digits of a cs2-v1 code.
fn cs2_digits(code: &str) -> Option<&[u8]> {
    let digits = code.strip_prefix("CS")?.as_bytes();
    let ok = !code.starts_with("CSGO") && digits.len() == 44 && digits.iter().all(|&c| digit(c).is_some());
    ok.then_some(digits)
}

/// The 25 digits of a CSGO-xxxxx-xxxxx-xxxxx-xxxxx-xxxxx code (dashes optional).
fn legacy_digits(code: &str) -> Option<Vec<u8>> {
    let mut rest = code.strip_prefix("CSGO")?.as_bytes();
    let mut digits = Vec::with_capacity(25);
    for _ in 0..5 {
        rest = rest.strip_prefix(b"-").unwrap_or(rest);
        let group = rest.get(..5)?;
        if !group.iter().all(|&c| digit(c).is_some()) {
            return None;
        }
        digits.extend_from_slice(group);
        rest = &rest[5..];
    }
    rest.is_empty().then_some(digits)
}

fn read_raw(code: &str, screen_height: u32) -> Option<(&'static str, Crosshair)> {
    if let Some(digits) = cs2_digits(code) {
        let bytes = to_bytes(digits, 32)?;
        return (sum_ok(&bytes) && bytes[1] == 1).then(|| ("cs2-v1", read_cs2(&bytes)));
    }
    let bytes = to_bytes(&legacy_digits(code)?, 18)?;
    match bytes[1] {
        _ if !sum_ok(&bytes) => None,
        1 => Some(("legacy-v1", read_tenths(&bytes, screen_height))),
        3 => Some(("legacy-v3", read_pixel_era(&bytes, 3))),
        4 => Some(("legacy-v4", read_pixel_era(&bytes, 4))),
        _ => None,
    }
}

/// Reads any CS2 or CS:GO crosshair code. `screen_height` places an old
/// (legacy-v1, sizes in tenths) code at the player's resolution.
pub fn read_code(code: &str, screen_height: u32) -> Result<Decoded, NotACode> {
    let (format, raw) = read_raw(code.trim(), screen_height).ok_or(NotACode)?;
    Ok(Decoded { format, code_style: raw.style, crosshair: raw.normalized() })
}

/// The cs2-v1 code CS2 imports today.
pub fn write_code(crosshair: &Crosshair) -> String {
    let s = crosshair.normalized();
    let e = &s.extra;
    let percent = |value: f32| js_round(f64::from(value) * 100.0) as u32;
    let field = u32::from(e.split_distance)
        | percent(e.inner_split_alpha) << 7
        | (percent(e.outer_split_alpha) - 30) << 14
        | percent(e.split_size_ratio) << 21
        | u32::from(e.scope_dot_uses_color) << 28;
    let flags = if e.follow_recoil { 0x20 } else { 0 } | if s.dot { 0x40 } else { 0 } | if s.t_shape { 0x80 } else { 0 };
    let mut bytes = [0u8; 32];
    bytes[1] = 1;
    bytes[2..4].copy_from_slice(&s.made_for.to_le_bytes());
    bytes[4] = s.style | flags;
    bytes[5..8].copy_from_slice(&s.color);
    bytes[8] = s.alpha;
    bytes[9..12].copy_from_slice(&s.outline_color);
    bytes[12] = s.outline_alpha;
    // CS2 keeps whole pixels; a half step rounds
    bytes[13] = js_round(f64::from(s.thickness)) as u8 | s.outline << 6;
    bytes[14..16].copy_from_slice(&s.gap.to_le_bytes());
    bytes[16] = s.length;
    bytes[17] = e.spread_limit;
    bytes[18..22].copy_from_slice(&field.to_le_bytes());
    bytes[22] = (percent(e.scope_dot_scale) - 10) as u8;
    bytes[0] = bytes[1..].iter().fold(0u8, |total, &b| total.wrapping_add(b));
    format!("CS{}", to_digits(&bytes, 44))
}

/* Drawing */

/// Sizes in physical pixels on a screen `screen_height` pixels tall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pixels {
    pub length: i32,
    /// Quarter pixels; the fraction is drawn as a soft far edge.
    pub thickness: f32,
    pub gap: i32,
    pub outline: i32,
}

/// CS2 scales every size by the screen's height over the height it was made at.
pub fn pixels(crosshair: &Crosshair, screen_height: u32) -> Pixels {
    let s = crosshair.normalized();
    let k = f64::from(screen_height) / f64::from(s.made_for);
    Pixels {
        length: js_round(f64::from(s.length) * k) as i32,
        thickness: (js_round(f64::from(s.thickness) * k * 4.0) / 4.0) as f32,
        gap: js_round(f64::from(s.gap) * k) as i32,
        outline: (js_round(f64::from(s.outline_width) * k) as i32).max(1),
    }
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

/// x, y, w, h
type Bar = [f32; 4];

fn push(bars: &mut Vec<Bar>, x: f32, y: f32, w: f32, h: f32) {
    if w > 0.0 && h > 0.0 {
        bars.push([x, y, w, h]);
    }
}

/// The four bars beyond the gap; the T shape drops the top one.
fn cross_bars(bars: &mut Vec<Bar>, from: f32, base: f32, t: f32, p: &Pixels, t_shape: bool) {
    let (gap, length) = (p.gap as f32, p.length as f32);
    push(bars, from + base + gap, from, length, t);
    push(bars, from - gap - length, from, length, t);
    push(bars, from, from + base + gap, t, length);
    if !t_shape {
        push(bars, from, from - gap - length, t, length);
    }
}

/// A frame around the gap; the quad keeps only its corners. Its sides are
/// whole pixels, and it is placed from its own width so it stays centred
/// like a cross of that width.
fn frame_bars(bars: &mut Vec<Bar>, c: f32, t: f32, p: &Pixels, quad: bool) {
    let gap = p.gap.max(0) as f32;
    let w = t.round().max(1.0);
    let from = c - (w / 2.0).floor();
    let inner = from - gap;
    let span = w + 2.0 * gap;
    let outer = span + 2.0 * w;
    let (left, top, right, bottom) = (inner - w, inner - w, inner + span, inner + span);
    if !quad {
        push(bars, left, top, outer, w);
        push(bars, left, bottom, outer, w);
        push(bars, left, inner, w, span);
        push(bars, right, inner, w, span);
        return;
    }
    let arm = outer.min((2.0 * w).max(p.length as f32));
    let far = left + outer - arm;
    for (x, y) in [(left, top), (far, top), (left, bottom), (far, bottom)] {
        push(bars, x, y, arm, w);
    }
    for (x, y) in [(left, top), (right, top), (left, far), (right, far)] {
        push(bars, x, y, w, arm);
    }
}

fn outline_of(bars: &[Bar], mode: u8, o: f32) -> Vec<Bar> {
    let mut edges = Vec::new();
    for &[x, y, w, h] in bars {
        match mode {
            1 => edges.push([x - o, y - o, w + 2.0 * o, h + 2.0 * o]),
            2 => edges.extend([[x - o, y - o, w + o, o], [x - o, y, o, h]]),
            _ => {}
        }
    }
    edges
}

pub fn draw(crosshair: &Crosshair, screen_height: u32) -> Drawing {
    let s = crosshair.normalized();
    let p = pixels(&s, screen_height);
    let shape = s.shape();
    let t = if shape == Shape::Dot { p.thickness.max(1.0) } else { p.thickness };
    // Bars start on whole pixels; a fractional thickness adds a soft far edge
    let base = t.floor();
    let reach = p.gap.max(0) + p.length + t.ceil() as i32 + p.outline + 2;
    let size = 2 * (reach + 2);
    let c = (size / 2) as f32;
    let from = c - (base / 2.0).floor();
    let mut bars = Vec::new();
    match shape {
        Shape::Cross if t > 0.0 => cross_bars(&mut bars, from, base, t, &p, s.t_shape),
        Shape::Square | Shape::Quad if t > 0.0 => frame_bars(&mut bars, c, t, &p, shape == Shape::Quad),
        _ => {}
    }
    if s.dot || shape == Shape::Dot {
        let d = t.max(1.0);
        push(&mut bars, from, from, d, d);
    }
    let edges = outline_of(&bars, s.outline, p.outline as f32);
    let [r, g, b] = s.color;
    let [or, og, ob] = s.outline_color;
    let rect = |rgba: [u8; 4]| move |[x, y, w, h]: Bar| Piece::Rect { x, y, w, h, rgba };
    let mut pieces: Vec<Piece> = edges.into_iter().map(rect([or, og, ob, s.outline_alpha])).collect();
    pieces.extend(bars.into_iter().map(rect([r, g, b, s.alpha])));
    if shape == Shape::Circle && t > 0.0 {
        let radius = (p.gap as f32 + p.length as f32 / 2.0).max(1.0);
        // A ring has no top or left edge, so both outline modes draw it whole.
        if s.outline != 0 {
            let width = t + 2.0 * p.outline as f32;
            pieces.insert(0, Piece::Ring { cx: c, cy: c, r: radius, width, rgba: [or, og, ob, s.outline_alpha] });
        }
        pieces.push(Piece::Ring { cx: c, cy: c, r: radius, width: t, rgba: [r, g, b, s.alpha] });
    }
    Drawing { size: size as u32, pieces, pixels: p }
}

fn css([r, g, b, a]: [u8; 4]) -> String {
    format!("rgba({r},{g},{b},{:.3})", f32::from(a) / 255.0)
}

/// The same drawing as SVG, for previews in the settings window.
pub fn to_svg(drawing: &Drawing) -> String {
    let size = drawing.size;
    let mut svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">"#);
    // Consecutive rects of one colour share a group, so the outline comes first
    let mut group: Option<[u8; 4]> = None;
    for piece in &drawing.pieces {
        match *piece {
            Piece::Rect { x, y, w, h, rgba } => {
                if group != Some(rgba) {
                    if group.is_some() {
                        svg.push_str("</g>");
                    }
                    svg.push_str(&format!(r#"<g fill="{}">"#, css(rgba)));
                    group = Some(rgba);
                }
                svg.push_str(&format!(r#"<rect x="{x}" y="{y}" width="{w}" height="{h}"/>"#));
            }
            Piece::Ring { cx, cy, r, width, rgba } => {
                if group.take().is_some() {
                    svg.push_str("</g>");
                }
                svg.push_str(&format!(r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="{}" stroke-width="{width}"/>"#, css(rgba)));
            }
        }
    }
    if group.is_some() {
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    svg
}
