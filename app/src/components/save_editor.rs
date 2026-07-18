use std::collections::BTreeSet;

use badpiggies_editor_core::io::save::parser::SaveData;
use badpiggies_editor_core::worker_protocol::{WorkerRequest, WorkerResponse};
use dioxus::prelude::*;

use super::text_editor::RtonTextEditor;
use crate::app_actions::files;
use crate::app_view::APP_ASSETS;
use crate::editor_state::{EditorState, SaveViewMode};
use crate::platform::processing;

const CONTRAPTION_PREVIEW_RUNTIME: &str = include_str!("../../assets/contraption_preview.js");

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContraptionPreviewMessage {
    Ready,
    Error { message: String },
}

#[component]
pub fn SaveEditor() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let (name, xml, data, selected, view, parse_error, filter) = {
        let editor = state.read();
        let tab = editor.active();
        let save = tab.save.as_ref().expect("save tab has save document");
        (
            tab.file_name.clone(),
            save.xml.clone(),
            save.data.clone(),
            save.selected.clone(),
            save.view,
            save.parse_error.clone(),
            save.filter.clone(),
        )
    };
    let row_count = data.as_ref().map_or(0, save_len);
    let t = state.read().t();
    let entry_count = t.format("save_entry_count", &[("count", row_count.to_string())]);
    rsx! {
        section { class: "save-editor",
            header { class: "save-editor-heading",
                div {
                    strong { "{name}" }
                    span { "{entry_count}" }
                }
                div { class: "save-heading-actions",
                    button { onclick: move |_| files::export_save(state), {t.get("save_export_encrypted")} }
                }
            }
            nav { class: "save-toolbar",
                div { class: "save-view-segments",
                    ViewButton { label: t.get("save_view_table"), mode: SaveViewMode::Table, active: view == SaveViewMode::Table }
                    ViewButton { label: t.get("save_viewer_raw_xml"), mode: SaveViewMode::Xml, active: view == SaveViewMode::Xml }
                    ViewButton { label: t.get("save_view_split"), mode: SaveViewMode::Split, active: view == SaveViewMode::Split }
                    if matches!(data, Some(SaveData::Contraption(_))) {
                        ViewButton { label: t.get("save_view_preview"), mode: SaveViewMode::Preview, active: view == SaveViewMode::Preview }
                    }
                }
                input {
                    class: "save-filter",
                    r#type: "search",
                    placeholder: t.get("save_filter_entries"),
                    value: filter.clone(),
                    oninput: move |event| if let Some(save) = state.write().active_mut().save.as_mut() { save.filter = event.value(); },
                }
                span { class: "save-toolbar-spacer" }
                button { disabled: data.is_none(), onclick: move |_| state.write().add_save_row(), {t.get("btn_add")} }
                button { disabled: selected.is_empty(), onclick: move |_| state.write().duplicate_selected_save(), {t.get("menu_duplicate")} }
                button { disabled: selected.is_empty(), class: "danger", onclick: move |_| state.write().delete_selected_save(), {t.get("menu_delete")} }
            }
            if let Some(error) = parse_error {
                div { class: "save-parse-error", {t.format("save_xml_parse_error", &[("error", error)])} }
            }
            match view {
                SaveViewMode::Xml => rsx! {
                    div { class: "save-xml-editor",
                        RtonTextEditor {
                            text: xml,
                            on_change: move |text| update_save_xml(state, text),
                            on_undo: move |_| state.write().undo(),
                            on_redo: move |_| state.write().redo(),
                        }
                    }
                },
                SaveViewMode::Preview => rsx! {
                    if let Some(SaveData::Contraption(parts)) = data.clone() {
                        ContraptionPreview { parts }
                    } else {
                        div { class: "save-empty", {t.get("save_preview_contraption_only")} }
                    }
                },
                SaveViewMode::Split => rsx! {
                    div { class: "save-split",
                        div { class: "save-xml-editor",
                            RtonTextEditor {
                                text: xml,
                                on_change: move |text| update_save_xml(state, text),
                                on_undo: move |_| state.write().undo(),
                                on_redo: move |_| state.write().redo(),
                            }
                        }
                        if let Some(data) = data.clone() {
                            SaveTable { data, selected: selected.clone(), filter: filter.clone() }
                        } else {
                            div { class: "save-empty", {t.get("save_fix_xml")} }
                        }
                    }
                },
                SaveViewMode::Table => rsx! {
                    if let Some(data) = data.clone() {
                        SaveTable { data, selected: selected.clone(), filter: filter.clone() }
                    } else {
                        div { class: "save-empty", {t.get("save_fix_xml")} }
                    }
                },
            }
        }
    }
}

#[component]
fn ViewButton(label: String, mode: SaveViewMode, active: bool) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    rsx! {
        button {
            class: if active { "active" } else { "" },
            onclick: move |_| if let Some(save) = state.write().active_mut().save.as_mut() {
                save.view = mode;
            },
            "{label}"
        }
    }
}

#[component]
fn SaveTable(data: SaveData, selected: BTreeSet<usize>, filter: String) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    rsx! {
        div { class: "save-table-wrap",
            table { class: "save-table",
                match data {
                    SaveData::Progress(entries) => rsx! {
                        thead { tr { th {
                            input { r#type: "checkbox", checked: !entries.is_empty() && selected.len() == entries.len(), onchange: move |event| if event.checked() { state.write().select_all_save(); } else { state.write().clear_selection(); } }
                        } th { {t.get("save_col_type")} } th { {t.get("save_col_key")} } th { {t.get("save_col_value")} } } }
                        tbody {
                            for (index, entry) in entries.clone().into_iter().enumerate() {
                                if entry.value_type.to_ascii_lowercase().contains(&filter.to_ascii_lowercase()) || entry.key.to_ascii_lowercase().contains(&filter.to_ascii_lowercase()) || entry.value.to_ascii_lowercase().contains(&filter.to_ascii_lowercase()) {
                                  tr { class: if selected.contains(&index) { "selected" } else { "" },
                                    td { input { r#type: "checkbox", checked: selected.contains(&index), onchange: move |_| state.write().set_save_selection(index, true) } }
                                    td { input { value: entry.value_type, oninput: move |event| update_progress(state, index, 0, event.value()) } }
                                    td { input { value: entry.key, oninput: move |event| update_progress(state, index, 1, event.value()) } }
                                    td { input { value: entry.value, oninput: move |event| update_progress(state, index, 2, event.value()) } }
                                  }
                                }
                            }
                        }
                    },
                    SaveData::Achievements(entries) => rsx! {
                        thead { tr { th {
                            input { r#type: "checkbox", checked: !entries.is_empty() && selected.len() == entries.len(), onchange: move |event| if event.checked() { state.write().select_all_save(); } else { state.write().clear_selection(); } }
                        } th { "ID" } th { {t.get("save_col_progress")} } th { {t.get("save_col_completed")} } th { {t.get("save_col_synced")} } } }
                        tbody {
                            for (index, entry) in entries.clone().into_iter().enumerate() {
                                if entry.id.to_ascii_lowercase().contains(&filter.to_ascii_lowercase()) {
                                  tr { class: if selected.contains(&index) { "selected" } else { "" },
                                    td { input { r#type: "checkbox", checked: selected.contains(&index), onchange: move |_| state.write().set_save_selection(index, true) } }
                                    td { input { value: entry.id, oninput: move |event| update_achievement_text(state, index, event.value()) } }
                                    td { input { r#type: "number", step: "0.01", value: "{entry.progress}", oninput: move |event| if let Ok(value) = event.value().parse() { update_achievement_progress(state, index, value); } } }
                                    td { input { r#type: "checkbox", checked: entry.completed, onchange: move |event| update_achievement_flag(state, index, true, event.checked()) } }
                                    td { input { r#type: "checkbox", checked: entry.synced, onchange: move |event| update_achievement_flag(state, index, false, event.checked()) } }
                                  }
                                }
                            }
                        }
                    },
                    SaveData::Contraption(parts) => rsx! {
                        thead { tr { th {
                            input { r#type: "checkbox", checked: !parts.is_empty() && selected.len() == parts.len(), onchange: move |event| if event.checked() { state.write().select_all_save(); } else { state.write().clear_selection(); } }
                        } th { "X" } th { "Y" } th { {t.get("save_col_part_type")} } th { {t.get("save_col_custom_idx")} } th { {t.get("save_col_rot")} } th { {t.get("save_col_flipped")} } } }
                        tbody {
                            for (index, part) in parts.clone().into_iter().enumerate() {
                                if filter.is_empty() || part.part_type.to_string().contains(&filter) || part.custom_part_index.to_string().contains(&filter) {
                                  tr { class: if selected.contains(&index) { "selected" } else { "" },
                                    td { input { r#type: "checkbox", checked: selected.contains(&index), onchange: move |_| state.write().set_save_selection(index, true) } }
                                    for (field, value) in [part.x, part.y, part.part_type, part.custom_part_index, part.rot].into_iter().enumerate() {
                                        td { input { r#type: "number", value: "{value}", oninput: move |event| if let Ok(value) = event.value().parse() { update_contraption_number(state, index, field, value); } } }
                                    }
                                    td { input { r#type: "checkbox", checked: part.flipped, onchange: move |event| update_contraption_flipped(state, index, event.checked()) } }
                                  }
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn ContraptionPreview(
    parts: Vec<badpiggies_editor_core::io::save::parser::ContraptionPart>,
) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    if parts.is_empty() {
        return rsx! { div { class: "save-empty", {t.get("save_contraption_empty")} } };
    }
    let preview_parts = parts.clone();
    let start_preview = move |_| {
        let asset_root = serde_json::to_string(&APP_ASSETS.to_string())
            .unwrap_or_else(|_| "\"/assets\"".to_string());
        let runtime = CONTRAPTION_PREVIEW_RUNTIME.replace("__BP_ASSET_ROOT__", &asset_root);
        let mut evaluator = document::eval(&runtime);
        spawn(async move {
            while let Ok(message) = evaluator.recv::<ContraptionPreviewMessage>().await {
                match message {
                    ContraptionPreviewMessage::Ready => {
                        log::info!("Contraption preview renderer is ready");
                    }
                    ContraptionPreviewMessage::Error { message } => {
                        log::error!("{message}");
                        state.write().active_mut().status = message;
                    }
                }
            }
        });
    };
    use_effect(move || {
        let theme = state.read().theme.code();
        let parts_json = serde_json::to_string(&preview_parts).unwrap_or_else(|_| "[]".to_string());
        let theme_json = serde_json::to_string(theme).unwrap_or_else(|_| "\"system\"".to_string());
        spawn(async move {
            let script = format!(
                "window.bpContraptionPreview && window.bpContraptionPreview.render({{parts:{parts_json},theme:{theme_json}}});"
            );
            let _ = document::eval(&script).await;
        });
    });
    rsx! {
        div { class: "contraption-preview-scroll",
            canvas {
                id: "contraption-preview-canvas",
                aria_label: t.get("contraption_preview_title"),
                onmounted: start_preview,
            }
        }
    }
}

fn save_len(data: &SaveData) -> usize {
    match data {
        SaveData::Progress(entries) => entries.len(),
        SaveData::Contraption(parts) => parts.len(),
        SaveData::Achievements(entries) => entries.len(),
    }
}

fn update_save_xml(mut state: Signal<EditorState>, xml: String) {
    let Some((tab_index, file_type)) = state.write().begin_save_xml_update(xml.clone()) else {
        return;
    };
    spawn(async move {
        let request = WorkerRequest::ParseSave {
            file_type,
            bytes: xml.as_bytes().to_vec(),
        };
        let parsed = match processing::perform(request).await {
            Ok(WorkerResponse::Save { data }) => Ok(data),
            Ok(_) => Err(state.read().t().get("status_unexpected_worker_response")),
            Err(error) => Err(error),
        };
        state
            .write()
            .finish_save_xml_update(tab_index, &xml, parsed);
    });
}

fn update_progress(mut state: Signal<EditorState>, index: usize, field: usize, value: String) {
    state.write().mutate_save(|data| {
        if let SaveData::Progress(entries) = data
            && let Some(entry) = entries.get_mut(index)
        {
            match field {
                0 => entry.value_type = value,
                1 => entry.key = value,
                _ => entry.value = value,
            }
        }
    });
}

fn update_achievement_text(mut state: Signal<EditorState>, index: usize, value: String) {
    state.write().mutate_save(|data| {
        if let SaveData::Achievements(entries) = data
            && let Some(entry) = entries.get_mut(index)
        {
            entry.id = value;
        }
    });
}

fn update_achievement_progress(mut state: Signal<EditorState>, index: usize, value: f64) {
    state.write().mutate_save(|data| {
        if let SaveData::Achievements(entries) = data
            && let Some(entry) = entries.get_mut(index)
        {
            entry.progress = value;
        }
    });
}

fn update_achievement_flag(
    mut state: Signal<EditorState>,
    index: usize,
    completed: bool,
    value: bool,
) {
    state.write().mutate_save(|data| {
        if let SaveData::Achievements(entries) = data
            && let Some(entry) = entries.get_mut(index)
        {
            if completed {
                entry.completed = value;
            } else {
                entry.synced = value;
            }
        }
    });
}

fn update_contraption_number(
    mut state: Signal<EditorState>,
    index: usize,
    field: usize,
    value: i32,
) {
    state.write().mutate_save(|data| {
        if let SaveData::Contraption(parts) = data
            && let Some(part) = parts.get_mut(index)
        {
            match field {
                0 => part.x = value,
                1 => part.y = value,
                2 => part.part_type = value,
                3 => part.custom_part_index = value,
                _ => part.rot = value,
            }
        }
    });
}

fn update_contraption_flipped(mut state: Signal<EditorState>, index: usize, value: bool) {
    state.write().mutate_save(|data| {
        if let SaveData::Contraption(parts) = data
            && let Some(part) = parts.get_mut(index)
        {
            part.flipped = value;
        }
    });
}
