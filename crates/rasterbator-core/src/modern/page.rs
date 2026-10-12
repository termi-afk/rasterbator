//! Page composition (design §3.9).
//!
//! One page is: paper fill, the cell window plus a halo of `halo_cells` on all
//! four sides, then guides and labels. The lattice is anchored to the poster, so
//! phase is continuous across seams and duplicate drawing at a seam is
//! visually idempotent. Shapes past the media box are clipped by the page box
//! itself — no clip path and no white-out rectangles.
//!
//! Poster space has `y` down and PDF space has `y` up, so the frame conversion
//! happens once, here, and the pattern code stays in poster coordinates.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::modern::color::{
    hsl_lightness, invert, lerp_rgb, mosaic_contrast, shade, shading_factor, Rgb,
};
use crate::modern::dither::DitherState;
use crate::modern::geometry::{CellWindow, JobGeometry, PageBoxes, PT_PER_MM};
use crate::modern::options::{
    GuideStyle, InkMode, LabelStyle, ModernOptions, Pattern, RenderMode, ScreenFamily, Stagger,
};
use crate::modern::pattern::{emit_path, shape_area_mm2, PathSink};
use crate::modern::sample::CellSampler;
use crate::modern::session::{EncodedImage, ImageComponent, PageTarget, RenderError, RenderStats};
use crate::modern::strokefont;
use crate::modern::tone::{extent_mm, plain_grey, transfer, ToneParams};
use crate::modern::preview::RasterCanvas;

/// Everything one cell needs after sampling, tone and colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellInk {
    /// Tone output `t` after dithering.
    pub t: f64,
    /// Tone output before dithering, used by the dual-tone continuous blend.
    pub t_raw: f64,
    /// Shape half-extent in millimetres.
    pub e: f64,
    /// Resolved 8-bit fill colour.
    pub rgb: Rgb,
}

/// Resolve the fill colour of a cell (design §3.5).
///
/// The dual-tone decision is a hard two-level choice at `dual_softness = 1.0`.
/// Below 1.0 the emitted colour is blended from the continuous foreground ->
/// background ramp towards the hard decision, so the two-tone image gets
/// smoother as the knob turns down (spec §3.5 and amendment A-2).
pub fn cell_ink(opts: &ModernOptions, v: crate::modern::sample::CellValue, t: f64, t_raw: f64) -> Rgb {
    let mut rgb = match opts.ink {
        InkMode::Fixed => opts.ink_color,
        InkMode::Average => v.rgb(),
        InkMode::Grayscale => {
            let l = (v.level * 255.0).round().clamp(0.0, 255.0) as u8;
            Rgb(l, l, l)
        }
        InkMode::DualTone => {
            let u = t_raw.clamp(0.0, 1.0);
            let hard = if t >= 0.5 { opts.fg_color } else { opts.bg_color };
            let cont = lerp_rgb(opts.bg_color, opts.fg_color, u);
            lerp_rgb(cont, hard, opts.dual_softness.clamp(0.0, 1.0))
        }
    };
    if opts.pattern == Pattern::Mosaic {
        rgb = mosaic_contrast(rgb, opts.mosaic_contrast);
    }
    if opts.shading {
        rgb = shade(rgb, shading_factor(t, opts.shading_strength));
    }
    if opts.invert_colors {
        rgb = invert(rgb);
    }
    rgb
}

/// The tone parameters of a job.
pub fn tone_params(opts: &ModernOptions) -> ToneParams {
    ToneParams {
        min: opts.min_raster,
        max: opts.max_raster,
        curve: opts.tone_curve,
        exponent: opts.tone_exponent,
        log_k: opts.tone_log_k,
    }
}

/// Converts poster millimetres to page points for one page.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// Media box height in points.
    pub height_pt: f64,
}

impl Frame {
    /// Build the frame of a page.
    pub fn new(boxes: &PageBoxes) -> Self {
        Frame { height_pt: boxes.media[3] }
    }

    /// Poster millimetres to page points.
    pub fn pt(&self, mm: f64) -> f64 {
        mm * PT_PER_MM
    }

    /// Poster millimetres to page points, flipping `y`.
    pub fn x(&self, mm: f64) -> f64 {
        mm * PT_PER_MM
    }

    /// Poster millimetres to page points, flipping `y` about the media box.
    pub fn y(&self, mm: f64) -> f64 {
        self.height_pt - mm * PT_PER_MM
    }
}

/// A [`PathSink`] that forwards poster millimetres to a page target.
struct FrameSink<'a> {
    target: &'a mut dyn PageTarget,
    frame: Frame,
}

impl<'a> PathSink for FrameSink<'a> {
    fn move_to(&mut self, x: f64, y: f64) {
        self.target.move_to(self.frame.x(x), self.frame.y(y));
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.target.line_to(self.frame.x(x), self.frame.y(y));
    }
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        let f = self.frame;
        // The y flip mirrors the control points, which is what keeps the curve
        // the same shape in the page frame.
        self.target.curve_to(
            f.x(x1), f.y(y1), f.x(x2), f.y(y2), f.x(x3), f.y(y3),
        );
    }
    fn close_fill(&mut self) {
        self.target.close_fill();
    }
}

/// How many cells to step over between visits.
///
/// `1` for export. A larger value thins the lattice for a preview, which keeps
/// every sampled cell's centre and transfer exactly as they would be at full
/// density while bounding the preview cost.
pub type CellStride = u32;

/// Render one page.
#[allow(clippy::too_many_arguments)]
pub fn render_page(
    target: &mut dyn PageTarget,
    index: u32,
    job: &CellWindow,
    geom: &JobGeometry,
    opts: &ModernOptions,
    sampler: &CellSampler<'_>,
    dither: &mut DitherState,
    stats: &mut RenderStats,
    cancel: &AtomicBool,
) -> Result<(), RenderError> {
    render_page_strided(target, index, job, geom, opts, sampler, dither, stats, cancel, 1)
}

/// Render one page, visiting every `stride`-th cell.
#[allow(clippy::too_many_arguments)]
pub fn render_page_strided(
    target: &mut dyn PageTarget,
    index: u32,
    job: &CellWindow,
    geom: &JobGeometry,
    opts: &ModernOptions,
    sampler: &CellSampler<'_>,
    dither: &mut DitherState,
    stats: &mut RenderStats,
    cancel: &AtomicBool,
    stride: CellStride,
) -> Result<(), RenderError> {
    let stride = stride.max(1) as i64;
    let frame = Frame::new(&geom.boxes);
    let lattice = geom.lattice(opts);
    let params = tone_params(opts);
    let pitch = geom.pitch_mm;
    let fs = geom.family_scale;
    let raster = geom.render_mode == RenderMode::Raster;

    target.begin_page(index, &geom.boxes, job);

    // 2. paper fill over the bleed box.
    let mut canvas = if raster {
        Some(RasterCanvas::for_page(&geom.boxes, geom.effective_dpi))
    } else {
        None
    };
    if let Some(c) = canvas.as_mut() {
        c.fill_all(opts.paper_color);
    } else {
        target.fill_rgb(opts.paper_color);
        let b = &geom.boxes.bleed;
        target.move_to(b[0], b[1]);
        target.line_to(b[2], b[1]);
        target.line_to(b[2], b[3]);
        target.line_to(b[0], b[3]);
        target.close_fill();
    }

    let halo = i64::from(opts.halo_cells);
    let j_start = (job.j0 - halo).max(0);
    let i_start = (job.i0 - halo).max(0);
    let cell_pt2 = geom.cell_area_mm2 * PT_PER_MM * PT_PER_MM;
    let plain = opts.screen == ScreenFamily::Plain;
    let mut visited: u32 = 0;

    // 3. cells in poster row-major order (invariant I6).
    for j in (j_start..job.j1 + halo).step_by(stride as usize) {
        for i in (i_start..job.i1 + halo).step_by(stride as usize) {
            visited += 1;
            if visited % crate::modern::session::CANCEL_CHECK_CELLS == 0
                && cancel.load(Ordering::Relaxed)
            {
                return Err(RenderError::Cancelled);
            }
            let Some(box_mm) = lattice.sample_box(i, j) else {
                stats.cells_skipped += 1;
                continue;
            };
            // Cells whose sample box misses the source rectangle are skipped
            // and counted; `Contain` leaves the letterbox unpainted.
            if box_mm[2] <= geom.source_rect_mm[0]
                || box_mm[0] >= geom.source_rect_mm[2]
                || box_mm[3] <= geom.source_rect_mm[1]
                || box_mm[1] >= geom.source_rect_mm[3]
            {
                stats.cells_skipped += 1;
                continue;
            }
            let owned = job.owns(i, j);
            let v = sampler.sample(box_mm);
            let t_raw = transfer(v.level, &params, opts.invert_tone, 1.0);
            let t = dither.quantise(t_raw, i, j, owned);
            let centre = lattice.cell_centre(i, j);
            stats.page_area_pt2 += cell_pt2;

            if plain {
                let g = plain_grey(t);
                let rgb = Rgb(g, g, g);
                if let Some(c) = canvas.as_mut() {
                    c.fill_rect_box(frame, box_mm, rgb);
                } else {
                    emit_cell_fill(target, &frame, box_mm, rgb);
                }
                stats.cells_drawn += 1;
                continue;
            }

            let e = extent_mm(t, pitch, fs);
            if !(e > 0.0) {
                stats.cells_skipped += 1;
                continue;
            }
            let rgb = cell_ink(opts, v, t, t_raw);
            let area = shape_area_mm2(opts.pattern, e, opts.line_aspect);
            stats.ink_area_pt2 += area * PT_PER_MM * PT_PER_MM;
            stats.cells_drawn += 1;

            if let Some(c) = canvas.as_mut() {
                c.fill_shape(frame, opts.pattern, centre, e, t, i, j, opts.line_aspect, rgb)?;
            } else {
                target.fill_rgb(rgb);
                let mut sink = FrameSink { target: &mut *target, frame };
                emit_path(&mut sink, opts.pattern, centre, e, t, i, j, opts.line_aspect);
            }
        }
    }

    // Emit the rasterised page as a single image, then draw guides and labels
    // on top as vectors so they stay crisp.
    if let Some(c) = canvas.as_ref() {
        let img = c.encode_jpeg(geom.effective_quality)?;
        let m = geom.boxes.media;
        target.draw_image(&img, m);
    }

    // 4. guides
    draw_guides(target, geom, opts);
    // 5. labels
    if opts.labels == LabelStyle::Coordinates {
        draw_labels(target, geom, opts, job);
    }
    // 6.
    target.end_page();
    Ok(())
}

fn emit_cell_fill(target: &mut dyn PageTarget, frame: &Frame, box_mm: [f64; 4], rgb: Rgb) {
    target.fill_rgb(rgb);
    target.move_to(frame.x(box_mm[0]), frame.y(box_mm[1]));
    target.line_to(frame.x(box_mm[2]), frame.y(box_mm[1]));
    target.line_to(frame.x(box_mm[2]), frame.y(box_mm[3]));
    target.line_to(frame.x(box_mm[0]), frame.y(box_mm[3]));
    target.close_fill();
}

/// Stroke the trim box as a border, or eight corner arms.
pub fn draw_guides(target: &mut dyn PageTarget, geom: &JobGeometry, opts: &ModernOptions) {
    if opts.guides == GuideStyle::None {
        return;
    }
    target.stroke_rgb(opts.guide_color);
    target.line_width_pt(opts.guide_width_pt);
    let t = geom.boxes.trim;
    match opts.guides {
        GuideStyle::None => {}
        GuideStyle::Border => {
            target.move_to(t[0], t[1]);
            target.line_to(t[2], t[1]);
            target.line_to(t[2], t[3]);
            target.line_to(t[0], t[3]);
            target.close_stroke();
        }
        GuideStyle::Corners => {
            let c = opts.corner_mark_mm * PT_PER_MM;
            for sx in [-1.0f64, 1.0] {
                for sy in [-1.0f64, 1.0] {
                    // Horizontal arm on the trim edge, reaching outwards.
                    let y = if sy < 0.0 { t[1] } else { t[3] };
                    let x = if sx < 0.0 { t[0] } else { t[2] };
                    target.move_to(x, y);
                    target.line_to(x - sx * c, y);
                    target.close_stroke();
                    // Vertical arm on the trim edge, reaching outwards.
                    let x = if sx < 0.0 { t[0] } else { t[2] };
                    let y = if sy < 0.0 { t[1] } else { t[3] };
                    target.move_to(x, y);
                    target.line_to(x, y - sy * c);
                    target.close_stroke();
                }
            }
        }
    }
}

/// Draw the column letter, the row number and the sheet-(0,0) legend.
pub fn draw_labels(
    target: &mut dyn PageTarget,
    geom: &JobGeometry,
    opts: &ModernOptions,
    job: &CellWindow,
) {
    let size = opts.label_size_pt;
    let weight = strokefont::stroke_weight(size) * (opts.guide_width_pt / 0.5);
    let inset = opts.label_offset_mm * PT_PER_MM;
    let x_left = geom.boxes.trim[0] + inset;
    let y_top = geom.boxes.media[3] - inset - size;
    let col = strokefont::col_label(job.col + 1);
    let row = (job.row + 1).to_string();
    // Column letter on the left, vertically centred on the retained tile.
    let y_col = (geom.boxes.trim[1] + geom.boxes.trim[3]) / 2.0 - size / 2.0;
    strokefont::emit_text(target, &col, x_left, y_col, size, weight, opts.guide_color);
    // Row number on the top, horizontally centred on the retained tile.
    let w_row = strokefont::text_width(&row, size);
    let x_row = (geom.boxes.trim[0] + geom.boxes.trim[2]) / 2.0 - w_row / 2.0;
    strokefont::emit_text(target, &row, x_row, y_top, size, weight, opts.guide_color);
    // Legend on sheet (0, 0) only.
    if job.col == 0 && job.row == 0 {
        let legend = format!("R{}{}", job.col + 1, job.row + 1);
        let y = geom.boxes.trim[1] - inset * 0.25 - size;
        strokefont::emit_text(target, &legend, x_left, y, size, weight, opts.guide_color);
    }
}

/// The paper colour of a page fill.
pub fn page_fill(opts: &ModernOptions) -> Rgb {
    opts.paper_color
}

/// The grey of a plain-screen cell, exposed for tests.
pub fn plain_cell_grey(level: f64, opts: &ModernOptions) -> u8 {
    let t = transfer(level, &tone_params(opts), opts.invert_tone, 1.0);
    plain_grey(t)
}

/// The composited level of an 8-bit colour, exposed for tests.
pub fn level_of_rgb(c: Rgb) -> f64 {
    hsl_lightness(c)
}

/// Whether the job uses a staggered lattice.
pub fn uses_stagger(opts: &ModernOptions) -> bool {
    opts.screen == ScreenFamily::Lattice && opts.stagger == Stagger::HalfPitch
}

/// Build a stand-alone [`EncodedImage`] payload for tests.
pub fn gray_image(w: u32, h: u32, v: u8) -> EncodedImage {
    EncodedImage { w, h, component: ImageComponent::Gray, bytes: vec![v; (w * h) as usize] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::color::RgbaF64;
    use crate::modern::geometry::JobGeometry;
    use crate::modern::options::{ModernOptions, SizeSpec};
    use crate::modern::probe::CountingTarget;
    use crate::modern::sample::CellSampler;
    use crate::modern::source::{PreparedImage, SourceInfo};

    fn grey(level: f64) -> crate::modern::sample::CellValue {
        crate::modern::sample::CellValue {
            rgba: RgbaF64::rgb(level, level, level),
            level,
            inside: true,
        }
    }

    #[test]
    fn fixed_average_and_grayscale_ink() {
        let mut o = ModernOptions::default();
        o.ink_color = Rgb(1, 2, 3);
        assert_eq!(cell_ink(&o, grey(0.5), 1.0, 1.0), Rgb(1, 2, 3));
        o.ink = InkMode::Average;
        assert_eq!(cell_ink(&o, grey(0.5), 1.0, 1.0), Rgb(128, 128, 128));
        o.ink = InkMode::Grayscale;
        assert_eq!(cell_ink(&o, grey(0.5), 1.0, 1.0), Rgb(128, 128, 128));
        o.ink = InkMode::Average;
        assert_eq!(cell_ink(&o, grey(0.25), 1.0, 1.0), Rgb(64, 64, 64));
    }

    #[test]
    fn dual_tone_is_a_hard_two_level_decision_at_softness_one() {
        let mut o = ModernOptions::default();
        o.ink = InkMode::DualTone;
        o.fg_color = Rgb::BLACK;
        o.bg_color = Rgb::WHITE;
        o.dither_strength = 0.0;
        o.dual_softness = 1.0;
        assert_eq!(cell_ink(&o, grey(0.0), 0.49, 0.49), Rgb::WHITE);
        assert_eq!(cell_ink(&o, grey(0.0), 0.51, 0.51), Rgb::BLACK);
        assert_eq!(cell_ink(&o, grey(0.0), 0.0, 0.0), Rgb::WHITE);
        assert_eq!(cell_ink(&o, grey(0.0), 1.0, 1.0), Rgb::BLACK);
    }

    #[test]
    fn dual_tone_softness_moves_towards_the_continuous_ramp() {
        let mut o = ModernOptions::default();
        o.ink = InkMode::DualTone;
        o.fg_color = Rgb::BLACK;
        o.bg_color = Rgb::WHITE;
        o.dither_strength = 0.0;
        // At the decision boundary the hard decision and the continuous ramp
        // agree, so softness makes no difference there.
        o.dual_softness = 1.0;
        assert_eq!(cell_ink(&o, grey(0.0), 0.4999, 0.4999), Rgb::WHITE);
        assert_eq!(cell_ink(&o, grey(0.0), 0.5, 0.5), Rgb::BLACK, "t >= 0.5 is foreground");        // Away from it, softness pulls the hard choice back towards the ramp.
        let hard = cell_ink(&o, grey(0.0), 0.6, 0.6);
        assert_eq!(hard, Rgb::BLACK);
        o.dual_softness = 0.5;
        let soft = cell_ink(&o, grey(0.0), 0.6, 0.6);
        assert_eq!(soft, Rgb(51, 51, 51), "half way between the ramp and the hard choice");
        o.dual_softness = 0.05;
        let flat = cell_ink(&o, grey(0.0), 0.6, 0.6);
        assert!(flat.0 > soft.0, "softer means closer to the continuous ramp");
        assert!(flat.0 > hard.0, "softer means further from the hard black");
        o.dual_softness = 1.0;
        assert_eq!(cell_ink(&o, grey(0.0), 0.4, 0.4), Rgb::WHITE);
    }
    #[test]
    fn shading_and_invert_wrap_every_ink_mode() {
        let mut o = ModernOptions::default();
        o.ink_color = Rgb::WHITE;
        o.shading = true;
        o.shading_strength = 0.35;
        assert_eq!(cell_ink(&o, grey(0.0), 1.0, 1.0), Rgb::WHITE);
        assert_eq!(cell_ink(&o, grey(0.0), 0.0, 0.0), Rgb(166, 166, 166));
        o.shading = false;
        o.invert_colors = true;
        assert_eq!(cell_ink(&o, grey(0.0), 1.0, 1.0), Rgb::BLACK);
        assert_eq!(cell_ink(&o, grey(0.0), 0.0, 0.0), Rgb::BLACK, "shading off: inversion only");
    }
    #[test]
    fn mosaic_contrast_applies_to_the_colour_only() {
        let mut o = ModernOptions::default();
        o.ink = InkMode::Average;
        o.pattern = Pattern::Mosaic;
        o.mosaic_contrast = 2.0;
        assert_eq!(cell_ink(&o, grey(128.0 / 255.0), 1.0, 1.0), Rgb(129, 129, 129));
        o.mosaic_contrast = 1.0;
        assert_eq!(cell_ink(&o, grey(128.0 / 255.0), 1.0, 1.0), Rgb(128, 128, 128));
    }

    #[test]
    fn a_vector_page_emits_shapes_and_no_images() {
        let mut o = ModernOptions::default();
        o.size_spec = SizeSpec::WidthSheets(1.0);
        o.nominal_dot_mm = 20.0;
        o.page_warn = 0;
        let src = SourceInfo::synthetic(200, 200);
        let g = JobGeometry::solve(&o, &src).unwrap();
        let img = PreparedImage::solid(200, 200, Rgb(0, 0, 0));
        let sampler = CellSampler::new(&img, Rgb::WHITE, o.sampling, g.source_rect_mm);
        let mut dither = DitherState::new(g.cols_total, o.dither, o.dither_levels, o.dither_block, o.dither_strength);
        let mut stats = RenderStats::default();
        let cancel = AtomicBool::new(false);
        let mut t = CountingTarget::default();
        for p in 0..g.total_pages {
            let w = *g.cell_window(p);
            render_page(&mut t, p, &w, &g, &o, &sampler, &mut dither, &mut stats, &cancel).unwrap();
        }
        assert_eq!(t.pages, g.total_pages);
        assert_eq!(t.images, 0, "the vector path embeds no image");
        assert!(t.fills > 0);
        assert_eq!(t.fill_colors[0], Rgb::WHITE, "the paper fill comes first");
    }

    #[test]
    fn a_raster_page_embeds_exactly_one_image() {
        let mut o = ModernOptions::default();
        o.size_spec = SizeSpec::WidthSheets(1.0);
        o.nominal_dot_mm = 30.0;
        o.render = RenderMode::Raster;
        o.page_warn = 0;
        let src = SourceInfo::synthetic(200, 200);
        let g = JobGeometry::solve(&o, &src).unwrap();
        let img = PreparedImage::solid(200, 200, Rgb(0, 0, 0));
        let sampler = CellSampler::new(&img, Rgb::WHITE, o.sampling, g.source_rect_mm);
        let mut dither = DitherState::new(g.cols_total, o.dither, o.dither_levels, o.dither_block, o.dither_strength);
        let mut stats = RenderStats::default();
        let cancel = AtomicBool::new(false);
        let mut t = CountingTarget::default();
        for p in 0..g.total_pages {
            let w = *g.cell_window(p);
            render_page(&mut t, p, &w, &g, &o, &sampler, &mut dither, &mut stats, &cancel).unwrap();
        }
        assert_eq!(t.images, u64::from(g.total_pages));
        assert_eq!(t.fills, 0, "the raster path emits the page as one image");
    }

    #[test]
    fn halo_cells_are_drawn_but_not_owned() {
        let mut o = ModernOptions::default();
        o.size_spec = SizeSpec::Size { w: 570.0, h: 277.0 };
        o.nominal_dot_mm = 20.0;
        o.halo_cells = 2;
        o.page_warn = 0;
        o.page_limit = 0;
        let src = SourceInfo::synthetic(400, 200);
        let g = JobGeometry::solve(&o, &src).unwrap();
        let img = PreparedImage::solid(400, 200, Rgb(0, 0, 0));
        let sampler = CellSampler::new(&img, Rgb::WHITE, o.sampling, g.source_rect_mm);
        let mut d = DitherState::new(
            g.cols_total,
            crate::modern::options::Dither::None,
            o.dither_levels,
            o.dither_block,
            o.dither_strength,
        );
        let mut stats = RenderStats::default();
        let cancel = AtomicBool::new(false);
        let mut t = CountingTarget::default();
        for p in 0..g.total_pages {
            let w = *g.cell_window(p);
            render_page(&mut t, p, &w, &g, &o, &sampler, &mut d, &mut stats, &cancel).unwrap();
        }
        // Halo adds cells beyond the owned window on the interior pages.
        assert!(t.fills > g.total_cells);
    }

    #[test]
    fn the_plain_screen_emits_the_tone_plane() {
        let mut o = ModernOptions::default();
        o.screen = ScreenFamily::Plain;
        o.min_raster = 0.2;
        // White (level 1) -> t = 0.2 -> grey 204.
        assert_eq!(plain_cell_grey(1.0, &o), 204);
        // Black -> t = max_raster = 1 -> grey 0.
        assert_eq!(plain_cell_grey(0.0, &o), 0);
    }

    #[test]
    fn frame_flips_y_about_the_media_box() {
        let boxes = PageBoxes { media: [0.0, 0.0, 600.0, 800.0], trim: [0.0; 4], bleed: [0.0; 4] };
        let f = Frame::new(&boxes);
        assert_eq!(f.y(0.0), 800.0);
        assert!((f.y(10.0) - (800.0 - 10.0 * PT_PER_MM)).abs() < 1e-12);
        assert!((f.x(10.0) - 10.0 * PT_PER_MM).abs() < 1e-12);
        assert!((f.pt(1.0) - PT_PER_MM).abs() < 1e-12);
    }

    #[test]
    fn level_of_rgb_matches_the_colour_module() {
        assert_eq!(level_of_rgb(Rgb(255, 0, 0)), 0.5);
        assert!((level_of_rgb(Rgb(10, 200, 90)) - 105.0 / 255.0).abs() < 1e-12);
    }
}