use std::cell::RefCell;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use super::draw_list::FrameDrawList;
use super::types::DrawText;

/// 异步帧准备管线。
///
/// # 架构
///
/// ```text
/// 主线程:  [submit Frame N] → [request_prepare(texts)] → [submit Frame N+1]
/// 后台:                [文本塑形 Frame N+1]          → [文本塑形 Frame N+2]
/// ```
///
/// - `mpsc` channel 用于主线程 → 后台线程的帧数据传递
/// - `watch` channel 用于后台线程 → 主线程的结果传递（无锁读取）
///
/// 后台 `tokio::spawn` 任务拥有独立的 `glyphon::FontSystem`，
/// 将文本塑形（CPU 密集型）从主线程剥离。
pub struct FramePipeline {
    /// 接收已准备好的帧
    ready_rx: RefCell<watch::Receiver<Option<Arc<FrameDrawList>>>>,
    /// 发送待处理的帧数据
    prep_tx: mpsc::Sender<FramePrepData>,
    /// 后台任务句柄
    _task: tokio::task::JoinHandle<()>,
}

/// 帧准备数据：携带需要后台处理的绘制信息。
pub struct FramePrepData {
    pub screen_w: u32,
    pub screen_h: u32,
    /// 待塑形的文本绘制命令
    pub texts: Vec<DrawText>,
}

impl FramePipeline {
    /// 创建帧管线。
    ///
    /// `tokio::spawn` 异步任务，拥有独立的 `FontSystem` 进行文本塑形。
    /// `rt` 是当前进程的 tokio runtime handle，因为 winit 回调不在 runtime 上下文中。
    pub fn new(rt: &tokio::runtime::Handle) -> Self {
        let (prep_tx, mut prep_rx) = mpsc::channel::<FramePrepData>(4);
        let (ready_tx, ready_rx) = watch::channel(None);

        let task = rt.spawn(async move {
            // 独立的 FontSystem，避免与主线程竞争
            let mut font_system = glyphon::FontSystem::new();

            while let Some(data) = prep_rx.recv().await {
                // 文本塑形
                let mut buffers: Vec<glyphon::Buffer> = Vec::with_capacity(data.texts.len());
                for t in &data.texts {
                    let mut buffer = glyphon::Buffer::new(
                        &mut font_system,
                        glyphon::Metrics::new(t.font_size as f32, (t.font_size as f32) * 1.2),
                    );
                    buffer.set_size(Some(t.w as f32), Some(t.h as f32));
                    buffer.set_text(
                        &t.text,
                        &glyphon::Attrs::new()
                            .color(glyphon::Color::rgb(t.color.0, t.color.1, t.color.2)),
                        glyphon::Shaping::Advanced,
                        Some(glyphon::cosmic_text::Align::Center),
                    );
                    buffer.shape_until_scroll(&mut font_system, false);
                    buffers.push(buffer);
                }

                let draw_list = FrameDrawList {
                    texts: data.texts,
                    shaped_text_buffers: Some(buffers),
                    ..Default::default()
                };

                let _ = ready_tx.send(Some(Arc::new(draw_list)));
            }
        });

        FramePipeline {
            ready_rx: RefCell::new(ready_rx),
            prep_tx,
            _task: task,
        }
    }

    /// 请求后台准备下一帧（非阻塞）。
    pub fn request_prepare(&self, data: FramePrepData) {
        let _ = self.prep_tx.try_send(data);
    }

    /// 尝试获取最新已准备好的帧（非阻塞）。
    pub fn try_get_ready(&self) -> Option<Arc<FrameDrawList>> {
        self.ready_rx.borrow().borrow().clone()
    }
}
