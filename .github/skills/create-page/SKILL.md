---
name: create-page
description: "为 MuseSP 项目编写 Page 子类。使用 ask-questions 工具逐一询问页面行为（遮挡、组件、导航），再生成代码。Use when: 创建新页面, 写 Page, 加页面, create page, new page, add page."
argument-hint: "描述这个页面要做什么"
user-invocable: true
---

# 编写 Page

基于 `musesp_ui::router::AnyPage` trait，引导用户编写一个新的页面。

## 流程

加载本 skill 后，必须使用 `vscode_askQuestions` 工具分**两轮**向用户提问。第一轮确定基本信息和遮挡/事件行为；第二轮确定组件和导航逻辑。

### 第一轮提问

| 问题                           | 参考选项                                                         |
| ------------------------------ | ---------------------------------------------------------------- |
| **页面名称**（结构体名）       | `HomePage`、`SettingsPage`、`DialogPage`、`MenuPage`…            |
| **所属模块**（放在哪个 crate 下） | `musesp/src/pages/`、`musesp_editor/src/`…                      |
| **`full_shadow_promise()`**    | `true`（完全遮挡下层，跳过绘制下层）或 `false`（默认，下层可见） |
| **是否需要自定义事件分发**     | 默认委托给 `Page` / 需要拦截特定按键（如 Escape）…               |

### 第二轮提问

| 问题                     | 参考选项                             |
| ------------------------ | ------------------------------------ |
| **需要的组件**           | 无 / `Label` / `Button` / 多个组合…  |
| **`build()` 中做什么**   | 创建组件、`push` 到 `root.children`、加载资源… |
| **`destroy()` 中做什么** | 无操作（默认）/ 释放资源…            |
| **是否 `push` 子页面**   | 无 / 点击按钮 push 某页面 / …        |
| **`pop` 返回值**         | `None` / 字符串 / 选中项…            |

## 生成物

根据用户回答，生成：

1. Rust 结构体 + `impl AnyPage`，放在指定模块路径下
2. 带完整 `build`/`destroy` 逻辑
3. 组件通过 `self.page.root.children.push(Box::new(component))` 注册
4. 如有子页面导航，通过 `NavAction::Push`/`NavAction::Pop` 发送到 `page.nav` channel

## 参考

`AnyPage` trait 定义见 `musesp_ui/src/router.rs`，可用组件见 `musesp_ui/src/components.rs`。

### Page 生命周期（调用顺序）

```
build() → [on_hide()] → [on_activate()] → … → destroy()
           ↑ push 时        ↑ 子页 pop 时          ↑ pop 本页时
```

### 组件绑定示例

```rust
let nav = self.page.nav.clone().unwrap();
btn.base.bind_mouse_click(Box::new(move |_| {
    let _ = nav.send(NavAction::Pop);
    false
}));
```

### 页面模板

```rust
use musesp_ui::components::core::{ComponentBase, ComponentTrait, Constraintable, Direction};
use musesp_ui::renderer::UIRenderer;
use musesp_ui::router::{AnyPage, NavAction, Page};
use musesp_config::config::Config;

pub struct MyPage {
    pub page: Page,
}

impl MyPage {
    pub fn new() -> Self {
        MyPage { page: Page::new() }
    }
}

impl AnyPage for MyPage {
    fn page(&self) -> &Page { &self.page }
    fn page_mut(&mut self) -> &mut Page { &mut self.page }

    fn full_shadow_promise(&self) -> bool { false }

    fn build(&mut self) {
        let nav = self.page.nav.clone().unwrap();
        // 创建组件树...
    }

    fn prepare_layout(&mut self) {
        self.page.root.layout(None);
    }

    fn draw(&self, renderer: &mut UIRenderer) {
        self.page.draw(renderer);
    }

    fn draw_debug(&self, renderer: &mut UIRenderer, config: &Config) {
        self.page.draw_debug(renderer, config);
    }

    fn dispatch_event(&mut self, event: &winit::event::WindowEvent) {
        self.page.dispatch_event(event);
    }
}
```
