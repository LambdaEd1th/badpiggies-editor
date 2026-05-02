# 菜单栏
menu_file = 文件
tab_untitled = (未命名)
menu_open_level = 打开关卡文件…
menu_export_level = 导出关卡
menu_import_text = 导入 YAML/TOML…
menu_export_yaml = 导出为 YAML
menu_export_toml = 导出为 TOML
menu_edit = 编辑
menu_undo = 撤销
menu_redo = 恢复
menu_copy = 复制
menu_cut = 剪切
menu_paste = 粘贴
menu_duplicate = 复制并粘贴
menu_delete = 删除
menu_clear_selection = 取消选择
menu_expand_all = 全部展开
menu_collapse_all = 全部折叠
menu_add_object = 添加对象…
menu_view = 视图
menu_fit_view = 适应视图
menu_background = 背景
menu_object_list = 对象列表
menu_properties = 属性面板
menu_grid = 网格
menu_physics_ground = 物理地面
menu_level_bounds = 关卡边界
menu_dark_overlay = 暗层覆盖
menu_terrain_tris = 地形三角剖分
menu_language = 语言
menu_help = 帮助
menu_shortcuts = 说明
menu_about = 关于
menu_export_log = 导出日志

# 窗口标题
win_confirm_delete = 确认删除
win_shortcuts = 快捷键说明
win_about = 关于
win_add_object = 添加对象

# 按钮
btn_ok = 确定
btn_cancel = 取消
btn_confirm = 确认
btn_add = + 添加
btn_visual = 可视化
btn_text = 文本

# 快捷键窗口
shortcuts_key = 操作
shortcuts_action = 功能
shortcuts_section_mouse = 鼠标操作
shortcuts_scroll = 滚轮
shortcuts_zoom = 缩放视图
shortcuts_drag = 拖拽（空白处）
shortcuts_pan = 平移视图
shortcuts_click = 点击对象
shortcuts_select = 选中对象
shortcuts_cmd_click_action = 切换选中
shortcuts_shift_click = Shift+点击
shortcuts_shift_click_action = 范围选择
shortcuts_section_keyboard = 键盘快捷键
shortcuts_b_key = B
shortcuts_toggle_bg = 切换背景显示
shortcuts_undo_action = 撤销
shortcuts_redo_action = 恢复
shortcuts_copy_action = 复制对象
shortcuts_cut_action = 剪切对象
shortcuts_paste_action = 粘贴对象
shortcuts_duplicate_action = 复制并粘贴
shortcuts_delete_action = 删除对象
shortcuts_section_terrain = 地形编辑
shortcuts_terrain_select = 点击地形
shortcuts_terrain_select_action = 显示曲线节点
shortcuts_terrain_drag = 拖拽节点
shortcuts_terrain_drag_action = 移动曲线节点
shortcuts_terrain_dblclick = 双击线段
shortcuts_terrain_dblclick_action = 插入新节点
shortcuts_terrain_delete = 在节点上按 Delete / Backspace
shortcuts_terrain_delete_action = 删除节点（最少3个）
shortcuts_terrain_rclick = 右键点击节点
shortcuts_terrain_rclick_action = 打开节点菜单

# 关于窗口
about_built_with = 基于 eframe / egui / wgpu 构建
about_license = 许可证：GNU AGPL v3.0
about_version_prefix = 版本：

# 添加对象对话框
add_type = 类型:
add_name = 名称:
add_prefab_index = 预制体索引:
add_kind_prefab = 预制体
add_kind_parent = 父对象
add_data_type = 数据类型:
add_data_type_none = None
add_data_type_terrain = Terrain
add_data_type_prefab_overrides = PrefabOverrides
add_data_type_terrain_help = Terrain 会创建默认地形数据，并保留你选择的预制体索引。
add_data_type_prefab_overrides_help = PrefabOverrides 会创建一个空的覆盖数据，后续可在属性面板里编辑。

# 面板
panel_object_list = 对象列表
panel_properties = 属性
panel_select_hint = 选择一个对象查看属性
panel_drop_hint = 拖放 .bytes 文件到此窗口
panel_open_hint = 或使用 文件 > 打开关卡文件…

# 属性视图
prop_type_prefab = 类型: Prefab
prop_type_parent = 类型: Parent
prop_name = 名称:
prop_prefab_index = 预制体索引:
prop_position = 位置
prop_rotation = 旋转
prop_scale = 缩放
prop_data_type = 数据类型:
prop_terrain = 地形数据
prop_fill_tex_index = 填充纹理索引:
prop_fill_vert_count = 填充顶点数:
prop_curve_vert_count = 曲线顶点数:
prop_curve_tex_count = 曲线纹理数:
prop_collider = 碰撞器:
prop_fill_color = 填充颜色
prop_override = 重写数据
prop_byte_count = 字节数:
prop_child_count = 子对象数:
prop_fill_offset_x = 填充偏移 X:
prop_fill_offset_y = 填充偏移 Y:
prop_curve_tex = 曲线纹理 { $idx }
prop_strip_width = 条带宽度:
prop_fade_threshold = 淡入阈值:
prop_terrain_closed = 闭合曲线:
add_terrain = 附加地形数据

# 状态消息
status_welcome = 打开一个 .bytes 关卡文件开始编辑
status_loaded = 已加载: { $obj_count } 个对象, { $root_count } 个根节点
status_exported = 导出成功
status_added = 已添加: { $name }
status_delete_confirm = 确认删除 "{ $name }"？
status_read_error = 读取失败: { $name }
status_export_error = 导出失败: { $name }

# 重写树
override_name_hint = 名称

# CLI
cli_read_error = 读取 { $path } 失败: { $error }
cli_parse_error = 解析 { $path } 失败: { $error }
cli_unsupported_input = 不支持的输入格式: .{ $name }
cli_serialize_yaml_error = 序列化 YAML 失败: { $name }
cli_serialize_toml_error = 序列化 TOML 失败: { $name }
cli_unsupported_output = 不支持的输出格式: .{ $name }
cli_write_error = 写入 { $path } 失败: { $error }
cli_convert_ok = { $input } -> { $output } ({ $obj_count } 个对象, { $root_count } 个根节点)
cli_error_prefix = 错误: { $name }

# 标签页
menu_close_tab = 关闭标签页

# HUD 覆盖层
hud_zoom = 缩放
hud_theme = 主题
hud_unknown_theme = 未检测

# 工具模式
tool_select = 选择
tool_box_select = 框选
tool_draw_terrain = 绘制地形
tool_pan = 平移
tool_window_title = 工具

# 工具模式快捷键
shortcuts_section_tools = 工具模式
shortcuts_tool_select = V
shortcuts_tool_box_select = M
shortcuts_tool_draw_terrain = P
shortcuts_tool_pan = H

# 关卡边界编辑器
menu_edit_level_bounds = 编辑关卡边界…
win_level_bounds = 关卡边界
bounds_pos_x = 位置 X:
bounds_pos_y = 位置 Y:
bounds_size_w = 宽度:
bounds_size_h = 高度:

# 存档查看器
menu_open_save = 打开存档文件…
save_viewer_title = 存档查看器
save_viewer_type = 类型
save_viewer_size = 大小
save_viewer_filter = 筛选:
save_viewer_raw_xml = 原始 XML
save_viewer_structured = 结构化视图
save_viewer_no_data = 未加载数据
save_viewer_part_count = 零件数
save_viewer_completed = 已完成
save_col_key = 键
save_col_type = 类型
save_col_value = 值
save_col_part_type = 零件类型
save_col_custom_idx = 自定义索引
save_col_rot = 旋转
save_col_flipped = 翻转
save_col_progress = 进度
save_col_completed = 已完成
save_col_synced = 已同步
save_editor_modified = 已修改
save_editor_parse_xml = 解析 XML
save_editor_add_entry = + 添加条目
menu_export_save = 导出存档文件…
menu_import_xml = 导入 XML 存档…
menu_export_xml = 导出 XML 存档…
save_editor_regex_err = 正则无效
save_filter_hint = 筛选（支持正则）
save_edit_clear_all = 清空所有条目
save_edit_duplicate_all = 克隆所有条目
menu_select_all = 全选
save_edit_deselect_all = 取消全选
save_edit_delete_selected = 删除选中条目
save_edit_duplicate_selected = 克隆选中条目
save_viewer_reveal_xml = 定位到 XML
context_toggle_node_texture = 切换节点纹理
contraption_preview_title = 装置预览
