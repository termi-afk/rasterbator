//! The canonical geometry of a job (design §2.3).
//!
//! [`JobGeometry::solve`] is the **only** layout computation in the crate: the
//! CLI, both GUI previews and the PDF exporter all call it with the same
//! `(options, dimensions)` and therefore get the same answer (invariant I2).
//! It is pure: no filesystem access, no pixel access, only the source
//! dimensions.
//!
//! The cell windows are disjoint and contiguous (`i1(col) == i0(col+1)`),
//! so page iteration order equals poster row-major order (invariant I6), which
//! is what makes the dither row buffer and the halo correct without extra
//! bookkeeping.

use crate::modern::color::Rgb;
use crate::modern::diagnostics::{DiagCode, Diagnostic, ValidationReport};
use crate::modern::options::{
    Align, Coverage, Dither, Fit, LabelStyle, ModernOptions, Orientation, OverlapMode, Pattern,
    RenderMode, ScreenFamily, SizeSpec,
};
use crate::modern::pattern::shape_area_mm2;
use crate::modern::screen::{cell_height, family_scale};
use crate::modern::source::SourceInfo;
use crate::modern::tone::extent_mm;

/// PDF points per millimetre.
pub const PT_PER_MM: f64 = 72.0 / 25.4;

/// A size in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeMm {
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl SizeMm {
    /// Build a size.
    pub const fn new(w: f64, h: f64) -> Self {
        SizeMm { w, h }
    }

    /// The `min` of the two axes.
    pub fn min_axis(self) -> f64 {
        self.w.min(self.h)
    }
}

/// The three PDF page boxes, in points, `[x0, y0, x1, y1]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PageBoxes {
    /// The physical sheet.
    pub media: [f64; 4],
    /// The retained tile after cutting the margin away.
    pub trim: [f64; 4],
    /// The trim grown by the bleed, clamped to the media box.
    pub bleed: [f64; 4],
}

impl PageBoxes {
    /// Width of a `[x0, y0, x1, y1]` box.
    pub fn width(b: &[f64; 4]) -> f64 {
        b[2] - b[0]
    }

    /// Height of a `[x0, y0, x1, y1]` box.
    pub fn height(b: &[f64; 4]) -> f64 {
        b[3] - b[1]
    }
}

/// The cells a page owns, in poster cell indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellWindow {
    /// Column index (0-based, left to right).
    pub col: u32,
    /// Row index (0-based, top to bottom).
    pub row: u32,
    /// First owned column, inclusive.
    pub i0: i64,
    /// Last owned column, exclusive.
    pub i1: i64,
    /// First owned row, inclusive.
    pub j0: i64,
    /// Last owned row, exclusive.
    pub j1: i64,
}

impl CellWindow {
    /// Number of owned cells.
    pub fn cell_count(&self) -> u64 {
        let w = (self.i1 - self.i0).max(0) as u64;
        let h = (self.j1 - self.j0).max(0) as u64;
        w * h
    }

    /// True when the cell `(i, j)` is owned (a halo cell is not).
    pub fn owns(&self, i: i64, j: i64) -> bool {
        i >= self.i0 && i < self.i1 && j >= self.j0 && j < self.j1
    }
}

/// The effective source-resolution label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DpiLabel {
    /// Below 120 effective dpi.
    Low,
    /// 120 up to 200 effective dpi.
    Fair,
    /// 200 effective dpi or better.
    Good,
}

impl DpiLabel {
    /// The label text shown in the GUI.
    pub fn text(self) -> &'static str {
        match self {
            DpiLabel::Low => "Low",
            DpiLabel::Fair => "Fair",
            DpiLabel::Good => "Good",
        }
    }

    /// Classify an effective resolution.
    pub fn of(dpi: f64) -> DpiLabel {
        if dpi >= 200.0 {
            DpiLabel::Good
        } else if dpi >= 120.0 {
            DpiLabel::Fair
        } else {
            DpiLabel::Low
        }
    }
}

/// Everything the renderer needs to know about a job's layout.
#[derive(Clone, Debug)]
pub struct JobGeometry {
    /// Physical sheet size, orientation applied.
    pub sheet: SizeMm,
    /// Retained tile per sheet.
    pub visible: SizeMm,
    /// Distance between successive sheet origins.
    pub step: SizeMm,
    /// Assembled poster size.
    pub poster: SizeMm,
    /// Size actually covered by the page grid (may exceed the poster).
    pub cover: SizeMm,
    /// Effective cutting margin in millimetres.
    pub margin_mm: f64,
    /// Effective overlap in millimetres.
    pub overlap_mm: f64,
    /// Which overlap semantics produced [`JobGeometry::step`].
    pub overlap_mode: OverlapMode,
    /// Pages across.
    pub pages_x: u32,
    /// Pages down.
    pub pages_y: u32,
    /// `pages_x * pages_y`.
    pub total_pages: u32,
    /// Grid pitch in millimetres, exactly the nominal dot size.
    pub pitch_mm: f64,
    /// Vertical cell pitch in millimetres.
    pub cell_h_mm: f64,
    /// Columns covering the cover width.
    pub cols_total: u32,
    /// Rows covering the cover height.
    pub rows_total: u32,
    /// `total_pages`-sized cell windows, in row-major page order.
    pub windows: Vec<CellWindow>,
    /// The image rectangle in poster millimetres, `[x0, y0, x1, y1]`.
    pub source_rect_mm: [f64; 4],
    /// Page boxes in points.
    pub boxes: PageBoxes,
    /// The render path after the `Auto` policy resolved.
    pub render_mode: RenderMode,
    /// Effective raster resolution.
    pub effective_dpi: u16,
    /// Effective JPEG quality.
    pub effective_quality: u8,
    /// Coverage gain for the active (family, coverage) pair.
    pub family_scale: f64,
    /// Area of one cell in square millimetres.
    pub cell_area_mm2: f64,
    /// Area of every cell in square millimetres.
    pub cell_area_total_mm2: f64,
    /// Total number of cells on the cover.
    pub total_cells: u64,
    /// Ink area of an all-black poster, in square millimetres.
    pub ink_area_total_mm2: f64,
    /// Effective source resolution across the poster, in dpi.
    pub source_effective_dpi: f64,
    /// Display size of the source after orientation and rotation.
    pub source_display_px: (u32, u32),
}

impl JobGeometry {
    /// Solve the canonical geometry of a job.
    ///
    /// Returns the full validation report as the error when anything is
    /// blocking, so the CLI can print every message at once.
    pub fn solve(opts: &ModernOptions, src: &SourceInfo) -> Result<JobGeometry, ValidationReport> {
        let geom = match Self::compute(opts, src) {
            Ok(g) => g,
            Err(d) => return Err(ValidationReport::from_items(vec![d])),
        };
        let report = ValidationReport::from_items(geometry_diagnostics(opts, &geom));
        if report.has_blocking() {
            return Err(report);
        }
        Ok(geom)
    }

    /// The window of one page.
    pub fn cell_window(&self, page: u32) -> &CellWindow {
        &self.windows[page as usize]
    }

    /// The source-resolution label shown in the GUI.
    pub fn dpi_label(&self) -> DpiLabel {
        DpiLabel::of(self.source_effective_dpi)
    }

    /// The lattice of this job, anchored to the cover rectangle.
    pub fn lattice(&self, opts: &ModernOptions) -> crate::modern::screen::ScreenLattice {
        crate::modern::screen::ScreenLattice::new(
            opts.screen,
            self.pitch_mm,
            opts.stagger,
            opts.grid_phase_mm,
            (self.cover.w, self.cover.h),
        )
    }

    /// Posters with a fractional last page report the surplus in millimetres.
    pub fn partial_edge_mm(&self) -> (f64, f64) {
        (
            (self.cover.w - self.poster.w).max(0.0),
            (self.cover.h - self.poster.h).max(0.0),
        )
    }

    fn compute(opts: &ModernOptions, src: &SourceInfo) -> Result<JobGeometry, Diagnostic> {
        let mut sheet = SizeMm::new(opts.paper.w_mm, opts.paper.h_mm);
        if opts.orientation == Orientation::Landscape {
            sheet = SizeMm::new(sheet.h, sheet.w);
        }
        if !(50.0..=1000.0).contains(&sheet.w) || !(50.0..=1000.0).contains(&sheet.h) {
            return Err(Diagnostic::error(
                DiagCode::RangeOutOfBounds,
                format!("paper {}x{} mm is outside the 50..=1000 mm range", sheet.w, sheet.h),
            ));
        }
        let margin_cap = sheet.min_axis() / 2.0 - 10.0;
        let margin = opts.margin_mm.clamp(2.0, margin_cap.max(2.0));
        let visible = SizeMm::new(sheet.w - 2.0 * margin, sheet.h - 2.0 * margin);
        if visible.w < 10.0 || visible.h < 10.0 {
            return Err(Diagnostic::error(
                DiagCode::PrintableTooSmall,
                format!(
                    "the printable area is {:.1}x{:.1} mm after a {:.1} mm margin; at least 10x10 mm is required",
                    visible.w, visible.h, margin
                ),
            )
            .with_hint("reduce --margin-mm or pick a larger paper"));
        }

        // Overlap semantics (decision D2 / amendment A-1).
        let step = match opts.overlap_mode {
            OverlapMode::Margin => SizeMm::new(visible.w + opts.overlap_mm, visible.h + opts.overlap_mm),
            OverlapMode::Shared => SizeMm::new(visible.w - opts.overlap_mm, visible.h - opts.overlap_mm),
        };
        if step.w <= 0.0 || step.h <= 0.0 {
            return Err(Diagnostic::error(
                DiagCode::OverlapExceedsVisible,
                format!(
                    "overlap {:.2} mm leaves a zero step (visible {:.1}x{:.1} mm)",
                    opts.overlap_mm, visible.w, visible.h
                ),
            )
            .with_hint("reduce --overlap-mm"));
        }

        // Poster size.
        let (mut sw, mut sh) = src.exif_display_size();
        if opts.rotate.swaps_axes() {
            std::mem::swap(&mut sw, &mut sh);
        }
        if sw <= 0.0 || sh <= 0.0 {
            return Err(Diagnostic::error(
                DiagCode::RangeOutOfBounds,
                "the source image has no pixels",
            ));
        }
        let legacy_spec = match opts.size_spec {
            SizeSpec::WidthSheets(n) => crate::size::SizeSpec::WidthSheets(n),
            SizeSpec::HeightSheets(n) => crate::size::SizeSpec::HeightSheets(n),
            SizeSpec::Size { w, h } => crate::size::SizeSpec::SizeMm {
                width: opts.size_unit.to_mm(w),
                height: opts.size_unit.to_mm(h),
            },
        };
        let resolved = crate::size::resolve_output_size(legacy_spec, sheet.w, sheet.h, sw, sh);
        let poster = SizeMm::new(resolved.width_mm, resolved.height_mm);
        if poster.w <= 0.0 || poster.h <= 0.0 {
            return Err(Diagnostic::error(
                DiagCode::RangeOutOfBounds,
                format!("the requested poster size {poster:?} is empty"),
            ));
        }

        let pages_x = (poster.w / step.w).ceil().max(1.0) as u32;
        let pages_y = (poster.h / step.h).ceil().max(1.0) as u32;
        let total_pages = pages_x
            .checked_mul(pages_y)
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagCode::PageLimitExceeded,
                    "the page count overflows",
                )
            })?;
        let cover = SizeMm::new(
            (f64::from(pages_x) - 1.0) * step.w + visible.w,
            (f64::from(pages_y) - 1.0) * step.h + visible.h,
        );

        let pitch = opts.nominal_dot_mm;
        if !(pitch > 0.0) {
            return Err(Diagnostic::error(
                DiagCode::RangeOutOfBounds,
                format!("nominal dot size {pitch} mm must be positive"),
            ));
        }
        let cell_h = cell_height(opts.screen, pitch);
        let cols_total = (cover.w / pitch).ceil().max(1.0) as u32;
        let rows_total = (cover.h / cell_h).ceil().max(1.0) as u32;

        let mut windows = Vec::with_capacity(total_pages as usize);
        for row in 0..pages_y {
            for col in 0..pages_x {
                windows.push(window(
                    col, row, pages_x, pages_y, step.w, step.h, pitch, cell_h, cols_total,
                    rows_total,
                ));
            }
        }

        // Page boxes, in points.
        let media = [0.0, 0.0, 72.0 * sheet.w / 25.4, 72.0 * sheet.h / 25.4];
        let m = margin * PT_PER_MM;
        let trim = [m, m, media[2] - m, media[3] - m];
        let b = opts.bleed_mm * PT_PER_MM;
        let bleed = [
            (trim[0] - b).max(0.0),
            (trim[1] - b).max(0.0),
            (trim[2] + b).min(media[2]),
            (trim[3] + b).min(media[3]),
        ];

        let source_rect_mm = source_rect(poster, Fit::from(opts.fit), opts.align_h, opts.align_v, sw, sh);

        let total_cells = u64::from(cols_total) * u64::from(rows_total);
        let render_mode = resolve_render_mode(opts, total_cells);
        let family_scale = family_scale(opts.screen, opts.coverage);
        let cell_area = pitch * pitch;
        let max_extent = extent_mm(opts.max_raster, pitch, family_scale).max(0.0);
        let source_effective_dpi = {
            let rw = source_rect_mm[2] - source_rect_mm[0];
            let rh = source_rect_mm[3] - source_rect_mm[1];
            let dx = if rw > 0.0 { sw / rw * 25.4 } else { 0.0 };
            let dy = if rh > 0.0 { sh / rh * 25.4 } else { 0.0 };
            dx.min(dy)
        };

        Ok(JobGeometry {
            sheet,
            visible,
            step,
            poster,
            cover,
            margin_mm: margin,
            overlap_mm: opts.overlap_mm,
            overlap_mode: opts.overlap_mode,
            pages_x,
            pages_y,
            total_pages,
            pitch_mm: pitch,
            cell_h_mm: cell_h,
            cols_total,
            rows_total,
            windows,
            source_rect_mm,
            boxes: PageBoxes { media, trim, bleed },
            render_mode,
            effective_dpi: if opts.compress { opts.dpi.value().min(150) } else { opts.dpi.value() },
            effective_quality: if opts.compress { opts.jpeg_quality.min(60) } else { opts.jpeg_quality },
            family_scale,
            cell_area_mm2: cell_area,
            cell_area_total_mm2: total_cells as f64 * cell_area,
            total_cells,
            ink_area_total_mm2: total_cells as f64
                * shape_area_mm2(opts.pattern, max_extent, opts.line_aspect),
            source_effective_dpi,
            source_display_px: (sw as u32, sh as u32),
        })
    }
}

/// The cell window of one page, using the disjoint/contiguous partition rule.
#[allow(clippy::too_many_arguments)]
fn window(
    col: u32,
    row: u32,
    pages_x: u32,
    pages_y: u32,
    step_w: f64,
    step_h: f64,
    pitch: f64,
    cell_h: f64,
    cols_total: u32,
    rows_total: u32,
) -> CellWindow {
    let i0 = (f64::from(col) * step_w / pitch).ceil() as i64;
    let i1 = if col + 1 < pages_x {
        (f64::from(col + 1) * step_w / pitch).ceil() as i64
    } else {
        i64::from(cols_total)
    };
    let j0 = (f64::from(row) * step_h / cell_h).ceil() as i64;
    let j1 = if row + 1 < pages_y {
        (f64::from(row + 1) * step_h / cell_h).ceil() as i64
    } else {
        i64::from(rows_total)
    };
    CellWindow { col, row, i0, i1, j0, j1 }
}

/// The `Auto` policy of amendment A-5.
pub fn resolve_render_mode(opts: &ModernOptions, total_cells: u64) -> RenderMode {
    match opts.render {
        RenderMode::Vector => RenderMode::Vector,
        RenderMode::Raster => RenderMode::Raster,
        RenderMode::Auto => {
            let raster = opts.screen == ScreenFamily::Plain
                || opts.dither != Dither::None
                || (opts.auto_raster_cells > 0 && total_cells > u64::from(opts.auto_raster_cells));
            if raster {
                RenderMode::Raster
            } else {
                RenderMode::Vector
            }
        }
    }
}

/// Fit the source into the poster and place it (design §3.7).
pub fn source_rect(
    poster: SizeMm,
    fit: Fit,
    align_h: Align,
    align_v: Align,
    src_w: f64,
    src_h: f64,
) -> [f64; 4] {
    if src_w <= 0.0 || src_h <= 0.0 || poster.w <= 0.0 || poster.h <= 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let (w, h) = match fit {
        Fit::Stretch => (poster.w, poster.h),
        Fit::Contain => {
            let s = (poster.w / src_w).min(poster.h / src_h);
            (src_w * s, src_h * s)
        }
        Fit::Cover => {
            let s = (poster.w / src_w).max(poster.h / src_h);
            (src_w * s, src_h * s)
        }
    };
    let dx = match align_h {
        Align::Start => 0.0,
        Align::Center => (poster.w - w) / 2.0,
        Align::End => poster.w - w,
    };
    let dy = match align_v {
        Align::Start => 0.0,
        Align::Center => (poster.h - h) / 2.0,
        Align::End => poster.h - h,
    };
    [dx, dy, dx + w, dy + h]
}

/// Diagnostics that need the solved geometry. Shared by [`JobGeometry::solve`]
/// and [`crate::modern::validate`] so there is exactly one implementation.
pub(crate) fn geometry_diagnostics(opts: &ModernOptions, g: &JobGeometry) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if opts.page_limit != 0 && g.total_pages > opts.page_limit {
        out.push(
            Diagnostic::error(
                DiagCode::PageLimitExceeded,
                format!(
                    "{} pages exceeds the hard limit of {}",
                    g.total_pages, opts.page_limit
                ),
            )
            .with_hint("raise --page-limit, use a larger paper or fewer sheets"),
        );
    }
    if opts.page_warn != 0 && g.total_pages > opts.page_warn {
        out.push(Diagnostic::warning(
            DiagCode::PageWarnExceeded,
            format!("{} pages exceeds the warning threshold of {}", g.total_pages, opts.page_warn),
        ));
    }
    let (ex, ey) = g.partial_edge_mm();
    if ex > 0.5 {
        out.push(
            Diagnostic::warning(
                DiagCode::PagePartialEdge,
                format!(
                    "the last column overshoots the poster by {ex:.1} mm (poster {:.1} mm, covered {:.1} mm)",
                    g.poster.w, g.cover.w
                ),
            )
            .with_hint("that sheet is partly blank; adjust the size or the margin"),
        );
    }
    if ey > 0.5 {
        out.push(
            Diagnostic::warning(
                DiagCode::PagePartialEdge,
                format!(
                    "the last row overshoots the poster by {ey:.1} mm (poster {:.1} mm, covered {:.1} mm)",
                    g.poster.h, g.cover.h
                ),
            )
            .with_hint("that sheet is partly blank; adjust the size or the margin"),
        );
    }
    if g.source_effective_dpi > 0.0 && g.source_effective_dpi < 150.0 {
        out.push(
            Diagnostic::warning(
                DiagCode::SourceDpiLow,
                format!(
                    "the source resolves to {:.0} effective dpi over the poster ({})",
                    g.source_effective_dpi,
                    g.dpi_label().text()
                ),
            )
            .with_hint("dots will look soft; use a larger source or a smaller poster"),
        );
    }
    if opts.labels == LabelStyle::Coordinates {
        // The printed label lives inside the cutting margin: its top edge sits
        // label_offset_mm from the media edge, so the offset must clear the
        // text height.
        let text_mm = (opts.label_size_pt / PT_PER_MM).ceil();
        if opts.label_offset_mm < text_mm {
            out.push(
                Diagnostic::error(
                    DiagCode::LabelMarginTooSmall,
                    format!(
                        "labels need {:.0} mm of margin, label_offset_mm is {:.1} mm",
                        text_mm, opts.label_offset_mm
                    ),
                )
                .with_hint("raise --label-offset-mm or lower --label-size-pt"),
            );
        }
    }
    out
}

/// Coverage is only meaningful for the circle; warn otherwise.
pub fn coverage_is_inert(opts: &ModernOptions) -> bool {
    opts.coverage == Coverage::Tangent && opts.screen == ScreenFamily::Hex
}

/// The largest shape extent the job can produce, in millimetres.
pub fn max_extent_mm(opts: &ModernOptions, g: &JobGeometry) -> f64 {
    extent_mm(opts.max_raster, g.pitch_mm, g.family_scale).max(0.0)
}

/// Convert millimetres to points.
pub fn mm_to_pt(mm: f64) -> f64 {
    mm * PT_PER_MM
}

/// Convert points to millimetres.
pub fn pt_to_mm(pt: f64) -> f64 {
    pt / PT_PER_MM
}

/// The paper colour used for page backgrounds.
pub fn paper_color(opts: &ModernOptions) -> Rgb {
    opts.paper_color
}

/// The pattern the job draws (helper for the GUI read-outs).
pub fn pattern_of(opts: &ModernOptions) -> Pattern {
    opts.pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format_double;
    use crate::modern::options::{Fit, OverlapMode, SizeSpec};

    fn a4_golden() -> (ModernOptions, SourceInfo) {
        let mut o = ModernOptions::default();
        o.paper = crate::modern::options::PaperSpec::preset("a4").unwrap();
        o.size_spec = SizeSpec::WidthSheets(5.0);
        o.margin_mm = 10.0;
        o.overlap_mm = 0.0;
        o.nominal_dot_mm = 10.0;
        (o, SourceInfo::synthetic(4000, 3000))
    }

    #[test]
    fn a4_worked_case() {
        let (o, src) = a4_golden();
        let g = JobGeometry::solve(&o, &src).expect("valid job");
        assert_eq!(g.pages_x, 6);
        assert_eq!(g.pages_y, 3);
        assert_eq!(g.total_pages, 18);
        assert!((g.poster.w - 1050.0).abs() < 1e-9, "poster {:?}", g.poster);
        assert!((g.poster.h - 787.5).abs() < 1e-9);
        assert!((g.visible.w - 190.0).abs() < 1e-9);
        assert!((g.visible.h - 277.0).abs() < 1e-9);
        assert!((g.step.w - 190.0).abs() < 1e-9);
        assert!((g.cover.w - 1140.0).abs() < 1e-9, "cover {:?}", g.cover);
        assert!((g.cover.h - 831.0).abs() < 1e-9);
        assert_eq!(g.cols_total, 114);
        assert_eq!(g.rows_total, 84);
        assert_eq!(g.windows.len(), 18);
        let (ex, ey) = g.partial_edge_mm();
        assert!((ex - 90.0).abs() < 1e-9);
        assert!((ey - 43.5).abs() < 1e-9);

        let report = ValidationReport::from_items(geometry_diagnostics(&o, &g));
        assert!(report.has(DiagCode::PagePartialEdge, crate::modern::diagnostics::Severity::Warning));
        assert!(!report.has_blocking());
    }

    #[test]
    fn windows_are_disjoint_and_contiguous() {
        let (o, src) = a4_golden();
        let g = JobGeometry::solve(&o, &src).unwrap();
        for row in 0..g.pages_y {
            for col in 0..g.pages_x {
                let w = g.cell_window(row * g.pages_x + col);
                assert_eq!(w.col, col);
                assert_eq!(w.row, row);
                assert!(w.i1 > w.i0 && w.j1 > w.j0);
                if col + 1 < g.pages_x {
                    let next = g.cell_window(row * g.pages_x + col + 1);
                    assert_eq!(w.i1, next.i0, "x partition must be contiguous");
                } else {
                    assert_eq!(w.i1, i64::from(g.cols_total));
                }
                if row + 1 < g.pages_y {
                    let next = g.cell_window((row + 1) * g.pages_x + col);
                    assert_eq!(w.j1, next.j0, "y partition must be contiguous");
                } else {
                    assert_eq!(w.j1, i64::from(g.rows_total));
                }
            }
        }
        // The union of all windows is exactly the cover grid.
        let mut sum = 0u64;
        for w in &g.windows {
            sum += w.cell_count();
        }
        assert_eq!(sum, g.total_cells);
    }

    #[test]
    fn page_boxes_through_format_double() {
        let (o, src) = a4_golden();
        let g = JobGeometry::solve(&o, &src).unwrap();
        let f = |b: &[f64; 4]| {
            b.iter().map(|v| format_double(*v)).collect::<Vec<_>>().join(" ")
        };
        assert_eq!(f(&g.boxes.media), "0 0 595.28 841.89");
        assert_eq!(f(&g.boxes.trim), "28.35 28.35 566.93 813.54");
        assert_eq!(f(&g.boxes.bleed), f(&g.boxes.trim));

        let mut o2 = o.clone();
        o2.bleed_mm = 3.0;
        let g2 = JobGeometry::solve(&o2, &src).unwrap();
        assert_eq!(f(&g2.boxes.bleed), "19.84 19.84 575.43 822.05");
    }

    #[test]
    fn margin_overlap_grows_the_step() {
        let (mut o, src) = a4_golden();
        o.overlap_mm = 10.0;
        let g = JobGeometry::solve(&o, &src).unwrap();
        assert!((g.visible.w - 190.0).abs() < 1e-9);
        assert!((g.step.w - 200.0).abs() < 1e-9);
        assert_eq!(g.pages_x, 6);
        assert!((g.cover.w - 1190.0).abs() < 1e-9);
        assert!((g.partial_edge_mm().0 - 140.0).abs() < 1e-9);

        // `shared` shortens the step instead (amendment A-1).
        o.overlap_mode = OverlapMode::Shared;
        let gs = JobGeometry::solve(&o, &src).unwrap();
        assert!((gs.step.w - 180.0).abs() < 1e-9);
        assert_eq!(gs.pages_x, 6);
    }

    #[test]
    fn landscape_is_the_transposed_portrait() {
        let (mut o, src) = a4_golden();
        // With an explicit poster size the landscape layout is exactly the
        // transposed portrait layout, page for page.
        o.size_spec = SizeSpec::Size { w: 380.0, h: 554.0 };
        let p = JobGeometry::solve(&o, &src).unwrap();
        o.orientation = Orientation::Landscape;
        let l = JobGeometry::solve(&o, &src).unwrap();
        assert!((l.sheet.w - p.sheet.h).abs() < 1e-9);
        assert!((l.sheet.h - p.sheet.w).abs() < 1e-9);
        assert!((l.visible.w - p.visible.h).abs() < 1e-9);
        assert!((l.visible.h - p.visible.w).abs() < 1e-9);
        // The poster size is the user's own choice and does not transpose;
        // what transposes is everything derived from the sheet.
        let pt = |mm: f64| 72.0 * mm / 25.4;
        assert!((l.boxes.media[2] - pt(297.0)).abs() < 1e-9);
        assert!((l.boxes.media[3] - pt(210.0)).abs() < 1e-9);
        assert!((p.boxes.media[2] - pt(210.0)).abs() < 1e-9);
        assert!((p.boxes.media[3] - pt(297.0)).abs() < 1e-9);
        assert_eq!(p.boxes.trim[0], l.boxes.trim[1]);
        // Both layouts are well formed and partition their covers exactly.
        for g in [&p, &l] {
            let sum: u64 = g.windows.iter().map(CellWindow::cell_count).sum();
            assert_eq!(sum, g.total_cells);
        }
        assert!(l.total_pages > 0);
    }

    #[test]
    fn fit_and_alignment_invariants() {
        let (o, src) = a4_golden();
        let g = JobGeometry::solve(&o, &src).unwrap();
        let rw = g.source_rect_mm[2] - g.source_rect_mm[0];
        let rh = g.source_rect_mm[3] - g.source_rect_mm[1];
        let ar = rw / rh;
        assert!((ar - 4.0 / 3.0).abs() < 1e-9, "contain preserves the aspect");

        let mut o2 = o.clone();
        o2.fit = Fit::Cover;
        let gc = JobGeometry::solve(&o2, &src).unwrap();
        let cw = gc.source_rect_mm[2] - gc.source_rect_mm[0];
        let ch = gc.source_rect_mm[3] - gc.source_rect_mm[1];
        assert!((cw / ch - 4.0 / 3.0).abs() < 1e-9);
        assert!(cw >= g.poster.w || ch >= g.poster.h);

        o2.fit = Fit::Stretch;
        let gs = JobGeometry::solve(&o2, &src).unwrap();
        assert!((gs.source_rect_mm[2] - gs.source_rect_mm[0] - gs.poster.w).abs() < 1e-9);
        assert!((gs.source_rect_mm[3] - gs.source_rect_mm[1] - gs.poster.h).abs() < 1e-9);
    }

    #[test]
    fn alignment_positions_the_rect() {
        let poster = SizeMm::new(100.0, 50.0);
        let r = source_rect(poster, Fit::Stretch, Align::Start, Align::Start, 4.0, 3.0);
        assert_eq!(r[0], 0.0);
        assert_eq!(r[1], 0.0);
        let r = source_rect(poster, Fit::Stretch, Align::Center, Align::Center, 4.0, 3.0);
        assert_eq!(r[0], 0.0);
        let p = SizeMm::new(100.0, 50.0);
        let half = source_rect(p, Fit::Contain, Align::Center, Align::Center, 4.0, 3.0);
        let w = half[2] - half[0];
        assert!((half[0] - (p.w - w) / 2.0).abs() < 1e-12);
        let end = source_rect(p, Fit::Contain, Align::End, Align::End, 4.0, 3.0);
        assert!((end[2] - p.w).abs() < 1e-12);
    }

    #[test]
    fn rotation_swaps_the_source_aspect() {
        let (mut o, src) = a4_golden();
        o.rotate = crate::modern::options::Rotation::D90;
        let g = JobGeometry::solve(&o, &src).unwrap();
        let rw = g.source_rect_mm[2] - g.source_rect_mm[0];
        let rh = g.source_rect_mm[3] - g.source_rect_mm[1];
        assert!((rw / rh - 3.0 / 4.0).abs() < 1e-9, "a quarter turn swaps the aspect");
        assert_eq!(g.source_display_px, (3000, 4000));
    }

    #[test]
    fn auto_render_policy() {
        let (o, src) = a4_golden();
        assert_eq!(JobGeometry::solve(&o, &src).unwrap().render_mode, RenderMode::Vector);

        let mut o2 = o.clone();
        o2.screen = ScreenFamily::Plain;
        assert_eq!(JobGeometry::solve(&o2, &src).unwrap().render_mode, RenderMode::Raster);

        let mut o3 = o.clone();
        o3.dither = Dither::Bayer4x4;
        assert_eq!(JobGeometry::solve(&o3, &src).unwrap().render_mode, RenderMode::Raster);

        let mut o4 = o.clone();
        o4.auto_raster_cells = 100;
        assert_eq!(JobGeometry::solve(&o4, &src).unwrap().render_mode, RenderMode::Raster);

        let mut o5 = o4.clone();
        o5.auto_raster_cells = 0;
        o5.dither = Dither::None;
        assert_eq!(JobGeometry::solve(&o5, &src).unwrap().render_mode, RenderMode::Vector);

        // An explicit choice always wins.
        let mut o6 = o4.clone();
        o6.render = RenderMode::Vector;
        assert_eq!(JobGeometry::solve(&o6, &src).unwrap().render_mode, RenderMode::Vector);
    }

    #[test]
    fn hex_rows_follow_the_vertical_pitch() {
        let (mut o, src) = a4_golden();
        o.screen = ScreenFamily::Hex;
        let g = JobGeometry::solve(&o, &src).unwrap();
        assert!((g.cell_h_mm - 10.0 * 0.8660254037844386).abs() < 1e-9);
        assert_eq!(g.rows_total, (g.cover.h / g.cell_h_mm).ceil() as u32);
        let sum: u64 = g.windows.iter().map(CellWindow::cell_count).sum();
        assert_eq!(sum, g.total_cells);
    }

    #[test]
    fn cell_area_and_ink_totals() {
        let (o, src) = a4_golden();
        let g = JobGeometry::solve(&o, &src).unwrap();
        assert!((g.cell_area_mm2 - 100.0).abs() < 1e-12);
        assert!((g.cell_area_total_mm2 - 100.0 * 114.0 * 84.0).abs() < 1e-6);
        let expected = g.total_cells as f64 * std::f64::consts::PI * 50.0;
        assert!((g.ink_area_total_mm2 - expected).abs() < 1e-3);
    }

    #[test]
    fn page_limit_is_blocking() {
        let (mut o, src) = a4_golden();
        o.page_limit = 4;
        let report = JobGeometry::solve(&o, &src).unwrap_err();
        assert!(report.has_blocking());
        assert!(report.has(DiagCode::PageLimitExceeded, crate::modern::diagnostics::Severity::Error));
        o.page_limit = 0;
        assert!(JobGeometry::solve(&o, &src).is_ok());
    }

    #[test]
    fn dpi_labels() {
        assert_eq!(DpiLabel::of(300.0), DpiLabel::Good);
        assert_eq!(DpiLabel::of(200.0), DpiLabel::Good);
        assert_eq!(DpiLabel::of(150.0), DpiLabel::Fair);
        assert_eq!(DpiLabel::of(119.0), DpiLabel::Low);
    }

    #[test]
    fn compression_caps_the_raster_settings() {
        let (mut o, src) = a4_golden();
        o.dpi = crate::modern::options::Dpi::P300;
        o.jpeg_quality = 90;
        o.compress = true;
        let g = JobGeometry::solve(&o, &src).unwrap();
        assert_eq!(g.effective_dpi, 150);
        assert_eq!(g.effective_quality, 60);
    }

    #[test]
    fn single_cell_jobs_are_well_formed() {
        let mut o = ModernOptions::default();
        o.nominal_dot_mm = 1000.0;
        let src = SourceInfo::synthetic(100, 100);
        let g = JobGeometry::solve(&o, &src).expect("one cell per page is legal");
        assert!(g.cols_total >= 1 && g.rows_total >= 1);
        let sum: u64 = g.windows.iter().map(CellWindow::cell_count).sum();
        assert_eq!(sum, g.total_cells);
    }

    #[test]
    fn margin_is_clamped_into_the_documented_window() {
        let (mut o, src) = a4_golden();
        o.margin_mm = 0.5;
        let g = JobGeometry::solve(&o, &src).unwrap();
        assert!(g.margin_mm >= 2.0);
        o.margin_mm = 500.0;
        let g = JobGeometry::solve(&o, &src).unwrap();
        assert!(g.margin_mm <= 210.0 / 2.0 - 10.0 + 1e-9);
    }
}