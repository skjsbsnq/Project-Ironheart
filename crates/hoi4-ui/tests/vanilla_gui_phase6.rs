use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use egui_kittest::Harness;
use hoi4_content::decision::{DecisionCategory, DecisionMechanicKind};
use hoi4_content::focus::{Effect, Focus, FocusTree, Trigger};
use hoi4_paths::{PathConfig, PathError};
use hoi4_ui::decisions_panel::{DecisionsData, DecisionsPanel};
use hoi4_ui::egui::{Event, PointerButton, Pos2, Sense, Vec2};
use hoi4_ui::focus_tree_panel::{FocusCommand, FocusTreePanel};
use hoi4_ui::icons::IconBank;
use hoi4_ui::politics::{DecisionCommand, DecisionEntry, IdeaEntry, PoliticsData, PoliticsPanel};
use hoi4_ui::theme::apply_vanilla_theme;
use hoi4_ui::vanilla_gui::{
    vanilla_builtin_profile_descriptors, vanilla_profile_diagnostics_markdown,
    vanilla_profiles_diagnostics_markdown, GuiBindingMap, GuiRect, LayoutOptions,
    VanillaGuiDiagnosticCategory, VanillaGuiProfileDiagnostics, VanillaGuiRenderer,
    VanillaGuiRuntimeContext, VanillaGuiRuntimeLoadError, COUNTRY_DECISION_DESCRIPTOR,
    COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT, COUNTRY_POLITICS_DESCRIPTOR,
    COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT, NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_ROOT,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn report_path(gate: &str, file: &str) -> PathBuf {
    workspace_root().join("target").join(gate).join(file)
}

fn write_report(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn temp_case_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ironheart_phase6_{}_{}", name, std::process::id()))
}

fn make_fake_install(root: &Path) {
    std::fs::create_dir_all(root.join("map")).unwrap();
    std::fs::create_dir_all(root.join("common")).unwrap();
    std::fs::create_dir_all(root.join("interface")).unwrap();
}

fn write_minimal_gui(root: &Path, rel: &str, root_name: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            r#"
guiTypes = {{
    containerWindowType = {{
        name = "{root_name}"
        size = {{ width = 300 height = 160 }}
    }}
}}
"#
        ),
    )
    .unwrap();
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
        political_power: 150.0,
        mechanics: Vec::new(),
        decisions: vec![
            DecisionEntry {
                id: "phase6.expand_autobahn".to_owned(),
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
                id: "phase6.training_mission".to_owned(),
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
        ],
        country_flags: Vec::new(),
        prewar: None,
    }
}

fn sample_focus_tree() -> FocusTree {
    FocusTree {
        country: "GER".to_owned(),
        focuses: vec![
            Focus {
                id: "phase6_industry".to_owned(),
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
                id: "phase6_army".to_owned(),
                name: "Army Modernization".to_owned(),
                icon: "GFX_goal_generic_army_doctrines".to_owned(),
                position: (-1, 1),
                cost_days: 70,
                prerequisites: vec![vec!["phase6_industry".to_owned()]],
                mutually_exclusive: Vec::new(),
                available: Trigger::AlwaysTrue,
                completion_effect: vec![Effect::ArmyExperience(10.0)],
            },
        ],
    }
}

fn sample_politics_data() -> PoliticsData {
    PoliticsData::legacy(
        "fascism".to_owned(),
        vec![("fascism".to_owned(), 1.0)],
        vec![IdeaEntry {
            key: "spirit".to_owned(),
            name: "National Spirit".to_owned(),
            category: "country".to_owned(),
            picture: Some("GFX_idea_generic_political_reform".to_owned()),
            modifiers: Vec::new(),
        }],
    )
}

#[test]
fn gate32_missing_hoi4_path_diagnostic_is_categorized_without_panic() {
    let error = VanillaGuiRuntimeLoadError::Hoi4PathUnavailable(PathError::NotFound {
        tried: vec![PathBuf::from("Z:/not/a/hoi4/install")],
    });
    let diagnostics =
        VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_DECISION_DESCRIPTOR, error);
    let markdown = diagnostics.to_markdown();

    assert!(!diagnostics.runtime_available);
    assert!(diagnostics.category_names().contains(&"hoi4_path"));
    assert!(markdown.contains("HOI4 path unavailable"));
    assert!(markdown.contains("country_decisions"));
    write_report(
        &report_path("gate32", "missing_hoi4_path_diagnostic.md"),
        &markdown,
    );
}

#[test]
fn gate32_missing_gui_file_diagnostic_is_categorized_without_legacy_fallback() {
    let root = temp_case_dir("missing_gui");
    let _ = std::fs::remove_dir_all(&root);
    make_fake_install(&root);
    let path_cfg = PathConfig::with_game_path(&root);

    let error = VanillaGuiRuntimeContext::load_with_path_config_result(
        path_cfg,
        &[COUNTRY_DECISION_GUI_FILE],
    )
    .expect_err("missing countrydecisionview.gui should be reported");
    let diagnostics =
        VanillaGuiProfileDiagnostics::unavailable(&COUNTRY_DECISION_DESCRIPTOR, error);
    let markdown = diagnostics.to_markdown();

    assert!(!diagnostics.runtime_available);
    assert_eq!(
        diagnostics.missing_gui_files,
        vec![COUNTRY_DECISION_GUI_FILE]
    );
    assert!(diagnostics.category_names().contains(&"gui"));
    assert!(markdown.contains("required GUI file missing"));
    assert!(!markdown.contains("journal_show_decisions"));
    assert!(!markdown.contains("v9_decision_grid"));
    write_report(
        &report_path("gate32", "missing_gui_file_diagnostic.md"),
        &markdown,
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gate32_missing_sprite_diagnostic_and_placeholder_render_do_not_panic() {
    let root = temp_case_dir("missing_sprite");
    let _ = std::fs::remove_dir_all(&root);
    make_fake_install(&root);
    write_minimal_gui(&root, COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT);
    let path_cfg = PathConfig::with_game_path(&root);
    let context = VanillaGuiRuntimeContext::load_with_path_config_result(
        path_cfg.clone(),
        &[COUNTRY_DECISION_GUI_FILE],
    )
    .expect("minimal GUI should load");
    let diagnostics =
        VanillaGuiProfileDiagnostics::from_context(&context, &COUNTRY_DECISION_DESCRIPTOR);
    assert!(diagnostics.category_names().contains(&"gfx"));
    assert!(diagnostics
        .missing_sprites
        .contains(&"GFX_decision_item_bg".to_owned()));

    let doc = hoi4_ui::vanilla_gui::parse_gui_str(
        None,
        r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 120 height = 80 }
        iconType = {
            name = "missing_icon"
            position = { x = 10 y = 10 }
            size = { width = 40 height = 40 }
            spriteType = "GFX_phase6_missing_sprite"
        }
    }
}
"#,
    );
    let root_node = doc.template_index().get("root").unwrap().clone();
    let layout = hoi4_ui::vanilla_gui::compute_layout_tree(
        &root_node,
        &LayoutOptions::new(GuiRect::new(0.0, 0.0, 120.0, 80.0)),
    );
    let stats_slot = Rc::new(RefCell::new(None));
    let stats_out = Rc::clone(&stats_slot);
    let gfx_index = hoi4_ui::vanilla_gui::GfxIndex::from_files(Vec::<PathBuf>::new());
    let mut harness = Harness::builder()
        .with_size(Vec2::new(120.0, 80.0))
        .build(move |ctx| {
            let mut icon_bank = IconBank::new(ctx.clone(), path_cfg.clone());
            let renderer = VanillaGuiRenderer::new(&gfx_index);
            egui::Area::new(egui::Id::new("phase6_missing_sprite"))
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    let _ = ui.allocate_exact_size(Vec2::new(120.0, 80.0), Sense::hover());
                    let stats = renderer.paint_tree(
                        ui,
                        &root_node,
                        &layout,
                        &GuiBindingMap::default(),
                        &mut icon_bank,
                    );
                    *stats_out.borrow_mut() = Some(stats);
                });
        });
    harness.run();
    let stats = stats_slot.borrow().clone().unwrap();

    assert_eq!(stats.fallback_painted, 1, "{stats:?}");
    assert!(stats
        .fallback_labels
        .contains(&"GFX_phase6_missing_sprite".to_owned()));

    let mut markdown = diagnostics.to_markdown();
    let _ = writeln!(markdown);
    let _ = writeln!(
        markdown,
        "- [texture_decode] missing sprite render fallback labels: {:?}",
        stats.fallback_labels
    );
    write_report(
        &report_path("gate32", "missing_sprite_placeholder_diagnostic.md"),
        &markdown,
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gate32_diagnostic_categories_cover_runtime_failure_classes() {
    let categories = [
        VanillaGuiDiagnosticCategory::Hoi4Path,
        VanillaGuiDiagnosticCategory::Parser,
        VanillaGuiDiagnosticCategory::Layout,
        VanillaGuiDiagnosticCategory::Gfx,
        VanillaGuiDiagnosticCategory::TextureDecode,
        VanillaGuiDiagnosticCategory::Binding,
        VanillaGuiDiagnosticCategory::Gui,
    ]
    .map(VanillaGuiDiagnosticCategory::as_str);

    assert_eq!(
        categories,
        [
            "hoi4_path",
            "parser",
            "layout",
            "gfx",
            "texture_decode",
            "binding",
            "gui"
        ]
    );
}

#[test]
fn gate33_regression_matrix_real_resources_or_diagnostic_state() {
    let report_path = report_path("gate33", "vanilla_gui_regression_matrix.md");
    let mut report = String::from("# Vanilla GUI Phase 6 Regression Matrix\n\n");
    match VanillaGuiRuntimeContext::load_all_profiles(vanilla_builtin_profile_descriptors()) {
        Some(context) => {
            let _ = writeln!(report, "- runtime_available: true");
            for descriptor in vanilla_builtin_profile_descriptors() {
                let profile_report = context.profile_report(descriptor);
                let _ = writeln!(
                    report,
                    "- {}: gui_loaded={} root_loaded={} key_templates_missing={:?} missing_sprites={:?}",
                    descriptor.profile_id,
                    profile_report.gui_loaded,
                    profile_report.root_loaded,
                    profile_report.key_templates_missing,
                    profile_report.gfx_hits.missing
                );
                assert!(profile_report.gui_loaded, "{profile_report:?}");
                assert!(profile_report.root_loaded, "{profile_report:?}");
                assert!(
                    profile_report.key_templates_missing.is_empty(),
                    "{profile_report:?}"
                );
            }

            let politics = context
                .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
                .expect("politics root");
            let decision = context
                .root_template(COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT)
                .expect("decision root");
            let focus = context
                .root_template(NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_ROOT)
                .expect("focus root");
            for (name, root) in [
                ("politics", politics),
                ("decisions", decision),
                ("focus", focus),
            ] {
                let layout = hoi4_ui::vanilla_gui::compute_layout_tree(
                    root,
                    &LayoutOptions::new(GuiRect::new(0.0, 0.0, 1280.0, 720.0)).shown_position(true),
                );
                let _ = writeln!(
                    report,
                    "- {name}_layout: rect=({}, {}, {}, {})",
                    layout.rect.x, layout.rect.y, layout.rect.width, layout.rect.height
                );
                assert!(layout.rect.width > 0.0, "{name}");
                assert!(layout.rect.height > 0.0, "{name}");
            }
        }
        None => {
            let diagnostics =
                vanilla_profiles_diagnostics_markdown(None, vanilla_builtin_profile_descriptors());
            let _ = writeln!(report, "- runtime_available: false");
            let _ = writeln!(report);
            let _ = writeln!(report, "{diagnostics}");
            assert!(diagnostics.contains("runtime_available: false"));
            assert!(!diagnostics.contains("v9_show_focus_tree"));
            assert!(!diagnostics.contains("journal_show_decisions"));
        }
    }
    write_report(&report_path, &report);
}

#[test]
fn gate33_click_commands_still_route_through_vanilla_panels_when_available() {
    if VanillaGuiRuntimeContext::load(&[COUNTRY_DECISION_GUI_FILE]).is_none()
        || VanillaGuiRuntimeContext::load(&[NATIONAL_FOCUS_GUI_FILE]).is_none()
    {
        return;
    }

    let decisions = sample_decisions_data();
    let decision_commands: Rc<RefCell<Vec<DecisionCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let captured_decisions = Rc::clone(&decision_commands);
    let mut decision_harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let (_, commands) = DecisionsPanel::show(ctx, &decisions);
            captured_decisions.borrow_mut().extend(commands);
        });
    decision_harness.run_steps(40);
    click_at(&mut decision_harness, Pos2::new(480.0, 388.0));
    decision_harness.run_steps(4);
    assert!(
        decision_commands
            .borrow()
            .iter()
            .any(|command| matches!(command, DecisionCommand::Activate(id) if id == "phase6.expand_autobahn")),
        "decision_commands={:?}",
        decision_commands.borrow()
    );

    let tree = sample_focus_tree();
    let completed = HashSet::from(["phase6_industry".to_owned()]);
    let available = HashSet::from(["phase6_industry".to_owned(), "phase6_army".to_owned()]);
    let focus_commands: Rc<RefCell<Vec<FocusCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let captured_focus = Rc::clone(&focus_commands);
    let mut panel = FocusTreePanel::new();
    panel.open = true;
    panel.selected = Some("phase6_army".to_owned());
    let mut focus_harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 720.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            if let Some(command) = panel.show(ctx, &tree, &completed, None, 0.0, &available) {
                captured_focus.borrow_mut().push(command);
            }
        });
    focus_harness.run_steps(30);
    click_at(&mut focus_harness, Pos2::new(950.0, 170.0));
    focus_harness.run_steps(4);
    assert!(
        focus_commands
            .borrow()
            .iter()
            .any(|command| matches!(command, FocusCommand::Start(id) if id == "phase6_army")),
        "focus_commands={:?}",
        focus_commands.borrow()
    );
}

#[test]
fn gate33_formal_panel_paths_do_not_call_legacy_ui() {
    let ui_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let decisions =
        std::fs::read_to_string(ui_src.join("decisions_panel.rs")).expect("decisions source");
    let decisions_formal = source_between(
        &decisions,
        "impl DecisionsPanel",
        "fn journal_show_decisions",
    );
    for forbidden in [
        "journal_show_decisions(",
        "v9_show_decisions(",
        "v9_decision_grid(",
        "JournalPanelShell::new(",
    ] {
        assert!(
            !decisions_formal.contains(forbidden),
            "decision formal path calls {forbidden}"
        );
    }

    let focus = std::fs::read_to_string(ui_src.join("focus_tree_panel.rs")).expect("focus source");
    let focus_formal = source_between(&focus, "impl FocusTreePanel", "fn v9_show_focus_tree");
    for forbidden in [
        "v9_show_focus_tree(",
        "v9_focus_tree_body(",
        "v9_draw_focus_node(",
        "v9_draw_focus_lines(",
        "PanelShell::new(",
    ] {
        assert!(
            !focus_formal.contains(forbidden),
            "focus formal path calls {forbidden}"
        );
    }

    let politics = std::fs::read_to_string(ui_src.join("politics.rs")).expect("politics source");
    let politics_formal = source_between(
        &politics,
        "impl PoliticsPanel",
        "fn log_politics_render_stats",
    );
    for forbidden in [
        "v9_show_politics(",
        "vanilla_politics_body(",
        "vanilla_politics_shell(",
        "v9_politics_body(",
        "paint_politics_shell(",
        "IRONHEART_POLITICS_V9_FALLBACK",
    ] {
        assert!(
            !politics_formal.contains(forbidden),
            "politics formal path calls {forbidden}"
        );
    }
}

#[test]
fn gate34_final_visual_artifacts_are_declared_without_vanilla_assets_in_repo() {
    let report_path = report_path("gate34", "final_visual_artifacts.md");
    let snapshots = [
        "crates/hoi4-ui/tests/snapshots/politics_gate9_1936_1080p.png",
        "crates/hoi4-ui/tests/snapshots/decision_panel_gate23_vanilla.png",
        "crates/hoi4-ui/tests/snapshots/focus_tree_gate31_vanilla.png",
        "crates/hoi4-ui/tests/snapshots/politics_gate0_project_baseline.png",
        "crates/hoi4-ui/tests/snapshots/decisions_gate0_project_baseline.png",
        "crates/hoi4-ui/tests/snapshots/focus_tree_gate0_project_baseline.png",
    ];
    let mut report = String::from("# Final Vanilla GUI Phase 6 Artifacts\n\n");
    for snapshot in snapshots {
        let exists = workspace_root().join(snapshot).exists();
        let _ = writeln!(report, "- {snapshot}: exists={exists}");
        assert!(exists, "missing final snapshot {snapshot}");
    }

    let repo_files = std::process::Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(workspace_root())
        .output()
        .expect("git ls-files");
    let listed = String::from_utf8_lossy(&repo_files.stdout);
    for line in listed.lines() {
        let lower = line.replace('\\', "/").to_ascii_lowercase();
        assert!(
            !(lower.ends_with(".dds")
                || lower.ends_with(".gui")
                || lower.ends_with(".gfx")
                || lower.contains("interface/countrydecisionview.gui")
                || lower.contains("interface/nationalfocusview.gui")),
            "untracked vanilla-like asset should not be in repo: {line}"
        );
    }
    let _ = writeln!(report);
    let _ = writeln!(
        report,
        "Optional enhancements: search box, filters, shortcuts, continuous focus, focus styles, joint focus styles."
    );
    write_report(&report_path, &report);
}

#[test]
fn gate32_profile_diagnostics_markdown_handles_loaded_and_unloaded_profiles() {
    let loaded_or_unloaded = vanilla_profile_diagnostics_markdown(
        VanillaGuiRuntimeContext::load(&[COUNTRY_POLITICS_GUI_FILE]).as_ref(),
        &COUNTRY_POLITICS_DESCRIPTOR,
    );
    assert!(loaded_or_unloaded.contains("country_politics"));
    assert!(loaded_or_unloaded.contains("categories:"));
}

#[test]
fn gate32_politics_unavailable_path_has_no_legacy_commands() {
    let root = temp_case_dir("politics_unavailable");
    let _ = std::fs::remove_dir_all(&root);
    let path_cfg = PathConfig::with_game_path(&root);
    let ctx = egui::Context::default();
    let mut icon_bank = IconBank::new(ctx.clone(), path_cfg);
    let data = sample_politics_data();

    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 640.0))
        .build(move |ctx| {
            let _ = apply_vanilla_theme(ctx);
            let (_, commands) = PoliticsPanel::show(ctx, &data, &mut icon_bank);
            assert!(commands.is_empty());
        });
    harness.run();
    let _ = std::fs::remove_dir_all(&root);
}

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = source
        .find(start)
        .unwrap_or_else(|| panic!("missing {start}"));
    let rel_end = source[start_idx..]
        .find(end)
        .unwrap_or_else(|| panic!("missing {end} after {start}"));
    &source[start_idx..start_idx + rel_end]
}
