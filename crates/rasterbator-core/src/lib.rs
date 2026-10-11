//! # rasterbator-core
//!
//! Rasterbation engine: turn an image into a printable multi-page poster made
//! of a grid of halftone dots, written as a PDF.
//!
//! This is a faithful port of **The Rasterbator** standalone application
//! (Matias Ärje, GPL-2.0-or-later; see `original/` in the repository and the
//! analysis notes in the root `README.md`). The engine reproduces the
//! original's fixed 144 dpi working resolution, its dot-grid math, its
//! `System.Drawing` brightness handling, and the PDF content streams emitted
//! through iTextSharp 1.0.4.0 (including that library's number formatting).
//!
//! The crate is released under **AGPL-3.0-or-later** as part of the derived
//! work; see the repository `LICENSE`.
//!
//! ## Example
//!
//! ```no_run
//! use rasterbator_core::{
//!     load_rgba, rasterbate, ColorMode, EngineOptions, PdfDocument, SizeSpec,
//!     resolve_output_size, paper_size_pt,
//! };
//!
//! let image = load_rgba("poster.png")?;
//! let paper = (210.0, 297.0); // A4, mm
//! let out = resolve_output_size(SizeSpec::WidthSheets(5.0), paper.0, paper.1,
//!                               image.width() as f64, image.height() as f64);
//! let opts = EngineOptions {
//!     paper_width_mm: paper.0,
//!     paper_height_mm: paper.1,
//!     image_width_mm: out.width_mm,
//!     image_height_mm: out.height_mm,
//!     dot_size_mm: 10.0,
//!     color: ColorMode::Fixed { r: 0, g: 0, b: 0 },
//!     cutout: false,
//!     crop: None,
//! };
//! let mut pdf = PdfDocument::from_paper_mm(paper.0, paper.1);
//! let summary = rasterbate(&image, &opts, &mut pdf, &mut |_, _| {});
//! assert_eq!(summary.page_width_pt, paper_size_pt(paper.0, paper.1).0);
//! pdf.save("poster.pdf")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod canvas;
mod engine;
mod numfmt;
mod pdf;
mod size;

pub use canvas::{circle_path, Canvas, CIRCLE_K};
pub use engine::{rasterbate, ColorMode, CropRect, EngineOptions, EngineSummary};
pub use numfmt::format_double;
pub use pdf::PdfDocument;
pub use size::{
    paper_preset, paper_size_pt, resolve_output_size, PaperPreset, ResolvedSize, SizeSpec,
    PAPER_PRESETS,
};

pub use image::RgbaImage;

/// Load an image file and convert it to 8-bit RGBA.
pub fn load_rgba<P: AsRef<std::path::Path>>(path: P) -> Result<RgbaImage, image::ImageError> {
    Ok(image::open(path)?.to_rgba8())
}
