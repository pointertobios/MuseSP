use super::types::{DrawCompute, DrawImage, DrawRect, DrawRendererCanvas, DrawText};

/// 一帧的完整绘制数据，可跨线程发送。
///
/// 由后台任务准备，主线程消费后录制 GPU 命令。
/// 所有字段均为 owned，均实现 `Send`。
#[derive(Default)]
pub struct FrameDrawList {
    pub rects: Vec<DrawRect>,
    pub texts: Vec<DrawText>,
    pub images: Vec<DrawImage>,
    pub custom_draws: Vec<DrawRendererCanvas>,
    pub compute_draws: Vec<DrawCompute>,
    /// 后台预塑形的文本缓冲区（与 `texts` 一一对应）。
    /// 为 `None` 表示尚未塑形，渲染时需同步塑形。
    pub shaped_text_buffers: Option<Vec<glyphon::Buffer>>,
}

impl FrameDrawList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.rects.clear();
        self.texts.clear();
        self.images.clear();
        self.custom_draws.clear();
        self.compute_draws.clear();
        self.shaped_text_buffers = None;
    }

    /// 从 UIRenderer 提取绘制数据（消费 UIRenderer）。
    pub fn from_renderer(renderer: &mut super::UIRenderer) -> Self {
        FrameDrawList {
            rects: std::mem::take(&mut renderer.rects),
            texts: std::mem::take(&mut renderer.texts),
            images: std::mem::take(&mut renderer.images),
            custom_draws: std::mem::take(&mut renderer.custom_draws),
            compute_draws: std::mem::take(&mut renderer.compute_draws),
            shaped_text_buffers: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
            && self.texts.is_empty()
            && self.images.is_empty()
            && self.custom_draws.is_empty()
            && self.compute_draws.is_empty()
    }
}
