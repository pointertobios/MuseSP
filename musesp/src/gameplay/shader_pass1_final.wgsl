// Pass 1 Final：测量 + 自适应曲面细分（N ∈ [1, 8]）
// workgroup_size=1，每 workgroup 一个 raw 三角形

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
@group(0) @binding(3) var<storage, read_write> primary_in: array<OutVertex>;
@group(0) @binding(4) var<storage, read_write> vtx_out: array<OutVertex>;
@group(0) @binding(5) var<storage, read_write> idx_out: array<u32>;

const PI: f32 = 3.14159265359; const TAU: f32 = 6.28318530718;
const EVAL_N: u32 = 4u;
const TARGET_PIXELS_PER_SEGMENT: f32 = 8.0;
const ANGLE_PER_SEGMENT: f32 = 0.19634954084; // PI / 16
const MIN_SUBDIVISIONS: u32 = 1u;
const MAX_SUBDIVISIONS: u32 = 8u;
const PRIMARY_VERTICES_PER_TRIANGLE: u32 = 15u;
const MAX_VERTICES_PER_TRIANGLE: u32 = (MAX_SUBDIVISIONS + 1u) * (MAX_SUBDIVISIONS + 2u) / 2u; // 45
const MAX_INDICES_PER_TRIANGLE: u32 = MAX_SUBDIVISIONS * MAX_SUBDIVISIONS * 3u; // 192

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

fn grid_i(a: u32, b: u32, N: u32) -> u32 {
    return a * (2u * N + 3u - a) / 2u + b;
}

/// 逐段测量 4x4 primary 网格上所有边的屏幕像素长度和角跨度
fn measure(primary_base: u32) -> u32 {
    let sw = params.screen_width;
    let sh = params.screen_height;
    var max_screen_px: f32 = 0.0;
    var total_angle: f32 = 0.0;

    // 遍历 primary 网格：a ∈ [0, N), b ∈ [0, N-a)
    for (var a: u32 = 0u; a < EVAL_N; a += 1u) {
        for (var b: u32 = 0u; b < EVAL_N - a; b += 1u) {
            let i00 = primary_base + grid_i(a, b, EVAL_N);
            let i10 = primary_base + grid_i(a + 1u, b, EVAL_N);
            let i01 = primary_base + grid_i(a, b + 1u, EVAL_N);

            let v00 = primary_in[i00]; let v10 = primary_in[i10]; let v01 = primary_in[i01];

            // 屏幕像素弦长（跳过摄像机后方顶点，alpha=0 标记）
            if v00.color.a > 0.0 && v10.color.a > 0.0 {
                let c0 = v00.clip_pos; let c1 = v10.clip_pos;
                let ndc0 = c0.xy / c0.w; let ndc1 = c1.xy / c1.w;
                let dx = (ndc1.x - ndc0.x) * sw * 0.5;
                let dy = (ndc1.y - ndc0.y) * sh * 0.5;
                let px = sqrt(dx * dx + dy * dy);
                max_screen_px = max(max_screen_px, px);
            }
            if v00.color.a > 0.0 && v01.color.a > 0.0 {
                let c0 = v00.clip_pos; let c1 = v01.clip_pos;
                let ndc0 = c0.xy / c0.w; let ndc1 = c1.xy / c1.w;
                let dx = (ndc1.x - ndc0.x) * sw * 0.5;
                let dy = (ndc1.y - ndc0.y) * sh * 0.5;
                let px = sqrt(dx * dx + dy * dy);
                max_screen_px = max(max_screen_px, px);
            }

            // 第二个对角线（i10 → i01）
            if a + b + 2u <= EVAL_N {
                let i11 = primary_base + grid_i(a + 1u, b + 1u, EVAL_N);
                let v11 = primary_in[i11];
                if v10.color.a > 0.0 && v11.color.a > 0.0 {
                    let c1 = v10.clip_pos; let c2 = v11.clip_pos;
                    let ndc1 = c1.xy / c1.w; let ndc2 = c2.xy / c2.w;
                    let dx = (ndc2.x - ndc1.x) * sw * 0.5;
                    let dy = (ndc2.y - ndc1.y) * sh * 0.5;
                    let px = sqrt(dx * dx + dy * dy);
                    max_screen_px = max(max_screen_px, px);
                }
                if v11.color.a > 0.0 && v01.color.a > 0.0 {
                    let c2 = v11.clip_pos; let c3 = v01.clip_pos;
                    let ndc2 = c2.xy / c2.w; let ndc3 = c3.xy / c3.w;
                    let dx = (ndc3.x - ndc2.x) * sw * 0.5;
                    let dy = (ndc3.y - ndc2.y) * sh * 0.5;
                    let px = sqrt(dx * dx + dy * dy);
                    max_screen_px = max(max_screen_px, px);
                }
            }
        }
    }

    // 屏幕度量 → N_screen
    let px_per_segment = TARGET_PIXELS_PER_SEGMENT / f32(EVAL_N);
    var n_screen: u32 = 1u;
    if max_screen_px > 1e-6 {
        n_screen = u32(ceil(max_screen_px / px_per_segment));
    }

    return n_screen;
}

@compute @workgroup_size(1)
fn main(@builtin(workgroup_id) wg_id: vec3<u32>) {
    let tri = wg_id.x;
    if tri >= params.triangle_count { return; }

    let base_idx = tri * 3u;
    let i0 = indices[base_idx];
    let i1 = indices[base_idx + 1u];
    let i2 = indices[base_idx + 2u];
    let sv0 = vertices[i0];
    let sv1 = vertices[i1];
    let sv2 = vertices[i2];
    let phi_adj = adjust_phi(sv0.phi, sv1.phi, sv2.phi);

    // ── 测量：屏幕像素弦长 ──
    let primary_base = tri * PRIMARY_VERTICES_PER_TRIANGLE;
    let n_screen = measure(primary_base);

    // ── 测量：角跨度（安全网）──
    // 取 raw 三角形三个边的最大 |Δθ| + |Δφ|
    var max_angle: f32 = 0.0;
    // 边 0→1
    var da = abs(sv0.theta - sv1.theta) + abs(phi_adj.x - phi_adj.y);
    max_angle = max(max_angle, da);
    // 边 1→2
    da = abs(sv1.theta - sv2.theta) + abs(phi_adj.y - phi_adj.z);
    max_angle = max(max_angle, da);
    // 边 2→0
    da = abs(sv2.theta - sv0.theta) + abs(phi_adj.z - phi_adj.x);
    max_angle = max(max_angle, da);

    let n_angle = u32(ceil(max_angle / ANGLE_PER_SEGMENT));

    // 最终 N
    var n_adaptive = max(n_screen, n_angle);
    n_adaptive = clamp(n_adaptive, MIN_SUBDIVISIONS, MAX_SUBDIVISIONS);

    // ── 最终细分 ──
    let fN = f32(n_adaptive);
    let nv = (n_adaptive + 1u) * (n_adaptive + 2u) / 2u;

    var sx: array<vec4<f32>, 45u>;  // clip_pos
    var sw: array<vec4<f32>, 45u>;  // world_pos
    var sn: array<vec4<f32>, 45u>;  // world_normal
    var sc: array<vec4<f32>, 45u>;  // color
    var sv: array<bool, 45u>;

    var k: u32 = 0u;
    for (var a: u32 = 0u; a <= n_adaptive; a += 1u) {
        for (var b: u32 = 0u; b <= n_adaptive - a; b += 1u) {
            let fa = f32(a); let fb = f32(b);
            let alpha = fa / fN; let beta = fb / fN; let gamma = 1.0 - alpha - beta;

            let r = alpha * sv0.r + beta * sv1.r + gamma * sv2.r;
            let th = alpha * sv0.theta + beta * sv1.theta + gamma * sv2.theta;
            let pi = alpha * phi_adj.x + beta * phi_adj.y + gamma * phi_adj.z;
            let ph = pi - TAU * floor(pi / TAU);

            let cart = sph_to_cart(r, th, ph);
            let clip = params.view_proj * vec4<f32>(cart, 1.0);

            if clip.w <= 0.0 { sv[k] = false; }
            else {
                sx[k] = clip;
                sw[k] = vec4<f32>(cart, 1.0);
                sn[k] = vec4<f32>(normalize(cart), 0.0);
                sc[k] = alpha * sv0.color + beta * sv1.color + gamma * sv2.color;
                sv[k] = true;
            }
            k += 1u;
        }
    }

    // 写入顶点
    let vtx_base = tri * MAX_VERTICES_PER_TRIANGLE;
    for (var i: u32 = 0u; i < nv; i += 1u) {
        vtx_out[vtx_base + i] = OutVertex(sx[i], sw[i], sn[i], sc[i]);
    }

    // 写入索引
    let idx_base = tri * MAX_INDICES_PER_TRIANGLE;
    var idx_count: u32 = 0u;
    for (var a: u32 = 0u; a < n_adaptive; a += 1u) {
        for (var b: u32 = 0u; b < n_adaptive - a; b += 1u) {
            let i00 = grid_i(a, b, n_adaptive);
            let i10 = grid_i(a + 1u, b, n_adaptive);
            let i01 = grid_i(a, b + 1u, n_adaptive);
            if sv[i00] && sv[i10] && sv[i01] {
                idx_out[idx_base + idx_count] = vtx_base + i00;
                idx_out[idx_base + idx_count + 1u] = vtx_base + i10;
                idx_out[idx_base + idx_count + 2u] = vtx_base + i01;
                idx_count += 3u;
            }
            if a + b + 2u <= n_adaptive {
                let i11 = grid_i(a + 1u, b + 1u, n_adaptive);
                if sv[i10] && sv[i11] && sv[i01] {
                    idx_out[idx_base + idx_count] = vtx_base + i10;
                    idx_out[idx_base + idx_count + 1u] = vtx_base + i11;
                    idx_out[idx_base + idx_count + 2u] = vtx_base + i01;
                    idx_count += 3u;
                }
            }
        }
    }

    // 清空未用槽位：防止旧帧残留数据导致固定顶点
    let dead_clip = vec4<f32>(0.0, 0.0, 0.0, -1.0); // w=-1 → 总在裁剪体外
    let dead_vertex = OutVertex(dead_clip, vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    for (var i: u32 = nv; i < MAX_VERTICES_PER_TRIANGLE; i += 1u) {
        vtx_out[vtx_base + i] = dead_vertex;
    }
    // 未用索引全指向 vtx_base（形成退化三角形）
    for (var i: u32 = idx_count; i < MAX_INDICES_PER_TRIANGLE; i += 1u) {
        idx_out[idx_base + i] = vtx_base;
    }
}
