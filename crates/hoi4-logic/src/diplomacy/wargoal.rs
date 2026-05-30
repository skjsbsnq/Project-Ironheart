//! 战争目标（wargoal）的正当化机制。
//!
//! HOI4 流程：选 target + type → 立即扣 PP（initial cost）→ 每天前进 1 天进度 →
//! 满 days 后变为 `justified=true` → 才能据此宣战。
//! 我们简化为：调用 [`start_justification`] 一次性扣 PP，每日通过 [`advance_justification`] 推进。

use hoi4_state::{CountryId, StateId, Wargoal, WargoalType, World};

use super::constants::{BASE_JUSTIFY_DAYS, BASE_JUSTIFY_PP_COST};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WargoalError {
    BadCountry,
    /// PP 不足
    InsufficientPP,
    /// 该 type 必须指定 target_state，但未指定
    MissingTargetState,
    /// 该 type 不应指定 target_state，但指定了
    ExtraneousTargetState,
    /// 已存在同一 (claimant, target, kind, state) 的 wargoal
    DuplicateWargoal,
    /// 目标已被吞并 / 不存在
    BadTarget,
}

/// 开始正当化某 wargoal。返回 `Ok(())` 表示扣 PP + 写入 pending_wargoals。
pub fn start_justification(
    world: &mut World,
    claimant: CountryId,
    target: CountryId,
    kind: WargoalType,
    target_state: Option<StateId>,
) -> Result<(), WargoalError> {
    if claimant.is_none() || claimant.0 as usize >= world.countries.count {
        return Err(WargoalError::BadCountry);
    }
    if target.is_none() || target.0 as usize >= world.countries.count {
        return Err(WargoalError::BadTarget);
    }
    if kind.needs_state() && target_state.is_none() {
        return Err(WargoalError::MissingTargetState);
    }
    if !kind.needs_state() && target_state.is_some() {
        return Err(WargoalError::ExtraneousTargetState);
    }

    // 重复检查
    if let Some(list) = world.diplomacy.pending_wargoals.get(&claimant) {
        if list
            .iter()
            .any(|wg| wg.target == target && wg.kind == kind && wg.target_state == target_state)
        {
            return Err(WargoalError::DuplicateWargoal);
        }
    }

    let ci = claimant.0 as usize;
    if world.countries.political_power[ci] < BASE_JUSTIFY_PP_COST {
        return Err(WargoalError::InsufficientPP);
    }

    // 扣 PP
    world.countries.political_power[ci] -= BASE_JUSTIFY_PP_COST;

    // 计算时长（基于 type；某些 type 更慢）
    let days = match kind {
        WargoalType::Annex => BASE_JUSTIFY_DAYS * 1.5,
        WargoalType::Liberate => BASE_JUSTIFY_DAYS * 1.2,
        WargoalType::Puppet => BASE_JUSTIFY_DAYS * 1.3,
        WargoalType::ToppleGovernment => BASE_JUSTIFY_DAYS * 1.0,
        WargoalType::TakeState => BASE_JUSTIFY_DAYS * 1.0,
        WargoalType::NavalAccess => BASE_JUSTIFY_DAYS * 0.7,
    };

    let wg = Wargoal {
        claimant,
        target,
        kind,
        target_state,
        justified: false,
        justify_progress: 0.0,
        justify_total_days: days,
    };
    world
        .diplomacy
        .pending_wargoals
        .entry(claimant)
        .or_default()
        .push(wg);
    Ok(())
}

/// 每日推进所有正在正当化的 wargoal。
///
/// 满进度 → `justified = true`。返回新增 justified 的 wargoal 数。
pub fn advance_justification(world: &mut World) -> usize {
    let mut newly_done = 0usize;
    for list in world.diplomacy.pending_wargoals.values_mut() {
        for wg in list.iter_mut() {
            if wg.justified {
                continue;
            }
            wg.justify_progress += 1.0;
            if wg.justify_progress >= wg.justify_total_days {
                wg.justified = true;
                newly_done += 1;
            }
        }
    }
    newly_done
}

/// 列出 claimant 已正当化的 wargoal。
pub fn justified_wargoals(world: &World, claimant: CountryId) -> Vec<&Wargoal> {
    world
        .diplomacy
        .pending_wargoals
        .get(&claimant)
        .map(|v| v.iter().filter(|w| w.justified).collect())
        .unwrap_or_default()
}

/// 列出 claimant 全部 wargoal（含正在正当化的）。
pub fn all_wargoals(world: &World, claimant: CountryId) -> &[Wargoal] {
    world
        .diplomacy
        .pending_wargoals
        .get(&claimant)
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

/// 移除某 wargoal（按下标）。
pub fn remove_wargoal(world: &mut World, claimant: CountryId, index: usize) -> Option<Wargoal> {
    let list = world.diplomacy.pending_wargoals.get_mut(&claimant)?;
    if index >= list.len() {
        return None;
    }
    Some(list.remove(index))
}

/// 转移 claimant 全部已正当化的 wargoal 到目标 vec（用于宣战时把 wg 转到 War）。
pub fn drain_justified_into(world: &mut World, claimant: CountryId, target: &mut Vec<Wargoal>) {
    if let Some(list) = world.diplomacy.pending_wargoals.get_mut(&claimant) {
        let mut i = 0;
        while i < list.len() {
            if list[i].justified {
                target.push(list.remove(i));
            } else {
                i += 1;
            }
        }
    }
}
