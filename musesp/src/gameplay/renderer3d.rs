//! 球坐标渲染管线（两 pass 架构）。
//!
//! Pass 1 (GPU Subdivide)：在球坐标参数空间细分粗三角形 → Cartesian → 屏幕坐标
//! Pass 2 (GPU Rasterize)：屏幕空间子三角形边函数光栅化 + Alpha 混合
//!
//! CPU 只负责生成粗球坐标几何 + view_proj 矩阵。

use musesp_ui::renderer::ComputeSnapshot;
use std::f32::consts::PI;
use std::sync::Arc;

// ── 常量 ──────────────────────────────────────────────────────────────

const CAMERA_EYE: [f32; 3] = [4.0, 3.0, 4.0];
const CAMERA_TARGET: [f32; 3] = [0.0, 0.0, 0.0];
const CAMERA_UP: [f32; 3] = [0.0, 1.0, 0.0];
const FOV_DEGREES: f32 = 60.0;
const NEAR: f32 = 0.1;
const FAR: f32 = 100.0;
const ASPECT: f32 = 16.0 / 9.0;
const SUB_GRID_SIZE: u32 = 6;
const TAU: f32 = 2.0 * PI;

// ── 球坐标顶点（传给 GPU Pass 1）─────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SphericalVertex {
    pub r: f32, pub theta: f32, pub phi: f32, _pad: f32, pub color: [f32; 4],
}

impl SphericalVertex {
    fn new(r: f32, theta: f32, phi: f32, color: [f32; 4]) -> Self { SphericalVertex { r, theta, phi, _pad: 0.0, color } }
}

// ── 直线顶点（球坐标，传给 GPU line pipeline）─────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    /// 球坐标：半径 r（到原点距离）
    pub r: f32,
    /// 球坐标：极角 theta（从 Y 轴，0..π）
    pub theta: f32,
    /// 球坐标：方位角 phi（从 +X 轴，绕 Y 轴）
    pub phi: f32,
    _pad: f32,
    pub color: [f32; 4],
}

impl LineVertex {
    #[allow(dead_code)]
    pub fn from_cartesian(pos: [f32; 3], color: [f32; 4]) -> Self {
        let r = (pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]).sqrt();
        let (theta, phi) = if r < 1e-6 {
            (0.0, 0.0) // 原点退化，任意角度均可
        } else {
            (f32::acos(pos[1] / r), f32::atan2(pos[2], pos[0]))
        };
        LineVertex { r, theta, phi, _pad: 0.0, color }
    }
}

// ── Pass 1 参数（80 字节） ────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SubdivideParams {
    view_proj: [[f32; 4]; 4],
    /// 点光源 / 摄像机位置（世界空间）
    camera_eye: [f32; 3],
    _pad2: f32,
    triangle_count: u32,
    sub_grid_size: u32,
    screen_width: f32,
    screen_height: f32,
}

// ── 几何生成 ──────────────────────────────────────────────────────────

/// 东半球：θ ∈ [0, π]（全南北），φ ∈ [-π/2, π/2]（东半，+X 侧）
fn hemisphere(theta_bands: u32, phi_steps: u32, rot_phi: f32) -> (Vec<SphericalVertex>, Vec<u32>) {
    let r = 1.0f32;
    let mut v = Vec::with_capacity(((theta_bands + 1) * phi_steps) as usize);
    for i in 0..=theta_bands {
        let th = (i as f32 / theta_bands as f32) * PI;
        for j in 0..phi_steps {
            let bp = -PI/2.0 + (j as f32 / phi_steps as f32) * PI;
            v.push(SphericalVertex::new(r, th, (bp + rot_phi) % TAU, hemi_color(th, bp)));
        }
    }
    let mut idx = Vec::with_capacity((theta_bands * phi_steps * 6) as usize);
    for i in 0..theta_bands {
        for j in 0..phi_steps {
            let jn = (j + 1) % phi_steps;
            let (tl, tr, bl, br) = (i*phi_steps+j, i*phi_steps+jn, (i+1)*phi_steps+j, (i+1)*phi_steps+jn);
            idx.push(tl); idx.push(br); idx.push(bl); idx.push(tl); idx.push(tr); idx.push(br);
        }
    }
    (v, idx)
}

/// 颜色同时依赖 theta（极角，南北渐变）和 phi（方位角，水平渐变）。
/// 使用 HSL→RGB 转换，饱和度 100%，亮度 50%，产生鲜艳彩虹。
fn hemi_color(theta: f32, phi: f32) -> [f32; 4] {
    let tp = (phi + PI/2.0) / PI; // 0..1 色相
    let tt = theta / PI;           // 0..1 垂直
    // HSL 彩虹: H=tp 完整周期, S=1.0, L=0.5
    let h = tp * 6.0; // 0..6
    let c = 1.0; // chroma
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h < 1.0 { (c, x, 0.0) }
        else if h < 2.0 { (x, c, 0.0) }
        else if h < 3.0 { (0.0, c, x) }
        else if h < 4.0 { (0.0, x, c) }
        else if h < 5.0 { (x, 0.0, c) }
        else { (c, 0.0, x) };
    // 亮度在赤道最亮，两极渐暗
    let bright = 0.3 + 0.7 * (tt * (1.0 - tt) * 4.0).sqrt();
    [r1 * bright, g1 * bright, b1 * bright, 0.6]
}

fn all_geometry(elapsed_secs: f32) -> (Vec<SphericalVertex>, Vec<u32>) {
    let rot = elapsed_secs * 0.8;
    hemisphere(10, 20, rot)
}

/// 生成球坐标参考线的粗端点（每对 = 一条逻辑线段）。
///
/// GPU compute shader 负责在球坐标空间细分 + sph_to_cart 转换。
/// - 赤道（红色）：r=1, θ=π/2, φ: 0→2π
/// - 极轴北半（蓝色）：θ=0, r: 1→0
/// - 极轴南半（蓝色）：θ=π, r: 0→1
fn reference_lines() -> (Vec<LineVertex>, u32) {
    let equator_color = [1.0, 0.1, 0.1, 0.9];
    let polar_color = [0.1, 0.3, 1.0, 0.9];

    let verts = vec![
        // 赤道
        LineVertex { r: 1.0, theta: PI / 2.0, phi: 0.0, _pad: 0.0, color: equator_color },
        LineVertex { r: 1.0, theta: PI / 2.0, phi: TAU, _pad: 0.0, color: equator_color },
        // 极轴北半
        LineVertex { r: 1.0, theta: 0.0, phi: 0.0, _pad: 0.0, color: polar_color },
        LineVertex { r: 0.0, theta: 0.0, phi: 0.0, _pad: 0.0, color: polar_color },
        // 极轴南半
        LineVertex { r: 0.0, theta: PI, phi: 0.0, _pad: 0.0, color: polar_color },
        LineVertex { r: 1.0, theta: PI, phi: 0.0, _pad: 0.0, color: polar_color },
    ];
    let line_count = (verts.len() / 2) as u32;
    (verts, line_count)
}

// ── 矩阵运算（列优先，适配 wgpu / WGSL）───────────────────────────────
// 存储约定: m[col][row]，即 m[c] 为第 c 列

fn perspective(fov_rad: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_rad / 2.0).tan();
    let d = near - far;
    [
        [f / aspect, 0.0, 0.0, 0.0], // col 0
        [0.0, f, 0.0, 0.0],          // col 1
        [0.0, 0.0, far / d, -1.0],   // col 2
        [0.0, 0.0, (near * far) / d, 0.0], // col 3
    ]
}

fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let fwd = normalize(sub(target, eye));
    let right = normalize(cross(fwd, up));
    let up2 = cross(right, fwd);
    [
        [right[0], up2[0], -fwd[0], 0.0],                          // col 0
        [right[1], up2[1], -fwd[1], 0.0],                          // col 1
        [right[2], up2[2], -fwd[2], 0.0],                          // col 2
        [-dot(right, eye), -dot(up2, eye), dot(fwd, eye), 1.0],    // col 3
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
fn sub(a:[f32;3],b:[f32;3])->[f32;3]{[a[0]-b[0],a[1]-b[1],a[2]-b[2]]}
fn dot(a:[f32;3],b:[f32;3])->f32{a[0]*b[0]+a[1]*b[1]+a[2]*b[2]}
fn cross(a:[f32;3],b:[f32;3])->[f32;3]{[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}
fn normalize(v:[f32;3])->[f32;3]{let l=(v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();[v[0]/l,v[1]/l,v[2]/l]}

// ── 双 snapshot 输出 ──────────────────────────────────────────────────

#[derive(Clone)]
pub struct PrecomputedGeometry {
    pub coarse_verts: Vec<u8>,
    pub coarse_indices: Vec<u32>,
    pub coarse_tri_count: u32,
    pub view_proj: [[f32; 4]; 4],
    pub line_endpoints: Vec<u8>,
    pub line_count: u32,
}

pub fn precompute_geometry(elapsed_secs: f32) -> PrecomputedGeometry {
    let (verts, indices) = all_geometry(elapsed_secs);
    let (lverts, lcount) = reference_lines();
    let proj = perspective(FOV_DEGREES.to_radians(), ASPECT, NEAR, FAR);
    let view = look_at(CAMERA_EYE, CAMERA_TARGET, CAMERA_UP);
    let tri_count = (indices.len()/3) as u32;
    PrecomputedGeometry {
        coarse_verts: bytemuck::cast_slice(&verts).to_vec(),
        coarse_indices: indices,
        coarse_tri_count: tri_count,
        view_proj: mul4(&proj, &view),
        line_endpoints: bytemuck::cast_slice(&lverts).to_vec(),
        line_count: lcount,
    }
}

pub fn finalize_snapshot(geo: &PrecomputedGeometry, screen_w: u32, screen_h: u32) -> ComputeSnapshot {
    let p = SubdivideParams {
        view_proj: geo.view_proj,
        camera_eye: CAMERA_EYE,
        _pad2: 0.0,
        triangle_count: geo.coarse_tri_count,
        sub_grid_size: SUB_GRID_SIZE,
        screen_width: screen_w as f32,
        screen_height: screen_h as f32,
    };
    ComputeSnapshot { vertex_data: geo.coarse_verts.clone(), indices: geo.coarse_indices.clone(), uniform_data: bytemuck::bytes_of(&p).to_vec(), vertex_count: 0, triangle_count: geo.coarse_tri_count }
}

pub fn compute_lines_snapshot(
    geo: &PrecomputedGeometry,
    screen_w: u32,
    screen_h: u32,
) -> (Vec<u8>, u32, Vec<u8>) {
    // uniform: view_proj(64) + line_count(4) + _pad(4) + screen_w(4) + screen_h(4) = 80 bytes
    let mut uf = bytemuck::bytes_of(&geo.view_proj).to_vec();
    uf.extend_from_slice(&geo.line_count.to_le_bytes());
    uf.extend_from_slice(&0u32.to_le_bytes()); // _pad
    uf.extend_from_slice(&screen_w.to_le_bytes());
    uf.extend_from_slice(&screen_h.to_le_bytes());
    (geo.line_endpoints.clone(), geo.line_count, uf)
}

use std::time::Instant;
use tokio::sync::watch;

pub struct AsyncSnapshotProducer { rx: watch::Receiver<Option<PrecomputedGeometry>>, _tx: watch::Sender<Option<PrecomputedGeometry>> }

impl AsyncSnapshotProducer {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(None); let tx2 = tx.clone();
        tokio::spawn(async move { let start = Instant::now(); loop { let geo = precompute_geometry(start.elapsed().as_secs_f32()); if tx2.send(Some(geo)).is_err() { break; } tokio::task::yield_now().await; } });
        AsyncSnapshotProducer { rx, _tx: tx }
    }
    pub fn latest(&self, screen_w: u32, screen_h: u32) -> ComputeSnapshot {
        match &*self.rx.borrow() { Some(geo) => finalize_snapshot(geo, screen_w, screen_h), None => ComputeSnapshot::empty() }
    }
    pub fn latest_compute_lines(&self, screen_w: u32, screen_h: u32) -> (Vec<u8>, u32, Vec<u8>) {
        match &*self.rx.borrow() {
            Some(geo) => compute_lines_snapshot(geo, screen_w, screen_h),
            None => (Vec::new(), 0, Vec::new()),
        }
    }
}

static SNAPSHOT_PRODUCER: std::sync::OnceLock<Arc<AsyncSnapshotProducer>> = std::sync::OnceLock::new();
pub fn set_snapshot_producer(p: Arc<AsyncSnapshotProducer>) { let _ = SNAPSHOT_PRODUCER.set(p); }
pub fn latest_snapshot(screen_w: u32, screen_h: u32) -> ComputeSnapshot {
    SNAPSHOT_PRODUCER.get().map(|p| p.latest(screen_w, screen_h)).unwrap_or_else(ComputeSnapshot::empty)
}
pub fn latest_compute_lines_snapshot(screen_w: u32, screen_h: u32) -> (Vec<u8>, u32, Vec<u8>) {
    SNAPSHOT_PRODUCER.get().map(|p| p.latest_compute_lines(screen_w, screen_h)).unwrap_or_else(|| (Vec::new(), 0, Vec::new()))
}

