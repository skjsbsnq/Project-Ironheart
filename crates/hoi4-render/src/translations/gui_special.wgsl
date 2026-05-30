// =============================================================================
// gui_special.wgsl — Phase 3.11.14 国旗 / 国徽 / 头像 / linechart
// =============================================================================
//
// 覆盖 vanilla：
// - `maskedflag.shader`（国旗按 mask 圆形 / 盾形）
// - `coa_shield.shader`（纹章盾，~216 行 vanilla）
// - `portrait.shader`（人物头像，~128 行）
// - `linechart.shader`（经济政治曲线图，~96 行）

struct GuiSpecialParams {
    proj: mat4x4<f32>,
    /// variant: 0=maskedflag, 1=coa_shield, 2=portrait, 3=linechart
    variant: u32,
    _pad: vec3<u32>,
    primary_color: vec4<f32>,
    secondary_color: vec4<f32>,
    /// (chart_value, chart_min, chart_max, line_thickness)
    chart_state: vec4<f32>,
};
@group(0) @binding(0) var<uniform> gsp: GuiSpecialParams;

@group(2) @binding(0) var content_tex: texture_2d<f32>;
@group(2) @binding(1) var mask_tex: texture_2d<f32>;
@group(2) @binding(2) var content_sampler: sampler;

struct VsIn {
    @location(0) screen_pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};
struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = gsp.proj * vec4<f32>(in.screen_pos, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (gsp.variant == 0u) {
        // maskedflag — 旗帜按 mask alpha 裁剪
        let flag = textureSample(content_tex, content_sampler, in.uv);
        let mask = textureSample(mask_tex, content_sampler, in.uv).a;
        if (mask < 0.05) { discard; }
        return vec4<f32>(flag.rgb, flag.a * mask * gsp.primary_color.a);
    } else if (gsp.variant == 1u) {
        // coa_shield — 纹章盾（背景色 + 图案 + 边框 mask）
        let bg = gsp.primary_color.rgb;
        let pattern = textureSample(content_tex, content_sampler, in.uv);
        let frame = textureSample(mask_tex, content_sampler, in.uv);
        var color = mix(bg, pattern.rgb, pattern.a);
        color = mix(color, gsp.secondary_color.rgb, frame.r * frame.a);
        return vec4<f32>(color, max(pattern.a, frame.a));
    } else if (gsp.variant == 2u) {
        // portrait — 头像 + 玻璃反光 + 边框
        let portrait = textureSample(content_tex, content_sampler, in.uv);
        let frame = textureSample(mask_tex, content_sampler, in.uv);
        // 简单玻璃反光：上半部分加 ~10% 白
        let highlight = max(0.0, 1.0 - in.uv.y * 2.0) * 0.1;
        var color = portrait.rgb + vec3<f32>(highlight);
        color = mix(color, frame.rgb, frame.a);
        return vec4<f32>(color, max(portrait.a, frame.a));
    } else {
        // linechart — 在 UV space 画一条曲线（简化：单点高亮）
        let v = clamp((gsp.chart_state.x - gsp.chart_state.y) /
                      max(gsp.chart_state.z - gsp.chart_state.y, 1e-4),
                      0.0, 1.0);
        // 当前帧的曲线 uv y 应该是 1 - v；周围 thickness 内涂线色
        let line_y = 1.0 - v;
        let dy = abs(in.uv.y - line_y);
        let line_alpha = smoothstep(gsp.chart_state.w + 0.005,
                                    gsp.chart_state.w, dy);
        return vec4<f32>(gsp.primary_color.rgb, line_alpha * gsp.primary_color.a);
    }
}
