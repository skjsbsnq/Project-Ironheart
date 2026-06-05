//! V9 left side rail for panel entry points.

use egui::{
    Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use crate::{
    frame_model::PanelKind,
    v9::{
        paint,
        primitives::Tooltip,
        sound,
        tokens::{palette, spacing, TextRole},
    },
};

pub const SIDE_RAIL_X: f32 = 8.0;
pub const SIDE_RAIL_W: f32 = 106.0;
pub const SIDE_RAIL_TOP_OFFSET: f32 = 92.0;
pub const SIDE_RAIL_PANEL_GAP: f32 = 8.0;
pub const SIDE_RAIL_PANEL_LEFT: f32 = SIDE_RAIL_X + SIDE_RAIL_W + SIDE_RAIL_PANEL_GAP;

const ITEM_H: f32 = 44.0;
const ITEM_GAP: f32 = 0.0;

#[derive(Debug, Clone)]
pub struct SideRailEntry {
    pub kind: PanelKind,
    pub short: &'static str,
    pub label: &'static str,
    pub hotkey: &'static str,
    pub badge: u8,
}

impl SideRailEntry {
    pub const fn new(
        kind: PanelKind,
        short: &'static str,
        label: &'static str,
        hotkey: &'static str,
    ) -> Self {
        Self {
            kind,
            short,
            label,
            hotkey,
            badge: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SideRailData {
    pub active: Option<PanelKind>,
    pub entries: Vec<SideRailEntry>,
}

impl SideRailData {
    pub fn gameplay(active: Option<PanelKind>) -> Self {
        Self {
            active,
            entries: default_entries().to_vec(),
        }
    }

    pub fn with_badge(mut self, kind: PanelKind, badge: u8) -> Self {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.kind == kind) {
            entry.badge = badge;
        }
        self
    }
}

pub struct SideRail;

impl SideRail {
    pub fn show(ctx: &Context, data: &SideRailData) -> Option<PanelKind> {
        let screen = ctx.screen_rect();
        let height = rail_height(data.entries.len());
        let available_h = (screen.height() - SIDE_RAIL_TOP_OFFSET - spacing::S5).max(0.0);
        let size = Vec2::new(SIDE_RAIL_W, height.min(available_h));
        let pos = Pos2::new(
            screen.left() + SIDE_RAIL_X,
            screen.top() + SIDE_RAIL_TOP_OFFSET,
        );
        let mut clicked = None;

        Area::new(Id::new("v9_side_rail"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .default_size(size)
            .show(ctx, |ui| {
                let (rail_rect, _) = ui.allocate_exact_size(size, Sense::hover());
                paint_rail_frame(ui, rail_rect);

                let mut y = rail_rect.top() + 5.0;
                for entry in &data.entries {
                    let item_rect = Rect::from_min_size(
                        Pos2::new(rail_rect.left() + 5.0, y),
                        Vec2::new(rail_rect.width() - 10.0, ITEM_H),
                    );
                    let response = ui.interact(
                        item_rect,
                        ui.id().with(("v9_side_rail", entry.short)),
                        Sense::click(),
                    );
                    let active = data.active == Some(entry.kind);
                    paint_entry(ui, item_rect, entry, active, response.hovered());
                    sound::hook_response_auto(
                        &format!("side_rail:{}", entry.short),
                        &response,
                        true,
                    );
                    Tooltip::titled(entry.label, entry.hotkey)
                        .accent(if entry.badge > 0 {
                            palette::WARN
                        } else {
                            palette::BRASS_BRIGHT
                        })
                        .show_for_response(ui, &response);
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if response.clicked() {
                        clicked = Some(entry.kind);
                    }
                    y += ITEM_H + ITEM_GAP;
                }
            });

        clicked
    }
}

fn paint_entry(ui: &mut egui::Ui, rect: Rect, entry: &SideRailEntry, active: bool, hovered: bool) {
    let painter = ui.painter();
    let accent = if active {
        Color32::from_rgb(0xd7, 0xc0, 0x78)
    } else if hovered {
        Color32::from_rgb(0xb9, 0xa0, 0x68)
    } else {
        Color32::from_rgb(0x42, 0x3b, 0x2b)
    };
    let fill = if active {
        Color32::from_rgb(0x28, 0x27, 0x21)
    } else if hovered {
        Color32::from_rgb(0x1f, 0x20, 0x1b)
    } else {
        Color32::from_rgb(0x10, 0x12, 0x10)
    };
    painter.rect_filled(rect, 1.0, fill);
    paint::paint_vertical_gradient_mesh(
        painter,
        rect.shrink(1.0),
        Color32::from_white_alpha(if active { 14 } else { 7 }),
        Color32::from_black_alpha(120),
    );
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, accent),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, Color32::from_black_alpha(220)),
        StrokeKind::Inside,
    );
    if active {
        painter.rect_filled(
            Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + 4.0, rect.bottom())),
            0.0,
            accent,
        );
    }

    let icon_rect = Rect::from_min_size(rect.left_top() + Vec2::new(10.0, 9.0), Vec2::splat(26.0));
    let icon_color = if active || hovered {
        Color32::from_rgb(0xe1, 0xd3, 0xa5)
    } else {
        Color32::from_rgb(0xb7, 0xb1, 0x9a)
    };
    draw_generated_svg_icon(painter, icon_rect, entry.kind, icon_color);
    painter.text(
        Pos2::new(rect.left() + 45.0, rect.center().y),
        Align2::LEFT_CENTER,
        entry.label,
        TextRole::Caption.font_id(),
        if active || hovered {
            Color32::from_rgb(0xe1, 0xd3, 0xa5)
        } else {
            Color32::from_rgb(0xb7, 0xb1, 0x9a)
        },
    );

    if entry.badge > 0 {
        let dot = Pos2::new(rect.right() - 5.0, rect.top() + 5.0);
        painter.circle_filled(dot, 4.0, palette::WARN);
        painter.circle_stroke(dot, 4.0, Stroke::new(1.0, Color32::from_black_alpha(180)));
    }
}

fn paint_rail_frame(ui: &mut egui::Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x08, 0x0a, 0x09));
    paint::paint_vertical_gradient_mesh(
        painter,
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(0x18, 0x18, 0x15, 238),
        Color32::from_rgba_premultiplied(0x05, 0x06, 0x05, 250),
    );
    paint::paint_plate_grain(painter, rect.shrink(3.0), 3.0, 2);
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        Stroke::new(2.0, Color32::from_rgb(0x3a, 0x35, 0x28)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        egui::epaint::CornerRadius::same(0),
        Stroke::new(1.0, Color32::from_black_alpha(230)),
        StrokeKind::Inside,
    );
}

fn draw_generated_svg_icon(painter: &egui::Painter, rect: Rect, kind: PanelKind, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    let thin = Stroke::new(1.35, color);
    let p = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            rect.left() + rect.width() * x / 24.0,
            rect.top() + rect.height() * y / 24.0,
        )
    };
    let rr =
        |x: f32, y: f32, w: f32, h: f32| -> Rect { Rect::from_min_max(p(x, y), p(x + w, y + h)) };
    match kind {
        PanelKind::Politics => {
            svg_polyline(painter, &[p(3.0, 9.0), p(12.0, 4.0), p(21.0, 9.0)], stroke);
            painter.line_segment([p(5.0, 10.0), p(19.0, 10.0)], stroke);
            for x in [7.0, 11.0, 15.0] {
                painter.line_segment([p(x, 11.0), p(x, 18.0)], stroke);
            }
            painter.line_segment([p(5.0, 19.0), p(19.0, 19.0)], stroke);
            painter.line_segment([p(3.5, 21.0), p(20.5, 21.0)], stroke);
        }
        PanelKind::Decisions => {
            painter.rect_stroke(
                rr(6.0, 4.0, 12.0, 17.0),
                corner(1.0),
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                rr(9.0, 3.0, 6.0, 4.0),
                corner(1.0),
                thin,
                StrokeKind::Inside,
            );
            for y in [10.0, 14.0, 18.0] {
                painter.line_segment([p(9.0, y), p(16.0, y)], thin);
            }
        }
        PanelKind::Laws => {
            painter.line_segment([p(12.0, 5.0), p(12.0, 20.0)], stroke);
            painter.line_segment([p(6.0, 8.0), p(18.0, 8.0)], stroke);
            painter.line_segment([p(8.0, 8.0), p(5.0, 15.0)], thin);
            painter.line_segment([p(8.0, 8.0), p(11.0, 15.0)], thin);
            painter.line_segment([p(16.0, 8.0), p(13.0, 15.0)], thin);
            painter.line_segment([p(16.0, 8.0), p(19.0, 15.0)], thin);
            svg_polyline(
                painter,
                &[
                    p(4.5, 15.0),
                    p(11.5, 15.0),
                    p(10.0, 17.0),
                    p(6.0, 17.0),
                    p(4.5, 15.0),
                ],
                thin,
            );
            svg_polyline(
                painter,
                &[
                    p(12.5, 15.0),
                    p(19.5, 15.0),
                    p(18.0, 17.0),
                    p(14.0, 17.0),
                    p(12.5, 15.0),
                ],
                thin,
            );
            painter.line_segment([p(8.0, 21.0), p(16.0, 21.0)], stroke);
        }
        PanelKind::Pops => {
            painter.circle_stroke(p(12.0, 7.0), rect.width() * 2.2 / 24.0, stroke);
            painter.circle_stroke(p(6.5, 10.0), rect.width() * 1.8 / 24.0, thin);
            painter.circle_stroke(p(17.5, 10.0), rect.width() * 1.8 / 24.0, thin);
            svg_polyline(
                painter,
                &[
                    p(5.0, 20.0),
                    p(7.0, 15.0),
                    p(12.0, 13.0),
                    p(17.0, 15.0),
                    p(19.0, 20.0),
                ],
                stroke,
            );
            painter.line_segment([p(3.5, 20.0), p(20.5, 20.0)], thin);
        }
        PanelKind::Market => {
            svg_polyline(
                painter,
                &[p(4.0, 18.0), p(9.0, 13.0), p(13.0, 15.0), p(20.0, 7.0)],
                stroke,
            );
            painter.line_segment([p(4.0, 20.0), p(21.0, 20.0)], thin);
            painter.line_segment([p(4.0, 20.0), p(4.0, 5.0)], thin);
            svg_polyline(painter, &[p(17.5, 7.0), p(20.0, 7.0), p(20.0, 9.5)], stroke);
        }
        PanelKind::Finance => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 7.0 / 24.0, stroke);
            painter.line_segment([p(12.0, 6.0), p(12.0, 18.0)], thin);
            svg_polyline(
                painter,
                &[
                    p(15.5, 8.5),
                    p(10.0, 8.5),
                    p(8.5, 10.5),
                    p(10.0, 12.0),
                    p(14.0, 12.0),
                    p(15.5, 13.5),
                    p(14.0, 15.5),
                    p(8.5, 15.5),
                ],
                thin,
            );
        }
        PanelKind::Trade => {
            painter.rect_stroke(
                rr(5.0, 5.0, 5.5, 5.5),
                corner(1.0),
                thin,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                rr(13.5, 13.5, 5.5, 5.5),
                corner(1.0),
                thin,
                StrokeKind::Inside,
            );
            svg_polyline(
                painter,
                &[p(10.5, 8.0), p(17.5, 8.0), p(17.5, 11.0)],
                stroke,
            );
            svg_polyline(
                painter,
                &[p(13.5, 16.0), p(6.5, 16.0), p(6.5, 13.0)],
                stroke,
            );
            svg_polyline(
                painter,
                &[p(16.0, 9.5), p(17.5, 11.0), p(19.0, 9.5)],
                stroke,
            );
            svg_polyline(painter, &[p(8.0, 14.5), p(6.5, 13.0), p(5.0, 14.5)], stroke);
        }
        PanelKind::Construction => {
            painter.line_segment([p(5.0, 20.0), p(5.0, 7.0)], stroke);
            painter.line_segment([p(5.0, 7.0), p(17.0, 7.0)], stroke);
            painter.line_segment([p(5.0, 10.0), p(12.0, 7.0)], thin);
            painter.line_segment([p(9.0, 7.0), p(5.0, 12.0)], thin);
            painter.line_segment([p(17.0, 7.0), p(20.0, 10.0)], thin);
            painter.line_segment([p(17.0, 7.0), p(17.0, 14.0)], thin);
            painter.rect_stroke(
                rr(15.5, 14.0, 3.0, 3.0),
                corner(0.0),
                thin,
                StrokeKind::Inside,
            );
            painter.line_segment([p(3.0, 20.0), p(9.0, 20.0)], stroke);
        }
        PanelKind::Research => {
            painter.line_segment([p(9.0, 4.0), p(15.0, 4.0)], stroke);
            painter.line_segment([p(10.0, 4.0), p(10.0, 11.0)], thin);
            painter.line_segment([p(14.0, 4.0), p(14.0, 11.0)], thin);
            svg_polyline(
                painter,
                &[p(10.0, 11.0), p(6.0, 20.0), p(18.0, 20.0), p(14.0, 11.0)],
                stroke,
            );
            painter.line_segment([p(8.0, 16.0), p(16.0, 16.0)], thin);
        }
        PanelKind::Diplomacy => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 6.0 / 24.0, stroke);
            svg_polyline(
                painter,
                &[
                    p(5.0, 18.0),
                    p(8.0, 21.0),
                    p(12.0, 19.0),
                    p(16.0, 21.0),
                    p(19.0, 18.0),
                ],
                thin,
            );
            svg_polyline(
                painter,
                &[
                    p(5.0, 6.0),
                    p(8.0, 3.0),
                    p(12.0, 5.0),
                    p(16.0, 3.0),
                    p(19.0, 6.0),
                ],
                thin,
            );
            painter.circle_filled(p(12.0, 12.0), rect.width() * 1.5 / 24.0, color);
        }
        PanelKind::Military => {
            svg_polyline(
                painter,
                &[
                    p(5.0, 13.0),
                    p(7.0, 8.0),
                    p(12.0, 6.0),
                    p(17.0, 8.0),
                    p(19.0, 13.0),
                ],
                stroke,
            );
            painter.line_segment([p(5.0, 13.0), p(19.0, 13.0)], stroke);
            painter.line_segment([p(7.0, 16.0), p(17.0, 16.0)], thin);
            painter.line_segment([p(9.0, 19.0), p(15.0, 19.0)], thin);
        }
        PanelKind::Naval => {
            painter.line_segment([p(12.0, 5.0), p(12.0, 18.0)], stroke);
            painter.circle_stroke(p(12.0, 7.0), rect.width() * 2.0 / 24.0, thin);
            painter.line_segment([p(8.0, 11.0), p(16.0, 11.0)], stroke);
            svg_polyline(
                painter,
                &[
                    p(5.0, 15.0),
                    p(7.0, 20.0),
                    p(12.0, 22.0),
                    p(17.0, 20.0),
                    p(19.0, 15.0),
                ],
                stroke,
            );
            painter.line_segment([p(5.0, 15.0), p(8.0, 15.0)], thin);
            painter.line_segment([p(16.0, 15.0), p(19.0, 15.0)], thin);
        }
        PanelKind::Air => {
            svg_polyline(
                painter,
                &[
                    p(3.5, 13.0),
                    p(20.5, 6.0),
                    p(17.0, 13.0),
                    p(20.0, 18.0),
                    p(13.5, 15.0),
                    p(8.0, 20.0),
                    p(9.5, 14.0),
                    p(3.5, 13.0),
                ],
                stroke,
            );
        }
        PanelKind::Logistics => {
            painter.rect_stroke(
                rr(5.0, 8.0, 14.0, 11.0),
                corner(1.0),
                stroke,
                StrokeKind::Inside,
            );
            svg_polyline(painter, &[p(5.0, 8.0), p(12.0, 4.5), p(19.0, 8.0)], stroke);
            painter.line_segment([p(12.0, 4.5), p(12.0, 19.0)], thin);
            painter.line_segment([p(5.0, 13.0), p(19.0, 13.0)], thin);
        }
        PanelKind::Situation => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 7.0 / 24.0, stroke);
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 3.0 / 24.0, thin);
            painter.line_segment([p(12.0, 3.0), p(12.0, 7.0)], thin);
            painter.line_segment([p(12.0, 17.0), p(12.0, 21.0)], thin);
            painter.line_segment([p(3.0, 12.0), p(7.0, 12.0)], thin);
            painter.line_segment([p(17.0, 12.0), p(21.0, 12.0)], thin);
        }
        PanelKind::Settings | PanelKind::Saves => {
            painter.circle_stroke(p(12.0, 12.0), rect.width() * 6.0 / 24.0, stroke);
            for angle in [0.0_f32, 0.785, 1.57, 2.355, 3.14, 3.925, 4.71, 5.495] {
                let dir = Vec2::angled(angle);
                painter.line_segment(
                    [
                        p(12.0, 12.0) + dir * rect.width() * 0.28,
                        p(12.0, 12.0) + dir * rect.width() * 0.40,
                    ],
                    thin,
                );
            }
        }
    }
}

fn svg_polyline(painter: &egui::Painter, points: &[Pos2], stroke: Stroke) {
    painter.add(egui::Shape::line(points.to_vec(), stroke));
}

fn corner(r: f32) -> egui::epaint::CornerRadius {
    egui::epaint::CornerRadius::same(r as u8)
}

fn rail_height(items: usize) -> f32 {
    10.0 + items as f32 * ITEM_H + items.saturating_sub(1) as f32 * ITEM_GAP
}

fn default_entries() -> &'static [SideRailEntry; 15] {
    &DEFAULT_ENTRIES
}

const DEFAULT_ENTRIES: [SideRailEntry; 15] = [
    SideRailEntry::new(PanelKind::Politics, "政", "政治", "Q"),
    SideRailEntry::new(PanelKind::Decisions, "决", "决议", "D"),
    SideRailEntry::new(PanelKind::Laws, "法", "法案", "P"),
    SideRailEntry::new(PanelKind::Pops, "民", "人口", "F9"),
    SideRailEntry::new(PanelKind::Market, "市", "市场", "K"),
    SideRailEntry::new(PanelKind::Finance, "财", "财政", "F"),
    SideRailEntry::new(PanelKind::Trade, "贸", "贸易", "G"),
    SideRailEntry::new(PanelKind::Construction, "建", "建设", "B"),
    SideRailEntry::new(PanelKind::Research, "研", "科研", "Y"),
    SideRailEntry::new(PanelKind::Diplomacy, "外", "外交", "U"),
    SideRailEntry::new(PanelKind::Military, "陆", "陆军", "I"),
    SideRailEntry::new(PanelKind::Naval, "海", "海军", "O"),
    SideRailEntry::new(PanelKind::Air, "空", "空军", "A"),
    SideRailEntry::new(PanelKind::Logistics, "物", "后勤", "L"),
    SideRailEntry::new(PanelKind::Situation, "局", "局势", "J"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_rail_has_fifteen_panel_entries() {
        assert_eq!(default_entries().len(), 15);
        assert!(!default_entries()
            .iter()
            .any(|entry| entry.short == "PRD" || entry.label == "Production"));
    }

    #[test]
    fn badge_update_targets_entry() {
        let data = SideRailData::gameplay(None).with_badge(PanelKind::Situation, 3);
        let entry = data
            .entries
            .iter()
            .find(|entry| entry.kind == PanelKind::Situation)
            .unwrap();
        assert_eq!(entry.badge, 3);
    }
}
