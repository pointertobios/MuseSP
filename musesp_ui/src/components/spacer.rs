use winit::event::WindowEvent;

use crate::components::core::{ComponentBase, ComponentTrait, Constraintable};

pub struct Spacer {
    pub base: ComponentBase,
}

impl Spacer {
    pub fn new(width: i32, height: i32) -> Box<Self> {
        let mut s = Spacer {
            base: ComponentBase::new(0, 0, width, height),
        };
        s.base.h_constraint = Constraintable::Minimum;
        s.base.v_constraint = Constraintable::Minimum;
        s.base.debug_border_color = (139, 0, 0, 255);
        Box::new(s)
    }
}

impl ComponentTrait for Spacer {
    fn base(&self) -> &ComponentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn dispatch_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let (lx, ly) = self.base.local_pos(position.x, position.y);
                let local_pos = winit::dpi::PhysicalPosition::new(lx as f64, ly as f64);
                let local_event = WindowEvent::CursorMoved {
                    device_id: unsafe { std::mem::zeroed() },
                    position: local_pos,
                };
                for child in &mut self.base.children {
                    if !child.dispatch_event(&local_event) {
                        return false;
                    }
                }
            }
            WindowEvent::MouseInput { .. } => {
                let (lx, ly) = self.base.local_pos(self.base.cursor_x, self.base.cursor_y);
                for child in &mut self.base.children {
                    child.base_mut().cursor_x = lx as f64;
                    child.base_mut().cursor_y = ly as f64;
                    if !child.dispatch_event(event) {
                        return false;
                    }
                }
            }
            _ => {
                for child in &mut self.base.children {
                    if !child.dispatch_event(event) {
                        return false;
                    }
                }
            }
        }
        true
    }
}
