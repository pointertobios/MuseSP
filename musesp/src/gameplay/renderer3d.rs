//! 3D 渲染管线 —— 摄像机参数写死为常量，场景中包含一个旋转正方体。
//!
//! 使用 `precompute_geometry()` + `finalize_snapshot()` 构建 compute 管线快照。
//! 推荐通过 `AsyncSnapshotProducer` 在后台异步预计算几何数据。

use musesp_ui::renderer::ComputeSnapshot;
use std::f32::consts::PI;
use std::sync::Arc;

// ── 摄像机常量 ────────────────────────────────────────────────────────

/// 摄像机世界位置
const CAMERA_EYE: [f32; 3] = [0.0, 0.0, 5.0];
/// 摄像机注视目标
const CAMERA_TARGET: [f32; 3] = [0.0, 0.0, 0.0];
/// 世界 Up 向量
const CAMERA_UP: [f32; 3] = [0.0, 1.0, 0.0];
/// 垂直 FOV（度）
const FOV_DEGREES: f32 = 60.0;
/// 近裁剪面
const NEAR: f32 = 0.1;
/// 远裁剪面
const FAR: f32 = 100.0;
/// 屏幕宽高比（16:9）
const ASPECT: f32 = 16.0 / 9.0;

// ── Uniform 类型 ──────────────────────────────────────────────────────

/// 传给 compute shader 的 Params（与 WGSL Params 布局一致，80 字节）：
///   offset  0: view_proj      (mat4x4, 64B)
///   offset 64: triangle_count (u32,   4B)
///   offset 68: screen_width   (f32,   4B)
///   offset 72: screen_height  (f32,   4B)
///   offset 76: _pad           (u32,   4B)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ComputeParams {
    view_proj: [[f32; 4]; 4],
    triangle_count: u32,
    screen_width: f32,
    screen_height: f32,
    _pad: u32,
}

// ── 顶点类型 ──────────────────────────────────────────────────────────

/// 3D 顶点：位置 (vec3) + 填充 + 颜色 (vec4)，共 32 字节。
/// 布局匹配 WGSL compute shader 的 Vertex 结构体。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex3D {
    pub position: [f32; 3],
    _pad: f32,
    pub color: [f32; 4],
}

// ── 正方体几何数据 ────────────────────────────────────────────────────

/// 生成单位正方体的 8 个顶点（每面不同颜色，便于观察旋转）。
fn cube_vertices() -> Vec<Vertex3D> {
    // 6 个面分别着色：红、绿、蓝、黄、青、品红
    let colors: [[f32; 4]; 6] = [
        [1.0, 0.0, 0.0, 0.6], // +X 红
        [0.0, 1.0, 0.0, 0.6], // -X 绿
        [0.0, 0.0, 1.0, 0.6], // +Y 蓝
        [1.0, 1.0, 0.0, 0.6], // -Y 黄
        [0.0, 1.0, 1.0, 0.6], // +Z 青
        [1.0, 0.0, 1.0, 0.6], // -Z 品红
    ];

    // 半边长 1.0
    let h = 1.0f32;

    // 24 个顶点（每面 4 个，非共享）
    vec![
        // +X 面 (右)  —— 红
        v([h, h, h], colors[0]),
        v([h, -h, h], colors[0]),
        v([h, -h, -h], colors[0]),
        v([h, h, -h], colors[0]),
        // -X 面 (左)  —— 绿
        v([-h, h, -h], colors[1]),
        v([-h, -h, -h], colors[1]),
        v([-h, -h, h], colors[1]),
        v([-h, h, h], colors[1]),
        // +Y 面 (上)  —— 蓝
        v([-h, h, -h], colors[2]),
        v([-h, h, h], colors[2]),
        v([h, h, h], colors[2]),
        v([h, h, -h], colors[2]),
        // -Y 面 (下)  —— 黄
        v([-h, -h, h], colors[3]),
        v([-h, -h, -h], colors[3]),
        v([h, -h, -h], colors[3]),
        v([h, -h, h], colors[3]),
        // +Z 面 (前)  —— 青
        v([-h, h, h], colors[4]),
        v([-h, -h, h], colors[4]),
        v([h, -h, h], colors[4]),
        v([h, h, h], colors[4]),
        // -Z 面 (后)  —— 品红
        v([h, h, -h], colors[5]),
        v([h, -h, -h], colors[5]),
        v([-h, -h, -h], colors[5]),
        v([-h, h, -h], colors[5]),
    ]
}

fn v(pos: [f32; 3], color: [f32; 4]) -> Vertex3D {
    Vertex3D {
        position: pos,
        _pad: 0.0,
        color,
    }
}

/// 正方体索引（6 面 × 2 三角形 × 3 = 36 个索引）。
fn cube_indices() -> Vec<u32> {
    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
    }
    indices
}

// ── 坐标轴几何数据（细长四边形，TriangleList 渲染） ────────────────────

const AXIS_LENGTH: f32 = 2.5;
const AXIS_HALF_THICKNESS: f32 = 0.025;

/// X / Y / Z 正半轴：3 条 × 4 顶点 × 2 三角形 = 12 顶点 + 18 索引。
fn axis_vertices() -> Vec<Vertex3D> {
    let t = AXIS_HALF_THICKNESS;
    let l = AXIS_LENGTH;
    let red: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
    let green: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
    let blue: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

    vec![
        // X 轴（红）—— XY 平面上的水平细条
        v([0.0, -t, 0.0], red),
        v([0.0, t, 0.0], red),
        v([l, t, 0.0], red),
        v([l, -t, 0.0], red),
        // Y 轴（绿）—— XY 平面上的竖直细条
        v([-t, 0.0, 0.0], green),
        v([t, 0.0, 0.0], green),
        v([t, l, 0.0], green),
        v([-t, l, 0.0], green),
        // Z 轴（蓝）—— YZ 平面上的纵深细条
        v([0.0, -t, 0.0], blue),
        v([0.0, t, 0.0], blue),
        v([0.0, t, l], blue),
        v([0.0, -t, l], blue),
    ]
}

fn axis_indices(base: u32) -> Vec<u32> {
    let mut idx = Vec::with_capacity(18);
    for face in 0..3u32 {
        let b = base + face * 4;
        idx.push(b);
        idx.push(b + 1);
        idx.push(b + 2);
        idx.push(b);
        idx.push(b + 2);
        idx.push(b + 3);
    }
    idx
}

// ── 矩阵运算 ──────────────────────────────────────────────────────────

/// 透视投影矩阵（wgpu NDC: Y 向上, Z ∈ [0,1]）。
fn perspective(fov_rad: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_rad / 2.0).tan();
    let d = near - far; // = -(far - near)
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / d, (near * far) / d],
        [0.0, 0.0, -1.0, 0.0],
    ]
}

/// Look-at 视图矩阵（行优先）。
///
/// V = R^T * T(-eye)，将世界坐标变换到相机空间。
fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let fwd = normalize(sub(target, eye));
    let right = normalize(cross(fwd, up));
    let up2 = cross(right, fwd);

    // R^T: 行 = 相机基向量
    // 位移 = -R^T * eye = [-dot(right,eye), -dot(up2,eye), dot(fwd,eye)]
    [
        [right[0], right[1], right[2], -dot(right, eye)],
        [up2[0], up2[1], up2[2], -dot(up2, eye)],
        [-fwd[0], -fwd[1], -fwd[2], dot(fwd, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mul4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut m = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    m
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / len, v[1] / len, v[2] / len]
}

/// 转置 4×4 矩阵。
/// Rust 行优先 → WGSL 列优先（mat4x4 按列存储），发送前需转置。
fn transpose(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut t = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            t[i][j] = m[j][i];
        }
    }
    t
}

// ── Compute 管线接口 ─────────────────────────────────────────────────

/// 预计算的几何数据（不含屏幕尺寸）。
/// 后台 tokio 任务持续计算此结构，渲染线程补上屏幕参数即可使用。
#[derive(Clone)]
pub struct PrecomputedGeometry {
    pub vertex_data: Vec<u8>,
    pub indices: Vec<u32>,
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub view_proj: [[f32; 4]; 4],
}

/// 仅计算与屏幕尺寸无关的几何数据（顶点旋转 + view_proj）。
pub fn precompute_geometry(elapsed_secs: f32) -> PrecomputedGeometry {
    let mut vertices = cube_vertices();
    let axis_base = vertices.len() as u32;
    vertices.extend(axis_vertices());

    let cube_idx = cube_indices();
    let axis_idx = axis_indices(axis_base);
    let mut all_indices = cube_idx;
    all_indices.extend(axis_idx);

    let triangle_count = (all_indices.len() / 3) as u32;

    // CPU 旋转正方体顶点（绕 Y 轴），坐标轴不动
    let angle = elapsed_secs * 0.3 * TAU;
    let (sin_a, cos_a) = angle.sin_cos();
    for v in &mut vertices[..axis_base as usize] {
        let x = v.position[0] * cos_a + v.position[2] * sin_a;
        let z = -v.position[0] * sin_a + v.position[2] * cos_a;
        v.position[0] = x;
        v.position[2] = z;
    }

    let proj = perspective(FOV_DEGREES.to_radians(), ASPECT, NEAR, FAR);
    let view = look_at(CAMERA_EYE, CAMERA_TARGET, CAMERA_UP);
    let view_proj = mul4(&proj, &view);

    PrecomputedGeometry {
        vertex_data: bytemuck::cast_slice(&vertices).to_vec(),
        indices: all_indices,
        vertex_count: vertices.len() as u32,
        triangle_count,
        view_proj: transpose(&view_proj),
    }
}

/// 用预计算的几何 + 当前屏幕尺寸拼装完整 ComputeSnapshot。
pub fn finalize_snapshot(
    geo: &PrecomputedGeometry,
    screen_w: u32,
    screen_h: u32,
) -> ComputeSnapshot {
    let params = ComputeParams {
        view_proj: geo.view_proj,
        triangle_count: geo.triangle_count,
        screen_width: screen_w as f32,
        screen_height: screen_h as f32,
        _pad: 0,
    };

    ComputeSnapshot {
        vertex_data: geo.vertex_data.clone(),
        indices: geo.indices.clone(),
        uniform_data: bytemuck::bytes_of(&params).to_vec(),
        vertex_count: geo.vertex_count,
        triangle_count: geo.triangle_count,
    }
}

// ── 异步快照生产者 ────────────────────────────────────────────────────

use std::time::Instant;
use tokio::sync::watch;

/// 基于 tokio 后台任务的快照预计算器。
///
/// `tokio::spawn` 异步任务持续调用 `precompute_geometry()`，
/// 将结果通过 `watch` channel 发送。渲染线程调用 `latest()` 时
/// 无阻塞读取最新预计算结果。
///
/// 通过 `Arc` 共享，可在 `compute_draw_fn` 闭包中使用。
pub struct AsyncSnapshotProducer {
    rx: watch::Receiver<Option<PrecomputedGeometry>>,
    _tx: watch::Sender<Option<PrecomputedGeometry>>,
}

impl AsyncSnapshotProducer {
    /// 启动后台预计算任务（`tokio::spawn`，非阻塞池）。
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(None);
        let tx2 = tx.clone();

        tokio::spawn(async move {
            let start = Instant::now();
            loop {
                let elapsed = start.elapsed().as_secs_f32();
                let geo = precompute_geometry(elapsed);
                if tx2.send(Some(geo)).is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        });

        AsyncSnapshotProducer { rx, _tx: tx }
    }

    /// 同步读取最新预计算结果，补上屏幕尺寸后返回绘制命令所需数据。
    ///
    /// `watch::Receiver::borrow()` 内部已同步，`&self` 即可。
    /// 如果后台任务尚未产出第一帧，返回空的 snapshot。
    pub fn latest(&self, screen_w: u32, screen_h: u32) -> ComputeSnapshot {
        match &*self.rx.borrow() {
            Some(geo) => finalize_snapshot(geo, screen_w, screen_h),
            None => ComputeSnapshot::empty(),
        }
    }
}

/// 全局快照生产者（单例模式，供 compute_draw_fn 闭包使用）。
///
/// 在 gameplay_page 初始化时设置，之后各帧通过 `latest_snapshot()` 读取。
static SNAPSHOT_PRODUCER: std::sync::OnceLock<Arc<AsyncSnapshotProducer>> =
    std::sync::OnceLock::new();

/// 设置全局快照生产者。应在页面构建时调用一次。
pub fn set_snapshot_producer(producer: Arc<AsyncSnapshotProducer>) {
    let _ = SNAPSHOT_PRODUCER.set(producer);
}

/// 读取全局快照生产者的最新帧（无阻塞）。
pub fn latest_snapshot(screen_w: u32, screen_h: u32) -> ComputeSnapshot {
    SNAPSHOT_PRODUCER
        .get()
        .map(|p| p.latest(screen_w, screen_h))
        .unwrap_or_else(ComputeSnapshot::empty)
}

const TAU: f32 = 2.0 * PI;
