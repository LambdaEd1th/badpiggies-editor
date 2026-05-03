//! View menu — themes, locale, panels.

use eframe::egui;

use crate::i18n::locale::I18n;

use super::super::EditorApp;

impl EditorApp {
    pub(super) fn menu_view(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
        ui.menu_button(t.get("menu_view"), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let has_level = self.tabs[self.active_tab].level.is_some();
            if has_level {
                if ui.button(t.get("menu_fit_view")).clicked() {
                    ui.close();
                    self.tabs[self.active_tab].renderer.fit_to_level();
                }
                ui.separator();
                {
                    let mut v = self.show_object_tree;
                    if ui.checkbox(&mut v, t.get("menu_object_list")).clicked() {
                        ui.close();
                        self.show_object_tree = v;
                    }
                }
                {
                    let mut v = self.show_properties;
                    if ui.checkbox(&mut v, t.get("menu_properties")).clicked() {
                        ui.close();
                        self.show_properties = v;
                    }
                }
                {
                    let mut v = self.show_tools;
                    if ui.checkbox(&mut v, t.get("tool_window_title")).clicked() {
                        ui.close();
                        self.show_tools = v;
                    }
                }
                ui.separator();
                {
                    let mut v = self.tabs[self.active_tab].renderer.show_bg;
                    if ui.checkbox(&mut v, t.get("menu_background")).clicked() {
                        ui.close();
                        self.tabs[self.active_tab].renderer.show_bg = v;
                    }
                }
                {
                    let mut v = self.tabs[self.active_tab].renderer.show_grid;
                    if ui.checkbox(&mut v, t.get("menu_grid")).clicked() {
                        ui.close();
                        self.tabs[self.active_tab].renderer.show_grid = v;
                    }
                }
                {
                    let mut v = self.tabs[self.active_tab].renderer.show_ground;
                    if ui.checkbox(&mut v, t.get("menu_physics_ground")).clicked() {
                        ui.close();
                        self.tabs[self.active_tab].renderer.show_ground = v;
                    }
                }
                {
                    let mut v = self.tabs[self.active_tab].renderer.show_level_bounds;
                    if ui.checkbox(&mut v, t.get("menu_level_bounds")).clicked() {
                        ui.close();
                        self.tabs[self.active_tab].renderer.show_level_bounds = v;
                    }
                }
                if self.tabs[self.active_tab].renderer.is_dark_level() {
                    let mut v = self.tabs[self.active_tab].renderer.show_dark_overlay;
                    if ui.checkbox(&mut v, t.get("menu_dark_overlay")).clicked() {
                        ui.close();
                        self.tabs[self.active_tab].renderer.show_dark_overlay = v;
                    }
                }
                {
                    let mut v = self.tabs[self.active_tab].renderer.show_terrain_tris;
                    if ui.checkbox(&mut v, t.get("menu_terrain_tris")).clicked() {
                        ui.close();
                        self.tabs[self.active_tab].renderer.show_terrain_tris = v;
                    }
                }
                ui.separator();
            } // has_level
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                {
                    let mut v = sv.show_xml;
                    if ui.checkbox(&mut v, t.get("save_viewer_raw_xml")).clicked() {
                        ui.close();
                        sv.show_xml = v;
                    }
                }
                {
                    let mut v = sv.show_table;
                    if ui
                        .checkbox(&mut v, t.get("save_viewer_structured"))
                        .clicked()
                    {
                        ui.close();
                        sv.show_table = v;
                    }
                }
                if matches!(sv.data, Some(crate::io::save::parser::SaveData::Contraption(_))) {
                    let mut v = sv.show_preview;
                    if ui
                        .checkbox(&mut v, t.get("contraption_preview_title"))
                        .clicked()
                    {
                        ui.close();
                        sv.show_preview = v;
                    }
                }
                ui.separator();
            }
            ui.menu_button(t.get("menu_language"), |ui| {
                for &lang in crate::i18n::locale::Language::ALL {
                    if ui
                        .selectable_label(self.lang == lang, lang.display_name())
                        .clicked()
                    {
                        self.lang = lang;
                        ui.close();
                    }
                }
            });
        });
    }
}
