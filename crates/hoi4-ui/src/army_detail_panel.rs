//! 左侧军团详情面板 —— ROADMAP_MILITARY_UI_PARITY.md Phase C。
//!
//! 选中集团军时,在左侧 `egui::Area` 浮窗显示:
//! - 头部:大立绘 + 集团军/将领名 + 4 项加成数字
//! - 中段:该集团军师列表(NATO 符号 + 名 + 组织度条 + 装备% + 兵力%)
//! - 底部:师数 / 指挥上限 / 效率
//!
//! 这块只展示信息;命令(画线/箭头/执行)继续留在底栏命令条,职责分离。

use egui::{Color32, RichText};

use crate::{
    i18n::tr,
    military::{ArmyEntry, DivisionEntry, MilitaryCommand, MilitaryData},
    nato_icon,
    portrait::{self, PortraitStyle},
};

const PANEL_BG: Color32 = Color32::from_rgba_premultiplied(0x1a, 0x12, 0x0a, 232);
const GOLD: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);
const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xe0, 0xc0, 0x78);
const MUTED: Color32 = Color32::from_gray(150);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);
const COUNTRY_COLOR_DEFAULT: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);

pub struct ArmyDetailPanel;

fn v9_show_army_detail(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

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
    let (close, _) = PanelShell::new("army_detail_panel_v9", army.name.as_str())
        .subtitle(army.commander_name.as_deref().unwrap_or("无将领"))
        .class(PanelClass::Compact)
        .accent(accent)
        .footer("Q Close  |  Division CounterIcon roster")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("师团", divisions.len().to_string(), palette::GOLD),
                    ("指挥", limit, accent),
                    (
                        "效率",
                        format!("{:.0}%", army.command_efficiency * 100.0),
                        ratio_color(army.command_efficiency),
                    ),
                    (
                        "攻击",
                        format!("+{:.0}%", army.attack_bonus_pct),
                        palette::GOLD,
                    ),
                    (
                        "防御",
                        format!("+{:.0}%", army.defense_bonus_pct),
                        palette::GOLD,
                    ),
                ],
            );
            draw_tab_strip(ui, layout.tabs, "面板 / 师徽 / 战备", accent);
            v9_army_detail_body(ui, layout.body, army, &divisions, &mut cmds);
        });
    if close {
        cmds.push(MilitaryCommand::ClearArmySelection);
    }
    cmds
}

fn v9_army_detail_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    army: &ArmyEntry,
    divisions: &[&DivisionEntry],
    cmds: &mut Vec<MilitaryCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::primitives::{Button, ButtonSize, ButtonVariant, Card, CounterIcon};
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(
            vec![Track::Fixed(118.0), Track::Fr(1.0)],
            vec![Track::Fr(1.0)],
        )
        .with_gutter(0.0, spacing::S5);
        let cells = grid.measure(rect);
        let summary_rect = GridLayout::cell(&cells, 0, 0);
        let list_rect = GridLayout::cell(&cells, 1, 0);

        let summary = Card::new().as_ornate().show_at(ui, summary_rect);
        ui.painter().text(
            summary.left_top(),
            Align2::LEFT_TOP,
            "集团军状态",
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        let status = if army.executing {
            ("执行中", palette::GOOD)
        } else if army.has_arrow {
            ("计划就绪", palette::GOLD_HOT)
        } else if army.has_path {
            ("已有前线", palette::GOLD)
        } else {
            ("待命", palette::MUTED)
        };
        ui.painter().text(
            Pos2::new(summary.left(), summary.top() + 30.0),
            Align2::LEFT_TOP,
            status.0,
            TextRole::Subheading.font_id(),
            status.1,
        );
        ui.painter().text(
            Pos2::new(summary.left(), summary.top() + 54.0),
            Align2::LEFT_TOP,
            format!(
                "计划 +{:.0}%  恢复 +{:.0}%  补给 -{:.0}%",
                army.planning_bonus_pct, army.org_recovery_bonus_pct, army.supply_reduction_pct
            ),
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        let clear_rect = Rect::from_min_size(
            Pos2::new(summary.right() - 134.0, summary.bottom() - 32.0),
            Vec2::new(132.0, 28.0),
        );
        if Button::new("取消选中")
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .show_at(ui, clear_rect)
            .clicked()
        {
            cmds.push(MilitaryCommand::ClearArmySelection);
        }

        let list_inner = Card::new().as_panel().show_at(ui, list_rect);
        ui.painter().text(
            list_inner.left_top(),
            Align2::LEFT_TOP,
            format!("师徽列表 ({})", divisions.len()),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        if divisions.is_empty() {
            crate::v9::composites::panel_shell::draw_empty_state(
                ui,
                Rect::from_min_max(
                    Pos2::new(list_inner.left(), list_inner.top() + 34.0),
                    list_inner.right_bottom(),
                ),
                "暂无师团",
                "把选中师团加入集团军后会显示在这里。",
            );
            return;
        }
        let row_h = 44.0;
        let top = list_inner.top() + 36.0;
        let max_rows = ((list_inner.bottom() - top) / (row_h + spacing::S2))
            .floor()
            .max(0.0) as usize;
        for (idx, div) in divisions.iter().take(max_rows).enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(list_inner.left(), top + idx as f32 * (row_h + spacing::S2)),
                Vec2::new(list_inner.width(), row_h),
            );
            crate::v9::paint::paint_bevel(
                ui.painter(),
                row,
                palette::SOOT_BLACK,
                palette::EDGE_DARK,
                2.0,
            );
            let archetype = nato_icon::archetype_from_template_name(&div.name);
            CounterIcon::new(tr(&div.name), archetype)
                .accent(if div.in_combat {
                    palette::BAD
                } else {
                    palette::BRASS_BRIGHT
                })
                .show_at(
                    ui,
                    Rect::from_min_size(
                        row.left_top() + Vec2::new(spacing::S3, spacing::S3),
                        Vec2::new(150.0, 28.0),
                    ),
                );

            let org = div_org_ratio(div);
            let bar_left = row.left() + 166.0;
            let bar_w = (row.width() - 252.0).max(96.0);
            ui.painter().text(
                Pos2::new(bar_left, row.top() + 7.0),
                Align2::LEFT_TOP,
                format!("组织 {:.0}%", org * 100.0),
                TextRole::Caption.font_id(),
                ratio_color(org),
            );
            crate::v9::primitives::draw_progress_bar(
                ui,
                Rect::from_min_size(Pos2::new(bar_left, row.top() + 26.0), Vec2::new(bar_w, 8.0)),
                org,
                ratio_color(org),
            );
            ui.painter().text(
                Pos2::new(row.right() - spacing::S3, row.center().y),
                Align2::RIGHT_CENTER,
                format!(
                    "装 {:.0}%  兵 {:.0}%",
                    div.equipment_ratio * 100.0,
                    div.strength * 100.0
                ),
                TextRole::Caption.font_id(),
                palette::PARCHMENT_DIM,
            );
        }
    });
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
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &MilitaryData) -> Vec<MilitaryCommand> {
        return v9_show_army_detail(ctx, data);

        let mut cmds = Vec::new();

        let Some(army_id) = data.selected_army_id else {
            return cmds;
        };
        let Some(army) = data.armies.iter().find(|a| a.id == army_id) else {
            return cmds;
        };

        let screen = ctx.screen_rect();
        let max_h = (screen.height() - 200.0).max(240.0);

        egui::Area::new(egui::Id::new("army_detail_panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(screen.left() + 12.0, screen.top() + 64.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(egui::Stroke::new(1.2, GOLD))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(300.0);
                        ui.set_max_width(300.0);
                        ui.set_max_height(max_h);

                        // 头部
                        header_section(ui, army, &mut cmds);
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // 师列表
                        let divisions: Vec<&DivisionEntry> = data
                            .divisions
                            .iter()
                            .filter(|d| d.army_id == Some(army.id))
                            .collect();
                        division_section(ui, &divisions, max_h - 200.0);

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // 底部摘要
                        footer_section(ui, army, divisions.len());
                    });
            });

        cmds
    }
}

fn header_section(ui: &mut egui::Ui, army: &ArmyEntry, cmds: &mut Vec<MilitaryCommand>) {
    ui.horizontal(|ui| {
        portrait::draw_general_portrait(ui, army.commander_name.as_deref(), PortraitStyle::Large);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&army.name)
                    .strong()
                    .size(15.0)
                    .color(GOLD_BRIGHT),
            );
            ui.label(
                RichText::new(army.commander_name.as_deref().unwrap_or("无将领"))
                    .size(12.0)
                    .color(GOLD),
            );
            let status = if army.executing {
                ("执行中", GOOD)
            } else if army.has_arrow {
                ("计划已就绪", GOLD_BRIGHT)
            } else if army.has_path {
                ("前线已绘", GOLD)
            } else {
                ("待命", MUTED)
            };
            ui.label(RichText::new(status.0).size(11.0).color(status.1));

            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("攻 +{:.0}%", army.attack_bonus_pct))
                        .small()
                        .color(GOLD),
                );
                ui.label(
                    RichText::new(format!("防 +{:.0}%", army.defense_bonus_pct))
                        .small()
                        .color(GOLD),
                );
                ui.label(
                    RichText::new(format!("计 +{:.0}%", army.planning_bonus_pct))
                        .small()
                        .color(GOLD),
                );
                ui.label(
                    RichText::new(format!("组 +{:.0}%", army.org_recovery_bonus_pct))
                        .small()
                        .color(GOLD),
                );
                ui.label(
                    RichText::new(format!("耗 -{:.0}%", army.supply_reduction_pct))
                        .small()
                        .color(GOLD),
                );
            });

            if ui
                .small_button(RichText::new("取消选中").color(MUTED))
                .clicked()
            {
                cmds.push(MilitaryCommand::ClearArmySelection);
            }
        });
    });
}

fn division_section(ui: &mut egui::Ui, divisions: &[&DivisionEntry], max_h: f32) {
    ui.label(
        RichText::new(format!("师列表 ({})", divisions.len()))
            .strong()
            .color(GOLD),
    );
    if divisions.is_empty() {
        ui.label(RichText::new("暂无师团").small().color(MUTED));
        return;
    }
    egui::ScrollArea::vertical()
        .max_height(max_h.max(120.0))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for div in divisions {
                division_row(ui, div);
            }
        });
}

fn division_row(ui: &mut egui::Ui, div: &DivisionEntry) {
    ui.horizontal(|ui| {
        // NATO 符号
        let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 18.0), egui::Sense::hover());
        let archetype = nato_icon::archetype_from_template_name(&div.name);
        nato_icon::draw_nato_symbol(ui.painter(), rect, archetype, COUNTRY_COLOR_DEFAULT);

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if div.in_combat {
                    ui.label(RichText::new("●").size(10.0).color(BAD));
                }
                ui.label(RichText::new(&div.name).size(11.0).color(GOLD_BRIGHT));
            });

            ui.horizontal(|ui| {
                let org_frac = if div.max_organisation > 0.0 {
                    (div.organisation / div.max_organisation).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let org_color = if org_frac < 0.5 {
                    BAD
                } else if org_frac < 0.8 {
                    WARN
                } else {
                    GOOD
                };
                ui.add(
                    egui::ProgressBar::new(org_frac)
                        .desired_width(110.0)
                        .text(format!("{:.0}", div.organisation))
                        .fill(org_color),
                );
                let equip_color = if div.equipment_ratio < 0.8 {
                    WARN
                } else {
                    GOOD
                };
                ui.label(
                    RichText::new(format!("装{:.0}%", div.equipment_ratio * 100.0))
                        .size(10.0)
                        .color(equip_color),
                );
                let str_color = if div.strength < 0.8 { BAD } else { GOOD };
                ui.label(
                    RichText::new(format!("兵{:.0}%", div.strength * 100.0))
                        .size(10.0)
                        .color(str_color),
                );
            });
        });
    });
    ui.add_space(2.0);
}

fn footer_section(ui: &mut egui::Ui, army: &ArmyEntry, division_count: usize) {
    let (limit_text, color) = match army.command_limit {
        Some(limit) => {
            let over = division_count > limit as usize;
            let eff_pct = (army.command_efficiency * 100.0).round();
            (
                format!(
                    "{} 师 / 上限 {} / 效率 {:.0}%",
                    division_count, limit, eff_pct
                ),
                if over { BAD } else { MUTED },
            )
        }
        None => (format!("{} 师 / 无将领加成", division_count), MUTED),
    };
    ui.label(RichText::new(limit_text).small().color(color));
}
