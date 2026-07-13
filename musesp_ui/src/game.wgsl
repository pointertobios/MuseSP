struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color_flag: f32,
}

struct VertexOutput3D {
    @builtin(position) position: vec4<f32>,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    rotation: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@vertex
fn vs_main_3d(in: VertexInput) -> VertexOutput3D {
    let rotated = camera.rotation * vec4<f32>(in.position, 1.0);
    let clip = camera.view_proj * vec4<f32>(rotated.xyz, 1.0);
    return VertexOutput3D(clip);
}

@fragment
fn fs_main_3d(in: VertexOutput3D) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 0.5);
}
