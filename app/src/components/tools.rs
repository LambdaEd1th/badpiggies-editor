use badpiggies_editor_core::domain::level_warning::LevelWarningSeverity;
use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdBoxSelect, LdCircle, LdHammer, LdHand, LdMousePointer2, LdPause, LdPenTool, LdPlay,
    LdRectangleHorizontal, LdSquare, LdTriangle,
};
use dioxus_free_icons::{Icon, IconShape};

use crate::editor_state::{
    CursorModeState, EditorState, Modal, PreviewState, TerrainDrawModeState, TerrainPresetState,
};

#[component]
pub fn ToolPanel() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    if !state.read().show_tools {
        return rsx! {};
    }
    let cursor_mode = state.read().cursor_mode;
    let draw_mode = state.read().terrain_draw_mode;
    let preset = state.read().terrain_preset;
    let is_terrain = cursor_mode == CursorModeState::DrawTerrain;
    let t = state.read().t();
    rsx! {
        aside { class: "canvas-tool-panel", aria_label: t.get("tool_aria_editor"),
            div { class: "tool-button-row",
                ToolButton { label: format!("{} (V)", t.get("tool_select")), active: cursor_mode == CursorModeState::Select, icon: tool_icon(LdMousePointer2), onclick: move |_| state.write().cursor_mode = CursorModeState::Select }
                ToolButton { label: format!("{} (M)", t.get("tool_box_select")), active: cursor_mode == CursorModeState::BoxSelect, icon: tool_icon(LdBoxSelect), onclick: move |_| state.write().cursor_mode = CursorModeState::BoxSelect }
                ToolButton { label: format!("{} (P)", t.get("tool_draw_terrain")), active: cursor_mode == CursorModeState::DrawTerrain, icon: tool_icon(LdPenTool), onclick: move |_| state.write().cursor_mode = CursorModeState::DrawTerrain }
                ToolButton { label: format!("{} (H)", t.get("tool_pan")), active: cursor_mode == CursorModeState::Pan, icon: tool_icon(LdHand), onclick: move |_| state.write().cursor_mode = CursorModeState::Pan }
            }
            if is_terrain {
                div { class: "tool-panel-section",
                    span { class: "tool-panel-label", {t.get("tool_terrain_draw_mode")} }
                    div { class: "tool-segment-row",
                        for (mode, label) in [
                            (TerrainDrawModeState::Curve, t.get("tool_terrain_draw_mode_curve")),
                            (TerrainDrawModeState::CircularArc, t.get("tool_terrain_draw_mode_arc")),
                            (TerrainDrawModeState::Horizontal, t.get("tool_terrain_draw_mode_horizontal")),
                            (TerrainDrawModeState::Vertical, t.get("tool_terrain_draw_mode_vertical")),
                        ] {
                            button {
                                class: if draw_mode == mode { "tool-segment active" } else { "tool-segment" },
                                title: label,
                                onclick: move |_| {
                                    let mut editor = state.write();
                                    editor.terrain_draw_mode = if editor.terrain_draw_mode == mode { TerrainDrawModeState::Free } else { mode };
                                    if editor.terrain_draw_mode != TerrainDrawModeState::Free {
                                        editor.terrain_preset = None;
                                    }
                                },
                                "{label}"
                            }
                        }
                    }
                }
                div { class: "tool-panel-section",
                    span { class: "tool-panel-label", {t.get("tool_terrain_presets")} }
                    div { class: "tool-button-row presets",
                        PresetButton { label: t.get("tool_terrain_preset_circle"), active: preset == Some(TerrainPresetState::Circle), icon: preset_icon(LdCircle, "terrain-preset-icon ellipse"), preset: TerrainPresetState::Circle }
                        PresetButton { label: t.get("tool_terrain_preset_perfect_circle"), active: preset == Some(TerrainPresetState::PerfectCircle), icon: tool_icon(LdCircle), preset: TerrainPresetState::PerfectCircle }
                        PresetButton { label: t.get("tool_terrain_preset_rectangle"), active: preset == Some(TerrainPresetState::Rectangle), icon: preset_icon(LdRectangleHorizontal, "terrain-preset-icon rectangle"), preset: TerrainPresetState::Rectangle }
                        PresetButton { label: t.get("tool_terrain_preset_square"), active: preset == Some(TerrainPresetState::Square), icon: tool_icon(LdSquare), preset: TerrainPresetState::Square }
                        PresetButton { label: t.get("tool_terrain_preset_equilateral_triangle"), active: preset == Some(TerrainPresetState::EquilateralTriangle), icon: tool_icon(LdTriangle), preset: TerrainPresetState::EquilateralTriangle }
                    }
                }
                label { class: "tool-inline-field",
                    span { {t.get("tool_terrain_curve_segments")} }
                    input {
                        r#type: "number",
                        min: "3",
                        max: "128",
                        value: "{state.read().terrain_curve_segments}",
                        oninput: move |event| if let Ok(value) = event.value().parse::<usize>() {
                            state.write().terrain_curve_segments = value.clamp(3, 128);
                        }
                    }
                }
                div { class: "tool-inline-field",
                    span { {t.get("tool_terrain_draw_splat")} }
                    div { class: "tool-segment-row compact",
                        for texture in 0..=1usize {
                            button {
                                class: if state.read().terrain_texture_index == texture { "tool-segment active" } else { "tool-segment" },
                                onclick: move |_| state.write().terrain_texture_index = texture,
                                "{texture}"
                            }
                        }
                    }
                }
                label { class: "tool-check",
                    input {
                        r#type: "checkbox",
                        checked: state.read().terrain_has_collider,
                        onchange: move |event| state.write().terrain_has_collider = event.checked(),
                    }
                    span { {t.get("prop_collider")} }
                }
            }
        }
    }
}

#[component]
fn ToolButton(
    label: String,
    active: bool,
    icon: Element,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if active { "canvas-tool-button active" } else { "canvas-tool-button" },
            title: label.clone(),
            aria_label: label,
            onclick: move |event| onclick.call(event),
            {icon}
        }
    }
}

#[component]
fn PresetButton(label: String, active: bool, icon: Element, preset: TerrainPresetState) -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    rsx! {
        button {
            class: if active { "canvas-tool-button active" } else { "canvas-tool-button" },
            title: label.clone(),
            aria_label: label,
            onclick: move |_| {
                let mut editor = state.write();
                editor.terrain_preset = if editor.terrain_preset == Some(preset) { None } else { Some(preset) };
                editor.terrain_draw_mode = TerrainDrawModeState::Free;
            },
            {icon}
        }
    }
}

#[component]
pub fn PreviewControls() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    if !state.read().show_preview_controls {
        return rsx! {};
    }
    let preview = state.read().preview_state;
    let t = state.read().t();
    rsx! {
        aside { class: "preview-controls", aria_label: t.get("tool_preview_playback"),
            ToolButton { label: t.get("tool_preview_build"), active: preview == PreviewState::Build, icon: tool_icon(LdHammer), onclick: move |_| state.write().request_preview_state(PreviewState::Build) }
            ToolButton { label: t.get("tool_preview_play"), active: preview == PreviewState::Play, icon: tool_icon(LdPlay), onclick: move |_| state.write().request_preview_state(PreviewState::Play) }
            ToolButton { label: t.get("tool_preview_pause"), active: preview == PreviewState::Pause, icon: tool_icon(LdPause), onclick: move |_| state.write().request_preview_state(PreviewState::Pause) }
            label { class: "preview-night-vision", title: t.get("tool_preview_night_vision"),
                input {
                    r#type: "checkbox",
                    checked: state.read().night_vision,
                    onchange: move |event| state.write().night_vision = event.checked(),
                }
                span { {t.get("tool_preview_night_vision")} }
            }
        }
    }
}

#[component]
pub fn LevelWarningBadge() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let warnings = state.read().current_level_warnings();
    if warnings.is_empty() {
        return rsx! {};
    }
    let high = warnings
        .iter()
        .filter(|warning| warning.severity() == LevelWarningSeverity::High)
        .count();
    let t = state.read().t();
    let warning_count = t.format(
        "level_warning_count",
        &[("count", warnings.len().to_string())],
    );
    rsx! {
        button {
            class: if high > 0 { "level-warning-badge high" } else { "level-warning-badge" },
            title: t.get("level_warning_review"),
            onclick: move |_| {
                state.write().pending_preview_state = None;
                state.write().modal = Some(Modal::LevelWarnings);
            },
            "{warning_count}"
        }
    }
}

fn tool_icon<T>(shape: T) -> Element
where
    T: IconShape + Clone + PartialEq + 'static,
{
    rsx! {
        Icon { width: 17, height: 17, fill: "currentColor", icon: shape }
    }
}

fn preset_icon<T>(shape: T, class: &'static str) -> Element
where
    T: IconShape + Clone + PartialEq + 'static,
{
    rsx! {
        Icon { class, width: 17, height: 17, fill: "currentColor", icon: shape }
    }
}
