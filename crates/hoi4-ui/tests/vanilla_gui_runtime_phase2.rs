use std::fmt::Write as _;
use std::path::PathBuf;

use hoi4_ui::vanilla_gui::{
    collect_profile_gfx_references, required_focus_marker_report,
    vanilla_builtin_profile_descriptors, vanilla_profiles_diagnostics_markdown, GuiRect,
    LayoutOptions, VanillaGuiRuntimeContext, COUNTRY_DECISION_DESCRIPTOR,
    COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT, COUNTRY_POLITICS_DESCRIPTOR,
    COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT, NATIONAL_FOCUS_DESCRIPTOR,
    NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_ROOT,
};

fn gate13_report_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/gate13/vanilla_gui_runtime_profiles.md")
}

#[test]
fn gate13_runtime_smoke_covers_politics_decisions_and_focus_gui() {
    let report_path = gate13_report_path();
    std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();

    let Some(context) =
        VanillaGuiRuntimeContext::load_all_profiles(vanilla_builtin_profile_descriptors())
    else {
        let report =
            vanilla_profiles_diagnostics_markdown(None, vanilla_builtin_profile_descriptors());
        std::fs::write(&report_path, report).unwrap();
        return;
    };

    let mut report = vanilla_profiles_diagnostics_markdown(
        Some(&context),
        vanilla_builtin_profile_descriptors(),
    );
    let _ = writeln!(report);
    let _ = writeln!(report, "## Smoke Details");

    let politics_root = context
        .root_template(COUNTRY_POLITICS_GUI_FILE, COUNTRY_POLITICS_ROOT)
        .expect("politics root template");
    let decision_root = context
        .root_template(COUNTRY_DECISION_GUI_FILE, COUNTRY_DECISION_ROOT)
        .expect("decision root template");
    let focus_root = context
        .root_template(NATIONAL_FOCUS_GUI_FILE, NATIONAL_FOCUS_ROOT)
        .expect("focus root template");

    let politics_doc_nodes = context
        .document(COUNTRY_POLITICS_GUI_FILE)
        .unwrap()
        .node_count();
    let decision_doc_nodes = context
        .document(COUNTRY_DECISION_GUI_FILE)
        .unwrap()
        .node_count();
    let focus_doc_nodes = context
        .document(NATIONAL_FOCUS_GUI_FILE)
        .unwrap()
        .node_count();
    assert!(politics_doc_nodes >= politics_root.node_count());
    assert!(
        decision_doc_nodes >= 70,
        "decision_doc_nodes={decision_doc_nodes}"
    );
    assert!(focus_doc_nodes >= 120, "focus_doc_nodes={focus_doc_nodes}");

    for (descriptor, root) in [
        (&COUNTRY_POLITICS_DESCRIPTOR, politics_root),
        (&COUNTRY_DECISION_DESCRIPTOR, decision_root),
        (&NATIONAL_FOCUS_DESCRIPTOR, focus_root),
    ] {
        let profile_report = context.profile_report(descriptor);
        assert!(profile_report.gui_loaded, "{profile_report:?}");
        assert!(profile_report.root_loaded, "{profile_report:?}");
        assert!(
            profile_report.key_templates_missing.is_empty(),
            "{profile_report:?}"
        );
        let gfx_refs = collect_profile_gfx_references(&context, descriptor);
        let _ = writeln!(
            report,
            "- {}: nodes={} gui_gfx_refs={} required_sprite_hits={}/{} missing={:?}",
            descriptor.profile_id,
            context
                .document(descriptor.required_gui_files[0])
                .map(|document| document.node_count())
                .unwrap_or(root.node_count()),
            gfx_refs.len(),
            profile_report.gfx_hits.hits,
            profile_report.gfx_hits.requested,
            profile_report.gfx_hits.missing
        );
    }

    let viewport = GuiRect::new(0.0, 0.0, 1920.0, 1080.0);
    let politics_layout = hoi4_ui::vanilla_gui::compute_layout_tree(
        politics_root,
        &LayoutOptions::new(viewport).shown_position(true),
    );
    let decision_layout = hoi4_ui::vanilla_gui::compute_layout_tree(
        decision_root,
        &LayoutOptions::new(viewport).shown_position(true),
    );
    let focus_layout = hoi4_ui::vanilla_gui::compute_layout_tree(
        focus_root,
        &LayoutOptions::new(viewport).shown_position(true),
    );

    assert!(politics_layout.find_by_name("active_goal").is_some());
    assert!(decision_layout.find_by_name("decision_grid").is_some());
    assert!(focus_layout.find_by_name("tree").is_some());
    assert!(focus_layout.find_by_name("grid").is_some());

    let markers = required_focus_marker_report(focus_root);
    assert_eq!(markers.get("focus_spacing"), Some(&(96.0, 130.0)));
    assert_eq!(markers.get("national_focus_center"), Some(&(130.0, 32.0)));
    let _ = writeln!(report, "- national_focus_markers: {:?}", markers);

    std::fs::write(&report_path, report).unwrap();
}
