// =============================================================================
// saturation_slider.wgsl — Phase 3.11.13 用户饱和度滑块
// =============================================================================
//
// 设置面板"画面饱和度"滑块的最后一步 post pass。

//#include "shader_lib.wgsl"

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct SatParams {
    saturation: f32, // 0 = 灰阶，1 = 原图，>1 = 过饱和
    _pad: vec3<f32>,
};
@group(0) @binding(2) var<uniform> sp: SatParams;

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
    let c = textureSample(src_tex, src_sampler, in.uv).rgb;
    let grey = vec3<f32>(dot(c, LUMINANCE_VECTOR_LOCAL));
    return vec4<f32>(mix(grey, c, sp.saturation), 1.0);
}
