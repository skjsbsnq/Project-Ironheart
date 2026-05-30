//! V9 left side rail for panel entry points.

use egui::{
    Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use crate::{
    frame_model::PanelKind,
    v9::{
        frame::{FrameStyle, PanelFrame},
        paint,
        primitives::Tooltip,
        sound,
        tokens::{palette, spacing, Elevation, TextRole},
    },
};

const RAIL_W: f32 = 58.0;
const ITEM_H: f32 = 30.0;
const ITEM_GAP: f32 = 5.0;
const TOP_OFFSET: f32 = 92.0;

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
        let pos = Pos2::new(screen.left() + spacing::S3, screen.top() + TOP_OFFSET);
        let size = Vec2::new(
            RAIL_W,
            height.min(screen.height() - TOP_OFFSET - spacing::S5),
        );
        let mut clicked = None;

        Area::new(Id::new("v9_side_rail"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .default_size(size)
            .show(ctx, |ui| {
                let (rail_rect, _) = ui.allocate_exact_size(size, Sense::hover());
                PanelFrame::new(FrameStyle::Glass, rail_rect)
                    .with_accent(palette::BRASS_DARK)
                    .with_elevation(Elevation::E2)
                    .draw(ui.painter());

                let mut y = rail_rect.top() + spacing::S4;
                for entry in &data.entries {
                    let item_rect = Rect::from_min_size(
                        Pos2::new(rail_rect.left() + spacing::S3, y),
                        Vec2::new(rail_rect.width() - spacing::S3 * 2.0, ITEM_H),
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
        palette::GOLD_HOT
    } else if hovered {
        palette::BRASS_BRIGHT
    } else {
        palette::BRASS_DARK
    };
    let fill = if active {
        palette::IRON
    } else if hovered {
        palette::IRON_DARK
    } else {
        palette::SOOT_BLACK
    };
    paint::paint_bevel(painter, rect, fill, accent, 1.0);
    painter.rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(1),
        Stroke::new(1.0, Color32::from_black_alpha(210)),
        StrokeKind::Inside,
    );
    if active || hovered {
        painter.hline(
            (rect.left() + 6.0)..=(rect.right() - 6.0),
            rect.bottom() - 3.0,
            Stroke::new(1.0, accent),
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        entry.short,
        TextRole::Code.font_id(),
        if active || hovered {
            palette::PARCHMENT
        } else {
            palette::PARCHMENT_DIM
        },
    );

    if entry.badge > 0 {
        let dot = Pos2::new(rect.right() - 5.0, rect.top() + 5.0);
        painter.circle_filled(dot, 4.0, palette::WARN);
        painter.circle_stroke(dot, 4.0, Stroke::new(1.0, Color32::from_black_alpha(180)));
    }
}

fn rail_height(items: usize) -> f32 {
    spacing::S4 * 2.0 + items as f32 * ITEM_H + items.saturating_sub(1) as f32 * ITEM_GAP
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
