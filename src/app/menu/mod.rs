//! Menu bar — File, Edit, View, Help.

mod edit;
mod file;
mod help;
mod view;

use eframe::egui;

use crate::error::AppError;
use crate::locale::I18n;

use super::EditorApp;

pub(super) fn status_export_error_message(t: &'static I18n, error: impl Into<AppError>) -> String {
    let error = error.into();
    t.fmt1("status_export_error", &error.localized(t))
}

impl EditorApp {
    pub(super) fn render_menu_bar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        t: &'static I18n,
    ) {
        egui::Panel::top("menu_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.menu_file(ui, ctx, t);
                self.menu_edit(ui, t);
                self.menu_view(ui, t);
                self.menu_help(ui, ctx, t);
            });
        });
    }
}
