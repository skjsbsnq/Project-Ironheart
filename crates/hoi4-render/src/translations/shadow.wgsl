// =============================================================================
// shadow.wgsl — Phase 3.11.12 directional shadow caster pass
// =============================================================================
//
// 单级 directional shadow map（对 RTS 俯视角度足够；vanilla 也是单级）。
// terrain + buildings + trees + units 顶点投到 shadow proj 的 depth-only RT。
// fragment shader 是空（depth-only），但保留以便未来加 alpha test (透明叶片 / 旗)。

//#include "global_uniform.wgsl"

struct ShadowParams {
    /// shadow_view_proj — main.rs 算的太阳方向 view × ortho proj
    shadow_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> shadow: ShadowParams;

// caster 自己的纹理（如果有 alpha test）
@group(2) @binding(0) var caster_alpha: texture_2d<f32>;
@group(2) @binding(1) var caster_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = shadow.shadow_view_proj * vec4<f32>(in.pos, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) {
    // alpha test for trees (cutout)
    let a = textureSample(caster_alpha, caster_sampler, in.uv).a;
    if (a < 0.5) {
        discard;
    }
    // depth-only — 不写颜色
}
