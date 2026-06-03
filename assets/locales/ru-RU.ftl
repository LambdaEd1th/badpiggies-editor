locale_native_name = Русский

# Menu bar
menu_file = Файл
tab_untitled = (без названия)
menu_open_level = Открыть файл уровня…
menu_export_from_unity3d = Экспортировать из файла unity3d…
menu_export_level = Экспортировать уровень
menu_import_to_unity3d = Импортировать в файл unity3d…
menu_import_text = Импортировать YAML/TOML…
menu_export_yaml = Экспортировать как YAML
menu_export_toml = Экспортировать как TOML
menu_edit = Правка
menu_undo = Отменить
menu_redo = Повторить
menu_copy = Копировать
menu_cut = Вырезать
menu_paste = Вставить
menu_duplicate = Дублировать
menu_flip_horizontal = Отразить горизонтально
menu_flip_vertical = Отразить вертикально
menu_delete = Удалить
menu_clear_selection = Очистить выделение
menu_expand_all = Развернуть всё
menu_collapse_all = Свернуть всё
menu_add_object = Добавить объект…
menu_view = Вид
menu_fit_view = Подогнать к экрану
menu_background = Фон
menu_object_list = Список объектов
menu_properties = Свойства
menu_grid = Сетка
menu_physics_ground = Физическая земля
menu_level_bounds = Границы уровня
menu_dark_overlay = Тёмный оверлей
menu_terrain_tris = Треугольники местности
menu_language = Язык
menu_help = Справка
menu_shortcuts = Клавиатурные сокращения
menu_about = О программе
menu_export_log = Экспортировать журнал

# Window titles
win_confirm_delete = Подтвердить удаление
win_level_warning = Предупреждение об известном риске
win_shortcuts = Клавиатурные сокращения
win_about = О программе
win_add_object = Добавить объект
win_export_from_unity3d = Экспортировать из файла unity3d
win_import_to_unity3d = Импортировать в файл unity3d

# Buttons
btn_ok = ОК
btn_cancel = Отмена
btn_confirm = Подтвердить
btn_i_understand_the_risks = Я понимаю риски
btn_add = + Добавить
btn_open_selected = Открыть выбранное
btn_import_current_level = Импортировать текущий уровень
btn_select_all = Выделить всё
btn_visual = Визуальный
btn_clear_all = Очистить выделение
btn_text = Текст

# unity3d dialogs
label_unity3d_file = Файл unity3d:
label_current_level = Текущий уровень:
label_unity3d_entries = Записи

# Shortcuts window
shortcuts_key = Сокращение
shortcuts_action = Действие
shortcuts_section_mouse = Мышь
shortcuts_scroll = Колесо прокрутки
shortcuts_zoom = Масштабирование
shortcuts_drag = Перетаскивание (пустая область)
shortcuts_pan = Панорамирование
shortcuts_click = Щелчок на объекте
shortcuts_select = Выделение объекта
shortcuts_cmd_click_action = Переключение выделения
shortcuts_shift_click = Shift+щелчок
shortcuts_shift_click_action = Диапазонное выделение
shortcuts_section_keyboard = Клавиатурные сокращения
shortcuts_b_key = B
shortcuts_toggle_bg = Переключить фон
shortcuts_undo_action = Отменить
shortcuts_redo_action = Повторить
shortcuts_copy_action = Копировать объект
shortcuts_cut_action = Вырезать объект
shortcuts_paste_action = Вставить объект
shortcuts_duplicate_action = Дублировать
shortcuts_delete_action = Удалить объект
shortcuts_section_terrain = Редактирование местности
shortcuts_terrain_select = Щелчок на местности
shortcuts_terrain_select_action = Показать узлы кривой
shortcuts_terrain_drag = Перетащить узел
shortcuts_terrain_drag_action = Переместить узел кривой
shortcuts_terrain_dblclick = Двойной щелчок на сегменте
shortcuts_terrain_dblclick_action = Вставить новый узел
shortcuts_terrain_delete = Удалить / Backspace на узле
shortcuts_terrain_delete_action = Удалить узел (мин. 3)
shortcuts_terrain_rclick = Правый щелчок на узле
shortcuts_terrain_rclick_action = Открыть меню узла

# About window
about_built_with = Создано с помощью eframe / egui / wgpu
about_license = Лицензия: GNU AGPL v3.0
about_version_prefix = Версия: 

# Add object dialog
add_type = Тип:
add_name = Название:
add_search = Поиск:
add_search_hint = Фильтровать названия префабов
add_search_no_matches = Нет совпадающих префабов
add_prefab_index = Индекс префаба:
add_data_type = Тип данных:
add_data_type_none = Нет
add_data_type_terrain = Местность
add_data_type_prefab_overrides = PrefabOverrides

# Panels
panel_object_list = Список объектов
panel_properties = Свойства
panel_select_hint = Выберите объект для просмотра свойств
panel_drop_hint = Перетащите файл .bytes сюда
panel_open_hint = или используйте Файл > Открыть файл уровня…

# Properties view
prop_type_prefab = Тип: Префаб
prop_type_parent = Тип: Родитель
prop_name = Название:
prop_prefab_index = Индекс префаба:
prop_position = Позиция
prop_rotation = Вращение
prop_scale = Масштаб
prop_data_type = Тип данных:
prop_terrain = Данные местности
prop_fill_tex_index = Индекс текстуры заполнения:
prop_fill_vert_count = Количество вершин заполнения:
prop_curve_vert_count = Количество вершин кривой:
prop_collider = Коллайдер:
prop_fill_color = Цвет заполнения
prop_override = Данные переопределения
prop_child_count = Количество дочерних элементов:
prop_fill_offset_x = Смещение заполнения X:
prop_fill_offset_y = Смещение заполнения Y:
prop_curve_tex = Текстура кривой { $idx }
prop_strip_width = Ширина полосы:
prop_fade_threshold = Порог затухания:
prop_terrain_closed = Замкнутый цикл:

# Status messages
status_welcome = Откройте файл уровня .bytes для начала редактирования
status_loaded = Загружено: { $obj_count } объектов, { $root_count } корней
status_exported = Успешно экспортировано
status_unity3d_imported = Текущий уровень импортирован в файл unity3d
status_unity3d_no_text_assets = В этом файле unity3d не найдено записей TextAsset
status_added = Добавлено: { $name }
status_deleted = Удалено: { $name }
status_delete_confirm = Удалить «{ $name }»?
status_read_error = Ошибка чтения: { $name }
status_export_error = Ошибка экспорта: { $name }
status_parse_error = Ошибка парсинга: { $name }
status_utf8_decode_failed = Не удалось декодировать UTF-8
level_warning_intro_preview = Этот уровень содержит известные рискованные комбинации объектов. Запуск предпросмотра может не соответствовать поведению оригинальной игры.
level_warning_intro_export = Этот уровень содержит известные рискованные комбинации объектов. Его экспорт может создать уровень, который сломает оригинальную игру.
level_warning_intro_editor = Это редактирование добавило известные рискованные комбинации объектов. Оригинальная игра может вести себя непредсказуемо или сломаться.
level_warning_intro_preview_low = Этот уровень содержит известные проблемы с объектами. Предпросмотр может пропустить дополнительное поведение оригинальной игры.
level_warning_intro_export_low = Этот уровень содержит известные проблемы с объектами. Его экспорт может отключить дополнительное поведение оригинальной игры без необходимого разрушения уровня.
level_warning_intro_editor_low = Это редактирование добавило известные проблемы с объектами. Оригинальная игра может пропустить дополнительные системы или вести себя иначе в некоторых ситуациях.
level_warning_section_high = Высокий риск
level_warning_section_low = Меньший эффект
level_warning_badge_high = ВЫСОКИЙ
level_warning_badge_low = НИЗКИЙ
level_warning_multiple_camera_system = Найдено { $count } объект(ов) «{ $name }». Каждый CameraSystem создаёт объекты GameCamera и HUDCamera, в то время как оригинальная игра позже использует единичные поиски камер, такие как Camera.main, HUDCamera, GameCamera и CameraSystem по названию.
level_warning_multiple_game_camera = Найдено { $count } объект(ов) «{ $name }». Оригинальная игра ожидает одну MainCamera/GameCamera и использует единичные поиски, такие как Camera.main, FindGameObjectWithTag(«MainCamera»), и GameObject.Find(«GameCamera»).
level_warning_multiple_hud_camera = Найдено { $count } объект(ов) «{ $name }». Оригинальная игра ожидает одну HUD камеру и многократно использует FindGameObjectWithTag(«HUDCamera») без обработки неоднозначности.
level_warning_multiple_world_object = Найдено { $count } объект(ов) «{ $name }». LevelManager разрешает одиночный объект фона с тегом World, а затем читает его PositionSerializer или название без разрешения неоднозначности.
level_warning_multiple_singleton = Найдено { $count } объект(ов) «{ $name }». Оригинальная игра использует поиски сцены подобные синглтону для этого объекта и может выбрать произвольный экземпляр или сломаться когда существует несколько.
level_warning_missing_level_manager = Объект «{ $name }» не найден. Многие системы оригинальной игры разыменовывают LevelManager напрямую, поэтому предпросмотр/экспорт геймплея может сразу сломаться.
level_warning_missing_level_start = Объект «{ $name }» не найден. Оригинальная игра переходит к исходной позиции по умолчанию для размещения контрапции и настройки камеры, что может привести к нарушению уровня.
level_warning_missing_camera_system = Объект «{ $name }» не найден. Оригинальная игра ожидает, что CameraSystem создаст GameCamera и HUDCamera, затем разрешит камеры через единичные поиски, такие как Camera.main и HUDCamera.
level_warning_missing_world_object = Объект «{ $name }» не найден. LevelManager ищет одиночный объект фона с тегом World при загрузке и сразу читает его PositionSerializer или название.
level_warning_missing_goal_area = Объект цели, соответствующий «{ $name }», не найден. В не-песочнице уровнях оригинальная игра разрешает одиночную цель с тегом Goal для завершения, поэтому завершение уровня может быть невозможно.
level_warning_multiple_goal_area = Найдено { $count } объект(ов) цели, соответствующих «{ $name }». LevelManager разрешает цель через единичный поиск по тегу Goal и может использовать произвольный когда существует несколько целей.
level_warning_missing_dessert_places = Объект «{ $name }» не найден. Поставляемые уровни последовательно включают этот корень, но BaseGameMode мягко пропускает размещение десерта когда он отсутствует, поэтому появление десерта может быть отключено вместо разрушения.
level_warning_multiple_dessert_places = Найдено { $count } объект(ов) «{ $name }». BaseGameMode разрешает одиночный корень DessertPlaces через GameObject.Find(«DessertPlaces»), поэтому несколько корней могут загружать размещение десерта из произвольной ветви.
level_warning_continue = Продолжить в любом случае?

# App errors
app_error_io = Ошибка ввода-вывода: { $name }
app_error_invalid_data = Неверные данные: { $name }
app_error_crypto = Ошибка криптографии: { $name }
app_error_browser = Ошибка браузера: { $name }
app_error_state = Ошибка внутреннего состояния: { $name }
error_unknown_file_type = Неизвестный тип файла
error_unsupported_file_format = Неподдерживаемый формат файла
error_parse_yaml_level = Не удалось разобрать уровень YAML: { $name }
error_parse_toml_level = Не удалось разобрать уровень TOML: { $name }
error_serialize_yaml_level = Не удалось сериализовать уровень YAML: { $name }
error_serialize_toml_level = Не удалось сериализовать уровень TOML: { $name }
error_xml_parse = Ошибка парсинга XML: { $name }
error_invalid_utf8 = Неверная кодировка UTF-8: { $name }
error_aes_decrypt_failed = Ошибка расшифровки AES: { $name }
error_file_too_short_sha1 = Файл слишком короток для хеша SHA1
error_sha1_mismatch = Несоответствие хеша SHA1 — файл может быть повреждён
error_browser_api_call_failed = Вызов API браузера не удался: { $name }
error_download_link_unavailable = Не удалось создать ссылку для скачивания
error_window_unavailable = Окно недоступно
error_document_unavailable = Документ недоступен
error_document_body_unavailable = Тело документа недоступно
error_terrain_control_png_encode = Не удалось закодировать PNG управления местностью: { $name }

# Override tree
override_name_hint = Название

# CLI
cli_read_error = Не удалось прочитать { $path }: { $error }
cli_parse_error = Не удалось разобрать { $path }: { $error }
cli_unsupported_input = Неподдерживаемый формат входа: .{ $name }
cli_serialize_yaml_error = Не удалось сериализовать YAML: { $name }
cli_serialize_toml_error = Не удалось сериализовать TOML: { $name }
cli_unsupported_output = Неподдерживаемый формат выхода: .{ $name }
cli_write_error = Не удалось записать в { $path }: { $error }
cli_detect_save_type_error_input = Не удалось определить тип файла сохранения из имени входного файла «{ $name }». Используйте --type progress|contraption|achievements
cli_detect_save_type_error_output = Не удалось определить тип файла сохранения из имени выходного файла «{ $name }». Используйте --type progress|contraption|achievements
cli_stdout_write_error = Не удалось записать на стандартный вывод: { $name }
cli_convert_ok = { $input } -> { $output } ({ $obj_count } объектов, { $root_count } корней)
cli_decrypt_ok = Расшифровано { $input } ({ $type }) -> { $output } ({ $bytes } байтов)
cli_encrypt_ok = Зашифровано { $input } -> { $output } ({ $type }, { $bytes } байтов)
cli_error_prefix = Ошибка: { $name }

# Tabs
menu_close_tab = Закрыть вкладку

# HUD overlay
hud_zoom = Масштаб
hud_theme = Тема
hud_unknown_theme = Неизвестная тема

# Tool modes
tool_select = Выделение
tool_box_select = Выделение по области
tool_draw_terrain = Рисовать местность
tool_pan = Панорамирование
tool_window_title = Инструменты
tool_preview_title = Состояние предпросмотра
tool_preview_dark_overlay_title = Предпросмотр тёмного оверлея
tool_preview_build = Построение
tool_preview_play = Воспроизведение
tool_preview_pause = Пауза
tool_preview_night_vision = Ночное видение
tool_terrain_presets = Предустановки местности
tool_terrain_preset_circle = Эллипс
tool_terrain_preset_rectangle = Прямоугольник
tool_terrain_preset_perfect_circle = Идеальный круг
tool_terrain_preset_square = Квадрат
tool_terrain_preset_equilateral_triangle = Равносторонний треугольник
tool_terrain_curve_segments = Узлы кривой
tool_terrain_draw_mode = Режим рисования
tool_terrain_draw_mode_curve = Коническая кривая
tool_terrain_draw_mode_horizontal = Горизонталь
tool_terrain_draw_mode_vertical = Вертикаль
tool_terrain_draw_splat = Splat рисования
tool_terrain_draw_splat0 = Splat0
tool_terrain_draw_splat1 = Splat1

# Tool mode shortcuts
shortcuts_section_tools = Режимы инструментов
shortcuts_tool_select = V
shortcuts_tool_box_select = M
shortcuts_tool_draw_terrain = P
shortcuts_tool_pan = H

# Level bounds editor

# Save file viewer
menu_open_save = Открыть файл сохранения…
save_viewer_type = Тип
save_viewer_size = Размер
save_viewer_filter = Фильтр:
save_viewer_status_type_bytes = { $type }: { $bytes } байтов
save_viewer_status_file_type_bytes = { $file_name }: { $type }, { $bytes } байтов
save_viewer_raw_xml = Сырой XML
save_viewer_structured = Структурированный вид
save_viewer_no_data = Данные не загружены
save_file_type_unknown = Неизвестно
save_file_type_progress = Прогресс
save_file_type_contraption = Контрапция
save_file_type_achievements = Достижения
save_viewer_part_count = Детали
save_viewer_completed = завершён
save_col_key = Ключ
save_col_type = Тип
save_col_value = Значение
save_col_part_type = Тип детали
save_col_custom_idx = Пользовательский индекс
save_col_rot = Вращение
save_col_flipped = Перевёрнуто
save_col_progress = Прогресс
save_col_completed = Завершено
save_col_synced = Синхронизировано
save_editor_modified = Изменено
save_editor_parse_xml = Разобрать XML
save_editor_add_entry = + Добавить запись
menu_export_save = Экспортировать файл сохранения…
menu_import_xml = Импортировать сохранение XML…
menu_export_xml = Экспортировать сохранение XML…
save_editor_regex_err = Неверное регулярное выражение
save_filter_hint = фильтр (регулярное выражение)
menu_select_all = Выделить всё
save_edit_deselect_all = Снять выделение со всех
save_edit_delete_selected = Удалить выбранное
save_edit_duplicate_selected = Дублировать выбранное
save_viewer_reveal_xml = Показать в XML
context_toggle_node_texture = Переключить текстуру узла
contraption_preview_title = Предпросмотр контрапции
