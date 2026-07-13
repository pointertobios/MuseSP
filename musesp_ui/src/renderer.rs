pub struct UIRenderer {
    pub rects: Vec<DrawRect>,
    pub texts: Vec<DrawText>,
    pub images: Vec<DrawImage>,
    pub draw_3ds: Vec<Draw3D>,
}

#[derive(Debug, Clone)]
pub struct DrawRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub color: (u8, u8, u8, u8),
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
}

#[derive(Debug, Clone)]
pub struct DrawImage {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct Draw3D {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub vbo: Vec<f32>,
    pub ibo: Vec<u32>,
}

impl UIRenderer {
    pub fn new() -> Self {
        UIRenderer {
            rects: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
            draw_3ds: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.rects.clear();
        self.texts.clear();
        self.images.clear();
        self.draw_3ds.clear();
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: (u8, u8, u8, u8)) {
        self.rects.push(DrawRect { x, y, w, h, color });
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
        });
    }

    pub fn draw_image(&mut self, x: i32, y: i32, w: i32, h: i32, path: &str) {
        self.images.push(DrawImage {
            x,
            y,
            w,
            h,
            path: path.to_string(),
        });
    }

    pub fn draw_3d(&mut self, x: i32, y: i32, w: i32, h: i32, vbo: &[f32], ibo: &[u32]) {
        self.draw_3ds.push(Draw3D {
            x,
            y,
            w,
            h,
            vbo: vbo.to_vec(),
            ibo: ibo.to_vec(),
        });
    }
}
