use musesp_ui::components::core::Constraintable;
use musesp_ui::components::image_button::ImageButton;
use musesp_ui::components::renderer_canvas::RendererCanvas;
use musesp_ui::components::VertexLayoutDesc;
use musesp_ui::router::{AnyPage, NavAction, Page};

use crate::menu_page::MenuPage;

pub struct GameplayPage {
    pub page: Page,
}

impl GameplayPage {
    pub fn new() -> Self {
        GameplayPage { page: Page::new() }
    }
}

#[async_trait::async_trait]
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

    async fn build(&mut self) {
        // 顶点着色器：简单的 2D 位置 → NDC 变换
        let shader = r#"
struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    return VertexOutput(vec4<f32>(in.pos, 0.0, 1.0), in.color);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

        let layout = VertexLayoutDesc {
            array_stride: 24,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: vec![
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 8,
                    shader_location: 1,
                },
            ],
        };

        let (mut canvas, _sender) = RendererCanvas::new(shader, layout, 0, 0, 0, 0);
        canvas.base.h_constraint = Constraintable::Maximum;
        canvas.base.v_constraint = Constraintable::Maximum;
        self.page.root.children.push(canvas);

        let nav = self.page.nav.clone().unwrap();
        let mut btn = ImageButton::new("assets/ui/menu_button.svg", "", 16, 16, 44, 44, 18).await;
        btn.base.name = Some("menu_btn".into());
        btn.base.h_constraint = Constraintable::None;
        btn.base.v_constraint = Constraintable::None;
        let n = nav.clone();
        btn.base.bind_mouse_click(Box::new(move |_| {
            let _ = n.blocking_send(NavAction::Push(Box::new(MenuPage::new())));
            false
        }));
        self.page.root.children.push(btn);
    }

    async fn dispatch_event(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::ElementState;
        use winit::keyboard::{KeyCode, PhysicalKey};
        if let winit::event::WindowEvent::KeyboardInput {
            event: key_event, ..
        } = event
        {
            if key_event.state == ElementState::Pressed
                && key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
            {
                // 模拟菜单按钮鼠标点击（对齐 Python: 创建 dummy event 并 emit 到按钮）
                if let Some(btn) = self.page.root.find_by_name_mut("menu_btn") {
                    btn.emit("mouse_click", None);
                }
                return;
            }
        }
        self.page.dispatch_event(event);
    }
}
