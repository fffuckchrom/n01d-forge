//! Dark cyberpunk theme for n01d-forge

use eframe::egui::{self, Color32, Rounding, Stroke, Vec2};

pub const BG_DARK: Color32 = Color32::from_rgb(12, 15, 20);
pub const BG_PANEL: Color32 = Color32::from_rgb(20, 25, 32);
pub const BG_WIDGET: Color32 = Color32::from_rgb(30, 36, 45);
pub const ACCENT: Color32 = Color32::from_rgb(0, 212, 170);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0, 150, 120);
pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(235, 235, 235);
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 150, 160);
pub const DANGER: Color32 = Color32::from_rgb(255, 80, 90);
pub const WARNING: Color32 = Color32::from_rgb(255, 170, 0);
pub const SUCCESS: Color32 = Color32::from_rgb(0, 200, 150);
pub const BORDER: Color32 = Color32::from_rgb(50, 60, 70);

pub fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG_DARK;
    style.visuals.window_fill = BG_PANEL;
    style.visuals.extreme_bg_color = BG_WIDGET;
    
    style.visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    style.visuals.widgets.noninteractive.rounding = Rounding::same(6.0);
    
    style.visuals.widgets.inactive.bg_fill = BG_WIDGET;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    style.visuals.widgets.inactive.rounding = Rounding::same(6.0);
    
    style.visuals.widgets.hovered.bg_fill = BG_WIDGET;
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.hovered.rounding = Rounding::same(6.0);
    
    style.visuals.widgets.active.bg_fill = ACCENT_DIM;
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, BG_DARK);
    style.visuals.widgets.active.rounding = Rounding::same(6.0);
    
    style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.25);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.window_rounding = Rounding::same(10.0);
    style.visuals.window_stroke = Stroke::new(1.0, BORDER);
    
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    
    ctx.set_style(style);
}
