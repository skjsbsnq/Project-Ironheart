// 3D terrain shader for HOI4 rust port.
//
// 5.2 — heightmap displacement, normal/hillshade, latitude correction.
// 5.3 — terrain.bmp index → palette, mix with political colour, snow line,
//       coastal sand, triplanar hash noise.
// 5.4 — SDF-based soft borders (country thick + province thin + LOD fade),
//       occupation stripes, click-pick selection pulse glow.
// 5.5 — sun position from game date drives hillshade, water surface gets
//       animated fbm normal perturbation + Phong specular, coastal foam from
//       coast SDF, drifting cloud shadow layer, season-shifted snow line.
// 3.6 — zoom-dependent terrain blend, province inner glow, country border
//       color stroke, coastline emphasis, reduced cloud shadow + ambient boost,
//       removed inline vignette, dual-layer atlas sampling.

struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    map_size: vec4<f32>,    // x=world_w, y=world_d, z=height_scale, w=lat_correction
};

struct ChunkUniform {
    grid: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

struct RenderParams {
    selected_province_id: u32,
    screen_width: f32,
    screen_height: f32,
    vignette_strength: f32,
    zoom_factor: f32,
    time: f32,
    border_country_px: f32,
    border_province_px: f32,
    sun_dir: vec4<f32>,
    month_phase: f32,
    season_snow_offset: f32,
    map_mode_terrain_blend: f32,
    diplomacy_mode: u32,
    object_opacity: f32,
    object_scale: f32,
    _reserved5: vec2<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var province_tex: texture_2d<u32>;
@group(0) @binding(2) var color_lut: texture_2d<f32>;
@group(0) @binding(3) var heightmap_tex: texture_2d<f32>;
@group(0) @binding(4) var<uniform> chunk: ChunkUniform;
@group(0) @binding(5) var terrain_idx_tex: texture_2d<u32>;
@group(0) @binding(6) var terrain_palette_tex: texture_2d<f32>;
@group(0) @binding(7) var country_sdf_tex: texture_2d<f32>;
@group(0) @binding(8) var province_sdf_tex: texture_2d<f32>;
@group(0) @binding(9) var occupation_lut: texture_2d<f32>;
@group(0) @binding(10) var sdf_sampler: sampler;
@group(0) @binding(11) var<uniform> params: RenderParams;
@group(0) @binding(12) var coast_sdf_tex: texture_2d<f32>;
@group(0) @binding(13) var terrain_atlas_tex: texture_2d<f32>;
@group(0) @binding(14) var terrain_atlas_sampler: sampler;
@group(0) @binding(15) var colormap_tex: texture_2d<f32>;
@group(0) @binding(16) var colormap_sampler: sampler;
// 3.6.6 — rivers texture: R8Unorm, value = level / 255 * 64..255 (level 0..4)
@group(0) @binding(17) var rivers_tex: texture_2d<f32>;
@group(0) @binding(18) var rivers_sampler: sampler;
@group(0) @binding(19) var diplomacy_border_tex: texture_2d<f32>;

const LUT_WIDTH: u32 = 256u;
const SEA_LEVEL: f32 = 95.0 / 255.0;
const NOISE_AMOUNT: f32 = 0.10;
const ID_NONE: u32 = 4294967295u;

struct VertexInput {
    @builtin(vertex_index) vid: u32,
    @builtin(instance_index) iid: u32,
    @location(0) origin_xz: vec2<f32>,
    @location(1) size_xz: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) raw_height: f32,
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

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let grid = chunk.grid;
    let q_count = grid * grid;
    let q_idx = input.vid / 6u;
    let v_in_q = input.vid % 6u;

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

    let cell_size = input.size_xz / f32(grid);
    let local_xz = vec2<f32>(f32(qx_u + ox), f32(qz_u + oz)) * cell_size;
    let world_xz_raw = input.origin_xz + local_xz;

    let world_size = camera.map_size.xy;
    let uv = clamp(world_xz_raw / max(world_size, vec2<f32>(0.0001)), vec2<f32>(0.0), vec2<f32>(1.0));
    let world_xz = latitude_correct(world_xz_raw, world_size, camera.map_size.w);

    let h = load_height(uv);
    let height_scale = camera.map_size.z;
    var world_y: f32 = h * height_scale;
    if (h < SEA_LEVEL) {
        world_y = SEA_LEVEL * height_scale;
    }

    let tex_dim = vec2<f32>(textureDimensions(heightmap_tex));
    let eps = vec2<f32>(1.0) / tex_dim;
    let h_l = load_height(uv - vec2<f32>(eps.x, 0.0));
    let h_r = load_height(uv + vec2<f32>(eps.x, 0.0));
    let h_d = load_height(uv - vec2<f32>(0.0, eps.y));
    let h_u = load_height(uv + vec2<f32>(0.0, eps.y));
    let world_step_x = world_size.x * eps.x;
    let normal = normalize(vec3<f32>(
        (h_l - h_r) * height_scale,
        2.0 * world_step_x,
        (h_d - h_u) * height_scale
    ));

    let world_pos = vec3<f32>(world_xz.x, world_y, world_xz.y);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = uv;
    out.world_pos = world_pos;
    out.world_normal = normal;
    out.raw_height = h;
    return out;
}

fn province_at(uv: vec2<f32>) -> u32 {
    let tex_size = vec2<f32>(textureDimensions(province_tex));
    let coord = vec2<i32>(uv * tex_size);
    let cx = clamp(coord.x, 0, i32(tex_size.x) - 1);
    let cy = clamp(coord.y, 0, i32(tex_size.y) - 1);
    return textureLoad(province_tex, vec2<i32>(cx, cy), 0).r;
}

fn province_color(id: u32) -> vec4<f32> {
    let lut_coord = vec2<i32>(i32(id % LUT_WIDTH), i32(id / LUT_WIDTH));
    return textureLoad(color_lut, lut_coord, 0);
}

fn occupation_color(id: u32) -> vec4<f32> {
    let lut_coord = vec2<i32>(i32(id % LUT_WIDTH), i32(id / LUT_WIDTH));
    return textureLoad(occupation_lut, lut_coord, 0);
}

fn terrain_color(uv: vec2<f32>) -> vec3<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * dim);
    let cx = clamp(coord.x, 0, i32(dim.x) - 1);
    let cy = clamp(coord.y, 0, i32(dim.y) - 1);
    let idx = textureLoad(terrain_idx_tex, vec2<i32>(cx, cy), 0).r;
    let p = textureLoad(terrain_palette_tex, vec2<i32>(i32(idx), 0), 0);
    return p.rgb;
}

// Phase 3.5: Sample terrain atlas (4×4 tile grid, 2048×2048).
// Tile selection: terrain_idx_tex.r ∈ [0, 15] picks one of 16 tiles.
// Within-tile UV: derived from world position so neighboring chunks blend
// without obvious seams (using map UV directly would stretch the texture).
fn terrain_atlas_color(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dim = vec2<f32>(textureDimensions(terrain_idx_tex));
    let coord = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * dim);
    let cx = clamp(coord.x, 0, i32(dim.x) - 1);
    let cy = clamp(coord.y, 0, i32(dim.y) - 1);
    let raw_idx = textureLoad(terrain_idx_tex, vec2<i32>(cx, cy), 0).r;
    let idx = raw_idx & 15u;  // clamp to 0-15 grid

    // Tile coords in 4×4 grid
    let tile_x = f32(idx % 4u);
    let tile_y = f32(idx / 4u);

    // Within-tile UV from world XZ — wraps every ~3 world units
    let tile_uv = fract(world_pos.xz * 0.35);

    // Atlas UV: (tile + tile_uv) / 4 → maps into 4×4 grid
    let atlas_uv = (vec2<f32>(tile_x, tile_y) + tile_uv) * 0.25;

    return textureSample(terrain_atlas_tex, terrain_atlas_sampler, atlas_uv).rgb;
}

// Phase 3.6.1: Sample colormap — continent-scale natural color base layer.
// The colormap is a low-res texture covering the entire map, providing natural
// color variation (green continents, sandy deserts, etc.) that eliminates the
// flat-color feel from large terrain areas.
fn colormap_color(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(colormap_tex, colormap_sampler, uv).rgb;
}

fn country_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(country_sdf_tex, sdf_sampler, uv).r * 255.0;
}
fn province_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(province_sdf_tex, sdf_sampler, uv).r * 255.0;
}
fn coast_dist_px(uv: vec2<f32>) -> f32 {
    return textureSample(coast_sdf_tex, sdf_sampler, uv).r * 255.0;
}
fn diplomacy_border_at(uv: vec2<f32>) -> f32 {
    return textureSample(diplomacy_border_tex, sdf_sampler, uv).r * 255.0;
}

// --- procedural hash noise --------------------------------------------------

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

// 4-octave fractal Brownian motion. Output ≈ [0, 1].
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

// Animated water surface: returns a perturbed normal and a "ripple"
// brightness scalar in [0,1].
fn water_surface(world_pos: vec3<f32>, time: f32) -> vec3<f32> {
    // Two layers of fbm flowing in different directions.
    let p1 = world_pos.xz * 1.5 + vec2<f32>(time * 0.30, time * 0.10);
    let p2 = world_pos.xz * 3.5 + vec2<f32>(-time * 0.18, time * 0.45);
    let h1 = fbm2d(p1);
    let h2 = fbm2d(p2);
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(province_tex));
    let coord = vec2<i32>(in.uv * tex_size);
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(tex_size.x) || coord.y >= i32(tex_size.y)) {
        return vec4<f32>(0.05, 0.05, 0.12, 1.0);
    }

    let pid = province_at(in.uv);
    let pol = province_color(pid).rgb;
    // Phase 3.5: Real terrain atlas texture
    let atlas_terr = terrain_atlas_color(in.uv, in.world_pos);
    let palette_terr = terrain_color(in.uv);
    // Dual-layer atlas sampling to reduce tiling repetition (3.6.5)
    let atlas_terr2 = terrain_atlas_color(in.uv, vec3<f32>(in.world_pos.x + 7.3, in.world_pos.y, in.world_pos.z + 11.7) * 0.34);
    let blended_atlas = mix(atlas_terr, atlas_terr2, 0.35);
    // Phase 3.6.1: Colormap base layer — blend 30% colormap + 70% atlas to eliminate flat-color feel
    let cmap = colormap_color(in.uv);
    let atlas_with_colormap = mix(cmap, blended_atlas, 0.7);
    let terr = mix(palette_terr, atlas_with_colormap * (palette_terr * 1.8), 0.7);

    // 3.6.1: Zoom-dependent terrain blend.
    // Base blend from map mode (political=0.30, terrain=0.75, other=0.40).
    // Zoom modulates: far → more political color, near → more terrain detail.
    let base_blend = params.map_mode_terrain_blend;
    let zoom_blend_adjust = (params.zoom_factor - 0.5) * 0.3; // ±0.15 range
    let terrain_blend = clamp(base_blend + zoom_blend_adjust, 0.10, 0.75);
    var color = mix(pol, terr, terrain_blend);

    let is_water = in.raw_height <= SEA_LEVEL;

    // 3) Snow line (with seasonal shift).
    let lat = abs(in.uv.y - 0.5) * 2.0;
    let alt_thr = 0.62 + params.season_snow_offset;
    let snow_alt = clamp((in.raw_height - alt_thr) * 6.0, 0.0, 1.0);
    let snow_lat = clamp((lat - (0.86 + params.season_snow_offset * 0.8)) * 8.0, 0.0, 1.0);
    let snow = max(snow_alt, snow_lat);
    if (!is_water) {
        color = mix(color, vec3<f32>(0.95, 0.97, 1.0), snow);
    }

    // 4) Triplanar hash noise (land only, reduced).
    if (!is_water) {
        let n = triplanar_noise(in.world_pos, in.world_normal);
        color = color * (1.0 + n * NOISE_AMOUNT);
    }

    // ─── Water surface ──────────────────────────────────────────────────
    var surface_normal = normalize(in.world_normal);
    if (is_water) {
        let depth_ratio = clamp((SEA_LEVEL - in.raw_height) / SEA_LEVEL, 0.0, 1.0);
        let shallow_color = vec3<f32>(0.45, 0.62, 0.65);
        let deep_color = vec3<f32>(0.08, 0.15, 0.28);
        let water_base = mix(shallow_color, deep_color, pow(depth_ratio, 0.6));
        color = water_base;

        let wn = water_surface(in.world_pos, params.time);
        surface_normal = wn;
        let wave_lo = fbm2d(in.world_pos.xz * 1.5 + vec2<f32>(params.time * 0.30, params.time * 0.10));
        let wave_hi = fbm2d(in.world_pos.xz * 4.0 + vec2<f32>(params.time * 0.55, -params.time * 0.20));
        let ripple = wave_lo * 0.65 + wave_hi * 0.35;
        color = color * (0.88 + 0.24 * ripple);

        // Coast foam temporarily disabled while shoreline artifacts are fixed.
    }

    // 5) Occupation stripes — land only.
    if (!is_water) {
        let occ = occupation_color(pid);
        if (occ.a > 0.0) {
            let stripe_phase = (in.world_pos.x + in.world_pos.z) * 1.6;
            let stripe = step(0.55, fract(stripe_phase));
            let band_strength = mix(0.06, 0.24, stripe);
            color = mix(color, occ.rgb, occ.a * band_strength);
        }
    }

    // 3.6.2: Province inner glow (SDF-based brightness modulation).
    // Provinces appear slightly brighter in the center, giving a subtle "pillow" effect.
    let pdist = province_dist_px(in.uv);
    if (!is_water) {
        let inner_bright = 1.0 + smoothstep(0.0, 12.0, pdist) * 0.08;
        color = color * inner_bright;
    }

    // 6) Selected-province pulse glow.
    if (params.selected_province_id != ID_NONE && pid == params.selected_province_id) {
        let pulse = 0.5 + 0.5 * sin(params.time * 3.5);
        color = color * (1.0 + 0.18 * pulse);
        let rim = clamp(1.0 - pdist / 5.0, 0.0, 1.0);
        color = mix(color, vec3<f32>(1.0, 0.92, 0.45), rim * (0.4 + 0.4 * pulse));
    }

    // 7) Soft SDF borders — 3.6.2 refined.
    let cdist = country_dist_px(in.uv);

    // 3.6.2: Coastline emphasis — dark brown line at land/water boundary.
    // Province borders: lighter gray, fade aggressively with distance.
    var p_alpha = 0.0;
    if (!is_water) {
        let p_border_width = params.border_province_px * params.zoom_factor * params.zoom_factor;
        // Fade out completely when zoomed far out (zoom_factor < 0.25).
        let p_fade = smoothstep(0.15, 0.35, params.zoom_factor);
        p_alpha = (1.0 - smoothstep(0.0, max(p_border_width, 0.1), pdist)) * p_fade;
    }
    // Province borders: medium gray instead of near-black, lower alpha.
    color = mix(color, vec3<f32>(0.35, 0.35, 0.35), p_alpha * 0.30);

    // Country borders: solid black core.
    let c_border_width = select(params.border_country_px, params.border_country_px * 0.6, is_water);
    let c_alpha = 1.0 - smoothstep(0.0, c_border_width, cdist);
    color = mix(color, vec3<f32>(0.0, 0.0, 0.0), c_alpha);

    // Diplomacy-aware country border color stroke.
    if (!is_water && cdist > c_border_width && cdist < c_border_width + 2.5) {
        let stroke_t = 1.0 - smoothstep(c_border_width, c_border_width + 2.5, cdist);
        if (params.diplomacy_mode > 0u) {
            // Diplomacy mode: colour borders by relationship.
            let diplo = diplomacy_border_at(in.uv);
            if (diplo > 0.5 && diplo < 1.5) {
                // War border — red
                color = mix(color, vec3<f32>(0.90, 0.15, 0.10), stroke_t * 0.80);
            } else if (diplo > 1.5) {
                // Same faction — green
                color = mix(color, vec3<f32>(0.15, 0.80, 0.20), stroke_t * 0.65);
            } else {
                // Neutral — muted gray stroke
                color = mix(color, vec3<f32>(0.45, 0.45, 0.50), stroke_t * 0.40);
            }
        } else {
            // Default: political colour stroke.
            color = mix(color, pol, stroke_t * 0.55);
        }
    }

    // ─── Cloud shadow (reduced intensity) ───────────────────────────────
    let cloud_uv = in.uv * 4.0 + vec2<f32>(params.time * 0.012, params.time * 0.005);
    let cloud_n = fbm2d(cloud_uv);
    let cloud_shadow = smoothstep(0.55, 0.75, cloud_n) * 0.08;
    color = color * (1.0 - cloud_shadow);

    // 8) Hillshade — boosted ambient for brighter overall look.
    let sun = normalize(params.sun_dir.xyz);
    let nrm = normalize(surface_normal);
    let lambert = max(dot(nrm, sun), 0.0);
    let ambient = 0.65;
    var shade = ambient + (1.0 - ambient) * lambert;

    // Specular highlight on water.
    if (is_water) {
        let view_dir = normalize(camera.eye.xyz - in.world_pos);
        let halfway = normalize(sun + view_dir);
        let spec = pow(max(dot(nrm, halfway), 0.0), 64.0);
        color = color + vec3<f32>(1.0, 0.97, 0.85) * (spec * 0.6);
    }

    color = color * shade;

    // Atmospheric perspective (reduced — less gray at distance).
    let view_dist = distance(camera.eye.xyz, in.world_pos);
    let max_dist = camera.map_size.x * 2.5;
    let atmo_t = clamp(view_dist / max_dist, 0.0, 1.0);
    let horizon_color = vec3<f32>(0.55, 0.62, 0.72);
    color = mix(color, horizon_color, atmo_t * atmo_t * 0.25);

    // 3.6.6 — river overlay (sample R8Unorm rivers texture; LOD-gated by zoom).
    // Texture stores level/255 = 0 / 0.25 / 0.5 / 0.75 / 1.0 for levels 0..4.
    if (!is_water) {
        let river_lvl = textureSample(rivers_tex, rivers_sampler, in.uv).r;
        // Threshold per zoom: when zoomed out, hide small rivers (level < 3).
        // zoom_factor 0=far, 1=close. Hide cutoff: small rivers need ≥0.4 zoom.
        let zoom_cut = mix(0.55, 0.20, params.zoom_factor); // far → require lvl ≥ 0.55 (≥3); near → ≥0.20 (≥1)
        if (river_lvl >= zoom_cut) {
            // Width modulation: wider rivers get stronger blue.
            let river_alpha = smoothstep(zoom_cut - 0.10, zoom_cut + 0.10, river_lvl) *
                              (0.45 + 0.45 * river_lvl);
            let river_color = vec3<f32>(0.18, 0.36, 0.62); // navy-ish blue
            color = mix(color, river_color, clamp(river_alpha, 0.0, 0.85));
        }
    }

    // 3.6.4 — screen-space vignette (subtle: strength 0.15 by default).
    // Use fragment's @builtin(position) which is the framebuffer pixel coord.
    let screen_uv = vec2<f32>(
        in.clip_position.x / max(params.screen_width, 1.0),
        in.clip_position.y / max(params.screen_height, 1.0),
    );
    let vig_d = distance(screen_uv, vec2<f32>(0.5, 0.5));
    let vig_t = smoothstep(0.4, 0.85, vig_d);
    color = color * (1.0 - params.vignette_strength * vig_t);

    return vec4<f32>(color, 1.0);
}
