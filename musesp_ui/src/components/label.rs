use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::UIRenderer;

pub struct Label {
    pub base: ComponentBase,
    pub text: String,
    pub font_size: u32,
    pub color: (u8, u8, u8),
}

impl Label {
    pub fn new(text: &str, x: i32, y: i32, width: i32, height: i32, font_size: u32, color: (u8, u8, u8)) -> Box<Self> {
        Box::new(Label {
            base: ComponentBase::new(x, y, width, height),
            text: text.to_string(),
            font_size,
            color,
        })
    }
}

impl ComponentTrait for Label {
    fn base(&self) -> &ComponentBase { &self.base }
    fn base_mut(&mut self) -> &mut ComponentBase { &mut self.base }
    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        renderer.draw_text(&self.text, dx, dy, self.base.width, self.base.height, self.font_size, self.color);
    }
}
