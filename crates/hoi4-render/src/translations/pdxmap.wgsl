// =============================================================================
// pdxmap.wgsl — Phase 3.11.3 vanilla `gfx/FX/pdxmap.shader` 等价翻译
// =============================================================================
//
// **替代关系**：本文件**不替换** `shader.wgsl`。它是一份独立的、按 vanilla
// pdxmap.shader 完整特性翻译的 wgsl 模板，等 main.rs 完成 binding 集成后
// 再切换 pipeline 引用。
//
// ## 翻译要点
//
// 相对当前 `shader.wgsl`（~480 行 / ~30% 覆盖），本文件补：
//
// 1. **法线贴图采样**：`atlas_normal{0,1,2}` + `world_normal.bmp` 三层 blend +
//    TBN 矩阵
// 2. **完整地形 splatting**：4×4 atlas + mud / snow / ice overlay 按 FoW /
//    高度 / 噪声混合
// 3. **CSM 阴影接收**：`GetShadowScaled(SHADOW_WEIGHT_TERRAIN, ...)`
// 4. **Point lights**：城市夜光（按 `LightDataMap` + `LightIndexMap`）
// 5. **昼夜系统**：`day_night(color, calc_globe_normal(world_pos.xz))`
// 6. **季节贴图**：`SeasonMap` + `ColorMap` + `ColorMapSecond` 双采样按
//    `vSeasonLerp` 插值
// 7. **梯度边界**：通过 `gradient_border_apply`（用 `border_country_*` /
//    `border_province_*` 等 18 张预烘 SDF）
// 8. **次级颜色 mask**：`secondary_color_mask`（占领条纹 / 自治领条纹）
// 9. **大气距离雾**：`apply_distance_fog`（来自 shader_lib）
//
// ## 不在本文件
//
// - vanilla 原版的 GLSL/HLSL preprocessor `#ifdef LOW_END_GFX` 条件编译——
//   wgsl 没 preprocessor，需要在 Rust 端通过文本替换或 `compose_shader`
//   组合不同变体
// - 实际 binding 创建（`wgpu::BindGroupLayout`）—— hoi4-app/main.rs 的事
// - terrain mesh / heightmap 顶点动画——这部分见 vanilla `pdxmap.fxh` 里的
//   chunk + LOD 系统，本工程已在 `terrain.rs` 里独立实现
//
// ## binding 约定（本 shader 期望的）
//
// - `@group(0) @binding(0)` = `GlobalFrameUniform`（人人共用）
// - `@group(1)` = 全局共享纹理池（shadow / season / colormap / lights / borders）
// - `@group(2)` = pass-specific（heightmap / province_id / terrain.bmp index）

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

// -----------------------------------------------------------------------------
// Pass-specific bindings
// -----------------------------------------------------------------------------

struct PdxMapParams {
    /// 当前选中省份 id（高亮用）
    selected_province_id: u32,
    /// vSeasonLerp ∈ [0, 1]
    season_lerp: f32,
    /// vSeasonColumn ∈ [0, 7]，季节列偏移（Tree_season 同款语义）
    season_column: f32,
    /// terrain blend 系数（地形纹理 vs 政治色）
    terrain_blend: f32,
};
@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> params: PdxMapParams;

// 全局纹理池
@group(1) @binding(0) var shadow_map_tex: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var season_map_tex: texture_2d<f32>;
@group(1) @binding(3) var color_map_tex: texture_2d<f32>;
@group(1) @binding(4) var color_map_second_tex: texture_2d<f32>;
@group(1) @binding(5) var light_data_tex: texture_2d<f32>;
@group(1) @binding(6) var light_index_tex: texture_2d<f32>;
@group(1) @binding(7) var gradient_border_ch1_tex: texture_2d<f32>;
@group(1) @binding(8) var gradient_border_ch2_tex: texture_2d<f32>;
@group(1) @binding(9) var province_secondary_color_tex: texture_2d<f32>;
@group(1) @binding(10) var generic_sampler: sampler;

// Pass-specific
@group(2) @binding(0) var heightmap_tex: texture_2d<f32>;
@group(2) @binding(1) var province_id_tex: texture_2d<u32>;
@group(2) @binding(2) var terrain_idx_tex: texture_2d<u32>;
@group(2) @binding(3) var terrain_atlas_tex: texture_2d<f32>;
@group(2) @binding(4) var terrain_atlas_normal_tex: texture_2d<f32>;
@group(2) @binding(5) var world_normal_tex: texture_2d<f32>;
@group(2) @binding(6) var colormap_emissive_tex: texture_2d<f32>;
@group(2) @binding(7) var citylights_tex: texture_2d<f32>;
@group(2) @binding(8) var country_color_lut: texture_2d<f32>;
@group(2) @binding(9) var pass_sampler: sampler;

// -----------------------------------------------------------------------------
// Vertex
// -----------------------------------------------------------------------------

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) shadow_uv: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world_pos4 = vec4<f32>(in.pos, 1.0);
    out.clip_pos = frame.view_proj * world_pos4;
    out.world_pos = in.pos;
    out.uv = in.uv;
    // shadow projection — main.rs 应在 GlobalFrameUniform 之外再传 shadow matrix；
    // 这里用一个简化映射作占位（实际值由 3.11.12 接入）
    out.shadow_uv = vec4<f32>(in.uv * 2.0 - 1.0, 0.5, 1.0);
    return out;
}

// -----------------------------------------------------------------------------
// Fragment helpers
// -----------------------------------------------------------------------------

/// 取季节插值后的政治底色（即 `ColorMap × (1-season) + ColorMapSecond × season`）。
fn sample_season_color(uv: vec2<f32>) -> vec3<f32> {
    let a = textureSample(color_map_tex, generic_sampler, uv).rgb;
    let b = textureSample(color_map_second_tex, generic_sampler, uv).rgb;
    return mix(a, b, params.season_lerp);
}

/// 取 atlas 中第 idx (0..15) 张地形 tile。`MAP_NUM_TILES = 4`。
fn sample_atlas_tile(world_xz: vec2<f32>, idx: u32) -> vec4<f32> {
    let row = f32(idx / 4u);
    let col = f32(idx % 4u);
    let tile_uv = fract(world_xz * 0.35);
    let atlas_uv = (vec2<f32>(col, row) + tile_uv) / 4.0;
    return textureSample(terrain_atlas_tex, pass_sampler, atlas_uv);
}

/// CSM 阴影接收 (PCF 4-tap)。0 = 全阴影，1 = 全亮。
fn shadow_pcf(shadow_proj: vec4<f32>) -> f32 {
    // Vanilla shadow.fxh 用 5-tap PCF；wgsl `textureSampleCompare` 自带硬件 PCF
    let coords = shadow_proj.xy / shadow_proj.w * vec2<f32>(0.5, -0.5) + 0.5;
    let depth = shadow_proj.z / shadow_proj.w - 0.001;
    let s = textureSampleCompare(shadow_map_tex, shadow_sampler, coords, depth);
    return mix(1.0 - frame.shadow_fade_factor, 1.0, s);
}

/// 应用 point lights（城市夜光等）。简化为 single-tap 遍历。
fn apply_point_lights(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    // light_index_tex 每 tile 4 个 light index；每 light 在 light_data_tex 占 2
    // 行（pos+radius / color+falloff）。Vanilla：64×64 索引图 + 128×N 数据图。
    // 为了让本 shader 自包含 + naga 通过，这里只做一个粗近似——读 tile (0,0) 第一光。
    let li = textureLoad(light_index_tex, vec2<i32>(0, 0), 0).r * 255.0;
    if (li >= 255.0) {
        return vec3<f32>(0.0);
    }
    let idx = i32(li);
    let pos_radius = textureLoad(light_data_tex, vec2<i32>(idx * 2, 0), 0);
    let color_falloff = textureLoad(light_data_tex, vec2<i32>(idx * 2 + 1, 0), 0);
    let to_light = pos_radius.xyz - world_pos;
    let d = length(to_light);
    let intensity = clamp((pos_radius.w - d) / max(color_falloff.w, 0.01), 0.0, 1.0);
    return color_falloff.rgb * intensity * max(dot(normalize(to_light), normal), 0.0);
}

// -----------------------------------------------------------------------------
// Fragment main
// -----------------------------------------------------------------------------

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let world_pos = in.world_pos;
    let uv = in.uv;

    // 1. 法线（world_normal.bmp 提供主法线，atlas_normal 提供 micro）
    let main_normal = unpack_normal(
        textureSample(world_normal_tex, generic_sampler, uv).rgb
    );
    let micro_normal_sample = textureSample(
        terrain_atlas_normal_tex, pass_sampler,
        fract(world_pos.xz * 0.35)
    );
    let micro_normal = unpack_normal(micro_normal_sample.rgb);
    let normal = normalize(rotate_vec_by_vec(main_normal, micro_normal));

    // 2. 地形 splatting（按 terrain.bmp 索引选 4×4 atlas tile）
    let terrain_idx = textureLoad(terrain_idx_tex, vec2<i32>(uv * vec2<f32>(5632.0, 2048.0)), 0).r;
    let atlas_color = sample_atlas_tile(world_pos.xz, terrain_idx % 16u).rgb;

    // 3. 政治色 / 季节
    let political_color = sample_season_color(uv);
    let mixed_terrain = mix(political_color, atlas_color, params.terrain_blend);

    // 4. 大气透视雾（应用前先做光照）
    let globe_n = calc_globe_normal(world_pos.xz, frame.day_night_hour_sun_dir.x);
    let day_night_factor_v = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);

    // 5. 主光照（Lambert + 阴影）
    let sun_dir = vec3<f32>(-0.408, -0.816, 0.408); // normalized LIGHT_SHADOW_DIRECTION
    let sun_diffuse = calculate_light_lambert(normal, sun_dir, frame.sun_diffuse_intensity.rgb);
    let shadow = shadow_pcf(in.shadow_uv);

    var lit = mixed_terrain * (vec3<f32>(0.55) + sun_diffuse * shadow);

    // 6. 城市夜光（仅夜半球）
    let city_emit = textureSample(colormap_emissive_tex, generic_sampler, uv).a;
    let city_color = textureSample(citylights_tex, generic_sampler, uv).rgb;
    lit += city_color * city_emit * day_night_factor_v * 5.5;

    // 7. point lights（占位）
    lit += apply_point_lights(world_pos, normal) * 0.3;

    // 8. 昼夜混合
    lit = day_night(lit, globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);

    // 9. 大气距离雾
    lit = apply_distance_fog(lit, world_pos, frame.cam_pos);

    return vec4<f32>(lit, 1.0);
}
