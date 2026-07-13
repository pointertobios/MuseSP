# Python → Rust 移植进度

## 工作流
1. 确定要移植的模块
2. 逐行对比 Python ↔ Rust 行为
3. 修复差异
4. 标记完成

## 基础组件

### core.rs ← core.py ✅
- [x] ComponentBase 字段完全对照
- [x] Constraintable / Direction 枚举
- [x] layout / layout_axis 算法（分类、分配、max 迭代、居中偏移）
- [x] dispatch_event 事件分发（self + children 同 Python Component.dispatch_event）
- [x] handle_mouse_move / handle_mouse_input / handle_keyboard
- [x] emit 返回值语义（全执行，任一 false 即停）
- [x] force_mouse_exit
- [x] draw / draw_self / draw_debug（2px 边框线 + config 控制）

### button.rs ← button.py ✅
- [x] 状态颜色：normal(80,80,80) hovered(140,140,140) pressed(100,100,100) disabled(+半透明黑)
- [x] enable/disable + 事件（_handle_event → dispatch_event override）

### label.rs ← label.py ✅
- [x] draw_self 调用 renderer.draw_text

### spacer.rs ← spacer.py ✅
- [x] dispatch_event（跳过自身事件，仅分发给子组件）
- [x] debug_border_color = (139, 0, 0, 255)

### image_button.rs ← image_button.py ✅
- [x] 横向/竖向布局分支
- [x] draw_self 状态颜色同 Button
- [x] enable/disable

### scroll_list.rs ← scroll_list.py ✅
- [x] set_items（设 item_height + propagate_width + update_positions）
- [x] draw_self（背景 + 滚动条）
- [x] draw（裁剪可见子项）
- [x] dispatch_event（滚轮滚动 + MOUSEBUTTONDOWN 点击选中 + 可见性过滤）

## 框架层

### application.rs ← application.py ✅
- [x] RunMode 行为逐模式对照
- [x] 事件循环逻辑对照
- [x] 渲染管线

### router.rs ← router.py ✅
- [x] Page/PageToken 结构
- [x] AnyPage trait（含 initial_mode）
- [x] init_page 顺序（router→build→set size→prepare_layout→layout→mode）
- [x] push/pop/clear_and_push/pop_then_else 逻辑
- [x] draw_pages（黑底→shadow_promise→逐页 draw+draw_debug）

## 页面

### home.rs ← home.py ✅
- [x] build 组件树 + 约束 + 事件绑定（逐行对照）
- [x] prepare_layout（spacer max_width = root.width * 2/7）
- [x] 事件：开始→push MusicListPage, 退出→should_exit

### music_list.rs ← music_list.py ✅
- [x] build + prepare_layout 对照
- [x] _load_list + 事件

## 玩法

### gameplay_page.rs ← gameplay_page.py ✅
- [x] build（RendererCanvas MAX + menu button NONE）
- [x] build_test_geometry（8顶点 + 12三角形 匹配）
- [x] dispatch_event（Esc→push MenuPage）

### menu_page.rs ← menu_page.py ✅
- [x] build（top MAX + row + bot MAX）
- [x] prepare_layout（居中计算 + root 尺寸/位置 匹配）
- [x] draw（半透明白色背景 匹配）
- [x] dispatch_event（Esc→pop, 菜单外点击→跳过）
- [x] _on_exit: clear_and_push(MusicListPage::new()) 完全对齐 Python

## 配置

### config.rs ← config.py ✅
- [x] 结构 + load_config + TOML 解析

## 编辑器

### main.rs ← main.py ✅
- [x] EditorPage build 对照
