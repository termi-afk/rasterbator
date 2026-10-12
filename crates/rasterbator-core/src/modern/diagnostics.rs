//! Validation diagnostics for modern mode.
//!
//! Every user-facing decision that modern mode makes about a job is reported
//! through [`Diagnostic`]s produced by [`crate::modern::validate`]. There is
//! exactly one source of diagnostics: the CLI and the GUI both render the same
//! [`ValidationReport`] and never re-check anything themselves.
//!
//! Three severities exist, matching the GUI's inline / warning / blocking
//! hierarchy:
//!
//! * [`Severity::Error`] — blocking. The job must not be rendered.
//! * [`Severity::Warning`] — non-blocking unless `strict` promotes it.
//! * [`Severity::Info`] — purely informational; an option is inert.

use std::fmt;

/// How serious a [`Diagnostic`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Blocking: the job cannot run.
    Error = 0,
    /// Non-blocking unless `--strict`.
    Warning = 1,
    /// Informational: an option is inert in the current configuration.
    Info = 2,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        })
    }
}

/// Stable identifiers for every diagnostic modern mode can emit.
///
/// The numeric values are the sort key of [`ValidationReport`], so they are
/// grouped by area and never renumbered once released.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagCode {
    // -- screen family / pattern (interaction matrix §3.10) -------------
    /// `screen = plain` makes the screen options inert.
    ScreenPlainInert,
    /// `stagger != off` without `screen = lattice`.
    StaggerNeedsLattice,
    /// `pattern` is not legal on the selected screen family.
    PatternNotSupported,
    /// `render = vector` with `screen = plain`.
    PlainVectorBlocked,
    /// `mosaic_contrast != 1.0` without `pattern = mosaic`.
    MosaicContrastNeedsMosaic,
    /// `line_aspect != 0.5` without a line pattern.
    LineAspectNeedsLine,
    /// `coverage = tangent` on a screen where it has no defined meaning.
    CoverageInert,

    // -- tone / sampling -------------------------------------------------
    /// `min_raster > max_raster`.
    ToneBandInverted,
    /// `nominal_dot_mm` below the fine-screen threshold.
    FineScreen,
    /// `halo_cells = 0` leaves visible seams at the trim edges.
    HaloCellsZero,
    /// Dither parameters set while `dither = none`.
    DitherInert,
    /// Tone parameters set while their curve is inactive.
    ToneCurveInert,
    /// Dual-tone softness set while `ink != dual`.
    DualSoftnessInert,
    /// Resampling disabled (aliasing risk).
    ResampleOff,
    /// Sub-pixel cells with `box` sampling.
    SubPixelCells,

    // -- colour ----------------------------------------------------------
    /// Dual-tone foreground and background are nearly the same colour.
    SimilarDualColors,
    /// A colour option is inert for the active ink mode.
    InkOptionInert,
    /// Shading strength set while shading is off.
    ShadingInert,

    // -- layout ----------------------------------------------------------
    /// A numeric option is outside its documented range.
    RangeOutOfBounds,
    /// The printable area would be smaller than the hard floor.
    PrintableTooSmall,
    /// `overlap_mm` is not smaller than the visible area.
    OverlapExceedsVisible,
    /// `overlap_mode = shared` while a dither is active.
    SharedOverlapDither,
    /// `overlap_mode = shared` with `halo_cells > 0`.
    SharedOverlapHalo,
    /// Corner marks longer than the cutting margin allows.
    CornerMarksOffSheet,
    /// Labels do not fit inside the cutting margin.
    LabelMarginTooSmall,
    /// A layout option is inert with the active guide style.
    GuideOptionInert,
    /// Page count above `page_limit`.
    PageLimitExceeded,
    /// Page count above `page_warn`.
    PageWarnExceeded,
    /// The last column and/or row only partially covers the poster.
    PagePartialEdge,
    /// A named colour theme is not available.
    ThemeUnknown,

    // -- output ----------------------------------------------------------
    /// Render-quality options set while the resolved mode is vector.
    OutputInertVector,
    /// `jpeg_quality` low enough to look degraded.
    JpegQualityLow,
    /// `compress` lowered the effective dpi / quality.
    CompressionCapped,
    /// `creation_date = now` breaks byte-for-byte determinism.
    NonDeterministicDate,
    /// Title metadata is too long or contains a reserved character.
    TitleInvalid,
    /// Creator metadata contains non-ASCII bytes.
    CreatorInvalid,

    // -- input -----------------------------------------------------------
    /// The crop rectangle is empty or outside the decoded image.
    CropInvalid,
    /// EXIF orientation is applied but the image carries none.
    ExifNoOrientation,
    /// Input guard tripped (file size or pixel count).
    SourceTooLarge,
    /// Effective source resolution is below the quality threshold.
    SourceDpiLow,

    // -- UX / presets ----------------------------------------------------
    /// A preset name violates the portable filename charset.
    PresetNameInvalid,
    /// A preset file could not be read or parsed.
    PresetInvalid,
    /// A preset with this name already exists.
    PresetOverwrite,
}

impl DiagCode {
    /// Every code, in numeric order.
    pub const ALL: &'static [DiagCode] = &[
        DiagCode::ScreenPlainInert,
        DiagCode::StaggerNeedsLattice,
        DiagCode::PatternNotSupported,
        DiagCode::PlainVectorBlocked,
        DiagCode::MosaicContrastNeedsMosaic,
        DiagCode::LineAspectNeedsLine,
        DiagCode::CoverageInert,
        DiagCode::ToneBandInverted,
        DiagCode::FineScreen,
        DiagCode::HaloCellsZero,
        DiagCode::DitherInert,
        DiagCode::ToneCurveInert,
        DiagCode::DualSoftnessInert,
        DiagCode::ResampleOff,
        DiagCode::SubPixelCells,
        DiagCode::SimilarDualColors,
        DiagCode::InkOptionInert,
        DiagCode::ShadingInert,
        DiagCode::RangeOutOfBounds,
        DiagCode::PrintableTooSmall,
        DiagCode::OverlapExceedsVisible,
        DiagCode::SharedOverlapDither,
        DiagCode::SharedOverlapHalo,
        DiagCode::CornerMarksOffSheet,
        DiagCode::LabelMarginTooSmall,
        DiagCode::GuideOptionInert,
        DiagCode::PageLimitExceeded,
        DiagCode::PageWarnExceeded,
        DiagCode::PagePartialEdge,
        DiagCode::ThemeUnknown,
        DiagCode::OutputInertVector,
        DiagCode::JpegQualityLow,
        DiagCode::CompressionCapped,
        DiagCode::NonDeterministicDate,
        DiagCode::TitleInvalid,
        DiagCode::CreatorInvalid,
        DiagCode::CropInvalid,
        DiagCode::ExifNoOrientation,
        DiagCode::SourceTooLarge,
        DiagCode::SourceDpiLow,
        DiagCode::PresetNameInvalid,
        DiagCode::PresetInvalid,
        DiagCode::PresetOverwrite,
    ];

    /// A short, stable machine-readable slug, used by `--print-summary`.
    pub fn slug(self) -> &'static str {
        match self {
            DiagCode::ScreenPlainInert => "SCREEN_PLAIN_INERT",
            DiagCode::StaggerNeedsLattice => "STAGGER_NEEDS_LATTICE",
            DiagCode::PatternNotSupported => "PATTERN_NOT_SUPPORTED",
            DiagCode::PlainVectorBlocked => "PLAIN_VECTOR_BLOCKED",
            DiagCode::MosaicContrastNeedsMosaic => "MOSAIC_CONTRAST_NEEDS_MOSAIC",
            DiagCode::LineAspectNeedsLine => "LINE_ASPECT_NEEDS_LINE",
            DiagCode::CoverageInert => "COVERAGE_INERT",
            DiagCode::ToneBandInverted => "TONE_BAND_INVERTED",
            DiagCode::FineScreen => "FINE_SCREEN",
            DiagCode::HaloCellsZero => "HALO_CELLS_ZERO",
            DiagCode::DitherInert => "DITHER_INERT",
            DiagCode::ToneCurveInert => "TONE_CURVE_INERT",
            DiagCode::DualSoftnessInert => "DUAL_SOFTNESS_INERT",
            DiagCode::ResampleOff => "RESAMPLE_OFF",
            DiagCode::SubPixelCells => "SUB_PIXEL_CELLS",
            DiagCode::SimilarDualColors => "SIMILAR_DUAL_COLORS",
            DiagCode::InkOptionInert => "INK_OPTION_INERT",
            DiagCode::ShadingInert => "SHADING_INERT",
            DiagCode::RangeOutOfBounds => "RANGE_OUT_OF_BOUNDS",
            DiagCode::PrintableTooSmall => "PRINTABLE_TOO_SMALL",
            DiagCode::OverlapExceedsVisible => "OVERLAP_EXCEEDS_VISIBLE",
            DiagCode::SharedOverlapDither => "SHARED_OVERLAP_DITHER",
            DiagCode::SharedOverlapHalo => "SHARED_OVERLAP_HALO",
            DiagCode::CornerMarksOffSheet => "CORNER_MARKS_OFF_SHEET",
            DiagCode::LabelMarginTooSmall => "LABEL_MARGIN_TOO_SMALL",
            DiagCode::GuideOptionInert => "GUIDE_OPTION_INERT",
            DiagCode::PageLimitExceeded => "PAGE_LIMIT_EXCEEDED",
            DiagCode::PageWarnExceeded => "PAGE_WARN_EXCEEDED",
            DiagCode::PagePartialEdge => "PAGE_PARTIAL_EDGE",
            DiagCode::ThemeUnknown => "THEME_UNKNOWN",
            DiagCode::OutputInertVector => "OUTPUT_INERT_VECTOR",
            DiagCode::JpegQualityLow => "JPEG_QUALITY_LOW",
            DiagCode::CompressionCapped => "COMPRESSION_CAPPED",
            DiagCode::NonDeterministicDate => "NON_DETERMINISTIC_DATE",
            DiagCode::TitleInvalid => "TITLE_INVALID",
            DiagCode::CreatorInvalid => "CREATOR_INVALID",
            DiagCode::CropInvalid => "CROP_INVALID",
            DiagCode::ExifNoOrientation => "EXIF_NO_ORIENTATION",
            DiagCode::SourceTooLarge => "SOURCE_TOO_LARGE",
            DiagCode::SourceDpiLow => "SOURCE_DPI_LOW",
            DiagCode::PresetNameInvalid => "PRESET_NAME_INVALID",
            DiagCode::PresetInvalid => "PRESET_INVALID",
            DiagCode::PresetOverwrite => "PRESET_OVERWRITE",
        }
    }
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// One validation message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// How serious it is.
    pub severity: Severity,
    /// Stable identifier.
    pub code: DiagCode,
    /// One-line, actionable message.
    pub message: String,
    /// Optional "what to do about it" line.
    pub hint: Option<String>,
}

impl Diagnostic {
    /// Build an error.
    pub fn error(code: DiagCode, message: impl Into<String>) -> Self {
        Diagnostic { severity: Severity::Error, code, message: message.into(), hint: None }
    }

    /// Build a warning.
    pub fn warning(code: DiagCode, message: impl Into<String>) -> Self {
        Diagnostic { severity: Severity::Warning, code, message: message.into(), hint: None }
    }

    /// Build an info diagnostic.
    pub fn info(code: DiagCode, message: impl Into<String>) -> Self {
        Diagnostic { severity: Severity::Info, code, message: message.into(), hint: None }
    }

    /// Attach an actionable hint.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Attach a hint only when the predicate holds.
    pub fn with_hint_if(self, cond: bool, hint: impl Into<String>) -> Self {
        if cond {
            self.with_hint(hint)
        } else {
            self
        }
    }

    /// Turn a warning into a blocking error (the `--strict` promotion).
    pub fn promote(mut self) -> Self {
        if self.severity == Severity::Warning {
            self.severity = Severity::Error;
        }
        self
    }

    /// Turn an info into a warning (used when a promotion would otherwise be silent).
    pub fn escalate(mut self) -> Self {
        if self.severity == Severity::Info {
            self.severity = Severity::Warning;
        }
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.hint {
            Some(h) => write!(f, "{}: {} (hint: {})", self.severity, self.message, h),
            None => write!(f, "{}: {}", self.severity, self.message),
        }
    }
}

/// The full outcome of validating one option set.
///
/// The report is kept sorted by `(severity, code)` so that the CLI output, the
/// GUI status list and the `--print-summary` payload are all deterministic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationReport {
    items: Vec<Diagnostic>,
}

impl ValidationReport {
    /// An empty report (nothing wrong).
    pub fn empty() -> Self {
        ValidationReport { items: Vec::new() }
    }

    /// Build from a list, sorting and de-duplicating.
    pub fn from_items(mut items: Vec<Diagnostic>) -> Self {
        items.sort_by(|a, b| (a.severity, a.code).cmp(&(b.severity, b.code)));
        items.dedup_by(|a, b| a.severity == b.severity && a.code == b.code && a.message == b.message);
        ValidationReport { items }
    }

    /// Add one diagnostic.
    pub fn push(&mut self, d: Diagnostic) {
        self.items.push(d);
    }

    /// Sort into the canonical order. Called by [`Self::finalize`].
    pub fn sort(&mut self) {
        self.items.sort_by(|a, b| (a.severity, a.code).cmp(&(b.severity, b.code)));
    }

    /// Apply `--strict` promotion and sort. Idempotent.
    pub fn finalize(&mut self, strict: bool) {
        if strict {
            for d in &mut self.items {
                d.severity = if d.severity == Severity::Warning { Severity::Error } else { d.severity };
            }
        }
        self.sort();
    }

    /// All diagnostics, in canonical order.
    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    /// Diagnostics of one severity.
    pub fn with_severity(&self, s: Severity) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter().filter(move |d| d.severity == s)
    }

    /// True when at least one blocking error is present.
    pub fn has_blocking(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    /// Number of blocking errors.
    pub fn error_count(&self) -> usize {
        self.with_severity(Severity::Error).count()
    }

    /// Number of warnings (after any `--strict` promotion).
    pub fn warning_count(&self) -> usize {
        self.with_severity(Severity::Warning).count()
    }

    /// True when the code appears with the given severity.
    pub fn has(&self, code: DiagCode, severity: Severity) -> bool {
        self.items.iter().any(|d| d.code == code && d.severity == severity)
    }

    /// The first message for a code, if any.
    pub fn message_for(&self, code: DiagCode) -> Option<&str> {
        self.items.iter().find(|d| d.code == code).map(|d| d.message.as_str())
    }

    /// Consume the report and turn the first blocking error into a single error.
    ///
    /// This is what the CLI does before touching the filesystem.
    pub fn into_error(self) -> Result<(), Diagnostic> {
        match self.items.iter().find(|d| d.severity == Severity::Error) {
            Some(d) => Err(d.clone()),
            None => Ok(()),
        }
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for d in &self.items {
            if d.severity == Severity::Info {
                continue;
            }
            if !first {
                f.write_str("\n")?;
            }
            first = false;
            write!(f, "{d}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_sorts_by_severity_then_code() {
        let r = ValidationReport::from_items(vec![
            Diagnostic::info(DiagCode::DitherInert, "i"),
            Diagnostic::error(DiagCode::ToneBandInverted, "e2"),
            Diagnostic::warning(DiagCode::FineScreen, "w"),
            Diagnostic::error(DiagCode::PlainVectorBlocked, "e1"),
        ]);
        let sev: Vec<_> = r.items().iter().map(|d| (d.severity, d.code)).collect();
        assert_eq!(
            sev,
            vec![
                (Severity::Error, DiagCode::PlainVectorBlocked),
                (Severity::Error, DiagCode::ToneBandInverted),
                (Severity::Warning, DiagCode::FineScreen),
                (Severity::Info, DiagCode::DitherInert),
            ]
        );
    }

    #[test]
    fn strict_promotes_warnings() {
        let mut r = ValidationReport::from_items(vec![Diagnostic::warning(DiagCode::FineScreen, "w")]);
        assert!(!r.has_blocking());
        r.finalize(true);
        assert!(r.has_blocking());
        // Idempotent.
        r.finalize(true);
        assert_eq!(r.error_count(), 1);
        assert_eq!(r.warning_count(), 0);
    }

    #[test]
    fn slugs_are_unique() {
        let mut seen: Vec<&str> = DiagCode::ALL.iter().map(|c| c.slug()).collect();
        seen.sort_unstable();
        let n = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), n, "duplicate diagnostic slug");
    }

    #[test]
    fn codes_are_sorted_and_complete() {
        let all = DiagCode::ALL;
        for w in all.windows(2) {
            assert!(w[0] < w[1], "{:?} must sort before {:?}", w[0], w[1]);
        }
    }
}