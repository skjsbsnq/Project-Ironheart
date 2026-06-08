use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use egui_kittest::Harness;
use hoi4_content::focus::{Effect, Focus, FocusTree, Trigger};
use hoi4_ui::egui::{Event, PointerButton, Pos2, Vec2};
use hoi4_ui::focus_tree_panel::{FocusCommand, FocusTreePanel, NationalFocusProfile};
use hoi4_ui::theme::apply_vanilla_theme;
use hoi4_ui::vanilla_gui::{
    required_focus_marker_report, VanillaPanelProfile, VanillaProfileRegistry,
    NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_PROFILE_ID, NATIONAL_FOCUS_ROOT,
};

fn report_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/gate31/focus_tree_panel_phase5.md")
}

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_FOCUS_PHASE5_SNAPSHOT").is_some()
        || std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    if enabled {
        harness.snapshot(name);
    }
}

fn click_at<State>(harness: &mut Harness<'_, State>, pos: Pos2) {
    let modifiers = harness.input().modifiers;
    let input = harness.input_mut();
    input.events.push(Event::PointerMoved(pos));
    input.events.push(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers,
    });
    input.events.push(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers,
    });
}

fn sample_focus_tree() -> FocusTree {
    FocusTree {
        country: "GER".to_owned(),
        focuses: vec![
            Focus {
                id: "phase5_industry".to_owned(),
                name: "Industrial Recovery".to_owned(),
                icon: "GFX_goal_generic_construct_civ_factory".to_owned(),
                position: (0, 0),
                cost_days: 70,
                prerequisites: Vec::new(),
                mutually_exclusive: Vec::new(),
                available: Trigger::AlwaysTrue,
                completion_effect: vec![Effect::AddPoliticalPower(35.0)],
            },
            Focus {
                id: "phase5_army".to_owned(),
                name: "Army Modernization".to_owned(),
                icon: "GFX_goal_generic_army_doctrines".to_owned(),
                position: (-1, 1),
                cost_days: 70,
                prerequisites: vec![vec!["phase5_industry".to_owned()]],
                mutually_exclusive: vec!["phase5_diplomacy".to_owned()],
                available: Trigger::AlwaysTrue,
                completion_effect: vec![Effect::ArmyExperience(10.0)],
            },
            Focus {
                id: "phase5_diplomacy".to_owned(),
                name: "Diplomatic Pressure".to_owned(),
                icon: String::new(),
                position: (1, 1),
                cost_days: 56,
                prerequisites: vec![vec!["phase5_industry".to_owned()]],
                mutually_exclusive: vec!["phase5_army".to_owned()],
                available: Trigger::AlwaysTrue,
                completion_effect: vec![Effect::AddOpinion {
                    target: "ITA".to_owned(),
                    amount: 25,
                }],
            },
            Focus {
                id: "phase5_locked".to_owned(),
                name: "Long Rearmament".to_owned(),
                icon: "GFX_goal_unknown".to_owned(),
                position: (0, 2),
                cost_days: 70,
                prerequisites: vec![vec![
                    "phase5_army".to_owned(),
                    "phase5_diplomacy".to_owned(),
                ]],
                mutually_exclusive: Vec::new(),
                available: Trigger::AlwaysTrue,
                completion_effect: Vec::new(),
            },
        ],
    }
}

#[test]
fn gate24_national_focus_profile_descriptor_matches_vanilla_targets() {
    let profile = NationalFocusProfile;
    let descriptor = profile.descriptor();

    assert_eq!(descriptor.profile_id, NATIONAL_FOCUS_PROFILE_ID);
    assert_eq!(descriptor.root_template, NATIONAL_FOCUS_ROOT);
    assert_eq!(descriptor.required_gui_files, &[NATIONAL_FOCUS_GUI_FILE]);
    for template in [
        "national_focus_item",
        "national_focus_link",
        "national_focus_exclusive_item",
        "national_focus_detail_view",
    ] {
        assert!(descriptor.key_templates.contains(&template));
    }

    let mut registry = VanillaProfileRegistry::new();
    registry.register(&profile);
    assert!(registry.get(NATIONAL_FOCUS_PROFILE_ID).is_some());
}

#[test]
fn gate24_nationalfocusview_root_layout_and_markers_load_when_vanilla_available() {
    let Some(context) =
        hoi4_ui::vanilla_gui::VanillaGuiRuntimeContext::load(&[NATIONAL_FOCUS_GUI_FILE])
    else {
        return;
    };
    let root = context
        .root_template(NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_ROOT)
        .expect("nationalfocusview root");
    let layout = hoi4_ui::vanilla_gui::compute_layout_tree(
        root,
        &hoi4_ui::vanilla_gui::LayoutOptions::new(hoi4_ui::vanilla_gui::GuiRect::new(
            0.0, 0.0, 1920.0, 1080.0,
        )),
    );
    let markers = required_focus_marker_report(root);

    assert!(layout.find_by_name("tree").is_some());
    assert!(layout
        .find_by_name("grid_window")
        .and_then(|node| node.find_by_name("grid"))
        .is_some());
    assert_eq!(markers.get("focus_spacing"), Some(&(96.0, 130.0)));
    assert_eq!(markers.get("national_focus_center"), Some(&(130.0, 32.0)));
    assert_eq!(markers.get("link_spacing"), Some(&(16.0, 16.0)));
}

#[test]
fn gate31_focus_tree_vanilla_snapshot_select_and_start_command() {
    let Some(_context) =
        hoi4_ui::vanilla_gui::VanillaGuiRuntimeContext::load(&[NATIONAL_FOCUS_GUI_FILE])
    else {
        std::fs::create_dir_all(report_path().parent().unwrap()).unwrap();
        std::fs::write(
            report_path(),
            "# Focus Tree Panel Phase 5\n- skipped: HOI4 vanilla runtime unavailable\n",
        )
        .unwrap();
        return;
    };

    let tree = sample_focus_tree();
    let completed = HashSet::from(["phase5_industry".to_owned()]);
    let available = HashSet::from([
        "phase5_industry".to_owned(),
        "phase5_army".to_owned(),
        "phase5_diplomacy".to_owned(),
    ]);
    let commands: Rc<RefCell<Vec<FocusCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&commands);
    let mut panel = FocusTreePanel::new();
    panel.open = true;
    panel.zoom = 1.0;
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            if let Some(command) = panel.show(ctx, &tree, &completed, None, 0.0, &available) {
                captured.borrow_mut().push(command);
            }
        });

    harness.run_steps(30);
    snapshot_if_enabled(&mut harness, "focus_tree_gate31_vanilla");
    click_at(&mut harness, Pos2::new(315.0, 397.0));
    harness.run_steps(4);
    snapshot_if_enabled(&mut harness, "focus_tree_gate31_detail_vanilla");
    click_at(&mut harness, Pos2::new(950.0, 170.0));
    harness.run_steps(4);

    assert!(
        commands
            .borrow()
            .iter()
            .any(|command| matches!(command, FocusCommand::Start(id) if id == "phase5_army")),
        "commands={:?}",
        commands.borrow()
    );

    std::fs::create_dir_all(report_path().parent().unwrap()).unwrap();
    std::fs::write(
        report_path(),
        "# Focus Tree Panel Phase 5\n- vanilla_runtime: true\n- snapshot: crates/hoi4-ui/tests/snapshots/focus_tree_gate31_vanilla.png\n- tree_detail_selection: phase5_army\n- start_command: phase5_army\n",
    )
    .unwrap();
}

#[test]
fn gate31_focus_tree_show_no_longer_calls_legacy_formal_paths() {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/focus_tree_panel.rs"),
    )
    .expect("read focus_tree_panel.rs");
    let show_start = source
        .find("impl FocusTreePanel")
        .expect("FocusTreePanel impl");
    let show_body = &source[show_start
        ..source[show_start..]
            .find("fn v9_show_focus_tree")
            .map(|idx| show_start + idx)
            .expect("v9 function after show")];

    assert!(!show_body.contains("v9_show_focus_tree("));
    assert!(!show_body.contains("v9_focus_tree_body("));
    assert!(!show_body.contains("v9_draw_focus_node("));
    assert!(!show_body.contains("v9_draw_focus_lines("));
    assert!(!show_body.contains("PanelShell::new("));
}
