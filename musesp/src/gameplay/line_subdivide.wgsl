// 线段球坐标细分 Compute Shader
// 输入：粗球坐标线段端点（每对连续顶点 = 一条逻辑线段）
// 输出：球坐标空间细分 → sph_to_cart → Cartesian 顶点 + LineList 索引

struct LineEndpoint {
    r: f32,
    theta: f32,
    phi: f32,
    _pad: f32,
    color: vec4<f32>,
}

struct Params {
    line_count: u32,
    sub_divisions: u32,
    _pad: vec2<u32>,
}

struct OutVertex {
    position: vec3<f32>,
    _pad: f32,
    color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> endpoints: array<LineEndpoint>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> vtx_out: array<OutVertex>;
@group(0) @binding(3) var<storage, read_write> idx_out: array<u32>;

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

fn sph_to_cart(r: f32, theta: f32, phi: f32) -> vec3<f32> {
    let st = sin(theta);
    return vec3<f32>(r * st * cos(phi), r * cos(theta), r * st * sin(phi));
}

@compute @workgroup_size(1)
fn main(@builtin(workgroup_id) wg_id: vec3<u32>) {
    let line_idx = wg_id.x;
    if line_idx >= params.line_count { return; }

    let n = params.sub_divisions;
    let base = line_idx * 2u; // 每对端点 = 一条线段
    let start = endpoints[base];
    let end = endpoints[base + 1u];

    let vtx_base = line_idx * (n + 1u);

    // 球坐标空间线性插值 → Cartesian
    for (var i: u32 = 0u; i <= n; i += 1u) {
        let t = f32(i) / f32(n);
        let r = start.r + (end.r - start.r) * t;
        let theta = start.theta + (end.theta - start.theta) * t;
        let phi = start.phi + (end.phi - start.phi) * t;
        let cart = sph_to_cart(r, theta, phi);

        let cr = start.color[0] + (end.color[0] - start.color[0]) * t;
        let cg = start.color[1] + (end.color[1] - start.color[1]) * t;
        let cb = start.color[2] + (end.color[2] - start.color[2]) * t;
        let ca = start.color[3] + (end.color[3] - start.color[3]) * t;

        vtx_out[vtx_base + i] = OutVertex(cart, 0.0, vec4<f32>(cr, cg, cb, ca));
    }

    // LineList 索引：(0,1), (1,2), ..., (n-1, n)
    let idx_base = line_idx * n * 2u;
    for (var i: u32 = 0u; i < n; i += 1u) {
        idx_out[idx_base + i * 2u] = vtx_base + i;
        idx_out[idx_base + i * 2u + 1u] = vtx_base + i + 1u;
    }
}
