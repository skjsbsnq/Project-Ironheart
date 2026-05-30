// Phase 3.5: Building icons — instanced colored squares.
// Vertex layout: [f32;3] pos + f32 kind (0=civ, 1=mil, 2=dock)

struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    map_size: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct BuildingParams {
    opacity: f32,
    scale: f32,
    brightness: f32,
    _pad0: f32,
};
@group(0) @binding(1) var<uniform> building_params: BuildingParams;

struct VsIn {
    @builtin(vertex_index) vid: u32,
    @location(0) pos: vec3<f32>,
    @location(1) kind: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // Billboard quad: 6 vertices (2 triangles)
    let size = 0.06 * building_params.scale;
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let c = corners[in.vid % 6u];

    // Billboard facing camera
    let to_cam = normalize(camera.eye.xyz - in.pos);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_cam));
    let up = vec3<f32>(0.0, 1.0, 0.0);

    let world = in.pos + right * c.x * size + up * c.y * size;

    var out: VsOut;
    out.clip_pos = camera.view_proj * vec4<f32>(world, 1.0);

    // Color by type
    if in.kind < 0.5 {
        out.color = vec3<f32>(0.2, 0.6, 0.2); // civilian = green
    } else if in.kind < 1.5 {
        out.color = vec3<f32>(0.7, 0.2, 0.2); // military = red
    } else {
        out.color = vec3<f32>(0.2, 0.3, 0.7); // dockyard = blue
    }
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color * building_params.brightness, 0.85 * building_params.opacity);
}
