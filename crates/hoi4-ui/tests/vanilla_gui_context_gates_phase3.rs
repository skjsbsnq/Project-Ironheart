use std::path::PathBuf;

use hoi4_ui::vanilla_gui::{
    bind_profile_tree_with_path_and_context, parse_gui_str, GuiBinding, GuiBindingMap,
    GuiInstanceContext, GuiNodePath, GuiPoint, GuiRect, GuiRuntimeFrame, GuiRuntimeFrameInput,
    GuiRuntimeInstanceSpec, GuiTemplateRegistry, TemplateInstanceOptions, VanillaPanelProfile,
    VanillaProfileDescriptor, VanillaTemplateInstance,
};

const POLITICS_GUI_FILE: &str = "interface/countrypoliticsview.gui";
const DECISIONS_GUI_FILE: &str = "interface/countrydecisionview.gui";
const FOCUS_GUI_FILE: &str = "interface/nationalfocusview.gui";

const PHASE3_DESCRIPTOR: VanillaProfileDescriptor = VanillaProfileDescriptor {
    profile_id: "phase3_profile",
    root_template: "countrypoliticsview",
    required_gui_files: &[POLITICS_GUI_FILE, DECISIONS_GUI_FILE, FOCUS_GUI_FILE],
    template_instances: &[
        VanillaTemplateInstance {
            template_name: "political_party_info_entry",
            count: 4,
        },
        VanillaTemplateInstance {
            template_name: "decision_item",
            count: 2,
        },
        VanillaTemplateInstance {
            template_name: "national_focus_item",
            count: 3,
        },
    ],
    required_sprites: &[],
    key_templates: &[
        "political_party_info_entry",
        "decision_item",
        "national_focus_item",
    ],
};

struct ContextProfile;

impl VanillaPanelProfile for ContextProfile {
    type Data = Vec<String>;
    type Command = String;

    fn root_template(&self) -> &'static str {
        "root"
    }

    fn required_gui_files(&self) -> &'static [&'static str] {
        &[]
    }

    fn bind_node(&self, _: &GuiNodePath, _: &Self::Data) -> GuiBinding {
        GuiBinding::default()
    }

    fn bind_node_with_context(
        &self,
        node_path: &GuiNodePath,
        data: &Self::Data,
        instance: Option<&GuiInstanceContext>,
    ) -> GuiBinding {
        let name = node_path.0.last().map(String::as_str).unwrap_or_default();
        let Some(instance) = instance else {
            return GuiBinding::default();
        };
        match name {
            "name" => {
                GuiBinding::default().text(data.get(instance.index).cloned().unwrap_or_default())
            }
            "btn_select" => GuiBinding::default()
                .click(format!("{}:{}", instance.template_name, instance.index)),
            "role" => {
                GuiBinding::default().text(instance.semantic_role.clone().unwrap_or_default())
            }
            _ => GuiBinding::default(),
        }
    }

    fn handle_action(
        &self,
        action: hoi4_ui::vanilla_gui::GuiAction,
        _: &Self::Data,
    ) -> Option<Self::Command> {
        Some(action.node_path.to_string())
    }
}

#[test]
fn gate13_template_registry_resolves_three_target_templates_across_documents() {
    let (root_doc, decision_doc, focus_doc) = phase3_documents();
    let registry = GuiTemplateRegistry::from_documents([
        (POLITICS_GUI_FILE, &root_doc),
        (DECISIONS_GUI_FILE, &decision_doc),
        (FOCUS_GUI_FILE, &focus_doc),
    ]);
    let root = root_doc
        .template_index()
        .get("countrypoliticsview")
        .unwrap();
    let bindings = GuiBindingMap::default();
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 800.0, 600.0),
            "phase3_profile",
            &bindings,
        )
        .with_profile_descriptor(&PHASE3_DESCRIPTOR)
        .with_template_registry(registry),
    );

    assert_eq!(frame.diagnostics.templates.templates.len(), 3);
    assert!(frame
        .diagnostics
        .templates
        .templates
        .iter()
        .any(
            |template| template.template_name == "political_party_info_entry"
                && template.source_gui_file.as_deref() == Some(POLITICS_GUI_FILE)
        ));
    assert!(frame
        .diagnostics
        .templates
        .templates
        .iter()
        .any(|template| template.template_name == "decision_item"
            && template.source_gui_file.as_deref() == Some(DECISIONS_GUI_FILE)));
    assert!(frame
        .diagnostics
        .templates
        .templates
        .iter()
        .any(|template| template.template_name == "national_focus_item"
            && template.source_gui_file.as_deref() == Some(FOCUS_GUI_FILE)));
}

#[test]
fn gate14_runtime_expands_grid_instances_from_binding_count_and_capacity() {
    let (root_doc, decision_doc, focus_doc) = phase3_documents();
    let root = root_doc
        .template_index()
        .get("countrypoliticsview")
        .unwrap();
    let registry = GuiTemplateRegistry::from_documents([
        (POLITICS_GUI_FILE, &root_doc),
        (DECISIONS_GUI_FILE, &decision_doc),
        (FOCUS_GUI_FILE, &focus_doc),
    ]);
    let mut bindings = GuiBindingMap::default();
    let grid_path = GuiNodePath::root("countrypoliticsview").child("parties_grid");
    bindings.insert_path(grid_path.clone(), GuiBinding::default().instances(7));
    let spec = GuiRuntimeInstanceSpec::grid("political_party_info_entry", grid_path.clone(), 0)
        .with_options(TemplateInstanceOptions::default().template_size(true))
        .with_semantic_role("party_row");
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 800.0, 600.0),
            "phase3_grid",
            &bindings,
        )
        .with_template_registry(registry)
        .add_instance_spec(spec),
    );

    let parties: Vec<_> = frame
        .generated_instances
        .iter()
        .filter(|instance| instance.template_name == "political_party_info_entry")
        .collect();
    assert_eq!(parties.len(), 4);
    assert_eq!(
        parties[0].path.to_string(),
        "countrypoliticsview.parties_grid.political_party_info_entry[0]"
    );
    assert_eq!(parties[1].slot_rect, GuiRect::new(20.0, 46.0, 230.0, 16.0));
    assert_eq!(parties[3].slot_rect, GuiRect::new(20.0, 78.0, 230.0, 16.0));
}

#[test]
fn gate14_runtime_expands_n_column_grid_and_clamps_capacity() {
    let doc = parse_gui_str(
        Some(PathBuf::from(POLITICS_GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 300 height = 200 }
        gridBoxType = {
            name = "grid"
            position = { x = 5 y = 10 }
            size = { width = 180 height = 80 }
            slotsize = { width = 60 height = 40 }
            max_slots = { x = 3 y = 2 }
        }
    }
    containerWindowType = {
        name = "decision_item"
        size = { width = 60 height = 40 }
    }
}
"#,
    );
    let root = doc.template_index().get("root").unwrap();
    let registry = GuiTemplateRegistry::from_documents([(POLITICS_GUI_FILE, &doc)]);
    let spec =
        GuiRuntimeInstanceSpec::grid("decision_item", GuiNodePath::root("root").child("grid"), 8);
    let bindings = GuiBindingMap::default();
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 300.0, 200.0),
            "phase3_n_column",
            &bindings,
        )
        .with_template_registry(registry)
        .add_instance_spec(spec),
    );

    assert_eq!(frame.generated_instances.len(), 6);
    assert_eq!(
        frame.generated_instances[0].slot_rect,
        GuiRect::new(5.0, 10.0, 60.0, 40.0)
    );
    assert_eq!(
        frame.generated_instances[2].slot_rect,
        GuiRect::new(125.0, 10.0, 60.0, 40.0)
    );
    assert_eq!(
        frame.generated_instances[5].slot_rect,
        GuiRect::new(125.0, 50.0, 60.0, 40.0)
    );
}

#[test]
fn gate15_runtime_expands_absolute_instances_with_zoom_scroll_and_focus_markers() {
    let (root_doc, decision_doc, focus_doc) = phase3_documents();
    let root = root_doc
        .template_index()
        .get("countrypoliticsview")
        .unwrap();
    let registry = GuiTemplateRegistry::from_documents([
        (POLITICS_GUI_FILE, &root_doc),
        (DECISIONS_GUI_FILE, &decision_doc),
        (FOCUS_GUI_FILE, &focus_doc),
    ]);
    let parent_path = GuiNodePath::root("nationalfocusview")
        .child("tree")
        .child("grid");
    let spec = GuiRuntimeInstanceSpec::absolute_rects(
        "national_focus_item",
        parent_path,
        [
            GuiRect::new(300.0, 200.0, 1.0, 1.0),
            GuiRect::new(420.0, 330.0, 1.0, 1.0),
        ],
    )
    .with_options(
        TemplateInstanceOptions::default()
            .template_size(true)
            .zoom(0.5)
            .transform_origin(GuiPoint { x: 200.0, y: 100.0 })
            .scroll_offset(GuiPoint { x: 10.0, y: 20.0 }),
    )
    .with_semantic_role("focus_node")
    .with_model_keys(["focus_a", "focus_b"]);
    let bindings = GuiBindingMap::default();
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 800.0, 600.0),
            "phase3_absolute",
            &bindings,
        )
        .with_template_registry(registry)
        .add_instance_spec(spec),
    );

    let focus = frame
        .generated_instances
        .iter()
        .find(|instance| instance.context.model_key.as_deref() == Some("focus_a"))
        .unwrap();
    assert_eq!(
        focus.path.to_string(),
        "nationalfocusview.tree.grid.national_focus_item[0]"
    );
    assert_eq!(focus.resolved_rect, GuiRect::new(240.0, 130.0, 82.5, 64.0));
    assert_eq!(focus.context.semantic_role.as_deref(), Some("focus_node"));
}

#[test]
fn gate16_profile_binding_receives_instance_context_without_path_string_parsing() {
    let (_, decision_doc, _) = phase3_documents();
    let template = decision_doc.template_index().get("decision_item").unwrap();
    let parent_path = GuiNodePath::root("countrydecisionview").child("decision_grid");
    let instance_path = parent_path.child("decision_item[1]");
    let context = GuiInstanceContext::new("decision_item", 1, parent_path)
        .with_semantic_role("decision_entry")
        .with_model_key("decision_two");
    let bindings = bind_profile_tree_with_path_and_context(
        &ContextProfile,
        template,
        &vec!["Decision One".to_owned(), "Decision Two".to_owned()],
        instance_path.clone(),
        Some(&context),
    );

    assert_eq!(
        bindings
            .for_node(&instance_path.child("name"), Some("name"))
            .text
            .as_deref(),
        Some("Decision Two")
    );
    assert_eq!(
        bindings
            .for_node(&instance_path.child("role"), Some("role"))
            .text
            .as_deref(),
        Some("decision_entry")
    );
    assert_eq!(
        bindings
            .for_node(&instance_path.child("btn_select"), Some("btn_select"))
            .click
            .as_ref()
            .map(|click| click.command.as_str()),
        Some("decision_item:1")
    );
}

#[test]
fn gate17_instance_diagnostics_explain_party_decision_and_focus_instances() {
    let (root_doc, decision_doc, focus_doc) = phase3_documents();
    let root = root_doc
        .template_index()
        .get("countrypoliticsview")
        .unwrap();
    let registry = GuiTemplateRegistry::from_documents([
        (POLITICS_GUI_FILE, &root_doc),
        (DECISIONS_GUI_FILE, &decision_doc),
        (FOCUS_GUI_FILE, &focus_doc),
    ]);
    let party_grid = GuiNodePath::root("countrypoliticsview").child("parties_grid");
    let decision_grid = GuiNodePath::root("countrydecisionview").child("decision_grid");
    let focus_grid = GuiNodePath::root("nationalfocusview")
        .child("tree")
        .child("grid");
    let specs = vec![
        GuiRuntimeInstanceSpec::grid("political_party_info_entry", party_grid, 4)
            .with_options(TemplateInstanceOptions::default().template_size(true))
            .with_semantic_role("party_row"),
        GuiRuntimeInstanceSpec::absolute_rects(
            "decision_item",
            decision_grid,
            [GuiRect::new(30.0, 100.0, 502.0, 41.0)],
        )
        .with_semantic_role("decision_entry")
        .with_model_keys(["decision_autobahn"]),
        GuiRuntimeInstanceSpec::absolute_rects(
            "national_focus_item",
            focus_grid,
            [GuiRect::new(300.0, 200.0, 1.0, 1.0)],
        )
        .with_options(TemplateInstanceOptions::default().template_size(true))
        .with_semantic_role("focus_node")
        .with_model_keys(["focus_industry"]),
    ];
    let bindings = GuiBindingMap::default();
    let frame = GuiRuntimeFrame::build(
        GuiRuntimeFrameInput::new(
            root,
            GuiRect::new(0.0, 0.0, 800.0, 600.0),
            "phase3_instances",
            &bindings,
        )
        .with_template_registry(registry)
        .with_instance_specs(specs),
    );

    let markdown = frame.diagnostics.instance_markdown();
    assert!(markdown.contains("political_party_info_entry[0]"));
    assert!(markdown.contains("party_row"));
    assert!(markdown.contains("decision_autobahn"));
    assert!(markdown.contains("focus_industry"));
    assert!(markdown.contains("slot=(20.0, 30.0, 230.0, 16.0)"));
    write_gate17_report(&markdown);
}

fn phase3_documents() -> (
    hoi4_ui::vanilla_gui::GuiDocument,
    hoi4_ui::vanilla_gui::GuiDocument,
    hoi4_ui::vanilla_gui::GuiDocument,
) {
    let politics = parse_gui_str(
        Some(PathBuf::from(POLITICS_GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrypoliticsview"
        size = { width = 550 height = 480 }
        gridBoxType = {
            name = "parties_grid"
            position = { x = 20 y = 30 }
            size = { width = 230 height = 64 }
            slotsize = { width = 230 height = 16 }
            max_slots = { x = 1 y = 4 }
            add_horizontal = no
        }
    }
    containerWindowType = {
        name = "political_party_info_entry"
        size = { width = 230 height = 16 }
        instantTextboxType = { name = "name" maxWidth = 160 maxHeight = 16 }
        iconType = { name = "color_block" size = { width = 16 height = 16 } }
    }
}
"#,
    );
    let decisions = parse_gui_str(
        Some(PathBuf::from(DECISIONS_GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "countrydecisionview"
        size = { width = 550 height = 480 }
        gridBoxType = {
            name = "decision_grid"
            size = { width = 502 height = 240 }
        }
    }
    containerWindowType = {
        name = "decision_item"
        size = { width = 502 height = 41 }
        instantTextboxType = { name = "name" maxWidth = 360 maxHeight = 20 }
        instantTextboxType = { name = "role" maxWidth = 160 maxHeight = 20 }
        buttonType = { name = "btn_select" size = { width = 502 height = 41 } }
    }
}
"#,
    );
    let focus = parse_gui_str(
        Some(PathBuf::from(FOCUS_GUI_FILE)),
        r#"
guiTypes = {
    containerWindowType = {
        name = "nationalfocusview"
        size = { width = 800 height = 600 }
        containerWindowType = {
            name = "tree"
            gridBoxType = { name = "grid" size = { width = 800 height = 600 } }
        }
    }
    containerWindowType = {
        name = "national_focus_item"
        size = { width = 165 height = 128 }
        iconType = {
            name = "bg"
            position = { x = 5 y = 40 }
            size = { width = 90 height = 60 }
        }
        buttonType = {
            name = "symbol"
            position = { x = 5 y = -44 }
            size = { width = 70 height = 70 }
            Orientation = center
            centerposition = yes
        }
        positionType = { name = "focus_spacing" position = { x = 96 y = 130 } }
        positionType = { name = "national_focus_center" position = { x = 130 y = 32 } }
        positionType = { name = "link_begin" position = { x = 80 y = 64 } }
        positionType = { name = "link_end" position = { x = 80 y = 0 } }
    }
}
"#,
    );
    (politics, decisions, focus)
}

fn write_gate17_report(markdown: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("target/clausewitz_gui/gate17_instance_report.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, markdown).unwrap();
}
