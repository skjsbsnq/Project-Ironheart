//! 政治相关数据类型：意识形态、国家精神（idea）、国策树。

use clausewitz_parser::parser::Block;
use std::collections::HashMap;

// ─── 意识形态 ──────────────────────────────────────────────────────

/// 意识形态（democratic / communism / fascism / neutrality）
#[derive(Debug, Clone)]
pub struct Ideology {
    pub key: String,
    /// 子类型（liberalism / stalinism / nazism …）
    pub types: Vec<String>,
    /// RGB 颜色（HUD 用）
    pub color: [u8; 3],
}

impl Ideology {
    /// 4 种基础意识形态 key
    pub fn vanilla_keys() -> [&'static str; 4] {
        ["democratic", "communism", "fascism", "neutrality"]
    }
}

// ─── 国家精神 (Ideas) ──────────────────────────────────────────────

/// 单条国家精神（如 `general_staff` / `civilian_economy`）。
///
/// HOI4 中 idea 在 `common/ideas/*.txt` 里组织成 `ideas = { country = { foo = {...} } theatre = {...} }`。
/// 我们只关心 country-level idea 的 modifier 列表。
#[derive(Debug, Clone)]
pub struct IdeaDef {
    pub key: String,
    /// 所属类别（country / mobilization_laws / political_advisor / theorist / …）
    pub category: String,
    /// 移除此 idea 的 PP 成本（HOI4: removal_cost；-1 表示不可移除）
    pub removal_cost: i32,
    /// UI 图标 sprite（HOI4: picture，通常形如 `GFX_idea_xxx`）。
    pub picture: Option<String>,
    /// 国家级 modifier。key=modifier 名，value=数值
    pub modifiers: HashMap<String, f32>,
    /// 静态规则（rule = { ... }），暂只保存键名
    pub rules: Vec<String>,
}

// ─── 决议类别 (Decision Categories) ──────────────────────────────────

/// 决议类别元数据（来自 `vanilla-decisions-dir-removed/categories/*.txt`）
#[derive(Debug, Clone)]
pub struct DecisionCategoryDef {
    pub id: String,
    /// 图标 sprite 名
    pub icon: Option<String>,
    /// 该类别的可用条件（trigger block，未求值）
    pub available: Option<Block>,
    /// 是否默认折叠
    pub collapsed: bool,
}

// ─── 决议 (Decision) ──────────────────────────────────────────────

/// 决议定义（来自 `vanilla-decisions-dir-removed/*.txt`）
#[derive(Debug, Clone)]
pub struct DecisionDef {
    pub id: String,
    pub category: String,
    /// allowed trigger（游戏开始时检查一次；不满足则永远不显示）
    pub allowed: Option<Block>,
    /// available trigger（每日检查；不满足则灰显）
    pub available: Option<Block>,
    /// visible trigger（不满足则隐藏）
    pub visible: Option<Block>,
    /// 完成时执行的 effect
    pub complete_effect: Option<Block>,
    /// 移除时执行的 effect（days_remove 到期后）
    pub remove_effect: Option<Block>,
    /// 执行后冷却天数（0 = 无冷却）
    pub days_remove: u32,
    /// 执行所需 PP
    pub cost: f32,
    /// 决议图标（sprite 名，可选）
    pub icon: Option<String>,
    /// 决议在 UI 中显示的名称（loc key，通常 = id）
    pub name: Option<String>,
    /// 仅限特定国家 tag
    pub allowed_country: Option<String>,
    /// fire_only_once 标记
    pub fire_only_once: bool,
    /// days_mission_timeout（任务型决议的超时天数）
    pub days_mission_timeout: u32,
    /// cooldown_days（执行后冷却天数，与 days_remove 分离）
    pub cooldown_days: u32,
}

impl IdeaDef {
    /// 是否影响政治力量增益
    pub fn affects_pp_gain(&self) -> bool {
        self.modifiers.contains_key("political_power_gain")
            || self.modifiers.contains_key("political_power_factor")
    }
}

// ─── 国策树 (National Focus) ───────────────────────────────────────

/// 国策树整体
#[derive(Debug, Clone)]
pub struct FocusTree {
    pub id: String,
    /// 该树绑定的国家（按 tag 过滤；空表示通用 / 后备）
    pub country_tag: Option<String>,
    /// 是否默认（通用） tree
    pub default: bool,
    /// 国策表（focus key → 定义）
    pub focuses: HashMap<String, NationalFocus>,
}

/// 单条国策
#[derive(Debug, Clone)]
pub struct NationalFocus {
    pub id: String,
    /// 所属树 id
    pub tree_id: String,
    /// 完成所需"周数"（HOI4 默认 cost=10 → 70 天）
    pub cost_weeks: f32,
    /// 前置（OR 关系：每个 prerequisite 块内是 AND，多个 prerequisite 块之间是 AND）
    /// 简化：合并为单层 Vec<Vec<String>>，外层 AND，内层 OR
    pub prerequisites: Vec<Vec<String>>,
    /// 互斥国策（同组只能选一个）
    pub mutually_exclusive: Vec<String>,
    /// 完成奖励（保留为原始 Block，运行时由 effect runner 解释）
    pub completion_reward: Option<Block>,
    /// 选择条件 `available = { ... }`（保留 Block，trigger eval 由脚本引擎处理；3.3 阶段仅作展示）
    pub available: Option<Block>,
    /// 是否资本ulated 时仍可选
    pub available_if_capitulated: bool,
}

impl NationalFocus {
    /// 一个 cost 单位 = 7 天（HOI4 1 cost = 1 周）
    pub const DAYS_PER_COST_UNIT: f32 = 7.0;

    /// 总完成天数
    pub fn cost_days(&self) -> f32 {
        self.cost_weeks * Self::DAYS_PER_COST_UNIT
    }

    /// 检查给定的"已完成国策集合"是否满足前置（所有 prerequisite 行都至少有一个被完成）
    pub fn prereqs_met(&self, completed: &std::collections::HashSet<String>) -> bool {
        for any_of in &self.prerequisites {
            if any_of.is_empty() {
                continue;
            }
            if !any_of.iter().any(|p| completed.contains(p)) {
                return false;
            }
        }
        true
    }
}
