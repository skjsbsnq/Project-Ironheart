// =============================================================================
// traderoute.wgsl — Phase 3.11.11 vanilla `gfx/FX/traderoute.shader`
// =============================================================================
//
// 贸易路线：流动虚线 + 颜色按贸易方向。

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct TradeParams {
    flow_speed: f32,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> t_params: TradeParams;

@group(2) @binding(0) var dash_tex: texture_2d<f32>;
@group(2) @binding(1) var dash_sampler: sampler;

struct VsIn {
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) trade_amount: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) amount: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.world_pos, 1.0);
    // 流动 UV：x 轴随时间滚动
    out.uv = vec2<f32>(in.uv.x - frame.global_time * t_params.flow_speed, in.uv.y);
    out.amount = in.trade_amount;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(dash_tex, dash_sampler, in.uv);
    if (s.a < 0.1) {
        discard;
    }
    let color = mix(t_params.color_a.rgb, t_params.color_b.rgb, in.amount);
    return vec4<f32>(color * s.rgb, s.a * t_params.color_a.a);
}
