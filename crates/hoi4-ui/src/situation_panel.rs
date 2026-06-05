//! J.5b: 局势面板 (Situation Panel)。快捷键 J。
//!
//! 显示活跃局势的双方进度条 + 支持国 + 介入按钮。

#![allow(deprecated)]

use crate::i18n::tr;
use egui::{Color32, Pos2, Rect, Sense, Vec2};

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
    pub fn show(ctx: &egui::Context, data: &SituationPanelData) -> (bool, Vec<SituationCommand>) {
        v9_show_situations(ctx, data)
    }
}

fn v9_show_situations(
    ctx: &egui::Context,
    data: &SituationPanelData,
) -> (bool, Vec<SituationCommand>) {
    use crate::vanilla_iron::{JournalPanelShell, VanillaIron};

    let active_count = data.situations.iter().filter(|sit| !sit.ended).count();
    let ended_count = data.situations.len().saturating_sub(active_count);
    let intervention_count = data
        .situations
        .iter()
        .map(|sit| sit.interventions.len())
        .sum::<usize>();
    let (close, output) = JournalPanelShell::new("situation_panel_iron", tr("situations"))
        .subtitle("国际局势")
        .accent(if active_count > 0 {
            VanillaIron::WARN
        } else {
            VanillaIron::GOOD
        })
        .footer("Esc 返回")
        .show(ctx, |ui, layout| {
            let mut cmds = Vec::new();
            draw_situation_summary(
                ui,
                layout.nav.shrink2(Vec2::new(8.0, 8.0)),
                &[
                    (
                        tr("active"),
                        active_count.to_string(),
                        if active_count > 0 {
                            VanillaIron::WARN
                        } else {
                            VanillaIron::GOOD
                        },
                    ),
                    (tr("ended"), ended_count.to_string(), VanillaIron::MUTED),
                    (
                        tr("interventions"),
                        intervention_count.to_string(),
                        if intervention_count > 0 {
                            VanillaIron::BRASS_BRIGHT
                        } else {
                            VanillaIron::MUTED
                        },
                    ),
                ],
            );
            if data.situations.is_empty() {
                draw_iron_empty_state(
                    ui,
                    layout.main.shrink2(Vec2::new(10.0, 10.0)),
                    tr("no_active_situations"),
                    "当前没有活跃国际局势。",
                );
            } else {
                v9_situations_body(
                    ui,
                    layout.main.shrink2(Vec2::new(8.0, 8.0)),
                    data,
                    &mut cmds,
                );
            }
            draw_situation_side(ui, layout.side.shrink2(Vec2::new(8.0, 8.0)), data);
            cmds
        });
    (close, output.unwrap_or_default())
}

fn draw_situation_summary(ui: &mut egui::Ui, rect: Rect, items: &[(&str, String, Color32)]) {
    use crate::vanilla_iron::VanillaIron;
    let mut y = rect.top();
    ui.painter().text(
        Pos2::new(rect.left(), y),
        egui::Align2::LEFT_TOP,
        "概览",
        crate::v9::TextRole::Subheading.font_id(),
        VanillaIron::BRASS_BRIGHT,
    );
    y += 28.0;
    for (label, value, color) in items {
        let row = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), 48.0));
        VanillaIron::paint_region(ui.painter(), row, VanillaIron::CARD_DEEP);
        ui.painter().text(
            Pos2::new(row.left() + 8.0, row.top() + 7.0),
            egui::Align2::LEFT_TOP,
            *label,
            crate::v9::TextRole::Caption.font_id(),
            VanillaIron::MUTED,
        );
        ui.painter().text(
            Pos2::new(row.left() + 8.0, row.top() + 24.0),
            egui::Align2::LEFT_TOP,
            value,
            crate::v9::TextRole::Heading.font_id(),
            *color,
        );
        y += 56.0;
    }
}

fn draw_iron_empty_state(ui: &mut egui::Ui, rect: Rect, title: &str, body: &str) {
    use crate::vanilla_iron::VanillaIron;
    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD_DEEP);
    ui.painter().text(
        Pos2::new(rect.center().x, rect.center().y - 12.0),
        egui::Align2::CENTER_CENTER,
        title,
        crate::v9::TextRole::Heading.font_id(),
        VanillaIron::BRASS_BRIGHT,
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.center().y + 14.0),
        egui::Align2::CENTER_CENTER,
        body,
        crate::v9::TextRole::Body.font_id(),
        VanillaIron::MUTED,
    );
}

fn draw_situation_side(ui: &mut egui::Ui, rect: Rect, data: &SituationPanelData) {
    use crate::vanilla_iron::VanillaIron;
    ui.painter().text(
        rect.left_top(),
        egui::Align2::LEFT_TOP,
        "介入日志",
        crate::v9::TextRole::Subheading.font_id(),
        VanillaIron::BRASS_BRIGHT,
    );
    let mut y = rect.top() + 30.0;
    let mut shown = 0usize;
    for sit in &data.situations {
        for log in sit.intervention_log.iter().rev().take(2) {
            shown += 1;
            let row = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), 54.0));
            VanillaIron::paint_region(ui.painter(), row, VanillaIron::CARD_DEEP);
            ui.painter().text(
                Pos2::new(row.left() + 8.0, row.top() + 7.0),
                egui::Align2::LEFT_TOP,
                format!("{} - {}", log.country_tag, log.intervention_name),
                crate::v9::TextRole::Caption.font_id(),
                VanillaIron::TEXT,
            );
            ui.painter().text(
                Pos2::new(row.left() + 8.0, row.top() + 27.0),
                egui::Align2::LEFT_TOP,
                format!("{} +{:.0}", log.side_name, log.progress_boost),
                crate::v9::TextRole::Small.font_id(),
                VanillaIron::MUTED,
            );
            y += 62.0;
        }
    }
    if shown == 0 {
        ui.painter().text(
            Pos2::new(rect.left(), y),
            egui::Align2::LEFT_TOP,
            "暂无介入记录。",
            crate::v9::TextRole::Body.font_id(),
            VanillaIron::MUTED,
        );
    }
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
        tokens::{palette, spacing, TextRole},
    };
    use crate::vanilla_iron::VanillaIron;
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
    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD);
    let inner = rect.shrink2(Vec2::new(12.0, 10.0));
    ui.painter().text(
        Pos2::new(inner.left(), inner.top()),
        egui::Align2::LEFT_TOP,
        &sit.title,
        TextRole::Display.font_id(),
        VanillaIron::TEXT,
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
        draw_side_progress(ui, cell, side);
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
            VanillaIron::paint_region(
                ui.painter(),
                row,
                if enabled {
                    VanillaIron::CARD_DEEP
                } else {
                    VanillaIron::BLACK
                },
            );
            ui.painter().text(
                Pos2::new(row.left() + spacing::S4, row.center().y),
                egui::Align2::LEFT_CENTER,
                &action.name,
                TextRole::Body.font_id(),
                if enabled {
                    VanillaIron::TEXT
                } else {
                    VanillaIron::MUTED
                },
            );
            let impact_rect = Rect::from_min_max(
                Pos2::new(row.left() + 190.0, row.top()),
                Pos2::new(row.right() - 98.0, row.bottom()),
            );
            let impact_font = crate::v9::text::fit_font_to_width(
                &action.expected_impact,
                TextRole::Caption.font_id(),
                impact_rect.width().max(1.0),
                0.72,
            );
            ui.painter().with_clip_rect(impact_rect).text(
                impact_rect.left_center(),
                egui::Align2::LEFT_CENTER,
                &action.expected_impact,
                impact_font,
                VanillaIron::BRASS_BRIGHT,
            );
            let btn_rect = Rect::from_min_size(
                Pos2::new(row.right() - 92.0, row.top() + 5.0),
                Vec2::new(84.0, 26.0),
            );
            let button = ui.interact(
                btn_rect,
                ui.id().with(("situation_intervene", &sit.id, &action.id)),
                Sense::click(),
            );
            let clickable = enabled && button.clicked();
            VanillaIron::paint_region(
                ui.painter(),
                btn_rect,
                if enabled && button.hovered() {
                    VanillaIron::CARD_SOFT
                } else {
                    VanillaIron::CARD_DEEP
                },
            );
            ui.painter().text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                tr("intervene"),
                TextRole::Caption.font_id(),
                if enabled {
                    VanillaIron::BRASS_BRIGHT
                } else {
                    VanillaIron::MUTED
                },
            );
            if enabled && button.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if clickable {
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

fn draw_side_progress(ui: &mut egui::Ui, rect: Rect, side: &SituationSideEntry) {
    use crate::vanilla_iron::VanillaIron;

    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD_DEEP);
    let inner = rect.shrink2(Vec2::new(8.0, 8.0));
    let progress = (side.progress / 100.0).clamp(0.0, 1.0);
    let title_rect = Rect::from_min_max(
        inner.left_top(),
        Pos2::new(inner.right(), inner.top() + 18.0),
    );
    let title_font = crate::v9::text::fit_font_to_width(
        &side.name,
        crate::v9::TextRole::Caption.font_id(),
        title_rect.width(),
        0.72,
    );
    ui.painter().with_clip_rect(title_rect).text(
        title_rect.left_center(),
        egui::Align2::LEFT_CENTER,
        &side.name,
        title_font,
        side.color,
    );

    let bar = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 28.0),
        Pos2::new(inner.right(), inner.top() + 42.0),
    );
    ui.painter().rect_filled(bar, 1.0, VanillaIron::BLACK);
    ui.painter().rect_filled(
        Rect::from_min_max(
            bar.left_top(),
            Pos2::new(bar.left() + bar.width() * progress, bar.bottom()),
        ),
        1.0,
        side.color,
    );
    ui.painter().rect_stroke(
        bar,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, VanillaIron::EDGE_DARK),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().text(
        Pos2::new(inner.right(), inner.bottom() - 2.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.0}%", side.progress),
        crate::v9::TextRole::Caption.font_id(),
        VanillaIron::TEXT,
    );
}

fn v9_situation_badge(ui: &mut egui::Ui, rect: Rect, label: &str, color: Color32) {
    use crate::vanilla_iron::VanillaIron;
    VanillaIron::paint_region(ui.painter(), rect, VanillaIron::CARD_DEEP);
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
