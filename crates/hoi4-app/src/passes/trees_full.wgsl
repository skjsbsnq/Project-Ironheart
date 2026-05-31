// TreeFullPass — vanilla tree.shader complete integration
//
// Features vs old trees_mesh.wgsl:
// 1. Season coloring via Tree_season.bmp atlas + season_lerp/season_column
// 2. Per-tree tint via Tree_tint.bmp + get_overlay()
// 3. Normal mapping per species
// 4. Shadow receiving via shadow_pcf
// 5. Day/night + distance fog (shared shader_lib)
// 6. Distant culling via TreeMaskTexture

struct TreeParams {
    season_lerp: f32,
    season_column: f32,
    fade_start: f32,
    fade_end: f32,
    world_w: f32,
    world_d: f32,
    season_column_next: f32,
    season_blend: f32,
    opacity: f32,
    scale: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> tparams: TreeParams;

@group(1) @binding(0) var shadow_map_tex: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var season_map_tex: texture_2d<f32>;
@group(1) @binding(3) var tint_map_tex: texture_2d<f32>;
@group(1) @binding(4) var tree_mask_tex: texture_2d<f32>;
@group(1) @binding(5) var map_sampler: sampler;

@group(2) @binding(0) var tree_diffuse: texture_2d<f32>;
@group(2) @binding(1) var tree_normal: texture_2d<f32>;
@group(2) @binding(2) var tree_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) inst_pos: vec3<f32>,
    @location(4) inst_scale: f32,
    @location(5) inst_type: f32,
    @location(6) inst_slope: vec2<f32>,
    @location(7) inst_tint_uv: vec2<f32>,
    @location(8) inst_season_row: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tint_uv: vec2<f32>,
    @location(4) season_row: f32,
    @location(5) tree_fade: f32,
    @location(6) map_px: vec2<f32>,
};

fn shadow_pcf(shadow_proj: vec4<f32>) -> f32 {
    if (shadow_proj.w <= 0.0) { return 1.0; }
    let coords = shadow_proj.xy / shadow_proj.w * vec2<f32>(0.5, -0.5) + 0.5;
    if (coords.x < 0.0 || coords.x > 1.0 || coords.y < 0.0 || coords.y > 1.0) { return 1.0; }
    let depth = shadow_proj.z / shadow_proj.w - 0.002;
    let s = textureSampleCompare(shadow_map_tex, shadow_sampler, coords, depth);
    return mix(1.0 - frame.shadow_fade_factor, 1.0, s);
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;

    let scaled = in.pos * in.inst_scale * tparams.scale;
    var world = scaled + in.inst_pos;
    world.y += scaled.x * in.inst_slope.x + scaled.z * in.inst_slope.y;

    out.clip_pos = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_pos = world;
    out.normal = in.normal;
    out.uv = in.uv;
    out.tint_uv = in.inst_tint_uv;
    out.season_row = in.inst_season_row;
    out.map_px = world_xz_to_map_px(world.xz, vec2<f32>(tparams.world_w, tparams.world_d));

    let d = length(world - frame.cam_pos);
    out.tree_fade = 1.0 - clamp((d - tparams.fade_start) / max(tparams.fade_end - tparams.fade_start, 0.01), 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let diffuse_sample = textureSample(tree_diffuse, tree_sampler, in.uv);
    if (diffuse_sample.a < 0.3) {
        discard;
    }

    let map_uv = map_px_to_uv(in.map_px);

    let mask = textureSample(tree_mask_tex, map_sampler, map_uv).r;
    if (mask < 0.17 && in.tree_fade < 0.05) {
        discard;
    }

    let n_sample = textureSample(tree_normal, tree_sampler, in.uv).rgb;
    let normal = normalize(in.normal + (n_sample - vec3<f32>(0.5)) * 0.5);

    // Season coloring — two-row blend for smooth transitions
    let s_uv_a = vec2<f32>(tparams.season_column / 8.0 + 1.0 / 16.0, in.season_row);
    let season_a = textureSample(season_map_tex, map_sampler, s_uv_a).rgb;
    var season_color = season_a;
    if (tparams.season_blend > 0.01) {
        let s_uv_b = vec2<f32>(tparams.season_column_next / 8.0 + 1.0 / 16.0, in.season_row);
        let season_b = textureSample(season_map_tex, map_sampler, s_uv_b).rgb;
        season_color = mix(season_a, season_b, tparams.season_blend);
    }

    // Per-tree tint from Tree_tint.bmp at per-instance UV
    let tint = textureSample(tint_map_tex, map_sampler, in.tint_uv).rgb;
    var color = diffuse_sample.rgb;
    color *= season_color;
    color = get_overlay(color, tint, 0.5);

    let shadow_proj = frame.shadow_view_proj * vec4<f32>(in.world_pos, 1.0);
    let shadow = shadow_pcf(shadow_proj);

    let sun_dir = normalize(-vec3<f32>(0.408, -0.816, 0.408));
    let n_dot_l = max(dot(normal, sun_dir), 0.2);
    var lit = color * (vec3<f32>(0.45) + frame.sun_diffuse_intensity.rgb * n_dot_l * shadow);

    let globe_n = calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x);
    lit = day_night(lit, globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);

    lit = apply_distance_fog(lit, in.world_pos, frame.cam_pos);

    let alpha = diffuse_sample.a * in.tree_fade * tparams.opacity;
    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(lit, alpha);
}
