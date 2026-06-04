// Construction panel DTO builder.
use std::collections::{BTreeMap, HashMap};

use hoi4_state::World;

use super::names::{DisplayNameKind, DisplayNameResolver};

pub fn build_construction_v6_panel_data(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    econ: &hoi4_logic::economy::EconomyState,
    player: usize,
    construction_mode: &Option<String>,
    auto_build_enabled: bool,
    auto_build_explanations: &[hoi4_logic::economy::construction_planner::ConstructionCandidateScore],
) -> Option<hoi4_ui::construction_v6_panel::ConstructionV6PanelData> {
    let name_resolver = DisplayNameResolver::new(None);
    let country_id = hoi4_state::CountryId(player as u16);
    let state_ids: Vec<hoi4_state::StateId> = (0..world.states.count)
        .filter(|&si| world.states.owners[si] == country_id)
        .map(|si| hoi4_state::StateId(si as u16))
        .collect();
    let queue: Vec<hoi4_ui::construction_v6_panel::ConstructionQueueV6Entry> = econ
        .construction
        .get(player)
        .map(|q| {
            q.items
                .iter()
                .map(|item| {
                    let building_name = building_name(&name_resolver, v6_db, &item.building_key);
                    let building_def = v6_db
                        .buildings
                        .iter()
                        .find(|building| building.id == item.building_key);
                    let state_name =
                        state_name(&name_resolver, world, item.target_state.0 as usize);
                    let current_level =
                        current_building_level(world, &item.building_key, item.target_state);
                    let target_level = if item.target_level == 0 {
                        current_level.saturating_add(1)
                    } else {
                        item.target_level
                    };
                    hoi4_ui::construction_v6_panel::ConstructionQueueV6Entry {
                        building_name,
                        state_name,
                        current_level,
                        target_level,
                        recipe_cp_cost: building_def
                            .map(|def| def.construction_recipe.cp_cost)
                            .unwrap_or(item.cost),
                        recipe_funds_rm: building_def
                            .map(|def| def.construction_recipe.funds_rm)
                            .unwrap_or(item.budget_needed_rm),
                        recipe_materials_summary: building_def
                            .map(|def| {
                                construction_materials_summary(
                                    &name_resolver,
                                    v6_db,
                                    &def.construction_recipe.materials,
                                )
                            })
                            .unwrap_or_else(|| queued_materials_summary(v6_db, item)),
                        recipe_labor: building_def
                            .map(|def| def.construction_recipe.labor)
                            .unwrap_or(0),
                        recipe_engineering: building_def
                            .map(|def| def.construction_recipe.engineering)
                            .unwrap_or(0),
                        recipe_region_summary: building_def
                            .map(|def| recipe_region_summary(def))
                            .unwrap_or_default(),
                        progress: item.completion(),
                        funding_source_label: construction_funding_source_label(
                            item.funding_source,
                        )
                        .to_owned(),
                        owner_on_completion_label: building_owner_label(item.owner_on_completion)
                            .to_owned(),
                        paid_funds_rm: item.paid_funds_rm,
                        budget_needed_rm: item.budget_needed_rm,
                        material_fulfillment: item.material_fulfillment(),
                        fund_ratio: item.runtime.fund_ratio,
                        priority: item.priority,
                        weight: item.weight,
                        paused: item.paused,
                        allocated_cp: item.runtime.allocated_cp,
                        effective_cp: item.runtime.effective_cp,
                        blocked_cp: item.runtime.blocked_cp,
                        bottleneck_label: item.runtime.bottleneck.clone(),
                        estimated_days: item.runtime.estimated_days,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let mut buildable_catalog: Vec<hoi4_ui::construction_v6_panel::BuildableBuildingEntry> = v6_db
        .buildings
        .iter()
        .filter(|bd| bd.buildable)
        .map(|bd| {
            let locked_reason = building_lock_reason(&name_resolver, world, v6_db, player, bd);
            let state_limit_reason = building_state_limit_reason(world, bd);
            hoi4_ui::construction_v6_panel::BuildableBuildingEntry {
                building_def_id: bd.id.clone(),
                building_name: name_resolver.content_name(
                    DisplayNameKind::Building,
                    &bd.id,
                    &bd.name,
                ),
                group_name: if bd.group.is_empty() {
                    building_group(bd.kind).to_string()
                } else {
                    bd.group.clone()
                },
                sector: construction_sector(bd.economic_sector),
                facility_class: construction_facility_class(bd.kind),
                recipe_cp_cost: bd.construction_recipe.cp_cost,
                recipe_funds_rm: bd.construction_recipe.funds_rm,
                recipe_materials_summary: construction_materials_summary(
                    &name_resolver,
                    v6_db,
                    &bd.construction_recipe.materials,
                ),
                recipe_labor: bd.construction_recipe.labor,
                recipe_engineering: bd.construction_recipe.engineering,
                recipe_region_summary: recipe_region_summary(bd),
                locked_reason,
                state_limit_reason,
            }
        })
        .collect();
    buildable_catalog.sort_by(|a, b| {
        a.group_name
            .cmp(&b.group_name)
            .then(a.building_name.cmp(&b.building_name))
    });
    #[derive(Default)]
    struct BuildingAggregate {
        total_level: u32,
        weighted_employment: f32,
        profit_rm_weekly: f64,
        outputs: HashMap<String, f32>,
        inputs: HashMap<String, f32>,
        input_shortages: BTreeMap<String, f32>,
        labor_gap: u32,
        pm_counts: BTreeMap<String, u32>,
        states: Vec<hoi4_ui::construction_v6_panel::BuildingStateV6Entry>,
    }

    let market = world.countries.market.markets.get(player);
    let mut aggregates: BTreeMap<String, BuildingAggregate> = BTreeMap::new();
    for (building_idx, b) in world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, b)| state_ids.contains(&b.state) && b.level > 0)
    {
        let pms = hoi4_content::active_pms_for_building(b, v6_db);
        let employment_gap = employment_gap_for_building(v6_db, b);
        let labor_gap: u32 = employment_gap.iter().sum();
        let blocking_law = if let Some((cat, law_id)) = &b.requires_law {
            let current = &world.countries.law_store.law_sets[player].0[cat.index()].current;
            if current != law_id {
                Some(format!("{:?}: {}", cat, law_id))
            } else {
                None
            }
        } else {
            None
        };
        let is_law_blocked = blocking_law.is_some();
        let state_name = state_name(&name_resolver, world, b.state.0 as usize);
        let pm_groups = pm_groups_for_building(&name_resolver, world, v6_db, player, b);
        let pm_candidates: Vec<(String, String)> = pm_groups
            .iter()
            .flat_map(|group| {
                group
                    .candidates
                    .iter()
                    .filter(|pm| pm.locked_reason.is_none())
                    .map(|pm| (pm.pm_id.clone(), pm.pm_name.clone()))
            })
            .collect();
        let display_profit_rm_daily = b.profit_rm;

        let aggregate = aggregates.entry(b.building_def_id.clone()).or_default();
        aggregate.total_level += b.level as u32;
        aggregate.weighted_employment += b.production_rate * b.level as f32;
        aggregate.profit_rm_weekly += display_profit_rm_daily * 7.0;
        aggregate.labor_gap += labor_gap;
        *aggregate
            .pm_counts
            .entry(
                pms.iter()
                    .map(|pm| {
                        format!(
                            "{}: {}",
                            hoi4_content::production_method_group_name(pm),
                            name_resolver.content_name(
                                DisplayNameKind::ProductionMethod,
                                &pm.id,
                                &pm.name,
                            )
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .or_default() += b.level as u32;

        for pm in &pms {
            for (i, good_id) in pm.output_good_ids.iter().enumerate() {
                let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                    * pm.throughput_modifier.max(0.0)
                    * b.level as f32
                    * b.production_rate;
                *aggregate.outputs.entry(good_id.clone()).or_default() += amount;
            }
            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                    * b.level as f32
                    * b.production_rate;
                *aggregate.inputs.entry(good_id.clone()).or_default() += amount;
                if let Some(market) = market {
                    let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
                    let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
                    if demand > supply && amount > 0.0 {
                        let shortage_pct =
                            ((demand - supply) / demand.max(1.0) * 100.0).clamp(1.0, 100.0);
                        aggregate
                            .input_shortages
                            .entry(good_id.clone())
                            .or_insert(shortage_pct);
                    }
                }
            }
        }

        aggregate
            .states
            .push(hoi4_ui::construction_v6_panel::BuildingStateV6Entry {
                building_idx,
                state_name,
                level: b.level,
                employment_rate: b.production_rate,
                profit_rm_weekly: display_profit_rm_daily * 7.0,
                employment_gap,
                is_law_blocked,
                blocking_law,
                active_pm: b.active_pm.clone(),
                pm_candidates,
                pm_groups,
            });
    }

    let entries: Vec<hoi4_ui::construction_v6_panel::BuildingTypeV6Entry> = aggregates
        .into_iter()
        .map(|(building_def_id, mut aggregate)| {
            aggregate
                .states
                .sort_by(|a, b| a.state_name.cmp(&b.state_name));
            let building_name = building_name(&name_resolver, v6_db, &building_def_id);
            let building_def = v6_db
                .buildings
                .iter()
                .find(|building| building.id == building_def_id);
            let mut warnings: Vec<String> = aggregate
                .input_shortages
                .iter()
                .map(|(good_id, pct)| {
                    format!(
                        "{}: {} {:.0}%",
                        hoi4_ui::i18n::tr("input_shortage"),
                        good_name(&name_resolver, v6_db, good_id),
                        pct
                    )
                })
                .collect();
            if aggregate.labor_gap > 0 {
                warnings.push(format!(
                    "{}: {}",
                    hoi4_ui::i18n::tr("labor_shortage"),
                    aggregate.labor_gap
                ));
            }
            let pm_summary = aggregate
                .pm_counts
                .iter()
                .map(|(pm, level)| format!("{} Lv {}", pm, level))
                .collect::<Vec<_>>()
                .join(", ");
            hoi4_ui::construction_v6_panel::BuildingTypeV6Entry {
                building_def_id,
                building_name,
                sector: building_def
                    .map(|def| construction_sector(def.economic_sector))
                    .unwrap_or(hoi4_ui::construction_v6_panel::ConstructionV9Sector::Secondary),
                facility_class: building_def
                    .map(|def| construction_facility_class(def.kind))
                    .unwrap_or(
                        hoi4_ui::construction_v6_panel::ConstructionV9FacilityClass::Standard,
                    ),
                total_level: aggregate.total_level,
                employment_rate: if aggregate.total_level > 0 {
                    aggregate.weighted_employment / aggregate.total_level as f32
                } else {
                    0.0
                },
                profit_rm_weekly: aggregate.profit_rm_weekly,
                output_summary: flow_summary(&name_resolver, v6_db, &aggregate.outputs, "+"),
                input_summary: flow_summary(&name_resolver, v6_db, &aggregate.inputs, "-"),
                warnings,
                pm_summary,
                states: aggregate.states,
            }
        })
        .collect();
    let construction_capacity = econ
        .construction
        .get(player)
        .map(|queue| queue.capacity.clone())
        .unwrap_or_default();
    let total_cp = if construction_capacity.total_cp > 0.0 {
        construction_capacity.total_cp
    } else {
        hoi4_logic::economy::construction_tick::construction_cp_pool(world, player) as f32
    };
    let treasury = &world.countries.treasury.treasuries[player];
    let (working_age_pop, unemployed_pop) = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pop| state_ids.contains(&pop.state) && pop.class != hoi4_state::PopClass::Soldier)
        .fold((0u64, 0u64), |(total, unemployed), pop| {
            let size = pop.size as u64;
            (
                total + size,
                unemployed + if pop.employed_at.is_none() { size } else { 0 },
            )
        });
    let unemployment_rate = if working_age_pop > 0 {
        unemployed_pop as f32 / working_age_pop as f32
    } else {
        0.0
    };
    let mefo_risk = if treasury.gdp_rm > 0.0 {
        (treasury.mefo_debt_rm / treasury.gdp_rm) as f32
    } else {
        0.0
    };
    let investment_account = |kind: hoi4_state::InvestmentAccountKind| {
        world
            .countries
            .investment_accounts
            .iter()
            .find(|account| account.country == country_id && account.account_kind == kind)
    };
    let private_rm = investment_account(hoi4_state::InvestmentAccountKind::Private)
        .map(|account| account.balance_rm)
        .unwrap_or_else(|| {
            world
                .countries
                .private_investment_pool_rm
                .get(player)
                .copied()
                .unwrap_or(0.0)
        });
    let cartel_rm = investment_account(hoi4_state::InvestmentAccountKind::Cartel)
        .map(|account| account.balance_rm)
        .unwrap_or(0.0);
    let state_development_bank_rm =
        investment_account(hoi4_state::InvestmentAccountKind::StateDevelopmentBank)
            .map(|account| account.balance_rm)
            .unwrap_or(0.0);
    let colonial_extraction_rm =
        investment_account(hoi4_state::InvestmentAccountKind::ColonialExtraction)
            .map(|account| account.balance_rm)
            .unwrap_or(0.0);
    let foreign_capital_rm = investment_account(hoi4_state::InvestmentAccountKind::ForeignCapital)
        .map(|account| account.balance_rm)
        .unwrap_or(0.0);
    let investment_income_rm: f64 = world
        .countries
        .investment_accounts
        .iter()
        .filter(|account| account.country == country_id)
        .map(|account| account.last_income_rm)
        .sum();
    let investment_spent_rm: f64 = world
        .countries
        .investment_accounts
        .iter()
        .filter(|account| account.country == country_id)
        .map(|account| account.last_spent_rm)
        .sum();
    Some(hoi4_ui::construction_v6_panel::ConstructionV6PanelData {
        entries,
        queue,
        buildable_catalog,
        active_construction_key: construction_mode.clone(),
        available_cp: construction_capacity.allocated_cp,
        total_cp,
        allocated_cp: construction_capacity.allocated_cp,
        idle_cp: construction_capacity.idle_cp,
        blocked_cp: construction_capacity.blocked_cp,
        gdp_gbp: treasury.gdp_gbp,
        gdp_growth_yoy: treasury.gdp_growth_yoy,
        construction_spend_rm: treasury.daily_budget.expense_construction_goods_rm
            + treasury.daily_budget.expense_construction_wages_rm,
        unemployment_rate,
        military_orders_rm: treasury.daily_budget.expense_military_procurement_rm,
        mefo_risk,
        auto_build_enabled,
        auto_build_explanations: auto_build_explanations
            .iter()
            .map(|candidate| {
                let building_name = building_name(&name_resolver, v6_db, &candidate.building_id);
                let state_name = state_name(&name_resolver, world, candidate.state.0 as usize);
                hoi4_ui::construction_v6_panel::AutoBuildExplanationEntry {
                    building_name,
                    state_name,
                    score: candidate.score,
                    reasons: candidate.reasons.clone(),
                }
            })
            .collect(),
        investment_pool: hoi4_ui::construction_v6_panel::InvestmentPoolV6Data {
            total_rm: private_rm
                + cartel_rm
                + state_development_bank_rm
                + colonial_extraction_rm
                + foreign_capital_rm,
            private_rm,
            cartel_rm,
            state_development_bank_rm,
            colonial_extraction_rm,
            foreign_capital_rm,
            income_rm: investment_income_rm,
            spent_rm: investment_spent_rm,
        },
    })
}

fn building_group(kind: hoi4_content::v6_loader::BuildingKindDef) -> &'static str {
    match kind {
        hoi4_content::v6_loader::BuildingKindDef::Resource
        | hoi4_content::v6_loader::BuildingKindDef::Agriculture => "Resource and agriculture",
        hoi4_content::v6_loader::BuildingKindDef::Industrial => "Industrial",
        hoi4_content::v6_loader::BuildingKindDef::ConsumerGoods => "Consumer goods",
        hoi4_content::v6_loader::BuildingKindDef::Infrastructure => "Infrastructure",
        hoi4_content::v6_loader::BuildingKindDef::Military => "Military",
        hoi4_content::v6_loader::BuildingKindDef::MilitaryBase => "Military base",
        hoi4_content::v6_loader::BuildingKindDef::Service => "Service",
    }
}

fn construction_sector(
    sector: hoi4_content::v6_loader::EconomicSectorDef,
) -> hoi4_ui::construction_v6_panel::ConstructionV9Sector {
    match sector {
        hoi4_content::v6_loader::EconomicSectorDef::Primary => {
            hoi4_ui::construction_v6_panel::ConstructionV9Sector::Primary
        }
        hoi4_content::v6_loader::EconomicSectorDef::Secondary => {
            hoi4_ui::construction_v6_panel::ConstructionV9Sector::Secondary
        }
        hoi4_content::v6_loader::EconomicSectorDef::Tertiary => {
            hoi4_ui::construction_v6_panel::ConstructionV9Sector::Tertiary
        }
    }
}

fn construction_facility_class(
    kind: hoi4_content::v6_loader::BuildingKindDef,
) -> hoi4_ui::construction_v6_panel::ConstructionV9FacilityClass {
    match kind {
        hoi4_content::v6_loader::BuildingKindDef::Infrastructure => {
            hoi4_ui::construction_v6_panel::ConstructionV9FacilityClass::Infrastructure
        }
        hoi4_content::v6_loader::BuildingKindDef::Military
        | hoi4_content::v6_loader::BuildingKindDef::MilitaryBase => {
            hoi4_ui::construction_v6_panel::ConstructionV9FacilityClass::Military
        }
        _ => hoi4_ui::construction_v6_panel::ConstructionV9FacilityClass::Standard,
    }
}

fn construction_funding_source_label(
    source: hoi4_logic::economy::ConstructionFundingSource,
) -> &'static str {
    match source {
        hoi4_logic::economy::ConstructionFundingSource::Government => "Government",
        hoi4_logic::economy::ConstructionFundingSource::Mefo => "MEFO bills",
        hoi4_logic::economy::ConstructionFundingSource::PrivatePool => "Private pool",
        hoi4_logic::economy::ConstructionFundingSource::CartelPool => "Cartel pool",
        hoi4_logic::economy::ConstructionFundingSource::OverlordInvestment { .. } => {
            "Overlord investment"
        }
        hoi4_logic::economy::ConstructionFundingSource::ForeignInvestment { .. } => {
            "Foreign investment"
        }
    }
}

fn building_owner_label(owner: hoi4_state::BuildingOwner) -> &'static str {
    match owner {
        hoi4_state::BuildingOwner::State => "State",
        hoi4_state::BuildingOwner::Private => "Private",
        hoi4_state::BuildingOwner::Cartel => "Cartel",
    }
}

fn construction_materials_summary(
    name_resolver: &DisplayNameResolver<'_>,
    db: &hoi4_content::V6Database,
    materials: &[hoi4_content::v6_loader::ConstructionMaterialDef],
) -> String {
    materials
        .iter()
        .filter(|material| material.amount > 0.0)
        .map(|material| {
            format!(
                "{} {:.0}",
                good_name(name_resolver, db, &material.good_id),
                material.amount
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn queued_materials_summary(
    db: &hoi4_content::V6Database,
    item: &hoi4_logic::economy::ConstructionItem,
) -> String {
    item.material_needs
        .iter()
        .filter(|need| need.total_needed > 0.0)
        .map(|need| {
            let good_name = db
                .goods
                .iter()
                .find(|good| good.id == need.good_id)
                .map(|good| good.name.as_str())
                .unwrap_or(need.good_id.as_str());
            format!("{} {:.0}", good_name, need.total_needed)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn recipe_region_summary(building_def: &hoi4_content::v6_loader::BuildingDef) -> String {
    let mut restrictions = building_def
        .construction_recipe
        .regional_restrictions
        .clone();
    if let Some(limit) = &building_def.state_limit_kind {
        if !restrictions.iter().any(|restriction| restriction == limit) {
            restrictions.push(limit.clone());
        }
    }
    restrictions.join(", ")
}

fn required_law_label(
    name_resolver: &DisplayNameResolver<'_>,
    db: &hoi4_content::V6Database,
    requirement: Option<&(hoi4_content::v6_loader::LawCategoryDef, String)>,
) -> Option<String> {
    let (category, law_id) = requirement?;
    let law_name = match category {
        hoi4_content::v6_loader::LawCategoryDef::Conscription => db
            .conscription_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| name_resolver.content_name(DisplayNameKind::Law, &law.id, &law.name)),
        hoi4_content::v6_loader::LawCategoryDef::Economy => db
            .economy_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| name_resolver.content_name(DisplayNameKind::Law, &law.id, &law.name)),
        hoi4_content::v6_loader::LawCategoryDef::Trade => db
            .trade_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| name_resolver.content_name(DisplayNameKind::Law, &law.id, &law.name)),
        hoi4_content::v6_loader::LawCategoryDef::Taxation => db
            .taxation_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| name_resolver.content_name(DisplayNameKind::Law, &law.id, &law.name)),
        hoi4_content::v6_loader::LawCategoryDef::CivilRights => db
            .civil_rights_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| name_resolver.content_name(DisplayNameKind::Law, &law.id, &law.name)),
        hoi4_content::v6_loader::LawCategoryDef::InformationControl => db
            .information_control_laws
            .iter()
            .find(|law| law.id == *law_id)
            .map(|law| name_resolver.content_name(DisplayNameKind::Law, &law.id, &law.name)),
    };
    Some(format!(
        "Requires law: {}",
        law_name.unwrap_or_else(|| {
            name_resolver.content_name(DisplayNameKind::Law, law_id.as_str(), law_id.as_str())
        })
    ))
}

fn building_lock_reason(
    name_resolver: &DisplayNameResolver<'_>,
    world: &World,
    db: &hoi4_content::V6Database,
    player: usize,
    building_def: &hoi4_content::v6_loader::BuildingDef,
) -> Option<String> {
    if !building_def.buildable {
        return Some("Not buildable".to_owned());
    }

    if let Some(reason) = required_law_label(name_resolver, db, building_def.requires_law.as_ref())
    {
        let current = building_def.requires_law.as_ref().map(|(cat, _)| {
            &world.countries.law_store.law_sets[player].0[law_category_index(*cat)].current
        });
        if let Some(current) = current {
            if let Some((_, required_law)) = &building_def.requires_law {
                if current != required_law {
                    return Some(reason);
                }
            }
        }
    }

    if let Some(tech) = db.technologies.iter().find(|tech| {
        tech.unlocks.iter().any(|unlock| matches!(unlock, hoi4_content::v6_loader::TechUnlockDef::Building(id) if id == &building_def.id))
    }) {
        let unlocked = world.countries.completed_techs[player].contains(&tech.id)
            || world.countries.unlocked_buildings[player].contains(&building_def.id);
        if !unlocked {
            return Some(format!(
                "Requires technology: {}",
                name_resolver.content_name(DisplayNameKind::Technology, &tech.id, &tech.name)
            ));
        }
    }

    None
}

fn building_state_limit_reason(
    world: &World,
    building_def: &hoi4_content::v6_loader::BuildingDef,
) -> Option<String> {
    match building_def.state_limit_kind.as_deref() {
        Some("coastal") => Some("State limit: coastal states only".to_owned()),
        Some("urban") => Some("State limit: urban states only".to_owned()),
        Some("resource") => Some("State limit: resource states only".to_owned()),
        Some(kind) => Some(format!("State limit: {}", kind)),
        None => {
            let _ = world;
            None
        }
    }
}

fn law_category_index(category: hoi4_content::v6_loader::LawCategoryDef) -> usize {
    match category {
        hoi4_content::v6_loader::LawCategoryDef::Conscription => {
            hoi4_state::LawCategory::Conscription.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::Economy => {
            hoi4_state::LawCategory::Economy.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::Trade => hoi4_state::LawCategory::Trade.index(),
        hoi4_content::v6_loader::LawCategoryDef::Taxation => {
            hoi4_state::LawCategory::Taxation.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::CivilRights => {
            hoi4_state::LawCategory::CivilRights.index()
        }
        hoi4_content::v6_loader::LawCategoryDef::InformationControl => {
            hoi4_state::LawCategory::InformationControl.index()
        }
    }
}

fn current_building_level(world: &World, building_key: &str, state: hoi4_state::StateId) -> u8 {
    world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .find(|building| building.state == state && building.building_def_id == building_key)
        .map(|building| building.level)
        .unwrap_or(0)
}

fn good_name(
    name_resolver: &DisplayNameResolver<'_>,
    db: &hoi4_content::V6Database,
    good_id: &str,
) -> String {
    db.goods
        .iter()
        .find(|good| good.id == good_id)
        .map(|good| name_resolver.content_name(DisplayNameKind::Good, &good.id, &good.name))
        .unwrap_or_else(|| name_resolver.content_name(DisplayNameKind::Good, good_id, good_id))
}

fn active_pm_id_for_group(
    db: &hoi4_content::V6Database,
    building: &hoi4_state::Building,
    group: &str,
) -> String {
    building
        .active_pm_by_group
        .iter()
        .find(|active| active.group == group)
        .map(|active| active.pm_id.clone())
        .or_else(|| {
            hoi4_content::active_pms_for_building(building, db)
                .into_iter()
                .find(|pm| hoi4_content::production_method_group(pm) == group)
                .map(|pm| pm.id.clone())
        })
        .unwrap_or_else(|| building.active_pm.clone())
}

fn pm_lock_reason(
    name_resolver: &DisplayNameResolver<'_>,
    world: &World,
    db: &hoi4_content::V6Database,
    player: usize,
    pm: &hoi4_content::ProductionMethodDef,
) -> Option<String> {
    if let Some(tech_id) = &pm.unlocked_by {
        if !world.countries.completed_techs[player].contains(tech_id) {
            let tech_name = db
                .technologies
                .iter()
                .find(|tech| tech.id == *tech_id)
                .map(|tech| {
                    name_resolver.content_name(DisplayNameKind::Technology, &tech.id, &tech.name)
                })
                .unwrap_or_else(|| {
                    name_resolver.content_name(
                        DisplayNameKind::Technology,
                        tech_id.as_str(),
                        tech_id.as_str(),
                    )
                });
            return Some(format!("Requires technology: {}", tech_name));
        }
    }
    if let Some(reason) = required_law_label(name_resolver, db, pm.required_law.as_ref()) {
        if let Some((cat, law_id)) = &pm.required_law {
            let current =
                &world.countries.law_store.law_sets[player].0[law_category_index(*cat)].current;
            if current != law_id {
                return Some(reason);
            }
        }
    }
    None
}

fn pm_prediction(
    name_resolver: &DisplayNameResolver<'_>,
    db: &hoi4_content::V6Database,
    pm: &hoi4_content::ProductionMethodDef,
    level: u8,
) -> String {
    let mut parts = Vec::new();
    let output = pm
        .output_good_ids
        .iter()
        .enumerate()
        .take(2)
        .map(|(i, good_id)| {
            let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                * pm.throughput_modifier.max(0.0)
                * level as f32;
            format!("{} +{:.0}/d", good_name(name_resolver, db, good_id), amount)
        })
        .collect::<Vec<_>>()
        .join(", ");
    if !output.is_empty() {
        parts.push(format!("??? {}", output));
    }
    let input = pm
        .input_good_ids
        .iter()
        .enumerate()
        .take(2)
        .map(|(i, good_id)| {
            let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0) * level as f32;
            format!("{} -{:.0}/d", good_name(name_resolver, db, good_id), amount)
        })
        .collect::<Vec<_>>()
        .join(", ");
    if !input.is_empty() {
        parts.push(format!("??? {}", input));
    }
    let workers: u32 = pm.employment_demand.iter().sum::<u32>() * level as u32;
    if workers > 0 {
        parts.push(format!("??? {}", workers));
    }
    parts.join(", ")
}

fn pm_groups_for_building(
    name_resolver: &DisplayNameResolver<'_>,
    world: &World,
    db: &hoi4_content::V6Database,
    player: usize,
    building: &hoi4_state::Building,
) -> Vec<hoi4_ui::construction_v6_panel::ProductionMethodGroupV6Entry> {
    let mut grouped: BTreeMap<String, Vec<&hoi4_content::ProductionMethodDef>> = BTreeMap::new();
    for pm in db
        .production_methods
        .iter()
        .filter(|pm| pm.building_id == building.building_def_id)
    {
        grouped
            .entry(hoi4_content::production_method_group(pm).to_owned())
            .or_default()
            .push(pm);
    }

    grouped
        .into_iter()
        .map(|(group_id, mut pms)| {
            pms.sort_by(|a, b| a.name.cmp(&b.name));
            let active_pm = active_pm_id_for_group(db, building, &group_id);
            let group_name = pms
                .first()
                .map(|pm| hoi4_content::production_method_group_name(pm))
                .unwrap_or_else(|| hoi4_content::pm_group_name(&group_id).to_owned());
            let candidates = pms
                .into_iter()
                .map(
                    |pm| hoi4_ui::construction_v6_panel::ProductionMethodCandidateV6Entry {
                        pm_id: pm.id.clone(),
                        pm_name: name_resolver.content_name(
                            DisplayNameKind::ProductionMethod,
                            &pm.id,
                            &pm.name,
                        ),
                        locked_reason: pm_lock_reason(name_resolver, world, db, player, pm),
                        prediction: pm_prediction(name_resolver, db, pm, building.level),
                    },
                )
                .collect();
            hoi4_ui::construction_v6_panel::ProductionMethodGroupV6Entry {
                group_id,
                group_name,
                active_pm,
                candidates,
            }
        })
        .collect()
}

fn building_name(
    name_resolver: &DisplayNameResolver<'_>,
    db: &hoi4_content::V6Database,
    building_id: &str,
) -> String {
    db.buildings
        .iter()
        .find(|bd| bd.id == building_id)
        .map(|bd| name_resolver.content_name(DisplayNameKind::Building, &bd.id, &bd.name))
        .unwrap_or_else(|| {
            name_resolver.content_name(DisplayNameKind::Building, building_id, building_id)
        })
}

fn flow_summary(
    name_resolver: &DisplayNameResolver<'_>,
    db: &hoi4_content::V6Database,
    flows: &HashMap<String, f32>,
    sign: &str,
) -> String {
    let mut items: Vec<_> = flows.iter().collect();
    items.sort_by(|a, b| {
        b.1.abs()
            .partial_cmp(&a.1.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items
        .into_iter()
        .take(3)
        .filter(|(_, amount)| amount.abs() > 0.01)
        .map(|(good_id, amount)| {
            format!(
                "{} {}{:.0}/d",
                good_name(name_resolver, db, good_id),
                sign,
                amount.abs()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn state_name(name_resolver: &DisplayNameResolver<'_>, world: &World, state_idx: usize) -> String {
    world
        .states
        .names
        .get(state_idx)
        .map(|raw| name_resolver.state_name(raw, state_idx))
        .unwrap_or_else(|| name_resolver.state_name("", state_idx))
}

fn employment_gap_for_building(
    db: &hoi4_content::V6Database,
    building: &hoi4_state::Building,
) -> [u32; 6] {
    let mut employment_gap = [0u32; 6];
    let pms = hoi4_content::active_pms_for_building(building, db);
    for ci in 0..6 {
        let needed_per_level: u32 = pms
            .iter()
            .map(|pm| pm.employment_demand.get(ci).copied().unwrap_or(0))
            .sum();
        let needed = needed_per_level * building.level as u32;
        let filled = building.employment.get(ci).copied().unwrap_or(0);
        employment_gap[ci] = needed.saturating_sub(filled);
    }
    employment_gap
}
