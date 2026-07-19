use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::renderer::{FrameDrawList, FramePipeline, UIRenderer, WgpuRenderer};
use crate::router::{AnyPage, PageToken, Router};
use musesp_config::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Event,
    Fps,
    Vsync,
}

pub struct Application {
    name: String,
    router: Option<Arc<RefCell<Router>>>,
    config: Config,
    window: Option<Arc<Window>>,
    wgpu: Option<WgpuRenderer>,
    renderer: UIRenderer,
    last_frame: Instant,
    font_system: glyphon::FontSystem,
    swash_cache: glyphon::SwashCache,
    glyphon_cache: Option<glyphon::Cache>,
    text_atlas: Option<glyphon::TextAtlas>,
    text_renderer: Option<glyphon::TextRenderer>,
    text_buffers: Vec<glyphon::Buffer>,
    initial_page: Option<Box<dyn AnyPage>>,
    should_exit: Arc<AtomicBool>,
    rt_handle: tokio::runtime::Handle,
    /// 异步帧管线：协调后台帧准备与主线程 GPU 提交
    frame_pipeline: Option<FramePipeline>,
    /// 当前帧的绘制数据（从 UIRenderer 提取，可跨线程传递）
    current_draw_list: FrameDrawList,
}

impl Application {
    pub fn run<P: AnyPage + 'static>(name: &str, page: P) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut app = Application {
            name: name.to_string(),
            router: None,
            config: rt.block_on(musesp_config::config::load_config()),
            window: None,
            wgpu: None,
            renderer: UIRenderer::new(),
            last_frame: Instant::now(),
            font_system: glyphon::FontSystem::new(),
            swash_cache: glyphon::SwashCache::new(),
            glyphon_cache: None,
            text_atlas: None,
            text_renderer: None,
            text_buffers: Vec::new(),
            initial_page: Some(Box::new(page)),
            should_exit: Arc::new(AtomicBool::new(false)),
            rt_handle: rt.handle().clone(),
            frame_pipeline: None,
            current_draw_list: FrameDrawList::new(),
        };
        let event_loop = winit::event_loop::EventLoop::new().unwrap();
        event_loop.run_app(&mut app).unwrap();
    }

    fn mode(&self) -> RunMode {
        self.router
            .as_ref()
            .map_or(RunMode::Event, |r| r.borrow().mode.get())
    }

    fn init_router(&mut self, win_w: i32, win_h: i32) {
        if self.router.is_some() {
            return;
        }
        let router = Router::new(win_w, win_h);
        self.should_exit = router.should_exit.clone();
        let router_rc = Arc::new(RefCell::new(router));
        {
            let mut router_ref = router_rc.borrow_mut();
            if let Some(mut page) = self.initial_page.take() {
                self.rt_handle.block_on(router_ref.init_page(&mut page));
                router_ref.stack.push((page, PageToken::new()));
            }
        }
        self.router = Some(router_rc);
    }

    fn render(&mut self) {
        let wgpu = self.wgpu.as_mut().unwrap();
        let atlas = self.text_atlas.as_mut().unwrap();
        let text_renderer = self.text_renderer.as_mut().unwrap();

        // 1. 收集绘制命令（主线程，遍历组件树）
        self.renderer.clear();
        if let Some(ref router) = self.router {
            router.borrow().draw_pages(&mut self.renderer, &self.config);
        }

        // 2. 提取为 FrameDrawList（Send-able，为异步准备管线打基础）
        self.current_draw_list = FrameDrawList::from_renderer(&mut self.renderer);
        let dl = &self.current_draw_list;

        // 3. 获取 surface texture
        let output = match wgpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                wgpu.surface.configure(&wgpu.device, &wgpu.config);
                return;
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = wgpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // 4. glyphon 文本排版
        let mut viewport =
            glyphon::Viewport::new(&wgpu.device, self.glyphon_cache.as_ref().unwrap());
        viewport.update(
            &wgpu.queue,
            glyphon::Resolution {
                width: wgpu.config.width,
                height: wgpu.config.height,
            },
        );

        // 尝试获取后台预塑形的文本缓冲区
        let ready = self.frame_pipeline.as_ref().and_then(|p| p.try_get_ready());
        let shaped_buffers = ready.as_ref().and_then(|r| r.shaped_text_buffers.as_ref());

        self.text_buffers.clear();
        if let Some(buffers) = shaped_buffers {
            // 使用后台预塑形结果（零 CPU 开销）
            self.text_buffers = buffers.clone();
        } else {
            // 回退：同步塑形（首帧或管线尚未就绪时）
            for t in &dl.texts {
                let mut buffer = glyphon::Buffer::new(
                    &mut self.font_system,
                    glyphon::Metrics::new(t.font_size as f32, (t.font_size as f32) * 1.2),
                );
                buffer.set_size(Some(t.w as f32), Some(t.h as f32));
                buffer.set_text(
                    &t.text,
                    &glyphon::Attrs::new()
                        .color(glyphon::Color::rgb(t.color.0, t.color.1, t.color.2)),
                    glyphon::Shaping::Advanced,
                    Some(glyphon::cosmic_text::Align::Center),
                );
                buffer.shape_until_scroll(&mut self.font_system, false);
                self.text_buffers.push(buffer);
            }
        }

        // 发送文本数据给后台管线，为下一帧预塑形
        if let Some(ref pipeline) = self.frame_pipeline {
            pipeline.request_prepare(crate::renderer::FramePrepData {
                screen_w: wgpu.config.width,
                screen_h: wgpu.config.height,
                texts: dl.texts.clone(),
            });
        }

        let text_areas: Vec<glyphon::TextArea> = dl
            .texts
            .iter()
            .zip(self.text_buffers.iter())
            .map(|(t, buffer)| {
                let line_h = t.font_size as f32 * 1.2;
                let v_off = ((t.h as f32 - line_h) / 2.0).max(0.0);
                let bounds = glyphon::TextBounds {
                    left: t.x,
                    top: t.y,
                    right: t.x + t.w,
                    bottom: t.y + t.h,
                };
                glyphon::TextArea {
                    buffer,
                    left: t.x as f32,
                    top: t.y as f32 + v_off,
                    scale: 1.0,
                    bounds,
                    default_color: glyphon::Color::rgb(t.color.0, t.color.1, t.color.2),
                    custom_glyphs: &[],
                }
            })
            .collect();

        if let Err(e) = text_renderer.prepare(
            &wgpu.device,
            &wgpu.queue,
            &mut self.font_system,
            atlas,
            &viewport,
            text_areas,
            &mut self.swash_cache,
        ) {
            eprintln!("glyphon prepare error: {:?}", e);
            return;
        }

        // 5. 更新相机
        let elapsed = wgpu.start_time.elapsed().as_secs_f32();
        wgpu.update_camera(elapsed);

        // 6. 自定义 compute passes（由 gameplay 层通过 RenderPipeline trait 注入）
        {
            let router = self.router.as_ref().unwrap();
            let mut router_ref = router.borrow_mut();
            if let Some(pipeline) = router_ref.get_render_pipeline_mut() {
                pipeline.record_compute(
                    &wgpu.device,
                    &wgpu.queue,
                    &mut encoder,
                    &wgpu.config,
                    wgpu.sample_count,
                );
            }
        }

        // 7. 渲染通道（渲染到 MSAA 目标，自动 resolve 到 surface）
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &wgpu.msaa_color_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.8,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &wgpu.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // UI 元素（rect、image、custom canvas）
            wgpu.draw_rects(&mut rp, &dl.rects);
            wgpu.draw_images(&mut rp, &dl.images);
            wgpu.draw_custom(&mut rp, &dl.custom_draws);

            // 自定义渲染管线（gameplay 层在 UI 之后绘制，可写深度）
            {
                let router = self.router.as_ref().unwrap();
                let router_ref = router.borrow();
                if let Some(pipeline) = router_ref.get_render_pipeline() {
                    pipeline.record_render(
                        &wgpu.device,
                        &wgpu.queue,
                        &mut rp,
                        &wgpu.config,
                        wgpu.sample_count,
                    );
                }
            }

            if let Err(e) = text_renderer.render(atlas, &viewport, &mut rp) {
                eprintln!("glyphon render error: {:?}", e);
            }
        }

        // 8. 提交 + 呈现
        wgpu.queue.submit(Some(encoder.finish()));
        wgpu.queue.present(output);
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let wa = Window::default_attributes()
            .with_title(&self.name)
            .with_inner_size(winit::dpi::LogicalSize::new(1920, 1080))
            .with_resizable(false)
            .with_maximized(false);
        let window = Arc::new(event_loop.create_window(wa).unwrap());
        let size = window.inner_size();

        self.init_router(size.width as i32, size.height as i32);

        // 将窗口句柄注入 Router，以便页面可以控制系统光标
        if let Some(ref router) = self.router {
            router.borrow_mut().set_window(window.clone());
        }

        let wgpu = self.rt_handle.block_on(WgpuRenderer::new(window.clone()));

        // ShaderLibrary 已由 WgpuRenderer 在初始化时一并创建，此处共享给 Router
        let shader_library = wgpu.shader_library.clone();
        if let Some(router) = &self.router {
            router
                .borrow_mut()
                .set_shader_library(Some(shader_library));
        }

        let format = wgpu.config.format;

        let cache = glyphon::Cache::new(&wgpu.device);
        let mut atlas = glyphon::TextAtlas::new(&wgpu.device, &wgpu.queue, &cache, format);
        let text_renderer = glyphon::TextRenderer::new(
            &mut atlas,
            &wgpu.device,
            wgpu::MultisampleState {
                count: wgpu.sample_count,
                ..Default::default()
            },
            Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        );

        self.window = Some(window);
        self.glyphon_cache = Some(cache);
        self.text_atlas = Some(atlas);
        self.text_renderer = Some(text_renderer);
        self.wgpu = Some(wgpu);

        // 初始化异步帧管线：后台线程独立塑形文本
        self.frame_pipeline = Some(FramePipeline::new(&self.rt_handle));

        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let mode = self.mode();

        match &event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(ref mut wgpu) = self.wgpu {
                    wgpu.resize(new_size.width, new_size.height);
                }
                if let Some(ref router) = self.router {
                    let mut router = router.borrow_mut();
                    if let Some((page, _)) = router.stack.last_mut() {
                        page.page_mut().root.width = new_size.width as i32;
                        page.page_mut().root.height = new_size.height as i32;
                        page.prepare_layout();
                        page.page_mut().root.layout(None);
                    }
                }
                if let Some(ref w) = self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.last_frame = Instant::now();
                self.render();
                if mode == RunMode::Vsync {
                    if let Some(ref w) = self.window {
                        w.request_redraw();
                    }
                } else if mode == RunMode::Event {
                    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
                }
            }
            WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::ModifiersChanged(_) => {
                if let Some(ref router) = self.router {
                    self.rt_handle
                        .block_on(router.borrow_mut().dispatch_event(&event));
                    if self.should_exit.load(Ordering::Relaxed) {
                        event_loop.exit();
                        return;
                    }
                }
                if mode == RunMode::Event {
                    if let Some(ref w) = self.window {
                        w.request_redraw();
                    }
                }
            }
            _ => {}
        }

        if mode == RunMode::Fps
            && self.last_frame.elapsed().as_secs_f64()
                >= 1.0 / self.router.as_ref().map_or(60, |r| r.borrow().target_fps) as f64
        {
            if let Some(ref w) = self.window {
                w.request_redraw();
            }
        }
        if mode == RunMode::Fps || mode == RunMode::Vsync {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        }
    }
}
