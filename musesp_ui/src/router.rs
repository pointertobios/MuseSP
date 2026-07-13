use std::any::Any;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use winit::event::WindowEvent;

use crate::components::core::ComponentBase;
use crate::renderer::UIRenderer;
use musesp_config::config::Config;

pub use crate::application::RunMode;

pub struct Page {
    pub root: ComponentBase,
    pub should_exit: Option<Arc<AtomicBool>>,
    pub nav: Option<mpsc::Sender<NavAction>>,
}

pub struct PageToken {
    pub value: Option<Arc<dyn Any + Send + Sync>>,
    pub resolved: bool,
}

impl PageToken {
    pub fn new() -> Self {
        PageToken {
            value: None,
            resolved: false,
        }
    }
    pub fn resolve(&mut self, value: Arc<dyn Any + Send + Sync>) {
        self.value = Some(value);
        self.resolved = true;
    }
}

impl Page {
    pub fn new() -> Self {
        Page {
            root: ComponentBase::new(0, 0, 0, 0),
            should_exit: None,
            nav: None,
        }
    }

    pub fn push_page<P: AnyPage + 'static>(&self, page: P) {
        let _ = self
            .nav
            .as_ref()
            .unwrap()
            .send(NavAction::Push(Box::new(page)));
    }

    pub fn pop_page(&self) {
        let _ = self.nav.as_ref().unwrap().send(NavAction::Pop);
    }

    pub fn exit(&self) {
        if let Some(exit) = &self.should_exit {
            exit.store(true, Ordering::Relaxed);
        }
    }

    pub fn build(&mut self) {}
    pub fn destroy(&mut self) {}
    pub fn on_hide(&mut self) {}
    pub fn on_activate(&mut self) {}
    pub fn full_shadow_promise(&self) -> bool {
        false
    }
    pub fn prepare_layout(&mut self) {}

    pub fn dispatch_event(&mut self, event: &WindowEvent) {
        self.root.dispatch_event(event);
    }

    pub fn draw(&self, renderer: &mut UIRenderer) {
        for child in &self.root.children {
            child.draw(renderer, 0, 0);
        }
    }

    pub fn draw_debug(&self, renderer: &mut UIRenderer, config: &Config) {
        for child in &self.root.children {
            child.draw_debug(renderer, config, 0, 0);
        }
    }
}

pub trait AnyPage: Any + Send {
    fn page(&self) -> &Page;
    fn page_mut(&mut self) -> &mut Page;

    fn build(&mut self) {}
    fn destroy(&mut self) {}
    fn on_hide(&mut self) {}
    fn on_activate(&mut self) {}
    fn full_shadow_promise(&self) -> bool {
        false
    }
    fn prepare_layout(&mut self) {}
    fn initial_mode(&self) -> RunMode {
        RunMode::Event
    }
    fn dispatch_event(&mut self, event: &WindowEvent) {
        self.page_mut().dispatch_event(event);
    }
    fn draw(&self, renderer: &mut UIRenderer) {
        self.page().draw(renderer);
    }
    fn draw_debug(&self, renderer: &mut UIRenderer, config: &Config) {
        self.page().draw_debug(renderer, config);
    }
}

pub struct Router {
    pub stack: Vec<(Box<dyn AnyPage>, PageToken)>,
    pub win_w: i32,
    pub win_h: i32,
    pub mode: Cell<RunMode>,
    pub target_fps: u32,
    pub should_exit: Arc<AtomicBool>,
    nav_sender: mpsc::Sender<NavAction>,
    nav_receiver: mpsc::Receiver<NavAction>,
}

pub enum NavAction {
    Push(Box<dyn AnyPage>),
    Pop,
    ClearAndPush(Box<dyn AnyPage>),
    PopThenElse(Box<dyn AnyPage>),
}

impl Router {
    pub fn new(win_w: i32, win_h: i32) -> Self {
        let (nav_sender, nav_receiver) = mpsc::channel();
        Router {
            stack: Vec::new(),
            win_w,
            win_h,
            mode: Cell::new(RunMode::Event),
            target_fps: 60,
            should_exit: Arc::new(AtomicBool::new(false)),
            nav_sender,
            nav_receiver,
        }
    }

    pub fn init_page(&mut self, page: &mut Box<dyn AnyPage>) {
        page.page_mut().nav = Some(self.nav_sender.clone());
        page.page_mut().should_exit = Some(self.should_exit.clone());
        page.build();
        page.page_mut().root.width = self.win_w;
        page.page_mut().root.height = self.win_h;
        page.prepare_layout();
        page.page_mut().root.layout(None);
        self.mode.set(page.initial_mode());
    }

    pub fn push<P: AnyPage + 'static>(&mut self, page: P) {
        if let Some((current, _)) = self.stack.last_mut() {
            current.on_hide();
            current.page_mut().root.force_mouse_exit();
        }
        let mut boxed: Box<dyn AnyPage> = Box::new(page);
        self.init_page(&mut boxed);
        self.stack.push((boxed, PageToken::new()));
    }

    pub fn pop(&mut self, value: Option<Arc<dyn Any + Send + Sync>>) {
        if self.stack.len() <= 1 {
            return;
        }
        let (mut page, mut token) = self.stack.pop().unwrap();
        if let Some(v) = value {
            token.resolve(v);
        }
        page.destroy();
        if let Some((current, _)) = self.stack.last_mut() {
            current.on_activate();
            self.mode.set(current.initial_mode());
        }
    }

    pub fn clear_and_push<P: AnyPage + 'static>(&mut self, page: P) {
        if let Some((current, _)) = self.stack.last_mut() {
            current.on_hide();
            current.page_mut().root.force_mouse_exit();
        }
        for (mut p, _) in self.stack.drain(..) {
            p.destroy();
        }
        let mut boxed: Box<dyn AnyPage> = Box::new(page);
        self.init_page(&mut boxed);
        self.stack.push((boxed, PageToken::new()));
    }

    pub fn pop_then_else<F: FnOnce() -> Box<dyn AnyPage>>(
        &mut self,
        fallback: F,
        value: Option<Arc<dyn Any + Send + Sync>>,
    ) {
        if self.stack.len() > 1 {
            self.pop(value);
            return;
        }
        let (mut page, mut token) = self.stack.pop().unwrap();
        page.on_hide();
        page.page_mut().root.force_mouse_exit();
        if let Some(v) = value {
            token.resolve(v);
        }
        page.destroy();
        self.stack.clear();
        let mut boxed = fallback();
        self.init_page(&mut boxed);
        self.stack.push((boxed, PageToken::new()));
    }

    pub fn pop_n_and_push<P: AnyPage + 'static>(
        &mut self,
        n: usize,
        page: P,
        value: Option<Arc<dyn Any + Send + Sync>>,
    ) {
        if n == 0 {
            self.push(page);
            return;
        }
        if n >= self.stack.len() {
            self.clear_and_push(page);
            return;
        }
        for _ in 0..n {
            self.pop(value.clone());
        }
        self.push(page);
    }

    pub fn dispatch_event(&mut self, event: &WindowEvent) {
        if let Some((page, _)) = self.stack.last_mut() {
            page.dispatch_event(event);
        }
        self.drain_nav_actions();
    }

    fn drain_nav_actions(&mut self) {
        while let Ok(action) = self.nav_receiver.try_recv() {
            match action {
                NavAction::Push(page) => {
                    if let Some((current, _)) = self.stack.last_mut() {
                        current.on_hide();
                        current.page_mut().root.force_mouse_exit();
                    }
                    let mut boxed = page;
                    self.init_page(&mut boxed);
                    self.stack.push((boxed, PageToken::new()));
                }
                NavAction::Pop => {
                    self.pop(None);
                }
                NavAction::ClearAndPush(page) => {
                    if let Some((current, _)) = self.stack.last_mut() {
                        current.on_hide();
                        current.page_mut().root.force_mouse_exit();
                    }
                    for (mut p, _) in self.stack.drain(..) {
                        p.destroy();
                    }
                    let mut boxed = page;
                    self.init_page(&mut boxed);
                    self.stack.push((boxed, PageToken::new()));
                }
                NavAction::PopThenElse(fallback) => {
                    if self.stack.len() > 1 {
                        self.pop(None);
                    } else {
                        if let Some((current, _)) = self.stack.last_mut() {
                            current.on_hide();
                            current.page_mut().root.force_mouse_exit();
                        }
                        for (mut p, _) in self.stack.drain(..) {
                            p.destroy();
                        }
                        let mut boxed = fallback;
                        self.init_page(&mut boxed);
                        self.stack.push((boxed, PageToken::new()));
                    }
                }
            }
        }
    }

    pub fn draw_pages(&self, renderer: &mut UIRenderer, config: &Config) {
        renderer.draw_rect(0, 0, self.win_w, self.win_h, (0, 0, 0, 255));
        let mut start: usize = 0;
        for i in (0..self.stack.len()).rev() {
            if self.stack[i].0.full_shadow_promise() {
                start = i;
                break;
            }
        }
        for i in start..self.stack.len() {
            let (page, _) = &self.stack[i];
            page.draw(renderer);
            page.draw_debug(renderer, config);
        }
    }
}
