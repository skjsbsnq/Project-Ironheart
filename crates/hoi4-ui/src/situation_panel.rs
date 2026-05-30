//! J.5b: 局势面板 (Situation Panel)。快捷键 J。
//!
//! 显示活跃局势的双方进度条 + 支持国 + 介入按钮。

#![allow(deprecated)]

use crate::{components, data_table, i18n::tr};
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};

const GOLD: Color32 = components::GOLD;
const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);

/// 局势面板中一个 side 的数据。
pub struct SituationSideEntry {
    pub name: String,
    pub progress: f32,
    pub color: Color32,
    pub supporters: Vec<String>,
}

/// 介入按钮数据。
pub struct InterventionEntry {
    pub id: String,
    pub name: String,
    pub cost_desc: String,
    pub expected_impact: String,
    pub available: bool,
    pub cooldown_days: u32,
    pub side_name: String,
}

/// 介入日志条目（用于显示历史介入）。
pub struct InterventionLogEntry {
    pub country_tag: String,
    pub intervention_name: String,
    pub side_name: String,
    pub progress_boost: f32,
    pub army_xp: f32,
    pub air_xp: f32,
}

/// 单个活跃局势的数据。
pub struct SituationEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub sides: Vec<SituationSideEntry>,
    pub interventions: Vec<InterventionEntry>,
    pub ended: bool,
    pub winner: Option<String>,
    pub intervention_log: Vec<InterventionLogEntry>,
    /// Phase 6: 军事概览（非 territorial 局势可为空）。
    pub military_overview: Option<MilitaryOverview>,
}

/// Phase 6: 军事概览 — 让玩家看到战争实情。
pub struct MilitaryOverview {
    /// 双方师数（与 sides 对齐）
    pub division_counts: Vec<u32>,
    /// 双方装备储备（infantry_equipment 数量；与 sides 对齐）
    pub equipment_stockpile: Vec<f32>,
    /// 双方参战国 tag 列表（与 sides 对齐）
    pub belligerent_tags: Vec<Vec<String>>,
    /// 战区州控制情况：每方控制多少州 / 战区总州数
    pub theater_control: Vec<u32>,
    pub theater_total: u32,
    /// 关键省份控制者：(name, controller_tag) 列表
    pub key_provinces: Vec<(String, String)>,
}

/// 面板数据。
pub struct SituationPanelData {
    pub situations: Vec<SituationEntry>,
}

/// 面板命令。
#[derive(Debug)]
pub enum SituationCommand {
    Intervene {
        situation_id: String,
        intervention_id: String,
    },
}

pub struct SituationPanel;

impl SituationPanel {
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &SituationPanelData) -> (bool, Vec<SituationCommand>) {
        return v9_show_situations(ctx, data);

        let mut close = false;
        let mut cmds = Vec::new();
        egui::SidePanel::left("situation_panel")
            .default_width(460.0)
            .min_width(380.0)
            .resizable(false)
            .show(ctx, |ui| {
                components::panel_header(ui, tr("situations"), &mut close);
                let active_count = data.situations.iter().filter(|sit| !sit.ended).count();
                let ended_count = data.situations.len().saturating_sub(active_count);
                let intervention_count = data
                    .situations
                    .iter()
                    .map(|sit| sit.interventions.len())
                    .sum::<usize>();
                components::summary_strip(
                    ui,
                    &[
                        ("活跃局势", active_count.to_string()),
                        ("已结束", ended_count.to_string()),
                        ("可干预项", intervention_count.to_string()),
                    ],
                );
                ui.add_space(4.0);
                render_status_banner(ui, data, active_count, intervention_count);
                ui.separator();

                if data.situations.is_empty() {
                    components::empty_state(
                        ui,
                        tr("no_active_situations"),
                        "国际局势平稳，暂无需要干预的事件。 ",
                    );
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for sit in &data.situations {
                        ui.add_space(6.0);
                        egui::Frame::new()
                            .fill(PANEL_CARD)
                            .stroke(egui::Stroke::new(1.0, STROKE_DARK))
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&sit.title).strong().color(GOLD));
                                    let stage = if sit.ended { "已结束" } else { "进行中" };
                                    ui.colored_label(
                                        Color32::LIGHT_GRAY,
                                        format!("阶段：{}", stage),
                                    );
                                });
                                ui.label(
                                    RichText::new(&sit.description)
                                        .small()
                                        .color(Color32::LIGHT_GRAY),
                                );

                                components::section(ui, "进度与参战方", |ui| {
                                    for side in &sit.sides {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&side.name).color(side.color));
                                            ui.add(
                                                egui::ProgressBar::new(
                                                    (side.progress / 100.0).clamp(0.0, 1.0),
                                                )
                                                .desired_width(190.0)
                                                .text(format!("{:.0}%", side.progress)),
                                            );
                                        });
                                        let supporters = if side.supporters.is_empty() {
                                            "暂无支持国".to_owned()
                                        } else {
                                            format!("支持国：{}", side.supporters.join("、"))
                                        };
                                        ui.label(
                                            RichText::new(supporters).small().color(Color32::GRAY),
                                        );
                                    }
                                    if let Some(ref winner) = sit.winner {
                                        ui.colored_label(
                                            Color32::GREEN,
                                            format!("胜利方：{}", winner),
                                        );
                                    }
                                });

                                if let Some(ov) = &sit.military_overview {
                                    components::section(ui, "战区与关键省份", |ui| {
                                        if ov.theater_total > 0 {
                                            egui::Grid::new(format!(
                                                "situation_theater_{}",
                                                sit.id
                                            ))
                                            .num_columns(4)
                                            .spacing([12.0, 4.0])
                                            .striped(true)
                                            .show(
                                                ui,
                                                |ui| {
                                                    data_table::header(ui, "阵营");
                                                    data_table::header(ui, "州控制");
                                                    data_table::header(ui, "师数");
                                                    data_table::header(ui, "步兵装备");
                                                    ui.end_row();
                                                    for (i, side) in sit.sides.iter().enumerate() {
                                                        let cnt = ov
                                                            .theater_control
                                                            .get(i)
                                                            .copied()
                                                            .unwrap_or(0);
                                                        let pct = 100.0 * cnt as f32
                                                            / ov.theater_total as f32;
                                                        let div = ov
                                                            .division_counts
                                                            .get(i)
                                                            .copied()
                                                            .unwrap_or(0);
                                                        let eq = ov
                                                            .equipment_stockpile
                                                            .get(i)
                                                            .copied()
                                                            .unwrap_or(0.0);
                                                        ui.colored_label(side.color, &side.name);
                                                        ui.label(format!(
                                                            "{}/{}（{:.0}%）",
                                                            cnt, ov.theater_total, pct
                                                        ));
                                                        ui.label(format!("{} 个师", div));
                                                        ui.label(format!("{:.0}", eq));
                                                        ui.end_row();
                                                    }
                                                },
                                            );
                                        }
                                        if !ov.key_provinces.is_empty() {
                                            let key_text = ov
                                                .key_provinces
                                                .iter()
                                                .map(|(name, ctrl)| format!("{}：{}", name, ctrl))
                                                .collect::<Vec<_>>()
                                                .join("；");
                                            ui.label(
                                                RichText::new(format!("关键省份：{}", key_text))
                                                    .small()
                                                    .color(Color32::LIGHT_YELLOW),
                                            );
                                        }
                                    });
                                }

                                if !sit.ended {
                                    components::section(ui, "干预行动", |ui| {
                                        if sit.interventions.is_empty() {
                                            components::empty_state(
                                                ui,
                                                "暂无可用干预行动",
                                                "该局势只能观察，或尚未开放玩家干预。",
                                            );
                                        }
                                        for interv in &sit.interventions {
                                            egui::Frame::new()
                                                .fill(PANEL_CARD_SOFT)
                                                .stroke(egui::Stroke::new(1.0, STROKE_DARK))
                                                .inner_margin(egui::Margin::symmetric(8, 6))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        let enabled = interv.available
                                                            && interv.cooldown_days == 0;
                                                        let btn = components::action_button(
                                                            ui,
                                                            enabled,
                                                            &interv.name,
                                                        );
                                                        if btn.clicked() {
                                                            cmds.push(
                                                                SituationCommand::Intervene {
                                                                    situation_id: sit.id.clone(),
                                                                    intervention_id: interv
                                                                        .id
                                                                        .clone(),
                                                                },
                                                            );
                                                        }
                                                        ui.label(format!(
                                                            "支援：{}",
                                                            interv.side_name
                                                        ));
                                                        if interv.cooldown_days > 0 {
                                                            ui.colored_label(
                                                                components::DANGER,
                                                                format!(
                                                                    "冷却：{} 天",
                                                                    interv.cooldown_days
                                                                ),
                                                            );
                                                        } else {
                                                            ui.colored_label(
                                                                components::SUCCESS,
                                                                "可执行",
                                                            );
                                                        }
                                                    });
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "代价：{}",
                                                            if interv.cost_desc.is_empty() {
                                                                "无直接代价"
                                                            } else {
                                                                &interv.cost_desc
                                                            }
                                                        ))
                                                        .small()
                                                        .color(Color32::LIGHT_GRAY),
                                                    );
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "预期影响：{}",
                                                            interv.expected_impact
                                                        ))
                                                        .small()
                                                        .color(Color32::LIGHT_YELLOW),
                                                    );
                                                });
                                        }
                                    });
                                }

                                if !sit.intervention_log.is_empty() {
                                    components::section(ui, "介入日志", |ui| {
                                        for log in sit.intervention_log.iter().rev().take(5) {
                                            let xp_str = if log.army_xp > 0.0 || log.air_xp > 0.0 {
                                                format!(
                                                    "，陆军经验 +{:.0}，空军经验 +{:.0}",
                                                    log.army_xp, log.air_xp
                                                )
                                            } else {
                                                String::new()
                                            };
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} 执行 {}，{} 进度 +{:.0}{}",
                                                    log.country_tag,
                                                    log.intervention_name,
                                                    log.side_name,
                                                    log.progress_boost,
                                                    xp_str
                                                ))
                                                .small()
                                                .color(Color32::LIGHT_YELLOW),
                                            );
                                        }
                                        if sit.intervention_log.len() > 5 {
                                            ui.colored_label(
                                                Color32::GRAY,
                                                format!(
                                                    "已隐藏较早的 {} 条记录",
                                                    sit.intervention_log.len() - 5
                                                ),
                                            );
                                        }
                                    });
                                }
                            });
                    }
                });
            });
        (close, cmds)
    }
}

fn v9_show_situations(
    ctx: &egui::Context,
    data: &SituationPanelData,
) -> (bool, Vec<SituationCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_empty_state, draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let active_count = data.situations.iter().filter(|sit| !sit.ended).count();
    let ended_count = data.situations.len().saturating_sub(active_count);
    let intervention_count = data
        .situations
        .iter()
        .map(|sit| sit.interventions.len())
        .sum::<usize>();
    let (close, output) = PanelShell::new("situation_panel_v9", tr("situations"))
        .class(PanelClass::MilitaryDiplomacy)
        .accent(if active_count > 0 {
            palette::WARN
        } else {
            palette::GOOD
        })
        .footer("Q Close  |  Situation cards")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("active"),
                        active_count.to_string(),
                        if active_count > 0 {
                            palette::WARN
                        } else {
                            palette::GOOD
                        },
                    ),
                    (tr("ended"), ended_count.to_string(), palette::MUTED),
                    (
                        tr("interventions"),
                        intervention_count.to_string(),
                        if intervention_count > 0 {
                            palette::GOLD
                        } else {
                            palette::MUTED
                        },
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "局势 / 阶段进度 / 介入",
                palette::BRASS_BRIGHT,
            );
            let mut cmds = Vec::new();
            if data.situations.is_empty() {
                draw_empty_state(
                    ui,
                    layout.body,
                    tr("no_active_situations"),
                    "当前没有活跃国际局势。",
                );
            } else {
                v9_situations_body(ui, layout.body, data, &mut cmds);
            }
            cmds
        });
    (close, output.unwrap_or_default())
}

fn v9_situations_body(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &SituationPanelData,
    cmds: &mut Vec<SituationCommand>,
) {
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.set_min_size(rect.size());
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for sit in &data.situations {
                    v9_situation_card(ui, sit, cmds);
                    ui.add_space(crate::v9::spacing::S4);
                }
            });
    });
}

fn v9_situation_card(ui: &mut egui::Ui, sit: &SituationEntry, cmds: &mut Vec<SituationCommand>) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{Button, ButtonSize, ButtonVariant, Card, ProgressRing},
        tokens::{palette, spacing, TextRole},
    };
    let side_rows = sit.sides.len().max(1);
    let intervention_rows = if sit.ended {
        0
    } else {
        sit.interventions.len().min(4)
    };
    let log_rows = sit.intervention_log.len().min(3);
    let height =
        178.0 + side_rows as f32 * 74.0 + intervention_rows as f32 * 44.0 + log_rows as f32 * 22.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height.max(260.0)),
        Sense::hover(),
    );
    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        Pos2::new(inner.left(), inner.top()),
        egui::Align2::LEFT_TOP,
        &sit.title,
        TextRole::Display.font_id(),
        palette::GOLD_HOT,
    );
    let stage = if sit.ended { tr("ended") } else { tr("active") };
    v9_situation_badge(
        ui,
        Rect::from_min_size(
            Pos2::new(inner.right() - 112.0, inner.top() + 2.0),
            Vec2::new(108.0, 24.0),
        ),
        stage,
        if sit.ended {
            palette::MUTED
        } else {
            palette::WARN
        },
    );
    let desc = truncate_situation_text(&sit.description, 180);
    let galley = ui.painter().layout(
        desc,
        TextRole::Body.font_id(),
        palette::PARCHMENT_DIM,
        inner.width() - 12.0,
    );
    ui.painter().galley(
        Pos2::new(inner.left(), inner.top() + 34.0),
        galley,
        palette::PARCHMENT_DIM,
    );
    let mut y = inner.top() + 86.0;

    let cols = vec![Track::Fr(1.0); sit.sides.len().max(1)];
    let ring_grid = GridLayout::new(vec![Track::Fixed(72.0)], cols).with_gutter(spacing::S5, 0.0);
    let ring_rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 72.0));
    let cells = ring_grid.measure(ring_rect);
    for (idx, side) in sit.sides.iter().enumerate() {
        let cell = GridLayout::cell(&cells, 0, idx);
        ProgressRing::new(side.progress / 100.0, &side.name)
            .accent(side.color)
            .show_at(ui, cell);
    }
    y += 84.0;
    if let Some(winner) = &sit.winner {
        v9_situation_line(ui, inner.left(), y, "胜方", winner, palette::GOOD);
        y += 24.0;
    }
    if let Some(overview) = &sit.military_overview {
        v9_military_overview(
            ui,
            Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 58.0)),
            sit,
            overview,
        );
        y += 68.0;
    }
    if !sit.ended {
        ui.painter().text(
            Pos2::new(inner.left(), y),
            egui::Align2::LEFT_TOP,
            tr("interventions"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        y += 28.0;
        for action in sit.interventions.iter().take(4) {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 36.0));
            let enabled = action.available && action.cooldown_days == 0;
            crate::v9::paint::paint_bevel(
                ui.painter(),
                row,
                palette::SOOT_BLACK,
                if enabled {
                    palette::BRASS_DARK
                } else {
                    palette::HAIRLINE
                },
                1.0,
            );
            ui.painter().text(
                Pos2::new(row.left() + spacing::S4, row.center().y),
                egui::Align2::LEFT_CENTER,
                &action.name,
                TextRole::Body.font_id(),
                if enabled {
                    palette::PARCHMENT
                } else {
                    palette::MUTED
                },
            );
            ui.painter().text(
                Pos2::new(row.left() + 210.0, row.center().y),
                egui::Align2::LEFT_CENTER,
                &action.expected_impact,
                TextRole::Caption.font_id(),
                palette::BRASS_BRIGHT,
            );
            let btn_rect = Rect::from_min_size(
                Pos2::new(row.right() - 92.0, row.top() + 5.0),
                Vec2::new(84.0, 26.0),
            );
            if Button::new(tr("intervene"))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Secondary)
                .enabled(enabled)
                .show_at(ui, btn_rect)
                .clicked()
            {
                cmds.push(SituationCommand::Intervene {
                    situation_id: sit.id.clone(),
                    intervention_id: action.id.clone(),
                });
            }
            y += 42.0;
        }
    }
    if !sit.intervention_log.is_empty() {
        ui.painter().text(
            Pos2::new(inner.left(), y + 4.0),
            egui::Align2::LEFT_TOP,
            tr("intervention_log"),
            TextRole::Heading.font_id(),
            palette::BRASS_BRIGHT,
        );
        y += 32.0;
        for log in sit.intervention_log.iter().rev().take(3) {
            let text = format!(
                "{}: {} -> {} +{:.0}",
                log.country_tag, log.intervention_name, log.side_name, log.progress_boost
            );
            v9_situation_line(ui, inner.left(), y, "日志", &text, palette::PARCHMENT_DIM);
            y += 22.0;
        }
    }
}

fn v9_military_overview(
    ui: &mut egui::Ui,
    rect: Rect,
    sit: &SituationEntry,
    ov: &MilitaryOverview,
) {
    crate::v9::paint::paint_recessed_panel(ui.painter(), rect, 1.0);
    let cols = sit.sides.len().max(1) as f32;
    let each = rect.width() / cols;
    for (idx, side) in sit.sides.iter().enumerate() {
        let x = rect.left() + idx as f32 * each + 8.0;
        let div = ov.division_counts.get(idx).copied().unwrap_or(0);
        let eq = ov.equipment_stockpile.get(idx).copied().unwrap_or(0.0);
        let control = ov.theater_control.get(idx).copied().unwrap_or(0);
        ui.painter().text(
            Pos2::new(x, rect.top() + 7.0),
            egui::Align2::LEFT_TOP,
            &side.name,
            crate::v9::TextRole::Caption.font_id(),
            side.color,
        );
        ui.painter().text(
            Pos2::new(x, rect.top() + 27.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{} div  |  {:.0} eq  |  {}/{} states",
                div, eq, control, ov.theater_total
            ),
            crate::v9::TextRole::Small.font_id(),
            crate::v9::palette::PARCHMENT_DIM,
        );
    }
}

fn v9_situation_badge(ui: &mut egui::Ui, rect: Rect, label: &str, color: Color32) {
    crate::v9::paint::paint_bevel(
        ui.painter(),
        rect,
        crate::v9::palette::SOOT_BLACK,
        color,
        1.0,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        crate::v9::TextRole::Caption.font_id(),
        color,
    );
}

fn v9_situation_line(ui: &mut egui::Ui, x: f32, y: f32, label: &str, value: &str, color: Color32) {
    ui.painter().text(
        Pos2::new(x, y),
        egui::Align2::LEFT_TOP,
        label,
        crate::v9::TextRole::Caption.font_id(),
        crate::v9::palette::MUTED,
    );
    ui.painter().text(
        Pos2::new(x + 92.0, y),
        egui::Align2::LEFT_TOP,
        value,
        crate::v9::TextRole::Body.font_id(),
        color,
    );
}

fn truncate_situation_text(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let cut = text
        .char_indices()
        .nth(max)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    format!("{}...", &text[..cut])
}

fn render_status_banner(
    ui: &mut egui::Ui,
    data: &SituationPanelData,
    active_count: usize,
    intervention_count: usize,
) {
    let (label, text, color) = if active_count == 0 {
        (
            "暂无活跃局势",
            "当前没有需要立即介入的局势，面板主要用于回看已结束事件。".to_owned(),
            Color32::from_rgb(0x70, 0xc8, 0x78),
        )
    } else if intervention_count == 0 {
        (
            "局势可观察",
            "有活跃局势，但暂时没有可执行的干预动作。".to_owned(),
            Color32::from_rgb(0xff, 0xc0, 0x60),
        )
    } else {
        (
            "局势紧张",
            format!("当前有 {} 个活跃局势，优先关注可干预项。", active_count),
            Color32::from_rgb(0xff, 0xc0, 0x60),
        )
    };

    let ended_count = data.situations.iter().filter(|sit| sit.ended).count();
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x1d, 0x16, 0x10, 230))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(label).strong().color(color));
                ui.label(
                    RichText::new(format!("{}  已结束 {}", text, ended_count))
                        .small()
                        .color(Color32::from_rgb(0xe0, 0xd2, 0xa8)),
                );
            });
        });
    ui.add_space(4.0);
}
