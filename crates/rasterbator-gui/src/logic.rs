//! Pure (window-free) logic for the modern GUI shell.
//!
//! Everything in here is unit-testable without an egui `Context`, so the
//! gui crate keeps coverage without ever opening a test window.

use std::path::{Path, PathBuf};

use rasterbator_core::modern::{
    JobGeometry, ModernOptions, PresetStore, PreviewDetail, RenderMode, Severity,
};

// ---------------------------------------------------------------------------
// App mode + preview tab
// ---------------------------------------------------------------------------

/// Which workspace the shell shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    Legacy,
    Modern,
}

impl AppMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AppMode::Legacy => "legacy",
            AppMode::Modern => "modern",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "legacy" => Some(AppMode::Legacy),
            "modern" => Some(AppMode::Modern),
            _ => None,
        }
    }
}

/// Central preview tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewTab {
    Poster,
    Sheet,
    Sheets,
}

impl PreviewTab {
    pub fn label(self) -> &'static str {
        match self {
            PreviewTab::Poster => "Poster",
            PreviewTab::Sheet => "Sheet",
            PreviewTab::Sheets => "Sheets",
        }
    }
}

// ---------------------------------------------------------------------------
// Preview detail (GUI-only setting, not part of `ModernOptions`)
// ---------------------------------------------------------------------------

/// Canonical preview cell caps (low/medium/high -> 100k/250k/1M).
pub fn preview_cell_cap(detail: PreviewDetail) -> u64 {
    detail.cell_cap()
}

/// Downscale factor for a job: integer `k` so the preview stays under the
/// cap. Re-exports the core policy for the GUI layer + tests.
pub fn preview_factor(total_cells: u64, detail: PreviewDetail) -> u64 {
    crate::compat::preview_factor(total_cells, detail)
}

pub fn preview_detail_label(detail: PreviewDetail) -> &'static str {
    match detail {
        PreviewDetail::Low => "low (100k cells)",
        PreviewDetail::Medium => "medium (250k cells)",
        PreviewDetail::High => "high (1M cells)",
    }
}

pub fn preview_detail_from_str(s: &str) -> Option<PreviewDetail> {
    PreviewDetail::parse_cli(s)
}

// ---------------------------------------------------------------------------
// Validation display mapping (Severity -> icon/label/colour)
// ---------------------------------------------------------------------------

/// How one severity renders in the bottom status strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagDisplay {
    /// Glyph prefix: blocking ▲ / warning ● / info ⓘ.
    pub icon: &'static str,
    /// Plain-language severity label.
    pub label: &'static str,
    /// sRGB status colour.
    pub color: (u8, u8, u8),
}

/// Map a [`Severity`] to its status-strip presentation.
pub fn diag_display(severity: Severity) -> DiagDisplay {
    match severity {
        Severity::Error => DiagDisplay { icon: "▲", label: "error", color: (200, 40, 40) },
        Severity::Warning => DiagDisplay { icon: "●", label: "warning", color: (190, 130, 0) },
        Severity::Info => DiagDisplay { icon: "ⓘ", label: "info", color: (120, 120, 120) },
    }
}

// ---------------------------------------------------------------------------
// Inert-control reasons
// ---------------------------------------------------------------------------
//
// A control is shown *disabled* only when its field is info-level inert
// (changing it cannot affect output). Fields tied to blocking errors stay
// editable so the user can fix the value; the status strip explains the
// error. `None` means editable.

/// Short reason a control for `field` must be shown disabled, if any.
///
/// `field` uses the `ModernOptions` field name. `geom` (when solved) feeds
/// the vector-mode check for the raster-only output controls.
pub fn inert_reason(
    field: &str,
    opts: &ModernOptions,
    geom: Option<&JobGeometry>,
) -> Option<&'static str> {
    // Plain screen ignores the whole pattern family (warning-level inert).
    if opts.screen == rasterbator_core::modern::ScreenFamily::Plain {
        match field {
            "pattern" | "stagger" | "coverage" | "mosaic_contrast" | "line_aspect"
            | "nominal_dot_mm" => return Some("inert on the plain screen"),
            _ => {}
        }
    }
    match field {
        "tone_exponent" if opts.tone_curve != rasterbator_core::modern::ToneCurve::Exponential => {
            Some("inert unless tone is exp")
        }
        "tone_log_k" if opts.tone_curve != rasterbator_core::modern::ToneCurve::Logarithmic => {
            Some("inert unless tone is log")
        }
        "dither_strength" | "dither_levels" | "dither_block"
            if opts.dither == rasterbator_core::modern::Dither::None =>
        {
            Some("inert when dither is none")
        }
        "shading_strength" if !opts.shading => Some("needs shading on"),
        "label_size_pt" | "label_offset_mm"
            if opts.labels == rasterbator_core::modern::LabelStyle::None =>
        {
            Some("needs coordinate labels on")
        }
        "corner_mark_mm" if opts.guides != rasterbator_core::modern::GuideStyle::Corners => {
            Some("needs corner guides")
        }
        "guide_width_pt" | "guide_color"
            if opts.guides == rasterbator_core::modern::GuideStyle::None =>
        {
            Some("needs guides on")
        }
        "resample_dpi" if !opts.resample => Some("needs resampling on"),
        "ink_color" if opts.ink != rasterbator_core::modern::InkMode::Fixed => {
            Some("only for fixed ink")
        }
        "fg_color" | "bg_color" if opts.ink != rasterbator_core::modern::InkMode::DualTone => {
            Some("only for dual-tone ink")
        }
        "dpi" | "jpeg_quality" | "compress" if resolved_vector(opts, geom) => {
            Some("inert in vector mode")
        }
        _ => None,
    }
}

/// Mirror of the core `Auto` render resolution for the inert check only.
fn resolved_vector(opts: &ModernOptions, geom: Option<&JobGeometry>) -> bool {
    match opts.render {
        RenderMode::Vector => true,
        RenderMode::Raster => false,
        RenderMode::Auto => {
            if let Some(g) = geom {
                g.render_mode == RenderMode::Vector
            } else {
                opts.screen != rasterbator_core::modern::ScreenFamily::Plain
                    && opts.dither == rasterbator_core::modern::Dither::None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Settings persistence
// ---------------------------------------------------------------------------
//
// The non-image `ModernOptions` fields travel through the core `PresetStore`
// under the file name "settings" (`settings.options`). The two GUI-only keys
// (`last_mode`/`remember`, `preview_detail`) live in a tiny hand-rolled
// sidecar (`settings.gui`) in the same directory: the preset text format
// rejects unknown keys, so they cannot share the file.

pub const SETTINGS_NAME: &str = "settings";

fn gui_sidecar_path(dir: &Path) -> PathBuf {
    dir.join("settings.gui")
}

/// Clear image-specific / session fields before persisting options.
pub fn sanitize_for_settings(opts: &ModernOptions, dir: &Path) -> ModernOptions {
    let mut o = opts.clone();
    o.crop = None;
    o.preset = None;
    o.save_preset = None;
    o.delete_preset = None;
    o.list_presets = false;
    o.preset_dir = dir.to_path_buf();
    o
}

/// GUI shell settings: options + mode memory + preview detail.
#[derive(Clone, Debug)]
pub struct UiSettings {
    pub opts: ModernOptions,
    pub last_mode: Option<AppMode>,
    pub remember: bool,
    pub preview_detail: PreviewDetail,
}

impl Default for UiSettings {
    fn default() -> Self {
        UiSettings {
            opts: ModernOptions::default(),
            last_mode: None,
            remember: false,
            preview_detail: PreviewDetail::Medium,
        }
    }
}

impl UiSettings {
    /// Load settings from `dir`; any missing/corrupt part falls back to its
    /// default without failing (the GUI must always start).
    pub fn load(dir: &Path) -> Self {
        let mut s = UiSettings::default();
        let store = PresetStore::new(dir.to_path_buf());
        if let Ok(opts) = store.load(SETTINGS_NAME) {
            let mut clean = opts;
            clean.crop = None;
            clean.preset = None;
            clean.save_preset = None;
            clean.delete_preset = None;
            s.opts = clean;
        }
        if let Ok(text) = std::fs::read_to_string(gui_sidecar_path(dir)) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    match k.trim() {
                        "last_mode" => {
                            s.last_mode = match v.trim() {
                                "none" => None,
                                other => AppMode::parse(other),
                            };
                        }
                        "remember" => s.remember = v.trim() == "true",
                        "preview_detail" => {
                            if let Some(d) = preview_detail_from_str(v.trim()) {
                                s.preview_detail = d;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        s
    }

    /// Persist settings to `dir` (PresetStore for the options, sidecar for
    /// the GUI keys). Creates the directory on demand.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let store = PresetStore::new(dir.to_path_buf());
        let clean = sanitize_for_settings(&self.opts, dir);
        store.save_settings(&clean).map_err(|d| d.message.clone())?;
        let body = format!(
            "# rasterbator gui settings (written automatically)\nlast_mode={}\nremember={}\npreview_detail={}\n",
            self.last_mode.map(|m| m.as_str()).unwrap_or("none"),
            if self.remember { "true" } else { "false" },
            self.preview_detail.cli_name(),
        );
        std::fs::write(gui_sidecar_path(dir), body).map_err(|e| e.to_string())
    }
}

/// CLI spelling helper for [`PreviewDetail`] (the core type only has
/// `parse_cli`; the canonical names live here for the sidecar writer).
trait PreviewDetailName {
    fn cli_name(self) -> &'static str;
}

impl PreviewDetailName for PreviewDetail {
    fn cli_name(self) -> &'static str {
        match self {
            PreviewDetail::Low => "low",
            PreviewDetail::Medium => "medium",
            PreviewDetail::High => "high",
        }
    }
}

// ---------------------------------------------------------------------------
// Small pure helpers for the workspace
// ---------------------------------------------------------------------------

/// Clamp a zoom factor to the workspace range.
pub fn clamp_zoom(zoom: f32) -> f32 {
    zoom.clamp(0.1, 8.0)
}

/// First page index shown on the Sheets tab for `total` pages at `offset`.
pub fn sheets_window(total_pages: u32, offset: u32) -> (u32, u32) {
    const PAGE: u32 = 64;
    let start = offset.min(total_pages.saturating_sub(1));
    let end = (start + PAGE).min(total_pages);
    (start, end)
}

/// Format a poster/sheet readout in mm.
pub fn format_mm(w: f64, h: f64) -> String {
    format!("{w:.1} × {h:.1} mm")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rasterbator_core::modern::{validate, SourceInfo};

    #[test]
    fn app_mode_round_trip() {
        assert_eq!(AppMode::parse("legacy"), Some(AppMode::Legacy));
        assert_eq!(AppMode::parse("Modern"), Some(AppMode::Modern));
        assert_eq!(AppMode::parse("nope"), None);
        assert_eq!(AppMode::Legacy.as_str(), "legacy");
        assert_eq!(AppMode::Modern.as_str(), "modern");
    }

    #[test]
    fn preview_detail_caps() {
        assert_eq!(preview_cell_cap(PreviewDetail::Low), 100_000);
        assert_eq!(preview_cell_cap(PreviewDetail::Medium), 250_000);
        assert_eq!(preview_cell_cap(PreviewDetail::High), 1_000_000);
        // Boundary behaviour of the shared downscale policy.
        assert_eq!(preview_factor(0, PreviewDetail::Medium), 1);
        assert_eq!(preview_factor(250_000, PreviewDetail::Medium), 1);
        assert_eq!(preview_factor(250_001, PreviewDetail::Medium), 2);
        assert_eq!(preview_factor(1_000_000, PreviewDetail::Medium), 2);
        assert_eq!(preview_factor(100_000, PreviewDetail::Low), 1);
        assert_eq!(preview_factor(100_001, PreviewDetail::Low), 2);
        assert_eq!(preview_detail_label(PreviewDetail::High), "high (1M cells)");
        assert_eq!(preview_detail_from_str("high"), Some(PreviewDetail::High));
        assert_eq!(preview_detail_from_str("bogus"), None);
    }

    #[test]
    fn validation_display_mapping() {
        let e = diag_display(Severity::Error);
        assert_eq!((e.icon, e.label), ("▲", "error"));
        let w = diag_display(Severity::Warning);
        assert_eq!((w.icon, w.label), ("●", "warning"));
        let i = diag_display(Severity::Info);
        assert_eq!((i.icon, i.label), ("ⓘ", "info"));
        // Colours are distinct per severity.
        assert_ne!(e.color, w.color);
        assert_ne!(w.color, i.color);

        // Wiring: a blocking report surfaces as an error display, a clean
        // report does not.
        let ok = ModernOptions::default();
        let src = SourceInfo::synthetic(4000, 3000);
        let geom = JobGeometry::solve(&ok, &src).unwrap();
        let rep = validate(&ok, Some(&src), Some(&geom));
        assert!(!rep.has_blocking());
        let mut bad = ModernOptions::default();
        bad.min_raster = 0.9;
        bad.max_raster = 0.1;
        let rep = validate(&bad, Some(&src), Some(&geom));
        assert!(rep.has_blocking());
        let first_err = rep.items().iter().find(|d| d.severity == Severity::Error).unwrap().clone();
        let d = diag_display(first_err.severity);
        assert_eq!(d.label, "error");
        assert!(first_err.hint.as_deref().is_some_and(|h| !h.is_empty()));
    }

    #[test]
    fn inert_reason_table() {
        let mut o = ModernOptions::default();
        // Tone params: only the active curve's knob is editable.
        assert!(inert_reason("tone_exponent", &o, None).is_some());
        assert!(inert_reason("tone_log_k", &o, None).is_some());
        o.tone_curve = rasterbator_core::modern::ToneCurve::Exponential;
        assert!(inert_reason("tone_exponent", &o, None).is_none());
        assert!(inert_reason("tone_log_k", &o, None).is_some());
        // Dither knobs inert without a dither algorithm...
        let o = ModernOptions::default();
        assert!(inert_reason("dither_strength", &o, None).is_some());
        assert!(inert_reason("dither_levels", &o, None).is_some());
        assert!(inert_reason("dither_block", &o, None).is_some());
        // ...but error-linked fields stay editable so the value can be fixed.
        assert!(inert_reason("mosaic_contrast", &o, None).is_none());
        assert!(inert_reason("line_aspect", &o, None).is_none());
        // Shading / labels / guides / resample gating.
        assert!(inert_reason("shading_strength", &o, None).is_some());
        assert!(inert_reason("label_size_pt", &o, None).is_some());
        assert!(inert_reason("label_offset_mm", &o, None).is_some());
        assert!(inert_reason("corner_mark_mm", &o, None).is_none()); // corners default on
        assert!(inert_reason("resample_dpi", &o, None).is_none());
        let mut o2 = ModernOptions::default();
        o2.resample = false;
        assert!(inert_reason("resample_dpi", &o2, None).is_some());
        // Ink-gated colours.
        assert!(inert_reason("ink_color", &o, None).is_none()); // fixed default
        assert!(inert_reason("fg_color", &o, None).is_some());
        o2.ink = rasterbator_core::modern::InkMode::DualTone;
        assert!(inert_reason("fg_color", &o2, None).is_none());
        assert!(inert_reason("ink_color", &o2, None).is_some());
        // Plain screen parks the pattern family.
        o2.screen = rasterbator_core::modern::ScreenFamily::Plain;
        assert!(inert_reason("pattern", &o2, None).is_some());
        assert!(inert_reason("nominal_dot_mm", &o2, None).is_some());
        // Vector mode parks the raster-only output knobs.
        let mut o3 = ModernOptions::default();
        o3.render = RenderMode::Vector;
        assert!(inert_reason("dpi", &o3, None).is_some());
        assert!(inert_reason("jpeg_quality", &o3, None).is_some());
        assert!(inert_reason("compress", &o3, None).is_some());
        o3.render = RenderMode::Raster;
        assert!(inert_reason("dpi", &o3, None).is_none());
    }

    #[test]
    fn settings_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "rasterbator-gui-settings-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = UiSettings::default();
        s.opts.nominal_dot_mm = 15.0;
        s.opts.crop = Some(rasterbator_core::modern::CropRectPx {
            x1: 1,
            y1: 2,
            x2: 3,
            y2: 4,
        });
        s.last_mode = Some(AppMode::Modern);
        s.remember = true;
        s.preview_detail = PreviewDetail::High;
        s.save(&dir).unwrap();
        // Options travelled through the real PresetStore file...
        let store = PresetStore::new(dir.clone());
        assert!(store.exists(SETTINGS_NAME));
        // ...crop stripped (non-image settings only)...
        let back = UiSettings::load(&dir);
        assert_eq!(back.opts.nominal_dot_mm, 15.0);
        assert_eq!(back.opts.crop, None);
        assert_eq!(back.last_mode, Some(AppMode::Modern));
        assert!(back.remember);
        assert_eq!(back.preview_detail, PreviewDetail::High);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_fallbacks() {
        // Missing directory -> defaults, no panic.
        let dir = std::env::temp_dir().join(format!(
            "rasterbator-gui-missing-{}-{}",
            std::process::id(),
            "noload"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let s = UiSettings::load(&dir);
        assert_eq!(s.last_mode, None);
        assert!(!s.remember);
        assert_eq!(s.preview_detail, PreviewDetail::Medium);
        // Corrupt sidecar -> options still load, detail falls back.
        let dir2 = std::env::temp_dir().join(format!(
            "rasterbator-gui-corrupt-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir2);
        UiSettings::default().save(&dir2).unwrap();
        std::fs::write(dir2.join("settings.gui"), "preview_detail=bogus\nremember=yes\n").unwrap();
        let s2 = UiSettings::load(&dir2);
        assert_eq!(s2.preview_detail, PreviewDetail::Medium);
        assert!(!s2.remember);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[test]
    fn sheets_window_and_zoom() {
        assert_eq!(sheets_window(18, 0), (0, 18));
        assert_eq!(sheets_window(200, 0), (0, 64));
        assert_eq!(sheets_window(200, 190), (190, 200));
        assert_eq!(sheets_window(0, 0), (0, 0));
        assert_eq!(clamp_zoom(100.0), 8.0);
        assert_eq!(clamp_zoom(0.0), 0.1);
        assert_eq!(clamp_zoom(1.5), 1.5);
        assert_eq!(format_mm(1050.0, 787.5), "1050.0 × 787.5 mm");
    }
}
