//! V6 经济系统 tick trait：市场经济与计划经济两条独立路径的公共接口。
//!
//! D4 硬约束（HC-3）：`market_tick.rs` 与 `planned_tick.rs` 不可互 import。
//! 所有共享逻辑必须经本 trait。

use hoi4_content::V6Database;
use hoi4_state::World;

use super::EconomyState;

/// 经济系统每日 tick 的公共 trait。
pub trait EconomicSystemTick {
    /// 执行一日经济 tick。
    ///
    /// 调用顺序：
    /// 1. building_production — 建筑产出推 market.supply / 拉原料 demand
    /// 2. pop_employment — 就业匹配
    /// 3. pop_wage — 工资支付
    /// 4. pop_consumption — POP 消费推 market.demand
    /// 5. pop_satisfaction — 满意度计算
    /// 6. pop_loyalty — 政治忠诚
    /// 7. price_update — 周更价格（每 7 天）
    /// 8. class_mobility — 阶级流动（每 90 天）
    /// 9. military_production — 军工建筑产出入装备库存
    fn tick_daily(
        world: &mut World,
        econ: &mut EconomyState,
        db: &V6Database,
        country_idx: usize,
        day: i64,
    );
}
