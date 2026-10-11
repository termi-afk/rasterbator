//! The rasterbation engine: a faithful port of `Rasterbator.CreateImage` /
//! `RasterbatePage` from the original binary (2007 build), including its
//! 144 dpi fixed resolution, unit handling and float arithmetic.

use image::RgbaImage;

use crate::canvas::Canvas;

/// Fixed working resolution of the original engine (pixels per inch).
const RESOLUTION: f64 = 144.0;

/// Dot color mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// All dots in one fixed color.
    Fixed { r: u8, g: u8, b: u8 },
    /// Each dot takes the average color of the source area it covers.
    Average,
}

impl ColorMode {
    /// Parse `black`, `avg` or `RRGGBB` (hex, as used by the original UI).
    pub fn parse(s: &str) -> Result<ColorMode, String> {
        match s {
            "black" => Ok(ColorMode::Fixed { r: 0, g: 0, b: 0 }),
            "avg" => Ok(ColorMode::Average),
            hex if hex.len() == 6 => {
                let parse = |i: usize| {
                    u8::from_str_radix(&hex[i..i + 2], 16)
                        .map_err(|_| format!("invalid color '{hex}'"))
                };
                Ok(ColorMode::Fixed {
                    r: parse(0)?,
                    g: parse(2)?,
                    b: parse(4)?,
                })
            }
            other => Err(format!("invalid color '{other}': use black, avg or RRGGBB")),
        }
    }
}

/// Rectangle of the source image to use, in source pixels.
#[derive(Clone, Copy, Debug)]
pub struct CropRect {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// Engine parameters, corresponding to the attribute collection the original
/// UI hands to the engine. Sizes are in millimeters.
#[derive(Clone, Copy, Debug)]
pub struct EngineOptions {
    /// Physical paper width of one sheet (mm).
    pub paper_width_mm: f64,
    /// Physical paper height of one sheet (mm).
    pub paper_height_mm: f64,
    /// Width of the whole poster (mm). Truncated to whole mm like the UI does.
    pub image_width_mm: f64,
    /// Height of the whole poster (mm). Truncated to whole mm like the UI does.
    pub image_height_mm: f64,
    /// Maximum dot size (mm) at full black.
    pub dot_size_mm: f64,
    pub color: ColorMode,
    /// Draw the cutting guide rectangle around the rasterbated area.
    pub cutout: bool,
    /// Source crop, defaults to the whole image.
    pub crop: Option<CropRect>,
}

/// Values computed once per job, exposed for callers (UI, CLI summaries).
#[derive(Clone, Copy, Debug)]
pub struct EngineSummary {
    pub pages_x: u32,
    pub pages_y: u32,
    pub total_pages: u32,
    pub squares_x: u32,
    pub squares_y: u32,
    /// Page size in PDF points, as written to the PDF page box.
    pub page_width_pt: f32,
    pub page_height_pt: f32,
}

/// Rasterbate `image` onto `canvas` according to `opts`.
///
/// `on_page` is called after each finished page with `(pages_done, total_pages)`.
pub fn rasterbate(
    image: &RgbaImage,
    opts: &EngineOptions,
    canvas: &mut dyn Canvas,
    on_page: &mut dyn FnMut(u32, u32),
) -> EngineSummary {
    // --- setup, mirroring Rasterbator.CreateImage ---
    // The UI truncates the computed poster size to whole millimeters before
    // passing it to the engine; paper sizes are whole millimeters, too.
    let image_w_mm = opts.image_width_mm.trunc() as i64;
    let image_h_mm = opts.image_height_mm.trunc() as i64;
    let paper_w_mm = opts.paper_width_mm.trunc() as i64;
    let paper_h_mm = opts.paper_height_mm.trunc() as i64;

    let image_w_px = (image_w_mm as f64 / 25.4 * RESOLUTION) as i32;
    let image_h_px = (image_h_mm as f64 / 25.4 * RESOLUTION) as i32;
    let paper_w_px = (paper_w_mm as f64 / 25.4 * RESOLUTION) as i32;
    let paper_h_px = (paper_h_mm as f64 / 25.4 * RESOLUTION) as i32;

    // MaxRadius = dot_size_mm / 25.4f * 144f / 2f  (float32 arithmetic)
    let dot_size = opts.dot_size_mm as f32;
    let max_radius: f32 = dot_size / 25.4f32 * RESOLUTION as f32 / 2.0f32;

    // Crop rectangle (already in source pixels; the UI passes the whole image).
    let crop = opts.crop.unwrap_or(CropRect {
        x1: 0.0,
        y1: 0.0,
        x2: image.width() as f64,
        y2: image.height() as f64,
    });
    let x1 = crop.x1.trunc() as i32;
    let y1 = crop.y1.trunc() as i32;
    let x2 = crop.x2.trunc() as i32;
    let y2 = crop.y2.trunc() as i32;
    let area_w = x2 - x1;
    let area_h = y2 - y1;

    let pages_x = (image_w_mm as f64 / paper_w_mm as f64).ceil() as i32;
    let pages_y = (image_h_mm as f64 / paper_h_mm as f64).ceil() as i32;

    // SquareSize = (float)(2f * (MaxRadius - 1f)) / sqrt(2)   (double division)
    let square_size: f32 =
        ((2.0f32 * (max_radius - 1.0f32)) as f64 / 2.0f64.sqrt()) as f32;

    let squares_x = (paper_w_px as f32 / square_size) as i32;
    let squares_y = (paper_h_px as f32 / square_size) as i32;

    // Paper size in points: 72f * mm / 25.4f  (float32 arithmetic)
    let (psx, psy) = crate::size::paper_size_pt(paper_w_mm as f64, paper_h_mm as f64);

    let total_pages = (pages_x * pages_y).max(0) as u32;
    let summary = EngineSummary {
        pages_x: pages_x.max(0) as u32,
        pages_y: pages_y.max(0) as u32,
        total_pages,
        squares_x: squares_x.max(0) as u32,
        squares_y: squares_y.max(0) as u32,
        page_width_pt: psx,
        page_height_pt: psy,
    };

    // --- page loop, mirroring CreateImage's double loop ---
    let mut pages_done = 0u32;
    for py in 0..pages_y {
        for px in 0..pages_x {
            rasterbate_page(
                image, opts, canvas, px, py, x1, y1, x2, y2, area_w, area_h, image_w_px,
                image_h_px, paper_w_px, paper_h_px, squares_x, squares_y, square_size,
                max_radius, psx, psy, pages_done as usize,
            );
            pages_done += 1;
            on_page(pages_done, total_pages);
        }
    }

    summary
}

/// One output page, mirroring `Rasterbator.RasterbatePage`.
#[allow(clippy::too_many_arguments)]
fn rasterbate_page(
    image: &RgbaImage,
    opts: &EngineOptions,
    canvas: &mut dyn Canvas,
    px: i32,
    py: i32,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    area_w: i32,
    area_h: i32,
    image_w_px: i32,
    image_h_px: i32,
    paper_w_px: i32,
    paper_h_px: i32,
    squares_x: i32,
    squares_y: i32,
    square_size: f32,
    max_radius: f32,
    psx: f32,
    psy: f32,
    page_index: usize,
) {
    canvas.begin_page(page_index);

    let (fixed_r, fixed_g, fixed_b) = match opts.color {
        ColorMode::Fixed { r, g, b } => (r, g, b),
        ColorMode::Average => (0, 0, 0),
    };
    let use_average = opts.color == ColorMode::Average;
    if !use_average {
        canvas.set_fill_rgb(fixed_r, fixed_g, fixed_b);
    }

    // Tracks the extent of the drawn dots for the bleed cleanup rectangles.
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    let page_off_x = paper_w_px * px;
    let page_off_y = paper_h_px * py;
    let scale_x = area_w as f32 / image_w_px as f32;
    let scale_y = area_h as f32 / image_h_px as f32;

    // From -1 to +1 beyond the grid: large dots of neighbouring pages may
    // partially show on this page as well.
    for sy in -1..squares_y + 1 {
        let sq_y = (page_off_y as f32 + sy as f32 * square_size) as i32;
        let src_y1 = (y1 as f32 + sq_y as f32 * scale_y) as i32;
        if src_y1 >= y2 {
            break;
        }
        let src_y2 = (y1 as f32 + (sq_y as f32 + square_size) * scale_y) as i32;

        for sx in -1..squares_x + 1 {
            let sq_x = (page_off_x as f32 + sx as f32 * square_size) as i32;
            let src_x1 = (x1 as f32 + sq_x as f32 * scale_x) as i32;
            if src_x1 >= x2 {
                break;
            }
            let src_x2 = (x1 as f32 + (sq_x as f32 + square_size) * scale_x) as i32;

            // Average brightness of the source area covered by this square.
            // Out-of-image pixels count as white (brightness 1).
            let mut brightness_sum = 0.0f32;
            let mut count = 0.0f32;
            let mut avg_count = 0.0f32;
            let mut r_sum = 0.0f32;
            let mut g_sum = 0.0f32;
            let mut b_sum = 0.0f32;

            for by in src_y1..=src_y2 {
                for bx in src_x1..=src_x2 {
                    count += 1.0;
                    if bx >= image.width() as i32
                        || by >= image.height() as i32
                        || bx > x2
                        || by > y2
                        || bx < 0
                        || by < 0
                    {
                        brightness_sum += 1.0;
                        continue;
                    }
                    let p = image.get_pixel(bx as u32, by as u32);
                    brightness_sum += get_brightness(p[0], p[1], p[2]);
                    if use_average {
                        avg_count += 1.0;
                        r_sum += p[0] as f32;
                        g_sum += p[1] as f32;
                        b_sum += p[2] as f32;
                    }
                }
            }

            // Radius of the dot: darker area -> larger dot.
            let radius = (1.0f32 - brightness_sum / count) * max_radius;

            // Circle centers are placed at square centers; the square grid is
            // in 144 dpi pixels while the drawn coordinates are PDF points
            // (72 dpi), hence the /2.
            let ex = (sx as f32 + 0.5f32) * square_size / 2.0f32;
            let ey = (sy as f32 + 0.5f32) * square_size / 2.0f32;

            if use_average {
                let (r, g, b) = (
                    float_to_int_clamped(r_sum / avg_count) & 0xFF,
                    float_to_int_clamped(g_sum / avg_count) & 0xFF,
                    float_to_int_clamped(b_sum / avg_count) & 0xFF,
                );
                canvas.set_fill_rgb(r as u8, g as u8, b as u8);
            }

            canvas.circle(ex, psy - ey, radius / 2.0f32);

            let ex = ex + square_size / 4.0f32;
            let ey = ey + square_size / 4.0f32;
            if ex > max_x {
                max_x = ex;
            }
            if ey > max_y {
                max_y = ey;
            }
            canvas.fill();
        }
    }

    // Extent of the rasterbated area on this page, used to trim the bleed.
    let mut rx = squares_x as f32 * square_size / 2.0f32;
    let mut ry = squares_y as f32 * square_size / 2.0f32;
    if max_x < rx {
        rx = max_x;
    }
    if max_y < ry {
        ry = max_y;
    }

    // Remove bleed: white rectangles over the margins where neighbouring
    // pages' dots bled in.
    canvas.set_fill_rgb(255, 255, 255);
    canvas.rectangle(-square_size, 0.0, psx + square_size, -square_size); // top
    canvas.fill();
    canvas.rectangle(-square_size, -square_size, 0.0, square_size); // left
    canvas.fill();
    canvas.rectangle(rx, -square_size, square_size, psy + square_size); // right
    canvas.fill();
    canvas.rectangle(-square_size, psy - ry, psx + square_size, -square_size); // bottom
    canvas.fill();

    if opts.cutout {
        canvas.set_line_width(0.5f32);
        canvas.move_to(0.0, psy);
        canvas.set_stroke_rgb(0xBB, 0xBB, 0xBB);
        canvas.line_to(rx, psy);
        canvas.line_to(rx, psy - ry);
        canvas.line_to(0.0, psy - ry);
        canvas.line_to(0.0, psy);
        canvas.stroke();
    }

    canvas.end_page();
}

/// `System.Drawing.Color.GetBrightness` (HSL lightness in 0..=1).
#[inline]
fn get_brightness(r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32 / 255.0f32;
    let gf = g as f32 / 255.0f32;
    let bf = b as f32 / 255.0f32;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    (max + min) / 2.0f32
}

/// C# `(int)float` conversion semantics on x86/x64: truncation toward zero,
/// NaN and out-of-range values become `i32::MIN`.
#[inline]
fn float_to_int_clamped(v: f32) -> i32 {
    if !v.is_finite() || v < i32::MIN as f32 || v > i32::MAX as f32 {
        i32::MIN
    } else {
        v as i32
    }
}
