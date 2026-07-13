use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::UIRenderer;

pub struct Canvas { pub base: ComponentBase }

impl Canvas {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Box<Self> {
        Box::new(Canvas { base: ComponentBase::new(x, y, width, height) })
    }
}

impl ComponentTrait for Canvas {
    fn base(&self) -> &ComponentBase { &self.base }
    fn base_mut(&mut self) -> &mut ComponentBase { &mut self.base }
    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        renderer.draw_rect(dx, dy, self.base.width, self.base.height, (0, 0, 0, 255));
    }
}
