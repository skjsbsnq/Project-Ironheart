// =============================================================================
// maparrow.wgsl — Phase 3.11.11 vanilla `gfx/FX/maparrow.shader`
// =============================================================================
//
// 单位移动 / 入侵 / 空降 / 海运箭头：贝塞尔曲线 + 闪烁动画 + 起点 / 终点头尾。
// 通过 instanced quad 渲染，每个 instance 是路径上的一段。

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct ArrowParams {
    blink_speed: f32,
    blink_range: f32,
    body_color: vec4<f32>,
    head_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> a_params: ArrowParams;

@group(2) @binding(0) var arrow_tex: texture_2d<f32>;
@group(2) @binding(1) var arrow_sampler: sampler;

struct VsIn {
    /// 起点世界 XZ
    @location(0) start_xz: vec2<f32>,
    /// 终点世界 XZ
    @location(1) end_xz: vec2<f32>,
    /// 路径段宽度
    @location(2) width: f32,
    /// 节点类型 (0=body, 1=head, 2=tail)
    @location(3) segment_type: u32,
    /// 局部 quad 顶点 (-1,-1)..(1,1)
    @location(4) corner: vec2<f32>,
    @location(5) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) seg_t: f32,
    @location(2) seg_type: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let dir = in.end_xz - in.start_xz;
    let len = length(dir);
    let dir_n = dir / max(len, 0.001);
    let perp = vec2<f32>(-dir_n.y, dir_n.x);

    // 沿路径进度 ∈ [0, 1]
    let t = in.corner.x * 0.5 + 0.5;
    let along = mix(in.start_xz, in.end_xz, t);
    let world_xz = along + perp * in.corner.y * in.width;

    out.clip_pos = frame.view_proj * vec4<f32>(world_xz.x, 0.5, world_xz.y, 1.0);
    out.uv = in.uv;
    out.seg_t = t;
    out.seg_type = f32(in.segment_type);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(arrow_tex, arrow_sampler, in.uv);
    if (s.a < 0.05) {
        discard;
    }

    // 闪烁：cos(time × blink_speed) × range
    let blink = (cos(frame.global_time * a_params.blink_speed) * 0.5 + 0.5) * a_params.blink_range;
    var color = a_params.body_color.rgb;
    if (in.seg_type > 0.5) {
        // head / tail 染色
        color = a_params.head_color.rgb;
    }
    color *= 1.0 + blink;

    return vec4<f32>(color * s.rgb, s.a * a_params.body_color.a);
}
