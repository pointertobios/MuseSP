use std::cell::RefCell;

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::{RenderSnapshot, UIRenderer, VertexLayoutDesc};

/// 自定义着色器画布组件。
///
/// 使用者传入自己的 WGSL 着色器和顶点布局，通过 `mpsc::Sender<RenderSnapshot>`
/// 推送渲染快照。每帧渲染时，使用最新收到的快照进行绘制。
///
/// # 约定
///
/// 着色器必须遵循以下 bind group 布局：
/// ```wgsl
/// @group(0) @binding(0) var t_diffuse: texture_2d<f32>;
/// @group(0) @binding(1) var s_diffuse: sampler;
/// @group(0) @binding(2) var<uniform> u_params: YourUniformType;
/// ```
///
/// 顶点着色器入口为 `vs_main`，片元着色器入口为 `fs_main`。
///
/// # 示例
///
/// ```ignore
/// let (canvas, sender) = RendererCanvas::new(
///     include_str!("my_shader.wgsl"),
///     my_vertex_layout,
///     0, 0, 800, 600,
/// );
///
/// // 在另一线程/位置推送渲染数据
/// sender.send(RenderSnapshot {
///     vertex_data: bytemuck::cast_slice(&my_vertices).to_vec(),
///     index_data: my_indices.to_vec(),
///     vertex_count: my_vertices.len() as u32,
///     index_count: my_indices.len() as u32,
///     texture: None,
///     uniform_data: bytemuck::cast_slice(&[my_uniform]).to_vec(),
/// }).unwrap();
/// ```
pub struct RendererCanvas {
    pub base: ComponentBase,
    shader_wgsl: String,
    vertex_layout: VertexLayoutDesc,
    rx: RefCell<mpsc::Receiver<RenderSnapshot>>,
    latest: RefCell<RenderSnapshot>,
}

impl RendererCanvas {
    /// 创建 RendererCanvas 并返回 `(组件, 发送端)`。
    ///
    /// - `shader_wgsl`: WGSL 着色器源码
    /// - `vertex_layout`: 顶点缓冲区布局描述
    /// - `x, y, width, height`: 组件位置与尺寸
    ///
    /// 返回的 `Sender<RenderSnapshot>` 用于向画布推送渲染数据。
    /// 画布每帧使用最新收到的快照；若未收到任何快照，则不绘制。
    pub fn new(
        shader_wgsl: &str,
        vertex_layout: VertexLayoutDesc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> (Box<Self>, mpsc::Sender<RenderSnapshot>) {
        let (tx, rx) = mpsc::channel(32);
        let canvas = Box::new(RendererCanvas {
            base: ComponentBase::new(x, y, width, height),
            shader_wgsl: shader_wgsl.to_string(),
            vertex_layout,
            rx: RefCell::new(rx),
            latest: RefCell::new(RenderSnapshot::empty()),
        });
        (canvas, tx)
    }

    /// 获取着色器源码引用
    pub fn shader_wgsl(&self) -> &str {
        &self.shader_wgsl
    }

    /// 获取顶点布局描述引用
    pub fn vertex_layout(&self) -> &VertexLayoutDesc {
        &self.vertex_layout
    }

    /// 获取当前最新快照的引用
    pub fn latest_snapshot(&self) -> std::cell::Ref<'_, RenderSnapshot> {
        self.latest.borrow()
    }
}

impl ComponentTrait for RendererCanvas {
    fn base(&self) -> &ComponentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        // 消费所有待处理的快照，只保留最新一个
        let mut rx = self.rx.borrow_mut();
        let mut latest = self.latest.borrow_mut();
        loop {
            match rx.try_recv() {
                Ok(snap) => *latest = snap,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        drop(rx);

        // 如果有顶点数据，推送自定义绘制命令
        if !latest.vertex_data.is_empty() && latest.vertex_count > 0 {
            renderer.draw_renderer_canvas(
                dx,
                dy,
                self.base.width,
                self.base.height,
                &self.shader_wgsl,
                &self.vertex_layout,
                &latest,
            );
        }
    }
}
