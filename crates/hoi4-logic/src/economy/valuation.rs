//! P1.4 统一估值 helper：建筑利润、GDP 价值增量、RM 口径单一权威源。
//!
//! 所有建筑产出/投入/工资/利润/价值增量的计算必须通过本模块完成。
//! 财政 tick、GDP tick、市场 tick、UI 面板均读取 `Building` 上的运行时字段，
//! 不再各自重新推导。

use hoi4_content::{ProductionMethodDef, V6Database};
use hoi4_state::{Building, LawCategory, PopClass, World};

/// 商品 RM 缩放常数（唯一权威定义）。
/// 1 单位商品吞吐量 × 市场价格 × GOODS_RM_SCALE = 每日 RM 产出。
pub const GOODS_RM_SCALE: f64 = 1_200.0;

/// GDP 价值增量缩放常数（唯一权威定义）。
/// 价值增量的商品吞吐量单位通过此常数转换为年化 RM GDP。
pub const GDP_OUTPUT_RM_SCALE: f64 = 1_200.0;

/// 建筑每日估值结果，写入 `Building` 运行时字段。
#[derive(Clone, Debug, Default)]
pub struct BuildingValuation {
    /// 实际就业率（0..1）
    pub employment_ratio: f32,
    /// 产出价值（RM/日），含 throughput、equipment output、employment_ratio
    pub output_value_rm: f64,
    /// 投入成本（RM/日），不含 throughput，含 employment_ratio
    pub input_cost_rm: f64,
    /// 工资成本（RM/日），含经济法工资乘数
    pub wage_rm: f32,
    /// 实际利润（RM/日）= output - input - wage，可为负
    pub actual_profit_rm: f64,
    /// 估算利润（RM/日）= 满雇用时的利润，≥ 0
    pub estimated_profit_rm: f64,
    /// 产出价值（GBP/日）
    pub output_value_gbp: f64,
    /// 价值增量（用于 GDP 计算），含 35% 下限
    pub value_added_rm: f64,
}

/// 统一计算建筑每日估值。
///
/// 所有口径由本函数决定，调用方只读取结果。
pub fn compute_building_valuation(
    building: &Building,
    world: &World,
    db: &V6Database,
    ci: usize,
) -> BuildingValuation {
    let pms = hoi4_content::active_pms_for_building(building, db);
    if pms.is_empty() || building.level == 0 {
        return BuildingValuation::default();
    }

    let market = &world.countries.market.markets[ci];
    let rm_per_gbp = world.countries.treasury.exchange_rates[ci]
        .rm_per_gbp
        .max(0.1) as f64;
    let wage_mults = economy_wage_mults(world, db, ci);

    let emp_ratio = employment_ratio(building, &pms);

    let output_rm = output_value_rm(building, &pms, market) * GOODS_RM_SCALE * emp_ratio as f64;
    let input_rm = input_cost_rm(building, &pms, market) * GOODS_RM_SCALE * emp_ratio as f64;
    let wage = wage_cost_rm(building, &pms, wage_mults);

    let actual_profit = output_rm - input_rm - wage as f64;

    let estimated_output = output_value_rm(building, &pms, market) * GOODS_RM_SCALE;
    let estimated_input = input_cost_rm(building, &pms, market) * GOODS_RM_SCALE;
    let estimated_wage = wage_cost_at_full_employment(building, &pms, wage_mults);
    let estimated_profit = (estimated_output - estimated_input - estimated_wage as f64).max(0.0);

    let output_gbp = output_rm / rm_per_gbp;

    let value_added = compute_value_added(building, &pms, market, emp_ratio);

    BuildingValuation {
        employment_ratio: emp_ratio,
        output_value_rm: output_rm,
        input_cost_rm: input_rm,
        wage_rm: wage,
        actual_profit_rm: actual_profit,
        estimated_profit_rm: estimated_profit,
        output_value_gbp: output_gbp,
        value_added_rm: value_added,
    }
}

/// 计算价值增量（用于 GDP），含 35% 下限保护。
fn compute_value_added(
    building: &Building,
    pms: &[&ProductionMethodDef],
    market: &hoi4_state::market::NationalMarket,
    emp_ratio: f32,
) -> f64 {
    let mut output_value = 0.0_f64;
    let mut input_cost = 0.0_f64;
    for pm in pms {
        let throughput = pm.throughput_modifier.max(0.0);
        for (i, good_id) in pm.output_good_ids.iter().enumerate() {
            let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                * throughput
                * building.level as f32;
            let price = market.price.get(good_id).copied().unwrap_or(1.0);
            output_value += amount as f64 * price as f64;
        }
        if let Some(eq_out) = &pm.equipment_output {
            let price = equipment_procurement_good(&eq_out.equipment_category)
                .and_then(|good_id| market.price.get(good_id))
                .copied()
                .unwrap_or(1.0);
            output_value += eq_out.daily_per_level as f64
                * throughput as f64
                * building.level as f64
                * price as f64
                * 100.0;
        }
        for (i, good_id) in pm.input_good_ids.iter().enumerate() {
            let amount =
                pm.input_good_amounts.get(i).copied().unwrap_or(0.0) * building.level as f32;
            let price = market.price.get(good_id).copied().unwrap_or(1.0);
            input_cost += amount as f64 * price as f64;
        }
    }
    let va = (output_value - input_cost).max(output_value * 0.35) * emp_ratio as f64;
    va * GOODS_RM_SCALE
}

fn employment_ratio(building: &Building, pms: &[&ProductionMethodDef]) -> f32 {
    if building.level == 0 {
        return 0.0;
    }
    let mut total_needed = 0.0_f32;
    let mut total_filled = 0.0_f32;
    for class_idx in 0..PopClass::COUNT {
        let needed = pms
            .iter()
            .map(|pm| pm.employment_demand.get(class_idx).copied().unwrap_or(0) as f32)
            .sum::<f32>()
            * building.level as f32;
        let filled = building.employment.get(class_idx).copied().unwrap_or(0) as f32;
        total_needed += needed;
        total_filled += filled.min(needed);
    }
    if total_needed <= 0.0 {
        1.0
    } else {
        (total_filled / total_needed).clamp(0.0, 1.0)
    }
}

fn output_value_rm(
    building: &Building,
    pms: &[&ProductionMethodDef],
    market: &hoi4_state::market::NationalMarket,
) -> f64 {
    pms.iter()
        .map(|pm| {
            let throughput = pm.throughput_modifier.max(0.0);
            let goods_value: f64 = pm
                .output_good_ids
                .iter()
                .enumerate()
                .map(|(i, good_id)| {
                    let amount = pm.output_good_amounts.get(i).copied().unwrap_or(0.0)
                        * throughput
                        * building.level as f32;
                    let price = market.price.get(good_id).copied().unwrap_or(1.0);
                    amount as f64 * price as f64
                })
                .sum();
            goods_value
                + pm.equipment_output
                    .as_ref()
                    .map(|eq| {
                        let price = equipment_procurement_good(&eq.equipment_category)
                            .and_then(|good_id| market.price.get(good_id))
                            .copied()
                            .unwrap_or(1.0);
                        eq.daily_per_level as f64
                            * throughput as f64
                            * building.level as f64
                            * price as f64
                            * 100.0
                    })
                    .unwrap_or(0.0)
        })
        .sum()
}

fn input_cost_rm(
    building: &Building,
    pms: &[&ProductionMethodDef],
    market: &hoi4_state::market::NationalMarket,
) -> f64 {
    pms.iter()
        .flat_map(|pm| {
            pm.input_good_ids
                .iter()
                .enumerate()
                .map(move |(i, good_id)| {
                    let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0)
                        * building.level as f32;
                    let price = market.price.get(good_id).copied().unwrap_or(1.0);
                    amount as f64 * price as f64
                })
        })
        .sum()
}

/// 实际就业对应的工资成本（含经济法乘数）。
fn wage_cost_rm(building: &Building, pms: &[&ProductionMethodDef], wage_mults: [f32; 6]) -> f32 {
    let base_wage = [1.5_f32, 3.0, 5.0, 15.0, 25.0, 2.5];
    (0..PopClass::COUNT)
        .map(|idx| {
            let needed = pms
                .iter()
                .map(|pm| pm.employment_demand.get(idx).copied().unwrap_or(0) as f32)
                .sum::<f32>()
                * building.level as f32;
            let filled = building.employment.get(idx).copied().unwrap_or(0) as f32;
            filled.min(needed) * base_wage[idx] * wage_mults[idx]
        })
        .sum()
}

/// 满雇用时的工资成本（含经济法乘数），用于估算利润。
fn wage_cost_at_full_employment(
    building: &Building,
    pms: &[&ProductionMethodDef],
    wage_mults: [f32; 6],
) -> f32 {
    let base_wage = [1.5_f32, 3.0, 5.0, 15.0, 25.0, 2.5];
    (0..PopClass::COUNT)
        .map(|idx| {
            let needed = pms
                .iter()
                .map(|pm| pm.employment_demand.get(idx).copied().unwrap_or(0) as f32)
                .sum::<f32>()
                * building.level as f32;
            needed * base_wage[idx] * wage_mults[idx]
        })
        .sum()
}

fn economy_wage_mults(world: &World, db: &V6Database, ci: usize) -> [f32; 6] {
    let current_economy_law = world.countries.law_store.law_sets[ci].0
        [LawCategory::Economy.index()]
    .current
    .as_str();
    let economy_def = db
        .economy_laws
        .iter()
        .find(|law| law.id == current_economy_law);
    let mut out = [1.0; 6];
    out[PopClass::Worker.index()] = economy_def
        .map(|law| law.wage_multiplier_worker)
        .unwrap_or(1.0);
    out[PopClass::Capitalist.index()] = economy_def
        .map(|law| law.wage_multiplier_capitalist)
        .unwrap_or(1.0);
    out
}

/// 估算满雇用利润（供 UI 建造面板和 AI 建造规划使用）。
pub fn expected_profit_rm_daily(
    pms: &[&ProductionMethodDef],
    level: u8,
    market: Option<&hoi4_state::market::NationalMarket>,
) -> f64 {
    if level == 0 || pms.is_empty() {
        return 0.0;
    }
    let Some(market) = market else {
        return 0.0;
    };
    let mut output_rm = 0.0_f64;
    let mut input_rm = 0.0_f64;
    let mut wage_rm = 0.0_f32;
    let base_wage = [1.5_f32, 3.0, 5.0, 15.0, 25.0, 2.5];
    for pm in pms {
        let throughput = pm.throughput_modifier.max(0.0);
        for (i, good_id) in pm.output_good_ids.iter().enumerate() {
            let amount =
                pm.output_good_amounts.get(i).copied().unwrap_or(0.0) * throughput * level as f32;
            let price = market.price.get(good_id).copied().unwrap_or(1.0);
            output_rm += amount as f64 * price as f64;
        }
        if let Some(eq_out) = &pm.equipment_output {
            let price = equipment_procurement_good(&eq_out.equipment_category)
                .and_then(|good_id| market.price.get(good_id))
                .copied()
                .unwrap_or(1.0);
            output_rm += eq_out.daily_per_level as f64
                * throughput as f64
                * level as f64
                * price as f64
                * 100.0;
        }
        for (i, good_id) in pm.input_good_ids.iter().enumerate() {
            let amount = pm.input_good_amounts.get(i).copied().unwrap_or(0.0) * level as f32;
            let price = market.price.get(good_id).copied().unwrap_or(1.0);
            input_rm += amount as f64 * price as f64;
        }
        for class_idx in 0..PopClass::COUNT {
            wage_rm += pm.employment_demand.get(class_idx).copied().unwrap_or(0) as f32
                * level as f32
                * base_wage[class_idx];
        }
    }
    (output_rm * GOODS_RM_SCALE - input_rm * GOODS_RM_SCALE - wage_rm as f64).max(0.0)
}

fn equipment_procurement_good(category: &str) -> Option<&'static str> {
    match category {
        "infantry_equipment" => Some("small_arms"),
        "artillery" => Some("artillery"),
        "anti_tank" => Some("anti_tank"),
        "anti_air" => Some("anti_air"),
        "support_equipment" => Some("support_equipment"),
        "motorized" => Some("trucks"),
        "mechanized" => Some("halftracks"),
        "light_tank" | "medium_tank" | "heavy_tank" | "modern_tank" => Some("tanks"),
        "fighter" | "cas" | "tactical_bomber" | "strategic_bomber" | "naval_bomber" => {
            Some("airframes")
        }
        "convoy" => Some("convoys"),
        "destroyer" | "submarine" | "cruiser" | "capital_ship" => Some("warships"),
        "train" => Some("trains"),
        _ => None,
    }
}
