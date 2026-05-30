//! 简化海战解析器。
//!
//! 模型：
//! - 两支舰队（attacker / defender）位于同一海区，按 4 小时一轮交火
//! - 每轮：双方根据 [`super::FleetStats`] 计算总火力，对对方造成 HP / org 伤害
//! - 单船 HP 损失按其 max_hp 在舰队总 HP 中的占比分摊
//! - 舰队 HP 比例 < RETREAT_HP_RATIO 或 org 比例 < RETREAT_ORG_RATIO 即撤退
//! - 装甲 / AP 影响伤害（AP < armor → ×0.5..1）
//! - 鱼雷 / 反潜攻击额外加成
//!
//! 不实现：航母舰载机出击、视野 / 侦察、护航火力分配、空中支援

use crate::military::battle::DeterministicRng;
use hoi4_data::GameData;
use hoi4_state::{FleetId, World};

use super::constants::{
    HOURS_PER_ROUND, HP_DAMAGE_FACTOR, ORG_DAMAGE_FACTOR, RETREAT_HP_RATIO, RETREAT_ORG_RATIO,
};
use super::stats::FleetStats;

/// 战斗一方
#[derive(Debug, Clone)]
pub struct NavalBattleSide {
    pub fleet: FleetId,
    pub stats: FleetStats,
}

/// 战斗结果
#[derive(Debug, Clone)]
pub struct NavalBattleOutcome {
    pub rounds_fought: u32,
    pub attacker_won: bool,
    pub attacker_retreated: bool,
    pub defender_retreated: bool,
    /// 战后 attacker 舰队 stats
    pub attacker: NavalBattleSide,
    /// 战后 defender 舰队 stats
    pub defender: NavalBattleSide,
    /// 沉没舰数
    pub attacker_ships_sunk: u32,
    pub defender_ships_sunk: u32,
}

/// 模拟一次海战，最多 max_hours 小时
pub fn simulate(
    world: &mut World,
    data: &GameData,
    attacker_fleet: FleetId,
    defender_fleet: FleetId,
    max_hours: u32,
    rng: &mut DeterministicRng,
) -> NavalBattleOutcome {
    let mut atk_stats = FleetStats::aggregate(world, data, attacker_fleet);
    let mut def_stats = FleetStats::aggregate(world, data, defender_fleet);

    // 标记 in_combat
    set_fleet_in_combat(world, attacker_fleet, true);
    set_fleet_in_combat(world, defender_fleet, true);

    let init_atk_ships = atk_stats.ship_count();
    let init_def_ships = def_stats.ship_count();

    let mut hours = 0u32;
    let mut attacker_retreated = false;
    let mut defender_retreated = false;

    while hours < max_hours {
        // 随机扰动 ±10%（让两方不严格对称）
        let atk_jitter = 0.9 + rng.next_f32() * 0.2;
        let def_jitter = 0.9 + rng.next_f32() * 0.2;

        // 进攻方对防御方造成伤害
        let damage_to_def = compute_round_damage(&atk_stats, &def_stats) * atk_jitter;
        // 防御方反击
        let damage_to_atk = compute_round_damage(&def_stats, &atk_stats) * def_jitter;

        apply_fleet_damage(world, data, defender_fleet, damage_to_def);
        apply_fleet_damage(world, data, attacker_fleet, damage_to_atk);

        // 重新汇总
        atk_stats = FleetStats::aggregate(world, data, attacker_fleet);
        def_stats = FleetStats::aggregate(world, data, defender_fleet);

        hours += HOURS_PER_ROUND;

        // 撤退判定
        let atk_hp_ratio = if atk_stats.total_hp > 0.0 {
            atk_stats.current_hp / atk_stats.total_hp
        } else {
            0.0
        };
        let def_hp_ratio = if def_stats.total_hp > 0.0 {
            def_stats.current_hp / def_stats.total_hp
        } else {
            0.0
        };
        let atk_org_ratio = if atk_stats.total_max_org > 0.0 {
            atk_stats.current_org / atk_stats.total_max_org
        } else {
            0.0
        };
        let def_org_ratio = if def_stats.total_max_org > 0.0 {
            def_stats.current_org / def_stats.total_max_org
        } else {
            0.0
        };

        if atk_hp_ratio < RETREAT_HP_RATIO || atk_org_ratio < RETREAT_ORG_RATIO {
            attacker_retreated = true;
            break;
        }
        if def_hp_ratio < RETREAT_HP_RATIO || def_org_ratio < RETREAT_ORG_RATIO {
            defender_retreated = true;
            break;
        }
    }

    set_fleet_in_combat(world, attacker_fleet, false);
    set_fleet_in_combat(world, defender_fleet, false);

    let final_atk_stats = FleetStats::aggregate(world, data, attacker_fleet);
    let final_def_stats = FleetStats::aggregate(world, data, defender_fleet);

    let atk_ships_sunk = init_atk_ships.saturating_sub(final_atk_stats.ship_count());
    let def_ships_sunk = init_def_ships.saturating_sub(final_def_stats.ship_count());

    let attacker_won = !attacker_retreated && defender_retreated;

    NavalBattleOutcome {
        rounds_fought: hours / HOURS_PER_ROUND,
        attacker_won,
        attacker_retreated,
        defender_retreated,
        attacker: NavalBattleSide {
            fleet: attacker_fleet,
            stats: final_atk_stats,
        },
        defender: NavalBattleSide {
            fleet: defender_fleet,
            stats: final_def_stats,
        },
        attacker_ships_sunk: atk_ships_sunk,
        defender_ships_sunk: def_ships_sunk,
    }
}

/// 计算单轮总伤害（attacker 对 defender）
fn compute_round_damage(attacker: &FleetStats, defender: &FleetStats) -> f32 {
    // 三种火力混合：naval（surface）+ torpedo（surface）+ sub（vs sub）
    let surface_attack = attacker.total_naval_attack + attacker.total_torpedo_attack * 0.7;
    let sub_attack = attacker.total_sub_attack;

    // 装甲影响：armor_factor = clamp(AP/armor, 0.5, 1.0)
    let armor_factor = if defender.avg_armor > 0.0 {
        let ratio = attacker.max_ap / defender.avg_armor;
        ratio.clamp(0.5, 1.0)
    } else {
        1.0
    };

    // 防御方有潜艇时反潜火力对潜艇生效；反之 sub_attack 浪费
    let defender_has_subs = defender.submarine_count > 0;
    let usable_sub_attack = if defender_has_subs { sub_attack } else { 0.0 };

    // 命中率 = attack / (attack + defender_armor*10 + 100)
    let total_atk = surface_attack + usable_sub_attack;
    let denom = total_atk + defender.avg_armor * 10.0 + 100.0;
    let hit_rate = total_atk / denom;
    let damage = total_atk * hit_rate * armor_factor;
    damage.max(0.0)
}

/// 把伤害分摊到 fleet 中各 ship（按 max_hp 比例）
fn apply_fleet_damage(world: &mut World, data: &GameData, fleet: FleetId, damage: f32) {
    if fleet.is_none() {
        return;
    }
    let fi = fleet.0 as usize;
    if fi >= world.fleets.count {
        return;
    }
    if damage <= 0.0 {
        return;
    }
    // 总 max_hp（按当前未沉舰只计算）
    let mut total_max_hp = 0.0;
    for &s in &world.fleets.ships[fi] {
        if s.is_none() {
            continue;
        }
        let si = s.0 as usize;
        if si >= world.ships.count {
            continue;
        }
        if world.ships.hp[si] <= 0.0 {
            continue;
        }
        total_max_hp += world.ships.max_hp[si];
    }
    if total_max_hp <= 0.0 {
        return;
    }

    let hp_total = damage * HP_DAMAGE_FACTOR;
    let org_total = damage * ORG_DAMAGE_FACTOR;

    let ships = world.fleets.ships[fi].clone();
    for s in ships {
        if s.is_none() {
            continue;
        }
        let si = s.0 as usize;
        if si >= world.ships.count {
            continue;
        }
        if world.ships.hp[si] <= 0.0 {
            continue;
        }
        let share = world.ships.max_hp[si] / total_max_hp;

        // HP 损失（受装甲影响）
        let class_armor = data
            .ship_classes
            .get(&world.ships.class_keys[si])
            .map(|c| c.armor)
            .unwrap_or(0.0);
        let armor_dmg_factor = if class_armor > 50.0 { 0.6 } else { 1.0 };
        world.ships.hp[si] -= hp_total * share * armor_dmg_factor;
        world.ships.hp[si] = world.ships.hp[si].max(0.0);

        // org 损失
        let org_loss =
            org_total * share / world.ships.max_hp[si].max(1.0) * world.ships.max_organisation[si];
        world.ships.organisation[si] = (world.ships.organisation[si] - org_loss).max(0.0);
    }
}

fn set_fleet_in_combat(world: &mut World, fleet: FleetId, value: bool) {
    if fleet.is_none() {
        return;
    }
    let fi = fleet.0 as usize;
    if fi >= world.fleets.count {
        return;
    }
    let ships = world.fleets.ships[fi].clone();
    for s in ships {
        if s.is_none() {
            continue;
        }
        let si = s.0 as usize;
        if si < world.ships.count {
            world.ships.in_combat[si] = value;
        }
    }
}
