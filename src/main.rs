//! n01d-forge - Secure Image Burner

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

mod theme;
mod ui;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_min_inner_size([700.0, 500.0])
            .with_title("n01d-forge"),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "n01d-forge",
        options,
        Box::new(|cc| {
            theme::setup_style(&cc.egui_ctx);
            Box::new(ui::ForgeApp::new())
        }),
    )
}
