//! 右上军团摘要徽章 —— ROADMAP_MILITARY_UI_PARITY.md Phase D。
//!
//! 选中集团军时,在右上角(对位 vanilla HoI4"西班牙第1战区"位置)显示一个
//! 紧凑摘要徽章:迷你立绘 + 集团军名 + 师数 + 状态。我们没有 Theater 数据,
//! 所以这块对位为"选中军团摘要"。
//!
//! 右键徽章 → 取消选中。

use egui::Color32;

use crate::military::{MilitaryCommand, MilitaryData};

pub struct ArmyBadge;

fn v9_show_army_badge(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
    use crate::nato_icon::NatoArchetype;
    use crate::v9::{
        frame::{FrameStyle, PanelFrame},
        primitives::CounterIcon,
        tokens::{palette, spacing, Elevation, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let mut cmds = Vec::new();
    let Some(army_id) = data.selected_army_id else {
        return cmds;
    };
    let Some(army) = data.armies.iter().find(|a| a.id == army_id) else {
        return cmds;
    };

    let screen = ctx.screen_rect();
    let size = Vec2::new(254.0, 72.0);
    let pos = Pos2::new(screen.right() - size.x - spacing::S6, screen.top() + 58.0);
    egui::Area::new(egui::Id::new("army_summary_badge_v9"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::click());
            let accent = v9_badge_accent(army);
            let frame = PanelFrame::new(FrameStyle::Glass, rect)
                .with_accent(accent)
                .with_elevation(Elevation::E2);
            frame.draw(ui.painter());
            let inner = frame.inner_rect();
            let response = ui
                .interact(
                    rect,
                    egui::Id::new("army_summary_badge_v9_interact"),
                    Sense::click(),
                )
                .on_hover_text("右键取消选中集团军");
            if response.secondary_clicked() {
                cmds.push(MilitaryCommand::ClearArmySelection);
            }

            let counter_label = format!("{} 师", army.member_count);
            CounterIcon::new(counter_label.as_str(), NatoArchetype::Infantry)
                .accent(accent)
                .selected(true)
                .show_at(
                    ui,
                    Rect::from_min_size(inner.left_top(), Vec2::new(82.0, 30.0)),
                );

            let text_left = inner.left() + 92.0;
            let painter = ui.painter().with_clip_rect(Rect::from_min_max(
                Pos2::new(text_left, inner.top()),
                inner.right_bottom(),
            ));
            painter.text(
                Pos2::new(text_left, inner.top() + 8.0),
                Align2::LEFT_TOP,
                army.name.as_str(),
                TextRole::Subheading.font_id(),
                palette::PARCHMENT,
            );
            painter.text(
                Pos2::new(text_left, inner.top() + 29.0),
                Align2::LEFT_TOP,
                army.commander_name.as_deref().unwrap_or("无将领"),
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );
            painter.text(
                Pos2::new(inner.right(), inner.bottom() - 2.0),
                Align2::RIGHT_BOTTOM,
                v9_badge_status(army),
                TextRole::Caption.font_id(),
                accent,
            );
        });

    cmds
}

fn v9_badge_status(army: &crate::military::ArmyEntry) -> &'static str {
    let over_limit = army
        .command_limit
        .is_some_and(|limit| army.member_count > limit as usize);
    if over_limit {
        "超限"
    } else if army.executing {
        "执行中"
    } else if army.has_arrow {
        "计划"
    } else if army.has_path {
        "前线"
    } else {
        "待命"
    }
}

fn v9_badge_accent(army: &crate::military::ArmyEntry) -> Color32 {
    let over_limit = army
        .command_limit
        .is_some_and(|limit| army.member_count > limit as usize);
    if over_limit {
        crate::v9::tokens::palette::BAD
    } else if army.executing {
        crate::v9::tokens::palette::GOOD
    } else if army.has_arrow {
        crate::v9::tokens::palette::GOLD_HOT
    } else if army.has_path {
        crate::v9::tokens::palette::GOLD
    } else {
        crate::v9::tokens::palette::MUTED
    }
}

impl ArmyBadge {
    pub fn show(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
        v9_show_army_badge(ctx, data)
    }
}
