// 显示着色器：将 compute 输出的 framebuffer 渲染到 render target
//
// 使用覆盖整个 viewport 的三角形，通过 @builtin(position) 索引 framebuffer。

// ── 顶点着色器：全屏三角形 ────────────────────────────────────────────

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    // 单个三角形覆盖整个 NDC 空间 [-1,1]×[-1,1]
    // 顶点: 0→(-1,-1), 1→(3,-1), 2→(-1,3)
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[vid], 0.0, 1.0);
}

// ── Uniform：仅传递 framebuffer 行跨度 ─────────────────────────────────

struct FsParams {
    fb_pitch: u32,  // framebuffer 每行像素数 (= screen_width)
}

@group(0) @binding(0) var<storage, read> framebuffer: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> fs_params: FsParams;

// ── 片元着色器 ────────────────────────────────────────────────────────

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    // wgpu 的 @builtin(position) 在片元着色器中给出视口坐标：
    //   x: [0, W) 从左到右
    //   y: [0, H) 从上到下
    // 这与 compute shader 中 framebuffer 的行索引方式一致
    let idx = u32(pos.y) * fs_params.fb_pitch + u32(pos.x);
    return framebuffer[idx];
}
