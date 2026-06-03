// =============================================================================
// pdxwater.wgsl - vanilla `gfx/FX/pdxwater.shader` reference translation
// =============================================================================
//
// This file is kept in the shader registry as the standalone pdxwater reference.
// The live app pass uses `crates/hoi4-app/src/passes/water.rs`, but this source
// must track the same Phase 7/8 resource semantics so shader-registry audits do
// not drift from the active pipeline.

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

struct WaterParams {
    time_speed: f32,
    fresnel_power: f32,
    foam_threshold: f32,
    ice_latitude: f32,
    world_w: f32,
    world_d: f32,
    height_scale: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> wparams: WaterParams;
@group(0) @binding(2) var heightmap_tex: texture_2d<f32>;
@group(0) @binding(3) var heightmap_sampler: sampler;

@group(1) @binding(0) var environment_cube: texture_cube<f32>;
@group(1) @binding(1) var environment_sampler: sampler;

@group(2) @binding(0) var water_normal_lean1: texture_2d<f32>;
@group(2) @binding(1) var water_normal_lean2: texture_2d<f32>;
@group(2) @binding(2) var reflection_tex: texture_2d<f32>;
@group(2) @binding(3) var fow_water_spec: texture_2d<f32>;
@group(2) @binding(4) var colormap_water: texture_2d<f32>;
@group(2) @binding(5) var colormap_water_1: texture_2d<f32>;
@group(2) @binding(6) var colormap_water_2: texture_2d<f32>;
@group(2) @binding(7) var ice_diffuse: texture_2d<f32>;
@group(2) @binding(8) var water_sampler: sampler;
@group(2) @binding(9) var ice_noise: texture_2d<f32>;
@group(2) @binding(10) var ice_noise_1: texture_2d<f32>;
@group(2) @binding(11) var reflection_land_unit: texture_2d<f32>;
@group(2) @binding(12) var underwater_terrain: texture_2d<f32>;
@group(2) @binding(13) var shadow_map: texture_2d<f32>;
@group(2) @binding(14) var gradient_border_ch1: texture_2d<f32>;
@group(2) @binding(15) var gradient_border_ch2: texture_2d<f32>;
@group(2) @binding(16) var gradient_border_ch3: texture_2d<f32>;
@group(2) @binding(17) var province_secondary_color: texture_2d<f32>;
@group(2) @binding(18) var fow_tex: texture_2d<f32>;
@group(2) @binding(19) var light_data_tex: texture_2d<f32>;
@group(2) @binding(20) var light_index_tex: texture_2d<f32>;
@group(2) @binding(21) var mud_snow_tex: texture_2d<f32>;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) map_uv: vec2<f32>,
    @location(2) map_px: vec2<f32>,
};

const SEA_LEVEL: f32 = 95.0 / 255.0;

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let world_size = vec2<f32>(wparams.world_w, wparams.world_d);
    let map_uv = world_xz_to_map_uv(in.pos.xz, world_size);

    var out: VsOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.pos, 1.0);
    out.world_pos = in.pos;
    out.map_uv = map_uv;
    out.map_px = map_uv_to_px(map_uv);
    return out;
}

fn load_height_bilinear(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(heightmap_tex));
    let coord_f = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (dim - vec2<f32>(1.0));
    let coord_i = vec2<i32>(floor(coord_f));
    let frac_xy = fract(coord_f);
    let max_x = i32(dim.x) - 1;
    let max_y = i32(dim.y) - 1;
    let x0 = clamp(coord_i.x, 0, max_x);
    let y0 = clamp(coord_i.y, 0, max_y);
    let x1 = clamp(coord_i.x + 1, 0, max_x);
    let y1 = clamp(coord_i.y + 1, 0, max_y);
    let h00 = textureLoad(heightmap_tex, vec2<i32>(x0, y0), 0).r;
    let h10 = textureLoad(heightmap_tex, vec2<i32>(x1, y0), 0).r;
    let h01 = textureLoad(heightmap_tex, vec2<i32>(x0, y1), 0).r;
    let h11 = textureLoad(heightmap_tex, vec2<i32>(x1, y1), 0).r;
    let h0 = mix(h00, h10, frac_xy.x);
    let h1 = mix(h01, h11, frac_xy.x);
    return mix(h0, h1, frac_xy.y);
}

fn blend_lean(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>((a.rg + b.rg) * 0.5, max((a.ba + b.ba) * 0.5, vec2<f32>(0.0)));
}

fn unpack_lean_normal(sample: vec4<f32>) -> vec3<f32> {
    let mean = sample.rg * 2.0 - 1.0;
    let variance = dot(sample.ba, vec2<f32>(0.5));
    let strength = clamp(0.72 - variance * 0.20, 0.35, 0.72);
    let nx = mean.x * strength;
    let nz = mean.y * strength;
    let ny = sqrt(max(1.0 - nx * nx - nz * nz, 0.0));
    return normalize(vec3<f32>(nx, ny, nz));
}

fn sample_water_normal_lean(map_px: vec2<f32>, time: f32) -> vec3<f32> {
    let water_px = map_px / 128.0;
    let uv0 = water_px * 0.04 + vec2<f32>(time * 0.012, time * 0.008);
    let uv1 = water_px * 0.08 + vec2<f32>(-time * 0.011, time * 0.013);
    let uv2 = water_px * 0.16 + vec2<f32>(time * 0.007, -time * 0.009);
    let uv3 = water_px * 0.32 + vec2<f32>(-time * 0.005, -time * 0.011);

    let lean_a = blend_lean(
        textureSample(water_normal_lean1, water_sampler, uv0),
        textureSample(water_normal_lean1, water_sampler, uv1)
    );
    let lean_b = blend_lean(
        textureSample(water_normal_lean2, water_sampler, uv2),
        textureSample(water_normal_lean2, water_sampler, uv3)
    );
    return unpack_lean_normal(blend_lean(lean_a, lean_b));
}

fn sample_water(map_uv: vec2<f32>, depth_ratio: f32, camera_dist: f32) -> vec3<f32> {
    let near_color = textureSample(colormap_water, water_sampler, map_uv).rgb;
    let mid_color = textureSample(colormap_water_1, water_sampler, map_uv).rgb;
    let far_color = textureSample(colormap_water_2, water_sampler, map_uv).rgb;
    let lod_t = smoothstep(32.0, 180.0, camera_dist);
    let map_color = mix(mix(near_color, mid_color, lod_t), far_color, lod_t * lod_t);
    let underwater = textureSample(underwater_terrain, water_sampler, map_uv * 4.0).rgb;
    return mix(map_color, underwater, clamp(depth_ratio * 0.22, 0.0, 0.22));
}

fn sample_refraction(map_uv: vec2<f32>, normal: vec3<f32>, depth_ratio: f32) -> vec3<f32> {
    let offset = normal.xz * (0.0018 + 0.0026 * (1.0 - depth_ratio));
    let refracted_uv = map_uv + offset;
    let water_lod = textureSample(colormap_water_1, water_sampler, refracted_uv).rgb;
    let under = textureSample(underwater_terrain, water_sampler, refracted_uv * 4.0).rgb;
    return mix(water_lod, under, 0.35 * depth_ratio);
}

fn apply_water_mud_snow(map_uv: vec2<f32>, base_color: vec3<f32>, depth_ratio: f32) -> vec3<f32> {
    let mud_snow = textureSample(mud_snow_tex, water_sampler, map_uv);
    let season = clamp(frame.fow_opacity_time_snow_max_speed.z, 0.0, 1.0);
    let snow = get_snow(mud_snow, season);
    let mud = mix(mud_snow.r, mud_snow.a, season);
    let snow_tint = vec3<f32>(0.58, 0.68, 0.78);
    let mud_tint = vec3<f32>(0.06, 0.10, 0.13);
    var color = mix(base_color, mud_tint, mud * 0.12 * (1.0 - depth_ratio));
    color = mix(color, snow_tint, snow * 0.20);
    return color;
}

fn apply_ice(map_uv: vec2<f32>, base_color: vec3<f32>) -> vec3<f32> {
    let lat_t = abs(map_uv.y - 0.5) * 2.0;
    if (lat_t <= wparams.ice_latitude) {
        return base_color;
    }
    let ice = textureSample(ice_diffuse, water_sampler, map_uv * 4.0).rgb;
    let noise0 = textureSample(ice_noise, water_sampler, map_uv * 8.0).r;
    let noise1 = textureSample(ice_noise_1, water_sampler, map_uv * 16.0 + vec2<f32>(0.37, 0.19)).r;
    let ice_mask = smoothstep(wparams.ice_latitude, wparams.ice_latitude + 0.06, lat_t)
                 * (0.35 + 0.45 * noise0 + 0.20 * noise1);
    return mix(base_color, ice, clamp(ice_mask, 0.0, 1.0));
}

fn calculate_point_lights_water(map_px: vec2<f32>, world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let globe_n = calc_globe_normal(map_px, frame.day_night_hour_sun_dir.x);
    let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    return calculate_point_lights(
        light_data_tex,
        light_index_tex,
        map_px,
        world_pos,
        normal,
        (0.15 + night * 0.85) * 0.40
    );
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let map_uv = in.map_uv;
    let h = load_height_bilinear(map_uv);
    if (h > SEA_LEVEL + 0.002) {
        discard;
    }

    let depth_ratio = clamp((SEA_LEVEL - h) / SEA_LEVEL, 0.0, 1.0);
    let time = frame.global_time * wparams.time_speed * 25.0;
    let camera_dist = length(frame.cam_pos - in.world_pos);
    let normal = sample_water_normal_lean(in.map_px, time);

    var base = sample_water(map_uv, depth_ratio, camera_dist);
    let secondary = textureSample(province_secondary_color, water_sampler, map_uv);
    base = mix(base, secondary.rgb, secondary.a * 0.30);

    let to_camera = normalize(frame.cam_pos - in.world_pos);
    let reflect_dir = reflect(-to_camera, normal);
    let env_raw = textureSample(environment_cube, environment_sampler, reflect_dir).rgb * frame.cubemap_intensity;
    let plane_refl = textureSample(reflection_tex, water_sampler, map_uv).rgb;
    let land_unit_refl = textureSample(reflection_land_unit, water_sampler, map_uv).rgb;
    let refraction = sample_refraction(map_uv, normal, depth_ratio);
    let reflected = mix(mix(plane_refl, env_raw, 0.35), land_unit_refl, secondary.a * 0.15);

    let fresnel_t = pow(1.0 - max(dot(normal, to_camera), 0.0), wparams.fresnel_power);
    let reflection_contribution = clamp(0.08 + fresnel_t * 0.52, 0.0, 0.70);
    var color = mix(mix(refraction, base, 0.68), reflected, reflection_contribution);

    let sun_dir = normalize(frame.day_night_hour_sun_dir.yzw);
    let half_dir = normalize(to_camera + sun_dir);
    let n_dot_h = max(dot(normal, half_dir), 0.0);
    let spec_mask = textureSample(fow_water_spec, water_sampler, map_uv).a;
    let sun_spec = pow(n_dot_h, 192.0) * spec_mask * max(frame.sun_specular_intensity, 0.4);
    color = color + vec3<f32>(1.0, 0.97, 0.85) * sun_spec * 0.20;

    let projected_shadow = textureSample(shadow_map, water_sampler, map_uv);
    let country_d_px = textureSample(gradient_border_ch1, water_sampler, map_uv).r * 255.0;
    let province_d_px = textureSample(gradient_border_ch2, water_sampler, map_uv).r * 255.0;
    let semantic_d_px = textureSample(gradient_border_ch3, water_sampler, map_uv).r * 255.0;
    let border_hint = 1.0 - smoothstep(0.0, 3.0, min(min(country_d_px, province_d_px), semantic_d_px));
    color = mix(color, vec3<f32>(0.09, 0.16, 0.21), border_hint * 0.045);
    color = mix(color, color * projected_shadow.r, 1.0 - projected_shadow.r);

    color = color + calculate_point_lights_water(in.map_px, in.world_pos, normal) * 0.12;
    color = apply_water_mud_snow(map_uv, color, depth_ratio);
    color = apply_ice(map_uv, color);

    let globe_n = calc_globe_normal(in.map_px, frame.day_night_hour_sun_dir.x);
    color = day_night(color, globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
    color = apply_distance_fog(color, in.world_pos, frame.cam_pos);
    let fow_visibility = min(textureSample(fow_tex, water_sampler, map_uv).g, max(projected_shadow.b, projected_shadow.g));
    color = mix(color * 0.52, color, fow_visibility);

    return vec4<f32>(color, 1.0);
}
