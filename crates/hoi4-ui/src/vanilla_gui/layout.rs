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
        match value.as_deref().map(str::trim) {
            Some("UPPER_RIGHT") => Self::UpperRight,
            Some("LOWER_LEFT") => Self::LowerLeft,
            Some("LOWER_RIGHT") => Self::LowerRight,
            Some("CENTER") => Self::Center,
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
    pub margin: f32,
    pub smooth_scrolling: bool,
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
    let orientation = GuiOrientation::parse(node.string("Orientation"));
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
        if matches!(node.kind, GuiNodeKind::Background) {
            return parent.size();
        }
        let width = node.f32("maxWidth").unwrap_or(0.0);
        let height = node.f32("maxHeight").unwrap_or(0.0);
        return GuiSize {
            width: width * scale,
            height: height * scale,
        };
    };
    let width_dim = dim_from_block(block, "width", issues);
    let height_dim = dim_from_block(block, "height", issues);
    GuiSize {
        width: width_dim.resolve(parent.width, 0.0).max(0.0) * scale,
        height: height_dim.resolve(parent.height, 0.0).max(0.0) * scale,
    }
}

fn parse_point_block(block: Option<&Block>, issues: &mut Vec<VanillaGuiIssue>) -> GuiPoint {
    let Some(block) = block else {
        return GuiPoint::default();
    };
    let x = dim_from_block(block, "x", issues).resolve(0.0, 0.0);
    let y = dim_from_block(block, "y", issues).resolve(0.0, 0.0);
    GuiPoint { x, y }
}

fn dim_from_block(block: &Block, key: &str, issues: &mut Vec<VanillaGuiIssue>) -> GuiDim {
    let Some(value) = block.get(key) else {
        return GuiDim::Missing;
    };
    match value {
        Value::Integer(value) => GuiDim::Px(*value as f32),
        Value::Float(value) => GuiDim::Px(*value as f32),
        Value::Percent(value) => GuiDim::Percent(*value as f32),
        Value::String(value) => match value.parse::<f32>() {
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

fn scroll_spec(node: &GuiNode) -> Option<ScrollSpec> {
    let has_vertical_scrollbar = node
        .children
        .iter()
        .any(|child| matches!(child.kind, GuiNodeKind::VerticalScrollbar))
        || node.block("verticalScrollbar").is_some();
    if !has_vertical_scrollbar && node.prop("scroll_wheel_factor").is_none() {
        return None;
    }
    Some(ScrollSpec {
        has_vertical_scrollbar,
        scroll_wheel_factor: node.f32("scroll_wheel_factor").unwrap_or(1.0),
        margin: node.f32("margin").unwrap_or(0.0),
        smooth_scrolling: node.bool("smooth_scrolling").unwrap_or(false),
    })
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
                width: dim_from_block(block, "width", &mut issues)
                    .resolve(fallback.width, fallback.width)
                    .max(1.0),
                height: dim_from_block(block, "height", &mut issues)
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
                    .and_then(|block| block.get("x"))
                    .and_then(GuiValueExt::as_lossy_f32)
                    .map(|value| value as usize)
            });
        let explicit_rows = node
            .f32("max_slots_vertical")
            .map(|value| value as usize)
            .or_else(|| {
                max_slots
                    .and_then(|block| block.get("y"))
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
}
