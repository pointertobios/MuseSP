use musesp_ui::components::button::Button;
use musesp_ui::components::core::{ComponentBase, Constraintable, Direction};
use musesp_ui::components::label::Label;
use musesp_ui::components::spacer::Spacer;
use musesp_ui::router::{AnyPage, NavAction, Page};
use std::sync::atomic::Ordering;

use crate::pages::music_list::MusicListPage;

pub struct HomePage {
    pub page: Page,
}

impl HomePage {
    pub fn new() -> Self {
        HomePage { page: Page::new() }
    }
}

#[async_trait::async_trait]
impl AnyPage for HomePage {
    fn page(&self) -> &Page {
        &self.page
    }
    fn page_mut(&mut self) -> &mut Page {
        &mut self.page
    }
    fn full_shadow_promise(&self) -> bool {
        true
    }

    fn on_activate(&mut self) {}

    async fn build(&mut self) {
        self.page.root.layout_direction = Direction::Horizontal;

        let mut content = ComponentBase::new(0, 0, 0, 0);
        content.layout_direction = Direction::Vertical;
        content.h_constraint = Constraintable::Maximum;
        content.v_constraint = Constraintable::Minimum;

        let mut title = Label::new("MuseSP", 0, 0, 400, 120, 72, (255, 255, 255));
        title.base.v_constraint = Constraintable::Minimum;
        title.base.h_constraint = Constraintable::Minimum;
        title.base.min_height = 120;
        title.base.min_width = 400;
        content.children.push(title);

        let mut sp = Spacer::new(0, 30);
        sp.base.v_constraint = Constraintable::Minimum;
        sp.base.h_constraint = Constraintable::Maximum;
        sp.base.min_height = 30;
        content.children.push(sp);

        let should_exit = self.page.should_exit.clone().unwrap();
        let nav = self.page.nav.clone().unwrap();

        let mut btn_start = Button::new("开始", 0, 0, 200, 50, 24);
        btn_start.base.v_constraint = Constraintable::Minimum;
        btn_start.base.h_constraint = Constraintable::Maximum;
        btn_start.base.min_height = 50;
        btn_start.base.min_width = 200;
        let n = nav.clone();
        btn_start.base.bind_mouse_click(Box::new(move |_| {
            let n = n.clone();
            Box::pin(async move {
                println!("[click] 开始");
                let _ = n
                    .send(NavAction::Push(Box::new(MusicListPage::new())))
                    .await;
                false
            })
        }));
        content.children.push(btn_start);

        let mut sp2 = Spacer::new(0, 10);
        sp2.base.v_constraint = Constraintable::Minimum;
        sp2.base.min_height = 10;
        content.children.push(sp2);

        let mut btn_settings = Button::new("设置", 0, 0, 200, 50, 24);
        btn_settings.base.v_constraint = Constraintable::Minimum;
        btn_settings.base.h_constraint = Constraintable::Maximum;
        btn_settings.base.min_height = 50;
        btn_settings.base.min_width = 200;
        content.children.push(btn_settings);

        let mut sp3 = Spacer::new(0, 10);
        sp3.base.v_constraint = Constraintable::Minimum;
        sp3.base.min_height = 10;
        content.children.push(sp3);

        let exit_clone = should_exit.clone();
        let mut btn_exit = Button::new("退出", 0, 0, 200, 50, 24);
        btn_exit.base.v_constraint = Constraintable::Minimum;
        btn_exit.base.h_constraint = Constraintable::Maximum;
        btn_exit.base.min_height = 50;
        btn_exit.base.min_width = 200;
        btn_exit.base.bind_mouse_click(Box::new(move |_| {
            let exit_clone = exit_clone.clone();
            Box::pin(async move {
                exit_clone.store(true, Ordering::Relaxed);
                false
            })
        }));
        content.children.push(btn_exit);

        let mut spacer_left = Spacer::new(0, 0);
        spacer_left.base.h_constraint = Constraintable::Maximum;
        spacer_left.base.v_constraint = Constraintable::Minimum;
        self.page.root.children.push(spacer_left);
        self.page.root.children.push(Box::new(content));
        let mut spacer_right = Spacer::new(0, 0);
        spacer_right.base.h_constraint = Constraintable::Maximum;
        spacer_right.base.v_constraint = Constraintable::Minimum;
        self.page.root.children.push(spacer_right);
    }

    fn prepare_layout(&mut self) {
        let cap = self.page.root.width * 2 / 7;
        let children = &mut self.page.root.children;
        if children.len() >= 3 {
            children[0].base_mut().max_width = cap;
            children[2].base_mut().max_width = cap;
        }
    }
}
