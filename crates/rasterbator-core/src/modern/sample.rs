//! Per-cell sampling (design §3.4).
//!
//! A cell sample box in poster millimetres is mapped to source pixels through
//! the inverse of [`JobGeometry::source_rect_mm`](crate::modern::geometry::JobGeometry::source_rect_mm).
//! Every sampled pixel is composited source-over the paper colour *before*
//! aggregation, so a transparent region reads as paper rather than black.
//!
//! The cell level is the HSL lightness of the composited value,
//! `(max(R,G,B) + min(R,G,B)) / 2` in `0..=1`. No gamma, no sRGB transfer.

use crate::modern::color::{level_of, source_over, Rgb, RgbaF64};
use crate::modern::options::Sampling;
use crate::modern::source::PreparedImage;

/// The result of sampling one cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellValue {
    /// The composited, linear colour of the cell.
    pub rgba: RgbaF64,
    /// The cell level: HSL lightness in `0..=1`.
    pub level: f64,
    /// False when the cell's sample box missed the source rectangle entirely.
    pub inside: bool,
}

impl CellValue {
    /// The paper colour, level 1 (white).
    pub fn paper() -> Self {
        CellValue { rgba: RgbaF64::from_rgb(Rgb::WHITE), level: 1.0, inside: false }
    }

    /// The 8-bit rendering of [`CellValue::rgba`].
    pub fn rgb(self) -> Rgb {
        self.rgba.to_rgb()
    }
}

/// Samples source pixels for the cells of one job.
#[derive(Clone, Copy, Debug)]
pub struct CellSampler<'a> {
    img: &'a PreparedImage,
    paper: Rgb,
    method: Sampling,
    /// The image rectangle in poster millimetres, `[x0, y0, x1, y1]`.
    rect: [f64; 4],
}

impl<'a> CellSampler<'a> {
    /// Build a sampler.
    pub fn new(
        img: &'a PreparedImage,
        paper: Rgb,
        method: Sampling,
        rect: [f64; 4],
    ) -> Self {
        CellSampler { img, paper, method, rect }
    }

    /// The paper colour transparent pixels resolve to.
    pub fn paper(&self) -> Rgb {
        self.paper
    }

    /// The sampling method.
    pub fn method(&self) -> Sampling {
        self.method
    }

    /// Sample the cell whose poster-millimetre box is `box_mm`.
    pub fn sample(&self, box_mm: [f64; 4]) -> CellValue {
        let (w, h) = (f64::from(self.img.width), f64::from(self.img.height));
        if self.rect[2] - self.rect[0] <= 0.0 || self.rect[3] - self.rect[1] <= 0.0 {
            return CellValue::paper();
        }
        // Poster mm -> source pixels through the inverse of source_rect_mm.
        let sx0 = (box_mm[0] - self.rect[0]) / (self.rect[2] - self.rect[0]) * w;
        let sx1 = (box_mm[2] - self.rect[0]) / (self.rect[2] - self.rect[0]) * w;
        let sy0 = (box_mm[1] - self.rect[1]) / (self.rect[3] - self.rect[1]) * h;
        let sy1 = (box_mm[3] - self.rect[1]) / (self.rect[3] - self.rect[1]) * h;
        match self.method {
            Sampling::BoxAverage => self.box_average(sx0, sy0, sx1, sy1),
            Sampling::Nearest => self.nearest((sx0 + sx1) / 2.0, (sy0 + sy1) / 2.0),
            Sampling::Bilinear => self.bilinear((sx0 + sx1) / 2.0, (sy0 + sy1) / 2.0),
        }
    }

    /// Mean of every pixel whose integer centre falls in the box, weighted by
    /// alpha; pixels outside the image contribute paper with `a = 1`.
    fn box_average(&self, sx0: f64, sy0: f64, sx1: f64, sy1: f64) -> CellValue {
        // Pixel `p` has its centre at `p + 0.5`, so the covered range is
        // `[ceil(lo - 0.5), ceil(hi - 0.5))`.
        let x0 = (sx0 - 0.5).ceil() as i64;
        let x1 = (sx1 - 0.5).ceil() as i64;
        let y0 = (sy0 - 0.5).ceil() as i64;
        let y1 = (sy1 - 0.5).ceil() as i64;
        let mut acc = RgbaF64::default();
        let mut wsum = 0.0;
        let mut inside = false;
        for y in y0..y1 {
            for x in x0..x1 {
                let comp = match self.img.pixel(x, y) {
                    Some(p) => {
                        inside = true;
                        source_over(p, self.paper)
                    }
                    None => RgbaF64::from_rgb(self.paper),
                };
                let wa = comp.a.max(0.0);
                acc.r += comp.r * wa;
                acc.g += comp.g * wa;
                acc.b += comp.b * wa;
                wsum += wa;
            }
        }
        if wsum <= 0.0 || !inside {
            return CellValue::paper();
        }
        let v = RgbaF64 { r: acc.r / wsum, g: acc.g / wsum, b: acc.b / wsum, a: 1.0 };
        CellValue { level: level_of(v), rgba: v, inside: true }
    }

    /// The single pixel at the box centre.
    fn nearest(&self, sx: f64, sy: f64) -> CellValue {
        let px = sx.floor() as i64;
        let py = sy.floor() as i64;
        self.from_pixel(px, py)
    }

    /// Bilinear interpolation on premultiplied RGBA at the box centre, then
    /// unpremultiplied so a transparent neighbour cannot bleed its colour.
    fn bilinear(&self, sx: f64, sy: f64) -> CellValue {
        let w = f64::from(self.img.width);
        let h = f64::from(self.img.height);
        if sx < 0.0 || sy < 0.0 || sx >= w || sy >= h {
            return CellValue::paper();
        }
        let fx = sx - 0.5;
        let fy = sy - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let xi = x0 as i64;
        let yi = y0 as i64;
        let fetch = |x: i64, y: i64| -> RgbaF64 {
            match self.img.pixel(x, y) {
                Some(p) => p,
                None => RgbaF64::from_rgb(self.paper).with_alpha(0.0),
            }
        };
        let p00 = fetch(xi, yi).premultiplied();
        let p10 = fetch(xi + 1, yi).premultiplied();
        let p01 = fetch(xi, yi + 1).premultiplied();
        let p11 = fetch(xi + 1, yi + 1).premultiplied();
        let mut pr = 0.0;
        let mut pg = 0.0;
        let mut pb = 0.0;
        let mut pa = 0.0;
        for (di, weight) in [(0usize, (1.0 - tx) * (1.0 - ty)), (1, tx * (1.0 - ty))] {
            for (dj, w2) in [(0usize, 1.0 - ty), (1, ty)] {
                let wgt = weight * w2;
                let (r, g, b, a) = if di == 0 && dj == 0 {
                    (p00.0, p00.1, p00.2, fetch(xi, yi).a)
                } else if di == 1 && dj == 0 {
                    (p10.0, p10.1, p10.2, fetch(xi + 1, yi).a)
                } else if di == 0 {
                    (p01.0, p01.1, p01.2, fetch(xi, yi + 1).a)
                } else {
                    (p11.0, p11.1, p11.2, fetch(xi + 1, yi + 1).a)
                };
                pr += r * wgt;
                pg += g * wgt;
                pb += b * wgt;
                pa += a * wgt;
            }
        }
        let comp = if pa <= 1e-12 {
            RgbaF64::from_rgb(self.paper)
        } else {
            RgbaF64 { r: pr / pa, g: pg / pa, b: pb / pa, a: 1.0 }
        };
        CellValue { level: level_of(comp), rgba: comp, inside: true }
    }

    fn from_pixel(&self, x: i64, y: i64) -> CellValue {
        match self.img.pixel(x, y) {
            Some(p) => {
                let comp = source_over(p, self.paper);
                CellValue { level: level_of(comp), rgba: comp, inside: true }
            }
            None => CellValue::paper(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::source::PreparedImage;

    const W: u32 = 4;
    const H: u32 = 4;

    fn from_fn(f: impl Fn(u32, u32) -> image::Rgba<u8>) -> PreparedImage {
        let mut img = image::RgbaImage::new(W, H);
        for y in 0..H {
            for x in 0..W {
                img.put_pixel(x, y, f(x, y));
            }
        }
        PreparedImage::from_rgba(&img)
    }

    /// A sampler covering the whole image with the poster rect `[0,0,W,H]`.
    fn sampler(img: &PreparedImage, m: Sampling) -> CellSampler<'_> {
        CellSampler::new(img, Rgb::WHITE, m, [0.0, 0.0, f64::from(W), f64::from(H)])
    }

    fn solid(v: u8) -> PreparedImage {
        from_fn(|_, _| image::Rgba([v, v, v, 255]))
    }

    #[test]
    fn constant_image_agrees_across_methods() {
        let img = solid(0x20);
        for m in Sampling::ALL {
            let s = sampler(&img, *m);
            let v = s.sample([0.0, 0.0, 4.0, 4.0]);
            assert!((v.level - 32.0 / 255.0).abs() < 1e-9, "{m:?} box {:?}", v.level);
            let v2 = s.sample([0.2, 0.2, 0.9, 0.9]);
            assert!((v2.level - 32.0 / 255.0).abs() < 1e-9, "{m:?} small box");
        }
    }

    #[test]
    fn half_black_half_white() {
        // Left half black, right half white.
        let img = from_fn(|x, _| match x { 0 | 1 => image::Rgba([0, 0, 0, 255]), _ => image::Rgba([255, 255, 255, 255]) });
        let box_mm = [0.0, 0.0, 4.0, 4.0];

        let s = sampler(&img, Sampling::BoxAverage);
        let v = s.sample(box_mm);
        assert!((v.level - 0.5).abs() < 1e-9, "box average level {}", v.level);

        let s = sampler(&img, Sampling::Nearest);
        let left = s.sample([0.0, 0.0, 1.4, 4.0]);
        assert!(left.level < 1e-9, "nearest in the black half");
        let right = s.sample([2.6, 0.0, 4.0, 4.0]);
        assert!(right.level > 1.0 - 1e-9, "nearest in the white half");

        let s = sampler(&img, Sampling::Bilinear);
        let mid = s.sample([1.0, 0.0, 3.0, 4.0]);
        assert!((mid.level - 0.5).abs() < 1e-6, "bilinear at the boundary {}", mid.level);
    }

    #[test]
    fn out_of_image_is_paper_for_every_method() {
        let img = solid(0);
        let s = CellSampler::new(&img, Rgb::WHITE, Sampling::BoxAverage, [0.0, 0.0, 4.0, 4.0]);
        let outside = s.sample([10.0, 10.0, 14.0, 14.0]);
        assert!(!outside.inside);
        assert_eq!(outside.level, 1.0);
        assert_eq!(outside.rgb(), Rgb::WHITE);

        let s = CellSampler::new(&img, Rgb::WHITE, Sampling::Nearest, [0.0, 0.0, 4.0, 4.0]);
        let outside = s.sample([10.0, 10.0, 14.0, 14.0]);
        assert!(!outside.inside);
        assert_eq!(outside.level, 1.0);

        let s = CellSampler::new(&img, Rgb::WHITE, Sampling::Bilinear, [0.0, 0.0, 4.0, 4.0]);
        let outside = s.sample([10.0, 10.0, 14.0, 14.0]);
        assert!(!outside.inside);
        assert_eq!(outside.level, 1.0);
    }

    #[test]
    fn bilinear_does_not_bleed_across_transparency() {
        // One fully transparent pixel next to opaque black.
        let img = from_fn(|x, _| match x { 0 => image::Rgba([255, 0, 0, 0]), _ => image::Rgba([0, 0, 0, 255]) });
        let s = sampler(&img, Sampling::Bilinear);
        let v = s.sample([0.0, 0.0, 2.0, 4.0]);
        assert_eq!(v.rgb(), Rgb::BLACK, "no halo from the transparent pixel");
    }

    #[test]
    fn transparent_pixels_resolve_to_paper() {
        let img = from_fn(|_, _| image::Rgba([0, 0, 0, 0]));
        for m in Sampling::ALL {
            let s = sampler(&img, *m);
            let v = s.sample([0.0, 0.0, 4.0, 4.0]);
            assert_eq!(v.level, 1.0, "{m:?}");
            assert_eq!(v.rgb(), Rgb::WHITE, "{m:?}");
        }
    }

    #[test]
    fn custom_paper_colour_is_used() {
        let img = from_fn(|_, _| image::Rgba([0, 0, 0, 0]));
        let s = CellSampler::new(&img, Rgb(255, 0, 0), Sampling::Nearest, [0.0, 0.0, 4.0, 4.0]);
        assert_eq!(s.sample([1.0, 1.0, 2.0, 2.0]).rgb(), Rgb(255, 0, 0));
    }

    #[test]
    fn empty_rect_is_paper() {
        let img = solid(0);
        let s = CellSampler::new(&img, Rgb::WHITE, Sampling::BoxAverage, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(s.sample([0.0, 0.0, 4.0, 4.0]).rgb(), Rgb::WHITE);
    }

    #[test]
    fn resampling_puts_at_least_one_source_pixel_in_every_cell() {
        // A 4x4 two-pixel checkerboard: raw sampling at cell resolution sees
        // single pixels (aliasing), resampling averages each cell's box.
        let img = from_fn(|x, y| match (x + y) % 2 {
            0 => image::Rgba([0, 0, 0, 255]),
            _ => image::Rgba([255, 255, 255, 255]),
        });
        let raw = CellSampler::new(&img, Rgb::WHITE, Sampling::BoxAverage, [0.0, 0.0, 4.0, 4.0]);
        let mut px = img.pixels.clone();
        // Resample to 2x2 the way `prepare` does at 72 dpi: each destination
        // pixel is the mean of a 2x2 source box, which is mid grey.
        for p in px.iter_mut() {
            p.a = 1.0;
        }
        let (w, h, out) = crate::modern::prepare::box_resample(&px, 4, 4, 1, 1);        let filtered = PreparedImage { width: w, height: h, pixels: out, resampled: true };
        let flt = CellSampler::new(&filtered, Rgb::WHITE, Sampling::BoxAverage, [0.0, 0.0, 4.0, 4.0]);

        // Raw: the cell box covers the whole image and half of it is black.
        let a = raw.sample([0.0, 0.0, 4.0, 4.0]).level;
        assert!((a - 0.5).abs() < 1e-9, "raw box level {a}");
        // Resampled: every cell sees an averaged pixel.
        let b = flt.sample([0.0, 0.0, 4.0, 4.0]).level;
        assert!((b - 0.5).abs() < 1e-9, "filtered level {b}");
        // A single raw pixel is pure; the same cell after resampling is mid grey.
        let pure = raw.sample([0.0, 0.0, 1.0, 1.0]).level;
        let mixed = flt.sample([0.0, 0.0, 4.0, 4.0]).level;
        assert_ne!(pure, mixed);
        assert!((mixed - 0.5).abs() < (pure - 0.5).abs());
    }}
