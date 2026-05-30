//! 世界紧张度（world tension）的日常计算。
//!
//! 紧张度 0..100 决定：
//! - 民主国家是否能宣战 / 加入阵营 / 正当化（HOI4 规则；我们暂不实施门槛但累计该值）
//! - 干预决定（主要给 AI 用）
//!
//! 来源：
//! - 进行中的每场战争（参战国数量加权）
//! - 在正当化的 wargoal
//! - 已发生的吞并（一次性 +X 在 [`super::peace`] 里直接加）
//! - 每日少量衰减（避免滚雪球）

use hoi4_state::World;

use super::constants::{
    TENSION_CAP, TENSION_DAILY_DECAY, TENSION_PER_PENDING_WARGOAL_PER_DAY, TENSION_PER_WAR_PER_DAY,
};

/// 单日紧张度变化的贡献明细（用于 debug / UI）。
#[derive(Debug, Clone, Default)]
pub struct TensionContributors {
    pub from_wars: f32,
    pub from_pending_wargoals: f32,
    pub decay: f32,
    /// 当日净变化（正 = 上升）
    pub net_change: f32,
    /// 当日结束时的紧张度
    pub final_tension: f32,
}

/// 每日推进世界紧张度。返回贡献明细。
pub fn tick_world_tension_daily(world: &mut World) -> TensionContributors {
    let mut c = TensionContributors::default();

    // 1. 进行中的战争：每场 +TENSION_PER_WAR_PER_DAY × ln(参战国总数+1)
    for war in world.diplomacy.wars.values() {
        let n = (war.attackers.len() + war.defenders.len()) as f32;
        let scale = (n + 1.0).ln().max(0.5); // 1 vs 1 战争 ≈ 1.0
        c.from_wars += TENSION_PER_WAR_PER_DAY * scale;
    }

    // 2. 正当化中的 wargoal
    let pending: usize = world
        .diplomacy
        .pending_wargoals
        .values()
        .map(|v| v.iter().filter(|w| !w.justified).count())
        .sum();
    c.from_pending_wargoals = pending as f32 * TENSION_PER_PENDING_WARGOAL_PER_DAY;

    // 3. 衰减
    c.decay = -TENSION_DAILY_DECAY * world.diplomacy.world_tension;

    c.net_change = c.from_wars + c.from_pending_wargoals + c.decay;

    let new_t = (world.diplomacy.world_tension + c.net_change).clamp(0.0, TENSION_CAP);
    world.diplomacy.world_tension = new_t;
    c.final_tension = new_t;
    c
}
