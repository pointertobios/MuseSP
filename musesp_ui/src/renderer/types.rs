use std::sync::Arc;
use crate::components::image::ImageData;

/// 一帧渲染所需的完整数据快照。
///
/// 使用者通过 channel 将快照推送到 RendererCanvas，
/// 渲染器每帧使用最新的快照进行绘制。
#[derive(Clone)]
pub struct RenderSnapshot {
    /// 顶点缓冲区原始数据
    pub vertex_data: Vec<u8>,
    /// 索引缓冲区（u32 索引），为空表示非索引绘制
    pub index_data: Vec<u32>,
    /// 顶点数量（非索引绘制时使用）
    pub vertex_count: u32,
    /// 索引数量（索引绘制时使用，0 表示非索引绘制）
    pub index_count: u32,
    /// 可选纹理数据：(RGBA字节, 宽度, 高度)
    pub texture: Option<(Vec<u8>, u32, u32)>,
    /// Uniform 缓冲区数据，直接传给 shader 的 u_params
    pub uniform_data: Vec<u8>,
}

impl RenderSnapshot {
    /// 创建一个空的快照（无任何绘制数据）
    pub fn empty() -> Self {
        RenderSnapshot {
            vertex_data: Vec::new(),
            index_data: Vec::new(),
            vertex_count: 0,
            index_count: 0,
            texture: None,
            uniform_data: Vec::new(),
        }
    }
}

/// 描述顶点缓冲区的内存布局，用于创建渲染管线。
#[derive(Clone, Debug)]
pub struct VertexLayoutDesc {
    /// 单个顶点的字节跨度
    pub array_stride: u64,
    /// 顶点步进模式（Vertex 或 Instance）
    pub step_mode: wgpu::VertexStepMode,
    /// 顶点属性列表
    pub attributes: Vec<wgpu::VertexAttribute>,
}

/// 自定义着色器绘制命令，由 RendererCanvas 生成。
pub struct DrawRendererCanvas {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub clip_rect: Option<(u32, u32, u32, u32)>,
    pub shader_wgsl: String,
    pub vertex_layout: VertexLayoutDesc,
    pub snapshot: RenderSnapshot,
}

// ── Compute 管线类型 ───────────────────────────────────────────────

/// 一帧 compute 渲染所需的数据快照。
#[derive(Clone)]
pub struct ComputeSnapshot {
    /// 顶点缓冲区（Vertex { position: vec3, color: vec4 }）
    pub vertex_data: Vec<u8>,
    /// 索引缓冲区（每 3 个 u32 = 1 个三角形，顺序任意）
    pub indices: Vec<u32>,
    /// Uniform 数据（Params { view_proj, time, triangle_count, screen_w, screen_h }）
    pub uniform_data: Vec<u8>,
    /// 顶点总数
    pub vertex_count: u32,
    /// 三角形总数
    pub triangle_count: u32,
}

impl ComputeSnapshot {
    pub fn empty() -> Self {
        ComputeSnapshot {
            vertex_data: Vec::new(),
            indices: Vec::new(),
            uniform_data: Vec::new(),
            vertex_count: 0,
            triangle_count: 0,
        }
    }
}

/// Compute 管线绑定模式。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComputeBindingMode {
    Standard,
    /// 细分 pass：vertices(0,r)+indices(1,r)+uniform(2)+vtx_out(3,rw)+idx_out(4,rw)
    Subdivide,
    RasterizeOnly,
}

/// Compute 管线绘制命令。
pub struct DrawCompute {
    pub compute_wgsl: String,
    pub display_wgsl: String,
    pub snapshot: ComputeSnapshot,
    pub binding_mode: ComputeBindingMode,
}

/// 自适应细分常量（与 compute shader 保持一致）
/// 曲面 eval N（与 shader_pass1_eval.wgsl / shader_pass1_final.wgsl 中的 EVAL_N 一致）
pub const EVAL_N: u32 = 4;
/// 线段 eval N（与 line_subdivide_eval.wgsl / line_subdivide_final.wgsl 中的 EVAL_N 一致）
pub const EVAL_N_LINE: u32 = 4;
/// Eval pass workgroup size（与 shader @workgroup_size 一致）
pub const EVAL_WORKGROUP_SIZE: u32 = 64;
/// 曲面每三角形 primary vertices 数量
pub const PRIMARY_VERTICES_PER_TRIANGLE: u32 = (EVAL_N + 1) * (EVAL_N + 2) / 2; // 15
/// 线段每条线 primary vertices 数量
pub const PRIMARY_VERTICES_PER_LINE: u32 = EVAL_N_LINE + 1; // 5
/// 曲面最大细分数
pub const MAX_SUBDIVISIONS_SURFACE: u32 = 8;
/// 线段最大细分数
pub const MAX_SUBDIVISIONS_LINE: u32 = 256;
/// 曲面每三角形最大细分顶点数
pub const MAX_VERTICES_PER_TRIANGLE: u32 = (MAX_SUBDIVISIONS_SURFACE + 1) * (MAX_SUBDIVISIONS_SURFACE + 2) / 2; // 45
/// 曲面每三角形最大细分索引数
pub const MAX_INDICES_PER_TRIANGLE: u32 = MAX_SUBDIVISIONS_SURFACE * MAX_SUBDIVISIONS_SURFACE * 3; // 192
/// 线段每条线最大细分顶点数
pub const MAX_VERTICES_PER_LINE: u32 = MAX_SUBDIVISIONS_LINE + 1; // 257
/// 线段每条线最大细分索引数
pub const MAX_INDICES_PER_LINE: u32 = MAX_SUBDIVISIONS_LINE * 2; // 512

/// Compute 细分 + 硬件光栅化：compute 输出顶点/索引缓冲 → render pass 绘制。
///
/// 两-pass compute：eval（预细分 → primary vertices）+ final（测量 + 自适应细分）。
pub struct DrawSubdivideAndRender {
    pub eval_module: Arc<wgpu::ShaderModule>,
    pub final_module: Arc<wgpu::ShaderModule>,
    pub vertex_module: Arc<wgpu::ShaderModule>,
    pub fragment_module: Arc<wgpu::ShaderModule>,
    pub snapshot: ComputeSnapshot,
}

/// Compute 线段细分 + 硬件光栅化（LineList）。
///
/// 两-pass compute：eval（预细分 → primary vertices）+ final（测量 + 自适应细分）。
/// compute shader 完成后，render pass 用直通 VS + FS 绘制。
pub struct DrawComputeLines {
    pub eval_module: Arc<wgpu::ShaderModule>,
    pub final_module: Arc<wgpu::ShaderModule>,
    pub vertex_module: Arc<wgpu::ShaderModule>,
    pub fragment_module: Arc<wgpu::ShaderModule>,
    /// 球坐标线段端点（LineVertex 格式，每对 = 一条逻辑线段）
    pub endpoint_data: Vec<u8>,
    /// 逻辑线段数量
    pub line_count: u32,
    /// Uniform：view_proj + line_count + _pad + screen_w + screen_h（80 bytes）
    pub uniform_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DrawRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub color: (u8, u8, u8, u8),
    pub clip_rect: Option<(u32, u32, u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct DrawText {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub font_size: u32,
    pub color: (u8, u8, u8),
    pub clip_rect: Option<(u32, u32, u32, u32)>,
}

#[derive(Clone)]
pub struct DrawImage {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub data: ImageData,
    pub clip_rect: Option<(u32, u32, u32, u32)>,
}

/// 3D 直线绘制命令：世界空间线段端点 + view_proj 矩阵。
pub struct DrawLines {
    /// WGSL 着色器源码（含 @vertex 和 @fragment 入口）
    pub shader_wgsl: String,
    /// 顶点缓冲原始字节（LineVertex { position: vec3, _pad: f32, color: vec4 }）
    pub vertex_data: Vec<u8>,
    /// 索引缓冲（每 2 个 u32 = 一条线段）
    pub index_data: Vec<u32>,
    /// Uniform：view_proj mat4x4（列优先，64 字节）
    pub uniform_data: Vec<u8>,
}
