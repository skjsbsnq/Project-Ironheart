//! `hoi4-logic` — 游戏逻辑层。
//!
//! 包含：
//! - [`economy`] — 建筑建造、生产线、资源、人力、燃油（Phase 3.1）
//! - [`research`] — 科技研发、解锁、学说（Phase 3.2）
//! - [`politics`] — PP、国策树、国家精神、意识形态（Phase 3.3）
//! - [`military`] — 师编制、战斗、组织度、战术（Phase 3.4）
//! - [`naval`] — 舰船、舰队、海战、制海权（Phase 3.5）
//! - [`air`] — 空军联队、空战、战略轰炸、空中支援、制空权（Phase 3.6）
//! - [`diplomacy`] — 阵营、宣战、wargoal、世界紧张度、和平会议、傀儡（Phase 3.7）
//!
//! 后续会加入：脚本引擎。

#![allow(dead_code)]

pub mod air;
pub mod balance;
pub mod diplomacy;
pub mod economy;
pub mod feedback;
pub mod military;
pub mod naval;
pub mod occupation;
pub mod politics;
pub mod research;
pub mod scripted_effects;
pub mod trade;

pub use air::{AirBattleOutcome, AirBattleSide, AirControl, AirWingStats, GroundSupport};
pub use diplomacy::{
    add_delayed_war_participant, cleanup_war, declare_war, evict_capitulated_daily,
    peace_conference, reconcile_war_membership, remove_empty_wars, set_puppet, set_war_join_policy,
    start_justification, tick_peace_resolution_daily, white_peace, FactionError, PeaceOutcome,
    PeaceResolutionOutcome, ResolvedWarInfo, SurrenderEvaluation, SurrenderOutcome,
    SurrenderWarCleanupResult, TensionContributors, TriggeredEvent, WarError, WargoalError,
};
pub use economy::{
    tick_daily_v6, BuildOrder, ConstructionFundingSource, ConstructionItem, ConstructionQueue,
    EconomicSystemTick, EconomyState, GovernmentOrder, GovernmentOrderFundingSource, MarketTick,
    PlannedTick, ProductionLine, ResourceBalance,
};
pub use feedback::{DomainEvent, FeedbackBus};
pub use military::{BattleOutcome, BattleSide, DivisionStats, MilitaryDemand};
pub use naval::{FleetStats, NavalBattleOutcome, NavalBattleSide, SeaControl};
pub use politics::{PoliticsCache, PoliticsError};
pub use research::{
    apply_v6_tech_unlocks, tick_daily_v6 as tick_daily_research_v6, ResearchError, ResearchSlot,
    ResearchState,
};
