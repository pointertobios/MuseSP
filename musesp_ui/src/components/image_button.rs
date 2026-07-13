use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use winit::event::WindowEvent;

use crate::components::core::{ComponentBase, ComponentTrait, Constraintable, Direction, EventHandler};
use crate::components::image::{Image, ImageMode};
use crate::components::label::Label;
use crate::renderer::UIRenderer;

pub struct ImageButton {
    pub base: ComponentBase,
    enabled: Arc<AtomicBool>,
}

impl ImageButton {
    pub fn new(
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

        let mut img = Image::new(path, 0, 0, 0, 0, ImageMode::KeepRate, ImageMode::Cover);
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
            if base.layout_direction == Direction::Horizontal {
                label.base.h_constraint = Constraintable::Minimum;
                label.base.v_constraint = Constraintable::None;
            } else {
                label.base.v_constraint = Constraintable::Minimum;
                label.base.h_constraint = Constraintable::None;
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
            self.base.emit("enable", None);
        }
    }

    pub fn disable(&mut self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.enabled.store(false, Ordering::Relaxed);
            self.base.hovered = false;
            self.base.pressed = false;
            self.base.emit("disable", None);
        }
    }

    pub fn bind_enable(&mut self, handler: EventHandler) {
        self.base.bind_event("enable", handler);
    }

    pub fn bind_disable(&mut self, handler: EventHandler) {
        self.base.bind_event("disable", handler);
    }
}

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

    fn dispatch_event(&mut self, event: &WindowEvent) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return true;
        }
        self.base.dispatch_event(event)
    }
}
