use std::sync::Arc;

use musesp_ui::components::core::Constraintable;
use musesp_ui::components::image_button::ImageButton;
use musesp_ui::router::{AnyPage, NavAction, Page};

use crate::gameplay::renderer3d::{self, AsyncSnapshotProducer, GameplayRenderPipeline};
use crate::menu_page::MenuPage;

pub struct GameplayPage {
    pub page: Page,
    /// 保持后台任务存活
    _producer: Option<Arc<AsyncSnapshotProducer>>,
}

impl GameplayPage {
    pub fn new() -> Self {
        GameplayPage {
            page: Page::new(),
            _producer: None,
        }
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
        // 启动异步快照生产者并注册为全局单例
        let producer = Arc::new(AsyncSnapshotProducer::new());
        renderer3d::set_snapshot_producer(Arc::clone(&producer));
        self._producer = Some(producer);

        // 创建自定义渲染管线：接管所有 compute/subdivide/line 渲染
        let shaders = self
            .page
            .shader_library
            .as_ref()
            .expect("ShaderLibrary not set");
        self.page.render_pipeline =
            Some(Box::new(GameplayRenderPipeline::new(Arc::clone(shaders))));

        let nav = self.page.nav.clone().unwrap();
        let mut btn = ImageButton::new("assets/ui/menu_button.svg", "", 16, 16, 44, 44, 18).await;
        btn.base.name = Some("menu_btn".into());
        btn.base.h_constraint = Constraintable::None;
        btn.base.v_constraint = Constraintable::None;
        let n = nav.clone();
        btn.base.bind_mouse_click(Box::new(move |_| {
            let n = n.clone();
            Box::pin(async move {
                let _ = n.send(NavAction::Push(Box::new(MenuPage::new()))).await;
                false
            })
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
                if let Some(btn) = self.page.root.find_by_name_mut("menu_btn") {
                    btn.emit("mouse_click", None).await;
                }
                return;
            }
        }
        self.page.dispatch_event(event).await;
    }
}
