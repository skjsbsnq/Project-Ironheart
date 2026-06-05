//! V6 建筑与生产面板 UI：可建目录 + 真实建造队列 + 现有建筑概览。

use egui::{Color32, RichText};

use crate::{
    components,
    i18n::tr,
    vanilla_iron::{VanillaIron, WorkbenchShell},
    ActiveDetailPanel, BuildingDetailTarget, PanelCommand, StateDetailTarget,
};

const GOLD: Color32 = components::GOLD;
const GOLD_BRIGHT: Color32 = components::GOLD_BRIGHT;
const MUTED: Color32 = components::MUTED;
const PANEL_CARD: Color32 = Color32::from_rgb(0x24, 0x1a, 0x12);
const PANEL_CARD_SOFT: Color32 = Color32::from_rgb(0x31, 0x24, 0x18);
const STROKE_DARK: Color32 = Color32::from_rgb(0x5a, 0x44, 0x2c);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc0, 0x60);

#[derive(Debug, Clone)]
pub struct BuildingStateV6Entry {
    pub building_idx: usize,
    pub state_id: u16,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionV9Sector {
    Primary,
    Secondary,
    Tertiary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionV9FacilityClass {
    Standard,
    Infrastructure,
    Military,
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
    pub sector: ConstructionV9Sector,
    pub facility_class: ConstructionV9FacilityClass,
    pub total_level: u32,
    pub employment_rate: f32,
    pub profit_rm_weekly: f64,
    pub outputs: Vec<BuildingGoodFlowEntry>,
    pub inputs: Vec<BuildingGoodFlowEntry>,
    pub output_summary: String,
    pub input_summary: String,
    pub warnings: Vec<String>,
    pub pm_summary: String,
    pub states: Vec<BuildingStateV6Entry>,
}

#[derive(Debug, Clone)]
pub struct BuildingGoodFlowEntry {
    pub good_id: String,
    pub good_name: String,
    pub amount: f32,
}

#[derive(Debug, Clone)]
pub struct ConstructionQueueV6Entry {
    pub building_key: String,
    pub building_name: String,
    pub state_id: u16,
    pub state_name: String,
    pub current_level: u8,
    pub target_level: u8,
    pub recipe_cp_cost: f32,
    pub recipe_funds_rm: f64,
    pub recipe_materials_summary: String,
    pub recipe_labor: u32,
    pub recipe_engineering: u32,
    pub recipe_region_summary: String,
    pub progress: f32,
    pub funding_source_label: String,
    pub owner_on_completion_label: String,
    pub paid_funds_rm: f64,
    pub budget_needed_rm: f64,
    pub material_fulfillment: f32,
    pub fund_ratio: f32,
    pub priority: i16,
    pub weight: f32,
    pub paused: bool,
    pub allocated_cp: f32,
    pub effective_cp: f32,
    pub blocked_cp: f32,
    pub bottleneck_label: String,
    pub estimated_days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct BuildableBuildingEntry {
    pub building_def_id: String,
    pub building_name: String,
    pub group_name: String,
    pub sector: ConstructionV9Sector,
    pub facility_class: ConstructionV9FacilityClass,
    pub recipe_cp_cost: f32,
    pub recipe_funds_rm: f64,
    pub recipe_materials_summary: String,
    pub recipe_labor: u32,
    pub recipe_engineering: u32,
    pub recipe_region_summary: String,
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
    pub active_construction_key: Option<String>,
    pub available_cp: f32,
    pub total_cp: f32,
    pub national_admin_cp: f32,
    pub construction_sector_cp: f32,
    pub regional_labor_cp: f32,
    pub engineering_equipment_cp: f32,
    pub finance_cp: f32,
    pub material_cp: f32,
    pub allocated_cp: f32,
    pub idle_cp: f32,
    pub blocked_cp: f32,
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
    ToggleProjectPaused(usize, bool),
    SetProjectPriority {
        idx: usize,
        priority: i16,
    },
    SetProjectWeight {
        idx: usize,
        weight: f32,
    },
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
    Panel(PanelCommand),
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

const CONSTRUCTION_V9_FOOTER: &str = "Q 关闭 | 总览 / 队列 / 目录 / 瓶颈";

pub const CONSTRUCTION_V9_SECONDARY_TABS: [(&str, &str); 10] = [
    ("overview", "总览"),
    ("queue", "队列"),
    ("catalog", "建筑目录"),
    ("primary", "一产建筑"),
    ("secondary", "二产建筑"),
    ("tertiary", "三产建筑"),
    ("infrastructure", "基础设施"),
    ("military", "军事设施"),
    ("bottlenecks", "瓶颈"),
    ("auto_build", "自动建设"),
];

pub fn construction_v9_secondary_tabs() -> &'static [(&'static str, &'static str)] {
    &CONSTRUCTION_V9_SECONDARY_TABS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstructionPanelTab {
    Overview,
    Queue,
    Catalog,
    Primary,
    Secondary,
    Tertiary,
    Infrastructure,
    Military,
    Bottlenecks,
    AutoBuild,
}

impl ConstructionPanelTab {
    fn id(self) -> &'static str {
        CONSTRUCTION_V9_SECONDARY_TABS[self as usize].0
    }

    fn from_id(id: &str) -> Self {
        match id {
            "overview" | "investment" => Self::Overview,
            "queue" => Self::Queue,
            "catalog" => Self::Catalog,
            "primary" => Self::Primary,
            "secondary" => Self::Secondary,
            "tertiary" => Self::Tertiary,
            "infrastructure" => Self::Infrastructure,
            "military" => Self::Military,
            "bottlenecks" | "problems" => Self::Bottlenecks,
            "auto_build" => Self::AutoBuild,
            _ => Self::Catalog,
        }
    }

    fn label(self) -> &'static str {
        CONSTRUCTION_V9_SECONDARY_TABS[self as usize].1
    }
}

fn construction_v9_tab_order() -> [ConstructionPanelTab; 10] {
    [
        ConstructionPanelTab::Overview,
        ConstructionPanelTab::Queue,
        ConstructionPanelTab::Catalog,
        ConstructionPanelTab::Primary,
        ConstructionPanelTab::Secondary,
        ConstructionPanelTab::Tertiary,
        ConstructionPanelTab::Infrastructure,
        ConstructionPanelTab::Military,
        ConstructionPanelTab::Bottlenecks,
        ConstructionPanelTab::AutoBuild,
    ]
}

impl ConstructionV6Panel {
    pub fn show(
        ctx: &egui::Context,
        data: &ConstructionV6PanelData,
    ) -> (bool, Vec<ConstructionV6Command>) {
        workbench_show_construction(ctx, data)
    }
}

fn workbench_show_construction(
    ctx: &egui::Context,
    data: &ConstructionV6PanelData,
) -> (bool, Vec<ConstructionV6Command>) {
    let tab_id = egui::Id::new("buildings_panel_workbench_tab");
    let selected_catalog_id = egui::Id::new("buildings_panel_workbench_selected_catalog");
    let selected_building_id = egui::Id::new("buildings_panel_workbench_selected_building");
    let mut tab = ctx
        .data_mut(|d| d.get_persisted::<ConstructionPanelTab>(tab_id))
        .unwrap_or(ConstructionPanelTab::Catalog);
    let mut selected_catalog = ctx
        .data_mut(|d| d.get_persisted::<Option<String>>(selected_catalog_id))
        .unwrap_or_else(|| {
            data.buildable_catalog
                .first()
                .map(|entry| entry.building_def_id.clone())
        });
    let mut selected_building = ctx
        .data_mut(|d| d.get_persisted::<Option<String>>(selected_building_id))
        .unwrap_or_else(|| {
            data.entries
                .first()
                .map(|entry| entry.building_def_id.clone())
        });

    let cp_ratio = if data.total_cp > 0.0 {
        (data.available_cp / data.total_cp).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let active_building_name = data.active_construction_key.as_deref().and_then(|key| {
        data.buildable_catalog
            .iter()
            .find(|entry| entry.building_def_id == key)
            .map(|entry| entry.building_name.as_str())
    });
    let subtitle = active_building_name
        .map(|name| format!("建造模式：{name}"))
        .unwrap_or_else(|| "建筑目录 / 施工队列 / 建设瓶颈".to_owned());

    let (close, output) =
        WorkbenchShell::new("construction_v6_workbench", tr("buildings_panel_title"))
            .subtitle(&subtitle)
            .footer("Q 关闭  |  点击建筑打开详情")
            .accent(if data.blocked_cp > 0.0 || cp_ratio < 0.15 {
                VanillaIron::WARN
            } else {
                VanillaIron::BRASS_BRIGHT
            })
            .show(ctx, |ui, layout| {
                let mut cmds = Vec::new();
                let nav = layout.nav.shrink2(egui::Vec2::new(8.0, 7.0));
                ui.allocate_ui_at_rect(nav, |ui| {
                    workbench_construction_nav(ui, data, &mut tab, &mut cmds);
                });

                let main = layout.main.shrink2(egui::Vec2::new(8.0, 7.0));
                ui.allocate_ui_at_rect(main, |ui| {
                    workbench_construction_main(
                        ui,
                        data,
                        tab,
                        &mut selected_catalog,
                        &mut selected_building,
                        &mut cmds,
                    );
                });

                let side = layout.side.shrink2(egui::Vec2::new(8.0, 7.0));
                ui.allocate_ui_at_rect(side, |ui| {
                    workbench_construction_side(
                        ui,
                        data,
                        cp_ratio,
                        &selected_catalog,
                        &selected_building,
                        &mut cmds,
                    );
                });
                cmds
            });

    ctx.data_mut(|d| {
        d.insert_persisted(tab_id, tab);
        d.insert_persisted(selected_catalog_id, selected_catalog);
        d.insert_persisted(selected_building_id, selected_building);
    });

    (close, output.unwrap_or_default())
}

fn workbench_construction_nav(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    tab: &mut ConstructionPanelTab,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    VanillaIron::section_heading(ui, "建设状态");
    VanillaIron::info_row(
        ui,
        "建造力",
        format!("{:.0}/{:.0}", data.available_cp, data.total_cp),
    );
    VanillaIron::info_row(ui, "队列", format!("{} 项", data.queue.len()));
    VanillaIron::info_row(ui, "受阻 CP", format!("{:.0}", data.blocked_cp));
    ui.add_space(8.0);

    for next in [
        ConstructionPanelTab::Catalog,
        ConstructionPanelTab::Queue,
        ConstructionPanelTab::Bottlenecks,
        ConstructionPanelTab::Primary,
        ConstructionPanelTab::Secondary,
        ConstructionPanelTab::Tertiary,
        ConstructionPanelTab::Infrastructure,
        ConstructionPanelTab::Military,
        ConstructionPanelTab::AutoBuild,
    ] {
        let label = match next {
            ConstructionPanelTab::Queue => format!("{} ({})", next.label(), data.queue.len()),
            ConstructionPanelTab::Bottlenecks => {
                format!("{} ({})", next.label(), count_bottlenecks(data))
            }
            ConstructionPanelTab::Catalog => {
                format!("{} ({})", next.label(), data.buildable_catalog.len())
            }
            ConstructionPanelTab::Primary
            | ConstructionPanelTab::Secondary
            | ConstructionPanelTab::Tertiary
            | ConstructionPanelTab::Infrastructure
            | ConstructionPanelTab::Military => {
                format!("{} ({})", next.label(), count_entries_for_tab(data, next))
            }
            ConstructionPanelTab::Overview | ConstructionPanelTab::AutoBuild => {
                next.label().to_owned()
            }
        };
        if ui
            .selectable_label(*tab == next, RichText::new(label).color(VanillaIron::TEXT))
            .clicked()
        {
            *tab = next;
        }
    }

    ui.add_space(10.0);
    let mut enabled = data.auto_build_enabled;
    if ui.checkbox(&mut enabled, "自动建设").changed() {
        cmds.push(ConstructionV6Command::ToggleAutoBuild(enabled));
    }
    ui.label(
        RichText::new("地图建造模式会高亮可建设州。")
            .small()
            .color(VanillaIron::MUTED),
    );
}

fn workbench_construction_main(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    tab: ConstructionPanelTab,
    selected_catalog: &mut Option<String>,
    selected_building: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    match tab {
        ConstructionPanelTab::Queue => {
            VanillaIron::section_heading(ui, "施工队列");
            workbench_queue_rows(ui, data, data.queue.iter().enumerate(), cmds);
        }
        ConstructionPanelTab::Bottlenecks => {
            VanillaIron::section_heading(ui, "瓶颈与问题");
            let blocked = data.queue.iter().enumerate().filter(|(_, entry)| {
                entry.blocked_cp > 0.0
                    || entry.paused
                    || !matches!(entry.bottleneck_label.as_str(), "none" | "idle")
            });
            workbench_queue_rows(ui, data, blocked, cmds);
            ui.add_space(8.0);
            for entry in data
                .entries
                .iter()
                .filter(|entry| is_problem_entry(entry))
                .take(8)
            {
                workbench_existing_row(ui, entry, selected_building, cmds);
            }
        }
        ConstructionPanelTab::AutoBuild | ConstructionPanelTab::Overview => {
            VanillaIron::section_heading(ui, "建设总览");
            VanillaIron::info_row(ui, "投资池", format_rm_stock(data.investment_pool.total_rm));
            VanillaIron::info_row(ui, "建设支出", format_rm(data.construction_spend_rm));
            VanillaIron::info_row(
                ui,
                "失业率",
                format!("{:.1}%", data.unemployment_rate * 100.0),
            );
            ui.add_space(8.0);
            render_auto_build_explanations(ui, data);
            ui.add_space(8.0);
            workbench_queue_rows(ui, data, data.queue.iter().enumerate().take(6), cmds);
        }
        ConstructionPanelTab::Catalog => {
            VanillaIron::section_heading(ui, "建筑目录");
            workbench_catalog_rows(
                ui,
                data,
                ConstructionPanelTab::Catalog,
                selected_catalog,
                cmds,
            );
        }
        ConstructionPanelTab::Primary
        | ConstructionPanelTab::Secondary
        | ConstructionPanelTab::Tertiary
        | ConstructionPanelTab::Infrastructure
        | ConstructionPanelTab::Military => {
            VanillaIron::section_heading(ui, tab.label());
            workbench_catalog_rows(ui, data, tab, selected_catalog, cmds);
            ui.add_space(8.0);
            for entry in filtered_entries_for_tab(data, tab) {
                workbench_existing_row(ui, &entry, selected_building, cmds);
            }
        }
    }
}

fn workbench_catalog_rows(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    tab: ConstructionPanelTab,
    selected_catalog: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    let visible: Vec<&BuildableBuildingEntry> = data
        .buildable_catalog
        .iter()
        .filter(|entry| catalog_entry_matches_tab(entry, tab))
        .collect();
    if visible.is_empty() {
        ui.label(
            RichText::new("当前分类没有可建建筑。")
                .small()
                .color(VanillaIron::MUTED),
        );
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for entry in visible {
                let selected = selected_catalog.as_deref() == Some(entry.building_def_id.as_str());
                egui::Frame::new()
                    .fill(if selected {
                        VanillaIron::CARD_SOFT
                    } else {
                        VanillaIron::CARD_DEEP
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if selected {
                            VanillaIron::BRASS_BRIGHT
                        } else {
                            VanillaIron::EDGE_DARK
                        },
                    ))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let label = format!(
                                "{}  CP {:.0}  {}",
                                entry.building_name, entry.recipe_cp_cost, entry.group_name
                            );
                            if ui
                                .selectable_label(
                                    selected,
                                    RichText::new(label).color(VanillaIron::TEXT),
                                )
                                .clicked()
                            {
                                *selected_catalog = Some(entry.building_def_id.clone());
                                cmds.push(ConstructionV6Command::Panel(building_detail_command(
                                    &entry.building_def_id,
                                    None,
                                )));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(
                                            entry.locked_reason.is_none(),
                                            egui::Button::new("建造"),
                                        )
                                        .clicked()
                                    {
                                        *selected_catalog = Some(entry.building_def_id.clone());
                                        cmds.push(ConstructionV6Command::StartConstructionMode {
                                            building_key: entry.building_def_id.clone(),
                                        });
                                    }
                                },
                            );
                        });
                        if let Some(reason) = &entry.locked_reason {
                            ui.label(
                                RichText::new(format!("无法建造：{reason}"))
                                    .small()
                                    .color(VanillaIron::BAD),
                            );
                        } else if let Some(reason) = &entry.state_limit_reason {
                            ui.label(
                                RichText::new(format!("州限制：{reason}"))
                                    .small()
                                    .color(VanillaIron::WARN),
                            );
                        } else {
                            ui.label(
                                RichText::new(format!("材料：{}", entry.recipe_materials_summary))
                                    .small()
                                    .color(VanillaIron::MUTED),
                            );
                        }
                    });
                ui.add_space(4.0);
            }
        });
}

fn workbench_queue_rows<'a>(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    entries: impl Iterator<Item = (usize, &'a ConstructionQueueV6Entry)>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    let mut shown = 0usize;
    for (idx, entry) in entries {
        shown += 1;
        egui::Frame::new()
            .fill(VanillaIron::CARD_DEEP)
            .stroke(egui::Stroke::new(1.0, VanillaIron::EDGE_DARK))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}：{} Lv {} -> {}",
                            entry.state_name,
                            entry.building_name,
                            entry.current_level,
                            entry.target_level,
                        ))
                        .strong()
                        .color(VanillaIron::TEXT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("州详情").clicked() {
                            cmds.push(ConstructionV6Command::Panel(PanelCommand::OpenDetail(
                                ActiveDetailPanel::State(StateDetailTarget {
                                    state_id: entry.state_id,
                                }),
                            )));
                        }
                        if ui.small_button("建筑详情").clicked() {
                            cmds.push(ConstructionV6Command::Panel(building_detail_command(
                                &entry.building_key,
                                Some(entry.state_id),
                            )));
                        }
                    });
                });
                ui.add(
                    egui::ProgressBar::new(entry.progress.clamp(0.0, 1.0))
                        .text(format!("{:.0}%", entry.progress.clamp(0.0, 1.0) * 100.0)),
                );
                ui.label(
                    RichText::new(format!(
                        "资金 {}  材料 {:.0}%  CP {:.1}/{:.1}  瓶颈 {}",
                        entry.funding_source_label,
                        entry.material_fulfillment * 100.0,
                        entry.effective_cp,
                        entry.allocated_cp,
                        entry.bottleneck_label,
                    ))
                    .small()
                    .color(if entry.blocked_cp > 0.0 || entry.paused {
                        VanillaIron::WARN
                    } else {
                        VanillaIron::MUTED
                    }),
                );
                ui.horizontal_wrapped(|ui| {
                    let mut paused = entry.paused;
                    if ui.checkbox(&mut paused, "暂停").changed() {
                        cmds.push(ConstructionV6Command::ToggleProjectPaused(idx, paused));
                    }
                    if ui.add_enabled(idx > 0, egui::Button::new("上移")).clicked() {
                        cmds.push(ConstructionV6Command::MoveUp(idx));
                    }
                    if ui
                        .add_enabled(idx + 1 < data.queue.len(), egui::Button::new("下移"))
                        .clicked()
                    {
                        cmds.push(ConstructionV6Command::MoveDown(idx));
                    }
                    if ui.button("取消").clicked() {
                        cmds.push(ConstructionV6Command::Remove(idx));
                    }
                });
            });
        ui.add_space(5.0);
    }
    if shown == 0 {
        ui.label(
            RichText::new("当前没有施工队列项。")
                .small()
                .color(VanillaIron::MUTED),
        );
    }
}

fn workbench_existing_row(
    ui: &mut egui::Ui,
    entry: &BuildingTypeV6Entry,
    selected_building: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    let selected = selected_building.as_deref() == Some(entry.building_def_id.as_str());
    let response = egui::Frame::new()
        .fill(if selected {
            VanillaIron::CARD_SOFT
        } else {
            VanillaIron::CARD_DEEP
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                VanillaIron::BRASS_BRIGHT
            } else {
                VanillaIron::EDGE_DARK
            },
        ))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} Lv {}  就业 {:.0}%  {}",
                        entry.building_name,
                        entry.total_level,
                        entry.employment_rate * 100.0,
                        signed_rm_stock(entry.profit_rm_weekly),
                    ))
                    .color(VanillaIron::TEXT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("详情").clicked() {
                        cmds.push(ConstructionV6Command::Panel(building_detail_command(
                            &entry.building_def_id,
                            None,
                        )));
                    }
                });
            });
            if !entry.warnings.is_empty() {
                ui.label(
                    RichText::new(entry.warnings.join(" | "))
                        .small()
                        .color(VanillaIron::WARN),
                );
            } else if !entry.output_summary.is_empty() {
                ui.label(
                    RichText::new(format!("产出：{}", entry.output_summary))
                        .small()
                        .color(VanillaIron::MUTED),
                );
            }
        })
        .response;
    if response.clicked() {
        *selected_building = Some(entry.building_def_id.clone());
        cmds.push(ConstructionV6Command::Panel(building_detail_command(
            &entry.building_def_id,
            None,
        )));
    }
    ui.add_space(4.0);
}

fn workbench_construction_side(
    ui: &mut egui::Ui,
    data: &ConstructionV6PanelData,
    cp_ratio: f32,
    selected_catalog: &Option<String>,
    selected_building: &Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    VanillaIron::section_heading(ui, "当前选择摘要");
    VanillaIron::info_row(ui, "可用 CP", format!("{:.0}%", cp_ratio * 100.0));
    VanillaIron::info_row(ui, "闲置 CP", format!("{:.0}", data.idle_cp));
    VanillaIron::info_row(ui, "投资池", format_rm_stock(data.investment_pool.total_rm));
    ui.add_space(8.0);

    if let Some(entry) = selected_building.as_deref().and_then(|id| {
        data.entries
            .iter()
            .find(|entry| entry.building_def_id == id)
    }) {
        VanillaIron::section_heading(ui, &entry.building_name);
        VanillaIron::info_row(ui, "等级", entry.total_level.to_string());
        VanillaIron::info_row(ui, "就业", format!("{:.0}%", entry.employment_rate * 100.0));
        VanillaIron::info_row(ui, "每周收支", signed_rm_stock(entry.profit_rm_weekly));
        if !entry.output_summary.is_empty() {
            ui.label(
                RichText::new(format!("产出：{}", entry.output_summary))
                    .small()
                    .color(VanillaIron::MUTED),
            );
        }
        if VanillaIron::compact_button(ui, "打开建筑详情").clicked() {
            cmds.push(ConstructionV6Command::Panel(building_detail_command(
                &entry.building_def_id,
                None,
            )));
        }
        return;
    }

    if let Some(entry) = selected_catalog.as_deref().and_then(|id| {
        data.buildable_catalog
            .iter()
            .find(|entry| entry.building_def_id == id)
    }) {
        VanillaIron::section_heading(ui, &entry.building_name);
        VanillaIron::info_row(ui, "分组", entry.group_name.clone());
        VanillaIron::info_row(ui, "CP 成本", format!("{:.0}", entry.recipe_cp_cost));
        VanillaIron::info_row(ui, "资金", format_rm_stock(entry.recipe_funds_rm));
        if !entry.recipe_materials_summary.is_empty() {
            ui.label(
                RichText::new(format!("材料：{}", entry.recipe_materials_summary))
                    .small()
                    .color(VanillaIron::MUTED),
            );
        }
        if VanillaIron::compact_button(ui, "打开建筑详情").clicked() {
            cmds.push(ConstructionV6Command::Panel(building_detail_command(
                &entry.building_def_id,
                None,
            )));
        }
        if ui
            .add_enabled(
                entry.locked_reason.is_none(),
                egui::Button::new("进入地图建造"),
            )
            .clicked()
        {
            cmds.push(ConstructionV6Command::StartConstructionMode {
                building_key: entry.building_def_id.clone(),
            });
        }
    } else {
        ui.label(
            RichText::new("从目录、现有建筑或队列选择对象。")
                .small()
                .color(VanillaIron::MUTED),
        );
    }
}

fn building_detail_command(building_key: &str, state_id: Option<u16>) -> PanelCommand {
    PanelCommand::OpenDetail(ActiveDetailPanel::Building(BuildingDetailTarget {
        building_key: building_key.to_owned(),
        state_id,
    }))
}

fn v9_show_construction(
    ctx: &egui::Context,
    data: &ConstructionV6PanelData,
) -> (bool, Vec<ConstructionV6Command>) {
    use crate::v9::composites::panel_shell::{draw_summary_tiles, PanelClass, PanelShell};
    use crate::v9::tokens::palette;

    let tab_id = egui::Id::new("buildings_panel_v9_tab");
    let selected_catalog_id = egui::Id::new("buildings_panel_v9_selected_catalog");
    let selected_building_id = egui::Id::new("buildings_panel_v9_selected_building");
    let mut tab = ctx
        .data_mut(|d| d.get_persisted::<ConstructionPanelTab>(tab_id))
        .unwrap_or(ConstructionPanelTab::Catalog);
    let mut selected_catalog = ctx
        .data_mut(|d| d.get_persisted::<Option<String>>(selected_catalog_id))
        .unwrap_or_else(|| {
            data.buildable_catalog
                .first()
                .map(|entry| entry.building_def_id.clone())
        });
    let mut selected_building = ctx
        .data_mut(|d| d.get_persisted::<Option<String>>(selected_building_id))
        .unwrap_or_else(|| {
            data.entries
                .first()
                .map(|entry| entry.building_def_id.clone())
        });

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
    let active_building_name = data.active_construction_key.as_deref().and_then(|key| {
        data.buildable_catalog
            .iter()
            .find(|entry| entry.building_def_id == key)
            .map(|entry| entry.building_name.as_str())
    });
    let subtitle = active_building_name
        .map(|name| format!("建造模式：{}", name))
        .unwrap_or_else(|| "V6 经济".to_owned());
    let (close, output) = PanelShell::new("construction_v6_panel_v9", tr("buildings_panel_title"))
        .subtitle(&subtitle)
        .class(PanelClass::Economy)
        .accent(accent)
        .footer(CONSTRUCTION_V9_FOOTER)
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
                    (tr("gdp"), format_gbp(data.gdp_gbp), palette::GOLD),
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
            v9_construction_tabs(ui, layout.tabs, &mut tab, data, accent);
            let mut cmds = Vec::new();
            v9_construction_body(
                ui,
                layout.body,
                data,
                cp_ratio,
                tab,
                &mut selected_catalog,
                &mut selected_building,
                &mut cmds,
            );
            cmds
        });
    ctx.data_mut(|d| {
        d.insert_persisted(tab_id, tab);
        d.insert_persisted(selected_catalog_id, selected_catalog);
        d.insert_persisted(selected_building_id, selected_building);
    });
    (close, output.unwrap_or_default())
}

fn v9_construction_tabs(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    tab: &mut ConstructionPanelTab,
    data: &ConstructionV6PanelData,
    _accent: egui::Color32,
) {
    use crate::v9::primitives::{TabBar, TabItem};

    let mut active = tab.id();
    let items: Vec<TabItem> = construction_v9_tab_order()
        .into_iter()
        .map(|tab| {
            let item = TabItem::new(tab.id(), tab.label());
            match tab {
                ConstructionPanelTab::Queue => item.with_badge(data.queue.len() as u32),
                ConstructionPanelTab::Catalog => {
                    item.with_badge(data.buildable_catalog.len() as u32)
                }
                ConstructionPanelTab::Primary
                | ConstructionPanelTab::Secondary
                | ConstructionPanelTab::Tertiary
                | ConstructionPanelTab::Infrastructure
                | ConstructionPanelTab::Military => {
                    item.with_badge(count_entries_for_tab(data, tab) as u32)
                }
                ConstructionPanelTab::Bottlenecks => {
                    item.with_badge(count_bottlenecks(data) as u32)
                }
                ConstructionPanelTab::Overview | ConstructionPanelTab::AutoBuild => item,
            }
        })
        .collect();
    if let Some(clicked) = TabBar::show_at(ui, rect, &items, &mut active) {
        *tab = ConstructionPanelTab::from_id(clicked);
    }
}

#[allow(clippy::too_many_arguments)]
fn v9_construction_body(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    cp_ratio: f32,
    tab: ConstructionPanelTab,
    selected_catalog: &mut Option<String>,
    selected_building: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    use crate::v9::{
        layout::{GridLayout, Track},
        tokens::spacing,
    };

    ui.allocate_ui_at_rect(rect, |ui| {
        let grid = GridLayout::new(vec![Track::Fr(1.0)], vec![Track::Fr(0.58), Track::Fr(0.42)])
            .with_gutter(spacing::S5, 0.0);
        let cells = grid.measure(rect);
        let main = GridLayout::cell(&cells, 0, 0);
        let detail = GridLayout::cell(&cells, 0, 1);
        match tab {
            ConstructionPanelTab::Catalog => {
                v9_catalog_list_panel(ui, main, data, selected_catalog, cmds);
                v9_catalog_detail_panel(ui, detail, data, selected_catalog, cmds);
            }
            ConstructionPanelTab::Queue => {
                v9_queue_panel(ui, main, data, cmds);
                v9_investment_detail_panel(ui, detail, data, cp_ratio, cmds);
            }
            ConstructionPanelTab::Primary
            | ConstructionPanelTab::Secondary
            | ConstructionPanelTab::Tertiary
            | ConstructionPanelTab::Infrastructure
            | ConstructionPanelTab::Military => {
                v9_catalog_list_panel_for_tab(ui, main, data, tab, selected_catalog, cmds);
                v9_existing_list_panel_for_tab(ui, detail, data, selected_building, tab);
            }
            ConstructionPanelTab::Bottlenecks => {
                v9_bottleneck_queue_panel(ui, main, data, cmds);
                v9_problem_overview_panel(ui, detail, data, selected_building);
            }
            ConstructionPanelTab::Overview => {
                v9_investment_overview_panel(ui, main, data, cp_ratio, cmds);
                v9_problem_overview_panel(ui, detail, data, selected_building);
            }
            ConstructionPanelTab::AutoBuild => {
                v9_investment_detail_panel(ui, main, data, cp_ratio, cmds);
                v9_queue_panel(ui, detail, data, cmds);
            }
        }
    });
}

fn v9_card_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui, egui::Rect),
) {
    use crate::v9::{
        primitives::Card,
        tokens::{palette, spacing, TextRole},
    };
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
    )
    .shrink2(egui::vec2(spacing::S1, spacing::S1));
    ui.allocate_ui_at_rect(content, |ui| {
        ui.set_clip_rect(content);
        ui.set_min_size(content.size());
        ui.set_width(content.width());
        add_contents(ui, content);
    });
}

fn v9_scroll(
    ui: &mut egui::Ui,
    content_rect: egui::Rect,
    id_salt: impl std::hash::Hash,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .max_height(content_rect.height())
        .max_width(content_rect.width())
        .auto_shrink([false, false])
        .scroll_bar_visibility(
            egui::containers::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
        )
        .show(ui, |ui| {
            ui.set_min_width(content_rect.width());
            ui.set_max_width(content_rect.width());
            add_contents(ui);
        });
}

fn v9_catalog_list_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    selected_catalog: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    v9_catalog_list_panel_for_tab(
        ui,
        rect,
        data,
        ConstructionPanelTab::Catalog,
        selected_catalog,
        cmds,
    );
}

fn v9_catalog_list_panel_for_tab(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    tab: ConstructionPanelTab,
    selected_catalog: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    let visible: Vec<&BuildableBuildingEntry> = data
        .buildable_catalog
        .iter()
        .filter(|entry| catalog_entry_matches_tab(entry, tab))
        .collect();
    let title = if tab == ConstructionPanelTab::Catalog {
        tr("buildable_buildings").to_owned()
    } else {
        format!("{}目录", tab.label())
    };
    v9_card_panel(ui, rect, &title, |ui, content| {
        if visible.is_empty() {
            components::empty_state(ui, tr("no_buildable_buildings"), "");
            return;
        }
        v9_scroll(
            ui,
            content,
            ("v9_construction_catalog_list", tab.id()),
            |ui| {
                for entry in visible {
                    v9_buildable_row(
                        ui,
                        entry,
                        data.active_construction_key.as_deref(),
                        selected_catalog,
                        cmds,
                    );
                }
            },
        );
    });
}

fn v9_buildable_row(
    ui: &mut egui::Ui,
    entry: &BuildableBuildingEntry,
    active_key: Option<&str>,
    selected_catalog: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    use crate::v9::{
        paint,
        primitives::{Button, ButtonSize, ButtonVariant},
        tokens::{palette, radius, spacing, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let selected = selected_catalog.as_deref() == Some(entry.building_def_id.as_str());
    let active = active_key == Some(entry.building_def_id.as_str());
    let can_start = entry.locked_reason.is_none();
    let row_h = 68.0;
    let (raw_rect, row_resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
    let row_rect = raw_rect.shrink2(Vec2::new(0.0, 1.0));
    let fill = if active || selected {
        palette::OIL_BLACK
    } else if row_resp.hovered() {
        palette::IRON_DARK
    } else {
        palette::SOOT_BLACK
    };
    let stroke = if active {
        palette::GOLD_HOT
    } else if selected || row_resp.hovered() {
        palette::BRASS_DARK
    } else {
        palette::EDGE_DARK
    };

    if row_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if row_resp.clicked() {
        *selected_catalog = Some(entry.building_def_id.clone());
    }

    let painter = ui.painter().clone();
    paint::paint_bevel(&painter, row_rect, fill, stroke, radius::R1);
    paint::paint_plate_grain(&painter, row_rect.shrink(4.0), 3.0, 2);
    paint::paint_speckle(&painter, row_rect.shrink(4.0), 6, 1);
    let accent = if active {
        palette::GOLD_HOT
    } else if entry.locked_reason.is_some() {
        palette::BAD
    } else if entry.state_limit_reason.is_some() {
        palette::WARN
    } else {
        palette::GOOD
    };
    painter.rect_filled(
        Rect::from_min_max(
            row_rect.min + Vec2::new(3.0, 5.0),
            Pos2::new(row_rect.left() + 7.0, row_rect.bottom() - 5.0),
        ),
        egui::epaint::CornerRadius::ZERO,
        accent,
    );

    let button_rect = Rect::from_min_size(
        Pos2::new(row_rect.right() - 92.0, row_rect.center().y - 13.0),
        Vec2::new(84.0, 26.0),
    );
    let text_left = row_rect.left() + spacing::S5;
    let text_right = (button_rect.left() - spacing::S4).max(text_left + 12.0);
    let title = if active {
        format!("{}  -  建造目标", entry.building_name)
    } else {
        entry.building_name.clone()
    };
    let title_painter = painter.with_clip_rect(Rect::from_min_max(
        Pos2::new(text_left, row_rect.top() + 5.0),
        Pos2::new(text_right, row_rect.top() + 28.0),
    ));
    title_painter.text(
        Pos2::new(text_left, row_rect.top() + 7.0),
        Align2::LEFT_TOP,
        title,
        TextRole::Subheading.font_id(),
        if active || selected {
            palette::GOLD_HOT
        } else {
            palette::PARCHMENT
        },
    );

    let group_painter = painter.with_clip_rect(Rect::from_min_max(
        Pos2::new(text_left, row_rect.top() + 27.0),
        Pos2::new(text_right, row_rect.top() + 43.0),
    ));
    group_painter.text(
        Pos2::new(text_left, row_rect.top() + 28.0),
        Align2::LEFT_TOP,
        &entry.group_name,
        TextRole::Caption.font_id(),
        palette::PARCHMENT_DIM,
    );

    let note = entry
        .locked_reason
        .as_deref()
        .or(entry.state_limit_reason.as_deref())
        .unwrap_or("点击建造后，在地图上选择高亮州加入建造队列。");
    let note_color = if entry.locked_reason.is_some() {
        palette::BAD
    } else if entry.state_limit_reason.is_some() {
        palette::WARN
    } else {
        palette::MUTED
    };
    let note_painter = painter.with_clip_rect(Rect::from_min_max(
        Pos2::new(text_left, row_rect.top() + 45.0),
        Pos2::new(text_right, row_rect.bottom() - 4.0),
    ));
    note_painter.text(
        Pos2::new(text_left, row_rect.top() + 47.0),
        Align2::LEFT_TOP,
        note,
        TextRole::Small.font_id(),
        note_color,
    );

    let button_resp = Button::new(if active {
        "已选择"
    } else if can_start {
        "建造"
    } else {
        "不可建"
    })
    .size(ButtonSize::Sm)
    .variant(if active {
        ButtonVariant::Secondary
    } else {
        ButtonVariant::Primary
    })
    .enabled(can_start)
    .show_at(ui, button_rect);
    if can_start && button_resp.clicked() {
        *selected_catalog = Some(entry.building_def_id.clone());
        cmds.push(ConstructionV6Command::StartConstructionMode {
            building_key: entry.building_def_id.clone(),
        });
    }
    ui.add_space(spacing::S2);
}

fn v9_catalog_detail_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    selected_catalog: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    v9_card_panel(ui, rect, "建筑详情", |ui, content| {
        let selected = selected_catalog
            .as_deref()
            .and_then(|id| {
                data.buildable_catalog
                    .iter()
                    .find(|entry| entry.building_def_id == id)
            })
            .or_else(|| data.buildable_catalog.first());
        if let Some(entry) = selected {
            v9_scroll(ui, content, "v9_construction_catalog_detail", |ui| {
                render_catalog_detail(ui, entry, data, cmds);
                if data.active_construction_key.as_deref() == Some(entry.building_def_id.as_str()) {
                    ui.separator();
                    ui.label(
                        RichText::new("正在建造模式：点击地图上高亮州加入队列；右键或 ESC 退出。")
                            .small()
                            .color(GOLD_BRIGHT),
                    );
                }
            });
        } else {
            components::empty_state(ui, "未选择建筑", "从左侧建造目录选择一个建筑。");
        }
    });
}

fn v9_queue_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    v9_card_panel(ui, rect, tr("construction_queue"), |ui, content| {
        v9_scroll(ui, content, "v9_construction_queue", |ui| {
            render_queue(ui, &data.queue, cmds);
        });
    });
}

#[allow(dead_code)]
fn v9_existing_list_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    selected_building: &mut Option<String>,
    problems_only: bool,
) {
    let title = if problems_only {
        "问题建筑"
    } else {
        tr("existing_buildings")
    };
    v9_card_panel(ui, rect, title, |ui, content| {
        let mut visible: Vec<&BuildingTypeV6Entry> = data
            .entries
            .iter()
            .filter(|entry| !problems_only || is_problem_entry(entry))
            .collect();
        visible.sort_by(|a, b| {
            problem_score(b)
                .cmp(&problem_score(a))
                .then(a.building_name.cmp(&b.building_name))
        });
        if visible.is_empty() {
            components::empty_state(ui, tr("no_existing_buildings"), "");
            return;
        }
        let scroll_id = if problems_only {
            "v9_construction_problem_buildings"
        } else {
            "v9_construction_existing_buildings"
        };
        v9_scroll(ui, content, scroll_id, |ui| {
            for entry in visible {
                v9_existing_building_row(ui, entry, selected_building);
            }
        });
    });
}

fn v9_existing_list_panel_for_tab(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    selected_building: &mut Option<String>,
    tab: ConstructionPanelTab,
) {
    let visible = filtered_entries_for_tab(data, tab);
    v9_card_panel(ui, rect, tab.label(), |ui, content| {
        if visible.is_empty() {
            components::empty_state(ui, "暂无建筑", "当前分类下没有现有建筑。");
            return;
        }
        v9_scroll(
            ui,
            content,
            ("v9_construction_existing_by_tab", tab.id()),
            |ui| {
                for entry in visible {
                    v9_existing_building_row(ui, &entry, selected_building);
                }
            },
        );
    });
}

fn v9_bottleneck_queue_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    use crate::v9::tokens::palette;

    v9_card_panel(ui, rect, "建造瓶颈", |ui, content| {
        v9_scroll(ui, content, "v9_construction_bottleneck_queue", |ui| {
            ui.label(
                RichText::new(format!(
                    "已分配 CP {:.0}  闲置 CP {:.0}  受阻 CP {:.0}",
                    data.allocated_cp, data.idle_cp, data.blocked_cp
                ))
                .strong()
                .color(if data.blocked_cp > 0.0 {
                    palette::WARN
                } else {
                    palette::GOOD
                }),
            );
            ui.label(
                RichText::new(cp_source_summary(data))
                    .small()
                    .color(palette::PARCHMENT_DIM),
            );
            ui.add_space(6.0);
            let has_blocked_entries = data.queue.iter().any(|entry| {
                entry.blocked_cp > 0.0
                    || entry.paused
                    || !matches!(entry.bottleneck_label.as_str(), "none" | "idle")
            });
            if !has_blocked_entries {
                components::empty_state(
                    ui,
                    "暂无建造瓶颈",
                    "当前队列没有明显资金、材料、劳工、工程或基础设施阻塞。",
                );
            } else {
                render_queue(ui, &data.queue, cmds);
            }
        });
    });
}

fn count_entries_for_tab(data: &ConstructionV6PanelData, tab: ConstructionPanelTab) -> usize {
    data.entries
        .iter()
        .filter(|entry| entry_matches_tab(entry, tab))
        .count()
}

fn count_bottlenecks(data: &ConstructionV6PanelData) -> usize {
    let queue_bottlenecks = data
        .queue
        .iter()
        .filter(|entry| {
            entry.blocked_cp > 0.0
                || entry.paused
                || !matches!(entry.bottleneck_label.as_str(), "none" | "idle")
        })
        .count();
    queue_bottlenecks
        + data
            .entries
            .iter()
            .filter(|entry| is_problem_entry(entry))
            .count()
}

fn filtered_entries_for_tab(
    data: &ConstructionV6PanelData,
    tab: ConstructionPanelTab,
) -> Vec<BuildingTypeV6Entry> {
    let mut entries: Vec<BuildingTypeV6Entry> = data
        .entries
        .iter()
        .filter(|entry| entry_matches_tab(entry, tab))
        .cloned()
        .collect();
    entries.sort_by(|a, b| {
        problem_score(b)
            .cmp(&problem_score(a))
            .then(a.building_name.cmp(&b.building_name))
    });
    entries
}

fn entry_matches_tab(entry: &BuildingTypeV6Entry, tab: ConstructionPanelTab) -> bool {
    match tab {
        ConstructionPanelTab::Primary => {
            entry.facility_class == ConstructionV9FacilityClass::Standard
                && entry.sector == ConstructionV9Sector::Primary
        }
        ConstructionPanelTab::Secondary => {
            entry.facility_class == ConstructionV9FacilityClass::Standard
                && entry.sector == ConstructionV9Sector::Secondary
        }
        ConstructionPanelTab::Tertiary => {
            entry.facility_class == ConstructionV9FacilityClass::Standard
                && entry.sector == ConstructionV9Sector::Tertiary
        }
        ConstructionPanelTab::Infrastructure => {
            entry.facility_class == ConstructionV9FacilityClass::Infrastructure
        }
        ConstructionPanelTab::Military => {
            entry.facility_class == ConstructionV9FacilityClass::Military
        }
        _ => true,
    }
}

fn catalog_entry_matches_tab(entry: &BuildableBuildingEntry, tab: ConstructionPanelTab) -> bool {
    match tab {
        ConstructionPanelTab::Primary => {
            entry.facility_class == ConstructionV9FacilityClass::Standard
                && entry.sector == ConstructionV9Sector::Primary
        }
        ConstructionPanelTab::Secondary => {
            entry.facility_class == ConstructionV9FacilityClass::Standard
                && entry.sector == ConstructionV9Sector::Secondary
        }
        ConstructionPanelTab::Tertiary => {
            entry.facility_class == ConstructionV9FacilityClass::Standard
                && entry.sector == ConstructionV9Sector::Tertiary
        }
        ConstructionPanelTab::Infrastructure => {
            entry.facility_class == ConstructionV9FacilityClass::Infrastructure
        }
        ConstructionPanelTab::Military => {
            entry.facility_class == ConstructionV9FacilityClass::Military
        }
        _ => true,
    }
}

fn v9_existing_building_row(
    ui: &mut egui::Ui,
    entry: &BuildingTypeV6Entry,
    selected_building: &mut Option<String>,
) {
    use crate::v9::{
        paint,
        tokens::{palette, radius, spacing, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let selected = selected_building.as_deref() == Some(entry.building_def_id.as_str());
    let has_warning = !entry.warnings.is_empty();
    let row_h = if has_warning { 76.0 } else { 64.0 };
    let (raw_rect, row_resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
    let row_rect = raw_rect.shrink2(Vec2::new(0.0, 1.0));
    if row_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if row_resp.clicked() {
        *selected_building = Some(entry.building_def_id.clone());
    }

    let accent = if has_warning {
        palette::WARN
    } else if entry.profit_rm_weekly < 0.0 {
        palette::BAD
    } else {
        palette::GOOD
    };
    let fill = if selected {
        palette::OIL_BLACK
    } else if row_resp.hovered() {
        palette::IRON_DARK
    } else {
        palette::SOOT_BLACK
    };
    let stroke = if selected || row_resp.hovered() {
        palette::BRASS_DARK
    } else {
        palette::EDGE_DARK
    };

    let painter = ui.painter().clone();
    paint::paint_bevel(&painter, row_rect, fill, stroke, radius::R1);
    paint::paint_plate_grain(&painter, row_rect.shrink(4.0), 3.0, 2);
    paint::paint_speckle(&painter, row_rect.shrink(4.0), 6, 1);
    painter.rect_filled(
        Rect::from_min_max(
            row_rect.min + Vec2::new(3.0, 5.0),
            Pos2::new(row_rect.left() + 7.0, row_rect.bottom() - 5.0),
        ),
        egui::epaint::CornerRadius::ZERO,
        accent,
    );

    let text_left = row_rect.left() + spacing::S5;
    let right_w = 150.0_f32.min((row_rect.width() * 0.42).max(104.0));
    let title_right = (row_rect.right() - right_w - spacing::S4).max(text_left + 16.0);
    let title_painter = painter.with_clip_rect(Rect::from_min_max(
        Pos2::new(text_left, row_rect.top() + 5.0),
        Pos2::new(title_right, row_rect.top() + 29.0),
    ));
    title_painter.text(
        Pos2::new(text_left, row_rect.top() + 7.0),
        Align2::LEFT_TOP,
        &entry.building_name,
        TextRole::Subheading.font_id(),
        if selected {
            palette::GOLD_HOT
        } else {
            palette::PARCHMENT
        },
    );

    let right_label = format!(
        "Lv {}  就业 {:.0}%",
        entry.total_level,
        entry.employment_rate.clamp(0.0, 1.0) * 100.0
    );
    let right_painter = painter.with_clip_rect(Rect::from_min_max(
        Pos2::new(row_rect.right() - right_w, row_rect.top() + 5.0),
        Pos2::new(row_rect.right() - spacing::S4, row_rect.top() + 29.0),
    ));
    right_painter.text(
        Pos2::new(row_rect.right() - spacing::S4, row_rect.top() + 8.0),
        Align2::RIGHT_TOP,
        right_label,
        TextRole::Numeric.font_id(),
        accent,
    );

    let detail = if has_warning {
        entry.warnings.join(" | ")
    } else if !entry.output_summary.is_empty() {
        format!("产出：{}", entry.output_summary)
    } else if !entry.input_summary.is_empty() {
        format!("投入：{}", entry.input_summary)
    } else {
        "暂无生产摘要".to_owned()
    };
    let detail_painter = painter.with_clip_rect(Rect::from_min_max(
        Pos2::new(text_left, row_rect.top() + 31.0),
        Pos2::new(row_rect.right() - spacing::S4, row_rect.bottom() - 22.0),
    ));
    detail_painter.text(
        Pos2::new(text_left, row_rect.top() + 32.0),
        Align2::LEFT_TOP,
        detail,
        TextRole::Caption.font_id(),
        if has_warning {
            palette::WARN
        } else {
            palette::PARCHMENT_DIM
        },
    );

    let footer = format!("每周收支 {}", signed_rm_stock(entry.profit_rm_weekly));
    let footer_painter = painter.with_clip_rect(Rect::from_min_max(
        Pos2::new(text_left, row_rect.bottom() - 20.0),
        Pos2::new(row_rect.right() - spacing::S4, row_rect.bottom() - 3.0),
    ));
    footer_painter.text(
        Pos2::new(text_left, row_rect.bottom() - 18.0),
        Align2::LEFT_TOP,
        footer,
        TextRole::Small.font_id(),
        if entry.profit_rm_weekly < 0.0 {
            palette::BAD
        } else {
            palette::MUTED
        },
    );
    ui.add_space(spacing::S2);
}

#[allow(dead_code)]
fn v9_existing_detail_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    selected_building: &mut Option<String>,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    v9_card_panel(ui, rect, "建筑二级面板", |ui, content| {
        let selected = selected_building
            .as_deref()
            .and_then(|id| {
                data.entries
                    .iter()
                    .find(|entry| entry.building_def_id == id)
            })
            .or_else(|| data.entries.first());
        if let Some(entry) = selected {
            v9_scroll(ui, content, "v9_construction_existing_detail", |ui| {
                render_entry(ui, entry, cmds);
            });
        } else {
            components::empty_state(ui, "未选择建筑", "从左侧选择一个现有建筑查看详情。");
        }
    });
}

fn v9_investment_detail_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    cp_ratio: f32,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    v9_card_panel(ui, rect, "投资与自动建造", |ui, content| {
        v9_scroll(ui, content, "v9_construction_investment_detail", |ui| {
            render_command_bar(ui, data, cp_ratio, cmds);
            render_auto_build_explanations(ui, data);
        });
    });
}

fn v9_investment_overview_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    cp_ratio: f32,
    cmds: &mut Vec<ConstructionV6Command>,
) {
    v9_card_panel(ui, rect, "投资池", |ui, content| {
        v9_scroll(ui, content, "v9_construction_investment_overview", |ui| {
            render_summary(ui, data);
            ui.add_space(6.0);
            ui.label(RichText::new(cp_explanation_text()).small().color(MUTED));
            ui.label(RichText::new(cp_source_summary(data)).small().color(MUTED));
            ui.add_space(6.0);
            render_status_banner(ui, data, cp_ratio);
            ui.add_space(6.0);
            render_command_bar(ui, data, cp_ratio, cmds);
        });
    });
}

fn cp_explanation_text() -> &'static str {
    "建造力 CP 表示国家每日可投入建设的能力；已分配 CP 推进队列，闲置 CP 代表未使用产能，受阻 CP 来自资金、材料、劳工、工程或基础设施瓶颈。"
}

fn cp_source_summary(data: &ConstructionV6PanelData) -> String {
    format!(
        "来源：行政 {:.0} / 建设部门 {:.0} / 地区劳力 {:.0} / 工程设备 {:.0}；上限：资金 {:.0} / 材料 {:.0}",
        data.national_admin_cp,
        data.construction_sector_cp,
        data.regional_labor_cp,
        data.engineering_equipment_cp,
        data.finance_cp,
        data.material_cp,
    )
}

fn v9_problem_overview_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    data: &ConstructionV6PanelData,
    selected_building: &mut Option<String>,
) {
    v9_card_panel(ui, rect, "问题与提示", |ui, content| {
        v9_scroll(ui, content, "v9_construction_problem_overview", |ui| {
            let mut shown = 0usize;
            for entry in data.entries.iter().filter(|entry| is_problem_entry(entry)) {
                shown += 1;
                v9_existing_building_row(ui, entry, selected_building);
                render_problem_card(ui, entry);
                ui.add_space(6.0);
            }
            if shown == 0 {
                components::empty_state(
                    ui,
                    "暂无问题建筑",
                    "当前建筑没有明显短缺、亏损或低就业警告。",
                );
            }
        });
    });
}

#[allow(dead_code)]
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
            TableColumn::new("州", 1.0),
            TableColumn::new(tr("progress"), 0.7).right(),
            TableColumn::new("材料", 0.6).right(),
            TableColumn::new("资金", 0.6).right(),
            TableColumn::new("来源", 1.0),
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

#[allow(dead_code)]
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
            TableColumn::new("等级", 0.4).right(),
            TableColumn::new(tr("employment_rate"), 0.7).right(),
            TableColumn::new(tr("building_profit_weekly"), 0.8).right(),
            TableColumn::new(tr("building_outputs"), 1.2),
            TableColumn::new("警告", 0.5).center(),
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

#[allow(dead_code)]
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
        sound,
        tokens::{palette, spacing, TextRole},
    };
    use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};

    let inner = Card::new().as_panel().show_at(ui, rect);
    ui.painter().text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "投资池",
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
        ("总额", format_rm_stock(pool.total_rm), palette::GOLD),
        ("私人", format_rm_stock(pool.private_rm), palette::GOOD),
        ("卡特尔", format_rm_stock(pool.cartel_rm), palette::WARN),
        (
            "银行",
            format_rm_stock(pool.state_development_bank_rm),
            palette::INFO,
        ),
        (
            "外资",
            format_rm_stock(pool.foreign_capital_rm),
            palette::COLD_STEEL,
        ),
        ("已用", format_rm_stock(pool.spent_rm), palette::BAD),
    ];
    for (idx, (label, value, color)) in tiles.iter().enumerate() {
        Tile::new(label, value)
            .accent(*color)
            .show_at(ui, GridLayout::cell(&cells, idx / 2, idx % 2));
    }

    let mut y = tile_rect.bottom() + spacing::S5;
    let toggle_label = if data.auto_build_enabled {
        "关闭自动"
    } else {
        "启用自动"
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
    let queue_controls_reserved = if data.queue.first().is_some() {
        118.0
    } else {
        0.0
    };
    let available_rows_h = (inner.bottom() - y - queue_controls_reserved - spacing::S3).max(0.0);
    let max_buildable_rows = ((available_rows_h / 34.0).floor() as usize).clamp(4, 9);
    let active_key = data.active_construction_key.as_deref();
    for entry in data.buildable_catalog.iter().take(max_buildable_rows) {
        let row = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 30.0));
        let button_rect = Rect::from_min_size(
            Pos2::new(row.right() - 82.0, row.top() + 3.0),
            Vec2::new(78.0, 24.0),
        );
        let label_rect = Rect::from_min_max(
            row.left_top(),
            Pos2::new(button_rect.left() - spacing::S2, row.bottom()),
        );
        let can_start = entry.locked_reason.is_none();
        let active = active_key == Some(entry.building_def_id.as_str());
        let response = ui.interact(
            label_rect,
            ui.id().with(("v9_buildable_row", &entry.building_def_id)),
            Sense::click(),
        );
        sound::hook_response_auto(
            &format!("buildable:{}", entry.building_def_id),
            &response,
            can_start,
        );
        if can_start && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let fill = if active {
            Color32::from_rgba_premultiplied(
                palette::GOLD.r(),
                palette::GOLD.g(),
                palette::GOLD.b(),
                28,
            )
        } else if response.hovered() && can_start {
            Color32::from_black_alpha(96)
        } else {
            Color32::from_black_alpha(54)
        };
        ui.painter()
            .rect_filled(row, egui::epaint::CornerRadius::same(1), fill);
        ui.painter().rect_stroke(
            row,
            egui::epaint::CornerRadius::same(1),
            Stroke::new(
                1.0,
                if active {
                    palette::GOLD
                } else {
                    palette::EDGE_DARK
                },
            ),
            egui::StrokeKind::Inside,
        );
        let title_color = if !can_start {
            palette::MUTED
        } else if active {
            palette::GOLD_HOT
        } else {
            palette::PARCHMENT
        };
        let note = entry
            .locked_reason
            .as_deref()
            .or(entry.state_limit_reason.as_deref())
            .unwrap_or(entry.group_name.as_str());
        let text_clip = ui
            .painter()
            .with_clip_rect(label_rect.shrink2(Vec2::new(6.0, 0.0)));
        text_clip.text(
            Pos2::new(label_rect.left() + spacing::S2, row.center().y - 5.0),
            Align2::LEFT_CENTER,
            &entry.building_name,
            TextRole::Body.font_id(),
            title_color,
        );
        text_clip.text(
            Pos2::new(label_rect.left() + spacing::S2, row.center().y + 9.0),
            Align2::LEFT_CENTER,
            note,
            TextRole::Caption.font_id(),
            if entry.locked_reason.is_some() {
                palette::BAD
            } else {
                palette::MUTED
            },
        );
        let button_label = if active { "已选择" } else { "建造" };
        let button_clicked = Button::new(button_label)
            .size(ButtonSize::Sm)
            .variant(if active {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Primary
            })
            .enabled(can_start)
            .show_at(ui, button_rect)
            .clicked();
        if can_start && (response.clicked() || button_clicked) {
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
            "队列控制",
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
        tab_button(ui, tab, ConstructionPanelTab::Overview, "总览");
        tab_button(
            ui,
            tab,
            ConstructionPanelTab::Queue,
            &format!("建造队列 {}", data.queue.len()),
        );
        tab_button(ui, tab, ConstructionPanelTab::Catalog, "建筑目录");
        tab_button(ui, tab, ConstructionPanelTab::Primary, "一产建筑");
        tab_button(ui, tab, ConstructionPanelTab::Secondary, "二产建筑");
        tab_button(ui, tab, ConstructionPanelTab::Tertiary, "三产建筑");
        tab_button(ui, tab, ConstructionPanelTab::Infrastructure, "基础设施");
        tab_button(ui, tab, ConstructionPanelTab::Military, "军事设施");
        tab_button(
            ui,
            tab,
            ConstructionPanelTab::Bottlenecks,
            &format!("瓶颈 {}", count_bottlenecks(data)),
        );
        tab_button(ui, tab, ConstructionPanelTab::AutoBuild, "自动建设");
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
            ui.label(
                RichText::new(format!(
                    "CP {:.0} | {:.1}M RM | {}",
                    entry.recipe_cp_cost,
                    entry.recipe_funds_rm / 1_000_000.0,
                    entry.recipe_materials_summary
                ))
                .small()
                .color(MUTED),
            );
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
    use crate::v9::{
        primitives::{Button, ButtonSize, ButtonVariant},
        tokens::{palette, TextRole},
    };

    ui.label(
        RichText::new(&entry.building_name)
            .font(TextRole::Heading.font_id())
            .color(palette::GOLD_HOT),
    );
    ui.label(
        RichText::new(format!("分组：{}", entry.group_name))
            .font(TextRole::Caption.font_id())
            .color(palette::PARCHMENT_DIM),
    );
    ui.label(
        RichText::new(format!(
            "配方：CP {:.0} | {:.1}M RM | 劳力 {} | 工程 {}",
            entry.recipe_cp_cost,
            entry.recipe_funds_rm / 1_000_000.0,
            entry.recipe_labor,
            entry.recipe_engineering
        ))
        .font(TextRole::Body.font_id())
        .color(palette::PARCHMENT),
    );
    if !entry.recipe_materials_summary.is_empty() {
        ui.label(
            RichText::new(format!("材料：{}", entry.recipe_materials_summary))
                .font(TextRole::Caption.font_id())
                .color(palette::PARCHMENT_DIM),
        );
    }
    if !entry.recipe_region_summary.is_empty() {
        ui.label(
            RichText::new(format!("地区限制：{}", entry.recipe_region_summary))
                .font(TextRole::Caption.font_id())
                .color(palette::PARCHMENT_DIM),
        );
    }
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
        let build_label = format!("建造 {}", entry.building_name);
        if Button::new(&build_label)
            .size(ButtonSize::Md)
            .variant(ButtonVariant::Primary)
            .show(ui)
            .clicked()
        {
            cmds.push(ConstructionV6Command::StartConstructionMode {
                building_key: entry.building_def_id.clone(),
            });
        }
    }
}

fn render_problem_card(ui: &mut egui::Ui, entry: &BuildingTypeV6Entry) {
    use crate::v9::{
        paint,
        tokens::{palette, radius, spacing, TextRole},
    };
    use egui::{Align2, Pos2, Rect, Sense, Vec2};

    let warning_lines = entry.warnings.len().clamp(1, 3) as f32;
    let row_h = if entry.warnings.is_empty() {
        62.0
    } else {
        56.0 + warning_lines * 16.0
    };
    let (raw_rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
    let rect = raw_rect.shrink2(Vec2::new(0.0, 1.0));
    let painter = ui.painter().clone();
    paint::paint_bevel(
        &painter,
        rect,
        palette::SOOT_BLACK,
        palette::EDGE_DARK,
        radius::R1,
    );
    paint::paint_plate_grain(&painter, rect.shrink(4.0), 3.0, 2);
    painter.rect_filled(
        Rect::from_min_max(
            rect.min + Vec2::new(3.0, 5.0),
            Pos2::new(rect.left() + 7.0, rect.bottom() - 5.0),
        ),
        egui::epaint::CornerRadius::ZERO,
        palette::WARN,
    );

    let left = rect.left() + spacing::S5;
    let right = rect.right() - spacing::S4;
    painter
        .with_clip_rect(Rect::from_min_max(
            Pos2::new(left, rect.top() + 6.0),
            Pos2::new(right, rect.top() + 28.0),
        ))
        .text(
            Pos2::new(left, rect.top() + 7.0),
            Align2::LEFT_TOP,
            format!("{} Lv {}", entry.building_name, entry.total_level),
            TextRole::Subheading.font_id(),
            palette::PARCHMENT,
        );
    painter
        .with_clip_rect(Rect::from_min_max(
            Pos2::new(left, rect.top() + 30.0),
            Pos2::new(right, rect.top() + 48.0),
        ))
        .text(
            Pos2::new(left, rect.top() + 31.0),
            Align2::LEFT_TOP,
            format!(
                "就业 {:.0}%  |  每周收支 {:+.1}M RM",
                entry.employment_rate.clamp(0.0, 1.0) * 100.0,
                entry.profit_rm_weekly / 1_000_000.0,
            ),
            TextRole::Caption.font_id(),
            if entry.profit_rm_weekly < 0.0 {
                palette::BAD
            } else {
                palette::PARCHMENT_DIM
            },
        );

    if !entry.warnings.is_empty() {
        let warning_text = entry
            .warnings
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ");
        painter
            .with_clip_rect(Rect::from_min_max(
                Pos2::new(left, rect.top() + 50.0),
                Pos2::new(right, rect.bottom() - 4.0),
            ))
            .text(
                Pos2::new(left, rect.top() + 51.0),
                Align2::LEFT_TOP,
                warning_text,
                TextRole::Small.font_id(),
                palette::WARN,
            );
    }
    ui.add_space(spacing::S2);
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
            ("已分配CP", format!("{:.0}", data.allocated_cp)),
            ("闲置CP", format!("{:.0}", data.idle_cp)),
            ("受阻CP", format!("{:.0}", data.blocked_cp)),
            ("行政CP", format!("{:.0}", data.national_admin_cp)),
            ("建设部门CP", format!("{:.0}", data.construction_sector_cp)),
            ("劳力CP", format!("{:.0}", data.regional_labor_cp)),
            ("工程CP", format!("{:.0}", data.engineering_equipment_cp)),
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
    use crate::v9::tokens::palette;

    let (label, text, color) = if data.investment_pool.total_rm <= 0.0 {
        (
            "投资池枯竭",
            "私人/法团/外资账户暂无可用资金，民间扩建会明显放慢。",
            palette::WARN,
        )
    } else if data.auto_build_enabled {
        (
            "自动建造启用",
            "系统会根据短缺、军工和建设能力自动补入队列。",
            palette::GOOD,
        )
    } else if cp_ratio < 0.3 {
        (
            "建造能力紧张",
            "可用建造能力偏低，建议先恢复建设预算或压缩支出。",
            palette::WARN,
        )
    } else {
        (
            "建造体系稳定",
            "当前建造能力尚可，重点处理队列和问题建筑即可。",
            palette::GOOD,
        )
    };

    egui::Frame::new()
        .fill(palette::SOOT_BLACK)
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
                    .color(palette::PARCHMENT),
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
    use crate::v9::tokens::palette;

    egui::Frame::new()
        .fill(palette::SOOT_BLACK)
        .stroke(egui::Stroke::new(1.0, palette::BRASS_DARK))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.columns(3, |columns| {
                columns[0].label(
                    RichText::new("建造力")
                        .small()
                        .color(palette::PARCHMENT_DIM),
                );
                columns[0].add(
                    egui::ProgressBar::new(cp_ratio.clamp(0.0, 1.0))
                        .fill(palette::GOLD_HOT)
                        .text(format!("{:.0} / {:.0}", data.available_cp, data.total_cp)),
                );
                columns[0].label(
                    RichText::new(format!(
                        "闲置 {:.0}  受阻 {:.0}",
                        data.idle_cp, data.blocked_cp
                    ))
                    .small()
                    .color(if data.blocked_cp > 0.0 {
                        palette::WARN
                    } else {
                        palette::MUTED
                    }),
                );

                columns[0].label(
                    RichText::new(cp_source_summary(data))
                        .small()
                        .color(palette::PARCHMENT_DIM),
                );

                columns[1].label(
                    RichText::new("投资池")
                        .small()
                        .color(palette::PARCHMENT_DIM),
                );
                columns[1].label(
                    RichText::new(format_rm_stock(data.investment_pool.total_rm))
                        .strong()
                        .color(palette::GOLD_HOT),
                );
                columns[1].label(
                    RichText::new(format!(
                        "私人 {}  法团 {}  外资 {}",
                        format_rm_stock(data.investment_pool.private_rm),
                        format_rm_stock(data.investment_pool.cartel_rm),
                        format_rm_stock(data.investment_pool.foreign_capital_rm),
                    ))
                    .small()
                    .color(palette::MUTED),
                );

                columns[2].horizontal_wrapped(|ui| {
                    let mut enabled = data.auto_build_enabled;
                    if ui.checkbox(&mut enabled, "自动建造").changed() {
                        cmds.push(ConstructionV6Command::ToggleAutoBuild(enabled));
                    }
                    ui.label(
                        RichText::new("每月按短缺、军工和建设能力补队列")
                            .small()
                            .color(palette::MUTED),
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
                    .color(palette::MUTED),
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
    use crate::v9::{
        primitives::{Button, ButtonSize, ButtonVariant},
        tokens::palette,
    };

    ui.label(
        RichText::new(tr("construction_queue"))
            .strong()
            .color(palette::GOLD_HOT),
    );
    if entries.is_empty() {
        ui.label(tr("empty_construction"));
        return;
    }

    for (idx, entry) in entries.iter().enumerate() {
        egui::Frame::new()
            .fill(palette::SOOT_BLACK)
            .stroke(egui::Stroke::new(1.0, palette::BRASS_DARK))
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
                        .fill(palette::INFO),
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
                        .color(palette::PARCHMENT_DIM),
                    );
                });
                ui.label(
                    RichText::new(format!(
                        "配方：CP {:.0} | {:.1}M RM | 劳力 {} | 工程 {}",
                        entry.recipe_cp_cost,
                        entry.recipe_funds_rm / 1_000_000.0,
                        entry.recipe_labor,
                        entry.recipe_engineering
                    ))
                    .small()
                    .color(palette::PARCHMENT_DIM),
                );
                if !entry.recipe_materials_summary.is_empty() {
                    ui.label(
                        RichText::new(format!("材料：{}", entry.recipe_materials_summary))
                            .small()
                            .color(palette::PARCHMENT_DIM),
                    );
                }
                if !entry.recipe_region_summary.is_empty() {
                    ui.label(
                        RichText::new(format!("地区限制：{}", entry.recipe_region_summary))
                            .small()
                            .color(palette::PARCHMENT_DIM),
                    );
                }
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
                                palette::WARN
                            } else {
                                palette::PARCHMENT_DIM
                            }),
                        );
                        if entry.fund_ratio < 1.0 {
                            ui.label(
                                RichText::new(format!(
                                    "资金短缺·速度 ×{:.0}%",
                                    entry.fund_ratio * 100.0
                                ))
                                .small()
                                .color(palette::BAD),
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
                        .color(palette::WARN),
                    );
                }
                ui.label(
                    RichText::new(format!(
                        "CP 分配 {:.1} / 有效 {:.1} / 受阻 {:.1} | 瓶颈 {} | ETA {}",
                        entry.allocated_cp,
                        entry.effective_cp,
                        entry.blocked_cp,
                        entry.bottleneck_label,
                        entry
                            .estimated_days
                            .map(|days| format!("{days} 天"))
                            .unwrap_or_else(|| "未知".to_owned()),
                    ))
                    .small()
                    .color(if entry.blocked_cp > 0.0 || entry.paused {
                        palette::WARN
                    } else {
                        palette::PARCHMENT_DIM
                    }),
                );
                ui.horizontal(|ui| {
                    let mut paused = entry.paused;
                    if ui.checkbox(&mut paused, "暂停").changed() {
                        cmds.push(ConstructionV6Command::ToggleProjectPaused(idx, paused));
                    }
                    let mut priority = entry.priority;
                    if ui
                        .add(
                            egui::DragValue::new(&mut priority)
                                .speed(1.0)
                                .prefix("优先级 "),
                        )
                        .changed()
                    {
                        cmds.push(ConstructionV6Command::SetProjectPriority { idx, priority });
                    }
                    let mut weight = entry.weight;
                    if ui
                        .add(
                            egui::DragValue::new(&mut weight)
                                .speed(0.1)
                                .range(0.1..=10.0)
                                .prefix("权重 "),
                        )
                        .changed()
                    {
                        cmds.push(ConstructionV6Command::SetProjectWeight {
                            idx,
                            weight: weight.max(0.1),
                        });
                    }
                });
                ui.horizontal(|ui| {
                    let move_up_clicked = Button::new(tr("move_up"))
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .enabled(idx > 0)
                        .show(ui)
                        .clicked();
                    if idx > 0 && move_up_clicked {
                        cmds.push(ConstructionV6Command::MoveUp(idx));
                    }
                    let move_down_clicked = Button::new(tr("move_down"))
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Secondary)
                        .enabled(idx + 1 < entries.len())
                        .show(ui)
                        .clicked();
                    if idx + 1 < entries.len() && move_down_clicked {
                        cmds.push(ConstructionV6Command::MoveDown(idx));
                    }
                    if Button::new(tr("cancel_construction"))
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Danger)
                        .show(ui)
                        .clicked()
                    {
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
    use crate::v9::{
        primitives::{Button, ButtonSize, ButtonVariant},
        tokens::palette,
    };

    egui::Frame::new()
        .fill(palette::SOOT_BLACK)
        .stroke(egui::Stroke::new(1.0, palette::BRASS_DARK))
        .inner_margin(6.0)
        .outer_margin(2.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("{}  Lv {}", entry.building_name, entry.total_level))
                            .color(palette::PARCHMENT),
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
                        .color(palette::PARCHMENT_DIM),
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
                        ui.label(RichText::new(warning).small().color(palette::WARN));
                    }
                });
            }
            ui.label(
                RichText::new(format!("{}：{}", tr("production_method"), entry.pm_summary))
                    .small()
                    .color(palette::INFO),
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

            if Button::new(tr("expand_one_level"))
                .size(ButtonSize::Md)
                .variant(ButtonVariant::Primary)
                .show(ui)
                .clicked()
            {
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
            state_id: 51,
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
            sector: ConstructionV9Sector::Secondary,
            facility_class: ConstructionV9FacilityClass::Standard,
            total_level: 2,
            employment_rate: 0.8,
            profit_rm_weekly: 2_100_000.0,
            outputs: vec![BuildingGoodFlowEntry {
                good_id: "steel".into(),
                good_name: "钢材".into(),
                amount: 40.0,
            }],
            inputs: vec![BuildingGoodFlowEntry {
                good_id: "coal".into(),
                good_name: "煤炭".into(),
                amount: 20.0,
            }],
            output_summary: "钢材 +40/d".into(),
            input_summary: "煤炭 -20/d".into(),
            warnings: vec!["投入品短缺：煤炭 12%".into()],
            pm_summary: "基础炼钢".into(),
            states: vec![sample_state_entry()],
        }
    }

    fn sample_buildable(
        id: &str,
        sector: ConstructionV9Sector,
        facility_class: ConstructionV9FacilityClass,
    ) -> BuildableBuildingEntry {
        BuildableBuildingEntry {
            building_def_id: id.into(),
            building_name: id.into(),
            group_name: "测试".into(),
            sector,
            facility_class,
            recipe_cp_cost: 1_000.0,
            recipe_funds_rm: 50_000_000.0,
            recipe_materials_summary: "steel 20".into(),
            recipe_labor: 10,
            recipe_engineering: 5,
            recipe_region_summary: String::new(),
            locked_reason: None,
            state_limit_reason: None,
        }
    }

    fn sample_queue_entry() -> ConstructionQueueV6Entry {
        ConstructionQueueV6Entry {
            building_key: "steel_mill".into(),
            building_name: "钢铁厂".into(),
            state_id: 51,
            state_name: "莱茵兰".into(),
            current_level: 2,
            target_level: 3,
            recipe_cp_cost: 9_800.0,
            recipe_funds_rm: 520_000_000.0,
            recipe_materials_summary: "steel 160, machinery 65".into(),
            recipe_labor: 1_180,
            recipe_engineering: 90,
            recipe_region_summary: String::new(),
            progress: 0.35,
            funding_source_label: "政府".into(),
            owner_on_completion_label: "国有".into(),
            paid_funds_rm: 50_000_000.0,
            budget_needed_rm: 360_000_000.0,
            material_fulfillment: 0.85,
            fund_ratio: 1.0,
            priority: 1,
            weight: 1.5,
            paused: false,
            allocated_cp: 40.0,
            effective_cp: 34.0,
            blocked_cp: 6.0,
            bottleneck_label: "materials".into(),
            estimated_days: Some(18),
        }
    }

    fn sample_panel_data() -> ConstructionV6PanelData {
        ConstructionV6PanelData {
            entries: vec![sample_entry()],
            queue: vec![sample_queue_entry()],
            buildable_catalog: vec![sample_buildable(
                "steel_mill",
                ConstructionV9Sector::Secondary,
                ConstructionV9FacilityClass::Standard,
            )],
            active_construction_key: Some("steel_mill".into()),
            available_cp: 50.0,
            total_cp: 100.0,
            national_admin_cp: 20.0,
            construction_sector_cp: 35.0,
            regional_labor_cp: 25.0,
            engineering_equipment_cp: 20.0,
            finance_cp: 90.0,
            material_cp: 80.0,
            allocated_cp: 50.0,
            idle_cp: 25.0,
            blocked_cp: 25.0,
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
        let data = sample_panel_data();
        assert_eq!(data.entries.len(), 1);
        assert_eq!(data.queue.len(), 1);
        assert_eq!(data.buildable_catalog.len(), 1);
        assert!((data.available_cp - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn construction_v9_secondary_tabs_are_complete() {
        let labels: Vec<&str> = construction_v9_tab_order()
            .into_iter()
            .map(ConstructionPanelTab::label)
            .collect();
        assert_eq!(
            labels,
            vec![
                "总览",
                "队列",
                "建筑目录",
                "一产建筑",
                "二产建筑",
                "三产建筑",
                "基础设施",
                "军事设施",
                "瓶颈",
                "自动建设",
            ]
        );
    }

    #[test]
    fn construction_v9_footer_is_player_visible_chinese() {
        assert!(CONSTRUCTION_V9_FOOTER.contains("关闭"));
        assert!(CONSTRUCTION_V9_FOOTER.contains("总览"));
        assert!(CONSTRUCTION_V9_FOOTER.contains("队列"));
        assert!(!CONSTRUCTION_V9_FOOTER.contains("Close"));
        assert!(!CONSTRUCTION_V9_FOOTER.contains("Overview"));
    }

    #[test]
    fn construction_catalog_sector_filter_counts_entries() {
        let mut data = sample_panel_data();
        data.entries.push(BuildingTypeV6Entry {
            building_def_id: "grain_farm".into(),
            building_name: "农场".into(),
            sector: ConstructionV9Sector::Primary,
            facility_class: ConstructionV9FacilityClass::Standard,
            warnings: Vec::new(),
            ..sample_entry()
        });
        data.entries.push(BuildingTypeV6Entry {
            building_def_id: "railway".into(),
            building_name: "铁路".into(),
            sector: ConstructionV9Sector::Secondary,
            facility_class: ConstructionV9FacilityClass::Infrastructure,
            warnings: Vec::new(),
            ..sample_entry()
        });
        data.entries.push(BuildingTypeV6Entry {
            building_def_id: "arms_industry".into(),
            building_name: "军工厂".into(),
            sector: ConstructionV9Sector::Secondary,
            facility_class: ConstructionV9FacilityClass::Military,
            warnings: Vec::new(),
            ..sample_entry()
        });
        data.buildable_catalog.push(sample_buildable(
            "grain_farm",
            ConstructionV9Sector::Primary,
            ConstructionV9FacilityClass::Standard,
        ));
        data.buildable_catalog.push(sample_buildable(
            "railway",
            ConstructionV9Sector::Secondary,
            ConstructionV9FacilityClass::Infrastructure,
        ));
        data.buildable_catalog.push(sample_buildable(
            "arms_industry",
            ConstructionV9Sector::Secondary,
            ConstructionV9FacilityClass::Military,
        ));

        assert_eq!(
            count_entries_for_tab(&data, ConstructionPanelTab::Primary),
            1
        );
        assert_eq!(
            count_entries_for_tab(&data, ConstructionPanelTab::Secondary),
            1
        );
        assert_eq!(
            count_entries_for_tab(&data, ConstructionPanelTab::Infrastructure),
            1
        );
        assert_eq!(
            count_entries_for_tab(&data, ConstructionPanelTab::Military),
            1
        );
        assert!(catalog_entry_matches_tab(
            &data.buildable_catalog[1],
            ConstructionPanelTab::Primary
        ));
        assert!(catalog_entry_matches_tab(
            &data.buildable_catalog[2],
            ConstructionPanelTab::Infrastructure
        ));
    }

    #[test]
    fn construction_bottleneck_panel_counts_queue_and_building_problems() {
        let mut data = sample_panel_data();
        assert_eq!(count_bottlenecks(&data), 2);
        data.queue[0].blocked_cp = 0.0;
        data.queue[0].bottleneck_label = "none".into();
        data.entries[0].warnings.clear();
        assert_eq!(count_bottlenecks(&data), 0);
    }

    #[test]
    fn construction_overview_explains_cp() {
        let text = cp_explanation_text();
        assert!(text.contains("已分配 CP"));
        assert!(text.contains("闲置 CP"));
        assert!(text.contains("受阻 CP"));
        assert!(text.contains("资金"));
        assert!(text.contains("材料"));
    }

    #[test]
    fn construction_queue_actions_complete() {
        let actions = [
            ConstructionV6Command::MoveUp(1),
            ConstructionV6Command::MoveDown(1),
            ConstructionV6Command::Remove(1),
            ConstructionV6Command::ToggleProjectPaused(1, true),
            ConstructionV6Command::SetProjectPriority {
                idx: 1,
                priority: 2,
            },
            ConstructionV6Command::SetProjectWeight {
                idx: 1,
                weight: 1.25,
            },
        ];
        assert_eq!(actions.len(), 6);
    }

    #[test]
    fn construction_queue_entries_expose_per_project_cp_and_eta() {
        let data = sample_panel_data();
        let item = &data.queue[0];
        assert!(item.allocated_cp > 0.0);
        assert!(item.effective_cp > 0.0);
        assert!(item.blocked_cp > 0.0);
        assert_eq!(item.bottleneck_label, "materials");
        assert_eq!(item.estimated_days, Some(18));
        assert!(item.recipe_cp_cost > 0.0);
        assert!(item.recipe_funds_rm > 0.0);
        assert!(item.recipe_labor > 0);
        assert!(item.recipe_engineering > 0);
    }

    #[test]
    fn construction_capacity_sources_are_split_for_ui() {
        let data = sample_panel_data();
        assert!(data.national_admin_cp > 0.0);
        assert!(data.construction_sector_cp > 0.0);
        assert!(data.regional_labor_cp > 0.0);
        assert!(data.engineering_equipment_cp > 0.0);
        assert!(data.finance_cp > 0.0);
        assert!(data.material_cp > 0.0);
        let summary = cp_source_summary(&data);
        assert!(summary.contains("行政"));
        assert!(summary.contains("建设"));
        assert!(summary.contains("劳力"));
        assert!(summary.contains("工程"));
        assert!(summary.contains("资金"));
        assert!(summary.contains("材料"));
    }
}
