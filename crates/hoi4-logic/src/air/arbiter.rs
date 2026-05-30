//! 空战仲裁器：每 4 小时（HOURS_PER_ROUND）扫一遍所有 air_wing，按 `region_id`
//! 分组，在同区内为对立 owner（双方战争中）的两支联队挑战斗最强者一对一拦截。
//!
//! ## v1 局限（必读）
//! `world.air_wings.region_id` **不是真正的战略空区 id**。它在 OOB 加载阶段被
//! 填成 `state_id`（见 `World::populate_from_history`）。Phase 6 加载
//! `map/airregions.txt` 后须替换为真空区 id。
//!
//! ## 调度
//! `hoi4-app` 的 `military_hourly` 每小时调用，本仲裁器内部 `% HOURS_PER_ROUND`
//! 决定是否真正跑一轮。

use std::collections::HashMap;

use hoi4_data::{AircraftKind, GameData};
use hoi4_state::{AirWingId, CountryId, World};

use super::battle;
use super::constants::HOURS_PER_ROUND;
use super::stats::AirWingStats;
use crate::military::battle::DeterministicRng;

/// 每 4 小时一轮。
pub fn tick_hourly(world: &mut World, data: &GameData) {
    if world.elapsed_hours % HOURS_PER_ROUND as u64 != 0 {
        return;
    }
    run_round(world, data);
}

/// 强制跑一轮（测试 / 集成验证用）。
pub fn run_round(world: &mut World, data: &GameData) {
    let mut by_region: HashMap<u32, Vec<AirWingId>> = HashMap::new();
    for wi in 0..world.air_wings.count {
        if world.air_wings.transfer_arrival_hour[wi] != 0 {
            continue;
        }
        if !super::operations::is_air_to_air_mission(world.air_wings.mission[wi]) {
            continue;
        }
        let region = if world.air_wings.target_region[wi] == super::regions::NO_AIR_REGION {
            world.air_wings.region_id[wi]
        } else {
            world.air_wings.target_region[wi]
        };
        if region == u32::MAX {
            continue;
        }
        let owner = world.air_wings.owners[wi];
        if owner.is_none() {
            continue;
        }
        if world.air_wings.count_planes[wi] == 0 {
            continue;
        }
        by_region
            .entry(region)
            .or_default()
            .push(AirWingId(wi as u32));
    }

    let mut regions: Vec<u32> = by_region.keys().copied().collect();
    regions.sort();

    for region in regions {
        let wings = by_region.remove(&region).unwrap_or_default();
        if wings.len() < 2 {
            continue;
        }
        run_region(world, data, region, &wings);
    }
}

/// 单空区一轮：取 fighter 系（拦截方）+ 任一对立非战斗机（被拦截方）。
/// 无战斗机则取战斗最强的对立 wing 对战。
fn run_region(world: &mut World, data: &GameData, region: u32, wings: &[AirWingId]) {
    // 第一遍：每 owner 选最强 wing；同时记录最强 fighter（拦截优先级）
    let mut best_per_owner: HashMap<CountryId, (AirWingId, f32)> = HashMap::new();
    let mut best_fighter_per_owner: HashMap<CountryId, (AirWingId, f32)> = HashMap::new();
    for &wing in wings {
        let owner = world.air_wings.owners[wing.0 as usize];
        let stats = AirWingStats::aggregate(world, data, wing);
        let score = stats.total_air_attack + stats.total_air_defense * 0.5;
        if score <= 1e-3 {
            continue;
        }
        match best_per_owner.get(&owner) {
            Some(&(_, prev)) if prev >= score => {}
            _ => {
                best_per_owner.insert(owner, (wing, score));
            }
        }
        if matches!(
            stats.kind,
            Some(AircraftKind::Fighter) | Some(AircraftKind::HeavyFighter)
        ) {
            match best_fighter_per_owner.get(&owner) {
                Some(&(_, prev)) if prev >= score => {}
                _ => {
                    best_fighter_per_owner.insert(owner, (wing, score));
                }
            }
        }
    }
    if best_per_owner.len() < 2 {
        return;
    }

    let mut owners: Vec<CountryId> = best_per_owner.keys().copied().collect();
    owners.sort_by_key(|c| c.0);

    for i in 0..owners.len() {
        for j in (i + 1)..owners.len() {
            let a = owners[i];
            let b = owners[j];
            if !world.diplomacy.at_war_with(a, b) {
                continue;
            }
            // 拦截方优先 = fighter；若两方都没 fighter，按 best_per_owner
            let wing_a = best_fighter_per_owner
                .get(&a)
                .map(|&(w, _)| w)
                .unwrap_or(best_per_owner[&a].0);
            let wing_b = best_fighter_per_owner
                .get(&b)
                .map(|&(w, _)| w)
                .unwrap_or(best_per_owner[&b].0);
            let seed = derive_seed(world.elapsed_hours, region, wing_a.0, wing_b.0);
            let mut rng = DeterministicRng::new(seed);
            let _ = battle::simulate(world, data, wing_a, wing_b, HOURS_PER_ROUND, &mut rng);
            return;
        }
    }
}

fn derive_seed(hour: u64, region: u32, a: u32, b: u32) -> u32 {
    let mut s = (hour as u32) ^ region.rotate_left(11) ^ a.rotate_left(7) ^ b.rotate_left(19);
    s = s.wrapping_mul(0x9E37_79B1).wrapping_add(0xFEED_FACE);
    if s == 0 {
        0xABAD_BABE
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_stable_and_distinct() {
        let s = derive_seed(1, 1, 1, 1);
        assert_eq!(s, derive_seed(1, 1, 1, 1));
        assert_ne!(s, derive_seed(2, 1, 1, 1));
        assert_ne!(s, derive_seed(1, 2, 1, 1));
    }

    #[test]
    fn seed_never_zero() {
        assert_ne!(derive_seed(0, 0, 0, 0), 0);
    }
}
