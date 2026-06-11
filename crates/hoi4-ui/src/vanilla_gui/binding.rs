use std::collections::HashMap;

use egui::Color32;

use super::ast::GuiNodePath;
use super::layout::{GuiPoint, GuiSize};

#[derive(Debug, Clone, PartialEq)]
pub struct GuiBinding {
    pub visible: Option<bool>,
    pub enabled: Option<bool>,
    pub checked: Option<bool>,
    pub text: Option<String>,
    pub input_text: Option<String>,
    pub text_color: Option<Color32>,
    pub sprite: Option<String>,
    pub tint: Option<Color32>,
    pub frame: Option<u32>,
    pub progress: Option<f32>,
    pub pie_segments: Vec<(f32, Color32)>,
    pub tooltip: Option<String>,
    pub click: Option<GuiClickCommand>,
    pub instance_count: Option<usize>,
    pub layout_position: Option<GuiPoint>,
    pub layout_size: Option<GuiSize>,
}

impl Default for GuiBinding {
    fn default() -> Self {
        Self {
            visible: None,
            enabled: None,
            checked: None,
            text: None,
            input_text: None,
            text_color: None,
            sprite: None,
            tint: None,
            frame: None,
            progress: None,
            pie_segments: Vec::new(),
            tooltip: None,
            click: None,
            instance_count: None,
            layout_position: None,
            layout_size: None,
        }
    }
}

impl GuiBinding {
    pub fn visible(mut self, value: bool) -> Self {
        self.visible = Some(value);
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = Some(value);
        self
    }

    pub fn checked(mut self, value: bool) -> Self {
        self.checked = Some(value);
        self
    }

    pub fn text(mut self, value: impl Into<String>) -> Self {
        self.text = Some(value.into());
        self
    }

    pub fn input_text(mut self, value: impl Into<String>) -> Self {
        self.input_text = Some(value.into());
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

    pub fn layout_position(mut self, x: f32, y: f32) -> Self {
        self.layout_position = Some(GuiPoint { x, y });
        self
    }

    pub fn layout_size(mut self, width: f32, height: f32) -> Self {
        self.layout_size = Some(GuiSize { width, height });
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

    pub fn extend(&mut self, other: GuiBindingMap) {
        self.by_path.extend(other.by_path);
        self.by_name.extend(other.by_name);
    }

    pub fn visibility_overrides(&self) -> HashMap<GuiNodePath, bool> {
        self.by_path
            .iter()
            .filter_map(|(path, binding)| binding.visible.map(|visible| (path.clone(), visible)))
            .collect()
    }

    pub fn layout_position_overrides(&self) -> HashMap<GuiNodePath, GuiPoint> {
        self.by_path
            .iter()
            .filter_map(|(path, binding)| {
                binding
                    .layout_position
                    .map(|position| (path.clone(), position))
            })
            .collect()
    }

    pub fn layout_size_overrides(&self) -> HashMap<GuiNodePath, GuiSize> {
        self.by_path
            .iter()
            .filter_map(|(path, binding)| binding.layout_size.map(|size| (path.clone(), size)))
            .collect()
    }
}

fn merge_binding(into: &mut GuiBinding, other: &GuiBinding) {
    if other.visible.is_some() {
        into.visible = other.visible;
    }
    if other.enabled.is_some() {
        into.enabled = other.enabled;
    }
    if other.checked.is_some() {
        into.checked = other.checked;
    }
    if other.text.is_some() {
        into.text = other.text.clone();
    }
    if other.input_text.is_some() {
        into.input_text = other.input_text.clone();
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
    if other.layout_position.is_some() {
        into.layout_position = other.layout_position;
    }
    if other.layout_size.is_some() {
        into.layout_size = other.layout_size;
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
                .input_text("typed")
                .text_color(Color32::RED)
                .tint(Color32::BLUE)
                .visible(false)
                .enabled(false)
                .checked(true),
        );

        let binding = map.for_node(&path, Some("close"));

        assert_eq!(binding.text.as_deref(), Some("X"));
        assert_eq!(binding.input_text.as_deref(), Some("typed"));
        assert_eq!(binding.text_color, Some(Color32::RED));
        assert_eq!(binding.tint, Some(Color32::BLUE));
        assert_eq!(binding.visible, Some(false));
        assert_eq!(binding.enabled, Some(false));
        assert_eq!(binding.checked, Some(true));
    }
}
