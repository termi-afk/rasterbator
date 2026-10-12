//! Colour types and the compositing / emission rules of modern mode.
//!
//! Modern mode keeps every channel as `f64` in `0..=1` from decode to shape
//! emission, so a box average or a bilinear weight is exact and rounding to
//! `u8` happens exactly once, when a colour is handed to a page target.
//!
//! There is no gamma and no sRGB transfer anywhere: the cell level is the HSL
//! lightness of the composited value, exactly as the legacy engine computes
//! it, only with transparent pixels resolved against the paper colour first
//! (spec amendment A-11).

use std::fmt;

/// An 8-bit sRGB triple. No alpha: the engine is opaque by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Black.
    pub const BLACK: Rgb = Rgb(0, 0, 0);
    /// White.
    pub const WHITE: Rgb = Rgb(255, 255, 255);

    /// Build from `0..=1` components, rounding half away from zero.
    pub fn from_unit(r: f64, g: f64, b: f64) -> Rgb {
        fn q(v: f64) -> u8 {
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        }
        Rgb(q(r), q(g), q(b))
    }

    /// Components as `0..=1`.
    pub fn unit(self) -> (f64, f64, f64) {
        (f64::from(self.0) / 255.0, f64::from(self.1) / 255.0, f64::from(self.2) / 255.0)
    }

    /// Perceptual RGB distance, summed over channels (`0..=765`).
    ///
    /// Used by the dual-tone "these two colours are nearly the same" warning.
    pub fn distance(self, other: Rgb) -> f64 {
        f64::from(self.0.abs_diff(other.0))
            + f64::from(self.1.abs_diff(other.1))
            + f64::from(self.2.abs_diff(other.2))
    }

    /// Parse `RRGGBB`, `#RRGGBB` or `R,G,B` (decimal components).
    ///
    /// The accepted forms match the CLI help text exactly; anything else is an
    /// error naming the accepted shapes.
    pub fn parse(s: &str) -> Result<Rgb, String> {
        let t = s.trim();
        let bad = || {
            format!("invalid color '{s}': use RRGGBB, #RRGGBB or R,G,B")
        };
        if let Some(hex) = t.strip_prefix('#').or_else(|| {
            if t.len() == 6 && t.bytes().all(|b| b.is_ascii_hexdigit()) {
                Some(t)
            } else {
                None
            }
        }) {
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(bad());
            }
            let v = u32::from_str_radix(hex, 16).map_err(|_| bad())?;
            return Ok(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8));
        }
        let parts: Vec<&str> = t.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            let mut c = [0u8; 3];
            for (i, p) in parts.iter().enumerate() {
                let v: u16 = p.parse().map_err(|_| bad())?;
                if v > 255 {
                    return Err(bad());
                }
                c[i] = v as u8;
            }
            return Ok(Rgb(c[0], c[1], c[2]));
        }
        Err(bad())
    }

    /// Lowercase six-digit hex, without a leading `#`.
    pub fn to_hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// A linear (no gamma) `0..=1` RGBA sample value.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RgbaF64 {
    /// Red, `0..=1`.
    pub r: f64,
    /// Green, `0..=1`.
    pub g: f64,
    /// Blue, `0..=1`.
    pub b: f64,
    /// Alpha, `0..=1`.
    pub a: f64,
}

impl RgbaF64 {
    /// Opaque colour from `0..=1` components.
    pub fn rgb(r: f64, g: f64, b: f64) -> Self {
        RgbaF64 { r, g, b, a: 1.0 }
    }

    /// Opaque colour from an [`Rgb`].
    pub fn from_rgb(c: Rgb) -> Self {
        let (r, g, b) = c.unit();
        RgbaF64::rgb(r, g, b)
    }

    /// Build from 8-bit channels.
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        RgbaF64 {
            r: f64::from(r) / 255.0,
            g: f64::from(g) / 255.0,
            b: f64::from(b) / 255.0,
            a: f64::from(a) / 255.0,
        }
    }

    /// The same colour with a different alpha.
    pub fn with_alpha(self, a: f64) -> Self {
        RgbaF64 { a, ..self }
    }

    /// Round to opaque 8-bit.
    pub fn to_rgb(self) -> Rgb {
        Rgb::from_unit(self.r, self.g, self.b)
    }

    /// Premultiplied RGB.
    pub fn premultiplied(self) -> (f64, f64, f64) {
        (self.r * self.a, self.g * self.a, self.b * self.a)
    }
}

/// Source-over composite of an opaque 8-bit source over an opaque background.
///
/// `t = a / 255`, `out = src * t + paper * (1 - t)`. Rounding happens once,
/// here, so the result is exactly the `u8` a page target will see.
pub fn source_over_u8(src: Rgb, a: u8, paper: Rgb) -> Rgb {
    let t = f64::from(a) / 255.0;
    let (sr, sg, sb) = src.unit();
    let (pr, pg, pb) = paper.unit();
    Rgb::from_unit(
        sr * t + pr * (1.0 - t),
        sg * t + pg * (1.0 - t),
        sb * t + pb * (1.0 - t),
    )
}

/// Source-over composite of a linear sample over an opaque background.
///
/// Alpha is the source's own alpha; the result is opaque.
pub fn source_over(src: RgbaF64, paper: Rgb) -> RgbaF64 {
    let t = src.a.clamp(0.0, 1.0);
    let (pr, pg, pb) = paper.unit();
    RgbaF64 {
        r: src.r * t + pr * (1.0 - t),
        g: src.g * t + pg * (1.0 - t),
        b: src.b * t + pb * (1.0 - t),
        a: 1.0,
    }
}

/// HSL lightness of an 8-bit triple: `(max + min) / 2 / 255`.
///
/// This is the cell level metric; it is deliberately not gamma-corrected.
pub fn hsl_lightness(c: Rgb) -> f64 {
    let mx = c.0.max(c.1).max(c.2) as f64;
    let mn = c.0.min(c.1).min(c.2) as f64;
    (mx + mn) / 2.0 / 255.0
}

/// Cell level of a composited linear sample: `(max + min) / 2`.
pub fn level_of(v: RgbaF64) -> f64 {
    let mx = v.r.max(v.g).max(v.b);
    let mn = v.r.min(v.g).min(v.b);
    ((mx + mn) / 2.0).clamp(0.0, 1.0)
}

/// Intensity shading factor for a tone output `t` in `0..=1`.
///
/// `k = (1 - strength) + strength * t`: `1.0` at `t = 1`, `1 - strength` at
/// `t = 0`.
pub fn shading_factor(t: f64, strength: f64) -> f64 {
    let s = strength.clamp(0.0, 1.0);
    (1.0 - s) + s * t
}

/// Multiply a colour by the shading factor, rounding once.
pub fn shade(c: Rgb, factor: f64) -> Rgb {
    let (r, g, b) = c.unit();
    Rgb::from_unit(r * factor, g * factor, b * factor)
}

/// Per-channel inversion.
pub fn invert(c: Rgb) -> Rgb {
    Rgb(255 - c.0, 255 - c.1, 255 - c.2)
}

/// Mosaic high-contrast map (feature A4 / amendment A-8):
/// `c' = round(clamp((c - 127.5) * g + 127.5, 0, 255))`.
///
/// Identity at `g = 1.0`; at `g = 2.0` this is exactly
/// `clamp(2c - 127.5, 0, 255)`.
pub fn mosaic_contrast_channel(c: u8, gain: f64) -> u8 {
    let v = (f64::from(c) - 127.5) * gain + 127.5;
    v.clamp(0.0, 255.0).round() as u8
}

/// Apply [`mosaic_contrast_channel`] to every channel.
pub fn mosaic_contrast(c: Rgb, gain: f64) -> Rgb {
    Rgb(
        mosaic_contrast_channel(c.0, gain),
        mosaic_contrast_channel(c.1, gain),
        mosaic_contrast_channel(c.2, gain),
    )
}

/// Blend two linear colours: `a` at `0`, `b` at `1`.
pub fn lerp_rgb(a: Rgb, b: Rgb, u: f64) -> Rgb {
    let (ar, ag, ab) = a.unit();
    let (br, bg, bb) = b.unit();
    let u = u.clamp(0.0, 1.0);
    Rgb::from_unit(
        ar + (br - ar) * u,
        ag + (bg - ag) * u,
        ab + (bb - ab) * u,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_over_matches_expected() {
        assert_eq!(source_over_u8(Rgb::BLACK, 128, Rgb::WHITE), Rgb(127, 127, 127));
        assert_eq!(source_over_u8(Rgb::WHITE, 128, Rgb::WHITE), Rgb(255, 255, 255));
        assert_eq!(source_over_u8(Rgb::BLACK, 0, Rgb::WHITE), Rgb::WHITE);
        assert_eq!(source_over_u8(Rgb::WHITE, 255, Rgb::BLACK), Rgb::WHITE);
    }

    #[test]
    fn hsl_lightness_matches_spec() {
        assert!((hsl_lightness(Rgb(255, 0, 0)) - 0.5).abs() < 1e-12);
        assert!((hsl_lightness(Rgb(10, 200, 90)) - 105.0 / 255.0).abs() < 1e-12);
        assert!((hsl_lightness(Rgb(7, 7, 7)) - 7.0 / 255.0).abs() < 1e-12);
        // Not gamma-corrected: mid grey stays mid grey.
        assert!((hsl_lightness(Rgb(128, 128, 128)) - 128.0 / 255.0).abs() < 1e-12);
    }

    #[test]
    fn level_of_is_hsl_lightness_of_the_composite() {
        let v = source_over(RgbaF64::from_u8(0, 0, 0, 128), Rgb::WHITE);
        assert!((level_of(v) - 127.0 / 255.0).abs() < 1e-12);
    }

    #[test]
    fn mosaic_contrast_mapping() {
        assert_eq!(mosaic_contrast_channel(0, 2.0), 0);
        assert_eq!(mosaic_contrast_channel(64, 2.0), 1);
        assert_eq!(mosaic_contrast_channel(128, 2.0), 129);
        assert_eq!(mosaic_contrast_channel(255, 2.0), 255);
        for c in 0u16..=255 {
            let c = c as u8;
            assert_eq!(mosaic_contrast_channel(c, 1.0), c, "identity at gain 1");
        }
    }

    #[test]
    fn shading_factors() {
        assert!((shading_factor(1.0, 0.35) - 1.0).abs() < 1e-12);
        assert!((shading_factor(0.0, 0.35) - 0.65).abs() < 1e-12);
        assert_eq!(shade(Rgb::WHITE, 1.0), Rgb::WHITE);
        assert_eq!(shade(Rgb::WHITE, 0.65), Rgb(166, 166, 166));
    }

    #[test]
    fn parse_and_format() {
        assert_eq!(Rgb::parse("ff8000").unwrap(), Rgb(255, 128, 0));
        assert_eq!(Rgb::parse("#FF8000").unwrap(), Rgb(255, 128, 0));
        assert_eq!(Rgb::parse("255,128,0").unwrap(), Rgb(255, 128, 0));
        assert_eq!(Rgb::parse(" 0,0,0 ").unwrap(), Rgb::BLACK);
        assert!(Rgb::parse("nope").is_err());
        assert!(Rgb::parse("12345").is_err());
        assert!(Rgb::parse("255,256,0").is_err());
        assert_eq!(Rgb(255, 128, 0).to_string(), "ff8000");
    }

    #[test]
    fn invert_and_distance() {
        assert_eq!(invert(Rgb::BLACK), Rgb::WHITE);
        assert_eq!(invert(Rgb(10, 20, 30)), Rgb(245, 235, 225));
        assert_eq!(Rgb::BLACK.distance(Rgb::WHITE), 765.0);
        assert_eq!(Rgb::BLACK.distance(Rgb(1, 2, 3)), 6.0);
    }
}