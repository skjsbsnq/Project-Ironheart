// =============================================================================
// gui_special_local.wgsl — Phase 3.12.13 local binding layout
// =============================================================================
// Adapted from translations/gui_special.wgsl:
//   group(0) = uniform (dynamic offset)
//   group(1) = content_tex + sampler
//   group(2) = mask_tex + sampler

struct GuiSpecialParams {
    proj: mat4x4<f32>,
    variant_and_pad: vec4<u32>,
    primary_color: vec4<f32>,
    secondary_color: vec4<f32>,
    chart_state: vec4<f32>,
};
@group(0) @binding(0) var<uniform> gsp: GuiSpecialParams;

@group(1) @binding(0) var content_tex: texture_2d<f32>;
@group(1) @binding(1) var content_sampler: sampler;

@group(2) @binding(0) var mask_tex: texture_2d<f32>;
@group(2) @binding(1) var mask_sampler: sampler;

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
    if (gsp.variant_and_pad.x == 0u) {
        let flag = textureSample(content_tex, content_sampler, in.uv);
        let mask = textureSample(mask_tex, mask_sampler, in.uv).a;
        if (mask < 0.05) { discard; }
        return vec4<f32>(flag.rgb, flag.a * mask * gsp.primary_color.a);
    } else if (gsp.variant_and_pad.x == 1u) {
        let bg = gsp.primary_color.rgb;
        let pattern = textureSample(content_tex, content_sampler, in.uv);
        let frame = textureSample(mask_tex, mask_sampler, in.uv);
        var color = mix(bg, pattern.rgb, pattern.a);
        color = mix(color, gsp.secondary_color.rgb, frame.r * frame.a);
        return vec4<f32>(color, max(pattern.a, frame.a));
    } else if (gsp.variant_and_pad.x == 2u) {
        let portrait = textureSample(content_tex, content_sampler, in.uv);
        let frame = textureSample(mask_tex, mask_sampler, in.uv);
        let highlight = max(0.0, 1.0 - in.uv.y * 2.0) * 0.1;
        var color = portrait.rgb + vec3<f32>(highlight);
        color = mix(color, frame.rgb, frame.a);
        return vec4<f32>(color, max(portrait.a, frame.a));
    } else {
        let v = clamp((gsp.chart_state.x - gsp.chart_state.y) /
                      max(gsp.chart_state.z - gsp.chart_state.y, 1e-4),
                      0.0, 1.0);
        let line_y = 1.0 - v;
        let dy = abs(in.uv.y - line_y);
        let line_alpha = smoothstep(gsp.chart_state.w + 0.005,
                                    gsp.chart_state.w, dy);
        return vec4<f32>(gsp.primary_color.rgb, line_alpha * gsp.primary_color.a);
    }
}
