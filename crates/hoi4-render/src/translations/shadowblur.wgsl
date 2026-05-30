// =============================================================================
// shadowblur.wgsl — Phase 3.11.12 vanilla `gfx/FX/shadowblur.shader`
// =============================================================================
//
// 对 shadow map 做 7-tap 高斯模糊。两个 pass：horizontal + vertical。
// vanilla 用 PCF 5-tap；我们 wgsl 这里用稍宽的可分离高斯。

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct BlurParams {
    /// (1/width, 1/height) 当 horizontal=1，否则 (0, 1/height)
    direction_inv_size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(2) var<uniform> bp: BlurParams;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    // 全屏三角形
    var out: VsOut;
    let uv = vec2<f32>(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.clip_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // 7-tap 离散高斯（σ ≈ 1.5）
    let weights = array<f32, 4>(0.227027, 0.194595, 0.121622, 0.054054);
    let dir = bp.direction_inv_size;

    var c = textureSample(src_tex, src_sampler, in.uv) * weights[0];
    c = c + textureSample(src_tex, src_sampler, in.uv + dir * 1.0) * weights[1];
    c = c + textureSample(src_tex, src_sampler, in.uv - dir * 1.0) * weights[1];
    c = c + textureSample(src_tex, src_sampler, in.uv + dir * 2.0) * weights[2];
    c = c + textureSample(src_tex, src_sampler, in.uv - dir * 2.0) * weights[2];
    c = c + textureSample(src_tex, src_sampler, in.uv + dir * 3.0) * weights[3];
    c = c + textureSample(src_tex, src_sampler, in.uv - dir * 3.0) * weights[3];
    return c;
}
