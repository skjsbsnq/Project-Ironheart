//! `hoi4-ai` — 战略 AI（Phase 4.1）。
//!
//! 总体设计：
//!
//! 战略 AI 是"长期规划者"，每若干天评估一次：
//! - 选下一个国策（[`focus`]）
//! - 加入/创建阵营、宣战时机（[`diplomacy`]）
//! - 把民工/军工分给消费品/建造队列/装备生产（[`production`]）
//! - 给空闲的研究槽分配科技（[`research`]）
//!
//! 战术 AI 是"短期指挥官"：
//! - 计算前线（[`frontline`]）
//! - 给陆军做出 进攻/防御/包围 决策（[`ground`]）
//! - 给舰队分配任务（[`naval`]）
//! - 给空军联队分配任务（[`air`]）
//!
//! AI 不是"替玩家操控所有按钮"——它只调用 `hoi4-logic` / `hoi4-state` 的公开 API，
//! 与人类玩家走完全相同的入口；不绕过任何检查。
//!
//! ## 架构图
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │              StrategicAi (orchestrator)                 │
//! │   per-country state + cadence (每 N 天评估一次)         │
//! ├─────────────────────────────────────────────────────────┤
//! │  intent + profile → NationalIntent (每 7 天)           │
//! ├─────────────────────────────────────────────────────────┤
//! │  focus │ diplomacy │ production │ research │  (4.1)     │
//! ├─────────────────────────────────────────────────────────┤
//! │  frontline │ ground │ naval │ air │           (4.2)     │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## 基本原则
//! 1. **纯函数式评分**：每个 AI 子模块输出 `Vec<Scored<T>>`；最高分取舍由 orchestrator 拍板。
//! 2. **避免抖动**：决策有 minimum cadence + cooldown，避免每天来回切。
//! 3. **可观察**：所有评分细节都能输出（用于调试 / UI 展示）。
//! 4. **确定性**：不直接使用 `rand`；通过 `World.random_seed` + 简易 LCG 抖动以保证存档一致。

pub mod air;
pub mod china_theater;
pub mod constants;
pub mod diagnostics;
pub mod diplomacy;
pub mod focus;
pub mod frontline;
pub mod frontline_ai;
pub mod ground;
pub mod ground_orders;
pub mod intent;
pub mod naval;
pub mod orchestrator;
pub mod production;
pub mod profile;
pub mod recruit;
pub mod research;
pub mod scoring;
pub mod strategic_profile;

pub use air::{apply_air_decisions, evaluate_air, AirDecision};
pub use diagnostics::{build_country_summary, AiCountrySummary, AiLogRingbuf, AiStance};
pub use diplomacy::{DiplomacyDecision, DiplomacyEvaluation};
pub use focus::{FocusDecision, FocusScore};
pub use frontline::{compute_front, compute_segment, FrontLine, FrontSegment};
pub use ground::{
    detect_encirclement_opportunities, evaluate_ground, EncirclementTarget, FrontDecision,
    FrontSector, GroundEvaluation, GroundPosture,
};
pub use ground_orders::execute_ground_orders;
pub use ground_orders::TACTICAL_EVAL_CADENCE_DAYS;
pub use intent::{evaluate_intent, BuildFocus, FrontAssignment, NationalIntent, NationalStance};
pub use naval::{apply_naval_decisions, evaluate_naval, NavalDecision};
pub use orchestrator::{AiCadence, StrategicAi, StrategicTickReport};
pub use production::{ProductionDecision, ProductionEvaluation};
pub use profile::AiProfile;
pub use research::{ResearchDecision, ResearchScore};
pub use scoring::{Scored, Scorer};
pub use strategic_profile::{weekly_strategic_tick, StrategicProfileReport};
