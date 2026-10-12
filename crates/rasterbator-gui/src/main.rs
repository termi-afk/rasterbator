//! Rasterbator desktop GUI: startup mode chooser + classic wizard +
//! modern workspace.
//!
//! The classic wizard lives in [`legacy`] (moved verbatim from the original
//! `main.rs`; only the struct was renamed `App` -> `LegacyApp`). The modern
//! workspace lives in [`modern_app`]. Window-free logic (settings,
//! inert-reason table, display mapping) lives in [`logic`] and is covered
//! by unit tests that never open a window.

// Build as a Windows GUI application: without this, Windows gives the process
// a console window (Rust defaults to the console subsystem). Ignored on
// non-Windows targets. For console output while debugging, temporarily
// remove this attribute.
#![windows_subsystem = "windows"]

mod legacy;
mod logic;
mod compat;
mod modern_app;

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;
use rasterbator_core::modern::options::default_preset_dir;

use legacy::LegacyApp;
use logic::{AppMode, UiSettings};
use modern_app::ModernApp;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("The Rasterbator"),
        ..Default::default()
    };
    eframe::run_native(
        "Rasterbator",
        native_options,
        Box::new(|_cc| Ok(Box::new(RootApp::new()))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RootMode {
    Chooser,
    Legacy,
    Modern,
}

struct RootApp {
    mode: RootMode,
    legacy: LegacyApp,
    modern: ModernApp,
    remember: bool,
    last_mode: Option<AppMode>,
    store_dir: PathBuf,
    last_save: Instant,
}

impl RootApp {
    fn new() -> Self {
        let store_dir = default_preset_dir();
        let settings = UiSettings::load(&store_dir);
        let mode = match (settings.remember, settings.last_mode) {
            (true, Some(AppMode::Legacy)) => RootMode::Legacy,
            (true, Some(AppMode::Modern)) => RootMode::Modern,
            _ => RootMode::Chooser,
        };
        RootApp {
            mode,
            legacy: LegacyApp::default(),
            modern: ModernApp::new(&settings),
            remember: settings.remember,
            last_mode: settings.last_mode,
            store_dir,
            last_save: Instant::now(),
        }
    }

    fn collect_settings(&self) -> UiSettings {
        let (opts, detail) = self.modern.snapshot();
        UiSettings { opts, last_mode: self.last_mode, remember: self.remember, preview_detail: detail }
    }

    fn save_settings(&mut self) {
        let s = self.collect_settings();
        // Best-effort: a failed settings write must never break the session.
        let _ = s.save(&self.store_dir);
        self.last_save = Instant::now();
    }
}

impl eframe::App for RootApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Persist settings on close and every 30 s.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_settings();
        } else if self.last_save.elapsed().as_secs() >= 30 {
            self.save_settings();
        }

        match self.mode {
            RootMode::Chooser => self.ui_chooser(ctx),
            RootMode::Legacy => {
                egui::TopBottomPanel::top("modeswitch").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("classic wizard").small().weak());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Switch mode").clicked() {
                                self.mode = RootMode::Chooser;
                            }
                        });
                    });
                });
                self.legacy.update(ctx, frame);
            }
            RootMode::Modern => {
                self.modern.show(ctx);
                if self.modern.request_switch {
                    self.modern.request_switch = false;
                    self.mode = RootMode::Chooser;
                }
            }
        }
    }
}

impl RootApp {
    fn choose(&mut self, mode: AppMode) {
        self.last_mode = Some(mode);
        self.mode = match mode {
            AppMode::Legacy => RootMode::Legacy,
            AppMode::Modern => RootMode::Modern,
        };
        // Persist the choice immediately so it survives a crash.
        self.save_settings();
    }

    fn ui_chooser(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(24.0);
            ui.heading("The Rasterbator");
            ui.add_space(4.0);
            ui.label("Choose your workspace. Either mode keeps its own state when you switch.");
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.set_min_size(egui::vec2(300.0, 150.0));
                    ui.vertical(|ui| {
                        ui.heading("Classic wizard");
                        ui.label("The original five-step flow, bit-identical output.");
                        ui.add_space(8.0);
                        if ui.button("  Open classic wizard  ").clicked() {
                            self.choose(AppMode::Legacy);
                        }
                    });
                });
                ui.group(|ui| {
                    ui.set_min_size(egui::vec2(300.0, 150.0));
                    ui.vertical(|ui| {
                        ui.heading("Modern workspace");
                        ui.label("Full option surface with live preview.");
                        ui.add_space(8.0);
                        if ui.button("  Open modern workspace  ").clicked() {
                            self.choose(AppMode::Modern);
                        }
                    });
                });
            });
            ui.add_space(12.0);
            if ui.checkbox(&mut self.remember, "Remember my choice").changed() {
                self.save_settings();
            }
        });
    }
}
