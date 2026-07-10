# MuseSP 项目架构文档

## 项目概览

MuseSP 是一个基于 pygame-ce 的音游项目，采用多子包架构，使用 `uv` 管理依赖，`setuptools` 作为构建后端。

## 目录结构

```
MuseSP/
├── pyproject.toml              # 项目配置（构建系统、依赖、entry points、包发现）
├── .github/
│   ├── copilot-instructions.md  # 项目级 AI 编程指令
│   └── skills/
│       └── create-page/
│           └── SKILL.md         # 编写 Page 的 skill
├── musesp/                      # 主应用包
│   ├── main.py                  # 入口：Application("MuseSP", starts_with=HomePage())
│   └── pages/
│       └── home.py              # HomePage 实现
├── musesp_ui/                   # UI 框架包
│   ├── application.py           # Application 类（pygame 窗口 + 主循环）
│   ├── router.py                # Page 基类 + Router + PageToken
│   ├── components.py            # Component 基类 + Label + Button + 约束布局
│   └── font.py                  # 全局字体接口（Sarasa UI SC）
├── musesp_editor/               # 编辑器应用
│   └── main.py                  # 入口：Application("MuseSP Editor")
├── musesp_asset/                # 资产管理（预留）
└── musesp_gameplay/             # 游戏玩法（预留）
```

## 构建与运行

```bash
uv sync                # 安装依赖 + editable 安装所有 musesp* 包
uv run musesp          # 启动主应用
uv run musesp-editor   # 启动编辑器
```

`pyproject.toml` 中 `[tool.setuptools.packages.find]` 使用 `include = ["musesp*"]` 自动发现所有子包。

## 核心架构

### 应用层 (`musesp_ui/application.py`)

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

### 路由系统 (`musesp_ui/router.py`)

#### Page 基类

```python
class Page:
    _root: Component          # 根组件（容器）

    # 生命周期
    build()                   # push 时调用，创建组件
    destroy()                 # pop 时调用，清理资源
    on_hide()                 # 子页面 push 覆盖本页时
    on_activate()             # 子页面 pop 后本页恢复可见时

    # 绘制控制
    full_shadow_promise() → bool   # True: 完全遮挡下层，跳过下层绘制
    hide_last()                    # 定制覆盖上一页时的视觉效果

    # 组件管理
    add_component(c)          # 添加到根组件
    dispatch_event(event)     # 分发给根组件
    draw(surface)             # 绘制根组件树
```

#### Router

```
栈结构: [(Page, PageToken), ...]

push(page) → PageToken       # 同步入栈，当前页 on_hide，新页 build
pop(value)                   # 出栈，resolve token，destroy 页面，新栈顶 on_activate
dispatch_event(event)        # 仅发给栈顶页面
draw_pages(surface)          # 从栈顶向下找首个 full_shadow_promise=True 的页面，向上绘制
```

#### PageToken

```python
token = router.push(some_page)   # 同步返回 token
value = token.get()              # 子页 pop(value) 后获取返回值
```

### 组件系统 (`musesp_ui/components.py`)

#### 坐标与嵌套

- 所有组件的 `x/y` 是**相对父组件的偏移**
- 每个组件维护 `sub_components` 列表，支持无限嵌套
- `draw()` 和 `dispatch_event()` 递归处理坐标转换

#### Component 基类

```python
class Component:
    x, y, width, height           # 相对父组件的 bounds
    min_width, min_height          # 约束布局的最小尺寸
    h_constraint, v_constraint     # Constraintable: NONE / MINIMUM / MAXIMUM
    layout_direction               # Direction: VERTICAL / HORIZONTAL
    _sub_components                # 子组件列表
    _hovered, _pressed             # 鼠标状态跟踪

    # 方法
    add_sub_component(c)           # 添加子组件
    layout()                       # 按约束排列子组件
    dispatch_event(event)          # 递归分发 pygame 事件
    draw(surface)                  # 递归绘制
    bind_on_mouse_*(handler)       # 鼠标事件绑定
    bind_on_key_*(handler)         # 键盘事件绑定
```

#### 事件传播

```
Router.dispatch_event(event)
  → Page.dispatch_event(event)
    → root.dispatch_event(event)
      → 平移 event.pos 为局部坐标
      → _handle_event(local)       # 检查 _in_rect(0..w, 0..h)
      → 递归传递给 sub_components
```

#### 事件绑定

| 方法                       | 触发时机              |
| -------------------------- | --------------------- |
| `bind_on_mouse_enter/exit` | 鼠标进入/离开组件区域 |
| `bind_on_mouse_down/up`    | 鼠标按下/释放         |
| `bind_on_mouse_click`      | down+up 都在区域内    |
| `bind_on_key_down/up`      | 键盘按下/释放         |

#### Label

```python
Label(text, x, y, width, height, font_size, color)
# 仅渲染文字，无背景填充
```

#### Button

```python
Button(text, x, y, width, height, font_size)
# 内部组合 Label（NONE 约束，居中 + 4px padding）
# 三种视觉状态：默认(80,80,80) / 悬停(140,140,140) / 按下(100,100,100)
```

### 约束布局系统

#### Constraintable 枚举

| 值        | 行为                                                                |
| --------- | ------------------------------------------------------------------- |
| `NONE`    | 不受布局影响，保持原有坐标和尺寸                                    |
| `MINIMUM` | 获得 `min_size`，可被 `MAXIMUM` 挤占                                |
| `MAXIMUM` | 等分剩余空间（`parent_size − Σ min`），挤占其他 `MAXIMUM`/`MINIMUM` |

#### Direction 枚举

| 值           | 主轴                      | 交叉轴                 |
| ------------ | ------------------------- | ---------------------- |
| `VERTICAL`   | `v_constraint` → y/height | width = parent.width   |
| `HORIZONTAL` | `h_constraint` → x/width  | height = parent.height |

#### 调用时机

`layout()` 必须在父组件尺寸确定后调用。当前在 `Application.__init__` 设置根组件尺寸后执行。

### 字体系统 (`musesp_ui/font.py`)

```python
get_font(size: int) → pygame.font.Font
# 内部调用 pygame.font.SysFont("Sarasa UI SC", size)
# 支持 .ttc 字体集
```

## 项目规范

记录在 `.github/copilot-instructions.md`：
- 不使用 `__init__.py`，flat module 方式
- 模块目录名用下划线（`musesp_ui`），不用连字符
- `pyproject.toml` 的 `[tool.setuptools.packages.find]` 声明所有子包
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
│  [退出] (Button)            │  v=MINIMUM, h=MAXIMUM, 填充剩余
└─────────────────────────────┘
```
