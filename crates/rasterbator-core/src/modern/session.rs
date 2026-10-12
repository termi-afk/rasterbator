//! The paged render session (design §2.4).
//!
//! [`RenderSession::plan`] is available before any rendering, so the CLI can
//! print the page count and the GUI can show a progress total immediately.
//! Exactly one page is in flight, so a 200-page poster never materialises 200
//! buffers. [`RenderSession::render_next`] returns `Ok` after each page, with
//! [`Progress::phase`] == [`RenderPhase::Finished`] on the call that renders
//! the last page; further calls return [`RenderError::Cancelled`].
//!
//! Cancellation is checked every [`CANCEL_CHECK_CELLS`] cells and at page
//! boundaries. On cancel the partial page is dropped with
//! [`PageTarget::abort_page`], so [`ModernPdf`](crate::modern::pdf::ModernPdf)
//! never emits a truncated stream.
//!
//! The [`DitherState`] is allocated once from `cols_total` and survives page
//! boundaries, which is what makes poster-wide error diffusion correct.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::modern::color::Rgb;
use crate::modern::dither::DitherState;
use crate::modern::geometry::{CellWindow, JobGeometry, PageBoxes};
use crate::modern::options::ModernOptions;
use crate::modern::source::PreparedImage;
use crate::modern::sample::CellSampler;

/// How often the cancellation flag is polled while cells are emitted.
pub const CANCEL_CHECK_CELLS: u32 = 64;

/// Coarse stage of a render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderPhase {
    /// Cells are being sampled and emitted.
    Sampling,
    /// The page bitmap is being encoded.
    Encoding,
    /// The last page has been rendered.
    Finished,
    /// The job was cancelled.
    Cancelled,
}

/// A snapshot of job progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    /// Pages completed.
    pub pages_done: u32,
    /// Pages in the job.
    pub total_pages: u32,
    /// Cells emitted so far.
    pub cells_done: u64,
    /// Cells in the job.
    pub total_cells: u64,
    /// The page most recently rendered, 0-based; `u32::MAX` before the first.
    pub page_index: u32,
    /// Current stage.
    pub phase: RenderPhase,
}

impl Progress {
    /// Completed fraction in `0..=1`.
    pub fn fraction(&self) -> f64 {
        if self.total_cells > 0 {
            (self.cells_done as f64 / self.total_cells as f64).clamp(0.0, 1.0)
        } else if self.total_pages > 0 {
            (self.pages_done as f64 / self.total_pages as f64).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Estimated seconds remaining, given the elapsed time at this snapshot.
    pub fn eta_secs(&self, elapsed_secs: f64) -> Option<f64> {
        let f = self.fraction();
        if f <= 0.02 || f >= 1.0 {
            return None;
        }
        Some(elapsed_secs / f * (1.0 - f))
    }
}

/// Aggregate counters of a finished job.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderStats {
    /// Cells that produced a shape.
    pub cells_drawn: u64,
    /// Cells skipped (outside the source rectangle, or zero extent).
    pub cells_skipped: u64,
    /// Sum of the emitted shape areas, in square points.
    pub ink_area_pt2: f64,
    /// Sum of the cell areas considered, in square points. This is the
    /// denominator of [`RenderStats::ink_density_percent`]: the C8 estimate is
    /// `100 * ink / cells`, not `100 * ink / sheet`.
    pub page_area_pt2: f64,
    /// Pixel count of the prepared source.
    pub source_px: u64,
    /// Size of the written file in bytes.
    pub output_bytes: u64,
}

impl RenderStats {
    /// Ink density estimate in percent.
    ///
    /// May exceed 100 for full-cell coverage; an all-black lattice with
    /// full-cell coverage and round dots reaches 157.08 %.
    pub fn ink_density_percent(&self) -> f64 {
        if self.page_area_pt2 <= 0.0 {
            0.0
        } else {
            100.0 * self.ink_area_pt2 / self.page_area_pt2
        }
    }
}

/// A failure that is not a validation error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The cancel flag was observed; the partial page was dropped.
    Cancelled,
    /// A filesystem or device failure, with a message.
    Io(String),
    /// An encoder failure, with a message.
    Encode(String),
    /// The options or geometry made the job impossible to render.
    Invalid(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Cancelled => f.write_str("cancelled"),
            RenderError::Io(m) => write!(f, "{m}"),
            RenderError::Encode(m) => write!(f, "{m}"),
            RenderError::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// The colour-space of an [`EncodedImage`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageComponent {
    /// One 8-bit sample per pixel.
    Gray,
    /// Three 8-bit samples per pixel.
    Rgb,
}

impl ImageComponent {
    /// The PDF `/ColorSpace` name.
    pub fn pdf_name(self) -> &'static str {
        match self {
            ImageComponent::Gray => "DeviceGray",
            ImageComponent::Rgb => "DeviceRGB",
        }
    }

    /// Samples per pixel.
    pub fn channels(self) -> usize {
        match self {
            ImageComponent::Gray => 1,
            ImageComponent::Rgb => 3,
        }
    }
}

/// A raster payload ready to be embedded as a PDF image XObject.
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedImage {
    /// Pixel width.
    pub w: u32,
    /// Pixel height.
    pub h: u32,
    /// Sample layout.
    pub component: ImageComponent,
    /// Encoded bytes: a JFIF/JPEG stream for the `/DCTDecode` path.
    pub bytes: Vec<u8>,
}

/// A device-independent page sink.
///
/// Coordinates are PDF points with the origin at the bottom-left and `y` up;
/// the poster-to-page frame conversion happens once, in the page composer.
pub trait PageTarget {
    /// Start a page. `boxes` are the page boxes in points.
    fn begin_page(&mut self, index: u32, boxes: &PageBoxes, job: &CellWindow);
    /// Set the fill colour.
    fn fill_rgb(&mut self, c: Rgb);
    /// Set the stroke colour.
    fn stroke_rgb(&mut self, c: Rgb);
    /// Set the stroke width in points.
    fn line_width_pt(&mut self, w: f64);
    /// Start a subpath.
    fn move_to(&mut self, x: f64, y: f64);
    /// Extend the subpath.
    fn line_to(&mut self, x: f64, y: f64);
    /// Extend the subpath with a cubic curve.
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64);
    /// Close and fill the current path.
    fn close_fill(&mut self);
    /// Close and stroke the current path.
    fn close_stroke(&mut self);
    /// Place an image over a `[x0, y0, x1, y1]` rectangle in points.
    fn draw_image(&mut self, img: &EncodedImage, rect: [f64; 4]);
    /// Drop the page in flight without emitting it.
    fn abort_page(&mut self);
    /// Finish the page in flight.
    fn end_page(&mut self);
}

/// A paged renderer over one prepared source, one option set and one geometry.
pub struct RenderSession<'a> {
    prepared: &'a PreparedImage,
    opts: &'a ModernOptions,
    geom: &'a JobGeometry,
    cancel: &'a AtomicBool,
    dither: DitherState,
    sampler: CellSampler<'a>,
    next_page: u32,
    page_index: u32,
    stats: RenderStats,
    started: Instant,
}

impl<'a> RenderSession<'a> {
    /// Open a session. Exactly one page is in flight at a time.
    pub fn new(
        prepared: &'a PreparedImage,
        opts: &'a ModernOptions,
        geom: &'a JobGeometry,
        cancel: &'a AtomicBool,
    ) -> Self {
        let dither = DitherState::new(
            geom.cols_total,
            opts.dither,
            opts.dither_levels,
            opts.dither_block,
            opts.dither_strength,
        );
        let sampler = CellSampler::new(prepared, opts.paper_color, opts.sampling, geom.source_rect_mm);
        RenderSession {
            prepared,
            opts,
            geom,
            cancel,
            dither,
            sampler,
            next_page: 0,
            page_index: u32::MAX,
            stats: RenderStats { source_px: prepared.pixel_count(), ..RenderStats::default() },
            started: Instant::now(),
        }
    }

    /// `(pages_x, pages_y, total_pages)`, available before any rendering.
    pub fn plan(&self) -> (u32, u32, u32) {
        (self.geom.pages_x, self.geom.pages_y, self.geom.total_pages)
    }

    /// The current progress snapshot.
    pub fn progress(&self) -> Progress {
        let phase = if self.next_page >= self.geom.total_pages {
            RenderPhase::Finished
        } else if self.geom.render_mode == crate::modern::options::RenderMode::Raster {
            RenderPhase::Encoding
        } else {
            RenderPhase::Sampling
        };
        Progress {
            pages_done: self.next_page,
            total_pages: self.geom.total_pages,
            cells_done: self.stats.cells_drawn + self.stats.cells_skipped,
            total_cells: self.geom.total_cells,
            page_index: self.page_index,
            phase,
        }
    }

    /// Seconds since the session was opened.
    pub fn elapsed_secs(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// The canonical geometry in use.
    pub fn geometry(&self) -> &'a JobGeometry {
        self.geom
    }

    /// The prepared source in use.
    pub fn prepared(&self) -> &'a PreparedImage {
        self.prepared
    }

    /// The aggregate counters.
    pub fn stats(&self) -> &RenderStats {
        &self.stats
    }

    /// The dither row buffer, for tests that assert poster-wide propagation.
    pub fn dither(&self) -> &DitherState {
        &self.dither
    }

    /// True once every page has been rendered.
    pub fn is_finished(&self) -> bool {
        self.next_page >= self.geom.total_pages
    }

    /// Render exactly one page.
    pub fn render_next(&mut self, t: &mut dyn PageTarget) -> Result<Progress, RenderError> {
        if self.is_finished() {
            return Err(RenderError::Cancelled);
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err(RenderError::Cancelled);
        }
        let index = self.next_page;
        self.page_index = index;
        let window = *self.geom.cell_window(index);
        match crate::modern::page::render_page(
            t,
            index,
            &window,
            self.geom,
            self.opts,
            &self.sampler,
            &mut self.dither,
            &mut self.stats,
            self.cancel,
        ) {
            Ok(()) => {
                self.next_page += 1;
                Ok(self.progress())
            }
            Err(e) => {
                t.abort_page();
                Err(e)
            }
        }
    }

    /// Record the size of the written file.
    pub fn set_output_bytes(&mut self, bytes: u64) {
        self.stats.output_bytes = bytes;
    }

    /// Mutable access to the statistics, for the PDF writer.
    pub fn stats_mut(&mut self) -> &mut RenderStats {
        &mut self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::geometry::JobGeometry;
    use crate::modern::options::ModernOptions;
    use crate::modern::source::{PreparedImage, SourceInfo};
    use crate::modern::{ModernPdf, probe::CountingTarget};

    fn job() -> (ModernOptions, SourceInfo, PreparedImage) {
        let mut o = ModernOptions::default();
        o.size_spec = crate::modern::options::SizeSpec::WidthSheets(1.0);
        o.nominal_dot_mm = 30.0;
        o.page_warn = 0;
        o.page_limit = 0;
        o.auto_raster_cells = 0;
        let src = SourceInfo::synthetic(400, 300);
        let img = PreparedImage::solid(400, 300, crate::modern::color::Rgb(128, 128, 128));
        (o, src, img)
    }

    #[test]
    fn plan_is_available_before_rendering() {
        let (o, src, img) = job();
        let g = JobGeometry::solve(&o, &src).unwrap();
        let cancel = AtomicBool::new(false);
        let s = RenderSession::new(&img, &o, &g, &cancel);
        let (x, y, n) = s.plan();
        assert_eq!(n, x * y);
        assert_eq!(n, g.total_pages);
        assert_eq!(s.progress().phase, RenderPhase::Sampling);
        assert_eq!(s.progress().pages_done, 0);
    }

    #[test]
    fn renders_every_page_then_reports_cancelled() {
        let (o, src, img) = job();
        let g = JobGeometry::solve(&o, &src).unwrap();
        let cancel = AtomicBool::new(false);
        let mut s = RenderSession::new(&img, &o, &g, &cancel);
        let mut t = CountingTarget::default();
        let mut last = None;
        for _ in 0..g.total_pages {
            last = Some(s.render_next(&mut t).unwrap());
        }
        assert_eq!(t.pages, g.total_pages);
        assert_eq!(t.aborted, 0);
        assert_eq!(last.unwrap().phase, RenderPhase::Finished);
        assert!(s.is_finished());
        assert_eq!(s.render_next(&mut t), Err(RenderError::Cancelled));
        assert_eq!(t.aborted, 0, "no page is in flight, so nothing is aborted");
        assert_eq!(t.pages, g.total_pages);
        assert!(s.stats().cells_drawn > 0);
    }

    #[test]
    fn cancellation_aborts_the_page_in_flight() {
        let (mut o, src, img) = job();
        o.nominal_dot_mm = 6.0;
        let g = JobGeometry::solve(&o, &src).unwrap();
        let cancel = AtomicBool::new(true);
        let mut s = RenderSession::new(&img, &o, &g, &cancel);
        let mut t = CountingTarget::default();
        assert_eq!(s.render_next(&mut t), Err(RenderError::Cancelled));
        assert_eq!(t.pages, 0, "the page was never begun");
    }

    #[test]
    fn ink_density_uses_cell_areas() {
        let stats = RenderStats {
            cells_drawn: 1000,
            ink_area_pt2: 157_079.632_679_489_66,
            page_area_pt2: 100_000.0,
            ..RenderStats::default()
        };
        assert!((stats.ink_density_percent() - 157.079_632_7).abs() < 1e-6);
        let empty = RenderStats::default();
        assert_eq!(empty.ink_density_percent(), 0.0);
    }

    #[test]
    fn progress_fraction_and_eta() {
        let p = Progress {
            pages_done: 1,
            total_pages: 4,
            cells_done: 250,
            total_cells: 1000,
            page_index: 0,
            phase: RenderPhase::Sampling,
        };
        assert!((p.fraction() - 0.25).abs() < 1e-12);
        let eta = p.eta_secs(4.0).unwrap();
        assert!((eta - 12.0).abs() < 1e-9);
        let done = Progress { cells_done: 1000, ..p };
        assert!(done.eta_secs(4.0).is_none());
    }

    #[test]
    fn a_session_feeds_the_pdf_writer() {
        let (o, src, img) = job();
        let g = JobGeometry::solve(&o, &src).unwrap();
        let cancel = AtomicBool::new(false);
        let mut s = RenderSession::new(&img, &o, &g, &cancel);
        let mut pdf = ModernPdf::new(&o, &g);
        let mut target = crate::modern::pdf::PdfTarget::new(&mut pdf);
        while !s.is_finished() {
            s.render_next(&mut target).unwrap();
        }
        target.finish();
        let bytes = pdf.bytes();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF\n"));
        assert!(bytes.len() > 800);
    }
}