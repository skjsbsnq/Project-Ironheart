//! V6 甯傚満 tick锛氬缓绛戜骇锟?锟?market.supply 锟?POP demand 锟?浠锋牸鍏紡锟?//!
//! 鏈ā鍧楀疄锟?`EconomicSystemTick` trait 鐨勫競鍦虹粡娴庝晶锟?//! D4 纭害鏉燂細鏈枃锟?**涓嶅彲** import `planned_tick` 鐨勪换浣曞唴閮ㄥ嚱鏁帮拷?
use hoi4_content::{PopNeedEntryDef, PopNeedTierDef, V6Database};
use hoi4_state::{BuildingKind, BuildingOwner, LawCategory, PopClass, World};

#[cfg(test)]
use super::building_tick_common::compute_qualification_ratio;
use super::building_tick_common::{
    active_available_pms, compute_employment_ratio, compute_input_fulfillment_ratio,
    compute_qualification_ratio_from_totals, country_building_indices, country_pop_indices,
    good_is_unlocked, pop_group_qualifies_for_class_job,
    state_owned_by_parts as state_is_owned_by_parts, state_owned_by_world as state_is_owned_by,
};
use super::econ_system_tick::EconomicSystemTick;
use super::finance_tick;
use super::EconomyState;
use crate::trade;

const PRICE_UPDATE_INTERVAL_DAYS: i64 = 7;
const LABOR_UPDATE_INTERVAL_DAYS: i64 = 7;
const CLASS_MOBILITY_INTERVAL_DAYS: i64 = 90;
const PRICE_EMA_FACTOR: f32 = 0.05;
const SATISFACTION_EMA_FACTOR: f32 = 0.02;

pub struct MarketTick;

impl EconomicSystemTick for MarketTick {
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

        step_building_production(world, econ, db, ci, &completed_techs);
        step_pop_employment(world, db, ci);
        econ.invalidate_qualification_totals();
        super::building_runtime::update(world, db, ci);
        step_pop_wage(world, db, ci);
        if day % PRICE_UPDATE_INTERVAL_DAYS == 0 {
            step_price_update(world, db, ci, &completed_techs);
        }

        if day % CLASS_MOBILITY_INTERVAL_DAYS == 0 {
            step_class_mobility(world, ci);
        }

        step_military_production(world, econ, db, ci, &completed_techs);
        step_military_government_procurement(world, econ, db, ci);

        finance_tick::step_collect_taxes(world, db, ci);
        step_pop_consumption_demand(world, db, ci);
        finance_tick::step_pay_wages_to_treasury(world, db, ci);
        finance_tick::step_pay_military_upkeep(world, econ, ci);
        finance_tick::step_construction_cost(world, db, ci);
        finance_tick::step_construction_project_funding(world, econ, ci);
        finance_tick::step_welfare_spending(world, db, ci);

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
        step_pop_radicalism(world, ci);
        step_pop_loyalty(world, db, ci);
    }
}

fn step_building_production(
    world: &mut World,
    econ: &mut EconomyState,
    db: &V6Database,
    ci: usize,
    completed_techs: &[String],
) {
    let country_id = hoi4_state::CountryId(ci as u16);
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

    let good_keys: std::collections::HashSet<String> = world.countries.market.markets[ci]
        .supply
        .keys()
        .chain(world.countries.market.markets[ci].demand.keys())
        .cloned()
        .collect();
    for good_key in &good_keys {
        if let Some(supply) = world.countries.market.markets[ci].supply.get_mut(good_key) {
            *supply = 0.0;
        }
        if let Some(demand) = world.countries.market.markets[ci].demand.get_mut(good_key) {
            *demand = 0.0;
        }
    }
    for import in world.countries.market.markets[ci].imports.values_mut() {
        *import = 0.0;
    }
    for export in world.countries.market.markets[ci].exports.values_mut() {
        *export = 0.0;
    }

    let buildings = &world.countries.buildings_v6.buildings;
    let building_indices = country_building_indices(world, ci);
    let mut supply_delta: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let mut demand_delta: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let qualification_totals = econ.qualification_totals(world);

    for building_idx in building_indices {
        let Some(building) = buildings.get(building_idx) else {
            continue;
        };
        if !state_is_owned_by(world, building.state, country_id) {
            continue;
        }
        if building.level == 0 {
            continue;
        }

        if let Some((cat, law_id)) = &building.requires_law {
            let current_law =
                &world.countries.law_store.law_sets[ci].0[map_law_category_internal(*cat)].current;
            if current_law != law_id {
                continue;
            }
        }

        let pms = active_available_pms(building, db, ci, world, completed_techs)
            .into_iter()
            .filter(|pm| pm.equipment_output.is_none())
            .collect::<Vec<_>>();
        if pms.is_empty() {
            continue;
        }

        let state_idx = building.state.0 as usize;
        let infra = if state_idx < world.states.count {
            world.states.infrastructure[state_idx] as f32
        } else {
            0.0
        };
        let infra_mult = 1.0 + 0.2 * infra;

        let employment_ratio = compute_employment_ratio(building, &pms);
        let input_fulfillment_ratio = compute_input_fulfillment_ratio(
            &pms,
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
            compute_qualification_ratio_from_totals(&qualification_totals, building_idx, &pms);

        let output_mult = production_employment_ratio
            * input_fulfillment_ratio
            * qualification_ratio
            * infra_mult;

        for pm in &pms {
            let throughput = pm.throughput_modifier.max(0.0);
            for (i, good_id) in pm.output_good_ids.iter().enumerate() {
                if locked_goods.contains(good_id.as_str()) {
                    continue;
                }
                let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                    * building.level as f32
                    * output_mult
                    * throughput;
                *supply_delta.entry(good_id.clone()).or_insert(0.0) += amount;
            }

            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                if locked_goods.contains(good_id.as_str()) {
                    continue;
                }
                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                    * building.level as f32
                    * employment_ratio;
                *demand_delta.entry(good_id.clone()).or_insert(0.0) += amount;
            }
        }
    }

    let market = &mut world.countries.market.markets[ci];
    for (good_id, amount) in supply_delta {
        *market.supply.entry(good_id).or_insert(0.0) += amount;
    }
    for (good_id, amount) in demand_delta {
        *market.demand.entry(good_id.clone()).or_insert(0.0) += amount;
        market.bucket_demand.add(
            &good_id,
            hoi4_state::market::DemandBucketKind::BuildingInput,
            amount,
        );
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
        .filter(|pg| state_is_owned_by_parts(state_count, state_owners, pg.state, country_id))
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
                && state_is_owned_by_parts(state_count, state_owners, pg.state, country_id)
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
                && state_is_owned_by_parts(state_count, state_owners, building.state, country_id)
        })
        .map(|building| building.level as u32)
        .sum();
    let university_capacity = university_levels as f32 * 250_000.0 / population as f32;
    let clerk_ratio = clerk_population as f32 / population as f32;
    let education_target =
        (0.35 + university_capacity * 0.60 + clerk_ratio * 1.30).clamp(0.05, 0.95);
    let skilled_target = (0.08 + university_capacity * 0.40 + clerk_ratio * 0.90).clamp(0.02, 0.80);

    for pg in &mut world.countries.pops.groups {
        if !state_is_owned_by_parts(state_count, state_owners, pg.state, country_id) {
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

fn step_pop_employment(world: &mut World, db: &V6Database, ci: usize) {
    let country_id = hoi4_state::CountryId(ci as u16);

    let completed_techs = world.countries.completed_techs[ci].clone();

    let building_indices: Vec<usize> = world
        .country_building_index
        .get(ci)
        .filter(|indices| !indices.is_empty())
        .cloned()
        .unwrap_or_else(|| (0..world.countries.buildings_v6.buildings.len()).collect());

    for building_idx in building_indices {
        {
            let building = &world.countries.buildings_v6.buildings[building_idx];
            if !state_is_owned_by(world, building.state, country_id) || building.level == 0 {
                continue;
            }
            if let Some((cat, law_id)) = &building.requires_law {
                let current_law = &world.countries.law_store.law_sets[ci].0
                    [map_law_category_internal(*cat)]
                .current;
                if current_law != law_id {
                    release_building_employment(world, building_idx);
                    continue;
                }
            }
        }

        let building_level = world.countries.buildings_v6.buildings[building_idx].level;

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
        let building_state = world.countries.buildings_v6.buildings[building_idx].state;
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
                                && state_is_owned_by(world, pg.state, country_id)
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
                        state_is_owned_by(world, pg.state, country_id)
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
    let building_wage_factors: Vec<f32> = world
        .countries
        .buildings_v6
        .buildings
        .iter()
        .map(|building| match building.owner {
            BuildingOwner::State => 1.0,
            BuildingOwner::Private | BuildingOwner::Cartel => {
                if building.estimated_profit_rm > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
        })
        .collect();

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
        if let Some(bid) = pg.employed_at {
            let factor = building_wage_factors
                .get(bid.0 as usize)
                .copied()
                .unwrap_or(0.0);
            pg.wage_rm = wage * factor;
        } else {
            pg.wage_rm = 0.0;
        }
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

fn step_pop_consumption_demand(world: &mut World, db: &V6Database, ci: usize) {
    let completed_techs = world.countries.completed_techs[ci].clone();

    let mut unlocked_needs: Vec<Vec<PopNeedEntryDef>> = Vec::with_capacity(PopClass::COUNT);
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

    let market_price = world.countries.market.markets[ci].price.clone();
    let integration_status = world.states.integration_status.clone();
    let mut demand_additions = Vec::new();

    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        let goods = unlocked_needs
            .get(pg.class.index())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
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

        let is_soldier = pg.class == PopClass::Soldier;
        let is_unemployed = pg.employed_at.is_none() && !is_soldier;
        let non_basic_mult = if is_soldier {
            0.8
        } else if is_unemployed {
            0.3
        } else {
            1.0
        };

        let mut basic_budget_rm = 0.0_f32;
        let mut non_basic_budget_rm = 0.0_f32;

        for need in goods {
            let base_demand = pop_millions * need.amount_per_million;
            let price = market_price.get(&need.good_id).copied().unwrap_or(1.0);
            let cost_rm = base_demand * price;
            match need.tier {
                PopNeedTierDef::Essential | PopNeedTierDef::Normal => {
                    basic_budget_rm += cost_rm;
                }
                PopNeedTierDef::Luxury => {
                    non_basic_budget_rm += cost_rm * non_basic_mult;
                }
            }
        }

        let total_budget_rm = basic_budget_rm + non_basic_budget_rm;
        let budget_cap = pg.disposable_income_rm * pg.size as f32;
        let budget_mult = if total_budget_rm > 0.0 && budget_cap > 0.0 {
            (budget_cap / total_budget_rm).min(1.0)
        } else if budget_cap <= 0.0 && total_budget_rm > 0.0 {
            if is_soldier || is_unemployed {
                0.5
            } else {
                0.2
            }
        } else {
            1.0
        };

        pg.basic_consumption_budget = basic_budget_rm * budget_mult;

        for need in goods {
            let mut demand = pop_millions * need.amount_per_million;
            let bucket_kind = match need.tier {
                PopNeedTierDef::Essential => {
                    demand *= budget_mult.max(0.1);
                    hoi4_state::market::DemandBucketKind::PopBasicConsumption
                }
                PopNeedTierDef::Normal => {
                    demand *= budget_mult.max(0.1);
                    hoi4_state::market::DemandBucketKind::PopBasicConsumption
                }
                PopNeedTierDef::Luxury => {
                    demand *= non_basic_mult * budget_mult;
                    hoi4_state::market::DemandBucketKind::PopNonBasicConsumption
                }
            };
            if demand > 0.0 {
                demand_additions.push((need.good_id.clone(), bucket_kind, demand));
            }
        }
    }

    let market = &mut world.countries.market.markets[ci];
    for (good_id, bucket_kind, demand) in demand_additions {
        *market.demand.entry(good_id.clone()).or_insert(0.0) += demand;
        market.bucket_demand.add(&good_id, bucket_kind, demand);
    }
}

/// 清算后计算 POP 满足度：从 clearing_sheet 读取每个商品的满足率
fn step_pop_satisfaction_from_clearing(world: &mut World, db: &V6Database, ci: usize) {
    let completed_techs = world.countries.completed_techs[ci].clone();

    let mut unlocked_needs: Vec<Vec<PopNeedEntryDef>> = Vec::with_capacity(PopClass::COUNT);
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
    }

    step_pop_satisfaction(world, db, ci);
}

/// 从清算表读取商品满足率
fn clearing_fulfillment(sheet: &hoi4_state::market::MarketClearingSheet, good_id: &str) -> f32 {
    if let Some(result) = sheet.results.get(good_id) {
        if result.total_fulfilled + result.total_unmet > 0.0 {
            result.total_fulfilled / (result.total_fulfilled + result.total_unmet)
        } else {
            1.0
        }
    } else {
        let market_default = 1.0;
        market_default
    }
}

fn step_pop_satisfaction(world: &mut World, db: &V6Database, ci: usize) {
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

    for pop_idx in pop_indices {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        let essential_needs_fulfillment = pg.essential_needs_fulfillment.clamp(0.0, 1.0);
        let normal_needs_fulfillment = pg.normal_needs_fulfillment.clamp(0.0, 1.0);
        let luxury_needs_fulfillment = pg.luxury_needs_fulfillment.clamp(0.0, 1.0);
        let standard_of_living = pg.standard_of_living.clamp(0.0, 1.0);
        let needs_fulfillment = pg.needs_fulfillment.clamp(0.0, 1.0);

        let employment_security = if pg.class == PopClass::Soldier {
            1.0
        } else {
            (1.0 - unemployment_rate).clamp(0.0, 1.0)
        };
        let law_effect = (1.0 + law_modifier + pg.satisfaction_law_modifier).clamp(0.0, 1.0);
        let tax_burden = pg.tax_burden.clamp(0.0, 1.0);
        let income_ratio = if pg.income_rm > 0.0 {
            (pg.disposable_income_rm / pg.income_rm).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let satisfaction_target = (0.35 * essential_needs_fulfillment
            + 0.20 * standard_of_living
            + 0.15 * employment_security
            + 0.10 * income_ratio
            + 0.05 * (1.0 - tax_burden)
            + 0.10 * law_effect
            + 0.03 * normal_needs_fulfillment
            + 0.02 * luxury_needs_fulfillment
            + 0.05 * needs_fulfillment)
            .clamp(0.0, 1.0);
        let class_target = satisfaction_target;

        pg.satisfaction +=
            SATISFACTION_EMA_FACTOR * (class_target.clamp(0.0, 1.0) - pg.satisfaction);
        pg.satisfaction = pg.satisfaction.clamp(0.0, 1.0);
        pg.standard_of_living =
            (0.75 * pg.standard_of_living + 0.25 * pg.needs_fulfillment).clamp(0.0, 1.0);
    }
}

fn step_pop_radicalism(world: &mut World, ci: usize) {
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
        let essential_pressure = ((0.8 - pg.essential_needs_fulfillment) / 0.8).clamp(0.0, 1.0);
        let unemployment_pressure = if pg.class == PopClass::Soldier {
            0.0
        } else {
            ((unemployment_rate - 0.1) / 0.4).clamp(0.0, 1.0)
        };
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

fn step_price_update(world: &mut World, db: &V6Database, ci: usize, completed_techs: &[String]) {
    let market = &mut world.countries.market.markets[ci];

    for good in &db.goods {
        if let Some(unlock_tech) = &good.unlocked_by {
            if !completed_techs.contains(unlock_tech) {
                *market.supply.entry(good.id.clone()).or_insert(0.0) = 0.0;
                *market.demand.entry(good.id.clone()).or_insert(0.0) = 0.0;
                *market
                    .price
                    .entry(good.id.clone())
                    .or_insert(good.base_price_rm) = good.base_price_rm;
                continue;
            }
        }

        let raw_supply = market.supply.get(&good.id).copied().unwrap_or(0.0);
        let raw_demand = market.demand.get(&good.id).copied().unwrap_or(0.0);
        if raw_supply <= 0.0 && raw_demand <= 0.0 {
            market
                .price
                .entry(good.id.clone())
                .or_insert(good.base_price_rm);
            continue;
        }
        let stockpile = market
            .stockpile
            .get(&good.id)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let coverage_days = market
            .stockpile_coverage_days
            .get(&good.id)
            .copied()
            .unwrap_or_else(|| {
                if raw_demand > 0.0 {
                    stockpile / raw_demand
                } else {
                    0.0
                }
            });
        let unmet = market.unmet_demand.get(&good.id).copied().unwrap_or(0.0);
        let supply = (raw_supply + stockpile.min(raw_demand.max(0.0) * 0.25)).max(0.01);
        let demand = raw_demand.max(0.01);
        let ratio = demand / supply;
        let stockpile_pressure = if coverage_days <= 3.0 {
            1.35
        } else if coverage_days <= 10.0 {
            1.15
        } else if coverage_days >= 25.0 {
            0.90
        } else {
            1.0
        };
        let shortage_pressure = if raw_demand > 0.0 {
            1.0 + (unmet / raw_demand).clamp(0.0, 1.0) * 0.75
        } else {
            1.0
        };
        let target_price =
            good.base_price_rm * ratio.clamp(0.25, 4.0) * stockpile_pressure * shortage_pressure;

        let current_price = market
            .price
            .get(&good.id)
            .copied()
            .unwrap_or(good.base_price_rm);
        let new_price = current_price + PRICE_EMA_FACTOR * (target_price - current_price);
        market.price.insert(good.id.clone(), new_price.max(0.01));
    }
}

fn step_military_government_procurement(
    world: &mut World,
    econ: &EconomyState,
    db: &V6Database,
    ci: usize,
) {
    let economy_law = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
        .current
        .clone();
    let procurement_mult = military_procurement_multiplier(world, ci, &economy_law);
    if procurement_mult <= 0.0 {
        return;
    }

    let is_mefo_funded = economy_law == "corporatist_war_economy"
        && !world.countries.treasury.treasuries[ci].mefo_disabled
        && world.countries.treasury.treasuries[ci].mefo_debt_rm > 0.0;
    if is_mefo_funded {
        let mefo_room = if world.countries.treasury.treasuries[ci].gdp_rm > 0.0 {
            (world.countries.treasury.treasuries[ci].gdp_rm * 0.30
                - world.countries.treasury.treasuries[ci].mefo_debt_rm)
                .max(0.0)
        } else {
            0.0
        };
        world.countries.treasury.treasuries[ci].mefo_military_budget_rm = mefo_room * 0.01;
    }

    let completed_techs: Vec<String> = world.countries.completed_techs[ci].clone();
    let procurement_scale = (procurement_mult / 10.0).clamp(0.25, 3.0);
    let mut procurement_demand: Vec<(String, f32)> = Vec::new();

    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        if building.kind != BuildingKind::Military || building.level == 0 {
            continue;
        }

        let pms = active_available_pms(building, db, ci, world, &completed_techs)
            .into_iter()
            .filter(|pm| pm.equipment_output.is_some())
            .collect::<Vec<_>>();
        for pm in pms {
            if let Some(eq_output) = &pm.equipment_output {
                if let Some(good_id) = equipment_procurement_good(db, &eq_output.equipment_category)
                {
                    let amount = eq_output.daily_per_level
                        * pm.throughput_modifier.max(0.0)
                        * building.level as f32
                        * procurement_scale;
                    if amount > 0.0 {
                        procurement_demand.push((good_id, amount));
                    }
                }
            }
            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                    * building.level as f32
                    * procurement_scale;
                if amount > 0.0 {
                    procurement_demand.push((good_id.clone(), amount));
                }
            }
        }
    }
    for order in econ.government_orders.get(ci).into_iter().flatten() {
        if let Some(pm) = db.production_methods.iter().find(|pm| {
            pm.equipment_output
                .as_ref()
                .map(|out| {
                    super::stockpile::normalize_equipment_id(&out.equipment_category)
                        == order.equipment_category
                })
                .unwrap_or(false)
        }) {
            let budget_mult = (order.daily_budget_rm / 100_000.0).clamp(1.0, 25.0) as f32;
            if let Some(eq_output) = &pm.equipment_output {
                if let Some(good_id) = equipment_procurement_good(db, &eq_output.equipment_category)
                {
                    let amount =
                        eq_output.daily_per_level * pm.throughput_modifier.max(0.0) * budget_mult;
                    if amount > 0.0 {
                        procurement_demand.push((good_id, amount));
                    }
                }
            }
            for (i, good_id) in pm.input_good_ids.iter().enumerate() {
                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0) * budget_mult;
                if amount > 0.0 {
                    procurement_demand.push((good_id.clone(), amount));
                }
            }
        }
    }

    for (good_id, qty) in procurement_demand {
        let market = &mut world.countries.market.markets[ci];
        *market.demand.entry(good_id.clone()).or_insert(0.0) += qty;
        market.bucket_demand.add(
            &good_id,
            hoi4_state::market::DemandBucketKind::GovernmentProcurement,
            qty,
        );
    }
}

fn military_procurement_multiplier(world: &World, ci: usize, economy_law: &str) -> f32 {
    let base = match economy_law {
        "corporatist_war_economy" => 8.0,
        "war_economy" => 3.0,
        "interventionism" => 1.0,
        _ => 0.0,
    };
    if base <= 0.0 {
        return 0.0;
    }

    let country_id = hoi4_state::CountryId(ci as u16);
    let division_count = if world.runtime_country_indexes_valid {
        world
            .country_division_index
            .get(ci)
            .map(|divisions| divisions.len())
            .unwrap_or(0)
    } else {
        world
            .divisions
            .owners
            .iter()
            .filter(|owner| **owner == country_id)
            .count()
    } as f32;
    let army_target_mult = 1.0 + division_count * 0.05;

    if economy_law != "corporatist_war_economy" {
        return base * army_target_mult;
    }

    let treasury = &world.countries.treasury.treasuries[ci];
    let mefo_room_ratio = if treasury.gdp_rm > 0.0 {
        ((treasury.gdp_rm * 0.30 - treasury.mefo_debt_rm).max(0.0) / treasury.gdp_rm) as f32
    } else {
        0.30
    };
    let mefo_credit_mult = 1.0 + mefo_room_ratio * 10.0;
    base * army_target_mult * mefo_credit_mult
}

fn equipment_procurement_good(db: &V6Database, equipment_category: &str) -> Option<String> {
    let normalized = super::stockpile::normalize_equipment_id(equipment_category);
    let candidate = match normalized.as_str() {
        "infantry_equipment" => "small_arms",
        "support_equipment" => "support_equipment",
        "artillery" => "artillery_shells",
        "anti_tank" => "anti_tank_guns",
        "anti_air" => "anti_air_guns",
        "motorized" | "mechanized" => "vehicles",
        "armor" => "tanks",
        "aircraft" => "aircraft_parts",
        "naval_vessel" => "ship_components",
        "train" => "locomotives",
        "convoy" => "ship_components",
        other => other,
    };
    db.goods
        .iter()
        .any(|good| good.id == candidate)
        .then(|| candidate.to_owned())
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
                wage_rm: average_wage_for_class(world, ci, *to_class),
                tax_burden: average_tax_burden_for_class(world, ci, *to_class),
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

fn average_wage_for_class(world: &World, ci: usize, class: PopClass) -> f32 {
    let mut weighted = 0.0;
    let mut total = 0u32;
    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get(pop_idx) else {
            continue;
        };
        if pg.class != class {
            continue;
        }
        weighted += pg.wage_rm * pg.size as f32;
        total += pg.size;
    }
    if total > 0 {
        weighted / total as f32
    } else {
        0.0
    }
}

fn average_tax_burden_for_class(world: &World, ci: usize, class: PopClass) -> f32 {
    let mut weighted = 0.0;
    let mut total = 0u32;
    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get(pop_idx) else {
            continue;
        };
        if pg.class != class {
            continue;
        }
        weighted += pg.tax_burden * pg.size as f32;
        total += pg.size;
    }
    if total > 0 {
        weighted / total as f32
    } else {
        0.0
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

fn step_military_production(
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
                if locked_goods.contains(good_id.as_str()) {
                    continue;
                }
                let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                    * building.level as f32
                    * employment_ratio;
                *world.countries.market.markets[ci]
                    .demand
                    .entry(good_id.clone())
                    .or_insert(0.0) += amount;
                world.countries.market.markets[ci].bucket_demand.add(
                    good_id,
                    hoi4_state::market::DemandBucketKind::MilitaryInput,
                    amount,
                );
            }
        }
    }
}

fn map_law_category_internal(cat: LawCategory) -> usize {
    cat.index()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_content::ProductionMethodDef;
    use std::sync::Arc;

    fn empty_world() -> World {
        let mut data = hoi4_data::GameData::default();
        let tag = hoi4_data::CountryTag::new("TST");
        data.countries.insert(
            tag.clone(),
            hoi4_data::Country {
                tag: tag.clone(),
                color: hoi4_data::Color {
                    r: 80,
                    g: 80,
                    b: 80,
                },
                graphical_culture: "test_gfx".to_owned(),
                capital: 1,
                ruling_party: "neutrality".to_owned(),
                technologies: vec![],
            },
        );
        data.states.push(hoi4_data::State {
            id: 1,
            name: "Test State".to_owned(),
            manpower: 1_000_000,
            owner: tag.clone(),
            cores: vec![tag],
            provinces: vec![],
            category: "city".to_owned(),
            infrastructure: 0,
            victory_points: vec![],
            resources: vec![],
        });
        World::new(
            Arc::new(hoi4_map::GameMap {
                definitions: vec![],
                rgb_to_id: std::collections::HashMap::new(),
                province_map: hoi4_map::ProvinceMap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                adjacencies: vec![],
                special_adjacencies: vec![],
                heightmap: hoi4_map::Heightmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                },
                terrain_bmp: hoi4_map::TerrainBitmap {
                    width: 1,
                    height: 1,
                    pixels: vec![0],
                    palette: [[0; 3]; 256],
                },
                terrain_catalog: hoi4_map::TerrainCatalog::default(),
                tree_definition_bmp: None,
                tree_indices: std::collections::HashSet::new(),
            }),
            Arc::new(data),
        )
    }

    #[test]
    fn employment_ratio_full() {
        let building = hoi4_state::Building {
            kind: BuildingKind::Industrial,
            building_def_id: "steel_mill".to_owned(),
            state: hoi4_state::StateId(0),
            level: 1,
            active_pm: "default".to_owned(),
            employment: [0, 100, 20, 10, 0, 0],
            owner: BuildingOwner::Private,
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
    fn employment_ratio_half() {
        let building = hoi4_state::Building {
            kind: BuildingKind::Industrial,
            building_def_id: "steel_mill".to_owned(),
            state: hoi4_state::StateId(0),
            level: 1,
            active_pm: "default".to_owned(),
            employment: [0, 50, 10, 5, 0, 0],
            owner: BuildingOwner::Private,
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

    #[test]
    fn low_qualification_reduces_advanced_industry_output() {
        let mut world = empty_world();
        world
            .countries
            .buildings_v6
            .buildings
            .push(hoi4_state::Building {
                kind: BuildingKind::Industrial,
                building_def_id: "machine_tool_works".to_owned(),
                state: hoi4_state::StateId(0),
                level: 1,
                active_pm: "machine_tool_works_default".to_owned(),
                employment: [0, 100, 0, 0, 0, 0],
                owner: BuildingOwner::State,
                requires_law: None,
                built_progress: 1.0,
                ..hoi4_state::Building::runtime_defaults()
            });
        let building_id = hoi4_state::BuildingId(0);
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Worker,
            state: hoi4_state::StateId(0),
            size: 100,
            employed_at: Some(building_id),
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
            literacy: 0.20,
            skilled_ratio: 0.05,
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
        let pm = ProductionMethodDef {
            id: "machine_tool_works_default".to_owned(),
            name: "Machine Tools".to_owned(),
            building_id: "machine_tool_works".to_owned(),
            group: "base".to_owned(),
            group_name: "基础工艺".to_owned(),
            input_good_ids: vec![],
            input_good_amounts: vec![],
            output_good_ids: vec!["machine_tools".to_owned()],
            output_good_amounts: vec![10.0],
            employment_demand: [0, 100, 0, 0, 0, 0],
            unlocked_by: None,
            required_law: None,
            throughput_modifier: 1.0,
            automation_modifier: 1.0,
            required_literacy: 0.60,
            required_skilled_ratio: 0.40,
            equipment_output: None,
        };

        let low_ratio =
            compute_qualification_ratio(&world, &world.countries.buildings_v6.buildings[0], &[&pm]);
        world.countries.pops.groups[0].literacy = 0.80;
        world.countries.pops.groups[0].skilled_ratio = 0.50;
        let high_ratio =
            compute_qualification_ratio(&world, &world.countries.buildings_v6.buildings[0], &[&pm]);

        assert!(low_ratio < high_ratio, "low={low_ratio} high={high_ratio}");
        assert!(low_ratio <= 0.35 + f32::EPSILON, "low={low_ratio}");
        assert!((high_ratio - 1.0).abs() < 0.01, "high={high_ratio}");
    }

    #[test]
    fn high_skill_jobs_skip_unqualified_pop_groups() {
        let mut world = empty_world();
        world
            .countries
            .buildings_v6
            .buildings
            .push(hoi4_state::Building {
                kind: BuildingKind::Industrial,
                building_def_id: "advanced_plant".to_owned(),
                state: hoi4_state::StateId(0),
                level: 1,
                active_pm: "advanced_plant_default".to_owned(),
                employment: [0; 6],
                owner: BuildingOwner::Private,
                requires_law: None,
                built_progress: 1.0,
                ..hoi4_state::Building::runtime_defaults()
            });
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Worker,
            state: hoi4_state::StateId(0),
            size: 100,
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
            literacy: 0.20,
            skilled_ratio: 0.05,
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
        let mut db = V6Database::default();
        db.production_methods.push(ProductionMethodDef {
            id: "advanced_plant_default".to_owned(),
            name: "Advanced Plant".to_owned(),
            building_id: "advanced_plant".to_owned(),
            group: "base".to_owned(),
            group_name: "Base".to_owned(),
            input_good_ids: vec![],
            input_good_amounts: vec![],
            output_good_ids: vec![],
            output_good_amounts: vec![],
            employment_demand: [0, 100, 0, 0, 0, 0],
            unlocked_by: None,
            required_law: None,
            throughput_modifier: 1.0,
            automation_modifier: 1.0,
            required_literacy: 0.60,
            required_skilled_ratio: 0.40,
            equipment_output: None,
        });

        step_pop_employment(&mut world, &db, 0);
        assert_eq!(world.countries.buildings_v6.buildings[0].employment[1], 0);
        assert!(world
            .countries
            .pops
            .groups
            .iter()
            .all(|pg| pg.employed_at.is_none()));

        world.countries.pops.groups[0].literacy = 0.80;
        world.countries.pops.groups[0].skilled_ratio = 0.50;
        step_pop_employment(&mut world, &db, 0);

        assert_eq!(world.countries.buildings_v6.buildings[0].employment[1], 50);
        assert!(world
            .countries
            .pops
            .groups
            .iter()
            .any(|pg| pg.employed_at == Some(hoi4_state::BuildingId(0))));
    }

    #[test]
    fn shortage_clearing_reduces_pop_satisfaction() {
        let mut world = empty_world();
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Worker,
            state: hoi4_state::StateId(0),
            size: 100,
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
            satisfaction: 0.8,
            political_loyalty: 0.0,
            literacy: 0.55,
            skilled_ratio: 0.18,
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
        world.countries.market.markets[0]
            .clearing_sheet
            .results
            .insert(
                "grain".to_owned(),
                hoi4_state::market::GoodClearingResult {
                    total_fulfilled: 0.0,
                    total_unmet: 100.0,
                    shortage_ratio: 1.0,
                    ..Default::default()
                },
            );
        let mut db = V6Database::default();
        db.pop_needs.push(hoi4_content::PopClassNeedsDef {
            class: PopClass::Worker,
            needs: vec![hoi4_content::PopNeedEntryDef {
                good_id: "grain".to_owned(),
                tier: hoi4_content::PopNeedTierDef::Essential,
                amount_per_million: 1.0,
            }],
        });

        step_pop_satisfaction_from_clearing(&mut world, &db, 0);

        let worker = &world.countries.pops.groups[0];
        assert_eq!(worker.essential_needs_fulfillment, 0.0);
        assert!(
            worker.satisfaction < 0.8,
            "satisfaction should move down when essential goods are unmet"
        );
    }

    #[test]
    fn university_and_clerks_raise_qualifications() {
        let mut world = empty_world();
        world
            .countries
            .buildings_v6
            .buildings
            .push(hoi4_state::Building {
                kind: BuildingKind::Service,
                building_def_id: "university".to_owned(),
                state: hoi4_state::StateId(0),
                level: 5,
                active_pm: "university_default".to_owned(),
                employment: [0; 6],
                owner: BuildingOwner::State,
                requires_law: None,
                built_progress: 1.0,
                ..hoi4_state::Building::runtime_defaults()
            });
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Worker,
            state: hoi4_state::StateId(0),
            size: 900,
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
            literacy: 0.30,
            skilled_ratio: 0.05,
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });
        world.countries.pops.groups.push(hoi4_state::PopGroup {
            class: PopClass::Clerk,
            state: hoi4_state::StateId(0),
            size: 100,
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
            literacy: 0.40,
            skilled_ratio: 0.10,
            standard_of_living: 0.5,
            needs_fulfillment: 1.0,
            essential_needs_fulfillment: 1.0,
            normal_needs_fulfillment: 1.0,
            luxury_needs_fulfillment: 1.0,
            radicalism: 0.0,
        });

        let before_literacy = world.countries.pops.groups[0].literacy;
        let before_skilled = world.countries.pops.groups[0].skilled_ratio;
        step_pop_education(&mut world, &V6Database::default(), 0);

        assert!(world.countries.pops.groups[0].literacy > before_literacy);
        assert!(world.countries.pops.groups[0].skilled_ratio > before_skilled);
    }
}
