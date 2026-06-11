use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hoi4_paths::PathConfig;
use hoi4_ui::icons::IconBank;
use hoi4_ui::vanilla_gui::{
    vanilla_profile_diagnostics_markdown, GfxIndex, VanillaGuiProfileDiagnostics,
    VanillaGuiRuntimeContext, COUNTRY_LOGISTICS_DESCRIPTOR, COUNTRY_LOGISTICS_GUI_FILE,
    COUNTRY_LOGISTICS_PROFILE_ID, COUNTRY_LOGISTICS_ROOT, LOGISTICS_REQUIRED_SPRITES,
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
        .join("target/logistics_vanilla_gui")
        .join(name)
}

fn write_report(name: &str, body: impl AsRef<str>) {
    let path = report_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

#[test]
fn logistics_vanilla_gui_gate9_profile_inventory_report() {
    let required_templates = [
        "logistics_overview_land_equipment_entry",
        "logistics_overview_naval_equipment_entry",
        "logistics_overview_air_equipment_entry",
        "logistics_overview_resource_item",
    ];
    for template in required_templates {
        assert!(
            COUNTRY_LOGISTICS_DESCRIPTOR
                .key_templates
                .contains(&template),
            "missing logistics key template descriptor {template}"
        );
    }
    for sprite in LOGISTICS_REQUIRED_SPRITES {
        assert!(
            COUNTRY_LOGISTICS_DESCRIPTOR
                .required_sprites
                .contains(sprite),
            "missing logistics required sprite descriptor {sprite}"
        );
    }

    let mut report = String::from("# Gate 9 Logistics Profile Inventory\n\n");
    match VanillaGuiRuntimeContext::load_result(COUNTRY_LOGISTICS_DESCRIPTOR.required_gui_files) {
        Ok(context) => {
            let profile = context.profile_report(&COUNTRY_LOGISTICS_DESCRIPTOR);
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
        }
        Err(error) => {
            let diagnostics =
                VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_LOGISTICS_DESCRIPTOR, error);
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report, "{}", diagnostics.to_markdown());
            assert!(!diagnostics.runtime_available);
        }
    }
    write_report("gate9_profile_inventory.md", report);
}

#[test]
fn logistics_vanilla_gui_gate10_runtime_load_report() {
    let mut report = String::from("# Gate 10 Logistics Runtime Load\n\n");
    match VanillaGuiRuntimeContext::load_result(COUNTRY_LOGISTICS_DESCRIPTOR.required_gui_files) {
        Ok(context) => {
            let diagnostics =
                vanilla_profile_diagnostics_markdown(Some(&context), &COUNTRY_LOGISTICS_DESCRIPTOR);
            let profile = context.profile_report(&COUNTRY_LOGISTICS_DESCRIPTOR);
            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(
                report,
                "- hoi4_root: {}",
                context.path_cfg.game_path().display()
            );
            let _ = writeln!(report, "- root: {COUNTRY_LOGISTICS_ROOT}");
            let _ = writeln!(report, "- root_loaded: {}", profile.root_loaded);
            let _ = writeln!(report, "- gui_file: {COUNTRY_LOGISTICS_GUI_FILE}");
            let _ = writeln!(report);
            let _ = writeln!(report, "{diagnostics}");
            assert!(profile.gui_loaded);
        }
        Err(error) => {
            let diagnostics =
                VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_LOGISTICS_DESCRIPTOR, error);
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report, "{}", diagnostics.to_markdown());
            assert!(diagnostics
                .to_markdown()
                .contains(COUNTRY_LOGISTICS_PROFILE_ID));
        }
    }

    let fake_root = std::env::temp_dir().join(format!(
        "ironheart_logistics_gate10_missing_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&fake_root);
    std::fs::create_dir_all(fake_root.join("interface")).unwrap();
    let fake_path_cfg = PathConfig::with_game_path(&fake_root);
    let fake_result = VanillaGuiRuntimeContext::load_with_path_config_result(
        fake_path_cfg,
        COUNTRY_LOGISTICS_DESCRIPTOR.required_gui_files,
    );
    assert!(fake_result.is_err(), "fake install unexpectedly loaded");
    let _ = writeln!(
        report,
        "\n## Missing Install Simulation\n- result: readable diagnostic, no panic"
    );
    let _ = std::fs::remove_dir_all(&fake_root);
    write_report("gate10_runtime_load.md", report);
}

#[test]
fn logistics_vanilla_gui_gate11_iconbank_gfx_strategy_report() {
    let mut report = String::from("# Gate 11 Logistics IconBank / GfxIndex Strategy\n\n");
    match PathConfig::resolve(Default::default()) {
        Ok(path_cfg) => {
            let gfx = GfxIndex::from_path_config(&path_cfg);
            let ctx = egui::Context::default();
            let mut icon_bank = IconBank::new(ctx, path_cfg.clone());
            icon_bank.add_profile_search_dirs(COUNTRY_LOGISTICS_PROFILE_ID);
            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(report, "- search_dirs: {:?}", icon_bank.search_dirs());
            for sprite in LOGISTICS_REQUIRED_SPRITES {
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
    write_report("gate11_iconbank_gfx_strategy.md", report);
}

#[test]
fn logistics_vanilla_gui_gate12_local_asset_policy_report() {
    let root = workspace_root();
    let local_asset_dir = root.join("crates/hoi4-ui/assets/vanilla_gui");
    let mut src_hits = Vec::new();
    for entry in walk_rs(root.join("crates/hoi4-ui/src")) {
        let Ok(text) = std::fs::read_to_string(&entry) else {
            continue;
        };
        if text.contains("GFX_ih_logistics") {
            src_hits.push(entry.display().to_string());
        }
    }

    let mut report = String::from("# Gate 12 Local Vanilla-GUI Asset Policy\n\n");
    let _ = writeln!(
        report,
        "- local_asset_dir_exists: {}",
        local_asset_dir.exists()
    );
    let _ = writeln!(report, "- production_src_hits: {:?}", src_hits);
    report.push_str(
        "- policy: any files under `crates/hoi4-ui/assets/vanilla_gui/` are treated as local diagnostic leftovers only and are not part of the logistics profile loading path.\n",
    );
    report.push_str(
        "- implementation: logistics profile uses runtime `interface/countrylogisticsview.gui` and GfxIndex/IconBank from the user's HOI4 path.\n",
    );
    assert!(
        src_hits.is_empty(),
        "local pseudo logistics assets must not be production dependencies: {src_hits:?}"
    );
    write_report("gate12_local_asset_policy.md", report);
}

fn walk_rs(root: PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_rs(path));
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
        {
            out.push(path);
        }
    }
    out
}
