// =============================================================================
// gui_button_local.wgsl — Phase 3.12.13 local binding layout
// =============================================================================
// Adapted from translations/gui_button.wgsl for the UI-pass-removed pipeline layout:
//   group(0) = uniform (dynamic offset)
//   group(1) = sprite_tex + sampler
//   group(2) = mask_tex + sampler (unused by button)

struct GuiButtonParams {
    proj: mat4x4<f32>,
    frame_state: vec4<f32>,
    variant: u32,
    state_flags: u32,
    _pad0: vec2<f32>,
    tint: vec4<f32>,
};
@group(0) @binding(0) var<uniform> gp: GuiButtonParams;

@group(1) @binding(0) var sprite_tex: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

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

fn sample_frame(uv: vec2<f32>, frame_idx: f32, total_frames: f32) -> vec4<f32> {
    // 4.3 Step A: total_frames <= 1 means "no atlas". UV is normalized texture coords
    // and may exceed [0,1] for 9-slice tiling. Pass straight through so the
    // sampler::Repeat address mode can tile the source texture as intended.
    if (total_frames <= 1.0) {
        return textureSample(sprite_tex, sprite_sampler, uv);
    }
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
        color = mix(s_cur, s_tgt, blend_t);
    } else if (variant == 2u) {
        let half_t = blend_t * 2.0;
        if (half_t < 1.0) {
            color = vec4<f32>(s_cur.rgb * (1.0 - half_t), s_cur.a);
        } else {
            color = vec4<f32>(s_tgt.rgb * (half_t - 1.0), s_tgt.a);
        }
    } else if (variant == 3u) {
        color = mix(s_cur, s_tgt, smoothstep(0.0, 1.0, blend_t));
    }

    if ((gp.state_flags & 1u) != 0u) {
        let LUMINANCE_VECTOR_LOCAL = vec3<f32>(0.2125, 0.7154, 0.0721);
        let grey = vec3<f32>(dot(color.rgb, LUMINANCE_VECTOR_LOCAL));
        color = vec4<f32>(mix(grey, color.rgb, 0.3) * 0.6, color.a);
    }
    if ((gp.state_flags & 2u) != 0u) {
        color = vec4<f32>(color.rgb * 1.2, color.a);
    }
    if ((gp.state_flags & 4u) != 0u) {
        color = vec4<f32>(color.rgb * 0.85, color.a);
    }

    return vec4<f32>(color.rgb * gp.tint.rgb, color.a * gp.tint.a);
}
