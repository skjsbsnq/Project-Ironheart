//! V8 construction planning: shared scoring for player auto-build and later investment AI.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use hoi4_content::{default_pms_for_building, v6_loader::BuildingKindDef, V6Database};
use hoi4_state::{CountryId, LawCategory, StateId, World};
use rayon::prelude::*;

use super::{construction_tick, EconomyState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionPlanningMode {
    PlayerAutoBuild,
    PrivateInvestment,
    CartelInvestment,
    PlannedEconomy,
    AiCountry,
    OverlordDevelopment,
}

#[derive(Debug, Clone)]
pub struct ConstructionCandidateScore {
    pub building_id: String,
    pub state: StateId,
    pub score: f32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ConstructionRejectedCandidate {
    pub building_id: String,
    pub state: Option<StateId>,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConstructionPlanResult {
    pub candidates: Vec<ConstructionCandidateScore>,
    pub rejected: Vec<ConstructionRejectedCandidate>,
}

pub fn target_queue_len_from_cp(cp: u32) -> usize {
    (((cp.max(1) as usize) / 6) + 2).clamp(3, 8)
}

pub fn plan_construction(
    world: &World,
    econ: &EconomyState,
    db: &V6Database,
    country: CountryId,
    mode: ConstructionPlanningMode,
    cp: u32,
) -> ConstructionPlanResult {
    if country.is_none() || country.0 as usize >= world.countries.count {
        return ConstructionPlanResult::default();
    }
    let ci = country.0 as usize;
    let queue = econ.construction.get(ci);
    let market = world.countries.market.markets.get(ci);
    let mut queued_by_state = vec![0u16; world.states.count];
    let mut queued_by_building_state: HashMap<(u16, String), u8> = HashMap::new();
    if let Some(queue) = queue {
        for item in &queue.items {
            let si = item.target_state.0 as usize;
            if si < queued_by_state.len() {
                queued_by_state[si] = queued_by_state[si].saturating_add(1);
            }
            *queued_by_building_state
                .entry((item.target_state.0, item.building_key.clone()))
                .or_default() += 1;
        }
    }

    let military = military_demand(db, econ, ci);
    let is_corporatist = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
        .current
        == "corporatist_war_economy";
    let has_military_demand = !military.demanded_buildings.is_empty();
    let shortage = good_shortages(market);
    let building_level_by_state_def = building_level_index(world);
    let mut result = ConstructionPlanResult::default();

    let mut state_ids = world.country_state_ids(country);
    if state_ids.is_empty() {
        state_ids = (0..world.states.count)
            .filter(|&si| world.states.owners[si] == country)
            .map(|si| StateId(si as u16))
            .collect();
    }

    let state_results: Vec<ConstructionPlanResult> = state_ids
        .par_iter()
        .copied()
        .map(|state| {
            let mut local = ConstructionPlanResult::default();
            let si = state.0 as usize;
            if si >= world.states.count {
                return local;
            }
            if world.states.owners[si] != country || world.states.controllers[si] != country {
                return local;
            }
            let used_projected = world
                .state_building_levels(state)
                .saturating_add(queued_by_state[si]);
            if used_projected >= v6_state_building_capacity(world, state) {
                local.rejected.push(ConstructionRejectedCandidate {
                    building_id: "*".to_owned(),
                    state: Some(state),
                    reason: "州建筑槽已满".to_owned(),
                });
                return local;
            }

            for def in &db.buildings {
                if !def.buildable {
                    local.rejected.push(ConstructionRejectedCandidate {
                        building_id: def.id.clone(),
                        state: Some(state),
                        reason: "建筑定义不可建造".to_owned(),
                    });
                    continue;
                }
                if matches!(def.kind, BuildingKindDef::MilitaryBase) {
                    continue;
                }
                let current_level =
                    current_building_level(&building_level_by_state_def, &def.id, state);
                let queued_same = queued_by_building_state
                    .get(&(state.0, def.id.clone()))
                    .copied()
                    .unwrap_or(0);
                if current_level.saturating_add(queued_same) >= def.max_level {
                    local.rejected.push(ConstructionRejectedCandidate {
                        building_id: def.id.clone(),
                        state: Some(state),
                        reason: "已达到建筑等级上限".to_owned(),
                    });
                    continue;
                }
                if let Err(reason) =
                    construction_tick::validate_build_location(world, db, ci, def, state)
                {
                    local.rejected.push(ConstructionRejectedCandidate {
                        building_id: def.id.clone(),
                        state: Some(state),
                        reason: format!("选址不可用：{reason:?}"),
                    });
                    continue;
                }

                let mut score = world.states.infrastructure[si] as f32 * 6.0;
                let mut reasons = vec![format!(
                    "基础设施 {} 提供建造效率",
                    world.states.infrastructure[si]
                )];
                if !has_military_demand {
                    let pms = default_pms_for_building(db, &def.id);
                    let expected_profit = expected_profit_rm_daily(
                        &pms,
                        current_level.saturating_add(queued_same).saturating_add(1),
                        market,
                    );
                    let profit_score = (expected_profit / 1_000.0).clamp(-40.0, 220.0) as f32;
                    score += profit_score;
                    if profit_score.abs() > 1.0 {
                        reasons.push(format!("预计日利润 {:.1}K RM", expected_profit / 1_000.0));
                    }
                }

                add_kind_score(def.kind, &def.id, mode, &mut score, &mut reasons);
                add_shortage_score(db, &def.id, &shortage, &mut score, &mut reasons);
                add_chain_score(db, &def.id, &shortage, &mut score, &mut reasons);
                add_military_score(
                    &def.id,
                    def.kind,
                    &military,
                    is_corporatist,
                    &mut score,
                    &mut reasons,
                );
                if def.id == "construction_sector" {
                    let bonus = if cp < 30 { 110.0 } else { 35.0 };
                    score += bonus;
                    reasons.push("建造能力不足，优先扩建造部门".to_owned());
                }
                if def.id == "power_plant" {
                    score += 18.0;
                    reasons.push("电力支撑工业扩张".to_owned());
                }

                local.candidates.push(ConstructionCandidateScore {
                    building_id: def.id.clone(),
                    state,
                    score,
                    reasons: top_reasons(reasons),
                });
            }
            local
        })
        .collect();

    for mut local in state_results {
        result.candidates.append(&mut local.candidates);
        result.rejected.append(&mut local.rejected);
    }

    result.candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.building_id.cmp(&b.building_id))
    });
    result
}

fn v6_state_building_capacity(world: &World, state: StateId) -> u16 {
    let si = state.0 as usize;
    if si >= world.states.count {
        return 0;
    }
    (world.states.category_slots[si] as u16).max(4) + 20
}

fn building_level_index(world: &World) -> HashMap<(u16, String), u8> {
    let mut out = HashMap::with_capacity(world.countries.buildings_v6.buildings.len());
    for building in &world.countries.buildings_v6.buildings {
        out.entry((building.state.0, building.building_def_id.clone()))
            .and_modify(|level: &mut u8| *level = (*level).max(building.level))
            .or_insert(building.level);
    }
    out
}

fn current_building_level(
    building_level_by_state_def: &HashMap<(u16, String), u8>,
    building_id: &str,
    state: StateId,
) -> u8 {
    building_level_by_state_def
        .get(&(state.0, building_id.to_owned()))
        .copied()
        .unwrap_or(0)
}

fn good_shortages(market: Option<&hoi4_state::market::NationalMarket>) -> HashMap<String, f32> {
    let mut shortages = HashMap::new();
    let Some(market) = market else {
        return shortages;
    };
    for (good_id, demand) in &market.demand {
        let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
        let unmet = market.unmet_demand.get(good_id).copied().unwrap_or(0.0);
        let shortage = unmet.max((*demand - supply).max(0.0));
        if shortage > 0.01 {
            shortages.insert(good_id.clone(), shortage / demand.max(1.0));
        }
    }
    shortages
}

fn add_kind_score(
    kind: BuildingKindDef,
    building_id: &str,
    mode: ConstructionPlanningMode,
    score: &mut f32,
    reasons: &mut Vec<String>,
) {
    let bonus = match kind {
        BuildingKindDef::Military => 8.0,
        BuildingKindDef::Industrial => 46.0,
        BuildingKindDef::Resource => 40.0,
        BuildingKindDef::Infrastructure => 30.0,
        BuildingKindDef::Agriculture => 24.0,
        BuildingKindDef::ConsumerGoods => 22.0,
        BuildingKindDef::Service => 18.0,
        BuildingKindDef::MilitaryBase => 0.0,
    };
    *score += bonus;
    if matches!(
        mode,
        ConstructionPlanningMode::PlannedEconomy | ConstructionPlanningMode::AiCountry
    ) && matches!(
        building_id,
        "steel_mill" | "machinery_workshop" | "power_plant"
    ) {
        *score += 20.0;
        reasons.push("计划/AI 模式偏好基础工业".to_owned());
    }
}

fn add_shortage_score(
    db: &V6Database,
    building_id: &str,
    shortage: &HashMap<String, f32>,
    score: &mut f32,
    reasons: &mut Vec<String>,
) {
    for pm in db
        .production_methods
        .iter()
        .filter(|pm| pm.building_id == building_id)
    {
        for good_id in &pm.output_good_ids {
            if let Some(ratio) = shortage.get(good_id) {
                let bonus = (*ratio * 90.0).clamp(0.0, 90.0);
                *score += bonus;
                reasons.push(format!("缓解 {good_id} 短缺"));
            }
        }
    }
}

fn add_chain_score(
    db: &V6Database,
    building_id: &str,
    shortage: &HashMap<String, f32>,
    score: &mut f32,
    reasons: &mut Vec<String>,
) {
    if shortage.contains_key("clothes") && building_id == "textile_mill" {
        *score += 85.0;
        reasons.push("衣物短缺，纺织厂是直接补缺建筑".to_owned());
    }
    if shortage.contains_key("meat")
        && shortage.contains_key("grain")
        && building_id == "grain_farm"
    {
        *score += 120.0;
        reasons.push("肉类链缺饲料，先补粮食/农业基础".to_owned());
    } else if shortage.contains_key("meat") && building_id == "livestock_ranch" {
        let bonus = if shortage.contains_key("grain") {
            15.0
        } else {
            90.0
        };
        *score += bonus;
        reasons.push(if shortage.contains_key("grain") {
            "肉类短缺但粮食不足，牧场扩张降权".to_owned()
        } else {
            "肉类短缺，牧场可直接补缺".to_owned()
        });
    }
    if shortage.contains_key("steel")
        && matches!(building_id, "steel_mill" | "iron_mine" | "coal_mine")
    {
        *score += 75.0;
        reasons.push("钢铁短缺，比较钢厂与煤铁上游".to_owned());
    }
    if shortage.contains_key("rubber_parts") {
        let outputs_rubber_chain = db.production_methods.iter().any(|pm| {
            pm.building_id == building_id
                && pm
                    .output_good_ids
                    .iter()
                    .any(|good| matches!(good.as_str(), "rubber_parts" | "rubber"))
        });
        if outputs_rubber_chain || matches!(building_id, "rubber_plantation" | "synthetic_refinery")
        {
            *score += 95.0;
            reasons.push("军工橡胶件短缺，追到橡胶/橡胶件上游".to_owned());
        }
    }
}

struct MilitaryDemandScore {
    demanded_buildings: HashMap<String, f32>,
    upstream_buildings: HashSet<String>,
}

fn military_demand(db: &V6Database, econ: &EconomyState, ci: usize) -> MilitaryDemandScore {
    let normalize = |id: &str| id.trim().to_ascii_lowercase();
    let mut demanded_buildings: HashMap<String, f32> = HashMap::new();
    let mut demanded_inputs: HashSet<String> = HashSet::new();
    for order in econ.government_orders.get(ci).into_iter().flatten() {
        let order_category = normalize(&order.equipment_category);
        let order_weight = (order.daily_budget_rm / 100_000.0).clamp(1.0, 40.0) as f32;
        for pm in &db.production_methods {
            let Some(eq_out) = &pm.equipment_output else {
                continue;
            };
            if normalize(&eq_out.equipment_category) != order_category {
                continue;
            }
            *demanded_buildings
                .entry(pm.building_id.clone())
                .or_default() += order_weight;
            demanded_inputs.extend(pm.input_good_ids.iter().cloned());
        }
    }
    let upstream_buildings = db
        .production_methods
        .iter()
        .filter(|pm| {
            pm.output_good_ids
                .iter()
                .any(|good| demanded_inputs.contains(good))
        })
        .map(|pm| pm.building_id.clone())
        .collect();
    MilitaryDemandScore {
        demanded_buildings,
        upstream_buildings,
    }
}

fn add_military_score(
    building_id: &str,
    kind: BuildingKindDef,
    military: &MilitaryDemandScore,
    is_corporatist: bool,
    score: &mut f32,
    reasons: &mut Vec<String>,
) {
    let demand_weight = military
        .demanded_buildings
        .get(building_id)
        .copied()
        .unwrap_or(0.0);
    if demand_weight > 0.0 {
        *score += 35.0 + demand_weight * 12.0;
        reasons.push("政府军购订单需要该建筑产能".to_owned());
    } else if matches!(kind, BuildingKindDef::Military) {
        *score += 8.0;
    }
    if is_corporatist {
        if demand_weight > 0.0 {
            *score += 90.0 + demand_weight * 10.0;
            reasons.push("法团战备经济强化军工订单优先级".to_owned());
        } else if matches!(kind, BuildingKindDef::Military) {
            *score += 12.0;
        }
        if military.upstream_buildings.contains(building_id) {
            *score += 95.0;
            reasons.push("军工订单上游投入品短缺风险".to_owned());
        }
        if matches!(
            building_id,
            "steel_mill"
                | "machinery_workshop"
                | "chemical_plant"
                | "electrical_works"
                | "coal_mine"
                | "iron_mine"
                | "oil_rig"
                | "rubber_plantation"
                | "bauxite_mine"
                | "synthetic_refinery"
                | "power_plant"
        ) {
            *score += 75.0;
            reasons.push("战备经济偏好重工业与战略资源".to_owned());
        }
    }
}

fn expected_profit_rm_daily(
    pms: &[&hoi4_content::ProductionMethodDef],
    level: u8,
    market: Option<&hoi4_state::market::NationalMarket>,
) -> f64 {
    super::valuation::expected_profit_rm_daily(pms, level, market)
}

fn top_reasons(mut reasons: Vec<String>) -> Vec<String> {
    reasons.dedup();
    reasons.sort_by_key(|reason| {
        if reason.contains("衣物短缺")
            || reason.contains("先补粮食")
            || reason.contains("军工")
            || reason.contains("钢铁短缺")
            || reason.contains("肉类短缺")
        {
            0
        } else if reason.contains("缓解") {
            1
        } else {
            2
        }
    });
    reasons.into_iter().take(3).collect()
}
