//! V6 财政面板 UI：现金 RM / 储备 £ / 汇率 / 收入 / 支出 / 债务 / MEFO / 信用评级。
//!
//! V6.C 验收要求：财政面板可操作，能发行债券/印MEFO/卖金。
//!
//! V7 视觉重做：Hero 金库头 + 5-tab 布局（总览/预算/债务/外汇/操作），
//! HoI4 vanilla 金棕边框 + Vic3 报表内排版。

use crate::{components, i18n::tr};
use egui::{Color32, RichText};

// 调色板 / 视觉 helpers 全部从 components 共享。
use components::{
    BAD, BLUE, BRONZE, GOLD, GOLD_BRIGHT, GOLD_DIM, GOOD, HERO_FILL, MUTED, PANEL_CARD_DEEP,
    PANEL_CARD_SOFT, PARCHMENT, STROKE_TILE, WARN,
};

// ─── 数据类型（保持原 API 不变） ─────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct BudgetBreakdownData {
    pub income_taxes_rm: f64,
    pub income_state_profit_rm: f64,
    pub income_domestic_bonds_rm: f64,
    pub income_other_rm: f64,
    pub expense_state_payroll_rm: f64,
    pub expense_military_wages_rm: f64,
    pub expense_military_procurement_rm: f64,
    pub expense_military_maintenance_rm: f64,
    pub expense_construction_goods_rm: f64,
    pub expense_construction_wages_rm: f64,
    pub expense_welfare_rm: f64,
    pub expense_debt_interest_rm: f64,
    pub expense_foreign_currency_rm: f64,
    pub expense_mefo_forced_payment_rm: f64,
    pub expense_research_rm: f64,
    pub expense_other_rm: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FinancingBreakdownData {
    pub mefo_issued_rm: f64,
    pub mefo_interest_capitalized_rm: f64,
    pub domestic_bond_issued_rm: f64,
}

impl BudgetBreakdownData {
    pub fn operating_income_rm(&self) -> f64 {
        self.income_taxes_rm + self.income_state_profit_rm + self.income_other_rm
    }

    pub fn total_income_rm(&self) -> f64 {
        self.operating_income_rm() + self.income_domestic_bonds_rm
    }

    pub fn total_expense_rm(&self) -> f64 {
        self.expense_state_payroll_rm
            + self.expense_military_wages_rm
            + self.expense_military_procurement_rm
            + self.expense_military_maintenance_rm
            + self.expense_construction_goods_rm
            + self.expense_construction_wages_rm
            + self.expense_welfare_rm
            + self.expense_debt_interest_rm
            + self.expense_foreign_currency_rm
            + self.expense_mefo_forced_payment_rm
            + self.expense_research_rm
            + self.expense_other_rm
    }
}

#[derive(Debug, Clone)]
pub struct FinancePanelData {
    pub cash_rm: f64,
    pub reserve_gbp: f64,
    pub gold_kg: f64,
    pub daily_income_rm: f64,
    pub daily_expense_rm: f64,
    pub budget_breakdown: BudgetBreakdownData,
    pub financing_breakdown: FinancingBreakdownData,
    pub operating_income_rm: f64,
    pub operating_expense_rm: f64,
    pub original_deficit_rm: f64,
    pub mefo_coverage_rm: f64,
    pub post_financing_cash_change_rm: f64,
    pub public_debt_gbp: f64,
    pub public_debt_rm: f64,
    pub mefo_debt_rm: f64,
    pub mefo_military_budget_rm: f64,
    pub mefo_military_spent_rm: f64,
    pub credit_rating: String,
    pub bond_interest_rate: f32,
    pub gdp_rm: f64,
    pub gdp_gbp: f64,
    pub domestic_gdp_rm: f64,
    pub domestic_gdp_gbp: f64,
    pub colonial_gdp_rm: f64,
    pub colonial_gdp_gbp: f64,
    pub colonial_extracted_value_rm: f64,
    pub colonial_extracted_value_gbp: f64,
    pub exchange_rate_rm_per_gbp: f32,
    pub can_print_mefo: bool,
    pub can_issue_foreign_bond: bool,
    pub is_foreign_exchange_control: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FinanceCommand {
    IssueDomesticBond { amount_rm: f64 },
    IssueForeignBond { amount_gbp: f64 },
    PrintMefo,
    SellGold { kg: f64 },
    BuyForeignCurrency { gbp_amount: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinancePanelTab {
    Overview,
    Budget,
    Debt,
    Exchange,
    Actions,
}

pub struct FinancePanel;

impl FinancePanel {
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &FinancePanelData) -> (bool, Vec<FinanceCommand>) {
        return v9_show_finance(ctx, data);

        let mut close = false;
        let mut cmds: Vec<FinanceCommand> = Vec::new();
        let tab_id = egui::Id::new("finance_panel_tab_v7");
        let mut tab = ctx
            .data_mut(|d| d.get_persisted::<FinancePanelTab>(tab_id))
            .unwrap_or(FinancePanelTab::Overview);

        egui::SidePanel::left("finance_panel")
            .default_width(560.0)
            .min_width(480.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(0x12, 0x13, 0x12))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x32, 0x27)))
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 6,
                        bottom: 8,
                    }),
            )
            .show(ctx, |ui| {
                components::panel_header(ui, tr("v6_finance_panel_title"), &mut close);
                ui.add_space(4.0);
                render_hero_treasury(ui, data);
                ui.add_space(8.0);
                render_tab_bar(ui, &mut tab);
                ui.add_space(2.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match tab {
                        FinancePanelTab::Overview => render_overview_tab(ui, data),
                        FinancePanelTab::Budget => render_budget_tab(ui, data),
                        FinancePanelTab::Debt => render_debt_tab(ui, data, &mut cmds),
                        FinancePanelTab::Exchange => render_exchange_tab(ui, data),
                        FinancePanelTab::Actions => render_actions_tab(ui, data, &mut cmds),
                    });
            });

        ctx.data_mut(|d| d.insert_persisted(tab_id, tab));
        (close, cmds)
    }
}

// ─── Hero Treasury 头部 ─────────────────────────────────────────

fn v9_show_finance(ctx: &egui::Context, data: &FinancePanelData) -> (bool, Vec<FinanceCommand>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let daily_balance = data.daily_income_rm - data.daily_expense_rm;
    let debt_ratio = if data.gdp_rm > 0.0 {
        (data.public_debt_rm + data.mefo_debt_rm) / data.gdp_rm
    } else {
        0.0
    };
    let mefo_ratio = if data.gdp_rm > 0.0 {
        data.mefo_debt_rm / data.gdp_rm
    } else {
        0.0
    };
    let accent = if daily_balance < 0.0 || mefo_ratio > 0.25 {
        palette::WARN
    } else {
        palette::GOLD
    };

    let (close, output) = PanelShell::new("finance_panel_v9", tr("v6_finance_panel_title"))
        .subtitle("Treasury / budget / debt instruments")
        .class(PanelClass::Economy)
        .accent(accent)
        .footer("Q Close  |  Budget table / Financing actions")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("v6_finance_cash_rm"),
                        format_million(data.cash_rm),
                        if daily_balance >= 0.0 {
                            palette::GOOD
                        } else {
                            palette::WARN
                        },
                    ),
                    (
                        "Daily net",
                        signed_million(daily_balance),
                        v9_finance_balance_color(daily_balance),
                    ),
                    (
                        tr("v6_finance_reserve_gbp"),
                        format!("GBP {}", format_million(data.reserve_gbp)),
                        palette::INFO,
                    ),
                    (
                        "Debt/GDP",
                        format!("{:.1}%", debt_ratio * 100.0),
                        v9_finance_debt_color(debt_ratio),
                    ),
                    (
                        "MEFO/GDP",
                        format!("{:.1}%", mefo_ratio * 100.0),
                        v9_finance_mefo_color(mefo_ratio),
                    ),
                    (
                        tr("v6_finance_credit_rating"),
                        data.credit_rating.clone(),
                        v9_rating_color(&data.credit_rating),
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "Tile grid / Income-expense table / Actions",
                accent,
            );
            let mut cmds = Vec::new();
            v9_finance_body(ui, layout.body, data, debt_ratio, mefo_ratio, &mut cmds);
            cmds
        });

    (close, output.unwrap_or_default())
}

fn v9_finance_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &FinancePanelData,
    debt_ratio: f64,
    mefo_ratio: f64,
    cmds: &mut Vec<FinanceCommand>,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(
            vec![Track::Fr(1.0)],
            vec![Track::Fr(0.34), Track::Fr(0.42), Track::Fr(0.24)],
        )
        .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        v9_finance_tile_grid(
            ui,
            GridLayout::cell(&cells, 0, 0),
            data,
            debt_ratio,
            mefo_ratio,
        );
        v9_finance_budget_table(ui, GridLayout::cell(&cells, 0, 1), data);
        v9_finance_action_panel(ui, GridLayout::cell(&cells, 0, 2), data, mefo_ratio, cmds);
    });
}

fn v9_finance_tile_grid(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &FinancePanelData,
    debt_ratio: f64,
    mefo_ratio: f64,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{draw_progress_bar, Card, Tile, TileTrend},
        tokens::{palette, spacing, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Treasury",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let tile_area = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        Pos2::new(inner.right(), inner.bottom() - 72.0),
    );
    let grid = GridLayout::new(
        vec![
            Track::Fixed(52.0),
            Track::Fixed(52.0),
            Track::Fixed(52.0),
            Track::Fixed(52.0),
        ],
        vec![Track::Fr(1.0), Track::Fr(1.0)],
    )
    .with_gutter(spacing::S4, spacing::S4);
    let cells = grid.measure(tile_area);
    let cash_days = if data.daily_expense_rm > 0.0 {
        data.cash_rm / data.daily_expense_rm
    } else {
        0.0
    };
    let tiles = [
        (
            "Reserve",
            format!("GBP {}", format_million(data.reserve_gbp)),
            palette::INFO,
            TileTrend::None,
        ),
        (
            "Gold",
            format!("{:.0} kg", data.gold_kg),
            palette::GOLD,
            TileTrend::None,
        ),
        (
            "Rating",
            data.credit_rating.clone(),
            v9_rating_color(&data.credit_rating),
            TileTrend::None,
        ),
        (
            "Exchange",
            format!("{:.2}", data.exchange_rate_rm_per_gbp),
            palette::INFO,
            TileTrend::None,
        ),
        (
            "GDP",
            format_million(data.gdp_rm),
            palette::GOLD,
            TileTrend::None,
        ),
        (
            "Cash cover",
            format!("{:.0}d", cash_days),
            v9_finance_cover_color(cash_days),
            TileTrend::None,
        ),
        (
            "Operating",
            signed_million(data.operating_income_rm - data.operating_expense_rm),
            v9_finance_balance_color(data.operating_income_rm - data.operating_expense_rm),
            if data.operating_income_rm >= data.operating_expense_rm {
                TileTrend::Up
            } else {
                TileTrend::Down
            },
        ),
        (
            "Post finance",
            signed_million(data.post_financing_cash_change_rm),
            v9_finance_balance_color(data.post_financing_cash_change_rm),
            if data.post_financing_cash_change_rm >= 0.0 {
                TileTrend::Up
            } else {
                TileTrend::Down
            },
        ),
    ];
    for (idx, (label, value, color, trend)) in tiles.iter().enumerate() {
        Tile::new(label, value)
            .accent(*color)
            .trend(*trend)
            .show_at(ui, GridLayout::cell(&cells, idx / 2, idx % 2));
    }

    let debt_bar = Rect::from_min_size(
        Pos2::new(inner.left(), inner.bottom() - 54.0),
        Vec2::new(inner.width(), 10.0),
    );
    draw_progress_bar(
        ui,
        debt_bar,
        debt_ratio as f32,
        v9_finance_debt_color(debt_ratio),
    );
    ui.painter().text(
        Pos2::new(inner.left(), debt_bar.bottom() + 6.0),
        Align2::LEFT_TOP,
        format!(
            "Debt {} RM / MEFO {} RM",
            format_million(data.public_debt_rm),
            format_million(data.mefo_debt_rm)
        ),
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );
    let mefo_bar = Rect::from_min_size(
        Pos2::new(inner.left(), inner.bottom() - 24.0),
        Vec2::new(inner.width(), 10.0),
    );
    draw_progress_bar(
        ui,
        mefo_bar,
        mefo_ratio as f32,
        v9_finance_mefo_color(mefo_ratio),
    );
}

fn v9_finance_budget_table(ui: &mut egui::Ui, rect: egui::Rect, data: &FinancePanelData) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        tr("v6_finance_budget"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let mut rows = Vec::new();
    let income_items: [(&str, f64); 4] = [
        (
            tr("v6_budget_income_taxes"),
            data.budget_breakdown.income_taxes_rm,
        ),
        (
            tr("v6_budget_income_state_profit"),
            data.budget_breakdown.income_state_profit_rm,
        ),
        (
            tr("v6_budget_income_domestic_bonds"),
            data.budget_breakdown.income_domestic_bonds_rm,
        ),
        (
            tr("v6_budget_income_other"),
            data.budget_breakdown.income_other_rm,
        ),
    ];
    for (label, amount) in income_items {
        rows.push(
            TableRow::new(vec![
                TableCell::colored("Income", palette::GOOD),
                TableCell::strong(label),
                TableCell::colored(format_million(amount), palette::GOOD).right(),
            ])
            .accent(palette::GOOD),
        );
    }

    let expense_items: [(&str, f64); 12] = [
        (
            tr("v6_budget_expense_state_payroll"),
            data.budget_breakdown.expense_state_payroll_rm,
        ),
        (
            tr("v6_budget_expense_military_wages"),
            data.budget_breakdown.expense_military_wages_rm,
        ),
        (
            tr("v6_budget_expense_military_procurement"),
            data.budget_breakdown.expense_military_procurement_rm,
        ),
        (
            tr("v6_budget_expense_military_maintenance"),
            data.budget_breakdown.expense_military_maintenance_rm,
        ),
        (
            tr("v6_budget_expense_construction_goods"),
            data.budget_breakdown.expense_construction_goods_rm,
        ),
        (
            tr("v6_budget_expense_construction_wages"),
            data.budget_breakdown.expense_construction_wages_rm,
        ),
        (
            tr("v6_budget_expense_welfare"),
            data.budget_breakdown.expense_welfare_rm,
        ),
        (
            tr("v6_budget_expense_debt_interest"),
            data.budget_breakdown.expense_debt_interest_rm,
        ),
        (
            tr("v6_budget_expense_forex"),
            data.budget_breakdown.expense_foreign_currency_rm,
        ),
        (
            tr("v6_budget_expense_mefo_forced"),
            data.budget_breakdown.expense_mefo_forced_payment_rm,
        ),
        ("Research", data.budget_breakdown.expense_research_rm),
        (
            tr("v6_budget_expense_other"),
            data.budget_breakdown.expense_other_rm,
        ),
    ];
    for (label, amount) in expense_items {
        rows.push(
            TableRow::new(vec![
                TableCell::colored("Expense", palette::BAD),
                TableCell::strong(label),
                TableCell::colored(format_million(amount), palette::BAD).right(),
            ])
            .accent(palette::BAD),
        );
    }
    rows.push(
        TableRow::new(vec![
            TableCell::colored(
                "Net",
                v9_finance_balance_color(data.daily_income_rm - data.daily_expense_rm),
            ),
            TableCell::strong("Daily balance"),
            TableCell::colored(
                signed_million(data.daily_income_rm - data.daily_expense_rm),
                v9_finance_balance_color(data.daily_income_rm - data.daily_expense_rm),
            )
            .right(),
        ])
        .accent(v9_finance_balance_color(
            data.daily_income_rm - data.daily_expense_rm,
        )),
    );

    DataTable::new(
        vec![
            TableColumn::new("Side", 0.65),
            TableColumn::new("Line item", 1.55),
            TableColumn::new("RM/day", 0.85).right(),
        ],
        rows,
    )
    .row_height(27.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 34.0),
            inner.right_bottom(),
        ),
    );
}

fn v9_finance_action_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &FinancePanelData,
    mefo_ratio: f64,
    cmds: &mut Vec<FinanceCommand>,
) {
    use crate::v9::primitives::{
        Button, ButtonSize, ButtonVariant, Card, DataTable, TableCell, TableColumn, TableRow,
    };
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        tr("v6_finance_actions"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let deficit = data.daily_expense_rm - data.daily_income_rm;
    let mefo_can = data.can_print_mefo && deficit > 0.0 && mefo_ratio < 0.30;
    let actions = [
        (
            tr("v6_finance_issue_domestic_bond"),
            true,
            ButtonVariant::Primary,
            FinanceCommand::IssueDomesticBond {
                amount_rm: 500_000_000.0,
            },
        ),
        (
            tr("v6_finance_issue_foreign_bond"),
            data.can_issue_foreign_bond,
            ButtonVariant::Secondary,
            FinanceCommand::IssueForeignBond {
                amount_gbp: 100_000_000.0,
            },
        ),
        (
            tr("v6_finance_print_mefo"),
            mefo_can,
            ButtonVariant::Primary,
            FinanceCommand::PrintMefo,
        ),
        (
            tr("v6_finance_sell_gold"),
            data.gold_kg > 0.0,
            ButtonVariant::Secondary,
            FinanceCommand::SellGold {
                kg: data.gold_kg * 0.1,
            },
        ),
        (
            tr("v6_finance_buy_gbp"),
            !data.is_foreign_exchange_control,
            ButtonVariant::Secondary,
            FinanceCommand::BuyForeignCurrency {
                gbp_amount: 10_000_000.0,
            },
        ),
    ];

    let mut y = inner.top() + 36.0;
    for (label, enabled, variant, cmd) in actions {
        let button_rect = Rect::from_min_size(
            Pos2::new(inner.left(), y),
            Vec2::new(inner.width(), ButtonSize::Md.min_size().y),
        );
        if enabled
            && Button::new(label)
                .size(ButtonSize::Md)
                .variant(variant)
                .enabled(enabled)
                .show_at(ui, button_rect)
                .clicked()
        {
            cmds.push(cmd);
        } else if !enabled {
            Button::new(label)
                .size(ButtonSize::Md)
                .variant(ButtonVariant::Ghost)
                .enabled(false)
                .show_at(ui, button_rect);
        }
        y += ButtonSize::Md.min_size().y + spacing::S3;
    }

    let table_top = y + spacing::S3;
    let rows = vec![
        TableRow::new(vec![
            TableCell::strong("MEFO issued"),
            TableCell::new(format_million(data.financing_breakdown.mefo_issued_rm)).right(),
        ]),
        TableRow::new(vec![
            TableCell::strong("MEFO interest"),
            TableCell::new(format_million(
                data.financing_breakdown.mefo_interest_capitalized_rm,
            ))
            .right(),
        ]),
        TableRow::new(vec![
            TableCell::strong("Domestic bonds"),
            TableCell::new(format_million(
                data.financing_breakdown.domestic_bond_issued_rm,
            ))
            .right(),
        ]),
        TableRow::new(vec![
            TableCell::strong("MEFO cover"),
            TableCell::new(format_million(data.mefo_coverage_rm)).right(),
        ]),
    ];
    DataTable::new(
        vec![
            TableColumn::new("Instrument", 1.2),
            TableColumn::new("Value", 0.9).right(),
        ],
        rows,
    )
    .row_height(27.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), table_top),
            Pos2::new(inner.right(), inner.bottom() - 64.0),
        ),
    );

    let status = if data.is_foreign_exchange_control {
        tr("v6_finance_forex_control").to_owned()
    } else if deficit > 0.0 {
        format!("Daily deficit {}", format_million(deficit))
    } else {
        "Treasury flow is positive.".to_owned()
    };
    let status_color = if deficit > 0.0 {
        palette::WARN
    } else {
        palette::PARCHMENT_DIM
    };
    let galley = ui.painter().layout(
        status,
        TextRole::Caption.font_id(),
        status_color,
        inner.width(),
    );
    ui.painter().galley(
        Pos2::new(inner.left(), inner.bottom() - 48.0),
        galley,
        status_color,
    );
}

fn render_hero_treasury(ui: &mut egui::Ui, data: &FinancePanelData) {
    let inner = egui::Frame::new()
        .fill(HERO_FILL)
        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(0x58, 0x4a, 0x36)))
        .inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 12,
            bottom: 13,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(tr("v6_finance_cash_rm"))
                        .size(12.0)
                        .color(PARCHMENT)
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format_million(data.cash_rm))
                        .strong()
                        .size(30.0)
                        .color(GOOD),
                );
                let balance = data.daily_income_rm - data.daily_expense_rm;
                let (sym, color) = if balance >= 0.0 {
                    ("▲", GOOD)
                } else {
                    ("▼", BAD)
                };
                ui.label(
                    RichText::new(format!("{}  {} / 日", sym, signed_million(balance)))
                        .size(14.0)
                        .color(color)
                        .strong(),
                );
            });
            ui.add_space(8.0);
            components::ornament_divider(ui);
            ui.add_space(6.0);
            ui.columns(3, |c| {
                components::hero_kpi(
                    &mut c[0],
                    "外汇储备",
                    format!("£ {}", format_million(data.reserve_gbp)),
                    BLUE,
                );
                components::hero_kpi(
                    &mut c[1],
                    "黄金储备",
                    format!("{:.0} kg", data.gold_kg),
                    GOLD_BRIGHT,
                );
                components::hero_kpi(
                    &mut c[2],
                    "信用评级",
                    data.credit_rating.clone(),
                    rating_color(&data.credit_rating),
                );
            });
        });
    let rect = inner.response.rect;
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        0.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(180)),
        egui::StrokeKind::Inside,
    );
    ui.painter().hline(
        (rect.left() + 4.0)..=(rect.right() - 4.0),
        rect.top() + 2.0,
        egui::Stroke::new(1.0, GOLD_DIM),
    );
}

// ─── Tab Bar ───────────────────────────────────────────────────

fn render_tab_bar(ui: &mut egui::Ui, tab: &mut FinancePanelTab) {
    egui::Frame::new()
        .fill(PANEL_CARD_DEEP)
        .stroke(egui::Stroke::new(1.0, BRONZE))
        .inner_margin(egui::Margin::symmetric(4, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                tab_select(ui, tab, FinancePanelTab::Overview, "总览");
                tab_select(ui, tab, FinancePanelTab::Budget, "预算");
                tab_select(ui, tab, FinancePanelTab::Debt, "债务");
                tab_select(ui, tab, FinancePanelTab::Exchange, "外汇");
                tab_select(ui, tab, FinancePanelTab::Actions, "操作");
            });
        });
}

fn tab_select(ui: &mut egui::Ui, tab: &mut FinancePanelTab, target: FinancePanelTab, label: &str) {
    if components::tab_chip(ui, *tab == target, label) {
        *tab = target;
    }
}

// ─── Tab 内容渲染 ───────────────────────────────────────────────

fn render_overview_tab(ui: &mut egui::Ui, data: &FinancePanelData) {
    let balance = data.daily_income_rm - data.daily_expense_rm;
    let mefo_ratio = if data.gdp_rm > 0.0 {
        data.mefo_debt_rm / data.gdp_rm
    } else {
        0.0
    };
    let (label, body, color) = if balance < -5_000_000.0 {
        (
            "赤字扩大",
            format!(
                "每日缺口 {}，优先用债券、MEFO 或卖金覆盖。",
                format_million(-balance)
            ),
            BAD,
        )
    } else if mefo_ratio > 0.25 {
        (
            "MEFO 风险",
            format!(
                "梅福票据已达 GDP {:.1}%，继续扩张会压低信用。",
                mefo_ratio * 100.0
            ),
            WARN,
        )
    } else if data.is_foreign_exchange_control {
        (
            "外汇管制",
            "外汇购买受限，英镑储备需要通过出口或卖金补充。".to_owned(),
            WARN,
        )
    } else {
        (
            "财政稳定",
            "现金流仍可支撑当前开支，保持对债务和外汇的监控。".to_owned(),
            GOOD,
        )
    };
    render_finance_status(ui, color, label, &body);

    components::section_card(ui, "财政健康度", |ui| {
        let debt_ratio = if data.gdp_rm > 0.0 {
            (data.public_debt_rm + data.mefo_debt_rm) / data.gdp_rm
        } else {
            0.0
        };
        let reserve_cover = if data.daily_expense_rm > 0.0 {
            data.cash_rm / data.daily_expense_rm
        } else {
            0.0
        };
        ui.columns(2, |c| {
            components::metric_tile(
                &mut c[0],
                "债务 / GDP",
                format!("{:.1}%", debt_ratio * 100.0),
                debt_ratio_color(debt_ratio),
            );
            components::metric_tile(
                &mut c[1],
                "MEFO / GDP",
                format!("{:.1}%", mefo_ratio * 100.0),
                mefo_color(mefo_ratio),
            );
        });
        ui.add_space(4.0);
        ui.columns(2, |c| {
            components::metric_tile(
                &mut c[0],
                "现金覆盖天数",
                format!("{:.1} 天", reserve_cover),
                cover_color(reserve_cover),
            );
            components::metric_tile(
                &mut c[1],
                "经营赤字",
                signed_million(data.operating_income_rm - data.operating_expense_rm),
                balance_color(data.operating_income_rm, data.operating_expense_rm),
            );
        });
    });

    components::section_card(ui, "财政压力来源", |ui| {
        let military_total = data.budget_breakdown.expense_military_wages_rm
            + data.budget_breakdown.expense_military_procurement_rm
            + data.budget_breakdown.expense_military_maintenance_rm;
        let construction_total = data.budget_breakdown.expense_construction_goods_rm
            + data.budget_breakdown.expense_construction_wages_rm;
        let debt_total = data.budget_breakdown.expense_debt_interest_rm
            + data.budget_breakdown.expense_mefo_forced_payment_rm;
        ui.columns(3, |c| {
            components::pressure_tile(
                &mut c[0],
                "军费",
                format_million(military_total),
                "工资 / 采购 / 维护",
                Color32::from_rgb(0xff, 0xaa, 0x66),
            );
            components::pressure_tile(
                &mut c[1],
                "建造",
                format_million(construction_total),
                "建材 / 施工",
                BLUE,
            );
            components::pressure_tile(
                &mut c[2],
                "债务",
                format_million(debt_total),
                "利息 / MEFO 偿付",
                BAD,
            );
        });
        let deficit = (data.daily_expense_rm - data.daily_income_rm).max(0.0);
        if deficit > 0.0 {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "赤字风险：每日缺口 {}，需要债券、MEFO、卖金或削减支出覆盖。",
                    format_million(deficit)
                ))
                .size(11.0)
                .color(BAD),
            );
        }
    });
}

fn render_finance_status(ui: &mut egui::Ui, color: Color32, label: &str, body: &str) {
    let inner = egui::Frame::new()
        .fill(PANEL_CARD_SOFT)
        .stroke(egui::Stroke::new(1.0, STROKE_TILE))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(label).strong().size(13.0).color(color));
            ui.add_space(2.0);
            ui.add(egui::Label::new(RichText::new(body).size(12.0).color(PARCHMENT)).wrap());
        });
    let rect = inner.response.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.left_top(), egui::vec2(4.0, rect.height())),
        0.0,
        color,
    );
}

fn render_budget_tab(ui: &mut egui::Ui, data: &FinancePanelData) {
    let operating_balance = data.operating_income_rm - data.operating_expense_rm;
    let post_balance = data.daily_income_rm - data.daily_expense_rm;

    components::section_card(ui, tr("v6_finance_budget"), |ui| {
        ui.label(RichText::new("经营性 ── 含融资").size(11.0).color(MUTED));
        ui.add_space(2.0);
        ui.columns(3, |c| {
            components::metric_tile(
                &mut c[0],
                "经营性收入",
                format_million(data.operating_income_rm),
                GOOD,
            );
            components::metric_tile(
                &mut c[1],
                "经营性支出",
                format_million(data.operating_expense_rm),
                BAD,
            );
            components::metric_tile(
                &mut c[2],
                "原始赤字",
                signed_million(operating_balance),
                balance_color(data.operating_income_rm, data.operating_expense_rm),
            );
        });
        ui.add_space(6.0);
        ui.columns(3, |c| {
            components::metric_tile(
                &mut c[0],
                "含融资收入",
                format_million(data.daily_income_rm),
                GOOD,
            );
            components::metric_tile(
                &mut c[1],
                tr("v6_finance_daily_expense"),
                format_million(data.daily_expense_rm),
                BAD,
            );
            components::metric_tile(
                &mut c[2],
                "融资后日净额",
                signed_million(post_balance),
                balance_color(data.daily_income_rm, data.daily_expense_rm),
            );
        });
        if data.mefo_coverage_rm > 0.0 {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "MEFO 本日覆盖赤字 {}，不计入经营性收入。",
                    format_million(data.mefo_coverage_rm)
                ))
                .size(11.0)
                .color(GOLD),
            );
        }
    });

    components::section_card(ui, "收支明细", |ui| {
        let income_items: [(&str, f64); 4] = [
            (
                tr("v6_budget_income_taxes"),
                data.budget_breakdown.income_taxes_rm,
            ),
            (
                tr("v6_budget_income_state_profit"),
                data.budget_breakdown.income_state_profit_rm,
            ),
            (
                tr("v6_budget_income_domestic_bonds"),
                data.budget_breakdown.income_domestic_bonds_rm,
            ),
            (
                tr("v6_budget_income_other"),
                data.budget_breakdown.income_other_rm,
            ),
        ];
        let expense_items: [(&str, f64); 12] = [
            (
                tr("v6_budget_expense_state_payroll"),
                data.budget_breakdown.expense_state_payroll_rm,
            ),
            (
                tr("v6_budget_expense_military_wages"),
                data.budget_breakdown.expense_military_wages_rm,
            ),
            (
                tr("v6_budget_expense_military_procurement"),
                data.budget_breakdown.expense_military_procurement_rm,
            ),
            (
                tr("v6_budget_expense_military_maintenance"),
                data.budget_breakdown.expense_military_maintenance_rm,
            ),
            (
                tr("v6_budget_expense_construction_goods"),
                data.budget_breakdown.expense_construction_goods_rm,
            ),
            (
                tr("v6_budget_expense_construction_wages"),
                data.budget_breakdown.expense_construction_wages_rm,
            ),
            (
                tr("v6_budget_expense_welfare"),
                data.budget_breakdown.expense_welfare_rm,
            ),
            (
                tr("v6_budget_expense_debt_interest"),
                data.budget_breakdown.expense_debt_interest_rm,
            ),
            (
                tr("v6_budget_expense_forex"),
                data.budget_breakdown.expense_foreign_currency_rm,
            ),
            (
                tr("v6_budget_expense_mefo_forced"),
                data.budget_breakdown.expense_mefo_forced_payment_rm,
            ),
            ("研究经费", data.budget_breakdown.expense_research_rm),
            (
                tr("v6_budget_expense_other"),
                data.budget_breakdown.expense_other_rm,
            ),
        ];

        ui.columns(2, |c| {
            c[0].label(
                RichText::new(tr("v6_budget_income_breakdown"))
                    .strong()
                    .color(GOOD)
                    .size(12.0),
            );
            c[0].add_space(2.0);
            for (idx, (label, amount)) in income_items.iter().enumerate() {
                components::zebra_row(&mut c[0], idx, label, format_million(*amount), GOOD);
            }
            c[0].add_space(3.0);
            let income_sum = data.budget_breakdown.total_income_rm();
            let income_ok = (income_sum - data.daily_income_rm).abs()
                <= data.daily_income_rm.abs().max(1.0) * 0.001;
            components::total_row(
                &mut c[0],
                tr("total"),
                format_million(income_sum),
                income_ok,
            );

            c[1].label(
                RichText::new(tr("v6_budget_expense_breakdown"))
                    .strong()
                    .color(BAD)
                    .size(12.0),
            );
            c[1].add_space(2.0);
            for (idx, (label, amount)) in expense_items.iter().enumerate() {
                components::zebra_row(&mut c[1], idx, label, format_million(*amount), BAD);
            }
            c[1].add_space(3.0);
            let expense_sum = data.budget_breakdown.total_expense_rm();
            let expense_ok = (expense_sum - data.daily_expense_rm).abs()
                <= data.daily_expense_rm.abs().max(1.0) * 0.001;
            components::total_row(
                &mut c[1],
                tr("total"),
                format_million(expense_sum),
                expense_ok,
            );
        });
    });
}

fn render_debt_tab(ui: &mut egui::Ui, data: &FinancePanelData, cmds: &mut Vec<FinanceCommand>) {
    let mefo_ratio = if data.gdp_rm > 0.0 {
        data.mefo_debt_rm / data.gdp_rm
    } else {
        0.0
    };

    components::section_card(ui, tr("v6_finance_debt"), |ui| {
        ui.columns(3, |c| {
            components::metric_tile(
                &mut c[0],
                tr("v6_finance_public_debt_rm"),
                format_million(data.public_debt_rm),
                GOLD,
            );
            components::metric_tile(
                &mut c[1],
                tr("v6_finance_mefo_debt"),
                format!(
                    "{} / {:.1}%",
                    format_million(data.mefo_debt_rm),
                    mefo_ratio * 100.0
                ),
                mefo_color(mefo_ratio),
            );
            components::metric_tile(
                &mut c[2],
                tr("v6_finance_credit_rating"),
                data.credit_rating.clone(),
                rating_color(&data.credit_rating),
            );
        });
        ui.add_space(8.0);
        components::kv_row(
            ui,
            tr("v6_finance_public_debt_gbp"),
            format!("£ {}", format_million(data.public_debt_gbp)),
            GOLD,
        );
        components::kv_row(
            ui,
            tr("v6_finance_interest_rate"),
            format!("{:.2}%", data.bond_interest_rate * 100.0),
            PARCHMENT,
        );
        if data.mefo_military_budget_rm > 0.0 || data.mefo_military_spent_rm > 0.0 {
            ui.add_space(4.0);
            components::kv_row(
                ui,
                "MEFO 军工预算",
                format_million(data.mefo_military_budget_rm),
                GOLD,
            );
            components::kv_row(
                ui,
                "MEFO 军工已用",
                format_million(data.mefo_military_spent_rm),
                WARN,
            );
        }
        if data.financing_breakdown.mefo_interest_capitalized_rm > 0.0 {
            components::kv_row(
                ui,
                "MEFO 利息资本化",
                format_million(data.financing_breakdown.mefo_interest_capitalized_rm),
                WARN,
            );
        }
    });

    components::section_card(ui, "发行债券", |ui| {
        ui.label(
            RichText::new("发行国内 / 国外债券补充国库，债务会按当前利率计息。")
                .size(11.0)
                .color(MUTED),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if components::action_button_colored(
                ui,
                true,
                tr("v6_finance_issue_domestic_bond"),
                GOLD_BRIGHT,
            )
            .on_hover_text("每次发行 500M RM 国内债券")
            .clicked()
            {
                cmds.push(FinanceCommand::IssueDomesticBond {
                    amount_rm: 500_000_000.0,
                });
            }
            if components::action_button_colored(
                ui,
                data.can_issue_foreign_bond,
                tr("v6_finance_issue_foreign_bond"),
                BLUE,
            )
            .on_hover_text("每次发行 100M £ 外债，受信用与外汇政策约束")
            .clicked()
            {
                cmds.push(FinanceCommand::IssueForeignBond {
                    amount_gbp: 100_000_000.0,
                });
            }
        });
    });
}

fn render_exchange_tab(ui: &mut egui::Ui, data: &FinancePanelData) {
    components::section_card(ui, tr("v6_finance_exchange"), |ui| {
        ui.columns(3, |c| {
            components::metric_tile(
                &mut c[0],
                tr("v6_finance_reserve_gbp"),
                format!("£ {}", format_million(data.reserve_gbp)),
                BLUE,
            );
            components::metric_tile(
                &mut c[1],
                tr("v6_finance_gold_kg"),
                format!("{:.0} kg", data.gold_kg),
                GOLD_BRIGHT,
            );
            components::metric_tile(
                &mut c[2],
                tr("v6_exchange_rate"),
                format!("{:.2} RM/£", data.exchange_rate_rm_per_gbp),
                GOLD,
            );
        });
        if data.is_foreign_exchange_control {
            ui.add_space(6.0);
            components::status_banner(ui, BAD, "外汇管制", tr("v6_finance_forex_control"));
        }
    });

    components::section_card(ui, "GDP 构成", |ui| {
        components::kv_row(
            ui,
            "GDP（总）",
            format!(
                "{} RM  /  £ {}",
                format_million(data.gdp_rm),
                format_million(data.gdp_gbp)
            ),
            PARCHMENT,
        );
        components::kv_row(
            ui,
            "本土 GDP",
            format!(
                "{} RM  /  £ {}",
                format_million(data.domestic_gdp_rm),
                format_million(data.domestic_gdp_gbp)
            ),
            GOOD,
        );
        components::kv_row(
            ui,
            "殖民 GDP",
            format!(
                "{} RM  /  £ {}",
                format_million(data.colonial_gdp_rm),
                format_million(data.colonial_gdp_gbp)
            ),
            WARN,
        );
        components::kv_row(
            ui,
            "殖民抽取值",
            format!(
                "{} RM  /  £ {}",
                format_million(data.colonial_extracted_value_rm),
                format_million(data.colonial_extracted_value_gbp)
            ),
            GOLD,
        );
    });
}

fn render_actions_tab(ui: &mut egui::Ui, data: &FinancePanelData, cmds: &mut Vec<FinanceCommand>) {
    let deficit = data.daily_expense_rm - data.daily_income_rm;
    let mefo_can =
        data.can_print_mefo && deficit > 0.0 && data.mefo_debt_rm / data.gdp_rm.max(1.0) < 0.30;

    components::section_card(ui, tr("v6_finance_actions"), |ui| {
        ui.label(
            RichText::new(
                "操作会直接影响现金、外汇储备、债务结构和信用评级。优先处理赤字和外汇短缺。",
            )
            .size(11.0)
            .color(MUTED),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if components::action_button_colored(
                ui,
                mefo_can,
                tr("v6_finance_print_mefo"),
                GOLD_BRIGHT,
            )
            .on_hover_text("MEFO/GDP 低于 30% 且存在赤字时可印发梅福票据")
            .clicked()
            {
                cmds.push(FinanceCommand::PrintMefo);
            }
            if components::action_button_colored(
                ui,
                data.gold_kg > 0.0,
                tr("v6_finance_sell_gold"),
                GOLD,
            )
            .on_hover_text("一次性卖出当前金库 10% 的黄金换 £")
            .clicked()
            {
                cmds.push(FinanceCommand::SellGold {
                    kg: data.gold_kg * 0.1,
                });
            }
            if components::action_button_colored(
                ui,
                !data.is_foreign_exchange_control,
                tr("v6_finance_buy_gbp"),
                BLUE,
            )
            .on_hover_text("用 RM 买入 10M £；外汇管制下禁用")
            .clicked()
            {
                cmds.push(FinanceCommand::BuyForeignCurrency {
                    gbp_amount: 10_000_000.0,
                });
            }
        });
    });

    if data.financing_breakdown.mefo_issued_rm > 0.0
        || data.financing_breakdown.domestic_bond_issued_rm > 0.0
        || data.financing_breakdown.mefo_interest_capitalized_rm > 0.0
    {
        components::section_card(ui, "融资现金流", |ui| {
            if data.financing_breakdown.mefo_issued_rm > 0.0 {
                components::kv_row(
                    ui,
                    "MEFO 票据发行",
                    format_million(data.financing_breakdown.mefo_issued_rm),
                    GOLD,
                );
            }
            if data.financing_breakdown.mefo_interest_capitalized_rm > 0.0 {
                components::kv_row(
                    ui,
                    "MEFO 利息资本化",
                    format_million(data.financing_breakdown.mefo_interest_capitalized_rm),
                    WARN,
                );
            }
            if data.financing_breakdown.domestic_bond_issued_rm > 0.0 {
                components::kv_row(
                    ui,
                    "国内债券发行",
                    format_million(data.financing_breakdown.domestic_bond_issued_rm),
                    BLUE,
                );
            }
            if data.mefo_coverage_rm > 0.0 {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "MEFO 本日覆盖赤字 {}，不改变经营性收入。",
                        format_million(data.mefo_coverage_rm)
                    ))
                    .size(11.0)
                    .color(GOLD),
                );
            }
        });
    }
}

// ─── 数值 / 颜色辅助 ────────────────────────────────────────────

fn v9_rating_color(rating: &str) -> Color32 {
    use crate::v9::tokens::palette;
    match rating {
        "AAA" | "AA" | "A" => palette::GOOD,
        "BBB" | "BB" => palette::WARN,
        "B" | "CCC" | "D" => palette::BAD,
        _ => palette::PARCHMENT_DIM,
    }
}

fn v9_finance_balance_color(value: f64) -> Color32 {
    use crate::v9::tokens::palette;
    if value >= 0.0 {
        palette::GOOD
    } else if value > -1_000_000.0 {
        palette::WARN
    } else {
        palette::BAD
    }
}

fn v9_finance_debt_color(value: f64) -> Color32 {
    use crate::v9::tokens::palette;
    if value > 0.60 {
        palette::BAD
    } else if value > 0.40 {
        palette::WARN
    } else {
        palette::GOOD
    }
}

fn v9_finance_mefo_color(value: f64) -> Color32 {
    use crate::v9::tokens::palette;
    if value > 0.30 {
        palette::BAD
    } else if value > 0.20 {
        palette::WARN
    } else {
        palette::GOLD
    }
}

fn v9_finance_cover_color(days: f64) -> Color32 {
    use crate::v9::tokens::palette;
    if days < 30.0 {
        palette::BAD
    } else if days < 90.0 {
        palette::WARN
    } else {
        palette::GOOD
    }
}

fn rating_color(rating: &str) -> Color32 {
    match rating {
        "AAA" => Color32::from_rgb(0x60, 0xc0, 0x60),
        "AA" => Color32::from_rgb(0x80, 0xc0, 0x50),
        "A" => Color32::from_rgb(0xa0, 0xc0, 0x30),
        "BBB" => Color32::from_rgb(0xc0, 0xc0, 0x30),
        "BB" => Color32::from_rgb(0xc0, 0xa0, 0x20),
        "B" => Color32::from_rgb(0xc0, 0x80, 0x20),
        "CCC" => Color32::from_rgb(0xc0, 0x60, 0x20),
        "D" => Color32::from_rgb(0xc0, 0x30, 0x30),
        _ => Color32::from_gray(200),
    }
}

fn balance_color(income: f64, expense: f64) -> Color32 {
    let diff = income - expense;
    if diff > 0.0 {
        Color32::from_rgb(0x60, 0xc0, 0x60)
    } else if diff > -1_000_000.0 {
        Color32::from_rgb(0xc0, 0xc0, 0x30)
    } else {
        Color32::from_rgb(0xc0, 0x40, 0x40)
    }
}

fn debt_ratio_color(r: f64) -> Color32 {
    if r > 0.60 {
        BAD
    } else if r > 0.40 {
        WARN
    } else {
        GOOD
    }
}

fn mefo_color(r: f64) -> Color32 {
    if r > 0.30 {
        BAD
    } else if r > 0.20 {
        WARN
    } else {
        GOLD
    }
}

fn cover_color(days: f64) -> Color32 {
    if days < 30.0 {
        BAD
    } else if days < 90.0 {
        WARN
    } else {
        GOOD
    }
}

fn signed_million(v: f64) -> String {
    let sign = if v >= 0.0 { "+" } else { "" };
    format!("{}{}", sign, format_million(v))
}

fn format_million(v: f64) -> String {
    if v.abs() >= 1_000_000_000.0 {
        format!("{:.1}B", v / 1_000_000_000.0)
    } else if v.abs() >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v.abs() >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finance_panel_data_constructs() {
        let d = FinancePanelData {
            cash_rm: 5_000_000_000.0,
            reserve_gbp: 400_000_000.0,
            gold_kg: 700_000.0,
            daily_income_rm: 50_000_000.0,
            daily_expense_rm: 45_000_000.0,
            budget_breakdown: BudgetBreakdownData::default(),
            financing_breakdown: FinancingBreakdownData::default(),
            operating_income_rm: 40_000_000.0,
            operating_expense_rm: 45_000_000.0,
            original_deficit_rm: 5_000_000.0,
            mefo_coverage_rm: 5_000_000.0,
            post_financing_cash_change_rm: 5_000_000.0,
            public_debt_gbp: 0.0,
            public_debt_rm: 0.0,
            mefo_debt_rm: 0.0,
            mefo_military_budget_rm: 0.0,
            mefo_military_spent_rm: 0.0,
            credit_rating: "AAA".into(),
            bond_interest_rate: 0.04,
            gdp_rm: 83_000_000_000.0,
            gdp_gbp: 6_640_000_000.0,
            domestic_gdp_rm: 70_000_000_000.0,
            domestic_gdp_gbp: 5_600_000_000.0,
            colonial_gdp_rm: 13_000_000_000.0,
            colonial_gdp_gbp: 1_040_000_000.0,
            colonial_extracted_value_rm: 4_550_000_000.0,
            colonial_extracted_value_gbp: 364_000_000.0,
            exchange_rate_rm_per_gbp: 12.5,
            can_print_mefo: true,
            can_issue_foreign_bond: true,
            is_foreign_exchange_control: false,
        };
        assert_eq!(d.credit_rating, "AAA");
        assert!(d.can_print_mefo);
    }

    #[test]
    fn format_million_billions() {
        assert_eq!(format_million(5_000_000_000.0), "5.0B");
    }

    #[test]
    fn rating_color_aaa_green() {
        let c = rating_color("AAA");
        assert_eq!(c, Color32::from_rgb(0x60, 0xc0, 0x60));
    }

    #[test]
    fn balance_color_positive_green() {
        let c = balance_color(100.0, 50.0);
        assert_eq!(c, Color32::from_rgb(0x60, 0xc0, 0x60));
    }
}
