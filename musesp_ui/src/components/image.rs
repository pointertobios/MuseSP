use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::UIRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode { Cover, Centered, KeepRate, Origin }

pub struct Image {
    pub base: ComponentBase,
    pub path: String,
    pub h_mode: ImageMode,
    pub v_mode: ImageMode,
}

impl Image {
    pub fn new(path: &str, x: i32, y: i32, width: i32, height: i32, h_mode: ImageMode, v_mode: ImageMode) -> Box<Self> {
        Box::new(Image { base: ComponentBase::new(x, y, width, height), path: path.to_string(), h_mode, v_mode })
    }

    pub fn set_image(&mut self, path: &str) {
        self.path = path.to_string();
    }
}

impl ComponentTrait for Image {
    fn base(&self) -> &ComponentBase { &self.base }
    fn base_mut(&mut self) -> &mut ComponentBase { &mut self.base }
    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        renderer.draw_image(dx, dy, self.base.width, self.base.height, &self.path);
    }

    fn set_image_path(&mut self, path: &str) {
        self.path = path.to_string();
    }
}
