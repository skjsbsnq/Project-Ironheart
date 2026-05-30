//! Bug #7：每日师团清理（daily_division_cleanup）。
//!
//! 解决"师团永生不灭"——残兵败将被打到 strength≈0 / org≈0 后撤退到友方省，
//! 之后慢慢恢复 org，永远占着 `enemy_div_states` 让前线不消失。
//!
//! ## 销毁条件（满足任一即销毁）
//! 1. **极低战力（即时）**：strength < `IMMEDIATE_DESTROY_STRENGTH` —— 师团事实上
//!    已被打散到只剩残骸（< 0.1%），无论 org 是否恢复都直接清除。
//! 2. **持续濒死**：strength < `LOW_STRENGTH_THRESHOLD` 且 org_ratio < `LOW_ORG_RATIO`
//!    连续 `MIN_LOW_DAYS` 个 daily tick —— 让师团有 1 天缓冲机会被人力补满，
//!    避免误杀刚被打掉一轮但补给充足的师；超过则视为"无法恢复"。
//! 3. **被包围且濒死**：strength < `SURROUNDED_STRENGTH_THRESHOLD` 且
//!    位于敌控省 + BFS 在 `RETREAT_BFS_DEPTH` 跳内找不到友方省 —— 退无可退的
//!    残兵，直接投降销毁。
//!
//! ## 调度
//! 每天 `military_daily` 中调用一次。在 `daily_movement_tick` 之后调用，
//! 让 movement 先处理撤退，再判断"撤无可撤"的师。

use std::collections::{HashSet, VecDeque};

use hoi4_state::{CountryId, World};

/// 即时销毁阈值：strength < 5% 视作师团事实上已不存在。
const IMMEDIATE_DESTROY_STRENGTH: f32 = 0.05;
/// 持续濒死的 strength 上限。
const LOW_STRENGTH_THRESHOLD: f32 = 0.10;
/// 持续濒死的 org_ratio 上限。
const LOW_ORG_RATIO: f32 = 0.05;
/// 持续濒死必须连续多少个 daily tick 才销毁（>=1 表示"今天 + 至少另一天"）。
const MIN_LOW_DAYS: u8 = 1;
/// 被包围销毁的 strength 上限。
const SURROUNDED_STRENGTH_THRESHOLD: f32 = 0.15;
/// 撤退 BFS 深度（与 movement.rs 中的 MAX_DEPTH 保持一致：30 跳）。
const RETREAT_BFS_DEPTH: u32 = 30;

/// 每日师团清理：扫描所有师，把符合销毁条件的师从 World 中永久删除。
///
/// 返回销毁的师数（用于诊断 / 测试）。
pub fn daily_division_cleanup(world: &mut World) -> usize {
    let mut to_destroy: Vec<usize> = Vec::new();

    // 第一遍：累加 / 重置 low_strength_days，并标记即时销毁。
    for i in 0..world.divisions.count {
        let str_now = world.divisions.strength[i];
        let max_org = world.divisions.max_organisation[i].max(1e-6);
        let org_ratio = world.divisions.organisation[i] / max_org;

        if str_now < IMMEDIATE_DESTROY_STRENGTH {
            to_destroy.push(i);
            continue;
        }

        let is_low = str_now < LOW_STRENGTH_THRESHOLD && org_ratio < LOW_ORG_RATIO;
        if is_low {
            // 累加，饱和到 u8::MAX
            let cur = world.divisions.low_strength_days[i];
            world.divisions.low_strength_days[i] = cur.saturating_add(1);
            if world.divisions.low_strength_days[i] >= MIN_LOW_DAYS + 1 {
                // 已经濒死至少 MIN_LOW_DAYS 天 → 销毁
                to_destroy.push(i);
            }
        } else {
            world.divisions.low_strength_days[i] = 0;
        }
    }

    // 第二遍：包围销毁（只对 strength 低、不在友方控制省的师做 BFS）。
    // 已在 to_destroy 列表里的跳过避免重复 BFS。
    let already_doomed: HashSet<usize> = to_destroy.iter().copied().collect();
    for i in 0..world.divisions.count {
        if already_doomed.contains(&i) {
            continue;
        }
        let owner = world.divisions.owners[i];
        if owner.is_none() {
            continue;
        }
        let str_now = world.divisions.strength[i];
        if str_now >= SURROUNDED_STRENGTH_THRESHOLD {
            continue;
        }
        let cur = world.divisions.locations[i];
        let pi = cur.0 as usize;
        if pi >= world.provinces.count {
            continue;
        }
        if world.provinces.controllers[pi] == owner {
            // 已在友方省，不算被包围
            continue;
        }
        if !is_surrounded(world, owner, cur.0, RETREAT_BFS_DEPTH) {
            continue;
        }
        to_destroy.push(i);
    }

    if to_destroy.is_empty() {
        return 0;
    }

    let removed = world.remove_divisions(&to_destroy);
    if removed > 0 {
        tracing::info!(
            target: "military_cleanup",
            destroyed = removed,
            "daily_division_cleanup: 销毁残兵"
        );
    }
    removed
}

/// 从 `start` 出发 BFS 陆地省，看能否在 `max_depth` 跳内找到 controller==owner
/// 的省份。找不到 → 视为"被包围"。
fn is_surrounded(world: &World, owner: CountryId, start: u16, max_depth: u32) -> bool {
    let map = &world.map;
    if (start as usize) >= map.adjacencies.len() {
        return true;
    }
    let mut visited: HashSet<u16> = HashSet::new();
    let mut queue: VecDeque<(u16, u32)> = VecDeque::new();
    visited.insert(start);
    queue.push_back((start, 0));
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if (node as usize) >= map.adjacencies.len() {
            continue;
        }
        for &nb in &map.adjacencies[node as usize] {
            if !is_land(map, nb) {
                continue;
            }
            if !visited.insert(nb) {
                continue;
            }
            let nbi = nb as usize;
            if nbi >= world.provinces.count {
                continue;
            }
            if world.provinces.controllers[nbi] == owner {
                return false; // 找到友方省 → 没被包围
            }
            queue.push_back((nb, depth + 1));
        }
    }
    true
}

fn is_land(map: &hoi4_map::GameMap, raw_id: u16) -> bool {
    match map.get_province(raw_id) {
        Some(def) => matches!(def.province_type, hoi4_map::ProvinceType::Land),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_consistent() {
        // 即时销毁阈值必须低于持续濒死阈值，否则即时销毁路径永远是持续路径的子集。
        assert!(IMMEDIATE_DESTROY_STRENGTH < LOW_STRENGTH_THRESHOLD);
        // 包围阈值高于持续濒死阈值：包围路径覆盖更多"略微残血但退无可退"的师。
        assert!(SURROUNDED_STRENGTH_THRESHOLD >= LOW_STRENGTH_THRESHOLD);
    }
}
