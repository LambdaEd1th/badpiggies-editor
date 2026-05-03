//! Help menu — shortcuts + about.

use eframe::egui;

use crate::locale::I18n;

use super::super::EditorApp;
#[cfg(not(target_arch = "wasm32"))]
use super::super::civil_from_days;
#[cfg(target_arch = "wasm32")]
use super::file::export_bytes_wasm;

impl EditorApp {
    pub(super) fn menu_help(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: &'static I18n) {
        let _ = ctx;
        ui.menu_button(t.get("menu_help"), |ui| {
            ui.set_min_width(80.0);
            if ui.button(t.get("menu_export_log")).clicked() {
                ui.close();
                let lines = crate::log_buffer::drain();
                let content = lines.join("\n");
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let log_name = {
                        use std::time::{SystemTime, UNIX_EPOCH};
                        let secs = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let s = secs % 60;
                        let m = (secs / 60) % 60;
                        let h = (secs / 3600) % 24;
                        let days = secs / 86400;
                        let (y, mo, d) = civil_from_days(days as i64);
                        format!("{:04}{:02}{:02}_{:02}{:02}{:02}.log", y, mo, d, h, m, s)
                    };
                    if let Some(path) = rfd::FileDialog::new().set_file_name(&log_name).save_file()
                    {
                        if let Err(e) = std::fs::write(&path, &content) {
                            self.tabs[self.active_tab].status = format!("Log export error: {e}");
                        } else {
                            self.tabs[self.active_tab].status =
                                format!("Log exported: {}", path.display());
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    if let Err(e) = export_bytes_wasm("editor.log", content.into_bytes()) {
                        self.tabs[self.active_tab].status = format!("Log export error: {e}");
                    }
                }
            }
            ui.separator();
            if ui.button(t.get("menu_shortcuts")).clicked() {
                ui.close();
                self.show_shortcuts = true;
            }
            if ui.button(t.get("menu_about")).clicked() {
                ui.close();
                self.show_about = true;
            }
        });
    }
}
