use musesp_ui::components::core::Constraintable;
use musesp_ui::components::image_button::ImageButton;
use musesp_ui::components::renderer_canvas::RendererCanvas;
use musesp_ui::router::{AnyPage, NavAction, Page};

use crate::menu_page::MenuPage;

pub struct GameplayPage {
    pub page: Page,
}

impl GameplayPage {
    pub fn new() -> Self {
        GameplayPage { page: Page::new() }
    }

    fn build_test_cube() -> (Vec<f32>, Vec<u32>) {
        let s: f32 = 1.0;
        let vbo = vec![
            -s, -s, -s, 0.0, s, -s, -s, 0.0, s, s, -s, 0.0, -s, s, -s, 0.0, -s, -s, s, 0.0, s, -s,
            s, 0.0, s, s, s, 0.0, -s, s, s, 0.0,
        ];
        let ibo = vec![
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 2, 3, 7, 2, 7, 6, 0, 3, 7, 0, 7,
            4, 1, 2, 6, 1, 6, 5,
        ];
        (vbo, ibo)
    }
}

impl AnyPage for GameplayPage {
    fn page(&self) -> &Page {
        &self.page
    }
    fn page_mut(&mut self) -> &mut Page {
        &mut self.page
    }
    fn full_shadow_promise(&self) -> bool {
        true
    }

    fn initial_mode(&self) -> musesp_ui::application::RunMode {
        musesp_ui::application::RunMode::Vsync
    }

    fn on_activate(&mut self) {}

    fn build(&mut self) {
        let (vbo, ibo) = Self::build_test_cube();
        let mut canvas = RendererCanvas::new(0, 0, 0, 0, vbo, ibo);
        canvas.base.h_constraint = Constraintable::Maximum;
        canvas.base.v_constraint = Constraintable::Maximum;
        self.page.root.children.push(canvas);

        let nav = self.page.nav.clone().unwrap();
        let mut btn = ImageButton::new("assets/ui/menu_button.svg", "", 16, 16, 44, 44, 18);
        btn.base.h_constraint = Constraintable::None;
        btn.base.v_constraint = Constraintable::None;
        let n = nav.clone();
        btn.base.bind_mouse_click(Box::new(move |_| {
            let _ = n.send(NavAction::Push(Box::new(MenuPage::new())));
            false
        }));
        self.page.root.children.push(btn);
    }

    fn dispatch_event(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::ElementState;
        use winit::keyboard::{KeyCode, PhysicalKey};
        if let winit::event::WindowEvent::KeyboardInput {
            event: key_event, ..
        } = event
        {
            if key_event.state == ElementState::Pressed
                && key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
            {
                if let Some(ref nav) = self.page.nav {
                    let _ = nav.send(NavAction::Push(Box::new(MenuPage::new())));
                }
                return;
            }
        }
        self.page.dispatch_event(event);
    }
}
