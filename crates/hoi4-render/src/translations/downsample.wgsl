// =============================================================================
// downsample.wgsl — Phase 3.11.13 vanilla `gfx/FX/downsample.shader`
// =============================================================================
//
// 高斯下采样 1/2 → 1/4 → 1/8 用于 bloom 链。13-tap "Kawase" / "PartialKarisAvg"
// 风格，远比朴素 bilinear 干净。

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct DownParams {
    /// (inv_src_w, inv_src_h)
    inv_src_size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(2) var<uniform> dp: DownParams;

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = dp.inv_src_size;
    // 13-tap downsample (Call of Duty / Bartlomiej Wronski 2014 paper)
    let a = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>(-1.0,  1.0));
    let b = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 1.0,  1.0));
    let c = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>(-1.0, -1.0));
    let d = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 1.0, -1.0));

    let e = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>(-2.0,  0.0));
    let f = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 2.0,  0.0));
    let g = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 0.0,  2.0));
    let h = textureSample(src_tex, src_sampler, in.uv + p * vec2<f32>( 0.0, -2.0));

    let i = textureSample(src_tex, src_sampler, in.uv);

    return (a + b + c + d) * 0.125 + (e + f + g + h) * 0.0625 + i * 0.25;
}
