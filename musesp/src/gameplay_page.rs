use std::cell::Cell;
use std::sync::Arc;

use musesp_ui::components::core::Constraintable;
use musesp_ui::components::image_button::ImageButton;
use musesp_ui::renderer::UIRenderer;
use musesp_ui::router::{AnyPage, NavAction, Page};

use crate::gameplay::renderer3d::{self, AsyncSnapshotProducer, CameraMode, GameplayRenderPipeline};
use crate::menu_page::MenuPage;

/// 十字指针大小（像素）
const CROSSHAIR_HALF: i32 = 12;
const CROSSHAIR_THICK: i32 = 2;

/// 摄像机移动速度（每帧，vsync ≈ 60fps）
const CAMERA_MOVE_SPEED: f32 = 0.15;
/// 摄像机轨道旋转灵敏度（弧度/像素）
const CAMERA_ORBIT_SENSITIVITY: f32 = 0.003;
/// FOV 滚轮调整速度
const CAMERA_FOV_SPEED: f32 = 2.0;

/// 摄像机移动按键 bitmask
const KEY_W: u8 = 1 << 0;
const KEY_S: u8 = 1 << 1;
const KEY_A: u8 = 1 << 2;
const KEY_D: u8 = 1 << 3;
const KEY_SPACE: u8 = 1 << 4;
const KEY_LALT: u8 = 1 << 5;

pub struct GameplayPage {
    pub page: Page,
    /// 保持后台任务存活
    _producer: Option<Arc<AsyncSnapshotProducer>>,
    /// 玩法状态句柄
    gs: Option<Arc<std::sync::Mutex<renderer3d::GameplayState>>>,
    /// 十字指针位置（屏幕像素坐标，鼠标移动时更新）
    crosshair_pos: Option<(i32, i32)>,
    /// Ctrl 键是否按下（用于检测 Ctrl+F1）
    ctrl_pressed: bool,
    /// 上一帧鼠标位置（用于计算轨道旋转增量）
    last_mouse_pos: Option<(f32, f32)>,
    /// 是否允许摄像机调整（从配置读取）
    camera_adjustable: bool,
    /// 当前按下的移动键 bitmask（W/A/S/D/Space/LAlt）
    held_keys: u8,
    /// 摄像机是否有未保存的修改
    camera_dirty: Cell<bool>,
}

impl GameplayPage {
    pub fn new() -> Self {
        GameplayPage {
            page: Page::new(),
            _producer: None,
            gs: None,
            crosshair_pos: None,
            ctrl_pressed: false,
            last_mouse_pos: None,
            camera_adjustable: true,
            held_keys: 0,
            camera_dirty: Cell::new(false),
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

    fn on_activate(&mut self) {
        // 进入 gameplay 时隐藏鼠标光标
        if let Some(window) = &self.page.window {
            window.set_cursor_visible(false);
        }
    }

    fn on_hide(&mut self) {
        // 离开 gameplay（被其他页面覆盖）时恢复鼠标光标
        if let Some(window) = &self.page.window {
            window.set_cursor_visible(true);
        }
    }

    fn destroy(&mut self, is_async: bool) {
        // 如果启用了摄像机调整且有未保存修改，保存到配置文件
        if self.camera_adjustable && self.camera_dirty.get() {
            if let Some(ref gs) = self.gs {
                let state = gs.lock().unwrap();
                let default = musesp_config::config::CameraConfig::default();
                let cam_config = musesp_config::config::CameraConfig {
                    eye: state.camera.eye,
                    direction: state.camera.direction,
                    up: state.camera.up,
                    fov_degrees: state.camera.fov_degrees,
                    near: default.near,
                    far: default.far,
                };
                drop(state);
                if is_async {
                    tokio::spawn(async move {
                        musesp_config::config::save_camera_config(&cam_config).await;
                    });
                } else {
                    musesp_config::config::save_camera_config_sync(&cam_config);
                }
            }
            self.camera_dirty.set(false);
        }

        // 恢复鼠标光标
        if let Some(window) = &self.page.window {
            window.set_cursor_visible(true);
        }

        // 清理 AsyncSnapshotProducer（停止后台任务）
        self._producer = None;
    }

    fn draw(&self, renderer: &mut UIRenderer, screen_w: u32, screen_h: u32) {
        // 先绘制 UI 组件树
        self.page.draw(renderer, screen_w, screen_h);

        // 摄像机调整模式：每帧根据按下的键持续移动
        if self.held_keys != 0 {
            if let Some(ref gs) = self.gs {
                let mut state = gs.lock().unwrap();
                if state.camera_mode == CameraMode::Adjusting {
                    let fwd = state.camera.forward();
                    let right = state.camera.right();
                    let mut moved = false;

                    if self.held_keys & KEY_W != 0 {
                        state.camera.translate([
                            fwd[0] * CAMERA_MOVE_SPEED,
                            0.0,
                            fwd[2] * CAMERA_MOVE_SPEED,
                        ]);
                        moved = true;
                    }
                    if self.held_keys & KEY_S != 0 {
                        state.camera.translate([
                            -fwd[0] * CAMERA_MOVE_SPEED,
                            0.0,
                            -fwd[2] * CAMERA_MOVE_SPEED,
                        ]);
                        moved = true;
                    }
                    if self.held_keys & KEY_A != 0 {
                        state.camera.translate([
                            -right[0] * CAMERA_MOVE_SPEED,
                            0.0,
                            -right[2] * CAMERA_MOVE_SPEED,
                        ]);
                        moved = true;
                    }
                    if self.held_keys & KEY_D != 0 {
                        state.camera.translate([
                            right[0] * CAMERA_MOVE_SPEED,
                            0.0,
                            right[2] * CAMERA_MOVE_SPEED,
                        ]);
                        moved = true;
                    }
                    if self.held_keys & KEY_SPACE != 0 {
                        state.camera.translate([0.0, CAMERA_MOVE_SPEED, 0.0]);
                        moved = true;
                    }
                    if self.held_keys & KEY_LALT != 0 {
                        state.camera.translate([0.0, -CAMERA_MOVE_SPEED, 0.0]);
                        moved = true;
                    }
                    if moved {
                        self.camera_dirty.set(true);
                    }
                }
            }
        }

        // 在 UI 之上绘制十字指针
        if let Some((cx, cy)) = self.crosshair_pos {
            renderer.draw_rect(
                cx - CROSSHAIR_HALF,
                cy - CROSSHAIR_THICK / 2,
                CROSSHAIR_HALF * 2,
                CROSSHAIR_THICK,
                (0, 255, 128, 220),
            );
            renderer.draw_rect(
                cx - CROSSHAIR_THICK / 2,
                cy - CROSSHAIR_HALF,
                CROSSHAIR_THICK,
                CROSSHAIR_HALF * 2,
                (0, 255, 128, 220),
            );
        }
    }

    async fn build(&mut self) {
        // 加载配置（用于摄像机参数和调试开关）
        let config = musesp_config::config::load_config().await;
        self.camera_adjustable = config.debug.gameplay.camera_adjustable;

        // 初始化玩法全局状态
        let gs = renderer3d::init_gameplay_state();
        {
            let mut state = gs.lock().unwrap();
            state.screen_size = (
                self.page.root.width as f32,
                self.page.root.height as f32,
            );
            // 应用配置文件中的摄像机参数
            state.apply_camera_config(&config.gameplay.camera);
        }
        self.gs = Some(gs);

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

        // 查询当前模式
        let mode = self
            .gs
            .as_ref()
            .map(|gs| gs.lock().unwrap().camera_mode)
            .unwrap_or(CameraMode::Playing);

        match event {
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let pos = (position.x as f32, position.y as f32);

                if mode == CameraMode::Adjusting {
                    // 调整模式：鼠标移动用于轨道旋转
                    if let Some(last) = self.last_mouse_pos {
                        let dx = pos.0 - last.0;
                        let dy = pos.1 - last.1;

                        if let Some(ref gs) = self.gs {
                            let mut state = gs.lock().unwrap();
                            state.camera.rotate_view(
                                -dx * CAMERA_ORBIT_SENSITIVITY,
                                -dy * CAMERA_ORBIT_SENSITIVITY,
                            );
                            self.camera_dirty.set(true);
                        }
                    }
                    self.last_mouse_pos = Some(pos);
                } else {
                    // 正常游戏：更新鼠标位置用于拾取
                    if let Some(ref gs) = self.gs {
                        let mut state = gs.lock().unwrap();
                        state.mouse_screen = Some(pos);
                    }
                }
                // 始终更新十字指针
                self.crosshair_pos = Some((pos.0 as i32, pos.1 as i32));
            }
            winit::event::WindowEvent::ModifiersChanged(modifiers) => {
                self.ctrl_pressed = modifiers.state().control_key();
            }
            winit::event::WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                let pressed = key_event.state == ElementState::Pressed;

                // Ctrl 键跟踪
                if key_event.physical_key == PhysicalKey::Code(KeyCode::ControlLeft)
                    || key_event.physical_key == PhysicalKey::Code(KeyCode::ControlRight)
                {
                    self.ctrl_pressed = pressed;
                    return;
                }

                // Ctrl+1：切换摄像机模式
                if pressed
                    && self.ctrl_pressed
                    && key_event.physical_key == PhysicalKey::Code(KeyCode::Digit1)
                    && self.camera_adjustable
                {
                    self.toggle_camera_mode();
                    return;
                }

                // ── 正常游戏模式按键 ──
                if mode == CameraMode::Playing {
                    if pressed
                        && key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                    {
                        if let Some(btn) = self.page.root.find_by_name_mut("menu_btn") {
                            btn.emit("mouse_click", None).await;
                        }
                        return;
                    }
                    self.page.dispatch_event(event).await;
                    return;
                }

                // ── 摄像机调整模式按键 ──
                if mode == CameraMode::Adjusting {
                    // Esc 也退回 Playing 模式
                    if pressed
                        && key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                    {
                        self.toggle_camera_mode();
                        return;
                    }

                    // 记录移动键按下/释放状态（每帧在 draw() 中持续移动）
                    let bit = match key_event.physical_key {
                        PhysicalKey::Code(KeyCode::KeyW) => KEY_W,
                        PhysicalKey::Code(KeyCode::KeyS) => KEY_S,
                        PhysicalKey::Code(KeyCode::KeyA) => KEY_A,
                        PhysicalKey::Code(KeyCode::KeyD) => KEY_D,
                        PhysicalKey::Code(KeyCode::Space) => KEY_SPACE,
                        PhysicalKey::Code(KeyCode::AltLeft) => KEY_LALT,
                        _ => 0,
                    };
                    if bit != 0 {
                        if pressed {
                            self.held_keys |= bit;
                        } else {
                            self.held_keys &= !bit;
                        }
                    }
                    return;
                }
            }
            winit::event::WindowEvent::MouseWheel {
                delta: winit::event::MouseScrollDelta::LineDelta(_x, y),
                ..
            } => {
                if mode == CameraMode::Adjusting {
                    if let Some(ref gs) = self.gs {
                        let mut state = gs.lock().unwrap();
                        state.camera.adjust_fov(-(*y) * CAMERA_FOV_SPEED);
                        self.camera_dirty.set(true);
                    }
                }
            }
            _ => {}
        }

        // 在 Playing 模式下，将事件传递给 UI 组件树
        if mode == CameraMode::Playing {
            self.page.dispatch_event(event).await;
        }
    }
}

impl GameplayPage {
    /// 切换摄像机模式（光标始终保持隐藏）
    fn toggle_camera_mode(&mut self) {
        if let Some(ref gs) = self.gs {
            let mut state = gs.lock().unwrap();
            state.camera_mode = match state.camera_mode {
                CameraMode::Playing => CameraMode::Adjusting,
                CameraMode::Adjusting => CameraMode::Playing,
            };

            if state.camera_mode == CameraMode::Playing {
                // 切回 Playing 模式时重置状态
                self.last_mouse_pos = None;
            }
            // 模式切换时清除所有按键状态
            self.held_keys = 0;
        }
    }
}
