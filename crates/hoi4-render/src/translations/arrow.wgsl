// =============================================================================
// arrow.wgsl — Phase 3.11.11 vanilla `gfx/FX/arrow.shader` (基础箭头)
// =============================================================================
//
// 通用基础箭头 — direction pointer / waypoint 等的简单贴片实现。

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct BaseArrowParams {
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> ba_params: BaseArrowParams;

@group(2) @binding(0) var arrow_tex: texture_2d<f32>;
@group(2) @binding(1) var arrow_sampler: sampler;

struct VsIn {
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.world_pos, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(arrow_tex, arrow_sampler, in.uv);
    if (s.a < 0.05) {
        discard;
    }
    return vec4<f32>(s.rgb * ba_params.color.rgb, s.a * ba_params.color.a);
}
