//! 外交系统 — Phase 3.7
//!
//! 子模块：
//! - [`factions`]：阵营创建 / 邀请 / 加入 / 退出 / 踢出 / 解散
//! - [`wargoal`]：战争目标的正当化（PP 消耗 + 天数）
//! - [`war`]：宣战（阵营连带入战）/ 白和 / 投降判定
//! - [`tension`]：世界紧张度的日常累加
//! - [`peace`]：和平会议（执行获胜方 wargoal：吞并 / 占州 / 傀儡 / 推翻政府）
//! - [`puppet`]：傀儡自治度的日常进度与升级 / 整合

pub mod action;
pub mod factions;
pub mod peace;
pub mod puppet;
pub mod surrender;
pub mod tension;
pub mod war;
pub mod wargoal;

pub use action::{
    debug_grant_justified_wargoal, evaluate_action, execute_action, preview_action,
    tick_diplomatic_requests, ActionAvailability, ActionOutcome, ActionPreview, DiplomacyError,
    DiplomaticAction, UnavailableReason,
};
pub use factions::{FactionError, FactionOp};
pub use peace::{peace_conference, PeaceOutcome};
pub use puppet::{set_puppet, tick_autonomy_daily, AutonomyError};
pub use surrender::{
    apply_scripted_surrenders, evaluate_scripted_surrender, resolve_surrender_war_cleanup,
    SurrenderEvaluation, SurrenderOutcome, SurrenderWarCleanupResult, TriggeredEvent,
};
pub use tension::{tick_world_tension_daily, TensionContributors};
pub use war::{
    add_delayed_war_participant, add_war_participant, cleanup_war, declare_war,
    evict_capitulated_daily, force_declare_war, reconcile_war_membership, remove_empty_wars,
    set_war_join_policy, tick_peace_resolution_daily, white_peace, PeaceResolutionOutcome,
    ResolvedWarInfo, WarError,
};
pub use wargoal::{advance_justification, start_justification, WargoalError};

pub mod constants {
    /// 正当化 wargoal 的基础天数（HOI4: 一般 wargoal ≈ 70 天）
    pub const BASE_JUSTIFY_DAYS: f32 = 70.0;

    /// 正当化 wargoal 的基础 PP 一次性成本
    pub const BASE_JUSTIFY_PP_COST: f32 = 50.0;

    /// 宣战时世界紧张度直接 +X
    pub const TENSION_PER_DECLARATION: f32 = 5.0;

    /// 每个未完成 wargoal 每日给世界紧张度 +X
    pub const TENSION_PER_PENDING_WARGOAL_PER_DAY: f32 = 0.05;

    /// 每场进行中的战争每日给世界紧张度 +X（参战国数量乘）
    pub const TENSION_PER_WAR_PER_DAY: f32 = 0.10;

    /// 每个被吞并国家给世界紧张度 +X（一次性）
    pub const TENSION_PER_ANNEXATION: f32 = 8.0;

    /// 紧张度衰减（每日）
    pub const TENSION_DAILY_DECAY: f32 = 0.02;

    /// 紧张度上限
    pub const TENSION_CAP: f32 = 100.0;

    /// 一国 30 天没参与对手任何攻势即视为投降条件之一（占领首都）
    pub const CAPITULATION_HOLD_DAYS: u32 = 30;

    /// 自治度每日基础增长（受国 IC / 兵力贡献率会乘以此值）
    pub const AUTONOMY_DAILY_BASE: f32 = 0.50;

    /// 主国对受国的整合（取消傀儡）所需自治度阈值（更低 = 越接近 Integrated）
    pub const INTEGRATION_BELOW: f32 = 5.0;
}
