//! `hoi4-script` — Phase 3.8 脚本引擎。
//!
//! HOI4 mod 兼容的核心。原版引擎用 trigger / effect / scope 三件套驱动整个游戏：
//! 国策完成回报、决议触发、事件链、AI 决策、scripted_effect 等都是数据驱动的脚本。
//!
//! ## 模块组织
//! - [`value`]：[`ScriptValue`] —— 运行时通用值
//! - [`scope`]：[`Scope`] / [`ScopeKind`] / [`ScopeChain`] —— THIS / ROOT / FROM / PREV
//! - [`context`]：[`ScriptContext`] —— 引擎一次执行所需的全部输入（World + Scope chain）
//! - [`triggers`]：注册表 + ~25 个常用 trigger
//! - [`effects`]：注册表 + ~25 个常用 effect
//! - [`vars`]：country / global 变量与 flag
//! - [`events`]：[`EventDef`] + [`EventScheduler`]
//! - [`decisions`]：[`DecisionDef`] + [`DecisionCatalog`]
//!
//! 与 `hoi4-logic` 的关系：logic 层负责"非脚本"的硬编码玩法（每日 tick、战斗、生产线），
//! script 层负责"脚本"逻辑（事件触发、effect 执行）。logic 在合适的时机调用 script。

pub mod context;
pub mod decisions;
pub mod effects;
pub mod events;
pub mod scope;
pub mod triggers;
pub mod value;
pub mod vars;

pub use context::{ScriptContext, ScriptError};
pub use decisions::{DecisionCatalog, DecisionInstance};
pub use effects::{run_effect_block, EffectFn, EffectRegistry, EffectReport, ScriptCommand};
pub use events::{EventDef, EventOption, EventScheduler, MeanTimeToHappen};
pub use scope::{Scope, ScopeChain, ScopeKind};
pub use triggers::{eval_trigger_block, TriggerFn, TriggerRegistry};
pub use value::ScriptValue;
pub use vars::{Flags, VarKind, Variables};
