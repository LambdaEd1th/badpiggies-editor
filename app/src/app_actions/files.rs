use std::collections::BTreeSet;

use badpiggies_editor_core::io::crypto::SaveFileType;
use badpiggies_editor_core::io::save::parser::detect_type_from_xml;
use badpiggies_editor_core::worker_protocol::{LevelFormat, WorkerRequest, WorkerResponse};
use dioxus::prelude::*;

use crate::editor_state::{
    EditorState, Modal, UnityAssetSource, UnityBundleDocument, UnityBundleMode,
};
use crate::platform::processing;

pub async fn import_level(mut state: Signal<EditorState>, name: String, bytes: Vec<u8>) {
    state.write().active_mut().status = localized_with(state, "status_loading", "name", &name);
    let format = level_format(&name);
    let request = WorkerRequest::ParseLevel { bytes, format };
    match processing::perform(request).await {
        Ok(WorkerResponse::Level { level }) => state.write().load_level(name, level),
        Ok(_) => {
            state.write().active_mut().status =
                localized(state, "status_unexpected_worker_response")
        }
        Err(error) => state.write().active_mut().status = error,
    }
}

pub async fn import_save(mut state: Signal<EditorState>, name: String, bytes: Vec<u8>) {
    state.write().active_mut().status = localized_with(state, "status_loading", "name", &name);
    let Some(file_type) = SaveFileType::detect(&name) else {
        state.write().active_mut().status = localized(state, "status_unsupported_save_name");
        return;
    };
    let request = WorkerRequest::DecryptSave { file_type, bytes };
    match processing::perform(request).await {
        Ok(WorkerResponse::Bytes { bytes }) => match String::from_utf8(bytes) {
            Ok(xml) => load_parsed_save(state, name, xml, file_type).await,
            Err(error) => state.write().active_mut().status = error.to_string(),
        },
        Ok(_) => {
            state.write().active_mut().status =
                localized(state, "status_unexpected_worker_response")
        }
        Err(error) => state.write().active_mut().status = error,
    }
}

pub async fn import_save_xml(mut state: Signal<EditorState>, name: String, bytes: Vec<u8>) {
    let xml = match String::from_utf8(bytes) {
        Ok(xml) => xml,
        Err(error) => {
            state.write().active_mut().status = error.to_string();
            return;
        }
    };
    let Some(file_type) = SaveFileType::detect(&name).or_else(|| detect_type_from_xml(&xml)) else {
        state.write().active_mut().status = localized(state, "status_unsupported_save_xml");
        return;
    };
    load_parsed_save(state, name, xml, file_type).await;
}

pub async fn import_auto(state: Signal<EditorState>, name: String, bytes: Vec<u8>) {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".unity3d") {
        open_unity_bundle(state, name, bytes, UnityBundleMode::ExtractLevels).await;
    } else if lower.ends_with(".assets") {
        open_unity_assets_file(state, name, bytes, UnityBundleMode::ExtractLevels).await;
    } else if lower.ends_with(".bytes")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".toml")
    {
        import_level(state, name, bytes).await;
    } else if bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'<')
    {
        import_save_xml(state, name, bytes).await;
    } else {
        import_save(state, name, bytes).await;
    }
}

pub async fn open_unity_bundle(
    mut state: Signal<EditorState>,
    name: String,
    bytes: Vec<u8>,
    mode: UnityBundleMode,
) {
    state.write().active_mut().status = localized_with(state, "status_reading", "name", &name);
    let request = WorkerRequest::ListUnityTextAssets {
        bundle_name: name.clone(),
        bytes: bytes.clone(),
    };
    match processing::perform(request).await {
        Ok(WorkerResponse::UnityEntries { entries }) if entries.is_empty() => {
            state.write().active_mut().status = localized(state, "status_no_level_text_assets");
        }
        Ok(WorkerResponse::UnityEntries { entries }) => {
            let selected = if mode == UnityBundleMode::ReplaceLevel {
                BTreeSet::from([0])
            } else {
                BTreeSet::new()
            };
            let mut editor = state.write();
            editor.unity_bundle = Some(UnityBundleDocument {
                name,
                bytes,
                entries,
                selected,
                mode,
                source: UnityAssetSource::Bundle,
            });
            editor.modal = Some(Modal::Unity3d);
            editor.menu_open = None;
        }
        Ok(_) => {
            state.write().active_mut().status =
                localized(state, "status_unexpected_worker_response")
        }
        Err(error) => state.write().active_mut().status = error,
    }
}

pub async fn open_unity_assets_file(
    mut state: Signal<EditorState>,
    name: String,
    bytes: Vec<u8>,
    mode: UnityBundleMode,
) {
    state.write().active_mut().status = localized_with(state, "status_reading", "name", &name);
    let request = WorkerRequest::ListUnitySerializedFileLevels {
        asset_name: name.clone(),
        bytes: bytes.clone(),
    };
    match processing::perform(request).await {
        Ok(WorkerResponse::UnityEntries { entries }) if entries.is_empty() => {
            state.write().active_mut().status =
                localized(state, "status_no_level_text_assets_in_file");
        }
        Ok(WorkerResponse::UnityEntries { entries }) => {
            let selected = if mode == UnityBundleMode::ReplaceLevel {
                BTreeSet::from([0])
            } else {
                BTreeSet::new()
            };
            let mut editor = state.write();
            editor.unity_bundle = Some(UnityBundleDocument {
                name,
                bytes,
                entries,
                selected,
                mode,
                source: UnityAssetSource::SerializedFile,
            });
            editor.modal = Some(Modal::Unity3d);
            editor.menu_open = None;
        }
        Ok(_) => {
            state.write().active_mut().status =
                localized(state, "status_unexpected_worker_response")
        }
        Err(error) => state.write().active_mut().status = error,
    }
}

pub fn extract_unity_levels(mut state: Signal<EditorState>) {
    spawn(async move {
        let (source, bundle_name, bundle_bytes, entries) = {
            let editor = state.read();
            let Some(bundle) = editor.unity_bundle.as_ref() else {
                return;
            };
            let entries = bundle
                .selected
                .iter()
                .filter_map(|index| bundle.entries.get(*index).cloned())
                .collect::<Vec<_>>();
            (
                bundle.source,
                bundle.name.clone(),
                bundle.bytes.clone(),
                entries,
            )
        };
        if entries.is_empty() {
            return;
        }
        state.write().modal = None;
        match source {
            UnityAssetSource::Bundle => {
                for entry in entries {
                    let request = WorkerRequest::ReadUnityTextAsset {
                        bundle_name: bundle_name.clone(),
                        bytes: bundle_bytes.clone(),
                        entry: entry.clone(),
                    };
                    match processing::perform(request).await {
                        Ok(WorkerResponse::Bytes { bytes }) => {
                            import_level(state, level_file_name(&entry.display_name), bytes).await;
                        }
                        Ok(_) => {
                            state.write().active_mut().status =
                                localized(state, "status_unexpected_worker_response");
                        }
                        Err(error) => state.write().active_mut().status = error,
                    }
                }
            }
            UnityAssetSource::SerializedFile => {
                let request = WorkerRequest::ReadUnitySerializedFileLevels {
                    asset_name: bundle_name,
                    bytes: bundle_bytes,
                    entries,
                };
                match processing::perform(request).await {
                    Ok(WorkerResponse::ExtractedUnityTextAssets { assets }) => {
                        for asset in assets {
                            import_level(state, level_file_name(&asset.display_name), asset.bytes)
                                .await;
                        }
                    }
                    Ok(_) => {
                        state.write().active_mut().status =
                            localized(state, "status_unexpected_worker_response");
                    }
                    Err(error) => state.write().active_mut().status = error,
                }
            }
        }
        state.write().unity_bundle = None;
    });
}

pub fn replace_unity_level(mut state: Signal<EditorState>) {
    spawn(async move {
        let (level, source, asset_name, asset_bytes, entry) = {
            let editor = state.read();
            let Some(level) = editor.active().level.clone() else {
                return;
            };
            let Some(bundle) = editor.unity_bundle.as_ref() else {
                return;
            };
            let Some(entry) = bundle
                .selected
                .iter()
                .next()
                .and_then(|index| bundle.entries.get(*index))
                .cloned()
            else {
                return;
            };
            (
                level,
                bundle.source,
                bundle.name.clone(),
                bundle.bytes.clone(),
                entry,
            )
        };
        let request = WorkerRequest::SerializeLevel {
            level,
            format: LevelFormat::Bytes,
        };
        let level_bytes = match processing::perform(request).await {
            Ok(WorkerResponse::Bytes { bytes }) => bytes,
            Ok(_) => {
                state.write().active_mut().status =
                    localized(state, "status_unexpected_worker_response");
                return;
            }
            Err(error) => {
                state.write().active_mut().status = error;
                return;
            }
        };
        let request = match source {
            UnityAssetSource::Bundle => WorkerRequest::ReplaceUnityTextAsset {
                bytes: asset_bytes,
                entry,
                replacement: level_bytes,
            },
            UnityAssetSource::SerializedFile => {
                WorkerRequest::ReplaceUnitySerializedFileTextAsset {
                    bytes: asset_bytes,
                    entry,
                    replacement: level_bytes,
                }
            }
        };
        match processing::perform(request).await {
            Ok(WorkerResponse::Bytes { bytes }) => {
                state.write().modal = None;
                state.write().unity_bundle = None;
                let extension = match source {
                    UnityAssetSource::Bundle => "unity3d",
                    UnityAssetSource::SerializedFile => "assets",
                };
                save_bytes(state, asset_name, bytes, extension).await;
            }
            Ok(_) => {
                state.write().active_mut().status =
                    localized(state, "status_unexpected_worker_response");
            }
            Err(error) => state.write().active_mut().status = error,
        }
    });
}

pub fn export_level(mut state: Signal<EditorState>, format: LevelFormat) {
    spawn(async move {
        let (level, file_name) = {
            let state = state.read();
            let tab = state.active();
            let Some(level) = tab.level.clone() else {
                return;
            };
            (level, export_level_name(&tab.file_name, format))
        };
        let request = WorkerRequest::SerializeLevel { level, format };
        match processing::perform(request).await {
            Ok(WorkerResponse::Bytes { bytes }) => {
                save_bytes(state, file_name, bytes, format.extension()).await;
            }
            Ok(_) => {
                state.write().active_mut().status =
                    localized(state, "status_unexpected_worker_response")
            }
            Err(error) => state.write().active_mut().status = error,
        }
    });
}

pub fn export_save(mut state: Signal<EditorState>) {
    spawn(async move {
        let (file_name, file_type, xml) = {
            let state = state.read();
            let tab = state.active();
            let Some(save) = tab.save.as_ref() else {
                return;
            };
            (
                tab.file_name.clone(),
                save.file_type,
                save.xml.as_bytes().to_vec(),
            )
        };
        let request = WorkerRequest::EncryptSave {
            file_type,
            bytes: xml,
        };
        match processing::perform(request).await {
            Ok(WorkerResponse::Bytes { bytes }) => {
                save_bytes(state, file_name, bytes, "dat").await;
            }
            Ok(_) => {
                state.write().active_mut().status =
                    localized(state, "status_unexpected_worker_response")
            }
            Err(error) => state.write().active_mut().status = error,
        }
    });
}

pub fn export_save_xml(state: Signal<EditorState>) {
    spawn(async move {
        let (name, xml) = {
            let editor = state.read();
            let tab = editor.active();
            let Some(save) = tab.save.as_ref() else {
                return;
            };
            let stem = tab
                .file_name
                .strip_suffix(".dat")
                .or_else(|| tab.file_name.strip_suffix(".contraption"))
                .or_else(|| tab.file_name.strip_suffix(".xml"))
                .unwrap_or(&tab.file_name);
            (format!("{stem}.xml"), save.xml.as_bytes().to_vec())
        };
        save_bytes(state, name, xml, "xml").await;
    });
}

pub fn export_logs(state: Signal<EditorState>) {
    spawn(async move {
        let bytes = crate::platform::log_buffer::snapshot().into_bytes();
        save_bytes(state, "badpiggies-editor.log".to_string(), bytes, "log").await;
    });
}

pub(crate) async fn save_bytes(
    mut state: Signal<EditorState>,
    name: String,
    bytes: Vec<u8>,
    extension: &'static str,
) {
    let export_filter = localized(state, "filter_export");
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter(export_filter, &[extension])
        .set_file_name(&name)
        .save_file()
        .await
    else {
        return;
    };
    match file.write(&bytes).await {
        Ok(()) => {
            let mut state = state.write();
            state.active_mut().dirty = false;
            state.active_mut().status = state.t().get("status_exported");
        }
        Err(error) => state.write().active_mut().status = error.to_string(),
    }
}

fn level_format(name: &str) -> LevelFormat {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        LevelFormat::Yaml
    } else if lower.ends_with(".toml") {
        LevelFormat::Toml
    } else {
        LevelFormat::Bytes
    }
}

fn export_level_name(name: &str, format: LevelFormat) -> String {
    let stem = name
        .strip_suffix(".bytes")
        .or_else(|| name.strip_suffix(".yaml"))
        .or_else(|| name.strip_suffix(".yml"))
        .or_else(|| name.strip_suffix(".toml"))
        .unwrap_or(name);
    format!("{stem}.{}", format.extension())
}

fn level_file_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".bytes") {
        name.to_string()
    } else {
        format!("{name}.bytes")
    }
}

async fn load_parsed_save(
    mut state: Signal<EditorState>,
    name: String,
    xml: String,
    file_type: SaveFileType,
) {
    let request = WorkerRequest::ParseSave {
        file_type,
        bytes: xml.as_bytes().to_vec(),
    };
    match processing::perform(request).await {
        Ok(WorkerResponse::Save { data }) => {
            state
                .write()
                .load_save_parsed(name, xml, file_type, Ok(data));
        }
        Ok(_) => {
            state.write().active_mut().status =
                localized(state, "status_unexpected_worker_response")
        }
        Err(error) => state
            .write()
            .load_save_parsed(name, xml, file_type, Err(error)),
    }
}

fn localized(state: Signal<EditorState>, key: &str) -> String {
    state.read().t().get(key)
}

fn localized_with(state: Signal<EditorState>, key: &str, name: &str, value: &str) -> String {
    state.read().t().format(key, &[(name, value.to_string())])
}
