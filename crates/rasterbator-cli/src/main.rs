//! Command line interface for the Rasterbator port.
//!
//! Mirrors the wizard of The Rasterbator: source image, paper size, output
//! size (in sheets), rasterbation options and output file.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{ArgAction, Parser};
use rasterbator_core::{
    load_rgba, paper_preset, rasterbate, resolve_output_size, ColorMode, CropRect,
    EngineOptions, PdfDocument, SizeSpec, PAPER_PRESETS,
};

/// Turn an image into a printable multi-page poster of halftone dots (PDF).
///
/// A port of The Rasterbator: the poster is split over as many paper sheets
/// as needed and every sheet is filled with a grid of dots whose size
/// follows the brightness of the source image.
#[derive(Parser, Debug)]
#[command(name = "rasterbator", version, about, max_term_width = 100)]
#[command(after_help = "EXAMPLES:
  rasterbator photo.jpg                          A4 portrait, 5 sheets wide, black 10 mm dots
  rasterbator photo.jpg -w 10 --dot-size 15      ten sheets wide, 15 mm dots
  rasterbator photo.jpg --paper a3 --height-sheets 4    A3, four sheets high
  rasterbator photo.png --color ff3300 --cutout
  rasterbator photo.png --size 900x600 -o big.pdf       exact 900 x 600 mm poster")]
struct Cli {
    /// Source image (PNG, JPEG, GIF, BMP, TIFF, WebP)
    input: PathBuf,

    /// Output PDF path (default: input file name with .pdf; a numbered name
    /// is used if the file already exists)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Paper size: a4, a3, letter, legal, or custom WIDTHxHEIGHT in mm
    #[arg(long, default_value = "a4", value_name = "PAPER")]
    paper: String,

    /// Print on landscape sheets
    #[arg(long, action = ArgAction::SetTrue)]
    landscape: bool,

    /// Poster width in sheets (the height follows from the image aspect)
    #[arg(short = 'w', long, default_value_t = 5.0, value_name = "N")]
    width_sheets: f64,

    /// Poster height in sheets (overrides --width-sheets)
    #[arg(long, value_name = "N")]
    height_sheets: Option<f64>,

    /// Exact poster size WIDTHxHEIGHT in mm (overrides the sheet options)
    #[arg(short = 's', long, value_name = "WxH")]
    size: Option<String>,

    /// Maximum dot size in mm
    #[arg(short = 'd', long, default_value_t = 10.0, value_name = "MM")]
    dot_size: f64,

    /// Dot color: black, avg (average source color) or RRGGBB hex
    #[arg(short = 'c', long, default_value = "black", value_name = "COLOR")]
    color: String,

    /// Draw the cutting guide rectangle around the rasterbated area
    #[arg(long, action = ArgAction::SetTrue)]
    cutout: bool,

    /// Use only this part of the source image: X1,Y1,X2,Y2 in pixels
    #[arg(long, value_name = "X1,Y1,X2,Y2")]
    crop: Option<String>,

    /// Suppress progress output
    #[arg(short, long, action = ArgAction::SetTrue)]
    quiet: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("rasterbator: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), String> {
    // --- wizard step 1: source image ---
    if !cli.input.exists() {
        return Err(format!("cannot find file {}", cli.input.display()));
    }
    let image = load_rgba(&cli.input).map_err(|e| format!("cannot read {}: {e}", cli.input.display()))?;

    // --- wizard step 2: paper size ---
    let (mut paper_w, mut paper_h) = parse_paper(&cli.paper)?;
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
            None => SizeSpec::WidthSheets(cli.width_sheets),
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
    let color = ColorMode::parse(&cli.color)?;
    if cli.dot_size < 3.0 {
        eprintln!(
            "warning: a dot size below 3 mm ({} mm) produces very fine dots; \
             large posters may take long and be hard to print",
            cli.dot_size
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
        dot_size_mm: cli.dot_size,
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
            cli.dot_size
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
