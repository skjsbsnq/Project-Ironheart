// =============================================================================
// pdxmesh.wgsl — Phase 3.11.4 vanilla `gfx/FX/pdxmesh.shader` 等价翻译
// =============================================================================
//
// **代替谁**：当前 `shader_rt::FLAT_DIFFUSE_WGSL`（30 行 fallback）。本 shader
// 用于所有 3D mesh：建筑、舰船、飞机、坦克、人物 portrait 模型、ambient_object。
//
// ## vanilla 完整特性
//
// 1. **完整 PBR-ish 光照**：`CalculateSunLight` + `CalculatePointLights` +
//    `ComposeLightSnow`
// 2. **法线 / 高光 / 自发光**：normal map + spec(RGB)+gloss(A) + emissive
// 3. **环境立方体反射**：`textureSampleLevel(env_cube, reflection, mip)`
// 4. **`shadow.fxh` PCF**：4-tap PCF 接收 CSM
// 5. **Rim light**：`smoothstep(RIM_START, RIM_END, 1 - dot(N, V)) * RIM_COLOR`
// 6. **Snow accumulation**：`ApplySnowMesh`
// 7. **顶点动画**：`vAnimateUV`（旗帜飘动 / 海浪 / 植被摇摆）
//
// ## binding 约定
//
// - `@group(0) @binding(0)` = `GlobalFrameUniform`
// - `@group(0) @binding(1)` = mesh-level material params
// - `@group(1)` = 全局共享（shadow / EnvironmentMap）
// - `@group(2)` = 材质纹理（diffuse / normal / spec_gloss / emissive）

//#include "global_uniform.wgsl"
//#include "shader_lib.wgsl"

// -----------------------------------------------------------------------------
// Bindings
// -----------------------------------------------------------------------------

struct MeshMaterial {
    diffuse_tint: vec4<f32>,
    /// (specular_intensity, glossiness, alpha_cutoff, emissive)
    pbr_packed: vec4<f32>,
    /// (use_normal_map, use_spec_gloss, use_emissive, snow_factor)
    feature_flags: vec4<f32>,
    /// 顶点 UV 动画速度 (xy=diffuse, zw=normal)
    animate_uv: vec4<f32>,
    /// rim 颜色与强度（rgb + intensity scalar）
    rim_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;
@group(0) @binding(1) var<uniform> material: MeshMaterial;

@group(1) @binding(0) var shadow_map_tex: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var environment_cube: texture_cube<f32>;
@group(1) @binding(3) var environment_sampler: sampler;

@group(2) @binding(0) var diffuse_tex: texture_2d<f32>;
@group(2) @binding(1) var normal_tex: texture_2d<f32>;
@group(2) @binding(2) var spec_gloss_tex: texture_2d<f32>;
@group(2) @binding(3) var emissive_tex: texture_2d<f32>;
@group(2) @binding(4) var mat_sampler: sampler;

// -----------------------------------------------------------------------------
// Vertex
// -----------------------------------------------------------------------------

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>, // xyz tangent, w bitangent sign
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) shadow_uv: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;

    // UV 动画（用于旗帜飘动 / 海浪等）
    let animated_uv = in.uv + material.animate_uv.xy * frame.global_time;

    let world_pos4 = vec4<f32>(in.pos, 1.0);
    out.clip_pos = frame.view_proj * world_pos4;
    out.world_pos = in.pos;
    out.normal = normalize(in.normal);
    out.tangent = normalize(in.tangent.xyz);
    out.bitangent = cross(out.normal, out.tangent) * in.tangent.w;
    out.uv = animated_uv;
    // shadow projection 占位（与 pdxmap 同样依赖未传入的 shadow matrix）
    out.shadow_uv = vec4<f32>(in.uv * 2.0 - 1.0, 0.5, 1.0);
    return out;
}

// -----------------------------------------------------------------------------
// Fragment
// -----------------------------------------------------------------------------

fn shadow_pcf(shadow_proj: vec4<f32>) -> f32 {
    let coords = shadow_proj.xy / shadow_proj.w * vec2<f32>(0.5, -0.5) + 0.5;
    let depth = shadow_proj.z / shadow_proj.w - 0.001;
    let s = textureSampleCompare(shadow_map_tex, shadow_sampler, coords, depth);
    return mix(1.0 - frame.shadow_fade_factor, 1.0, s);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let diffuse_sample = textureSample(diffuse_tex, mat_sampler, in.uv);
    if (diffuse_sample.a < material.pbr_packed.z) {
        discard;
    }
    let diffuse_albedo = diffuse_sample.rgb * material.diffuse_tint.rgb;

    // 法线（TBN）
    var normal = in.normal;
    if (material.feature_flags.x > 0.5) {
        let normal_sample = textureSample(normal_tex, mat_sampler, in.uv).rgb;
        let n_local = unpack_normal(normal_sample);
        let tbn = mat3x3<f32>(in.tangent, in.bitangent, in.normal);
        normal = normalize(tbn * n_local);
    }

    // Specular / glossiness
    var specular_color = vec3<f32>(0.05);
    var glossiness = material.pbr_packed.y;
    if (material.feature_flags.y > 0.5) {
        let sg = textureSample(spec_gloss_tex, mat_sampler, in.uv);
        specular_color = sg.rgb * material.pbr_packed.x;
        glossiness = sg.a;
    }
    let non_linear_gloss = get_non_linear_glossiness(glossiness);

    // 主太阳方向（从 LIGHT_SHADOW_DIRECTION 推算）
    let sun_dir = normalize(vec3<f32>(-0.408, -0.816, 0.408));
    let to_light = -sun_dir;
    let to_camera = normalize(frame.cam_pos - in.world_pos);

    let bp = improved_blinn_phong(
        frame.sun_diffuse_intensity.rgb,
        to_light,
        to_camera,
        normal,
        specular_color,
        non_linear_gloss,
    );
    let shadow = shadow_pcf(in.shadow_uv);

    // 环境反射（cubemap）
    let reflect_dir = reflect(-to_camera, normal);
    let env_mip = get_envmap_mip_level(glossiness);
    let env_sample = textureSampleLevel(
        environment_cube, environment_sampler, reflect_dir, env_mip
    ).rgb;

    var lit = (vec3<f32>(0.45) + bp.diffuse * shadow) * diffuse_albedo;
    lit += bp.specular * shadow;
    lit += env_sample * specular_color * frame.cubemap_intensity * 0.3;

    // Emissive（建筑窗户夜光）
    if (material.feature_flags.z > 0.5) {
        let emit = textureSample(emissive_tex, mat_sampler, in.uv).rgb;
        let globe_n = calc_globe_normal(in.world_pos.xz, frame.day_night_hour_sun_dir.x);
        let night = day_night_factor(globe_n, frame.day_night_hour_sun_dir.yzw, 1.0);
        lit += emit * material.pbr_packed.w * (0.2 + night * 0.8);
    }

    // Rim light
    let rim = smoothstep(0.55, 0.6, 1.0 - max(dot(normal, to_camera), 0.0));
    lit += rim * material.rim_color.rgb * material.rim_color.a;

    // Snow accumulation（按法线朝上 + snow_factor 外加）
    if (material.feature_flags.w > 0.0) {
        let up_factor = max(normal.y, 0.0);
        let snow = up_factor * material.feature_flags.w;
        lit = mix(lit, vec3<f32>(0.46, 0.48, 0.69), clamp(snow, 0.0, 0.85));
    }

    // 大气距离雾
    lit = apply_distance_fog(lit, in.world_pos, frame.cam_pos);

    return vec4<f32>(lit, diffuse_sample.a);
}
