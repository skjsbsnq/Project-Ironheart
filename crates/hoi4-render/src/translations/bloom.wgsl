// =============================================================================
// bloom.wgsl — Phase 3.11.13 vanilla `gfx/FX/bloom.shader`
// =============================================================================
//
// 高光 bright-pass + 5 级模糊 + 上采样混合。本 shader 是 bright-pass + 一次
// 模糊；多级链由 main.rs 调度 N 个 bloom pass 完成。

//#include "shader_lib.wgsl"

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct BloomParams {
    bright_threshold: f32,
    bloom_strength: f32,
    inv_size_x: f32,
    inv_size_y: f32,
};
@group(0) @binding(2) var<uniform> bp: BloomParams;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let LUMINANCE_VECTOR_LOCAL = vec3<f32>(0.2125, 0.7154, 0.0721);

    // 中心 + 4 邻居 9-tap 取平均（小高斯）
    let p = vec2<f32>(bp.inv_size_x, bp.inv_size_y);
    let c0 = textureSample(src_tex, src_sampler, in.uv).rgb;
    let c1 = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 1.0,  0.0)).rgb;
    let c2 = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>(-1.0,  0.0)).rgb;
    let c3 = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 0.0,  1.0)).rgb;
    let c4 = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 0.0, -1.0)).rgb;
    let avg = (c0 * 4.0 + c1 + c2 + c3 + c4) / 8.0;

    // bright-pass：去掉 < threshold 的部分
    let lum = dot(avg, LUMINANCE_VECTOR_LOCAL);
    let factor = max(lum - bp.bright_threshold, 0.0) / max(lum, 1e-4);

    return vec4<f32>(avg * factor * bp.bloom_strength, 1.0);
}
