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

/// Compute 管线绘制命令。
pub struct DrawCompute {
    pub compute_wgsl: String,
    pub display_wgsl: String,
    pub snapshot: ComputeSnapshot,
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
