//! MEDIA-RENDERER: deterministic present-mode viewport overlays.
//!
//! This module is the honest low-resolution-ceiling fix: instead of
//! upscaling the 640x480 viewport, `render::render_present` renders a real
//! frame at any requested size and then composites three overlays into the
//! SAME raster:
//!
//! - a world grid on the sketch plane (Z=0), spacing chosen from the model
//!   extent with a deterministic 1/2/5 step so it is stable across frames,
//! - an XYZ axes triad drawn through the projected model origin (CAD
//!   convention: X red, Y green, Z blue), with a fixed on-screen arm length
//!   so it stays legible at any resolution,
//! - screen-space dimension/selection label lines (the editor already holds
//!   them: the driven dimensions resolved from the oep-sketch variable set
//!   plus the current selection id), rendered with a tiny dependency-free
//!   5x7 bitmap font on a dark chip for legibility.
//!
//! Everything is std-only and deterministic (no font, no external raster
//! dependency), which is what keeps a 1920x1080 frame byte-reproducible on
//! any machine and therefore commit-able as a regenerable demo artifact.
//!
//! The default (non-present) render path never touches this module, so the
//! 640x480 default output and every existing golden are unchanged.

use crate::render::RgbaImage;
use oep_geo::math::Vec3;

/// Width of one glyph cell in pixels.
pub const FONT_WIDTH: u32 = 5;
/// Height of one glyph cell in pixels.
pub const FONT_HEIGHT: u32 = 7;
/// Vertical gap between label lines in pixels.
pub const LINE_GAP: u32 = 2;
/// Horizontal padding of the label chip in pixels.
pub const CHIP_PAD_X: u32 = 6;
/// Vertical padding of the label chip in pixels.
pub const CHIP_PAD_Y: u32 = 4;

/// One glyph: 7 rows of 5 bits (bit 4 = leftmost column).
type Glyph = [u8; 7];

/// The 5x7 bitmap font, authored in plain text: each glyph is its key on its
/// own line followed by exactly 7 lines of 5 characters (`#` = on, `.` =
/// off). Parsed once into [`glyph()`]. Labels are rendered in upper case
/// (dimension names from the variable set are uppercased before raster), so
/// the table covers A-Z, the digits, and the small punctuation set used by
/// measured values and selection ids. Any character with no glyph renders as
/// a space.
const FONT_5X7: &str = "\
A
.###.
#...#
#...#
#####
#...#
#...#
#...#
B
####.
#...#
#...#
####.
#...#
#...#
####.
C
.###.
#...#
#....
#....
#....
#...#
.###.
D
####.
#...#
#...#
#...#
#...#
#...#
####.
E
#####
#....
#....
####.
#....
#....
#####
F
#####
#....
#....
####.
#....
#....
#....
G
.###.
#...#
#....
#..##
#...#
#...#
.###.
H
#...#
#...#
#...#
#####
#...#
#...#
#...#
I
.###.
..#..
..#..
..#..
..#..
..#..
.###.
J
...##
....#
....#
....#
....#
#...#
.###.
K
#...#
#..#.
#.#..
##...
#.#..
#..#.
#...#
L
#....
#....
#....
#....
#....
#....
#####
M
#...#
##.##
#.#.#
#.#.#
#...#
#...#
#...#
N
#...#
##..#
#.#.#
#..##
#...#
#...#
#...#
O
.###.
#...#
#...#
#...#
#...#
#...#
.###.
P
####.
#...#
#...#
####.
#....
#....
#....
Q
.###.
#...#
#...#
#...#
#...#
#..##
.####.
R
####.
#...#
#...#
####.
##...
#.#..
#..#.
S
.###.
#...#
#....
.##..
....#
#...#
.###.
T
#####
..#..
..#..
..#..
..#..
..#..
..#..
U
#...#
#...#
#...#
#...#
#...#
#...#
.###.
V
#...#
#...#
#...#
#...#
.#.#.
..#..
..#..
W
#...#
#...#
#...#
#.#.#
#.#.#
##.##
#...#
X
#...#
#...#
.#.#.
..#..
.#.#.
#...#
#...#
Y
#...#
#...#
.#.#.
..#..
..#..
..#..
..#..
Z
#####
....#
...#.
..#..
.#...
#....
#####
0
.###.
#...#
#..##
#.#.#
##..#
#...#
.###.
1
..#..
.##..
..#..
..#..
..#..
..#..
.###.
2
.###.
#...#
....#
...#.
..#..
.#...
#####
3
####.
....#
...#.
..##.
....#
#...#
.###.
4
...##
..#.#
.#..#
.####
....#
....#
....#
5
#####
#....
#....
####.
....#
#...#
.###.
6
..##.
.#...
#....
###..
#...#
#...#
.###.
7
#####
....#
...##
..#..
..#..
.#...
.#...
8
.###.
#...#
#...#
.###.
#...#
#...#
.###.
9
.###.
#...#
#...#
.###.
....#
...#.
.##..
 
.....
.....
.....
.....
.....
.....
.....
.
.....
.....
.....
.....
.....
.....
..##.
=
.....
.....
#####
.....
#####
.....
.....
-
.....
.....
.....
#####
.....
.....
.....
_
.....
.....
.....
.....
.....
.....
#####
:
.....
..##.
..##.
.....
..##.
..##.
.....
";

/// Look up the 5x7 glyph for one character (fallback: space). The char is
/// uppercased first so labels render uniformly.
fn glyph(ch: char) -> Glyph {
    let ch = ch.to_ascii_uppercase();
    let mut current: Option<char> = None;
    let mut rows: Vec<u8> = Vec::with_capacity(7);
    // Build a glyph from the collected rows of `cur` when it is the target.
    let finish = |cur: char, rows: &[u8]| -> Option<Glyph> {
        if cur == ch && rows.len() >= 7 {
            let mut g = [0u8; 7];
            for (i, r) in rows.iter().enumerate().take(7) {
                g[i] = *r;
            }
            Some(g)
        } else {
            None
        }
    };
    for raw in FONT_5X7.lines() {
        if raw.len() == 1 {
            if let Some(c) = current {
                if let Some(g) = finish(c, &rows) {
                    return g;
                }
            }
            current = Some(raw.chars().next().unwrap_or(' '));
            rows.clear();
        } else if raw.len() >= 5 {
            let mut b = 0u8;
            for (i, c) in raw.chars().enumerate().take(5) {
                if c == '#' {
                    b |= 1 << (4 - i);
                }
            }
            rows.push(b);
        }
    }
    // The final glyph has no following key to trigger the check above.
    if let Some(c) = current {
        if let Some(g) = finish(c, &rows) {
            return g;
        }
    }
    [0u8; 7]
}

/// Width in pixels of `text` at 1x scale (spaces included).
pub fn text_width(text: &str) -> u32 {
    text.chars().count() as u32 * FONT_WIDTH
}

/// Draw `text` with its top-left at pixel `(x, y)` using `color` (opaque).
/// Returns `(width, height)` in pixels that were touched.
pub fn draw_text(img: &mut RgbaImage, x: i64, y: i64, text: &str, color: [u8; 4]) -> (u32, u32) {
    let mut cx = x;
    let h = FONT_HEIGHT as i64;
    for ch in text.chars() {
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    set_safe(img, cx + col as i64, y + row as i64, color);
                }
            }
        }
        cx += FONT_WIDTH as i64;
    }
    (text_width(text), h as u32)
}

/// Rasterize the label lines inside an opaque dark chip so the text reads
/// over any model content. Returns the chip `(width, height)`.
pub fn draw_label_chip(
    img: &mut RgbaImage,
    x: i64,
    y: i64,
    labels: &[String],
    fg: [u8; 4],
    bg: [u8; 4],
) -> (u32, u32) {
    let mut w = 0u32;
    for l in labels {
        w = w.max(text_width(l));
    }
    let line_h = FONT_HEIGHT + LINE_GAP;
    let chip_w = w + CHIP_PAD_X * 2;
    let chip_h = if labels.is_empty() {
        0
    } else {
        (labels.len() as u32) * line_h + CHIP_PAD_Y * 2 - LINE_GAP
    };
    fill_rect(img, x, y, chip_w, chip_h, bg);
    let mut ty = y + CHIP_PAD_Y as i64;
    for l in labels {
        draw_text(img, x + CHIP_PAD_X as i64, ty, l, fg);
        ty += line_h as i64;
    }
    (chip_w, chip_h)
}

/// Fill a solid rectangle (clipped to the image).
pub fn fill_rect(img: &mut RgbaImage, x: i64, y: i64, w: u32, h: u32, color: [u8; 4]) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w as i64).min(img.width as i64);
    let y1 = (y + h as i64).min(img.height as i64);
    for py in y0..y1 {
        for px in x0..x1 {
            set_safe(img, px, py, color);
        }
    }
}

fn set_safe(img: &mut RgbaImage, x: i64, y: i64, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= img.width as i64 || y >= img.height as i64 {
        return;
    }
    img.set_pixel(x as u32, y as u32, color);
}

/// A deterministic grid spacing near `target` using the 1/2/5*10^k ladder, so
/// the same model frames at the same spacing at any resolution.
pub fn nice_spacing(target: f64) -> f64 {
    if !target.is_finite() || target <= 0.0 {
        return 10.0;
    }
    let exp = target.log10().floor();
    let base = 10f64.powf(exp);
    let unit = target / base;
    let step = if unit < 1.5 {
        1.0
    } else if unit < 3.5 {
        2.0
    } else if unit < 7.5 {
        5.0
    } else {
        10.0
    };
    step * base
}

/// World-space grid segments in the sketch plane (Z=0), covering
/// `[-extent, extent]` on both X and Y at `spacing`. Deterministic: same
/// inputs, same segments.
pub fn grid_segments(extent: f64, spacing: f64) -> Vec<[Vec3; 2]> {
    let mut segs = Vec::new();
    if spacing <= 0.0 || extent <= 0.0 {
        return segs;
    }
    let lo = -(extent / spacing).floor() * spacing;
    let hi = (extent / spacing).floor() * spacing;
    let mut v = lo;
    while v <= hi + spacing * 0.5 {
        segs.push([Vec3::new(v, -hi, 0.0), Vec3::new(v, hi, 0.0)]);
        segs.push([Vec3::new(-hi, v, 0.0), Vec3::new(hi, v, 0.0)]);
        v += spacing;
    }
    segs
}

/// The three world-axis arms of the triad: [X, Y, Z], each one segment from
/// `origin` to `origin + axis * arm`.
pub fn axis_arms(origin: Vec3, arm: f64) -> [[Vec3; 2]; 3] {
    [
        [origin, origin.add(&Vec3::new(arm, 0.0, 0.0))],
        [origin, origin.add(&Vec3::new(0.0, arm, 0.0))],
        [origin, origin.add(&Vec3::new(0.0, 0.0, arm))],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: u32, h: u32) -> RgbaImage {
        RgbaImage {
            width: w,
            height: h,
            data: vec![0; (w * h * 4) as usize],
        }
    }

    #[test]
    fn glyph_table_has_all_label_chars() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .=-_:".chars() {
            let g = glyph(ch);
            if ch == ' ' {
                assert!(g.iter().all(|r| *r == 0), "space glyph is blank");
            } else {
                let lit = g.iter().any(|r| *r != 0);
                assert!(lit, "glyph for {ch:?} must be drawn");
            }
        }
    }

    #[test]
    fn glyph_table_has_no_blank_leading_or_trailing() {
        // Every A-Z glyph uses the full 7 rows (7 rows, not 0), as authored.
        for ch in 'A'..='Z' {
            let g = glyph(ch);
            assert!(
                g[0] != 0 && g[6] != 0,
                "glyph for {ch} should have lit top/bottom"
            );
        }
    }

    #[test]
    fn draw_text_paints_expected_pixels() {
        let mut img = canvas(64, 64);
        let (w, h) = draw_text(&mut img, 0, 0, "0", [255, 255, 255, 255]);
        assert_eq!(w, 5);
        assert_eq!(h, 7);
        let mut lit = 0;
        for y in 0..8 {
            for x in 0..6 {
                if img.pixel(x, y)[0] == 255 {
                    lit += 1;
                }
            }
        }
        assert!(
            lit >= 10,
            "glyph '0' should light several pixels, got {lit}"
        );
    }

    #[test]
    fn draw_text_uppercases() {
        let mut a = canvas(64, 16);
        let mut b = canvas(64, 16);
        draw_text(&mut a, 0, 0, "width", [255, 255, 255, 255]);
        draw_text(&mut b, 0, 0, "WIDTH", [255, 255, 255, 255]);
        assert_eq!(a.data, b.data, "lowercase renders as uppercase");
    }

    #[test]
    fn draw_text_clips_out_of_bounds() {
        let mut img = canvas(8, 8);
        draw_text(&mut img, 6, 6, "abc", [255, 255, 255, 255]);
        // No panic; the image is untouched at the origin.
        assert_eq!(img.pixel(0, 0)[0], 0);
    }

    #[test]
    fn grid_segments_cover_and_are_deterministic() {
        let a = grid_segments(100.0, 10.0);
        let b = grid_segments(100.0, 10.0);
        assert_eq!(a.len(), b.len());
        assert!(
            a.len() >= 42,
            "grid over +-100 at 10 should have >=42 segments"
        );
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x, y);
        }
    }

    #[test]
    fn nice_spacing_follows_125_ladder() {
        assert_eq!(nice_spacing(1.0), 1.0);
        assert_eq!(nice_spacing(2.0), 2.0);
        assert_eq!(nice_spacing(5.0), 5.0);
        assert_eq!(nice_spacing(100.0), 100.0);
        assert_eq!(nice_spacing(55.0), 50.0);
        assert_eq!(nice_spacing(0.0), 10.0);
    }

    #[test]
    fn axis_arms_are_unit_aligned() {
        let arms = axis_arms(Vec3::zero(), 10.0);
        assert_eq!(arms[0][1], Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(arms[1][1], Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(arms[2][1], Vec3::new(0.0, 0.0, 10.0));
    }
}
