//! 军事相关数据：子单位（subunit / battalion）、战斗战术、师编制模板。

use std::collections::HashMap;

// ─── 子单位（兵种） ──────────────────────────────────────────────

/// 单个 battalion 的基础属性（来自 `common/units/<name>.txt`）。
///
/// 我们仅保留战斗/经济相关字段；地形修正存为子表
#[derive(Debug, Clone, Default)]
pub struct SubunitDef {
    /// 唯一 key（如 `infantry` / `medium_armor` / `artillery`）
    pub key: String,
    /// 缩写（"INF" / "MTK" / "MAR"）
    pub abbreviation: String,
    /// 种类，用于 doctrine 加成（infantry / armor / motorized / mechanized / cavalry / artillery）
    pub group: String,
    /// 类型标签（如 ["infantry"] 或 ["armor"]），用于 tech 加成
    pub types: Vec<String>,

    // ─── 战斗 ───
    /// 占用的战斗宽度
    pub combat_width: f32,
    /// 软攻
    pub soft_attack: f32,
    /// 硬攻
    pub hard_attack: f32,
    /// 防御
    pub defense: f32,
    /// 突破
    pub breakthrough: f32,
    /// 装甲
    pub armor_value: f32,
    /// AP 攻
    pub ap_attack: f32,
    /// 硬度（0..1，1 = 完全硬目标）
    pub hardness: f32,

    // ─── 编制 ───
    /// 满员强度
    pub max_strength: f32,
    /// 最大组织度
    pub max_organisation: f32,
    /// 默认初始士气（解锁此 unit 时给的 default morale）
    pub default_morale: f32,
    /// 训练所需基础人力
    pub manpower: u32,
    /// 训练时间（天）
    pub training_time: u32,
    /// 镇压能力
    pub suppression: f32,

    // ─── 后勤 ───
    /// 每日补给消耗（HOI4 单位）
    pub supply_consumption: f32,
    /// 行进/部署时的"重量"（影响补给消耗）
    pub weight: f32,

    /// 装备需求 `equipment_archetype → 数量`
    pub need: HashMap<String, u32>,

    /// 类别集合（如 `category_front_line` / `category_all_infantry`）
    pub categories: Vec<String>,
}

impl SubunitDef {
    /// 是否陆地兵种（vs 海/空）
    pub fn is_land(&self) -> bool {
        self.types.iter().any(|t| {
            matches!(
                t.as_str(),
                "infantry" | "armor" | "cavalry" | "motorized" | "mechanized" | "artillery"
            )
        })
    }
}

// ─── 战斗战术 ──────────────────────────────────────────────────────

/// 战斗战术（来自 `common/combat_tactics.txt`）。
///
/// HOI4 中战术每 12 小时随机一次，weighted 选择，给战斗双方加 / 减 buff。
/// 我们只保留核心数值。
#[derive(Debug, Clone)]
pub struct CombatTactic {
    pub key: String,
    /// 是否仅供进攻方
    pub is_attacker: bool,
    /// 选择权重基值（HOI4 base.factor）
    pub base_weight: f32,
    /// 进攻方加成（攻击 ×）
    pub attacker_bonus: f32,
    /// 防御方加成
    pub defender_bonus: f32,
    /// 移动加成
    pub movement_bonus: f32,
    /// 被这些战术克制（克制方权重×4）
    pub countered_by: Vec<String>,
}

impl Default for CombatTactic {
    fn default() -> Self {
        Self {
            key: String::new(),
            is_attacker: false,
            base_weight: 1.0,
            attacker_bonus: 0.0,
            defender_bonus: 0.0,
            movement_bonus: 0.0,
            countered_by: Vec::new(),
        }
    }
}

// ─── 师编制模板 ────────────────────────────────────────────────────

/// 一个师的编制模板（来自 `history/units/<TAG>_1936.txt` 或 `common/units/divisions/`）。
#[derive(Debug, Clone)]
pub struct DivisionTemplate {
    pub name: String,
    /// 所属国家 tag（从文件名推断）
    pub country_tag: Option<String>,
    /// 主战兵团：subunit_key 列表（按出现顺序，未去重 — 相同 key 出现 N 次表示 N 个 battalion）
    pub regiments: Vec<String>,
    /// 支援连：subunit_key 列表
    pub support: Vec<String>,
    /// 师名分组（用于自动命名）
    pub division_names_group: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReinforcementPriority {
    Low,
    #[default]
    Normal,
    High,
}

/// V7 editable HOI4-style template layout: 5x5 line battalion grid plus 5 support slots.
#[derive(Debug, Clone)]
pub struct EditableDivisionTemplate {
    pub id: u32,
    pub name: String,
    pub country_tag: Option<String>,
    pub line_battalions: [[Option<String>; 5]; 5],
    pub support_companies: [Option<String>; 5],
    pub division_names_group: Option<String>,
    pub priority: ReinforcementPriority,
    pub locked_by_history: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TemplatePreview {
    pub combat_width: f32,
    pub manpower: u32,
    pub max_organisation: f32,
    pub soft_attack: f32,
    pub hard_attack: f32,
    pub defense: f32,
    pub breakthrough: f32,
    pub armor_value: f32,
    pub ap_attack: f32,
    pub supply_consumption: f32,
    pub suppression: f32,
    pub equipment_needed: HashMap<String, u32>,
    pub training_days: f32,
    pub stockpile_satisfied_divisions: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateEditorCommand {
    Open(u16),
    Clone(u16),
    Rename(u16, String),
    SetLineBattalion {
        template: u16,
        row: u8,
        col: u8,
        subunit: Option<String>,
    },
    SetSupportCompany {
        template: u16,
        slot: u8,
        subunit: Option<String>,
    },
    Save(u16),
    Delete(u16),
}

impl DivisionTemplate {
    /// 总 battalion 数（含 support）
    pub fn battalion_count(&self) -> usize {
        self.regiments.len() + self.support.len()
    }

    /// 唯一 subunit 类型集合
    pub fn unique_subunit_keys(&self) -> std::collections::HashSet<&str> {
        self.regiments
            .iter()
            .chain(self.support.iter())
            .map(|s| s.as_str())
            .collect()
    }

    pub fn to_editable(&self, id: u32, locked_by_history: bool) -> EditableDivisionTemplate {
        let mut line_battalions = std::array::from_fn(|_| std::array::from_fn(|_| None));
        for (idx, subunit) in self.regiments.iter().take(25).enumerate() {
            line_battalions[idx / 5][idx % 5] = Some(subunit.clone());
        }

        let mut support_companies = std::array::from_fn(|_| None);
        for (idx, subunit) in self.support.iter().take(5).enumerate() {
            support_companies[idx] = Some(subunit.clone());
        }

        EditableDivisionTemplate {
            id,
            name: self.name.clone(),
            country_tag: self.country_tag.clone(),
            line_battalions,
            support_companies,
            division_names_group: self.division_names_group.clone(),
            priority: ReinforcementPriority::Normal,
            locked_by_history,
        }
    }
}

impl EditableDivisionTemplate {
    pub fn empty(id: u32, name: impl Into<String>, country_tag: Option<String>) -> Self {
        Self {
            id,
            name: name.into(),
            country_tag,
            line_battalions: std::array::from_fn(|_| std::array::from_fn(|_| None)),
            support_companies: std::array::from_fn(|_| None),
            division_names_group: None,
            priority: ReinforcementPriority::Normal,
            locked_by_history: false,
        }
    }

    pub fn to_division_template(&self) -> DivisionTemplate {
        let regiments = self
            .line_battalions
            .iter()
            .flat_map(|row| row.iter())
            .filter_map(|slot| slot.clone())
            .collect();
        let support = self
            .support_companies
            .iter()
            .filter_map(|slot| slot.clone())
            .collect();

        DivisionTemplate {
            name: self.name.clone(),
            country_tag: self.country_tag.clone(),
            regiments,
            support,
            division_names_group: self.division_names_group.clone(),
        }
    }
}
