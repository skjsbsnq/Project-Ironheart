//! V7.I1 POP 面板只读版。
//!
//! 先让玩家能看到 POP 的总量、阶级结构和州分布，不接入新的经济或政治规则。

use crate::{components, data_table, i18n::tr};
use egui::{Color32, RichText};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopIntegrationKind {
    Domestic,
    Colonial,
}

impl PopIntegrationKind {
    fn matches_filter(self, filter: PopIntegrationFilter) -> bool {
        matches!(
            (self, filter),
            (_, PopIntegrationFilter::All)
                | (Self::Domestic, PopIntegrationFilter::Domestic)
                | (Self::Colonial, PopIntegrationFilter::Colonial)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopIntegrationFilter {
    All,
    Domestic,
    Colonial,
}

impl PopIntegrationFilter {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Domestic,
            2 => Self::Colonial,
            _ => Self::All,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::All => 0,
            Self::Domestic => 1,
            Self::Colonial => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PopClassEntry {
    pub class_name: String,
    pub size: u64,
    pub employed: u64,
    pub unemployed: u64,
    pub avg_wage_rm: f32,
    pub avg_tax_burden: f32,
    pub avg_income_rm: f32,
    pub avg_tax_paid_rm: f32,
    pub avg_disposable_income_rm: f32,
    pub avg_satisfaction: f32,
    pub avg_loyalty: f32,
    pub avg_standard_of_living: f32,
    pub literacy: f32,
    pub skilled_ratio: f32,
    pub needs_fulfillment: f32,
    pub essential_needs_fulfillment: f32,
    pub normal_needs_fulfillment: f32,
    pub luxury_needs_fulfillment: f32,
    pub radicalism: f32,
}

#[derive(Debug, Clone)]
pub struct PopNeedEntry {
    pub tier_name: String,
    pub fulfillment: f32,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct PopPoliticalPressureEntry {
    pub source: String,
    pub pressure: f32,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct PopStateEntry {
    pub state_name: String,
    pub state_id: u16,
    pub integration_kind: PopIntegrationKind,
    pub integration_label: String,
    pub population: u64,
    pub employed: u64,
    pub unemployment_rate: f32,
    pub avg_satisfaction: f32,
    pub avg_wage_rm: f32,
    pub avg_income_rm: f32,
    pub avg_disposable_income_rm: f32,
    pub dominant_class: String,
}

#[derive(Debug, Clone)]
pub struct PopPanelData {
    pub total_population: u64,
    pub workforce: u64,
    pub employed: u64,
    pub unemployed: u64,
    pub unemployment_rate: f32,
    pub average_wage_rm: f32,
    pub average_income_rm: f32,
    pub average_disposable_income_rm: f32,
    pub average_satisfaction: f32,
    pub average_loyalty: f32,
    pub average_standard_of_living: f32,
    pub literacy: f32,
    pub skilled_ratio: f32,
    pub needs_fulfillment: f32,
    pub essential_needs_fulfillment: f32,
    pub normal_needs_fulfillment: f32,
    pub luxury_needs_fulfillment: f32,
    pub radicalism: f32,
    pub strike_risk: f32,
    pub draft_resistance: f32,
    pub soldier_pool: u64,
    pub classes: Vec<PopClassEntry>,
    pub states: Vec<PopStateEntry>,
    pub needs: Vec<PopNeedEntry>,
    pub political_pressures: Vec<PopPoliticalPressureEntry>,
    pub alerts: Vec<String>,
}

pub struct PopPanel;

impl PopPanel {
    #[allow(unreachable_code)]
    pub fn show(ctx: &egui::Context, data: &PopPanelData) -> (bool, Vec<()>) {
        return v9_show_pop(ctx, data);

        let mut close = false;

        egui::SidePanel::left("pop_panel")
            .default_width(720.0)
            .min_width(560.0)
            .resizable(true)
            .show(ctx, |ui| {
                components::panel_header(ui, tr("pops"), &mut close);
                components::summary_strip(
                    ui,
                    &[
                        ("总人口", format!("{}", data.total_population)),
                        ("劳动力", format!("{}", data.workforce)),
                        ("就业", format!("{}", data.employed)),
                        ("失业率", format!("{:.1}%", data.unemployment_rate * 100.0)),
                    ],
                );
                components::summary_strip(
                    ui,
                    &[
                        ("识字率", format!("{:.0}%", data.literacy * 100.0)),
                        ("熟练人口", format!("{:.0}%", data.skilled_ratio * 100.0)),
                        (
                            "资质瓶颈",
                            if data.skilled_ratio < 0.25 || data.literacy < 0.45 {
                                "严重".to_owned()
                            } else if data.skilled_ratio < 0.35 || data.literacy < 0.55 {
                                "中等".to_owned()
                            } else {
                                "可控".to_owned()
                            },
                        ),
                        ("教育增长", "大学/职员推动".to_owned()),
                    ],
                );
                components::summary_strip(
                    ui,
                    &[
                        ("激进化", format!("{:.0}%", data.radicalism * 100.0)),
                        ("罢工风险", format!("{:.0}%", data.strike_risk * 100.0)),
                        ("征兵抵抗", format!("{:.0}%", data.draft_resistance * 100.0)),
                        ("政治压力", format!("{} 项", data.political_pressures.len())),
                    ],
                );
                components::summary_strip(
                    ui,
                    &[
                        ("平均工资", format!("{:.1} RM", data.average_wage_rm)),
                        ("平均收入", format!("{:.1} RM", data.average_income_rm)),
                        (
                            "平均可支配",
                            format!("{:.1} RM", data.average_disposable_income_rm),
                        ),
                        ("士兵池", format!("{}", data.soldier_pool)),
                    ],
                );
                components::summary_strip(
                    ui,
                    &[
                        (
                            "平均满意度",
                            format!("{:.0}%", data.average_satisfaction * 100.0),
                        ),
                        ("平均忠诚", format!("{:.0}%", data.average_loyalty * 100.0)),
                        (
                            "生活水平",
                            format!("{:.0}%", data.average_standard_of_living * 100.0),
                        ),
                        (
                            "需求满足",
                            format!("{:.0}%", data.needs_fulfillment * 100.0),
                        ),
                    ],
                );
                components::summary_strip(
                    ui,
                    &[
                        (
                            "基础需求",
                            format!("{:.0}%", data.essential_needs_fulfillment * 100.0),
                        ),
                        (
                            "普通需求",
                            format!("{:.0}%", data.normal_needs_fulfillment * 100.0),
                        ),
                        (
                            "奢侈需求",
                            format!("{:.0}%", data.luxury_needs_fulfillment * 100.0),
                        ),
                        (
                            "普通/奢侈",
                            format!(
                                "{:.0}% / {:.0}%",
                                data.normal_needs_fulfillment * 100.0,
                                data.luxury_needs_fulfillment * 100.0
                            ),
                        ),
                    ],
                );

                if !data.alerts.is_empty() {
                    ui.add_space(4.0);
                    for alert in &data.alerts {
                        ui.colored_label(Color32::from_rgb(0xc0, 0x60, 0x60), alert);
                    }
                }

                let zero_pop_states: Vec<_> =
                    data.states.iter().filter(|s| s.population == 0).collect();
                if !zero_pop_states.is_empty() {
                    ui.add_space(4.0);
                    ui.colored_label(
                        Color32::from_rgb(0xc0, 0x80, 0x40),
                        format!(
                            "⚠ {} 个州无人口数据（ID: {}）",
                            zero_pop_states.len(),
                            zero_pop_states
                                .iter()
                                .map(|s| s.state_id.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    );
                }

                ui.add_space(4.0);
                ui.separator();

                let filter_id = ui.make_persistent_id("pop_integration_filter");
                let mut filter_index = ctx
                    .data_mut(|data| data.get_persisted::<usize>(filter_id))
                    .unwrap_or(0);
                let mut filter = PopIntegrationFilter::from_index(filter_index);
                let domestic_population: u64 = data
                    .states
                    .iter()
                    .filter(|row| row.integration_kind == PopIntegrationKind::Domestic)
                    .map(|row| row.population)
                    .sum();
                let colonial_population: u64 = data
                    .states
                    .iter()
                    .filter(|row| row.integration_kind == PopIntegrationKind::Colonial)
                    .map(|row| row.population)
                    .sum();
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("人口口径").strong());
                    if ui
                        .selectable_label(filter == PopIntegrationFilter::All, "全部")
                        .clicked()
                    {
                        filter = PopIntegrationFilter::All;
                    }
                    if ui
                        .selectable_label(filter == PopIntegrationFilter::Domestic, "本土")
                        .clicked()
                    {
                        filter = PopIntegrationFilter::Domestic;
                    }
                    if ui
                        .selectable_label(filter == PopIntegrationFilter::Colonial, "殖民/占领")
                        .clicked()
                    {
                        filter = PopIntegrationFilter::Colonial;
                    }
                    ui.separator();
                    ui.label(format!("本土 {}", domestic_population));
                    ui.label(format!("殖民/占领 {}", colonial_population));
                });
                filter_index = filter.index();
                ctx.data_mut(|data| data.insert_persisted(filter_id, filter_index));

                egui::ScrollArea::vertical().show(ui, |ui| {
                    components::section(ui, "阶级", |ui| {
                        if data.classes.is_empty() {
                            components::empty_state(
                                ui,
                                "没有 POP 数据",
                                "当前国家尚未注入可统计的人口。 ",
                            );
                        } else {
                            egui::Grid::new("pop_class_grid")
                                .num_columns(18)
                                .spacing([10.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    data_table::header(ui, "阶级");
                                    data_table::header(ui, "人口");
                                    data_table::header(ui, "就业");
                                    data_table::header(ui, "失业");
                                    data_table::header(ui, "工资");
                                    data_table::header(ui, "税负");
                                    data_table::header(ui, "收入");
                                    data_table::header(ui, "缴税");
                                    data_table::header(ui, "可支配");
                                    data_table::header(ui, "满意度");
                                    data_table::header(ui, "忠诚");
                                    data_table::header(ui, "生活");
                                    data_table::header(ui, "识字");
                                    data_table::header(ui, "熟练");
                                    data_table::header(ui, "基础");
                                    data_table::header(ui, "普通");
                                    data_table::header(ui, "奢侈");
                                    data_table::header(ui, "激进");
                                    ui.end_row();

                                    for row in &data.classes {
                                        ui.label(RichText::new(&row.class_name).strong());
                                        ui.label(format!("{}", row.size));
                                        ui.label(format!("{}", row.employed));
                                        ui.label(format!("{}", row.unemployed));
                                        ui.label(format!("{:.1}", row.avg_wage_rm));
                                        ui.label(format!("{:.2}", row.avg_tax_burden));
                                        ui.label(format!("{:.1}", row.avg_income_rm));
                                        ui.label(format!("{:.2}", row.avg_tax_paid_rm));
                                        ui.label(format!("{:.1}", row.avg_disposable_income_rm));
                                        ui.label(format!("{:.0}%", row.avg_satisfaction * 100.0));
                                        ui.label(format!("{:.0}%", row.avg_loyalty * 100.0));
                                        ui.label(format!(
                                            "{:.0}%",
                                            row.avg_standard_of_living * 100.0
                                        ));
                                        ui.label(format!("{:.0}%", row.literacy * 100.0));
                                        ui.label(format!("{:.0}%", row.skilled_ratio * 100.0));
                                        ui.label(format!(
                                            "{:.0}%",
                                            row.essential_needs_fulfillment * 100.0
                                        ));
                                        ui.label(format!(
                                            "{:.0}%",
                                            row.normal_needs_fulfillment * 100.0
                                        ));
                                        ui.label(format!(
                                            "{:.0}%",
                                            row.luxury_needs_fulfillment * 100.0
                                        ));
                                        ui.label(format!("{:.0}%", row.radicalism * 100.0));
                                        ui.end_row();
                                    }
                                });
                        }
                    });

                    components::section(ui, "政治压力", |ui| {
                        if data.political_pressures.is_empty() {
                            components::empty_state(
                                ui,
                                "没有显著政治压力",
                                "POP 激进化处于可控范围。 ",
                            );
                        } else {
                            egui::Grid::new("pop_political_pressure_grid")
                                .num_columns(3)
                                .spacing([10.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    data_table::header(ui, "来源");
                                    data_table::header(ui, "压力");
                                    data_table::header(ui, "说明");
                                    ui.end_row();

                                    for row in &data.political_pressures {
                                        ui.label(RichText::new(&row.source).strong());
                                        ui.label(format!("{:.0}%", row.pressure * 100.0));
                                        ui.label(&row.description);
                                        ui.end_row();
                                    }
                                });
                        }
                    });

                    components::section(ui, "生活需求", |ui| {
                        if data.needs.is_empty() {
                            components::empty_state(
                                ui,
                                "没有需求数据",
                                "POP 需求尚未进入市场 tick。 ",
                            );
                        } else {
                            egui::Grid::new("pop_needs_grid")
                                .num_columns(3)
                                .spacing([10.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    data_table::header(ui, "层级");
                                    data_table::header(ui, "满足率");
                                    data_table::header(ui, "说明");
                                    ui.end_row();

                                    for row in &data.needs {
                                        ui.label(RichText::new(&row.tier_name).strong());
                                        ui.label(format!("{:.0}%", row.fulfillment * 100.0));
                                        ui.label(&row.description);
                                        ui.end_row();
                                    }
                                });
                        }
                    });

                    components::section(ui, "州分布", |ui| {
                        let filtered_states: Vec<_> = data
                            .states
                            .iter()
                            .filter(|row| row.integration_kind.matches_filter(filter))
                            .collect();
                        if filtered_states.is_empty() {
                            components::empty_state(
                                ui,
                                "没有州数据",
                                "当前口径下没有可统计的州人口。 ",
                            );
                        } else {
                            egui::Grid::new("pop_state_grid")
                                .num_columns(10)
                                .spacing([10.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    data_table::header(ui, "州");
                                    data_table::header(ui, "口径");
                                    data_table::header(ui, "人口");
                                    data_table::header(ui, "就业");
                                    data_table::header(ui, "失业率");
                                    data_table::header(ui, "平均工资");
                                    data_table::header(ui, "平均收入");
                                    data_table::header(ui, "可支配");
                                    data_table::header(ui, "满意度");
                                    data_table::header(ui, "主导阶级");
                                    ui.end_row();

                                    for row in filtered_states {
                                        ui.label(RichText::new(&row.state_name).strong());
                                        ui.label(&row.integration_label);
                                        ui.label(format!("{}", row.population));
                                        ui.label(format!("{}", row.employed));
                                        ui.label(format!("{:.1}%", row.unemployment_rate * 100.0));
                                        ui.label(format!("{:.1}", row.avg_wage_rm));
                                        ui.label(format!("{:.1}", row.avg_income_rm));
                                        ui.label(format!("{:.1}", row.avg_disposable_income_rm));
                                        ui.label(format!("{:.0}%", row.avg_satisfaction * 100.0));
                                        ui.label(&row.dominant_class);
                                        ui.end_row();
                                    }
                                });
                        }
                    });
                });
            });

        (close, Vec::new())
    }
}

fn v9_show_pop(ctx: &egui::Context, data: &PopPanelData) -> (bool, Vec<()>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let filter_id = egui::Id::new("pop_panel_v9_integration_filter");
    let mut filter = PopIntegrationFilter::from_index(
        ctx.data_mut(|data| data.get_persisted::<usize>(filter_id))
            .unwrap_or(0),
    );
    let accent = if data.radicalism >= 0.45 || data.strike_risk >= 0.35 {
        palette::BAD
    } else if data.unemployment_rate >= 0.12 || data.needs_fulfillment < 0.65 {
        palette::WARN
    } else {
        palette::GOOD
    };

    let (close, _) = PanelShell::new("pop_panel_v9", tr("pops"))
        .subtitle("Population structure / states / political pressure")
        .class(PanelClass::Economy)
        .accent(accent)
        .footer("Q Close  |  Select integration filter")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    ("Population", v9_count(data.total_population), palette::GOLD),
                    ("Workforce", v9_count(data.workforce), palette::INFO),
                    (
                        "Unemployed",
                        v9_count(data.unemployed),
                        v9_bad_percent_color(data.unemployment_rate),
                    ),
                    (
                        "Satisfaction",
                        v9_percent(data.average_satisfaction),
                        v9_good_percent_color(data.average_satisfaction),
                    ),
                    (
                        "Literacy",
                        v9_percent(data.literacy),
                        v9_good_percent_color(data.literacy),
                    ),
                    (
                        "Alerts",
                        data.alerts.len().to_string(),
                        if data.alerts.is_empty() {
                            palette::GOOD
                        } else {
                            palette::WARN
                        },
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "Class DataTable / State DataTable / Needs Progress",
                accent,
            );
            v9_pop_body(ui, layout.body, data, &mut filter);
        });

    ctx.data_mut(|data| data.insert_persisted(filter_id, filter.index()));
    (close, Vec::new())
}

fn v9_pop_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &PopPanelData,
    filter: &mut PopIntegrationFilter,
) {
    use crate::v9::layout::{GridLayout, Track};
    use crate::v9::tokens::spacing;

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.46), Track::Fr(0.54)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);

        let left_grid =
            GridLayout::new(vec![Track::Fr(0.58), Track::Fr(0.42)], vec![Track::Fr(1.0)])
                .with_gutter(0.0, spacing::S5);
        let left_cells = left_grid.measure(GridLayout::cell(&cells, 0, 0));
        v9_pop_class_table(ui, GridLayout::cell(&left_cells, 0, 0), data);
        v9_pop_pressure_table(ui, GridLayout::cell(&left_cells, 1, 0), data);

        let right_grid =
            GridLayout::new(vec![Track::Fr(0.62), Track::Fr(0.38)], vec![Track::Fr(1.0)])
                .with_gutter(0.0, spacing::S5);
        let right_cells = right_grid.measure(GridLayout::cell(&cells, 0, 1));
        v9_pop_state_table(ui, GridLayout::cell(&right_cells, 0, 0), data, filter);
        v9_pop_needs_panel(ui, GridLayout::cell(&right_cells, 1, 0), data);
    });
}

fn v9_pop_class_table(ui: &mut egui::Ui, rect: egui::Rect, data: &PopPanelData) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Class structure",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    if data.classes.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 34.0),
                inner.right_bottom(),
            ),
            "No POP data",
            "Population classes have not been generated yet.",
        );
        return;
    }

    let rows: Vec<TableRow> = data
        .classes
        .iter()
        .take(12)
        .map(|row| {
            let accent = if row.radicalism >= 0.35 {
                palette::BAD
            } else if row.needs_fulfillment < 0.65 || row.avg_satisfaction < 0.45 {
                palette::WARN
            } else {
                palette::GOOD
            };
            TableRow::new(vec![
                TableCell::strong(row.class_name.as_str()),
                TableCell::new(v9_count(row.size)).right(),
                TableCell::new(v9_count(row.unemployed)).right(),
                TableCell::new(format!("{:.1}", row.avg_wage_rm)).right(),
                TableCell::new(format!("{:.1}", row.avg_income_rm)).right(),
                TableCell::colored(
                    v9_percent(row.avg_satisfaction),
                    v9_good_percent_color(row.avg_satisfaction),
                )
                .right(),
                TableCell::colored(
                    v9_percent(row.needs_fulfillment),
                    v9_good_percent_color(row.needs_fulfillment),
                )
                .right(),
                TableCell::colored(
                    v9_percent(row.radicalism),
                    v9_bad_percent_color(row.radicalism),
                )
                .right(),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("Class", 1.05),
            TableColumn::new("Pop", 0.72).right(),
            TableColumn::new("Unemp", 0.72).right(),
            TableColumn::new("Wage", 0.58).right(),
            TableColumn::new("Income", 0.66).right(),
            TableColumn::new("Sat", 0.52).right(),
            TableColumn::new("Needs", 0.58).right(),
            TableColumn::new("Rad", 0.52).right(),
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

fn v9_pop_pressure_table(ui: &mut egui::Ui, rect: egui::Rect, data: &PopPanelData) {
    use crate::v9::primitives::{Card, DataTable, TableCell, TableColumn, TableRow};
    use crate::v9::tokens::{palette, TextRole};
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Political pressure",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    if data.political_pressures.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 34.0),
                inner.right_bottom(),
            ),
            "No pressure",
            "POP radicalization is currently controlled.",
        );
        return;
    }

    let rows: Vec<TableRow> = data
        .political_pressures
        .iter()
        .take(8)
        .map(|row| {
            let accent = v9_bad_percent_color(row.pressure);
            TableRow::new(vec![
                TableCell::strong(row.source.as_str()),
                TableCell::colored(v9_percent(row.pressure), accent).right(),
                TableCell::new(row.description.as_str()),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("Source", 0.9),
            TableColumn::new("Pressure", 0.55).right(),
            TableColumn::new("Detail", 1.55),
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

fn v9_pop_state_table(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &PopPanelData,
    filter: &mut PopIntegrationFilter,
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
        "State distribution",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    for (idx, (next, label)) in [
        (PopIntegrationFilter::All, "All"),
        (PopIntegrationFilter::Domestic, "Core"),
        (PopIntegrationFilter::Colonial, "Colonial"),
    ]
    .iter()
    .enumerate()
    {
        let button_rect = Rect::from_min_size(
            Pos2::new(inner.left() + idx as f32 * 86.0, inner.top() + 30.0),
            Vec2::new(80.0, ButtonSize::Sm.min_size().y),
        );
        if Button::new(label)
            .size(ButtonSize::Sm)
            .variant(if *filter == *next {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Ghost
            })
            .show_at(ui, button_rect)
            .clicked()
        {
            *filter = *next;
        }
    }

    let filtered: Vec<&PopStateEntry> = data
        .states
        .iter()
        .filter(|row| row.integration_kind.matches_filter(*filter))
        .collect();
    if filtered.is_empty() {
        crate::v9::composites::panel_shell::draw_empty_state(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.top() + 62.0),
                inner.right_bottom(),
            ),
            "No states",
            "No state population matches the current filter.",
        );
        return;
    }

    let rows: Vec<TableRow> = filtered
        .into_iter()
        .take(12)
        .map(|row| {
            let accent = if row.population == 0 {
                palette::WARN
            } else {
                v9_good_percent_color(row.avg_satisfaction)
            };
            TableRow::new(vec![
                TableCell::strong(row.state_name.as_str()),
                TableCell::new(row.integration_label.as_str()),
                TableCell::new(v9_count(row.population)).right(),
                TableCell::new(v9_count(row.employed)).right(),
                TableCell::colored(
                    v9_percent(row.unemployment_rate),
                    v9_bad_percent_color(row.unemployment_rate),
                )
                .right(),
                TableCell::new(format!("{:.1}", row.avg_wage_rm)).right(),
                TableCell::colored(
                    v9_percent(row.avg_satisfaction),
                    v9_good_percent_color(row.avg_satisfaction),
                )
                .right(),
                TableCell::new(row.dominant_class.as_str()),
            ])
            .accent(accent)
        })
        .collect();

    DataTable::new(
        vec![
            TableColumn::new("State", 1.0),
            TableColumn::new("Type", 0.72),
            TableColumn::new("Pop", 0.74).right(),
            TableColumn::new("Emp", 0.70).right(),
            TableColumn::new("Unemp", 0.58).right(),
            TableColumn::new("Wage", 0.58).right(),
            TableColumn::new("Sat", 0.52).right(),
            TableColumn::new("Class", 0.9),
        ],
        rows,
    )
    .row_height(27.0)
    .show_at(
        ui,
        Rect::from_min_max(
            Pos2::new(inner.left(), inner.top() + 62.0 + spacing::S2),
            inner.right_bottom(),
        ),
    );
}

fn v9_pop_needs_panel(ui: &mut egui::Ui, rect: egui::Rect, data: &PopPanelData) {
    use crate::v9::primitives::{
        draw_progress_bar, Card, DataTable, TableCell, TableColumn, TableRow,
    };
    use crate::v9::tokens::{palette, spacing, TextRole};
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Needs and cohesion",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );

    let bars = [
        ("Essential", data.essential_needs_fulfillment),
        ("Normal", data.normal_needs_fulfillment),
        ("Luxury", data.luxury_needs_fulfillment),
        ("Loyalty", data.average_loyalty),
        ("Living standard", data.average_standard_of_living),
        ("Draft resistance", data.draft_resistance),
    ];
    let mut y = inner.top() + 34.0;
    for (label, value) in bars {
        let color = if label == "Draft resistance" {
            v9_bad_percent_color(value)
        } else {
            v9_good_percent_color(value)
        };
        ui.painter().text(
            Pos2::new(inner.left(), y),
            Align2::LEFT_TOP,
            label,
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        ui.painter().text(
            Pos2::new(inner.right(), y),
            Align2::RIGHT_TOP,
            v9_percent(value),
            TextRole::Numeric.font_id(),
            color,
        );
        draw_progress_bar(
            ui,
            Rect::from_min_size(
                Pos2::new(inner.left(), y + 16.0),
                Vec2::new(inner.width(), 9.0),
            ),
            value,
            color,
        );
        y += 34.0;
    }

    let rows: Vec<TableRow> = data
        .needs
        .iter()
        .take(5)
        .map(|need| {
            let accent = v9_good_percent_color(need.fulfillment);
            TableRow::new(vec![
                TableCell::strong(need.tier_name.as_str()),
                TableCell::colored(v9_percent(need.fulfillment), accent).right(),
                TableCell::new(need.description.as_str()),
            ])
            .accent(accent)
        })
        .collect();

    if !rows.is_empty() {
        DataTable::new(
            vec![
                TableColumn::new("Need", 0.72),
                TableColumn::new("Fill", 0.42).right(),
                TableColumn::new("Detail", 1.36),
            ],
            rows,
        )
        .row_height(27.0)
        .show_at(
            ui,
            Rect::from_min_max(
                Pos2::new(inner.left(), y + spacing::S2),
                inner.right_bottom(),
            ),
        );
    }
}

fn v9_count(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn v9_percent(value: f32) -> String {
    format!("{:.0}%", value.clamp(0.0, 9.99) * 100.0)
}

fn v9_good_percent_color(value: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if value >= 0.70 {
        palette::GOOD
    } else if value >= 0.45 {
        palette::WARN
    } else {
        palette::BAD
    }
}

fn v9_bad_percent_color(value: f32) -> Color32 {
    use crate::v9::tokens::palette;
    if value >= 0.30 {
        palette::BAD
    } else if value >= 0.12 {
        palette::WARN
    } else {
        palette::GOOD
    }
}
