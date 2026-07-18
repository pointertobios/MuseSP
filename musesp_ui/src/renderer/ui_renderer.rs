use super::types::{
    DrawImage, DrawRect, DrawRendererCanvas, DrawText, RenderSnapshot, VertexLayoutDesc,
};
use crate::components::image::ImageData;

pub struct UIRenderer {
    pub rects: Vec<DrawRect>,
    pub texts: Vec<DrawText>,
    pub images: Vec<DrawImage>,
    pub custom_draws: Vec<DrawRendererCanvas>,
    clip_stack: Vec<(i32, i32, i32, i32)>,
}

impl UIRenderer {
    pub fn new() -> Self {
        UIRenderer {
            rects: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
            custom_draws: Vec::new(),
            clip_stack: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.rects.clear();
        self.texts.clear();
        self.images.clear();
        self.custom_draws.clear();
        self.clip_stack.clear();
    }

    fn current_clip(&self) -> Option<(u32, u32, u32, u32)> {
        self.clip_stack.last().map(|&(x, y, w, h)| {
            (
                x.max(0) as u32,
                y.max(0) as u32,
                w.max(0) as u32,
                h.max(0) as u32,
            )
        })
    }

    pub fn push_clip(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let clip = if let Some(&(cx, cy, cw, ch)) = self.clip_stack.last() {
            let nx = x.max(cx);
            let ny = y.max(cy);
            let nw = ((x + w).min(cx + cw) - nx).max(0);
            let nh = ((y + h).min(cy + ch) - ny).max(0);
            (nx, ny, nw, nh)
        } else {
            (x, y, w, h)
        };
        self.clip_stack.push(clip);
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: (u8, u8, u8, u8)) {
        self.rects.push(DrawRect {
            x,
            y,
            w,
            h,
            color,
            clip_rect: self.current_clip(),
        });
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        font_size: u32,
        color: (u8, u8, u8),
    ) {
        self.texts.push(DrawText {
            text: text.to_string(),
            x,
            y,
            w,
            h,
            font_size,
            color,
            clip_rect: self.current_clip(),
        });
    }

    pub fn draw_image(&mut self, x: i32, y: i32, w: i32, h: i32, data: Option<&ImageData>) {
        if let Some(d) = data {
            self.images.push(DrawImage {
                x,
                y,
                w,
                h,
                data: d.clone(),
                clip_rect: self.current_clip(),
            });
        }
    }

    /// 添加一个自定义着色器绘制命令。
    pub fn draw_renderer_canvas(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        shader_wgsl: &str,
        vertex_layout: &VertexLayoutDesc,
        snapshot: &RenderSnapshot,
    ) {
        self.custom_draws.push(DrawRendererCanvas {
            x,
            y,
            w,
            h,
            clip_rect: self.current_clip(),
            shader_wgsl: shader_wgsl.to_string(),
            vertex_layout: vertex_layout.clone(),
            snapshot: snapshot.clone(),
        });
    }
}
