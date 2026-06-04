//! V6 璁″垝缁忔祹 tick锛氶厤棰濅笅锟?锟?閰嶇粰鍒讹拷?//!
//! D4 纭害鏉燂紙HC-3锛夛細鏈枃锟?**涓嶅彲** import `market_tick` 鐨勪换浣曞唴閮ㄥ嚱鏁帮拷?//! 鎵€鏈夊叡浜€昏緫蹇呴』锟?`EconomicSystemTick` trait锟?//!
//! V6.D 瀹屾暣瀹炵幇锟?
use hoi4_content::{PopNeedTierDef, ProductionMethodDef, V6Database};
use hoi4_state::{BuildingKind, LawCategory, PopClass, World};

use super::building_tick_common::{
    active_available_pms, compute_employment_ratio, compute_input_fulfillment_ratio,
    compute_qualification_ratio_from_totals, country_building_indices, country_pop_indices,
    good_is_unlocked, pop_group_qualifies_for_class_job, state_owned_by_parts,
    state_owned_by_world,
};
use super::econ_system_tick::EconomicSystemTick;
use super::finance_tick;
use super::EconomyState;
use crate::trade;

const SATISFACTION_EMA_FACTOR: f32 = 0.02;
const LABOR_UPDATE_INTERVAL_DAYS: i64 = 7;
const CLASS_MOBILITY_INTERVAL_DAYS: i64 = 90;
pub struct PlannedTick;

impl EconomicSystemTick for PlannedTick {
    fn tick_daily(
        world: &mut World,
        econ: &mut EconomyState,
        db: &V6Database,
        country_idx: usize,
        day: i64,
    ) {
        let ci = country_idx;
        if ci >= world.countries.count {
            return;
        }

        finance_tick::reset_daily_accumulators(world, ci);
        super::law_modifiers::apply(world, db, ci);
        if day % LABOR_UPDATE_INTERVAL_DAYS == 0 {
            step_pop_education(world, db, ci);
        }

        let completed_techs: Vec<String> = world.countries.completed_techs[ci].clone();

        step_quota_production(world, econ, db, ci, &completed_techs);
        if day % LABOR_UPDATE_INTERVAL_DAYS == 0 {
            step_pop_employment(world, db, ci);
            econ.invalidate_qualification_totals();
            step_pop_wage(world, db, ci);
        } else {
            step_pop_income_from_existing_wage(world, ci);
        }
        step_rationing(world, db, ci);

        if day % CLASS_MOBILITY_INTERVAL_DAYS == 0 {
            step_class_mobility(world, ci);
        }

        step_military_production_planned(world, econ, db, ci, &completed_techs);
        super::building_runtime::update(world, db, ci);

        finance_tick::step_collect_taxes_planned(world, db, ci);
        finance_tick::step_pay_wages_to_treasury(world, db, ci);
        finance_tick::step_pay_military_upkeep(world, econ, ci);
        finance_tick::step_construction_cost(world, db, ci);
        finance_tick::step_construction_project_funding(world, econ, ci);
        finance_tick::step_welfare_spending(world, db, ci);

        finance_tick::step_planned_price_lock(world, db, ci);

        finance_tick::step_pay_interest(world, ci);
        finance_tick::step_update_gdp(world, db, ci, day);
        finance_tick::step_check_mefo_crisis(world, db, ci, day);
        finance_tick::step_update_exchange_rate(world, db, ci, day);

        trade::step_blockade_check(world, ci);
        trade::step_trade_matching(world, db, ci);
        trade::step_trade_agreements(world, db, ci);
        super::market_balance::settle_market(world, db, ci);
        super::market_balance::confirm_government_procurement(world, ci);
        super::market_balance::confirm_construction_procurement(world, ci);
        trade::step_blockade_check(world, ci);
        trade::step_blockade_satisfaction_impact(world, ci);
        trade::step_gold_standard_and_forex(world, db, ci);
        finance_tick::step_auto_mefo(world, db, ci);
        finance_tick::step_compute_fiscal_summary(world, ci);

        step_pop_satisfaction_from_clearing(world, db, ci);
        step_pop_radicalism_planned(world, ci);
        step_pop_loyalty(world, db, ci);
    }
}

fn step_quota_production(
    world: &mut World,
    econ: &mut EconomyState,
    db: &V6Database,
    ci: usize,
    completed_techs: &[String],
) {
    let locked_goods: std::collections::HashSet<&str> = db
        .goods
        .iter()
        .filter_map(|good| {
            good.unlocked_by
                .as_ref()
                .filter(|tech| !completed_techs.contains(tech))
                .map(|_| good.id.as_str())
        })
        .collect();

    let previous_supply = world.countries.market.markets[ci].supply.clone();
    let previous_stockpile = world.countries.market.markets[ci].stockpile.clone();
    {
        let market = &mut world.countries.market.markets[ci];
        let good_keys: Vec<String> = market.supply.keys().cloned().collect();
        for good_key in &good_keys {
            if let Some(supply) = market.supply.get_mut(good_key) {
                *supply = 0.0;
            }
            if let Some(demand) = market.demand.get_mut(good_key) {
                *demand = 0.0;
            }
        }
        for import in market.imports.values_mut() {
            *import = 0.0;
        }
        for export in market.exports.values_mut() {
            *export = 0.0;
        }
    }

    let active_plan = db.pyatiletka_plans.iter().find(|p| {
        let year = world.date.year as i32;
        year >= p.year_start as i32 && year <= p.year_end as i32
    });

    let buildings = &world.countries.buildings_v6.buildings;
    let mut supply_delta: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let mut planned_output_so_far: std::collections::HashMap<String, f32> =
        std::collections::HashMap::new();
    let qualification_totals = econ.qualification_totals(world);

    for building_idx in country_building_indices(world, ci) {
        let Some(building) = buildings.get(building_idx) else {
            continue;
        };
        if building.level == 0 {
            continue;
        }

        if let Some((cat, law_id)) = &building.requires_law {
            let current_law = &world.countries.law_store.law_sets[ci].0[cat.index()].current;
            if current_law != law_id {
                continue;
            }
        }

        let forced_pm_id = active_plan.and_then(|plan| {
            plan.forced_pms
                .iter()
                .find(|fp| fp.building_def_id == building.building_def_id)
                .map(|fp| fp.forced_pm_id.clone())
        });

        let pm = match forced_pm_id {
            Some(fpm) => db
                .production_methods
                .iter()
                .find(|pm| {
                    pm.id == fpm
                        || (pm.building_id == building.building_def_id && pm.id.ends_with(&fpm))
                })
                .or_else(|| {
                    db.production_methods.iter().find(|pm| {
                        pm.building_id == building.building_def_id && pm.id.ends_with("default")
                    })
                }),
            None => db.production_methods.iter().find(|pm| {
                pm.building_id == building.building_def_id && pm.id.ends_with("default")
            }),
        };
        let pm = match pm {
            Some(pm) => pm,
            None => continue,
        };

        if let Some(unlock_tech) = &pm.unlocked_by {
            if !completed_techs.contains(unlock_tech) {
                continue;
            }
        }

        let state_idx = building.state.0 as usize;
        let infra = if state_idx < world.states.count {
            world.states.infrastructure[state_idx] as f32
        } else {
            0.0
        };
        let infra_mult = 1.0 + 0.2 * infra;

        let employment_ratio = compute_employment_ratio(building, &[pm]);
        let input_fulfillment_ratio = compute_input_fulfillment_ratio(
            &[pm],
            building.level,
            employment_ratio,
            &previous_supply,
            &previous_stockpile,
        );
        let production_employment_ratio = if input_fulfillment_ratio < 0.5 {
            employment_ratio * 0.5
        } else {
            employment_ratio
        };

        let qualification_ratio =
            compute_qualification_ratio_from_totals(&qualification_totals, building_idx, &[pm]);

        let output_mult = production_employment_ratio
            * input_fulfillment_ratio
            * qualification_ratio
            * infra_mult;

        let quota_mult = planned_quota_multiplier(active_plan, pm, building.level, output_mult);

        for (i, good_id) in pm.output_good_ids.iter().enumerate() {
            if locked_goods.contains(good_id.as_str()) {
                continue;
            }
            let mut amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                * building.level as f32
                * output_mult
                * quota_mult;
            if let Some(target) = planned_target_for_good(active_plan, good_id) {
                let produced_so_far = planned_output_so_far.get(good_id).copied().unwrap_or(0.0);
                let remaining = (target - produced_so_far).max(0.0);
                amount = amount.min(remaining);
                planned_output_so_far.insert(good_id.clone(), produced_so_far + amount);
            }
            if amount <= 0.0 {
                continue;
            }
            *supply_delta.entry(good_id.clone()).or_insert(0.0) += amount;
        }

        for (i, good_id) in pm.input_good_ids.iter().enumerate() {
            let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                * building.level as f32
                * employment_ratio;
            let market = &mut world.countries.market.markets[ci];
            let stockpile = market.stockpile.get(good_id).copied().unwrap_or(0.0);
            let available = stockpile.max(0.0);
            let consumed = amount.min(available);
            if consumed > 0.0 {
                if let Some(sp) = market.stockpile.get_mut(good_id) {
                    *sp -= consumed;
                }
            }
        }
    }

    let market = &mut world.countries.market.markets[ci];
    for (good_id, amount) in supply_delta {
        *market.supply.entry(good_id.clone()).or_insert(0.0) += amount;
        *market.stockpile.entry(good_id).or_insert(0.0) += amount;
    }
}

fn step_pop_education(world: &mut World, _db: &V6Database, ci: usize) {
    let country_id = hoi4_state::CountryId(ci as u16);
    let state_count = world.states.count;
    let state_owners = &world.states.owners;
    if !state_owners.iter().any(|owner| *owner == country_id) {
        return;
    }

    let population: u32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| state_owned_by_parts(state_count, state_owners, pg.state, country_id))
        .map(|pg| pg.size)
        .sum();
    if population == 0 {
        return;
    }

    let clerk_population: u32 = world
        .countries
        .pops
        .groups
        .iter()
        .filter(|pg| {
            pg.class == PopClass::Clerk
                && state_owned_by_parts(state_count, state_owners, pg.state, country_id)
        })
        .map(|pg| pg.size)
        .sum();
    let university_levels: u32 = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .filter(|building| {
            building.building_def_id == "university"
                && building.level > 0
                && state_owned_by_parts(state_count, state_owners, building.state, country_id)
        })
        .map(|building| building.level as u32)
        .sum();
    let university_capacity = university_levels as f32 * 250_000.0 / population as f32;
    let clerk_ratio = clerk_population as f32 / population as f32;
    let education_target =
        (0.35 + university_capacity * 0.60 + clerk_ratio * 1.30).clamp(0.05, 0.95);
    let skilled_target = (0.08 + university_capacity * 0.40 + clerk_ratio * 0.90).clamp(0.02, 0.80);

    for pg in &mut world.countries.pops.groups {
        if !state_owned_by_parts(state_count, state_owners, pg.state, country_id) {
            continue;
        }
        let class_literacy_target = (education_target * 0.65 + pg.class.baseline_literacy() * 0.35)
            .max(pg.class.baseline_literacy() * 0.50)
            .clamp(0.0, 1.0);
        let class_skilled_target = (skilled_target * 0.60
            + pg.class.baseline_skilled_ratio() * 0.40)
            .max(pg.class.baseline_skilled_ratio() * 0.50)
            .clamp(0.0, 1.0);
        pg.literacy += (class_literacy_target - pg.literacy) * 0.0008;
        pg.skilled_ratio += (class_skilled_target - pg.skilled_ratio) * 0.0010;
        pg.literacy = pg.literacy.clamp(0.0, 1.0);
        pg.skilled_ratio = pg.skilled_ratio.clamp(0.0, 1.0);
    }
}

fn planned_quota_multiplier(
    active_plan: Option<&hoi4_content::v6_loader::PyatiletkaDef>,
    pm: &ProductionMethodDef,
    building_level: u8,
    output_mult: f32,
) -> f32 {
    let Some(plan) = active_plan else {
        return 1.0;
    };
    let Some(target) = pm
        .output_good_ids
        .iter()
        .find_map(|good_id| planned_target_for_good(Some(plan), good_id))
    else {
        return 1.0;
    };
    let current_output: f32 = pm
        .output_good_ids
        .iter()
        .zip(pm.output_good_amounts.iter())
        .filter(|(good_id, _)| planned_target_for_good(Some(plan), good_id).is_some())
        .map(|(_, amount)| *amount * building_level as f32 * output_mult)
        .sum();
    if current_output > 0.0 {
        (target / current_output).clamp(0.0, 2.0)
    } else {
        1.0
    }
}

fn planned_target_for_good(
    active_plan: Option<&hoi4_content::v6_loader::PyatiletkaDef>,
    good_id: &str,
) -> Option<f32> {
    active_plan
        .and_then(|plan| plan.targets.iter().find(|target| target.good_id == good_id))
        .map(|target| target.target_daily_output)
}

fn step_pop_employment(world: &mut World, db: &V6Database, ci: usize) {
    let country_id = hoi4_state::CountryId(ci as u16);

    let completed_techs = world.countries.completed_techs[ci].clone();
    let building_count = world.countries.buildings_v6.buildings.len();

    for building_idx in 0..building_count {
        {
            let building = &world.countries.buildings_v6.buildings[building_idx];
            if !state_owned_by_world(world, building.state, country_id) || building.level == 0 {
                continue;
            }
            if let Some((cat, law_id)) = &building.requires_law {
                let current_law = &world.countries.law_store.law_sets[ci].0[cat.index()].current;
                if current_law != law_id {
                    release_building_employment(world, building_idx);
                    continue;
                }
            }
        }

        let building_level = world.countries.buildings_v6.buildings[building_idx].level;
        let building_state = world.countries.buildings_v6.buildings[building_idx].state;

        let pms = active_available_pms(
            &world.countries.buildings_v6.buildings[building_idx],
            db,
            ci,
            world,
            &completed_techs,
        );
        if pms.is_empty() {
            continue;
        }

        let building_bld_id = hoi4_state::BuildingId(building_idx as u32);
        let mut employment_demand = [0u32; 6];
        for pm in &pms {
            for class_idx in 0..6 {
                employment_demand[class_idx] += pm.employment_demand[class_idx];
            }
        }

        for class_idx in 0..6 {
            let needed_per_level = employment_demand.get(class_idx).copied().unwrap_or(0) as u32;
            let needed_total = needed_per_level * building_level as u32;
            let current_employed =
                world.countries.buildings_v6.buildings[building_idx].employment[class_idx];

            if current_employed < needed_total {
                let deficit = needed_total - current_employed;
                let pop_class = match PopClass::from_index(class_idx) {
                    Some(c) => c,
                    None => continue,
                };

                let mut hired: u32 = 0;
                let state_class_pool: u32 = world
                    .countries
                    .pops
                    .groups
                    .iter()
                    .filter(|pg| {
                        pg.class == pop_class
                            && pg.state == building_state
                            && pg.employed_at.is_none()
                            && pop_group_qualifies_for_class_job(pg, pop_class, &pms)
                    })
                    .map(|pg| pg.size)
                    .sum();
                let country_class_pool: u32 = if state_class_pool == 0 {
                    world
                        .countries
                        .pops
                        .groups
                        .iter()
                        .filter(|pg| {
                            pg.class == pop_class
                                && state_owned_by_world(world, pg.state, country_id)
                                && pg.employed_at.is_none()
                                && pop_group_qualifies_for_class_job(pg, pop_class, &pms)
                        })
                        .map(|pg| pg.size)
                        .sum()
                } else {
                    state_class_pool
                };
                let building_hire_cap = (country_class_pool / 2).max(1);
                let hire_limit = deficit.min(building_hire_cap);
                let mut pop_idx = 0;
                while pop_idx < world.countries.pops.groups.len() {
                    if hired >= hire_limit {
                        break;
                    }
                    let pg = &world.countries.pops.groups[pop_idx];
                    if pg.class != pop_class || pg.employed_at.is_some() {
                        pop_idx += 1;
                        continue;
                    }
                    if !pop_group_qualifies_for_class_job(pg, pop_class, &pms) {
                        pop_idx += 1;
                        continue;
                    }
                    let can_hire_from_state = if state_class_pool > 0 {
                        pg.state == building_state
                    } else {
                        state_owned_by_world(world, pg.state, country_id)
                    };
                    if !can_hire_from_state {
                        pop_idx += 1;
                        continue;
                    }

                    let available = pg.size;
                    let want = hire_limit - hired;
                    let take = available.min(want);

                    if take > 0 {
                        if take < available {
                            let mut hired_group = world.countries.pops.groups[pop_idx].clone();
                            hired_group.size = take;
                            hired_group.employed_at = Some(building_bld_id);
                            world.countries.pops.groups[pop_idx].size -= take;
                            world.push_pop_group(hired_group);
                        } else {
                            world.countries.pops.groups[pop_idx].employed_at =
                                Some(building_bld_id);
                        }
                        world.countries.buildings_v6.buildings[building_idx].employment
                            [class_idx] += take;
                        hired += take;
                    }
                    pop_idx += 1;
                }
            } else if current_employed > needed_total {
                let surplus = current_employed - needed_total;
                let pop_class = match PopClass::from_index(class_idx) {
                    Some(c) => c,
                    None => continue,
                };

                let mut fired: u32 = 0;
                for pop_idx in 0..world.countries.pops.groups.len() {
                    if fired >= surplus {
                        break;
                    }
                    let pg = &world.countries.pops.groups[pop_idx];
                    if pg.class != pop_class || pg.employed_at != Some(building_bld_id) {
                        continue;
                    }

                    let take = pg.size.min(surplus - fired);
                    if take > 0 {
                        world.countries.pops.groups[pop_idx].employed_at = None;
                        world.countries.buildings_v6.buildings[building_idx].employment
                            [class_idx] -= take;
                        fired += take;
                    }
                }
            }
        }
    }
}

fn step_pop_wage(world: &mut World, db: &V6Database, ci: usize) {
    let current_economy_law = world.countries.law_store.law_sets[ci].0
        [LawCategory::Economy.index()]
    .current
    .clone();
    let economy_def = db.economy_laws.iter().find(|l| l.id == current_economy_law);
    let worker_mult = economy_def
        .map(|ed| ed.wage_multiplier_worker)
        .unwrap_or(1.0);
    let capitalist_mult = economy_def
        .map(|ed| ed.wage_multiplier_capitalist)
        .unwrap_or(1.0);

    let base_wage: [f32; 6] = [1.5, 3.0, 5.0, 15.0, 25.0, 2.5];

    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        let class_idx = pg.class.index();
        let mut wage = base_wage[class_idx];
        match pg.class {
            PopClass::Worker => wage *= worker_mult,
            PopClass::Capitalist => wage *= capitalist_mult,
            _ => {}
        }
        if pg.employed_at.is_some() {
            pg.wage_rm = wage;
        } else {
            pg.wage_rm = 0.0;
        }
        pg.tax_burden = 0.0;
        pg.income_rm = pg.wage_rm;
        pg.tax_paid_rm = 0.0;
        pg.disposable_income_rm = pg.income_rm;
    }
}

fn step_pop_income_from_existing_wage(world: &mut World, ci: usize) {
    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        if pg.class == PopClass::Soldier {
            continue;
        }
        if pg.income_rm == 0.0 && pg.wage_rm > 0.0 {
            pg.income_rm = pg.wage_rm;
            pg.disposable_income_rm = pg.income_rm - pg.tax_paid_rm;
        }
    }
}

fn step_rationing(world: &mut World, db: &V6Database, ci: usize) {
    let completed_techs = world.countries.completed_techs[ci].clone();
    let integration_status = world.states.integration_status.clone();
    let market_price = world.countries.market.markets[ci].price.clone();

    let mut total_pop_demand: std::collections::HashMap<String, f32> =
        std::collections::HashMap::new();
    let mut luxury_demand: std::collections::HashMap<String, f32> =
        std::collections::HashMap::new();
    let mut pop_basic_budgets: Vec<(usize, f32)> = Vec::new();

    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get(pop_idx) else {
            continue;
        };
        if pg.class == PopClass::Soldier {
            continue;
        }

        let goods = db.pop_need_entries_for_class(pg.class);
        if goods.is_empty() {
            continue;
        }

        let si = pg.state.0 as usize;
        let integration_factor = integration_status
            .get(si)
            .copied()
            .map(finance_tick::integration_consumption_factor)
            .unwrap_or(1.0);
        let pop_millions = pg.size as f32 / 1_000_000.0 * integration_factor;

        let is_unemployed = pg.employed_at.is_none();
        let non_basic_mult = if is_unemployed { 0.3 } else { 1.0 };
        let mut basic_budget_rm = 0.0_f32;

        for need in goods {
            if !good_is_unlocked(db, &completed_techs, &need.good_id) {
                continue;
            }
            let price = market_price.get(&need.good_id).copied().unwrap_or(1.0);
            let demand = match need.tier {
                PopNeedTierDef::Essential | PopNeedTierDef::Normal => {
                    let demand = pop_millions * need.amount_per_million;
                    basic_budget_rm += demand * price;
                    demand
                }
                PopNeedTierDef::Luxury => {
                    let d = pop_millions * need.amount_per_million * non_basic_mult;
                    *luxury_demand.entry(need.good_id.clone()).or_insert(0.0) += d;
                    d
                }
            };
            *total_pop_demand.entry(need.good_id.clone()).or_insert(0.0) += demand;
        }
        pop_basic_budgets.push((pop_idx, basic_budget_rm));
    }

    for (pop_idx, basic_budget_rm) in pop_basic_budgets {
        if let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) {
            pg.basic_consumption_budget = basic_budget_rm;
        }
    }

    let market = &mut world.countries.market.markets[ci];
    let active_plan = db.pyatiletka_plans.iter().find(|p| {
        let year = world.date.year as i32;
        year >= p.year_start as i32 && year <= p.year_end as i32
    });

    for (good_id, demand) in &total_pop_demand {
        let supply = market.supply.get(good_id).copied().unwrap_or(0.0)
            + market.stockpile.get(good_id).copied().unwrap_or(0.0);
        let plan_allocation = active_plan
            .map(|plan| {
                plan.targets
                    .iter()
                    .find(|t| t.good_id == *good_id)
                    .map(|t| t.target_daily_output)
                    .unwrap_or(supply)
            })
            .unwrap_or(supply);

        let rationed_amount = demand.min(plan_allocation);
        if rationed_amount > 0.0 {
            let available = market.stockpile.get(good_id).copied().unwrap_or(0.0)
                + market.supply.get(good_id).copied().unwrap_or(0.0);
            let consumed = rationed_amount.min(available);
            if consumed > 0.0 {
                if let Some(sp) = market.stockpile.get_mut(good_id) {
                    let from_stockpile = consumed.min(*sp);
                    *sp -= from_stockpile;
                    if consumed - from_stockpile > 0.0 {
                        if let Some(sup) = market.supply.get_mut(good_id) {
                            *sup -= consumed - from_stockpile;
                        }
                    }
                } else if let Some(sup) = market.supply.get_mut(good_id) {
                    *sup -= consumed;
                }
            }
        }

        *market.demand.entry(good_id.clone()).or_insert(0.0) += *demand;

        let lux_amount = luxury_demand.get(good_id).copied().unwrap_or(0.0);
        if lux_amount > 0.0 {
            market.bucket_demand.add(
                good_id,
                hoi4_state::market::DemandBucketKind::PopNonBasicConsumption,
                lux_amount,
            );
        }
        let basic_amount = *demand - lux_amount;
        if basic_amount > 0.0 {
            market.bucket_demand.add(
                good_id,
                hoi4_state::market::DemandBucketKind::PopBasicConsumption,
                basic_amount,
            );
        }
    }
}

fn step_pop_satisfaction_from_clearing(world: &mut World, db: &V6Database, ci: usize) {
    let completed_techs = world.countries.completed_techs[ci].clone();

    let current_economy_law = world.countries.law_store.law_sets[ci].0
        [LawCategory::Economy.index()]
    .current
    .clone();
    let economy_law_mod = db
        .economy_laws
        .iter()
        .find(|l| l.id == current_economy_law)
        .map(|l| l.pop_modifiers.satisfaction)
        .unwrap_or(0.0);

    let law_modifier = economy_law_mod;

    let mut unlocked_needs: Vec<Vec<hoi4_content::PopNeedEntryDef>> =
        Vec::with_capacity(PopClass::COUNT);
    for class_idx in 0..PopClass::COUNT {
        let Some(class) = PopClass::from_index(class_idx) else {
            unlocked_needs.push(Vec::new());
            continue;
        };
        let goods = db.pop_need_entries_for_class(class);
        unlocked_needs.push(
            goods
                .iter()
                .filter(|need| good_is_unlocked(db, &completed_techs, &need.good_id))
                .cloned()
                .collect(),
        );
    }

    let clearing_sheet = &world.countries.market.markets[ci].clearing_sheet;

    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        if pg.class == PopClass::Soldier {
            continue;
        }

        let configured_goods = db.pop_need_entries_for_class(pg.class);
        if configured_goods.is_empty() {
            continue;
        }
        let goods = unlocked_needs
            .get(pg.class.index())
            .map(Vec::as_slice)
            .unwrap_or(configured_goods);

        let mut essential_sum = 0.0;
        let mut essential_count = 0.0;
        let mut normal_sum = 0.0;
        let mut normal_count = 0.0;
        let mut luxury_sum = 0.0;
        let mut luxury_count = 0.0;

        for need in goods {
            let fulfillment = clearing_fulfillment(clearing_sheet, &need.good_id);
            match need.tier {
                PopNeedTierDef::Essential => {
                    essential_sum += fulfillment;
                    essential_count += 1.0;
                }
                PopNeedTierDef::Normal => {
                    normal_sum += fulfillment;
                    normal_count += 1.0;
                }
                PopNeedTierDef::Luxury => {
                    luxury_sum += fulfillment;
                    luxury_count += 1.0;
                }
            }
        }

        pg.essential_needs_fulfillment = if essential_count > 0.0 {
            essential_sum / essential_count
        } else {
            1.0
        };
        pg.normal_needs_fulfillment = if normal_count > 0.0 {
            normal_sum / normal_count
        } else {
            1.0
        };
        pg.luxury_needs_fulfillment = if luxury_count > 0.0 {
            luxury_sum / luxury_count
        } else {
            1.0
        };
        pg.needs_fulfillment = (0.5 * pg.essential_needs_fulfillment
            + 0.35 * pg.normal_needs_fulfillment
            + 0.15 * pg.luxury_needs_fulfillment)
            .clamp(0.0, 1.0);

        let standard_of_living = pg.standard_of_living.clamp(0.0, 1.0);
        let employment_security = if pg.class == PopClass::Soldier {
            1.0
        } else if pg.employed_at.is_some() {
            1.0
        } else {
            0.3
        };
        let tax_burden = pg.tax_burden.clamp(0.0, 1.0);
        let income_ratio = if pg.income_rm > 0.0 {
            (pg.disposable_income_rm / pg.income_rm).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let law_effect = (1.0 + law_modifier + pg.satisfaction_law_modifier).clamp(0.0, 1.0);
        let class_target = (0.35 * pg.essential_needs_fulfillment
            + 0.20 * standard_of_living
            + 0.15 * employment_security
            + 0.10 * income_ratio
            + 0.05 * (1.0 - tax_burden)
            + 0.10 * law_effect
            + 0.03 * pg.normal_needs_fulfillment
            + 0.02 * pg.luxury_needs_fulfillment
            + 0.05 * pg.needs_fulfillment)
            .clamp(0.0, 1.0);

        pg.satisfaction +=
            SATISFACTION_EMA_FACTOR * (class_target.clamp(0.0, 1.0) - pg.satisfaction);
        pg.satisfaction = pg.satisfaction.clamp(0.0, 1.0);
        pg.standard_of_living =
            (0.75 * pg.standard_of_living + 0.25 * pg.needs_fulfillment).clamp(0.0, 1.0);
    }
}

fn clearing_fulfillment(sheet: &hoi4_state::market::MarketClearingSheet, good_id: &str) -> f32 {
    if let Some(result) = sheet.results.get(good_id) {
        if result.total_fulfilled + result.total_unmet > 0.0 {
            result.total_fulfilled / (result.total_fulfilled + result.total_unmet)
        } else {
            1.0
        }
    } else {
        1.0
    }
}

fn step_pop_radicalism_planned(world: &mut World, ci: usize) {
    let pop_indices = country_pop_indices(world, ci);
    let employed_count: u32 = pop_indices
        .iter()
        .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
        .filter(|p| p.class != PopClass::Soldier && p.employed_at.is_some())
        .map(|p| p.size)
        .sum();
    let total_civilian: u32 = pop_indices
        .iter()
        .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
        .filter(|p| p.class != PopClass::Soldier)
        .map(|p| p.size)
        .sum();
    let unemployment_rate = if total_civilian > 0 {
        1.0 - employed_count as f32 / total_civilian as f32
    } else {
        0.0
    };

    let mut radical_weighted = 0.0f64;
    let mut total_weight = 0u64;
    for pop_idx in pop_indices {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        if pg.class == PopClass::Soldier {
            continue;
        }

        let essential_pressure = ((0.8 - pg.essential_needs_fulfillment) / 0.8).clamp(0.0, 1.0);
        let unemployment_pressure = ((unemployment_rate - 0.1) / 0.4).clamp(0.0, 1.0);
        let income_pressure = if pg.income_rm > 0.0 {
            ((1.0 - pg.disposable_income_rm / pg.income_rm) - 0.3).clamp(0.0, 0.7) / 0.7
        } else {
            0.5
        };
        let tax_pressure = ((pg.tax_burden - 0.5) / 0.5).clamp(0.0, 1.0);
        let satisfaction_pressure = ((0.45 - pg.satisfaction) / 0.45).clamp(0.0, 1.0);
        let law_pressure = (-pg.satisfaction_law_modifier).clamp(0.0, 0.3) / 0.3;

        let target = (0.30 * essential_pressure
            + 0.20 * unemployment_pressure
            + 0.15 * income_pressure
            + 0.15 * tax_pressure
            + 0.15 * satisfaction_pressure
            + 0.05 * law_pressure)
            .clamp(0.0, 1.0);
        pg.radicalism += 0.08 * (target - pg.radicalism);
        pg.radicalism = pg.radicalism.clamp(0.0, 1.0);

        radical_weighted += pg.radicalism as f64 * pg.size as f64;
        total_weight += pg.size as u64;
    }

    if total_weight > 0 {
        let average_radicalism = (radical_weighted / total_weight as f64) as f32;
        let stability_pressure = (average_radicalism - 0.20).max(0.0) * 0.01;
        world.countries.stability[ci] =
            (world.countries.stability[ci] - stability_pressure).clamp(0.0, 1.0);
    }
}

fn step_pop_loyalty(world: &mut World, _db: &V6Database, ci: usize) {
    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        let satisfaction_impact = (pg.satisfaction - 0.5) * 0.01 * pg.loyalty_coefficient;
        let target_loyalty_delta = satisfaction_impact * pg.loyalty_decay_mult;
        pg.political_loyalty += target_loyalty_delta;
        pg.political_loyalty = pg.political_loyalty.clamp(-1.0, 1.0);
    }
}

fn step_class_mobility(world: &mut World, ci: usize) {
    let country_id = hoi4_state::CountryId(ci as u16);
    let state_count = world.states.count;
    let state_owners = world.states.owners.clone();
    let first_state = (0..state_count)
        .find(|&si| state_owners[si] == country_id)
        .map(|si| hoi4_state::StateId(si as u16))
        .unwrap_or(hoi4_state::StateId(0));

    let transitions: [(PopClass, PopClass); 3] = [
        (PopClass::Peasant, PopClass::Worker),
        (PopClass::Worker, PopClass::Clerk),
        (PopClass::Clerk, PopClass::Capitalist),
    ];

    for (from_class, to_class) in &transitions {
        let pop_indices = country_pop_indices(world, ci);
        let from_count: u32 = pop_indices
            .iter()
            .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
            .filter(|p| p.class == *from_class)
            .map(|p| p.size)
            .sum();
        let to_employed: u32 = pop_indices
            .iter()
            .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
            .filter(|p| p.class == *to_class && p.employed_at.is_some())
            .map(|p| p.size)
            .sum();
        let to_needed: u32 = compute_total_employment_demand(world, ci, *to_class);
        let demand_excess = if to_needed > to_employed {
            to_needed - to_employed
        } else {
            0
        };

        if demand_excess == 0 || from_count == 0 {
            continue;
        }

        let rate = 0.02 * demand_excess as f32 / to_employed.max(1) as f32;
        let flow = (from_count as f32 * rate) as u32;
        if flow == 0 {
            continue;
        }

        let mut remaining = flow;
        for &pop_idx in &pop_indices {
            if remaining == 0 {
                break;
            }
            let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
                continue;
            };
            if pg.class != *from_class {
                continue;
            }
            let take = pg.size.min(remaining);
            if take > 0 {
                pg.size -= take;
                remaining -= take;
            }
        }

        let mut to_add = flow - remaining;
        for &pop_idx in &pop_indices {
            if to_add == 0 {
                break;
            }
            let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
                continue;
            };
            if pg.class != *to_class {
                continue;
            }
            pg.size += to_add;
            to_add = 0;
        }

        if to_add > 0 {
            world.push_pop_group(hoi4_state::PopGroup {
                class: *to_class,
                state: first_state,
                size: to_add,
                employed_at: None,
                wage_rm: 0.0,
                tax_burden: 0.0,
                income_rm: 0.0,
                tax_paid_rm: 0.0,
                disposable_income_rm: 0.0,
                basic_consumption_budget: 0.0,
                satisfaction_law_modifier: 0.0,
                loyalty_coefficient: 1.0,
                loyalty_decay_mult: 1.0,
                satisfaction: 0.5,
                political_loyalty: 0.0,
                literacy: to_class.baseline_literacy(),
                skilled_ratio: to_class.baseline_skilled_ratio(),
                standard_of_living: 0.5,
                needs_fulfillment: 1.0,
                essential_needs_fulfillment: 1.0,
                normal_needs_fulfillment: 1.0,
                luxury_needs_fulfillment: 1.0,
                radicalism: 0.0,
            });
        }
    }
}

fn step_military_production_planned(
    world: &mut World,
    econ: &mut EconomyState,
    db: &V6Database,
    ci: usize,
    completed_techs: &[String],
) {
    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        if building.kind != BuildingKind::Military {
            continue;
        }
        if building.level == 0 {
            continue;
        }

        let pms = active_available_pms(building, db, ci, world, completed_techs)
            .into_iter()
            .filter(|pm| pm.equipment_output.is_some())
            .collect::<Vec<_>>();
        if pms.is_empty() {
            continue;
        }

        let employment_ratio = compute_employment_ratio(building, &pms);
        let qualification_ratio = {
            let qualification_totals = econ.qualification_totals(world);
            compute_qualification_ratio_from_totals(qualification_totals, building_idx, &pms)
        };
        let input_fulfillment_ratio = compute_input_fulfillment_ratio(
            &pms,
            building.level,
            employment_ratio,
            &world.countries.market.markets[ci].supply,
            &world.countries.market.markets[ci].stockpile,
        );
        let production_ratio = employment_ratio * input_fulfillment_ratio * qualification_ratio;

        for pm in &pms {
            let Some(eq_output) = &pm.equipment_output else {
                continue;
            };
            let daily_output = eq_output.daily_per_level
                * pm.throughput_modifier.max(0.0)
                * building.level as f32
                * production_ratio;
            let stockpile = &mut econ.stockpile[ci];
            let entry = stockpile
                .entry(super::stockpile::normalize_equipment_id(
                    &eq_output.equipment_category,
                ))
                .or_insert(0.0);
            *entry += daily_output;

            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                    * building.level as f32
                    * employment_ratio;
                let mkt = &mut world.countries.market.markets[ci];
                *mkt.demand.entry(good_id.clone()).or_insert(0.0) += amount;
                mkt.bucket_demand.add(
                    good_id,
                    hoi4_state::market::DemandBucketKind::MilitaryInput,
                    amount,
                );
            }
        }
    }
}

fn release_building_employment(world: &mut World, building_idx: usize) {
    let building_id = hoi4_state::BuildingId(building_idx as u32);
    if let Some(building) = world.countries.buildings_v6.buildings.get_mut(building_idx) {
        building.employment = [0; 6];
    }
    for pg in &mut world.countries.pops.groups {
        if pg.employed_at == Some(building_id) {
            pg.employed_at = None;
            pg.wage_rm = 0.0;
            pg.tax_burden = 0.0;
        }
    }
}

fn compute_total_employment_demand(world: &World, ci: usize, class: PopClass) -> u32 {
    let class_idx = class.index();
    let mut total: u32 = 0;
    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        total += building.employment.get(class_idx).copied().unwrap_or(0);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_state::BuildingOwner;

    #[test]
    fn employment_ratio_full_planned() {
        let building = hoi4_state::Building {
            kind: BuildingKind::Industrial,
            building_def_id: "steel_mill".to_owned(),
            state: hoi4_state::StateId(0),
            level: 1,
            active_pm: "default".to_owned(),
            employment: [0, 100, 20, 10, 0, 0],
            owner: BuildingOwner::State,
            requires_law: None,
            built_progress: 1.0,
            ..hoi4_state::Building::runtime_defaults()
        };
        let pm = ProductionMethodDef {
            id: "test".to_owned(),
            name: "test".to_owned(),
            building_id: "steel_mill".to_owned(),
            group: "base".to_owned(),
            group_name: "基础工艺".to_owned(),
            input_good_ids: vec![],
            input_good_amounts: vec![],
            output_good_ids: vec![],
            output_good_amounts: vec![],
            employment_demand: [0, 100, 20, 10, 0, 0],
            unlocked_by: None,
            required_law: None,
            throughput_modifier: 1.0,
            automation_modifier: 1.0,
            required_literacy: 0.0,
            required_skilled_ratio: 0.0,
            equipment_output: None,
        };
        let ratio = compute_employment_ratio(&building, &[&pm]);
        assert!((ratio - 1.0).abs() < 0.01);
    }

    #[test]
    fn employment_ratio_half_planned() {
        let building = hoi4_state::Building {
            kind: BuildingKind::Industrial,
            building_def_id: "steel_mill".to_owned(),
            state: hoi4_state::StateId(0),
            level: 1,
            active_pm: "default".to_owned(),
            employment: [0, 50, 10, 5, 0, 0],
            owner: BuildingOwner::State,
            requires_law: None,
            built_progress: 1.0,
            ..hoi4_state::Building::runtime_defaults()
        };
        let pm = ProductionMethodDef {
            id: "test".to_owned(),
            name: "test".to_owned(),
            building_id: "steel_mill".to_owned(),
            group: "base".to_owned(),
            group_name: "基础工艺".to_owned(),
            input_good_ids: vec![],
            input_good_amounts: vec![],
            output_good_ids: vec![],
            output_good_amounts: vec![],
            employment_demand: [0, 100, 20, 10, 0, 0],
            unlocked_by: None,
            required_law: None,
            throughput_modifier: 1.0,
            automation_modifier: 1.0,
            required_literacy: 0.0,
            required_skilled_ratio: 0.0,
            equipment_output: None,
        };
        let ratio = compute_employment_ratio(&building, &[&pm]);
        assert!((ratio - 0.5).abs() < 0.01);
    }
}
