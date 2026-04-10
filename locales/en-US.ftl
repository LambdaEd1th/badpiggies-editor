# Menu bar
menu_file = File
menu_open_level = Open Level File…
menu_export_level = Export Level
menu_import_text = Import YAML/TOML…
menu_export_yaml = Export as YAML
menu_export_toml = Export as TOML
menu_edit = Edit
menu_undo = Undo
menu_redo = Redo
menu_copy = Copy
menu_cut = Cut
menu_paste = Paste
menu_duplicate = Duplicate
menu_delete = Delete
menu_add_object = Add Object…
menu_view = View
menu_fit_view = Fit to View
menu_background = Background
menu_object_list = Object List
menu_properties = Properties
menu_grid = Grid
menu_physics_ground = Physics Ground
menu_level_bounds = Level Bounds
menu_dark_overlay = Dark Overlay
menu_terrain_tris = Terrain Triangles
menu_language = Language
menu_help = Help
menu_shortcuts = Shortcuts
menu_about = About
menu_export_log = Export Log

# Window titles
win_confirm_delete = Confirm Delete
win_shortcuts = Keyboard Shortcuts
win_about = About
win_add_object = Add Object

# Buttons
btn_ok = OK
btn_cancel = Cancel
btn_confirm = Confirm
btn_add = + Add
btn_visual = Visual
btn_text = Text

# Shortcuts window
shortcuts_key = Shortcut
shortcuts_action = Action
shortcuts_section_mouse = Mouse
shortcuts_scroll = Scroll Wheel
shortcuts_zoom = Zoom View
shortcuts_drag = Drag (empty area)
shortcuts_pan = Pan View
shortcuts_click = Click Object
shortcuts_select = Select Object
shortcuts_cmd_click = ⌘+Click / Ctrl+Click
shortcuts_cmd_click_action = Toggle Selection
shortcuts_shift_click = Shift+Click
shortcuts_shift_click_action = Range Select
shortcuts_section_keyboard = Keyboard Shortcuts
shortcuts_b_key = B
shortcuts_toggle_bg = Toggle Background
shortcuts_undo = ⌘Z / Ctrl+Z
shortcuts_undo_action = Undo
shortcuts_redo = Shift+⌘+Z / Ctrl+Shift+Z
shortcuts_redo_action = Redo
shortcuts_copy = ⌘C / Ctrl+C
shortcuts_copy_action = Copy Object
shortcuts_cut = ⌘X / Ctrl+X
shortcuts_cut_action = Cut Object
shortcuts_paste = ⌘V / Ctrl+V
shortcuts_paste_action = Paste Object
shortcuts_duplicate = ⌘D / Ctrl+D
shortcuts_duplicate_action = Duplicate Object
shortcuts_delete = Delete / Backspace
shortcuts_delete_action = Delete Object
shortcuts_section_terrain = Terrain Editing
shortcuts_terrain_select = Click Terrain
shortcuts_terrain_select_action = Show Curve Nodes
shortcuts_terrain_drag = Drag Node
shortcuts_terrain_drag_action = Move Curve Node
shortcuts_terrain_dblclick = Double-click Segment
shortcuts_terrain_dblclick_action = Insert New Node
shortcuts_terrain_delete = Delete / Backspace on Node
shortcuts_terrain_delete_action = Delete Node (min 3)
shortcuts_terrain_rclick = Right-click Node
shortcuts_terrain_rclick_action = Toggle Node Texture

# About window
about_built_with = Built with eframe / egui / wgpu
about_license = License: GNU AGPL v3.0
about_version_prefix = Version: 

# Add object dialog
add_type = Type:
add_name = Name:
add_prefab_index = Prefab Index:

# Panels
panel_object_list = Object List
panel_properties = Properties
panel_select_hint = Select an object to view properties
panel_drop_hint = Drop a .bytes file here
panel_open_hint = or use  File > Open Level File…

# Properties view
prop_type_prefab = Type: Prefab
prop_type_parent = Type: Parent
prop_name = Name:
prop_prefab_index = Prefab Index:
prop_position = Position
prop_rotation = Rotation
prop_scale = Scale
prop_data_type = Data Type:
prop_terrain = Terrain Data
prop_fill_tex_index = Fill Texture Index:
prop_fill_vert_count = Fill Vertex Count:
prop_curve_vert_count = Curve Vertex Count:
prop_curve_tex_count = Curve Texture Count:
prop_collider = Collider:
prop_fill_color = Fill Color
prop_override = Override Data
prop_byte_count = Byte Count:
prop_child_count = Child Count:
prop_fill_offset_x = Fill Offset X:
prop_fill_offset_y = Fill Offset Y:
prop_curve_tex = Curve Texture { $idx }
prop_strip_width = Strip Width:
prop_fade_threshold = Fade Threshold:
prop_terrain_closed = Closed Loop:
add_terrain = Attach Terrain Data

# Status messages
status_welcome = Open a .bytes level file to start editing
status_loaded = Loaded: { $obj_count } objects, { $root_count } roots
status_exported = Exported successfully
status_added = Added: { $name }
status_delete_confirm = Delete "{ $name }"?
status_read_error = Read error: { $name }
status_export_error = Export error: { $name }

# Override tree
override_name_hint = Name

# CLI
cli_read_error = Failed to read { $path }: { $error }
cli_parse_error = Failed to parse { $path }: { $error }
cli_unsupported_input = Unsupported input format: .{ $name }
cli_serialize_yaml_error = Failed to serialize YAML: { $name }
cli_serialize_toml_error = Failed to serialize TOML: { $name }
cli_unsupported_output = Unsupported output format: .{ $name }
cli_write_error = Failed to write { $path }: { $error }
cli_convert_ok = { $input } -> { $output } ({ $obj_count } objects, { $root_count } roots)
cli_error_prefix = Error: { $name }

# Tabs
menu_close_tab = Close Tab

# HUD overlay
hud_zoom = Zoom
hud_theme = Theme
hud_unknown_theme = Unknown

# Tool modes
tool_select = Select
tool_box_select = Box Select
tool_draw_terrain = Draw Terrain
tool_pan = Pan
tool_window_title = Tools

# Tool mode shortcuts
shortcuts_section_tools = Tool Modes
shortcuts_tool_select = V
shortcuts_tool_box_select = M
shortcuts_tool_draw_terrain = P
shortcuts_tool_pan = H

# Level bounds editor
menu_edit_level_bounds = Edit Level Bounds…
win_level_bounds = Level Bounds
bounds_pos_x = Position X:
bounds_pos_y = Position Y:
bounds_size_w = Width:
bounds_size_h = Height:
