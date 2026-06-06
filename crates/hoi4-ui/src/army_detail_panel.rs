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

fn iron_show_army_detail(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
    use crate::{
        v9::{
            composites::side_rail::SIDE_RAIL_PANEL_LEFT,
            layout::{GridLayout, Track},
            text::fit_font_to_width,
            tokens::{spacing, TextRole},
        },
        vanilla_iron::VanillaIron,
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
    let accent = army_accent(army, divisions.len());
    let limit = army
        .command_limit
        .map(|limit| format!("{}/{}", divisions.len(), limit))
        .unwrap_or_else(|| "无将领".to_owned());

    let screen = ctx.screen_rect();
    let reserved_left = if screen.width() >= 420.0 {
        SIDE_RAIL_PANEL_LEFT
    } else {
        spacing::S4
    };
    let available_w = (screen.width() - reserved_left - spacing::S6).max(260.0);
    let width = (screen.width() * 0.25).clamp(360.0, 480.0).min(available_w);
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

    Area::new(Id::new("army_detail_tactical_drawer_iron"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
            VanillaIron::paint_panel(ui, outer, accent);

            let inner = outer.shrink2(Vec2::new(12.0, 10.0));
            let grid = GridLayout::new(
                vec![
                    Track::Fixed(54.0),
                    Track::Fixed(70.0),
                    Track::Fixed(74.0),
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

            VanillaIron::paint_region(ui.painter(), header, VanillaIron::CARD_DEEP);
            ui.painter().hline(
                header.left()..=header.right(),
                header.bottom() - 1.0,
                Stroke::new(1.0, VanillaIron::EDGE),
            );
            let close_rect = Rect::from_min_size(
                Pos2::new(header.right() - 28.0, header.top() + 6.0),
                Vec2::new(23.0, 23.0),
            );
            let title_clip = Rect::from_min_max(
                header.left_top() + Vec2::new(spacing::S4, spacing::S3),
                Pos2::new(close_rect.left() - spacing::S3, header.bottom()),
            );
            let title_painter = ui.painter().with_clip_rect(title_clip);
            let title_font = fit_font_to_width(
                army.name.as_str(),
                TextRole::Heading.font_id(),
                title_clip.width(),
                0.70,
            );
            title_painter.text(
                title_clip.left_top(),
                Align2::LEFT_TOP,
                army.name.as_str(),
                title_font,
                VanillaIron::TEXT,
            );
            title_painter.text(
                Pos2::new(title_clip.left(), title_clip.bottom() - spacing::S2),
                Align2::LEFT_BOTTOM,
                army.commander_name.as_deref().unwrap_or("无将领"),
                TextRole::Caption.font_id(),
                VanillaIron::MUTED,
            );
            if VanillaIron::close_button(ui, close_rect, ("army_detail_close", army.id))
                .on_hover_text("取消选中")
                .clicked()
            {
                cmds.push(MilitaryCommand::ClearArmySelection);
            }

            iron_metric_tiles(
                ui,
                summary,
                &[
                    ("师团", divisions.len().to_string(), VanillaIron::BRASS),
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
                        VanillaIron::BRASS,
                    ),
                ],
            );

            iron_army_status_card(ui, status, army, accent, &mut cmds);
            iron_army_roster_card(ui, roster, divisions.as_slice());

            ui.painter().hline(
                footer.left()..=footer.right(),
                footer.top(),
                Stroke::new(1.0, VanillaIron::EDGE_DARK),
            );
            let footer_painter = ui.painter().with_clip_rect(footer);
            footer_painter.text(
                Pos2::new(footer.left() + spacing::S1, footer.center().y),
                Align2::LEFT_CENTER,
                footer_text,
                TextRole::Caption.font_id(),
                VanillaIron::MUTED,
            );
        });

    cmds
}

fn iron_metric_tiles(ui: &mut egui::Ui, rect: egui::Rect, items: &[(&str, String, Color32)]) {
    use crate::{
        v9::{text::fit_font_to_width, tokens::TextRole},
        vanilla_iron::VanillaIron,
    };
    use egui::{Align2, Pos2, Rect, Vec2};

    if items.is_empty() || rect.width() <= 8.0 || rect.height() <= 8.0 {
        return;
    }
    let gap = 6.0;
    let cols = if rect.width() < 340.0 { 2 } else { 4 };
    let rows = items.len().div_ceil(cols);
    let tile_w = (rect.width() - gap * (cols.saturating_sub(1) as f32)) / cols as f32;
    let tile_h = (rect.height() - gap * (rows.saturating_sub(1) as f32)) / rows as f32;

    for (idx, (label, value, color)) in items.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let tile = Rect::from_min_size(
            rect.left_top() + Vec2::new(col as f32 * (tile_w + gap), row as f32 * (tile_h + gap)),
            Vec2::new(tile_w, tile_h),
        );
        VanillaIron::paint_region(ui.painter(), tile, VanillaIron::CARD);
        let inner = tile.shrink2(Vec2::new(7.0, 5.0));
        ui.painter().with_clip_rect(inner).text(
            inner.left_top(),
            Align2::LEFT_TOP,
            *label,
            fit_font_to_width(label, TextRole::Caption.font_id(), inner.width(), 0.70),
            VanillaIron::MUTED,
        );
        ui.painter().with_clip_rect(inner).text(
            Pos2::new(inner.left(), inner.bottom()),
            Align2::LEFT_BOTTOM,
            value,
            fit_font_to_width(value, TextRole::Subheading.font_id(), inner.width(), 0.64),
            *color,
        );
    }
}

fn iron_army_status_card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    army: &ArmyEntry,
    accent: Color32,
    cmds: &mut Vec<MilitaryCommand>,
) {
    use crate::{
        v9::{text::fit_font_to_width, tokens::TextRole},
        vanilla_iron::VanillaIron,
    };
    use egui::{Align2, Pos2, Rect, Vec2};

    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD_SOFT);
    let inner = rect.shrink2(Vec2::new(10.0, 8.0));
    let (status, status_color) = if army.executing {
        ("执行中", VanillaIron::GOOD)
    } else if army.has_arrow {
        ("计划就绪", VanillaIron::BRASS_BRIGHT)
    } else if army.has_path {
        ("已有前线", VanillaIron::BRASS)
    } else {
        ("待命", VanillaIron::MUTED)
    };

    let clear_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 96.0, inner.center().y - 13.0),
        Vec2::new(92.0, 26.0),
    );
    let title_clip = Rect::from_min_max(
        inner.left_top(),
        Pos2::new(clear_rect.left() - 8.0, inner.bottom()),
    );
    let painter = ui.painter().with_clip_rect(title_clip);
    painter.text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "集团军状态",
        TextRole::Subheading.font_id(),
        accent,
    );
    let status_font = fit_font_to_width(
        status,
        TextRole::Heading.font_id(),
        title_clip.width(),
        0.72,
    );
    painter.text(
        Pos2::new(inner.left(), inner.top() + 26.0),
        Align2::LEFT_TOP,
        status,
        status_font,
        status_color,
    );
    draw_army_wrapped_text(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 49.0),
            Pos2::new(title_clip.right(), inner.bottom()),
        ),
        &format!(
            "计划 +{:.0}%  恢复 +{:.0}%  补给 -{:.0}%",
            army.planning_bonus_pct, army.org_recovery_bonus_pct, army.supply_reduction_pct
        ),
        TextRole::Caption,
        VanillaIron::MUTED,
    );

    if VanillaIron::compact_button_at(ui, clear_rect, "取消选中", ("army_detail_clear", army.id))
        .clicked()
    {
        cmds.push(MilitaryCommand::ClearArmySelection);
    }
}

fn iron_army_roster_card(ui: &mut egui::Ui, rect: egui::Rect, divisions: &[&DivisionEntry]) {
    use crate::{v9::tokens::spacing, vanilla_iron::VanillaIron};
    use egui::{Pos2, Rect, Sense, Vec2};

    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD_DEEP);
    let title_rect =
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), rect.top() + 28.0));
    VanillaIron::section_title_at(ui, title_rect, &format!("师团列表 ({})", divisions.len()));

    let list_rect = Rect::from_min_max(
        Pos2::new(rect.left() + spacing::S4, title_rect.bottom() + spacing::S3),
        rect.right_bottom() - Vec2::new(spacing::S4, spacing::S4),
    );
    if divisions.is_empty() {
        draw_army_empty_state(
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
                    iron_draw_division_row(ui, row, div);
                    ui.add_space(spacing::S2);
                }
            });
    });
}

fn iron_draw_division_row(ui: &mut egui::Ui, row: egui::Rect, div: &DivisionEntry) {
    use crate::v9::primitives::{draw_progress_bar, CounterIcon};
    use crate::v9::tokens::{spacing, TextRole};
    use crate::vanilla_iron::VanillaIron;
    use egui::{Align2, Pos2, Rect, Vec2};

    VanillaIron::paint_region(ui.painter(), row, VanillaIron::CARD);

    let counter_w: f32 = if row.width() < 360.0 { 132.0 } else { 158.0 };
    let counter_rect = Rect::from_min_size(
        row.left_top() + Vec2::new(spacing::S3, spacing::S3),
        Vec2::new(counter_w.min(row.width() * 0.44), 28.0),
    );
    let archetype = nato_icon::archetype_from_template_name(&div.name);
    CounterIcon::new(tr(&div.name), archetype)
        .accent(if div.in_combat {
            VanillaIron::BAD
        } else {
            VanillaIron::BRASS_BRIGHT
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
        VanillaIron::MUTED,
    );
}

fn draw_army_empty_state(ui: &mut egui::Ui, rect: egui::Rect, title: &str, body: &str) {
    use crate::{v9::tokens::TextRole, vanilla_iron::VanillaIron};
    use egui::{Align2, Pos2};

    ui.painter().text(
        Pos2::new(rect.center().x, rect.center().y - 10.0),
        Align2::CENTER_CENTER,
        title,
        TextRole::Subheading.font_id(),
        VanillaIron::MUTED,
    );
    draw_army_wrapped_text(
        ui,
        egui::Rect::from_min_max(
            Pos2::new(rect.left() + 16.0, rect.center().y + 8.0),
            rect.right_bottom() - egui::Vec2::new(16.0, 8.0),
        ),
        body,
        TextRole::Caption,
        VanillaIron::MUTED,
    );
}

fn draw_army_wrapped_text(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    text: &str,
    role: crate::v9::TextRole,
    color: Color32,
) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(rect);
    let galley = painter.layout(text.to_owned(), role.font_id(), color, rect.width());
    painter.galley(rect.left_top(), galley, color);
}

fn army_accent(army: &ArmyEntry, division_count: usize) -> Color32 {
    use crate::vanilla_iron::VanillaIron;

    if army
        .command_limit
        .is_some_and(|limit| division_count > limit as usize)
    {
        VanillaIron::BAD
    } else if army.executing {
        VanillaIron::GOOD
    } else if army.has_arrow {
        VanillaIron::BRASS_BRIGHT
    } else if army.has_path {
        VanillaIron::BRASS
    } else {
        VanillaIron::BRASS_BRIGHT
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
    use crate::vanilla_iron::VanillaIron;

    if value < 0.5 {
        VanillaIron::BAD
    } else if value < 0.8 {
        VanillaIron::WARN
    } else {
        VanillaIron::GOOD
    }
}

impl ArmyDetailPanel {
    pub fn show(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
        iron_show_army_detail(ctx, data)
    }
}
