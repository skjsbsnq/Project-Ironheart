use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hoi4_ui::vanilla_gui::{
    parse_gui_str, GuiBindingMap, GuiRuntimeFrame, GuiRuntimeFrameInput, GuiRuntimeState,
    VanillaGuiRuntimeContext, COUNTRY_DECISION_DESCRIPTOR, COUNTRY_DECISION_GUI_FILE,
    COUNTRY_DECISION_ROOT, COUNTRY_POLITICS_DESCRIPTOR, COUNTRY_POLITICS_GUI_FILE,
    COUNTRY_POLITICS_ROOT,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn report_path(file: &str) -> PathBuf {
    workspace_root().join("target/clausewitz_gui").join(file)
}

fn write_report(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn gate7_transform_stack_reports_are_written_without_unknown_offsets() {
    let viewport = hoi4_ui::vanilla_gui::GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let bindings = GuiBindingMap::default();

    let politics_context =
        VanillaGuiRuntimeContext::load(COUNTRY_POLITICS_DESCRIPTOR.required_gui_files);
    let politics_root = politics_context
        .as_ref()
        .and_then(|context| context.root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT))
        .cloned()
        .unwrap_or_else(fallback_politics_root);
    let politics_frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            &politics_root,
            viewport,
            COUNTRY_POLITICS_DESCRIPTOR.profile_id,
            &bindings,
        )
        .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );
    let mut politics = String::new();
    let _ = writeln!(politics, "# Gate 7 Politics Transform Stack");
    let _ = writeln!(
        politics,
        "- source: {}",
        if politics_context.is_some() {
            "vanilla interface/countrypoliticsview.gui"
        } else {
            "fallback fixture"
        }
    );
    let _ = writeln!(politics);
    let _ = writeln!(
        politics,
        "{}",
        politics_frame.diagnostics.coordinate_markdown()
    );
    let _ = writeln!(
        politics,
        "{}",
        politics_frame.transform_stack_markdown_for_name("add_national_goal_button")
    );
    assert!(!politics.contains("unknown offset"));
    assert!(politics.contains("root_animation_offset"));
    assert!(politics.contains("final_rect [screen]"));
    write_report(&report_path("gate7_transform_stack_politics.md"), &politics);

    let decision_context =
        VanillaGuiRuntimeContext::load(COUNTRY_DECISION_DESCRIPTOR.required_gui_files);
    let decision_root = decision_context
        .as_ref()
        .and_then(|context| context.root_template(COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT))
        .cloned()
        .unwrap_or_else(fallback_decision_root);
    let decision_frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            &decision_root,
            viewport,
            COUNTRY_DECISION_DESCRIPTOR.profile_id,
            &bindings,
        )
        .with_runtime_state(GuiRuntimeState::shown(viewport)),
    );
    let mut decisions = String::new();
    let _ = writeln!(decisions, "# Gate 7 Decisions Transform Stack");
    let _ = writeln!(
        decisions,
        "- source: {}",
        if decision_context.is_some() {
            "vanilla interface/countrydecisionview.gui"
        } else {
            "fallback fixture"
        }
    );
    let _ = writeln!(decisions);
    let _ = writeln!(
        decisions,
        "{}",
        decision_frame.diagnostics.coordinate_markdown()
    );
    let _ = writeln!(
        decisions,
        "{}",
        decision_frame.transform_stack_markdown_for_name("decision_grid")
    );
    assert!(!decisions.contains("unknown offset"));
    assert!(decisions.contains("root_animation_offset"));
    assert!(decisions.contains("final_rect [screen]"));
    write_report(
        &report_path("gate7_transform_stack_decisions.md"),
        &decisions,
    );
}

fn fallback_politics_root() -> hoi4_ui::vanilla_gui::GuiNode {
    let doc = parse_gui_str(
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
            }
        }
    }
}
"#,
    );
    doc.template_index()
        .get(COUNTRY_POLITICS_ROOT)
        .expect("fallback politics root")
        .clone()
}

fn fallback_decision_root() -> hoi4_ui::vanilla_gui::GuiNode {
    let doc = parse_gui_str(
        None,
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrydecisionview"
        position = { x = -606 y = 78 }
        show_position = { x = -6 y = 78 }
        size = { width = 550 height = 100%% }
        containerWindowType = {
            name = "decision_grid_container"
            position = { x = -1 y = 123 }
            size = { width = 560 height = 900 }
            gridBoxType = {
                name = "decision_grid"
                position = { x = 0 y = 0 }
                size = { width = 540 height = 850 }
            }
        }
    }
}
"#,
    );
    doc.template_index()
        .get(COUNTRY_DECISION_ROOT)
        .expect("fallback decision root")
        .clone()
}
