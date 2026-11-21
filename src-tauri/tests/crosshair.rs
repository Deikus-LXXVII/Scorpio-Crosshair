//! Share codes and drawing, checked against published reference codes.

use scorpio_lib::core::crosshair::{draw, pixels, read_code, resting_style, to_svg, write_code, Crosshair, Decoded, Piece, Pixels};
use serde_json::Value;

fn vectors() -> Vec<(String, Value)> {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/cs2-sharecodes.json")).unwrap();
    fixture["vectors"].as_array().unwrap().iter().map(|v| (v["code"].as_str().unwrap().to_owned(), v["expected"].clone())).collect()
}

fn format_of(expected: &Value) -> &str {
    expected["format"].as_str().unwrap()
}

fn code_starting(prefix: &str) -> (String, Value) {
    vectors().into_iter().find(|(code, _)| code.starts_with(prefix)).unwrap()
}

fn int(v: &Value) -> i64 {
    v.as_i64().unwrap()
}

/// Scorpio's fields as the fixture names them (akiver/csgo-sharecode's names).
#[derive(Debug, PartialEq)]
struct View {
    format: String,
    style: u8,
    dot: bool,
    t_shape: bool,
    color: [u8; 3],
    alpha: u8,
    gap: i64,
    length: i64,
    thickness: f64,
    made_for: i64,
    outline: u8,
}

fn view(d: &Decoded) -> View {
    let s = &d.crosshair;
    View {
        format: d.format.to_owned(),
        style: s.style,
        dot: s.dot,
        t_shape: s.t_shape,
        color: s.color,
        alpha: s.alpha,
        gap: i64::from(s.gap),
        length: i64::from(s.length),
        thickness: f64::from(s.thickness),
        made_for: i64::from(s.made_for),
        outline: s.outline,
    }
}

fn expected_view(e: &Value) -> View {
    let outline = e.get("outlineMode").map_or_else(|| u8::from(e["outlineEnabled"].as_bool().unwrap()), |mode| int(mode) as u8);
    View {
        format: format_of(e).to_owned(),
        style: resting_style(int(&e["style"]) as u8),
        dot: e["centerDotEnabled"].as_bool().unwrap(),
        t_shape: e["tStyleEnabled"].as_bool().unwrap(),
        color: [int(&e["red"]) as u8, int(&e["green"]) as u8, int(&e["blue"]) as u8],
        alpha: int(&e["alpha"]) as u8,
        gap: int(&e["gap"]),
        length: int(&e["length"]),
        thickness: e["thickness"].as_f64().unwrap(),
        made_for: int(&e["screenHeight"]),
        outline,
    }
}

#[test]
fn every_pixel_era_reference_code_reads_to_its_published_values() {
    let pixel_era: Vec<_> = vectors().into_iter().filter(|(_, e)| format_of(e) != "legacy-v1").collect();
    assert_eq!(pixel_era.len(), 12);
    for (code, expected) in pixel_era {
        assert_eq!(view(&read_code(&code, 1080).unwrap()), expected_view(&expected), "{code}");
    }
}

#[test]
fn a_cs2_code_comes_back_character_for_character() {
    let statics: Vec<_> = vectors()
        .into_iter()
        .filter(|(_, e)| format_of(e) == "cs2-v1" && [3, 4, 6, 8, 9].contains(&int(&e["style"])))
        .collect();
    assert!(!statics.is_empty());
    for (code, _) in statics {
        assert_eq!(write_code(&read_code(&code, 1080).unwrap().crosshair), code);
    }

    let (dynamic, _) = vectors().into_iter().find(|(_, e)| format_of(e) == "cs2-v1" && int(&e["style"]) == 2).unwrap();
    let decoded = read_code(&dynamic, 1080).unwrap();
    assert_eq!((decoded.code_style, decoded.crosshair.style), (2, 4), "a dynamic style comes in at rest");
}

#[test]
fn the_split_recoil_and_scope_values_survive_a_round_trip() {
    let (code, e) = code_starting("CSAB6");
    let again = read_code(&write_code(&read_code(&code, 1080).unwrap().crosshair), 1080).unwrap();
    let x = &again.crosshair.extra;
    let f = |key: &str| e[key].as_f64().unwrap() as f32;
    assert_eq!(x.follow_recoil, e["followRecoil"].as_bool().unwrap());
    assert_eq!(i64::from(x.spread_limit), int(&e["dynamicSpreadLimit"]));
    assert_eq!(i64::from(x.split_distance), int(&e["splitDistance"]));
    assert_eq!(x.inner_split_alpha, f("innerSplitAlpha"));
    assert_eq!(x.outer_split_alpha, f("outerSplitAlpha"));
    assert_eq!(x.split_size_ratio, f("splitSizeRatio"));
    assert_eq!(x.scope_dot_scale, f("scopeDotScale"));
    assert_eq!(x.scope_dot_uses_color, e["scopeDotUseCrosshairColor"].as_bool().unwrap());
}

#[test]
fn an_old_csgo_code_is_placed_at_the_players_screen_as_the_old_renderer_drew_it() {
    let code = "CSGO-WsnnD-eHaMw-QNDf9-oxuDh-ydOUD";
    assert!(vectors().iter().any(|(c, _)| c == code));
    let at480 = read_code(code, 480).unwrap();
    let s = &at480.crosshair;
    assert_eq!(at480.format, "legacy-v1");
    assert_eq!((s.length, s.thickness, s.gap, s.alpha, s.outline), (10, 1.0, 1, 200, 1));
    assert_eq!(read_code(code, 1080).unwrap().crosshair.length, 22, "size 10 at 1080p is 22.5, which rounds to even");
}

#[test]
fn every_legacy_v1_reference_code_reads() {
    for (code, e) in vectors().into_iter().filter(|(_, e)| format_of(e) == "legacy-v1") {
        let decoded = read_code(&code, 1080).unwrap();
        assert_eq!(decoded.format, "legacy-v1", "{code}");
        assert_eq!(decoded.code_style, int(&e["style"]) as u8, "{code}");
        assert_eq!(decoded.crosshair.color, [int(&e["red"]) as u8, int(&e["green"]) as u8, int(&e["blue"]) as u8], "{code}");
    }
}

#[test]
fn anything_else_is_refused() {
    let first = &vectors()[0].0;
    let bad_sum = format!("{}A", &first[..first.len() - 1]);
    for bad in ["", "hello", "CSGO-12345-12345-12345-12345-12345", &bad_sum, &format!("CS{}", "A".repeat(44))] {
        assert!(read_code(bad, 1080).is_err(), "{bad}");
    }
}

#[test]
fn a_code_with_spaces_around_it_reads() {
    let (code, _) = code_starting("CSAB6");
    assert!(read_code(&format!("  {code}\n"), 1080).is_ok());
}

#[test]
fn settings_are_kept_in_range() {
    // The reference also feeds length 999 and colour [300, -4, "x"]; Rust's
    // u8 fields cannot hold those, so the type system does that part.
    let s = Crosshair { thickness: 1.3, gap: -9999, outline: 7, outline_width: 0, made_for: 10, style: 2, ..Crosshair::default() }.normalized();
    assert_eq!((s.thickness, s.gap, s.outline, s.outline_width, s.made_for, s.style), (1.5, -3840, 2, 1, 240, 4));
    let wild = Crosshair { thickness: f32::NAN, style: 31, ..Crosshair::default() }.normalized();
    assert_eq!((wild.thickness, wild.style), (2.0, 4));
}

#[test]
fn sizes_scale_with_the_screen_thickness_keeps_quarter_pixels() {
    let at720 = Crosshair { length: 8, thickness: 2.0, gap: 4, made_for: 720, ..Crosshair::default() };
    assert_eq!(pixels(&at720, 1440), Pixels { length: 16, thickness: 4.0, gap: 8, outline: 2 });
    let thin = Crosshair { length: 8, thickness: 1.0, gap: 4, made_for: 1080, ..Crosshair::default() };
    assert_eq!(pixels(&thin, 1440), Pixels { length: 11, thickness: 1.25, gap: 5, outline: 1 });
}

fn bars(crosshair: &Crosshair) -> (Vec<[f32; 4]>, f32, u32) {
    let drawing = draw(crosshair, 1080);
    let rects = drawing
        .pieces
        .iter()
        .map(|piece| match *piece {
            Piece::Rect { x, y, w, h, .. } => [x, y, w, h],
            Piece::Ring { .. } => panic!("no rings in these shapes"),
        })
        .collect();
    (rects, (drawing.size / 2) as f32, drawing.size)
}

fn cross(length: u8, thickness: f32, gap: i16) -> Crosshair {
    Crosshair { style: 4, length, thickness, gap, made_for: 1080, ..Crosshair::default() }
}

#[test]
fn a_static_cross_four_bars_beyond_the_gap_all_starting_on_whole_pixels() {
    let (bars, c, size) = bars(&Crosshair { dot: true, ..cross(8, 2.0, 4) });
    assert_eq!(size % 2, 0);
    assert_eq!(bars[0], [c - 1.0 + 2.0 + 4.0, c - 1.0, 8.0, 2.0]);
    assert_eq!(*bars.last().unwrap(), [c - 1.0, c - 1.0, 2.0, 2.0], "the dot is as wide as the lines");
    assert_eq!(bars.len(), 5);

    let (half, _, _) = self::bars(&cross(8, 1.5, 4));
    assert!(half.iter().all(|b| b[0].fract() == 0.0 && b[1].fract() == 0.0), "a half step only softens the far edge");
    assert_eq!(half[0][3], 1.5);
}

#[test]
fn the_t_shape_drops_the_top_bar_not_another_one() {
    let (bars, c, _) = bars(&Crosshair { t_shape: true, ..cross(6, 2.0, 3) });
    assert_eq!(bars.len(), 3);
    assert!(bars.iter().all(|b| b[1] + b[3] > c - 1.0), "nothing above the centre block");
}

#[test]
fn outline_width_grows_the_full_outline_and_the_half_outlines_two_edges() {
    let dot = |outline, outline_width| Crosshair { style: 6, thickness: 4.0, outline, outline_width, made_for: 1080, ..Crosshair::default() };
    let full = draw(&dot(1, 3), 1080);
    match full.pieces[0] {
        Piece::Rect { x, y, w, h, .. } => assert_eq!((x == y, w, h), (true, 10.0, 10.0), "a 4 px dot with 3 px all round"),
        Piece::Ring { .. } => panic!("expected a rect"),
    }
    assert!(to_svg(&full).contains(r#"width="10" height="10"/>"#));
    assert!(to_svg(&draw(&dot(2, 2), 1080)).contains(r#"height="2"/>"#), "a 2 px top edge");
    assert_eq!(write_code(&Crosshair { outline_width: 5, ..Crosshair::default() }), write_code(&Crosshair::default()), "not part of a share code");
}

#[test]
fn the_svg_draws_the_outline_first_and_rings_as_strokes() {
    let outlined = Crosshair { outline: 1, outline_color: [1, 2, 3], color: [200, 100, 50], ..Crosshair::default() };
    let svg = to_svg(&draw(&outlined, 1080));
    let outline_at = svg.find("rgba(1,2,3,1.000)").unwrap();
    assert!(outline_at < svg.find("rgba(200,100,50,1.000)").unwrap());
    assert!(svg.starts_with("<svg") && svg.ends_with("</g></svg>"));

    let circle = to_svg(&draw(&Crosshair { style: 3, ..Crosshair::default() }, 1080));
    assert!(circle.contains(r#"<circle cx="#) && circle.contains(r#"fill="none""#), "{circle}");
}

fn frame_centre(crosshair: &Crosshair) -> (f32, f32) {
    let d = draw(crosshair, 1080);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for piece in &d.pieces {
        if let Piece::Rect { x, y, w, h, .. } = piece {
            (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(x + w), y1.max(y + h));
        }
    }
    ((x0 + x1) / 2.0 - d.size as f32 / 2.0, (y0 + y1) / 2.0 - d.size as f32 / 2.0)
}

#[test]
fn square_and_quad_sit_where_a_cross_of_their_width_sits() {
    for style in [8, 9] {
        for (thickness, width) in [(1.0, 1.0), (1.5, 2.0), (2.0, 2.0), (2.5, 3.0), (3.0, 3.0)] {
            let c = Crosshair { style, thickness, length: 6, gap: 4, ..Crosshair::default() };
            let cross = Crosshair { style: 4, thickness: width, length: 6, gap: 4, ..Crosshair::default() };
            assert_eq!(frame_centre(&c), frame_centre(&cross), "style {style} thickness {thickness}");
        }
    }
}

#[test]
fn a_circle_has_an_outline_ring_under_it_as_wide_as_the_outline_asks() {
    for mode in [1, 2] {
        let c = Crosshair { style: 3, outline: mode, outline_width: 3, outline_color: [0, 0, 0], ..Crosshair::default() };
        let d = draw(&c, 1080);
        let rings: Vec<_> = d.pieces.iter().filter_map(|p| match p { Piece::Ring { width, rgba, .. } => Some((*width, *rgba)), _ => None }).collect();
        assert_eq!(rings.len(), 2);
        assert_eq!(rings[0].1[..3], [0, 0, 0]);
        assert_eq!(rings[0].0, rings[1].0 + 6.0);
    }
    let plain = draw(&Crosshair { style: 3, outline: 0, ..Crosshair::default() }, 1080);
    assert_eq!(plain.pieces.iter().filter(|p| matches!(p, Piece::Ring { .. })).count(), 1);
}
