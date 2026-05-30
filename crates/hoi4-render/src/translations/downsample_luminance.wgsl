// =============================================================================
// downsample_luminance.wgsl — Phase 3.11.13
// =============================================================================
//
// vanilla `downsample_luminance.shader` — 自动曝光：对场景做 log luminance
// reduction，得到当前帧平均亮度，驱动眼适应。

//#include "shader_lib.wgsl"

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct LumParams {
    inv_src_size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(2) var<uniform> lp: LumParams;

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
    let p = lp.inv_src_size;

    var sum_log_lum = 0.0;
    let offsets = array<vec2<f32>, 9>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 0.0, -1.0), vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  0.0), vec2<f32>( 0.0,  0.0), vec2<f32>( 1.0,  0.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 0.0,  1.0), vec2<f32>( 1.0,  1.0),
    );

    for (var i = 0; i < 9; i = i + 1) {
        let s = textureSample(src_tex, src_sampler, in.uv + offsets[i] * p).rgb;
        let lum = max(dot(s, LUMINANCE_VECTOR_LOCAL), 1e-4);
        sum_log_lum = sum_log_lum + log(lum);
    }
    let avg_log_lum = sum_log_lum / 9.0;
    return vec4<f32>(avg_log_lum, 0.0, 0.0, 1.0);
}
