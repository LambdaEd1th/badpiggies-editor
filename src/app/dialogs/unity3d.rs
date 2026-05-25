use eframe::egui;

use crate::diagnostics::error::AppError;
#[cfg(target_arch = "wasm32")]
use crate::diagnostics::error::AppResult;
use crate::i18n::locale::I18n;
use crate::io::unity3d::{
    Unity3dTextAssetEntry, list_text_assets_from_bytes, read_text_asset_from_bytes,
    replace_text_asset_in_bundle_bytes,
};

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use super::super::EditorApp;
#[cfg(target_arch = "wasm32")]
use super::super::{WASM_OPEN_UNITY3D_EXPORT, WASM_OPEN_UNITY3D_IMPORT};

#[derive(Clone)]
struct SelectableUnity3dEntry {
    entry: Unity3dTextAssetEntry,
    selected: bool,
}

#[derive(Clone)]
struct Unity3dBundleState {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    bundle_name: String,
    bundle_label: String,
    bundle_bytes: Vec<u8>,
    #[cfg(not(target_arch = "wasm32"))]
    bundle_path: Option<PathBuf>,
}

pub(in crate::app) struct Unity3dExportDialogState {
    bundle: Unity3dBundleState,
    entries: Vec<SelectableUnity3dEntry>,
}

pub(in crate::app) struct Unity3dImportDialogState {
    bundle: Unity3dBundleState,
    entries: Vec<Unity3dTextAssetEntry>,
    selected_index: Option<usize>,
}

impl Unity3dExportDialogState {
    fn new(bundle: Unity3dBundleState, entries: Vec<Unity3dTextAssetEntry>) -> Self {
        Self {
            bundle,
            entries: entries
                .into_iter()
                .map(|entry| SelectableUnity3dEntry {
                    entry,
                    selected: false,
                })
                .collect(),
        }
    }
}

impl Unity3dImportDialogState {
    fn new(
        bundle: Unity3dBundleState,
        entries: Vec<Unity3dTextAssetEntry>,
        current_level_name: Option<&str>,
    ) -> Self {
        let selected_index = current_level_name.and_then(|current_level_name| {
            entries.iter().position(|entry| {
                entry.display_name.eq_ignore_ascii_case(current_level_name)
                    || entry.asset_path.eq_ignore_ascii_case(current_level_name)
            })
        });

        Self {
            bundle,
            entries,
            selected_index: selected_index.or(Some(0)),
        }
    }
}

impl EditorApp {
    pub(in crate::app) fn open_unity3d_export_dialog(
        &mut self,
        _ctx: &egui::Context,
        _t: &'static I18n,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Unity3D files", &["unity3d"])
                .pick_file()
            else {
                return;
            };

            match std::fs::read(&path) {
                Ok(bundle_bytes) => {
                    let bundle_name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "bundle.unity3d".to_string());
                    self.finish_open_unity3d_export_dialog(
                        Unity3dBundleState {
                            bundle_name,
                            bundle_label: path.display().to_string(),
                            bundle_bytes,
                            bundle_path: Some(path),
                        },
                        _t,
                    );
                }
                Err(error) => {
                    self.tabs[self.active_tab].status =
                        _t.fmt1("status_read_error", &AppError::from(error).localized(_t));
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let repaint_ctx = _ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(file) = rfd::AsyncFileDialog::new()
                    .add_filter("Unity3D files", &["unity3d"])
                    .pick_file()
                    .await
                {
                    let name = file.file_name();
                    let data = file.read().await;
                    WASM_OPEN_UNITY3D_EXPORT.with(|q| {
                        q.borrow_mut().replace((name, data));
                    });
                    repaint_ctx.request_repaint();
                }
            });
        }
    }

    pub(in crate::app) fn open_unity3d_import_dialog(
        &mut self,
        _ctx: &egui::Context,
        _t: &'static I18n,
    ) {
        if self.tabs[self.active_tab].level.is_none() {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Unity3D files", &["unity3d"])
                .pick_file()
            else {
                return;
            };

            match std::fs::read(&path) {
                Ok(bundle_bytes) => {
                    let bundle_name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "bundle.unity3d".to_string());
                    self.finish_open_unity3d_import_dialog(
                        Unity3dBundleState {
                            bundle_name,
                            bundle_label: path.display().to_string(),
                            bundle_bytes,
                            bundle_path: Some(path),
                        },
                        _t,
                    );
                }
                Err(error) => {
                    self.tabs[self.active_tab].status =
                        _t.fmt1("status_read_error", &AppError::from(error).localized(_t));
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let repaint_ctx = _ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(file) = rfd::AsyncFileDialog::new()
                    .add_filter("Unity3D files", &["unity3d"])
                    .pick_file()
                    .await
                {
                    let name = file.file_name();
                    let data = file.read().await;
                    WASM_OPEN_UNITY3D_IMPORT.with(|q| {
                        q.borrow_mut().replace((name, data));
                    });
                    repaint_ctx.request_repaint();
                }
            });
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(in crate::app) fn handle_pending_unity3d_file_dialogs(&mut self, t: &'static I18n) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some((bundle_name, bundle_bytes)) =
                WASM_OPEN_UNITY3D_EXPORT.with(|q| q.borrow_mut().take())
            {
                self.finish_open_unity3d_export_dialog(
                    Unity3dBundleState {
                        bundle_label: bundle_name.clone(),
                        bundle_name,
                        bundle_bytes,
                    },
                    t,
                );
            }
            if let Some((bundle_name, bundle_bytes)) =
                WASM_OPEN_UNITY3D_IMPORT.with(|q| q.borrow_mut().take())
            {
                self.finish_open_unity3d_import_dialog(
                    Unity3dBundleState {
                        bundle_label: bundle_name.clone(),
                        bundle_name,
                        bundle_bytes,
                    },
                    t,
                );
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = t;
        }
    }

    fn finish_open_unity3d_export_dialog(&mut self, bundle: Unity3dBundleState, t: &'static I18n) {
        match list_text_assets_from_bytes(&bundle.bundle_label, &bundle.bundle_bytes) {
            Ok(entries) if entries.is_empty() => {
                self.tabs[self.active_tab].status = t.get("status_unity3d_no_text_assets");
            }
            Ok(entries) => {
                self.unity3d_import_dialog = None;
                self.unity3d_export_dialog = Some(Unity3dExportDialogState::new(bundle, entries));
            }
            Err(error) => {
                self.tabs[self.active_tab].status =
                    t.fmt1("status_read_error", &error.localized(t));
            }
        }
    }

    /// Open the unity3d export dialog directly from in-memory bytes (e.g. drag-and-drop).
    pub(in crate::app) fn open_unity3d_export_with_bytes(
        &mut self,
        bundle_name: String,
        bundle_label: String,
        bundle_bytes: Vec<u8>,
        #[cfg(not(target_arch = "wasm32"))] bundle_path: Option<std::path::PathBuf>,
        t: &'static I18n,
    ) {
        self.finish_open_unity3d_export_dialog(
            Unity3dBundleState {
                bundle_name,
                bundle_label,
                bundle_bytes,
                #[cfg(not(target_arch = "wasm32"))]
                bundle_path,
            },
            t,
        );
    }

    fn finish_open_unity3d_import_dialog(&mut self, bundle: Unity3dBundleState, t: &'static I18n) {
        match list_text_assets_from_bytes(&bundle.bundle_label, &bundle.bundle_bytes) {
            Ok(entries) if entries.is_empty() => {
                self.tabs[self.active_tab].status = t.get("status_unity3d_no_text_assets");
            }
            Ok(entries) => {
                let current_level_name = self.tabs[self.active_tab].file_name.as_deref();
                self.unity3d_export_dialog = None;
                self.unity3d_import_dialog = Some(Unity3dImportDialogState::new(
                    bundle,
                    entries,
                    current_level_name,
                ));
            }
            Err(error) => {
                self.tabs[self.active_tab].status =
                    t.fmt1("status_read_error", &error.localized(t));
            }
        }
    }

    pub(in crate::app) fn render_unity3d_export_dialog(
        &mut self,
        ctx: &egui::Context,
        t: &'static I18n,
    ) {
        let Some(mut dialog) = self.unity3d_export_dialog.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_clicked = false;
        let mut open_selected = false;

        egui::Window::new(t.get("win_export_from_unity3d"))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(560.0, 420.0))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut keep_open)
            .show(ctx, |ui| {
                let selected_count = dialog.entries.iter().filter(|entry| entry.selected).count();

                ui.horizontal(|ui| {
                    ui.label(t.get("label_unity3d_file"));
                    ui.monospace(&dialog.bundle.bundle_label);
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(t.get("btn_select_all")).clicked() {
                        for entry in &mut dialog.entries {
                            entry.selected = true;
                        }
                    }
                    if ui.button(t.get("btn_clear_all")).clicked() {
                        for entry in &mut dialog.entries {
                            entry.selected = false;
                        }
                    }
                    ui.separator();
                    ui.label(format!("{selected_count}/{}", dialog.entries.len()));
                });
                ui.add_space(6.0);
                ui.label(t.get("label_unity3d_entries"));
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for selectable in &mut dialog.entries {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut selectable.selected, "");
                                ui.vertical(|ui| {
                                    ui.label(&selectable.entry.display_name);
                                    ui.small(&selectable.entry.asset_path);
                                });
                            });
                            ui.separator();
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            dialog.entries.iter().any(|entry| entry.selected),
                            egui::Button::new(t.get("btn_open_selected")),
                        )
                        .clicked()
                    {
                        open_selected = true;
                    }
                    if ui.button(t.get("btn_cancel")).clicked() {
                        close_clicked = true;
                    }
                });
            });

        if close_clicked {
            keep_open = false;
        }

        if open_selected {
            let selected_entries: Vec<_> = dialog
                .entries
                .iter()
                .filter(|entry| entry.selected)
                .map(|entry| entry.entry.clone())
                .collect();
            let mut levels = Vec::with_capacity(selected_entries.len());
            for entry in &selected_entries {
                match read_text_asset_from_bytes(
                    &dialog.bundle.bundle_label,
                    &dialog.bundle.bundle_bytes,
                    entry,
                ) {
                    Ok(data) => levels.push((
                        entry.display_name.clone(),
                        data,
                        Some(format!(
                            "{}::{}",
                            dialog.bundle.bundle_label, entry.asset_path
                        )),
                    )),
                    Err(error) => {
                        self.tabs[self.active_tab].status =
                            t.fmt1("status_read_error", &error.localized(t));
                        self.unity3d_export_dialog = Some(dialog);
                        return;
                    }
                }
            }
            for (name, data, source_path) in levels {
                self.load_level_into_tab(name, data, source_path);
            }
            keep_open = false;
        }

        if keep_open {
            self.unity3d_export_dialog = Some(dialog);
        }
    }

    pub(in crate::app) fn render_unity3d_import_dialog(
        &mut self,
        ctx: &egui::Context,
        t: &'static I18n,
    ) {
        let Some(mut dialog) = self.unity3d_import_dialog.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_clicked = false;
        let mut import_current_level = false;
        let current_level_name = self.tabs[self.active_tab]
            .file_name
            .clone()
            .unwrap_or_else(|| "level.bytes".to_string());

        egui::Window::new(t.get("win_import_to_unity3d"))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(560.0, 420.0))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t.get("label_unity3d_file"));
                    ui.monospace(&dialog.bundle.bundle_label);
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("label_current_level"));
                    ui.monospace(&current_level_name);
                });
                ui.add_space(6.0);
                ui.label(t.get("label_unity3d_entries"));
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for (index, entry) in dialog.entries.iter().enumerate() {
                            let selected = dialog.selected_index == Some(index);
                            if ui.selectable_label(selected, &entry.display_name).clicked() {
                                dialog.selected_index = Some(index);
                            }
                            ui.small(&entry.asset_path);
                            ui.separator();
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            dialog.selected_index.is_some()
                                && self.tabs[self.active_tab].level.is_some(),
                            egui::Button::new(t.get("btn_import_current_level")),
                        )
                        .clicked()
                    {
                        import_current_level = true;
                    }
                    if ui.button(t.get("btn_cancel")).clicked() {
                        close_clicked = true;
                    }
                });
            });

        if close_clicked {
            keep_open = false;
        }

        if import_current_level {
            let Some(selected_index) = dialog.selected_index else {
                self.unity3d_import_dialog = Some(dialog);
                return;
            };
            let Some(level_bytes) = self.tabs[self.active_tab].export_level() else {
                self.unity3d_import_dialog = Some(dialog);
                return;
            };

            match replace_text_asset_in_bundle_bytes(
                &dialog.bundle.bundle_bytes,
                &dialog.entries[selected_index],
                &level_bytes,
            ) {
                Ok(updated_bundle_bytes) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(path) = dialog.bundle.bundle_path.as_ref() {
                            match std::fs::write(path, &updated_bundle_bytes) {
                                Ok(()) => {
                                    self.tabs[self.active_tab].status =
                                        t.get("status_unity3d_imported");
                                    keep_open = false;
                                }
                                Err(error) => {
                                    self.tabs[self.active_tab].status = t.fmt1(
                                        "status_export_error",
                                        &AppError::from(error).localized(t),
                                    );
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        match export_bytes_wasm(&dialog.bundle.bundle_name, updated_bundle_bytes) {
                            Ok(()) => {
                                self.tabs[self.active_tab].status =
                                    t.get("status_unity3d_imported");
                                keep_open = false;
                            }
                            Err(error) => {
                                self.tabs[self.active_tab].status =
                                    t.fmt1("status_export_error", &error.localized(t));
                            }
                        }
                    }
                }
                Err(error) => {
                    self.tabs[self.active_tab].status =
                        t.fmt1("status_export_error", &error.localized(t));
                }
            }
        }

        if keep_open {
            self.unity3d_import_dialog = Some(dialog);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn export_bytes_wasm(file_name: &str, bytes: Vec<u8>) -> AppResult<()> {
    let js_error =
        |error| AppError::browser_key1("error_browser_api_call_failed", format!("{:?}", error));

    let arr = js_sys::Array::new();
    let u8arr = js_sys::Uint8Array::from(bytes.as_slice());
    arr.push(&u8arr.buffer());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&arr).map_err(js_error)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;

    let window =
        web_sys::window().ok_or_else(|| AppError::state_key("error_window_unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| AppError::state_key("error_document_unavailable"))?;
    let body = document
        .body()
        .ok_or_else(|| AppError::state_key("error_document_body_unavailable"))?;

    let anchor = document
        .create_element("a")
        .map_err(js_error)?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| AppError::browser_key("error_download_link_unavailable"))?;

    anchor.set_attribute("href", &url).map_err(js_error)?;
    anchor
        .set_attribute("download", file_name)
        .map_err(js_error)?;
    anchor
        .set_attribute("style", "display:none")
        .map_err(js_error)?;

    body.append_child(&anchor).map_err(js_error)?;
    anchor.click();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}
