//! Page position labels without a PDF font resource (design §2.6, decision D7).
//!
//! Each glyph is a list of polylines on a unit grid (`x` and `y` in `[0, 1]`,
//! baseline at `y = 0`). Rendering emits `m` / `l` / `S` only: no `/Font`
//! dictionary, no `BT`/`ET`, no embedded font file, no encoding table.
//!
//! Why not `/BaseFont /Helvetica`: non-embedded base-14 fonts are substituted
//! by the reader, and the substitution changes metrics and weight, which breaks
//! the `label_offset_mm >= 8 mm` guarantee and makes output viewer-dependent.
//! Position labels are two to four glyphs (`B3`, `AA`) where kerning and
//! hinting are irrelevant at 9 pt, and the same table drives the GUI preview.
//!
//! The tracking constant is fixed by the two documented widths:
//! `text_width("B3", 9.0) == 27.0` and `text_width("-", 9.0) == 4.5`, i.e. two
//! glyph advances plus one inter-glyph gap, and a single half-width glyph with
//! no gap.

use crate::modern::color::Rgb;
use crate::modern::session::PageTarget;

/// Inter-glyph spacing, in em.
pub const LETTER_TRACKING_EM: f64 = 1.0;

/// Minimum stroke weight in points.
pub const MIN_STROKE_PT: f64 = 0.35;

/// Advance-to-size ratio that produces a usable stroke weight.
pub const STROKE_PER_PT: f64 = 0.09;

/// Every glyph: name, polylines on the unit grid, advance in em.
#[rustfmt::skip]
pub const GLYPHS: &[(&str, &[&[[f32; 2]]], f32)] = &[
    ("A", &[&[[0.0,0.0],[0.5,1.0],[1.0,0.0]], &[[0.18,0.37],[0.82,0.37]]], 1.0),
    ("B", &[&[[0.0,0.0],[0.0,1.0],[0.65,1.0],[0.85,0.82],[0.85,0.68],[0.65,0.5],[0.0,0.5]],
            &[[0.65,0.5],[0.9,0.32],[0.9,0.16],[0.68,0.0],[0.0,0.0]]], 1.0),
    ("C", &[&[[1.0,0.85],[0.8,1.0],[0.2,1.0],[0.0,0.8],[0.0,0.2],[0.2,0.0],[0.8,0.0],[1.0,0.15]]], 1.0),
    ("D", &[&[[0.0,0.0],[0.0,1.0],[0.6,1.0],[1.0,0.7],[1.0,0.3],[0.6,0.0],[0.0,0.0]]], 1.0),
    ("E", &[&[[1.0,1.0],[0.0,1.0],[0.0,0.0],[1.0,0.0]], &[[0.0,0.5],[0.75,0.5]]], 1.0),
    ("F", &[&[[1.0,1.0],[0.0,1.0],[0.0,0.0]], &[[0.0,0.5],[0.75,0.5]]], 1.0),
    ("G", &[&[[1.0,0.85],[0.8,1.0],[0.2,1.0],[0.0,0.8],[0.0,0.2],[0.2,0.0],[0.8,0.0],[1.0,0.18],[1.0,0.45],[0.55,0.45]]], 1.0),
    ("H", &[&[[0.0,0.0],[0.0,1.0]], &[[1.0,0.0],[1.0,1.0]], &[[0.0,0.5],[1.0,0.5]]], 1.0),
    ("I", &[&[[0.5,0.0],[0.5,1.0]], &[[0.2,1.0],[0.8,1.0]], &[[0.2,0.0],[0.8,0.0]]], 1.0),
    ("J", &[&[[0.8,1.0],[0.8,0.2],[0.6,0.0],[0.25,0.0],[0.05,0.2]]], 1.0),
    ("K", &[&[[0.0,0.0],[0.0,1.0]], &[[1.0,1.0],[0.0,0.42]], &[[0.35,0.6],[1.0,0.0]]], 1.0),
    ("L", &[&[[0.0,1.0],[0.0,0.0],[1.0,0.0]]], 1.0),
    ("M", &[&[[0.0,0.0],[0.0,1.0],[0.5,0.45],[1.0,1.0],[1.0,0.0]]], 1.0),
    ("N", &[&[[0.0,0.0],[0.0,1.0],[1.0,0.0],[1.0,1.0]]], 1.0),
    ("O", &[&[[0.2,0.0],[0.8,0.0],[1.0,0.2],[1.0,0.8],[0.8,1.0],[0.2,1.0],[0.0,0.8],[0.0,0.2],[0.2,0.0]]], 1.0),
    ("P", &[&[[0.0,0.0],[0.0,1.0],[0.7,1.0],[1.0,0.78],[1.0,0.6],[0.7,0.45],[0.0,0.45]]], 1.0),
    ("Q", &[&[[0.2,0.0],[0.8,0.0],[1.0,0.2],[1.0,0.8],[0.8,1.0],[0.2,1.0],[0.0,0.8],[0.0,0.2],[0.2,0.0]],
            &[[0.6,0.25],[0.95,0.05]]], 1.0),
    ("R", &[&[[0.0,0.0],[0.0,1.0],[0.7,1.0],[1.0,0.78],[1.0,0.6],[0.7,0.45],[0.0,0.45]], &[[0.45,0.45],[1.0,0.0]]], 1.0),
    ("S", &[&[[1.0,0.85],[0.75,1.0],[0.25,1.0],[0.0,0.82],[0.0,0.64],[0.2,0.52],[0.8,0.48],[1.0,0.36],[1.0,0.16],[0.75,0.0],[0.25,0.0],[0.0,0.15]]], 1.0),
    ("T", &[&[[0.0,1.0],[1.0,1.0]], &[[0.5,1.0],[0.5,0.0]]], 1.0),
    ("U", &[&[[0.0,1.0],[0.0,0.2],[0.2,0.0],[0.8,0.0],[1.0,0.2],[1.0,1.0]]], 1.0),
    ("V", &[&[[0.0,1.0],[0.5,0.0],[1.0,1.0]]], 1.0),
    ("W", &[&[[0.0,1.0],[0.2,0.0],[0.5,0.6],[0.8,0.0],[1.0,1.0]]], 1.0),
    ("X", &[&[[0.0,0.0],[1.0,1.0]], &[[0.0,1.0],[1.0,0.0]]], 1.0),
    ("Y", &[&[[0.0,1.0],[0.5,0.5],[1.0,1.0]], &[[0.5,0.5],[0.5,0.0]]], 1.0),
    ("Z", &[&[[0.0,1.0],[1.0,1.0],[0.0,0.0],[1.0,0.0]]], 1.0),
    ("0", &[&[[0.2,0.0],[0.8,0.0],[1.0,0.2],[1.0,0.8],[0.8,1.0],[0.2,1.0],[0.0,0.8],[0.0,0.2],[0.2,0.0]],
            &[[0.15,0.15],[0.85,0.85]]], 1.0),
    ("1", &[&[[0.15,0.8],[0.5,1.0],[0.5,0.0]], &[[0.2,0.0],[0.8,0.0]]], 1.0),
    ("2", &[&[[0.0,0.85],[0.25,1.0],[0.75,1.0],[1.0,0.8],[1.0,0.6],[0.0,0.0],[1.0,0.0]]], 1.0),
    ("3", &[&[[0.0,1.0],[1.0,1.0],[0.45,0.58]], &[[0.45,0.58],[0.8,0.5],[1.0,0.32],[1.0,0.15],[0.75,0.0],[0.2,0.0],[0.0,0.15]]], 1.0),
    ("4", &[&[[0.75,0.0],[0.75,1.0],[0.0,0.3],[1.0,0.3]]], 1.0),
    ("5", &[&[[1.0,1.0],[0.1,1.0],[0.05,0.55]], &[[0.05,0.55],[0.65,0.55],[1.0,0.38],[1.0,0.15],[0.75,0.0],[0.15,0.0],[0.0,0.15]]], 1.0),
    ("6", &[&[[0.9,0.9],[0.7,1.0],[0.3,1.0],[0.05,0.75],[0.0,0.35],[0.05,0.12],[0.3,0.0],[0.7,0.0],[1.0,0.2],[1.0,0.38],[0.75,0.55],[0.3,0.55],[0.05,0.4]]], 1.0),
    ("7", &[&[[0.0,1.0],[1.0,1.0],[0.35,0.0]]], 1.0),
    ("8", &[&[[0.25,0.55],[0.05,0.7],[0.05,0.88],[0.28,1.0],[0.72,1.0],[0.95,0.88],[0.95,0.7],[0.75,0.55],[0.3,0.55],
            [0.0,0.38],[0.0,0.15],[0.25,0.0],[0.75,0.0],[1.0,0.15],[1.0,0.38],[0.7,0.55]]], 1.0),
    ("9", &[&[[0.1,0.1],[0.3,0.0],[0.7,0.0],[0.95,0.25],[1.0,0.65],[0.95,0.88],[0.7,1.0],[0.3,1.0],[0.0,0.8],[0.0,0.62],[0.25,0.45],[0.7,0.45],[0.95,0.6]]], 1.0),
    ("-", &[&[[0.1,0.5],[0.9,0.5]]], 0.5),
    (" ", &[], 0.5),
];

/// Look up a glyph by character; unsupported characters are skipped.
pub fn glyph(c: char) -> Option<(&'static str, &'static [&'static [[f32; 2]]], f32)> {
    let up = c.to_ascii_uppercase().to_string();
    let want: &str = if up.len() == 1 && c.is_ascii() { &up } else { return None };
    GLYPHS.iter().find(|g| g.0 == want).copied()
}

/// The stroke weight for a label size: `max(0.35, size_pt * 0.09)`.
pub fn stroke_weight(size_pt: f64) -> f64 {
    (size_pt * STROKE_PER_PT).max(MIN_STROKE_PT)
}

/// Width of a string in points.
pub fn text_width(s: &str, size_pt: f64) -> f64 {
    let n = s.chars().filter(|c| glyph(*c).is_some()).count();
    if n == 0 {
        return 0.0;
    }
    let adv: f32 = s.chars().filter_map(glyph).map(|g| g.2).sum();
    let gaps = (n - 1) as f32 * LETTER_TRACKING_EM as f32;
    f64::from(adv + gaps) * size_pt
}

/// Emit `s` with its baseline at `(x_pt, y_pt)` and its box rising `size_pt`.
///
/// Only `m`, `l` and `S` operators are used, so no font resource is needed.
pub fn emit_text(
    t: &mut dyn PageTarget,
    s: &str,
    x_pt: f64,
    y_pt: f64,
    size_pt: f64,
    weight_pt: f64,
    color: Rgb,
) {
    t.stroke_rgb(color);
    t.line_width_pt(weight_pt);
    let mut pen = x_pt;
    for c in s.chars() {
        let Some((_, strokes, adv)) = glyph(c) else { continue };
        for poly in strokes {
            if poly.is_empty() {
                continue;
            }
            let first = poly[0];
            t.move_to(pen + f64::from(first[0]) * size_pt, y_pt + f64::from(first[1]) * size_pt);
            for p in &poly[1..] {
                t.line_to(pen + f64::from(p[0]) * size_pt, y_pt + f64::from(p[1]) * size_pt);
            }
            t.close_stroke();
        }
        pen += (f64::from(adv) + LETTER_TRACKING_EM) * size_pt;
    }
}

/// Bijective base-26 column label: `1 -> A`, `26 -> Z`, `27 -> AA`.
pub fn col_label(col: u32) -> String {
    let mut n = col;
    let mut out = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push((b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    if out.is_empty() {
        return "A".to_string();
    }
    out.iter().rev().collect()
}

/// The printed label of one page: column letter followed by the 1-based row.
pub fn page_label(col: u32, row: u32) -> String {
    format!("{}{}", col_label(col + 1), row + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_the_documented_shape() {
        assert_eq!(GLYPHS.len(), 38, "A-Z, 0-9, '-' and space");
        let names: Vec<&str> = GLYPHS.iter().map(|g| g.0).collect();
        assert_eq!(names.len(), 38);
        for (name, strokes, _) in GLYPHS {
            assert!(name.chars().count() <= 1, "{name}");
            if *name == " " {
                assert!(strokes.is_empty());
                continue;
            }
            assert!(!strokes.is_empty(), "{name} has no strokes");
            for poly in *strokes {
                assert!(!poly.is_empty(), "{name} has an empty polyline");
                for p in poly.iter() {
                    for v in p {
                        assert!((0.0..=1.0).contains(v), "{name} point {v:?} is out of the unit box");
                    }
                }
            }
        }
    }

    #[test]
    fn advances_match_the_documented_table() {
        for (name, _, adv) in GLYPHS {
            let want = if *name == "-" || *name == " " { 0.5 } else { 1.0 };
            assert_eq!(*adv, want, "{name}");
        }
    }

    #[test]
    fn documented_text_widths() {
        assert!((text_width("B3", 9.0) - 27.0).abs() < 1e-12);
        assert!((text_width("-", 9.0) - 4.5).abs() < 1e-12);
        assert_eq!(text_width("", 9.0), 0.0);
    }

    #[test]
    fn bijective_base_26_columns() {
        // The column label is the bijective base-26 form of the 1-based page
        // number: 1 -> A, 26 -> Z, 27 -> AA.
        assert_eq!(col_label(1), "A");
        assert_eq!(col_label(25), "Y");
        assert_eq!(col_label(26), "Z");
        assert_eq!(col_label(27), "AA");
        assert_eq!(col_label(29), "AC");
        assert_eq!(col_label(30), "AD");
        let seq: Vec<String> = (1..=30).map(col_label).collect();
        assert_eq!(seq[0], "A");
        assert_eq!(seq[24], "Y");
        assert_eq!(seq[25], "Z");
        assert_eq!(seq[26], "AA");
        assert_eq!(seq[27], "AB");
        assert_eq!(seq[28], "AC");
        assert_eq!(seq[29], "AD");
        // A zero column has no representation and falls back to the first.
        assert_eq!(col_label(0), "A");
    }

    #[test]
    fn page_label_combines_column_and_row() {
        assert_eq!(page_label(1, 2), "B3");
        assert_eq!(page_label(26, 0), "AA1");
    }

    #[test]
    fn stroke_weight_floor() {
        assert!((stroke_weight(9.0) - 0.81).abs() < 1e-12);
        assert_eq!(stroke_weight(1.0), MIN_STROKE_PT);
    }

    #[test]
    fn unsupported_characters_are_skipped() {
        assert!(glyph('#').is_none());
        assert_eq!(glyph('a').map(|g| g.0), Some("A"));
        assert_eq!(glyph(' ').map(|g| g.0), Some(" "));
    }

    #[test]
    fn emit_only_uses_stroke_operators() {
        use crate::modern::geometry::{CellWindow, PageBoxes};
        use crate::modern::probe::CountingTarget;
        let mut t = CountingTarget::default();
        let boxes = PageBoxes { media: [0.0, 0.0, 10.0, 10.0], trim: [0.0, 0.0, 10.0, 10.0], bleed: [0.0, 0.0, 10.0, 10.0] };
        let w = CellWindow { col: 0, row: 0, i0: 0, i1: 0, j0: 0, j1: 0 };
        t.begin_page(0, &boxes, &w);
        emit_text(&mut t, "AA12", 1.0, 2.0, 9.0, 0.81, Rgb::BLACK);
        t.end_page();
        assert!(t.strokes >= 7, "four glyphs of one to two strokes each");
        assert_eq!(t.fills, 0, "labels never fill");
    }
}
