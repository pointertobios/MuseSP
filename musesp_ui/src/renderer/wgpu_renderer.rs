use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use wgpu::util::DeviceExt;
use winit::window::Window;

use super::types::{DrawImage, DrawRect, DrawRendererCanvas, VertexLayoutDesc};

const SHADER_WGSL: &str = include_str!("../shaders/rect.wgsl");
const TEXTURE_SHADER_WGSL: &str = include_str!("../shaders/texture.wgsl");

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniformData {
    view_proj: [[f32; 4]; 4],
    rotation: [[f32; 4]; 4],
}

/// 持有所有 wgpu 状态并负责将 `UIRenderer` 中的绘制命令实际提交到 GPU。
pub struct WgpuRenderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    pipeline_texture: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    camera_bind_group_layout: wgpu::BindGroupLayout,
    pub camera_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub start_time: Instant,
    custom_pipelines: HashMap<(u64, u64), wgpu::RenderPipeline>,
    custom_bgl: Option<wgpu::BindGroupLayout>,
    custom_bgl_hash: u64,
}

impl WgpuRenderer {
    pub async fn new(window: Arc<Window>) -> Self {
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

        // -- rect pipeline --
        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SHADER_WGSL)),
        });
        let rect_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
                ..Default::default()
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
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

        // -- texture pipeline --
        let tex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
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
        let tex_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&texture_bind_group_layout)],
                ..Default::default()
            });
        let pipeline_texture =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&tex_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &tex_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 16,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                    })],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &tex_shader,
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

        WgpuRenderer {
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
            custom_pipelines: HashMap::new(),
            custom_bgl: None,
            custom_bgl_hash: 0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// 绘制纯色矩形列表。
    pub fn draw_rects(&self, rp: &mut wgpu::RenderPass<'_>, rects: &[DrawRect]) {
        rp.set_pipeline(&self.pipeline);
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        for rect in rects {
            if let Some(clip) = rect.clip_rect {
                rp.set_scissor_rect(clip.0, clip.1, clip.2, clip.3);
            }
            let vertices = rect_vertices(
                rect.x as f32,
                rect.y as f32,
                rect.w as f32,
                rect.h as f32,
                sw,
                sh,
                rect.color,
            );
            let buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            rp.set_vertex_buffer(0, buf.slice(..));
            rp.draw(0..6, 0..1);
            if rect.clip_rect.is_some() {
                rp.set_scissor_rect(0, 0, self.config.width, self.config.height);
            }
        }
    }

    /// 绘制纹理图片列表。
    pub fn draw_images(&self, rp: &mut wgpu::RenderPass<'_>, images: &[DrawImage]) {
        rp.set_pipeline(&self.pipeline_texture);
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        for img in images {
            if let Some(clip) = img.clip_rect {
                rp.set_scissor_rect(clip.0, clip.1, clip.2, clip.3);
            }
            let tex_size = wgpu::Extent3d {
                width: img.data.width,
                height: img.data.height,
                depth_or_array_layers: 1,
            };
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
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
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            rp.set_bind_group(0, &bind_group, &[]);

            let vertices = texture_vertices(
                img.x as f32,
                img.y as f32,
                img.w as f32,
                img.h as f32,
                sw,
                sh,
            );
            let buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            rp.set_vertex_buffer(0, buf.slice(..));
            rp.draw(0..6, 0..1);
            if img.clip_rect.is_some() {
                rp.set_scissor_rect(0, 0, self.config.width, self.config.height);
            }
        }
    }

    /// 绘制所有自定义着色器绘制命令（RendererCanvas）。
    pub fn draw_custom(
        &mut self,
        rp: &mut wgpu::RenderPass<'_>,
        draws: &[DrawRendererCanvas],
    ) {
        let format = self.config.format;
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;

        for draw in draws {
            if draw.snapshot.vertex_data.is_empty() || draw.snapshot.vertex_count == 0 {
                continue;
            }
            if let Some(clip) = draw.clip_rect {
                rp.set_scissor_rect(clip.0, clip.1, clip.2, clip.3);
            }

            let has_tex = draw.snapshot.texture.is_some();
            let has_uniform = !draw.snapshot.uniform_data.is_empty();

            let pipeline = self.get_or_create_custom_pipeline(
                &draw.shader_wgsl,
                &draw.vertex_layout,
                has_tex,
                has_uniform,
                format,
            );

            if has_tex || has_uniform {
                let bgl = self.get_or_create_bgl(has_tex, has_uniform);

                let (tex_view, sampler) = if let Some((ref rgba, tw, th)) =
                    draw.snapshot.texture
                {
                    let tex_size = wgpu::Extent3d {
                        width: tw,
                        height: th,
                        depth_or_array_layers: 1,
                    };
                    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("custom_tex"),
                        size: tex_size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        rgba,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * tw),
                            rows_per_image: Some(th),
                        },
                        tex_size,
                    );
                    let view =
                        texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let s = self.device.create_sampler(&wgpu::SamplerDescriptor {
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        ..Default::default()
                    });
                    (Some(view), Some(s))
                } else {
                    (None, None)
                };

                let uniform_buf = if has_uniform {
                    Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("custom_uniform"),
                            contents: &draw.snapshot.uniform_data,
                            usage: wgpu::BufferUsages::UNIFORM,
                        },
                    ))
                } else {
                    None
                };

                let mut entries: Vec<wgpu::BindGroupEntry<'_>> = Vec::new();
                if let Some(ref tv) = tex_view {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(tv),
                    });
                }
                if let Some(ref s) = sampler {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(s),
                    });
                }
                if let Some(ref ub) = uniform_buf {
                    let binding = if has_tex { 2u32 } else { 0u32 };
                    entries.push(wgpu::BindGroupEntry {
                        binding,
                        resource: ub.as_entire_binding(),
                    });
                }

                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("custom_bg"),
                    layout: &bgl,
                    entries: &entries,
                });
                rp.set_bind_group(0, &bind_group, &[]);
            }

            let vertex_buf =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("custom_vbuf"),
                        contents: &draw.snapshot.vertex_data,
                        usage: wgpu::BufferUsages::VERTEX,
                    });

            let is_indexed =
                draw.snapshot.index_count > 0 && !draw.snapshot.index_data.is_empty();
            rp.set_pipeline(&pipeline);
            rp.set_vertex_buffer(0, vertex_buf.slice(..));
            if is_indexed {
                let index_buf =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("custom_ibuf"),
                            contents: bytemuck::cast_slice(&draw.snapshot.index_data),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                rp.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..draw.snapshot.index_count, 0, 0..1);
            } else {
                rp.draw(0..draw.snapshot.vertex_count, 0..1);
            }

            if draw.clip_rect.is_some() {
                rp.set_scissor_rect(0, 0, sw as u32, sh as u32);
            }
        }
    }

    /// 更新相机 uniform。
    pub fn update_camera(&self, elapsed_secs: f32) {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let proj = perspective(60.0f32.to_radians(), aspect, 0.1, 100.0);
        let tilt = rotation_x(25.0f32.to_radians());
        let cam_pos = translation(0.0, 0.0, -5.0);
        let cam_view = mul4(&tilt, &cam_pos);
        let view_proj = mul4(&proj, &cam_view);
        let angle = elapsed_secs * 0.5 * 2.0 * std::f32::consts::PI;
        let rot = rotation_y(angle);
        let camera_data = CameraUniformData {
            view_proj,
            rotation: rot,
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_data));
    }

    // ── private helpers ────────────────────────────────────────────────

    fn get_or_create_custom_pipeline(
        &mut self,
        shader_wgsl: &str,
        vertex_layout: &VertexLayoutDesc,
        has_tex: bool,
        has_uniform: bool,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        use std::hash::{Hash, Hasher};
        let key = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            shader_wgsl.hash(&mut h);
            vertex_layout.array_stride.hash(&mut h);
            format!("{:?}", &vertex_layout.attributes).hash(&mut h);
            has_tex.hash(&mut h);
            has_uniform.hash(&mut h);
            h.finish()
        };
        self.custom_pipelines
            .entry((key, 0))
            .or_insert_with(|| {
                create_custom_pipeline(
                    &self.device,
                    format,
                    shader_wgsl,
                    vertex_layout,
                    has_tex,
                    has_uniform,
                )
            })
            .clone()
    }

    fn get_or_create_bgl(
        &mut self,
        has_texture: bool,
        has_uniform: bool,
    ) -> wgpu::BindGroupLayout {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        has_texture.hash(&mut h);
        has_uniform.hash(&mut h);
        let key = h.finish();

        if self.custom_bgl_hash == key {
            if let Some(ref bgl) = self.custom_bgl {
                return bgl.clone();
            }
        }

        let bgl = create_bind_group_layout(&self.device, has_texture, has_uniform);
        self.custom_bgl_hash = key;
        self.custom_bgl = Some(bgl.clone());
        bgl
    }
}

// ── helper functions ─────────────────────────────────────────────────────

fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y / 2.0).tan();
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, (far + near) / (near - far), (2.0 * far * near) / (near - far)],
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
            m[r][c] =
                a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + a[r][3] * b[3][c];
        }
    }
    m
}

fn rect_vertices(
    x: f32, y: f32, w: f32, h: f32,
    screen_w: f32, screen_h: f32,
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
    [
        [x1, y1, 0.0, 0.0],
        [x2, y1, 1.0, 0.0],
        [x2, y2, 1.0, 1.0],
        [x1, y1, 0.0, 0.0],
        [x2, y2, 1.0, 1.0],
        [x1, y2, 0.0, 1.0],
    ]
}

fn create_bind_group_layout(
    device: &wgpu::Device,
    has_texture: bool,
    has_uniform: bool,
) -> wgpu::BindGroupLayout {
    let mut entries = Vec::new();
    if has_texture {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    if has_uniform {
        let binding = if has_texture { 2u32 } else { 0u32 };
        entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("custom_bgl"),
        entries: &entries,
    })
}

fn create_custom_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader_wgsl: &str,
    vertex_layout: &VertexLayoutDesc,
    has_texture: bool,
    has_uniform: bool,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("custom_canvas_shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_wgsl)),
    });

    let mut bgl_entries = Vec::new();
    if has_texture {
        bgl_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        bgl_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    if has_uniform {
        let binding = if has_texture { 2u32 } else { 0u32 };
        bgl_entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }

    let bgl = if bgl_entries.is_empty() {
        None
    } else {
        Some(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("custom_pipe_bgl"),
            entries: &bgl_entries,
        }))
    };

    let bgl_refs: Vec<Option<&wgpu::BindGroupLayout>> = bgl.iter().map(Some).collect();

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("custom_pipe_layout"),
        bind_group_layouts: &bgl_refs,
        ..Default::default()
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("custom_canvas_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: vertex_layout.array_stride,
                step_mode: vertex_layout.step_mode,
                attributes: &vertex_layout.attributes,
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
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
    })
}
