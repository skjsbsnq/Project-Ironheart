//! 简化战斗解析器。
//!
//! 模型（参照 HOI4，但显著简化）：
//! - 进攻方与防御方各持一个 [`super::DivisionStats`] + 当前 strength/org
//! - 战斗按 4 小时一轮（HOURS_PER_ROUND）
//! - 每轮：
//!   1. 进攻方与防御方各加权随机选择一个战术 ([`select_tactic`])
//!   2. 计算双方有效攻击力 (soft × (1-hardness) + hard × hardness)
//!   3. 攻击除以对方 defense（防御方）/ breakthrough（进攻方被反击时）→ 命中率
//!   4. 伤害基数 = raw_attack × hit_rate；应用战术加成 → 实际伤害
//!   5. 扣减对方 org（×ORG_DAMAGE_FACTOR）和 strength（×STRENGTH_DAMAGE_FACTOR）
//! - 战斗结束条件：双方任一 org ≤ 0，或者超出 hours 上限

use hoi4_data::{CombatTactic, GameData};

use super::constants::{
    COUNTER_TACTIC_WEIGHT_BONUS, DEFAULT_TACTIC_WEIGHT, HOURS_PER_ROUND, ORG_DAMAGE_FACTOR,
    STRENGTH_DAMAGE_FACTOR,
};
use super::stats::DivisionStats;

/// 战斗一方
#[derive(Debug, Clone)]
pub struct BattleSide {
    pub stats: DivisionStats,
    /// 当前 strength（0..1）
    pub strength: f32,
    /// 当前 org（0..max_organisation）
    pub organisation: f32,
}

impl BattleSide {
    pub fn from_stats(stats: DivisionStats) -> Self {
        let org = stats.max_organisation;
        Self {
            stats,
            strength: 1.0,
            organisation: org,
        }
    }

    pub fn is_broken(&self) -> bool {
        // 用 5% 阈值。值得注意：阈值过高会让 simulate 入口立刻 break、
        // 0 轮战斗、双方一直挂着低 org 打死循环。撤退判定走外部逻辑。
        self.organisation <= 0.05 * self.stats.max_organisation
    }
}

/// 战斗结果
#[derive(Debug, Clone)]
pub struct BattleOutcome {
    pub rounds_fought: u32,
    pub attacker_won: bool,
    pub attacker: BattleSide,
    pub defender: BattleSide,
    /// 已选过的战术名（debug 用）
    pub tactic_history: Vec<(String, String)>,
}

/// 极简的确定性 PRNG（xorshift32）— 测试可重现
#[derive(Debug, Clone)]
pub struct DeterministicRng {
    state: u32,
}

impl DeterministicRng {
    pub fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0xDEADBEEF } else { seed },
        }
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
    /// 0..1
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() & 0xFFFF_FFFF) as f32 / u32::MAX as f32
    }
}

/// 选战术：从所有 tactic 里按权重抽样
pub fn select_tactic<'a>(
    tactics: &'a [(&'a String, &'a CombatTactic)],
    is_for_attacker: bool,
    last_enemy_tactic: Option<&str>,
    rng: &mut DeterministicRng,
) -> Option<&'a CombatTactic> {
    let mut weights = Vec::with_capacity(tactics.len());
    let mut total = 0.0;

    for (_, t) in tactics {
        // 基本筛选：is_attacker 是否匹配
        if t.is_attacker != is_for_attacker {
            weights.push(0.0);
            continue;
        }
        let mut w = if t.base_weight > 0.0 {
            t.base_weight
        } else {
            DEFAULT_TACTIC_WEIGHT
        };
        // 克制加成
        if let Some(prev) = last_enemy_tactic {
            if t.countered_by.iter().any(|c| c == prev) {
                w += COUNTER_TACTIC_WEIGHT_BONUS;
            }
        }
        total += w;
        weights.push(w);
    }
    if total <= 0.0 {
        return None;
    }
    let mut roll = rng.next_f32() * total;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return Some(tactics[i].1);
        }
    }
    Some(tactics.last()?.1)
}

/// 模拟一场战斗，最多 `max_hours` 小时
pub fn simulate(
    attacker: BattleSide,
    defender: BattleSide,
    data: &GameData,
    max_hours: u32,
    rng: &mut DeterministicRng,
) -> BattleOutcome {
    let mut atk = attacker;
    let mut def = defender;
    let mut history = Vec::new();
    let tactics: Vec<(&String, &CombatTactic)> = data.combat_tactics.iter().collect();

    let mut hours = 0u32;
    while hours < max_hours {
        // 双方都还能打
        if atk.is_broken() || def.is_broken() {
            break;
        }

        // 选战术（attacker 看 defender 上次选的，反之亦然）
        let last_def_tactic = history.last().map(|(_, d): &(String, String)| d.as_str());
        let last_atk_tactic = history.last().map(|(a, _): &(String, String)| a.as_str());
        let atk_tactic = select_tactic(&tactics, true, last_def_tactic, rng);
        let def_tactic = select_tactic(&tactics, false, last_atk_tactic, rng);

        let atk_atk_bonus = atk_tactic.map(|t| t.attacker_bonus).unwrap_or(0.0);
        let def_def_bonus = def_tactic.map(|t| t.defender_bonus).unwrap_or(0.0);

        // 进攻方攻击防御方（防御方用 defense 抵抗）
        let damage_to_def = round_damage(&atk, &def, false, atk_atk_bonus);
        // 防御方反击进攻方（进攻方用 breakthrough 抵抗）
        let damage_to_atk = round_damage(&def, &atk, true, def_def_bonus);

        def.organisation = (def.organisation - damage_to_def * ORG_DAMAGE_FACTOR).max(0.0);
        atk.organisation = (atk.organisation - damage_to_atk * ORG_DAMAGE_FACTOR).max(0.0);
        def.strength = (def.strength - damage_to_def * STRENGTH_DAMAGE_FACTOR).max(0.0);
        atk.strength = (atk.strength - damage_to_atk * STRENGTH_DAMAGE_FACTOR).max(0.0);

        history.push((
            atk_tactic.map(|t| t.key.clone()).unwrap_or_default(),
            def_tactic.map(|t| t.key.clone()).unwrap_or_default(),
        ));

        hours += HOURS_PER_ROUND;
    }

    let attacker_won = !atk.is_broken() && def.is_broken();

    BattleOutcome {
        rounds_fought: hours / HOURS_PER_ROUND,
        attacker_won,
        attacker: atk,
        defender: def,
        tactic_history: history,
    }
}

/// 单轮伤害（攻方对防方）
///
/// `target_is_attacker` 为 true 时，目标（防方参数）是战斗中的进攻方，
/// 应使用其 breakthrough 而非 defense 来抵抗伤害（HOI4 原版规则）。
fn round_damage(
    attacker: &BattleSide,
    defender: &BattleSide,
    target_is_attacker: bool,
    tactic_bonus: f32,
) -> f32 {
    let soft_target = 1.0 - defender.stats.hardness;
    let hard_target = defender.stats.hardness;

    let raw_attack = attacker
        .stats
        .effective_soft(attacker.strength, attacker.organisation)
        * soft_target
        + attacker
            .stats
            .effective_hard(attacker.strength, attacker.organisation)
            * hard_target;

    // AP vs armor：如果 AP < armor，攻击力打折
    let armor_factor = if defender.stats.armor_value > 0.0 {
        let ratio = attacker.stats.ap_attack / defender.stats.armor_value;
        ratio.clamp(0.5, 1.0)
    } else {
        1.0
    };

    // 命中率公式：attack / (attack + effective_def + 1)
    // 防守方用 defense，进攻方被反击时用 breakthrough（进攻时的防御值）
    let effective_def = if target_is_attacker {
        defender.stats.breakthrough
    } else {
        defender.stats.defense
    };
    let hit_rate = raw_attack / (raw_attack + effective_def + 1.0);
    let base_damage = raw_attack * hit_rate;
    base_damage * armor_factor * (1.0 + tactic_bonus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_deterministic() {
        let mut a = DeterministicRng::new(42);
        let mut b = DeterministicRng::new(42);
        for _ in 0..10 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn battle_side_from_stats() {
        let mut stats = DivisionStats::default();
        stats.max_organisation = 60.0;
        let side = BattleSide::from_stats(stats);
        assert_eq!(side.strength, 1.0);
        assert_eq!(side.organisation, 60.0);
        assert!(!side.is_broken());
    }

    #[test]
    fn broken_when_org_zero() {
        let mut stats = DivisionStats::default();
        stats.max_organisation = 60.0;
        let mut side = BattleSide::from_stats(stats);
        side.organisation = 0.0;
        assert!(side.is_broken());
    }
}
