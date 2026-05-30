// =============================================================================
// shader_lib.wgsl — Phase 3.11.1
// =============================================================================
//
// `gfx/FX/standardfuncsgfx.fxh` (1046 lines HLSL) 的 wgsl 等价库。
//
// ## 用法
//
// 后续翻译的每个 vanilla shader 都通过 `compose_shader(SHADER_LIB_WGSL, ...)`
// 把本文件作为前缀注入；它**不导出**任何 `@vertex` / `@fragment` 入口，只提供
// 函数与常量。具体 binding 由调用方决定（本文件不持有 `@group/@binding`，
// 所有需要采样的纹理都作为函数参数传入）。
//
// ## 函数组（按职责）
//
// 1. **gamma**：`to_gamma` / `to_linear`
// 2. **rotation**：`rotate_vec_by_vec` / `rotate_vec_2d`
// 3. **HSV/RGB**：`hue` / `hsv_to_rgb` / `rgb_to_hsv` / `hue_post` / `hsv_to_rgb_post`
// 4. **mix helpers**：`get_overlay` / `levels1` / `levels3`
// 5. **camera & fog**：`cam_distance` / `calculate_distance_fog_factor` / `apply_distance_fog`
// 6. **globe / day-night**：`calc_globe_normal` / `day_night_factor` / `nightify_color` / `day_night`
// 7. **lighting (Blinn-Phong)**：`fresnel_schlick` / `improved_blinn_phong`（含 PBR-ish）
// 8. **misc**：`fmod_loop` / `unpack_normal`
//
// ## 与 Rust 端 [`crate::defines`] 的同步
//
// 本文件中所有 `const FOO: f32 = X;` 都对应 `defines.rs::FOO`。
// `tests/defines_match_shader_lib.rs` 用文本扫描两侧确保一致。
//
// ## 不实现（推迟到后续 phase）
//
// - `CalculateShadow` PCF 采样（3.11.12 阴影管线时再加）
// - `CalculatePointLights`（3.11.3 pdxmap 完整版才需要）
// - `gradient_border_*` 系列（3.11.8 border 翻译时再加）
// - `secondary_color_mask` / `dominance_fx_apply`（3.11.3 pdxmap 内）
// - `SampleWater` / `BlendLEAN`（3.11.5 pdxwater 内）
// - 调试 `DebugReturn`
//
// ## 与原版的差异
//
// HLSL `vec3(0.45)` / `pow(rgb, vec3(2.2))` 在 wgsl 用 `vec3<f32>(0.45)`；
// `tex2D` 走采样器函数参数化；`saturate(x)` → `clamp(x, 0.0, 1.0)`；
// `lerp` → `mix`；`fmod` → 自实现（wgsl 没 `fmod`，`%` 是整数取余）。

// -----------------------------------------------------------------------------
// 常量（与 defines.rs 对应；wgsl 这边再写一份是因为 wgsl 不能 #include Rust）
// -----------------------------------------------------------------------------

const LUMINANCE_VECTOR: vec3<f32> = vec3<f32>(0.2125, 0.7154, 0.0721);

// Fog tuned so vanilla-style "extreme distance haze only" — the gameplay
// camera (cam_y ≈ 6-25 units, look-at distance ≈ 50-150 units) lands
// almost entirely below FOG_BEGIN, so the map keeps full saturation and
// only a thin top-of-frame horizon haze remains visible at max zoom-out.
// Compare the previous (1.7×, 6.7×, 0.18) which baked 5-15% global haze
// into every wide-angle frame.
const FOG_COLOR: vec3<f32> = vec3<f32>(0.62, 0.72, 0.82);
const WORLD_EXTENT: f32 = 119.5;
const FOG_BEGIN: f32 = WORLD_EXTENT * 4.0;
const FOG_END: f32 = WORLD_EXTENT * 12.0;
const FOG_MAX: f32 = 0.025;

const FOW_CAMERA_MIN: f32 = 200.0;
const FOW_CAMERA_MAX: f32 = 500.0;

const SPECULAR_MULTIPLIER: f32 = 1.0;

const NIGHT_AMBIENT_BOOST: f32 = 3.0;
const NIGHT_OPACITY: f32 = 1.0;

const GMT_OFFSET: f32 = 2793.0;
const SOUTH_POLE_OFFSET: f32 = 0.17;
const NORTH_POLE_OFFSET: f32 = 0.93;
const FEATHER_MIN: f32 = -0.024;
const FEATHER_MAX: f32 = 0.024;
const MOON_FEATHER_MIN: f32 = -0.05;
const MOON_FEATHER_MAX: f32 = 0.05;
const GLOBE_NORMAL_LIMIT: f32 = 0.7;

const MAP_SIZE_X: f32 = 5632.0;
const MAP_SIZE_Y: f32 = 2048.0;

const SNOW_COLOR_LIB: vec3<f32> = vec3<f32>(0.46, 0.48, 0.69);
const ICE_COLOR_LIB: vec3<f32> = vec3<f32>(0.50, 0.60, 0.90);

// PI 倍数（vanilla 用 6.2831f / 3.1415f 字面量）
const TAU: f32 = 6.2831853;
const PI_LIB: f32 = 3.1415927;

const LIGHT_SHADOW_DIRECTION_X: f32 = -5.0;
const LIGHT_SHADOW_DIRECTION_Y: f32 = -8.0;
const LIGHT_SHADOW_DIRECTION_Z: f32 = 5.0;

const SHADOW_WEIGHT_TERRAIN_LIB: f32 = 0.7;

const MAP_ARROW_NORMALS_STR_TERR: f32 = 0.0125;
const MAP_ARROW_NORMALS_STR_WATER: f32 = 0.08;

// -----------------------------------------------------------------------------
// 工具：fmod_loop（HLSL `fmod` 在 wgsl 没有 builtin，自实现 floored）
// -----------------------------------------------------------------------------

fn fmod_loop(x: f32, m: f32) -> f32 {
    return x - m * floor(x / m);
}

// -----------------------------------------------------------------------------
// 1. Gamma
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:38` — `ToGamma(float)` 单分量
fn to_gamma_scalar(linear_in: f32) -> f32 {
    return pow(linear_in, 0.45);
}

/// `standardfuncsgfx.fxh:43` — `ToGamma(float3)`
fn to_gamma(linear_in: vec3<f32>) -> vec3<f32> {
    return pow(linear_in, vec3<f32>(0.45));
}

/// `standardfuncsgfx.fxh:48` — `ToLinear(float3)`
fn to_linear(gamma_in: vec3<f32>) -> vec3<f32> {
    return pow(gamma_in, vec3<f32>(2.2));
}

/// `standardfuncsgfx.fxh:53` — `ToLinear(float4)`，alpha 不变换
fn to_linear4(gamma_in: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(pow(gamma_in.rgb, vec3<f32>(2.2)), gamma_in.a);
}

// -----------------------------------------------------------------------------
// 2. Rotation
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:59` — `RotateVectorByVector(v1, v2)`
/// 把 v2（局部空间）旋转到由 v1 作为 z-up 的坐标系。
fn rotate_vec_by_vec(v1: vec3<f32>, v2: vec3<f32>) -> vec3<f32> {
    let z_axis = v1;
    let x_axis = normalize(cross(z_axis, vec3<f32>(0.0, 0.0, 1.0)));
    let y_axis = normalize(cross(x_axis, z_axis));
    return x_axis * v2.x + z_axis * v2.y + y_axis * v2.z;
}

/// `standardfuncsgfx.fxh:69` — `RotateVector2D(v, angle)`
fn rotate_vec_2d(v: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(v.x * c - v.y * s, v.y * c + v.x * s);
}

// -----------------------------------------------------------------------------
// 3. HSV / RGB
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:91` — `HuePost(H)`，0..6 段彩虹
fn hue_post(h: f32) -> vec3<f32> {
    let x = 1.0 - abs((fmod_loop(h, 2.0)) - 1.0);
    if (h < 1.0) { return vec3<f32>(1.0, x, 0.0); }
    else if (h < 2.0) { return vec3<f32>(x, 1.0, 0.0); }
    else if (h < 3.0) { return vec3<f32>(0.0, 1.0, x); }
    else if (h < 4.0) { return vec3<f32>(0.0, x, 1.0); }
    else if (h < 5.0) { return vec3<f32>(x, 0.0, 1.0); }
    else { return vec3<f32>(1.0, 0.0, x); }
}

/// `standardfuncsgfx.fxh:102` — `HSVtoRGBPost(hsv)`
fn hsv_to_rgb_post(hsv: vec3<f32>) -> vec3<f32> {
    if (hsv.y != 0.0) {
        let c = hsv.y * hsv.z;
        return clamp(hue_post(hsv.x) * c + (hsv.z - c), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    return clamp(vec3<f32>(hsv.z), vec3<f32>(0.0), vec3<f32>(1.0));
}

/// `standardfuncsgfx.fxh:112` — `RGBtoHSV(rgb)`
fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let cmax = max(rgb.r, max(rgb.g, rgb.b));
    let cmin = min(rgb.r, min(rgb.g, rgb.b));
    let diff = cmax - cmin;

    var h: f32 = 0.0;
    var s: f32 = 0.0;
    if (diff != 0.0) {
        s = diff / cmax;
        if (cmax == rgb.r) {
            h = (rgb.g - rgb.b) / diff + 6.0;
        } else if (cmax == rgb.g) {
            h = (rgb.b - rgb.r) / diff + 2.0;
        } else {
            h = (rgb.r - rgb.g) / diff + 4.0;
        }
        h = fmod_loop(h, 6.0);
    }
    return vec3<f32>(h, s, cmax);
}

/// `standardfuncsgfx.fxh:136` — `Hue(H)` (saturate 版的 H)
fn hue(h: f32) -> vec3<f32> {
    let r = abs(h * 6.0 - 3.0) - 1.0;
    let g = 2.0 - abs(h * 6.0 - 2.0);
    let b = 2.0 - abs(h * 6.0 - 4.0);
    return clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));
}

/// `standardfuncsgfx.fxh:145` — `HSVtoRGB(H, S, V)` → linear
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let hue_rgb = hue(h);
    let val = (hue_rgb - vec3<f32>(1.0)) * s + vec3<f32>(1.0);
    return to_linear(val * v);
}

// -----------------------------------------------------------------------------
// 4. Mix helpers
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:159` — `GetOverlay(color, overlay, percent)`
/// Photoshop "overlay" 混合模式（在 gamma 空间做，最后回 linear）。
fn get_overlay(color: vec3<f32>, overlay: vec3<f32>, percent: f32) -> vec3<f32> {
    let cg = to_gamma(color);
    let og = to_gamma(overlay);

    var res: vec3<f32>;
    if (og.r < 0.5) { res.r = 2.0 * og.r * cg.r; } else { res.r = 1.0 - 2.0 * (1.0 - og.r) * (1.0 - cg.r); }
    if (og.g < 0.5) { res.g = 2.0 * og.g * cg.g; } else { res.g = 1.0 - 2.0 * (1.0 - og.g) * (1.0 - cg.g); }
    if (og.b < 0.5) { res.b = 2.0 * og.b * cg.b; } else { res.b = 1.0 - 2.0 * (1.0 - og.b) * (1.0 - cg.b); }

    return mix(color, to_linear(res), percent);
}

/// `standardfuncsgfx.fxh:172` — `Levels(vec3, min, max)`：把 [min, 1] 重映射到 [0, 1]
/// 注意 vanilla 实现里 `vMaxInput` 似乎有 typo（公式只用了 vMin），这里**保留**
/// 原版 bug 行为以保证视觉等价。
fn levels3(v: vec3<f32>, min_in: f32, _max_in: f32) -> vec3<f32> {
    return clamp(v - vec3<f32>(min_in), vec3<f32>(0.0), vec3<f32>(1.0));
}

/// `standardfuncsgfx.fxh:179` — `Levels(float, min, max)`：标量版本，**正常**线性映射。
fn levels1(value: f32, min_in: f32, max_in: f32) -> f32 {
    return clamp((value - min_in) / (max_in - min_in), 0.0, 1.0);
}

// -----------------------------------------------------------------------------
// 5. Camera & fog
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:184` — `cam_distance(min, max)`：相机高度归一化到 [0, 1]
/// 调用方传入相机 Y 坐标（vanilla 在 const buffer 里取 `vCamPos.y`）。
fn cam_distance_y(cam_pos_y: f32, min_d: f32, max_d: f32) -> f32 {
    let cy = clamp(cam_pos_y, min_d, max_d);
    return (cy - min_d) / (max_d - min_d);
}

/// `standardfuncsgfx.fxh:203` — `CalculateDistanceFogFactor`
fn calculate_distance_fog_factor(
    world_pos: vec3<f32>,
    cam_pos: vec3<f32>,
) -> f32 {
    let diff = world_pos - cam_pos;
    let fog_dir_factor = 1.0 - abs(normalize(diff).y);
    let sq = dot(diff, diff);

    let v_begin = FOG_BEGIN * FOG_BEGIN;
    let v_end = FOG_END * FOG_END;

    let v_min = min((sq - v_begin) / (v_end - v_begin), FOG_MAX);
    return clamp(v_min, 0.0, 1.0) * fog_dir_factor;
}

/// `standardfuncsgfx.fxh:221` — `ApplyDistanceFog(color, factor)`
fn apply_distance_fog_factor(color: vec3<f32>, factor: f32) -> vec3<f32> {
    return mix(color, FOG_COLOR, factor);
}

/// `standardfuncsgfx.fxh:226` — `ApplyDistanceFog(color, world_pos)`
fn apply_distance_fog(color: vec3<f32>, world_pos: vec3<f32>, cam_pos: vec3<f32>) -> vec3<f32> {
    return apply_distance_fog_factor(color, calculate_distance_fog_factor(world_pos, cam_pos));
}

// -----------------------------------------------------------------------------
// 6. Globe normal & day/night
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:347` — `CalcGlobeNormal(world_xz)`
/// 把地图 XZ 坐标 + 当前小时（day_night_hour ∈ [0,1]）反投影到球面法线。
fn calc_globe_normal(world_xz: vec2<f32>, day_night_hour: f32) -> vec3<f32> {
    var x = fmod_loop((world_xz.x - GMT_OFFSET) / MAP_SIZE_X + day_night_hour, 1.0);
    var y = world_xz.y / MAP_SIZE_Y;
    y = SOUTH_POLE_OFFSET + (NORTH_POLE_OFFSET - SOUTH_POLE_OFFSET) * y;
    y = -cos(y * PI_LIB);
    let xz_len = 1.0 - abs(y);
    return normalize(vec3<f32>(sin(x * TAU) * xz_len, y, cos(x * TAU) * xz_len));
}

/// `standardfuncsgfx.fxh:358` — `DayNightFactor(globe_normal, min, max)`
/// 接近 1 → 夜，接近 0 → 日。
fn day_night_factor_range(
    globe_normal: vec3<f32>,
    sun_dir_yzw: vec3<f32>,
    fade_factor: f32,
    min_v: f32,
    max_v: f32,
) -> f32 {
    let d = dot(globe_normal, sun_dir_yzw);
    return clamp((d - min_v) / (max_v - min_v), 0.0, 1.0) * fade_factor;
}

/// `standardfuncsgfx.fxh:365` — 默认 feather 范围
fn day_night_factor(globe_normal: vec3<f32>, sun_dir_yzw: vec3<f32>, fade_factor: f32) -> f32 {
    return day_night_factor_range(globe_normal, sun_dir_yzw, fade_factor, FEATHER_MIN, FEATHER_MAX);
}

/// `standardfuncsgfx.fxh:370` — `NightifyColor(day_color, blend)`
/// 把白天颜色压成偏蓝灰的"夜景"。
fn nightify_color(day_color: vec3<f32>, blend: f32) -> vec3<f32> {
    let desat = mix(0.0, 0.8, blend * blend * blend);
    let grey = dot(day_color, vec3<f32>(0.4, 0.3, 0.05));
    let night = clamp(mix(vec3<f32>(grey), grey * vec3<f32>(0.2, 0.7, 1.2), vec3<f32>(0.25)), vec3<f32>(0.0), vec3<f32>(1.0));
    return mix(day_color, night, vec3<f32>(desat));
}

/// `standardfuncsgfx.fxh:384` — `DayNightWithBlend`
fn day_night_with_blend(
    day_color: vec3<f32>,
    globe_normal: vec3<f32>,
    sun_dir_yzw: vec3<f32>,
    fade_factor: f32,
    blend: f32,
) -> vec3<f32> {
    let factor = day_night_factor(globe_normal, sun_dir_yzw, fade_factor);
    return mix(day_color, nightify_color(day_color, blend), factor * NIGHT_OPACITY);
}

/// `standardfuncsgfx.fxh:397` — `DayNight(day_color, globe_normal)`
fn day_night(
    day_color: vec3<f32>,
    globe_normal: vec3<f32>,
    sun_dir_yzw: vec3<f32>,
    fade_factor: f32,
) -> vec3<f32> {
    return day_night_with_blend(day_color, globe_normal, sun_dir_yzw, fade_factor, 1.0);
}

// -----------------------------------------------------------------------------
// 7. Lighting (Blinn-Phong / Fresnel)
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:498` — `FresnelSchlick`
fn fresnel_schlick(specular_color: vec3<f32>, e: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    return specular_color + (vec3<f32>(1.0) - specular_color) * pow(1.0 - clamp(dot(e, h), 0.0, 1.0), 5.0);
}

/// `standardfuncsgfx.fxh:504` — `FresnelGlossy`
fn fresnel_glossy(specular_color: vec3<f32>, e: vec3<f32>, n: vec3<f32>, smoothness: f32) -> vec3<f32> {
    let edge = max(vec3<f32>(smoothness), specular_color);
    return specular_color + (edge - specular_color) * pow(1.0 - clamp(dot(e, n), 0.0, 1.0), 5.0);
}

/// `standardfuncsgfx.fxh:532` — `CalculateLight(normal, light_dir, light_intensity)`
/// 返回 Lambertian 漫反射贡献。
fn calculate_light_lambert(normal: vec3<f32>, light_dir: vec3<f32>, light_color: vec3<f32>) -> vec3<f32> {
    let n_dot_l = dot(normal, -light_dir);
    return max(n_dot_l, 0.0) * light_color;
}

/// `standardfuncsgfx.fxh:567` — `ImprovedBlinnPhong`
/// `non_linear_glossiness = exp2(11 * gloss)`（参见 `GetNonLinearGlossiness`）
fn improved_blinn_phong(
    light_color: vec3<f32>,
    to_light_dir: vec3<f32>,
    to_camera_dir: vec3<f32>,
    normal: vec3<f32>,
    specular_color: vec3<f32>,
    non_linear_glossiness: f32,
) -> BlinnPhongOut {
    let h = normalize(to_camera_dir + to_light_dir);
    let n_dot_l = clamp(dot(normal, to_light_dir), 0.0, 1.0);
    let n_dot_h = clamp(dot(normal, h), 0.0, 1.0);

    let normalization = (non_linear_glossiness + 2.0) / 8.0;
    let spec = normalization * pow(n_dot_h, non_linear_glossiness)
        * fresnel_schlick(specular_color, to_light_dir, h);

    var out: BlinnPhongOut;
    out.diffuse = light_color * n_dot_l;
    out.specular = light_color * spec * n_dot_l;
    return out;
}

struct BlinnPhongOut {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
};

/// `standardfuncsgfx.fxh:557` — `GetNonLinearGlossiness`
fn get_non_linear_glossiness(glossiness: f32) -> f32 {
    return exp2(11.0 * glossiness);
}

/// `standardfuncsgfx.fxh:562` — `GetEnvmapMipLevel`
fn get_envmap_mip_level(glossiness: f32) -> f32 {
    return (1.0 - glossiness) * 8.0;
}

// -----------------------------------------------------------------------------
// 8. Misc
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:303` — `UnpackNormal(sampler, uv)`
/// 调用方传入已采样的 RGB（0..1）。
fn unpack_normal(rgb: vec3<f32>) -> vec3<f32> {
    return normalize(rgb - vec3<f32>(0.5));
}

/// `standardfuncsgfx.fxh:311` — `UnpackRRxGNormal`：DXT5nm 风格 BC5/RG 法线解包
fn unpack_rrxg_normal(sample_rgba: vec4<f32>) -> vec3<f32> {
    let x = sample_rgba.a * 2.0 - 1.0;
    let y = sample_rgba.g * 2.0 - 1.0;
    let z = sqrt(clamp(1.0 - x * x - y * y, 0.0, 1.0));
    return vec3<f32>(x, y, z);
}

// -----------------------------------------------------------------------------
// 9. 雪线 / 雪覆盖（pdxmap 与 pdxmesh 共用，公开此函数为辅助）
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:259` — `GetSnow(mud_snow_color)`：从 mud/snow 4 通道纹理取雪强度
/// 通道含义：r=mud_now, g=snow_winter, b=snow_now, a=mud_winter。`vMudSnowFade ∈ [0, 1]`
/// 在春→冬之间插值。
fn get_snow(mud_snow_color: vec4<f32>, mud_snow_fade: f32) -> f32 {
    return mix(mud_snow_color.b, mud_snow_color.g, mud_snow_fade);
}

// -----------------------------------------------------------------------------
// 10. 占用条纹 / 国境条纹（用于 secondary color mask）
// -----------------------------------------------------------------------------

/// `standardfuncsgfx.fxh:825` — `CalculateBorderStripes(uv)`
/// 返回 [-0.03, 0.01] 的轻微亮度调制。
fn calculate_border_stripes(uv: vec2<f32>, global_time: f32, cam_pos_y: f32) -> f32 {
    let blink_speed = 1.5;
    let stripe_width = 350.0;
    let t = fmod_loop(global_time * blink_speed, TAU);
    var stripe = cos(uv.x * cos(t) * stripe_width + uv.y * sin(t) * stripe_width);

    let cam_d = cam_distance_y(cam_pos_y, 100.0, 200.0);
    stripe = smoothstep(0.0, 1.0, stripe * 2.0) * mix(1.0, 0.3, cam_d);
    return mix(mix(-0.03, 0.01, stripe), 0.0, cam_d);
}

/// `standardfuncsgfx.fxh:965` — `CalculateOccupationMask(uv)`
fn calculate_occupation_mask(uv: vec2<f32>, global_time: f32, cam_pos_y: f32) -> f32 {
    let blink_speed = 1.5;
    let stripe_width = 200.0;
    let t = fmod_loop(global_time * blink_speed, TAU);
    var stripe = cos(uv.x * cos(t) * stripe_width + uv.y * sin(t) * stripe_width);

    let cam_d = cam_distance_y(cam_pos_y, 300.0, 1200.0);
    stripe = smoothstep(0.0, 1.0, stripe * 1.7) * mix(1.0, 0.3, cam_d);
    return stripe;
}

// =============================================================================
// 库结束 — 调用方应在此之后追加自己的 @vertex / @fragment 入口
// =============================================================================
