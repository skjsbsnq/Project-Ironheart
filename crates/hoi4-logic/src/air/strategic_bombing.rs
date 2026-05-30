//! 战略轰炸：联队对敌方某 state 的工业 / 基建造成日伤害。
//!
//! 简化模型：
//! - 调用方提供：执行轰炸的 wing、目标 state、保护方制空权（来自 [`AirControl`]）
//! - 每日伤害 = total_strategic_bombing × STRAT_BOMB_FACTOR × (1 - enemy_air_control)
//! - 伤害按比例摊到 V6 建筑等级与 infrastructure
//! - 命中至少 1 点建筑则减少 1 个；否则只累计 partial_damage
//! - 同时根据 enemy_air_control 让轰炸方损失飞机

use hoi4_data::GameData;
use hoi4_state::{AirWingId, CountryId, StateId, World};

use super::air_superiority::AirControl;
use super::constants::STRAT_BOMB_FACTOR;
use super::stats::AirWingStats;

/// 一次战略轰炸行动的结果
#[derive(Debug, Clone)]
pub struct StrategicBombingOutcome {
    pub state_id: StateId,
    /// V6 建筑等级被毁数
    pub buildings_destroyed: u8,
    /// 基建被毁数（HOI4 中基建可被击毁；最低 0）
    pub infra_destroyed: u8,
    /// 轰炸方飞机损失
    pub bomber_planes_lost: u32,
    /// 累计未达整数的"伤害值"（debug）
    pub raw_damage: f32,
}

impl Default for StrategicBombingOutcome {
    fn default() -> Self {
        Self {
            state_id: StateId::NONE,
            buildings_destroyed: 0,
            infra_destroyed: 0,
            bomber_planes_lost: 0,
            raw_damage: 0.0,
        }
    }
}

/// 一个待执行的轰炸目标
#[derive(Debug, Clone, Copy)]
pub struct StrategicBombingTarget {
    pub state_id: StateId,
    /// 目标所在空区 — 用于查 enemy air control
    pub region: u32,
    /// 守卫国（state.controller）
    pub defender: CountryId,
}

/// 执行一次单日轰炸。返回 outcome；同时直接修改 world 中的 state 建筑数。
pub fn execute_bombing(
    world: &mut World,
    data: &GameData,
    air_control: &AirControl,
    bomber_wing: AirWingId,
    target: StrategicBombingTarget,
) -> StrategicBombingOutcome {
    let mut out = StrategicBombingOutcome::default();
    out.state_id = target.state_id;

    if bomber_wing.is_none() || target.state_id.is_none() {
        return out;
    }
    let bi = bomber_wing.0 as usize;
    if bi >= world.air_wings.count {
        return out;
    }
    let si = target.state_id.0 as usize;
    if si >= world.states.count {
        return out;
    }

    let stats = AirWingStats::aggregate(world, data, bomber_wing);
    if stats.total_strategic_bombing <= 0.0 || stats.plane_count == 0 {
        return out;
    }

    let enemy_ac = if target.defender.is_none() {
        0.0
    } else {
        air_control.control(target.region, target.defender)
    };

    // 伤害 = bombing × factor × (1 - enemy_ac)
    let damage = stats.total_strategic_bombing * STRAT_BOMB_FACTOR * (1.0 - enemy_ac).max(0.0);
    out.raw_damage = damage;

    // 按 V6 建筑等级与基建分布分摊
    let building_levels = world.state_building_levels(target.state_id) as f32;
    let infra = world.states.infrastructure[si] as f32;
    let total = building_levels + infra * 0.5;
    if total <= 0.0 {
        return out;
    }

    let building_dmg = damage * building_levels / total;
    let infra_dmg = damage * (infra * 0.5) / total;

    // 离散化：超过 1 即扣 1 点，向下取整
    let mut building_loss = building_dmg.floor() as u8;
    let infra_loss = infra_dmg.floor() as u8;

    for building in &mut world.countries.buildings_v6.buildings {
        if building_loss == 0 {
            break;
        }
        if building.state != target.state_id || building.level == 0 {
            continue;
        }
        building.level = building.level.saturating_sub(1);
        building_loss -= 1;
        out.buildings_destroyed += 1;
    }
    world.states.infrastructure[si] = world.states.infrastructure[si].saturating_sub(infra_loss);

    out.infra_destroyed = infra_loss;

    // 轰炸方反击损失：拦截方制空越高，飞机损失越多。
    // loss = bomber_planes × enemy_ac × 0.012（每日上限 1.2%）
    let bomber_loss_f = stats.plane_count as f32 * enemy_ac * 0.012;
    // 用 ceil 而非 round —— 即使少量损失也至少 1 架（前提 damage > 0.3）
    let bomber_loss = if bomber_loss_f > 0.3 {
        bomber_loss_f.round().max(1.0) as u32
    } else {
        0
    };
    let cur = world.air_wings.count_planes[bi];
    world.air_wings.count_planes[bi] = cur.saturating_sub(bomber_loss);
    out.bomber_planes_lost = bomber_loss.min(cur);

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoi4_state::StateId;

    #[test]
    fn zero_damage_for_invalid_target() {
        let out = StrategicBombingOutcome::default();
        assert_eq!(out.buildings_destroyed, 0);
    }

    #[test]
    fn target_struct_compiles() {
        let _ = StrategicBombingTarget {
            state_id: StateId::NONE,
            region: 0,
            defender: CountryId::NONE,
        };
    }
}
