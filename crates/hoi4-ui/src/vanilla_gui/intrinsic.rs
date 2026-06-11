use std::collections::{BTreeMap, HashMap};

use egui::FontId;

use super::ast::{GuiNode, GuiNodeKind};
use super::binding::GuiBindingMap;
use super::gfx_index::{GfxIndex, GfxResource, GfxResourceKind};
use super::layout::{GuiPoint, GuiRect, GuiSize};
use crate::icons::IconBank;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiIntrinsicSizeSource {
    GfxSize,
    IconTexture,
    FontMetrics,
    MissingResourceFallback,
}

impl GuiIntrinsicSizeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GfxSize => "gfx_size",
            Self::IconTexture => "icon_texture",
            Self::FontMetrics => "font_metrics",
            Self::MissingResourceFallback => "missing_resource_fallback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiIntrinsicAnchor {
    TopLeft,
    PieChartCenterXTop,
}

impl GuiIntrinsicAnchor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::PieChartCenterXTop => "pie_chart_center_x_top",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiIntrinsicSize {
    pub resource_name: String,
    pub resource_kind: Option<GfxResourceKind>,
    pub size: GuiSize,
    pub source: GuiIntrinsicSizeSource,
    pub anchor: GuiIntrinsicAnchor,
    pub frame_count: Option<u32>,
    pub detail: String,
}

impl GuiIntrinsicSize {
    pub fn fallback(resource_name: impl Into<String>) -> Self {
        let resource_name = resource_name.into();
        Self {
            resource_name: resource_name.clone(),
            resource_kind: None,
            size: GuiSize {
                width: 1.0,
                height: 1.0,
            },
            source: GuiIntrinsicSizeSource::MissingResourceFallback,
            anchor: GuiIntrinsicAnchor::TopLeft,
            frame_count: None,
            detail: format!("missing `{resource_name}`; using 1x1 fallback"),
        }
    }

    pub fn is_positive(&self) -> bool {
        self.size.width > 0.0 && self.size.height > 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiIntrinsicDiagnostic {
    pub resource_name: String,
    pub source: GuiIntrinsicSizeSource,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiLayoutIntrinsics {
    enabled: bool,
    resources: HashMap<String, GuiIntrinsicSize>,
    diagnostics: Vec<GuiIntrinsicDiagnostic>,
}

impl GuiLayoutIntrinsics {
    pub fn empty() -> Self {
        Self {
            enabled: false,
            resources: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn enabled_empty() -> Self {
        Self {
            enabled: true,
            resources: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn from_gfx_index_for_tree(gfx_index: &GfxIndex, root: &GuiNode) -> Self {
        let names = collect_resource_names_from_node(root);
        let mut resolver = GuiIntrinsicSizeResolver::new(gfx_index, None);
        resolver.collect(names.iter().map(String::as_str))
    }

    pub fn from_gfx_index_for_tree_with_bindings(
        gfx_index: &GfxIndex,
        root: &GuiNode,
        bindings: &GuiBindingMap,
    ) -> Self {
        let names = collect_resource_names_from_node_with_bindings(root, bindings);
        let mut resolver = GuiIntrinsicSizeResolver::new(gfx_index, None);
        resolver.collect(names.iter().map(String::as_str))
    }

    pub fn from_gfx_index_and_icon_bank_for_tree(
        gfx_index: &GfxIndex,
        icon_bank: &mut IconBank,
        root: &GuiNode,
    ) -> Self {
        let names = collect_resource_names_from_node(root);
        let mut resolver = GuiIntrinsicSizeResolver::new(gfx_index, Some(icon_bank));
        resolver.collect(names.iter().map(String::as_str))
    }

    pub fn from_gfx_index_icon_bank_and_bindings_for_tree(
        gfx_index: &GfxIndex,
        icon_bank: &mut IconBank,
        root: &GuiNode,
        bindings: &GuiBindingMap,
    ) -> Self {
        let names = collect_resource_names_from_node_with_bindings(root, bindings);
        let mut resolver = GuiIntrinsicSizeResolver::new(gfx_index, Some(icon_bank));
        resolver.collect(names.iter().map(String::as_str))
    }

    pub fn insert(&mut self, intrinsic: GuiIntrinsicSize) {
        self.enabled = true;
        self.resources
            .insert(intrinsic.resource_name.clone(), intrinsic);
    }

    pub fn add_diagnostic(&mut self, diagnostic: GuiIntrinsicDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn resource(&self, resource_name: &str) -> Option<&GuiIntrinsicSize> {
        self.resources.get(resource_name)
    }

    pub fn resource_or_fallback(&self, resource_name: &str) -> GuiIntrinsicSize {
        self.resource(resource_name)
            .cloned()
            .unwrap_or_else(|| GuiIntrinsicSize::fallback(resource_name))
    }

    pub fn intrinsic_for_node(&self, node: &GuiNode) -> Option<GuiIntrinsicSize> {
        if !self.enabled {
            return None;
        }
        let resource_name = node_resource_name(node)?;
        Some(self.resource_or_fallback(&resource_name))
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn diagnostics(&self) -> &[GuiIntrinsicDiagnostic] {
        &self.diagnostics
    }

    pub fn resources(&self) -> &HashMap<String, GuiIntrinsicSize> {
        &self.resources
    }
}

pub struct GuiIntrinsicSizeResolver<'a> {
    gfx_index: &'a GfxIndex,
    icon_bank: Option<&'a mut IconBank>,
}

impl<'a> GuiIntrinsicSizeResolver<'a> {
    pub fn new(gfx_index: &'a GfxIndex, icon_bank: Option<&'a mut IconBank>) -> Self {
        Self {
            gfx_index,
            icon_bank,
        }
    }

    pub fn collect<'n>(&mut self, names: impl IntoIterator<Item = &'n str>) -> GuiLayoutIntrinsics {
        let mut out = GuiLayoutIntrinsics::empty();
        for name in names {
            let intrinsic = self.resolve(name);
            out.add_diagnostic(GuiIntrinsicDiagnostic {
                resource_name: intrinsic.resource_name.clone(),
                source: intrinsic.source,
                detail: intrinsic.detail.clone(),
            });
            out.insert(intrinsic);
        }
        out
    }

    pub fn resolve(&mut self, resource_name: &str) -> GuiIntrinsicSize {
        let resource = self.gfx_index.get(resource_name);
        if let Some(intrinsic) = resource.and_then(resource_declared_intrinsic_size) {
            return intrinsic;
        }
        if let Some(size) = self.icon_texture_size(resource_name, resource) {
            let frame_count = resource
                .and_then(|resource| resource.frame_count)
                .unwrap_or(1)
                .max(1);
            let size = if frame_count > 1 {
                GuiSize {
                    width: size.width / frame_count as f32,
                    height: size.height,
                }
            } else {
                size
            };
            return GuiIntrinsicSize {
                resource_name: resource_name.to_owned(),
                resource_kind: resource.map(|resource| resource.kind.clone()),
                size: normalize_intrinsic_size(size, resource.map(|resource| &resource.kind)),
                source: GuiIntrinsicSizeSource::IconTexture,
                anchor: intrinsic_anchor(resource.map(|resource| &resource.kind)),
                frame_count: resource.and_then(|resource| resource.frame_count),
                detail: format!(
                    "`{resource_name}` intrinsic from decoded texture, frame_count={frame_count}"
                ),
            };
        }
        GuiIntrinsicSize::fallback(resource_name)
    }

    fn icon_texture_size(
        &mut self,
        resource_name: &str,
        resource: Option<&GfxResource>,
    ) -> Option<GuiSize> {
        let icon_bank = self.icon_bank.as_deref_mut()?;
        let size = match resource.map(|resource| &resource.kind) {
            Some(GfxResourceKind::ProgressBar) => resource
                .and_then(|resource| resource.primary_texture.as_deref())
                .and_then(|texture| icon_bank.size_of_texture_file(texture))
                .map(|size| egui::vec2(size[0] as f32, size[1] as f32)),
            _ => icon_bank
                .size_of(resource_name)
                .map(|size| egui::vec2(size[0] as f32, size[1] as f32)),
        }?;
        Some(GuiSize {
            width: size.x,
            height: size.y,
        })
    }
}

fn resource_declared_intrinsic_size(resource: &GfxResource) -> Option<GuiIntrinsicSize> {
    let size = resource.size?;
    let size = GuiSize {
        width: size.x,
        height: size.y,
    };
    let size = normalize_intrinsic_size(size, Some(&resource.kind));
    (size.width > 0.0 && size.height > 0.0).then(|| GuiIntrinsicSize {
        resource_name: resource.name.clone(),
        resource_kind: Some(resource.kind.clone()),
        size,
        source: GuiIntrinsicSizeSource::GfxSize,
        anchor: intrinsic_anchor(Some(&resource.kind)),
        frame_count: resource.frame_count,
        detail: format!(
            "`{}` intrinsic from {} size=({:.1}, {:.1})",
            resource.name,
            resource.kind.label(),
            size.width,
            size.height
        ),
    })
}

fn normalize_intrinsic_size(size: GuiSize, kind: Option<&GfxResourceKind>) -> GuiSize {
    match kind {
        Some(GfxResourceKind::PieChart) => {
            let radius = size.width.min(size.height).max(0.0);
            GuiSize {
                width: radius * 2.0,
                height: radius * 2.0,
            }
        }
        _ => size,
    }
}

fn intrinsic_anchor(kind: Option<&GfxResourceKind>) -> GuiIntrinsicAnchor {
    match kind {
        Some(GfxResourceKind::PieChart) => GuiIntrinsicAnchor::PieChartCenterXTop,
        _ => GuiIntrinsicAnchor::TopLeft,
    }
}

pub fn collect_resource_names_from_node(root: &GuiNode) -> Vec<String> {
    let mut out = BTreeMap::<String, ()>::new();
    collect_resource_names(root, &mut out);
    out.into_keys().collect()
}

pub fn collect_resource_names_from_node_with_bindings(
    root: &GuiNode,
    bindings: &GuiBindingMap,
) -> Vec<String> {
    let mut out = BTreeMap::<String, ()>::new();
    collect_resource_names_with_bindings(
        root,
        &GuiNodePathSegments::root(root.path_label(0)),
        bindings,
        &mut out,
    );
    out.into_keys().collect()
}

fn collect_resource_names(node: &GuiNode, out: &mut BTreeMap<String, ()>) {
    collect_node_resource_names(node, out);
    for child in &node.children {
        collect_resource_names(child, out);
    }
}

fn collect_resource_names_with_bindings(
    node: &GuiNode,
    path: &GuiNodePathSegments,
    bindings: &GuiBindingMap,
    out: &mut BTreeMap<String, ()>,
) {
    collect_node_resource_names(node, out);
    let binding = bindings.for_node(&path.to_path(), node.name.as_deref());
    if let Some(sprite) = binding.sprite {
        out.insert(sprite, ());
    }
    for (index, child) in node.children.iter().enumerate() {
        collect_resource_names_with_bindings(
            child,
            &path.child(child.path_label(index)),
            bindings,
            out,
        );
    }
}

pub fn node_resource_name(node: &GuiNode) -> Option<String> {
    node_resource_names(node).into_iter().next()
}

pub fn node_resource_names(node: &GuiNode) -> Vec<String> {
    let mut out = Vec::new();
    for key in [
        "spriteType",
        "quadTextureSprite",
        "buttonSprite",
        "buttonTextSprite",
        "textureFile",
        "texturefile",
        "textureFile1",
        "textureFile2",
        "textureFile3",
    ] {
        if let Some(name) = node.string(key) {
            push_unique(&mut out, name);
        }
    }
    out
}

fn collect_node_resource_names(node: &GuiNode, out: &mut BTreeMap<String, ()>) {
    for name in node_resource_names(node) {
        out.insert(name, ());
    }
}

fn push_unique(out: &mut Vec<String>, name: String) {
    if !out.iter().any(|existing| existing == &name) {
        out.push(name);
    }
}

#[derive(Debug, Clone)]
struct GuiNodePathSegments(Vec<String>);

impl GuiNodePathSegments {
    fn root(label: String) -> Self {
        Self(vec![label])
    }

    fn child(&self, label: String) -> Self {
        let mut next = self.0.clone();
        next.push(label);
        Self(next)
    }

    fn to_path(&self) -> super::ast::GuiNodePath {
        super::ast::GuiNodePath(self.0.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontToken {
    Title,
    Header,
    BodyLarge,
    Body,
    Caption,
    Small,
}

impl FontToken {
    pub fn from_vanilla(font: Option<&str>) -> Self {
        match font.unwrap_or_default().to_ascii_lowercase().as_str() {
            "hoi_36header" | "hoi_36b" | "hoi_32b" => Self::Title,
            "hoi4_typewriter22" | "hoi_24b" | "hoi_24bs" | "hoi_22b" | "hoi_22bs" | "hoi_20b"
            | "hoi_20bs" => Self::Header,
            "hoi_18mbs" | "hoi_18b" | "hoi_18bs" | "hoi_16mbs" | "hoi_16b" | "hoi_16bs" => {
                Self::Body
            }
            "hoi4_typewriter16" => Self::BodyLarge,
            "hoi_14mbs" | "hoi_14" | "hoi_14b" | "hoi_14bs" => Self::Caption,
            "hoi_12mbs" | "hoi_12" | "hoi_12b" | "hoi_12bs" => Self::Small,
            _ => Self::Body,
        }
    }

    pub fn font_id(self) -> FontId {
        match self {
            Self::Title => crate::v9::TextRole::Title.font_id(),
            Self::Header => crate::v9::TextRole::Heading.font_id(),
            Self::BodyLarge => crate::v9::TextRole::Subheading.font_id(),
            Self::Body => crate::v9::TextRole::Body.font_id(),
            Self::Caption => crate::v9::TextRole::Caption.font_id(),
            Self::Small => crate::v9::TextRole::Small.font_id(),
        }
    }

    pub fn metrics(self) -> GuiFontMetrics {
        GuiFontMetrics::from_font_id(self, self.font_id())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiFontMetrics {
    pub token: FontToken,
    pub font_id: FontId,
    pub line_height: f32,
    pub ascender: f32,
    pub descender: f32,
}

impl GuiFontMetrics {
    pub fn from_font_id(token: FontToken, font_id: FontId) -> Self {
        let font_size = font_id.size;
        let line_height = (font_size * 1.15).max(1.0);
        Self {
            token,
            font_id,
            line_height,
            ascender: font_size * 0.82,
            descender: font_size * 0.18,
        }
    }

    pub fn scaled(mut self, scale: f32) -> Self {
        let scale = scale.max(0.01);
        self.font_id.size = (self.font_id.size * scale).max(1.0);
        self.line_height = (self.line_height * scale).max(1.0);
        self.ascender = (self.ascender * scale).max(0.0);
        self.descender = (self.descender * scale).max(0.0);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiTextHorizontalAlign {
    Left,
    Center,
    Right,
}

impl GuiTextHorizontalAlign {
    pub fn from_format(format: Option<&str>) -> Self {
        match format.unwrap_or_default().to_ascii_lowercase().as_str() {
            "right" => Self::Right,
            "center" | "centre" => Self::Center,
            _ => Self::Left,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiTextVerticalAlign {
    Top,
    Center,
    Bottom,
}

impl GuiTextVerticalAlign {
    pub fn from_node_value(value: Option<&str>) -> Self {
        match value.unwrap_or_default().to_ascii_lowercase().as_str() {
            "center" | "centre" | "middle" => Self::Center,
            "bottom" | "lower" => Self::Bottom,
            _ => Self::Top,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiTextLayout {
    pub box_rect: GuiRect,
    pub horizontal: GuiTextHorizontalAlign,
    pub vertical: GuiTextVerticalAlign,
    pub fixed_size: bool,
    pub font_token: FontToken,
    pub metrics: GuiFontMetrics,
}

impl GuiTextLayout {
    pub fn anchor_position(&self) -> GuiPoint {
        let x = match self.horizontal {
            GuiTextHorizontalAlign::Left => self.box_rect.x,
            GuiTextHorizontalAlign::Center => self.box_rect.x + self.box_rect.width * 0.5,
            GuiTextHorizontalAlign::Right => self.box_rect.right(),
        };
        let y = match self.vertical {
            GuiTextVerticalAlign::Top => self.box_rect.y,
            GuiTextVerticalAlign::Center => self.box_rect.y + self.box_rect.height * 0.5,
            GuiTextVerticalAlign::Bottom => self.box_rect.bottom(),
        };
        GuiPoint { x, y }
    }
}

pub fn resolve_text_layout(node: &GuiNode, layout_rect: GuiRect, scale: f32) -> GuiTextLayout {
    let scale = scale.max(0.01);
    let font_token = FontToken::from_vanilla(node.string("font").as_deref());
    let metrics = font_token.metrics().scaled(scale);
    let max_width = node
        .f32("maxWidth")
        .map(|width| width * scale)
        .unwrap_or(layout_rect.width);
    let max_height = node
        .f32("maxHeight")
        .map(|height| height * scale)
        .unwrap_or_else(|| {
            if layout_rect.height > 0.0 {
                layout_rect.height
            } else {
                metrics.line_height
            }
        });
    let box_rect = GuiRect::new(
        layout_rect.x,
        layout_rect.y,
        max_width.max(0.0),
        max_height.max(0.0),
    );
    GuiTextLayout {
        box_rect,
        horizontal: GuiTextHorizontalAlign::from_format(node.string("format").as_deref()),
        vertical: GuiTextVerticalAlign::from_node_value(
            node.string("vertical_alignment").as_deref(),
        ),
        fixed_size: node.bool("fixedsize").unwrap_or(false),
        font_token,
        metrics,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPixelSnapMode {
    Round,
    Floor,
    Ceil,
    Outward,
}

impl GuiPixelSnapMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Round => "round",
            Self::Floor => "floor",
            Self::Ceil => "ceil",
            Self::Outward => "outward",
        }
    }
}

pub fn snap_value(value: f32, pixels_per_point: f32, mode: GuiPixelSnapMode) -> f32 {
    let pixels_per_point = pixels_per_point.max(0.001);
    let physical = value * pixels_per_point;
    let snapped = match mode {
        GuiPixelSnapMode::Round | GuiPixelSnapMode::Outward => physical.round(),
        GuiPixelSnapMode::Floor => physical.floor(),
        GuiPixelSnapMode::Ceil => physical.ceil(),
    };
    snapped / pixels_per_point
}

pub fn snap_rect_to_physical_pixels(
    rect: GuiRect,
    pixels_per_point: f32,
    mode: GuiPixelSnapMode,
) -> GuiRect {
    match mode {
        GuiPixelSnapMode::Outward => GuiRect::new(
            snap_value(rect.x, pixels_per_point, GuiPixelSnapMode::Floor),
            snap_value(rect.y, pixels_per_point, GuiPixelSnapMode::Floor),
            (snap_value(rect.right(), pixels_per_point, GuiPixelSnapMode::Ceil)
                - snap_value(rect.x, pixels_per_point, GuiPixelSnapMode::Floor))
            .max(0.0),
            (snap_value(rect.bottom(), pixels_per_point, GuiPixelSnapMode::Ceil)
                - snap_value(rect.y, pixels_per_point, GuiPixelSnapMode::Floor))
            .max(0.0),
        ),
        mode => {
            let x = snap_value(rect.x, pixels_per_point, mode);
            let y = snap_value(rect.y, pixels_per_point, mode);
            let right = snap_value(rect.right(), pixels_per_point, mode);
            let bottom = snap_value(rect.bottom(), pixels_per_point, mode);
            GuiRect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
        }
    }
}

pub fn rect_is_physical_pixel_aligned(rect: GuiRect, pixels_per_point: f32) -> bool {
    [rect.x, rect.y, rect.right(), rect.bottom()]
        .into_iter()
        .all(|value| {
            let physical = value * pixels_per_point;
            (physical.round() - physical).abs() <= 0.001
        })
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiControlRects {
    pub layout_rect: GuiRect,
    pub paint_rect: GuiRect,
    pub visual_rect: GuiRect,
    pub hit_rect: GuiRect,
    pub resource_rect: Option<GuiRect>,
    pub text_rect: Option<GuiRect>,
    pub text_layout: Option<GuiTextLayout>,
}

impl GuiControlRects {
    pub fn from_layout_rect(rect: GuiRect) -> Self {
        Self {
            layout_rect: rect,
            paint_rect: rect,
            visual_rect: rect,
            hit_rect: rect,
            resource_rect: None,
            text_rect: None,
            text_layout: None,
        }
    }

    pub fn resolve(
        node: &GuiNode,
        layout_rect: GuiRect,
        intrinsic: Option<&GuiIntrinsicSize>,
        scale: f32,
        pixels_per_point: f32,
    ) -> Self {
        let mut rects = Self::from_layout_rect(layout_rect);
        let resource_rect = intrinsic
            .filter(|intrinsic| intrinsic.is_positive())
            .map(|_| layout_rect);
        let paint_mode = match node.kind {
            GuiNodeKind::Button | GuiNodeKind::CheckBox => GuiPixelSnapMode::Round,
            _ => GuiPixelSnapMode::Round,
        };
        rects.paint_rect = snap_rect_to_physical_pixels(layout_rect, pixels_per_point, paint_mode);
        rects.visual_rect = rects.paint_rect;
        rects.hit_rect = match node.kind {
            GuiNodeKind::Button | GuiNodeKind::CheckBox => snap_rect_to_physical_pixels(
                layout_rect,
                pixels_per_point,
                GuiPixelSnapMode::Outward,
            ),
            _ => rects.paint_rect,
        };
        rects.resource_rect = resource_rect.map(|rect| {
            snap_rect_to_physical_pixels(rect, pixels_per_point, GuiPixelSnapMode::Round)
        });
        if matches!(
            node.kind,
            GuiNodeKind::InstantTextbox | GuiNodeKind::EditBox
        ) {
            let mut text_layout = resolve_text_layout(node, layout_rect, scale);
            text_layout.box_rect = snap_rect_to_physical_pixels(
                text_layout.box_rect,
                pixels_per_point,
                GuiPixelSnapMode::Round,
            );
            rects.text_rect = Some(text_layout.box_rect);
            rects.text_layout = Some(text_layout);
        }
        rects
    }

    pub fn translate(&mut self, offset: GuiPoint) {
        translate_rect(&mut self.layout_rect, offset);
        translate_rect(&mut self.paint_rect, offset);
        translate_rect(&mut self.visual_rect, offset);
        translate_rect(&mut self.hit_rect, offset);
        if let Some(rect) = &mut self.resource_rect {
            translate_rect(rect, offset);
        }
        if let Some(rect) = &mut self.text_rect {
            translate_rect(rect, offset);
        }
        if let Some(text_layout) = &mut self.text_layout {
            translate_rect(&mut text_layout.box_rect, offset);
        }
    }
}

fn translate_rect(rect: &mut GuiRect, offset: GuiPoint) {
    rect.x += offset.x;
    rect.y += offset.y;
}
