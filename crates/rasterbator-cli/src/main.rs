//! Command line interface for the Rasterbator port.
//!
//! Two engines share one binary:
//!   * `--mode legacy` (the default) runs the original wizard-equivalent path
//!     verbatim, producing byte-identical output to the pre-modern CLI.
//!   * `--mode modern` runs the modern engine (`ModernOptions` -> `JobGeometry`
//!     -> `RenderSession` -> `ModernPdf`) with the full option surface.
//!
//! Every [`ModernOptions`] field has exactly one flag (see `build_options`), so
//! a new option cannot be added to the model without a mapping here.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use clap::{ArgAction, Parser, ValueEnum};
use rasterbator_core::{
    load_rgba, paper_preset, rasterbate, resolve_output_size, ColorMode, CropRect, EngineOptions,
    PdfDocument, SizeSpec, PAPER_PRESETS,
};

use rasterbator_core::modern::{
    validate, CropRectPx, Diagnostic, JobGeometry, ModernOptions, ModernPdf, PaperSpec,
    PresetStore, Progress, ProgressMode, RenderError, RenderPhase, RenderSession,
    Rgb, Severity, SourceInfo, ValidationReport,
};
use rasterbator_core::modern::pdf::PdfTarget;
use rasterbator_core::modern::{
    Align, Coverage, CreationDate, Dither, Dpi, ExifOrientation, Fit, GuideStyle, InkMode,
    LabelStyle, Orientation, OverlapMode, Pattern, Rotation, Sampling, ScreenFamily,
    SizeSpec as ModernSizeSpec, SizeUnit, Stagger, ToneCurve,
};
use rasterbator_core::modern::options::PrintScaling;
use rasterbator_core::modern::prepare::{prepare_image, resample_target_for, PrepareOptions};
use rasterbator_core::modern::source::{decode_rgba, inspect};

/// Which engine to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// Original wizard-equivalent path (byte-identical output).
    Legacy,
    /// Full modern option surface.
    Modern,
}

/// Turn an image into a printable multi-page poster of halftone dots (PDF).
///
/// A port of The Rasterbator: the poster is split over as many paper sheets
/// as needed and every sheet is filled with a grid of dots whose size
/// follows the brightness of the source image.
#[derive(Parser, Debug)]
#[command(name = "rasterbator", version, about, max_term_width = 110)]
#[command(after_help = "EXAMPLES:
  rasterbator photo.jpg                          A4 portrait, 5 sheets wide, black 10 mm dots
  rasterbator photo.jpg -w 10 --dot-size 15      ten sheets wide, 15 mm dots
  rasterbator photo.jpg --paper a3 --height-sheets 4    A3, four sheets high
  rasterbator --mode modern photo.jpg            modern engine, same A4 defaults
  rasterbator --mode modern photo.jpg --screen hex --pattern line-morph --dither atkinson
  rasterbator --mode modern photo.jpg --paper a3 --landscape --width-sheets 4 --overlap-mm 10")]
struct Cli {
    /// Source image (PNG, JPEG, GIF, BMP, TIFF, WebP)
    input: PathBuf,

    /// Engine to use
    #[arg(long, value_enum, default_value_t = Mode::Legacy, value_name = "legacy|modern")]
    mode: Mode,

    // ----- Mode and output -------------------------------------------------
    /// Output PDF path (default: input file name with .pdf; a numbered name
    /// is used if the file already exists)
    #[arg(short, long, help_heading = "Mode and output")]
    output: Option<PathBuf>,

    /// Suppress progress output
    #[arg(short, long, action = ArgAction::SetTrue, help_heading = "Mode and output")]
    quiet: bool,

    /// Progress reporting: auto (tty-aware), off, tty or plain
    #[arg(long, value_name = "auto|off|tty|plain", help_heading = "Mode and output")]
    progress: Option<String>,

    /// Write a machine-readable JSON summary to stdout
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Mode and output")]
    print_summary: bool,

    /// Treat modern validation warnings as errors (modern mode)
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Mode and output")]
    strict: bool,

    // ----- Paper and size --------------------------------------------------
    /// Paper size: a4, a3, letter, legal, or custom WIDTHxHEIGHT in mm
    #[arg(long, value_name = "PAPER", help_heading = "Paper and size")]
    paper: Option<String>,

    /// Print on landscape sheets (modern: sets the orientation)
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Paper and size")]
    landscape: bool,

    /// Poster width in sheets (the height follows from the image aspect)
    #[arg(short = 'w', long, value_name = "N", help_heading = "Paper and size")]
    width_sheets: Option<f64>,

    /// Poster height in sheets (overrides --width-sheets)
    #[arg(long, value_name = "N", help_heading = "Paper and size")]
    height_sheets: Option<f64>,

    /// Exact poster size WIDTHxHEIGHT in mm (overrides the sheet options)
    #[arg(short = 's', long, value_name = "WxH", help_heading = "Paper and size")]
    size: Option<String>,

    /// Unit for --size in modern mode: mm, cm or in
    #[arg(long, value_name = "mm|cm|in", help_heading = "Paper and size")]
    size_unit: Option<String>,

    /// How the source fills the poster: contain, cover or stretch
    #[arg(long, value_name = "contain|cover|stretch", help_heading = "Paper and size")]
    fit: Option<String>,

    /// Horizontal placement: start, center or end
    #[arg(long, value_name = "start|center|end", help_heading = "Paper and size")]
    align_h: Option<String>,

    /// Vertical placement: start, center or end
    #[arg(long, value_name = "start|center|end", help_heading = "Paper and size")]
    align_v: Option<String>,

    // ----- Screen and pattern ---------------------------------------------
    /// Dot screen family: lattice, hex or plain
    #[arg(long, value_name = "lattice|hex|plain", help_heading = "Screen and pattern")]
    screen: Option<String>,

    /// Stagger every other row: off or half-pitch
    #[arg(long, value_name = "off|half-pitch", help_heading = "Screen and pattern")]
    stagger: Option<String>,

    /// Cells of halo drawn around the visible window
    #[arg(long, value_name = "N", help_heading = "Screen and pattern")]
    halo_cells: Option<u32>,

    /// Cell pattern: dot, square, triangle, triangle-alt, line-h, line-v,
    /// line-morph or mosaic
    #[arg(long, value_name = "PATTERN", help_heading = "Screen and pattern")]
    pattern: Option<String>,

    /// Maximum dot size in mm
    #[arg(short = 'd', long, value_name = "MM", help_heading = "Screen and pattern")]
    dot_size: Option<f64>,

    /// Cell coverage: full-cell or tangent
    #[arg(long, value_name = "full-cell|tangent", help_heading = "Screen and pattern")]
    coverage: Option<String>,

    /// Grid phase offset X,Y in mm
    #[arg(long, value_name = "X,Y", allow_hyphen_values = true, help_heading = "Screen and pattern")]
    grid_phase: Option<String>,

    // ----- Tone and sampling ----------------------------------------------
    /// Raster tone band minimum (0..=1)
    #[arg(long, value_name = "F", help_heading = "Tone and sampling")]
    min_raster: Option<f64>,

    /// Raster tone band maximum (0..=1)
    #[arg(long, value_name = "F", help_heading = "Tone and sampling")]
    max_raster: Option<f64>,

    /// Tone curve: none, exp or log
    #[arg(long, value_name = "none|exp|log", help_heading = "Tone and sampling")]
    tone: Option<String>,

    /// Tone exponent for --tone exp
    #[arg(long, value_name = "F", help_heading = "Tone and sampling")]
    tone_exponent: Option<f64>,

    /// Tone log factor for --tone log
    #[arg(long, value_name = "K", help_heading = "Tone and sampling")]
    tone_log_k: Option<f64>,

    /// Invert the tone transfer
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Tone and sampling")]
    invert_tone: bool,

    /// Cell sampling: box, nearest or bilinear
    #[arg(long, value_name = "box|nearest|bilinear", help_heading = "Tone and sampling")]
    sampling: Option<String>,

    /// Dither algorithm: none, fs, atkinson or bayer4x4
    #[arg(long, value_name = "none|fs|atkinson|bayer4x4", help_heading = "Tone and sampling")]
    dither: Option<String>,

    /// Dither strength (0..=1)
    #[arg(long, value_name = "F", help_heading = "Tone and sampling")]
    dither_strength: Option<f64>,

    /// Dither quantisation levels (2..=64)
    #[arg(long, value_name = "N", help_heading = "Tone and sampling")]
    dither_levels: Option<u32>,

    /// Dither error-diffusion block size (1..=8)
    #[arg(long, value_name = "N", help_heading = "Tone and sampling")]
    dither_block: Option<u32>,

    /// Aspect ratio for line patterns (0.05..=1.0)
    #[arg(long, value_name = "F", help_heading = "Tone and sampling")]
    line_aspect: Option<f64>,

    /// Mosaic contrast multiplier (0.1..=8.0)
    #[arg(long, value_name = "F", help_heading = "Tone and sampling")]
    mosaic_contrast: Option<f64>,

    // ----- Colour ----------------------------------------------------------
    /// Ink mode / colour: black, RRGGBB, fixed:RRGGBB, average, grayscale
    /// or dual:FG/BG
    #[arg(short = 'c', long, value_name = "COLOR", help_heading = "Colour")]
    color: Option<String>,

    /// Fixed ink colour RRGGBB
    #[arg(long, value_name = "RRGGBB", help_heading = "Colour")]
    ink_color: Option<String>,

    /// Dual-tone foreground colour RRGGBB
    #[arg(long, value_name = "RRGGBB", help_heading = "Colour")]
    fg_color: Option<String>,

    /// Dual-tone background colour RRGGBB
    #[arg(long, value_name = "RRGGBB", help_heading = "Colour")]
    bg_color: Option<String>,

    /// Paper / background colour RRGGBB
    #[arg(long, value_name = "RRGGBB", help_heading = "Colour")]
    paper_color: Option<String>,

    /// Built-in theme name
    #[arg(long, value_name = "NAME", help_heading = "Colour")]
    theme: Option<String>,

    /// Enable directional shading
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Colour")]
    shading: bool,

    /// Shading strength (0..=1)
    #[arg(long, value_name = "F", help_heading = "Colour")]
    shading_strength: Option<f64>,

    /// Invert all colours
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Colour")]
    invert_colors: bool,

    /// Dual-tone quantisation softness (0.05..=1.0)
    #[arg(long, value_name = "F", help_heading = "Colour")]
    dual_softness: Option<f64>,

    // ----- Layout and guides ----------------------------------------------
    /// Page margin in mm
    #[arg(long, value_name = "MM", help_heading = "Layout and guides")]
    margin_mm: Option<f64>,

    /// Sheet overlap in mm
    #[arg(long, value_name = "MM", help_heading = "Layout and guides")]
    overlap_mm: Option<f64>,

    /// Overlap semantics: margin or shared
    #[arg(long, value_name = "margin|shared", help_heading = "Layout and guides")]
    overlap_mode: Option<String>,

    /// Bleed in mm
    #[arg(long, value_name = "MM", help_heading = "Layout and guides")]
    bleed_mm: Option<f64>,

    /// Alignment guides: none, border or corners
    #[arg(long, value_name = "none|border|corners", help_heading = "Layout and guides")]
    guides: Option<String>,

    /// Guide stroke width in pt
    #[arg(long, value_name = "PT", help_heading = "Layout and guides")]
    guide_width_pt: Option<f64>,

    /// Corner mark length in mm
    #[arg(long, value_name = "MM", help_heading = "Layout and guides")]
    corner_mark_mm: Option<f64>,

    /// Guide colour RRGGBB
    #[arg(long, value_name = "RRGGBB", help_heading = "Layout and guides")]
    guide_color: Option<String>,

    /// Page labels: none or coordinates
    #[arg(long, value_name = "none|coordinates", help_heading = "Layout and guides")]
    labels: Option<String>,

    /// Label size in pt
    #[arg(long, value_name = "PT", help_heading = "Layout and guides")]
    label_size_pt: Option<f64>,

    /// Label offset in mm
    #[arg(long, value_name = "MM", help_heading = "Layout and guides")]
    label_offset_mm: Option<f64>,

    /// Warn when the poster exceeds this many pages
    #[arg(long, value_name = "N", help_heading = "Layout and guides")]
    page_warn: Option<u32>,

    /// Block when the poster exceeds this many pages (0 = unlimited)
    #[arg(long, value_name = "N", help_heading = "Layout and guides")]
    page_limit: Option<u32>,

    // ----- Input preparation ----------------------------------------------
    /// Use only this part of the source image: X1,Y1,X2,Y2 in pixels
    #[arg(long, value_name = "X1,Y1,X2,Y2", allow_hyphen_values = true, help_heading = "Input preparation")]
    crop: Option<String>,

    /// Rotate the source: 0, 90, 180 or 270 degrees
    #[arg(long, value_name = "0|90|180|270", help_heading = "Input preparation")]
    rotate: Option<String>,

    /// EXIF orientation handling: apply or ignore
    #[arg(long, value_name = "apply|ignore", help_heading = "Input preparation")]
    exif_orientation: Option<String>,

    /// Brightness adjustment (-100..=100)
    #[arg(long, value_name = "F", allow_hyphen_values = true, help_heading = "Input preparation")]
    brightness: Option<f64>,

    /// Contrast adjustment (-100..=100)
    #[arg(long, value_name = "F", allow_hyphen_values = true, help_heading = "Input preparation")]
    contrast: Option<f64>,

    /// Saturation adjustment (-100..=100)
    #[arg(long, value_name = "F", allow_hyphen_values = true, help_heading = "Input preparation")]
    saturation: Option<f64>,

    /// Alpha cut-off (0..=255); pixels below become paper
    #[arg(long, value_name = "N", help_heading = "Input preparation")]
    alpha_threshold: Option<u8>,

    /// Disable source resampling
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Input preparation")]
    no_resample: bool,

    /// Resample target DPI
    #[arg(long, value_name = "N", help_heading = "Input preparation")]
    resample_dpi: Option<u16>,

    // ----- Output ----------------------------------------------------------
    /// Render mode: auto, vector or raster
    #[arg(long, value_name = "auto|vector|raster", help_heading = "Output")]
    render: Option<String>,

    /// Raster DPI: 150, 200 or 300
    #[arg(long, value_name = "150|200|300", help_heading = "Output")]
    dpi: Option<String>,

    /// Raster tile JPEG quality (0..=100)
    #[arg(long, value_name = "N", help_heading = "Output")]
    jpeg_quality: Option<u8>,

    /// Force compressed/lean raster output
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Output")]
    compress: bool,

    /// PDF title metadata
    #[arg(long, value_name = "TEXT", help_heading = "Output")]
    title: Option<String>,

    /// PDF creator metadata
    #[arg(long, value_name = "TEXT", help_heading = "Output")]
    creator: Option<String>,

    /// PDF creation date: none or now
    #[arg(long, value_name = "none|now", help_heading = "Output")]
    creation_date: Option<String>,

    /// PDF viewer print scaling: none (100 %) or viewer
    #[arg(long, value_name = "none|viewer", help_heading = "Output")]
    print_scaling: Option<String>,

    /// Disable atomic temp-file-then-rename saving
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Output")]
    no_atomic: bool,

    /// Cell count above which auto render switches to raster (0 = never)
    #[arg(long, value_name = "N", help_heading = "Output")]
    auto_raster_cells: Option<u32>,

    // ----- Presets ---------------------------------------------------------
    /// Load a saved preset as the base option set
    #[arg(long, value_name = "NAME", help_heading = "Presets")]
    preset: Option<String>,

    /// Save the effective options under NAME, then exit
    #[arg(long, value_name = "NAME", help_heading = "Presets")]
    save_preset: Option<String>,

    /// Delete the preset NAME, then exit
    #[arg(long, value_name = "NAME", help_heading = "Presets")]
    delete_preset: Option<String>,

    /// List saved presets, then exit
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Presets")]
    list_presets: bool,

    /// Preset directory
    #[arg(long, value_name = "PATH", help_heading = "Presets")]
    preset_dir: Option<PathBuf>,

    // ----- Legacy-only -----------------------------------------------------
    /// Draw the cutting guide rectangle around the rasterbated area (legacy)
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Legacy (ignored in modern mode)")]
    cutout: bool,

    // ----- Development hooks (hidden) -------------------------------------
    /// Print the effective modern options and exit without rendering
    #[arg(long, hide = true)]
    dump_options: bool,

    /// Emit uncompressed PDF content streams (modern, debugging)
    #[arg(long, hide = true)]
    raw_content: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // The modern-only development hooks (`--dump-options`, `--raw-content`)
    // imply the modern engine; otherwise they would be silently ignored and a
    // legacy render would run instead.
    let mode = if cli.dump_options || cli.raw_content {
        Mode::Modern
    } else {
        cli.mode
    };
    match mode {
        Mode::Legacy => match run(&cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("rasterbator: {message}");
                ExitCode::FAILURE
            }
        },
        Mode::Modern => run_modern(&cli),
    }
}

// ---------------------------------------------------------------------------
// Legacy path (must stay byte-identical to the pre-modern CLI).
// ---------------------------------------------------------------------------

fn run(cli: &Cli) -> Result<(), String> {
    // --- wizard step 1: source image ---
    if !cli.input.exists() {
        return Err(format!("cannot find file {}", cli.input.display()));
    }
    let image = load_rgba(&cli.input).map_err(|e| format!("cannot read {}: {e}", cli.input.display()))?;

    // --- wizard step 2: paper size ---
    let paper = cli.paper.as_deref().unwrap_or("a4");
    let (mut paper_w, mut paper_h) = parse_paper(paper)?;
    if cli.landscape {
        std::mem::swap(&mut paper_w, &mut paper_h);
    }

    // --- wizard step 3: output size ---
    let spec = match &cli.size {
        Some(s) => {
            let (w, h) = parse_size(s)?;
            SizeSpec::SizeMm { width: w, height: h }
        }
        None => match cli.height_sheets {
            Some(n) => SizeSpec::HeightSheets(n),
            None => SizeSpec::WidthSheets(cli.width_sheets.unwrap_or(5.0)),
        },
    };
    let resolved = resolve_output_size(
        spec,
        paper_w,
        paper_h,
        image.width() as f64,
        image.height() as f64,
    );

    // --- wizard step 4: rasterbation options ---
    let dot_size = cli.dot_size.unwrap_or(10.0);
    let color = ColorMode::parse(cli.color.as_deref().unwrap_or("black"))?;
    if dot_size < 3.0 {
        eprintln!(
            "warning: a dot size below 3 mm ({} mm) produces very fine dots; \
             large posters may take long and be hard to print",
            dot_size
        );
    }
    let crop = match &cli.crop {
        Some(s) => {
            let v = parse_numbers(s, 4)?;
            Some(CropRect { x1: v[0], y1: v[1], x2: v[2], y2: v[3] })
        }
        None => None,
    };

    let opts = EngineOptions {
        paper_width_mm: paper_w,
        paper_height_mm: paper_h,
        image_width_mm: resolved.width_mm,
        image_height_mm: resolved.height_mm,
        dot_size_mm: dot_size,
        color,
        cutout: cli.cutout,
        crop,
    };

    let mut pdf = PdfDocument::from_paper_mm(paper_w, paper_h);

    // --- wizard step 5: message before rasterbating ---
    let (cm_w, cm_h) = (resolved.width_mm / 10.0, resolved.height_mm / 10.0);
    if !cli.quiet {
        eprintln!(
            "rasterbating: {} -> {} sheet(s) wide, {:.0} x {:.0} mm paper, \
             poster {:.0} x {:.0} cm",
            cli.input.display(),
            resolved.sheets_x,
            paper_w,
            paper_h,
            cm_w,
            cm_h
        );
    }

    let quiet = cli.quiet;
    let mut progress = |done: u32, total: u32| {
        if !quiet {
            eprint!("\rpage {done}/{total}");
        }
    };
    let summary = rasterbate(&image, &opts, &mut pdf, &mut progress);
    if !cli.quiet {
        eprintln!();
    }

    // --- save ---
    let output = match &cli.output {
        Some(path) => path.clone(),
        None => default_output_path(&cli.input),
    };
    pdf.save(&output)
        .map_err(|e| format!("cannot write {}: {e}", output.display()))?;

    if !cli.quiet {
        eprintln!(
            "wrote {}: {} page(s) ({} x {} sheets), dot size {} mm",
            output.display(),
            summary.total_pages,
            summary.pages_x,
            summary.pages_y,
            dot_size
        );
    }
    Ok(())
}

fn parse_paper(spec: &str) -> Result<(f64, f64), String> {
    if let Some(preset) = paper_preset(spec) {
        return Ok((preset.width_mm, preset.height_mm));
    }
    let (w, h) = parse_size(spec)?;
    if w < 50.0 || h < 50.0 || w > 10000.0 || h > 10000.0 {
        return Err(format!(
            "paper size {w} x {h} mm out of range (50..10000 mm); presets: {}",
            PAPER_PRESETS.iter().map(|p| p.id).collect::<Vec<_>>().join(", ")
        ));
    }
    Ok((w, h))
}

fn parse_size(spec: &str) -> Result<(f64, f64), String> {
    let lower = spec.to_ascii_lowercase();
    let (w, h) = lower
        .split_once('x')
        .ok_or_else(|| format!("invalid size '{spec}': expected WIDTHxHEIGHT in mm"))?;
    let w = w.trim().parse::<f64>().map_err(|_| format!("invalid size '{spec}'"))?;
    let h = h.trim().parse::<f64>().map_err(|_| format!("invalid size '{spec}'"))?;
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return Err(format!("invalid size '{spec}'"));
    }
    Ok((w, h))
}

fn parse_numbers(spec: &str, expected: usize) -> Result<Vec<f64>, String> {
    let v: Result<Vec<f64>, _> = spec
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect();
    let v = v.map_err(|_| format!("invalid number list '{spec}'"))?;
    if v.len() != expected {
        return Err(format!("expected {expected} numbers in '{spec}'"));
    }
    Ok(v)
}

/// Default output name: the source file name with `.pdf`, numbered if taken
/// (the wizard's suggestion logic).
fn default_output_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "rasterbation".to_string());
    let dir = input.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = dir.join(format!("{stem}.pdf"));
    let mut n = 0;
    while candidate.exists() {
        n += 1;
        candidate = dir.join(format!("{stem}_{n}.pdf"));
    }
    candidate
}

// ---------------------------------------------------------------------------
// Modern path.
// ---------------------------------------------------------------------------

/// Set a string enum field from an optional CLI string.
macro_rules! set_enum {
    ($o:expr, $f:ident, $arg:expr, $ty:ty, $flag:literal) => {
        if let Some(v) = $arg {
            $o.$f = <$ty>::parse_cli(v).ok_or_else(|| {
                let allowed: Vec<String> = <$ty>::ALL.iter().map(|x| x.to_string()).collect();
                format!("invalid {} '{}': expected one of {}", $flag, v, allowed.join(", "))
            })?;
        }
    };
}

fn parse_rgb(flag: &str, s: &str) -> Result<Rgb, String> {
    Rgb::parse(s).map_err(|_| format!("invalid {flag} '{s}': use RRGGBB, #RRGGBB or R,G,B"))
}

fn parse_pair(s: &str) -> Result<(f64, f64), String> {
    let v: Vec<&str> = s.split(',').collect();
    if v.len() != 2 {
        return Err(format!("expected 2 numbers in '{s}'"));
    }
    let a = v[0].trim().parse::<f64>().map_err(|_| format!("invalid number list '{s}'"))?;
    let b = v[1].trim().parse::<f64>().map_err(|_| format!("invalid number list '{s}'"))?;
    Ok((a, b))
}

/// `--size` value: `WxH` (exact), `width:N` / `height:N` (sheets) or a bare
/// number (width in sheets).
fn parse_size_spec(s: &str) -> Option<ModernSizeSpec> {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("width:") {
        return rest.trim().parse::<f64>().ok().map(ModernSizeSpec::WidthSheets);
    }
    if let Some(rest) = t.strip_prefix("height:") {
        return rest.trim().parse::<f64>().ok().map(ModernSizeSpec::HeightSheets);
    }
    if let Some((a, b)) = t.split_once(['x', 'X']) {
        let w = a.trim().parse::<f64>().ok()?;
        let h = b.trim().parse::<f64>().ok()?;
        return Some(ModernSizeSpec::Size { w, h });
    }
    t.parse::<f64>().ok().map(ModernSizeSpec::WidthSheets)
}

/// Apply the extended `--color` value set (A-6): `black`, `RRGGBB`,
/// `fixed:RRGGBB`, `average`, `grayscale`, `dual:FG/BG`.
fn apply_color_arg(o: &mut ModernOptions, s: &str) -> Result<(), String> {
    let t = s.trim();
    let l = t.to_ascii_lowercase();
    match l.as_str() {
        "black" => {
            o.ink = InkMode::Fixed;
            o.ink_color = Rgb(0, 0, 0);
        }
        "average" | "avg" => o.ink = InkMode::Average,
        "grayscale" | "greyscale" | "gray" | "grey" => o.ink = InkMode::Grayscale,
        _ => {
            if let Some(rest) = l.strip_prefix("fixed:") {
                o.ink = InkMode::Fixed;
                o.ink_color = parse_rgb("--color", rest)?;
            } else if let Some(rest) = l.strip_prefix("dual:") {
                let (fg, bg) = rest.split_once('/').ok_or_else(|| {
                    format!("invalid --color '{s}': dual expects dual:FG/BG")
                })?;
                o.ink = InkMode::DualTone;
                o.fg_color = parse_rgb("--color", fg)?;
                o.bg_color = parse_rgb("--color", bg)?;
            } else if let Ok(rgb) = Rgb::parse(t) {
                o.ink = InkMode::Fixed;
                o.ink_color = rgb;
            } else {
                return Err(format!(
                    "invalid color '{s}': use black, RRGGBB, fixed:RRGGBB, average, grayscale or dual:FG/BG"
                ));
            }
        }
    }
    Ok(())
}

/// Build the modern option set from the CLI, starting from `base` (a loaded
/// preset, or the model default). One branch per `ModernOptions` field.
fn build_options(cli: &Cli, mut o: ModernOptions) -> Result<ModernOptions, String> {
    // render & screen
    set_enum!(o, screen, cli.screen.as_deref(), ScreenFamily, "--screen");
    set_enum!(o, stagger, cli.stagger.as_deref(), Stagger, "--stagger");
    if let Some(v) = cli.halo_cells {
        o.halo_cells = v;
    }
    set_enum!(o, pattern, cli.pattern.as_deref(), Pattern, "--pattern");
    if let Some(v) = cli.dot_size {
        o.nominal_dot_mm = v;
    }
    set_enum!(o, coverage, cli.coverage.as_deref(), Coverage, "--coverage");
    if let Some(v) = &cli.grid_phase {
        o.grid_phase_mm = parse_pair(v)?;
    }

    // tone
    if let Some(v) = cli.min_raster {
        o.min_raster = v;
    }
    if let Some(v) = cli.max_raster {
        o.max_raster = v;
    }
    set_enum!(o, tone_curve, cli.tone.as_deref(), ToneCurve, "--tone");
    if let Some(v) = cli.tone_exponent {
        o.tone_exponent = v;
    }
    if let Some(v) = cli.tone_log_k {
        o.tone_log_k = v;
    }
    if cli.invert_tone {
        o.invert_tone = true;
    }
    set_enum!(o, sampling, cli.sampling.as_deref(), Sampling, "--sampling");
    set_enum!(o, dither, cli.dither.as_deref(), Dither, "--dither");
    if let Some(v) = cli.dither_strength {
        o.dither_strength = v;
    }
    if let Some(v) = cli.dither_levels {
        o.dither_levels = v;
    }
    if let Some(v) = cli.dither_block {
        o.dither_block = v;
    }
    if let Some(v) = cli.line_aspect {
        o.line_aspect = v;
    }
    if let Some(v) = cli.mosaic_contrast {
        o.mosaic_contrast = v;
    }

    // colour
    if let Some(v) = &cli.color {
        apply_color_arg(&mut o, v)?;
    }
    if let Some(v) = &cli.ink_color {
        o.ink_color = parse_rgb("--ink-color", v)?;
    }
    if let Some(v) = &cli.fg_color {
        o.fg_color = parse_rgb("--fg-color", v)?;
    }
    if let Some(v) = &cli.bg_color {
        o.bg_color = parse_rgb("--bg-color", v)?;
    }
    if let Some(v) = &cli.paper_color {
        o.paper_color = parse_rgb("--paper-color", v)?;
    }
    if let Some(v) = &cli.theme {
        o.theme = v.clone();
    }
    if cli.shading {
        o.shading = true;
    }
    if let Some(v) = cli.shading_strength {
        o.shading_strength = v;
    }
    if cli.invert_colors {
        o.invert_colors = true;
    }
    if let Some(v) = cli.dual_softness {
        o.dual_softness = v;
    }

    // layout
    if let Some(v) = &cli.paper {
        o.paper = PaperSpec::parse_token(v)
            .map_err(|_| format!("invalid --paper '{v}': use a4, a3, letter, legal or WxH in mm"))?;
    }
    if cli.landscape {
        o.orientation = Orientation::Landscape;
    }
    if let Some(s) = &cli.size {
        o.size_spec = parse_size_spec(s)
            .ok_or_else(|| format!("invalid --size '{s}': use WxH, width:N, height:N or a number"))?;
    } else if let Some(n) = cli.height_sheets {
        o.size_spec = ModernSizeSpec::HeightSheets(n);
    } else if let Some(n) = cli.width_sheets {
        o.size_spec = ModernSizeSpec::WidthSheets(n);
    }
    set_enum!(o, size_unit, cli.size_unit.as_deref(), SizeUnit, "--size-unit");
    set_enum!(o, fit, cli.fit.as_deref(), Fit, "--fit");
    set_enum!(o, align_h, cli.align_h.as_deref(), Align, "--align-h");
    set_enum!(o, align_v, cli.align_v.as_deref(), Align, "--align-v");
    if let Some(v) = cli.margin_mm {
        o.margin_mm = v;
    }
    if let Some(v) = cli.overlap_mm {
        o.overlap_mm = v;
    }
    set_enum!(o, overlap_mode, cli.overlap_mode.as_deref(), OverlapMode, "--overlap-mode");
    if let Some(v) = cli.bleed_mm {
        o.bleed_mm = v;
    }
    set_enum!(o, guides, cli.guides.as_deref(), GuideStyle, "--guides");
    if let Some(v) = cli.guide_width_pt {
        o.guide_width_pt = v;
    }
    if let Some(v) = cli.corner_mark_mm {
        o.corner_mark_mm = v;
    }
    if let Some(v) = &cli.guide_color {
        o.guide_color = parse_rgb("--guide-color", v)?;
    }
    set_enum!(o, labels, cli.labels.as_deref(), LabelStyle, "--labels");
    if let Some(v) = cli.label_size_pt {
        o.label_size_pt = v;
    }
    if let Some(v) = cli.label_offset_mm {
        o.label_offset_mm = v;
    }
    if let Some(v) = cli.page_warn {
        o.page_warn = v;
    }
    if let Some(v) = cli.page_limit {
        o.page_limit = v;
    }

    // output
    set_enum!(o, render, cli.render.as_deref(), rasterbator_core::modern::RenderMode, "--render");
    set_enum!(o, dpi, cli.dpi.as_deref(), Dpi, "--dpi");
    if let Some(v) = cli.jpeg_quality {
        o.jpeg_quality = v;
    }
    if cli.compress {
        o.compress = true;
    }
    if let Some(v) = &cli.title {
        o.title = v.clone();
    }
    if let Some(v) = &cli.creator {
        o.creator = v.clone();
    }
    set_enum!(o, creation_date, cli.creation_date.as_deref(), CreationDate, "--creation-date");
    set_enum!(o, print_scaling, cli.print_scaling.as_deref(), PrintScaling, "--print-scaling");
    if cli.no_atomic {
        o.atomic = false;
    }
    if cli.strict {
        o.strict = true;
    }
    if let Some(v) = cli.auto_raster_cells {
        o.auto_raster_cells = v;
    }

    // input
    if let Some(v) = &cli.crop {
        o.crop = Some(
            CropRectPx::parse_token(v)
                .map_err(|_| format!("invalid --crop '{v}': expected X1,Y1,X2,Y2 in pixels"))?,
        );
    }
    set_enum!(o, rotate, cli.rotate.as_deref(), Rotation, "--rotate");
    set_enum!(o, exif_orientation, cli.exif_orientation.as_deref(), ExifOrientation, "--exif-orientation");
    if let Some(v) = cli.brightness {
        o.brightness = v;
    }
    if let Some(v) = cli.contrast {
        o.contrast = v;
    }
    if let Some(v) = cli.saturation {
        o.saturation = v;
    }
    if let Some(v) = cli.alpha_threshold {
        o.alpha_threshold = v;
    }
    if cli.no_resample {
        o.resample = false;
    }
    if let Some(v) = cli.resample_dpi {
        o.resample_dpi = v;
    }

    // ux
    if let Some(v) = &cli.preset {
        o.preset = Some(v.clone());
    }
    if let Some(v) = &cli.save_preset {
        o.save_preset = Some(v.clone());
    }
    if let Some(v) = &cli.delete_preset {
        o.delete_preset = Some(v.clone());
    }
    if cli.list_presets {
        o.list_presets = true;
    }
    if let Some(v) = &cli.preset_dir {
        o.preset_dir = v.clone();
    }
    set_enum!(o, progress, cli.progress.as_deref(), ProgressMode, "--progress");
    if cli.quiet {
        o.quiet = true;
    }
    if cli.print_summary {
        o.print_summary = true;
    }

    Ok(o)
}

fn print_diag(d: &Diagnostic) {
    eprintln!("rasterbator: {}", d.message);
    if let Some(h) = &d.hint {
        eprintln!("  hint: {h}");
    }
}

fn print_report(r: &ValidationReport, quiet: bool) {
    for d in r.items() {
        match d.severity {
            Severity::Error => {
                eprintln!("rasterbator: {}", d.message);
                if let Some(h) = &d.hint {
                    eprintln!("  hint: {h}");
                }
            }
            Severity::Warning => eprintln!("warning: {}", d.message),
            Severity::Info => {
                if !quiet {
                    eprintln!("note: {}", d.message);
                }
            }
        }
    }
}

fn effective_progress(o: &ModernOptions) -> ProgressMode {
    if o.quiet {
        return ProgressMode::Off;
    }
    match o.progress {
        ProgressMode::Auto => {
            use std::io::IsTerminal;
            if std::io::stderr().is_terminal() {
                ProgressMode::Tty
            } else {
                ProgressMode::Plain
            }
        }
        m => m,
    }
}

fn fmt_eta(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Throttled progress reporter (design-a §4.5).
struct ProgressPrinter {
    mode: ProgressMode,
    last: Instant,
    last_bucket: i64,
}

impl ProgressPrinter {
    fn new(mode: ProgressMode) -> Self {
        ProgressPrinter { mode, last: Instant::now() - std::time::Duration::from_secs(1), last_bucket: -1 }
    }

    fn update(&mut self, p: &Progress, elapsed: f64) {
        match self.mode {
            ProgressMode::Off => {}
            ProgressMode::Tty => {
                let now = Instant::now();
                if now.duration_since(self.last).as_millis() < 50 && p.phase != RenderPhase::Finished {
                    return;
                }
                self.last = now;
                let f = p.fraction();
                let eta = if f > 0.02 && f < 1.0 {
                    fmt_eta(elapsed / f * (1.0 - f))
                } else {
                    "--:--".to_string()
                };
                eprint!(
                    "\r  page {}/{}  {:.0}%  cells {:.1}M/{:.1}M  eta {}\x1b[K",
                    p.pages_done,
                    p.total_pages,
                    f * 100.0,
                    p.cells_done as f64 / 1e6,
                    p.total_cells as f64 / 1e6,
                    eta
                );
            }
            ProgressMode::Plain => {
                let bucket = (p.fraction() * 100.0 / 4.0).floor() as i64;
                if bucket == self.last_bucket {
                    return;
                }
                self.last_bucket = bucket;
                eprintln!(
                    "  page {}/{}  {:.0}%",
                    p.pages_done,
                    p.total_pages,
                    p.fraction() * 100.0
                );
            }
            ProgressMode::Auto => {}
        }
    }

    fn finish(&mut self) {
        if matches!(self.mode, ProgressMode::Tty) {
            eprintln!();
        }
    }
}

fn fmt_mm(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn run_modern(cli: &Cli) -> ExitCode {
    // 1. Base: model default, optionally overridden by a loaded preset.
    let default_base = ModernOptions::default();
    let preset_dir = cli
        .preset_dir
        .clone()
        .unwrap_or_else(|| default_base.preset_dir.clone());
    let mut base = default_base;
    if let Some(name) = &cli.preset {
        let store = PresetStore::new(preset_dir.clone());
        match store.load(name) {
            Ok(o) => base = o,
            Err(d) => {
                print_diag(&d);
                return ExitCode::from(2);
            }
        }
    }

    // 2. Apply CLI overrides.
    let opts = match build_options(cli, base) {
        Ok(o) => o,
        Err(m) => {
            eprintln!("rasterbator: {m}");
            return ExitCode::from(2);
        }
    };

    // 3. Development hook: dump and exit, no rendering.
    if cli.dump_options {
        print!("{}", opts.to_text());
        return ExitCode::SUCCESS;
    }

    // 4. Preset short-circuits (no rendering).
    let store = PresetStore::new(opts.preset_dir.clone());
    if opts.list_presets {
        match store.list() {
            Ok(entries) => {
                for entry in entries {
                    println!("{}", entry.name);
                }
            }
            Err(d) => {
                print_diag(&d);
                return ExitCode::from(2);
            }
        }
        return ExitCode::SUCCESS;
    }
    if let Some(name) = &opts.save_preset {
        match store.save(name, &opts, true) {
            Ok(()) => {
                eprintln!("saved preset {name}");
                return ExitCode::SUCCESS;
            }
            Err(d) => {
                print_diag(&d);
                return ExitCode::from(2);
            }
        }
    }
    if let Some(name) = &opts.delete_preset {
        match store.delete(name) {
            Ok(()) => {
                eprintln!("deleted preset {name}");
                return ExitCode::SUCCESS;
            }
            Err(d) => {
                print_diag(&d);
                return ExitCode::from(2);
            }
        }
    }

    // 5. Load the source with the modern guards.
    let img = match inspect(&cli.input).and_then(|_| decode_rgba(&cli.input)) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("rasterbator: cannot read {}: {e}", cli.input.display());
            return ExitCode::from(1);
        }
    };
    let src = SourceInfo::synthetic(img.width(), img.height());

    // 6. Solve geometry (only layout source).
    let geom = match JobGeometry::solve(&opts, &src) {
        Ok(g) => g,
        Err(report) => {
            print_report(&report, opts.quiet);
            return ExitCode::from(2);
        }
    };

    // 7. Validate exactly once.
    let report = validate(&opts, Some(&src), Some(&geom));
    print_report(&report, opts.quiet);
    if report.has_blocking() {
        return ExitCode::from(2);
    }

    // 8. Prepare the source. The resample targets the cell grid of the source
    //    rectangle, so every cell averages its own pixel.
    let po = PrepareOptions {
        resample_target: resample_target_for(&opts, geom.source_rect_mm),
        ..PrepareOptions::from_options(&opts)
    };
    let prepared = match prepare_image(&img, &po) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rasterbator: {e}");
            return ExitCode::from(1);
        }
    };

    // 9. Render page by page into the modern PDF.
    let cancel = AtomicBool::new(false);
    let mut session = RenderSession::new(&prepared, &opts, &geom, &cancel);
    let mut pdf = ModernPdf::new(&opts, &geom);
    pdf.set_raw_content(cli.raw_content);
    let mut printer = ProgressPrinter::new(effective_progress(&opts));

    {
        let mut target = PdfTarget::new(&mut pdf);
        loop {
            match session.render_next(&mut target) {
                Ok(p) => {
                    printer.update(&p, session.elapsed_secs());
                    if p.phase == RenderPhase::Finished {
                        break;
                    }
                }
                Err(RenderError::Cancelled) => {
                    printer.finish();
                    eprintln!("rasterbator: cancelled");
                    return ExitCode::from(3);
                }
                Err(e) => {
                    printer.finish();
                    eprintln!("rasterbator: {e}");
                    return ExitCode::from(1);
                }
            }
        }
        target.finish();
    }
    printer.finish();
    let stats = *session.stats();

    // 10. Save (atomic unless disabled).
    let output = match &cli.output {
        Some(path) => path.clone(),
        None => default_output_path(&cli.input),
    };
    let bytes = match pdf.save(&output, opts.atomic) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("rasterbator: cannot write {}: {e}", output.display());
            return ExitCode::from(1);
        }
    };

    // 11. Summary on stderr.
    if !opts.quiet {
        eprintln!(
            "wrote {}: {} pages ({} x {} sheets), {:.1} M cells, dot {} mm, {}/{}, ink {:.1} %, {:.1} MB",
            output.display(),
            geom.total_pages,
            geom.pages_x,
            geom.pages_y,
            geom.total_cells as f64 / 1e6,
            fmt_mm(opts.nominal_dot_mm),
            opts.screen,
            opts.pattern,
            stats.ink_density_percent(),
            bytes as f64 / 1e6
        );
    }

    // 12. Machine-readable summary on stdout.
    if opts.print_summary {
        println!(
            "{{\"output\":{},\"pages\":{},\"pages_x\":{},\"pages_y\":{},\"cells\":{},\"dot_mm\":{},\"screen\":\"{}\",\"pattern\":\"{}\",\"ink_percent\":{:.2},\"bytes\":{}}}",
            json_str(&output.to_string_lossy()),
            geom.total_pages,
            geom.pages_x,
            geom.pages_y,
            geom.total_cells,
            opts.nominal_dot_mm,
            opts.screen,
            opts.pattern,
            stats.ink_density_percent(),
            bytes
        );
    }

    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The default engine is the legacy one (no modern flag consulted).
    #[test]
    fn legacy_is_the_default_mode() {
        let cli = Cli::try_parse_from(["rasterbator", "photo.png"]).unwrap();
        assert_eq!(cli.mode, Mode::Legacy);
    }

    /// Every [`ModernOptions`] field is reachable from a CLI flag: parse a
    /// fully-populated argv and compare against the hand-built expectation
    /// (which differs from the model default in every field).
    #[test]
    fn every_option_settable_from_cli() {
        let argv: Vec<&str> = vec![
            "rasterbator", "input.png",
            "--mode", "modern",
            // screen & pattern
            "--screen", "hex",
            "--stagger", "half-pitch",
            "--halo-cells", "2",
            "--pattern", "mosaic",
            "--dot-size", "7.5",
            "--coverage", "tangent",
            "--grid-phase", "1.5,2.5",
            // tone
            "--min-raster", "0.1",
            "--max-raster", "0.9",
            "--tone", "exp",
            "--tone-exponent", "3.0",
            "--tone-log-k", "6.0",
            "--invert-tone",
            "--sampling", "bilinear",
            "--dither", "atkinson",
            "--dither-strength", "0.4",
            "--dither-levels", "16",
            "--dither-block", "2",
            "--line-aspect", "0.7",
            "--mosaic-contrast", "2.0",
            // colour
            "--color", "average",
            "--ink-color", "010203",
            "--fg-color", "0a0b0c",
            "--bg-color", "0d0e0f",
            "--paper-color", "f0f0f0",
            "--theme", "mono",
            "--shading",
            "--shading-strength", "0.5",
            "--invert-colors",
            "--dual-softness", "0.6",
            // layout
            "--paper", "a3",
            "--landscape",
            "--size", "900x600",
            "--size-unit", "in",
            "--fit", "cover",
            "--align-h", "start",
            "--align-v", "end",
            "--margin-mm", "12",
            "--overlap-mm", "5",
            "--overlap-mode", "shared",
            "--bleed-mm", "3",
            "--guides", "border",
            "--guide-width-pt", "1.5",
            "--corner-mark-mm", "6",
            "--guide-color", "7f7f7f",
            "--labels", "coordinates",
            "--label-size-pt", "12",
            "--label-offset-mm", "10",
            "--page-warn", "50",
            "--page-limit", "500",
            // output
            "--render", "raster",
            "--dpi", "200",
            "--jpeg-quality", "70",
            "--compress",
            "--title", "My Poster",
            "--creator", "tester",
            "--creation-date", "now",
            "--print-scaling", "viewer",
            "--no-atomic",
            "--strict",
            "--auto-raster-cells", "1000",
            // input
            "--crop", "1,2,3,4",
            "--rotate", "90",
            "--exif-orientation", "ignore",
            "--brightness", "-5",
            "--contrast", "10",
            "--saturation", "-20",
            "--alpha-threshold", "200",
            "--no-resample",
            "--resample-dpi", "150",
            // ux
            "--preset", "base",
            "--save-preset", "sp1",
            "--delete-preset", "dp1",
            "--list-presets",
            "--preset-dir", "mapping-test-presets",
            "--progress", "tty",
            "--quiet",
            "--print-summary",
        ];
        let cli = Cli::try_parse_from(argv).unwrap();
        let got = build_options(&cli, ModernOptions::default()).unwrap();

        let mut e = ModernOptions::default();
        e.screen = ScreenFamily::Hex;
        e.stagger = Stagger::HalfPitch;
        e.halo_cells = 2;
        e.pattern = Pattern::Mosaic;
        e.nominal_dot_mm = 7.5;
        e.coverage = Coverage::Tangent;
        e.grid_phase_mm = (1.5, 2.5);
        e.min_raster = 0.1;
        e.max_raster = 0.9;
        e.tone_curve = ToneCurve::Exponential;
        e.tone_exponent = 3.0;
        e.tone_log_k = 6.0;
        e.invert_tone = true;
        e.sampling = Sampling::Bilinear;
        e.dither = Dither::Atkinson;
        e.dither_strength = 0.4;
        e.dither_levels = 16;
        e.dither_block = 2;
        e.line_aspect = 0.7;
        e.mosaic_contrast = 2.0;
        e.ink = InkMode::Average;
        e.ink_color = Rgb(0x01, 0x02, 0x03);
        e.fg_color = Rgb(0x0a, 0x0b, 0x0c);
        e.bg_color = Rgb(0x0d, 0x0e, 0x0f);
        e.paper_color = Rgb(0xf0, 0xf0, 0xf0);
        e.theme = "mono".to_string();
        e.shading = true;
        e.shading_strength = 0.5;
        e.invert_colors = true;
        e.dual_softness = 0.6;
        e.paper = PaperSpec::parse_token("a3").unwrap();
        e.orientation = Orientation::Landscape;
        e.size_spec = ModernSizeSpec::Size { w: 900.0, h: 600.0 };
        e.size_unit = SizeUnit::In;
        e.fit = Fit::Cover;
        e.align_h = Align::Start;
        e.align_v = Align::End;
        e.margin_mm = 12.0;
        e.overlap_mm = 5.0;
        e.overlap_mode = OverlapMode::Shared;
        e.bleed_mm = 3.0;
        e.guides = GuideStyle::Border;
        e.guide_width_pt = 1.5;
        e.corner_mark_mm = 6.0;
        e.guide_color = Rgb(0x7f, 0x7f, 0x7f);
        e.labels = LabelStyle::Coordinates;
        e.label_size_pt = 12.0;
        e.label_offset_mm = 10.0;
        e.page_warn = 50;
        e.page_limit = 500;
        e.render = rasterbator_core::modern::RenderMode::Raster;
        e.dpi = Dpi::P200;
        e.jpeg_quality = 70;
        e.compress = true;
        e.title = "My Poster".to_string();
        e.creator = "tester".to_string();
        e.creation_date = CreationDate::Now;
        e.print_scaling = PrintScaling::Viewer;
        e.atomic = false;
        e.strict = true;
        e.auto_raster_cells = 1000;
        e.crop = Some(CropRectPx { x1: 1, y1: 2, x2: 3, y2: 4 });
        e.rotate = Rotation::D90;
        e.exif_orientation = ExifOrientation::Ignore;
        e.brightness = -5.0;
        e.contrast = 10.0;
        e.saturation = -20.0;
        e.alpha_threshold = 200;
        e.resample = false;
        e.resample_dpi = 150;
        e.preset = Some("base".to_string());
        e.save_preset = Some("sp1".to_string());
        e.delete_preset = Some("dp1".to_string());
        e.list_presets = true;
        e.preset_dir = PathBuf::from("mapping-test-presets");
        e.progress = ProgressMode::Tty;
        e.quiet = true;
        e.print_summary = true;

        assert_eq!(got, e, "CLI mapping did not cover every ModernOptions field");

        // And the expectation itself exercises every documented key: every
        // option_names() entry differs from the model default.
        let d = ModernOptions::default();
        let mut differing = std::collections::BTreeSet::new();
        for name in ModernOptions::option_names() {
            let gv = got.to_text();
            let dv = d.to_text();
            let gline = gv.lines().find(|l| l.starts_with(&format!("{name}=")));
            let dline = dv.lines().find(|l| l.starts_with(&format!("{name}=")));
            if gline != dline {
                differing.insert(*name);
            }
        }
        for name in ModernOptions::option_names() {
            assert!(
                differing.contains(name),
                "option '{name}' is not settable from the CLI mapping under test"
            );
        }
    }

    /// `--color` accepts the A-6 extended value set.
    #[test]
    fn color_value_set() {
        let cases = [
            ("black", InkMode::Fixed),
            ("average", InkMode::Average),
            ("grayscale", InkMode::Grayscale),
            ("fixed:112233", InkMode::Fixed),
            ("aabbcc", InkMode::Fixed),
            ("dual:000000/ffffff", InkMode::DualTone),
        ];
        for (arg, ink) in cases {
            let cli = Cli::try_parse_from(["rasterbator", "i.png", "--mode", "modern", "--color", arg])
                .unwrap();
            let o = build_options(&cli, ModernOptions::default()).unwrap();
            assert_eq!(o.ink, ink, "color {arg}");
        }
        let cli = Cli::try_parse_from(["rasterbator", "i.png", "--color", "nonsense"]).unwrap();
        assert!(build_options(&cli, ModernOptions::default()).is_err());
    }
}
