//! `hoi4-content` — 自研 RON 内容定义（国策树 / 事件 / 决议）。
//!
//! V5 不解析 vanilla 脚本，所有玩法内容用 RON 格式自研。
//! V6 新增经济/法律/POP/市场数据加载器。

pub mod decision;
pub mod decision_tick;
pub mod eval;
pub mod event;
pub mod event_tick;
pub mod focus;
pub mod focus_tick;
pub mod registry;
pub mod situation;
pub mod surrender;
pub mod v6_loader;
pub mod v7_history_loader;
pub mod validate;

pub use decision::{
    Decision, DecisionCategory, DecisionDb, DecisionMechanicKind, DecisionValidationError,
};
pub use decision_tick::{
    daily_decision_tick, ActivateError, DecisionState, DecisionTickEvent, MissionInstance,
};
pub use eval::{eval_trigger, run_effects, EffectReport, GlobalFlags, PendingTrigger};
pub use event::{Event, EventDb, EventOption, EventScope, EventValidationError};
pub use event_tick::{daily_event_tick, EventScheduler, HiddenEventLogEntry, PendingEvent};
pub use focus::{Effect, Focus, FocusTree, Trigger};
pub use focus_tick::{daily_focus_tick, start_focus, TickResult};
pub use registry::{
    load_scenario_content, load_scenario_content_from_manifest, RegistryError, ScenarioContent,
    ScenarioManifest,
};
pub use situation::{
    ActiveSituation, Intervention, ProgressSource, SituationDb, SituationDef, SituationEffect,
    SituationSide, SituationState, SituationValidationError,
};
pub use surrender::{ScriptedSurrenderDef, SurrenderDb, SurrenderValidationError};
pub use v6_loader::{
    active_pms_for_building, default_pms_for_building, execute_finance_command,
    execute_nationalization, inject_v6_into_world, lock_trade_law_for_planned_economy,
    planned_research_direction_modifier, pm_group_name, production_method_group,
    production_method_group_name, set_law, tick_law_cooldowns, unlock_trade_law_on_leaving_planned,
    EquipmentOutputDef, FinanceCommand, PopClassNeedsDef, PopNeedEntryDef, PopNeedTierDef,
    ProductionMethodDef, TechCategoryDef, TechDef, TechUnlockDef, V6Database,
};
pub use v7_history_loader::{
    HeadOfState1936Def, Historical1936Database, HistoricalCountryEconomyDef, HistoricalDataQuality,
    HistoricalMilitaryProfileDef, HistoricalTradeProfileDef, InitialLawSet,
    MilitaryEquipmentDemandDef, ResourceDepositDef, SectorShares, StateIntegrationDef,
    StatePopulation1936Def, StateResourceDepositDef, TradeRouteKindDef, WorkforceProfileDef,
};
pub use validate::{validate_focus_tree, ValidationError};
