use hoi4_render::shader_rt::compose_shader;

#[test]
fn river_wgsl_validates_with_lib_and_global_uniform() {
    let source = include_str!("../src/passes/river.rs");
    let start = source
        .find("const RIVER_WGSL: &str = r#\"")
        .expect("river shader literal start");
    let source = &source[start + "const RIVER_WGSL: &str = r#\"".len()..];
    let end = source.find("\"#;").expect("river shader literal end");
    let wgsl = &source[..end];
    let composed = compose_shader(wgsl, true, true);
    let module = match naga::front::wgsl::parse_str(&composed) {
        Ok(module) => module,
        Err(err) => {
            eprintln!(
                "[shader-validate] river.wgsl parse failed:\n{}",
                err.emit_to_string(&composed)
            );
            panic!("river.wgsl parse failed");
        }
    };

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    if let Err(err) = validator.validate(&module) {
        eprintln!(
            "[shader-validate] river.wgsl validation failed:\n{}",
            err.emit_to_string(&composed)
        );
        panic!("river.wgsl validation failed");
    }
}

#[test]
fn river_wgsl_contains_phase7_material_and_flow_terms() {
    let source = include_str!("../src/passes/river.rs");
    for token in [
        "RiverSurface",
        "river_diffuse_0",
        "river_diffuse_1",
        "river_diffuse_2",
        "river_normal_0",
        "river_normal_1",
        "river_normal_2",
        "river_masks",
        "river_sample.gb",
        "level_alpha",
        "clip.z = clip.z - rparams.z_bias * clip.w",
        "fn apply_river_gradient_border",
        "gradient_border_page_uv(uv, 0.0)",
        "let country_gate = clamp(ch2.g, 0.0, 1.0)",
        "color = apply_river_gradient_border(color, in.map_uv)",
    ] {
        assert!(source.contains(token), "missing Phase 7 token: {token}");
    }
    assert!(
        !source.contains("let border_hint = 1.0 - smoothstep"),
        "river should use semantic GradientBorder colors instead of old grayscale hint"
    );
}

#[test]
fn river_projected_shadow_uses_screen_coordinate() {
    let source = include_str!("../src/passes/river.rs");
    assert!(source.contains("projected_shadow_uv = in.clip_pos.xy / max(frame.screen_size"));
    assert!(source.contains("textureSample(shadow_map, river_sampler, clamp(projected_shadow_uv"));
    assert!(!source.contains("textureSample(shadow_map, river_sampler, in.map_uv)"));
}
