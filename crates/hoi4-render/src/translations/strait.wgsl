// =============================================================================
// strait.wgsl — Phase 3.11.11 vanilla `gfx/FX/strait.shader`
// =============================================================================
//
// 海峡过道（直布罗陀 / 丹麦海峡等特殊连接）。视觉是横跨海峡的虚线 + 流动箭头。

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct StraitParams {
    color: vec4<f32>,
    pulse_speed: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> s_params: StraitParams;

@group(2) @binding(0) var strait_tex: texture_2d<f32>;
@group(2) @binding(1) var strait_sampler: sampler;

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
    let s = textureSample(strait_tex, strait_sampler, in.uv);
    if (s.a < 0.05) { discard; }
    let pulse = 0.5 + 0.5 * cos(frame.global_time * s_params.pulse_speed);
    return vec4<f32>(s_params.color.rgb * (0.6 + 0.4 * pulse), s.a * s_params.color.a);
}
