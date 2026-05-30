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

    /// terrain.bmp palette index → atlas tile index LUT.
    /// Packed as 4 × vec4<u32> to satisfy uniform alignment (stride = 16).
    atlas_idx_array: array<vec4<u32>, 4>,
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
@group(1) @binding(11) var occupation_lut_tex: texture_2d<f32>;
@group(1) @binding(12) var coast_sdf_tex: texture_2d<f32>;
@group(1) @binding(13) var rivers_tex: texture_2d<f32>;

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

const LUT_WIDTH: u32 = 256u;
const SEA_LEVEL: f32 = 95.0 / 255.0;
const NOISE_AMOUNT: f32 = 0.018;
const TERRAIN_ATLAS_TILE_SCALE: f32 = 0.95;
const CLOSE_DETAIL_AMOUNT: f32 = 0.030;
const TERRAIN_ID_JITTER_PIXELS: f32 = 0.55;
const ID_NONE: u32 = 4294967295u;

const TERRAIN_DEBUG_OFF: u32 = 0u;
const TERRAIN_DEBUG_TERRAIN_ID: u32 = 1u;
const TERRAIN_DEBUG_ATLAS_TILE_ID: u32 = 2u;
const TERRAIN_DEBUG_POLITICAL_COLOR: u32 = 3u;
const TERRAIN_DEBUG_TERRAIN_ALBEDO: u32 = 4u;
const TERRAIN_DEBUG_NORMAL: u32 = 5u;
const TERRAIN_DEBUG_HEIGHT_SLOPE: u32 = 6u;
const TERRAIN_DEBUG_SNOW_MASK: u32 = 7u;
const TERRAIN_DEBUG_RIVER_MASK: u32 = 8u;

struct TerrainMaterialWeights {
    terrain_albedo_weight: f32,
    political_tint_weight: f32,
    season_weight: f32,
    detail_weight: f32,
    snow_weight: f32,
    map_mode_weight: f32,
};

struct TerrainMaterial {
    hdr_color: vec3<f32>,
    terrain_albedo: vec3<f32>,
    normal: vec3<f32>,
    snow_mask: f32,
    river_mask: f32,
    slope: f32,
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
    @location(0) uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) raw_height: f32,
    @location(4) shadow_pos: vec4<f32>,
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
    let uv_raw = world_xz_raw / max(world_size, vec2<f32>(0.0001));
    let uv = vec2<f32>(fract(uv_raw.x), clamp(uv_raw.y, 0.0, 1.0));
    let world_xz = latitude_correct(world_xz_raw, world_size, lat_correction);

    let h = load_height_bilinear(uv);
    var world_y: f32 = h * height_scale;

    // Phase 11.2 / 3.12.18 — LOD skirt: drop chunk-edge vertices a tiny
    // amount to fill the hairline gap between adjacent-LOD chunks.
    //
    // Previous value `0.05` units was visible as **dark diagonal creases**
    // on every chunk boundary — the dropped edge formed a vertical wall
    // the size of a man, lit from a low angle, which read as a black
    // pixel-wide line through both land and sea.
    //
    // 0.005 is small enough to be sub-pixel at all gameplay zoom levels
    // yet still patches the typical 1-2 hm-pixel LOD mismatch (heightmap
    // step = 1/255 ≈ 0.004 → height_scale * 0.004 ≈ 0.016 world-y).
    let on_edge_x = qx_u == 0u || qx_u == grid - 1u;
    let on_edge_z = qz_u == 0u || qz_u == grid - 1u;
    if (on_edge_x || on_edge_z) {
        world_y -= 0.005;
    }

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
    out.clip_position = frame.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = uv;
    out.world_pos = world_pos;
    out.world_normal = normal;
    out.raw_height = h;
    out.shadow_pos = frame.shadow_view_proj * vec4<f32>(world_pos, 1.0);
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

fn province_color(id: u32) -> vec4<f32> {
    let lut_coord = vec2<i32>(i32(id % LUT_WIDTH), i32(id / LUT_WIDTH));
    return textureLoad(country_color_lut, lut_coord, 0);
}

fn occupation_color_at(id: u32) -> vec4<f32> {
    let lut_coord = vec2<i32>(i32(id % LUT_WIDTH), i32(id / LUT_WIDTH));
    return textureLoad(occupation_lut_tex, lut_coord, 0);
}

fn lookup_atlas_idx(terrain_id: u32) -> u32 {
    let row = terrain_id / 4u;
    let col = terrain_id % 4u;
    return params.atlas_idx_array[row][col];
}

fn sample_atlas_tile(id: u32, world_pos: vec3<f32>) -> vec3<f32> {
    let atlas_idx = lookup_atlas_idx(id);
    let tile_x = f32(atlas_idx % 4u);
    let tile_y = f32(atlas_idx / 4u);
    let tile_uv = fract(world_pos.xz * TERRAIN_ATLAS_TILE_SCALE);
    let inset = 0.001;
    let atlas_uv = (vec2<f32>(tile_x, tile_y) + clamp(tile_uv, vec2<f32>(inset), vec2<f32>(1.0 - inset))) * 0.25;
    return textureSample(terrain_atlas_tex, pass_sampler, atlas_uv).rgb;
}

fn sample_atlas_normal_tile(id: u32, world_pos: vec3<f32>) -> vec3<f32> {
    let atlas_idx = lookup_atlas_idx(id);
    let tile_x = f32(atlas_idx % 4u);
    let tile_y = f32(atlas_idx / 4u);
    let tile_uv = fract(world_pos.xz * TERRAIN_ATLAS_TILE_SCALE);
    let inset = 0.001;
    let atlas_uv = (vec2<f32>(tile_x, tile_y) + clamp(tile_uv, vec2<f32>(inset), vec2<f32>(1.0 - inset))) * 0.25;
    let n_raw = textureSample(terrain_atlas_normal_tex, pass_sampler, atlas_uv).rgb;
    return normalize(n_raw * 2.0 - 1.0);
}

fn load_terrain_id(uv: vec2<f32>, offset: vec2<i32>) -> u32 {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord_f = uv * dim - vec2<f32>(0.5);
    let coord_i = vec2<i32>(floor(coord_f)) + offset;
    let cx = clamp(coord_i.x, 0, i32(dim.x) - 1);
    let cy = clamp(coord_i.y, 0, i32(dim.y) - 1);
    return textureLoad(terrain_idx_tex, vec2<i32>(cx, cy), 0).r & 15u;
}

fn terrain_atlas_color_blended(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord_f = uv * dim - vec2<f32>(0.5);
    let frac = fract(coord_f);

    let id00 = load_terrain_id(uv, vec2<i32>(0, 0));
    let id10 = load_terrain_id(uv, vec2<i32>(1, 0));
    let id01 = load_terrain_id(uv, vec2<i32>(0, 1));
    let id11 = load_terrain_id(uv, vec2<i32>(1, 1));

    if (id00 == id10 && id00 == id01 && id00 == id11) {
        return sample_atlas_tile(id00, world_pos);
    }

    let c00 = sample_atlas_tile(id00, world_pos);
    let c10 = sample_atlas_tile(id10, world_pos);
    let c01 = sample_atlas_tile(id01, world_pos);
    let c11 = sample_atlas_tile(id11, world_pos);

    return mix(mix(c00, c10, frac.x), mix(c01, c11, frac.x), frac.y);
}

fn terrain_atlas_normal_blended(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord_f = uv * dim - vec2<f32>(0.5);
    let frac = fract(coord_f);

    let id00 = load_terrain_id(uv, vec2<i32>(0, 0));
    let id10 = load_terrain_id(uv, vec2<i32>(1, 0));
    let id01 = load_terrain_id(uv, vec2<i32>(0, 1));
    let id11 = load_terrain_id(uv, vec2<i32>(1, 1));

    if (id00 == id10 && id00 == id01 && id00 == id11) {
        return sample_atlas_normal_tile(id00, world_pos);
    }

    let n00 = sample_atlas_normal_tile(id00, world_pos);
    let n10 = sample_atlas_normal_tile(id10, world_pos);
    let n01 = sample_atlas_normal_tile(id01, world_pos);
    let n11 = sample_atlas_normal_tile(id11, world_pos);

    return normalize(mix(mix(n00, n10, frac.x), mix(n01, n11, frac.x), frac.y));
}

fn close_detail_factor() -> f32 {
    return smoothstep(0.45, 0.95, params.zoom_factor);
}

fn jitter_terrain_uv(uv: vec2<f32>, world_pos: vec3<f32>) -> vec2<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let close_detail = close_detail_factor();
    let jx = vnoise2d(world_pos.xz * 5.7 + vec2<f32>(13.1, 2.7)) - 0.5;
    let jy = vnoise2d(world_pos.xz * 5.7 + vec2<f32>(5.9, 17.3)) - 0.5;
    let jitter = vec2<f32>(jx, jy) * TERRAIN_ID_JITTER_PIXELS * close_detail / max(dim, vec2<f32>(1.0));
    return clamp(uv + jitter, vec2<f32>(0.0), vec2<f32>(1.0));
}

fn terrain_atlas_color(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    return terrain_atlas_color_blended(jitter_terrain_uv(uv, world_pos), world_pos);
}

fn terrain_atlas_normal(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    return terrain_atlas_normal_blended(jitter_terrain_uv(uv, world_pos), world_pos);
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

/// CSM 阴影接收 (硬件 PCF)。0 = 全阴影，1 = 全亮。
fn shadow_pcf(shadow_proj: vec4<f32>) -> f32 {
    if (shadow_proj.w <= 0.0) {
        return 1.0;
    }
    let coords = shadow_proj.xy / shadow_proj.w * vec2<f32>(0.5, -0.5) + 0.5;
    if (coords.x < 0.0 || coords.x > 1.0 || coords.y < 0.0 || coords.y > 1.0) {
        return 1.0;
    }
    let depth = shadow_proj.z / shadow_proj.w - 0.001;
    let s = textureSampleCompare(shadow_map_tex, shadow_sampler, coords, depth);
    return mix(1.0 - frame.shadow_fade_factor, 1.0, s);
}

fn country_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(gradient_border_ch1_tex, generic_sampler, uv).r * 255.0;
}
fn province_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(gradient_border_ch2_tex, generic_sampler, uv).r * 255.0;
}
fn coast_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(coast_sdf_tex, generic_sampler, uv).r * 255.0;
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

fn terrain_control_enabled(value: f32) -> bool {
    return value > 0.5;
}

fn terrain_debug_view() -> u32 {
    return u32(clamp(params.terrain_controls.x + 0.5, 0.0, 8.0));
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
    let base_blend = params.map_mode_terrain_blend;
    let zoom_blend_adjust = (params.zoom_factor - 0.5) * 0.12;

    var weights: TerrainMaterialWeights;
    weights.terrain_albedo_weight = 0.72;
    weights.political_tint_weight = 1.0 - weights.terrain_albedo_weight;
    weights.season_weight = 0.06;
    weights.detail_weight = close_detail_factor();
    weights.snow_weight = 1.0;
    weights.map_mode_weight = clamp(base_blend + zoom_blend_adjust, 0.10, 1.00);
    return weights;
}

fn snow_mask_at(uv: vec2<f32>, real_h: f32) -> f32 {
    let alt_thr = 0.62 + params.season_params.y;
    let snow_alt = clamp((real_h - alt_thr) * 6.0, 0.0, 1.0);
    return snow_alt;
}

fn false_color_from_id(id: u32) -> vec3<f32> {
    return fract(vec3<f32>(0.13, 0.37, 0.61) * f32(id + 1u));
}

fn build_terrain_material(frag: VsOut, real_h: f32, is_water: bool, pid: u32) -> TerrainMaterial {
    let weights = terrain_material_weights();
    let political_color = province_color(pid).rgb;
    let atlas_terr = terrain_atlas_color(frag.uv, frag.world_pos);
    let atlas_terr2 = terrain_atlas_color(
        frag.uv,
        vec3<f32>(frag.world_pos.x * 0.48 + 7.3, frag.world_pos.y, frag.world_pos.z * 0.48 + 11.7),
    );
    let blended_atlas = mix(atlas_terr, atlas_terr2, 0.25);
    let cmap = colormap_color(frag.uv);
    let terrain_albedo = mix(cmap, blended_atlas, 0.78);

    let season_color = sample_season_color(frag.uv);
    let political_tint = mix(political_color, season_color, weights.season_weight);
    let tint_weight_sum = max(
        weights.terrain_albedo_weight + weights.political_tint_weight,
        0.0001,
    );
    let terrain_tint =
        (terrain_albedo * weights.terrain_albedo_weight + political_tint * weights.political_tint_weight) / tint_weight_sum;
    var color = mix(political_tint, terrain_tint, weights.map_mode_weight);

    // Vanilla pdxmap-style normal stack:
    // mesh height normal + world_normal.bmp macro normal + atlas normal detail.
    var combined_normal = normalize(frag.world_normal);
    if (!is_water) {
        let raw_wn = textureSample(world_normal_tex, generic_sampler, frag.uv).rg * 2.0 - 1.0;
        let nx = raw_wn.x;
        let nz = raw_wn.y;
        let ny = sqrt(max(0.0, 1.0 - nx * nx - nz * nz));
        let main_normal = normalize(vec3<f32>(nx, ny, nz));
        let micro = terrain_atlas_normal(frag.uv, frag.world_pos);
        combined_normal = normalize(rotate_vec_by_vec(main_normal, micro));
        combined_normal = normalize(mix(frag.world_normal, combined_normal, 0.6));
    }

    var snow = 0.0;
    var river_mask = 0.0;
    var surface_normal = combined_normal;

    if (!is_water) {
        let band_top = SEA_LEVEL + 0.025;
        if (real_h < band_top) {
            let t = clamp(1.0 - (real_h - SEA_LEVEL) / 0.025, 0.0, 1.0);
            color = mix(color, vec3<f32>(0.86, 0.79, 0.55), t * 0.20);
        }

        let cdist_coast_land = coast_dist_px(frag.uv);
        let coast_line = 1.0 - smoothstep(0.0, 1.65, cdist_coast_land);
        let coast_aa = smoothstep(0.18, 0.70, params.zoom_factor);
        color = mix(color, vec3<f32>(0.54, 0.50, 0.36), coast_line * coast_aa * 0.08);

        snow = snow_mask_at(frag.uv, real_h) * weights.snow_weight;
        color = mix(color, vec3<f32>(0.95, 0.97, 1.0), snow);

        let n = triplanar_noise(frag.world_pos, combined_normal);
        color = color * (1.0 + n * NOISE_AMOUNT);
        let fine = fbm2d(frag.world_pos.xz * 18.0 + vec2<f32>(31.7, 8.4)) - 0.5;
        let grain = vnoise2d(frag.world_pos.xz * 36.0 + vec2<f32>(4.2, 19.6)) - 0.5;
        let detail = fine * 0.85 + grain * 0.15;
        color = color * (1.0 + detail * CLOSE_DETAIL_AMOUNT * weights.detail_weight * (1.0 - snow * 0.75));

        let river_lvl = river_level_at(frag.uv);
        let zoom_cut = mix(0.55, 0.18, params.zoom_factor);
        river_mask = smoothstep(zoom_cut - 0.06, zoom_cut + 0.08, river_lvl);
        if (terrain_overlays_enabled()) {
            let river_alpha = river_mask * 0.34;
            let river_blue = mix(vec3<f32>(0.11, 0.28, 0.42), vec3<f32>(0.20, 0.46, 0.62), river_lvl);
            color = mix(color, river_blue, river_alpha);

            let occ = occupation_color_at(pid);
            if (occ.a > 0.0) {
                let stripe_phase = (frag.world_pos.x + frag.world_pos.z) * 1.6;
                let stripe = step(0.55, fract(stripe_phase));
                let band_strength = mix(0.06, 0.24, stripe);
                color = mix(color, occ.rgb, occ.a * occupation_overlay_opacity() * band_strength);
            }

            let active_selected_opacity = selected_overlay_opacity();
            if (params.selected_province_id != ID_NONE && pid == params.selected_province_id && active_selected_opacity > 0.0) {
                let pdist = province_dist_px(frag.uv);
                let pulse = 0.5 + 0.5 * sin(frame.global_time * 3.5);
                let rim = clamp(1.0 - pdist / 5.0, 0.0, 1.0);
                color = color * (1.0 + 0.18 * pulse * active_selected_opacity);
                color = mix(color, vec3<f32>(1.0, 0.92, 0.45), rim * (0.4 + 0.4 * pulse) * active_selected_opacity);
            }

            let hover_alpha = hover_overlay_opacity();
            if (params.hovered_province_id != ID_NONE && pid == params.hovered_province_id && hover_alpha > 0.0) {
                let hd = province_dist_px(frag.uv);
                let hrim = clamp(1.0 - hd / 4.0, 0.0, 1.0);
                color = mix(color, vec3<f32>(1.0, 0.96, 0.78), hrim * hover_alpha * 0.55);
            }

            let mode_alpha = map_mode_overlay_opacity();
            if (mode_alpha > 0.0) {
                let value = province_color(pid).rgb;
                color = mix(color, value, mode_alpha * 0.16);
            }
        }
    } else if (terrain_owns_water_color()) {
        let depth_ratio = clamp((SEA_LEVEL - real_h) / SEA_LEVEL, 0.0, 1.0);
        let shallow_color = vec3<f32>(0.30, 0.55, 0.62);
        let deep_color = vec3<f32>(0.05, 0.18, 0.32);
        color = mix(shallow_color, deep_color, pow(depth_ratio, 0.6));

        surface_normal = water_surface(frag.world_pos, frame.global_time);
        let wave_lo = fbm2d(frag.world_pos.xz * 1.5 + vec2<f32>(frame.global_time * 0.30, frame.global_time * 0.10));
        let wave_hi = fbm2d(frag.world_pos.xz * 4.0 + vec2<f32>(frame.global_time * 0.55, -frame.global_time * 0.20));
        let ripple = wave_lo * 0.65 + wave_hi * 0.35;
        color = color * (0.88 + 0.10 * ripple);

        let cdist_coast = coast_dist_px(frag.uv);
        let foam = clamp(1.0 - cdist_coast / 0.75, 0.0, 1.0);
        let foam_n = vnoise2d(frag.world_pos.xz * 12.0 + vec2<f32>(frame.global_time * 0.4, 0.0));
        let foam_alpha = foam * smoothstep(0.4, 1.0, foam_n + foam);
        color = mix(color, vec3<f32>(0.74, 0.86, 0.92), foam_alpha * 0.12);
    } else {
        // Final-quality frames let WaterPass own visible water. Terrain still
        // writes depth. Keep the hidden fallback close to the water material so
        // tiny coast coverage gaps cannot show as black seams.
        let depth_ratio = clamp((SEA_LEVEL - real_h) / SEA_LEVEL, 0.0, 1.0);
        let shallow_color = vec3<f32>(0.14, 0.34, 0.50);
        let deep_color = vec3<f32>(0.035, 0.11, 0.24);
        color = mix(shallow_color, deep_color, pow(depth_ratio, 0.55));
        color = mix(color, cmap, 0.10);
    }

    var material: TerrainMaterial;
    material.hdr_color = color;
    material.terrain_albedo = terrain_albedo;
    material.normal = surface_normal;
    material.snow_mask = snow;
    material.river_mask = river_mask;
    material.slope = clamp(1.0 - max(surface_normal.y, 0.0), 0.0, 1.0);
    return material;
}

fn terrain_debug_color(view: u32, frag: VsOut, material: TerrainMaterial, real_h: f32, pid: u32) -> vec3<f32> {
    let terrain_id = load_terrain_id(frag.uv, vec2<i32>(0, 0));
    if (view == TERRAIN_DEBUG_TERRAIN_ID) {
        return false_color_from_id(terrain_id);
    }
    if (view == TERRAIN_DEBUG_ATLAS_TILE_ID) {
        return false_color_from_id(lookup_atlas_idx(terrain_id));
    }
    if (view == TERRAIN_DEBUG_POLITICAL_COLOR) {
        return province_color(pid).rgb;
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
    if (view == TERRAIN_DEBUG_RIVER_MASK) {
        return vec3<f32>(material.river_mask);
    }
    return material.hdr_color;
}

// =============================================================================
// Fragment main
// =============================================================================

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(province_id_tex));
    let coord = vec2<i32>(in.uv * tex_size);
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(tex_size.x) || coord.y >= i32(tex_size.y)) {
        return vec4<f32>(0.05, 0.05, 0.12, 1.0);
    }

    let pid = province_at(in.uv);
    let real_h = load_height_bilinear(in.uv);
    let is_water = real_h <= SEA_LEVEL;
    let material = build_terrain_material(in, real_h, is_water, pid);

    let debug_view = terrain_debug_view();
    if (debug_view != TERRAIN_DEBUG_OFF) {
        return vec4<f32>(terrain_debug_color(debug_view, in, material, real_h, pid), 1.0);
    }

    var color = material.hdr_color;

    // Terrain SDF borders are only a fallback owner. In final-quality frames
    // BorderPass owns ordinary province/country boundary color.
    if (terrain_owns_sdf_borders() && !is_water) {
        let pdist = province_dist_px(in.uv);
        let inner_bright = 1.0 + smoothstep(0.0, 12.0, pdist) * 0.08;
        color = color * inner_bright;

        let aa = clamp(fwidth(pdist) * 0.85, 0.08, 0.45);
        let p_fade = smoothstep(0.18, 0.52, params.zoom_factor);
        let outer_width = params.border_province_px * mix(0.85, 1.15, params.zoom_factor);
        let core_width = max(outer_width * 0.42, 0.26);
        let outer = (1.0 - smoothstep(outer_width - aa, outer_width + aa, pdist)) * p_fade;
        let core = (1.0 - smoothstep(core_width - aa, core_width + aa, pdist)) * p_fade;
        color = mix(color, vec3<f32>(0.28, 0.27, 0.23), outer * 0.18);
        color = mix(color, vec3<f32>(0.07, 0.075, 0.07), core * 0.34);
    }

    // ── 大气 / 云影 ──
    let cloud_uv = in.uv * 4.0 + vec2<f32>(frame.global_time * 0.012, frame.global_time * 0.005);
    let cloud_n = fbm2d(cloud_uv);
    let cloud_shadow = smoothstep(0.55, 0.75, cloud_n) * 0.035;
    color = color * (1.0 - cloud_shadow);

    // ── 主光照（Lambert + 阴影 PCF）──
    let sun_dir = normalize(frame.day_night_hour_sun_dir.yzw);
    let nrm = normalize(material.normal);
    let lambert = max(dot(nrm, sun_dir), 0.0);
    let shadow = shadow_pcf(in.shadow_pos);
    let ambient = 0.55;
    let shade = ambient + (1.0 - ambient) * lambert * shadow;

    if (is_water && terrain_owns_water_color()) {
        let view_dir = normalize(frame.cam_pos - in.world_pos);
        let halfway = normalize(sun_dir + view_dir);
        let spec = pow(max(dot(nrm, halfway), 0.0), 64.0);
        color = color + vec3<f32>(1.0, 0.97, 0.85) * (spec * 0.6 * shadow);
    }

    color = color * shade;

    // ── City lights / emissive（仅夜半球）──
    if (!is_water) {
        let globe_n = calc_globe_normal(in.world_pos.xz, frame.day_night_hour_sun_dir.x);
        let dn_factor = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
        let city_emit = textureSample(colormap_emissive_tex, generic_sampler, in.uv).a;
        let city_color = textureSample(citylights_tex, generic_sampler, in.uv).rgb;
        color = color + city_color * city_emit * dn_factor * 4.5;
        color = day_night(color, globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    }

    color = apply_distance_fog(color, in.world_pos, frame.cam_pos);

    let screen_uv = vec2<f32>(
        in.clip_position.x / max(params.screen_width, 1.0),
        in.clip_position.y / max(params.screen_height, 1.0),
    );
    let vig_d = distance(screen_uv, vec2<f32>(0.5, 0.5));
    let vig_t = smoothstep(0.4, 0.85, vig_d);
    color = color * (1.0 - params.vignette_strength * vig_t);

    return vec4<f32>(color, 1.0);
}
