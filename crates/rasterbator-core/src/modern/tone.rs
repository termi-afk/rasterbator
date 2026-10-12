//! Tone response and the dot-size transfer function (design §3.6).
//!
//! This is the heart of the rendering model and is deliberately the only place
//! where a square root appears: `sqrt(2.0)`.
//!
//! ```text
//! d_raw   = 1.0 - level                                          linear, not clamped
//! d_curve = None        => d_raw
//!           Exponential => d_raw.powf(exponent)
//!           Logarithmic => if k <= 1.0 { d_raw } else { ln(1 + d_raw*(k-1)) / ln(k) }
//! d_inv   = if invert { 1.0 - d_curve } else { d_curve }
//! d       = clamp(d_inv * darkness_scale, 0, 1)
//! t       = min + (max - min) * d                               linear lerp, NOT clamped
//! extent  = (pitch / sqrt(2.0)) * (t * family_scale)            millimetres
//! ```
//!
//! `t` is intentionally not clamped before the multiply: the tone band is a
//! gain and an offset, and an inverted band is rejected by validation instead
//! of being silently repaired.

use crate::modern::options::ToneCurve;

/// The tone-band and curve parameters of one job.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneParams {
    /// Size fraction at full white, where the darkness is zero.
    pub min: f64,
    /// Size fraction at full black, where the darkness is one.
    pub max: f64,
    /// Response curve applied to the raw darkness.
    pub curve: ToneCurve,
    /// Exponent for [`ToneCurve::Exponential`].
    pub exponent: f64,
    /// Strength `k` for [`ToneCurve::Logarithmic`]; `k <= 1` is the identity.
    pub log_k: f64,
}

impl Default for ToneParams {
    fn default() -> Self {
        ToneParams {
            min: 0.0,
            max: 1.0,
            curve: ToneCurve::None,
            exponent: 2.2,
            log_k: 8.0,
        }
    }
}

/// `1/sqrt(2)`: the full-cell lattice maximum extent divided by the pitch.
pub const INV_SQRT2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Apply the response curve to a raw darkness value in `0..=1`.
pub fn tone_curve(curve: ToneCurve, d_raw: f64, exponent: f64, log_k: f64) -> f64 {
    match curve {
        ToneCurve::None => d_raw,
        ToneCurve::Exponential => d_raw.powf(exponent),
        ToneCurve::Logarithmic => {
            if log_k <= 1.0 + 1e-12 {
                d_raw
            } else {
                ((1.0 + d_raw * (log_k - 1.0)).ln() / log_k.ln()).clamp(0.0, 1.0)
            }
        }
    }
}

/// Full transfer: cell level in `0..=1` to the tone output `t`.
///
/// `invert` flips the tone mapping; `darkness_scale` is `1.0` for every dot
/// pattern and is exposed as a parameter so alternative renderers (the plain
/// enlarge path, future pattern families) can reuse the curve.
pub fn transfer(level: f64, p: &ToneParams, invert: bool, darkness_scale: f64) -> f64 {
    let d_raw = 1.0 - level;
    let d_curve = tone_curve(p.curve, d_raw, p.exponent, p.log_k);
    let d_inv = if invert { 1.0 - d_curve } else { d_curve };
    let d = (d_inv * darkness_scale).clamp(0.0, 1.0);
    p.min + (p.max - p.min) * d
}

/// Dot half-extent in millimetres for a tone output `t`.
///
/// `e = (pitch / sqrt(2)) * (t * family_scale)`, with `family_scale` from
/// [`crate::modern::screen::family_scale`] so that `max_raster = 1.0` means
/// "fills this cell" for every family and coverage pair.
pub fn extent_mm(t: f64, pitch_mm: f64, family_scale: f64) -> f64 {
    (pitch_mm * INV_SQRT2) * (t * family_scale)
}

/// The plain screen has no shape: the tone output *is* the output tone.
///
/// `grey = round(255 * (1 - t))`, so `min_raster = 0.2` floors white at 204.
pub fn plain_grey(t: f64) -> u8 {
    (255.0 * (1.0 - t)).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> ToneParams {
        ToneParams::default()
    }

    /// The table of design §6.2: `pitch = 1 mm`, `family_scale = 1`,
    /// `min = 0`, `max = 1`.
    #[test]
    fn transfer_table_exact_extents() {
        let p = p();
        let cases: &[(f64, &str, f64)] = &[
            (0.0, "#000000 over white", 0.7071067812),
            (1.0, "#FFFFFF over white", 0.0),
            (0.5, "#FF0000 over white", 0.3535533906),
            (0.5, "#FFFF00 over white", 0.3535533906),
            (128.0 / 255.0, "#808080", 0.3521669067),
            (127.0 / 255.0, "#7F7F7F", 0.3549398745),
            (127.0 / 255.0, "#000000 a=128 over white gives 127", 0.3549398745),
            (105.0 / 255.0, "#0AC85A", 0.4159451654),
        ];
        for &(level, why, want) in cases {
            let t = transfer(level, &p, false, 1.0);
            let e = extent_mm(t, 1.0, 1.0);
            assert!(
                (e - want).abs() < 1e-9,
                "level {level} ({why}): extent {e:.10} != {want:.10}"
            );
        }
    }

    #[test]
    fn coverage_and_family_scale_multiply_the_extent() {
        let p = p();
        let full = extent_mm(transfer(0.0, &p, false, 1.0), 1.0, 1.0);
        let tangent = extent_mm(transfer(0.0, &p, false, 1.0), 1.0, INV_SQRT2);
        assert!((tangent - full * INV_SQRT2).abs() < 1e-12);
        let hex = extent_mm(transfer(0.0, &p, false, 1.0), 1.0, 0.6123724356957945);
        assert!((hex - full * 0.6123724356957945).abs() < 1e-12);
    }

    #[test]
    fn tone_band_is_a_gain_and_an_offset() {
        let mut b = p();
        b.min = 0.1;
        b.max = 0.9;
        for d in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let t = transfer(1.0 - d, &b, false, 1.0);
            let want = 0.1 + 0.8 * d;
            assert!((t - want).abs() < 1e-12, "d={d}: {t} != {want}");
            let e = extent_mm(t, 1.0, 1.0);
            assert!((e - INV_SQRT2 * want).abs() < 1e-12);
        }
    }

    #[test]
    fn exponential_curve_values() {
        let cases = [
            (0.25, 0.0473661427),
            (0.5, 0.2176376408),
            (0.75, 0.5310492251),
            (0.0, 0.0),
            (1.0, 1.0),
        ];
        for (d, want) in cases {
            let got = tone_curve(ToneCurve::Exponential, d, 2.2, 8.0);
            assert!((got - want).abs() < 1e-9, "exp d={d}: {got:.10} != {want:.10}");
        }
    }

    #[test]
    fn logarithmic_curve_values() {
        let cases = [
            (0.25, 0.4864772062),
            (0.5, 0.7233083338),
            (0.75, 0.8812853966),
            (0.0, 0.0),
            (1.0, 1.0),
        ];
        for (d, want) in cases {
            let got = tone_curve(ToneCurve::Logarithmic, d, 2.2, 8.0);
            assert!((got - want).abs() < 1e-9, "log d={d}: {got:.10} != {want:.10}");
        }
        for d in [0.0, 0.3, 1.0] {
            assert!((tone_curve(ToneCurve::Logarithmic, d, 2.2, 1.0) - d).abs() < 1e-12);
        }
    }

    #[test]
    fn invert_is_complementary_without_a_curve() {
        for i in 0..=100 {
            let d = f64::from(i) / 100.0;
            let a = transfer(d, &p(), true, 1.0);
            let b = transfer(1.0 - d, &p(), true, 1.0);
            assert!((a + b - 1.0).abs() < 1e-12, "d={d}: {a} + {b} != 1");
        }
    }

    #[test]
    fn level_is_not_gamma_corrected() {
        // #808080 must land on 128/255, not on a luma value.
        let level = 128.0 / 255.0;
        let t = transfer(level, &p(), false, 1.0);
        assert!((t - 127.0 / 255.0).abs() < 1e-12);
    }

    #[test]
    fn darkness_scale_clamps_the_darkness_only() {
        let p = p();
        assert!((transfer(0.0, &p, false, 0.5) - 0.5).abs() < 1e-12);
        assert!((transfer(0.0, &p, false, 2.0) - 1.0).abs() < 1e-12);
        assert_eq!(transfer(0.0, &p, false, 0.0), 0.0);
    }

    #[test]
    fn plain_grey_is_the_tone_plane() {
        assert_eq!(plain_grey(0.0), 255);
        assert_eq!(plain_grey(1.0), 0);
        assert_eq!(plain_grey(0.2), 204);
    }
}