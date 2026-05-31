// =============================================================================
// border.wgsl — Phase 3.11.8 vanilla `gfx/FX/border.shader` 等价翻译
// =============================================================================
//
// 替换当前 SDF 边界（虽然 SDF 也保留作 fallback）。原版用 6 类 × 3 LOD = 18 张
// 预烘 SDF 风格纹理：
// - border_country / border_province / border_state /
//   border_sea / border_sea_region / border_impassable
//
// 每类 3 个 LOD（_0/_1/_2）。本 shader 按相机距离选 LOD + 类别选纹理 +
// gradient_border_apply 风格混合（来自 standardfuncsgfx.fxh）。

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct BorderParams {
    /// 相机距离归一化（0=近，1=远）
    cam_distance_norm: f32,
    /// 当前选中省份 / 国家高亮强度
    selection_intensity: f32,
    /// 各类边界全局开关 mask
    enabled_mask: u32,
    _pad: u32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> bparams: BorderParams;

@group(2) @binding(0) var border_country_lod0: texture_2d<f32>;
@group(2) @binding(1) var border_country_lod1: texture_2d<f32>;
@group(2) @binding(2) var border_country_lod2: texture_2d<f32>;
@group(2) @binding(3) var border_province_lod0: texture_2d<f32>;
@group(2) @binding(4) var border_province_lod1: texture_2d<f32>;
@group(2) @binding(5) var border_province_lod2: texture_2d<f32>;
@group(2) @binding(6) var border_state_lod0: texture_2d<f32>;
@group(2) @binding(7) var border_impassable_lod0: texture_2d<f32>;
@group(2) @binding(8) var border_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};
struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.pos, 1.0);
    out.world_pos = in.pos;
    out.uv = in.uv;
    return out;
}

/// LOD 选择：3 张 LOD 按 `cam_distance_norm` mix 出最终值。
fn sample_border_3lod(
    t0: texture_2d<f32>, t1: texture_2d<f32>, t2: texture_2d<f32>,
    uv: vec2<f32>,
) -> vec4<f32> {
    let s0 = textureSample(t0, border_sampler, uv);
    let s1 = textureSample(t1, border_sampler, uv);
    let s2 = textureSample(t2, border_sampler, uv);
    let d = bparams.cam_distance_norm;
    if (d < 0.5) {
        return mix(s0, s1, d * 2.0);
    } else {
        return mix(s1, s2, (d - 0.5) * 2.0);
    }
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var color = vec3<f32>(0.0);
    var alpha = 0.0;

    // 国境（最显眼、最粗）
    if ((bparams.enabled_mask & 1u) != 0u) {
        let s = sample_border_3lod(border_country_lod0, border_country_lod1, border_country_lod2, in.uv);
        color += s.rgb * s.a;
        alpha = max(alpha, s.a * 1.0);
    }
    // 省界（最细，远景隐藏）
    if ((bparams.enabled_mask & 2u) != 0u && bparams.cam_distance_norm < 0.7) {
        let s = sample_border_3lod(
            border_province_lod0, border_province_lod1, border_province_lod2, in.uv
        );
        color += s.rgb * s.a * (1.0 - bparams.cam_distance_norm);
        alpha = max(alpha, s.a * (1.0 - bparams.cam_distance_norm));
    }
    // 州界（中等粗细）
    if ((bparams.enabled_mask & 4u) != 0u) {
        let s = textureSample(border_state_lod0, border_sampler, in.uv);
        color += s.rgb * s.a * 0.7;
        alpha = max(alpha, s.a * 0.7);
    }
    // 不可通行（如阿尔卑斯山脉）
    if ((bparams.enabled_mask & 8u) != 0u) {
        let s = textureSample(border_impassable_lod0, border_sampler, in.uv);
        color += s.rgb * s.a;
        alpha = max(alpha, s.a);
    }

    if (alpha < 0.01) {
        discard;
    }

    // 选中高亮（脉冲）
    if (bparams.selection_intensity > 0.0) {
        let pulse = (sin(frame.global_time * 4.0) * 0.5 + 0.5) * bparams.selection_intensity;
        color += vec3<f32>(0.4, 0.4, 0.0) * pulse;
    }

    // 昼夜（夜间边界稍微去饱和）
    let map_px = world_xz_to_map_px(in.world_pos.xz, frame.vanilla_map_size_world_size.zw);
    let globe_n = calc_globe_normal(map_px, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    let grey = vec3<f32>(dot(color, LUMINANCE_VECTOR));
    color = mix(color, mix(color, grey, 0.2), night);

    return vec4<f32>(color, alpha);
}
