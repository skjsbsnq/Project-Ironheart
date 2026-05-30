// Railway line shader. Renders as LineList with per-vertex position + level.

struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    map_size: vec4<f32>,
};

struct RailwayParams {
    alpha: f32,
    zoom_factor: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> rparams: RailwayParams;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) level: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) level: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(input.pos, 1.0);
    out.level = input.level;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (rparams.alpha <= 0.01) {
        discard;
    }

    // Level 1 = light track, 2 = medium, 3 = major trunk.
    let t = clamp((in.level - 1.0) / 2.0, 0.0, 1.0);
    let minor = vec3<f32>(0.48, 0.44, 0.36);
    let major = vec3<f32>(0.18, 0.16, 0.13);
    let color = mix(minor, major, t);
    let close_boost = smoothstep(0.42, 0.82, rparams.zoom_factor);
    let alpha = rparams.alpha * mix(0.48, 0.74, t) * mix(0.78, 1.0, close_boost);
    return vec4<f32>(color, clamp(alpha, 0.0, 0.82));
}
