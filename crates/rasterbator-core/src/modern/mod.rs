//! Modern mode: the second, additive rendering engine.
//!
//! Modern mode is a complete second rasterbation engine behind the same tools
//! as the legacy engine. It never touches the legacy path, so legacy output
//! stays byte-identical (invariant I1), and it adds no new crate dependency
//! (invariant I3).
//!
//! # Architecture
//!
//! ```text
//! ModernOptions ──validate()──> ValidationReport
//!       │
//!       ├──prepare──> PreparedImage
//!       │
//! JobGeometry::solve(opts, src)          <- the only layout computation (I2)
//!       │
//! RenderSession ──render_next()──> PageTarget
//!                                    ├─ pdf::PdfTarget     -> ModernPdf
//!                                    ├─ preview::BitmapTarget
//!                                    └─ probe::CountingTarget (tests)
//! ```
//!
//! Exactly one page is in flight, so peak memory is the prepared source plus
//! one page target. Every consumer takes `&JobGeometry`: the GUI never
//! recomputes sheet counts, boxes or cell windows.
//!
//! # Determinism
//!
//! Identical inputs and options produce identical bytes: no `CreationDate` by
//! default, fixed `f64` operation order, fixed PDF object-id assignment, and
//! object ids reserved from the plan rather than allocated as pages arrive.

pub mod color;
pub mod diagnostics;
pub mod dither;
pub mod geometry;
pub mod options;
pub mod page;
pub mod pattern;
pub mod pdf;
pub mod prepare;
pub mod presets;
pub mod preview;
pub mod sample;
pub mod screen;
pub mod session;
pub mod source;
pub mod strokefont;
pub mod theme;
pub mod tone;
pub mod validate;

pub use color::{Rgb, RgbaF64};
pub use diagnostics::{DiagCode, Diagnostic, Severity, ValidationReport};
pub use geometry::{CellWindow, DpiLabel, JobGeometry, PageBoxes, SizeMm, PT_PER_MM};
pub use options::*;
pub use page::render_page;
pub use pattern::{PathSink, RecordedPath};
pub use pdf::ModernPdf;
pub use prepare::prepare;
pub use prepare::PrepareOptions;
pub use presets::PresetStore;
pub use preview::{BitmapTarget, PreviewDetail, PreviewFrame};
pub use sample::{CellSampler, CellValue};
pub use screen::ScreenLattice;
pub use session::{
    EncodedImage, ImageComponent, PageTarget, Progress, RenderError, RenderPhase, RenderSession,
    RenderStats,
};
pub use source::{PreparedImage, SourceInfo, MAX_FILE_BYTES, MAX_PIXELS};
pub use theme::Theme;
pub use validate::validate;

/// A counting page target, for tests that assert nothing escapes a window.
pub mod probe {
    use super::{CellWindow, PageBoxes, EncodedImage, PageTarget, Rgb};

    /// A [`PageTarget`] that only counts, and optionally records every op.
    #[derive(Clone, Debug, Default)]
    pub struct CountingTarget {
        /// Pages begun.
        pub pages: u32,
        /// Pages aborted.
        pub aborted: u32,
        /// Paths closed and filled.
        pub fills: u64,
        /// Paths closed and stroked.
        pub strokes: u64,
        /// Images drawn.
        pub images: u64,
        /// Fill colours seen.
        pub fill_colors: Vec<Rgb>,
        /// The window of the page in flight.
        pub current: Option<CellWindow>,
    }

    impl CountingTarget {
        /// Number of drawing ops recorded.
        pub fn ops(&self) -> u64 {
            self.fills + self.strokes + self.images
        }
    }

    impl PageTarget for CountingTarget {
        fn begin_page(&mut self, _index: u32, _boxes: &PageBoxes, job: &CellWindow) {
            self.pages += 1;
            self.current = Some(*job);
        }
        fn fill_rgb(&mut self, c: Rgb) {
            self.fill_colors.push(c);
        }
        fn stroke_rgb(&mut self, _c: Rgb) {}
        fn line_width_pt(&mut self, _w: f64) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn curve_to(&mut self, _x1: f64, _y1: f64, _x2: f64, _y2: f64, _x3: f64, _y3: f64) {}
        fn close_fill(&mut self) {
            self.fills += 1;
        }
        fn close_stroke(&mut self) {
            self.strokes += 1;
        }
        fn draw_image(&mut self, _img: &EncodedImage, _rect: [f64; 4]) {
            self.images += 1;
        }
        fn abort_page(&mut self) {
            self.aborted += 1;
            self.current = None;
        }
        fn end_page(&mut self) {
            self.current = None;
        }
    }
}