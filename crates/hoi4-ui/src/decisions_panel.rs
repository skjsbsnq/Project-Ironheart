#![allow(deprecated)]

use crate::components;
use crate::i18n::tr;
use crate::politics::{DecisionCommand, DecisionEntry};
use crate::{
    vanilla_iron::{JournalPanelShell, VanillaIron},
    ActiveDetailPanel, JournalEntryDetailTarget, PanelCommand,
};
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};

#[derive(Debug, Clone)]
pub struct DecisionsData {
    pub country_tag: String,
    pub political_power: f32,
    pub mechanics: Vec<MechanicGauge>,
    pub decisions: Vec<DecisionEntry>,
    /// 当前玩家国家已设置的 country flag 集合（不含 "FLAG:" 前缀）。
    /// SPA 派系小游戏用它判断佛朗哥/莫拉/赫迪利亚等人物的当前阶段。
    pub country_flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MechanicGauge {
    pub label: String,
    pub value: f32,
    pub max: f32,
    pub detail: String,
}

pub struct DecisionsPanel;

// ─── 派系小游戏专用调色 ────────────────────────────────────────
const FRANCO_RED: Color32 = Color32::from_rgb(0xb8, 0x4a, 0x3a);
const FALANGE_BLUE: Color32 = Color32::from_rgb(0x4a, 0x82, 0xc8);
const TRADITION_CRIMSON: Color32 = Color32::from_rgb(0x9c, 0x4a, 0x66);
const FOREIGN_GRAY: Color32 = Color32::from_rgb(0x88, 0x80, 0x70);
const ROUTE_LOCKED: Color32 = Color32::from_rgb(0xd1, 0xdf, 0xdd);
const ROUTE_DIM: Color32 = Color32::from_rgb(0x4a, 0x3a, 0x28);
const RAIL_DARK: Color32 = Color32::from_rgb(0x10, 0x0a, 0x05);
const RAIL_INK: Color32 = Color32::from_rgb(0x1a, 0x12, 0x09);

impl DecisionsPanel {
    pub fn show(ctx: &egui::Context, data: &DecisionsData) -> (bool, Vec<DecisionCommand>) {
        journal_show_decisions(ctx, data)
    }
}

fn journal_show_decisions(
    ctx: &egui::Context,
    data: &DecisionsData,
) -> (bool, Vec<DecisionCommand>) {
    let filter_id = egui::Id::new("decisions_journal_filter");
    let mut filter = ctx
        .data_mut(|d| d.get_persisted::<String>(filter_id))
        .unwrap_or_else(|| "available".to_owned());
    let available = data
        .decisions
        .iter()
        .filter(|entry| entry.clickable)
        .count();
    let cooldown = data
        .decisions
        .iter()
        .filter(|entry| entry.cooldown_remaining.is_some())
        .count();
    let (close, output) = JournalPanelShell::new("decisions_journal_panel", tr("decisions"))
        .subtitle("决议 / 局势 / 事务日志")
        .footer("Q 关闭 | 点击条目打开详情")
        .accent(if available > 0 {
            VanillaIron::BRASS_BRIGHT
        } else {
            VanillaIron::MUTED
        })
        .show(ctx, |ui, layout| {
            let mut cmds = Vec::new();
            decisions_journal_nav(ui, layout.nav, data, &mut filter, available, cooldown);
            decisions_journal_main(ui, layout.main, data, &filter, &mut cmds);
            decisions_journal_side(ui, layout.side, data, &mut cmds);
            cmds
        });
    ctx.data_mut(|d| d.insert_persisted(filter_id, filter));
    (close, output.unwrap_or_default())
}

fn decisions_journal_nav(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &DecisionsData,
    filter: &mut String,
    available: usize,
    cooldown: usize,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "筛选");
        for (id, label, count) in [
            ("available", "可执行", available),
            ("cooldown", "冷却中", cooldown),
            (
                "expiring",
                "即将到期",
                data.decisions
                    .iter()
                    .filter(|entry| entry.mission_remaining.is_some_and(|days| days <= 30))
                    .count(),
            ),
            (
                "history",
                "历史",
                data.decisions
                    .iter()
                    .filter(|entry| entry.already_fired)
                    .count(),
            ),
            ("all", "全部", data.decisions.len()),
        ] {
            if ui
                .selectable_label(filter == id, format!("{label} ({count})"))
                .clicked()
            {
                *filter = id.to_owned();
            }
        }
        ui.separator();
        VanillaIron::info_row(ui, "国家", data.country_tag.clone());
        VanillaIron::info_row(ui, "政治力量", format!("{:.0} PP", data.political_power));
    });
}

fn decisions_journal_main(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &DecisionsData,
    filter: &str,
    cmds: &mut Vec<DecisionCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "事务列表");
        let rows: Vec<&DecisionEntry> = data
            .decisions
            .iter()
            .filter(|entry| decision_matches_filter(entry, filter))
            .collect();
        if rows.is_empty() {
            ui.label(
                RichText::new("没有符合筛选条件的决议或局势。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("decisions_journal_grid")
                    .striped(true)
                    .spacing(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        for entry in rows {
                            if ui
                                .selectable_label(
                                    false,
                                    RichText::new(&entry.name)
                                        .strong()
                                        .color(if entry.clickable {
                                            VanillaIron::TEXT
                                        } else {
                                            VanillaIron::MUTED
                                        }),
                                )
                                .on_hover_text("打开日志详情")
                                .clicked()
                            {
                                cmds.push(DecisionCommand::Panel(PanelCommand::OpenDetail(
                                    ActiveDetailPanel::JournalEntry(JournalEntryDetailTarget {
                                        entry_id: entry.id.clone(),
                                    }),
                                )));
                            }
                            ui.label(
                                RichText::new(decision_journal_status(entry))
                                    .small()
                                    .color(decision_journal_status_color(entry)),
                            );
                            ui.label(format!("{:.0} PP", entry.cost_political_power));
                            ui.label(
                                RichText::new(&entry.effect_preview)
                                    .small()
                                    .color(VanillaIron::MUTED),
                            );
                            if ui
                                .add_enabled(entry.clickable, egui::Button::new("执行"))
                                .clicked()
                            {
                                cmds.push(DecisionCommand::Activate(entry.id.clone()));
                            }
                            ui.end_row();
                        }
                    });
            });
    });
}

fn decisions_journal_side(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &DecisionsData,
    cmds: &mut Vec<DecisionCommand>,
) {
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(8.0, 7.0)), |ui| {
        VanillaIron::section_heading(ui, "局势指标");
        if data.mechanics.is_empty() {
            ui.label(
                RichText::new("暂无局势指标。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
        } else {
            for gauge in &data.mechanics {
                VanillaIron::info_row(
                    ui,
                    &gauge.label,
                    format!("{:.0}/{:.0}", gauge.value, gauge.max),
                );
                if !gauge.detail.is_empty() {
                    ui.label(
                        RichText::new(&gauge.detail)
                            .small()
                            .color(VanillaIron::MUTED),
                    );
                }
            }
        }
        ui.separator();
        VanillaIron::section_heading(ui, "下一步");
        if let Some(entry) = data.decisions.iter().find(|entry| entry.clickable) {
            if ui.link(&entry.name).clicked() {
                cmds.push(DecisionCommand::Panel(PanelCommand::OpenDetail(
                    ActiveDetailPanel::JournalEntry(JournalEntryDetailTarget {
                        entry_id: entry.id.clone(),
                    }),
                )));
            }
        } else {
            ui.label(
                RichText::new("当前没有可执行决议。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
        }
    });
}

fn decision_matches_filter(entry: &DecisionEntry, filter: &str) -> bool {
    match filter {
        "available" => entry.clickable,
        "cooldown" => entry.cooldown_remaining.is_some(),
        "expiring" => entry.mission_remaining.is_some_and(|days| days <= 30),
        "history" => entry.already_fired,
        _ => true,
    }
}

fn decision_journal_status(entry: &DecisionEntry) -> String {
    if entry.already_fired {
        "历史".to_owned()
    } else if let Some(days) = entry.mission_remaining {
        format!("进行中 {days} 天")
    } else if let Some(days) = entry.cooldown_remaining {
        format!("冷却 {days} 天")
    } else if entry.clickable {
        "可执行".to_owned()
    } else {
        "不可执行".to_owned()
    }
}

fn decision_journal_status_color(entry: &DecisionEntry) -> Color32 {
    if entry.clickable {
        VanillaIron::GOOD
    } else if entry.cooldown_remaining.is_some() || entry.mission_remaining.is_some() {
        VanillaIron::WARN
    } else {
        VanillaIron::MUTED
    }
}

fn v9_show_decisions(ctx: &egui::Context, data: &DecisionsData) -> (bool, Vec<DecisionCommand>) {
    use crate::v9::{
        composites::side_rail::{SIDE_RAIL_PANEL_LEFT, SIDE_RAIL_TOP_OFFSET},
        paint,
        tokens::TextRole,
    };

    let accent = decision_gold();
    let screen = ctx.screen_rect();
    let left_gap = if screen.width() >= 980.0 {
        SIDE_RAIL_PANEL_LEFT
    } else {
        8.0
    };
    let top_gap = if screen.height() >= 680.0 {
        SIDE_RAIL_TOP_OFFSET
    } else {
        72.0
    };
    let panel_w = 820.0_f32.min((screen.width() - left_gap - 8.0).max(420.0));
    let panel_h = (screen.height() - top_gap - 8.0).max(360.0);
    let panel_pos = Pos2::new(screen.left() + left_gap, screen.top() + top_gap);
    let panel_size = Vec2::new(panel_w, panel_h);

    let mut close = false;
    let mut output = Vec::new();
    egui::Area::new(egui::Id::new("decisions_panel_vanilla_1936"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel_pos)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(panel_size, Sense::click_and_drag());
            paint::paint_shadow(ui.painter(), outer, crate::v9::Elevation::E2, 1.0);
            paint_decisions_shell(ui, outer, accent);

            let inner = outer.shrink2(Vec2::new(14.0, 12.0));
            ui.painter().text(
                Pos2::new(inner.left() + 2.0, inner.top() + 5.0),
                egui::Align2::LEFT_TOP,
                tr("decisions"),
                TextRole::Display.font_id(),
                decision_text(),
            );

            let close_rect = Rect::from_min_size(
                Pos2::new(inner.right() - 28.0, inner.top() - 2.0),
                Vec2::splat(23.0),
            );
            if decision_close_button(ui, close_rect)
                .on_hover_text(tr("panel_close_hint"))
                .clicked()
            {
                close = true;
            }

            let body = Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 52.0),
                Pos2::new(inner.right(), inner.bottom() - 8.0),
            );

            let mut cmds = Vec::new();
            v9_decisions_body(ui, body, data, &mut cmds);
            output = cmds;
        });

    (close, output)
}

fn v9_decisions_body(
    ui: &mut egui::Ui,
    rect: Rect,
    data: &DecisionsData,
    cmds: &mut Vec<DecisionCommand>,
) {
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.set_min_size(rect.size());
        egui::ScrollArea::vertical()
            .id_salt("decisions_vanilla_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width().max(360.0);
                if width >= 760.0 {
                    let gap = 10.0;
                    let right_w = 330.0_f32.min((width - gap) * 0.42);
                    let left_w = (width - gap - right_w).max(380.0);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui.allocate_ui_with_layout(
                            Vec2::new(left_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(left_w);
                                v9_decision_grid(ui, data, cmds);
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(right_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(right_w);
                                v9_impact_sidebar(ui, data);
                            },
                        );
                    });
                } else {
                    v9_impact_sidebar(ui, data);
                    v9_decision_grid(ui, data, cmds);
                }
            });
    });
}

fn v9_decision_grid(ui: &mut egui::Ui, data: &DecisionsData, cmds: &mut Vec<DecisionCommand>) {
    let mut any_visible = false;
    for category in decision_category_order() {
        let entries: Vec<&DecisionEntry> = data
            .decisions
            .iter()
            .filter(|entry| entry.visible && entry.category == category)
            .collect();
        if entries.is_empty() {
            continue;
        }
        any_visible = true;
        vanilla_decision_category_card(ui, category_label_zh(category), &entries, cmds);
    }
    if !any_visible {
        vanilla_empty_card(
            ui,
            "暂无决议",
            "当前国家没有可见决议。推进战局、积累政治力量或等待事件以解锁新的决议。",
        );
    }
}

fn vanilla_decision_category_card(
    ui: &mut egui::Ui,
    title: &str,
    entries: &[&DecisionEntry],
    cmds: &mut Vec<DecisionCommand>,
) {
    let row_h = 86.0;
    let height = 42.0 + entries.len() as f32 * (row_h + 6.0) + 5.0;
    decision_panel_card(ui, height, |ui, inner| {
        decision_section_title(ui, inner, title);
        let mut y = inner.top() + 36.0;
        for entry in entries {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), row_h));
            vanilla_decision_row(ui, row, entry, cmds);
            y += row_h + 6.0;
        }
    });
}

fn vanilla_decision_row(
    ui: &mut egui::Ui,
    rect: Rect,
    entry: &DecisionEntry,
    cmds: &mut Vec<DecisionCommand>,
) {
    decision_row_frame(ui, rect);
    let accent = v9_decision_accent(entry);
    ui.painter().rect_filled(
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.left() + 3.0, rect.bottom())),
        0.0,
        accent,
    );

    let icon = Rect::from_min_size(rect.left_top() + Vec2::new(10.0, 9.0), Vec2::splat(28.0));
    decision_slot(ui, icon, Color32::from_rgb(0x4d, 0x45, 0x35));
    draw_decision_svg_icon(ui.painter(), icon.shrink(6.0), decision_muted());

    let action_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 82.0, rect.top() + 9.0),
        Vec2::new(72.0, 25.0),
    );
    let (state, state_color) = v9_decision_state(entry);
    if decision_state_button(
        ui,
        action_rect,
        &entry.id,
        state,
        state_color,
        entry.clickable,
    ) {
        cmds.push(DecisionCommand::Activate(entry.id.clone()));
    }

    let text_left = icon.right() + 12.0;
    let text_right = action_rect.left() - 10.0;
    let text_rect = Rect::from_min_max(
        Pos2::new(text_left, rect.top() + 7.0),
        Pos2::new(text_right, rect.bottom() - 8.0),
    );
    let clipped = ui.painter().with_clip_rect(text_rect);
    clipped.text(
        text_rect.left_top(),
        egui::Align2::LEFT_TOP,
        entry.name.as_str(),
        fit_decision_font(
            entry.name.as_str(),
            crate::v9::TextRole::Body.font_id(),
            text_rect.width(),
        ),
        decision_gold_hot(),
    );

    if !entry.description.is_empty() {
        let desc = truncate_chars(&entry.description, 132);
        let galley = clipped.layout(
            desc,
            crate::v9::TextRole::Caption.font_id(),
            decision_text_dim(),
            text_rect.width(),
        );
        clipped.galley(
            Pos2::new(text_rect.left(), text_rect.top() + 20.0),
            galley,
            decision_text_dim(),
        );
    }

    let meta = decision_meta_line(entry);
    clipped.text(
        Pos2::new(text_rect.left(), rect.bottom() - 17.0),
        egui::Align2::LEFT_CENTER,
        meta.as_str(),
        crate::v9::TextRole::Caption.font_id(),
        decision_muted(),
    );

    if let Some(remaining) = entry.mission_remaining {
        let total = entry.mission_total.unwrap_or(remaining).max(1);
        let done = total.saturating_sub(remaining);
        let progress = done as f32 / total as f32;
        let bar = Rect::from_min_size(
            Pos2::new(text_left, rect.bottom() - 7.0),
            Vec2::new((text_right - text_left).max(24.0), 4.0),
        );
        decision_progress_bar(ui, bar, progress, state_color);
    }

    let response = ui.interact(
        rect,
        ui.id().with(("decision_vanilla_row", &entry.id)),
        Sense::hover(),
    );
    response.on_hover_text(decision_hover_text(entry));
}

fn v9_impact_sidebar(ui: &mut egui::Ui, data: &DecisionsData) {
    vanilla_decision_overview_card(ui, data);
    vanilla_decision_preview_card(ui, data);
    if !data.mechanics.is_empty() {
        vanilla_mechanics_card(ui, data);
    }
}

fn vanilla_decision_overview_card(ui: &mut egui::Ui, data: &DecisionsData) {
    let visible = data.decisions.iter().filter(|entry| entry.visible).count();
    let available = data
        .decisions
        .iter()
        .filter(|entry| entry.visible && entry.clickable)
        .count();
    decision_panel_card(ui, 132.0, |ui, inner| {
        decision_section_title(ui, inner, "概览");
        let mut y = inner.top() + 38.0;
        decision_info_row(
            ui,
            Pos2::new(inner.left() + 4.0, y),
            "国家",
            if data.country_tag.is_empty() {
                tr("country")
            } else {
                data.country_tag.as_str()
            },
            decision_text(),
        );
        y += 24.0;
        decision_info_row(
            ui,
            Pos2::new(inner.left() + 4.0, y),
            tr("political_power"),
            &format!("{:.0}", data.political_power),
            decision_gold_hot(),
        );
        y += 24.0;
        decision_info_row(
            ui,
            Pos2::new(inner.left() + 4.0, y),
            "可用 / 可见",
            &format!("{} / {}", available, visible),
            if available > 0 {
                decision_good()
            } else {
                decision_muted()
            },
        );
    });
}

fn vanilla_decision_preview_card(ui: &mut egui::Ui, data: &DecisionsData) {
    let selected = data
        .decisions
        .iter()
        .find(|d| d.visible && d.clickable)
        .or_else(|| data.decisions.iter().find(|d| d.visible));

    decision_panel_card(ui, 236.0, |ui, inner| {
        decision_section_title(ui, inner, "影响预览");
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 36.0),
            inner.right_bottom(),
        );
        if let Some(entry) = selected {
            let title_rect = Rect::from_min_max(
                content.left_top(),
                Pos2::new(content.right(), content.top() + 26.0),
            );
            let clipped = ui.painter().with_clip_rect(title_rect);
            clipped.text(
                title_rect.left_top(),
                egui::Align2::LEFT_TOP,
                entry.name.as_str(),
                fit_decision_font(
                    entry.name.as_str(),
                    crate::v9::TextRole::Body.font_id(),
                    title_rect.width(),
                ),
                decision_gold_hot(),
            );

            let effect = if entry.effect_preview.is_empty() {
                if entry.description.is_empty() {
                    "暂无即时效果预览。".to_owned()
                } else {
                    entry.description.clone()
                }
            } else {
                entry.effect_preview.clone()
            };
            let effect_rect = Rect::from_min_max(
                Pos2::new(content.left(), content.top() + 30.0),
                Pos2::new(content.right(), content.bottom() - 54.0),
            );
            let effect_painter = ui.painter().with_clip_rect(effect_rect);
            let galley = effect_painter.layout(
                effect,
                crate::v9::TextRole::Caption.font_id(),
                decision_text(),
                effect_rect.width(),
            );
            effect_painter.galley(effect_rect.left_top(), galley, decision_text());

            let (state, color) = v9_decision_state(entry);
            let status_rect = Rect::from_min_size(
                Pos2::new(content.left(), content.bottom() - 40.0),
                Vec2::new(content.width(), 30.0),
            );
            decision_sidebar_row(ui, status_rect, "状态", state, color);
            let cost_rect = status_rect.translate(Vec2::new(0.0, 24.0));
            decision_sidebar_row(
                ui,
                cost_rect,
                "消耗",
                &format!("PP {:.0}", entry.cost_political_power),
                decision_gold(),
            );
        } else {
            decision_empty_text(ui, content, "当前没有可见决议");
        }
    });
}

fn vanilla_mechanics_card(ui: &mut egui::Ui, data: &DecisionsData) {
    let rows = data.mechanics.len().min(8);
    let height = 46.0 + rows as f32 * 39.0;
    decision_panel_card(ui, height.max(126.0), |ui, inner| {
        decision_section_title(ui, inner, "机制");
        let mut y = inner.top() + 36.0;
        for gauge in data.mechanics.iter().take(8) {
            let row =
                Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 32.0));
            decision_sidebar_row(
                ui,
                Rect::from_min_size(row.left_top(), Vec2::new(row.width(), 17.0)),
                gauge.label.as_str(),
                &format!("{:.1}/{:.0}", gauge.value, gauge.max),
                decision_text(),
            );
            let value = if gauge.max <= 0.0 {
                0.0
            } else {
                (gauge.value / gauge.max).clamp(0.0, 1.0)
            };
            let bar = Rect::from_min_size(
                Pos2::new(row.left(), row.top() + 20.0),
                Vec2::new(row.width(), 7.0),
            );
            decision_progress_bar(
                ui,
                bar,
                value,
                if value >= 0.70 {
                    decision_warn()
                } else {
                    decision_gold()
                },
            );
            y += 39.0;
        }
    });
}

fn decision_sidebar_row(ui: &mut egui::Ui, rect: Rect, label: &str, value: &str, color: Color32) {
    ui.painter().text(
        Pos2::new(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        crate::v9::TextRole::Caption.font_id(),
        decision_muted(),
    );
    ui.painter().text(
        Pos2::new(rect.right(), rect.center().y),
        egui::Align2::RIGHT_CENTER,
        value,
        crate::v9::TextRole::Numeric.font_id(),
        color,
    );
}

fn decision_category_order() -> [hoi4_content::DecisionCategory; 5] {
    [
        hoi4_content::DecisionCategory::Crisis,
        hoi4_content::DecisionCategory::Internal,
        hoi4_content::DecisionCategory::Military,
        hoi4_content::DecisionCategory::Diplomacy,
        hoi4_content::DecisionCategory::Industry,
    ]
}

fn paint_decisions_shell(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, decision_black());
    crate::v9::paint::paint_vertical_gradient_mesh(
        painter,
        rect.shrink(2.0),
        Color32::from_rgba_premultiplied(0x18, 0x18, 0x15, 244),
        Color32::from_rgba_premultiplied(0x06, 0x07, 0x06, 252),
    );
    crate::v9::paint::paint_plate_grain(painter, rect.shrink(4.0), 3.0, 3);
    paint_decision_border(painter, rect);
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.top() + 3.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(18)),
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.bottom() - 4.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(235)),
    );
    for corner in [
        rect.left_top() + Vec2::new(14.0, 14.0),
        rect.right_top() + Vec2::new(-14.0, 14.0),
    ] {
        painter.circle_filled(corner, 2.2, Color32::from_black_alpha(210));
        painter.circle_stroke(
            corner,
            2.2,
            egui::Stroke::new(
                1.0,
                Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 90),
            ),
        );
    }
}

fn paint_decision_border(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect.translate(Vec2::new(1.0, 1.0)),
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
        egui::epaint::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, decision_edge()),
        egui::epaint::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(220)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn decision_close_button(ui: &mut egui::Ui, rect: Rect) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.id().with("decisions_vanilla_close"),
        Sense::click(),
    );
    let fill = if response.hovered() {
        Color32::from_rgb(0x27, 0x28, 0x23)
    } else {
        Color32::from_rgb(0x12, 0x13, 0x10)
    };
    ui.painter().rect_filled(rect, 1.0, fill);
    paint_decision_border(ui.painter(), rect);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "X",
        crate::v9::TextRole::Caption.font_id(),
        decision_muted(),
    );
    response
}

fn decision_panel_card(
    ui: &mut egui::Ui,
    height: f32,
    add_contents: impl FnOnce(&mut egui::Ui, Rect),
) {
    let width = ui.available_width().max(300.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_decision_card_frame(ui, rect);
    let inner = rect.shrink2(Vec2::new(9.0, 7.0));
    add_contents(ui, inner);
    ui.add_space(6.0);
}

fn paint_decision_card_frame(ui: &mut egui::Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x08, 0x09, 0x08));
    crate::v9::paint::paint_vertical_gradient_mesh(
        painter,
        rect.shrink(2.0),
        Color32::from_rgba_premultiplied(0x19, 0x1b, 0x17, 238),
        Color32::from_rgba_premultiplied(0x03, 0x04, 0x03, 252),
    );
    crate::v9::paint::paint_horizontal_gradient_mesh(
        painter,
        rect.shrink(2.0),
        Color32::from_black_alpha(120),
        Color32::from_white_alpha(3),
    );
    crate::v9::paint::paint_plate_grain(painter, rect.shrink(4.0), 3.0, 2);
    paint_decision_border(painter, rect);
    painter.rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(190)),
        egui::epaint::StrokeKind::Inside,
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.top() + 3.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(12)),
    );
    painter.hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.bottom() - 3.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(230)),
    );
}

fn decision_section_title(ui: &mut egui::Ui, inner: Rect, title: &str) {
    let title_rect = Rect::from_min_size(inner.left_top(), Vec2::new(inner.width(), 26.0));
    ui.painter()
        .rect_filled(title_rect, 0.0, Color32::from_rgb(0x10, 0x11, 0x0f));
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        title_rect,
        Color32::from_rgba_premultiplied(0x44, 0x43, 0x38, 95),
        Color32::from_black_alpha(205),
    );
    ui.painter().rect_stroke(
        title_rect,
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(210)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().hline(
        (title_rect.left() + 4.0)..=(title_rect.right() - 4.0),
        title_rect.top() + 1.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(20)),
    );
    ui.painter().hline(
        (title_rect.left() + 4.0)..=(title_rect.right() - 4.0),
        title_rect.bottom() - 1.0,
        egui::Stroke::new(1.0, Color32::from_rgb(0x6b, 0x55, 0x32)),
    );
    ui.painter().text(
        Pos2::new(title_rect.left() + 6.0, title_rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        crate::v9::TextRole::Subheading.font_id(),
        decision_gold(),
    );
}

fn decision_row_frame(ui: &mut egui::Ui, rect: Rect) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x12, 0x14, 0x12));
    crate::v9::paint::paint_vertical_gradient_mesh(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_rgba_premultiplied(0x25, 0x27, 0x22, 150),
        Color32::from_rgba_premultiplied(0x08, 0x09, 0x08, 235),
    );
    paint_decision_border(ui.painter(), rect);
    ui.painter().hline(
        (rect.left() + 4.0)..=(rect.right() - 4.0),
        rect.top() + 1.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(6)),
    );
}

fn decision_slot(ui: &mut egui::Ui, rect: Rect, accent: Color32) {
    ui.painter()
        .rect_filled(rect, 1.0, Color32::from_rgb(0x0d, 0x10, 0x10));
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, accent),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(3.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, Color32::from_black_alpha(210)),
        egui::epaint::StrokeKind::Inside,
    );
}

fn decision_state_button(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &str,
    label: &str,
    color: Color32,
    enabled: bool,
) -> bool {
    let response = ui.interact(
        rect,
        ui.id().with(("decision_state_button", id)),
        Sense::click(),
    );
    let active = enabled && response.hovered();
    ui.painter().rect_filled(
        rect,
        1.0,
        if active {
            Color32::from_rgb(0x31, 0x32, 0x2a)
        } else {
            Color32::from_rgb(0x0a, 0x0b, 0x09)
        },
    );
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, Color32::from_black_alpha(220)),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        egui::epaint::CornerRadius::same(0),
        egui::Stroke::new(1.0, if enabled { color } else { decision_edge() }),
        egui::epaint::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        fit_decision_font(
            label,
            crate::v9::TextRole::Caption.font_id(),
            rect.width() - 6.0,
        ),
        if enabled { color } else { decision_muted() },
    );
    if response.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    enabled && response.clicked()
}

fn decision_progress_bar(ui: &mut egui::Ui, rect: Rect, value: f32, color: Color32) {
    ui.painter().rect_filled(rect, 1.0, RAIL_INK);
    let fill = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(
            rect.left() + rect.width() * value.clamp(0.0, 1.0),
            rect.bottom(),
        ),
    );
    ui.painter().rect_filled(fill, 1.0, color);
    ui.painter().rect_stroke(
        rect,
        egui::epaint::CornerRadius::same(1),
        egui::Stroke::new(1.0, decision_edge()),
        egui::epaint::StrokeKind::Inside,
    );
}

fn decision_info_row(ui: &mut egui::Ui, pos: Pos2, label: &str, value: &str, value_color: Color32) {
    ui.painter().text(
        pos,
        egui::Align2::LEFT_TOP,
        label,
        crate::v9::TextRole::Body.font_id(),
        decision_muted(),
    );
    ui.painter().text(
        pos + Vec2::new(92.0, 0.0),
        egui::Align2::LEFT_TOP,
        value,
        crate::v9::TextRole::Body.font_id(),
        value_color,
    );
}

fn vanilla_empty_card(ui: &mut egui::Ui, title: &str, body: &str) {
    decision_panel_card(ui, 190.0, |ui, inner| {
        decision_section_title(ui, inner, title);
        let content = Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 36.0),
            inner.right_bottom(),
        );
        decision_empty_text(ui, content, body);
    });
}

fn decision_empty_text(ui: &mut egui::Ui, rect: Rect, text: &str) {
    let galley = ui.painter().layout(
        text.to_owned(),
        crate::v9::TextRole::Body.font_id(),
        decision_muted(),
        rect.width(),
    );
    ui.painter().galley(
        Pos2::new(rect.left(), rect.center().y - galley.size().y * 0.5),
        galley,
        decision_muted(),
    );
}

fn draw_decision_svg_icon(painter: &egui::Painter, rect: Rect, color: Color32) {
    let stroke = egui::Stroke::new(1.55, color);
    let thin = egui::Stroke::new(1.15, color);
    let p = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            rect.left() + rect.width() * x / 24.0,
            rect.top() + rect.height() * y / 24.0,
        )
    };
    let rr =
        |x: f32, y: f32, w: f32, h: f32| -> Rect { Rect::from_min_max(p(x, y), p(x + w, y + h)) };
    painter.rect_stroke(
        rr(6.0, 4.0, 12.0, 16.5),
        egui::epaint::CornerRadius::same(1),
        stroke,
        egui::epaint::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rr(8.5, 2.8, 7.0, 4.0),
        egui::epaint::CornerRadius::same(1),
        thin,
        egui::epaint::StrokeKind::Inside,
    );
    for y in [9.0, 12.5, 16.0] {
        painter.line_segment([p(9.0, y), p(16.0, y)], thin);
    }
}

fn decision_meta_line(entry: &DecisionEntry) -> String {
    let mut parts = vec![entry.mechanic_kind.label().to_owned()];
    if entry.cost_political_power > 0.0 {
        parts.push(format!("PP {:.0}", entry.cost_political_power));
    }
    if let Some(remaining) = entry.cooldown_remaining {
        parts.push(format!("{} {}天", tr("cooldown"), remaining));
    }
    if let Some(remaining) = entry.mission_remaining {
        parts.push(format!("剩余 {}天", remaining));
    }
    if entry.already_fired {
        parts.push(tr("done").to_owned());
    }
    parts.join(" / ")
}

fn decision_hover_text(entry: &DecisionEntry) -> String {
    if entry.effect_preview.is_empty() {
        entry.description.clone()
    } else if entry.description.is_empty() {
        entry.effect_preview.clone()
    } else {
        format!("{}\n\n{}", entry.description, entry.effect_preview)
    }
}

fn fit_decision_font(text: &str, mut font: egui::FontId, max_width: f32) -> egui::FontId {
    let estimated = text.chars().count() as f32 * font.size * 0.56;
    if estimated > max_width && estimated > 1.0 {
        font.size *= (max_width / estimated).clamp(0.70, 1.0);
    }
    font
}

fn decision_black() -> Color32 {
    Color32::from_rgb(0x08, 0x0a, 0x09)
}

fn decision_edge() -> Color32 {
    Color32::from_rgb(0x55, 0x48, 0x31)
}

fn decision_text() -> Color32 {
    Color32::from_rgb(0xd6, 0xca, 0x9b)
}

fn decision_text_dim() -> Color32 {
    Color32::from_rgb(0xaa, 0xa2, 0x82)
}

fn decision_muted() -> Color32 {
    Color32::from_rgb(0x8d, 0x8b, 0x80)
}

fn decision_gold() -> Color32 {
    Color32::from_rgb(0xc9, 0xb4, 0x72)
}

fn decision_gold_hot() -> Color32 {
    Color32::from_rgb(0xe1, 0xd3, 0xa5)
}

fn decision_good() -> Color32 {
    Color32::from_rgb(0x70, 0xc8, 0x78)
}

fn decision_warn() -> Color32 {
    Color32::from_rgb(0xff, 0xc0, 0x60)
}

fn v9_decision_state(entry: &DecisionEntry) -> (&'static str, Color32) {
    if entry.mission_remaining.is_some() {
        (tr("active"), decision_warn())
    } else if entry.cooldown_remaining.is_some() {
        (tr("cooldown"), decision_muted())
    } else if entry.already_fired {
        (tr("done"), decision_good())
    } else if entry.clickable {
        (tr("execute"), decision_gold_hot())
    } else {
        (tr("locked"), decision_muted())
    }
}

fn v9_decision_accent(entry: &DecisionEntry) -> Color32 {
    if entry.clickable {
        decision_gold()
    } else if entry.mission_remaining.is_some() {
        decision_warn()
    } else if entry.already_fired {
        decision_good()
    } else {
        decision_muted()
    }
}

fn truncate_chars(text: &str, max: usize) -> String {
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

fn top_meta_strip(ui: &mut egui::Ui, tag: &str, pp: f32) {
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(components::PANEL_CARD_DEEP)
            .stroke(egui::Stroke::new(1.0, components::BRONZE))
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.label(RichText::new("国家").size(10.0).color(components::MUTED));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(tag)
                        .strong()
                        .size(13.0)
                        .color(components::GOLD_BRIGHT),
                );
            });
        egui::Frame::new()
            .fill(components::PANEL_CARD_DEEP)
            .stroke(egui::Stroke::new(1.0, components::BRONZE))
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(tr("political_power"))
                        .size(10.0)
                        .color(components::MUTED),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{:.0}", pp))
                        .strong()
                        .size(13.0)
                        .color(components::GOLD_BRIGHT),
                );
            });
    });
}

// ─────────────────────────────────────────────────────────────
// 派系小游戏 — KR/TNO 式：路线指南针 + 具名领袖任职阶段 + 决议直接挂在派系卡上
// ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    Franco,
    Falange,
    Syndicalist,
}

/// 派系视图：从机制条 + 国家 flag 集合派生出的当前权力布局。
struct FactionView<'a> {
    // gauges
    franco: f32,
    falange: f32,
    army: f32,
    church: f32,
    carlist: f32,
    foreign: f32,
    resistance: f32,
    // flags (借引用，避免 clone)
    flags: &'a std::collections::HashSet<String>,
}

impl<'a> FactionView<'a> {
    fn from(mechanics: &[MechanicGauge], flags: &'a std::collections::HashSet<String>) -> Self {
        let get = |label: &str| {
            mechanics
                .iter()
                .find(|g| g.label == label)
                .map(|g| g.value)
                .unwrap_or(0.0)
        };
        Self {
            franco: get("佛朗哥权威"),
            falange: get("长枪党/蓝衫"),
            army: get("军队忠诚"),
            church: get("教会影响"),
            carlist: get("卡洛斯派怨恨"),
            foreign: get("外援依赖"),
            resistance: get("占领区抵抗"),
            flags,
        }
    }

    fn has(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    /// 指南针二维坐标，X、Y 都标准化到 [-1, +1]。
    /// X 轴：左 = 委员会制度，右 = 个人独裁。  (佛朗哥权威 - 军政府合法性的代理)
    /// Y 轴：下 = 党化革命，  上 = 老式军政府。 (长枪党权力相对值)
    fn compass(&self) -> (f32, f32) {
        // junta_legitimacy proxy = army忠诚 + carlist + church(轻权重)
        let junta_proxy = self.army * 0.5 + self.church * 0.2;
        let x = ((self.franco - junta_proxy) / 12.0).clamp(-1.0, 1.0);
        // y: 长枪党/蓝衫越高，向下越深（党化革命）
        let y = (1.0 - (self.falange / 12.0)).clamp(-1.0, 1.0);
        (x, y)
    }

    /// 三个胜利路线在指南针上的位置（不带 P9/P10/P11 这种内部代号）。
    fn route_anchors() -> [(Route, &'static str, f32, f32); 3] {
        [
            // 佛朗哥：极右 + 上半（仍是军政府结构，但个人独裁）
            (Route::Franco, "佛朗哥胜利", 0.65, 0.35),
            // 长枪党：中右 + 下（个人独裁中等，党化推到极深）
            (Route::Falange, "长枪党胜利", 0.25, -0.65),
            // 架空考迪罗：右上 + 中下（佛朗哥保留为象征，党化制度化）
            (Route::Syndicalist, "架空考迪罗", 0.45, -0.20),
        ]
    }

    /// 当前最接近哪一条路线（按二维距离）。
    fn current_route(&self) -> (Route, f32) {
        let (x, y) = self.compass();
        let anchors = Self::route_anchors();
        let mut best = (anchors[0].0, f32::MAX);
        for (r, _, ax, ay) in anchors {
            let d = ((x - ax).powi(2) + (y - ay).powi(2)).sqrt();
            if d < best.1 {
                best = (r, d);
            }
        }
        best
    }

    /// 锁定：佛朗哥/长枪党/架空考迪罗各自的硬阈值。
    fn locked_route(&self) -> Option<Route> {
        if self.has("spa_franco_caudillo_proclaimed") && self.franco >= 12.0 && self.falange < 10.0
        {
            Some(Route::Franco)
        } else if self.falange >= 12.0 && self.franco < 10.0 {
            Some(Route::Falange)
        } else if self.falange >= 10.0
            && self.franco >= 10.0
            && self.army >= 8.0
            && self.has("spa_decision_falange_local_committees")
        {
            Some(Route::Syndicalist)
        } else {
            None
        }
    }
}

/// 顶层入口：渲染整个国民军权力小游戏。
fn render_mini_game(
    ui: &mut egui::Ui,
    data: &DecisionsData,
    cmds: &mut Vec<DecisionCommand>,
    consumed: &mut std::collections::HashSet<String>,
) {
    let flags: std::collections::HashSet<String> = data.country_flags.iter().cloned().collect();
    let v = FactionView::from(&data.mechanics, &flags);

    egui::Frame::new()
        .fill(components::PANEL_CARD)
        .stroke(egui::Stroke::new(1.2, components::BRONZE))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 10,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            // ── 标题条
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("◆")
                        .color(components::GOLD)
                        .strong()
                        .size(13.0),
                );
                ui.label(
                    RichText::new("国民军权力结构")
                        .strong()
                        .color(components::GOLD_BRIGHT)
                        .size(15.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(crisis_subtitle(&v))
                            .size(10.0)
                            .color(components::GOLD_DIM)
                            .italics(),
                    );
                });
            });

            // ── 当前局势叙事（动态，按 flags / gauge）
            ui.add(
                egui::Label::new(
                    RichText::new(crisis_ribbon(&v))
                        .size(10.5)
                        .color(components::MUTED),
                )
                .wrap(),
            );
            ui.add_space(8.0);

            // ── 二维路线指南针
            render_compass(ui, &v);
            ui.add_space(10.0);
            components::ornament_divider(ui);
            ui.add_space(7.0);

            // ── 派系卡片 2×2，决议直接挂在派系下
            render_faction_grid(ui, data, &v, cmds, consumed);
        });
}

/// 顶部副标题：随权力布局变化的"地点 · 日期 · 当前关键词"。
fn crisis_subtitle(v: &FactionView) -> String {
    let base = if v.has("spa_franco_caudillo_proclaimed") {
        "布尔戈斯 · 考迪罗已立"
    } else if v.has("spa_unification_decree_signed") {
        "萨拉曼卡 · 统一法令已下"
    } else if v.has("spa_mola_dies_in_air_crash_resolved") || v.has("spa_mola_dead") {
        "布尔戈斯 · 莫拉之后"
    } else if v.has("spa_sanjurjo_death_power_vacuum") {
        "布尔戈斯 · 桑胡尔霍空难之后"
    } else {
        "布尔戈斯 · 起义军政府"
    };
    base.to_owned()
}

/// 主叙事条：根据当前指南针位置、锁定状态和关键 flag，写一句压力句。
fn crisis_ribbon(v: &FactionView) -> &'static str {
    if let Some(r) = v.locked_route() {
        return match r {
            Route::Franco => {
                "胜利路线已锁向佛朗哥：将领效忠书、外援账本和教会祝祷都堆在同一张桌上。"
            }
            Route::Falange => "胜利路线已锁向长枪党：秘书处取代参谋部，旧衫派排着队挤进部委大门。",
            Route::Syndicalist => {
                "胜利路线已锁向架空考迪罗：佛朗哥被留作签字象征，制度由工团秘书处吃掉。"
            }
        };
    }
    if v.foreign >= 12.0 {
        "外援账本已经压过军政府的话事权——德意会把胜利写成抵押贷款。"
    } else if v.resistance >= 10.0 {
        "后方占领区开始燃烧，宪兵抽不出力气同时镇压三省。"
    } else if v.carlist >= 10.0 {
        "纳瓦拉的红贝雷已经在国旗下另插自己的徽章，统一法令一旦下达，北方就会炸开。"
    } else if v.falange >= 10.0 && v.franco >= 8.0 {
        "长枪党秘书处和大本营两台机器同时运转——谁先吞掉谁，胜利的形状会完全不同。"
    } else if v.has("spa_sanjurjo_death_power_vacuum") {
        "桑胡尔霍坠机后，军政府的椅子还空着；莫拉、佛朗哥、卡瓦内利亚斯都在递自己的名字。"
    } else {
        "起义军政府只是临时议事桌——每一笔决议都在把它推向其中一种永久形状。"
    }
}

// ─── 二维路线指南针 ────────────────────────────────────────────

fn render_compass(ui: &mut egui::Ui, v: &FactionView) {
    let locked = v.locked_route();
    let (current_route, _dist) = v.current_route();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("路线指南针")
                .strong()
                .size(11.0)
                .color(components::GOLD),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(r) = locked {
                ui.label(
                    RichText::new(format!("⛓ 已锁定 · {}", route_name(r)))
                        .strong()
                        .size(10.5)
                        .color(ROUTE_LOCKED),
                );
            } else {
                ui.label(
                    RichText::new(format!("✸ 当前趋向 · {}", route_name(current_route)))
                        .size(10.5)
                        .color(components::GOLD)
                        .italics(),
                );
            }
        });
    });
    ui.add_space(4.0);

    let width = ui.available_width().max(220.0);
    let height = 128.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let painter = ui.painter_at(rect);

    // 底面板
    painter.rect_filled(rect, 3.0, RAIL_DARK);
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, components::BRONZE),
        egui::StrokeKind::Inside,
    );

    // 内边距
    let pad = 14.0;
    let inner = rect.shrink(pad);

    // 网格 + 中央十字
    let cx = inner.center().x;
    let cy = inner.center().y;
    painter.line_segment(
        [egui::pos2(cx, inner.top()), egui::pos2(cx, inner.bottom())],
        egui::Stroke::new(0.6, ROUTE_DIM),
    );
    painter.line_segment(
        [egui::pos2(inner.left(), cy), egui::pos2(inner.right(), cy)],
        egui::Stroke::new(0.6, ROUTE_DIM),
    );

    // 轴标签
    let axis_color = components::MUTED;
    painter.text(
        egui::pos2(inner.right() - 2.0, cy - 6.0),
        egui::Align2::RIGHT_BOTTOM,
        "个人独裁 →",
        egui::FontId::proportional(9.0),
        axis_color,
    );
    painter.text(
        egui::pos2(inner.left() + 2.0, cy - 6.0),
        egui::Align2::LEFT_BOTTOM,
        "← 委员会制度",
        egui::FontId::proportional(9.0),
        axis_color,
    );
    painter.text(
        egui::pos2(cx + 6.0, inner.top() + 2.0),
        egui::Align2::LEFT_TOP,
        "↑ 老军政府",
        egui::FontId::proportional(9.0),
        axis_color,
    );
    painter.text(
        egui::pos2(cx + 6.0, inner.bottom() - 2.0),
        egui::Align2::LEFT_BOTTOM,
        "↓ 党化革命",
        egui::FontId::proportional(9.0),
        axis_color,
    );

    // 三条路线的锚点 + 收敛半径环
    for (route, name, ax, ay) in FactionView::route_anchors() {
        let px = cx + (inner.width() * 0.5) * ax;
        let py = cy - (inner.height() * 0.5) * ay; // Y 反向：屏幕向下为正
        let is_locked = locked == Some(route);
        let is_current = !is_locked && current_route == route;
        let (ring_color, label_color) = if is_locked {
            (ROUTE_LOCKED, ROUTE_LOCKED)
        } else if is_current {
            (components::GOLD, components::GOLD_BRIGHT)
        } else {
            (ROUTE_DIM, components::MUTED)
        };
        // 收敛半径环
        painter.circle_stroke(
            egui::pos2(px, py),
            18.0,
            egui::Stroke::new(if is_locked || is_current { 1.4 } else { 0.8 }, ring_color),
        );
        // 中心点
        painter.circle_filled(egui::pos2(px, py), 3.0, ring_color);
        // 路线名称
        painter.text(
            egui::pos2(px, py + 22.0),
            egui::Align2::CENTER_TOP,
            name,
            egui::FontId::proportional(10.0),
            label_color,
        );
    }

    // 当前位置
    let (x, y) = v.compass();
    let dot_x = cx + (inner.width() * 0.5) * x;
    let dot_y = cy - (inner.height() * 0.5) * y;
    let dot_color = if locked.is_some() {
        ROUTE_LOCKED
    } else {
        components::GOLD_BRIGHT
    };
    // 当前位置光圈
    painter.circle_stroke(
        egui::pos2(dot_x, dot_y),
        7.0,
        egui::Stroke::new(1.0, dot_color),
    );
    painter.circle_filled(egui::pos2(dot_x, dot_y), 4.0, dot_color);
    painter.text(
        egui::pos2(dot_x + 7.0, dot_y - 1.0),
        egui::Align2::LEFT_CENTER,
        "现在",
        egui::FontId::proportional(9.0),
        dot_color,
    );
}

fn route_name(r: Route) -> &'static str {
    match r {
        Route::Franco => "佛朗哥胜利",
        Route::Falange => "长枪党胜利",
        Route::Syndicalist => "架空考迪罗",
    }
}

// ─── 派系卡片网格 ──────────────────────────────────────────────

/// 决议所属派系：决定它出现在哪张卡片下。`None` = 仍在底部决议列表中。
#[derive(Clone, Copy)]
enum SpaFaction {
    Junta,
    Falange,
    Tradition,
    Foreign,
}

fn decision_faction(id: &str) -> Option<SpaFaction> {
    match id {
        "spa.centralize_general_staff" => Some(SpaFaction::Junta),
        "spa.convene_junta_counterweight" => Some(SpaFaction::Junta),
        "spa.empower_falange_local_committees" => Some(SpaFaction::Falange),
        "spa.falange_repression_columns" => Some(SpaFaction::Falange),
        "spa.court_church_and_traditionalists" => Some(SpaFaction::Tradition),
        "spa.request_condor_logistics" => Some(SpaFaction::Foreign),
        "spa.request_italian_artillery" => Some(SpaFaction::Foreign),
        "spa.military_occupation_administration" => Some(SpaFaction::Foreign),
        _ => None,
    }
}

fn render_faction_grid(
    ui: &mut egui::Ui,
    data: &DecisionsData,
    v: &FactionView,
    cmds: &mut Vec<DecisionCommand>,
    consumed: &mut std::collections::HashSet<String>,
) {
    let by_faction = |which: SpaFaction| -> Vec<&DecisionEntry> {
        data.decisions
            .iter()
            .filter(|e| e.visible && matches!(decision_faction(&e.id), Some(f) if std::mem::discriminant(&f) == std::mem::discriminant(&which)))
            .collect()
    };

    let cards: [(
        SpaFaction,
        Color32,
        &str,
        &str,
        (&str, &str, &str),
        (&str, Color32),
        &str,
        Vec<GaugeMini>,
    ); 4] = {
        let franco = franco_career(v);
        let hed = falange_career(v);
        let trad = tradition_career(v);
        let frn = foreign_career(v);
        [
            (
                SpaFaction::Junta,
                FRANCO_RED,
                "★",
                "军政府",
                franco,
                franco_tier(v.franco),
                franco_action(v),
                vec![
                    GaugeMini {
                        label: "佛朗哥权威",
                        value: v.franco,
                        max: 15.0,
                        thresholds: &[8.0, 12.0],
                        risk: false,
                    },
                    GaugeMini {
                        label: "军队忠诚",
                        value: v.army,
                        max: 15.0,
                        thresholds: &[8.0, 12.0],
                        risk: false,
                    },
                ],
            ),
            (
                SpaFaction::Falange,
                FALANGE_BLUE,
                "◆",
                "长枪党",
                hed,
                falange_tier(v.falange),
                falange_action(v),
                vec![GaugeMini {
                    label: "长枪党/蓝衫",
                    value: v.falange,
                    max: 15.0,
                    thresholds: &[8.0, 12.0],
                    risk: true,
                }],
            ),
            (
                SpaFaction::Tradition,
                TRADITION_CRIMSON,
                "✚",
                "传统派",
                trad,
                tradition_tier(v.church, v.carlist),
                tradition_action(v),
                vec![
                    GaugeMini {
                        label: "教会影响",
                        value: v.church,
                        max: 15.0,
                        thresholds: &[8.0, 12.0],
                        risk: false,
                    },
                    GaugeMini {
                        label: "卡洛斯派怨恨",
                        value: v.carlist,
                        max: 15.0,
                        thresholds: &[8.0, 12.0],
                        risk: true,
                    },
                ],
            ),
            (
                SpaFaction::Foreign,
                FOREIGN_GRAY,
                "⚓",
                "外援与后方",
                frn,
                foreign_tier(v.foreign, v.resistance),
                foreign_action(v),
                vec![
                    GaugeMini {
                        label: "外援依赖",
                        value: v.foreign,
                        max: 15.0,
                        thresholds: &[8.0, 12.0],
                        risk: true,
                    },
                    GaugeMini {
                        label: "占领区抵抗",
                        value: v.resistance,
                        max: 15.0,
                        thresholds: &[8.0, 12.0],
                        risk: true,
                    },
                ],
            ),
        ]
    };

    for (which, accent, emblem, title, career, tier, action, gauges) in cards {
        let decisions = by_faction(which);
        for d in &decisions {
            consumed.insert(d.id.clone());
        }
        faction_card(
            ui,
            FactionCardData {
                accent,
                emblem,
                title,
                leader: career.0,
                leader_title: career.1,
                tier_label: tier,
                action,
                gauges: &gauges,
                detail: career.2,
            },
            &decisions,
            cmds,
        );
        ui.add_space(6.0);
    }
}

struct FactionCardData<'a> {
    accent: Color32,
    emblem: &'a str,
    title: &'a str,
    leader: &'a str,
    leader_title: &'a str,
    tier_label: (&'a str, Color32),
    action: &'a str,
    gauges: &'a [GaugeMini<'a>],
    detail: &'a str,
}

struct GaugeMini<'a> {
    label: &'a str,
    value: f32,
    max: f32,
    thresholds: &'a [f32],
    risk: bool,
}

fn wrap_label(ui: &mut egui::Ui, text: RichText) {
    ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Wrap));
}

fn faction_card(
    ui: &mut egui::Ui,
    c: FactionCardData,
    decisions: &[&DecisionEntry],
    cmds: &mut Vec<DecisionCommand>,
) {
    // 派系背景上微调，给卡片一种淡淡的派系气氛色，但不过度
    let tint_fill =
        Color32::from_rgba_premultiplied(c.accent.r() / 8, c.accent.g() / 8, c.accent.b() / 8, 220);

    let frame = egui::Frame::new()
        .fill(tint_fill)
        .stroke(egui::Stroke::new(1.0, components::STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 16,
            right: 12,
            top: 10,
            bottom: 11,
        })
        .show(ui, |ui| {
            // ─ 顶行：派系徽记 + 派系名（小） + tier chip（右）
            ui.horizontal(|ui| {
                ui.label(RichText::new(c.emblem).strong().size(14.0).color(c.accent));
                ui.label(RichText::new(c.title).size(10.0).color(c.accent).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    tier_chip(ui, c.tier_label.0, c.tier_label.1);
                });
            });

            // ─ 领袖名：大字号、独占一行（焦点）
            wrap_label(
                ui,
                RichText::new(c.leader)
                    .strong()
                    .size(16.0)
                    .color(components::GOLD_BRIGHT),
            );

            // ─ 当前任职阶段（独占一行，斜体）
            wrap_label(
                ui,
                RichText::new(c.leader_title)
                    .size(11.0)
                    .color(components::PARCHMENT)
                    .italics(),
            );
            ui.add_space(6.0);

            // ─ 派系动作叙事 — 左色条 + 引文
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(2.0, 1.0), Sense::hover());
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(bar_rect.left_top(), egui::vec2(2.0, 1.0)),
                    0.0,
                    c.accent,
                );
                ui.add_space(5.0);
                wrap_label(
                    ui,
                    RichText::new(c.action).size(11.0).color(components::MUTED),
                );
            });
            ui.add_space(7.0);

            // ─ Gauge bars，多个时横向并排，单个则铺满
            if c.gauges.len() == 1 {
                gauge_mini(ui, &c.gauges[0], c.accent);
            } else {
                let total_w = ui.available_width();
                let each = (total_w - 8.0) / c.gauges.len() as f32;
                ui.horizontal(|ui| {
                    for (i, g) in c.gauges.iter().enumerate() {
                        ui.allocate_ui_with_layout(
                            egui::vec2(each, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                ui.set_width(each);
                                gauge_mini(ui, g, c.accent);
                            },
                        );
                        if i + 1 < c.gauges.len() {
                            ui.add_space(8.0);
                        }
                    }
                });
            }

            // ─ 内嵌决议
            if !decisions.is_empty() {
                ui.add_space(8.0);
                let sep_color = Color32::from_rgba_premultiplied(0x40, 0x30, 0x18, 200);
                let avail_w = ui.available_width();
                let (sep, _) = ui.allocate_exact_size(egui::vec2(avail_w, 1.0), Sense::hover());
                ui.painter_at(sep).line_segment(
                    [sep.left_top(), sep.right_top()],
                    egui::Stroke::new(0.8, sep_color),
                );
                ui.add_space(5.0);
                ui.label(
                    RichText::new("派系决议")
                        .size(9.5)
                        .color(components::GOLD_DIM)
                        .strong(),
                );
                ui.add_space(3.0);
                for d in decisions {
                    if let Some(cmd) = render_spa_decision_chip(ui, d, c.accent) {
                        cmds.push(cmd);
                    }
                    ui.add_space(3.0);
                }
            }
        });

    // 左侧 3px accent 条带
    let rect = frame.response.rect;
    let stripe = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(3.0, rect.height() - 2.0),
    );
    ui.painter().rect_filled(stripe, 0.0, c.accent);

    frame.response.on_hover_text(c.detail);
}

/// 派系卡内的紧凑决议条目：左侧 accent 圆点、名字、右侧按钮。
fn render_spa_decision_chip(
    ui: &mut egui::Ui,
    entry: &DecisionEntry,
    accent: Color32,
) -> Option<DecisionCommand> {
    let mut cmd = None;
    let (state_label, state_color) = if entry.mission_remaining.is_some() {
        ("进行中", components::WARN)
    } else if entry.cooldown_remaining.is_some() {
        ("冷却", components::MUTED)
    } else if entry.already_fired {
        ("已用", components::GOOD)
    } else if entry.clickable {
        ("执行", components::GOLD_BRIGHT)
    } else if entry.cost_political_power > 0.0 {
        ("PP 不足", components::MUTED)
    } else {
        ("不可用", components::MUTED)
    };

    let frame = egui::Frame::new()
        .fill(components::PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(
            0.8,
            Color32::from_rgba_premultiplied(0x40, 0x30, 0x18, 220),
        ))
        .inner_margin(egui::Margin {
            left: 9,
            right: 8,
            top: 6,
            bottom: 7,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // accent 圆点
                let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                ui.painter_at(dot_rect)
                    .circle_filled(dot_rect.center(), 3.5, accent);
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(&entry.name)
                            .size(11.0)
                            .strong()
                            .color(components::PARCHMENT),
                    )
                    .wrap_mode(egui::TextWrapMode::Wrap),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = egui::Button::new(
                        RichText::new(state_label)
                            .strong()
                            .size(10.0)
                            .color(state_color),
                    )
                    .min_size(egui::vec2(64.0, 22.0))
                    .fill(Color32::from_rgba_premultiplied(0x18, 0x10, 0x08, 230))
                    .stroke(egui::Stroke::new(0.8, state_color));
                    if ui.add_enabled(entry.clickable, btn).clicked() {
                        cmd = Some(DecisionCommand::Activate(entry.id.clone()));
                    }
                });
            });

            // 元信息行：PP / 冷却 / 任务剩余
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(12.0);
                if entry.cost_political_power > 0.0 {
                    ui.label(
                        RichText::new(format!("PP {:.0}", entry.cost_political_power))
                            .size(9.5)
                            .color(components::GOLD_DIM),
                    );
                }
                if let Some(d) = entry.cooldown_remaining {
                    ui.label(
                        RichText::new(format!("· 冷却 {}d", d))
                            .size(9.5)
                            .color(components::MUTED),
                    );
                }
                if let Some(d) = entry.mission_remaining {
                    ui.label(
                        RichText::new(format!("· 剩 {}d", d))
                            .size(9.5)
                            .color(components::WARN),
                    );
                }
            });

            // 效果摘要（独立行，避免和元信息挤）
            if !entry.effect_preview.is_empty() {
                ui.add_space(1.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("→ {}", entry.effect_preview))
                                .size(9.5)
                                .color(components::GOOD)
                                .italics(),
                        )
                        .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                });
            }
        });

    let tooltip = if entry.description.is_empty() {
        entry.name.clone()
    } else {
        format!("{}\n\n{}", entry.name, entry.description)
    };
    frame.response.on_hover_text(tooltip);
    cmd
}

fn tier_chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x12, 0x0c, 0x06, 240))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).strong().size(9.5).color(color));
        });
}

fn gauge_mini(ui: &mut egui::Ui, g: &GaugeMini, accent: Color32) {
    let max = g.max.max(1.0);
    let ratio = (g.value / max).clamp(0.0, 1.0);
    let bar_color = gauge_color(g.value, g.risk, accent);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(g.label)
                .size(10.0)
                .color(components::PARCHMENT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:.1} / {:.0}", g.value, max))
                    .strong()
                    .size(10.0)
                    .color(bar_color),
            );
        });
    });
    ui.add_space(1.0);
    let width = ui.available_width().max(80.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 7.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 1.5, RAIL_DARK);
    let fill = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.left() + rect.width() * ratio, rect.bottom()),
    );
    painter.rect_filled(fill, 1.5, bar_color);
    for (i, t) in g.thresholds.iter().enumerate() {
        let x = rect.left() + rect.width() * (t / max).clamp(0.0, 1.0);
        let stroke_c = if i + 1 == g.thresholds.len() {
            components::GOLD_BRIGHT
        } else {
            components::GOLD_DIM
        };
        painter.line_segment(
            [
                egui::pos2(x, rect.top() - 1.0),
                egui::pos2(x, rect.bottom() + 1.0),
            ],
            egui::Stroke::new(1.0, stroke_c),
        );
    }
    painter.rect_stroke(
        rect,
        1.5,
        egui::Stroke::new(1.0, components::BRONZE),
        egui::StrokeKind::Inside,
    );
}

fn gauge_color(value: f32, risk: bool, accent: Color32) -> Color32 {
    if risk {
        if value >= 12.0 {
            components::BAD
        } else if value >= 8.0 {
            components::WARN
        } else if value >= 4.0 {
            components::GOLD_BRIGHT
        } else {
            components::BLUE
        }
    } else if value >= 12.0 {
        ROUTE_LOCKED
    } else if value >= 8.0 {
        accent
    } else if value >= 4.0 {
        components::GOLD_BRIGHT
    } else {
        components::MUTED
    }
}

// ─── 派系层级与人物任职阶段 ───────────────────────────────────────

fn franco_tier(v: f32) -> (&'static str, Color32) {
    if v >= 12.0 {
        ("考迪罗", components::GOLD_BRIGHT)
    } else if v >= 8.0 {
        ("统帅候选", components::GOLD)
    } else if v >= 5.0 {
        ("外援枢纽", components::BLUE)
    } else if v >= 2.0 {
        ("协调者", components::MUTED)
    } else {
        ("边缘候选", components::BAD)
    }
}

fn franco_action(v: &FactionView) -> &'static str {
    if v.franco >= 12.0 {
        "签字的笔从未离开过他的手；任命名单由他批准。"
    } else if v.franco >= 8.0 && v.army >= 8.0 {
        "在布尔戈斯指挥部接受将领效忠，军政府选举近在眼前。"
    } else if v.franco >= 5.0 {
        "靠德意运输和非洲军团积累筹码，谨慎地不站到莫拉前面。"
    } else if v.army < 4.0 {
        "军队还没决定听谁的；将军们在等下一场战役结果。"
    } else {
        "佛朗哥仍是非洲军团的将军，不是西班牙的统帅。"
    }
}

/// 佛朗哥的当前任职阶段：(领袖名, 阶段, 详细 tooltip)。
/// 阶段按 gauge 阈值 + 关键 flag 推进，模拟 1936→统一→考迪罗→……的轨迹。
fn franco_career(v: &FactionView) -> (&'static str, &'static str, &'static str) {
    let name = "佛朗哥";
    let title = if v.has("spa_franco_caudillo_proclaimed") {
        "国家元首 · 考迪罗"
    } else if v.franco >= 12.0 {
        "大本营统帅 · 考迪罗候选"
    } else if v.has("spa_supreme_command_franco") || (v.franco >= 8.0 && v.army >= 8.0) {
        "起义军最高统帅"
    } else if v.franco >= 5.0 {
        "外援协调者 · 大本营第二人"
    } else if v.has("spa_sanjurjo_death_power_vacuum") || v.franco >= 2.0 {
        "国防委员会成员 · 非洲军团总司令"
    } else {
        "非洲军团总司令（加那利空军总司令）"
    };
    let detail = if v.has("spa_franco_caudillo_proclaimed") {
        "考迪罗已宣告——军政府的椅子被搬走，只剩他一张签字桌。\n继续推动佛朗哥权威可压制长枪党秘书处；放任长枪党或卡洛斯派坐大则会反向解锁架空考迪罗路线。"
    } else if v.franco >= 8.0 {
        "桑胡尔霍已死，莫拉北方网络收拢中。佛朗哥从外援协调者上升为统帅候选——下一步是国防委员会的统帅投票或绕开委员会的个人任命。"
    } else {
        "桑胡尔霍坠机后军政府权力真空。佛朗哥目前还是非洲军团的将军，不是西班牙的统帅——决议会决定他是登上统帅椅，还是被国防委员会用合议压回去。"
    };
    (name, title, detail)
}

fn falange_tier(v: f32) -> (&'static str, Color32) {
    if v >= 12.0 {
        ("蓝衫革命", components::BAD)
    } else if v >= 8.0 {
        ("党国上升", components::WARN)
    } else if v >= 4.0 {
        ("地方民兵", components::GOLD_BRIGHT)
    } else {
        ("分散街头", components::MUTED)
    }
}

fn falange_action(v: &FactionView) -> &'static str {
    if v.falange >= 12.0 {
        "秘书处发出任命，蓝衫已经走进部长办公室。"
    } else if v.falange >= 8.0 {
        "赫迪利亚召开党务会议，旧衫派要求「真正长枪党」主导。"
    } else if v.falange >= 4.0 {
        "省级蓝衫民兵巡逻街道，宣传机器开始组装。"
    } else {
        "长枪党仍是地方俱乐部和酒吧争吵，没有统一意志。"
    }
}

/// 长枪党当前实际控制者：何塞·安东尼奥缺席，所以名字会随事件推进改变。
fn falange_career(v: &FactionView) -> (&'static str, &'static str, &'static str) {
    if v.has("spa_falange_seizes_secretariat") {
        return (
            "赫迪利亚",
            "全国秘书处书记 · 旧衫派领袖",
            "赫迪利亚拿下秘书处——旧衫派从街头组织升级为国家机器。统一法令一旦下来，他会决定是被佛朗哥收编还是反过来吃掉佛朗哥。",
        );
    }
    if v.has("spa_hedilla_arrested") {
        return (
            "塞拉诺·苏涅尔",
            "佛朗哥连襟 · 党务调停人",
            "赫迪利亚被捕；长枪党的合法继承被搁在佛朗哥的桌子上。塞拉诺·苏涅尔接手党务重组，旧衫派被迫低头。",
        );
    }
    if v.has("spa_hedilla_compromise") {
        return (
            "赫迪利亚",
            "秘书处书记 · 与佛朗哥妥协",
            "赫迪利亚同意进政府，长枪党的革命口号被装进国家框架——旧衫派愤怒，但暂时收起匕首。",
        );
    }
    if v.has("spa_jose_antonio_dead") || v.has("spa_falange_martyrdom") {
        return (
            "赫迪利亚",
            "继承危机中的旧衫派党魁",
            "何塞·安东尼奥之死把党推进继承危机：殉道者已死，赫迪利亚、塞拉诺·苏涅尔、巴利亚拉斯都伸手要旗帜。",
        );
    }
    (
        "赫迪利亚",
        "代理书记 · 等何塞·安东尼奥的消息",
        "何塞·安东尼奥仍在阿利坎特狱中。赫迪利亚、塞拉诺·苏涅尔和旧衫派都在等他的死讯——党的合法性还没有第一具尸体。",
    )
}

fn tradition_tier(church: f32, carlist: f32) -> (&'static str, Color32) {
    if carlist >= 12.0 {
        ("北方反弹", components::BAD)
    } else if church >= 10.0 {
        ("国民天主教", components::GOLD_BRIGHT)
    } else if carlist >= 8.0 {
        ("红贝雷警惕", components::WARN)
    } else if church >= 5.0 {
        ("讲台背书", components::GOLD)
    } else {
        ("观望", components::MUTED)
    }
}

fn tradition_action(v: &FactionView) -> &'static str {
    if v.carlist >= 12.0 {
        "纳瓦拉的红贝雷拒绝整合，统一法令前夜北方紧张。"
    } else if v.church >= 10.0 {
        "主教会议要求把战争写成十字军；学校与婚姻回到神父手里。"
    } else if v.carlist >= 8.0 {
        "法尔·孔德要求王位承诺和民兵自治作为参战代价。"
    } else if v.church >= 5.0 {
        "地方主教在弥撒中为国民军祝祷，但避免谈党化。"
    } else {
        "教会和卡洛斯派暂时与军政府同行，没有写下条件。"
    }
}

fn tradition_career(v: &FactionView) -> (&'static str, &'static str, &'static str) {
    if v.has("spa_unification_decree_signed") {
        return (
            "法尔·孔德（流亡）/ 戈马枢机",
            "卡洛斯派被强制并入 · 教会另谋讲台",
            "统一法令下达后，法尔·孔德被驱逐出境，红贝雷被改写成统一民兵的徽章。教会保留弥撒，但放弃自主政治。",
        );
    }
    if v.carlist >= 10.0 {
        return (
            "法尔·孔德 / 主教会议",
            "纳瓦拉北方反弹 · 王位悬而未决",
            "卡洛斯派要求王位承诺和民兵自治作为继续参战的代价——红贝雷的合作开始按笔交易，不再是免费的北方人力。",
        );
    }
    if v.church >= 10.0 {
        return (
            "戈马枢机 / 主教会议",
            "国民天主教 · 十字军讲台",
            "教会把战争解释成十字军；学校、婚姻和地方救济回到神父手里——长枪党的革命口号被压在祭坛之下。",
        );
    }
    (
        "法尔·孔德 / 主教会议",
        "红贝雷与十字军 · 同行但未签字",
        "教会把战争解释成十字军；卡洛斯派要求王位与民兵自治。统一法令一旦下达，怨恨会决定北方是否反弹。",
    )
}

fn foreign_tier(foreign: f32, resistance: f32) -> (&'static str, Color32) {
    if foreign >= 12.0 || resistance >= 12.0 {
        ("抵押账本", components::BAD)
    } else if foreign >= 8.0 || resistance >= 8.0 {
        ("代价上升", components::WARN)
    } else if foreign >= 4.0 || resistance >= 4.0 {
        ("可控成本", components::GOLD_BRIGHT)
    } else {
        ("尚未结账", components::MUTED)
    }
}

fn foreign_action(v: &FactionView) -> &'static str {
    if v.foreign >= 12.0 {
        "柏林与罗马的账本写在国库门上：钨矿、基地与外交服从被列为还款。"
    } else if v.resistance >= 12.0 {
        "占领区抵抗压过军政府治理能力；后方在燃烧。"
    } else if v.foreign >= 8.0 {
        "秃鹰军团和 CTV 进驻关键战场，西班牙开始向轴心倾斜。"
    } else if v.resistance >= 8.0 {
        "白色恐怖让地方游击重新出现，军法庭忙碌起来。"
    } else if v.foreign >= 4.0 {
        "德意武器抵达，但还款条件留白。"
    } else {
        "外援尚是货物清单，不是政治账本。"
    }
}

fn foreign_career(v: &FactionView) -> (&'static str, &'static str, &'static str) {
    if v.foreign >= 12.0 {
        return (
            "施佩里 / 法格上将（驻西轴心代表）",
            "外援债主 · 已写进战后还款表",
            "秃鹰军团的飞机和 CTV 的炮兵都已经在前线——账本上的还款条款是钨矿、基地和外交服从。",
        );
    }
    if v.resistance >= 10.0 {
        return (
            "占领区军管 · 宪兵总督",
            "白色恐怖外溢 · 抵抗复活",
            "宪兵、长枪党民兵和军法庭在后方同时运转——抵抗反而被白色恐怖喂养，地方游击重新出现。",
        );
    }
    if v.foreign >= 8.0 {
        return (
            "里希特霍芬男爵 / 罗阿塔将军",
            "秃鹰军团与 CTV · 共同作战",
            "德国教官把空军当成实验室，意大利把 CTV 当成威望工程——胜利会属于西班牙，账单不会。",
        );
    }
    (
        "尚未签账 · 临时使节",
        "外援尚是货物，不是政治账本",
        "德国矿产、意大利威望、占领区清洗——这些账本不会出现在胜利游行里，但会决定战后是孤立还是抵押。",
    )
}

// ─────────────────────────────────────────────────────────────
// 决议列表
// ─────────────────────────────────────────────────────────────

fn render_decisions(
    ui: &mut egui::Ui,
    data: &DecisionsData,
    cmds: &mut Vec<DecisionCommand>,
    consumed: &std::collections::HashSet<String>,
) {
    let mut any_visible = false;
    for category in [
        hoi4_content::DecisionCategory::Crisis,
        hoi4_content::DecisionCategory::Internal,
        hoi4_content::DecisionCategory::Military,
        hoi4_content::DecisionCategory::Diplomacy,
        hoi4_content::DecisionCategory::Industry,
    ] {
        let entries: Vec<&DecisionEntry> = data
            .decisions
            .iter()
            .filter(|entry| {
                entry.visible && entry.category == category && !consumed.contains(&entry.id)
            })
            .collect();
        if entries.is_empty() {
            continue;
        }
        any_visible = true;
        components::section(ui, category_label_zh(category), |ui| {
            for entry in entries {
                if let Some(cmd) = render_decision(ui, entry) {
                    cmds.push(cmd);
                }
                ui.add_space(5.0);
            }
        });
    }
    if !any_visible && consumed.is_empty() {
        ui.add_space(8.0);
        components::empty_state(
            ui,
            "暂无可见决议",
            "积累政治点、推进战局或处理派系危机以解锁决议。",
        );
    }
}

fn category_label_zh(c: hoi4_content::DecisionCategory) -> &'static str {
    match c {
        hoi4_content::DecisionCategory::Industry => "工业 · 建造",
        hoi4_content::DecisionCategory::Diplomacy => "外交 · 阵营",
        hoi4_content::DecisionCategory::Military => "军事 · 动员",
        hoi4_content::DecisionCategory::Internal => "内政 · 宣传",
        hoi4_content::DecisionCategory::Crisis => "危机 · 一次性",
    }
}

fn render_decision(ui: &mut egui::Ui, entry: &DecisionEntry) -> Option<DecisionCommand> {
    let mut cmd = None;
    let (state_label, state_color) = decision_state(entry);

    let inner = egui::Frame::new()
        .fill(components::PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, components::STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 13,
            right: 10,
            top: 8,
            bottom: 9,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.horizontal(|ui| {
                let title_width = (ui.available_width() - 86.0).max(120.0);
                ui.add_sized(
                    egui::vec2(title_width, 0.0),
                    egui::Label::new(
                        RichText::new(&entry.name)
                            .strong()
                            .color(components::GOLD_BRIGHT)
                            .size(13.0),
                    )
                    .wrap(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let text = RichText::new(state_label)
                        .color(state_color)
                        .strong()
                        .size(12.0);
                    if ui
                        .add_enabled(
                            entry.clickable,
                            egui::Button::new(text).min_size(egui::vec2(72.0, 26.0)),
                        )
                        .clicked()
                    {
                        cmd = Some(DecisionCommand::Activate(entry.id.clone()));
                    }
                });
            });

            if !entry.description.is_empty() {
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(&entry.description)
                            .size(11.0)
                            .color(components::PARCHMENT),
                    )
                    .wrap(),
                );
            }

            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                meta_chip(
                    ui,
                    entry.mechanic_kind.label(),
                    components::GOLD_DIM,
                    components::PARCHMENT,
                );
                if entry.cost_political_power > 0.0 {
                    meta_chip(
                        ui,
                        &format!("PP {:.0}", entry.cost_political_power),
                        components::GOLD_DIM,
                        components::GOLD_BRIGHT,
                    );
                }
                if let Some(remaining) = entry.cooldown_remaining {
                    meta_chip(
                        ui,
                        &format!("冷却 {}d", remaining),
                        components::MUTED,
                        components::MUTED,
                    );
                }
                if entry.already_fired {
                    meta_chip(ui, "已完成", components::GOOD, components::GOOD);
                }
            });

            if let Some(remaining) = entry.mission_remaining {
                let total = entry.mission_total.unwrap_or(remaining).max(1);
                let done = total.saturating_sub(remaining);
                ui.add_space(4.0);
                render_mission_progress(ui, done, total);
            }

            if !entry.effect_preview.is_empty() {
                ui.add_space(3.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("→ {}", entry.effect_preview))
                            .size(10.5)
                            .color(components::GOOD),
                    )
                    .wrap(),
                );
            }
        });

    let rect = inner.response.rect;
    let stripe = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(3.0, rect.height() - 2.0),
    );
    ui.painter().rect_filled(stripe, 0.0, state_color);

    cmd
}

fn decision_state(entry: &DecisionEntry) -> (&'static str, Color32) {
    if entry.mission_remaining.is_some() {
        ("进行中", components::WARN)
    } else if entry.cooldown_remaining.is_some() {
        ("冷却", components::MUTED)
    } else if entry.already_fired {
        ("已完成", components::GOOD)
    } else if entry.clickable {
        ("执行", components::GOLD_BRIGHT)
    } else {
        ("不可用", components::MUTED)
    }
}

fn meta_chip(ui: &mut egui::Ui, label: &str, border: Color32, text_color: Color32) {
    egui::Frame::new()
        .fill(components::PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, border))
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(10.0).color(text_color).strong());
        });
}

fn render_mission_progress(ui: &mut egui::Ui, done: u32, total: u32) {
    let total_f = total.max(1) as f32;
    let ratio = (done as f32 / total_f).clamp(0.0, 1.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("任务进度")
                .size(10.0)
                .color(components::MUTED),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{} / {} 天", done, total))
                    .size(10.0)
                    .color(components::WARN)
                    .strong(),
            );
        });
    });
    ui.add_space(1.0);
    let width = ui.available_width().max(120.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 5.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 1.5, RAIL_INK);
    let fill = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.left() + rect.width() * ratio, rect.bottom()),
    );
    painter.rect_filled(fill, 1.5, components::WARN);
    painter.rect_stroke(
        rect,
        1.5,
        egui::Stroke::new(1.0, components::BRONZE),
        egui::StrokeKind::Inside,
    );
}
