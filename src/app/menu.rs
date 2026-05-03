//! Menu bar rendering — File, Edit, View, Help menus.

use eframe::egui;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::locale::I18n;
use crate::types::*;

use super::EditorApp;

#[cfg(target_arch = "wasm32")]
use super::{WASM_OPEN_RESULT, WASM_OPEN_XML_SAVE};

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

    fn menu_file(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: &'static I18n) {
        ui.menu_button(t.get("menu_file"), |ui| {
            if ui.button(t.get("menu_open_level")).clicked() {
                ui.close();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Level files", &["bytes"])
                        .pick_file()
                    {
                        match std::fs::read(&path) {
                            Ok(data) => {
                                let name = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                self.load_level_into_tab(
                                    name,
                                    data,
                                    Some(path.to_string_lossy().into_owned()),
                                );
                            }
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    t.fmt1("status_read_error", &e.to_string());
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = ctx.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("Level files", &["bytes"])
                            .pick_file()
                            .await
                        {
                            let name = file.file_name();
                            let data = file.read().await;
                            WASM_OPEN_RESULT.with(|q| {
                                q.borrow_mut().replace((name, data));
                            });
                            repaint_ctx.request_repaint();
                        }
                    });
                }
            }
            if ui.button(t.get("menu_import_text")).clicked() {
                ui.close();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("YAML / TOML", &["yaml", "yml", "toml"])
                        .pick_file()
                    {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        match std::fs::read_to_string(&path) {
                            Ok(text) => {
                                self.load_level_text_into_tab(
                                    name,
                                    &text,
                                    Some(path.to_string_lossy().into_owned()),
                                );
                            }
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    t.fmt1("status_read_error", &e.to_string());
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = ctx.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("YAML / TOML", &["yaml", "yml", "toml"])
                            .pick_file()
                            .await
                        {
                            let name = file.file_name();
                            let data = file.read().await;
                            WASM_OPEN_RESULT.with(|q| {
                                q.borrow_mut().replace((name, data));
                            });
                            repaint_ctx.request_repaint();
                        }
                    });
                }
            }
            if ui.button(t.get("menu_open_save")).clicked() {
                ui.close();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Save files", &["dat", "contraption", "xml"])
                        .pick_file()
                    {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        match std::fs::read(&path) {
                            Ok(data) => {
                                self.load_save_into_tab(name, data);
                            }
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    t.fmt1("status_read_error", &e.to_string());
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = ctx.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("Save files", &["dat", "contraption", "xml"])
                            .pick_file()
                            .await
                        {
                            let name = file.file_name();
                            let data = file.read().await;
                            WASM_OPEN_RESULT.with(|q| {
                                q.borrow_mut().replace((name, data));
                            });
                            repaint_ctx.request_repaint();
                        }
                    });
                }
            }
            if ui.button(t.get("menu_import_xml")).clicked() {
                ui.close();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("XML files", &["xml"])
                        .pick_file()
                    {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        match std::fs::read(&path) {
                            Ok(data) => {
                                self.load_xml_into_tab(name, data);
                            }
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    t.fmt1("status_read_error", &e.to_string());
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = ctx.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("XML files", &["xml"])
                            .pick_file()
                            .await
                        {
                            let name = file.file_name();
                            let data = file.read().await;
                            WASM_OPEN_XML_SAVE.with(|q| {
                                q.borrow_mut().replace((name, data));
                            });
                            repaint_ctx.request_repaint();
                        }
                    });
                }
            }
            let is_save_tab = self.tabs[self.active_tab].is_save_tab();
            let has_level = self.tabs[self.active_tab].level.is_some();
            if is_save_tab || has_level {
                ui.separator();
            }
            if is_save_tab {
                if ui.button(t.get("menu_export_save")).clicked() {
                    ui.close();
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(ref sv) = self.tabs[self.active_tab].save_view {
                            if let Some(encrypted) = sv.export_encrypted() {
                                let default_name = self.tabs[self.active_tab]
                                    .file_name
                                    .as_deref()
                                    .unwrap_or("save.dat");
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_file_name(default_name)
                                    .save_file()
                                {
                                    match std::fs::write(&path, encrypted) {
                                        Ok(()) => {
                                            if let Some(ref mut sv) =
                                                self.tabs[self.active_tab].save_view
                                            {
                                                sv.dirty = false;
                                            }
                                            self.tabs[self.active_tab].status =
                                                t.get("status_exported");
                                        }
                                        Err(e) => {
                                            self.tabs[self.active_tab].status =
                                                t.fmt1("status_export_error", &e.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(ref sv) = self.tabs[self.active_tab].save_view {
                            if let Some(encrypted) = sv.export_encrypted() {
                                let file_name = self.tabs[self.active_tab]
                                    .file_name
                                    .clone()
                                    .unwrap_or_else(|| "save.dat".to_string());
                                match export_bytes_wasm(&file_name, encrypted) {
                                    Ok(()) => {
                                        if let Some(ref mut sv) =
                                            self.tabs[self.active_tab].save_view
                                        {
                                            sv.dirty = false;
                                        }
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e);
                                    }
                                }
                            }
                        }
                    }
                }
                if ui.button(t.get("menu_export_xml")).clicked() {
                    ui.close();
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(ref sv) = self.tabs[self.active_tab].save_view {
                            let default_name = self.tabs[self.active_tab]
                                .file_name
                                .as_deref()
                                .map(|n| format!("{n}.xml"))
                                .unwrap_or_else(|| "save.xml".into());
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("XML files", &["xml"])
                                .set_file_name(&default_name)
                                .save_file()
                            {
                                match std::fs::write(&path, sv.xml_text.as_bytes()) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e.to_string());
                                    }
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(ref sv) = self.tabs[self.active_tab].save_view {
                            let file_name = self.tabs[self.active_tab]
                                .file_name
                                .clone()
                                .map(|n| format!("{n}.xml"))
                                .unwrap_or_else(|| "save.xml".to_string());
                            match export_bytes_wasm(&file_name, sv.xml_text.as_bytes().to_vec()) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status = t.get("status_exported");
                                }
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        t.fmt1("status_export_error", &e);
                                }
                            }
                        }
                    }
                }
            }
            if has_level {
                if ui.button(t.get("menu_export_level")).clicked() {
                    ui.close();
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let default_name = self.tabs[self.active_tab]
                            .file_name
                            .as_deref()
                            .unwrap_or("level.bytes");
                        if let Some(data) = self.tabs[self.active_tab].export_level()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Level files", &["bytes"])
                                .set_file_name(default_name)
                                .save_file()
                        {
                            match std::fs::write(&path, data) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status = t.get("status_exported");
                                }
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        t.fmt1("status_export_error", &e.to_string());
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(data) = self.tabs[self.active_tab].export_level() {
                            let file_name = self.tabs[self.active_tab]
                                .file_name
                                .clone()
                                .unwrap_or_else(|| "level.bytes".to_string());
                            match export_bytes_wasm(&file_name, data) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status = t.get("status_exported");
                                }
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        t.fmt1("status_export_error", &e);
                                }
                            }
                        }
                    }
                }
                if ui.button(t.get("menu_export_yaml")).clicked() {
                    ui.close();
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let yaml_name = self.tabs[self.active_tab]
                            .file_name
                            .as_deref()
                            .map(|n| format!("{n}.yaml"))
                            .unwrap_or_else(|| "level.yaml".into());
                        if let Some(text) = self.tabs[self.active_tab].export_yaml()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("YAML files", &["yaml"])
                                .set_file_name(&yaml_name)
                                .save_file()
                        {
                            match std::fs::write(&path, text.as_bytes()) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status = t.get("status_exported");
                                }
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        t.fmt1("status_export_error", &e.to_string());
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(text) = self.tabs[self.active_tab].export_yaml() {
                            let file_name = self.tabs[self.active_tab]
                                .file_name
                                .as_deref()
                                .map(|n| format!("{n}.yaml"))
                                .unwrap_or_else(|| "level.yaml".to_string());
                            match export_bytes_wasm(&file_name, text.into_bytes()) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status = t.get("status_exported");
                                }
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        t.fmt1("status_export_error", &e);
                                }
                            }
                        }
                    }
                }
                if ui.button(t.get("menu_export_toml")).clicked() {
                    ui.close();
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let toml_name = self.tabs[self.active_tab]
                            .file_name
                            .as_deref()
                            .map(|n| format!("{n}.toml"))
                            .unwrap_or_else(|| "level.toml".into());
                        if let Some(text) = self.tabs[self.active_tab].export_toml()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("TOML files", &["toml"])
                                .set_file_name(&toml_name)
                                .save_file()
                        {
                            match std::fs::write(&path, text.as_bytes()) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status = t.get("status_exported");
                                }
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        t.fmt1("status_export_error", &e.to_string());
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(text) = self.tabs[self.active_tab].export_toml() {
                            let file_name = self.tabs[self.active_tab]
                                .file_name
                                .as_deref()
                                .map(|n| format!("{n}.toml"))
                                .unwrap_or_else(|| "level.toml".to_string());
                            match export_bytes_wasm(&file_name, text.into_bytes()) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status = t.get("status_exported");
                                }
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        t.fmt1("status_export_error", &e);
                                }
                            }
                        }
                    }
                }
            } // level exports
        });
    }

    fn menu_edit(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
        ui.menu_button(t.get("menu_edit"), |ui| {
            ui.set_min_width(120.0);
            let has_level = self.tabs[self.active_tab].level.is_some();
            let is_save_tab = self.tabs[self.active_tab].is_save_tab();
            if is_save_tab {
                self.menu_edit_save(ui, t);
                return;
            }
            if !has_level {
                ui.label(t.get("save_viewer_no_data"));
                return;
            }
            let is_mac = cfg!(target_os = "macos");
            let undo_shortcut = if is_mac { "Cmd+Z" } else { "Ctrl+Z" };
            let redo_shortcut = if is_mac { "Shift+Cmd+Z" } else { "Ctrl+Y" };
            if ui
                .add(egui::Button::new(t.get("menu_undo")).shortcut_text(undo_shortcut))
                .clicked()
            {
                ui.close();
                self.undo();
            }
            if ui
                .add(egui::Button::new(t.get("menu_redo")).shortcut_text(redo_shortcut))
                .clicked()
            {
                ui.close();
                self.redo();
            }
            ui.separator();
            let has_sel = !self.tabs[self.active_tab].selected.is_empty()
                && self.tabs[self.active_tab].level.is_some();
            let has_clip = self.clipboard.is_some() && self.tabs[self.active_tab].level.is_some();
            let copy_shortcut = if is_mac { "Cmd+C" } else { "Ctrl+C" };
            let cut_shortcut = if is_mac { "Cmd+X" } else { "Ctrl+X" };
            let paste_shortcut = if is_mac { "Cmd+V" } else { "Ctrl+V" };
            let dup_shortcut = if is_mac { "Cmd+D" } else { "Ctrl+D" };
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_copy")).shortcut_text(copy_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.copy_selected();
            }
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_cut")).shortcut_text(cut_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.cut_selected();
            }
            if ui
                .add_enabled(
                    has_clip,
                    egui::Button::new(t.get("menu_paste")).shortcut_text(paste_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.paste();
            }
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_duplicate")).shortcut_text(dup_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.duplicate_selected();
            }
            ui.separator();
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_delete")).shortcut_text("Del"),
                )
                .clicked()
            {
                ui.close();
                if !self.tabs[self.active_tab].selected.is_empty()
                    && let Some(ref level) = self.tabs[self.active_tab].level
                {
                    let indices: Vec<ObjectIndex> = self.tabs[self.active_tab]
                        .selected
                        .iter()
                        .copied()
                        .collect();
                    let label = if indices.len() == 1 {
                        level.objects[indices[0]].name().to_string()
                    } else {
                        format!("{} objects", indices.len())
                    };
                    self.tabs[self.active_tab].pending_delete = Some((indices, label));
                }
            }
            ui.separator();
            if ui.button(t.get("menu_add_object")).clicked() {
                ui.close();
                self.prepare_add_object_dialog();
            }
        });
    }

    /// Edit menu contents for save tabs (undo/redo + select all/delete all entries).
    fn menu_edit_save(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
        let is_mac = cfg!(target_os = "macos");
        let undo_shortcut = if is_mac { "Cmd+Z" } else { "Ctrl+Z" };
        let redo_shortcut = if is_mac { "Shift+Cmd+Z" } else { "Ctrl+Y" };

        let can_undo = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| sv.can_undo());
        let can_redo = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| sv.can_redo());

        if ui
            .add_enabled(
                can_undo,
                egui::Button::new(t.get("menu_undo")).shortcut_text(undo_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.undo();
            }
        }
        if ui
            .add_enabled(
                can_redo,
                egui::Button::new(t.get("menu_redo")).shortcut_text(redo_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.redo();
            }
        }
        ui.separator();
        let has_data = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| sv.data.is_some());
        let has_selection = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| !sv.selected.is_empty());
        let select_all_shortcut = if is_mac { "Cmd+A" } else { "Ctrl+A" };
        let delete_shortcut = if is_mac { "Delete" } else { "Del" };
        if ui
            .add_enabled(
                has_data,
                egui::Button::new(t.get("menu_select_all")).shortcut_text(select_all_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.select_all();
            }
        }
        if ui.button(t.get("save_edit_deselect_all")).clicked() {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.deselect_all();
            }
        }
        ui.separator();
        if ui
            .add_enabled(
                has_selection,
                egui::Button::new(t.get("save_edit_delete_selected"))
                    .shortcut_text(delete_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.delete_selected();
            }
        }
        if ui
            .add_enabled(
                has_selection,
                egui::Button::new(t.get("save_edit_duplicate_selected")),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.duplicate_selected();
            }
        }
    }

    fn menu_view(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
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
                if matches!(sv.data, Some(crate::save_parser::SaveData::Contraption(_))) {
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
                for &lang in crate::locale::Language::ALL {
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

    fn menu_help(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: &'static I18n) {
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
                        let (y, mo, d) = super::civil_from_days(days as i64);
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

#[cfg(target_arch = "wasm32")]
fn export_bytes_wasm(file_name: &str, bytes: Vec<u8>) -> Result<(), String> {
    let arr = js_sys::Array::new();
    let u8arr = js_sys::Uint8Array::from(bytes.as_slice());
    arr.push(&u8arr.buffer());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&arr).map_err(|e| format!("{:?}", e))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("{:?}", e))?;

    let window = web_sys::window().ok_or_else(|| "window 不可用".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "document 不可用".to_string())?;
    let body = document
        .body()
        .ok_or_else(|| "document.body 不可用".to_string())?;

    let anchor = document
        .create_element("a")
        .map_err(|e| format!("{:?}", e))?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| "无法创建下载链接".to_string())?;

    anchor
        .set_attribute("href", &url)
        .map_err(|e| format!("{:?}", e))?;
    anchor
        .set_attribute("download", file_name)
        .map_err(|e| format!("{:?}", e))?;
    anchor
        .set_attribute("style", "display:none")
        .map_err(|e| format!("{:?}", e))?;

    body.append_child(&anchor).map_err(|e| format!("{:?}", e))?;
    anchor.click();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}
