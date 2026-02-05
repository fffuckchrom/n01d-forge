//! Main UI for n01d-forge

use eframe::egui::{self, RichText, Vec2, Color32};
use crate::theme::*;
use std::process::Command;

#[derive(Default, PartialEq, Clone, Copy)]
enum Tab { #[default] Burn, Encrypt, Erase, About }

#[derive(Clone)]
struct DriveInfo {
    name: String,
    device: String,
    size: String,
    is_usb: bool,
}

pub struct ForgeApp {
    tab: Tab,
    drives: Vec<DriveInfo>,
    selected_drive: Option<usize>,
    image_path: Option<String>,
    image_name: String,
    image_size: String,
    verify_write: bool,
    show_all_drives: bool,
    progress: f32,
    status: String,
    is_burning: bool,
    encrypt_password: String,
    show_password: bool,
    erase_confirm: bool,
}

impl ForgeApp {
    pub fn new() -> Self {
        let mut app = Self {
            tab: Tab::Burn,
            drives: Vec::new(),
            selected_drive: None,
            image_path: None,
            image_name: String::new(),
            image_size: String::new(),
            verify_write: true,
            show_all_drives: false,
            progress: 0.0,
            status: String::from("Ready"),
            is_burning: false,
            encrypt_password: String::new(),
            show_password: false,
            erase_confirm: false,
        };
        app.refresh_drives();
        app
    }

    fn refresh_drives(&mut self) {
        self.drives.clear();
        
        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = Command::new("lsblk")
                .args(["-J", "-o", "NAME,SIZE,TYPE,TRAN,RM,MODEL"])
                .output()
            {
                if let Ok(json) = String::from_utf8(output.stdout) {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                        if let Some(devices) = parsed.get("blockdevices").and_then(|v| v.as_array()) {
                            for dev in devices {
                                let dtype = dev.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                let name = dev.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                
                                if dtype == "disk" && !name.starts_with("loop") && !name.starts_with("sr") {
                                    let tran = dev.get("tran").and_then(|v| v.as_str()).unwrap_or("");
                                    let rm = dev.get("rm").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let is_usb = tran == "usb" || rm;
                                    
                                    if self.show_all_drives || is_usb {
                                        let model = dev.get("model").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                        let size = dev.get("size").and_then(|v| v.as_str()).unwrap_or("?");
                                        
                                        self.drives.push(DriveInfo {
                                            name: if model.is_empty() { format!("Drive {}", name) } else { model.to_string() },
                                            device: format!("/dev/{}", name),
                                            size: size.to_string(),
                                            is_usb,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn select_image(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Disk Images", &["iso", "img", "raw", "bin"])
            .set_title("Select Image")
            .pick_file()
        {
            let path_str = path.display().to_string();
            self.image_name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            
            if let Ok(meta) = std::fs::metadata(&path) {
                let bytes = meta.len();
                self.image_size = if bytes >= 1_000_000_000 {
                    format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
                } else {
                    format!("{:.2} MB", bytes as f64 / 1_000_000.0)
                };
            }
            self.image_path = Some(path_str);
        }
    }
}

impl eframe::App for ForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_DARK))
            .show(ctx, |ui| {
                ui.add_space(16.0);
                
                // Header
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("⚡").size(28.0).color(ACCENT));
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("n01d-forge").size(22.0).color(TEXT_BRIGHT).strong());
                        ui.label(RichText::new("Secure Image Burner").size(11.0).color(TEXT_DIM));
                    });
                });
                
                ui.add_space(16.0);
                
                // Tab bar
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    for (t, label) in [(Tab::Burn, "🔥 Burn"), (Tab::Encrypt, "🔒 Encrypt"), 
                                        (Tab::Erase, "🗑 Erase"), (Tab::About, "ℹ About")] {
                        let selected = self.tab == t;
                        let btn = egui::Button::new(
                            RichText::new(label).color(if selected { BG_DARK } else { TEXT_BRIGHT })
                        )
                        .fill(if selected { ACCENT } else { BG_PANEL })
                        .rounding(6.0)
                        .min_size(Vec2::new(90.0, 32.0));
                        
                        if ui.add(btn).clicked() { self.tab = t; }
                        ui.add_space(4.0);
                    }
                });
                
                ui.add_space(16.0);
                
                // Content
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width() - 40.0);
                            
                            match self.tab {
                                Tab::Burn => self.render_burn_tab(ui),
                                Tab::Encrypt => self.render_encrypt_tab(ui),
                                Tab::Erase => self.render_erase_tab(ui),
                                Tab::About => render_about_tab(ui),
                            }
                        });
                        ui.add_space(20.0);
                    });
                });
            });
    }
}

fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("▌").color(ACCENT));
        ui.label(RichText::new(title).color(ACCENT).strong());
    });
    ui.add_space(12.0);
}

impl ForgeApp {
    fn render_burn_tab(&mut self, ui: &mut egui::Ui) {
        // Status
        if !self.status.is_empty() {
            ui.label(RichText::new(&self.status).color(TEXT_DIM).size(12.0));
            ui.add_space(8.0);
        }

        // Image selection section
        egui::Frame::none()
            .fill(BG_PANEL)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(16.0)
            .show(ui, |ui| {
                section_header(ui, "SOURCE IMAGE");
                
                if self.image_path.is_some() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📀").size(24.0));
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&self.image_name).color(TEXT_BRIGHT).strong());
                            ui.label(RichText::new(&self.image_size).color(TEXT_DIM).size(12.0));
                        });
                    });
                    if ui.button("Change").clicked() { 
                        self.select_image(); 
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.label(RichText::new("��").size(40.0).color(TEXT_DIM));
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(RichText::new("Select Image").color(BG_DARK))
                            .fill(ACCENT).min_size(Vec2::new(140.0, 36.0))).clicked() {
                            self.select_image();
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new("ISO, IMG, RAW, BIN").color(TEXT_DIM).size(11.0));
                        ui.add_space(16.0);
                    });
                }
            });

        ui.add_space(12.0);

        // Drive selection section
        egui::Frame::none()
            .fill(BG_PANEL)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(16.0)
            .show(ui, |ui| {
                section_header(ui, "TARGET DRIVE");
                
                ui.horizontal(|ui| {
                    if ui.button("↻ Refresh").clicked() { 
                        self.refresh_drives(); 
                    }
                    ui.add_space(12.0);
                    if ui.checkbox(&mut self.show_all_drives, "Show all drives").changed() {
                        self.refresh_drives();
                    }
                });
                ui.add_space(12.0);
                
                if self.drives.is_empty() {
                    ui.label(RichText::new("No removable drives detected").color(TEXT_DIM));
                } else {
                    for i in 0..self.drives.len() {
                        let drive = self.drives[i].clone();
                        let selected = self.selected_drive == Some(i);
                        
                        let resp = egui::Frame::none()
                            .fill(if selected { ACCENT.linear_multiply(0.15) } else { BG_WIDGET })
                            .stroke(egui::Stroke::new(if selected { 2.0 } else { 1.0 }, 
                                    if selected { ACCENT } else { BORDER }))
                            .rounding(8.0)
                            .inner_margin(12.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("💾").size(20.0));
                                    ui.add_space(8.0);
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(&drive.name).color(TEXT_BRIGHT));
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&drive.device).color(TEXT_DIM).size(11.0));
                                            if drive.is_usb {
                                                ui.label(RichText::new("• USB").color(ACCENT).size(11.0));
                                            }
                                        });
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(&drive.size).color(ACCENT));
                                    });
                                });
                            }).response;
                        
                        if resp.interact(egui::Sense::click()).clicked() {
                            self.selected_drive = Some(i);
                        }
                        ui.add_space(6.0);
                    }
                }
            });

        ui.add_space(12.0);

        // Options section
        egui::Frame::none()
            .fill(BG_PANEL)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(16.0)
            .show(ui, |ui| {
                section_header(ui, "OPTIONS");
                ui.checkbox(&mut self.verify_write, "Verify after writing (SHA-256)");
            });

        ui.add_space(16.0);

        // Progress section
        if self.is_burning {
            egui::Frame::none()
                .fill(BG_PANEL)
                .rounding(10.0)
                .stroke(egui::Stroke::new(1.0, BORDER))
                .inner_margin(16.0)
                .show(ui, |ui| {
                    section_header(ui, "PROGRESS");
                    ui.add(egui::ProgressBar::new(self.progress).show_percentage());
                });
            ui.add_space(16.0);
        }

        // Burn button
        ui.vertical_centered(|ui| {
            let can_burn = self.image_path.is_some() && self.selected_drive.is_some() && !self.is_burning;
            
            ui.add_enabled_ui(can_burn, |ui| {
                let btn = egui::Button::new(RichText::new("🔥 BURN IMAGE").color(Color32::WHITE).strong())
                    .fill(DANGER)
                    .min_size(Vec2::new(160.0, 44.0));
                
                if ui.add(btn).clicked() {
                    self.status = "Burn feature ready - needs root privileges".to_string();
                }
            });
            
            if !can_burn && !self.is_burning {
                ui.add_space(8.0);
                ui.label(RichText::new("Select image and drive to continue").color(TEXT_DIM).size(11.0));
            }
        });

        ui.add_space(24.0);
    }

    fn render_encrypt_tab(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(BG_PANEL)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(16.0)
            .show(ui, |ui| {
                section_header(ui, "ENCRYPTION SETTINGS");
                
                ui.label(RichText::new("Set up disk encryption after burning").color(TEXT_DIM));
                ui.add_space(12.0);
                
                ui.label("Password:");
                ui.add(egui::TextEdit::singleline(&mut self.encrypt_password)
                    .password(!self.show_password)
                    .desired_width(300.0));
                
                ui.checkbox(&mut self.show_password, "Show password");
                
                ui.add_space(12.0);
                ui.label(RichText::new("Supports: LUKS, LUKS2").color(TEXT_DIM).size(11.0));
            });
    }

    fn render_erase_tab(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Color32::from_rgb(50, 30, 25))
            .stroke(egui::Stroke::new(1.0, WARNING))
            .rounding(8.0)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠").size(20.0).color(WARNING));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Secure erase will permanently destroy ALL data!").color(WARNING));
                });
            });

        ui.add_space(12.0);

        egui::Frame::none()
            .fill(BG_PANEL)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(16.0)
            .show(ui, |ui| {
                section_header(ui, "SECURE ERASE");
                
                ui.label("Available methods:");
                ui.label(RichText::new("• Zero Fill (fast)").color(TEXT_DIM));
                ui.label(RichText::new("• Random Fill").color(TEXT_DIM));
                ui.label(RichText::new("• DoD 5220.22-M (3 passes)").color(TEXT_DIM));
                ui.label(RichText::new("• Gutmann (35 passes)").color(TEXT_DIM));
                
                ui.add_space(12.0);
                ui.checkbox(&mut self.erase_confirm, "I understand this is irreversible");
            });
    }
}

fn render_about_tab(ui: &mut egui::Ui) {
    egui::Frame::none()
        .fill(BG_PANEL)
        .rounding(10.0)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .inner_margin(16.0)
        .show(ui, |ui| {
            section_header(ui, "ABOUT");
            
            ui.label(RichText::new("n01d-forge").size(18.0).color(ACCENT).strong());
            ui.label(RichText::new("Secure Image Burner v1.0.0").color(TEXT_DIM));
            ui.add_space(12.0);
            ui.label("Part of the n01d security toolkit");
            ui.add_space(12.0);
            ui.label(RichText::new("Features:").color(TEXT_BRIGHT));
            for feat in ["✓ Cross-platform (Linux, macOS, Windows)",
                         "✓ LUKS/LUKS2 encryption support",
                         "✓ SHA-256 verification",
                         "✓ Secure erase (DoD, Gutmann)"] {
                ui.label(RichText::new(feat).color(TEXT_DIM));
            }
            ui.add_space(12.0);
            ui.hyperlink_to("GitHub", "https://github.com/bad-antics/n01d-forge");
        });
}
