// Line Pass 2 Final：测量 + 自适应线段细分（N ∈ [1, 32]）
// workgroup_size=1，每 workgroup 一条逻辑线段

struct LineEndpoint {
    r: f32,
    theta: f32,
    phi: f32,
    _pad: f32,
    color: vec4<f32>,
}

struct Params {
    view_proj: mat4x4<f32>,
    line_count: u32,
    _pad0: u32,
    screen_width: f32,
    screen_height: f32,
}

struct PrimaryVertex {
    clip_pos: vec4<f32>,
    color: vec4<f32>,
}

struct OutVertex {
    clip_pos: vec4<f32>,
    color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> endpoints: array<LineEndpoint>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> primary_in: array<PrimaryVertex>;
@group(0) @binding(3) var<storage, read_write> vtx_out: array<OutVertex>;
@group(0) @binding(4) var<storage, read_write> idx_out: array<u32>;

const PI: f32 = 3.14159265359; const TAU: f32 = 6.28318530718;
const EVAL_N: u32 = 4u;
const TARGET_PIXELS_PER_SEGMENT: f32 = 2.0;
const ANGLE_PER_SEGMENT: f32 = 0.04908738521; // PI / 64 ≈ 2.8°
const MIN_SUBDIVISIONS: u32 = 1u;
const MAX_SUBDIVISIONS: u32 = 256u;
const PRIMARY_VERTICES_PER_LINE: u32 = 5u;
const MAX_VERTICES_PER_LINE: u32 = MAX_SUBDIVISIONS + 1u;   // 257
const MAX_INDICES_PER_LINE: u32 = MAX_SUBDIVISIONS * 2u;   // 512

fn sph_to_cart(r: f32, theta: f32, phi: f32) -> vec3<f32> {
    let st = sin(theta); return vec3<f32>(r * st * cos(phi), r * cos(theta), r * st * sin(phi));
}

@compute @workgroup_size(1)
fn main(@builtin(workgroup_id) wg_id: vec3<u32>) {
    let line_id = wg_id.x;
    if line_id >= params.line_count { return; }

    let base = line_id * 2u;
    let start = endpoints[base];
    let end = endpoints[base + 1u];

    // ── 测量：屏幕像素弦长（逐段独立）──
    let sw = params.screen_width;
    let sh = params.screen_height;
    let primary_base = line_id * PRIMARY_VERTICES_PER_LINE;
    var max_screen_px: f32 = 0.0;

    for (var i: u32 = 0u; i < EVAL_N; i += 1u) {
        let v0 = primary_in[primary_base + i];
        let v1 = primary_in[primary_base + i + 1u];
        if v0.color.a <= 0.0 || v1.color.a <= 0.0 { continue; }

        let c0 = v0.clip_pos; let c1 = v1.clip_pos;
        let ndc0 = c0.xy / c0.w; let ndc1 = c1.xy / c1.w;
        let dx = (ndc1.x - ndc0.x) * sw * 0.5;
        let dy = (ndc1.y - ndc0.y) * sh * 0.5;
        let px = sqrt(dx * dx + dy * dy);
        max_screen_px = max(max_screen_px, px);
    }

    let pixels_per_segment = TARGET_PIXELS_PER_SEGMENT / f32(EVAL_N);  // 2/4 = 0.5px
    var n_screen: u32 = 1u;
    if max_screen_px > 1e-6 {
        n_screen = u32(ceil(max_screen_px / pixels_per_segment));
    }

    // ── 测量：角跨度（安全网）──
    let angle_span = abs(start.theta - end.theta) + abs(start.phi - end.phi);
    let n_angle = u32(ceil(angle_span / ANGLE_PER_SEGMENT));

    // 最终 N
    var n_adaptive = max(n_screen, n_angle);
    n_adaptive = clamp(n_adaptive, MIN_SUBDIVISIONS, MAX_SUBDIVISIONS);

    // ── 最终细分 ──
    let vtx_base = line_id * MAX_VERTICES_PER_LINE;
    for (var i: u32 = 0u; i <= n_adaptive; i += 1u) {
        let t = f32(i) / f32(n_adaptive);
        let r = start.r + (end.r - start.r) * t;
        let theta = start.theta + (end.theta - start.theta) * t;
        let phi = start.phi + (end.phi - start.phi) * t;
        let cart = sph_to_cart(r, theta, phi);
        let clip = params.view_proj * vec4<f32>(cart, 1.0);

        let cr = start.color[0] + (end.color[0] - start.color[0]) * t;
        let cg = start.color[1] + (end.color[1] - start.color[1]) * t;
        let cb = start.color[2] + (end.color[2] - start.color[2]) * t;
        let ca = start.color[3] + (end.color[3] - start.color[3]) * t;

        vtx_out[vtx_base + i] = OutVertex(clip, vec4<f32>(cr, cg, cb, ca));
    }

    // LineList 索引
    let idx_base = line_id * MAX_INDICES_PER_LINE;
    for (var i: u32 = 0u; i < n_adaptive; i += 1u) {
        idx_out[idx_base + i * 2u] = vtx_base + i;
        idx_out[idx_base + i * 2u + 1u] = vtx_base + i + 1u;
    }

    // 清空未用槽位
    let dead_vertex = OutVertex(vec4<f32>(0.0, 0.0, 0.0, -1.0), vec4<f32>(0.0));
    for (var i: u32 = n_adaptive + 1u; i < MAX_VERTICES_PER_LINE; i += 1u) {
        vtx_out[vtx_base + i] = dead_vertex;
    }
    let total_idx = n_adaptive * 2u;
    for (var i: u32 = total_idx; i < MAX_INDICES_PER_LINE; i += 1u) {
        idx_out[idx_base + i] = vtx_base;
    }
}
