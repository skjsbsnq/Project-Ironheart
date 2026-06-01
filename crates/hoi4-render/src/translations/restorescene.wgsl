// =============================================================================
// restorescene.wgsl - Phase 5 postprocess reference translation
// =============================================================================
//
// Vanilla RestoreScene reflection evidence:
// - MainScene_Texture / MainScene_Sampler: t0 / s0
// - RestoreBloom_Texture / RestoreBloom_Sampler: t1 / s1
// - ColorCube_Texture / ColorCube_Sampler: t2 / s2
// - AverageLuminanceTexture_Texture / AverageLuminanceTexture_Sampler: t3 / s3
//
// The live hoi4-app pass maps these semantics to WGSL group 0 bindings below.
// Numeric WGSL bindings do not mirror D3D11 slots; the semantic order is kept
// explicit so project_mapping.md and shader_bindings.json remain traceable.

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;
@group(0) @binding(4) var lum_tex: texture_2d<f32>;
@group(0) @binding(5) var lum_sampler: sampler;
@group(0) @binding(6) var color_cube_tex: texture_2d_array<f32>;
@group(0) @binding(7) var color_cube_sampler: sampler;

struct RestoreParams {
    middle_grey: f32,
    bloom_strength: f32,
    srgb_target: f32,
    exposure_bias: f32,
    exposure_min: f32,
    exposure_max: f32,
    uncharted_white_point: f32,
    saturation: f32,
    debug_view: f32,
    bloom_debug_gain: f32,
    lut_strength: f32,
    lut_size: f32,
    hsv_hue_shift: f32,
    hsv_saturation: f32,
    hsv_value: f32,
    _pad0: f32,
    color_balance: vec4<f32>,
    lut_blend: vec4<f32>,
};
@group(0) @binding(8) var<uniform> rp: RestoreParams;

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

fn uncharted2_tonemap_partial(x: vec3<f32>) -> vec3<f32> {
    let a = 0.15;
    let b = 0.50;
    let c = 0.10;
    let d = 0.20;
    let e = 0.02;
    let f = 0.30;
    return ((x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f)) - e / f;
}

fn uncharted2_tonemap(color: vec3<f32>, white_point: f32) -> vec3<f32> {
    let curr = uncharted2_tonemap_partial(color);
    let white_scale = 1.0 / uncharted2_tonemap_partial(vec3<f32>(max(white_point, 0.001))).r;
    return clamp(curr * white_scale, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn restorescene_rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = select(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), c.b < c.g);
    let q = select(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), p.x < c.r);
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn restorescene_hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + k.xyz) * 6.0 - k.www);
    return c.z * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn apply_hsv(color: vec3<f32>) -> vec3<f32> {
    var hsv = restorescene_rgb_to_hsv(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)));
    hsv.x = fract(hsv.x + rp.hsv_hue_shift);
    hsv.y = clamp(hsv.y * rp.hsv_saturation, 0.0, 2.0);
    hsv.z = max(hsv.z * rp.hsv_value, 0.0);
    return restorescene_hsv_to_rgb(hsv);
}

fn apply_color_cube(color: vec3<f32>) -> vec3<f32> {
    let n = max(rp.lut_size, 2.0);
    let c = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let b_idx = c.b * (n - 1.0);
    let b_lo = floor(b_idx);
    let b_hi = min(b_lo + 1.0, n - 1.0);
    let b_frac = b_idx - b_lo;
    let inv_n = 1.0 / n;
    let cell_w = inv_n;
    let u_lo = (b_lo + c.r * (1.0 - inv_n) + 0.5 * inv_n) * cell_w;
    let u_hi = (b_hi + c.r * (1.0 - inv_n) + 0.5 * inv_n) * cell_w;
    let v = c.g * (1.0 - inv_n) + 0.5 * inv_n;
    let day_layer = i32(max(rp.lut_blend.x, 0.0));
    let night_layer = i32(max(rp.lut_blend.y, 0.0));
    let lo_day = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_lo, v), day_layer).rgb;
    let hi_day = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_hi, v), day_layer).rgb;
    let lo_night = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_lo, v), night_layer).rgb;
    let hi_night = textureSample(color_cube_tex, color_cube_sampler, vec2<f32>(u_hi, v), night_layer).rgb;
    let lut_day = mix(lo_day, hi_day, b_frac);
    let lut_night = mix(lo_night, hi_night, b_frac);
    let lut = mix(lut_day, lut_night, clamp(rp.lut_blend.z, 0.0, 1.0));
    return mix(c, lut, clamp(rp.lut_strength, 0.0, 1.0));
}

fn apply_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
    let lum = dot(color, vec3<f32>(0.2125, 0.7154, 0.0721));
    return mix(vec3<f32>(lum), color, saturation);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(hdr_tex, hdr_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let scene_with_bloom = scene + bloom * rp.bloom_strength;

    let raw_log_lum = textureSample(lum_tex, lum_sampler, vec2<f32>(0.5, 0.5)).r;
    let avg_log_lum = clamp(raw_log_lum, -8.0, 8.0);
    let avg_lum = max(exp(avg_log_lum), 1e-3);
    let exposure = clamp((rp.middle_grey / avg_lum) * rp.exposure_bias, rp.exposure_min, rp.exposure_max);
    let tonemap_input = scene_with_bloom * exposure;
    let tonemapped = uncharted2_tonemap(tonemap_input, rp.uncharted_white_point);

    var graded = apply_color_cube(tonemapped);
    graded = apply_hsv(graded);
    graded = apply_saturation(graded, rp.saturation);
    graded = max(graded + rp.color_balance.rgb, vec3<f32>(0.0));
    var ldr = clamp(graded, vec3<f32>(0.0), vec3<f32>(1.0));

    if (rp.debug_view > 0.5 && rp.debug_view < 1.5) {
        ldr = clamp(scene, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 1.5 && rp.debug_view < 2.5) {
        ldr = clamp(bloom * rp.bloom_debug_gain, vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 2.5 && rp.debug_view < 3.5) {
        let lum_debug = clamp(log2(max(avg_lum, 1e-4)) / 8.0 + 0.5, 0.0, 1.0);
        ldr = vec3<f32>(lum_debug);
    } else if (rp.debug_view > 3.5 && rp.debug_view < 4.5) {
        ldr = clamp(tonemap_input / max(rp.uncharted_white_point, 1.0), vec3<f32>(0.0), vec3<f32>(1.0));
    } else if (rp.debug_view > 4.5 && rp.debug_view < 5.5) {
        ldr = tonemapped;
    } else if (rp.debug_view > 5.5 && rp.debug_view < 6.5) {
        ldr = tonemapped;
    } else if (rp.debug_view > 6.5 && rp.debug_view < 7.5) {
        ldr = clamp(graded, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // Hardware sRGB encoding owns final gamma when the swapchain format is
    // Bgra8UnormSrgb. Manual gamma is only for non-sRGB targets.
    if (rp.srgb_target < 0.5) {
        ldr = pow(ldr, vec3<f32>(1.0 / 2.2));
    }

    return vec4<f32>(ldr, 1.0);
}
