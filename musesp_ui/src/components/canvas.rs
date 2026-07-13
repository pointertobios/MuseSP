use std::cell::RefCell;
use std::time::Instant;

use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::UIRenderer;

type DrawFn = Box<dyn FnMut(&mut UIRenderer, f32) + Send>;

/// 独立画布组件。
///
/// 每帧调用 `draw_fn(renderer, dt)`，其中 `dt` 为距离上一帧的毫秒数。
/// 对齐 Python `Canvas` 的行为。
pub struct Canvas {
    pub base: ComponentBase,
    draw_fn: RefCell<DrawFn>,
    last_tick: RefCell<Instant>,
}

impl Canvas {
    pub fn new(
        draw_fn: Box<dyn FnMut(&mut UIRenderer, f32) + Send>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Box<Self> {
        Box::new(Canvas {
            base: ComponentBase::new(x, y, width, height),
            draw_fn: RefCell::new(draw_fn),
            last_tick: RefCell::new(Instant::now()),
        })
    }
}

impl ComponentTrait for Canvas {
    fn base(&self) -> &ComponentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn draw_self(&self, renderer: &mut UIRenderer, _dx: i32, _dy: i32) {
        let mut last_tick = self.last_tick.borrow_mut();
        let now = Instant::now();
        let dt = (now - *last_tick).as_secs_f32() * 1000.0;
        *last_tick = now;
        drop(last_tick);

        let mut draw_fn = self.draw_fn.borrow_mut();
        draw_fn(renderer, dt);
    }
}
