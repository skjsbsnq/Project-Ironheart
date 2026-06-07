use std::collections::BTreeMap;

use super::ast::{GuiNode, GuiNodePath};
use super::binding::{GuiAction, GuiBinding, GuiBindingMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VanillaTemplateInstance {
    pub template_name: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaProfileDescriptor {
    pub profile_id: &'static str,
    pub root_template: &'static str,
    pub required_gui_files: &'static [&'static str],
    pub template_instances: &'static [VanillaTemplateInstance],
}

pub trait VanillaPanelProfile {
    type Data;
    type Command;

    fn profile_id(&self) -> &'static str {
        self.root_template()
    }

    fn root_template(&self) -> &'static str;

    fn required_gui_files(&self) -> &'static [&'static str];

    fn template_instances(&self) -> &'static [VanillaTemplateInstance] {
        &[]
    }

    fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding;

    fn handle_action(&self, action: GuiAction, data: &Self::Data) -> Option<Self::Command>;

    fn uses_slide_animation(&self) -> bool {
        true
    }

    fn descriptor(&self) -> VanillaProfileDescriptor {
        VanillaProfileDescriptor {
            profile_id: self.profile_id(),
            root_template: self.root_template(),
            required_gui_files: self.required_gui_files(),
            template_instances: self.template_instances(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct VanillaProfileRegistry {
    profiles: BTreeMap<&'static str, VanillaProfileDescriptor>,
}

impl VanillaProfileRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P: VanillaPanelProfile>(&mut self, profile: &P) {
        self.profiles
            .insert(profile.profile_id(), profile.descriptor());
    }

    pub fn get(&self, profile_id: &str) -> Option<&VanillaProfileDescriptor> {
        self.profiles.get(profile_id)
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    pub fn profile_ids(&self) -> Vec<&'static str> {
        self.profiles.keys().copied().collect()
    }
}

pub fn bind_profile_tree<P: VanillaPanelProfile>(
    profile: &P,
    root: &GuiNode,
    data: &P::Data,
) -> GuiBindingMap {
    bind_profile_tree_with_path(profile, root, data, GuiNodePath::root(root.path_label(0)))
}

pub fn bind_profile_tree_with_path<P: VanillaPanelProfile>(
    profile: &P,
    root: &GuiNode,
    data: &P::Data,
    root_path: GuiNodePath,
) -> GuiBindingMap {
    let mut bindings = GuiBindingMap::default();
    bind_profile_node(profile, root, data, root_path, &mut bindings);
    bindings
}

fn bind_profile_node<P: VanillaPanelProfile>(
    profile: &P,
    node: &GuiNode,
    data: &P::Data,
    path: GuiNodePath,
    bindings: &mut GuiBindingMap,
) {
    bindings.insert_path(path.clone(), profile.bind_node(&path, data));
    for (index, child) in node.children.iter().enumerate() {
        bind_profile_node(
            profile,
            child,
            data,
            path.child(child.path_label(index)),
            bindings,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::binding::{GuiActionKind, GuiClickCommand};

    struct DummyProfile;

    impl VanillaPanelProfile for DummyProfile {
        type Data = String;
        type Command = String;

        fn root_template(&self) -> &'static str {
            "dummy_root"
        }

        fn required_gui_files(&self) -> &'static [&'static str] {
            &["interface/dummy.gui"]
        }

        fn template_instances(&self) -> &'static [VanillaTemplateInstance] {
            &[VanillaTemplateInstance {
                template_name: "dummy_row",
                count: 3,
            }]
        }

        fn bind_node(&self, node_path: &GuiNodePath, data: &Self::Data) -> GuiBinding {
            let path = node_path.to_string();
            if path.contains("hidden") {
                GuiBinding::default().visible(false)
            } else if path.contains("title") {
                GuiBinding::default()
                    .text(data)
                    .sprite("GFX_dummy")
                    .progress(0.42)
                    .tooltip("dummy tooltip")
                    .click("dummy_click")
                    .instances(3)
            } else {
                GuiBinding::default()
            }
        }

        fn handle_action(
            &self,
            action: crate::vanilla_gui::binding::GuiAction,
            _: &Self::Data,
        ) -> Option<Self::Command> {
            match action.kind {
                GuiActionKind::Click => Some(action.node_path.to_string()),
                GuiActionKind::Hover => None,
            }
        }
    }

    struct SecondProfile;

    impl VanillaPanelProfile for SecondProfile {
        type Data = ();
        type Command = ();

        fn root_template(&self) -> &'static str {
            "second_root"
        }

        fn required_gui_files(&self) -> &'static [&'static str] {
            &["interface/second.gui"]
        }

        fn bind_node(&self, _: &GuiNodePath, _: &Self::Data) -> GuiBinding {
            GuiBinding::default()
        }

        fn handle_action(&self, _: GuiAction, _: &Self::Data) -> Option<Self::Command> {
            None
        }
    }

    #[test]
    fn dummy_profile_stays_data_snapshot_based() {
        let profile = DummyProfile;
        let path = GuiNodePath::root("dummy_root").child("title");
        let binding = profile.bind_node(&path, &"Hello".to_owned());
        assert_eq!(binding.text.as_deref(), Some("Hello"));
        assert_eq!(binding.sprite.as_deref(), Some("GFX_dummy"));
        assert_eq!(binding.progress, Some(0.42));
        assert_eq!(binding.tooltip.as_deref(), Some("dummy tooltip"));
        assert_eq!(
            binding.click.as_ref().map(|click| click.command.as_str()),
            Some("dummy_click")
        );
        assert_eq!(binding.instance_count, Some(3));

        let hidden = profile.bind_node(
            &GuiNodePath::root("dummy_root").child("temporarily_hidden"),
            &"Hello".to_owned(),
        );
        assert_eq!(hidden.visible, Some(false));

        assert_eq!(profile.root_template(), "dummy_root");
        assert_eq!(profile.required_gui_files(), &["interface/dummy.gui"]);
        assert_eq!(profile.template_instances()[0].template_name, "dummy_row");
        assert!(profile.uses_slide_animation());
        assert_eq!(
            GuiClickCommand {
                command: "x".to_owned()
            }
            .command,
            "x"
        );
    }

    #[test]
    fn bind_profile_tree_uses_layout_path_labels() {
        let doc = crate::vanilla_gui::parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "dummy_root"
        instantTextboxType = { name = "title" }
    }
}
"#,
        );
        let root = doc.template_index().get("dummy_root").unwrap();
        let bindings = bind_profile_tree(&DummyProfile, &root, &"Hello".to_owned());
        let binding = bindings.for_node(
            &GuiNodePath::root("dummy_root").child("title"),
            Some("title"),
        );
        assert_eq!(binding.text.as_deref(), Some("Hello"));
        assert_eq!(binding.instance_count, Some(3));
    }

    #[test]
    fn registry_keeps_multiple_profile_descriptors() {
        let mut registry = VanillaProfileRegistry::new();
        registry.register(&DummyProfile);
        registry.register(&SecondProfile);

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.profile_ids(), vec!["dummy_root", "second_root"]);
        let dummy = registry.get("dummy_root").unwrap();
        assert_eq!(dummy.root_template, "dummy_root");
        assert_eq!(dummy.required_gui_files, &["interface/dummy.gui"]);
        assert_eq!(
            dummy.template_instances,
            &[VanillaTemplateInstance {
                template_name: "dummy_row",
                count: 3,
            }]
        );
    }
}
