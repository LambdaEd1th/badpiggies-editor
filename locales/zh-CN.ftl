# 菜单栏
menu_file = 文件
menu_open_level = 打开关卡文件…
menu_export_level = 导出关卡
menu_import_text = 导入 YAML/TOML…
menu_export_yaml = 导出为 YAML
menu_export_toml = 导出为 TOML
menu_edit = 编辑
menu_undo = 撤销
menu_redo = 恢复
menu_add_object = 添加对象…
menu_view = 视图
menu_fit_view = 适应视图
menu_hide_bg = 隐藏背景 (B)
menu_show_bg = 显示背景 (B)
menu_object_list = 对象列表
menu_properties = 属性面板
menu_grid = 网格
menu_physics_ground = 物理地面
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
btn_delete = 删除
btn_confirm = 确认
btn_add = + 添加
btn_visual = 可视化
btn_text = 文本

# 快捷键窗口
shortcuts_key = 快捷键
shortcuts_action = 功能
shortcuts_scroll = 滚轮
shortcuts_zoom = 缩放视图
shortcuts_drag = 拖拽（空白处）
shortcuts_pan = 平移视图
shortcuts_click = 点击对象
shortcuts_select = 选中对象
shortcuts_b_key = B
shortcuts_toggle_bg = 切换背景显示
shortcuts_undo = ⌘Z / Ctrl+Z
shortcuts_undo_action = 撤销
shortcuts_redo = Shift+⌘Z / Ctrl+Shift+Z
shortcuts_redo_action = 恢复

# 关于窗口
about_built_with = 基于 eframe / egui / wgpu 构建
about_license = 许可证：GNU AGPL v3.0
about_version_prefix = 版本：

# 添加对象对话框
add_type = 类型:
add_name = 名称:
add_prefab_index = 预制体索引:

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

# HUD 覆盖层
hud_zoom = 缩放
hud_theme = 主题
hud_unknown_theme = 未检测
