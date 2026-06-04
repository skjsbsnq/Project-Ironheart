//! V6 市场面板 UI：25 种商品价格 / 供需 / 汇率总览。
//!
//! V6.A 验收要求：只读面板，显示所有商品及汇率。

use crate::{components, i18n::tr};
use egui::{Color32, RichText};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoodCategory {
    RawMaterial,
    Intermediate,
    Consumer,
    Luxury,
    Service,
    MilitaryIntermediate,
}

fn category_label(cat: &GoodCategory) -> &'static str {
    match cat {
        GoodCategory::RawMaterial => tr("v6_good_raw_material"),
        GoodCategory::Intermediate => tr("v6_good_intermediate"),
        GoodCategory::Consumer => tr("v6_good_consumer"),
        GoodCategory::Luxury => tr("v6_good_luxury"),
        GoodCategory::Service => tr("v6_good_service"),
        GoodCategory::MilitaryIntermediate => tr("v6_good_military_intermediate"),
    }
}

fn category_color(cat: &GoodCategory) -> Color32 {
    match cat {
        GoodCategory::RawMaterial => Color32::from_rgb(0x80, 0xa0, 0x50),
        GoodCategory::Intermediate => Color32::from_rgb(0x60, 0x90, 0xb0),
        GoodCategory::Consumer => Color32::from_rgb(0xc0, 0xa0, 0x30),
        GoodCategory::Luxury => Color32::from_rgb(0xb0, 0x70, 0xc0),
        GoodCategory::Service => Color32::from_rgb(0x50, 0xb0, 0x90),
        GoodCategory::MilitaryIntermediate => Color32::from_rgb(0xa0, 0x50, 0x50),
    }
}

fn category_sort_order(cat: &GoodCategory) -> u8 {
    match cat {
        GoodCategory::RawMaterial => 0,
        GoodCategory::Intermediate => 1,
        GoodCategory::MilitaryIntermediate => 2,
        GoodCategory::Consumer => 3,
        GoodCategory::Luxury => 4,
        GoodCategory::Service => 5,
    }
}

#[derive(Debug, Clone)]
pub struct GoodEntry {
    pub id: String,
    pub name: String,
    pub category: GoodCategory,
    pub price: f32,
    pub base_price: f32,
    pub supply: f32,
    pub demand: f32,
    pub traded: f32,
    pub stockpile: f32,
    pub stockpile_coverage_days: f32,
    pub unmet_demand: f32,
    pub domestic_production: f32,
    pub stockpile_draw: f32,
    pub building_input_demand: f32,
    pub pop_consumption_demand: f32,
    pub military_order_demand: f32,
    pub supply_sources: Vec<GoodSupplySourceEntry>,
    pub producers: Vec<GoodFlowSource>,
    pub consumers: Vec<GoodFlowSource>,
    pub government_orders: Vec<GoodFlowSource>,
    pub construction_demand: f32,
    pub imports: f32,
    pub exports: f32,
    pub is_blockaded: bool,
    pub affected_buildings: Vec<GoodFlowSource>,
    pub affected_pop_classes: Vec<GoodFlowSource>,
    pub affected_demand_buckets: Vec<GoodFlowSource>,
    pub upstream_goods: Vec<String>,
    pub downstream_goods: Vec<String>,
    pub shortage_reason: String,
    pub actionable_fixes: Vec<MarketActionEntry>,
    pub paid_rm: f64,
    pub clearing_fulfilled: f32,
    pub clearing_unmet: f32,
    pub clearing_shortage_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct GoodFlowSource {
    pub name: String,
    pub amount: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoodSupplySourceKind {
    Domestic,
    MarketBloc,
    Subject,
    WorldSpot,
    Stockpile,
}

#[derive(Debug, Clone)]
pub struct GoodSupplySourceEntry {
    pub kind: GoodSupplySourceKind,
    pub label: String,
    pub amount: f32,
}

#[derive(Debug, Clone)]
pub struct MarketPanelData {
    pub goods: Vec<GoodEntry>,
    pub bloc: Option<MarketBlocPanelData>,
    pub exchange_rate: f32,
    pub cash_rm: f64,
    pub total_shortage_value_rm: f64,
    pub total_import_value_gbp: f64,
    pub total_export_value_gbp: f64,
    pub pop_needs_fulfillment: f32,
    pub military_supply_pressure: f32,
    pub subjects: Vec<MarketSubjectEntry>,
    pub actions: Vec<MarketActionEntry>,
    pub alerts: Vec<MarketAlertEntry>,
}

#[derive(Debug, Clone)]
pub struct MarketSubjectEntry {
    pub tag: String,
    pub autonomy_level: String,
    pub master_resource_share: f32,
    pub resource_contribution: Vec<GoodFlowSource>,
    pub fiscal_contribution_gbp: f64,
    pub risk: String,
}

#[derive(Debug, Clone)]
pub struct MarketActionEntry {
    pub title: String,
    pub description: String,
    pub related_good_id: Option<String>,
    pub priority: u8,
}

#[derive(Debug, Clone)]
pub struct MarketBlocPanelData {
    pub name: String,
    pub kind: String,
    pub leader_tag: String,
    pub members: Vec<MarketBlocMemberEntry>,
    pub internal_trade_value_gbp: f64,
    pub external_trade_value_gbp: f64,
}

#[derive(Debug, Clone)]
pub struct MarketBlocMemberEntry {
    pub tag: String,
    pub relation: String,
    pub market_access: f32,
    pub contribution_supply_value_rm: f64,
    pub contribution_demand_value_rm: f64,
    pub strategic_goods: Vec<GoodFlowSource>,
}

#[derive(Debug, Clone)]
pub struct MarketAlertEntry {
    pub severity: MarketAlertSeverity,
    pub good_id: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketAlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketPanelTab {
    Overview,
    Shortages,
    Goods,
    IndustryChain,
    MarketBloc,
    Subjects,
    Trade,
    Actions,
}

fn market_v9_tab_order() -> [MarketPanelTab; 8] {
    [
        MarketPanelTab::Overview,
        MarketPanelTab::Shortages,
        MarketPanelTab::Goods,
        MarketPanelTab::IndustryChain,
        MarketPanelTab::Trade,
        MarketPanelTab::MarketBloc,
        MarketPanelTab::Subjects,
        MarketPanelTab::Actions,
    ]
}

impl MarketPanelTab {
    fn id(self) -> &'static str {
        match self {
            MarketPanelTab::Overview => "overview",
            MarketPanelTab::Shortages => "shortages",
            MarketPanelTab::Goods => "goods",
            MarketPanelTab::IndustryChain => "industry_chain",
            MarketPanelTab::Trade => "trade",
            MarketPanelTab::MarketBloc => "market_bloc",
            MarketPanelTab::Subjects => "subjects",
            MarketPanelTab::Actions => "actions",
        }
    }

    fn label(self) -> &'static str {
        match self {
            MarketPanelTab::Overview => "总览",
            MarketPanelTab::Shortages => "短缺",
            MarketPanelTab::Goods => "商品",
            MarketPanelTab::IndustryChain => "产业链",
            MarketPanelTab::Trade => "进口出口",
            MarketPanelTab::MarketBloc => "市场圈",
            MarketPanelTab::Subjects => "殖民/傀儡供给",
            MarketPanelTab::Actions => "行动建议",
        }
    }

    fn from_id(id: &str) -> Self {
        market_v9_tab_order()
            .into_iter()
            .find(|tab| tab.id() == id)
            .unwrap_or(MarketPanelTab::Overview)
    }
}

pub struct MarketPanel;

impl MarketPanel {
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &MarketPanelData) -> (bool, Vec<()>) {
        return v9_show_market(ctx, data);

        let mut close = false;
        let search_id = egui::Id::new("market_panel_search");
        let shortage_first_id = egui::Id::new("market_panel_shortage_first");
        let tab_id = egui::Id::new("market_panel_tab_v7_i4");
        let mut search = ctx
            .data_mut(|d| d.get_persisted::<String>(search_id))
            .unwrap_or_default();
        let mut shortage_first = ctx
            .data_mut(|d| d.get_persisted::<bool>(shortage_first_id))
            .unwrap_or(true);
        let mut tab = ctx
            .data_mut(|d| d.get_persisted::<MarketPanelTab>(tab_id))
            .unwrap_or(MarketPanelTab::Overview);

        egui::SidePanel::left("market_panel")
            .default_width(520.0)
            .min_width(420.0)
            .resizable(true)
            .show(ctx, |ui| {
                components::panel_header(ui, tr("v6_market_panel_title"), &mut close);
                components::summary_strip(
                    ui,
                    &[
                        (
                            tr("v6_exchange_rate"),
                            format!("{:.2} RM/£", data.exchange_rate),
                        ),
                        (tr("v6_cash_rm"), format!("{:.0} RM", data.cash_rm)),
                        (
                            "短缺价值",
                            format!("{:.0} RM", data.total_shortage_value_rm),
                        ),
                        (
                            "POP 满足",
                            format!("{:.0}%", data.pop_needs_fulfillment * 100.0),
                        ),
                    ],
                );
                ui.add_space(4.0);
                ui.separator();

                render_tabs(ui, &mut tab);
                ui.separator();

                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("搜索").small().color(Color32::from_gray(150)));
                    ui.add(
                        egui::TextEdit::singleline(&mut search)
                            .hint_text("商品、建筑、来源或去向")
                            .desired_width(220.0),
                    );
                    ui.checkbox(&mut shortage_first, "短缺优先");
                    if !search.is_empty() && ui.small_button("清空").clicked() {
                        search.clear();
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| match tab {
                    MarketPanelTab::Overview => render_overview_tab(ui, data),
                    MarketPanelTab::Shortages => render_shortages_tab(ui, data, &search),
                    MarketPanelTab::Goods => render_goods_tab(ui, data, &search, shortage_first),
                    MarketPanelTab::IndustryChain => render_industry_chain_tab(ui, data, &search),
                    MarketPanelTab::MarketBloc => render_market_bloc_tab(ui, data),
                    MarketPanelTab::Subjects => render_subjects_tab(ui, data),
                    MarketPanelTab::Trade => render_trade_tab(ui, data, &search),
                    MarketPanelTab::Actions => render_actions_tab(ui, data),
                });
            });

        ctx.data_mut(|d| {
            d.insert_persisted(search_id, search);
            d.insert_persisted(shortage_first_id, shortage_first);
            d.insert_persisted(tab_id, tab);
        });

        (close, Vec::new())
    }
}

fn v9_show_market(ctx: &egui::Context, data: &MarketPanelData) -> (bool, Vec<()>) {
    use crate::v9::composites::panel_shell::{draw_summary_tiles, PanelClass, PanelShell};
    use crate::v9::tokens::palette;

    let tab_id = egui::Id::new("market_panel_v9_tab");
    let mut tab = ctx
        .data_mut(|d| d.get_persisted::<MarketPanelTab>(tab_id))
        .unwrap_or(MarketPanelTab::Overview);
    let critical_alerts = data
        .alerts
        .iter()
        .filter(|alert| alert.severity == MarketAlertSeverity::Critical)
        .count();
    let shortage_count = data
        .goods
        .iter()
        .filter(|good| shortage_amount(good) > 0.0)
        .count();
    let accent = if critical_alerts > 0 || data.total_shortage_value_rm > 0.0 {
        palette::BAD
    } else if data.military_supply_pressure > 0.65 {
        palette::WARN
    } else {
        palette::GOLD
    };

    let (close, _) = PanelShell::new("market_panel_v9", tr("v6_market_panel_title"))
        .subtitle("商品清算 / 短缺 / 供应链")
        .class(PanelClass::Economy)
        .accent(accent)
        .footer("Q Close  |  Goods table / Commodity detail")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("v6_exchange_rate"),
                        format!("{:.2} RM/GBP", data.exchange_rate),
                        palette::INFO,
                    ),
                    (tr("v6_cash_rm"), v9_money_rm(data.cash_rm), palette::GOLD),
                    (
                        "短缺",
                        format!(
                            "{} / {}",
                            shortage_count,
                            v9_money_rm(data.total_shortage_value_rm)
                        ),
                        if shortage_count > 0 {
                            palette::BAD
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        "人口需求",
                        format!("{:.0}%", data.pop_needs_fulfillment * 100.0),
                        v9_ratio_color(data.pop_needs_fulfillment),
                    ),
                    (
                        "进口",
                        v9_money_gbp(data.total_import_value_gbp),
                        palette::INFO,
                    ),
                    (
                        tr("v6_exports"),
                        v9_money_gbp(data.total_export_value_gbp),
                        palette::GOOD,
                    ),
                ],
            );
            v9_market_tabs(ui, layout.tabs, &mut tab, data);
            v9_market_body(ui, layout.body, data, tab);
        });
    ctx.data_mut(|d| d.insert_persisted(tab_id, tab));

    (close, Vec::new())
}

fn v9_market_tabs(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    tab: &mut MarketPanelTab,
    data: &MarketPanelData,
) {
    use crate::v9::primitives::{TabBar, TabItem};

    let shortage_count = data
        .goods
        .iter()
        .filter(|good| shortage_amount(good) > 0.0)
        .count();
    let mut active = tab.id();
    let items: Vec<TabItem> = market_v9_tab_order()
        .into_iter()
        .map(|tab| {
            let item = TabItem::new(tab.id(), tab.label());
            match tab {
                MarketPanelTab::Shortages => item.with_badge(shortage_count as u32),
                MarketPanelTab::Actions => item.with_badge(data.actions.len() as u32),
                MarketPanelTab::Subjects => item.with_badge(data.subjects.len() as u32),
                MarketPanelTab::MarketBloc => {
                    if data.bloc.is_some() {
                        item.with_badge(1)
                    } else {
                        item
                    }
                }
                MarketPanelTab::Overview
                | MarketPanelTab::Goods
                | MarketPanelTab::IndustryChain
                | MarketPanelTab::Trade => item,
            }
        })
        .collect();
    if let Some(clicked) = TabBar::show_at(ui, rect, &items, &mut active) {
        *tab = MarketPanelTab::from_id(clicked);
    }
}

fn v9_market_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MarketPanelData,
    tab: MarketPanelTab,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.66), Track::Fr(0.34)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        let selected = data.goods.iter().max_by(|a, b| {
            shortage_value(a)
                .partial_cmp(&shortage_value(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    price_ratio(b)
                        .partial_cmp(&price_ratio(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        match tab {
            MarketPanelTab::Overview => {
                v9_market_overview_panel(ui, GridLayout::cell(&cells, 0, 0), data)
            }
            MarketPanelTab::Shortages => {
                v9_market_shortages_panel(ui, GridLayout::cell(&cells, 0, 0), data)
            }
            MarketPanelTab::Goods => {
                v9_market_goods_table(ui, GridLayout::cell(&cells, 0, 0), data)
            }
            MarketPanelTab::IndustryChain => {
                v9_market_chain_panel(ui, GridLayout::cell(&cells, 0, 0), data)
            }
            MarketPanelTab::Trade => {
                v9_market_trade_panel(ui, GridLayout::cell(&cells, 0, 0), data)
            }
            MarketPanelTab::MarketBloc => {
                v9_market_bloc_panel(ui, GridLayout::cell(&cells, 0, 0), data)
            }
            MarketPanelTab::Subjects => {
                v9_market_subjects_panel(ui, GridLayout::cell(&cells, 0, 0), data)
            }
            MarketPanelTab::Actions => {
                v9_market_actions_panel(ui, GridLayout::cell(&cells, 0, 0), data)
            }
        }
        v9_market_detail_sidebar(ui, GridLayout::cell(&cells, 0, 1), data, selected);
    });
}

fn v9_card_scroll_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    use crate::v9::primitives::Card;
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        title,
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let content = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 34.0),
        inner.right_bottom(),
    );
    ui.allocate_ui_at_rect(content, |ui| {
        egui::ScrollArea::vertical().show(ui, add_contents);
    });
}

fn v9_market_overview_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    v9_card_scroll_panel(ui, rect, "Overview", |ui| render_overview_tab(ui, data));
}

fn v9_market_shortages_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    v9_card_scroll_panel(ui, rect, "Shortages", |ui| {
        render_shortages_tab(ui, data, "")
    });
}

fn v9_market_chain_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    v9_card_scroll_panel(ui, rect, "Industry Chain", |ui| {
        render_industry_chain_tab(ui, data, "")
    });
}

fn v9_market_trade_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    v9_card_scroll_panel(ui, rect, "Import / Export", |ui| {
        render_trade_tab(ui, data, "")
    });
}

fn v9_market_bloc_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    v9_card_scroll_panel(ui, rect, "Market Bloc", |ui| {
        render_market_bloc_tab(ui, data)
    });
}

fn v9_market_subjects_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    v9_card_scroll_panel(ui, rect, "Subject Supply", |ui| {
        render_subjects_tab(ui, data)
    });
}

fn v9_market_actions_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    v9_card_scroll_panel(ui, rect, "Actions", |ui| render_actions_tab(ui, data));
}

fn v9_market_goods_table(ui: &mut egui::Ui, rect: egui::Rect, data: &MarketPanelData) {
    use crate::v9::primitives::{
        Card, DataTable, Pill, PillTone, TableCell, TableColumn, TableRow,
    };
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        tr("v6_good"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let shortage_count = data
        .goods
        .iter()
        .filter(|good| shortage_amount(good) > 0.0)
        .count();
    let pill_label = if shortage_count > 0 {
        format!("{} 项短缺", shortage_count)
    } else {
        "清算稳定".to_owned()
    };
    Pill::new(pill_label.as_str())
        .tone(if shortage_count > 0 {
            PillTone::Bad
        } else {
            PillTone::Good
        })
        .show_at(
            ui,
            Rect::from_min_size(
                Pos2::new(inner.right() - 154.0, inner.top()),
                egui::vec2(150.0, 22.0),
            ),
        );

    let mut goods: Vec<&GoodEntry> = data.goods.iter().collect();
    goods.sort_by(|a, b| {
        shortage_amount(b)
            .partial_cmp(&shortage_amount(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| category_sort_order(&a.category).cmp(&category_sort_order(&b.category)))
            .then_with(|| a.name.cmp(&b.name))
    });
    let rows: Vec<TableRow> = goods
        .into_iter()
        .take(18)
        .map(|good| {
            let shortage = shortage_amount(good);
            let ratio = price_ratio(good);
            let accent = if shortage > 0.0 {
                palette::BAD
            } else if ratio > 1.5 {
                palette::WARN
            } else {
                v9_category_color(&good.category)
            };
            TableRow::new(vec![
                TableCell::strong(good.name.as_str()),
                TableCell::colored(
                    category_label(&good.category),
                    v9_category_color(&good.category),
                ),
                TableCell::colored(format!("{:.2}x", ratio), v9_price_ratio_color(ratio)).right(),
                TableCell::new(format!("{:.1}", good.supply)).right(),
                TableCell::new(format!("{:.1}", good.demand)).right(),
                TableCell::colored(
                    if shortage > 0.0 {
                        format!("{:.1}", shortage)
                    } else {
                        "-".to_owned()
                    },
                    if shortage > 0.0 {
                        palette::BAD
                    } else {
                        palette::MUTED
                    },
                )
                .right(),
                TableCell::new(format!("{:.1}d", good.stockpile_coverage_days)).right(),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new(tr("v6_good"), 1.35),
            TableColumn::new("类别", 1.0),
            TableColumn::new("价格", 0.58).right(),
            TableColumn::new(tr("v6_supply"), 0.62).right(),
            TableColumn::new(tr("v6_demand"), 0.62).right(),
            TableColumn::new(tr("v6_shortage"), 0.72).right(),
            TableColumn::new("覆盖", 0.62).right(),
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

fn v9_market_detail_sidebar(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &MarketPanelData,
    good: Option<&GoodEntry>,
) {
    use crate::v9::primitives::{
        draw_progress_bar, Card, DataTable, Pill, PillTone, TableCell, TableColumn, TableRow,
    };
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "商品详情",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let Some(good) = good else {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 34.0),
                inner.right_bottom(),
            ),
            "暂无商品",
            "市场中没有可显示的商品。",
        );
        return;
    };

    let shortage = shortage_amount(good);
    let status_label = if shortage > 0.0 { "短缺" } else { "稳定" };
    Pill::new(status_label)
        .tone(if shortage > 0.0 {
            PillTone::Bad
        } else {
            PillTone::Good
        })
        .show_at(
            ui,
            Rect::from_min_size(
                Pos2::new(inner.right() - 112.0, inner.top()),
                Vec2::new(108.0, 22.0),
            ),
        );

    let mut y = inner.top() + 34.0;
    ui.painter().text(
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        good.name.as_str(),
        TextRole::Subheading.font_id(),
        v9_category_color(&good.category),
    );
    y += 24.0;

    let clearing = if good.demand > 0.0 {
        (good.traded / good.demand).clamp(0.0, 1.0)
    } else {
        1.0
    };
    draw_progress_bar(
        ui,
        Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 10.0)),
        clearing,
        if clearing < 0.8 {
            palette::BAD
        } else if clearing < 0.95 {
            palette::WARN
        } else {
            palette::GOOD
        },
    );
    y += 24.0;

    let detail_rows = vec![
        TableRow::new(vec![
            TableCell::strong("供给"),
            TableCell::new(format!("{:.1}", good.supply)).right(),
        ]),
        TableRow::new(vec![
            TableCell::strong("需求"),
            TableCell::new(format!("{:.1}", good.demand)).right(),
        ]),
        TableRow::new(vec![
            TableCell::strong("短缺"),
            TableCell::colored(
                format!("{:.1}", shortage),
                if shortage > 0.0 {
                    palette::BAD
                } else {
                    palette::GOOD
                },
            )
            .right(),
        ]),
        TableRow::new(vec![
            TableCell::strong("库存"),
            TableCell::new(format!("{:.1}", good.stockpile)).right(),
        ]),
        TableRow::new(vec![
            TableCell::strong("已支付 RM"),
            TableCell::new(v9_money_rm(good.paid_rm)).right(),
        ]),
    ];
    DataTable::new(
        vec![
            TableColumn::new("指标", 1.0),
            TableColumn::new("数值", 1.0).right(),
        ],
        detail_rows,
    )
    .row_height(26.0)
    .show_at(
        ui,
        Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 156.0)),
    );
    y += 168.0;

    let mut flow_rows: Vec<TableRow> = Vec::new();
    for source in good.producers.iter().take(3) {
        flow_rows.push(TableRow::new(vec![
            TableCell::colored("生产方", palette::GOOD),
            TableCell::new(source.name.as_str()),
            TableCell::new(format!("{:.1}", source.amount)).right(),
        ]));
    }
    for source in good.consumers.iter().take(3) {
        flow_rows.push(TableRow::new(vec![
            TableCell::colored("消费方", palette::WARN),
            TableCell::new(source.name.as_str()),
            TableCell::new(format!("{:.1}", source.amount)).right(),
        ]));
    }
    for source in good.government_orders.iter().take(2) {
        flow_rows.push(TableRow::new(vec![
            TableCell::colored("订单", palette::GOLD),
            TableCell::new(source.name.as_str()),
            TableCell::new(format!("{:.1}", source.amount)).right(),
        ]));
    }
    DataTable::new(
        vec![
            TableColumn::new("方向", 0.8),
            TableColumn::new("来源", 1.4),
            TableColumn::new("流量", 0.7).right(),
        ],
        flow_rows,
    )
    .row_height(26.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), y),
            Pos2::new(inner.right(), inner.bottom() - 90.0),
        ),
    );

    let recommendation = recommendation_for(good);
    let footer = if data.alerts.is_empty() {
        recommendation.to_owned()
    } else {
        format!("{} | Alerts: {}", recommendation, data.alerts.len())
    };
    let galley = ui.painter().layout(
        footer,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
        inner.width(),
    );
    ui.painter().galley(
        Pos2::new(inner.left(), inner.bottom() - 76.0),
        galley,
        palette::PARCHMENT_DIM,
    );
}

fn render_tabs(ui: &mut egui::Ui, tab: &mut MarketPanelTab) {
    ui.horizontal_wrapped(|ui| {
        tab_button(ui, tab, MarketPanelTab::Overview, "总览");
        tab_button(ui, tab, MarketPanelTab::Shortages, "短缺");
        tab_button(ui, tab, MarketPanelTab::Goods, "商品");
        tab_button(ui, tab, MarketPanelTab::IndustryChain, "产业链");
        tab_button(ui, tab, MarketPanelTab::MarketBloc, "市场圈");
        tab_button(ui, tab, MarketPanelTab::Subjects, "殖民/傀儡");
        tab_button(ui, tab, MarketPanelTab::Trade, "贸易");
        tab_button(ui, tab, MarketPanelTab::Actions, "行动建议");
    });
}

fn tab_button(ui: &mut egui::Ui, tab: &mut MarketPanelTab, target: MarketPanelTab, label: &str) {
    if ui.selectable_label(*tab == target, label).clicked() {
        *tab = target;
    }
}

fn shortage_amount(good: &GoodEntry) -> f32 {
    good.unmet_demand.max((good.demand - good.supply).max(0.0))
}

fn good_matches_search(good: &GoodEntry, search: &str) -> bool {
    let query = search.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    good.name.to_lowercase().contains(&query)
        || category_label(&good.category)
            .to_lowercase()
            .contains(&query)
        || good
            .producers
            .iter()
            .any(|row| row.name.to_lowercase().contains(&query))
        || good
            .consumers
            .iter()
            .any(|row| row.name.to_lowercase().contains(&query))
        || good
            .government_orders
            .iter()
            .any(|row| row.name.to_lowercase().contains(&query))
        || good
            .affected_buildings
            .iter()
            .any(|row| row.name.to_lowercase().contains(&query))
        || good
            .affected_pop_classes
            .iter()
            .any(|row| row.name.to_lowercase().contains(&query))
        || good
            .affected_demand_buckets
            .iter()
            .any(|row| row.name.to_lowercase().contains(&query))
        || good.shortage_reason.to_lowercase().contains(&query)
        || good.actionable_fixes.iter().any(|action| {
            action.title.to_lowercase().contains(&query)
                || action.description.to_lowercase().contains(&query)
        })
}

fn render_overview_tab(ui: &mut egui::Ui, data: &MarketPanelData) {
    if let Some(bloc) = &data.bloc {
        components::section(ui, "市场圈", |ui| {
            ui.label(RichText::new(&bloc.name).strong());
            ui.label(format!(
                "{}，领导国 {}，成员 {} 个",
                bloc.kind,
                bloc.leader_tag,
                bloc.members.len()
            ));
        });
    }

    components::section(ui, "市场诊断", |ui| {
        components::summary_strip(
            ui,
            &[
                (
                    "进口价值",
                    format!("{:.1} £/日", data.total_import_value_gbp),
                ),
                (
                    "出口价值",
                    format!("{:.1} £/日", data.total_export_value_gbp),
                ),
                (
                    "军工压力",
                    format!("{:.0}%", data.military_supply_pressure * 100.0),
                ),
            ],
        );
        for alert in &data.alerts {
            let color = match alert.severity {
                MarketAlertSeverity::Info => Color32::LIGHT_GRAY,
                MarketAlertSeverity::Warning => Color32::from_rgb(0xff, 0xc0, 0x60),
                MarketAlertSeverity::Critical => Color32::from_rgb(0xff, 0x70, 0x70),
            };
            ui.colored_label(color, RichText::new(&alert.title).strong());
            ui.label(
                RichText::new(&alert.description)
                    .small()
                    .color(Color32::LIGHT_GRAY),
            );
        }
    });

    let mut shortages: Vec<&GoodEntry> = data
        .goods
        .iter()
        .filter(|g| shortage_amount(g) > 0.0)
        .collect();
    shortages.sort_by(|a, b| {
        shortage_value(b)
            .partial_cmp(&shortage_value(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    render_ranked_goods(ui, "最严重短缺", &shortages, 5);

    let mut price_risers: Vec<&GoodEntry> = data.goods.iter().collect();
    price_risers.sort_by(|a, b| {
        price_ratio(b)
            .partial_cmp(&price_ratio(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    render_ranked_goods(ui, "价格压力最高", &price_risers, 5);
}

fn render_market_bloc_tab(ui: &mut egui::Ui, data: &MarketPanelData) {
    let Some(bloc) = &data.bloc else {
        components::empty_state(ui, "未加入市场圈", "当前国家没有帝国、阵营或共同市场归属。");
        return;
    };

    components::section(ui, "市场圈总览", |ui| {
        ui.label(RichText::new(&bloc.name).strong());
        components::summary_strip(
            ui,
            &[
                ("类型", bloc.kind.clone()),
                ("领导国", bloc.leader_tag.clone()),
                ("成员", format!("{}", bloc.members.len())),
                (
                    "内部贸易",
                    format!("{:.1} £/日", bloc.internal_trade_value_gbp),
                ),
                (
                    "外部贸易",
                    format!("{:.1} £/日", bloc.external_trade_value_gbp),
                ),
            ],
        );
    });

    components::section(ui, "成员贡献", |ui| {
        for member in &bloc.members {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&member.tag).strong());
                    ui.label(&member.relation);
                    ui.label(format!("准入 {:.0}%", member.market_access * 100.0));
                });
                ui.label(format!(
                    "供给贡献 {:.0} RM / 需求贡献 {:.0} RM",
                    member.contribution_supply_value_rm, member.contribution_demand_value_rm
                ));
                render_flow_section(
                    ui,
                    "战略商品",
                    &member.strategic_goods,
                    Color32::from_rgb(0xc9, 0xa5, 0x5b),
                );
            });
        }
    });
}

fn render_shortages_tab(ui: &mut egui::Ui, data: &MarketPanelData, search: &str) {
    let mut rows: Vec<&GoodEntry> = data
        .goods
        .iter()
        .filter(|good| shortage_amount(good) > 0.0 && good_matches_search(good, search))
        .collect();
    rows.sort_by(|a, b| {
        shortage_value(b)
            .partial_cmp(&shortage_value(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if rows.is_empty() {
        components::empty_state(ui, "暂无短缺", "当前没有匹配的未满足需求。");
        return;
    }
    for good in rows {
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.colored_label(
                    Color32::from_rgb(0xff, 0x80, 0x70),
                    RichText::new(&good.name).strong(),
                );
                ui.label(format!("缺口 {:.1}/日", shortage_amount(good)));
                ui.label(format!("库存 {:.1} 天", good.stockpile_coverage_days));
            });
            render_good_detail(ui, good);
            render_action_section(ui, "可执行修复", &good.actionable_fixes);
            ui.label(
                RichText::new(recommendation_for(good))
                    .small()
                    .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
            );
        });
    }
}

fn render_goods_tab(ui: &mut egui::Ui, data: &MarketPanelData, search: &str, shortage_first: bool) {
    let mut grouped: Vec<(GoodCategory, Vec<&GoodEntry>)> = Vec::new();
    let mut visible: Vec<&GoodEntry> = data
        .goods
        .iter()
        .filter(|good| good_matches_search(good, search))
        .collect();
    if shortage_first {
        visible.sort_by(|a, b| {
            shortage_amount(b)
                .partial_cmp(&shortage_amount(a))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    for good in visible {
        if let Some(slot) = grouped.iter_mut().find(|(c, _)| *c == good.category) {
            slot.1.push(good);
        } else {
            grouped.push((good.category, vec![good]));
        }
    }
    grouped.sort_by_key(|(c, _)| category_sort_order(c));

    if grouped.is_empty() {
        components::empty_state(ui, "没有匹配的商品", "可清空搜索或关闭筛选。");
    }
    for (cat, goods) in &grouped {
        ui.colored_label(
            category_color(cat),
            RichText::new(category_label(cat)).strong(),
        );
        ui.add_space(2.0);
        for good in goods {
            render_good_row(ui, good);
        }
        ui.add_space(6.0);
    }
}

fn render_industry_chain_tab(ui: &mut egui::Ui, data: &MarketPanelData, search: &str) {
    let mut rows: Vec<&GoodEntry> = data
        .goods
        .iter()
        .filter(|good| good_matches_search(good, search))
        .collect();
    rows.sort_by(|a, b| {
        shortage_amount(b)
            .partial_cmp(&shortage_amount(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for good in rows.into_iter().take(24) {
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(&good.name)
                    .strong()
                    .color(category_color(&good.category)),
            );
            ui.label(format!("上游：{}", display_chain(&good.upstream_goods)));
            ui.label(format!("下游：{}", display_chain(&good.downstream_goods)));
            if shortage_amount(good) > 0.0 {
                ui.label(
                    RichText::new(&good.shortage_reason)
                        .small()
                        .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
                );
                ui.colored_label(
                    Color32::from_rgb(0xff, 0x90, 0x70),
                    recommendation_for(good),
                );
                render_action_section(ui, "补缺方案", &good.actionable_fixes);
            }
        });
    }
}

fn render_trade_tab(ui: &mut egui::Ui, data: &MarketPanelData, search: &str) {
    components::section(ui, "贸易与成交", |ui| {
        components::summary_strip(
            ui,
            &[
                ("进口", format!("{:.1} £/日", data.total_import_value_gbp)),
                ("出口", format!("{:.1} £/日", data.total_export_value_gbp)),
                ("现金", format!("{:.0} RM", data.cash_rm)),
            ],
        );
    });
    for good in data.goods.iter().filter(|g| good_matches_search(g, search)) {
        if good.imports.abs() <= 0.001
            && good.exports.abs() <= 0.001
            && good.military_order_demand <= 0.001
        {
            continue;
        }
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(&good.name).strong());
            ui.label(format!(
                "进口 {:.1} / 出口 {:.1} / 封锁 {}",
                good.imports,
                good.exports,
                if good.is_blockaded { "是" } else { "否" }
            ));
            if good.military_order_demand > 0.0 {
                ui.label(format!(
                    "军购：想买 {:.1}，实际成交 {:.1}，未成交 {:.1}，付款 {:.0} RM",
                    good.military_order_demand,
                    good.traded,
                    (good.military_order_demand - good.traded).max(0.0),
                    good.paid_rm
                ));
            }
        });
    }
}

fn render_subjects_tab(ui: &mut egui::Ui, data: &MarketPanelData) {
    if data.subjects.is_empty() {
        components::empty_state(
            ui,
            "无殖民/傀儡经济",
            "当前国家没有可显示的 subject 经济贡献。",
        );
        return;
    }
    components::section(ui, "殖民与傀儡经济", |ui| {
        for subject in &data.subjects {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&subject.tag).strong());
                    ui.label(&subject.autonomy_level);
                    ui.label(format!(
                        "宗主资源份额 {:.0}%",
                        subject.master_resource_share * 100.0
                    ));
                });
                ui.label(format!(
                    "财政/外汇贡献 {:.1} £/日",
                    subject.fiscal_contribution_gbp
                ));
                ui.label(
                    RichText::new(&subject.risk)
                        .small()
                        .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
                );
                render_flow_section(
                    ui,
                    "资源贡献",
                    &subject.resource_contribution,
                    Color32::from_rgb(0xc9, 0xa5, 0x5b),
                );
            });
        }
    });
}

fn render_actions_tab(ui: &mut egui::Ui, data: &MarketPanelData) {
    if data.actions.is_empty() {
        components::empty_state(ui, "暂无行动建议", "当前市场没有需要立即处理的问题。");
        return;
    }
    components::section(ui, "行动建议", |ui| {
        let mut actions: Vec<&MarketActionEntry> = data.actions.iter().collect();
        actions.sort_by_key(|entry| entry.priority);
        for action in actions {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&action.title).strong());
                ui.label(
                    RichText::new(&action.description)
                        .small()
                        .color(Color32::LIGHT_GRAY),
                );
                if let Some(good_id) = &action.related_good_id {
                    ui.label(
                        RichText::new(format!("相关商品：{good_id}"))
                            .small()
                            .color(Color32::from_gray(150)),
                    );
                }
            });
        }
    });
}

fn render_ranked_goods(ui: &mut egui::Ui, title: &str, rows: &[&GoodEntry], limit: usize) {
    components::section(ui, title, |ui| {
        if rows.is_empty() {
            components::empty_state(ui, "无数据", "当前没有可显示项目。");
            return;
        }
        for good in rows.iter().take(limit) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&good.name).strong());
                ui.label(format!("缺口 {:.1}", shortage_amount(good)));
                ui.label(format!("价格 {:.1}x", price_ratio(good)));
            });
        }
    });
}

fn shortage_value(good: &GoodEntry) -> f64 {
    shortage_amount(good) as f64 * good.price as f64
}

fn price_ratio(good: &GoodEntry) -> f32 {
    if good.base_price > 0.0 {
        good.price / good.base_price
    } else {
        1.0
    }
}

fn display_chain(goods: &[String]) -> String {
    if goods.is_empty() {
        "无".to_owned()
    } else {
        goods.join(" -> ")
    }
}

fn recommendation_for(good: &GoodEntry) -> &'static str {
    if good.imports > 0.0 && good.is_blockaded {
        "建议：解除封锁、改走陆路或提高国内生产。"
    } else if good.domestic_production <= 0.0 {
        "建议：建设对应生产建筑或进口该商品。"
    } else if good.building_input_demand > good.pop_consumption_demand {
        "建议：扩建上游产线、切换生产方式或暂缓消耗该商品的建筑。"
    } else if good.pop_consumption_demand > 0.0 {
        "建议：扩大民生供给或进口，避免 POP 满意度下降。"
    } else {
        "建议：检查上游瓶颈、进口与政府订单规模。"
    }
}

fn supply_source_kind_label(kind: GoodSupplySourceKind) -> &'static str {
    match kind {
        GoodSupplySourceKind::Domestic => "国内",
        GoodSupplySourceKind::MarketBloc => "市场圈",
        GoodSupplySourceKind::Subject => "殖民/傀儡",
        GoodSupplySourceKind::WorldSpot => "世界现货",
        GoodSupplySourceKind::Stockpile => "库存",
    }
}

fn price_color(price: f32, base_price: f32) -> Color32 {
    if base_price <= 0.0 {
        return Color32::from_gray(200);
    }
    let ratio = price / base_price;
    if ratio <= 1.5 {
        Color32::from_rgb(0x60, 0xc0, 0x60)
    } else if ratio <= 2.0 {
        Color32::from_rgb(0xc0, 0xc0, 0x30)
    } else {
        Color32::from_rgb(0xc0, 0x40, 0x40)
    }
}

fn v9_category_color(cat: &GoodCategory) -> Color32 {
    use crate::v9::tokens::palette;
    match cat {
        GoodCategory::RawMaterial => palette::GOOD,
        GoodCategory::Intermediate => palette::INFO,
        GoodCategory::Consumer => palette::GOLD,
        GoodCategory::Luxury => palette::BRASS_BRIGHT,
        GoodCategory::Service => palette::COLD_ATOMIC,
        GoodCategory::MilitaryIntermediate => palette::BAD,
    }
}

fn v9_price_ratio_color(ratio: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if ratio <= 1.15 {
        palette::GOOD
    } else if ratio <= 1.75 {
        palette::WARN
    } else {
        palette::BAD
    }
}

fn v9_ratio_color(value: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if value >= 0.90 {
        palette::GOOD
    } else if value >= 0.70 {
        palette::WARN
    } else {
        palette::BAD
    }
}

fn v9_money_rm(value: f64) -> String {
    if value.abs() >= 1_000_000_000.0 {
        format!("{:.1}B RM", value / 1_000_000_000.0)
    } else if value.abs() >= 1_000_000.0 {
        format!("{:.1}M RM", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("{:.1}K RM", value / 1_000.0)
    } else {
        format!("{:.0} RM", value)
    }
}

fn v9_money_gbp(value: f64) -> String {
    if value.abs() >= 1_000_000_000.0 {
        format!("{:.1}B GBP", value / 1_000_000_000.0)
    } else if value.abs() >= 1_000_000.0 {
        format!("{:.1}M GBP", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("{:.1}K GBP", value / 1_000.0)
    } else {
        format!("{:.0} GBP", value)
    }
}

fn render_good_row(ui: &mut egui::Ui, good: &GoodEntry) {
    egui::CollapsingHeader::new(RichText::new(&good.name).color(Color32::from_gray(210)))
        .id_salt(format!("market_good_detail_{}", good.id))
        .show(ui, |ui| {
            render_good_detail(ui, good);
        })
        .header_response
        .on_hover_text(tr("v6_market_good_detail_hint"));

    ui.horizontal(|ui| {
        ui.add_space(18.0);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let total = good.supply + good.demand;
            let supply_ratio = if total > 0.0 {
                good.supply / total
            } else {
                0.5
            };

            let bar_w = 80.0;
            let bar_h = 14.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, bar_h), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                ui.painter().rect_filled(rect, 2.0, Color32::from_gray(50));
                let supply_w = bar_w * supply_ratio;
                if supply_w > 0.0 {
                    let supply_rect =
                        egui::Rect::from_min_size(rect.min, egui::vec2(supply_w, bar_h));
                    ui.painter()
                        .rect_filled(supply_rect, 2.0, Color32::from_rgb(0x50, 0x90, 0xc0));
                }
                let demand_w = bar_w - supply_w;
                if demand_w > 0.0 {
                    let demand_rect = egui::Rect::from_min_size(
                        rect.min + egui::vec2(supply_w, 0.0),
                        egui::vec2(demand_w, bar_h),
                    );
                    ui.painter()
                        .rect_filled(demand_rect, 2.0, Color32::from_rgb(0xc0, 0x70, 0x40));
                }
            }

            ui.label(
                RichText::new(format!("{:.1}", good.price))
                    .small()
                    .color(price_color(good.price, good.base_price)),
            );
        });
    });
    ui.add_space(1.0);
}

fn render_good_detail(ui: &mut egui::Ui, good: &GoodEntry) {
    ui.horizontal(|ui| {
        ui.label(format!("{}: {:.1}", tr("v6_supply"), good.supply));
        ui.label(format!("{}: {:.1}", tr("v6_demand"), good.demand));
        ui.label(format!("成交: {:.1}", good.traded));
        ui.label(format!("库存: {:.1}", good.stockpile));
        let shortage = shortage_amount(good);
        if shortage > 0.0 {
            ui.colored_label(
                Color32::from_rgb(0xc0, 0x40, 0x40),
                format!("{}: {:.1}", tr("v6_shortage"), shortage),
            );
        }
    });

    let shortage = shortage_amount(good);
    if shortage > 0.0 {
        ui.label(
            RichText::new(format!(
                "解释链：未满足需求 {:.1}/日，库存还能覆盖 {:.1} 天，优先查看消费者、政府采购、建造需求和进口/封锁。",
                shortage, good.stockpile_coverage_days
            ))
            .small()
            .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
        );
    } else {
        ui.label(
            RichText::new(format!(
                "解释链：当前供给和库存覆盖需求，库存覆盖 {:.1} 天，价格主要由供需比例、贸易和库存变化影响。",
                good.stockpile_coverage_days
            ))
                .small()
                .color(Color32::LIGHT_GRAY),
        );
    }

    ui.label(
        RichText::new(format!(
            "供给拆分：国内 {:.1} / 进口 {:.1} / 库存释放 {:.1}",
            good.domestic_production, good.imports, good.stockpile_draw
        ))
        .small()
        .color(Color32::LIGHT_GRAY),
    );

    if good.clearing_fulfilled > 0.0 || good.clearing_unmet > 0.0 {
        ui.label(
            RichText::new(format!(
                "清算：满足 {:.1} / 未满足 {:.1} / 短缺率 {:.0}%",
                good.clearing_fulfilled,
                good.clearing_unmet,
                good.clearing_shortage_ratio * 100.0,
            ))
            .small()
            .color(if good.clearing_shortage_ratio > 0.3 {
                Color32::from_rgb(0xff, 0x80, 0x80)
            } else if good.clearing_shortage_ratio > 0.1 {
                Color32::from_rgb(0xff, 0xc0, 0x60)
            } else {
                Color32::from_rgb(0x80, 0xc0, 0x80)
            }),
        );
    }

    if !good.shortage_reason.is_empty() {
        ui.label(
            RichText::new(&good.shortage_reason)
                .small()
                .color(if shortage > 0.0 {
                    Color32::from_rgb(0xff, 0xc0, 0x60)
                } else {
                    Color32::LIGHT_GRAY
                }),
        );
    }

    if !good.supply_sources.is_empty() {
        ui.label(
            RichText::new("供给来源")
                .strong()
                .color(Color32::from_rgb(0x70, 0xb0, 0x70)),
        );
        for source in good.supply_sources.iter().take(8) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(supply_source_kind_label(source.kind)).small());
                ui.label(RichText::new(&source.label).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.1}/日", source.amount)).small());
                });
            });
        }
    }
    ui.label(
        RichText::new(format!(
            "需求拆分：建筑 {:.1} / POP {:.1} / 军购 {:.1} / 建造 {:.1} / 出口 {:.1}",
            good.building_input_demand,
            good.pop_consumption_demand,
            good.military_order_demand,
            good.construction_demand,
            good.exports
        ))
        .small()
        .color(Color32::LIGHT_GRAY),
    );

    render_flow_section(
        ui,
        tr("v6_market_producers"),
        &good.producers,
        Color32::from_rgb(0x70, 0xb0, 0x70),
    );
    render_flow_section(
        ui,
        tr("v6_market_consumers"),
        &good.consumers,
        Color32::from_rgb(0xc0, 0x90, 0x50),
    );
    render_flow_section(
        ui,
        tr("government_orders"),
        &good.government_orders,
        Color32::from_rgb(0xb0, 0x80, 0x50),
    );
    render_flow_section(
        ui,
        "受影响需求桶",
        &good.affected_demand_buckets,
        Color32::from_rgb(0xff, 0xc0, 0x60),
    );

    if good.construction_demand > 0.0 {
        ui.label(format!(
            "{}: {:.1}",
            tr("v6_market_construction_demand"),
            good.construction_demand
        ));
        ui.label(
            RichText::new("建造需求来自当前施工队列，会推高机械等建材需求并传导到财政建造开支。")
                .small()
                .color(Color32::LIGHT_GRAY),
        );
    }

    if good.imports.abs() > 0.001 || good.exports.abs() > 0.001 || good.is_blockaded {
        ui.horizontal(|ui| {
            ui.label(format!("{}: {:.1}", tr("v6_imports"), good.imports));
            ui.label(format!("{}: {:.1}", tr("v6_exports"), good.exports));
            if good.is_blockaded {
                ui.colored_label(Color32::from_rgb(0xc0, 0x40, 0x40), tr("v6_blockaded"));
            }
        });
        if good.is_blockaded {
            ui.label(
                RichText::new("封锁影响：进口无法稳定补足缺口，相关建筑和财政外汇压力会上升。")
                    .small()
                    .color(Color32::from_rgb(0xff, 0x80, 0x80)),
            );
        }
    }

    if !good.affected_buildings.is_empty() {
        render_flow_section(
            ui,
            tr("v6_market_affected_buildings"),
            &good.affected_buildings,
            Color32::from_rgb(0xc0, 0x50, 0x50),
        );
    }
    if !good.affected_pop_classes.is_empty() {
        render_flow_section(
            ui,
            "受影响 POP",
            &good.affected_pop_classes,
            Color32::from_rgb(0xc0, 0x80, 0x50),
        );
    }
    render_action_section(ui, "可建设解决方案", &good.actionable_fixes);
}

fn render_action_section(ui: &mut egui::Ui, title: &str, rows: &[MarketActionEntry]) {
    if rows.is_empty() {
        return;
    }
    ui.label(
        RichText::new(title)
            .strong()
            .color(Color32::from_rgb(0x90, 0xc0, 0x80)),
    );
    for action in rows.iter().take(4) {
        ui.label(RichText::new(&action.title).small().strong());
        ui.label(
            RichText::new(&action.description)
                .small()
                .color(Color32::LIGHT_GRAY),
        );
    }
}

fn render_flow_section(ui: &mut egui::Ui, title: &str, rows: &[GoodFlowSource], color: Color32) {
    ui.label(RichText::new(title).strong().color(color));
    if rows.is_empty() {
        ui.label(
            RichText::new(tr("v6_market_no_sources"))
                .small()
                .color(Color32::from_gray(150)),
        );
        return;
    }
    for row in rows.iter().take(6) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&row.name).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{:.1}/日", row.amount)).small());
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_entry_constructs() {
        let g = GoodEntry {
            id: "steel".into(),
            name: "钢材".into(),
            category: GoodCategory::Intermediate,
            price: 12.0,
            base_price: 10.0,
            supply: 50.0,
            demand: 40.0,
            traded: 40.0,
            stockpile: 10.0,
            stockpile_coverage_days: 0.25,
            unmet_demand: 0.0,
            domestic_production: 50.0,
            stockpile_draw: 0.0,
            building_input_demand: 20.0,
            pop_consumption_demand: 0.0,
            military_order_demand: 0.0,
            supply_sources: vec![],
            producers: vec![],
            consumers: vec![],
            government_orders: vec![],
            construction_demand: 0.0,
            imports: 0.0,
            exports: 0.0,
            is_blockaded: false,
            affected_buildings: vec![],
            affected_pop_classes: vec![],
            affected_demand_buckets: vec![],
            upstream_goods: vec![],
            downstream_goods: vec![],
            shortage_reason: "steel: stable".into(),
            actionable_fixes: vec![],
            paid_rm: 0.0,
            clearing_fulfilled: 40.0,
            clearing_unmet: 0.0,
            clearing_shortage_ratio: 0.0,
        };
        assert_eq!(g.id, "steel");
        assert_eq!(g.category, GoodCategory::Intermediate);
    }

    #[test]
    fn market_panel_data_constructs() {
        let d = MarketPanelData {
            goods: vec![],
            bloc: None,
            exchange_rate: 1.5,
            cash_rm: 1000.0,
            total_shortage_value_rm: 0.0,
            total_import_value_gbp: 0.0,
            total_export_value_gbp: 0.0,
            pop_needs_fulfillment: 1.0,
            military_supply_pressure: 0.0,
            subjects: vec![],
            actions: vec![],
            alerts: vec![],
        };
        assert_eq!(d.exchange_rate, 1.5);
        assert!(d.goods.is_empty());
    }

    #[test]
    fn price_color_near_base_is_green() {
        let c = price_color(10.0, 10.0);
        assert_eq!(c, Color32::from_rgb(0x60, 0xc0, 0x60));
    }

    #[test]
    fn price_color_over_2x_is_red() {
        let c = price_color(25.0, 10.0);
        assert_eq!(c, Color32::from_rgb(0xc0, 0x40, 0x40));
    }

    #[test]
    fn category_sort_order_raw_first() {
        assert!(
            category_sort_order(&GoodCategory::RawMaterial)
                < category_sort_order(&GoodCategory::Intermediate)
        );
    }

    #[test]
    fn market_v9_secondary_tabs_are_complete() {
        let ids: Vec<&str> = market_v9_tab_order()
            .into_iter()
            .map(MarketPanelTab::id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "overview",
                "shortages",
                "goods",
                "industry_chain",
                "trade",
                "market_bloc",
                "subjects",
                "actions",
            ]
        );
    }

    #[test]
    fn market_panel_splits_pop_consumption_demand() {
        let g = GoodEntry {
            id: "grain".into(),
            name: "粮食".into(),
            category: GoodCategory::Consumer,
            price: 2.0,
            base_price: 1.5,
            supply: 10.0,
            demand: 20.0,
            traded: 10.0,
            stockpile: 0.0,
            stockpile_coverage_days: 0.0,
            unmet_demand: 10.0,
            domestic_production: 10.0,
            stockpile_draw: 0.0,
            building_input_demand: 0.0,
            pop_consumption_demand: 12.0,
            military_order_demand: 0.0,
            supply_sources: vec![],
            producers: vec![],
            consumers: vec![],
            government_orders: vec![],
            construction_demand: 0.0,
            imports: 0.0,
            exports: 0.0,
            is_blockaded: false,
            affected_buildings: vec![],
            affected_pop_classes: vec![GoodFlowSource {
                name: "Worker".into(),
                amount: 12.0,
            }],
            affected_demand_buckets: vec![GoodFlowSource {
                name: "POP 基础消费".into(),
                amount: 10.0,
            }],
            upstream_goods: vec![],
            downstream_goods: vec![],
            shortage_reason: "grain shortage affects Worker POP".into(),
            actionable_fixes: vec![],
            paid_rm: 0.0,
            clearing_fulfilled: 10.0,
            clearing_unmet: 10.0,
            clearing_shortage_ratio: 0.5,
        };
        assert_eq!(shortage_amount(&g), 10.0);
        assert!(g.pop_consumption_demand > 0.0);
        assert_eq!(g.affected_pop_classes[0].name, "Worker");
    }

    #[test]
    fn market_panel_distinguishes_order_from_traded_volume() {
        let g = GoodEntry {
            id: "gun_barrels".into(),
            name: "炮管".into(),
            category: GoodCategory::MilitaryIntermediate,
            price: 16.0,
            base_price: 16.0,
            supply: 2.0,
            demand: 10.0,
            traded: 2.0,
            stockpile: 0.0,
            stockpile_coverage_days: 0.0,
            unmet_demand: 8.0,
            domestic_production: 2.0,
            stockpile_draw: 0.0,
            building_input_demand: 0.0,
            pop_consumption_demand: 0.0,
            military_order_demand: 10.0,
            supply_sources: vec![],
            producers: vec![],
            consumers: vec![],
            government_orders: vec![GoodFlowSource {
                name: "火炮生产 Mk1".into(),
                amount: 10.0,
            }],
            construction_demand: 0.0,
            imports: 0.0,
            exports: 0.0,
            is_blockaded: false,
            affected_buildings: vec![],
            affected_pop_classes: vec![],
            affected_demand_buckets: vec![GoodFlowSource {
                name: "军工投入".into(),
                amount: 8.0,
            }],
            upstream_goods: vec!["steel".into(), "machine_tools".into()],
            downstream_goods: vec!["artillery".into()],
            shortage_reason: "gun barrels shortage affects military orders".into(),
            actionable_fixes: vec![MarketActionEntry {
                title: "建设军工厂".into(),
                description: "提高火炮部件产能".into(),
                related_good_id: Some("gun_barrels".into()),
                priority: 0,
            }],
            paid_rm: 32.0,
            clearing_fulfilled: 2.0,
            clearing_unmet: 8.0,
            clearing_shortage_ratio: 0.8,
        };
        assert!(g.military_order_demand > g.traded);
        assert_eq!(shortage_amount(&g), 8.0);
        assert!(g.paid_rm > 0.0);
        assert!(g.shortage_reason.contains("military"));
        assert_eq!(g.affected_demand_buckets[0].name, "军工投入");
        assert_eq!(
            g.actionable_fixes[0].related_good_id.as_deref(),
            Some("gun_barrels")
        );
        assert!(good_matches_search(&g, "军工厂"));
    }
}
