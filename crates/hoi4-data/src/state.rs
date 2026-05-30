use crate::country::CountryTag;
use crate::resource::ResourceKind;

/// A state (collection of provinces owned by a country)
#[derive(Debug, Clone)]
pub struct State {
    pub id: u16,
    pub name: String,
    pub manpower: u64,
    pub owner: CountryTag,
    pub cores: Vec<CountryTag>,
    pub provinces: Vec<u16>,
    pub category: String,
    pub infrastructure: u8,
    pub victory_points: Vec<(u16, u8)>, // (province_id, value)
    /// 该州出产的战略资源（基础数量；不含基建加成）
    pub resources: Vec<(ResourceKind, f32)>,
}
