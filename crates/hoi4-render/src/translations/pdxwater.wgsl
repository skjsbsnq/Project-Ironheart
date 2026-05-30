// =============================================================================
// pdxwater.wgsl — Phase 3.11.5 vanilla `gfx/FX/pdxwater.shader` 等价翻译
// =============================================================================
//
// **替代谁**：当前 `shader.wgsl` 内嵌的水面 fbm 噪声路径（程序化伪造）。
// 本文件按 vanilla pdxwater.shader 真实着色：
//
// 1. **WaterNormal LEAN**：BlendLEAN 4 时多频混合
// 2. **`reflection.dds`** 平面反射 + `EnvironmentMap` 立方体反射 mix
// 3. **Fresnel** 边缘高光
// 4. **`fow_rgb_waterspec_a.dds`** A 通道高光遮罩
// 5. **`colormap_water_{0,1,2}.dds`** 海域 LOD 基色
// 6. **海岸 foam**（保留 coast SDF）
// 7. **冰层**：高纬度叠加 ice_diffuse + 噪声

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct WaterParams {
    time_speed: f32,
    fresnel_power: f32,
    foam_threshold: f32,
    ice_latitude: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> wparams: WaterParams;

@group(1) @binding(0) var environment_cube: texture_cube<f32>;
@group(1) @binding(1) var environment_sampler: sampler;

@group(2) @binding(0) var water_normal_lean1: texture_2d<f32>;
@group(2) @binding(1) var water_normal_lean2: texture_2d<f32>;
@group(2) @binding(2) var reflection_tex: texture_2d<f32>;
@group(2) @binding(3) var fow_water_spec: texture_2d<f32>;
@group(2) @binding(4) var colormap_water: texture_2d<f32>;
@group(2) @binding(5) var ice_diffuse: texture_2d<f32>;
@group(2) @binding(6) var ice_noise: texture_2d<f32>;
@group(2) @binding(7) var coast_sdf: texture_2d<f32>;
@group(2) @binding(8) var water_sampler: sampler;

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

/// 多频水面法线（vanilla `SampleWater` 4 频混合的 wgsl 简化）。
fn sample_water_normal(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let t = time * 0.02;
    let n1 = textureSample(water_normal_lean1, water_sampler, uv * 0.9 + vec2<f32>(t, t * 0.1)).rgb;
    let n2 = textureSample(water_normal_lean2, water_sampler, uv * 1.05 + vec2<f32>(-t * 0.6, t * 2.0)).rgb;
    let n3 = textureSample(water_normal_lean1, water_sampler, uv * 0.75 + vec2<f32>(-t * 0.2, -t * 2.0)).rgb;
    let n4 = textureSample(water_normal_lean2, water_sampler, uv * 0.5 + vec2<f32>(-t, -t * 0.1)).rgb;
    return normalize((unpack_normal(n1) + unpack_normal(n2) + unpack_normal(n3) + unpack_normal(n4)) / 4.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let world_pos = in.world_pos;

    // 1. 水面法线（多频混合）
    let normal = sample_water_normal(in.uv, frame.global_time * wparams.time_speed);

    // 2. 基色（colormap_water LOD 提供深海 / 近海 / 极地差异）
    let base = textureSample(colormap_water, water_sampler, in.uv).rgb;

    // 3. 环境立方体反射
    let to_camera = normalize(frame.cam_pos - world_pos);
    let reflect_dir = reflect(-to_camera, normal);
    let env = textureSample(environment_cube, environment_sampler, reflect_dir).rgb;

    // 4. 平面反射（reflection.dds 是离屏 RT；这里直接采 UV 占位）
    let plane_refl = textureSample(reflection_tex, water_sampler, in.uv).rgb;

    // 5. Fresnel
    let fresnel_t = pow(1.0 - max(dot(normal, to_camera), 0.0), wparams.fresnel_power);
    let reflected = mix(plane_refl, env, 0.5);

    // 6. 太阳高光（用 fow_rgb_waterspec_a 的 A 通道遮罩）
    let spec_mask = textureSample(fow_water_spec, water_sampler, in.uv).a;
    let sun_dir = normalize(vec3<f32>(-0.408, -0.816, 0.408));
    let to_light = -sun_dir;
    let h = normalize(to_camera + to_light);
    let n_dot_h = max(dot(normal, h), 0.0);
    let sun_spec = pow(n_dot_h, 256.0) * spec_mask * frame.sun_specular_intensity;

    var color = mix(base, reflected, fresnel_t * 0.7);
    color += vec3<f32>(sun_spec);

    // 7. 海岸 foam（cost SDF 越靠近 0 越是海岸）
    let coast_d = textureSample(coast_sdf, water_sampler, in.uv).r;
    let foam = (1.0 - smoothstep(0.0, wparams.foam_threshold, coast_d)) * 0.85;
    color = mix(color, vec3<f32>(0.95, 0.97, 1.0), foam);

    // 8. 冰层（高纬度 + 噪声）
    let lat_t = abs(in.uv.y - 0.5) * 2.0;
    if (lat_t > wparams.ice_latitude) {
        let ice = textureSample(ice_diffuse, water_sampler, in.uv * 4.0).rgb;
        let n = textureSample(ice_noise, water_sampler, in.uv * 8.0).r;
        let ice_t = smoothstep(wparams.ice_latitude, wparams.ice_latitude + 0.1, lat_t) * n;
        color = mix(color, ice, ice_t);
    }

    // 9. 昼夜
    let globe_n = calc_globe_normal(world_pos.xz, frame.day_night_hour_sun_dir.x);
    color = day_night(color, globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);

    // 10. 距离雾
    color = apply_distance_fog(color, world_pos, frame.cam_pos);

    return vec4<f32>(color, 1.0);
}
