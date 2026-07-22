use std::collections::BTreeSet;

use badpiggies_editor_core::data::goal_animation::{
    GoalAnimationState, parse_goal_animation_state, set_goal_animation_state,
};
use badpiggies_editor_core::domain::level_warning::LevelWarningSeverity;
use badpiggies_editor_core::domain::prefab_override::{
    OverrideNode, parse_override_text, serialize_override_tree,
};
use badpiggies_editor_core::domain::types::{
    DataType, LevelObject, ObjectIndex, PrefabInstance, PrefabOverrideData, TerrainData,
};
use badpiggies_editor_core::worker_protocol::LevelFormat;
use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdCheck, LdCheckCheck, LdChevronDown, LdChevronRight, LdEllipsis, LdFileUp, LdFolderOpen,
    LdGithub, LdInfo, LdKeyboard, LdLanguages, LdListTree, LdMenu, LdMonitor, LdMoon, LdPanelRight,
    LdPlus, LdRedo2, LdScan, LdScrollText, LdSettings, LdSlidersHorizontal, LdSquare, LdSun,
    LdTrash2, LdUndo2, LdX,
};
use dioxus_free_icons::{Icon, IconShape};
use dioxus_html::HasFileData;

use crate::app_actions::files;
use crate::app_view::canvas::EditorCanvas;
use crate::editor_state::{
    CanvasPointerState, EditorState, MobilePanel, Modal, ThemePreference, UnityAssetSource,
    UnityBundleMode,
};
use crate::i18n::locale::Language;
use crate::platform;

pub(crate) mod context_menu_transition;
mod save_editor;
mod text_editor;
mod tools;

use context_menu_transition::ContextMenuTransition;
use save_editor::SaveEditor;
use tools::{LevelWarningBadge, PreviewControls, ToolPanel};

const AUTHOR_NAME: &str = "LambdaEd1th";
const AUTHOR_URL: &str = "https://space.bilibili.com/8217621";
const GITHUB_URL: &str = "https://github.com/LambdaEd1th/badpiggies-editor";
const WORKSPACE_RESIZER_RUNTIME: &str = include_str!("../../assets/workspace_resizer.js");

#[derive(Clone, Copy, PartialEq, Eq)]
struct ShortcutSpec {
    label_key: &'static str,
    keys: &'static [&'static str],
}

const EDITING_SHORTCUTS: &[ShortcutSpec] = &[
    ShortcutSpec {
        label_key: "shortcuts_undo_action",
        keys: &["⌘", "Z"],
    },
    ShortcutSpec {
        label_key: "shortcuts_redo_action",
        keys: &["⌘", "⇧", "Z"],
    },
    ShortcutSpec {
        label_key: "menu_select_all",
        keys: &["⌘", "A"],
    },
    ShortcutSpec {
        label_key: "shortcuts_copy_action",
        keys: &["⌘", "C"],
    },
    ShortcutSpec {
        label_key: "shortcuts_cut_action",
        keys: &["⌘", "X"],
    },
    ShortcutSpec {
        label_key: "shortcuts_paste_action",
        keys: &["⌘", "V"],
    },
    ShortcutSpec {
        label_key: "shortcuts_duplicate_action",
        keys: &["⌘", "D"],
    },
    ShortcutSpec {
        label_key: "shortcuts_delete_action",
        keys: &["Delete"],
    },
    ShortcutSpec {
        label_key: "save_edit_deselect_all",
        keys: &["Esc"],
    },
];

const TOOL_SHORTCUTS: &[ShortcutSpec] = &[
    ShortcutSpec {
        label_key: "tool_select",
        keys: &["V"],
    },
    ShortcutSpec {
        label_key: "tool_box_select",
        keys: &["M"],
    },
    ShortcutSpec {
        label_key: "tool_draw_terrain",
        keys: &["P"],
    },
    ShortcutSpec {
        label_key: "tool_pan",
        keys: &["H"],
    },
];

const VIEW_SHORTCUTS: &[ShortcutSpec] = &[ShortcutSpec {
    label_key: "menu_fit_view",
    keys: &["F"],
}];

#[component]
pub fn EditorShell() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let mut dragging_files = use_signal(|| false);
    use_effect(|| {
        document::eval(WORKSPACE_RESIZER_RUNTIME);
    });
    let (show_tree, show_properties, mobile_panel, is_save, is_empty_workspace) = {
        let state = state.read();
        (
            state.show_tree,
            state.show_properties,
            state.mobile_panel,
            state.active().save.is_some(),
            state.active().is_workspace_placeholder(),
        )
    };
    let tree_open = show_tree && !is_save;
    let properties_open = show_properties && !is_save;
    let mobile_tree_open = mobile_panel == Some(MobilePanel::Objects) && !is_save;
    let mobile_properties_open = mobile_panel == Some(MobilePanel::Properties) && !is_save;
    let dragging_files_snapshot = *dragging_files.read();
    let workspace_class = format!(
        "rton-workspace-shell {} {} {} {} {} {} {}",
        if dragging_files_snapshot {
            "dragging-files"
        } else {
            ""
        },
        if tree_open {
            "file-drawer-mounted file-drawer-open"
        } else {
            "file-drawer-unmounted file-drawer-closed"
        },
        if properties_open {
            "inspector-drawer-mounted inspector-drawer-open"
        } else {
            "inspector-drawer-unmounted inspector-drawer-closed"
        },
        if mobile_tree_open {
            "mobile-file-drawer-open"
        } else {
            ""
        },
        if mobile_properties_open {
            "mobile-inspector-drawer-open"
        } else {
            ""
        },
        if is_save {
            "save-workspace"
        } else {
            "level-workspace"
        },
        if mobile_panel.is_some() && !is_save {
            "mobile-drawer-active"
        } else {
            ""
        },
    );

    let shell_class = state.read().theme.shell_class();
    let t = state.read().t();

    rsx! {
        div {
            class: shell_class,
            ondragover: move |event| {
                if workspace_drag_has_files(&event) {
                    event.prevent_default();
                    dragging_files.set(true);
                }
            },
            ondragleave: move |_| dragging_files.set(false),
            ondrop: move |event| {
                if !workspace_drag_has_files(&event) {
                    return;
                }
                event.prevent_default();
                dragging_files.set(false);
                for file in event.files() {
                    let name = file.name();
                    dioxus::dioxus_core::spawn_forever(async move {
                        match file.read_bytes().await {
                            Ok(bytes) => files::import_auto(state, name, bytes.to_vec()).await,
                            Err(error) => state.write().active_mut().status = error.to_string(),
                        }
                    });
                }
            },
            CommandBar {}
            div { class: workspace_class,
                if dragging_files_snapshot {
                    div {
                        class: "rton-workspace-drop-indicator",
                        aria_hidden: "true",
                        span { class: "rton-workspace-drop-indicator-icon", {icon(LdFileUp)} }
                        strong { {t.get("empty_drop_title")} }
                    }
                }
                if (mobile_tree_open || mobile_properties_open) && !is_save {
                    button {
                        class: "rton-file-drawer-backdrop",
                        aria_label: t.get("aria_close_panel"),
                        onclick: move |_| state.write().mobile_panel = None,
                    }
                }
                main { class: "rton-main-content",
                    if !is_save {
                        ObjectTree {}
                        div {
                            class: "rton-resize-handle rton-resize-handle-left",
                            role: "separator",
                            tabindex: "0",
                            aria_label: t.get("aria_resize_object_panel"),
                            aria_orientation: "vertical",
                        }
                    }
                    section { class: "rton-center-panel",
                        TabBar {}
                        section { class: "rton-editor-stage center-pane",
                            if is_save {
                                SaveEditor {}
                            } else {
                                EditorCanvas {}
                                if is_empty_workspace {
                                    EmptyDropStage {}
                                } else {
                                    ToolPanel {}
                                    PreviewControls {}
                                    LevelWarningBadge {}
                                }
                            }
                        }
                    }
                    if !is_save {
                        div {
                            class: "rton-resize-handle rton-resize-handle-right",
                            role: "separator",
                            tabindex: "0",
                            aria_label: t.get("aria_resize_properties_panel"),
                            aria_orientation: "vertical",
                        }
                        PropertiesPanel {}
                    }
                }
            }
            StatusBar {}
            ModalLayer {}
        }
    }
}

fn workspace_drag_has_files(event: &DragEvent) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(web_event) = event.data().downcast::<web_sys::DragEvent>()
            && let Some(data_transfer) = web_event.data_transfer()
        {
            if data_transfer
                .files()
                .is_some_and(|files| files.length() > 0)
            {
                return true;
            }
            let items = data_transfer.items();
            for index in 0..items.length() {
                if items.get(index).is_some_and(|item| item.kind() == "file") {
                    return true;
                }
            }
        }
    }

    !event.files().is_empty()
}

#[component]
fn EmptyDropStage() -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let (title, subtitle) = {
        let state = state.read();
        let t = state.t();
        (t.get("empty_drop_title"), t.get("empty_drop_subtitle"))
    };

    rsx! {
        div { class: "rton-empty-drop-stage",
            div { class: "empty-editor-icon", {icon(LdFolderOpen)} }
            div { class: "empty-editor-title", "{title}" }
            div { class: "empty-editor-subtitle", "{subtitle}" }
        }
    }
}

#[component]
fn CommandBar() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let menu_anchor = use_signal(|| None::<MenuAnchor>);
    let can_undo = state.read().can_undo();
    let can_redo = state.read().can_redo();
    let selected = state.read().has_selection();
    let is_save = state.read().active().save.is_some();
    let mobile_panel = state.read().mobile_panel;
    let show_tree = state.read().show_tree;
    let show_properties = state.read().show_properties;
    let open = state.read().menu_open;
    let active_file = {
        let state = state.read();
        if state.active().is_workspace_placeholder() {
            state.t().get("common_no_file")
        } else {
            state.active().title()
        }
    };
    let t = state.read().t();
    let shortcuts_title = t.get("win_shortcuts");
    rsx! {
        header { class: if open == Some("more") { "rton-toolbar rton-commandbar mobile-menu-open" } else { "rton-toolbar rton-commandbar" },
            button {
                class: if show_tree { "rton-command-icon-button rton-sidebar-toggle desktop-command-control active" } else { "rton-command-icon-button rton-sidebar-toggle desktop-command-control" },
                title: t.get("panel_object_list"),
                aria_label: t.get("panel_object_list"),
                disabled: is_save,
                onclick: move |_| state.write().show_tree = !show_tree,
                {icon(LdMenu)}
            }
            button {
                class: if mobile_panel == Some(MobilePanel::Objects) { "rton-command-icon-button rton-sidebar-toggle mobile-command-control active" } else { "rton-command-icon-button rton-sidebar-toggle mobile-command-control" },
                title: t.get("panel_object_list"),
                aria_label: t.get("panel_object_list"),
                disabled: is_save,
                onclick: move |_| toggle_mobile_panel(state, MobilePanel::Objects),
                {icon(LdMenu)}
            }
            div { class: "rton-commandbar-document", title: "{active_file}",
                span { class: "rton-commandbar-document-dot" }
                span { class: "rton-commandbar-document-name", "{active_file}" }
            }
            div { class: "rton-mobile-command-actions",
                button { class: "rton-command-icon-button", title: t.get("menu_undo"), disabled: !can_undo, onclick: move |_| state.write().undo(), {icon(LdUndo2)} }
                button { class: "rton-command-icon-button", title: t.get("menu_redo"), disabled: !can_redo, onclick: move |_| state.write().redo(), {icon(LdRedo2)} }
                button {
                    class: if open == Some("more") { "rton-command-icon-button active" } else { "rton-command-icon-button" },
                    title: t.get("common_more"),
                    aria_label: t.get("common_more"),
                    onclick: move |_| toggle_menu(state, "more"),
                    {icon(LdEllipsis)}
                }
            }
            div { class: "rton-commandbar-groups rton-commandbar-groups-inline",
                div { class: "rton-commandbar-group-scroll",
                    div { class: "rton-toolbar-group-shell",
                        div { class: "rton-toolbar-group",
                            MenuButton { id: "file", label: t.get("menu_file"), active: open == Some("file"), menu_anchor }
                            MenuButton { id: "edit", label: t.get("menu_edit"), active: open == Some("edit"), menu_anchor }
                            MenuButton { id: "view", label: t.get("menu_view"), active: open == Some("view"), menu_anchor }
                        }
                    }
                    div { class: "rton-toolbar-group-shell",
                        div { class: "rton-toolbar-group",
                            IconButton { label: t.get("menu_add_object"), disabled: is_save, icon: icon(LdPlus), onclick: move |_| state.write().modal = Some(Modal::AddObject) }
                            IconButton { label: t.get("menu_undo"), disabled: !can_undo, icon: icon(LdUndo2), onclick: move |_| state.write().undo() }
                            IconButton { label: t.get("menu_redo"), disabled: !can_redo, icon: icon(LdRedo2), onclick: move |_| state.write().redo() }
                            IconButton { label: t.get("menu_delete"), disabled: !selected, icon: icon(LdTrash2), onclick: move |_| state.write().request_delete_selected() }
                        }
                    }
                }
            }
            if let Some(menu) = open {
                if menu == "more" {
                    MobileCommandMenu { menu_anchor }
                } else {
                    MenuPopup { menu, anchor: *menu_anchor.read() }
                }
            }
            button {
                id: "shortcut-help-button",
                class: "rton-command-icon-button rton-shortcuts-button desktop-command-control",
                title: "{shortcuts_title}",
                aria_label: "{shortcuts_title}",
                onclick: move |_| {
                    let is_open = state.read().modal == Some(Modal::Shortcuts);
                    state.write().modal = if is_open { None } else { Some(Modal::Shortcuts) };
                },
                {icon(LdKeyboard)}
            }
            button {
                class: if show_properties { "rton-command-icon-button rton-inspector-toggle desktop-command-control active" } else { "rton-command-icon-button rton-inspector-toggle desktop-command-control" },
                title: t.get("panel_properties"),
                aria_label: t.get("panel_properties"),
                disabled: is_save,
                onclick: move |_| state.write().show_properties = !show_properties,
                {icon(LdPanelRight)}
            }
            button {
                class: if mobile_panel == Some(MobilePanel::Properties) { "rton-command-icon-button rton-inspector-toggle mobile-command-control active" } else { "rton-command-icon-button rton-inspector-toggle mobile-command-control" },
                title: t.get("panel_properties"),
                aria_label: t.get("panel_properties"),
                disabled: is_save,
                onclick: move |_| toggle_mobile_panel(state, MobilePanel::Properties),
                {icon(LdPanelRight)}
            }
            button {
                class: "rton-command-icon-button rton-settings-button",
                title: t.get("settings_title"),
                aria_label: t.get("settings_title"),
                onclick: move |_| state.write().modal = Some(Modal::Settings),
                {icon(LdSettings)}
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct MenuAnchor {
    menu: &'static str,
    x: f64,
    y: f64,
}

fn toggle_mobile_panel(mut state: Signal<EditorState>, panel: MobilePanel) {
    let is_open = state.read().mobile_panel == Some(panel);
    state.write().mobile_panel = if is_open { None } else { Some(panel) };
}

fn toggle_menu(mut state: Signal<EditorState>, menu: &'static str) {
    let is_open = state.read().menu_open == Some(menu);
    state.write().menu_open = if is_open { None } else { Some(menu) };
}

#[component]
fn IconButton(
    label: String,
    disabled: bool,
    #[props(default)] active: bool,
    icon: Element,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if active { "icon-button active" } else { "icon-button" },
            title: label.clone(),
            aria_label: label,
            disabled,
            onclick: move |event| onclick.call(event),
            {icon}
        }
    }
}

fn icon<T>(shape: T) -> Element
where
    T: IconShape + Clone + PartialEq + 'static,
{
    rsx! {
        Icon {
            class: "rton-lucide-icon",
            width: 16,
            height: 16,
            fill: "currentColor",
            icon: shape
        }
    }
}

fn theme_label_key(theme: ThemePreference) -> &'static str {
    match theme {
        ThemePreference::System => "theme_system",
        ThemePreference::Light => "theme_light",
        ThemePreference::Dark => "theme_dark",
    }
}

fn theme_icon(theme: ThemePreference) -> Element {
    match theme {
        ThemePreference::System => icon(LdMonitor),
        ThemePreference::Light => icon(LdSun),
        ThemePreference::Dark => icon(LdMoon),
    }
}

#[component]
fn MenuButton(
    id: &'static str,
    label: String,
    active: bool,
    mut menu_anchor: Signal<Option<MenuAnchor>>,
) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let mut mounted = use_signal(|| None::<MountedEvent>);
    rsx! {
        button {
            class: if active { "rton-button menu-button active" } else { "rton-button menu-button" },
            onmounted: move |event| mounted.set(Some(event)),
            onclick: move |_| {
                if state.read().menu_open == Some(id) {
                    state.write().menu_open = None;
                    return;
                }
                let Some(event) = mounted.peek().clone() else {
                    state.write().menu_open = Some(id);
                    return;
                };
                spawn(async move {
                    if let Ok(rect) = event.get_client_rect().await {
                        menu_anchor.set(Some(MenuAnchor {
                            menu: id,
                            x: rect.origin.x,
                            y: rect.origin.y + rect.height() + 8.0,
                        }));
                    }
                    state.write().menu_open = Some(id);
                });
            },
            "{label}"
        }
    }
}

#[component]
fn MobileCommandMenu(menu_anchor: Signal<Option<MenuAnchor>>) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let can_undo = state.read().can_undo();
    let can_redo = state.read().can_redo();
    let selected = state.read().has_selection();
    let is_save = state.read().active().save.is_some();
    let t = state.read().t();
    rsx! {
        div { class: "rton-commandbar-groups rton-commandbar-more-menu mobile-open",
            button {
                class: "rton-commandbar-menu-backdrop",
                aria_label: t.get("aria_close_menu"),
                onclick: move |_| state.write().menu_open = None,
            }
            div { class: "rton-commandbar-menu-header",
                div {
                    strong { {t.get("common_commands")} }
                    span { "Bad Piggies Editor" }
                }
                button {
                    class: "rton-command-icon-button",
                    title: t.get("common_close"),
                    aria_label: t.get("aria_close_menu"),
                    onclick: move |_| state.write().menu_open = None,
                    {icon(LdX)}
                }
            }
            div { class: "rton-commandbar-group-scroll",
                section { class: "mobile-command-section",
                    strong { class: "mobile-command-section-title", {t.get("common_menus")} }
                    div { class: "mobile-command-grid",
                        MenuButton { id: "file", label: t.get("menu_file"), active: false, menu_anchor }
                        MenuButton { id: "edit", label: t.get("menu_edit"), active: false, menu_anchor }
                        MenuButton { id: "view", label: t.get("menu_view"), active: false, menu_anchor }
                    }
                }
                section { class: "mobile-command-section",
                    strong { class: "mobile-command-section-title", {t.get("menu_edit")} }
                    div { class: "mobile-command-grid actions",
                        MobileAction { label: t.get("menu_undo"), disabled: !can_undo, icon: icon(LdUndo2), onclick: move |_| { state.write().undo(); state.write().menu_open = None; } }
                        MobileAction { label: t.get("menu_redo"), disabled: !can_redo, icon: icon(LdRedo2), onclick: move |_| { state.write().redo(); state.write().menu_open = None; } }
                        MobileAction { label: t.get("menu_add_object"), disabled: is_save, icon: icon(LdPlus), onclick: move |_| state.write().modal = Some(Modal::AddObject) }
                        MobileAction { label: t.get("menu_delete"), disabled: !selected, icon: icon(LdTrash2), onclick: move |_| { state.write().request_delete_selected(); state.write().menu_open = None; } }
                        MobileAction { label: t.get("menu_fit_view"), disabled: is_save, icon: icon(LdScan), onclick: move |_| { state.write().fit_view(); state.write().menu_open = None; } }
                    }
                }
            }
        }
    }
}

#[component]
fn MobileAction(
    label: String,
    disabled: bool,
    icon: Element,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: "mobile-command-action",
            disabled,
            onclick: move |event| onclick.call(event),
            span { class: "mobile-command-action-icon", {icon} }
            span { "{label}" }
        }
    }
}

#[component]
fn MenuPopup(menu: &'static str, anchor: Option<MenuAnchor>) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let has_level = {
        let editor = state.read();
        editor.active().is_level() && !editor.active().is_workspace_placeholder()
    };
    let has_save = state.read().active().save.is_some();
    let menu_title = match menu {
        "file" => t.get("menu_file"),
        "edit" => t.get("menu_edit"),
        "view" => t.get("menu_view"),
        _ => String::new(),
    };
    let position = anchor
        .filter(|anchor| anchor.menu == menu)
        .map(|anchor| {
            format!(
                "--menu-popup-left: {:.0}px; --menu-popup-top: {:.0}px",
                anchor.x, anchor.y
            )
        })
        .unwrap_or_default();
    rsx! {
        button { class: "menu-popup-backdrop", aria_label: t.get("aria_close_menu"), onclick: move |_| state.write().menu_open = None }
        div { class: "menu-popup menu-{menu}", style: position, onclick: move |event| event.stop_propagation(),
            div { class: "menu-popup-title", "{menu_title}" }
            div { class: "menu-popup-content",
                match menu {
                    "file" => rsx! {
                        FileMenuItem { label: t.get("menu_open_level"), kind: ImportKind::Level }
                        FileMenuItem { label: t.get("menu_import_text"), kind: ImportKind::LevelText }
                        FileMenuItem { label: t.get("menu_export_from_unity3d"), kind: ImportKind::UnityExtract }
                        FileMenuItem { label: t.get("menu_export_from_unity_assets"), kind: ImportKind::UnityAssetsExtract }
                        FileMenuItem { label: t.get("menu_open_save"), kind: ImportKind::Save }
                        FileMenuItem { label: t.get("menu_import_xml"), kind: ImportKind::SaveXml }
                        div { class: "menu-rule" }
                        MenuItem { label: t.get("menu_export_save"), disabled: !has_save, onclick: move |_| files::export_save(state) }
                        MenuItem { label: t.get("menu_export_xml"), disabled: !has_save, onclick: move |_| files::export_save_xml(state) }
                        MenuItem { label: t.get("menu_export_level"), disabled: !has_level, onclick: move |_| files::export_level(state, LevelFormat::Bytes) }
                        MenuItem { label: t.get("menu_export_yaml"), disabled: !has_level, onclick: move |_| files::export_level(state, LevelFormat::Yaml) }
                        MenuItem { label: t.get("menu_export_toml"), disabled: !has_level, onclick: move |_| files::export_level(state, LevelFormat::Toml) }
                        FileMenuItem { label: t.get("menu_import_to_unity3d"), kind: ImportKind::UnityReplace, disabled: !has_level }
                        FileMenuItem { label: t.get("menu_import_to_unity_assets"), kind: ImportKind::UnityAssetsReplace, disabled: !has_level }
                    },
                    "edit" => rsx! {
                        MenuItem { label: t.get("menu_undo"), shortcut: "Ctrl/Cmd+Z", disabled: !state.read().can_undo(), onclick: move |_| state.write().undo() }
                        MenuItem { label: t.get("menu_redo"), shortcut: "Ctrl/Cmd+Shift+Z", disabled: !state.read().can_redo(), onclick: move |_| state.write().redo() }
                        div { class: "menu-rule" }
                        MenuItem { label: t.get("menu_select_all"), shortcut: "Ctrl/Cmd+A", onclick: move |_| state.write().select_all() }
                        MenuItem { label: t.get("save_edit_deselect_all"), shortcut: "Esc", onclick: move |_| state.write().clear_selection() }
                        div { class: "menu-rule" }
                        MenuItem { label: t.get("menu_copy"), shortcut: "Ctrl/Cmd+C", disabled: has_save, onclick: move |_| state.write().copy_selected() }
                        MenuItem { label: t.get("menu_cut"), shortcut: "Ctrl/Cmd+X", disabled: has_save, onclick: move |_| state.write().cut_selected() }
                        MenuItem { label: t.get("menu_paste"), shortcut: "Ctrl/Cmd+V", disabled: has_save || state.read().clipboard.is_none(), onclick: move |_| state.write().paste() }
                        MenuItem { label: t.get("menu_duplicate"), shortcut: "Ctrl/Cmd+D", disabled: !state.read().has_selection(), onclick: move |_| state.write().duplicate_selected() }
                        MenuItem { label: t.get("menu_flip_horizontal"), disabled: has_save || !state.read().has_selection(), onclick: move |_| state.write().flip_selected(true) }
                        MenuItem { label: t.get("menu_flip_vertical"), disabled: has_save || !state.read().has_selection(), onclick: move |_| state.write().flip_selected(false) }
                        MenuItem { label: t.get("menu_delete"), shortcut: "Delete", disabled: !state.read().has_selection(), onclick: move |_| state.write().request_delete_selected() }
                        div { class: "menu-rule" }
                        MenuItem { label: t.get("menu_add_object"), disabled: has_save, onclick: move |_| state.write().modal = Some(Modal::AddObject) }
                    },
                    "view" => rsx! {
                        CheckItem { label: t.get("menu_object_list"), checked: state.read().show_tree, onchange: move |value| state.write().show_tree = value }
                        CheckItem { label: t.get("menu_properties"), checked: state.read().show_properties, onchange: move |value| state.write().show_properties = value }
                        CheckItem { label: t.get("menu_tools"), checked: state.read().show_tools, onchange: move |value| state.write().show_tools = value }
                        CheckItem { label: t.get("menu_preview_controls"), checked: state.read().show_preview_controls, onchange: move |value| state.write().show_preview_controls = value }
                        div { class: "menu-rule" }
                        CheckItem { label: t.get("menu_grid"), checked: state.read().show_grid, onchange: move |value| state.write().show_grid = value }
                        CheckItem { label: t.get("menu_background"), checked: state.read().show_background, onchange: move |value| state.write().show_background = value }
                        CheckItem { label: t.get("menu_construction_grid"), checked: state.read().show_construction_grid, onchange: move |value| state.write().show_construction_grid = value }
                        CheckItem { label: t.get("menu_dark_overlay"), checked: state.read().show_dark_overlay, onchange: move |value| state.write().show_dark_overlay = value }
                        CheckItem { label: t.get("menu_physics_ground"), checked: state.read().show_ground, onchange: move |value| state.write().show_ground = value }
                        CheckItem { label: t.get("menu_terrain_tris"), checked: state.read().show_terrain_triangles, onchange: move |value| state.write().show_terrain_triangles = value }
                        CheckItem { label: t.get("menu_preview_route"), checked: state.read().show_preview_route, onchange: move |value| state.write().show_preview_route = value }
                        div { class: "menu-rule" }
                        MenuItem { label: t.get("menu_fit_view"), onclick: move |_| state.write().fit_view() }
                    },
                    _ => rsx! {},
                }
            }
        }
    }
}

#[component]
fn MenuItem(
    label: String,
    #[props(default)] shortcut: &'static str,
    #[props(default)] disabled: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button { class: "menu-item", disabled, onclick: move |event| onclick.call(event),
            span { "{label}" }
            span { class: "shortcut", "{shortcut}" }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct SelectOption {
    value: String,
    label: String,
}

impl SelectOption {
    fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

#[component]
fn CustomSelect(
    aria_label: String,
    value: String,
    options: Vec<SelectOption>,
    onchange: EventHandler<String>,
    #[props(default)] class: String,
) -> Element {
    let mut open = use_signal(|| false);
    let is_open = *open.read();
    let selected_label = options
        .iter()
        .find(|option| option.value == value)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| value.clone());
    let root_class = if class.is_empty() {
        if is_open {
            "rton-custom-select open".to_string()
        } else {
            "rton-custom-select".to_string()
        }
    } else if is_open {
        format!("rton-custom-select {class} open")
    } else {
        format!("rton-custom-select {class}")
    };

    rsx! {
        div {
            class: "{root_class}",
            onkeydown: move |event| {
                let key = event.key().to_string();
                if matches!(key.as_str(), "Enter" | " " | "ArrowDown") && !*open.read() {
                    event.prevent_default();
                    open.set(true);
                } else if key == "Escape" && *open.read() {
                    event.prevent_default();
                    event.stop_propagation();
                    open.set(false);
                }
            },
            if is_open {
                div {
                    class: "rton-custom-select-backdrop",
                    aria_hidden: "true",
                    onclick: move |_| open.set(false),
                }
            }
            button {
                r#type: "button",
                class: "rton-custom-select-control",
                aria_label: "{aria_label}",
                aria_haspopup: "listbox",
                aria_expanded: is_open,
                onclick: move |_| open.set(!is_open),
                span { class: "rton-custom-select-value", "{selected_label}" }
                span { class: "rton-custom-select-caret", aria_hidden: "true", {icon(LdChevronDown)} }
            }
            div {
                class: "rton-custom-select-menu",
                role: "listbox",
                aria_label: "{aria_label}",
                aria_hidden: !is_open,
                for option in options {
                    {
                        let selected = option.value == value;
                        let option_value = option.value.clone();
                        rsx! {
                            button {
                                key: "{option.value}",
                                r#type: "button",
                                class: if selected { "active" } else { "" },
                                role: "option",
                                tabindex: if is_open { "0" } else { "-1" },
                                aria_selected: selected,
                                onclick: move |_| {
                                    open.set(false);
                                    onchange.call(option_value.clone());
                                },
                                span { "{option.label}" }
                                if selected {
                                    span { class: "rton-custom-select-check", aria_hidden: "true", {icon(LdCheck)} }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ImportKind {
    Level,
    LevelText,
    Save,
    SaveXml,
    UnityExtract,
    UnityAssetsExtract,
    UnityReplace,
    UnityAssetsReplace,
}

impl ImportKind {
    const fn accept(self) -> &'static str {
        match self {
            Self::Level => ".bytes",
            Self::LevelText => ".yaml,.yml,.toml",
            Self::Save => ".dat,.contraption,.xml",
            Self::SaveXml => ".xml",
            Self::UnityExtract | Self::UnityReplace => ".unity3d",
            Self::UnityAssetsExtract | Self::UnityAssetsReplace => ".assets",
        }
    }

    const fn filter_key(self) -> &'static str {
        match self {
            Self::Level => "filter_level_files",
            Self::LevelText => "filter_level_text_files",
            Self::Save => "filter_save_files",
            Self::SaveXml => "filter_save_xml",
            Self::UnityExtract | Self::UnityReplace => "filter_unity3d_files",
            Self::UnityAssetsExtract | Self::UnityAssetsReplace => "filter_unity_assets_files",
        }
    }

    const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Level => &["bytes"],
            Self::LevelText => &["yaml", "yml", "toml"],
            Self::Save => &["dat", "contraption", "xml"],
            Self::SaveXml => &["xml"],
            Self::UnityExtract | Self::UnityReplace => &["unity3d"],
            Self::UnityAssetsExtract | Self::UnityAssetsReplace => &["assets"],
        }
    }
}

async fn import_picked_file(
    mut state: Signal<EditorState>,
    kind: ImportKind,
    name: String,
    bytes: Vec<u8>,
) {
    let reading = state
        .read()
        .t()
        .format("status_reading", &[("name", name.clone())]);
    state.write().active_mut().status = reading;
    state.write().menu_open = None;
    match kind {
        ImportKind::Level | ImportKind::LevelText => files::import_level(state, name, bytes).await,
        ImportKind::Save => files::import_save(state, name, bytes).await,
        ImportKind::SaveXml => files::import_save_xml(state, name, bytes).await,
        ImportKind::UnityExtract => {
            files::open_unity_bundle(state, name, bytes, UnityBundleMode::ExtractLevels).await
        }
        ImportKind::UnityAssetsExtract => {
            files::open_unity_assets_file(state, name, bytes, UnityBundleMode::ExtractLevels).await
        }
        ImportKind::UnityReplace => {
            files::open_unity_bundle(state, name, bytes, UnityBundleMode::ReplaceLevel).await
        }
        ImportKind::UnityAssetsReplace => {
            files::open_unity_assets_file(state, name, bytes, UnityBundleMode::ReplaceLevel).await
        }
    }
}

fn open_native_import_dialog(mut state: Signal<EditorState>, kind: ImportKind) {
    state.write().menu_open = None;
    let filter_name = state.read().t().get(kind.filter_key());
    dioxus::dioxus_core::spawn_forever(async move {
        let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter(filter_name, kind.extensions())
            .pick_file()
            .await
        else {
            return;
        };
        let name = file.file_name();
        let bytes = file.read().await;
        import_picked_file(state, kind, name, bytes).await;
    });
}

#[component]
fn FileMenuItem(label: String, kind: ImportKind, #[props(default)] disabled: bool) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();

    rsx! {
        if cfg!(target_arch = "wasm32") {
            label {
                class: if disabled { "menu-item file-menu-item disabled" } else { "menu-item file-menu-item" },
                aria_disabled: disabled,
                input {
                    class: "file-menu-input",
                    r#type: "file",
                    accept: kind.accept(),
                    disabled,
                    onchange: move |event| {
                        let Some(file) = event.files().into_iter().next() else {
                            return;
                        };
                        let name = file.name();
                        dioxus::dioxus_core::spawn_forever(async move {
                            match file.read_bytes().await {
                                Ok(bytes) => {
                                    import_picked_file(state, kind, name, bytes.to_vec()).await;
                                }
                                Err(error) => {
                                    state.write().active_mut().status = error.to_string();
                                }
                            }
                        });
                    }
                }
                span { "{label}" }
            }
        } else {
            button {
                class: "menu-item file-menu-item",
                disabled,
                onclick: move |_| open_native_import_dialog(state, kind),
                "{label}"
            }
        }
    }
}

#[cfg(test)]
mod import_filter_tests {
    use super::ImportKind;

    #[test]
    fn import_filters_match_the_formats_each_action_can_parse() {
        assert_eq!(ImportKind::Level.extensions(), &["bytes"]);
        assert_eq!(ImportKind::LevelText.extensions(), &["yaml", "yml", "toml"]);
        assert_eq!(
            ImportKind::Save.extensions(),
            &["dat", "contraption", "xml"]
        );
        assert_eq!(ImportKind::SaveXml.extensions(), &["xml"]);
        assert_eq!(ImportKind::UnityExtract.extensions(), &["unity3d"]);
        assert_eq!(ImportKind::UnityAssetsExtract.extensions(), &["assets"]);
        assert_eq!(ImportKind::UnityReplace.extensions(), &["unity3d"]);
        assert_eq!(ImportKind::UnityAssetsReplace.extensions(), &["assets"]);
        assert_eq!(ImportKind::Level.accept(), ".bytes");
        assert_eq!(ImportKind::LevelText.accept(), ".yaml,.yml,.toml");
    }
}

#[component]
fn CheckItem(label: String, checked: bool, onchange: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "menu-check",
            input { r#type: "checkbox", checked, onchange: move |event| onchange.call(event.checked()) }
            span { "{label}" }
        }
    }
}

#[component]
fn TabBar() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let tabs = {
        let state = state.read();
        state
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| !tab.is_workspace_placeholder())
            .map(|(index, tab)| {
                (
                    index,
                    tab.file_name.clone(),
                    tab.dirty,
                    index == state.active_tab,
                )
            })
            .collect::<Vec<_>>()
    };
    rsx! {
        nav { class: "rton-tab-strip",
            div { class: "rton-file-tabs",
                for (index, title, dirty, active) in tabs {
                    div {
                        class: if active { "rton-file-tab active" } else { "rton-file-tab" },
                        draggable: "true",
                        ondragstart: move |_| state.write().tab_dragging = Some(index),
                        ondragend: move |_| state.write().tab_dragging = None,
                        ondragover: move |event| event.prevent_default(),
                        ondrop: move |event| {
                            event.prevent_default();
                            let source = state.read().tab_dragging;
                            if let Some(source) = source { state.write().reorder_tabs(source, index); }
                            state.write().tab_dragging = None;
                        },
                        button { class: "rton-file-tab-label", onclick: move |_| state.write().activate_tab(index),
                            span { class: "rton-file-tab-name", "{title}" }
                            if dirty { span { class: "rton-file-tab-dirty" } }
                        }
                        button { class: "rton-file-tab-close", title: t.get("menu_close_tab"), onclick: move |_| state.write().request_close_tab(index), {icon(LdX)} }
                    }
                }
                button { class: "rton-new-tab", title: t.get("menu_new_level"), onclick: move |_| state.write().new_tab(), {icon(LdPlus)} }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct TreeContextMenu {
    x: i32,
    y: i32,
    indices: Option<Vec<ObjectIndex>>,
}

fn invoke_tree_object_action(
    action: &'static str,
    indices: Vec<ObjectIndex>,
    mut state: Signal<EditorState>,
    context_menu: ContextMenuTransition<TreeContextMenu>,
) {
    context_menu.dismiss();
    let mut editor = state.write();
    editor.set_selection(indices);
    match action {
        "copy" => editor.copy_selected(),
        "cut" => editor.cut_selected(),
        "duplicate" => editor.duplicate_selected(),
        "delete" => editor.request_delete_selected(),
        _ => {}
    }
}

#[component]
fn ObjectTree() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let context_menu = ContextMenuTransition::new(
        use_signal(|| None::<TreeContextMenu>),
        use_signal(|| false),
        use_signal(|| 0_u64),
    );
    let mut collapsed = use_signal(BTreeSet::<ObjectIndex>::new);
    let mobile_open = state.read().mobile_panel == Some(MobilePanel::Objects);
    let roots = state
        .read()
        .active()
        .level
        .as_ref()
        .map(|level| level.roots.clone())
        .unwrap_or_default();
    let t = state.read().t();
    rsx! {
        aside {
            id: "rton-file-drawer",
            class: "rton-side-panel rton-side-panel-left tree-pane",
            onclick: move |_| context_menu.dismiss(),
            oncontextmenu: move |event| event.prevent_default(),
            div { class: "panel-header panel-heading",
                div { class: "panel-header-title-line",
                    span { class: "panel-header-icon", {icon(LdListTree)} }
                    span { class: "panel-title", {state.read().t().get("panel_object_list")} }
                }
                button {
                    class: "panel-close-mobile",
                    title: t.get("common_close"),
                    aria_label: t.get("aria_close_object_list"),
                    onclick: move |_| {
                        if mobile_open { state.write().mobile_panel = None; } else { state.write().show_tree = false; }
                    },
                    {icon(LdX)}
                }
            }
            div { class: "tree-actions",
                button { title: t.get("win_add_object"), onclick: move |_| state.write().modal = Some(Modal::AddObject), {icon(LdPlus)} }
                button { title: t.get("menu_delete"), onclick: move |_| state.write().request_delete_selected(), {icon(LdTrash2)} }
            }
            div {
                class: "tree-scroll",
                onclick: move |_| {
                    context_menu.dismiss();
                    state.write().clear_selection();
                },
                onscroll: move |_| context_menu.dismiss(),
                oncontextmenu: move |event| {
                    event.prevent_default();
                    event.stop_propagation();
                    let point = event.client_coordinates();
                    context_menu.show(TreeContextMenu {
                        x: point.x.round() as i32,
                        y: point.y.round() as i32,
                        indices: None,
                    });
                },
                for root in roots {
                    TreeNode {
                        index: root,
                        depth: 0,
                        context_menu,
                        collapsed,
                    }
                }
            }
        }
        if let Some(menu) = context_menu.value() {
            button {
                class: if context_menu.is_closing() { "tree-context-backdrop closing" } else { "tree-context-backdrop" },
                aria_label: t.get("text_close_context_menu"),
                onclick: move |_| context_menu.dismiss(),
                oncontextmenu: move |event| {
                    event.prevent_default();
                    context_menu.dismiss();
                },
            }
            div {
                class: if context_menu.is_closing() { "tree-context-menu closing" } else { "tree-context-menu" },
                role: "menu",
                style: "left: max(4px, min(calc(100vw - 190px), {menu.x}px)); top: max(4px, min(calc(100dvh - 250px), {menu.y}px));",
                onmousedown: move |event| {
                    event.prevent_default();
                    event.stop_propagation();
                },
                oncontextmenu: move |event| {
                    event.prevent_default();
                    event.stop_propagation();
                },
                if let Some(indices) = menu.indices.clone() {
                    button {
                        role: "menuitem",
                        onclick: {
                            let indices = indices.clone();
                            move |_| invoke_tree_object_action("copy", indices.clone(), state, context_menu)
                        },
                        {state.read().t().get("menu_copy")}
                    }
                    button {
                        role: "menuitem",
                        onclick: {
                            let indices = indices.clone();
                            move |_| invoke_tree_object_action("cut", indices.clone(), state, context_menu)
                        },
                        {state.read().t().get("menu_cut")}
                    }
                    button {
                        role: "menuitem",
                        onclick: {
                            let indices = indices.clone();
                            move |_| invoke_tree_object_action("duplicate", indices.clone(), state, context_menu)
                        },
                        {state.read().t().get("menu_duplicate")}
                    }
                    hr {}
                    button {
                        role: "menuitem",
                        onclick: move |_| invoke_tree_object_action("delete", indices.clone(), state, context_menu),
                        {state.read().t().get("menu_delete")}
                    }
                } else {
                    button {
                        role: "menuitem",
                        disabled: state.read().clipboard.is_none(),
                        onclick: move |_| {
                            context_menu.dismiss();
                            let mut editor = state.write();
                            editor.clear_selection();
                            editor.paste();
                        },
                        {state.read().t().get("menu_paste")}
                    }
                    hr {}
                    button {
                        role: "menuitem",
                        onclick: move |_| {
                            context_menu.dismiss();
                            collapsed.write().clear();
                        },
                        {state.read().t().get("menu_expand_all")}
                    }
                    button {
                        role: "menuitem",
                        onclick: move |_| {
                            context_menu.dismiss();
                            let parents = state
                                .read()
                                .active()
                                .level
                                .as_ref()
                                .map(|level| {
                                    level
                                        .objects
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(index, object)| {
                                            matches!(object, LevelObject::Parent(_)).then_some(index)
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            collapsed.set(parents);
                        },
                        {state.read().t().get("menu_collapse_all")}
                    }
                    hr {}
                    button {
                        role: "menuitem",
                        disabled: !state.read().has_selection(),
                        onclick: move |_| {
                            context_menu.dismiss();
                            state.write().clear_selection();
                        },
                        {state.read().t().get("menu_clear_selection")}
                    }
                    hr {}
                    button {
                        role: "menuitem",
                        onclick: move |_| {
                            context_menu.dismiss();
                            state.write().modal = Some(Modal::AddObject);
                        },
                        {state.read().t().get("menu_add_object")}
                    }
                }
            }
        }
    }
}

#[component]
fn TreeNode(
    index: ObjectIndex,
    depth: usize,
    context_menu: ContextMenuTransition<TreeContextMenu>,
    mut collapsed: Signal<BTreeSet<ObjectIndex>>,
) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let item = state
        .read()
        .active()
        .level
        .as_ref()
        .and_then(|level| level.objects.get(index))
        .cloned();
    let Some(item) = item else {
        return rsx! {};
    };
    let (name, kind, children, is_parent) = match item {
        LevelObject::Parent(parent) => (parent.name, "Group", parent.children, true),
        LevelObject::Prefab(prefab) => (
            prefab.name,
            if prefab.terrain_data.is_some() {
                "Terrain"
            } else {
                "Prefab"
            },
            Vec::new(),
            false,
        ),
    };
    let selected = state.read().active().selected.contains(&index);
    let context_indices = if selected {
        state.read().active().selected.iter().copied().collect()
    } else {
        vec![index]
    };
    let is_collapsed = is_parent && collapsed.read().contains(&index);
    let t = state.read().t();
    let kind = match kind {
        "Group" => t.get("tree_group"),
        "Terrain" => t.get("add_data_type_terrain"),
        _ => t.get("tree_prefab"),
    };
    rsx! {
        div { class: "tree-parent",
            button {
                class: if selected { "tree-row selected" } else { "tree-row" },
                style: "padding-left: {depth * 14 + 8}px",
                draggable: "true",
                onclick: move |event| {
                    event.stop_propagation();
                    context_menu.dismiss();
                    let modifiers = event.modifiers();
                    state.write().select_from_tree(
                        index,
                        modifiers.intersects(Modifiers::CONTROL | Modifiers::META),
                        modifiers.contains(Modifiers::SHIFT),
                    );
                },
                oncontextmenu: move |event| {
                    event.prevent_default();
                    event.stop_propagation();
                    let point = event.client_coordinates();
                    context_menu.show(TreeContextMenu {
                        x: point.x.round() as i32,
                        y: point.y.round() as i32,
                        indices: Some(context_indices.clone()),
                    });
                },
                ondragstart: move |_| state.write().tree_dragging = Some(index),
                ondragend: move |_| state.write().tree_dragging = None,
                ondragover: move |event| event.prevent_default(),
                ondrop: move |event| {
                    event.prevent_default();
                    let source = state.read().tree_dragging;
                    if let Some(source) = source {
                        state.write().move_tree_object(source, index);
                    }
                    state.write().tree_dragging = None;
                },
                if is_parent {
                    span {
                        class: "tree-disclosure",
                        title: if is_collapsed { t.get("tree_expand_group") } else { t.get("tree_collapse_group") },
                        aria_hidden: "true",
                        onclick: move |event| {
                            event.stop_propagation();
                            let mut collapsed = collapsed.write();
                            if !collapsed.remove(&index) {
                                collapsed.insert(index);
                            }
                        },
                        if is_collapsed { {icon(LdChevronRight)} } else { {icon(LdChevronDown)} }
                    }
                } else {
                    span { class: "tree-disclosure spacer", aria_hidden: "true" }
                }
                span { class: "tree-kind", "{kind}" }
                span { class: "tree-label", "{name}" }
                span { class: "tree-index", "{index}" }
            }
            if !is_collapsed {
                for child in children {
                    TreeNode {
                        index: child,
                        depth: depth + 1,
                        context_menu,
                        collapsed,
                    }
                }
            }
        }
    }
}

#[component]
fn PropertiesPanel() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let mobile_open = state.read().mobile_panel == Some(MobilePanel::Properties);
    let selected = state.read().active().selected.iter().next().copied();
    let object = selected.and_then(|index| {
        state
            .read()
            .active()
            .level
            .as_ref()
            .and_then(|level| level.objects.get(index))
            .cloned()
    });
    let t = state.read().t();
    rsx! {
        aside { id: "rton-inspector-drawer", class: "rton-side-panel rton-side-panel-right properties-pane",
            div { class: "panel-header panel-heading",
                div { class: "panel-header-title-line",
                    span { class: "panel-header-icon", {icon(LdSlidersHorizontal)} }
                    span { class: "panel-title", {state.read().t().get("panel_properties")} }
                }
                button {
                    class: "panel-close-mobile",
                    title: t.get("common_close"),
                    aria_label: t.get("aria_close_properties"),
                    onclick: move |_| {
                        if mobile_open { state.write().mobile_panel = None; } else { state.write().show_properties = false; }
                    },
                    {icon(LdX)}
                }
            }
            div { class: "properties-scroll",
                match (selected, object) {
                    (Some(index), Some(LevelObject::Parent(parent))) => rsx! {
                        TextField { label: t.get("prop_name"), value: parent.name, onchange: move |value| update_name(state, index, value) }
                        Vector3Fields { label: t.get("prop_position"), values: [parent.position.x, parent.position.y, parent.position.z], onchange: move |values| update_position(state, index, values) }
                        div { class: "meta", {t.format("prop_group_children", &[("count", parent.children.len().to_string())])} }
                    },
                    (Some(index), Some(LevelObject::Prefab(prefab))) => rsx! { PrefabProperties { index, prefab } },
                    _ => rsx! { div { class: "meta", {t.get("panel_select_hint")} } },
                }
            }
        }
    }
}

#[component]
fn TextField(label: String, value: String, onchange: EventHandler<String>) -> Element {
    rsx! {
        label { class: "property-field",
            span { "{label}" }
            input { value, oninput: move |event| onchange.call(event.value()) }
        }
    }
}

#[component]
fn Vector3Fields(label: String, values: [f32; 3], onchange: EventHandler<[f32; 3]>) -> Element {
    rsx! {
        div { class: "property-field",
            span { "{label}" }
            div { class: "vector-grid",
                for (axis, axis_label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    label {
                        span { class: "axis-label", "{axis_label}" }
                        input {
                            r#type: "number",
                            step: "0.1",
                            value: "{values[axis]}",
                            oninput: move |event| {
                                if let Ok(value) = event.value().parse::<f32>() {
                                    let mut next = values;
                                    next[axis] = value;
                                    onchange.call(next);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AddObjectVector(label: String, kind: &'static str) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let values = {
        let editor = state.read();
        let value = match kind {
            "position" => editor.add_object.position,
            "rotation" => editor.add_object.rotation,
            _ => editor.add_object.scale,
        };
        [value.x, value.y, value.z]
    };
    rsx! {
        div { class: "property-field",
            span { "{label}" }
            div { class: "vector-grid",
                for (axis, axis_label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    label {
                        span { class: "axis-label", "{axis_label}" }
                        input {
                            r#type: "number",
                            step: "0.1",
                            value: "{values[axis]}",
                            oninput: move |event| if let Ok(value) = event.value().parse::<f32>() {
                                let mut editor = state.write();
                                let target = match kind {
                                    "position" => &mut editor.add_object.position,
                                    "rotation" => &mut editor.add_object.rotation,
                                    _ => &mut editor.add_object.scale,
                                };
                                match axis { 0 => target.x = value, 1 => target.y = value, _ => target.z = value }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PrefabProperties(index: usize, prefab: PrefabInstance) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let is_goal = prefab.name.to_ascii_lowercase().starts_with("goalarea");
    let goal_animation = parse_goal_animation_state(
        prefab
            .override_data
            .as_ref()
            .map(|data| data.raw_text.as_str()),
    );
    rsx! {
        TextField { label: t.get("prop_name"), value: prefab.name.clone(), onchange: move |value| update_name(state, index, value) }
        label { class: "property-field",
            span { {t.get("prop_prefab_index")} }
            input {
                r#type: "number",
                value: "{prefab.prefab_index}",
                oninput: move |event| if let Ok(value) = event.value().parse::<i16>() {
                    update_prefab_index(state, index, value);
                }
            }
        }
        Vector3Fields { label: t.get("prop_position"), values: [prefab.position.x, prefab.position.y, prefab.position.z], onchange: move |values| update_position(state, index, values) }
        Vector3Fields { label: t.get("prop_rotation"), values: [prefab.rotation.x, prefab.rotation.y, prefab.rotation.z], onchange: move |values| update_rotation(state, index, values) }
        Vector3Fields { label: t.get("prop_scale"), values: [prefab.scale.x, prefab.scale.y, prefab.scale.z], onchange: move |values| update_scale(state, index, values) }
        div { class: "property-field",
            span { {t.get("prop_data_type")} }
            CustomSelect {
                aria_label: t.get("prop_data_type"),
                value: match prefab.data_type {
                    DataType::Terrain => "terrain",
                    DataType::PrefabOverrides => "overrides",
                    DataType::None => "none",
                }.to_string(),
                options: vec![
                    SelectOption::new("none", t.get("add_data_type_none")),
                    SelectOption::new("terrain", t.get("add_data_type_terrain")),
                    SelectOption::new("overrides", t.get("add_data_type_prefab_overrides")),
                ],
                onchange: move |value: String| update_data_type(state, index, &value),
            }
        }
        if is_goal {
            div { class: "property-field",
                span { {t.get("prop_goal_animation")} }
                CustomSelect {
                    aria_label: t.get("prop_goal_animation"),
                    value: (if goal_animation == GoalAnimationState::Vanishing { "vanishing" } else { "idle" }).to_string(),
                    options: vec![
                        SelectOption::new("idle", t.get("prop_goal_animation_idle")),
                        SelectOption::new("vanishing", t.get("prop_goal_animation_vanishing")),
                    ],
                    onchange: move |value: String| update_goal_animation(state, index, &value),
                }
            }
        }
        if let Some(terrain) = prefab.terrain_data {
            TerrainProperties { index, terrain: *terrain }
        }
        if let Some(overrides) = prefab.override_data {
            OverrideProperties { index, raw_text: overrides.raw_text }
        }
    }
}

#[component]
fn TerrainProperties(index: usize, terrain: TerrainData) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let nodes = badpiggies_editor_core::domain::terrain_gen::extract_curve_nodes(&terrain);
    let closed = badpiggies_editor_core::domain::terrain_gen::is_closed_loop(&nodes);
    let rgba = terrain.fill_color.to_rgba8();
    let fill_color = format!("#{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2]);
    let alpha_percent = terrain.fill_color.a.clamp(0.0, 1.0) * 100.0;
    rsx! {
        section { class: "property-section",
            h3 { {t.get("prop_terrain")} }
            div { class: "property-stats",
                span { {t.format("prop_fill_vertices", &[("count", terrain.fill_mesh.vertices.len().to_string())])} }
                span { {t.format("prop_curve_vertices", &[("count", terrain.curve_mesh.vertices.len().to_string())])} }
            }
            label { class: "property-toggle",
                span { {t.get("prop_collider")} }
                input { r#type: "checkbox", checked: terrain.has_collider, onchange: move |event| update_terrain_bool(state, index, "collider", event.checked()) }
            }
            label { class: "property-toggle",
                span { {t.get("prop_terrain_closed")} }
                input { r#type: "checkbox", checked: closed, onchange: move |event| update_terrain_closed(state, index, event.checked()) }
            }
            label { class: "property-field color-field",
                span { {t.get("prop_fill_color")} }
                div {
                    input { r#type: "color", value: fill_color, oninput: move |event| update_terrain_color(state, index, &event.value()) }
                    div { class: "terrain-alpha-slider", style: "--alpha-progress: {alpha_percent}%;",
                        input { r#type: "range", min: "0", max: "1", step: "0.01", value: "{terrain.fill_color.a}", title: t.get("prop_alpha"), aria_label: t.get("prop_fill_opacity"), oninput: move |event| if let Ok(value) = event.value().parse() { update_terrain_alpha(state, index, value); } }
                    }
                }
            }
            TerrainNumberField { index, label: t.get("prop_fill_tex_index"), field: "texture", value: terrain.fill_texture_index as f32, step: "1" }
            TerrainNumberField { index, label: t.get("prop_fill_offset_x"), field: "offset_x", value: terrain.fill_texture_tile_offset_x, step: "0.01" }
            TerrainNumberField { index, label: t.get("prop_fill_offset_y"), field: "offset_y", value: terrain.fill_texture_tile_offset_y, step: "0.01" }
            for (texture_index, texture) in terrain.curve_textures.into_iter().enumerate() {
                div { class: "curve-texture-fields",
                    strong { {t.format("prop_curve_tex", &[("idx", texture_index.to_string())])} }
                    TerrainCurveField { index, texture_index, label: t.get("prop_strip_width"), field: "width", value: texture.size.y }
                    TerrainCurveField { index, texture_index, label: t.get("prop_fade_threshold"), field: "fade", value: texture.fade_threshold }
                }
            }
        }
    }
}

#[component]
fn TerrainNumberField(
    index: usize,
    label: String,
    field: &'static str,
    value: f32,
    step: &'static str,
) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    rsx! {
        label { class: "property-field",
            span { "{label}" }
            input { r#type: "number", step, value: "{value}", oninput: move |event| if let Ok(value) = event.value().parse() { update_terrain_number(state, index, field, value); } }
        }
    }
}

#[component]
fn TerrainCurveField(
    index: usize,
    texture_index: usize,
    label: String,
    field: &'static str,
    value: f32,
) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    rsx! {
        label { class: "property-field compact",
            span { "{label}" }
            input { r#type: "number", step: "0.01", value: "{value}", oninput: move |event| if let Ok(value) = event.value().parse() { update_curve_texture(state, index, texture_index, field, value); } }
        }
    }
}

#[component]
fn OverrideProperties(index: usize, raw_text: String) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let nodes = parse_override_text(&raw_text);
    rsx! {
        section { class: "property-section override-section",
            h3 { {t.get("override_title")} }
            details { open: true,
                summary { {t.get("override_visual_tree")} }
                div { class: "override-tree",
                    for (node_index, node) in nodes.into_iter().enumerate() {
                        OverrideNodeEditor { object_index: index, path: vec![node_index], node }
                    }
                    button { class: "override-add", onclick: move |_| add_override_root(state, index), {t.get("override_add_root")} }
                }
            }
            details {
                summary { {t.get("override_raw_text")} }
                textarea {
                    class: "override-raw-editor",
                    value: raw_text,
                    spellcheck: "false",
                    oninput: move |event| update_override_raw(state, index, event.value()),
                }
            }
        }
    }
}

#[component]
fn OverrideNodeEditor(object_index: usize, path: Vec<usize>, node: OverrideNode) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let has_children = !node.children.is_empty();
    rsx! {
        details { class: "override-node", open: path.len() < 3,
            summary {
                span { class: "override-type", "{node.node_type}" }
                input {
                    value: node.name,
                    aria_label: t.get("override_field_name"),
                    onclick: move |event| event.stop_propagation(),
                    oninput: {
                        let path = path.clone();
                        move |event| update_override_node(state, object_index, &path, Some(event.value()), None)
                    }
                }
                if let Some(value) = node.value {
                    input {
                        value,
                        aria_label: t.get("override_field_value"),
                        onclick: move |event| event.stop_propagation(),
                        oninput: {
                            let path = path.clone();
                            move |event| update_override_node(state, object_index, &path, None, Some(event.value()))
                        }
                    }
                }
                button {
                    title: t.get("override_delete_field"),
                    onclick: {
                        let path = path.clone();
                        move |event| { event.stop_propagation(); delete_override_node(state, object_index, &path); }
                    },
                    "×"
                }
            }
            if has_children {
                div { class: "override-children",
                    for (child_index, child) in node.children.into_iter().enumerate() {
                        OverrideNodeEditor { object_index, path: { let mut next = path.clone(); next.push(child_index); next }, node: child }
                    }
                }
            }
            button { class: "override-add", onclick: { let path = path.clone(); move |_| add_override_child(state, object_index, &path) }, {t.get("override_add_child")} }
        }
    }
}

fn update_name(mut state: Signal<EditorState>, index: usize, value: String) {
    state.write().mutate_level(|level| {
        if let Some(object) = level.objects.get_mut(index) {
            match object {
                LevelObject::Parent(parent) => parent.name = value,
                LevelObject::Prefab(prefab) => prefab.name = value,
            }
        }
    });
}

fn update_position(mut state: Signal<EditorState>, index: usize, values: [f32; 3]) {
    state.write().mutate_level(|level| {
        if let Some(object) = level.objects.get_mut(index) {
            match object {
                LevelObject::Parent(parent) => {
                    [parent.position.x, parent.position.y, parent.position.z] = values;
                }
                LevelObject::Prefab(prefab) => {
                    [prefab.position.x, prefab.position.y, prefab.position.z] = values;
                }
            }
        }
    });
}

fn update_rotation(mut state: Signal<EditorState>, index: usize, values: [f32; 3]) {
    state.write().mutate_level(|level| {
        if let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) {
            [prefab.rotation.x, prefab.rotation.y, prefab.rotation.z] = values;
        }
    });
}

fn update_scale(mut state: Signal<EditorState>, index: usize, values: [f32; 3]) {
    state.write().mutate_level(|level| {
        if let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) {
            [prefab.scale.x, prefab.scale.y, prefab.scale.z] = values;
        }
    });
}

fn update_prefab_index(mut state: Signal<EditorState>, index: usize, value: i16) {
    state.write().mutate_level(|level| {
        if let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) {
            prefab.prefab_index = value;
        }
    });
}

fn update_data_type(mut state: Signal<EditorState>, index: usize, value: &str) {
    let data_type = match value {
        "terrain" => DataType::Terrain,
        "overrides" => DataType::PrefabOverrides,
        _ => DataType::None,
    };
    state.write().mutate_level(|level| {
        let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) else {
            return;
        };
        prefab.data_type = data_type;
        match data_type {
            DataType::None => {
                prefab.terrain_data = None;
                prefab.override_data = None;
            }
            DataType::Terrain => {
                prefab
                    .terrain_data
                    .get_or_insert_with(|| Box::new(crate::editor_state::default_terrain_data()));
                prefab.override_data = None;
            }
            DataType::PrefabOverrides => {
                prefab.terrain_data = None;
                prefab
                    .override_data
                    .get_or_insert_with(|| PrefabOverrideData {
                        raw_text: format!("GameObject {}\n", prefab.name),
                        raw_bytes: format!("GameObject {}\n", prefab.name).into_bytes(),
                    });
            }
        }
    });
}

fn update_goal_animation(mut state: Signal<EditorState>, index: usize, value: &str) {
    let animation = if value == "vanishing" {
        GoalAnimationState::Vanishing
    } else {
        GoalAnimationState::Idle
    };
    state.write().mutate_level(|level| {
        let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) else {
            return;
        };
        let overrides = prefab
            .override_data
            .get_or_insert_with(|| PrefabOverrideData {
                raw_text: format!("GameObject {}\n", prefab.name),
                raw_bytes: Vec::new(),
            });
        set_goal_animation_state(&mut overrides.raw_text, animation);
        overrides.raw_bytes = overrides.raw_text.as_bytes().to_vec();
        prefab.data_type = DataType::PrefabOverrides;
        prefab.terrain_data = None;
    });
}

fn mutate_terrain(
    mut state: Signal<EditorState>,
    index: usize,
    update: impl FnOnce(&mut TerrainData),
) {
    state.write().mutate_level(|level| {
        let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) else {
            return;
        };
        if let Some(terrain) = prefab.terrain_data.as_mut() {
            update(terrain);
        }
    });
}

fn update_terrain_bool(state: Signal<EditorState>, index: usize, field: &'static str, value: bool) {
    mutate_terrain(state, index, |terrain| {
        if field == "collider" {
            terrain.has_collider = value;
        }
    });
}

fn update_terrain_closed(state: Signal<EditorState>, index: usize, closed: bool) {
    mutate_terrain(state, index, |terrain| {
        let mut nodes = badpiggies_editor_core::domain::terrain_gen::extract_curve_nodes(terrain);
        let is_closed = badpiggies_editor_core::domain::terrain_gen::is_closed_loop(&nodes);
        if closed && !is_closed && nodes.len() >= 2 {
            nodes.push(nodes[0].clone());
        } else if !closed && is_closed && nodes.len() >= 3 {
            nodes.pop();
        }
        badpiggies_editor_core::domain::terrain_gen::regenerate_terrain(terrain, &nodes);
    });
}

fn update_terrain_color(state: Signal<EditorState>, index: usize, value: &str) {
    let Some(hex) = value.strip_prefix('#') else {
        return;
    };
    let Ok(rgb) = u32::from_str_radix(hex, 16) else {
        return;
    };
    mutate_terrain(state, index, |terrain| {
        terrain.fill_color.r = ((rgb >> 16) & 0xff) as f32 / 255.0;
        terrain.fill_color.g = ((rgb >> 8) & 0xff) as f32 / 255.0;
        terrain.fill_color.b = (rgb & 0xff) as f32 / 255.0;
    });
}

fn update_terrain_alpha(state: Signal<EditorState>, index: usize, value: f32) {
    mutate_terrain(state, index, |terrain| {
        terrain.fill_color.a = value.clamp(0.0, 1.0);
    });
}

fn update_terrain_number(
    state: Signal<EditorState>,
    index: usize,
    field: &'static str,
    value: f32,
) {
    mutate_terrain(state, index, |terrain| match field {
        "texture" => terrain.fill_texture_index = value.round() as i32,
        "offset_x" => terrain.fill_texture_tile_offset_x = value,
        _ => terrain.fill_texture_tile_offset_y = value,
    });
}

fn update_curve_texture(
    state: Signal<EditorState>,
    index: usize,
    texture_index: usize,
    field: &'static str,
    value: f32,
) {
    mutate_terrain(state, index, |terrain| {
        let Some(texture) = terrain.curve_textures.get_mut(texture_index) else {
            return;
        };
        if field == "width" {
            texture.size.y = value.clamp(0.01, 5.0);
            let nodes = badpiggies_editor_core::domain::terrain_gen::extract_curve_nodes(terrain);
            badpiggies_editor_core::domain::terrain_gen::regenerate_terrain(terrain, &nodes);
        } else {
            texture.fade_threshold = value.clamp(0.0, 1.0);
        }
    });
}

fn update_override_raw(mut state: Signal<EditorState>, index: usize, raw_text: String) {
    state.write().mutate_level(|level| {
        let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) else {
            return;
        };
        prefab.override_data = Some(PrefabOverrideData {
            raw_bytes: raw_text.as_bytes().to_vec(),
            raw_text,
        });
        prefab.data_type = DataType::PrefabOverrides;
        prefab.terrain_data = None;
    });
}

fn mutate_override_tree(
    mut state: Signal<EditorState>,
    index: usize,
    update: impl FnOnce(&mut Vec<OverrideNode>),
) {
    state.write().mutate_level(|level| {
        let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) else {
            return;
        };
        let mut nodes = prefab
            .override_data
            .as_ref()
            .map(|data| parse_override_text(&data.raw_text))
            .unwrap_or_default();
        update(&mut nodes);
        let raw_text = serialize_override_tree(&nodes);
        prefab.override_data = Some(PrefabOverrideData {
            raw_bytes: raw_text.as_bytes().to_vec(),
            raw_text,
        });
        prefab.data_type = DataType::PrefabOverrides;
        prefab.terrain_data = None;
    });
}

fn override_node_mut<'a>(
    nodes: &'a mut [OverrideNode],
    path: &[usize],
) -> Option<&'a mut OverrideNode> {
    let (first, rest) = path.split_first()?;
    let node = nodes.get_mut(*first)?;
    if rest.is_empty() {
        Some(node)
    } else {
        override_node_mut(&mut node.children, rest)
    }
}

fn override_children_mut<'a>(
    nodes: &'a mut Vec<OverrideNode>,
    parent_path: &[usize],
) -> Option<&'a mut Vec<OverrideNode>> {
    if parent_path.is_empty() {
        Some(nodes)
    } else {
        Some(&mut override_node_mut(nodes, parent_path)?.children)
    }
}

fn update_override_node(
    state: Signal<EditorState>,
    index: usize,
    path: &[usize],
    name: Option<String>,
    value: Option<String>,
) {
    let path = path.to_vec();
    mutate_override_tree(state, index, |nodes| {
        if let Some(node) = override_node_mut(nodes, &path) {
            if let Some(name) = name {
                node.name = name;
            }
            if let Some(value) = value {
                node.value = Some(value);
            }
        }
    });
}

fn delete_override_node(state: Signal<EditorState>, index: usize, path: &[usize]) {
    let mut parent = path.to_vec();
    let Some(node_index) = parent.pop() else {
        return;
    };
    mutate_override_tree(state, index, |nodes| {
        if let Some(children) = override_children_mut(nodes, &parent)
            && node_index < children.len()
        {
            children.remove(node_index);
        }
    });
}

fn add_override_root(state: Signal<EditorState>, index: usize) {
    mutate_override_tree(state, index, |nodes| {
        nodes.push(new_override_field());
    });
}

fn add_override_child(state: Signal<EditorState>, index: usize, path: &[usize]) {
    let path = path.to_vec();
    mutate_override_tree(state, index, |nodes| {
        if let Some(node) = override_node_mut(nodes, &path) {
            node.children.push(new_override_field());
        }
    });
}

fn new_override_field() -> OverrideNode {
    OverrideNode {
        node_type: "String".to_string(),
        name: "field".to_string(),
        value: Some(String::new()),
        children: Vec::new(),
    }
}

#[component]
fn StatusBar() -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let pointer_state = consume_context::<Signal<CanvasPointerState>>();
    let (status, file_label, show_pointer) = {
        let state = state.read();
        (
            state.status_text(),
            state.active().status_bar_file_label(),
            state.active().level.is_some() && !state.active().is_workspace_placeholder(),
        )
    };
    let pointer_text = if show_pointer {
        pointer_state
            .read()
            .world
            .map(|pointer| format!("X: {:.2}  Y: {:.2}", pointer.x, pointer.y))
    } else {
        None
    };
    rsx! {
        footer { class: "status-bar",
            span { class: "status-main", "{status}" }
            if let Some(pointer_text) = pointer_text {
                span {
                    class: "status-coordinates",
                    title: "{pointer_text}",
                    "{pointer_text}"
                }
            }
            span { class: "status-spacer" }
            if let Some(file_label) = file_label {
                span { class: "status-document", title: "{file_label}", "{file_label}" }
            }
        }
    }
}

#[component]
fn ModalLayer() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let modal = state.read().modal;
    let Some(modal) = modal else {
        return rsx! {};
    };
    let is_settings = modal == Modal::Settings;
    let is_shortcuts = modal == Modal::Shortcuts;
    let is_logs = modal == Modal::Logs;
    let theme = state.read().theme;
    let t = state.read().t();
    let warnings = state.read().current_level_warnings();
    let delete_names = {
        let editor = state.read();
        editor
            .active()
            .level
            .as_ref()
            .map(|level| {
                editor
                    .pending_delete
                    .iter()
                    .filter_map(|index| level.objects.get(*index))
                    .map(|object| object.name().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let unity_bundle = state.read().unity_bundle.as_ref().map(|bundle| {
        (
            bundle.name.clone(),
            bundle.mode,
            bundle.source,
            bundle.entries.clone(),
            bundle.selected.clone(),
        )
    });
    let close_title = state
        .read()
        .pending_close
        .and_then(|index| {
            state
                .read()
                .tabs
                .get(index)
                .map(|tab| tab.file_name.clone())
        })
        .unwrap_or_default();
    rsx! {
        div { class: "modal-backdrop", onclick: move |_| state.write().modal = None,
            section {
                class: if is_settings {
                    "modal rton-settings-dialog"
                } else if is_shortcuts {
                    "modal rton-shortcuts-dialog"
                } else if is_logs {
                    "modal rton-logs-dialog"
                } else {
                    "modal"
                },
                onclick: move |event| event.stop_propagation(),
                if !is_settings && !is_shortcuts {
                    button { class: "modal-close", title: t.get("common_close"), onclick: move |_| state.write().modal = None, {icon(LdX)} }
                }
                match modal {
                    Modal::Shortcuts => rsx! {
                        header { class: "rton-shortcuts-header",
                            div { class: "rton-shortcuts-title-row",
                                span { class: "rton-shortcuts-title-icon", {icon(LdKeyboard)} }
                                h2 { {t.get("win_shortcuts")} }
                            }
                            button {
                                class: "rton-shortcuts-close",
                                title: t.get("common_close"),
                                aria_label: t.get("common_close"),
                                onclick: move |_| state.write().modal = None,
                                {icon(LdX)}
                            }
                        }
                        div { class: "rton-shortcuts-content",
                            ShortcutSection {
                                title: t.get("shortcuts_section_keyboard"),
                                shortcuts: EDITING_SHORTCUTS,
                            }
                            ShortcutSection {
                                title: t.get("shortcuts_section_tools"),
                                shortcuts: TOOL_SHORTCUTS,
                            }
                            ShortcutSection {
                                title: t.get("menu_view"),
                                shortcuts: VIEW_SHORTCUTS,
                            }
                        }
                    },
                    Modal::AddObject => rsx! {
                        h2 { {t.get("win_add_object")} }
                        div { class: "add-object-form",
                            label { class: "property-field",
                                span { {t.get("add_name")} }
                                input { value: state.read().add_object.name.clone(), oninput: move |event| state.write().add_object.name = event.value() }
                            }
                            label { class: "property-field",
                                span { {t.get("add_prefab_index")} }
                                input { r#type: "number", value: "{state.read().add_object.prefab_index}", oninput: move |event| if let Ok(value) = event.value().parse() { state.write().add_object.prefab_index = value; } }
                            }
                            div { class: "property-field",
                                span { {t.get("add_data_type")} }
                                CustomSelect {
                                    aria_label: t.get("add_data_type"),
                                    value: match state.read().add_object.data_type {
                                        DataType::Terrain => "terrain",
                                        DataType::PrefabOverrides => "overrides",
                                        DataType::None => "none",
                                    }.to_string(),
                                    options: vec![
                                        SelectOption::new("none", t.get("add_data_type_none")),
                                        SelectOption::new("terrain", t.get("add_data_type_terrain")),
                                        SelectOption::new("overrides", t.get("add_data_type_prefab_overrides")),
                                    ],
                                    onchange: move |value: String| state.write().set_add_object_data_type(match value.as_str() {
                                        "terrain" => DataType::Terrain,
                                        "overrides" => DataType::PrefabOverrides,
                                        _ => DataType::None,
                                    }),
                                }
                            }
                            AddObjectVector { label: t.get("prop_position"), kind: "position" }
                            AddObjectVector { label: t.get("prop_rotation"), kind: "rotation" }
                            AddObjectVector { label: t.get("prop_scale"), kind: "scale" }
                            if state.read().add_object.data_type == DataType::Terrain {
                                label { class: "property-toggle",
                                    span { {t.get("prop_collider")} }
                                    input { r#type: "checkbox", checked: state.read().add_object.terrain_has_collider, onchange: move |event| state.write().set_add_object_terrain_has_collider(event.checked()) }
                                }
                            }
                        }
                        div { class: "modal-actions add-object-actions",
                            button { onclick: move |_| state.write().add_parent(), {t.get("btn_add_group")} }
                            button { onclick: move |_| state.write().add_terrain(), {t.get("btn_add_terrain")} }
                            button { class: "primary", onclick: move |_| state.write().add_prefab(), {t.get("btn_add_prefab")} }
                        }
                    },
                    Modal::DeleteConfirm => rsx! {
                        h2 { {t.get("win_confirm_delete")} }
                        p { {t.get("delete_objects_description")} }
                        ul { class: "confirm-object-list",
                            for name in delete_names { li { "{name}" } }
                        }
                        div { class: "modal-actions",
                            button { onclick: move |_| state.write().cancel_delete(), {t.get("btn_cancel")} }
                            button { class: "danger", onclick: move |_| state.write().confirm_delete(), {t.get("menu_delete")} }
                        }
                    },
                    Modal::LevelWarnings => rsx! {
                        h2 { {t.get("win_level_warning")} }
                        p { {t.get("level_warning_description")} }
                        div { class: "level-warning-list",
                            for warning in warnings {
                                div { class: if warning.severity() == LevelWarningSeverity::High { "level-warning-row high" } else { "level-warning-row low" },
                                    strong { {if warning.severity() == LevelWarningSeverity::High { t.get("level_warning_section_high") } else { t.get("level_warning_section_low") }} }
                                    span { {t.format("level_warning_found", &[("name", warning.object_name.to_string()), ("count", warning.count.to_string())])} }
                                }
                            }
                        }
                        div { class: "modal-actions",
                            button { onclick: move |_| { state.write().pending_preview_state = None; state.write().modal = None; }, {t.get("btn_cancel")} }
                            button { onclick: move |_| state.write().confirm_level_warnings(), {t.get("btn_i_understand_the_risks")} }
                        }
                    },
                    Modal::Unity3d => rsx! {
                        if let Some((bundle_name, mode, source, entries, selected)) = unity_bundle {
                            h2 {{
                                if mode == UnityBundleMode::ReplaceLevel && source == UnityAssetSource::SerializedFile {
                                    t.get("unity_assets_replace_title")
                                } else if mode == UnityBundleMode::ReplaceLevel {
                                    t.get("unity_replace_title")
                                } else if source == UnityAssetSource::SerializedFile {
                                    t.get("unity_assets_extract_title")
                                } else {
                                    t.get("unity_extract_title")
                                }
                            }}
                            div { class: "unity-entry-header",
                                p { class: "modal-description", title: "{bundle_name}", "{bundle_name}" }
                                if mode == UnityBundleMode::ExtractLevels {
                                    div { class: "unity-entry-toolbar",
                                    button {
                                        class: "rton-button secondary",
                                        onclick: move |_| {
                                            if let Some(bundle) = state.write().unity_bundle.as_mut() {
                                                bundle.selected = (0..bundle.entries.len()).collect();
                                            }
                                        },
                                        span { class: "button-icon", {icon(LdCheckCheck)} }
                                        {t.get("btn_select_all")}
                                    }
                                    button {
                                        class: "rton-button secondary",
                                        disabled: selected.is_empty(),
                                        onclick: move |_| {
                                            if let Some(bundle) = state.write().unity_bundle.as_mut() {
                                                bundle.selected.clear();
                                            }
                                        },
                                        span { class: "button-icon", {icon(LdSquare)} }
                                        {t.get("btn_clear_all")}
                                    }
                                }
                                }
                            }
                            div { class: "unity-entry-list",
                                for (entry_index, entry) in entries.into_iter().enumerate() {
                                    label { class: if selected.contains(&entry_index) { "unity-entry selected" } else { "unity-entry" },
                                        input {
                                            r#type: if mode == UnityBundleMode::ReplaceLevel { "radio" } else { "checkbox" },
                                            name: "unity-entry",
                                            checked: selected.contains(&entry_index),
                                            onchange: move |event| {
                                                let mut editor = state.write();
                                                if let Some(bundle) = editor.unity_bundle.as_mut() {
                                                    if mode == UnityBundleMode::ReplaceLevel {
                                                        bundle.selected.clear();
                                                        bundle.selected.insert(entry_index);
                                                    } else if event.checked() {
                                                        bundle.selected.insert(entry_index);
                                                    } else {
                                                        bundle.selected.remove(&entry_index);
                                                    }
                                                }
                                            }
                                        }
                                        span {
                                            strong {
                                                {badpiggies_editor_core::data::level_db::level_display_name_for_filename(&entry.display_name)
                                                    .unwrap_or_else(|| entry.display_name.clone())}
                                            }
                                            small { "{entry.asset_path}" }
                                        }
                                    }
                                }
                            }
                            div { class: "modal-actions",
                                button { onclick: move |_| { state.write().unity_bundle = None; state.write().modal = None; }, {t.get("btn_cancel")} }
                                if mode == UnityBundleMode::ExtractLevels {
                                    button { disabled: selected.is_empty(), class: "primary", onclick: move |_| files::extract_unity_levels(state), {t.get("unity_extract_selected")} }
                                } else {
                                    button { disabled: selected.len() != 1, class: "primary", onclick: move |_| files::replace_unity_level(state), {t.get("unity_replace_export")} }
                                }
                            }
                        }
                    },
                    Modal::Logs => rsx! { LogsDialog {} },
                    Modal::CloseConfirm => rsx! {
                        h2 { {t.get("close_unsaved_title")} }
                        p { {t.format("close_unsaved_description", &[("name", close_title)])} }
                        div { class: "modal-actions",
                            button { onclick: move |_| { state.write().pending_close = None; state.write().modal = None; }, {t.get("btn_cancel")} }
                            button { class: "danger", onclick: move |_| state.write().confirm_close_tab(), {t.get("close_discard")} }
                        }
                    },
                    Modal::Settings => rsx! {
                        header { class: "rton-settings-header",
                            div { class: "rton-settings-title-row",
                                span { class: "rton-settings-title-icon", {icon(LdSettings)} }
                                h2 { {t.get("settings_title")} }
                            }
                            button {
                                class: "rton-settings-close",
                                title: t.get("common_close"),
                                aria_label: t.get("common_close"),
                                onclick: move |_| state.write().modal = None,
                                {icon(LdX)}
                            }
                        }
                        div { class: "rton-settings-content",
                            section { class: "rton-settings-section",
                                div { class: "rton-settings-section-heading",
                                    span { class: "rton-settings-section-icon", {icon(LdMonitor)} }
                                    span { {t.get("settings_appearance")} }
                                }
                                div { class: "rton-settings-theme-segments",
                                    for option in ThemePreference::ALL {
                                        button {
                                            key: "{option.code()}",
                                            class: if option == theme { "active" } else { "" },
                                            aria_pressed: option == theme,
                                            onclick: move |_| {
                                                let saved = platform::save_theme_preference(option);
                                                let mut editor = state.write();
                                                editor.theme = option;
                                                if let Err(error) = saved {
                                                    editor.active_mut().status = t.format("status_theme_save_error", &[("error", error.to_string())]);
                                                }
                                            },
                                            span { class: "rton-settings-theme-icon", {theme_icon(option)} }
                                            span { {t.get(theme_label_key(option))} }
                                        }
                                    }
                                }
                            }
                            section { class: "rton-settings-section",
                                div { class: "rton-settings-section-heading",
                                    span { class: "rton-settings-section-icon", {icon(LdLanguages)} }
                                    span { {t.get("settings_language")} }
                                }
                                CustomSelect {
                                    class: "rton-settings-select".to_string(),
                                    aria_label: t.get("settings_language"),
                                    value: state.read().language.display_name().to_string(),
                                    options: Language::all()
                                        .iter()
                                        .map(|language| SelectOption::new(language.display_name(), language.display_name()))
                                        .collect(),
                                    onchange: move |value: String| {
                                        if let Some(language) = Language::all()
                                            .iter()
                                            .copied()
                                            .find(|language| language.display_name() == value)
                                        {
                                            state.write().language = language;
                                        }
                                    },
                                }
                            }
                            section { class: "rton-settings-section rton-settings-logs-section",
                                div { class: "rton-settings-section-heading",
                                    span { class: "rton-settings-section-icon", {icon(LdScrollText)} }
                                    span { {t.get("settings_logs")} }
                                }
                                button {
                                    class: "rton-settings-action-button",
                                    onclick: move |_| state.write().modal = Some(Modal::Logs),
                                    {icon(LdScrollText)}
                                    span { {t.get("settings_logs_open")} }
                                }
                            }
                            section { class: "rton-settings-section rton-settings-about-section",
                                div { class: "rton-settings-section-heading",
                                    span { class: "rton-settings-section-icon", {icon(LdInfo)} }
                                    span { {t.get("settings_about")} }
                                }
                                dl { class: "rton-settings-about-list",
                                    SettingsAboutItem {
                                        label: t.get("about_version"),
                                        value: env!("CARGO_PKG_VERSION").to_string(),
                                    }
                                    SettingsAboutItem {
                                        label: t.get("about_license_label"),
                                        value: env!("CARGO_PKG_LICENSE").to_string(),
                                    }
                                    div { class: "rton-settings-about-item",
                                        dt { {t.get("about_author")} }
                                        dd {
                                            a {
                                                href: AUTHOR_URL,
                                                target: "_blank",
                                                rel: "noopener noreferrer",
                                                "{AUTHOR_NAME}"
                                            }
                                        }
                                    }
                                }
                                a {
                                    class: "rton-settings-github-link",
                                    href: GITHUB_URL,
                                    target: "_blank",
                                    rel: "noopener noreferrer",
                                    title: t.get("about_github"),
                                    {icon(LdGithub)}
                                    span { {t.get("about_github")} }
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
fn ShortcutSection(title: String, shortcuts: &'static [ShortcutSpec]) -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    rsx! {
        section { class: "rton-shortcut-section",
            h3 { "{title}" }
            div { class: "rton-shortcut-list",
                for shortcut in shortcuts {
                    div { class: "rton-shortcut-row", key: "{shortcut.label_key}",
                        span { class: "rton-shortcut-action", {t.get(shortcut.label_key)} }
                        span { class: "rton-shortcut-combo",
                            for (key_index, key_label) in shortcut.keys.iter().enumerate() {
                                if key_index > 0 {
                                    span { class: "rton-shortcut-joiner", "+" }
                                }
                                kbd { class: "rton-shortcut-key", "{key_label}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LogsDialog() -> Element {
    let state = consume_context::<Signal<EditorState>>();
    let t = state.read().t();
    let mut logs = use_signal(platform::log_buffer::snapshot);
    let log_text = logs.read().clone();

    use_future(move || async move {
        loop {
            platform::sleep_ms(500).await;
            let next = platform::log_buffer::snapshot();
            let changed = logs.peek().as_str() != next.as_str();
            if changed {
                logs.set(next);
            }
        }
    });

    rsx! {
        h2 { {t.get("settings_logs")} }
        textarea {
            class: "log-viewer",
            readonly: true,
            value: "{log_text}",
            spellcheck: "false",
            wrap: "off",
            aria_label: t.get("settings_logs"),
            onfocus: move |_| logs.set(platform::log_buffer::snapshot()),
        }
        div { class: "modal-actions",
            button {
                onclick: move |_| {
                    platform::log_buffer::clear();
                    logs.set(String::new());
                },
                {t.get("logs_clear")}
            }
            button {
                class: "primary",
                onclick: move |_| files::export_logs(state),
                {t.get("menu_export_log")}
            }
        }
    }
}

#[component]
fn SettingsAboutItem(label: String, value: String) -> Element {
    rsx! {
        div { class: "rton-settings-about-item",
            dt { "{label}" }
            dd { "{value}" }
        }
    }
}
