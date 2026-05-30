// Phase 3.6.3: Instanced 3D mesh tree shader.
//
// Each tree instance provides a world position + scale + tint.
// The mesh geometry (position, normal, UV) is shared across all instances
// of the same tree type. Multiple draw calls handle different mesh types.

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
@group(0) @binding(2) var diffuse_tex: texture_2d<f32>;
@group(0) @binding(3) var diffuse_sampler: sampler;

// Mesh vertex data (vertex-rate)
struct MeshVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

// Per-instance data (instance-rate)
struct InstanceData {
    @location(3) inst_pos: vec3<f32>,
    @location(4) inst_scale: f32,
    @location(5) inst_tint: vec4<f32>,
    // Phase 3.10.2: per-instance terrain slope (Snorm8x2 → vec2<f32> in [-1, 1]).
    // Used to tilt the tree's vertical extent so the trunk follows the
    // terrain instead of poking sideways.
    @location(6) inst_slope: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) tint: vec3<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) dist_fade: f32,
};

@vertex
fn vs_main(mesh: MeshVertex, inst: InstanceData) -> VsOut {
    // Transform mesh vertex by instance position + uniform scale.
    // Paradox meshes are Y-up, which matches our world space.
    let scaled_pos = mesh.position * inst.inst_scale * params.object_scale;

    // Phase 3.10.2: slope correction (vSlopes equivalent). Lift Y by a
    // multiple of the local XZ position so taller mesh vertices follow the
    // terrain. inst_slope is already in "world-Y per world-XZ" units —
    // applying it directly to scaled_pos.{x,z} keeps trunk verticality
    // proportional to canopy reach.
    var world_pos = scaled_pos + inst.inst_pos;
    world_pos.y += scaled_pos.x * inst.inst_slope.x
                + scaled_pos.z * inst.inst_slope.y;

    // Distance-based LOD fade.
    let to_cam = camera.eye.xyz - world_pos;
    let dist = length(to_cam);
    let max_view = camera.map_size.x * 0.5;
    let fade_start = max_view * 0.55;
    let fade_end = max_view * 0.75;
    let dist_fade = 1.0 - smoothstep(fade_start, fade_end, dist);

    var out: VsOut;
    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_normal = mesh.normal; // uniform scale doesn't distort normals
    out.uv = mesh.uv;
    out.tint = inst.inst_tint.rgb;
    out.world_pos = world_pos;
    out.dist_fade = dist_fade;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (in.dist_fade <= 0.01) { discard; }

    // Sample diffuse texture.
    let tex_color = textureSample(diffuse_tex, diffuse_sampler, in.uv);

    // Alpha test — tree textures have transparent backgrounds.
    if (tex_color.a < 0.3) { discard; }

    // Apply tint (modulate).
    var color = tex_color.rgb * in.tint * 1.4;

    // Simple Lambert lighting.
    let sun = normalize(params.sun_dir.xyz);
    let n = normalize(in.world_normal);
    let lambert = max(dot(n, sun), 0.0);
    let ambient = 0.55;
    let shade = ambient + (1.0 - ambient) * lambert;
    color = color * shade;

    // Apply distance fade to alpha.
    let alpha = tex_color.a * in.dist_fade * params.object_opacity;

    return vec4<f32>(color, alpha);
}
