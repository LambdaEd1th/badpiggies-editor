//! egui application — main editor UI with three-panel layout.

use eframe::egui;

use crate::locale::{I18n, Language};
use crate::parser;
use crate::renderer::LevelRenderer;
use crate::types::*;

/// Main application state.
pub struct EditorApp {
    /// Currently loaded level data.
    level: Option<LevelData>,
    /// File name of the loaded level.
    file_name: Option<String>,
    /// Currently selected object index.
    selected: Option<ObjectIndex>,
    /// Canvas renderer state.
    renderer: LevelRenderer,
    /// Status message.
    status: String,
    /// Add-object dialog state.
    show_add_dialog: bool,
    add_obj_is_parent: bool,
    add_obj_name: String,
    add_obj_prefab_index: i16,
    /// Pending file data from drag-and-drop or file picker.
    #[cfg(target_arch = "wasm32")]
    pending_file: Option<(String, Vec<u8>)>,
    /// Pending delete confirmation: (object_index, object_name).
    pending_delete: Option<(ObjectIndex, String)>,
    /// Whether the object tree panel is visible.
    show_object_tree: bool,
    /// Whether the properties panel is visible.
    show_properties: bool,
    /// Whether the shortcuts help window is visible.
    show_shortcuts: bool,
    /// Whether the about window is visible.
    show_about: bool,
    /// Current UI language.
    lang: Language,
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_cjk_fonts(&cc.egui_ctx);
        let mut renderer = LevelRenderer::new(cc.wgpu_render_state.as_ref());

        // Auto-detect asset base directory relative to the executable
        #[cfg(not(target_arch = "wasm32"))]
        {
            let asset_base = Self::detect_asset_base();
            if let Some(base) = asset_base {
                renderer.asset_base = Some(base);
            }
        }

        Self {
            level: None,
            file_name: None,
            selected: None,
            renderer,
            status: crate::locale::Language::default()
                .i18n()
                .get("status_welcome"),
            show_add_dialog: false,
            add_obj_is_parent: false,
            add_obj_name: String::new(),
            add_obj_prefab_index: 0,
            #[cfg(target_arch = "wasm32")]
            pending_file: None,
            pending_delete: None,
            show_object_tree: true,
            show_properties: true,
            show_shortcuts: false,
            show_about: false,
            lang: Language::default(),
        }
    }

    /// Returns the current language's translations.
    fn t(&self) -> &'static I18n {
        self.lang.i18n()
    }

    /// Detect the asset base directory by searching upward from the executable.
    #[cfg(not(target_arch = "wasm32"))]
    fn detect_asset_base() -> Option<String> {
        // First try ASSET_BASE env var
        if let Ok(val) = std::env::var("ASSET_BASE") {
            let p = std::path::Path::new(&val);
            if p.is_dir() {
                return Some(val);
            }
        }
        // Walk up from the executable to find assets/ or level-editor/public/assets
        let exe = std::env::current_exe().ok()?;
        let mut dir = exe.parent()?;
        for _ in 0..6 {
            // Prefer local assets/ directory (bundled with the editor)
            let local = dir.join("assets");
            if local.join("sprites").is_dir() {
                return Some(local.to_string_lossy().into_owned());
            }
            let candidate = dir.join("level-editor/public/assets");
            if candidate.is_dir() {
                return Some(candidate.to_string_lossy().into_owned());
            }
            dir = dir.parent()?;
        }
        None
    }

    fn load_level(&mut self, name: String, data: Vec<u8>) {
        match parser::parse_level(data) {
            Ok(level) => {
                let obj_count = level.objects.len();
                let root_count = level.roots.len();
                self.renderer.set_level_key(&name);
                self.renderer.set_level(&level);
                self.level = Some(level);
                self.file_name = Some(name);
                self.selected = None;
                self.status = self.lang.i18n().fmt_status_loaded(obj_count, root_count);
            }
            Err(e) => {
                self.status = format!("解析失败: {}", e);
            }
        }
    }

    fn export_level(&self) -> Option<Vec<u8>> {
        self.level.as_ref().map(parser::serialize_level)
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let t = self.t();
        // Handle dropped files
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                // On WASM, bytes is populated; on native, path is populated
                let file_data: Option<(String, Vec<u8>)> = if let Some(ref bytes) = file.bytes {
                    Some((file.name.clone(), bytes.to_vec()))
                } else if let Some(ref path) = file.path {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        std::fs::read(path).ok().map(|data| {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| file.name.clone());
                            (name, data)
                        })
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = path;
                        None
                    }
                } else {
                    None
                };

                if let Some((name, data)) = file_data {
                    match parser::parse_level(data) {
                        Ok(level) => {
                            let obj_count = level.objects.len();
                            let root_count = level.roots.len();
                            self.renderer.set_level_key(&name);
                            self.renderer.set_level(&level);
                            self.level = Some(level);
                            self.file_name = Some(name);
                            self.selected = None;
                            self.status = self.lang.i18n().fmt_status_loaded(obj_count, root_count);
                        }
                        Err(e) => {
                            self.status = format!("解析失败: {}", e);
                        }
                    }
                }
            }
        });

        // B key — toggle background visibility
        if ctx.input(|i| i.key_pressed(egui::Key::B)) {
            self.renderer.show_bg = !self.renderer.show_bg;
        }

        // Handle Delete / Backspace key — queue confirmation dialog
        // Only when no text widget has focus (avoid intercepting text editing)
        if let Some(sel) = self.selected {
            let delete_pressed = !ctx.egui_wants_keyboard_input()
                && ctx.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                });
            if delete_pressed
                && self.pending_delete.is_none()
                && let Some(ref level) = self.level
            {
                let name = level.objects[sel].name().to_string();
                self.pending_delete = Some((sel, name));
            }
        }

        // Delete confirmation dialog
        if let Some((del_idx, ref del_name)) = self.pending_delete.clone() {
            let mut action = 0u8; // 0=pending, 1=confirm, 2=cancel
            egui::Window::new(t.get("win_confirm_delete"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&ctx, |ui| {
                    ui.label(t.fmt1("status_delete_confirm", del_name));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.button(t.get("btn_ok")).clicked();
                        if ui.button(t.get("btn_cancel")).clicked() {
                            action = 2;
                        }
                    });
                });
            match action {
                1 => {
                    if let Some(ref mut level) = self.level {
                        level.delete_object(del_idx);
                        self.selected = None;
                        self.renderer.set_level(level);
                        self.status = format!("已删除: {}", del_name);
                    }
                    self.pending_delete = None;
                }
                2 => {
                    self.pending_delete = None;
                }
                _ => {}
            }
        }

        // ── Top menu bar ──
        egui::Panel::top("menu_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(t.get("menu_file"), |ui| {
                    if ui.button(t.get("menu_open_level")).clicked() {
                        ui.close();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Level files", &["bytes"])
                                .pick_file()
                            {
                                match std::fs::read(&path) {
                                    Ok(data) => {
                                        let name = path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default();
                                        self.load_level(name, data);
                                    }
                                    Err(e) => {
                                        self.status = t.fmt1("status_read_error", &e.to_string());
                                    }
                                }
                            }
                        }
                    }
                    ui.separator();
                    if ui.button(t.get("menu_export_level")).clicked() {
                        ui.close();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if let Some(data) = self.export_level()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Level files", &["bytes"])
                                    .save_file()
                            {
                                match std::fs::write(&path, data) {
                                    Ok(()) => {
                                        self.status = t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.status = t.fmt1("status_export_error", &e.to_string());
                                    }
                                }
                            }
                        }
                    }
                });
                ui.menu_button(t.get("menu_edit"), |ui| {
                    if ui.button(t.get("menu_add_object")).clicked() {
                        ui.close();
                        if self.level.is_some() {
                            self.add_obj_name = "NewObject".into();
                            self.add_obj_prefab_index = 0;
                            self.add_obj_is_parent = false;
                            self.show_add_dialog = true;
                        }
                    }
                });
                ui.menu_button(t.get("menu_view"), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if ui.button(t.get("menu_fit_view")).clicked() {
                        ui.close();
                        self.renderer.fit_to_level();
                    }
                    let bg_label = if self.renderer.show_bg {
                        t.get("menu_hide_bg")
                    } else {
                        t.get("menu_show_bg")
                    };
                    if ui.button(bg_label).clicked() {
                        ui.close();
                        self.renderer.show_bg = !self.renderer.show_bg;
                    }
                    ui.separator();
                    {
                        let mut v = self.show_object_tree;
                        if ui.checkbox(&mut v, t.get("menu_object_list")).clicked() {
                            ui.close();
                            self.show_object_tree = v;
                        }
                    }
                    {
                        let mut v = self.show_properties;
                        if ui.checkbox(&mut v, t.get("menu_properties")).clicked() {
                            ui.close();
                            self.show_properties = v;
                        }
                    }
                    ui.separator();
                    {
                        let mut v = self.renderer.show_ground;
                        if ui.checkbox(&mut v, t.get("menu_physics_ground")).clicked() {
                            ui.close();
                            self.renderer.show_ground = v;
                        }
                    }
                    ui.separator();
                    ui.menu_button(t.get("menu_language"), |ui| {
                        for &lang in Language::ALL {
                            if ui
                                .selectable_label(self.lang == lang, lang.display_name())
                                .clicked()
                            {
                                self.lang = lang;
                                ui.close();
                            }
                        }
                    });
                });
                ui.menu_button(t.get("menu_help"), |ui| {
                    ui.set_min_width(120.0);
                    if ui.button(t.get("menu_shortcuts")).clicked() {
                        ui.close();
                        self.show_shortcuts = true;
                    }
                    if ui.button(t.get("menu_about")).clicked() {
                        ui.close();
                        self.show_about = true;
                    }
                });
            });
        });

        // ── 说明窗口 ──
        if self.show_shortcuts {
            egui::Window::new(t.get("win_shortcuts"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut self.show_shortcuts)
                .show(&ctx, |ui| {
                    egui::Grid::new("shortcuts_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong(t.get("shortcuts_key"));
                            ui.strong(t.get("shortcuts_action"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_scroll"));
                            ui.label(t.get("shortcuts_zoom"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_drag"));
                            ui.label(t.get("shortcuts_pan"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_click"));
                            ui.label(t.get("shortcuts_select"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_b_key"));
                            ui.label(t.get("shortcuts_toggle_bg"));
                            ui.end_row();
                        });
                });
        }

        // ── 关于窗口 ──
        if self.show_about {
            egui::Window::new(t.get("win_about"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut self.show_about)
                .show(&ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Bad Piggies Editor");
                        ui.label(format!(
                            "{}{}",
                            t.get("about_version_prefix"),
                            env!("CARGO_PKG_VERSION")
                        ));
                        ui.separator();
                        ui.label(env!("CARGO_PKG_AUTHORS"));
                        ui.label(t.get("about_built_with"));
                        ui.label(t.get("about_license"));
                    });
                });
        }

        // ── Add Object Dialog ──
        if self.show_add_dialog {
            let mut open = true;
            egui::Window::new(t.get("win_add_object"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(t.get("add_type"));
                        ui.radio_value(&mut self.add_obj_is_parent, false, "Prefab");
                        ui.radio_value(&mut self.add_obj_is_parent, true, "Parent");
                    });
                    ui.horizontal(|ui| {
                        ui.label(t.get("add_name"));
                        ui.text_edit_singleline(&mut self.add_obj_name);
                    });
                    if !self.add_obj_is_parent {
                        ui.horizontal(|ui| {
                            ui.label(t.get("add_prefab_index"));
                            ui.add(egui::DragValue::new(&mut self.add_obj_prefab_index));
                        });
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(t.get("btn_ok")).clicked() {
                            if let Some(ref mut level) = self.level {
                                let name = if self.add_obj_name.is_empty() {
                                    "NewObject".to_string()
                                } else {
                                    self.add_obj_name.clone()
                                };
                                let new_idx = level.objects.len();
                                if self.add_obj_is_parent {
                                    level.objects.push(LevelObject::Parent(ParentObject {
                                        name: name.clone(),
                                        position: Vec3 {
                                            x: 0.0,
                                            y: 0.0,
                                            z: 0.0,
                                        },
                                        children: Vec::new(),
                                        parent: None,
                                    }));
                                } else {
                                    level.objects.push(LevelObject::Prefab(PrefabInstance {
                                        name: name.clone(),
                                        position: Vec3 {
                                            x: 0.0,
                                            y: 0.0,
                                            z: 0.0,
                                        },
                                        prefab_index: self.add_obj_prefab_index,
                                        rotation: Vec3 {
                                            x: 0.0,
                                            y: 0.0,
                                            z: 0.0,
                                        },
                                        scale: Vec3 {
                                            x: 1.0,
                                            y: 1.0,
                                            z: 1.0,
                                        },
                                        data_type: DataType::None,
                                        terrain_data: None,
                                        override_data: None,
                                        parent: None,
                                    }));
                                }
                                level.roots.push(new_idx);
                                self.selected = Some(new_idx);
                                self.renderer.set_level(level);
                                self.status = t.fmt1("status_added", &name);
                            }
                            self.show_add_dialog = false;
                        }
                        if ui.button(t.get("btn_cancel")).clicked() {
                            self.show_add_dialog = false;
                        }
                    });
                });
            if !open {
                self.show_add_dialog = false;
            }
        }

        // ── Status bar ──
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                // Mouse world coordinates
                if let Some(mw) = self.renderer.mouse_world {
                    ui.separator();
                    ui.label(format!("X: {:.2}  Y: {:.2}", mw.x, mw.y));
                }
                if let Some(ref name) = self.file_name {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(name);
                    });
                }
            });
        });

        // ── Left panel: Object tree ──
        if self.show_object_tree {
            egui::Panel::left("object_tree")
                .default_size(240.0)
                .show_inside(ui, |ui| {
                    ui.heading(t.get("panel_object_list"));
                    ui.separator();

                    if let Some(ref level) = self.level {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut new_selection = self.selected;
                            for &root_idx in &level.roots {
                                show_object_tree(ui, level, root_idx, &mut new_selection, 0);
                            }
                            self.selected = new_selection;
                        });
                    }
                });
        }

        // ── Right panel: Properties ──
        if self.show_properties {
            egui::Panel::right("properties")
                .default_size(280.0)
                .size_range(120.0..=500.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    // TextEdits fill available width without forcing the panel
                    // wider — width_range caps the panel size.
                    ui.spacing_mut().text_edit_width = f32::INFINITY;
                    ui.heading(t.get("panel_properties"));
                    ui.separator();

                    if let (Some(level), Some(sel)) = (&mut self.level, self.selected) {
                        if sel < level.objects.len() {
                            let changed = show_properties_editable(ui, &mut level.objects[sel], t);
                            if changed {
                                // Rebuild renderer when properties change
                                self.renderer.set_level(level);
                            }
                        }
                    } else {
                        ui.label(t.get("panel_select_hint"));
                    }
                });
        }

        // ── Central panel: Canvas ──
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.level.is_some() {
                self.renderer.show(ui, self.selected, t);
                // Pick up click-to-select from renderer
                if let Some(idx) = self.renderer.clicked_object {
                    self.selected = Some(idx);
                }
                // Pick up drag result — update object position
                if let Some((idx, delta)) = self.renderer.drag_result.take()
                    && let Some(ref mut level) = self.level
                    && idx < level.objects.len()
                {
                    match &mut level.objects[idx] {
                        LevelObject::Prefab(p) => {
                            p.position.x += delta.x;
                            p.position.y += delta.y;
                        }
                        LevelObject::Parent(p) => {
                            p.position.x += delta.x;
                            p.position.y += delta.y;
                        }
                    }
                    // Rebuild draw data but preserve camera position/zoom
                    let cam = self.renderer.camera.clone();
                    self.renderer.set_level(level);
                    self.renderer.camera = cam;
                }
            } else {
                let rect = ui.available_rect_before_wrap();
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                            ui.vertical_centered(|ui| {
                                let center_y = rect.center().y - 40.0;
                                ui.add_space((center_y - rect.top()).max(0.0));
                                ui.label(
                                    egui::RichText::new("⬇")
                                        .size(32.0)
                                        .color(egui::Color32::from_gray(160)),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(t.get("panel_drop_hint"))
                                        .color(egui::Color32::from_gray(180)),
                                );
                                ui.label(
                                    egui::RichText::new(t.get("panel_open_hint"))
                                        .color(egui::Color32::from_gray(140)),
                                );
                            });
                        },
                    );
                });
            }
        });
    }
}

/// Recursively render the object tree.
fn show_object_tree(
    ui: &mut egui::Ui,
    level: &LevelData,
    idx: ObjectIndex,
    selected: &mut Option<ObjectIndex>,
    depth: usize,
) {
    let obj = &level.objects[idx];
    let is_selected = *selected == Some(idx);

    match obj {
        LevelObject::Parent(parent) => {
            let id = ui.make_persistent_id(format!("obj_{}", idx));
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                depth < 1,
            )
            .show_header(ui, |ui| {
                if ui.selectable_label(is_selected, &parent.name).clicked() {
                    *selected = Some(idx);
                }
            })
            .body(|ui| {
                for &child in &parent.children {
                    show_object_tree(ui, level, child, selected, depth + 1);
                }
            });
        }
        LevelObject::Prefab(prefab) => {
            let label = format!("{} [{}]", prefab.name, prefab.prefab_index);
            if ui.selectable_label(is_selected, label).clicked() {
                *selected = Some(idx);
            }
        }
    }
}

/// Show editable properties. Returns true if anything changed.
fn show_properties_editable(
    ui: &mut egui::Ui,
    obj: &mut LevelObject,
    t: &'static crate::locale::I18n,
) -> bool {
    let mut changed = false;
    match obj {
        LevelObject::Prefab(p) => {
            ui.label(t.get("prop_type_prefab"));
            ui.horizontal(|ui| {
                ui.label(t.get("prop_name"));
                changed |= ui.text_edit_singleline(&mut p.name).changed();
            });
            ui.label(format!("{} {}", t.get("prop_prefab_index"), p.prefab_index));
            ui.separator();

            ui.label(t.get("prop_position"));
            changed |= edit_vec3(ui, "p_pos", &mut p.position);

            ui.label(t.get("prop_rotation"));
            changed |= edit_vec3(ui, "p_rot", &mut p.rotation);

            ui.label(t.get("prop_scale"));
            changed |= edit_vec3(ui, "p_scl", &mut p.scale);

            ui.separator();
            ui.label(format!("{} {:?}", t.get("prop_data_type"), p.data_type));

            if let Some(ref mut td) = p.terrain_data {
                ui.separator();
                ui.label(t.get("prop_terrain"));
                ui.label(format!(
                    "{} {}",
                    t.get("prop_fill_vert_count"),
                    td.fill_mesh.vertices.len()
                ));
                ui.label(format!(
                    "{} {}",
                    t.get("prop_curve_vert_count"),
                    td.curve_mesh.vertices.len()
                ));
                ui.label(format!("{} {}", t.get("prop_collider"), td.has_collider));
                show_color(ui, &t.get("prop_fill_color"), &td.fill_color);
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_offset_x"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_tile_offset_x).speed(0.01))
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_offset_y"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_tile_offset_y).speed(0.01))
                        .changed();
                });
            }

            if let Some(ref mut od) = p.override_data {
                ui.separator();

                // Toggle between tree view and raw text
                let toggle_id = ui.make_persistent_id("ov_raw_toggle");
                let mut show_raw: bool = ui.data(|d| d.get_temp(toggle_id).unwrap_or(false));
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_override"));
                    if ui
                        .small_button(if show_raw {
                            t.get("btn_visual")
                        } else {
                            t.get("btn_text")
                        })
                        .clicked()
                    {
                        show_raw = !show_raw;
                        ui.data_mut(|d| d.insert_temp(toggle_id, show_raw));
                    }
                });

                if show_raw {
                    // Raw text editor
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            if ui.text_edit_multiline(&mut od.raw_text).changed() {
                                od.raw_bytes = od.raw_text.as_bytes().to_vec();
                                changed = true;
                            }
                        });
                } else {
                    // Tree view
                    let mut tree = parse_override_text(&od.raw_text);
                    let mut tree_changed = false;
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            tree_changed = show_override_tree(ui, &mut tree, 0, t);
                        });
                    if tree_changed {
                        let new_text = serialize_override_tree(&tree, 0);
                        od.raw_text = new_text.clone();
                        od.raw_bytes = new_text.into_bytes();
                        changed = true;
                    }
                }
            }
        }
        LevelObject::Parent(p) => {
            ui.label(t.get("prop_type_parent"));
            ui.horizontal(|ui| {
                ui.label(t.get("prop_name"));
                changed |= ui.text_edit_singleline(&mut p.name).changed();
            });
            ui.label(format!(
                "{} {}",
                t.get("prop_child_count"),
                p.children.len()
            ));
            ui.separator();

            ui.label(t.get("prop_position"));
            changed |= edit_vec3(ui, "par_pos", &mut p.position);
        }
    }
    changed
}

/// Editable Vec3 with three DragValue fields. Returns true if changed.
fn edit_vec3(ui: &mut egui::Ui, id_prefix: &str, v: &mut Vec3) -> bool {
    let mut changed = false;
    ui.push_id(id_prefix, |ui| {
        ui.horizontal(|ui| {
            ui.label("  X");
            changed |= ui.add(egui::DragValue::new(&mut v.x).speed(0.05)).changed();
            ui.label("Y");
            changed |= ui.add(egui::DragValue::new(&mut v.y).speed(0.05)).changed();
            ui.label("Z");
            changed |= ui.add(egui::DragValue::new(&mut v.z).speed(0.05)).changed();
        });
    });
    changed
}

fn show_color(ui: &mut egui::Ui, label: &str, c: &Color) {
    let rgba = c.to_rgba8();
    ui.horizontal(|ui| {
        let color = egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
        ui.label(format!("  {}: ", label));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color);
        ui.label(format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            rgba[0], rgba[1], rgba[2], rgba[3]
        ));
    });
}

/// Load a system CJK font and register it as a fallback for proportional + monospace.
fn configure_cjk_fonts(ctx: &egui::Context) {
    let Some(data) = load_system_cjk_font() else {
        log::warn!("No system CJK font found — Chinese text will render as squares");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".into(),
        std::sync::Arc::new(egui::FontData::from_owned(data)),
    );
    // Append as fallback after default Latin fonts
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("cjk".into());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push("cjk".into());
    }
    ctx.set_fonts(fonts);
}

#[cfg(not(target_arch = "wasm32"))]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    let candidates = [
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        // Windows
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        // Linux
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            log::info!("Loaded CJK font: {}", path);
            return Some(data);
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    // TODO: bundle or fetch a CJK font for WASM builds
    None
}

// ── Override tree data structures and editor ──

/// A node in the parsed override tree.
struct OverrideNode {
    node_type: String,
    name: String,
    value: Option<String>,
    children: Vec<OverrideNode>,
}

/// Parse ObjectDeserializer tab-indented text into a tree of OverrideNodes.
fn parse_override_text(raw: &str) -> Vec<OverrideNode> {
    let lines: Vec<&str> = raw.lines().collect();
    parse_override_range(&lines, 0, lines.len(), 0)
}

fn parse_override_range(
    lines: &[&str],
    start: usize,
    end: usize,
    base_depth: usize,
) -> Vec<OverrideNode> {
    let mut result = Vec::new();
    let mut i = start;
    while i < end {
        let line = lines[i].trim_end_matches('\r');
        let depth = line.len() - line.trim_start_matches('\t').len();
        let trimmed = line.trim();
        if trimmed.is_empty() || depth < base_depth {
            i += 1;
            continue;
        }
        if depth > base_depth {
            i += 1;
            continue;
        } // skip orphan deeper lines

        let (node_type, name, value) = parse_override_line(trimmed);

        // Find child range: all subsequent lines with depth > current
        let child_start = i + 1;
        let mut child_end = child_start;
        while child_end < end {
            let cl = lines[child_end].trim_end_matches('\r');
            let cd = cl.len() - cl.trim_start_matches('\t').len();
            if cl.trim().is_empty() {
                child_end += 1;
                continue;
            }
            if cd <= depth {
                break;
            }
            child_end += 1;
        }

        let children = if child_start < child_end {
            parse_override_range(lines, child_start, child_end, depth + 1)
        } else {
            Vec::new()
        };

        result.push(OverrideNode {
            node_type,
            name,
            value,
            children,
        });
        i = child_end;
    }
    result
}

fn parse_override_line(trimmed: &str) -> (String, String, Option<String>) {
    // Check for " = " (value present) or trailing " =" (empty value)
    if let Some(eq_pos) = trimmed.find(" = ").or_else(|| {
        if trimmed.ends_with(" =") {
            Some(trimmed.len() - 2)
        } else {
            None
        }
    }) {
        let before = &trimmed[..eq_pos];
        let after = if eq_pos + 3 <= trimmed.len() {
            &trimmed[eq_pos + 3..]
        } else {
            ""
        };
        let parts: Vec<&str> = before.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            (
                parts[0].to_string(),
                parts[1].to_string(),
                Some(after.to_string()),
            )
        } else {
            (parts[0].to_string(), String::new(), Some(after.to_string()))
        }
    } else {
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string(), None)
        } else {
            (parts[0].to_string(), String::new(), None)
        }
    }
}

/// Serialize override tree back to tab-indented text.
fn serialize_override_tree(nodes: &[OverrideNode], depth: usize) -> String {
    let mut out = String::new();
    let indent: String = "\t".repeat(depth);
    for n in nodes {
        if let Some(ref val) = n.value {
            out.push_str(&format!("{}{} {} = {}\n", indent, n.node_type, n.name, val));
        } else {
            out.push_str(&format!("{}{} {}\n", indent, n.node_type, n.name));
        }
        out.push_str(&serialize_override_tree(&n.children, depth + 1));
    }
    out
}

/// Type badge color for override node types.
fn override_type_color(t: &str) -> egui::Color32 {
    match t {
        "GameObject" => egui::Color32::from_rgb(79, 195, 247),
        "Component" => egui::Color32::from_rgb(102, 187, 106),
        "Float" | "Integer" => egui::Color32::from_rgb(206, 147, 216),
        "Boolean" => egui::Color32::from_rgb(255, 183, 77),
        "String" => egui::Color32::from_rgb(165, 214, 167),
        "Enum" => egui::Color32::from_rgb(244, 143, 177),
        "Vector2" | "Vector3" | "Quaternion" | "Rect" => egui::Color32::from_rgb(128, 222, 234),
        "Color" => egui::Color32::from_rgb(239, 154, 154),
        "Array" | "AnimationCurve" | "Generic" => egui::Color32::from_rgb(144, 164, 174),
        _ => egui::Color32::from_rgb(176, 190, 197),
    }
}

/// Show the override tree editor. Returns true if any value changed.
fn show_override_tree(
    ui: &mut egui::Ui,
    nodes: &mut Vec<OverrideNode>,
    depth: usize,
    t: &'static crate::locale::I18n,
) -> bool {
    let mut changed = false;
    let mut to_delete: Option<usize> = None;

    for (i, node) in nodes.iter_mut().enumerate() {
        let has_children = !node.children.is_empty();
        let is_container = matches!(
            node.node_type.as_str(),
            "GameObject" | "Component" | "Array" | "AnimationCurve" | "Generic"
        ) || has_children;

        let id = ui.make_persistent_id(format!("ovr_{}_{}", depth, i));

        if is_container {
            // Collapsible section
            let header = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                depth < 2,
            );
            header
                .show_header(ui, |ui| {
                    let color = override_type_color(&node.node_type);
                    ui.colored_label(color, &node.node_type);
                    // Place delete button first (right-to-left) so it's never clipped
                    if ui.small_button(t.get("btn_delete")).clicked() {
                        to_delete = Some(i);
                    }
                    let w = ui.available_width().max(30.0);
                    if ui
                        .add(egui::TextEdit::singleline(&mut node.name).desired_width(w))
                        .changed()
                    {
                        changed = true;
                    }
                })
                .body(|ui| {
                    changed |= show_override_tree(ui, &mut node.children, depth + 1, t);
                });
        } else if node.value.is_some() {
            // Leaf value — editable inline
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 12.0);
                let color = override_type_color(&node.node_type);
                ui.colored_label(color, &node.node_type);
                if ui.small_button(t.get("btn_delete")).clicked() {
                    to_delete = Some(i);
                }
                let avail = ui.available_width();
                let name_w = (avail * 0.4).max(20.0);
                let val_w = (avail - name_w - 12.0).max(20.0); // 12px for "="
                if ui
                    .add(egui::TextEdit::singleline(&mut node.name).desired_width(name_w))
                    .changed()
                {
                    changed = true;
                }
                ui.label("=");
                let val = node.value.as_mut().unwrap();
                if ui
                    .add(egui::TextEdit::singleline(val).desired_width(val_w))
                    .changed()
                {
                    changed = true;
                }
            });
        } else {
            // Non-container without value
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 12.0);
                let color = override_type_color(&node.node_type);
                ui.colored_label(color, &node.node_type);
                if ui.small_button(t.get("btn_delete")).clicked() {
                    to_delete = Some(i);
                }
                let w = ui.available_width().max(30.0);
                if ui
                    .add(egui::TextEdit::singleline(&mut node.name).desired_width(w))
                    .changed()
                {
                    changed = true;
                }
            });
        }
    }

    if let Some(idx) = to_delete {
        nodes.remove(idx);
        changed = true;
    }

    // Add-node button
    let add_id = ui.make_persistent_id(format!("ovr_add_{}", depth));
    let mut adding: bool = ui.data(|d| d.get_temp(add_id).unwrap_or(false));
    if adding {
        changed |= show_add_node_form(ui, nodes, add_id, &mut adding, depth, t);
    } else {
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            if ui.small_button(t.get("btn_add")).clicked() {
                adding = true;
                ui.data_mut(|d| d.insert_temp(add_id, true));
            }
        });
    }

    changed
}

/// All supported override node types.
const OVERRIDE_ALL_TYPES: &[&str] = &[
    "GameObject",
    "Component",
    "Float",
    "Integer",
    "Boolean",
    "String",
    "Enum",
    "Vector2",
    "Vector3",
    "Quaternion",
    "Color",
    "Array",
    "Generic",
    "Element",
    "AnimationCurve",
    "Keyframe",
    "Rect",
    "Bounds",
    "ObjectReference",
];

/// Default value for a leaf override type.
fn override_default_value(t: &str) -> Option<String> {
    match t {
        "Float" => Some("0".to_string()),
        "Integer" | "Enum" | "ObjectReference" => Some("0".to_string()),
        "Boolean" => Some("False".to_string()),
        "String" => Some(String::new()),
        _ => None,
    }
}

/// Default children for compound/container override types.
fn override_default_children(t: &str) -> Vec<OverrideNode> {
    match t {
        "Array" => vec![OverrideNode {
            node_type: "ArraySize".to_string(),
            name: "size".to_string(),
            value: Some("0".to_string()),
            children: Vec::new(),
        }],
        "Vector2" => ["x", "y"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Vector3" => ["x", "y", "z"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Quaternion" => ["x", "y", "z", "w"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Color" => ["r", "g", "b", "a"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Rect" => ["x", "y", "width", "height"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Keyframe" => ["time", "value", "inTangent", "outTangent"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Inline form for adding a new override node. Returns true if tree changed.
fn show_add_node_form(
    ui: &mut egui::Ui,
    nodes: &mut Vec<OverrideNode>,
    add_id: egui::Id,
    adding: &mut bool,
    depth: usize,
    t: &'static crate::locale::I18n,
) -> bool {
    let type_id = ui.make_persistent_id(format!("{:?}_type", add_id));
    let name_id = ui.make_persistent_id(format!("{:?}_name", add_id));
    let mut selected_idx: usize = ui.data(|d| d.get_temp(type_id).unwrap_or(0));
    let mut name_buf: String = ui.data(|d| d.get_temp::<String>(name_id).unwrap_or_default());
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 12.0);
        egui::ComboBox::from_id_salt(format!("{:?}_combo", add_id))
            .width(90.0)
            .selected_text(OVERRIDE_ALL_TYPES[selected_idx])
            .show_index(ui, &mut selected_idx, OVERRIDE_ALL_TYPES.len(), |i| {
                OVERRIDE_ALL_TYPES[i].to_string()
            });
        ui.data_mut(|d| d.insert_temp(type_id, selected_idx));

        let resp = ui.add(
            egui::TextEdit::singleline(&mut name_buf)
                .desired_width(60.0)
                .hint_text(t.get("override_name_hint")),
        );
        ui.data_mut(|d| d.insert_temp(name_id, name_buf.clone()));

        if ui.small_button(t.get("btn_confirm")).clicked()
            || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
        {
            let ty = OVERRIDE_ALL_TYPES[selected_idx];
            let name = if name_buf.trim().is_empty() {
                "unnamed".to_string()
            } else {
                name_buf.trim().to_string()
            };
            nodes.push(OverrideNode {
                node_type: ty.to_string(),
                name,
                value: override_default_value(ty),
                children: override_default_children(ty),
            });
            *adding = false;
            ui.data_mut(|d| d.insert_temp(add_id, false));
            ui.data_mut(|d| d.remove_by_type::<usize>());
            changed = true;
        }
        if ui.small_button(t.get("btn_cancel")).clicked() {
            *adding = false;
            ui.data_mut(|d| d.insert_temp(add_id, false));
        }
    });

    changed
}
