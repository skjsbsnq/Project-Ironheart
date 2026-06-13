//! Phase 3.12.4 — naga static validation of the new terrain.wgsl.
//!
//! Runs naga's wgsl front-end + validator against the composed shader
//! (lib + global uniform + terrain.wgsl). Catches typos, type mismatches,
//! and invalid uniform layout early — without needing a GPU.

use hoi4_render::shader_rt::compose_shader;

#[test]
fn terrain_wgsl_validates_with_lib_and_global_uniform() {
    // Public re-export from `hoi4_app::passes::terrain` is private (binary
    // crate), so we read the file via include_str here.
    let source = include_str!("../src/passes/terrain.wgsl");
    let composed = compose_shader(source, true, true);
    let module = match naga::front::wgsl::parse_str(&composed) {
        Ok(module) => module,
        Err(err) => {
            eprintln!(
                "[shader-validate] terrain.wgsl parse failed:\n{}",
                err.emit_to_string(&composed)
            );
            panic!("terrain.wgsl parse failed");
        }
    };

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    if let Err(err) = validator.validate(&module) {
        eprintln!(
            "[shader-validate] terrain.wgsl validation failed:\n{}",
            err.emit_to_string(&composed)
        );
        panic!("terrain.wgsl validation failed");
    }
}

#[test]
fn terrain_wgsl_has_phase3_runtime_debug_and_ownership_gates() {
    let source = include_str!("../src/passes/terrain.wgsl");
    for token in [
        "struct TerrainMaterial",
        "struct TerrainMaterialWeights",
        "terrain_controls: vec4<f32>",
        "feature_flags: vec4<f32>",
        "fn build_terrain_material",
        "fn apply_province_secondary_color",
        "var color = mix(political_color, terrain_albedo, weights.map_mode_weight)",
        "get_overlay(atlas_terr, cmap, COLORMAP_OVERLAY_STRENGTH_TERRAIN)",
        "calculate_occupation_mask",
        "fn terrain_debug_color",
        "fn terrain_owns_water_color",
        "fn terrain_owns_sdf_borders",
        "map_px: vec2<f32>",
        "vanilla_terrain_tile_repeat",
        "const TERRAIN_DEBUG_TERRAIN_ID",
        "const TERRAIN_DEBUG_BLEND_STATE",
        "const TERRAIN_DEBUG_CORNERS",
        "const TERRAIN_DEBUG_RIVER_MASK",
        "const TERRAIN_DEBUG_MUD_MASK",
        "const TERRAIN_DEBUG_CITY_EMIT_MASK",
        "const TERRAIN_DEBUG_CITYLIGHTS_RGB",
        "const TERRAIN_DEBUG_CITYLIGHT_CONTRIB",
        "const TERRAIN_DEBUG_MAP_PX_GRID",
        "const TERRAIN_DEBUG_CITYLIGHT_UV",
        "const TERRAIN_DEBUG_GRADIENT_BORDER_CH3",
        "const TERRAIN_DEBUG_GRADIENT_BORDER_CH1_RGB",
        "const TERRAIN_DEBUG_GRADIENT_BORDER_CH1_ALPHA",
        "const TERRAIN_DEBUG_GRADIENT_BORDER_CH2_RGB",
        "const TERRAIN_DEBUG_GRADIENT_BORDER_CH2_ALPHA",
        "const TERRAIN_DEBUG_PROVINCE_SECONDARY",
        "const TERRAIN_DEBUG_FOW_UNEXPLORED",
        "const TERRAIN_DEBUG_FOW_VISIBILITY",
        "const TERRAIN_DEBUG_FOW_ENEMY_SPOTTED",
        "const TERRAIN_DEBUG_MUD_SNOW_SNOW_AMOUNT",
        "const TERRAIN_DEBUG_MUD_SNOW_MUD_AMOUNT",
        "const TERRAIN_DEBUG_MUD_SNOW_TARGET",
        "const TERRAIN_DEBUG_POINT_LIGHT_CONTRIB",
        "const TERRAIN_DEBUG_COLORMAP",
        "const TERRAIN_DEBUG_FINAL_BEFORE_POSTPROCESS",
        "political_base",
        "colormap",
        "point_light_contribution",
        "fn get_mud_snow_color",
        "fn apply_snow",
        "fn get_mud_color",
        "fn lookup_terrain_flags",
        "calculate_point_lights",
        "light_data_tex",
        "light_index_tex",
        "fn calculate_map_tex_index",
        "fn province_secondary_at",
        "fn gradient_border_page_uv",
        "fn gradient_border_ch1_sample",
        "fn gradient_border_ch2_sample",
        "fn gradient_border_ch3_dist_px",
        "const TERRAIN_ATLAS_ALBEDO_STRENGTH: f32 = 0.78",
        "const TERRAIN_ATLAS_MAX_DARKEN_TERRAIN: f32 = 0.48",
        "const TERRAIN_FOREST_ALBEDO_STRENGTH: f32 = 0.82",
        "const TERRAIN_FOREST_POLITICAL_STRENGTH: f32 = 0.48",
        "const TERRAIN_ATLAS_NORMAL_STRENGTH: f32 = 0.44",
        "const TERRAIN_ATLAS_MIP_BIAS: f32 = -0.70",
        "const TERRAIN_PROJECTED_SHADOW_STRENGTH: f32 = 0.0",
        "const TERRAIN_WATER_RELIEF_STRENGTH: f32 = 0.0",
        "const TERRAIN_COAST_WHITE_EDGE_STRENGTH: f32 = 0.0",
        "const TERRAIN_WATER_FOAM_STRENGTH: f32 = 0.0",
        "canopy_large",
        "crown_shadow",
        "const TERRAIN_MUD_ALBEDO_STRENGTH: f32 = 0.0",
        "const TERRAIN_MUD_NORMAL_STRENGTH: f32 = 0.0",
        "province_secondary_color_tex",
        "gradient_border_ch3_tex",
        "fow_tex",
        "mud_snow_tex",
    ] {
        assert!(source.contains(token), "missing Phase 3 token: {token}");
    }
}

#[test]
fn terrain_wgsl_disables_city_and_point_light_glow_by_default() {
    let source = include_str!("../src/passes/terrain.wgsl");
    assert!(
        source.contains("const TERRAIN_CITY_LIGHTS_ENABLED: bool = false"),
        "terrain city light glow should stay disabled by default"
    );
    assert!(
        source.contains("const TERRAIN_POINT_LIGHTS_ENABLED: bool = false"),
        "terrain point light glow should stay disabled by default"
    );
}

#[test]
fn terrain_wgsl_uses_map_mode_blend_in_final_material() {
    let source = include_str!("../src/passes/terrain.wgsl");
    let material_start = source
        .find("fn build_terrain_material")
        .expect("build_terrain_material should exist");
    let debug_start = source
        .find("fn terrain_debug_color")
        .expect("terrain_debug_color should exist");
    let material_body = &source[material_start..debug_start];

    assert!(
        source.contains("weights.map_mode_weight = clamp(params.map_mode_terrain_blend, 0.0, 1.0)"),
        "map-mode terrain blend must drive final terrain/political mixing"
    );
    assert!(
        material_body.contains("mix(political_color, terrain_albedo, weights.map_mode_weight)"),
        "political/map-mode color should be part of the normal final material, not only a debug view"
    );
    assert!(
        material_body.contains("clamp_dark_terrain_detail(cmap, atlas_overlay_raw, TERRAIN_ATLAS_MAX_DARKEN_TERRAIN)")
            && material_body.contains("mix(cmap, atlas_overlay, TERRAIN_ATLAS_ALBEDO_STRENGTH)"),
        "political terrain blend should restore atlas color detail while clamping dark grid artifacts"
    );
    assert!(
        material_body.contains("apply_forest_terrain_tint(")
            && source.contains("fn terrain_category_forest")
            && source.contains("fn terrain_category_jungle"),
        "forest and jungle terrain categories should feed the political terrain material"
    );
}

#[test]
fn terrain_wgsl_final_path_gates_semantic_overlay_inputs() {
    let source = include_str!("../src/passes/terrain.wgsl");
    let material_start = source
        .find("fn build_terrain_material")
        .expect("build_terrain_material should exist");
    let debug_start = source
        .find("fn terrain_debug_color")
        .expect("terrain_debug_color should exist");
    let material_body = &source[material_start..debug_start];

    let overlay_gate = material_body
        .find("if (terrain_overlays_enabled())")
        .expect("terrain overlays should have a material gate");
    let secondary_apply = material_body
        .find("apply_province_secondary_color(color, frag.map_uv)")
        .expect("ProvinceSecondaryColorMap should remain available to terrain overlays");
    assert!(
        secondary_apply > overlay_gate,
        "ProvinceSecondaryColorMap must not feed the base terrain material before the overlay gate"
    );
    assert!(
        source.contains("secondary.a * stripe * occupation_overlay_opacity()"),
        "occupation stripe alpha must be controlled by the semantic overlay opacity"
    );
    assert!(
        !material_body.contains("apply_gradient_border_channels(color, frag.map_uv)")
            && !source.contains("if (terrain_owns_sdf_borders() && !is_water)"),
        "GradientBorderChannel1/2 must stay out of the final terrain material path"
    );
    assert!(
        !material_body.contains("gradient_border_ch3_dist_px("),
        "gradient_border_ch3 must stay out of the terrain final material path"
    );
    assert!(
        !material_body.contains("occupation_color_at("),
        "legacy occupation LUT must stay out of the default terrain material path"
    );
    assert!(
        source.contains("const GB_TEXTURE_HEIGHT_TERRAIN: f32 = 1024.0")
            && source.contains("let half_pix = 0.5 / GB_TEXTURE_HEIGHT_TERRAIN")
            && source.contains("gradient_border_page_uv(uv, 0.0)")
            && source.contains("gradient_border_page_uv(uv, 1.0)"),
        "GradientBorderChannel1/2 must be sampled through the vanilla two-page UV layout"
    );
    assert!(
        source.contains("if (view == TERRAIN_DEBUG_PROVINCE_SECONDARY)"),
        "province_secondary should remain available as an explicit debug view"
    );
    assert!(
        source.contains("if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH3)"),
        "gradient_border_ch3 should remain available as an explicit debug view"
    );
    for token in [
        "if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH1_RGB)",
        "if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH1_ALPHA)",
        "if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH2_RGB)",
        "if (view == TERRAIN_DEBUG_GRADIENT_BORDER_CH2_ALPHA)",
    ] {
        assert!(
            source.contains(token),
            "gradient_border ch1/ch2 RGB and alpha should remain available as explicit debug views"
        );
    }
}
