use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use musesp_ui::components::button::Button;
use musesp_ui::components::core::{ComponentBase, ComponentTrait, Constraintable, Direction};
use musesp_ui::components::image::{Image, ImageMode};
use musesp_ui::components::image_button::ImageButton;
use musesp_ui::components::scroll_list::ScrollList;
use musesp_ui::components::spacer::Spacer;
use musesp_ui::renderer::UIRenderer;
use musesp_ui::router::{AnyPage, NavAction, Page};

use crate::gameplay_page::GameplayPage;
use crate::pages::home::HomePage;

pub struct MusicListPage {
    pub page: Page,
    inner: Arc<Mutex<MusicListInner>>,
}

struct MusicListInner {
    music_sources: HashMap<String, PathBuf>,
    /// 共享选中状态，每个 MusicListItem 持有 clone 用于 draw_self 判断
    selected_item_id: Arc<Mutex<Option<String>>>,
    selected_level: Option<i32>,
    level_btn_names: Vec<String>,
    /// 缓存当前选中音乐的难度列表，用于重建按钮
    cached_levels: Vec<(String, i32)>,
    /// dispatch_event 后处理：待处理的选中 item_id
    pending_select: Option<String>,
    /// dispatch_event 后处理：待处理的难度选择
    pending_level_action: Option<i32>,
}

/// 音乐列表项 —— 支持悬停/选中视觉变化
struct MusicListItem {
    base: ComponentBase,
    name: String,
    author: String,
    /// 共享的选中 item_id，draw_self 时读取
    selected_item_id: Arc<Mutex<Option<String>>>,
}

impl MusicListItem {
    fn new(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        item_id: String,
        name: String,
        author: String,
        selected_item_id: Arc<Mutex<Option<String>>>,
    ) -> Box<Self> {
        let mut base = ComponentBase::new(x, y, width, height);
        base.item_id = Some(item_id);
        Box::new(MusicListItem {
            base,
            name,
            author,
            selected_item_id,
        })
    }
}

impl ComponentTrait for MusicListItem {
    fn base(&self) -> &ComponentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn draw_self(&self, renderer: &mut UIRenderer, dx: i32, dy: i32) {
        let is_selected =
            self.selected_item_id.blocking_lock().as_deref() == self.base.item_id.as_deref();

        let (name_size, name_color, author_color): (u32, (u8, u8, u8), (u8, u8, u8)) =
            if is_selected {
                // Python: selected → name 22px yellow, author 220,220,220
                (22, (255, 255, 100), (220, 220, 220))
            } else if self.base.hovered {
                // Python: hovered → name 22px 255,255,160, author 200,200,200
                (22, (255, 255, 160), (200, 200, 200))
            } else {
                // Python: normal → name 20px 255,255,255, author 160,160,160
                (20, (255, 255, 255), (160, 160, 160))
            };

        // Name label: y=4, height=24
        renderer.draw_text(
            &self.name,
            dx,
            dy + 4,
            self.base.width,
            24,
            name_size,
            name_color,
        );
        // Author label: y=28, height=20
        renderer.draw_text(
            &self.author,
            dx,
            dy + 28,
            self.base.width,
            20,
            14,
            author_color,
        );
    }
}

impl MusicListPage {
    pub fn new() -> Self {
        let selected_item_id = Arc::new(Mutex::new(None));
        MusicListPage {
            page: Page::new(),
            inner: Arc::new(Mutex::new(MusicListInner {
                music_sources: HashMap::new(),
                selected_item_id: selected_item_id.clone(),
                selected_level: None,
                level_btn_names: Vec::new(),
                cached_levels: Vec::new(),
                pending_select: None,
                pending_level_action: None,
            })),
        }
    }
}

#[async_trait::async_trait]
impl AnyPage for MusicListPage {
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

        let nav = self.page.nav.clone().unwrap();

        let mut back_btn =
            ImageButton::new("assets/ui/return_button.svg", "", 16, 16, 44, 44, 18).await;
        back_btn.base.h_constraint = Constraintable::None;
        back_btn.base.v_constraint = Constraintable::None;
        let n = nav.clone();
        back_btn.base.bind_mouse_click(Box::new(move |_| {
            let n = n.clone();
            Box::pin(async move {
                let _ = n
                    .send(NavAction::PopThenElse(Box::new(HomePage::new())))
                    .await;
                false
            })
        }));
        self.page.root.children.push(back_btn);

        let mut content = ComponentBase::new(0, 0, 0, 0);
        content.layout_direction = Direction::Horizontal;
        content.h_constraint = Constraintable::Maximum;
        content.v_constraint = Constraintable::Minimum;

        let mut left = ComponentBase::new(0, 0, 320, 0);
        left.layout_direction = Direction::Vertical;
        left.h_constraint = Constraintable::Minimum;
        left.v_constraint = Constraintable::Minimum;
        left.min_width = 320;

        let mut scroll = ScrollList::new(0, 0, 280, 0, 52);
        scroll.base.name = Some("scroll_list".into());
        scroll.base.v_constraint = Constraintable::Maximum;
        scroll.base.h_constraint = Constraintable::Minimum;
        scroll.base.min_width = 280;
        let inner_select = self.inner.clone();
        let handler: musesp_ui::components::scroll_list::SelectHandler =
            Box::new(move |item_id: &str| {
                let item_id = item_id.to_string();
                let inner = inner_select.clone();
                Box::pin(async move {
                    inner.lock().await.pending_select = Some(item_id);
                })
            });
        scroll.bind_on_select(handler);
        left.children.push(scroll);
        content.children.push(Box::new(left));

        let mut sep = Spacer::new(2, 0);
        sep.base.h_constraint = Constraintable::Minimum;
        sep.base.v_constraint = Constraintable::Minimum;
        sep.base.min_width = 2;
        content.children.push(sep);

        let mut detail = ComponentBase::new(0, 0, 0, 0);
        detail.layout_direction = Direction::Vertical;
        detail.h_constraint = Constraintable::Maximum;
        detail.v_constraint = Constraintable::Minimum;
        detail.name = Some("detail".into());

        let mut st = Spacer::new(0, 0);
        st.base.name = Some("spacer_top".into());
        st.base.v_constraint = Constraintable::Maximum;
        detail.children.push(st);

        let mut cover = Image::new("", 0, 0, 200, 0, ImageMode::KeepRate, ImageMode::Cover).await;
        cover.base.name = Some("cover".into());
        cover.base.v_constraint = Constraintable::Minimum;
        cover.base.h_constraint = Constraintable::Minimum;
        cover.base.min_width = 200;
        detail.children.push(cover);

        let mut g1 = Spacer::new(0, 8);
        g1.base.v_constraint = Constraintable::Minimum;
        g1.base.min_height = 8;
        detail.children.push(g1);

        let mut diff_row = ComponentBase::new(0, 0, 0, 44);
        diff_row.layout_direction = Direction::Horizontal;
        diff_row.v_constraint = Constraintable::Minimum;
        diff_row.h_constraint = Constraintable::Minimum;
        diff_row.min_height = 44;
        diff_row.name = Some("diff_row".into());
        // Python: diff_row 初始为空，难度按钮在 _on_music_select 中动态创建

        detail.children.push(Box::new(diff_row));

        let mut g2 = Spacer::new(0, 8);
        g2.base.v_constraint = Constraintable::Minimum;
        g2.base.min_height = 8;
        detail.children.push(g2);

        let mut play = Button::new("\u{25B6} Play", 0, 0, 200, 44, 24);
        play.base.name = Some("btn_play".into());
        play.base.v_constraint = Constraintable::Minimum;
        play.base.h_constraint = Constraintable::Maximum;
        play.base.min_height = 44;
        play.base.min_width = 200;
        let inner = self.inner.clone();
        let nav = self.page.nav.clone().unwrap();
        play.base.bind_mouse_click(Box::new(move |_| {
            let inner = inner.clone();
            let nav = nav.clone();
            Box::pin(async move {
                if inner.lock().await.selected_level.is_none() {
                    return false;
                }
                let _ = nav
                    .send(NavAction::ClearAndPush(Box::new(GameplayPage::new())))
                    .await;
                false
            })
        }));
        detail.children.push(play);

        let mut sb = Spacer::new(0, 0);
        sb.base.v_constraint = Constraintable::Maximum;
        detail.children.push(sb);
        content.children.push(Box::new(detail));

        let mut sl = Spacer::new(0, 0);
        sl.base.name = Some("spacer_left".into());
        sl.base.h_constraint = Constraintable::Maximum;
        sl.base.v_constraint = Constraintable::Minimum;
        self.page.root.children.push(sl);
        self.page.root.children.push(Box::new(content));
        let mut sr = Spacer::new(0, 0);
        sr.base.name = Some("spacer_right".into());
        sr.base.h_constraint = Constraintable::Maximum;
        sr.base.v_constraint = Constraintable::Minimum;
        self.page.root.children.push(sr);

        self.load_list().await;
    }

    fn prepare_layout(&mut self) {
        let cap = self.page.root.width * 2 / 11;
        if let Some(sl) = self.page.root.find_by_name_mut("spacer_left") {
            sl.max_width = cap;
        }
        if let Some(sr) = self.page.root.find_by_name_mut("spacer_right") {
            sr.max_width = cap;
        }
        let detail_h = self
            .page
            .root
            .find_by_name("detail")
            .map(|d| d.height)
            .unwrap_or(0);
        let diff_row_min_h = self
            .page
            .root
            .find_by_name("diff_row")
            .map(|d| d.min_height)
            .unwrap_or(44);
        let btn_play_min_h = self
            .page
            .root
            .find_by_name("btn_play")
            .map(|d| d.min_height)
            .unwrap_or(44);
        let fixed = 8 + diff_row_min_h + 8 + btn_play_min_h;
        let available = 0.max(detail_h - fixed);
        let cover_min_h = 300.max(available * 3 / 2);
        if let Some(cover) = self.page.root.find_by_name_mut("cover") {
            cover.min_height = cover_min_h;
        }
    }

    async fn dispatch_event(&mut self, event: &winit::event::WindowEvent) {
        // 先分发事件到组件树（按钮回调在其中同步执行）
        self.page.dispatch_event(event).await;

        // 后处理：检查是否有待处理的选中/难度切换
        let pending_select = {
            let mut inner = self.inner.lock().await;
            inner.pending_select.take()
        };
        if let Some(item_id) = pending_select {
            self.on_music_select(&item_id).await;
        }

        let pending_level = {
            let mut inner = self.inner.lock().await;
            inner.pending_level_action.take()
        };
        if let Some(level) = pending_level {
            self.inner.lock().await.selected_level = Some(level);
            self.rebuild_level_buttons().await;
        }
    }
}

impl MusicListPage {
    async fn load_list(&mut self) {
        let config = musesp_config::config::load_config().await;
        let mut comps: Vec<Box<dyn ComponentTrait>> = Vec::new();
        for path_str in &config.gameplay.music_assets_path {
            let base = PathBuf::from(path_str);
            let list_file = self.resolve_list_file(&base).await;
            let Some(list_file) = list_file else { continue };
            let content = match tokio::fs::read_to_string(&list_file).await {
                Ok(c) => c,
                Err(_) => continue,
            };
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                if parts.len() < 3 {
                    continue;
                }
                let subdir = parts[0];
                let name = parts[1];
                let author = parts[2];
                let item_id = format!("{}/{}", path_str, subdir);
                self.inner
                    .lock()
                    .await
                    .music_sources
                    .insert(item_id.clone(), base.join(subdir));

                let selected_id = { self.inner.lock().await.selected_item_id.clone() };
                let item = MusicListItem::new(
                    0,
                    0,
                    280,
                    52,
                    item_id,
                    name.to_string(),
                    author.to_string(),
                    selected_id,
                );
                comps.push(item);
            }
        }
        if let Some(scroll) = self.page.root.find_component_by_name_mut("scroll_list") {
            scroll.set_scroll_items(comps);
        }
    }

    async fn resolve_list_file(&mut self, base: &PathBuf) -> Option<PathBuf> {
        if tokio::fs::metadata(base)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            let f = base.join("list.txt");
            if tokio::fs::metadata(&f).await.is_ok() {
                return Some(f);
            }
        }
        None
    }

    async fn on_music_select(&mut self, item_id: &str) {
        let src = {
            let inner = self.inner.lock().await;
            match inner.music_sources.get(item_id) {
                Some(s) => s.clone(),
                None => return,
            }
        };
        if !tokio::fs::metadata(&src)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            return;
        }
        let meta = match self.load_meta(&src).await {
            Some(m) => m,
            None => return,
        };

        // Cache levels data for later button rebuilds
        let levels_data: Vec<(String, i32)> = meta
            .get("music")
            .and_then(|v| v.get("levels"))
            .and_then(|v| v.as_table())
            .map(|levels| {
                let mut pairs: Vec<(String, i32)> = levels
                    .keys()
                    .map(|k| (k.clone(), k.parse::<i32>().unwrap_or(0)))
                    .collect();
                pairs.sort_by_key(|(_, lv)| *lv);
                pairs
            })
            .unwrap_or_default();

        {
            let mut inner = self.inner.lock().await;
            *inner.selected_item_id.lock().await = Some(item_id.to_string());
            inner.selected_level = None;
            inner.cached_levels = levels_data;
        }

        // Update cover image
        let cover_name = meta
            .get("music")
            .and_then(|v| v.get("cover"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !cover_name.is_empty() {
            let path_str = src.join(&cover_name).to_string_lossy().to_string();
            if let Some(comp) = self.page.root.find_component_by_name_mut("cover") {
                comp.set_image_path(&path_str).await;
            }
        }

        self.rebuild_level_buttons().await;
    }

    async fn rebuild_level_buttons(&mut self) {
        let (levels_data, selected_level) = {
            let inner = self.inner.lock().await;
            (inner.cached_levels.clone(), inner.selected_level)
        };

        if let Some(diff_row) = self.page.root.find_by_name_mut("diff_row") {
            diff_row.children.clear();
        }
        self.inner.lock().await.level_btn_names.clear();

        for (i, (lv_str, lv_num)) in levels_data.iter().enumerate() {
            let btn_name = format!("lv_btn_{}", lv_str);
            let mut btn = Button::new(&format!("Lv.{}", lv_str), 0, 0, 80, 36, 16);
            btn.base.name = Some(btn_name.clone());
            btn.base.h_constraint = Constraintable::Maximum;
            btn.base.v_constraint = Constraintable::Minimum;
            btn.base.min_height = 36;
            btn.base.min_width = 70;

            // Set initial enable/disable state
            if selected_level == Some(*lv_num) {
                btn.disable().await;
            }

            // Click handler: just set pending flag, dispatch_event will handle the rebuild
            let inner = self.inner.clone();
            let level = *lv_num;
            btn.base.bind_mouse_click(Box::new(move |_| {
                let inner = inner.clone();
                Box::pin(async move {
                    inner.lock().await.pending_level_action = Some(level);
                    false
                })
            }));

            self.inner.lock().await.level_btn_names.push(btn_name);
            if let Some(diff_row) = self.page.root.find_by_name_mut("diff_row") {
                diff_row.children.push(btn);
            }
            if i < levels_data.len() - 1 {
                let mut g = Spacer::new(4, 0);
                g.base.h_constraint = Constraintable::Minimum;
                g.base.min_width = 4;
                if let Some(diff_row) = self.page.root.find_by_name_mut("diff_row") {
                    diff_row.children.push(g);
                }
            }
        }

        self.page.root.layout(None);
    }

    async fn load_meta(&mut self, src: &PathBuf) -> Option<toml::Table> {
        let meta_file = src.join("meta.toml");
        if tokio::fs::metadata(&meta_file).await.is_err() {
            return None;
        }
        let content = tokio::fs::read_to_string(&meta_file).await.ok()?;
        toml::from_str(&content).ok()
    }
}
