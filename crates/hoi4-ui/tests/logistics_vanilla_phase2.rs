use std::fmt::Write as _;
use std::path::PathBuf;

use hoi4_ui::logistics_panel::{
    logistics_vanilla_entry_kind, logistics_vanilla_entry_resource_instance_specs,
    logistics_vanilla_resource_strip_instance_specs, logistics_vanilla_summary_text,
    CountryLogisticsProfile, LogisticsData, LogisticsEntry, ResourceEntry, ResourceInputEntry,
};
use hoi4_ui::vanilla_gui::{
    parse_gui_str, GuiInstanceContext, GuiNodePath, GuiRect, VanillaPanelProfile,
    COUNTRY_LOGISTICS_ROOT,
};

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

fn entry_context(name: &str) -> GuiInstanceContext {
    GuiInstanceContext::new(
        "logistics_overview_land_equipment_entry",
        0,
        GuiNodePath::root("materiel_grid"),
    )
    .with_semantic_role("logistics_equipment_entry")
    .with_model_key(name)
}

fn resources_grid_doc(body: &str) -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        None,
        &format!(
            r#"
guiTypes = {{
    gridBoxType = {{
        name = "resources_grid"
        {body}
    }}
}}
"#
        ),
    )
}

#[test]
fn logistics_vanilla_gate9_binds_main_window_summary_and_flag_state() {
    let profile = CountryLogisticsProfile;
    let data = data(vec![
        entry("步兵装备", 0.0, 2.0, 5.0, Vec::new(), Vec::new()),
        entry("火炮", 20.0, 5.0, 1.0, Vec::new(), Vec::new()),
    ]);

    let summary = logistics_vanilla_summary_text(&data);
    assert_eq!(
        summary,
        "类型 2 | 缺口 1 | 日产 +7.0/日 | 需求 -6.0/日 | 净变化 +1.0/日"
    );

    let title = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("logistics_title"),
        &data,
    );
    assert!(title.text.as_deref().is_some_and(|text| !text.is_empty()));
    assert_ne!(title.text.as_deref(), Some("LOGISTICS_LOGISTICS_LABEL"));
    assert_eq!(title.tooltip.as_deref(), Some(summary.as_str()));

    let flag = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("viewing_flag"),
        &data,
    );
    assert_eq!(flag.visible, Some(false));

    let resource_strip = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("resource_strip_container")
            .child("resources_grid"),
        &data,
    );
    assert_eq!(resource_strip.visible, None);
    assert!(resource_strip.tooltip.is_none());

    let mut report = String::from("# Gate 9 Logistics Summary Binding\n\n");
    let _ = writeln!(report, "- summary: {summary}");
    let _ = writeln!(report, "- title_text: {:?}", title.text);
    let _ = writeln!(report, "- viewing_flag_visible: {:?}", flag.visible);
    let _ = writeln!(
        report,
        "- resource_strip_visible: {:?}",
        resource_strip.visible
    );
    write_report(report_path("gate9_summary_binding.md"), report);
}

#[test]
fn logistics_vanilla_gate10_binds_equipment_row_fields_and_progress() {
    let profile = CountryLogisticsProfile;
    let data = data(vec![
        entry("步兵装备", 0.0, 4.0, 8.0, Vec::new(), Vec::new()),
        entry("支援装备", 12.0, 0.0, 0.0, Vec::new(), Vec::new()),
    ]);
    let ctx = entry_context("步兵装备");

    let equipment_type = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("equipment_type"),
        &data,
        Some(&ctx),
    );
    let produced = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("produced"),
        &data,
        Some(&ctx),
    );
    let needs = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("needs"),
        &data,
        Some(&ctx),
    );
    let balance = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("balance"),
        &data,
        Some(&ctx),
    );
    let in_stock = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("in_stock"),
        &data,
        Some(&ctx),
    );
    let progress = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]")
            .child("status_progressbar"),
        &data,
        Some(&ctx),
    );

    assert_eq!(equipment_type.text.as_deref(), Some("步兵装备"));
    assert_eq!(produced.text.as_deref(), Some("4.0"));
    assert_eq!(needs.text.as_deref(), Some("8.0"));
    assert_eq!(balance.text.as_deref(), Some("-4.0"));
    assert_eq!(in_stock.text.as_deref(), Some("0"));
    assert_eq!(progress.progress, Some(0.5));
    assert!(needs.text_color.is_some());
    assert!(balance.text_color.is_some());

    let no_need_ctx = entry_context("支援装备");
    let no_need_progress = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[1]")
            .child("status_progressbar"),
        &data,
        Some(&no_need_ctx),
    );
    assert_eq!(no_need_progress.progress, Some(1.0));
}

#[test]
fn logistics_vanilla_gate11_binds_top_and_row_resources_from_project_data() {
    let profile = CountryLogisticsProfile;
    let data = data(vec![entry(
        "步兵装备",
        10.0,
        4.0,
        8.0,
        Vec::new(),
        vec!["steel", "rubber_parts"],
    )]);

    let specs = logistics_vanilla_resource_strip_instance_specs(
        &data,
        GuiNodePath::root("resources_grid"),
        resources_grid_doc(
            r#"
            size = { width = 490 height = 31 }
            slotsize = { width = 70 height = 31 }
            max_slots = { x = 7 y = 1 }
            add_horizontal = no
            "#,
        )
        .template_index()
        .get("resources_grid")
        .unwrap(),
        GuiRect::new(20.0, 46.0, 490.0, 30.0),
    );
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].template_name, "logistics_overview_resource_item");
    assert_eq!(
        specs[0].semantic_role.as_deref(),
        Some("logistics_resource_strip_item")
    );
    assert_eq!(
        specs[0].model_keys,
        vec!["oil".to_owned(), "steel".to_owned()]
    );

    let entry_ctx = entry_context("步兵装备");
    let resource_ctx = GuiInstanceContext::new(
        "logistics_overview_resource_item",
        0,
        GuiNodePath::root("resources_grid"),
    )
    .with_semantic_role("logistics_resource_strip_item")
    .with_model_key("oil");
    let resource_icon = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_resource_item[0]").child("icon"),
        &data,
        Some(&resource_ctx),
    );
    let resource_value = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_resource_item[0]").child("value"),
        &data,
        Some(&resource_ctx),
    );
    assert_eq!(resource_icon.sprite.as_deref(), Some("GFX_resources_strip"));
    assert_eq!(resource_icon.frame, Some(1));
    assert_eq!(resource_value.text.as_deref(), Some("7"));

    let row_grid = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("resources_grid"),
        &data,
        Some(&entry_ctx),
    );
    assert_eq!(row_grid.visible, None);
    assert!(row_grid.tooltip.is_none());

    let equipment_icon = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("equipment_icon"),
        &data,
        Some(&entry_ctx),
    );
    assert_eq!(equipment_icon.visible, Some(true));
    assert_eq!(
        equipment_icon.sprite.as_deref(),
        Some("GFX_archetype_infantry_equipment_medium")
    );

    let row_specs = logistics_vanilla_entry_resource_instance_specs(
        &data.entries[0],
        GuiNodePath::root("resources_grid"),
        resources_grid_doc(
            r#"
            size = { width = 75 height = 25 }
            slotsize = { width = 25 height = 25 }
            max_slots_vertical = 2
            "#,
        )
        .template_index()
        .get("resources_grid")
        .unwrap(),
        GuiRect::new(420.0, 5.0, 75.0, 25.0),
    );
    assert_eq!(row_specs.len(), 1);
    assert_eq!(row_specs[0].template_name, "logistics_entry_resource_item");
    assert_eq!(
        row_specs[0].semantic_role.as_deref(),
        Some("logistics_entry_resource_item")
    );
    assert_eq!(
        row_specs[0].model_keys,
        vec!["steel".to_owned(), "rubber_parts".to_owned()]
    );

    let mut report = String::from("# Gate 11 Resource Binding\n\n");
    let _ = writeln!(report, "- top_resource_instances: {}", specs.len());
    let _ = writeln!(
        report,
        "- row_resource_inputs: {:?}",
        data.entries[0].resource_inputs
    );
    let _ = writeln!(
        report,
        "- row_resources_grid_visible: {:?}",
        row_grid.visible
    );
    let _ = writeln!(
        report,
        "- equipment_icon_visible: {:?}",
        equipment_icon.visible
    );
    report.push_str(
        "\nResource data remains in `LogisticsData`; row equipment icons bind concrete vanilla sprites rather than drawing the dynamic `GFX_equipment_item` atlas directly.\n",
    );
    write_report(report_path("gate11_resource_binding.md"), report);
}

#[test]
fn logistics_vanilla_gate12_binds_production_sources_to_row_tooltip() {
    let profile = CountryLogisticsProfile;
    let data = data(vec![
        entry(
            "步兵装备",
            0.0,
            1.0,
            5.0,
            vec![
                "柏林 军工厂 Lv3 +3.0/day",
                "汉堡 军工厂 Lv2 +2.0/day",
                "慕尼黑 军工厂 Lv1 +1.0/day",
                "科隆 军工厂 Lv1 +0.5/day",
            ],
            vec!["steel"],
        ),
        entry("火炮", 0.0, 0.0, 4.0, Vec::new(), vec!["steel"]),
    ]);

    let row = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]"),
        &data,
        Some(&entry_context("步兵装备")),
    );
    let tooltip = row.tooltip.as_deref().unwrap_or_default();
    assert!(tooltip.contains("生产来源"));
    assert!(tooltip.contains("柏林 军工厂 Lv3 +3.0/day"));
    assert!(tooltip.contains("慕尼黑 军工厂 Lv1 +1.0/day"));
    assert!(!tooltip.contains("科隆 军工厂 Lv1 +0.5/day"));
    assert!(!tooltip.contains("主要投入"));
    assert!(row.click.is_none());

    let row_hit = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[0]").child("row_hit"),
        &data,
        Some(&entry_context("姝ゅ叺瑁呭")),
    );
    assert!(row_hit.click.is_some());

    let no_source = profile.bind_node_with_context(
        &GuiNodePath::root("logistics_overview_land_equipment_entry[1]"),
        &data,
        Some(&entry_context("火炮")),
    );
    assert!(no_source
        .tooltip
        .as_deref()
        .is_some_and(|tooltip| tooltip.contains("当前无有效军工产出")));

    let mut report = String::from("# Gate 12 Production Source Binding\n\n");
    let _ = writeln!(
        report,
        "- tooltip_source_lines_capped_at_3: {}",
        !tooltip.contains("科隆 军工厂 Lv1 +0.5/day")
    );
    let _ = writeln!(
        report,
        "- row_click_command: {:?}",
        row_hit.click.as_ref().map(|click| click.command.as_str())
    );
    write_report(report_path("gate12_production_sources.md"), report);
}

#[test]
fn logistics_vanilla_gate13_binds_bottom_budget_area_without_fake_factories() {
    let profile = CountryLogisticsProfile;
    let mut data = data(vec![
        entry("舰艇", 5.0, 2.0, 1.0, Vec::new(), vec!["steel"]),
        entry("运输船", 5.0, 1.0, 3.0, Vec::new(), vec!["steel"]),
    ]);
    data.military_procurement_rm = 0.0;
    data.military_maintenance_rm = 0.0;

    let military = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("military_factories"),
        &data,
    );
    assert_eq!(military.text.as_deref(), Some("军购 0 RM/日"));
    assert!(military
        .tooltip
        .as_deref()
        .is_some_and(|tooltip| tooltip.contains("军购预算")));

    let military_usage = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("military_factories")
            .child("military_factories_usage"),
        &data,
    );
    assert!(military_usage.progress.is_some());

    let naval = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("naval_factories"),
        &data,
    );
    assert_eq!(naval.text.as_deref(), Some("维护 0 RM/日"));
    assert!(naval
        .tooltip
        .as_deref()
        .is_some_and(|tooltip| tooltip.contains("舰船/运输船日产 3.0/日")));

    let nuke = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("nuke"),
        &data,
    );
    let nuke_count = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("nuke")
            .child("nuke_count"),
        &data,
    );
    assert_eq!(nuke.visible, Some(false));
    assert_eq!(nuke_count.visible, Some(false));
    assert_eq!(nuke_count.text.as_deref(), Some("0"));

    data.military_procurement_rm = 250_000.0;
    data.military_maintenance_rm = 125_000.0;
    let nonzero_military = profile.bind_node(
        &GuiNodePath::root(COUNTRY_LOGISTICS_ROOT)
            .child("production_win_bottom")
            .child("military_factories"),
        &data,
    );
    assert_eq!(nonzero_military.text.as_deref(), Some("军购 250.0K RM/日"));

    let mut report = String::from("# Gate 13 Fiscal And Bottom Binding\n\n");
    let _ = writeln!(report, "- military_text_zero: {:?}", military.text);
    let _ = writeln!(report, "- naval_text_zero: {:?}", naval.text);
    let _ = writeln!(report, "- nuke_visible: {:?}", nuke.visible);
    let _ = writeln!(
        report,
        "- nonzero_military_text: {:?}",
        nonzero_military.text
    );
    report.push_str("\nNo vanilla HOI4 factory counts are synthesized; bottom values are project budget/output summaries.\n");
    write_report(report_path("gate13_bottom_binding.md"), report);
}
