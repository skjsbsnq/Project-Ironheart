use std::collections::HashMap;

use egui::Color32;

use super::ast::GuiNodePath;

#[derive(Debug, Clone, PartialEq)]
pub struct GuiBinding {
    pub visible: Option<bool>,
    pub text: Option<String>,
    pub text_color: Option<Color32>,
    pub sprite: Option<String>,
    pub tint: Option<Color32>,
    pub frame: Option<u32>,
    pub progress: Option<f32>,
    pub pie_segments: Vec<(f32, Color32)>,
    pub tooltip: Option<String>,
    pub click: Option<GuiClickCommand>,
    pub instance_count: Option<usize>,
}

impl Default for GuiBinding {
    fn default() -> Self {
        Self {
            visible: None,
            text: None,
            text_color: None,
            sprite: None,
            tint: None,
            frame: None,
            progress: None,
            pie_segments: Vec::new(),
            tooltip: None,
            click: None,
            instance_count: None,
        }
    }
}

impl GuiBinding {
    pub fn visible(mut self, value: bool) -> Self {
        self.visible = Some(value);
        self
    }

    pub fn text(mut self, value: impl Into<String>) -> Self {
        self.text = Some(value.into());
        self
    }

    pub fn text_color(mut self, value: Color32) -> Self {
        self.text_color = Some(value);
        self
    }

    pub fn sprite(mut self, value: impl Into<String>) -> Self {
        self.sprite = Some(value.into());
        self
    }

    pub fn tint(mut self, value: Color32) -> Self {
        self.tint = Some(value);
        self
    }

    pub fn frame(mut self, value: u32) -> Self {
        self.frame = Some(value.max(1));
        self
    }

    pub fn progress(mut self, value: f32) -> Self {
        self.progress = Some(value.clamp(0.0, 1.0));
        self
    }

    pub fn tooltip(mut self, value: impl Into<String>) -> Self {
        self.tooltip = Some(value.into());
        self
    }

    pub fn click(mut self, command: impl Into<String>) -> Self {
        self.click = Some(GuiClickCommand {
            command: command.into(),
        });
        self
    }

    pub fn instances(mut self, count: usize) -> Self {
        self.instance_count = Some(count);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiClickCommand {
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiAction {
    pub node_path: GuiNodePath,
    pub kind: GuiActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiActionKind {
    Click,
    Hover,
}

#[derive(Debug, Clone, Default)]
pub struct GuiBindingMap {
    by_path: HashMap<GuiNodePath, GuiBinding>,
    by_name: HashMap<String, GuiBinding>,
}

impl GuiBindingMap {
    pub fn insert_path(&mut self, path: GuiNodePath, binding: GuiBinding) {
        self.by_path.insert(path, binding);
    }

    pub fn insert_name(&mut self, name: impl Into<String>, binding: GuiBinding) {
        self.by_name.insert(name.into(), binding);
    }

    pub fn for_node(&self, path: &GuiNodePath, name: Option<&str>) -> GuiBinding {
        let mut binding = name
            .and_then(|name| self.by_name.get(name))
            .cloned()
            .unwrap_or_default();
        if let Some(path_binding) = self.by_path.get(path) {
            merge_binding(&mut binding, path_binding);
        }
        binding
    }
}

fn merge_binding(into: &mut GuiBinding, other: &GuiBinding) {
    if other.visible.is_some() {
        into.visible = other.visible;
    }
    if other.text.is_some() {
        into.text = other.text.clone();
    }
    if other.text_color.is_some() {
        into.text_color = other.text_color;
    }
    if other.sprite.is_some() {
        into.sprite = other.sprite.clone();
    }
    if other.tint.is_some() {
        into.tint = other.tint;
    }
    if other.frame.is_some() {
        into.frame = other.frame;
    }
    if other.progress.is_some() {
        into.progress = other.progress;
    }
    if !other.pie_segments.is_empty() {
        into.pie_segments = other.pie_segments.clone();
    }
    if other.tooltip.is_some() {
        into.tooltip = other.tooltip.clone();
    }
    if other.click.is_some() {
        into.click = other.click.clone();
    }
    if other.instance_count.is_some() {
        into.instance_count = other.instance_count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_binding_overrides_name_binding() {
        let path = GuiNodePath::root("root").child("button");
        let mut map = GuiBindingMap::default();
        map.insert_name("close", GuiBinding::default().text("Close"));
        map.insert_path(
            path.clone(),
            GuiBinding::default()
                .text("X")
                .text_color(Color32::RED)
                .tint(Color32::BLUE)
                .visible(false),
        );

        let binding = map.for_node(&path, Some("close"));

        assert_eq!(binding.text.as_deref(), Some("X"));
        assert_eq!(binding.text_color, Some(Color32::RED));
        assert_eq!(binding.tint, Some(Color32::BLUE));
        assert_eq!(binding.visible, Some(false));
    }
}
