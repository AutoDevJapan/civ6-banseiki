//! Pure, portable positive recognition of the *English initial* Civ6 loading screen.
//! This is visual evidence only, never permission to reset a GPU or infer a hung process.
//! Reference: owner diagnostic D:/CC/Tools/civ6-live/pr86-startup-28-20261003.png,
//! 1456x939 RGB; text bounds (600,470)..=(854,488). The tiny embedded thresholded
//! glyph mask comes directly from that capture, not OCR or rendered replacement text.
//! Other languages/fonts and ambiguous/occluded images deliberately return unknown.

use crate::{Error, Result};
use png::{BitDepth, ColorType, Decoder, Limits};
use serde_json::{json, Value};
use std::io::Cursor;

pub const MAX_PNG_BYTES: usize = 16 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 40 * 1024 * 1024;
const GRID_W: usize = 128;
const GRID_H: usize = 16;
const MIN_SCORE: f64 = 0.82;

/// Accepts only bounded, opaque, static 8-bit RGB/RGBA PNGs. Invalid or unsupported
/// PNGs are errors; valid images lacking positive evidence have status `unknown`.
/// Bounds are inclusive PNG pixel coordinates. No files, platform APIs or state changes.
pub fn classify_png(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_PNG_BYTES {
        return Err(Error::capture("startup PNG exceeds encoded byte limit"));
    }
    let mut decoder = Decoder::new_with_limits(
        Cursor::new(bytes),
        Limits {
            bytes: MAX_DECODED_BYTES,
        },
    );
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let header = decoder.read_header_info().map_err(png_error)?;
    let (w, h) = (header.width as usize, header.height as usize);
    if !(640..=4096).contains(&w) || !(400..=2560).contains(&h) {
        return Err(Error::capture(
            "startup PNG dimensions outside 640..4096 by 400..2560",
        ));
    }
    if header.bit_depth != BitDepth::Eight
        || !matches!(header.color_type, ColorType::Rgb | ColorType::Rgba)
    {
        return Err(Error::capture("startup PNG requires 8-bit RGB or RGBA"));
    }
    let mut reader = decoder.read_info().map_err(png_error)?;
    if reader.info().animation_control.is_some() || reader.info().trns.is_some() {
        return Err(Error::capture(
            "animated or color-key-transparent startup PNG is unsupported",
        ));
    }
    let size = reader
        .output_buffer_size()
        .filter(|n| *n <= MAX_DECODED_BYTES)
        .ok_or_else(|| Error::capture("startup PNG exceeds decoded byte limit"))?;
    let mut pixels = vec![0; size];
    let frame = reader.next_frame(&mut pixels).map_err(png_error)?;
    reader.finish().map_err(png_error)?;
    let channels = frame.color_type.samples();
    if channels == 4 && pixels.chunks_exact(4).any(|p| p[3] != 255) {
        return Ok(unknown("transparent_image", None, None));
    }

    // Ignore a conservative title/frame strip only; never crop to the central message.
    // This allows title bars at different DPI while testing almost the whole client body.
    let side = (w / 200).max(4);
    let top = (h / 12).max(64);
    let bottom = h - (h / 200).max(4);
    let gray = |x: usize, y: usize| pixels[(y * w + x) * channels];
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    let mut foreground = 0;
    for y in top..bottom {
        for x in side..w - side {
            let p = &pixels[(y * w + x) * channels..][..3];
            let hi = *p.iter().max().unwrap();
            let lo = *p.iter().min().unwrap();
            if hi > 12 && hi - lo > 6 {
                return Ok(unknown("non_black_or_colored_body", None, None));
            }
            if hi >= 32 {
                foreground += 1;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if foreground == 0 {
        return Ok(unknown("no_visible_text", None, None));
    }
    let bounds = [x0, y0, x1, y1];
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let aspect = bw as f64 / bh as f64;
    let cx = (x0 + x1) as f64 / 2.0 / w as f64;
    let cy = (y0 + y1) as f64 / 2.0 / h as f64;
    if !(12.3..=14.6).contains(&aspect)
        || !(12..=96).contains(&bh)
        || !(0.46..=0.54).contains(&cx)
        || !(0.46..=0.56).contains(&cy)
        || bw as f64 / w as f64 > 0.55
    {
        return Ok(unknown("text_geometry", Some(bounds), None));
    }
    for y in top..bottom {
        for x in side..w - side {
            if (x + 2 < x0 || x > x1 + 2 || y + 2 < y0 || y > y1 + 2) && gray(x, y) > 12 {
                return Ok(unknown("additional_visible_content", Some(bounds), None));
            }
        }
    }
    let actual = normalize(bw, bh, |x, y| gray(x0 + x, y0 + y) >= 32);
    let reference = normalize(bw, bh, |x, y| REFERENCE[y * 19 / bh][x * 255 / bw] == b'#');
    let sum: f64 = actual.iter().chain(&reference).sum();
    let overlap: f64 = actual.iter().zip(&reference).map(|(a, b)| a.min(*b)).sum();
    let score = if sum > 0.0 { 2.0 * overlap / sum } else { 0.0 };
    if score < MIN_SCORE {
        return Ok(unknown("glyph_mismatch", Some(bounds), Some(score)));
    }
    // A high whole-line score must not hide a missing/replaced letter or punctuation.
    // These nonempty horizontal spans are measured from the captured mask itself.
    for (left, right) in GLYPH_SPANS {
        let mut local_sum = 0.0;
        let mut local_overlap = 0.0;
        for gy in 0..GRID_H {
            for gx in 0..GRID_W {
                let rx = (gx * 2 + 1) * 255 / (GRID_W * 2);
                if (left..=right).contains(&rx) {
                    let i = gy * GRID_W + gx;
                    local_sum += actual[i] + reference[i];
                    local_overlap += actual[i].min(reference[i]);
                }
            }
        }
        if local_sum == 0.0 || 2.0 * local_overlap / local_sum < 0.70 {
            return Ok(unknown(
                "incomplete_glyph_pattern",
                Some(bounds),
                Some(score),
            ));
        }
    }
    Ok(
        json!({"status":"recognized", "pattern":"civ6_initial_loading_english",
        "bounds":bounds, "score":score, "width":w, "height":h}),
    )
}

fn png_error(e: png::DecodingError) -> Error {
    Error::capture(format!("invalid startup PNG: {e}"))
}
fn unknown(reason: &str, bounds: Option<[usize; 4]>, score: Option<f64>) -> Value {
    json!({"status":"unknown", "pattern":null, "reason":reason, "bounds":bounds, "score":score})
}

// Fixed-size area sampling keeps glyph evidence independent of window size and DPI.
fn normalize(width: usize, height: usize, ink: impl Fn(usize, usize) -> bool) -> Vec<f64> {
    let mut grid = vec![0.0; GRID_W * GRID_H];
    for gy in 0..GRID_H {
        for gx in 0..GRID_W {
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = ((gx * 8 + sx * 2 + 1) * width / (GRID_W * 8)).min(width - 1);
                    let y = ((gy * 8 + sy * 2 + 1) * height / (GRID_H * 8)).min(height - 1);
                    if ink(x, y) {
                        grid[gy * GRID_W + gx] += 1.0 / 16.0;
                    }
                }
            }
        }
    }
    grid
}

const GLYPH_SPANS: [(usize, usize); 18] = [
    (0, 13),
    (15, 27),
    (29, 42),
    (44, 56),
    (59, 64),
    (67, 80),
    (82, 94),
    (97, 99),
    (106, 119),
    (122, 131),
    (133, 166),
    (169, 179),
    (185, 223),
    (225, 230),
    (232, 243),
    (245, 246),
    (249, 250),
    (253, 254),
];

const REFERENCE: [&[u8]; 19] = [
    b"########...................................................................................................##########....................................................................#######.....##.....######.............................................",
    b"..####......................................................................................................####...####....................................................................####......##.......##...............................................",
    b"...###.......................................................................................................###....####...................................................................####.....####......##...............................................",
    b"...###.......................................................................................................###....####....................................................................###.....####.....##.........................##........##...........",
    b"...###............#######..........##.......##########.....######..####.....#####.....########...............###.....###..######.....##########.......##........######...##########.........###.....####.....##.........##.......######.############...........",
    b"...###...........###...###.........##.........###...####.....##.....####.....###....###....###...............###.....###....##.........##....##.......##.......###..##.....##....##.........####....#####....##.........##.........##...##...##...##...........",
    b"...###..........##......###.......####........##.....###.....##......###......#....###.......#...............###....####....##.........##.....#......####......##....##....##.....#..........###...##.###...##.........####........##...#....##....#...........",
    b"...###.........###.......###......####........##......###....##......####.....#...###........#...............###....###.....##.........##............####......###.........##................###...##.###...##.........####........##........##................",
    b"...###.........###.......###......#.###.......##......###....##......#.###....#...###........................###...###......##.........##...##.......#.###.....###.........##...##...........####..#...###..##.........#.###.......##........##................",
    b"...###.........##........###.....##.###.......##.......##....##......#..###...#...###........................########.......##.........##...##......##.###.....#####.......##...##............###.##...###..#.........##.###.......##........##................",
    b"...###.........##.........##.....#...##.......##.......##....##......#...###..#...##.........................###............##.........#######......#...##.......#####.....#######............###.##...######.........#...##.......##........##................",
    b"...###.........###........##....########......##......###....##......#...###..#...###.....#####..............###............##.........##...##.....########.......#####....##...##............#####.....#####........########......##........##................",
    b"...###.........###.......###....##...###......##......###....##......#....###.#...###.......##...............###............##.........##..........##...###.........###....##..................####.....#####........##...###......##........##................",
    b"...###......##.###.......###....#.....##......##......###....##......#.....####...###.......##...............###............##.........##......#...#.....##...##....###....##......#...........####.....####.........#.....##......##........##................",
    b"...###......##..###......##....##.....###.....##.....###.....##......#......###....###......##...###.........###............##.....#...##.....##..##.....###...#.....##....##.....##...........###.......###........##.....###.....##........##................",
    b"..#####...###....###...###.....##.....###.....###...###......##.....###......##.....###....###...###........####............###...##...###....##..##.....###...##...##.....###....##...........###.......###........##.....###.....##........###.....##..##..##",
    b"#############.....#######....#####...######.#########......######..#####.....##.......########....##......########........##########.##########.#####...######.######....##########.............##.......##.......#####...######.######....######....##..##..##",
    b"..................................................................................................##...........................................................................................................................................................",
    b".................................................................................................##............................................................................................................................................................",
];
