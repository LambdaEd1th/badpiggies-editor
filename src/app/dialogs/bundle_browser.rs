use crate::io::unity_bundle::UnityBundleReader;
use crate::io::unity_assets::SerializedFile;
use eframe::egui;

pub struct BundleBrowserDialog {
    bundle: UnityBundleReader,
    entries: Vec<Entry>,
    selected_index: Option<usize>,
    filter: String,
}

struct Entry {
    path: String,
    size: u64,
    source: EntrySource,
}

enum EntrySource {
    Bundle(String),
    Serialized { data: Vec<u8> },
}

pub enum BundleBrowserResult {
    Pending,
    Loaded(Vec<u8>),
    Cancelled,
}

impl BundleBrowserDialog {
    pub fn new(bundle: UnityBundleReader) -> Self {
        let mut entries = Vec::new();
        
        for node in bundle.list_files() {
            if node.path.ends_with(".bytes") {
                entries.push(Entry {
                    path: node.path.clone(),
                    size: node.size,
                    source: EntrySource::Bundle(node.path.clone()),
                });
            } else {
                // Scan for embedded assets
                if let Ok(data) = bundle.read_file(&node.path) {
                    let sf = SerializedFile::new(data);
                    for (asset_name, asset_data) in sf.extract_text_assets() {
                        entries.push(Entry {
                            path: format!("{} -> {}", node.path, asset_name),
                            size: asset_data.len() as u64,
                            source: EntrySource::Serialized { 
                                data: asset_data 
                            },
                        });
                    }
                }
            }
        }

        Self {
            bundle,
            entries,
            selected_index: None,
            filter: String::new(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> BundleBrowserResult {
        let mut result = BundleBrowserResult::Pending;
        let mut should_close = false;

        egui::Window::new("Asset Bundle Browser")
            .resizable(true)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.text_edit_singleline(&mut self.filter);
                });

                ui.separator();

                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for (idx, entry) in self.entries.iter().enumerate() {
                        if !self.filter.is_empty() && !entry.path.to_lowercase().contains(&self.filter.to_lowercase()) {
                            continue;
                        }

                        let label = format!("{} ({:.2} KB)", entry.path, entry.size as f32 / 1024.0);
                        if ui.selectable_label(self.selected_index == Some(idx), label).clicked() {
                            self.selected_index = Some(idx);
                        }
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Open Selected").enabled() && self.selected_index.is_some() {
                        if let Some(idx) = self.selected_index {
                            let entry = &self.entries[idx];
                            match &entry.source {
                                EntrySource::Bundle(path) => {
                                    match self.bundle.read_file(path) {
                                        Ok(data) => {
                                            result = BundleBrowserResult::Loaded(data);
                                            should_close = true;
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to read file from bundle: {}", e);
                                        }
                                    }
                                }
                                EntrySource::Serialized { data, .. } => {
                                    result = BundleBrowserResult::Loaded(data.clone());
                                    should_close = true;
                                }
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_close && matches!(result, BundleBrowserResult::Pending) {
            result = BundleBrowserResult::Cancelled;
        }

        result
    }
}
