use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use serde::Deserialize;

use crate::components::context_menu_transition::ContextMenuTransition;
use crate::editor_state::{CameraState, CanvasPointerState, CursorModeState, EditorState, Modal};
use badpiggies_editor_core::domain::types::Vec2;

#[cfg(target_arch = "wasm32")]
const CANVAS_RUNTIME: &str = concat!(
    include_str!("../../assets/canvas_touch_navigation.js"),
    "\n",
    include_str!("../../assets/editor_canvas.js")
);
#[cfg(not(target_arch = "wasm32"))]
const CANVAS_RUNTIME: &str = concat!(
    include_str!("../../assets/canvas_touch_navigation.js"),
    "\n",
    include_str!("../../assets/native_canvas_host.js")
);

#[cfg(target_arch = "wasm32")]
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NativeCanvasMessage {
    Ready,
    Bounds {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        window_width: f32,
        window_height: f32,
    },
    Pointer {
        kind: String,
        x: f32,
        y: f32,
        button: i16,
        detail: i16,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    },
    Wheel {
        x: f32,
        y: f32,
    },
    Key {
        key: String,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    },
    TouchTransform {
        zoom: f32,
        dx: f32,
        dy: f32,
        x: f32,
        y: f32,
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

#[derive(Clone)]
enum CanvasCommandBridge {
    #[cfg(target_arch = "wasm32")]
    Web,
    #[cfg(not(target_arch = "wasm32"))]
    Native(crate::platform::native_renderer::NativeRendererContext),
}

fn invoke_context_action(
    action: &'static str,
    menu: ContextMenuTransition<CanvasContextMenu>,
    bridge: CanvasCommandBridge,
) {
    menu.dismiss();
    match bridge {
        #[cfg(target_arch = "wasm32")]
        CanvasCommandBridge::Web => {
            spawn(async move {
                let script = format!(
                    "window.bpEditorCanvas && window.bpEditorCanvas.command('context:{action}');"
                );
                let _ = document::eval(&script).await;
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        CanvasCommandBridge::Native(renderer) => {
            renderer.command(&format!("context:{action}"));
        }
    }
}

fn apply_editor_command(name: &str, mut state: Signal<EditorState>) {
    let mut state = state.write();
    match name {
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
        "tool_draw_terrain" => state.cursor_mode = CursorModeState::DrawTerrain,
        "tool_pan" => state.cursor_mode = CursorModeState::Pan,
        _ => {}
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_native_renderer_event(
    event: badpiggies_editor_renderer::RendererEvent,
    mut state: Signal<EditorState>,
    mut pointer_state: Signal<CanvasPointerState>,
    context_menu: ContextMenuTransition<CanvasContextMenu>,
) {
    use badpiggies_editor_renderer::RendererEvent;

    match event {
        RendererEvent::Ready => log::info!("Native renderer is ready"),
        RendererEvent::Selection { indices } => state.write().set_selection(indices),
        RendererEvent::MoveObjects {
            anchor_index,
            dx,
            dy,
        } => state.write().move_objects(anchor_index, dx, dy),
        RendererEvent::RotateObjects {
            anchor_index,
            degrees,
        } => state.write().rotate_objects(anchor_index, degrees),
        RendererEvent::ScaleObject { index, x, y } => {
            state.write().scale_object(index, x, y);
        }
        RendererEvent::TerrainNodeMove {
            object_index,
            node_index,
            x,
            y,
        } => state
            .write()
            .move_terrain_node(object_index, node_index, Vec2 { x, y }),
        RendererEvent::TerrainNodeEdit {
            action,
            object_index,
            node_index,
            x,
            y,
        } => state.write().edit_terrain_node(
            action,
            object_index,
            node_index,
            x.zip(y).map(|(x, y)| Vec2 { x, y }),
        ),
        RendererEvent::DrawTerrain {
            points,
            closed,
            texture_index,
            has_collider,
        } => state
            .write()
            .draw_terrain(points, closed, texture_index, has_collider),
        RendererEvent::BoundsChanged { target, bounds } => {
            state.write().update_bounds(target, bounds);
        }
        RendererEvent::RouteNodeChanged { index, x, y } => {
            state.write().update_route_node(index, Vec2 { x, y });
        }
        RendererEvent::ContextAction {
            action,
            indices,
            x,
            y,
        } => {
            let mut editor = state.write();
            if !indices.is_empty() {
                editor.set_selection(indices.clone());
            }
            match action {
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
        RendererEvent::ContextMenu {
            screen_x,
            screen_y,
            indices,
            has_node,
            can_delete_node,
            can_flip,
            has_clipboard,
        } => context_menu.show(CanvasContextMenu {
            x: screen_x,
            y: screen_y,
            indices,
            has_node,
            can_delete_node,
            can_flip,
            has_clipboard,
        }),
        RendererEvent::Camera { x, y, zoom } => {
            state.write().camera = CameraState { x, y, zoom };
        }
        RendererEvent::PointerWorld { x, y } => {
            pointer_state.write().world = x.zip(y).map(|(x, y)| Vec2 { x, y });
        }
    }
}

#[component]
pub fn EditorCanvas() -> Element {
    let mut state = consume_context::<Signal<EditorState>>();
    let mut pointer_state = consume_context::<Signal<CanvasPointerState>>();
    #[cfg(not(target_arch = "wasm32"))]
    let native_renderer =
        consume_context::<crate::platform::native_renderer::NativeRendererContext>();
    #[cfg(target_arch = "wasm32")]
    let command_bridge = CanvasCommandBridge::Web;
    #[cfg(not(target_arch = "wasm32"))]
    let command_bridge = CanvasCommandBridge::Native(native_renderer.clone());
    use_drop(move || pointer_state.write().world = None);
    let context_menu = ContextMenuTransition::new(
        use_signal(|| None::<CanvasContextMenu>),
        use_signal(|| false),
        use_signal(|| 0_u64),
    );
    let sent_scene = use_hook(|| Rc::new(RefCell::new(None::<(String, u64)>)));

    #[cfg(target_arch = "wasm32")]
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
                        apply_editor_command(&name, state);
                    }
                    CanvasMessage::Error { message } => {
                        log::error!("Renderer error: {message}");
                        state.write().active_mut().status = message;
                    }
                }
            }
        });
    };

    #[cfg(not(target_arch = "wasm32"))]
    let start_canvas = {
        let renderer = native_renderer.clone();
        move |_| {
            let mut evaluator = document::eval(CANVAS_RUNTIME);
            let renderer = renderer.clone();
            spawn(async move {
                while let Ok(message) = evaluator.recv::<NativeCanvasMessage>().await {
                    match message {
                        NativeCanvasMessage::Ready => {
                            log::info!("Native canvas host is ready");
                        }
                        NativeCanvasMessage::Bounds {
                            x,
                            y,
                            width,
                            height,
                            window_width,
                            window_height,
                        } => renderer.set_viewport(
                            x,
                            y,
                            width,
                            height,
                            window_width,
                            window_height,
                            24.0,
                        ),
                        NativeCanvasMessage::Pointer {
                            kind,
                            x,
                            y,
                            button,
                            detail,
                            alt,
                            ctrl,
                            shift,
                            command,
                        } => renderer
                            .pointer_event(&kind, x, y, button, detail, alt, ctrl, shift, command),
                        NativeCanvasMessage::Wheel { x, y } => renderer.wheel(x, y),
                        NativeCanvasMessage::Key {
                            key,
                            alt,
                            ctrl,
                            shift,
                            command,
                        } => renderer.key(&key, alt, ctrl, shift, command),
                        NativeCanvasMessage::TouchTransform { zoom, dx, dy, x, y } => {
                            renderer.touch_transform(zoom, dx, dy, x, y);
                        }
                        NativeCanvasMessage::Command { name } => {
                            apply_editor_command(&name, state);
                        }
                        NativeCanvasMessage::Error { message } => {
                            log::error!("Native canvas host error: {message}");
                            state.write().active_mut().status = message;
                        }
                    }
                }
            });
        }
    };

    #[cfg(target_arch = "wasm32")]
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

    #[cfg(not(target_arch = "wasm32"))]
    {
        let renderer = native_renderer.clone();
        use_effect(move || {
            let editor = state.read();
            let identity = editor.canvas_scene_identity();
            let full_scene = sent_scene.borrow().as_ref() != Some(&identity);
            if full_scene {
                *sent_scene.borrow_mut() = Some(identity);
                let scene = editor.canvas_scene().into_renderer_payload();
                drop(editor);
                renderer.set_scene(scene);
            } else {
                let view = editor.canvas_view().into_renderer_payload();
                drop(editor);
                renderer.set_view(view);
            }
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut renderer = native_renderer.clone();
        use_effect(move || {
            for event in renderer.take_events() {
                apply_native_renderer_event(event, state, pointer_state, context_menu);
            }
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let renderer = native_renderer.clone();
        use_effect(move || {
            let cursor = serde_json::to_string(&renderer.cursor())
                .unwrap_or_else(|_| "\"default\"".to_string());
            spawn(async move {
                let script =
                    format!("window.bpEditorCanvas && window.bpEditorCanvas.setCursor({cursor});");
                let _ = document::eval(&script).await;
            });
        });
    }
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
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("copy", context_menu, bridge.clone())
                        },
                        {t.get("menu_copy")}
                    }
                    button {
                        disabled: menu.indices.is_empty(),
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("cut", context_menu, bridge.clone())
                        },
                        {t.get("menu_cut")}
                    }
                    button {
                        disabled: !menu.has_clipboard,
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("paste", context_menu, bridge.clone())
                        },
                        {t.get("menu_paste")}
                    }
                    hr {}
                    button {
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("add", context_menu, bridge.clone())
                        },
                        {t.get("menu_add_object")}
                    }
                    button {
                        disabled: menu.indices.is_empty(),
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("duplicate", context_menu, bridge.clone())
                        },
                        {t.get("menu_duplicate")}
                    }
                    button {
                        disabled: menu.indices.is_empty(),
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("delete", context_menu, bridge.clone())
                        },
                        {t.get("menu_delete")}
                    }
                    hr {}
                    button {
                        disabled: !menu.can_flip,
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("flip_horizontal", context_menu, bridge.clone())
                        },
                        {t.get("menu_flip_horizontal")}
                    }
                    button {
                        disabled: !menu.can_flip,
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("flip_vertical", context_menu, bridge.clone())
                        },
                        {t.get("menu_flip_vertical")}
                    }
                    if menu.has_node {
                        hr {}
                        button {
                            onclick: {
                                let bridge = command_bridge.clone();
                                move |_| invoke_context_action("toggle_node_texture", context_menu, bridge.clone())
                            },
                            {t.get("context_toggle_node_texture")}
                        }
                        button {
                            disabled: !menu.can_delete_node,
                            onclick: {
                                let bridge = command_bridge.clone();
                                move |_| invoke_context_action("delete_node", context_menu, bridge.clone())
                            },
                            {t.get("context_delete_node")}
                        }
                    }
                    hr {}
                    button {
                        onclick: {
                            let bridge = command_bridge.clone();
                            move |_| invoke_context_action("fit", context_menu, bridge.clone())
                        },
                        {t.get("menu_fit_view")}
                    }
                }
            }
        }
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
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
