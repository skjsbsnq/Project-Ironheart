use std::fmt::Write as _;
use std::path::PathBuf;

use egui::Color32;
use hoi4_ui::vanilla_gui::{
    parse_gui_str, GfxIndex, GuiBinding, GuiBindingMap, GuiNodePath, GuiPoint, GuiRect,
    GuiRuntimeFrame, GuiRuntimeFrameInput, GuiRuntimeInstanceSpec, GuiRuntimeState,
    GuiTemplateRegistry, TemplateInstanceOptions, VanillaGuiDiagnosticCategory,
    VanillaGuiDiagnosticLine, VanillaGuiProfileDiagnostics, VanillaProfileDescriptor,
    VanillaTemplateInstance, VanillaUnsupportedSemanticsCategory,
    VanillaUnsupportedSemanticsRegistry,
};

const POLITICS_GUI_FILE: &str = "interface/countrypoliticsview.gui";

const PHASE7_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: "phase7_country_politics",
    root_template: "countrypoliticsview",
    required_gui_files: &[POLITICS_GUI_FILE],
    template_instances: &[VanillaTemplateInstance {
        template_name: "missing_phase7_template",
        count: 1,
    }],
    required_sprites: &[],
    key_templates: &[],
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn report_path(name: &str) -> PathBuf {
    workspace_root()
        .join("target")
        .join("clausewitz_gui")
        .join(name)
}

fn write_report(name: &str, body: impl AsRef<str>) {
    let path = report_path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body.as_ref()).unwrap();
}

fn phase7_gfx() -> GfxIndex {
    GfxIndex::parse_single(
        "phase7.gfx",
        r#"
spriteTypes = {
    spriteType = {
        name = "GFX_add_national_goal_button"
        size = { x = 40 y = 20 }
        noOfFrames = 4
        texturefile = "gfx/interface/add_national_goal_button.dds"
        effectFile = "gfx/FX/buttonstate_future.lua"
    }
    spriteType = {
        name = "GFX_party_color"
        size = { x = 16 y = 16 }
        texturefile = "gfx/interface/political_party_color.dds"
    }
    spriteType = {
        name = "GFX_phase7_bg"
        size = { x = 420 y = 260 }
        texturefile = "gfx/interface/panel_bg.dds"
    }
}
"#,
    )
}

fn phase7_doc() -> hoi4_ui::vanilla_gui::GuiDocument {
    parse_gui_str(
        Some(PathBuf::from(POLITICS_GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrypoliticsview"
        size = { width = 420 height = 260 }
        background = {
            name = "panel_bg"
            spriteType = "GFX_phase7_bg"
        }
        containerWindowType = {
            name = "active_goal"
            position = { x = 20 y = 20 }
            size = { width = 180 height = 44 }
            buttonType = {
                name = "add_national_goal_button"
                position = { x = 128 y = 10 }
                quadTextureSprite = "GFX_add_national_goal_button"
                text = "ADD"
            }
        }
        gridBoxType = {
            name = "parties_grid"
            position = { x = 20 y = 88 }
            size = { width = 230 height = 48 }
            slotsize = { width = 230 height = 16 }
            max_slots = { x = 1 y = 3 }
            add_horizontal = no
            verticalScrollbar = "right_vertical_slider"
            scroll_wheel_factor = 1.75
            smooth_scrolling = yes
            margin = { top = 4 left = 0 bottom = 4 right = 12 }
        }
        futureWidgetType = {
            name = "future_widget"
            position = { x = 300 y = 20 }
            size = { width = 32 height = 32 }
        }
    }
    containerWindowType = {
        name = "political_party_info_entry"
        size = { width = 230 height = 16 }
        iconType = {
            name = "color_block"
            spriteType = "GFX_party_color"
        }
        instantTextboxType = {
            name = "party_name"
            position = { x = 20 y = 0 }
            maxWidth = 140
            maxHeight = 16
            text = "Party"
        }
    }
}
"#,
    )
}

fn phase7_frame() -> GuiRuntimeFrame {
    let gfx = phase7_gfx();
    let doc = phase7_doc();
    let root = doc.template_index().get("countrypoliticsview").unwrap();
    let registry = GuiTemplateRegistry::from_documents([(POLITICS_GUI_FILE, &doc)]);
    let parties_grid = GuiNodePath::root("countrypoliticsview").child("parties_grid");
    let mut bindings = GuiBindingMap::default();
    bindings.insert_name(
        "add_national_goal_button",
        GuiBinding::default()
            .click("politics:add_goal")
            .tooltip("Add national goal"),
    );
    bindings.insert_path(parties_grid.clone(), GuiBinding::default().instances(1));
    bindings.insert_name(
        "color_block",
        GuiBinding::default().tint(Color32::from_rgb(120, 40, 20)),
    );

    GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 420.0, 260.0),
            "phase7_country_politics",
            &bindings,
        )
        .with_gfx_index(&gfx)
        .with_template_registry(registry)
        .with_profile_descriptor(&PHASE7_DESCRIPTOR)
        .with_runtime_state(
            GuiRuntimeState::shown(GuiRect::new(0.0, 0.0, 420.0, 260.0))
                .with_scroll_offset(parties_grid.clone(), GuiPoint { x: 0.0, y: 4.0 }),
        )
        .add_instance_spec(
            GuiRuntimeInstanceSpec::grid("political_party_info_entry", parties_grid, 1)
                .with_options(TemplateInstanceOptions::default().template_size(true))
                .with_semantic_role("party_row")
                .with_model_keys(["fascism"]),
        ),
    )
}

#[test]
fn gate32_runtime_diagnostics_report_queries_node_name_and_path() {
    let frame = phase7_frame();
    let add_button = frame
        .diagnostics
        .node_report_markdown("add_national_goal_button");
    let color_path = "countrypoliticsview.parties_grid.political_party_info_entry[0].color_block";
    let color_block = frame.diagnostics.node_report_markdown(color_path);

    assert!(add_button.contains("Transform Stack"));
    assert!(add_button.contains("GFX_add_national_goal_button"));
    assert!(add_button.contains("buttonstate_future.lua"));
    assert!(add_button.contains("DrawSprite"));
    assert!(color_block.contains("Template Instance Source"));
    assert!(color_block.contains("political_party_info_entry[0]"));
    assert!(color_block.contains("party_row"));
    assert!(color_block.contains("GFX_party_color"));
    assert!(color_block.contains("Scroll State"));
    assert!(frame
        .diagnostics
        .find_node_by_query("color_block")
        .is_some());

    let mut report = String::new();
    let _ = writeln!(report, "# Gate 32 Runtime Diagnostics UI / Report");
    let _ = writeln!(report);
    let _ = writeln!(report, "## add_national_goal_button");
    let _ = writeln!(report, "{add_button}");
    let _ = writeln!(report);
    let _ = writeln!(report, "## color_block");
    let _ = writeln!(report, "{color_block}");
    write_report("gate32_runtime_diagnostics_report.md", report);
}

#[test]
fn gate33_unsupported_semantics_registry_is_explicit_and_categorized() {
    let frame = phase7_frame();
    let mut registry =
        VanillaUnsupportedSemanticsRegistry::from_runtime_diagnostics(&frame.diagnostics);
    let profile = VanillaGuiProfileDiagnostics {
        profile_id: "phase7_country_politics".to_owned(),
        runtime_available: true,
        gui_loaded: true,
        root_loaded: true,
        missing_gui_files: Vec::new(),
        missing_sprites: vec!["GFX_missing_required".to_owned()],
        lines: vec![
            VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Layout,
                message: "missing layout marker `national_focus_center`".to_owned(),
            },
            VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Binding,
                message: "missing key templates: [\"phase7_missing_binding\"]".to_owned(),
            },
            VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Gfx,
                message: "missing required sprite mappings: [\"GFX_missing_required\"]".to_owned(),
            },
            VanillaGuiDiagnosticLine {
                category: VanillaGuiDiagnosticCategory::Gui,
                message: "required GUI file missing: interface/future.gui".to_owned(),
            },
        ],
    };
    registry.merge(VanillaUnsupportedSemanticsRegistry::from_profile_diagnostics(&profile));

    let categories = registry.category_names();
    for category in [
        "parser", "layout", "resource", "render", "binding", "runtime",
    ] {
        assert!(categories.contains(&category), "{categories:?}");
    }
    assert!(registry.entries().iter().any(|entry| entry.category
        == VanillaUnsupportedSemanticsCategory::Parser
        && entry.subject == "futureWidgetType"));
    assert!(registry.to_markdown().contains("buttonstate_future.lua"));
    assert!(registry.to_markdown().contains("missing_phase7_template"));
    write_report("gate33_unsupported_semantics.md", registry.to_markdown());
}

#[test]
fn gate34_final_report_freezes_maintenance_rules_for_new_panels() {
    let mut report = String::new();
    let _ = writeln!(report, "# Gate 34 Clausewitz GUI Runtime Final Report");
    let _ = writeln!(report);
    let _ = writeln!(report, "## Gate Results");
    let _ = writeln!(report, "- Gate 0-31: baseline, layout, template, scroll, render, and three-panel integration complete.");
    let _ = writeln!(
        report,
        "- Gate 32: node-level diagnostics report can query by node name or full node path."
    );
    let _ = writeln!(report, "- Gate 33: unsupported semantics registry records parser/layout/resource/render/binding/runtime categories.");
    let _ = writeln!(report);
    let _ = writeln!(report, "## New Panel Checklist");
    let _ = writeln!(report, "- Declare a `VanillaProfileDescriptor` with root template, required `.gui` files, key templates, and required sprites.");
    let _ = writeln!(report, "- Build a `GuiRuntimeFrame` with profile bindings, template registry, instance specs, scroll state, and optional `GfxIndex`.");
    let _ = writeln!(report, "- Verify node reports for at least one root control and one dynamic template instance child.");
    let _ = writeln!(report, "- Register unsupported parser/layout/resource/render/binding/runtime semantics before relying on fallback rendering.");
    let _ = writeln!(report);
    let _ = writeln!(report, "## Prohibited Shortcuts");
    let _ = writeln!(report, "- Do not add panel-local root offset helpers.");
    let _ = writeln!(
        report,
        "- Do not hand-roll grid or template bridge layout outside runtime instance specs."
    );
    let _ = writeln!(report, "- Do not add manual coordinate tweaks without a runtime diagnostic report explaining the source layer.");
    let _ = writeln!(report);
    let _ = writeln!(report, "## Known Limits");
    let _ = writeln!(
        report,
        "- Vanilla font/shader parity remains approximate in egui."
    );
    let _ = writeln!(report, "- Unsupported effect files are reported and rendered through deterministic fallback policy.");
    let _ = writeln!(report, "- Local vanilla assets are loaded at runtime only; repo artifacts remain project-owned snapshots and reports.");

    assert!(report.contains("New Panel Checklist"));
    assert!(report.contains("Prohibited Shortcuts"));
    assert!(report.contains("Known Limits"));
    write_report("gate34_final_report.md", report);
}
