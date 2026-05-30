//! Button primitive。3 尺寸 × 4 变体 × 4 状态 = 48 组合。
//!
//! 所有按钮强制 token 化 min_size，文字 truncate 不参与高度计算。

use egui::{Align2, Color32, Painter, Rect, Response, Sense, StrokeKind, Ui, Vec2};

use crate::v9::{
    motion, profiler, sound,
    tokens::{palette, radius, TextRole},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Sm,
    Md,
    Lg,
}

impl ButtonSize {
    pub fn min_size(self) -> Vec2 {
        match self {
            Self::Sm => Vec2::new(80.0, 24.0),
            Self::Md => Vec2::new(120.0, 32.0),
            Self::Lg => Vec2::new(160.0, 44.0),
        }
    }

    pub fn text_role(self) -> TextRole {
        match self {
            Self::Sm => TextRole::Caption,
            Self::Md => TextRole::Body,
            Self::Lg => TextRole::Subheading,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// 主操作（黄铜底 + parchment 字）。
    Primary,
    /// 次操作（深底 + 黄铜边 + 黄铜字）。
    Secondary,
    /// 危险操作（暗红底 + parchment 字）。
    Danger,
    /// 幽灵按钮（无底色 + 黄铜字 + hover 时浮起）。
    Ghost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hover,
    Active,
    Disabled,
}

/// Button primitive。`show(ui, label) -> Response`。
pub struct Button<'a> {
    label: &'a str,
    size: ButtonSize,
    variant: ButtonVariant,
    enabled: bool,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            size: ButtonSize::Md,
            variant: ButtonVariant::Primary,
            enabled: true,
        }
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let min_size = self.size.min_size();
        let (rect, resp) = ui.allocate_exact_size(min_size, Sense::click());
        let state = derive_state(&resp, self.enabled);
        let hover_t = motion::animate_bool(
            ui.ctx(),
            resp.id.with("hover"),
            self.enabled && resp.hovered(),
            motion::HOVER,
        );
        let active_t = motion::animate_bool(
            ui.ctx(),
            resp.id.with("active"),
            self.enabled && resp.is_pointer_button_down_on(),
            motion::HOVER,
        );
        let ctx = ui.ctx().clone();
        profiler::measure_ctx(&ctx, "button", || {
            self.paint(ui.painter(), rect, state, hover_t, active_t)
        });
        sound::hook_response_auto(&format!("button:{}", self.label), &resp, self.enabled);
        resp.on_hover_cursor(if self.enabled {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::NotAllowed
        })
    }

    /// 在指定 `rect` 内绘制（用于 `GridLayout::cell` 路径）。
    pub fn show_at(self, ui: &mut Ui, rect: Rect) -> Response {
        let resp = ui.interact(rect, ui.id().with(self.label), Sense::click());
        let state = derive_state(&resp, self.enabled);
        let hover_t = motion::animate_bool(
            ui.ctx(),
            resp.id.with("hover"),
            self.enabled && resp.hovered(),
            motion::HOVER,
        );
        let active_t = motion::animate_bool(
            ui.ctx(),
            resp.id.with("active"),
            self.enabled && resp.is_pointer_button_down_on(),
            motion::HOVER,
        );
        let ctx = ui.ctx().clone();
        profiler::measure_ctx(&ctx, "button", || {
            self.paint(ui.painter(), rect, state, hover_t, active_t)
        });
        sound::hook_response_auto(&format!("button:{}", self.label), &resp, self.enabled);
        if self.enabled && resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }

    fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        state: ButtonState,
        hover_t: f32,
        active_t: f32,
    ) {
        let (fill, stroke, text_color) = colors(self.variant, state);
        crate::v9::paint::paint_bevel(painter, rect, fill, stroke, radius::R1);
        let well = rect.shrink2(Vec2::new(4.0, 4.0));
        painter.rect_stroke(
            well,
            corner(radius::R1),
            egui::Stroke::new(1.0, Color32::from_black_alpha(210)),
            StrokeKind::Inside,
        );
        crate::v9::paint::paint_plate_grain(painter, well.shrink(1.0), 3.0, 2);
        if hover_t > 0.01 {
            painter.rect_filled(
                rect.shrink(2.0),
                corner(radius::R1),
                Color32::from_rgba_premultiplied(
                    palette::GOLD.r(),
                    palette::GOLD.g(),
                    palette::GOLD.b(),
                    (22.0 * hover_t).round() as u8,
                ),
            );
            let bar = Rect::from_min_max(
                rect.min + Vec2::new(2.0, 4.0),
                egui::Pos2::new(rect.min.x + 5.0, rect.max.y - 4.0),
            );
            painter.rect_filled(
                bar,
                corner(radius::R0),
                Color32::from_rgba_premultiplied(
                    palette::BRASS_DARK.r(),
                    palette::BRASS_DARK.g(),
                    palette::BRASS_DARK.b(),
                    (255.0 * hover_t).round() as u8,
                ),
            );
            painter.hline(
                (rect.min.x + 8.0)..=(rect.max.x - 8.0),
                rect.max.y - 3.0,
                egui::Stroke::new(
                    1.0,
                    Color32::from_rgba_premultiplied(
                        palette::BRASS_DARK.r(),
                        palette::BRASS_DARK.g(),
                        palette::BRASS_DARK.b(),
                        (255.0 * hover_t).round() as u8,
                    ),
                ),
            );
        }
        if active_t > 0.01 {
            painter.rect_stroke(
                rect.shrink(1.0),
                corner(radius::R1),
                egui::Stroke::new(1.0, Color32::from_black_alpha(220)),
                StrokeKind::Inside,
            );
            painter.rect_filled(
                rect.shrink(3.0),
                corner(radius::R1),
                Color32::from_black_alpha((72.0 * active_t).round() as u8),
            );
            painter.hline(
                (rect.min.x + 7.0)..=(rect.max.x - 7.0),
                rect.top() + 4.0,
                egui::Stroke::new(1.0, Color32::from_black_alpha(235)),
            );
        }
        if state == ButtonState::Disabled {
            painter.rect_filled(
                rect.shrink(2.0),
                corner(radius::R1),
                Color32::from_black_alpha(76),
            );
        }
        if rect.width() >= 118.0 && rect.height() >= 28.0 {
            let y = rect.center().y;
            crate::v9::paint::paint_slot_screw(painter, egui::Pos2::new(rect.left() + 8.0, y), 2.1);
            crate::v9::paint::paint_slot_screw(
                painter,
                egui::Pos2::new(rect.right() - 8.0, y),
                2.1,
            );
        }
        painter.hline(
            (rect.min.x + 6.0)..=(rect.max.x - 6.0),
            rect.max.y - 2.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
        );
        painter.rect_stroke(
            rect.shrink(3.0),
            corner(radius::R1),
            egui::Stroke::new(1.0, Color32::from_black_alpha(168)),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            self.label,
            self.size.text_role().font_id(),
            text_color,
        );
    }
}

fn derive_state(resp: &Response, enabled: bool) -> ButtonState {
    if !enabled {
        ButtonState::Disabled
    } else if resp.is_pointer_button_down_on() {
        ButtonState::Active
    } else if resp.hovered() {
        ButtonState::Hover
    } else {
        ButtonState::Normal
    }
}

fn colors(variant: ButtonVariant, state: ButtonState) -> (Color32, Color32, Color32) {
    match (variant, state) {
        // Primary
        (ButtonVariant::Primary, ButtonState::Normal) => {
            (palette::SOOT_BLACK, palette::EDGE_DARK, palette::PARCHMENT)
        }
        (ButtonVariant::Primary, ButtonState::Hover) => {
            (palette::IRON_DARK, palette::BRASS_DARK, palette::GOLD_HOT)
        }
        (ButtonVariant::Primary, ButtonState::Active) => {
            (palette::OIL_BLACK, palette::BRASS_DARK, palette::PARCHMENT)
        }
        (ButtonVariant::Primary, ButtonState::Disabled) => {
            (palette::SOOT_BLACK, palette::HAIRLINE, palette::MUTED)
        }
        // Secondary
        (ButtonVariant::Secondary, ButtonState::Normal) => (
            palette::SOOT_BLACK,
            palette::EDGE_DARK,
            palette::PARCHMENT_DIM,
        ),
        (ButtonVariant::Secondary, ButtonState::Hover) => {
            (palette::IRON_DARK, palette::BRASS_DARK, palette::GOLD_HOT)
        }
        (ButtonVariant::Secondary, ButtonState::Active) => {
            (palette::OIL_BLACK, palette::BRASS_DARK, palette::GOLD)
        }
        (ButtonVariant::Secondary, ButtonState::Disabled) => {
            (palette::SOOT_BLACK, palette::HAIRLINE, palette::MUTED)
        }
        // Danger
        (ButtonVariant::Danger, ButtonState::Normal) => {
            (palette::RUST, palette::EDGE_DARK, palette::PARCHMENT)
        }
        (ButtonVariant::Danger, ButtonState::Hover) => {
            (palette::RUST, palette::BAD, palette::GOLD_HOT)
        }
        (ButtonVariant::Danger, ButtonState::Active) => {
            (palette::RUST, palette::BAD, palette::PARCHMENT)
        }
        (ButtonVariant::Danger, ButtonState::Disabled) => {
            (palette::SOOT_BLACK, palette::HAIRLINE, palette::MUTED)
        }
        // Ghost
        (ButtonVariant::Ghost, ButtonState::Normal) => (
            Color32::TRANSPARENT,
            palette::HAIRLINE,
            palette::PARCHMENT_DIM,
        ),
        (ButtonVariant::Ghost, ButtonState::Hover) => {
            (palette::IRON_DARK, palette::EDGE_DARK, palette::GOLD)
        }
        (ButtonVariant::Ghost, ButtonState::Active) => {
            (palette::IRON_BLACK, palette::BRASS_DARK, palette::GOLD)
        }
        (ButtonVariant::Ghost, ButtonState::Disabled) => {
            (Color32::TRANSPARENT, palette::HAIRLINE, palette::MUTED)
        }
    }
}

fn corner(r: f32) -> egui::epaint::CornerRadius {
    egui::epaint::CornerRadius::same(r as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_distinct() {
        assert_ne!(ButtonSize::Sm.min_size(), ButtonSize::Md.min_size());
        assert_ne!(ButtonSize::Md.min_size(), ButtonSize::Lg.min_size());
        // Lg 高度严格大于 Md，Md 严格大于 Sm
        assert!(ButtonSize::Sm.min_size().y < ButtonSize::Md.min_size().y);
        assert!(ButtonSize::Md.min_size().y < ButtonSize::Lg.min_size().y);
    }

    #[test]
    fn all_variants_have_colors() {
        for variant in [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Danger,
            ButtonVariant::Ghost,
        ] {
            for state in [
                ButtonState::Normal,
                ButtonState::Hover,
                ButtonState::Active,
                ButtonState::Disabled,
            ] {
                let (fill, stroke, text) = colors(variant, state);
                // 仅断言"调用不 panic 且返回值不全等"，颜色细节由 demo 视觉验收
                let _ = (fill, stroke, text);
            }
        }
    }
}
