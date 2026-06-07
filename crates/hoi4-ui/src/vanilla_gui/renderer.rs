use egui::{Color32, FontId, Id, Pos2, Rect, Sense, Stroke, Vec2};

use super::ast::{GuiNode, GuiNodeKind};
use super::binding::{GuiBinding, GuiBindingMap};
use super::gfx_index::{GfxIndex, GfxResource, GfxResourceKind, GfxSize};
use super::layout::{GuiRect, LayoutNode};
use crate::icons::IconBank;
use crate::vanilla_iron::VanillaIron;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub nodes_seen: usize,
    pub nodes_painted: usize,
    pub sprites_painted: usize,
    pub fallback_painted: usize,
    pub text_painted: usize,
    pub buttons: usize,
    pub progress_bars: usize,
    pub pie_charts: usize,
    pub clicked_commands: Vec<String>,
}

pub struct VanillaGuiRenderer<'a> {
    pub gfx_index: &'a GfxIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVisualState {
    Normal,
    Hover,
    Pressed,
    Disabled,
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

pub fn frame_animated_frame(frame_count: Option<u32>, fps: Option<f32>, time_secs: f64) -> u32 {
    let frames = frame_count.unwrap_or(1).max(1);
    if frames <= 1 {
        return 1;
    }
    let fps = fps.unwrap_or(24.0).max(0.001);
    ((time_secs.max(0.0) * fps as f64).floor() as u32 % frames) + 1
}

pub fn resource_hit_rect(
    rect: Rect,
    gfx_name: &str,
    resource: Option<&GfxResource>,
    icon_bank: &mut IconBank,
) -> Rect {
    rect_for_resource_hit(rect, gfx_name, resource, icon_bank)
}

impl<'a> VanillaGuiRenderer<'a> {
    pub fn new(gfx_index: &'a GfxIndex) -> Self {
        Self { gfx_index }
    }

    pub fn paint_tree(
        &self,
        ui: &mut egui::Ui,
        node: &GuiNode,
        layout: &LayoutNode,
        bindings: &GuiBindingMap,
        icon_bank: &mut IconBank,
    ) -> RenderStats {
        let mut stats = RenderStats::default();
        self.paint_node(ui, node, layout, bindings, icon_bank, &mut stats);
        stats
    }

    fn paint_node(
        &self,
        ui: &mut egui::Ui,
        node: &GuiNode,
        layout: &LayoutNode,
        bindings: &GuiBindingMap,
        icon_bank: &mut IconBank,
        stats: &mut RenderStats,
    ) {
        stats.nodes_seen += 1;
        let binding = bindings.for_node(&layout.path, node.name.as_deref());
        let visible = binding.visible.unwrap_or(layout.visible);
        if !visible {
            return;
        }
        stats.nodes_painted += 1;

        let rect: Rect = layout.rect.into();
        let clip: Rect = layout.clip_rect.into();
        let painter = ui.painter().with_clip_rect(clip);

        match node.kind {
            GuiNodeKind::Background | GuiNodeKind::Icon => {
                if let Some(sprite) = sprite_for(node, &binding) {
                    self.paint_resource(
                        &painter,
                        rect,
                        &sprite,
                        node.f32("frame").map(|frame| frame as u32),
                        &binding,
                        node.bool("centerposition").unwrap_or(false),
                        icon_bank,
                        stats,
                    );
                }
            }
            GuiNodeKind::Button => {
                stats.buttons += 1;
                let clicked = self.paint_button(ui, rect, node, &binding, icon_bank, stats);
                if clicked {
                    if let Some(command) = binding.click.as_ref() {
                        stats.clicked_commands.push(command.command.clone());
                    }
                }
            }
            GuiNodeKind::InstantTextbox => {
                self.paint_textbox(ui, rect, node, &binding, stats);
            }
            GuiNodeKind::ContainerWindow
            | GuiNodeKind::GridBox
            | GuiNodeKind::VerticalScrollbar => {
                if let Some(sprite) = sprite_for(node, &binding) {
                    self.paint_resource(
                        &painter,
                        rect,
                        &sprite,
                        node.f32("frame").map(|frame| frame as u32),
                        &binding,
                        node.bool("centerposition").unwrap_or(false),
                        icon_bank,
                        stats,
                    );
                }
            }
            GuiNodeKind::Unknown(_) => {
                paint_fallback(&painter, rect, "unknown");
                stats.fallback_painted += 1;
            }
        }

        if !matches!(node.kind, GuiNodeKind::Button) {
            self.install_hit_region(ui, rect, &binding, stats);
        }

        for (child, child_layout) in node.children.iter().zip(layout.children.iter()) {
            self.paint_node(ui, child, child_layout, bindings, icon_bank, stats);
        }
    }

    fn paint_resource(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        gfx_name: &str,
        node_frame: Option<u32>,
        binding: &GuiBinding,
        centerposition: bool,
        icon_bank: &mut IconBank,
        stats: &mut RenderStats,
    ) -> bool {
        let resource = self.gfx_index.get(gfx_name);
        let kind = resource
            .map(|resource| &resource.kind)
            .unwrap_or(&GfxResourceKind::Sprite);

        match kind {
            GfxResourceKind::CorneredTile => {
                if let Some(resource) = resource {
                    if paint_cornered_tile(painter, rect, gfx_name, resource, icon_bank) {
                        stats.sprites_painted += 1;
                        true
                    } else {
                        paint_nine_slice_placeholder(painter, rect);
                        stats.fallback_painted += 1;
                        false
                    }
                } else if paint_sprite_resource(
                    painter,
                    rect,
                    gfx_name,
                    resource,
                    binding.frame.or(node_frame),
                    binding.tint,
                    binding.progress,
                    centerposition,
                    icon_bank,
                ) {
                    stats.sprites_painted += 1;
                    true
                } else {
                    paint_nine_slice_placeholder(painter, rect);
                    stats.fallback_painted += 1;
                    false
                }
            }
            GfxResourceKind::ProgressBar => {
                paint_progress_bar(
                    painter,
                    rect,
                    binding.progress.unwrap_or(0.0),
                    resource,
                    icon_bank,
                    stats,
                );
                stats.progress_bars += 1;
                true
            }
            GfxResourceKind::PieChart => {
                paint_pie_chart(
                    painter,
                    rect,
                    &binding.pie_segments,
                    default_pie_segments(),
                    resource,
                    icon_bank,
                    stats,
                );
                stats.pie_charts += 1;
                true
            }
            GfxResourceKind::MaskedShield => {
                if paint_masked_shield(painter, rect, gfx_name, resource, icon_bank) {
                    stats.sprites_painted += 1;
                    true
                } else {
                    paint_flag_fallback(painter, rect);
                    stats.fallback_painted += 1;
                    false
                }
            }
            GfxResourceKind::FrameAnimated => {
                if let Some(resource) = resource {
                    let frame = binding.frame.or(node_frame).unwrap_or_else(|| {
                        frame_animated_frame(
                            resource.frame_count,
                            resource.fps,
                            painter.ctx().input(|input| input.time),
                        )
                    });
                    if paint_sprite_resource(
                        painter,
                        rect,
                        gfx_name,
                        Some(resource),
                        Some(frame),
                        binding.tint,
                        binding.progress,
                        centerposition,
                        icon_bank,
                    ) {
                        stats.sprites_painted += 1;
                        true
                    } else {
                        paint_fallback(painter, rect, gfx_name);
                        stats.fallback_painted += 1;
                        false
                    }
                } else if paint_sprite_resource(
                    painter,
                    rect,
                    gfx_name,
                    resource,
                    binding.frame.or(node_frame),
                    binding.tint,
                    binding.progress,
                    centerposition,
                    icon_bank,
                ) {
                    stats.sprites_painted += 1;
                    true
                } else {
                    paint_fallback(painter, rect, gfx_name);
                    stats.fallback_painted += 1;
                    false
                }
            }
            GfxResourceKind::TextSprite | GfxResourceKind::Sprite | GfxResourceKind::Unknown(_) => {
                if paint_sprite_resource(
                    painter,
                    rect,
                    gfx_name,
                    resource,
                    binding.frame.or(node_frame),
                    binding.tint,
                    binding.progress,
                    centerposition,
                    icon_bank,
                ) {
                    stats.sprites_painted += 1;
                    true
                } else {
                    paint_fallback(painter, rect, gfx_name);
                    stats.fallback_painted += 1;
                    false
                }
            }
        }
    }

    fn paint_button(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        node: &GuiNode,
        binding: &GuiBinding,
        icon_bank: &mut IconBank,
        stats: &mut RenderStats,
    ) -> bool {
        let enabled = !node.bool("disabled").unwrap_or(false);
        let sprite = sprite_for(node, binding);
        let resource = sprite.as_deref().and_then(|name| self.gfx_index.get(name));
        let button_rect = sprite
            .as_deref()
            .map(|name| rect_for_resource_hit(rect, name, resource, icon_bank))
            .unwrap_or(rect);
        let id = Id::new((
            "vanilla_gui_button",
            node.name.as_deref(),
            button_rect.min.x.to_bits(),
            button_rect.min.y.to_bits(),
        ));
        let mut response = ui.interact(button_rect, id, Sense::click());
        if let Some(tooltip) = &binding.tooltip {
            response = response.on_hover_text(tooltip);
        }
        let painter = ui.painter();
        let mut painted_sprite = false;
        if let Some(sprite) = sprite {
            let visual_state = if !enabled {
                ButtonVisualState::Disabled
            } else if response.is_pointer_button_down_on() {
                ButtonVisualState::Pressed
            } else if response.hovered() {
                ButtonVisualState::Hover
            } else {
                ButtonVisualState::Normal
            };
            let state_frame =
                resource.map(|resource| button_frame_for_state(resource.frame_count, visual_state));
            painted_sprite = self.paint_resource(
                painter,
                snap_rect_to_physical_pixels(button_rect, painter.ctx().pixels_per_point()),
                &sprite,
                node.f32("frame").map(|frame| frame as u32).or(state_frame),
                binding,
                node.bool("centerposition").unwrap_or(false),
                icon_bank,
                stats,
            );
        }
        if !painted_sprite {
            let fill = if !enabled {
                Color32::from_rgb(0x14, 0x16, 0x15)
            } else if response.hovered() {
                Color32::from_rgb(0x24, 0x31, 0x33)
            } else {
                Color32::from_rgb(0x12, 0x15, 0x14)
            };
            painter.rect_filled(button_rect, 1.0, fill);
            VanillaIron::paint_border(painter, button_rect);
        } else if !enabled {
            painter.rect_filled(button_rect, 0.0, Color32::from_black_alpha(120));
        } else if response.is_pointer_button_down_on() {
            painter.rect_filled(button_rect, 0.0, Color32::from_black_alpha(70));
        } else if response.hovered() {
            painter.rect_filled(button_rect, 0.0, Color32::from_white_alpha(28));
        }
        let node_text = node.string("buttonText").or_else(|| node.string("text"));
        if let Some(text) = binding.text.as_deref().or(node_text.as_deref()) {
            let text = vanilla_display_text(text);
            let node_font = node.string("buttonFont").or_else(|| node.string("font"));
            let font_id = fit_font_to_width(
                text,
                FontToken::from_vanilla(node_font.as_deref()).font_id(),
                button_rect.width() - 6.0,
            );
            painter.text(
                button_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font_id,
                if enabled {
                    VanillaIron::TEXT
                } else {
                    VanillaIron::MUTED
                },
            );
        }
        enabled && response.clicked()
    }

    fn paint_textbox(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        node: &GuiNode,
        binding: &GuiBinding,
        stats: &mut RenderStats,
    ) {
        let node_text = node.string("text");
        let node_name = node.string("name");
        let Some(text) = binding
            .text
            .as_deref()
            .or(node_text.as_deref())
            .or(node_name.as_deref())
        else {
            return;
        };
        let text = vanilla_display_text(text);
        let max_w = node.f32("maxWidth").unwrap_or(rect.width());
        let max_h = node.f32("maxHeight").unwrap_or(rect.height());
        let text_rect = Rect::from_min_size(rect.min, Vec2::new(max_w, max_h));
        let painter = ui.painter().with_clip_rect(text_rect);
        let align = align_from_format(node.string("format").as_deref());
        let node_font = node.string("font");
        let font_id = fit_font_to_rect(
            text,
            FontToken::from_vanilla(node_font.as_deref()).font_id(),
            text_rect,
        );
        let color = binding
            .text_color
            .unwrap_or_else(|| text_color_from_vanilla_font(node_font.as_deref()));
        if matches!(align, egui::Align2::LEFT_TOP | egui::Align2::LEFT_CENTER)
            && text_rect.height() > 36.0
        {
            let galley = painter.layout(text.to_owned(), font_id, color, text_rect.width());
            painter.galley(text_rect.min, galley, color);
            stats.text_painted += 1;
            return;
        }
        let pos = match align {
            egui::Align2::LEFT_TOP | egui::Align2::LEFT_CENTER => text_rect.left_center(),
            egui::Align2::RIGHT_TOP | egui::Align2::RIGHT_CENTER => text_rect.right_center(),
            _ => text_rect.center(),
        };
        painter.text(pos, align, text, font_id, color);
        stats.text_painted += 1;
    }

    fn install_hit_region(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        binding: &GuiBinding,
        stats: &mut RenderStats,
    ) {
        let Some(command) = binding.click.as_ref() else {
            return;
        };
        let mut response = ui.interact(
            rect,
            Id::new(("vanilla_gui_hit", command.command.as_str())),
            Sense::click(),
        );
        if let Some(tooltip) = &binding.tooltip {
            response = response.on_hover_text(tooltip);
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            stats.clicked_commands.push(command.command.clone());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontToken {
    Title,
    Header,
    Body,
    Caption,
    Small,
}

impl FontToken {
    pub fn from_vanilla(font: Option<&str>) -> Self {
        match font.unwrap_or_default().to_ascii_lowercase().as_str() {
            "hoi_36header" | "hoi_36b" | "hoi_32b" => Self::Title,
            "hoi_24b" | "hoi_24bs" | "hoi_22b" | "hoi_22bs" | "hoi_20b" | "hoi_20bs" => {
                Self::Header
            }
            "hoi_18mbs" | "hoi_18b" | "hoi_18bs" | "hoi_16mbs" | "hoi_16b" | "hoi_16bs" => {
                Self::Body
            }
            "hoi_14mbs" | "hoi_14" | "hoi_14b" | "hoi_14bs" => Self::Caption,
            "hoi_12mbs" | "hoi_12" | "hoi_12b" | "hoi_12bs" => Self::Small,
            _ => Self::Body,
        }
    }

    pub fn font_id(self) -> FontId {
        match self {
            Self::Title => crate::v9::TextRole::Title.font_id(),
            Self::Header => crate::v9::TextRole::Heading.font_id(),
            Self::Body => crate::v9::TextRole::Body.font_id(),
            Self::Caption => crate::v9::TextRole::Caption.font_id(),
            Self::Small => crate::v9::TextRole::Small.font_id(),
        }
    }
}

fn sprite_for(node: &GuiNode, binding: &GuiBinding) -> Option<String> {
    binding
        .sprite
        .clone()
        .or_else(|| node.string("spriteType"))
        .or_else(|| node.string("quadTextureSprite"))
}

fn vanilla_display_text(text: &str) -> &str {
    match text {
        "POLITICS_NATIONAL_SPIRIT" => crate::i18n::tr("national_spirits"),
        "POLITICS_IDEOLOGY" | "ideology" => crate::i18n::tr("ideology"),
        "POLITICS_ELECTIONS" | "elections" => crate::i18n::tr("elections"),
        "POLITICS_NO_ELECTIONS" => crate::i18n::tr("no_elections"),
        "POLITICS_NATIONAL_FOCUS" => crate::i18n::tr("national_focus"),
        "National Spirits" => crate::i18n::tr("national_spirits"),
        "Government / Laws" => crate::i18n::tr("government_laws"),
        "Advisors" => crate::i18n::tr("advisors"),
        key => {
            let translated = crate::i18n::tr(key);
            if translated == key {
                text
            } else {
                translated
            }
        }
    }
}

fn paint_icon(
    painter: &egui::Painter,
    rect: Rect,
    gfx_name: &str,
    icon_bank: &mut IconBank,
) -> bool {
    let Some(handle) = icon_bank.get_or_load(gfx_name) else {
        return false;
    };
    painter.image(
        handle.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    true
}

fn paint_sprite_resource(
    painter: &egui::Painter,
    rect: Rect,
    gfx_name: &str,
    resource: Option<&GfxResource>,
    frame_override: Option<u32>,
    tint: Option<Color32>,
    progress: Option<f32>,
    centerposition: bool,
    icon_bank: &mut IconBank,
) -> bool {
    let Some(handle) = icon_bank.get_or_load(gfx_name).cloned() else {
        return false;
    };
    let source_size = GfxSize {
        x: handle.size_vec2().x,
        y: handle.size_vec2().y,
    };
    if source_size.x <= 0.0 || source_size.y <= 0.0 {
        return true;
    }

    let frame_count = resource
        .and_then(|resource| resource.frame_count)
        .unwrap_or(1)
        .max(1);
    let frame = frame_override.unwrap_or(1).clamp(1, frame_count);
    let frame_w = resource
        .and_then(|resource| resource.size)
        .map(|size| size.x)
        .filter(|width| *width > 0.0 && *width <= source_size.x)
        .unwrap_or_else(|| source_size.x / frame_count as f32);
    let frame_h = resource
        .and_then(|resource| resource.size)
        .map(|size| size.y)
        .filter(|height| *height > 0.0 && *height <= source_size.y)
        .unwrap_or(source_size.y);
    let src_left = frame_w * (frame - 1) as f32;
    let src = Rect::from_min_size(Pos2::new(src_left, 0.0), Vec2::new(frame_w, frame_h));
    let dst = rect_with_intrinsic_size_centered(rect, Vec2::new(frame_w, frame_h), centerposition);
    let mut dst = dst;
    let mut src = src;
    if let Some(progress) = progress {
        let progress = progress.clamp(0.0, 1.0);
        dst = Rect::from_min_max(
            dst.min,
            Pos2::new(dst.left() + dst.width() * progress, dst.bottom()),
        );
        src = Rect::from_min_max(
            src.min,
            Pos2::new(src.left() + src.width() * progress, src.bottom()),
        );
    }
    if dst.width() <= 0.0 || src.width() <= 0.0 {
        return true;
    }
    let dst = if centerposition {
        snap_rect_outward_to_physical_pixels(dst, painter.ctx().pixels_per_point())
    } else {
        snap_rect_to_physical_pixels(dst, painter.ctx().pixels_per_point())
    };
    paint_texture_region_tinted(
        painter,
        handle.id(),
        dst,
        src,
        source_size,
        tint.unwrap_or(Color32::WHITE),
    );
    true
}

fn rect_for_resource_hit(
    rect: Rect,
    gfx_name: &str,
    resource: Option<&GfxResource>,
    icon_bank: &mut IconBank,
) -> Rect {
    if rect.width() > 0.0 && rect.height() > 0.0 {
        return rect;
    }
    let intrinsic = resource
        .and_then(|resource| resource.size)
        .map(|size| Vec2::new(size.x, size.y))
        .filter(|size| size.x > 0.0 && size.y > 0.0)
        .or_else(|| {
            icon_bank.get_or_load(gfx_name).map(|handle| {
                let mut size = handle.size_vec2();
                let frame_count = resource
                    .and_then(|resource| resource.frame_count)
                    .unwrap_or(1)
                    .max(1) as f32;
                if frame_count > 1.0 {
                    size.x /= frame_count;
                }
                size
            })
        })
        .unwrap_or_else(|| Vec2::new(1.0, 1.0));
    rect_with_intrinsic_size(rect, intrinsic)
}

fn rect_with_intrinsic_size(rect: Rect, intrinsic: Vec2) -> Rect {
    rect_with_intrinsic_size_centered(rect, intrinsic, false)
}

fn rect_with_intrinsic_size_centered(rect: Rect, intrinsic: Vec2, centerposition: bool) -> Rect {
    let width = if rect.width() > 0.0 {
        rect.width()
    } else {
        intrinsic.x
    };
    let height = if rect.height() > 0.0 {
        rect.height()
    } else {
        intrinsic.y
    };
    let size = Vec2::new(width.max(1.0), height.max(1.0));
    if centerposition && (rect.width() <= 0.0 || rect.height() <= 0.0) {
        Rect::from_center_size(rect.min, size)
    } else {
        Rect::from_min_size(rect.min, size)
    }
}

fn snap_rect_to_physical_pixels(rect: Rect, pixels_per_point: f32) -> Rect {
    let pixels_per_point = pixels_per_point.max(1.0);
    Rect::from_min_max(
        Pos2::new(
            (rect.min.x * pixels_per_point).round() / pixels_per_point,
            (rect.min.y * pixels_per_point).round() / pixels_per_point,
        ),
        Pos2::new(
            (rect.max.x * pixels_per_point).round() / pixels_per_point,
            (rect.max.y * pixels_per_point).round() / pixels_per_point,
        ),
    )
}

fn snap_rect_outward_to_physical_pixels(rect: Rect, pixels_per_point: f32) -> Rect {
    let pixels_per_point = pixels_per_point.max(1.0);
    Rect::from_min_max(
        Pos2::new(
            (rect.min.x * pixels_per_point).floor() / pixels_per_point,
            (rect.min.y * pixels_per_point).floor() / pixels_per_point,
        ),
        Pos2::new(
            (rect.max.x * pixels_per_point).ceil() / pixels_per_point,
            (rect.max.y * pixels_per_point).ceil() / pixels_per_point,
        ),
    )
}

fn paint_texture_file(
    painter: &egui::Painter,
    rect: Rect,
    texture_file: &str,
    icon_bank: &mut IconBank,
) -> bool {
    let Some(handle) = icon_bank.get_or_load_texture_file(texture_file).cloned() else {
        return false;
    };
    painter.image(
        handle.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    true
}

fn paint_cornered_tile(
    painter: &egui::Painter,
    rect: Rect,
    gfx_name: &str,
    resource: &GfxResource,
    icon_bank: &mut IconBank,
) -> bool {
    let Some(handle) = icon_bank.get_or_load(gfx_name).cloned() else {
        return false;
    };
    let source_size = resource.size.unwrap_or_else(|| {
        let size = handle.size_vec2();
        GfxSize {
            x: size.x,
            y: size.y,
        }
    });
    if source_size.x <= 0.0 || source_size.y <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return true;
    }
    let Some(border) = resource.border else {
        paint_texture_region(
            painter,
            handle.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(source_size.x, source_size.y)),
            source_size,
        );
        return true;
    };

    let x_axis = SliceAxis::new(
        rect.left(),
        rect.right(),
        source_size.x,
        border.left,
        border.right,
    );
    let y_axis = SliceAxis::new(
        rect.top(),
        rect.bottom(),
        source_size.y,
        border.top,
        border.bottom,
    );
    let tile_center = gfx_bool_property(resource, "tilingCenter").unwrap_or(true);

    for y in 0..3 {
        for x in 0..3 {
            let src = Rect::from_min_max(
                Pos2::new(x_axis.src[x], y_axis.src[y]),
                Pos2::new(x_axis.src[x + 1], y_axis.src[y + 1]),
            );
            let dst = Rect::from_min_max(
                Pos2::new(x_axis.dst[x], y_axis.dst[y]),
                Pos2::new(x_axis.dst[x + 1], y_axis.dst[y + 1]),
            );
            if src.width() <= 0.0
                || src.height() <= 0.0
                || dst.width() <= 0.0
                || dst.height() <= 0.0
            {
                continue;
            }
            if tile_center && (x == 1 || y == 1) {
                paint_texture_region_tiled(painter, handle.id(), dst, src, source_size);
            } else {
                paint_texture_region(painter, handle.id(), dst, src, source_size);
            }
        }
    }
    true
}

fn paint_masked_shield(
    painter: &egui::Painter,
    rect: Rect,
    gfx_name: &str,
    resource: Option<&GfxResource>,
    icon_bank: &mut IconBank,
) -> bool {
    if let Some(resource) = resource {
        if let Some(overlay) = texture_file_by_key(resource, "textureFile1") {
            return paint_texture_file(painter, rect, overlay, icon_bank);
        }
    }
    paint_icon(painter, rect, gfx_name, icon_bank)
}

#[derive(Debug, Clone, Copy)]
struct SliceAxis {
    src: [f32; 4],
    dst: [f32; 4],
}

impl SliceAxis {
    fn new(dst_min: f32, dst_max: f32, source_total: f32, leading: f32, trailing: f32) -> Self {
        let source_total = source_total.max(0.0);
        let leading = leading.clamp(0.0, source_total);
        let trailing = trailing.clamp(0.0, (source_total - leading).max(0.0));
        let dst_total = (dst_max - dst_min).max(0.0);
        let border_total = leading + trailing;
        let scale = if border_total > dst_total && border_total > 0.0 {
            dst_total / border_total
        } else {
            1.0
        };
        let dst_leading = leading * scale;
        let dst_trailing = trailing * scale;
        Self {
            src: [
                0.0,
                leading,
                (source_total - trailing).max(leading),
                source_total,
            ],
            dst: [
                dst_min,
                dst_min + dst_leading,
                (dst_max - dst_trailing).max(dst_min + dst_leading),
                dst_max,
            ],
        }
    }
}

fn paint_texture_region(
    painter: &egui::Painter,
    texture_id: egui::TextureId,
    dst: Rect,
    src: Rect,
    source_size: GfxSize,
) {
    paint_texture_region_tinted(painter, texture_id, dst, src, source_size, Color32::WHITE);
}

fn paint_texture_region_tinted(
    painter: &egui::Painter,
    texture_id: egui::TextureId,
    dst: Rect,
    src: Rect,
    source_size: GfxSize,
    tint: Color32,
) {
    painter.image(texture_id, dst, normalized_uv(src, source_size), tint);
}

fn paint_texture_region_tiled(
    painter: &egui::Painter,
    texture_id: egui::TextureId,
    dst: Rect,
    src: Rect,
    source_size: GfxSize,
) {
    let tile_w = src.width().max(1.0);
    let tile_h = src.height().max(1.0);
    let mut y = dst.top();
    while y < dst.bottom() {
        let h = (dst.bottom() - y).min(tile_h);
        let mut x = dst.left();
        while x < dst.right() {
            let w = (dst.right() - x).min(tile_w);
            let tile_dst = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
            let tile_src = Rect::from_min_max(
                src.min,
                Pos2::new(
                    src.left() + src.width() * (w / tile_w),
                    src.top() + src.height() * (h / tile_h),
                ),
            );
            paint_texture_region(painter, texture_id, tile_dst, tile_src, source_size);
            x += w;
        }
        y += h;
    }
}

fn normalized_uv(src: Rect, source_size: GfxSize) -> Rect {
    Rect::from_min_max(
        Pos2::new(src.left() / source_size.x, src.top() / source_size.y),
        Pos2::new(src.right() / source_size.x, src.bottom() / source_size.y),
    )
}

fn gfx_bool_property(resource: &GfxResource, key: &str) -> Option<bool> {
    resource
        .raw_properties
        .iter()
        .find(|property| property.key.eq_ignore_ascii_case(key))
        .and_then(
            |property| match property.value.to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            },
        )
}

fn paint_nine_slice_placeholder(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x0a, 0x0d, 0x0c));
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, VanillaIron::EDGE),
        egui::epaint::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(0),
        Stroke::new(1.0, Color32::from_black_alpha(190)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn paint_progress_bar(
    painter: &egui::Painter,
    rect: Rect,
    value: f32,
    resource: Option<&GfxResource>,
    icon_bank: &mut IconBank,
    stats: &mut RenderStats,
) {
    let mut textured = false;
    if let Some(resource) = resource {
        if let Some(background) = texture_file_by_key(resource, "textureFile2") {
            if paint_texture_file(painter, rect, background, icon_bank) {
                textured = true;
                stats.sprites_painted += 1;
            }
        }
        if let Some(foreground) = texture_file_by_key(resource, "textureFile1") {
            if paint_progress_texture_file(painter, rect, foreground, value, icon_bank) {
                textured = true;
                stats.sprites_painted += 1;
            }
        }
    }
    if textured {
        return;
    }
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x16, 0x18, 0x15));
    let fill = Rect::from_min_max(
        rect.min,
        Pos2::new(
            rect.left() + rect.width() * value.clamp(0.0, 1.0),
            rect.bottom(),
        ),
    );
    painter.rect_filled(fill, 0.0, Color32::from_rgb(0x4f, 0x78, 0x52));
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(0),
        Stroke::new(1.0, Color32::from_black_alpha(190)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn paint_progress_texture_file(
    painter: &egui::Painter,
    rect: Rect,
    texture_file: &str,
    value: f32,
    icon_bank: &mut IconBank,
) -> bool {
    let Some(handle) = icon_bank.get_or_load_texture_file(texture_file).cloned() else {
        return false;
    };
    let clamped = value.clamp(0.0, 1.0);
    if clamped <= 0.0 {
        return true;
    }
    let source_size = GfxSize {
        x: handle.size_vec2().x,
        y: handle.size_vec2().y,
    };
    if source_size.x <= 0.0 || source_size.y <= 0.0 {
        return true;
    }
    let fill = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.left() + rect.width() * clamped, rect.bottom()),
    );
    let src = Rect::from_min_max(
        Pos2::ZERO,
        Pos2::new(source_size.x * clamped, source_size.y),
    );
    paint_texture_region(painter, handle.id(), fill, src, source_size);
    true
}

fn paint_pie_chart(
    painter: &egui::Painter,
    rect: Rect,
    segments: &[(f32, Color32)],
    fallback: &[(f32, Color32)],
    resource: Option<&GfxResource>,
    icon_bank: &mut IconBank,
    stats: &mut RenderStats,
) {
    let rect = resource
        .and_then(|resource| resource.size)
        .map(|size| rect_with_intrinsic_size(rect, Vec2::new(size.x, size.y)))
        .unwrap_or(rect);
    if let Some(resource) = resource {
        if let Some(texture) = resource.fallback_texture_name() {
            if paint_texture_file(painter, rect, texture, icon_bank) {
                stats.sprites_painted += 1;
            }
        }
    }
    let segments = if segments.is_empty() {
        fallback
    } else {
        segments
    };
    let total: f32 = segments.iter().map(|(value, _)| value.max(0.0)).sum();
    if total <= f32::EPSILON {
        return;
    }
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;
    painter.circle_filled(center, radius, Color32::from_rgb(0x12, 0x13, 0x10));
    let mut start = -std::f32::consts::FRAC_PI_2;
    for (value, color) in segments {
        let sweep = value.max(0.0) / total * std::f32::consts::TAU;
        let steps = ((sweep.abs() / std::f32::consts::TAU) * 48.0)
            .ceil()
            .max(2.0) as usize;
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let a = start + sweep * (i as f32 / steps as f32);
            points.push(center + Vec2::new(a.cos() * radius, a.sin() * radius));
        }
        painter.add(egui::Shape::convex_polygon(points, *color, Stroke::NONE));
        start += sweep;
    }
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(1.0, Color32::from_black_alpha(180)),
    );
    painter.circle_stroke(
        center,
        radius * 0.82,
        Stroke::new(1.0, Color32::from_white_alpha(38)),
    );
    painter.circle_filled(center, radius * 0.18, Color32::from_black_alpha(150));
    painter.circle_stroke(
        center,
        radius * 0.18,
        Stroke::new(1.0, Color32::from_black_alpha(220)),
    );
}

fn texture_file_by_key<'a>(resource: &'a GfxResource, key: &str) -> Option<&'a str> {
    resource
        .raw_properties
        .iter()
        .find(|property| property.key.eq_ignore_ascii_case(key))
        .map(|property| property.value.as_str())
}

fn fit_font_to_width(text: &str, font: FontId, max_width: f32) -> FontId {
    crate::v9::text::fit_font_to_width(text, font, max_width.max(1.0), 0.60)
}

fn fit_font_to_rect(text: &str, font: FontId, rect: Rect) -> FontId {
    let mut font = fit_font_to_width(text, font, rect.width() - 2.0);
    if rect.height() > 0.0 {
        font.size = font.size.min((rect.height() * 0.82).max(8.0));
    }
    font
}

fn paint_flag_fallback(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x2a, 0x35, 0x3a));
    painter.line_segment(
        [rect.left_top(), rect.right_bottom()],
        Stroke::new(1.0, Color32::from_white_alpha(80)),
    );
    painter.line_segment(
        [rect.right_top(), rect.left_bottom()],
        Stroke::new(1.0, Color32::from_white_alpha(80)),
    );
    VanillaIron::paint_border(painter, rect);
}

fn paint_fallback(painter: &egui::Painter, rect: Rect, label: &str) {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x42, 0x10, 0x10));
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(0),
        Stroke::new(1.0, Color32::from_rgb(0xd8, 0x30, 0x30)),
        egui::epaint::StrokeKind::Inside,
    );
    if rect.width() >= 36.0 && rect.height() >= 14.0 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            crate::v9::TextRole::Small.font_id(),
            Color32::WHITE,
        );
    }
}

fn align_from_format(format: Option<&str>) -> egui::Align2 {
    match format.unwrap_or_default().to_ascii_lowercase().as_str() {
        "right" => egui::Align2::RIGHT_CENTER,
        "center" | "centre" => egui::Align2::CENTER_CENTER,
        _ => egui::Align2::LEFT_CENTER,
    }
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
fn rect_from_gui(rect: GuiRect) -> Rect {
    rect.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_mapping_does_not_use_vanilla_fnt() {
        assert_eq!(FontToken::from_vanilla(Some("hoi_20b")), FontToken::Header);
        assert_eq!(
            FontToken::from_vanilla(Some("hoi_36header")),
            FontToken::Title
        );
        assert_eq!(FontToken::from_vanilla(Some("hoi_20bs")), FontToken::Header);
        assert_eq!(FontToken::from_vanilla(Some("hoi_18mbs")), FontToken::Body);
        assert_eq!(FontToken::from_vanilla(Some("hoi_16mbs")), FontToken::Body);
        assert_eq!(
            FontToken::from_vanilla(Some("hoi_14mbs")),
            FontToken::Caption
        );
        assert_eq!(
            FontToken::from_vanilla(Some("unknown_font")),
            FontToken::Body
        );
    }

    #[test]
    fn cjk_text_fit_shrinks_without_loading_vanilla_fnt() {
        let base = FontToken::from_vanilla(Some("hoi_20b")).font_id();
        let fitted = fit_font_to_width("非常漫长的中文政治标题", base.clone(), 56.0);

        assert!(fitted.size < base.size);
        assert!(fitted.size >= base.size * 0.60);
    }

    #[test]
    fn format_alignment_maps_common_values() {
        assert_eq!(
            align_from_format(Some("centre")),
            egui::Align2::CENTER_CENTER
        );
        assert_eq!(align_from_format(Some("right")), egui::Align2::RIGHT_CENTER);
        assert_eq!(align_from_format(None), egui::Align2::LEFT_CENTER);
    }

    #[test]
    fn typewriter_fonts_use_dark_event_ink() {
        assert_eq!(
            text_color_from_vanilla_font(Some("hoi4_typewriter16")),
            Color32::from_rgb(0x2f, 0x29, 0x1b)
        );
        assert_eq!(
            text_color_from_vanilla_font(Some("hoi_20bs")),
            VanillaIron::TEXT
        );
    }

    #[test]
    fn vanilla_text_keys_are_localized_before_painting() {
        crate::i18n::set_language(crate::i18n::Language::Chinese);

        assert_eq!(vanilla_display_text("POLITICS_NATIONAL_SPIRIT"), "国家精神");
        assert_eq!(vanilla_display_text("ideology"), "意识形态");
        assert_eq!(vanilla_display_text("elections"), "选举");
        assert_eq!(vanilla_display_text("unknown_raw_key"), "unknown_raw_key");
    }

    #[test]
    fn cornered_tile_axis_preserves_event_midsection_caps() {
        let axis = SliceAxis::new(121.0, 221.0, 66.0, 16.0, 16.0);

        assert_eq!(axis.src, [0.0, 16.0, 50.0, 66.0]);
        assert_eq!(axis.dst, [121.0, 137.0, 205.0, 221.0]);
    }

    #[test]
    fn cornered_tile_axis_scales_borders_when_rect_is_too_small() {
        let axis = SliceAxis::new(0.0, 20.0, 66.0, 16.0, 16.0);

        assert_eq!(axis.src, [0.0, 16.0, 50.0, 66.0]);
        assert_eq!(axis.dst, [0.0, 10.0, 10.0, 20.0]);
    }

    #[test]
    fn gfx_bool_property_reads_tiling_center() {
        let resource = GfxResource {
            name: "GFX_test".to_owned(),
            kind: GfxResourceKind::CorneredTile,
            source: None,
            primary_texture: None,
            textures: Vec::new(),
            size: None,
            frame_count: None,
            fps: None,
            looped: None,
            border: None,
            raw_properties: vec![crate::vanilla_gui::gfx_index::GfxProperty {
                key: "tilingCenter".to_owned(),
                value: "no".to_owned(),
            }],
        };

        assert_eq!(gfx_bool_property(&resource, "tilingCenter"), Some(false));
    }

    #[test]
    fn progress_texture_files_keep_foreground_background_roles() {
        let resource = GfxResource {
            name: "GFX_progress".to_owned(),
            kind: GfxResourceKind::ProgressBar,
            source: None,
            primary_texture: None,
            textures: Vec::new(),
            size: None,
            frame_count: None,
            fps: None,
            looped: None,
            border: None,
            raw_properties: vec![
                crate::vanilla_gui::gfx_index::GfxProperty {
                    key: "textureFile1".to_owned(),
                    value: "gfx/interface/progress_foreground.dds".to_owned(),
                },
                crate::vanilla_gui::gfx_index::GfxProperty {
                    key: "textureFile2".to_owned(),
                    value: "gfx/interface/progress_background.dds".to_owned(),
                },
            ],
        };

        assert_eq!(
            texture_file_by_key(&resource, "textureFile1"),
            Some("gfx/interface/progress_foreground.dds")
        );
        assert_eq!(
            texture_file_by_key(&resource, "textureFile2"),
            Some("gfx/interface/progress_background.dds")
        );
    }

    #[test]
    fn gate3_button_state_frames_follow_vanilla_three_frame_buttons() {
        assert_eq!(
            button_frame_for_state(Some(3), ButtonVisualState::Normal),
            1
        );
        assert_eq!(button_frame_for_state(Some(3), ButtonVisualState::Hover), 2);
        assert_eq!(
            button_frame_for_state(Some(3), ButtonVisualState::Pressed),
            3
        );
        assert_eq!(
            button_frame_for_state(Some(3), ButtonVisualState::Disabled),
            3
        );
        assert_eq!(button_frame_for_state(None, ButtonVisualState::Pressed), 1);
    }

    #[test]
    fn gate_dpi_snap_rect_aligns_to_physical_pixels() {
        let rect = Rect::from_min_size(Pos2::new(100.2, 7.2), Vec2::new(277.4, 82.4));
        let snapped = snap_rect_to_physical_pixels(rect, 1.5);

        assert!(((snapped.min.x * 1.5).round() - snapped.min.x * 1.5).abs() < f32::EPSILON);
        assert!(((snapped.min.y * 1.5).round() - snapped.min.y * 1.5).abs() < f32::EPSILON);
        assert!(((snapped.max.x * 1.5).round() - snapped.max.x * 1.5).abs() < f32::EPSILON);
        assert!(((snapped.max.y * 1.5).round() - snapped.max.y * 1.5).abs() < f32::EPSILON);
        assert!(snapped.width() > 0.0);
        assert!(snapped.height() > 0.0);
    }

    #[test]
    fn gate_dpi_snap_centered_goal_icon_rect_at_150_percent() {
        let zero_size_center = Rect::from_min_size(Pos2::new(226.0, 174.0), Vec2::ZERO);
        let icon_rect =
            rect_with_intrinsic_size_centered(zero_size_center, Vec2::new(94.0, 76.0), true);
        let snapped = snap_rect_outward_to_physical_pixels(icon_rect, 1.5);

        for value in [
            snapped.min.x,
            snapped.min.y,
            snapped.max.x,
            snapped.max.y,
        ] {
            let physical = value * 1.5;
            assert!(
                (physical.round() - physical).abs() < f32::EPSILON,
                "{value} is not physical-pixel aligned at 150% DPI"
            );
        }
        let max_center_drift = 0.001;
        assert!((snapped.center().x - zero_size_center.min.x).abs() <= max_center_drift);
        assert!((snapped.center().y - zero_size_center.min.y).abs() <= max_center_drift);
    }

    #[test]
    fn gate3_frame_animated_sprite_uses_time_based_frames() {
        assert_eq!(frame_animated_frame(Some(4), Some(2.0), 0.0), 1);
        assert_eq!(frame_animated_frame(Some(4), Some(2.0), 0.51), 2);
        assert_eq!(frame_animated_frame(Some(4), Some(2.0), 1.51), 4);
        assert_eq!(frame_animated_frame(Some(4), Some(2.0), 2.01), 1);
    }

    #[test]
    fn gate4_zero_size_button_hit_rect_uses_resource_intrinsic_size() {
        let resource = GfxResource {
            name: "GFX_add_national_goal_button".to_owned(),
            kind: GfxResourceKind::Sprite,
            source: None,
            primary_texture: Some("gfx/interface/add_national_goal_button.dds".to_owned()),
            textures: vec!["gfx/interface/add_national_goal_button.dds".to_owned()],
            size: Some(GfxSize { x: 259.0, y: 83.0 }),
            frame_count: Some(3),
            fps: None,
            looped: None,
            border: None,
            raw_properties: Vec::new(),
        };
        let ctx = egui::Context::default();
        let path_cfg = hoi4_paths::PathConfig::with_game_path(std::env::temp_dir());
        let mut icon_bank = IconBank::new(ctx, path_cfg);
        let rect = Rect::from_min_size(Pos2::new(279.0, 55.0), Vec2::ZERO);
        let hit = resource_hit_rect(
            rect,
            "GFX_add_national_goal_button",
            Some(&resource),
            &mut icon_bank,
        );

        assert_eq!(hit.min, rect.min);
        assert_eq!(hit.width(), 259.0);
        assert_eq!(hit.height(), 83.0);
    }
}
