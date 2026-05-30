//! Tabs primitive：顶部标签栏（chip 风 + 激活底部下划线）。

use egui::{Align2, Color32, Rect, Sense, Ui};

use crate::v9::{
    motion, profiler, sound,
    tokens::{border, palette, radius, spacing, TextRole},
};

/// 单个 tab item。
pub struct TabItem<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub badge: Option<u32>,
}

impl<'a> TabItem<'a> {
    pub fn new(id: &'a str, label: &'a str) -> Self {
        Self {
            id,
            label,
            badge: None,
        }
    }

    pub fn with_badge(mut self, count: u32) -> Self {
        self.badge = Some(count);
        self
    }
}

/// 标签栏。`show_at(rect, &items, &mut active_id)` 在 rect 中横向绘制 tabs，
/// 点击修改 `active_id`。返回新选中的 id。
pub struct TabBar;

impl TabBar {
    /// 在 `rect` 内绘制。返回点击后的新激活 id（如果点了），否则保持原值。
    pub fn show_at<'a>(
        ui: &mut Ui,
        rect: Rect,
        items: &[TabItem<'a>],
        active: &mut &'a str,
    ) -> Option<&'a str> {
        if items.is_empty() {
            return None;
        }
        let mut clicked: Option<&'a str> = None;
        let chip_w = rect.width() / items.len() as f32;
        for (i, item) in items.iter().enumerate() {
            let chip_rect = Rect::from_min_max(
                egui::Pos2::new(rect.min.x + chip_w * i as f32, rect.min.y),
                egui::Pos2::new(rect.min.x + chip_w * (i + 1) as f32, rect.max.y),
            );
            let id = ui.id().with(("v9_tab", item.id));
            let resp = ui.interact(chip_rect, id, Sense::click());
            let is_active = *active == item.id;
            let active_t =
                motion::animate_bool(ui.ctx(), id.with("active"), is_active, motion::TAB_SWITCH);
            let hover_t =
                motion::animate_bool(ui.ctx(), id.with("hover"), resp.hovered(), motion::HOVER);
            let ctx = ui.ctx().clone();
            profiler::measure_ctx(&ctx, "tabs", || {
                paint_chip(
                    ui.painter(),
                    chip_rect,
                    item,
                    is_active,
                    resp.hovered(),
                    active_t,
                    hover_t,
                )
            });
            sound::hook_response_auto(&format!("tab:{}", item.id), &resp, true);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                *active = item.id;
                clicked = Some(item.id);
            }
        }
        clicked
    }
}

fn paint_chip(
    painter: &egui::Painter,
    rect: Rect,
    item: &TabItem,
    active: bool,
    hover: bool,
    active_t: f32,
    hover_t: f32,
) {
    let (fill, stroke_color, text_color) = match (active, hover) {
        (true, _) => (palette::OIL_BLACK, palette::BRASS_DARK, palette::GOLD_HOT),
        (false, true) => (palette::IRON_DARK, palette::EDGE_DARK, palette::GOLD),
        (false, false) => (
            palette::SOOT_BLACK,
            palette::EDGE_DARK,
            palette::PARCHMENT_DIM,
        ),
    };
    crate::v9::paint::paint_bevel(painter, rect, fill, stroke_color, radius::R1);
    crate::v9::paint::paint_plate_grain(painter, rect.shrink(4.0), 3.0, 2);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        item.label,
        TextRole::Subheading.font_id(),
        text_color,
    );
    if active_t > 0.01 {
        painter.hline(
            (rect.min.x + spacing::S3)..=(rect.max.x - spacing::S3),
            rect.max.y - 3.0,
            egui::Stroke::new(
                border::B2,
                Color32::from_rgba_premultiplied(
                    palette::GOLD.r(),
                    palette::GOLD.g(),
                    palette::GOLD.b(),
                    (255.0 * active_t).round() as u8,
                ),
            ),
        );
        painter.hline(
            (rect.min.x + spacing::S4)..=(rect.max.x - spacing::S4),
            rect.max.y - 6.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(220)),
        );
        painter.hline(
            (rect.min.x + spacing::S4)..=(rect.max.x - spacing::S4),
            rect.top() + 2.0,
            egui::Stroke::new(1.0, Color32::from_white_alpha(4)),
        );
    } else if hover_t > 0.01 {
        painter.hline(
            (rect.min.x + spacing::S3)..=(rect.max.x - spacing::S3),
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
    if let Some(n) = item.badge {
        let badge_rect = Rect::from_min_size(
            egui::Pos2::new(rect.max.x - 14.0, rect.min.y + 2.0),
            egui::Vec2::new(12.0, 12.0),
        );
        painter.circle_filled(badge_rect.center(), 6.0, palette::BAD);
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            &format!("{}", n.min(99)),
            TextRole::Small.font_id(),
            Color32::WHITE,
        );
    }
}
