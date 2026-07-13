use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use crate::components::core::{ComponentBase, ComponentTrait};
use crate::renderer::UIRenderer;

pub struct ScrollList {
    pub base: ComponentBase,
    pub item_height: i32,
    pub scroll: i32,
    pub max_scroll: i32,
    pub on_select: Option<Box<dyn FnMut(&str) + Send>>,
    pub selected_id: Option<String>,
}

impl ScrollList {
    pub fn new(x: i32, y: i32, width: i32, height: i32, item_height: i32) -> Box<Self> {
        Box::new(ScrollList {
            base: ComponentBase::new(x, y, width, height),
            item_height,
            scroll: 0,
            max_scroll: 0,
            on_select: None,
            selected_id: None,
        })
    }

    pub fn set_items(&mut self, items: Vec<Box<dyn ComponentTrait>>) {
        self.base.children.clear();
        let count = items.len() as i32;
        for mut item in items {
            item.base_mut().height = self.item_height;
            self.base.children.push(item);
        }
        self.max_scroll = (count * self.item_height - self.base.height).max(0);
        self.scroll = self.scroll.min(self.max_scroll);
        self.update_positions();
        self.propagate_width();
    }

    pub fn bind_on_select(&mut self, handler: Box<dyn FnMut(&str) + Send>) {
        self.on_select = Some(handler);
    }

    fn update_positions(&mut self) {
        for (i, child) in self.base.children.iter_mut().enumerate() {
            child.base_mut().y = i as i32 * self.item_height - self.scroll;
        }
    }

    fn propagate_width(&mut self) {
        let w = self.base.width;
        Self::propagate_width_recursive(&mut self.base.children, w);
    }

    fn propagate_width_recursive(children: &mut [Box<dyn ComponentTrait>], width: i32) {
        for child in children {
            child.base_mut().width = width;
            Self::propagate_width_recursive(child.base_mut().children.as_mut_slice(), width);
        }
    }
}

impl ComponentTrait for ScrollList {
    fn base(&self) -> &ComponentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }
    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        renderer.draw_rect(dx, dy, self.base.width, self.base.height, (30, 30, 30, 255));
        let total = self.base.children.len() as i32 * self.item_height;
        let max_scroll = (total - self.base.height).max(0);
        if max_scroll > 0 {
            let bar_w = 4i32;
            let bar_h = (self.base.height * self.base.height / total).max(20);
            let track_h = self.base.height - bar_h;
            let bar_y = if max_scroll > 0 {
                dy + self.scroll * track_h / max_scroll
            } else {
                dy
            };
            let bar_x = dx + self.base.width - bar_w - 2;
            renderer.draw_rect(bar_x, bar_y, bar_w, bar_h, (100, 100, 100, 255));
        }
    }

    fn draw(&self, renderer: &mut UIRenderer, offset_x: i32, offset_y: i32) {
        let dx = self.base.x + offset_x;
        let dy = self.base.y + offset_y;
        self.draw_self(renderer, dx, dy);
        for child in &self.base.children {
            let cy = child.base().y;
            if -child.base().height < cy && cy < self.base.height {
                child.draw(renderer, dx, dy);
            }
        }
    }

    fn dispatch_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                let (lx, ly) = self.base.local_pos(self.base.cursor_x, self.base.cursor_y);
                if self.base.in_rect(lx, ly) {
                    let scroll_amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => (*y * 20.0) as i32,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                    };
                    let new_scroll = (self.scroll - scroll_amount).clamp(0, self.max_scroll);
                    if new_scroll != self.scroll {
                        self.scroll = new_scroll;
                        self.update_positions();
                    }
                    return true;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (lx, ly) = self.base.local_pos(position.x, position.y);
                if !self.base.handle_mouse_move(lx, ly, event) {
                    return false;
                }
                let local_event = WindowEvent::CursorMoved {
                    device_id: unsafe { std::mem::zeroed() },
                    position: winit::dpi::PhysicalPosition::new(lx as f64, ly as f64),
                };
                return self.dispatch_to_visible_children(&local_event);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let (lx, ly) = self.base.local_pos(self.base.cursor_x, self.base.cursor_y);

                if *state == ElementState::Pressed && *button == MouseButton::Left {
                    if self.base.in_rect(lx, ly) {
                        for child in &self.base.children {
                            let cy = child.base().y;
                            if -child.base().height < cy && cy < self.base.height {
                                if child.base().in_rect(lx, ly - cy) {
                                    if let Some(ref item_id) = child.base().item_id {
                                        if self.selected_id.as_deref() != Some(item_id) {
                                            self.selected_id = Some(item_id.clone());
                                            if let Some(ref mut handler) = self.on_select {
                                                handler(item_id);
                                            }
                                        }
                                    }
                                    return false;
                                }
                            }
                        }
                    }
                }

                if !self.base.handle_mouse_input(*state, *button, lx, ly, event) {
                    return false;
                }
                return self.dispatch_to_visible_children(event);
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if !self.base.handle_keyboard(key_event, event) {
                    return false;
                }
                return self.dispatch_to_visible_children(event);
            }
            _ => return self.dispatch_to_visible_children(event),
        }
        true
    }

    fn set_scroll_items(&mut self, items: Vec<Box<dyn ComponentTrait>>) {
        self.set_items(items);
    }
}

impl ScrollList {
    fn dispatch_to_visible_children(&mut self, event: &WindowEvent) -> bool {
        let n = self.base.children.len();
        for i in 0..n {
            let cy = self.base.children[i].base().y;
            if -self.base.children[i].base().height < cy && cy < self.base.height {
                self.base.children[i].base_mut().cursor_x = self.base.cursor_x;
                self.base.children[i].base_mut().cursor_y = self.base.cursor_y;
                if !self.base.children[i].dispatch_event(event) {
                    return false;
                }
            }
        }
        true
    }
}
