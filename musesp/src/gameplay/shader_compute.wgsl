// Compute shader：完整软件光栅化管线
//
// 管线阶段（全部在 compute 中实现）：
//   1. 顶点变换（view_proj × position，旋转已在 CPU 完成）
//   2. 透视除法 → NDC
//   3. 视口变换 → 屏幕坐标
//   4. 三角形光栅化（边函数检测）
//   5. 收集片段并插入排序（深度降序，远→近 = 后→前）
//   6. Alpha 混合

// ── 结构体 ────────────────────────────────────────────────────────────

struct Vertex {
    position: vec3<f32>,
    color: vec4<f32>,
}

struct Params {
    view_proj: mat4x4<f32>,
    triangle_count: u32,
    screen_width: f32,
    screen_height: f32,
}

// ── 绑定 ──────────────────────────────────────────────────────────────

@group(0) @binding(0) var<storage, read> vertices: array<Vertex>;
@group(0) @binding(1) var<storage, read> indices: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> framebuffer: array<vec4<f32>>;

// ── 常量 ──────────────────────────────────────────────────────────────

const CLEAR_COLOR: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 0.0);
const MAX_FRAGMENTS: u32 = 32u;

// ── 边函数：2D 叉积 ───────────────────────────────────────────────────

fn edge(a: vec2<f32>, b: vec2<f32>, p: vec2<f32>) -> f32 {
    return (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
}

// ── 入口 ──────────────────────────────────────────────────────────────

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let px = global_id.x;
    let py = global_id.y;

    if px >= u32(params.screen_width) || py >= u32(params.screen_height) {
        return;
    }

    let pixel_center = vec2<f32>(f32(px) + 0.5, f32(py) + 0.5);
    let fb_idx = py * u32(params.screen_width) + px;
    let w = params.screen_width;
    let h = params.screen_height;

    // ── 第一遍：收集所有覆盖该像素的片段 ──
    var frag_depths: array<f32, MAX_FRAGMENTS>;
    var frag_colors: array<vec4<f32>, MAX_FRAGMENTS>;
    var frag_count: u32 = 0u;

    for (var tri: u32 = 0u; tri < params.triangle_count; tri += 1u) {
        let base = tri * 3u;
        let i0 = indices[base];
        let i1 = indices[base + 1u];
        let i2 = indices[base + 2u];

        let v0 = vertices[i0];
        let v1 = vertices[i1];
        let v2 = vertices[i2];

        // 顶点变换：world → clip
        let clip0 = params.view_proj * vec4<f32>(v0.position, 1.0);
        let clip1 = params.view_proj * vec4<f32>(v1.position, 1.0);
        let clip2 = params.view_proj * vec4<f32>(v2.position, 1.0);

        if clip0.w <= 0.0 && clip1.w <= 0.0 && clip2.w <= 0.0 {
            continue;
        }

        // 透视除法 → NDC
        let ndc0 = clip0.xyz / clip0.w;
        let ndc1 = clip1.xyz / clip1.w;
        let ndc2 = clip2.xyz / clip2.w;

        // 视口变换
        let sc0 = vec2<f32>((ndc0.x + 1.0) * 0.5 * w, (1.0 - ndc0.y) * 0.5 * h);
        let sc1 = vec2<f32>((ndc1.x + 1.0) * 0.5 * w, (1.0 - ndc1.y) * 0.5 * h);
        let sc2 = vec2<f32>((ndc2.x + 1.0) * 0.5 * w, (1.0 - ndc2.y) * 0.5 * h);

        // 边函数检测：e_i 与 tri_area 同号则为内部像素
        // tri_area > 0 时 e_i > 0（正面）；tri_area < 0 时 e_i < 0（背面）
        let tri_area = -edge(sc0, sc1, sc2);
        let e0 = -edge(sc1, sc2, pixel_center);
        let e1 = -edge(sc2, sc0, pixel_center);
        let e2 = -edge(sc0, sc1, pixel_center);

        // 任意 e_i 与 tri_area 异号 = 像素在三角形外
        if e0 * tri_area < 0.0 || e1 * tri_area < 0.0 || e2 * tri_area < 0.0 || abs(tri_area) < 1e-7 {
            continue;
        }

        // 重心插值：NDC 深度 + 颜色
        let inv_area = 1.0 / tri_area;
        let depth = (ndc0.z * e0 + ndc1.z * e1 + ndc2.z * e2) * inv_area;
        let color = (v0.color * e0 + v1.color * e1 + v2.color * e2) * inv_area;

        // ── 插入排序：按深度降序（NDC z 大 = 远，先处理） ──
        // 找到插入位置：深度比当前元素大的位置
        var insert_pos: u32 = frag_count;
        for (var k: u32 = 0u; k < frag_count; k += 1u) {
            if depth > frag_depths[k] {
                insert_pos = k;
                break;
            }
        }
        // 后移 [insert_pos, frag_count) 的元素，腾出空位
        if frag_count < MAX_FRAGMENTS {
            for (var k: u32 = frag_count; k > insert_pos; k -= 1u) {
                frag_depths[k] = frag_depths[k - 1u];
                frag_colors[k] = frag_colors[k - 1u];
            }
            frag_depths[insert_pos] = depth;
            frag_colors[insert_pos] = color;
            frag_count += 1u;
        }
    }

    // ── Alpha 混合（后→前，straight-alpha over 算子） ──
    var result = CLEAR_COLOR;
    for (var i: u32 = 0u; i < frag_count; i += 1u) {
        let c = frag_colors[i];
        // straight-alpha over: dst = src + dst * (1 - src.a)
        result = vec4<f32>(
            c.rgb * c.a + result.rgb * (1.0 - c.a),
            c.a + result.a * (1.0 - c.a),
        );
    }
    framebuffer[fb_idx] = result;
}
