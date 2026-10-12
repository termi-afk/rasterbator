//! Adapters between the modern GUI's preview helpers and the core API.
//!
//! The GUI draws its preview rasters itself from the canonical `JobGeometry`;
//! these helpers give it exactly the colour and tone decisions the export path
//! makes, so preview and PDF cannot diverge on those steps.

use rasterbator_core::modern::{
    color::{self, Rgb},
    options::{InkMode, ModernOptions},
    page, prepare::{self, PrepareOptions},
    preview::{self, PreviewDetail},
    sample::CellSampler,
    screen::ScreenLattice,
    source::{PreparedImage, MAX_FILE_BYTES, MAX_PIXELS},
    theme::{self, Theme},
    tone,
};

pub use rasterbator_core::modern::theme::THEMES;

/// A sampler over the source rectangle, using the export path's paper colour.
pub fn cell_sampler<'a>(
    prepared: &'a PreparedImage,
    opts: &ModernOptions,
    rect: [f64; 4],
) -> CellSampler<'a> {
    CellSampler::new(prepared, opts.paper_color, opts.sampling, rect)
}

/// The cell centre of the canonical lattice for these options.
pub fn cell_centre(i: i64, j: i64, opts: &ModernOptions) -> (f64, f64) {
    ScreenLattice::new(opts.screen, opts.nominal_dot_mm, opts.stagger, opts.grid_phase_mm, (0.0, 0.0))
        .cell_centre(i, j)
}

/// The tone mapping the export path applies (previews do not dither).
pub fn transfer_opts(level: f64, opts: &ModernOptions) -> f64 {
    let params = page::tone_params(opts);
    tone::transfer(level, &params, opts.invert_tone, 1.0)
}

/// Dual-tone decision helper. Softness acts on the colour inside
/// [`resolve_ink`], exactly as the core's `page::cell_ink` does.
pub fn dual_blend(t: f64, _softness: f64) -> f64 {
    t
}

/// Preview mirror of the core's per-cell ink decision.
#[allow(clippy::too_many_arguments)]
pub fn resolve_ink(
    ink: InkMode,
    ink_color: Rgb,
    fg: Rgb,
    bg: Rgb,
    sampled: Rgb,
    level: f64,
    t: f64,
    dual_softness: f64,
    shading: bool,
    shading_strength: f64,
    invert_colors: bool,
) -> Rgb {
    let mut rgb = match ink {
        InkMode::Fixed => ink_color,
        InkMode::Average => sampled,
        InkMode::Grayscale => {
            let l = (level * 255.0).round().clamp(0.0, 255.0) as u8;
            Rgb(l, l, l)
        }
        InkMode::DualTone => {
            let hard = if t >= 0.5 { fg } else { bg };
            let cont = color::lerp_rgb(bg, fg, t.clamp(0.0, 1.0));
            color::lerp_rgb(cont, hard, dual_softness.clamp(0.0, 1.0))
        }
    };
    if shading {
        rgb = color::shade(rgb, color::shading_factor(t, shading_strength));
    }
    if invert_colors {
        rgb = color::invert(rgb);
    }
    rgb
}

/// Shading of a fixed ink at tone `t`.
pub fn apply_shading(c: Rgb, t: f64, strength: f64) -> Rgb {
    color::shade(c, color::shading_factor(t, strength))
}

/// Channel inversion.
pub fn invert_color(c: Rgb) -> Rgb {
    color::invert(c)
}

/// The mosaic high-contrast colour map.
pub fn mosaic_map_color(c: Rgb, gain: f64) -> Rgb {
    color::mosaic_contrast(c, gain)
}

/// Prepare a decoded image with the job's preparation options.
pub fn prepare_from_rgba(
    img: &image::RgbaImage,
    opts: &ModernOptions,
) -> Result<PreparedImage, String> {
    prepare::prepare_image(img, &PrepareOptions::from_options(opts))
}

/// File-size guard (100 MB).
pub fn check_file_size(len: u64) -> Result<(), String> {
    if len > MAX_FILE_BYTES {
        Err("file is over the 100 MB limit".to_string())
    } else {
        Ok(())
    }
}

/// Decoded-pixel guard (100 MP).
pub fn check_pixels(w: u32, h: u32) -> Result<(), String> {
    if (w as u64) * (h as u64) > MAX_PIXELS {
        Err("image is over the 100 megapixel limit".to_string())
    } else {
        Ok(())
    }
}

/// Preview thinning factor for the requested detail level.
pub fn preview_factor(total_cells: u64, detail: PreviewDetail) -> u64 {
    preview::preview_stride(total_cells, detail.cell_cap()) as u64
}

/// Look up a built-in theme by id.
pub fn find_theme(id: &str) -> Option<Theme> {
    theme::theme(id)
}

/// Apply a built-in theme to an option set.
pub fn apply_theme(o: &mut ModernOptions, id: &str) {
    let _ = theme::apply_theme(id, o);
}