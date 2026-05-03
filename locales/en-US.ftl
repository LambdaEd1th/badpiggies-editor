# Menu bar
menu_file = File
tab_untitled = (untitled)
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
menu_clear_selection = Clear Selection
menu_expand_all = Expand All
menu_collapse_all = Collapse All
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
shortcuts_cmd_click_action = Toggle Selection
shortcuts_shift_click = Shift+Click
shortcuts_shift_click_action = Range Select
shortcuts_section_keyboard = Keyboard Shortcuts
shortcuts_b_key = B
shortcuts_toggle_bg = Toggle Background
shortcuts_undo_action = Undo
shortcuts_redo_action = Redo
shortcuts_copy_action = Copy Object
shortcuts_cut_action = Cut Object
shortcuts_paste_action = Paste Object
shortcuts_duplicate_action = Duplicate
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
shortcuts_terrain_rclick_action = Open Node Menu

# About window
about_built_with = Built with eframe / egui / wgpu
about_license = License: GNU AGPL v3.0
about_version_prefix = Version: 

# Add object dialog
add_type = Type:
add_name = Name:
add_prefab_index = Prefab Index:
add_kind_prefab = Prefab
add_kind_parent = Parent
add_data_type = Data Type:
add_data_type_none = None
add_data_type_terrain = Terrain
add_data_type_prefab_overrides = PrefabOverrides
add_data_type_terrain_help = Terrain creates default terrain data and keeps the prefab index you selected.
add_data_type_prefab_overrides_help = PrefabOverrides creates an empty override payload that you can edit later in the properties panel.

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
status_parse_error = Parse error: { $name }

# App errors
app_error_io = I/O error: { $name }
app_error_invalid_data = Invalid data: { $name }
app_error_crypto = Cryptography error: { $name }
app_error_browser = Browser error: { $name }
app_error_state = Internal state error: { $name }
error_unknown_file_type = Unknown file type
error_unsupported_file_format = Unsupported file format
error_parse_yaml_level = Failed to parse YAML level: { $name }
error_parse_toml_level = Failed to parse TOML level: { $name }
error_serialize_yaml_level = Failed to serialize YAML level: { $name }
error_serialize_toml_level = Failed to serialize TOML level: { $name }
error_xml_parse = XML parse error: { $name }
error_invalid_utf8 = Invalid UTF-8: { $name }
error_pbkdf2_init = Failed to initialize PBKDF2 HMAC-SHA1
error_aes_encrypt_buffer_too_small = AES encryption buffer too small
error_aes_decrypt_failed = AES decryption failed: { $name }
error_file_too_short_sha1 = File too short for SHA1 hash
error_sha1_mismatch = SHA1 hash mismatch — file may be corrupted
error_browser_api_call_failed = Browser API call failed: { $name }
error_download_link_unavailable = Unable to create download link
error_window_unavailable = Window unavailable
error_document_unavailable = Document unavailable
error_document_body_unavailable = Document body unavailable
error_icon_layers_missing = Embedded asset icon-layers.toml is missing
error_icon_layers_not_utf8 = icon-layers.toml is not valid UTF-8: { $name }
error_icon_layers_parse = Failed to parse icon-layers.toml: { $name }
error_bg_data_parse = Failed to parse bg-data.toml: { $name }
error_sprite_data_parse = Failed to parse sprite-data.toml: { $name }
error_level_refs_parse = Failed to parse level-refs.toml: { $name }
error_terrain_control_png_encode = Failed to encode terrain control PNG: { $name }

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
cli_detect_save_type_error_input = Cannot detect save file type from input filename "{ $name }". Use --type progress|contraption|achievements
cli_detect_save_type_error_output = Cannot detect save file type from output filename "{ $name }". Use --type progress|contraption|achievements
cli_stdout_write_error = Failed to write to stdout: { $name }
cli_convert_ok = { $input } -> { $output } ({ $obj_count } objects, { $root_count } roots)
cli_decrypt_ok = Decrypted { $input } ({ $type }) -> { $output } ({ $bytes } bytes)
cli_encrypt_ok = Encrypted { $input } -> { $output } ({ $type }, { $bytes } bytes)
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
tool_terrain_presets = Terrain Presets
tool_terrain_preset_circle = Ellipse
tool_terrain_preset_rectangle = Rectangle
tool_terrain_preset_perfect_circle = Perfect Circle
tool_terrain_preset_square = Square
tool_terrain_preset_equilateral_triangle = Equilateral Triangle
tool_terrain_round_segments = Ellipse/Circle Nodes

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

# Save file viewer
menu_open_save = Open Save File…
save_viewer_title = Save Viewer
save_viewer_type = Type
save_viewer_size = Size
save_viewer_filter = Filter:
save_viewer_status_type_bytes = { $type }: { $bytes } bytes
save_viewer_status_file_type_bytes = { $file_name }: { $type }, { $bytes } bytes
save_viewer_raw_xml = Raw XML
save_viewer_structured = Structured View
save_viewer_no_data = No data loaded
save_file_type_unknown = Unknown
save_file_type_progress = Progress
save_file_type_contraption = Contraption
save_file_type_achievements = Achievements
save_viewer_part_count = Parts
save_viewer_completed = completed
save_col_key = Key
save_col_type = Type
save_col_value = Value
save_col_part_type = Part Type
save_col_custom_idx = Custom Index
save_col_rot = Rotation
save_col_flipped = Flipped
save_col_progress = Progress
save_col_completed = Completed
save_col_synced = Synced
save_editor_modified = Modified
save_editor_parse_xml = Parse XML
save_editor_add_entry = + Add Entry
menu_export_save = Export Save File…
menu_import_xml = Import XML Save…
menu_export_xml = Export XML Save…
save_editor_regex_err = Invalid regex
save_filter_hint = filter (regex)
save_edit_clear_all = Clear All Entries
save_edit_duplicate_all = Duplicate All Entries
menu_select_all = Select All
save_edit_deselect_all = Deselect All
save_edit_delete_selected = Delete Selected
save_edit_duplicate_selected = Duplicate Selected
save_viewer_reveal_xml = Reveal in XML
context_toggle_node_texture = Toggle Node Texture
contraption_preview_title = Contraption Preview
