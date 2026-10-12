//! Option validation (design §1 rules and the §3.10 interaction matrix).
//!
//! This is the **only** source of diagnostics in modern mode. The CLI prints
//! what it returns and exits 2 on a blocking error; the GUI tints the offending
//! control and disables Render. Neither re-checks anything itself.
//!
//! The report is sorted by `(severity, code)`, so the output is stable across
//! runs, and `strict` promotes every warning to a blocking error.

use crate::modern::diagnostics::{DiagCode, Diagnostic, ValidationReport};
use crate::modern::geometry::{geometry_diagnostics, JobGeometry};
use crate::modern::options::{
    Align, Coverage, CreationDate, Dither, Fit, GuideStyle, InkMode, LabelStyle, ModernOptions,
    Pattern, RenderMode, Rotation, ScreenFamily, Stagger, ToneCurve,
};
use crate::modern::source::SourceInfo;
use crate::modern::theme;

/// Validate an option set, optionally against a loaded source and a solved
/// geometry.
///
/// * `src` enables the crop-in-bounds check.
/// * `geom` enables the page-count and partial-edge checks; those are also
///   produced by [`JobGeometry::solve`], which is the single implementation.
pub fn validate(
    opts: &ModernOptions,
    src: Option<&SourceInfo>,
    geom: Option<&JobGeometry>,
) -> ValidationReport {
    let mut items = Vec::new();
    items.extend(render_and_screen(opts));
    items.extend(tone(opts, geom));
    items.extend(colour(opts));
    items.extend(layout(opts, geom));
    items.extend(output(opts, geom));
    items.extend(input(opts, src));
    items.extend(ux(opts));
    if let Some(g) = geom {
        items.extend(geometry_diagnostics(opts, g));
    }
    let mut report = ValidationReport::from_items(items);
    report.finalize(opts.strict);
    report
}

/// Validate against a set of known theme ids, adding a blocking diagnostic for
/// an unknown one. [`validate`] cannot check existence without the store.
pub fn validate_with_themes(
    opts: &ModernOptions,
    src: Option<&SourceInfo>,
    geom: Option<&JobGeometry>,
    known_themes: &[&str],
) -> ValidationReport {
    let mut report = validate(opts, src, geom);
    if !opts.theme.is_empty() {
        let id = opts.theme.trim();
        let found = known_themes.iter().any(|t| t.eq_ignore_ascii_case(id))
            || known_themes.is_empty() && theme::theme(id).is_some();
        if !found {
            report.push(
                Diagnostic::error(
                    DiagCode::ThemeUnknown,
                    format!("theme '{id}' is not available"),
                )
                .with_hint(format!(
                    "available: {}",
                    known_themes
                        .iter()
                        .copied()
                        .chain(theme::theme_ids())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
            report.sort();
        }
    }
    report
}

fn render_and_screen(o: &ModernOptions) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let plain = o.screen == ScreenFamily::Plain;

    if plain {
        // One combined diagnostic for every inert screen option.
        let inert: Vec<&str> = [
            ("pattern", o.pattern != Pattern::Dot),
            ("stagger", o.stagger != Stagger::Off),
            ("coverage", o.coverage != Coverage::FullCell),
            ("mosaic_contrast", o.mosaic_contrast != 1.0),
            ("line_aspect", o.line_aspect != 0.5),
            ("nominal_dot_mm", o.nominal_dot_mm != 10.0),
        ]
        .into_iter()
        .filter(|(_, set)| *set)
        .map(|(name, _)| name)
        .collect();
        if !inert.is_empty() {
            d.push(
                Diagnostic::warning(
                    DiagCode::ScreenPlainInert,
                    format!(
                        "screen=plain ignores: {}",
                        inert.join(", ")
                    ),
                )
                .with_hint("the plain screen has no lattice; the tone band still controls its black and white points"),
            );
        }
    }

    if o.stagger != Stagger::Off && o.screen == ScreenFamily::Hex {
        d.push(
            Diagnostic::error(
                DiagCode::StaggerNeedsLattice,
                format!("stagger={} needs screen=lattice, not {}", o.stagger.cli_str(), o.screen.cli_str()),
            )
            .with_hint("the honeycomb screen has its own half-pitch row shift"),
        );
    }

    if o.screen == ScreenFamily::Hex && o.pattern != Pattern::Dot {
        let bad = match o.pattern {
            Pattern::Mosaic => Some("a half-pitch-staggered cell is not a square lattice"),
            p if p.is_line() => Some("a bar in a staggered cell reads as a printing fault"),
            _ => None,
        };
        if let Some(why) = bad {
            d.push(
                Diagnostic::error(
                    DiagCode::PatternNotSupported,
                    format!("pattern={} is not legal on screen=hex: {why}", o.pattern.cli_str()),
                )
                .with_hint("screen=hex supports dot, square, triangle and triangle-alt"),
            );
        }
    }

    if o.render == RenderMode::Vector && plain {
        d.push(
            Diagnostic::error(
                DiagCode::PlainVectorBlocked,
                "render=vector with screen=plain has no renderer",
            )
            .with_hint("use --render raster or --render auto with --screen plain"),
        );
    }

    if o.mosaic_contrast != 1.0 && o.pattern != Pattern::Mosaic {
        d.push(
            Diagnostic::error(
                DiagCode::MosaicContrastNeedsMosaic,
                format!(
                    "mosaic_contrast={} needs pattern=mosaic, not {}",
                    o.mosaic_contrast,
                    o.pattern.cli_str()
                ),
            )
            .with_hint("--mosaic-contrast changes colours, and only mosaic squares carry it"),
        );
    }

    if o.line_aspect != 0.5 && !o.pattern.is_line() {
        d.push(
            Diagnostic::error(
                DiagCode::LineAspectNeedsLine,
                format!(
                    "line_aspect={} needs a line pattern, not {}",
                    o.line_aspect,
                    o.pattern.cli_str()
                ),
            )
            .with_hint("line-aspect is the bar thickness of line-h, line-v and line-morph"),
        );
    }

    if o.halo_cells == 0 {
        d.push(
            Diagnostic::warning(
                DiagCode::HaloCellsZero,
                "halo_cells=0 leaves visible seams where dots cross a trim edge",
            )
            .with_hint("use 1 to draw one extra cell ring per side"),
        );
    }

    if o.nominal_dot_mm < 3.0 {
        d.push(
            Diagnostic::warning(
                DiagCode::FineScreen,
                format!(
                    "dot size {:.2} mm is a fine screen: slow to render and light on ink",
                    o.nominal_dot_mm
                ),
            )
            .with_hint("3 mm or more prints reliably on most presses"),
        );
    }

    for (name, v, lo, hi) in [
        ("halo_cells", f64::from(o.halo_cells), 0.0, 2.0),
        ("nominal_dot_mm", o.nominal_dot_mm, 1.0, 500.0),
        ("grid_phase_mm.x", o.grid_phase_mm.0, -500.0, 500.0),
        ("grid_phase_mm.y", o.grid_phase_mm.1, -500.0, 500.0),
    ] {
        if v < lo || v > hi {
            d.push(
                Diagnostic::error(
                    DiagCode::RangeOutOfBounds,
                    format!("{name}={v} is outside {lo}..={hi}"),
                )
                .with_hint("see the range printed by --help"),
            );
        }
    }
    d
}

fn tone(o: &ModernOptions, geom: Option<&JobGeometry>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    if o.min_raster > o.max_raster {
        d.push(
            Diagnostic::error(
                DiagCode::ToneBandInverted,
                format!("min_raster {} is larger than max_raster {}", o.min_raster, o.max_raster),
            )
            .with_hint("the tone band is a gain and an offset; it may not be inverted"),
        );
    }
    for (name, v, lo, hi) in [
        ("min_raster", o.min_raster, 0.0, 1.0),
        ("max_raster", o.max_raster, 0.0, 1.0),
        ("tone_exponent", o.tone_exponent, 0.1, 8.0),
        ("tone_log_k", o.tone_log_k, 1.0, 16.0),
        ("dither_strength", o.dither_strength, 0.0, 1.0),
        ("line_aspect", o.line_aspect, 0.05, 1.0),
        ("mosaic_contrast", o.mosaic_contrast, 0.1, 8.0),
    ] {
        if v < lo || v > hi {
            d.push(
                Diagnostic::error(
                    DiagCode::RangeOutOfBounds,
                    format!("{name}={v} is outside {lo}..={hi}"),
                )
                .with_hint("see the range printed by --help"),
            );
        }
    }
    if !(2..=64).contains(&o.dither_levels) {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("dither_levels={} is outside 2..=64", o.dither_levels),
        ));
    }
    if !(1..=8).contains(&o.dither_block) {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("dither_block={} is outside 1..=8", o.dither_block),
        ));
    }

    if o.dither == Dither::None && (o.dither_strength != 1.0 || o.dither_levels != 8 || o.dither_block != 1)
    {
        d.push(
            Diagnostic::info(
                DiagCode::DitherInert,
                "dither options are inert while dither=none",
            )
            .with_hint("set --dither fs, atkinson or bayer4x4 to use them"),
        );
    }
    if o.tone_curve == ToneCurve::None && (o.tone_exponent != 2.2 || o.tone_log_k != 8.0) {
        d.push(
            Diagnostic::info(
                DiagCode::ToneCurveInert,
                "tone parameters are inert while tone=none",
            )
            .with_hint("set --tone exp or --tone log to use them"),
        );
    }
    if o.tone_curve == ToneCurve::Exponential && o.tone_log_k != 8.0 {
        d.push(Diagnostic::info(
            DiagCode::ToneCurveInert,
            "tone_log_k is inert while tone=exp",
        ));
    }
    if o.tone_curve == ToneCurve::Logarithmic && o.tone_exponent != 2.2 {
        d.push(Diagnostic::info(
            DiagCode::ToneCurveInert,
            "tone_exponent is inert while tone=log",
        ));
    }
    if o.ink != InkMode::DualTone && o.dual_softness != 1.0 {
        d.push(Diagnostic::info(
            DiagCode::DualSoftnessInert,
            "dual_softness is inert unless ink=dual",
        ));
    }
    if !o.resample {
        d.push(
            Diagnostic::warning(
                DiagCode::ResampleOff,
                "resample is off: cells map to the original pixel grid and will alias",
            )
            .with_hint("leave resampling on unless you are matching an existing grid"),
        );
    }
    // Sub-pixel cells: the source cannot describe the cell grid.
    if let Some(g) = geom {
        let px_per_cell = f64::from(g.source_display_px.0) * 25.4 / (g.poster.w.max(1.0) * 25.4)
            * (g.cell_area_mm2.max(1e-9).sqrt());
        if px_per_cell > 0.0 && px_per_cell < 1.0 && o.sampling == crate::modern::options::Sampling::BoxAverage {
            d.push(Diagnostic::warning(
                DiagCode::SubPixelCells,
                format!(
                    "the source provides only {px_per_cell:.2} pixels per cell; fine cells will sample the same pixels repeatedly"
                ),
            ));
        }
    }
    d
}

fn colour(o: &ModernOptions) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    if o.ink == InkMode::DualTone && o.fg_color.distance(o.bg_color) < 48.0 {
        d.push(
            Diagnostic::warning(
                DiagCode::SimilarDualColors,
                format!(
                    "fg {} and bg {} differ by only {:.0} over 765",
                    o.fg_color,
                    o.bg_color,
                    o.fg_color.distance(o.bg_color)
                ),
            )
            .with_hint("two-tone printing needs clearly different inks"),
        );
    }
    if o.ink != InkMode::Fixed && o.ink_color != crate::modern::color::Rgb::BLACK {
        d.push(Diagnostic::info(
            DiagCode::InkOptionInert,
            format!("ink_color is inert while ink={}", o.ink.cli_str()),
        ));
    }
    if o.ink != InkMode::DualTone && (o.fg_color != crate::modern::color::Rgb::BLACK
        || o.bg_color != crate::modern::color::Rgb::WHITE)
    {
        d.push(Diagnostic::info(
            DiagCode::InkOptionInert,
            format!("fg_color and bg_color are inert while ink={}", o.ink.cli_str()),
        ));
    }
    if !o.shading && o.shading_strength != 0.35 {
        d.push(Diagnostic::info(
            DiagCode::ShadingInert,
            "shading_strength is inert while shading is off",
        ));
    }
    if !(0.0..=1.0).contains(&o.shading_strength) {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("shading_strength={} is outside 0..=1", o.shading_strength),
        ));
    }
    if !(0.05..=1.0).contains(&o.dual_softness) {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("dual_softness={} is outside 0.05..=1.0", o.dual_softness),
        ));
    }
    if o.screen == ScreenFamily::Plain && o.ink != InkMode::Grayscale {
        d.push(Diagnostic::info(
            DiagCode::InkOptionInert,
            "the plain screen emits the tone plane as grey; the ink mode has no effect",
        ));
    }
    d
}

fn layout(o: &ModernOptions, geom: Option<&JobGeometry>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let sheet_min = o.paper.w_mm.min(o.paper.h_mm);
    let margin_cap = sheet_min / 2.0 - 10.0;
    if o.margin_mm < 2.0 || o.margin_mm > margin_cap {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("margin_mm={} is outside 2..={margin_cap:.1}", o.margin_mm),
        ));
    }
    if o.overlap_mm < 0.0 || o.overlap_mm > o.margin_mm {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("overlap_mm={} is outside 0..={}", o.overlap_mm, o.margin_mm),
        ));
    }
    if o.bleed_mm < 0.0 || o.bleed_mm > o.margin_mm {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("bleed_mm={} is outside 0..={}", o.bleed_mm, o.margin_mm),
        ));
    }
    if o.overlap_mode == crate::modern::options::OverlapMode::Shared {
        if o.dither != Dither::None {
            d.push(
                Diagnostic::error(
                    DiagCode::SharedOverlapDither,
                    "overlap_mode=shared cannot be combined with a dither",
                )
                .with_hint("the error-diffusion row buffer needs the disjoint cell windows"),
            );
        }
        if o.halo_cells > 0 {
            d.push(
                Diagnostic::warning(
                    DiagCode::SharedOverlapHalo,
                    "overlap_mode=shared with halo_cells>0 draws the shared strip twice",
                )
                .with_hint("set halo_cells=0 for shared overlap"),
            );
        }
    }
    for (name, v, lo, hi) in [
        ("guide_width_pt", o.guide_width_pt, 0.1, 6.0),
        ("corner_mark_mm", o.corner_mark_mm, 1.0, 30.0),
        ("label_size_pt", o.label_size_pt, 4.0, 36.0),
    ] {
        if v < lo || v > hi {
            d.push(Diagnostic::error(
                DiagCode::RangeOutOfBounds,
                format!("{name}={v} is outside {lo}..={hi}"),
            ));
        }
    }
    if o.label_offset_mm < 8.0 || o.label_offset_mm > o.margin_mm {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!(
                "label_offset_mm={} is outside 8..={} (8 mm is the documented floor)",
                o.label_offset_mm, o.margin_mm
            ),
        ));
    }
    if o.guides == GuideStyle::Corners && 2.0 * o.corner_mark_mm > o.margin_mm {
        d.push(
            Diagnostic::warning(
                DiagCode::CornerMarksOffSheet,
                format!(
                    "corner marks of {:.1} mm on each side need a {:.1} mm margin",
                    o.corner_mark_mm,
                    2.0 * o.corner_mark_mm
                ),
            )
            .with_hint("reduce corner_mark_mm or raise margin_mm"),
        );
    }
    if o.guides == GuideStyle::None && o.guide_width_pt != 0.5 {
        d.push(Diagnostic::info(
            DiagCode::GuideOptionInert,
            "guide_width_pt is inert while guides=none",
        ));
    }
    if o.guides != GuideStyle::Corners && o.corner_mark_mm != 4.0 {
        d.push(Diagnostic::info(
            DiagCode::GuideOptionInert,
            "corner_mark_mm is inert unless guides=corners",
        ));
    }
    if o.labels == LabelStyle::None && (o.label_size_pt != 9.0 || o.label_offset_mm != 8.0) {
        d.push(Diagnostic::info(
            DiagCode::GuideOptionInert,
            "label options are inert while labels=none",
        ));
    }
    if o.labels == LabelStyle::Coordinates {
        // The label is drawn inside the cutting margin, so the offset has to
        // clear the text height.
        let text_mm = (o.label_size_pt / crate::modern::geometry::PT_PER_MM).ceil();
        if o.label_offset_mm < text_mm {
            d.push(
                Diagnostic::error(
                    DiagCode::LabelMarginTooSmall,
                    format!(
                        "labels need {:.0} mm of margin, label_offset_mm is {:.1}",
                        text_mm, o.label_offset_mm
                    ),
                )
                .with_hint("raise --label-offset-mm or lower --label-size-pt"),
            );
        }
    }
    // Alignment is inert for a stretch fit, and for a fit that fills the poster.
    if o.fit == Fit::Stretch {
        if o.align_h != Align::Center || o.align_v != Align::Center {
            d.push(Diagnostic::info(
                DiagCode::GuideOptionInert,
                "alignment is inert while fit=stretch",
            ));
        }
    } else if let Some(g) = geom {
        let rw = g.source_rect_mm[2] - g.source_rect_mm[0];
        let rh = g.source_rect_mm[3] - g.source_rect_mm[1];
        let full = (rw - g.poster.w).abs() < 0.5 && (rh - g.poster.h).abs() < 0.5;
        if full && (o.align_h != Align::Center || o.align_v != Align::Center) {
            d.push(Diagnostic::info(
                DiagCode::GuideOptionInert,
                format!("the image already fills the poster, so alignment={}/{} has nothing to align", o.align_h.cli_str(), o.align_v.cli_str()),
            ));
        }
    }
    match o.size_spec {
        crate::modern::options::SizeSpec::WidthSheets(n) | crate::modern::options::SizeSpec::HeightSheets(n)
            if n <= 0.0 =>
        {
            d.push(Diagnostic::error(
                DiagCode::RangeOutOfBounds,
                "the sheet count must be positive",
            ));
        }
        crate::modern::options::SizeSpec::Size { w, h } => {
            let unit = o.size_unit;
            if unit.to_mm(w) <= 0.0 || unit.to_mm(h) <= 0.0 {
                d.push(Diagnostic::error(
                    DiagCode::RangeOutOfBounds,
                    format!("size {w}x{h} {} is empty", unit.suffix()),
                ));
            }
        }
        _ => {}
    }
    d
}

fn output(o: &ModernOptions, geom: Option<&JobGeometry>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let vector = geom.map_or(o.render == RenderMode::Vector, |g| g.render_mode == RenderMode::Vector);
    if vector && (o.dpi != crate::modern::options::Dpi::P300 || o.jpeg_quality != 85 || o.compress) {
        d.push(
            Diagnostic::warning(
                DiagCode::OutputInertVector,
                "dpi, jpeg_quality and compress are inert on the vector render path",
            )
            .with_hint("they only affect --render raster"),
        );
    }
    if o.jpeg_quality <= 24 {
        d.push(
            Diagnostic::warning(
                DiagCode::JpegQualityLow,
                format!("jpeg_quality={} will show visible compression artefacts", o.jpeg_quality),
            )
            .with_hint("85 is a good default for a halftone page"),
        );
    }
    if o.compress && (o.dpi != crate::modern::options::Dpi::P150 || o.jpeg_quality > 60) {
        d.push(
            Diagnostic::warning(
                DiagCode::CompressionCapped,
                format!(
                    "compress caps dpi at 150 and quality at 60 (effective {} / {})",
                    if o.compress { o.dpi.value().min(150) } else { o.dpi.value() },
                    if o.compress { o.jpeg_quality.min(60) } else { o.jpeg_quality }
                ),
            )
            .with_hint("that is the trade-off the flag names"),
        );
    }
    if o.creation_date == CreationDate::Now {
        d.push(
            Diagnostic::warning(
                DiagCode::NonDeterministicDate,
                "creation_date=now writes the current time, so two runs differ",
            )
            .with_hint("leave it at none for byte-identical output"),
        );
    }
    if o.title.len() > 256 || !o.title.is_ascii() || o.title.contains(['(', ')', '\\']) {
        d.push(
            Diagnostic::error(
                DiagCode::TitleInvalid,
                "title must be at most 256 ASCII bytes without '(' ')' or '\\'",
            )
            .with_hint("the title is written as a PDF literal string"),
        );
    }
    if !o.creator.is_ascii() {
        d.push(Diagnostic::error(
            DiagCode::CreatorInvalid,
            "creator must be ASCII",
        ));
    }
    d
}

fn input(o: &ModernOptions, src: Option<&SourceInfo>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    for (name, v, lo, hi) in [
        ("brightness", o.brightness, -100.0, 100.0),
        ("contrast", o.contrast, -100.0, 100.0),
        ("saturation", o.saturation, -100.0, 100.0),
    ] {
        if v < lo || v > hi {
            d.push(Diagnostic::error(
                DiagCode::RangeOutOfBounds,
                format!("{name}={v} is outside {lo}..={hi}"),
            ));
        }
    }
    if !(72..=1200).contains(&o.resample_dpi) {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("resample_dpi={} is outside 72..=1200", o.resample_dpi),
        ));
    }
    if !matches!(o.rotate, Rotation::D0 | Rotation::D90 | Rotation::D180 | Rotation::D270) {
        d.push(Diagnostic::error(
            DiagCode::RangeOutOfBounds,
            format!("rotate must be 0, 90, 180 or 270"),
        ));
    }
    if let Some(c) = o.crop {
        if c.x2 <= c.x1 || c.y2 <= c.y1 {
            d.push(
                Diagnostic::error(
                    DiagCode::CropInvalid,
                    format!(
                        "crop {} has x2<=x1 or y2<=y1",
                        c.to_token()
                    ),
                )
                .with_hint("the rectangle is [x1,y1,x2,y2] with the far corner exclusive"),
            );
        } else if let Some(s) = src {
            let (w, h) = (i64::from(s.width), i64::from(s.height));
            if c.x1 < 0 || c.y1 < 0 || c.x2 > w || c.y2 > h {
                d.push(
                    Diagnostic::error(
                        DiagCode::CropInvalid,
                        format!("crop {} leaves the {w}x{h} source", c.to_token()),
                    )
                    .with_hint("crop coordinates are source pixels of the decoded image"),
                );
            }
        }
    }
    if o.exif_orientation == crate::modern::options::ExifOrientation::Apply
        && !o.rotate.swaps_axes()
        && src.is_some_and(|s| !s.exif_swaps_axes)
        && o.crop.is_none()
    {
        d.push(Diagnostic::info(
            DiagCode::ExifNoOrientation,
            "this source carries no EXIF orientation, so the setting has no effect",
        ));
    }
    d
}

fn ux(o: &ModernOptions) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    for (name, v) in [
        ("preset", o.preset.as_deref()),
        ("save_preset", o.save_preset.as_deref()),
        ("delete_preset", o.delete_preset.as_deref()),
    ] {
        if let Some(n) = v {
            if !crate::modern::options::is_portable_name(n) {
                d.push(
                    Diagnostic::error(
                        DiagCode::PresetNameInvalid,
                        format!("{name} '{n}' is not a portable preset name"),
                    )
                    .with_hint("use 1 to 48 characters from A-Z, a-z, 0-9, '_' or '-'"),
                );
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modern::diagnostics::Severity;
    use crate::modern::options::*;

    fn defaults() -> ModernOptions {
        ModernOptions::default()
    }

    fn src() -> SourceInfo {
        SourceInfo::synthetic(4000, 3000)
    }

    fn geom(o: &ModernOptions) -> JobGeometry {
        JobGeometry::solve(o, &src()).expect("valid geometry")
    }

    /// Every row of the interaction matrix.
    #[test]
    fn interaction_matrix_rows() {
        // Plain reports its inert options in one combined diagnostic.
        let mut o = defaults();
        o.screen = ScreenFamily::Plain;
        o.pattern = Pattern::Triangle;
        o.stagger = Stagger::HalfPitch;
        let r = validate(&o, None, None);
        assert!(r.has(DiagCode::ScreenPlainInert, Severity::Warning));
        assert!(!r.has_blocking(), "plain alone is legal: {:?}", r);

        // Stagger needs the lattice.
        let mut o = defaults();
        o.screen = ScreenFamily::Hex;
        o.stagger = Stagger::HalfPitch;
        assert!(validate(&o, None, None).has(DiagCode::StaggerNeedsLattice, Severity::Error));

        // Hex rejects mosaic and the line patterns.
        let mut o = defaults();
        o.screen = ScreenFamily::Hex;
        o.pattern = Pattern::Mosaic;
        assert!(validate(&o, None, None).has(DiagCode::PatternNotSupported, Severity::Error));
        for p in [Pattern::LineH, Pattern::LineV, Pattern::LineMorph] {
            o.pattern = p;
            assert!(
                validate(&o, None, None).has(DiagCode::PatternNotSupported, Severity::Error),
                "{p:?}"
            );
        }
        // ... and accepts the four legal ones.
        for p in [Pattern::Dot, Pattern::Square, Pattern::Triangle, Pattern::TriangleAlt] {
            o.pattern = p;
            assert!(!validate(&o, None, None).has_blocking(), "{p:?}");
        }

        // Vector + plain is blocking.
        let mut o = defaults();
        o.screen = ScreenFamily::Plain;
        o.render = RenderMode::Vector;
        assert!(validate(&o, None, None).has(DiagCode::PlainVectorBlocked, Severity::Error));

        // Mosaic contrast and line aspect need their pattern.
        let mut o = defaults();
        o.mosaic_contrast = 2.0;
        assert!(validate(&o, None, None).has(DiagCode::MosaicContrastNeedsMosaic, Severity::Error));
        o.pattern = Pattern::Mosaic;
        assert!(!validate(&o, None, None).has_blocking());
        let mut o = defaults();
        o.line_aspect = 0.25;
        assert!(validate(&o, None, None).has(DiagCode::LineAspectNeedsLine, Severity::Error));
        o.pattern = Pattern::LineH;
        assert!(!validate(&o, None, None).has_blocking());

        // Inverted tone band.
        let mut o = defaults();
        o.min_raster = 0.8;
        o.max_raster = 0.2;
        assert!(validate(&o, None, None).has(DiagCode::ToneBandInverted, Severity::Error));

        // Inert dither settings are info, not errors.
        let mut o = defaults();
        o.dither_strength = 0.5;
        let r = validate(&o, None, None);
        assert!(r.has(DiagCode::DitherInert, Severity::Info));
        assert!(!r.has_blocking());

        // Resample off and halo zero are warnings.
        let mut o = defaults();
        o.resample = false;
        o.halo_cells = 0;
        let r = validate(&o, None, None);
        assert!(r.has(DiagCode::ResampleOff, Severity::Warning));
        assert!(r.has(DiagCode::HaloCellsZero, Severity::Warning));
        assert!(!r.has_blocking());

        // Fine screen warns.
        let mut o = defaults();
        o.nominal_dot_mm = 2.0;
        assert!(validate(&o, None, None).has(DiagCode::FineScreen, Severity::Warning));
        o.nominal_dot_mm = 0.5;
        assert!(validate(&o, None, None).has_blocking(), "below 1 mm is out of range");
    }

    #[test]
    fn shared_overlap_rules() {
        let mut o = defaults();
        o.overlap_mode = OverlapMode::Shared;
        o.overlap_mm = 5.0;
        o.dither = Dither::FloydSteinberg;
        assert!(validate(&o, None, None).has(DiagCode::SharedOverlapDither, Severity::Error));
        o.dither = Dither::None;
        o.halo_cells = 1;
        assert!(validate(&o, None, None).has(DiagCode::SharedOverlapHalo, Severity::Warning));
        o.halo_cells = 0;
        assert!(!validate(&o, None, None).has_blocking());
    }

    #[test]
    fn page_limits_and_partial_edges() {
        let mut o = defaults();
        o.size_spec = SizeSpec::WidthSheets(5.0);
        o.page_limit = 4;
        let g = JobGeometry::solve(&o, &src()).unwrap_err();
        assert!(g.has(DiagCode::PageLimitExceeded, Severity::Error));
        o.page_limit = 0;
        let gg = geom(&o);
        o.page_warn = 4;
        let r = validate(&o, Some(&src()), Some(&gg));
        assert!(r.has(DiagCode::PageWarnExceeded, Severity::Warning));
        assert!(r.has(DiagCode::PagePartialEdge, Severity::Warning));
    }

    #[test]
    fn crop_is_checked_against_the_source() {
        let mut o = defaults();
        o.crop = Some(CropRectPx { x1: 10, y1: 10, x2: 5, y2: 20 });
        assert!(validate(&o, Some(&src()), None).has(DiagCode::CropInvalid, Severity::Error));
        o.crop = Some(CropRectPx { x1: 0, y1: 0, x2: 999_999, y2: 10 });
        assert!(validate(&o, Some(&src()), None).has(DiagCode::CropInvalid, Severity::Error));
        // Without a source the bounds check cannot run; only the shape rule is checkable.
        assert!(!validate(&o, None, None).has_blocking());
        o.crop = Some(CropRectPx { x1: 0, y1: 0, x2: 100, y2: 100 });
        assert!(!validate(&o, Some(&src()), None).has_blocking());
    }

    #[test]
    fn label_margin_rule() {
        let mut o = defaults();
        o.labels = LabelStyle::Coordinates;
        o.margin_mm = 10.0;
        o.label_offset_mm = 8.0;
        o.label_size_pt = 9.0;
        assert!(!validate(&o, None, None).has_blocking(), "the defaults must fit");
        // 36 pt is 13 mm of text, which no legal offset can clear.
        o.label_size_pt = 36.0;
        assert!(validate(&o, None, None).has(DiagCode::LabelMarginTooSmall, Severity::Error));
        o.label_size_pt = 9.0;
        o.label_offset_mm = 7.0;
        assert!(validate(&o, None, None).has(DiagCode::RangeOutOfBounds, Severity::Error));
    }

    #[test]
    fn title_and_creator_rules() {
        let mut o = defaults();
        o.title = "a (bad) title".into();
        assert!(validate(&o, None, None).has(DiagCode::TitleInvalid, Severity::Error));
        o.title = "x".repeat(257);
        assert!(validate(&o, None, None).has(DiagCode::TitleInvalid, Severity::Error));
        o.title = "fine".into();
        o.creator = "caf\u{e9}".into();
        assert!(validate(&o, None, None).has(DiagCode::CreatorInvalid, Severity::Error));
    }

    #[test]
    fn output_warnings() {
        let mut o = defaults();
        o.creation_date = CreationDate::Now;
        assert!(validate(&o, None, None).has(DiagCode::NonDeterministicDate, Severity::Warning));
        o.creation_date = CreationDate::None;
        o.jpeg_quality = 10;
        assert!(validate(&o, None, None).has(DiagCode::JpegQualityLow, Severity::Warning));
        o.jpeg_quality = 85;
        o.compress = true;
        assert!(validate(&o, None, None).has(DiagCode::CompressionCapped, Severity::Warning));
    }

    #[test]
    fn strict_promotes_and_is_idempotent() {
        let mut o = defaults();
        o.resample = false;
        let plain = validate(&o, None, None);
        assert!(!plain.has_blocking());
        o.strict = true;
        let strict = validate(&o, None, None);
        assert!(strict.has_blocking());
        assert_eq!(strict, validate(&o, None, None), "validation must be idempotent");
    }

    #[test]
    fn report_order_is_stable() {
        let mut o = defaults();
        o.resample = false;
        o.halo_cells = 0;
        o.jpeg_quality = 5;
        o.nominal_dot_mm = 2.0;
        let a = validate(&o, None, None);
        let b = validate(&o, None, None);
        assert_eq!(a, b);
        let sev: Vec<Severity> = a.items().iter().map(|d| d.severity).collect();
        let mut sorted = sev.clone();
        sorted.sort();
        assert_eq!(sev, sorted, "report must be sorted by severity");
    }

    #[test]
    fn every_option_is_covered_by_a_range_rule_or_a_value_rule() {
        // An out-of-range numeric value must produce a blocking error for
        // every numeric option, driven by the option key table.
        let numeric: &[(&str, fn(&mut ModernOptions, f64))] = &[
            ("nominal_dot_mm", |o, v| o.nominal_dot_mm = v),
            ("min_raster", |o, v| o.min_raster = v),
            ("max_raster", |o, v| o.max_raster = v),
            ("tone_exponent", |o, v| o.tone_exponent = v),
            ("tone_log_k", |o, v| o.tone_log_k = v),
            ("dither_strength", |o, v| o.dither_strength = v),
            ("line_aspect", |o, v| o.line_aspect = v),
            ("mosaic_contrast", |o, v| o.mosaic_contrast = v),
            ("dual_softness", |o, v| o.dual_softness = v),
            ("shading_strength", |o, v| o.shading_strength = v),
            ("margin_mm", |o, v| o.margin_mm = v),
            ("overlap_mm", |o, v| o.overlap_mm = v),
            ("bleed_mm", |o, v| o.bleed_mm = v),
            ("guide_width_pt", |o, v| o.guide_width_pt = v),
            ("corner_mark_mm", |o, v| o.corner_mark_mm = v),
            ("label_size_pt", |o, v| o.label_size_pt = v),
            ("label_offset_mm", |o, v| o.label_offset_mm = v),
            ("brightness", |o, v| o.brightness = v),
            ("contrast", |o, v| o.contrast = v),
            ("saturation", |o, v| o.saturation = v),
            ("resample_dpi", |o, v| o.resample_dpi = v as u16),
        ];
        for (name, set) in numeric {
            let mut o = defaults();
            set(&mut o, 1e9);
            let r = validate(&o, None, None);
            assert!(
                r.has(DiagCode::RangeOutOfBounds, Severity::Error)
                    || r.has_blocking()
                        && r.items().iter().any(|d| d.severity == Severity::Error),
                "option {name} is not range-checked"
            );
        }
        for name in ["halo_cells", "dither_levels", "dither_block"] {
            assert!(ModernOptions::option_names().contains(&name));
        }
    }

    #[test]
    fn enum_options_are_parsed_from_the_canonical_tables() {
        // Every option key appears in `to_text`, in `option_names` order, and
        // every value round-trips (string escaping is covered by the option
        // module's own tests).
        let o = ModernOptions::default();
        let text = o.to_text();
        let keys: Vec<&str> = text.lines().skip(1).filter_map(|l| l.split_once('=').map(|(k, _)| k)).collect();
        assert_eq!(keys, ModernOptions::option_names());
        assert_eq!(ModernOptions::from_text(&text).unwrap(), o);
    }
    #[test]
    fn unknown_theme_is_blocking() {
        let mut o = defaults();
        o.theme = "no-such-theme".into();
        let r = validate_with_themes(&o, None, None, &["black", "sepia"]);
        assert!(r.has(DiagCode::ThemeUnknown, Severity::Error));
        o.theme = "black".into();
        assert!(!validate_with_themes(&o, None, None, &["black", "sepia"]).has_blocking());
        // With no explicit list the built-in table is consulted.
        assert!(!validate_with_themes(&o, None, None, &[]).has_blocking());
    }

    #[test]
    fn validate_matches_geometry_solve_for_page_limits() {
        let mut o = defaults();
        o.page_limit = 2;
        let s = src();
        // solve() rejects; validate() with a geometry agrees.
        assert!(JobGeometry::solve(&o, &s).is_err());
        let relaxed = {
            let mut p = o.clone();
            p.page_limit = 0;
            let g = geom(&p);
            validate(&p, Some(&s), Some(&g))
        };
        assert!(!relaxed.has(DiagCode::PageLimitExceeded, Severity::Error));
    }

    #[test]
    fn default_options_are_error_free() {
        let o = defaults();
        let s = src();
        let g = JobGeometry::solve(&o, &s).unwrap();
        let r = validate(&o, Some(&s), Some(&g));
        assert!(!r.has_blocking(), "defaults must be renderable: {r}");
    }
}