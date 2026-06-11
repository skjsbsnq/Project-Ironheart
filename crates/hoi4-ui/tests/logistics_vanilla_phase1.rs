use std::fmt::Write as _;
use std::path::PathBuf;

use hoi4_paths::PathConfig;
use hoi4_ui::logistics_panel::{
    logistics_vanilla_entry_instance_specs, logistics_vanilla_entry_kind,
    logistics_vanilla_entry_plan, CountryLogisticsProfile, LogisticsData, LogisticsEntry,
    LogisticsVanillaEntryKind, ResourceEntry,
};
use hoi4_ui::vanilla_gui::{
    generate_runtime_instances, parse_gui_str, vanilla_builtin_profile_descriptors,
    vanilla_profile_diagnostics_markdown, GuiAction, GuiActionKind, GuiBindingMap, GuiNodePath,
    GuiRect, GuiRuntimeFrame, GuiRuntimeFrameInput, GuiRuntimeInstanceSource, GuiTemplateRegistry,
    LayoutOptions, VanillaGuiProfileDiagnostics, VanillaGuiRuntimeContext,
    VanillaGuiRuntimeLoadError, VanillaPanelProfile, VanillaProfileRegistry,
    COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT, COUNTRY_LOGISTICS_DESCRIPTOR,
    COUNTRY_LOGISTICS_GUI_FILE, COUNTRY_LOGISTICS_PROFILE_ID, COUNTRY_LOGISTICS_ROOT,
    COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT, NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_ROOT,
};
use hoi4_ui::{ActiveDetailPanel, DetailSource, PanelCommand};

fn report_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/logistics_vanilla_gui")
        .join(name)
}

fn write_report(path: PathBuf, body: impl AsRef<str>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

fn entry(name: &str, deficit: f32) -> LogisticsEntry {
    LogisticsEntry {
        id: name.to_owned(),
        name: name.to_owned(),
        kind: logistics_vanilla_entry_kind(name),
        equipment_icon_sprite: hoi4_ui::logistics_panel::logistics_equipment_icon_sprite_for_id(
            name,
        )
        .map(str::to_owned),
        stockpile: 100.0,
        daily_production: 4.0,
        daily_replenishment_need: deficit + 1.0,
        daily_training_need: 0.5,
        daily_maintenance_need: 0.5,
        daily_consumption: deficit + 2.0,
        net_change: 2.0 - deficit,
        deficit,
        days_until_empty: None,
        procurement_rm: 0.0,
        production_sources: Vec::new(),
        resource_inputs: Vec::new(),
    }
}

fn logistics_data(entries: Vec<LogisticsEntry>) -> LogisticsData {
    let total_daily_production = entries.iter().map(|entry| entry.daily_production).sum();
    let total_daily_need = entries.iter().map(|entry| entry.daily_consumption).sum();
    LogisticsData {
        total_types: entries.len(),
        deficit_types: entries.iter().filter(|entry| entry.deficit > 0.0).count(),
        total_daily_production,
        total_daily_need,
        military_procurement_rm: 125_000.0,
        military_maintenance_rm: 75_000.0,
        entries,
        resources: vec![ResourceEntry {
            name: "Steel".to_owned(),
            produced: 8.0,
            consumed: 6.0,
            stored: 120.0,
        }],
    }
}

fn current_twelve_entries() -> Vec<LogisticsEntry> {
    vec![
        entry("步兵装备", 2.0),
        entry("火炮", 0.0),
        entry("反坦克炮", 0.0),
        entry("防空炮", 0.0),
        entry("支援装备", 0.0),
        entry("摩托化装备", 0.0),
        entry("机械化装备", 0.0),
        entry("装甲车辆", 0.0),
        entry("飞机", 4.0),
        entry("舰艇", 3.0),
        entry("运输船", 1.0),
        entry("火车", 0.0),
    ]
}

fn minimal_logistics_doc() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        Some(PathBuf::from(COUNTRY_LOGISTICS_GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrylogisticsview"
        position = { x = -606 y = 78 }
        show_position = { x = -6 y = 78 }
        size = { width = 550 height = 100%% }
        instantTextboxType = { name = "logistics_title" position = { x = 12 y = 8 } size = { width = 260 height = 24 } text = "" }
        buttonType = { name = "close_button" position = { x = 512 y = 8 } size = { width = 24 height = 24 } }
        gridBoxType = {
            name = "materiel_grid"
            position = { x = 12 y = 48 }
            size = { width = 500 height = 116 }
            slotsize = { width = 500 height = 58 }
        }
    }
    containerWindowType = {
        name = "logistics_overview_land_equipment_entry"
        size = { width = 500 height = 58 }
        instantTextboxType = { name = "equipment_type" position = { x = 8 y = 4 } size = { width = 160 height = 20 } text = "" }
        instantTextboxType = { name = "produced" position = { x = 190 y = 4 } size = { width = 60 height = 20 } text = "" }
        instantTextboxType = { name = "needs" position = { x = 260 y = 4 } size = { width = 60 height = 20 } text = "" }
        instantTextboxType = { name = "balance" position = { x = 330 y = 4 } size = { width = 60 height = 20 } text = "" }
        instantTextboxType = { name = "in_stock" position = { x = 400 y = 4 } size = { width = 60 height = 20 } text = "" }
    }
    containerWindowType = {
        name = "logistics_overview_naval_equipment_entry"
        size = { width = 500 height = 58 }
        instantTextboxType = { name = "equipment_type" position = { x = 8 y = 4 } size = { width = 160 height = 20 } text = "" }
    }
    containerWindowType = {
        name = "logistics_overview_air_equipment_entry"
        size = { width = 500 height = 58 }
        instantTextboxType = { name = "equipment_type" position = { x = 8 y = 4 } size = { width = 160 height = 20 } text = "" }
    }
    containerWindowType = {
        name = "logistics_overview_resource_item"
        size = { width = 100 height = 30 }
    }
}
"#,
    )
}

#[test]
fn logistics_vanilla_gui_gate4_descriptor_registered_as_builtin_profile() {
    let profile = CountryLogisticsProfile;
    let descriptor = profile.descriptor();

    assert_eq!(profile.profile_id(), COUNTRY_LOGISTICS_PROFILE_ID);
    assert_eq!(profile.root_template(), COUNTRY_LOGISTICS_ROOT);
    assert_eq!(profile.required_gui_files(), &[COUNTRY_LOGISTICS_GUI_FILE]);
    assert_eq!(descriptor, COUNTRY_LOGISTICS_DESCRIPTOR);
    for template in [
        "logistics_overview_land_equipment_entry",
        "logistics_overview_naval_equipment_entry",
        "logistics_overview_air_equipment_entry",
        "logistics_overview_resource_item",
        "logistics_entry_resource_item",
    ] {
        assert!(descriptor.key_templates.contains(&template));
    }

    let mut registry = VanillaProfileRegistry::new();
    for descriptor in vanilla_builtin_profile_descriptors() {
        registry.register_descriptor(descriptor.clone());
    }
    assert!(registry.get(COUNTRY_LOGISTICS_PROFILE_ID).is_some());

    if let Some(context) =
        VanillaGuiRuntimeContext::load_all_profiles(vanilla_builtin_profile_descriptors())
    {
        assert!(context
            .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
            .is_some());
        assert!(context
            .root_template(COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT)
            .is_some());
        assert!(context
            .root_template(NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_ROOT)
            .is_some());
    }
}

#[test]
fn logistics_vanilla_gui_gate5_runtime_load_report_or_fallback_diagnostic() {
    let mut report = String::from("# Gate 5 Logistics Runtime Load\n\n");
    match VanillaGuiRuntimeContext::load_result(COUNTRY_LOGISTICS_DESCRIPTOR.required_gui_files) {
        Ok(context) => {
            let profile = context.profile_report(&COUNTRY_LOGISTICS_DESCRIPTOR);
            let _ = writeln!(report, "- runtime_available: true");
            let _ = writeln!(
                report,
                "- hoi4_root: {}",
                context.path_cfg.game_path().display()
            );
            let _ = writeln!(report, "- root_loaded: {}", profile.root_loaded);
            let _ = writeln!(report, "- loaded_gui_files: {:?}", profile.loaded_gui_files);
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
                "- required_sprites_hit: {}/{}",
                profile.gfx_hits.hits, profile.gfx_hits.requested
            );
            let _ = writeln!(
                report,
                "- required_sprites_missing: {:?}",
                profile.gfx_hits.missing
            );
            assert!(profile.gui_loaded, "{profile:?}");
            assert!(profile.root_loaded, "{profile:?}");
            assert!(profile.key_templates_missing.is_empty(), "{profile:?}");
        }
        Err(error) => {
            let diagnostics =
                VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_LOGISTICS_DESCRIPTOR, error);
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report, "{}", diagnostics.to_markdown());
            assert!(!diagnostics.runtime_available);
            assert!(diagnostics.to_markdown().contains("country_logistics"));
        }
    }
    report.push_str("\n- note: HOI4 path is diagnostic-only and is not hard-coded into code.\n");
    write_report(report_path("gate5_runtime_load.md"), report);
}

#[test]
fn logistics_vanilla_gui_gate5_missing_install_diagnostic_does_not_panic() {
    let fake_root = std::env::temp_dir().join(format!(
        "ironheart_logistics_missing_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&fake_root);
    std::fs::create_dir_all(fake_root.join("interface")).unwrap();
    let path_cfg = PathConfig::with_game_path(&fake_root);
    let error = VanillaGuiRuntimeContext::load_with_path_config_result(
        path_cfg,
        COUNTRY_LOGISTICS_DESCRIPTOR.required_gui_files,
    )
    .expect_err("fake install must not contain countrylogisticsview.gui");
    assert!(matches!(
        error,
        VanillaGuiRuntimeLoadError::MissingGuiFile { .. }
    ));
    let diagnostics =
        VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_LOGISTICS_DESCRIPTOR, error);
    assert!(!diagnostics.runtime_available);
    assert_eq!(
        diagnostics.missing_gui_files,
        vec![COUNTRY_LOGISTICS_GUI_FILE]
    );
    assert!(diagnostics.category_names().contains(&"gui"));
    let _ = std::fs::remove_dir_all(&fake_root);
}

#[test]
fn logistics_vanilla_gui_gate6_selects_templates_for_current_twelve_categories() {
    for (name, expected) in [
        ("infantry_equipment", LogisticsVanillaEntryKind::Land),
        ("artillery", LogisticsVanillaEntryKind::Land),
        ("anti_tank", LogisticsVanillaEntryKind::Land),
        ("anti_air", LogisticsVanillaEntryKind::Land),
        ("support_equipment", LogisticsVanillaEntryKind::Land),
        ("motorized", LogisticsVanillaEntryKind::Land),
        ("mechanized", LogisticsVanillaEntryKind::Land),
        ("armor", LogisticsVanillaEntryKind::Land),
        ("aircraft", LogisticsVanillaEntryKind::Air),
        ("naval_vessel", LogisticsVanillaEntryKind::Naval),
        ("convoy", LogisticsVanillaEntryKind::Naval),
        ("train", LogisticsVanillaEntryKind::Land),
        ("飞机", LogisticsVanillaEntryKind::Air),
        ("舰艇", LogisticsVanillaEntryKind::Naval),
        ("运输船", LogisticsVanillaEntryKind::Naval),
    ] {
        assert_eq!(logistics_vanilla_entry_kind(name), expected, "{name}");
    }
}

#[test]
fn logistics_vanilla_gui_gate6_generates_stable_paths_for_zero_one_twelve_and_overflow_rows() {
    let doc = minimal_logistics_doc();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let root_layout =
        hoi4_ui::vanilla_gui::compute_layout_tree(root, &LayoutOptions::new(viewport));
    let grid = root_layout.find_by_name("materiel_grid").unwrap();
    let grid_node = root.find_node_by_name("materiel_grid").unwrap();
    let registry = GuiTemplateRegistry::from_documents([(COUNTRY_LOGISTICS_GUI_FILE, &doc)]);

    let empty = logistics_data(Vec::new());
    assert!(logistics_vanilla_entry_plan(&empty).is_empty());
    assert!(logistics_vanilla_entry_instance_specs(
        &empty,
        grid.path.clone(),
        grid_node,
        grid.rect
    )
    .is_empty());

    let one = logistics_data(vec![entry("飞机", 1.0)]);
    let one_specs =
        logistics_vanilla_entry_instance_specs(&one, grid.path.clone(), grid_node, grid.rect);
    assert_eq!(instance_count(&one_specs), 1);
    assert_eq!(
        one_specs[0].template_name,
        "logistics_overview_air_equipment_entry"
    );

    let twelve = logistics_data(current_twelve_entries());
    let plan = logistics_vanilla_entry_plan(&twelve);
    assert_eq!(plan.len(), 12);
    assert_eq!(plan[0].model_key, "飞机");
    assert_eq!(plan[1].model_key, "舰艇");
    let twelve_specs =
        logistics_vanilla_entry_instance_specs(&twelve, grid.path.clone(), grid_node, grid.rect);
    assert_eq!(instance_count(&twelve_specs), 12);
    assert!(twelve_specs
        .iter()
        .any(|spec| spec.template_name == "logistics_overview_land_equipment_entry"));
    assert!(twelve_specs
        .iter()
        .any(|spec| spec.template_name == "logistics_overview_naval_equipment_entry"));
    assert!(twelve_specs
        .iter()
        .any(|spec| spec.template_name == "logistics_overview_air_equipment_entry"));

    let instances = generate_runtime_instances(
        &registry,
        &twelve_specs,
        &root_layout,
        &GuiBindingMap::default(),
        1.0,
    );
    assert_eq!(instances.len(), 12);
    let paths: Vec<String> = instances
        .iter()
        .map(|instance| instance.path.to_string())
        .collect();
    assert!(paths.iter().all(|path| path.contains("materiel_grid")));
    assert!(paths
        .iter()
        .any(|path| path.contains("logistics_overview_air_equipment_entry[0]")));
    assert!(instances
        .iter()
        .any(|instance| instance.context.model_key.as_deref() == Some("飞机")));

    let mut overflow_entries = current_twelve_entries();
    overflow_entries.extend([
        entry("rocket_artillery", 0.0),
        entry("heavy_aircraft", 0.0),
        entry("landing_ship", 0.0),
        entry("armored_train", 0.0),
    ]);
    let overflow = logistics_data(overflow_entries);
    let overflow_specs =
        logistics_vanilla_entry_instance_specs(&overflow, grid.path.clone(), grid_node, grid.rect);
    assert_eq!(instance_count(&overflow_specs), 16);
    assert!(overflow_specs.iter().any(|spec| match &spec.source {
        GuiRuntimeInstanceSource::Absolute { rects } => rects
            .iter()
            .any(|rect| rect.y + rect.height > grid.rect.y + grid.rect.height),
        _ => false,
    }));
}

#[test]
fn logistics_vanilla_gui_gate7_profile_binds_frame_close_and_basic_text() {
    let profile = CountryLogisticsProfile;
    let data = logistics_data(vec![entry("步兵装备", 3.0)]);
    let doc = minimal_logistics_doc();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    let viewport = GuiRect::new(0.0, 0.0, 1280.0, 720.0);
    let bindings = hoi4_ui::vanilla_gui::bind_profile_tree(&profile, root, &data);
    let frame = GuiRuntimeFrame::build(GuiRuntimeFrameInput::new(
        root,
        viewport,
        profile.profile_id(),
        &bindings,
    ));

    assert!(frame.visible);
    assert_eq!(frame.profile_id, COUNTRY_LOGISTICS_PROFILE_ID);
    assert!(frame.root_layout.find_by_name("close_button").is_some());
    let title = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("logistics_title"),
        &data,
    );
    assert!(title.text.as_deref().is_some_and(|text| !text.is_empty()));
    let close = profile.handle_action(
        GuiAction {
            node_path: GuiNodePath::root("close"),
            kind: GuiActionKind::Click,
        },
        &data,
    );
    assert_eq!(close, Some(PanelCommand::ClosePrimary));

    let open_detail = profile.handle_action(
        GuiAction {
            node_path: GuiNodePath::root("logistics:entry:步兵装备"),
            kind: GuiActionKind::Click,
        },
        &data,
    );
    match open_detail {
        Some(PanelCommand::OpenDetail(ActiveDetailPanel::Goods(target))) => {
            assert_eq!(target.good_id, "步兵装备");
            assert_eq!(target.source, Some(DetailSource::Logistics));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn logistics_vanilla_gui_gate8_diagnostics_markdown_keeps_ledger_fallback_named() {
    let markdown = vanilla_profile_diagnostics_markdown(None, &COUNTRY_LOGISTICS_DESCRIPTOR);
    assert!(markdown.contains("country_logistics"));
    assert!(!markdown.contains("v9_show_logistics"));
    assert!(!markdown.contains("ledger_show_logistics"));
}

fn instance_count(specs: &[hoi4_ui::vanilla_gui::GuiRuntimeInstanceSpec]) -> usize {
    specs
        .iter()
        .map(|spec| match &spec.source {
            GuiRuntimeInstanceSource::Descriptor { count }
            | GuiRuntimeInstanceSource::Grid { count } => *count,
            GuiRuntimeInstanceSource::Absolute { rects } => rects.len(),
        })
        .sum()
}
