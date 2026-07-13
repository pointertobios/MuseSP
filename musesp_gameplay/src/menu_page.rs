use winit::event::{ElementState, WindowEvent};

use musesp_config::config::Config;
use musesp_ui::components::core::{ComponentBase, ComponentTrait, Constraintable, Direction};
use musesp_ui::components::image_button::ImageButton;
use musesp_ui::components::spacer::Spacer;
use musesp_ui::renderer::UIRenderer;
use musesp_ui::router::{AnyPage, NavAction, Page};

pub struct MenuPage {
    pub page: Page,
    menu_x: i32,
    menu_y: i32,
    menu_w: i32,
    menu_h: i32,
}


impl MenuPage {
    pub fn new() -> Self {
        MenuPage {
            page: Page::new(),
            menu_x: 0,
            menu_y: 0,
            menu_w: 0,
            menu_h: 0,
        }
    }
}

impl AnyPage for MenuPage {
    fn page(&self) -> &Page {
        &self.page
    }
    fn page_mut(&mut self) -> &mut Page {
        &mut self.page
    }
    fn full_shadow_promise(&self) -> bool {
        false
    }

    fn build(&mut self) {
        self.page.root.layout_direction = Direction::Vertical;

        let mut top = Spacer::new(0, 0);
        top.base.v_constraint = Constraintable::Maximum;
        self.page.root.children.push(top);

        let mut row = ComponentBase::new(0, 0, 0, 0);
        row.layout_direction = Direction::Horizontal;
        row.v_constraint = Constraintable::Minimum;
        row.h_constraint = Constraintable::Minimum;
        row.min_height = 36;

        let btns: [(&str, &str); 4] = [
            ("assets/ui/cancel_button.svg", "取消"),
            ("assets/ui/continue_button.svg", "继续"),
            ("assets/ui/replay_button.svg", "重来"),
            ("assets/ui/exit_button.svg", "退出"),
        ];

        let nav = self.page.nav.clone().unwrap();

        for (i, (path, label)) in btns.iter().enumerate() {
            if i == 0 {
                let mut s = Spacer::new(8, 0);
                s.base.h_constraint = Constraintable::Minimum;
                s.base.min_width = 8;
                row.children.push(s);
            }
            let mut btn = ImageButton::new(path, label, 0, 0, 130, 36, 14);
            btn.base.h_constraint = Constraintable::Minimum;
            btn.base.v_constraint = Constraintable::Minimum;
            btn.base.min_width = 120;
            btn.base.min_height = 36;

            let n = nav.clone();
            let lbl = *label;
            btn.base.bind_mouse_click(Box::new(move |_| {
                match lbl {
                    "取消" => {
                        let _ = n.send(NavAction::Pop);
                    }
                    "退出" => {
                        let _ = n.send(NavAction::Pop);
                        let _ = n.send(NavAction::Pop);
                    }
                    _ => {}
                }
                false
            }));
            row.children.push(btn);

            let mut s = Spacer::new(8, 0);
            s.base.h_constraint = Constraintable::Minimum;
            s.base.min_width = 8;
            row.children.push(s);
        }
        self.page
            .root
            .children
            .push(Box::new(row));

        let mut bot = Spacer::new(0, 0);
        bot.base.v_constraint = Constraintable::Maximum;
        self.page.root.children.push(bot);
    }

    fn prepare_layout(&mut self) {
        let rw = self.page.root.width;
        let rh = self.page.root.height;
        let mw = rw * 2 / 5;
        let mh = rh / 4;
        self.menu_x = (rw - mw) / 2;
        self.menu_y = (rh - mh) / 2;
        self.menu_w = mw;
        self.menu_h = mh;
        self.page.root.x = self.menu_x;
        self.page.root.y = self.menu_y;
        self.page.root.width = self.menu_w;
        self.page.root.height = self.menu_h;
        self.page.root.layout(None);
    }

    fn draw(&self, renderer: &mut UIRenderer) {
        renderer.draw_rect(
            self.menu_x,
            self.menu_y,
            self.menu_w,
            self.menu_h,
            (255, 255, 255, 180),
        );
        self.page.draw(renderer);
    }

    fn draw_debug(&self, renderer: &mut UIRenderer, config: &Config) {
        self.page.draw_debug(renderer, config);
    }

    fn dispatch_event(&mut self, event: &WindowEvent) {
        use winit::event::WindowEvent;
        use winit::keyboard::{KeyCode, PhysicalKey};

        if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = event
        {
            if key_event.state == ElementState::Pressed
                && key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
            {
                if let Some(ref nav) = self.page.nav {
                    let _ = nav.send(NavAction::Pop);
                }
                return;
            }
        }

        let in_menu = match event {
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as i32;
                let py = position.y as i32;
                self.menu_x <= px
                    && px < self.menu_x + self.menu_w
                    && self.menu_y <= py
                    && py < self.menu_y + self.menu_h
            }
            WindowEvent::MouseInput { .. } => {
                let px = self.page.root.cursor_x as i32;
                let py = self.page.root.cursor_y as i32;
                self.menu_x <= px
                    && px < self.menu_x + self.menu_w
                    && self.menu_y <= py
                    && py < self.menu_y + self.menu_h
            }
            _ => true,
        };
        if !in_menu {
            return;
        }
        self.page.dispatch_event(event);
    }
}
