use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use badpiggies_editor_core::data::runtime_assets::install_runtime_assets;
use badpiggies_editor_core::domain::types::{LevelData, ObjectIndex, Vec2};
use js_sys::{Function, Reflect, Uint8Array};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::JsFuture;

use crate::contraption_preview::{ContraptionPreview, ContraptionPreviewPayload};
use crate::gpu2d::{self, Key, PointerButton};
use crate::i18n::locale;
use crate::renderer::{
    BoundsEditTarget, CanvasContextAction, CursorMode, LevelRenderer, NodeEditAction,
    PreviewPlaybackState, TerrainDrawMode, TerrainPresetShape,
};

const RENDERER_ASSETS: &[&str] = &[
    "data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage",
    "locales/en-US.ftl",
    "shader/e2d__curve.wgsl",
    "shader/_custom__unlit_color_geometry__terrain_fill.wgsl",
    "shader/unlit__transparent_cutout__sprite.wgsl",
    "shader/_custom__unlit_colortransparent_geometry__sprite.wgsl",
    "shader/_custom__unlit_monochrome.wgsl",
    "shader/_custom__unlit_color_geometry.wgsl",
    "shader/_custom__unlit_colortransparent_geometry.wgsl",
    "shader/_custom__unlit_alpha8bit_color.wgsl",
    "shader/unlit__transparent.wgsl",
    "shader/unlit__transparent_cutout.wgsl",
    "shader/depth_mask__unlit_transparent_cg__runtime.wgsl",
    "shader/depth_mask__maskoverlay__runtime.wgsl",
    "shader/depth_mask__maskoverlaynv__runtime.wgsl",
];

#[derive(Deserialize)]
pub struct ScenePayload {
    document_key: String,
    revision: u64,
    file_name: String,
    level: Option<LevelData>,
    #[serde(flatten)]
    view: ViewPayload,
}

#[derive(Deserialize)]
pub struct ViewPayload {
    #[serde(default)]
    selected: Vec<ObjectIndex>,
    #[serde(default = "default_true")]
    grid: bool,
    #[serde(default = "default_true")]
    background: bool,
    #[serde(default = "default_true")]
    construction_grid: bool,
    #[serde(default = "default_true")]
    dark_overlay: bool,
    #[serde(default)]
    ground: bool,
    #[serde(default)]
    terrain_triangles: bool,
    #[serde(default)]
    preview_route: bool,
    #[serde(default)]
    cursor_mode: String,
    #[serde(default)]
    preview_state: String,
    #[serde(default = "default_true")]
    night_vision: bool,
    #[serde(default)]
    terrain_draw_mode: String,
    #[serde(default)]
    terrain_preset: Option<String>,
    #[serde(default = "default_curve_segments")]
    terrain_curve_segments: usize,
    #[serde(default = "default_texture_index")]
    terrain_texture_index: usize,
    #[serde(default = "default_true")]
    terrain_has_collider: bool,
    #[serde(default)]
    terrain_continuation_anchor: Option<Vec2>,
    #[serde(default)]
    has_clipboard: bool,
    #[serde(default)]
    camera_command: u64,
}

const fn default_true() -> bool {
    true
}

const fn default_curve_segments() -> usize {
    24
}

const fn default_texture_index() -> usize {
    1
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RendererEvent {
    Ready,
    Selection {
        indices: Vec<ObjectIndex>,
    },
    MoveObjects {
        anchor_index: ObjectIndex,
        dx: f32,
        dy: f32,
    },
    RotateObjects {
        anchor_index: ObjectIndex,
        degrees: f32,
    },
    ScaleObject {
        index: ObjectIndex,
        x: f32,
        y: f32,
    },
    TerrainNodeMove {
        object_index: ObjectIndex,
        node_index: usize,
        x: f32,
        y: f32,
    },
    TerrainNodeEdit {
        action: &'static str,
        object_index: ObjectIndex,
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
        target: &'static str,
        bounds: [f32; 4],
    },
    RouteNodeChanged {
        index: usize,
        x: f32,
        y: f32,
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
    ContextAction {
        action: &'static str,
        indices: Vec<ObjectIndex>,
        x: Option<f32>,
        y: Option<f32>,
    },
    ContextMenu {
        screen_x: f32,
        screen_y: f32,
        indices: Vec<ObjectIndex>,
        has_node: bool,
        can_delete_node: bool,
        can_flip: bool,
        has_clipboard: bool,
    },
}

pub struct RendererApp {
    renderer: LevelRenderer,
    contraption_preview: Option<ContraptionPreview>,
    level_key: Option<(String, u64)>,
    has_level: bool,
    selected: BTreeSet<ObjectIndex>,
    cursor_mode: CursorMode,
    has_clipboard: bool,
    camera_command: u64,
    last_emitted_camera: (f32, f32, f32),
    last_emitted_pointer_world: Option<(i64, i64)>,
}

impl RendererApp {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let renderer = LevelRenderer::new(device, queue, format);
        let camera = &renderer.camera;
        Self {
            last_emitted_camera: (camera.center.x, camera.center.y, camera.zoom),
            renderer,
            contraption_preview: None,
            level_key: None,
            has_level: false,
            selected: BTreeSet::new(),
            cursor_mode: CursorMode::Select,
            has_clipboard: false,
            camera_command: 0,
            last_emitted_pointer_world: None,
        }
    }

    fn apply_scene(&mut self, scene: ScenePayload) {
        self.contraption_preview = None;
        let same_document = self
            .level_key
            .as_ref()
            .is_some_and(|(document, _)| document == &scene.document_key);
        let next_key = (scene.document_key, scene.revision);
        let level_changed = self.level_key.as_ref() != Some(&next_key);
        if level_changed {
            self.renderer.set_level_key(&scene.file_name);
            if let Some(level) = scene.level.as_ref() {
                if same_document {
                    self.renderer.reload_level_preserving_preview_state(level);
                } else {
                    self.renderer.set_level(level);
                }
                self.has_level = true;
            } else {
                self.has_level = false;
            }
            self.level_key = Some(next_key);
        }

        self.apply_view(scene.view);
    }

    fn apply_view(&mut self, view: ViewPayload) {
        self.selected = view.selected.into_iter().collect();
        self.renderer.show_grid = view.grid;
        self.renderer.show_bg = view.background;
        self.renderer.show_grid_overlay = view.construction_grid;
        self.renderer.show_dark_overlay = view.dark_overlay;
        self.renderer.show_ground = view.ground;
        self.renderer.show_terrain_tris = view.terrain_triangles;
        self.renderer.show_preview_route = view.preview_route;
        self.cursor_mode = match view.cursor_mode.as_str() {
            "box_select" => CursorMode::BoxSelect,
            "draw_terrain" => CursorMode::DrawTerrain,
            "pan" => CursorMode::Pan,
            _ => CursorMode::Select,
        };
        self.renderer
            .set_preview_playback_state(match view.preview_state.as_str() {
                "build" => PreviewPlaybackState::Build,
                "pause" => PreviewPlaybackState::Pause,
                _ => PreviewPlaybackState::Play,
            });
        self.renderer.set_night_vision_enabled(view.night_vision);
        self.renderer
            .set_terrain_draw_mode(match view.terrain_draw_mode.as_str() {
                "curve" => TerrainDrawMode::Curve,
                "circular_arc" => TerrainDrawMode::CircularArc,
                "horizontal" => TerrainDrawMode::Horizontal,
                "vertical" => TerrainDrawMode::Vertical,
                _ => TerrainDrawMode::Free,
            });
        self.renderer
            .set_terrain_curve_segments(view.terrain_curve_segments);
        self.renderer
            .set_terrain_draw_texture_index(view.terrain_texture_index);
        self.renderer
            .set_terrain_draw_has_collider(view.terrain_has_collider);
        let requested_preset = view
            .terrain_preset
            .as_deref()
            .and_then(|preset| match preset {
                "circle" => Some(TerrainPresetShape::Circle),
                "perfect_circle" => Some(TerrainPresetShape::PerfectCircle),
                "rectangle" => Some(TerrainPresetShape::Rectangle),
                "square" => Some(TerrainPresetShape::Square),
                "equilateral_triangle" => Some(TerrainPresetShape::EquilateralTriangle),
                _ => None,
            });
        if self.renderer.active_terrain_preset() != requested_preset {
            if let Some(preset) = requested_preset {
                self.renderer.toggle_terrain_preset(preset);
            } else {
                self.renderer.clear_terrain_preset();
            }
        }
        if self.cursor_mode == CursorMode::DrawTerrain {
            if let Some(anchor) = view.terrain_continuation_anchor {
                self.renderer.prime_terrain_draw_anchor_if_idle(anchor);
            } else {
                self.renderer.clear_terrain_draw_anchor_if_idle();
            }
        }
        self.has_clipboard = view.has_clipboard;

        if view.camera_command > self.camera_command {
            self.renderer.fit_to_level();
            self.camera_command = view.camera_command;
        }
        self.emit_camera_if_changed();
    }

    fn set_contraption_preview(&mut self, payload: ContraptionPreviewPayload) {
        if let Some(preview) = self.contraption_preview.as_mut() {
            preview.update(payload);
        } else {
            self.contraption_preview = Some(ContraptionPreview::new(payload));
        }
        self.has_level = false;
        self.renderer.mouse_world = None;
    }

    fn emit_camera_if_changed(&mut self) {
        let camera = &self.renderer.camera;
        let next = (camera.center.x, camera.center.y, camera.zoom);
        if (next.0 - self.last_emitted_camera.0).abs() > 0.0001
            || (next.1 - self.last_emitted_camera.1).abs() > 0.0001
            || (next.2 - self.last_emitted_camera.2).abs() > 0.0001
        {
            self.last_emitted_camera = next;
            emit(&RendererEvent::Camera {
                x: next.0,
                y: next.1,
                zoom: next.2,
            });
        }
    }

    fn emit_pointer_world_if_changed(&mut self) {
        let pointer = self.renderer.mouse_world;
        let display_position = pointer.map(|position| {
            (
                (position.x * 100.0).round() as i64,
                (position.y * 100.0).round() as i64,
            )
        });
        if display_position == self.last_emitted_pointer_world {
            return;
        }
        self.last_emitted_pointer_world = display_position;
        emit(&RendererEvent::PointerWorld {
            x: pointer.map(|position| position.x),
            y: pointer.map(|position| position.y),
        });
    }

    fn emit_renderer_results(&mut self) {
        if let Some(request) = self.renderer.context_menu_request.take() {
            emit(&RendererEvent::ContextMenu {
                screen_x: request.screen_pos.x,
                screen_y: request.screen_pos.y,
                indices: request.indices,
                has_node: request.node.is_some(),
                can_delete_node: request.node.is_some_and(|(_, _, can_delete)| can_delete),
                can_flip: request.can_flip,
                has_clipboard: self.has_clipboard,
            });
        }
        if let Some(index) = self.renderer.context_selected_object.take()
            && !self.renderer.context_menu_just_opened
        {
            self.selected = BTreeSet::from([index]);
            self.emit_selection();
        }

        if let Some(index) = self.renderer.clicked_object {
            if self.renderer.clicked_with_cmd {
                if !self.selected.remove(&index) {
                    self.selected.insert(index);
                }
            } else {
                self.selected = BTreeSet::from([index]);
            }
            self.emit_selection();
        } else if self.renderer.clicked_empty && !self.renderer.clicked_with_cmd {
            self.selected.clear();
            self.emit_selection();
        }

        if let Some(result) = self.renderer.box_select_result.take() {
            self.selected = result.indices;
            self.emit_selection();
        }
        if let Some((anchor_index, delta)) = self.renderer.drag_result.take() {
            emit(&RendererEvent::MoveObjects {
                anchor_index,
                dx: delta.x,
                dy: delta.y,
            });
        }
        if let Some((anchor_index, degrees)) = self.renderer.rotation_drag_result.take() {
            emit(&RendererEvent::RotateObjects {
                anchor_index,
                degrees,
            });
        }
        if let Some((index, scale)) = self.renderer.scale_drag_result.take() {
            emit(&RendererEvent::ScaleObject {
                index,
                x: scale.x,
                y: scale.y,
            });
        }
        if let Some(result) = self.renderer.node_drag_result.take() {
            emit(&RendererEvent::TerrainNodeMove {
                object_index: result.object_index,
                node_index: result.node_index,
                x: result.new_local_pos.x,
                y: result.new_local_pos.y,
            });
        }
        if let Some(action) = self.renderer.node_edit_action.take() {
            let event = match action {
                NodeEditAction::Delete {
                    object_index,
                    node_index,
                } => RendererEvent::TerrainNodeEdit {
                    action: "delete",
                    object_index,
                    node_index,
                    x: None,
                    y: None,
                },
                NodeEditAction::Insert {
                    object_index,
                    after_node,
                    local_pos,
                } => RendererEvent::TerrainNodeEdit {
                    action: "insert",
                    object_index,
                    node_index: after_node,
                    x: Some(local_pos.x),
                    y: Some(local_pos.y),
                },
                NodeEditAction::ToggleTexture {
                    object_index,
                    node_index,
                } => RendererEvent::TerrainNodeEdit {
                    action: "toggle_texture",
                    object_index,
                    node_index,
                    x: None,
                    y: None,
                },
            };
            emit(&event);
        }
        if let Some(result) = self.renderer.draw_terrain_result.take() {
            let continuation_anchor = if result.closed {
                None
            } else {
                result.points.last().copied()
            };
            self.renderer
                .set_terrain_draw_continuation_anchor(continuation_anchor);
            emit(&RendererEvent::DrawTerrain {
                points: result.points,
                closed: result.closed,
                texture_index: result.texture_index,
                has_collider: self.renderer.terrain_draw_has_collider(),
            });
        }
        if let Some(result) = self.renderer.bounds_drag_result.take() {
            let target = match result.target {
                BoundsEditTarget::InitialView => "initial_view",
                BoundsEditTarget::CameraLimits => "camera_limits",
                BoundsEditTarget::ConstructionView => "construction_view",
            };
            emit(&RendererEvent::BoundsChanged {
                target,
                bounds: result.new_bounds,
            });
        }
        if let Some(result) = self.renderer.route_node_drag_result.take() {
            emit(&RendererEvent::RouteNodeChanged {
                index: result.index,
                x: result.new_position.x,
                y: result.new_position.y,
            });
        }
        if let Some(action) = self.renderer.context_action.take() {
            let event = match action {
                CanvasContextAction::Copy(indices) => context_event("copy", indices, None),
                CanvasContextAction::Cut(indices) => context_event("cut", indices, None),
                CanvasContextAction::AddObject { world_pos } => {
                    context_event("add", Vec::new(), world_pos)
                }
                CanvasContextAction::Paste {
                    context_indices,
                    world_pos,
                } => context_event("paste", context_indices, world_pos),
                CanvasContextAction::Duplicate(indices) => {
                    context_event("duplicate", indices, None)
                }
                CanvasContextAction::FlipHorizontal(indices) => {
                    context_event("flip_horizontal", indices, None)
                }
                CanvasContextAction::FlipVertical(indices) => {
                    context_event("flip_vertical", indices, None)
                }
                CanvasContextAction::Delete(indices) => context_event("delete", indices, None),
            };
            emit(&event);
        }
        self.emit_camera_if_changed();
    }

    fn emit_selection(&self) {
        emit(&RendererEvent::Selection {
            indices: self.selected.iter().copied().collect(),
        });
    }

    fn ui(&mut self, ui: &mut gpu2d::Ui) {
        if let Some(preview) = self.contraption_preview.as_mut() {
            preview.show(ui);
        } else if self.has_level {
            self.renderer.show(
                ui,
                &self.selected,
                self.cursor_mode,
                locale::english(),
                self.has_clipboard,
            );
            self.emit_renderer_results();
        } else {
            self.renderer.mouse_world = None;
            ui.painter().rect_filled(
                ui.max_rect(),
                0.0,
                crate::gpu2d::Color32::from_rgb(24, 27, 32),
            );
        }
        self.emit_pointer_world_if_changed();
    }
}

fn context_event(
    action: &'static str,
    indices: Vec<ObjectIndex>,
    world_pos: Option<Vec2>,
) -> RendererEvent {
    RendererEvent::ContextAction {
        action,
        indices,
        x: world_pos.map(|position| position.x),
        y: world_pos.map(|position| position.y),
    }
}

fn emit(event: &RendererEvent) {
    let Ok(value) = serde_wasm_bindgen::to_value(event) else {
        return;
    };
    let global = js_sys::global();
    let Ok(callback) = Reflect::get(&global, &JsValue::from_str("bpRendererEvent")) else {
        return;
    };
    if let Some(callback) = callback.dyn_ref::<Function>() {
        let _ = callback.call1(&JsValue::UNDEFINED, &value);
    }
}

#[derive(Default)]
struct RawInput {
    pointer_pos: Option<gpu2d::Pos2>,
    hovered: bool,
    drag_delta: gpu2d::Vec2,
    buttons_down: HashSet<PointerButton>,
    buttons_pressed: HashSet<PointerButton>,
    buttons_released: HashSet<PointerButton>,
    buttons_clicked: HashSet<PointerButton>,
    press_origins: HashMap<PointerButton, gpu2d::Pos2>,
    double_clicked: bool,
    scroll: gpu2d::Vec2,
    keys_pressed: HashSet<Key>,
    modifiers: gpu2d::Modifiers,
    multi_touch_zoom: Option<f32>,
    multi_touch_translation: gpu2d::Vec2,
}

impl RawInput {
    fn pointer_event(
        &mut self,
        kind: &str,
        x: f32,
        y: f32,
        button: i16,
        detail: i16,
        modifiers: gpu2d::Modifiers,
    ) {
        let position = gpu2d::pos2(x, y);
        if let Some(previous) = self.pointer_pos {
            self.drag_delta += position - previous;
        }
        self.pointer_pos = Some(position);
        self.modifiers = modifiers;
        let button = match button {
            0 => Some(PointerButton::Primary),
            1 => Some(PointerButton::Middle),
            2 => Some(PointerButton::Secondary),
            _ => None,
        };
        match (kind, button) {
            ("enter", _) | ("move", _) => self.hovered = true,
            ("leave", _) => self.hovered = false,
            ("down", Some(button)) => {
                self.hovered = true;
                self.buttons_down.insert(button);
                self.buttons_pressed.insert(button);
                self.press_origins.insert(button, position);
                if button == PointerButton::Primary && detail >= 2 {
                    self.double_clicked = true;
                }
            }
            ("up", Some(button)) => {
                self.buttons_down.remove(&button);
                self.buttons_released.insert(button);
                if self.press_origins.remove(&button).is_some_and(|origin| {
                    let delta = position - origin;
                    delta.length_sq() <= 36.0
                }) {
                    self.buttons_clicked.insert(button);
                }
            }
            ("cancel", Some(button)) => {
                self.buttons_down.remove(&button);
                self.buttons_released.insert(button);
                self.press_origins.remove(&button);
            }
            _ => {}
        }
    }

    fn frame(
        &mut self,
        width: u32,
        height: u32,
        stable_dt: f32,
    ) -> (gpu2d::InputState, gpu2d::Response) {
        let rect =
            gpu2d::Rect::from_min_size(gpu2d::Pos2::ZERO, gpu2d::vec2(width as f32, height as f32));
        let pointer_pos = self.pointer_pos.filter(|position| rect.contains(*position));
        let input = gpu2d::InputState {
            stable_dt,
            modifiers: self.modifiers,
            smooth_scroll_delta: self.scroll,
            pointer: gpu2d::PointerState {
                position: pointer_pos,
            },
            multi_touch_zoom: self.multi_touch_zoom,
            multi_touch_translation: self.multi_touch_translation,
            keys_pressed: std::mem::take(&mut self.keys_pressed),
        };
        let response = gpu2d::Response {
            rect,
            pointer_pos,
            hovered: self.hovered && pointer_pos.is_some(),
            drag_delta: self.drag_delta,
            buttons_down: self.buttons_down.clone(),
            buttons_pressed: std::mem::take(&mut self.buttons_pressed),
            buttons_released: std::mem::take(&mut self.buttons_released),
            buttons_clicked: std::mem::take(&mut self.buttons_clicked),
            double_clicked: std::mem::take(&mut self.double_clicked),
        };
        self.drag_delta = gpu2d::Vec2::ZERO;
        self.scroll = gpu2d::Vec2::ZERO;
        self.multi_touch_zoom = None;
        self.multi_touch_translation = gpu2d::Vec2::ZERO;
        (input, response)
    }
}

enum CanvasTarget {
    Html(web_sys::HtmlCanvasElement),
    Offscreen(web_sys::OffscreenCanvas),
}

impl CanvasTarget {
    fn render_size(&self) -> (u32, u32) {
        match self {
            Self::Html(canvas) => (
                canvas.client_width().max(1) as u32,
                canvas.client_height().max(1) as u32,
            ),
            Self::Offscreen(canvas) => (canvas.width().max(1), canvas.height().max(1)),
        }
    }

    fn set_size(&self, width: u32, height: u32) {
        match self {
            Self::Html(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
            Self::Offscreen(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
        }
    }
}

struct Runtime {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    canvas: CanvasTarget,
    context: gpu2d::Context,
    gpu2d_renderer: gpu2d::Renderer,
    app: RendererApp,
    input: RawInput,
    last_frame_ms: Option<f64>,
}

impl Runtime {
    fn resize(&mut self) -> bool {
        let (width, height) = self.canvas.render_size();
        if self.config.width == width && self.config.height == height {
            return false;
        }
        self.canvas.set_size(width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        true
    }

    fn resize_to(&mut self, width: u32, height: u32) {
        self.canvas.set_size(width.max(1), height.max(1));
        self.resize();
    }

    fn frame(&mut self, timestamp_ms: f64) -> &'static str {
        self.context.take_repaint_request();
        self.resize();
        let stable_dt = self
            .last_frame_ms
            .replace(timestamp_ms)
            .map(|previous| ((timestamp_ms - previous) / 1000.0) as f32)
            .unwrap_or(1.0 / 60.0)
            .clamp(1.0 / 240.0, 0.1);
        let (input, response) = self
            .input
            .frame(self.config.width, self.config.height, stable_dt);
        self.context.reset_cursor_icon();
        let mut ui = gpu2d::Ui::new(
            self.context.clone(),
            gpu2d::vec2(self.config.width as f32, self.config.height as f32),
            input,
            response,
        );
        self.app.ui(&mut ui);
        let commands = ui.take_commands();

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                self.context.request_repaint();
                return self.context.cursor_icon().css();
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.context.request_repaint();
                return self.context.cursor_icon().css();
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("wgpu surface frame failed validation");
                self.context.request_repaint();
                return self.context.cursor_icon().css();
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.gpu2d_renderer.render(
            &self.device,
            &self.queue,
            &view,
            &self.context,
            commands,
            self.config.width,
            self.config.height,
        );
        self.queue.present(frame);
        self.context.cursor_icon().css()
    }
}

#[wasm_bindgen]
pub struct RendererHandle {
    runtime: Rc<RefCell<Option<Runtime>>>,
    panicked: Rc<Cell<bool>>,
}

impl Default for RendererHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl RendererHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self {
            runtime: Rc::new(RefCell::new(None)),
            panicked: Rc::new(Cell::new(false)),
        }
    }

    pub async fn start(
        &self,
        canvas: web_sys::HtmlCanvasElement,
        asset_root: String,
    ) -> Result<(), JsValue> {
        *self.runtime.borrow_mut() =
            Some(create_runtime(CanvasTarget::Html(canvas), &asset_root).await?);
        emit(&RendererEvent::Ready);
        Ok(())
    }

    pub async fn start_offscreen(
        &self,
        canvas: web_sys::OffscreenCanvas,
        asset_root: String,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        canvas.set_width(width.max(1));
        canvas.set_height(height.max(1));
        *self.runtime.borrow_mut() =
            Some(create_runtime(CanvasTarget::Offscreen(canvas), &asset_root).await?);
        emit(&RendererEvent::Ready);
        Ok(())
    }

    pub fn set_scene(&self, scene: JsValue) -> Result<(), JsValue> {
        let scene = serde_wasm_bindgen::from_value(scene)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.app.apply_scene(scene);
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn warm_up(&self) {
        badpiggies_editor_core::data::prepare_renderer_assets();
    }

    pub fn set_view(&self, view: JsValue) -> Result<(), JsValue> {
        let view = serde_wasm_bindgen::from_value(view)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.app.apply_view(view);
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn set_contraption_preview(&self, preview: JsValue) -> Result<(), JsValue> {
        let preview = serde_wasm_bindgen::from_value(preview)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.app.set_contraption_preview(preview);
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.resize_to(width, height);
            runtime.context.request_repaint();
        }
    }

    pub fn font_backend(&self) -> String {
        self.runtime
            .borrow()
            .as_ref()
            .map(|runtime| runtime.context.font_backend())
            .unwrap_or("uninitialized")
            .to_string()
    }

    pub fn command(&self, command: &str) -> Result<(), JsValue> {
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        let app = &mut runtime.app;
        match command {
            "fit" => app.renderer.fit_to_level(),
            "build" => app
                .renderer
                .set_preview_playback_state(PreviewPlaybackState::Build),
            "play" => app
                .renderer
                .set_preview_playback_state(PreviewPlaybackState::Play),
            "pause" => app
                .renderer
                .set_preview_playback_state(PreviewPlaybackState::Pause),
            action if action.starts_with("context:") => {
                if !app.renderer.apply_context_menu_action(&action[8..]) {
                    return Err(JsValue::from_str("invalid context menu action"));
                }
                app.emit_renderer_results();
            }
            _ => return Err(JsValue::from_str("unknown renderer command")),
        }
        app.emit_camera_if_changed();
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn has_panicked(&self) -> bool {
        self.panicked.get()
    }

    pub fn destroy(&self) {
        *self.runtime.borrow_mut() = None;
    }

    pub fn frame(&self, timestamp_ms: f64) -> String {
        self.runtime
            .borrow_mut()
            .as_mut()
            .map(|runtime| runtime.frame(timestamp_ms).to_string())
            .unwrap_or_else(|| "default".to_string())
    }

    pub fn needs_repaint(&self) -> bool {
        self.runtime
            .borrow()
            .as_ref()
            .is_some_and(|runtime| runtime.context.repaint_requested())
    }

    pub fn frame_stats(&self) -> String {
        self.runtime
            .borrow()
            .as_ref()
            .and_then(|runtime| serde_json::to_string(&runtime.gpu2d_renderer.last_stats()).ok())
            .unwrap_or_else(|| "{}".to_string())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pointer_event(
        &self,
        kind: &str,
        x: f32,
        y: f32,
        button: i16,
        detail: i16,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.input.pointer_event(
                kind,
                x,
                y,
                button,
                detail,
                gpu2d::Modifiers {
                    alt,
                    ctrl,
                    shift,
                    command,
                },
            );
            runtime.context.request_repaint();
        }
    }

    pub fn wheel(&self, x: f32, y: f32) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.input.scroll += gpu2d::vec2(x, y);
            runtime.context.request_repaint();
        }
    }

    pub fn key(&self, key: &str, alt: bool, ctrl: bool, shift: bool, command: bool) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.input.modifiers = gpu2d::Modifiers {
                alt,
                ctrl,
                shift,
                command,
            };
            match key {
                "Enter" => {
                    runtime.input.keys_pressed.insert(Key::Enter);
                }
                "Escape" => {
                    runtime.input.keys_pressed.insert(Key::Escape);
                }
                "Delete" => {
                    runtime.input.keys_pressed.insert(Key::Delete);
                }
                "Backspace" => {
                    runtime.input.keys_pressed.insert(Key::Backspace);
                }
                _ => {}
            }
            runtime.context.request_repaint();
        }
    }

    pub fn touch_transform(&self, zoom_delta: f32, dx: f32, dy: f32) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.input.multi_touch_zoom = Some(zoom_delta);
            runtime.input.multi_touch_translation += gpu2d::vec2(dx, dy);
            runtime.context.request_repaint();
        }
    }
}

async fn create_runtime(canvas: CanvasTarget, asset_root: &str) -> Result<Runtime, JsValue> {
    preload_assets(asset_root).await?;
    let instance = wgpu::Instance::default();
    let surface = match &canvas {
        CanvasTarget::Html(canvas) => instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| {
                JsValue::from_str(&format!("failed to create canvas surface: {error}"))
            })?,
        CanvasTarget::Offscreen(canvas) => instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
            .map_err(|error| {
                JsValue::from_str(&format!("failed to create offscreen surface: {error}"))
            })?,
    };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        })
        .await
        .map_err(|error| JsValue::from_str(&format!("failed to request GPU adapter: {error}")))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("bad_piggies_editor_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|error| JsValue::from_str(&format!("failed to request GPU device: {error}")))?;
    let (width, height) = canvas.render_size();
    canvas.set_size(width, height);
    let mut config = surface
        .get_default_config(&adapter, width, height)
        .ok_or_else(|| JsValue::from_str("canvas surface is unsupported by the GPU adapter"))?;
    config.present_mode = wgpu::PresentMode::Fifo;
    config.desired_maximum_frame_latency = 2;
    surface.configure(&device, &config);
    let context = gpu2d::Context::new(&device, &queue);
    let gpu2d_renderer = gpu2d::Renderer::new(&device, config.format, &context);
    let app = RendererApp::new(&device, &queue, config.format);
    Ok(Runtime {
        _instance: instance,
        surface,
        device,
        queue,
        config,
        canvas,
        context,
        gpu2d_renderer,
        app,
        input: RawInput::default(),
        last_frame_ms: None,
    })
}

async fn preload_assets(asset_root: &str) -> Result<(), JsValue> {
    let missing =
        badpiggies_editor_core::data::runtime_assets::missing_runtime_assets(RENDERER_ASSETS);
    if missing.is_empty() {
        return Ok(());
    }
    let global = js_sys::global();
    let fetch = Reflect::get(&global, &JsValue::from_str("fetch"))?
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str("global fetch is unavailable"))?;
    let mut requests = Vec::with_capacity(missing.len());
    for relative in &missing {
        let url = format!("{}/{relative}", asset_root.trim_end_matches('/'));
        let promise = fetch
            .call1(&global, &JsValue::from_str(&url))?
            .dyn_into::<js_sys::Promise>()
            .map_err(|_| JsValue::from_str("fetch did not return a Promise"))?;
        requests.push((relative.clone(), url, promise));
    }

    let mut assets = Vec::with_capacity(requests.len());
    for (relative, url, promise) in requests {
        let response = JsFuture::from(promise).await?;
        let response: web_sys::Response = response
            .dyn_into()
            .map_err(|_| JsValue::from_str("invalid asset response"))?;
        if !response.ok() {
            return Err(JsValue::from_str(&format!(
                "failed to fetch {url}: HTTP {}",
                response.status()
            )));
        }
        let buffer = JsFuture::from(response.array_buffer()?).await?;
        assets.push((relative, Uint8Array::new(&buffer).to_vec()));
    }
    install_runtime_assets(assets);
    Ok(())
}
