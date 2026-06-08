use std::collections::HashMap;

use clausewitz_parser::{Block, Value};

use super::ast::{GuiNode, GuiNodeKind, GuiNodePath, GuiValueExt};
use super::error::{VanillaGuiIssue, VanillaGuiIssueKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiDim {
    Px(f32),
    Percent(f32),
    Missing,
    Invalid,
}

impl GuiDim {
    pub fn resolve(self, parent_extent: f32, fallback: f32) -> f32 {
        match self {
            Self::Px(value) => value,
            Self::Percent(value) => parent_extent * value / 100.0,
            Self::Missing | Self::Invalid => fallback,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GuiPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GuiSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GuiRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl GuiRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn size(self) -> GuiSize {
        GuiSize {
            width: self.width,
            height: self.height,
        }
    }

    pub fn intersect(self, other: GuiRect) -> GuiRect {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        GuiRect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
    }
}

impl From<GuiRect> for egui::Rect {
    fn from(value: GuiRect) -> Self {
        egui::Rect::from_min_size(
            egui::pos2(value.x, value.y),
            egui::vec2(value.width, value.height),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiOrientation {
    UpperLeft,
    UpperRight,
    LowerLeft,
    LowerRight,
    Center,
}

impl GuiOrientation {
    pub fn parse(value: Option<String>) -> Self {
        let Some(value) = value else {
            return Self::UpperLeft;
        };
        match value
            .trim()
            .trim_matches('"')
            .replace('-', "_")
            .to_ascii_lowercase()
            .as_str()
        {
            "upper_right" | "right" => Self::UpperRight,
            "lower_left" | "bottom_left" | "left_bottom" | "lower" => Self::LowerLeft,
            "lower_right" | "bottom_right" | "right_bottom" => Self::LowerRight,
            "center" | "centre" | "middle" => Self::Center,
            _ => Self::UpperLeft,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub viewport: GuiRect,
    pub use_show_position: bool,
    pub visibility_overrides: HashMap<GuiNodePath, bool>,
    pub instance_overrides: HashMap<GuiNodePath, usize>,
}

impl LayoutOptions {
    pub fn new(viewport: GuiRect) -> Self {
        Self {
            viewport,
            use_show_position: false,
            visibility_overrides: HashMap::new(),
            instance_overrides: HashMap::new(),
        }
    }

    pub fn shown_position(mut self, value: bool) -> Self {
        self.use_show_position = value;
        self
    }
}

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub path: GuiNodePath,
    pub name: Option<String>,
    pub kind: GuiNodeKind,
    pub rect: GuiRect,
    pub clip_rect: GuiRect,
    pub visible: bool,
    pub scale: f32,
    pub scroll: Option<ScrollSpec>,
    pub children: Vec<LayoutNode>,
    pub issues: Vec<VanillaGuiIssue>,
}

impl LayoutNode {
    pub fn find_by_name(&self, name: &str) -> Option<&LayoutNode> {
        if self.name.as_deref() == Some(name) {
            return Some(self);
        }
        self.children
            .iter()
            .find_map(|child| child.find_by_name(name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollSpec {
    pub has_vertical_scrollbar: bool,
    pub scroll_wheel_factor: f32,
    pub margin: GuiMargin,
    pub smooth_scrolling: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GuiMargin {
    pub top: f32,
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiMarker {
    pub name: String,
    pub position: GuiPoint,
    pub rect: GuiRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiMarkerLookup {
    pub marker: GuiMarker,
    pub issues: Vec<VanillaGuiIssue>,
}

impl GuiMarkerLookup {
    pub fn position(&self) -> GuiPoint {
        self.marker.position
    }
}

pub fn position_marker(root: &GuiNode, name: &str, fallback: GuiPoint) -> GuiMarkerLookup {
    let mut issues = Vec::new();
    let marker = find_position_marker(root, name).unwrap_or_else(|| {
        issues.push(VanillaGuiIssue::new(
            VanillaGuiIssueKind::MissingLayoutMarker,
            format!("missing positionType marker `{name}`; using fallback"),
        ));
        GuiMarker {
            name: name.to_owned(),
            position: fallback,
            rect: GuiRect::new(fallback.x, fallback.y, 0.0, 0.0),
        }
    });
    GuiMarkerLookup { marker, issues }
}

pub fn focus_spacing_marker(root: &GuiNode) -> GuiMarkerLookup {
    position_marker(root, "focus_spacing", GuiPoint { x: 96.0, y: 130.0 })
}

pub fn national_focus_center_marker(root: &GuiNode) -> GuiMarkerLookup {
    position_marker(
        root,
        "national_focus_center",
        GuiPoint { x: 130.0, y: 32.0 },
    )
}

pub fn link_spacing_marker(root: &GuiNode) -> GuiMarkerLookup {
    position_marker(root, "link_spacing", GuiPoint { x: 16.0, y: 16.0 })
}

pub fn link_begin_marker(root: &GuiNode) -> GuiMarkerLookup {
    position_marker(root, "link_begin", GuiPoint { x: 80.0, y: 64.0 })
}

pub fn link_end_marker(root: &GuiNode) -> GuiMarkerLookup {
    position_marker(root, "link_end", GuiPoint { x: 80.0, y: 0.0 })
}

fn find_position_marker(node: &GuiNode, name: &str) -> Option<GuiMarker> {
    if matches!(node.kind, GuiNodeKind::Position) && node.name.as_deref() == Some(name) {
        let mut issues = Vec::new();
        let position = parse_point_block(node.block("position"), &mut issues);
        return Some(GuiMarker {
            name: name.to_owned(),
            position,
            rect: GuiRect::new(position.x, position.y, 0.0, 0.0),
        });
    }
    node.children
        .iter()
        .find_map(|child| find_position_marker(child, name))
}

pub fn compute_layout_tree(root: &GuiNode, options: &LayoutOptions) -> LayoutNode {
    let label = root.path_label(0);
    compute_layout_tree_with_path(root, options, GuiNodePath::root(label))
}

pub fn compute_layout_tree_with_path(
    root: &GuiNode,
    options: &LayoutOptions,
    root_path: GuiNodePath,
) -> LayoutNode {
    let path = root_path;
    layout_node(root, path, options.viewport, options.viewport, options)
}

pub fn grid_slots(node: &GuiNode, rect: GuiRect, count: usize) -> Vec<GuiRect> {
    let spec = GridSpec::from_node(node, rect.size());
    spec.slots(rect, count)
}

fn layout_node(
    node: &GuiNode,
    path: GuiNodePath,
    parent: GuiRect,
    parent_clip: GuiRect,
    options: &LayoutOptions,
) -> LayoutNode {
    let mut issues = Vec::new();
    let scale = node.f32("scale").unwrap_or(1.0).max(0.01);
    let position_key = if options.use_show_position && node.block("show_position").is_some() {
        "show_position"
    } else {
        "position"
    };
    let local = parse_point_block(node.block(position_key), &mut issues);
    let size = resolve_size(node, parent, scale, &mut issues);
    let orientation = GuiOrientation::parse(node.string("orientation"));
    let centerposition = node.bool("centerposition").unwrap_or(false);
    let hidden = node.bool("hide").unwrap_or(false);
    let visible = options
        .visibility_overrides
        .get(&path)
        .copied()
        .unwrap_or(!hidden);

    let mut x = local.x * scale;
    let mut y = local.y * scale;
    match orientation {
        GuiOrientation::UpperLeft => {}
        GuiOrientation::UpperRight => {
            x = edge_anchored_offset(parent.width, x, size.width);
        }
        GuiOrientation::LowerLeft => {
            y = edge_anchored_offset(parent.height, y, size.height);
        }
        GuiOrientation::LowerRight => {
            x = edge_anchored_offset(parent.width, x, size.width);
            y = edge_anchored_offset(parent.height, y, size.height);
        }
        GuiOrientation::Center => {
            x += parent.width * 0.5;
            y += parent.height * 0.5;
        }
    }
    if centerposition {
        x -= size.width * 0.5;
        y -= size.height * 0.5;
    }

    let rect = GuiRect::new(parent.x + x, parent.y + y, size.width, size.height);
    let clip_rect = if node.bool("clipping").unwrap_or(false) {
        parent_clip.intersect(rect)
    } else {
        parent_clip
    };

    let mut children = Vec::new();
    for (index, child) in node.children.iter().enumerate() {
        let child_label = child.path_label(index);
        let child_path = path.child(child_label);
        let child_layout = layout_node(child, child_path, rect, clip_rect, options);
        children.push(child_layout);
    }

    LayoutNode {
        path,
        name: node.name.clone(),
        kind: node.kind.clone(),
        rect,
        clip_rect,
        visible,
        scale,
        scroll: scroll_spec(node),
        children,
        issues,
    }
}

fn edge_anchored_offset(parent_extent: f32, offset: f32, child_extent: f32) -> f32 {
    if offset < 0.0 {
        parent_extent + offset
    } else {
        parent_extent - offset - child_extent
    }
}

fn resolve_size(
    node: &GuiNode,
    parent: GuiRect,
    scale: f32,
    issues: &mut Vec<VanillaGuiIssue>,
) -> GuiSize {
    let Some(block) = node.block("size") else {
        if matches!(
            node.kind,
            GuiNodeKind::Background | GuiNodeKind::GridBox | GuiNodeKind::OverlappingElementsBox
        ) {
            return parent.size();
        }
        let width = node.f32("maxWidth").unwrap_or(0.0);
        let height = node.f32("maxHeight").unwrap_or(0.0);
        return GuiSize {
            width: width * scale,
            height: height * scale,
        };
    };
    let width_dim = dim_from_block_any(block, &["width", "x"], issues);
    let height_dim = dim_from_block_any(block, &["height", "y"], issues);
    GuiSize {
        width: resolve_size_extent(width_dim, parent.width, 0.0).max(0.0) * scale,
        height: resolve_size_extent(height_dim, parent.height, 0.0).max(0.0) * scale,
    }
}

fn parse_point_block(block: Option<&Block>, issues: &mut Vec<VanillaGuiIssue>) -> GuiPoint {
    let Some(block) = block else {
        return GuiPoint::default();
    };
    let x = dim_from_block_any(block, &["x", "width"], issues).resolve(0.0, 0.0);
    let y = dim_from_block_any(block, &["y", "height"], issues).resolve(0.0, 0.0);
    GuiPoint { x, y }
}

fn dim_from_block_any(block: &Block, keys: &[&str], issues: &mut Vec<VanillaGuiIssue>) -> GuiDim {
    for key in keys {
        if let Some(dim) = dim_from_block_key(block, key, issues) {
            return dim;
        }
    }
    let axis_index = if keys
        .iter()
        .any(|key| key.eq_ignore_ascii_case("width") || key.eq_ignore_ascii_case("x"))
    {
        0
    } else {
        1
    };
    dim_from_block_value(block, axis_index, issues).unwrap_or(GuiDim::Missing)
}

fn dim_from_block(block: &Block, key: &str, issues: &mut Vec<VanillaGuiIssue>) -> GuiDim {
    dim_from_block_key(block, key, issues).unwrap_or(GuiDim::Missing)
}

fn dim_from_block_key(
    block: &Block,
    key: &str,
    issues: &mut Vec<VanillaGuiIssue>,
) -> Option<GuiDim> {
    let value = get_value_ci(block, key)?;
    Some(dim_from_value(value, key, issues))
}

fn dim_from_block_value(
    block: &Block,
    index: usize,
    issues: &mut Vec<VanillaGuiIssue>,
) -> Option<GuiDim> {
    let value = block.values.get(index)?;
    Some(dim_from_value(value, &format!("value[{index}]"), issues))
}

fn dim_from_value(value: &Value, key: &str, issues: &mut Vec<VanillaGuiIssue>) -> GuiDim {
    match value {
        Value::Integer(value) => GuiDim::Px(*value as f32),
        Value::Float(value) => GuiDim::Px(*value as f32),
        Value::Percent(value) => GuiDim::Percent(*value as f32),
        Value::String(value) => match parse_vanilla_numeric_prefix(value) {
            Ok(value) => GuiDim::Px(value),
            Err(_) => {
                issues.push(VanillaGuiIssue::new(
                    VanillaGuiIssueKind::InvalidPosition,
                    format!("invalid numeric value `{value}` for `{key}`"),
                ));
                GuiDim::Invalid
            }
        },
        _ => GuiDim::Invalid,
    }
}

fn resolve_size_extent(dim: GuiDim, parent_extent: f32, fallback: f32) -> f32 {
    match dim {
        GuiDim::Px(value) if value < 0.0 => parent_extent + value,
        _ => dim.resolve(parent_extent, fallback),
    }
}

fn parse_vanilla_numeric_prefix(value: &str) -> Result<f32, std::num::ParseFloatError> {
    let trimmed = value.trim();
    let end = trimmed
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.'))
        .map(|(idx, ch)| idx + ch.len_utf8())
        .last()
        .unwrap_or(0);
    trimmed[..end].parse::<f32>()
}

fn scroll_spec(node: &GuiNode) -> Option<ScrollSpec> {
    let has_vertical_scrollbar = node
        .children
        .iter()
        .any(|child| matches!(child.kind, GuiNodeKind::VerticalScrollbar))
        || node.prop("verticalScrollbar").is_some();
    if !has_vertical_scrollbar && node.prop("scroll_wheel_factor").is_none() {
        return None;
    }
    Some(ScrollSpec {
        has_vertical_scrollbar,
        scroll_wheel_factor: node.f32("scroll_wheel_factor").unwrap_or(1.0),
        margin: margin_from_node(node),
        smooth_scrolling: node.bool("smooth_scrolling").unwrap_or(false),
    })
}

fn margin_from_node(node: &GuiNode) -> GuiMargin {
    let Some(value) = node.prop("margin") else {
        return GuiMargin::default();
    };
    match value {
        Value::Integer(value) => GuiMargin {
            top: *value as f32,
            left: *value as f32,
            bottom: *value as f32,
            right: *value as f32,
        },
        Value::Float(value) => {
            let value = *value as f32;
            GuiMargin {
                top: value,
                left: value,
                bottom: value,
                right: value,
            }
        }
        Value::Block(block) => {
            let mut issues = Vec::new();
            let top = dim_from_block(block, "top", &mut issues).resolve(0.0, 0.0);
            let left = dim_from_block(block, "left", &mut issues).resolve(0.0, 0.0);
            let bottom = dim_from_block(block, "bottom", &mut issues).resolve(0.0, 0.0);
            let right = dim_from_block(block, "right", &mut issues).resolve(0.0, 0.0);
            GuiMargin {
                top,
                left,
                bottom,
                right,
            }
        }
        _ => GuiMargin::default(),
    }
}

#[derive(Debug, Clone, Copy)]
struct GridSpec {
    slot: GuiSize,
    columns: usize,
    rows: usize,
    add_horizontal: bool,
}

impl GridSpec {
    fn from_node(node: &GuiNode, fallback: GuiSize) -> Self {
        let mut issues = Vec::new();
        let slot = match node.block("slotsize") {
            Some(block) => GuiSize {
                width: dim_from_block_any(block, &["width", "x"], &mut issues)
                    .resolve(fallback.width, fallback.width)
                    .max(1.0),
                height: dim_from_block_any(block, &["height", "y"], &mut issues)
                    .resolve(fallback.height, fallback.height)
                    .max(1.0),
            },
            None => fallback,
        };
        let max_slots = node.block("max_slots");
        let explicit_columns = node
            .f32("max_slots_horizontal")
            .map(|value| value as usize)
            .or_else(|| {
                max_slots
                    .and_then(|block| get_value_ci(block, "x"))
                    .and_then(GuiValueExt::as_lossy_f32)
                    .map(|value| value as usize)
            });
        let explicit_rows = node
            .f32("max_slots_vertical")
            .map(|value| value as usize)
            .or_else(|| {
                max_slots
                    .and_then(|block| get_value_ci(block, "y"))
                    .and_then(GuiValueExt::as_lossy_f32)
                    .map(|value| value as usize)
            });
        let add_horizontal = node.bool("add_horizontal").unwrap_or(true);
        let columns = explicit_columns
            .unwrap_or_else(|| inferred_grid_extent(fallback.width, slot.width))
            .max(1);
        let rows = explicit_rows
            .unwrap_or_else(|| {
                if add_horizontal {
                    inferred_grid_extent(fallback.height, slot.height)
                } else {
                    usize::MAX
                }
            })
            .max(1);
        Self {
            slot,
            columns,
            rows,
            add_horizontal,
        }
    }

    fn slots(self, rect: GuiRect, count: usize) -> Vec<GuiRect> {
        let capacity = self.columns.saturating_mul(self.rows);
        let count = count.min(capacity);
        let mut out = Vec::with_capacity(count);
        for index in 0..count {
            let (col, row) = if self.add_horizontal {
                (index % self.columns, index / self.columns)
            } else {
                (index / self.rows, index % self.rows)
            };
            out.push(GuiRect::new(
                rect.x + col as f32 * self.slot.width,
                rect.y + row as f32 * self.slot.height,
                self.slot.width,
                self.slot.height,
            ));
        }
        out
    }
}

fn inferred_grid_extent(container: f32, slot: f32) -> usize {
    if container > 0.0 && slot > 0.0 {
        (container / slot).floor().max(1.0) as usize
    } else {
        usize::MAX
    }
}

fn get_value_ci<'a>(block: &'a Block, key: &str) -> Option<&'a Value> {
    block
        .entries
        .iter()
        .find(|entry| entry.key.eq_ignore_ascii_case(key))
        .map(|entry| &entry.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vanilla_gui::ast::parse_gui_str;

    #[test]
    fn root_uses_show_position_and_percent_height() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "countrypoliticsview"
        position = { x=-606 y=78 }
        show_position = { x=-6 y=78 }
        size = { width=550 height=100%% }
    }
}
"#,
        );
        let root = doc.template_index().get("countrypoliticsview").unwrap();
        let options =
            LayoutOptions::new(GuiRect::new(0.0, 0.0, 1920.0, 1080.0)).shown_position(true);
        let layout = compute_layout_tree(root, &options);
        assert_eq!(layout.rect.x, -6.0);
        assert_eq!(layout.rect.y, 78.0);
        assert_eq!(layout.rect.width, 550.0);
        assert_eq!(layout.rect.height, 1080.0);
    }

    #[test]
    fn upper_right_anchor_places_child_from_parent_right() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width=550 height=400 }
        buttonType = {
            name = "close"
            position = { x=8 y=10 }
            size = { width=24 height=24 }
            Orientation = "UPPER_RIGHT"
        }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap();
        let layout = compute_layout_tree(
            root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 800.0, 600.0)),
        );
        let close = layout.find_by_name("close").unwrap();
        assert_eq!(close.rect.x, 550.0 - 8.0 - 24.0);
        assert_eq!(close.rect.y, 10.0);
    }

    #[test]
    fn upper_right_negative_offset_matches_clausewitz_right_edge_offset() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width=550 height=400 }
        buttonType = {
            name = "close"
            position = { x=-42 y=9 }
            Orientation = "UPPER_RIGHT"
        }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap();
        let layout = compute_layout_tree(
            root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 800.0, 600.0)),
        );
        let close = layout.find_by_name("close").unwrap();
        assert_eq!(close.rect.x, 508.0);
        assert_eq!(close.rect.y, 9.0);
    }

    #[test]
    fn center_orientation_offsets_from_parent_center() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "EventWindow"
        position = { x=-282 y=-310 }
        size = { width=581 height=427 }
        Orientation = CENTER
    }
}
"#,
        );
        let root = doc.template_index().get("EventWindow").unwrap();
        let layout = compute_layout_tree(
            root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 1920.0, 1080.0)),
        );
        assert_eq!(layout.rect.x, 678.0);
        assert_eq!(layout.rect.y, 230.0);
        assert_eq!(layout.rect.width, 581.0);
        assert_eq!(layout.rect.height, 427.0);
    }

    #[test]
    fn vanilla_numeric_suffixes_keep_their_leading_value() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 100 height = 100 }
        iconType = {
            name = "overlay"
            position = { x = -2 y = 1s }
        }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap();
        let layout = compute_layout_tree(
            root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 100.0, 100.0)),
        );
        let overlay = layout.find_by_name("overlay").unwrap();

        assert_eq!(overlay.rect.x, -2.0);
        assert_eq!(overlay.rect.y, 1.0);
        assert!(overlay.issues.is_empty(), "{:?}", overlay.issues);
    }

    #[test]
    fn grid_slots_match_law_row_expectation() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "country_politics_idea_category_entry"
        size = { width=550 height=100 }
        gridboxtype = {
            name = "ideas_grid"
            position = { x=0 y=30 }
            size = { width=550 height=64 }
            slotsize = { width = 80 height = 64 }
            max_slots = { x = 7 y = 1 }
        }
    }
}
"#,
        );
        let grid = doc.find_node_by_name("ideas_grid").unwrap();
        let slots = grid_slots(grid, GuiRect::new(0.0, 30.0, 550.0, 64.0), 6);
        assert_eq!(slots.len(), 6);
        assert_eq!(slots[0], GuiRect::new(0.0, 30.0, 80.0, 64.0));
        assert_eq!(slots[5], GuiRect::new(400.0, 30.0, 80.0, 64.0));
    }

    #[test]
    fn grid_slots_infer_single_column_for_politics_party_grid() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "chart_explanation"
        size = { width=330 height=70 }
        gridboxtype = {
            name = "parties_grid"
            position = { x=260 y=184 }
            size = { width=100%% height=100%% }
            slotsize = { width=230 height=16 }
            format = "UPPER_LEFT"
        }
    }
}
"#,
        );
        let grid = doc.find_node_by_name("parties_grid").unwrap();
        let slots = grid_slots(grid, GuiRect::new(272.0, 378.0, 330.0, 70.0), 4);

        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0], GuiRect::new(272.0, 378.0, 230.0, 16.0));
        assert_eq!(slots[1], GuiRect::new(272.0, 394.0, 230.0, 16.0));
        assert_eq!(slots[2], GuiRect::new(272.0, 410.0, 230.0, 16.0));
        assert_eq!(slots[3], GuiRect::new(272.0, 426.0, 230.0, 16.0));
    }

    #[test]
    fn gate6_layout_accepts_clausewitz_case_and_xy_size_forms() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "root"
        size = { width = 400 height = 300 }
        editBoxType = {
            name = "search"
            position = { x = 10 y = 20 }
            size = { x = 150 y = 25 }
        }
        containerWindowType = {
            name = "lower"
            position = { x = 15 y = -30 }
            size = { 120 20 }
            orientation = lower_left
        }
        containerWindowType = {
            name = "centered"
            position = { x = -20 y = -10 }
            size = { width = 40 height = 20 }
            Orientation = center
        }
    }
}
"#,
        );
        let root = doc.template_index().get("root").unwrap();
        let layout = compute_layout_tree(
            root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 800.0, 600.0)),
        );

        let search = layout.find_by_name("search").unwrap();
        assert_eq!(search.rect, GuiRect::new(10.0, 20.0, 150.0, 25.0));
        let lower = layout.find_by_name("lower").unwrap();
        assert_eq!(lower.rect, GuiRect::new(15.0, 270.0, 120.0, 20.0));
        let centered = layout.find_by_name("centered").unwrap();
        assert_eq!(centered.rect, GuiRect::new(180.0, 140.0, 40.0, 20.0));
    }

    #[test]
    fn gate6_grid_slots_accept_xy_slot_size_and_case_insensitive_max_slots() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    gridBoxType = {
        name = "grid"
        size = { width = 300 height = 200 }
        slotsize = { x = 45 y = 30 }
        max_slots = { X = 2 Y = 2 }
    }
}
"#,
        );
        let grid = doc.find_node_by_name("grid").unwrap();
        let slots = grid_slots(grid, GuiRect::new(5.0, 10.0, 300.0, 200.0), 5);

        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0], GuiRect::new(5.0, 10.0, 45.0, 30.0));
        assert_eq!(slots[1], GuiRect::new(50.0, 10.0, 45.0, 30.0));
        assert_eq!(slots[2], GuiRect::new(5.0, 40.0, 45.0, 30.0));
    }

    #[test]
    fn gate6_margin_block_is_not_flattened_to_single_number() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = {
        name = "scroll"
        size = { width = 100 height = 100 }
        verticalScrollbar = "right_vertical_slider"
        margin = { top = 13 left = 0 bottom = 24 right = 25 }
    }
}
"#,
        );
        let node = doc.find_node_by_name("scroll").unwrap();
        let layout = compute_layout_tree(
            node,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 100.0, 100.0)),
        );
        let margin = layout.scroll.unwrap().margin;

        assert_eq!(
            margin,
            GuiMargin {
                top: 13.0,
                left: 0.0,
                bottom: 24.0,
                right: 25.0,
            }
        );
    }

    #[test]
    fn gate6_real_decision_and_focus_key_layout_nodes_are_reasonable() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };

        let Some(decision_gui_path) = path_cfg.find("interface/countrydecisionview.gui") else {
            return;
        };
        let decision_doc = crate::vanilla_gui::parse_gui_file(decision_gui_path).unwrap();
        let decision_root = decision_doc
            .template_index()
            .get("countrydecisionview")
            .unwrap();
        let decision_layout = compute_layout_tree(
            decision_root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 1920.0, 1080.0)).shown_position(true),
        );
        let decision_grid_container = decision_layout
            .find_by_name("decision_grid_container")
            .unwrap();
        assert_eq!(decision_grid_container.rect.x, -1.0);
        assert_eq!(decision_grid_container.rect.y, 123.0);
        assert!(decision_grid_container.rect.width > 540.0);
        assert!(decision_grid_container.rect.height > 900.0);

        let Some(focus_gui_path) = path_cfg.find("interface/nationalfocusview.gui") else {
            return;
        };
        let focus_doc = crate::vanilla_gui::parse_gui_file(focus_gui_path).unwrap();
        let focus_root = focus_doc.template_index().get("nationalfocusview").unwrap();
        let focus_layout = compute_layout_tree(
            focus_root,
            &LayoutOptions::new(GuiRect::new(0.0, 0.0, 1920.0, 1080.0)),
        );
        let tree = focus_layout.find_by_name("tree").unwrap();
        let grid_window = tree.find_by_name("grid_window").unwrap();
        let grid = grid_window.find_by_name("grid").unwrap();
        assert!(grid.rect.x >= tree.rect.x);
        assert!(grid.rect.y >= tree.rect.y);
        assert!(grid.rect.width >= 1.0);
        assert!(grid.rect.height >= 1.0);
    }

    #[test]
    fn gate7_position_type_markers_are_queryable() {
        let Ok(path_cfg) = hoi4_paths::PathConfig::resolve(Default::default()) else {
            return;
        };
        let Some(gui_path) = path_cfg.find("interface/nationalfocusview.gui") else {
            return;
        };
        let doc = crate::vanilla_gui::parse_gui_file(gui_path).unwrap();
        let root = doc.template_index().get("nationalfocusview").unwrap();

        assert_eq!(
            focus_spacing_marker(root).position(),
            GuiPoint { x: 96.0, y: 130.0 }
        );
        assert_eq!(
            national_focus_center_marker(root).position(),
            GuiPoint { x: 130.0, y: 32.0 }
        );
        assert_eq!(
            link_spacing_marker(root).position(),
            GuiPoint { x: 16.0, y: 16.0 }
        );
        assert_eq!(
            link_begin_marker(root).position(),
            GuiPoint { x: 80.0, y: 64.0 }
        );
        assert_eq!(
            link_end_marker(root).position(),
            GuiPoint { x: 80.0, y: 0.0 }
        );
    }

    #[test]
    fn gate7_missing_position_marker_returns_fallback_and_diagnostic() {
        let doc = parse_gui_str(
            None,
            r#"
guiTypes = {
    containerWindowType = { name = "nationalfocusview" }
}
"#,
        );
        let root = doc.template_index().get("nationalfocusview").unwrap();
        let lookup = position_marker(root, "focus_spacing", GuiPoint { x: 96.0, y: 130.0 });

        assert_eq!(lookup.position(), GuiPoint { x: 96.0, y: 130.0 });
        assert_eq!(lookup.issues.len(), 1);
        assert_eq!(
            lookup.issues[0].kind,
            VanillaGuiIssueKind::MissingLayoutMarker
        );
    }
}
