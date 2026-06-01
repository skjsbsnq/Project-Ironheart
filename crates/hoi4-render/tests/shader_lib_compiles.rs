//! Phase 3.11.1 — wgsl 静态校验。
//!
//! 用 `naga` 直接 parse + validate 三组 wgsl：
//!
//! 1. `SHADER_LIB_WGSL` — 不含入口，但作为模块应能 parse + validate
//! 2. `GLOBAL_FRAME_UNIFORM_WGSL` — 仅 struct 定义；放在一个最小入口里能编译
//! 3. 现有 7 个等价 shader entry（pdxmap / tree / pdxmesh / pdxwater /
//!    restorescene / river + flat_diffuse fallback）拼上新 lib 后仍能 parse
//!
//! ## 什么算通过
//!
//! `naga::front::wgsl::parse_str` 不返回 Err。后续 phase 加入口时再做完整
//! validate（vertex/fragment 入口需要 IO 类型签名）。本节只校验**语法 + 类型
//! resolve**，足够保证 lib 函数本身书写正确。

use hoi4_render::shader_rt::{compose_shader, ShaderRegistry};

/// 用 naga 解析 wgsl 字符串。失败时把第一段错误打印出来便于定位。
fn parse_wgsl(label: &str, source: &str) {
    match naga::front::wgsl::parse_str(source) {
        Ok(_module) => { /* 通过 */ }
        Err(err) => {
            // err.emit_to_string 给出带行号的人类可读错误
            eprintln!(
                "[shader-validate:{}] WGSL parse failed:\n{}",
                label,
                err.emit_to_string(source)
            );
            panic!("WGSL parse failed for '{}'", label);
        }
    }
}

#[test]
fn shader_lib_wgsl_parses() {
    // 库本身应能独立 parse（即使没有 @vertex/@fragment 入口）
    parse_wgsl("shader_lib", hoi4_render::SHADER_LIB_WGSL);
}

#[test]
fn global_frame_uniform_wgsl_parses_in_minimal_shell() {
    // GlobalFrameUniform struct 必须能被 wgsl 接受；构造一个最小入口让它有用。
    let shell = format!(
        "{}\n\
        @group(0) @binding(0) var<uniform> frame: GlobalFrameUniform;\n\
        @vertex fn vs() -> @builtin(position) vec4<f32> {{\n\
            return frame.view_proj * vec4<f32>(0.0, 0.0, 0.0, 1.0);\n\
        }}\n",
        hoi4_render::global_uniform::GLOBAL_FRAME_UNIFORM_WGSL
    );
    parse_wgsl("global_uniform_shell", &shell);
}

#[test]
fn shader_lib_plus_minimal_entry_parses() {
    // 拼 lib + 最小 fragment 入口，确认 lib 函数被外部 entry 引用时不冲突
    let user = r#"
@vertex
fn vs(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

@fragment
fn fs() -> @location(0) vec4<f32> {
    let g = to_gamma_scalar(0.5);
    let h = hue(0.3);
    let f = fmod_loop(7.0, 3.0);
    return vec4<f32>(h * g * f, 1.0);
}
"#;
    let composed = compose_shader(user, true, false);
    parse_wgsl("lib+entry", &composed);
}

#[test]
fn fallback_flat_diffuse_parses() {
    // ShaderRegistry::fallback() 拿到的是内置 FLAT_DIFFUSE_WGSL，应能 parse
    let reg = ShaderRegistry::new();
    parse_wgsl("flat_diffuse_fallback", reg.fallback().source);
}

#[test]
fn registered_main_shaders_still_parse() {
    // Phase 3.11.3+ 注册表指向新翻译的 wgsl，需要走 compose_shader 注入 lib + uniform
    // 才能 parse。直接 parse_wgsl 对新源会缺定义。
    let reg = ShaderRegistry::new();
    for name in &[
        "pdxmap", "tree", "pdxmesh", "pdxwater", "river", "border", "mapname",
    ] {
        let entry = reg.resolve(name);
        let composed = compose_shader(entry.source, true, true);
        parse_wgsl(&format!("entry/{}", name), &composed);
    }
    // standardfuncsgfx 仍是 fallback 占位（不需要 lib）
    let stub = reg.resolve("standardfuncsgfx");
    parse_wgsl("entry/standardfuncsgfx", stub.source);
}

#[test]
fn compose_shader_with_lib_compiles_for_user_using_lib_funcs() {
    // 模拟未来 3.11.x 的工作流：用户 wgsl 调用 lib 的 day_night / globe_normal /
    // apply_distance_fog 等函数。必须能 parse。
    let user = r#"
struct Uniforms {
    cam_pos: vec3<f32>,
    pad0: f32,
    sun_dir: vec3<f32>,
    pad1: f32,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs() -> @builtin(position) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    let world_pos = vec3<f32>(100.0, 5.0, 200.0);
    let map_px = vec2<f32>(2816.0, 1024.0);
    let globe_n = calc_globe_normal(map_px, 0.5);
    let dn = day_night_factor(globe_n, u.sun_dir, 1.0);
    let fogged = apply_distance_fog(vec3<f32>(0.5, 0.6, 0.4), world_pos, u.cam_pos);
    let lit = mix(fogged, vec3<f32>(0.05, 0.05, 0.15), dn);
    return vec4<f32>(lit, 1.0);
}
"#;
    let composed = compose_shader(user, true, false);
    parse_wgsl("user_calls_lib", &composed);
}

#[test]
fn inline_directive_replacement_compiles() {
    // 验证 `//#include "shader_lib.wgsl"` 行会被替换为完整 lib 文本
    // 并且替换后 wgsl 能正常 parse。
    let user = r#"
//#include "shader_lib.wgsl"

@vertex
fn vs() -> @builtin(position) vec4<f32> {
    let s = to_gamma_scalar(0.7);
    return vec4<f32>(s, s, s, 1.0);
}
"#;
    let composed = compose_shader(user, true, false);
    parse_wgsl("inline_directive", &composed);
}

#[test]
fn lib_text_includes_expected_helpers() {
    let lib = hoi4_render::SHADER_LIB_WGSL;
    // 对必须存在的函数做粗 grep，作为"我们没误删某个 helper"的回归
    for needle in &[
        "fn to_gamma",
        "fn to_linear",
        "fn rotate_vec_by_vec",
        "fn rotate_vec_2d",
        "fn hue",
        "fn hsv_to_rgb",
        "fn rgb_to_hsv",
        "fn get_overlay",
        "fn levels1",
        "fn cam_distance_y",
        "fn calculate_distance_fog_factor",
        "fn apply_distance_fog",
        "fn world_xz_to_map_uv",
        "fn world_xz_to_map_px",
        "fn map_uv_to_px",
        "fn vanilla_terrain_tile_repeat",
        "fn vanilla_citylight_uv",
        "fn calc_globe_normal",
        "fn day_night_factor",
        "fn nightify_color",
        "fn day_night",
        "fn fresnel_schlick",
        "fn improved_blinn_phong",
        "fn unpack_normal",
        "fn fmod_loop",
        "fn calculate_border_stripes",
        "fn calculate_occupation_mask",
    ] {
        assert!(
            lib.contains(needle),
            "shader_lib.wgsl missing expected helper: {}",
            needle
        );
    }
}

#[test]
fn shader_lib_uses_project_scaled_vanilla_fog() {
    let lib = hoi4_render::SHADER_LIB_WGSL;
    assert!(lib.contains("const FOG_COLOR: vec3<f32> = vec3<f32>(0.12, 0.28, 0.60);"));
    assert!(lib.contains("const FOG_BEGIN: f32 = WORLD_EXTENT * 2.2;"));
    assert!(lib.contains("const FOG_END: f32 = WORLD_EXTENT * 8.0;"));
    assert!(lib.contains("const FOG_MAX: f32 = 0.12;"));
}

// =============================================================================
// Phase 3.11.3 ~ 3.11.14 — 每个翻译过的 shader 单独做 naga parse 校验
// =============================================================================

/// 拼好 (lib + uniform + shader) 后用 naga 解析。
fn parse_translated(label: &str, source: &str) {
    let composed = compose_shader(source, true, true);
    parse_wgsl(label, &composed);
}

#[test]
fn pdxmap_translation_parses() {
    parse_translated("pdxmap", hoi4_render::SHADER_PDXMAP_WGSL);
}

#[test]
fn pdxmesh_translation_parses() {
    parse_translated("pdxmesh", hoi4_render::SHADER_PDXMESH_WGSL);
}

#[test]
fn pdxwater_translation_parses() {
    parse_translated("pdxwater", hoi4_render::SHADER_PDXWATER_WGSL);
}

#[test]
fn river_translation_parses() {
    parse_translated("river", hoi4_render::SHADER_RIVER_WGSL);
}

#[test]
fn tree_full_translation_parses() {
    parse_translated("tree_full", hoi4_render::SHADER_TREE_FULL_WGSL);
}

#[test]
fn border_translation_parses() {
    parse_translated("border", hoi4_render::SHADER_BORDER_WGSL);
}

#[test]
fn mapname_vanilla_translation_parses() {
    parse_translated("mapname_vanilla", hoi4_render::SHADER_MAPNAME_VANILLA_WGSL);
}

#[test]
fn particle_translation_parses() {
    parse_translated("particle", hoi4_render::SHADER_PARTICLE_WGSL);
}

#[test]
fn sky_translation_parses() {
    parse_translated("sky", hoi4_render::SHADER_SKY_WGSL);
}

#[test]
fn maparrow_translation_parses() {
    parse_translated("maparrow", hoi4_render::SHADER_MAPARROW_WGSL);
}

#[test]
fn traderoute_translation_parses() {
    parse_translated("traderoute", hoi4_render::SHADER_TRADEROUTE_WGSL);
}

#[test]
fn arrow_translation_parses() {
    parse_translated("arrow", hoi4_render::SHADER_ARROW_WGSL);
}

#[test]
fn strait_translation_parses() {
    parse_translated("strait", hoi4_render::SHADER_STRAIT_WGSL);
}

#[test]
fn mapsymbol_translation_parses() {
    parse_translated("mapsymbol", hoi4_render::SHADER_MAPSYMBOL_WGSL);
}

#[test]
fn shadow_translation_parses() {
    // shadow.wgsl 用 GlobalFrameUniform 但不需要 shader_lib
    let composed = compose_shader(hoi4_render::SHADER_SHADOW_WGSL, false, true);
    parse_wgsl("shadow", &composed);
}

#[test]
fn shadowblur_translation_parses() {
    // 后处理 pass 不需要 shader_lib 也不需要 GlobalFrameUniform
    parse_wgsl("shadowblur", hoi4_render::SHADER_SHADOWBLUR_WGSL);
}

#[test]
fn downsample_translation_parses() {
    parse_wgsl("downsample", hoi4_render::SHADER_DOWNSAMPLE_WGSL);
}

#[test]
fn downsample_luminance_translation_parses() {
    let composed = compose_shader(hoi4_render::SHADER_DOWNSAMPLE_LUMINANCE_WGSL, true, false);
    parse_wgsl("downsample_luminance", &composed);
}

#[test]
fn bloom_translation_parses() {
    let composed = compose_shader(hoi4_render::SHADER_BLOOM_WGSL, true, false);
    parse_wgsl("bloom", &composed);
}

#[test]
fn lut_blender_translation_parses() {
    let composed = compose_shader(hoi4_render::SHADER_LUT_BLENDER_WGSL, true, false);
    parse_wgsl("lut_blender", &composed);
}

#[test]
fn restorescene_translation_parses() {
    let composed = compose_shader(hoi4_render::SHADER_RESTORESCENE_WGSL, true, false);
    assert!(hoi4_render::SHADER_RESTORESCENE_WGSL.contains("hdr_tex"));
    assert!(hoi4_render::SHADER_RESTORESCENE_WGSL.contains("bloom_tex"));
    assert!(hoi4_render::SHADER_RESTORESCENE_WGSL.contains("lum_tex"));
    assert!(hoi4_render::SHADER_RESTORESCENE_WGSL.contains("color_cube_tex"));
    assert!(hoi4_render::SHADER_RESTORESCENE_WGSL.contains("srgb_target"));
    parse_wgsl("restorescene", &composed);
}

#[test]
fn saturation_slider_translation_parses() {
    let composed = compose_shader(hoi4_render::SHADER_SATURATION_SLIDER_WGSL, true, false);
    parse_wgsl("saturation_slider", &composed);
}

#[test]
fn gui_button_translation_parses() {
    let composed = compose_shader(hoi4_render::SHADER_GUI_BUTTON_WGSL, true, false);
    parse_wgsl("gui_button", &composed);
}

#[test]
fn gui_progress_translation_parses() {
    parse_wgsl("gui_progress", hoi4_render::SHADER_GUI_PROGRESS_WGSL);
}

#[test]
fn gui_special_translation_parses() {
    parse_wgsl("gui_special", hoi4_render::SHADER_GUI_SPECIAL_WGSL);
}
