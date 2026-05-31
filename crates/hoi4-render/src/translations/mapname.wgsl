// =============================================================================
// mapname.wgsl — Phase 3.11.9 vanilla `gfx/FX/mapname.shader` 等价翻译
// =============================================================================
//
// **关系**：本工程已有 `mapname_3d.wgsl`（自研 OBB + atlas 路径，Phase 3.10.3）。
// 本文件按 vanilla 原版路径：
//
// 1. **VS**: `vDistortedPos = vPos - vCamLookAtDir * 0.5`，反挤近相机防 z-fight
// 2. **PS**: `DayNightFactor(CalcGlobeNormal(prepos.xz)) * 0.35` 昼夜暗化
// 3. **stencil**: ref=4 / not_equal — UI sprite 区域置 4，标签自动隐藏
//
// 与 3.10.3 自研路径的关系：
// - 3.10.3 path 用 OBB 计算 + fontdue R8 atlas，永远可用
// - 3.11.9 path 是默认（资产齐全时），fallback 到 3.10.3 当 atlas 烘焙失败
// - 切换由 main.rs 在 pipeline 创建时决定

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct MapnameParams {
    text_color: vec4<f32>,
    outline_color: vec4<f32>,
    /// 反挤系数（vanilla = 0.5）
    distortion_amount: f32,
    /// 整体 alpha
    fade: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> mn_params: MapnameParams;

@group(2) @binding(0) var name_atlas: texture_2d<f32>;
@group(2) @binding(1) var name_sampler: sampler;

struct VsIn {
    /// 标签 quad 的世界中心 + 尺寸（xy=center.xz, zw=halfwidth/halfheight）
    @location(0) center_size: vec4<f32>,
    /// quad 顶点局部坐标 (-1,-1) (-1,1) (1,1) (1,-1)
    @location(1) corner: vec2<f32>,
    /// atlas UV 矩形 (uv_min.xy, uv_max.xy)
    @location(2) uv_rect: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let center_xz = in.center_size.xy;
    let half = in.center_size.zw;
    let world = vec3<f32>(
        center_xz.x + in.corner.x * half.x,
        0.5,
        center_xz.y + in.corner.y * half.y,
    );

    // vanilla: 反挤朝相机近端，避免 z-fight 与树 / 单位
    let to_cam = normalize(frame.cam_pos - world);
    let distorted = world + to_cam * mn_params.distortion_amount;

    out.clip_pos = frame.view_proj * vec4<f32>(distorted, 1.0);
    // 朝相机方向 NDC 偏移
    out.clip_pos.z = out.clip_pos.z - 0.001 * out.clip_pos.w;

    let u = mix(in.uv_rect.x, in.uv_rect.z, in.corner.x * 0.5 + 0.5);
    let v = mix(in.uv_rect.y, in.uv_rect.w, in.corner.y * 0.5 + 0.5);
    out.uv = vec2<f32>(u, v);
    out.world_xz = center_xz;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(name_atlas, name_sampler, in.uv).r;
    if (s < 0.05) {
        discard;
    }

    // R8 atlas two-tier 编码：>= 0.65 = text，0.3..0.65 = outline，< 0.3 = transparent
    let text_t = smoothstep(0.45, 0.7, s);
    let outline_t = smoothstep(0.15, 0.45, s) * (1.0 - text_t);

    var color = mn_params.outline_color.rgb * outline_t + mn_params.text_color.rgb * text_t;
    let alpha = (text_t + outline_t) * mn_params.fade;

    // 昼夜暗化（vanilla 系数 0.35）
    let map_px = world_xz_to_map_px(in.world_xz, frame.vanilla_map_size_world_size.zw);
    let globe_n = calc_globe_normal(map_px, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    let dim = 1.0 - night * 0.35;
    color *= dim;

    return vec4<f32>(color, alpha);
}
