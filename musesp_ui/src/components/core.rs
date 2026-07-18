use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::PhysicalKey;

use musesp_config::config::Config;

use crate::renderer::UIRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraintable {
    None,
    Minimum,
    Maximum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Vertical,
    Horizontal,
}

pub type EventHandler =
    Box<dyn FnMut(&WindowEvent) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send>;

#[async_trait]
pub trait ComponentTrait: Any + Send {
    fn base(&self) -> &ComponentBase;
    fn base_mut(&mut self) -> &mut ComponentBase;

    fn as_any(&self) -> &dyn Any
    where
        Self: Sized,
    {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any
    where
        Self: Sized,
    {
        self
    }

    fn draw_self(&self, _r: &mut UIRenderer, _dx: i32, _dy: i32) {}

    fn draw(&self, renderer: &mut UIRenderer, offset_x: i32, offset_y: i32) {
        let base = self.base();
        let dx = base.x + offset_x;
        let dy = base.y + offset_y;
        self.draw_self(renderer, dx, dy);
        for child in &base.children {
            child.draw(renderer, dx, dy);
        }
    }

    async fn dispatch_event(&mut self, event: &WindowEvent) -> bool {
        self.base_mut().dispatch_event(event).await
    }

    fn set_scroll_items(&mut self, _items: Vec<Box<dyn ComponentTrait>>) {}
    async fn set_image_path(&mut self, _path: &str) {}

    /// 递归计算整棵子树的布局。子类可覆写以在布局后执行额外操作。
    fn layout(&mut self, h_override: Option<Direction>) {
        self.base_mut().do_layout(h_override);
    }

    fn item_id(&self) -> Option<&str> {
        self.base().item_id.as_deref()
    }
}

pub struct ComponentBase {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub min_width: i32,
    pub min_height: i32,
    pub max_width: i32,
    pub max_height: i32,
    pub h_constraint: Constraintable,
    pub v_constraint: Constraintable,
    pub layout_direction: Direction,
    pub centered_horizontal: bool,
    pub centered_vertical: bool,
    pub hovered: bool,
    pub pressed: bool,
    pressed_button: Option<MouseButton>,
    pressed_keys: Vec<u32>,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub children: Vec<Box<dyn ComponentTrait>>,
    handlers: HashMap<String, Vec<EventHandler>>,
    pub debug_border_color: (u8, u8, u8, u8),
    pub item_id: Option<String>,
    pub name: Option<String>,
}

impl ComponentTrait for ComponentBase {
    fn base(&self) -> &ComponentBase {
        self
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        self
    }
}

impl ComponentBase {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        let mut handlers = HashMap::new();
        for key in &[
            "mouse_enter",
            "mouse_exit",
            "mouse_down",
            "mouse_up",
            "mouse_click",
            "mouse_hold",
            "key_down",
            "key_up",
            "key_click",
            "key_hold",
        ] {
            handlers.insert(key.to_string(), Vec::new());
        }
        ComponentBase {
            x,
            y,
            width,
            height,
            min_width: 0,
            min_height: 0,
            max_width: 0,
            max_height: 0,
            h_constraint: Constraintable::None,
            v_constraint: Constraintable::None,
            layout_direction: Direction::Vertical,
            centered_horizontal: true,
            centered_vertical: true,
            hovered: false,
            pressed: false,
            pressed_button: None,
            pressed_keys: Vec::new(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            children: Vec::new(),
            handlers,
            debug_border_color: (0, 255, 0, 255),
            item_id: None,
            name: None,
        }
    }

    pub fn bind_event(&mut self, event: &str, handler: EventHandler) {
        self.handlers
            .entry(event.to_string())
            .or_default()
            .push(handler);
    }

    pub fn bind_mouse_enter(&mut self, handler: EventHandler) {
        self.bind_event("mouse_enter", handler);
    }

    pub fn bind_mouse_exit(&mut self, handler: EventHandler) {
        self.bind_event("mouse_exit", handler);
    }

    pub fn bind_mouse_down(&mut self, handler: EventHandler) {
        self.bind_event("mouse_down", handler);
    }

    pub fn bind_mouse_up(&mut self, handler: EventHandler) {
        self.bind_event("mouse_up", handler);
    }

    pub fn bind_mouse_click(&mut self, handler: EventHandler) {
        self.bind_event("mouse_click", handler);
    }

    pub fn bind_key_down(&mut self, handler: EventHandler) {
        self.bind_event("key_down", handler);
    }

    pub fn bind_key_up(&mut self, handler: EventHandler) {
        self.bind_event("key_up", handler);
    }

    pub fn bind_mouse_hold(&mut self, handler: EventHandler) {
        self.bind_event("mouse_hold", handler);
    }

    pub fn bind_key_click(&mut self, handler: EventHandler) {
        self.bind_event("key_click", handler);
    }

    pub fn bind_key_hold(&mut self, handler: EventHandler) {
        self.bind_event("key_hold", handler);
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    pub fn pressed(&self) -> bool {
        self.pressed
    }

    pub fn in_rect(&self, px: i32, py: i32) -> bool {
        px >= 0 && px <= self.width && py >= 0 && py <= self.height
    }

    pub fn find_by_name(&self, name: &str) -> Option<&ComponentBase> {
        if self.name.as_deref() == Some(name) {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.base().find_by_name(name) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_by_name_mut(&mut self, name: &str) -> Option<&mut ComponentBase> {
        if self.name.as_deref() == Some(name) {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.base_mut().find_by_name_mut(name) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_component_by_name(&self, name: &str) -> Option<&dyn ComponentTrait> {
        if self.name.as_deref() == Some(name) {
            return None;
        }
        for child in &self.children {
            if child.base().name.as_deref() == Some(name) {
                return Some(child.as_ref());
            }
            if let Some(found) = child.base().find_component_by_name(name) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_component_by_name_mut(&mut self, name: &str) -> Option<&mut dyn ComponentTrait> {
        if self.name.as_deref() == Some(name) {
            return None;
        }
        for child in &mut self.children {
            if child.base().name.as_deref() == Some(name) {
                return Some(child.as_mut());
            }
            if let Some(found) = child.base_mut().find_component_by_name_mut(name) {
                return Some(found);
            }
        }
        None
    }

    pub async fn emit(&mut self, name: &str, event: Option<&WindowEvent>) -> bool {
        let mut propagate = true;
        if let Some(handlers) = self.handlers.get_mut(name) {
            if let Some(ev) = event {
                for handler in handlers.iter_mut() {
                    if !handler(ev).await {
                        propagate = false;
                    }
                }
            } else {
                let dummy = WindowEvent::CursorMoved {
                    device_id: winit::event::DeviceId::dummy(),
                    position: winit::dpi::PhysicalPosition::new(0.0, 0.0),
                };
                for handler in handlers.iter_mut() {
                    if !handler(&dummy).await {
                        propagate = false;
                    }
                }
            }
        }
        propagate
    }

    pub(crate) fn local_pos(&self, px: f64, py: f64) -> (i32, i32) {
        ((px as i32) - self.x, (py as i32) - self.y)
    }

    pub async fn dispatch_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                self.cursor_x = position.x;
                self.cursor_y = position.y;
                let (lx, ly) = self.local_pos(position.x, position.y);
                if !self.handle_mouse_move(lx, ly, event).await {
                    return false;
                }
                let local_pos = winit::dpi::PhysicalPosition::new(lx as f64, ly as f64);
                let local_event = WindowEvent::CursorMoved {
                    device_id: *device_id,
                    position: local_pos,
                };
                let n = self.children.len();
                for i in 0..n {
                    if !self.children[i].dispatch_event(&local_event).await {
                        return false;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let (lx, ly) = self.local_pos(self.cursor_x, self.cursor_y);
                if !self
                    .handle_mouse_input(*state, *button, lx, ly, event)
                    .await
                {
                    return false;
                }
                let n = self.children.len();
                for i in 0..n {
                    self.children[i].base_mut().cursor_x = lx as f64;
                    self.children[i].base_mut().cursor_y = ly as f64;
                    if !self.children[i].dispatch_event(event).await {
                        return false;
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if !self.handle_keyboard(key_event, event).await {
                    return false;
                }
                let n = self.children.len();
                for i in 0..n {
                    if !self.children[i].dispatch_event(event).await {
                        return false;
                    }
                }
            }
            _ => {
                let n = self.children.len();
                for i in 0..n {
                    if !self.children[i].dispatch_event(event).await {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub(crate) async fn handle_mouse_move(
        &mut self,
        lx: i32,
        ly: i32,
        event: &WindowEvent,
    ) -> bool {
        let was_hovered = self.hovered;
        self.hovered = self.in_rect(lx, ly);
        if self.hovered && !was_hovered {
            self.emit("mouse_enter", Some(event)).await
        } else if !self.hovered && was_hovered {
            self.emit("mouse_exit", Some(event)).await
        } else {
            true
        }
    }

    pub(crate) async fn handle_mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        lx: i32,
        ly: i32,
        event: &WindowEvent,
    ) -> bool {
        match state {
            ElementState::Pressed => {
                if self.in_rect(lx, ly) {
                    self.pressed = true;
                    self.pressed_button = Some(button);
                    self.emit("mouse_down", Some(event)).await
                } else {
                    true
                }
            }
            ElementState::Released => {
                let was_pressed = self.pressed;
                self.pressed = false;
                self.pressed_button = None;
                if !self.emit("mouse_up", Some(event)).await {
                    return false;
                }
                if was_pressed && self.in_rect(lx, ly) {
                    self.emit("mouse_click", Some(event)).await
                } else {
                    true
                }
            }
        }
    }

    pub(crate) async fn handle_keyboard(
        &mut self,
        key_event: &winit::event::KeyEvent,
        event: &WindowEvent,
    ) -> bool {
        let PhysicalKey::Code(keycode) = key_event.physical_key else {
            return true;
        };
        match key_event.state {
            ElementState::Pressed => {
                self.pressed_keys.push(keycode as u32);
                self.emit("key_down", Some(event)).await
            }
            ElementState::Released => {
                self.pressed_keys.retain(|&x| x != keycode as u32);
                self.emit("key_up", Some(event)).await
            }
        }
    }

    pub async fn force_mouse_exit(&mut self) {
        if self.hovered || self.pressed {
            if self.pressed {
                self.pressed = false;
                self.pressed_button = None;
                self.emit("mouse_up", None).await;
            }
            self.hovered = false;
            self.emit("mouse_exit", None).await;
        }
        // 递归调用需要 Box::pin 避免无限大小 future
        for child in &mut self.children {
            Box::pin(child.base_mut().force_mouse_exit()).await;
        }
    }

    /// 便捷方法：通过 ComponentTrait 分发 layout。调用方持有 `ComponentBase` 时使用。
    pub fn layout(&mut self, h_override: Option<Direction>) {
        <ComponentBase as ComponentTrait>::layout(self, h_override);
    }

    /// 内部布局实现：排列子组件后递归 layout 子组件（通过 trait 分发）。
    pub(crate) fn do_layout(&mut self, h_override: Option<Direction>) {
        let dir = h_override.unwrap_or(self.layout_direction);
        if dir == Direction::Vertical {
            self.layout_axis(false);
        } else {
            self.layout_axis(true);
        }
        let n = self.children.len();
        for i in 0..n {
            self.children[i].layout(None);
        }
    }

    fn layout_axis(&mut self, horizontal: bool) {
        let parent_size = if horizontal { self.width } else { self.height };
        let cross = if horizontal { self.height } else { self.width };

        let get_constraint: fn(&ComponentBase) -> Constraintable = if horizontal {
            |c: &ComponentBase| c.h_constraint
        } else {
            |c: &ComponentBase| c.v_constraint
        };
        let get_min: fn(&ComponentBase) -> i32 = if horizontal {
            |c: &ComponentBase| c.min_width
        } else {
            |c: &ComponentBase| c.min_height
        };
        let get_max: fn(&ComponentBase) -> i32 = if horizontal {
            |c: &ComponentBase| c.max_width
        } else {
            |c: &ComponentBase| c.max_height
        };

        let constrained: Vec<usize> = self
            .children
            .iter()
            .enumerate()
            .filter(|(_, c)| get_constraint(c.base()) != Constraintable::None)
            .map(|(i, _)| i)
            .collect();

        if constrained.is_empty() {
            return;
        }

        let mut maximums: Vec<usize> = Vec::new();
        let mut minimum_fixed: Vec<usize> = Vec::new();
        let mut minimum_zero: Vec<usize> = Vec::new();

        for &i in &constrained {
            match get_constraint(self.children[i].base()) {
                Constraintable::Maximum => maximums.push(i),
                Constraintable::Minimum if get_min(self.children[i].base()) > 0 => {
                    minimum_fixed.push(i)
                }
                _ => minimum_zero.push(i),
            }
        }

        let total_fixed: i32 = minimum_fixed
            .iter()
            .map(|&i| get_min(self.children[i].base()))
            .sum();
        let mut sizes: HashMap<usize, i32> = HashMap::new();

        for &i in &minimum_fixed {
            sizes.insert(i, get_min(self.children[i].base()));
        }
        for &i in &minimum_zero {
            sizes.insert(i, 0);
        }

        if !maximums.is_empty() {
            let mut uncapped: Vec<usize> = maximums.clone();
            loop {
                let capped_total: i32 = maximums
                    .iter()
                    .filter(|i| !uncapped.contains(i))
                    .map(|&i| sizes.get(&i).copied().unwrap_or(0))
                    .sum();
                let uncapped_min: i32 = uncapped
                    .iter()
                    .map(|&i| get_min(self.children[i].base()))
                    .sum();
                let remaining = parent_size - total_fixed - capped_total - uncapped_min;
                let share = std::cmp::max(0, remaining / std::cmp::max(uncapped.len() as i32, 1));

                let mut still_uncapped: Vec<usize> = Vec::new();
                let mut capped_this_round = false;
                for &i in &uncapped {
                    let size = get_min(self.children[i].base()) + share;
                    let max_s = get_max(self.children[i].base());
                    if max_s > 0 && size > max_s {
                        sizes.insert(i, max_s);
                        capped_this_round = true;
                    } else {
                        still_uncapped.push(i);
                    }
                }
                if !capped_this_round {
                    for &i in &uncapped {
                        sizes.insert(i, get_min(self.children[i].base()) + share);
                    }
                    break;
                }
                uncapped = still_uncapped;
            }
        } else {
            let remaining = parent_size - total_fixed;
            let share = std::cmp::max(0, remaining / std::cmp::max(minimum_zero.len() as i32, 1));
            for &i in &minimum_zero {
                let mut size = share;
                let max_s = get_max(self.children[i].base());
                if max_s > 0 && size > max_s {
                    size = max_s;
                }
                sizes.insert(i, size);
            }
        }

        let mut pos: i32 = 0;
        for i in 0..self.children.len() {
            if get_constraint(self.children[i].base()) == Constraintable::None {
                continue;
            }
            let base = self.children[i].base_mut();
            if horizontal {
                base.x = pos;
                base.height = cross;
                base.width = sizes[&i];
            } else {
                base.y = pos;
                base.width = cross;
                base.height = sizes[&i];
            }
            pos += sizes[&i];
        }

        let remaining = parent_size - pos;
        if remaining > 0 {
            if !horizontal && self.centered_vertical {
                let offset = remaining / 2;
                for i in 0..self.children.len() {
                    if get_constraint(self.children[i].base()) != Constraintable::None {
                        self.children[i].base_mut().y += offset;
                    }
                }
            } else if horizontal && self.centered_horizontal {
                let offset = remaining / 2;
                for i in 0..self.children.len() {
                    if get_constraint(self.children[i].base()) != Constraintable::None {
                        self.children[i].base_mut().x += offset;
                    }
                }
            }
        }
    }
}

impl dyn ComponentTrait {
    pub fn draw_debug(
        &self,
        renderer: &mut UIRenderer,
        config: &Config,
        offset_x: i32,
        offset_y: i32,
    ) {
        if !config.debug.ui.component_border {
            return;
        }
        let base = self.base();
        let dx = base.x + offset_x;
        let dy = base.y + offset_y;
        let w = base.width.max(0);
        let h = base.height.max(0);
        let bw = 2;
        let c = base.debug_border_color;
        renderer.draw_rect(dx, dy, w, bw, c);
        renderer.draw_rect(dx, dy + h - bw, w, bw, c);
        renderer.draw_rect(dx, dy, bw, h, c);
        renderer.draw_rect(dx + w - bw, dy, bw, h, c);
        for child in &base.children {
            child.draw_debug(renderer, config, dx, dy);
        }
    }
}
