//! V6 财政与双货币 schema（D5 + D8）。

use serde::{Deserialize, Serialize};

use crate::market::NationalMarket;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BudgetBreakdown {
    pub income_taxes_rm: f64,
    pub income_state_profit_rm: f64,
    pub income_domestic_bonds_rm: f64,
    pub income_other_rm: f64,
    pub expense_state_payroll_rm: f64,
    pub expense_military_wages_rm: f64,
    pub expense_military_procurement_rm: f64,
    pub expense_military_maintenance_rm: f64,
    #[serde(default)]
    pub expense_military_training_rm: f64,
    pub expense_construction_goods_rm: f64,
    pub expense_construction_wages_rm: f64,
    pub expense_welfare_rm: f64,
    pub expense_debt_interest_rm: f64,
    pub expense_foreign_currency_rm: f64,
    pub expense_mefo_forced_payment_rm: f64,
    #[serde(default)]
    pub expense_research_rm: f64,
    pub expense_other_rm: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FinancingBreakdown {
    pub mefo_issued_rm: f64,
    pub mefo_interest_capitalized_rm: f64,
    pub domestic_bond_issued_rm: f64,
    pub foreign_bond_issued_rm: f64,
    pub gold_sold_rm: f64,
}

impl BudgetBreakdown {
    pub fn operating_income_rm(&self) -> f64 {
        self.income_taxes_rm + self.income_state_profit_rm + self.income_other_rm
    }

    pub fn total_income_rm(&self) -> f64 {
        self.operating_income_rm() + self.income_domestic_bonds_rm
    }

    pub fn total_expense_rm(&self) -> f64 {
        self.expense_state_payroll_rm
            + self.expense_military_wages_rm
            + self.expense_military_procurement_rm
            + self.expense_military_maintenance_rm
            + self.expense_military_training_rm
            + self.expense_construction_goods_rm
            + self.expense_construction_wages_rm
            + self.expense_welfare_rm
            + self.expense_debt_interest_rm
            + self.expense_foreign_currency_rm
            + self.expense_mefo_forced_payment_rm
            + self.expense_research_rm
            + self.expense_other_rm
    }
}

impl FinancingBreakdown {
    pub fn total_financing_rm(&self) -> f64 {
        self.mefo_issued_rm
            + self.domestic_bond_issued_rm
            + self.foreign_bond_issued_rm
            + self.gold_sold_rm
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CreditRating {
    AAA,
    AA,
    A,
    BBB,
    BB,
    B,
    CCC,
    D,
}

impl CreditRating {
    pub fn from_debt_ratio(ratio: f64) -> Self {
        if ratio < 0.3 {
            Self::AAA
        } else if ratio < 0.5 {
            Self::AA
        } else if ratio < 0.8 {
            Self::A
        } else if ratio < 1.2 {
            Self::BBB
        } else if ratio < 1.6 {
            Self::BB
        } else if ratio < 2.0 {
            Self::B
        } else if ratio < 3.0 {
            Self::CCC
        } else {
            Self::D
        }
    }

    pub fn interest_rate_bonus(&self) -> f32 {
        match self {
            Self::AAA => -0.005,
            Self::AA => 0.0,
            Self::A => 0.005,
            Self::BBB => 0.01,
            Self::BB => 0.02,
            Self::B => 0.04,
            Self::CCC => 0.08,
            Self::D => 0.15,
        }
    }

    pub fn can_issue_foreign_bonds(&self) -> bool {
        matches!(self, Self::AAA | Self::AA | Self::A | Self::BBB)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Treasury {
    pub cash_rm: f64,
    pub daily_income_rm: f64,
    pub daily_expense_rm: f64,
    pub daily_budget: BudgetBreakdown,
    pub daily_financing: FinancingBreakdown,

    pub operating_income_rm: f64,
    pub operating_expense_rm: f64,
    pub original_deficit_rm: f64,
    pub mefo_coverage_rm: f64,
    pub post_financing_cash_change_rm: f64,

    pub reserve_gbp: f64,
    pub gold_kg: f64,
    pub daily_trade_balance_gbp: f64,

    pub public_debt_gbp: f64,
    pub public_debt_rm: f64,
    pub mefo_debt_rm: f64,
    #[serde(default)]
    pub mefo_disabled: bool,
    #[serde(default)]
    pub mefo_military_budget_rm: f64,
    #[serde(default)]
    pub mefo_military_spent_rm: f64,
    pub bond_interest_rate: f32,
    pub credit_rating: CreditRating,

    pub gdp_gbp: f64,
    pub gdp_rm: f64,
    #[serde(default)]
    pub domestic_gdp_gbp: f64,
    #[serde(default)]
    pub domestic_gdp_rm: f64,
    #[serde(default)]
    pub colonial_gdp_gbp: f64,
    #[serde(default)]
    pub colonial_gdp_rm: f64,
    #[serde(default)]
    pub colonial_extracted_value_gbp: f64,
    #[serde(default)]
    pub colonial_extracted_value_rm: f64,
    #[serde(default)]
    pub gdp_last_year_gbp: f64,
    #[serde(default)]
    pub gdp_growth_yoy: f32,
    /// Effective law-derived tax rates: income, consumption, corporate.
    pub tax_rates: [f32; 3],
}

impl Default for Treasury {
    fn default() -> Self {
        Self {
            cash_rm: 0.0,
            daily_income_rm: 0.0,
            daily_expense_rm: 0.0,
            daily_budget: BudgetBreakdown::default(),
            daily_financing: FinancingBreakdown::default(),
            operating_income_rm: 0.0,
            operating_expense_rm: 0.0,
            original_deficit_rm: 0.0,
            mefo_coverage_rm: 0.0,
            post_financing_cash_change_rm: 0.0,
            reserve_gbp: 0.0,
            gold_kg: 0.0,
            daily_trade_balance_gbp: 0.0,
            public_debt_gbp: 0.0,
            public_debt_rm: 0.0,
            mefo_debt_rm: 0.0,
            mefo_disabled: false,
            mefo_military_budget_rm: 0.0,
            mefo_military_spent_rm: 0.0,
            bond_interest_rate: 0.04,
            credit_rating: CreditRating::AAA,
            gdp_gbp: 0.0,
            gdp_rm: 0.0,
            domestic_gdp_gbp: 0.0,
            domestic_gdp_rm: 0.0,
            colonial_gdp_gbp: 0.0,
            colonial_gdp_rm: 0.0,
            colonial_extracted_value_gbp: 0.0,
            colonial_extracted_value_rm: 0.0,
            gdp_last_year_gbp: 0.0,
            gdp_growth_yoy: 0.0,
            tax_rates: [0.20, 0.10, 0.20],
        }
    }
}

impl Treasury {
    /// Reset daily accounting counters before a new fiscal day starts.
    pub fn reset_daily_accumulators(&mut self) {
        self.daily_income_rm = 0.0;
        self.daily_expense_rm = 0.0;
        self.daily_trade_balance_gbp = 0.0;
        self.daily_budget = BudgetBreakdown::default();
        self.daily_financing = FinancingBreakdown::default();
        self.operating_income_rm = 0.0;
        self.operating_expense_rm = 0.0;
        self.original_deficit_rm = 0.0;
        self.mefo_coverage_rm = 0.0;
        self.post_financing_cash_change_rm = 0.0;
        self.mefo_military_spent_rm = 0.0;
    }

    pub fn receive(&mut self, amount_rm: f64, purpose: &str) -> f64 {
        let actual = amount_rm.max(0.0);
        if actual <= 0.0 {
            return 0.0;
        }
        self.cash_rm += actual;
        self.daily_income_rm += actual;
        match purpose {
            "taxes" => {
                self.daily_budget.income_taxes_rm += actual;
                self.operating_income_rm += actual;
            }
            "state_profit" => {
                self.daily_budget.income_state_profit_rm += actual;
                self.operating_income_rm += actual;
            }
            "domestic_bond" => {
                self.daily_budget.income_domestic_bonds_rm += actual;
                self.daily_financing.domestic_bond_issued_rm += actual;
            }
            _ => {
                self.daily_budget.income_other_rm += actual;
                self.operating_income_rm += actual;
            }
        }
        actual
    }

    pub fn record_expense(&mut self, amount_rm: f64, purpose: &str) {
        match purpose {
            "state_payroll" => self.daily_budget.expense_state_payroll_rm += amount_rm,
            "military_wages" => self.daily_budget.expense_military_wages_rm += amount_rm,
            "military_procurement" => {
                self.daily_budget.expense_military_procurement_rm += amount_rm
            }
            "military_maintenance" => {
                self.daily_budget.expense_military_maintenance_rm += amount_rm
            }
            "military_training" => self.daily_budget.expense_military_training_rm += amount_rm,
            "construction_goods" => self.daily_budget.expense_construction_goods_rm += amount_rm,
            "construction_wages" => self.daily_budget.expense_construction_wages_rm += amount_rm,
            "welfare" => self.daily_budget.expense_welfare_rm += amount_rm,
            "debt_interest" => self.daily_budget.expense_debt_interest_rm += amount_rm,
            "foreign_currency_purchase" => {
                self.daily_budget.expense_foreign_currency_rm += amount_rm
            }
            "mefo_forced_payment" => self.daily_budget.expense_mefo_forced_payment_rm += amount_rm,
            "research" => self.daily_budget.expense_research_rm += amount_rm,
            _ => self.daily_budget.expense_other_rm += amount_rm,
        }
    }

    /// P6 关键（HC-2）：唯一允许扣 cash_rm 的路径（测试代码除外）。
    /// 向全国市场下采购单。返回实际采购量（可能因 cash 或 supply 不足而少于请求量）。
    pub fn gov_buy(&mut self, market: &mut NationalMarket, good_id: &str, qty: f32) -> f32 {
        self.gov_buy_for(market, good_id, qty, "government_procurement")
    }

    pub fn gov_buy_for(
        &mut self,
        market: &mut NationalMarket,
        good_id: &str,
        qty: f32,
        purpose: &str,
    ) -> f32 {
        let unit_price = market.price.get(good_id).copied().unwrap_or(0.01).max(0.01);
        let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
        let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
        let available = (supply - demand).max(0.0);
        let cap_by_supply = available;
        let cap_by_cash = if unit_price > 0.0 {
            (self.cash_rm / unit_price as f64) as f32
        } else {
            qty
        };
        let actual = qty.min(cap_by_supply).min(cap_by_cash).max(0.0);
        if actual > 0.0 {
            let cost = (actual * unit_price) as f64;
            self.cash_rm -= cost;
            self.daily_expense_rm += cost;
            self.operating_expense_rm += cost;
            self.record_expense(cost, purpose);
            if let Some(d) = market.demand.get_mut(good_id) {
                *d += actual;
            }
        }
        actual
    }

    pub fn gov_buy_on_credit_for(
        &mut self,
        market: &mut NationalMarket,
        good_id: &str,
        qty: f32,
        purpose: &str,
    ) -> f32 {
        let unit_price = market.price.get(good_id).copied().unwrap_or(0.01).max(0.01);
        let supply = market.supply.get(good_id).copied().unwrap_or(0.0);
        let demand = market.demand.get(good_id).copied().unwrap_or(0.0);
        let available = (supply - demand).max(0.0);
        let actual = qty.min(available).max(0.0);
        if actual > 0.0 {
            let cost = (actual * unit_price) as f64;
            self.cash_rm -= cost;
            self.daily_expense_rm += cost;
            self.operating_expense_rm += cost;
            self.record_expense(cost, purpose);
            *market.demand.entry(good_id.to_owned()).or_insert(0.0) += actual;
        }
        actual
    }

    /// Pay a non-market expense such as wages, welfare, research, or interest.
    pub fn pay(&mut self, amount_rm: f64, purpose: &str) -> f64 {
        let actual = amount_rm.max(0.0);
        if actual > 0.0 {
            self.cash_rm -= actual;
            self.daily_expense_rm += actual;
            self.operating_expense_rm += actual;
            self.record_expense(actual, purpose);
        }
        actual
    }

    /// 发行国内国债（RM 计价）
    pub fn issue_domestic_bond(&mut self, amount_rm: f64) {
        if amount_rm <= 0.0 {
            return;
        }
        self.public_debt_rm += amount_rm;
        self.receive(amount_rm, "domestic_bond");
    }

    /// 发行外国国债（£ 计价），需 credit_rating ≥ BBB
    pub fn issue_foreign_bond(&mut self, amount_gbp: f64) -> Result<(), String> {
        if !self.credit_rating.can_issue_foreign_bonds() {
            return Err("credit rating too low to issue foreign bonds (need BBB+)".to_owned());
        }
        if amount_gbp <= 0.0 {
            return Ok(());
        }
        self.reserve_gbp += amount_gbp;
        self.public_debt_gbp += amount_gbp;
        Ok(())
    }

    /// 印 MEFO 票据（隐性国债）。仅补当日赤字，上限当日赤字。
    pub fn print_mefo(&mut self, max_amount: f64) -> f64 {
        if self.mefo_disabled {
            return 0.0;
        }
        let daily_deficit = self.daily_expense_rm - self.operating_income_rm;
        if daily_deficit <= 0.0 {
            return 0.0;
        }
        let actual = max_amount.min(daily_deficit).max(0.0);
        if actual > 0.0 {
            self.mefo_debt_rm += actual;
            self.cash_rm += actual;
            self.daily_income_rm += actual;
            self.mefo_coverage_rm += actual;
            self.daily_financing.mefo_issued_rm += actual;
        }
        actual
    }

    /// 卖金换 £（按伦敦金价，简化为固定比率 0.84 £/kg）
    pub fn sell_gold(&mut self, kg: f64) -> f64 {
        let actual = kg.min(self.gold_kg).max(0.0);
        if actual <= 0.0 {
            return 0.0;
        }
        let gbp = actual * 0.84;
        self.gold_kg -= actual;
        self.reserve_gbp += gbp;
        gbp
    }

    /// 用 RM 买 £（受 Trade 法限制）
    pub fn buy_foreign_currency(&mut self, rm_per_gbp: f32, gbp_amount: f64) -> f64 {
        let cost_rm = gbp_amount * rm_per_gbp as f64;
        if cost_rm > self.cash_rm {
            return 0.0;
        }
        self.pay(cost_rm, "foreign_currency_purchase");
        self.reserve_gbp += gbp_amount;
        gbp_amount
    }

    /// 付国债利息（每日）。
    ///
    /// 公债（£ / RM）：按 `bond_interest_rate + credit_rating bonus` 计息；£ 利息优先从储备扣，扣不到的滚入 `public_debt_gbp`（复利）；RM 利息走 `pay()` 从现金扣。
    /// MEFO 票据：历史化处理——利息**资本化**滚入 `mefo_debt_rm` 本金，**不**形成现金流出、**不**计入 `expense_debt_interest_rm`、**不**在支出面板显示。利率固定 4% 年化（与公债 rating 解耦，体现"隐性短期票据"性质）。
    pub fn pay_interest(&mut self) {
        let effective_rate = self.bond_interest_rate + self.credit_rating.interest_rate_bonus();
        let daily_interest_gbp = self.public_debt_gbp * effective_rate as f64 / 365.0;
        let daily_interest_rm = self.public_debt_rm * effective_rate as f64 / 365.0;
        let paid_interest_gbp = self.reserve_gbp.max(0.0).min(daily_interest_gbp);
        self.reserve_gbp -= paid_interest_gbp;
        self.public_debt_gbp += (daily_interest_gbp - paid_interest_gbp).max(0.0);
        self.pay(daily_interest_rm, "debt_interest");

        let mefo_interest = self.mefo_debt_rm * 0.04 / 365.0;
        self.mefo_debt_rm += mefo_interest;
        self.daily_financing.mefo_interest_capitalized_rm += mefo_interest;
    }

    /// 更新信用评级
    pub fn update_credit_rating(&mut self, rm_per_gbp: f32) {
        let total_debt_gbp = self.public_debt_gbp + self.public_debt_rm / rm_per_gbp as f64;
        let ratio = if self.gdp_gbp > 0.0 {
            total_debt_gbp / self.gdp_gbp
        } else {
            0.0
        };
        self.credit_rating = CreditRating::from_debt_ratio(ratio);
    }

    /// 检查 MEFO 爆雷条件：mefo_debt_rm / gdp_rm > 0.30
    pub fn compute_fiscal_summary(&mut self) {
        self.operating_expense_rm = self.daily_expense_rm;
        self.original_deficit_rm = (self.operating_expense_rm - self.operating_income_rm).max(0.0);
        self.post_financing_cash_change_rm = self.daily_income_rm - self.daily_expense_rm;
    }

    pub fn record_mefo_military_spending(&mut self, amount_rm: f64) {
        if amount_rm <= 0.0 {
            return;
        }
        self.mefo_military_spent_rm += amount_rm;
    }

    pub fn check_mefo_crisis(&self) -> bool {
        self.check_mefo_crisis_at(0.30)
    }

    pub fn check_mefo_crisis_at(&self, threshold_ratio: f64) -> bool {
        if self.gdp_rm <= 0.0 {
            return false;
        }
        self.mefo_debt_rm / self.gdp_rm > threshold_ratio
    }

    /// MEFO 爆雷效果
    pub fn trigger_mefo_crisis(&mut self, rm_per_gbp: f32) {
        self.trigger_mefo_crisis_with_ratios(rm_per_gbp, 0.4, 0.6);
    }

    pub fn trigger_mefo_crisis_with_ratios(
        &mut self,
        rm_per_gbp: f32,
        forced_payment_ratio: f64,
        residual_debt_ratio: f64,
    ) {
        let forced_payment = self.mefo_debt_rm * forced_payment_ratio.max(0.0);
        self.pay(forced_payment, "mefo_forced_payment");
        self.public_debt_rm += self.mefo_debt_rm * residual_debt_ratio.max(0.0);
        self.mefo_debt_rm = 0.0;
        self.mefo_disabled = true;
        self.credit_rating = CreditRating::D;
        self.update_credit_rating(rm_per_gbp);
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExchangeRate {
    pub rm_per_gbp: f32,
}

impl ExchangeRate {
    pub const BASE_RATE: f32 = 12.5;
    const EMA_FACTOR: f32 = 0.05;

    /// §4.5.2 汇率更新公式
    pub fn compute_target(
        &self,
        gold_kg: f64,
        public_debt_gbp: f64,
        gdp_gbp: f64,
        trade_law_modifier: f32,
    ) -> f32 {
        let mut target = Self::BASE_RATE;
        let gold_modifier = (1.0 - 0.005 * (gold_kg / 1000.0) as f32).clamp(0.90, 1.10);
        target *= gold_modifier;
        let debt_pressure = if gdp_gbp > 0.0 {
            public_debt_gbp / gdp_gbp
        } else {
            0.0
        };
        if debt_pressure > 0.5 {
            target *= 1.0 + 0.01 * (debt_pressure - 0.5) as f32;
        }
        target *= trade_law_modifier;
        target
    }

    /// 每周 EMA 平滑更新
    pub fn weekly_update(
        &mut self,
        gold_kg: f64,
        public_debt_gbp: f64,
        gdp_gbp: f64,
        trade_law_modifier: f32,
    ) {
        let target = self.compute_target(gold_kg, public_debt_gbp, gdp_gbp, trade_law_modifier);
        self.rm_per_gbp += Self::EMA_FACTOR * (target - self.rm_per_gbp);
        self.rm_per_gbp = self.rm_per_gbp.max(0.1);
    }

    /// 检测月度汇率剧变（>10%）
    pub fn is_crisis(&self, previous: f32) -> bool {
        if previous <= 0.0 {
            return false;
        }
        let change = (self.rm_per_gbp - previous).abs() / previous;
        change > 0.10
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TreasuryStore {
    pub treasuries: Vec<Treasury>,
    pub exchange_rates: Vec<ExchangeRate>,
}

impl TreasuryStore {
    pub fn new(count: usize) -> Self {
        Self {
            treasuries: vec![Treasury::default(); count],
            exchange_rates: vec![ExchangeRate { rm_per_gbp: 12.5 }; count],
        }
    }

    pub fn ensure_capacity(&mut self, n: usize) {
        while self.treasuries.len() < n {
            self.treasuries.push(Treasury::default());
        }
        while self.exchange_rates.len() < n {
            self.exchange_rates.push(ExchangeRate { rm_per_gbp: 12.5 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Treasury;

    #[test]
    fn daily_accumulator_zeroes() {
        let mut treasury = Treasury::default();
        treasury.daily_income_rm = 365.0;
        treasury.daily_expense_rm = 730.0;
        treasury.daily_trade_balance_gbp = -12.0;

        treasury.reset_daily_accumulators();

        assert_eq!(treasury.daily_income_rm, 0.0);
        assert_eq!(treasury.daily_expense_rm, 0.0);
        assert_eq!(treasury.daily_trade_balance_gbp, 0.0);
    }
}
