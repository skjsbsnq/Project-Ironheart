// Tree billboard shader — Phase 3.6.3.
//
// Each instance is a "card" in world space, billboarded around the Y axis
// (cylindrical billboard) so trees stay vertical regardless of camera tilt.
// 6 vertices form two triangles for one quad. The fragment shader carves the
// quad into a soft triangular crown shape with a darker trunk band.
//
// Distance-based LOD: trees fade out beyond a threshold to avoid cluttering
// the far view.

struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    map_size: vec4<f32>,
};

struct RenderParams {
    selected_province_id: u32,
    screen_width: f32,
    screen_height: f32,
    vignette_strength: f32,
    zoom_factor: f32,
    time: f32,
    border_country_px: f32,
    border_province_px: f32,
    sun_dir: vec4<f32>,
    month_phase: f32,
    season_snow_offset: f32,
    map_mode_terrain_blend: f32,
    diplomacy_mode: u32,
    object_opacity: f32,
    object_scale: f32,
    _reserved5: vec2<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> params: RenderParams;

struct VertexInput {
    @builtin(vertex_index) vid: u32,
    @location(0) pos: vec3<f32>,
    @location(1) scale: f32,
    @location(2) tint: vec4<f32>,
    // Phase 3.10.2: tree_type is now Uint8x2 (.x = type, .y = pad).
    @location(3) tree_type_packed: vec2<u32>,
    // Phase 3.10.2: slope (Snorm8x2) — billboard ignores it (vertical card),
    // but layout has to stay in sync with TreeInstance struct.
    @location(4) slope: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv: vec2<f32>,
    @location(1) tint: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
    @location(3) tree_type: u32,
    @location(4) dist_fade: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var local_x: f32 = -0.5;
    var local_y: f32 = 0.0;
    let v = input.vid;
    if (v == 1u || v == 2u || v == 4u) { local_x = 0.5; }
    if (v == 2u || v == 4u || v == 5u) { local_y = 1.0; }

    let to_cam = vec3<f32>(camera.eye.x - input.pos.x, 0.0, camera.eye.z - input.pos.z);
    let to_cam_len = max(length(to_cam), 1e-4);
    let to_cam_n = to_cam / to_cam_len;
    let right = vec3<f32>(to_cam_n.z, 0.0, -to_cam_n.x);
    let up = vec3<f32>(0.0, 1.0, 0.0);

    let width = input.scale * params.object_scale;
    let height = input.scale * params.object_scale * 2.2;

    let world_pos = input.pos + right * (local_x * width) + up * (local_y * height);

    // Distance-based fade.
    let max_view = camera.map_size.x * 0.5;
    let fade_start = max_view * 0.5;
    let fade_end = max_view * 0.75;
    let dist_fade = 1.0 - smoothstep(fade_start, fade_end, to_cam_len);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.local_uv = vec2<f32>(local_x, local_y);
    out.tint = input.tint.rgb;
    out.world_pos = world_pos;
    out.tree_type = input.tree_type_packed.x;
    out.dist_fade = dist_fade;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.dist_fade <= 0.01) { discard; }

    let cx = in.local_uv.x;
    let cy = in.local_uv.y;
    let abs_x = abs(cx);

    var alpha: f32;
    var color: vec3<f32>;

    // Tree shape varies by type.
    if (in.tree_type == 1u) {
        // Conifer: narrow pointed triangle (pine tree shape).
        let trunk_band = 0.10;
        if (cy < trunk_band) {
            if (abs_x < 0.04) {
                alpha = 1.0;
                color = vec3<f32>(0.28, 0.18, 0.10);
            } else { discard; }
        } else {
            let crown_t = (cy - trunk_band) / (1.0 - trunk_band);
            // Narrower crown for pine.
            let crown_half = (1.0 - crown_t) * 0.35;
            let edge = abs_x - crown_half;
            let aa = fwidth(edge) + 1e-4;
            alpha = 1.0 - smoothstep(-aa, aa, edge);
            if (alpha <= 0.01) { discard; }
            // Darker at bottom, lighter at top.
            let shade = mix(0.65, 1.1, crown_t);
            color = in.tint * shade;
        }
    } else if (in.tree_type == 2u) {
        // Tropical: palm-like shape (thin trunk + top tuft).
        let trunk_top = 0.65;
        if (cy < trunk_top) {
            // Thin curved trunk.
            let trunk_width = 0.03 + cy * 0.01;
            if (abs_x < trunk_width) {
                alpha = 1.0;
                color = vec3<f32>(0.35, 0.22, 0.12);
            } else { discard; }
        } else {
            // Fan-shaped crown at top.
            let crown_t = (cy - trunk_top) / (1.0 - trunk_top);
            let crown_half = (1.0 - crown_t * 0.7) * 0.45;
            let edge = abs_x - crown_half;
            let aa = fwidth(edge) + 1e-4;
            alpha = 1.0 - smoothstep(-aa, aa, edge);
            if (alpha <= 0.01) { discard; }
            let shade = mix(0.8, 1.1, crown_t);
            color = in.tint * shade;
        }
    } else {
        // Deciduous: rounded/triangular crown (default).
        let trunk_band = 0.12;
        if (cy < trunk_band) {
            if (abs_x < 0.05) {
                alpha = 1.0;
                color = vec3<f32>(0.30, 0.19, 0.11);
            } else { discard; }
        } else {
            let crown_t = (cy - trunk_band) / (1.0 - trunk_band);
            // Rounder crown shape (parabolic).
            let crown_half = (1.0 - crown_t * crown_t) * 0.48;
            let edge = abs_x - crown_half;
            let aa = fwidth(edge) + 1e-4;
            alpha = 1.0 - smoothstep(-aa, aa, edge);
            if (alpha <= 0.01) { discard; }
            let shade = mix(0.7, 1.05, crown_t);
            color = in.tint * shade;
        }
    }

    // Lambert lighting.
    let sun = normalize(params.sun_dir.xyz);
    let approx_normal = vec3<f32>(0.0, 0.7, -0.7);
    let lambert = clamp(dot(approx_normal, sun), 0.4, 1.0);
    color = color * lambert;

    // Distance fade.
    alpha = alpha * in.dist_fade * params.object_opacity;

    return vec4<f32>(color, alpha);
}
