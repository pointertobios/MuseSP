// Pass 1 Eval：曲面预细分（N=4）→ primary vertices
// workgroup_size=64，每线程一个 primary vertex

struct SphericalVertex {
    r: f32,
    theta: f32,
    phi: f32,
    _pad: f32,
    color: vec4<f32>,
}

struct Params {
    view_proj: mat4x4<f32>,
    camera_eye: vec3<f32>,
    _pad2: f32,
    triangle_count: u32,
    _unused: u32,
    screen_width: f32,
    screen_height: f32,
}

struct OutVertex {
    clip_pos: vec4<f32>,
    world_pos: vec4<f32>,
    world_normal: vec4<f32>,
    color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> vertices: array<SphericalVertex>;
@group(0) @binding(1) var<storage, read> indices: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> primary_out: array<OutVertex>;

const PI: f32 = 3.14159265359; const TAU: f32 = 6.28318530718;
const EVAL_N: u32 = 4u;  // per-triangle primary vertices: (4+1)(4+2)/2 = 15

fn sph_to_cart(r: f32, theta: f32, phi: f32) -> vec3<f32> {
    let st = sin(theta); return vec3<f32>(r * st * cos(phi), r * cos(theta), r * st * sin(phi));
}

fn adjust_phi(p0: f32, p1: f32, p2: f32) -> vec3<f32> {
    let mn = min(min(p0, p1), p2); let mx = max(max(p0, p1), p2);
    if mx - mn <= PI { return vec3<f32>(p0, p1, p2); }
    var a0 = p0; var a1 = p1; var a2 = p2;
    if p0 < PI { a0 += TAU; } if p1 < PI { a1 += TAU; } if p2 < PI { a2 += TAU; }
    return vec3<f32>(a0, a1, a2);
}

/// 将 primary vertex 局部索引 k 映射到 barycentric (a, b)。硬编码 N=4。
fn k_to_ab(k: u32) -> vec2<u32> {
    // row 0: len 5  (k=0..4)  → cumulative 0
    // row 1: len 4  (k=5..8)  → cumulative 5
    // row 2: len 3  (k=9..11) → cumulative 9
    // row 3: len 2  (k=12..13)→ cumulative 12
    // row 4: len 1  (k=14)    → cumulative 14
    if k < 5u { return vec2<u32>(0u, k); }
    if k < 9u { return vec2<u32>(1u, k - 5u); }
    if k < 12u { return vec2<u32>(2u, k - 9u); }
    if k < 14u { return vec2<u32>(3u, k - 12u); }
    return vec2<u32>(4u, 0u);
}

const PRIMARY_VERTICES_PER_TRIANGLE: u32 = (EVAL_N + 1u) * (EVAL_N + 2u) / 2u;

@compute @workgroup_size(64) // 必须与 Rust EVAL_WORKGROUP_SIZE 一致
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let total = params.triangle_count * PRIMARY_VERTICES_PER_TRIANGLE;
    if gid.x >= total { return; }

    let tri_id = gid.x / PRIMARY_VERTICES_PER_TRIANGLE;
    let local_k = gid.x % PRIMARY_VERTICES_PER_TRIANGLE;

    let base = tri_id * 3u;
    let i0 = indices[base];
    let i1 = indices[base + 1u];
    let i2 = indices[base + 2u];
    let sv0 = vertices[i0];
    let sv1 = vertices[i1];
    let sv2 = vertices[i2];

    let phi_adj = adjust_phi(sv0.phi, sv1.phi, sv2.phi);

    let ab = k_to_ab(local_k);
    let a = ab.x; let b = ab.y;
    let fa = f32(a) / f32(EVAL_N);
    let fb = f32(b) / f32(EVAL_N);
    let alpha = fa; let beta = fb; let gamma = 1.0 - alpha - beta;

    let r = alpha * sv0.r + beta * sv1.r + gamma * sv2.r;
    let th = alpha * sv0.theta + beta * sv1.theta + gamma * sv2.theta;
    let pi = alpha * phi_adj.x + beta * phi_adj.y + gamma * phi_adj.z;
    let ph = pi - TAU * floor(pi / TAU);

    let cart = sph_to_cart(r, th, ph);
    let clip = params.view_proj * vec4<f32>(cart, 1.0);

    var col = alpha * sv0.color + beta * sv1.color + gamma * sv2.color;
    // 顶点在摄像机后方：alpha=0 标记，Pass 2 测量时跳过屏幕 px 度量
    if clip.w <= 0.0 { col.a = 0.0; }

    let out_idx = tri_id * PRIMARY_VERTICES_PER_TRIANGLE + local_k;
    primary_out[out_idx] = OutVertex(
        clip,
        vec4<f32>(cart, 1.0),
        vec4<f32>(normalize(cart), 0.0),
        col,
    );
}
