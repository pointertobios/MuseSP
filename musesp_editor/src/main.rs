use musesp_ui::application::Application;
use musesp_ui::components::button::Button;
use musesp_ui::components::core::{Constraintable, Direction};
use musesp_ui::components::label::Label;
use musesp_ui::components::spacer::Spacer;
use musesp_ui::router::{AnyPage, NavAction, Page};

struct EditorPage {
    pub page: Page,
}
impl EditorPage {
    fn new() -> Self {
        EditorPage { page: Page::new() }
    }
}
#[async_trait::async_trait]
impl AnyPage for EditorPage {
    fn page(&self) -> &Page {
        &self.page
    }
    fn page_mut(&mut self) -> &mut Page {
        &mut self.page
    }
    fn full_shadow_promise(&self) -> bool {
        true
    }

    async fn build(&mut self) {
        self.page.root.layout_direction = Direction::Vertical;

        let mut st = Spacer::new(0, 0);
        st.base.v_constraint = Constraintable::Maximum;
        self.page.root.children.push(st);

        let mut title = Label::new("MuseSP Editor", 0, 0, 400, 80, 48, (255, 255, 255));
        title.base.v_constraint = Constraintable::Minimum;
        title.base.h_constraint = Constraintable::Maximum;
        title.base.min_height = 80;
        title.base.min_width = 400;
        self.page.root.children.push(title);

        let mut sp = Spacer::new(0, 20);
        sp.base.v_constraint = Constraintable::Minimum;
        sp.base.min_height = 20;
        self.page.root.children.push(sp);

        let mut back_btn = Button::new("返回", 0, 0, 200, 50, 24);
        back_btn.base.v_constraint = Constraintable::Minimum;
        back_btn.base.h_constraint = Constraintable::Maximum;
        back_btn.base.min_height = 50;
        back_btn.base.min_width = 200;
        let nav = self.page.nav.clone().unwrap();
        back_btn.base.bind_mouse_click(Box::new(move |_| {
            let _ = nav.blocking_send(NavAction::Pop);
            false
        }));
        self.page.root.children.push(back_btn);

        let mut sb = Spacer::new(0, 0);
        sb.base.v_constraint = Constraintable::Maximum;
        self.page.root.children.push(sb);
    }
}

fn main() {
    Application::run("MuseSP Editor", EditorPage::new());
}
