//! 空战解析器。
//!
//! 模型：
//! - 两侧各持一组联队（attacker / defender），同空区相遇
//! - 每 4 小时一轮：双方按 [`super::AirWingStats`] 加权交火
//! - 命中率 ≈ atk_attack / (atk_attack + def_defense × agility_factor + 1)
//! - 战损：飞机数减少（按伤害 × LOSS_FACTOR），组织度同步下降
//! - 一方组织度 / 飞机数低于阈值即撤退
//! - 拦截方对纯轰炸机 / CAS 联队享受 INTERCEPTOR_BONUS 命中加成

use hoi4_data::{AircraftKind, GameData};
use hoi4_state::{AirWingId, World};

use crate::military::battle::DeterministicRng;

use super::constants::{
    HOURS_PER_ROUND, INTERCEPTOR_BONUS, ORG_LOSS_FACTOR, PLANE_LOSS_FACTOR, RETREAT_ORG_RATIO,
    RETREAT_PLANE_RATIO,
};
use super::stats::AirWingStats;

/// 空战一方
#[derive(Debug, Clone)]
pub struct AirBattleSide {
    pub wing: AirWingId,
    pub stats: AirWingStats,
}

/// 空战结果
#[derive(Debug, Clone)]
pub struct AirBattleOutcome {
    pub rounds_fought: u32,
    pub attacker_won: bool,
    pub attacker_retreated: bool,
    pub defender_retreated: bool,
    pub attacker: AirBattleSide,
    pub defender: AirBattleSide,
    pub attacker_planes_lost: u32,
    pub defender_planes_lost: u32,
}

/// 模拟一次空战，最多 max_hours 小时。
///
/// `attacker` 通常为 AirSuperiority / Interception 任务发起方；
/// `defender` 通常为执行任务被拦截的一方（可能是轰炸机 / CAS）。
pub fn simulate(
    world: &mut World,
    data: &GameData,
    attacker_wing: AirWingId,
    defender_wing: AirWingId,
    max_hours: u32,
    rng: &mut DeterministicRng,
) -> AirBattleOutcome {
    let init_atk_planes = world
        .air_wings
        .count_planes
        .get(attacker_wing.0 as usize)
        .copied()
        .unwrap_or(0);
    let init_def_planes = world
        .air_wings
        .count_planes
        .get(defender_wing.0 as usize)
        .copied()
        .unwrap_or(0);

    set_wing_in_combat(world, attacker_wing, true);
    set_wing_in_combat(world, defender_wing, true);

    let mut hours = 0u32;
    let mut attacker_retreated = false;
    let mut defender_retreated = false;

    while hours < max_hours {
        let atk = AirWingStats::aggregate(world, data, attacker_wing);
        let def = AirWingStats::aggregate(world, data, defender_wing);

        // 任一方已无飞机或 org=0 → 结束
        if atk.plane_count == 0 || def.plane_count == 0 {
            break;
        }

        // 拦截加成：如 attacker 是 fighter / heavy_fighter，defender 是 bomber 系列
        let atk_intercept_bonus = is_fighter(atk.kind) && is_bomber_or_cas(def.kind);
        let def_intercept_bonus = is_fighter(def.kind) && is_bomber_or_cas(atk.kind);

        let jitter_a = 0.9 + rng.next_f32() * 0.2;
        let jitter_d = 0.9 + rng.next_f32() * 0.2;

        let dmg_to_def = compute_round_damage(&atk, &def, atk_intercept_bonus) * jitter_a;
        let dmg_to_atk = compute_round_damage(&def, &atk, def_intercept_bonus) * jitter_d;

        apply_wing_damage(world, defender_wing, dmg_to_def);
        apply_wing_damage(world, attacker_wing, dmg_to_atk);

        hours += HOURS_PER_ROUND;

        // 撤退判断
        let post_atk = AirWingStats::aggregate(world, data, attacker_wing);
        let post_def = AirWingStats::aggregate(world, data, defender_wing);
        if post_atk.plane_ratio() < RETREAT_PLANE_RATIO
            || post_atk.org_ratio() < RETREAT_ORG_RATIO
            || post_atk.plane_count == 0
        {
            attacker_retreated = true;
            break;
        }
        if post_def.plane_ratio() < RETREAT_PLANE_RATIO
            || post_def.org_ratio() < RETREAT_ORG_RATIO
            || post_def.plane_count == 0
        {
            defender_retreated = true;
            break;
        }
    }

    set_wing_in_combat(world, attacker_wing, false);
    set_wing_in_combat(world, defender_wing, false);

    let final_atk_stats = AirWingStats::aggregate(world, data, attacker_wing);
    let final_def_stats = AirWingStats::aggregate(world, data, defender_wing);

    let atk_lost = init_atk_planes.saturating_sub(final_atk_stats.plane_count);
    let def_lost = init_def_planes.saturating_sub(final_def_stats.plane_count);

    let attacker_won =
        !attacker_retreated && (defender_retreated || final_def_stats.plane_count == 0);

    AirBattleOutcome {
        rounds_fought: hours / HOURS_PER_ROUND,
        attacker_won,
        attacker_retreated,
        defender_retreated,
        attacker: AirBattleSide {
            wing: attacker_wing,
            stats: final_atk_stats,
        },
        defender: AirBattleSide {
            wing: defender_wing,
            stats: final_def_stats,
        },
        attacker_planes_lost: atk_lost,
        defender_planes_lost: def_lost,
    }
}

/// 单轮伤害（attacker → defender）。
fn compute_round_damage(atk: &AirWingStats, def: &AirWingStats, intercept_bonus: bool) -> f32 {
    let raw_atk = atk.total_air_attack;
    if raw_atk <= 0.0 {
        return 0.0;
    }
    // agility_factor：def 的敏捷提供闪避；atk 高 agility 抵消之
    let agi_def = def.avg_agility.max(1.0);
    let agi_atk = atk.avg_agility.max(1.0);
    let agility_factor = (agi_def / agi_atk).clamp(0.5, 2.0);
    let denom = raw_atk + def.total_air_defense * agility_factor + 1.0;
    let hit_rate = raw_atk / denom;
    let mut dmg = raw_atk * hit_rate;
    if intercept_bonus {
        dmg *= INTERCEPTOR_BONUS;
    }
    dmg.max(0.0)
}

/// 把伤害施加到联队：扣减 plane_count + organisation。
fn apply_wing_damage(world: &mut World, wing: AirWingId, damage: f32) {
    if wing.is_none() || damage <= 0.0 {
        return;
    }
    let i = wing.0 as usize;
    if i >= world.air_wings.count {
        return;
    }
    // 飞机损失（按 LOSS_FACTOR 转换）
    let plane_loss = (damage * PLANE_LOSS_FACTOR).round() as u32;
    let cur = world.air_wings.count_planes[i];
    world.air_wings.count_planes[i] = cur.saturating_sub(plane_loss);

    // 组织度损失
    let max_org = world.air_wings.max_organisation[i];
    let org_loss = damage * ORG_LOSS_FACTOR / world.air_wings.max_planes[i].max(1) as f32 * max_org;
    let new_org = (world.air_wings.organisation[i] - org_loss).max(0.0);
    world.air_wings.organisation[i] = new_org;
}

fn set_wing_in_combat(world: &mut World, wing: AirWingId, value: bool) {
    if wing.is_none() {
        return;
    }
    let i = wing.0 as usize;
    if i < world.air_wings.count {
        world.air_wings.in_combat[i] = value;
    }
}

fn is_fighter(kind: Option<AircraftKind>) -> bool {
    matches!(
        kind,
        Some(AircraftKind::Fighter) | Some(AircraftKind::HeavyFighter)
    )
}

fn is_bomber_or_cas(kind: Option<AircraftKind>) -> bool {
    matches!(
        kind,
        Some(AircraftKind::CloseAirSupport)
            | Some(AircraftKind::TacticalBomber)
            | Some(AircraftKind::StrategicBomber)
            | Some(AircraftKind::NavalBomber)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fighter_classification() {
        assert!(is_fighter(Some(AircraftKind::Fighter)));
        assert!(is_fighter(Some(AircraftKind::HeavyFighter)));
        assert!(!is_fighter(Some(AircraftKind::CloseAirSupport)));
        assert!(!is_fighter(None));
    }

    #[test]
    fn bomber_classification() {
        assert!(is_bomber_or_cas(Some(AircraftKind::CloseAirSupport)));
        assert!(is_bomber_or_cas(Some(AircraftKind::StrategicBomber)));
        assert!(!is_bomber_or_cas(Some(AircraftKind::Fighter)));
    }
}
