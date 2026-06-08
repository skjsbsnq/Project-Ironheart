use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use egui_kittest::Harness;
use hoi4_content::decision::{DecisionCategory, DecisionMechanicKind};
use hoi4_ui::decisions_panel::{CountryDecisionProfile, DecisionsData, DecisionsPanel};
use hoi4_ui::egui::{Event, PointerButton, Pos2, Vec2};
use hoi4_ui::politics::{DecisionCommand, DecisionEntry};
use hoi4_ui::theme::apply_vanilla_theme;
use hoi4_ui::vanilla_gui::{
    VanillaPanelProfile, VanillaProfileRegistry, COUNTRY_DECISION_GUI_FILE,
    COUNTRY_DECISION_PROFILE_ID, COUNTRY_DECISION_ROOT,
};

fn report_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/gate23/decision_panel_phase4.md")
}

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_DECISION_PHASE4_SNAPSHOT").is_some()
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

fn sample_data() -> DecisionsData {
    DecisionsData {
        country_tag: "GER".to_owned(),
        political_power: 150.0,
        mechanics: Vec::new(),
        decisions: vec![
            DecisionEntry {
                id: "phase4.expand_autobahn".to_owned(),
                name: "Expand the Autobahn".to_owned(),
                description: "Commit political capital to public works.".to_owned(),
                icon: "GFX_decision_generic_construction".to_owned(),
                effect_preview: "Construction capacity +1".to_owned(),
                category: DecisionCategory::Industry,
                mechanic_kind: DecisionMechanicKind::Standard,
                cost_political_power: 50.0,
                visible: true,
                clickable: true,
                mission_remaining: None,
                mission_total: None,
                cooldown_remaining: None,
                already_fired: false,
            },
            DecisionEntry {
                id: "phase4.training_mission".to_owned(),
                name: "Army Training Mission".to_owned(),
                description: "Run a timed readiness program.".to_owned(),
                icon: "GFX_decision_generic_army_support".to_owned(),
                effect_preview: "Army experience +10".to_owned(),
                category: DecisionCategory::Military,
                mechanic_kind: DecisionMechanicKind::Timed,
                cost_political_power: 25.0,
                visible: true,
                clickable: false,
                mission_remaining: Some(21),
                mission_total: Some(70),
                cooldown_remaining: None,
                already_fired: false,
            },
            DecisionEntry {
                id: "phase4.hidden".to_owned(),
                name: "Hidden Decision".to_owned(),
                description: String::new(),
                icon: "GFX_decision_unknown".to_owned(),
                effect_preview: String::new(),
                category: DecisionCategory::Internal,
                mechanic_kind: DecisionMechanicKind::Standard,
                cost_political_power: 0.0,
                visible: false,
                clickable: false,
                mission_remaining: None,
                mission_total: None,
                cooldown_remaining: None,
                already_fired: false,
            },
        ],
        country_flags: Vec::new(),
    }
}

#[test]
fn gate20_country_decision_profile_descriptor_matches_vanilla_targets() {
    let profile = CountryDecisionProfile;
    let descriptor = profile.descriptor();

    assert_eq!(descriptor.profile_id, COUNTRY_DECISION_PROFILE_ID);
    assert_eq!(descriptor.root_template, COUNTRY_DECISION_ROOT);
    assert_eq!(descriptor.required_gui_files, &[COUNTRY_DECISION_GUI_FILE]);
    for template in [
        "category_header",
        "decision_category_desc",
        "decision_item",
        "timed_decision_item",
        "category_end",
    ] {
        assert!(descriptor.key_templates.contains(&template));
    }

    let mut registry = VanillaProfileRegistry::new();
    registry.register(&profile);
    assert!(registry.get(COUNTRY_DECISION_PROFILE_ID).is_some());
}

#[test]
fn gate20_countrydecisionview_root_layout_loads_when_vanilla_available() {
    let Some(context) =
        hoi4_ui::vanilla_gui::VanillaGuiRuntimeContext::load(&[COUNTRY_DECISION_GUI_FILE])
    else {
        return;
    };
    let root = context
        .root_template(COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT)
        .expect("countrydecisionview root");
    let layout = hoi4_ui::vanilla_gui::compute_layout_tree(
        root,
        &hoi4_ui::vanilla_gui::LayoutOptions::new(hoi4_ui::vanilla_gui::GuiRect::new(
            0.0, 0.0, 1920.0, 1080.0,
        ))
        .shown_position(true),
    );

    assert!(layout.find_by_name("decision_grid").is_some());
    assert!(layout.find_by_name("close_button").is_some());
}

#[test]
fn gate23_decision_panel_vanilla_snapshot_and_activate_command() {
    let Some(_context) =
        hoi4_ui::vanilla_gui::VanillaGuiRuntimeContext::load(&[COUNTRY_DECISION_GUI_FILE])
    else {
        std::fs::create_dir_all(report_path().parent().unwrap()).unwrap();
        std::fs::write(
            report_path(),
            "# Decision Panel Phase 4\n- skipped: HOI4 vanilla runtime unavailable\n",
        )
        .unwrap();
        return;
    };

    let data = sample_data();
    let commands: Rc<RefCell<Vec<DecisionCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&commands);
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let (_, frame_commands) = DecisionsPanel::show(ctx, &data);
            captured.borrow_mut().extend(frame_commands);
        });

    harness.run_steps(40);
    snapshot_if_enabled(&mut harness, "decision_panel_gate23_vanilla");
    click_at(&mut harness, Pos2::new(480.0, 388.0));
    harness.run_steps(4);

    assert!(
        commands
            .borrow()
            .iter()
            .any(|command| matches!(command, DecisionCommand::Activate(id) if id == "phase4.expand_autobahn")),
        "commands={:?}",
        commands.borrow()
    );
    assert!(
        !commands
            .borrow()
            .iter()
            .any(|command| matches!(command, DecisionCommand::Activate(id) if id == "phase4.training_mission")),
        "disabled timed decision should not activate: {:?}",
        commands.borrow()
    );

    std::fs::create_dir_all(report_path().parent().unwrap()).unwrap();
    std::fs::write(
        report_path(),
        "# Decision Panel Phase 4\n- vanilla_runtime: true\n- snapshot: crates/hoi4-ui/tests/snapshots/decision_panel_gate23_vanilla.png\n- activate_command: phase4.expand_autobahn\n",
    )
    .unwrap();
}

#[test]
fn gate23_decisions_show_no_longer_calls_legacy_formal_paths() {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/decisions_panel.rs"),
    )
    .expect("read decisions_panel.rs");
    let show_start = source
        .find("impl DecisionsPanel")
        .expect("DecisionsPanel impl");
    let show_body = &source[show_start
        ..source[show_start..]
            .find("fn journal_show_decisions")
            .map(|idx| show_start + idx)
            .expect("journal function after show")];

    assert!(!show_body.contains("journal_show_decisions("));
    assert!(!show_body.contains("v9_show_decisions("));
    assert!(!show_body.contains("v9_decision_grid("));
}
