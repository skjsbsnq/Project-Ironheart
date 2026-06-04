use hoi4_logic::economy::{ConstructionFundingSource, EconomyState};
use hoi4_state::{CountryId, InvestmentAccountKind, World};

use crate::ui_data::names::{DisplayNameKind, DisplayNameResolver};

pub fn build_finance_panel_data(
    world: &World,
    econ: &EconomyState,
    v6_db: &hoi4_content::V6Database,
    player: usize,
) -> Option<hoi4_ui::finance_panel::FinancePanelData> {
    let treasury = world.countries.treasury.treasuries.get(player)?;
    let exchange_rate = world
        .countries
        .treasury
        .exchange_rates
        .get(player)
        .map(|er| er.rm_per_gbp)
        .unwrap_or(12.5);
    let can_print_mefo = hoi4_logic::economy::finance_tick::can_print_mefo(world, v6_db, player);
    let is_forex_control =
        hoi4_logic::economy::finance_tick::is_foreign_exchange_control(world, v6_db, player);
    let construction_funding = build_construction_funding_trace(econ, player);
    let investment_pool = build_investment_pool_data(world, player);
    let sector_buildings = build_sector_building_entries(world, v6_db, player);
    let employment_rows = build_employment_rows(&sector_buildings);
    let diagnostics = build_economy_diagnostics(
        treasury,
        &construction_funding,
        &investment_pool,
        &sector_buildings,
        &employment_rows,
    );

    Some(build_finance_panel_data_from_parts(
        treasury,
        exchange_rate,
        can_print_mefo,
        is_forex_control,
        construction_funding,
        investment_pool,
        sector_buildings,
        employment_rows,
        diagnostics,
    ))
}

fn credit_rating_label(rating: hoi4_state::CreditRating) -> &'static str {
    match rating {
        hoi4_state::CreditRating::AAA => "AAA",
        hoi4_state::CreditRating::AA => "AA",
        hoi4_state::CreditRating::A => "A",
        hoi4_state::CreditRating::BBB => "BBB",
        hoi4_state::CreditRating::BB => "BB",
        hoi4_state::CreditRating::B => "B",
        hoi4_state::CreditRating::CCC => "CCC",
        hoi4_state::CreditRating::D => "D",
    }
}

fn build_finance_panel_data_from_parts(
    treasury: &hoi4_state::Treasury,
    exchange_rate: f32,
    can_print_mefo: bool,
    is_foreign_exchange_control: bool,
    construction_funding: hoi4_ui::finance_panel::ConstructionFundingTraceData,
    investment_pool: hoi4_ui::finance_panel::InvestmentPoolData,
    sector_buildings: Vec<hoi4_ui::finance_panel::EconomySectorBuildingEntry>,
    employment_rows: Vec<hoi4_ui::finance_panel::EconomyEmploymentEntry>,
    diagnostics: Vec<hoi4_ui::finance_panel::EconomyDiagnosticEntry>,
) -> hoi4_ui::finance_panel::FinancePanelData {
    let credit_rating = credit_rating_label(treasury.credit_rating);
    let fiscal_revenue = build_fiscal_revenue(treasury);
    let fiscal_expense = build_fiscal_expense(treasury);

    hoi4_ui::finance_panel::FinancePanelData {
        cash_rm: treasury.cash_rm,
        reserve_gbp: treasury.reserve_gbp,
        gold_kg: treasury.gold_kg,
        daily_income_rm: treasury.daily_income_rm,
        daily_expense_rm: treasury.daily_expense_rm,
        budget_breakdown: hoi4_ui::finance_panel::BudgetBreakdownData {
            income_taxes_rm: treasury.daily_budget.income_taxes_rm,
            income_pop_taxes_rm: treasury.daily_budget.income_pop_taxes_rm,
            income_consumption_taxes_rm: treasury.daily_budget.income_consumption_taxes_rm,
            income_corporate_taxes_rm: treasury.daily_budget.income_corporate_taxes_rm,
            income_trade_tariffs_rm: treasury.daily_budget.income_trade_tariffs_rm,
            income_state_profit_rm: treasury.daily_budget.income_state_profit_rm,
            income_domestic_bonds_rm: treasury.daily_budget.income_domestic_bonds_rm,
            income_other_rm: treasury.daily_budget.income_other_rm,
            expense_state_payroll_rm: treasury.daily_budget.expense_state_payroll_rm,
            expense_military_wages_rm: treasury.daily_budget.expense_military_wages_rm,
            expense_military_procurement_rm: treasury.daily_budget.expense_military_procurement_rm,
            expense_military_maintenance_rm: treasury.daily_budget.expense_military_maintenance_rm,
            expense_construction_goods_rm: treasury.daily_budget.expense_construction_goods_rm,
            expense_construction_wages_rm: treasury.daily_budget.expense_construction_wages_rm,
            expense_welfare_rm: treasury.daily_budget.expense_welfare_rm,
            expense_debt_interest_rm: treasury.daily_budget.expense_debt_interest_rm,
            expense_foreign_currency_rm: treasury.daily_budget.expense_foreign_currency_rm,
            expense_mefo_forced_payment_rm: treasury.daily_budget.expense_mefo_forced_payment_rm,
            expense_research_rm: treasury.daily_budget.expense_research_rm,
            expense_other_rm: treasury.daily_budget.expense_other_rm,
        },
        financing_breakdown: hoi4_ui::finance_panel::FinancingBreakdownData {
            mefo_issued_rm: treasury.daily_financing.mefo_issued_rm,
            mefo_interest_capitalized_rm: treasury.daily_financing.mefo_interest_capitalized_rm,
            domestic_bond_issued_rm: treasury.daily_financing.domestic_bond_issued_rm,
        },
        fiscal_revenue,
        fiscal_expense,
        construction_funding,
        investment_pool,
        operating_income_rm: treasury.operating_income_rm,
        operating_expense_rm: treasury.operating_expense_rm,
        original_deficit_rm: treasury.original_deficit_rm,
        mefo_coverage_rm: treasury.mefo_coverage_rm,
        post_financing_cash_change_rm: treasury.post_financing_cash_change_rm,
        public_debt_gbp: treasury.public_debt_gbp,
        public_debt_rm: treasury.public_debt_rm,
        mefo_debt_rm: treasury.mefo_debt_rm,
        mefo_military_budget_rm: treasury.mefo_military_budget_rm,
        mefo_military_spent_rm: treasury.mefo_military_spent_rm,
        credit_rating: credit_rating.to_owned(),
        bond_interest_rate: treasury.bond_interest_rate,
        gdp_rm: treasury.gdp_rm,
        gdp_gbp: treasury.gdp_gbp,
        domestic_gdp_rm: treasury.domestic_gdp_rm,
        domestic_gdp_gbp: treasury.domestic_gdp_gbp,
        colonial_gdp_rm: treasury.colonial_gdp_rm,
        colonial_gdp_gbp: treasury.colonial_gdp_gbp,
        colonial_extracted_value_rm: treasury.colonial_extracted_value_rm,
        colonial_extracted_value_gbp: treasury.colonial_extracted_value_gbp,
        gdp_breakdown: hoi4_ui::finance_panel::GdpBreakdownData {
            building_primary_rm: treasury.gdp_breakdown.building_primary_rm,
            building_secondary_rm: treasury.gdp_breakdown.building_secondary_rm,
            building_tertiary_rm: treasury.gdp_breakdown.building_tertiary_rm,
            pop_income_rm: treasury.gdp_breakdown.pop_income_rm,
            pop_consumption_rm: treasury.gdp_breakdown.pop_consumption_rm,
            government_services_rm: treasury.gdp_breakdown.government_services_rm,
            military_procurement_rm: treasury.gdp_breakdown.military_procurement_rm,
            net_exports_rm: treasury.gdp_breakdown.net_exports_rm,
            colonial_value_added_rm: treasury.gdp_breakdown.colonial_value_added_rm,
            historical_validation_gbp: treasury.gdp_breakdown.historical_validation_gbp,
            historical_validation_error_ratio: treasury
                .gdp_breakdown
                .historical_validation_error_ratio,
        },
        sector_buildings,
        employment_rows,
        diagnostics,
        exchange_rate_rm_per_gbp: exchange_rate,
        can_print_mefo,
        can_issue_foreign_bond: treasury.credit_rating.can_issue_foreign_bonds(),
        is_foreign_exchange_control,
    }
}

fn build_sector_building_entries(
    world: &World,
    v6_db: &hoi4_content::V6Database,
    player: usize,
) -> Vec<hoi4_ui::finance_panel::EconomySectorBuildingEntry> {
    let country = CountryId(player as u16);
    let name_resolver = DisplayNameResolver::new(None);
    let mut rows = Vec::new();
    for building in &world.countries.buildings_v6.buildings {
        let state_idx = building.state.0 as usize;
        if state_idx >= world.states.count
            || world.states.owners[state_idx] != country
            || building.level == 0
        {
            continue;
        }

        let def = v6_db
            .buildings
            .iter()
            .find(|def| def.id == building.building_def_id);
        let sector = def
            .map(|def| def.economic_sector)
            .unwrap_or(hoi4_content::v6_loader::EconomicSectorDef::Secondary);
        let (sector_id, sector_name) = economy_sector_label(sector);
        let building_name = def
            .map(|def| name_resolver.content_name(DisplayNameKind::Building, &def.id, &def.name))
            .unwrap_or_else(|| {
                name_resolver.content_name(
                    DisplayNameKind::Building,
                    &building.building_def_id,
                    &building.building_def_id,
                )
            });
        let demand = building_employment_demand(building, v6_db);
        let employed: u32 = building.employment.iter().copied().sum();
        rows.push(hoi4_ui::finance_panel::EconomySectorBuildingEntry {
            sector_id: sector_id.to_owned(),
            sector_name: sector_name.to_owned(),
            building_name,
            level: building.level,
            employed,
            demand,
            employment_rate: ratio_u32(employed, demand),
            value_added_rm: building.value_added_rm.max(0.0),
        });
    }
    rows.sort_by(|a, b| {
        sector_sort_key(&a.sector_id)
            .cmp(&sector_sort_key(&b.sector_id))
            .then_with(|| b.value_added_rm.total_cmp(&a.value_added_rm))
            .then_with(|| a.building_name.cmp(&b.building_name))
    });
    rows
}

fn build_employment_rows(
    sector_buildings: &[hoi4_ui::finance_panel::EconomySectorBuildingEntry],
) -> Vec<hoi4_ui::finance_panel::EconomyEmploymentEntry> {
    let sector_specs = [
        ("一产", "primary"),
        ("二产", "secondary"),
        ("三产", "tertiary"),
        ("政府服务", "government"),
        ("军工支援", "military_support"),
        ("基础设施", "infrastructure"),
    ];
    let mut rows: Vec<_> = sector_specs
        .iter()
        .map(|(label, sector_id)| {
            let employed = sector_buildings
                .iter()
                .filter(|row| row.sector_id == *sector_id)
                .map(|row| row.employed)
                .sum();
            let demand = sector_buildings
                .iter()
                .filter(|row| row.sector_id == *sector_id)
                .map(|row| row.demand)
                .sum();
            let value_added_rm = sector_buildings
                .iter()
                .filter(|row| row.sector_id == *sector_id)
                .map(|row| row.value_added_rm)
                .sum();
            hoi4_ui::finance_panel::EconomyEmploymentEntry {
                label: (*label).to_owned(),
                employed,
                demand,
                employment_rate: ratio_u32(employed, demand),
                value_added_rm,
            }
        })
        .collect();
    let total_employed = rows.iter().map(|row| row.employed).sum();
    let total_demand = rows.iter().map(|row| row.demand).sum();
    let total_value_added = rows.iter().map(|row| row.value_added_rm).sum();
    rows.push(hoi4_ui::finance_panel::EconomyEmploymentEntry {
        label: "全部建筑".to_owned(),
        employed: total_employed,
        demand: total_demand,
        employment_rate: ratio_u32(total_employed, total_demand),
        value_added_rm: total_value_added,
    });
    rows
}

fn build_economy_diagnostics(
    treasury: &hoi4_state::Treasury,
    construction_funding: &hoi4_ui::finance_panel::ConstructionFundingTraceData,
    investment_pool: &hoi4_ui::finance_panel::InvestmentPoolData,
    sector_buildings: &[hoi4_ui::finance_panel::EconomySectorBuildingEntry],
    employment_rows: &[hoi4_ui::finance_panel::EconomyEmploymentEntry],
) -> Vec<hoi4_ui::finance_panel::EconomyDiagnosticEntry> {
    let daily_balance = treasury.daily_income_rm - treasury.daily_expense_rm;
    let debt_ratio = if treasury.gdp_rm > 0.0 {
        (treasury.public_debt_rm + treasury.mefo_debt_rm) / treasury.gdp_rm
    } else {
        0.0
    };
    let mefo_ratio = if treasury.gdp_rm > 0.0 {
        treasury.mefo_debt_rm / treasury.gdp_rm
    } else {
        0.0
    };
    let total_employment_rate = employment_rows
        .iter()
        .find(|row| row.label == "全部建筑")
        .map(|row| row.employment_rate)
        .unwrap_or(0.0);
    let underfilled_buildings = sector_buildings
        .iter()
        .filter(|row| row.demand > row.employed)
        .count();

    vec![
        hoi4_ui::finance_panel::EconomyDiagnosticEntry {
            source: "财政现金流".to_owned(),
            status: if daily_balance >= 0.0 {
                "正常"
            } else {
                "赤字"
            }
            .to_owned(),
            detail: format!("日净额 {}", signed_amount(daily_balance)),
        },
        hoi4_ui::finance_panel::EconomyDiagnosticEntry {
            source: "债务压力".to_owned(),
            status: if debt_ratio > 0.60 {
                "高风险"
            } else if debt_ratio > 0.40 {
                "关注"
            } else {
                "正常"
            }
            .to_owned(),
            detail: format!("债务/GDP {:.1}%", debt_ratio * 100.0),
        },
        hoi4_ui::finance_panel::EconomyDiagnosticEntry {
            source: "MEFO 暴露".to_owned(),
            status: if mefo_ratio > 0.30 {
                "高风险"
            } else if mefo_ratio > 0.20 {
                "关注"
            } else {
                "正常"
            }
            .to_owned(),
            detail: format!("MEFO/GDP {:.1}%", mefo_ratio * 100.0),
        },
        hoi4_ui::finance_panel::EconomyDiagnosticEntry {
            source: "就业吸收".to_owned(),
            status: if total_employment_rate < 0.75 {
                "短缺"
            } else if total_employment_rate < 0.92 {
                "关注"
            } else {
                "正常"
            }
            .to_owned(),
            detail: format!(
                "建筑就业率 {:.1}%，缺员建筑 {}",
                total_employment_rate * 100.0,
                underfilled_buildings
            ),
        },
        hoi4_ui::finance_panel::EconomyDiagnosticEntry {
            source: "建设资金".to_owned(),
            status: if construction_funding.remaining_rm > construction_funding.paid_rm.max(1.0) {
                "排队"
            } else {
                "正常"
            }
            .to_owned(),
            detail: format!(
                "项目 {}，待支付 {:.0} RM",
                construction_funding.active_projects, construction_funding.remaining_rm
            ),
        },
        hoi4_ui::finance_panel::EconomyDiagnosticEntry {
            source: "投资池".to_owned(),
            status: if investment_pool.total_rm <= 0.0 {
                "枯竭"
            } else {
                "可用"
            }
            .to_owned(),
            detail: format!(
                "余额 {:.0} RM，本日流入 {:.0} RM",
                investment_pool.total_rm, investment_pool.income_rm
            ),
        },
    ]
}

fn economy_sector_label(
    sector: hoi4_content::v6_loader::EconomicSectorDef,
) -> (&'static str, &'static str) {
    match sector {
        hoi4_content::v6_loader::EconomicSectorDef::Primary => ("primary", "一产"),
        hoi4_content::v6_loader::EconomicSectorDef::Secondary => ("secondary", "二产"),
        hoi4_content::v6_loader::EconomicSectorDef::Tertiary => ("tertiary", "三产"),
        hoi4_content::v6_loader::EconomicSectorDef::Government => ("government", "政府服务"),
        hoi4_content::v6_loader::EconomicSectorDef::MilitarySupport => {
            ("military_support", "军工支援")
        }
        hoi4_content::v6_loader::EconomicSectorDef::Infrastructure => {
            ("infrastructure", "基础设施")
        }
    }
}

fn sector_sort_key(sector_id: &str) -> u8 {
    match sector_id {
        "primary" => 0,
        "secondary" => 1,
        "tertiary" => 2,
        "government" => 3,
        "military_support" => 4,
        "infrastructure" => 5,
        _ => 6,
    }
}

fn building_employment_demand(
    building: &hoi4_state::Building,
    v6_db: &hoi4_content::V6Database,
) -> u32 {
    hoi4_content::active_pms_for_building(building, v6_db)
        .into_iter()
        .flat_map(|pm| pm.employment_demand)
        .sum::<u32>()
        .saturating_mul(building.level as u32)
}

fn ratio_u32(numerator: u32, denominator: u32) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn signed_amount(value: f64) -> String {
    if value >= 0.0 {
        format!("+{:.0} RM", value)
    } else {
        format!("{:.0} RM", value)
    }
}

fn build_fiscal_revenue(
    treasury: &hoi4_state::Treasury,
) -> hoi4_ui::finance_panel::FiscalRevenueBreakdownData {
    hoi4_ui::finance_panel::FiscalRevenueBreakdownData {
        pop_income_taxes_rm: treasury.daily_budget.income_pop_taxes_rm,
        consumption_taxes_rm: treasury.daily_budget.income_consumption_taxes_rm,
        corporate_taxes_rm: treasury.daily_budget.income_corporate_taxes_rm,
        trade_tariffs_rm: treasury.daily_budget.income_trade_tariffs_rm,
        state_profit_rm: treasury.daily_budget.income_state_profit_rm,
        financing_rm: treasury.daily_budget.income_domestic_bonds_rm
            + treasury.daily_financing.mefo_issued_rm
            + treasury.daily_financing.foreign_bond_issued_rm
            + treasury.daily_financing.gold_sold_rm,
        other_rm: treasury.daily_budget.income_other_rm,
    }
}

fn build_fiscal_expense(
    treasury: &hoi4_state::Treasury,
) -> hoi4_ui::finance_panel::FiscalExpenseBreakdownData {
    hoi4_ui::finance_panel::FiscalExpenseBreakdownData {
        military_rm: treasury.daily_budget.expense_military_wages_rm
            + treasury.daily_budget.expense_military_procurement_rm
            + treasury.daily_budget.expense_military_maintenance_rm
            + treasury.daily_budget.expense_military_training_rm,
        construction_rm: treasury.daily_budget.expense_construction_goods_rm
            + treasury.daily_budget.expense_construction_wages_rm,
        welfare_rm: treasury.daily_budget.expense_welfare_rm,
        administration_rm: treasury.daily_budget.expense_state_payroll_rm,
        interest_rm: treasury.daily_budget.expense_debt_interest_rm
            + treasury.daily_budget.expense_mefo_forced_payment_rm,
        foreign_exchange_rm: treasury.daily_budget.expense_foreign_currency_rm,
        research_rm: treasury.daily_budget.expense_research_rm,
        other_rm: treasury.daily_budget.expense_other_rm,
    }
}

fn build_construction_funding_trace(
    econ: &EconomyState,
    player: usize,
) -> hoi4_ui::finance_panel::ConstructionFundingTraceData {
    let mut trace = hoi4_ui::finance_panel::ConstructionFundingTraceData::default();
    let Some(queue) = econ.construction.get(player) else {
        return trace;
    };
    trace.active_projects = queue.items.len();
    for item in &queue.items {
        let amount = item.budget_needed_rm.max(item.reserved_funds_rm).max(0.0);
        trace.paid_rm += item.paid_funds_rm.max(0.0);
        trace.remaining_rm += item.funds_remaining_rm();
        match item.funding_source {
            ConstructionFundingSource::Government => trace.government_rm += amount,
            ConstructionFundingSource::Mefo => trace.mefo_rm += amount,
            ConstructionFundingSource::PrivatePool => trace.private_pool_rm += amount,
            ConstructionFundingSource::CartelPool => trace.cartel_pool_rm += amount,
            ConstructionFundingSource::OverlordInvestment { .. } => {
                trace.overlord_investment_rm += amount
            }
            ConstructionFundingSource::ForeignInvestment { .. } => {
                trace.foreign_investment_rm += amount
            }
        }
    }
    trace
}

fn build_investment_pool_data(
    world: &World,
    player: usize,
) -> hoi4_ui::finance_panel::InvestmentPoolData {
    let country = hoi4_state::CountryId(player as u16);
    let account = |kind: InvestmentAccountKind| {
        world
            .countries
            .investment_accounts
            .iter()
            .find(|account| account.country == country && account.account_kind == kind)
    };
    let private_rm = account(InvestmentAccountKind::Private)
        .map(|account| account.balance_rm)
        .unwrap_or_else(|| {
            world
                .countries
                .private_investment_pool_rm
                .get(player)
                .copied()
                .unwrap_or(0.0)
        });
    let cartel_rm = account(InvestmentAccountKind::Cartel)
        .map(|account| account.balance_rm)
        .unwrap_or(0.0);
    let state_development_bank_rm = account(InvestmentAccountKind::StateDevelopmentBank)
        .map(|account| account.balance_rm)
        .unwrap_or(0.0);
    let colonial_extraction_rm = account(InvestmentAccountKind::ColonialExtraction)
        .map(|account| account.balance_rm)
        .unwrap_or(0.0);
    let foreign_capital_rm = account(InvestmentAccountKind::ForeignCapital)
        .map(|account| account.balance_rm)
        .unwrap_or(0.0);
    let income_rm = world
        .countries
        .investment_accounts
        .iter()
        .filter(|account| account.country == country)
        .map(|account| account.last_income_rm)
        .sum();
    let spent_rm = world
        .countries
        .investment_accounts
        .iter()
        .filter(|account| account.country == country)
        .map(|account| account.last_spent_rm)
        .sum();
    hoi4_ui::finance_panel::InvestmentPoolData {
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
        income_rm,
        spent_rm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finance_builder_preserves_treasury_breakdown() {
        let mut treasury = hoi4_state::Treasury::default();
        treasury.cash_rm = 120.0;
        treasury.reserve_gbp = 7.5;
        treasury.gold_kg = 30.0;
        treasury.daily_budget.income_taxes_rm = 11.0;
        treasury.daily_budget.income_pop_taxes_rm = 5.0;
        treasury.daily_budget.income_consumption_taxes_rm = 3.0;
        treasury.daily_budget.income_corporate_taxes_rm = 2.0;
        treasury.daily_budget.income_trade_tariffs_rm = 1.0;
        treasury.daily_budget.income_state_profit_rm = 4.0;
        treasury.daily_budget.income_domestic_bonds_rm = 6.0;
        treasury.daily_budget.expense_military_wages_rm = 1.0;
        treasury.daily_budget.expense_military_procurement_rm = 2.0;
        treasury.daily_budget.expense_military_maintenance_rm = 3.0;
        treasury.daily_budget.expense_construction_goods_rm = 4.0;
        treasury.daily_budget.expense_construction_wages_rm = 2.0;
        treasury.daily_budget.expense_welfare_rm = 1.5;
        treasury.daily_budget.expense_debt_interest_rm = 0.75;
        treasury.daily_financing.mefo_issued_rm = 3.0;
        treasury.daily_financing.foreign_bond_issued_rm = 2.0;
        treasury.daily_financing.gold_sold_rm = 1.0;
        treasury.public_debt_rm = 45.0;
        treasury.gdp_rm = 900.0;
        treasury.gdp_breakdown.building_secondary_rm = 500.0;
        treasury.gdp_breakdown.pop_consumption_rm = 250.0;
        treasury.gdp_breakdown.historical_validation_gbp = 60.0;
        treasury.gdp_breakdown.historical_validation_error_ratio = 0.25;
        treasury.credit_rating = hoi4_state::CreditRating::BBB;

        let construction_funding = hoi4_ui::finance_panel::ConstructionFundingTraceData {
            government_rm: 20.0,
            private_pool_rm: 5.0,
            paid_rm: 9.0,
            remaining_rm: 16.0,
            active_projects: 2,
            ..Default::default()
        };
        let investment_pool = hoi4_ui::finance_panel::InvestmentPoolData {
            total_rm: 18.0,
            private_rm: 10.0,
            cartel_rm: 4.0,
            foreign_capital_rm: 4.0,
            income_rm: 1.25,
            spent_rm: 0.75,
            ..Default::default()
        };
        let sector_buildings = vec![hoi4_ui::finance_panel::EconomySectorBuildingEntry {
            sector_id: "secondary".to_owned(),
            sector_name: "二产".to_owned(),
            building_name: "钢铁厂".to_owned(),
            level: 2,
            employed: 120,
            demand: 160,
            employment_rate: 0.75,
            value_added_rm: 500.0,
        }];
        let employment_rows = build_employment_rows(&sector_buildings);
        let diagnostics = build_economy_diagnostics(
            &treasury,
            &construction_funding,
            &investment_pool,
            &sector_buildings,
            &employment_rows,
        );

        let dto = build_finance_panel_data_from_parts(
            &treasury,
            12.75,
            true,
            true,
            construction_funding,
            investment_pool,
            sector_buildings,
            employment_rows,
            diagnostics,
        );

        assert_eq!(dto.cash_rm, 120.0);
        assert_eq!(dto.reserve_gbp, 7.5);
        assert_eq!(dto.gold_kg, 30.0);
        assert_eq!(dto.budget_breakdown.income_taxes_rm, 11.0);
        assert_eq!(dto.budget_breakdown.tax_source_total_rm(), 11.0);
        assert_eq!(dto.budget_breakdown.expense_construction_goods_rm, 4.0);
        assert_eq!(dto.financing_breakdown.mefo_issued_rm, 3.0);
        assert_eq!(dto.fiscal_revenue.operating_total_rm(), 15.0);
        assert_eq!(dto.fiscal_revenue.financing_rm, 12.0);
        assert_eq!(dto.fiscal_expense.military_rm, 6.0);
        assert_eq!(dto.fiscal_expense.construction_rm, 6.0);
        assert_eq!(dto.fiscal_expense.interest_rm, 0.75);
        assert_eq!(dto.construction_funding.total_budget_rm(), 25.0);
        assert_eq!(dto.construction_funding.active_projects, 2);
        assert_eq!(dto.investment_pool.total_rm, 18.0);
        assert_eq!(dto.investment_pool.foreign_capital_rm, 4.0);
        assert_eq!(dto.public_debt_rm, 45.0);
        assert_eq!(dto.gdp_rm, 900.0);
        assert_eq!(dto.gdp_breakdown.building_secondary_rm, 500.0);
        assert_eq!(dto.gdp_breakdown.pop_consumption_rm, 250.0);
        assert_eq!(dto.gdp_breakdown.historical_validation_gbp, 60.0);
        assert_eq!(dto.gdp_breakdown.historical_validation_error_ratio, 0.25);
        assert_eq!(dto.sector_buildings.len(), 1);
        assert_eq!(dto.sector_buildings[0].sector_id, "secondary");
        assert_eq!(dto.sector_buildings[0].demand, 160);
        assert_eq!(dto.employment_rows.len(), 4);
        assert_eq!(
            dto.employment_rows
                .iter()
                .find(|row| row.label == "全部建筑")
                .map(|row| row.employed),
            Some(120)
        );
        assert!(dto.diagnostics.iter().any(|row| row.source == "就业吸收"));
        assert_eq!(dto.exchange_rate_rm_per_gbp, 12.75);
        assert_eq!(dto.credit_rating, "BBB");
        assert!(dto.can_print_mefo);
        assert!(dto.can_issue_foreign_bond);
        assert!(dto.is_foreign_exchange_control);
    }
}
