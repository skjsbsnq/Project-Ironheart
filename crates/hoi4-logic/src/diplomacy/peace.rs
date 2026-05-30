//! 和平会议。
//!
//! 简化模型：
//! - 战争结束时调用 [`peace_conference`]，传入 war_id + 获胜方
//! - 获胜方的全部 wargoal 被立即兑现：
//!   - `Annex` → 战败方 owner 全部 state 转移给 claimant，并标记国家被吞并
//!   - `TakeState` → 单个 state 转移
//!   - `Liberate` → 单个 state 转移给受害国（HOI4 中是新国家；这里简化为给 claimant）
//!   - `Puppet` → 把战败方设为 claimant 的傀儡（Puppet 等级）
//!   - `ToppleGovernment` → 切换战败方 ruling_party 为 claimant 的同意识形态
//!   - `NavalAccess` → 我们简化为 +20 opinion，不做实际效果
//! - 战争从 wars HashMap 删除；at_war 标记重新计算
//! - 紧张度根据吞并次数累加

use hoi4_state::{CountryId, WarSide, WargoalType, World};

use super::constants::{TENSION_CAP, TENSION_PER_ANNEXATION};

/// 和平会议执行结果（统计）
#[derive(Debug, Clone, Default)]
pub struct PeaceOutcome {
    pub annexed_countries: u32,
    pub states_transferred: u32,
    pub puppets_created: u32,
    pub governments_toppled: u32,
    pub naval_access_granted: u32,
}

/// 执行和平会议。`winning_side` 表示哪一方胜利。
///
/// 仅获胜方的 wargoal 被兑现。
pub fn peace_conference(
    world: &mut World,
    war_id: u32,
    winning_side: WarSide,
) -> Option<PeaceOutcome> {
    let war = world.diplomacy.wars.remove(&war_id)?;
    let mut out = PeaceOutcome::default();

    let wargoals = match winning_side {
        WarSide::Attacker => war.attacker_wargoals.clone(),
        WarSide::Defender => war.defender_wargoals.clone(),
    };

    for wg in wargoals {
        match wg.kind {
            WargoalType::Annex => {
                // 把 wg.target 拥有的全部 state 转给 wg.claimant
                let count = annex_country(world, wg.target, wg.claimant);
                out.annexed_countries += 1;
                out.states_transferred += count;
                world.diplomacy.world_tension =
                    (world.diplomacy.world_tension + TENSION_PER_ANNEXATION).min(TENSION_CAP);
            }
            WargoalType::TakeState | WargoalType::Liberate => {
                if let Some(state) = wg.target_state {
                    if transfer_state(world, state, wg.claimant) {
                        out.states_transferred += 1;
                    }
                }
            }
            WargoalType::Puppet => {
                use super::puppet::set_puppet;
                let _ = set_puppet(
                    world,
                    wg.claimant,
                    wg.target,
                    hoi4_state::AutonomyLevel::Puppet,
                );
                out.puppets_created += 1;
            }
            WargoalType::ToppleGovernment => {
                if let (Some(claimant_ruling), Some(_target_idx)) = (
                    world
                        .countries
                        .ruling_party
                        .get(wg.claimant.0 as usize)
                        .cloned(),
                    Some(wg.target.0 as usize),
                ) {
                    let ti = wg.target.0 as usize;
                    if ti < world.countries.count {
                        world.countries.ruling_party[ti] = claimant_ruling;
                        // J.1.8：被推翻政府的元首肖像随之更新
                        world.refresh_country_leader(wg.target);
                    }
                }
                out.governments_toppled += 1;
            }
            WargoalType::NavalAccess => {
                world.diplomacy.opinions.modify(wg.claimant, wg.target, 20);
                out.naval_access_granted += 1;
            }
        }
    }

    // 重置全部参战国的 at_war
    let participants: Vec<CountryId> = war
        .attackers
        .iter()
        .chain(war.defenders.iter())
        .copied()
        .collect();
    for c in participants {
        let ci = c.0 as usize;
        if ci < world.countries.count {
            world.countries.at_war[ci] = world.diplomacy.is_at_war(c);
        }
    }

    Some(out)
}

/// 把 victim 全部 state 转给 winner，并标记 victim 已被吞并。
/// 返回转移的 state 数。
fn annex_country(world: &mut World, victim: CountryId, winner: CountryId) -> u32 {
    let mut count = 0u32;
    for i in 0..world.states.count {
        if world.states.owners[i] == victim {
            world.states.owners[i] = winner;
            world.states.controllers[i] = winner;
            // 同时刷新 province ownership
            let provs = world.states.provinces[i].clone();
            for p in provs {
                let pi = p.0 as usize;
                if pi < world.provinces.count {
                    world.provinces.owners[pi] = winner;
                    world.provinces.controllers[pi] = winner;
                }
            }
            count += 1;
        }
    }
    hoi4_state::scripted_effects::remove_country_control(world, victim);
    if count > 0 {
        world.diplomacy.annexed_countries.insert(victim);
        // 受害国的 at_war 视为不在战
        let vi = victim.0 as usize;
        if vi < world.countries.count {
            world.countries.at_war[vi] = false;
        }
    }
    world.recalc_country_caches();
    count
}

/// 把单个 state 转给 winner（含其省份控制权）。返回是否成功。
fn transfer_state(world: &mut World, state: hoi4_state::StateId, winner: CountryId) -> bool {
    let si = state.0 as usize;
    if si >= world.states.count {
        return false;
    }
    world.states.owners[si] = winner;
    world.states.controllers[si] = winner;
    let provs = world.states.provinces[si].clone();
    for p in provs {
        let pi = p.0 as usize;
        if pi < world.provinces.count {
            world.provinces.owners[pi] = winner;
            world.provinces.controllers[pi] = winner;
        }
    }
    world.recalc_country_caches();
    true
}
