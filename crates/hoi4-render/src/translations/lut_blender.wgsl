// =============================================================================
// lut_blender.wgsl — Phase 3.11.13 vanilla `gfx/FX/lut_blender.shader`
// =============================================================================
//
// 3D LUT 调色（vanilla `gfx/lut/*.dds` 全套 — 政治模式 / 战时 / DLC 主题色）。
// 输入 RGB → LUT 查表 → 输出调色后 RGB。本 shader 假定 LUT 是 16×16×16
// 解开成 256×16 的 2D atlas（vanilla 习惯）。

//#include "shader_lib.wgsl"

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var lut_tex: texture_2d<f32>;
@group(0) @binding(3) var lut_sampler: sampler;

struct LutParams {
    /// LUT 强度（0 = 原图，1 = 全 LUT）
    strength: f32,
    /// LUT 维度（vanilla = 16；shader 假定）
    lut_size: f32,
    _pad0: f32,
    _pad1: f32,
};
@group(0) @binding(4) var<uniform> lp: LutParams;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

/// 16×16 = 256 行 LUT 查表，blue 维度横向铺开 16 个 16×16 block。
fn apply_lut(color: vec3<f32>) -> vec3<f32> {
    let n = lp.lut_size;
    let r = clamp(color.r, 0.0, 1.0);
    let g = clamp(color.g, 0.0, 1.0);
    let b = clamp(color.b, 0.0, 1.0);

    let b_idx = b * (n - 1.0);
    let b_lo = floor(b_idx);
    let b_hi = min(b_lo + 1.0, n - 1.0);
    let b_frac = b_idx - b_lo;

    let cell_w = 1.0 / n;
    let inv_n = 1.0 / n;

    let u_lo = (b_lo + r * (1.0 - inv_n) + 0.5 * inv_n) * cell_w;
    let u_hi = (b_hi + r * (1.0 - inv_n) + 0.5 * inv_n) * cell_w;
    let v = g * (1.0 - inv_n) + 0.5 * inv_n;

    let lo = textureSample(lut_tex, lut_sampler, vec2<f32>(u_lo, v)).rgb;
    let hi = textureSample(lut_tex, lut_sampler, vec2<f32>(u_hi, v)).rgb;
    return mix(lo, hi, b_frac);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let src = textureSample(src_tex, src_sampler, in.uv).rgb;
    let lut = apply_lut(src);
    return vec4<f32>(mix(src, lut, lp.strength), 1.0);
}
