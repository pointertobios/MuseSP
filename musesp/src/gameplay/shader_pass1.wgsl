// Pass 1：球坐标几何细分 → 顶点缓冲 (clip_pos+color) + 索引缓冲
// 主顶点 = 粗三角形顶点；非主顶点 = 细分产生（颜色插值）

struct SphericalVertex {
    r: f32,
    theta: f32,
    phi: f32,
    _pad: f32,
    color: vec4<f32>}
struct Params {
    view_proj: mat4x4<f32>,
    camera_eye: vec3<f32>,
    _pad2: f32,
    triangle_count: u32,
    sub_grid_size: u32,
    screen_width: f32,
    screen_height: f32}

// 输出顶点：clip-space position + world position + world normal + color = 64 bytes
struct OutVertex {
    clip_pos: vec4<f32>,
    world_pos: vec4<f32>,
    world_normal: vec4<f32>,
    color: vec4<f32>}

@group(0) @binding(0) var<storage, read> vertices: array<SphericalVertex>;
@group(0) @binding(1) var<storage, read> indices: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> vtx_out: array<OutVertex>;
@group(0) @binding(4) var<storage, read_write> idx_out: array<u32>;

const PI: f32 = 3.14159265359; const TAU: f32 = 6.28318530718;

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
fn grid_i(a: u32, b: u32, N: u32) -> u32 { return a * (2u * N + 3u - a) / 2u + b; }

@compute @workgroup_size(1)
fn main(@builtin(workgroup_id) wg_id: vec3<u32>) {
    let tri = wg_id.x;
    if tri >= params.triangle_count { return; }

    let base = tri * 3u;
    let i0 = indices[base]; let i1 = indices[base + 1u]; let i2 = indices[base + 2u];
    let sv0 = vertices[i0]; let sv1 = vertices[i1]; let sv2 = vertices[i2];
    let phi_adj = adjust_phi(sv0.phi, sv1.phi, sv2.phi);

    let N = params.sub_grid_size; let fN = f32(N);
    let nv = (N + 1u) * (N + 2u) / 2u;

    var sx: array<vec4<f32>, 28u>; // clip_pos
    var sw: array<vec4<f32>, 28u>; // world_pos
    var sn: array<vec4<f32>, 28u>; // world_normal
    var sc: array<vec4<f32>, 28u>; // color
    var sv: array<bool, 28u>;

    var k: u32 = 0u;
    for (var a: u32 = 0u; a <= N; a += 1u) {
        for (var b: u32 = 0u; b <= N - a; b += 1u) {
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
                sx[k] = clip; // 保留 clip-space（含 w）
                sw[k] = vec4<f32>(cart, 1.0); // 世界空间位置
                sn[k] = vec4<f32>(normalize(cart), 0.0); // 世界空间法线（球体质心在原点）
                sc[k] = alpha * sv0.color + beta * sv1.color + gamma * sv2.color;
                sv[k] = true;
            }
            k += 1u;
        }
    }

    // 写入顶点：tri * nv 起始
    let vtx_base = tri * nv;
    for (var i: u32 = 0u; i < nv; i += 1u) {
        vtx_out[vtx_base + i] = OutVertex(sx[i], sw[i], sn[i], sc[i]);
    }

    // 写入索引：tri * N*N*3 起始
    let idx_base = tri * N * N * 3u;
    var idx_count: u32 = 0u;
    for (var a: u32 = 0u; a < N; a += 1u) {
        for (var b: u32 = 0u; b < N - a; b += 1u) {
            let i00 = grid_i(a, b, N); let i10 = grid_i(a + 1u, b, N); let i01 = grid_i(a, b + 1u, N);
            if sv[i00] && sv[i10] && sv[i01] {
                idx_out[idx_base + idx_count] = vtx_base + i00;
                idx_out[idx_base + idx_count + 1u] = vtx_base + i10;
                idx_out[idx_base + idx_count + 2u] = vtx_base + i01;
                idx_count += 3u;
            }
            if a + b + 2u <= N {
                let i11 = grid_i(a + 1u, b + 1u, N);
                if sv[i10] && sv[i11] && sv[i01] {
                    idx_out[idx_base + idx_count] = vtx_base + i10;
                    idx_out[idx_base + idx_count + 1u] = vtx_base + i11;
                    idx_out[idx_base + idx_count + 2u] = vtx_base + i01;
                    idx_count += 3u;
                }
            }
        }
    }
}

