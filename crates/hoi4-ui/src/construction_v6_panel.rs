//! V6 建筑与生产面板 UI：可建目录 + 真实建造队列 + 现有建筑概览。

use egui::{Color32, RichText};

use crate::{components, i18n::tr};

const GOLD: Color32 = components::GOLD;
const GOLD_BRIGHT: Color32 = components::GOLD_BRIGHT;
const MUTED: Color32 = components::MUTED;
const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const GOOD: Color32 = Color32::from_rgb(0x70, 0xc8, 0x78);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);

#[derive(Debug, Clone)]
pub struct BuildingStateV6Entry {
    pub building_idx: usize,
    pub state_name: String,
    pub level: u8,
    pub employment_rate: f32,
    pub profit_rm_weekly: f64,
    pub employment_gap: [u32; 6],
    pub is_law_blocked: bool,
    pub blocking_law: Option<String>,
    pub active_pm: String,
    pub pm_candidates: Vec<(String, String)>,
    pub pm_groups: Vec<ProductionMethodGroupV6Entry>,
}

#[derive(Debug, Clone)]
pub struct ProductionMethodGroupV6Entry {
    pub group_id: String,
    pub group_name: String,
    pub active_pm: String,
    pub candidates: Vec<ProductionMethodCandidateV6Entry>,
}

#[derive(Debug, Clone)]
pub struct ProductionMethodCandidateV6Entry {
    pub pm_id: String,
    pub pm_name: String,
    pub locked_reason: Option<String>,
    pub prediction: String,
}

#[derive(Debug, Clone)]
pub struct BuildingTypeV6Entry {
    pub building_def_id: String,
    pub building_name: String,
    pub total_level: u32,
    pub employment_rate: f32,
    pub profit_rm_weekly: f64,
    pub output_summary: String,
    pub input_summary: String,
    pub warnings: Vec<String>,
    pub pm_summary: String,
    pub states: Vec<BuildingStateV6Entry>,
}

#[derive(Debug, Clone)]
pub struct ConstructionQueueV6Entry {
    pub building_name: String,
    pub state_name: String,
    pub current_level: u8,
    pub target_level: u8,
    pub progress: f32,
    pub funding_source_label: String,
    pub owner_on_completion_label: String,
    pub paid_funds_rm: f64,
    pub budget_needed_rm: f64,
    pub material_fulfillment: f32,
    pub fund_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct BuildableBuildingEntry {
    pub building_def_id: String,
    pub building_name: String,
    pub group_name: String,
    pub locked_reason: Option<String>,
    pub state_limit_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AutoBuildExplanationEntry {
    pub building_name: String,
    pub state_name: String,
    pub score: f32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InvestmentPoolV6Data {
    pub total_rm: f64,
    pub private_rm: f64,
    pub cartel_rm: f64,
    pub state_development_bank_rm: f64,
    pub colonial_extraction_rm: f64,
    pub foreign_capital_rm: f64,
    pub income_rm: f64,
    pub spent_rm: f64,
}

#[derive(Debug, Clone)]
pub struct ConstructionV6PanelData {
    pub entries: Vec<BuildingTypeV6Entry>,
    pub queue: Vec<ConstructionQueueV6Entry>,
    pub buildable_catalog: Vec<BuildableBuildingEntry>,
    pub available_cp: f32,
    pub total_cp: f32,
    pub gdp_gbp: f64,
    pub gdp_growth_yoy: f32,
    pub construction_spend_rm: f64,
    pub unemployment_rate: f32,
    pub military_orders_rm: f64,
    pub mefo_risk: f32,
    pub auto_build_enabled: bool,
    pub auto_build_explanations: Vec<AutoBuildExplanationEntry>,
    pub investment_pool: InvestmentPoolV6Data,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstructionV6Command {
    ToggleAutoBuild(bool),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
    StartConstructionMode {
        building_key: String,
    },
    SwitchPM {
        building_idx: usize,
        group: String,
        pm_id: String,
    },
    SwitchPMNationwide {
        building_def_id: String,
        group: String,
        pm_id: String,
    },
}

const EMPLOYMENT_CLASS_COLORS: [Color32; 6] = [
    Color32::from_rgb(0x80, 0xc0, 0xf0),
    Color32::from_rgb(0xf0, 0xc0, 0x80),
    Color32::from_rgb(0x80, 0xf0, 0x80),
    Color32::from_rgb(0xf0, 0x80, 0x80),
    Color32::from_rgb(0xc0, 0x80, 0xf0),
    Color32::from_rgb(0xf0, 0xf0, 0x80),
];

pub struct ConstructionV6Panel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstructionPanelTab {
    Overview,
    Catalog,
    Queue,
    Existing,
    Problems,
}

impl ConstructionV6Panel {
    #[allow(unreachable_code)]
    pub fn show(
        ctx: &egui::Context,
        data: &ConstructionV6PanelData,
    ) -> (bool, Vec<ConstructionV6Command>) {
        return v9_show_construction(ctx, data);

        let mut close = false;
        let mut cmds: Vec<ConstructionV6Command> = Vec::new();

        let tab_id = egui::Id::new("buildings_panel_tab");
        let search_id = egui::Id::new("buildings_panel_search");
        let selected_building_id = egui::Id::new("buildings_panel_selected_building");
        let selected_catalog_id = egui::Id::new("buildings_panel_selected_catalog");
        let mut tab = ctx
            .data_mut(|d| d.get_persisted::<ConstructionPanelTab>(tab_id))
            .unwrap_or(ConstructionPanelTab::Catalog);
        let mut search = ctx
            .data_mut(|d| d.get_persisted::<String>(search_id))
            .unwrap_or_default();
        let mut selected_building = ctx
            .data_mut(|d| d.get_persisted::<Option<String>>(selected_building_id))
            .unwrap_or(None);
        let mut selected_catalog = ctx
            .data_mut(|d| d.get_persisted::<Option<String>>(selected_catalog_id))
            .unwrap_or(None);

        egui::SidePanel::left("buildings_panel")
            .default_width(760.0)
            .min_width(620.0)
            .max_width(980.0)
            .resizable(true)
            .show(ctx, |ui| {
                components::panel_header(ui, tr("buildings_panel_title"), &mut close);
                components::panel_hint(
                    ui,
                    "经济主入口：统一管理可建建筑、建造队列、现有建筑和生产方式",
                );
                ui.add_space(4.0);

                let cp_ratio = if data.total_cp > 0.0 {
                    data.available_cp / data.total_cp
                } else {
                    0.0
                };
                render_summary(ui, data);
                render_status_banner(ui, data, cp_ratio);
                ui.add_space(4.0);
                render_command_bar(ui, data, cp_ratio, &mut cmds);
                ui.add_space(4.0);
                ui.separator();

                render_tabs(ui, &mut tab, data);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("搜索").small().color(Color32::from_gray(150)));
                    ui.add(
                        egui::TextEdit::singleline(&mut search)
                            .hint_text("输入建筑、州、分组或警告")
                            .desired_width(260.0),
                    );
                    if !search.is_empty() && ui.small_button("清空").clicked() {
                        search.clear();
                    }
                });
                ui.separator();

                let tab_content_height = ui.available_height().max(200.0);

                match tab {
                    ConstructionPanelTab::Overview => {
                        render_overview_tab(ui, data, &mut cmds, tab_content_height);
                    }
                    ConstructionPanelTab::Catalog => {
                        render_catalog_tab(
                            ui,
                            data,
                            &search,
                            &mut selected_catalog,
                            &mut cmds,
                            tab_content_height,
                        );
                    }
                    ConstructionPanelTab::Queue => {
                        egui::ScrollArea::vertical()
                            .max_height(tab_content_height)
                            .show(ui, |ui| render_queue(ui, &data.queue, &mut cmds));
                    }
                    ConstructionPanelTab::Existing => {
                        render_existing_tab(
                            ui,
                            &data.entries,
                            &search,
                            false,
                            &mut selected_building,
                            &mut cmds,
                            tab_content_height,
                        );
                    }
                    ConstructionPanelTab::Problems => {
                        render_existing_tab(
                            ui,
                            &data.entries,
                            &search,
                            true,
                            &mut selected_building,
                            &mut cmds,
                            tab_content_height,
                        );
                    }
                }
            });

        ctx.data_mut(|d| {
            d.insert_persisted(tab_id, tab);
            d.insert_persisted(search_id, search);
            d.insert_persisted(selected_building_id, selected_building);
            d.insert_persisted(selected_catalog_id, selected_catalog);
        });

        (close, cmds)
    }
}

fn v9_show_construction(
    ctx: &egui::Context,
    data: &ConstructionV6PanelData,
) -> (bool, Vec<ConstructionV6Command>) {
    use crate::v9::composites::panel_shell::{
        draw_summary_tiles, draw_tab_strip, PanelClass, PanelShell,
    };
    use crate::v9::tokens::palette;

    let cp_ratio = if data.total_cp > 0.0 {
        (data.available_cp / data.total_cp).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let accent = if data.entries.iter().any(|entry| !entry.warnings.is_empty()) {
        palette::WARN
    } else if cp_ratio < 0.15 {
        palette::BAD
    } else {
        palette::GOLD
    };
    let (close, output) = PanelShell::new("construction_v6_panel_v9", tr("buildings_panel_title"))
        .subtitle("V6 economy")
        .class(PanelClass::Economy)
        .accent(accent)
        .footer("Q Close  |  Queue / Buildings / Investment")
        .show(ctx, |ui, layout| {
            draw_summary_tiles(
                ui,
                layout.summary,
                &[
                    (
                        tr("v6_construction_cp"),
                        format!("{:.0}/{:.0}", data.available_cp, data.total_cp),
                        if cp_ratio > 0.45 {
                            palette::GOOD
                        } else {
                            accent
                        },
                    ),
                    ("GDP", format_gbp(data.gdp_gbp), palette::GOLD),
                    (
                        tr("gdp_growth"),
                        format!("{:+.1}%", data.gdp_growth_yoy * 100.0),
                        if data.gdp_growth_yoy >= 0.0 {
                            palette::GOOD
                        } else {
                            palette::BAD
                        },
                    ),
                    (
                        tr("v6_construction_spend"),
                        format_rm(data.construction_spend_rm),
                        palette::WARN,
                    ),
                    (
                        tr("v6_unemployment"),
                        format!("{:.1}%", data.unemployment_rate * 100.0),
                        if data.unemployment_rate > 0.08 {
                            palette::WARN
                        } else {
                            palette::GOOD
                        },
                    ),
                    (
                        tr("construction_queue"),
                        data.queue.len().to_string(),
                        palette::PARCHMENT,
                    ),
                ],
            );
            draw_tab_strip(
                ui,
                layout.tabs,
                "Queue / Existing buildings / Investment pool",
                accent,
            );
            let mut cmds = Vec::new();
            v9_construction_body(ui, layout.body, data, cp_ratio, &mut cmds);
            cmds
        });
    (close, output.unwrap_or_default())
}

fn v9_construction_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    cp_ratio: f32,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        tokens::spacing,
    };

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.64), Track::Fr(0.36)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        let left = GridLayout::cell(&cells, 0, 0);
        let right = GridLayout::cell(&cells, 0, 1);
        let left_grid =
            GridLayout::new(vec![Track::Fr(0.44), Track::Fr(0.56)], vec![Track::Fr(1.0)])
                .with_gutter(0.0, spacing::S5);
        let left_cells = left_grid.measure(left);
        v9_construction_queue(ui, GridLayout::cell(&left_cells, 0, 0), data);
        v9_construction_buildings(ui, GridLayout::cell(&left_cells, 1, 0), data);
        v9_construction_side(ui, right, data, cp_ratio, cmds);
    });
}

fn v9_construction_queue(ui: &mut egui::Ui, rect: egui::Rect, data: &ConstructionV6PanelData) {
    use crate::v9::{
        primitives::{Card, DataTable, TableCell, TableColumn, TableRow},
        tokens::{palette, TextRole},
    };
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        tr("construction_queue"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let rows: Vec<TableRow> = data
        .queue
        .iter()
        .take(10)
        .map(|entry| {
            let progress = entry.progress.clamp(0.0, 1.0);
            let accent = if entry.material_fulfillment < 0.75 || entry.fund_ratio < 0.75 {
                palette::WARN
            } else {
                palette::GOOD
            };
            TableRow::new(vec![
                TableCell::strong(entry.building_name.as_str()),
                TableCell::new(entry.state_name.as_str()),
                TableCell::colored(format!("{:.0}%", progress * 100.0), accent).right(),
                TableCell::new(format!("{:.0}%", entry.material_fulfillment * 100.0)).right(),
                TableCell::new(format!("{:.0}%", entry.fund_ratio * 100.0)).right(),
                TableCell::new(entry.funding_source_label.as_str()),
            ])
            .accent(accent)
        })
        .collect();
    DataTable::new(
        vec![
            TableColumn::new(tr("buildable_buildings"), 1.3),
            TableColumn::new("State", 1.0),
            TableColumn::new(tr("progress"), 0.7).right(),
            TableColumn::new("Mat", 0.6).right(),
            TableColumn::new("Fund", 0.6).right(),
            TableColumn::new("Source", 1.0),
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

fn v9_construction_buildings(ui: &mut egui::Ui, rect: egui::Rect, data: &ConstructionV6PanelData) {
    use crate::v9::{
        primitives::{Card, DataTable, TableCell, TableColumn, TableRow},
        tokens::{palette, TextRole},
    };
    use egui::{Align2, Pos2, Rect};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        tr("existing_buildings"),
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let rows: Vec<TableRow> = data
        .entries
        .iter()
        .take(13)
        .map(|entry| {
            let accent = if !entry.warnings.is_empty() {
                palette::WARN
            } else if entry.profit_rm_weekly < 0.0 {
                palette::BAD
            } else {
                palette::GOOD
            };
            TableRow::new(vec![
                TableCell::strong(entry.building_name.as_str()),
                TableCell::new(entry.total_level.to_string()).right(),
                TableCell::new(format!("{:.0}%", entry.employment_rate * 100.0)).right(),
                TableCell::colored(signed_rm_stock(entry.profit_rm_weekly), accent).right(),
                TableCell::new(entry.output_summary.as_str()),
                TableCell::new(if entry.warnings.is_empty() { "-" } else { "!" }).center(),
            ])
            .accent(accent)
        })
        .collect();
    DataTable::new(
        vec![
            TableColumn::new(tr("existing_buildings"), 1.4),
            TableColumn::new("Lv", 0.4).right(),
            TableColumn::new(tr("employment_rate"), 0.7).right(),
            TableColumn::new(tr("building_profit_weekly"), 0.8).right(),
            TableColumn::new(tr("building_outputs"), 1.2),
            TableColumn::new("Warn", 0.5).center(),
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

fn v9_construction_side(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    cp_ratio: f32,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        primitives::{draw_progress_bar, Button, ButtonSize, ButtonVariant, Card, Tile},
        tokens::{palette, spacing, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "Investment pool",
        TextRole::Heading.font_id(),
        palette::BRASS_BRIGHT,
    );
    let bar_rect = Rect::from_min_size(
        Pos2::new(inner.left(), inner.top() + 30.0),
        Vec2::new(inner.width(), 10.0),
    );
    draw_progress_bar(
        ui,
        bar_rect,
        cp_ratio,
        if cp_ratio > 0.45 {
            palette::GOOD
        } else {
            palette::WARN
        },
    );

    let tile_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 52.0),
        Pos2::new(inner.right(), inner.top() + 198.0),
    );
    let grid = GridLayout::new(
        vec![Track::Fixed(44.0), Track::Fixed(44.0), Track::Fixed(44.0)],
        vec![Track::Fr(1.0), Track::Fr(1.0)],
    )
    .with_gutter(spacing::S4, spacing::S4);
    let cells = grid.measure(tile_rect);
    let pool = &data.investment_pool;
    let tiles = [
        ("Total", format_rm_stock(pool.total_rm), palette::GOLD),
        ("Private", format_rm_stock(pool.private_rm), palette::GOOD),
        ("Cartel", format_rm_stock(pool.cartel_rm), palette::WARN),
        (
            "Bank",
            format_rm_stock(pool.state_development_bank_rm),
            palette::INFO,
        ),
        (
            "Foreign",
            format_rm_stock(pool.foreign_capital_rm),
            palette::COLD_STEEL,
        ),
        ("Spent", format_rm_stock(pool.spent_rm), palette::BAD),
    ];
    for (idx, (label, value, color)) in tiles.iter().enumerate() {
        Tile::new(label, value)
            .accent(*color)
            .show_at(ui, GridLayout::cell(&cells, idx / 2, idx % 2));
    }

    let mut y = tile_rect.bottom() + spacing::S5;
    let toggle_label = if data.auto_build_enabled {
        "Disable auto"
    } else {
        "Enable auto"
    };
    if Button::new(toggle_label)
        .size(ButtonSize::Md)
        .variant(ButtonVariant::Secondary)
        .show_at(
            ui,
            Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(150.0, 30.0)),
        )
        .clicked()
    {
        cmds.push(ConstructionV6Command::ToggleAutoBuild(
            !data.auto_build_enabled,
        ));
    }
    y += 42.0;

    ui.painter().text(
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        tr("buildable_buildings"),
        TextRole::Subheading.font_id(),
        palette::GOLD,
    );
    y += 26.0;
    for entry in data.buildable_catalog.iter().take(4) {
        let row = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 30.0));
        let locked = entry.locked_reason.is_some() || entry.state_limit_reason.is_some();
        ui.painter().text(
            Pos2::new(row.left(), row.center().y),
            Align2::LEFT_CENTER,
            &entry.building_name,
            TextRole::Body.font_id(),
            if locked {
                palette::MUTED
            } else {
                palette::PARCHMENT
            },
        );
        if Button::new(tr("expand_one_level"))
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Primary)
            .enabled(!locked)
            .show_at(
                ui,
                Rect::from_min_size(
                    Pos2::new(row.right() - 88.0, row.top() + 3.0),
                    Vec2::new(84.0, 24.0),
                ),
            )
            .clicked()
        {
            cmds.push(ConstructionV6Command::StartConstructionMode {
                building_key: entry.building_def_id.clone(),
            });
        }
        y += 34.0;
    }

    if let Some(first) = data.queue.first() {
        y += spacing::S3;
        ui.painter().text(
            Pos2::new(inner.left(), y),
            Align2::LEFT_TOP,
            "Queue controls",
            TextRole::Subheading.font_id(),
            palette::GOLD,
        );
        y += 26.0;
        ui.painter().text(
            Pos2::new(inner.left(), y + 12.0),
            Align2::LEFT_CENTER,
            format!("{} - {}", first.building_name, first.state_name),
            TextRole::Caption.font_id(),
            palette::PARCHMENT_DIM,
        );
        let buttons = [
            (
                tr("move_up"),
                ConstructionV6Command::MoveUp(0),
                ButtonVariant::Secondary,
            ),
            (
                tr("move_down"),
                ConstructionV6Command::MoveDown(0),
                ButtonVariant::Secondary,
            ),
            (
                tr("remove"),
                ConstructionV6Command::Remove(0),
                ButtonVariant::Danger,
            ),
        ];
        for (idx, (label, cmd, variant)) in buttons.iter().enumerate() {
            if Button::new(label)
                .size(ButtonSize::Sm)
                .variant(*variant)
                .show_at(
                    ui,
                    Rect::from_min_size(
                        Pos2::new(inner.left() + idx as f32 * 86.0, y + 32.0),
                        Vec2::new(80.0, 24.0),
                    ),
                )
                .clicked()
            {
                cmds.push(cmd.clone());
            }
        }
    }
}

fn render_tabs(ui: &mut egui::Ui, tab: &mut ConstructionPanelTab, data: &ConstructionV6PanelData) {
    ui.horizontal_wrapped(|ui| {
        tab_button(ui, tab, ConstructionPanelTab::Catalog, "建造");
        tab_button(
            ui,
            tab,
            ConstructionPanelTab::Queue,
            &format!("建造队列 {}", data.queue.len()),
        );
        tab_button(ui, tab, ConstructionPanelTab::Overview, "总览");
        tab_button(ui, tab, ConstructionPanelTab::Existing, "现有建筑");
        tab_button(
            ui,
            tab,
            ConstructionPanelTab::Problems,
            &format!(
                "问题优先 {}",
                data.entries.iter().filter(|e| is_problem_entry(e)).count()
            ),
        );
    });
}

fn tab_button(
    ui: &mut egui::Ui,
    tab: &mut ConstructionPanelTab,
    target: ConstructionPanelTab,
    label: &str,
) {
    if ui.selectable_label(*tab == target, label).clicked() {
        *tab = target;
    }
}

fn render_overview_tab(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    cmds: &mut Vec<ConstructionV6Command>,
    available_height: f32,
) {
    let col_height = available_height - 28.0;
    ui.columns(2, |columns| {
        columns[0].label(
            RichText::new("待处理重点")
                .strong()
                .color(Color32::from_rgb(0xc9, 0xa5, 0x5b)),
        );
        let mut shown = 0;
        egui::ScrollArea::vertical()
            .max_height(col_height.max(160.0))
            .show(&mut columns[0], |ui| {
                for entry in data
                    .entries
                    .iter()
                    .filter(|entry| is_problem_entry(entry))
                    .take(8)
                {
                    shown += 1;
                    render_problem_card(ui, entry);
                }
                if shown == 0 {
                    ui.label(
                        RichText::new("暂无短缺、亏损或低就业建筑。可切换到可建建筑继续扩建。")
                            .small(),
                    );
                }
            });

        columns[1].label(
            RichText::new("当前建造队列")
                .strong()
                .color(Color32::from_rgb(0xc9, 0xa5, 0x5b)),
        );
        egui::ScrollArea::vertical()
            .max_height(col_height.max(160.0))
            .show(&mut columns[1], |ui| render_queue(ui, &data.queue, cmds));
    });
}

fn render_catalog_tab(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    search: &str,
    selected_catalog: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
    available_height: f32,
) {
    let visible: Vec<&BuildableBuildingEntry> = data
        .buildable_catalog
        .iter()
        .filter(|entry| catalog_matches_search(entry, search))
        .collect();

    egui::ScrollArea::vertical()
        .max_height(available_height.max(220.0))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if visible.is_empty() {
                components::empty_state(
                    ui,
                    "没有匹配的可建建筑",
                    "换一个建筑、州、分组或警告关键词。 ",
                );
                return;
            }

            let selected = selected_catalog
                .as_deref()
                .and_then(|id| {
                    visible
                        .iter()
                        .copied()
                        .find(|entry| entry.building_def_id == id)
                })
                .or_else(|| visible.first().copied());
            if let Some(entry) = selected {
                egui::Frame::new()
                    .fill(PANEL_CARD)
                    .stroke(egui::Stroke::new(1.0, STROKE_DARK))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| render_catalog_detail(ui, entry, data, cmds));
                ui.add_space(8.0);
            }

            ui.label(RichText::new("可建建筑").strong().color(GOLD));
            let column_count = (ui.available_width() / 220.0).floor().clamp(2.0, 4.0) as usize;
            for row in visible.chunks(column_count) {
                ui.columns(column_count, |columns| {
                    for (column, entry) in columns.iter_mut().zip(row.iter()) {
                        render_buildable_card(column, entry, selected_catalog, cmds);
                    }
                });
                ui.add_space(6.0);
            }
        });
}

fn render_buildable_card(
    ui: &mut egui::Ui,
    entry: &BuildableBuildingEntry,
    selected_catalog: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    let selected = selected_catalog.as_deref() == Some(entry.building_def_id.as_str());
    egui::Frame::new()
        .fill(if selected {
            PANEL_CARD_SOFT
        } else {
            PANEL_CARD
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected { GOLD } else { STROKE_DARK },
        ))
        .inner_margin(egui::Margin::symmetric(8, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if ui
                .selectable_label(selected, RichText::new(&entry.building_name).strong())
                .clicked()
            {
                *selected_catalog = Some(entry.building_def_id.clone());
            }
            ui.label(RichText::new(&entry.group_name).small().color(MUTED));
            if let Some(reason) = &entry.locked_reason {
                ui.label(RichText::new(reason).small().color(Color32::LIGHT_RED));
            } else if let Some(reason) = &entry.state_limit_reason {
                ui.label(RichText::new(reason).small().color(WARN));
            } else {
                ui.label(
                    RichText::new("可建造，点击后在地图选择州。 ")
                        .small()
                        .color(MUTED),
                );
            }
            if ui
                .add_enabled(entry.locked_reason.is_none(), egui::Button::new("进入建造"))
                .clicked()
            {
                *selected_catalog = Some(entry.building_def_id.clone());
                cmds.push(ConstructionV6Command::StartConstructionMode {
                    building_key: entry.building_def_id.clone(),
                });
            }
        });
}

fn render_existing_tab(
    ui: &mut egui::Ui,
    entries: &[BuildingTypeV6Entry],
    search: &str,
    problems_only: bool,
    selected_building: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
    available_height: f32,
) {
    let mut visible: Vec<&BuildingTypeV6Entry> = entries
        .iter()
        .filter(|entry| building_matches_search(entry, search))
        .filter(|entry| !problems_only || is_problem_entry(entry))
        .collect();
    visible.sort_by(|a, b| {
        problem_score(b)
            .cmp(&problem_score(a))
            .then(a.building_name.cmp(&b.building_name))
    });

    let col_height = available_height - 28.0;
    ui.columns(2, |columns| {
        columns[0].label(
            RichText::new(if problems_only {
                "问题建筑"
            } else {
                "建筑列表"
            })
            .strong()
            .color(Color32::from_rgb(0xc9, 0xa5, 0x5b)),
        );
        egui::ScrollArea::vertical()
            .max_height(col_height.max(160.0))
            .show(&mut columns[0], |ui| {
                if visible.is_empty() {
                    ui.label(if problems_only {
                        "暂无匹配的问题建筑。"
                    } else {
                        tr("no_existing_buildings")
                    });
                    return;
                }
                for entry in &visible {
                    let label = format!(
                        "{} Lv {}｜就业 {:.0}%｜{:+.1}M RM/周{}",
                        entry.building_name,
                        entry.total_level,
                        entry.employment_rate.clamp(0.0, 1.0) * 100.0,
                        entry.profit_rm_weekly / 1_000_000.0,
                        if entry.warnings.is_empty() {
                            ""
                        } else {
                            "｜警告"
                        },
                    );
                    if ui
                        .selectable_label(
                            selected_building.as_deref() == Some(entry.building_def_id.as_str()),
                            label,
                        )
                        .clicked()
                    {
                        *selected_building = Some(entry.building_def_id.clone());
                    }
                }
            });

        columns[1].label(
            RichText::new("建筑详情")
                .strong()
                .color(Color32::from_rgb(0xc9, 0xa5, 0x5b)),
        );
        egui::ScrollArea::vertical()
            .max_height(col_height.max(160.0))
            .show(&mut columns[1], |ui| {
                let selected = selected_building
                    .as_deref()
                    .and_then(|id| {
                        visible
                            .iter()
                            .copied()
                            .find(|entry| entry.building_def_id == id)
                    })
                    .or_else(|| visible.first().copied());
                if let Some(entry) = selected {
                    render_entry(ui, entry, cmds);
                } else {
                    ui.label("选择左侧建筑查看投入、产出、州分布和生产方式。");
                }
            });
    });
}

fn render_catalog_detail(
    ui: &mut egui::Ui,
    entry: &BuildableBuildingEntry,
    data: &ConstructionV6PanelData,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    ui.heading(&entry.building_name);
    ui.label(
        RichText::new(format!("分组：{}", entry.group_name))
            .small()
            .color(Color32::from_gray(160)),
    );
    if let Some(existing) = data
        .entries
        .iter()
        .find(|e| e.building_def_id == entry.building_def_id)
    {
        ui.label(format!(
            "当前规模：Lv {}，就业 {:.0}%，每周收支 {:+.1}M RM",
            existing.total_level,
            existing.employment_rate.clamp(0.0, 1.0) * 100.0,
            existing.profit_rm_weekly / 1_000_000.0,
        ));
        if !existing.output_summary.is_empty() {
            ui.label(format!("预计关联产出：{}", existing.output_summary));
        }
        if !existing.input_summary.is_empty() {
            ui.label(format!("主要投入品：{}", existing.input_summary));
        }
    } else {
        ui.label("当前国内尚无该建筑。建造后会加入现有建筑列表。形式收益取决于所选州、市场价格、劳动力和生产方式。 ");
    }
    ui.separator();

    if let Some(reason) = &entry.locked_reason {
        ui.label(
            RichText::new(format!("无法建造：{}", reason))
                .color(Color32::from_rgb(0xff, 0x80, 0x80)),
        );
    } else {
        if let Some(reason) = &entry.state_limit_reason {
            ui.label(
                RichText::new(format!("州限制：{}", reason))
                    .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
            );
        }
        ui.label(RichText::new("操作影响：进入地图建造模式，点击合规州后加入建造队列；会占用建造力并增加建造开支。 ").small());
        if ui.button(format!("建造 {}", entry.building_name)).clicked() {
            cmds.push(ConstructionV6Command::StartConstructionMode {
                building_key: entry.building_def_id.clone(),
            });
        }
    }
}

fn render_problem_card(ui: &mut egui::Ui, entry: &BuildingTypeV6Entry) {
    egui::Frame::group(ui.style())
        .inner_margin(6.0)
        .outer_margin(2.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} Lv {}", entry.building_name, entry.total_level)).strong(),
            );
            ui.label(
                RichText::new(format!(
                    "就业 {:.0}%｜每周收支 {:+.1}M RM",
                    entry.employment_rate.clamp(0.0, 1.0) * 100.0,
                    entry.profit_rm_weekly / 1_000_000.0,
                ))
                .small(),
            );
            if !entry.warnings.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for warning in &entry.warnings {
                        ui.label(
                            RichText::new(warning)
                                .small()
                                .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
                        );
                    }
                });
            }
        });
}

fn catalog_matches_search(entry: &BuildableBuildingEntry, search: &str) -> bool {
    let query = search.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    entry.building_name.to_lowercase().contains(&query)
        || entry.group_name.to_lowercase().contains(&query)
        || entry
            .locked_reason
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(&query)
        || entry
            .state_limit_reason
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(&query)
}

fn building_matches_search(entry: &BuildingTypeV6Entry, search: &str) -> bool {
    let query = search.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    entry.building_name.to_lowercase().contains(&query)
        || entry.output_summary.to_lowercase().contains(&query)
        || entry.input_summary.to_lowercase().contains(&query)
        || entry.pm_summary.to_lowercase().contains(&query)
        || entry
            .warnings
            .iter()
            .any(|warning| warning.to_lowercase().contains(&query))
        || entry
            .states
            .iter()
            .any(|state| state.state_name.to_lowercase().contains(&query))
}

fn is_problem_entry(entry: &BuildingTypeV6Entry) -> bool {
    !entry.warnings.is_empty()
        || entry.profit_rm_weekly < 0.0
        || entry.employment_rate < 0.8
        || entry.states.iter().any(|state| state.is_law_blocked)
}

fn problem_score(entry: &BuildingTypeV6Entry) -> u8 {
    let mut score = 0;
    if !entry.warnings.is_empty() {
        score += 4;
    }
    if entry.profit_rm_weekly < 0.0 {
        score += 3;
    }
    if entry.employment_rate < 0.8 {
        score += 2;
    }
    if entry.states.iter().any(|state| state.is_law_blocked) {
        score += 1;
    }
    score
}

fn render_summary(ui: &mut egui::Ui, data: &ConstructionV6PanelData) {
    components::summary_strip(
        ui,
        &[
            (tr("gdp"), format_gbp(data.gdp_gbp)),
            ("投资池", format_rm_stock(data.investment_pool.total_rm)),
            (tr("gdp_growth"), format!("{:+.1}%", data.gdp_growth_yoy)),
            (
                tr("v6_construction_spend"),
                format_rm(data.construction_spend_rm),
            ),
            (
                tr("v6_unemployment"),
                format!("{:.1}%", data.unemployment_rate * 100.0),
            ),
            ("投资流入", signed_rm_stock(data.investment_pool.income_rm)),
            ("投资支出", format_rm_stock(data.investment_pool.spent_rm)),
        ],
    );
}

fn render_status_banner(ui: &mut egui::Ui, data: &ConstructionV6PanelData, cp_ratio: f32) {
    let (label, text, color) = if data.investment_pool.total_rm <= 0.0 {
        (
            "投资池枯竭",
            "私人/法团/外资账户暂无可用资金，民间扩建会明显放慢。",
            WARN,
        )
    } else if data.auto_build_enabled {
        (
            "自动建造启用",
            "系统会根据短缺、军工和建设能力自动补入队列。",
            GOOD,
        )
    } else if cp_ratio < 0.3 {
        (
            "建造能力紧张",
            "可用建造能力偏低，建议先恢复建设预算或压缩支出。",
            WARN,
        )
    } else {
        (
            "建造体系稳定",
            "当前建造能力尚可，重点处理队列和问题建筑即可。",
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
                    RichText::new(format!(
                        "{}  现有建筑 {} 项  队列 {} 项",
                        text,
                        data.entries.len(),
                        data.queue.len()
                    ))
                    .small()
                    .color(Color32::from_rgb(0xe0, 0xd2, 0xa8)),
                );
            });
        });
}

fn render_command_bar(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    cp_ratio: f32,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    egui::Frame::new()
        .fill(PANEL_CARD)
        .stroke(egui::Stroke::new(1.0, STROKE_DARK))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.columns(3, |columns| {
                columns[0].label(RichText::new("建造力").small().color(MUTED));
                columns[0].add(
                    egui::ProgressBar::new(cp_ratio.clamp(0.0, 1.0))
                        .fill(GOLD_BRIGHT)
                        .text(format!("{:.0} / {:.0}", data.available_cp, data.total_cp)),
                );

                columns[1].label(RichText::new("投资池").small().color(MUTED));
                columns[1].label(
                    RichText::new(format_rm_stock(data.investment_pool.total_rm))
                        .strong()
                        .color(GOLD_BRIGHT),
                );
                columns[1].label(
                    RichText::new(format!(
                        "私人 {}  法团 {}  外资 {}",
                        format_rm_stock(data.investment_pool.private_rm),
                        format_rm_stock(data.investment_pool.cartel_rm),
                        format_rm_stock(data.investment_pool.foreign_capital_rm),
                    ))
                    .small()
                    .color(MUTED),
                );

                columns[2].horizontal_wrapped(|ui| {
                    let mut enabled = data.auto_build_enabled;
                    if ui.checkbox(&mut enabled, "自动建造").changed() {
                        cmds.push(ConstructionV6Command::ToggleAutoBuild(enabled));
                    }
                    ui.label(
                        RichText::new("每月按短缺、军工和建设能力补队列")
                            .small()
                            .color(MUTED),
                    );
                });
            });

            if data.investment_pool.state_development_bank_rm > 0.0
                || data.investment_pool.colonial_extraction_rm > 0.0
            {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "开发银行 {}  殖民抽取 {}  本期流入 {}  本期支出 {}",
                        format_rm_stock(data.investment_pool.state_development_bank_rm),
                        format_rm_stock(data.investment_pool.colonial_extraction_rm),
                        signed_rm_stock(data.investment_pool.income_rm),
                        format_rm_stock(data.investment_pool.spent_rm),
                    ))
                    .small()
                    .color(MUTED),
                );
            }

            render_auto_build_explanations(ui, data);
        });
}

fn render_auto_build_explanations(ui: &mut egui::Ui, data: &ConstructionV6PanelData) {
    if data.auto_build_explanations.is_empty() {
        return;
    }
    egui::CollapsingHeader::new("本月自动建造解释")
        .default_open(false)
        .show(ui, |ui| {
            for entry in data.auto_build_explanations.iter().take(5) {
                ui.label(
                    RichText::new(format!(
                        "{}：{}  评分 {:.0}",
                        entry.state_name, entry.building_name, entry.score
                    ))
                    .small()
                    .color(GOLD),
                );
                for reason in entry.reasons.iter().take(3) {
                    ui.label(RichText::new(format!("- {reason}")).small().color(MUTED));
                }
            }
        });
}

fn format_rm_stock(value: f64) -> String {
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

fn signed_rm_stock(value: f64) -> String {
    if value >= 0.0 {
        format!("+{}", format_rm_stock(value))
    } else {
        format!("-{}", format_rm_stock(-value))
    }
}

fn format_gbp(value: f64) -> String {
    if value.abs() >= 1_000_000_000.0 {
        format!("£{:.1}B", value / 1_000_000_000.0)
    } else if value.abs() >= 1_000_000.0 {
        format!("£{:.1}M", value / 1_000_000.0)
    } else {
        format!("£{:.0}", value)
    }
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

fn render_queue(
    ui: &mut egui::Ui,
    entries: &[ConstructionQueueV6Entry],
    cmds: &mut Vec<ConstructionV6Command>,
) {
    ui.label(RichText::new(tr("construction_queue")).strong());
    if entries.is_empty() {
        ui.label(tr("empty_construction"));
        return;
    }

    for (idx, entry) in entries.iter().enumerate() {
        egui::Frame::group(ui.style())
            .inner_margin(6.0)
            .outer_margin(2.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}. {}：{} Lv {} -> {}",
                        idx + 1,
                        entry.state_name,
                        entry.building_name,
                        entry.current_level,
                        entry.target_level,
                    ));
                });
                ui.add(
                    egui::ProgressBar::new(entry.progress.clamp(0.0, 1.0))
                        .text(format!("{:.0}%", entry.progress.clamp(0.0, 1.0) * 100.0))
                        .fill(Color32::from_rgb(0x60, 0x90, 0xc0)),
                );
                let fund_pct = if entry.budget_needed_rm > 0.0 {
                    (entry.paid_funds_rm / entry.budget_needed_rm * 100.0) as f32
                } else {
                    100.0
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "资金：{}  建成产权：{}",
                            entry.funding_source_label, entry.owner_on_completion_label
                        ))
                        .small()
                        .color(Color32::from_gray(160)),
                    );
                });
                if entry.budget_needed_rm > 0.0 {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "已拨 {:.1}M / {:.1}M RM ({:.0}%)",
                                entry.paid_funds_rm / 1_000_000.0,
                                entry.budget_needed_rm / 1_000_000.0,
                                fund_pct,
                            ))
                            .small()
                            .color(if fund_pct < 30.0 {
                                Color32::from_rgb(0xff, 0xc0, 0x60)
                            } else {
                                Color32::from_gray(160)
                            }),
                        );
                        if entry.fund_ratio < 1.0 {
                            ui.label(
                                RichText::new(format!(
                                    "资金短缺·速度 ×{:.0}%",
                                    entry.fund_ratio * 100.0
                                ))
                                .small()
                                .color(Color32::from_rgb(0xff, 0x80, 0x80)),
                            );
                        }
                    });
                }
                if entry.material_fulfillment < 1.0 {
                    ui.label(
                        RichText::new(format!(
                            "材料满足率 {:.0}%·速度受限",
                            entry.material_fulfillment * 100.0
                        ))
                        .small()
                        .color(WARN),
                    );
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(idx > 0, egui::Button::new(tr("move_up")))
                        .clicked()
                    {
                        cmds.push(ConstructionV6Command::MoveUp(idx));
                    }
                    if ui
                        .add_enabled(idx + 1 < entries.len(), egui::Button::new(tr("move_down")))
                        .clicked()
                    {
                        cmds.push(ConstructionV6Command::MoveDown(idx));
                    }
                    if ui.button(tr("cancel_construction")).clicked() {
                        cmds.push(ConstructionV6Command::Remove(idx));
                    }
                });
            });
    }
}

fn render_entry(
    ui: &mut egui::Ui,
    entry: &BuildingTypeV6Entry,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    egui::Frame::group(ui.style())
        .inner_margin(6.0)
        .outer_margin(2.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("{}  Lv {}", entry.building_name, entry.total_level))
                            .color(Color32::from_gray(220)),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} {:.0}%  {} {:+.1}M RM/周",
                            tr("employment_rate"),
                            entry.employment_rate.clamp(0.0, 1.0) * 100.0,
                            tr("building_profit_weekly"),
                            entry.profit_rm_weekly / 1_000_000.0,
                        ))
                        .small()
                        .color(Color32::from_gray(160)),
                    );
                });
            });

            if !entry.output_summary.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{}：{}",
                        tr("building_outputs"),
                        entry.output_summary
                    ))
                    .small(),
                );
            }
            if !entry.input_summary.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{}：{}",
                        tr("building_inputs"),
                        entry.input_summary
                    ))
                    .small(),
                );
            }
            if !entry.warnings.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for warning in &entry.warnings {
                        ui.label(
                            RichText::new(warning)
                                .small()
                                .color(Color32::from_rgb(0xff, 0xc0, 0x60)),
                        );
                    }
                });
            }
            ui.label(
                RichText::new(format!("{}：{}", tr("production_method"), entry.pm_summary))
                    .small()
                    .color(Color32::from_rgb(0xa0, 0xc0, 0xe0)),
            );

            for state in &entry.states {
                for group in &state.pm_groups {
                    let Some(active) = group
                        .candidates
                        .iter()
                        .find(|pm| pm.pm_id == group.active_pm)
                    else {
                        continue;
                    };
                    if group
                        .candidates
                        .iter()
                        .filter(|pm| pm.locked_reason.is_none())
                        .count()
                        <= 1
                    {
                        continue;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("全国{}", group.group_name)).small());
                        egui::ComboBox::from_id_salt((
                            "v6_pm_nationwide",
                            &entry.building_def_id,
                            &group.group_id,
                        ))
                        .selected_text(active.pm_name.as_str())
                        .show_ui(ui, |ui| {
                            for pm in &group.candidates {
                                let enabled = pm.locked_reason.is_none();
                                let label = if pm.prediction.is_empty() {
                                    pm.pm_name.clone()
                                } else {
                                    format!("{}｜{}", pm.pm_name, pm.prediction)
                                };
                                let response = ui.add_enabled(
                                    enabled,
                                    egui::SelectableLabel::new(pm.pm_id == group.active_pm, label),
                                );
                                if response.clicked() {
                                    cmds.push(ConstructionV6Command::SwitchPMNationwide {
                                        building_def_id: entry.building_def_id.clone(),
                                        group: group.group_id.clone(),
                                        pm_id: pm.pm_id.clone(),
                                    });
                                }
                                if let Some(reason) = &pm.locked_reason {
                                    ui.label(
                                        RichText::new(reason)
                                            .small()
                                            .color(Color32::from_rgb(0xff, 0x80, 0x80)),
                                    );
                                }
                            }
                        });
                    });
                    break;
                }
                break;
            }

            egui::CollapsingHeader::new(tr("state_distribution"))
                .id_salt(("v6_building_states", &entry.building_def_id))
                .show(ui, |ui| {
                    for state in &entry.states {
                        render_state_entry(ui, state, cmds);
                    }
                });

            if ui.button(tr("expand_one_level")).clicked() {
                cmds.push(ConstructionV6Command::StartConstructionMode {
                    building_key: entry.building_def_id.clone(),
                });
            }
        });

    ui.add_space(2.0);
}

fn render_state_entry(
    ui: &mut egui::Ui,
    entry: &BuildingStateV6Entry,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    let bg_tint = if entry.is_law_blocked {
        Color32::from_rgba_premultiplied(60, 20, 20, 40)
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame::group(ui.style())
        .inner_margin(5.0)
        .outer_margin(1.0)
        .fill(bg_tint)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{}  Lv {}  {} {:.0}%  {:+.1}M RM/周",
                        entry.state_name,
                        entry.level,
                        tr("employment_rate"),
                        entry.employment_rate.clamp(0.0, 1.0) * 100.0,
                        entry.profit_rm_weekly / 1_000_000.0,
                    ))
                    .small(),
                );
            });

            if let Some(ref law) = entry.blocking_law {
                ui.label(
                    RichText::new(format!("{}: {}", tr("v6_construction_blocked_by"), law))
                        .small()
                        .color(Color32::from_rgb(0xff, 0x80, 0x80)),
                );
            }

            let has_gap = entry.employment_gap.iter().any(|&g| g > 0);
            if has_gap {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(tr("v6_construction_employment_gap"))
                            .small()
                            .color(Color32::from_gray(140)),
                    );
                    for (ci, &gap) in entry.employment_gap.iter().enumerate() {
                        if gap > 0 {
                            ui.label(
                                RichText::new(format!("C{}:{}", ci + 1, gap))
                                    .small()
                                    .color(EMPLOYMENT_CLASS_COLORS[ci]),
                            );
                        }
                    }
                });
            }

            ui.horizontal(|ui| {
                let selected_name = entry
                    .pm_candidates
                    .iter()
                    .find(|(pm_id, _)| pm_id == &entry.active_pm)
                    .map(|(_, name)| name.as_str())
                    .unwrap_or(entry.active_pm.as_str());
                ui.label(
                    RichText::new(format!("{}: {}", tr("production_method"), selected_name))
                        .small(),
                );
                if !entry.pm_groups.is_empty() {
                    ui.add_enabled_ui(!entry.is_law_blocked, |ui| {
                        for group in &entry.pm_groups {
                            let selected = group
                                .candidates
                                .iter()
                                .find(|pm| pm.pm_id == group.active_pm)
                                .map(|pm| pm.pm_name.as_str())
                                .unwrap_or(group.active_pm.as_str());
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(format!("{}: {}", group.group_name, selected))
                                        .small(),
                                );
                                egui::ComboBox::from_id_salt((
                                    "v6_pm_select",
                                    entry.building_idx,
                                    &group.group_id,
                                ))
                                .selected_text(selected)
                                .show_ui(ui, |ui| {
                                    for pm in &group.candidates {
                                        let enabled = pm.locked_reason.is_none();
                                        let text = if pm.prediction.is_empty() {
                                            pm.pm_name.clone()
                                        } else {
                                            format!("{}｜{}", pm.pm_name, pm.prediction)
                                        };
                                        let response = ui.add_enabled(
                                            enabled,
                                            egui::SelectableLabel::new(
                                                pm.pm_id == group.active_pm,
                                                text,
                                            ),
                                        );
                                        if response.clicked() {
                                            cmds.push(ConstructionV6Command::SwitchPM {
                                                building_idx: entry.building_idx,
                                                group: group.group_id.clone(),
                                                pm_id: pm.pm_id.clone(),
                                            });
                                        }
                                        if let Some(reason) = &pm.locked_reason {
                                            ui.label(
                                                RichText::new(reason)
                                                    .small()
                                                    .color(Color32::from_rgb(0xff, 0x80, 0x80)),
                                            );
                                        }
                                    }
                                });
                            });
                        }
                    });
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state_entry() -> BuildingStateV6Entry {
        BuildingStateV6Entry {
            building_idx: 0,
            state_name: "莱茵兰".into(),
            level: 2,
            employment_rate: 0.8,
            profit_rm_weekly: 2_100_000.0,
            employment_gap: [10, 0, 5, 0, 0, 0],
            is_law_blocked: false,
            blocking_law: None,
            active_pm: "pm_steel_basic".into(),
            pm_candidates: vec![
                ("pm_steel_basic".into(), "基础炼钢".into()),
                ("pm_steel_advanced".into(), "先进炼钢".into()),
            ],
            pm_groups: vec![ProductionMethodGroupV6Entry {
                group_id: "base".into(),
                group_name: "基础工艺".into(),
                active_pm: "pm_steel_basic".into(),
                candidates: vec![ProductionMethodCandidateV6Entry {
                    pm_id: "pm_steel_basic".into(),
                    pm_name: "基础炼钢".into(),
                    locked_reason: None,
                    prediction: "产出 钢材 +10/d".into(),
                }],
            }],
        }
    }

    fn sample_entry() -> BuildingTypeV6Entry {
        BuildingTypeV6Entry {
            building_def_id: "steel_mill".into(),
            building_name: "钢铁厂".into(),
            total_level: 2,
            employment_rate: 0.8,
            profit_rm_weekly: 2_100_000.0,
            output_summary: "钢材 +40/d".into(),
            input_summary: "煤炭 -20/d".into(),
            warnings: vec!["投入品短缺：煤炭 12%".into()],
            pm_summary: "基础炼钢".into(),
            states: vec![sample_state_entry()],
        }
    }

    #[test]
    fn sample_entry_constructs() {
        let e = sample_entry();
        assert_eq!(e.total_level, 2);
        assert_eq!(e.states.len(), 1);
        assert!(!e.states[0].is_law_blocked);
    }

    #[test]
    fn command_equality() {
        assert_eq!(
            ConstructionV6Command::MoveUp(1),
            ConstructionV6Command::MoveUp(1)
        );
        assert_eq!(
            ConstructionV6Command::StartConstructionMode {
                building_key: "steel_mill".into()
            },
            ConstructionV6Command::StartConstructionMode {
                building_key: "steel_mill".into()
            },
        );
    }

    #[test]
    fn panel_data_constructs() {
        let data = ConstructionV6PanelData {
            entries: vec![sample_entry()],
            queue: vec![ConstructionQueueV6Entry {
                building_name: "钢铁厂".into(),
                state_name: "莱茵兰".into(),
                current_level: 2,
                target_level: 3,
                progress: 0.35,
                funding_source_label: "政府".into(),
                owner_on_completion_label: "国有".into(),
                paid_funds_rm: 50_000_000.0,
                budget_needed_rm: 360_000_000.0,
                material_fulfillment: 0.85,
                fund_ratio: 1.0,
            }],
            buildable_catalog: vec![BuildableBuildingEntry {
                building_def_id: "steel_mill".into(),
                building_name: "钢铁厂".into(),
                group_name: "城市工业".into(),
                locked_reason: None,
                state_limit_reason: Some("仅可在沿海州建造".into()),
            }],
            available_cp: 50.0,
            total_cp: 100.0,
            gdp_gbp: 1_000_000_000.0,
            gdp_growth_yoy: 2.5,
            construction_spend_rm: 500_000.0,
            unemployment_rate: 0.04,
            military_orders_rm: 750_000.0,
            mefo_risk: 0.12,
            auto_build_enabled: false,
            auto_build_explanations: vec![AutoBuildExplanationEntry {
                building_name: "钢铁厂".into(),
                state_name: "莱茵兰".into(),
                score: 180.0,
                reasons: vec!["缓解 steel 短缺".into()],
            }],
            investment_pool: InvestmentPoolV6Data {
                total_rm: 750_000_000.0,
                private_rm: 500_000_000.0,
                cartel_rm: 250_000_000.0,
                ..Default::default()
            },
        };
        assert_eq!(data.entries.len(), 1);
        assert_eq!(data.queue.len(), 1);
        assert_eq!(data.buildable_catalog.len(), 1);
        assert!((data.available_cp - 50.0).abs() < f32::EPSILON);
    }
}
