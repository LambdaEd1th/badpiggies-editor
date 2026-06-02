//! Editor state primitives: per-tab state, undo stack, clipboard.

use std::collections::BTreeSet;

use crate::diagnostics::error::AppError;
use crate::domain::level_warning::LevelWarning;
use crate::domain::parser;
use crate::domain::types::*;
use crate::i18n::locale::I18n;
use crate::renderer::{LevelRenderer, PreviewPlaybackState};

use super::save_viewer;
use super::text_codec::{
    parse_level_text, serialize_level_toml, serialize_level_yaml, status_parse_error_message,
};

/// Maximum number of undo snapshots to keep.
pub(super) const UNDO_MAX: usize = 100;

/// A snapshot of the editor state for undo/redo.
pub(super) struct Snapshot {
    pub(super) level: LevelData,
    pub(super) selected: BTreeSet<ObjectIndex>,
}

/// Undo/redo history stack.
pub(super) struct UndoStack {
    pub(super) undo: Vec<Snapshot>,
    pub(super) redo: Vec<Snapshot>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingLevelWarningAction {
    AcknowledgeOnly,
    PreviewPlaybackState(PreviewPlaybackState),
    ExportLevel,
}

#[derive(Clone)]
pub(super) struct PendingLevelWarning {
    pub(super) warnings: Vec<LevelWarning>,
    pub(super) action: PendingLevelWarningAction,
}

/// Clipboard contents for copy/cut/paste.
#[derive(Clone)]
pub(super) struct Clipboard {
    /// Each entry is a cloned subtree (objects[0] is root of that subtree).
    pub(super) subtrees: Vec<Vec<LevelObject>>,
}

/// Per-tab editor state (each tab is an independent level editor).
pub(super) struct Tab {
    /// Currently loaded level data.
    pub(super) level: Option<LevelData>,
    /// File name of the loaded level.
    pub(super) file_name: Option<String>,
    /// Native path of the loaded source file, if available.
    pub(super) source_path: Option<String>,
    /// Currently selected object indices.
    pub(super) selected: BTreeSet<ObjectIndex>,
    /// Canvas renderer state (owns per-level caches; shares GPU pipelines via Arc).
    pub(super) renderer: LevelRenderer,
    /// Transient status override. Empty means the status bar derives a localized default.
    pub(super) status: String,
    /// Pending delete confirmation: (object_indices, display_label).
    pub(super) pending_delete: Option<(Vec<ObjectIndex>, String)>,
    /// Pending level-risk warning confirmation before preview/export.
    pub(super) pending_level_warning: Option<PendingLevelWarning>,
    /// Undo/redo history.
    pub(super) history: UndoStack,
    /// Whether properties were changed in the previous frame (for undo coalescing).
    pub(super) props_changed_prev: bool,
    /// Anchor for Shift+click range selection in the object tree.
    pub(super) select_anchor: Option<ObjectIndex>,
    /// Save file viewer data (if this tab is viewing a save file).
    pub(super) save_view: Option<save_viewer::SaveViewerData>,
}

impl Tab {
    pub(super) fn new(renderer: LevelRenderer) -> Self {
        Self {
            level: None,
            file_name: None,
            source_path: None,
            selected: BTreeSet::new(),
            renderer,
            status: String::new(),
            pending_delete: None,
            pending_level_warning: None,
            history: UndoStack {
                undo: Vec::new(),
                redo: Vec::new(),
            },
            props_changed_prev: false,
            select_anchor: None,
            save_view: None,
        }
    }

    pub(super) fn status_text(&self, i18n: &I18n) -> String {
        if !self.status.is_empty() {
            return self.status.clone();
        }

        if let Some(save_view) = self.save_view.as_ref() {
            return save_view.status_bar_text(self.file_name.as_deref(), i18n);
        }

        if let Some(level) = self.level.as_ref() {
            return i18n.fmt_status_loaded(level.objects.len(), level.roots.len());
        }

        i18n.get("status_welcome")
    }

    /// Display name for the tab.
    pub(super) fn title(&self, fallback: &str) -> String {
        self.file_name
            .clone()
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Whether this tab is a save file viewer (not a level editor).
    pub(super) fn is_save_tab(&self) -> bool {
        self.save_view.is_some()
    }

    pub(super) fn status_bar_file_label(&self) -> Option<String> {
        let file_name = self.file_name.as_ref()?;

        if let Some(level_name) = self
            .save_view
            .as_ref()
            .and_then(|save_view| save_view.level_name.as_deref())
        {
            return Some(format!("{file_name} -> {level_name}"));
        }

        if let Some(level_name) = crate::data::level_db::level_display_name_for_filename(file_name)
        {
            return Some(format!("{file_name} -> {level_name}"));
        }

        Some(file_name.clone())
    }

    pub(super) fn load_level(
        &mut self,
        name: String,
        data: Vec<u8>,
        i18n: &I18n,
        source_path: Option<String>,
    ) {
        match parser::parse_level(data) {
            Ok(level) => {
                self.renderer.set_level_key(&name);
                self.renderer.set_level(&level);
                self.level = Some(level);
                self.file_name = Some(name);
                self.source_path = source_path;
                self.selected.clear();
                self.history.undo.clear();
                self.history.redo.clear();
                self.status.clear();
            }
            Err(e) => {
                self.status = status_parse_error_message(i18n, AppError::from(e));
            }
        }
    }

    pub(super) fn load_level_text(
        &mut self,
        name: String,
        text: &str,
        i18n: &I18n,
        source_path: Option<String>,
    ) {
        match parse_level_text(&name, text) {
            Ok(level) => {
                self.renderer.set_level_key(&name);
                self.renderer.set_level(&level);
                self.level = Some(level);
                self.file_name = Some(name);
                self.source_path = source_path;
                self.selected.clear();
                self.history.undo.clear();
                self.history.redo.clear();
                self.status.clear();
            }
            Err(e) => {
                self.status = status_parse_error_message(i18n, e);
            }
        }
    }

    pub(super) fn export_level(&self) -> Option<Vec<u8>> {
        self.level.as_ref().map(parser::serialize_level)
    }

    pub(super) fn export_yaml(&self) -> crate::diagnostics::error::AppResult<Option<String>> {
        self.level.as_ref().map(serialize_level_yaml).transpose()
    }

    pub(super) fn export_toml(&self) -> crate::diagnostics::error::AppResult<Option<String>> {
        self.level.as_ref().map(serialize_level_toml).transpose()
    }

    /// Determine the target parent for a paste operation.
    /// Uses the first selected object to find the parent.
    pub(super) fn paste_target_parent(
        level: &LevelData,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ObjectIndex> {
        let &sel = selected.iter().next()?;
        if sel >= level.objects.len() {
            return None;
        }
        match &level.objects[sel] {
            LevelObject::Parent(_) => Some(sel),
            LevelObject::Prefab(p) => p.parent,
        }
    }
}
