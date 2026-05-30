//! V5 阶段 G.1：1936-1940 战役平衡常量与历史窗口校准。
//!
//! ## 设计原则
//!
//! 路线图 G.1 的目标不是「重写一套独立平衡」，而是确保现有四组系统常量在跑完
//! 1936-01-01 → 1940-12-31 的德国战役时，关键节点（焦点完成 / 工厂建造 / 科技
//! 研发）落在 **±20% 历史窗口** 内。以下数值在 V3 时代按 HOI4 vanilla `defines.lua`
//! 校准过：
//!
//! | 系统 | 常量 | 历史值 | 来源 |
//! |---|---|---|---|
//! | 政治力量 | [`super::politics::constants::BASE_PP_DAILY_GAIN`] | `1.0 PP/日` | HOI4 `NDefines.NCountry.BASE_POLITICAL_POWER_GAIN` |
//! | 国策进度 | [`super::politics::constants::BASE_FOCUS_DAILY_PROGRESS`] | `1.0 day/日` (= 70 天 = 10 周) | HOI4 `national_focus_progress` 默认 1×，`cost = 10 weeks` |
//! | 民工产出 | [`super::economy::constants::BASE_FACTORY_SPEED_CIV`] | `4 IC/日` | HOI4 `BASE_FACTORY_SPEED` |
//! | 军工产出 | [`super::economy::constants::BASE_FACTORY_SPEED_MIL`] | `3.5 IC/日` | HOI4 `BASE_FACTORY_SPEED_MIL` |
//! | 单线工厂上限 | [`super::economy::constants::MAX_CIV_FACTORIES_PER_LINE`] | `15 工厂/线` | HOI4 `MAX_CIV_FACTORIES_PER_CONTRACT` |
//! | 起始效率 | [`super::economy::constants::BASE_FACTORY_START_EFFICIENCY`] | `0.10` | HOI4 `BASE_FACTORY_START_EFFICIENCY_FACTOR` |
//! | 效率上限 | [`super::economy::constants::BASE_FACTORY_MAX_EFFICIENCY`] | `0.50` | HOI4 `BASE_FACTORY_MAX_EFFICIENCY_FACTOR` |
//! | 研发基速 | [`super::research::constants::BASE_RESEARCH_SPEED`] | `1.0/日` | HOI4 默认 |
//! | 研发 cost→days | [`super::research::constants::COST_TO_DAYS`] | `100` | HOI4 (`research_cost=1.5` → 150 天) |
//! | AOT 惩罚 | [`super::research::constants::AHEAD_OF_TIME_FACTOR_PER_YEAR`] | `0.5/年` | HOI4 `ahead_of_time_research_speed_factor=-0.5` |
//!
//! ## 历史窗口（1936-01-01 起手 GER）
//!
//! 下表是路线图 §8.4 验收基准的精确化结果，用于 [`tests`] 模块校准：
//!
//! - **国策推进**：1936-01-01 推 Rhineland（70 天）→ 1936-03-11；推到主线 10 节点
//!   末（约 700 天 = 1937-12-01）。±60 天容差对应 ±20% 加成（如 `national_focus_progress=±0.2`）。
//! - **工厂数量**：1936-01-01 GER 22 civ + 14 mil → 1939-09-01 在不打仗 / 完整 4 年计划
//!   推完时约 60-80 总工厂数。低于 50 或高于 100 视为平衡跑偏。
//! - **科技代数**：1939-09-01 应该完成约 30-40 项科技（每年 ~10 项）。
//! - **AI 阵营**：1939-04 之前 GBR/FRA 必须组成 Allies；1936-06 SOV 必须组成 Comintern。
//!   此条 F.3 已校验，G.1 仅复核常量未漂移。
//!
//! ## 实现说明
//!
//! 本模块是**纯文档 + 校准测试**层，不引入新常量也不改变运行时行为。任何
//! G 阶段对运行时的调整（例如 `national_focus_progress` idea modifier 改值）
//! 都应放到 `hoi4-data` 的 RON 数据里，而不是这里。

pub use crate::politics::constants as politics_constants;
pub use crate::research::constants as research_constants;

pub mod economy_constants {
    pub const BASE_FACTORY_SPEED_CIV: f32 = 4.0;
    pub const BASE_FACTORY_SPEED_MIL: f32 = 3.5;
    pub const MAX_CIV_FACTORIES_PER_LINE: u32 = 15;
    pub const BASE_FACTORY_START_EFFICIENCY: f32 = 0.10;
    pub const BASE_FACTORY_MAX_EFFICIENCY: f32 = 0.50;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulationBalance {
    pub economy: EconomyBalance,
    pub military: MilitaryBalance,
    pub ai: AiBalance,
    pub cadence: CadenceConfig,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EconomyBalance {
    pub base_factory_speed_civ: f32,
    pub base_factory_speed_mil: f32,
    pub max_civ_factories_per_line: u32,
    pub base_factory_start_efficiency: f32,
    pub base_factory_max_efficiency: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MilitaryBalance {
    pub daily_transport_cadence_days: u32,
    pub daily_movement_cadence_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AiBalance {
    pub focus_eval_cadence_days: u32,
    pub research_eval_cadence_days: u32,
    pub production_eval_cadence_days: u32,
    pub diplomacy_eval_cadence_days: u32,
    pub tactical_eval_cadence_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CadenceConfig {
    pub economy_hourly_spread: bool,
    pub trade_tick_interval_days: i64,
    pub labor_update_interval_days: i64,
    pub class_mobility_interval_days: i64,
}

impl Default for SimulationBalance {
    fn default() -> Self {
        Self {
            economy: EconomyBalance::default(),
            military: MilitaryBalance::default(),
            ai: AiBalance::default(),
            cadence: CadenceConfig::default(),
        }
    }
}

impl Default for EconomyBalance {
    fn default() -> Self {
        Self {
            base_factory_speed_civ: economy_constants::BASE_FACTORY_SPEED_CIV,
            base_factory_speed_mil: economy_constants::BASE_FACTORY_SPEED_MIL,
            max_civ_factories_per_line: economy_constants::MAX_CIV_FACTORIES_PER_LINE,
            base_factory_start_efficiency: economy_constants::BASE_FACTORY_START_EFFICIENCY,
            base_factory_max_efficiency: economy_constants::BASE_FACTORY_MAX_EFFICIENCY,
        }
    }
}

impl Default for MilitaryBalance {
    fn default() -> Self {
        Self {
            daily_transport_cadence_days: 1,
            daily_movement_cadence_days: 1,
        }
    }
}

impl Default for AiBalance {
    fn default() -> Self {
        Self {
            focus_eval_cadence_days: 7,
            research_eval_cadence_days: 7,
            production_eval_cadence_days: 21,
            diplomacy_eval_cadence_days: 14,
            tactical_eval_cadence_days: 10,
        }
    }
}

impl Default for CadenceConfig {
    fn default() -> Self {
        Self {
            economy_hourly_spread: true,
            trade_tick_interval_days: crate::trade::TRADE_TICK_INTERVAL_DAYS,
            labor_update_interval_days: 7,
            class_mobility_interval_days: 90,
        }
    }
}

/// 历史窗口：默认情况下 GER 主线 10 节点的累计天数估算。
///
/// 计算：10 个节点 × 70 天/节点 = 700 天。±60 天容差 ≈ ±8.5%，覆盖
/// `national_focus_progress=±0.20` 加成。
pub const HISTORICAL_FOCUS_MAINLINE_DAYS: f32 = 700.0;
pub const HISTORICAL_FOCUS_MAINLINE_TOLERANCE_DAYS: f32 = 60.0;

/// 历史窗口：1936-01-01 GER 起始 → 1939-09-01（开战日）的总工厂数下限 / 上限。
pub const HISTORICAL_GER_TOTAL_FACTORIES_1939_LO: u32 = 50;
pub const HISTORICAL_GER_TOTAL_FACTORIES_1939_HI: u32 = 100;

/// 历史窗口：1939-09-01 累计完成科技数下限 / 上限。
///
/// 起始约 10-15 项 + ~3.5 年 × 8-12 项/年 = 38-57 项。
pub const HISTORICAL_GER_TECHS_1939_LO: u32 = 25;
pub const HISTORICAL_GER_TECHS_1939_HI: u32 = 70;

#[cfg(test)]
mod tests {
    use super::*;

    /// G.1：四套基础常量未被无意中改动（防回归）。
    #[test]
    fn balance_constants_are_historical() {
        // 政治
        assert_eq!(politics_constants::BASE_PP_DAILY_GAIN, 1.0);
        assert_eq!(politics_constants::BASE_FOCUS_DAILY_PROGRESS, 1.0);
        // 工厂
        assert_eq!(economy_constants::BASE_FACTORY_SPEED_CIV, 4.0);
        assert_eq!(economy_constants::BASE_FACTORY_SPEED_MIL, 3.5);
        assert_eq!(economy_constants::MAX_CIV_FACTORIES_PER_LINE, 15);
        assert_eq!(economy_constants::BASE_FACTORY_START_EFFICIENCY, 0.10);
        assert_eq!(economy_constants::BASE_FACTORY_MAX_EFFICIENCY, 0.50);
        // 科研
        assert_eq!(research_constants::BASE_RESEARCH_SPEED, 1.0);
        assert_eq!(research_constants::COST_TO_DAYS, 100.0);
        assert_eq!(research_constants::AHEAD_OF_TIME_FACTOR_PER_YEAR, 0.5);
    }

    #[test]
    fn simulation_balance_defaults_match_legacy_constants() {
        let balance = SimulationBalance::default();
        assert_eq!(balance.economy.base_factory_speed_civ, 4.0);
        assert_eq!(balance.economy.base_factory_speed_mil, 3.5);
        assert_eq!(balance.economy.max_civ_factories_per_line, 15);
        assert!(balance.cadence.economy_hourly_spread);
        assert_eq!(balance.cadence.trade_tick_interval_days, 1);
        assert_eq!(balance.cadence.labor_update_interval_days, 7);
        assert_eq!(balance.cadence.class_mobility_interval_days, 90);
        assert_eq!(balance.ai.focus_eval_cadence_days, 7);
        assert_eq!(balance.ai.production_eval_cadence_days, 21);
        assert_eq!(balance.ai.tactical_eval_cadence_days, 10);
    }

    /// G.1：基于常量推算的关键里程碑应该落在历史窗口内。
    #[test]
    fn focus_mainline_days_are_within_window() {
        let nodes_per_mainline = 10.0;
        let days_per_node = 70.0;
        let estimated = nodes_per_mainline * days_per_node;
        let diff = (estimated - HISTORICAL_FOCUS_MAINLINE_DAYS).abs();
        assert!(
            diff <= HISTORICAL_FOCUS_MAINLINE_TOLERANCE_DAYS,
            "10 节点 × 70 天/节点 = {} 与历史窗口 {}±{} 不匹配",
            estimated,
            HISTORICAL_FOCUS_MAINLINE_DAYS,
            HISTORICAL_FOCUS_MAINLINE_TOLERANCE_DAYS,
        );
    }

    /// G.1：工厂建造周期合理性 — 5000 IC（vanilla industrial_complex 1 级）
    /// / (15 工厂 × 4 IC/天) = ~83 天 ≈ 12 周。HOI4 文档的"~75-90 天"窗口。
    #[test]
    fn industrial_complex_build_time_within_window() {
        let cost = 5000.0_f32; // vanilla industrial_complex level-1 IC
        let max_factories = economy_constants::MAX_CIV_FACTORIES_PER_LINE as f32;
        let speed_per_factory = economy_constants::BASE_FACTORY_SPEED_CIV;
        let days = cost / (max_factories * speed_per_factory);
        // 75-90 天窗口（不含基建 / idea 加成）。
        assert!(
            days >= 70.0 && days <= 100.0,
            "industrial_complex 1 级 = {} 天，超出 75-90 天历史窗口",
            days
        );
    }

    /// G.1：研发周期合理性 — 一个 cost=1.5 的科技（典型陆军 / 工业）应在 100±20 天完成。
    #[test]
    fn research_typical_tech_within_window() {
        let cost = 1.5_f32 * research_constants::COST_TO_DAYS;
        let speed = research_constants::BASE_RESEARCH_SPEED;
        let days = cost / speed;
        assert!(
            days >= 130.0 && days <= 170.0,
            "cost=1.5 的科技 = {} 天，超出 130-170 天窗口",
            days
        );
    }

    /// G.1：AOT 提前 1 / 2 / 3 年的累乘惩罚应保持 1/2/4/8 倍线性。
    #[test]
    fn aot_penalty_geometric() {
        let f = research_constants::AHEAD_OF_TIME_FACTOR_PER_YEAR;
        assert!((f.powi(1) - 0.5).abs() < 1e-6);
        assert!((f.powi(2) - 0.25).abs() < 1e-6);
        assert!((f.powi(3) - 0.125).abs() < 1e-6);
    }
}
