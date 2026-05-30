//! List primitive：虚拟化斑马纹列表 + 选中态。
//!
//! Phase A 不做完整虚拟化（行数 < 1000 直接画），仅提供统一斑马纹 + 选中态 + 选中色条接口。

use egui::{Align2, Color32, Pos2, Rect, Sense, Ui};

use crate::v9::{
    accessibility, profiler, sound,
    tokens::{palette, spacing, TextRole},
};

/// 列表项。
pub struct ListItem<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub trailing: Option<&'a str>,
    pub accent: Option<Color32>,
}

impl<'a> ListItem<'a> {
    pub fn new(id: &'a str, label: &'a str) -> Self {
        Self {
            id,
            label,
            trailing: None,
            accent: None,
        }
    }

    pub fn with_trailing(mut self, s: &'a str) -> Self {
        self.trailing = Some(s);
        self
    }

    pub fn with_accent(mut self, color: Color32) -> Self {
        self.accent = Some(color);
        self
    }
}

/// 列表视图。`show_at(rect, items, selected)` 在 rect 内绘制列表，点击修改 selected。
pub struct ListView {
    pub row_height: f32,
}

impl ListView {
    pub fn new() -> Self {
        Self { row_height: 28.0 }
    }

    pub fn with_row_height(mut self, h: f32) -> Self {
        self.row_height = h;
        self
    }

    /// 在 `rect` 内绘制，返回新选中的 id（如点击了）。
    pub fn show_at<'a>(
        &self,
        ui: &mut Ui,
        rect: Rect,
        items: &[ListItem<'a>],
        selected: &mut Option<&'a str>,
    ) -> Option<&'a str> {
        let mut clicked: Option<&'a str> = None;
        let max_visible = ((rect.height() / self.row_height) as usize).min(items.len());
        for (i, item) in items.iter().take(max_visible).enumerate() {
            let row_rect = Rect::from_min_max(
                Pos2::new(rect.min.x, rect.min.y + self.row_height * i as f32),
                Pos2::new(rect.max.x, rect.min.y + self.row_height * (i + 1) as f32),
            );
            let id = ui.id().with(("v9_list", item.id, rect_id_key(row_rect)));
            let resp = ui.interact(row_rect, id, Sense::click());
            let is_selected = *selected == Some(item.id);
            let ctx = ui.ctx().clone();
            profiler::measure_ctx(&ctx, "list_row", || {
                paint_row(ui.painter(), row_rect, i, item, is_selected, resp.hovered())
            });
            sound::hook_response_auto(&format!("list:{}", item.id), &resp, true);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                *selected = Some(item.id);
                clicked = Some(item.id);
            }
        }
        clicked
    }
}

impl Default for ListView {
    fn default() -> Self {
        Self::new()
    }
}

fn paint_row(
    painter: &egui::Painter,
    rect: Rect,
    idx: usize,
    item: &ListItem,
    selected: bool,
    hover: bool,
) {
    let fill = if selected {
        Color32::from_rgba_premultiplied(0x0d, 0x0c, 0x08, 244)
    } else if hover {
        Color32::from_rgba_premultiplied(0x0c, 0x0e, 0x0c, 238)
    } else if idx % 2 == 0 {
        Color32::from_rgba_premultiplied(0x06, 0x07, 0x06, 238)
    } else {
        Color32::from_rgba_premultiplied(0x09, 0x0a, 0x09, 238)
    };
    painter.rect_filled(rect, egui::epaint::CornerRadius::ZERO, fill);
    crate::v9::paint::paint_plate_grain(painter, rect.shrink(2.0), 3.0, 2);
    crate::v9::paint::paint_speckle(painter, rect.shrink(2.0), 4, 1);
    painter.hline(
        rect.min.x..=rect.max.x,
        rect.max.y - 0.5,
        egui::Stroke::new(1.0, Color32::from_black_alpha(235)),
    );
    painter.hline(
        (rect.min.x + 2.0)..=(rect.max.x - 2.0),
        rect.min.y + 0.5,
        egui::Stroke::new(
            1.0,
            Color32::from_white_alpha(if hover || selected { 4 } else { 1 }),
        ),
    );

    if selected {
        painter.hline(
            (rect.min.x + 5.0)..=(rect.max.x - 5.0),
            rect.max.y - 2.0,
            egui::Stroke::new(1.0, palette::BRASS_DARK),
        );
        painter.hline(
            rect.min.x..=rect.max.x,
            rect.max.y - 0.5,
            egui::Stroke::new(1.0, Color32::from_black_alpha(245)),
        );
        let bar = Rect::from_min_max(
            rect.min + egui::Vec2::new(1.0, 2.0),
            Pos2::new(rect.min.x + 4.0, rect.max.y - 2.0),
        );
        painter.rect_filled(bar, egui::epaint::CornerRadius::ZERO, palette::GOLD);
    } else if hover {
        let bar = Rect::from_min_max(
            rect.min + egui::Vec2::new(1.0, 3.0),
            Pos2::new(rect.min.x + 3.0, rect.max.y - 3.0),
        );
        painter.rect_filled(bar, egui::epaint::CornerRadius::ZERO, palette::BRASS_DARK);
    }

    let text_x = if let Some(accent) = item.accent {
        let accessible = accessibility::semantic_tone_for_color(accent)
            .map(|tone| accessibility::style_for(painter.ctx(), tone));
        let accent = accessible.map(|style| style.color).unwrap_or(accent);
        let bar_x = rect.min.x + (if selected { 9.0 } else { 8.0 });
        let bar = Rect::from_min_size(
            Pos2::new(bar_x, rect.center().y - 8.0),
            egui::Vec2::new(3.0, 16.0),
        );
        painter.rect_filled(bar, egui::epaint::CornerRadius::ZERO, accent);
        if let Some(style) = accessible {
            accessibility::paint_marker_with_painter(
                painter,
                Rect::from_center_size(
                    Pos2::new(bar.right() + 7.0, rect.center().y),
                    egui::Vec2::splat(10.0),
                ),
                style.marker,
                accent,
            );
        }
        painter.hline(
            bar.min.x..=bar.max.x,
            bar.min.y + 0.5,
            egui::Stroke::new(1.0, Color32::from_white_alpha(7)),
        );
        painter.hline(
            bar.min.x..=bar.max.x,
            bar.max.y - 0.5,
            egui::Stroke::new(1.0, Color32::from_black_alpha(170)),
        );
        if accessible.is_some() {
            bar_x + 22.0
        } else {
            bar_x + 10.0
        }
    } else {
        rect.min.x + spacing::S5
    };

    let trailing_w = if item.trailing.is_some() { 76.0 } else { 0.0 };
    let label_clip = Rect::from_min_max(
        Pos2::new(text_x, rect.top()),
        Pos2::new(
            (rect.right() - spacing::S5 - trailing_w).max(text_x),
            rect.bottom(),
        ),
    );
    let label_painter = painter.with_clip_rect(label_clip);
    label_painter.text(
        Pos2::new(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        item.label,
        TextRole::Body.font_id(),
        if selected {
            palette::GOLD
        } else {
            palette::PARCHMENT_DIM
        },
    );

    if let Some(t) = item.trailing {
        let trailing_clip = Rect::from_min_max(
            Pos2::new(
                (rect.right() - spacing::S5 - trailing_w).max(text_x),
                rect.top(),
            ),
            Pos2::new(rect.right() - spacing::S3, rect.bottom()),
        );
        let trailing_painter = painter.with_clip_rect(trailing_clip);
        trailing_painter.text(
            Pos2::new(rect.max.x - spacing::S5, rect.center().y),
            Align2::RIGHT_CENTER,
            t,
            TextRole::Numeric.font_id(),
            if selected {
                palette::GOLD
            } else {
                palette::BRASS_BRIGHT
            },
        );
    }
}

fn rect_id_key(rect: Rect) -> (i32, i32, i32, i32) {
    (
        rect.min.x.round() as i32,
        rect.min.y.round() as i32,
        rect.width().round() as i32,
        rect.height().round() as i32,
    )
}
