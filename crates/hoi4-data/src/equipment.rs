//! 装备定义（来自 `common/units/equipment/`）

use std::collections::HashMap;

/// 装备类型大类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipmentCategory {
    Infantry,
    Artillery,
    AntiTank,
    AntiAir,
    SupportEquipment,
    Motorized,
    Mechanized,
    Tank,
    Plane,
    Ship,
    Convoy,
    Train,
    Other,
}

impl EquipmentCategory {
    pub const COMBAT_CATEGORY_COUNT: usize = 12;

    pub fn from_str(s: &str) -> Self {
        match s {
            "infantry" | "infantry_equipment" => Self::Infantry,
            "artillery" => Self::Artillery,
            "anti_tank" | "antitank" => Self::AntiTank,
            "anti_air" | "antiair" => Self::AntiAir,
            "support" | "support_equipment" => Self::SupportEquipment,
            "motorized" | "motorized_equipment" => Self::Motorized,
            "mechanized" | "mechanized_equipment" => Self::Mechanized,
            "armor" | "tank" | "light_tank" | "medium_tank" | "heavy_tank" => Self::Tank,
            "fighter" | "cas" | "bomber" | "tactical_bomber" | "strategic_bomber"
            | "naval_bomber" | "transport_plane" | "plane" | "aircraft" => Self::Plane,
            "ship" | "naval_vessel" => Self::Ship,
            "convoy" => Self::Convoy,
            "train" => Self::Train,
            _ => Self::Other,
        }
    }
}

/// 单个装备型号
#[derive(Debug, Clone)]
pub struct EquipmentDef {
    /// 唯一 key，如 `infantry_equipment_1`、`fighter_1`、`tank_medium_1`
    pub key: String,
    /// `is_archetype = yes` 表示这是基础原型（不可直接生产）
    pub is_archetype: bool,
    /// 父原型 key（继承经济参数）
    pub archetype: Option<String>,
    /// 直接父型号（用于切换效率惩罚）
    pub parent: Option<String>,
    /// 大类
    pub category: EquipmentCategory,
    /// 解锁年份（用于分类显示）
    pub year: u16,
    /// 是否可建造（`is_buildable = no` 表示否）
    pub is_buildable: bool,
    /// 单台 IC 成本（HOI4 单位）
    pub build_cost_ic: f32,
    /// 单台资源消耗（如 steel=2 rubber=1）
    pub resources: HashMap<String, f32>,
}

impl EquipmentDef {
    /// 生效的 IC cost（如果是子型号且未声明，从 archetype 继承）
    pub fn effective_cost(&self) -> f32 {
        self.build_cost_ic
    }
}
