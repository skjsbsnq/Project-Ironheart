// =============================================================================
// river.wgsl — Phase 3.11.6 vanilla `gfx/FX/river.shader` 等价翻译
// =============================================================================
//
// **替代谁**：当前 shader.wgsl 内嵌的"navy 蓝叠加"。本文件按 vanilla：
//
// 1. **`RiverSurface_diffuse_{0,1,2}.dds`** + `RiverSurface_normal_{0,1,2}.dds`
//    + `RiverSurface_masks.dds` 全套
// 2. **流向贴图**：`rivers.bmp` palette 0/1 = source/mouth；shader 按
//    流向滚动 normal UV
// 3. **河流宽度**：mask R/G/B/A 通道编码 level 1-4 → 影响透明度
// 4. **独立 pass**：river 是 LineList 几何，本 shader 用线段实例化绘制

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct RiverParams {
    flow_speed: f32,
    base_alpha: f32,
    /// LOD 阈值（zoom_factor < threshold 时仅画大河）
    lod_threshold: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> rparams: RiverParams;

@group(2) @binding(0) var river_diffuse: texture_2d<f32>;
@group(2) @binding(1) var river_normal: texture_2d<f32>;
@group(2) @binding(2) var river_masks: texture_2d<f32>;
@group(2) @binding(3) var rivers_bmp: texture_2d<f32>; // R8 with packed level + flow
@group(2) @binding(4) var river_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    /// 流向（来自 rivers.bmp source/mouth 拓扑），单位向量
    @location(2) flow_dir: vec2<f32>,
    /// 河流等级 1..4（vanilla mask 通道编码）
    @location(3) level: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) flow_dir: vec2<f32>,
    @location(3) level: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.pos, 1.0);
    out.world_pos = in.pos;
    out.uv = in.uv;
    out.flow_dir = in.flow_dir;
    out.level = in.level;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // 流向滚动 UV：扣除 time × flow_dir
    let scrolled_uv = in.uv - in.flow_dir * frame.global_time * rparams.flow_speed;

    // 漫反射（diffuse）+ 法线
    let diffuse = textureSample(river_diffuse, river_sampler, scrolled_uv).rgb;
    let normal = unpack_normal(textureSample(river_normal, river_sampler, scrolled_uv).rgb);

    // 流向相关高光（与 dot(flow_dir, view_dir) 相关）
    let to_camera = normalize(frame.cam_pos - in.world_pos);
    let half = normalize(to_camera + vec3<f32>(in.flow_dir.x, 1.0, in.flow_dir.y));
    let spec = pow(max(dot(normal, half), 0.0), 64.0) * 0.4;

    // 等级 → mask 通道映射（level 1-4 → channel R/G/B/A）
    let mask = textureSample(river_masks, river_sampler, scrolled_uv);
    let level_alpha = mix(
        mix(mask.r, mask.g, clamp(in.level - 1.0, 0.0, 1.0)),
        mix(mask.b, mask.a, clamp(in.level - 3.0, 0.0, 1.0)),
        clamp((in.level - 2.0) * 0.5, 0.0, 1.0),
    );

    var color = diffuse + vec3<f32>(spec);
    color = day_night(
        color,
        calc_globe_normal(in.world_pos.xz, frame.day_night_hour_sun_dir.x),
        frame.day_night_hour_sun_dir.yzw,
        1.0,
    );
    color = apply_distance_fog(color, in.world_pos, frame.cam_pos);

    let alpha = level_alpha * rparams.base_alpha;
    return vec4<f32>(color, alpha);
}
