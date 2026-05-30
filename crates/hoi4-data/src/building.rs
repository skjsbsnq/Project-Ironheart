//! 建筑类型定义（来自 `common/buildings/`）

/// 建筑分类（决定 cap 计算与图标）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingScope {
    /// 州级建筑（一个州一份），如 industrial_complex / arms_factory
    State,
    /// 省级建筑（每个省独立计数），如 naval_base / bunker
    Province,
}

/// 建筑定义
#[derive(Debug, Clone)]
pub struct BuildingDef {
    /// 建筑 key，如 `industrial_complex`、`arms_factory`、`infrastructure`
    pub key: String,
    /// 建造一级所需 IC（HOI4 单位：4 IC/天/CIC）
    pub base_cost: f32,
    /// 每升一级额外加成（部分建筑使用 base_cost_conversion / per_level_extra_cost）
    pub per_level_extra_cost: f32,
    /// 是否基建建筑
    pub is_infrastructure: bool,
    /// 是否军工厂（产生 MIC）
    pub is_military: bool,
    /// 是否民用工厂（产生 CIC）
    pub is_civilian: bool,
    /// 是否海军船坞
    pub is_dockyard: bool,
    /// 是否空军基地
    pub is_air_base: bool,
    /// 是否海军基地
    pub is_naval_base: bool,
    /// 州级 / 省级 / 共享槽位
    pub scope: BuildingScope,
    /// 单州最大等级（State 级建筑）；None 表示无限制
    pub state_max: Option<u8>,
    /// 单省最大等级（Province 级建筑）；None 表示无限制
    pub province_max: Option<u8>,
    /// 是否与其它建筑共享 state 建筑槽（industrial_complex/arms_factory 共享）
    pub shares_slots: bool,
}

impl BuildingDef {
    /// 总建造 cost = base_cost + (level - 1) × per_level_extra_cost
    pub fn cost_for_level(&self, target_level: u8) -> f32 {
        if target_level == 0 {
            return 0.0;
        }
        self.base_cost + (target_level as f32 - 1.0) * self.per_level_extra_cost
    }
}
