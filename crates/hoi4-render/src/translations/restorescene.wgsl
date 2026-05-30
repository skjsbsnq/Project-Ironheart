// =============================================================================
// restorescene.wgsl — Phase 3.11.13 vanilla `gfx/FX/restorescene.shader`
// =============================================================================
//
// HDR → SDR：ACES tonemap + gamma + 最终 vignette + bloom 叠加。
// 在 LUT pass 之后、saturation_slider 之前。

//#include "shader_lib.wgsl"

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;

struct RestoreParams {
    /// 平均亮度（来自 downsample_luminance）
    avg_log_lum: f32,
    /// 中灰目标
    middle_grey: f32,
    /// 白点平方
    lum_white2: f32,
    /// vignette 强度
    vignette_strength: f32,
    /// bloom 混合权重
    bloom_strength: f32,
    /// 屏幕 UV 中心（一般 0.5, 0.5）
    center_uv: vec2<f32>,
    _pad: f32,
};
@group(0) @binding(4) var<uniform> rp: RestoreParams;

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

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    // Narkowicz ACES 简化
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let LUMINANCE_VECTOR_LOCAL = vec3<f32>(0.2125, 0.7154, 0.0721);

    let scene = textureSample(hdr_tex, hdr_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let combined = scene + bloom * rp.bloom_strength;

    // 自动曝光：avg_log_lum → 线性平均 → 中灰目标
    let avg_lum = max(exp(rp.avg_log_lum), 1e-3);
    let exposure = rp.middle_grey / avg_lum;
    let exposed = combined * exposure;

    // ACES tonemap
    var ldr = aces_tonemap(exposed);

    // gamma 矫正（sRGB 输出 — 如果 swap chain 是 BGRA8UnormSrgb 这步可省略）
    ldr = pow(ldr, vec3<f32>(1.0 / 2.2));

    // vignette
    let d = distance(in.uv, rp.center_uv);
    let vig = 1.0 - smoothstep(0.4, 0.85, d) * rp.vignette_strength;
    ldr *= vig;

    return vec4<f32>(ldr, 1.0);
}
