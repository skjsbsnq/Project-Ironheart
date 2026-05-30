//! AI 调参常量：cadence、阈值、权重。

// ─── 评估 cadence（游戏天数）──────────────────────────

/// 国策评估：国策周期很长，按国家错峰周检即可，避免每天全国家扫描。
pub const FOCUS_EVAL_CADENCE: u32 = 7;

/// 科技评估：每周一次
pub const RESEARCH_EVAL_CADENCE: u32 = 7;

/// 生产评估：错峰每三周一次，减少高速跑时间时的批量卡顿。
pub const PRODUCTION_EVAL_CADENCE: u32 = 21;

/// 外交评估：每两周一次
pub const DIPLOMACY_EVAL_CADENCE: u32 = 14;

// ─── 建造优先级 ────────────────────────────────────

/// 民工:军工 目标比例（低于此值优先造民工）
pub const CIV_TO_MIL_RATIO_TARGET: f32 = 1.5;

/// 基建低于此值时考虑修基建
pub const INFRA_BUILD_THRESHOLD: u8 = 3;

/// 1939 年后切换为军工优先
pub const MIL_FOCUS_YEAR: u16 = 1939;

// ─── 战争门槛 ─────────────────────────────────────

/// 宣战最少师数
pub const WAR_MIN_DIVISIONS: u32 = 24;

/// 宣战最少总工厂数
pub const WAR_MIN_FACTORIES: u32 = 20;

/// 世界紧张度低于此值不考虑主动宣战
pub const WAR_TENSION_THRESHOLD: f32 = 25.0;

// ─── 研究 ─────────────────────────────────────────

/// 不研究提前超过 N 年的科技
pub const AHEAD_OF_TIME_MAX_YEARS: f32 = 1.0;

/// 学说科技额外加分
pub const DOCTRINE_BONUS: f32 = 15.0;

// ─── 评分权重 ─────────────────────────────────────

/// focus 给 PP 的基础加分
pub const FOCUS_PP_BONUS: f32 = 2.0;

/// focus 给稳定/战争支持的加分
pub const FOCUS_STABILITY_BONUS: f32 = 3.0;

/// focus 给经验的加分
pub const FOCUS_XP_BONUS: f32 = 5.0;

/// focus 给 idea 的加分
pub const FOCUS_IDEA_BONUS: f32 = 5.0;

/// focus 给人力的加分
pub const FOCUS_MANPOWER_BONUS: f32 = 4.0;

/// focus 基础分（有可用 focus 就有分）
pub const FOCUS_BASE_SCORE: f32 = 10.0;

/// 科技解锁装备的加分
pub const TECH_EQUIPMENT_BONUS: f32 = 20.0;

/// 科技解锁子单位的加分
pub const TECH_SUBUNIT_BONUS: f32 = 15.0;

/// 科技解锁建筑的加分
pub const TECH_BUILDING_BONUS: f32 = 15.0;

/// 步兵武器类科技加分
pub const TECH_INFANTRY_BONUS: f32 = 10.0;

/// 工业类科技加分
pub const TECH_INDUSTRY_BONUS: f32 = 12.0;

// ─── 战术 AI（4.2）───────────────────────────────

/// 战术 AI 评估 cadence（天）：每 10 天一次，并按国家错峰。
///
/// 该常量是战术评估周期的唯一真值；`ground_orders::TACTICAL_EVAL_CADENCE_DAYS`
/// 是为兼容外部引用保留的别名，二者必须一致。
pub const TACTICAL_EVAL_CADENCE: u32 = 10;

/// Posture 切换冷却（天）。仿 vanilla "execute_order" 持续性：
/// 师团一旦决定攻 / 守，至少持续 N 天，避免 ratio 抖动来回改向。
pub const POSTURE_STICKY_DAYS: u32 = 14;

/// 力量比 ≥ 此值时进攻
pub const OFFENSIVE_RATIO: f32 = 1.30;

/// 力量比 < 此值时撤退（视作高风险防御）
pub const RETREAT_RATIO: f32 = 0.70;

/// 一个州被对方控制的邻州比例 ≥ 此值即视为可包围目标
pub const ENCIRCLEMENT_RATIO: f32 = 0.50;

/// 一个州被敌人控制的邻州比例 ≥ 此值即视为有被包围风险
pub const AT_RISK_RATIO: f32 = 0.60;

/// 海军 AI：潜艇默认通商破袭，超过此舰队比例的潜艇优先 ConvoyRaiding
pub const SUBMARINE_RAIDING_RATIO: f32 = 0.5;

/// 空军 AI：制空权低于此值时优先 AirSuperiority
pub const AIR_CONTROL_LOW_THRESHOLD: f32 = 0.40;

/// 空军 AI：制空权高于此值时把多余战斗机改为护航 / CAS
pub const AIR_CONTROL_HIGH_THRESHOLD: f32 = 0.65;
