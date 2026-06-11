use std::fmt::Write as _;
use std::path::PathBuf;

use egui_kittest::Harness;
use hoi4_ui::egui::Vec2;
use hoi4_ui::logistics_panel::{
    logistics_vanilla_entry_kind, logistics_vanilla_entry_resource_instance_specs,
    logistics_vanilla_resource_strip_instance_specs, logistics_vanilla_runtime_frame,
    logistics_vanilla_summary_text, CountryLogisticsProfile, LogisticsData, LogisticsEntry,
    LogisticsPanel, ResourceEntry, ResourceInputEntry, LOGISTICS_VANILLA_SNAPSHOT_1080P,
};
use hoi4_ui::theme::apply_vanilla_theme;
use hoi4_ui::vanilla_gui::{
    parse_gui_str, AnimationPhase, AnimationSpec, GuiAction, GuiActionKind, GuiDrawCommandKind,
    GuiNodePath, GuiPoint, GuiRect, GuiRuntimeFrame, GuiRuntimeFrameInput, GuiRuntimeRootPosition,
    GuiRuntimeState, GuiTemplateRegistry, VanillaPanelProfile, COUNTRY_LOGISTICS_PROFILE_ID,
    COUNTRY_LOGISTICS_ROOT,
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

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_LOGISTICS_PHASE3_SNAPSHOT").is_some()
        || std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    if enabled {
        harness.snapshot(name);
    }
}

fn entry(
    name: &str,
    stockpile: f32,
    daily_production: f32,
    daily_consumption: f32,
    production_sources: Vec<&str>,
    resource_inputs: Vec<&str>,
) -> LogisticsEntry {
    let deficit = (daily_consumption - daily_production).max(0.0);
    LogisticsEntry {
        id: name.to_owned(),
        name: name.to_owned(),
        kind: logistics_vanilla_entry_kind(name),
        equipment_icon_sprite: hoi4_ui::logistics_panel::logistics_equipment_icon_sprite_for_id(
            name,
        )
        .map(str::to_owned),
        stockpile,
        daily_production,
        daily_replenishment_need: daily_consumption * 0.5,
        daily_training_need: daily_consumption * 0.25,
        daily_maintenance_need: daily_consumption * 0.25,
        daily_consumption,
        net_change: daily_production - daily_consumption,
        deficit,
        days_until_empty: None,
        procurement_rm: if deficit > 0.0 { 42_000.0 } else { 0.0 },
        production_sources: production_sources.into_iter().map(str::to_owned).collect(),
        resource_inputs: resource_inputs
            .into_iter()
            .map(|id| ResourceInputEntry {
                id: id.to_owned(),
                name: id.to_owned(),
                amount: 1.0,
            })
            .collect(),
    }
}

fn data(entries: Vec<LogisticsEntry>) -> LogisticsData {
    LogisticsData {
        total_types: entries.len(),
        deficit_types: entries.iter().filter(|entry| entry.deficit > 0.0).count(),
        total_daily_production: entries.iter().map(|entry| entry.daily_production).sum(),
        total_daily_need: entries.iter().map(|entry| entry.daily_consumption).sum(),
        military_procurement_rm: 125_000.0,
        military_maintenance_rm: 75_000.0,
        entries,
        resources: vec![
            ResourceEntry {
                name: "oil".to_owned(),
                produced: 3.0,
                consumed: 1.0,
                stored: 7.0,
            },
            ResourceEntry {
                name: "steel".to_owned(),
                produced: 10.0,
                consumed: 14.0,
                stored: 3.0,
            },
        ],
    }
}

fn overflow_data() -> LogisticsData {
    data(
        (0..18)
            .map(|index| {
                let name = match index {
                    0 => "infantry_equipment",
                    1 => "artillery",
                    2 => "aircraft",
                    3 => "convoy",
                    _ => {
                        return entry(
                            &format!("equipment_{index:02}"),
                            10.0,
                            2.0,
                            1.0,
                            Vec::new(),
                            Vec::new(),
                        )
                    }
                };
                entry(
                    name,
                    10.0,
                    if index % 2 == 0 { 1.0 } else { 5.0 },
                    if index % 2 == 0 { 6.0 } else { 1.0 },
                    Vec::new(),
                    vec!["steel"],
                )
            })
            .collect(),
    )
}

fn minimal_phase3_doc() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        None,
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrylogisticsview"
        position = { x = -606 y = 78 }
        show_position = { x = -6 y = 78 }
        show_animation_type = decelerated
        hide_animation_type = accelerated
        animation_time = 300
        size = { width = 550 height = 100%% }

        instantTextboxType = { name = "logistics_title" position = { x = 14 y = 8 } size = { width = 260 height = 24 } text = "" }
        buttonType = { name = "close_button" position = { x = -43 y = 10 } size = { width = 32 height = 32 } Orientation = "UPPER_RIGHT" shortcut = "ESCAPE" }

        containerWindowType = {
            name = "resource_strip_container"
            position = { x = 16 y = 46 }
            size = { width = 500 height = 31 }
            gridBoxType = {
                name = "resources_grid"
                position = { x = 0 y = 0 }
                size = { width = 490 height = 31 }
                slotsize = { width = 70 height = 31 }
                max_slots = { x = 7 y = 1 }
            }
        }

        containerWindowType = {
            name = "materiel"
            position = { x = 0 y = 82 }
            size = { width = 550 height = -98 }
            margin = { top = 33 bottom = 123 }
            verticalScrollbar = "right_vertical_slider"
            scroll_wheel_factor = 40
            smooth_scrolling = yes
            gridBoxType = {
                name = "materiel_grid"
                position = { x = 13 y = 0 }
                size = { width = 565 height = 100%% }
                slotsize = { width = 100% height = 58 }
                max_slots_horizontal = 1
            }
        }

        containerWindowType = {
            name = "headers"
            position = { x = 18 y = 88 }
            size = { width = -20 height = 25 }
            iconType = { name = "efficiency_icon" position = { x = 0 y = 0 } spriteType = "GFX_efficiency_icon" }
            instantTextboxType = { name = "equipment_type_label" position = { x = 30 y = 0 } size = { width = 125 height = 20 } text = "LOGISTICS_EQUIPMENT_TYPE_LABEL" }
            iconType = { name = "producing_icon" position = { x = 156 y = 0 } spriteType = "GFX_producing_icon" }
            instantTextboxType = { name = "status_label" position = { x = 183 y = 0 } size = { width = 70 height = 20 } text = "LOGISTICS_STATUS_LABEL" }
            iconType = { name = "need_icon" position = { x = 244 y = 0 } spriteType = "GFX_need_icon" }
            iconType = { name = "balance_icon" position = { x = 325 y = 0 } spriteType = "GFX_balance_icon" }
            iconType = { name = "in_stock_icon" position = { x = 405 y = 0 } spriteType = "GFX_in_stock_icon" }
            instantTextboxType = { name = "resources_label" position = { x = 465 y = 0 } size = { width = 50 height = 20 } text = "LOGISTICS_RESOURCES_LABEL" }
        }

        containerWindowType = {
            name = "fuel_info"
            position = { x = 0 y = -210 }
            size = { width = 546 height = 103 }
            verticalScrollbar = "right_vertical_slider"
            Orientation = LOWER_LEFT
            clipping = yes
            instantTextboxType = { name = "label" position = { x = 60 y = 7 } size = { width = 220 height = 24 } text = "Fuel" }
        }

        containerWindowType = {
            name = "production_win_bottom"
            position = { x = 1 y = -98 }
            size = { width = 546 height = 98 }
            verticalScrollbar = "right_vertical_slider"
            Orientation = LOWER_LEFT
            containerWindowType = { name = "military_factories" position = { x = 20 y = 4 } size = { width = 130 height = 70 } }
            containerWindowType = { name = "naval_factories" position = { x = 170 y = 4 } size = { width = 130 height = 70 } }
            containerWindowType = { name = "nuke" position = { x = 320 y = 4 } size = { width = 90 height = 70 } instantTextboxType = { name = "nuke_count" text = "0" } }
        }
    }

    containerWindowType = {
        name = "logistics_overview_land_equipment_entry"
        size = { width = 565 height = 58 }
        background = { name = "Background" position = { x = 0 y = 0 } size = { width = 545 height = 58 } spriteType = "GFX_logistics_equipment_entry_bg" }
        buttonType = { name = "row_hit" position = { x = 0 y = 0 } size = { width = 545 height = 58 } }
        iconType = { name = "equipment_icon" position = { x = 4 y = 4 } size = { width = 40 height = 40 } spriteType = "GFX_logistics_equipment_entry_bg" }
        instantTextboxType = { name = "equipment_type" position = { x = 48 y = 6 } size = { width = 132 height = 20 } text = "" }
        instantTextboxType = { name = "produced" position = { x = 190 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "needs" position = { x = 260 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "balance" position = { x = 330 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "in_stock" position = { x = 400 y = 6 } size = { width = 50 height = 20 } text = "" }
        progressbarType = { name = "status_progressbar" position = { x = 48 y = 34 } size = { width = 320 height = 10 } }
        gridBoxType = { name = "resources_grid" position = { x = 460 y = 4 } size = { width = 75 height = 50 } slotsize = { width = 25 height = 25 } max_slots = { x = 3 y = 2 } }
    }

    containerWindowType = {
        name = "logistics_overview_naval_equipment_entry"
        size = { width = 565 height = 58 }
        background = { name = "Background" position = { x = 0 y = 0 } size = { width = 545 height = 58 } spriteType = "GFX_logistics_naval_equipment_entry_bg" }
        buttonType = { name = "row_hit" position = { x = 0 y = 0 } size = { width = 545 height = 58 } }
        instantTextboxType = { name = "equipment_type" position = { x = 48 y = 6 } size = { width = 132 height = 20 } text = "" }
        instantTextboxType = { name = "produced" position = { x = 190 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "needs" position = { x = 260 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "balance" position = { x = 330 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "in_stock" position = { x = 400 y = 6 } size = { width = 50 height = 20 } text = "" }
        progressbarType = { name = "status_progressbar" position = { x = 48 y = 34 } size = { width = 320 height = 10 } }
        gridBoxType = { name = "resources_grid" position = { x = 460 y = 4 } size = { width = 75 height = 50 } slotsize = { width = 25 height = 25 } max_slots = { x = 3 y = 2 } }
    }

    containerWindowType = {
        name = "logistics_overview_air_equipment_entry"
        size = { width = 565 height = 58 }
        background = { name = "Background" position = { x = 0 y = 0 } size = { width = 545 height = 58 } spriteType = "GFX_logistics_air_equipment_entry_bg" }
        buttonType = { name = "row_hit" position = { x = 0 y = 0 } size = { width = 545 height = 58 } }
        instantTextboxType = { name = "equipment_type" position = { x = 48 y = 6 } size = { width = 132 height = 20 } text = "" }
        instantTextboxType = { name = "produced" position = { x = 190 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "needs" position = { x = 260 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "balance" position = { x = 330 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "in_stock" position = { x = 400 y = 6 } size = { width = 50 height = 20 } text = "" }
        progressbarType = { name = "status_progressbar" position = { x = 48 y = 34 } size = { width = 320 height = 10 } }
        gridBoxType = { name = "resources_grid" position = { x = 460 y = 4 } size = { width = 75 height = 50 } slotsize = { width = 25 height = 25 } max_slots = { x = 3 y = 2 } }
    }

    containerWindowType = {
        name = "logistics_overview_resource_item"
        size = { width = 70 height = 30 }
        buttonType = { name = "button" size = { width = 70 height = 30 } }
        iconType = { name = "icon" position = { x = 0 y = 3 } size = { width = 20 height = 20 } spriteType = "GFX_resources_strip" }
        instantTextboxType = { name = "value" position = { x = 24 y = 4 } size = { width = 40 height = 20 } text = "" }
    }

    containerWindowType = {
        name = "logistics_entry_resource_item"
        size = { width = 25 height = 25 }
        iconType = { name = "icon" size = { width = 20 height = 20 } spriteType = "GFX_resources_strip" }
    }
}
"#,
    )
}

fn root_and_registry() -> (
    hoi4_ui::vanilla_gui::GuiDocument,
    GuiTemplateRegistry<'static>,
) {
    let doc = minimal_phase3_doc();
    let leaked: &'static hoi4_ui::vanilla_gui::GuiDocument = Box::leak(Box::new(doc.clone()));
    let registry = GuiTemplateRegistry::from_documents([("phase3", leaked)]);
    (doc, registry)
}

fn build_frame(
    data: &LogisticsData,
    viewport: GuiRect,
    runtime_state: GuiRuntimeState,
) -> GuiRuntimeFrame {
    let (doc, registry) = root_and_registry();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    logistics_vanilla_runtime_frame(root, data, viewport, runtime_state, registry)
}

fn equipment_context(
    template_name: &'static str,
    model_key: &str,
) -> hoi4_ui::vanilla_gui::GuiInstanceContext {
    hoi4_ui::vanilla_gui::GuiInstanceContext::new(
        template_name,
        0,
        GuiNodePath::root("materiel_grid"),
    )
    .with_semantic_role("logistics_equipment_entry")
    .with_model_key(model_key)
}

#[test]
fn logistics_vanilla_gate13_country_logistics_profile_skeleton_builds_frame_and_close() {
    let profile = CountryLogisticsProfile;
    let sample = data(vec![entry(
        "infantry_equipment",
        10.0,
        2.0,
        5.0,
        Vec::new(),
        Vec::new(),
    )]);
    assert_eq!(profile.profile_id(), COUNTRY_LOGISTICS_PROFILE_ID);
    assert_eq!(profile.root_template(), COUNTRY_LOGISTICS_ROOT);

    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = build_frame(&sample, viewport, GuiRuntimeState::shown(viewport));
    assert_eq!(frame.profile_id, COUNTRY_LOGISTICS_PROFILE_ID);
    assert_eq!(
        frame.root_layout.name.as_deref(),
        Some(COUNTRY_LOGISTICS_ROOT)
    );
    assert!(frame.visible);

    let close = profile.handle_action(
        GuiAction {
            node_path: GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("close_button"),
            kind: GuiActionKind::Click,
        },
        &sample,
    );
    assert_eq!(close, Some(PanelCommand::ClosePrimary));

    let mut report = String::from("# Gate 13 CountryLogisticsProfile\n\n");
    let _ = writeln!(report, "- profile_id: {COUNTRY_LOGISTICS_PROFILE_ID}");
    let _ = writeln!(report, "- root_template: {COUNTRY_LOGISTICS_ROOT}");
    let _ = writeln!(report, "- frame_visible: {}", frame.visible);
    report.push_str("- close_button: PanelCommand::ClosePrimary\n");
    write_report(report_path("gate13_country_logistics_profile.md"), report);
}

#[test]
fn logistics_vanilla_gate14_root_title_close_and_summary_bind_project_data() {
    let profile = CountryLogisticsProfile;
    let sample = data(vec![
        entry("infantry_equipment", 0.0, 2.0, 5.0, Vec::new(), Vec::new()),
        entry("artillery", 20.0, 5.0, 1.0, Vec::new(), Vec::new()),
    ]);
    let summary = logistics_vanilla_summary_text(&sample);
    assert!(summary.contains("2"));
    assert!(summary.contains("1"));

    let title = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("logistics_title"),
        &sample,
    );
    assert!(title.text.as_deref().is_some_and(|text| !text.is_empty()));
    assert_eq!(title.tooltip.as_deref(), Some(summary.as_str()));

    let close = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("close_button"),
        &sample,
    );
    assert_eq!(
        close.click.as_ref().map(|click| click.command.as_str()),
        Some("close")
    );

    let flag = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("viewing_flag"),
        &sample,
    );
    assert_eq!(flag.visible, Some(false));

    let mut report = String::from("# Gate 14 Root Summary Binding\n\n");
    let _ = writeln!(report, "- summary: {summary}");
    let _ = writeln!(report, "- title_text: {:?}", title.text);
    let _ = writeln!(report, "- close_command: {:?}", close.click);
    let _ = writeln!(report, "- viewing_flag_visible: {:?}", flag.visible);
    write_report(report_path("gate14_root_summary_binding.md"), report);
}

#[test]
fn logistics_vanilla_gate15_resource_strip_uses_project_resource_entries() {
    let profile = CountryLogisticsProfile;
    let sample = data(vec![entry(
        "infantry_equipment",
        10.0,
        4.0,
        8.0,
        Vec::new(),
        vec!["steel"],
    )]);
    let doc = minimal_phase3_doc();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    let resources_grid_node = root.find_node_by_name("resources_grid").unwrap();
    let specs = logistics_vanilla_resource_strip_instance_specs(
        &sample,
        GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("resource_strip_container")
            .child("resources_grid"),
        resources_grid_node,
        GuiRect::new(16.0, 46.0, 490.0, 31.0),
    );
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].template_name, "logistics_overview_resource_item");
    assert_eq!(
        specs[0].model_keys,
        vec!["oil".to_owned(), "steel".to_owned()]
    );

    let oil_context = hoi4_ui::vanilla_gui::GuiInstanceContext::new(
        "logistics_overview_resource_item",
        0,
        GuiNodePath::root("resources_grid"),
    )
    .with_semantic_role("logistics_resource_strip_item")
    .with_model_key("oil");
    let value = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_resource_item[0]").child("value"),
        &sample,
        Some(&oil_context),
    );
    assert_eq!(value.text.as_deref(), Some("7"));
    let icon = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_resource_item[0]").child("icon"),
        &sample,
        Some(&oil_context),
    );
    assert_eq!(icon.sprite.as_deref(), Some("GFX_resources_strip"));

    let mut report = String::from("# Gate 15 Resource Strip\n\n");
    let _ = writeln!(report, "- resource_order: {:?}", specs[0].model_keys);
    let _ = writeln!(report, "- oil_value_text: {:?}", value.text);
    report.push_str("- source: LogisticsData.resources; no vanilla resource demand derivation\n");
    write_report(report_path("gate15_resource_strip.md"), report);
}

#[test]
fn logistics_vanilla_gate16_dynamic_equipment_instances_use_project_entries() {
    let sample = overflow_data();
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = build_frame(&sample, viewport, GuiRuntimeState::shown(viewport));
    let row_instances: Vec<_> = frame
        .generated_instances
        .iter()
        .filter(|instance| {
            instance.context.semantic_role.as_deref() == Some("logistics_equipment_entry")
        })
        .collect();
    assert_eq!(row_instances.len(), sample.entries.len());

    let mut model_keys: Vec<_> = row_instances
        .iter()
        .filter_map(|instance| instance.context.model_key.clone())
        .collect();
    model_keys.sort();
    model_keys.dedup();
    assert_eq!(model_keys.len(), sample.entries.len());
    assert!(model_keys.contains(&"aircraft".to_owned()));
    assert!(model_keys.contains(&"convoy".to_owned()));

    let entry_hits: Vec<_> = frame
        .hit_regions
        .iter()
        .filter(|hit| hit.command.starts_with("logistics:entry:"))
        .collect();
    assert!(entry_hits
        .iter()
        .any(|hit| hit.command == "logistics:entry:aircraft"));

    let mut report = String::from("# Gate 16 Equipment Instances\n\n");
    let _ = writeln!(report, "- row_instances: {}", row_instances.len());
    let _ = writeln!(report, "- unique_model_keys: {}", model_keys.len());
    let _ = writeln!(report, "- hit_regions: {}", entry_hits.len());
    report.push_str("- template_selection: land/naval/air by LogisticsEntry.kind\n");
    write_report(report_path("gate16_equipment_instances.md"), report);
}

#[test]
fn logistics_vanilla_gate17_equipment_fields_bind_entry_values_without_duplicate_need() {
    let profile = CountryLogisticsProfile;
    let sample = data(vec![
        entry(
            "infantry_equipment",
            0.0,
            1.0,
            5.0,
            Vec::new(),
            vec!["steel"],
        ),
        entry("artillery", 20.0, 5.0, 1.0, Vec::new(), vec!["steel"]),
        entry("support_equipment", 0.0, 0.0, 0.0, Vec::new(), Vec::new()),
    ]);
    let deficit_ctx = equipment_context(
        "logistics_overview_land_equipment_entry",
        "infantry_equipment",
    );
    let surplus_ctx = equipment_context("logistics_overview_land_equipment_entry", "artillery");
    let no_need_ctx = equipment_context(
        "logistics_overview_land_equipment_entry",
        "support_equipment",
    );

    let needs = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("needs"),
        &sample,
        Some(&deficit_ctx),
    );
    assert_eq!(needs.text.as_deref(), Some("5.0"));
    assert!(needs.text_color.is_some());

    let balance = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("balance"),
        &sample,
        Some(&deficit_ctx),
    );
    assert_eq!(balance.text.as_deref(), Some("-4.0"));
    assert!(balance.text_color.is_some());

    let stock = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("in_stock"),
        &sample,
        Some(&deficit_ctx),
    );
    assert_eq!(stock.text.as_deref(), Some("0"));

    let surplus_balance = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[1]").child("balance"),
        &sample,
        Some(&surplus_ctx),
    );
    assert_eq!(surplus_balance.text.as_deref(), Some("4.0"));

    let no_need_progress = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[2]")
            .child("status_progressbar"),
        &sample,
        Some(&no_need_ctx),
    );
    assert_eq!(no_need_progress.progress, Some(1.0));

    let doc = minimal_phase3_doc();
    let row_template = doc
        .template_index()
        .get("logistics_overview_land_equipment_entry")
        .unwrap();
    let resources_grid_node = row_template.find_node_by_name("resources_grid").unwrap();
    let row_resources = logistics_vanilla_entry_resource_instance_specs(
        &sample.entries[0],
        GuiNodePath::root("resources_grid"),
        resources_grid_node,
        GuiRect::new(460.0, 4.0, 75.0, 50.0),
    );
    assert_eq!(row_resources[0].model_keys, vec!["steel".to_owned()]);

    let mut report = String::from("# Gate 17 Equipment Fields\n\n");
    let _ = writeln!(report, "- needs_text: {:?}", needs.text);
    let _ = writeln!(report, "- balance_text: {:?}", balance.text);
    let _ = writeln!(
        report,
        "- no_need_progress: {:?}",
        no_need_progress.progress
    );
    let _ = writeln!(
        report,
        "- row_resource_inputs: {:?}",
        row_resources[0].model_keys
    );
    report.push_str("- needs column binds entry.daily_consumption directly\n");
    write_report(report_path("gate17_equipment_fields.md"), report);
}

#[test]
fn logistics_vanilla_gate18_bottom_and_deferred_windows_do_not_show_fake_vanilla_data() {
    let profile = CountryLogisticsProfile;
    let mut sample = data(vec![
        entry("naval_vessel", 5.0, 2.0, 1.0, Vec::new(), vec!["steel"]),
        entry("convoy", 5.0, 1.0, 3.0, Vec::new(), vec!["steel"]),
    ]);
    sample.military_procurement_rm = 250_000.0;
    sample.military_maintenance_rm = 125_000.0;

    let military = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("military_factories"),
        &sample,
    );
    assert!(military
        .text
        .as_deref()
        .is_some_and(|text| text.contains("RM")));

    let naval = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("naval_factories"),
        &sample,
    );
    assert!(naval
        .text
        .as_deref()
        .is_some_and(|text| text.contains("RM")));

    let nuke = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("nuke"),
        &sample,
    );
    assert_eq!(nuke.visible, Some(false));
    let fuel = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("fuel_info"),
        &sample,
    );
    assert_eq!(fuel.visible, Some(false));

    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = build_frame(&sample, viewport, GuiRuntimeState::shown(viewport));
    assert!(frame
        .root_layout
        .find_by_name("logistics_info_window")
        .is_none());

    let mut report = String::from("# Gate 18 Bottom And Deferred Windows\n\n");
    let _ = writeln!(report, "- military_text: {:?}", military.text);
    let _ = writeln!(report, "- naval_text: {:?}", naval.text);
    let _ = writeln!(report, "- nuke_visible: {:?}", nuke.visible);
    let _ = writeln!(report, "- fuel_info_visible: {:?}", fuel.visible);
    report.push_str("- logistics_info_window: deferred to project detail panel path\n");
    report.push_str("- no fake vanilla factories, nukes, fuel priority, or fuel stockpile values are synthesized\n");
    write_report(report_path("gate18_bottom_and_deferred_windows.md"), report);
}

#[test]
fn logistics_vanilla_gate14_uses_550px_left_slide_root_without_local_offset() {
    let sample = data(vec![entry(
        "infantry_equipment",
        10.0,
        2.0,
        5.0,
        Vec::new(),
        Vec::new(),
    )]);
    let mut report = String::from("# Gate 14 Logistics Root Shell And Slide Animation\n\n");
    for (name, width, height) in [
        ("small", 960.0, 640.0),
        ("1080p", 1920.0, 1080.0),
        ("1440p", 2560.0, 1440.0),
    ] {
        let viewport = GuiRect::new(0.0, 0.0, width, height);
        let shown = build_frame(&sample, viewport, GuiRuntimeState::shown(viewport));
        assert_eq!(
            shown.diagnostics.root_hidden_position,
            GuiPoint { x: -606.0, y: 78.0 }
        );
        assert_eq!(
            shown.diagnostics.root_shown_position,
            GuiPoint { x: -6.0, y: 78.0 }
        );
        assert_eq!(
            shown.root_layout.rect,
            GuiRect::new(-6.0, 78.0, 550.0, height)
        );
        assert_eq!(
            shown.diagnostics.root_animation_offset,
            GuiPoint { x: 0.0, y: 0.0 }
        );

        let hidden = build_frame(&sample, viewport, GuiRuntimeState::hidden(viewport));
        assert_eq!(
            hidden.root_layout.rect,
            GuiRect::new(-606.0, 78.0, 550.0, height)
        );

        let current = build_frame(
            &sample,
            viewport,
            GuiRuntimeState {
                root_position: GuiRuntimeRootPosition::Current(GuiPoint { x: -306.0, y: 78.0 }),
                phase: Some(AnimationPhase::Opening),
                visible: true,
                ..GuiRuntimeState::shown(viewport)
            },
        );
        assert_eq!(
            current.root_layout.rect,
            GuiRect::new(-306.0, 78.0, 550.0, height)
        );
        assert_eq!(
            current.diagnostics.root_animation_offset,
            GuiPoint { x: -300.0, y: 0.0 }
        );
        let _ = writeln!(
            report,
            "- {name}: shown={:?} hidden={:?} current={:?}",
            shown.root_layout.rect, hidden.root_layout.rect, current.root_layout.rect
        );
    }

    let (doc, _) = root_and_registry();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    let spec = AnimationSpec::from_node(root);
    assert_eq!(spec.hidden_position, GuiPoint { x: -606.0, y: 78.0 });
    assert_eq!(spec.shown_position, GuiPoint { x: -6.0, y: 78.0 });
    assert_eq!(spec.duration_ms, 300.0);

    let profile = CountryLogisticsProfile;
    let close = profile.handle_action(
        GuiAction {
            node_path: GuiNodePath::root("close"),
            kind: GuiActionKind::Click,
        },
        &sample,
    );
    assert_eq!(close, Some(PanelCommand::ClosePrimary));
    report.push_str(
        "\n- local_root_offset: none; frame uses runtime `AnimationSpec::from_node(root)`.\n",
    );
    report.push_str("- close_button: mapped to PanelCommand::ClosePrimary; ESC shortcut remains on vanilla button metadata.\n");
    write_report(report_path("gate14_root_shell_animation.md"), report);
}

#[test]
fn logistics_vanilla_gate15_headers_and_rows_follow_vanilla_visual_slots() {
    let sample = overflow_data();
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = build_frame(&sample, viewport, GuiRuntimeState::shown(viewport));
    let headers = frame.root_layout.find_by_name("headers").unwrap();
    assert_eq!(headers.rect.height, 25.0);
    assert_eq!(headers.rect.y, frame.root_layout.rect.y + 88.0);
    assert!(headers.find_by_name("efficiency_icon").is_some());
    assert!(headers.find_by_name("equipment_type_label").is_some());
    assert!(headers.find_by_name("status_label").is_some());
    assert!(headers.find_by_name("producing_icon").is_some());
    assert!(headers.find_by_name("need_icon").is_some());
    assert!(headers.find_by_name("balance_icon").is_some());
    assert!(headers.find_by_name("in_stock_icon").is_some());
    let resources_label = headers.find_by_name("resources_label").unwrap();
    assert!(!resources_label.visible);

    let resource_strip = frame
        .root_layout
        .find_by_name("resource_strip_container")
        .unwrap();
    assert!(resource_strip.visible);
    assert_eq!(resource_strip.rect.height, 31.0);
    let materiel_grid = frame.root_layout.find_by_name("materiel_grid").unwrap();
    assert!(materiel_grid.clip_rect.y >= materiel_grid.rect.y);

    let raw_text: Vec<_> = frame
        .draw_list
        .iter()
        .filter_map(|command| match &command.kind {
            GuiDrawCommandKind::DrawText(text) => Some(text.raw_text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        raw_text.iter().all(|text| !text.starts_with("LOGISTICS_")),
        "raw logistics localization keys leaked into draw list: {raw_text:?}"
    );
    assert!(
        raw_text.contains(&"装备"),
        "equipment header must be explicitly localized"
    );
    assert!(
        raw_text.contains(&"供需"),
        "status header must be explicitly localized"
    );

    let row_instances: Vec<_> = frame
        .generated_instances
        .iter()
        .filter(|instance| {
            instance.context.semantic_role.as_deref() == Some("logistics_equipment_entry")
        })
        .collect();
    assert_eq!(row_instances.len(), sample.entries.len());
    assert!(row_instances
        .iter()
        .all(|instance| instance.slot_rect.height == 58.0));
    assert!(row_instances
        .iter()
        .all(|instance| instance.resolved_rect.height <= 58.0));
    assert!(row_instances
        .iter()
        .all(|instance| instance.resolved_rect.width <= materiel_grid.rect.width));
    let draw_paths: Vec<_> = frame
        .draw_list
        .iter()
        .map(|command| command.source_path.to_string())
        .collect();
    assert!(
        draw_paths
            .iter()
            .any(|path| path.contains("resource_strip_container")),
        "top resource strip must be drawn from the GUI template"
    );
    assert!(
        draw_paths
            .iter()
            .any(|path| path.contains("equipment_icon") || path.contains("Background")),
        "row art slots must be drawn from the GUI template"
    );
    let resource_names: Vec<_> = frame
        .draw_list
        .iter()
        .filter_map(|command| command.resource_name())
        .collect();
    assert!(resource_names.contains(&"GFX_resources_strip"));
    assert!(resource_names
        .iter()
        .any(|name| name.starts_with("GFX_logistics_")));
    assert!(
        frame.diagnostics.draw_commands.fallback <= frame.diagnostics.draw_commands.commands.len()
    );

    let mut report = String::from("# Gate 15 Logistics Headers And Materiel Visual\n\n");
    let _ = writeln!(report, "- snapshot: {LOGISTICS_VANILLA_SNAPSHOT_1080P}");
    let _ = writeln!(report, "- row_instances: {}", row_instances.len());
    let _ = writeln!(report, "- row_height_px: 58");
    let _ = writeln!(report, "- headers_height_px: {}", headers.rect.height);
    let _ = writeln!(
        report,
        "- resource_strip_visible: {}",
        resource_strip.visible
    );
    let _ = writeln!(
        report,
        "- resources_label_visible: {}",
        resources_label.visible
    );
    let _ = writeln!(report, "- no_raw_logistics_keys: true");
    write_report(report_path("gate15_headers_materiel_visual.md"), report);
}

#[test]
fn logistics_vanilla_gate15_snapshot_1080p_when_runtime_available() {
    let Some(_context) = hoi4_ui::vanilla_gui::country_logistics_runtime_context() else {
        write_report(
            report_path("gate15_snapshot_1080p.md"),
            "# Gate 15 Logistics 1080p Snapshot\n\n- skipped: HOI4 vanilla runtime unavailable\n",
        );
        return;
    };
    let sample = overflow_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1920.0, 1080.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = LogisticsPanel::show(ctx, &sample);
        });

    harness.run_steps(40);
    snapshot_if_enabled(&mut harness, "logistics_vanilla_1080p");
    let snapshot_exists = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots/logistics_vanilla_1080p.png")
        .exists();
    let mut report = String::from("# Gate 15 Logistics 1080p Snapshot\n\n");
    let _ = writeln!(report, "- runtime_available: true");
    let _ = writeln!(report, "- snapshot: {LOGISTICS_VANILLA_SNAPSHOT_1080P}");
    let _ = writeln!(report, "- snapshot_exists: {snapshot_exists}");
    report.push_str(
        "- note: snapshot is project test output; no vanilla HOI4 assets are committed.\n",
    );
    write_report(report_path("gate15_snapshot_1080p.md"), report);
}

#[test]
fn logistics_vanilla_gate16_scroll_clip_and_hit_regions_route_visible_rows() {
    let sample = overflow_data();
    let viewport = GuiRect::new(0.0, 0.0, 960.0, 640.0);
    let (doc, registry) = root_and_registry();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    let base = GuiRuntimeFrame::build(GuiRuntimeFrameInput::new(
        root,
        viewport,
        COUNTRY_LOGISTICS_PROFILE_ID,
        &hoi4_ui::vanilla_gui::bind_profile_tree(&CountryLogisticsProfile, root, &sample),
    ));
    let materiel_path = base
        .root_layout
        .find_by_name("materiel")
        .unwrap()
        .path
        .clone();
    let frame = logistics_vanilla_runtime_frame(
        root,
        &sample,
        viewport,
        GuiRuntimeState::shown(viewport)
            .with_scroll_offset(materiel_path, GuiPoint { x: 0.0, y: 116.0 }),
        registry,
    );

    let materiel_scroll = frame
        .scroll_states
        .iter()
        .find(|state| state.node_name.as_deref() == Some("materiel"))
        .unwrap();
    assert!(materiel_scroll.spec.has_vertical_scrollbar);
    assert_eq!(materiel_scroll.spec.scroll_wheel_factor, 40.0);
    assert!(materiel_scroll.spec.smooth_scrolling);
    assert_eq!(materiel_scroll.spec.margin.top, 33.0);
    assert_eq!(materiel_scroll.spec.margin.bottom, 123.0);

    let close_hit = frame
        .hit_regions
        .iter()
        .find(|hit| hit.command == "close")
        .expect("close hit region");
    let entry_hits: Vec<_> = frame
        .hit_regions
        .iter()
        .filter(|hit| hit.command.starts_with("logistics:entry:"))
        .collect();
    assert!(!entry_hits.is_empty());
    assert!(entry_hits
        .iter()
        .all(|hit| hit.rect == hit.rect.intersect(materiel_scroll.content_clip_rect)));
    assert!(entry_hits
        .iter()
        .all(|hit| hit.rect.intersect(close_hit.rect).width == 0.0
            || hit.rect.intersect(close_hit.rect).height == 0.0));

    let visible_commands: Vec<_> = entry_hits.iter().map(|hit| hit.command.as_str()).collect();
    assert!(visible_commands
        .iter()
        .any(|command| command.contains("equipment_")));
    assert!(close_hit.enabled);

    let mut report = String::from("# Gate 16 Logistics Scroll Clip And Hit Test\n\n");
    let _ = writeln!(
        report,
        "- scroll_state: {}",
        frame.diagnostics.scroll_markdown()
    );
    let _ = writeln!(report, "- close_hit: {:?}", close_hit.rect);
    let _ = writeln!(report, "- entry_hit_regions: {}", entry_hits.len());
    let _ = writeln!(report, "- visible_entry_commands: {:?}", visible_commands);
    write_report(report_path("gate16_scroll_hit_test.md"), report);
}

#[test]
fn logistics_vanilla_gate17_detail_action_covers_deficit_and_surplus_rows() {
    let profile = CountryLogisticsProfile;
    let sample = data(vec![
        entry(
            "infantry_equipment",
            0.0,
            1.0,
            5.0,
            Vec::new(),
            vec!["steel"],
        ),
        entry("artillery", 20.0, 5.0, 1.0, Vec::new(), vec!["steel"]),
    ]);

    for key in ["infantry_equipment", "artillery"] {
        let command = profile.handle_action(
            GuiAction {
                node_path: GuiNodePath::root(format!("logistics:entry:{key}")),
                kind: GuiActionKind::Click,
            },
            &sample,
        );
        match command {
            Some(PanelCommand::OpenDetail(ActiveDetailPanel::Goods(target))) => {
                assert_eq!(target.good_id, key);
                assert_eq!(target.source, Some(DetailSource::Logistics));
            }
            other => panic!("unexpected command for {key}: {other:?}"),
        }
    }

    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = build_frame(&sample, viewport, GuiRuntimeState::shown(viewport));
    let entry_hits: Vec<_> = frame
        .hit_regions
        .iter()
        .filter(|hit| hit.command.starts_with("logistics:entry:"))
        .collect();
    assert_eq!(entry_hits.len(), 2);
    assert!(frame
        .root_layout
        .find_by_name("logistics_info_window")
        .is_none());

    let mut report = String::from("# Gate 17 Logistics Detail Action\n\n");
    report.push_str("- action: equipment row -> PanelCommand::OpenDetail(ActiveDetailPanel::Goods(... DetailSource::Logistics))\n");
    report.push_str("- logistics_info_window: deferred; no fake history chart is generated without project history data.\n");
    let _ = writeln!(
        report,
        "- tested_rows: deficit=infantry_equipment surplus=artillery"
    );
    write_report(report_path("gate17_detail_action.md"), report);
}

#[test]
fn logistics_vanilla_gate18_fuel_info_hidden_when_project_has_no_fuel_data() {
    let profile = CountryLogisticsProfile;
    let sample = data(vec![entry(
        "infantry_equipment",
        0.0,
        1.0,
        5.0,
        Vec::new(),
        Vec::new(),
    )]);
    let fuel = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("fuel_info"),
        &sample,
    );
    assert_eq!(fuel.visible, Some(false));

    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = build_frame(&sample, viewport, GuiRuntimeState::shown(viewport));
    let fuel_node = frame.root_layout.find_by_name("fuel_info").unwrap();
    assert!(
        fuel_node.rect.height > 0.0,
        "vanilla slot exists but binding hides it"
    );
    assert!(
        frame
            .draw_list
            .iter()
            .all(|command| !command.source_path.to_string().contains("fuel_info")),
        "hidden fuel_info must not emit draw commands"
    );
    assert!(frame.visible);

    let mut report = String::from("# Gate 18 Fuel Strategy\n\n");
    report.push_str("- project_fuel_data: unavailable in current LogisticsData contract\n");
    report.push_str("- fuel_info_binding: visible=false\n");
    report.push_str(
        "- priority_buttons: deferred; no vanilla HOI4 priority simulation is hard-coded\n",
    );
    report.push_str("- zero_or_fake_100_percent_bar: not rendered\n");
    write_report(report_path("gate18_fuel_strategy.md"), report);
}
