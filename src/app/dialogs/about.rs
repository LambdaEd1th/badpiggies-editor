//! About dialog window.

use eframe::egui;

use super::super::EditorApp;

impl EditorApp {
    /// About window.
    pub(in crate::app) fn render_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let t = self.t();
        egui::Window::new(t.get("win_about"))
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_about)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Bad Piggies Editor");
                    ui.label(format!(
                        "{}{}",
                        t.get("about_version_prefix"),
                        env!("CARGO_PKG_VERSION")
                    ));
                    ui.separator();
                    ui.hyperlink_to(
                        env!("CARGO_PKG_AUTHORS"),
                        "https://space.bilibili.com/8217621",
                    );
                    ui.label(t.get("about_built_with"));
                    ui.label(t.get("about_license"));
                });
            });
    }
}
