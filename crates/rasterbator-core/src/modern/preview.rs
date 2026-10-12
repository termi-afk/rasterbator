//! Preview rendering and the shared rasteriser (design §5.4).
//!
//! [`BitmapTarget`] is a [`PageTarget`] that rasterises the page's vector ops
//! into an RGBA8 buffer, bounded to [`MAX_PREVIEW_PX`] on the long side, with
//! 4x4 supersampled circles and scanline-polygon fill elsewhere. Because it is
//! a [`PageTarget`], the preview and the export run exactly the same page
//! composer and differ only in the sink.
//!
//! [`RasterCanvas`] is the pixel buffer the **raster render path** uses: the
//! page composer fills it instead of issuing vector ops, encodes it once, and
//! hands it to the target as a single image. Sharing it here keeps one
//! rasteriser for both paths.
//!
//! The preview detail setting (amendment A-3) is a GUI setting, deliberately
//! not part of [`ModernOptions`](crate::modern::ModernOptions).

use crate::modern::color::Rgb;
use crate::modern::geometry::{CellWindow, JobGeometry, PageBoxes, PT_PER_MM};
use crate::modern::options::{ModernOptions, Pattern};
use crate::modern::page::Frame;
use crate::modern::pattern::shape_area_mm2;
use crate::modern::session::{EncodedImage, ImageComponent, PageTarget, RenderError};

/// Longest preview edge in pixels.
pub const MAX_PREVIEW_PX: u32 = 2048;

/// Supersampling grid per axis for circle antialiasing.
pub const CIRCLE_AA: usize = 4;

/// GUI preview detail (amendment A-3): a cell budget, not an engine option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PreviewDetail {
    /// 100 000 cells.
    Low,
    /// 250 000 cells (the default).
    #[default]
    Medium,
    /// 1 000 000 cells.
    High,
}

impl PreviewDetail {
    /// Every variant, low to high.
    pub const ALL: &'static [PreviewDetail] =
        &[PreviewDetail::Low, PreviewDetail::Medium, PreviewDetail::High];

    /// The cell budget of this detail level.
    pub fn cell_cap(self) -> u64 {
        match self {
            PreviewDetail::Low => 100_000,
            PreviewDetail::Medium => 250_000,
            PreviewDetail::High => 1_000_000,
        }
    }

    /// CLI / settings-file token.
    pub fn cli_str(self) -> &'static str {
        match self {
            PreviewDetail::Low => "low",
            PreviewDetail::Medium => "medium",
            PreviewDetail::High => "high",
        }
    }

    /// Parse a settings token.
    pub fn parse_cli(s: &str) -> Option<PreviewDetail> {
        let t = s.trim().to_ascii_lowercase();
        PreviewDetail::ALL.iter().copied().find(|d| d.cli_str() == t)
    }
}

/// The cell stride that keeps a preview under a cell budget.
///
/// `k = max(1, ceil(sqrt(total_cells / cap)))`. Every visited cell keeps its
/// canonical centre and transfer; only the density of the lattice is reduced.
pub fn preview_stride(total_cells: u64, cap: u64) -> u32 {
    if total_cells == 0 || cap == 0 {
        return 1;
    }
    let k = (total_cells as f64 / cap as f64).sqrt().ceil();
    if !(k >= 1.0) {
        1
    } else if k >= 1000.0 {
        1000
    } else {
        k as u32
    }
}

/// One rendered preview page.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewFrame {
    /// The page this frame shows.
    pub page: u32,
    /// Buffer width in pixels.
    pub width: u32,
    /// Buffer height in pixels.
    pub height: u32,
    /// Row-major RGBA8.
    pub rgba: Vec<u8>,
    /// The cell stride used.
    pub stride: u32,
    /// Shapes emitted.
    pub shapes: u64,
    /// Ink area of the page in square points.
    pub ink_area_pt2: f64,
    /// Cell area of the page in square points.
    pub cell_area_pt2: f64,
}

impl PreviewFrame {
    /// Ink density of this page, in percent.
    pub fn ink_percent(&self) -> f64 {
        if self.cell_area_pt2 <= 0.0 {
            0.0
        } else {
            100.0 * self.ink_area_pt2 / self.cell_area_pt2
        }
    }

    /// The frame as an `image::RgbaImage`.
    pub fn to_image(&self) -> image::RgbaImage {
        image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .unwrap_or_else(|| image::RgbaImage::new(self.width.max(1), self.height.max(1)))
    }
}

/// An RGBA8 pixel buffer addressed in page points.
pub struct RasterCanvas {
    width: u32,
    height: u32,
    sx: f64,
    sy: f64,
    px: Vec<u8>,
}

impl RasterCanvas {
    /// A transparent-white canvas of `width x height` pixels at scale 1.
    pub fn new(width: u32, height: u32) -> RasterCanvas {
        let w = width.max(1);
        let h = height.max(1);
        RasterCanvas {
            width: w,
            height: h,
            sx: 1.0,
            sy: 1.0,
            px: vec![255u8; (w as usize) * (h as usize) * 4],
        }
    }

    /// A canvas covering `[x0, x1] x [y0, y1]` in page points at the given
    /// points-per-pixel scale.
    pub fn with_scale(width: u32, height: u32, sx: f64, sy: f64) -> RasterCanvas {
        let mut c = RasterCanvas::new(width, height);
        c.sx = if sx > 0.0 { sx } else { 1.0 };
        c.sy = if sy > 0.0 { sy } else { 1.0 };
        c
    }

    /// A canvas sized to a page at the given resolution, capped to
    /// [`MAX_PREVIEW_PX`] on the long side.
    pub fn for_page(boxes: &PageBoxes, dpi: u16) -> RasterCanvas {
        let pw = PageBoxes::width(&boxes.media).max(1.0);
        let ph = PageBoxes::height(&boxes.media).max(1.0);
        let r = f64::from(dpi) / 72.0;
        let mut w = (pw * r).round().max(1.0);
        let mut h = (ph * r).round().max(1.0);
        let long = w.max(h);
        if long > f64::from(MAX_PREVIEW_PX) {
            let k = long / f64::from(MAX_PREVIEW_PX);
            w /= k;
            h /= k;
        }
        RasterCanvas::with_scale(w.round() as u32, h.round() as u32, w / pw, h / ph)
    }

    /// Buffer size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The `(sx, sy)` points-to-pixels factors.
    pub fn scales(&self) -> (f64, f64) {
        (self.sx, self.sy)
    }

    /// The raw RGBA bytes.
    pub fn pixels(&self) -> &[u8] {
        &self.px
    }

    fn blend(&mut self, x: i64, y: i64, c: Rgb, a: f64) {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return;
        }
        let a = a.clamp(0.0, 1.0);
        if a <= 0.0 {
            return;
        }
        let i = (y as usize) * (self.width as usize) + (x as usize);
        let src = [f64::from(c.0), f64::from(c.1), f64::from(c.2)];
        for k in 0..3 {
            let old = f64::from(self.px[i * 4 + k]);
            let new = src[k] * a + old * (1.0 - a);
            self.px[i * 4 + k] = new.round().clamp(0.0, 255.0) as u8;
        }
        self.px[i * 4 + 3] = 255;
    }

    /// Fill every pixel with a colour.
    pub fn fill_all(&mut self, c: Rgb) {
        for px in self.px.chunks_exact_mut(4) {
            px[0] = c.0;
            px[1] = c.1;
            px[2] = c.2;
            px[3] = 255;
        }
    }

    /// Fill the axis-aligned rectangle `[x0, y0, x1, y1]` in page points.
    pub fn fill_rect(&mut self, b: [f64; 4], c: Rgb) {
        let x0 = (b[0] * self.sx).round() as i64;
        let x1 = (b[2] * self.sx).round() as i64;
        let y0 = (b[1] * self.sy).round() as i64;
        let y1 = (b[3] * self.sy).round() as i64;
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend(x, y, c, 1.0);
            }
        }
    }

    /// Fill the rectangle of a cell sample box given in poster millimetres.
    pub fn fill_rect_box(&mut self, frame: Frame, box_mm: [f64; 4], c: Rgb) {
        self.fill_rect(
            [
                frame.x(box_mm[0]),
                frame.y(box_mm[3]),
                frame.x(box_mm[2]),
                frame.y(box_mm[1]),
            ],
            c,
        );
    }

    /// Fill a convex polygon given in page points with a scanline fill.
    pub fn fill_polygon(&mut self, pts: &[(f64, f64)], c: Rgb) {
        if pts.len() < 3 {
            return;
        }
        let min_x = pts.iter().fold(f64::MAX, |m, p| m.min(p.0)).max(0.0);
        let max_x = pts.iter().fold(f64::MIN, |m, p| m.max(p.0)).min(f64::from(self.width));
        let min_y = pts.iter().fold(f64::MAX, |m, p| m.min(p.1)).max(0.0);
        let max_y = pts.iter().fold(f64::MIN, |m, p| m.max(p.1)).min(f64::from(self.height));
        let y0 = (min_y * self.sy).floor() as i64;
        let y1 = (max_y * self.sy).ceil() as i64;
        for py in y0..y1 {
            let yc = (py as f64 + 0.5) / self.sy;
            let mut xs: Vec<f64> = Vec::new();
            for k in 0..pts.len() {
                let a = pts[k];
                let b = pts[(k + 1) % pts.len()];
                if (a.1 <= yc && b.1 > yc) || (b.1 <= yc && a.1 > yc) {
                    let t = (yc - a.1) / (b.1 - a.1);
                    xs.push(a.0 + t * (b.0 - a.0));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mut k = 0;
            while k + 1 < xs.len() {
                let xa = (xs[k].max(min_x) * self.sx).round() as i64;
                let xb = (xs[k + 1].min(max_x) * self.sx).round() as i64;
                for px in xa..xb {
                    self.blend(px, py, c, 1.0);
                }
                k += 2;
            }
        }
    }

    /// Fill a circle with 4x4 supersampled coverage.
    pub fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, c: Rgb) {
        if r <= 0.0 {
            return;
        }
        let x0 = ((cx - r) * self.sx).floor() as i64 - 1;
        let x1 = ((cx + r) * self.sx).ceil() as i64 + 1;
        let y0 = ((cy - r) * self.sy).floor() as i64 - 1;
        let y1 = ((cy + r) * self.sy).ceil() as i64 + 1;
        let r2 = r * r;
        let n = (CIRCLE_AA * CIRCLE_AA) as f64;
        let step_x = 1.0 / (CIRCLE_AA as f64 * self.sx);
        let step_y = 1.0 / (CIRCLE_AA as f64 * self.sy);
        for py in y0..y1 {
            for px in x0..x1 {
                let mut hits = 0.0;
                for sy in 0..CIRCLE_AA {
                    for sx in 0..CIRCLE_AA {
                        let dx = (px as f64 + (sx as f64 + 0.5) * step_x) / self.sx - cx;
                        let dy = (py as f64 + (sy as f64 + 0.5) * step_y) / self.sy - cy;
                        if dx * dx + dy * dy <= r2 {
                            hits += 1.0;
                        }
                    }
                }
                if hits > 0.0 {
                    self.blend(px, py, c, hits / n);
                }
            }
        }
    }

    /// Fill a pattern shape given in poster millimetres.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_shape(
        &mut self,
        frame: Frame,
        pattern: Pattern,
        centre: (f64, f64),
        e: f64,
        t: f64,
        i: i64,
        j: i64,
        line_aspect: f64,
        c: Rgb,
    ) -> Result<(), RenderError> {
        let to_pt = |x: f64, y: f64| (frame.x(x), frame.y(y));
        match pattern {
            Pattern::Dot => {
                self.fill_circle(to_pt(centre.0, centre.1).0, to_pt(centre.0, centre.1).1, e * PT_PER_MM, c);
            }
            Pattern::Square | Pattern::Mosaic => {
                self.fill_polygon(
                    &[
                        to_pt(centre.0 - e, centre.1 - e),
                        to_pt(centre.0 + e, centre.1 - e),
                        to_pt(centre.0 + e, centre.1 + e),
                        to_pt(centre.0 - e, centre.1 + e),
                    ],
                    c,
                );
            }
            Pattern::Triangle | Pattern::TriangleAlt => {
                let flip = pattern == Pattern::TriangleAlt && (i + j) & 1 == 1;
                let s3 = 3.0f64.sqrt() / 2.0;
                let verts = [(0.0, 1.0), (-s3, -0.5), (s3, -0.5)];
                let pts: Vec<(f64, f64)> = verts
                    .iter()
                    .map(|(a, b)| {
                        let (a, b) = if flip { (-a, -b) } else { (*a, *b) };
                        to_pt(centre.0 + a * e, centre.1 + b * e)
                    })
                    .collect();
                self.fill_polygon(&pts, c);
            }
            Pattern::LineH | Pattern::LineV | Pattern::LineMorph => {
                let (along, across) = match pattern {
                    Pattern::LineH => ((e, 0.0), (0.0, e * line_aspect)),
                    Pattern::LineV => ((0.0, e), (e * line_aspect, 0.0)),
                    _ => crate::modern::pattern::line_morph_axes(e, t, line_aspect),
                };
                let corner = |a: f64, b: f64| {
                    to_pt(
                        centre.0 + a * along.0 + b * across.0,
                        centre.1 + a * along.1 + b * across.1,
                    )
                };
                self.fill_polygon(
                    &[
                        corner(-1.0, -1.0),
                        corner(1.0, -1.0),
                        corner(1.0, 1.0),
                        corner(-1.0, 1.0),
                    ],
                    c,
                );
            }
        }
        Ok(())
    }

    /// Encode the canvas as a baseline JPEG and strip the JFIF `APP0` segment.
    ///
    /// The encoder always writes a leading `FF E0 0010` JFIF marker; removing it
    /// unconditionally leaves a clean, deterministic `DCTDecode` payload.
    pub fn encode_jpeg(&self, quality: u8) -> Result<EncodedImage, RenderError> {
        let img = image::RgbaImage::from_raw(self.width, self.height, self.px.clone())
            .ok_or_else(|| RenderError::Encode("preview buffer size mismatch".into()))?;
        let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
            .encode_image(&rgb)
            .map_err(|e| RenderError::Encode(format!("jpeg encode failed: {e}")))?;
        Ok(EncodedImage {
            w: self.width,
            h: self.height,
            component: ImageComponent::Rgb,
            bytes: strip_jfif(&out),
        })
    }
}

/// Remove the leading JFIF `APP0` segment from a JPEG stream.
pub fn strip_jfif(data: &[u8]) -> Vec<u8> {
    if data.len() > 5 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF && data[3] == 0xE0 {
        let len = usize::from(data[4]) * 256 + usize::from(data[5]) + 2;
        if len + 2 <= data.len() {
            let mut v = Vec::with_capacity(data.len() - len);
            v.push(0xFF);
            v.push(0xD8);
            v.extend_from_slice(&data[2 + len..]);
            return v;
        }
    }
    data.to_vec()
}

fn polygon_area(pts: &[(f64, f64)]) -> f64 {
    let mut a = 0.0;
    for k in 0..pts.len() {
        let p = pts[k];
        let q = pts[(k + 1) % pts.len()];
        a += p.0 * q.1 - q.0 * p.1;
    }
    (a / 2.0).abs()
}

/// A [`PageTarget`] that rasterises vector ops into an RGBA buffer.
pub struct BitmapTarget {
    canvas: RasterCanvas,
    fill: Rgb,
    stroke: Rgb,
    width: f64,
    path: Vec<(f64, f64)>,
    shapes: u64,
    ink_area_pt2: f64,
    cell_area_pt2: f64,
}

impl BitmapTarget {
    /// A target rendering the given page boxes at the given resolution.
    pub fn new(boxes: &PageBoxes, dpi: u16) -> BitmapTarget {
        BitmapTarget {
            canvas: RasterCanvas::for_page(boxes, dpi),
            fill: Rgb::BLACK,
            stroke: Rgb::BLACK,
            width: 0.5,
            path: Vec::new(),
            shapes: 0,
            ink_area_pt2: 0.0,
            cell_area_pt2: 0.0,
        }
    }

    /// Consume the target and return the buffer.
    pub fn finish(mut self) -> PreviewFrame {
        self.path.clear();
        PreviewFrame {
            page: 0,
            width: self.canvas.width,
            height: self.canvas.height,
            rgba: self.canvas.px,
            stride: 1,
            shapes: self.shapes,
            ink_area_pt2: self.ink_area_pt2,
            cell_area_pt2: self.cell_area_pt2,
        }
    }

    /// Number of shapes rasterised.
    pub fn shapes(&self) -> u64 {
        self.shapes
    }

    fn stroke_segment(&mut self, a: (f64, f64), b: (f64, f64), w: f64) {
        let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 4.0).ceil().max(1.0) as i64;
        let r = w / 2.0;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let x = a.0 + (b.0 - a.0) * t;
            let y = a.1 + (b.1 - a.1) * t;
            self.canvas.fill_polygon(
                &[(x - r, y - r), (x + r, y - r), (x + r, y + r), (x - r, y + r)],
                self.stroke,
            );
        }
    }
}

impl PageTarget for BitmapTarget {
    fn begin_page(&mut self, _index: u32, _boxes: &PageBoxes, _job: &CellWindow) {
        self.canvas.fill_all(Rgb::WHITE);
    }
    fn fill_rgb(&mut self, c: Rgb) {
        self.fill = c;
    }
    fn stroke_rgb(&mut self, c: Rgb) {
        self.stroke = c;
    }
    fn line_width_pt(&mut self, w: f64) {
        self.width = w;
    }
    fn move_to(&mut self, x: f64, y: f64) {
        self.path.clear();
        self.path.push((x, y));
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.path.push((x, y));
    }
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        let start = self.path.last().copied().unwrap_or((x1, y1));
        const N: usize = 16;
        for k in 1..=N {
            let t = k as f64 / N as f64;
            let u = 1.0 - t;
            let b = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            self.path.push((
                b[0] * start.0 + b[1] * x1 + b[2] * x2 + b[3] * x3,
                b[0] * start.1 + b[1] * y1 + b[2] * y2 + b[3] * y3,
            ));
        }
    }
    fn close_fill(&mut self) {
        if self.path.len() >= 3 {
            self.canvas.fill_polygon(&self.path, self.fill);
            self.shapes += 1;
            self.ink_area_pt2 += polygon_area(&self.path);
        }
        self.path.clear();
    }
    fn close_stroke(&mut self) {
        if self.path.len() >= 2 {
            let w = self.width.max(0.2);
            for k in 1..self.path.len() {
                let (a, b) = (self.path[k - 1], self.path[k]);
                self.stroke_segment(a, b, w);
            }
        }
        self.path.clear();
    }
    fn draw_image(&mut self, img: &EncodedImage, rect: [f64; 4]) {
        // A preview of a raster page decodes the payload back into the canvas
        // so both render modes show through the same target.
        if let Some(bytes) = decode_gray_or_rgb(img) {
            let (w, h) = (bytes.0, bytes.1);
            let sx = (rect[2] - rect[0]) * self.canvas.scales().0 / f64::from(w.max(1));
            let sy = (rect[3] - rect[1]) * self.canvas.scales().1 / f64::from(h.max(1));
            let x0 = (rect[0] * self.canvas.scales().0).round() as i64;
            let y0 = (rect[1] * self.canvas.scales().1).round() as i64;
            for y in 0..h as i64 {
                for x in 0..w as i64 {
                    let i = ((y * i64::from(w)) + x) as usize * 3;
                    let px = x0 + (x as f64 * sx).round() as i64;
                    let py = y0 + (y as f64 * sy).round() as i64;
                    if px >= 0 && py >= 0 {
                        // Reuse `fill_rect` semantics by blending one pixel.
                        let c = Rgb(bytes.2[i], bytes.2[i + 1], bytes.2[i + 2]);
                        if px < i64::from(self.canvas.width) && py < i64::from(self.canvas.height)
                        {
                            let o = (py as usize * self.canvas.width as usize + px as usize) * 4;
                            if o + 2 < self.canvas.px.len() {
                                self.canvas.px[o] = c.0;
                                self.canvas.px[o + 1] = c.1;
                                self.canvas.px[o + 2] = c.2;
                                self.canvas.px[o + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }
    fn abort_page(&mut self) {
        self.path.clear();
    }
    fn end_page(&mut self) {
        self.path.clear();
    }
}

fn decode_gray_or_rgb(img: &EncodedImage) -> Option<(u32, u32, Vec<u8>)> {
    let decoded = image::load_from_memory_with_format(&img.bytes, image::ImageFormat::Jpeg).ok()?;
    let rgb = decoded.to_rgb8();
    let (w, h) = rgb.dimensions();
    Some((w, h, rgb.into_raw()))
}

/// Shape area per cell, exposed so the GUI can show the same estimate the
/// renderer uses.
pub fn cell_area_lookup(pattern: Pattern, e: f64, line_aspect: f64) -> f64 {
    shape_area_mm2(pattern, e, line_aspect)
}

/// The preview cell budget for a job and detail level.
pub fn preview_cap(_opts: &ModernOptions, geom: &JobGeometry, detail: PreviewDetail) -> u64 {
    detail.cell_cap().min(geom.total_cells.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_detail_caps() {
        assert_eq!(PreviewDetail::Low.cell_cap(), 100_000);
        assert_eq!(PreviewDetail::Medium.cell_cap(), 250_000);
        assert_eq!(PreviewDetail::High.cell_cap(), 1_000_000);
        assert_eq!(PreviewDetail::default(), PreviewDetail::Medium);
        assert_eq!(PreviewDetail::parse_cli("HIGH"), Some(PreviewDetail::High));
        assert_eq!(PreviewDetail::parse_cli("nope"), None);
    }

    #[test]
    fn stride_caps_the_cell_budget() {
        assert_eq!(preview_stride(100_000, 250_000), 1);
        assert_eq!(preview_stride(250_000, 250_000), 1);
        assert_eq!(preview_stride(400_000, 250_000), 2);
        assert_eq!(preview_stride(1_000_000, 250_000), 2);
        assert_eq!(preview_stride(4_000_000, 250_000), 4);
        assert_eq!(preview_stride(0, 250_000), 1);
        assert_eq!(preview_stride(100_000, 0), 1);
    }

    #[test]
    fn jfif_is_stripped() {
        let mut data = vec![0xFF, 0xD8];
        data.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        data.extend_from_slice(&[7u8; 14]);
        data.extend_from_slice(&[0xFF, 0xD9]);
        let out = strip_jfif(&data);
        assert_eq!(out, vec![0xFF, 0xD8, 0xFF, 0xD9]);
        let plain = vec![0xFF, 0xD8, 0xFF, 0xDB, 1, 2];
        assert_eq!(strip_jfif(&plain), plain);
    }

    #[test]
    fn canvas_fills_and_encodes() {
        let boxes = PageBoxes {
            media: [0.0, 0.0, 200.0, 100.0],
            trim: [10.0, 10.0, 190.0, 90.0],
            bleed: [10.0, 10.0, 190.0, 90.0],
        };
        let mut c = RasterCanvas::for_page(&boxes, 72);
        assert_eq!(c.size(), (200, 100));
        c.fill_all(Rgb::WHITE);
        c.fill_rect([10.0, 10.0, 20.0, 20.0], Rgb::BLACK);
        let (w, _h) = c.size();
        let px = c.pixels().to_vec();
        let at = |x: usize, y: usize| (px[(y * w as usize + x) * 4], px[(y * w as usize + x) * 4 + 1]);
        assert_eq!(at(15, 15), (0, 0));
        assert_eq!(at(50, 50), (255, 255));
        let img = c.encode_jpeg(80).unwrap();
        assert_eq!(img.component, ImageComponent::Rgb);
        assert!(img.w > 0 && img.h > 0);
        assert!(img.bytes.len() > 100);
        assert!(img.bytes.starts_with(&[0xFF, 0xD8]));
        assert!(!img.bytes.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0]), "JFIF must be stripped");
    }

    #[test]
    fn canvas_is_capped_for_previews() {
        let boxes =
            PageBoxes { media: [0.0, 0.0, 20000.0, 10000.0], trim: [0.0; 4], bleed: [0.0; 4] };
        let c = RasterCanvas::for_page(&boxes, 300);
        let (w, h) = c.size();
        assert!(w <= MAX_PREVIEW_PX && h <= MAX_PREVIEW_PX, "{w}x{h}");
        assert_eq!(w, MAX_PREVIEW_PX);
    }

    #[test]
    fn bitmap_target_rasterises_shapes() {
        let boxes =
            PageBoxes { media: [0.0, 0.0, 200.0, 200.0], trim: [0.0; 4], bleed: [0.0; 4] };
        let job = CellWindow { col: 0, row: 0, i0: 0, i1: 1, j0: 0, j1: 1 };
        let mut t = BitmapTarget::new(&boxes, 72);
        t.begin_page(0, &boxes, &job);
        t.fill_rgb(Rgb::BLACK);
        t.move_to(80.0, 80.0);
        t.line_to(120.0, 80.0);
        t.line_to(120.0, 120.0);
        t.line_to(80.0, 120.0);
        t.close_fill();
        t.end_page();
        let f = t.finish();
        assert_eq!(f.shapes, 1);
        assert!((f.ink_area_pt2 - 1600.0).abs() < 1.0, "ink {}", f.ink_area_pt2);
        let i = 100 * 200 + 100;
        assert_eq!(f.rgba[i * 4], 0, "the square centre is inked");
        let i = 10 * 200 + 10;
        assert_eq!(f.rgba[i * 4], 255, "outside the square stays paper");
    }

    #[test]
    fn bitmap_target_rounds_circles() {
        let boxes =
            PageBoxes { media: [0.0, 0.0, 100.0, 100.0], trim: [0.0; 4], bleed: [0.0; 4] };
        let job = CellWindow { col: 0, row: 0, i0: 0, i1: 1, j0: 0, j1: 1 };
        let mut t = BitmapTarget::new(&boxes, 72);
        t.begin_page(0, &boxes, &job);
        t.fill_rgb(Rgb::BLACK);
        let (cx, cy, r) = (60.0, 50.0, 10.0);
        t.move_to(cx + r, cy);
        let k = 0.5523;
        t.curve_to(cx + r, cy + r * k, cx + r * k, cy + r, cx, cy + r);
        t.curve_to(cx - r * k, cy + r, cx - r, cy + r * k, cx - r, cy);
        t.curve_to(cx - r, cy - r * k, cx - r * k, cy - r, cx, cy - r);
        t.curve_to(cx + r * k, cy - r, cx + r, cy - r * k, cx + r, cy);
        t.close_fill();
        t.end_page();
        let f = t.finish();
        let centre = 50 * 100 + 60;
        assert_eq!(f.rgba[centre * 4], 0, "the circle centre is inked");
        let outside = 50 * 100 + 5;
        assert_eq!(f.rgba[outside * 4], 255, "outside the circle stays paper");
    }
}
