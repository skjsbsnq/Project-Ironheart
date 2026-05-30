//! J.4b: 物流仓储面板。快捷键 L 开关。
//!
//! 上半部分：装备库存（库存/日产/军队需求/采购/缺口）
//! 下半部分：战略资源速览（只保留摘要，详细供需在市场面板）。

use crate::{components, i18n::tr};
use egui::{Color32, RichText};

const GOLD: Color32 = Color32::from_rgb(0xc9, 0xa5, 0x5b);
const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xe0, 0xc0, 0x78);
const MUTED: Color32 = Color32::from_gray(155);
const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x60, 0x58);
const BLUE: Color32 = Color32::from_rgb(0x68, 0xa0, 0xd8);

/// 单条装备库存条目。
pub struct LogisticsEntry {
    pub name: String,
    pub stockpile: f32,
    pub daily_production: f32,
    pub daily_replenishment_need: f32,
    pub daily_training_need: f32,
    pub daily_maintenance_need: f32,
    pub daily_consumption: f32,
    pub net_change: f32,
    pub deficit: f32,
    pub days_until_empty: Option<f32>,
    pub procurement_rm: f64,
    pub production_sources: Vec<String>,
}

/// 单条资源条目。
pub struct ResourceEntry {
    pub name: String,
    pub produced: f32,
    pub consumed: f32,
    /// 累积仓储量
    pub stored: f32,
}

/// 面板数据快照。
pub struct LogisticsData {
    pub entries: Vec<LogisticsEntry>,
    pub total_types: usize,
    pub deficit_types: usize,
    pub total_daily_production: f32,
    pub total_daily_need: f32,
    pub military_procurement_rm: f64,
    pub military_maintenance_rm: f64,
    /// 战略资源收支
    pub resources: Vec<ResourceEntry>,
}

pub struct LogisticsPanel;

impl LogisticsPanel {
    /// 返回 close_requested。
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &LogisticsData) -> bool {
        return v9_show_logistics(ctx, data);

        let mut close = false;
        egui::SidePanel::left("logistics_panel")
            .default_width(540.0)
            .min_width(460.0)
            .resizable(true)
            .show(ctx, |ui| {
                components::panel_header(ui, tr("logistics"), &mut close);
                render_summary(ui, data);
                render_status_banner(ui, data);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_equipment_section(ui, data);
                        render_resource_section(ui, data);
                    });
            });
        close
    }
}

fn v9_show_logistics(ctx: &egui::Context, data: &LogisticsData) -> bool {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let net = data.total_daily_production - data.total_daily_need;
    let accent = if data.deficit_types > 0 {
        palette::BAD
    } else if net < 0.0 {
        palette::WARN
    } else {
        palette::GOOD
    };
    let (close, _) = PanelShell::new("logistics_panel_v9", tr("logistics"))
        .subtitle("Equipment stockpile / deficit pressure")
        .class(PanelClass::Economy)
        .accent(accent)
        .footer("Q Close  |  Equipment table / Deficit bars")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("Types", data.total_types.to_string(), palette::GOLD),
                    (
                        "Deficits",
                        data.deficit_types.to_string(),
                        if data.deficit_types > 0 {
                            palette::BAD
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        "Production",
                        signed_one_decimal(data.total_daily_production),
                        palette::GOOD,
                    ),
                    (
                        "Need",
                        format!("-{:.1}/d", data.total_daily_need),
                        palette::WARN,
                    ),
                    (
                        "Net",
                        signed_one_decimal(net),
                        v9_logistics_signed_color(net),
                    ),
                    (
                        "Procurement",
                        format_rm(data.military_procurement_rm),
                        palette::GOLD,
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "Equipment DataTable / Deficit ProgressBar",
                accent,
            );
            v9_logistics_body(ui, layout.body, data);
        });
    close
}

fn v9_logistics_body(ui: &mut egui::Ui, rect: egui::Rect, data: &LogisticsData) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.68), Track::Fr(0.32)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_logistics_equipment_table(ui, GridLayout::cell(&cells, 0, 0), data);
        v9_logistics_side(ui, GridLayout::cell(&cells, 0, 1), data);
    });
}

fn v9_logistics_equipment_table(ui: &mut egui::Ui, rect: egui::Rect, data: &LogisticsData) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Equipment stockpile",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let mut entries: Vec<&LogisticsEntry> = data.entries.iter().collect();
    entries.sort_by(|a, b| {
        b.deficit
            .partial_cmp(&a.deficit)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.days_until_empty
                    .unwrap_or(f32::MAX)
                    .partial_cmp(&b.days_until_empty.unwrap_or(f32::MAX))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let rows: Vec<TableRow> = entries
        .into_iter()
        .take(18)
        .map(|entry| {
            let accent = if entry.deficit > 0.0 {
                palette::BAD
            } else if entry.net_change < 0.0 {
                palette::WARN
            } else {
                palette::GOOD
            };
            TableRow::new(vec![
                TableCell::strong(entry.name.as_str()),
                TableCell::new(format!("{:.0}", entry.stockpile)).right(),
                TableCell::colored(format!("{:+.1}", entry.daily_production), palette::GOOD)
                    .right(),
                TableCell::new(format!("{:.1}", entry.daily_consumption)).right(),
                TableCell::colored(format!("{:+.1}", entry.net_change), accent).right(),
                TableCell::colored(
                    if entry.deficit > 0.0 {
                        format!("{:.1}", entry.deficit)
                    } else {
                        "-".to_owned()
                    },
                    if entry.deficit > 0.0 {
                        palette::BAD
                    } else {
                        palette::MUTED
                    },
                )
                .right(),
                TableCell::new(
                    entry
                        .days_until_empty
                        .map(|days| format!("{:.0}d", days))
                        .unwrap_or_else(|| "-".to_owned()),
                )
                .right(),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("Equipment", 1.25),
            TableColumn::new("Stock", 0.65).right(),
            TableColumn::new("Prod", 0.65).right(),
            TableColumn::new("Need", 0.65).right(),
            TableColumn::new("Net", 0.65).right(),
            TableColumn::new("Gap", 0.65).right(),
            TableColumn::new("Empty", 0.65).right(),
        ],
        rows,
    )
    .row_height(28.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        ),
    );
}

fn v9_logistics_side(ui: &mut egui::Ui, rect: egui::Rect, data: &LogisticsData) {
    use crate::v9::primitives::{
        draw_progress_bar, Card, DataTable, TableCell, TableColumn, TableRow,
    };
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Deficit pressure",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let max_deficit = data
        .entries
        .iter()
        .map(|entry| entry.deficit.max(0.0))
        .fold(0.0_f32, f32::max)
        .max(1.0);
    let mut deficit_entries: Vec<&LogisticsEntry> = data
        .entries
        .iter()
        .filter(|entry| entry.deficit > 0.0 || entry.net_change < 0.0)
        .collect();
    deficit_entries.sort_by(|a, b| {
        b.deficit
            .partial_cmp(&a.deficit)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut y = inner.top() + 34.0;
    if deficit_entries.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 112.0)),
            "Stockpile stable",
            "No equipment deficit is currently active.",
        );
        y += 124.0;
    } else {
        for entry in deficit_entries.iter().take(6) {
            ui.painter().text(
                Pos2::new(inner.left(), y),
                Align2::LEFT_TOP,
                entry.name.as_str(),
                TextRole::Caption.font_id(),
                palette::PARCHMENT,
            );
            let ratio = (entry.deficit.max(-entry.net_change) / max_deficit).clamp(0.0, 1.0);
            draw_progress_bar(
                ui,
                Rect::from_min_size(
                    Pos2::new(inner.left(), y + 18.0),
                    Vec2::new(inner.width(), 9.0),
                ),
                ratio,
                if entry.deficit > 0.0 {
                    palette::BAD
                } else {
                    palette::WARN
                },
            );
            y += 38.0;
        }
    }

    let rows: Vec<TableRow> = data
        .resources
        .iter()
        .take(8)
        .map(|entry| {
            let net = entry.produced - entry.consumed;
            let accent = v9_logistics_signed_color(net);
            TableRow::new(vec![
                TableCell::strong(entry.name.as_str()),
                TableCell::new(format!("{:.0}", entry.stored)).right(),
                TableCell::colored(format!("{:+.0}", net), accent).right(),
            ])
            .accent(accent)
        })
        .collect();
    ui.painter().text(
        Pos2::new(inner.left(), y + spacing::S4),
        Align2::LEFT_TOP,
        "Strategic resources",
        TextRole::Subheading.font_id(),
        palette::BRASS_BRIGHT,
    );
    DataTable::new(
        vec![
            TableColumn::new("Resource", 1.1),
            TableColumn::new("Stored", 0.7).right(),
            TableColumn::new("Net", 0.7).right(),
        ],
        rows,
    )
    .row_height(27.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), y + spacing::S7),
            Pos2::new(inner.right(), inner.bottom() - 56.0),
        ),
    );

    let procurement = format!(
        "Procurement {}  |  Maintenance {}",
        format_rm(data.military_procurement_rm),
        format_rm(data.military_maintenance_rm)
    );
    let galley = ui.painter().layout(
        procurement,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
        inner.width(),
    );
    ui.painter().galley(
        Pos2::new(inner.left(), inner.bottom() - 42.0),
        galley,
        palette::PARCHMENT_DIM,
    );
}

fn v9_logistics_signed_color(value: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if value >= 0.0 {
        palette::GOOD
    } else {
        palette::BAD
    }
}

fn render_summary(ui: &mut egui::Ui, data: &LogisticsData) {
    let net = data.total_daily_production - data.total_daily_need;
    ui.add_space(6.0);
    components::summary_strip(
        ui,
        &[
            ("装备种类", data.total_types.to_string()),
            ("缺口种类", data.deficit_types.to_string()),
            ("军工日产", signed_one_decimal(data.total_daily_production)),
            ("军队需求", format!("-{:.1}/日", data.total_daily_need)),
            ("净变化", signed_one_decimal(net)),
        ],
    );
    ui.add_space(6.0);
}

fn render_status_banner(ui: &mut egui::Ui, data: &LogisticsData) {
    let net = data.total_daily_production - data.total_daily_need;
    let (label, text, color) = if data.deficit_types > 0 {
        (
            "补给缺口",
            format!(
                "{} 类装备存在缺口，优先检查军购分摊和生产来源。",
                data.deficit_types
            ),
            BAD,
        )
    } else if net < 0.0 {
        (
            "库存消耗",
            format!("军队每日净消耗 {:.1}，库存正在被动下降。", -net),
            WARN,
        )
    } else {
        (
            "库存稳定",
            "当前军工日产覆盖军队需求，可继续观察重点装备来源。".to_owned(),
            GOOD,
        )
    };

    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(0x1d, 0x16, 0x10, 230))
        .stroke(egui::Stroke::new(1.0, color))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(label).strong().color(color));
                ui.label(
                    RichText::new(text)
                        .small()
                        .color(Color32::from_rgb(0xe0, 0xd2, 0xa8)),
                );
            });
        });
    ui.add_space(8.0);
}

fn render_equipment_section(ui: &mut egui::Ui, data: &LogisticsData) {
    logistics_card(ui, "装备库存与军工闭环", |ui| {
        ui.columns(2, |columns| {
            metric_tile(
                &mut columns[0],
                "政府军购",
                format_rm(data.military_procurement_rm),
                GOLD_BRIGHT,
            );
            metric_tile(
                &mut columns[1],
                "维护费",
                format_rm(data.military_maintenance_rm),
                MUTED,
            );
        });
        ui.add_space(6.0);

        if data.entries.is_empty() {
            components::empty_state(ui, "暂无装备库存", "生产线与部队需求出现后会在这里汇总。");
            return;
        }

        for entry in &data.entries {
            render_equipment_row(ui, entry);
            ui.add_space(5.0);
        }
    });
}

fn render_equipment_row(ui: &mut egui::Ui, entry: &LogisticsEntry) {
    let accent = if entry.deficit > 0.0 {
        BAD
    } else if entry.net_change < 0.0 {
        WARN
    } else {
        GOOD
    };
    egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, accent))
        .inner_margin(egui::Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&entry.name).strong().color(Color32::WHITE));
                status_pill(ui, format!("库存 {:.0}", entry.stockpile), GOOD);
                status_pill(ui, format!("日产 +{:.1}", entry.daily_production), BLUE);
                status_pill(
                    ui,
                    format!("净 {:+.1}/日", entry.net_change),
                    signed_color(entry.net_change),
                );
            });
            ui.add_space(4.0);
            ui.columns(4, |columns| {
                small_metric(
                    &mut columns[0],
                    "补充",
                    format!("-{:.1}/日", entry.daily_replenishment_need),
                );
                small_metric(
                    &mut columns[1],
                    "训练",
                    format!("-{:.1}/日", entry.daily_training_need),
                );
                small_metric(
                    &mut columns[2],
                    "维护",
                    format!("-{:.1}/日", entry.daily_maintenance_need),
                );
                small_metric(&mut columns[3], "军购", format_rm(entry.procurement_rm));
            });
            if entry.deficit > 0.0 {
                let empty_text = entry
                    .days_until_empty
                    .map(|days| format!("预计 {:.0} 天耗尽", days))
                    .unwrap_or_else(|| "库存已耗尽".to_owned());
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("缺口 {:.1}/日，{}", entry.deficit, empty_text))
                        .small()
                        .color(BAD),
                );
            }
            if !entry.production_sources.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("来源：{}", entry.production_sources.join("、")))
                        .small()
                        .color(MUTED),
                );
            }
        });
}

fn render_resource_section(ui: &mut egui::Ui, data: &LogisticsData) {
    logistics_card(ui, "战略资源速览", |ui| {
        ui.label(
            RichText::new("详细供需与市场流向请在市场面板查看。")
                .small()
                .color(MUTED),
        );
        ui.add_space(6.0);
        if data.resources.is_empty() {
            components::empty_state(
                ui,
                "暂无战略资源库存",
                "资源产出和消耗出现后会在这里显示摘要。",
            );
            return;
        }
        for entry in &data.resources {
            let net = entry.produced - entry.consumed;
            let accent = if net < 0.0 { WARN } else { GOOD };
            egui::Frame::new()
                .fill(PANEL_CARD_SOFT)
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x48, 0x36, 0x24)))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&entry.name).strong().color(Color32::WHITE));
                        status_pill(ui, format!("库存 {:.0}", entry.stored), GOOD);
                        status_pill(ui, format!("产出 +{:.0}/日", entry.produced), BLUE);
                        status_pill(ui, format!("消耗 -{:.0}/日", entry.consumed), WARN);
                        status_pill(ui, format!("净 {:+.0}/日", net), accent);
                    });
                });
            ui.add_space(4.0);
        }
    });
}

fn logistics_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(5.0);
    egui::Frame::new()
        .fill(PANEL_CARD)
        .stroke(egui::Stroke::new(1.0, STROKE_DARK))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(GOLD));
            ui.separator();
            add_contents(ui);
        });
}

fn metric_tile(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x48, 0x36, 0x24)))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.label(RichText::new(label).small().color(MUTED));
            ui.label(RichText::new(value).strong().color(color));
        });
}

fn small_metric(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(RichText::new(label).small().color(MUTED));
    ui.label(
        RichText::new(value)
            .small()
            .color(Color32::from_rgb(0xe0, 0xd2, 0xa8)),
    );
}

fn status_pill(ui: &mut egui::Ui, text: String, color: Color32) {
    ui.label(RichText::new(text).small().strong().color(color));
}

fn signed_color(value: f32) -> Color32 {
    if value >= 0.0 {
        GOOD
    } else {
        BAD
    }
}

fn signed_one_decimal(value: f32) -> String {
    format!("{:+.1}/日", value)
}

fn format_rm(value: f64) -> String {
    if value.abs() >= 1_000_000.0 {
        format!("{:.1}M RM/日", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("{:.1}K RM/日", value / 1_000.0)
    } else {
        format!("{:.0} RM/日", value)
    }
}
