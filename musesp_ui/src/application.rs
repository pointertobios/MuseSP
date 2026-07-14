use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::renderer::UIRenderer;
use crate::router::{AnyPage, PageToken, Router};
use musesp_config::config::Config;

const SHADER_WGSL: &str = include_str!("rect.wgsl");
const TEXTURE_SHADER_WGSL: &str = include_str!("texture.wgsl");

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniformData {
    view_proj: [[f32; 4]; 4],
    rotation: [[f32; 4]; 4],
}

fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y / 2.0).tan();
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [
            0.0,
            0.0,
            (far + near) / (near - far),
            (2.0 * far * near) / (near - far),
        ],
        [0.0, 0.0, -1.0, 0.0],
    ]
}

fn rotation_y(angle: f32) -> [[f32; 4]; 4] {
    let (s, c) = angle.sin_cos();
    [
        [c, 0.0, s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-s, 0.0, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rotation_x(angle: f32) -> [[f32; 4]; 4] {
    let (s, c) = angle.sin_cos();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, c, -s, 0.0],
        [0.0, s, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn translation(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [x, y, z, 1.0],
    ]
}

fn mul4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut m = [[0.0f32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            m[r][c] = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + a[r][3] * b[3][c];
        }
    }
    m
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Event,
    Fps,
    Vsync,
}

struct WgpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    pipeline_texture: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    camera_bind_group_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    start_time: Instant,
}

impl WgpuState {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SHADER_WGSL)),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<CameraUniformData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Texture pipeline for image rendering
        let texture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(TEXTURE_SHADER_WGSL)),
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&texture_bind_group_layout)],
                ..Default::default()
            });
        let pipeline_texture =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&texture_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &texture_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 16,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                    })],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &texture_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        WgpuState {
            surface,
            device,
            queue,
            config,
            pipeline,
            pipeline_texture,
            texture_bind_group_layout,
            camera_bind_group_layout,
            camera_buffer,
            depth_texture,
            depth_view,
            start_time: Instant::now(),
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}

pub struct Application {
    name: String,
    router: Option<Arc<RefCell<Router>>>,
    config: Config,
    window: Option<Arc<Window>>,
    wgpu: Option<WgpuState>,
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
}

impl Application {
    pub fn run<P: AnyPage + 'static>(name: &str, page: P) {
        let mut app = Application {
            name: name.to_string(),
            router: None,
            config: musesp_config::config::load_config(),
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
                router_ref.init_page(&mut page);
                router_ref.stack.push((page, PageToken::new()));
            }
        }
        self.router = Some(router_rc);
    }

    fn render(&mut self) {
        let wgpu = self.wgpu.as_ref().unwrap();
        let atlas = self.text_atlas.as_mut().unwrap();
        let text_renderer = self.text_renderer.as_mut().unwrap();

        self.renderer.clear();
        if let Some(ref router) = self.router {
            router.borrow().draw_pages(&mut self.renderer, &self.config);
        }

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

        let mut viewport =
            glyphon::Viewport::new(&wgpu.device, self.glyphon_cache.as_ref().unwrap());
        viewport.update(
            &wgpu.queue,
            glyphon::Resolution {
                width: wgpu.config.width,
                height: wgpu.config.height,
            },
        );

        self.text_buffers.clear();
        for t in &self.renderer.texts {
            let mut buffer = glyphon::Buffer::new(
                &mut self.font_system,
                glyphon::Metrics::new(t.font_size as f32, (t.font_size as f32) * 1.2),
            );
            buffer.set_size(Some(t.w as f32), Some(t.h as f32));
            buffer.set_text(
                &t.text,
                &glyphon::Attrs::new().color(glyphon::Color::rgb(t.color.0, t.color.1, t.color.2)),
                glyphon::Shaping::Advanced,
                Some(glyphon::cosmic_text::Align::Center),
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            self.text_buffers.push(buffer);
        }

        let text_areas: Vec<glyphon::TextArea> = self
            .renderer
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

        // Update camera uniform
        let elapsed = wgpu.start_time.elapsed().as_secs_f32();
        let angle = elapsed * 0.5 * 2.0 * std::f32::consts::PI;
        let proj = perspective(
            60.0f32.to_radians(),
            wgpu.config.width as f32 / wgpu.config.height as f32,
            0.1,
            100.0,
        );
        let tilt = rotation_x(25.0f32.to_radians());
        let cam_pos = translation(0.0, 0.0, -5.0);
        let cam_view = mul4(&tilt, &cam_pos);
        let view_proj = mul4(&proj, &cam_view);
        let rot = rotation_y(angle);
        let camera_data = CameraUniformData {
            view_proj,
            rotation: rot,
        };
        wgpu.queue
            .write_buffer(&wgpu.camera_buffer, 0, bytemuck::bytes_of(&camera_data));

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
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

            // 2D rects
            rp.set_pipeline(&wgpu.pipeline);
            let w = wgpu.config.width as f32;
            let h = wgpu.config.height as f32;
            for rect in &self.renderer.rects {
                if let Some(clip) = rect.clip_rect {
                    rp.set_scissor_rect(clip.0, clip.1, clip.2, clip.3);
                }
                let vertices = rect_vertices(
                    rect.x as f32,
                    rect.y as f32,
                    rect.w as f32,
                    rect.h as f32,
                    w,
                    h,
                    rect.color,
                );
                let buf = wgpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                rp.set_vertex_buffer(0, buf.slice(..));
                rp.draw(0..6, 0..1);
                if rect.clip_rect.is_some() {
                    rp.set_scissor_rect(0, 0, wgpu.config.width, wgpu.config.height);
                }
            }

            // Images (textured quads)
            rp.set_pipeline(&wgpu.pipeline_texture);
            for img in &self.renderer.images {
                if let Some(clip) = img.clip_rect {
                    rp.set_scissor_rect(clip.0, clip.1, clip.2, clip.3);
                }
                let tex_size = wgpu::Extent3d {
                    width: img.data.width,
                    height: img.data.height,
                    depth_or_array_layers: 1,
                };
                let texture = wgpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: tex_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                wgpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &img.data.rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * img.data.width),
                        rows_per_image: Some(img.data.height),
                    },
                    tex_size,
                );
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let sampler = wgpu.device.create_sampler(&wgpu::SamplerDescriptor {
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                });
                let bind_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &wgpu.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                    ],
                });
                rp.set_bind_group(0, &bind_group, &[]);

                let vertices = texture_vertices(
                    img.x as f32, img.y as f32,
                    img.w as f32, img.h as f32,
                    w, h,
                );
                let buf = wgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                rp.set_vertex_buffer(0, buf.slice(..));
                rp.draw(0..6, 0..1);
                if img.clip_rect.is_some() {
                    rp.set_scissor_rect(0, 0, wgpu.config.width, wgpu.config.height);
                }
            }

            if let Err(e) = text_renderer.render(atlas, &viewport, &mut rp) {
                eprintln!("glyphon render error: {:?}", e);
            }
        }

        wgpu.queue.submit(Some(encoder.finish()));
        wgpu.queue.present(output);
    }
}

fn rect_vertices(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    screen_w: f32,
    screen_h: f32,
    color: (u8, u8, u8, u8),
) -> [[f32; 6]; 6] {
    let x1 = x / screen_w * 2.0 - 1.0;
    let y1 = 1.0 - y / screen_h * 2.0;
    let x2 = (x + w) / screen_w * 2.0 - 1.0;
    let y2 = 1.0 - (y + h) / screen_h * 2.0;
    let r = color.0 as f32 / 255.0;
    let g = color.1 as f32 / 255.0;
    let b = color.2 as f32 / 255.0;
    let a = color.3 as f32 / 255.0;
    [
        [x1, y1, r, g, b, a],
        [x2, y1, r, g, b, a],
        [x2, y2, r, g, b, a],
        [x1, y1, r, g, b, a],
        [x2, y2, r, g, b, a],
        [x1, y2, r, g, b, a],
    ]
}

fn texture_vertices(
    x: f32, y: f32, w: f32, h: f32,
    screen_w: f32, screen_h: f32,
) -> [[f32; 4]; 6] {
    let x1 = x / screen_w * 2.0 - 1.0;
    let y1 = 1.0 - y / screen_h * 2.0;
    let x2 = (x + w) / screen_w * 2.0 - 1.0;
    let y2 = 1.0 - (y + h) / screen_h * 2.0;
    // pos(x,y), uv(u,v): TL(0,0) TR(1,0) BR(1,1) BL(0,1)
    [
        [x1, y1, 0.0, 0.0],
        [x2, y1, 1.0, 0.0],
        [x2, y2, 1.0, 1.0],
        [x1, y1, 0.0, 0.0],
        [x2, y2, 1.0, 1.0],
        [x1, y2, 0.0, 1.0],
    ]
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
        let wgpu = pollster::block_on(WgpuState::new(window.clone()));
        let format = wgpu.config.format;

        let cache = glyphon::Cache::new(&wgpu.device);
        let mut atlas = glyphon::TextAtlas::new(&wgpu.device, &wgpu.queue, &cache, format);
        let text_renderer = glyphon::TextRenderer::new(
            &mut atlas,
            &wgpu.device,
            wgpu::MultisampleState::default(),
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
            | WindowEvent::KeyboardInput { .. } => {
                if let Some(ref router) = self.router {
                    router.borrow_mut().dispatch_event(&event);
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
