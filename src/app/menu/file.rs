//! File menu — open/save/export/import.

use eframe::egui;

use crate::diagnostics::error::AppError;

#[cfg(target_arch = "wasm32")]
use crate::diagnostics::error::AppResult;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::i18n::locale::I18n;

use super::super::EditorApp;
#[cfg(target_arch = "wasm32")]
use super::super::{WASM_OPEN_RESULT, WASM_OPEN_XML_SAVE};
use super::status_export_error_message;

impl EditorApp {
    pub(super) fn menu_file(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, t: &'static I18n) {
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
                                    t.fmt1("status_read_error", &AppError::from(e).localized(t));
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = _ctx.clone();
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
                                    t.fmt1("status_read_error", &AppError::from(e).localized(t));
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = _ctx.clone();
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
                                    t.fmt1("status_read_error", &AppError::from(e).localized(t));
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = _ctx.clone();
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
                                    t.fmt1("status_read_error", &AppError::from(e).localized(t));
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let repaint_ctx = _ctx.clone();
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
                            match sv.export_encrypted() {
                                Ok(Some(encrypted)) => {
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
                                                    status_export_error_message(t, e);
                                            }
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        status_export_error_message(t, e);
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(ref sv) = self.tabs[self.active_tab].save_view {
                            match sv.export_encrypted() {
                                Ok(Some(encrypted)) => {
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
                                                status_export_error_message(t, e);
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    self.tabs[self.active_tab].status =
                                        status_export_error_message(t, e);
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
                                            status_export_error_message(t, e);
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
                                        status_export_error_message(t, e);
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
                                        status_export_error_message(t, e);
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
                                        status_export_error_message(t, e);
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
                        match self.tabs[self.active_tab].export_yaml() {
                            Ok(Some(text)) => {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("YAML files", &["yaml"])
                                    .set_file_name(&yaml_name)
                                    .save_file()
                                {
                                    match std::fs::write(&path, text.as_bytes()) {
                                        Ok(()) => {
                                            self.tabs[self.active_tab].status =
                                                t.get("status_exported");
                                        }
                                        Err(e) => {
                                            self.tabs[self.active_tab].status =
                                                status_export_error_message(t, e);
                                        }
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    status_export_error_message(t, e);
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        match self.tabs[self.active_tab].export_yaml() {
                            Ok(Some(text)) => {
                                let file_name = self.tabs[self.active_tab]
                                    .file_name
                                    .as_deref()
                                    .map(|n| format!("{n}.yaml"))
                                    .unwrap_or_else(|| "level.yaml".to_string());
                                match export_bytes_wasm(&file_name, text.into_bytes()) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            status_export_error_message(t, e);
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    status_export_error_message(t, e);
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
                        match self.tabs[self.active_tab].export_toml() {
                            Ok(Some(text)) => {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("TOML files", &["toml"])
                                    .set_file_name(&toml_name)
                                    .save_file()
                                {
                                    match std::fs::write(&path, text.as_bytes()) {
                                        Ok(()) => {
                                            self.tabs[self.active_tab].status =
                                                t.get("status_exported");
                                        }
                                        Err(e) => {
                                            self.tabs[self.active_tab].status =
                                                status_export_error_message(t, e);
                                        }
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    status_export_error_message(t, e);
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        match self.tabs[self.active_tab].export_toml() {
                            Ok(Some(text)) => {
                                let file_name = self.tabs[self.active_tab]
                                    .file_name
                                    .as_deref()
                                    .map(|n| format!("{n}.toml"))
                                    .unwrap_or_else(|| "level.toml".to_string());
                                match export_bytes_wasm(&file_name, text.into_bytes()) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            status_export_error_message(t, e);
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.tabs[self.active_tab].status =
                                    status_export_error_message(t, e);
                            }
                        }
                    }
                }
            } // level exports
        });
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn export_bytes_wasm(file_name: &str, bytes: Vec<u8>) -> AppResult<()> {
    let js_error = |error| {
        AppError::browser_key1("error_browser_api_call_failed", format!("{:?}", error))
    };

    let arr = js_sys::Array::new();
    let u8arr = js_sys::Uint8Array::from(bytes.as_slice());
    arr.push(&u8arr.buffer());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&arr).map_err(js_error)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;

    let window = web_sys::window().ok_or_else(|| AppError::state_key("error_window_unavailable"))?;
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

    anchor
        .set_attribute("href", &url)
        .map_err(js_error)?;
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
