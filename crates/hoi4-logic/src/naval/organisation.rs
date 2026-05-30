//! 海军组织度系统：每日恢复（仅当不在战斗中）。
//!
//! HOI4 中舰船 organisation 在远离港口巡逻时缓慢恢复，待在港时更快。我们
//! 简化为一律 30%/天（与陆军一致），直到 max_organisation。HP 不自动恢复
//! （需要造船坞维修，留给 Phase 6 / Phase 8）。

use hoi4_state::World;

/// 与陆军 `ORG_REGEN_PER_DAY` 一致的恢复率。
const ORG_REGEN_PER_DAY: f32 = 0.30;

/// 每日 tick：所有不在战斗的舰只恢复 org
pub fn tick_daily(world: &mut World) {
    let n = world.ships.count;
    for i in 0..n {
        if world.ships.in_combat[i] {
            continue;
        }
        let max_org = world.ships.max_organisation[i];
        let cur = world.ships.organisation[i];
        if cur >= max_org {
            continue;
        }
        let new_org = (cur + max_org * ORG_REGEN_PER_DAY).min(max_org);
        world.ships.organisation[i] = new_org;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regen_amount_matches_30_percent() {
        let max: f32 = 60.0;
        let cur: f32 = 10.0;
        let after = (cur + max * ORG_REGEN_PER_DAY).min(max);
        assert!((after - 28.0).abs() < 1e-3);
    }

    #[test]
    fn regen_caps_at_max() {
        let max: f32 = 60.0;
        let cur: f32 = 55.0;
        let after = (cur + max * ORG_REGEN_PER_DAY).min(max);
        assert_eq!(after, max);
    }
}
