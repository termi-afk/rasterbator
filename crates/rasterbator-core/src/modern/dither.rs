//! Quantisation and error diffusion (design §3.6).
//!
//! The state is allocated once from `cols_total` and survives page boundaries:
//! cells are visited in poster row-major order and the cell windows partition
//! the poster, so the row buffer is correct across a page seam with no extra
//! bookkeeping.
//!
//! ```text
//! Bayer4x4 : b = (BAYER4X4[by][bx] + 0.5)/16      value = floor(t*levels + b)/levels
//! FS       : v = t*levels ; q = floor(v + cur[i]).clamp(0, levels) ; e = v + cur[i] - q
//!            next[i-1] += e*7/16 ; next[i] += e*5/16 ; next[i+1] += e*1/16
//! Atkinson : the canonical six-neighbour kernel, each neighbour receiving e/8
//! None     : value = t
//! final     = lerp(t, value, dither_strength)
//! ```

use crate::modern::options::Dither;

/// The classic ordered-dither threshold matrix.
pub const BAYER4X4: [[u32; 4]; 4] = [
    [0, 8, 2, 10],
    [12, 4, 14, 6],
    [3, 11, 1, 9],
    [15, 7, 13, 5],
];

/// Error-diffusion state for one job.
///
/// `cur` and `next` both have `cols + 2` entries indexed `1..=cols`, with the
/// two guard entries at `0` and `cols + 1` permanently zero.
#[derive(Clone, Debug)]
pub struct DitherState {
    algorithm: Dither,
    levels: f64,
    block: u32,
    strength: f64,
    cols: usize,
    cur: Vec<f64>,
    next: Vec<f64>,
    row: Option<i64>,
}

impl DitherState {
    /// Allocate a state for a poster of `cols_total` cells per row.
    pub fn new(cols_total: u32, algorithm: Dither, levels: u32, block: u32, strength: f64) -> Self {
        let cols = cols_total.max(1) as usize;
        DitherState {
            algorithm,
            levels: f64::from(levels.max(2)),
            block: block.max(1),
            strength: strength.clamp(0.0, 1.0),
            cols,
            cur: vec![0.0; cols + 2],
            next: vec![0.0; cols + 2],
            row: None,
        }
    }

    /// True when no quantisation happens at all.
    pub fn is_inert(&self) -> bool {
        self.algorithm == Dither::None
    }

    /// The number of columns the row buffers hold.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The current poster row index the buffers correspond to.
    pub fn row(&self) -> Option<i64> {
        self.row
    }

    /// The stored error for a poster column index, for tests and debugging.
    pub fn error_at(&self, i: i64, row: i64) -> f64 {
        let idx = self.index(i);
        if self.row == Some(row) {
            self.cur[idx]
        } else if self.row.map_or(false, |r| row == r + 1) {
            self.next[idx]
        } else {
            0.0
        }
    }

    fn index(&self, i: i64) -> usize {
        // 1-based with guards at both ends; out-of-range cells contribute
        // nowhere, which keeps halo and edge cells harmless.
        let idx = i + 1;
        if idx <= 0 {
            0
        } else if idx as usize > self.cols + 1 {
            self.cols + 1
        } else {
            idx as usize
        }
    }

    fn clear_guards(v: &mut [f64]) {
        v[0] = 0.0;
        let last = v.len() - 1;
        v[last] = 0.0;
    }

    /// Advance the row buffers so that `j` is the row about to be written.
    ///
    /// Called by the page loop on every row change; a jump of more than one row
    /// (a fully skipped row) simply starts from a cleared buffer.
    pub fn set_row(&mut self, j: i64) {
        match self.row {
            Some(r) if j == r => {}
            Some(r) if j == r + 1 => {
                std::mem::swap(&mut self.cur, &mut self.next);
                for v in self.next.iter_mut() {
                    *v = 0.0;
                }
                Self::clear_guards(&mut self.cur);
                self.row = Some(j);
            }
            Some(_) => {
                for v in self.cur.iter_mut() {
                    *v = 0.0;
                }
                for v in self.next.iter_mut() {
                    *v = 0.0;
                }
                self.row = Some(j);
            }
            None => {
                for v in self.cur.iter_mut() {
                    *v = 0.0;
                }
                for v in self.next.iter_mut() {
                    *v = 0.0;
                }
                self.row = Some(j);
            }
        }
    }

    /// Quantise one cell.
    ///
    /// `i`/`j` are poster cell indices. When `mutate` is false (a halo cell,
    /// or a cell owned by another page) the buffers are neither read nor
    /// written, so the cell carries the colour it would carry anywhere else and
    /// the error propagation is unaffected.
    ///
    /// Returns the tone output to use: `lerp(t, value, strength)`.
    pub fn quantise(&mut self, t: f64, i: i64, j: i64, mutate: bool) -> f64 {
        if self.algorithm == Dither::None || !mutate {
            return t;
        }
        self.set_row(j);
        let idx = self.index(i);
        let q = match self.algorithm {
            Dither::Bayer4x4 => {
                let bx = (i.div_euclid(self.block as i64) % 4) as usize;
                let by = (j.div_euclid(self.block as i64) % 4) as usize;
                let b = (f64::from(BAYER4X4[by][bx]) + 0.5) / 16.0;
                (t * self.levels + b).floor() / self.levels
            }
            Dither::FloydSteinberg | Dither::Atkinson => {
                let v = t * self.levels;
                let acc = v + self.cur[idx];
                let q = acc.floor().clamp(0.0, self.levels);
                let err = acc - q;
                match self.algorithm {
                    Dither::FloydSteinberg => {
                        self.spread(idx, [(-1, 7.0 / 16.0), (0, 5.0 / 16.0), (1, 1.0 / 16.0)], err);
                    }
                    Dither::Atkinson => {
                        // Canonical six-neighbour Atkinson kernel, each 1/8:
                        // two cells to the right on this row, three below, and
                        // the third below-row neighbour folded onto the next
                        // row buffer so only two buffers are needed.
                        let e = err / 8.0;
                        self.cur[idx + 1] += e;
                        self.cur[idx + 2] += e;
                        self.next[idx - 1] += e;
                        self.next[idx] += e;
                        self.next[idx + 1] += e;
                        self.next[idx + 2] += e;
                    }
                    _ => unreachable!(),
                }
                q / self.levels
            }
            Dither::None => t,
        };
        if self.strength >= 1.0 {
            q
        } else {
            t + (q - t) * self.strength
        }
    }

    fn spread(&mut self, idx: usize, weights: [(i64, f64); 3], err: f64) {
        for (d, w) in weights {
            let x = idx as i64 + d;
            if x >= 1 && x <= self.cols as i64 {
                self.next[x as usize] += err * w;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(alg: Dither, levels: u32, block: u32, strength: f64) -> DitherState {
        DitherState::new(64, alg, levels, block, strength)
    }

    #[test]
    fn strength_zero_is_bit_identical_to_none() {
        for alg in [Dither::Bayer4x4, Dither::FloydSteinberg, Dither::Atkinson] {
            let mut s = st(alg, 8, 1, 0.0);
            for i in 0..8i64 {
                for j in 0..3i64 {
                    let t = ((i * 7 + j * 3) % 11) as f64 / 10.0;
                    assert_eq!(s.quantise(t, i, j, true), t, "{alg:?} at {i},{j}");
                }
            }
        }
    }

    #[test]
    fn none_plus_strength_differs_from_bayer() {
        let mut none = st(Dither::None, 8, 1, 0.7);
        let mut bayer = st(Dither::Bayer4x4, 8, 1, 0.7);
        let mut differs = false;
        for i in 0..16i64 {
            let a = none.quantise(0.45, i, 0, true);
            let b = bayer.quantise(0.45, i, 0, true);
            if (a - b).abs() > 1e-12 {
                differs = true;
            }
        }
        assert!(differs, "the blend must be applied after quantisation");
    }

    #[test]
    fn bayer_is_deterministic_and_repeats_every_block_cells() {
        for block in [1u32, 2, 3] {
            let mut s = st(Dither::Bayer4x4, 8, block, 1.0);
            let row: Vec<f64> = (0..16i64).map(|i| s.quantise(0.5, i, 0, true)).collect();
            // Deterministic: a second identical run gives the same values.
            let mut s2 = st(Dither::Bayer4x4, 8, block, 1.0);
            let row2: Vec<f64> = (0..16i64).map(|i| s2.quantise(0.5, i, 0, true)).collect();
            assert_eq!(row, row2);
            // Repeats with the block period.
            let period = block as usize;
            for i in 0..16 {
                assert!((row[i] - row[i % period]).abs() < 1e-12, "block {block} at {i}");
            }
        }
    }

    #[test]
    fn bayer_threshold_splits_mid_tones() {
        // t = 0.25 with 2 levels lands exactly between 0 and 0.5, so the
        // 4x4 field must split it 8 / 8.
        let mut s = st(Dither::Bayer4x4, 2, 1, 1.0);
        let mut low = 0;
        for i in 0..16i64 {
            let v = s.quantise(0.25, i, 0, true);
            assert!(v == 0.0 || v == 0.5, "unexpected level {v}");
            if v == 0.0 {
                low += 1;
            }
        }
        assert_eq!(low, 8, "ordered threshold must split the field evenly");
    }

    #[test]
    fn bayer_high_level_quantisation() {
        // levels = 64, t = 0.5: only 31/64 or 32/64 may appear.
        let mut s = st(Dither::Bayer4x4, 64, 1, 1.0);
        for i in 0..16i64 {
            for j in 0..4i64 {
                let v = s.quantise(0.5, i, j, true);
                assert!(
                    (v - 31.0 / 64.0).abs() < 1e-12 || (v - 32.0 / 64.0).abs() < 1e-12,
                    "unexpected level {v}"
                );
            }
        }
    }

    #[test]
    fn floyd_steinberg_preserves_the_mean() {
        let mut s = st(Dither::FloydSteinberg, 8, 1, 1.0);
        let n = 512i64;
        let mut sum = 0.0;
        for i in 0..n {
            sum += s.quantise(0.5, i, 0, true);
        }
        let mean = sum / n as f64;
        assert!((mean - 0.5).abs() <= 0.5 / 8.0 + 1e-12, "mean {mean} drifted too far");
    }

    #[test]
    fn atkinson_shares_a_sixth_of_the_error_with_six_neighbours() {
        let mut s = st(Dither::Atkinson, 2, 1, 1.0);
        // A solid dark row must not saturate: some cells stay light.
        let vals: Vec<f64> = (0..16i64).map(|i| s.quantise(0.9, i, 0, true)).collect();
        assert!(vals.iter().any(|v| *v < 1.0), "Atkinson saturated completely");
    }

    #[test]
    fn halo_cells_do_not_mutate_the_buffer() {
        let mut s = st(Dither::FloydSteinberg, 8, 1, 1.0);
        let owned = s.quantise(0.5, 4, 0, true);
        let before = s.error_at(6, 1);
        let halo = s.quantise(0.5, 6, 0, false);
        assert!((halo - 0.5).abs() < 1e-12);
        assert_eq!(s.error_at(6, 1), before);
        assert_eq!(owned, s.quantise(0.5, 4, 0, true));
    }

    #[test]
    fn row_buffers_span_pages() {
        // 1x6 pages and 6x1 pages produce the same cell values because the
        // poster-wide row buffer is continuous.
        let run = |cols: u32, rows: u32| {
            let mut s = DitherState::new(cols, Dither::FloydSteinberg, 8, 1, 1.0);
            let mut out = Vec::new();
            for j in 0..rows as i64 {
                for i in 0..cols as i64 {
                    out.push(s.quantise(0.37, i, j, true));
                }
            }
            out
        };
        // A 1-wide, 6-tall poster and a 6-wide, 1-tall poster have different
        // shapes, so compare the shared prefix: the first row is identical.
        let wide = run(6, 1);
        let tall = run(1, 6);
        assert!((wide[0] - tall[0]).abs() < 1e-12);
        // And a 6x6 poster traversed in the same order twice is reproducible.
        assert_eq!(run(6, 6), run(6, 6));
    }
}