use crate::components::image::ImageData;

pub struct UIRenderer {
    pub rects: Vec<DrawRect>,
    pub texts: Vec<DrawText>,
    pub images: Vec<DrawImage>,
    clip_stack: Vec<(i32, i32, i32, i32)>,
}

#[derive(Debug, Clone)]
pub struct DrawRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub color: (u8, u8, u8, u8),
    pub clip_rect: Option<(u32, u32, u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct DrawText {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub font_size: u32,
    pub color: (u8, u8, u8),
    pub clip_rect: Option<(u32, u32, u32, u32)>,
}

#[derive(Clone)]
pub struct DrawImage {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub data: ImageData,
    pub clip_rect: Option<(u32, u32, u32, u32)>,
}

impl UIRenderer {
    pub fn new() -> Self {
        UIRenderer {
            rects: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
            clip_stack: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.rects.clear();
        self.texts.clear();
        self.images.clear();
        self.clip_stack.clear();
    }

    fn current_clip(&self) -> Option<(u32, u32, u32, u32)> {
        self.clip_stack.last().map(|&(x, y, w, h)| {
            (x.max(0) as u32, y.max(0) as u32, w.max(0) as u32, h.max(0) as u32)
        })
    }

    pub fn push_clip(&mut self, x: i32, y: i32, w: i32, h: i32) {
        // 与栈顶 clip 求交集
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
        self.rects.push(DrawRect { x, y, w, h, color, clip_rect: self.current_clip() });
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
            self.images.push(DrawImage { x, y, w, h, data: d.clone(), clip_rect: self.current_clip() });
        }
    }
}
