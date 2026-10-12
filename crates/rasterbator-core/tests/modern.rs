//! End-to-end tests for modern mode.
//!
//! These exercise the whole pipeline — prepare, solve, render, write — from
//! the public API, and the invariants that span more than one module: the
//! canonical geometry drives both the export and the preview, the dither row
//! buffer is poster-wide, and identical inputs give identical bytes.

use std::sync::atomic::AtomicBool;

use rasterbator_core::modern::{
    options::{
        Align, Coverage, Dither, Dpi, Fit, GuideStyle, InkMode, LabelStyle, ModernOptions,
        Orientation, OverlapMode, Pattern, RenderMode, Rotation, ScreenFamily, Sampling, SizeSpec,
        Stagger, ToneCurve,
    },
    prepare::{prepare_image, PrepareOptions},
    validate, DpiLabel, JobGeometry, ModernPdf, PageTarget,
    PreparedImage, PresetStore, RenderSession, SourceInfo, ValidationReport,
};
use rasterbator_core::modern::preview::{BitmapTarget, PreviewDetail, preview_stride};

const TEST_IMAGE_NAME: &str = "gradient.png";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("test-images")
        .join(TEST_IMAGE_NAME)
}

/// A deterministic synthetic source: a horizontal ramp with a red/green tint,
/// so tone, colour and sampling all have something to chew on.
fn ramp_source(w: u32, h: u32) -> PreparedImage {
    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let v = (x * 255 / w.max(1)) as u8;
            img.put_pixel(x, y, image::Rgba([v, (y * 255 / h.max(1)) as u8, v, 255]));
        }
    }
    PreparedImage::from_rgba(&img)
}

fn small_job() -> (ModernOptions, SourceInfo, PreparedImage) {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    o.nominal_dot_mm = 12.0;
    o.page_warn = 0;
    o.page_limit = 0;
    o.auto_raster_cells = 0;
    o.guides = GuideStyle::Corners;
    let img = ramp_source(120, 90);
    let src = SourceInfo::synthetic(img.width, img.height);
    (o, src, img)
}

fn render_to_bytes(o: &ModernOptions, src: &SourceInfo, img: &PreparedImage) -> Vec<u8> {
    let geom = JobGeometry::solve(o, src).expect("the job is renderable");
    let cancel = AtomicBool::new(false);
    let mut session = RenderSession::new(img, o, &geom, &cancel);
    let mut pdf = ModernPdf::new(o, &geom);
    let mut target = rasterbator_core::modern::pdf::PdfTarget::new(&mut pdf);
    while !session.is_finished() {
        session.render_next(&mut target).expect("rendering a page");
    }
    target.finish();
    session.set_output_bytes(pdf.bytes().len() as u64);
    pdf.bytes()
}

#[test]
fn defaults_validate_and_solve() {
    let (o, src, img) = small_job();
    let geom = JobGeometry::solve(&o, &src).unwrap();
    let report = validate(&o, Some(&src), Some(&geom));
    assert!(!report.has_blocking(), "the defaults must be renderable: {report}");
    assert_eq!(geom.windows.len(), geom.total_pages as usize);
    assert!(geom.total_cells > 0);
    assert!(img.pixel_count() > 0);
}

#[test]
fn a_vector_job_produces_a_well_formed_pdf() {
    let (o, src, img) = small_job();
    let bytes = render_to_bytes(&o, &src, &img);
    assert!(bytes.starts_with(b"%PDF-1.4\n"));
    assert!(bytes.ends_with(b"%%EOF\n"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/TrimBox"), "modern pages carry a trim box");
    assert!(text.contains("/BleedBox"), "modern pages carry a bleed box");
    assert!(text.contains("/ProcSet [/PDF]"));
    assert!(!text.contains("/CreationDate"), "no date by default keeps bytes stable");
    assert!(!text.contains("/Font"), "labels use the stroke font, not a font resource");
    // One Page object per page, numbered from the plan.
    let pages = text.matches("/Type /Page ").count();
    assert_eq!(pages, JobGeometry::solve(&o, &src).unwrap().total_pages as usize);
}

#[test]
fn identical_inputs_produce_identical_bytes() {
    let (o, src, img) = small_job();
    assert_eq!(render_to_bytes(&o, &src, &img), render_to_bytes(&o, &src, &img));
}

#[test]
fn every_legal_screen_pattern_combination_renders() {
    let (mut base, src, img) = small_job();
    // A coarse screen on a small sheet at the cheapest raster setting keeps
    // the sweep quick; the geometry is covered in detail elsewhere.
    base.nominal_dot_mm = 40.0;
    base.paper = rasterbator_core::modern::options::PaperSpec::custom(100.0, 100.0);
    base.dpi = Dpi::P150;
    base.compress = true;
    let mut rendered = 0usize;
    for family in [ScreenFamily::Lattice, ScreenFamily::Hex, ScreenFamily::Plain] {
        for pattern in Pattern::ALL {
            for coverage in [Coverage::FullCell, Coverage::Tangent] {
                for sampling in Sampling::ALL {
                    let mut o = base.clone();
                    o.screen = family;
                    o.pattern = *pattern;
                    o.coverage = coverage;
                    o.sampling = *sampling;
                    o.dither = Dither::None;
                    // `validate` owns the interaction matrix; `solve` only
                    // guards the geometry. A legal set must solve and render.
                    let legal = !validate(&o, Some(&src), None).has_blocking();
                    if legal {
                        assert!(
                            JobGeometry::solve(&o, &src).is_ok(),
                            "{family:?}/{pattern:?}/{coverage:?}/{sampling:?} should solve"
                        );
                        let bytes = render_to_bytes(&o, &src, &img);
                        assert!(bytes.len() > 400, "{family:?}/{pattern:?} produced no content");
                        rendered += 1;
                    }
                }
            }
        }
    }
    // Hex rejects the mosaic square and the three bar patterns; every other
    // pattern is legal on all three families.
    assert_eq!(rendered, (8 + 4 + 8) * 2 * 3);
}

#[test]
fn the_ink_density_estimate_is_monotone_along_a_tone_ramp() {
    // White to black: darker pixels must never reduce the ink coverage.
    let mut prev = f64::NEG_INFINITY;
    for step in (0..=10).rev() {
        let level = f64::from(step) / 10.0;
        let mut o = ModernOptions::default();
        o.size_spec = SizeSpec::WidthSheets(1.0);
        o.nominal_dot_mm = 10.0;
        o.page_warn = 0;
        o.auto_raster_cells = 0;
        let img = PreparedImage::solid(60, 60, rasterbator_core::modern::Rgb::from_unit(level, level, level));
        let src = SourceInfo::synthetic(60, 60);
        let geom = JobGeometry::solve(&o, &src).unwrap();
        let cancel = AtomicBool::new(false);
        let mut session = RenderSession::new(&img, &o, &geom, &cancel);
        let mut probe = rasterbator_core::modern::probe::CountingTarget::default();
        while !session.is_finished() {
            session.render_next(&mut probe).unwrap();
        }
        let d = session.stats().ink_density_percent();
        assert!(d >= prev - 1e-9, "ink density fell from {prev} to {d} at level {level}");
        prev = d;
    }
    assert!(prev > 100.0, "an all-black full-cell lattice exceeds 100 %");
}

#[test]
fn the_preview_target_agrees_with_the_export_geometry() {
    let (o, src, img) = small_job();
    let geom = JobGeometry::solve(&o, &src).expect("renderable");
    // Both consumers take the same geometry; the preview thins the lattice
    // with a stride but never changes a page count or a box.
    assert_eq!(preview_stride(geom.total_cells, PreviewDetail::High.cell_cap()), 1);
    let cancel = AtomicBool::new(false);
    let mut session = RenderSession::new(&img, &o, &geom, &cancel);
    let mut pdf = ModernPdf::new(&o, &geom);
    let mut target = rasterbator_core::modern::pdf::PdfTarget::new(&mut pdf);
    let before = target_page_count(&mut session, &mut target);
    assert_eq!(before, geom.total_pages);
    let mut again = JobGeometry::solve(&o, &src).unwrap();
    std::mem::swap(&mut again, &mut JobGeometry::solve(&o, &src).unwrap());
}

fn target_page_count(session: &mut RenderSession<'_>, target: &mut dyn PageTarget) -> u32 {
    let mut n = 0;
    while !session.is_finished() {
        session.render_next(target).unwrap();
        n += 1;
    }
    n
}

#[test]
fn the_preview_bitmap_target_draws_the_page() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    o.nominal_dot_mm = 25.0;
    o.page_warn = 0;
    o.auto_raster_cells = 0;
    let img = ramp_source(80, 60);
    let src = SourceInfo::synthetic(80, 60);
    let geom = JobGeometry::solve(&o, &src).unwrap();
    let cancel = AtomicBool::new(false);
    let mut session = RenderSession::new(&img, &o, &geom, &cancel);
    let mut target = BitmapTarget::new(&geom.boxes, 72);
    session.render_next(&mut target).unwrap();
    let frame = target.finish();
    assert!(frame.shapes > 0);
    assert!(frame.width > 0 && frame.height > 0);
    assert_eq!(frame.rgba.len(), (frame.width * frame.height * 4) as usize);
    let dark = frame.rgba.chunks_exact(4).filter(|p| p[0] < 40).count();
    assert!(dark > 0, "the preview must contain inked pixels");
}

#[test]
fn cancelling_mid_job_leaves_no_partial_page() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(3.0);
    o.nominal_dot_mm = 8.0;
    o.page_warn = 0;
    o.page_limit = 0;
    o.auto_raster_cells = 0;
    let img = ramp_source(200, 150);
    let src = SourceInfo::synthetic(200, 150);
    let geom = JobGeometry::solve(&o, &src).unwrap();
    assert!(geom.total_pages > 1);
    let cancel = AtomicBool::new(false);
    let mut session = RenderSession::new(&img, &o, &geom, &cancel);
    let mut pdf = ModernPdf::new(&o, &geom);
    let mut target = rasterbator_core::modern::pdf::PdfTarget::new(&mut pdf);
    session.render_next(&mut target).unwrap();
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(session.render_next(&mut target).is_err());
    target.finish();
    assert_eq!(pdf.page_count(), 1, "the interrupted page was dropped");
}

#[test]
fn poster_wide_dithering_is_page_order_independent() {
    // The error buffer is poster-wide, so chunking the traversal (which is
    // exactly what a page boundary does) cannot change any cell value.
    let cols = 37i64;
    let rows = 11i64;
    let t = |i: i64, j: i64| ((i * 37 + j * 11).rem_euclid(23)) as f64 / 22.0;
    let mut sweep =
        rasterbator_core::modern::dither::DitherState::new(cols as u32, Dither::FloydSteinberg, 8, 1, 1.0);
    let mut straight = Vec::new();
    for j in 0..rows {
        for i in 0..cols {
            straight.push(sweep.quantise(t(i, j), i, j, true));
        }
    }
    // Same traversal with halo reads interleaved on both sides, which must be
    // side-effect free.
    let mut chunked = Vec::new();
    let mut state =
        rasterbator_core::modern::dither::DitherState::new(cols as u32, Dither::FloydSteinberg, 8, 1, 1.0);
    for j in 0..rows {
        let left = state.quantise(t(0, j), -1, j, false);
        assert!((left - t(0, j)).abs() < 1e-12, "a halo read must not change the cell");
        for i in 0..cols {
            chunked.push(state.quantise(t(i, j), i, j, true));
        }
        let right = state.quantise(t(0, j), cols, j, false);
        assert!((right - t(0, j)).abs() < 1e-12);
    }
    assert_eq!(straight, chunked);
}

#[test]
fn overlap_mode_changes_the_step_but_not_the_dot_grid() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(2.0);
    o.overlap_mm = 8.0;
    let src = SourceInfo::synthetic(400, 300);
    let margin = JobGeometry::solve(&o, &src).unwrap();
    o.overlap_mode = OverlapMode::Shared;
    let shared = JobGeometry::solve(&o, &src).unwrap();
    assert!((shared.step.w - (margin.visible.w - 8.0)).abs() < 1e-9);
    assert!((margin.step.w - (margin.visible.w + 8.0)).abs() < 1e-9);
    assert_eq!(shared.pitch_mm, margin.pitch_mm, "the grid pitch is unchanged");
    assert_eq!(shared.source_rect_mm.len(), 4);
}

#[test]
fn shared_overlap_with_a_dither_is_rejected() {
    let mut o = ModernOptions::default();
    o.overlap_mode = OverlapMode::Shared;
    o.overlap_mm = 5.0;
    o.dither = Dither::Atkinson;
    let report = validate(&o, None, None);
    assert!(report.has_blocking());
    assert!(report.into_error().is_err());
}

#[test]
fn plain_is_greyscale_and_raster_only() {
    let mut o = ModernOptions::default();
    o.screen = ScreenFamily::Plain;
    o.size_spec = SizeSpec::WidthSheets(1.0);
    o.min_raster = 0.2;
    o.page_warn = 0;
    o.paper = rasterbator_core::modern::options::PaperSpec::custom(100.0, 100.0);
    o.compress = true;
    let img = PreparedImage::solid(40, 40, rasterbator_core::modern::Rgb::WHITE);
    let src = SourceInfo::synthetic(40, 40);
    let geom = JobGeometry::solve(&o, &src).unwrap();
    assert_eq!(geom.render_mode, RenderMode::Raster, "A-5: plain always rasterises");
    let bytes = render_to_bytes(&o, &src, &img);
    assert!(String::from_utf8_lossy(&bytes).contains("/DCTDecode"));
    o.render = RenderMode::Vector;
    assert!(validate(&o, Some(&src), Some(&geom)).has_blocking());
}

#[test]
fn raster_path_embeds_one_image_per_page() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    o.render = RenderMode::Raster;
    o.dpi = Dpi::P150;
    o.nominal_dot_mm = 20.0;
    o.page_warn = 0;
    let img = ramp_source(60, 45);
    let src = SourceInfo::synthetic(60, 45);
    let geom = JobGeometry::solve(&o, &src).unwrap();
    let bytes = render_to_bytes(&o, &src, &img);
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(text.matches("/Subtype /Image").count(), geom.total_pages as usize);
    assert_eq!(text.matches("/XObject << /Im0").count(), geom.total_pages as usize);
}

#[test]
fn labels_are_drawn_with_the_stroke_font() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    o.labels = LabelStyle::Coordinates;
    o.nominal_dot_mm = 40.0;
    o.page_warn = 0;
    o.auto_raster_cells = 0;
    let img = ramp_source(60, 45);
    let src = SourceInfo::synthetic(60, 45);
    JobGeometry::solve(&o, &src).unwrap();
    let bytes = render_to_bytes(&o, &src, &img);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/PageLabels"), "coordinate labels get PDF page labels");
    assert!(!text.contains("/BaseFont"), "no font resource is needed");
    // Stroked polylines for the labels, so `S` appears in the raw stream.
    assert_eq!(text.matches("/PageLabels").count(), 1);
}

#[test]
fn page_boxes_shrink_with_a_bigger_margin() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    let src = SourceInfo::synthetic(100, 100);
    let a = JobGeometry::solve(&o, &src).unwrap();
    o.margin_mm = 20.0;
    let b = JobGeometry::solve(&o, &src).unwrap();
    assert!(b.boxes.trim[0] > a.boxes.trim[0]);
    assert!((a.visible.w - b.visible.w - 20.0).abs() < 1e-9);
    // The margin never changes the poster size.
    assert!((a.poster.w - b.poster.w).abs() < 1e-9);
}

#[test]
fn fit_and_alignment_place_the_image() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::Size { w: 400.0, h: 300.0 };
    let src = SourceInfo::synthetic(800, 800);
    let contain = JobGeometry::solve(&o, &src).unwrap();
    let rw = contain.source_rect_mm[2] - contain.source_rect_mm[0];
    let rh = contain.source_rect_mm[3] - contain.source_rect_mm[1];
    assert!((rw / rh - 1.0).abs() < 1e-9, "contain keeps the aspect");
    assert!((contain.source_rect_mm[0] - (400.0 - rw) / 2.0).abs() < 1e-9, "centred");

    o.align_h = Align::End;
    o.align_v = Align::End;
    let end = JobGeometry::solve(&o, &src).unwrap();
    assert!((end.source_rect_mm[2] - 400.0).abs() < 1e-9);

    o.fit = Fit::Stretch;
    let stretch = JobGeometry::solve(&o, &src).unwrap();
    assert!((stretch.source_rect_mm[2] - stretch.source_rect_mm[0] - 400.0).abs() < 1e-9);
    assert!((stretch.source_rect_mm[3] - stretch.source_rect_mm[1] - 300.0).abs() < 1e-9);
}

#[test]
fn rotation_swaps_the_layout_aspect() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    o.rotate = Rotation::D90;
    let src = SourceInfo::synthetic(800, 400);
    let g = JobGeometry::solve(&o, &src).unwrap();
    let rw = g.source_rect_mm[2] - g.source_rect_mm[0];
    let rh = g.source_rect_mm[3] - g.source_rect_mm[1];
    assert!((rw / rh - 0.5).abs() < 1e-9, "a quarter turn flips the aspect");
}

#[test]
fn landscape_swallows_the_sheet() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    let src = SourceInfo::synthetic(400, 400);
    o.orientation = Orientation::Portrait;
    let p = JobGeometry::solve(&o, &src).unwrap();
    o.orientation = Orientation::Landscape;
    let l = JobGeometry::solve(&o, &src).unwrap();
    assert_eq!(p.sheet.w, l.sheet.h);
    assert_eq!(p.sheet.h, l.sheet.w);
}

#[test]
fn tone_and_ink_options_change_the_output() {
    let (base, src, img) = small_job();
    let plain = render_to_bytes(&base, &src, &img);
    let mut o = base.clone();
    o.tone_curve = ToneCurve::Exponential;
    o.tone_exponent = 3.0;
    let curved = render_to_bytes(&o, &src, &img);
    assert_ne!(plain, curved);

    let mut o = base.clone();
    o.ink = InkMode::Grayscale;
    let grey = render_to_bytes(&o, &src, &img);
    assert_ne!(plain, grey);

    let mut o = base.clone();
    o.coverage = Coverage::Tangent;
    let tangent = render_to_bytes(&o, &src, &img);
    assert_ne!(plain, tangent);

    let mut o = base.clone();
    o.stagger = Stagger::HalfPitch;
    let staggered = render_to_bytes(&o, &src, &img);
    assert_ne!(plain, staggered);
}

#[test]
fn the_preparation_pipeline_changes_the_render() {
    let (base, src, _img) = small_job();
    let raw = ramp_source(120, 90);
    let plain = render_to_bytes(&base, &src, &raw);

    let mut p = PrepareOptions::from_options(&base);
    p.resample = false;
    p.rotate = Rotation::D180;
    p.crop = Some(rasterbator_core::modern::options::CropRectPx { x1: 10, y1: 5, x2: 100, y2: 80 });
    p.brightness = 30.0;
    p.contrast = 25.0;
    p.saturation = 50.0;
    let prepared = prepare_image(&raw_as_image(&raw), &p).unwrap();
    assert_eq!((prepared.width, prepared.height), (90, 75));
    let cropped = render_to_bytes(&base, &src, &prepared);
    assert_ne!(plain, cropped);
}

fn raw_as_image(p: &PreparedImage) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(p.width, p.height);
    let w = p.width as usize;
    for (i, s) in p.pixels.iter().enumerate() {
        img.put_pixel(
            (i % w) as u32,
            (i / w) as u32,
            image::Rgba([
                (s.r * 255.0).round() as u8,
                (s.g * 255.0).round() as u8,
                (s.b * 255.0).round() as u8,
                (s.a * 255.0).round() as u8,
            ]),
        );
    }
    img
}

#[test]
fn the_test_image_fixture_decodes_and_prepares() {
    let path = fixture_path();
    if !path.is_file() {
        eprintln!("fixture {} is absent; skipping", path.display());
        return;
    }
    let info = rasterbator_core::modern::source::inspect(&path).unwrap();
    assert!(info.width > 0 && info.height > 0);
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(1.0);
    o.nominal_dot_mm = 15.0;
    o.page_warn = 0;
    o.page_limit = 0;
    let (_, prepared) =
        rasterbator_core::modern::prepare::prepare(&path, &PrepareOptions::from_options(&o))
            .unwrap();
    let geom = JobGeometry::solve(&o, &info).unwrap();
    let bytes = render_to_bytes(&o, &info, &prepared);
    assert!(bytes.len() > 2000, "{} bytes", bytes.len());
    assert_eq!(geom.total_pages, geom.pages_x * geom.pages_y);
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
}

#[test]
fn the_dpi_label_follows_the_effective_resolution() {
    let mut o = ModernOptions::default();
    o.size_spec = SizeSpec::WidthSheets(4.0);
    let hi = JobGeometry::solve(&o, &SourceInfo::synthetic(8000, 6000)).unwrap();
    let lo = JobGeometry::solve(&o, &SourceInfo::synthetic(400, 300)).unwrap();
    assert_eq!(hi.dpi_label(), DpiLabel::Good);
    assert_eq!(lo.dpi_label(), DpiLabel::Low);
    assert!(hi.source_effective_dpi > lo.source_effective_dpi);
}

#[test]
fn presets_round_trip_through_a_store() {
    let dir = std::env::temp_dir().join(format!("rasterbator-it-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = PresetStore::new(dir.clone());
    let (mut o, _, _) = small_job();
    o.screen = ScreenFamily::Hex;
    o.pattern = Pattern::Triangle;
    o.dither = Dither::Bayer4x4;
    o.labels = LabelStyle::Coordinates;
    store.save("integration", &o, false).unwrap();
    assert_eq!(store.load("integration").unwrap(), o);
    let names: Vec<String> = store.list().unwrap().into_iter().map(|e| e.name).collect();
    assert!(names.contains(&"integration".to_string()));
    store.delete("integration").unwrap();
    assert!(!store.exists("integration"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn validation_report_is_stable_and_idempotent() {
    let mut o = ModernOptions::default();
    o.resample = false;
    o.halo_cells = 0;
    o.jpeg_quality = 10;
    o.nominal_dot_mm = 2.0;
    o.min_raster = 0.9;
    o.max_raster = 0.1;
    let a: ValidationReport = validate(&o, None, None);
    let b = validate(&o, None, None);
    assert_eq!(a, b);
    assert!(a.has_blocking());
    assert!(a.error_count() >= 1, "the inverted tone band is blocking");
}

#[test]
fn the_output_metadata_is_neutral() {
    let (o, src, img) = small_job();
    let bytes = render_to_bytes(&o, &src, &img);
    let text = String::from_utf8_lossy(&bytes);
    let start = text.find("/Producer").expect("an Info dictionary is always written");
    let tail = &text[start..];
    let info = &tail[..tail.find(">>").unwrap()];
    assert!(info.contains("/Creator (rasterbator-rs modern)"), "{info}");
    assert!(info.contains("/Producer (rasterbator-rs)"), "{info}");
    for forbidden in ["http", "www.", "@", "github", ".com", ".org", "://"] {
        assert!(!info.contains(forbidden), "metadata leaked '{forbidden}': {info}");
    }
}

#[test]
fn atomic_and_plain_writes_produce_the_same_bytes() {
    let (o, src, img) = small_job();
    let geom = JobGeometry::solve(&o, &src).unwrap();
    let cancel = AtomicBool::new(false);
    let mut session = RenderSession::new(&img, &o, &geom, &cancel);
    let mut pdf = ModernPdf::new(&o, &geom);
    let mut target = rasterbator_core::modern::pdf::PdfTarget::new(&mut pdf);
    while !session.is_finished() {
        session.render_next(&mut target).unwrap();
    }
    target.finish();

    let dir = std::env::temp_dir().join(format!("rasterbator-it-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.pdf");
    let b = dir.join("b.pdf");
    pdf.save(&a, true).unwrap();
    pdf.save(&b, false).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert!(!dir.join("a.pdf.tmp0").exists());
    let _ = std::fs::remove_dir_all(&dir);
}