use std::fmt::Write as _;
use std::path::PathBuf;

use hoi4_ui::vanilla_gui::{
    parse_gui_str, rect_is_physical_pixel_aligned, resolve_text_layout,
    snap_rect_to_physical_pixels, FontToken, GfxIndex, GuiBindingMap, GuiControlRects, GuiRect,
    GuiRuntimeFrame, GuiRuntimeFrameInput, GuiRuntimeState, GuiTextHorizontalAlign,
    GuiTextVerticalAlign, VanillaGuiRuntimeContext, COUNTRY_POLITICS_DESCRIPTOR,
    COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_PROFILE_ID, COUNTRY_POLITICS_ROOT,
};

fn report_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/clausewitz_gui")
        .join(name)
}

fn write_report(name: &str, body: impl AsRef<str>) {
    let path = report_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

fn phase2_fixture_gfx() -> GfxIndex {
    GfxIndex::parse_single(
        "phase2.gfx",
        r#"
spriteTypes = {
    spriteType = {
        name = "GFX_add_national_goal_button"
        size = { x = 259 y = 83 }
        noOfFrames = 3
        texturefile = "gfx/interface/add_national_goal_button.dds"
    }
    spriteType = {
        name = "GFX_closebutton"
        size = { x = 29 y = 29 }
        noOfFrames = 4
    }
    spriteType = {
        name = "GFX_portrait_GER_adolf_hitler"
        size = { x = 156 y = 210 }
    }
    frameAnimatedSpriteType = {
        name = "GFX_goal_anim"
        size = { x = 94 y = 76 }
        noOfFrames = 4
    }
    progressbarType = {
        name = "GFX_activegoal_progress"
        size = { x = 237 y = 6 }
        textureFile1 = "gfx/interface/progress.dds"
    }
    pieChartType = {
        name = "GFX_political_chart"
        size = 27
    }
}
"#,
    )
}

fn phase2_fixture_root() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        None,
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrypoliticsview"
        position = { x = -606 y = 78 }
        show_position = { x = -6 y = 78 }
        size = { width = 550 height = 100%% }
        containerWindowType = {
            name = "active_goal"
            position = { x = 173 y = 126 }
            size = { width = 330 height = 50 }
            buttonType = {
                name = "add_national_goal_button"
                position = { x = 100 y = 7 }
                spriteType = "GFX_add_national_goal_button"
            }
            iconType = {
                name = "goal_icon"
                position = { x = 95 y = 25 }
                centerposition = yes
                spriteType = "GFX_goal_anim"
            }
            iconType = {
                name = "missing_icon"
                position = { x = 10 y = 10 }
                spriteType = "GFX_missing_phase2"
            }
        }
        iconType = {
            name = "leader"
            position = { x = 22 y = 58 }
        }
        iconType = {
            name = "political_pie_chart"
            position = { x = 292 y = 387 }
            spriteType = "GFX_political_chart"
        }
        buttonType = {
            name = "close_button"
            position = { x = -42 y = 9 }
            Orientation = "UPPER_RIGHT"
            quadTextureSprite = "GFX_closebutton"
        }
        instantTextboxType = {
            name = "ideology"
            position = { x = 18 y = 260 }
            maxWidth = 96
            maxHeight = 22
            format = center
            vertical_alignment = center
            font = "hoi_14mbs"
            text = "ideology"
        }
        instantTextboxType = {
            name = "elections"
            position = { x = 18 y = 286 }
            maxWidth = 96
            maxHeight = 18
            format = right
            font = "hoi_12mbs"
            text = "elections"
        }
    }
}
"#,
    )
}

#[test]
fn gate8_gfx_intrinsic_resolver_covers_button_pie_and_missing_fallback() {
    let gfx = phase2_fixture_gfx();
    let doc = phase2_fixture_root();
    let root = doc.template_index().get("countrypoliticsview").unwrap();
    let intrinsics = hoi4_ui::vanilla_gui::GuiLayoutIntrinsics::from_gfx_index_for_tree(&gfx, root);

    let add = intrinsics.resource("GFX_add_national_goal_button").unwrap();
    assert_eq!(add.size.width, 259.0);
    assert_eq!(add.size.height, 83.0);
    assert_eq!(add.frame_count, Some(3));

    let pie = intrinsics.resource("GFX_political_chart").unwrap();
    assert_eq!(pie.size.width, 54.0);
    assert_eq!(pie.size.height, 54.0);

    let missing = intrinsics.resource("GFX_missing_phase2").unwrap();
    assert_eq!(missing.size.width, 1.0);
    assert_eq!(missing.size.height, 1.0);
    assert!(intrinsics
        .diagnostics()
        .iter()
        .any(|diag| diag.detail.contains("GFX_missing_phase2")));

    let mut report = String::new();
    let _ = writeln!(report, "# Gate 8 Intrinsic Resolver");
    for diagnostic in intrinsics.diagnostics() {
        let _ = writeln!(
            report,
            "- [{}] {}: {}",
            diagnostic.source.as_str(),
            diagnostic.resource_name,
            diagnostic.detail
        );
    }
    write_report("gate8_intrinsic_size_resolver.md", report);
}

#[test]
fn gate9_two_pass_layout_fills_missing_size_and_centerposition() {
    let gfx = phase2_fixture_gfx();
    let doc = phase2_fixture_root();
    let root = doc.template_index().get("countrypoliticsview").unwrap();
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let mut bindings = GuiBindingMap::default();
    bindings.insert_name(
        "leader",
        hoi4_ui::vanilla_gui::GuiBinding::default().sprite("GFX_portrait_GER_adolf_hitler"),
    );
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(root, viewport, COUNTRY_POLITICS_PROFILE_ID, &bindings)
            .with_gfx_index(&gfx)
            .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );

    let button = frame
        .root_layout
        .find_by_name("add_national_goal_button")
        .unwrap();
    assert_eq!(button.rect.width, 259.0);
    assert_eq!(button.rect.height, 83.0);

    let goal_icon = frame.root_layout.find_by_name("goal_icon").unwrap();
    assert_eq!(goal_icon.rect.width, 94.0);
    assert_eq!(goal_icon.rect.height, 76.0);
    assert_eq!(goal_icon.rect.x, -6.0 + 173.0 + 95.0 - 47.0);
    assert_eq!(goal_icon.rect.y, 78.0 + 126.0 + 25.0 - 38.0);

    let close = frame.root_layout.find_by_name("close_button").unwrap();
    assert_eq!(close.rect.width, 29.0);
    assert_eq!(close.rect.height, 29.0);

    let leader = frame.root_layout.find_by_name("leader").unwrap();
    assert_eq!(leader.rect.width, 156.0);
    assert_eq!(leader.rect.height, 210.0);
    assert_eq!(
        leader
            .intrinsic_size
            .as_ref()
            .map(|size| size.resource_name.as_str()),
        Some("GFX_portrait_GER_adolf_hitler")
    );

    write_report(
        "gate9_two_pass_layout.md",
        frame.transform_stack_markdown_for_name("add_national_goal_button"),
    );
}

#[test]
fn gate10_control_specific_rects_separate_layout_paint_hit_and_text_rects() {
    let gfx = phase2_fixture_gfx();
    let doc = phase2_fixture_root();
    let root = doc.template_index().get("countrypoliticsview").unwrap();
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            viewport,
            COUNTRY_POLITICS_PROFILE_ID,
            &GuiBindingMap::default(),
        )
        .with_gfx_index(&gfx)
        .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );

    let button = frame
        .root_layout
        .find_by_name("add_national_goal_button")
        .unwrap();
    assert_eq!(button.rects.layout_rect, button.rect);
    assert_eq!(button.rects.visual_rect, button.rects.paint_rect);
    assert!(button.rects.hit_rect.width >= button.rect.width);
    assert_eq!(button.rects.resource_rect, Some(button.rects.paint_rect));

    let ideology = frame.root_layout.find_by_name("ideology").unwrap();
    assert_eq!(ideology.rects.text_rect.unwrap().width, 96.0);
    assert_eq!(ideology.rects.text_rect.unwrap().height, 22.0);

    let rects = GuiControlRects::resolve(
        root.find_node_by_name("add_national_goal_button").unwrap(),
        button.rect,
        button.intrinsic_size.as_ref(),
        button.scale,
        1.25,
    );
    assert!(rect_is_physical_pixel_aligned(rects.hit_rect, 1.25));

    let mut report = String::new();
    let _ = writeln!(report, "# Gate 10 Control Rects");
    let _ = writeln!(
        report,
        "{}",
        frame.transform_stack_markdown_for_name("add_national_goal_button")
    );
    let _ = writeln!(
        report,
        "{}",
        frame.transform_stack_markdown_for_name("ideology")
    );
    write_report("gate10_control_rects.md", report);
}

#[test]
fn gate11_font_metrics_cover_format_and_vertical_alignment() {
    let doc = phase2_fixture_root();
    let root = doc.template_index().get("countrypoliticsview").unwrap();
    let ideology = root.find_node_by_name("ideology").unwrap();
    let elections = root.find_node_by_name("elections").unwrap();

    let ideology_layout = resolve_text_layout(ideology, GuiRect::new(12.0, 338.0, 0.0, 0.0), 1.0);
    assert_eq!(ideology_layout.horizontal, GuiTextHorizontalAlign::Center);
    assert_eq!(ideology_layout.vertical, GuiTextVerticalAlign::Center);
    assert_eq!(ideology_layout.box_rect.width, 96.0);
    assert_eq!(ideology_layout.box_rect.height, 22.0);
    assert_eq!(ideology_layout.font_token, FontToken::Caption);

    let elections_layout = resolve_text_layout(elections, GuiRect::new(12.0, 364.0, 0.0, 0.0), 1.0);
    assert_eq!(elections_layout.horizontal, GuiTextHorizontalAlign::Right);
    assert_eq!(elections_layout.vertical, GuiTextVerticalAlign::Top);
    assert_eq!(elections_layout.font_token, FontToken::Small);

    let mut report = String::new();
    let _ = writeln!(report, "# Gate 11 Font Metrics");
    let _ = writeln!(report, "- ideology: {:?}", ideology_layout);
    let _ = writeln!(report, "- elections: {:?}", elections_layout);
    write_report("gate11_font_metrics.md", report);
}

#[test]
fn gate12_pixel_snapping_reports_alignment_across_dpi_values() {
    let raw = GuiRect::new(100.2, 7.2, 277.4, 82.4);
    let mut report = String::new();
    let _ = writeln!(report, "# Gate 12 Pixel Snapping");
    for pixels_per_point in [1.0, 1.25, 1.5, 2.0] {
        let snapped = snap_rect_to_physical_pixels(
            raw,
            pixels_per_point,
            hoi4_ui::vanilla_gui::GuiPixelSnapMode::Round,
        );
        assert!(
            rect_is_physical_pixel_aligned(snapped, pixels_per_point),
            "ppp={pixels_per_point} snapped={snapped:?}"
        );
        let _ = writeln!(
            report,
            "- ppp={pixels_per_point:.2}: ({:.3}, {:.3}, {:.3}, {:.3})",
            snapped.x, snapped.y, snapped.width, snapped.height
        );
    }
    write_report("gate12_pixel_snapping.md", report);
}

#[test]
fn phase2_real_politics_intrinsic_nodes_when_vanilla_available() {
    let Some(context) =
        VanillaGuiRuntimeContext::load(COUNTRY_POLITICS_DESCRIPTOR.required_gui_files)
    else {
        return;
    };
    let Some(root) = context.root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT) else {
        return;
    };
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            viewport,
            COUNTRY_POLITICS_PROFILE_ID,
            &GuiBindingMap::default(),
        )
        .with_gfx_index(&context.gfx_index)
        .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );

    let add = frame
        .root_layout
        .find_by_name("add_national_goal_button")
        .unwrap();
    assert!(add.rect.width > 0.0);
    assert!(add.rect.height > 0.0);
    let chart_intrinsic = frame
        .diagnostics
        .intrinsics
        .resource("GFX_political_chart")
        .unwrap();
    assert_eq!(chart_intrinsic.size.width, 54.0);
    assert_eq!(chart_intrinsic.size.height, 54.0);

    let mut report = String::new();
    let _ = writeln!(report, "{}", frame.diagnostics.intrinsic_markdown());
    let _ = writeln!(
        report,
        "{}",
        frame.transform_stack_markdown_for_name("ideology")
    );
    let _ = writeln!(
        report,
        "{}",
        frame.transform_stack_markdown_for_name("elections")
    );
    write_report("gate11_politics_text_diagnostics.md", report);
}

#[test]
fn phase2_real_politics_iconbank_intrinsics_keep_key_sprites_visible_when_vanilla_available() {
    let Some(context) =
        VanillaGuiRuntimeContext::load(COUNTRY_POLITICS_DESCRIPTOR.required_gui_files)
    else {
        return;
    };
    let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
        return;
    };
    if path_cfg.find(COUNTRY_POLITICS_GUI_FILE).is_none() {
        return;
    }
    let Some(root) = context.root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT) else {
        return;
    };

    let ctx = egui::Context::default();
    let mut icon_bank = hoi4_ui::icons::IconBank::new(ctx, path_cfg);
    icon_bank.add_politics_search_dirs();
    icon_bank.add_leader_dirs(["GER"]);
    assert!(icon_bank
        .get_or_load("GFX_portrait_GER_adolf_hitler")
        .is_some());
    assert!(icon_bank.get_or_load("GFX_closebutton").is_some());
    assert!(icon_bank
        .get_or_load("GFX_add_national_goal_button")
        .is_some());

    let mut bindings = GuiBindingMap::default();
    bindings.insert_name(
        "leader",
        hoi4_ui::vanilla_gui::GuiBinding::default().sprite("GFX_portrait_GER_adolf_hitler"),
    );
    bindings.insert_name(
        "close_button",
        hoi4_ui::vanilla_gui::GuiBinding::default().click("close"),
    );
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(root, viewport, COUNTRY_POLITICS_PROFILE_ID, &bindings)
            .with_gfx_index(&context.gfx_index)
            .with_icon_bank(&mut icon_bank)
            .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );

    for (node_name, resource_name, min_w, min_h) in [
        ("leader", "GFX_portrait_GER_adolf_hitler", 80.0, 100.0),
        ("close_button", "GFX_closebutton", 20.0, 20.0),
        (
            "add_national_goal_button",
            "GFX_add_national_goal_button",
            40.0,
            20.0,
        ),
    ] {
        let node = frame
            .root_layout
            .find_by_name(node_name)
            .unwrap_or_else(|| panic!("missing politics node {node_name}"));
        assert!(
            node.rect.width >= min_w && node.rect.height >= min_h,
            "{node_name} collapsed after intrinsic resolution: {:?}",
            node.rect
        );
        assert!(
            icon_bank.get_or_load(resource_name).is_some(),
            "{resource_name} should load through IconBank fallback/search paths"
        );
    }

    let mut report = String::new();
    let _ = writeln!(
        report,
        "{}",
        frame.transform_stack_markdown_for_name("leader")
    );
    let _ = writeln!(
        report,
        "{}",
        frame.transform_stack_markdown_for_name("close_button")
    );
    let _ = writeln!(
        report,
        "{}",
        frame.transform_stack_markdown_for_name("add_national_goal_button")
    );
    write_report("gate12_key_sprite_intrinsic_regression.md", report);
}

#[test]
fn phase2_iconbank_intrinsics_use_cached_sizes_without_first_open_load() {
    let Some(context) =
        VanillaGuiRuntimeContext::load(COUNTRY_POLITICS_DESCRIPTOR.required_gui_files)
    else {
        return;
    };
    let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
        return;
    };
    if path_cfg.find(COUNTRY_POLITICS_GUI_FILE).is_none() {
        return;
    }
    let Some(root) = context.root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT) else {
        return;
    };

    let ctx = egui::Context::default();
    let mut icon_bank = hoi4_ui::icons::IconBank::new(ctx, path_cfg);
    icon_bank.add_politics_search_dirs();
    icon_bank.add_leader_dirs(["GER"]);

    let mut bindings = GuiBindingMap::default();
    bindings.insert_name(
        "leader",
        hoi4_ui::vanilla_gui::GuiBinding::default().sprite("GFX_portrait_GER_adolf_hitler"),
    );
    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let _frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(root, viewport, COUNTRY_POLITICS_PROFILE_ID, &bindings)
            .with_gfx_index(&context.gfx_index)
            .with_icon_bank(&mut icon_bank)
            .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );

    assert_eq!(
        icon_bank.size_of("GFX_portrait_GER_adolf_hitler"),
        None,
        "layout intrinsics must not decode uncached dynamic portraits on first open"
    );
}
