# Rasterbator (Rust port)

[![Release](https://github.com/termi-afk/rasterbator/actions/workflows/release.yml/badge.svg)](https://github.com/termi-afk/rasterbator/actions/workflows/release.yml)

This repository is a reverse engineering + reimplementation project around
**The Rasterbator**, the classic tool that turns a picture into a huge,
printable poster made of a grid of halftone dots, split across many paper
sheets and written as a PDF.

It contains:

| Path | What it is |
| --- | --- |
| `Rasterbator.exe` | The analyzed original binary (Windows .NET, build 1.0.2693, ~2007). |
| `crates/rasterbator-core` | The Rust engine as a reusable library (image → dot pages → PDF). |
| `crates/rasterbator-cli` | Command line tool (`rasterbator`). |
| `crates/rasterbator-gui` | Desktop wizard (`rasterbator-gui`) mirroring the original UI flow. |
| `original/` | The original v1.0 (2005) C# sources, kept under their GPL license. |
| `analysis/` | Reverse engineering artifacts and the verification harness. |

The Rust port reproduces the original's output **op for op**: for a matrix of
16 input/option combinations the PDF content streams of the original engine
and of this port are numerically identical (see
[Verification](#verification)).

---

## 1. What the original does (analysis)

`Rasterbator.exe` is a 32-bit .NET WinForms wizard (metadata v1.1.4322), with
the rasterbation engine in class `Rasterbator.Rasterbator` and the wizard in
`Rasterbator.MainForm`. It renders through **iTextSharp 1.0.4.0** (a
Java-ported PDF library). The shipped source of version 1.0 (2005) is in
`original/`; the binary is a later build with a few differences that the port
follows (see `original/README.md`).

The engine, as implemented and verified:

1. Works at a fixed **144 dpi** grid. Poster size `(w,h)` in mm → `w/25.4*144`
   px; the page is a PDF page of `72*w/25.4` points.
2. Sizes the dot grid: `MaxRadius = dot_mm/25.4*144/2` (radius of the largest
   dot in working pixels); the grid pitch is `SquareSize = 2*(MaxRadius-1)/√2`
   working pixels, i.e. `SquareSize/2` points on the page. Fully black cells
   produce dots of `MaxRadius/2` points radius.
3. For every grid cell, averages the **HSL lightness** (`Color.GetBrightness`)
   of the source area it covers, treating anything outside the image (or the
   crop) as white. Dot radius is `(1 - lightness) * MaxRadius / 2` points.
4. Colors: a fixed RGB (`black` or `RRGGBB`) or `avg`, where each dot takes the
   average source color of its cell (channel sums truncated like `(int)`).
5. Cells from one grid step outside the page are also drawn (dots of
   neighbouring pages bleed in), then four white rectangles trim the bleeds
   back to the rasterbated area; `--cutout` adds the 0.5 pt grey guide
   rectangle around that area.
6. Pages are laid out row-major over `ceil(size/paper)` sheet grids, and the
   PDF is written with `/ViewerPreferences <</FitWindow true>>`, `/Title
   (Rasterbation)` and `/Creator (The Rasterbator at
   http://homokaasu.org/rasterbator/)`.

### Details the port replicates deliberately

These are not accidents; they are required for output equality:

- **Number formatting.** iTextSharp 1.0.4 quantizes coordinates: values < 1
  with up to 5 decimals, values ≤ 32767 with 2 decimals, larger as integers,
  trailing zeros trimmed. `rasterbator-core::format_double` is an exact port.
- **Circle geometry.** Dots are four cubic Béziers with the constant
  `0.5523`, counterclockwise from `(x + r, y)`, followed by a fill.
- **First-page prologue.** iText 1.0.4 writes `q / BT / 0 <height> Td / ET / Q`
  at the start of the first page's content stream; the port writes it too.
- **Float32 arithmetic order** of every formula (e.g. `(2f*(MaxRadius-1f)) /
  sqrt(2)` computed in double then cast), so values round identically.
- **`(int)` truncation semantics** for sizes and average colors, including the
  x86 behavior for NaN/out-of-range float→int casts.

## 2. Layout

```
Cargo.toml                  workspace (core + cli + gui)
crates/rasterbator-core/    library: engine, canvas, PDF writer, number format
crates/rasterbator-cli/     `rasterbator` binary
crates/rasterbator-gui/     `rasterbator-gui` binary (eframe/egui wizard)
original/                   2005 GPL sources + COPYING + provenance (Wayback link)
analysis/                   decompiled sources, IL dumps, oracle harness, comparators
```

## 3. Usage

### CLI

```
rasterbator photo.jpg                          # A4 portrait, 5 sheets wide, 10 mm black dots
rasterbator photo.jpg -w 10 --dot-size 15      # ten sheets wide, larger dots
rasterbator photo.jpg --paper a3 -h 4          # A3, four sheets high
rasterbator photo.jpg --size 900x600 -o big.pdf  # exact 900 x 600 mm poster
rasterbator photo.jpg --color avg --cutout     # average colors + cutout guides
rasterbator photo.jpg --paper 210x297 --crop 100,50,700,550
```

Run `rasterbator --help` for all options. The CLI mirrors the wizard: paper
presets (`a4`, `a3`, `letter`, `legal`) or custom `WxH` mm, orientation,
poster size given as a number of sheets (`-w`/`--height-sheets`, the other
dimension follows the image aspect, exactly like the wizard) or as an exact
size, dot size in mm, color mode, cutout guides, and an optional source crop.
Dot sizes below 3 mm print a warning (the wizard asks for confirmation
instead). Progress is written to stderr.

### GUI wizard

```
cargo run --release -p rasterbator-gui
```

The GUI is built as a Windows GUI application (`#![windows_subsystem =
"windows"]` in `crates/rasterbator-gui/src/main.rs`), so no console window
opens; for console output while debugging, temporarily remove that attribute.

Five steps like the original: source image → paper size → output size (with
page-split preview and paper consumption) → rasterbation options (dot size,
color mode, cutout) → save as; then a progress page and completion page with
"open the produced PDF" on close.

### Library

```rust
fn rasterbate_to_pdf(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let image = rasterbator_core::load_rgba(input)?;
    let (paper_w, paper_h) = (210.0, 297.0); // A4, mm
    let size = rasterbator_core::resolve_output_size(
        rasterbator_core::SizeSpec::WidthSheets(5.0), paper_w, paper_h,
        image.width() as f64, image.height() as f64);
    let opts = rasterbator_core::EngineOptions {
        paper_width_mm: paper_w,
        paper_height_mm: paper_h,
        image_width_mm: size.width_mm,
        image_height_mm: size.height_mm,
        dot_size_mm: 10.0,
        color: rasterbator_core::ColorMode::Fixed { r: 0, g: 0, b: 0 },
        cutout: true,
        crop: None,
    };
    let mut pdf = rasterbator_core::PdfDocument::from_paper_mm(paper_w, paper_h);
    let summary = rasterbator_core::rasterbate(&image, &opts, &mut pdf, &mut |done, total| {
        eprintln!("page {done}/{total}");
    });
    eprintln!("{} pages", summary.total_pages);
    pdf.save(output)?;
    Ok(())
}
```

## Verification

How the port was validated against the original (details and scripts in
`analysis/`):

1. **Decompilation.** The original assembly was decompiled (ILSpy) and its IL
   inspected to pin down exact arithmetic (`Rasterbator.cs`, IL dumps in
   `analysis/`). The 2005 GPL sources from the original distribution
   (recovered via the Wayback Machine, see `original/README.md`) were used to
   cross-check the algorithm; differences between 2005 source and the 2007
   binary are documented.
2. **Behavioral oracle.** `analysis/refgen` is a small C# harness that
   references the *shipped* `Rasterbator.exe` assembly and runs its engine —
   with the original iTextSharp 1.0.4.0 from the same 2005 archive — to
   produce reference PDFs.
3. **Comparison.** `analysis/compare_pdfs.py` decompresses both PDFs and
   compares every content-stream token per page (numbers with tolerance),
   page count, media boxes, and the Title/Creator metadata.
4. **Matrix.** `pwsh analysis/run_matrix.ps1` regenerates 16 scenarios —
   gradient/photo/solid/alpha/palette-indexed/tiny images, multiple papers
   (presets, custom, landscape), presets of poster sizes (sheets wide/high,
   exact mm), dot sizes 5–25 mm (including fractional), colors (black,
   `RRGGBB`, `avg`), cutout on/off, crops, single- and multi-page layouts —
   and compares original vs port. **All 16 match** (numbers equal within
   0.011 pt; the JPEG scenarios also match at that tolerance).

Known limits of the verification: the oracle runs the original engine's IL on
a modern .NET runtime (the binary itself needs .NET 1.1), it exercises the
engine, not the WinForms UI; image decoding uses `image` crate decoders
instead of GDI+, which can differ by ±1 in a channel for lossy formats (below
the point where the original's 2-decimal quantization changes a coordinate in
the tested cases).

## Differences from the original (intentional)

- `/Producer` names this port; `/CreationDate` is the current date
  (`Title`/`Creator`/page geometry/`FitWindow` match the original).
- No localization: the wizard's `languages/*.xml` files are not used; UI text
  is English.
- The "rasterbate on low priority" thread option is not ported.
- The wizard one-dimension rounding uses the exact image aspect instead of the
  rounded preview size (sub-pixel difference, no effect on dot layout).
- Error messages/UX text are new.

## Building

```
cargo build --release            # all three crates
cargo test                       # unit tests (number format, size math)
cargo run --release -p rasterbator-cli -- --help
```

Requirements: Rust 1.75+ (edition 2021 workspace); the GUI uses
eframe/egui 0.31 and rfd for file dialogs.

Prebuilt archives for Windows x64, Linux x64 and macOS (x64/arm64) are
attached to [releases](https://github.com/termi-afk/rasterbator/releases);
pushing a version tag (e.g. `0.1.0`) runs the release workflow
(`.github/workflows/release.yml`), which tests the workspace, builds and
packages both executables for each platform and publishes the GitHub release.

## Licensing

The Rust port is a derived work of The Rasterbator (GPL-2.0-or-later,
Copyright © 2004-2005 Matias Ärje) and is released under
**AGPL-3.0-or-later** (see [LICENSE](LICENSE)); GPL-2.0-or-later permits use
under GPLv3, and GPLv3 §13 allows conveying the combined work together with an
AGPLv3 work. The files in `original/` remain under their original GPL-2.0-or-later
license with their own [COPYING](original/COPYING). iTextSharp is not
redistributed here; the port reimplements the tiny part of its output behavior
it needs.
