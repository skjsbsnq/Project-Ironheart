//! Phase 1.5：AI 接入。
//!
//! 把 `hoi4-ai::StrategicAi` 的 `tick` 挂到 [`crate::schedule::SystemSchedule`]
//! 的 `Ai::Daily` 槽位。orchestrator 内部已经处理 cadence（focus=1d、research=7d、
//! production=30d、diplomacy=14d、tactical=同 default），所以这里的 daily wrapper
//! 只是无脑转发。
//!
//! ## 设计要点
//! - `StrategicAi` 是有状态的（每国 last_xxx_eval cooldown）。它必须以 companion
//!   state 形式跟着 `SimContext` 流转，跟 `EconomyState`/`ResearchState`/`PoliticsCache`/
//!   `ScriptState` 同等地位。
//! - orchestrator 的 `tick` 已经 skip 玩家国家与被吞并国，调用方不需要再过滤。
//! - tick 报告（`StrategicTickReport`）当前丢弃；UI / 日志面板可在 Phase 4 接通。

use hoi4_ai::StrategicAi;

use crate::schedule::SimContext;

const INTERACTIVE_AI_BUCKETS: usize = 8;

/// 战略 AI companion 状态。
pub struct AiState {
    pub strategic: StrategicAi,
}

impl AiState {
    pub fn new(world: &hoi4_state::World) -> Self {
        Self {
            strategic: StrategicAi::new(world),
        }
    }

    pub fn ensure_capacity(&mut self, world: &hoi4_state::World) {
        self.strategic.ensure_capacity(world);
    }
}

/// 每日 AI tick：转发到 [`StrategicAi::tick`]。
pub fn ai_daily(ctx: &mut SimContext) {
    let _report = ctx
        .ai
        .strategic
        .tick(ctx.world, ctx.econ, ctx.research, ctx.v6_db);
}

fn ai_daily_bucket(ctx: &mut SimContext, bucket: usize) {
    let _report = ctx.ai.strategic.tick_bucket(
        ctx.world,
        ctx.econ,
        ctx.research,
        ctx.v6_db,
        bucket,
        INTERACTIVE_AI_BUCKETS,
    );
}

pub fn ai_daily_bucket_0(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 0);
}

pub fn ai_daily_bucket_1(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 1);
}

pub fn ai_daily_bucket_2(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 2);
}

pub fn ai_daily_bucket_3(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 3);
}

pub fn ai_daily_bucket_4(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 4);
}

pub fn ai_daily_bucket_5(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 5);
}

pub fn ai_daily_bucket_6(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 6);
}

pub fn ai_daily_bucket_7(ctx: &mut SimContext) {
    ai_daily_bucket(ctx, 7);
}
