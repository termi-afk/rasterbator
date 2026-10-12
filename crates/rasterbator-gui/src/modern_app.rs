//! Modern workspace: full option surface with live preview.
//!
//! Layout per the GUI surface design: left options panel (collapsing
//! groups over every `ModernOptions` field), right read-out panel fed from
//! the canonical `JobGeometry` only, central Poster/Sheet/Sheets tabs with
//! zoom/pan/fit, bottom status strip (validation + progress + cancel).
//!
//! Deviations from the design draft, all forced by the actual core API in
//! this working copy:
//! - The core `BitmapTarget` is counting-only (no rasterisation), so the
//!   preview worker rasterises cells itself through the canonical
//!   `CellSampler` + `transfer_opts` + `resolve_ink` over the canonical
//!   `JobGeometry` with the core `preview_factor` downscale policy. Layout
//!   is never recomputed in the GUI.
//! - Non-dot patterns are drawn as circles in the coarse previews
//!   (Square/Mosaic as rects); the Sheet tab draws the exact cell window.
//! - The core has no JPEG-tile raster path yet (`page.rs` emits vector
//!   shapes; `encode_jpeg_rgb` has no caller), so export is the vector
//!   session in all modes. Preview and export still share one `JobGeometry`.
//! - Pan is via scrollbars; zoom via Fit / − / + / slider.

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::time::{Duration, Instant};

use eframe::egui;
use rasterbator_core::modern::options::default_preset_dir;
use rasterbator_core::modern::options::PrintScaling;
use rasterbator_core::modern::pdf::PdfTarget;
use rasterbator_core::modern::strokefont::page_label;
use rasterbator_core::modern::tone::{extent_mm, plain_grey};
use rasterbator_core::modern::{
    validate, JobGeometry, ModernOptions, ModernPdf, PreparedImage, PresetStore, PreviewDetail,
    Progress, RenderError, RenderSession, Rgb, Severity, SourceInfo, ValidationReport,
};

use crate::compat::{
    apply_shading, apply_theme, cell_centre, cell_sampler, check_file_size, check_pixels, dual_blend,
    find_theme, invert_color, mosaic_map_color, prepare_from_rgba, resolve_ink, transfer_opts, THEMES,
};

use crate::logic::{
    clamp_zoom, diag_display, format_mm, inert_reason, preview_cell_cap, preview_detail_label,
    preview_factor, sheets_window, PreviewTab, UiSettings,
};
/// The GUI's own preview frame: poster and sheet RGB buffers.
#[derive(Clone)]
pub struct PreviewFrame {
    /// Poster buffer width in pixels.
    pub poster_w: u32,
    /// Poster buffer height in pixels.
    pub poster_h: u32,
    /// Poster RGB8, row-major.
    pub poster: Vec<u8>,
    /// Sheet buffer width in pixels.
    pub sheet_w: u32,
    /// Sheet buffer height in pixels.
    pub sheet_h: u32,
    /// Sheet RGB8, row-major.
    pub sheet: Vec<u8>,
    /// Ink coverage estimate in percent.
    pub ink_percent: f64,
}

// ---------------------------------------------------------------------------
// Worker messages
// ---------------------------------------------------------------------------

enum PreviewMsg {
    Ready(PreviewFrame),
    Failed(String),
}

const CANCELLED_MARKER: &str = "preview cancelled";

/// Summary reported after a successful export.
#[derive(Clone, Debug)]
pub struct ExportReport {
    pub bytes: u64,
    pub pages: u32,
    pub cells_drawn: u64,
    pub ink_percent: f64,
}

enum ExportMsg {
    Progress(Progress),
    Done(Result<ExportReport, String>),
    Cancelled,
}

struct PreviewWorker {
    rx: Receiver<PreviewMsg>,
    cancel: Arc<AtomicBool>,
}

struct ExportWorker {
    rx: Receiver<ExportMsg>,
    cancel: Arc<AtomicBool>,
}

// ---------------------------------------------------------------------------
// Coarse preview raster (worker thread)
// ---------------------------------------------------------------------------

fn pattern_is_rect(opts: &ModernOptions) -> bool {
    matches!(
        opts.pattern,
        rasterbator_core::modern::Pattern::Square
            | rasterbator_core::modern::Pattern::Mosaic
    )
}

fn rects_overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

/// Preview colour + half-extent for one sampled cell, mirroring `page.rs`.
fn preview_cell_appearance(
    opts: &ModernOptions,
    geom: &JobGeometry,
    sampled: Rgb,
    level: f64,
    t: f64,
) -> (Rgb, f64) {
    use rasterbator_core::modern::{InkMode, ScreenFamily};
    if opts.screen == ScreenFamily::Plain {
        let g = plain_grey(t);
        let rgb = match opts.ink {
            InkMode::Grayscale => Rgb(g, g, g),
            InkMode::Average => sampled,
            InkMode::DualTone => {
                let u = dual_blend(t, opts.dual_softness);
                if u >= 0.5 {
                    opts.fg_color
                } else {
                    opts.bg_color
                }
            }
            _ => {
                let mut c = opts.ink_color;
                if opts.shading {
                    c = apply_shading(c, t, opts.shading_strength);
                }
                if opts.invert_colors {
                    c = invert_color(c);
                }
                c
            }
        };
        return (rgb, f64::INFINITY);
    }
    let u = dual_blend(t, opts.dual_softness);
    let _qu = if opts.ink == InkMode::DualTone {
        if u >= 0.5 {
            1.0
        } else {
            0.0
        }
    } else {
        t
    };
    let mut rgb = resolve_ink(
        opts.ink,
        opts.ink_color,
        opts.fg_color,
        opts.bg_color,
        sampled,
        level,
        t,
        opts.dual_softness,
        opts.shading,
        opts.shading_strength,
        opts.invert_colors,
    );
    if opts.pattern == rasterbator_core::modern::Pattern::Mosaic
        && (opts.mosaic_contrast - 1.0).abs() > 1e-12
    {
        rgb = mosaic_map_color(rgb, opts.mosaic_contrast);
    }
    // Export extent at the export pitch; callers scale it to the preview
    // lattice so dots stay proportional.
    let e = extent_mm(t, geom.pitch_mm, geom.family_scale);
    (rgb, e)
}

/// Build the poster + sheet preview buffers on the worker thread.
fn render_preview_coarse(
    prepared: &PreparedImage,
    opts: &ModernOptions,
    geom: &JobGeometry,
    detail: PreviewDetail,
    sheet_page: u32,
    cancel: &AtomicBool,
) -> Result<PreviewFrame, String> {
    let paper = opts.paper_color;
    let paper_arr = [paper.0, paper.1, paper.2];
    // --- poster (downscaled lattice) ---
    let k = preview_factor(geom.total_cells, detail).max(1);
    let pitch = geom.pitch_mm * k as f64;
    let base_cell_h = geom.cell_area_mm2 / geom.pitch_mm.max(1e-9);
    let cell_h = base_cell_h * k as f64;
    let cols = ((geom.cover.w / pitch).ceil() as u32).max(1);
    let rows = ((geom.cover.h / cell_h).ceil() as u32).max(1);
    let long = geom.cover.w.max(geom.cover.h).max(1.0);
    let scale = (420.0 / long).min(4.0);
    let pw = ((geom.cover.w * scale).ceil() as u32).max(1).min(2048);
    let ph = ((geom.cover.h * scale).ceil() as u32).max(1).min(2048);
    let mut poster = vec![0u8; (pw as usize) * (ph as usize) * 3];
    for px in poster.chunks_exact_mut(3) {
        px.copy_from_slice(&paper_arr);
    }
    let (ink_px, _visited) = raster_cells_scaled(
        &mut poster,
        pw,
        ph,
        cols,
        rows,
        pitch,
        cell_h,
        geom.cover.w,
        geom.cover.h,
        prepared,
        opts,
        geom,
        cancel,
    )?;
    let total_px = (pw as u64) * (ph as u64);
    let ink_percent = if total_px > 0 {
        100.0 * ink_px as f64 / total_px as f64
    } else {
        0.0
    };
    // --- sheet (exact cell window, full resolution) ---
    let page = sheet_page.min(geom.total_pages.saturating_sub(1));
    let win = geom.cell_window(page).clone();
    let wcols = (win.i1 - win.i0).max(1) as u32;
    let wrows = (win.j1 - win.j0).max(1) as u32;
    let win_w_mm = wcols as f64 * geom.pitch_mm;
    let win_h_mm = wrows as f64 * base_cell_h;
    let s2 = (520.0 / win_w_mm.max(win_h_mm).max(1.0)).min(8.0);
    let sw = ((win_w_mm * s2).ceil() as u32).max(1).min(2048);
    let sh = ((win_h_mm * s2).ceil() as u32).max(1).min(2048);
    let mut sheet = vec![0u8; (sw as usize) * (sh as usize) * 3];
    for px in sheet.chunks_exact_mut(3) {
        px.copy_from_slice(&paper_arr);
    }
    raster_window(
        &mut sheet,
        sw,
        sh,
        &win,
        prepared,
        opts,
        geom,
        base_cell_h,
        cancel,
    )?;
    Ok(PreviewFrame {
        poster_w: pw,
        poster_h: ph,
        poster,
        sheet_w: sw,
        sheet_h: sh,
        sheet,
        ink_percent,
    })
}

/// Poster raster with an explicit cover-mm mapping.
#[allow(clippy::too_many_arguments)]
fn raster_cells_scaled(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    cols: u32,
    rows: u32,
    pitch: f64,
    cell_h: f64,
    cover_w: f64,
    cover_h: f64,
    prepared: &PreparedImage,
    opts: &ModernOptions,
    geom: &JobGeometry,
    cancel: &AtomicBool,
) -> Result<(u64, u64), String> {
    let mut po = opts.clone();
    po.nominal_dot_mm = pitch;
    let sx = buf_w as f64 / cover_w;
    let sy = buf_h as f64 / cover_h;
    let sampler = cell_sampler(prepared, opts, geom.source_rect_mm);
    let rects = pattern_is_rect(opts);
    let k = pitch / geom.pitch_mm.max(1e-9);
    let mut ink_px: u64 = 0;
    let mut since_check = 0u32;
    for j in 0..rows {
        for i in 0..cols {
            since_check += 1;
            if since_check >= 2048 {
                since_check = 0;
                if cancel.load(Ordering::Relaxed) {
                    return Err(CANCELLED_MARKER.to_string());
                }
            }
            let (cx, cy) = cell_centre(i as i64, j as i64, &po);
            let bx0 = cx - pitch / 2.0;
            let by0 = cy - cell_h / 2.0;
            let bx1 = cx + pitch / 2.0;
            let by1 = cy + cell_h / 2.0;
            if bx1 <= 0.0 || by1 <= 0.0 || bx0 >= cover_w || by0 >= cover_h {
                continue;
            }
            if !rects_overlap([bx0, by0, bx1, by1], geom.source_rect_mm) {
                continue;
            }
            let v = sampler.sample([bx0, by0, bx1, by1]);
            let t = transfer_opts(v.level, opts);
            let (rgb, extent) = preview_cell_appearance(opts, geom, v.rgb(), v.level, t);
            let e = if extent.is_infinite() { pitch / 2.0 } else { extent * k };
            if e <= 0.0 {
                continue;
            }
            paint_cell(buf, buf_w, buf_h, cx * sx, cy * sy, e * sx, e * sy, rgb, rects, &mut ink_px);
        }
    }
    Ok((ink_px, 0))
}

/// Sheet raster over one exact cell window at full resolution.
#[allow(clippy::too_many_arguments)]
fn raster_window(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    win: &rasterbator_core::modern::CellWindow,
    prepared: &PreparedImage,
    opts: &ModernOptions,
    geom: &JobGeometry,
    cell_h: f64,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let wcols = (win.i1 - win.i0).max(1) as u32;
    let wrows = (win.j1 - win.j0).max(1) as u32;
    let sx = buf_w as f64 / (wcols as f64 * geom.pitch_mm);
    let sy = buf_h as f64 / (wrows as f64 * cell_h);
    let x0_mm = win.i0 as f64 * geom.pitch_mm;
    let y0_mm = win.j0 as f64 * cell_h;
    let sampler = cell_sampler(prepared, opts, geom.source_rect_mm);
    let rects = pattern_is_rect(opts);
    let mut ink_px: u64 = 0;
    let mut since_check = 0u32;
    for jj in 0..wrows {
        for ii in 0..wcols {
            since_check += 1;
            if since_check >= 1024 {
                since_check = 0;
                if cancel.load(Ordering::Relaxed) {
                    return Err(CANCELLED_MARKER.to_string());
                }
            }
            let i = win.i0 + ii as i64;
            let j = win.j0 + jj as i64;
            let (cx, cy) = cell_centre(i, j, opts);
            let p = geom.pitch_mm;
            let bx0 = cx - p / 2.0;
            let by0 = cy - cell_h / 2.0;
            let bx1 = cx + p / 2.0;
            let by1 = cy + cell_h / 2.0;
            if !rects_overlap([bx0, by0, bx1, by1], geom.source_rect_mm) {
                continue;
            }
            let v = sampler.sample([bx0, by0, bx1, by1]);
            let t = transfer_opts(v.level, opts);
            let (rgb, extent) = preview_cell_appearance(opts, geom, v.rgb(), v.level, t);
            let e = if extent.is_infinite() { p / 2.0 } else { extent };
            if e <= 0.0 {
                continue;
            }
            paint_cell(
                buf,
                buf_w,
                buf_h,
                (cx - x0_mm) * sx,
                (cy - y0_mm) * sy,
                e * sx,
                e * sy,
                rgb,
                rects,
                &mut ink_px,
            );
        }
    }
    Ok(())
}

fn paint_cell(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    ccx: f64,
    ccy: f64,
    ex: f64,
    ey: f64,
    rgb: Rgb,
    rects: bool,
    ink_px: &mut u64,
) {
    let x0 = (ccx - ex).floor() as i64;
    let x1 = (ccx + ex).ceil() as i64;
    let y0 = (ccy - ey).floor() as i64;
    let y1 = (ccy + ey).ceil() as i64;
    let ex = ex.max(1e-9);
    let ey = ey.max(1e-9);
    for y in y0.max(0)..y1.min(buf_h as i64) {
        for x in x0.max(0)..x1.min(buf_w as i64) {
            let inside = if rects {
                (x as f64 + 0.5 - ccx).abs() <= ex && (y as f64 + 0.5 - ccy).abs() <= ey
            } else {
                let dx = (x as f64 + 0.5 - ccx) / ex;
                let dy = (y as f64 + 0.5 - ccy) / ey;
                dx * dx + dy * dy <= 1.0
            };
            if inside {
                let o = (y as usize * buf_w as usize + x as usize) * 3;
                buf[o] = rgb.0;
                buf[o + 1] = rgb.1;
                buf[o + 2] = rgb.2;
                *ink_px += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Export worker (shared canonical geometry -> RenderSession -> ModernPdf)
// ---------------------------------------------------------------------------

/// Temp path used by the atomic save protocol (mirrors `ModernPdf::save`).
fn export_tmp_path(out: &std::path::Path) -> PathBuf {
    out.with_extension(format!("tmp{}", std::process::id()))
}

fn run_export_job(
    prepared: Arc<PreparedImage>,
    opts: ModernOptions,
    geom: Arc<JobGeometry>,
    out: PathBuf,
    tx: Sender<ExportMsg>,
    cancel: Arc<AtomicBool>,
) {
    let send_cancel = |tx: &Sender<ExportMsg>, out: &PathBuf| {
        let _ = std::fs::remove_file(export_tmp_path(out));
        let _ = tx.send(ExportMsg::Cancelled);
    };
    let mut session = RenderSession::new(&prepared, &opts, &geom, &cancel);
    let mut pdf = ModernPdf::new(&opts, &geom);
    let total = geom.total_pages;
    {
        let mut target = PdfTarget::new(&mut pdf);
        for _ in 0..total {
            if cancel.load(Ordering::Relaxed) {
                send_cancel(&tx, &out);
                return;
            }
            match session.render_next(&mut target) {
                Err(RenderError::Cancelled) => {
                    send_cancel(&tx, &out);
                    return;
                }
                Err(e) => {
                    let _ = tx.send(ExportMsg::Done(Err(e.to_string())));
                    return;
                }
                Ok(_) => {}
            }
            let _ = tx.send(ExportMsg::Progress(session.progress()));
        }
        target.finish();
    }
    if cancel.load(Ordering::Relaxed) {
        send_cancel(&tx, &out);
        return;
    }
    let cells = session.stats().cells_drawn;
    match pdf.save(&out, opts.atomic) {
        Ok(bytes) => {
            let ink = session.stats().ink_density_percent();
            let _ = tx.send(ExportMsg::Done(Ok(ExportReport {
                bytes,
                pages: total,
                cells_drawn: cells,
                ink_percent: ink,
            })));
        }
        Err(e) => {
            let _ = tx.send(ExportMsg::Done(Err(e)));
        }
    }
}

// ---------------------------------------------------------------------------
// Control helpers (free functions so `&mut opts` never aliases `&self`)
// ---------------------------------------------------------------------------

/// Show a control disabled with its reason when its field is inert.
/// Returns true when the control changed the options.
fn inert_scope(
    opts: &mut ModernOptions,
    geom: Option<&JobGeometry>,
    ui: &mut egui::Ui,
    field: &str,
    add: impl FnOnce(&mut egui::Ui, &mut ModernOptions) -> bool,
) -> bool {
    let reason = inert_reason(field, opts, geom);
    let mut out = false;
    ui.add_enabled_ui(reason.is_none(), |ui| {
        out = add(ui, opts);
    });
    if let Some(r) = reason {
        ui.label(egui::RichText::new(format!("({r})")).small().weak());
    }
    out
}

fn combo_row<T>(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    value: &mut T,
    all: &[T],
    def: &str,
) -> bool
where
    T: Copy + PartialEq + std::fmt::Display,
{
    let mut out = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let mut v = *value;
        egui::ComboBox::from_id_salt(id)
            .selected_text(v.to_string())
            .show_ui(ui, |ui| {
                for a in all {
                    if ui.selectable_value(&mut v, *a, a.to_string()).changed() {
                        out = true;
                    }
                }
            });
        *value = v;
        ui.label(egui::RichText::new(format!("default: {def}")).small().weak());
    });
    out
}

fn drag_row<N>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut N,
    range: std::ops::RangeInclusive<f64>,
    def: &str,
) -> bool
where
    N: egui::emath::Numeric,
{
    let mut out = false;
    ui.horizontal(|ui| {
        ui.label(label);
        if ui
            .add(egui::DragValue::new(value).range(range).speed(0.1))
            .changed()
        {
            out = true;
        }
        ui.label(egui::RichText::new(format!("default: {def}")).small().weak());
    });
    out
}

fn slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    def: &str,
) -> bool {
    let mut out = false;
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.add(egui::Slider::new(value, range)).changed() {
            out = true;
        }
        ui.label(egui::RichText::new(format!("default: {def}")).small().weak());
    });
    out
}

fn color_row(ui: &mut egui::Ui, label: &str, value: &mut Rgb, reason: Option<&str>) -> bool {
    let mut out = false;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add_enabled_ui(reason.is_none(), |ui| {
            let mut arr = [value.0, value.1, value.2];
            if egui::color_picker::color_edit_button_srgb(ui, &mut arr).changed() {
                *value = Rgb(arr[0], arr[1], arr[2]);
                out = true;
            }
        });
        if let Some(r) = reason {
            ui.label(egui::RichText::new(format!("({r})")).small().weak());
        }
    });
    out
}

fn valid_preset_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn format_seconds(seconds: f64) -> String {
    let s = seconds.round() as u64;
    if s >= 60 {
        format!("{} min {} s", s / 60, s % 60)
    } else {
        format!("{s} s")
    }
}

/// Nearest-neighbour crop + downscale of an RGB region to `max_w` wide.
fn downsample_region(
    src: &[u8],
    sw: u32,
    sh: u32,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    max_w: u32,
) -> (usize, usize, Vec<u8>) {
    let x1 = (x0 + w).min(sw).max(x0 + 1);
    let y1 = (y0 + h).min(sh).max(y0 + 1);
    let w = x1 - x0;
    let h = y1 - y0;
    let k = ((w as f64 / max_w as f64).ceil() as u32).max(1);
    let tw = ((w + k - 1) / k).max(1) as usize;
    let th = ((h + k - 1) / k).max(1) as usize;
    let mut out = vec![0u8; tw * th * 3];
    for y in 0..th {
        for x in 0..tw {
            let sx = (x0 + (x as u32) * k).min(sw - 1) as usize;
            let sy = (y0 + (y as u32) * k).min(sh - 1) as usize;
            let si = (sy * sw as usize + sx) * 3;
            let di = (y * tw + x) * 3;
            out[di..di + 3].copy_from_slice(&src[si..si + 3]);
        }
    }
    (tw, th, out)
}

// ---------------------------------------------------------------------------
// ModernApp
// ---------------------------------------------------------------------------

pub struct ModernApp {
    opts: ModernOptions,
    pub detail: PreviewDetail,
    store: PresetStore,

    source_path: String,
    decoded: Option<image::RgbaImage>,
    image_error: Option<String>,
    prepared: Option<Arc<PreparedImage>>,
    prep_key: String,
    geom: Option<Arc<JobGeometry>>,
    report: ValidationReport,

    preview_frame: Option<PreviewFrame>,
    frame_id: u64,
    poster_tex: Option<egui::TextureHandle>,
    sheet_tex: Option<egui::TextureHandle>,
    thumbs: Vec<egui::TextureHandle>,
    thumbs_key: (u64, u32, u32),

    preview_tab: PreviewTab,
    sheet_page: u32,
    sheets_offset: u32,
    zoom: f32,

    preview_worker: Option<PreviewWorker>,
    pending_at: Option<Instant>,
    export_worker: Option<ExportWorker>,
    export_progress: Option<Progress>,
    export_started: Option<Instant>,
    export_out: Option<PathBuf>,

    preset_names: Vec<String>,
    preset_sel: String,
    save_name: String,
    confirm_delete: bool,

    crop_text: String,
    paper_custom: String,
    size_n: f64,
    exact_w: f64,
    exact_h: f64,

    status_msg: String,
    pub request_switch: bool,
}

impl ModernApp {
    pub fn new(settings: &UiSettings) -> Self {
        let store = PresetStore::new(default_preset_dir());
        let preset_names = store.list().map(|v| v.into_iter().map(|e| e.name).collect()).unwrap_or_default();
        let mut app = ModernApp {
            opts: settings.opts.clone(),
            detail: settings.preview_detail,
            store,
            source_path: String::new(),
            decoded: None,
            image_error: None,
            prepared: None,
            prep_key: String::new(),
            geom: None,
            report: ValidationReport::default(),
            preview_frame: None,
            frame_id: 0,
            poster_tex: None,
            sheet_tex: None,
            thumbs: Vec::new(),
            thumbs_key: (0, 0, 0),
            preview_tab: PreviewTab::Poster,
            sheet_page: 0,
            sheets_offset: 0,
            zoom: 1.0,
            preview_worker: None,
            pending_at: None,
            export_worker: None,
            export_progress: None,
            export_started: None,
            export_out: None,
            preset_names,
            preset_sel: String::new(),
            save_name: String::new(),
            confirm_delete: false,
            crop_text: settings.opts.crop.map(|c| c.to_token()).unwrap_or_default(),
            paper_custom: String::from("210x297"),
            size_n: match settings.opts.size_spec {
                rasterbator_core::modern::SizeSpec::WidthSheets(n) => n,
                rasterbator_core::modern::SizeSpec::HeightSheets(n) => n,
                _ => 5.0,
            },
            exact_w: 900.0,
            exact_h: 600.0,
            status_msg: String::from("Pick a source image to begin."),
            request_switch: false,
        };
        app.refresh();
        app
    }

    /// Snapshot for settings persistence.
    pub fn snapshot(&self) -> (ModernOptions, PreviewDetail) {
        (self.opts.clone(), self.detail)
    }

    // -- pipeline ----------------------------------------------------------

    fn src_info(&self) -> Option<SourceInfo> {
        self.decoded
            .as_ref()
            .map(|d| SourceInfo::synthetic(d.width(), d.height()))
    }

    fn prep_fingerprint(&self) -> String {
        let o = &self.opts;
        format!(
            "{}x{}|{:?}|{:?}|{}|{}|{}|{}|{}|{}|{:?}",
            self.decoded.as_ref().map(|d| d.width()).unwrap_or(0),
            self.decoded.as_ref().map(|d| d.height()).unwrap_or(0),
            o.rotate,
            o.crop,
            o.brightness,
            o.contrast,
            o.saturation,
            o.alpha_threshold,
            o.resample,
            o.resample_dpi,
            o.paper_color,
        )
    }

    /// Re-prepare (only when preparation inputs changed), re-solve the
    /// canonical geometry, re-validate, and schedule a debounced preview.
    fn refresh(&mut self) {
        // 1. prepare
        if self.decoded.is_some() {
            let key = self.prep_fingerprint();
            if key != self.prep_key {
                self.prep_key = key;
                let res = self
                    .decoded
                    .as_ref()
                    .map(|d| prepare_from_rgba(d, &self.opts));
                match res {
                    Some(Ok(p)) => {
                        self.prepared = Some(Arc::new(p));
                        self.status_msg.clear();
                    }
                    Some(Err(e)) => {
                        self.prepared = None;
                        self.status_msg = format!("prepare failed: {e}");
                    }
                    None => self.prepared = None,
                }
            }
        } else {
            self.prepared = None;
            self.prep_key.clear();
        }
        // 2. canonical geometry (shared with export; never recomputed elsewhere)
        let src = self.src_info();
        self.geom = match (&src, self.prepared.is_some()) {
            (Some(s), true) => match JobGeometry::solve(&self.opts, s) {
                Ok(g) => Some(Arc::new(g)),
                Err(_) => None,
            },
            _ => None,
        };
        // 3. validation (single source of diagnostics)
        self.report = validate(&self.opts, src.as_ref(), self.geom.as_deref());
        // 4. debounce the preview worker
        if self.prepared.is_some() && self.geom.is_some() {
            self.pending_at = Some(Instant::now() + Duration::from_millis(180));
        } else {
            self.pending_at = None;
            self.preview_frame = None;
            self.poster_tex = None;
            self.sheet_tex = None;
            self.thumbs.clear();
        }
        if let Some(g) = &self.geom {
            self.sheet_page = self.sheet_page.min(g.total_pages.saturating_sub(1));
        }
    }

    fn load_image(&mut self) {
        self.decoded = None;
        self.image_error = None;
        if self.source_path.trim().is_empty() {
            self.refresh();
            return;
        }
        let path = PathBuf::from(self.source_path.trim());
        let img = (|| -> Result<image::RgbaImage, String> {
            let meta = std::fs::metadata(&path).map_err(|e| format!("cannot read input: {e}"))?;
            check_file_size(meta.len())?;
            let img = image::open(&path)
                .map_err(|e| format!("cannot decode input: {e}"))?
                .to_rgba8();
            check_pixels(img.width(), img.height())?;
            Ok(img)
        })();
        match img {
            Ok(d) => {
                self.decoded = Some(d);
                self.prep_key.clear(); // force re-prepare
                self.status_msg = String::from("Image loaded; preview rendering…");
            }
            Err(e) => self.image_error = Some(e),
        }
        self.refresh();
    }

    // -- workers -----------------------------------------------------------

    fn poll_workers(&mut self, ctx: &egui::Context) {
        // preview
        let mut got: Vec<PreviewMsg> = Vec::new();
        if let Some(w) = &self.preview_worker {
            while let Ok(m) = w.rx.try_recv() {
                got.push(m);
            }
        }
        let mut preview_done = false;
        for m in got {
            match m {
                PreviewMsg::Ready(f) => {
                    self.preview_frame = Some(f);
                    self.frame_id += 1;
                    self.upload_preview_textures(ctx);
                    self.status_msg.clear();
                }
                PreviewMsg::Failed(e) => {
                    if e != CANCELLED_MARKER {
                        self.status_msg = format!("preview failed: {e}");
                    }
                }
            }
            preview_done = true;
        }
        if preview_done {
            self.preview_worker = None;
        }
        // export
        let mut emsgs: Vec<ExportMsg> = Vec::new();
        if let Some(w) = &self.export_worker {
            while let Ok(m) = w.rx.try_recv() {
                emsgs.push(m);
            }
        }
        for m in emsgs {
            match m {
                ExportMsg::Progress(p) => self.export_progress = Some(p),
                ExportMsg::Done(r) => {
                    self.export_worker = None;
                    self.export_progress = None;
                    match r {
                        Ok(rep) => {
                            self.status_msg = format!(
                                "Wrote {}: {} pages, {} cells, ink {:.1} %, {} bytes",
                                self.export_out
                                    .as_ref()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_default(),
                                rep.pages,
                                rep.cells_drawn,
                                rep.ink_percent,
                                rep.bytes
                            );
                        }
                        Err(e) => self.status_msg = format!("export failed: {e}"),
                    }
                    ctx.request_repaint();
                }
                ExportMsg::Cancelled => {
                    self.export_worker = None;
                    self.export_progress = None;
                    self.status_msg = String::from("Export cancelled; no partial file remains.");
                    ctx.request_repaint();
                }
            }
        }
        if self.preview_worker.is_some() || self.export_worker.is_some() || self.pending_at.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }

    fn maybe_spawn_preview(&mut self) {
        let due = matches!(self.pending_at, Some(t) if Instant::now() >= t);
        if !due || self.preview_worker.is_some() || self.export_worker.is_some() {
            return;
        }
        let prep = self.prepared.clone();
        let geom = self.geom.clone();
        let (Some(prep), Some(geom)) = (prep, geom) else {
            return;
        };
        self.pending_at = None;
        let (tx, rx): (Sender<PreviewMsg>, Receiver<PreviewMsg>) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let opts = self.opts.clone();
        let detail = self.detail;
        let page = self.sheet_page;
        let c2 = cancel.clone();
        std::thread::spawn(move || {
            match render_preview_coarse(&prep, &opts, &geom, detail, page, &c2) {
                Ok(f) => {
                    let _ = tx.send(PreviewMsg::Ready(f));
                }
                Err(e) => {
                    let _ = tx.send(PreviewMsg::Failed(e));
                }
            }
        });
        self.preview_worker = Some(PreviewWorker { rx, cancel });
    }

    fn cancel_preview(&mut self) {
        if let Some(w) = &self.preview_worker {
            w.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn start_export(&mut self, out: PathBuf) {
        let prep = self.prepared.clone();
        let geom = self.geom.clone();
        let (Some(prep), Some(geom)) = (prep, geom) else {
            return;
        };
        self.cancel_preview();
        let (tx, rx): (Sender<ExportMsg>, Receiver<ExportMsg>) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let opts = self.opts.clone();
        let c2 = cancel.clone();
        let out2 = out.clone();
        std::thread::spawn(move || run_export_job(prep, opts, geom, out2, tx, c2));
        self.export_worker = Some(ExportWorker { rx, cancel });
        self.export_progress = None;
        self.export_started = Some(Instant::now());
        self.export_out = Some(out);
        self.status_msg = String::from("Exporting…");
    }

    fn upload_preview_textures(&mut self, ctx: &egui::Context) {
        if let Some(f) = &self.preview_frame {
            let poster = egui::ColorImage::from_rgb(
                [f.poster_w as usize, f.poster_h as usize],
                f.poster.as_slice(),
            );
            self.poster_tex =
                Some(ctx.load_texture("modern-poster", poster, egui::TextureOptions::NEAREST));
            let sheet = egui::ColorImage::from_rgb(
                [f.sheet_w as usize, f.sheet_h as usize],
                f.sheet.as_slice(),
            );
            self.sheet_tex =
                Some(ctx.load_texture("modern-sheet", sheet, egui::TextureOptions::NEAREST));
            self.thumbs.clear();
        }
    }

    fn rebuild_thumbs(&mut self, ctx: &egui::Context) {
        let frame = match &self.preview_frame {
            Some(f) => f.clone(),
            None => return,
        };
        let geom = match &self.geom {
            Some(g) => g.clone(),
            None => return,
        };
        let (start, end) = sheets_window(geom.total_pages, self.sheets_offset);
        let key = (self.frame_id, start, end);
        if self.thumbs_key == key && !self.thumbs.is_empty() {
            return;
        }
        self.thumbs.clear();
        self.thumbs_key = key;
        let sx = frame.poster_w as f64 / geom.cover.w;
        let sy = frame.poster_h as f64 / geom.cover.h;
        for page in start..end {
            let col = page % geom.pages_x;
            let row = page / geom.pages_x;
            let x0 = ((col as f64 * geom.step.w) * sx) as u32;
            let y0 = ((row as f64 * geom.step.h) * sy) as u32;
            let w = ((geom.visible.w * sx) as u32).max(1);
            let h = ((geom.visible.h * sy) as u32).max(1);
            let (tw, th, px) =
                downsample_region(&frame.poster, frame.poster_w, frame.poster_h, x0, y0, w, h, 160);
            let img = egui::ColorImage::from_rgb([tw, th], px.as_slice());
            self.thumbs.push(ctx.load_texture(
                format!("modern-thumb-{page}"),
                img,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    // -- top-level show ----------------------------------------------------

    pub fn show(&mut self, ctx: &egui::Context) {
        self.poll_workers(ctx);
        self.maybe_spawn_preview();

        self.show_header(ctx);
        self.show_options(ctx);
        self.show_readouts(ctx);
        self.show_central(ctx);
        self.show_status(ctx);
    }

    // -- panels ------------------------------------------------------------

    fn has_blocking(&self) -> bool {
        self.report.has_blocking()
    }

    fn warn_count(&self) -> usize {
        self.report
            .items()
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }

    fn show_header(&mut self, ctx: &egui::Context) {
        let mut switch = false;
        let mut do_render = false;
        egui::TopBottomPanel::top("mheader").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Modern workspace");
                ui.label(egui::RichText::new("modern").small().weak());
                ui.separator();
                // Presets
                let mut sel = self.preset_sel.clone();
                egui::ComboBox::from_id_salt("mpreset")
                    .selected_text(if sel.is_empty() { "(preset)" } else { sel.as_str() })
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut sel, String::new(), "(none)");
                        for n in &self.preset_names {
                            ui.selectable_value(&mut sel, n.clone(), n);
                        }
                    });
                if sel != self.preset_sel {
                    self.preset_sel = sel.clone();
                    if !sel.is_empty() {
                        match self.store.load(&sel) {
                            Ok(mut o) => {
                                o.preset_dir = default_preset_dir();
                                self.opts = o;
                                self.crop_text =
                                    self.opts.crop.map(|c| c.to_token()).unwrap_or_default();
                                self.status_msg = format!("Preset '{sel}' applied.");
                                self.refresh();
                            }
                            Err(e) => {
                                self.status_msg = format!("cannot load preset: {}", e.message)
                            }
                        }
                    }
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.save_name)
                        .desired_width(110.0)
                        .hint_text("preset name"),
                );
                if ui.button("Save").clicked() {
                    if !valid_preset_name(&self.save_name) {
                        self.status_msg =
                            String::from("invalid preset name: use [A-Za-z0-9_-]{1,48}");
                    } else {
                        let existed = self.store.exists(&self.save_name);
                        let name = self.save_name.clone();
                        match self.store.save(&name, &self.opts, true) {
                            Ok(()) => {
                                self.status_msg = if existed {
                                    format!("Preset '{name}' overwritten.")
                                } else {
                                    format!("Preset '{name}' saved.")
                                };
                                self.preset_names = self.store.list().map(|v| v.into_iter().map(|e| e.name).collect()).unwrap_or_default();
                            }
                            Err(e) => {
                                self.status_msg = format!("cannot save preset: {}", e.message)
                            }
                        }
                    }
                }
                if !self.confirm_delete {
                    if ui.button("Delete").clicked() {
                        self.confirm_delete = true;
                    }
                } else if ui.button("Confirm?").clicked() {
                    if self.preset_sel.is_empty() {
                        self.status_msg = String::from("Pick a preset first.");
                    } else {
                        let name = self.preset_sel.clone();
                        match self.store.delete(&name) {
                            Ok(()) => {
                                self.status_msg = format!("Preset '{name}' deleted.");
                                self.preset_sel.clear();
                                self.preset_names = self.store.list().map(|v| v.into_iter().map(|e| e.name).collect()).unwrap_or_default();
                            }
                            Err(e) => {
                                self.status_msg = format!("cannot delete preset: {}", e.message)
                            }
                        }
                    }
                    self.confirm_delete = false;
                }
                ui.separator();
                // Preview detail (GUI-only setting)
                let mut det = self.detail;
                egui::ComboBox::from_id_salt("mdetail")
                    .selected_text(preview_detail_label(det))
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for d in [
                            PreviewDetail::Low,
                            PreviewDetail::Medium,
                            PreviewDetail::High,
                        ] {
                            ui.selectable_value(&mut det, d, preview_detail_label(d));
                        }
                    });
                if det != self.detail {
                    self.detail = det;
                    self.refresh();
                }
                // Warning / error badges
                let w = self.warn_count();
                if w > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "● {w} warning{}",
                            if w == 1 { "" } else { "s" }
                        ))
                        .color(egui::Color32::from_rgb(190, 130, 0)),
                    );
                }
                if self.has_blocking() {
                    let n = self
                        .report
                        .items()
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .count();
                    ui.label(
                        egui::RichText::new(format!("▲ {n} error{}", if n == 1 { "" } else { "s" }))
                            .color(egui::Color32::from_rgb(200, 40, 40)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Switch mode").clicked() {
                        switch = true;
                    }
                    let can = !self.has_blocking()
                        && self.prepared.is_some()
                        && self.geom.is_some()
                        && self.export_worker.is_none();
                    if ui
                        .add_enabled(can, egui::Button::new("Render…"))
                        .on_disabled_hover_text("Fix blocking errors / pick a source image first")
                        .clicked()
                    {
                        do_render = true;
                    }
                });
            });
            ui.add_space(4.0);
        });
        if switch {
            self.request_switch = true;
        }
        if do_render {
            if let Some(path) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).save_file() {
                self.start_export(path);
            }
        }
    }

    fn show_options(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("moptions")
            .default_width(340.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut changed = false;
                    changed |= self.group_source(ui);
                    changed |= self.group_paper(ui);
                    changed |= self.group_screen(ui);
                    changed |= self.group_tone(ui);
                    changed |= self.group_colour(ui);
                    changed |= self.group_layout(ui);
                    changed |= self.group_output(ui);
                    if changed {
                        self.refresh();
                    }
                });
            });
    }

    fn show_readouts(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("mmeta")
            .default_width(240.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Job");
                ui.add_space(4.0);
                if let Some(g) = &self.geom {
                    let o = &self.opts;
                    ui.label(format!("Poster {}", format_mm(g.poster.w, g.poster.h)));
                    ui.label(format!(
                        "Sheets {} × {} ({} total)",
                        g.pages_x, g.pages_y, g.total_pages
                    ));
                    ui.label(format!("Cells {} total", g.total_cells));
                    ui.label(format!("Pitch {:.3} mm", g.pitch_mm));
                    // Extent at darkness 0 / 0.5 / 1 through the tone band.
                    for d in [0.0, 0.5, 1.0] {
                        let t = o.min_raster + (o.max_raster - o.min_raster) * d;
                        let e = extent_mm(t, g.pitch_mm, g.family_scale);
                        ui.label(format!("Extent d={d}: {e:.3} mm"));
                    }
                    if let Some(f) = &self.preview_frame {
                        ui.label(format!("Ink density {:.1} % (preview)", f.ink_percent));
                    } else {
                        ui.label("Ink density … (preview pending)");
                    }
                    ui.label(format!("Preview cap {} cells", preview_cell_cap(self.detail)));
                    let chip = match g.dpi_label() {
                        rasterbator_core::modern::DpiLabel::Good => "Good",
                        rasterbator_core::modern::DpiLabel::Fair => "Fair",
                        rasterbator_core::modern::DpiLabel::Low => "Low",
                    };
                    ui.label(format!("Source {:.0} dpi [{chip}]", g.source_effective_dpi));
                    ui.label(format!(
                        "Effective {} dpi / q{}",
                        g.effective_dpi, g.effective_quality
                    ));
                    ui.label(format!("Render {}", g.render_mode));
                    let pt = rasterbator_core::modern::PT_PER_MM;
                    let trim_w = (g.boxes.trim[2] - g.boxes.trim[0]) / pt;
                    let trim_h = (g.boxes.trim[3] - g.boxes.trim[1]) / pt;
                    let bleed_w = (g.boxes.bleed[2] - g.boxes.bleed[0]) / pt;
                    let bleed_h = (g.boxes.bleed[3] - g.boxes.bleed[1]) / pt;
                    ui.label(format!("Trim {}", format_mm(trim_w, trim_h)));
                    ui.label(format!("Bleed {}", format_mm(bleed_w, bleed_h)));
                } else {
                    ui.weak("No geometry yet — pick a source image.");
                }
            });
    }

    fn show_central(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                for t in [PreviewTab::Poster, PreviewTab::Sheet, PreviewTab::Sheets] {
                    ui.selectable_value(&mut self.preview_tab, t, t.label());
                }
                if self.preview_tab == PreviewTab::Sheet {
                    let total = self.geom.as_ref().map(|g| g.total_pages).unwrap_or(0);
                    if total > 0 {
                        let labels: Vec<String> = self
                            .geom
                            .as_ref()
                            .map(|g| {
                                g.windows
                                    .iter()
                                    .map(|w| page_label(w.col, w.row))
                                    .collect()
                            })
                            .unwrap_or_default();
                        ui.separator();
                        if ui.button("◀").clicked() {
                            self.sheet_page = self.sheet_page.saturating_sub(1);
                            self.refresh();
                        }
                        let mut p = self.sheet_page;
                        egui::ComboBox::from_id_salt("msheetpage")
                            .selected_text(format!("page {}/{}", p + 1, total))
                            .show_ui(ui, |ui| {
                                for i in 0..total {
                                    ui.selectable_value(
                                        &mut p,
                                        i,
                                        format!("p{} {}", i + 1, labels[i as usize]),
                                    );
                                }
                            });
                        if p != self.sheet_page {
                            self.sheet_page = p;
                            self.refresh();
                        }
                        if ui.button("▶").clicked() {
                            self.sheet_page = (self.sheet_page + 1).min(total.saturating_sub(1));
                            self.refresh();
                        }
                    }
                }
                if self.preview_tab == PreviewTab::Sheets {
                    if let Some(g) = &self.geom {
                        if g.total_pages > 64 {
                            ui.separator();
                            if ui.button("◀").clicked() {
                                self.sheets_offset = self.sheets_offset.saturating_sub(64);
                                self.thumbs.clear();
                            }
                            let (s, e) = sheets_window(g.total_pages, self.sheets_offset);
                            ui.label(format!("{}–{} of {}", s + 1, e, g.total_pages));
                            if ui.button("▶").clicked() {
                                self.sheets_offset =
                                    (self.sheets_offset + 64).min(g.total_pages - 1);
                                self.thumbs.clear();
                            }
                        }
                    }
                }
                ui.separator();
                if ui.button("Fit").clicked() {
                    self.zoom = 1.0;
                }
                if ui.button("−").clicked() {
                    self.zoom = clamp_zoom(self.zoom / 1.25);
                }
                if ui.button("+").clicked() {
                    self.zoom = clamp_zoom(self.zoom * 1.25);
                }
                let mut z = (self.zoom * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut z, 10.0..=800.0).suffix(" %").text("zoom"))
                    .changed()
                {
                    self.zoom = clamp_zoom(z / 100.0);
                }
            });
            ui.separator();
            match self.preview_tab {
                PreviewTab::Poster => self.ui_poster(ui),
                PreviewTab::Sheet => self.ui_sheet(ui),
                PreviewTab::Sheets => self.ui_sheets(ui, ctx),
            }
        });
    }

    fn show_status(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("mstatus").show(ctx, |ui| {
            // Validation list.
            egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                if self.report.items().is_empty() {
                    ui.label(egui::RichText::new("ⓘ no diagnostics").weak());
                }
                for d in self.report.items() {
                    let dd = diag_display(d.severity);
                    ui.label(
                        egui::RichText::new(format!("{} {}: {}", dd.icon, dd.label, d.message))
                            .color(egui::Color32::from_rgb(dd.color.0, dd.color.1, dd.color.2)),
                    );
                    if let Some(h) = &d.hint {
                        ui.label(egui::RichText::new(format!("hint: {h}")).small().weak());
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if let Some(p) = self.export_progress {
                    let frac = p.fraction().clamp(0.0, 1.0) as f32;
                    ui.add(egui::ProgressBar::new(frac).desired_width(280.0).show_percentage());
                    let eta = self
                        .export_started
                        .map(|s| {
                            let el = s.elapsed().as_secs_f64();
                            if frac > 0.02 {
                                let r = el / frac as f64 * (1.0 - frac as f64);
                                format!(" · eta {}", format_seconds(r))
                            } else {
                                String::new()
                            }
                        })
                        .unwrap_or_default();
                    ui.label(format!(
                        "page {}/{} · cells {}/{}{}",
                        p.pages_done, p.total_pages, p.cells_done, p.total_cells, eta
                    ));
                    if ui.button("Cancel").clicked() {
                        if let Some(w) = &self.export_worker {
                            w.cancel.store(true, Ordering::Relaxed);
                        }
                    }
                } else if self.preview_worker.is_some() {
                    ui.spinner();
                    ui.label("rendering preview…");
                } else if !self.status_msg.is_empty() {
                    ui.label(self.status_msg.clone());
                } else {
                    ui.label(format!("zoom {:.0} %", self.zoom * 100.0));
                }
            });
        });
    }

    fn ui_poster(&mut self, ui: &mut egui::Ui) {
        if let Some(tex) = &self.poster_tex {
            let size = tex.size_vec2() * self.zoom;
            egui::ScrollArea::both().show(ui, |ui| {
                ui.add(egui::Image::new((tex.id(), size)));
            });
            if let Some(g) = &self.geom {
                ui.label(format!(
                    "Poster {} · {} sheets · step {:.1} mm",
                    format_mm(g.cover.w, g.cover.h),
                    g.total_pages,
                    g.step.w
                ));
            }
        } else {
            ui.label("No preview yet — pick a source image.");
            if let Some(e) = &self.image_error {
                ui.colored_label(egui::Color32::RED, e.clone());
            }
        }
    }

    fn ui_sheet(&mut self, ui: &mut egui::Ui) {
        if let Some(tex) = &self.sheet_tex {
            let size = tex.size_vec2() * self.zoom;
            egui::ScrollArea::both().show(ui, |ui| {
                ui.add(egui::Image::new((tex.id(), size)));
            });
            if let Some(g) = &self.geom {
                let w = g.cell_window(self.sheet_page.min(g.total_pages.saturating_sub(1)));
                ui.label(format!(
                    "Sheet {} · window i {}..{} j {}..{} (exact export window)",
                    page_label(w.col, w.row),
                    w.i0,
                    w.i1,
                    w.j0,
                    w.j1
                ));
            }
        } else {
            ui.label("No preview yet — pick a source image.");
        }
    }

    fn ui_sheets(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.rebuild_thumbs(ctx);
        if self.thumbs.is_empty() {
            ui.label("No previews yet — pick a source image.");
            return;
        }
        let geom = match &self.geom {
            Some(g) => g.clone(),
            None => return,
        };
        let (start, _end) = sheets_window(geom.total_pages, self.sheets_offset);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (k, tex) in self.thumbs.iter().enumerate() {
                    let page = start + k as u32;
                    let w = &geom.windows[page as usize];
                    ui.vertical(|ui| {
                        let size = tex.size_vec2();
                        ui.add(egui::Image::new((tex.id(), size)));
                        ui.small(page_label(w.col, w.row));
                    });
                }
            });
        });
    }
}

// ---------------------------------------------------------------------------
// Option groups (each returns true when something changed)
// ---------------------------------------------------------------------------

impl ModernApp {
    fn group_source(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Source & preparation")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Source image:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.source_path)
                                .desired_width(200.0)
                                .hint_text("path to an image file"),
                        )
                        .changed()
                    {
                        self.load_image();
                    }
                    if ui.button("Browse…").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                "Images",
                                &["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp"],
                            )
                            .pick_file()
                        {
                            self.source_path = path.to_string_lossy().to_string();
                            self.load_image();
                        }
                    }
                });
                if let Some(e) = &self.image_error {
                    ui.colored_label(egui::Color32::RED, e.clone());
                }
                if let Some(d) = &self.decoded {
                    ui.label(format!("{} × {} px", d.width(), d.height()));
                }
                if combo_row(
                    ui,
                    "Rotate",
                    "mrotate",
                    &mut self.opts.rotate,
                    rasterbator_core::modern::Rotation::ALL,
                    "0",
                ) {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "EXIF orientation",
                    "mexif",
                    &mut self.opts.exif_orientation,
                    rasterbator_core::modern::ExifOrientation::ALL,
                    "apply",
                ) {
                    changed = true;
                }
                if slider_row(ui, "Brightness %", &mut self.opts.brightness, -100.0..=100.0, "0")
                {
                    changed = true;
                }
                if slider_row(ui, "Contrast %", &mut self.opts.contrast, -100.0..=100.0, "0") {
                    changed = true;
                }
                if slider_row(ui, "Saturation %", &mut self.opts.saturation, -100.0..=100.0, "0")
                {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Alpha threshold");
                    let mut v = self.opts.alpha_threshold as f64;
                    if ui.add(egui::Slider::new(&mut v, 0.0..=255.0)).changed() {
                        self.opts.alpha_threshold = v.round() as u8;
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: 128").small().weak());
                });
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.opts.resample, "Resample").changed() {
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: on").small().weak());
                });
                if inert_scope(&mut self.opts, self.geom.as_deref(), ui, "resample_dpi",
                    |ui, o| {
                        let mut v = o.resample_dpi as f64;
                        let mut r = false;
                        ui.horizontal(|ui| {
                            ui.label("Resample dpi");
                            if ui
                                .add(egui::DragValue::new(&mut v).range(72.0..=1200.0).suffix(" dpi"))
                                .changed()
                            {
                                o.resample_dpi = v.round() as u16;
                                r = true;
                            }
                            ui.label(egui::RichText::new("default: 300").small().weak());
                        });
                        r
                    })
                {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Crop px");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.crop_text)
                                .desired_width(150.0)
                                .hint_text("empty, or X1,Y1,X2,Y2"),
                        )
                        .changed()
                    {
                        let t = self.crop_text.trim().to_string();
                        if t.is_empty() {
                            self.opts.crop = None;
                            changed = true;
                        } else if let Some(c) =
                            rasterbator_core::modern::CropRectPx::parse_token(&t).ok()
                        {
                            self.opts.crop = Some(c);
                            changed = true;
                        } else {
                            self.status_msg =
                                String::from("invalid crop: use X1,Y1,X2,Y2 in pixels");
                        }
                    }
                });
            });
        changed
    }

    fn group_paper(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Paper & poster size")
            .default_open(true)
            .show(ui, |ui| {
                // Paper preset / custom.
                let cur = if let Some(p) = self.opts.paper.preset {
                    p.to_string()
                } else {
                    "custom".to_string()
                };
                let mut sel = cur.clone();
                ui.horizontal(|ui| {
                    ui.label("Paper");
                    egui::ComboBox::from_id_salt("mpaper")
                        .selected_text(cur.as_str())
                        .show_ui(ui, |ui| {
                            for p in ["a4", "a3", "letter", "legal", "custom"] {
                                ui.selectable_value(&mut sel, p.to_string(), p);
                            }
                        });
                    ui.label(egui::RichText::new("default: a4").small().weak());
                });
                if sel != cur {
                    if sel == "custom" {
                        if let Some(spec) =
                            rasterbator_core::modern::PaperSpec::parse_token(&self.paper_custom).ok()
                        {
                            self.opts.paper = spec;
                            changed = true;
                        }
                    } else if let Some(spec) =
                        rasterbator_core::modern::PaperSpec::parse_token(&sel).ok()
                    {
                        self.opts.paper = spec;
                        changed = true;
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Custom WxH mm");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.paper_custom)
                                .desired_width(110.0)
                                .hint_text("default: 210x297"),
                        )
                        .changed()
                        && self.opts.paper.preset.is_none()
                    {
                        if let Some(spec) =
                            rasterbator_core::modern::PaperSpec::parse_token(&self.paper_custom.clone()).ok()
                        {
                            self.opts.paper = spec;
                            changed = true;
                        }
                    }
                });
                if combo_row(
                    ui,
                    "Orientation",
                    "morient",
                    &mut self.opts.orientation,
                    rasterbator_core::modern::Orientation::ALL,
                    "portrait",
                ) {
                    changed = true;
                }
                // Size spec radio.
                let mut mode = match self.opts.size_spec {
                    rasterbator_core::modern::SizeSpec::WidthSheets(_) => 0,
                    rasterbator_core::modern::SizeSpec::HeightSheets(_) => 1,
                    rasterbator_core::modern::SizeSpec::Size { .. } => 2,
                };
                ui.horizontal(|ui| {
                    ui.radio_value(&mut mode, 0, "wide");
                    ui.radio_value(&mut mode, 1, "high");
                    ui.radio_value(&mut mode, 2, "exact");
                    ui.label(egui::RichText::new("default: wide").small().weak());
                });
                let cur_mode = match self.opts.size_spec {
                    rasterbator_core::modern::SizeSpec::WidthSheets(_) => 0,
                    rasterbator_core::modern::SizeSpec::HeightSheets(_) => 1,
                    rasterbator_core::modern::SizeSpec::Size { .. } => 2,
                };
                if mode != cur_mode {
                    self.opts.size_spec = match mode {
                        0 => rasterbator_core::modern::SizeSpec::WidthSheets(self.size_n),
                        1 => rasterbator_core::modern::SizeSpec::HeightSheets(self.size_n),
                        _ => rasterbator_core::modern::SizeSpec::Size {
                            w: self.exact_w,
                            h: self.exact_h,
                        },
                    };
                    changed = true;
                }
                if mode < 2 {
                    ui.horizontal(|ui| {
                        ui.label("Sheets");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.size_n)
                                    .range(0.1..=1000.0)
                                    .speed(0.1),
                            )
                            .changed()
                        {
                            self.opts.size_spec = if mode == 0 {
                                rasterbator_core::modern::SizeSpec::WidthSheets(self.size_n)
                            } else {
                                rasterbator_core::modern::SizeSpec::HeightSheets(self.size_n)
                            };
                            changed = true;
                        }
                        ui.label(egui::RichText::new("default: 5").small().weak());
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("W");
                        if ui
                            .add(egui::DragValue::new(&mut self.exact_w).range(1.0..=100000.0))
                            .changed()
                        {
                            changed = true;
                        }
                        ui.label("H");
                        if ui
                            .add(egui::DragValue::new(&mut self.exact_h).range(1.0..=100000.0))
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    if changed {
                        self.opts.size_spec = rasterbator_core::modern::SizeSpec::Size {
                            w: self.exact_w,
                            h: self.exact_h,
                        };
                    }
                    if combo_row(
                        ui,
                        "Size unit",
                        "msizeunit",
                        &mut self.opts.size_unit,
                        rasterbator_core::modern::SizeUnit::ALL,
                        "mm",
                    ) {
                        changed = true;
                    }
                }
                if combo_row(
                    ui,
                    "Fit",
                    "mfit",
                    &mut self.opts.fit,
                    rasterbator_core::modern::Fit::ALL,
                    "contain",
                ) {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "Align H",
                    "malignh",
                    &mut self.opts.align_h,
                    rasterbator_core::modern::Align::ALL,
                    "center",
                ) {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "Align V",
                    "malignv",
                    &mut self.opts.align_v,
                    rasterbator_core::modern::Align::ALL,
                    "center",
                ) {
                    changed = true;
                }
                if drag_row(ui, "Margin mm", &mut self.opts.margin_mm, 2.0..=490.0, "10") {
                    changed = true;
                }
                if drag_row(ui, "Overlap mm", &mut self.opts.overlap_mm, 0.0..=490.0, "0") {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "Overlap mode",
                    "moverlapmode",
                    &mut self.opts.overlap_mode,
                    rasterbator_core::modern::OverlapMode::ALL,
                    "margin",
                ) {
                    changed = true;
                }
                if drag_row(ui, "Bleed mm", &mut self.opts.bleed_mm, 0.0..=490.0, "0") {
                    changed = true;
                }
            });
        changed
    }

    fn group_screen(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Screen & pattern")
            .default_open(true)
            .show(ui, |ui| {
                if combo_row(
                    ui,
                    "Screen",
                    "mscreen",
                    &mut self.opts.screen,
                    rasterbator_core::modern::ScreenFamily::ALL,
                    "lattice",
                ) {
                    changed = true;
                }
                if inert_scope(&mut self.opts, self.geom.as_deref(), ui, "stagger", |ui, o| {
                    combo_row(ui, "Stagger", "mstagger", &mut o.stagger,
                        rasterbator_core::modern::Stagger::ALL, "off")
                }) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Halo cells");
                    let mut v = self.opts.halo_cells as f64;
                    if ui.add(egui::DragValue::new(&mut v).range(0.0..=2.0)).changed() {
                        self.opts.halo_cells = v.round() as u32;
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: 1").small().weak());
                });
                if inert_scope(&mut self.opts, self.geom.as_deref(), ui, "pattern", |ui, o| {
                    combo_row(ui, "Pattern", "mpattern", &mut o.pattern,
                        rasterbator_core::modern::Pattern::ALL, "dot")
                }) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "nominal_dot_mm",
                    |ui, o| drag_row(ui, "Dot size mm", &mut o.nominal_dot_mm, 1.0..=500.0, "10"),
                ) {
                    changed = true;
                }
                if inert_scope(&mut self.opts, self.geom.as_deref(), ui, "coverage", |ui, o| {
                    combo_row(ui, "Coverage", "mcoverage", &mut o.coverage,
                        rasterbator_core::modern::Coverage::ALL, "full-cell")
                }) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Grid phase X");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.opts.grid_phase_mm.0)
                                .range(-500.0..=500.0)
                                .speed(0.5),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label("Y");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.opts.grid_phase_mm.1)
                                .range(-500.0..=500.0)
                                .speed(0.5),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: 0,0").small().weak());
                });
            });
        changed
    }

    fn group_tone(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Tone & sampling")
            .default_open(false)
            .show(ui, |ui| {
                if slider_row(ui, "Min raster", &mut self.opts.min_raster, 0.0..=1.0, "0") {
                    changed = true;
                }
                if slider_row(ui, "Max raster", &mut self.opts.max_raster, 0.0..=1.0, "1") {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "Tone curve",
                    "mtone",
                    &mut self.opts.tone_curve,
                    rasterbator_core::modern::ToneCurve::ALL,
                    "none",
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "tone_exponent",
                    |ui, o| drag_row(ui, "Tone exponent", &mut o.tone_exponent, 0.1..=8.0, "2.2"),
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "tone_log_k",
                    |ui, o| drag_row(ui, "Tone log k", &mut o.tone_log_k, 1.0..=16.0, "8"),
                ) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.opts.invert_tone, "Invert tone").changed() {
                        changed = true;
                    }
                });
                if combo_row(
                    ui,
                    "Sampling",
                    "msampling",
                    &mut self.opts.sampling,
                    rasterbator_core::modern::Sampling::ALL,
                    "box",
                ) {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "Dither",
                    "mdither",
                    &mut self.opts.dither,
                    rasterbator_core::modern::Dither::ALL,
                    "none",
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "dither_strength",
                    |ui, o| slider_row(ui, "Dither strength", &mut o.dither_strength, 0.0..=1.0, "1"),
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "dither_levels",
                    |ui, o| {
                        let mut v = o.dither_levels as f64;
                        let mut r = false;
                        ui.horizontal(|ui| {
                            ui.label("Dither levels");
                            if ui.add(egui::DragValue::new(&mut v).range(2.0..=64.0)).changed() {
                                o.dither_levels = v.round() as u32;
                                r = true;
                            }
                            ui.label(egui::RichText::new("default: 8").small().weak());
                        });
                        r
                    },
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "dither_block",
                    |ui, o| {
                        let mut v = o.dither_block as f64;
                        let mut r = false;
                        ui.horizontal(|ui| {
                            ui.label("Dither block");
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=8.0)).changed() {
                                o.dither_block = v.round() as u32;
                                r = true;
                            }
                            ui.label(egui::RichText::new("default: 1").small().weak());
                        });
                        r
                    },
                ) {
                    changed = true;
                }
                if slider_row(ui, "Line aspect", &mut self.opts.line_aspect, 0.05..=1.0, "0.5") {
                    changed = true;
                }
                if drag_row(
                    ui,
                    "Mosaic contrast",
                    &mut self.opts.mosaic_contrast,
                    0.1..=8.0,
                    "1",
                ) {
                    changed = true;
                }
            });
        changed
    }

    fn group_colour(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Colour")
            .default_open(false)
            .show(ui, |ui| {
                if combo_row(
                    ui,
                    "Ink",
                    "mink",
                    &mut self.opts.ink,
                    rasterbator_core::modern::InkMode::ALL,
                    "fixed",
                ) {
                    changed = true;
                }
                let r = inert_reason("ink_color", &self.opts, self.geom.as_deref());
                if color_row(ui, "Ink colour", &mut self.opts.ink_color, r) {
                    changed = true;
                }
                let r = inert_reason("fg_color", &self.opts, self.geom.as_deref());
                if color_row(ui, "Foreground", &mut self.opts.fg_color, r) {
                    changed = true;
                }
                let r = inert_reason("bg_color", &self.opts, self.geom.as_deref());
                if color_row(ui, "Background", &mut self.opts.bg_color, r) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Paper colour");
                    let mut arr = [self.opts.paper_color.0, self.opts.paper_color.1, self.opts.paper_color.2];
                    if egui::color_picker::color_edit_button_srgb(ui, &mut arr).changed() {
                        self.opts.paper_color = Rgb(arr[0], arr[1], arr[2]);
                        changed = true;
                    }
                });
                // Theme picker.
                let mut sel = self.opts.theme.clone();
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    egui::ComboBox::from_id_salt("mtheme")
                        .selected_text(if sel.is_empty() { "(none)" } else { sel.as_str() })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel, String::new(), "(none)");
                            for t in THEMES {
                                ui.selectable_value(&mut sel, t.id.to_string(), t.name);
                            }
                        });
                });
                if sel != self.opts.theme {
                    if sel.is_empty() {
                        self.opts.theme.clear();
                        changed = true;
                    } else if find_theme(&sel).is_some() {
                        self.opts.theme = sel.clone();
                        apply_theme(&mut self.opts, &sel);
                        changed = true;
                    } else {
                        self.status_msg = format!("unknown theme '{sel}'");
                    }
                }
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.opts.shading, "Shading").changed() {
                        changed = true;
                    }
                });
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "shading_strength",
                    |ui, o| {
                        slider_row(ui, "Shading strength", &mut o.shading_strength, 0.0..=1.0, "0.35")
                    },
                ) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.opts.invert_colors, "Invert colours").changed() {
                        changed = true;
                    }
                });
                if slider_row(
                    ui,
                    "Dual softness",
                    &mut self.opts.dual_softness,
                    0.05..=1.0,
                    "1",
                ) {
                    changed = true;
                }
            });
        changed
    }

    fn group_layout(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Layout & guides")
            .default_open(false)
            .show(ui, |ui| {
                if combo_row(
                    ui,
                    "Guides",
                    "mguides",
                    &mut self.opts.guides,
                    rasterbator_core::modern::GuideStyle::ALL,
                    "corners",
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "guide_width_pt",
                    |ui, o| drag_row(ui, "Guide width pt", &mut o.guide_width_pt, 0.1..=6.0, "0.5"),
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "corner_mark_mm",
                    |ui, o| drag_row(ui, "Corner mark mm", &mut o.corner_mark_mm, 1.0..=30.0, "4"),
                ) {
                    changed = true;
                }
                let r = inert_reason("guide_color", &self.opts, self.geom.as_deref());
                if color_row(ui, "Guide colour", &mut self.opts.guide_color, r) {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "Labels",
                    "mlabels",
                    &mut self.opts.labels,
                    rasterbator_core::modern::LabelStyle::ALL,
                    "none",
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "label_size_pt",
                    |ui, o| drag_row(ui, "Label size pt", &mut o.label_size_pt, 4.0..=36.0, "9"),
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "label_offset_mm",
                    |ui, o| {
                        drag_row(ui, "Label offset mm", &mut o.label_offset_mm, 8.0..=490.0, "8")
                    },
                ) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Page warn");
                    let mut v = self.opts.page_warn as f64;
                    if ui.add(egui::DragValue::new(&mut v).range(0.0..=65535.0)).changed() {
                        self.opts.page_warn = v.round() as u32;
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: 64").small().weak());
                });
                ui.horizontal(|ui| {
                    ui.label("Page limit");
                    let mut v = self.opts.page_limit as f64;
                    if ui.add(egui::DragValue::new(&mut v).range(0.0..=65535.0)).changed() {
                        self.opts.page_limit = v.round() as u32;
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: 1024").small().weak());
                });
            });
        changed
    }

    fn group_output(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Output & quality")
            .default_open(false)
            .show(ui, |ui| {
                if combo_row(
                    ui,
                    "Render",
                    "mrender",
                    &mut self.opts.render,
                    rasterbator_core::modern::RenderMode::ALL,
                    "auto",
                ) {
                    changed = true;
                }
                if inert_scope(&mut self.opts, self.geom.as_deref(), ui, "dpi", |ui, o| {
                    combo_row(ui, "DPI", "mdpi", &mut o.dpi,
                        rasterbator_core::modern::Dpi::ALL, "300")
                }) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "jpeg_quality",
                    |ui, o| {
                        let mut v = o.jpeg_quality as f64;
                        let mut r = false;
                        ui.horizontal(|ui| {
                            ui.label("JPEG quality");
                            if ui.add(egui::Slider::new(&mut v, 0.0..=100.0)).changed() {
                                o.jpeg_quality = v.round() as u8;
                                r = true;
                            }
                            ui.label(egui::RichText::new("default: 85").small().weak());
                        });
                        r
                    },
                ) {
                    changed = true;
                }
                if inert_scope(
                    &mut self.opts,
                    self.geom.as_deref(),
                    ui,
                    "compress",
                    |ui, o| {
                        let mut r = false;
                        ui.horizontal(|ui| {
                            let mut c = o.compress;
                            if ui.checkbox(&mut c, "Compress (150 dpi / q60)").changed() {
                                o.compress = c;
                                r = true;
                            }
                            ui.label(egui::RichText::new("default: off").small().weak());
                        });
                        r
                    },
                ) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Title");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.opts.title)
                                .desired_width(170.0)
                                .hint_text("default: empty"),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Creator");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.opts.creator)
                                .desired_width(170.0)
                                .hint_text("default: rasterbator-rs modern"),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
                if combo_row(
                    ui,
                    "Creation date",
                    "mcreationdate",
                    &mut self.opts.creation_date,
                    rasterbator_core::modern::CreationDate::ALL,
                    "none",
                ) {
                    changed = true;
                }
                if combo_row(
                    ui,
                    "Print scaling",
                    "mprintscaling",
                    &mut self.opts.print_scaling,
                    PrintScaling::ALL,
                    "none",
                ) {
                    changed = true;
                }
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.opts.atomic, "Atomic save").changed() {
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: on").small().weak());
                });
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut self.opts.strict, "Strict (warnings block)")
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: off").small().weak());
                });
                ui.horizontal(|ui| {
                    ui.label("Auto-raster cells");
                    let mut v = self.opts.auto_raster_cells as f64;
                    if ui
                        .add(egui::DragValue::new(&mut v).range(0.0..=100000000.0))
                        .changed()
                    {
                        self.opts.auto_raster_cells = v.round() as u32;
                        changed = true;
                    }
                    ui.label(egui::RichText::new("default: 200000").small().weak());
                });
            });
        changed
    }
}
