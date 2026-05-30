//! 空军系统 — Phase 3.6
//!
//! 子模块：
//! - [`stats`]：空军联队属性汇总
//! - [`spawn`]：联队创建 / 飞机增减 / 损失清理
//! - [`battle`]：空战解析器（空中遭遇 + 制空 / 拦截）
//! - [`strategic_bombing`]：战略轰炸 — 摧毁敌方建筑 / 基建
//! - [`air_support`]：CAS 给地面战斗加 buff
//! - [`air_superiority`]：每空区每国制空权计算

pub mod air_superiority;
pub mod air_support;
pub mod arbiter;
pub mod battle;
pub mod operations;
pub mod regions;
pub mod spawn;
pub mod stats;
pub mod strategic_bombing;

pub use air_superiority::AirControl;
pub use air_support::{ground_support_modifier, GroundSupport};
pub use battle::{AirBattleOutcome, AirBattleSide};
pub use stats::AirWingStats;
pub use strategic_bombing::{StrategicBombingOutcome, StrategicBombingTarget};

pub mod constants {
    /// 空战每轮等价的小时数（HOI4 中空战每小时 tick，但我们按 4 小时一轮以与陆/海统一）
    pub const HOURS_PER_ROUND: u32 = 4;

    /// 单架飞机基础 HP（baseline；实际从 AircraftDef.max_hp 取）
    pub const BASE_PLANE_HP: f32 = 30.0;

    /// 每轮空战的损失系数（拟合：典型拦截战 6-12 小时即决出胜负）
    pub const PLANE_LOSS_FACTOR: f32 = 0.0010;

    /// 每轮组织度伤害系数
    pub const ORG_LOSS_FACTOR: f32 = 0.18;

    /// 撤回阈值（联队飞机数 / 满员 < 此值则撤回基地）
    pub const RETREAT_PLANE_RATIO: f32 = 0.30;

    /// 撤回阈值（组织度比例）
    pub const RETREAT_ORG_RATIO: f32 = 0.20;

    /// 战略轰炸每天对单 IC 建筑的伤害比例
    pub const STRAT_BOMB_FACTOR: f32 = 0.0006;

    /// 拦截方对轰炸方的命中加成（轰炸机不灵活）
    pub const INTERCEPTOR_BONUS: f32 = 1.5;

    /// CAS 联队对地面战斗的最大加成（HOI4 ≈ +50%）
    pub const MAX_CAS_BONUS: f32 = 0.50;

    /// 制空权达到此值时 CAS 才发挥满效；以下按比例缩减
    pub const FULL_AIR_CONTROL_THRESHOLD: f32 = 0.75;
}
