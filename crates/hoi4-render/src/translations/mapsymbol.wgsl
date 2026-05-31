// =============================================================================
// mapsymbol.wgsl — Phase 3.12.15.bis.0 vanilla `gfx/FX/maparrow.shader`
//                   SymbolVertexShader + SymbolPixelShader 逐行翻译
// =============================================================================
//
// 原版兵牌（unit counter / map symbol）是 3D 世界空间 quad + 地形法线扰动 +
// Blinn-Phong 光照 + 距离雾 + 昼夜。本文件 1:1 翻译 `maparrow.shader` 的
// Symbol 部分（VS_INPUT_MAPSYMBOL / VS_OUTPUT_MAPSYMBOL / SymbolVertexShader /
// SymbolPixelShader / CalculateTerrainNormal / CalculateLighting）。
//
// ## 原版多层合成机制
//
// `SymbolPixelShader` 单次 fragment 只采样 1 张 `TexPattern`；兵牌的视觉分层
// （底框 + 兵种符号 + 战斗 overlay + 意识形态色条）由 **4 次独立 draw call
// 叠加**实现，每次换纹理和 `SymbolColor`，共享 `Position_Scale`。
//
// ## binding 约定
//
// - `@group(0) @binding(0)` = `GlobalFrameUniform`
// - `@group(0) @binding(1)` = `MapSymbolParams`（48 bytes per-instance uniform）
// - `@group(1)` = 纹理池（独立 layout，引用 TerrainPass 的 Arc<TextureView>）
// - `@group(2)` = per-draw：`TexPattern`（兵牌图案纹理）+ sampler

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

// -----------------------------------------------------------------------------
// MapSymbolParams — vanilla ConstantBuffer(4) 等价 (48 bytes std140)
// -----------------------------------------------------------------------------

struct MapSymbolParams {
    symbol_color: vec4<f32>,
    position_scale: vec4<f32>,
    time_selected_intersect_rot: vec4<f32>,
    frame_info: vec4<f32>, // x = frame_index, y = no_of_frames
};
@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> sym: MapSymbolParams;

// -----------------------------------------------------------------------------
// @group(1) — 纹理池（独立 layout，binding 顺序对齐原版 Sampler Index 0-10）
// -----------------------------------------------------------------------------

@group(1) @binding(0) var height_normal_tex: texture_2d<f32>;
@group(1) @binding(1) var terrain_id_tex: texture_2d<u32>;
@group(1) @binding(2) var heightmap_tex: texture_2d<f32>;
@group(1) @binding(3) var terrain_normal_tex: texture_2d<f32>;
@group(1) @binding(4) var lean1_tex: texture_2d<f32>;
@group(1) @binding(5) var lean2_tex: texture_2d<f32>;
@group(1) @binding(6) var shadow_map_tex: texture_depth_2d;
@group(1) @binding(7) var light_data_tex: texture_2d<f32>;
@group(1) @binding(8) var light_index_tex: texture_2d<f32>;
@group(1) @binding(9) var pool_sampler: sampler;
@group(1) @binding(10) var shadow_sampler: sampler_comparison;

// -----------------------------------------------------------------------------
// @group(2) — per-draw：TexPattern + sampler（Linear, mipmap bias -0.5）
// -----------------------------------------------------------------------------

@group(2) @binding(0) var tex_pattern: texture_2d<f32>;
@group(2) @binding(1) var pattern_sampler: sampler;

// -----------------------------------------------------------------------------
// Map dimensions & terrain constants (mirrored from defines.rs / constants.fxh)
// -----------------------------------------------------------------------------

const MAP_POW2_X: f32 = 5632.0 / 8192.0;
const MAP_POW2_Y: f32 = 2048.0 / 2048.0;
const TERRAIN_TILE_FREQ_SYM: f32 = 128.0;
const ATLAS_TEXEL_POW2_EXP_SYM: f32 = 11.0;
const MAP_NUM_TILES_SYM: f32 = 4.0;
const WATER_HEIGHT_SYM: f32 = 9.5;

// -----------------------------------------------------------------------------
// Vertex I/O — vanilla VS_INPUT_MAPSYMBOL / VS_OUTPUT_MAPSYMBOL
// -----------------------------------------------------------------------------

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec3<f32>,
    @location(2) inst_symbol_color: vec4<f32>,
    @location(3) inst_position_scale: vec4<f32>,
    @location(4) inst_time_sel_int_rot: vec4<f32>,
    @location(5) inst_frame_info: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) screen_coord: vec4<f32>,
    @location(2) uv_terrain: vec2<f32>,
    @location(3) uv_terrain_id: vec2<f32>,
    @location(4) prepos: vec3<f32>,
    @location(5) inst_symbol_color: vec4<f32>,
    @location(6) inst_time_sel_int_rot: vec4<f32>,
    @location(7) inst_frame_info: vec4<f32>,
};

// -----------------------------------------------------------------------------
// SymbolVertexShader — flat ground-laid quad in world space
//
// Vanilla `SymbolVertexShader` takes a pre-baked quad whose vertices live
// in the XZ plane (Y≈0 for thickness), scales it by `Position_Scale.w`,
// rotates it in-place around Y, then translates by `Position_Scale.xyz`.
// The result is a card lying flat on the terrain — NOT a camera-facing
// billboard. Reproduce that here:
//
//   - in.position.x  →  X offset (long edge, "width")
//   - in.position.y  →  Z offset (short edge, "depth")     ← key swap!
//   - terrain Y from heightmap stays untouched (counter sits ON the ground)
//
// `inst_frame_info.z` carries height/width aspect of the current frame so
// the depth axis matches the texture's proportions (BG: 27/66 ≈ 0.41).
// -----------------------------------------------------------------------------

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;

    // vSize = Position_Scale.w + (isIntersect * Position_Scale.w * 0.25)
    let v_size = in.inst_position_scale.w
        + in.inst_time_sel_int_rot.z * (in.inst_position_scale.w * 0.25);

    // Aspect = frame_h / frame_w. The flat quad is wider than it is deep.
    let aspect = in.inst_frame_info.z;

    // Flat quad: in.position.xy maps to local XZ.
    var v_true_position: vec3<f32> = vec3<f32>(
        in.inst_position_scale.x + in.position.x * v_size,
        in.inst_position_scale.y,
        in.inst_position_scale.z + in.position.y * v_size * aspect,
    );

    out.prepos = v_true_position;
    out.clip_pos = frame.view_proj * vec4<f32>(v_true_position, 1.0);
    // UV.v flipped because our flat quad lays in world XZ — `position.y → +Z`
    // points away from the camera (screen up), which is the texture top.
    // Vanilla bakes the right orientation into its pre-baked vertices; we
    // generate the quad in-shader so we need the flip here.
    out.uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);

    // terrain UV — use the instance's world position (center of the quad)
    // not the per-vertex position, so the UV distortion is uniform across the quad
    out.uv_terrain_id = vec2<f32>(
        (in.inst_position_scale.x + 0.5) / MAP_SIZE_X,
        (in.inst_position_scale.z + 0.5) / MAP_SIZE_Y,
    );
    out.uv_terrain.x = (in.inst_position_scale.x + 0.5) / MAP_SIZE_X;
    out.uv_terrain.y = (in.inst_position_scale.z + 0.5 - MAP_SIZE_Y) / -MAP_SIZE_Y;
    out.uv_terrain = out.uv_terrain * vec2<f32>(MAP_POW2_X, MAP_POW2_Y);

    // screen coord for shadow / point lights
    out.screen_coord.x = out.clip_pos.x * 0.5 + out.clip_pos.w * 0.5;
    out.screen_coord.y = out.clip_pos.w * 0.5 - out.clip_pos.y * 0.5;
    out.screen_coord.z = out.clip_pos.w;
    out.screen_coord.w = out.clip_pos.w;

    out.inst_symbol_color = in.inst_symbol_color;
    out.inst_time_sel_int_rot = in.inst_time_sel_int_rot;
    out.inst_frame_info = in.inst_frame_info;

    return out;
}

// -----------------------------------------------------------------------------
// Fragment helpers — vanilla maparrow.shader:243-341
// -----------------------------------------------------------------------------

/// Vanilla `calculate_water_or_land` — returns 1.0 on water, 0.0 on land.
fn calculate_water_or_land(vUV: vec2<f32>) -> f32 {
    let v_height = textureSample(heightmap_tex, pool_sampler, vUV).x * 255.0;
    let v_start = WATER_HEIGHT_SYM * 10.0;
    let v_end = v_start - 5.0;
    let v_leveled = levels1(v_height, v_end, v_start);
    return 1.0 - v_leveled;
}

/// Vanilla `calculate_map_tex_index` — 从 terrain_id 4 通道解码 atlas tile 索引。
/// Returns (IndexU, IndexV, vAllSame) packed into vec4 for each corner.
fn calculate_map_tex_index(
    terrain_id_sample: vec4<f32>,
) -> vec4<f32> {
    // terrain_id_sample 每通道存 tile index / MAP_NUM_TILES
    let base_u = floor(terrain_id_sample.r * 255.0 + 0.5);
    let base_v = floor(terrain_id_sample.g * 255.0 + 0.5);
    // 简化：只返回主 tile 的 U/V 坐标，allSame 标记放 .w
    let rd_u = floor(terrain_id_sample.b * 255.0 + 0.5);
    let rd_v = floor(terrain_id_sample.a * 255.0 + 0.5);
    return vec4<f32>(base_u, base_v, rd_u, rd_v);
}

/// Vanilla `sample_terrain` — 把 tile index 换算到 atlas 内 UV。
fn sample_terrain_uv(
    tile_u: f32,
    tile_v: f32,
    tile_repeat: vec2<f32>,
    mip_texels: f32,
    lod: f32,
) -> vec2<f32> {
    let atlas_size = exp2(ATLAS_TEXEL_POW2_EXP_SYM);
    let tile_size = mip_texels;
    let col = tile_u;
    let row = tile_v;
    return (vec2<f32>(col, row) * tile_size + fract(tile_repeat) * tile_size) / atlas_size;
}

/// Vanilla `SampleWater` — 简化版水面法线采样（4 频 LEAN 混合）。
fn sample_water_normal(uv: vec2<f32>, time: f32) -> vec3<f32> {
    let t = time * 0.02;
    let n1 = unpack_normal(textureSample(lean1_tex, pool_sampler, uv * 0.9 + vec2<f32>(t, t * 0.1)).rgb);
    let n2 = unpack_normal(textureSample(lean2_tex, pool_sampler, uv * 1.05 + vec2<f32>(-t * 0.6, t * 2.0)).rgb);
    let n3 = unpack_normal(textureSample(lean1_tex, pool_sampler, uv * 0.75 + vec2<f32>(-t * 0.2, -t * 2.0)).rgb);
    let n4 = unpack_normal(textureSample(lean2_tex, pool_sampler, uv * 0.5 + vec2<f32>(-t, -t * 0.1)).rgb);
    return normalize((n1 + n2 + n3 + n4) / 4.0);
}

/// Vanilla `CalculateTerrainNormal` — maparrow.shader:254-306
/// 采样 heightmap / terrain_id / terrain_normal / water → 混合出最终法线。
fn calculate_terrain_normal(
    vUvTerrain: vec2<f32>,
    vUvTerrainId: vec2<f32>,
    vTime: f32,
) -> vec3<f32> {
    // 1. heightmap normal
    var normal = normalize(
        textureSample(height_normal_tex, pool_sampler, vUvTerrain).rbg - 0.5
    );

    // 2. terrain tile normal (atlas + LOD)
    let vTerrainUV = vUvTerrainId + vec2<f32>(
        -0.5 / MAP_SIZE_X,
        -0.5 / MAP_SIZE_Y,
    );
    let terrain_id_coords = vec2<i32>(
        clamp(i32(vTerrainUV.x * f32(textureDimensions(terrain_id_tex).x)), 0, i32(textureDimensions(terrain_id_tex).x) - 1),
        clamp(i32(vTerrainUV.y * f32(textureDimensions(terrain_id_tex).y)), 0, i32(textureDimensions(terrain_id_tex).y) - 1),
    );
    let terrain_id_raw = textureLoad(terrain_id_tex, terrain_id_coords, 0);
    let terrain_id_sample = vec4<f32>(
        f32(terrain_id_raw.r) / 255.0,
        f32(terrain_id_raw.g) / 255.0,
        f32(terrain_id_raw.b) / 255.0,
        f32(terrain_id_raw.a) / 255.0,
    );

    let vWaterValue = calculate_water_or_land(vUvTerrain);
    let tile_repeat_raw = vUvTerrain * TERRAIN_TILE_FREQ_SYM;
    let vTileRepeat = vec2<f32>(
        tile_repeat_raw.x * (MAP_SIZE_X / MAP_SIZE_Y),
        tile_repeat_raw.y,
    );

    // LOD — 使用 textureDimensions 近似 mipmapLevel（wgsl 没有
    // textureQueryLod，用 distance-based 估算代替）
    let cam_dist = distance(vec2<f32>(frame.cam_pos.x, frame.cam_pos.z),
                            vec2<f32>(vUvTerrainId.x * MAP_SIZE_X, vUvTerrainId.y * MAP_SIZE_Y));
    let lod = clamp(floor(log2(max(cam_dist, 1.0) * 0.005) + 2.0), 0.0, 6.0);
    let vMipTexels = exp2(ATLAS_TEXEL_POW2_EXP_SYM - lod);

    // 解码 terrain_id → 4 邻 tile index
    let idx = calculate_map_tex_index(terrain_id_sample);

    // 主 tile 法线
    let main_uv = sample_terrain_uv(idx.x, idx.y, vTileRepeat, vMipTexels, lod);
    var terrain_normal = textureSample(terrain_normal_tex, pool_sampler, main_uv).rbg - 0.5;

    // 4 邻 blend（当 tile 不全相同时）
    // 简化：用主 tile + 偏移的 3 个邻居取平均（与原版 vAllSame < 1.0 分支等价）
    let idx_rd = vec2<f32>(idx.z, idx.w);
    if (idx.z != idx.x || idx.w != idx.y) {
        let uv_rd = sample_terrain_uv(idx.z, idx.w, vTileRepeat, vMipTexels, lod);
        let tn_rd = textureSample(terrain_normal_tex, pool_sampler, uv_rd).rbg - 0.5;
        terrain_normal = (terrain_normal + tn_rd) * 0.5;
    }

    // 3. water normal
    let water_normal = sample_water_normal(vUvTerrain, vTime);

    // 4. 水面区域用 flat normal 替换 topology normal
    normal = mix(normal, vec3<f32>(0.0, 1.0, 0.0), vWaterValue);

    // 5. terrain normal 与 water normal 插值
    terrain_normal = mix(terrain_normal, water_normal, vWaterValue);
    terrain_normal = normalize(terrain_normal);

    // 6. TBN blend: topology normal 是 z-axis，terrain normal 在切线空间
    let zaxis = normal;
    var xaxis = cross(zaxis, vec3<f32>(0.0, 0.0, 1.0));
    xaxis = normalize(xaxis);
    var yaxis = cross(xaxis, zaxis);
    yaxis = normalize(yaxis);
    normal = xaxis * terrain_normal.x + zaxis * terrain_normal.y + yaxis * terrain_normal.z;

    return normal;
}

/// Vanilla `CalculateLighting` — maparrow.shader:308-326
/// Blinn-Phong + shadow + point lights.
fn calculate_symbol_lighting(
    prepos: vec3<f32>,
    screenCoord: vec4<f32>,
    vNormal: vec3<f32>,
    vColor: vec4<f32>,
) -> vec3<f32> {
    let to_camera_dir = normalize(frame.cam_pos - prepos);
    let glossiness = 0.05;
    let specular_color = vec3<f32>(vColor.a);
    let non_linear_gloss = get_non_linear_glossiness(vColor.a);

    // Sun light (Blinn-Phong)
    let sun_dir = -normalize(vec3<f32>(
        LIGHT_SHADOW_DIRECTION_X,
        LIGHT_SHADOW_DIRECTION_Y,
        LIGHT_SHADOW_DIRECTION_Z,
    ));
    let bp = improved_blinn_phong(
        frame.sun_diffuse_intensity.rgb,
        sun_dir,
        to_camera_dir,
        vNormal,
        specular_color,
        non_linear_gloss,
    );

    // Shadow
    let shadow_uv = frame.shadow_view_proj * vec4<f32>(prepos, 1.0);
    let shadow_coords = shadow_uv.xy / shadow_uv.w * vec2<f32>(0.5, -0.5) + 0.5;
    let shadow_depth = shadow_uv.z / shadow_uv.w - 0.001;
    let shadow_term = textureSampleCompare(
        shadow_map_tex, shadow_sampler, shadow_coords, shadow_depth,
    );
    let fShadowTerm = mix(1.0 - frame.shadow_fade_factor, 1.0, shadow_term) * SHADOW_WEIGHT_TERRAIN_LIB;

    var diffuse_light = bp.diffuse * fShadowTerm;
    var specular_light = bp.specular * fShadowTerm;

    // Point lights — simplified single-tap (same as pdxmap.wgsl pattern)
    let li = textureLoad(light_index_tex, vec2<i32>(0, 0), 0).r * 255.0;
    if (li < 254.0) {
        let idx = i32(li);
        let pos_radius = textureLoad(light_data_tex, vec2<i32>(idx * 2, 0), 0);
        let color_falloff = textureLoad(light_data_tex, vec2<i32>(idx * 2 + 1, 0), 0);
        let to_light = pos_radius.xyz - prepos;
        let d = length(to_light);
        let intensity = clamp((pos_radius.w - d) / max(color_falloff.w, 0.01), 0.0, 1.0);
        let pt_diffuse = color_falloff.rgb * intensity * max(dot(normalize(to_light), vNormal), 0.0);
        diffuse_light += pt_diffuse;
    }

    // reflectiveColor contribution (vanilla: vec3(0) for symbol)
    let reflective_color = vec3<f32>(0.0);
    diffuse_light += reflective_color * glossiness;

    // ComposeLight: ambient + diffuse + specular
    let ambient = vec3<f32>(0.55);
    return ambient + vColor.rgb * diffuse_light + specular_light;
}

// -----------------------------------------------------------------------------
// SymbolPixelShader — vanilla maparrow.shader:455-482
// -----------------------------------------------------------------------------

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // 1. Remap UV to select the correct frame from the atlas.
    //    The texture is a horizontal atlas with `no_of_frames` frames.
    //    in.uv.x ∈ [0,1] → remap to [frame/N, (frame+1)/N].
    let nof = max(in.inst_frame_info.y, 1.0);
    let fi = clamp(in.inst_frame_info.x, 0.0, nof - 1.0);
    let frame_uv = vec2<f32>(
        (fi + in.uv.x) / nof,
        in.uv.y,
    );

    var vColor = textureSample(tex_pattern, pattern_sampler, frame_uv);
    vColor = vColor * in.inst_symbol_color * sym.symbol_color;

    // 2. Simple directional lighting (sun only).
    //
    // The quad is camera-facing, so its surface normal is roughly (0,0,1)
    // in view space; that means a strict NdotL term collapses to ~0 for a
    // mostly-vertical sun and the counter would only get the ambient.
    //
    // We MULTIPLY the texture color by the lighting term (not add), so that
    // a country-colored BG layer or a white archetype symbol comes through
    // intact. The ambient floor of 0.85 keeps the night side readable;
    // the day-night helper below dims it further when the sun is opposite.
    let sun_dir = -normalize(vec3<f32>(
        LIGHT_SHADOW_DIRECTION_X,
        LIGHT_SHADOW_DIRECTION_Y,
        LIGHT_SHADOW_DIRECTION_Z,
    ));
    // Use world-up as the surface normal — counters represent ground units,
    // so the lighting that lands on them mirrors the ground beneath.
    let NdotL = max(dot(vec3<f32>(0.0, 1.0, 0.0), sun_dir), 0.0);
    let lit = clamp(0.85 + NdotL * 0.3, 0.0, 1.15);
    var color_rgb = vColor.rgb * lit;

    // 3. Distance fog
    color_rgb = apply_distance_fog(color_rgb, in.prepos, frame.cam_pos);

    // 4. Day/night (subtle, blend=0.2 like vanilla ArrowPixelShader)
    let map_px = world_xz_to_map_px(in.prepos.xz, frame.vanilla_map_size_world_size.zw);
    let globe_n = calc_globe_normal(map_px, frame.day_night_hour_sun_dir.x);
    color_rgb = day_night_with_blend(
        color_rgb,
        globe_n,
        frame.day_night_hour_sun_dir.yzw,
        1.0,
        0.2,
    );

    return vec4<f32>(color_rgb, vColor.a);
}
