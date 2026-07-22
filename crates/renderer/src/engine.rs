use std::collections::{BTreeSet, HashMap, HashSet};

use badpiggies_editor_core::domain::types::{LevelData, ObjectIndex, Vec2};
use serde::{Deserialize, Serialize};

use crate::contraption_preview::{ContraptionPreview, ContraptionPreviewPayload};
use crate::gpu2d::{self, Key, PointerButton};
use crate::i18n::locale;
use crate::renderer::{
    BoundsEditTarget, CanvasContextAction, CursorMode, LevelRenderer, NodeEditAction,
    PreviewPlaybackState, TerrainDrawMode, TerrainPresetShape,
};

#[derive(Deserialize)]
pub struct ScenePayload {
    pub document_key: String,
    pub revision: u64,
    pub file_name: String,
    pub level: Option<LevelData>,
    #[serde(flatten)]
    pub view: ViewPayload,
}

#[derive(Deserialize)]
pub struct ViewPayload {
    #[serde(default)]
    pub selected: Vec<ObjectIndex>,
    #[serde(default = "default_true")]
    pub grid: bool,
    #[serde(default = "default_true")]
    pub background: bool,
    #[serde(default = "default_true")]
    pub construction_grid: bool,
    #[serde(default = "default_true")]
    pub dark_overlay: bool,
    #[serde(default)]
    pub ground: bool,
    #[serde(default)]
    pub terrain_triangles: bool,
    #[serde(default)]
    pub preview_route: bool,
    #[serde(default)]
    pub cursor_mode: String,
    #[serde(default)]
    pub preview_state: String,
    #[serde(default = "default_true")]
    pub night_vision: bool,
    #[serde(default)]
    pub terrain_draw_mode: String,
    #[serde(default)]
    pub terrain_preset: Option<String>,
    #[serde(default = "default_curve_segments")]
    pub terrain_curve_segments: usize,
    #[serde(default = "default_texture_index")]
    pub terrain_texture_index: usize,
    #[serde(default = "default_true")]
    pub terrain_has_collider: bool,
    #[serde(default)]
    pub terrain_continuation_anchor: Option<Vec2>,
    #[serde(default)]
    pub has_clipboard: bool,
    #[serde(default)]
    pub camera_command: u64,
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

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RendererEvent {
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

pub(crate) struct RendererApp {
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
    events: Vec<RendererEvent>,
}

impl RendererApp {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
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
            events: Vec::new(),
        }
    }

    pub(crate) fn apply_scene(&mut self, scene: ScenePayload) {
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

    pub(crate) fn apply_view(&mut self, view: ViewPayload) {
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

    pub(crate) fn set_contraption_preview(&mut self, payload: ContraptionPreviewPayload) {
        if let Some(preview) = self.contraption_preview.as_mut() {
            preview.update(payload);
        } else {
            self.contraption_preview = Some(ContraptionPreview::new(payload));
        }
        self.has_level = false;
        self.renderer.mouse_world = None;
    }

    pub(crate) fn command(&mut self, command: &str) -> Result<(), &'static str> {
        match command {
            "fit" => self.renderer.fit_to_level(),
            "build" => self
                .renderer
                .set_preview_playback_state(PreviewPlaybackState::Build),
            "play" => self
                .renderer
                .set_preview_playback_state(PreviewPlaybackState::Play),
            "pause" => self
                .renderer
                .set_preview_playback_state(PreviewPlaybackState::Pause),
            action if action.starts_with("context:") => {
                if !self.renderer.apply_context_menu_action(&action[8..]) {
                    return Err("invalid context menu action");
                }
                self.emit_renderer_results();
            }
            _ => return Err("unknown renderer command"),
        }
        self.emit_camera_if_changed();
        Ok(())
    }

    pub(crate) fn ready(&mut self) {
        self.events.push(RendererEvent::Ready);
    }

    pub(crate) fn drain_events(&mut self) -> Vec<RendererEvent> {
        std::mem::take(&mut self.events)
    }

    fn emit_camera_if_changed(&mut self) {
        let camera = &self.renderer.camera;
        let next = (camera.center.x, camera.center.y, camera.zoom);
        if (next.0 - self.last_emitted_camera.0).abs() > 0.0001
            || (next.1 - self.last_emitted_camera.1).abs() > 0.0001
            || (next.2 - self.last_emitted_camera.2).abs() > 0.0001
        {
            self.last_emitted_camera = next;
            self.events.push(RendererEvent::Camera {
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
        self.events.push(RendererEvent::PointerWorld {
            x: pointer.map(|position| position.x),
            y: pointer.map(|position| position.y),
        });
    }

    fn emit_renderer_results(&mut self) {
        if let Some(request) = self.renderer.context_menu_request.take() {
            self.events.push(RendererEvent::ContextMenu {
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
            self.events.push(RendererEvent::MoveObjects {
                anchor_index,
                dx: delta.x,
                dy: delta.y,
            });
        }
        if let Some((anchor_index, degrees)) = self.renderer.rotation_drag_result.take() {
            self.events.push(RendererEvent::RotateObjects {
                anchor_index,
                degrees,
            });
        }
        if let Some((index, scale)) = self.renderer.scale_drag_result.take() {
            self.events.push(RendererEvent::ScaleObject {
                index,
                x: scale.x,
                y: scale.y,
            });
        }
        if let Some(result) = self.renderer.node_drag_result.take() {
            self.events.push(RendererEvent::TerrainNodeMove {
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
            self.events.push(event);
        }
        if let Some(result) = self.renderer.draw_terrain_result.take() {
            let continuation_anchor = if result.closed {
                None
            } else {
                result.points.last().copied()
            };
            self.renderer
                .set_terrain_draw_continuation_anchor(continuation_anchor);
            self.events.push(RendererEvent::DrawTerrain {
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
            self.events.push(RendererEvent::BoundsChanged {
                target,
                bounds: result.new_bounds,
            });
        }
        if let Some(result) = self.renderer.route_node_drag_result.take() {
            self.events.push(RendererEvent::RouteNodeChanged {
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
            self.events.push(event);
        }
        self.emit_camera_if_changed();
    }

    fn emit_selection(&mut self) {
        self.events.push(RendererEvent::Selection {
            indices: self.selected.iter().copied().collect(),
        });
    }

    pub(crate) fn ui(&mut self, ui: &mut gpu2d::Ui) {
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

#[derive(Default)]
pub(crate) struct RawInput {
    pointer_pos: Option<gpu2d::Pos2>,
    hovered: bool,
    drag_delta: gpu2d::Vec2,
    buttons_down: HashSet<PointerButton>,
    buttons_pressed: HashSet<PointerButton>,
    buttons_released: HashSet<PointerButton>,
    buttons_cancelled: HashSet<PointerButton>,
    buttons_clicked: HashSet<PointerButton>,
    press_origins: HashMap<PointerButton, gpu2d::Pos2>,
    double_clicked: bool,
    scroll: gpu2d::Vec2,
    keys_pressed: HashSet<Key>,
    modifiers: gpu2d::Modifiers,
    pointer_source: gpu2d::PointerSource,
    touch_transforms: Vec<gpu2d::TouchTransform>,
}

impl RawInput {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pointer_event(
        &mut self,
        kind: &str,
        x: f32,
        y: f32,
        button: i16,
        detail: i16,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
        source: &str,
    ) {
        let position = gpu2d::pos2(x, y);
        if kind == "move"
            && !self.buttons_down.is_empty()
            && let Some(previous) = self.pointer_pos
        {
            self.drag_delta += position - previous;
        }
        self.pointer_pos = Some(position);
        self.modifiers = gpu2d::Modifiers {
            alt,
            ctrl,
            shift,
            command,
        };
        self.pointer_source = match source {
            "touch" => gpu2d::PointerSource::Touch,
            "pen" => gpu2d::PointerSource::Pen,
            _ => gpu2d::PointerSource::Mouse,
        };
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
                if self.press_origins.get(&button).is_some_and(|origin| {
                    let delta = position - *origin;
                    delta.length_sq() <= 36.0
                }) {
                    self.buttons_clicked.insert(button);
                }
            }
            ("cancel", Some(button)) => {
                self.buttons_down.remove(&button);
                self.buttons_cancelled.insert(button);
                self.press_origins.remove(&button);
            }
            _ => {}
        }
    }

    pub(crate) fn wheel(&mut self, x: f32, y: f32) {
        self.scroll += gpu2d::vec2(x, y);
    }

    pub(crate) fn key(&mut self, key: &str, alt: bool, ctrl: bool, shift: bool, command: bool) {
        self.modifiers = gpu2d::Modifiers {
            alt,
            ctrl,
            shift,
            command,
        };
        let key = match key {
            "Enter" => Some(Key::Enter),
            "Escape" => Some(Key::Escape),
            "Delete" => Some(Key::Delete),
            "Backspace" => Some(Key::Backspace),
            _ => None,
        };
        if let Some(key) = key {
            self.keys_pressed.insert(key);
        }
    }

    pub(crate) fn touch_transform(
        &mut self,
        zoom_delta: f32,
        dx: f32,
        dy: f32,
        center_x: f32,
        center_y: f32,
    ) {
        if !zoom_delta.is_finite()
            || zoom_delta <= 0.0
            || !dx.is_finite()
            || !dy.is_finite()
            || !center_x.is_finite()
            || !center_y.is_finite()
        {
            return;
        }
        self.touch_transforms.push(gpu2d::TouchTransform {
            zoom_delta,
            translation_delta: gpu2d::vec2(dx, dy),
            center: gpu2d::pos2(center_x, center_y),
        });
    }

    pub(crate) fn frame(
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
            touch_transforms: std::mem::take(&mut self.touch_transforms),
            keys_pressed: std::mem::take(&mut self.keys_pressed),
        };
        let buttons_released = std::mem::take(&mut self.buttons_released);
        let press_origins = self.press_origins.clone();
        for button in &buttons_released {
            if !self.buttons_down.contains(button) {
                self.press_origins.remove(button);
            }
        }
        let response = gpu2d::Response {
            rect,
            pointer_pos,
            hovered: self.hovered && pointer_pos.is_some(),
            drag_delta: self.drag_delta,
            buttons_down: self.buttons_down.clone(),
            buttons_pressed: std::mem::take(&mut self.buttons_pressed),
            buttons_released,
            buttons_cancelled: std::mem::take(&mut self.buttons_cancelled),
            buttons_clicked: std::mem::take(&mut self.buttons_clicked),
            press_origins,
            pointer_source: self.pointer_source,
            double_clicked: std::mem::take(&mut self.double_clicked),
        };
        self.drag_delta = gpu2d::Vec2::ZERO;
        self.scroll = gpu2d::Vec2::ZERO;
        (input, response)
    }
}

#[cfg(test)]
mod raw_input_tests {
    use super::RawInput;
    use crate::gpu2d::{PointerButton, PointerSource};

    #[test]
    fn touch_transforms_keep_order_until_the_next_frame() {
        let mut input = RawInput::default();
        input.touch_transform(1.1, 3.0, 4.0, 20.0, 30.0);
        input.touch_transform(0.9, -2.0, 5.0, 24.0, 35.0);

        let (frame, _) = input.frame(320, 240, 1.0 / 60.0);
        assert_eq!(frame.touch_transforms.len(), 2);
        assert_eq!(frame.touch_transforms[0].zoom_delta, 1.1);
        assert_eq!(frame.touch_transforms[0].center.x, 20.0);
        assert_eq!(frame.touch_transforms[1].translation_delta.y, 5.0);

        let (next_frame, _) = input.frame(320, 240, 1.0 / 60.0);
        assert!(next_frame.touch_transforms.is_empty());
    }

    #[test]
    fn touch_transforms_reject_non_finite_input() {
        let mut input = RawInput::default();
        input.touch_transform(f32::NAN, 0.0, 0.0, 10.0, 10.0);
        input.touch_transform(1.0, f32::INFINITY, 0.0, 10.0, 10.0);

        let (frame, _) = input.frame(320, 240, 1.0 / 60.0);
        assert!(frame.touch_transforms.is_empty());
    }

    #[test]
    fn touch_pointer_keeps_press_origin_and_source_during_direct_drag() {
        let mut input = RawInput::default();
        input.pointer_event(
            "down", 200.0, 180.0, 0, 1, false, false, false, false, "touch",
        );
        input.pointer_event(
            "up", 200.0, 180.0, 0, 1, false, false, false, false, "touch",
        );
        let _ = input.frame(320, 240, 1.0 / 60.0);
        input.pointer_event(
            "down", 12.0, 18.0, 0, 1, false, false, false, false, "touch",
        );
        input.pointer_event(
            "move", 30.0, 42.0, 0, 0, false, false, false, false, "touch",
        );

        let (_, response) = input.frame(320, 240, 1.0 / 60.0);
        assert_eq!(response.pointer_source(), PointerSource::Touch);
        assert_eq!(
            response.press_origin(PointerButton::Primary),
            Some(crate::gpu2d::pos2(12.0, 18.0))
        );
        assert!(response.drag_started_by(PointerButton::Primary));
        assert!(response.dragged_by(PointerButton::Primary));
        assert_eq!(
            response.interact_pointer_pos(),
            Some(crate::gpu2d::pos2(30.0, 42.0))
        );
        assert_eq!(response.drag_delta(), crate::gpu2d::vec2(18.0, 24.0));
    }

    #[test]
    fn pointer_cancel_is_not_a_release_or_click() {
        let mut input = RawInput::default();
        input.pointer_event(
            "down", 10.0, 10.0, 0, 1, false, false, false, false, "touch",
        );
        let _ = input.frame(320, 240, 1.0 / 60.0);
        input.pointer_event(
            "cancel", 18.0, 10.0, 0, 0, false, false, false, false, "touch",
        );

        let (_, response) = input.frame(320, 240, 1.0 / 60.0);
        assert!(response.drag_cancelled_by(PointerButton::Primary));
        assert!(!response.drag_stopped_by(PointerButton::Primary));
        assert!(!response.clicked());
        assert_eq!(response.press_origin(PointerButton::Primary), None);
    }

    #[test]
    fn quick_touch_drag_can_complete_between_renderer_frames() {
        let mut input = RawInput::default();
        input.pointer_event(
            "down", 10.0, 12.0, 0, 1, false, false, false, false, "touch",
        );
        input.pointer_event(
            "move", 28.0, 18.0, 0, 0, false, false, false, false, "touch",
        );
        input.pointer_event("up", 28.0, 18.0, 0, 0, false, false, false, false, "touch");

        let (_, response) = input.frame(320, 240, 1.0 / 60.0);
        assert!(response.drag_completed_by(PointerButton::Primary));
        assert_eq!(
            response.press_origin(PointerButton::Primary),
            Some(crate::gpu2d::pos2(10.0, 12.0))
        );
        assert_eq!(response.drag_delta(), crate::gpu2d::vec2(18.0, 6.0));
        assert!(!response.clicked());

        let (_, response) = input.frame(320, 240, 1.0 / 60.0);
        assert_eq!(response.press_origin(PointerButton::Primary), None);
    }

    #[test]
    fn touch_secondary_click_reaches_context_menu_input() {
        let mut input = RawInput::default();
        input.pointer_event(
            "down", 80.0, 90.0, 2, 1, false, false, false, false, "touch",
        );
        input.pointer_event("up", 80.0, 90.0, 2, 1, false, false, false, false, "touch");

        let (_, response) = input.frame(320, 240, 1.0 / 60.0);
        assert_eq!(response.pointer_source(), PointerSource::Touch);
        assert!(response.secondary_clicked());
        assert_eq!(
            response.interact_pointer_pos(),
            Some(crate::gpu2d::pos2(80.0, 90.0))
        );
    }
}
