// Phase 14: POI icon pass — instanced colored billboards with shape differentiation.
// Vertex layout: [f32;3] pos + f32 kind + f32 level (20 bytes per instance)
//
// Each instance is a small world-space billboard (camera-facing quad).
// `kind` selects the color and shape; `level` scales the icon slightly
// for higher-level buildings.

struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    map_size: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct PoiIconParams {
    opacity: f32,
    scale: f32,
    outline_strength: f32,
    _pad0: f32,
};
@group(0) @binding(1) var<uniform> poi_params: PoiIconParams;

struct VsIn {
    @builtin(vertex_index) vid: u32,
    @location(0) pos: vec3<f32>,
    @location(1) kind: f32,
    @location(2) level: f32,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_uv: vec2<f32>,
    @location(1) kind: f32,
    @location(2) level: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[in.vid % 6u];

    let to_cam = normalize(camera.eye.xyz - in.pos);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), to_cam));
    let up = vec3<f32>(0.0, 1.0, 0.0);

    let base_size = (0.055 + 0.005 * clamp(in.level, 1.0, 10.0)) * poi_params.scale;
    let world = in.pos + right * (c.x - 0.5) * base_size * 2.0 + up * (c.y - 0.5) * base_size * 2.0;

    var out: VsOut;
    out.clip_pos = camera.view_proj * vec4<f32>(world, 1.0);
    out.local_uv = c;
    out.kind = in.kind;
    out.level = in.level;
    return out;
}

fn get_icon_color(kind: f32) -> vec3<f32> {
    let k = i32(kind);
    if k == 0 { return vec3<f32>(0.20, 0.60, 0.20); }  // industrial_complex = green
    if k == 1 { return vec3<f32>(0.70, 0.20, 0.20); }   // arms_factory = red
    if k == 2 { return vec3<f32>(0.20, 0.35, 0.70); }   // dockyard = blue
    if k == 3 { return vec3<f32>(0.45, 0.55, 0.80); }   // air_base = light blue
    if k == 4 { return vec3<f32>(0.22, 0.50, 0.72); }   // naval_base = blue-cyan
    if k == 5 { return vec3<f32>(0.48, 0.46, 0.38); }   // bunker = stone
    if k == 6 { return vec3<f32>(0.38, 0.50, 0.44); }   // coastal_bunker = green-grey
    if k == 7 { return vec3<f32>(0.72, 0.64, 0.35); }   // anti_air = brass
    if k == 8 { return vec3<f32>(0.60, 0.60, 0.20); }   // radar = yellow-green
    if k == 9 { return vec3<f32>(0.55, 0.30, 0.60); }   // synthetic_refinery = purple
    if k == 10 { return vec3<f32>(0.40, 0.50, 0.55); }  // fuel_silo = slate
    if k == 11 { return vec3<f32>(0.80, 0.75, 0.15); }  // nuclear = yellow
    if k == 12 { return vec3<f32>(0.60, 0.40, 0.20); }  // rocket_site = brown
    if k == 13 { return vec3<f32>(0.15, 0.15, 0.15); }  // oil = black
    if k == 14 { return vec3<f32>(0.70, 0.70, 0.75); }  // aluminium = silver
    if k == 15 { return vec3<f32>(0.60, 0.20, 0.55); }  // rubber = magenta
    if k == 16 { return vec3<f32>(0.50, 0.50, 0.15); }  // tungsten = olive
    if k == 17 { return vec3<f32>(0.55, 0.55, 0.55); }  // steel = grey
    if k == 18 { return vec3<f32>(0.40, 0.55, 0.30); }  // chromium = green-grey
    return vec3<f32>(0.30, 0.30, 0.30);                  // coal + default
}

fn is_resource(kind: f32) -> bool {
    return i32(kind) >= 13;
}

fn icon_shape(uv: vec2<f32>, kind: f32) -> f32 {
    let cx = uv.x - 0.5;
    let cy = uv.y - 0.5;
    let k = i32(kind);

    // Factories (0-1): rounded square
    if k <= 1 {
        let d = max(abs(cx), abs(cy)) - 0.35;
        return 1.0 - smoothstep(-0.03, 0.03, d);
    }
    // Dockyard (2): diamond
    if k == 2 {
        let d = abs(cx) + abs(cy) - 0.35;
        return 1.0 - smoothstep(-0.03, 0.03, d);
    }
    // Air base (3): triangle (pointing up)
    if k == 3 {
        let d = cy + 0.35 - abs(cx) * 1.5;
        return 1.0 - smoothstep(-0.03, 0.03, -d);
    }
    // Naval base (4): diamond
    if k == 4 {
        let d = abs(cx) + abs(cy) - 0.32;
        return 1.0 - smoothstep(-0.03, 0.03, d);
    }
    // Bunkers / anti-air (5-7): compact square
    if k >= 5 && k <= 7 {
        let d = max(abs(cx), abs(cy)) - 0.30;
        return 1.0 - smoothstep(-0.03, 0.03, d);
    }
    // Radar (8): small circle
    if k == 8 {
        let d = length(vec2<f32>(cx, cy)) - 0.30;
        return 1.0 - smoothstep(-0.03, 0.03, d);
    }
    // Other buildings (9-12): small square
    if k <= 12 {
        let d = max(abs(cx), abs(cy)) - 0.30;
        return 1.0 - smoothstep(-0.03, 0.03, d);
    }
    // Resources (13-19): circle
    let d = length(vec2<f32>(cx, cy)) - 0.28;
    return 1.0 - smoothstep(-0.03, 0.03, d);
}

fn icon_border(uv: vec2<f32>, kind: f32) -> f32 {
    let cx = uv.x - 0.5;
    let cy = uv.y - 0.5;
    let k = i32(kind);
    var d: f32;
    if k <= 1 {
        d = max(abs(cx), abs(cy)) - 0.35;
    } else if k == 2 {
        d = abs(cx) + abs(cy) - 0.35;
    } else if k == 3 {
        d = abs(cy + 0.05) + abs(cx) * 1.25 - 0.42;
    } else if k == 4 {
        d = abs(cx) + abs(cy) - 0.32;
    } else if k >= 5 && k <= 7 {
        d = max(abs(cx), abs(cy)) - 0.30;
    } else if k == 8 {
        d = length(vec2<f32>(cx, cy)) - 0.30;
    } else if k <= 12 {
        d = max(abs(cx), abs(cy)) - 0.30;
    } else {
        d = length(vec2<f32>(cx, cy)) - 0.28;
    }
    return smoothstep(-0.05, 0.02, d) * (1.0 - smoothstep(0.02, 0.09, d));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let shape = icon_shape(in.local_uv, in.kind);
    if shape < 0.5 {
        discard;
    }

    let base_color = get_icon_color(in.kind);
    let highlight = vec3<f32>(1.0, 1.0, 1.0) * 0.15;
    var color = base_color + highlight * shape;
    let border = icon_border(in.local_uv, in.kind) * poi_params.outline_strength;
    color = mix(color, vec3<f32>(0.025, 0.022, 0.018), border);

    // Resource icons: slightly dimmer, with a thin border
    var alpha = 0.88;
    if is_resource(in.kind) {
        alpha = 0.78;
    }

    return vec4<f32>(color, alpha * poi_params.opacity);
}
