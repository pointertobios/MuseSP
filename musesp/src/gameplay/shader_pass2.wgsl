// Pass 2：直通 clip-space VS + 点光源光照 FS

// ── Vertex Shader ──

struct VertexInput {
    @location(0) clip_pos: vec4<f32>,
    @location(1) world_pos: vec4<f32>,
    @location(2) world_normal: vec4<f32>,
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = in.clip_pos;
    out.world_pos = in.world_pos.xyz;
    out.world_normal = in.world_normal.xyz;
    out.color = in.color;
    return out;
}

// ── Fragment Shader ──

struct LightingParams {
    camera_eye: vec4<f32>,
}

@group(0) @binding(0) var<uniform> lighting: LightingParams;

@fragment
fn fs_main(
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
) -> @location(0) vec4<f32> {
    let N = normalize(world_normal);
    let to_light = lighting.camera_eye.xyz - world_pos;
    let dist_sq = dot(to_light, to_light);
    let dist = sqrt(dist_sq);

    let attenuation = 1.0 / (dist_sq + 0.001);

    let light_dir = to_light / dist;
    let NoL = max(dot(N, light_dir), 0.0);

    let ambient = 0.1;
    let diffuse = NoL * attenuation * 15.0;

    let lit = color.rgb * (ambient + diffuse);
    return vec4<f32>(lit, color.a);
}
