use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use serde::Deserialize;

use crate::components::context_menu_transition::ContextMenuTransition;
use crate::editor_state::{CameraState, CanvasPointerState, CursorModeState, EditorState, Modal};
use badpiggies_editor_core::domain::types::Vec2;

const CANVAS_RUNTIME: &str = include_str!("../../assets/editor_canvas.js");

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CanvasMessage {
    Ready,
    Selection {
        indices: Vec<usize>,
    },
    MoveObjects {
        anchor_index: usize,
        dx: f32,
        dy: f32,
    },
    RotateObjects {
        anchor_index: usize,
        degrees: f32,
    },
    ScaleObject {
        index: usize,
        x: f32,
        y: f32,
    },
    TerrainNodeMove {
        object_index: usize,
        node_index: usize,
        x: f32,
        y: f32,
    },
    TerrainNodeEdit {
        action: String,
        object_index: usize,
        node_index: usize,
        x: Option<f32>,
        y: Option<f32>,
    },
    DrawTerrain {
        points: Vec<Vec2>,
        closed: bool,
        texture_index: usize,
        has_collider: bool,
    },
    BoundsChanged {
        target: String,
        bounds: [f32; 4],
    },
    RouteNodeChanged {
        index: usize,
        x: f32,
        y: f32,
    },
    ContextAction {
        action: String,
        indices: Vec<usize>,
        x: Option<f32>,
        y: Option<f32>,
    },
    ContextMenu {
        screen_x: f32,
        screen_y: f32,
        indices: Vec<usize>,
        has_node: bool,
        can_delete_node: bool,
        can_flip: bool,
        has_clipboard: bool,
    },
    Camera {
        x: f32,
        y: f32,
        zoom: f32,
    },
    PointerWorld {
        x: Option<f32>,
        y: Option<f32>,
    },
    Command {
        name: String,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, PartialEq)]
struct CanvasContextMenu {
    x: f32,
    y: f32,
    indices: Vec<usize>,
    has_node: bool,
    can_delete_node: bool,
    can_flip: bool,
    has_clipboard: bool,
}

fn invoke_context_action(action: &'static str, menu: ContextMenuTransition<CanvasContextMenu>) {
    menu.dismiss();
    spawn(async move {
        let script =
            format!("window.bpEditorCanvas && window.bpEditorCanvas.command('context:{action}');");
        let _ = document::eval(&script).await;
    });
}

#[component]
pub fn EditorCanvas() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let mut pointer_state = consume_context::<Signal<CanvasPointerState>>();
    use_drop(move || pointer_state.write().world = None);
    let context_menu = ContextMenuTransition::new(
        use_signal(|| None::<CanvasContextMenu>),
        use_signal(|| false),
        use_signal(|| 0_u64),
    );
    let sent_scene = use_hook(|| Rc::new(RefCell::new(None::<(String, u64)>)));

    let start_canvas = move |_| {
        let asset_root = serde_json::to_string(&super::APP_ASSETS.to_string())
            .unwrap_or_else(|_| "\"/assets\"".to_string());
        let runtime = CANVAS_RUNTIME.replace("__BP_ASSET_ROOT__", &asset_root);
        let mut evaluator = document::eval(&runtime);
        spawn(async move {
            while let Ok(message) = evaluator.recv::<CanvasMessage>().await {
                match message {
                    CanvasMessage::Ready => log::info!("Renderer is ready"),
                    CanvasMessage::Selection { indices } => state.write().set_selection(indices),
                    CanvasMessage::MoveObjects {
                        anchor_index,
                        dx,
                        dy,
                    } => state.write().move_objects(anchor_index, dx, dy),
                    CanvasMessage::RotateObjects {
                        anchor_index,
                        degrees,
                    } => state.write().rotate_objects(anchor_index, degrees),
                    CanvasMessage::ScaleObject { index, x, y } => {
                        state.write().scale_object(index, x, y);
                    }
                    CanvasMessage::TerrainNodeMove {
                        object_index,
                        node_index,
                        x,
                        y,
                    } => state
                        .write()
                        .move_terrain_node(object_index, node_index, Vec2 { x, y }),
                    CanvasMessage::TerrainNodeEdit {
                        action,
                        object_index,
                        node_index,
                        x,
                        y,
                    } => state.write().edit_terrain_node(
                        &action,
                        object_index,
                        node_index,
                        x.zip(y).map(|(x, y)| Vec2 { x, y }),
                    ),
                    CanvasMessage::DrawTerrain {
                        points,
                        closed,
                        texture_index,
                        has_collider,
                    } => state
                        .write()
                        .draw_terrain(points, closed, texture_index, has_collider),
                    CanvasMessage::BoundsChanged { target, bounds } => {
                        state.write().update_bounds(&target, bounds);
                    }
                    CanvasMessage::RouteNodeChanged { index, x, y } => {
                        state.write().update_route_node(index, Vec2 { x, y });
                    }
                    CanvasMessage::ContextAction {
                        action,
                        indices,
                        x,
                        y,
                    } => {
                        let mut editor = state.write();
                        if !indices.is_empty() {
                            editor.set_selection(indices.clone());
                        }
                        match action.as_str() {
                            "copy" => editor.copy_selected(),
                            "cut" => editor.cut_selected(),
                            "paste" => editor.paste(),
                            "duplicate" => editor.duplicate_selected(),
                            "delete" => editor.request_delete_selected(),
                            "flip_horizontal" => editor.flip_objects(&indices, true),
                            "flip_vertical" => editor.flip_objects(&indices, false),
                            "add" => {
                                editor.modal = Some(Modal::AddObject);
                                if let Some((x, y)) = x.zip(y) {
                                    editor.add_object.position.x = x;
                                    editor.add_object.position.y = y;
                                }
                            }
                            _ => {}
                        }
                    }
                    CanvasMessage::ContextMenu {
                        screen_x,
                        screen_y,
                        indices,
                        has_node,
                        can_delete_node,
                        can_flip,
                        has_clipboard,
                    } => {
                        context_menu.show(CanvasContextMenu {
                            x: screen_x,
                            y: screen_y,
                            indices,
                            has_node,
                            can_delete_node,
                            can_flip,
                            has_clipboard,
                        });
                    }
                    CanvasMessage::Camera { x, y, zoom } => {
                        state.write().camera = CameraState { x, y, zoom };
                    }
                    CanvasMessage::PointerWorld { x, y } => {
                        pointer_state.write().world = x.zip(y).map(|(x, y)| Vec2 { x, y });
                    }
                    CanvasMessage::Command { name } => {
                        let mut state = state.write();
                        match name.as_str() {
                            "undo" => state.undo(),
                            "redo" => state.redo(),
                            "copy" => state.copy_selected(),
                            "cut" => state.cut_selected(),
                            "paste" => state.paste(),
                            "duplicate" => state.duplicate_selected(),
                            "delete" => state.request_delete_selected(),
                            "select_all" => state.select_all(),
                            "deselect" => state.clear_selection(),
                            "fit" => state.fit_view(),
                            "tool_select" => state.cursor_mode = CursorModeState::Select,
                            "tool_box_select" => state.cursor_mode = CursorModeState::BoxSelect,
                            "tool_draw_terrain" => {
                                state.cursor_mode = CursorModeState::DrawTerrain;
                            }
                            "tool_pan" => state.cursor_mode = CursorModeState::Pan,
                            _ => {}
                        }
                    }
                    CanvasMessage::Error { message } => {
                        log::error!("Renderer error: {message}");
                        state.write().active_mut().status = message;
                    }
                }
            }
        });
    };

    use_effect(move || {
        let editor = state.read();
        let identity = editor.canvas_scene_identity();
        let full_scene = sent_scene.borrow().as_ref() != Some(&identity);
        let payload = if full_scene {
            *sent_scene.borrow_mut() = Some(identity);
            serde_json::to_string(&editor.canvas_scene())
        } else {
            serde_json::to_string(&editor.canvas_view())
        }
        .unwrap_or_default();
        let method = if full_scene { "render" } else { "renderView" };
        drop(editor);
        spawn(async move {
            let script =
                format!("window.bpEditorCanvas && window.bpEditorCanvas.{method}({payload});");
            let _ = document::eval(&script).await;
        });
    });
    let t = state.read().t();

    rsx! {
        div {
            class: "canvas-host",
            onclick: move |_| context_menu.dismiss(),
            oncontextmenu: move |event| event.prevent_default(),
            canvas {
                id: "editor-canvas",
                tabindex: "0",
                aria_label: t.get("aria_level_canvas"),
                onmounted: start_canvas,
            }
            if let Some(menu) = context_menu.value() {
                div {
                    class: if context_menu.is_closing() { "canvas-context-menu closing" } else { "canvas-context-menu" },
                    style: "left: max(4px, min(calc(100% - 190px), {menu.x}px)); top: max(4px, min(calc(100% - 330px), {menu.y}px));",
                    onclick: move |event| event.stop_propagation(),
                    oncontextmenu: move |event| {
                        event.prevent_default();
                        event.stop_propagation();
                    },
                    button {
                        disabled: menu.indices.is_empty(),
                        onclick: move |_| invoke_context_action("copy", context_menu),
                        {t.get("menu_copy")}
                    }
                    button {
                        disabled: menu.indices.is_empty(),
                        onclick: move |_| invoke_context_action("cut", context_menu),
                        {t.get("menu_cut")}
                    }
                    button {
                        disabled: !menu.has_clipboard,
                        onclick: move |_| invoke_context_action("paste", context_menu),
                        {t.get("menu_paste")}
                    }
                    hr {}
                    button {
                        onclick: move |_| invoke_context_action("add", context_menu),
                        {t.get("menu_add_object")}
                    }
                    button {
                        disabled: menu.indices.is_empty(),
                        onclick: move |_| invoke_context_action("duplicate", context_menu),
                        {t.get("menu_duplicate")}
                    }
                    button {
                        disabled: menu.indices.is_empty(),
                        onclick: move |_| invoke_context_action("delete", context_menu),
                        {t.get("menu_delete")}
                    }
                    hr {}
                    button {
                        disabled: !menu.can_flip,
                        onclick: move |_| invoke_context_action("flip_horizontal", context_menu),
                        {t.get("menu_flip_horizontal")}
                    }
                    button {
                        disabled: !menu.can_flip,
                        onclick: move |_| invoke_context_action("flip_vertical", context_menu),
                        {t.get("menu_flip_vertical")}
                    }
                    if menu.has_node {
                        hr {}
                        button {
                            onclick: move |_| invoke_context_action("toggle_node_texture", context_menu),
                            {t.get("context_toggle_node_texture")}
                        }
                        button {
                            disabled: !menu.can_delete_node,
                            onclick: move |_| invoke_context_action("delete_node", context_menu),
                            {t.get("context_delete_node")}
                        }
                    }
                    hr {}
                    button {
                        onclick: move |_| invoke_context_action("fit", context_menu),
                        {t.get("menu_fit_view")}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CanvasMessage;

    #[test]
    fn pointer_world_events_decode_positions_and_leave_state() {
        let position =
            serde_json::from_str::<CanvasMessage>(r#"{"type":"pointer_world","x":1.25,"y":-2.5}"#)
                .expect("pointer position event");
        assert!(matches!(
            position,
            CanvasMessage::PointerWorld {
                x: Some(1.25),
                y: Some(-2.5)
            }
        ));

        let leave =
            serde_json::from_str::<CanvasMessage>(r#"{"type":"pointer_world","x":null,"y":null}"#)
                .expect("pointer leave event");
        assert!(matches!(
            leave,
            CanvasMessage::PointerWorld { x: None, y: None }
        ));
    }
}
