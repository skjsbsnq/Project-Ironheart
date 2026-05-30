// Phase 3.3: Post-processing pass (vignette + tonemap + saturation).
// Equivalent to vanilla restorescene.shader + saturation_slider.shader.

struct PostFxParams {
    screen_size: vec2<f32>,
    vignette_strength: f32,
    saturation: f32,
    exposure: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> params: PostFxParams;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle (3 vertices, no vertex buffer needed)
@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    // Generate fullscreen triangle: vid 0,1,2 → covers [-1,1]²
    let x = f32(i32(vid & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vid >> 1u)) * 4.0 - 1.0;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// ACES filmic tonemap (simple approximation)
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = x * (x + vec3<f32>(0.0245786)) - vec3<f32>(0.000090537);
    let b = x * (vec3<f32>(0.983729) * x + vec3<f32>(0.4329510)) + vec3<f32>(0.238081);
    return a / b;
}

// Vignette: darken edges
fn vignette(uv: vec2<f32>, strength: f32) -> f32 {
    let d = distance(uv, vec2<f32>(0.5, 0.5));
    return 1.0 - strength * smoothstep(0.3, 0.85, d);
}

// Saturation adjustment
fn adjust_saturation(color: vec3<f32>, sat: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luma), color, sat);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, scene_sampler, in.uv).rgb;

    // 1) Exposure
    var color = scene * params.exposure;

    // 2) Tonemap
    color = aces_tonemap(color);

    // 3) Saturation
    color = adjust_saturation(color, params.saturation);

    // 4) Vignette
    let vig = vignette(in.uv, params.vignette_strength);
    color = color * vig;

    // 5) Gamma (linear → sRGB)
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
