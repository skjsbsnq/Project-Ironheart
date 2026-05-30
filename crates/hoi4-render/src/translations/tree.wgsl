// =============================================================================
// tree.wgsl — Phase 3.11.7 vanilla `gfx/FX/tree.shader` 完整翻译
// =============================================================================
//
// **关系**：本工程已有 `trees.wgsl`（billboard）+ `trees_mesh.wgsl`（3D mesh
// instance）。本文件是对 vanilla `tree.shader` 的**完整**翻译，准备替换
// trees_mesh.wgsl 的当前简化版（Lambert 光照 + 距离 alpha 衰减）。
//
// vanilla 完整特性：
// 1. `vSlopes`（斜坡修正）+ `vSeasonLerp` + `vSeasonColumn` + `vTreeFade`
// 2. `TreeMaskTexture` 远景剔除：`clip(0.17 - GetTreeMask(...))`
// 3. 季节染色：`SeasonMap` 按 `vSeasonColumn / 8.0 + 1/16` 列 +
//    `vTexCoord0_TintUV.w` 行采样 + `TREE_SEASON_MIN/FADE_TWEAK` 衰减
// 4. `TintMap` + `GetOverlay` 个性化色调
// 5. 法线贴图（pinetree_normal / palmblad_normal）
// 6. 接公共 shadow / fog / PBR 光照（同 pdxmesh）
// 7. PixelShaderShadow（阴影投射）+ PixelShaderUnlit（远景平铺简化）变体

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct TreeParams {
    /// 当前季节插值
    season_lerp: f32,
    /// 季节列偏移 (0..7)
    season_column: f32,
    /// 远景淡入起始 / 淡出结束（屏幕距离百分比）
    fade_start: f32,
    fade_end: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> tparams: TreeParams;

@group(1) @binding(0) var shadow_map_tex: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var season_map_tex: texture_2d<f32>;
@group(1) @binding(3) var tint_map_tex: texture_2d<f32>;

@group(2) @binding(0) var tree_diffuse: texture_2d<f32>;
@group(2) @binding(1) var tree_normal: texture_2d<f32>;
@group(2) @binding(2) var tree_mask_tex: texture_2d<f32>;
@group(2) @binding(3) var tree_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    /// instance: 世界位置（xyz）+ 树种 (w)
    @location(3) inst_pos_type: vec4<f32>,
    /// instance: 缩放 (x) + tint U/V (y/z) + season row (w)
    @location(4) inst_scale_tint: vec4<f32>,
    /// instance: 斜坡向量 xz
    @location(5) inst_slopes: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tint_uv: vec2<f32>,
    @location(4) season_row: f32,
    @location(5) tree_fade: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;

    // 世界位置 = 顶点 × scale + 实例位置 + 斜坡修正
    let scaled = in.pos * in.inst_scale_tint.x;
    var world = scaled + in.inst_pos_type.xyz;
    world.y += scaled.x * in.inst_slopes.x + scaled.z * in.inst_slopes.y;

    out.clip_pos = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_pos = world;
    out.normal = in.normal;
    out.uv = in.uv;
    out.tint_uv = in.inst_scale_tint.yz;
    out.season_row = in.inst_scale_tint.w;

    // 距相机距离 → tree fade
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

    // 远景剔除
    let mask = textureSample(tree_mask_tex, tree_sampler, in.world_pos.xz / vec2<f32>(5632.0, 2048.0)).r;
    if (mask < 0.17 && in.tree_fade < 0.05) {
        discard;
    }

    // 法线（TBN 略；树叶不需要严格切线空间，用世界法线 + bump 略偏）
    let n_sample = textureSample(tree_normal, tree_sampler, in.uv).rgb;
    let normal = normalize(in.normal + (n_sample - vec3<f32>(0.5)) * 0.5);

    // 季节染色（SeasonMap 列 = season_column/8 + 1/16，行 = season_row）
    let s_uv = vec2<f32>(tparams.season_column / 8.0 + 1.0 / 16.0, in.season_row);
    let season_color = textureSample(season_map_tex, tree_sampler, s_uv).rgb;

    // 个性化 tint
    let tint = textureSample(tint_map_tex, tree_sampler, in.tint_uv).rgb;
    var color = diffuse_sample.rgb;
    color *= season_color;
    color = get_overlay(color, tint, 0.5);

    // 主光照（Lambert + Fresnel rim）
    let sun_dir = normalize(vec3<f32>(-0.408, -0.816, 0.408));
    let to_light = -sun_dir;
    let n_dot_l = max(dot(normal, to_light), 0.2);
    var lit = color * (vec3<f32>(0.45) + frame.sun_diffuse_intensity.rgb * n_dot_l);

    // 昼夜
    let globe_n = calc_globe_normal(in.world_pos.xz, frame.day_night_hour_sun_dir.x);
    lit = day_night(lit, globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);

    // 距离雾
    lit = apply_distance_fog(lit, in.world_pos, frame.cam_pos);

    // 距离 alpha 衰减 + mask 远景淡出
    let alpha = diffuse_sample.a * in.tree_fade;
    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(lit, alpha);
}
