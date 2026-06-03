locale_native_name = Español

# Menu bar
menu_file = Archivo
tab_untitled = (sin título)
menu_open_level = Abrir archivo de nivel…
menu_export_from_unity3d = Exportar desde archivo unity3d…
menu_export_level = Exportar nivel
menu_import_to_unity3d = Importar a archivo unity3d…
menu_import_text = Importar YAML/TOML…
menu_export_yaml = Exportar como YAML
menu_export_toml = Exportar como TOML
menu_edit = Edición
menu_undo = Deshacer
menu_redo = Rehacer
menu_copy = Copiar
menu_cut = Cortar
menu_paste = Pegar
menu_duplicate = Duplicar
menu_flip_horizontal = Voltear horizontalmente
menu_flip_vertical = Voltear verticalmente
menu_delete = Eliminar
menu_clear_selection = Limpiar selección
menu_expand_all = Expandir todo
menu_collapse_all = Contraer todo
menu_add_object = Añadir objeto…
menu_view = Ver
menu_fit_view = Ajustar a la vista
menu_background = Fondo
menu_object_list = Lista de objetos
menu_properties = Propiedades
menu_grid = Cuadrícula
menu_physics_ground = Suelo físico
menu_level_bounds = Límites del nivel
menu_dark_overlay = Superposición oscura
menu_terrain_tris = Triángulos del terreno
menu_language = Idioma
menu_help = Ayuda
menu_shortcuts = Atajos de teclado
menu_about = Acerca de
menu_export_log = Exportar registro

# Window titles
win_confirm_delete = Confirmar eliminación
win_level_warning = Advertencia de riesgo conocido
win_shortcuts = Atajos de teclado
win_about = Acerca de
win_add_object = Añadir objeto
win_export_from_unity3d = Exportar desde archivo unity3d
win_import_to_unity3d = Importar a archivo unity3d

# Buttons
btn_ok = Aceptar
btn_cancel = Cancelar
btn_confirm = Confirmar
btn_i_understand_the_risks = Entiendo los riesgos
btn_add = + Añadir
btn_open_selected = Abrir selección
btn_import_current_level = Importar nivel actual
btn_select_all = Seleccionar todo
btn_visual = Visual
btn_clear_all = Limpiar selección
btn_text = Texto

# unity3d dialogs
label_unity3d_file = Archivo unity3d:
label_current_level = Nivel actual:
label_unity3d_entries = Entradas

# Shortcuts window
shortcuts_key = Atajo
shortcuts_action = Acción
shortcuts_section_mouse = Ratón
shortcuts_scroll = Rueda de desplazamiento
shortcuts_zoom = Zoom de vista
shortcuts_drag = Arrastrar (área vacía)
shortcuts_pan = Panorámica
shortcuts_click = Hacer clic en objeto
shortcuts_select = Seleccionar objeto
shortcuts_cmd_click_action = Alternar selección
shortcuts_shift_click = Mayús+clic
shortcuts_shift_click_action = Selección de rango
shortcuts_section_keyboard = Atajos de teclado
shortcuts_b_key = B
shortcuts_toggle_bg = Alternar fondo
shortcuts_undo_action = Deshacer
shortcuts_redo_action = Rehacer
shortcuts_copy_action = Copiar objeto
shortcuts_cut_action = Cortar objeto
shortcuts_paste_action = Pegar objeto
shortcuts_duplicate_action = Duplicar
shortcuts_delete_action = Eliminar objeto
shortcuts_section_terrain = Edición del terreno
shortcuts_terrain_select = Hacer clic en terreno
shortcuts_terrain_select_action = Mostrar nodos de curva
shortcuts_terrain_drag = Arrastrar nodo
shortcuts_terrain_drag_action = Mover nodo de curva
shortcuts_terrain_dblclick = Doble clic en segmento
shortcuts_terrain_dblclick_action = Insertar nodo nuevo
shortcuts_terrain_delete = Suprimir / Retroceso en nodo
shortcuts_terrain_delete_action = Eliminar nodo (mín. 3)
shortcuts_terrain_rclick = Clic derecho en nodo
shortcuts_terrain_rclick_action = Abrir menú de nodo

# About window
about_built_with = Creado con eframe / egui / wgpu
about_license = Licencia: GNU AGPL v3.0
about_version_prefix = Versión: 

# Add object dialog
add_type = Tipo:
add_name = Nombre:
add_search = Búsqueda:
add_search_hint = Filtrar nombres de prefabricated
add_search_no_matches = No hay prefabricados coincidentes
add_prefab_index = Índice de prefabricado:
add_data_type = Tipo de datos:
add_data_type_none = Ninguno
add_data_type_terrain = Terreno
add_data_type_prefab_overrides = PrefabOverrides

# Panels
panel_object_list = Lista de objetos
panel_properties = Propiedades
panel_select_hint = Seleccione un objeto para ver propiedades
panel_drop_hint = Suelte un archivo .bytes aquí
panel_open_hint = o use Archivo > Abrir archivo de nivel…

# Properties view
prop_type_prefab = Tipo: Prefabricado
prop_type_parent = Tipo: Principal
prop_name = Nombre:
prop_prefab_index = Índice de prefabricado:
prop_position = Posición
prop_rotation = Rotación
prop_scale = Escala
prop_data_type = Tipo de datos:
prop_terrain = Datos del terreno
prop_fill_tex_index = Índice de textura de relleno:
prop_fill_vert_count = Cantidad de vértices de relleno:
prop_curve_vert_count = Cantidad de vértices de curva:
prop_collider = Colisionador:
prop_fill_color = Color de relleno
prop_override = Datos de invalidación
prop_child_count = Cantidad de objetos secundarios:
prop_fill_offset_x = Desplazamiento de relleno X:
prop_fill_offset_y = Desplazamiento de relleno Y:
prop_curve_tex = Textura de curva { $idx }
prop_strip_width = Ancho de franja:
prop_fade_threshold = Umbral de desvanecimiento:
prop_terrain_closed = Bucle cerrado:

# Status messages
status_welcome = Abra un archivo de nivel .bytes para comenzar la edición
status_loaded = Cargado: { $obj_count } objetos, { $root_count } raíces
status_exported = Exportado exitosamente
status_unity3d_imported = Nivel actual importado al archivo unity3d
status_unity3d_no_text_assets = No se encontraron entradas TextAsset en este archivo unity3d
status_added = Agregado: { $name }
status_deleted = Eliminado: { $name }
status_delete_confirm = ¿Eliminar «{ $name }»?
status_read_error = Error de lectura: { $name }
status_export_error = Error de exportación: { $name }
status_parse_error = Error de análisis: { $name }
status_utf8_decode_failed = Error al decodificar UTF-8
level_warning_intro_preview = Este nivel contiene combinaciones de objetos de riesgo conocido. Iniciar vista previa puede no coincidir con el comportamiento del juego original.
level_warning_intro_export = Este nivel contiene combinaciones de objetos de riesgo conocido. Exportarlo puede crear un nivel que rompe el juego original.
level_warning_intro_editor = Esta edición introdujo combinaciones de objetos de riesgo conocido. El juego original puede comportarse de manera impredecible o romperse.
level_warning_intro_preview_low = Este nivel contiene problemas de objetos conocidos. La vista previa puede perder comportamiento opcional del juego original.
level_warning_intro_export_low = Este nivel contiene problemas de objetos conocidos. Exportarlo puede deshabilitar el comportamiento opcional del juego original sin necesariamente romper el nivel.
level_warning_intro_editor_low = Esta edición introdujo problemas de objetos conocidos. El juego original puede omitir sistemas opcionales o comportarse diferente en algunas situaciones.
level_warning_section_high = Riesgo alto
level_warning_section_low = Impacto menor
level_warning_badge_high = ALTO
level_warning_badge_low = BAJO
level_warning_multiple_camera_system = Se encontraron { $count } objeto(s) «{ $name }». Cada CameraSystem genera objetos GameCamera y HUDCamera, mientras que el juego original usa búsquedas singulares de cámaras como Camera.main, HUDCamera, GameCamera y CameraSystem por nombre.
level_warning_multiple_game_camera = Se encontraron { $count } objeto(s) «{ $name }». El juego original espera una MainCamera/GameCamera única y usa búsquedas singulares como Camera.main, FindGameObjectWithTag(«MainCamera») y GameObject.Find(«GameCamera»).
level_warning_multiple_hud_camera = Se encontraron { $count } objeto(s) «{ $name }». El juego original espera una cámara HUD única y usa repetidamente FindGameObjectWithTag(«HUDCamera») sin manejo de ambigüedad.
level_warning_multiple_world_object = Se encontraron { $count } objeto(s) «{ $name }». LevelManager resuelve un único objeto de fondo con etiqueta World y luego lee su PositionSerializer o nombre sin desambiguar.
level_warning_multiple_singleton = Se encontraron { $count } objeto(s) «{ $name }». El juego original usa búsquedas de escena tipo singleton para este objeto y puede elegir una instancia arbitraria o romperse cuando existen múltiples.
level_warning_missing_level_manager = No se encontró el objeto «{ $name }». Muchos sistemas del juego original derreferencian LevelManager directamente, por lo que la vista previa/exportación del gameplay puede romperse inmediatamente.
level_warning_missing_level_start = No se encontró el objeto «{ $name }». El juego original retrocede a un origen predeterminado para colocación de mecanismo y configuración de cámara, lo que puede producir comportamiento de nivel roto.
level_warning_missing_camera_system = No se encontró el objeto «{ $name }». El juego original espera que CameraSystem cree GameCamera y HUDCamera, luego resuelve cámaras mediante búsquedas singulares como Camera.main y HUDCamera.
level_warning_missing_world_object = No se encontró «{ $name }». LevelManager busca un único objeto de fondo etiquetado como World durante la carga y lee inmediatamente su PositionSerializer o nombre.
level_warning_missing_goal_area = No se encontró objeto de destino que coincida con «{ $name }». En niveles que no son sandbox, el juego original resuelve un único destino etiquetado como Goal para completar, por lo que terminar el nivel puede ser imposible.
level_warning_multiple_goal_area = Se encontraron { $count } objeto(s) destino que coinciden con «{ $name }». LevelManager resuelve el destino mediante una búsqueda única de etiqueta Goal y puede usar uno arbitrario cuando existen múltiples áreas de destino.
level_warning_missing_dessert_places = No se encontró el objeto «{ $name }». Los niveles enviados incluyen consistentemente esta raíz, pero BaseGameMode omite suavemente la colocación de postre cuando falta, por lo que el spawning de postre puede deshabilitarse en lugar de romperse.
level_warning_multiple_dessert_places = Se encontraron { $count } objeto(s) «{ $name }». BaseGameMode resuelve una única raíz DessertPlaces mediante GameObject.Find(«DessertPlaces»), por lo que múltiples raíces pueden hacer que la colocación de postre cargue desde una rama arbitraria.
level_warning_continue = ¿Continuar de todas formas?

# App errors
app_error_io = Error de E/S: { $name }
app_error_invalid_data = Datos inválidos: { $name }
app_error_crypto = Error criptográfico: { $name }
app_error_browser = Error del navegador: { $name }
app_error_state = Error de estado interno: { $name }
error_unknown_file_type = Tipo de archivo desconocido
error_unsupported_file_format = Formato de archivo no soportado
error_parse_yaml_level = Error al analizar nivel YAML: { $name }
error_parse_toml_level = Error al analizar nivel TOML: { $name }
error_serialize_yaml_level = Error al serializar nivel YAML: { $name }
error_serialize_toml_level = Error al serializar nivel TOML: { $name }
error_xml_parse = Error de análisis XML: { $name }
error_invalid_utf8 = UTF-8 inválido: { $name }
error_aes_decrypt_failed = Error de descifrado AES: { $name }
error_file_too_short_sha1 = Archivo demasiado corto para hash SHA1
error_sha1_mismatch = Desajuste de hash SHA1 — el archivo puede estar corrupto
error_browser_api_call_failed = Error en llamada de API del navegador: { $name }
error_download_link_unavailable = No se puede crear enlace de descarga
error_window_unavailable = Ventana no disponible
error_document_unavailable = Documento no disponible
error_document_body_unavailable = Cuerpo del documento no disponible
error_terrain_control_png_encode = Error al codificar PNG de control del terreno: { $name }

# Override tree
override_name_hint = Nombre

# CLI
cli_read_error = Error al leer { $path }: { $error }
cli_parse_error = Error al analizar { $path }: { $error }
cli_unsupported_input = Formato de entrada no soportado: .{ $name }
cli_serialize_yaml_error = Error al serializar YAML: { $name }
cli_serialize_toml_error = Error al serializar TOML: { $name }
cli_unsupported_output = Formato de salida no soportado: .{ $name }
cli_write_error = Error al escribir { $path }: { $error }
cli_detect_save_type_error_input = No se puede detectar tipo de archivo de guardado desde nombre de archivo de entrada «{ $name }». Use --type progress|contraption|achievements
cli_detect_save_type_error_output = No se puede detectar tipo de archivo de guardado desde nombre de archivo de salida «{ $name }». Use --type progress|contraption|achievements
cli_stdout_write_error = Error al escribir en salida estándar: { $name }
cli_convert_ok = { $input } -> { $output } ({ $obj_count } objetos, { $root_count } raíces)
cli_decrypt_ok = Descifrado { $input } ({ $type }) -> { $output } ({ $bytes } bytes)
cli_encrypt_ok = Cifrado { $input } -> { $output } ({ $type }, { $bytes } bytes)
cli_error_prefix = Error: { $name }

# Tabs
menu_close_tab = Cerrar pestaña

# HUD overlay
hud_zoom = Zoom
hud_theme = Tema
hud_unknown_theme = Tema desconocido

# Tool modes
tool_select = Seleccionar
tool_box_select = Selección de caja
tool_draw_terrain = Dibujar terreno
tool_pan = Panorámica
tool_window_title = Herramientas
tool_preview_title = Estado de vista previa
tool_preview_dark_overlay_title = Vista previa de superposición oscura
tool_preview_build = Construir
tool_preview_play = Reproducir
tool_preview_pause = Pausa
tool_preview_night_vision = Visión nocturna
tool_terrain_presets = Ajustes preestablecidos de terreno
tool_terrain_preset_circle = Elipse
tool_terrain_preset_rectangle = Rectángulo
tool_terrain_preset_perfect_circle = Círculo perfecto
tool_terrain_preset_square = Cuadrado
tool_terrain_preset_equilateral_triangle = Triángulo equilátero
tool_terrain_curve_segments = Nodos de curva
tool_terrain_draw_mode = Modo de dibujo
tool_terrain_draw_mode_curve = Curva cónica
tool_terrain_draw_mode_arc = Arco circular
tool_terrain_draw_mode_horizontal = Horizontal
tool_terrain_draw_mode_vertical = Vertical
tool_terrain_draw_splat = Splat de dibujo
tool_terrain_draw_splat0 = Splat0
tool_terrain_draw_splat1 = Splat1

# Tool mode shortcuts
shortcuts_section_tools = Modos de herramienta
shortcuts_tool_select = V
shortcuts_tool_box_select = M
shortcuts_tool_draw_terrain = P
shortcuts_tool_pan = H

# Level bounds editor

# Save file viewer
menu_open_save = Abrir archivo de guardado…
save_viewer_type = Tipo
save_viewer_size = Tamaño
save_viewer_filter = Filtro:
save_viewer_status_type_bytes = { $type }: { $bytes } bytes
save_viewer_status_file_type_bytes = { $file_name }: { $type }, { $bytes } bytes
save_viewer_raw_xml = XML sin procesar
save_viewer_structured = Vista estructurada
save_viewer_no_data = Sin datos cargados
save_file_type_unknown = Desconocido
save_file_type_progress = Progreso
save_file_type_contraption = Mecanismo
save_file_type_achievements = Logros
save_viewer_part_count = Piezas
save_viewer_completed = completado
save_col_key = Clave
save_col_type = Tipo
save_col_value = Valor
save_col_part_type = Tipo de pieza
save_col_custom_idx = Índice personalizado
save_col_rot = Rotación
save_col_flipped = Volteado
save_col_progress = Progreso
save_col_completed = Completado
save_col_synced = Sincronizado
save_editor_modified = Modificado
save_editor_parse_xml = Analizar XML
save_editor_add_entry = + Agregar entrada
menu_export_save = Exportar archivo de guardado…
menu_import_xml = Importar guardado XML…
menu_export_xml = Exportar guardado XML…
save_editor_regex_err = Expresión regular inválida
save_filter_hint = filtro (expresión regular)
menu_select_all = Seleccionar todo
save_edit_deselect_all = Deseleccionar todo
save_edit_delete_selected = Eliminar selección
save_edit_duplicate_selected = Duplicar selección
save_viewer_reveal_xml = Revelar en XML
context_toggle_node_texture = Alternar textura de nodo
contraption_preview_title = Vista previa de mecanismo
