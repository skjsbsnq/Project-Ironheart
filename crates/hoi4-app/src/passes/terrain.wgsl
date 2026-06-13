// Phase 3.12.4 — TerrainPass shader (pdxmap integration).
//
// 替代旧 `crates/hoi4-render/src/shader.wgsl`：
//
// 1. 顶点：保留原来的 chunk 实例化 LOD 系统（每 chunk 一份 ChunkInstance，
//    grid×grid 三角形阵列，根据 heightmap 位移 Y）—— 这是项目已有的 mesh 路径，
//    pdxmap.wgsl 的 flat 顶点输入不能直接接进来，所以本文件融合这两套。
//
// 2. 片元：按 vanilla `gfx/FX/pdxmap.shader` 的特性补：
//    - **atlas_normal** + **world_normal.bmp** 双层法线贴图 → 表面 micro 细节
//    - **colormap_rgb_cityemissivemask_a** 的 .a 通道作 city emission mask
//    - **citylights_rgb_snowmask_a** 的 .rgb 作夜光发光颜色
//    - **shadow_pcf**：用 GlobalFrameUniform.shadow_view_proj 投影到 shadow map
//    - **day_night**：用 frame.day_night_hour_sun_dir 做球面法线昼夜插值
//    - **apply_distance_fog**：来自 shader_lib
//
// 3. 兼容已有特性：保留 SDF 边界（country/province/coast）、占领条纹、河流、
//    选中省份脉冲、海面 fbm 涟漪 + Phong 高光。这些在原 shader.wgsl 里就工作，
//    本节不退化。
//
// ## binding 布局（3 个 group）
//
// **`@group(0)`** — frame-global（每 LOD 一个 bind group，因为 chunk uniform 不同）
//   - `@binding(0)` GlobalFrameUniform（共享）
//   - `@binding(1)` PdxMapParams（每帧更新）
//   - `@binding(2)` ChunkUniform（per-LOD）
//
// **`@group(1)`** — shared（跨 LOD 复用同一个 bind group）
//   - shadow / season / 双 colormap / lights / gradient_border / secondary_color
//   - 加上 occupation_lut / coast_sdf / rivers（兼容 pre-3.12 的 vanilla feature）
//
// **`@group(2)`** — pass-specific（地形位图 + atlas + 法线）
//   - heightmap / province_id / terrain_idx / terrain_atlas / atlas_normal
//   - world_normal / colormap_emissive / citylights / country_color_lut
//   - pass_sampler

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct PdxMapParams {
    /// 当前选中省份 id；u32::MAX = 无选中。
    selected_province_id: u32,
    selected_state_id: u32,
    hovered_province_id: u32,
    /// terrain blend 系数（地形纹理 vs 政治色）。
    terrain_blend: f32,

    // 兼容 vanilla 现有 RenderParams
    screen_width: f32,
    screen_height: f32,
    vignette_strength: f32,
    zoom_factor: f32,

    border_country_px: f32,
    border_province_px: f32,
    /// vSeasonLerp ∈ [0, 1]
    season_lerp: f32,
    map_mode_terrain_blend: f32,

    /// world XZ 大小（由 main.rs 写）。
    world_size_xy_height_lat: vec4<f32>, // x=world_w, y=world_d, z=height_scale, w=lat_correction

    /// x = season_column, y = season_snow_offset.
    season_params: vec4<f32>,

    /// x = debug view, y = terrain-owned final water color, z = terrain SDF borders,
    /// w = terrain overlays (rivers/occupation/selection).
    terrain_controls: vec4<f32>,
    overlay_controls: vec4<f32>,
    feature_flags: vec4<f32>,

    /// terrain.bmp palette index → atlas tile index LUT.
    /// Packed as 4 × vec4<u32> to satisfy uniform alignment (stride = 16).
    atlas_idx_array: array<vec4<u32>, 64>,
    terrain_flags_array: array<vec4<u32>, 64>,
};

struct ChunkUniform {
    grid: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> params: PdxMapParams;
@group(0) @binding(2) var<uniform> chunk: ChunkUniform;

// ── Group 1 — shared cross-LOD ──
@group(1) @binding(0) var shadow_map_tex: texture_2d<f32>;
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
@group(1) @binding(11) var occupation_lut_tex: texture_2d<f32>;
@group(1) @binding(12) var coast_sdf_tex: texture_2d<f32>;
@group(1) @binding(13) var rivers_tex: texture_2d<f32>;
@group(1) @binding(14) var gradient_border_ch3_tex: texture_2d<f32>;
@group(1) @binding(15) var fow_tex: texture_2d<f32>;
@group(1) @binding(16) var mud_snow_tex: texture_2d<f32>;

// ── Group 2 — pass-specific terrain bitmaps ──
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
@group(2) @binding(10) var snow_normal_diffuse_tex: texture_2d<f32>;
@group(2) @binding(11) var mud_diffuse_gloss_tex: texture_2d<f32>;
@group(2) @binding(12) var mud_normal_spec_tex: texture_2d<f32>;
@group(2) @binding(13) var water_mask_tex: texture_2d<u32>;

const LUT_WIDTH: u32 = 256u;
const SEA_LEVEL: f32 = 95.0 / 255.0;
const NOISE_AMOUNT: f32 = 0.014;
const TERRAIN_ATLAS_TILE_SCALE: f32 = 0.95;
const CLOSE_DETAIL_AMOUNT: f32 = 0.030;
const TERRAIN_ID_JITTER_PIXELS: f32 = 0.20;
const COLORMAP_OVERLAY_STRENGTH_TERRAIN: f32 = 0.75;
const COLORMAP_MUD_OVERLAY_STRENGTH_TERRAIN: f32 = 0.5;
const TERRAIN_ATLAS_ALBEDO_STRENGTH: f32 = 0.78;
const TERRAIN_ATLAS_MAX_DARKEN_TERRAIN: f32 = 0.48;
const TERRAIN_FOREST_ALBEDO_STRENGTH: f32 = 0.82;
const TERRAIN_FOREST_POLITICAL_STRENGTH: f32 = 0.48;
const TERRAIN_ATLAS_NORMAL_STRENGTH: f32 = 0.44;
const TERRAIN_ATLAS_MIP_BIAS: f32 = -0.70;
const TERRAIN_MUD_ALBEDO_STRENGTH: f32 = 0.0;
const TERRAIN_MUD_NORMAL_STRENGTH: f32 = 0.0;
const TERRAIN_PROJECTED_SHADOW_STRENGTH: f32 = 0.0;
const TERRAIN_WATER_RELIEF_STRENGTH: f32 = 0.0;
const TERRAIN_WATER_HEIGHT_BAND_STRENGTH: f32 = 0.0;
const TERRAIN_WATER_BED_VISIBILITY_STRENGTH: f32 = 0.0;
const TERRAIN_COAST_WHITE_EDGE_STRENGTH: f32 = 0.0;
const TERRAIN_WATER_FOAM_STRENGTH: f32 = 0.0;
const CITY_LIGHTS_INTENSITY_TERRAIN: f32 = 5.5;
const CITY_LIGHTS_BLOOM_FACTOR_TERRAIN: f32 = 0.3;
const TERRAIN_CITY_LIGHTS_ENABLED: bool = false;
const TERRAIN_POINT_LIGHTS_ENABLED: bool = false;
const MUD_TILING_TERRAIN: f32 = 0.09;
const SNOW_TILING_TERRAIN: f32 = 0.05;
const SNOW_NORMAL_START_TERRAIN: f32 = 0.7;
const GB_THRESHOLD_TERRAIN: f32 = 0.05;
const GB_THRESHOLD2_TERRAIN: f32 = 0.25;
const GB_COUNTRY_FILL_NEAR_TERRAIN: f32 = 0.94;
const GB_COUNTRY_FILL_FAR_TERRAIN: f32 = 0.96;
const GB_OUTLINE_STRENGTH_TERRAIN: f32 = 0.38;
const GB_OUTLINE_DARKEN_TERRAIN: f32 = 0.70;
const GB_LIGHTING_RESTORE_TERRAIN: f32 = 0.8;
const GB_NIGHT_DESAT_BLEND_TERRAIN: f32 = 0.20;
const GB_TEXTURE_HEIGHT_TERRAIN: f32 = 1024.0;
const BORDER_FOW_REMOVAL_FACTOR_TERRAIN: f32 = 0.8;
const ID_NONE: u32 = 4294967295u;

const TERRAIN_DEBUG_OFF: u32 = 0u;
const TERRAIN_DEBUG_TERRAIN_ID: u32 = 1u;
const TERRAIN_DEBUG_ATLAS_TILE_ID: u32 = 2u;
const TERRAIN_DEBUG_BLEND_STATE: u32 = 3u;
const TERRAIN_DEBUG_CORNERS: u32 = 4u;
const TERRAIN_DEBUG_POLITICAL_BASE: u32 = 5u;
const TERRAIN_DEBUG_TERRAIN_ALBEDO: u32 = 6u;
const TERRAIN_DEBUG_NORMAL: u32 = 7u;
const TERRAIN_DEBUG_HEIGHT_SLOPE: u32 = 8u;
const TERRAIN_DEBUG_SNOW_MASK: u32 = 9u;
const TERRAIN_DEBUG_MUD_MASK: u32 = 10u;
const TERRAIN_DEBUG_RIVER_MASK: u32 = 11u;
const TERRAIN_DEBUG_CITY_EMIT_MASK: u32 = 12u;
const TERRAIN_DEBUG_CITYLIGHTS_RGB: u32 = 13u;
const TERRAIN_DEBUG_NIGHT_FACTOR: u32 = 14u;
const TERRAIN_DEBUG_CITYLIGHT_CONTRIB: u32 = 15u;
const TERRAIN_DEBUG_MAP_UV: u32 = 16u;
const TERRAIN_DEBUG_MAP_PX_GRID: u32 = 17u;
const TERRAIN_DEBUG_VANILLA_TILE_REPEAT: u32 = 18u;
const TERRAIN_DEBUG_CITYLIGHT_UV: u32 = 19u;
const TERRAIN_DEBUG_GRADIENT_BORDER_CH3: u32 = 20u;
const TERRAIN_DEBUG_PROVINCE_SECONDARY: u32 = 21u;
const TERRAIN_DEBUG_FOW_UNEXPLORED: u32 = 22u;
const TERRAIN_DEBUG_FOW_VISIBILITY: u32 = 23u;
const TERRAIN_DEBUG_FOW_ENEMY_SPOTTED: u32 = 24u;
const TERRAIN_DEBUG_MUD_SNOW_SNOW_AMOUNT: u32 = 25u;
const TERRAIN_DEBUG_MUD_SNOW_MUD_AMOUNT: u32 = 26u;
const TERRAIN_DEBUG_MUD_SNOW_TARGET: u32 = 27u;
const TERRAIN_DEBUG_POINT_LIGHT_CONTRIB: u32 = 28u;
const TERRAIN_DEBUG_COLORMAP: u32 = 29u;
const TERRAIN_DEBUG_FINAL_BEFORE_POSTPROCESS: u32 = 30u;
const TERRAIN_DEBUG_GRADIENT_BORDER_CH1_RGB: u32 = 31u;
const TERRAIN_DEBUG_GRADIENT_BORDER_CH1_ALPHA: u32 = 32u;
const TERRAIN_DEBUG_GRADIENT_BORDER_CH2_RGB: u32 = 33u;
const TERRAIN_DEBUG_GRADIENT_BORDER_CH2_ALPHA: u32 = 34u;

struct TerrainMaterialWeights {
    terrain_albedo_weight: f32,
    political_tint_weight: f32,
    season_weight: f32,
    detail_weight: f32,
    snow_weight: f32,
    map_mode_weight: f32,
};

struct GradientBorderResult {
    color: vec3<f32>,
    bloom_alpha: f32,
};

struct TerrainMaterial {
    hdr_color: vec3<f32>,
    political_base: vec3<f32>,
    colormap: vec3<f32>,
    terrain_albedo: vec3<f32>,
    normal: vec3<f32>,
    snow_mask: f32,
    mud_mask: f32,
    river_mask: f32,
    slope: f32,
    city_emit_mask: f32,
    city_lights_rgb: vec3<f32>,
    night_factor: f32,
    border_bloom_alpha: f32,
    city_light_bloom_alpha: f32,
    city_light_contribution: vec3<f32>,
    point_light_contribution: vec3<f32>,
};

// =============================================================================
// Vertex
// =============================================================================

struct VsIn {
    @builtin(vertex_index) vid: u32,
    @builtin(instance_index) iid: u32,
    @location(0) origin_xz: vec2<f32>,
    @location(1) size_xz: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) map_uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) raw_height: f32,
    @location(4) shadow_pos: vec4<f32>,
    @location(5) map_px: vec2<f32>,
};

fn latitude_correct(world_xz: vec2<f32>, world_size: vec2<f32>, factor: f32) -> vec2<f32> {
    let v = world_xz.y / max(world_size.y, 0.0001);
    let dist_from_eq = abs(v - 0.5) * 2.0;
    let squash = 1.0 - factor * dist_from_eq * dist_from_eq;
    let centre_z = world_size.y * 0.5;
    let new_z = (world_xz.y - centre_z) * squash + centre_z;
    return vec2<f32>(world_xz.x, new_z);
}

fn load_height(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let xy = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (dim - vec2<f32>(1.0)));
    return textureLoad(heightmap_tex, xy, 0).r;
}

/// 3.12.18 (2026-05-18): bilinear-sampled heightmap for fragment-side
/// `is_water` / sand-band tests. The heightmap binding is non-filterable
/// (R8Unorm with no sampler), so we hand-roll a 2×2 tap bilinear here.
/// Removes the chunky pixel staircase along coastlines that
/// `textureLoad` produced when `real_h` crossed `SEA_LEVEL`.
fn load_height_bilinear(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let coord_f = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (dim - vec2<f32>(1.0));
    let coord_i = vec2<i32>(floor(coord_f));
    let frac_xy = fract(coord_f);
    let max_x = i32(dim.x) - 1;
    let max_y = i32(dim.y) - 1;
    let x0 = clamp(coord_i.x, 0, max_x);
    let y0 = clamp(coord_i.y, 0, max_y);
    let x1 = clamp(coord_i.x + 1, 0, max_x);
    let y1 = clamp(coord_i.y + 1, 0, max_y);
    let h00 = textureLoad(heightmap_tex, vec2<i32>(x0, y0), 0).r;
    let h10 = textureLoad(heightmap_tex, vec2<i32>(x1, y0), 0).r;
    let h01 = textureLoad(heightmap_tex, vec2<i32>(x0, y1), 0).r;
    let h11 = textureLoad(heightmap_tex, vec2<i32>(x1, y1), 0).r;
    let h0 = mix(h00, h10, frac_xy.x);
    let h1 = mix(h01, h11, frac_xy.x);
    return mix(h0, h1, frac_xy.y);
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let grid = chunk.grid;
    let q_count = grid * grid;
    let q_idx = in.vid / 6u;
    let v_in_q = in.vid % 6u;

    var qx_u: u32 = q_idx % grid;
    var qz_u: u32 = q_idx / grid;
    if (q_idx >= q_count) {
        qx_u = 0u;
        qz_u = 0u;
    }

    var ox: u32 = 0u;
    var oz: u32 = 0u;
    if (v_in_q == 1u || v_in_q == 2u || v_in_q == 4u) { ox = 1u; }
    if (v_in_q == 2u || v_in_q == 4u || v_in_q == 5u) { oz = 1u; }

    let cell_size = in.size_xz / f32(grid);
    let local_xz = vec2<f32>(f32(qx_u + ox), f32(qz_u + oz)) * cell_size;
    let world_xz_raw = in.origin_xz + local_xz;

    let world_size = params.world_size_xy_height_lat.xy;
    let height_scale = params.world_size_xy_height_lat.z;
    let lat_correction = params.world_size_xy_height_lat.w;
    let uv = world_xz_to_map_uv(world_xz_raw, world_size);
    let map_px = map_uv_to_px(uv);
    let world_xz = latitude_correct(world_xz_raw, world_size, lat_correction);

    let h = load_height_bilinear(uv);
    var world_y: f32 = h * height_scale;

    // Chunk-edge skirts are disabled here. Even a tiny per-chunk drop reads as
    // a regular dark grid on land once terrain normals and shadows are applied.
    let tex_dim = vec2<f32>(textureDimensions(heightmap_tex));
    let eps = vec2<f32>(1.0) / tex_dim;
    let h_l = load_height_bilinear(uv - vec2<f32>(eps.x, 0.0));
    let h_r = load_height_bilinear(uv + vec2<f32>(eps.x, 0.0));
    let h_d = load_height_bilinear(uv - vec2<f32>(0.0, eps.y));
    let h_u = load_height_bilinear(uv + vec2<f32>(0.0, eps.y));
    let world_step_x = world_size.x * eps.x;
    let normal = normalize(vec3<f32>(
        (h_l - h_r) * height_scale,
        2.0 * world_step_x,
        (h_d - h_u) * height_scale
    ));

    let world_pos = vec3<f32>(world_xz.x, world_y, world_xz.y);

    var out: VsOut;
    let visual_pos = apply_map_horizon_bend(world_pos, frame.cam_pos, world_size);

    out.clip_position = apply_map_horizon_bend_clip(
        frame.view_proj * vec4<f32>(visual_pos, 1.0),
        world_pos,
        frame.cam_pos,
        world_size
    );
    out.map_uv = uv;
    out.world_pos = world_pos;
    out.world_normal = normal;
    out.raw_height = h;
    out.shadow_pos = frame.shadow_view_proj * vec4<f32>(world_pos, 1.0);
    out.map_px = map_px;
    return out;
}

// =============================================================================
// Fragment helpers
// =============================================================================

fn province_at(uv: vec2<f32>) -> u32 {
    let tex_size = vec2<f32>(textureDimensions(province_id_tex));
    let coord = vec2<i32>(uv * tex_size);
    let cx = clamp(coord.x, 0, i32(tex_size.x) - 1);
    let cy = clamp(coord.y, 0, i32(tex_size.y) - 1);
    return textureLoad(province_id_tex, vec2<i32>(cx, cy), 0).r;
}

fn water_mask_at(uv: vec2<f32>) -> bool {
    let tex_size = vec2<f32>(textureDimensions(water_mask_tex));
    let coord = vec2<i32>(uv * tex_size);
    let cx = clamp(coord.x, 0, i32(tex_size.x) - 1);
    let cy = clamp(coord.y, 0, i32(tex_size.y) - 1);
    return textureLoad(water_mask_tex, vec2<i32>(cx, cy), 0).r != 0u;
}

fn province_color(id: u32) -> vec4<f32> {
    let lut_coord = vec2<i32>(i32(id % LUT_WIDTH), i32(id / LUT_WIDTH));
    return textureLoad(country_color_lut, lut_coord, 0);
}

fn occupation_color_at(id: u32) -> vec4<f32> {
    let lut_coord = vec2<i32>(i32(id % LUT_WIDTH), i32(id / LUT_WIDTH));
    return textureLoad(occupation_lut_tex, lut_coord, 0);
}

fn lookup_atlas_idx(terrain_id: u32) -> u32 {
    let id = min(terrain_id, 255u);
    let row = id / 4u;
    let col = id % 4u;
    return params.atlas_idx_array[row][col];
}

fn lookup_terrain_flags(terrain_id: u32) -> u32 {
    let id = min(terrain_id, 255u);
    let row = id / 4u;
    let col = id % 4u;
    return params.terrain_flags_array[row][col];
}

fn terrain_perm_snow(terrain_id: u32) -> bool {
    return (lookup_terrain_flags(terrain_id) & 1u) != 0u;
}

fn terrain_category_water(terrain_id: u32) -> bool {
    return (lookup_terrain_flags(terrain_id) & 2u) != 0u;
}

fn terrain_category_forest(terrain_id: u32) -> bool {
    return (lookup_terrain_flags(terrain_id) & 4u) != 0u;
}

fn terrain_category_jungle(terrain_id: u32) -> bool {
    return (lookup_terrain_flags(terrain_id) & 8u) != 0u;
}

fn calculate_map_tex_index(terrain_id: u32) -> u32 {
    return lookup_atlas_idx(terrain_id);
}

fn sample_atlas_tile(id: u32, map_px: vec2<f32>) -> vec3<f32> {
    let atlas_idx = calculate_map_tex_index(id);
    let tile_x = f32(atlas_idx % 4u);
    let tile_y = f32(atlas_idx / 4u);
    var tile_uv = fract(vanilla_terrain_tile_repeat(map_px));
    let atlas_dim = vec2<f32>(textureDimensions(terrain_atlas_tex));
    let tile_dim = max(min(atlas_dim.x, atlas_dim.y) * 0.25, 1.0);
    var inset = 0.5 / tile_dim;
    if (terrain_detail_noise_enabled()) {
        tile_uv = fract(vanilla_terrain_tile_repeat(map_px) * TERRAIN_ATLAS_TILE_SCALE);
        inset = max(inset, 0.001);
    }
    let atlas_uv = (vec2<f32>(tile_x, tile_y) + clamp(tile_uv, vec2<f32>(inset), vec2<f32>(1.0 - inset))) * 0.25;
    return textureSampleBias(terrain_atlas_tex, pass_sampler, atlas_uv, TERRAIN_ATLAS_MIP_BIAS).rgb;
}

fn sample_atlas_normal_tile(id: u32, map_px: vec2<f32>) -> vec3<f32> {
    let atlas_idx = calculate_map_tex_index(id);
    let tile_x = f32(atlas_idx % 4u);
    let tile_y = f32(atlas_idx / 4u);
    var tile_uv = fract(vanilla_terrain_tile_repeat(map_px));
    let atlas_dim = vec2<f32>(textureDimensions(terrain_atlas_normal_tex));
    let tile_dim = max(min(atlas_dim.x, atlas_dim.y) * 0.25, 1.0);
    var inset = 0.5 / tile_dim;
    if (terrain_detail_noise_enabled()) {
        tile_uv = fract(vanilla_terrain_tile_repeat(map_px) * TERRAIN_ATLAS_TILE_SCALE);
        inset = max(inset, 0.001);
    }
    let atlas_uv = (vec2<f32>(tile_x, tile_y) + clamp(tile_uv, vec2<f32>(inset), vec2<f32>(1.0 - inset))) * 0.25;
    let n_raw = textureSampleBias(terrain_atlas_normal_tex, pass_sampler, atlas_uv, TERRAIN_ATLAS_MIP_BIAS).rgb;
    return normalize(n_raw * 2.0 - 1.0);
}

fn sample_terrain(id: u32, map_px: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(sample_atlas_tile(id, map_px), 1.0);
}

fn load_terrain_id(uv: vec2<f32>, offset: vec2<i32>) -> u32 {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord_f = uv * dim - vec2<f32>(0.5);
    let coord_i = vec2<i32>(floor(coord_f)) + offset;
    let cx = clamp(coord_i.x, 0, i32(dim.x) - 1);
    let cy = clamp(coord_i.y, 0, i32(dim.y) - 1);
    return textureLoad(terrain_idx_tex, vec2<i32>(cx, cy), 0).r;
}

fn terrain_atlas_color_blended(uv: vec2<f32>, map_px: vec2<f32>) -> vec3<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord_f = uv * dim - vec2<f32>(0.5);
    let frac = fract(coord_f);

    let id00 = load_terrain_id(uv, vec2<i32>(0, 0));
    let id10 = load_terrain_id(uv, vec2<i32>(1, 0));
    let id01 = load_terrain_id(uv, vec2<i32>(0, 1));
    let id11 = load_terrain_id(uv, vec2<i32>(1, 1));

    if (id00 == id10 && id00 == id01 && id00 == id11) {
        return sample_atlas_tile(id00, map_px);
    }

    let c00 = sample_atlas_tile(id00, map_px);
    let c10 = sample_atlas_tile(id10, map_px);
    let c01 = sample_atlas_tile(id01, map_px);
    let c11 = sample_atlas_tile(id11, map_px);

    return mix(mix(c00, c10, frac.x), mix(c01, c11, frac.x), frac.y);
}

fn terrain_atlas_normal_blended(uv: vec2<f32>, map_px: vec2<f32>) -> vec3<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord_f = uv * dim - vec2<f32>(0.5);
    let frac = fract(coord_f);

    let id00 = load_terrain_id(uv, vec2<i32>(0, 0));
    let id10 = load_terrain_id(uv, vec2<i32>(1, 0));
    let id01 = load_terrain_id(uv, vec2<i32>(0, 1));
    let id11 = load_terrain_id(uv, vec2<i32>(1, 1));

    if (id00 == id10 && id00 == id01 && id00 == id11) {
        return sample_atlas_normal_tile(id00, map_px);
    }

    let n00 = sample_atlas_normal_tile(id00, map_px);
    let n10 = sample_atlas_normal_tile(id10, map_px);
    let n01 = sample_atlas_normal_tile(id01, map_px);
    let n11 = sample_atlas_normal_tile(id11, map_px);

    return normalize(mix(mix(n00, n10, frac.x), mix(n01, n11, frac.x), frac.y));
}

fn close_detail_factor() -> f32 {
    return smoothstep(0.45, 0.95, params.zoom_factor);
}

fn jitter_terrain_uv(uv: vec2<f32>, map_px: vec2<f32>) -> vec2<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let close_detail = close_detail_factor();
    let jitter_px = map_px / 64.0;
    let jx = vnoise2d(jitter_px * 5.7 + vec2<f32>(13.1, 2.7)) - 0.5;
    let jy = vnoise2d(jitter_px * 5.7 + vec2<f32>(5.9, 17.3)) - 0.5;
    let jitter = vec2<f32>(jx, jy) * TERRAIN_ID_JITTER_PIXELS * close_detail / max(dim, vec2<f32>(1.0));
    return clamp(uv + jitter, vec2<f32>(0.0), vec2<f32>(1.0));
}

fn terrain_atlas_color(uv: vec2<f32>, map_px: vec2<f32>) -> vec3<f32> {
    if (terrain_detail_noise_enabled()) {
        return terrain_atlas_color_blended(jitter_terrain_uv(uv, map_px), map_px);
    }
    return terrain_atlas_color_blended(uv, map_px);
}

fn terrain_atlas_normal(uv: vec2<f32>, map_px: vec2<f32>) -> vec3<f32> {
    if (terrain_detail_noise_enabled()) {
        return terrain_atlas_normal_blended(jitter_terrain_uv(uv, map_px), map_px);
    }
    return terrain_atlas_normal_blended(uv, map_px);
}

fn colormap_color(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(color_map_tex, generic_sampler, uv).rgb;
}

/// 取季节插值后的政治底色（即 `ColorMap × (1-season) + ColorMapSecond × season`）。
fn sample_season_color(uv: vec2<f32>) -> vec3<f32> {
    let a = textureSample(color_map_tex, generic_sampler, uv).rgb;
    let b = textureSample(color_map_second_tex, generic_sampler, uv).rgb;
    return mix(a, b, params.season_lerp);
}

/// Order 81 samples the packed projected ShadowMap/FOW screen target, not the
/// ordinary depth shadow texture. The current producer is neutral but keeps the
/// binding and sampling semantics aligned with the vanilla dynamic target.
fn shadow_pcf(screen_uv: vec2<f32>) -> f32 {
    let packed = textureSample(shadow_map_tex, generic_sampler, clamp(screen_uv, vec2<f32>(0.0), vec2<f32>(1.0)));
    let projected_shadow = packed.r;
    let sampled_shadow = mix(1.0 - frame.shadow_fade_factor, 1.0, projected_shadow);
    return mix(1.0, sampled_shadow, TERRAIN_PROJECTED_SHADOW_STRENGTH);
}

fn gradient_border_page_uv(uv: vec2<f32>, page: f32) -> vec2<f32> {
    let half_pix = 0.5 / GB_TEXTURE_HEIGHT_TERRAIN;
    return vec2<f32>(
        uv.x,
        uv.y * (0.5 - half_pix) + page * 0.5
    );
}

fn country_dist_px(uv: vec2<f32>) -> f32 {
    return (1.0 - textureSample(gradient_border_ch1_tex, generic_sampler, gradient_border_page_uv(uv, 0.0)).a) * 255.0;
}
fn province_dist_px(uv: vec2<f32>) -> f32 {
    return (1.0 - textureSample(gradient_border_ch1_tex, generic_sampler, gradient_border_page_uv(uv, 1.0)).a) * 255.0;
}
fn gradient_border_ch1_sample(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(gradient_border_ch1_tex, generic_sampler, gradient_border_page_uv(uv, 0.0));
}
fn gradient_border_ch2_sample(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(gradient_border_ch2_tex, generic_sampler, gradient_border_page_uv(uv, 0.0));
}
fn gradient_border_ch3_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(gradient_border_ch3_tex, generic_sampler, uv).r * 255.0;
}
fn coast_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(coast_sdf_tex, generic_sampler, uv).r * 255.0;
}

fn province_secondary_at(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(province_secondary_color_tex, generic_sampler, uv);
}

fn fow_visibility_at(uv: vec2<f32>) -> f32 {
    return textureSample(fow_tex, generic_sampler, uv).g;
}

fn fow_sample_at(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(fow_tex, generic_sampler, uv);
}

fn mud_snow_target_at(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(mud_snow_tex, generic_sampler, uv);
}

fn river_level_at(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(rivers_tex));
    let xy = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (dim - vec2<f32>(1.0)));
    return textureLoad(rivers_tex, xy, 0).r;
}

// --- procedural hash noise (kept from shader.wgsl for water surface) -------

fn hash21(p_in: vec2<f32>) -> f32 {
    var q = fract(p_in * vec2<f32>(123.34, 456.21));
    q = q + dot(q, q + 78.233);
    return fract(q.x * q.y);
}

fn vnoise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm2d(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var amp = 0.5;
    var sum = 0.0;
    for (var i: i32 = 0; i < 4; i = i + 1) {
        sum = sum + amp * vnoise2d(p);
        p = p * 2.0;
        amp = amp * 0.5;
    }
    return sum;
}

fn triplanar_noise(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    let scale = 4.0;
    let p = world_pos * scale;
    let nx = vnoise2d(p.yz);
    let ny = vnoise2d(p.xz);
    let nz = vnoise2d(p.xy);
    var w = abs(normal);
    w = pow(w, vec3<f32>(2.0));
    let wsum = max(w.x + w.y + w.z, 0.0001);
    let n = (nx * w.x + ny * w.y + nz * w.z) / wsum;
    return n - 0.5;
}

fn water_surface(world_pos: vec3<f32>, time: f32) -> vec3<f32> {
    let p1 = world_pos.xz * 1.5 + vec2<f32>(time * 0.30, time * 0.10);
    let p2 = world_pos.xz * 3.5 + vec2<f32>(-time * 0.18, time * 0.45);
    let h_l = fbm2d(p1 - vec2<f32>(0.05, 0.0));
    let h_r = fbm2d(p1 + vec2<f32>(0.05, 0.0));
    let h_d = fbm2d(p1 - vec2<f32>(0.0, 0.05));
    let h_u = fbm2d(p1 + vec2<f32>(0.0, 0.05));
    let bump = 0.45;
    let n = normalize(vec3<f32>(
        (h_l - h_r) * bump,
        1.0,
        (h_d - h_u) * bump
    ));
    return n;
}

fn terrain_water_height_relief(map_uv: vec2<f32>, depth_ratio: f32) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let texel = 1.0 / max(dim, vec2<f32>(1.0));
    let h = load_height_bilinear(map_uv);
    let h_l = load_height_bilinear(map_uv - vec2<f32>(texel.x * 2.0, 0.0));
    let h_r = load_height_bilinear(map_uv + vec2<f32>(texel.x * 2.0, 0.0));
    let h_d = load_height_bilinear(map_uv - vec2<f32>(0.0, texel.y * 2.0));
    let h_u = load_height_bilinear(map_uv + vec2<f32>(0.0, texel.y * 2.0));
    let h_lw = load_height_bilinear(map_uv - vec2<f32>(texel.x * 8.0, 0.0));
    let h_rw = load_height_bilinear(map_uv + vec2<f32>(texel.x * 8.0, 0.0));
    let h_dw = load_height_bilinear(map_uv - vec2<f32>(0.0, texel.y * 8.0));
    let h_uw = load_height_bilinear(map_uv + vec2<f32>(0.0, texel.y * 8.0));
    let slope_near = length(vec2<f32>(h_l - h_r, h_d - h_u)) * 60.0;
    let slope_wide = length(vec2<f32>(h_lw - h_rw, h_dw - h_uw)) * 26.0;
    let slope = clamp(slope_near * 0.64 + slope_wide * 0.36, 0.0, 1.0);
    let shelf = 1.0 - smoothstep(0.54, 0.98, depth_ratio);
    let contour = (0.5 + 0.5 * sin((SEA_LEVEL - h) * 220.0)) * shelf * 0.10 * TERRAIN_WATER_HEIGHT_BAND_STRENGTH;
    return clamp((slope * (0.42 + shelf * 0.52) + contour) * TERRAIN_WATER_RELIEF_STRENGTH, 0.0, 1.0);
}

fn terrain_water_backdrop(frag: VsOut, real_h: f32, dedicated_water: bool) -> vec3<f32> {
    let depth_ratio = clamp((SEA_LEVEL - real_h) / SEA_LEVEL, 0.0, 1.0);
    let shelf = smoothstep(0.10, 0.46, depth_ratio);
    let deep = smoothstep(0.44, 0.98, depth_ratio);
    let shallow_color = vec3<f32>(0.034, 0.060, 0.052);
    let shelf_color = vec3<f32>(0.014, 0.040, 0.052);
    let deep_color = vec3<f32>(0.001, 0.008, 0.028);
    var color = mix(mix(shallow_color, shelf_color, shelf), deep_color, deep);

    let slope = terrain_water_height_relief(frag.map_uv, depth_ratio);
    let broad = fbm2d(frag.world_pos.xz * 0.70 + vec2<f32>(11.0, 37.0)) - 0.5;
    let ridge = fbm2d(frag.world_pos.xz * 2.20 + vec2<f32>(91.0, 12.0)) - 0.5;
    let grain = vnoise2d(frag.world_pos.xz * 9.0 + vec2<f32>(23.0, 5.0)) - 0.5;
    let shelf_visibility = 1.0 - smoothstep(0.52, 0.96, depth_ratio);
    let height_band =
        (0.5 + 0.5 * sin((SEA_LEVEL - real_h) * 380.0 + slope * 2.2)) *
        shelf_visibility *
        TERRAIN_WATER_HEIGHT_BAND_STRENGTH;
    let procedural = (broad * 0.40 + ridge * 0.28 + grain * 0.12 + height_band * 0.18) * (0.30 + shelf_visibility * 0.70);
    let relief = clamp(slope * (0.76 + shelf_visibility * 1.18) + procedural, -0.38, 1.22) * TERRAIN_WATER_RELIEF_STRENGTH;
    color = color * (0.74 + relief * (0.26 + shelf_visibility * 0.34));
    color = color + vec3<f32>(0.044, 0.044, 0.026) * height_band * (1.0 - deep) * 0.18;

    let atlas_bed = terrain_atlas_color(
        frag.map_uv,
        frag.map_px * 0.58 + vec2<f32>(127.0, 311.0),
    );
    let cmap_bed = colormap_color(frag.map_uv);
    let bed_luma = clamp(dot(mix(atlas_bed, cmap_bed, 0.10), vec3<f32>(0.2126, 0.7152, 0.0722)), 0.08, 0.46);
    let muted_bed = mix(vec3<f32>(bed_luma), mix(atlas_bed, cmap_bed, 0.06), 0.24);
    let bed_texture = min(muted_bed, vec3<f32>(0.46));
    let shallow_bed_tint = vec3<f32>(0.42, 0.38, 0.24);
    let shelf_bed_tint = vec3<f32>(0.18, 0.31, 0.30);
    let deep_bed_tint = vec3<f32>(0.026, 0.060, 0.112);
    let bed_tint = mix(mix(shallow_bed_tint, shelf_bed_tint, shelf), deep_bed_tint, deep);
    let terrain_bed = bed_texture * bed_tint
        * clamp(0.52 + relief * 0.72 + height_band * 0.18, 0.24, 1.28)
        * (0.44 + shelf_visibility * 0.58);
    let bed_visibility =
        clamp(shelf_visibility * (0.26 + slope * 0.30 + height_band * 0.14), 0.0, 0.56) *
        TERRAIN_WATER_BED_VISIBILITY_STRENGTH;
    color = mix(color, terrain_bed, bed_visibility);

    let coast = 1.0 - smoothstep(1.0, 18.0, coast_dist_px(frag.map_uv));
    color = mix(
        color,
        color * vec3<f32>(0.94, 1.02, 1.00),
        coast * (0.012 + shelf_visibility * 0.018) * TERRAIN_COAST_WHITE_EDGE_STRENGTH
    );

    if (dedicated_water) {
        let dedicated_deep = smoothstep(0.54, 0.98, depth_ratio);
        color = mix(color, deep_color, dedicated_deep * 0.50);
        color = color * (0.88 + shelf_visibility * 0.08 - dedicated_deep * 0.12);
    }
    return max(color, vec3<f32>(0.0));
}

fn terrain_control_enabled(value: f32) -> bool {
    return value > 0.5;
}

fn terrain_debug_view() -> u32 {
    return u32(clamp(params.terrain_controls.x + 0.5, 0.0, 34.0));
}

fn terrain_owns_water_color() -> bool {
    return terrain_control_enabled(params.terrain_controls.y);
}

fn terrain_owns_sdf_borders() -> bool {
    return terrain_control_enabled(params.terrain_controls.z);
}

fn terrain_overlays_enabled() -> bool {
    return terrain_control_enabled(params.terrain_controls.w);
}

fn terrain_detail_noise_enabled() -> bool {
    return terrain_control_enabled(params.feature_flags.x);
}

fn terrain_river_overlay_enabled() -> bool {
    return terrain_control_enabled(params.feature_flags.y);
}

fn terrain_cloud_shadow_enabled() -> bool {
    return terrain_detail_noise_enabled();
}

fn terrain_map_mode_overlay_enabled() -> bool {
    return terrain_control_enabled(params.feature_flags.z);
}

fn terrain_coast_band_enabled() -> bool {
    return terrain_control_enabled(params.feature_flags.w);
}

fn occupation_overlay_opacity() -> f32 {
    return clamp(params.overlay_controls.x, 0.0, 1.0);
}

fn selected_overlay_opacity() -> f32 {
    return clamp(params.overlay_controls.y, 0.0, 1.0);
}

fn hover_overlay_opacity() -> f32 {
    return clamp(params.overlay_controls.z, 0.0, 1.0);
}

fn map_mode_overlay_opacity() -> f32 {
    return clamp(params.overlay_controls.w, 0.0, 1.0);
}

fn terrain_material_weights() -> TerrainMaterialWeights {
    var weights: TerrainMaterialWeights;
    weights.terrain_albedo_weight = 1.0;
    weights.political_tint_weight = 1.0 - clamp(params.map_mode_terrain_blend, 0.0, 1.0);
    weights.season_weight = 0.06;
    weights.detail_weight = close_detail_factor();
    weights.snow_weight = 1.0;
    weights.map_mode_weight = clamp(params.map_mode_terrain_blend, 0.0, 1.0);
    return weights;
}

fn apply_province_secondary_color(base_color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let secondary = province_secondary_at(uv);
    let stripe = calculate_occupation_mask(uv, frame.global_time, frame.cam_pos.y);
    return mix(base_color, secondary.rgb, clamp(secondary.a * stripe * occupation_overlay_opacity(), 0.0, 1.0));
}

fn gradient_border_alpha_from_distance(dist_px: f32) -> f32 {
    let dist_norm = clamp(dist_px / 255.0, 0.0, 1.0);
    return 1.0 - smoothstep(
        GB_THRESHOLD_TERRAIN,
        GB_THRESHOLD_TERRAIN + GB_THRESHOLD2_TERRAIN,
        dist_norm
    );
}

fn apply_gradient_border_channels(base_color: vec3<f32>, uv: vec2<f32>) -> GradientBorderResult {
    let ch1 = gradient_border_ch1_sample(uv);
    let ch2 = gradient_border_ch2_sample(uv);
    let country_gate = clamp(ch2.g, 0.0, 1.0);
    let country_color = ch1.rgb;
    let fill_strength = mix(GB_COUNTRY_FILL_FAR_TERRAIN, GB_COUNTRY_FILL_NEAR_TERRAIN, params.zoom_factor);
    let fill_alpha = country_gate * fill_strength;
    let outline_alpha =
        gradient_border_alpha_from_distance(country_dist_px(uv)) *
        country_gate *
        GB_OUTLINE_STRENGTH_TERRAIN;
    let fx_alpha = clamp(ch2.b, 0.0, 1.0);

    var result: GradientBorderResult;
    let filled = mix(base_color, country_color, fill_alpha);
    let outline_color = min(filled, country_color * GB_OUTLINE_DARKEN_TERRAIN);
    result.color = mix(filled, outline_color, clamp(outline_alpha + fx_alpha, 0.0, 1.0));
    result.bloom_alpha = 1.0 - clamp(max(outline_alpha, fill_alpha * 0.55), 0.0, 1.0);
    return result;
}

fn snow_mask_at(uv: vec2<f32>, real_h: f32) -> f32 {
    let alt_thr = 0.62 + params.season_params.y;
    let snow_alt = clamp((real_h - alt_thr) * 6.0, 0.0, 1.0);
    return snow_alt;
}

fn false_color_from_id(id: u32) -> vec3<f32> {
    return fract(vec3<f32>(0.13, 0.37, 0.61) * f32(id + 1u));
}

fn terrain_corner_ids(uv: vec2<f32>) -> vec4<u32> {
    return vec4<u32>(
        load_terrain_id(uv, vec2<i32>(0, 0)),
        load_terrain_id(uv, vec2<i32>(1, 0)),
        load_terrain_id(uv, vec2<i32>(0, 1)),
        load_terrain_id(uv, vec2<i32>(1, 1))
    );
}

fn terrain_corners_all_same(ids: vec4<u32>) -> bool {
    return ids.x == ids.y && ids.x == ids.z && ids.x == ids.w;
}

fn snow_mud_fade() -> f32 {
    return clamp(frame.fow_opacity_time_snow_max_speed.z, 0.0, 1.0);
}

fn get_mud_snow_color(uv: vec2<f32>) -> vec4<f32> {
    return mud_snow_target_at(uv);
}

fn get_mud_amount(mud_snow_color: vec4<f32>) -> f32 {
    return mix(mud_snow_color.r, mud_snow_color.a, snow_mud_fade());
}

fn get_mud_color(map_px: vec2<f32>, base_color: vec3<f32>, amount: f32) -> vec3<f32> {
    let mud = textureSample(mud_diffuse_gloss_tex, pass_sampler, map_px * MUD_TILING_TERRAIN);
    let overlaid = get_overlay(base_color, mud.rgb, COLORMAP_MUD_OVERLAY_STRENGTH_TERRAIN);
    return mix(base_color, overlaid, clamp(amount * TERRAIN_MUD_ALBEDO_STRENGTH, 0.0, 1.0));
}

fn clamp_dark_terrain_detail(base_color: vec3<f32>, detail_color: vec3<f32>, max_darken: f32) -> vec3<f32> {
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    let base_luma = max(dot(base_color, luma), 0.001);
    let detail_luma = max(dot(detail_color, luma), 0.001);
    let min_luma = base_luma * (1.0 - clamp(max_darken, 0.0, 0.95));
    let darken_fix = max(1.0, min_luma / detail_luma);
    return min(detail_color * darken_fix, vec3<f32>(1.0));
}

fn terrain_forest_amount(terrain_id: u32) -> f32 {
    if (terrain_category_jungle(terrain_id)) {
        return 1.0;
    }
    if (terrain_category_forest(terrain_id)) {
        return 0.82;
    }
    return 0.0;
}

fn apply_forest_terrain_tint(base_color: vec3<f32>, terrain_id: u32, map_px: vec2<f32>, amount_scale: f32) -> vec3<f32> {
    let amount = terrain_forest_amount(terrain_id) * amount_scale;
    if (amount <= 0.0) {
        return base_color;
    }
    let forest_tint = select(
        vec3<f32>(0.105, 0.205, 0.135),
        vec3<f32>(0.075, 0.185, 0.105),
        terrain_category_jungle(terrain_id)
    );
    let canopy_large = fbm2d(map_px * 0.018 + vec2<f32>(11.3, 47.9));
    let canopy_small = vnoise2d(map_px * 0.085 + vec2<f32>(3.1, 91.7));
    let canopy = clamp(canopy_large * 0.70 + canopy_small * 0.30, 0.0, 1.0);
    let crown_shadow = smoothstep(0.36, 0.88, canopy);
    let crown_highlight = smoothstep(0.62, 0.96, 1.0 - canopy);
    var textured_tint = forest_tint * (0.78 + crown_highlight * 0.16);
    textured_tint = mix(textured_tint, vec3<f32>(0.018, 0.045, 0.030), crown_shadow * 0.34);
    let forest_overlay = mix(get_overlay(base_color, textured_tint, 0.72), textured_tint, 0.24);
    return mix(base_color, forest_overlay, clamp(amount, 0.0, 1.0));
}

fn apply_snow(map_px: vec2<f32>, base_color: vec3<f32>, amount: f32) -> vec3<f32> {
    let snow = textureSample(snow_normal_diffuse_tex, pass_sampler, map_px * SNOW_TILING_TERRAIN);
    let snow_color = mix(SNOW_COLOR_LIB, vec3<f32>(0.95, 0.97, 1.0), snow.a);
    return mix(base_color, snow_color, clamp(amount, 0.0, 1.0));
}

fn snow_normal(map_px: vec2<f32>) -> vec3<f32> {
    let n = textureSample(snow_normal_diffuse_tex, pass_sampler, map_px * SNOW_TILING_TERRAIN).rgb;
    return normalize(n * 2.0 - 1.0);
}

fn mud_normal(map_px: vec2<f32>) -> vec3<f32> {
    let n = textureSample(mud_normal_spec_tex, pass_sampler, map_px * MUD_TILING_TERRAIN).rgb;
    return normalize(n * 2.0 - 1.0);
}

fn city_light_terms(map_px: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let globe_n = calc_globe_normal(map_px, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    let emit = textureSample(colormap_emissive_tex, generic_sampler, uv).a;
    return vec4<f32>(textureSample(citylights_tex, pass_sampler, vanilla_citylight_uv(map_px)).rgb, emit * night);
}

fn build_terrain_material(frag: VsOut, real_h: f32, is_water: bool, pid: u32) -> TerrainMaterial {
    let weights = terrain_material_weights();
    let terrain_id = load_terrain_id(frag.map_uv, vec2<i32>(0, 0));
    let terrain_is_water = is_water;
    let political_color = province_color(pid).rgb;
    let atlas_terr = terrain_atlas_color(frag.map_uv, frag.map_px);
    let cmap = sample_season_color(frag.map_uv);
    let atlas_overlay_raw = get_overlay(atlas_terr, cmap, COLORMAP_OVERLAY_STRENGTH_TERRAIN);
    let atlas_overlay = clamp_dark_terrain_detail(cmap, atlas_overlay_raw, TERRAIN_ATLAS_MAX_DARKEN_TERRAIN);
    var terrain_albedo = mix(cmap, atlas_overlay, TERRAIN_ATLAS_ALBEDO_STRENGTH);
    terrain_albedo = apply_forest_terrain_tint(
        terrain_albedo,
        terrain_id,
        frag.map_px,
        TERRAIN_FOREST_ALBEDO_STRENGTH
    );
    var color = mix(political_color, terrain_albedo, weights.map_mode_weight);
    color = apply_forest_terrain_tint(
        color,
        terrain_id,
        frag.map_px,
        TERRAIN_FOREST_POLITICAL_STRENGTH * (1.0 - weights.map_mode_weight)
    );
    if (terrain_detail_noise_enabled()) {
        let atlas_terr2 = terrain_atlas_color(
            frag.map_uv,
            frag.map_px * 0.48 + vec2<f32>(365.0, 585.0),
        );
        let blended_atlas = mix(atlas_terr, atlas_terr2, 0.25);
        terrain_albedo = mix(colormap_color(frag.map_uv), blended_atlas, 0.78);
        let season_color = sample_season_color(frag.map_uv);
        let political_tint = mix(political_color, season_color, weights.season_weight);
        let tint_weight_sum = max(
            weights.terrain_albedo_weight + weights.political_tint_weight,
            0.0001,
        );
        let terrain_tint =
            (terrain_albedo * weights.terrain_albedo_weight + political_tint * weights.political_tint_weight) / tint_weight_sum;
        color = mix(political_tint, terrain_tint, weights.map_mode_weight);
    }

    var combined_normal = normalize(frag.world_normal);
    if (!terrain_is_water) {
        let raw_wn = textureSample(world_normal_tex, generic_sampler, frag.map_uv).rg * 2.0 - 1.0;
        let nx = raw_wn.x;
        let nz = raw_wn.y;
        let ny = sqrt(max(0.0, 1.0 - nx * nx - nz * nz));
        let main_normal = normalize(vec3<f32>(nx, ny, nz));
        let micro = terrain_atlas_normal(frag.map_uv, frag.map_px);
        let detailed_normal = normalize(rotate_vec_by_vec(main_normal, micro));
        combined_normal = normalize(mix(frag.world_normal, detailed_normal, TERRAIN_ATLAS_NORMAL_STRENGTH));
    }

    var snow = 0.0;
    var mud = 0.0;
    var river_mask = 0.0;
    var surface_normal = combined_normal;
    var border_bloom_alpha = 1.0;

    if (!terrain_is_water) {
        let mud_snow = get_mud_snow_color(frag.map_uv);
        snow = get_snow(mud_snow, snow_mud_fade());
        mud = get_mud_amount(mud_snow);
        color = get_mud_color(frag.map_px, color, mud);
        color = apply_snow(frag.map_px, color, snow);
        let mud_n = rotate_vec_by_vec(surface_normal, mud_normal(frag.map_px));
        surface_normal = normalize(mix(surface_normal, mud_n, mud * TERRAIN_MUD_NORMAL_STRENGTH));
        let snow_n = rotate_vec_by_vec(surface_normal, snow_normal(frag.map_px));
        surface_normal = normalize(mix(surface_normal, snow_n, snow * 0.22));

        if (terrain_coast_band_enabled()) {
            let band_top = SEA_LEVEL + 0.025;
            if (real_h < band_top) {
                let t = clamp(1.0 - (real_h - SEA_LEVEL) / 0.025, 0.0, 1.0);
                color = mix(color, vec3<f32>(0.86, 0.79, 0.55), t * 0.20 * TERRAIN_COAST_WHITE_EDGE_STRENGTH);
            }

            let cdist_coast_land = coast_dist_px(frag.map_uv);
            let coast_line = 1.0 - smoothstep(0.0, 1.65, cdist_coast_land);
            let coast_aa = smoothstep(0.18, 0.70, params.zoom_factor);
            color = mix(color, vec3<f32>(0.54, 0.50, 0.36), coast_line * coast_aa * 0.08 * TERRAIN_COAST_WHITE_EDGE_STRENGTH);
        }

        if (terrain_detail_noise_enabled()) {
            let n = triplanar_noise(frag.world_pos, combined_normal);
            color = color * (1.0 + n * NOISE_AMOUNT);
            let fine = fbm2d(frag.world_pos.xz * 18.0 + vec2<f32>(31.7, 8.4)) - 0.5;
            let grain = vnoise2d(frag.world_pos.xz * 36.0 + vec2<f32>(4.2, 19.6)) - 0.5;
            let detail = fine * 0.85 + grain * 0.15;
            color = color * (1.0 + detail * CLOSE_DETAIL_AMOUNT * weights.detail_weight * (1.0 - snow * 0.75));
        }

        let river_lvl = river_level_at(frag.map_uv);
        let zoom_cut = mix(0.55, 0.18, params.zoom_factor);
        river_mask = smoothstep(zoom_cut - 0.06, zoom_cut + 0.08, river_lvl);
        if (terrain_owns_sdf_borders()) {
            let gb = apply_gradient_border_channels(color, frag.map_uv);
            color = gb.color;
            border_bloom_alpha = gb.bloom_alpha;
        }
        if (terrain_overlays_enabled()) {
            color = apply_province_secondary_color(color, frag.map_uv);

            if (terrain_river_overlay_enabled()) {
                let river_alpha = river_mask * 0.34;
                let river_blue = mix(vec3<f32>(0.11, 0.28, 0.42), vec3<f32>(0.20, 0.46, 0.62), river_lvl);
                color = mix(color, river_blue, river_alpha);
            }

            let active_selected_opacity = selected_overlay_opacity();
            if (params.selected_province_id != ID_NONE && pid == params.selected_province_id && active_selected_opacity > 0.0) {
                let pdist = province_dist_px(frag.map_uv);
                let pulse = 0.5 + 0.5 * sin(frame.global_time * 3.5);
                let rim = clamp(1.0 - pdist / 5.0, 0.0, 1.0);
                color = color * (1.0 + 0.18 * pulse * active_selected_opacity);
                color = mix(color, vec3<f32>(1.0, 0.92, 0.45), rim * (0.4 + 0.4 * pulse) * active_selected_opacity);
            }

            let hover_alpha = hover_overlay_opacity();
            if (params.hovered_province_id != ID_NONE && pid == params.hovered_province_id && hover_alpha > 0.0) {
                let hd = province_dist_px(frag.map_uv);
                let hrim = clamp(1.0 - hd / 4.0, 0.0, 1.0);
                color = mix(color, vec3<f32>(1.0, 0.96, 0.78), hrim * hover_alpha * 0.55);
            }

            let mode_alpha = map_mode_overlay_opacity();
            if (terrain_map_mode_overlay_enabled() && mode_alpha > 0.0) {
                let value = province_color(pid).rgb;
                color = mix(color, value, mode_alpha * 0.16);
            }
        }

    } else if (terrain_owns_water_color()) {
        let depth_ratio = clamp((SEA_LEVEL - real_h) / SEA_LEVEL, 0.0, 1.0);
        let deep = smoothstep(0.18, 0.92, depth_ratio);
        color = terrain_water_backdrop(frag, real_h, false);

        surface_normal = water_surface(frag.world_pos, frame.global_time);
        let wave_lo = fbm2d(frag.world_pos.xz * 0.95 + vec2<f32>(frame.global_time * 0.18, frame.global_time * 0.06));
        let wave_hi = fbm2d(frag.world_pos.xz * 2.2 + vec2<f32>(frame.global_time * 0.30, -frame.global_time * 0.12));
        let ripple = wave_lo * 0.72 + wave_hi * 0.28;
        color = color * (0.90 + 0.080 * ripple);

        let cdist_coast = coast_dist_px(frag.map_uv);
        let foam = clamp(1.0 - cdist_coast / 1.20, 0.0, 1.0) * (1.0 - deep * 0.75);
        let foam_n = vnoise2d(frag.world_pos.xz * 7.0 + vec2<f32>(frame.global_time * 0.25, 0.0));
        let foam_alpha = foam * smoothstep(0.50, 1.05, foam_n + foam);
        color = mix(color, vec3<f32>(0.70, 0.84, 0.90), foam_alpha * 0.08 * TERRAIN_WATER_FOAM_STRENGTH);
    } else {
        // Final-quality frames let WaterPass own visible water. Terrain still
        // writes depth. Keep the hidden fallback close to the water material so
        // tiny coast coverage gaps cannot show as black seams.
        color = terrain_water_backdrop(frag, real_h, true);
        surface_normal = water_surface(frag.world_pos, frame.global_time);
    }

    let globe_n = calc_globe_normal(frag.map_px, frame.day_night_hour_sun_dir.x);
    let terrain_light_sources_enabled = (TERRAIN_CITY_LIGHTS_ENABLED || TERRAIN_POINT_LIGHTS_ENABLED) && !terrain_is_water;
    let night = select(0.0, day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0), terrain_light_sources_enabled);
    var city_emit = 0.0;
    var city_rgb = vec3<f32>(0.0);
    var city_contrib = vec3<f32>(0.0);
    if (TERRAIN_CITY_LIGHTS_ENABLED && !terrain_is_water) {
        city_emit = textureSample(colormap_emissive_tex, generic_sampler, frag.map_uv).a;
        city_rgb = textureSample(citylights_tex, pass_sampler, vanilla_citylight_uv(frag.map_px)).rgb;
        city_contrib = city_rgb * city_emit * night * CITY_LIGHTS_INTENSITY_TERRAIN;
    }
    var point_contrib = vec3<f32>(0.0);
    if (TERRAIN_POINT_LIGHTS_ENABLED && !terrain_is_water) {
        point_contrib = calculate_point_lights(
            light_data_tex,
            light_index_tex,
            frag.map_px,
            frag.world_pos,
            surface_normal,
            (0.20 + night * 0.80) * 0.55
        );
    }

    var material: TerrainMaterial;
    material.hdr_color = color;
    material.political_base = political_color;
    material.colormap = cmap;
    material.terrain_albedo = terrain_albedo;
    material.normal = surface_normal;
    material.snow_mask = snow;
    material.mud_mask = mud;
    material.river_mask = river_mask;
    material.slope = clamp(1.0 - max(surface_normal.y, 0.0), 0.0, 1.0);
    material.city_emit_mask = city_emit;
    material.city_lights_rgb = city_rgb;
    material.night_factor = night;
    material.border_bloom_alpha = border_bloom_alpha;
    material.city_light_bloom_alpha = select(0.0, clamp(city_emit * night * CITY_LIGHTS_BLOOM_FACTOR_TERRAIN, 0.0, 1.0), TERRAIN_CITY_LIGHTS_ENABLED);
    material.city_light_contribution = city_contrib;
    material.point_light_contribution = point_contrib;
    return material;
}

fn terrain_debug_color(view: u32, frag: VsOut, material: TerrainMaterial, real_h: f32, pid: u32) -> vec3<f32> {
    let terrain_id = load_terrain_id(frag.map_uv, vec2<i32>(0, 0));
    if (view == TERRAIN_DEBUG_TERRAIN_ID) {
        return false_color_from_id(terrain_id);
    }
    if (view == TERRAIN_DEBUG_ATLAS_TILE_ID) {
        return false_color_from_id(lookup_atlas_idx(terrain_id));
    }
    if (view == TERRAIN_DEBUG_BLEND_STATE) {
        let ids = terrain_corner_ids(frag.map_uv);
        let all_same = terrain_corners_all_same(ids);
        return select(vec3<f32>(0.95, 0.24, 0.36), vec3<f32>(0.18, 0.85, 0.38), all_same);
    }
    if (view == TERRAIN_DEBUG_CORNERS) {
        let ids = terrain_corner_ids(frag.map_uv);
        let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
        let coord_f = frag.map_uv * dim - vec2<f32>(0.5);
        let q = step(vec2<f32>(0.5), fract(coord_f));
        if (q.x < 0.5 && q.y < 0.5) {
            return false_color_from_id(ids.x);
        }
        if (q.x >= 0.5 && q.y < 0.5) {
            return false_color_from_id(ids.y);
        }
        if (q.x < 0.5 && q.y >= 0.5) {
            return false_color_from_id(ids.z);
        }
        return false_color_from_id(ids.w);
    }
    if (view == TERRAIN_DEBUG_POLITICAL_BASE) {
        return material.political_base;
    }
    if (view == TERRAIN_DEBUG_TERRAIN_ALBEDO) {
        return material.terrain_albedo;
    }
    if (view == TERRAIN_DEBUG_NORMAL) {
        return material.normal * 0.5 + vec3<f32>(0.5);
    }
    if (view == TERRAIN_DEBUG_HEIGHT_SLOPE) {
        return vec3<f32>(real_h, material.slope, 1.0 - material.slope);
    }
    if (view == TERRAIN_DEBUG_SNOW_MASK) {
        return vec3<f32>(material.snow_mask);
    }
    if (view == TERRAIN_DEBUG_MUD_MASK) {
        return vec3<f32>(material.mud_mask);
    }
    if (view == TERRAIN_DEBUG_RIVER_MASK) {
        return vec3<f32>(material.river_mask);
    }
    if (view == TERRAIN_DEBUG_CITY_EMIT_MASK) {
        return vec3<f32>(material.city_emit_mask);
    }
    if (view == TERRAIN_DEBUG_CITYLIGHTS_RGB) {
        return material.city_lights_rgb;
    }
    if (view == TERRAIN_DEBUG_NIGHT_FACTOR) {
        return vec3<f32>(material.night_factor);
    }
    if (view == TERRAIN_DEBUG_CITYLIGHT_CONTRIB) {
        return material.city_light_contribution;
    }
    if (view == TERRAIN_DEBUG_MAP_UV) {
        return vec3<f32>(fract(frag.map_uv.x), frag.map_uv.y, 0.0);
    }
    if (view == TERRAIN_DEBUG_MAP_PX_GRID) {
        let major = vec2<f32>(
            1.0 - smoothstep(0.0, 2.0, min(fract(frag.map_px.x / 256.0), 1.0 - fract(frag.map_px.x / 256.0)) * 256.0),
            1.0 - smoothstep(0.0, 2.0, min(fract(frag.map_px.y / 256.0), 1.0 - fract(frag.map_px.y / 256.0)) * 256.0)
        );
        let minor = vec2<f32>(
            1.0 - smoothstep(0.0, 1.0, min(fract(frag.map_px.x / 64.0), 1.0 - fract(frag.map_px.x / 64.0)) * 64.0),
            1.0 - smoothstep(0.0, 1.0, min(fract(frag.map_px.y / 64.0), 1.0 - fract(frag.map_px.y / 64.0)) * 64.0)
        );
        let line = max(max(major.x, major.y), max(minor.x, minor.y) * 0.35);
        return mix(vec3<f32>(0.03, 0.06, 0.09), vec3<f32>(0.95, 0.88, 0.32), line);
    }
    if (view == TERRAIN_DEBUG_VANILLA_TILE_REPEAT) {
        let tile = fract(vanilla_terrain_tile_repeat(frag.map_px));
        return vec3<f32>(tile.x, tile.y, 0.2);
    }
    if (view == TERRAIN_DEBUG_CITYLIGHT_UV) {
        let city_uv = fract(vanilla_citylight_uv(frag.map_px));
        return vec3<f32>(city_uv.x, city_uv.y, 0.0);
    }
    if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH3) {
        let d = gradient_border_ch3_dist_px(frag.map_uv);
        return vec3<f32>(1.0 - clamp(d / 32.0, 0.0, 1.0), clamp(d / 32.0, 0.0, 1.0), 0.2);
    }
    if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH1_RGB) {
        return gradient_border_ch1_sample(frag.map_uv).rgb;
    }
    if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH1_ALPHA) {
        return vec3<f32>(gradient_border_ch1_sample(frag.map_uv).a);
    }
    if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH2_RGB) {
        return gradient_border_ch2_sample(frag.map_uv).rgb;
    }
    if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH2_ALPHA) {
        return vec3<f32>(gradient_border_ch2_sample(frag.map_uv).a);
    }
    if (view == TERRAIN_DEBUG_PROVINCE_SECONDARY) {
        let secondary = province_secondary_at(frag.map_uv);
        return mix(vec3<f32>(0.0), secondary.rgb, max(secondary.a, 0.08));
    }
    if (view == TERRAIN_DEBUG_FOW_UNEXPLORED) {
        let fow = fow_sample_at(frag.map_uv);
        return vec3<f32>(1.0 - fow.r);
    }
    if (view == TERRAIN_DEBUG_FOW_VISIBILITY) {
        return vec3<f32>(fow_visibility_at(frag.map_uv));
    }
    if (view == TERRAIN_DEBUG_FOW_ENEMY_SPOTTED) {
        let fow = fow_sample_at(frag.map_uv);
        return vec3<f32>(fow.b);
    }
    if (view == TERRAIN_DEBUG_MUD_SNOW_SNOW_AMOUNT) {
        let ms = mud_snow_target_at(frag.map_uv);
        return vec3<f32>(get_snow(ms, snow_mud_fade()));
    }
    if (view == TERRAIN_DEBUG_MUD_SNOW_MUD_AMOUNT) {
        let ms = mud_snow_target_at(frag.map_uv);
        return vec3<f32>(get_mud_amount(ms));
    }
    if (view == TERRAIN_DEBUG_MUD_SNOW_TARGET) {
        let ms = mud_snow_target_at(frag.map_uv);
        return vec3<f32>(ms.r, ms.b, ms.g);
    }
    if (view == TERRAIN_DEBUG_POINT_LIGHT_CONTRIB) {
        return material.point_light_contribution;
    }
    if (view == TERRAIN_DEBUG_COLORMAP) {
        return material.colormap;
    }
    if (view == TERRAIN_DEBUG_FINAL_BEFORE_POSTPROCESS) {
        return material.hdr_color;
    }
    return material.hdr_color;
}

// =============================================================================
// Fragment main
// =============================================================================

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(province_id_tex));
    let coord = vec2<i32>(in.map_uv * tex_size);
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(tex_size.x) || coord.y >= i32(tex_size.y)) {
        return vec4<f32>(0.05, 0.05, 0.12, 1.0);
    }

    let pid = province_at(in.map_uv);
    let real_h = load_height_bilinear(in.map_uv);
    let terrain_id = load_terrain_id(in.map_uv, vec2<i32>(0, 0));
    let is_water = water_mask_at(in.map_uv);
    let material = build_terrain_material(in, real_h, is_water, pid);

    let debug_view = terrain_debug_view();
    if (debug_view != TERRAIN_DEBUG_OFF && debug_view != TERRAIN_DEBUG_FINAL_BEFORE_POSTPROCESS) {
        return vec4<f32>(terrain_debug_color(debug_view, in, material, real_h, pid), 1.0);
    }

    var color = material.hdr_color;

    // ── 大气 / 云影 ──
    if (terrain_cloud_shadow_enabled()) {
        let cloud_uv = map_px_to_uv(in.map_px) * 4.0 + vec2<f32>(frame.global_time * 0.012, frame.global_time * 0.005);
        let cloud_n = fbm2d(cloud_uv);
        let cloud_shadow = smoothstep(0.55, 0.75, cloud_n) * 0.020;
        color = color * (1.0 - cloud_shadow);
    }

    // ── 主光照（Lambert + 阴影 PCF）──
    let sun_dir = normalize(vec3<f32>(0.408248, 0.816497, -0.408248));
    let nrm = normalize(material.normal);
    let lambert = max(dot(nrm, sun_dir), 0.0);
    let projected_shadow_uv = vec2<f32>(
        in.clip_position.x / max(params.screen_width, 1.0),
        in.clip_position.y / max(params.screen_height, 1.0),
    );
    let shadow = shadow_pcf(projected_shadow_uv);
    let ambient = 0.72;
    let shade = ambient + (1.0 - ambient) * lambert * shadow;

    if (is_water && terrain_owns_water_color()) {
        let view_dir = normalize(frame.cam_pos - in.world_pos);
        let halfway = normalize(sun_dir + view_dir);
        let spec = pow(max(dot(nrm, halfway), 0.0), 64.0);
        color = color + vec3<f32>(1.0, 0.97, 0.85) * (spec * 0.6 * shadow);
    }

    let hidden_water_under_dedicated_pass = is_water && !terrain_owns_water_color();
    let lit_color = color * select(shade, 1.0, hidden_water_under_dedicated_pass);
    color = mix(
        lit_color,
        material.hdr_color,
        GB_LIGHTING_RESTORE_TERRAIN * (1.0 - material.border_bloom_alpha)
    );

    // ── City lights / emissive（仅夜半球）──
    if (!is_water) {
        let globe_n = calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x);
        color = color + material.city_light_contribution;
        color = color + material.point_light_contribution;

        let fow_visibility = fow_visibility_at(in.map_uv);
        let border_fow_protect = BORDER_FOW_REMOVAL_FACTOR_TERRAIN * (1.0 - material.border_bloom_alpha);
        let fow_mix = mix(fow_visibility, 1.0, border_fow_protect);
        color = mix(color * 0.56, color, fow_mix);

        color = apply_wrapped_distance_fog(
            color,
            in.world_pos,
            frame.cam_pos,
            params.world_size_xy_height_lat.x
        );
        // Keep the default Phase-B terrain restore path visually stable until
        // traced posteffect/day-night volume classification is mirrored.
        color = day_night_with_blend(
            color,
            globe_n,
            frame.day_night_hour_sun_dir.yzw,
            0.0,
            mix(GB_NIGHT_DESAT_BLEND_TERRAIN, 1.0, material.border_bloom_alpha)
        );
    }

    if (is_water) {
        color = apply_wrapped_distance_fog(
            color,
            in.world_pos,
            frame.cam_pos,
            params.world_size_xy_height_lat.x
        );
    }

    if (debug_view == TERRAIN_DEBUG_FINAL_BEFORE_POSTPROCESS) {
        return vec4<f32>(color, material.city_light_bloom_alpha);
    }

    return vec4<f32>(color, material.city_light_bloom_alpha);
}
