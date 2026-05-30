// =============================================================================
// particle.wgsl — Phase 3.11.10 vanilla `gfx/FX/particle.shader`
// =============================================================================
//
// 单个粒子 = 朝向相机的 quad + soft alpha + 距离淡入淡出。

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct ParticleParams {
    fade_start: f32,
    fade_stop: f32,
    soft_thickness: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> p_params: ParticleParams;

@group(2) @binding(0) var particle_tex: texture_2d<f32>;
@group(2) @binding(1) var depth_tex: texture_depth_2d;
@group(2) @binding(2) var p_sampler: sampler;

struct VsIn {
    @location(0) world_pos: vec3<f32>,
    @location(1) size: f32,
    @location(2) tint: vec4<f32>,
    @location(3) corner: vec2<f32>, // (-1,-1)..(1,1)
    @location(4) uv: vec2<f32>,
    @location(5) angle: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) view_z: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;

    let to_cam = normalize(frame.cam_pos - in.world_pos);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(up, to_cam));
    let upr = cross(to_cam, right);

    // 旋转
    let cs = cos(in.angle);
    let sn = sin(in.angle);
    let local = vec2<f32>(
        in.corner.x * cs - in.corner.y * sn,
        in.corner.x * sn + in.corner.y * cs,
    ) * in.size;

    let world = in.world_pos + right * local.x + upr * local.y;
    out.clip_pos = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_pos = world;
    out.uv = in.uv;
    out.tint = in.tint;
    out.view_z = (frame.view_proj * vec4<f32>(world, 1.0)).z;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(particle_tex, p_sampler, in.uv);
    if (s.a < 0.01) {
        discard;
    }

    // 距离淡入淡出
    let d = length(in.world_pos - frame.cam_pos);
    let fade = 1.0 - smoothstep(p_params.fade_start, p_params.fade_stop, d);

    var color = s.rgb * in.tint.rgb;
    var alpha = s.a * in.tint.a * fade;

    return vec4<f32>(color * alpha, alpha);
}
