use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use winit::event::WindowEvent;

use async_trait::async_trait;

use crate::components::core::{
    ComponentBase, ComponentTrait, Constraintable, Direction, EventHandler,
};
use crate::components::image::{Image, ImageMode};
use crate::components::label::Label;
use crate::renderer::UIRenderer;

pub struct ImageButton {
    pub base: ComponentBase,
    enabled: Arc<AtomicBool>,
}

impl ImageButton {
    pub async fn new(
        path: &str,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        font_size: u32,
    ) -> Box<Self> {
        let mut base = ComponentBase::new(x, y, width, height);
        base.layout_direction = Direction::Horizontal;

        let mut img = Image::new(path, 0, 0, 0, 0, ImageMode::KeepRate, ImageMode::Cover).await;
        if base.layout_direction == Direction::Horizontal {
            img.base.h_constraint = Constraintable::Maximum;
            img.base.v_constraint = Constraintable::None;
            img.base.min_width = height;
            img.base.max_width = height;
        } else {
            img.base.v_constraint = Constraintable::Maximum;
            img.base.h_constraint = Constraintable::None;
            img.base.min_height = width;
            img.base.max_height = width;
        }
        base.children.push(img);

        if !text.is_empty() {
            let mut label = Label::new(text, 0, 0, 0, 0, font_size, (220, 220, 220));
            // 估算文本像素宽度：CJK 字符 ≈ font_size，ASCII ≈ font_size * 0.55
            let tw = estimate_text_width(text, font_size);
            if base.layout_direction == Direction::Horizontal {
                label.base.h_constraint = Constraintable::Minimum;
                label.base.v_constraint = Constraintable::None;
                label.base.min_width = tw + 8;
            } else {
                label.base.v_constraint = Constraintable::Minimum;
                label.base.h_constraint = Constraintable::None;
                label.base.min_height = (font_size as i32) + 4;
            }
            base.children.push(label);
        }

        Box::new(ImageButton {
            base,
            enabled: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn enable(&mut self) {
        if !self.enabled.load(Ordering::Relaxed) {
            self.enabled.store(true, Ordering::Relaxed);
            tokio::runtime::Handle::current().block_on(self.base.emit("enable", None));
        }
    }

    pub fn disable(&mut self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.enabled.store(false, Ordering::Relaxed);
            self.base.hovered = false;
            self.base.pressed = false;
            tokio::runtime::Handle::current().block_on(self.base.emit("disable", None));
        }
    }

    pub fn bind_enable(&mut self, handler: EventHandler) {
        self.base.bind_event("enable", handler);
    }

    pub fn bind_disable(&mut self, handler: EventHandler) {
        self.base.bind_event("disable", handler);
    }
}

/// 估算文本的像素宽度：CJK 字符 ≈ font_size，ASCII/数字 ≈ font_size * 0.55。
fn estimate_text_width(text: &str, font_size: u32) -> i32 {
    let fs = font_size as f32;
    text.chars()
        .map(|c| {
            if ('\u{4E00}'..='\u{9FFF}').contains(&c)
                || ('\u{3000}'..='\u{303F}').contains(&c)
                || ('\u{FF00}'..='\u{FFEF}').contains(&c)
            {
                fs
            } else {
                fs * 0.55
            }
        })
        .sum::<f32>()
        .ceil() as i32
}

#[async_trait]
impl ComponentTrait for ImageButton {
    fn base(&self) -> &ComponentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }
    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        let bg = if !self.enabled.load(Ordering::Relaxed) {
            (80, 80, 80, 255)
        } else if self.base.pressed() {
            (100, 100, 100, 255)
        } else if self.base.hovered() {
            (140, 140, 140, 255)
        } else {
            (80, 80, 80, 255)
        };
        renderer.draw_rect(dx, dy, self.base.width, self.base.height, bg);
        if !self.enabled.load(Ordering::Relaxed) {
            renderer.draw_rect(dx, dy, self.base.width, self.base.height, (0, 0, 0, 128));
        }
    }

    async fn dispatch_event(&mut self, event: &WindowEvent) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return true;
        }
        self.base.dispatch_event(event).await
    }
}
