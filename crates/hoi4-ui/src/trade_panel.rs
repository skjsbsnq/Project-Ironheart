//! V6 贸易面板 UI：进出口概览 / 贸易路线 / 封锁状态 / 外汇储备。
//!
//! V6.E 验收要求：面板显示进出口流量、贸易路线状态、封锁警告。

use crate::i18n::tr;
use egui::{Color32, RichText};

fn localized_trade_name(id: &str, fallback: &str) -> String {
    let translated = tr(id);
    if translated != id {
        translated.to_owned()
    } else if fallback.is_empty() {
        id.to_owned()
    } else {
        fallback.to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct TradeRouteEntry {
    pub good_id: String,
    pub good_name: String,
    pub kind: String,
    pub throughput: f32,
    pub is_blockaded: bool,
    pub historical: bool,
    pub partner_tag: String,
    pub affected: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowEntry {
    pub good_id: String,
    pub good_name: String,
    pub imports: f32,
    pub exports: f32,
    pub failure_reason: Option<String>,
    pub import_tariff_rate: f32,
    pub export_tariff_rate: f32,
}

#[derive(Debug, Clone)]
pub struct TradePanelData {
    pub flows: Vec<TradeFlowEntry>,
    pub routes: Vec<TradeRouteEntry>,
    pub reserve_gbp: f64,
    pub exchange_rate: f32,
    pub tariff_income_daily_rm: f64,
    pub trade_balance_daily_gbp: f64,
    pub is_fx_control: bool,
    pub is_blockaded: bool,
    pub current_trade_law: String,
    pub current_trade_law_name: String,
    pub trade_capacity: f32,
    pub trade_capacity_used: f32,
}

pub struct TradePanel;

impl TradePanel {
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &TradePanelData) -> (bool, Vec<()>) {
        return v9_show_trade(ctx, data);

        let mut close = false;

        egui::SidePanel::left("trade_panel")
            .default_width(500.0)
            .min_width(400.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new(tr("v6_trade_panel_title"))
                            .color(Color32::from_rgb(0xe0, 0xc0, 0x78)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            close = true;
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            tr("v6_trade_law"),
                            data.current_trade_law_name
                        ))
                        .color(Color32::from_rgb(0xa0, 0xa0, 0xa0)),
                    );
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("£: {:.1}M", data.reserve_gbp / 1_000_000.0))
                            .color(Color32::from_rgb(0x80, 0xc0, 0x60)),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(format!("RM/£: {:.2}", data.exchange_rate))
                            .color(Color32::from_rgb(0xa0, 0xa0, 0xc0)),
                    );
                    ui.separator();
                    if data.trade_capacity > 0.0 {
                        let cap_pct = if data.trade_capacity > 0.0 {
                            (data.trade_capacity_used / data.trade_capacity * 100.0).min(100.0)
                        } else {
                            0.0
                        };
                        ui.label(
                            RichText::new(format!(
                                "容量: {:.0}/{:.0} ({:.0}%)",
                                data.trade_capacity_used, data.trade_capacity, cap_pct
                            ))
                            .color(if cap_pct > 90.0 {
                                Color32::from_rgb(0xc0, 0x60, 0x60)
                            } else {
                                Color32::from_rgb(0xa0, 0xc0, 0x80)
                            }),
                        );
                        ui.separator();
                    }
                    ui.label(
                        RichText::new(format!(
                            "{}: {:.1}M £",
                            tr("v6_trade_balance"),
                            data.trade_balance_daily_gbp / 1_000_000.0
                        ))
                        .color(if data.trade_balance_daily_gbp >= 0.0 {
                            Color32::from_rgb(0x60, 0xc0, 0x60)
                        } else {
                            Color32::from_rgb(0xc0, 0x60, 0x60)
                        }),
                    );
                    if data.is_fx_control {
                        ui.separator();
                        ui.label(
                            RichText::new(tr("v6_fx_control_active"))
                                .color(Color32::from_rgb(0xc0, 0xa0, 0x30)),
                        );
                    }
                });

                if data.is_blockaded {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            Color32::from_rgb(0xc0, 0x30, 0x30),
                            RichText::new(tr("v6_blockade_warning")).strong(),
                        );
                    });
                    let affected: Vec<String> = data
                        .routes
                        .iter()
                        .filter(|route| route.is_blockaded)
                        .flat_map(|route| route.affected.iter().cloned())
                        .collect();
                    if !affected.is_empty() {
                        ui.label(
                            RichText::new(format!("受影响建筑：{}", affected.join("、")))
                                .small()
                                .color(Color32::from_rgb(0xff, 0xaa, 0x66)),
                        );
                    }
                } else if data.is_fx_control {
                    ui.label(
                        RichText::new(
                            "进口限制原因：当前贸易法启用外汇管制，进口会受外汇储备和法律约束。",
                        )
                        .small()
                        .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
                    );
                }

                ui.separator();
                ui.collapsing(tr("v6_trade_flows"), |ui| {
                    egui::Grid::new("trade_flows_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new(tr("v6_good")).strong());
                            ui.label(RichText::new(tr("v6_imports")).strong());
                            ui.label(RichText::new(tr("v6_exports")).strong());
                            ui.label(RichText::new("失败原因").strong());
                            ui.label(RichText::new(tr("v6_import_tariff")).strong());
                            ui.label(RichText::new(tr("v6_export_tariff")).strong());
                            ui.end_row();

                            for flow in &data.flows {
                                if flow.imports.abs() < 0.001
                                    && flow.exports.abs() < 0.001
                                    && flow.failure_reason.is_none()
                                {
                                    continue;
                                }
                                ui.label(localized_trade_name(&flow.good_id, &flow.good_name));
                                ui.label(format!("{:.1}", flow.imports));
                                ui.label(format!("{:.1}", flow.exports));
                                ui.label(
                                    flow.failure_reason
                                        .as_deref()
                                        .map(trade_failure_label)
                                        .unwrap_or("-"),
                                );
                                ui.label(format!("{:.0}%", flow.import_tariff_rate * 100.0));
                                ui.label(format!("{:.0}%", flow.export_tariff_rate * 100.0));
                                ui.end_row();
                            }
                        });
                });

                ui.collapsing(tr("v6_trade_routes"), |ui| {
                    egui::Grid::new("trade_routes_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new(tr("v6_good")).strong());
                            ui.label(RichText::new(tr("v6_route_type")).strong());
                            ui.label(RichText::new(tr("v6_throughput")).strong());
                            ui.label(RichText::new(tr("v6_status")).strong());
                            ui.end_row();

                            for route in &data.routes {
                                ui.label(localized_trade_name(&route.good_id, &route.good_name));
                                let route_label = if route.historical {
                                    format!("{}：{} · 历史", route.kind, route.partner_tag)
                                } else {
                                    format!("{}：{}", route.kind, route.partner_tag)
                                };
                                ui.label(route_label);
                                ui.label(format!("{:.1}", route.throughput));
                                if route.is_blockaded {
                                    ui.colored_label(
                                        Color32::from_rgb(0xc0, 0x30, 0x30),
                                        tr("v6_blockaded"),
                                    );
                                } else {
                                    ui.colored_label(
                                        Color32::from_rgb(0x60, 0xc0, 0x60),
                                        tr("v6_active"),
                                    );
                                }
                                ui.end_row();
                                if route.is_blockaded && !route.affected.is_empty() {
                                    ui.label("");
                                    ui.label(
                                        RichText::new("影响")
                                            .small()
                                            .color(Color32::from_rgb(0xff, 0xaa, 0x66)),
                                    );
                                    ui.label(
                                        RichText::new(route.affected.join("、"))
                                            .small()
                                            .color(Color32::from_gray(180)),
                                    );
                                    ui.label("");
                                    ui.end_row();
                                } else if route.is_blockaded {
                                    ui.label("");
                                    ui.label(
                                        RichText::new("失败原因")
                                            .small()
                                            .color(Color32::from_rgb(0xff, 0xaa, 0x66)),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "贸易路线被封锁，进口/出口吞吐无法稳定传导到市场。",
                                        )
                                        .small()
                                        .color(Color32::from_gray(180)),
                                    );
                                    ui.label("");
                                    ui.end_row();
                                } else {
                                    ui.label("");
                                    ui.label(
                                        RichText::new("影响").small().color(Color32::LIGHT_GRAY),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "路线正常，吞吐会进入市场进出口并影响外汇收支。 ",
                                        )
                                        .small()
                                        .color(Color32::from_gray(180)),
                                    );
                                    ui.label("");
                                    ui.end_row();
                                }
                            }
                        });
                });
            });

        (close, Vec::new())
    }
}

fn v9_show_trade(ctx: &egui::Context, data: &TradePanelData) -> (bool, Vec<()>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;
    let cap = if data.trade_capacity > 0.0 {
        (data.trade_capacity_used / data.trade_capacity).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let accent = if data.is_blockaded {
        palette::BAD
    } else if data.is_fx_control || cap > 0.9 {
        palette::WARN
    } else {
        palette::INFO
    };
    let (close, _) = PanelShell::new("trade_panel_v9", tr("v6_trade_panel_title"))
        .subtitle(&data.current_trade_law_name)
        .class(PanelClass::Economy)
        .accent(accent)
        .footer("Q Close  |  Route list / Impact table")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("v6_finance_reserve_gbp"),
                        format!("£ {:.1}M", data.reserve_gbp / 1_000_000.0),
                        palette::GOOD,
                    ),
                    (
                        tr("v6_exchange_rate"),
                        format!("{:.2} RM/£", data.exchange_rate),
                        palette::INFO,
                    ),
                    (
                        tr("v6_trade_balance"),
                        format!("{:+.1}M £/d", data.trade_balance_daily_gbp / 1_000_000.0),
                        if data.trade_balance_daily_gbp >= 0.0 {
                            palette::GOOD
                        } else {
                            palette::BAD
                        },
                    ),
                    (
                        "Capacity",
                        if data.trade_capacity > 0.0 {
                            format!("{:.0}/{:.0}", data.trade_capacity_used, data.trade_capacity)
                        } else {
                            "-".to_owned()
                        },
                        if cap > 0.9 {
                            palette::WARN
                        } else {
                            palette::GOLD
                        },
                    ),
                    (
                        tr("v6_status"),
                        if data.is_blockaded {
                            tr("v6_blockaded").to_owned()
                        } else if data.is_fx_control {
                            tr("v6_fx_control_active").to_owned()
                        } else {
                            tr("v6_active").to_owned()
                        },
                        accent,
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "Routes / Trade flows / Impact preview",
                accent,
            );
            v9_trade_body(ui, layout.body, data, cap);
        });
    (close, Vec::new())
}

fn v9_trade_body(ui: &mut egui::Ui, rect: egui::Rect, data: &TradePanelData, cap: f32) {
    use crate::v9::{
        layout::{GridLayout, Track},
        tokens::spacing,
    };

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.42), Track::Fr(0.58)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_trade_routes(ui, GridLayout::cell(&cells, 0, 0), data, cap);
        v9_trade_impact(ui, GridLayout::cell(&cells, 0, 1), data);
    });
}

fn v9_trade_routes(ui: &mut egui::Ui, rect: egui::Rect, data: &TradePanelData, cap: f32) {
    use crate::v9::{
        paint,
        primitives::{Card, Pill, PillTone},
        tokens::{palette, spacing, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        tr("v6_trade_routes"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let status_rect = Rect::from_min_size(
        Pos2::new(inner.right() - 136.0, inner.top()),
        Vec2::new(132.0, 22.0),
    );
    Pill::new(if data.is_blockaded {
        tr("v6_blockaded")
    } else if data.is_fx_control {
        tr("v6_fx_control_active")
    } else {
        tr("v6_active")
    })
    .tone(if data.is_blockaded {
        PillTone::Bad
    } else if data.is_fx_control || cap > 0.9 {
        PillTone::Warn
    } else {
        PillTone::Good
    })
    .show_at(ui, status_rect);

    let list_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    ui.allocate_ui_at_rect(list_rect, |ui| {
        ui.set_min_size(list_rect.size());
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if data.routes.is_empty() {
                    crate::v9::composites::panel_shell::draw_empty_state(
                        ui,
                        ui.available_rect_before_wrap(),
                        "No routes",
                        "No active trade route is registered.",
                    );
                    return;
                }
                for route in &data.routes {
                    let (row_rect, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 72.0), Sense::hover());
                    let accent = if route.is_blockaded {
                        palette::BAD
                    } else if route.historical {
                        palette::GOLD
                    } else {
                        palette::INFO
                    };
                    paint::paint_bevel(
                        ui.painter(),
                        row_rect,
                        palette::SOOT_BLACK,
                        palette::EDGE_DARK,
                        2.0,
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_size(row_rect.left_top(), Vec2::new(4.0, row_rect.height())),
                        egui::epaint::CornerRadius::ZERO,
                        accent,
                    );
                    let x = row_rect.left() + spacing::S5;
                    ui.painter().text(
                        Pos2::new(x, row_rect.top() + spacing::S3),
                        Align2::LEFT_TOP,
                        localized_trade_name(&route.good_id, &route.good_name),
                        TextRole::Subheading.font_id(),
                        palette::GOLD_HOT,
                    );
                    ui.painter().text(
                        Pos2::new(x, row_rect.top() + 30.0),
                        Align2::LEFT_TOP,
                        format!(
                            "{} -> {}  {:.1}/d",
                            route.kind, route.partner_tag, route.throughput
                        ),
                        TextRole::Body.font_id(),
                        palette::PARCHMENT_DIM,
                    );
                    let state = if route.is_blockaded {
                        tr("v6_blockaded")
                    } else {
                        tr("v6_active")
                    };
                    ui.painter().text(
                        Pos2::new(row_rect.right() - spacing::S5, row_rect.center().y),
                        Align2::RIGHT_CENTER,
                        state,
                        TextRole::Caption.font_id(),
                        accent,
                    );
                    ui.add_space(spacing::S3);
                }
            });
    });
}

fn v9_trade_impact(ui: &mut egui::Ui, rect: egui::Rect, data: &TradePanelData) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{Card, DataTable, TableCell, TableColumn, TableRow},
        tokens::{palette, spacing, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Vec2};

    let grid = GridLayout::new(vec![Track::Fr(0.60), Track::Fr(0.40)], vec![Track::Fr(1.0)])
        .with_gutter(0.0, spacing::S5);
    let cells = grid.measure(rect);
    let flow_rect = GridLayout::cell(&cells, 0, 0);
    let side_rect = GridLayout::cell(&cells, 1, 0);

    let flow_inner = Card::new().as_panel().show_at(ui, flow_rect);
    ui.painter().text(
        flow_inner.left_top(),
        Align2::LEFT_TOP,
        tr("v6_trade_flows"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let rows: Vec<TableRow> = data
        .flows
        .iter()
        .filter(|flow| {
            flow.imports.abs() >= 0.001
                || flow.exports.abs() >= 0.001
                || flow.failure_reason.is_some()
        })
        .take(14)
        .map(|flow| {
            let accent = if flow.failure_reason.is_some() {
                palette::WARN
            } else if flow.exports >= flow.imports {
                palette::GOOD
            } else {
                palette::INFO
            };
            TableRow::new(vec![
                TableCell::strong(localized_trade_name(&flow.good_id, &flow.good_name)),
                TableCell::new(format!("{:.1}", flow.imports)).right(),
                TableCell::new(format!("{:.1}", flow.exports)).right(),
                TableCell::colored(
                    flow.failure_reason
                        .as_deref()
                        .map(trade_failure_label)
                        .unwrap_or("-"),
                    if flow.failure_reason.is_some() {
                        palette::WARN
                    } else {
                        palette::MUTED
                    },
                ),
                TableCell::new(format!("{:.0}%", flow.import_tariff_rate * 100.0)).right(),
                TableCell::new(format!("{:.0}%", flow.export_tariff_rate * 100.0)).right(),
            ])
            .accent(accent)
        })
        .collect();
    DataTable::new(
        vec![
            TableColumn::new(tr("v6_good"), 1.4),
            TableColumn::new(tr("v6_imports"), 0.7).right(),
            TableColumn::new(tr("v6_exports"), 0.7).right(),
            TableColumn::new(tr("v6_status"), 1.2),
            TableColumn::new(tr("v6_import_tariff"), 0.8).right(),
            TableColumn::new(tr("v6_export_tariff"), 0.8).right(),
        ],
        rows,
    )
    .row_height(28.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(flow_inner.left(), flow_inner.top() + 34.0),
            flow_inner.right_bottom(),
        ),
    );

    let side_inner = Card::new().as_panel().show_at(ui, side_rect);
    ui.painter().text(
        side_inner.left_top(),
        Align2::LEFT_TOP,
        "Impact preview",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let mut y = side_inner.top() + 34.0;
    if data.is_blockaded {
        ui.painter().text(
            Pos2::new(side_inner.left(), y),
            Align2::LEFT_TOP,
            tr("v6_blockade_warning"),
            TextRole::Subheading.font_id(),
            palette::BAD,
        );
        y += 26.0;
    } else if data.is_fx_control {
        ui.painter().text(
            Pos2::new(side_inner.left(), y),
            Align2::LEFT_TOP,
            tr("v6_fx_control_active"),
            TextRole::Subheading.font_id(),
            palette::WARN,
        );
        y += 26.0;
    }

    let affected: Vec<String> = data
        .routes
        .iter()
        .filter(|route| route.is_blockaded)
        .flat_map(|route| route.affected.iter().cloned())
        .take(8)
        .collect();
    let body = if affected.is_empty() {
        format!(
            "{} {:.1}M £/d\nTariff income {:.1} RM/d",
            tr("v6_trade_balance"),
            data.trade_balance_daily_gbp / 1_000_000.0,
            data.tariff_income_daily_rm
        )
    } else {
        format!("Affected: {}", affected.join(", "))
    };
    let galley = ui.painter().layout(
        body,
        TextRole::Body.font_id(),
        palette::PARCHMENT_DIM,
        side_inner.width(),
    );
    ui.painter().galley(
        Pos2::new(side_inner.left(), y),
        galley,
        palette::PARCHMENT_DIM,
    );

    let mini = Rect::from_min_size(
        Pos2::new(side_inner.left(), side_inner.bottom() - 96.0),
        Vec2::new(side_inner.width(), 88.0),
    );
    let rows = vec![
        TableRow::new(vec![
            TableCell::strong(tr("v6_trade_law")),
            TableCell::colored(&data.current_trade_law_name, palette::GOLD),
        ]),
        TableRow::new(vec![
            TableCell::strong("Routes"),
            TableCell::new(data.routes.len().to_string()).right(),
        ]),
    ];
    DataTable::new(
        vec![
            TableColumn::new("Metric", 1.0),
            TableColumn::new("Value", 1.0).right(),
        ],
        rows,
    )
    .row_height(28.0)
    .show_at(ui, mini);
}

fn trade_failure_label(reason: &str) -> &'static str {
    // pipe | test
    match reason {
        "insufficient_foreign_exchange" => "外汇不足",
        "partial_due_to_foreign_exchange" => "外汇不足（部分）",
        "blockaded" => "封锁",
        "route_capacity" => "路线吞吐不足",
        "trade_law_restricted" => "贸易法限制",
        _ => "未知",
    }
}
