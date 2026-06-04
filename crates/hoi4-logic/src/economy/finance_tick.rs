//! V6 财政 tick：税收 → Treasury 收入 / 支出 / 利息 / MEFO / 汇率 / GDP。
//!
//! 本模块被 `market_tick` 和 `planned_tick` 共同调用（经 `EconomicSystemTick` trait），
//! 但自身不直接实现 trait，避免违反 HC-3。

use hoi4_content::{
    v6_loader::{BuildingGdpComponentDef, BuildingGdpRuleDef},
    V6Database,
};
use hoi4_state::{
    Building, BuildingOwner, CountryId, InvestmentAccountKind, LawCategory, OwnershipAccount,
    PopClass, StateIntegrationStatus, World,
};

use super::building_tick_common::{country_building_indices, country_pop_indices};
use crate::EconomyState;

const GDP_WEEKLY_INTERVAL: i64 = 7;

pub fn reset_daily_accumulators(world: &mut World, ci: usize) {
    world.countries.treasury.treasuries[ci].reset_daily_accumulators();
}

pub fn step_collect_taxes_planned(world: &mut World, _db: &V6Database, ci: usize) {
    let country_id = CountryId(ci as u16);
    let integration_status = world.states.integration_status.clone();

    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        if pg.class == PopClass::Soldier {
            continue;
        }
        pg.tax_burden = 0.0;
        pg.tax_paid_rm = 0.0;
        pg.disposable_income_rm = pg.income_rm;
    }

    let mut total_state_profit: f64 = 0.0;
    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        if building.level == 0 {
            continue;
        }
        let profit_estimate = building.profit_rm
            * integration_tax_factor(integration_status[building.state.0 as usize]) as f64
            * crate::occupation::state_governance_yield_factor(world, building.state) as f64;
        for share in ownership_shares_for(building, country_id) {
            if let OwnershipAccount::State { country } = share.account {
                if country == country_id {
                    total_state_profit += profit_estimate * share.share as f64;
                }
            }
        }
    }

    let treasury = &mut world.countries.treasury.treasuries[ci];
    treasury.receive(total_state_profit, "state_profit");
}

pub fn step_collect_taxes(world: &mut World, _db: &V6Database, ci: usize) {
    let country_id = CountryId(ci as u16);
    let state_count = world.states.count;
    let integration_status = world.states.integration_status.clone();
    let governance_factors: Vec<f32> = (0..state_count)
        .map(|si| {
            crate::occupation::state_governance_yield_factor(world, hoi4_state::StateId(si as u16))
        })
        .collect();

    let [income_tax_rate, consumption_tax_rate, corporate_tax_rate] =
        world.countries.treasury.treasuries[ci].tax_rates;

    let mut total_income_tax: f64 = 0.0;
    let mut total_consumption_tax: f64 = 0.0;
    let pop_indices = country_pop_indices(world, ci);

    for &pop_idx in &pop_indices {
        let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
            continue;
        };
        if pg.class == PopClass::Soldier {
            continue;
        }
        let state_idx = pg.state.0 as usize;
        let governance_factor = governance_factors.get(state_idx).copied().unwrap_or(1.0);
        let tax_factor = integration_status
            .get(state_idx)
            .copied()
            .map(integration_tax_factor)
            .unwrap_or(1.0) as f64
            * governance_factor as f64;
        let wage_total = pg.wage_rm as f64 * pg.size as f64 * tax_factor;
        total_income_tax += wage_total * income_tax_rate as f64;

        let consumption_weight = match pg.class {
            PopClass::Peasant => 0.6,
            PopClass::Worker => 0.8,
            PopClass::Clerk => 1.0,
            PopClass::Capitalist => 2.0,
            PopClass::Aristocrat => 1.5,
            PopClass::Soldier => 0.5,
        };
        total_consumption_tax += wage_total * consumption_weight * consumption_tax_rate as f64;
        let class_income_mult = match pg.class {
            PopClass::Peasant => 0.75_f32,
            PopClass::Worker => 1.0_f32,
            PopClass::Clerk => 1.1_f32,
            PopClass::Capitalist => 1.8_f32,
            PopClass::Aristocrat => 1.5_f32,
            PopClass::Soldier => 0.0_f32,
        };
        pg.tax_burden = ((income_tax_rate * class_income_mult
            + consumption_tax_rate * consumption_weight as f32)
            * tax_factor as f32)
            .clamp(0.0, 1.0);

        let income_tax_per_capita = pg.wage_rm * income_tax_rate * tax_factor as f32;
        let consumption_tax_per_capita =
            pg.wage_rm * consumption_weight as f32 * consumption_tax_rate * tax_factor as f32;
        pg.tax_paid_rm = (income_tax_per_capita + consumption_tax_per_capita)
            .min(pg.income_rm)
            .max(0.0);
        pg.disposable_income_rm = (pg.income_rm - pg.tax_paid_rm).max(0.0);
    }

    let mut total_corporate_tax: f64 = 0.0;
    let mut total_state_profit_share: f64 = 0.0;
    let mut cartel_capitalist_income: f64 = 0.0;
    let mut private_retained_profit: f64 = 0.0;
    let mut cartel_retained_profit: f64 = 0.0;
    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        if building.level == 0 {
            continue;
        }
        let profit_estimate = building.profit_rm
            * integration_tax_factor(integration_status[building.state.0 as usize]) as f64
            * crate::occupation::state_governance_yield_factor(world, building.state) as f64;
        for share in ownership_shares_for(building, country_id) {
            let amount = profit_estimate * share.share as f64;
            match share.account {
                OwnershipAccount::State { country } if country == country_id => {
                    total_state_profit_share += amount;
                }
                OwnershipAccount::DomesticPrivate { country } if country == country_id => {
                    total_corporate_tax += amount * corporate_tax_rate as f64;
                    private_retained_profit += amount * (1.0 - corporate_tax_rate as f64) * 0.03;
                }
                OwnershipAccount::Cartel { country } if country == country_id => {
                    total_corporate_tax += amount * corporate_tax_rate as f64;
                    cartel_capitalist_income += amount;
                    cartel_retained_profit += amount * (1.0 - corporate_tax_rate as f64) * 0.02;
                }
                OwnershipAccount::ForeignPrivate { .. } | OwnershipAccount::Overlord { .. } => {
                    total_corporate_tax += amount * corporate_tax_rate as f64;
                }
                _ => {}
            }
        }
    }

    if cartel_capitalist_income > 0.0 {
        let total_capitalists: u32 = pop_indices
            .iter()
            .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
            .filter(|pg| pg.class == PopClass::Capitalist)
            .map(|pg| pg.size)
            .sum();
        if total_capitalists > 0 {
            let per_capita = (cartel_capitalist_income / total_capitalists as f64) as f32;
            for &pop_idx in &pop_indices {
                let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
                    continue;
                };
                if pg.class != PopClass::Capitalist {
                    continue;
                }
                pg.wage_rm += per_capita;
                pg.income_rm += per_capita;
                pg.disposable_income_rm += per_capita;
            }
        }
    }

    let treasury = &mut world.countries.treasury.treasuries[ci];
    let daily_tax_income =
        total_income_tax + total_consumption_tax + total_corporate_tax + total_state_profit_share;
    treasury.receive(daily_tax_income, "taxes");
    treasury.daily_budget.income_pop_taxes_rm += total_income_tax.max(0.0);
    treasury.daily_budget.income_consumption_taxes_rm += total_consumption_tax.max(0.0);
    treasury.daily_budget.income_corporate_taxes_rm += total_corporate_tax.max(0.0);
    record_investment_income(
        world,
        country_id,
        InvestmentAccountKind::Private,
        private_retained_profit.max(0.0),
    );
    record_investment_income(
        world,
        country_id,
        InvestmentAccountKind::Cartel,
        cartel_retained_profit.max(0.0),
    );
    if let Some(pool) = world.countries.private_investment_pool_rm.get_mut(ci) {
        *pool += private_retained_profit.max(0.0);
    }
}

fn ownership_shares_for(
    building: &Building,
    fallback_country: CountryId,
) -> Vec<hoi4_state::OwnershipShare> {
    if building.ownership_shares.is_empty() {
        Building::default_ownership_shares(building.owner, fallback_country)
    } else {
        building.ownership_shares.clone()
    }
}

fn record_investment_income(
    world: &mut World,
    country: CountryId,
    account_kind: InvestmentAccountKind,
    amount_rm: f64,
) {
    if amount_rm <= 0.0 {
        return;
    }
    if let Some(account) = world
        .countries
        .investment_account_mut(country, account_kind)
    {
        account.balance_rm += amount_rm;
        account.last_income_rm += amount_rm;
    }
}

pub fn step_pay_wages_to_treasury(world: &mut World, db: &V6Database, ci: usize) {
    let mut total_wage_cost: f64 = 0.0;
    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        if building.level == 0 {
            continue;
        }
        if building.owner != BuildingOwner::State {
            continue;
        }

        let _pm = match db
            .production_methods
            .iter()
            .find(|pm| pm.building_id == building.building_def_id && pm.id.ends_with("default"))
        {
            Some(pm) => pm,
            None => continue,
        };
        let base_wage: [f32; 6] = [1.5, 3.0, 5.0, 15.0, 25.0, 2.5];
        for class_idx in 0..6 {
            let employed = building.employment.get(class_idx).copied().unwrap_or(0) as f64;
            let wage = base_wage.get(class_idx).copied().unwrap_or(3.0) as f64;
            total_wage_cost += employed * wage;
        }
    }

    world.countries.treasury.treasuries[ci].pay(total_wage_cost, "state_payroll");
}

pub fn step_pay_military_upkeep(world: &mut World, econ: &mut EconomyState, ci: usize) {
    let country_id = CountryId(ci as u16);
    let data = world.data.clone();
    let demand = crate::military::collect_military_demand(world, data.as_ref(), country_id);

    let military_wages = demand.manpower_needed as f64 * 4.0;
    let mefo_funding = !world.countries.treasury.treasuries[ci].mefo_disabled
        && world.countries.treasury.treasuries[ci].mefo_debt_rm > 0.0;
    let funding = if mefo_funding {
        super::GovernmentOrderFundingSource::Mefo
    } else {
        super::GovernmentOrderFundingSource::Treasury
    };
    create_procurement_orders(econ, ci, country_id, &demand, funding);
    let fuel_and_training = demand.fuel_needed as f64 * 10.0
        + demand
            .training_goods
            .values()
            .map(|qty| *qty as f64 * 25.0)
            .sum::<f64>();

    let treasury = &mut world.countries.treasury.treasuries[ci];
    treasury.pay(military_wages, "military_wages");
    treasury.pay(demand.maintenance_rm, "military_maintenance");
    treasury.pay(fuel_and_training, "military_training");
    econ.tick_government_orders(ci);

    let pop_indices = country_pop_indices(world, ci);
    let total_soldier_pop: u32 = pop_indices
        .iter()
        .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
        .filter(|pg| pg.class == PopClass::Soldier)
        .map(|pg| pg.size)
        .sum();

    if total_soldier_pop > 0 {
        let per_capita_wage = (military_wages / total_soldier_pop as f64) as f32;
        for pop_idx in pop_indices {
            let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
                continue;
            };
            if pg.class != PopClass::Soldier {
                continue;
            }
            pg.income_rm = per_capita_wage;
            pg.wage_rm = per_capita_wage;
            pg.tax_paid_rm = 0.0;
            pg.disposable_income_rm = pg.income_rm;
        }
    }
}

fn create_procurement_orders(
    econ: &mut EconomyState,
    ci: usize,
    country_id: CountryId,
    demand: &crate::military::MilitaryDemand,
    funding: super::GovernmentOrderFundingSource,
) {
    let stockpile = econ.stockpile.get(ci).cloned().unwrap_or_default();
    let equipment_need: Vec<(String, f32)> = demand
        .equipment_needed
        .iter()
        .map(|(equipment, needed)| (equipment.clone(), *needed))
        .collect();
    for (equipment, needed) in equipment_need {
        let have = stockpile.get(&equipment).copied().unwrap_or(0.0);
        let missing = (needed - have).max(0.0);
        if missing <= 0.0 {
            continue;
        }
        let order_budget =
            missing as f64 * crate::military::equipment_unit_cost_rm(&equipment) / 30.0;
        econ.upsert_government_order_with_funding(
            country_id,
            &equipment,
            order_budget,
            30,
            funding,
        );
    }
}

pub fn step_pay_interest(world: &mut World, ci: usize) {
    world.countries.treasury.treasuries[ci].pay_interest();
}

pub fn step_construction_cost(world: &mut World, _db: &V6Database, ci: usize) {
    let mut total_cp: f32 = 0.0;
    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        if building.building_def_id != "construction_sector" {
            continue;
        }
        total_cp += building.level as f32;
    }

    if total_cp <= 0.0 {
        return;
    }

    let machinery_qty = total_cp * 0.05;
    if machinery_qty > 0.0 {
        let market = &mut world.countries.market.markets[ci];
        *market.demand.entry("machinery".to_owned()).or_insert(0.0) += machinery_qty;
        market.bucket_demand.add(
            "machinery",
            hoi4_state::market::DemandBucketKind::ConstructionInput,
            machinery_qty,
        );
    }

    let wage_cost = total_cp * 0.10 * 3.0;
    world.countries.treasury.treasuries[ci].pay(wage_cost as f64, "construction_wages");
}

pub fn step_construction_project_funding(
    _world: &mut World,
    _econ: &mut crate::EconomyState,
    _ci: usize,
) {
    // G09 construction runtime allocates and funds every active project inside
    // `construction_tick::run`; this legacy hook remains for older tick order callers.
}

fn building_gdp_rule(db: &V6Database, building_def_id: &str) -> BuildingGdpRuleDef {
    db.buildings
        .iter()
        .find(|def| def.id == building_def_id)
        .map(|def| def.gdp_rule)
        .unwrap_or(BuildingGdpRuleDef {
            component: BuildingGdpComponentDef::SecondaryOutput,
            value_added_multiplier: 1.0,
        })
}

pub fn step_update_gdp(world: &mut World, db: &V6Database, ci: usize, day: i64) {
    let mut building_primary_rm: f64 = 0.0;
    let mut building_secondary_rm: f64 = 0.0;
    let mut building_tertiary_rm: f64 = 0.0;
    let mut colonial_building_value_added_rm: f64 = 0.0;
    for building_idx in country_building_indices(world, ci) {
        let Some(building) = world.countries.buildings_v6.buildings.get(building_idx) else {
            continue;
        };
        let state_idx = building.state.0 as usize;
        if building.level == 0 {
            continue;
        }

        let rule = building_gdp_rule(db, &building.building_def_id);
        let value = building.value_added_rm.max(0.0) * rule.value_added_multiplier.max(0.0) as f64;
        if world.states.integration_status[state_idx].is_domestic() {
            match rule.component {
                BuildingGdpComponentDef::PrimaryOutput => building_primary_rm += value,
                BuildingGdpComponentDef::SecondaryOutput
                | BuildingGdpComponentDef::MilitaryProcurement => building_secondary_rm += value,
                BuildingGdpComponentDef::TertiaryOutput
                | BuildingGdpComponentDef::GovernmentService
                | BuildingGdpComponentDef::InfrastructureService => building_tertiary_rm += value,
            }
        } else {
            colonial_building_value_added_rm += value
                * crate::occupation::state_governance_yield_factor(world, building.state) as f64;
        }
    }

    let mut pop_income_rm: f64 = 0.0;
    let mut pop_consumption_rm: f64 = 0.0;
    for pop_idx in country_pop_indices(world, ci) {
        let Some(pg) = world.countries.pops.groups.get(pop_idx) else {
            continue;
        };
        let state_idx = pg.state.0 as usize;
        let integration = world
            .states
            .integration_status
            .get(state_idx)
            .copied()
            .unwrap_or_default();
        let governance = if integration.is_domestic() {
            1.0
        } else {
            crate::occupation::state_governance_yield_factor(world, pg.state) as f64
        };
        let size = pg.size as f64;
        pop_income_rm += pg.income_rm.max(0.0) as f64 * size * governance;
        pop_consumption_rm += pg.basic_consumption_budget.max(0.0) as f64 * size * governance;
    }

    let treasury_snapshot = &world.countries.treasury.treasuries[ci];
    let government_spending_rm = treasury_snapshot.daily_budget.expense_state_payroll_rm
        + treasury_snapshot.daily_budget.expense_construction_wages_rm
        + treasury_snapshot.daily_budget.expense_welfare_rm;
    let military_procurement_rm = treasury_snapshot
        .daily_budget
        .expense_military_procurement_rm;
    let net_exports_rm = treasury_snapshot.daily_trade_balance_gbp
        * world.countries.treasury.exchange_rates[ci].rm_per_gbp as f64;

    let rm_per_gbp = world.countries.treasury.exchange_rates[ci].rm_per_gbp;
    let treasury = &mut world.countries.treasury.treasuries[ci];
    treasury.gdp_breakdown.building_primary_rm = building_primary_rm * 365.0;
    treasury.gdp_breakdown.building_secondary_rm = building_secondary_rm * 365.0;
    treasury.gdp_breakdown.building_tertiary_rm = building_tertiary_rm * 365.0;
    treasury.gdp_breakdown.pop_income_rm = pop_income_rm * 365.0;
    treasury.gdp_breakdown.pop_consumption_rm = pop_consumption_rm * 365.0;
    treasury.gdp_breakdown.government_services_rm = government_spending_rm.max(0.0) * 365.0;
    treasury.gdp_breakdown.military_procurement_rm = military_procurement_rm.max(0.0) * 365.0;
    treasury.gdp_breakdown.net_exports_rm = net_exports_rm * 365.0;
    treasury.gdp_breakdown.colonial_value_added_rm =
        colonial_building_value_added_rm.max(0.0) * 365.0;

    treasury.gdp_rm = treasury.gdp_breakdown.runtime_total_rm();
    treasury.gdp_gbp = treasury.gdp_rm / rm_per_gbp as f64;
    treasury.colonial_gdp_rm = treasury.gdp_breakdown.colonial_value_added_rm;
    treasury.domestic_gdp_rm = (treasury.gdp_rm - treasury.colonial_gdp_rm).max(0.0);
    treasury.colonial_extracted_value_rm = treasury.colonial_gdp_rm * 0.35;
    treasury.domestic_gdp_gbp = treasury.domestic_gdp_rm / rm_per_gbp as f64;
    treasury.colonial_gdp_gbp = treasury.colonial_gdp_rm / rm_per_gbp as f64;
    treasury.colonial_extracted_value_gbp =
        treasury.colonial_extracted_value_rm / rm_per_gbp as f64;

    if treasury.gdp_last_year_gbp <= 0.0 {
        treasury.gdp_last_year_gbp = treasury.gdp_gbp;
        treasury.gdp_growth_yoy = 0.0;
    } else {
        treasury.gdp_growth_yoy = (((treasury.gdp_gbp / treasury.gdp_last_year_gbp) - 1.0) * 100.0)
            .clamp(-15.0, 15.0) as f32;
    }
    if treasury.gdp_breakdown.historical_validation_gbp > 0.0 {
        treasury.gdp_breakdown.historical_validation_error_ratio = (treasury.gdp_gbp
            - treasury.gdp_breakdown.historical_validation_gbp)
            / treasury.gdp_breakdown.historical_validation_gbp;
    }
    if day > 0 && day % 365 == 0 {
        treasury.gdp_last_year_gbp = treasury.gdp_gbp;
    }
    treasury.update_credit_rating(rm_per_gbp);
}

pub fn step_update_exchange_rate(world: &mut World, db: &V6Database, ci: usize, day: i64) {
    if day % GDP_WEEKLY_INTERVAL != 0 {
        return;
    }

    let current_trade = world.countries.law_store.law_sets[ci].0[LawCategory::Trade.index()]
        .current
        .clone();
    let trade_law_modifier = db
        .trade_laws
        .iter()
        .find(|l| l.id == current_trade)
        .map(|l| l.trade_law_modifier)
        .unwrap_or(1.0);

    let treasury = &world.countries.treasury.treasuries[ci];
    let exchange = &mut world.countries.treasury.exchange_rates[ci];
    exchange.weekly_update(
        treasury.gold_kg,
        treasury.public_debt_gbp,
        treasury.gdp_gbp,
        trade_law_modifier,
    );
}

pub fn step_check_mefo_crisis(world: &mut World, db: &V6Database, ci: usize, day: i64) -> bool {
    // MEFO bills are designed as hidden short-term rearmament financing. They
    // should accumulate through the 1936-1938 build-up and only mature at the
    // 1938/1939 crisis gate, not explode immediately when early GDP estimates
    // are still warming up.
    if day < 1095 {
        return false;
    }
    if !world.countries.treasury.treasuries[ci].check_mefo_crisis_at(db.mefo.crisis_threshold_ratio)
    {
        return false;
    }
    let mut fired = Vec::new();
    crate::economy::v6_events::fire_event(world, db, ci, "mefo_crisis", &mut fired)
}

pub fn step_auto_mefo(world: &mut World, db: &V6Database, ci: usize) -> f64 {
    if !can_print_mefo(world, db, ci) {
        return 0.0;
    }
    let deficit = {
        let treasury = &world.countries.treasury.treasuries[ci];
        treasury.operating_expense_rm - treasury.operating_income_rm
    };
    if deficit <= 0.0 {
        return 0.0;
    }
    world.countries.treasury.treasuries[ci].print_mefo(deficit)
}

pub fn step_compute_fiscal_summary(world: &mut World, ci: usize) {
    world.countries.treasury.treasuries[ci].compute_fiscal_summary();
}

pub fn step_check_mark_crisis(world: &mut World, ci: usize, previous_rate: f32) -> bool {
    let exchange = &world.countries.treasury.exchange_rates[ci];
    exchange.is_crisis(previous_rate)
}

pub fn step_welfare_spending(world: &mut World, db: &V6Database, ci: usize) {
    let current_civil_rights = world.countries.law_store.law_sets[ci].0
        [LawCategory::CivilRights.index()]
    .current
    .clone();
    let welfare_rate = db
        .civil_rights_laws
        .iter()
        .find(|l| l.id == current_civil_rights)
        .map(|l| l.welfare_rate)
        .unwrap_or(0.0);

    if welfare_rate <= 0.0 {
        return;
    }

    let country_id = CountryId(ci as u16);
    let pop_indices = country_pop_indices(world, ci);

    let total_pop: f64 = pop_indices
        .iter()
        .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
        .filter_map(|pg| {
            let owner_ci = pg.state.0 as usize;
            if owner_ci >= world.states.count || world.states.owners[owner_ci] != country_id {
                return None;
            }
            Some(
                pg.size as f64
                    * integration_tax_factor(world.states.integration_status[owner_ci]) as f64
                    * crate::occupation::state_governance_yield_factor(world, pg.state) as f64,
            )
        })
        .sum();

    let welfare_cost = total_pop * welfare_rate as f64;
    world.countries.treasury.treasuries[ci].pay(welfare_cost, "welfare");

    let unemployed_pop: u32 = pop_indices
        .iter()
        .filter_map(|&pop_idx| world.countries.pops.groups.get(pop_idx))
        .filter(|pg| pg.class != PopClass::Soldier && pg.employed_at.is_none())
        .map(|pg| pg.size)
        .sum();

    if unemployed_pop > 0 && welfare_cost > 0.0 {
        let per_capita_welfare = (welfare_cost / unemployed_pop as f64) as f32;
        for pop_idx in pop_indices {
            let Some(pg) = world.countries.pops.groups.get_mut(pop_idx) else {
                continue;
            };
            if pg.class == PopClass::Soldier || pg.employed_at.is_some() {
                continue;
            }
            pg.income_rm += per_capita_welfare;
            pg.disposable_income_rm = pg.income_rm - pg.tax_paid_rm;
        }
    }
}

pub fn integration_tax_factor(status: StateIntegrationStatus) -> f32 {
    match status {
        StateIntegrationStatus::Metropole => 1.00,
        StateIntegrationStatus::Incorporated => 0.85,
        StateIntegrationStatus::Colony => 0.35,
        StateIntegrationStatus::Protectorate | StateIntegrationStatus::Mandate => 0.20,
        StateIntegrationStatus::Concession => 0.25,
        StateIntegrationStatus::Occupied => 0.10,
    }
}

pub fn integration_consumption_factor(status: StateIntegrationStatus) -> f32 {
    match status {
        StateIntegrationStatus::Metropole => 1.00,
        StateIntegrationStatus::Incorporated => 0.85,
        StateIntegrationStatus::Colony => 0.45,
        StateIntegrationStatus::Protectorate | StateIntegrationStatus::Mandate => 0.30,
        StateIntegrationStatus::Concession => 0.35,
        StateIntegrationStatus::Occupied => 0.20,
    }
}

/// §4.6.3：计划经济每日把所有商品价格钉回 `base_price_rm`，确保计划国家不随市场国家供需漂价格。
/// 由 `planned_tick.rs` 调用，不进入 market 路径。
pub fn step_planned_price_lock(world: &mut World, db: &V6Database, ci: usize) {
    if ci >= world.countries.market.markets.len() {
        return;
    }
    let market = &mut world.countries.market.markets[ci];
    for good in &db.goods {
        market.price.insert(good.id.clone(), good.base_price_rm);
    }
}

/// 获取外汇管制状态
pub fn is_foreign_exchange_control(world: &World, db: &V6Database, ci: usize) -> bool {
    let current_trade = world.countries.law_store.law_sets[ci].0[LawCategory::Trade.index()]
        .current
        .clone();
    db.trade_laws
        .iter()
        .find(|l| l.id == current_trade)
        .map(|l| l.foreign_exchange_control)
        .unwrap_or(false)
}

/// 判断 MEFO 是否可印（需 Conscription ≥ 有限征兵 + Economy ≥ 干预经济）
pub fn can_print_mefo(world: &World, db: &V6Database, ci: usize) -> bool {
    if world.countries.tags.get(ci).map(|t| t.as_str()) != Some("GER") {
        return false;
    }
    if world.countries.treasury.treasuries[ci].mefo_disabled {
        return false;
    }
    let conscription = world.countries.law_store.law_sets[ci].0[LawCategory::Conscription.index()]
        .current
        .clone();
    let economy = world.countries.law_store.law_sets[ci].0[LawCategory::Economy.index()]
        .current
        .clone();

    let conscription_ok = hoi4_content::v6_loader::law_at_least(
        conscription.as_str(),
        &db.mefo.unlock_conscription_min,
        &[
            "volunteer_only",
            "limited_conscription",
            "extensive_conscription",
            "total_mobilization",
        ],
    );
    let economy_ok = hoi4_content::v6_loader::law_at_least(
        economy.as_str(),
        &db.mefo.unlock_economy_min,
        &[
            "laissez_faire",
            "interventionism",
            "war_economy",
            "corporatist_war_economy",
            "planned_economy",
        ],
    );
    conscription_ok && economy_ok
}

#[cfg(test)]
mod tests {
    use hoi4_state::finance::{CreditRating, ExchangeRate, Treasury};

    #[test]
    fn gov_buy_deducts_cash() {
        let mut treasury = Treasury::default();
        treasury.cash_rm = 1000.0;
        let mut market = hoi4_state::market::NationalMarket::default();
        market.price.insert("steel".to_owned(), 10.0);
        market.supply.insert("steel".to_owned(), 100.0);
        market.demand.insert("steel".to_owned(), 0.0);

        let actual = treasury.gov_buy(&mut market, "steel", 5.0);
        assert!((actual - 5.0).abs() < 0.01);
        assert!((treasury.cash_rm - 950.0).abs() < 0.01);
    }

    #[test]
    fn gov_buy_caps_by_cash() {
        let mut treasury = Treasury::default();
        treasury.cash_rm = 30.0;
        let mut market = hoi4_state::market::NationalMarket::default();
        market.price.insert("steel".to_owned(), 10.0);
        market.supply.insert("steel".to_owned(), 100.0);
        market.demand.insert("steel".to_owned(), 0.0);

        let actual = treasury.gov_buy(&mut market, "steel", 5.0);
        assert!((actual - 3.0).abs() < 0.01);
    }

    #[test]
    fn gov_buy_caps_by_supply() {
        let mut treasury = Treasury::default();
        treasury.cash_rm = 10000.0;
        let mut market = hoi4_state::market::NationalMarket::default();
        market.price.insert("steel".to_owned(), 10.0);
        market.supply.insert("steel".to_owned(), 50.0);
        market.demand.insert("steel".to_owned(), 45.0);

        let actual = treasury.gov_buy(&mut market, "steel", 10.0);
        assert!((actual - 5.0).abs() < 0.01);
    }

    #[test]
    fn credit_rating_from_debt_ratio() {
        assert_eq!(CreditRating::from_debt_ratio(0.1), CreditRating::AAA);
        assert_eq!(CreditRating::from_debt_ratio(0.4), CreditRating::AA);
        assert_eq!(CreditRating::from_debt_ratio(0.7), CreditRating::A);
        assert_eq!(CreditRating::from_debt_ratio(1.0), CreditRating::BBB);
        assert_eq!(CreditRating::from_debt_ratio(1.5), CreditRating::BB);
        assert_eq!(CreditRating::from_debt_ratio(5.0), CreditRating::D);
    }

    #[test]
    fn foreign_bond_requires_bbb() {
        let mut t = Treasury::default();
        t.credit_rating = CreditRating::BBB;
        assert!(t.issue_foreign_bond(100.0).is_ok());
        t.credit_rating = CreditRating::BB;
        assert!(t.issue_foreign_bond(100.0).is_err());
    }

    #[test]
    fn mefo_crisis_threshold() {
        let mut t = Treasury::default();
        t.gdp_rm = 100.0;
        t.mefo_debt_rm = 20.0;
        assert!(!t.check_mefo_crisis());
        t.mefo_debt_rm = 35.0;
        assert!(t.check_mefo_crisis());
    }

    #[test]
    fn exchange_rate_weekly_update() {
        let mut er = ExchangeRate { rm_per_gbp: 12.5 };
        er.weekly_update(700_000.0, 0.0, 6_640_000_000.0, 1.0);
        assert!(er.rm_per_gbp < 12.5, "gold should strengthen RM");
    }

    #[test]
    fn mefo_print_capped_by_deficit() {
        let mut t = Treasury::default();
        t.operating_income_rm = 100.0;
        t.operating_expense_rm = 150.0;
        t.daily_income_rm = 100.0;
        t.daily_expense_rm = 150.0;
        let printed = t.print_mefo(100.0);
        assert!(
            (printed - 50.0).abs() < 0.01,
            "should only print deficit amount"
        );
    }

    #[test]
    fn sell_gold_adds_reserve() {
        let mut t = Treasury::default();
        t.gold_kg = 1000.0;
        let gbp = t.sell_gold(500.0);
        assert!((gbp - 420.0).abs() < 0.01);
        assert!((t.gold_kg - 500.0).abs() < 0.01);
    }
}
