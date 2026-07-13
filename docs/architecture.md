# MuseSP 项目架构文档

## 项目概览

MuseSP 是一个音游项目，Python 版基于 pygame-ce，Rust 版基于 winit + wgpu。
Python: `uv` 管理依赖，`setuptools` 构建。Rust: `cargo` 管理，workspace 多 crate。

## 目录结构

```
MuseSP/
├── pyproject.toml / Cargo.toml   # 项目配置
├── config.example.toml           # 配置文件（Python/Rust 共用）
├── docs/
│   ├── architecture.md           # 本文档
│   └── porting-progress.md       # Rust 移植进度清单
├── assets/                       # 静态资源（UI 图标、内置曲目）
├── musesp/                       # 主应用（Python 包 / Rust crate）
│   ├── main.py / src/main.rs     # 入口
│   ├── pages/                    # 页面实现（HomePage, MusicListPage）
│   └── src/gameplay/             # 玩法模块（GameplayPage, MenuPage）
├── musesp_ui/                    # UI 框架
│   ├── application.py / src/application.rs  # Application + wgpu 管线
│   ├── router.py / src/router.rs            # Page + Router
│   ├── components/                           # 组件
│   ├── font.py                              # Python 字体（Sarasa UI SC）
│   └── src/*.wgsl                           # Rust WGSL shader
├── musesp_editor/                # 编辑器应用（轻量 stub）
├── musesp_config/                # 配置加载（Python/Rust 共用 TOML）
└── musesp_gameplay/              # Python 玩法（Rust 已合并到 musesp crate）
```

---

## Rust 移植工作流

### 基本原则

1. **Python 版行为是唯一标准**，Rust 版必须完全对齐，禁止为修 bug 改变行为
2. **先对照 Python 实现，再写 Rust 代码**，避免凭空猜测
3. **逐行对比**每个文件的构造、绘制、事件、布局
4. **禁止新增** Python 版不存在的函数、类型、trait 方法或行为；仅当 Rust 语言确实无法直接表达 Python 写法时才可例外，并须在本文档注明原因

### 移植流程

```
1. 确定要移植的模块（从 porting-progress.md 选取）
2. 读 Python 源码，理解完整行为
3. 读 Rust 源码，逐行对比差异
4. 修复差异，确保行为一致
5. 编译验证（cargo build）
6. 在 porting-progress.md 标记完成
```

### 模块对照清单

| Python 文件 | Rust 文件 | 状态 |
|---|---|---|
| `musesp_ui/components/core.py` | `core.rs` | ✅ |
| `musesp_ui/components/button.py` | `button.rs` | ✅ |
| `musesp_ui/components/label.py` | `label.rs` | ✅ |
| `musesp_ui/components/spacer.py` | `spacer.rs` | ✅ |
| `musesp_ui/components/image_button.py` | `image_button.rs` | ✅ |
| `musesp_ui/components/image.py` | `image.rs` | ✅ |
| `musesp_ui/components/scroll_list.py` | `scroll_list.rs` | ✅ |
| `musesp_ui/components/renderer_canvas.py` | `renderer_canvas.rs` | ✅ |
| `musesp_ui/application.py` | `application.rs` | ✅ |
| `musesp_ui/router.py` | `router.rs` | ✅ |
| `musesp_config/config.py` | `config.rs` | ✅ |
| `musesp/pages/home.py` | `home.rs` | ✅ |
| `musesp/pages/music_list.py` | `music_list.rs` | ✅ |
| `musesp_gameplay/gameplay_page.py` | `gameplay_page.rs` | ✅ |
| `musesp_gameplay/menu_page.py` | `menu_page.rs` | ✅ |
| `musesp/main.py` | `main.rs` | ✅ |
| `musesp_editor/main.py` | `main.rs` | ✅ |

### Shader 规范

- Shader 文件放在 `musesp_ui/src/` 下（`.wgsl` 格式）
- 通过 `include_str!("xxx.wgsl")` 嵌入，禁止内联字符串常量
- 现有 shader: `rect.wgsl`（2D 四边形）、`game.wgsl`（3D 立方体）

---

## 移植中遇到的问题与解决方案

### 1. Spacer min_height/min_width 缺失

**问题**: `Spacer::new(0, 30)` 设置的是实际 `height`，不是 `min_height`。布局将 `min_height=0` 归为 `minimum_zero`，多个 spacer 平分剩余空间导致撑满全屏。

**修复**: 页面代码中对 `spacer.min_height/min_width` 的赋值必须逐行对照 Python，不能遗漏。

**受影响文件**: `home.rs`, `music_list.rs`, `menu_page.rs`, `musesp_editor/main.rs`

### 2. layout 方向被错误覆盖

**问题**: `init_page` / resize handler 中 `layout(Some(Direction::Vertical))` 强制覆盖了 root 在 `build()` 中设置的 `Horizontal` 方向，导致横向布局变竖向，spacer 不可见。

**修复**: 全部改为 `layout(None)`，让 root 使用自己的 `layout_direction`。

### 3. prepare_layout 调用目标错误

**问题**: `page.page_mut().prepare_layout()` 调用的是 `Page` 的空默认实现，而非具体页面在 `AnyPage` trait 中的 override。导致 HomePage 的 spacer max_width 从未设置。

**修复**: 改为 `page.prepare_layout()` 直接调用 trait 方法。

### 4. TOML 解析失败

**问题**: `content.parse::<toml::Value>()` 在 `toml` crate v1.x 中无法解析多 section TOML，静默失败返回 `Config::default()`（`component_border = false`）。

**修复**: 改为 `toml::from_str::<toml::Table>(&content)`。

### 5. Router 初始化顺序

**问题**: `init_page` 中 `build()` 在 `router` 设置之前调用。Python 的顺序是先设 `_router` 再 `build()`。导致 `build()` 中所有事件闭包捕获的 `Weak` 永远是 `None`，按钮全部不响应。

**修复**: `init_page` 中 `router` 设置在 `build()` 之前，对齐 Python 顺序。

### 6. RefCell 重入 panic

**问题**: `build()` / `on_activate()` 中调 `set_mode()` → `router.borrow_mut()` 时，Router 已被外层（`push`/`pop`/`init_router`）可变借用，触发 `RefCell already borrowed` panic。

**修复**: 
- `Page::set_mode()` 改用 `try_borrow_mut()`，借用失败时静默跳过
- `init_page()` 在 `build()` 之后显式调用 `self.mode = page.initial_mode()`
- `pop()` 在 `on_activate()` 之后显式调用 `self.mode = current.initial_mode()`
- `AnyPage` trait 新增 `initial_mode()` 方法（默认 `Event`，GameplayPage 覆写为 `Vsync`）

### 7. wgpu depth-stencil 不匹配

**问题**: render pass 有 `Depth32Float` depth attachment，但 2D rect pipeline 的 `depth_stencil: None`。set_pipeline 时报 validation error。

**修复**: 2D pipeline 添加 `DepthStencilState { format: Depth32Float, depth_write_enabled: false, depth_compare: Always }`。

### 8. Viewport 未设置分辨率

**问题**: `glyphon::Viewport::new()` 创建后未调用 `update()` 设置分辨率，导致文字着色器投影错误，文字不显示。

**修复**: 创建后立即调用 `viewport.update(queue, Resolution { width, height })`。

### 9. 文字未居中

**问题**: glyphon 的 `Buffer::set_text` 未使用 `Align::Center`，且 `TextArea` 的 `top` 未加垂直居中偏移。

**修复**: `set_text` 使用 `Some(Align::Center)`，`TextArea.top = t.y + (h - line_height) / 2`。

### 10. Debug 边框绘制为填充矩形

**问题**: `draw_debug` 使用 `draw_rect` 画填充矩形，而 Python 用 `pygame.draw.rect(..., 2)` 画 2px 边框线。

**修复**: 改为 4 条 2px 细边框线（上/下/左/右）。

### 11. crate 依赖循环

**问题**: `musesp` 依赖 `musesp_gameplay`（需 GameplayPage），`musesp_gameplay` 需引用 `musesp` 中的 `MusicListPage` 形成循环。

**修复**: 将 `musesp_gameplay` 合并到 `musesp` crate，消除跨 crate 依赖。同一 crate 内模块相互引用无循环限制。

### 12. glyphon TextRenderer depth-stencil 不匹配

**问题**: `glyphon::TextRenderer::new()` 第四个参数 `depth_stencil` 传 `None`，glyphon 内部管线不带 depth-stencil，但 render pass 使用 `Depth32Float`，`set_pipeline` 时报格式不兼容。

**修复**: 传入 `Some(DepthStencilState { format: Depth32Float, depth_write_enabled: false, depth_compare: Always, ... })`。

### 13. ScrollList 事件分发缺失

**问题**: Rust 版 ScrollList 没有自定义 `dispatch_event`，无法处理鼠标滚轮滚动和列表项点击选中。

**修复**: 
- `ComponentTrait` trait 新增 `dispatch_event` 方法（默认委托给 `ComponentBase::dispatch_event`）
- `ScrollList` 覆写 `dispatch_event`：处理 `MouseWheel`（滚动）、`MouseInput`（点击选中）、并对子组件做可见性过滤
- `ComponentBase` 的 `handle_mouse_move`/`handle_mouse_input`/`handle_keyboard`/`local_pos` 改为 `pub(crate)`
- `ComponentBase` 新增 `item_id: Option<String>` 字段用于列表项标识

---

## 核心架构

### Rust 应用层 (`musesp_ui/src/application.rs`)

```
Application::run(name, page):
  1. 创建 Application 实例（含 Router + Config + FontSystem）
  2. EventLoop::new() + run_app()

ApplicationHandler::resumed:
  1. 创建 winit Window
  2. 初始化 wgpu（surface, device, queue, 2D pipeline, 3D pipeline, depth buffer）
  3. 初始化 glyphon（FontSystem, SwashCache, Cache, TextAtlas, TextRenderer）
  4. init_router → push 初始页面

ApplicationHandler::window_event:
  CloseRequested → exit
  Resized → resize wgpu + 更新 root 尺寸 + layout
  RedrawRequested → render（2D rects → 3D → images → glyphon text）
  CursorMoved/MouseInput/KeyboardInput → dispatch_event + exit 检查

RunMode:
  Event  → ControlFlow::Wait（对齐 Python pygame.event.wait()）
  Fps    → ControlFlow::Poll + 帧率计时
  Vsync  → ControlFlow::Poll + 每帧 request_redraw
```

### Rust 路由系统 (`musesp_ui/src/router.rs`)

```
Page { root: ComponentBase, router: Option<Weak<RefCell<Router>>> }
AnyPage trait { page(), page_mut(), build(), destroy(), on_hide(), on_activate(),
                full_shadow_promise(), prepare_layout(), draw(), draw_debug(),
                dispatch_event(), initial_mode() }

Router { stack: Vec<(Box<dyn AnyPage>, PageToken)>, win_w, win_h, mode, should_exit }

init_page: router 设置 → build → root 尺寸 → prepare_layout → layout → mode
push: on_hide + force_mouse_exit → init_page → push 入栈
pop: 弹栈 → resolve token → destroy → on_activate + mode 更新
clear_and_push: 全部 destroy → init_page → push
draw_pages: 黑底 → 从 full_shadow_promise 页开始 → 逐页 draw + draw_debug
```

### Rust 组件系统 (`musesp_ui/src/components/core.rs`)

```
ComponentTrait: Any {
    base(), base_mut(), draw_self(), draw() [default impl],
    as_any(), as_any_mut() [Sized required]
}

ComponentBase {
    x, y, width, height, min/max_width/height,
    h_constraint, v_constraint, layout_direction,
    centered_horizontal, centered_vertical,
    hovered, pressed, cursor_x, cursor_y,
    children: Vec<Box<dyn ComponentTrait>>,
    handlers: HashMap<String, Vec<EventHandler>>,
    event_override: Option<Box<dyn FnMut(&WindowEvent) -> Option<bool>>>,
    debug_border_color
}

布局: layout_axis(horizontal) → 分类(MAXIMUM/min_nonzero/min_zero)
      → 迭代分配(MAXIMUM capped) → 设置 pos/size/cross → 居中偏移
事件: dispatch_event → event_override 检查 → CursorMoved/MouseInput/KeyboardInput
      → 坐标转换 → emit handler → 递归子组件
绘制: draw → draw_self + 递归 draw → draw_debug(2px边框线 + config控制)
```

### Python 应用层 (`musesp_ui/application.py`)

```
Application.__init__:
  1. pygame.init()
  2. Router(starts_with) → 构建初始页面
  3. 创建窗口 → 设置根组件尺寸
  4. 调用 root.layout()

Application.run():
  for event in pygame.event.get():
    if QUIT: break
    router.dispatch_event(event)   # 事件 → 栈顶页面 → 组件树
  router.draw_pages(screen)         # 从遮挡边界向上绘制
  pygame.display.flip()
```

### Python 路由系统 (`musesp_ui/router.py`)

```python
class Page:
    _root: Component          # 根组件（容器）
    build()                   # push 时调用，创建组件
    destroy()                 # pop 时调用，清理资源
    on_hide()                 # 子页面 push 覆盖本页时
    on_activate()             # 子页面 pop 后本页恢复可见时
    full_shadow_promise() → bool   # True: 完全遮挡下层，跳过下层绘制
    prepare_layout()          # root 尺寸确定后、layout 之前调用
    add_component(c)          # 添加到根组件
    dispatch_event(event)     # 分发给根组件
    draw(surface)             # 绘制根组件树

class Router:
    push(page) → PageToken    # 同步入栈，当前页 on_hide，新页 build
    pop(value)                # 出栈，resolve token，destroy 页面，新栈顶 on_activate
    clear_and_push(page)      # 清栈并压入新页
    pop_then_else(fallback)   # 弹栈，若将空则调用 fallback 生成新页
    draw_pages(surface)       # 从栈顶向下找首个 full_shadow_promise=True 的页面，向上绘制
```

### Python 约束布局系统

| 约束值 | 行为 |
|---|---|
| `NONE` | 不受布局影响，保持原有坐标和尺寸 |
| `MINIMUM` | 获得 `min_size`；`min>0` 受保护，`min==0` 可被 MAXIMUM 挤占至 0 |
| `MAXIMUM` | 等分剩余空间，可设 `max_size` 限制上限（0=无限制） |

| 方向 | 主轴 | 交叉轴 |
|---|---|---|
| `VERTICAL` | `v_constraint` → y/height | width = parent.width |
| `HORIZONTAL` | `h_constraint` → x/width | height = parent.height |

居中：`centered_vertical` 仅 VERTICAL 生效，`centered_horizontal` 仅 HORIZONTAL 生效。

### Python 事件系统

```
Router.dispatch_event(event)
  → Page.dispatch_event(event)
    → Component.dispatch_event(event)
      → _shift_event(pos - self.x, -self.y)  # 局部坐标
      → _handle_event(local)
      → 递归 sub_components

Handler 返回 bool: True=继续传播, False=停止
同一事件可绑定多个 handler，全部执行，任一返回 False 即停止
```

---

## 项目规范

- Python: 不用 `__init__.py`，flat module；Rust: 标准 crate 结构
- `pyproject.toml` 的 `[tool.setuptools.packages.find]` 声明所有子包
- Rust workspace: `musesp_config`, `musesp_ui`, `musesp`, `musesp_editor`
- Shader 文件独立在 `musesp_ui/src/*.wgsl`，通过 `include_str!` 引入
- 配置文件: `config.example.toml`，Python/Rust 共用
- **禁止手动编辑 `Cargo.toml`**，必须用 `cargo add` / `cargo remove` 管理依赖
- **禁止使用 `mod.rs` 模块结构**，使用 Rust 2018+ 新规范：`musesp/src/gameplay_page.rs` 直接声明为顶级模块（`mod gameplay_page;`），不再用 `gameplay/mod.rs` 嵌套
- 使用 `uv`，结构变更后执行 `uv sync`

## 当前页面

### HomePage (`musesp/pages/home.py`)

```
full_shadow_promise: True
layout_direction: VERTICAL

┌─────────────────────────────┐
│  MuseSP (Label)             │  v=MINIMUM, h=MINIMUM, 120px
├─────────────────────────────┤
│  [开始] (Button)            │  v=MINIMUM, h=MAXIMUM, 50px
├─────────────────────────────┤
│  [设置] (Button)            │  v=MINIMUM, h=MAXIMUM, 50px
├─────────────────────────────┤
│  [退出] (Button)            │  v=MINIMUM, h=MAXIMUM, 50px
└─────────────────────────────┘
```
