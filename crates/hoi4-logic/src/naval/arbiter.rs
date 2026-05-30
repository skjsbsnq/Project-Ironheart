//! 海战仲裁器：每 4 小时（HOURS_PER_ROUND）扫一遍所有 fleet，按 `region_id` 分组，
//! 在同区内为对立 owner（且双方在战争中）的两支舰队挑战斗最强者一对一对战。
//!
//! ## v1 局限（必读）
//! `world.fleets.region_id` **不是真正的战略海区 id**。它在 OOB 加载阶段被填成
//! `naval_base province id`（见 `World::populate_from_history`）。Phase 6.1 加
//! 载 `map/strategicregions/*.txt` 后必须替换为真海区 id。在那之前，本仲裁器只
//! 能把"同 naval_base 出发的对立 fleet"撮合到一起；这对 vanilla 1936-01-01
//! （未开战）和大多数测试场景已经够用。
//!
//! ## 调度
//! `hoi4-app` 的 `military_hourly` 每小时调用，本仲裁器内部 `% HOURS_PER_ROUND`
//! 决定是否真正跑一轮。

use std::collections::HashMap;

use hoi4_data::GameData;
use hoi4_state::{CountryId, FleetId, World};

use super::battle;
use super::constants::HOURS_PER_ROUND;
use super::stats::FleetStats;
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
    // 按 region_id 分组（仅采集"有人 / 非空"的 fleet）
    let mut by_region: HashMap<u32, Vec<FleetId>> = HashMap::new();
    for fi in 0..world.fleets.count {
        let region = world.fleets.region_id[fi];
        if region == u32::MAX {
            continue;
        }
        let owner = world.fleets.owners[fi];
        if owner.is_none() {
            continue;
        }
        // 空舰队（无 ship 引用）跳过
        if world.fleets.ships[fi].is_empty() {
            continue;
        }
        by_region
            .entry(region)
            .or_default()
            .push(FleetId(fi as u32));
    }

    // region_id 排序保证确定性
    let mut regions: Vec<u32> = by_region.keys().copied().collect();
    regions.sort();

    for region in regions {
        let fleets = by_region.remove(&region).unwrap_or_default();
        if fleets.len() < 2 {
            continue;
        }
        run_region(world, data, region, &fleets);
    }
}

/// 在单一海区内挑选对立 owner 的两支舰队对战。
///
/// 简化策略：找到**第一对**满足 (双方 owner 不同 + at_war_with) 的舰队，按 stats
/// `current_hp × org_ratio` 最高分别取代表，跑一轮 [`battle::simulate`]。
fn run_region(world: &mut World, data: &GameData, region: u32, fleets: &[FleetId]) {
    // 把 fleets 按 (owner, score) 分桶；每个 owner 取 score 最高一支
    let mut best_per_owner: HashMap<CountryId, (FleetId, f32)> = HashMap::new();
    for &fl in fleets {
        let owner = world.fleets.owners[fl.0 as usize];
        let stats = FleetStats::aggregate(world, data, fl);
        let hp = stats.current_hp;
        let org = if stats.total_max_org > 0.0 {
            stats.current_org / stats.total_max_org
        } else {
            0.0
        };
        let score = hp * org.max(0.05);
        if score <= 1e-3 {
            continue;
        }
        match best_per_owner.get(&owner) {
            Some(&(_, prev)) if prev >= score => {}
            _ => {
                best_per_owner.insert(owner, (fl, score));
            }
        }
    }
    if best_per_owner.len() < 2 {
        return;
    }

    // 在 owner 集合中找一对对立 + at_war，owner.0 较小者作为 attacker
    let mut owners: Vec<CountryId> = best_per_owner.keys().copied().collect();
    owners.sort_by_key(|c| c.0);

    for i in 0..owners.len() {
        for j in (i + 1)..owners.len() {
            let a = owners[i];
            let b = owners[j];
            if !world.diplomacy.at_war_with(a, b) {
                continue;
            }
            let fleet_a = best_per_owner[&a].0;
            let fleet_b = best_per_owner[&b].0;
            let seed = derive_seed(world.elapsed_hours, region, fleet_a.0, fleet_b.0);
            let mut rng = DeterministicRng::new(seed);
            let _ = battle::simulate(world, data, fleet_a, fleet_b, HOURS_PER_ROUND, &mut rng);
            // 单海区一轮就一场（v1 简化）；break 二维循环
            return;
        }
    }
}

fn derive_seed(hour: u64, region: u32, a: u32, b: u32) -> u32 {
    let mut s = (hour as u32) ^ region.rotate_left(8) ^ a.rotate_left(16) ^ b.rotate_left(24);
    s = s.wrapping_mul(0x9E37_79B1).wrapping_add(0xC0FF_EE17);
    if s == 0 {
        0xBEEF_DEAD
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_stable_and_distinct() {
        let s1 = derive_seed(10, 5, 1, 2);
        let s2 = derive_seed(10, 5, 1, 2);
        assert_eq!(s1, s2);
        assert_ne!(derive_seed(11, 5, 1, 2), s1);
        assert_ne!(derive_seed(10, 6, 1, 2), s1);
        assert_ne!(derive_seed(10, 5, 2, 1), s1);
    }

    #[test]
    fn seed_never_zero() {
        assert_ne!(derive_seed(0, 0, 0, 0), 0);
    }
}
