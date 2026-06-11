use std::fmt::Write as _;

use clausewitz_parser::{Block, Value};
use egui::Color32;

use super::ast::{GuiNode, GuiNodeKind, GuiNodePath, GuiValueExt};
use super::binding::{GuiBinding, GuiBindingMap};
use super::gfx_index::{GfxResource, GfxResourceKind, GfxSize};
use super::intrinsic::{
    node_resource_name, resolve_text_layout, FontToken, GuiTextHorizontalAlign, GuiTextLayout,
    GuiTextVerticalAlign,
};
use super::layout::{GuiRect, LayoutNode};
use super::GfxIndex;
use crate::vanilla_iron::VanillaIron;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVisualState {
    Normal,
    Hover,
    Pressed,
    Disabled,
}

impl ButtonVisualState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Hover => "hover",
            Self::Pressed => "pressed",
            Self::Disabled => "disabled",
        }
    }
}

pub fn button_frame_for_state(frame_count: Option<u32>, state: ButtonVisualState) -> u32 {
    let frames = frame_count.unwrap_or(1).max(1);
    match state {
        ButtonVisualState::Normal => 1,
        ButtonVisualState::Hover => 2.min(frames),
        ButtonVisualState::Pressed => 3.min(frames),
        ButtonVisualState::Disabled => 4.min(frames),
    }
}

pub fn checkbox_frame_for_state(
    frame_count: Option<u32>,
    state: ButtonVisualState,
    checked_frame: Option<u32>,
) -> Option<u32> {
    let frames = frame_count.unwrap_or(1).max(1);
    let base = checked_frame.unwrap_or(1).clamp(1, frames);
    Some(match state {
        ButtonVisualState::Normal => base,
        ButtonVisualState::Hover => (base + 1).min(frames),
        ButtonVisualState::Pressed => (base + 2).min(frames),
        ButtonVisualState::Disabled => base.min(frames),
    })
}

pub fn frame_animated_frame(frame_count: Option<u32>, fps: Option<f32>, time_secs: f64) -> u32 {
    let frames = frame_count.unwrap_or(1).max(1);
    if frames <= 1 {
        return 1;
    }
    let fps = fps.unwrap_or(24.0).max(0.001);
    ((time_secs.max(0.0) * fps as f64).floor() as u32 % frames) + 1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiButtonEffectPolicy {
    DefaultButtonState,
    OnlyDisable,
    NoDownEffect,
    BlendFrames,
    Unsupported(String),
}

impl GuiButtonEffectPolicy {
    pub fn from_effect_file(effect_file: Option<&str>) -> Self {
        let Some(effect_file) = effect_file.filter(|value| !value.trim().is_empty()) else {
            return Self::DefaultButtonState;
        };
        let normalized = normalize_effect_file(effect_file);
        if normalized.contains("buttonstate_onlydisable") {
            Self::OnlyDisable
        } else if normalized.contains("buttonstate_nodowneffect") {
            Self::NoDownEffect
        } else if normalized.contains("buttonstate_blendframes") {
            Self::BlendFrames
        } else if normalized.ends_with("/buttonstate.lua")
            || normalized.ends_with("/buttonstate.shader")
            || normalized == "buttonstate"
            || normalized == "buttonstate.lua"
            || normalized == "buttonstate.shader"
        {
            Self::DefaultButtonState
        } else if normalized.contains("buttonstate") {
            Self::Unsupported(effect_file.to_owned())
        } else {
            Self::DefaultButtonState
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::DefaultButtonState => "buttonstate",
            Self::OnlyDisable => "buttonstate_onlydisable",
            Self::NoDownEffect => "buttonstate_nodowneffect",
            Self::BlendFrames => "buttonstate_blendframes",
            Self::Unsupported(_) => "unsupported_buttonstate",
        }
    }

    pub fn unsupported_effect_file(&self) -> Option<&str> {
        match self {
            Self::Unsupported(effect_file) => Some(effect_file.as_str()),
            _ => None,
        }
    }
}

pub fn button_frame_for_effect(
    effect: &GuiButtonEffectPolicy,
    frame_count: Option<u32>,
    state: ButtonVisualState,
) -> u32 {
    let frames = frame_count.unwrap_or(1).max(1);
    match effect {
        GuiButtonEffectPolicy::OnlyDisable => match state {
            ButtonVisualState::Disabled => 2.min(frames),
            _ => 1,
        },
        GuiButtonEffectPolicy::NoDownEffect => match state {
            ButtonVisualState::Pressed => 2.min(frames),
            _ => button_frame_for_state(frame_count, state),
        },
        GuiButtonEffectPolicy::BlendFrames
        | GuiButtonEffectPolicy::DefaultButtonState
        | GuiButtonEffectPolicy::Unsupported(_) => button_frame_for_state(frame_count, state),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiProgressEffectPolicy {
    DefaultProgress,
    StartEnd,
    Unsupported(String),
}

impl GuiProgressEffectPolicy {
    pub fn from_effect_file(effect_file: Option<&str>) -> Self {
        let Some(effect_file) = effect_file.filter(|value| !value.trim().is_empty()) else {
            return Self::DefaultProgress;
        };
        let normalized = normalize_effect_file(effect_file);
        if normalized.contains("progress_startend") {
            Self::StartEnd
        } else if normalized.ends_with("/progress.lua")
            || normalized.ends_with("/progress.shader")
            || normalized == "progress"
            || normalized == "progress.lua"
            || normalized == "progress.shader"
        {
            Self::DefaultProgress
        } else if normalized.contains("progress") {
            Self::Unsupported(effect_file.to_owned())
        } else {
            Self::DefaultProgress
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::DefaultProgress => "progress",
            Self::StartEnd => "progress_startend",
            Self::Unsupported(_) => "unsupported_progress",
        }
    }

    pub fn unsupported_effect_file(&self) -> Option<&str> {
        match self {
            Self::Unsupported(effect_file) => Some(effect_file.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiAlphaSemantics {
    Opaque,
    NodeAlpha,
    AlwaysTransparentApproximation,
}

impl GuiAlphaSemantics {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::NodeAlpha => "node_alpha",
            Self::AlwaysTransparentApproximation => "always_transparent_approximation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiBlendMode {
    EguiTintStraightAlphaApproximation,
}

impl GuiBlendMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EguiTintStraightAlphaApproximation => "egui_tint_straight_alpha_approximation",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiDrawCommand {
    pub source_path: GuiNodePath,
    pub source_name: Option<String>,
    pub rect: GuiRect,
    pub clip_rect: GuiRect,
    pub z_index: usize,
    pub state: GuiDrawState,
    pub kind: GuiDrawCommandKind,
}

impl GuiDrawCommand {
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            GuiDrawCommandKind::DrawSprite(_) => "DrawSprite",
            GuiDrawCommandKind::DrawText(_) => "DrawText",
            GuiDrawCommandKind::DrawProgress(_) => "DrawProgress",
            GuiDrawCommandKind::DrawPieChart(_) => "DrawPieChart",
            GuiDrawCommandKind::DrawNineSlice(_) => "DrawNineSlice",
            GuiDrawCommandKind::DrawFallback(_) => "DrawFallback",
        }
    }

    pub fn resource_name(&self) -> Option<&str> {
        match &self.kind {
            GuiDrawCommandKind::DrawSprite(command)
            | GuiDrawCommandKind::DrawNineSlice(command) => Some(command.resource_name.as_str()),
            GuiDrawCommandKind::DrawProgress(command) => Some(command.resource_name.as_str()),
            GuiDrawCommandKind::DrawPieChart(command) => Some(command.resource_name.as_str()),
            GuiDrawCommandKind::DrawText(_) | GuiDrawCommandKind::DrawFallback(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiDrawState {
    pub visible: bool,
    pub enabled: bool,
    pub visual_state: Option<ButtonVisualState>,
    pub clip_applied: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GuiDrawCommandKind {
    DrawSprite(GuiSpriteDrawCommand),
    DrawText(GuiTextDrawCommand),
    DrawProgress(GuiProgressDrawCommand),
    DrawPieChart(GuiPieChartDrawCommand),
    DrawNineSlice(GuiSpriteDrawCommand),
    DrawFallback(GuiFallbackDrawCommand),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiSpriteDrawCommand {
    pub resource_name: String,
    pub resource_kind: Option<GfxResourceKind>,
    pub frame: Option<u32>,
    pub frame_count: Option<u32>,
    pub texture_progress: Option<f32>,
    pub tint: Color32,
    pub alpha: f32,
    pub alpha_semantics: GuiAlphaSemantics,
    pub blend_mode: GuiBlendMode,
    pub centerposition: bool,
    pub scale: f32,
    pub effect_file: Option<String>,
    pub button_effect: Option<GuiButtonEffectPolicy>,
    pub checkbox_base_frame: Option<u32>,
}

impl GuiSpriteDrawCommand {
    pub fn effective_tint(&self) -> Color32 {
        apply_alpha_to_tint(self.tint, self.alpha)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiTextDrawCommand {
    pub raw_text: String,
    pub text_layout: GuiTextLayout,
    pub color: Color32,
    pub font_token: FontToken,
    pub localization_fallback: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiProgressDrawCommand {
    pub resource_name: String,
    pub value: f32,
    pub effect_file: Option<String>,
    pub effect: GuiProgressEffectPolicy,
    pub fill_rect: GuiRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiPieChartDrawCommand {
    pub resource_name: String,
    pub segments: Vec<(f32, Color32)>,
    pub used_fallback_segments: bool,
    pub overlay_texture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiFallbackDrawCommand {
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GuiDrawCommandDiagnostics {
    pub total: usize,
    pub sprites: usize,
    pub text: usize,
    pub progress: usize,
    pub pie_charts: usize,
    pub nine_slices: usize,
    pub fallback: usize,
    pub unsupported_effects: Vec<GuiUnsupportedEffectDiagnostic>,
    pub commands: Vec<GuiDrawCommandSummary>,
}

impl GuiDrawCommandDiagnostics {
    pub fn from_commands(commands: &[GuiDrawCommand]) -> Self {
        let mut diagnostics = Self {
            total: commands.len(),
            ..Self::default()
        };
        for command in commands {
            match &command.kind {
                GuiDrawCommandKind::DrawSprite(sprite) => {
                    diagnostics.sprites += 1;
                    if let Some(effect) = &sprite.button_effect {
                        if let Some(effect_file) = effect.unsupported_effect_file() {
                            diagnostics
                                .unsupported_effects
                                .push(GuiUnsupportedEffectDiagnostic {
                                    source_path: command.source_path.clone(),
                                    resource_name: Some(sprite.resource_name.clone()),
                                    effect_file: effect_file.to_owned(),
                                    category: "button".to_owned(),
                                });
                        }
                    }
                }
                GuiDrawCommandKind::DrawText(_) => diagnostics.text += 1,
                GuiDrawCommandKind::DrawProgress(progress) => {
                    diagnostics.progress += 1;
                    if let Some(effect_file) = progress.effect.unsupported_effect_file() {
                        diagnostics
                            .unsupported_effects
                            .push(GuiUnsupportedEffectDiagnostic {
                                source_path: command.source_path.clone(),
                                resource_name: Some(progress.resource_name.clone()),
                                effect_file: effect_file.to_owned(),
                                category: "progress".to_owned(),
                            });
                    }
                }
                GuiDrawCommandKind::DrawPieChart(_) => diagnostics.pie_charts += 1,
                GuiDrawCommandKind::DrawNineSlice(sprite) => {
                    diagnostics.nine_slices += 1;
                    if let Some(effect) = &sprite.button_effect {
                        if let Some(effect_file) = effect.unsupported_effect_file() {
                            diagnostics
                                .unsupported_effects
                                .push(GuiUnsupportedEffectDiagnostic {
                                    source_path: command.source_path.clone(),
                                    resource_name: Some(sprite.resource_name.clone()),
                                    effect_file: effect_file.to_owned(),
                                    category: "button".to_owned(),
                                });
                        }
                    }
                }
                GuiDrawCommandKind::DrawFallback(_) => diagnostics.fallback += 1,
            }
            diagnostics.commands.push(GuiDrawCommandSummary {
                source_path: command.source_path.clone(),
                kind: command.kind_label().to_owned(),
                resource_name: command.resource_name().map(ToOwned::to_owned),
                rect: command.rect,
                clip_rect: command.clip_rect,
                z_index: command.z_index,
            });
        }
        diagnostics.unsupported_effects.sort_by(|a, b| {
            a.source_path
                .cmp(&b.source_path)
                .then(a.effect_file.cmp(&b.effect_file))
        });
        diagnostics
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Runtime Draw Command Diagnostics");
        let _ = writeln!(
            out,
            "- total: {} sprites={} text={} progress={} pie={} nine_slice={} fallback={}",
            self.total,
            self.sprites,
            self.text,
            self.progress,
            self.pie_charts,
            self.nine_slices,
            self.fallback
        );
        let _ = writeln!(
            out,
            "- unsupported_effects: {}",
            self.unsupported_effects.len()
        );
        for command in &self.commands {
            let _ = writeln!(
                out,
                "- z={} kind={} path={} resource={} rect=({:.1}, {:.1}, {:.1}, {:.1}) clip=({:.1}, {:.1}, {:.1}, {:.1})",
                command.z_index,
                command.kind,
                command.source_path,
                command.resource_name.as_deref().unwrap_or("<none>"),
                command.rect.x,
                command.rect.y,
                command.rect.width,
                command.rect.height,
                command.clip_rect.x,
                command.clip_rect.y,
                command.clip_rect.width,
                command.clip_rect.height
            );
        }
        out
    }

    pub fn unsupported_effects_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Unsupported Renderer Effects");
        let _ = writeln!(
            out,
            "- unsupported_effects: {}",
            self.unsupported_effects.len()
        );
        for effect in &self.unsupported_effects {
            let _ = writeln!(
                out,
                "- category={} path={} resource={} effectFile={}",
                effect.category,
                effect.source_path,
                effect.resource_name.as_deref().unwrap_or("<none>"),
                effect.effect_file
            );
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiDrawCommandSummary {
    pub source_path: GuiNodePath,
    pub kind: String,
    pub resource_name: Option<String>,
    pub rect: GuiRect,
    pub clip_rect: GuiRect,
    pub z_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiUnsupportedEffectDiagnostic {
    pub source_path: GuiNodePath,
    pub resource_name: Option<String>,
    pub effect_file: String,
    pub category: String,
}

pub fn collect_draw_commands_for_tree(
    root: &GuiNode,
    layout: &LayoutNode,
    bindings: &GuiBindingMap,
    gfx_index: Option<&GfxIndex>,
    time_secs: f64,
) -> Vec<GuiDrawCommand> {
    let mut out = Vec::new();
    collect_draw_commands_from_node(root, layout, bindings, gfx_index, time_secs, &mut out);
    renumber_draw_commands(&mut out);
    out
}

pub fn renumber_draw_commands(commands: &mut [GuiDrawCommand]) {
    for (index, command) in commands.iter_mut().enumerate() {
        command.z_index = index;
    }
}

fn collect_draw_commands_from_node(
    node: &GuiNode,
    layout: &LayoutNode,
    bindings: &GuiBindingMap,
    gfx_index: Option<&GfxIndex>,
    time_secs: f64,
    out: &mut Vec<GuiDrawCommand>,
) {
    let binding = bindings.for_node(&layout.path, layout.name.as_deref());
    let visible = binding.visible.unwrap_or(layout.visible);
    if !visible {
        return;
    }

    match node.kind {
        GuiNodeKind::Background | GuiNodeKind::Icon => {
            push_resource_command(node, layout, &binding, gfx_index, time_secs, None, out);
        }
        GuiNodeKind::Button | GuiNodeKind::CheckBox => {
            let enabled = binding
                .enabled
                .unwrap_or_else(|| !node.bool("disabled").unwrap_or(false));
            let visual_state = if enabled {
                ButtonVisualState::Normal
            } else {
                ButtonVisualState::Disabled
            };
            push_resource_command(
                node,
                layout,
                &binding,
                gfx_index,
                time_secs,
                Some(visual_state),
                out,
            );
            push_button_text_command(node, layout, &binding, enabled, out);
        }
        GuiNodeKind::InstantTextbox | GuiNodeKind::EditBox => {
            if matches!(node.kind, GuiNodeKind::EditBox) {
                if resource_name_for_node(node, &binding).is_some() {
                    push_resource_command(node, layout, &binding, gfx_index, time_secs, None, out);
                }
            }
            push_text_command(node, layout, &binding, out);
        }
        GuiNodeKind::ContainerWindow
        | GuiNodeKind::GridBox
        | GuiNodeKind::OverlappingElementsBox
        | GuiNodeKind::VerticalScrollbar => {
            push_resource_command(node, layout, &binding, gfx_index, time_secs, None, out);
        }
        GuiNodeKind::Position => {}
        GuiNodeKind::Unknown(_) => {
            push_fallback_command(
                layout,
                node.name.as_deref().unwrap_or(node.kind.as_key()),
                "unsupported GUI node kind",
                true,
                out,
            );
        }
    }

    for (child_node, child_layout) in node.children.iter().zip(layout.children.iter()) {
        collect_draw_commands_from_node(
            child_node,
            child_layout,
            bindings,
            gfx_index,
            time_secs,
            out,
        );
    }
}

fn push_resource_command(
    node: &GuiNode,
    layout: &LayoutNode,
    binding: &GuiBinding,
    gfx_index: Option<&GfxIndex>,
    time_secs: f64,
    visual_state: Option<ButtonVisualState>,
    out: &mut Vec<GuiDrawCommand>,
) {
    let Some(resource_name) = resource_name_for_node(node, binding) else {
        if matches!(node.kind, GuiNodeKind::Button | GuiNodeKind::CheckBox) {
            push_fallback_command(
                layout,
                node.name.as_deref().unwrap_or("button"),
                "button has no sprite resource",
                binding.enabled.unwrap_or(true),
                out,
            );
        }
        return;
    };
    let resource = gfx_index.and_then(|index| index.get(&resource_name));
    let resource_kind = resource.map(|resource| resource.kind.clone());
    let effect_file = resource.and_then(effect_file_for_resource);
    let frame_count = resource.and_then(|resource| resource.frame_count);
    let node_frame = node.f32("frame").map(|frame| frame as u32);
    let centerposition = node.bool("centerposition").unwrap_or(false);
    let enabled = binding
        .enabled
        .unwrap_or_else(|| !node.bool("disabled").unwrap_or(false));
    let state = GuiDrawState {
        visible: true,
        enabled,
        visual_state,
        clip_applied: layout.clip_rect != layout.rect,
    };

    match resource_kind.as_ref().unwrap_or(&GfxResourceKind::Sprite) {
        GfxResourceKind::CorneredTile => {
            let sprite = sprite_draw_command(
                node,
                binding,
                &resource_name,
                resource_kind,
                frame_count,
                node_frame,
                effect_file,
                centerposition,
                layout.scale,
                visual_state,
                time_secs,
            );
            out.push(GuiDrawCommand {
                source_path: layout.path.clone(),
                source_name: layout.name.clone(),
                rect: resource_rect(layout),
                clip_rect: layout.clip_rect,
                z_index: out.len(),
                state,
                kind: GuiDrawCommandKind::DrawNineSlice(sprite),
            });
        }
        GfxResourceKind::ProgressBar => {
            let value = binding.progress.unwrap_or(0.0).clamp(0.0, 1.0);
            out.push(GuiDrawCommand {
                source_path: layout.path.clone(),
                source_name: layout.name.clone(),
                rect: resource_rect(layout),
                clip_rect: layout.clip_rect,
                z_index: out.len(),
                state,
                kind: GuiDrawCommandKind::DrawProgress(GuiProgressDrawCommand {
                    resource_name,
                    value,
                    effect: GuiProgressEffectPolicy::from_effect_file(effect_file),
                    effect_file: effect_file.map(ToOwned::to_owned),
                    fill_rect: progress_fill_rect(resource_rect(layout), value),
                }),
            });
        }
        GfxResourceKind::PieChart => {
            let segments = if binding.pie_segments.is_empty() {
                default_pie_segments().to_vec()
            } else {
                binding.pie_segments.clone()
            };
            out.push(GuiDrawCommand {
                source_path: layout.path.clone(),
                source_name: layout.name.clone(),
                rect: resource_rect(layout),
                clip_rect: layout.clip_rect,
                z_index: out.len(),
                state,
                kind: GuiDrawCommandKind::DrawPieChart(GuiPieChartDrawCommand {
                    resource_name,
                    segments,
                    used_fallback_segments: binding.pie_segments.is_empty(),
                    overlay_texture: resource
                        .and_then(|resource| resource.fallback_texture_name())
                        .map(ToOwned::to_owned),
                }),
            });
        }
        _ => {
            let sprite = sprite_draw_command(
                node,
                binding,
                &resource_name,
                resource_kind,
                frame_count,
                node_frame,
                effect_file,
                centerposition,
                layout.scale,
                visual_state,
                time_secs,
            );
            out.push(GuiDrawCommand {
                source_path: layout.path.clone(),
                source_name: layout.name.clone(),
                rect: resource_rect(layout),
                clip_rect: layout.clip_rect,
                z_index: out.len(),
                state,
                kind: GuiDrawCommandKind::DrawSprite(sprite),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sprite_draw_command(
    node: &GuiNode,
    binding: &GuiBinding,
    resource_name: &str,
    resource_kind: Option<GfxResourceKind>,
    frame_count: Option<u32>,
    node_frame: Option<u32>,
    effect_file: Option<&str>,
    centerposition: bool,
    scale: f32,
    visual_state: Option<ButtonVisualState>,
    time_secs: f64,
) -> GuiSpriteDrawCommand {
    let button_effect = visual_state.map(|_| GuiButtonEffectPolicy::from_effect_file(effect_file));
    let checkbox_base = matches!(node.kind, GuiNodeKind::CheckBox)
        .then(|| checkbox_base_frame(node, binding))
        .flatten();
    let frame = match (node.kind.clone(), visual_state, button_effect.as_ref()) {
        (GuiNodeKind::CheckBox, Some(state), _) => {
            checkbox_frame_for_state(frame_count, state, checkbox_base)
        }
        (_, Some(state), Some(effect)) => {
            Some(node_frame.unwrap_or_else(|| button_frame_for_effect(effect, frame_count, state)))
        }
        (_, _, _) if matches!(resource_kind, Some(GfxResourceKind::FrameAnimated)) => Some(
            binding
                .frame
                .or(node_frame)
                .unwrap_or_else(|| frame_animated_frame(frame_count, None, time_secs)),
        ),
        _ => binding.frame.or(node_frame),
    };
    let alpha = node_alpha(node);
    let alpha_semantics = alpha_semantics(node);
    GuiSpriteDrawCommand {
        resource_name: resource_name.to_owned(),
        resource_kind,
        frame,
        frame_count,
        texture_progress: binding.progress,
        tint: binding.tint.unwrap_or(Color32::WHITE),
        alpha,
        alpha_semantics,
        blend_mode: GuiBlendMode::EguiTintStraightAlphaApproximation,
        centerposition,
        scale,
        effect_file: effect_file.map(ToOwned::to_owned),
        button_effect,
        checkbox_base_frame: checkbox_base,
    }
}

fn push_text_command(
    node: &GuiNode,
    layout: &LayoutNode,
    binding: &GuiBinding,
    out: &mut Vec<GuiDrawCommand>,
) {
    let node_text = node.string("text");
    let node_name = node.string("name");
    let Some(text) = binding
        .input_text
        .as_deref()
        .or(binding.text.as_deref())
        .or(node_text.as_deref())
        .or(node_name.as_deref())
    else {
        return;
    };
    let text_layout = layout
        .rects
        .text_layout
        .clone()
        .unwrap_or_else(|| resolve_text_layout(node, layout.rects.paint_rect, layout.scale));
    let node_font = node.string("font");
    let color = binding
        .text_color
        .unwrap_or_else(|| text_color_from_vanilla_font(node_font.as_deref()));
    let text_clip = text_layout.box_rect.intersect(layout.clip_rect);
    out.push(GuiDrawCommand {
        source_path: layout.path.clone(),
        source_name: layout.name.clone(),
        rect: text_layout.box_rect,
        clip_rect: text_clip,
        z_index: out.len(),
        state: GuiDrawState {
            visible: true,
            enabled: binding.enabled.unwrap_or(true),
            visual_state: None,
            clip_applied: text_clip != text_layout.box_rect,
        },
        kind: GuiDrawCommandKind::DrawText(GuiTextDrawCommand {
            raw_text: text.to_owned(),
            color,
            font_token: text_layout.font_token,
            localization_fallback: true,
            text_layout,
        }),
    });
}

fn push_button_text_command(
    node: &GuiNode,
    layout: &LayoutNode,
    binding: &GuiBinding,
    enabled: bool,
    out: &mut Vec<GuiDrawCommand>,
) {
    let node_text = node.string("buttonText").or_else(|| node.string("text"));
    let Some(text) = binding.text.as_deref().or(node_text.as_deref()) else {
        return;
    };
    let node_font = node.string("buttonFont").or_else(|| node.string("font"));
    let font_token = FontToken::from_vanilla(node_font.as_deref());
    let text_layout = GuiTextLayout {
        box_rect: layout.rects.visual_rect,
        horizontal: GuiTextHorizontalAlign::Center,
        vertical: GuiTextVerticalAlign::Center,
        fixed_size: false,
        font_token,
        metrics: font_token.metrics().scaled(layout.scale),
    };
    out.push(GuiDrawCommand {
        source_path: layout.path.clone(),
        source_name: layout.name.clone(),
        rect: text_layout.box_rect,
        clip_rect: text_layout.box_rect.intersect(layout.clip_rect),
        z_index: out.len(),
        state: GuiDrawState {
            visible: true,
            enabled,
            visual_state: Some(if enabled {
                ButtonVisualState::Normal
            } else {
                ButtonVisualState::Disabled
            }),
            clip_applied: layout.clip_rect != layout.rect,
        },
        kind: GuiDrawCommandKind::DrawText(GuiTextDrawCommand {
            raw_text: text.to_owned(),
            color: if enabled {
                VanillaIron::TEXT
            } else {
                VanillaIron::MUTED
            },
            font_token,
            localization_fallback: true,
            text_layout,
        }),
    });
}

fn push_fallback_command(
    layout: &LayoutNode,
    label: &str,
    reason: &str,
    enabled: bool,
    out: &mut Vec<GuiDrawCommand>,
) {
    out.push(GuiDrawCommand {
        source_path: layout.path.clone(),
        source_name: layout.name.clone(),
        rect: layout.rects.paint_rect,
        clip_rect: layout.clip_rect,
        z_index: out.len(),
        state: GuiDrawState {
            visible: true,
            enabled,
            visual_state: None,
            clip_applied: layout.clip_rect != layout.rect,
        },
        kind: GuiDrawCommandKind::DrawFallback(GuiFallbackDrawCommand {
            label: label.to_owned(),
            reason: reason.to_owned(),
        }),
    });
}

fn resource_name_for_node(node: &GuiNode, binding: &GuiBinding) -> Option<String> {
    binding.sprite.clone().or_else(|| node_resource_name(node))
}

fn resource_rect(layout: &LayoutNode) -> GuiRect {
    layout
        .rects
        .resource_rect
        .unwrap_or(layout.rects.paint_rect)
}

fn progress_fill_rect(rect: GuiRect, value: f32) -> GuiRect {
    let value = value.clamp(0.0, 1.0);
    GuiRect::new(rect.x, rect.y, rect.width * value, rect.height)
}

fn checkbox_base_frame(node: &GuiNode, binding: &GuiBinding) -> Option<u32> {
    let node_frame = node.f32("frame").map(|frame| frame as u32);
    match binding.checked {
        Some(true) => Some(node_frame.unwrap_or(2)),
        Some(false) => Some(1),
        None => node_frame,
    }
}

fn effect_file_for_resource(resource: &GfxResource) -> Option<&str> {
    raw_property(resource, "effectFile")
}

pub fn raw_property<'a>(resource: &'a GfxResource, key: &str) -> Option<&'a str> {
    resource
        .raw_properties
        .iter()
        .find(|property| property.key.eq_ignore_ascii_case(key))
        .map(|property| property.value.as_str())
}

fn normalize_effect_file(effect_file: &str) -> String {
    let mut value = effect_file.replace('\\', "/").to_ascii_lowercase();
    if let Some(stripped) = value.strip_suffix(".fx") {
        value = stripped.to_owned();
    }
    value
}

fn node_alpha(node: &GuiNode) -> f32 {
    let explicit = node
        .f32("alpha")
        .or_else(|| node.f32("transparency"))
        .map(normalize_alpha_value);
    let mut alpha = explicit.unwrap_or(1.0);
    if node_bool_any(
        node,
        &[
            "alwaystransparent",
            "alwaysTransparent",
            "always_transparent",
        ],
    )
    .unwrap_or(false)
    {
        alpha = alpha.min(0.5);
    }
    alpha.clamp(0.0, 1.0)
}

fn alpha_semantics(node: &GuiNode) -> GuiAlphaSemantics {
    if node_bool_any(
        node,
        &[
            "alwaystransparent",
            "alwaysTransparent",
            "always_transparent",
        ],
    )
    .unwrap_or(false)
    {
        GuiAlphaSemantics::AlwaysTransparentApproximation
    } else if node.f32("alpha").is_some() || node.f32("transparency").is_some() {
        GuiAlphaSemantics::NodeAlpha
    } else {
        GuiAlphaSemantics::Opaque
    }
}

fn normalize_alpha_value(value: f32) -> f32 {
    if value > 1.0 {
        (value / 255.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn node_bool_any(node: &GuiNode, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| node.bool(key))
}

pub fn apply_alpha_to_tint(tint: Color32, alpha: f32) -> Color32 {
    let alpha = (tint.a() as f32 * alpha.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), alpha)
}

fn text_color_from_vanilla_font(font: Option<&str>) -> Color32 {
    match font.unwrap_or_default() {
        font if font.starts_with("hoi4_typewriter") => Color32::from_rgb(0x2f, 0x29, 0x1b),
        _ => VanillaIron::TEXT,
    }
}

fn default_pie_segments() -> &'static [(f32, Color32)] {
    &[
        (0.35, crate::v9::palette::IDEO_FASCISM),
        (0.25, crate::v9::palette::IDEO_DEMOCRATIC),
        (0.20, crate::v9::palette::IDEO_COMMUNISM),
        (0.20, crate::v9::palette::IDEO_NEUTRALITY),
    ]
}

#[allow(dead_code)]
fn color_from_block(block: &Block) -> Option<Color32> {
    let r = block_value_f32(block, "r")
        .or_else(|| block.values.first().and_then(Value::as_lossy_f32))?;
    let g = block_value_f32(block, "g")
        .or_else(|| block.values.get(1).and_then(Value::as_lossy_f32))?;
    let b = block_value_f32(block, "b")
        .or_else(|| block.values.get(2).and_then(Value::as_lossy_f32))?;
    let a = block_value_f32(block, "a")
        .or_else(|| block.values.get(3).and_then(Value::as_lossy_f32))
        .unwrap_or(255.0);
    Some(Color32::from_rgba_premultiplied(
        normalize_color_channel(r),
        normalize_color_channel(g),
        normalize_color_channel(b),
        normalize_color_channel(a),
    ))
}

fn block_value_f32(block: &Block, key: &str) -> Option<f32> {
    block
        .entries
        .iter()
        .find(|entry| entry.key.eq_ignore_ascii_case(key))
        .and_then(|entry| entry.value.as_lossy_f32())
}

fn normalize_color_channel(value: f32) -> u8 {
    if value <= 1.0 {
        (value * 255.0).round().clamp(0.0, 255.0) as u8
    } else {
        value.round().clamp(0.0, 255.0) as u8
    }
}

pub fn pie_chart_render_rect(rect: GuiRect, resource: Option<&GfxResource>) -> GuiRect {
    if rect.width > 0.0 && rect.height > 0.0 {
        return rect;
    }
    let radius = resource
        .and_then(|resource| resource.size)
        .map(|size| size.x.min(size.y))
        .filter(|radius| *radius > 0.0)
        .unwrap_or(27.0);
    GuiRect::new(rect.x - radius, rect.y, radius * 2.0, radius * 2.0)
}

pub fn progress_fill_for_effect(
    rect: GuiRect,
    value: f32,
    effect: &GuiProgressEffectPolicy,
) -> GuiRect {
    let value = value.clamp(0.0, 1.0);
    match effect {
        GuiProgressEffectPolicy::DefaultProgress | GuiProgressEffectPolicy::Unsupported(_) => {
            progress_fill_rect(rect, value)
        }
        GuiProgressEffectPolicy::StartEnd => {
            let inset = (rect.height * 0.5).min(rect.width * 0.1).max(0.0);
            let x = rect.x + inset;
            let width = (rect.width - inset * 2.0).max(0.0) * value;
            GuiRect::new(x, rect.y, width, rect.height)
        }
    }
}

#[allow(dead_code)]
fn _size_from_resource(resource: Option<&GfxResource>) -> Option<GfxSize> {
    resource.and_then(|resource| resource.size)
}
