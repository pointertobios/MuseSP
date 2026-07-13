use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::UIRenderer;

pub struct RendererCanvas {
    pub base: ComponentBase,
    pub vbo: Vec<f32>,
    pub ibo: Vec<u32>,
}

impl RendererCanvas {
    pub fn new(x: i32, y: i32, width: i32, height: i32, vbo: Vec<f32>, ibo: Vec<u32>) -> Box<Self> {
        Box::new(RendererCanvas { base: ComponentBase::new(x, y, width, height), vbo, ibo })
    }
}

impl ComponentTrait for RendererCanvas {
    fn base(&self) -> &ComponentBase { &self.base }
    fn base_mut(&mut self) -> &mut ComponentBase { &mut self.base }
    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        renderer.draw_3d(dx, dy, self.base.width, self.base.height, &self.vbo, &self.ibo);
    }
}
