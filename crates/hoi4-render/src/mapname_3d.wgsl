// Phase 3.10.3 — 3D country-name labels.
//
// Each draw call emits one quad per country with vertex_count = 6 (two
// triangles in `[-1, +1] × [-1, +1]` local UV space). The instance buffer
// supplies the world-space centre, the major axis direction in the XZ plane,
// half-extents for both axes, and the atlas UV rect.
//
// Vertex shader:
//   axis2 = perp(axis1) in XZ
//   world = center + axis1·(local_x · width)  + axis2·(local_y · height)
//
// `clip.z -= bias·clip.w` pulls the quad slightly toward the camera in NDC
// space, keeping it from z-fighting the heightmap mesh underneath without
// changing the perspective look.
//
// Fragment shader: sample R8 atlas. Two-tier encoding (baked in
// `hoi4-app::mapname_atlas`):
//   v ∈ [0.05, 0.5]  → black outline (alpha = v * 2.0, capped)
//   v ∈ [0.5, 1.0]   → white text fill (alpha = 1.0)
// Day/night dim derived from the current sun direction in `RenderParams`.

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
@group(0) @binding(2) var atlas_tex: texture_2d<f32>;
@group(0) @binding(3) var atlas_sampler: sampler;

// Per-instance data (instance-rate buffer).
struct InstanceData {
    @location(0) center: vec3<f32>,
    @location(1) width_world: f32,
    @location(2) axis1: vec2<f32>,
    @location(3) height_world: f32,
    @location(4) _pad0: f32,
    @location(5) uv_min: vec2<f32>,
    @location(6) uv_max: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceData) -> VsOut {
    // 6 verts → two triangles for a quad in [-1, +1] × [-1, +1].
    var local_x: f32 = -1.0;
    var local_y: f32 = -1.0;
    if (vid == 1u || vid == 2u || vid == 4u) { local_x = 1.0; }
    if (vid == 2u || vid == 4u || vid == 5u) { local_y = 1.0; }

    // axis1_world: country major axis, embedded in XZ plane.
    let axis1_world = vec3<f32>(inst.axis1.x, 0.0, inst.axis1.y);
    // axis2_world: perpendicular in XZ (rotate 90° → (-y, +x)).
    let axis2_world = vec3<f32>(-inst.axis1.y, 0.0, inst.axis1.x);

    let world_pos = inst.center
        + axis1_world * (local_x * inst.width_world)
        + axis2_world * (local_y * inst.height_world);

    let uv = vec2<f32>(
        mix(inst.uv_min.x, inst.uv_max.x, (local_x + 1.0) * 0.5),
        // local_y = -1 → axis2 negative → smaller world Z → "up" on screen
        // (top-down camera: smaller Z is closer to screen top). Map that
        // to atlas top (uv.y = uv_min.y) so the text reads right-side up.
        mix(inst.uv_min.y, inst.uv_max.y, (local_y + 1.0) * 0.5),
    );

    var clip = camera.view_proj * vec4<f32>(world_pos, 1.0);
    // z-bias toward camera (~1/1000 NDC) — kills z-fight with terrain
    // without making the label disappear behind nearby tall trees.
    clip.z = clip.z - 0.001 * clip.w;

    var out: VsOut;
    out.clip_pos = clip;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let v = textureSample(atlas_tex, atlas_sampler, in.uv).r;
    if (v < 0.04) { discard; }

    // Day/night: project sun_dir onto +Y. >0 → daytime, <0 → night.
    // Half-spherical lerp 0.6..1.0 gives a clear dimming effect without
    // making nighttime labels unreadable.
    let sun = normalize(params.sun_dir.xyz);
    let day = clamp(sun.y * 0.5 + 0.5, 0.0, 1.0);
    let dim = mix(0.6, 1.0, day);

    // Two-tier encoding: outline (v ∈ [0.04, 0.5]) blended with text fill
    // (v ∈ [0.5, 1.0]). `text_w` is "this pixel is text fill"; outline_a
    // is the alpha multiplier of the black ring.
    let text_w = smoothstep(0.45, 0.7, v);
    let outline_a = clamp(v * 2.0, 0.0, 1.0);

    let text_color = vec3<f32>(0.96, 0.93, 0.85) * dim;
    let outline_color = vec3<f32>(0.05, 0.04, 0.03);

    let color = mix(outline_color, text_color, text_w);
    let alpha = max(text_w, outline_a);

    return vec4<f32>(color, alpha);
}
