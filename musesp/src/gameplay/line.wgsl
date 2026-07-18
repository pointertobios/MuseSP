// 3D 直线渲染管线（直通 compute shader 输出的 clip-space）

struct LineParams {
    _unused: mat4x4<f32>,  // view_proj 已在 compute shader 中应用
}

@group(0) @binding(0) var<uniform> _params: LineParams;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @location(0) clip_pos: vec4<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = clip_pos;
    out.color = color;
    return out;
}

@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}
