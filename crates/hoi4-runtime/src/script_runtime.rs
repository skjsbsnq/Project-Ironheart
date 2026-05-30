//! Phase 1.4：脚本引擎接入。
//!
//! 把 `hoi4-script` 的运行时（[`EventScheduler`] + [`DecisionCatalog`] + [`Variables`]
//! + [`Flags`] + 内置 trigger / effect 注册表）挂到 [`crate::schedule::SystemSchedule`]：
//!
//! - **每日**：[`EventScheduler::drain_due`] 弹出到期事件 → 对每条事件执行
//!   `immediate` block + 选 option 0 自动决议（v1：无 UI 时由 AI 默认走第一项；
//!   未来 Phase 4 接 UI 后由用户拍板）。
//! - **每日**：[`DecisionCatalog::tick_daily`] 推 days_remove 倒计时 → 对到期实例
//!   执行 `remove_effect`（fallback `complete_effect`）。
//! - **每月**：MTTH 重算 — 把所有 `is_triggered_only == false` 的事件按
//!   `mtth.days` 的指数分布做一次 Bernoulli 抽签（每月一轮），命中且 `trigger`
//!   求值为 true 即立即排入触发队列。
//! - **国策完成**：`hoi4_logic::politics::run_effect_block` 已在 `politics_daily`
//!   内被调用；1.4 不重复 wire（避免重复执行 effect）。Phase 5 全脚本扩容时再
//!   讨论是否切到 `hoi4-script::effects::run_effect_block`。
//!
//! ## 数据加载
//! `events/*.txt` / `vanilla-decisions-dir-removed/*.txt` / `common/scripted_*` 的实际加载是
//! **Phase 5.1** 的工作。Phase 1.4 只把"运行时管线"打通：当 `EventScheduler` /
//! `DecisionCatalog` 为空时所有 daily/monthly 调用都是 no-op；当外部测试或后续
//! 阶段灌入数据时它们立刻开始工作，不需要再改 wiring。

use hoi4_script::{
    decisions::DecisionCatalog,
    effects::{run_effect_block, EffectRegistry, EffectReport},
    events::{EventScheduler, MeanTimeToHappen},
    scope::{Scope, ScopeChain},
    triggers::{eval_trigger_block, TriggerRegistry},
    vars::{Flags, Variables},
};
use hoi4_state::{CountryId, World};

use crate::schedule::SimContext;

/// 脚本运行时的全部 companion state（与 World 一起穿过 [`SimContext`]）。
///
/// 比 `EconomyState` / `ResearchState` / `PoliticsCache` 多包含两个不可变注册表
/// （注册表本身在程序启动时构造一次，运行时只查不改）。
pub struct ScriptState {
    pub events: EventScheduler,
    pub decisions: DecisionCatalog,
    pub vars: Variables,
    pub flags: Flags,
    pub triggers: TriggerRegistry,
    pub effects: EffectRegistry,
    pub effect_reports: Vec<EffectReport>,
    /// MTTH 抽签用的伪随机状态。每次 monthly tick 后更新。
    rng_state: u64,
}

impl ScriptState {
    /// 创建一个空脚本状态（events / decisions 未加载）。
    pub fn new() -> Self {
        Self {
            events: EventScheduler::new(),
            decisions: DecisionCatalog::new(),
            vars: Variables::default(),
            flags: Flags::default(),
            triggers: TriggerRegistry::with_builtins(),
            effects: EffectRegistry::with_builtins(),
            effect_reports: Vec::new(),
            rng_state: 0xDEAD_BEEF_CAFE_F00D,
        }
    }

    /// xorshift64 PRNG，用于 MTTH 抽签。返回 u64。
    fn next_u64(&mut self) -> u64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        if x == 0 {
            x = 0xDEAD_BEEF_CAFE_F00D;
        }
        self.rng_state = x;
        x
    }

    /// 0..1 浮点
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }
}

impl Default for ScriptState {
    fn default() -> Self {
        Self::new()
    }
}

/// 每日脚本 tick：事件队列 + 决议倒计时。
pub fn script_daily(ctx: &mut SimContext) {
    drain_due_events(ctx);
    tick_decisions(ctx);
}

/// 每月脚本 tick：MTTH 重算。
pub fn script_monthly(ctx: &mut SimContext) {
    roll_mtth_events(ctx);
}

// ─── 内部实现 ────────────────────────────────────────────────────

/// 弹出到期事件并执行（immediate + option 0）。
fn drain_due_events(ctx: &mut SimContext) {
    let now = ctx.world.elapsed_hours;
    let due = ctx.script.events.drain_due(now);
    if due.is_empty() {
        return;
    }
    let data = ctx.world.data.clone();
    for (event_id, country_idx) in due {
        // fire_only_once 在 fire_immediate 里处理；这里我们手动模拟同样语义
        let already_fired = ctx.script.events.fired_once.contains(&event_id);
        let ev = match ctx.script.events.find(&event_id) {
            Some(e) => e.clone(),
            None => continue,
        };
        if ev.fire_only_once {
            if already_fired {
                continue;
            }
            ctx.script.events.fired_once.insert(event_id.clone());
        }

        let chain = ScopeChain::new(country_scope(ctx.world, country_idx));

        // immediate
        if let Some(b) = &ev.immediate {
            let report = run_effect_block(
                b,
                ctx.world,
                &data,
                &mut ctx.script.vars,
                &mut ctx.script.flags,
                &chain,
                &ctx.script.effects,
            );
            if !report.is_empty() {
                ctx.script.effect_reports.push(report);
            }
        }

        // 自动选 ai_chance 最高的 option（v1：无 UI 时由 AI 默认走第一项）
        if !ev.options.is_empty() {
            let mut best_idx = 0usize;
            let mut best_w = ev.options[0].ai_chance;
            for (i, opt) in ev.options.iter().enumerate().skip(1) {
                if opt.ai_chance > best_w {
                    best_idx = i;
                    best_w = opt.ai_chance;
                }
            }
            // 若该 option 有 trigger，先求值；不满足则跳过该 option（HOI4 灰显语义）
            let opt = &ev.options[best_idx];
            let allow = match &opt.trigger {
                Some(t) => eval_trigger_block(
                    t,
                    ctx.world,
                    &data,
                    &ctx.script.vars,
                    &ctx.script.flags,
                    &chain,
                    &ctx.script.triggers,
                ),
                None => true,
            };
            if allow {
                if let Some(eff) = &opt.effect {
                    let report = run_effect_block(
                        eff,
                        ctx.world,
                        &data,
                        &mut ctx.script.vars,
                        &mut ctx.script.flags,
                        &chain,
                        &ctx.script.effects,
                    );
                    if !report.is_empty() {
                        ctx.script.effect_reports.push(report);
                    }
                }
            }
        }
    }
}

/// 决议每日 tick + 到期 effect。
fn tick_decisions(ctx: &mut SimContext) {
    let completed = ctx.script.decisions.tick_daily();
    if completed.is_empty() {
        return;
    }
    let data = ctx.world.data.clone();
    for (dec_id, country_idx) in completed {
        let def = match ctx.world.data.decisions.get(&dec_id) {
            Some(d) => d.clone(),
            None => continue,
        };
        let chain = ScopeChain::new(country_scope(ctx.world, country_idx));
        // 优先 remove_effect（days_remove 到期时的语义）；fallback complete_effect
        let block = def.remove_effect.as_ref().or(def.complete_effect.as_ref());
        if let Some(b) = block {
            let report = run_effect_block(
                b,
                ctx.world,
                &data,
                &mut ctx.script.vars,
                &mut ctx.script.flags,
                &chain,
                &ctx.script.effects,
            );
            if !report.is_empty() {
                ctx.script.effect_reports.push(report);
            }
        }
    }
    ctx.script.decisions.purge_completed();
}

/// 每月 MTTH 抽签。对每个非 triggered-only 事件、且配 mtth 的事件：按 1/days
/// 的概率每月命中（指数分布的离散近似）。命中后求 `trigger`，通过则把事件以
/// 立即触发的方式排入队列（按各国分别尝试一次：v1 简化为 ROOT=None 全局 scope，
/// 与原版"global event"语义类似）。
///
/// 因 MTTH 事件大多是 country-event（特定国家 scope），原版引擎按国家逐一抽签。
/// 在 Phase 1.4 / 5.1 之前事件列表为空，本函数是 no-op；将来 5.1 加载后这里
/// 会变成主要 fire 通道。
fn roll_mtth_events(ctx: &mut SimContext) {
    if ctx.script.events.events.is_empty() {
        return;
    }
    let data = ctx.world.data.clone();
    // 收集"待考虑"事件 id：避免在迭代时改 ctx.script.events
    let candidates: Vec<(String, MeanTimeToHappen)> = ctx
        .script
        .events
        .events
        .iter()
        .filter(|e| !e.is_triggered_only)
        .filter_map(|e| e.mtth.as_ref().map(|m| (e.id.clone(), m.clone())))
        .collect();

    if candidates.is_empty() {
        return;
    }

    let now = ctx.world.elapsed_hours;
    let n_countries = ctx.world.countries.count;

    for (event_id, mtth) in candidates {
        let p_per_month = if mtth.days > 0.0 {
            // 30 天内"至少发生一次"的近似概率：1 - (1 - 1/days)^30
            let p_day = 1.0 / (mtth.days as f64);
            1.0 - (1.0 - p_day).powi(30)
        } else {
            0.0
        };
        if p_per_month <= 0.0 {
            continue;
        }

        for ci in 0..n_countries {
            let roll = ctx.script.rng_state; // 仅用于诊断，防止借用问题
            let _ = roll;
            let r = ctx.script.next_f64();
            if r >= p_per_month {
                continue;
            }
            // 求 trigger
            let chain = ScopeChain::new(country_scope(ctx.world, ci as u16));
            let ev_clone = match ctx.script.events.find(&event_id) {
                Some(e) => e.clone(),
                None => break,
            };
            let trig_ok = match &ev_clone.trigger {
                Some(t) => eval_trigger_block(
                    t,
                    ctx.world,
                    &data,
                    &ctx.script.vars,
                    &ctx.script.flags,
                    &chain,
                    &ctx.script.triggers,
                ),
                None => true,
            };
            if trig_ok {
                ctx.script.events.queue(&event_id, ci as u16, now);
            }
        }
    }
}

/// 把 country_idx 转换为 ScopeChain 用的 Scope。
fn country_scope(world: &World, idx: u16) -> Scope {
    if (idx as usize) < world.countries.count {
        Scope::Country(CountryId(idx))
    } else {
        Scope::None
    }
}
