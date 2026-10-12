//! Shape geometry per pattern (design §3.8).
//!
//! Every pattern is a pure function of `(centre, extent e, tone t, i, j)`
//! producing a closed path in the pattern's own poster-relative millimetre
//! frame, always centred on the cell centre.
//!
//! Patterns emit into a [`PathSink`] in poster coordinates (`y` down). The
//! caller adapts that to the device frame, which is what lets the PDF target,
//! the preview bitmap and the test recorder share exactly the same geometry.

use crate::modern::options::Pattern;
use crate::CIRCLE_K;

/// Receiver of a closed filled path in poster millimetres.
pub trait PathSink {
    /// Start a subpath.
    fn move_to(&mut self, x_mm: f64, y_mm: f64);
    /// Extend the current subpath.
    fn line_to(&mut self, x_mm: f64, y_mm: f64);
    /// Extend the current subpath with a cubic curve.
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64);
    /// Close the current subpath and fill it.
    fn close_fill(&mut self);
}

/// A [`PathSink`] that records everything, for tests and for the counting
/// target.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordedPath {
    /// Every emitted drawing command in order.
    pub ops: Vec<Op>,
}

/// One recorded command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// `m x y`
    Move(f64, f64),
    /// `l x y`
    Line(f64, f64),
    /// `c x1 y1 x2 y2 x3 y3`
    Curve([f64; 6]),
    /// `h f`
    CloseFill,
}

impl PathSink for RecordedPath {
    fn move_to(&mut self, x: f64, y: f64) {
        self.ops.push(Op::Move(x, y));
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.ops.push(Op::Line(x, y));
    }
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        self.ops.push(Op::Curve([x1, y1, x2, y2, x3, y3]));
    }
    fn close_fill(&mut self) {
        self.ops.push(Op::CloseFill);
    }
}

impl RecordedPath {
    /// Number of emitted operations.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// True when nothing was emitted.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// Half-axes of the morphing bar at tone `t`.
///
/// `theta = (pi/2) * t`: bright (`t -> 0`) horizontal, dark (`t -> 1`) vertical,
/// continuous in between.
pub fn line_morph_axes(e: f64, t: f64, line_aspect: f64) -> ((f64, f64), (f64, f64)) {
    let theta = std::f64::consts::FRAC_PI_2 * t;
    let (s, c) = theta.sin_cos();
    let along = (c * e, s * e);
    let across = (-s * e * line_aspect, c * e * line_aspect);
    (along, across)
}

/// Area of one shape in square millimetres, used for the ink-density estimate.
///
/// Circles `pi e^2`, squares `(2e)^2`, triangles `sqrt(3)/2 e^2`, bars
/// `4 e^2 * line_aspect`.
pub fn shape_area_mm2(pattern: Pattern, e: f64, line_aspect: f64) -> f64 {
    match pattern {
        Pattern::Dot => std::f64::consts::PI * e * e,
        Pattern::Square | Pattern::Mosaic => 4.0 * e * e,
        Pattern::Triangle | Pattern::TriangleAlt => (3.0f64).sqrt() / 2.0 * e * e,
        Pattern::LineH | Pattern::LineV | Pattern::LineMorph => 4.0 * e * e * line_aspect,
    }
}

fn emit_circle(sink: &mut dyn PathSink, cx: f64, cy: f64, r: f64) {
    let k = f64::from(CIRCLE_K);
    sink.move_to(cx + r, cy);
    sink.curve_to(cx + r, cy + r * k, cx + r * k, cy + r, cx, cy + r);
    sink.curve_to(cx - r * k, cy + r, cx - r, cy + r * k, cx - r, cy);
    sink.curve_to(cx - r, cy - r * k, cx - r * k, cy - r, cx, cy - r);
    sink.curve_to(cx + r * k, cy - r, cx + r, cy - r * k, cx + r, cy);
    sink.close_fill();
}

fn emit_square(sink: &mut dyn PathSink, cx: f64, cy: f64, e: f64) {
    sink.move_to(cx - e, cy - e);
    sink.line_to(cx + e, cy - e);
    sink.line_to(cx + e, cy + e);
    sink.line_to(cx - e, cy + e);
    sink.close_fill();
}

fn emit_triangle(sink: &mut dyn PathSink, cx: f64, cy: f64, e: f64, flip: bool) {
    // Vertices at 90 / 210 / 330 degrees with circumradius `e`; the first
    // vertex is at (0, +e) in the poster frame. `flip` rotates the triangle by
    // 180 degrees, which is exactly negating the offsets.
    let s3 = 3.0f64.sqrt() / 2.0;
    let verts = [(0.0, 1.0), (-s3, -0.5), (s3, -0.5)];
    for (k, (a, b)) in verts.iter().enumerate() {
        let (a, b) = if flip { (-a, -b) } else { (*a, *b) };
        let (x, y) = (cx + a * e, cy + b * e);
        if k == 0 {
            sink.move_to(x, y);
        } else {
            sink.line_to(x, y);
        }
    }
    sink.close_fill();
}

fn emit_bar(
    sink: &mut dyn PathSink,
    cx: f64,
    cy: f64,
    along: (f64, f64),
    across: (f64, f64),
) {
    let corner = |a: f64, b: f64| (cx + a * along.0 + b * across.0, cy + a * along.1 + b * across.1);
    let p0 = corner(-1.0, -1.0);
    let p1 = corner(1.0, -1.0);
    let p2 = corner(1.0, 1.0);
    let p3 = corner(-1.0, 1.0);
    sink.move_to(p0.0, p0.1);
    sink.line_to(p1.0, p1.1);
    sink.line_to(p2.0, p2.1);
    sink.line_to(p3.0, p3.1);
    sink.close_fill();
}

/// Emit one shape.
///
/// Nothing is emitted when `e <= 0`; the caller counts that as a skipped cell.
#[allow(clippy::too_many_arguments)]
pub fn emit_path(
    sink: &mut dyn PathSink,
    pattern: Pattern,
    centre: (f64, f64),
    e: f64,
    t: f64,
    i: i64,
    j: i64,
    line_aspect: f64,
) {
    if !(e > 0.0) {
        return;
    }
    let (cx, cy) = centre;
    match pattern {
        Pattern::Dot => emit_circle(sink, cx, cy, e),
        Pattern::Square | Pattern::Mosaic => emit_square(sink, cx, cy, e),
        Pattern::Triangle => emit_triangle(sink, cx, cy, e, false),
        Pattern::TriangleAlt => emit_triangle(sink, cx, cy, e, (i + j) & 1 == 1),
        Pattern::LineH => emit_bar(sink, cx, cy, (e, 0.0), (0.0, e * line_aspect)),
        Pattern::LineV => emit_bar(sink, cx, cy, (0.0, e), (e * line_aspect, 0.0)),
        Pattern::LineMorph => {
            let (along, across) = line_morph_axes(e, t, line_aspect);
            emit_bar(sink, cx, cy, along, across);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::options::Pattern;

    fn rec(p: Pattern, e: f64, t: f64, i: i64, j: i64) -> RecordedPath {
        let mut r = RecordedPath::default();
        emit_path(&mut r, p, (0.0, 0.0), e, t, i, j, 0.5);
        r
    }

    #[test]
    fn zero_extent_emits_nothing() {
        for p in Pattern::ALL {
            for e in [0.0, -1.0, f64::NAN] {
                assert!(rec(*p, e, 0.5, 0, 0).is_empty(), "{p:?} with e={e}");
            }
        }
    }

    #[test]
    fn line_morph_half_axis_rotates() {
        let e = 2.0;
        let (a0, _) = line_morph_axes(e, 0.0, 0.5);
        assert!((a0.0 - e).abs() < 1e-12 && a0.1.abs() < 1e-12);
        let (a1, _) = line_morph_axes(e, 1.0, 0.5);
        assert!(a1.0.abs() < 1e-12 && (a1.1 - e).abs() < 1e-12);
        let (ah, _) = line_morph_axes(e, 0.5, 0.5);
        let q = e * 0.7071067812;
        assert!((ah.0 - q).abs() < 1e-9 && (ah.1 - q).abs() < 1e-9);
    }

    #[test]
    fn line_morph_is_a_bar_rotated_about_the_centre() {
        let e = 2.0;
        let p = rec(Pattern::LineMorph, e, 0.5, 0, 0);
        // The half-length axis sits at 45 degrees and is `e` long; the
        // half-thickness axis is perpendicular and `e * line_aspect` long.
        let pts: Vec<(f64, f64)> = p
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Move(x, y) | Op::Line(x, y) => Some((*x, *y)),
                _ => None,
            })
            .collect();
        assert_eq!(pts.len(), 4);
        let (along, across) = line_morph_axes(e, 0.5, 0.5);
        for a in [-1.0f64, 1.0] {
            for b in [-1.0f64, 1.0] {
                let want = (
                    a * along.0 + b * across.0,
                    a * along.1 + b * across.1,
                );
                assert!(
                    pts.iter().any(|p| (p.0 - want.0).abs() < 1e-12 && (p.1 - want.1).abs() < 1e-12),
                    "missing corner {want:?} in {pts:?}"
                );
            }
        }
    }
    #[test]
    fn triangle_first_vertex_flips_on_the_alternating_pattern() {
        let up = rec(Pattern::TriangleAlt, 3.0, 0.5, 0, 0);
        let dn = rec(Pattern::TriangleAlt, 3.0, 0.5, 1, 0);
        assert_eq!(up.ops[0], Op::Move(0.0, 3.0));
        assert_eq!(dn.ops[0], Op::Move(0.0, -3.0));
        // Plain Triangle never flips.
        assert_eq!(rec(Pattern::Triangle, 3.0, 0.5, 1, 0).ops[0], Op::Move(0.0, 3.0));
        assert_eq!(rec(Pattern::Triangle, 3.0, 0.5, 0, 1).ops[0], Op::Move(0.0, 3.0));
    }

    #[test]
    fn square_and_mosaic_emit_identical_paths() {
        let a = rec(Pattern::Square, 2.5, 0.5, 0, 0);
        let b = rec(Pattern::Mosaic, 2.5, 0.5, 0, 0);
        assert_eq!(a.ops, b.ops);
        assert_eq!(
            a.ops,
            vec![
                Op::Move(-2.5, -2.5),
                Op::Line(2.5, -2.5),
                Op::Line(2.5, 2.5),
                Op::Line(-2.5, 2.5),
                Op::CloseFill,
            ]
        );
    }

    #[test]
    fn dot_bezier_control_points() {
        let r = 4.0;
        let k = f64::from(CIRCLE_K);
        let p = rec(Pattern::Dot, r, 0.5, 0, 0);
        assert_eq!(p.ops.len(), 6);
        assert_eq!(p.ops[0], Op::Move(r, 0.0));
        assert_eq!(p.ops[1], Op::Curve([r, r * k, r * k, r, 0.0, r]));
        assert_eq!(p.ops[2], Op::Curve([-r * k, r, -r, r * k, -r, 0.0]));
        assert_eq!(p.ops[3], Op::Curve([-r, -r * k, -r * k, -r, 0.0, -r]));
        assert_eq!(p.ops[4], Op::Curve([r * k, -r, r, -r * k, r, 0.0]));
        assert_eq!(p.ops[5], Op::CloseFill);
        assert!((k - 0.5523).abs() < 1e-7, "CIRCLE_K is a f32 constant: {k}");
    }

    #[test]
    fn line_h_and_line_v_are_transposed_bars() {
        let h = rec(Pattern::LineH, 2.0, 0.5, 0, 0);
        let v = rec(Pattern::LineV, 2.0, 0.5, 0, 0);
        assert_eq!(h.ops[0], Op::Move(-2.0, -1.0));
        assert_eq!(h.ops[1], Op::Line(2.0, -1.0));
        assert_eq!(v.ops[0], Op::Move(-1.0, -2.0));
        assert_eq!(v.ops[1], Op::Line(-1.0, 2.0));
    }

    #[test]
    fn areas_match_the_closed_forms() {
        let e = 2.0;
        assert!((shape_area_mm2(Pattern::Dot, e, 0.5) - std::f64::consts::PI * 4.0).abs() < 1e-12);
        assert!((shape_area_mm2(Pattern::Square, e, 0.5) - 16.0).abs() < 1e-12);
        assert!((shape_area_mm2(Pattern::Triangle, e, 0.5) - 3.0f64.sqrt() / 2.0 * 4.0).abs() < 1e-12);
        assert!((shape_area_mm2(Pattern::LineH, e, 0.5) - 4.0 * 4.0 * 0.5).abs() < 1e-12);
        // Pattern::area_factor agrees with the shape areas.
        assert!((Pattern::Dot.area_factor() * e * e - shape_area_mm2(Pattern::Dot, e, 0.5)).abs() < 1e-12);
    }
}