//! Source preparation (design §3.3).
//!
//! Every step is a whole-buffer pass in a fixed order, so the result depends
//! only on the input bytes and the options:
//!
//! ```text
//! 1. decode            guards: file <= 100 MB, w*h <= 100 MP
//! 2. exif_orientation  (applied at decode time, see source.rs)
//! 3. rotate            0 / 90 / 180 / 270, exact transpose, no resampling
//! 4. crop              source-pixel rectangle, clamped to the decoded bounds
//! 5. adjust            brightness -> contrast -> saturation
//! 6. alpha             a < alpha_threshold -> paper colour
//! 7. resample          optional box filter to the requested resolution
//! ```
//!
//! The optional resample is a box filter over the integer source-pixel
//! rectangle of each destination pixel, so it is exact and order-independent.

use crate::modern::color::{Rgb, RgbaF64};
use crate::modern::options::{CropRectPx, ModernOptions, Rotation};
use crate::modern::source::{PreparedImage, SourceInfo, MAX_FILE_BYTES, MAX_PIXELS};

/// Preparation settings, derived from [`ModernOptions`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrepareOptions {
    /// Quarter-turn applied before sizing.
    pub rotate: Rotation,
    /// Crop rectangle in source pixels, before the rotation.
    pub crop: Option<CropRectPx>,
    /// Brightness in percent (`-100..=100`).
    pub brightness: f64,
    /// Contrast in percent (`-100..=100`).
    pub contrast: f64,
    /// Saturation in percent (`-100..=100`).
    pub saturation: f64,
    /// Pixels with `a < alpha_threshold` become the paper colour.
    pub alpha_threshold: u8,
    /// Box-filter the source down to the cell grid first.
    pub resample: bool,
    /// Resolution the optional resample targets.
    pub resample_dpi: u16,
    /// Exact pixel size the resample targets: the cell grid of the source
    /// rectangle, capped by `resample_dpi`. `None` leaves the pixels as they
    /// are. See [`resample_target_for`].
    pub resample_target: Option<(u32, u32)>,
    /// Colour transparent pixels resolve to.
    pub paper_color: Rgb,
}

impl PrepareOptions {
    /// Extract the preparation settings from an option set.
    pub fn from_options(o: &ModernOptions) -> Self {
        PrepareOptions {
            rotate: o.rotate,
            crop: o.crop,
            brightness: o.brightness,
            contrast: o.contrast,
            saturation: o.saturation,
            alpha_threshold: o.alpha_threshold,
            resample: o.resample,
            resample_dpi: o.resample_dpi,
            resample_target: None,
            paper_color: o.paper_color,
        }
    }
}

/// The resample size for a source rectangle: one pixel per cell of the screen,
/// capped so the image never exceeds `resample_dpi` over the poster.
///
/// `None` when resampling is off. One pixel per cell is what keeps a
/// low-resolution source from aliasing: every cell then has its own pixel to
/// average, instead of most cells falling between two source pixels.
pub fn resample_target_for(o: &ModernOptions, rect_mm: [f64; 4]) -> Option<(u32, u32)> {
    if !o.resample || o.resample_dpi == 0 {
        return None;
    }
    let rw = (rect_mm[2] - rect_mm[0]).max(0.0);
    let rh = (rect_mm[3] - rect_mm[1]).max(0.0);
    if rw <= 0.0 || rh <= 0.0 {
        return None;
    }
    let pitch = o.nominal_dot_mm.max(1e-6);
    let cell_h = crate::modern::screen::cell_height(o.screen, pitch).max(1e-6);
    let dpi = f64::from(o.resample_dpi);
    let tw = (rw / pitch).ceil().min(rw / 25.4 * dpi).max(1.0) as u32;
    let th = (rh / cell_h).ceil().min(rh / 25.4 * dpi).max(1.0) as u32;
    Some((tw.max(1), th.max(1)))
}

/// Inspect, decode and prepare a source file in one call.
pub fn prepare(
    path: &std::path::Path,
    opts: &PrepareOptions,
) -> Result<(SourceInfo, PreparedImage), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("{}: file is over the 100 MB limit", path.display()));
    }
    let img = crate::modern::source::decode_rgba(path)?;
    let w = img.width();
    let h = img.height();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(format!("{}: image is over the 100 megapixel limit", path.display()));
    }
    let info = SourceInfo { path: path.to_path_buf(), width: w, height: h, exif_swaps_axes: false };
    let prepared = prepare_image(&img, opts)?;
    Ok((info, prepared))
}

/// Run the preparation pipeline on an already-decoded 8-bit RGBA buffer.
pub fn prepare_image(
    img: &image::RgbaImage,
    opts: &PrepareOptions,
) -> Result<PreparedImage, String> {
    let mut cur = img.clone();
    // 3. rotate (an exact transpose, never a resample)
    cur = match opts.rotate {
        Rotation::D0 => cur,
        Rotation::D90 => image::imageops::rotate90(&cur),
        Rotation::D180 => image::imageops::rotate180(&cur),
        Rotation::D270 => image::imageops::rotate270(&cur),
    };
    // 4. crop
    cur = crop(&cur, opts.crop);
    // 5./6. convert to linear and adjust
    let mut px = to_linear(&cur);
    adjust(&mut px, cur.width(), cur.height(), opts);
    apply_alpha_threshold(&mut px, cur.width(), cur.height(), opts);
    // 7. optional resample to the cell grid
    if opts.resample {
        if let Some((tw, th)) = opts.resample_target {
            let tw = tw.max(1);
            let th = th.max(1);
            if tw != cur.width() || th != cur.height() {
                let (_, _, res) = box_resample(&px, cur.width(), cur.height(), tw, th);
                px = res;
                return Ok(PreparedImage { width: tw, height: th, pixels: px, resampled: true });
            }
        }
    }
    Ok(PreparedImage { width: cur.width(), height: cur.height(), pixels: px, resampled: false })
}

/// Crop to a source-pixel rectangle, clamped to the decoded bounds.
///
/// A rectangle that does not intersect the image yields a 1x1 paper pixel.
pub fn crop(img: &image::RgbaImage, rect: Option<CropRectPx>) -> image::RgbaImage {
    let Some(r) = rect else { return img.clone() };
    let w = i64::from(img.width());
    let h = i64::from(img.height());
    let x1 = r.x1.clamp(0, w);
    let y1 = r.y1.clamp(0, h);
    let x2 = r.x2.clamp(0, w);
    let y2 = r.y2.clamp(0, h);
    if x2 <= x1 || y2 <= y1 {
        return image::RgbaImage::new(1, 1);
    }
    let sub = image::imageops::crop_imm(img, x1 as u32, y1 as u32, (x2 - x1) as u32, (y2 - y1) as u32);
    sub.to_image()
}

fn to_linear(img: &image::RgbaImage) -> Vec<RgbaF64> {
    let mut v = Vec::with_capacity(img.width() as usize * img.height() as usize * 4);
    for p in img.pixels() {
        v.push(RgbaF64::from_u8(p.0[0], p.0[1], p.0[2], p.0[3]));
    }
    v
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Brightness, then contrast, then saturation, in that order.
fn adjust(px: &mut [RgbaF64], w: u32, h: u32, opts: &PrepareOptions) {
    let n = (w as usize) * (h as usize);
    if n > px.len() {
        return;
    }
    let b = opts.brightness / 100.0;
    let c = opts.contrast / 100.0;
    let s = opts.saturation / 100.0;
    if b == 0.0 && c == 0.0 && s == 0.0 {
        return;
    }
    for p in px.iter_mut().take(n) {
        if b != 0.0 {
            p.r = clamp01(p.r + b);
            p.g = clamp01(p.g + b);
            p.b = clamp01(p.b + b);
        }
        if c != 0.0 {
            p.r = clamp01((p.r - 0.5) * (1.0 + c) + 0.5);
            p.g = clamp01((p.g - 0.5) * (1.0 + c) + 0.5);
            p.b = clamp01((p.b - 0.5) * (1.0 + c) + 0.5);
        }
        if s != 0.0 {
            let l = 0.2126 * p.r + 0.7152 * p.g + 0.0722 * p.b;
            p.r = clamp01(l + (p.r - l) * (1.0 + s));
            p.g = clamp01(l + (p.g - l) * (1.0 + s));
            p.b = clamp01(l + (p.b - l) * (1.0 + s));
        }
    }
}

/// Pixels below the alpha threshold become the paper colour (feature F3).
fn apply_alpha_threshold(px: &mut [RgbaF64], w: u32, h: u32, opts: &PrepareOptions) {
    let n = (w as usize) * (h as usize);
    let paper = RgbaF64::from_rgb(opts.paper_color);
    let t = f64::from(opts.alpha_threshold);
    for p in px.iter_mut().take(n) {
        if p.a * 255.0 < t {
            *p = paper;
        }
    }
}

/// Box-filter resample to `tw x th`.
///
/// The filter averages whole source pixels, so it is exact, order-independent
/// and free of cross-row float accumulation.
pub fn box_resample(src: &[RgbaF64], sw: u32, sh: u32, tw: u32, th: u32) -> (u32, u32, Vec<RgbaF64>) {
    let tw = tw.max(1);
    let th = th.max(1);
    let mut out = Vec::with_capacity(tw as usize * th as usize);
    for dy in 0..th {
        let y0 = (u64::from(dy) * u64::from(sh)) / u64::from(th);
        let y1 = ((u64::from(dy) + 1) * u64::from(sh)) / u64::from(th);
        for dx in 0..tw {
            let x0 = (u64::from(dx) * u64::from(sw)) / u64::from(tw);
            let x1 = ((u64::from(dx) + 1) * u64::from(sw)) / u64::from(tw);
            let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
            let mut n = 0.0f64;
            for y in y0..y1.max(y0 + 1) {
                for x in x0..x1.max(x0 + 1) {
                    if x >= u64::from(sw) || y >= u64::from(sh) {
                        continue;
                    }
                    let p = src[y as usize * sw as usize + x as usize];
                    r += p.r;
                    g += p.g;
                    b += p.b;
                    a += p.a;
                    n += 1.0;
                }
            }
            if n == 0.0 {
                out.push(RgbaF64::default());
            } else {
                out.push(RgbaF64 { r: r / n, g: g / n, b: b / n, a: a / n });
            }
        }
    }
    (tw, th, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::options::ModernOptions;

    fn base() -> PrepareOptions {
        PrepareOptions {
            rotate: Rotation::D0,
            crop: None,
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            alpha_threshold: 0,
            resample: false,
            resample_dpi: 300,
            resample_target: None,
            paper_color: Rgb::WHITE,
        }
    }

    fn checker(w: u32, h: u32, step: u32) -> image::RgbaImage {
        let mut img = image::RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let on = ((x / step) + (y / step)) % 2 == 0;
                let g: u8 = if on { 255 } else { 0 };
            img.put_pixel(x, y, image::Rgba([g, g, g, 255]));
            }
        }
        img
    }

    #[test]
    fn rotate_transposes_exactly() {
        let img = image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 4]));
        let mut o = base();
        o.rotate = Rotation::D90;
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!((p.width, p.height), (2, 3));
        o.rotate = Rotation::D270;
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!((p.width, p.height), (2, 3));
        o.rotate = Rotation::D180;
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!((p.width, p.height), (3, 2));
    }

    #[test]
    fn crop_clamps_to_bounds() {
        let img = image::RgbaImage::from_pixel(10, 8, image::Rgba([1, 2, 3, 4]));
        let mut o = base();
        o.crop = Some(CropRectPx { x1: 2, y1: 3, x2: 6, y2: 7 });
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!((p.width, p.height), (4, 4));
        o.crop = Some(CropRectPx { x1: -5, y1: -5, x2: 100, y2: 100 });
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!((p.width, p.height), (10, 8));
        o.crop = Some(CropRectPx { x1: 5, y1: 5, x2: 2, y2: 2 });
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!((p.width, p.height), (1, 1));
    }

    #[test]
    fn adjustments_use_the_documented_formulas() {
        let mut o = base();
        o.brightness = 50.0;
        let mut px = vec![RgbaF64::rgb(0.5, 0.5, 0.5)];
        adjust(&mut px, 1, 1, &o);
        assert!((px[0].r - 1.0).abs() < 1e-12);

        o = base();
        o.contrast = 100.0;
        let mut px = vec![RgbaF64::rgb(0.75, 0.5, 0.25)];
        adjust(&mut px, 1, 1, &o);
        assert!((px[0].r - 1.0).abs() < 1e-12);
        assert!((px[0].g - 0.5).abs() < 1e-12);
        assert!((px[0].b - 0.0).abs() < 1e-12);

        o = base();
        o.saturation = -100.0;
        let mut px = vec![RgbaF64::rgb(1.0, 0.0, 0.0)];
        adjust(&mut px, 1, 1, &o);
        let l = 0.2126 + 0.0 + 0.0;
        assert!((px[0].r - l).abs() < 1e-12);
        assert!((px[0].g - l).abs() < 1e-12);
    }

    #[test]
    fn alpha_threshold_replaces_transparent_pixels_with_paper() {
        let mut img = image::RgbaImage::from_pixel(2, 1, image::Rgba([10, 20, 30, 255]));
        img.put_pixel(1, 0, image::Rgba([10, 20, 30, 100]));
        let mut o = base();
        o.alpha_threshold = 128;
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!(p.pixel(0, 0).unwrap().to_rgb(), Rgb(10, 20, 30));
        assert_eq!(p.pixel(1, 0).unwrap().to_rgb(), Rgb::WHITE);

        o.alpha_threshold = 0;
        let p = prepare_image(&img, &o).unwrap();
        assert_eq!(p.pixel(1, 0).unwrap().to_rgb(), Rgb(10, 20, 30));
    }

    #[test]
    fn resample_moves_a_checkerboard_towards_mid_grey() {
        // A one-pixel checkerboard aliases at cell size and averages to mid
        // grey once the source is resampled.
        let img = checker(64, 64, 1);
        let mut o = base();
        o.resample = true;
        o.resample_target = Some((1, 1));
        let p = prepare_image(&img, &o).unwrap();
        assert!(p.resampled);
        assert_eq!((p.width, p.height), (1, 1));
        let v = p.pixel(0, 0).unwrap();
        let level = (v.r.max(v.g).max(v.b) + v.r.min(v.g).min(v.b)) / 2.0;
        assert!((level - 0.5).abs() < 1e-9, "resampled level {level}");

        o.resample = false;
        let p = prepare_image(&img, &o).unwrap();
        assert!(!p.resampled);
        assert_eq!(p.width, 64);
        let v = p.pixel(0, 0).unwrap();
        assert!((v.r - 1.0).abs() < 1e-12, "unsampled first pixel stays pure");
    }
    #[test]
    fn box_resample_averages_exactly() {
        // 2x2: [0, 100; 200, 255] -> 138.75
        let src = vec![
            RgbaF64::rgb(0.0, 0.0, 0.0),
            RgbaF64::rgb(100.0 / 255.0, 100.0 / 255.0, 100.0 / 255.0),
            RgbaF64::rgb(200.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0),
            RgbaF64::rgb(1.0, 1.0, 1.0),
        ];
        let (_, _, out) = box_resample(&src, 2, 2, 1, 1);
        let mean = (0.0 + 100.0 / 255.0 + 200.0 / 255.0 + 1.0) / 4.0;
        assert!((out[0].r - mean).abs() < 1e-12);
        assert!((mean * 255.0 - 138.75).abs() < 1e-9);
    }

    #[test]
    fn prepare_options_come_from_the_option_set() {
        let mut o = ModernOptions::default();
        o.rotate = Rotation::D180;
        o.alpha_threshold = 200;
        o.resample_dpi = 150;
        let p = PrepareOptions::from_options(&o);
        assert_eq!(p.rotate, Rotation::D180);
        assert_eq!(p.alpha_threshold, 200);
        assert_eq!(p.resample_dpi, 150);
        assert_eq!(p.paper_color, Rgb::WHITE);
    }

    #[test]
    fn resample_target_is_the_cell_grid_capped_by_dpi() {
        let mut o = ModernOptions::default();
        o.nominal_dot_mm = 12.0;
        o.resample_dpi = 300;
        // 297 mm at a 12 mm pitch: 25 x 25 cells; the 300 dpi cap is far above.
        assert_eq!(resample_target_for(&o, [61.5, 0.0, 358.5, 297.0]), Some((25, 25)));
        // A tiny pitch is capped by the requested resolution.
        o.nominal_dot_mm = 0.01;
        let (tw, th) = resample_target_for(&o, [0.0, 0.0, 297.0, 297.0]).unwrap();
        assert_eq!(tw, (297.0f64 / 25.4 * 300.0) as u32);
        assert_eq!(th, tw);
        // Resampling off -> no target.
        o.resample = false;
        assert!(resample_target_for(&o, [0.0, 0.0, 297.0, 297.0]).is_none());
    }
}