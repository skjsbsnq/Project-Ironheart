use crate::{
    air::AirData,
    construction_v6_panel::{
        BuildableBuildingEntry, BuildingGoodFlowEntry, BuildingTypeV6Entry, ConstructionV6PanelData,
    },
    decisions_panel::DecisionsData,
    diplomacy::{self, DiplomacyCommand, DiplomacyData},
    finance_panel::FinancePanelData,
    law_panel::{self, LawCommand, LawPanelData, LawSlotEntry, LawTierEntry},
    logistics_panel::LogisticsData,
    market_panel::{GoodCategory, GoodEntry, MarketPanelData},
    military::MilitaryData,
    naval::NavalData,
    pop_panel::PopPanelData,
    province_info::ProvinceInfoData,
    research::ResearchData,
    vanilla_iron::{DossierPanelShell, VanillaIron},
    ActiveDetailPanel, ActivePrimaryPanel, BuildingDetailTarget, CountryDetailTarget, DetailSource,
    GoodsDetailTarget, PanelCommand, PopGroupDetailTarget, ProvinceDetailTarget, StateDetailTarget,
};
use egui::{Color32, RichText, Sense, Vec2};
use hoi4_content::focus::{Focus, FocusTree};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct DetailPanelOutput {
    pub panel_command: Option<PanelCommand>,
    pub law_commands: Vec<LawCommand>,
    pub diplomacy_commands: Vec<DiplomacyCommand>,
    pub decision_commands: Vec<crate::politics::DecisionCommand>,
}

pub struct DetailPanelHost;

impl DetailPanelHost {
    pub fn show(
        ctx: &egui::Context,
        detail: Option<&ActiveDetailPanel>,
        market_data: Option<&MarketPanelData>,
        finance_data: Option<&FinancePanelData>,
        law_data: Option<&LawPanelData>,
        construction_data: Option<&ConstructionV6PanelData>,
        pop_data: Option<&PopPanelData>,
        diplomacy_data: Option<&DiplomacyData>,
        province_data: Option<&ProvinceInfoData>,
        military_data: Option<&MilitaryData>,
        naval_data: Option<&NavalData>,
        air_data: Option<&AirData>,
        logistics_data: Option<&LogisticsData>,
        research_data: Option<&ResearchData>,
        decisions_data: Option<&DecisionsData>,
        focus_tree: Option<&FocusTree>,
        completed_focuses: Option<&HashSet<String>>,
        current_focus: Option<&str>,
        current_focus_progress: f32,
        available_focus_ids: Option<&HashSet<String>>,
    ) -> Option<DetailPanelOutput> {
        let detail = detail?;
        let mut output = DetailPanelOutput::default();
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            Some(PanelCommand::CloseDetail)
        } else {
            None
        }
        .map(|cmd| output.panel_command = Some(cmd));

        let title = detail_title(detail);
        let mut shell = DossierPanelShell::new("gate1_detail_panel_host", title)
            .subtitle("统一对象详情")
            .footer("Esc 返回");
        if matches!(detail, ActiveDetailPanel::Province(_)) {
            let height = (ctx.screen_rect().height() - 104.0).clamp(420.0, 640.0);
            shell = shell
                .left_bottom(Vec2::new(142.0, 0.0))
                .fixed_size(Vec2::new(430.0, height));
        }
        let (close_clicked, _) = shell.show(ctx, |ui, layout| {
            let top = layout.top_strip.shrink2(Vec2::new(8.0, 7.0));
            let mut header_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("detail_header")
                    .max_rect(top)
                    .layout(egui::Layout::top_down(egui::Align::Min))
                    .sense(egui::Sense::hover()),
            );
            header_ui.set_clip_rect(top);
            if let Some(province) = province_summary_for_detail(detail, province_data) {
                VanillaIron::section_heading(&mut header_ui, &province.province_name);
                header_ui.label(
                    RichText::new(format!("所属州：{}", province.state_name))
                        .small()
                        .color(VanillaIron::MUTED),
                );
            } else {
                VanillaIron::section_heading(&mut header_ui, title);
                if !matches!(detail, ActiveDetailPanel::Province(_)) {
                    header_ui.label(
                        RichText::new(detail_identity(detail))
                            .small()
                            .color(VanillaIron::MUTED),
                    );
                }
            }

            let body = layout.main.shrink2(Vec2::new(8.0, 7.0));
            let mut body_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("detail_body")
                    .max_rect(body)
                    .layout(egui::Layout::top_down(egui::Align::Min))
                    .sense(egui::Sense::hover()),
            );
            body_ui.set_clip_rect(body);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_viewport(&mut body_ui, |ui, _| {
                    ui.set_min_size(layout.main.size());
                    match detail {
                        ActiveDetailPanel::Goods(target) => {
                            show_goods_detail(ui, target, market_data);
                        }
                        ActiveDetailPanel::Law { category, law_id } => {
                            show_law_detail(
                                ui,
                                category,
                                law_id.as_deref(),
                                law_data,
                                &mut output.law_commands,
                            );
                        }
                        ActiveDetailPanel::FinanceDebt => {
                            show_finance_debt_detail(ui, finance_data);
                        }
                        ActiveDetailPanel::Building(target) => {
                            show_building_detail(
                                ui,
                                target,
                                construction_data,
                                &mut output.panel_command,
                            );
                        }
                        ActiveDetailPanel::State(target) => {
                            show_state_detail(
                                ui,
                                target,
                                province_data,
                                pop_data,
                                construction_data,
                                &mut output.panel_command,
                            );
                        }
                        ActiveDetailPanel::Province(target) => {
                            show_province_detail(
                                ui,
                                target,
                                province_data,
                                &mut output.panel_command,
                            );
                        }
                        ActiveDetailPanel::Country(target) => {
                            show_country_detail(
                                ui,
                                target,
                                diplomacy_data,
                                &mut output.diplomacy_commands,
                            );
                        }
                        ActiveDetailPanel::PopGroup(target) => {
                            show_pop_detail(ui, target, pop_data, &mut output.panel_command);
                        }
                        ActiveDetailPanel::Army(target) => {
                            show_army_detail(
                                ui,
                                target.army_id,
                                military_data,
                                logistics_data,
                                &mut output.panel_command,
                            );
                        }
                        ActiveDetailPanel::Fleet(target) => {
                            show_fleet_detail(
                                ui,
                                target.fleet_id,
                                naval_data,
                                &mut output.panel_command,
                            );
                        }
                        ActiveDetailPanel::AirWing(target) => {
                            show_air_wing_detail(
                                ui,
                                target.air_wing_id,
                                air_data,
                                &mut output.panel_command,
                            );
                        }
                        ActiveDetailPanel::Technology(target) => {
                            show_technology_detail(ui, &target.technology_key, research_data);
                        }
                        ActiveDetailPanel::Focus(target) => {
                            show_focus_detail(
                                ui,
                                &target.focus_id,
                                focus_tree,
                                completed_focuses,
                                current_focus,
                                current_focus_progress,
                                available_focus_ids,
                            );
                        }
                        ActiveDetailPanel::JournalEntry(target) => {
                            show_journal_entry_detail(
                                ui,
                                &target.entry_id,
                                decisions_data,
                                &mut output.decision_commands,
                            );
                        }
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if VanillaIron::compact_button(ui, "关闭").clicked() {
                            output.panel_command = Some(PanelCommand::CloseDetail);
                        }
                    });
                });
        });

        if close_clicked {
            output.panel_command = Some(PanelCommand::CloseDetail);
        }

        Some(output)
    }
}

fn detail_identity(detail: &ActiveDetailPanel) -> String {
    match detail {
        ActiveDetailPanel::Goods(target) => format!("商品 ID：{}", target.good_id),
        ActiveDetailPanel::Building(target) => format!("建筑：{}", target.building_key),
        ActiveDetailPanel::State(target) => format!("州 ID：{}", target.state_id),
        ActiveDetailPanel::Province(target) => format!("省份 ID：{}", target.province_id),
        ActiveDetailPanel::Country(target) => format!("国家：{}", target.tag),
        ActiveDetailPanel::Army(target) => format!("军队 ID：{}", target.army_id),
        ActiveDetailPanel::Fleet(target) => format!("舰队 ID：{}", target.fleet_id),
        ActiveDetailPanel::AirWing(target) => format!("联队 ID：{}", target.air_wing_id),
        ActiveDetailPanel::Law { category, law_id } => {
            format!(
                "法律类别：{} / {}",
                category,
                law_id.as_deref().unwrap_or("当前法律")
            )
        }
        ActiveDetailPanel::Technology(target) => format!("科技：{}", target.technology_key),
        ActiveDetailPanel::Focus(target) => format!("国策：{}", target.focus_id),
        ActiveDetailPanel::PopGroup(target) => format!("人口组：{}", target.pop_group_key),
        ActiveDetailPanel::JournalEntry(target) => format!("日志：{}", target.entry_id),
        ActiveDetailPanel::FinanceDebt => "财政债务".to_owned(),
    }
}

fn province_summary_for_detail<'a>(
    detail: &ActiveDetailPanel,
    province_data: Option<&'a ProvinceInfoData>,
) -> Option<&'a crate::province_info::ProvinceTacticalInfo> {
    let ActiveDetailPanel::Province(target) = detail else {
        return None;
    };
    province_data
        .filter(|data| data.province.province_id == target.province_id)
        .map(|data| &data.province)
}

fn detail_title(detail: &ActiveDetailPanel) -> &'static str {
    match detail {
        ActiveDetailPanel::Goods(_) => "商品详情",
        ActiveDetailPanel::Building(_) => "建筑详情",
        ActiveDetailPanel::State(_) => "州详情",
        ActiveDetailPanel::Province(_) => "省份详情",
        ActiveDetailPanel::Country(_) => "国家详情",
        ActiveDetailPanel::Army(_) => "军队详情",
        ActiveDetailPanel::Fleet(_) => "舰队详情",
        ActiveDetailPanel::AirWing(_) => "联队详情",
        ActiveDetailPanel::Law { .. } => "法律详情",
        ActiveDetailPanel::Technology(_) => "科技详情",
        ActiveDetailPanel::Focus(_) => "国策详情",
        ActiveDetailPanel::PopGroup(_) => "人口组详情",
        ActiveDetailPanel::JournalEntry(_) => "日志详情",
        ActiveDetailPanel::FinanceDebt => "债务详情",
    }
}

fn show_goods_detail(
    ui: &mut egui::Ui,
    target: &GoodsDetailTarget,
    market_data: Option<&MarketPanelData>,
) {
    let good =
        market_data.and_then(|data| data.goods.iter().find(|entry| entry.id == target.good_id));

    if let Some(good) = good {
        goods_header(ui, good, target.source);
        ui.add_space(8.0);
        goods_metrics(ui, good);
        ui.add_space(8.0);
        flow_section(ui, "供给来源", &good.producers, good.domestic_production);
        flow_section(ui, "需求来源", &good.consumers, good.demand);
        flow_section(
            ui,
            "政府采购",
            &good.government_orders,
            good.military_order_demand,
        );
        if !good.shortage_reason.is_empty() {
            ui.add_space(6.0);
            VanillaIron::warning_row(ui, &good.shortage_reason);
        }
    } else {
        ui.heading("商品详情");
        ui.label("商品数据尚未载入。请保持市场面板打开，或从市场商品行重新打开详情。");
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("内部 ID：{}", target.good_id))
                .small()
                .color(Color32::from_rgb(0xa8, 0x9b, 0x78)),
        );
    }
}

fn goods_header(ui: &mut egui::Ui, good: &GoodEntry, source: Option<DetailSource>) {
    VanillaIron::section_heading(ui, &good.name);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(good_category_label(good.category))
                .strong()
                .color(good_category_color(good.category)),
        );
        if let Some(source) = source {
            ui.label(
                RichText::new(format!("来源：{}", detail_source_label(source)))
                    .small()
                    .color(Color32::from_rgb(0xa8, 0x9b, 0x78)),
            );
        }
    });
}

fn goods_metrics(ui: &mut egui::Ui, good: &GoodEntry) {
    let shortage = good.unmet_demand.max((good.demand - good.supply).max(0.0));
    key_value(
        ui,
        "价格",
        format!("{:.2} / 基准 {:.2}", good.price, good.base_price),
    );
    key_value(ui, "供给", format!("{:.1}/日", good.supply));
    key_value(ui, "需求", format!("{:.1}/日", good.demand));
    key_value(
        ui,
        "短缺",
        if shortage > 0.0 {
            format!("{:.1}/日", shortage)
        } else {
            "无".to_owned()
        },
    );
    key_value(
        ui,
        "库存覆盖",
        format!("{:.1} 天", good.stockpile_coverage_days),
    );
    key_value(
        ui,
        "进口 / 出口",
        format!("{:.1} / {:.1}", good.imports, good.exports),
    );
}

fn flow_section(
    ui: &mut egui::Ui,
    title: &str,
    rows: &[crate::market_panel::GoodFlowSource],
    total: f32,
) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(title)
            .strong()
            .color(Color32::from_rgb(0xd0, 0xb0, 0x6a)),
    );
    if rows.is_empty() {
        ui.label(
            RichText::new(if total > 0.0 {
                format!("总量 {:.1}/日，暂无可细分来源。", total)
            } else {
                "暂无数据。".to_owned()
            })
            .small()
            .color(Color32::from_rgb(0xa8, 0x9b, 0x78)),
        );
        return;
    }
    for row in rows.iter().take(6) {
        key_value(ui, &row.name, format!("{:.1}/日", row.amount));
    }
}

fn show_placeholder_detail(ui: &mut egui::Ui, title: &str, summary: &str, identity: &str) {
    ui.heading(title);
    ui.label(summary);
    ui.add_space(6.0);
    ui.label(
        RichText::new(identity)
            .small()
            .color(Color32::from_rgb(0xa8, 0x9b, 0x78)),
    );
}

fn show_finance_debt_detail(ui: &mut egui::Ui, finance_data: Option<&FinancePanelData>) {
    let Some(data) = finance_data else {
        ui.heading("债务详情");
        ui.label("财政数据尚未载入。请保持财政面板打开，或从财政账本重新打开债务详情。");
        return;
    };
    let total_debt = data.public_debt_rm + data.mefo_debt_rm;
    let debt_ratio = if data.gdp_rm > 0.0 {
        total_debt / data.gdp_rm
    } else {
        0.0
    };
    let mefo_ratio = if data.gdp_rm > 0.0 {
        data.mefo_debt_rm / data.gdp_rm
    } else {
        0.0
    };

    VanillaIron::section_heading(ui, "债务总览");
    key_value(ui, "公共债务", format_rm(data.public_debt_rm));
    key_value(
        ui,
        "外债折算",
        format!("GBP {}", format_rm(data.public_debt_gbp)),
    );
    key_value(ui, "MEFO 债务", format_rm(data.mefo_debt_rm));
    key_value(ui, "债务/GDP", format!("{:.1}%", debt_ratio * 100.0));
    key_value(ui, "MEFO/GDP", format!("{:.1}%", mefo_ratio * 100.0));
    key_value(ui, "信用评级", data.credit_rating.clone());
    key_value(
        ui,
        "债券利率",
        format!("{:.1}%", data.bond_interest_rate * 100.0),
    );
    ui.add_space(8.0);

    VanillaIron::section_heading(ui, "本日融资");
    key_value(
        ui,
        "国内债券",
        format_rm(data.financing_breakdown.domestic_bond_issued_rm),
    );
    key_value(
        ui,
        "MEFO 签发",
        format_rm(data.financing_breakdown.mefo_issued_rm),
    );
    key_value(
        ui,
        "MEFO 利息资本化",
        format_rm(data.financing_breakdown.mefo_interest_capitalized_rm),
    );
    key_value(ui, "MEFO 覆盖赤字", format_rm(data.mefo_coverage_rm));
    key_value(ui, "利息支出", format_rm(data.fiscal_expense.interest_rm));
    ui.add_space(8.0);

    VanillaIron::section_heading(ui, "军工票据");
    key_value(ui, "MEFO 军工预算", format_rm(data.mefo_military_budget_rm));
    key_value(ui, "MEFO 军工已用", format_rm(data.mefo_military_spent_rm));
    if data.mefo_debt_rm > 0.0 && mefo_ratio >= 0.30 {
        VanillaIron::warning_row(
            ui,
            "MEFO/GDP 已达到高风险区间，继续覆盖赤字会扩大偿付压力。",
        );
    } else if data.daily_income_rm < data.daily_expense_rm {
        VanillaIron::warning_row(ui, "当前存在日赤字，需要债券、MEFO、黄金或削减支出覆盖。");
    }
}

fn show_building_detail(
    ui: &mut egui::Ui,
    target: &BuildingDetailTarget,
    construction_data: Option<&ConstructionV6PanelData>,
    panel_command: &mut Option<PanelCommand>,
) {
    let Some(data) = construction_data else {
        show_placeholder_detail(
            ui,
            "建筑详情",
            "建设数据尚未载入。请从建设工作台或地图州详情重新打开建筑。",
            &format!("建筑：{}", target.building_key),
        );
        return;
    };

    if let Some(entry) = find_building_entry(data, &target.building_key) {
        VanillaIron::section_heading(ui, &entry.building_name);
        key_value(ui, "等级", entry.total_level.to_string());
        key_value(
            ui,
            "就业率",
            format!("{:.0}%", entry.employment_rate * 100.0),
        );
        key_value(ui, "每周收支", signed_rm(entry.profit_rm_weekly));
        if let Some(state_id) = target.state_id {
            key_value(ui, "目标州", format!("州 ID {}", state_id));
        }
        if !entry.pm_summary.is_empty() {
            key_value(ui, "生产方式", entry.pm_summary.clone());
        }
        for warning in &entry.warnings {
            VanillaIron::warning_row(ui, warning);
        }
        ui.add_space(8.0);
        building_good_flows(ui, "投入商品", &entry.inputs, panel_command);
        building_good_flows(ui, "产出商品", &entry.outputs, panel_command);
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "州分布");
        for state in &entry.states {
            egui::Frame::new()
                .fill(VanillaIron::CARD_DEEP)
                .stroke(egui::Stroke::new(1.0, VanillaIron::EDGE_DARK))
                .inner_margin(egui::Margin::symmetric(7, 5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} Lv {}  就业 {:.0}%",
                                state.state_name,
                                state.level,
                                state.employment_rate * 100.0
                            ))
                            .color(VanillaIron::TEXT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("州详情").clicked() {
                                *panel_command = Some(PanelCommand::OpenDetail(
                                    ActiveDetailPanel::State(StateDetailTarget {
                                        state_id: state.state_id,
                                    }),
                                ));
                            }
                        });
                    });
                    ui.label(
                        RichText::new(format!("每周收支 {}", signed_rm(state.profit_rm_weekly)))
                            .small()
                            .color(if state.profit_rm_weekly < 0.0 {
                                VanillaIron::BAD
                            } else {
                                VanillaIron::MUTED
                            }),
                    );
                });
            ui.add_space(4.0);
        }
        return;
    }

    if let Some(entry) = find_buildable_entry(data, &target.building_key) {
        VanillaIron::section_heading(ui, &entry.building_name);
        key_value(ui, "分组", entry.group_name.clone());
        key_value(ui, "CP 成本", format!("{:.0}", entry.recipe_cp_cost));
        key_value(ui, "资金成本", format_rm(entry.recipe_funds_rm));
        key_value(ui, "劳力", entry.recipe_labor.to_string());
        key_value(ui, "工程", entry.recipe_engineering.to_string());
        if !entry.recipe_materials_summary.is_empty() {
            key_value(ui, "建材", entry.recipe_materials_summary.clone());
        }
        if !entry.recipe_region_summary.is_empty() {
            key_value(ui, "地区限制", entry.recipe_region_summary.clone());
        }
        if let Some(reason) = &entry.locked_reason {
            VanillaIron::warning_row(ui, &format!("无法建造：{reason}"));
        } else if let Some(reason) = &entry.state_limit_reason {
            VanillaIron::warning_row(ui, &format!("州限制：{reason}"));
        } else {
            ui.label(
                RichText::new("可从建设工作台进入地图建造模式。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
        }
        return;
    }

    show_placeholder_detail(
        ui,
        "建筑详情",
        "没有找到该建筑的建设数据。",
        &format!("建筑：{}", target.building_key),
    );
}

fn building_good_flows(
    ui: &mut egui::Ui,
    title: &str,
    flows: &[BuildingGoodFlowEntry],
    panel_command: &mut Option<PanelCommand>,
) {
    VanillaIron::section_heading(ui, title);
    if flows.is_empty() {
        ui.label(
            RichText::new("暂无商品流量数据。")
                .small()
                .color(VanillaIron::MUTED),
        );
        return;
    }
    for flow in flows {
        ui.horizontal(|ui| {
            if ui.link(&flow.good_name).clicked() {
                *panel_command = Some(PanelCommand::OpenDetail(ActiveDetailPanel::Goods(
                    GoodsDetailTarget::from_source(
                        flow.good_id.clone(),
                        DetailSource::Construction,
                    ),
                )));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:.1}/日", flow.amount.abs()));
            });
        });
    }
}

fn show_state_detail(
    ui: &mut egui::Ui,
    target: &StateDetailTarget,
    province_data: Option<&ProvinceInfoData>,
    pop_data: Option<&PopPanelData>,
    construction_data: Option<&ConstructionV6PanelData>,
    panel_command: &mut Option<PanelCommand>,
) {
    VanillaIron::section_heading(ui, "州详情");
    if let Some(data) = province_data.filter(|data| data.state.state_id == target.state_id) {
        let state = &data.state;
        key_value(ui, "州", state.state_name.clone());
        key_value(ui, "人口", format_count(state.population));
        key_value(ui, "类型", state.state_category.clone());
        key_value(ui, "基础设施", format!("{}/10", state.infrastructure));
        key_value(
            ui,
            "建筑槽",
            format!("{}/{}", state.slots_used, state.slots_max),
        );
        country_detail_link(
            ui,
            "所有者",
            &state.owner_name,
            &state.owner_tag,
            panel_command,
        );
        country_detail_link(
            ui,
            "控制者",
            &state.controller_name,
            &state.controller_tag,
            panel_command,
        );
        ui.add_space(8.0);
        state_resource_section(ui, state);
    } else if let Some(state) = pop_data.and_then(|data| {
        data.states
            .iter()
            .find(|state| state.state_id == target.state_id)
    }) {
        key_value(ui, "州", state.state_name.clone());
        key_value(ui, "人口", format_count(state.population));
        key_value(ui, "就业", format_count(state.employed));
        key_value(
            ui,
            "失业率",
            format!("{:.1}%", state.unemployment_rate * 100.0),
        );
        key_value(ui, "主导阶层", state.dominant_class.clone());
    } else {
        key_value(ui, "州 ID", target.state_id.to_string());
        ui.label(
            RichText::new("尚无当前州的地图或人口上下文数据。")
                .small()
                .color(VanillaIron::MUTED),
        );
    }

    if let Some(data) = construction_data {
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "建筑");
        let mut shown = 0usize;
        for entry in &data.entries {
            for state in entry
                .states
                .iter()
                .filter(|state| state.state_id == target.state_id)
            {
                shown += 1;
                ui.horizontal(|ui| {
                    if ui.link(&entry.building_name).clicked() {
                        *panel_command = Some(PanelCommand::OpenDetail(
                            ActiveDetailPanel::Building(BuildingDetailTarget {
                                building_key: entry.building_def_id.clone(),
                                state_id: Some(state.state_id),
                            }),
                        ));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Lv {}", state.level));
                    });
                });
            }
        }
        if shown == 0 {
            ui.label(
                RichText::new("该州暂无可显示建筑。")
                    .small()
                    .color(VanillaIron::MUTED),
            );
        }
    }
}

fn show_province_detail(
    ui: &mut egui::Ui,
    target: &ProvinceDetailTarget,
    province_data: Option<&ProvinceInfoData>,
    panel_command: &mut Option<PanelCommand>,
) {
    let Some(data) = province_data.filter(|data| data.province.province_id == target.province_id)
    else {
        show_placeholder_detail(
            ui,
            "省份详情",
            "请从地图点击省份后查看完整战术和州经济数据。",
            &format!("省份 ID：{}", target.province_id),
        );
        return;
    };
    let province = &data.province;
    let state = &data.state;
    VanillaIron::section_heading(ui, "基本信息");
    key_value(ui, "所属州", province.state_name.clone());
    key_value(
        ui,
        "省份类型",
        format!("{} / {}", province.province_type, province.terrain),
    );
    key_value(ui, "补给", format!("{:.0}", province.supply));
    if province.coastal {
        key_value(ui, "海岸", "是".to_owned());
    }
    if province.victory_points > 0 {
        key_value(ui, "胜利点", province.victory_points.to_string());
    }
    country_detail_link(
        ui,
        "所有者",
        &province.owner_name,
        &province.owner_tag,
        panel_command,
    );
    country_detail_link(
        ui,
        "控制者",
        &province.controller_name,
        &province.controller_tag,
        panel_command,
    );
    if ui.link("打开州详情").clicked() {
        *panel_command = Some(PanelCommand::OpenDetail(ActiveDetailPanel::State(
            StateDetailTarget {
                state_id: data.state.state_id,
            },
        )));
    }

    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "州经济信息");
    key_value(ui, "人口", format_count(state.population));
    key_value(ui, "州类别", state.state_category.clone());
    key_value(ui, "基础设施", format!("{}/10", state.infrastructure));
    key_value(
        ui,
        "建筑槽",
        format!("{}/{}", state.slots_used, state.slots_max),
    );
    if state.owner_tag != province.owner_tag {
        country_detail_link(
            ui,
            "州拥有者",
            &state.owner_name,
            &state.owner_tag,
            panel_command,
        );
    }
    if state.controller_tag != province.controller_tag {
        country_detail_link(
            ui,
            "州控制者",
            &state.controller_name,
            &state.controller_tag,
            panel_command,
        );
    }

    ui.add_space(8.0);
    state_resource_section(ui, state);
    ui.add_space(8.0);
    state_building_section(ui, state);
    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "驻军与节点");
    let visible_nodes = province
        .strategic_nodes
        .iter()
        .filter(|node| !province_node_duplicates_basic_info(node, province));
    if province.divisions.is_empty() && visible_nodes.clone().next().is_none() {
        ui.label(
            RichText::new("暂无驻军或战略节点。")
                .small()
                .color(VanillaIron::MUTED),
        );
    } else {
        for node in visible_nodes {
            ui.label(RichText::new(node).small().color(VanillaIron::TEXT));
        }
        for division in &province.divisions {
            ui.label(
                RichText::new(format!("部队：{division}"))
                    .small()
                    .color(VanillaIron::TEXT),
            );
        }
    }
}

fn province_node_duplicates_basic_info(
    node: &str,
    province: &crate::province_info::ProvinceTacticalInfo,
) -> bool {
    let lower = node.to_ascii_lowercase();
    (province.victory_points > 0
        && node.contains(&province.victory_points.to_string())
        && (node.contains("胜利") || lower.contains("victory")))
        || (province.coastal && node == crate::i18n::tr("coastal_province"))
}

fn show_country_detail(
    ui: &mut egui::Ui,
    target: &CountryDetailTarget,
    diplomacy_data: Option<&DiplomacyData>,
    diplomacy_commands: &mut Vec<DiplomacyCommand>,
) {
    let detail = diplomacy_data.and_then(|data| {
        data.countries
            .iter()
            .find(|country| country.tag == target.tag)
            .and_then(|country| country.detail.as_ref())
    });
    if let Some(detail) = detail {
        diplomacy::render_country_diplomacy_detail(ui, detail, diplomacy_commands);
    } else {
        show_placeholder_detail(
            ui,
            "国家详情",
            "国家外交数据尚未载入。请从外交面板、贸易伙伴或地图关联入口重新打开。",
            &target.tag,
        );
    }
}

fn show_pop_detail(
    ui: &mut egui::Ui,
    target: &PopGroupDetailTarget,
    pop_data: Option<&PopPanelData>,
    panel_command: &mut Option<PanelCommand>,
) {
    let Some(data) = pop_data else {
        show_placeholder_detail(
            ui,
            "人口组详情",
            "人口数据尚未载入。请从人口工作台重新打开。",
            &target.pop_group_key,
        );
        return;
    };

    if let Some(state_id) = target.state_id {
        let Some(state) = data.states.iter().find(|state| state.state_id == state_id) else {
            show_placeholder_detail(
                ui,
                "人口组详情",
                "没有找到该州人口数据。",
                &target.pop_group_key,
            );
            return;
        };
        VanillaIron::section_heading(ui, &state.state_name);
        key_value(ui, "人口", format_count(state.population));
        key_value(ui, "就业", format_count(state.employed));
        key_value(
            ui,
            "失业率",
            format!("{:.1}%", state.unemployment_rate * 100.0),
        );
        key_value(ui, "平均收入", format!("{:.1} RM", state.avg_income_rm));
        key_value(
            ui,
            "满意度",
            format!("{:.0}%", state.avg_satisfaction * 100.0),
        );
        key_value(ui, "主导阶层", state.dominant_class.clone());
        if ui.link("打开州详情").clicked() {
            *panel_command = Some(PanelCommand::OpenDetail(ActiveDetailPanel::State(
                StateDetailTarget { state_id },
            )));
        }
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "相关建筑");
        for row in data
            .building_employment
            .iter()
            .filter(|row| row.state_id == state_id)
            .take(8)
        {
            if ui.link(&row.building_name).clicked() {
                *panel_command = Some(PanelCommand::OpenDetail(ActiveDetailPanel::Building(
                    BuildingDetailTarget {
                        building_key: row.building_key.clone(),
                        state_id: Some(row.state_id),
                    },
                )));
            }
        }
        return;
    }

    let Some(class) = data
        .classes
        .iter()
        .find(|class| class.class_name == target.pop_group_key)
    else {
        show_placeholder_detail(
            ui,
            "人口组详情",
            "没有找到该人口组。",
            &target.pop_group_key,
        );
        return;
    };
    VanillaIron::section_heading(ui, &class.class_name);
    key_value(ui, "人口", format_count(class.size));
    key_value(ui, "就业", format_count(class.employed));
    key_value(ui, "失业", format_count(class.unemployed));
    key_value(ui, "平均工资", format!("{:.1} RM", class.avg_wage_rm));
    key_value(ui, "平均收入", format!("{:.1} RM", class.avg_income_rm));
    key_value(
        ui,
        "可支配收入",
        format!("{:.1} RM", class.avg_disposable_income_rm),
    );
    key_value(
        ui,
        "满意度",
        format!("{:.0}%", class.avg_satisfaction * 100.0),
    );
    key_value(
        ui,
        "需求满足",
        format!("{:.0}%", class.needs_fulfillment * 100.0),
    );
    key_value(ui, "识字率", format!("{:.0}%", class.literacy * 100.0));
    key_value(ui, "熟练度", format!("{:.0}%", class.skilled_ratio * 100.0));
    key_value(ui, "激进化", format!("{:.0}%", class.radicalism * 100.0));
}

fn show_army_detail(
    ui: &mut egui::Ui,
    army_id: u32,
    military_data: Option<&MilitaryData>,
    logistics_data: Option<&LogisticsData>,
    panel_command: &mut Option<PanelCommand>,
) {
    let Some(data) = military_data else {
        show_placeholder_detail(
            ui,
            "军队详情",
            "陆军数据尚未载入。请从陆军指挥部重新打开军队详情。",
            &format!("军队 ID：{army_id}"),
        );
        return;
    };
    let Some(army) = data.armies.iter().find(|army| army.id == army_id) else {
        show_placeholder_detail(
            ui,
            "军队详情",
            "没有找到该集团军。它可能已经被解散或归属发生变化。",
            &format!("军队 ID：{army_id}"),
        );
        return;
    };

    VanillaIron::section_heading(ui, &army.name);
    key_value(ui, "编制", format!("{} 个师", army.member_count));
    key_value(
        ui,
        "将领",
        army.commander_name
            .clone()
            .unwrap_or_else(|| "未任命".to_owned()),
    );
    key_value(
        ui,
        "指挥效率",
        format!("{:.0}%", army.command_efficiency * 100.0),
    );
    key_value(
        ui,
        "状态",
        if army.executing {
            "执行计划中".to_owned()
        } else if army.active {
            "已激活".to_owned()
        } else {
            "待命".to_owned()
        },
    );
    key_value(
        ui,
        "前线",
        if army.has_path {
            "已有".to_owned()
        } else {
            "无".to_owned()
        },
    );
    key_value(
        ui,
        "进攻箭头",
        if army.has_arrow {
            "已有".to_owned()
        } else {
            "无".to_owned()
        },
    );
    ui.add_space(8.0);

    VanillaIron::section_heading(ui, "加成");
    key_value(ui, "攻击", format!("{:.0}%", army.attack_bonus_pct));
    key_value(ui, "防御", format!("{:.0}%", army.defense_bonus_pct));
    key_value(ui, "计划", format!("{:.0}%", army.planning_bonus_pct));
    key_value(
        ui,
        "组织恢复",
        format!("{:.0}%", army.org_recovery_bonus_pct),
    );
    key_value(ui, "补给减免", format!("{:.0}%", army.supply_reduction_pct));
    ui.add_space(8.0);

    VanillaIron::section_heading(ui, "下辖师团");
    let mut member_count = 0;
    for div in data
        .divisions
        .iter()
        .filter(|division| division.army_id == Some(army_id))
        .take(8)
    {
        member_count += 1;
        let org = if div.max_organisation > 0.0 {
            div.organisation / div.max_organisation
        } else {
            0.0
        };
        key_value(
            ui,
            &div.name,
            format!(
                "组织 {:.0}% / 装备 {:.0}% / {}",
                org * 100.0,
                div.equipment_ratio * 100.0,
                div.province_name
            ),
        );
    }
    if member_count == 0 {
        ui.label(
            RichText::new("暂无下辖师团数据。")
                .small()
                .color(VanillaIron::MUTED),
        );
    }
    if data
        .divisions
        .iter()
        .any(|division| division.army_id == Some(army_id) && division.equipment_ratio < 0.8)
    {
        VanillaIron::warning_row(ui, "该军队存在装备缺口。可打开物流账本定位缺什么、缺多少。");
    }
    if VanillaIron::compact_button(ui, "打开物流").clicked() {
        *panel_command = Some(PanelCommand::OpenPrimary(ActivePrimaryPanel::Logistics));
    }
    if let Some(logistics) = logistics_data {
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "主要缺口");
        for entry in logistics
            .entries
            .iter()
            .filter(|entry| entry.deficit > 0.0)
            .take(4)
        {
            key_value(ui, &entry.name, format!("{:.1}/日", entry.deficit));
        }
    }
}

fn show_fleet_detail(
    ui: &mut egui::Ui,
    fleet_id: u32,
    naval_data: Option<&NavalData>,
    panel_command: &mut Option<PanelCommand>,
) {
    let Some(data) = naval_data else {
        show_placeholder_detail(
            ui,
            "舰队详情",
            "海军数据尚未载入。",
            &format!("舰队 ID：{fleet_id}"),
        );
        return;
    };
    let Some(fleet) = data.fleets.iter().find(|fleet| fleet.id == fleet_id) else {
        show_placeholder_detail(
            ui,
            "舰队详情",
            "没有找到该舰队。",
            &format!("舰队 ID：{fleet_id}"),
        );
        return;
    };
    VanillaIron::section_heading(ui, &fleet.name);
    key_value(ui, "任务", fleet.mission.label().to_owned());
    key_value(ui, "所在海域", format!("{}", fleet.region_id));
    key_value(
        ui,
        "目标海域",
        fleet
            .target_region_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "无".to_owned()),
    );
    key_value(ui, "舰船", fleet.ship_count.to_string());
    key_value(ui, "受损舰船", fleet.damaged_ships.to_string());
    key_value(ui, "平均耐久", format!("{:.0}%", fleet.hp_ratio * 100.0));
    key_value(ui, "维修", fleet.repair_state.clone());
    key_value(ui, "航线风险", format!("{:.0}%", fleet.convoy_risk_pct));
    if fleet.damaged_ships > 0 || fleet.convoy_risk_pct >= 12.0 {
        VanillaIron::warning_row(ui, "舰队存在维修或补给风险。可打开物流账本查看相关压力。");
    }
    if VanillaIron::compact_button(ui, "打开物流").clicked() {
        *panel_command = Some(PanelCommand::OpenPrimary(ActivePrimaryPanel::Logistics));
    }
}

fn show_air_wing_detail(
    ui: &mut egui::Ui,
    wing_id: u32,
    air_data: Option<&AirData>,
    panel_command: &mut Option<PanelCommand>,
) {
    let Some(data) = air_data else {
        show_placeholder_detail(
            ui,
            "联队详情",
            "空军数据尚未载入。",
            &format!("联队 ID：{wing_id}"),
        );
        return;
    };
    let Some(wing) = data.wings.iter().find(|wing| wing.id == wing_id) else {
        show_placeholder_detail(
            ui,
            "联队详情",
            "没有找到该联队。",
            &format!("联队 ID：{wing_id}"),
        );
        return;
    };
    VanillaIron::section_heading(ui, &wing.name);
    key_value(ui, "机型", wing.aircraft_key.clone());
    key_value(ui, "任务", wing.mission.label().to_owned());
    key_value(ui, "基地州", wing.base_state.to_string());
    key_value(ui, "空域", wing.region_id.to_string());
    key_value(
        ui,
        "目标空域",
        wing.target_region
            .map(|id| id.to_string())
            .unwrap_or_else(|| "无".to_owned()),
    );
    key_value(ui, "飞机", format!("{}/{}", wing.planes, wing.max_planes));
    key_value(
        ui,
        "组织度",
        format!("{:.0}/{:.0}", wing.organisation, wing.max_organisation),
    );
    key_value(ui, "航程", format!("{:.0} km", wing.range_km));
    key_value(ui, "制空", format!("{:.0}%", wing.air_control_pct));
    key_value(
        ui,
        "任务效率",
        format!("{:.0}%", wing.mission_efficiency_pct),
    );
    key_value(
        ui,
        "补充",
        if wing.reinforce_enabled {
            "启用"
        } else {
            "暂停"
        }
        .to_owned(),
    );
    if wing.planes < wing.max_planes || wing.mission_efficiency_pct < 60.0 {
        VanillaIron::warning_row(
            ui,
            "联队存在飞机缺口或任务效率不足。可打开物流账本查看飞机库存。",
        );
    }
    if VanillaIron::compact_button(ui, "打开物流").clicked() {
        *panel_command = Some(PanelCommand::OpenPrimary(ActivePrimaryPanel::Logistics));
    }
}

fn show_technology_detail(
    ui: &mut egui::Ui,
    technology_key: &str,
    research_data: Option<&ResearchData>,
) {
    let Some(data) = research_data else {
        show_placeholder_detail(ui, "科技详情", "科研数据尚未载入。", technology_key);
        return;
    };
    let Some(tech) = data.techs.iter().find(|tech| tech.key == technology_key) else {
        show_placeholder_detail(ui, "科技详情", "没有找到该科技。", technology_key);
        return;
    };
    VanillaIron::section_heading(ui, &tech.name);
    key_value(
        ui,
        "类别",
        category_label_for_detail(&tech.category).to_owned(),
    );
    key_value(ui, "年份", tech.start_year.to_string());
    let status = if tech.completed {
        "已完成".to_owned()
    } else if tech.researching {
        format!("研究中 {:.0}%", tech.progress * 100.0)
    } else if tech.start_year > data.current_year {
        format!("超前 {} 年", tech.start_year - data.current_year)
    } else {
        "可研究".to_owned()
    };
    key_value(ui, "状态", status);
    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "前置");
    if tech.prerequisites.is_empty() {
        ui.label(
            RichText::new("无前置科技。")
                .small()
                .color(VanillaIron::MUTED),
        );
    } else {
        for prerequisite in &tech.prerequisites {
            ui.label(RichText::new(prerequisite).small().color(VanillaIron::TEXT));
        }
    }
    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "解锁内容");
    if tech.unlock_summary.is_empty() {
        ui.label(
            RichText::new("暂无解锁摘要。")
                .small()
                .color(VanillaIron::MUTED),
        );
    } else {
        for line in &tech.unlock_summary {
            ui.label(RichText::new(line).small().color(VanillaIron::TEXT));
        }
    }
}

fn show_focus_detail(
    ui: &mut egui::Ui,
    focus_id: &str,
    focus_tree: Option<&FocusTree>,
    completed_focuses: Option<&HashSet<String>>,
    current_focus: Option<&str>,
    current_focus_progress: f32,
    available_focus_ids: Option<&HashSet<String>>,
) {
    let Some(tree) = focus_tree else {
        show_placeholder_detail(ui, "国策详情", "国策树数据尚未载入。", focus_id);
        return;
    };
    let Some(focus) = tree.focuses.iter().find(|focus| focus.id == focus_id) else {
        show_placeholder_detail(ui, "国策详情", "没有找到该国策。", focus_id);
        return;
    };
    VanillaIron::section_heading(ui, focus_display_name_for_detail(focus));
    key_value(ui, "国家", tree.country.clone());
    key_value(ui, "耗时", format!("{} 天", focus.cost_days));
    key_value(
        ui,
        "状态",
        focus_status_for_detail(
            focus,
            completed_focuses,
            current_focus,
            current_focus_progress,
            available_focus_ids,
        ),
    );
    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "前置");
    if focus.prerequisites.is_empty() {
        ui.label(
            RichText::new("无前置国策。")
                .small()
                .color(VanillaIron::MUTED),
        );
    } else {
        for group in &focus.prerequisites {
            ui.label(
                RichText::new(group.join(" 或 "))
                    .small()
                    .color(VanillaIron::TEXT),
            );
        }
    }
    if !focus.mutually_exclusive.is_empty() {
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "互斥");
        ui.label(
            RichText::new(focus.mutually_exclusive.join("、"))
                .small()
                .color(VanillaIron::WARN),
        );
    }
    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "奖励");
    if focus.completion_effect.is_empty() {
        ui.label(
            RichText::new("暂无奖励效果。")
                .small()
                .color(VanillaIron::MUTED),
        );
    } else {
        for (idx, effect) in focus.completion_effect.iter().enumerate().take(8) {
            ui.label(
                RichText::new(format!("效果 {}：{:?}", idx + 1, effect))
                    .small()
                    .color(VanillaIron::TEXT),
            );
        }
    }
}

fn show_journal_entry_detail(
    ui: &mut egui::Ui,
    entry_id: &str,
    decisions_data: Option<&DecisionsData>,
    decision_commands: &mut Vec<crate::politics::DecisionCommand>,
) {
    let Some(data) = decisions_data else {
        show_placeholder_detail(ui, "日志详情", "决议/局势数据尚未载入。", entry_id);
        return;
    };
    let Some(entry) = data.decisions.iter().find(|entry| entry.id == entry_id) else {
        show_placeholder_detail(ui, "日志详情", "没有找到该决议或局势条目。", entry_id);
        return;
    };
    VanillaIron::section_heading(ui, &entry.name);
    key_value(ui, "国家", data.country_tag.clone());
    key_value(ui, "政治力量", format!("{:.0} PP", data.political_power));
    key_value(ui, "花费", format!("{:.0} PP", entry.cost_political_power));
    key_value(ui, "状态", decision_status(entry));
    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "条件");
    let reason = if !entry.visible {
        "不可见条件未满足"
    } else if entry.already_fired {
        "该决议已经执行过"
    } else if entry.cooldown_remaining.is_some() {
        "冷却中"
    } else if entry.mission_remaining.is_some() {
        "进行中"
    } else if !entry.clickable {
        "政治力量不足或可用条件未满足"
    } else {
        "可以执行"
    };
    ui.label(RichText::new(reason).small().color(if entry.clickable {
        VanillaIron::GOOD
    } else {
        VanillaIron::WARN
    }));
    ui.add_space(8.0);
    VanillaIron::section_heading(ui, "效果");
    ui.label(
        RichText::new(&entry.effect_preview)
            .small()
            .color(VanillaIron::TEXT),
    );
    if !entry.description.is_empty() {
        ui.add_space(8.0);
        VanillaIron::section_heading(ui, "日志");
        ui.label(
            RichText::new(&entry.description)
                .small()
                .color(VanillaIron::MUTED),
        );
    }
    ui.add_space(8.0);
    if VanillaIron::compact_button(ui, "执行决议").clicked() {
        if entry.clickable {
            decision_commands.push(crate::politics::DecisionCommand::Activate(entry.id.clone()));
        }
    }
}

fn state_building_section(ui: &mut egui::Ui, state: &crate::province_info::StateEconomicInfo) {
    VanillaIron::section_heading(ui, "州建筑");
    if state.buildings.is_empty() {
        ui.label(
            RichText::new("暂无建筑数据。")
                .small()
                .color(VanillaIron::MUTED),
        );
        return;
    }

    for building in &state.buildings {
        let row_h = if building.warnings.is_empty() {
            50.0
        } else {
            68.0
        };
        let row_w = (ui.available_width() - 14.0).max(260.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(row_w, row_h), Sense::hover());
        ui.painter().rect_filled(rect, 1.0, VanillaIron::CARD_DEEP);
        ui.painter().rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(1),
            egui::Stroke::new(1.0, VanillaIron::EDGE_DARK),
            egui::epaint::StrokeKind::Inside,
        );

        let inner = rect.shrink2(Vec2::new(10.0, 6.0));
        let profit_w = (inner.width() * 0.38).clamp(124.0, 156.0);
        let name_rect = egui::Rect::from_min_max(
            inner.left_top(),
            egui::pos2(inner.right() - profit_w - 8.0, inner.top() + 18.0),
        );
        let profit_rect = egui::Rect::from_min_max(
            egui::pos2(name_rect.right() + 8.0, inner.top()),
            egui::pos2(inner.right(), inner.top() + 18.0),
        );
        let name = format!("{} Lv {}", building.name, building.level);
        let name_font = crate::v9::text::fit_font_to_width(
            &name,
            crate::v9::TextRole::Body.font_id(),
            name_rect.width(),
            0.72,
        );
        let profit = signed_rm(building.profit_rm_weekly);
        let profit_font = crate::v9::text::fit_font_to_width(
            &profit,
            crate::v9::TextRole::Caption.font_id(),
            profit_rect.width(),
            0.70,
        );
        ui.painter().with_clip_rect(name_rect).text(
            name_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            name,
            name_font,
            VanillaIron::TEXT,
        );
        ui.painter().with_clip_rect(profit_rect).text(
            profit_rect.right_center(),
            egui::Align2::RIGHT_CENTER,
            profit,
            profit_font,
            if building.profit_rm_weekly < 0.0 {
                VanillaIron::BAD
            } else {
                VanillaIron::GOOD
            },
        );

        let employment = format!(
            "就业率 {:.0}%",
            building.employment_rate.clamp(0.0, 1.0) * 100.0
        );
        ui.painter().text(
            egui::pos2(inner.left(), inner.top() + 26.0),
            egui::Align2::LEFT_TOP,
            employment,
            crate::v9::TextRole::Caption.font_id(),
            VanillaIron::MUTED,
        );
        if !building.warnings.is_empty() {
            let warning_rect = egui::Rect::from_min_max(
                egui::pos2(inner.left(), inner.top() + 44.0),
                inner.right_bottom(),
            );
            let warning = building.warnings.join("；");
            let font = crate::v9::text::fit_font_to_width(
                &warning,
                crate::v9::TextRole::Small.font_id(),
                warning_rect.width(),
                0.70,
            );
            ui.painter().with_clip_rect(warning_rect).text(
                warning_rect.left_top(),
                egui::Align2::LEFT_TOP,
                warning,
                font,
                VanillaIron::WARN,
            );
        }
        ui.add_space(4.0);
    }
}

fn state_resource_section(ui: &mut egui::Ui, state: &crate::province_info::StateEconomicInfo) {
    VanillaIron::section_heading(ui, "资源与建设");
    if !state.resources_output.is_empty() {
        for (name, amount) in &state.resources_output {
            key_value(ui, name, format!("{:.1}/日", amount));
        }
    } else if !state.resources.is_empty() {
        for (name, amount) in &state.resources {
            key_value(ui, name, format!("{:.1}", amount));
        }
    } else {
        ui.label(
            RichText::new("暂无资源产出数据。")
                .small()
                .color(VanillaIron::MUTED),
        );
    }
    if !state.construction_projects.is_empty() {
        ui.add_space(6.0);
        VanillaIron::section_heading(ui, "施工项目");
        for project in &state.construction_projects {
            key_value(
                ui,
                &project.building_name,
                format!(
                    "Lv {} -> {}  {:.0}%",
                    project.current_level,
                    project.target_level,
                    project.progress * 100.0
                ),
            );
        }
    }
}

fn country_detail_link(
    ui: &mut egui::Ui,
    label: &str,
    name: &str,
    tag: &str,
    panel_command: &mut Option<PanelCommand>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).small().color(VanillaIron::MUTED));
        if tag.is_empty() {
            ui.label(name);
        } else if ui.link(format!("{name} ({tag})")).clicked() {
            *panel_command = Some(PanelCommand::OpenDetail(ActiveDetailPanel::Country(
                CountryDetailTarget {
                    tag: tag.to_owned(),
                },
            )));
        }
    });
}

fn find_building_entry<'a>(
    data: &'a ConstructionV6PanelData,
    key: &str,
) -> Option<&'a BuildingTypeV6Entry> {
    data.entries
        .iter()
        .find(|entry| entry.building_def_id == key || entry.building_name == key)
}

fn find_buildable_entry<'a>(
    data: &'a ConstructionV6PanelData,
    key: &str,
) -> Option<&'a BuildableBuildingEntry> {
    data.buildable_catalog
        .iter()
        .find(|entry| entry.building_def_id == key || entry.building_name == key)
}

fn format_count(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn signed_rm(v: f64) -> String {
    if v >= 0.0 {
        format!("+{}", format_rm(v))
    } else {
        format!("-{}", format_rm(-v))
    }
}

fn show_law_detail(
    ui: &mut egui::Ui,
    category_key: &str,
    law_id: Option<&str>,
    law_data: Option<&LawPanelData>,
    law_commands: &mut Vec<LawCommand>,
) {
    let Some(data) = law_data else {
        ui.heading("法律详情");
        ui.label("法律数据尚未载入。请从政治面板或法律面板重新打开详情。");
        return;
    };
    let Some(category) = law_panel::law_category_from_key(category_key) else {
        ui.heading("法律详情");
        ui.label("无法识别法律类别。");
        ui.label(
            RichText::new(format!("内部类别：{category_key}"))
                .small()
                .color(VanillaIron::MUTED),
        );
        return;
    };
    let Some(slot) = data.slots.iter().find(|slot| slot.category == category) else {
        ui.heading("法律详情");
        ui.label("当前国家没有该法律类别数据。");
        return;
    };

    let selected_id = ui.id().with(("law_detail_candidate", category_key));
    let mut selected_law = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<String>(selected_id))
        .or_else(|| law_id.map(str::to_owned))
        .unwrap_or_else(|| slot.current_id.clone());
    if !slot.tiers.iter().any(|tier| tier.id == selected_law) {
        selected_law = slot.current_id.clone();
    }

    law_detail_header(ui, slot, data.political_power);
    ui.add_space(8.0);
    law_current_section(ui, slot);
    ui.add_space(8.0);
    law_candidate_list(
        ui,
        slot,
        data.political_power,
        &mut selected_law,
        law_commands,
    );
    ui.add_space(8.0);
    law_candidate_detail(ui, slot, &selected_law, data.political_power);
    ui.ctx()
        .data_mut(|d| d.insert_persisted(selected_id, selected_law));
}

fn law_detail_header(ui: &mut egui::Ui, slot: &LawSlotEntry, political_power: f32) {
    VanillaIron::section_heading(ui, law_panel::law_category_label(&slot.category));
    key_value(ui, "当前法律", slot.current_name.clone());
    key_value(ui, "政治力量", format!("{political_power:.0} PP"));
    key_value(
        ui,
        "冷却",
        if slot.cooldown_days > 0 {
            format!("{} 天", slot.cooldown_days)
        } else {
            "无".to_owned()
        },
    );
    if let Some((_, target_name, remaining)) = &slot.pending {
        key_value(ui, "切换中", format!("{target_name}，剩余 {remaining} 天"));
    }
    if slot.is_locked {
        VanillaIron::warning_row(
            ui,
            slot.locked_reason
                .as_deref()
                .unwrap_or("该法律类别当前被锁定。"),
        );
    }
}

fn law_current_section(ui: &mut egui::Ui, slot: &LawSlotEntry) {
    VanillaIron::section_heading(ui, "当前效果");
    let Some(current) = slot.tiers.iter().find(|tier| tier.id == slot.current_id) else {
        ui.label("当前法律没有可显示的效果数据。");
        return;
    };
    render_law_effects(ui, current);
}

fn law_candidate_list(
    ui: &mut egui::Ui,
    slot: &LawSlotEntry,
    political_power: f32,
    selected_law: &mut String,
    law_commands: &mut Vec<LawCommand>,
) {
    VanillaIron::section_heading(ui, "候选法律");
    for tier in &slot.tiers {
        let selected = tier.id == *selected_law;
        let is_current = tier.id == slot.current_id;
        let is_pending = slot
            .pending
            .as_ref()
            .map_or(false, |(id, _, _)| id == &tier.id);
        let can_switch = law_panel::law_switch_available(slot, tier, political_power);
        let reason = law_panel::law_unavailable_reason(slot, tier, political_power);
        let height = if reason.is_some() && !is_current {
            64.0
        } else {
            50.0
        };
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
        let fill = if selected {
            VanillaIron::CARD_SOFT
        } else if response.hovered() {
            Color32::from_rgb(0x16, 0x18, 0x14)
        } else {
            VanillaIron::CARD_DEEP
        };
        ui.painter().rect_filled(rect, 1.0, fill);
        ui.painter().rect_stroke(
            rect,
            egui::epaint::CornerRadius::same(1),
            egui::Stroke::new(
                1.0,
                if selected {
                    VanillaIron::BRASS_BRIGHT
                } else {
                    VanillaIron::EDGE_DARK
                },
            ),
            egui::epaint::StrokeKind::Inside,
        );
        if response.clicked() {
            *selected_law = tier.id.clone();
        }
        response.on_hover_text("点击查看候选详情");
        let status = if is_current {
            "当前"
        } else if is_pending {
            "切换中"
        } else if can_switch {
            "可执行"
        } else {
            "不可执行"
        };
        let status_color = if is_current {
            VanillaIron::GOOD
        } else if is_pending {
            VanillaIron::WARN
        } else if can_switch {
            VanillaIron::BRASS_BRIGHT
        } else {
            VanillaIron::BAD
        };
        ui.painter().text(
            rect.left_top() + Vec2::new(8.0, 6.0),
            egui::Align2::LEFT_TOP,
            tier.name.as_str(),
            crate::v9::TextRole::Body.font_id(),
            VanillaIron::TEXT,
        );
        ui.painter().text(
            rect.left_top() + Vec2::new(8.0, 26.0),
            egui::Align2::LEFT_TOP,
            format!("花费 {} PP，冷却 {} 天", tier.pp_cost, tier.cooldown_days),
            crate::v9::TextRole::Caption.font_id(),
            VanillaIron::MUTED,
        );
        ui.painter().text(
            rect.right_top() + Vec2::new(-8.0, 8.0),
            egui::Align2::RIGHT_TOP,
            status,
            crate::v9::TextRole::Caption.font_id(),
            status_color,
        );
        if let Some(reason) = reason.as_deref().filter(|_| !is_current) {
            ui.painter().text(
                rect.left_bottom() + Vec2::new(8.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                reason,
                crate::v9::TextRole::Small.font_id(),
                VanillaIron::BAD,
            );
        }
        let button_rect = egui::Rect::from_min_size(
            rect.right_bottom() - Vec2::new(86.0, 28.0),
            Vec2::new(78.0, 22.0),
        );
        if can_switch {
            let button = ui.put(
                button_rect,
                egui::Button::new(RichText::new("执行").color(VanillaIron::TEXT))
                    .fill(VanillaIron::CARD_SOFT)
                    .stroke(egui::Stroke::new(1.0, VanillaIron::EDGE)),
            );
            if button.clicked() {
                law_commands.push(LawCommand::SwitchLaw {
                    category: slot.category,
                    target_law_id: tier.id.clone(),
                });
            }
        }
    }
}

fn law_candidate_detail(
    ui: &mut egui::Ui,
    slot: &LawSlotEntry,
    selected_law: &str,
    political_power: f32,
) {
    let Some(tier) = slot.tiers.iter().find(|tier| tier.id == selected_law) else {
        return;
    };
    VanillaIron::section_heading(ui, "候选详情");
    key_value(ui, "法律", tier.name.clone());
    key_value(ui, "执行成本", format!("{} PP", tier.pp_cost));
    key_value(ui, "切换冷却", format!("{} 天", tier.cooldown_days));
    if let Some(reason) = law_panel::law_unavailable_reason(slot, tier, political_power) {
        key_value(ui, "可用性", reason);
    } else {
        key_value(ui, "可用性", "可以执行".to_owned());
    }
    render_law_effects(ui, tier);
}

fn render_law_effects(ui: &mut egui::Ui, tier: &LawTierEntry) {
    if tier.effects.is_empty() {
        ui.label(
            RichText::new("暂无效果数据。")
                .small()
                .color(VanillaIron::MUTED),
        );
        return;
    }
    for effect in &tier.effects {
        ui.label(RichText::new(effect).small().color(VanillaIron::TEXT));
    }
}

fn key_value(ui: &mut egui::Ui, key: &str, value: String) {
    let width = ui.available_width().max(120.0);
    let label_width = (width * 0.34).clamp(72.0, 132.0);
    let value_width = (width - label_width - 10.0).max(64.0);
    ui.horizontal_top(|ui| {
        ui.add_sized(
            Vec2::new(label_width, 18.0),
            egui::Label::new(RichText::new(key).small().color(VanillaIron::MUTED)),
        );
        ui.add_sized(
            Vec2::new(value_width, 18.0),
            egui::Label::new(RichText::new(value).color(VanillaIron::TEXT)).wrap(),
        );
    });
}

fn format_rm(v: f64) -> String {
    if v.abs() >= 1_000_000_000.0 {
        format!("{:.1}B RM", v / 1_000_000_000.0)
    } else if v.abs() >= 1_000_000.0 {
        format!("{:.1}M RM", v / 1_000_000.0)
    } else if v.abs() >= 1_000.0 {
        format!("{:.1}K RM", v / 1_000.0)
    } else {
        format!("{:.0} RM", v)
    }
}

fn category_label_for_detail(category: &str) -> &'static str {
    match category {
        "industry" => "工业",
        "chemistry" => "化工",
        "electrical" => "电气",
        "metallurgy" => "冶金",
        "military_doctrine" => "军事学说",
        "air" => "航空",
        "naval" => "海军",
        "social_science" => "社会科学",
        "information_control" => "信息管制",
        _ => "科技",
    }
}

fn focus_display_name_for_detail(focus: &Focus) -> &str {
    if focus.name.is_empty() {
        &focus.id
    } else {
        &focus.name
    }
}

fn focus_status_for_detail(
    focus: &Focus,
    completed_focuses: Option<&HashSet<String>>,
    current_focus: Option<&str>,
    current_focus_progress: f32,
    available_focus_ids: Option<&HashSet<String>>,
) -> String {
    if completed_focuses.is_some_and(|completed| completed.contains(&focus.id)) {
        return "已完成".to_owned();
    }
    if current_focus == Some(focus.id.as_str()) {
        let progress = if focus.cost_days > 0 {
            current_focus_progress / focus.cost_days as f32
        } else {
            0.0
        };
        return format!("进行中 {:.0}%", progress.clamp(0.0, 1.0) * 100.0);
    }
    if available_focus_ids.is_some_and(|available| available.contains(&focus.id)) {
        "可执行".to_owned()
    } else {
        "不可执行：前置、互斥或可用条件未满足".to_owned()
    }
}

fn decision_status(entry: &crate::politics::DecisionEntry) -> String {
    if entry.already_fired {
        "已执行".to_owned()
    } else if let Some(remaining) = entry.mission_remaining {
        format!("进行中，剩余 {} 天", remaining)
    } else if let Some(remaining) = entry.cooldown_remaining {
        format!("冷却中，剩余 {} 天", remaining)
    } else if entry.clickable {
        "可执行".to_owned()
    } else {
        "不可执行".to_owned()
    }
}

fn good_category_label(category: GoodCategory) -> &'static str {
    match category {
        GoodCategory::RawMaterial => "原材料",
        GoodCategory::Intermediate => "中间品",
        GoodCategory::Consumer => "消费品",
        GoodCategory::Luxury => "奢侈品",
        GoodCategory::Service => "服务",
        GoodCategory::MilitaryIntermediate => "军工中间品",
    }
}

fn good_category_color(category: GoodCategory) -> Color32 {
    match category {
        GoodCategory::RawMaterial => Color32::from_rgb(0x80, 0xa0, 0x50),
        GoodCategory::Intermediate => Color32::from_rgb(0x60, 0x90, 0xb0),
        GoodCategory::Consumer => Color32::from_rgb(0xc0, 0xa0, 0x30),
        GoodCategory::Luxury => Color32::from_rgb(0xb0, 0x70, 0xc0),
        GoodCategory::Service => Color32::from_rgb(0x50, 0xb0, 0x90),
        GoodCategory::MilitaryIntermediate => Color32::from_rgb(0xa0, 0x50, 0x50),
    }
}

fn detail_source_label(source: DetailSource) -> &'static str {
    match source {
        DetailSource::Market => "市场",
        DetailSource::Finance => "财政",
        DetailSource::Construction => "建设",
        DetailSource::Trade => "贸易",
        DetailSource::Logistics => "物流",
        DetailSource::Map => "地图",
        DetailSource::Politics => "政治",
        DetailSource::Other => "其他",
    }
}
