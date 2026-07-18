// Line Pass 1 Eval：线段预细分（N=4）→ primary vertices
// workgroup_size=64，每线程一个 primary vertex

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

@group(0) @binding(0) var<storage, read> endpoints: array<LineEndpoint>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> primary_out: array<PrimaryVertex>;

const EVAL_N: u32 = 4u;
const PRIMARY_VERTICES_PER_LINE: u32 = EVAL_N + 1u; // 5

fn sph_to_cart(r: f32, theta: f32, phi: f32) -> vec3<f32> {
    let st = sin(theta); return vec3<f32>(r * st * cos(phi), r * cos(theta), r * st * sin(phi));
}

@compute @workgroup_size(64) // 必须与 Rust EVAL_WORKGROUP_SIZE 一致
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let total = params.line_count * PRIMARY_VERTICES_PER_LINE;
    if gid.x >= total { return; }

    let line_id = gid.x / PRIMARY_VERTICES_PER_LINE;
    let local_i = gid.x % PRIMARY_VERTICES_PER_LINE;

    let base = line_id * 2u;
    let start = endpoints[base];
    let end = endpoints[base + 1u];

    let t = f32(local_i) / f32(EVAL_N);
    let r = start.r + (end.r - start.r) * t;
    let theta = start.theta + (end.theta - start.theta) * t;
    let phi = start.phi + (end.phi - start.phi) * t;

    let cart = sph_to_cart(r, theta, phi);
    let clip = params.view_proj * vec4<f32>(cart, 1.0);

    let cr = start.color[0] + (end.color[0] - start.color[0]) * t;
    let cg = start.color[1] + (end.color[1] - start.color[1]) * t;
    let cb = start.color[2] + (end.color[2] - start.color[2]) * t;
    let ca = start.color[3] + (end.color[3] - start.color[3]) * t;
    var col = vec4<f32>(cr, cg, cb, ca);
    if clip.w <= 0.0 { col.a = 0.0; }

    primary_out[gid.x] = PrimaryVertex(clip, col);
}
