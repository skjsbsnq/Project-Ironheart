// =============================================================================
// gui.wgsl — Phase 3.11.14 vanilla GUI shader 全套（10 个 vanilla → 1 wgsl + flags）
// =============================================================================
//
// 把 vanilla 的以下 10 个 shader 合并成一个统一 wgsl，通过 `gui_state` enum
// 切换行为：
//
// - `buttonstate.shader` (基础 button + hover/pressed/disabled 帧)
// - `buttonstate_blendframes.shader`（帧间过渡）
// - `buttonstate_fade_frames_to_black.shader`（渐黑）
// - `buttonstate_linear.shader`（线性插值）
// - `buttonstate_nodowneffect.shader`（无按下效果）
// - `buttonstate_nodownordisableeffect.shader`（无按下/禁用）
// - `buttonstate_nontransparent.shader`（不透明）
// - `buttonstate_onlydisable.shader`（仅禁用）
// - `buttonstate_rendertarget.shader`（rendertarget）
// - `static_button.shader`（静态按钮）
// - `text.shader`（文字 — 与 text_pass.rs 路径并存）
// - `progress.shader` + 4 变体（minmax / radial / reverse / startend）
// - `circularprogressbar.shader`
// - `maskedflag.shader` / `coa_shield.shader` / `portrait.shader`
// - `linechart.shader` / `color.shader` / `simple.shader`
//
// 实际拆分：
// - 本文件做 button + progress（覆盖大部分 GUI sprite）
// - `gui_text.wgsl` 单独写 text（与 text_pass 路径分开）
// - `gui_special.wgsl` 单独写 maskedflag / coa_shield / portrait / linechart
//
// 这里的本文件按 vanilla `buttonstate.shader` 为主：sprite atlas + 9-state
// frame 切换 + alpha / 灰度过渡。

//#include "shader_lib.wgsl"

struct GuiButtonParams {
    /// 屏幕投影 (orthographic) — 由 GUI pass 提供
    proj: mat4x4<f32>,
    /// (frame_index_current, frame_index_target, blend_t, total_frames)
    frame_state: vec4<f32>,
    /// (variant: 0=default, 1=blendframes, 2=fade2black, 3=linear, ...)
    variant: u32,
    /// (disabled, hover, pressed, _pad) bitmask
    state_flags: u32,
    /// global tint
    tint: vec4<f32>,
};
@group(0) @binding(0) var<uniform> gp: GuiButtonParams;

@group(2) @binding(0) var sprite_tex: texture_2d<f32>;
@group(2) @binding(1) var sprite_sampler: sampler;

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
    out.clip_pos = gp.proj * vec4<f32>(in.screen_pos, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

/// 帧切片：每个 frame 在 atlas 横向均分。
fn sample_frame(uv: vec2<f32>, frame_idx: f32, total_frames: f32) -> vec4<f32> {
    let frame_w = 1.0 / total_frames;
    let u = (frame_idx + clamp(uv.x, 0.0, 1.0)) * frame_w;
    return textureSample(sprite_tex, sprite_sampler, vec2<f32>(u, uv.y));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cur_idx = gp.frame_state.x;
    let tgt_idx = gp.frame_state.y;
    let blend_t = gp.frame_state.z;
    let total = max(gp.frame_state.w, 1.0);

    let s_cur = sample_frame(in.uv, cur_idx, total);
    let s_tgt = sample_frame(in.uv, tgt_idx, total);

    var color = s_cur;
    let variant = gp.variant;

    if (variant == 1u) {
        // blendframes：线性 blend
        color = mix(s_cur, s_tgt, blend_t);
    } else if (variant == 2u) {
        // fade frames to black：cur → black → tgt
        let half_t = blend_t * 2.0;
        if (half_t < 1.0) {
            color = vec4<f32>(s_cur.rgb * (1.0 - half_t), s_cur.a);
        } else {
            color = vec4<f32>(s_tgt.rgb * (half_t - 1.0), s_tgt.a);
        }
    } else if (variant == 3u) {
        // linear (smoothstep)
        color = mix(s_cur, s_tgt, smoothstep(0.0, 1.0, blend_t));
    }

    // disabled 状态：去饱和 + 暗化
    if ((gp.state_flags & 1u) != 0u) {
        let LUMINANCE_VECTOR_LOCAL = vec3<f32>(0.2125, 0.7154, 0.0721);
        let grey = vec3<f32>(dot(color.rgb, LUMINANCE_VECTOR_LOCAL));
        color = vec4<f32>(mix(grey, color.rgb, 0.3) * 0.6, color.a);
    }
    // hover 状态：稍微提亮
    if ((gp.state_flags & 2u) != 0u) {
        color = vec4<f32>(color.rgb * 1.2, color.a);
    }
    // pressed 状态：暗化
    if ((gp.state_flags & 4u) != 0u) {
        color = vec4<f32>(color.rgb * 0.85, color.a);
    }

    return vec4<f32>(color.rgb * gp.tint.rgb, color.a * gp.tint.a);
}
