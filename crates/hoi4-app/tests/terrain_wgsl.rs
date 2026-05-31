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
        "const TERRAIN_DEBUG_PROVINCE_SECONDARY",
        "const TERRAIN_DEBUG_FOW_UNEXPLORED",
        "const TERRAIN_DEBUG_FOW_VISIBILITY",
        "const TERRAIN_DEBUG_FOW_ENEMY_SPOTTED",
        "const TERRAIN_DEBUG_MUD_SNOW_SNOW_AMOUNT",
        "const TERRAIN_DEBUG_MUD_SNOW_MUD_AMOUNT",
        "const TERRAIN_DEBUG_MUD_SNOW_TARGET",
        "fn get_mud_snow_color",
        "fn apply_snow",
        "fn get_mud_color",
        "fn lookup_terrain_flags",
        "fn calculate_map_tex_index",
        "province_secondary_color_tex",
        "gradient_border_ch3_tex",
        "fow_tex",
        "mud_snow_tex",
    ] {
        assert!(source.contains(token), "missing Phase 3 token: {token}");
    }
}
