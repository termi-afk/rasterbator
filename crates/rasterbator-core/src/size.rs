//! Paper presets and output-size handling, mirroring the wizard's logic.

/// A standard paper preset as offered by the original wizard.
#[derive(Clone, Copy, Debug)]
pub struct PaperPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub width_mm: f64,
    pub height_mm: f64,
}

/// Presets of the original wizard: A4, A3, Letter, Legal (portrait).
pub const PAPER_PRESETS: &[PaperPreset] = &[
    PaperPreset { id: "a4", name: "A4", width_mm: 210.0, height_mm: 297.0 },
    PaperPreset { id: "a3", name: "A3", width_mm: 297.0, height_mm: 420.0 },
    PaperPreset { id: "letter", name: "Letter", width_mm: 216.0, height_mm: 279.0 },
    PaperPreset { id: "legal", name: "Legal", width_mm: 216.0, height_mm: 356.0 },
];

/// Look up a preset by its id (case-insensitive).
pub fn paper_preset(id: &str) -> Option<PaperPreset> {
    let id = id.to_ascii_lowercase();
    PAPER_PRESETS.iter().copied().find(|p| p.id == id)
}

/// Page size in PDF points, exactly as the engine computes it:
/// `72f * (float)(int)mm / 25.4f` in float32 arithmetic.
pub fn paper_size_pt(paper_width_mm: f64, paper_height_mm: f64) -> (f32, f32) {
    let w = paper_width_mm.trunc() as f32;
    let h = paper_height_mm.trunc() as f32;
    (72.0f32 * w / 25.4f32, 72.0f32 * h / 25.4f32)
}

/// How the user specified the poster size (the wizard's "define output size"
/// step: a number of sheets along one dimension, the other is derived).
#[derive(Clone, Copy, Debug)]
pub enum SizeSpec {
    /// Poster width in sheets; height follows from the image aspect.
    WidthSheets(f64),
    /// Poster height in sheets; width follows from the image aspect.
    HeightSheets(f64),
    /// Explicit poster size in millimeters.
    SizeMm { width: f64, height: f64 },
}

/// Derived sheet counts and poster size.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedSize {
    pub sheets_x: f64,
    pub sheets_y: f64,
    pub width_mm: f64,
    pub height_mm: f64,
}

/// Resolve a [`SizeSpec`] into sheet counts and a poster size, mirroring the
/// wizard's `DisplayPaging` arithmetic (`num4`..`num7`).
///
/// `image_w`/`image_h` are the source image dimensions (the wizard uses its
/// display-sized copy; this port uses the original pixels, which only differs
/// by sub-pixel rounding).
pub fn resolve_output_size(
    spec: SizeSpec,
    paper_w_mm: f64,
    paper_h_mm: f64,
    image_w: f64,
    image_h: f64,
) -> ResolvedSize {
    match spec {
        SizeSpec::WidthSheets(sheets_x) => {
            let sheets_y = paper_w_mm / paper_h_mm * sheets_x * image_h / image_w;
            ResolvedSize {
                sheets_x,
                sheets_y,
                width_mm: paper_w_mm * sheets_x,
                height_mm: paper_h_mm * sheets_y,
            }
        }
        SizeSpec::HeightSheets(sheets_y) => {
            let sheets_x = paper_h_mm / paper_w_mm * sheets_y * image_w / image_h;
            ResolvedSize {
                sheets_x,
                sheets_y,
                width_mm: paper_w_mm * sheets_x,
                height_mm: paper_h_mm * sheets_y,
            }
        }
        SizeSpec::SizeMm { width, height } => ResolvedSize {
            sheets_x: width / paper_w_mm,
            sheets_y: height / paper_h_mm,
            width_mm: width,
            height_mm: height,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_matches_original() {
        let (w, h) = paper_size_pt(210.0, 297.0);
        assert_eq!(crate::numfmt::format_double(w as f64), "595.28");
        assert_eq!(crate::numfmt::format_double(h as f64), "841.89");
    }

    #[test]
    fn width_sheets_resolution() {
        // 5 A4 sheets wide, 4:3 landscape image.
        let r = resolve_output_size(SizeSpec::WidthSheets(5.0), 210.0, 297.0, 400.0, 300.0);
        assert_eq!(r.sheets_x, 5.0);
        assert!((r.width_mm - 1050.0).abs() < 1e-9);
        // rows = 210/297 * 5 * 300/400
        assert!((r.sheets_y - (210.0 / 297.0 * 5.0 * 0.75)).abs() < 1e-9);
    }
}
