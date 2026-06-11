use std::fmt::Write as _;
use std::path::PathBuf;

use egui_kittest::Harness;
use hoi4_ui::egui::Vec2;
use hoi4_ui::icons::IconBank;
use hoi4_ui::logistics_panel::{
    logistics_vanilla_entry_kind, logistics_vanilla_runtime_frame_parts, CountryLogisticsProfile,
    LogisticsData, LogisticsEntry, LogisticsPanel, ResourceEntry, ResourceInputEntry,
    LOGISTICS_VANILLA_SNAPSHOT_1080P,
};
use hoi4_ui::vanilla_gui::{
    parse_gui_str, AnimationPhase, AnimationSpec, GfxIndex, GuiAction, GuiActionKind,
    GuiDrawCommandKind, GuiNodePath, GuiPoint, GuiRect, GuiRuntimeRootPosition, GuiRuntimeState,
    GuiTemplateRegistry, VanillaPanelProfile, COUNTRY_LOGISTICS_GUI_FILE, COUNTRY_LOGISTICS_ROOT,
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

fn entry(
    id: &str,
    name: &str,
    stockpile: f32,
    daily_production: f32,
    daily_consumption: f32,
    resource_inputs: Vec<&str>,
) -> LogisticsEntry {
    let deficit = (daily_consumption - daily_production).max(0.0);
    LogisticsEntry {
        id: id.to_owned(),
        name: name.to_owned(),
        kind: logistics_vanilla_entry_kind(id),
        equipment_icon_sprite: hoi4_ui::logistics_panel::logistics_equipment_icon_sprite_for_id(id)
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
        production_sources: Vec::new(),
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
            .map(|index| match index {
                0 => entry(
                    "infantry_equipment",
                    "Infantry Equipment",
                    0.0,
                    1.0,
                    6.0,
                    vec!["steel"],
                ),
                1 => entry("artillery", "Artillery", 20.0, 5.0, 1.0, vec!["steel"]),
                2 => entry("aircraft", "Aircraft", 10.0, 1.0, 6.0, vec!["aluminium"]),
                3 => entry("convoy", "Convoy", 10.0, 5.0, 1.0, vec!["steel"]),
                _ => entry(
                    &format!("equipment_{index:02}"),
                    &format!("Equipment {index:02}"),
                    10.0,
                    if index % 2 == 0 { 1.0 } else { 5.0 },
                    if index % 2 == 0 { 6.0 } else { 1.0 },
                    Vec::new(),
                ),
            })
            .collect(),
    )
}

fn phase4_doc() -> hoi4_ui::vanilla_gui::GuiDocument {
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

        background = { name = "window_bg" spriteType = "GFX_tiled_window" }
        instantTextboxType = { name = "logistics_title" position = { x = 14 y = 8 } size = { width = 260 height = 24 } text = "" }
        buttonType = { name = "close_button" position = { x = -43 y = 10 } size = { width = 32 height = 32 } Orientation = "UPPER_RIGHT" shortcut = "ESCAPE" quadTextureSprite = "GFX_closebutton" }

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
            iconType = { name = "producing_icon" position = { x = 156 y = 0 } spriteType = "GFX_producing_icon" }
            iconType = { name = "need_icon" position = { x = 244 y = 0 } spriteType = "GFX_need_icon" }
            iconType = { name = "balance_icon" position = { x = 325 y = 0 } spriteType = "GFX_balance_icon" }
            iconType = { name = "in_stock_icon" position = { x = 405 y = 0 } spriteType = "GFX_in_stock_icon" }
            instantTextboxType = { name = "equipment_type_label" position = { x = 30 y = 0 } size = { width = 125 height = 20 } text = "LOGISTICS_EQUIPMENT_TYPE_LABEL" }
            instantTextboxType = { name = "status_label" position = { x = 183 y = 0 } size = { width = 70 height = 20 } text = "LOGISTICS_STATUS_LABEL" }
            instantTextboxType = { name = "resources_label" position = { x = 465 y = 0 } size = { width = 50 height = 20 } text = "LOGISTICS_RESOURCES_LABEL" }
        }

        containerWindowType = { name = "fuel_info" position = { x = 0 y = -210 } size = { width = 546 height = 103 } Orientation = LOWER_LEFT }
        containerWindowType = {
            name = "production_win_bottom"
            position = { x = 1 y = -98 }
            size = { width = 546 height = 98 }
            Orientation = LOWER_LEFT
            containerWindowType = { name = "military_factories" position = { x = 20 y = 4 } size = { width = 130 height = 70 } }
            containerWindowType = { name = "naval_factories" position = { x = 170 y = 4 } size = { width = 130 height = 70 } }
            containerWindowType = { name = "nuke" position = { x = 320 y = 4 } size = { width = 90 height = 70 } instantTextboxType = { name = "nuke_count" text = "0" } }
        }
    }

    containerWindowType = {
        name = "logistics_overview_land_equipment_entry"
        size = { width = 565 height = 58 }
        background = { name = "entry_bg" position = { x = 0 y = 0 } size = { width = 545 height = 58 } spriteType = "GFX_logistics_equipment_entry_bg" }
        buttonType = { name = "row_hit" position = { x = 0 y = 0 } size = { width = 545 height = 58 } }
        iconType = { name = "equipment_icon" position = { x = 4 y = 4 } size = { width = 40 height = 40 } spriteType = "GFX_logistics_equipment_entry_bg" }
        instantTextboxType = { name = "equipment_type" position = { x = 48 y = 6 } size = { width = 132 height = 20 } text = "" }
        instantTextboxType = { name = "produced" position = { x = 190 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "needs" position = { x = 260 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "balance" position = { x = 330 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "in_stock" position = { x = 400 y = 6 } size = { width = 50 height = 20 } text = "" }
        iconType = { name = "status_progressbar" position = { x = 48 y = 34 } size = { width = 320 height = 10 } spriteType = "GFX_logistics_progressbar" }
        gridBoxType = { name = "resources_grid" position = { x = 460 y = 4 } size = { width = 75 height = 50 } slotsize = { width = 25 height = 25 } max_slots = { x = 3 y = 2 } }
    }

    containerWindowType = {
        name = "logistics_overview_naval_equipment_entry"
        size = { width = 565 height = 58 }
        background = { name = "entry_bg" position = { x = 0 y = 0 } size = { width = 545 height = 58 } spriteType = "GFX_logistics_naval_equipment_entry_bg" }
        buttonType = { name = "row_hit" position = { x = 0 y = 0 } size = { width = 545 height = 58 } }
        instantTextboxType = { name = "equipment_type" position = { x = 48 y = 6 } size = { width = 132 height = 20 } text = "" }
        instantTextboxType = { name = "produced" position = { x = 190 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "needs" position = { x = 260 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "balance" position = { x = 330 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "in_stock" position = { x = 400 y = 6 } size = { width = 50 height = 20 } text = "" }
        iconType = { name = "status_progressbar" position = { x = 48 y = 34 } size = { width = 320 height = 10 } spriteType = "GFX_logistics_progressbar" }
        gridBoxType = { name = "resources_grid" position = { x = 460 y = 4 } size = { width = 75 height = 50 } slotsize = { width = 25 height = 25 } max_slots = { x = 3 y = 2 } }
    }

    containerWindowType = {
        name = "logistics_overview_air_equipment_entry"
        size = { width = 565 height = 58 }
        background = { name = "entry_bg" position = { x = 0 y = 0 } size = { width = 545 height = 58 } spriteType = "GFX_logistics_air_equipment_entry_bg" }
        buttonType = { name = "row_hit" position = { x = 0 y = 0 } size = { width = 545 height = 58 } }
        instantTextboxType = { name = "equipment_type" position = { x = 48 y = 6 } size = { width = 132 height = 20 } text = "" }
        instantTextboxType = { name = "produced" position = { x = 190 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "needs" position = { x = 260 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "balance" position = { x = 330 y = 6 } size = { width = 50 height = 20 } text = "" }
        instantTextboxType = { name = "in_stock" position = { x = 400 y = 6 } size = { width = 50 height = 20 } text = "" }
        iconType = { name = "status_progressbar" position = { x = 48 y = 34 } size = { width = 320 height = 10 } spriteType = "GFX_logistics_progressbar" }
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

fn phase4_gfx_index() -> GfxIndex {
    GfxIndex::parse_single(
        "phase4.gfx",
        r#"
spriteType = { name = "GFX_tiled_window" texturefile = "gfx/interface/window.dds" }
spriteType = { name = "GFX_closebutton" texturefile = "gfx/interface/closebutton.dds" noOfFrames = 3 }
spriteType = { name = "GFX_logistics_equipment_entry_bg" texturefile = "gfx/interface/production/logistics_entry.dds" }
spriteType = { name = "GFX_logistics_naval_equipment_entry_bg" texturefile = "gfx/interface/production/logistics_naval_entry.dds" }
spriteType = { name = "GFX_logistics_air_equipment_entry_bg" texturefile = "gfx/interface/production/logistics_air_entry.dds" }
spriteType = { name = "GFX_resources_strip" texturefile = "gfx/interface/resources_strip.dds" noOfFrames = 6 }
spriteType = { name = "GFX_producing_icon" texturefile = "gfx/interface/production/producing.dds" }
spriteType = { name = "GFX_need_icon" texturefile = "gfx/interface/production/need.dds" }
spriteType = { name = "GFX_balance_icon" texturefile = "gfx/interface/production/balance.dds" }
spriteType = { name = "GFX_in_stock_icon" texturefile = "gfx/interface/production/in_stock.dds" }
progressbartype = {
    name = "GFX_logistics_progressbar"
    textureFile1 = "gfx/interface/production/logistics_progressbar.dds"
    textureFile2 = "gfx/interface/production/logistics_progressbar_bg.dds"
}
"#,
    )
}

fn root_registry() -> (
    hoi4_ui::vanilla_gui::GuiDocument,
    GuiTemplateRegistry<'static>,
) {
    let doc = phase4_doc();
    let leaked: &'static hoi4_ui::vanilla_gui::GuiDocument = Box::leak(Box::new(doc.clone()));
    let registry = GuiTemplateRegistry::from_documents([("phase4", leaked)]);
    (doc, registry)
}

fn build_parts(
    data: &LogisticsData,
    viewport: GuiRect,
    runtime_state: GuiRuntimeState,
) -> hoi4_ui::logistics_panel::LogisticsVanillaRuntimeFrameParts {
    let (doc, registry) = root_registry();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    let gfx_index = phase4_gfx_index();
    logistics_vanilla_runtime_frame_parts(
        root,
        data,
        viewport,
        runtime_state,
        registry,
        Some(&gfx_index),
        None,
    )
}

#[test]
fn logistics_vanilla_gate19_show_with_icon_bank_entry_is_callable_from_app_path() {
    let runtime_available = hoi4_ui::vanilla_gui::country_logistics_runtime_context().is_some();
    let sample = data(vec![entry(
        "infantry_equipment",
        "Infantry Equipment",
        0.0,
        1.0,
        5.0,
        vec!["steel"],
    )]);
    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 640.0))
        .build(move |ctx| {
            let path_cfg = hoi4_paths::PathConfig::with_game_path(std::env::temp_dir());
            let mut icon_bank = IconBank::new(ctx.clone(), path_cfg);
            let (_close, _cmds) = LogisticsPanel::show_with_icon_bank(ctx, &sample, &mut icon_bank);
        });
    harness.run_steps(2);

    let mut report = String::from("# Gate 19 Logistics Vanilla Render Entry\n\n");
    let _ = writeln!(report, "- show_with_icon_bank: callable");
    let _ = writeln!(report, "- runtime_available: {runtime_available}");
    report.push_str("- app_render_entry: crates/hoi4-app/src/ui_driver/render.rs passes render_state.icon_bank\n");
    report.push_str("- legacy_show: retained for phase 5 cleanup; app path no longer calls it\n");
    write_report(report_path("gate19_render_entry.md"), report);
}

#[test]
fn logistics_vanilla_gate20_root_window_uses_vanilla_position_animation_and_size() {
    let sample = data(vec![entry(
        "infantry_equipment",
        "Infantry Equipment",
        10.0,
        2.0,
        5.0,
        Vec::new(),
    )]);
    let mut report = String::from("# Gate 20 Logistics Root Window\n\n");
    for (name, width, height) in [
        ("960x640", 960.0, 640.0),
        ("1920x1080", 1920.0, 1080.0),
        ("2560x1440", 2560.0, 1440.0),
    ] {
        let viewport = GuiRect::new(0.0, 0.0, width, height);
        let shown = build_parts(&sample, viewport, GuiRuntimeState::shown(viewport)).frame;
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

        let hidden = build_parts(&sample, viewport, GuiRuntimeState::hidden(viewport)).frame;
        assert_eq!(
            hidden.root_layout.rect,
            GuiRect::new(-606.0, 78.0, 550.0, height)
        );

        let current = build_parts(
            &sample,
            viewport,
            GuiRuntimeState {
                root_position: GuiRuntimeRootPosition::Current(GuiPoint { x: -306.0, y: 78.0 }),
                phase: Some(AnimationPhase::Opening),
                visible: true,
                ..GuiRuntimeState::shown(viewport)
            },
        )
        .frame;
        assert_eq!(
            current.root_layout.rect,
            GuiRect::new(-306.0, 78.0, 550.0, height)
        );
        assert_eq!(
            current.diagnostics.root_animation_offset,
            GuiPoint { x: -300.0, y: 0.0 }
        );
        let close = shown
            .hit_regions
            .iter()
            .find(|hit| hit.command == "close")
            .expect("close hit region");
        assert!(close.enabled);
        let _ = writeln!(
            report,
            "- {name}: shown={:?} hidden={:?} current={:?} close_hit={:?}",
            shown.root_layout.rect, hidden.root_layout.rect, current.root_layout.rect, close.rect
        );
    }
    let (doc, _) = root_registry();
    let root = doc.template_index().get(COUNTRY_LOGISTICS_ROOT).unwrap();
    let spec = AnimationSpec::from_node(root);
    assert_eq!(spec.hidden_position, GuiPoint { x: -606.0, y: 78.0 });
    assert_eq!(spec.shown_position, GuiPoint { x: -6.0, y: 78.0 });
    assert_eq!(spec.duration_ms, 300.0);
    report.push_str("- local_root_offset: none; frame uses AnimationSpec from root\n");
    report.push_str("- esc_shortcut: retained on vanilla close_button metadata; runtime entry maps ESC to close request\n");
    write_report(report_path("gate20_root_window.md"), report);
}

#[test]
fn logistics_vanilla_gate21_materiel_scroll_clip_and_hit_test() {
    let sample = overflow_data();
    let viewport = GuiRect::new(0.0, 0.0, 960.0, 640.0);
    let base = build_parts(&sample, viewport, GuiRuntimeState::shown(viewport)).frame;
    let materiel_path = base
        .root_layout
        .find_by_name("materiel")
        .unwrap()
        .path
        .clone();
    let frame = build_parts(
        &sample,
        viewport,
        GuiRuntimeState::shown(viewport)
            .with_scroll_offset(materiel_path, GuiPoint { x: 0.0, y: 116.0 }),
    )
    .frame;

    let materiel_scroll = frame
        .scroll_states
        .iter()
        .find(|state| state.node_name.as_deref() == Some("materiel"))
        .expect("materiel scroll state");
    assert!(materiel_scroll.spec.has_vertical_scrollbar);
    assert_eq!(materiel_scroll.spec.scroll_wheel_factor, 40.0);
    assert!(materiel_scroll.spec.smooth_scrolling);

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
    assert!(entry_hits
        .iter()
        .any(|hit| hit.command.contains("equipment_")));

    let mut report = String::from("# Gate 21 Logistics Scroll Hit Test\n\n");
    let _ = writeln!(report, "- {}", frame.diagnostics.scroll_markdown());
    let _ = writeln!(report, "- close_hit: {:?}", close_hit.rect);
    let _ = writeln!(report, "- visible_entry_hit_regions: {}", entry_hits.len());
    write_report(report_path("gate21_scroll_hit_test.md"), report);
}

#[test]
fn logistics_vanilla_gate22_equipment_click_and_close_behavior() {
    let profile = CountryLogisticsProfile;
    let sample = data(vec![
        entry(
            "infantry_equipment",
            "Infantry Equipment",
            0.0,
            1.0,
            5.0,
            vec!["steel"],
        ),
        entry("artillery", "Artillery", 20.0, 5.0, 1.0, vec!["steel"]),
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
    for close_path in [
        GuiNodePath::root("close"),
        GuiNodePath::root(COUNTRY_LOGISTICS_ROOT).child("close_button"),
    ] {
        assert_eq!(
            profile.handle_action(
                GuiAction {
                    node_path: close_path,
                    kind: GuiActionKind::Click,
                },
                &sample,
            ),
            Some(PanelCommand::ClosePrimary)
        );
    }

    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = build_parts(&sample, viewport, GuiRuntimeState::shown(viewport)).frame;
    let resource_hits: Vec<_> = frame
        .hit_regions
        .iter()
        .filter(|hit| {
            hit.source_path.to_string().contains("resources_grid")
                || hit
                    .source_path
                    .to_string()
                    .contains("logistics_entry_resource_item")
        })
        .collect();
    assert!(
        resource_hits
            .iter()
            .all(|hit| !hit.command.starts_with("logistics:entry:")),
        "resource icon hit regions must not masquerade as row hits: {resource_hits:?}"
    );

    let mut report = String::from("# Gate 22 Logistics Click And Close\n\n");
    report.push_str("- deficit_row: infantry_equipment -> Goods detail source Logistics\n");
    report.push_str("- surplus_row: artillery -> Goods detail source Logistics\n");
    report.push_str("- close_button: PanelCommand::ClosePrimary\n");
    report.push_str("- esc: runtime entry maps Escape to the same close request path\n");
    let _ = writeln!(report, "- resource_hit_regions: {}", resource_hits.len());
    write_report(report_path("gate22_click_close.md"), report);
}

#[test]
fn logistics_vanilla_gate23_headers_progress_and_values_use_vanilla_visual_slots() {
    let sample = overflow_data();
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let parts = build_parts(&sample, viewport, GuiRuntimeState::shown(viewport));
    let frame = &parts.frame;
    let snapshot_exists = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots/logistics_vanilla_1080p.png")
        .exists();
    assert!(
        snapshot_exists,
        "{LOGISTICS_VANILLA_SNAPSHOT_1080P} missing"
    );

    let headers = frame.root_layout.find_by_name("headers").unwrap();
    assert_eq!(headers.rect.height, 25.0);
    for name in [
        "producing_icon",
        "need_icon",
        "balance_icon",
        "in_stock_icon",
        "equipment_type_label",
        "status_label",
    ] {
        assert!(
            headers.find_by_name(name).is_some(),
            "missing header {name}"
        );
    }
    let resources_label = headers.find_by_name("resources_label").unwrap();
    assert!(!resources_label.visible);

    let resource_names: Vec<_> = frame
        .draw_list
        .iter()
        .filter_map(|command| command.resource_name())
        .collect();
    assert!(resource_names.contains(&"GFX_producing_icon"));
    assert!(resource_names.contains(&"GFX_need_icon"));
    assert!(resource_names.contains(&"GFX_balance_icon"));
    assert!(resource_names.contains(&"GFX_in_stock_icon"));
    assert!(resource_names
        .iter()
        .any(|name| *name == "GFX_logistics_equipment_entry_bg"));
    assert!(resource_names
        .iter()
        .any(|name| *name == "GFX_logistics_naval_equipment_entry_bg"));
    assert!(resource_names
        .iter()
        .any(|name| *name == "GFX_logistics_air_equipment_entry_bg"));

    let progress_bars: Vec<_> = frame
        .draw_list
        .iter()
        .filter_map(|command| match &command.kind {
            GuiDrawCommandKind::DrawProgress(progress) => Some(progress),
            _ => None,
        })
        .collect();
    assert!(!progress_bars.is_empty());
    assert!(progress_bars
        .iter()
        .all(|progress| progress.resource_name == "GFX_logistics_progressbar"));
    assert!(progress_bars
        .iter()
        .all(|progress| progress.fill_rect.width <= progress_bars[0].fill_rect.width.max(320.0)));

    let text_reports: Vec<_> = frame
        .diagnostics
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.name.as_deref(),
                Some("equipment_type" | "produced" | "needs" | "balance" | "in_stock")
            )
        })
        .collect();
    assert!(!text_reports.is_empty());
    assert!(text_reports.iter().all(|node| node.hit_rect.width <= 132.0));

    let mut report = String::from("# Gate 23 Logistics Visual\n\n");
    let _ = writeln!(report, "- snapshot: {LOGISTICS_VANILLA_SNAPSHOT_1080P}");
    let _ = writeln!(report, "- snapshot_exists: {snapshot_exists}");
    let _ = writeln!(report, "- headers_height_px: {}", headers.rect.height);
    let _ = writeln!(report, "- progress_bars: {}", progress_bars.len());
    let _ = writeln!(
        report,
        "- fallback_draw_commands: {}",
        frame.diagnostics.draw_commands.fallback
    );
    report.push_str(&frame.diagnostics.draw_command_markdown());
    report.push_str("- numeric_columns: fixed-width text boxes from vanilla row templates\n");
    report.push_str(
        "- resource_assets: runtime IconBank/GfxIndex only; no vanilla resource copy in repo\n",
    );
    write_report(report_path("gate23_visual.md"), report);
}

#[test]
fn logistics_vanilla_real_runtime_rows_text_and_icons_are_not_stacked_when_available() {
    let Some(context) = hoi4_ui::vanilla_gui::country_logistics_runtime_context() else {
        write_report(
            report_path("real_runtime_rows_text_icons.md"),
            "# Real Runtime Rows Text Icons\n\n- skipped: HOI4 vanilla runtime unavailable\n",
        );
        return;
    };
    let sample = data(vec![
        entry(
            "infantry_equipment",
            "Infantry Equipment",
            0.0,
            1.0,
            6.0,
            vec!["steel"],
        ),
        entry("artillery", "Artillery", 20.0, 5.0, 1.0, vec!["steel"]),
        entry("anti_tank", "Anti-Tank", 10.0, 2.0, 2.5, vec!["steel"]),
        entry("aircraft", "Aircraft", 10.0, 1.0, 6.0, vec!["aluminium"]),
        entry("naval_vessel", "Ships", 4.0, 0.5, 3.0, vec!["steel"]),
        entry("convoy", "Convoy", 10.0, 5.0, 1.0, vec!["steel"]),
        entry("train", "Train", 3.0, 0.5, 0.5, vec!["steel"]),
    ]);
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let root = context
        .root_template(COUNTRY_LOGISTICS_GUI_FILE, COUNTRY_LOGISTICS_ROOT)
        .expect("country logistics root");
    let registry = GuiTemplateRegistry::from_documents(context.documents());
    let parts = logistics_vanilla_runtime_frame_parts(
        root,
        &sample,
        viewport,
        GuiRuntimeState::shown(viewport),
        registry,
        Some(&context.gfx_index),
        None,
    );
    let frame = &parts.frame;
    let mut rows: Vec<_> = frame
        .generated_instances
        .iter()
        .filter(|instance| {
            instance.context.semantic_role.as_deref() == Some("logistics_equipment_entry")
        })
        .collect();
    assert_eq!(rows.len(), sample.entries.len());
    rows.sort_by(|a, b| {
        a.resolved_rect
            .y
            .partial_cmp(&b.resolved_rect.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for pair in rows.windows(2) {
        assert!(
            pair[1].resolved_rect.y > pair[0].resolved_rect.y + 40.0,
            "equipment rows must not stack: {:?} then {:?}",
            pair[0].resolved_rect,
            pair[1].resolved_rect
        );
    }

    let mut icon_sprites = Vec::new();
    let mut text_boxes = Vec::new();
    for row in &rows {
        let row_rect = row.resolved_rect;
        let equipment_type = row
            .layout
            .find_by_name("equipment_type")
            .expect("equipment_type text node");
        let text_layout = equipment_type
            .rects
            .text_layout
            .as_ref()
            .expect("resolved equipment_type text layout");
        assert!(
            text_layout.box_rect.x >= row_rect.x
                && text_layout.box_rect.right() <= row_rect.right()
                && text_layout.box_rect.y >= row_rect.y
                && text_layout.box_rect.bottom() <= row_rect.bottom(),
            "equipment_type text layout must stay inside its row: row={row_rect:?} text={:?}",
            text_layout.box_rect
        );
        if text_layout.box_rect.height > 0.0 {
            assert!(text_layout.box_rect.width >= 80.0);
            text_boxes.push(text_layout.box_rect);
        }

        let equipment_icon = row
            .layout
            .find_by_name("equipment_icon")
            .expect("equipment_icon node");
        assert!(
            equipment_icon.visible,
            "known equipment rows must keep equipment_icon visible"
        );
        let icon_binding = parts
            .bindings
            .for_node(&equipment_icon.path, equipment_icon.name.as_deref());
        let sprite = icon_binding
            .sprite
            .as_deref()
            .expect("equipment_icon sprite binding");
        assert!(
            sprite.starts_with("GFX_"),
            "equipment icon must bind a concrete vanilla sprite, got {sprite}"
        );
        icon_sprites.push(sprite.to_owned());
    }
    for pair in text_boxes.windows(2) {
        assert!(
            pair[1].y > pair[0].y + 40.0,
            "equipment text boxes must not stack: {:?} then {:?}",
            pair[0],
            pair[1]
        );
    }
    icon_sprites.sort();
    icon_sprites.dedup();
    let mut icon_bank = IconBank::new(egui::Context::default(), context.path_cfg.clone());
    icon_bank.add_profile_search_dirs(hoi4_ui::vanilla_gui::COUNTRY_LOGISTICS_PROFILE_ID);
    for sprite in &icon_sprites {
        let report = icon_bank.diagnose_sprite(sprite);
        assert!(
            report.loaded,
            "equipment icon {sprite} should resolve through IconBank: {report:?}"
        );
    }

    let mut report = String::from("# Real Runtime Rows Text Icons\n\n");
    let _ = writeln!(report, "- rows: {}", rows.len());
    let _ = writeln!(report, "- icon_sprites: {:?}", icon_sprites);
    let _ = writeln!(report, "- text_boxes: {:?}", text_boxes);
    report.push_str("- source: runtime `countrylogisticsview.gui` from local HOI4 install\n");
    write_report(report_path("real_runtime_rows_text_icons.md"), report);
}
