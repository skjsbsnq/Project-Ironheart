//! 组织度系统：每日恢复（仅当不在战斗中）。
//!
//! Bug #13 修复：刚打完仗的师不会立刻恢复 org，而是需要等待
//! `ORG_REGEN_DELAY_HOURS` 小时后才开始恢复，避免"打完即恢复"导致的无休止拉锯。

use hoi4_state::World;

use super::constants::ORG_REGEN_PER_DAY;

/// Bug #13：战斗结束后多少小时内不恢复 org。
/// vanilla HOI4 中战斗结束后有约 4 小时的"恢复延迟"；我们用 12 小时（3 个战斗轮次）
/// 让净 org 损耗在短时间内不可恢复，加速决战。
const ORG_REGEN_DELAY_HOURS: u64 = 12;

/// 每日 tick：所有不在战斗的师恢复 org（Bug #13：刚打完仗的师有恢复延迟）
pub fn tick_daily(world: &mut World) {
    let n = world.divisions.count;
    let now = world.elapsed_hours;
    for i in 0..n {
        if world.divisions.in_combat[i] {
            continue;
        }
        let last_combat = world.divisions.last_combat_hour[i];
        if last_combat != u64::MAX && now.saturating_sub(last_combat) < ORG_REGEN_DELAY_HOURS {
            continue;
        }
        let max_org = world.divisions.max_organisation[i];
        let cur = world.divisions.organisation[i];
        if cur >= max_org {
            continue;
        }
        let recovery_mult = super::general::modifier_for_division(world, i)
            .map(|modifier| modifier.org_recovery_mult)
            .unwrap_or(1.0);
        let new_org = (cur + max_org * ORG_REGEN_PER_DAY * recovery_mult).min(max_org);
        world.divisions.organisation[i] = new_org;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regen_amount() {
        // 起始 org=10、max=60、regen=18% -> +10.8 -> 20.8
        let max: f32 = 60.0;
        let cur: f32 = 10.0;
        let after = (cur + max * ORG_REGEN_PER_DAY).min(max);
        assert!((after - 20.8).abs() < 1e-3);
    }

    #[test]
    fn regen_caps_at_max() {
        let max: f32 = 60.0;
        let cur: f32 = 55.0;
        let after = (cur + max * ORG_REGEN_PER_DAY).min(max);
        assert_eq!(after, max);
    }
}
