// =============================================================================
// sky.wgsl — Phase 3.11.10 vanilla `gfx/FX/sky.shader`
// =============================================================================
//
// 远缩时地图边缘外的天空盒。vanilla 用 cubemap；本翻译同样。

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;

@group(2) @binding(0) var sky_cube: texture_cube<f32>;
@group(2) @binding(1) var sky_sampler: sampler;

struct VsIn {
    @location(0) corner: vec2<f32>, // (-1,-1)..(1,1) 全屏三角形或 quad
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = vec4<f32>(in.corner, 1.0, 1.0); // far plane
    // 反 view-proj 求方向
    let inv = transpose(frame.view_proj); // 简化：实际应用正确 inverse
    let world_h = inv * vec4<f32>(in.corner, 1.0, 1.0);
    out.view_dir = normalize(world_h.xyz / max(world_h.w, 0.0001));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dir = normalize(in.view_dir);
    var color = textureSample(sky_cube, sky_sampler, dir).rgb;

    // 昼夜：夜晚整体调暗 + 偏蓝
    let globe_n = calc_globe_normal(frame.cam_pos.xz, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    let night_color = vec3<f32>(0.05, 0.07, 0.15);
    color = mix(color, night_color, night * 0.7);

    return vec4<f32>(color, 1.0);
}
