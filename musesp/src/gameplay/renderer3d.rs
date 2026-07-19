//! 球坐标渲染管线（两 pass 架构）。
//!
//! Pass 1 (GPU Subdivide)：在球坐标参数空间细分粗三角形 → Cartesian → 屏幕坐标
//! Pass 2 (GPU Rasterize)：屏幕空间子三角形边函数光栅化 + Alpha 混合
//!
//! CPU 只负责生成粗球坐标几何 + view_proj 矩阵。
//!
//! 通过实现 `musesp_ui::renderer::RenderPipeline` trait，
//! 本模块拥有完整的 compute/graphics pass 控制权，musesp_ui 不感知具体实现。

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::Arc;

use musesp_ui::renderer::RenderPipeline;

// ── 自适应细分常量（与 compute shader 保持一致）────────────────────────

/// 曲面 eval N（与 shader_pass1_eval.wgsl / shader_pass1_final.wgsl 中的 EVAL_N 一致）
const EVAL_N: u32 = 4;
/// 线段 eval N（与 line_subdivide_eval.wgsl / line_subdivide_final.wgsl 中的 EVAL_N 一致）
const EVAL_N_LINE: u32 = 4;
/// Eval pass workgroup size（与 shader @workgroup_size 一致）
const EVAL_WORKGROUP_SIZE: u32 = 64;
/// 曲面每三角形 primary vertices 数量
const PRIMARY_VERTICES_PER_TRIANGLE: u32 = (EVAL_N + 1) * (EVAL_N + 2) / 2; // 15
/// 线段每条线 primary vertices 数量
const PRIMARY_VERTICES_PER_LINE: u32 = EVAL_N_LINE + 1; // 5
/// 曲面最大细分数
const MAX_SUBDIVISIONS_SURFACE: u32 = 8;
/// 线段最大细分数
const MAX_SUBDIVISIONS_LINE: u32 = 256;
/// 曲面每三角形最大细分顶点数
const MAX_VERTICES_PER_TRIANGLE: u32 =
    (MAX_SUBDIVISIONS_SURFACE + 1) * (MAX_SUBDIVISIONS_SURFACE + 2) / 2; // 45
/// 曲面每三角形最大细分索引数
const MAX_INDICES_PER_TRIANGLE: u32 = MAX_SUBDIVISIONS_SURFACE * MAX_SUBDIVISIONS_SURFACE * 3; // 192
/// 线段每条线最大细分顶点数
const MAX_VERTICES_PER_LINE: u32 = MAX_SUBDIVISIONS_LINE + 1; // 257
/// 线段每条线最大细分索引数
const MAX_INDICES_PER_LINE: u32 = MAX_SUBDIVISIONS_LINE * 2; // 512

// ── 几何常量 ──────────────────────────────────────────────────────────

const NEAR: f32 = 0.1;
const FAR: f32 = 200.0;
const SUB_GRID_SIZE: u32 = 6;

// ── 摄像机 ────────────────────────────────────────────────────────────

/// 摄像机运行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    /// 正常游戏：准星跟随鼠标，九宫格动态更新
    Playing,
    /// 调整模式：摄像机可移动，九宫格固定在切换前位置
    Adjusting,
}

/// 摄像机状态（动态可调）
#[derive(Debug, Clone)]
pub struct CameraState {
    pub eye: [f32; 3],
    /// 摄像机朝向（单位向量，从 eye 指向观察方向）
    pub direction: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
}

impl CameraState {
    pub fn from_config(cam: &musesp_config::config::CameraConfig) -> Self {
        CameraState {
            eye: cam.eye,
            direction: cam.direction,
            up: cam.up,
            fov_degrees: cam.fov_degrees,
        }
    }

    /// 计算 view_proj 矩阵
    pub fn view_proj(&self, aspect: f32) -> [[f32; 4]; 4] {
        let proj = perspective(self.fov_degrees.to_radians(), aspect, NEAR, FAR);
        let target = add(self.eye, self.direction);
        let view = look_at(self.eye, target, self.up);
        mul4(&proj, &view)
    }

    /// 摄像机前方方向（归一化）
    pub fn forward(&self) -> [f32; 3] {
        self.direction
    }

    /// 摄像机右方向
    pub fn right(&self) -> [f32; 3] {
        normalize(cross(self.direction, self.up))
    }

    /// 移动摄像机（只移动 eye，方向不变）
    pub fn translate(&mut self, delta: [f32; 3]) {
        self.eye[0] += delta[0];
        self.eye[1] += delta[1];
        self.eye[2] += delta[2];
    }

    /// 旋转摄像机朝向（第一人称视角），角度增量（弧度）
    /// delta_yaw: 水平旋转，delta_pitch: 垂直旋转
    pub fn rotate_view(&mut self, delta_yaw: f32, delta_pitch: f32) {
        // 限制 pitch 范围，防止翻转
        let current_pitch = self.direction[1].asin();
        let max_delta = 1.5 - current_pitch;
        let min_delta = -1.5 - current_pitch;
        let delta_pitch = delta_pitch.clamp(min_delta, max_delta);

        // Yaw：绕世界 Y 轴 (0,1,0) 旋转 direction
        let cos_y = delta_yaw.cos();
        let sin_y = delta_yaw.sin();
        let fwd: [f32; 3] = [
            self.direction[0] * cos_y + self.direction[2] * sin_y,
            self.direction[1],
            -self.direction[0] * sin_y + self.direction[2] * cos_y,
        ];
        let fwd = normalize(fwd);

        // Pitch：绕摄像机右轴旋转（Rodrigues 公式，k·v=0 简化）
        let right = normalize(cross(fwd, self.up));
        let cos_p = delta_pitch.cos();
        let sin_p = delta_pitch.sin();
        let k_cross_v = cross(right, fwd);

        self.direction = normalize([
            fwd[0] * cos_p + k_cross_v[0] * sin_p,
            fwd[1] * cos_p + k_cross_v[1] * sin_p,
            fwd[2] * cos_p + k_cross_v[2] * sin_p,
        ]);
    }

    /// 调整 FOV
    pub fn adjust_fov(&mut self, delta: f32) {
        self.fov_degrees = (self.fov_degrees + delta).clamp(10.0, 120.0);
    }
}

// ── 类型定义 ──────────────────────────────────────────────────────────

/// 一帧 compute 渲染所需的数据快照。
#[derive(Clone)]
pub struct ComputeSnapshot {
    /// 顶点缓冲区（Vertex { position: vec3, color: vec4 }）
    pub vertex_data: Vec<u8>,
    /// 索引缓冲区（每 3 个 u32 = 1 个三角形，顺序任意）
    pub indices: Vec<u32>,
    /// Uniform 数据（Params { view_proj, time, triangle_count, screen_w, screen_h }）
    pub uniform_data: Vec<u8>,
    /// 三角形总数
    pub triangle_count: u32,
}

impl ComputeSnapshot {
    pub fn empty() -> Self {
        ComputeSnapshot {
            vertex_data: Vec::new(),
            indices: Vec::new(),
            uniform_data: Vec::new(),
            triangle_count: 0,
        }
    }
}

/// Compute 管线绑定布局（用于 surface/line 自适应细分）。
/// 6 个 storage/uniform binding（只读顶点、只读索引、uniform、读写 primary/sub vtx/sub idx）。
const COMPUTE_BGL_ENTRIES: &[wgpu::BindGroupLayoutEntry] = &[
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    wgpu::BindGroupLayoutEntry {
        binding: 1,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    wgpu::BindGroupLayoutEntry {
        binding: 2,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    wgpu::BindGroupLayoutEntry {
        binding: 3,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    wgpu::BindGroupLayoutEntry {
        binding: 4,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    wgpu::BindGroupLayoutEntry {
        binding: 5,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
];

// ── 球坐标顶点（传给 GPU Pass 1）─────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SphericalVertex {
    pub r: f32,
    pub theta: f32,
    pub phi: f32,
    _pad: f32,
    pub color: [f32; 4],
}

impl SphericalVertex {
    #[allow(dead_code)]
    fn new(r: f32, theta: f32, phi: f32, color: [f32; 4]) -> Self {
        SphericalVertex {
            r,
            theta,
            phi,
            _pad: 0.0,
            color,
        }
    }
}

// ── 直线顶点（球坐标，传给 GPU line pipeline）─────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub r: f32,
    pub theta: f32,
    pub phi: f32,
    _pad: f32,
    pub color: [f32; 4],
}

impl LineVertex {
    #[allow(dead_code)]
    pub fn from_cartesian(pos: [f32; 3], color: [f32; 4]) -> Self {
        let r = (pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]).sqrt();
        let (theta, phi) = if r < 1e-6 {
            (0.0, 0.0)
        } else {
            (f32::acos(pos[1] / r), f32::atan2(pos[2], pos[0]))
        };
        LineVertex {
            r,
            theta,
            phi,
            _pad: 0.0,
            color,
        }
    }
}

// ── Pass 1 参数（80 字节） ────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SubdivideParams {
    view_proj: [[f32; 4]; 4],
    camera_eye: [f32; 3],
    _pad2: f32,
    triangle_count: u32,
    sub_grid_size: u32,
    screen_width: f32,
    screen_height: f32,
}

// ── 几何生成 ──────────────────────────────────────────────────────────

#[allow(dead_code)]
fn hemisphere(_theta_bands: u32, _phi_steps: u32, _rot_phi: f32) -> (Vec<SphericalVertex>, Vec<u32>) {
    // 测试几何已移除，由 gameplay 逻辑提供
    (Vec::new(), Vec::new())
}

fn all_geometry(_elapsed_secs: f32) -> (Vec<SphericalVertex>, Vec<u32>) {
    // 测试几何已移除
    (Vec::new(), Vec::new())
}

// ── 玩法状态 ──────────────────────────────────────────────────────────

/// 全局玩法状态（线程安全）。
#[derive(Debug, Clone)]
pub struct GameplayState {
    /// 鼠标屏幕坐标（像素）
    pub mouse_screen: Option<(f32, f32)>,
    /// 球面拾取点 (theta, phi)，弧度
    pub picked_spherical: Option<(f32, f32)>,
    /// 九宫格每格角尺寸（弧度）
    #[allow(dead_code)]
    pub grid_cell_angular: f32,
    /// 判定球面半径
    pub sphere_radius: f32,
    /// 屏幕尺寸
    pub screen_size: (f32, f32),
    /// 摄像机状态
    pub camera: CameraState,
    /// 摄像机模式
    pub camera_mode: CameraMode,
}

impl GameplayState {
    pub fn new() -> Self {
        GameplayState {
            mouse_screen: None,
            picked_spherical: None,
            grid_cell_angular: 12.0_f32.to_radians(),
            sphere_radius: 20.0,
            screen_size: (1920.0, 1080.0),
            camera: CameraState {
                eye: [35.0, 25.0, 35.0],
                direction: normalize(sub([0.0, 0.0, 0.0], [35.0, 25.0, 35.0])),
                up: [0.0, 1.0, 0.0],
                fov_degrees: 60.0,
            },
            camera_mode: CameraMode::Playing,
        }
    }

    /// 用配置文件中的摄像机参数初始化
    pub fn apply_camera_config(&mut self, cam: &musesp_config::config::CameraConfig) {
        self.camera = CameraState::from_config(cam);
    }
}

use std::sync::Mutex;
static GAMEPLAY_STATE: std::sync::OnceLock<Arc<Mutex<GameplayState>>> = std::sync::OnceLock::new();

pub fn init_gameplay_state() -> Arc<Mutex<GameplayState>> {
    GAMEPLAY_STATE
        .get_or_init(|| Arc::new(Mutex::new(GameplayState::new())))
        .clone()
}

pub fn get_gameplay_state() -> Option<Arc<Mutex<GameplayState>>> {
    GAMEPLAY_STATE.get().cloned()
}

// ── 球面拾取 ──────────────────────────────────────────────────────────

/// 屏幕坐标 → 球面交点 (theta, phi)。
/// 返回值：None 表示射线未命中球面。
pub fn pick_sphere(
    screen_pos: (f32, f32),
    screen_size: (f32, f32),
    view_proj: &[[f32; 4]; 4],
    camera_eye: [f32; 3],
    sphere_radius: f32,
) -> Option<(f32, f32)> {
    // 1. NDC
    let ndc_x = 2.0 * screen_pos.0 / screen_size.0 - 1.0;
    let ndc_y = 1.0 - 2.0 * screen_pos.1 / screen_size.1;

    // 2. 逆 view_proj
    let inv_vp = inverse4(view_proj)?;

    // 3. 近平面点 → 世界射线方向（wgpu NDC z 范围 [0, 1]，近平面 z=0）
    let near_ndc = [ndc_x, ndc_y, 0.0, 1.0f32];
    let near_world = transform_point(&inv_vp, &near_ndc);
    let ray_dir = normalize(sub(near_world, camera_eye));

    // 4. 射线-球面求交：|eye + t*dir|^2 = r^2
    let oc = camera_eye; // sphere center = origin
    let a = dot(ray_dir, ray_dir);
    let b = 2.0 * dot(oc, ray_dir);
    let c = dot(oc, oc) - sphere_radius * sphere_radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_d = discriminant.sqrt();
    let t0 = (-b - sqrt_d) / (2.0 * a);
    let t1 = (-b + sqrt_d) / (2.0 * a);

    // 选择更近的正交点
    let t = if t0 > 0.0 {
        t0
    } else if t1 > 0.0 {
        t1
    } else {
        return None;
    };

    // 5. 交点 Cartesian → 球坐标 (theta, phi)
    let hit = [
        camera_eye[0] + t * ray_dir[0],
        camera_eye[1] + t * ray_dir[1],
        camera_eye[2] + t * ray_dir[2],
    ];
    let r = (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt();
    if r < 1e-6 {
        return None;
    }
    let theta = f32::acos(hit[1] / r); // polar angle from +Y
    let phi = f32::atan2(hit[2], hit[0]); // azimuth in XZ plane

    Some((theta, phi))
}

// ── 九宫格线生成 ────────────────────────────────────────────────────

/// 在球面上生成 3×3 九宫格的线顶点（用于 line pipeline）。
/// center: (theta, phi) 九宫格中心
/// cell_angular: 每格角尺寸（弧度）
/// radius: 球面半径
/// color: 线条颜色
pub fn judgment_grid_lines(
    center: (f32, f32),
    cell_angular: f32,
    radius: f32,
    color: [f32; 4],
) -> (Vec<LineVertex>, u32) {
    let (ct, cp) = center;
    let half = 1.5 * cell_angular;

    let t_min = ct - half;
    let t_max = ct + half;
    let p_min = cp - half;
    let p_max = cp + half;

    let mut verts = Vec::new();

    // 4 条水平线（固定 theta，变化 phi）
    for i in 0..4 {
        let t = t_min + i as f32 * cell_angular;
        verts.push(LineVertex {
            r: radius,
            theta: t.clamp(0.0, PI),
            phi: p_min,
            _pad: 0.0,
            color,
        });
        verts.push(LineVertex {
            r: radius,
            theta: t.clamp(0.0, PI),
            phi: p_max,
            _pad: 0.0,
            color,
        });
    }
    // 4 条竖直线（固定 phi，变化 theta）
    for i in 0..4 {
        let p = p_min + i as f32 * cell_angular;
        verts.push(LineVertex {
            r: radius,
            theta: t_min.clamp(0.0, PI),
            phi: p,
            _pad: 0.0,
            color,
        });
        verts.push(LineVertex {
            r: radius,
            theta: t_max.clamp(0.0, PI),
            phi: p,
            _pad: 0.0,
            color,
        });
    }

    let line_count = (verts.len() / 2) as u32;
    (verts, line_count)
}

// ── 4×4 矩阵求逆 ────────────────────────────────────────────────────

fn inverse4(m: &[[f32; 4]; 4]) -> Option<[[f32; 4]; 4]> {
    let mut inv = [[0.0f32; 4]; 4];
    let det;
    let mut tmp = [0.0f32; 12]; // 用于子矩阵计算
    let src = [
        m[0][0], m[1][0], m[2][0], m[3][0], m[0][1], m[1][1], m[2][1], m[3][1], m[0][2], m[1][2],
        m[2][2], m[3][2], m[0][3], m[1][3], m[2][3], m[3][3],
    ];

    // 前 8 个 cofactor 的临时计算
    tmp[0] = src[10] * src[15];
    tmp[1] = src[11] * src[14];
    tmp[2] = src[9] * src[15];
    tmp[3] = src[11] * src[13];
    tmp[4] = src[9] * src[14];
    tmp[5] = src[10] * src[13];
    tmp[6] = src[8] * src[15];
    tmp[7] = src[11] * src[12];
    tmp[8] = src[8] * src[14];
    tmp[9] = src[10] * src[12];
    tmp[10] = src[8] * src[13];
    tmp[11] = src[9] * src[12];

    inv[0][0] = tmp[0] * src[5] + tmp[3] * src[6] + tmp[4] * src[7];
    inv[0][0] -= tmp[1] * src[5] + tmp[2] * src[6] + tmp[5] * src[7];
    inv[0][1] = tmp[1] * src[4] + tmp[6] * src[6] + tmp[9] * src[7];
    inv[0][1] -= tmp[0] * src[4] + tmp[7] * src[6] + tmp[8] * src[7];
    inv[0][2] = tmp[2] * src[4] + tmp[7] * src[5] + tmp[10] * src[7];
    inv[0][2] -= tmp[3] * src[4] + tmp[6] * src[5] + tmp[11] * src[7];
    inv[0][3] = tmp[5] * src[4] + tmp[8] * src[5] + tmp[11] * src[6];
    inv[0][3] -= tmp[4] * src[4] + tmp[9] * src[5] + tmp[10] * src[6];
    inv[1][0] = tmp[1] * src[1] + tmp[2] * src[2] + tmp[5] * src[3];
    inv[1][0] -= tmp[0] * src[1] + tmp[3] * src[2] + tmp[4] * src[3];
    inv[1][1] = tmp[0] * src[0] + tmp[7] * src[2] + tmp[8] * src[3];
    inv[1][1] -= tmp[1] * src[0] + tmp[6] * src[2] + tmp[9] * src[3];
    inv[1][2] = tmp[3] * src[0] + tmp[6] * src[1] + tmp[11] * src[3];
    inv[1][2] -= tmp[2] * src[0] + tmp[7] * src[1] + tmp[10] * src[3];
    inv[1][3] = tmp[4] * src[0] + tmp[9] * src[1] + tmp[10] * src[2];
    inv[1][3] -= tmp[5] * src[0] + tmp[8] * src[1] + tmp[11] * src[2];

    // 后 8 个 cofactor
    tmp[0] = src[2] * src[7];
    tmp[1] = src[3] * src[6];
    tmp[2] = src[1] * src[7];
    tmp[3] = src[3] * src[5];
    tmp[4] = src[1] * src[6];
    tmp[5] = src[2] * src[5];
    tmp[6] = src[0] * src[7];
    tmp[7] = src[3] * src[4];
    tmp[8] = src[0] * src[6];
    tmp[9] = src[2] * src[4];
    tmp[10] = src[0] * src[5];
    tmp[11] = src[1] * src[4];

    inv[2][0] = tmp[0] * src[13] + tmp[3] * src[14] + tmp[4] * src[15];
    inv[2][0] -= tmp[1] * src[13] + tmp[2] * src[14] + tmp[5] * src[15];
    inv[2][1] = tmp[1] * src[12] + tmp[6] * src[14] + tmp[9] * src[15];
    inv[2][1] -= tmp[0] * src[12] + tmp[7] * src[14] + tmp[8] * src[15];
    inv[2][2] = tmp[2] * src[12] + tmp[7] * src[13] + tmp[10] * src[15];
    inv[2][2] -= tmp[3] * src[12] + tmp[6] * src[13] + tmp[11] * src[15];
    inv[2][3] = tmp[5] * src[12] + tmp[8] * src[13] + tmp[11] * src[14];
    inv[2][3] -= tmp[4] * src[12] + tmp[9] * src[13] + tmp[10] * src[14];
    inv[3][0] = tmp[2] * src[10] + tmp[5] * src[11] + tmp[1] * src[9];
    inv[3][0] -= tmp[4] * src[11] + tmp[0] * src[9] + tmp[3] * src[10];
    inv[3][1] = tmp[8] * src[11] + tmp[0] * src[8] + tmp[7] * src[10];
    inv[3][1] -= tmp[6] * src[10] + tmp[9] * src[11] + tmp[1] * src[8];
    inv[3][2] = tmp[6] * src[9] + tmp[11] * src[11] + tmp[3] * src[8];
    inv[3][2] -= tmp[10] * src[11] + tmp[2] * src[8] + tmp[7] * src[9];
    inv[3][3] = tmp[10] * src[10] + tmp[4] * src[8] + tmp[9] * src[9];
    inv[3][3] -= tmp[8] * src[9] + tmp[11] * src[10] + tmp[5] * src[8];

    det = src[0] * inv[0][0] + src[1] * inv[0][1] + src[2] * inv[0][2] + src[3] * inv[0][3];
    if det.abs() < 1e-10 {
        return None;
    }
    let det_inv = 1.0 / det;
    for row in 0..4 {
        for col in 0..4 {
            inv[row][col] *= det_inv;
        }
    }
    Some(inv)
}

fn transform_point(m: &[[f32; 4]; 4], v: &[f32; 4]) -> [f32; 3] {
    let w = m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3];
    let inv_w = if w.abs() > 1e-10 { 1.0 / w } else { 1.0 };
    [
        (m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3]) * inv_w,
        (m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3]) * inv_w,
        (m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3]) * inv_w,
    ]
}

#[allow(dead_code)]
fn reference_lines() -> (Vec<LineVertex>, u32) {
    // 测试参考线已移除
    (Vec::new(), 0)
}

// ── 矩阵运算（列优先，适配 wgpu / WGSL）───────────────────────────────

fn perspective(fov_rad: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_rad / 2.0).tan();
    let d = near - far;
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / d, -1.0],
        [0.0, 0.0, (near * far) / d, 0.0],
    ]
}

fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let fwd = normalize(sub(target, eye));
    let right = normalize(cross(fwd, up));
    let up2 = cross(right, fwd);
    [
        [right[0], up2[0], -fwd[0], 0.0],
        [right[1], up2[1], -fwd[1], 0.0],
        [right[2], up2[2], -fwd[2], 0.0],
        [-dot(right, eye), -dot(up2, eye), dot(fwd, eye), 1.0],
    ]
}

fn mul4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut m = [[0.0f32; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            m[c][r] = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
        }
    }
    m
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

// ── 预计算几何 ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PrecomputedGeometry {
    pub coarse_verts: Vec<u8>,
    pub coarse_indices: Vec<u32>,
    pub coarse_tri_count: u32,
    pub view_proj: [[f32; 4]; 4],
    pub line_endpoints: Vec<u8>,
    pub line_count: u32,
    /// 摄像机位置（用于 pick_sphere 和 finalize_snapshot）
    pub camera_eye: [f32; 3],
}

pub fn precompute_geometry(_elapsed_secs: f32) -> PrecomputedGeometry {
    let (verts, indices) = all_geometry(_elapsed_secs);

    // 从 GameplayState 获取摄像机状态
    let state_arc = get_gameplay_state();
    let mut gs_lock = state_arc.as_ref().map(|a| a.lock().unwrap());

    // 使用动态摄像机参数计算 view_proj
    let (vp, cam_eye, _aspect) = if let Some(ref gs) = gs_lock {
        let cam = &gs.camera;
        let aspect = if gs.screen_size.1 > 0.0 {
            gs.screen_size.0 / gs.screen_size.1
        } else {
            16.0 / 9.0
        };
        (cam.view_proj(aspect), cam.eye, aspect)
    } else {
        // 回退：使用默认摄像机
        let default_cam = CameraState {
            eye: [35.0, 25.0, 35.0],
            direction: normalize(sub([0.0, 0.0, 0.0], [35.0, 25.0, 35.0])),
            up: [0.0, 1.0, 0.0],
            fov_degrees: 60.0,
        };
        (default_cam.view_proj(16.0 / 9.0), default_cam.eye, 16.0 / 9.0)
    };

    let tri_count = (indices.len() / 3) as u32;

    // 从玩法状态生成九宫格线
    let pick: Option<(f32, f32)>;
    if let Some(ref mut gs) = gs_lock {
        if gs.camera_mode == CameraMode::Playing {
            // 正常游戏：根据鼠标位置更新九宫格
            if let Some(ms) = gs.mouse_screen {
                gs.picked_spherical = pick_sphere(
                    ms,
                    gs.screen_size,
                    &vp,
                    cam_eye,
                    gs.sphere_radius,
                );
            }
        }
        // Adjusting 模式：保持 picked_spherical 不变（九宫格冻结）
        pick = gs.picked_spherical;
    } else {
        pick = None;
    }

    // 鼠标未命中球面时不显示九宫格
    let (line_verts, line_count) = if let Some(center) = pick {
        let cell_angular = 12.0_f32.to_radians();
        let (gv, lc) = judgment_grid_lines(
            center,
            cell_angular,
            20.0,
            [1.0, 1.0, 1.0, 0.8],
        );
        (gv, lc)
    } else {
        (Vec::new(), 0)
    };
    PrecomputedGeometry {
        coarse_verts: bytemuck::cast_slice(&verts).to_vec(),
        coarse_indices: indices,
        coarse_tri_count: tri_count,
        view_proj: vp,
        line_endpoints: bytemuck::cast_slice(&line_verts).to_vec(),
        line_count,
        camera_eye: cam_eye,
    }
}

pub fn finalize_snapshot(
    geo: &PrecomputedGeometry,
    screen_w: u32,
    screen_h: u32,
) -> ComputeSnapshot {
    let p = SubdivideParams {
        view_proj: geo.view_proj,
        camera_eye: geo.camera_eye,
        _pad2: 0.0,
        triangle_count: geo.coarse_tri_count,
        sub_grid_size: SUB_GRID_SIZE,
        screen_width: screen_w as f32,
        screen_height: screen_h as f32,
    };
    ComputeSnapshot {
        vertex_data: geo.coarse_verts.clone(),
        indices: geo.coarse_indices.clone(),
        uniform_data: bytemuck::bytes_of(&p).to_vec(),
        triangle_count: geo.coarse_tri_count,
    }
}

pub fn compute_lines_snapshot(
    geo: &PrecomputedGeometry,
    screen_w: u32,
    screen_h: u32,
) -> (Vec<u8>, u32, Vec<u8>) {
    let mut uf = bytemuck::bytes_of(&geo.view_proj).to_vec();
    uf.extend_from_slice(&geo.line_count.to_le_bytes());
    uf.extend_from_slice(&0u32.to_le_bytes());
    uf.extend_from_slice(&screen_w.to_le_bytes());
    uf.extend_from_slice(&screen_h.to_le_bytes());
    (geo.line_endpoints.clone(), geo.line_count, uf)
}

// ── 异步快照生产者 ────────────────────────────────────────────────────

use std::time::Instant;
use tokio::sync::watch;

pub struct AsyncSnapshotProducer {
    rx: watch::Receiver<Option<PrecomputedGeometry>>,
    _tx: watch::Sender<Option<PrecomputedGeometry>>,
}

impl AsyncSnapshotProducer {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(None);
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let start = Instant::now();
            loop {
                let geo = precompute_geometry(start.elapsed().as_secs_f32());
                if tx2.send(Some(geo)).is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        });
        AsyncSnapshotProducer { rx, _tx: tx }
    }
    pub fn latest(&self, screen_w: u32, screen_h: u32) -> ComputeSnapshot {
        match &*self.rx.borrow() {
            Some(geo) => finalize_snapshot(geo, screen_w, screen_h),
            None => ComputeSnapshot::empty(),
        }
    }
    pub fn latest_compute_lines(&self, screen_w: u32, screen_h: u32) -> (Vec<u8>, u32, Vec<u8>) {
        match &*self.rx.borrow() {
            Some(geo) => compute_lines_snapshot(geo, screen_w, screen_h),
            None => (Vec::new(), 0, Vec::new()),
        }
    }
}

static SNAPSHOT_PRODUCER: std::sync::OnceLock<Arc<AsyncSnapshotProducer>> =
    std::sync::OnceLock::new();
pub fn set_snapshot_producer(p: Arc<AsyncSnapshotProducer>) {
    let _ = SNAPSHOT_PRODUCER.set(p);
}
pub fn latest_snapshot(screen_w: u32, screen_h: u32) -> ComputeSnapshot {
    SNAPSHOT_PRODUCER
        .get()
        .map(|p| p.latest(screen_w, screen_h))
        .unwrap_or_else(ComputeSnapshot::empty)
}
pub fn latest_compute_lines_snapshot(screen_w: u32, screen_h: u32) -> (Vec<u8>, u32, Vec<u8>) {
    SNAPSHOT_PRODUCER
        .get()
        .map(|p| p.latest_compute_lines(screen_w, screen_h))
        .unwrap_or_else(|| (Vec::new(), 0, Vec::new()))
}

// ── GameplayRenderPipeline：实现 RenderPipeline trait ──────────────────

/// 持有所有 gameplay 渲染状态（compute/subdivide/line 的 buffer 池和管线缓存）。
/// 实现 `musesp_ui::renderer::RenderPipeline`，由 musesp_ui 在合适的帧时机调用。
pub struct GameplayRenderPipeline {
    shader_library: Arc<musesp_config::shader_library::ShaderLibrary>,

    // Compute 管线缓存
    compute_pipelines: HashMap<u64, wgpu::ComputePipeline>,
    compute_bgls: HashMap<u64, wgpu::BindGroupLayout>,
    _display_pipelines: HashMap<u64, wgpu::RenderPipeline>,
    _display_bgls: HashMap<u64, wgpu::BindGroupLayout>,

    // Compute 输出 framebuffer
    _framebuffer: Option<wgpu::Buffer>,
    _fb_capacity: u32,

    // 双缓冲 compute buffers
    comp_vertex_bufs: [Option<wgpu::Buffer>; 2],
    comp_vertex_caps: [u64; 2],
    comp_index_bufs: [Option<wgpu::Buffer>; 2],
    comp_index_caps: [u64; 2],
    comp_uniform_bufs: [Option<wgpu::Buffer>; 2],
    comp_frame_idx: usize,

    // Subdivide 输出
    sub_vtx_bufs: [Option<wgpu::Buffer>; 2],
    sub_vtx_caps: [u64; 2],
    sub_idx_bufs: [Option<wgpu::Buffer>; 2],
    sub_idx_caps: [u64; 2],

    // Eval 输出
    primary_bufs: [Option<wgpu::Buffer>; 2],
    primary_caps: [u64; 2],

    // Subdivide→Render 管线缓存
    _subdiv_render_pipeline: Option<wgpu::RenderPipeline>,
    _subdiv_render_key: u64,
    _subdiv_render_bgl: Option<wgpu::BindGroupLayout>,

    // camera_eye uniform buffer
    _camera_eye_buf: Option<wgpu::Buffer>,

    // 全屏参数 uniform
    _fs_uniform_bufs: [Option<wgpu::Buffer>; 2],

    // 3D 直线管线缓存
    _line_pipeline: Option<wgpu::RenderPipeline>,
    _line_pipeline_key: u64,
    _line_bgl: Option<wgpu::BindGroupLayout>,
    line_vtx_bufs: [Option<wgpu::Buffer>; 2],
    line_vtx_caps: [u64; 2],
    _line_idx_bufs: [Option<wgpu::Buffer>; 2],
    _line_idx_caps: [u64; 2],
    line_uniform_bufs: [Option<wgpu::Buffer>; 2],
    line_frame_idx: usize,

    // Compute Lines 输出缓冲
    line_sub_vtx_bufs: [Option<wgpu::Buffer>; 2],
    line_sub_vtx_caps: [u64; 2],
    line_sub_idx_bufs: [Option<wgpu::Buffer>; 2],
    line_sub_idx_caps: [u64; 2],

    // Compute Lines eval primary buffer
    line_primary_bufs: [Option<wgpu::Buffer>; 2],
    line_primary_caps: [u64; 2],

    // Compute Lines 管线缓存
    line_sub_comp_pipeline: Option<wgpu::ComputePipeline>,
    line_sub_comp_key: u64,
    line_sub_comp_bgl: Option<wgpu::BindGroupLayout>,
    line_final_comp_pipeline: Option<wgpu::ComputePipeline>,
    line_final_comp_key: u64,
    line_final_comp_bgl: Option<wgpu::BindGroupLayout>,
    _line_sub_render_pipeline: Option<wgpu::RenderPipeline>,
    _line_sub_render_key: u64,
    _line_sub_render_bgl: Option<wgpu::BindGroupLayout>,
}

impl GameplayRenderPipeline {
    pub fn new(shader_library: Arc<musesp_config::shader_library::ShaderLibrary>) -> Self {
        GameplayRenderPipeline {
            shader_library,
            compute_pipelines: HashMap::new(),
            compute_bgls: HashMap::new(),
            _display_pipelines: HashMap::new(),
            _display_bgls: HashMap::new(),
            _framebuffer: None,
            _fb_capacity: 0,
            comp_vertex_bufs: [None, None],
            comp_vertex_caps: [0, 0],
            comp_index_bufs: [None, None],
            comp_index_caps: [0, 0],
            comp_uniform_bufs: [None, None],
            comp_frame_idx: 0,
            sub_vtx_bufs: [None, None],
            sub_vtx_caps: [0, 0],
            sub_idx_bufs: [None, None],
            sub_idx_caps: [0, 0],
            primary_bufs: [None, None],
            primary_caps: [0, 0],
            _subdiv_render_pipeline: None,
            _subdiv_render_key: 0,
            _subdiv_render_bgl: None,
            _camera_eye_buf: None,
            _fs_uniform_bufs: [None, None],
            _line_pipeline: None,
            _line_pipeline_key: 0,
            _line_bgl: None,
            line_vtx_bufs: [None, None],
            line_vtx_caps: [0, 0],
            _line_idx_bufs: [None, None],
            _line_idx_caps: [0, 0],
            line_uniform_bufs: [None, None],
            line_frame_idx: 0,
            line_sub_vtx_bufs: [None, None],
            line_sub_vtx_caps: [0, 0],
            line_sub_idx_bufs: [None, None],
            line_sub_idx_caps: [0, 0],
            line_primary_bufs: [None, None],
            line_primary_caps: [0, 0],
            line_sub_comp_pipeline: None,
            line_sub_comp_key: 0,
            line_sub_comp_bgl: None,
            line_final_comp_pipeline: None,
            line_final_comp_key: 0,
            line_final_comp_bgl: None,
            _line_sub_render_pipeline: None,
            _line_sub_render_key: 0,
            _line_sub_render_bgl: None,
        }
    }
}

impl RenderPipeline for GameplayRenderPipeline {
    fn record_compute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        config: &wgpu::SurfaceConfiguration,
        _sample_count: u32,
    ) {
        let screen_w = config.width;
        let screen_h = config.height;

        // ── 曲面自适应细分（两-pass compute）──
        let se = self.shader_library.get("surface_eval").clone();
        let sf = self.shader_library.get("surface_final").clone();
        let snap = latest_snapshot(screen_w, screen_h);
        if snap.triangle_count > 0 {
            self.run_adaptive_subdivide(device, queue, encoder, &se, &sf, &snap);
        }

        // ── 线段自适应细分（两-pass compute）──
        let le = self.shader_library.get("line_eval").clone();
        let lf = self.shader_library.get("line_final").clone();
        let (endpoints, line_count, uniform) = latest_compute_lines_snapshot(screen_w, screen_h);
        if line_count > 0 {
            self.compute_line_subdivide(
                device,
                queue,
                encoder,
                &le,
                &lf,
                endpoints.as_slice(),
                line_count,
                uniform.as_slice(),
            );
        }
    }

    fn record_render<'rp>(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rp: &mut wgpu::RenderPass<'rp>,
        config: &wgpu::SurfaceConfiguration,
        sample_count: u32,
    ) {
        let screen_w = config.width;
        let screen_h = config.height;

        // ── 曲面细分渲染 ──
        let p2 = self.shader_library.get("surface_pass2");
        let snap = latest_snapshot(screen_w, screen_h);
        if snap.triangle_count > 0 {
            self.draw_subdivided(
                rp,
                device,
                queue,
                config,
                sample_count,
                p2.as_ref(),
                p2.as_ref(),
                &snap,
            );
        }

        // ── 线段细分渲染 ──
        let lr = self.shader_library.get("line_render");
        let (_endpoints, line_count, uniform) = latest_compute_lines_snapshot(screen_w, screen_h);
        if line_count > 0 {
            self.draw_compute_lines(
                rp,
                device,
                queue,
                config,
                sample_count,
                lr.as_ref(),
                lr.as_ref(),
                &[],
                line_count,
                uniform.as_slice(),
            );
        }
    }
}

// ── 内部实现（从 WgpuRenderer 迁移）───────────────────────────────────

impl GameplayRenderPipeline {
    fn ensure_comp_vertex_buf(&mut self, device: &wgpu::Device, idx: usize, capacity: u64) {
        if self.comp_vertex_caps[idx] >= capacity {
            return;
        }
        let size = capacity.max(256).next_power_of_two();
        self.comp_vertex_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comp_vertices"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.comp_vertex_caps[idx] = size;
    }

    fn ensure_comp_index_buf(&mut self, device: &wgpu::Device, idx: usize, capacity: u64) {
        if self.comp_index_caps[idx] >= capacity {
            return;
        }
        let size = capacity.max(256).next_power_of_two();
        self.comp_index_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comp_indices"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.comp_index_caps[idx] = size;
    }

    fn ensure_comp_uniform_buf(&mut self, device: &wgpu::Device, idx: usize, capacity: u64) {
        if self.comp_uniform_bufs[idx].is_some() {
            return;
        }
        let size = capacity.max(128).next_power_of_two();
        self.comp_uniform_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comp_params"),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }

    fn ensure_primary_buf(&mut self, device: &wgpu::Device, idx: usize, capacity: u64) {
        if self.primary_caps[idx] >= capacity {
            return;
        }
        let size = capacity.max(256).next_power_of_two();
        self.primary_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("primary_vtx"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));
        self.primary_caps[idx] = size;
    }

    fn ensure_sub_vtx_buf(&mut self, device: &wgpu::Device, idx: usize, capacity: u64) {
        if self.sub_vtx_caps[idx] >= capacity {
            return;
        }
        let size = capacity.max(256).next_power_of_two();
        self.sub_vtx_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sub_vtx"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.sub_vtx_caps[idx] = size;
    }

    fn ensure_sub_idx_buf(&mut self, device: &wgpu::Device, idx: usize, capacity: u64) {
        if self.sub_idx_caps[idx] >= capacity {
            return;
        }
        let size = capacity.max(256).next_power_of_two();
        self.sub_idx_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sub_idx"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.sub_idx_caps[idx] = size;
    }

    fn ensure_line_sub_bufs(
        &mut self,
        device: &wgpu::Device,
        idx: usize,
        vtx_cap: u64,
        idx_cap: u64,
    ) {
        if self.line_sub_vtx_caps[idx] < vtx_cap {
            let size = vtx_cap.max(256).next_power_of_two();
            self.line_sub_vtx_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line_sub_vtx"),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.line_sub_vtx_caps[idx] = size;
        }
        if self.line_sub_idx_caps[idx] < idx_cap {
            let size = idx_cap.max(256).next_power_of_two();
            self.line_sub_idx_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line_sub_idx"),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.line_sub_idx_caps[idx] = size;
        }
    }

    fn get_or_create_compute_pipeline_from_module(
        &mut self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        cache_key: u64,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
        if let Some(pipeline) = self.compute_pipelines.get(&cache_key) {
            let bgl = self.compute_bgls.get(&cache_key).unwrap().clone();
            return (pipeline.clone(), bgl);
        }

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute_bgl"),
            entries: COMPUTE_BGL_ENTRIES,
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compute_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute_pipeline"),
            layout: Some(&layout),
            module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        self.compute_pipelines.insert(cache_key, pipeline.clone());
        self.compute_bgls.insert(cache_key, bgl.clone());
        (pipeline, bgl)
    }

    // ── 自适应曲面细分 ─────────────────────────────────────────────

    fn run_adaptive_subdivide(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        eval_module: &Arc<wgpu::ShaderModule>,
        final_module: &Arc<wgpu::ShaderModule>,
        snap: &ComputeSnapshot,
    ) {
        let idx = self.comp_frame_idx;

        let primary_size = snap.triangle_count as u64 * PRIMARY_VERTICES_PER_TRIANGLE as u64 * 64;
        self.ensure_primary_buf(device, idx, primary_size);

        let nv = snap.triangle_count as u64 * MAX_VERTICES_PER_TRIANGLE as u64 * 64;
        let ni = snap.triangle_count as u64 * MAX_INDICES_PER_TRIANGLE as u64 * 4;
        self.ensure_sub_vtx_buf(device, idx, nv);
        self.ensure_sub_idx_buf(device, idx, ni);

        self.ensure_comp_vertex_buf(device, idx, snap.vertex_data.len() as u64);
        queue.write_buffer(
            self.comp_vertex_bufs[idx].as_ref().unwrap(),
            0,
            &snap.vertex_data,
        );
        let ib = bytemuck::cast_slice(&snap.indices);
        self.ensure_comp_index_buf(device, idx, ib.len() as u64);
        queue.write_buffer(self.comp_index_bufs[idx].as_ref().unwrap(), 0, ib);
        self.ensure_comp_uniform_buf(device, idx, snap.uniform_data.len() as u64);
        queue.write_buffer(
            self.comp_uniform_bufs[idx].as_ref().unwrap(),
            0,
            &snap.uniform_data,
        );

        // Pass 1: Eval
        let module_key = Arc::as_ptr(eval_module) as u64;
        let (ep, ebgl) =
            self.get_or_create_compute_pipeline_from_module(device, eval_module, module_key);
        let ebg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("surf_eval_bg"),
            layout: &ebgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.comp_vertex_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.comp_index_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.comp_uniform_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.primary_bufs[idx].as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.sub_vtx_bufs[idx].as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.sub_vtx_bufs[idx].as_ref().unwrap().as_entire_binding(),
                },
            ],
        });
        {
            let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("surf_eval"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&ep);
            cp.set_bind_group(0, &ebg, &[]);
            cp.dispatch_workgroups(
                (snap.triangle_count * PRIMARY_VERTICES_PER_TRIANGLE + EVAL_WORKGROUP_SIZE - 1)
                    / EVAL_WORKGROUP_SIZE,
                1,
                1,
            );
        }

        // Pass 2: Final
        let fp_key = Arc::as_ptr(final_module) as u64;
        let (fp, fbgl) =
            self.get_or_create_compute_pipeline_from_module(device, final_module, fp_key);
        let fbg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("surf_final_bg"),
            layout: &fbgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.comp_vertex_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.comp_index_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.comp_uniform_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.primary_bufs[idx].as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.sub_vtx_bufs[idx].as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.sub_idx_bufs[idx].as_ref().unwrap().as_entire_binding(),
                },
            ],
        });
        {
            let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("surf_final"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&fp);
            cp.set_bind_group(0, &fbg, &[]);
            cp.dispatch_workgroups(snap.triangle_count, 1, 1);
        }

        self.comp_frame_idx ^= 1;
    }

    fn draw_subdivided(
        &self,
        rp: &mut wgpu::RenderPass<'_>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
        sample_count: u32,
        vertex_module: &wgpu::ShaderModule,
        fragment_module: &wgpu::ShaderModule,
        snap: &ComputeSnapshot,
    ) {
        let idx = self.comp_frame_idx ^ 1;
        let vtx_buf = match self.sub_vtx_bufs[idx].as_ref() {
            Some(b) => b,
            None => return,
        };
        let idx_buf = match self.sub_idx_bufs[idx].as_ref() {
            Some(b) => b,
            None => return,
        };
        let format = config.format;
        let eye_bytes = &snap.uniform_data[64..80];

        // Pipeline caching via key stored in (&self) — we use the fact that
        // record_render borrows &self immutably, so subdiv_render_* fields need
        // interior mutability. Since wgpu types are Send+Sync but not Clone,
        // we create pipelines each frame for simplicity. For performance,
        // a OnceLock or similar could be used.
        //
        // For now, we rely on shader library providing pre-compiled modules;
        // the BGL/pipeline creation is cheap on modern drivers.

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("subdiv_fs_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("subdiv_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("subdiv_render"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                    0 => Float32x4,
                    1 => Float32x4,
                    2 => Float32x4,
                    3 => Float32x4,
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: fragment_module,
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
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let eye_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera_eye"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&eye_buf, 0, eye_bytes);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("subdiv_fs_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: eye_buf.as_entire_binding(),
            }],
        });

        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &bind_group, &[]);
        rp.set_vertex_buffer(0, vtx_buf.slice(..));
        rp.set_index_buffer(idx_buf.slice(..), wgpu::IndexFormat::Uint32);
        let max_index_count = snap.triangle_count * MAX_INDICES_PER_TRIANGLE;
        rp.draw_indexed(0..max_index_count, 0, 0..1);
    }

    // ── 线段自适应细分 ─────────────────────────────────────────────

    fn compute_line_subdivide(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        eval_module: &Arc<wgpu::ShaderModule>,
        final_module: &Arc<wgpu::ShaderModule>,
        endpoint_data: &[u8],
        line_count: u32,
        uniform_data: &[u8],
    ) {
        let idx = self.line_frame_idx;

        // Ensure endpoint buffer
        let ep_len = endpoint_data.len() as u64;
        if self.line_vtx_bufs[idx]
            .as_ref()
            .map_or(true, |b| b.size() < ep_len)
        {
            self.line_vtx_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line_ep"),
                size: ep_len.max(256),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            self.line_vtx_caps[idx] = ep_len.max(256);
        }
        queue.write_buffer(self.line_vtx_bufs[idx].as_ref().unwrap(), 0, endpoint_data);

        // Ensure uniform buffer
        let uf_len = uniform_data.len() as u64;
        if self.line_uniform_bufs[idx]
            .as_ref()
            .map_or(true, |b| b.size() < uf_len)
        {
            self.line_uniform_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line_uf"),
                size: uf_len.max(128),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(
            self.line_uniform_bufs[idx].as_ref().unwrap(),
            0,
            uniform_data,
        );

        // Ensure primary buffer
        let primary_size = line_count as u64 * PRIMARY_VERTICES_PER_LINE as u64 * 32;
        if self.line_primary_caps[idx] < primary_size {
            let size = primary_size.max(256).next_power_of_two();
            self.line_primary_bufs[idx] = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line_primary"),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            self.line_primary_caps[idx] = size;
        }

        let vtx_size = line_count as u64 * MAX_VERTICES_PER_LINE as u64 * 32;
        let idx_size = line_count as u64 * MAX_INDICES_PER_LINE as u64 * 4;
        self.ensure_line_sub_bufs(device, idx, vtx_size, idx_size);

        // Pass 1: Eval
        let ep_key = Arc::as_ptr(eval_module) as u64;
        if self.line_sub_comp_pipeline.is_none() || self.line_sub_comp_key != ep_key {
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("line_eval_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("line_eval_layout"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("line_eval"),
                layout: Some(&layout),
                module: eval_module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.line_sub_comp_pipeline = Some(pipeline);
            self.line_sub_comp_bgl = Some(bgl);
            self.line_sub_comp_key = ep_key;
        }

        let ebg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line_eval_bg"),
            layout: self.line_sub_comp_bgl.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.line_vtx_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.line_uniform_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.line_primary_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
            ],
        });
        {
            let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("line_eval"),
                timestamp_writes: None,
            });
            cp.set_pipeline(self.line_sub_comp_pipeline.as_ref().unwrap());
            cp.set_bind_group(0, &ebg, &[]);
            cp.dispatch_workgroups(
                (line_count * PRIMARY_VERTICES_PER_LINE + EVAL_WORKGROUP_SIZE - 1)
                    / EVAL_WORKGROUP_SIZE,
                1,
                1,
            );
        }

        // Pass 2: Final
        let fp_key = Arc::as_ptr(final_module) as u64;
        if self.line_final_comp_pipeline.is_none() || self.line_final_comp_key != fp_key {
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("line_final_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("line_final_layout"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("line_final_pipeline"),
                layout: Some(&layout),
                module: final_module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.line_final_comp_pipeline = Some(pipeline);
            self.line_final_comp_bgl = Some(bgl);
            self.line_final_comp_key = fp_key;
        }
        let fbg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line_final_bg"),
            layout: self.line_final_comp_bgl.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.line_vtx_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.line_uniform_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.line_primary_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.line_sub_vtx_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.line_sub_idx_bufs[idx]
                        .as_ref()
                        .unwrap()
                        .as_entire_binding(),
                },
            ],
        });
        {
            let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("line_final"),
                timestamp_writes: None,
            });
            cp.set_pipeline(self.line_final_comp_pipeline.as_ref().unwrap());
            cp.set_bind_group(0, &fbg, &[]);
            cp.dispatch_workgroups(line_count, 1, 1);
        }
    }

    fn draw_compute_lines(
        &self,
        rp: &mut wgpu::RenderPass<'_>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
        sample_count: u32,
        vertex_module: &wgpu::ShaderModule,
        fragment_module: &wgpu::ShaderModule,
        _endpoint_data: &[u8],
        line_count: u32,
        uniform_data: &[u8],
    ) {
        let idx = self.line_frame_idx;
        let vtx_buf = match self.line_sub_vtx_bufs[idx].as_ref() {
            Some(b) => b,
            None => return,
        };
        let idx_buf = match self.line_sub_idx_bufs[idx].as_ref() {
            Some(b) => b,
            None => return,
        };
        let format = config.format;

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("line_sub_render_bgl"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("line_sub_render_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line_sub_render"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 1,
                        },
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: fragment_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let uf_len = uniform_data.len() as u64;
        let uf_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line_sub_render_uf"),
            size: uf_len.max(64),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uf_buf, 0, uniform_data);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line_sub_render_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uf_buf.as_entire_binding(),
            }],
        });

        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &bg, &[]);
        rp.set_vertex_buffer(0, vtx_buf.slice(..));
        rp.set_index_buffer(idx_buf.slice(..), wgpu::IndexFormat::Uint32);
        let index_count = line_count * MAX_INDICES_PER_LINE;
        rp.draw_indexed(0..index_count, 0, 0..1);
    }
}
