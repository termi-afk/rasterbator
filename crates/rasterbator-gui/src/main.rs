//! Desktop wizard for the Rasterbator port, mirroring the original
//! five-step wizard: source image -> paper size -> output size -> options ->
//! save as, then rasterbating with a progress display and a completion page.

// Build as a Windows GUI application: without this, Windows gives the process
// a console window (Rust defaults to the console subsystem). Ignored on
// non-Windows targets. For console output while debugging, temporarily
// remove this attribute.
#![windows_subsystem = "windows"]

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use rasterbator_core::{
    load_rgba, rasterbate, resolve_output_size, ColorMode, EngineOptions, EngineSummary,
    PdfDocument, RgbaImage, SizeSpec,
};

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 680.0])
            .with_min_inner_size([780.0, 560.0])
            .with_title("The Rasterbator"),
        ..Default::default()
    };
    eframe::run_native(
        "Rasterbator",
        native_options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Source,
    Paper,
    Size,
    Options,
    SaveAs,
    Running,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorChoice {
    Black,
    Custom,
    Average,
}

enum Msg {
    Progress(u32, u32),
    Done(Result<EngineSummary, String>),
}

struct App {
    step: Step,
    source_path: String,
    output_path: String,
    image: Option<Arc<RgbaImage>>,
    texture: Option<egui::TextureHandle>,
    image_error: Option<String>,

    paper_standard: bool,
    preset_idx: usize,
    landscape: bool,
    custom_w: f64,
    custom_h: f64,

    measure_high: bool,
    output_value: f64,

    dot_size: f64,
    color: ColorChoice,
    custom_color: [u8; 3],
    cutout: bool,
    confirm_small_dots: bool,

    running: bool,
    rx: Option<Receiver<Msg>>,
    pages_done: u32,
    pages_total: u32,
    started: Option<Instant>,
    result: Option<Result<EngineSummary, String>>,
    open_after: bool,
}

impl Default for App {
    fn default() -> Self {
        App {
            step: Step::Welcome,
            source_path: String::new(),
            output_path: String::new(),
            image: None,
            texture: None,
            image_error: None,
            paper_standard: true,
            preset_idx: 0,
            landscape: false,
            custom_w: 200.0,
            custom_h: 200.0,
            measure_high: false,
            output_value: 5.0,
            dot_size: 10.0,
            color: ColorChoice::Black,
            custom_color: [0, 0, 0],
            cutout: true,
            confirm_small_dots: false,
            running: false,
            rx: None,
            pages_done: 0,
            pages_total: 0,
            started: None,
            result: None,
            open_after: true,
        }
    }
}

impl App {
    fn paper_mm(&self) -> (f64, f64) {
        let (mut w, mut h) = if self.paper_standard {
            let p = rasterbator_core::PAPER_PRESETS[self.preset_idx];
            (p.width_mm, p.height_mm)
        } else {
            (self.custom_w, self.custom_h)
        };
        if self.landscape {
            std::mem::swap(&mut w, &mut h);
        }
        (w, h)
    }

    fn size_spec(&self) -> SizeSpec {
        if self.measure_high {
            SizeSpec::HeightSheets(self.output_value)
        } else {
            SizeSpec::WidthSheets(self.output_value)
        }
    }

    fn resolved(&self) -> Option<rasterbator_core::ResolvedSize> {
        let image = self.image.as_ref()?;
        let (pw, ph) = self.paper_mm();
        Some(resolve_output_size(
            self.size_spec(),
            pw,
            ph,
            image.width() as f64,
            image.height() as f64,
        ))
    }

    fn engine_options(&self) -> Option<EngineOptions> {
        let resolved = self.resolved()?;
        let (pw, ph) = self.paper_mm();
        let color = match self.color {
            ColorChoice::Black => ColorMode::Fixed { r: 0, g: 0, b: 0 },
            ColorChoice::Custom => ColorMode::Fixed {
                r: self.custom_color[0],
                g: self.custom_color[1],
                b: self.custom_color[2],
            },
            ColorChoice::Average => ColorMode::Average,
        };
        Some(EngineOptions {
            paper_width_mm: pw,
            paper_height_mm: ph,
            image_width_mm: resolved.width_mm,
            image_height_mm: resolved.height_mm,
            dot_size_mm: self.dot_size,
            color,
            cutout: self.cutout,
            crop: None,
        })
    }

    fn load_image(&mut self, ctx: &egui::Context) {
        self.image = None;
        self.texture = None;
        self.image_error = None;
        if self.source_path.is_empty() {
            return;
        }
        match load_rgba(&self.source_path) {
            Ok(image) => {
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                );
                self.texture = Some(ctx.load_texture(
                    "source",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
                self.image = Some(Arc::new(image));
                if self.output_path.is_empty() {
                    self.output_path = suggested_output(&self.source_path);
                }
            }
            Err(e) => self.image_error = Some(format!("Cannot read image: {e}")),
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let Some(image) = self.image.clone() else { return };
        let Some(opts) = self.engine_options() else { return };
        let output = PathBuf::from(self.output_path.clone());
        let (tx, rx): (Sender<Msg>, Receiver<Msg>) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        self.running = true;
        self.result = None;
        self.pages_done = 0;
        self.pages_total = 0;
        self.started = Some(Instant::now());
        self.step = Step::Running;

        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut pdf = PdfDocument::from_paper_mm(opts.paper_width_mm, opts.paper_height_mm);
            let progress_tx = tx.clone();
            let mut on_page = |done: u32, total: u32| {
                let _ = progress_tx.send(Msg::Progress(done, total));
            };
            let summary = rasterbate(&image, &opts, &mut pdf, &mut on_page);
            let result = pdf.save(&output).map(|_| summary).map_err(|e| e.to_string());
            let _ = tx.send(Msg::Done(result));
            ctx.request_repaint();
        });
    }

    fn poll_worker(&mut self, ctx: &egui::Context) {
        let mut finished = None;
        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Msg::Progress(done, total) => {
                        self.pages_done = done;
                        self.pages_total = total;
                    }
                    Msg::Done(result) => finished = Some(result),
                }
            }
        }
        if let Some(result) = finished {
            self.rx = None;
            self.running = false;
            self.result = Some(result);
            self.step = Step::Done;
        }
        if self.running {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

/// Default output name: source file name with `.pdf`, numbered if taken.
fn suggested_output(source: &str) -> String {
    let path = std::path::Path::new(source);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "rasterbation".into());
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut candidate = dir.join(format!("{stem}.pdf"));
    let mut n = 0;
    while candidate.exists() {
        n += 1;
        candidate = dir.join(format!("{stem}_{n}.pdf"));
    }
    candidate.to_string_lossy().to_string()
}

fn open_file(path: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", path])
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

/// Format a length in mm the way the wizard shows output sizes
/// (cm below 1000 mm, meters above).
fn format_length(mm: f64) -> String {
    if mm <= 1000.0 {
        format!("{:.1} cm", mm / 10.0)
    } else {
        format!("{:.2} m", mm / 1000.0)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker(ctx);

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("The Rasterbator");
                ui.label("(Rust port)");
            });
            let titles = [
                (Step::Welcome, "Welcome"),
                (Step::Source, "1. Select source image"),
                (Step::Paper, "2. Select paper size"),
                (Step::Size, "3. Define output size"),
                (Step::Options, "4. Set rasterbation options"),
                (Step::SaveAs, "5. Save rasterbation as"),
                (Step::Running, "Rasterbating..."),
                (Step::Done, "Completed"),
            ];
            for (step, label) in titles {
                if step == self.step {
                    ui.label(egui::RichText::new(label).strong());
                    break;
                }
            }
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.step {
            Step::Welcome => self.ui_welcome(ui),
            Step::Source => self.ui_source(ui, ctx),
            Step::Paper => self.ui_paper(ui),
            Step::Size => self.ui_size(ui),
            Step::Options => self.ui_options(ui),
            Step::SaveAs => self.ui_save_as(ui),
            Step::Running => self.ui_running(ui),
            Step::Done => self.ui_done(ui, ctx),
        });
    }
}

impl App {
    fn ui_welcome(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.label("Welcome!");
        ui.add_space(8.0);
        ui.label(
            "The Rasterbator creates huge, rasterized images from any picture. \
             The rasterbated image can be printed and assembled into an \
             enormous poster. Simply upload the image and adjust the settings.",
        );
        ui.add_space(8.0);
        ui.label("The rasterbation process is wizard-like: it asks you questions and you press Continue.");
        ui.add_space(24.0);
        if ui.button("  Continue  ").clicked() {
            self.step = Step::Source;
        }
    }

    fn ui_source(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(10.0);
        ui.label("Select the image you want to rasterbate.");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Source image:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.source_path)
                    .desired_width(420.0)
                    .hint_text("path to an image file"),
            );
            if response.changed() {
                self.load_image(ctx);
            }
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Images",
                        &["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp"],
                    )
                    .pick_file()
                {
                    self.source_path = path.to_string_lossy().to_string();
                    self.load_image(ctx);
                }
            }
        });

        if let Some(e) = &self.image_error {
            ui.colored_label(egui::Color32::RED, e);
        }
        if let Some(image) = &self.image {
            ui.add_space(4.0);
            ui.label(format!("{} x {} pixels", image.width(), image.height()));
        }
        ui.add_space(12.0);
        if let Some(texture) = &self.texture {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(360.0, 300.0), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let size = fit_size(
                egui::vec2(texture.size()[0] as f32, texture.size()[1] as f32),
                rect.size(),
            );
            let img_rect = egui::Rect::from_center_size(rect.center(), size);
            painter.image(
                texture.id(),
                img_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                self.step = Step::Welcome;
            }
            let enabled = self.image.is_some();
            if ui
                .add_enabled(enabled, egui::Button::new("Continue"))
                .clicked()
            {
                self.step = Step::Paper;
            }
        });
    }

    fn ui_paper(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label("Select the size of the paper you are going to print the rasterbation on.");
        ui.add_space(10.0);
        ui.radio_value(&mut self.paper_standard, true, "Standard paper size");
        ui.radio_value(&mut self.paper_standard, false, "Custom paper size");

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Paper:");
            let preset_name = rasterbator_core::PAPER_PRESETS[self.preset_idx].name;
            egui::ComboBox::from_id_salt("preset")
                .selected_text(preset_name)
                .show_ui(ui, |ui| {
                    for (i, p) in rasterbator_core::PAPER_PRESETS.iter().enumerate() {
                        ui.selectable_value(&mut self.preset_idx, i, p.name);
                    }
                });
            ui.label("Alignment:");
            egui::ComboBox::from_id_salt("orientation")
                .selected_text(if self.landscape { "Landscape" } else { "Portrait" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.landscape, false, "Portrait");
                    ui.selectable_value(&mut self.landscape, true, "Landscape");
                });
        });

        ui.add_space(8.0);
        ui.add_enabled_ui(!self.paper_standard, |ui| {
            ui.horizontal(|ui| {
                ui.label("Custom width:");
                ui.add(
                    egui::DragValue::new(&mut self.custom_w)
                        .range(50.0..=10000.0)
                        .suffix(" mm"),
                );
                ui.label("Custom height:");
                ui.add(
                    egui::DragValue::new(&mut self.custom_h)
                        .range(50.0..=10000.0)
                        .suffix(" mm"),
                );
            });
        });

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                self.step = Step::Source;
            }
            if ui.button("Continue").clicked() {
                self.step = Step::Size;
            }
        });
    }

    fn ui_size(&mut self, ui: &mut egui::Ui) {
        let Some(resolved) = self.resolved() else {
            ui.label("No source image loaded.");
            return;
        };

        ui.add_space(10.0);
        ui.label("Define the size of the poster. The other dimension is calculated from the source image.");
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("Number of paper sheets:");
            ui.add(
                egui::DragValue::new(&mut self.output_value)
                    .range(1.0..=1000.0)
                    .speed(0.1)
                    .fixed_decimals(1),
            );
            egui::ComboBox::from_id_salt("measure")
                .selected_text(if self.measure_high { "high" } else { "wide" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.measure_high, false, "wide");
                    ui.selectable_value(&mut self.measure_high, true, "high");
                });
        });

        ui.add_space(8.0);
        let sheets_x = resolved.sheets_x.ceil();
        let sheets_y = resolved.sheets_y.ceil();
        let total = sheets_x * sheets_y;
        ui.label(format!(
            "Output image size:\n{} x {}",
            format_length(resolved.width_mm),
            format_length(resolved.height_mm)
        ));
        ui.add_space(4.0);
        ui.label(format!(
            "Paper consumption:\n{} x {} = {} sheet{}",
            sheets_x,
            sheets_y,
            total,
            if total == 1.0 { "" } else { "s" }
        ));

        // Preview with page splitting lines, like the wizard's picture box.
        ui.add_space(10.0);
        if let (Some(texture), Some(image)) = (&self.texture, &self.image) {
            let box_size = egui::vec2(340.0, 300.0);
            let (rect, _) = ui.allocate_exact_size(box_size, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let size = fit_size(
                egui::vec2(image.width() as f32, image.height() as f32),
                rect.size(),
            );
            let img_rect = egui::Rect::from_center_size(rect.center(), size);
            painter.image(
                texture.id(),
                img_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            let line = egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 136));
            let line_white =
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 136));
            let step_x = size.x / resolved.sheets_x as f32;
            let mut x = step_x;
            while x < size.x - 0.5 {
                let px = img_rect.left() + x;
                painter.line_segment(
                    [egui::pos2(px, img_rect.top()), egui::pos2(px, img_rect.bottom())],
                    line_white,
                );
                painter.line_segment(
                    [
                        egui::pos2(px + 1.0, img_rect.top()),
                        egui::pos2(px + 1.0, img_rect.bottom()),
                    ],
                    line,
                );
                x += step_x;
            }
            let step_y = size.y / resolved.sheets_y as f32;
            let mut y = step_y;
            while y < size.y - 0.5 {
                let py = img_rect.top() + y;
                painter.line_segment(
                    [egui::pos2(img_rect.left(), py), egui::pos2(img_rect.right(), py)],
                    line_white,
                );
                painter.line_segment(
                    [
                        egui::pos2(img_rect.left(), py + 1.0),
                        egui::pos2(img_rect.right(), py + 1.0),
                    ],
                    line,
                );
                y += step_y;
            }
        }

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                self.step = Step::Paper;
            }
            if ui.button("Continue").clicked() {
                self.step = Step::Options;
            }
        });
    }

    fn ui_options(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label("Set the rasterbation options.");
        ui.add_space(10.0);

        ui.checkbox(
            &mut self.cutout,
            "Draw cutout line around the rasterbated area",
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Dot size:");
            ui.add(
                egui::DragValue::new(&mut self.dot_size)
                    .range(1.0..=500.0)
                    .fixed_decimals(0)
                    .suffix(" mm"),
            );
        });
        ui.label("Dot size is the maximum size of the dots. Good values are between 5 and 25 mm.");
        ui.add_space(8.0);

        ui.label("Color mode:");
        ui.radio_value(&mut self.color, ColorChoice::Black, "Black");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.color, ColorChoice::Custom, "Custom color");
            let changed = egui::color_picker::color_edit_button_srgb(
                ui,
                &mut self.custom_color,
            )
            .changed();
            if changed {
                self.color = ColorChoice::Custom;
            }
        });
        ui.radio_value(&mut self.color, ColorChoice::Average, "Multi-color (average color)");

        if self.confirm_small_dots {
            ui.add_space(12.0);
            ui.colored_label(
                egui::Color32::from_rgb(180, 90, 0),
                "The dot size you selected is very small. This will produce a very \
                 detailed rasterbation with a big file size. Do you want to continue?",
            );
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    self.confirm_small_dots = false;
                    self.step = Step::SaveAs;
                }
                if ui.button("No").clicked() {
                    self.confirm_small_dots = false;
                }
            });
            return;
        }

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                self.step = Step::Size;
            }
            if ui.button("Continue").clicked() {
                if self.dot_size < 3.0 {
                    self.confirm_small_dots = true;
                } else {
                    self.step = Step::SaveAs;
                }
            }
        });
    }

    fn ui_save_as(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label("Save the rasterbation as a PDF file.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("File name:");
            ui.add(
                egui::TextEdit::singleline(&mut self.output_path)
                    .desired_width(420.0)
                    .hint_text("output .pdf path"),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .save_file()
                {
                    self.output_path = path.to_string_lossy().to_string();
                }
            }
        });

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                self.step = Step::Options;
            }
            let enabled = !self.output_path.trim().is_empty();
            if ui
                .add_enabled(enabled, egui::Button::new("  Rasterbate!  "))
                .clicked()
            {
                self.start(ui.ctx());
            }
        });
    }

    fn ui_running(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.label("Rasterbating. This may take a while; the more pages and smaller the dots, the longer it takes.");
        ui.add_space(14.0);
        let fraction = if self.pages_total > 0 {
            self.pages_done as f32 / self.pages_total as f32
        } else {
            0.0
        };
        ui.add(
            egui::ProgressBar::new(fraction)
                .desired_width(460.0)
                .show_percentage(),
        );
        ui.add_space(6.0);
        ui.label(format!("Page {} of {}", self.pages_done, self.pages_total));
        if let Some(started) = self.started {
            let elapsed = started.elapsed().as_secs_f64();
            let remaining = if fraction > 0.02 {
                Some(elapsed / fraction as f64 * (1.0 - fraction as f64))
            } else {
                None
            };
            if let Some(remaining) = remaining {
                if remaining > 1.0 {
                    ui.label(format!("Estimated time remaining: {}", format_seconds(remaining)));
                }
            }
        }
    }

    fn ui_done(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(20.0);
        let ok = matches!(self.result, Some(Ok(_)));
        if ok {
            ui.heading("Rasterbation complete!");
            if let Some(Ok(summary)) = &self.result {
                ui.add_space(6.0);
                ui.label(format!(
                    "{} page(s), {} x {} sheets.",
                    summary.total_pages, summary.pages_x, summary.pages_y
                ));
            }
        } else {
            ui.colored_label(egui::Color32::RED, "An error occurred while rasterbating.");
            if let Some(Err(message)) = &self.result {
                ui.label(message.clone());
            }
        }

        ui.add_space(14.0);
        ui.checkbox(&mut self.open_after, "Open the produced PDF when closing");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Open now").clicked() {
                open_file(&self.output_path);
            }
            if ui.button("Close").clicked() {
                if self.open_after && ok {
                    open_file(&self.output_path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

fn format_seconds(seconds: f64) -> String {
    let s = seconds.round() as u64;
    if s >= 60 {
        format!("{} min {} s", s / 60, s % 60)
    } else {
        format!("{s} s")
    }
}

/// Scale `image` to fit into `bounds` keeping the aspect ratio (like the
/// wizard's picture box).
fn fit_size(image: egui::Vec2, bounds: egui::Vec2) -> egui::Vec2 {
    if image.x <= 0.0 || image.y <= 0.0 {
        return bounds;
    }
    let scale = (bounds.x / image.x).min(bounds.y / image.y);
    image * scale
}
