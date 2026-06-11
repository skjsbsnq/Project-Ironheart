use std::collections::HashMap;
use std::fmt::Write as _;

use super::animation::{AnimationPhase, AnimationSpec};
use super::ast::{GuiDocument, GuiNode, GuiNodePath};
use super::binding::GuiBindingMap;
use super::draw_command::{
    collect_draw_commands_for_tree, GuiDrawCommand, GuiDrawCommandDiagnostics,
};
use super::gfx_index::GfxIndex;
use super::intrinsic::{GuiControlRects, GuiIntrinsicDiagnostic, GuiLayoutIntrinsics};
use super::layout::{
    compute_layout_tree, compute_layout_tree_with_path, GuiPoint, GuiRect, LayoutNode,
    LayoutOptions, ScrollSpec,
};
use super::profile::{GuiInstanceContext, VanillaProfileDescriptor};
use super::template_instancer::{template_instance_path, TemplateInstanceOptions};
use crate::icons::IconBank;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiRuntimeRootPosition {
    Hidden,
    Shown,
    Current(GuiPoint),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiRuntimeState {
    pub root_position: GuiRuntimeRootPosition,
    pub phase: Option<AnimationPhase>,
    pub visible: bool,
    pub scroll_offsets: HashMap<GuiNodePath, GuiPoint>,
    pub viewport: GuiRect,
    pub pixels_per_point: f32,
}

impl GuiRuntimeState {
    pub fn shown(viewport: GuiRect) -> Self {
        Self {
            root_position: GuiRuntimeRootPosition::Shown,
            phase: Some(AnimationPhase::Open),
            visible: true,
            scroll_offsets: HashMap::new(),
            viewport,
            pixels_per_point: 1.0,
        }
    }

    pub fn hidden(viewport: GuiRect) -> Self {
        Self {
            root_position: GuiRuntimeRootPosition::Hidden,
            phase: Some(AnimationPhase::Closed),
            visible: false,
            scroll_offsets: HashMap::new(),
            viewport,
            pixels_per_point: 1.0,
        }
    }

    pub fn with_scroll_offset(mut self, path: GuiNodePath, offset: GuiPoint) -> Self {
        self.scroll_offsets.insert(path, offset);
        self
    }

    pub fn with_pixels_per_point(mut self, pixels_per_point: f32) -> Self {
        self.pixels_per_point = pixels_per_point.max(0.001);
        self
    }
}

pub struct GuiRuntimeFrameInput<'a> {
    pub root: &'a GuiNode,
    pub viewport: GuiRect,
    pub profile_id: &'static str,
    pub bindings: &'a GuiBindingMap,
    pub runtime_state: GuiRuntimeState,
    pub gfx_index: Option<&'a GfxIndex>,
    pub icon_bank: Option<&'a mut IconBank>,
    pub template_registry: Option<GuiTemplateRegistry<'a>>,
    pub profile_descriptor: Option<&'a VanillaProfileDescriptor>,
    pub instance_specs: Vec<GuiRuntimeInstanceSpec>,
}

impl<'a> GuiRuntimeFrameInput<'a> {
    pub fn new(
        root: &'a GuiNode,
        viewport: GuiRect,
        profile_id: &'static str,
        bindings: &'a GuiBindingMap,
    ) -> Self {
        Self {
            root,
            viewport,
            profile_id,
            bindings,
            runtime_state: GuiRuntimeState::shown(viewport),
            gfx_index: None,
            icon_bank: None,
            template_registry: None,
            profile_descriptor: None,
            instance_specs: Vec::new(),
        }
    }

    pub fn with_runtime_state(mut self, runtime_state: GuiRuntimeState) -> Self {
        self.runtime_state = runtime_state;
        self
    }

    pub fn with_gfx_index(mut self, gfx_index: &'a GfxIndex) -> Self {
        self.gfx_index = Some(gfx_index);
        self
    }

    pub fn with_icon_bank(mut self, icon_bank: &'a mut IconBank) -> Self {
        self.icon_bank = Some(icon_bank);
        self
    }

    pub fn with_template_registry(mut self, registry: GuiTemplateRegistry<'a>) -> Self {
        self.template_registry = Some(registry);
        self
    }

    pub fn with_profile_descriptor(mut self, descriptor: &'a VanillaProfileDescriptor) -> Self {
        self.profile_descriptor = Some(descriptor);
        self
    }

    pub fn add_instance_spec(mut self, spec: GuiRuntimeInstanceSpec) -> Self {
        self.instance_specs.push(spec);
        self
    }

    pub fn with_instance_specs(mut self, specs: Vec<GuiRuntimeInstanceSpec>) -> Self {
        self.instance_specs = specs;
        self
    }
}

#[derive(Debug, Clone)]
pub struct GuiTemplateRegistry<'a> {
    templates: HashMap<&'static str, GuiTemplateRef<'a>>,
}

#[derive(Debug, Clone)]
pub struct GuiTemplateRef<'a> {
    pub template_name: &'static str,
    pub source_gui_file: Option<&'static str>,
    pub node: &'a GuiNode,
}

impl<'a> GuiTemplateRegistry<'a> {
    pub fn from_documents(
        documents: impl IntoIterator<Item = (&'static str, &'a GuiDocument)>,
    ) -> Self {
        let mut templates = HashMap::new();
        for (source_gui_file, document) in documents {
            for root in &document.roots {
                collect_templates(source_gui_file, root, &mut templates);
            }
        }
        Self { templates }
    }

    pub fn get(&self, template_name: &str) -> Option<&GuiTemplateRef<'a>> {
        self.templates.get(template_name)
    }

    pub fn template_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.templates.keys().copied().collect();
        names.sort();
        names
    }
}

fn collect_templates<'a>(
    source_gui_file: &'static str,
    node: &'a GuiNode,
    out: &mut HashMap<&'static str, GuiTemplateRef<'a>>,
) {
    if let Some(name) = node.name.as_deref() {
        let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
        out.entry(leaked).or_insert(GuiTemplateRef {
            template_name: leaked,
            source_gui_file: Some(source_gui_file),
            node,
        });
    }
    for child in &node.children {
        collect_templates(source_gui_file, child, out);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GuiRuntimeInstanceSource {
    Descriptor { count: usize },
    Grid { count: usize },
    Absolute { rects: Vec<GuiRect> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiRuntimeInstanceSpec {
    pub template_name: &'static str,
    pub parent_path: GuiNodePath,
    pub source: GuiRuntimeInstanceSource,
    pub options: TemplateInstanceOptions,
    pub semantic_role: Option<String>,
    pub model_keys: Vec<String>,
}

impl GuiRuntimeInstanceSpec {
    pub fn grid(template_name: &'static str, parent_path: GuiNodePath, count: usize) -> Self {
        Self {
            template_name,
            parent_path,
            source: GuiRuntimeInstanceSource::Grid { count },
            options: TemplateInstanceOptions::default(),
            semantic_role: None,
            model_keys: Vec::new(),
        }
    }

    pub fn absolute_rects(
        template_name: &'static str,
        parent_path: GuiNodePath,
        rects: impl IntoIterator<Item = GuiRect>,
    ) -> Self {
        Self {
            template_name,
            parent_path,
            source: GuiRuntimeInstanceSource::Absolute {
                rects: rects.into_iter().collect(),
            },
            options: TemplateInstanceOptions::default(),
            semantic_role: None,
            model_keys: Vec::new(),
        }
    }

    pub fn descriptor(template_name: &'static str, parent_path: GuiNodePath, count: usize) -> Self {
        Self {
            template_name,
            parent_path,
            source: GuiRuntimeInstanceSource::Descriptor { count },
            options: TemplateInstanceOptions::default(),
            semantic_role: None,
            model_keys: Vec::new(),
        }
    }

    pub fn with_options(mut self, options: TemplateInstanceOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_semantic_role(mut self, role: impl Into<String>) -> Self {
        self.semantic_role = Some(role.into());
        self
    }

    pub fn with_model_keys<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.model_keys = keys.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Debug, Clone)]
pub struct GuiGeneratedInstance {
    pub template_name: &'static str,
    pub path: GuiNodePath,
    pub parent_path: GuiNodePath,
    pub slot_rect: GuiRect,
    pub resolved_rect: GuiRect,
    pub layout: LayoutNode,
    pub context: GuiInstanceContext,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiHitRegion {
    pub source_path: GuiNodePath,
    pub rect: GuiRect,
    pub command: String,
    pub tooltip: Option<String>,
    pub enabled: bool,
    pub z_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiScrollState {
    pub key: String,
    pub path: GuiNodePath,
    pub node_name: Option<String>,
    pub offset: GuiPoint,
    pub spec: ScrollSpec,
    pub content_clip_rect: GuiRect,
}

#[derive(Debug, Clone)]
pub struct GuiRuntimeFrame {
    pub profile_id: &'static str,
    pub visible: bool,
    pub root_layout: LayoutNode,
    pub generated_instances: Vec<GuiGeneratedInstance>,
    pub scroll_states: Vec<GuiScrollState>,
    pub hit_regions: Vec<GuiHitRegion>,
    pub draw_list: Vec<GuiDrawCommand>,
    pub diagnostics: GuiRuntimeDiagnostics,
}

impl GuiRuntimeFrame {
    pub fn build(mut input: GuiRuntimeFrameInput<'_>) -> Self {
        let animation = AnimationSpec::from_node(input.root);
        let runtime_state = input.runtime_state.clone();
        let root_position = match runtime_state.root_position {
            GuiRuntimeRootPosition::Hidden => animation.hidden_position,
            GuiRuntimeRootPosition::Shown => animation.shown_position,
            GuiRuntimeRootPosition::Current(point) => point,
        };
        let root_animation_offset = GuiPoint {
            x: root_position.x - animation.shown_position.x,
            y: root_position.y - animation.shown_position.y,
        };

        let intrinsics = match (input.gfx_index, input.icon_bank.as_deref_mut()) {
            (Some(gfx), Some(icon_bank)) => {
                GuiLayoutIntrinsics::from_gfx_index_icon_bank_and_bindings_for_tree(
                    gfx,
                    icon_bank,
                    input.root,
                    input.bindings,
                )
            }
            (Some(gfx), None) => GuiLayoutIntrinsics::from_gfx_index_for_tree_with_bindings(
                gfx,
                input.root,
                input.bindings,
            ),
            _ => GuiLayoutIntrinsics::empty(),
        };
        let layout_options = LayoutOptions::new(input.viewport)
            .shown_position(true)
            .with_scroll_offsets(runtime_state.scroll_offsets.clone())
            .with_intrinsics(intrinsics.clone())
            .with_pixels_per_point(runtime_state.pixels_per_point)
            .with_visibility_overrides(input.bindings.visibility_overrides())
            .with_position_overrides(input.bindings.layout_position_overrides())
            .with_size_overrides(input.bindings.layout_size_overrides());
        let mut root_layout = compute_layout_tree(input.root, &layout_options);
        translate_layout_tree(&mut root_layout, root_animation_offset);

        let scroll_states = collect_scroll_states(input.profile_id, &root_layout, &runtime_state);

        let mut specs = input.instance_specs.clone();
        if let Some(descriptor) = input.profile_descriptor {
            for instance in descriptor.template_instances {
                specs.push(GuiRuntimeInstanceSpec::descriptor(
                    instance.template_name,
                    GuiNodePath::root(descriptor.root_template),
                    instance.count,
                ));
            }
        }
        let generated_instances = match input.template_registry.as_ref() {
            Some(registry) => {
                generate_runtime_instances(registry, &specs, &root_layout, input.bindings, 1.0)
            }
            None => Vec::new(),
        };

        let mut draw_list = collect_draw_commands_for_tree(
            input.root,
            &root_layout,
            input.bindings,
            input.gfx_index,
            0.0,
        );
        for instance in &generated_instances {
            let mut instance_draws = collect_draw_commands_for_tree(
                input
                    .template_registry
                    .as_ref()
                    .and_then(|registry| registry.get(instance.template_name))
                    .map(|template| template.node)
                    .unwrap_or(input.root),
                &instance.layout,
                input.bindings,
                input.gfx_index,
                0.0,
            );
            draw_list.append(&mut instance_draws);
        }
        for (idx, command) in draw_list.iter_mut().enumerate() {
            command.z_index = idx;
        }
        let draw_commands = GuiDrawCommandDiagnostics::from_commands(&draw_list);

        let mut hit_regions = Vec::new();
        collect_hit_regions(&root_layout, input.bindings, &mut hit_regions);
        for instance in &generated_instances {
            collect_hit_regions(&instance.layout, input.bindings, &mut hit_regions);
        }
        for (idx, hit) in hit_regions.iter_mut().enumerate() {
            hit.z_index = idx;
        }

        let template_diagnostics = GuiTemplateDiagnostics::from_descriptor(
            input.profile_descriptor,
            input.template_registry.as_ref(),
        );
        let node_reports = collect_node_reports(
            &root_layout,
            &draw_list,
            &generated_instances,
            &scroll_states,
        );
        let diagnostics = GuiRuntimeDiagnostics {
            profile_id: input.profile_id.to_owned(),
            root_hidden_position: animation.hidden_position,
            root_shown_position: animation.shown_position,
            root_animation_offset,
            intrinsics,
            templates: template_diagnostics,
            instances: generated_instances
                .iter()
                .map(GuiInstanceDiagnostic::from_instance)
                .collect(),
            scroll_states: scroll_states.clone(),
            draw_commands,
            nodes: node_reports,
        };

        Self {
            profile_id: input.profile_id,
            visible: runtime_state.visible,
            root_layout,
            generated_instances,
            scroll_states,
            hit_regions,
            draw_list,
            diagnostics,
        }
    }

    pub fn transform_stack_markdown_for_name(&self, query: &str) -> String {
        self.diagnostics.node_report_markdown(query)
    }
}

pub fn generate_runtime_instances(
    registry: &GuiTemplateRegistry<'_>,
    specs: &[GuiRuntimeInstanceSpec],
    root_layout: &LayoutNode,
    bindings: &GuiBindingMap,
    pixels_per_point: f32,
) -> Vec<GuiGeneratedInstance> {
    let mut out = Vec::new();
    for spec in specs {
        let Some(template) = registry.get(spec.template_name) else {
            continue;
        };
        let Some(parent_layout) = find_layout_by_path(root_layout, &spec.parent_path) else {
            continue;
        };
        let count = match &spec.source {
            GuiRuntimeInstanceSource::Descriptor { count }
            | GuiRuntimeInstanceSource::Grid { count } => {
                let bound = bindings
                    .for_node(&spec.parent_path, parent_layout.name.as_deref())
                    .instance_count
                    .unwrap_or(*count);
                bound
            }
            GuiRuntimeInstanceSource::Absolute { rects } => rects.len(),
        };
        let mut rects = match &spec.source {
            GuiRuntimeInstanceSource::Absolute { rects } => rects.clone(),
            GuiRuntimeInstanceSource::Descriptor { .. } | GuiRuntimeInstanceSource::Grid { .. } => {
                inferred_grid_slots(template.node, parent_layout, count)
            }
        };
        if !matches!(spec.source, GuiRuntimeInstanceSource::Absolute { .. }) {
            rects.truncate(count);
        }

        for (index, slot_rect) in rects.into_iter().enumerate() {
            let model_key = spec.model_keys.get(index).cloned();
            let context =
                GuiInstanceContext::new(spec.template_name, index, spec.parent_path.clone())
                    .with_optional_semantic_role(spec.semantic_role.clone())
                    .with_optional_model_key(model_key);
            let path = template_instance_path(&spec.parent_path, spec.template_name, index);
            let mut options = LayoutOptions::new(slot_rect)
                .with_pixels_per_point(pixels_per_point)
                .with_intrinsics(GuiLayoutIntrinsics::empty())
                .with_visibility_overrides(bindings.visibility_overrides())
                .with_position_overrides(bindings.layout_position_overrides())
                .with_size_overrides(bindings.layout_size_overrides());
            let mut layout = compute_layout_tree_with_path(template.node, &options, path.clone());
            if spec.options.use_template_size
                && (layout.rect.width != slot_rect.width || layout.rect.height != slot_rect.height)
            {
                options = LayoutOptions::new(GuiRect::new(
                    slot_rect.x,
                    slot_rect.y,
                    layout.rect.width,
                    layout.rect.height,
                ))
                .with_pixels_per_point(pixels_per_point)
                .with_visibility_overrides(bindings.visibility_overrides())
                .with_position_overrides(bindings.layout_position_overrides())
                .with_size_overrides(bindings.layout_size_overrides());
                layout = compute_layout_tree_with_path(template.node, &options, path.clone());
            } else {
                layout.rect.width = slot_rect.width;
                layout.rect.height = slot_rect.height;
                layout.rects = GuiControlRects::from_layout_rect(layout.rect);
            }
            if needs_instance_transform(spec.options) {
                transform_instance_layout(&mut layout, spec.options);
            }
            if parent_layout.scroll.is_some() {
                translate_layout_tree(
                    &mut layout,
                    GuiPoint {
                        x: -parent_layout.scroll_offset.x,
                        y: -parent_layout.scroll_offset.y,
                    },
                );
            }
            apply_parent_clip(&mut layout, parent_layout.clip_rect);
            let resolved_rect = layout.rect;
            out.push(GuiGeneratedInstance {
                template_name: spec.template_name,
                path,
                parent_path: spec.parent_path.clone(),
                slot_rect,
                resolved_rect,
                layout,
                context,
            });
        }
    }
    out
}

trait OptionalContextBuilder {
    fn with_optional_semantic_role(self, role: Option<String>) -> Self;
    fn with_optional_model_key(self, model_key: Option<String>) -> Self;
}

impl OptionalContextBuilder for GuiInstanceContext {
    fn with_optional_semantic_role(mut self, role: Option<String>) -> Self {
        self.semantic_role = role;
        self
    }

    fn with_optional_model_key(mut self, model_key: Option<String>) -> Self {
        self.model_key = model_key;
        self
    }
}

fn inferred_grid_slots(
    template: &GuiNode,
    parent_layout: &LayoutNode,
    count: usize,
) -> Vec<GuiRect> {
    let template_layout = compute_layout_tree(
        template,
        &LayoutOptions::new(GuiRect::new(
            parent_layout.rect.x,
            parent_layout.rect.y,
            parent_layout.rect.width,
            parent_layout.rect.height,
        )),
    );
    let slot_w = template_layout.rect.width.max(1.0);
    let slot_h = template_layout.rect.height.max(1.0);
    let columns = if slot_w >= parent_layout.rect.width * 0.9 {
        1
    } else {
        (parent_layout.rect.width / slot_w).floor().max(1.0) as usize
    };
    let rows = if parent_layout.scroll.is_some() {
        count.max(1)
    } else {
        (parent_layout.rect.height / slot_h).floor().max(1.0) as usize
    };
    let capacity = columns.saturating_mul(rows).max(1);
    let count = count.min(capacity);
    (0..count)
        .map(|index| {
            let col = index % columns;
            let row = index / columns;
            GuiRect::new(
                parent_layout.rect.x + col as f32 * slot_w,
                parent_layout.rect.y + row as f32 * slot_h,
                slot_w,
                slot_h,
            )
        })
        .collect()
}

fn needs_instance_transform(options: TemplateInstanceOptions) -> bool {
    (options.zoom - 1.0).abs() > f32::EPSILON
        || options.scroll_offset.x.abs() > f32::EPSILON
        || options.scroll_offset.y.abs() > f32::EPSILON
}

fn transform_instance_layout(layout: &mut LayoutNode, options: TemplateInstanceOptions) {
    let origin = options.transform_origin;
    transform_layout_tree_with(layout, &|rect| {
        GuiRect::new(
            origin.x + (rect.x - origin.x) * options.zoom - options.scroll_offset.x,
            origin.y + (rect.y - origin.y) * options.zoom - options.scroll_offset.y,
            rect.width * options.zoom,
            rect.height * options.zoom,
        )
    });
}

fn transform_layout_tree_with(layout: &mut LayoutNode, f: &impl Fn(GuiRect) -> GuiRect) {
    layout.rect = f(layout.rect);
    layout.clip_rect = f(layout.clip_rect);
    layout.rects.layout_rect = f(layout.rects.layout_rect);
    layout.rects.paint_rect = f(layout.rects.paint_rect);
    layout.rects.visual_rect = f(layout.rects.visual_rect);
    layout.rects.hit_rect = f(layout.rects.hit_rect);
    if let Some(rect) = &mut layout.rects.resource_rect {
        *rect = f(*rect);
    }
    if let Some(rect) = &mut layout.rects.text_rect {
        *rect = f(*rect);
    }
    if let Some(text) = &mut layout.rects.text_layout {
        text.box_rect = f(text.box_rect);
    }
    for child in &mut layout.children {
        transform_layout_tree_with(child, f);
    }
}

fn apply_parent_clip(layout: &mut LayoutNode, parent_clip: GuiRect) {
    layout.clip_rect = layout.clip_rect.intersect(parent_clip);
    layout.rects.hit_rect = layout.rects.hit_rect.intersect(parent_clip);
    layout.rects.paint_rect = layout.rects.paint_rect.intersect(parent_clip);
    layout.rects.visual_rect = layout.rects.visual_rect.intersect(parent_clip);
    if let Some(rect) = &mut layout.rects.resource_rect {
        *rect = rect.intersect(parent_clip);
    }
    if let Some(rect) = &mut layout.rects.text_rect {
        *rect = rect.intersect(parent_clip);
    }
    if let Some(text) = &mut layout.rects.text_layout {
        text.box_rect = text.box_rect.intersect(parent_clip);
    }
    for child in &mut layout.children {
        apply_parent_clip(child, parent_clip);
    }
}

fn translate_layout_tree(layout: &mut LayoutNode, offset: GuiPoint) {
    layout.rect.x += offset.x;
    layout.rect.y += offset.y;
    layout.clip_rect.x += offset.x;
    layout.clip_rect.y += offset.y;
    layout.rects.translate(offset);
    for child in &mut layout.children {
        translate_layout_tree(child, offset);
    }
}

fn rebase_layout_paths(layout: &mut LayoutNode, path: &GuiNodePath) {
    layout.path = path.clone();
    for (idx, child) in layout.children.iter_mut().enumerate() {
        let label = child.name.clone().unwrap_or_else(|| format!("child#{idx}"));
        rebase_layout_paths(child, &path.child(label));
    }
}

fn find_layout_by_path<'a>(layout: &'a LayoutNode, path: &GuiNodePath) -> Option<&'a LayoutNode> {
    if &layout.path == path {
        return Some(layout);
    }
    layout
        .children
        .iter()
        .find_map(|child| find_layout_by_path(child, path))
}

fn collect_scroll_states(
    profile_id: &str,
    layout: &LayoutNode,
    state: &GuiRuntimeState,
) -> Vec<GuiScrollState> {
    let mut out = Vec::new();
    collect_scroll_states_node(profile_id, layout, state, &mut out);
    out
}

fn collect_scroll_states_node(
    profile_id: &str,
    layout: &LayoutNode,
    state: &GuiRuntimeState,
    out: &mut Vec<GuiScrollState>,
) {
    if let Some(spec) = layout.scroll {
        out.push(GuiScrollState {
            key: format!("{profile_id}:{}", layout.path),
            path: layout.path.clone(),
            node_name: layout.name.clone(),
            offset: state
                .scroll_offsets
                .get(&layout.path)
                .copied()
                .unwrap_or(layout.scroll_offset),
            spec,
            content_clip_rect: layout.clip_rect,
        });
    }
    for child in &layout.children {
        collect_scroll_states_node(profile_id, child, state, out);
    }
}

fn collect_hit_regions(layout: &LayoutNode, bindings: &GuiBindingMap, out: &mut Vec<GuiHitRegion>) {
    let binding = bindings.for_node(&layout.path, layout.name.as_deref());
    if let Some(click) = binding.click.as_ref() {
        out.push(GuiHitRegion {
            source_path: layout.path.clone(),
            rect: layout.rects.hit_rect,
            command: click.command.clone(),
            tooltip: binding.tooltip.clone(),
            enabled: binding.enabled.unwrap_or(true),
            z_index: out.len(),
        });
    }
    for child in &layout.children {
        collect_hit_regions(child, bindings, out);
    }
}

#[derive(Debug, Clone)]
pub struct GuiRuntimeDiagnostics {
    pub profile_id: String,
    pub root_hidden_position: GuiPoint,
    pub root_shown_position: GuiPoint,
    pub root_animation_offset: GuiPoint,
    pub intrinsics: GuiLayoutIntrinsics,
    pub templates: GuiTemplateDiagnostics,
    pub instances: Vec<GuiInstanceDiagnostic>,
    pub scroll_states: Vec<GuiScrollState>,
    pub draw_commands: GuiDrawCommandDiagnostics,
    pub nodes: Vec<GuiNodeRuntimeReport>,
}

impl GuiRuntimeDiagnostics {
    pub fn coordinate_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "## Coordinate Spaces");
        let _ = writeln!(
            out,
            "- root_hidden_position=({:.1}, {:.1})",
            self.root_hidden_position.x, self.root_hidden_position.y
        );
        let _ = writeln!(
            out,
            "- root_shown_position=({:.1}, {:.1})",
            self.root_shown_position.x, self.root_shown_position.y
        );
        let _ = writeln!(
            out,
            "- root_animation_offset=({:.1}, {:.1})",
            self.root_animation_offset.x, self.root_animation_offset.y
        );
        out
    }

    pub fn intrinsic_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Intrinsic Sizes");
        for GuiIntrinsicDiagnostic {
            resource_name,
            source,
            detail,
        } in self.intrinsics.diagnostics()
        {
            let _ = writeln!(out, "- [{}] {}: {}", source.as_str(), resource_name, detail);
        }
        out
    }

    pub fn instance_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Runtime Template Instances");
        for instance in &self.instances {
            let _ = writeln!(
                out,
                "- {} role={} model={} slot=({:.1}, {:.1}, {:.1}, {:.1}) resolved=({:.1}, {:.1}, {:.1}, {:.1})",
                instance.path,
                instance.semantic_role.as_deref().unwrap_or("<none>"),
                instance.model_key.as_deref().unwrap_or("<none>"),
                instance.slot_rect.x,
                instance.slot_rect.y,
                instance.slot_rect.width,
                instance.slot_rect.height,
                instance.resolved_rect.x,
                instance.resolved_rect.y,
                instance.resolved_rect.width,
                instance.resolved_rect.height
            );
        }
        out
    }

    pub fn scroll_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Runtime Scroll States");
        for state in &self.scroll_states {
            let _ = writeln!(
                out,
                "- {} path={} offset=({:.1}, {:.1}) wheel_factor={:.2} smooth={} clip=({:.1}, {:.1}, {:.1}, {:.1})",
                state.node_name.as_deref().unwrap_or("<unnamed>"),
                state.path,
                state.offset.x,
                state.offset.y,
                state.spec.scroll_wheel_factor,
                state.spec.smooth_scrolling,
                state.content_clip_rect.x,
                state.content_clip_rect.y,
                state.content_clip_rect.width,
                state.content_clip_rect.height
            );
        }
        out
    }

    pub fn draw_command_markdown(&self) -> String {
        self.draw_commands.to_markdown()
    }

    pub fn unsupported_effects_markdown(&self) -> String {
        self.draw_commands.unsupported_effects_markdown()
    }

    pub fn find_node_by_name(&self, name: &str) -> Option<&GuiNodeRuntimeReport> {
        self.nodes
            .iter()
            .find(|node| node.name.as_deref() == Some(name))
    }

    pub fn find_node_by_query(&self, query: &str) -> Option<&GuiNodeRuntimeReport> {
        self.nodes
            .iter()
            .find(|node| node.name.as_deref() == Some(query) || node.path.to_string() == query)
    }

    pub fn node_report_markdown(&self, query: &str) -> String {
        let Some(node) = self.find_node_by_query(query) else {
            return format!("# Transform Stack\n\n- query `{query}` not found\n");
        };
        let mut out = String::new();
        let _ = writeln!(out, "# Transform Stack");
        let _ = writeln!(out, "- query: {query}");
        let _ = writeln!(out, "- path: {}", node.path);
        if let Some(name) = &node.name {
            let _ = writeln!(out, "- name: {name}");
        }
        let _ = writeln!(
            out,
            "- root_animation_offset=({:.1}, {:.1})",
            self.root_animation_offset.x, self.root_animation_offset.y
        );
        let _ = writeln!(
            out,
            "- final_rect [screen]=({:.1}, {:.1}, {:.1}, {:.1})",
            node.rect.x, node.rect.y, node.rect.width, node.rect.height
        );
        let _ = writeln!(
            out,
            "- hit_rect=({:.1}, {:.1}, {:.1}, {:.1})",
            node.hit_rect.x, node.hit_rect.y, node.hit_rect.width, node.hit_rect.height
        );
        if let Some(resource) = &node.resource_name {
            let _ = writeln!(out, "- resource: {resource}");
        }
        if let Some(effect) = &node.effect_file {
            let _ = writeln!(out, "- effectFile: {effect}");
        }
        if let Some(instance) = &node.instance_path {
            let _ = writeln!(out, "## Template Instance Source");
            let _ = writeln!(out, "- instance: {instance}");
            let _ = writeln!(
                out,
                "- role: {}",
                node.semantic_role.as_deref().unwrap_or("<none>")
            );
        }
        if !self.scroll_states.is_empty() {
            let _ = writeln!(out, "## Scroll State");
            for state in &self.scroll_states {
                if node.path.to_string().starts_with(&state.path.to_string())
                    || node
                        .instance_parent_path
                        .as_ref()
                        .is_some_and(|path| path == &state.path)
                {
                    let _ = writeln!(
                        out,
                        "- {} offset=({:.1}, {:.1})",
                        state.path, state.offset.x, state.offset.y
                    );
                }
            }
        }
        if let Some(draw_kind) = &node.draw_kind {
            let _ = writeln!(out, "## Draw");
            let _ = writeln!(out, "- {draw_kind}");
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct GuiTemplateDiagnostics {
    pub templates: Vec<GuiTemplateDiagnostic>,
    pub missing_templates: Vec<String>,
}

impl GuiTemplateDiagnostics {
    fn from_descriptor(
        descriptor: Option<&VanillaProfileDescriptor>,
        registry: Option<&GuiTemplateRegistry<'_>>,
    ) -> Self {
        let mut out = Self::default();
        if let Some(descriptor) = descriptor {
            for template in descriptor.template_instances {
                if let Some(found) =
                    registry.and_then(|registry| registry.get(template.template_name))
                {
                    out.templates.push(GuiTemplateDiagnostic {
                        template_name: template.template_name.to_owned(),
                        source_gui_file: found.source_gui_file.map(str::to_owned),
                        count: template.count,
                    });
                } else {
                    out.missing_templates
                        .push(template.template_name.to_owned());
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiTemplateDiagnostic {
    pub template_name: String,
    pub source_gui_file: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiInstanceDiagnostic {
    pub template_name: String,
    pub path: GuiNodePath,
    pub semantic_role: Option<String>,
    pub model_key: Option<String>,
    pub slot_rect: GuiRect,
    pub resolved_rect: GuiRect,
}

impl GuiInstanceDiagnostic {
    fn from_instance(instance: &GuiGeneratedInstance) -> Self {
        Self {
            template_name: instance.template_name.to_owned(),
            path: instance.path.clone(),
            semantic_role: instance.context.semantic_role.clone(),
            model_key: instance.context.model_key.clone(),
            slot_rect: instance.slot_rect,
            resolved_rect: instance.resolved_rect,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiNodeRuntimeReport {
    pub path: GuiNodePath,
    pub name: Option<String>,
    pub rect: GuiRect,
    pub clip_rect: GuiRect,
    pub hit_rect: GuiRect,
    pub resource_name: Option<String>,
    pub effect_file: Option<String>,
    pub draw_kind: Option<String>,
    pub instance_path: Option<GuiNodePath>,
    pub instance_parent_path: Option<GuiNodePath>,
    pub semantic_role: Option<String>,
    pub model_key: Option<String>,
}

fn collect_node_reports(
    root_layout: &LayoutNode,
    draw_list: &[GuiDrawCommand],
    instances: &[GuiGeneratedInstance],
    scroll_states: &[GuiScrollState],
) -> Vec<GuiNodeRuntimeReport> {
    let mut out = Vec::new();
    collect_layout_reports(root_layout, draw_list, None, None, &mut out);
    for instance in instances {
        collect_layout_reports(
            &instance.layout,
            draw_list,
            Some(instance),
            scroll_states
                .iter()
                .find(|state| state.path == instance.parent_path)
                .map(|_| instance),
            &mut out,
        );
    }
    out
}

fn collect_layout_reports(
    layout: &LayoutNode,
    draw_list: &[GuiDrawCommand],
    instance: Option<&GuiGeneratedInstance>,
    scroll_instance: Option<&GuiGeneratedInstance>,
    out: &mut Vec<GuiNodeRuntimeReport>,
) {
    let draw = draw_list
        .iter()
        .find(|command| command.source_path == layout.path);
    out.push(GuiNodeRuntimeReport {
        path: layout.path.clone(),
        name: layout.name.clone(),
        rect: layout.rect,
        clip_rect: layout.clip_rect,
        hit_rect: layout.rects.hit_rect,
        resource_name: draw.and_then(|command| command.resource_name().map(str::to_owned)),
        effect_file: draw.and_then(effect_file_from_command),
        draw_kind: draw.map(|command| command.kind_label().to_owned()),
        instance_path: instance.map(|instance| instance.path.clone()),
        instance_parent_path: scroll_instance.map(|instance| instance.parent_path.clone()),
        semantic_role: instance.and_then(|instance| instance.context.semantic_role.clone()),
        model_key: instance.and_then(|instance| instance.context.model_key.clone()),
    });
    for child in &layout.children {
        collect_layout_reports(child, draw_list, instance, scroll_instance, out);
    }
}

fn effect_file_from_command(command: &GuiDrawCommand) -> Option<String> {
    match &command.kind {
        super::draw_command::GuiDrawCommandKind::DrawSprite(sprite)
        | super::draw_command::GuiDrawCommandKind::DrawNineSlice(sprite) => {
            sprite.effect_file.clone()
        }
        super::draw_command::GuiDrawCommandKind::DrawProgress(progress) => {
            progress.effect_file.clone()
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VanillaUnsupportedSemanticsCategory {
    Parser,
    Layout,
    Resource,
    Render,
    Binding,
    Runtime,
}

impl VanillaUnsupportedSemanticsCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parser => "parser",
            Self::Layout => "layout",
            Self::Resource => "resource",
            Self::Render => "render",
            Self::Binding => "binding",
            Self::Runtime => "runtime",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaUnsupportedSemanticsEntry {
    pub category: VanillaUnsupportedSemanticsCategory,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct VanillaUnsupportedSemanticsRegistry {
    entries: Vec<VanillaUnsupportedSemanticsEntry>,
}

impl VanillaUnsupportedSemanticsRegistry {
    pub fn from_runtime_diagnostics(diagnostics: &GuiRuntimeDiagnostics) -> Self {
        let mut registry = Self::default();
        for node in &diagnostics.nodes {
            if node
                .name
                .as_deref()
                .is_some_and(|name| name == "future_widget")
            {
                registry.push(
                    VanillaUnsupportedSemanticsCategory::Parser,
                    "futureWidgetType",
                    "unsupported GUI node kind",
                );
            }
        }
        for effect in &diagnostics.draw_commands.unsupported_effects {
            registry.push(
                VanillaUnsupportedSemanticsCategory::Render,
                effect.effect_file.clone(),
                format!("unsupported {} effect", effect.category),
            );
        }
        for missing in &diagnostics.templates.missing_templates {
            registry.push(
                VanillaUnsupportedSemanticsCategory::Binding,
                missing.clone(),
                "template descriptor not found in registry",
            );
        }
        for state in &diagnostics.scroll_states {
            registry.push(
                VanillaUnsupportedSemanticsCategory::Layout,
                state.path.to_string(),
                "scroll approximation uses egui-compatible clipping",
            );
        }
        registry
    }

    pub fn from_profile_diagnostics(
        profile: &super::runtime::VanillaGuiProfileDiagnostics,
    ) -> Self {
        let mut registry = Self::default();
        for sprite in &profile.missing_sprites {
            registry.push(
                VanillaUnsupportedSemanticsCategory::Resource,
                sprite.clone(),
                "required sprite missing",
            );
        }
        for line in &profile.lines {
            let category = match line.category {
                super::runtime::VanillaGuiDiagnosticCategory::Layout => {
                    VanillaUnsupportedSemanticsCategory::Layout
                }
                super::runtime::VanillaGuiDiagnosticCategory::Binding => {
                    VanillaUnsupportedSemanticsCategory::Binding
                }
                super::runtime::VanillaGuiDiagnosticCategory::Gfx
                | super::runtime::VanillaGuiDiagnosticCategory::TextureDecode => {
                    VanillaUnsupportedSemanticsCategory::Resource
                }
                super::runtime::VanillaGuiDiagnosticCategory::Gui
                | super::runtime::VanillaGuiDiagnosticCategory::Hoi4Path => {
                    VanillaUnsupportedSemanticsCategory::Runtime
                }
                super::runtime::VanillaGuiDiagnosticCategory::Parser => {
                    VanillaUnsupportedSemanticsCategory::Parser
                }
            };
            registry.push(category, line.message.clone(), line.message.clone());
        }
        registry
    }

    pub fn merge(&mut self, other: Self) {
        self.entries.extend(other.entries);
        self.entries.sort_by(|a, b| {
            a.category
                .as_str()
                .cmp(b.category.as_str())
                .then(a.subject.cmp(&b.subject))
        });
        self.entries
            .dedup_by(|a, b| a.category == b.category && a.subject == b.subject);
    }

    pub fn entries(&self) -> &[VanillaUnsupportedSemanticsEntry] {
        &self.entries
    }

    pub fn category_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self
            .entries
            .iter()
            .map(|entry| entry.category.as_str())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Unsupported Vanilla GUI Semantics");
        for entry in &self.entries {
            let _ = writeln!(
                out,
                "- [{}] {}: {}",
                entry.category.as_str(),
                entry.subject,
                entry.detail
            );
        }
        out
    }

    fn push(
        &mut self,
        category: VanillaUnsupportedSemanticsCategory,
        subject: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.entries.push(VanillaUnsupportedSemanticsEntry {
            category,
            subject: subject.into(),
            detail: detail.into(),
        });
    }
}
