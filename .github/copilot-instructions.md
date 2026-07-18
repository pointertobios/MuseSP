# Project Guidelines

## 项目结构

本项目已从 Python 迁移至 Rust，采用 Cargo workspace 管理：

```
Cargo.toml          # workspace 根，声明所有成员 crate
musesp/             # 主程序入口 + 页面实现
musesp_ui/          # UI 框架（组件、路由、渲染器）
musesp_config/      # 配置模块
musesp_editor/      # 编辑器模块
```

## 构建与运行

- 使用 `cargo` 构建：`cargo build` / `cargo run`
- **绝对禁止直接编辑任何 `Cargo.toml`**。添加依赖使用 `cargo add <crate> --package <target>`，删除依赖使用 `cargo remove <crate> --package <target>`
- workspace resolver 为 `3`（Rust 2024 edition）

## 渲染架构

- `musesp_ui` 只提供最小渲染抽象（`RenderPipeline` trait），不感知 compute/graphics shader 细节
- 业务层（`musesp`）通过实现 `RenderPipeline` trait 来自定义渲染管线
- `WgpuRenderer` 只负责 UI 元素（rect、image、custom canvas）的渲染
- 所有 compute shader、自适应细分、3D 渲染等实现都在 `musesp::gameplay::renderer3d` 中

## Rust 代码规范

- 模块目录名使用下划线（`musesp_ui`、`musesp_gameplay`），不得使用连字符
- 每个子 crate 有独立的 `Cargo.toml` 和 `src/` 目录
- 页面实现 `AnyPage` trait（定义在 `musesp_ui::router`）
- 组件实现 `ComponentTrait` trait（定义在 `musesp_ui::components::core`）
- 纯容器节点直接使用 `ComponentBase`（已实现 `ComponentTrait`），无需包装类

## 关键 trait 和结构

| 概念 | Python（旧） | Rust（新） |
|------|-------------|-----------|
| 页面基类 | `Page` 类 | `Page` 结构体 + `AnyPage` trait |
| 组件基类 | `Component` 类 | `ComponentBase` 结构体 + `ComponentTrait` trait |
| 布局 | `Component.layout()` 虚方法 | `ComponentTrait::layout()` trait 默认方法，可覆写 |
| 路由 | `Router` 类 | `Router` 结构体（`musesp_ui::router`） |
| 渲染 | `pygame.Surface` | `UIRenderer` 命令缓冲 + wgpu 管线 |
