// =============================================================================
// gui_progress.wgsl — Phase 3.11.14 progress bar 5 变体合并
// =============================================================================
//
// 覆盖 vanilla：
// - `progress.shader` (基础水平进度)
// - `progress_minmax.shader` (双向)
// - `progress_radial.shader` (圆环)
// - `progress_reverse.shader` (反向)
// - `progress_startend.shader` (起止)
// - `circularprogressbar.shader` (圆形)

struct ProgressParams {
    proj: mat4x4<f32>,
    /// (current, max, variant, _pad)
    /// variant: 0=horizontal, 1=minmax, 2=radial, 3=reverse, 4=startend, 5=circular
    state: vec4<f32>,
    bg_color: vec4<f32>,
    fg_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> pp: ProgressParams;

@group(2) @binding(0) var bar_tex: texture_2d<f32>;
@group(2) @binding(1) var bar_sampler: sampler;

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
    out.clip_pos = pp.proj * vec4<f32>(in.screen_pos, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let progress = clamp(pp.state.x / max(pp.state.y, 1e-4), 0.0, 1.0);
    let variant = u32(pp.state.z);

    let tex = textureSample(bar_tex, bar_sampler, in.uv);
    var fill: f32 = 0.0;

    if (variant == 0u) {
        // horizontal
        fill = step(in.uv.x, progress);
    } else if (variant == 1u) {
        // minmax: bar grows from center
        let dist_from_center = abs(in.uv.x - 0.5) * 2.0;
        fill = step(dist_from_center, progress);
    } else if (variant == 2u || variant == 5u) {
        // radial / circular
        let centered = in.uv - vec2<f32>(0.5);
        let angle = atan2(centered.y, centered.x);
        let normalized = (angle + 3.1415927) / 6.2831853;
        fill = step(normalized, progress);
    } else if (variant == 3u) {
        // reverse
        fill = step(1.0 - in.uv.x, progress);
    } else if (variant == 4u) {
        // startend: 头尾都填，中间空
        let bar_t = in.uv.x;
        let from_start = step(bar_t, progress * 0.5);
        let from_end = step(1.0 - bar_t, progress * 0.5);
        fill = max(from_start, from_end);
    }

    let bar_color = mix(pp.bg_color, pp.fg_color, fill);
    return vec4<f32>(bar_color.rgb * tex.rgb, bar_color.a * tex.a);
}
