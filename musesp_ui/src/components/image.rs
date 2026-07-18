use async_trait::async_trait;

use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::UIRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode {
    Cover,
    Centered,
    KeepRate,
    Origin,
}

/// 已加载的图片 RGBA 数据
#[derive(Clone)]
pub struct ImageData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct Image {
    pub base: ComponentBase,
    pub path: String,
    pub h_mode: ImageMode,
    pub v_mode: ImageMode,
    /// 已加载的图片数据（None 表示加载失败或路径为空）
    image_data: Option<ImageData>,
}

impl Image {
    pub async fn new(
        path: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        h_mode: ImageMode,
        v_mode: ImageMode,
    ) -> Box<Self> {
        let image_data = Self::load_image(path).await;
        Box::new(Image {
            base: ComponentBase::new(x, y, width, height),
            path: path.to_string(),
            h_mode,
            v_mode,
            image_data,
        })
    }

    pub async fn set_image(&mut self, path: &str) {
        self.path = path.to_string();
        self.image_data = Self::load_image(path).await;
    }

    async fn load_image(path: &str) -> Option<ImageData> {
        if path.is_empty() {
            return None;
        }
        if path.ends_with(".svg") {
            return Self::load_svg(path).await;
        }
        let path_owned = path.to_owned();
        let img = tokio::task::spawn_blocking(move || image::open(path_owned))
            .await
            .ok()?
            .ok()?;
        let rgba = img.into_rgba8();
        let (w, h) = rgba.dimensions();
        Some(ImageData {
            rgba: rgba.into_raw(),
            width: w,
            height: h,
        })
    }

    async fn load_svg(path: &str) -> Option<ImageData> {
        let svg_data = tokio::fs::read(path).await.ok()?;
        let opts = usvg::Options::default();
        let tree = usvg::Tree::from_data(&svg_data, &opts).ok()?;
        let size = tree.size();
        // 以 SVG 自然尺寸渲染，至少 128px 保证清晰度
        let min_dim = (size.width() as f32).min(size.height() as f32);
        let scale = (128.0_f32 / min_dim).max(1.0);
        let w = (size.width() as f32 * scale).ceil() as u32;
        let h = (size.height() as f32 * scale).ceil() as u32;
        let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        Some(ImageData {
            rgba: pixmap.take(),
            width: w,
            height: h,
        })
    }

    /// 对齐 Python `_display_size`：根据 h_mode / v_mode 计算实际显示尺寸
    fn display_size(&self, iw: i32, ih: i32) -> (i32, i32) {
        let h_mode = self.h_mode;
        let v_mode = self.v_mode;

        // --- 宽度 ---
        let dw = match h_mode {
            ImageMode::Cover => self.base.width,
            ImageMode::Origin => iw,
            ImageMode::KeepRate => {
                if v_mode == ImageMode::Cover {
                    std::cmp::max(1, iw * self.base.height / ih)
                } else {
                    iw
                }
            }
            ImageMode::Centered => iw,
        };

        // --- 高度 ---
        let dh = match v_mode {
            ImageMode::Cover => self.base.height,
            ImageMode::Origin => ih,
            ImageMode::KeepRate => {
                if h_mode == ImageMode::Cover {
                    std::cmp::max(1, ih * self.base.width / iw)
                } else {
                    ih
                }
            }
            ImageMode::Centered => ih,
        };

        (dw, dh)
    }
}

#[async_trait]
impl ComponentTrait for Image {
    fn base(&self) -> &ComponentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn draw_self(&self, renderer: &mut UIRenderer, draw_x: i32, draw_y: i32) {
        let (iw, ih) = match &self.image_data {
            Some(d) => (d.width as i32, d.height as i32),
            None => return,
        };

        let (dw, dh) = self.display_size(iw, ih);

        // COVER / ORIGIN → 原点；CENTERED / KEEP_RATE → 居中
        let dx = if matches!(self.h_mode, ImageMode::Centered | ImageMode::KeepRate) {
            draw_x + (self.base.width - dw) / 2
        } else {
            draw_x
        };
        let dy = if matches!(self.v_mode, ImageMode::Centered | ImageMode::KeepRate) {
            draw_y + (self.base.height - dh) / 2
        } else {
            draw_y
        };

        renderer.draw_image(dx, dy, dw, dh, self.image_data.as_ref());
    }

    async fn set_image_path(&mut self, path: &str) {
        self.set_image(path).await;
    }
}
