use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hoi4_paths::PathConfig;
use hoi4_ui::icons::IconBank;
use hoi4_ui::vanilla_gui::{
    country_diplomacy_no_intel_bindings, vanilla_builtin_profile_descriptors,
    vanilla_profile_diagnostics_markdown, GfxIndex, GuiRect, GuiRuntimeFrame, GuiRuntimeFrameInput,
    GuiRuntimeState, VanillaGuiProfileDiagnostics, VanillaGuiRuntimeContext,
    COUNTRY_DIPLOMACY_DESCRIPTOR, COUNTRY_DIPLOMACY_GFX_FILE, COUNTRY_DIPLOMACY_GUI_FILE,
    COUNTRY_DIPLOMACY_PROFILE_ID, COUNTRY_DIPLOMACY_ROOT, DIPLOMACY_EXCLUDED_TEMPLATES,
    DIPLOMACY_INTEL_HIDDEN_NODES, DIPLOMACY_KEY_TEMPLATES, DIPLOMACY_REQUIRED_SPRITES,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn report_path(name: &str) -> PathBuf {
    workspace_root()
        .join("target/diplomacy_vanilla_gui")
        .join(name)
}

fn write_report(name: &str, body: impl AsRef<str>) {
    let path = report_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

#[test]
fn diplomacy_gate9_descriptor_registered_as_builtin_profile() {
    assert_eq!(COUNTRY_DIPLOMACY_PROFILE_ID, "country_diplomacy");
    assert_eq!(
        COUNTRY_DIPLOMACY_GUI_FILE,
        "interface/countrydiplomacyview.gui"
    );
    assert_eq!(
        COUNTRY_DIPLOMACY_GFX_FILE,
        "interface/countrydiplomacyview.gfx"
    );
    assert_eq!(COUNTRY_DIPLOMACY_ROOT, "countrydiplomacyview");
    assert_eq!(
        COUNTRY_DIPLOMACY_DESCRIPTOR.profile_id,
        COUNTRY_DIPLOMACY_PROFILE_ID
    );
    assert_eq!(
        COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files,
        &[COUNTRY_DIPLOMACY_GUI_FILE]
    );
    assert_eq!(
        COUNTRY_DIPLOMACY_DESCRIPTOR.root_template,
        COUNTRY_DIPLOMACY_ROOT
    );
    assert!(vanilla_builtin_profile_descriptors()
        .iter()
        .any(|profile| profile.profile_id == COUNTRY_DIPLOMACY_PROFILE_ID));

    let mut report = String::from("# Gate 9 Diplomacy Descriptor\n\n");
    let _ = writeln!(report, "- profile_id: {COUNTRY_DIPLOMACY_PROFILE_ID}");
    let _ = writeln!(report, "- gui_file: {COUNTRY_DIPLOMACY_GUI_FILE}");
    let _ = writeln!(report, "- gfx_file: {COUNTRY_DIPLOMACY_GFX_FILE}");
    let _ = writeln!(report, "- root: {COUNTRY_DIPLOMACY_ROOT}");
    let _ = writeln!(report, "- builtin_registered: true");
    let _ = writeln!(
        report,
        "- builtin_profile_count: {}",
        vanilla_builtin_profile_descriptors().len()
    );
    write_report("gate9_descriptor.md", report);
}

#[test]
fn diplomacy_gate10_profile_inventory_report() {
    for template in [
        "diplomacy_action_entry",
        "relation_strip_view",
        "subject_relation_strip_view",
        "diplomacy_country_list_country_entry",
        "diplomacy_wargoal_entry",
    ] {
        assert!(
            DIPLOMACY_KEY_TEMPLATES.contains(&template),
            "missing diplomacy key template descriptor {template}"
        );
        assert!(
            COUNTRY_DIPLOMACY_DESCRIPTOR
                .key_templates
                .contains(&template),
            "descriptor missing diplomacy key template {template}"
        );
    }
    for excluded in DIPLOMACY_EXCLUDED_TEMPLATES {
        assert!(
            !COUNTRY_DIPLOMACY_DESCRIPTOR
                .key_templates
                .contains(excluded),
            "intel/espionage template must not be a key template: {excluded}"
        );
    }
    for sprite in DIPLOMACY_REQUIRED_SPRITES {
        assert!(
            COUNTRY_DIPLOMACY_DESCRIPTOR
                .required_sprites
                .contains(sprite),
            "missing diplomacy required sprite descriptor {sprite}"
        );
    }

    let mut report = String::from("# Gate 10 Diplomacy Profile Inventory\n\n");
    let _ = writeln!(
        report,
        "- key_templates_required: {:?}",
        DIPLOMACY_KEY_TEMPLATES
    );
    let _ = writeln!(
        report,
        "- excluded_templates: {:?}",
        DIPLOMACY_EXCLUDED_TEMPLATES
    );
    let _ = writeln!(
        report,
        "- required_sprites: {:?}",
        DIPLOMACY_REQUIRED_SPRITES
    );

    match VanillaGuiRuntimeContext::load_result(COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files) {
        Ok(context) => {
            let profile = context.profile_report(&COUNTRY_DIPLOMACY_DESCRIPTOR);
            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(report, "- root_loaded: {}", profile.root_loaded);
            let _ = writeln!(
                report,
                "- key_templates_present: {:?}",
                profile.key_templates_present
            );
            let _ = writeln!(
                report,
                "- key_templates_missing: {:?}",
                profile.key_templates_missing
            );
            let _ = writeln!(
                report,
                "- required_sprites_present: {}/{}",
                profile.gfx_hits.hits, profile.gfx_hits.requested
            );
            let _ = writeln!(
                report,
                "- required_sprites_missing: {:?}",
                profile.gfx_hits.missing
            );
            assert!(profile.root_loaded, "{profile:?}");
            assert!(profile.key_templates_missing.is_empty(), "{profile:?}");
        }
        Err(error) => {
            let diagnostics =
                VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_DIPLOMACY_DESCRIPTOR, error);
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report, "{}", diagnostics.to_markdown());
            assert!(!diagnostics.runtime_available);
        }
    }
    write_report("gate10_profile_inventory.md", report);
}

#[test]
fn diplomacy_gate11_runtime_load_diagnostics_report() {
    let mut report = String::from("# Gate 11 Diplomacy Runtime Load\n\n");
    match VanillaGuiRuntimeContext::load_result(COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files) {
        Ok(context) => {
            let diagnostics =
                vanilla_profile_diagnostics_markdown(Some(&context), &COUNTRY_DIPLOMACY_DESCRIPTOR);
            let profile = context.profile_report(&COUNTRY_DIPLOMACY_DESCRIPTOR);
            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(
                report,
                "- hoi4_root: {}",
                context.path_cfg.game_path().display()
            );
            let _ = writeln!(report, "- root: {COUNTRY_DIPLOMACY_ROOT}");
            let _ = writeln!(report, "- root_loaded: {}", profile.root_loaded);
            let _ = writeln!(report, "- gui_file: {COUNTRY_DIPLOMACY_GUI_FILE}");
            let _ = writeln!(report, "- gfx_file: {COUNTRY_DIPLOMACY_GFX_FILE}");
            let _ = writeln!(report);
            let _ = writeln!(report, "{diagnostics}");
            assert!(profile.gui_loaded);
            assert!(profile.root_loaded);
        }
        Err(error) => {
            let diagnostics =
                VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_DIPLOMACY_DESCRIPTOR, error);
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report, "{}", diagnostics.to_markdown());
            assert!(diagnostics
                .to_markdown()
                .contains(COUNTRY_DIPLOMACY_PROFILE_ID));
        }
    }

    let fake_root = std::env::temp_dir().join(format!(
        "ironheart_diplomacy_gate11_missing_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&fake_root);
    std::fs::create_dir_all(fake_root.join("interface")).unwrap();
    let fake_path_cfg = PathConfig::with_game_path(&fake_root);
    let fake_result = VanillaGuiRuntimeContext::load_with_path_config_result(
        fake_path_cfg,
        COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files,
    );
    assert!(fake_result.is_err(), "fake install unexpectedly loaded");
    let _ = writeln!(
        report,
        "\n## Missing Install Simulation\n- result: readable diagnostic, no panic"
    );
    let _ = std::fs::remove_dir_all(&fake_root);
    write_report("gate11_runtime_load.md", report);
}

#[test]
fn diplomacy_gate12_iconbank_gfx_strategy_report() {
    let mut report = String::from("# Gate 12 Diplomacy IconBank / GfxIndex Strategy\n\n");
    match PathConfig::resolve(Default::default()) {
        Ok(path_cfg) => {
            let gfx = GfxIndex::from_path_config(&path_cfg);
            let ctx = egui::Context::default();
            let mut icon_bank = IconBank::new(ctx, path_cfg.clone());
            icon_bank.add_profile_search_dirs(COUNTRY_DIPLOMACY_PROFILE_ID);
            let dirs = icon_bank.search_dirs().to_vec();
            assert!(dirs.iter().any(|dir| dir == "gfx/interface"));
            assert!(dirs.iter().any(|dir| dir == "gfx/interface/diplomacy"));
            assert!(dirs.iter().any(|dir| dir == "gfx/interface/ideologies"));
            assert!(dirs.iter().any(|dir| dir == "gfx/interface/ideas"));
            assert!(dirs.iter().any(|dir| dir == "gfx/interface/goals"));
            assert!(dirs.iter().any(|dir| dir == "gfx/leaders"));
            let flag_probe = icon_bank.diagnose_sprite("GFX_flag_ITA_fascism");
            assert!(
                flag_probe.loaded,
                "diplomacy target flags should load through IconBank: {flag_probe:?}"
            );

            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(report, "- search_dirs: {:?}", dirs);
            let _ = writeln!(
                report,
                "- target_flag_probe: loaded={} failure={:?}",
                flag_probe.loaded, flag_probe.failure_reason
            );
            for sprite in DIPLOMACY_REQUIRED_SPRITES {
                if let Some(resource) = gfx.get(sprite) {
                    let source = resource
                        .source
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "<unknown>".to_owned());
                    let texture = resource.primary_texture.as_deref().unwrap_or("<none>");
                    let texture_path = resource
                        .primary_texture
                        .as_deref()
                        .and_then(|texture| path_cfg.find(texture))
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "<missing>".to_owned());
                    let _ = writeln!(
                        report,
                        "- {sprite}: source={source}; textureFile={texture}; texture_path={texture_path}"
                    );
                } else {
                    let probe = icon_bank.diagnose_sprite(sprite);
                    let _ = writeln!(
                        report,
                        "- {sprite}: missing mapping; fallback_attempts={:?}; failure={:?}",
                        probe.attempted_paths, probe.failure_reason
                    );
                }
            }
            assert!(!gfx.is_empty());
        }
        Err(error) => {
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report, "- reason: {error}");
        }
    }
    report.push_str(
        "\nNo `.dds`, `.gui`, or `.gfx` asset is copied into the repository by this strategy.\n",
    );
    write_report("gate12_iconbank_gfx_strategy.md", report);
}

#[test]
fn diplomacy_gate13_intel_nodes_hidden_and_espionage_templates_excluded() {
    for template in DIPLOMACY_EXCLUDED_TEMPLATES {
        assert!(
            !COUNTRY_DIPLOMACY_DESCRIPTOR
                .key_templates
                .contains(template),
            "espionage template must not be registered for first-pass diplomacy: {template}"
        );
    }

    let mut report = String::from("# Gate 13 Diplomacy No-Intel Policy\n\n");
    let _ = writeln!(report, "- hidden_nodes: {:?}", DIPLOMACY_INTEL_HIDDEN_NODES);
    let _ = writeln!(
        report,
        "- excluded_templates: {:?}",
        DIPLOMACY_EXCLUDED_TEMPLATES
    );

    match VanillaGuiRuntimeContext::load_result(COUNTRY_DIPLOMACY_DESCRIPTOR.required_gui_files) {
        Ok(context) => {
            let root = context
                .root_template(COUNTRY_DIPLOMACY_GUI_FILE, COUNTRY_DIPLOMACY_ROOT)
                .expect("diplomacy root template");
            let bindings = country_diplomacy_no_intel_bindings(root);
            let frame = GuiRuntimeFrame::build(
                GuiRuntimeFrameInput::new(
                    root,
                    GuiRect::new(0.0, 0.0, 1920.0, 1080.0),
                    COUNTRY_DIPLOMACY_PROFILE_ID,
                    &bindings,
                )
                .with_runtime_state(GuiRuntimeState::shown(GuiRect::new(
                    0.0, 0.0, 1920.0, 1080.0,
                )))
                .with_gfx_index(&context.gfx_index)
                .with_profile_descriptor(&COUNTRY_DIPLOMACY_DESCRIPTOR),
            );

            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(report, "- draw_commands: {}", frame.draw_list.len());
            for node_name in DIPLOMACY_INTEL_HIDDEN_NODES {
                let layout = frame
                    .root_layout
                    .find_by_name(node_name)
                    .unwrap_or_else(|| panic!("{node_name} missing from vanilla diplomacy root"));
                let has_draw = frame.draw_list.iter().any(|command| {
                    command.source_path == layout.path
                        || command.source_name.as_deref() == Some(*node_name)
                });
                let _ = writeln!(
                    report,
                    "- {node_name}: visible={} has_draw={has_draw} path={}",
                    layout.visible, layout.path
                );
                assert!(!layout.visible, "{node_name} should be hidden");
                assert!(!has_draw, "{node_name} should not produce draw commands");
            }
        }
        Err(error) => {
            let diagnostics =
                VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_DIPLOMACY_DESCRIPTOR, error);
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report, "{}", diagnostics.to_markdown());
            assert!(!diagnostics.runtime_available);
        }
    }

    write_report("gate13_no_intel.md", report);
}
