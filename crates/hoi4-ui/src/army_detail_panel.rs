//! 左侧军团详情面板 —— ROADMAP_MILITARY_UI_PARITY.md Phase C。
//!
//! 选中集团军时,在左侧 `egui::Area` 浮窗显示:
//! - 头部:大立绘 + 集团军/将领名 + 4 项加成数字
//! - 中段:该集团军师列表(NATO 符号 + 名 + 组织度条 + 装备% + 兵力%)
//! - 底部:师数 / 指挥上限 / 效率
//!
//! 这块只展示信息;命令(画线/箭头/执行)继续留在底栏命令条,职责分离。

use egui::Color32;

use crate::{
    i18n::tr,
    military::{ArmyEntry, DivisionEntry, MilitaryCommand, MilitaryData},
    nato_icon,
};

pub struct ArmyDetailPanel;

fn v9_show_army_detail(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
    use crate::v9::{
        frame::{FrameStyle, PanelFrame},
        layout::{GridLayout, Track},
        primitives::{Button, ButtonSize, ButtonVariant},
        tokens::{palette, spacing, Elevation, TextRole},
    };
    use egui::{Align2, Area, Id, Order, Pos2, Rect, Sense, Stroke, Vec2};

    let mut cmds = Vec::new();
    let Some(army_id) = data.selected_army_id else {
        return cmds;
    };
    let Some(army) = data.armies.iter().find(|a| a.id == army_id) else {
        return cmds;
    };

    let divisions: Vec<&DivisionEntry> = data
        .divisions
        .iter()
        .filter(|d| d.army_id == Some(army.id))
        .collect();
    let accent = v9_army_accent(army, divisions.len());
    let limit = army
        .command_limit
        .map(|limit| format!("{}/{}", divisions.len(), limit))
        .unwrap_or_else(|| "无将领".to_owned());

    let screen = ctx.screen_rect();
    let reserved_left = if screen.width() >= 900.0 {
        88.0
    } else {
        spacing::S4
    };
    let available_w = (screen.width() - reserved_left - spacing::S6).max(260.0);
    let width = (screen.width() * 0.24).clamp(340.0, 460.0).min(available_w);
    let height = (screen.height() * 0.52)
        .clamp(360.0, 620.0)
        .min((screen.height() - 164.0).max(300.0));
    let max_x = (screen.right() - width - spacing::S4).max(screen.left() + spacing::S4);
    let max_y = (screen.bottom() - height - 70.0).max(screen.top() + spacing::S4);
    let pos = Pos2::new(
        (screen.left() + reserved_left).min(max_x),
        (screen.top() + 98.0).min(max_y),
    );
    let footer_text = format!("{} 师团 | 指挥 {}", divisions.len(), limit);

    Area::new(Id::new("army_detail_tactical_drawer_v9"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
            let frame = PanelFrame::new(FrameStyle::Glass, outer)
                .with_accent(accent)
                .with_elevation(Elevation::E2);
            frame.draw(ui.painter());

            let inner = frame.inner_rect();
            let grid = GridLayout::new(
                vec![
                    Track::Fixed(56.0),
                    Track::Fixed(74.0),
                    Track::Fixed(72.0),
                    Track::Fr(1.0),
                    Track::Fixed(26.0),
                ],
                vec![Track::Fr(1.0)],
            )
            .with_gutter(0.0, spacing::S3);
            let cells = grid.measure(inner);
            let header = GridLayout::cell(&cells, 0, 0);
            let summary = GridLayout::cell(&cells, 1, 0);
            let status = GridLayout::cell(&cells, 2, 0);
            let roster = GridLayout::cell(&cells, 3, 0);
            let footer = GridLayout::cell(&cells, 4, 0);

            crate::v9::paint::paint_recessed_panel(ui.painter(), header, 1.0);
            ui.painter().hline(
                header.left()..=header.right(),
                header.bottom() - 1.0,
                Stroke::new(1.0, accent),
            );
            let close_rect = Rect::from_min_size(
                Pos2::new(header.right() - 30.0, header.top() + 8.0),
                Vec2::new(24.0, 24.0),
            );
            let title_clip = Rect::from_min_max(
                header.left_top(),
                Pos2::new(close_rect.left() - spacing::S3, header.bottom()),
            );
            let title_painter = ui.painter().with_clip_rect(title_clip);
            title_painter.text(
                Pos2::new(header.left() + spacing::S5, header.top() + 10.0),
                Align2::LEFT_TOP,
                army.name.as_str(),
                TextRole::Heading.font_id(),
                palette::GOLD_HOT,
            );
            title_painter.text(
                Pos2::new(header.left() + spacing::S5, header.bottom() - spacing::S3),
                Align2::LEFT_BOTTOM,
                army.commander_name.as_deref().unwrap_or("无将领"),
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );
            if Button::new("×")
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .show_at(ui, close_rect)
                .clicked()
            {
                cmds.push(MilitaryCommand::ClearArmySelection);
            }

            crate::v9::composites::panel_shell::draw_summary_tiles(
                ui,
                summary,
                &[
                    ("师团", divisions.len().to_string(), palette::GOLD),
                    ("指挥", limit.clone(), accent),
                    (
                        "效率",
                        format!("{:.0}%", army.command_efficiency * 100.0),
                        ratio_color(army.command_efficiency),
                    ),
                    (
                        "攻/防",
                        format!(
                            "+{:.0}/+{:.0}%",
                            army.attack_bonus_pct, army.defense_bonus_pct
                        ),
                        palette::GOLD,
                    ),
                ],
            );

            v9_army_status_card(ui, status, army, accent, &mut cmds);
            v9_army_roster_card(ui, roster, divisions.as_slice());

            let footer_painter = ui.painter().with_clip_rect(footer);
            footer_painter.text(
                Pos2::new(footer.left(), footer.center().y),
                Align2::LEFT_CENTER,
                footer_text,
                TextRole::Caption.font_id(),
                palette::MUTED,
            );
        });

    cmds
}

fn v9_army_status_card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    army: &ArmyEntry,
    accent: Color32,
    cmds: &mut Vec<MilitaryCommand>,
) {
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Card};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_ornate().show_at(ui, rect);
    let (status, status_color) = if army.executing {
        ("执行中", palette::GOOD)
    } else if army.has_arrow {
        ("计划就绪", palette::GOLD_HOT)
    } else if army.has_path {
        ("已有前线", palette::GOLD)
    } else {
        ("待命", palette::MUTED)
    };

    let title_clip = Rect::from_min_max(
        inner.left_top(),
        Pos2::new(inner.right() - 112.0, inner.bottom()),
    );
    let painter = ui.painter().with_clip_rect(title_clip);
    painter.text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "集团军状态",
        TextRole::Subheading.font_id(),
        accent,
    );
    painter.text(
        Pos2::new(inner.left(), inner.top() + 26.0),
        Align2::LEFT_TOP,
        status,
        TextRole::Heading.font_id(),
        status_color,
    );
    painter.text(
        Pos2::new(inner.left(), inner.top() + 50.0),
        Align2::LEFT_TOP,
        format!(
            "计划 +{:.0}%  恢复 +{:.0}%  补给 -{:.0}%",
            army.planning_bonus_pct, army.org_recovery_bonus_pct, army.supply_reduction_pct
        ),
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );

    let clear_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 104.0, inner.center().y - 14.0),
        Vec2::new(100.0, 28.0),
    );
    if Button::new("取消选中")
        .size(ButtonSize::Sm)
        .variant(ButtonVariant::Secondary)
        .show_at(ui, clear_rect)
        .clicked()
    {
        cmds.push(MilitaryCommand::ClearArmySelection);
    }
}

fn v9_army_roster_card(ui: &mut egui::Ui, rect: egui::Rect, divisions: &[&DivisionEntry]) {
    use crate::v9::primitives::Card;
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        format!("师团列表 ({})", divisions.len()),
        TextRole::Subheading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let list_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 30.0),
        inner.right_bottom(),
    );
    if divisions.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            list_rect,
            "暂无师团",
            "把师团加入集团军后会显示在这里。",
        );
        return;
    }

    ui.allocate_ui_at_rect(list_rect, |ui| {
        ui.set_clip_rect(list_rect);
        ui.set_min_size(list_rect.size());
        egui::ScrollArea::vertical()
            .id_salt("army_detail_roster_scroll_v9")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(list_rect.width());
                let row_h = 42.0;
                for div in divisions {
                    let (row, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_h),
                        Sense::hover(),
                    );
                    v9_draw_division_row(ui, row, div);
                    ui.add_space(spacing::S2);
                }
            });
    });
}

fn v9_draw_division_row(ui: &mut egui::Ui, row: egui::Rect, div: &DivisionEntry) {
    use crate::v9::primitives::{draw_progress_bar, CounterIcon};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    crate::v9::paint::paint_bevel(
        ui.painter(),
        row,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        2.0,
    );

    let counter_w: f32 = if row.width() < 360.0 { 132.0 } else { 158.0 };
    let counter_rect = Rect::from_min_size(
        row.left_top() + Vec2::new(spacing::S3, spacing::S3),
        Vec2::new(counter_w.min(row.width() * 0.44), 28.0),
    );
    let archetype = nato_icon::archetype_from_template_name(&div.name);
    CounterIcon::new(tr(&div.name), archetype)
        .accent(if div.in_combat {
            palette::BAD
        } else {
            palette::BRASS_BRIGHT
        })
        .show_at(ui, counter_rect);

    let stats_w = if row.width() < 380.0 { 70.0 } else { 118.0 };
    let stats_rect = Rect::from_min_max(
        Pos2::new(row.right() - stats_w - spacing::S3, row.top()),
        row.right_bottom() - Vec2::new(spacing::S3, 0.0),
    );
    let bar_left = counter_rect.right() + spacing::S3;
    let bar_right = stats_rect.left() - spacing::S3;
    let org = div_org_ratio(div);
    if bar_right - bar_left >= 64.0 {
        let bar_area = Rect::from_min_max(
            Pos2::new(bar_left, row.top() + 6.0),
            Pos2::new(bar_right, row.bottom() - 6.0),
        );
        let bar_painter = ui.painter().with_clip_rect(bar_area);
        bar_painter.text(
            bar_area.left_top(),
            Align2::LEFT_TOP,
            format!("组织 {:.0}%", org * 100.0),
            TextRole::Caption.font_id(),
            ratio_color(org),
        );
        draw_progress_bar(
            ui,
            Rect::from_min_size(
                Pos2::new(bar_area.left(), row.bottom() - 14.0),
                Vec2::new(bar_area.width(), 8.0),
            ),
            org,
            ratio_color(org),
        );
    }

    let stats = if row.width() < 380.0 {
        format!(
            "{:.0}/{:.0}",
            div.equipment_ratio * 100.0,
            div.strength * 100.0
        )
    } else {
        format!(
            "装 {:.0}%  兵 {:.0}%",
            div.equipment_ratio * 100.0,
            div.strength * 100.0
        )
    };
    ui.painter().with_clip_rect(stats_rect).text(
        Pos2::new(stats_rect.right(), stats_rect.center().y),
        Align2::RIGHT_CENTER,
        stats,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
}

fn v9_army_accent(army: &ArmyEntry, division_count: usize) -> Color32 {
    if army
        .command_limit
        .is_some_and(|limit| division_count > limit as usize)
    {
        crate::v9::tokens::palette::BAD
    } else if army.executing {
        crate::v9::tokens::palette::GOOD
    } else if army.has_arrow {
        crate::v9::tokens::palette::GOLD_HOT
    } else if army.has_path {
        crate::v9::tokens::palette::GOLD
    } else {
        crate::v9::tokens::palette::BRASS_BRIGHT
    }
}

fn div_org_ratio(div: &DivisionEntry) -> f32 {
    if div.max_organisation <= 0.0 {
        0.0
    } else {
        (div.organisation / div.max_organisation).clamp(0.0, 1.0)
    }
}

fn ratio_color(value: f32) -> Color32 {
    if value < 0.5 {
        crate::v9::tokens::palette::BAD
    } else if value < 0.8 {
        crate::v9::tokens::palette::WARN
    } else {
        crate::v9::tokens::palette::GOOD
    }
}

impl ArmyDetailPanel {
    pub fn show(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
        v9_show_army_detail(ctx, data)
    }
}
