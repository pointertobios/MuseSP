use wgpu;

/// 业务层（gameplay）自定义渲染管线的最小抽象。
///
/// `musesp_ui` 不关心 compute shader、graph shader 或具体 pass 的细节。
/// 业务层实现此 trait，在合适的帧时机被调用即可。
///
/// # 调用时序
///
/// ```text
/// 1. record_compute()  — 在主 render pass 之前，可录制任意 compute passes
/// 2. [UI 元素绘制]     — musesp_ui 绘制 rect/image/text/custom 等
/// 3. record_render()   — 在 UI 绘制之后，可录制自定义 draw calls（深度写入已开启）
/// ```
pub trait RenderPipeline: Send {
    /// 录制 compute passes。在相机更新后、主 render pass 开始前调用。
    fn record_compute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        config: &wgpu::SurfaceConfiguration,
        sample_count: u32,
    );

    /// 在 render pass 中录制自定义绘制命令。在 UI 元素绘制之后调用。
    fn record_render<'rp>(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rp: &mut wgpu::RenderPass<'rp>,
        config: &wgpu::SurfaceConfiguration,
        sample_count: u32,
    );
}
