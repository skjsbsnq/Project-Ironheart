use std::collections::HashSet;

use egui_kittest::Harness;
use hoi4_content::decision::{DecisionCategory, DecisionMechanicKind};
use hoi4_content::focus::{Effect, Focus, FocusTree, Trigger};
use hoi4_ui::decisions_panel::{DecisionsData, DecisionsPanel, MechanicGauge};
use hoi4_ui::egui::{Event, PointerButton, Pos2, Vec2};
use hoi4_ui::focus_tree_panel::{FocusCommand, FocusTreePanel};
use hoi4_ui::politics::{DecisionCommand, DecisionEntry};
use hoi4_ui::theme::apply_vanilla_theme;
use std::cell::RefCell;
use std::rc::Rc;

fn snapshot_if_enabled<State>(harness: &mut Harness<'_, State>, name: &str) {
    let enabled = std::env::var_os("RUN_DECISIONS_FOCUS_GATE0_SNAPSHOT").is_some()
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

fn sample_decisions_data() -> DecisionsData {
    DecisionsData {
        country_tag: "GER".to_owned(),
        political_power: 148.0,
        mechanics: vec![
            MechanicGauge {
                label: "Rear Area Security".to_owned(),
                value: 42.0,
                max: 100.0,
                detail: "Resistance pressure is contained.".to_owned(),
            },
            MechanicGauge {
                label: "Cabinet Cohesion".to_owned(),
                value: 73.0,
                max: 100.0,
                detail: "Factional pressure is low.".to_owned(),
            },
        ],
        decisions: vec![
            DecisionEntry {
                id: "gate0.expand_autobahn".to_owned(),
                name: "Expand the Autobahn".to_owned(),
                description: "Commit political capital to a visible public works program."
                    .to_owned(),
                icon: "GFX_decision_generic_industry".to_owned(),
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
                id: "gate0.training_mission".to_owned(),
                name: "Army Training Mission".to_owned(),
                description: "Run a timed army readiness program.".to_owned(),
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
                id: "gate0.cooldown".to_owned(),
                name: "Recent Propaganda Drive".to_owned(),
                description: "The public is not ready for another campaign.".to_owned(),
                icon: "GFX_decision_generic_political_discourse".to_owned(),
                effect_preview: "War support +2%".to_owned(),
                category: DecisionCategory::Internal,
                mechanic_kind: DecisionMechanicKind::Cooldown,
                cost_political_power: 35.0,
                visible: true,
                clickable: false,
                mission_remaining: None,
                mission_total: None,
                cooldown_remaining: Some(34),
                already_fired: false,
            },
        ],
        country_flags: vec!["gate0_baseline".to_owned()],
    }
}

fn sample_focus_tree() -> FocusTree {
    FocusTree {
        country: "GER".to_owned(),
        focuses: vec![
            Focus {
                id: "gate0_industry".to_owned(),
                name: "Industrial Recovery".to_owned(),
                icon: "GFX_goal_generic_construct_civilian".to_owned(),
                position: (0, 0),
                cost_days: 70,
                prerequisites: vec![],
                mutually_exclusive: vec![],
                available: Trigger::AlwaysTrue,
                completion_effect: vec![Effect::AddPoliticalPower(25.0)],
            },
            Focus {
                id: "gate0_army".to_owned(),
                name: "Army Modernization".to_owned(),
                icon: "GFX_goal_generic_army_doctrines".to_owned(),
                position: (1, 1),
                cost_days: 70,
                prerequisites: vec![vec!["gate0_industry".to_owned()]],
                mutually_exclusive: vec!["gate0_diplomacy".to_owned()],
                available: Trigger::AlwaysTrue,
                completion_effect: vec![Effect::ArmyExperience(10.0)],
            },
            Focus {
                id: "gate0_diplomacy".to_owned(),
                name: "Diplomatic Pressure".to_owned(),
                icon: "GFX_goal_generic_demand_territory".to_owned(),
                position: (-1, 1),
                cost_days: 70,
                prerequisites: vec![vec!["gate0_industry".to_owned()]],
                mutually_exclusive: vec!["gate0_army".to_owned()],
                available: Trigger::AlwaysTrue,
                completion_effect: vec![Effect::AddNamedThreat(1.0)],
            },
        ],
    }
}

#[test]
fn decisions_gate0_project_baseline_snapshot() {
    let data = sample_decisions_data();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = DecisionsPanel::show(ctx, &data);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "decisions_gate0_project_baseline");
}

#[test]
fn decisions_gate0_activate_command_still_emits() {
    if hoi4_ui::vanilla_gui::VanillaGuiRuntimeContext::load(&[
        hoi4_ui::vanilla_gui::COUNTRY_DECISION_GUI_FILE,
    ])
    .is_none()
    {
        return;
    }

    let data = sample_decisions_data();
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
    click_at(&mut harness, Pos2::new(480.0, 508.0));
    harness.run_steps(4);

    assert!(
        commands
            .borrow()
            .iter()
            .any(|command| matches!(command, DecisionCommand::Activate(id) if id == "gate0.expand_autobahn")),
        "commands={:?}",
        commands.borrow()
    );
}

#[test]
fn focus_tree_gate0_project_baseline_snapshot() {
    let tree = sample_focus_tree();
    let completed = HashSet::from(["gate0_industry".to_owned()]);
    let available = HashSet::from([
        "gate0_industry".to_owned(),
        "gate0_army".to_owned(),
        "gate0_diplomacy".to_owned(),
    ]);
    let mut panel = FocusTreePanel::new();
    panel.open = true;
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let _ = panel.show(ctx, &tree, &completed, None, 0.0, &available);
        });

    harness.run();
    snapshot_if_enabled(&mut harness, "focus_tree_gate0_project_baseline");
}

#[test]
fn focus_tree_gate0_start_command_still_emits_for_selected_focus() {
    if hoi4_ui::vanilla_gui::VanillaGuiRuntimeContext::load(&[
        hoi4_ui::vanilla_gui::NATIONAL_FOCUS_GUI_FILE,
    ])
    .is_none()
    {
        return;
    }

    let tree = sample_focus_tree();
    let completed = HashSet::from(["gate0_industry".to_owned()]);
    let available = HashSet::from([
        "gate0_industry".to_owned(),
        "gate0_army".to_owned(),
        "gate0_diplomacy".to_owned(),
    ]);
    let commands: Rc<RefCell<Vec<FocusCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&commands);
    let mut panel = FocusTreePanel::new();
    panel.open = true;
    panel.selected = Some("gate0_army".to_owned());
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            if let Some(command) = panel.show(ctx, &tree, &completed, None, 0.0, &available) {
                captured.borrow_mut().push(command);
            }
        });

    harness.run_steps(20);
    click_at(&mut harness, Pos2::new(950.0, 170.0));
    harness.run_steps(4);

    assert!(
        commands
            .borrow()
            .iter()
            .any(|command| matches!(command, FocusCommand::Start(id) if id == "gate0_army")),
        "commands={:?}",
        commands.borrow()
    );
}
