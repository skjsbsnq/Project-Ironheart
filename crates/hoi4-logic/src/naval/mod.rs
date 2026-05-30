//! 海军系统 — Phase 3.5
//!
//! 子模块：
//! - [`stats`]：舰队属性汇总
//! - [`spawn`]：舰船 / 舰队的创建 API
//! - [`battle`]：简化海战解析器
//! - [`sea_control`]：每海区每国家的制海权计算
//! - [`regions`]：战略海区与港口映射
//! - [`movement`]：舰队跨海区移动
//! - [`missions`]：任务 presence、convoy 风险、维修与造船 MVP

pub mod arbiter;
pub mod battle;
pub mod missions;
pub mod movement;
pub mod organisation;
pub mod regions;
pub mod sea_control;
pub mod spawn;
pub mod stats;

pub use battle::{NavalBattleOutcome, NavalBattleSide};
pub use sea_control::SeaControl;
pub use stats::FleetStats;

pub mod constants {
    /// 海战每轮等价的小时数（HOI4 海战计算每 4 小时一轮）
    pub const HOURS_PER_ROUND: u32 = 4;

    /// 每轮 HP 伤害系数（拟合：典型舰队战 1-3 天分胜负）
    pub const HP_DAMAGE_FACTOR: f32 = 0.05;

    /// 每轮组织度伤害系数
    pub const ORG_DAMAGE_FACTOR: f32 = 0.20;

    /// 撤退阈值（一方 fleet HP 比例低于此值即撤离）
    pub const RETREAT_HP_RATIO: f32 = 0.30;

    /// 撤退阈值（一方 fleet org 比例低于此值即撤离）
    pub const RETREAT_ORG_RATIO: f32 = 0.20;

    /// 船团（convoy）在制海权计算中的权重（远小于战舰）
    pub const CONVOY_PRESENCE_WEIGHT: f32 = 0.1;
}
