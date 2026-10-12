//! The cell lattice: where cell centres sit, how big a cell is, and how the
//! coverage constant scales the transfer (design §3.1).
//!
//! Coordinates are poster millimetres with the origin at the top-left, `x` to
//! the right and `y` downwards. The lattice is anchored to the **poster**, not
//! to any sheet, so phase is continuous across every page seam.

use crate::modern::options::{Coverage, ScreenFamily, Stagger};

/// `cos(30 deg)`: the vertical pitch factor of the honeycomb screen.
pub const COS30: f64 = 0.866_025_403_784_438_6;

/// `1/sqrt(2)`: the full-cell lattice maximum extent divided by the pitch.
pub const INV_SQRT2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Maximum shape half-extent divided by the pitch, for each (family, coverage).
///
/// | family | coverage | k | family_scale = k*sqrt(2) |
/// |---|---|---|---|
/// | Lattice / Plain | FullCell | 1/sqrt(2) | 1.0 |
/// | Lattice / Plain | Tangent | 0.5 | 0.7071067811865476 |
/// | Hex | FullCell | cos30/2 | 0.6123724356957945 |
/// | Hex | Tangent | cos30/(2 sqrt2) | 0.4330127018922193 |
pub fn family_scale(family: ScreenFamily, coverage: Coverage) -> f64 {
    match (family, coverage) {
        (ScreenFamily::Lattice, Coverage::FullCell) | (ScreenFamily::Plain, Coverage::FullCell) => 1.0,
        (ScreenFamily::Lattice, Coverage::Tangent) | (ScreenFamily::Plain, Coverage::Tangent) => INV_SQRT2,
        (ScreenFamily::Hex, Coverage::FullCell) => COS30 * INV_SQRT2,
        (ScreenFamily::Hex, Coverage::Tangent) => COS30 / 2.0,
    }
}

/// Vertical cell pitch for a screen family.
///
/// `pitch` for the square lattice and the plain screen, `pitch * cos30` for
/// the honeycomb.
pub fn cell_height(family: ScreenFamily, pitch_mm: f64) -> f64 {
    match family {
        ScreenFamily::Hex => pitch_mm * COS30,
        _ => pitch_mm,
    }
}

/// The cell lattice of one job.
///
/// Cheap to copy; holds no allocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenLattice {
    /// Screen family.
    pub family: ScreenFamily,
    /// Grid pitch in millimetres (exactly the nominal dot size, amendment A-7).
    pub pitch_mm: f64,
    /// Vertical cell pitch in millimetres.
    pub cell_h_mm: f64,
    /// Row stagger; ignored by [`ScreenFamily::Hex`].
    pub stagger: Stagger,
    /// Lattice phase shift in millimetres.
    pub phase: (f64, f64),
    /// Assembled poster size in millimetres (the lattice extent).
    pub poster: (f64, f64),
}

impl ScreenLattice {
    /// Build a lattice for a job.
    pub fn new(
        family: ScreenFamily,
        pitch_mm: f64,
        stagger: Stagger,
        phase: (f64, f64),
        poster: (f64, f64),
    ) -> Self {
        ScreenLattice {
            family,
            pitch_mm,
            cell_h_mm: cell_height(family, pitch_mm),
            stagger,
            phase,
            poster,
        }
    }

    /// Centre of cell `(i, j)` in poster millimetres.
    ///
    /// * lattice: `((i + 0.5) * pitch, (j + 0.5) * pitch)`
    /// * lattice + half pitch: odd rows shift by `pitch / 2`
    /// * hex: `((i + 0.5 + 0.5*(j & 1)) * pitch, (j + 0.5) * pitch * cos30)`
    ///
    /// plus the phase shift in both cases.
    pub fn cell_centre(&self, i: i64, j: i64) -> (f64, f64) {
        let (p, (phx, phy)) = (self.pitch_mm, self.phase);
        match self.family {
            ScreenFamily::Hex => (
                (i as f64 + 0.5 + 0.5 * (j & 1) as f64) * p + phx,
                (j as f64 + 0.5) * self.cell_h_mm + phy,
            ),
            ScreenFamily::Lattice | ScreenFamily::Plain => {
                let stagger = if self.stagger == Stagger::HalfPitch && (j & 1) == 1 {
                    0.5 * p
                } else {
                    0.0
                };
                ((i as f64 + 0.5) * p + stagger + phx, (j as f64 + 0.5) * self.cell_h_mm + phy)
            }
        }
    }

    /// Sample box of cell `(i, j)`: `[x0, y0, x1, y1]` in poster millimetres.
    ///
    /// The box is `[cx -/+ pitch/2] x [cy -/+ cell_h/2]` clipped to the poster
    /// rectangle. An empty result (box entirely outside the poster) returns
    /// `None`.
    pub fn sample_box(&self, i: i64, j: i64) -> Option<[f64; 4]> {
        let (cx, cy) = self.cell_centre(i, j);
        let x0 = (cx - self.pitch_mm / 2.0).max(0.0);
        let y0 = (cy - self.cell_h_mm / 2.0).max(0.0);
        let x1 = (cx + self.pitch_mm / 2.0).min(self.poster.0);
        let y1 = (cy + self.cell_h_mm / 2.0).min(self.poster.1);
        if x1 <= x0 || y1 <= y0 {
            None
        } else {
            Some([x0, y0, x1, y1])
        }
    }

    /// Number of columns that cover the poster.
    pub fn cols_for(&self, width_mm: f64) -> u32 {
        (width_mm / self.pitch_mm).ceil().max(0.0) as u32
    }

    /// Number of rows that cover the poster.
    pub fn rows_for(&self, height_mm: f64) -> u32 {
        (height_mm / self.cell_h_mm).ceil().max(0.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::options::Stagger;

    fn lattice(family: ScreenFamily, stagger: Stagger, phase: (f64, f64)) -> ScreenLattice {
        ScreenLattice::new(family, 1.0, stagger, phase, (100.0, 100.0))
    }

    #[test]
    fn lattice_centres() {
        let l = lattice(ScreenFamily::Lattice, Stagger::Off, (0.0, 0.0));
        assert_eq!(l.cell_centre(0, 0), (0.5, 0.5));
        assert_eq!(l.cell_centre(1, 0), (1.5, 0.5));
        assert_eq!(l.cell_centre(0, 1), (0.5, 1.5));
    }

    #[test]
    fn half_pitch_stagger_centres() {
        let l = lattice(ScreenFamily::Lattice, Stagger::HalfPitch, (0.0, 0.0));
        assert_eq!(l.cell_centre(0, 0).0, 0.5);
        assert_eq!(l.cell_centre(0, 1).0, 1.0);
        assert_eq!(l.cell_centre(1, 1).0, 2.0);
        // Row 0 is unchanged.
        assert_eq!(l.cell_centre(1, 0).0, 1.5);
    }

    #[test]
    fn hex_centres() {
        let l = lattice(ScreenFamily::Hex, Stagger::HalfPitch, (0.0, 0.0));
        // Hex ignores the stagger.
        for i in 0..3 {
            assert!((l.cell_centre(i, 0).0 - (i as f64 + 0.5)).abs() < 1e-12);
        }
        assert!((l.cell_centre(0, 0).1 - 0.4330127019).abs() < 1e-9);
        for i in 0..3 {
            assert!((l.cell_centre(i, 1).0 - (i as f64 + 1.0)).abs() < 1e-12);
        }
        assert!((l.cell_centre(0, 1).1 - 1.2990381057).abs() < 1e-9);
        assert!((l.cell_centre(0, 2).1 - 2.1650635096).abs() < 1e-9);
    }

    #[test]
    fn phase_shifts_every_centre_exactly() {
        let base = lattice(ScreenFamily::Lattice, Stagger::Off, (0.0, 0.0));
        let moved = lattice(ScreenFamily::Lattice, Stagger::Off, (2.0, 3.0));
        for i in 0..5 {
            for j in 0..5 {
                let a = base.cell_centre(i, j);
                let b = moved.cell_centre(i, j);
                assert!((b.0 - a.0 - 2.0).abs() < 1e-12);
                assert!((b.1 - a.1 - 3.0).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn adjacent_page_cells_differ_by_exactly_one_pitch() {
        // Phase continuity across a seam: the cell after the last one of a page
        // is offset by exactly `pitch` in `i` and nothing in `j`.
        let l = lattice(ScreenFamily::Lattice, Stagger::Off, (0.0, 0.0));
        let last = l.cell_centre(18, 5);
        let first = l.cell_centre(19, 5);
        assert!((first.0 - last.0 - l.pitch_mm).abs() < 1e-12);
        assert!((first.1 - last.1).abs() < 1e-12);
    }

    #[test]
    fn family_scale_table() {
        assert_eq!(family_scale(ScreenFamily::Lattice, Coverage::FullCell), 1.0);
        assert_eq!(family_scale(ScreenFamily::Plain, Coverage::FullCell), 1.0);
        assert!((family_scale(ScreenFamily::Lattice, Coverage::Tangent) - INV_SQRT2).abs() < 1e-15);
        assert!((family_scale(ScreenFamily::Hex, Coverage::FullCell) - 0.6123724356957945).abs() < 1e-15);
        assert!((family_scale(ScreenFamily::Hex, Coverage::Tangent) - 0.4330127018922193).abs() < 1e-15);
        // k = family_scale / sqrt(2)
        let k = |f, c| family_scale(f, c) * INV_SQRT2;
        assert!((k(ScreenFamily::Lattice, Coverage::FullCell) - INV_SQRT2).abs() < 1e-15);
        assert!((k(ScreenFamily::Lattice, Coverage::Tangent) - 0.5).abs() < 1e-15);
        assert!((k(ScreenFamily::Hex, Coverage::FullCell) - COS30 / 2.0).abs() < 1e-15);
        assert!((k(ScreenFamily::Hex, Coverage::Tangent) - COS30 / 2.0 * INV_SQRT2).abs() < 1e-15);
    }

    #[test]
    fn cell_height_table() {
        assert_eq!(cell_height(ScreenFamily::Lattice, 10.0), 10.0);
        assert_eq!(cell_height(ScreenFamily::Plain, 10.0), 10.0);
        assert!((cell_height(ScreenFamily::Hex, 10.0) - 8.660254037844386).abs() < 1e-12);
    }

    #[test]
    fn sample_boxes_are_clipped_to_the_poster() {
        let l = ScreenLattice::new(ScreenFamily::Lattice, 10.0, Stagger::Off, (0.0, 0.0), (25.0, 15.0));
        assert_eq!(l.sample_box(0, 0), Some([0.0, 0.0, 10.0, 10.0]));
        let b = l.sample_box(2, 1).unwrap();
        assert!((b[0] - 20.0).abs() < 1e-12 && (b[2] - 25.0).abs() < 1e-12);
        assert!(l.sample_box(-3, 0).is_none());
        assert!(l.sample_box(0, 9).is_none());
    }
}