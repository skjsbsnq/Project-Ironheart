use std::path::PathBuf;

use hoi4_ui::vanilla_gui::{
    parse_gui_str, GuiBinding, GuiBindingMap, GuiNodePath, GuiPoint, GuiRect, GuiRuntimeFrame,
    GuiRuntimeFrameInput, GuiRuntimeInstanceSpec, GuiRuntimeState, GuiTemplateRegistry,
    TemplateInstanceOptions,
};

const GUI_FILE: &str = "interface/countrydecisionview.gui";

#[test]
fn gate18_runtime_collects_stable_scroll_state_from_vanilla_properties() {
    let doc = phase4_document();
    let root = doc.template_index().get("countrydecisionview").unwrap();
    let bindings = GuiBindingMap::default();
    let grid_path = GuiNodePath::root("countrydecisionview").child("decision_grid_container");
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 800.0, 600.0),
            "country_decisions",
            &bindings,
        )
        .with_runtime_state(
            GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 800.0, 600.0))
                .with_scroll_offset(grid_path.clone(), GuiPoint { x: 0.0, y: 32.0 }),
        ),
    );

    assert_eq!(frame.scroll_states.len(), 1);
    let state = &frame.scroll_states[0];
    assert_eq!(
        state.key.to_string(),
        "country_decisions:countrydecisionview.decision_grid_container"
    );
    assert_eq!(state.path, grid_path);
    assert_eq!(state.offset, GuiPoint { x: 0.0, y: 32.0 });
    assert_eq!(state.spec.scroll_wheel_factor, 2.5);
    assert!(state.spec.smooth_scrolling);
    assert_eq!(state.spec.margin.top, 8.0);
    assert_eq!(state.spec.margin.right, 12.0);
    assert!(frame
        .diagnostics
        .scroll_markdown()
        .contains("wheel_factor=2.50"));
}

#[test]
fn gate19_scroll_transform_and_clip_are_shared_by_layout_instances_and_hit_regions() {
    let doc = phase4_document();
    let root = doc.template_index().get("countrydecisionview").unwrap();
    let registry = GuiTemplateRegistry::from_documents([(GUI_FILE, &doc)]);
    let grid_path = GuiNodePath::root("countrydecisionview").child("decision_grid_container");
    let item_path = grid_path.child("decision_item[0]");
    let button_path = item_path.child("btn_select");
    let mut bindings = GuiBindingMap::default();
    bindings.insert_path(grid_path.clone(), GuiBinding::default().instances(3));
    bindings.insert_path(
        button_path.clone(),
        GuiBinding::default()
            .click("decision:activate:first")
            .tooltip("Activate first decision"),
    );
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 800.0, 600.0),
            "country_decisions",
            &bindings,
        )
        .with_template_registry(registry)
        .with_runtime_state(
            GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 800.0, 600.0))
                .with_scroll_offset(grid_path.clone(), GuiPoint { x: 0.0, y: 32.0 }),
        )
        .add_instance_spec(
            GuiRuntimeInstanceSpec::grid("decision_item", grid_path.clone(), 3)
                .with_options(TemplateInstanceOptions::default().template_size(true))
                .with_semantic_role("decision_entry"),
        ),
    );

    let instance = frame
        .generated_instances
        .iter()
        .find(|instance| instance.path == item_path)
        .unwrap();
    assert_eq!(instance.slot_rect, GuiRect::new(20.0, 0.0, 300.0, 48.0));
    assert_eq!(
        instance.resolved_rect,
        GuiRect::new(20.0, -32.0, 300.0, 48.0)
    );
    let button = instance.layout.find_by_name("btn_select").unwrap();
    assert_eq!(button.clip_rect, GuiRect::new(20.0, 8.0, 288.0, 8.0));
    assert_eq!(button.rects.hit_rect, GuiRect::new(20.0, 8.0, 288.0, 8.0));

    let hit = frame
        .hit_regions
        .iter()
        .find(|region| region.source_path == button_path)
        .unwrap();
    assert_eq!(hit.rect, GuiRect::new(20.0, 8.0, 288.0, 8.0));
    assert_eq!(hit.command, "decision:activate:first");
    assert_eq!(hit.tooltip.as_deref(), Some("Activate first decision"));
}

#[test]
fn gate20_runtime_hit_regions_include_enabled_command_tooltip_source_and_z_order() {
    let doc = parse_gui_str(
        Some(PathBuf::from(GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 180 height = 80 }
        buttonType = {
            name = "back"
            size = { width = 100 height = 50 }
        }
        buttonType = {
            name = "front"
            position = { x = 20 y = 10 }
            size = { width = 100 height = 50 }
        }
    }
}
"#,
    );
    let root = doc.template_index().get("root").unwrap();
    let mut bindings = GuiBindingMap::default();
    bindings.insert_name(
        "back",
        GuiBinding::default()
            .click("back")
            .tooltip("Back")
            .enabled(false),
    );
    bindings.insert_name("front", GuiBinding::default().click("front"));
    let frame = GuiRuntimeFrame::build(GuiRuntimeFrameInput::new(
        root,
        GuiRect::new(0.0, 0.0, 180.0, 80.0),
        "hit_test",
        &bindings,
    ));

    assert_eq!(frame.hit_regions.len(), 2);
    assert_eq!(frame.hit_regions[0].source_path.to_string(), "root.back");
    assert_eq!(frame.hit_regions[0].command, "back");
    assert!(!frame.hit_regions[0].enabled);
    assert_eq!(frame.hit_regions[0].tooltip.as_deref(), Some("Back"));
    assert_eq!(frame.hit_regions[1].source_path.to_string(), "root.front");
    assert_eq!(frame.hit_regions[1].command, "front");
    assert!(frame.hit_regions[1].z_index > frame.hit_regions[0].z_index);
}

#[test]
fn gate21_clipping_no_children_can_draw_outside_parent_but_scroll_clip_still_clips() {
    let doc = parse_gui_str(
        Some(PathBuf::from(GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 200 height = 140 }
        containerWindowType = {
            name = "active_goal"
            position = { x = 20 y = 20 }
            size = { width = 40 height = 30 }
            clipping = no
            buttonType = {
                name = "overflow_child"
                position = { x = 30 y = 0 }
                size = { width = 60 height = 30 }
            }
        }
        containerWindowType = {
            name = "scroll_box"
            position = { x = 20 y = 70 }
            size = { width = 80 height = 40 }
            verticalScrollbar = "right"
            margin = { top = 0 left = 0 bottom = 0 right = 10 }
            buttonType = {
                name = "scroll_child"
                position = { x = 60 y = 0 }
                size = { width = 50 height = 30 }
            }
        }
    }
}
"#,
    );
    let root = doc.template_index().get("root").unwrap();
    let scroll_path = GuiNodePath::root("root").child("scroll_box");
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 200.0, 140.0),
            "clipping_matrix",
            &GuiBindingMap::default(),
        )
        .with_runtime_state(
            GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 200.0, 140.0))
                .with_scroll_offset(scroll_path, GuiPoint { x: 0.0, y: 0.0 }),
        ),
    );

    let overflow = frame.root_layout.find_by_name("overflow_child").unwrap();
    assert_eq!(overflow.rect, GuiRect::new(50.0, 20.0, 60.0, 30.0));
    assert_eq!(overflow.clip_rect, GuiRect::new(0.0, 0.0, 200.0, 140.0));

    let scroll_child = frame.root_layout.find_by_name("scroll_child").unwrap();
    assert_eq!(scroll_child.rect, GuiRect::new(80.0, 70.0, 50.0, 30.0));
    assert_eq!(scroll_child.clip_rect, GuiRect::new(20.0, 70.0, 70.0, 40.0));
    assert_eq!(
        scroll_child.rects.hit_rect,
        GuiRect::new(80.0, 70.0, 10.0, 30.0)
    );

    let report = clipping_matrix_report(&frame);
    assert!(report.contains("active_goal"));
    assert!(report.contains("scroll_box"));
    write_gate21_report(&report);
}

fn phase4_document() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        Some(PathBuf::from(GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrydecisionview"
        size = { width = 360 height = 140 }
        gridBoxType = {
            name = "decision_grid_container"
            position = { x = 20 y = 0 }
            size = { width = 300 height = 60 }
            slotsize = { width = 300 height = 48 }
            max_slots = { x = 1 y = 3 }
            add_horizontal = no
            verticalScrollbar = "right_vertical_slider"
            scroll_wheel_factor = 2.5
            smooth_scrolling = yes
            margin = { top = 8 left = 0 bottom = 8 right = 12 }
        }
    }
    containerWindowType = {
        name = "decision_item"
        size = { width = 300 height = 48 }
        buttonType = {
            name = "btn_select"
            size = { width = 300 height = 48 }
        }
    }
}
"#,
    )
}

fn clipping_matrix_report(frame: &GuiRuntimeFrame) -> String {
    let mut out = String::new();
    out.push_str("# Gate 21 Clipping Matrix\n\n");
    for name in [
        "active_goal",
        "overflow_child",
        "scroll_box",
        "scroll_child",
    ] {
        if let Some(node) = frame.diagnostics.find_node_by_name(name) {
            out.push_str(&format!(
                "- {} path={} rect=({:.1}, {:.1}, {:.1}, {:.1}) clip=({:.1}, {:.1}, {:.1}, {:.1}) hit=({:.1}, {:.1}, {:.1}, {:.1})\n",
                name,
                node.path,
                node.rect.x,
                node.rect.y,
                node.rect.width,
                node.rect.height,
                node.clip_rect.x,
                node.clip_rect.y,
                node.clip_rect.width,
                node.clip_rect.height,
                node.hit_rect.x,
                node.hit_rect.y,
                node.hit_rect.width,
                node.hit_rect.height
            ));
        }
    }
    out
}

fn write_gate21_report(markdown: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/clausewitz_gui/gate21_clipping_matrix.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, markdown).unwrap();
}
